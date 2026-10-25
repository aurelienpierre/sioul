// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Events and tasks, read and changed one by one (25 October 2026): get by
//! address, update (time, title, place, calendar or list, reminder,
//! margins, status), move, delete, and a task made an appointment or an
//! appointment a task. The window's own ways: an event's lines changed as
//! its editor changes them (`agenda::apply`), a task's as its form does
//! (`tasks::apply`), a move through the move journal (`vdir::move_item`),
//! a conversion as the agenda's (`appointments`), never twice, never lost.
//!
//! Refused as in the window: a meeting with others (an organizer or guests)
//! is never moved nor deleted, since its server would cancel it for them;
//! a calendar or a list that can only be read is never written. Refused as
//! for every agent: nothing kept from agents is found, nothing is changed in
//! a project closed to them (`access`, `Consent`).
//!
//! Each change is said in the window, with Undo, for as long as its files
//! stay as the agent left them (`agentwrites`).

use super::access;
use super::read::{self, Desk};
use super::tools::{Answer, Args};
use crate::{Session, one_line};
use jiff::Zoned;
use jiff::tz::TimeZone;
use serde_json::{Value, json};
use sioul_core::agenda::{self, EventEdit};
use sioul_core::agentwrites::{FileChange, Said};
use sioul_core::appointments;
use sioul_core::consent::Consent;
use sioul_core::links::{self, Loaded};
use sioul_core::tasks::{self, Status, Task, TaskEdit};
use sioul_core::vdir::{self, Collection, Kind};
use std::path::{Path, PathBuf};

/// An event found for an agent: its file, its calendar, its text, its UID.
struct Found {
    path: PathBuf,
    calendar: Collection,
    text: String,
    uid: String,
}

/// An event by its address (`sioul:event/<UID>`) or its UID, in any
/// calendar; refused when it is kept from agents, or the block of a task kept from them.
fn find_event(consent: &Consent, wanted: &str) -> Result<Found, String> {
    let wanted = wanted.trim();
    let uid = if wanted.starts_with("sioul:event/") { links::id_of(wanted) } else { wanted.to_string() };
    let gone = || format!("No event “{}” here: agenda and find give events' addresses (sioul:event/…).", one_line(wanted));
    for calendar in vdir::collections(Kind::Calendars) {
        for path in calendar.items() {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if !text.contains("BEGIN:VEVENT") || appointments::uid_in(&path) != uid {
                continue;
            }
            let task = sioul_core::lines::unfold(&text).iter().find(|l| sioul_core::lines::name(l) == sioul_core::blocks::TASK).map(|l| sioul_core::lines::value(l).trim().to_string()).unwrap_or_default();
            if !consent.allows(&links::event_uri(&uid)) || (!task.is_empty() && !consent.allows(&links::task_uri(&task))) {
                return Err(access::kept("This event"));
            }
            return Ok(Found { path, calendar, text, uid });
        }
    }
    Err(gone())
}

/// Whether something in these projects may be changed for an agent: each open; in none, while things outside projects are open.
fn may_change(consent: &Consent, uri: &str) -> Result<(), String> {
    let projects = consent.projects_of(uri);
    if let Some(closed) = projects.iter().find(|p| consent.knows(p) && !consent.is_open(p)) {
        return Err(access::closed_project(closed));
    }
    if consent.allows_projects(projects.iter().map(String::as_str)) { Ok(()) } else { Err(access::OUTSIDE_CLOSED.into()) }
}

/// A change said in the window, with Undo while its files stay as they are now.
fn say_in_window(line: String, files: Vec<FileChange>) {
    let said = Said { at: Zoned::now().timestamp().as_millisecond(), line, files };
    if let Err(e) = sioul_core::agentwrites::note(&said) {
        eprintln!("sioul mcp: {e}");
    }
}

/// The calendar named ("account/id") that takes events; "": the agenda's usual one.
fn calendar_named(id: &str, s: &Session) -> Result<Collection, String> {
    if id.is_empty() {
        return agenda::default_calendar().ok_or_else(|| s.tr.text("dav-no-calendar", None));
    }
    vdir::collections(Kind::Calendars)
        .into_iter()
        .find(|c| format!("{}/{}", c.account, c.id) == id && !c.read_only && c.holds("VEVENT"))
        .ok_or_else(|| format!("No calendar “{}” takes events here (account/id).", one_line(id)))
}

