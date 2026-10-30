// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Paper letters, for the window (docs/porch.md, "Paper letters"): the scans
//! dropped in the inbox folder read in the background (no notification: they
//! wait for the Porch's window, as mail does), then each a card there: who,
//! what, how much, by when; a task for its date, its appointment in the agenda,
//! its scan filed with its project. The place is there before any scan: the
//! folder they go to, opened or added to from the Porch, and whether this
//! computer reads them (`reader`).

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use serde::Serialize;
use sioul_core::letters::{Letter, Letters};
use sioul_core::tasks::{Link, TaskEdit};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

static BUSY: Mutex<()> = Mutex::new(());
/// Why scans could not be read lately (no OCR program): said on the cards' place.
static MISSING: Mutex<String> = Mutex::new(String::new());
/// Whether this computer reads scans, and when that was asked: "" when it
/// does, else how to install what reads them. None until the first tick asks.
static READER: Mutex<Option<(std::time::Instant, String)>> = Mutex::new(None);
/// How long an answer about the reading programs holds: one installed meanwhile is seen within it.
const READER_FRESH: std::time::Duration = std::time::Duration::from_secs(600);

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

/// The scans in the inbox not read yet, read; off the window's thread. A
/// phone reads none (docs/porch.md, "Paper letters"): a computer does, and the
/// cards it makes reach the phone with your notes.
pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    if cfg!(target_os = "android") {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let Some(_busy) = crate::backend::one_at_a_time(&BUSY) else { return };
        let config = load_config();
        let looked = sioul_core::words::Words::of(&config);
        // Whether reading works here, said on the Porch before any scan comes; asked again every ten minutes.
        let stale = READER.lock().map(|r| r.as_ref().is_none_or(|(at, _)| at.elapsed() >= READER_FRESH)).unwrap_or(false);
        if stale {
            let said = if sioul_sync::ocr::reads_here() { String::new() } else { sioul_sync::ocr::install_hint(&looked.ocr.tesseract) };
            let changed = READER.lock().map(|mut r| {
                let changed = r.as_ref().is_none_or(|(_, before)| *before != said);
                *r = Some((std::time::Instant::now(), said));
                changed
            });
            if changed.unwrap_or(false) {
                let _ = qt.queue(|mut sioul| sioul.as_mut().letters_changed());
            }
        }
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
            let received = std::fs::metadata(&path).and_then(|m| m.modified()).ok().map(|t| jiff::Timestamp::try_from(t).map(|t| t.to_zoned(sioul_core::zone::current()).date()).unwrap_or_else(|_| sioul_core::zone::now().date())).unwrap_or_else(|| sioul_core::zone::now().date());
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
    /// No OCR program, and scans waiting: how to install one.
    missing: String,
    /// No OCR program, before any scan: how to install one ("" when this computer reads scans, or not known yet).
    reader: String,
}

/// A letter's task as its row and its card say it (`task`), by the id this
/// device knows it by: Sioul's UID, or the id a list that keeps no UID gave
/// it (`Letter::task_elsewhere`), so that "Its task" opens the task that is
/// here (review of 679f96a, later 2); "" when the tasks known here (`tasks`,
/// None before the first read: as the letter says) hold it not, so that the
/// row offers "A task for that date" again rather than open a task that is
/// not here (review of e5bd27b, later 2).
pub(crate) fn task_shown(letter: &Letter, tasks: Option<&[sioul_core::tasks::Task]>) -> String {
    match tasks {
        Some(tasks) => found_task(letter, tasks).map(|t| t.uid.clone()).unwrap_or_default(),
        None => letter.task.clone(),
    }
}

/// The letter's task among `tasks`: by its UID, or by the id a list that
/// keeps no UID gave it (`Letter::task_in`'s rule).
fn found_task<'a>(letter: &Letter, tasks: &'a [sioul_core::tasks::Task]) -> Option<&'a sioul_core::tasks::Task> {
    if letter.task.is_empty() {
        return None;
    }
    tasks.iter().find(|t| t.uid == letter.task || (!letter.task_elsewhere.is_empty() && t.uid == letter.task_elsewhere))
}

