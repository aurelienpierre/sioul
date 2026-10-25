// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An appointment and the task that prepares it (docs/tasks.md, "An
//! appointment and its steps"). A CalDAV file holds one kind of thing (RFC
//! 4791 §4.1), so an appointment with steps is two files shown as one: the
//! event stays a plain event, with its guests, its place, its alarms; the
//! task holds the steps and the list, its title yours. They are tied both
//! ways, as a time block is to its task (`blocks`), by RFC 9253 links whose
//! relation is a tag URI (RFC 4151) and whose value is Sioul's address of the
//! other (`VALUE=URI`: RFC 9253 §2 wants a `UID` value in the same
//! collection, and the pair crosses a calendar and a task list):
//!
//! - in the task: `LINK;LINKREL="tag:aurelienpierre.com,2026:sioul/rel/appointment";VALUE=URI:sioul:event/<UID>`
//! - in the event: `LINK;LINKREL="tag:aurelienpierre.com,2026:sioul/rel/preparation";VALUE=URI:sioul:task/<UID>`
//!
//! An event that cannot be written (a calendar you only read, a meeting with
//! others, whose server would tell every guest) keeps no line: the task's is
//! enough, and the pair is found from either end (`preparation_of`).
//!
//! - **Its date**: the task's DUE becomes the appointment's day when it has
//!   none or a later one; the day written is kept on its link
//!   (`X-SIOUL-DUE`), so that it follows the appointment while it still says
//!   that day (`followed`), and goes with a detach only then (`detach`).
//! - **Its steps** are free on the appointment's day, not before, unless
//!   one has a day of its own (`plan`).
//! - **The appointment gone** (deleted, cancelled, or not here yet): the
//!   task and its steps wait for a date, offered nowhere, until another is
//!   attached (`plan`, `Planned::parked`).
//! - **Never** a time block, nor an appointment of a repeating event's date:
//!   a repeating appointment keeps the task's date as it is.

use crate::agenda::{self, EventRef};
use crate::composed::Change;
use crate::links;
use crate::tasks::{self, Status, Task, TaskEdit};
use jiff::Zoned;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The relation of the task's link to its appointment.
pub const APPOINTMENT: &str = "tag:aurelienpierre.com,2026:sioul/rel/appointment";
/// The relation of the event's link to the task that prepares it.
pub const PREPARATION: &str = "tag:aurelienpierre.com,2026:sioul/rel/preparation";
/// The day the pairing wrote as the task's DUE, on its link: what follows the appointment.
pub const DUE_WRITTEN: &str = "X-SIOUL-DUE";

/// Why a pairing is refused, as a word of the translations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub &'static str);

impl Refused {
    pub fn said(&self, tr: &crate::i18n::Translator) -> String {
        tr.text(self.0, None)
    }
}

/// What the plan knows of an appointment: its day, whether it was cancelled, whether it comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appointment {
    pub day: Date,
    pub cancelled: bool,
    pub recurring: bool,
}

/// Every event, by its UID, as the plan knows appointments (`plan::Settings::appointments`).
pub fn appointments_of(events: &[EventRef], zone: &TimeZone) -> BTreeMap<String, Appointment> {
    events
        .iter()
        .filter_map(|e| {
            let day = jiff::Timestamp::from_second(e.start).ok()?.to_zoned(zone.clone()).date();
            Some((e.uid.clone(), Appointment { day, cancelled: e.cancelled, recurring: e.recurring }))
        })
        .collect()
}

/// The UID of the appointment a task prepares.
pub fn appointment_of(task: &Task) -> Option<String> {
    task.links.iter().find(|l| l.rel == APPOINTMENT && l.uri.starts_with("sioul:event/")).map(|l| links::id_of(&l.uri))
}

/// Whether a LINK line is a pair's.
fn pair_line(line: &str, rel: &str) -> bool {
    crate::lines::name(line) == "LINK" && tasks::link_of(line).rel == rel
}

