// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Paper letters, for the window (docs/porch.md, "Paper letters"): the scans
//! dropped in the inbox folder read in the background (no notification: they
//! wait for the Porch's window, as mail does), then each a card there: who,
//! what, how much, by when; a task for its date, its appointment in the agenda,
//! its scan filed with its project.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use serde::Serialize;
use sioul_core::letters::{Letter, Letters};
use sioul_core::tasks::{Link, TaskEdit};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

static BUSY: Mutex<()> = Mutex::new(());
/// Why scans could not be read lately (no OCR program): said on the cards' place.
static MISSING: Mutex<String> = Mutex::new(String::new());

const SCANS: &[&str] = &["pdf", "png", "jpg", "jpeg", "tif", "tiff", "webp"];

/// The inbox: the folder chosen, else `letters/inbox` in the notes folder.
pub(crate) fn inbox() -> Option<PathBuf> {
    let config = load_config();
    match config.letters.inbox.as_deref().filter(|f| !f.trim().is_empty()) {
        Some(folder) => Some(sioul_core::config::expand_home(folder)),
        None => config.notes_root_path().map(|root| Letters::folder(&root).join("inbox")),
    }
}

fn source_of(path: &Path) -> String {
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map_or(0, |m| m.len());
    let time = meta.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    format!("{}|{size}|{time}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
}

/// The scans in the inbox not read yet, read; off the window's thread.
pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let Some(_busy) = crate::backend::one_at_a_time(&BUSY) else { return };
        let config = load_config();
        let looked = sioul_core::words::Words::of(&config);
        let (Some(inbox), Some(root)) = (inbox(), config.notes_root_path()) else { return };
        let Ok(entries) = std::fs::read_dir(&inbox) else { return };
        let Ok(letters) = Letters::load(&root) else { return };
        let known: Vec<String> = letters.list.iter().map(|l| l.source.clone()).collect();
        // Read first (seconds each), then added to the letters as the file holds them then.
        let mut read: Vec<Letter> = Vec::new();
        let projects = sioul_core::projects::ProjectStore::load(&root).map(|s| s.projects).unwrap_or_default();
        let mut added = 0;
        for path in entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|e| SCANS.contains(&e.to_string_lossy().to_ascii_lowercase().as_str()))) {
            let source = source_of(&path);
            if known.contains(&source) {
                continue;
            }
            // A file still being written (a scanner, a sync) is read next time.
            if std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() < 20) {
                continue;
            }
            let received = std::fs::metadata(&path).and_then(|m| m.modified()).ok().map(|t| jiff::Timestamp::try_from(t).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).date()).unwrap_or_else(|_| jiff::Zoned::now().date())).unwrap_or_else(|| jiff::Zoned::now().date());
            let id = format!("{received}-{:08x}", source.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193)));
            let work = sioul_core::config::cache_dir().join("ocr").join(&id);
            let mut letter = Letter { id: id.clone(), file: path.strip_prefix(&root).map_or_else(|_| path.display().to_string(), |p| p.to_string_lossy().replace('\\', "/")), source, received: Some(received), status: "new".into(), ..Letter::default() };
            match sioul_sync::ocr::text_of(&path, &work, &looked.ocr.tesseract) {
                Ok(text) => {
                    letter.reading = sioul_core::letters::read(&looked, &text, received);
                    letter.project = sioul_core::letters::project_of(&text, &letter.reading.sender, &projects).unwrap_or_default();
                    let _ = letters.save_text(&id, &text);
                    if let Ok(mut missing) = MISSING.lock() {
                        missing.clear();
                    }
                }
                Err(sioul_sync::ocr::Unread::Missing(hint)) => {
                    // Kept, unread: it can still be filed by hand; read once a program is there.
                    if let Ok(mut missing) = MISSING.lock() {
                        *missing = hint;
                    }
                    continue;
                }
                Err(sioul_sync::ocr::Unread::Failed(e)) => letter.problem = e,
            }
            read.push(letter);
            added += 1;
        }
        let taken = added > 0
            && Letters::change(&root, |letters| {
                for letter in read {
                    if !letters.list.iter().any(|l| l.source == letter.source) {
                        letters.put(letter);
                    }
                }
            })
            .is_ok();
        if taken {
            let _ = qt.queue(|mut sioul| sioul.as_mut().letters_changed());
            let _ = shared;
        }
    });
}

#[derive(Serialize)]
struct LetterView {
    id: String,
    sender: String,
    kind: &'static str,
    kind_label: String,
    received: String,
    /// What it says, a sentence each: what it asks, by when, registered, your number.
    lines: Vec<String>,
    /// Its scan, where it is on this computer.
    file: String,
    deadline: String,
    appointment: bool,
    task: String,
    project: String,
    problem: String,
}

#[derive(Serialize)]
struct ProjectChoice {
    id: String,
    title: String,
}

#[derive(Serialize, Default)]
struct View {
    inbox: String,
    letters: Vec<LetterView>,
    projects: Vec<ProjectChoice>,
    /// No OCR program: how to install one.
    missing: String,
}