/// A task on disk made for this letter elsewhere, whose letters.toml has
/// not come yet: one that links the letter's scan as filed (`describedby`,
/// `dated::links_scan`, read on every device relative to the notes folder
/// `root`). Adopted rather than a second made (review of 679f96a, later 1).
fn adoptable<'a>(letter: &Letter, tasks: &'a [sioul_core::tasks::Task], root: &Path) -> Option<&'a sioul_core::tasks::Task> {
    let scan = letter.file.replace('\\', "/");
    if !letter.task.is_empty() || scan.is_empty() {
        return None;
    }
    tasks.iter().find(|t| t.links.iter().any(|l| l.rel == "describedby" && sioul_core::dated::links_scan(l.uri.trim(), &scan, root)))
}

/// Its task taken out of the letters that name it, once that task is
/// deleted here (its ten seconds to undo over: `mail::perform`): the letter's
/// date asked is drawn again, and its row offers a task for it. Nothing
/// written when no letter names it. Whether any was.
pub(crate) fn untie_task(root: &Path, uid: &str) -> Result<bool, String> {
    // By Sioul's UID, or by the id a list that keeps no UID gave it here (review of 679f96a, later 2).
    let names = |l: &Letter| !l.task.is_empty() && (l.task == uid || (!l.task_elsewhere.is_empty() && l.task_elsewhere == uid));
    if uid.is_empty() || !Letters::load(root)?.list.iter().any(names) {
        return Ok(false);
    }
    Letters::try_change(root, |letters| {
        let tied: Vec<Letter> = letters.list.iter().filter(|l| names(l)).cloned().collect();
        for mut letter in tied {
            letter.task.clear();
            letter.task_elsewhere.clear();
            letters.put(letter);
        }
        Ok(true)
    })
}

/// The tasks a row of letters may trust to say a letter's task: the plan's,
/// while it is current (`PlanShown::current`: no write since it was read).
/// After any write (a task just made for a letter among them) none, so that
/// the row says the letter's own task until the plan is made again, never ""
/// for a task the plan has not read yet (review of 21436bf, M1).
fn tasks_trusted(plan: Option<Arc<crate::work::PlanShown>>) -> Option<Arc<sioul_core::links::Loaded>> {
    trusted_if(plan, crate::work::PlanShown::current)
}

/// `tasks_trusted`, whether a plan is current asked of `current` (the
/// tests', which other tests' writes would make flaky through the stamp).
fn trusted_if(plan: Option<Arc<crate::work::PlanShown>>, current: impl Fn(&crate::work::PlanShown) -> bool) -> Option<Arc<sioul_core::links::Loaded>> {
    plan.filter(|p| current(p)).map(|p| Arc::clone(p.loaded()))
}

/// A file just removed (its text, read before): when it held a task, the
/// letters naming that task are untied (`untie_task`), by the task's own UID
/// (`tasks::task_of_text`), never another component's (an alarm's UID, RFC
/// 9074); an event or a time block unties nothing (review of 21436bf, later
/// 4). Whether any letter was.
pub(crate) fn untie_removed(root: &Path, text: &str) -> bool {
    sioul_core::tasks::task_of_text(text, &sioul_core::zone::current()).is_some_and(|task| untie_task(root, &task.uid).unwrap_or(false))
}

