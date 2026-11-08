// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Batch edits in the full lists (8 November 2026): several tasks or events
//! selected in the task list or the Search tab, changed in one act with Undo:
//! their project, kind, category, date, status or list, marked done, or
//! deleted. Every write goes in one `composed::Change`, which the one rule
//! of events guards (`agenda::guard`), and one Undo puts every file back
//! (`agentwrites::Said::put_back`); a deletion waits its ten seconds, as one
//! deletion does (`mail::schedule_removals`).
//!
//! What a batch never does, said item by item: write a meeting with others
//! (the organizer's door is an act of its own, from its card, which tells its
//! guests), nor a calendar or a list you only read; move a repeating event to
//! another day (from its card, one time or all); give a task a kind to an
//! event, which has none, nor mark an event done; move a task into a list
//! that would lose some of it.

use crate::backend::{QtThread, Shared, say, tr};
use crate::{mail, pim};
use serde::{Deserialize, Serialize};
use sioul_core::agenda;
use sioul_core::composed::Change as Written;
use sioul_core::lines;
use sioul_core::tasks::{self, Status, Task, TaskEdit};
use sioul_core::vdir::{self, Collection, Kind};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One thing selected, as the lists name it.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Thing {
    /// "task" or "event".
    pub thing: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub uid: String,
}

/// What changes; each field left out stays as it is.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Asked {
    /// A project's id, or `thingsearch::NONE` for none.
    pub project: Option<String>,
    /// A kind, or `NONE`.
    pub kind: Option<String>,
    /// A category, or `NONE`.
    pub category: Option<String>,
    /// A day, "2026-11-12": a task's date asked (its hour kept), an event moved to it (its time kept);
    /// "" takes a task's date asked away.
    pub date: Option<String>,
    /// A task's ("needs-action", "in-process", "completed", "cancelled") or an event's ("confirmed", "tentative", "cancelled").
    pub status: Option<String>,
    /// A list or a calendar, "account/id".
    pub list: Option<String>,
    pub delete: bool,
}

/// What was left out, and why, in words.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Left {
    pub title: String,
    pub why: String,
    /// Both, as the list says them: "“Team meeting”: a meeting with others: …".
    pub line: String,
}

#[derive(Debug, Serialize)]
struct Answer {
    /// What was done, with Undo offered; "" when nothing was.
    said: String,
    left: Vec<Left>,
    /// What went wrong: nothing written.
    #[serde(skip_serializing_if = "String::is_empty")]
    error: String,
}

/// The change asked of `things` (JSON lists), done in one act with Undo; JSON `Answer`.
pub(crate) fn apply(qt: &QtThread, shared: &Arc<Shared>, things: &str, asked: &str) -> String {
    let things: Vec<Thing> = serde_json::from_str(things).unwrap_or_default();
    let asked: Asked = serde_json::from_str(asked).unwrap_or_default();
    let answer = run(qt, shared, &things, &asked);
    crate::backend::json(&answer)
}

/// What an act will do, before anything is written.
#[derive(Default)]
struct Planned {
    written: Written,
    /// To delete after ten seconds to undo: (account, file).
    removed: Vec<(String, PathBuf)>,
    left: Vec<Left>,
    count: usize,
    /// The tasks this act changes, for their blocks once done or dropped.
    changed: Vec<String>,
}

