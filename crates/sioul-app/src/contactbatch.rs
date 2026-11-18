// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Many contacts at once behind the Contacts page's selection (wave D, 18
//! November 2026): what the core plans (`sioul_core::contactbatch`) said in
//! words before it runs, then run off the window's thread, its progress in
//! the `contactsBatch` property, stopped on a press, ended by what it could
//! not do, card by card, and undone as a whole: from the status line for its
//! first seconds, then for thirty days from the list of Contacts ⚙ ▸ Address
//! books (`batches`, review F2), each record read off the window's thread. A
//! merge of two address books and an import run the same way (`addressbooks`,
//! `contactimport`). One batch at a time, its Undo included.

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use cxx_qt_lib::QString;
use serde::{Deserialize, Serialize};
use sioul_core::contactbatch::{self, Cannot, Outcome, Plan, Preview, Removed, Step};
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
    /// For the title's move: the cards ticked in its list, by file; none
    /// given, those whose title cannot be anything else (review F18).
    #[serde(default)]
    ticked: Option<Vec<String>>,
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
        _ => return Err(tr().text("batch-ask-none", None)),
    })
}

/// The title's move keeps the cards ticked (none given: the sure ones), each by its file.
fn only_ticked(plan: &mut Plan, ticked: Option<&[String]>) {
    let keep: BTreeSet<String> = match ticked {
        Some(keys) => keys.iter().cloned().collect(),
        None => plan.previews.iter().filter(|p| p.sure).map(|p| p.key.clone()).collect(),
    };
    plan.steps.retain(|(_, step)| keep.contains(&step.path().display().to_string()));
}

/// Who sees what goes into the book a move or a merge fills, as its server
/// last answered (`addressbooks::sharing`), or that it may be shared.
pub(crate) fn shared_line(book: &sioul_core::vdir::Collection) -> String {
    let place = format!("{}/{}", book.account, book.id);
    let known = crate::addressbooks::shared_with(&place);
    sioul_core::addressbooks::shared_line(book, tr(), known.as_deref())
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
    /// Who sees the book a move fills, or "".
    shared: String,
    /// What it does to who may reach the person, crossing a book whose
    /// groups are held back (review V1); for a move, that it stays so else.
    reach: Vec<String>,
    /// "14 cannot change:", then each.
    cannot_line: String,
    cannot: Vec<Cannot>,
    /// The title's move: each card, before and after, ticked at first when sure.
    previews: Vec<Preview>,
}