fn event_json(found: &Found, edit: &EventEdit, s: &Session) -> Value {
    let unfolded = sioul_core::lines::unfold(&found.text);
    let count = |name: &str| unfolded.iter().filter(|l| sioul_core::lines::name(l) == name).count();
    json!({
        "uri": links::event_uri(&found.uid),
        "uid": found.uid,
        "title": edit.title,
        "start": edit.start,
        "end": edit.end,
        "all_day": edit.all_day,
        "location": edit.location,
        "notes": edit.notes,
        "repeat": edit.repeat,
        "remind": edit.remind,
        "before": edit.margins.before,
        "after": edit.margins.after,
        "calendar": format!("{}/{}", found.calendar.account, found.calendar.id),
        "calendar_name": found.calendar.label(&s.config, &s.tr),
        "read_only": found.calendar.read_only,
        "with_others": agenda::with_others(&found.text),
        "guests": count("ATTENDEE"),
        "task_block": appointments::block_of(&found.text, ""),
    })
}

pub fn get_event(s: &Session, args: &Args) -> Result<Answer, String> {
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    let found = find_event(&consent, &args.needed("event")?)?;
    let edit = agenda::edit_of_text(&found.text, &TimeZone::system()).ok_or("This event's file does not read.")?;
    let data = event_json(&found, &edit, s);
    let when = if edit.start == edit.end { edit.start.clone() } else { format!("{} – {}", edit.start, edit.end) };
    Ok(Answer { text: format!("{} · {when} · {}  <{}>", one_line(&edit.title), data["calendar_name"].as_str().unwrap_or_default(), links::event_uri(&found.uid)), data })
}

/// A number of minutes given, 0 to a day; none when absent.
fn minutes(args: &Args, name: &str) -> Result<Option<u32>, String> {
    Ok(args.has(name).then(|| args.number(name, 0, 0, 1440)).transpose()?.map(|m| m as u32))
}

pub fn update_event(s: &Session, args: &Args) -> Result<Answer, String> {
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    let found = find_event(&consent, &args.needed("event")?)?;
    may_change(&consent, &links::event_uri(&found.uid))?;
    if found.calendar.read_only {
        return Err(s.tr.text("agenda-read-only", None));
    }
    let zone = TimeZone::system();
    let before = agenda::edit_of_text(&found.text, &zone).ok_or("This event's file does not read.")?;
    let mut edit = before.clone();
    if let Some(title) = args.line("title")? {
        edit.title = title;
    }
    if let Some(start) = args.moment("start")? {
        edit.all_day = !start.contains('T');
        edit.start = start;
    }
    if let Some(end) = args.moment("end")? {
        edit.end = end;
    }
    if args.has("all_day") {
        edit.all_day = args.flag("all_day")?;
    }
    if args.has("location") {
        edit.location = args.line("location")?.unwrap_or_default();
    }
    if args.has("notes") {
        edit.notes = args.body("notes")?;
    }
    if let Some(repeat) = args.text("repeat")? {
        if !matches!(repeat.as_str(), "none" | "daily" | "weekly" | "monthly" | "yearly") {
            return Err(format!("“repeat” is none, daily, weekly, monthly or yearly; not “{}”.", one_line(&repeat)));
        }
        edit.repeat = if repeat == "none" { String::new() } else { repeat };
    }
    if let Some(remind) = args.text("remind")? {
        if !(remind == "usual" || remind == "none" || remind.parse::<u32>().is_ok_and(|m| m <= 10_080)) {
            return Err("“remind” is usual, none, or minutes before it (\"15\").".into());
        }
        edit.remind = if remind == "usual" { String::new() } else { remind };
    }
    if let Some(m) = minutes(args, "before")? {
        edit.margins.before = m;
    }
    if let Some(m) = minutes(args, "after")? {
        edit.margins.after = m;
    }
    let timed = |m: &str| m.contains('T');
    if (edit.all_day && (timed(&edit.start) || timed(&edit.end))) || (!edit.all_day && (!timed(&edit.start) || !timed(&edit.end))) {
        return Err("A whole-day event starts and ends on days, as 2026-10-05; one at a time, at times, as 2026-10-05T09:00.".into());
    }
    let calendar = args.text("calendar")?;
    let moving = calendar.as_deref().is_some_and(|c| c != format!("{}/{}", found.calendar.account, found.calendar.id));
    // A meeting with others never moves: its server would cancel it for them, then invite them again.
    if moving && agenda::with_others(&found.text) {
        return Err(s.tr.text("event-move-with-others", None));
    }
    let text = agenda::apply(&found.text, &edit, &zone)?;
    let mut files = Vec::new();
    if text != found.text {
        vdir::write_item(&found.path, &text)?;
        files.push(FileChange::of(&found.path, Some(found.text.clone())));
    }
    let mut at = found.path.clone();
    let mut line = s.say("agent-event-changed", &[("title", one_line(&edit.title))]);
    if moving && let Some(id) = &calendar {
        let target = calendar_named(id, s)?;
        let moved = agenda::new_path(&target);
        let written = std::fs::read_to_string(&found.path).map_err(|e| e.to_string())?;
        vdir::move_item(&found.path, &moved)?;
        // As a move: there before, the old file gone.
        files = vec![FileChange { path: found.path.clone(), before: Some(found.text.clone()), after: None }, FileChange { path: moved.clone(), before: None, after: Some(written) }];
        at = moved;
        line = s.say("agent-event-moved", &[("title", one_line(&edit.title)), ("calendar", target.label(&s.config, &s.tr))]);
    }
    if files.is_empty() {
        return Ok(Answer { text: "Nothing changed: the event already says so.".into(), data: json!({ "uri": links::event_uri(&found.uid), "changed": false }) });
    }
    say_in_window(line.clone(), files);
    let calendar = pim_collection(&at).unwrap_or(found.calendar.clone());
    Ok(Answer { text: line, data: json!({ "uri": links::event_uri(&found.uid), "changed": true, "calendar": format!("{}/{}", calendar.account, calendar.id), "sent": false }) })
}