fn run(qt: &QtThread, shared: &Arc<Shared>, things: &[Thing], asked: &Asked) -> Answer {
    let Planned { written, removed, left, count, changed } = planned(shared, things, asked);
    if !removed.is_empty() {
        let line = tr().text("batch-deleted", Some(&tr().counted(count)));
        mail::schedule_removals(qt, shared, removed, None, line.clone());
        if let Ok(mut cache) = shared.loaded.lock() {
            *cache = None;
        }
        crate::work::show_work(qt, shared);
        pim::show_pim(qt, shared);
        return Answer { said: line, left, error: String::new() };
    }
    if written.is_empty() {
        return Answer { said: String::new(), left, error: String::new() };
    }
    let files = match written.write() {
        Ok(files) => files,
        Err(error) => return Answer { said: String::new(), left, error },
    };
    // Done or dropped: their blocks not begun go, as one task's do (`blocks`).
    if matches!(asked.status.as_deref(), Some("completed" | "cancelled")) {
        for uid in &changed {
            crate::blocks::closed(qt, shared, uid, jiff::Timestamp::now().as_second());
        }
    }
    let accounts: BTreeSet<String> = files.iter().filter_map(|f| pim::account_of(&f.path)).collect();
    for account in &accounts {
        pim::nudge(shared, account);
    }
    crate::work::replan_asked(shared);
    let line = tr().text("batch-changed", Some(&tr().counted(count)));
    let said = sioul_core::agentwrites::Said { at: 0, line: String::new(), files };
    mail::offer_back(qt, shared, line.clone(), move |qt, shared| {
        if !said.put_back() {
            return tr().text("undo-too-late", None);
        }
        for account in &accounts {
            pim::nudge(shared, account);
        }
        if let Ok(mut cache) = shared.loaded.lock() {
            *cache = None;
        }
        crate::work::show_work(qt, shared);
        pim::show_pim(qt, shared);
        String::new()
    });
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    crate::work::show_work(qt, shared);
    pim::show_pim(qt, shared);
    Answer { said: line, left, error: String::new() }
}

/// Each thing's change, or why it is left out; nothing written.
fn planned(shared: &Shared, things: &[Thing], asked: &Asked) -> Planned {
    let loaded = crate::work::loaded(shared);
    let own = agenda::own_addresses();
    let zone = sioul_core::zone::current();
    let now = sioul_core::zone::now();
    let mut left: Vec<Left> = Vec::new();
    let leave = |left: &mut Vec<Left>, title: &str, id: &str| {
        let why = tr().text(id, None);
        left.push(Left { title: title.to_string(), line: say("batch-left-line", &[("title", title.to_string()), ("why", why.clone())]), why });
    };
    let selected: BTreeSet<&str> = things.iter().filter(|t| t.thing == "task").map(|t| t.uid.as_str()).collect();
    let mut written = Written::default();
    let mut removed: Vec<(String, PathBuf)> = Vec::new();
    let mut count = 0usize;
    // The tasks this act changes, for their blocks once done or dropped.
    let mut changed: Vec<String> = Vec::new();
    for thing in things {
        if thing.thing == "task" {
            let Some(task) = loaded.tasks.iter().find(|t| t.uid == thing.uid) else { continue };
            if task.read_only {
                leave(&mut left, &task.title, "batch-left-read-only");
                continue;
            }
            if asked.delete {
                let path = PathBuf::from(&task.key);
                removed.push((pim::account_of(&path).unwrap_or_default(), path));
                removed.extend(crate::blocks::with_task(&task.uid));
                count += 1;
                continue;
            }
            match task_change(task, asked, &loaded, &selected, &zone, &now) {
                Ok(None) => {}
                Ok(Some(change)) => {
                    merge(&mut written, change);
                    changed.push(task.uid.clone());
                    count += 1;
                }
                Err(id) => leave(&mut left, &task.title, &id),
            }
        } else {
            let path = PathBuf::from(&thing.key);
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let title = agenda::edit_of_text(&text, &zone).map(|e| e.title).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| tr().text("agenda-untitled", None));
            // A meeting with others, yours to organize or not: never through the ordinary doors.
            if agenda::meeting_with_others(&text, &own) || agenda::organized_meeting(&text, &own) {
                leave(&mut left, &title, "batch-left-meeting");
                continue;
            }
            if calendar_of(&path).is_some_and(|c| c.read_only) {
                leave(&mut left, &title, "batch-left-read-only");
                continue;
            }
            if asked.delete {
                removed.push((pim::account_of(&path).unwrap_or_default(), path));
                count += 1;
                continue;
            }
            match event_change(&path, &text, asked, &zone) {
                Ok(None) => {}
                Ok(Some(change)) => {
                    merge(&mut written, change);
                    count += 1;
                }
                Err(id) => leave(&mut left, &title, &id),
            }
        }
    }
    Planned { written, removed, left, count, changed }
}

