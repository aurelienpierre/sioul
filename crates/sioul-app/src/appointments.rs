// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A task made an appointment, an appointment made a task, from the agenda
//! (21 October 2026; `sioul_core::appointments`). One appointment
//! was a task whose time block looked like an event: which one an item is
//! stays the person's to say, never guessed from its times.
//!
//! - "Make it an appointment", on a task's block: the event form opens with
//!   the task's title, place, notes and the block's time, in the calendar
//!   Settings ▸ Agenda names for new events, or another chosen there. Saved,
//!   the event is written and the task taken out, with its time blocks to
//!   come and the block it was opened from, in one step of the move journal
//!   (`vdir::replace_items`): never both, never neither.
//! - "Make it a task", on an event of your own: a task in the usual list, on
//!   its day, its length its estimate; its time, when still to come and not
//!   repeating, its block, made in the calendar for blocks. Then the event is
//!   taken out, in the same one step. An invitation stays an event: others
//!   expect you there.

use crate::backend::{QtThread, Shared, load_config, say, tell, tr};
use crate::{pim, work};
use jiff::tz::TimeZone;
use jiff::Zoned;
use sioul_core::agenda::{self, EventEdit};
use sioul_core::vdir::{self, Kind};
use sioul_core::appointments;
use sioul_core::tasks::Task;
use sioul_core::vdir::Collection;
use std::path::PathBuf;
use std::sync::Arc;


/// Every account of these files told to send them.
fn nudge_all(shared: &Shared, paths: &[PathBuf]) {
    let mut told = std::collections::BTreeSet::new();
    for account in paths.iter().filter_map(|p| pim::account_of(p)) {
        if told.insert(account.clone()) {
            pim::nudge(shared, &account);
        }
    }
}

/// The pages read again: the task views and the agenda, the tasks as they are now.
fn shown_again(qt: &QtThread, shared: &Arc<Shared>) {
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    work::show_work(qt, shared);
    pim::show_pim(qt, shared);
}

