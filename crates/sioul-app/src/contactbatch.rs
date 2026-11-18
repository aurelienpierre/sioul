// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Many contacts at once behind the Contacts page's selection (wave D, 18
//! November 2026): what the core plans (`sioul_core::contactbatch`) said in
//! words before it runs, then run off the window's thread, its progress in
//! the `contactsBatch` property, stopped on a press, ended by what it could
//! not do, card by card, and undone as a whole from the status line. A merge
//! of two address books and an import run the same way (`addressbooks`,
//! `contactimport`). One batch at a time.

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use cxx_qt_lib::QString;
use serde::{Deserialize, Serialize};
use sioul_core::contactbatch::{self, Cannot, Outcome, Plan, Step};
use sioul_core::contacts::{self, Contact};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The stop of the batch running, while one runs.
static RUNNING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// What the selection asks.
#[derive(Debug, Default, Deserialize)]
struct Asked {
    /// The cards, by file.
    #[serde(default)]
    keys: Vec<String>,
    /// "move", "group-add", "group-remove", "title", "delete".
    action: String,
    /// For a move: the book's place, "account/id".
    #[serde(default)]
    book: String,
    /// For a group's action; for the title's, the group to add them to too ("" none).
    #[serde(default)]
    group: String,
}

/// The cards asked, as they are now, those whose deletion waits left out.
fn chosen(shared: &Shared, keys: &[String]) -> Vec<Contact> {
    let wanted: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
    let (removed, _) = crate::mail::hidden_pim(shared);
    contacts::all().into_iter().filter(|c| wanted.contains(c.key.as_str()) && !removed.contains(Path::new(&c.key))).collect()
}

/// The plan, and the book a move goes to by its name.
fn planned(shared: &Shared, asked: &Asked) -> Result<(Plan, String), String> {
    let cards = chosen(shared, &asked.keys);
    let cards: Vec<&Contact> = cards.iter().collect();
    let group = asked.group.trim().to_string();
    Ok(match asked.action.as_str() {
        "move" => {
            let target = sioul_core::addressbooks::find(&asked.book).ok_or_else(|| tr().text("dav-no-book", None))?;
            let config = load_config();
            let provider = sioul_core::capabilities::provider_of(config.account(&target.account));
            (contactbatch::plan_move(&cards, &target, provider, tr()), target.name)
        }
        "group-add" | "group-remove" if group.is_empty() => return Err(tr().text("batch-needs-group", None)),
        "group-add" => (contactbatch::plan_groups(&cards, std::slice::from_ref(&group), &[], tr()), String::new()),
        "group-remove" => (contactbatch::plan_groups(&cards, &[], std::slice::from_ref(&group), tr()), String::new()),
        "title" => (contactbatch::plan_title(&cards, Some(group.as_str()).filter(|g| !g.is_empty()), tr()), String::new()),
        "delete" => (contactbatch::plan_delete(&cards, tr()), String::new()),
        other => return Err(format!("{other}: no such action")),
    })
}

#[derive(Serialize)]
struct PlanView {
    /// How many it will change.
    count: usize,
    /// "Move 118 contacts to “Professional”?"; "Nothing to change among these."
    sentence: String,
    /// What it keeps, in a sentence: a move keeps each identifier.
    note: String,
    /// "“Google” would not keep: …", or "".
    losses: String,
    /// "14 cannot change:", then each.
    cannot_line: String,
    cannot: Vec<Cannot>,
    /// The title's move: each name before and after.
    previews: Vec<[String; 2]>,
}