/// The letters not done with, as JSON.
pub(crate) fn view() -> String {
    let mut view = View { inbox: inbox().map(|p| p.display().to_string()).unwrap_or_default(), missing: MISSING.lock().map(|m| m.clone()).unwrap_or_default(), ..View::default() };
    let Some(root) = load_config().notes_root_path() else { return json(&view) };
    let Ok(letters) = Letters::load(&root) else { return json(&view) };
    let today = jiff::Zoned::now().date();
    let projects = sioul_core::projects::ProjectStore::load(&root).map(|s| s.projects).unwrap_or_default();
    view.projects = projects.iter().filter(|c| c.status.as_deref() != Some("closed")).map(|c| ProjectChoice { id: c.id.clone(), title: c.title.clone() }).collect();
    for letter in letters.list.iter().filter(|l| l.status != "done") {
        let r = &letter.reading;
        let day = |d: jiff::civil::Date| tr().day_in(d, today);
        let mut lines = Vec::new();
        if let Some(amount) = r.amount {
            lines.push(say("letter-amount", &[("amount", tr().money(amount))]));
        }
        if let Some(deadline) = r.deadline {
            lines.push(say("letter-deadline", &[("date", day(deadline)), ("why", r.why.clone())]));
        }
        if let Some((date, time)) = r.appointment {
            let when = match time {
                Some((h, m)) => format!("{} {h:02}:{m:02}", day(date)),
                None => day(date),
            };
            lines.push(say("letter-appointment", &[("when", when)]));
        }
        if r.registered {
            lines.push(tr().text("letter-registered", None));
        }
        if let Some(dated) = r.dated {
            lines.push(say("letter-dated", &[("date", day(dated))]));
        }
        if !r.reference.is_empty() {
            lines.push(say("letter-reference", &[("reference", r.reference.clone())]));
        }
        view.letters.push(LetterView {
            id: letter.id.clone(),
            sender: if r.sender.is_empty() { tr().text("letter-unknown-sender", None) } else { r.sender.clone() },
            kind: r.kind.id(),
            kind_label: tr().text(&format!("letter-kind-{}", r.kind.id()), None),
            received: letter.received.map(day).unwrap_or_default(),
            lines,
            file: letters.file_path(letter).display().to_string(),
            deadline: r.deadline.map(|d| d.to_string()).unwrap_or_default(),
            appointment: r.appointment.is_some(),
            task: letter.task.clone(),
            project: letter.project.clone(),
            problem: letter.problem.clone(),
        });
    }
    json(&view)
}

fn with_letter(id: &str, change: impl FnOnce(&mut Letters, &mut Letter) -> Result<String, String>) -> String {
    let result = (|| {
        let root = load_config().notes_root_path().ok_or_else(|| tr().text("papers-no-store", None))?;
        // Read, changed and written under the file's lock: over the letters as they are then.
        Letters::try_change(&root, |letters| {
            let mut letter = letters.get(id).cloned().ok_or_else(|| tr().text("papers-gone", None))?;
            let said = change(letters, &mut letter)?;
            letters.put(letter);
            Ok(said)
        })
    })();
    match result {
        Ok(said) => said,
        Err(e) => e,
    }
}

/// A task for its date, in your usual list, tied to its scan and its project.
pub(crate) fn make_task(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    with_letter(id, |letters, letter| {
        // Filed first: the task's link holds where the scan stays.
        letters.file_away(letter)?;
        let r = &letter.reading;
        let amount = r.amount.map(|a| tr().money(a)).unwrap_or_default();
        let title = say(&format!("letter-task-{}", r.kind.id()), &[("sender", r.sender.clone()), ("amount", amount)]);
        let notes = [r.why.clone(), if r.reference.is_empty() { String::new() } else { say("letter-reference", &[("reference", r.reference.clone())]) }].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n");
        let scan = crate::backend::file_url(&letters.file_path(letter));
        let edit = TaskEdit {
            title: title.trim().to_string(),
            notes,
            due: r.deadline.map(|d| d.to_string()).unwrap_or_default(),
            estimate: 20,
            projects: if letter.project.is_empty() { Vec::new() } else { vec![letter.project.clone()] },
            links: vec![Link { uri: scan, label: tr().text("letter-scan", None), rel: "describedby".into() }],
            ..TaskEdit::default()
        };
        let list = sioul_core::tasks::default_list().map(|l| format!("{}/{}", l.account, l.id));
        let uid = match list {
            Some(list) => crate::work::create_task(qt, shared, &edit, &list)?,
            None => crate::work::local_task(qt, shared, &tr().text("papers-list", None), &edit)?,
        };
        letter.task = uid;
        Ok(String::new())
    })
}

/// Its appointment in the agenda, an hour long, tied to its scan (filed first, so the tie holds).
pub(crate) fn make_event(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let filed = with_letter(id, |letters, letter| letters.file_away(letter).map(|()| String::new()));
    if !filed.is_empty() {
        return filed;
    }
    let root = load_config().notes_root_path();
    let letter = root.as_deref().and_then(|r| Letters::load(r).ok()).and_then(|l| Some((l.get(id)?.clone(), l)));
    let Some((letter, letters)) = letter else { return tr().text("papers-gone", None) };
    let Some((date, time)) = letter.reading.appointment else { return String::new() };
    let (start, end, all_day) = match time {
        // An hour long, or to the end of the day for one at 23:00 or later.
        Some((h, m)) if h >= 23 => (format!("{date}T{h:02}:{m:02}"), format!("{date}T23:59"), false),
        Some((h, m)) => (format!("{date}T{h:02}:{m:02}"), format!("{date}T{:02}:{m:02}", h + 1), false),
        None => (date.to_string(), date.to_string(), true),
    };
    let edit = serde_json::json!({
        "title": say("letter-event", &[("sender", letter.reading.sender.clone())]),
        "notes": letter.reading.why,
        "start": start,
        "end": end,
        "all_day": all_day,
        "links": [crate::backend::file_url(&letters.file_path(&letter))],
    });
    crate::pim::save_event(qt, shared, "", &edit.to_string(), "").err().unwrap_or_default()
}

/// Done with it: its project set, its scan filed in `letters/<year>/`.
pub(crate) fn done(id: &str, project: &str) -> String {
    with_letter(id, |letters, letter| {
        if !project.is_empty() {
            letter.project = project.to_string();
        }
        letters.file_away(letter)?;
        letter.status = "done".into();
        Ok(String::new())
    })
}