/// The event form a task becomes, as JSON (`EventEdit`): at its block's time
/// (`start`, `end`, Unix seconds, from the agenda; 0 for none), else its own;
/// {"error"} when it is gone.
pub(crate) fn form(shared: &Shared, uid: &str, start: i64, end: i64) -> String {
    let zone = TimeZone::system();
    match work::find_task(shared, uid) {
        Ok(task) => {
            let time = appointments::time_of(&task, (end > start && start > 0).then_some((start, end)), &zone);
            let mut form = serde_json::to_value(appointments::event_of(&task, time, Zoned::now().date(), &zone)).unwrap_or_default();
            // What it cannot keep, said before it is saved (F5).
            let waiting = waiting_for(&work::loaded(shared).tasks, uid);
            form["losses"] = serde_json::Value::String(lost_sentence("appointment-not-kept", &appointments::task_losses(&task), waiting));
            form.to_string()
        }
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

/// How many open tasks wait for `uid`: they stop waiting once it is an appointment.
fn waiting_for(tasks: &[Task], uid: &str) -> usize {
    tasks.iter().filter(|t| t.uid != uid && t.status.is_open() && t.relations.iter().any(|r| r.uid == uid && r.kind != "PARENT" && r.kind != "CHILD")).count()
}

/// "Not kept as an appointment: the date asked, its tags." (`intro` the
/// sentence's id), or "" when everything goes along.
fn lost_sentence(intro: &str, lost: &[&str], waiting: usize) -> String {
    let mut words: Vec<String> = lost.iter().map(|id| tr().text(&format!("conversion-lost-{id}"), None)).collect();
    if waiting > 0 {
        words.push(say("conversion-lost-waiting", &[("n", waiting.to_string())]));
    }
    if words.is_empty() { String::new() } else { say(intro, &[("what", words.join(", "))]) }
}

/// What an event cannot keep as a task, in a sentence; "" when everything goes along (F5).
pub(crate) fn task_losses(key: &str) -> String {
    pim::ours(key).and_then(|p| std::fs::read_to_string(p).ok()).map_or_else(String::new, |text| lost_sentence("task-not-kept", &appointments::event_losses(&text), 0))
}



/// A task made an appointment: the event form `edit` (JSON) written in
/// `calendar` ("account/id"; else the agenda's usual), then the task taken out
/// with its time blocks to come and `block`, the block it was opened from.
/// Returns what went wrong, else "".
pub(crate) fn task_to_event(qt: &QtThread, shared: &Arc<Shared>, uid: &str, block: &str, edit: &str, calendar: &str) -> String {
    let loaded = work::loaded(shared);
    let made = work::find_task(shared, uid).and_then(|task| make_event(&task, &loaded.tasks, block, edit, calendar));
    match made {
        Ok((gone, made, name)) => {
            // Kept (a task with steps, paired with the event): nothing to tie again.
            let retied = if gone.is_empty() { Vec::new() } else { appointments::retie(&loaded.tasks, uid, &appointments::uid_in(&made), true, &Zoned::now()) };
            nudge_all(shared, &[gone, vec![made], retied].concat());
            tell(qt, shared, say("appointment-made", &[("calendar", name)]));
            shown_again(qt, shared);
            String::new()
        }
        Err(problem) => problem,
    }
}

/// The event `edit` written for `task` in `calendar`, the task and its blocks
/// taken out (`others`: every task, to find its steps): what went, the new
/// file, and the calendar's name. A task with open steps stays, paired with
/// the event it prepares (`pairs::appointment_for`, docs/tasks.md, "An
/// appointment and its steps").
fn make_event(task: &Task, others: &[Task], block: &str, edit: &str, calendar: &str) -> Result<(Vec<PathBuf>, PathBuf, String), String> {
    let uid = task.uid.as_str();
    let edit: EventEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
    let target = vdir::collections(Kind::Calendars)
        .into_iter()
        .find(|c| !c.read_only && c.holds("VEVENT") && format!("{}/{}", c.account, c.id) == calendar)
        .or_else(agenda::default_calendar)
        .ok_or_else(|| tr().text("dav-no-calendar", None))?;
    if others.iter().any(|t| t.status.is_open() && t.parent() == Some(uid)) {
        let config = load_config();
        let (mut change, path) = sioul_core::pairs::appointment_for(others, uid, &edit, &target, &sioul_core::pairs::links_kept(&config), &Zoned::now()).map_err(|r| r.said(tr()))?;
        // The block whose time the form took: the appointment is that time now (F14).
        change.gone.extend(pim::ours(block).filter(|p| std::fs::read_to_string(p).is_ok_and(|t| appointments::block_of(&t, uid) && !agenda::with_others(&t)) && pim::collection_of(p).is_some_and(|c| !c.read_only)));
        change.write()?;
        return Ok((Vec::new(), path, target.label(&config, tr())));
    }
    // Never a block given a guest: a meeting with others now, it stays as it is (`agenda::may_change`).
    let opened = pim::ours(block).filter(|p| std::fs::read_to_string(p).is_ok_and(|t| appointments::block_of(&t, uid) && !agenda::with_others(&t)) && pim::collection_of(p).is_some_and(|c| !c.read_only));
    let blocks: Vec<PathBuf> = opened.into_iter().chain(crate::blocks::with_task(uid).into_iter().map(|(_, p)| p)).collect();
    let plan = appointments::task_to_event(task, others, &blocks, &edit, &target, &TimeZone::system()).map_err(|r| r.said(tr()))?;
    plan.write()?;
    Ok((plan.gone, plan.to, target.label(&load_config(), tr())))
}

/// An event's file, as the pairing reads it: its text, whether its calendar can be written.
fn event_file(key: &str) -> Result<(PathBuf, String, bool), String> {
    let path = pim::ours(key).ok_or_else(|| tr().text("agenda-gone", None))?;
    let text = std::fs::read_to_string(&path).map_err(|_| tr().text("agenda-gone", None))?;
    let writable = pim::collection_of(&path).is_some_and(|c| !c.read_only);
    Ok((path, text, writable))
}

/// A pairing's files written and said with Undo (`agents::say`).
fn paired_done(qt: &QtThread, shared: &Arc<Shared>, change: sioul_core::composed::Change, line: String) -> Result<(), String> {
    let files = change.write()?;
    crate::agents::say(qt, shared, sioul_core::agentwrites::Said { at: 0, line, files });
    Ok(())
}

/// A task tied to the appointment in `key` (an event's file): its steps and
/// list then prepare it (`pairs::attach`). Returns what went wrong, else "".
pub(crate) fn attach_to_event(qt: &QtThread, shared: &Arc<Shared>, uid: &str, key: &str) -> String {
    let attached = || -> Result<(), String> {
        let (path, text, writable) = event_file(key)?;
        let loaded = work::loaded(shared);
        let all = sioul_core::tasks::all(&TimeZone::system());
        let event = sioul_core::pairs::EventFile { path: &path, text: &text, writable };
        let config = load_config();
        let change = sioul_core::pairs::attach(&all, uid, &event, &loaded.events, &sioul_core::pairs::links_kept(&config), &Zoned::now()).map_err(|r| r.said(tr()))?;
        let title = all.iter().find(|t| t.uid == uid).map(|t| t.title.clone()).unwrap_or_default();
        let summary = agenda::edit_of_text(&text, &TimeZone::system()).map(|e| e.title).unwrap_or_default();
        paired_done(qt, shared, change, say("pair-attached", &[("title", title), ("event", summary)]))
    };
    attached().err().unwrap_or_default()
}

/// A task untied from its appointment (`pairs::detach`): its date with it
/// when the pairing wrote it. Returns what went wrong, else "".
pub(crate) fn detach_from_event(qt: &QtThread, shared: &Arc<Shared>, uid: &str) -> String {
    let detached = || -> Result<(), String> {
        let all = sioul_core::tasks::all(&TimeZone::system());
        let task = all.iter().find(|t| t.uid == uid).ok_or_else(|| tr().text("task-gone", None))?;
        let event = sioul_core::pairs::appointment_of(task).and_then(|e| work::loaded(shared).events.iter().find(|r| r.uid == e).map(|r| r.key.clone())).and_then(|key| event_file(&key).ok());
        let file = event.as_ref().map(|(path, text, writable)| sioul_core::pairs::EventFile { path, text, writable: *writable });
        let change = sioul_core::pairs::detach(task, file.as_ref(), &Zoned::now()).map_err(|r| r.said(tr()))?;
        paired_done(qt, shared, change, say("pair-detached", &[("title", task.title.clone())]))
    };
    detached().err().unwrap_or_default()
}

/// A task made to prepare the event in `key`, titled as it, paired with it,
/// in the usual list: {"uid"} or {"error"}.
pub(crate) fn steps_for_event(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    let made = || -> Result<String, String> {
        let (path, text, writable) = event_file(key)?;
        let list = work::target_list("")?;
        let loaded = work::loaded(shared);
        let all = sioul_core::tasks::all(&TimeZone::system());
        let event = sioul_core::pairs::EventFile { path: &path, text: &text, writable };
        let config = load_config();
        let (change, uid) = sioul_core::pairs::new_for(&all, &event, &loaded.events, &list, &sioul_core::pairs::links_kept(&config), &Zoned::now()).map_err(|r| r.said(tr()))?;
        let summary = agenda::edit_of_text(&text, &TimeZone::system()).map(|e| e.title).unwrap_or_default();
        paired_done(qt, shared, change, say("pair-made", &[("title", summary.clone()), ("event", summary)]))?;
        Ok(uid)
    };
    match made() {
        Ok(uid) => serde_json::json!({ "uid": uid }).to_string(),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

/// What prepares the event in `key`, for its details: {task: {uid, title,
/// next} or null, adoptable: [{uid, title}] (open tasks tied to it, a
/// "Prepare: …" made from it, none paired), can: whether steps can be added}.
pub(crate) fn pairing(shared: &Shared, key: &str) -> String {
    let loaded = work::loaded(shared);
    let Some(event) = loaded.events.iter().find(|e| e.key == key) else { return "null".into() };
    let task = sioul_core::pairs::preparation_of(&loaded.tasks, event);
    let next = task.and_then(|t| sioul_core::tasks::steps_of(&loaded.tasks, &t.uid).into_iter().find(|s| s.status.is_open()).map(|s| s.title.clone()));
    let names_it = |t: &Task| t.links.iter().any(|l| l.rel != sioul_core::pairs::APPOINTMENT && (l.uri == format!("uid:{}", event.uid) || l.uri == sioul_core::links::event_uri(&event.uid)));
    let named: Vec<String> = event.links.iter().filter(|l| l.rel != sioul_core::pairs::PREPARATION).filter_map(|l| l.uri.strip_prefix("uid:").map(str::to_string).or_else(|| l.uri.starts_with("sioul:task/").then(|| sioul_core::links::id_of(&l.uri)))).collect();
    // Never offered what the tie would refuse: a task of a list that keeps no link (Google Tasks, `pairs::links_kept`).
    let config = load_config();
    let keeps = sioul_core::pairs::links_kept(&config);
    let adoptable: Vec<serde_json::Value> = if task.is_some() {
        Vec::new()
    } else {
        loaded.tasks.iter().filter(|t| t.status.is_open() && !t.read_only && keeps(&t.list_id) && sioul_core::pairs::appointment_of(t).is_none() && (names_it(t) || named.contains(&t.uid))).map(|t| serde_json::json!({ "uid": t.uid, "title": t.title })).collect()
    };
    let block = std::fs::read_to_string(&event.key).is_ok_and(|text| sioul_core::appointments::block_of(&text, ""));
    // "Add steps and a list" makes the task in the usual list: offered when that list keeps the tie.
    let usual = work::target_list("").is_ok_and(|list| keeps(&format!("{}/{}", list.account, list.id)));
    serde_json::json!({
        "task": task.map(|t| serde_json::json!({ "uid": t.uid, "title": t.title, "next": next })),
        "adoptable": adoptable,
        "can": task.is_none() && !block && usual,
    })
    .to_string()
}

/// An event made a task (`key`, its file): in the usual list, its time its
/// block's when still to come and not repeating, then the event taken out.
/// Returns what went wrong, else "".
pub(crate) fn event_to_task(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    let old = pim::ours(key).map(|p| appointments::uid_in(&p)).unwrap_or_default();
    match work::target_list("").and_then(|list| make_task(key, &list)) {
        Ok((gone, made, list)) => {
            let retied = if old.is_empty() { Vec::new() } else { appointments::retie(&work::loaded(shared).tasks, &old, &appointments::uid_in(&made[0]), false, &Zoned::now()) };
            nudge_all(shared, &[vec![gone], made, retied].concat());
            tell(qt, shared, say("task-made-from-event", &[("list", list)]));
            shown_again(qt, shared);
            String::new()
        }
        Err(problem) => problem,
    }
}

/// The event `key` made a task in `list`: the event's file, what was written
/// (the task, its block), and the list's name.
fn make_task(key: &str, list: &Collection) -> Result<(PathBuf, Vec<PathBuf>, String), String> {
    let path = pim::ours(key).ok_or_else(|| tr().text("agenda-gone", None))?;
    if pim::collection_of(&path).is_none_or(|c| c.read_only) {
        return Err(tr().text("agenda-read-only", None));
    }
    let plan = appointments::event_to_task(&path, list, &crate::blocks::calendar_for, load_config().tasks.block_alarms, &Zoned::now()).map_err(|r| r.said(tr()))?;
    plan.write()?;
    let written: Vec<PathBuf> = std::iter::once(plan.to.clone()).chain(plan.made.iter().map(|(p, _)| p.clone())).collect();
    Ok((path, written, list.label(&load_config(), tr())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::tasks;

    /// Whether a file is a block of a task, or of this one.
    #[test]
    fn a_block_is_known_by_its_task() {
        let block = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:b\r\nX-SIOUL-TASK:t1\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert!(appointments::block_of(block, "t1") && appointments::block_of(block, "") && !appointments::block_of(block, "t2"));
        assert!(!appointments::block_of("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:e\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", ""));
    }

    /// The files of an account's calendars and lists that hold `what`.
    fn holding(account: &str, what: &str) -> Vec<PathBuf> {
        vdir::collections(Kind::Calendars).into_iter().filter(|c| c.account == account).flat_map(|c| c.items()).filter(|p| std::fs::read_to_string(p).is_ok_and(|t| t.contains(what))).collect()
    }

    /// An appointment kept as a task in a list,
    /// shown by its time block: made an appointment at the block's time, the
    /// task and its block gone; made a task again, the event gone, the task
    /// in the list with its length, its time its block. One file each time.
    #[test]
    fn a_task_becomes_an_appointment_and_back_once() {
        let account = "kinds-stand-in";
        let list = vdir::create(Kind::Calendars, account, "Plan", None, &["VTODO"]).unwrap();
        vdir::prepare(Kind::Calendars, account, "personal", "Personal", None).unwrap();
        let personal = vdir::collections(Kind::Calendars).into_iter().find(|c| c.account == account && c.id == "personal").unwrap();
        let task_path = list.dir.join("t-kinds.ics");
        std::fs::write(&task_path, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:t-kinds\r\nSUMMARY:Dentist\r\nLOCATION:1 Example Street\r\nDTSTART;VALUE=DATE:20991023\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
        let block_path = personal.dir.join("b-kinds.ics");
        std::fs::write(&block_path, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:b-kinds\r\nDTSTART:20991023T130000Z\r\nDTEND:20991023T140000Z\r\nSUMMARY:Dentist\r\nX-SIOUL-TASK:t-kinds\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let zone = TimeZone::system();
        let task = tasks::read(&task_path, &list, &zone).unwrap();
        let (start, end) = (4_096_443_600, 4_096_447_200);
        let edit = appointments::event_of(&task, appointments::time_of(&task, Some((start, end)), &zone), Zoned::now().date(), &zone);
        let edit = serde_json::to_string(&edit).unwrap();
        let calendar = format!("{account}/personal");

        // A task with steps stays a task, tied to the event made, which it prepares
        // (docs/tasks.md, "An appointment and its steps"): its steps keep the task they belong to.
        let original = std::fs::read_to_string(&task_path).unwrap();
        let block_text = std::fs::read_to_string(&block_path).unwrap();
        let step = Task { uid: "s".into(), relations: vec![tasks::Relation { kind: "PARENT".into(), uid: "t-kinds".into(), gap: 0 }], ..Task::default() };
        let (gone, made, _) = make_event(&task, &[task.clone(), step], block_path.to_str().unwrap(), &edit, &calendar).unwrap();
        assert!(gone.is_empty() && task_path.exists(), "the task stays");
        // F14: the block whose time the form took is the appointment's time now: it goes.
        assert!(!block_path.exists(), "no block beside the appointment at the same time");
        assert!(std::fs::read_to_string(&made).unwrap().contains("sioul:task/t-kinds"));
        assert!(std::fs::read_to_string(&task_path).unwrap().contains("rel/appointment"));
        // Untied for what follows: the event out, the task and its block as they were.
        std::fs::remove_file(&made).unwrap();
        std::fs::write(&task_path, original).unwrap();
        std::fs::write(&block_path, block_text).unwrap();

        // Done, it stays a task: the record of work done (F5).
        let done = Task { status: tasks::Status::Completed, ..task.clone() };
        assert!(make_event(&done, std::slice::from_ref(&done), block_path.to_str().unwrap(), &edit, &calendar).is_err());
        // A task that waits for it, and links it: its links follow, its wait goes (F5).
        let waiting_path = list.dir.join("w-kinds.ics");
        std::fs::write(&waiting_path, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:w-kinds\r\nSUMMARY:Pay\r\nRELATED-TO;RELTYPE=DEPENDS-ON:t-kinds\r\nLINK;LINKREL=related;VALUE=URI:sioul:task/t-kinds\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
        let waiting = tasks::read(&waiting_path, &list, &zone).unwrap();
        assert_eq!(waiting_for(std::slice::from_ref(&waiting), "t-kinds"), 1);
        let (gone, made, _) = make_event(&task, std::slice::from_ref(&task), block_path.to_str().unwrap(), &edit, &calendar).unwrap();
        assert_eq!(appointments::retie(std::slice::from_ref(&waiting), "t-kinds", &appointments::uid_in(&made), true, &Zoned::now()), [waiting_path.clone()]);
        let retied = std::fs::read_to_string(&waiting_path).unwrap();
        assert!(!retied.contains("RELATED-TO") && retied.contains(&format!("sioul:event/{}", appointments::uid_in(&made))), "{retied}");
        std::fs::remove_file(&waiting_path).unwrap();
        assert_eq!(gone, [task_path.clone(), block_path.clone()]);
        assert!(!task_path.exists() && !block_path.exists(), "the task and its block gone");
        let event = std::fs::read_to_string(&made).unwrap();
        assert!(event.contains("BEGIN:VEVENT") && event.contains("SUMMARY:Dentist") && event.contains("LOCATION:1 Example Street") && !event.contains("X-SIOUL-TASK"), "{event}");
        assert_eq!(holding(account, "Dentist"), [made.clone()], "one file: the appointment");

        // Back: a task in the list, an hour, its time its block's; its links with it.
        let linked = std::fs::read_to_string(&made).unwrap().replace("END:VEVENT", "LINK;LINKREL=via;VALUE=URI:mid:letter@example.org\r\nEND:VEVENT");
        std::fs::write(&made, linked).unwrap();
        let (gone, written, _) = make_task(made.to_str().unwrap(), &list).unwrap();
        assert!(std::fs::read_to_string(&written[0]).unwrap().contains("mid:letter@example.org"), "the event's links go along");
        assert_eq!(gone, made);
        assert!(!made.exists(), "the event gone");
        let task_text = std::fs::read_to_string(&written[0]).unwrap();
        assert!(task_text.contains("BEGIN:VTODO") && task_text.contains("ESTIMATED-DURATION:PT1H") && task_text.contains("LOCATION:1 Example Street") && task_text.contains("DTSTART;VALUE=DATE:20991023"), "{task_text}");
        let block = std::fs::read_to_string(&written[1]).unwrap();
        let uid = tasks::task_of_text(&task_text, &zone).unwrap().uid;
        assert!(block.contains(&format!("X-SIOUL-TASK:{uid}")) && block.contains("DTSTART:20991023T130000Z") && task_text.contains("time-block"), "{block}\n{task_text}");
        assert_eq!(holding(account, "Dentist").len(), 2, "the task and its block, nothing else");
        assert!(make_task(written[1].to_str().unwrap(), &list).is_err(), "a block is a task already");

        // An invitation stays an event; a repeat the task cannot say is never lost.
        let invited = personal.dir.join("i-kinds.ics");
        std::fs::write(&invited, "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:i\r\nDTSTART:20991024T090000Z\r\nDTEND:20991024T100000Z\r\nSUMMARY:Meeting\r\nORGANIZER:mailto:boss@example.org\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:me@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        // An invitation names you among its guests (an organizer alone tells nobody: no meeting, 26 October 2026).
        assert!(make_task(invited.to_str().unwrap(), &list).is_err() && invited.exists());
        let odd = personal.dir.join("o-kinds.ics");
        std::fs::write(&odd, "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:o\r\nDTSTART:20991024T090000Z\r\nDTEND:20991024T100000Z\r\nSUMMARY:Odd\r\nRRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=TU\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        assert!(make_task(odd.to_str().unwrap(), &list).is_err() && odd.exists());
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(account));
    }
}