/// What the batch asked (`text`, JSON {keys, action, book, group}) would do,
/// said before it runs: its count, its sentence, what cannot change and why.
pub(crate) fn plan(shared: &Arc<Shared>, text: &str) -> String {
    let asked: Asked = serde_json::from_str(text).unwrap_or_default();
    let (plan, book) = match planned(shared, &asked) {
        Ok(found) => found,
        Err(problem) => return json(&serde_json::json!({ "count": 0, "sentence": problem, "cannot": [], "previews": [] })),
    };
    let count = plan.steps.len();
    let n = count.to_string();
    let sentence = if count == 0 {
        tr().text("batch-ask-none", None)
    } else {
        match asked.action.as_str() {
            "move" => say("batch-ask-move", &[("n", n), ("book", book.clone())]),
            "group-add" => say("batch-ask-group-add", &[("n", n), ("group", asked.group.trim().to_string())]),
            "group-remove" => say("batch-ask-group-remove", &[("n", n), ("group", asked.group.trim().to_string())]),
            "title" => say("batch-ask-title", &[("n", n)]),
            _ => say("batch-ask-delete", &[("n", n)]),
        }
    };
    let note = match asked.action.as_str() {
        "move" if count > 0 => tr().text("batch-ask-move-note", None),
        "delete" if count > 0 => tr().text("batch-ask-delete-note", None),
        _ => String::new(),
    };
    let losses = if plan.losses.is_empty() { String::new() } else { say("batch-ask-losses", &[("book", book), ("list", plan.losses.join(", "))]) };
    let cannot_line = if plan.cannot.is_empty() { String::new() } else { say("batch-cannot", &[("n", plan.cannot.len().to_string())]) };
    json(&PlanView { count, sentence, note, losses, cannot_line, cannot: plan.cannot, previews: plan.previews.into_iter().map(|(a, b)| [a, b]).collect() })
}

/// Runs the batch asked, as its plan says now; "" once started, else why not.
pub(crate) fn run(qt: &QtThread, shared: &Arc<Shared>, text: &str) -> String {
    let asked: Asked = serde_json::from_str(text).unwrap_or_default();
    let (plan, _) = match planned(shared, &asked) {
        Ok(found) => found,
        Err(problem) => return problem,
    };
    let kind: &'static str = match asked.action.as_str() {
        "move" => "move",
        "group-add" | "group-remove" => "groups",
        "title" => "title",
        _ => "delete",
    };
    start(qt, shared, kind, plan.steps, None, Box::new(|_: &Outcome| String::new()))
}

/// The batch running stopped before its next card; what was done stays, and its Undo still takes it back.
pub(crate) fn stop() {
    if let Some(stop) = RUNNING.lock().ok().and_then(|r| r.clone()) {
        stop.store(true, Ordering::Relaxed);
    }
}

/// The accounts whose cards a batch touched: each syncs now.
fn accounts_of(steps: &[(String, Step)]) -> BTreeSet<String> {
    steps
        .iter()
        .flat_map(|(_, step)| match step {
            Step::Move { from, to } => vec![from.clone(), to.clone()],
            other => vec![other.path().to_path_buf()],
        })
        .filter_map(|p: PathBuf| crate::pim::account_of(&p))
        .collect()
}

/// The batch's state for the page: running, its count, what failed.
fn shown(qt: &QtThread, view: serde_json::Value) {
    let text = view.to_string();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_contacts_batch(QString::from(&text)));
}

/// What a batch did, in the status line: "118 moved; 14 could not: read only."
fn said(kind: &str, outcome: &Outcome) -> String {
    let n = outcome.done.to_string();
    let done = say(&format!("batch-done-{kind}"), &[("n", n)]);
    let mut line = done;
    if !outcome.failed.is_empty() {
        let mut reasons: Vec<String> = Vec::new();
        for failed in &outcome.failed {
            if !reasons.contains(&failed.why) {
                reasons.push(failed.why.clone());
            }
        }
        line = format!("{line} {}", say("batch-done-failed", &[("n", outcome.failed.len().to_string()), ("why", reasons.join("; "))]));
    }
    if outcome.stopped {
        line = format!("{line} {}", tr().text("batch-stopped", None));
    }
    line
}

