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
use sioul_core::checklist::{self, Mark};
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
        // The free text and the checklist apart (`checklist`): notes written back never wipe the list.
        "notes": checklist::free_text(&edit.notes),
        "checklist": checklist_json(&edit.notes),
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
        edit.notes = free_text_given(args, &edit.notes)?;
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

/// A checklist as data: each item's place, group, words, state, detail.
fn checklist_json(notes: &str) -> Value {
    Value::Array(checklist::items(notes).iter().map(|i| json!({ "number": i.number, "group": i.group, "text": i.text, "state": i.mark.id(), "detail": i.detail })).collect())
}

/// A checklist in lines: `1. [x] Carte Vitale (Bring)`.
fn checklist_lines(notes: &str) -> Vec<String> {
    checklist::items(notes).iter().map(|i| format!("{}. [{}] {}{}", i.number, i.mark.char(), one_line(&i.text), if i.group.is_empty() { String::new() } else { format!(" ({})", one_line(&i.group)) })).collect()
}

/// The notes given (`notes`): their free text, the checklist they hold kept
/// as it is (`checklist::with_free_text`). Lines of a checklist in them are
/// refused: the list is set by set_checklist, ticked by check_item.
fn free_text_given(args: &Args, notes: &str) -> Result<String, String> {
    let given = args.body("notes")?;
    if !checklist::items(&given).is_empty() {
        return Err("“notes” is the free text: the checklist (its “- [ ]” lines) is kept as it is; set_checklist writes it, check_item ticks it.".into());
    }
    Ok(checklist::with_free_text(notes, &given))
}

/// Files changed twice in one call: the first before, the last after (Undo puts back the first).
fn merge_files(files: &mut Vec<FileChange>, more: Vec<FileChange>) {
    for change in more {
        match files.iter_mut().find(|f| f.path == change.path) {
            Some(found) => found.after = change.after,
            None => files.push(change),
        }
    }
}

/// A composed task's change refused, said; what it would lose, with how to go on.
fn refused(s: &Session, refusal: sioul_core::composed::Refusal) -> String {
    match refusal {
        sioul_core::composed::Refusal::Losses(_) => format!("{} Give “confirm”: true to go on.", refusal.said(&s.tr)),
        refusal => refusal.said(&s.tr),
    }
}

pub fn convert_item(s: &Session, args: &Args) -> Result<Answer, String> {
    use sioul_core::composed;
    let desk = Desk::read(s);
    let (zone, now) = (TimeZone::system(), Zoned::now());
    let to = args.needed("to")?;
    // The files as they are, for what is written; what reaches the agent, found through the desk.
    let all = tasks::all(&zone);
    let (change, line) = match to.as_str() {
        "item" => {
            let step = find_open_task(s, &desk, &args.needed("step")?)?;
            may_change(&desk.consent, &links::task_uri(&step.uid))?;
            let whole = step.parent().and_then(|p| all.iter().find(|t| t.uid == p)).map(|t| t.title.clone()).unwrap_or_default();
            let chosen = s.config.tasks.blocks.clone().unwrap_or_default();
            let blocks = sioul_core::blocks::to_come_of(&step.uid, &chosen, now.timestamp().as_second(), &zone);
            let change = composed::step_to_item(&all, &step.uid, &blocks, args.flag("confirm")?, &now).map_err(|r| refused(s, r))?;
            (change, s.say("agent-folded", &[("title", one_line(&step.title)), ("whole", one_line(&whole))]))
        }
        "step" | "task" => {
            let holder = holder(s, &desk, &args.needed("of")?)?;
            let notes = holder.notes();
            let wanted = args.needed("item")?;
            let item = checklist::find(&notes, &wanted).map_err(|_| format!("No item “{}” alone in the list of “{}”: its items now: {}.", one_line(&wanted), one_line(&holder.title()), checklist_lines(&notes).join("; ")))?;
            let said = s.say(if to == "step" { "agent-item-step" } else { "agent-item-task" }, &[("title", one_line(&item.text))]);
            let planned = match (&holder, to.as_str()) {
                (Holder::Task(task), "step") => composed::item_to_step(&all, &task.uid, item.line, &now),
                (Holder::Event(_), "step") => return Err("An event's item becomes a task (“to”: “task”): steps are a task's.".into()),
                (Holder::Task(task), _) => {
                    let list = tasks::default_list().ok_or_else(|| s.tr.text("task-no-list-yet", None))?;
                    let task = all.iter().find(|t| t.uid == task.uid).ok_or_else(|| s.tr.text("task-gone", None))?;
                    composed::item_to_task(&composed::Holder::Task(task), item.line, &list, &now)
                }
                (Holder::Event(found), _) => {
                    if found.calendar.read_only {
                        return Err(s.tr.text("agenda-read-only", None));
                    }
                    if agenda::with_others(&found.text) {
                        return Err(s.tr.text("checklist-with-others", None));
                    }
                    let list = tasks::default_list().ok_or_else(|| s.tr.text("task-no-list-yet", None))?;
                    composed::item_to_task(&composed::Holder::Event { path: &found.path, text: &found.text, uid: &found.uid }, item.line, &list, &now)
                }
            };
            (planned.map(|(change, _)| change).map_err(|r| refused(s, r))?, said)
        }
        other => return Err(format!("“to” is step, task or item; not “{}”.", one_line(other))),
    };
    let files = change.write()?;
    say_in_window(line.clone(), files);
    Ok(Answer { text: line, data: json!({ "changed": true, "sent": false }) })
}