/// The task that prepares the event `uid`: the open one whose link names it,
/// else the one the event's own link names when that task names no other.
pub fn preparation_of<'a>(tasks: &'a [Task], event: &EventRef) -> Option<&'a Task> {
    let named = |t: &&Task| appointment_of(t).as_deref() == Some(event.uid.as_str());
    tasks.iter().filter(named).find(|t| t.status.is_open()).or_else(|| tasks.iter().find(named)).or_else(|| {
        let uid = event.links.iter().find(|l| l.rel == PREPARATION && l.uri.starts_with("sioul:task/")).map(|l| links::id_of(&l.uri))?;
        tasks.iter().find(|t| t.uid == uid && appointment_of(t).is_none_or(|other| other == event.uid))
    })
}

/// The DUE the pairing wrote, read from the task's link.
fn due_written(text: &str) -> Option<Date> {
    crate::lines::unfold(text).iter().find(|l| pair_line(l, APPOINTMENT)).and_then(|l| crate::lines::param(l, DUE_WRITTEN)).and_then(|d| Date::strptime("%Y%m%d", d.trim()).ok())
}

/// The task's link to its appointment, with the day it wrote as DUE.
fn appointment_line(event_uid: &str, due: Option<Date>) -> String {
    let due = due.map(|d| format!(";{DUE_WRITTEN}={}", d.strftime("%Y%m%d"))).unwrap_or_default();
    format!("LINK;LINKREL=\"{APPOINTMENT}\"{due};VALUE=URI:{}", links::event_uri(event_uid))
}

/// The event's link to the task that prepares it.
fn preparation_line(task_uid: &str) -> String {
    format!("LINK;LINKREL=\"{PREPARATION}\";VALUE=URI:{}", links::task_uri(task_uid))
}

/// An event in a calendar: its file, its text, whether its calendar can be written.
pub struct EventFile<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub writable: bool,
}

impl EventFile<'_> {
    /// Its link may be written: its calendar can be, and no guest would hear of it.
    fn takes_lines(&self) -> bool {
        self.writable && !agenda::with_others(self.text)
    }
}

/// The task `uid` paired with an event (the module's rules): the task's link
/// and, when it can be written, the event's; the task's DUE the appointment's
/// day when it has none or a later one (never for a repeating appointment).
/// Refused for a task you only read or done, an event that is a time block or
/// another appointment's leg, an event another open task prepares already.
pub fn attach(all: &[Task], uid: &str, event: &EventFile, refs: &[EventRef], now: &Zoned) -> Result<Change, Refused> {
    let task = all.iter().find(|t| t.uid == uid).ok_or(Refused("task-gone"))?;
    let current = std::fs::read_to_string(&task.key).map_err(|_| Refused("task-gone"))?;
    let (text, event_text) = paired(all, task, &current, event, refs, now)?;
    let mut change = Change { written: vec![(PathBuf::from(&task.key), text)], ..Change::default() };
    change.written.extend(event_text.map(|t| (event.path.to_path_buf(), t)));
    Ok(change)
}