/// The collection a file is in.
fn pim_collection(path: &Path) -> Option<Collection> {
    let dir = path.parent()?;
    vdir::collections(Kind::Calendars).into_iter().find(|c| c.dir == dir)
}

pub fn delete_event(s: &Session, args: &Args) -> Result<Answer, String> {
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    let found = find_event(&consent, &args.needed("event")?)?;
    may_change(&consent, &links::event_uri(&found.uid))?;
    if found.calendar.read_only {
        return Err(s.tr.text("agenda-read-only", None));
    }
    // Deleted, a meeting with others would be cancelled or declined for them by its server.
    if agenda::with_others(&found.text) {
        return Err(s.tr.text("event-delete-with-others", None));
    }
    let zone = TimeZone::system();
    let title = agenda::edit_of_text(&found.text, &zone).map(|e| e.title).unwrap_or_default();
    let line;
    let files = match args.moment("occurrence")? {
        // One time of a repeating event left out (EXDATE), as the agenda's "This time only".
        Some(when) => {
            let start = when.parse::<jiff::civil::DateTime>().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).or_else(|| when.parse::<jiff::civil::Date>().ok().and_then(|d| d.to_zoned(zone.clone()).ok())).map(|z| z.timestamp().as_second()).ok_or("“occurrence” is the time it starts, as 2026-10-05T09:00.")?;
            let text = agenda::skip_occurrence(&found.text, start).ok_or("This event does not repeat, or has no time then: delete it whole.")?;
            vdir::write_item(&found.path, &text)?;
            line = s.say("agent-event-skipped", &[("title", one_line(&title)), ("when", when)]);
            vec![FileChange::of(&found.path, Some(found.text.clone()))]
        }
        None => {
            std::fs::remove_file(&found.path).map_err(|e| e.to_string())?;
            line = s.say("agent-event-deleted", &[("title", one_line(&title))]);
            vec![FileChange { path: found.path.clone(), before: Some(found.text.clone()), after: None }]
        }
    };
    say_in_window(line.clone(), files);
    Ok(Answer { text: line, data: json!({ "uri": links::event_uri(&found.uid), "deleted": true, "sent": false }) })
}