/// What holds a checklist, found for an agent: a task, or an event.
enum Holder {
    Task(Task),
    Event(Found),
}

/// A task (its address, UID or words of its title) or an event (its address,
/// sioul:event/…), whose checklist an agent may change.
fn holder(s: &Session, desk: &Desk, wanted: &str) -> Result<Holder, String> {
    if wanted.trim().starts_with("sioul:event/") {
        let found = find_event(&desk.consent, wanted)?;
        may_change(&desk.consent, &links::event_uri(&found.uid))?;
        return Ok(Holder::Event(found));
    }
    let task = find_open_task(s, desk, wanted)?;
    may_change(&desk.consent, &links::task_uri(&task.uid))?;
    Ok(Holder::Task(task))
}

impl Holder {
    fn title(&self) -> String {
        match self {
            Holder::Task(task) => task.title.clone(),
            Holder::Event(found) => agenda::edit_of_text(&found.text, &TimeZone::system()).map(|e| e.title).unwrap_or_default(),
        }
    }

    fn notes(&self) -> String {
        match self {
            Holder::Task(task) => task.notes.clone(),
            Holder::Event(found) => agenda::edit_of_text(&found.text, &TimeZone::system()).map(|e| e.notes).unwrap_or_default(),
        }
    }

    fn uri(&self) -> String {
        match self {
            Holder::Task(task) => links::task_uri(&task.uid),
            Holder::Event(found) => links::event_uri(&found.uid),
        }
    }