fn merge(into: &mut Written, change: Written) {
    into.written.extend(change.written);
    into.moved.extend(change.moved);
    into.made.extend(change.made);
    into.gone.extend(change.gone);
}

/// The calendar a file is in.
fn calendar_of(path: &Path) -> Option<Collection> {
    vdir::collections(Kind::Calendars).into_iter().find(|c| path.parent() == Some(c.dir.as_path()))
}

/// What one task becomes; None when nothing changes; Err the words' id of why it is left out.
fn task_change(task: &Task, asked: &Asked, loaded: &sioul_core::links::Loaded, selected: &BTreeSet<&str>, zone: &jiff::tz::TimeZone, now: &jiff::Zoned) -> Result<Option<Written>, String> {
    let path = PathBuf::from(&task.key);
    let before = std::fs::read_to_string(&path).map_err(|_| "batch-left-gone".to_string())?;
    let mut edit = TaskEdit::of(task);
    let none = sioul_core::thingsearch::NONE;
    if let Some(project) = &asked.project {
        edit.projects = if project == none { Vec::new() } else { vec![project.clone()] };
    }
    if let Some(kind) = &asked.kind {
        edit.kind = if kind == none { String::new() } else { kind.clone() };
    }
    if let Some(category) = &asked.category {
        edit.categories = if category == none { Vec::new() } else { vec![category.clone()] };
    }
    if let Some(date) = &asked.date {
        edit.due = due_on(&task.due, date);
    }
    let mut text = if edit == TaskEdit::of(task) { before.clone() } else { tasks::apply(&before, &edit, zone, now)? };
    if let Some(status) = &asked.status {
        let status = match status.as_str() {
            "completed" => Status::Completed,
            "in-process" => Status::InProcess,
            "cancelled" => Status::Cancelled,
            "needs-action" => Status::NeedsAction,
            _ => return Err("batch-left-event-status".into()),
        };
        if task.status != status {
            text = tasks::set_status(&text, status, zone, now)?;
        }
    }
    let mut change = Written::default();
    if let Some(list) = &asked.list
        && task.list_id != *list
        && !task.parent().is_some_and(|p| selected.contains(p))
    {
        let Some(target) = tasks::lists().into_iter().find(|c| !c.read_only && format!("{}/{}", c.account, c.id) == *list) else { return Err("batch-left-no-tasks-there".into()) };
        if target.dir.as_path() != path.parent().unwrap_or(Path::new("")) {
            let config = crate::backend::load_config();
            let provider = sioul_core::capabilities::provider_of_collection(config.account(&target.account), &target);
            let family = family_of(task, &loaded.tasks);
            if family.iter().any(|t| t.read_only) {
                return Err("batch-left-read-only".into());
            }
            if family.iter().any(|t| !sioul_core::capabilities::task_losses(t, provider).is_empty()) {
                return Err("batch-left-list-loses".into());
            }
            let google = provider == sioul_core::capabilities::Provider::GoogleTasks;
            let moved = sioul_core::pairs::moved_into(&family, &target.dir, &loaded.events, google, now)?;
            // The task's own change rides with its move: its text there is the one changed here.
            let mut moved = moved;
            for (from, _, there) in &mut moved.moved {
                if *from == path && text != before {
                    *there = text.clone();
                }
            }
            return Ok(Some(moved));
        }
    }
    if text != before {
        change.written.push((path, text));
        return Ok(Some(change));
    }
    Ok(None)
}

/// A task and every step under it.
fn family_of<'a>(task: &'a Task, all: &'a [Task]) -> Vec<&'a Task> {
    let mut family: Vec<&Task> = Vec::new();
    let mut queue = vec![task.uid.clone()];
    while let Some(next) = queue.pop() {
        if let Some(found) = all.iter().find(|t| t.uid == next) {
            if family.iter().any(|t| t.uid == found.uid) {
                continue;
            }
            queue.extend(all.iter().filter(|t| t.parent() == Some(found.uid.as_str())).map(|t| t.uid.clone()));
            family.push(found);
        }
    }
    family
}