fn find_open_task(s: &Session, desk: &Desk, wanted: &str) -> Result<Task, String> {
    let open = desk.open_tasks();
    read::find_task(s, &open, wanted).cloned()
}

fn task_json(task: &Task) -> Value {
    json!({
        "uri": links::task_uri(&task.uid),
        "uid": task.uid,
        "title": task.title,
        "notes": task.notes,
        "location": task.location,
        "status": match task.status { Status::NeedsAction => "needs-action", Status::InProcess => "in-process", Status::Completed => "completed", Status::Cancelled => "cancelled" },
        "start": task.start,
        "due": task.due,
        "estimate": task.estimate,
        "priority": task.priority,
        "before": task.margins.before,
        "after": task.margins.after,
        "kind": task.kind,
        "tags": task.categories,
        "projects": task.projects,
        "repeat": task.repeat,
        "list": task.list_id,
        "list_name": task.list,
        "read_only": task.read_only,
        "parent": task.parent(),
    })
}

pub fn get_task(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let task = find_open_task(s, &desk, &args.needed("task")?)?;
    Ok(Answer { text: format!("{} · {}  <{}>", one_line(&task.title), one_line(&task.list), links::task_uri(&task.uid)), data: task_json(&task) })
}

pub fn update_task(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let task = find_open_task(s, &desk, &args.needed("task")?)?;
    may_change(&desk.consent, &links::task_uri(&task.uid))?;
    if task.read_only {
        return Err(s.tr.text("task-read-only", None));
    }
    let zone = TimeZone::system();
    let now = Zoned::now();
    let current = std::fs::read_to_string(&task.key).map_err(|e| format!("{}: {e}", task.key))?;
    let mut edit = TaskEdit::of(&task);
    if let Some(title) = args.line("title")? {
        edit.title = title;
    }
    if args.has("notes") {
        edit.notes = args.body("notes")?;
    }
    if args.has("location") {
        edit.location = args.line("location")?.unwrap_or_default();
    }
    if args.has("start") {
        edit.start = args.moment("start")?.unwrap_or_default();
    }
    if args.has("due") {
        edit.due = args.moment("due")?.unwrap_or_default();
    }
    if args.has("estimate") {
        edit.estimate = args.number("estimate", 0, 0, 6000)? as u32;
    }
    if args.has("priority") {
        edit.priority = args.number("priority", 0, 0, 9)? as u8;
    }
    if let Some(m) = minutes(args, "before")? {
        edit.margins.before = m;
    }
    if let Some(m) = minutes(args, "after")? {
        edit.margins.after = m;
    }
    let mut text = tasks::apply(&current, &edit, &zone, &now)?;
    if let Some(status) = args.text("status")? {
        let status = match status.as_str() {
            "needs-action" => Status::NeedsAction,
            "in-process" => Status::InProcess,
            "completed" => Status::Completed,
            "cancelled" => Status::Cancelled,
            other => return Err(format!("“status” is needs-action, in-process, completed or cancelled; not “{}”.", one_line(other))),
        };
        if status != task.status {
            text = tasks::set_status(&text, status, &zone, &now)?;
        }
    }
    let mut files = Vec::new();
    if text != current {
        vdir::write_item(Path::new(&task.key), &text)?;
        files.push(FileChange::of(Path::new(&task.key), Some(current.clone())));
    }
    let mut line = s.say("agent-task-changed", &[("title", one_line(&edit.title))]);
    if let Some(list) = args.text("list")?.filter(|l| *l != task.list_id) {
        let target = tasks::lists().into_iter().find(|c| !c.read_only && format!("{}/{}", c.account, c.id) == list).ok_or_else(|| s.tr.text("task-list-no-tasks", None))?;
        // What the list would not keep (Google Tasks'), said first, as the window asks.
        let provider = sioul_core::capabilities::provider_of_collection(s.config.account(&target.account), &target);
        let family: Vec<&Task> = std::iter::once(&task).chain(desk.loaded.tasks.iter().filter(|t| t.parent() == Some(task.uid.as_str()))).collect();
        let mut losses: Vec<&str> = family.iter().flat_map(|t| sioul_core::capabilities::task_losses(t, provider)).collect();
        losses.sort_unstable();
        losses.dedup();
        if !losses.is_empty() && !args.flag("confirm")? {
            return Err(format!("{}: {}. Give “confirm”: true to move it all the same.", s.tr.text("task-list-would-lose", None), losses.iter().map(|l| s.tr.text(&format!("loss-{l}"), None)).collect::<Vec<_>>().join(", ")));
        }
        if family.iter().any(|t| t.read_only) {
            return Err(s.tr.text("task-read-only", None));
        }
        // The task and its steps, each through the move journal.
        for t in family {
            let from = PathBuf::from(&t.key);
            if from.parent() == Some(target.dir.as_path()) {
                continue;
            }
            let written = std::fs::read_to_string(&from).map_err(|e| e.to_string())?;
            let to = target.dir.join(format!("{}.ics", vdir::new_name()));
            vdir::move_item(&from, &to)?;
            files.retain(|f| f.path != from);
            let was = if t.uid == task.uid { Some(current.clone()) } else { Some(written.clone()) };
            files.push(FileChange { path: from, before: was, after: None });
            files.push(FileChange { path: to, before: None, after: Some(written) });
        }
        line = s.say("agent-task-moved", &[("title", one_line(&edit.title)), ("list", target.label(&s.config, &s.tr))]);
    }
    if files.is_empty() {
        return Ok(Answer { text: "Nothing changed: the task already says so.".into(), data: json!({ "uri": links::task_uri(&task.uid), "changed": false }) });
    }
    say_in_window(line.clone(), files);
    Ok(Answer { text: line, data: json!({ "uri": links::task_uri(&task.uid), "changed": true, "sent": false }) })
}