/// The task's text paired with the event, and the event's when it changes (`attach`).
fn paired(all: &[Task], task: &Task, current: &str, event: &EventFile, refs: &[EventRef], now: &Zoned) -> Result<(String, Option<String>), Refused> {
    let zone = TimeZone::system();
    if task.read_only {
        return Err(Refused("task-read-only"));
    }
    if !task.status.is_open() {
        return Err(Refused("composed-done"));
    }
    if crate::appointments::block_of(event.text, "") {
        return Err(Refused("pair-block"));
    }
    let reference = agenda::event_ref(event.text, &event.path.display().to_string(), &zone).ok_or(Refused("agenda-gone"))?;
    if reference.related.iter().any(|r| r.kind == "PARENT") {
        return Err(Refused("pair-leg"));
    }
    if preparation_of(all, &reference).is_some_and(|t| t.uid != task.uid && t.status.is_open()) {
        return Err(Refused("pair-taken"));
    }
    let known = refs.iter().find(|e| e.uid == reference.uid).cloned().unwrap_or(reference.clone());
    let day = jiff::Timestamp::from_second(known.start).ok().map(|t| t.to_zoned(zone.clone()).date());
    // Its date: the appointment's day, when it has none or a later one; never a repeating one's.
    let mut edit = TaskEdit::of(task);
    let mut written_due = None;
    if let Some(day) = day.filter(|_| !known.recurring && known.start > 0)
        && task.due_date().is_none_or(|due| due > day)
    {
        edit.due = day.to_string();
        written_due = Some(day);
    }
    let text = tasks::apply(current, &edit, &zone, now).map_err(|_| Refused("task-gone"))?;
    let text = tasks::replace_lines(&text, |l| pair_line(l, APPOINTMENT), vec![appointment_line(&reference.uid, written_due)], now).map_err(|_| Refused("task-gone"))?;
    let event_text = if event.takes_lines() {
        let without = agenda::remove_lines(event.text, |l| pair_line(l, PREPARATION)).ok_or(Refused("agenda-gone"))?;
        let with = agenda::add_lines(&without, &[preparation_line(&task.uid)]).ok_or(Refused("agenda-gone"))?;
        (with != event.text).then_some(with)
    } else {
        None
    };
    Ok((text, event_text))
}

/// The pair undone: the task's link out, its DUE with it when it still says
/// the day the pairing wrote; the event's link out when it holds one and can
/// be written (`event`: its file, when it is here). The legs stay with the
/// event.
pub fn detach(task: &Task, event: Option<&EventFile>, now: &Zoned) -> Result<Change, Refused> {
    if task.read_only {
        return Err(Refused("task-read-only"));
    }
    let zone = TimeZone::system();
    let current = std::fs::read_to_string(&task.key).map_err(|_| Refused("task-gone"))?;
    if !crate::lines::unfold(&current).iter().any(|l| pair_line(l, APPOINTMENT)) {
        return Err(Refused("pair-none"));
    }
    let mut text = current.clone();
    if due_written(&current).is_some_and(|d| task.due_date() == Some(d)) {
        text = tasks::apply(&text, &TaskEdit { due: String::new(), ..TaskEdit::of(task) }, &zone, now).map_err(|_| Refused("task-gone"))?;
    }
    let text = tasks::remove_lines(&text, |l| pair_line(l, APPOINTMENT), now).map_err(|_| Refused("task-gone"))?;
    let mut change = Change { written: vec![(PathBuf::from(&task.key), text)], ..Change::default() };
    if let Some(event) = event.filter(|e| e.writable)
        && crate::lines::unfold(event.text).iter().any(|l| pair_line(l, PREPARATION) && tasks::link_of(l).uri == links::task_uri(&task.uid))
    {
        let without = agenda::remove_lines(event.text, |l| pair_line(l, PREPARATION) && tasks::link_of(l).uri == links::task_uri(&task.uid)).ok_or(Refused("agenda-gone"))?;
        change.written.push((event.path.to_path_buf(), without));
    }
    Ok(change)
}

/// The task's text with its DUE moved to its appointment's day, when the
/// pairing wrote the DUE it has and the appointment moved: None when nothing
/// is to change (its DUE set by you since, a repeating appointment, the same
/// day). Written at each re-plan, the same twice.
pub fn followed(task: &Task, text: &str, appointment: &Appointment, now: &Zoned) -> Option<String> {
    if appointment.recurring || appointment.cancelled || task.read_only || !task.status.is_open() {
        return None;
    }
    let written = due_written(text)?;
    if task.due_date() != Some(written) || written == appointment.day {
        return None;
    }
    let zone = TimeZone::system();
    let uid = appointment_of(task)?;
    let moved = tasks::apply(text, &TaskEdit { due: appointment.day.to_string(), ..TaskEdit::of(task) }, &zone, now).ok()?;
    tasks::replace_lines(&moved, |l| pair_line(l, APPOINTMENT), vec![appointment_line(&uid, Some(appointment.day))], now).ok()
}

