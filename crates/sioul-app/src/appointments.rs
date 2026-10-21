// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A task made an appointment, an appointment made a task, from the agenda
//! (21 October 2026; `sioul_core::appointments`). The owner's appointment
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

use crate::backend::{QtThread, Shared, json, load_config, say, tell, tr};
use crate::{pim, work};
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use sioul_core::agenda::{self, EventEdit};
use sioul_core::vdir::{self, Kind};
use sioul_core::{appointments, blocks, lines, tasks};
use sioul_core::tasks::Task;
use sioul_core::vdir::Collection;
use std::path::PathBuf;
use std::sync::Arc;

/// Whether an event's text is the time block of the task `uid` (any task when `uid` is "").
fn block_of(text: &str, uid: &str) -> bool {
    lines::unfold(text).iter().any(|l| lines::name(l) == blocks::TASK && (uid.is_empty() || lines::value(l).trim() == uid))
}

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
            json(&appointments::event_of(&task, time, Zoned::now().date(), &zone))
        }
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

/// A task made an appointment: the event form `edit` (JSON) written in
/// `calendar` ("account/id"; else the agenda's usual), then the task taken out
/// with its time blocks to come and `block`, the block it was opened from.
/// Returns what went wrong, else "".
pub(crate) fn task_to_event(qt: &QtThread, shared: &Arc<Shared>, uid: &str, block: &str, edit: &str, calendar: &str) -> String {
    let made = work::find_task(shared, uid).and_then(|task| make_event(&task, &work::loaded(shared).tasks, block, edit, calendar));
    match made {
        Ok((gone, made, name)) => {
            nudge_all(shared, &[gone, vec![made]].concat());
            tell(qt, shared, say("appointment-made", &[("calendar", name)]));
            shown_again(qt, shared);
            String::new()
        }
        Err(problem) => problem,
    }
}

/// The event `edit` written for `task` in `calendar`, the task and its blocks
/// taken out (`others`: every task, to find its steps): what went, the new
/// file, and the calendar's name.
fn make_event(task: &Task, others: &[Task], block: &str, edit: &str, calendar: &str) -> Result<(Vec<PathBuf>, PathBuf, String), String> {
    let uid = task.uid.as_str();
    if task.read_only {
        return Err(tr().text("task-read-only", None));
    }
    // Its steps would lose the task they belong to.
    if others.iter().any(|t| t.status.is_open() && t.parent() == Some(uid)) {
        return Err(tr().text("appointment-has-steps", None));
    }
    let edit: EventEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
    if edit.title.trim().is_empty() {
        return Err(tr().text("event-no-title", None));
    }
    let target = vdir::collections(Kind::Calendars)
        .into_iter()
        .find(|c| !c.read_only && c.holds("VEVENT") && format!("{}/{}", c.account, c.id) == calendar)
        .or_else(agenda::default_calendar)
        .ok_or_else(|| tr().text("dav-no-calendar", None))?;
    let zone = TimeZone::system();
    let text = agenda::new_event(&edit, &zone)?;
    // What the task was tied to, the event is (its blocks aside: they go).
    let links: Vec<String> = task.links.iter().filter(|l| !blocks::is_block_link(l)).map(tasks::link_line).collect();
    let text = agenda::add_lines(&text, &links).unwrap_or(text);
    let mut gone = vec![PathBuf::from(&task.key)];
    let opened = pim::ours(block).filter(|p| std::fs::read_to_string(p).is_ok_and(|t| block_of(&t, uid)) && pim::collection_of(p).is_some_and(|c| !c.read_only));
    for path in opened.into_iter().chain(crate::blocks::with_task(uid).into_iter().map(|(_, p)| p)) {
        if !gone.contains(&path) {
            gone.push(path);
        }
    }
    let path = agenda::new_path(&target);
    vdir::replace_items(&gone, &[], &path, &text)?;
    Ok((gone, path, target.label(&load_config(), tr())))
}