pub fn delete_task(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let task = find_open_task(s, &desk, &args.needed("task")?)?;
    may_change(&desk.consent, &links::task_uri(&task.uid))?;
    if task.read_only {
        return Err(s.tr.text("task-read-only", None));
    }
    let now = Zoned::now();
    // Its time blocks to come go with it, as the window deletes them.
    let chosen = s.config.tasks.blocks.clone().unwrap_or_default();
    let blocks = sioul_core::blocks::to_come_of(&task.uid, &chosen, now.timestamp().as_second(), now.time_zone());
    let mut files = Vec::new();
    for path in std::iter::once(PathBuf::from(&task.key)).chain(blocks) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        files.push(FileChange { path, before: Some(text), after: None });
    }
    let line = s.say("agent-task-deleted", &[("title", one_line(&task.title))]);
    say_in_window(line.clone(), files);
    Ok(Answer { text: line, data: json!({ "uri": links::task_uri(&task.uid), "deleted": true, "sent": false }) })
}

/// The other tasks that name `old`, as they are now: what a conversion's ties change, to undo them.
fn naming(tasks: &[Task], old: &str) -> Vec<(PathBuf, Option<String>)> {
    tasks.iter().filter(|t| t.uid != old).filter_map(|t| std::fs::read_to_string(&t.key).ok().filter(|text| text.contains(old)).map(|text| (PathBuf::from(&t.key), Some(text)))).collect()
}

/// Every file a replacement touches, before it is written: to say it and undo it.
fn before_of(plan: &appointments::Replacement) -> Vec<(PathBuf, Option<String>)> {
    plan.files().into_iter().map(|p| (p.clone(), std::fs::read_to_string(&p).ok())).collect()
}

fn after_of(before: Vec<(PathBuf, Option<String>)>) -> Vec<FileChange> {
    before.into_iter().map(|(path, was)| FileChange::of(&path, was)).collect()
}