    /// Its notes written as `notes`, their line alone; what it was, to undo.
    /// Refused where the window refuses: a list or a calendar that can only be
    /// read, a meeting with others (its organizer's words), notes Google
    /// Tasks would cut.
    fn write_notes(&self, s: &Session, notes: &str) -> Result<Option<FileChange>, String> {
        let zone = TimeZone::system();
        let (path, current, text) = match self {
            Holder::Task(task) => {
                if task.read_only {
                    return Err(s.tr.text("task-read-only", None));
                }
                let provider = tasks::lists().into_iter().find(|c| format!("{}/{}", c.account, c.id) == task.list_id).map(|c| sioul_core::capabilities::provider_of_collection(s.config.account(&c.account), &c));
                if provider.is_some_and(|p| sioul_core::capabilities::notes_too_long(p, notes)) {
                    return Err(s.tr.text("notes-google-too-long", None));
                }
                let current = std::fs::read_to_string(&task.key).map_err(|e| format!("{}: {e}", task.key))?;
                let edit = TaskEdit { notes: notes.to_string(), ..TaskEdit::of(task) };
                let text = tasks::apply(&current, &edit, &zone, &Zoned::now())?;
                (PathBuf::from(&task.key), current, text)
            }
            Holder::Event(found) => {
                if found.calendar.read_only {
                    return Err(s.tr.text("agenda-read-only", None));
                }
                if agenda::with_others(&found.text) {
                    return Err(s.tr.text("checklist-with-others", None));
                }
                let mut edit = agenda::edit_of_text(&found.text, &zone).ok_or("This event's file does not read.")?;
                edit.notes = notes.to_string();
                (found.path.clone(), found.text.clone(), agenda::apply(&found.text, &edit, &zone)?)
            }
        };
        if text == current {
            return Ok(None);
        }
        vdir::write_item(&path, &text)?;
        Ok(Some(FileChange::of(&path, Some(current))))
    }
}

pub fn check_item(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let holder = holder(s, &desk, &args.needed("of")?)?;
    let state = args.needed("state")?;
    let mark = Mark::parse(&state).ok_or_else(|| format!("“state” is done, open or skipped; not “{}”.", one_line(&state)))?;
    let notes = holder.notes();
    let wanted = args.needed("item")?;
    // Named by its words or its place in the list as it is now: never a guess on a stale reading.
    let item = checklist::find(&notes, &wanted).map_err(|now| {
        let listed = if now.is_empty() { "it has no list".to_string() } else { format!("its items now: {}", checklist_lines(&notes).join("; ")) };
        format!("No item “{}” alone in the list of “{}”: {listed}. Name one by its words or its number.", one_line(&wanted), one_line(&holder.title()))
    })?;
    let ticked = checklist::ticked(&notes, item.line, mark).map_err(|_| "The list changed while it was read: read it again.".to_string())?;
    let title = holder.title();
    let mut lines = Vec::new();
    match holder.write_notes(s, &ticked)? {
        Some(change) => {
            let line = s.say("agent-item-checked", &[("item", one_line(&item.text)), ("state", mark.id().to_string()), ("title", one_line(&title))]);
            say_in_window(line.clone(), vec![change]);
            lines.push(line);
        }
        None => lines.push("Nothing changed: the item says so already.".into()),
    }
    lines.extend(checklist_lines(&ticked));
    Ok(Answer { text: lines.join("\n"), data: json!({ "uri": holder.uri(), "item": item.number, "state": mark.id(), "checklist": checklist_json(&ticked), "sent": false }) })
}

/// The groups asked: a name each ("" for none) and items, a text each or {text, state}.
fn groups_asked(args: &Args) -> Result<Vec<checklist::Group>, String> {
    let wrong = || "“groups” is a list of {name, items}: items a list of texts, or of {text, state}.".to_string();
    let groups = args.raw("groups").and_then(Value::as_array).ok_or_else(wrong)?;
    let mut out = Vec::new();
    let mut count = 0;
    for group in groups {
        let fields = group.as_object().ok_or_else(wrong)?;
        if let Some(other) = fields.keys().find(|k| !matches!(k.as_str(), "name" | "items")) {
            return Err(format!("A group takes “name” and “items”, not “{}”.", one_line(other)));
        }
        let name = fields.get("name").and_then(Value::as_str).unwrap_or_default();
        let mut entries = Vec::new();
        for item in fields.get("items").and_then(Value::as_array).ok_or_else(wrong)? {
            let entry = match item {
                Value::String(text) => checklist::Entry { text: text.clone(), ..checklist::Entry::default() },
                Value::Object(item) => {
                    if let Some(other) = item.keys().find(|k| !matches!(k.as_str(), "text" | "state")) {
                        return Err(format!("An item takes “text” and “state”, not “{}”.", one_line(other)));
                    }
                    let mark = match item.get("state").and_then(Value::as_str) {
                        None => None,
                        Some(word) => Some(Mark::parse(word).ok_or_else(|| format!("An item's “state” is done, open or skipped; not “{}”.", one_line(word)))?),
                    };
                    checklist::Entry { text: item.get("text").and_then(Value::as_str).ok_or_else(wrong)?.to_string(), mark, detail: Vec::new() }
                }
                _ => return Err(wrong()),
            };
            if one_line(&entry.text).trim().is_empty() {
                return Err("An item has words.".into());
            }
            count += 1;
            entries.push(entry);
        }
        out.push(checklist::Group { name: one_line(name), items: entries });
    }
    if count > MOST_ITEMS {
        return Err(format!("A list holds {MOST_ITEMS} items at most: it is for scanning once."));
    }
    Ok(out)
}