/// A task's date asked moved to `day`, its hour kept; "" for none.
pub(crate) fn due_on(due: &str, day: &str) -> String {
    let day = day.trim();
    if day.is_empty() {
        return String::new();
    }
    match due.split_once('T') {
        Some((_, hour)) => format!("{day}T{hour}"),
        None => day.to_string(),
    }
}

/// What one event becomes; None when nothing changes; Err the words' id of why it is left out.
fn event_change(path: &Path, before: &str, asked: &Asked, zone: &jiff::tz::TimeZone) -> Result<Option<Written>, String> {
    let none = sioul_core::thingsearch::NONE;
    if asked.kind.as_deref().is_some_and(|k| !k.is_empty() && k != none) {
        return Err("batch-left-event-kind".into());
    }
    let mut text = before.to_string();
    if let Some(project) = &asked.project {
        text = agenda::remove_lines(&text, |l| lines::name(l) == "REFID").unwrap_or(text);
        if project != none {
            text = agenda::add_lines(&text, &[format!("{}:{}", tasks::REFID, lines::escape(project))]).unwrap_or(text);
        }
    }
    if let Some(category) = &asked.category {
        text = agenda::remove_lines(&text, |l| lines::name(l) == "CATEGORIES").unwrap_or(text);
        if category != none {
            text = agenda::add_lines(&text, &[format!("CATEGORIES:{}", lines::escape(category))]).unwrap_or(text);
        }
    }
    if let Some(status) = &asked.status {
        if !matches!(status.as_str(), "confirmed" | "tentative" | "cancelled") {
            return Err("batch-left-task-status".into());
        }
        text = agenda::with_status(&text, status).unwrap_or(text);
    }
    if let Some(day) = asked.date.as_deref().filter(|d| !d.trim().is_empty()) {
        let reference = agenda::event_ref(&text, &path.display().to_string(), zone).ok_or_else(|| "batch-left-gone".to_string())?;
        if reference.recurring {
            return Err("batch-left-repeats".into());
        }
        let day: jiff::civil::Date = day.trim().parse().map_err(|_| "batch-left-gone".to_string())?;
        let start = jiff::Timestamp::from_second(reference.start).map_err(|_| "batch-left-gone".to_string())?.to_zoned(zone.clone());
        let moved = day.to_datetime(start.time()).to_zoned(zone.clone()).map_err(|_| "batch-left-gone".to_string())?.timestamp().as_second();
        if moved != reference.start {
            let end = moved + (reference.end - reference.start).max(0);
            text = agenda::moved(&text, reference.start, moved, end, false, zone).map_err(|_| "batch-left-gone".to_string())?;
        }
    }
    let mut change = Written::default();
    if let Some(list) = &asked.list {
        let Some(target) = vdir::collections(Kind::Calendars).into_iter().find(|c| !c.read_only && c.holds("VEVENT") && format!("{}/{}", c.account, c.id) == *list) else { return Err("batch-left-no-events-there".into()) };
        if path.parent() != Some(target.dir.as_path()) {
            let name = path.file_name().map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{}.ics", vdir::new_name())));
            change.moved.push((path.to_path_buf(), target.dir.join(name), text));
            return Ok(Some(change));
        }
    }
    if text != before {
        change.written.push((path.to_path_buf(), text));
        return Ok(Some(change));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(dir: &Path, uid: &str, summary: &str, extra: &str) -> PathBuf {
        let path = dir.join(format!("{uid}.ics"));
        vdir::write_item(&path, &format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20991020T100000Z\r\nDTSTART:20991029T130000Z\r\nDTEND:20991029T133000Z\r\nSUMMARY:{summary}\r\n{extra}END:VEVENT\r\nEND:VCALENDAR\r\n")).unwrap();
        path
    }

    /// One act over tasks and events: a project given to each that may take
    /// it, a meeting with others and a list you only read left out with why;
    /// one Undo puts every file back. A day for a repeating event is left out
    /// (from its card); a kind for an event too; a task's date asked keeps its hour.
    #[test]
    fn a_batch_changes_what_it_may_and_says_what_it_left() {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "batch-calendar", None, &["VEVENT"]).unwrap();
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "batch-list", None, &["VTODO"]).unwrap();
        let dentist = event(&calendar.dir, "batch-dentist", "Dentist", "");
        let meeting = event(&calendar.dir, "batch-meeting", "Team meeting", "ORGANIZER:mailto:camille@example.org\r\nATTENDEE:mailto:sam@example.org\r\n");
        let yoga = event(&calendar.dir, "batch-yoga", "Yoga", "RRULE:FREQ=WEEKLY\r\n");
        let form = list.dir.join("batch-form.ics");
        vdir::write_item(&form, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:batch-form\r\nDTSTAMP:20991020T100000Z\r\nSUMMARY:Fill the form\r\nDUE;VALUE=DATE-TIME:20991112T170000\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
        let shared = Shared::default();
        let thing = |kind: &str, path: &Path, uid: &str| Thing { thing: kind.into(), key: path.display().to_string(), uid: uid.into() };
        let things = [thing("event", &dentist, "batch-dentist"), thing("event", &meeting, "batch-meeting"), thing("task", &form, "batch-form")];
        let before: Vec<String> = [&dentist, &meeting, &form].iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
        let plan = planned(&shared, &things, &Asked { project: Some("home".into()), ..Asked::default() });
        assert_eq!(plan.left.iter().map(|l| (l.title.as_str(), l.why.clone())).collect::<Vec<_>>(), [("Team meeting", tr().text("batch-left-meeting", None))], "the meeting left, with why");
        assert_eq!((plan.count, plan.written.written.len()), (2, 2));
        let files = plan.written.write().unwrap();
        let task = sioul_core::tasks::task_of_text(&std::fs::read_to_string(&form).unwrap(), &sioul_core::zone::current()).unwrap();
        assert_eq!(task.projects, ["home"]);
        let zone = sioul_core::zone::current();
        assert_eq!(agenda::event_ref(&std::fs::read_to_string(&dentist).unwrap(), "d", &zone).unwrap().projects, ["home"]);
        assert_eq!(std::fs::read_to_string(&meeting).unwrap(), before[1], "never written");
        assert!(sioul_core::agentwrites::Said { at: 0, line: String::new(), files }.put_back(), "one Undo");
        let after: Vec<String> = [&dentist, &meeting, &form].iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
        assert_eq!(after, before, "every file back");
        // A day: the task's date asked keeps its hour; the repeating event is left out, from its card.
        if let Ok(mut cache) = shared.loaded.lock() {
            *cache = None;
        }
        let things = [thing("event", &yoga, "batch-yoga"), thing("task", &form, "batch-form")];
        let plan = planned(&shared, &things, &Asked { date: Some("2099-11-20".into()), ..Asked::default() });
        assert_eq!(plan.left.iter().map(|l| l.why.clone()).collect::<Vec<_>>(), [tr().text("batch-left-repeats", None)]);
        plan.written.write().unwrap();
        let task = sioul_core::tasks::task_of_text(&std::fs::read_to_string(&form).unwrap(), &zone).unwrap();
        assert_eq!(task.due, "2099-11-20T17:00");
        // A kind given to an event: left out, said.
        let plan = planned(&shared, &[thing("event", &dentist, "batch-dentist")], &Asked { kind: Some("call".into()), ..Asked::default() });
        assert_eq!((plan.count, plan.left.len()), (0, 1));
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_dir_all(&list.dir);
    }

    /// A task's date asked moved keeps its hour; "" takes it away.
    #[test]
    fn a_date_asked_moves_with_its_hour() {
        assert_eq!(due_on("2026-11-12T17:00", "2026-11-20"), "2026-11-20T17:00");
        assert_eq!(due_on("2026-11-12", "2026-11-20"), "2026-11-20");
        assert_eq!(due_on("", "2026-11-20"), "2026-11-20");
        assert_eq!(due_on("2026-11-12", ""), "");
    }
}