/// What a move or a group does to who may reach the person (review V1),
/// counted on the cards it changes, by the lists' own step for groups.
fn reach_of(shared: &Shared, asked: &Asked, plan: &Plan) -> Vec<String> {
    let config = load_config();
    let senders = sioul_core::porch::Senders::load(&config);
    let changed: BTreeSet<String> = plan.steps.iter().map(|(_, step)| step.path().display().to_string()).collect();
    let cards: Vec<Contact> = chosen(shared, &asked.keys).into_iter().filter(|c| changed.contains(&c.key)).collect();
    let cards: Vec<&Contact> = cards.iter().collect();
    let group = asked.group.trim().to_string();
    match asked.action.as_str() {
        "move" => sioul_core::addressbooks::find(&asked.book).map(|target| sioul_core::addressbooks::reach_of_move(&cards, &target, &config, &senders, tr()).said).unwrap_or_default(),
        "group-add" | "title" if !group.is_empty() => Some(sioul_core::addressbooks::reach_of_groups(&cards, std::slice::from_ref(&group), &config, &senders, tr())).filter(|s| !s.is_empty()).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// What the batch asked (`text`, JSON {keys, action, book, group}) would do,
/// said before it runs: its count, its sentence, what cannot change and why.
pub(crate) fn plan(shared: &Arc<Shared>, text: &str) -> String {
    let shared_state: &Shared = shared;
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
    let shared = match asked.action.as_str() {
        "move" if count > 0 => sioul_core::addressbooks::find(&asked.book).map(|b| shared_line(&b)).unwrap_or_default(),
        _ => String::new(),
    };
    let reach = if count == 0 { Vec::new() } else { reach_of(shared_state, &asked, &plan) };
    let cannot_line = if plan.cannot.is_empty() { String::new() } else { say("batch-cannot", &[("n", plan.cannot.len().to_string())]) };
    json(&PlanView { count, sentence, note, losses, shared, reach, cannot_line, cannot: plan.cannot, previews: plan.previews })
}

/// Runs the batch asked, as its plan says now; "" once started, else why not.
pub(crate) fn run(qt: &QtThread, shared: &Arc<Shared>, text: &str) -> String {
    let asked: Asked = serde_json::from_str(text).unwrap_or_default();
    let (mut plan, _) = match planned(shared, &asked) {
        Ok(found) => found,
        Err(problem) => return problem,
    };
    if asked.action == "title" {
        only_ticked(&mut plan, asked.ticked.as_deref());
    }
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

/// The one-batch lock taken, its stop given; None while another batch or Undo runs.
fn begin() -> Option<Arc<AtomicBool>> {
    let stop = Arc::new(AtomicBool::new(false));
    let mut running = RUNNING.lock().ok()?;
    if running.is_some() {
        return None;
    }
    *running = Some(Arc::clone(&stop));
    Some(stop)
}

fn end() {
    if let Ok(mut running) = RUNNING.lock() {
        *running = None;
    }
}

/// A batch run off the window's thread: kept, then each card through the
/// doors, its progress shown; at its end, `after` (a merge's book removed)
/// adds its words, the line said with Undo, what failed listed card by card.
/// One at a time: "" once started, else why not.
pub(crate) fn start(qt: &QtThread, shared: &Arc<Shared>, kind: &'static str, steps: Vec<(String, Step)>, removed: Option<Removed>, after: Box<dyn FnOnce(&Outcome) -> String + Send>) -> String {
    if steps.is_empty() {
        return tr().text("batch-ask-none", None);
    }
    let Some(stop) = begin() else { return tr().text("batch-busy", None) };
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    let total = steps.len();
    shown(&qt, serde_json::json!({ "running": true, "done": 0, "total": total, "line": say("batch-running", &[("done", "0".into()), ("total", total.to_string())]), "failed": [] }));
    std::thread::spawn(move || {
        let accounts = accounts_of(&steps);
        let now = jiff::Timestamp::now().as_second();
        let dir = contactbatch::records_dir();
        contactbatch::forget_old(&dir, now);
        let mut last = std::time::Instant::now();
        let ran = contactbatch::run(&dir, kind, steps, removed, now, &stop, tr(), &mut |done, total| {
            // Told a few times a second at most: hundreds of cards never flood the window.
            if done == total || last.elapsed() >= std::time::Duration::from_millis(200) {
                last = std::time::Instant::now();
                shown(&qt, serde_json::json!({ "running": true, "done": done, "total": total, "line": say("batch-running", &[("done", done.to_string()), ("total", total.to_string())]), "failed": [] }));
            }
        });
        end();
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
        crate::mail::offer_back(&qt, &shared, line, move |qt, shared| undo(qt, shared, &id));
        show_batches(&qt);
    });
    String::new()
}

/// A batch undone as a whole, off the window's thread, its progress shown;
/// the line says what came back, and what could not. Inside the one-batch
/// lock (review F23): "" once started, else why not.
pub(crate) fn undo(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let Some(stop) = begin() else { return tr().text("batch-busy", None) };
    let (qt, shared, id) = (qt.clone(), Arc::clone(shared), id.to_string());
    std::thread::spawn(move || {
        let dir = contactbatch::records_dir();
        let accounts = contactbatch::record(&dir, &id).map(|r| accounts_of(&r.steps)).unwrap_or_default();
        let root = sioul_core::vdir::Kind::Contacts.root();
        let mut last = std::time::Instant::now();
        let undone = contactbatch::undo(&dir, &id, &root, &stop, tr(), &mut |done, total| {
            if done == total || last.elapsed() >= std::time::Duration::from_millis(200) {
                last = std::time::Instant::now();
                shown(&qt, serde_json::json!({ "running": true, "done": done, "total": total, "line": say("batch-undoing", &[("done", done.to_string()), ("total", total.to_string())]), "failed": [] }));
            }
        });
        end();
        for account in &accounts {
            crate::pim::nudge(&shared, account);
        }
        crate::pim::show_pim(&qt, &shared);
        let (line, failed) = match undone {
            Ok(undone) => {
                let mut line = say("batch-undone", &[("n", undone.back.to_string())]);
                if !undone.failed.is_empty() {
                    line = format!("{line} {}", say("batch-undone-left", &[("n", undone.failed.len().to_string())]));
                }
                (line, undone.failed)
            }
            Err(problem) => (problem, Vec::new()),
        };
        shown(&qt, serde_json::json!({ "running": false, "line": line, "failed": failed }));
        tell(&qt, &shared, line);
        show_batches(&qt);
    });
    String::new()
}

/// One batch of the last thirty days, as the list of Contacts ⚙ ▸ Address books shows it.
#[derive(Serialize)]
struct BatchLine {
    id: String,
    /// "18 November 2026 · 118 contacts moved."
    line: String,
    /// "Stopped after 40 of 118." or "".
    partly: String,
    /// "" while its Undo can do something, else why not.
    why_not: String,
    /// What its Undo would do, asked before it runs.
    ask: String,
}

/// The batches of the last thirty days, newest first, read off the
/// window's thread; their list comes as the `contactsBatches` property.
pub(crate) fn show_batches(qt: &QtThread) {
    let qt = qt.clone();
    std::thread::spawn(move || {
        let now = jiff::Timestamp::now().as_second();
        let lines: Vec<BatchLine> = contactbatch::list(&contactbatch::records_dir(), now, tr())
            .into_iter()
            .map(|b| {
                let what = say(&format!("batch-done-{}", b.kind), &[("n", b.done.to_string())]);
                BatchLine {
                    line: say("batch-listed", &[("when", sioul_core::view::date(tr(), Some(b.at))), ("what", what)]),
                    partly: if b.done < b.planned { say("batch-listed-partly", &[("done", b.done.to_string()), ("planned", b.planned.to_string())]) } else { String::new() },
                    why_not: if b.why_not.is_empty() { String::new() } else { say("batch-cannot-undo", &[("why", b.why_not)]) },
                    ask: say("batch-undo-ask", &[("n", b.done.to_string())]),
                    id: b.id,
                }
            })
            .collect();
        let text = json(&lines);
        let _ = qt.queue(move |mut sioul| sioul.as_mut().set_contacts_batches(QString::from(&text)));
    });
}