/// Every paired task whose DUE follows its appointment (`followed`): (file, new text).
pub fn upkeep(all: &[Task], appointments: &BTreeMap<String, Appointment>, now: &Zoned) -> Vec<(PathBuf, String)> {
    all.iter()
        .filter_map(|task| {
            let appointment = appointments.get(&appointment_of(task)?)?;
            let text = std::fs::read_to_string(&task.key).ok()?;
            Some((PathBuf::from(&task.key), followed(task, &text, appointment, now)?))
        })
        .collect()
}

/// A task to prepare the event (its steps and its list): titled as the
/// event, paired with it, in `list`. Its UID with the change.
pub fn new_for(all: &[Task], event: &EventFile, refs: &[EventRef], list: &crate::vdir::Collection, now: &Zoned) -> Result<(Change, String), Refused> {
    let zone = TimeZone::system();
    let edit = agenda::edit_of_text(event.text, &zone).ok_or(Refused("agenda-gone"))?;
    let uid = crate::vdir::new_name();
    let text = tasks::new_task(&TaskEdit { title: edit.title.clone(), status: Status::NeedsAction, ..TaskEdit::default() }, &uid, &zone, now).map_err(|_| Refused("task-gone"))?;
    let path = list.dir.join(format!("{uid}.ics"));
    let task = Task { key: path.display().to_string(), ..tasks::task_of_text(&text, &zone).ok_or(Refused("task-gone"))? };
    let (text, event_text) = paired(all, &task, &text, event, refs, now)?;
    let mut change = Change { made: vec![(path, text)], ..Change::default() };
    change.written.extend(event_text.map(|t| (event.path.to_path_buf(), t)));
    Ok((change, uid))
}