/// The letters not done with, as JSON; each letter's task as the tasks
/// known here say it (`task_shown`, the plan's while current: nothing read here).
pub(crate) fn view() -> String {
    let mut view = View {
        inbox: inbox().map(|p| p.display().to_string()).unwrap_or_default(),
        missing: MISSING.lock().map(|m| m.clone()).unwrap_or_default(),
        reader: READER.lock().ok().and_then(|r| r.as_ref().map(|(_, said)| said.clone())).unwrap_or_default(),
        ..View::default()
    };
    let Some(root) = load_config().notes_root_path() else { return json(&view) };
    let Ok(letters) = Letters::load(&root) else { return json(&view) };
    let today = sioul_core::zone::now().date();
    let known = tasks_trusted(crate::work::plan_shown());
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
            task: task_shown(letter, known.as_ref().map(|l| l.tasks.as_slice())),
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

/// The letter's own task whose deletion waits for its Undo (`removed`): its file.
fn deleted_task_of(letter: &Letter, all: &[sioul_core::tasks::Task], removed: &std::collections::BTreeSet<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    all.iter().filter(|t| letter.task_in(std::slice::from_ref(*t))).map(|t| std::path::PathBuf::from(&t.key)).find(|path| removed.contains(path))
}

/// The tasks read, less those whose deletion waits for its Undo (`removed`: `mail::hidden_pim`).
fn here_but(tasks: &[sioul_core::tasks::Task], removed: &std::collections::BTreeSet<std::path::PathBuf>) -> Vec<sioul_core::tasks::Task> {
    tasks.iter().filter(|t| !removed.contains(Path::new(&t.key))).cloned().collect()
}

/// Whether a letter needs a task made for its date: none named, or the one
/// it names in none of this device's lists (deleted, `Letter::task_in`). A
/// task there is never made twice, whatever a row says of it before the plan
/// is made again, and a double press makes one (review of 21436bf, M1).
fn needs_task(letter: &Letter, tasks: &[sioul_core::tasks::Task]) -> bool {
    !letter.task_in(tasks)
}

/// A task for its date, in your usual list, tied to its scan and its project;
/// nothing when the letter's task is there already (`needs_task`), and the
/// task another device made for it adopted (`adoptable`). A task whose
/// deletion waits for its Undo counts as gone: the press makes the new one at
/// once, the deletion still unties the old (review of 679f96a, later 5), and
/// that deletion is done then, its Undo gone, so that it never gives two tasks
/// for one scan (review of ab7daa4, later 16).
pub(crate) fn make_task(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let removed = crate::mail::hidden_pim(shared).0;
    let root = load_config().notes_root_path();
    with_letter(id, |letters, letter| {
        let all = sioul_core::tasks::all(&sioul_core::zone::current());
        let tasks = here_but(&all, &removed);
        if !needs_task(letter, &tasks) {
            return Ok(String::new());
        }
        if let Some(old) = deleted_task_of(letter, &all, &removed) {
            crate::mail::remove_now(qt, shared, &old);
        }
        // Filed first: the task's link holds where the scan stays.
        letters.file_away(letter)?;
        if let Some(found) = root.as_deref().and_then(|root| adoptable(letter, &tasks, root)) {
            letter.task = found.uid.clone();
            return Ok(String::new());
        }
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
    crate::pim::save_event(qt, shared, "", &event_of(&letters, &letter, date, time).to_string(), "").err().unwrap_or_default()
}

/// The event form "Into the agenda" saves for a letter's appointment on
/// `date` (at `time`): an hour long, or to the end of the day for one at
/// 23:00 or later, or the whole day; its scan's link, and the letter's
/// address labelled with its sender ("A letter" without one), so that the
/// event stands for the letter's appointment on every device
/// (`dated::letter_event`) and its tie reads by name (review of part 4, later
/// 11); in the letter's project, as its task is (`make_task`), so that what
/// the project keeps from agents keeps the event too (re-check of 857c39e, §3.4).
pub(crate) fn event_of(letters: &Letters, letter: &Letter, date: jiff::civil::Date, time: Option<(i8, i8)>) -> serde_json::Value {
    let (start, end, all_day) = match time {
        Some((h, m)) if h >= 23 => (format!("{date}T{h:02}:{m:02}"), format!("{date}T23:59"), false),
        Some((h, m)) => (format!("{date}T{h:02}:{m:02}"), format!("{date}T{:02}:{m:02}", h + 1), false),
        None => (date.to_string(), date.to_string(), true),
    };
    let sender = letter.reading.sender.trim();
    let label = if sender.is_empty() { tr().text("letter-unknown-sender", None) } else { sender.to_string() };
    serde_json::json!({
        "title": say("letter-event", &[("sender", letter.reading.sender.clone())]),
        "notes": letter.reading.why,
        "start": start,
        "end": end,
        "all_day": all_day,
        "links": [crate::backend::file_url(&letters.file_path(letter)), { "uri": sioul_core::links::letter_uri(&letter.id), "label": label }],
        "projects": if letter.project.is_empty() { Vec::new() } else { vec![letter.project.clone()] },
    })
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

/// The folder scans arrive in, made if it is not there yet; "" without one
/// (no folder chosen and no notes folder), or when it cannot be made.
pub(crate) fn folder() -> String {
    let Some(inbox) = inbox() else { return String::new() };
    if std::fs::create_dir_all(&inbox).is_err() {
        return String::new();
    }
    inbox.display().to_string()
}

/// A name free in `folder` for a file named `name`: itself, else "scan (2).pdf", "scan (3).pdf"…
fn free_name(folder: &Path, name: &str) -> PathBuf {
    let first = folder.join(name);
    if !first.exists() {
        return first;
    }
    let path = Path::new(name);
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let extension = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..)
        .map(|n| folder.join(format!("{stem} ({n}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

/// Scans chosen on this computer (a JSON array of `file://` addresses, or of
/// paths), copied into the folder they are read from: what happened, in a
/// sentence. The copy is read at a coming minute, as a scanner's file is; the
/// chosen file stays where it was.
pub(crate) fn add(files: &str) -> String {
    let Some(inbox) = inbox() else { return tr().text("letters-no-folder", None) };
    let chosen: Vec<String> = serde_json::from_str(files).unwrap_or_default();
    if let Err(e) = std::fs::create_dir_all(&inbox) {
        return say("letters-folder-failed", &[("folder", inbox.display().to_string()), ("detail", e.to_string())]);
    }
    let mut copied = 0usize;
    let mut problems: Vec<String> = Vec::new();
    for url in chosen.iter().filter(|u| !u.trim().is_empty()) {
        let source = crate::backend::local_path(url);
        let name = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let readable = source.extension().is_some_and(|e| SCANS.contains(&e.to_string_lossy().to_ascii_lowercase().as_str()));
        if !readable {
            problems.push(say("letters-add-not-scan", &[("name", name)]));
            continue;
        }
        match std::fs::copy(&source, free_name(&inbox, &name)) {
            Ok(_) => copied += 1,
            Err(e) => problems.push(say("letters-add-failed", &[("name", name), ("detail", e.to_string())])),
        }
    }
    let mut said: Vec<String> = Vec::new();
    if copied > 0 {
        said.push(tr().text("letters-added", Some(&tr().counted(copied))));
    }
    said.extend(problems);
    said.join(" ")
}

#[cfg(test)]
mod tests {
    use super::free_name;

    #[test]
    fn a_scan_added_twice_keeps_both() {
        let folder = std::env::temp_dir().join(format!("sioul-letters-free-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(free_name(&folder, "scan.pdf"), folder.join("scan.pdf"));
        std::fs::write(folder.join("scan.pdf"), b"%PDF").unwrap();
        assert_eq!(free_name(&folder, "scan.pdf"), folder.join("scan (2).pdf"));
        std::fs::write(folder.join("scan (2).pdf"), b"%PDF").unwrap();
        assert_eq!(free_name(&folder, "scan.pdf"), folder.join("scan (3).pdf"));
        assert_eq!(free_name(&folder, "photo"), folder.join("photo"));
        std::fs::remove_dir_all(&folder).unwrap();
    }
    /// A letter's task as its row and card say it: "" when the tasks known
    /// here hold it not (deleted, or a Google list's known here by another
    /// id), so that a task for that date is offered again; as the letter says
    /// before the first read (review of e5bd27b, later 2).
    #[test]
    fn a_letter_s_task_as_this_device_knows_it() {
        use super::task_shown;
        let task = sioul_core::tasks::Task { uid: "t2".into(), ..Default::default() };
        let letter = |task: &str| sioul_core::letters::Letter { task: task.into(), ..Default::default() };
        assert_eq!(task_shown(&letter("t1"), Some(std::slice::from_ref(&task))), "");
        assert_eq!(task_shown(&letter("t2"), Some(std::slice::from_ref(&task))), "t2");
        assert_eq!(task_shown(&letter("t1"), None), "t1", "nothing read yet");
        assert_eq!(task_shown(&letter(""), Some(&[])), "");
        // A Google list's task, by the id that list gave it (review of 21436bf, later 3).
        let google = sioul_core::letters::Letter { task: "t9".into(), task_elsewhere: "t2".into(), ..Default::default() };
        assert_eq!(task_shown(&google, Some(std::slice::from_ref(&task))), "t2", "the id this device knows it by (review of 679f96a, later 2)");
    }

    /// A task made elsewhere for a letter whose letters.toml has not come: the
    /// task on disk that links its scan is adopted, none made; one linking
    /// another scan is not; a task waiting for its deletion's Undo is gone
    /// (review of 679f96a, later 1 and later 5).
    #[test]
    fn a_task_made_elsewhere_for_a_letter_is_adopted() {
        use sioul_core::tasks::{Link, Task};
        let root = std::path::Path::new("/notes");
        let scan = |uid: &str, path: &str| Task { uid: uid.into(), key: format!("/lists/{uid}.ics"), links: vec![Link { uri: format!("file:///notes/{path}"), label: String::new(), rel: "describedby".into() }], ..Default::default() };
        let tasks = [scan("other", "letters/2026/2026-10-28 Bank bill.pdf"), scan("made", "letters/2026/2026-10-29 Water bill.pdf")];
        let letter = sioul_core::letters::Letter { file: "letters/2026/2026-10-29 Water bill.pdf".into(), ..Default::default() };
        assert_eq!(super::adoptable(&letter, &tasks, root).map(|t| t.uid.as_str()), Some("made"));
        let named = sioul_core::letters::Letter { task: "kept".into(), ..letter.clone() };
        assert!(super::adoptable(&named, &tasks, root).is_none(), "a letter naming its task adopts none");
        // Its deletion waiting for Undo: gone, a task made at once.
        let removed: std::collections::BTreeSet<std::path::PathBuf> = [std::path::PathBuf::from("/lists/made.ics")].into();
        let here = super::here_but(&tasks, &removed);
        assert!(super::adoptable(&letter, &here, root).is_none());
        let waiting = sioul_core::letters::Letter { task: "made".into(), ..Default::default() };
        assert!(super::needs_task(&waiting, &here) && !super::needs_task(&waiting, &tasks));
        // Its task made again, that deletion is done then (review of ab7daa4, later 16).
        assert_eq!(super::deleted_task_of(&waiting, &tasks, &removed), Some(std::path::PathBuf::from("/lists/made.ics")));
        assert_eq!(super::deleted_task_of(&waiting, &tasks, &Default::default()), None, "none waiting");
        let shared = crate::backend::Shared::default();
        crate::mail::wait_to_remove(&shared, std::path::PathBuf::from("/lists/made.ics"));
        assert!(crate::mail::pending_removal(&shared, std::path::Path::new("/lists/made.ics")).is_some(), "found among the works waiting");
        assert!(crate::mail::pending_removal(&shared, std::path::Path::new("/lists/other.ics")).is_none());
    }

    /// A deletion on the device that knows a Google list's task by Google's id
    /// unties the letter too (review of 679f96a, later 2).
    #[test]
    fn a_task_known_by_its_list_s_id_unties_its_letter() {
        use sioul_core::letters::{Letter, Letters};
        let root = std::env::temp_dir().join(format!("sioul-letters-elsewhere-untie-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Letters::change(&root, |letters| letters.put(Letter { id: "caf".into(), status: "new".into(), task: "sioul-1".into(), task_elsewhere: "g-17".into(), ..Default::default() })).unwrap();
        assert_eq!(super::untie_task(&root, "g-17"), Ok(true));
        let read = Letters::load(&root).unwrap();
        assert_eq!((read.get("caf").unwrap().task.as_str(), read.get("caf").unwrap().task_elsewhere.as_str()), ("", ""));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A letter's task is said as the letter names it until the plan is made
    /// again after a write (a task just made for it): a plan older than a write
    /// is no source to say it is gone (review of 21436bf, M1 and later 9).
    #[test]
    fn a_letter_s_task_is_said_as_written_until_the_plan_is_current() {
        sioul_core::config::stand_ins_only();
        let shared = crate::backend::Shared::default();
        crate::work::publish_now(&shared);
        // A write since the plan was read (the task made for the letter): the plan is not current.
        sioul_core::timeline::note_written();
        let trusted = super::tasks_trusted(crate::work::plan_shown());
        assert!(trusted.is_none(), "an older plan is not trusted");
        let letter = sioul_core::letters::Letter { task: "just-made".into(), ..Default::default() };
        assert_eq!(task_shown_of(&letter, trusted), "just-made", "said as written");
        // A current plan that lacks the task: gone, a task for that date offered again
        // (review of 679f96a, later 7; current asked here, never of the stamp other tests move).
        let current = super::trusted_if(crate::work::plan_shown(), |_| true);
        assert!(current.is_some(), "a current plan is trusted");
        assert_eq!(task_shown_of(&letter, current), "", "a current plan without it: gone");
        crate::work::forget_plan();
    }

    fn task_shown_of(letter: &sioul_core::letters::Letter, known: Option<std::sync::Arc<sioul_core::links::Loaded>>) -> String {
        super::task_shown(letter, known.as_ref().map(|l| l.tasks.as_slice()))
    }

    /// A letter whose task is in a list here gets none made again; one whose
    /// task is gone, or names none, gets one (review of 21436bf, M1).
    #[test]
    fn a_letter_s_task_is_made_once() {
        let there = sioul_core::tasks::Task { uid: "t1".into(), ..Default::default() };
        let letter = |task: &str| sioul_core::letters::Letter { task: task.into(), ..Default::default() };
        assert!(!super::needs_task(&letter("t1"), std::slice::from_ref(&there)), "its task there: none made");
        assert!(super::needs_task(&letter("t1"), &[]), "its task gone: one made");
        assert!(super::needs_task(&letter(""), std::slice::from_ref(&there)), "none named: one made");
    }

    /// A task deleted here is taken out of the letters that name it, those
    /// alone; nothing is written when none does (review of e5bd27b, later 6).
    #[test]
    fn a_deleted_task_frees_its_letter() {
        use sioul_core::letters::{Letter, Letters};
        let root = std::env::temp_dir().join(format!("sioul-letters-untie-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let letter = |id: &str, task: &str| Letter { id: id.into(), status: "new".into(), task: task.into(), ..Default::default() };
        Letters::change(&root, |letters| {
            letters.put(letter("caf", "t1"));
            letters.put(letter("bank", "t2"));
        })
        .unwrap();
        assert_eq!(super::untie_task(&root, "t1"), Ok(true));
        let read = Letters::load(&root).unwrap();
        assert_eq!((read.get("caf").unwrap().task.as_str(), read.get("bank").unwrap().task.as_str()), ("", "t2"));
        let file = Letters::folder(&root).join(sioul_core::letters::MANIFEST);
        let before = std::fs::read(&file).unwrap();
        assert_eq!(super::untie_task(&root, "nobody"), Ok(false));
        assert_eq!(std::fs::read(&file).unwrap(), before, "nothing written");
        let _ = std::fs::remove_dir_all(&root);
    }
    /// A removed file unties the letters naming its task by the task's own
    /// UID, even with an alarm's UID before it; an event and a time block
    /// untie nothing (review of 21436bf, later 4).
    #[test]
    fn a_removed_task_unties_by_its_own_uid() {
        use sioul_core::letters::{Letter, Letters};
        let root = std::env::temp_dir().join(format!("sioul-letters-removed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let letter = |id: &str, task: &str| Letter { id: id.into(), status: "new".into(), task: task.into(), ..Default::default() };
        Letters::change(&root, |letters| {
            letters.put(letter("caf", "task-1"));
            letters.put(letter("bank", "alarm-1"));
            letters.put(letter("rent", "block-1"));
        })
        .unwrap();
        let block = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:block-1\r\nDTSTART:20991023T090000Z\r\nDTEND:20991023T100000Z\r\nSUMMARY:Rent\r\n{}:rent-task\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", sioul_core::blocks::TASK);
        assert!(!super::untie_removed(&root, &block), "a time block unties nothing");
        let event = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:task-1\r\nDTSTART:20991023T090000Z\r\nDTEND:20991023T100000Z\r\nSUMMARY:Same id\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert!(!super::untie_removed(&root, event), "an event unties nothing");
        // An alarm's own UID before the task's (RFC 9074): the task's own unties.
        let task = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nBEGIN:VALARM\r\nUID:alarm-1\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nUID:task-1\r\nSUMMARY:Answer the CAF\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        assert!(super::untie_removed(&root, task));
        let read = Letters::load(&root).unwrap();
        assert_eq!([read.get("caf").unwrap().task.as_str(), read.get("bank").unwrap().task.as_str(), read.get("rent").unwrap().task.as_str()], ["", "alarm-1", "block-1"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