pub fn task_to_event(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let task = find_open_task(s, &desk, &args.needed("task")?)?;
    may_change(&desk.consent, &links::task_uri(&task.uid))?;
    let zone = TimeZone::system();
    let now = Zoned::now();
    let chosen = s.config.tasks.blocks.clone().unwrap_or_default();
    let blocks = sioul_core::blocks::to_come_of(&task.uid, &chosen, now.timestamp().as_second(), &zone);
    // Its time: its block's, else its own; what the agent gives over it.
    let block_time = blocks.first().and_then(|p| agenda::edit_of(p)).and_then(|e| {
        let at = |m: &str| m.parse::<jiff::civil::DateTime>().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map(|z| z.timestamp().as_second());
        Some((at(&e.start)?, at(&e.end)?))
    });
    let mut edit = appointments::event_of(&task, appointments::time_of(&task, block_time, &zone), now.date(), &zone);
    if let Some(start) = args.moment("start")? {
        if !start.contains('T') {
            return Err("“start” is a time, as 2026-10-05T09:00.".into());
        }
        // Its length kept, from its new start.
        let length = edit.end.parse::<jiff::civil::DateTime>().ok().zip(edit.start.parse::<jiff::civil::DateTime>().ok()).and_then(|(e, b)| e.since(b).ok());
        edit.start = start.clone();
        edit.end = length.and_then(|l| start.parse::<jiff::civil::DateTime>().ok()?.checked_add(l).ok()).map(|e| e.strftime("%Y-%m-%dT%H:%M").to_string()).unwrap_or(start);
    }
    if let Some(end) = args.moment("end")? {
        edit.end = end;
    }
    let target = calendar_named(&args.text("calendar")?.unwrap_or_default(), s)?;
    let plan = appointments::task_to_event(&task, &desk.loaded.tasks, &blocks, &edit, &target, &zone).map_err(|r| r.said(&s.tr))?;
    let mut before = before_of(&plan);
    let tied = naming(&desk.loaded.tasks, &task.uid);
    plan.write()?;
    let new_uid = appointments::uid_in(&plan.to);
    let retied = appointments::retie(&desk.loaded.tasks, &task.uid, &new_uid, true, &now);
    before.extend(tied.into_iter().filter(|(p, _)| retied.contains(p)));
    let files = after_of(before);
    let line = s.say("agent-task-to-event", &[("title", one_line(&task.title)), ("calendar", target.label(&s.config, &s.tr))]);
    say_in_window(line.clone(), files.into_iter().filter(|f| f.before.is_some() || f.after.is_some()).collect());
    Ok(Answer { text: format!("{line}\n<{}>", links::event_uri(&new_uid)), data: json!({ "uri": links::event_uri(&new_uid), "was": links::task_uri(&task.uid), "sent": false }) })
}

pub fn event_to_task(s: &Session, args: &Args) -> Result<Answer, String> {
    let loaded = Loaded::read(&s.config);
    let consent = access::of(s, &loaded);
    let found = find_event(&consent, &args.needed("event")?)?;
    may_change(&consent, &links::event_uri(&found.uid))?;
    if found.calendar.read_only {
        return Err(s.tr.text("agenda-read-only", None));
    }
    let list = match args.text("list")? {
        Some(id) => tasks::lists().into_iter().find(|c| !c.read_only && format!("{}/{}", c.account, c.id) == id).ok_or_else(|| s.tr.text("task-list-no-tasks", None))?,
        None => tasks::default_list().ok_or_else(|| s.tr.text("task-no-list-yet", None))?,
    };
    let now = Zoned::now();
    let name = s.tr.text("blocks-calendar", None);
    let config = s.config.clone();
    let place = move |task: &Task| sioul_core::blocks::calendar_of(&config, task, &name);
    let plan = appointments::event_to_task(&found.path, &list, &place, s.config.tasks.block_alarms, &now).map_err(|r| r.said(&s.tr))?;
    let mut before = before_of(&plan);
    let tied = naming(&loaded.tasks, &found.uid);
    plan.write()?;
    let new_uid = appointments::uid_in(&plan.to);
    let retied = appointments::retie(&loaded.tasks, &found.uid, &new_uid, false, &now);
    before.extend(tied.into_iter().filter(|(p, _)| retied.contains(p)));
    let files = after_of(before);
    let title = agenda::edit_of_text(&found.text, now.time_zone()).map(|e| e.title).unwrap_or_default();
    let line = s.say("agent-event-to-task", &[("title", one_line(&title)), ("list", list.label(&s.config, &s.tr))]);
    say_in_window(line.clone(), files.into_iter().filter(|f| f.before.is_some() || f.after.is_some()).collect());
    Ok(Answer { text: format!("{line}\n<{}>", links::task_uri(&new_uid)), data: json!({ "uri": links::task_uri(&new_uid), "was": links::event_uri(&found.uid), "sent": false }) })
}