/// An event made from a task that has steps (`appointments::task_to_event`
/// refuses to take such a task out): the event `edit` in `calendar`, paired
/// with the task, which stays with its steps. The event's file with the change.
pub fn appointment_for(all: &[Task], uid: &str, edit: &agenda::EventEdit, calendar: &crate::vdir::Collection, now: &Zoned) -> Result<(Change, PathBuf), Refused> {
    let zone = TimeZone::system();
    if calendar.read_only || !calendar.holds("VEVENT") {
        return Err(Refused("dav-no-calendar"));
    }
    if edit.title.trim().is_empty() {
        return Err(Refused("event-no-title"));
    }
    let task = all.iter().find(|t| t.uid == uid).ok_or(Refused("task-gone"))?;
    let current = std::fs::read_to_string(&task.key).map_err(|_| Refused("task-gone"))?;
    let text = agenda::new_event(edit, &zone).map_err(|_| Refused("event-no-title"))?;
    let path = agenda::new_path(calendar);
    let event = EventFile { path: &path, text: &text, writable: true };
    let reference = agenda::event_ref(&text, &path.display().to_string(), &zone).ok_or(Refused("agenda-gone"))?;
    let (task_text, event_text) = paired(all, task, &current, &event, std::slice::from_ref(&reference), now)?;
    let change = Change { written: vec![(PathBuf::from(&task.key), task_text)], made: vec![(path.clone(), event_text.unwrap_or(text))], ..Change::default() };
    Ok((change, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vdir::{self, Collection, Kind};

    fn now() -> Zoned {
        "2026-10-25T10:00:00+01:00[Europe/Paris]".parse().unwrap()
    }

    struct Desk {
        list: Collection,
        calendar: Collection,
    }

    impl Desk {
        fn new(name: &str) -> Desk {
            let list = vdir::create(Kind::Calendars, vdir::LOCAL, &format!("{name} tasks"), None, &["VTODO"]).unwrap();
            let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, &format!("{name} agenda"), None, &["VEVENT"]).unwrap();
            Desk { list, calendar }
        }

        fn task(&self, uid: &str, edit: TaskEdit) {
            vdir::write_item(&self.list.dir.join(format!("{uid}.ics")), &tasks::new_task(&TaskEdit { title: uid.into(), ..edit }, uid, &TimeZone::system(), &now()).unwrap()).unwrap();
        }

        fn event(&self, uid: &str, lines: &str) -> PathBuf {
            let path = self.calendar.dir.join(format!("{uid}.ics"));
            std::fs::write(&path, format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261020T100000Z\r\nDTSTART:20261029T130000Z\r\nDTEND:20261029T133000Z\r\nSUMMARY:Doctor\r\n{lines}END:VEVENT\r\nEND:VCALENDAR\r\n")).unwrap();
            path
        }

        fn tasks(&self) -> Vec<Task> {
            self.list.items().into_iter().filter_map(|p| tasks::read(&p, &self.list, &TimeZone::system())).collect()
        }

        fn refs(&self) -> Vec<EventRef> {
            let zone = TimeZone::system();
            self.calendar.items().into_iter().filter_map(|p| agenda::event_ref(&std::fs::read_to_string(&p).unwrap(), &p.display().to_string(), &zone)).collect()
        }
    }

    fn file<'a>(path: &'a Path, text: &'a str, writable: bool) -> EventFile<'a> {
        EventFile { path, text, writable }
    }

    #[test]
    fn a_task_paired_with_its_appointment_both_ways() {
        let desk = Desk::new("pair");
        desk.task("doctor", TaskEdit::default());
        desk.task("doctor-step", TaskEdit { parent: "doctor".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        let text = std::fs::read_to_string(&path).unwrap();
        // A task with steps is welcome; its date becomes the appointment's day.
        attach(&desk.tasks(), "doctor", &file(&path, &text, true), &desk.refs(), &now()).unwrap().write().unwrap();
        let task = desk.tasks().into_iter().find(|t| t.uid == "doctor").unwrap();
        assert_eq!((appointment_of(&task).as_deref(), task.due.as_str()), (Some("visit"), "2026-10-29"));
        let event = std::fs::read_to_string(&path).unwrap();
        assert!(event.contains("rel/preparation") && event.contains("sioul:task/doctor") && event.contains("DTSTAMP:20261020T100000Z"), "the event's link, its stamps as they were: {event}");
        assert_eq!(preparation_of(&desk.tasks(), &desk.refs()[0]).map(|t| t.uid.as_str()), Some("doctor"));
        // Another task for the same appointment: refused; a time block: refused.
        desk.task("other", TaskEdit::default());
        assert_eq!(attach(&desk.tasks(), "other", &file(&path, &event, true), &desk.refs(), &now()), Err(Refused("pair-taken")));
        let block = desk.event("block", "X-SIOUL-TASK:other\r\n");
        let block_text = std::fs::read_to_string(&block).unwrap();
        assert_eq!(attach(&desk.tasks(), "other", &file(&block, &block_text, true), &desk.refs(), &now()), Err(Refused("pair-block")));
        // The appointment moves: the date follows while it says the day written; not once you set yours.
        let moved = Appointment { day: "2026-11-03".parse().unwrap(), cancelled: false, recurring: false };
        let task = desk.tasks().into_iter().find(|t| t.uid == "doctor").unwrap();
        let text = std::fs::read_to_string(&task.key).unwrap();
        let followed_text = followed(&task, &text, &moved, &now()).unwrap();
        vdir::write_item(Path::new(&task.key), &followed_text).unwrap();
        let task = desk.tasks().into_iter().find(|t| t.uid == "doctor").unwrap();
        assert_eq!(task.due, "2026-11-03");
        assert!(followed(&task, &followed_text, &moved, &now()).is_none(), "the same twice");
        let mine = tasks::apply(&followed_text, &TaskEdit { due: "2026-11-01".into(), ..TaskEdit::of(&task) }, &TimeZone::system(), &now()).unwrap();
        let task_mine = tasks::task_of_text(&mine, &TimeZone::system()).map(|t| Task { key: task.key.clone(), ..t }).unwrap();
        assert!(followed(&task_mine, &mine, &Appointment { day: "2026-11-10".parse().unwrap(), ..moved }, &now()).is_none(), "your date stays");
        // Detached: both links out, the date the pairing wrote with them.
        let event_text = std::fs::read_to_string(&path).unwrap();
        detach(&task, Some(&file(&path, &event_text, true)), &now()).unwrap().write().unwrap();
        let task = desk.tasks().into_iter().find(|t| t.uid == "doctor").unwrap();
        assert_eq!((appointment_of(&task), task.due.as_str()), (None, ""));
        assert!(!std::fs::read_to_string(&path).unwrap().contains("rel/preparation"));
        assert_eq!(detach(&task, None, &now()), Err(Refused("pair-none")));
    }

    #[test]
    fn an_invitation_keeps_no_line_and_a_date_set_stays() {
        let desk = Desk::new("invited");
        desk.task("prepare", TaskEdit { due: "2026-10-27".into(), ..TaskEdit::default() });
        let path = desk.event("board", "ORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:jane@example.org\r\n");
        let text = std::fs::read_to_string(&path).unwrap();
        let change = attach(&desk.tasks(), "prepare", &file(&path, &text, true), &desk.refs(), &now()).unwrap();
        assert_eq!(change.written.len(), 1, "the task alone: its guests would hear of the event's line");
        change.write().unwrap();
        let task = desk.tasks().into_iter().find(|t| t.uid == "prepare").unwrap();
        assert_eq!(task.due, "2026-10-27", "an earlier date of its own stays");
        assert_eq!(preparation_of(&desk.tasks(), &desk.refs()[0]).map(|t| t.uid.as_str()), Some("prepare"), "found from the task's end");
        // Detached, a date of its own stays.
        detach(&task, Some(&file(&path, &text, true)), &now()).unwrap().write().unwrap();
        assert_eq!(desk.tasks()[0].due, "2026-10-27");
    }

    #[test]
    fn an_event_made_for_a_task_with_steps_keeps_the_task() {
        let desk = Desk::new("keep");
        desk.task("vet", TaskEdit::default());
        desk.task("vet-step", TaskEdit { parent: "vet".into(), ..TaskEdit::default() });
        let edit = agenda::EventEdit { title: "Vet".into(), start: "2026-10-30T10:00".into(), end: "2026-10-30T10:30".into(), ..agenda::EventEdit::default() };
        let (change, path) = appointment_for(&desk.tasks(), "vet", &edit, &desk.calendar, &now()).unwrap();
        change.write().unwrap();
        let event = std::fs::read_to_string(&path).unwrap();
        assert!(event.contains("sioul:task/vet"), "{event}");
        let task = desk.tasks().into_iter().find(|t| t.uid == "vet").unwrap();
        assert_eq!(appointment_of(&task), Some(crate::appointments::uid_in(&path)));
        assert_eq!(desk.tasks().len(), 2, "the task and its step stay");
        // A new task for an event: titled as it, paired.
        let other = desk.event("dentist", "");
        let other_text = std::fs::read_to_string(&other).unwrap();
        let (change, uid) = new_for(&desk.tasks(), &file(&other, &other_text, true), &desk.refs(), &desk.list, &now()).unwrap();
        change.write().unwrap();
        let made = desk.tasks().into_iter().find(|t| t.uid == uid).unwrap();
        assert_eq!((made.title.as_str(), appointment_of(&made).as_deref(), made.due.as_str()), ("Doctor", Some("dentist"), "2026-10-29"));
    }
}