/// An event made a task (`key`, its file): in the usual list, its time its
/// block's when still to come and not repeating, then the event taken out.
/// Returns what went wrong, else "".
pub(crate) fn event_to_task(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    match work::target_list("").and_then(|list| make_task(key, &list)) {
        Ok((gone, made, list)) => {
            nudge_all(shared, &[vec![gone], made].concat());
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
    let text = std::fs::read_to_string(&path).map_err(|_| tr().text("agenda-gone", None))?;
    if block_of(&text, "") {
        return Err(tr().text("task-made-already", None));
    }
    let unfolded = lines::unfold(&text);
    if unfolded.iter().any(|l| matches!(lines::name(l).as_str(), "ATTENDEE" | "ORGANIZER")) {
        return Err(tr().text("task-from-invitation", None));
    }
    let zone = TimeZone::system();
    let event = agenda::edit_of_text(&text, &zone).ok_or_else(|| tr().text("agenda-gone", None))?;
    // A repeat a task cannot say (every other Tuesday, until a day) is not lost: a task's is FREQ alone.
    let plain = format!("FREQ={}", event.repeat.to_ascii_uppercase());
    if unfolded.iter().filter(|l| lines::name(l) == "RRULE").any(|l| event.repeat.is_empty() || !lines::value(l).trim().eq_ignore_ascii_case(&plain))
        || unfolded.iter().any(|l| matches!(lines::name(l).as_str(), "RDATE" | "EXDATE" | "RECURRENCE-ID"))
    {
        return Err(tr().text("task-from-odd-repeat", None));
    }
    let edit = appointments::task_of(&event);
    let now = Zoned::now();
    let uid = vdir::new_name();
    let mut task_text = tasks::new_task(&edit, &uid, &zone, &now)?;
    let task_path = list.dir.join(format!("{uid}.ics"));
    let mut made = Vec::new();
    // Its time, still to come: the task's block, so that it keeps it, shown as a task.
    let at = |text: &str| text.parse::<DateTime>().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map(|z| z.timestamp().as_second());
    if !event.all_day
        && event.repeat.is_empty()
        && let (Some(start), Some(end)) = (at(&event.start), at(&event.end))
        && end > start
        && end > Timestamp::now().as_second()
        && let Some(mut task) = tasks::task_of_text(&task_text, &zone)
    {
        task.list_id = format!("{}/{}", list.account, list.id);
        let calendar = crate::blocks::calendar_for(&task)?;
        let block_uid = vdir::new_name();
        let block = blocks::new_block(&task, &block_uid, start, end, load_config().tasks.block_alarms, &now)?;
        made.push((calendar.dir.join(format!("{block_uid}.ics")), block));
        task_text = blocks::task_pinned(&task_text, Some(&block_uid), &now)?;
    }
    vdir::replace_items(std::slice::from_ref(&path), &made, &task_path, &task_text)?;
    let written: Vec<PathBuf> = std::iter::once(task_path).chain(made.into_iter().map(|(p, _)| p)).collect();
    Ok((path, written, list.label(&load_config(), tr())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether a file is a block of a task, or of this one.
    #[test]
    fn a_block_is_known_by_its_task() {
        let block = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:b\r\nX-SIOUL-TASK:t1\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert!(block_of(block, "t1") && block_of(block, "") && !block_of(block, "t2"));
        assert!(!block_of("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:e\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", ""));
    }

    /// The files of an account's calendars and lists that hold `what`.
    fn holding(account: &str, what: &str) -> Vec<PathBuf> {
        vdir::collections(Kind::Calendars).into_iter().filter(|c| c.account == account).flat_map(|c| c.items()).filter(|p| std::fs::read_to_string(p).is_ok_and(|t| t.contains(what))).collect()
    }

    /// The owner's appointment of 21 October 2026, a task in his list "Plan"
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

        // A task with steps stays a task: they would lose the task they belong to.
        let step = Task { uid: "s".into(), relations: vec![tasks::Relation { kind: "PARENT".into(), uid: "t-kinds".into(), gap: 0 }], ..Task::default() };
        assert!(make_event(&task, &[task.clone(), step], block_path.to_str().unwrap(), &edit, &calendar).is_err());
        assert!(task_path.exists() && block_path.exists());

        let (gone, made, _) = make_event(&task, std::slice::from_ref(&task), block_path.to_str().unwrap(), &edit, &calendar).unwrap();
        assert_eq!(gone, [task_path.clone(), block_path.clone()]);
        assert!(!task_path.exists() && !block_path.exists(), "the task and its block gone");
        let event = std::fs::read_to_string(&made).unwrap();
        assert!(event.contains("BEGIN:VEVENT") && event.contains("SUMMARY:Dentist") && event.contains("LOCATION:1 Example Street") && !event.contains("X-SIOUL-TASK"), "{event}");
        assert_eq!(holding(account, "Dentist"), [made.clone()], "one file: the appointment");

        // Back: a task in the list, an hour, its time its block's.
        let (gone, written, _) = make_task(made.to_str().unwrap(), &list).unwrap();
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
        std::fs::write(&invited, "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:i\r\nDTSTART:20991024T090000Z\r\nDTEND:20991024T100000Z\r\nSUMMARY:Meeting\r\nORGANIZER:mailto:boss@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        assert!(make_task(invited.to_str().unwrap(), &list).is_err() && invited.exists());
        let odd = personal.dir.join("o-kinds.ics");
        std::fs::write(&odd, "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:o\r\nDTSTART:20991024T090000Z\r\nDTEND:20991024T100000Z\r\nSUMMARY:Odd\r\nRRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=TU\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        assert!(make_task(odd.to_str().unwrap(), &list).is_err() && odd.exists());
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(account));
    }
}