/// The most items one list holds.
pub const MOST_ITEMS: usize = 60;

pub fn set_checklist(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let holder = holder(s, &desk, &args.needed("of")?)?;
    let groups = groups_asked(args)?;
    let notes = holder.notes();
    // Its free text kept; an item of the same words keeps its tick when no state is given.
    let written = checklist::written(&checklist::free_text(&notes), &groups, &notes);
    let title = holder.title();
    let mut lines = Vec::new();
    match holder.write_notes(s, &written)? {
        Some(change) => {
            let line = s.say("agent-list-written", &[("title", one_line(&title))]);
            say_in_window(line.clone(), vec![change]);
            lines.push(line);
        }
        None => lines.push("Nothing changed: the list says so already.".into()),
    }
    lines.extend(checklist_lines(&written));
    Ok(Answer { text: lines.join("\n"), data: json!({ "uri": holder.uri(), "checklist": checklist_json(&written), "sent": false }) })
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
        "notes": checklist::free_text(&task.notes),
        "checklist": checklist_json(&task.notes),
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

/// The steps of a bigger task an agent may see, in their sequence: one line
/// each, and as data.
fn steps_shown(desk: &Desk, parent: &str) -> (Vec<String>, Value) {
    let steps: Vec<&Task> = tasks::steps_of(&desk.loaded.tasks, parent).into_iter().filter(|t| desk.allows(&t.uid)).collect();
    let lines = steps.iter().enumerate().map(|(k, t)| format!("{}. {}  [{}]", k + 1, one_line(&t.title), t.uid)).collect();
    let data = steps.iter().map(|t| json!({ "uid": t.uid, "uri": links::task_uri(&t.uid), "title": t.title, "status": task_json(t)["status"] })).collect();
    (lines, Value::Array(data))
}

pub fn get_task(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let task = find_open_task(s, &desk, &args.needed("task")?)?;
    // Its steps in their order, those kept from agents left out.
    let (lines, steps) = steps_shown(&desk, &task.uid);
    let mut data = task_json(&task);
    data["steps"] = steps;
    let text = std::iter::once(format!("{} · {}  <{}>", one_line(&task.title), one_line(&task.list), links::task_uri(&task.uid))).chain(lines).chain(checklist_lines(&task.notes)).collect::<Vec<_>>().join("\n");
    Ok(Answer { text, data })
}

/// The most steps one call adds: a composed task is a few small steps.
pub const MOST_STEPS: usize = 30;

/// The steps asked: a title each, or {title, estimate}, in order.
fn steps_asked(args: &Args) -> Result<Vec<(String, u32)>, String> {
    let wrong = || "“steps” is a list of titles, or of {title, estimate}.".to_string();
    let items = args.raw("steps").and_then(Value::as_array).ok_or_else(wrong)?;
    if items.is_empty() || items.len() > MOST_STEPS {
        return Err(format!("“steps” holds 1 to {MOST_STEPS} steps."));
    }
    items
        .iter()
        .map(|item| {
            let (title, estimate) = match item {
                Value::String(title) => (title.as_str(), None),
                Value::Object(fields) => {
                    if let Some(other) = fields.keys().find(|k| !matches!(k.as_str(), "title" | "estimate")) {
                        return Err(format!("A step takes “title” and “estimate”, not “{}”.", one_line(other)));
                    }
                    (fields.get("title").and_then(Value::as_str).ok_or_else(wrong)?, fields.get("estimate"))
                }
                _ => return Err(wrong()),
            };
            let title = one_line(title).trim().to_string();
            if title.is_empty() {
                return Err("A step has a title.".into());
            }
            let minutes = match estimate {
                None | Some(Value::Null) => 0,
                Some(value) => value.as_u64().filter(|m| (1..=600).contains(m)).ok_or("A step's “estimate” is a whole number of minutes, 1 to 600.")? as u32,
            };
            Ok((title, minutes))
        })
        .collect()
}

pub fn add_steps(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let whole = find_open_task(s, &desk, &args.needed("task")?)?;
    may_change(&desk.consent, &links::task_uri(&whole.uid))?;
    if !whole.status.is_open() {
        return Err(format!("“{}” is done: steps of it would never be offered. Open it again first (update_task).", one_line(&whole.title)));
    }
    let asked = steps_asked(args)?;
    // In the bigger task's list, when it can be written; else where new tasks go.
    let list = tasks::lists().into_iter().find(|c| !c.read_only && format!("{}/{}", c.account, c.id) == whole.list_id).or_else(tasks::default_list).ok_or_else(|| s.tr.text("task-no-list-yet", None))?;
    let (zone, now) = (TimeZone::system(), Zoned::now());
    let mut place = tasks::next_in_sequence(tasks::steps_of(&desk.loaded.tasks, &whole.uid), &now);
    let mut files = Vec::new();
    for (title, estimate) in &asked {
        let edit = TaskEdit { title: title.clone(), estimate: *estimate, parent: whole.uid.clone(), sort_order: Some(place), ..TaskEdit::default() };
        place = place.saturating_add(tasks::SEQUENCE_GAP);
        let mut path = tasks::new_path(&list);
        while path.exists() {
            path = tasks::new_path(&list);
        }
        let text = tasks::new_task(&edit, &vdir::new_name(), &zone, &now)?;
        vdir::write_item(&path, &text)?;
        files.push(FileChange { path, before: None, after: Some(text) });
    }
    let mut counted = s.tr.counted(asked.len());
    counted.set("title", one_line(&whole.title));
    let line = s.tr.text("agent-steps-added", Some(&counted));
    say_in_window(line.clone(), files);
    let after = Desk::read(s);
    let (lines, steps) = steps_shown(&after, &whole.uid);
    Ok(Answer { text: std::iter::once(line).chain(lines).collect::<Vec<_>>().join("\n"), data: json!({ "uri": links::task_uri(&whole.uid), "steps": steps, "sent": false }) })
}

pub fn move_step(s: &Session, args: &Args) -> Result<Answer, String> {
    let desk = Desk::read(s);
    let step = find_open_task(s, &desk, &args.needed("step")?)?;
    may_change(&desk.consent, &links::task_uri(&step.uid))?;
    let Some(parent) = step.parent().map(str::to_string) else {
        return Err(s.say("task-not-a-step", &[("title", one_line(&step.title))]));
    };
    let whole = desk.loaded.tasks.iter().find(|t| t.uid == parent).map_or_else(|| parent.clone(), |t| t.title.clone());
    let before = match args.text("before")? {
        Some(wanted) => {
            let other = find_open_task(s, &desk, &wanted)?;
            if other.parent() != Some(parent.as_str()) {
                return Err(s.say("task-not-a-step-of", &[("title", one_line(&other.title)), ("whole", one_line(&whole))]));
            }
            Some(other.uid)
        }
        None => None,
    };
    let steps = tasks::steps_of(&desk.loaded.tasks, &parent);
    let writes = tasks::reorder(&steps, &step.uid, before.as_deref())?;
    let file_of = |uid: &str| steps.iter().find(|t| t.uid == uid).copied();
    // Renumbered, every step written is one the agent may change, in a list that can be written.
    for (uid, _) in &writes {
        let Some(other) = file_of(uid) else { continue };
        if other.read_only {
            return Err(s.tr.text("task-read-only", None));
        }
        if !desk.allows(uid) {
            return Err(access::kept("Another step of this task"));
        }
    }
    let mut files = Vec::new();
    for (uid, place) in &writes {
        let Some(other) = file_of(uid) else { continue };
        let current = std::fs::read_to_string(&other.key).map_err(|e| format!("{}: {e}", other.key))?;
        let text = tasks::set_sequence(&current, *place, &Zoned::now())?;
        if text != current {
            vdir::write_item(Path::new(&other.key), &text)?;
            files.push(FileChange::of(Path::new(&other.key), Some(current)));
        }
    }
    let after = Desk::read(s);
    let (lines, steps) = steps_shown(&after, &parent);
    if files.is_empty() {
        return Ok(Answer { text: std::iter::once("Nothing changed: the step is there already.".to_string()).chain(lines).collect::<Vec<_>>().join("\n"), data: json!({ "uri": links::task_uri(&step.uid), "changed": false, "steps": steps }) });
    }
    let line = s.say("agent-step-moved", &[("title", one_line(&step.title)), ("whole", one_line(&whole))]);
    say_in_window(line.clone(), files);
    Ok(Answer { text: std::iter::once(line).chain(lines).collect::<Vec<_>>().join("\n"), data: json!({ "uri": links::task_uri(&step.uid), "changed": true, "steps": steps, "sent": false }) })
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
        edit.notes = free_text_given(args, &edit.notes)?;
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
    // Notes its list would cut (Google Tasks keeps 8,192 characters): said, nothing written.
    if edit.notes != task.notes {
        let provider = tasks::lists().into_iter().find(|c| format!("{}/{}", c.account, c.id) == task.list_id).map(|c| sioul_core::capabilities::provider_of_collection(s.config.account(&c.account), &c));
        if provider.is_some_and(|p| sioul_core::capabilities::notes_too_long(p, &edit.notes)) {
            return Err(s.tr.text("notes-google-too-long", None));
        }
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
    // Done, its bigger task with it, when it was the last step (docs/tasks.md, "Composed tasks").
    if task.status != Status::Completed && sioul_core::tasks::task_of_text(&text, &zone).is_some_and(|t| t.status == Status::Completed) {
        let all = tasks::all(&zone);
        let closing = sioul_core::composed::close_with(&all, &task.uid, &now).map_err(|r| r.said(&s.tr))?;
        if let Some(whole) = sioul_core::composed::closing_with(&all, &task.uid).first().map(|t| t.title.clone()) {
            merge_files(&mut files, closing.write()?);
            line = format!("{line} {}", s.say("composed-done-too", &[("whole", one_line(&whole))]));
        }
    }
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
    // A step of another, or a task of its own again: after the rest, from the files as they are now.
    if let Some(parent) = args.text("parent")? {
        let all = tasks::all(&zone);
        let keeper = |c: &Collection| sioul_core::capabilities::provider_of_collection(s.config.account(&c.account), c);
        let (planned, said) = if parent.eq_ignore_ascii_case("none") {
            (sioul_core::composed::promote(&all, &task.uid, &now), s.say("agent-made-own", &[("title", one_line(&edit.title))]))
        } else {
            let open = desk.open_tasks();
            let whole = read::find_task(s, &open, &parent)?;
            may_change(&desk.consent, &links::task_uri(&whole.uid))?;
            (sioul_core::composed::demote(&all, &task.uid, &whole.uid, &tasks::lists(), &keeper, args.flag("confirm")?, &now), s.say("agent-made-step", &[("title", one_line(&edit.title)), ("whole", one_line(&whole.title))]))
        };
        let change = planned.map_err(|r| refused(s, r))?;
        if !change.is_empty() {
            merge_files(&mut files, change.write()?);
            line = said;
        }
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