/// A batch run off the window's thread: kept, then each card through the
/// doors, its progress shown; at its end, `after` (a merge's book removed)
/// adds its words, the line said with Undo, what failed listed card by card.
/// One at a time: "" once started, else why not.
pub(crate) fn start(qt: &QtThread, shared: &Arc<Shared>, kind: &'static str, steps: Vec<(String, Step)>, deleted_book: Option<String>, after: Box<dyn FnOnce(&Outcome) -> String + Send>) -> String {
    if steps.is_empty() {
        return tr().text("batch-ask-none", None);
    }
    let stop = Arc::new(AtomicBool::new(false));
    {
        let Ok(mut running) = RUNNING.lock() else { return tr().text("batch-busy", None) };
        if running.is_some() {
            return tr().text("batch-busy", None);
        }
        *running = Some(Arc::clone(&stop));
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    let total = steps.len();
    shown(&qt, serde_json::json!({ "running": true, "done": 0, "total": total, "line": say("batch-running", &[("done", "0".into()), ("total", total.to_string())]), "failed": [] }));
    std::thread::spawn(move || {
        let accounts = accounts_of(&steps);
        let now = jiff::Timestamp::now().as_second();
        let dir = contactbatch::records_dir();
        contactbatch::forget_old(&dir, now);
        let mut last = std::time::Instant::now();
        let ran = contactbatch::run(&dir, kind, steps, deleted_book, now, &stop, &mut |done, total| {
            // Told a few times a second at most: hundreds of cards never flood the window.
            if done == total || last.elapsed() >= std::time::Duration::from_millis(200) {
                last = std::time::Instant::now();
                shown(&qt, serde_json::json!({ "running": true, "done": done, "total": total, "line": say("batch-running", &[("done", done.to_string()), ("total", total.to_string())]), "failed": [] }));
            }
        });
        if let Ok(mut running) = RUNNING.lock() {
            *running = None;
        }
        for account in &accounts {
            crate::pim::nudge(&shared, account);
        }
        crate::pim::show_pim(&qt, &shared);
        let outcome = match ran {
            Ok(outcome) => outcome,
            Err(problem) => {
                shown(&qt, serde_json::json!({ "running": false, "line": problem, "failed": [] }));
                tell(&qt, &shared, problem);
                return;
            }
        };
        let more = after(&outcome);
        let line = [said(kind, &outcome), more].into_iter().filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        shown(&qt, serde_json::json!({ "running": false, "line": line, "failed": outcome.failed }));
        if outcome.id.is_empty() {
            tell(&qt, &shared, line);
            return;
        }
        let id = outcome.id.clone();
        crate::mail::offer_back(&qt, &shared, line, move |qt, shared| {
            undo(qt, shared, &id);
            String::new()
        });
    });
    String::new()
}

/// A batch undone as a whole, off the window's thread, its progress shown;
/// the line says what came back, and what could not.
fn undo(qt: &QtThread, shared: &Arc<Shared>, id: &str) {
    let (qt, shared, id) = (qt.clone(), Arc::clone(shared), id.to_string());
    let stop = Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || {
        let dir = contactbatch::records_dir();
        let accounts = contactbatch::record(&dir, &id).map(|r| accounts_of(&r.steps)).unwrap_or_default();
        let root = sioul_core::vdir::Kind::Contacts.root();
        let mut last = std::time::Instant::now();
        let undone = contactbatch::undo(&dir, &id, &root, &stop, &mut |done, total| {
            if done == total || last.elapsed() >= std::time::Duration::from_millis(200) {
                last = std::time::Instant::now();
                shown(&qt, serde_json::json!({ "running": true, "done": done, "total": total, "line": say("batch-undoing", &[("done", done.to_string()), ("total", total.to_string())]), "failed": [] }));
            }
        });
        for account in &accounts {
            crate::pim::nudge(&shared, account);
        }
        crate::pim::show_pim(&qt, &shared);
        let (line, failed) = match undone {
            Ok(undone) => {
                let mut line = say("batch-undone", &[("n", undone.back.to_string())]);
                if undone.aside > 0 {
                    line = format!("{line} {}", say("batch-undone-aside", &[("n", undone.aside.to_string()), ("path", dir.display().to_string())]));
                }
                if !undone.failed.is_empty() {
                    line = format!("{line} {}", say("batch-done-failed", &[("n", undone.failed.len().to_string()), ("why", undone.failed.iter().map(|f| f.why.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>().join("; "))]));
                }
                (line, undone.failed)
            }
            Err(problem) => (problem, Vec::new()),
        };
        shown(&qt, serde_json::json!({ "running": false, "line": line, "failed": failed }));
        tell(&qt, &shared, line);
    });
}
