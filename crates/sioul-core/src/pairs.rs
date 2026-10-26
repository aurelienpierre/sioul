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
//! enough, and the pair is found from either end (`preparation_of`). Untied
//! while such an event still names it, the task keeps a mark
//! (`X-SIOUL-UNTIED:<the event's UID>`), so that the event's stale line never
//! ties it back. A task of a Google Tasks list is never tied: Google keeps
//! no link, and the tie would be one device's.
//!
//! - **Its date**: the task's DUE becomes the appointment's day when it has
//!   none or a later one. The day written is kept on its link
//!   (`X-SIOUL-DUE`), and the date it replaced (`X-SIOUL-DUE-WAS`): the DUE
//!   follows the appointment while it says that day exactly (a bare date: a
//!   time you gave it is yours), and a detach puts back the date it
//!   replaced. Tied again, the same day stays Sioul's own. It follows only
//!   an appointment this device saw move (`Seen`): a device that has the
//!   task's new date before the moved event never writes the old one back.
//!   The day is the appointment's in this device's time zone.
//! - **Its steps** are free on the appointment's day, not before, unless one
//!   has a day of its own, to start or asked (`plan`).
//! - **The appointment gone** (deleted, cancelled, or not here yet): the
//!   task and its steps wait for a date, offered nowhere, until another is
//!   attached (`plan`, `Planned::parked`). An appointment of an account
//!   switched off is still here. One whose start cannot be read is here,
//!   without a day.
//! - **Never** a time block, nor an appointment of a repeating event's date:
//!   a repeating appointment keeps the task's date as it is.

use crate::agenda::{self, EventRef};
use crate::composed::Change;
use crate::links;
use crate::tasks::{self, Status, Task, TaskEdit};
use jiff::Zoned;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The relation of the task's link to its appointment.
pub const APPOINTMENT: &str = "tag:aurelienpierre.com,2026:sioul/rel/appointment";
/// The relation of the event's link to the task that prepares it.
pub const PREPARATION: &str = "tag:aurelienpierre.com,2026:sioul/rel/preparation";
/// The day the pairing wrote as the task's DUE, on its link: what follows the appointment.
pub const DUE_WRITTEN: &str = "X-SIOUL-DUE";
/// The task's own DUE the pairing replaced, as the task wrote it ("2026-11-30"), on its link.
pub const DUE_WAS: &str = "X-SIOUL-DUE-WAS";
/// In a task untied from an event that still names it: that event's UID.
pub const UNTIED: &str = "X-SIOUL-UNTIED";

/// Why a pairing is refused, as a word of the translations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub &'static str);

impl Refused {
    pub fn said(&self, tr: &crate::i18n::Translator) -> String {
        tr.text(self.0, None)
    }
}

/// What the plan knows of an appointment: its day (None when its start
/// cannot be read), whether it was cancelled, whether it comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appointment {
    pub day: Option<Date>,
    pub cancelled: bool,
    pub recurring: bool,
}

/// Every event, by its UID, as the plan knows appointments (`plan::Settings::appointments`).
pub fn appointments_of(events: &[EventRef], zone: &TimeZone) -> BTreeMap<String, Appointment> {
    events
        .iter()
        .map(|e| {
            let day = (e.start > 0).then(|| jiff::Timestamp::from_second(e.start).ok()).flatten().map(|t| t.to_zoned(zone.clone()).date());
            (e.uid.clone(), Appointment { day, cancelled: e.cancelled, recurring: e.recurring })
        })
        .collect()
}

/// The appointments there are: `events` (those the pages show) and those of
/// accounts switched off, which are still here and park nothing.
pub fn appointments_here(events: &[EventRef], zone: &TimeZone) -> BTreeMap<String, Appointment> {
    let mut all = appointments_of(events, zone);
    for dir in crate::vdir::switched_off_dirs(crate::vdir::Kind::Calendars) {
        let Ok(items) = std::fs::read_dir(&dir) else { continue };
        let refs: Vec<EventRef> = items
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "ics"))
            .filter_map(|p| std::fs::read_to_string(&p).ok().and_then(|t| agenda::event_ref(&t, &p.display().to_string(), zone)))
            .collect();
        for (uid, appointment) in appointments_of(&refs, zone) {
            all.entry(uid).or_insert(appointment);
        }
    }
    all
}

/// Whether a task list keeps a task's links, by its "account/id" (Google
/// Tasks does not, `capabilities`): what `attach` asks.
pub fn links_kept(config: &crate::config::Config) -> impl Fn(&str) -> bool + '_ {
    let lists = tasks::lists();
    move |id: &str| {
        lists.iter().find(|c| format!("{}/{}", c.account, c.id) == id).is_none_or(|c| crate::capabilities::provider_of_collection(config.account(&c.account), c) != crate::capabilities::Provider::GoogleTasks)
    }
}

/// The UID of the appointment a task prepares.
pub fn appointment_of(task: &Task) -> Option<String> {
    task.links.iter().find(|l| l.rel == APPOINTMENT && l.uri.starts_with("sioul:event/")).map(|l| links::id_of(&l.uri))
}

/// Whether a LINK line is a pair's.
fn pair_line(line: &str, rel: &str) -> bool {
    crate::lines::name(line) == "LINK" && tasks::link_of(line).rel == rel
}

/// Whether a link is one end of a pair: not to copy when a thing becomes another.
pub fn is_pair_link(link: &tasks::Link) -> bool {
    link.rel == APPOINTMENT || link.rel == PREPARATION
}

/// The task that prepares the event: among the open tasks whose link names
/// it, the one the event's own line names, else the oldest; else a done one
/// whose link names it; else the task the event's line names, when that
/// task names no other appointment and was not untied from it.
pub fn preparation_of<'a>(tasks: &'a [Task], event: &EventRef) -> Option<&'a Task> {
    let lined = event.links.iter().find(|l| l.rel == PREPARATION && l.uri.starts_with("sioul:task/")).map(|l| links::id_of(&l.uri));
    let named = |t: &&Task| appointment_of(t).as_deref() == Some(event.uid.as_str());
    let mut open: Vec<&Task> = tasks.iter().filter(named).filter(|t| t.status.is_open()).collect();
    open.sort_by(|a, b| (lined.as_deref() != Some(a.uid.as_str()), a.created, &a.uid).cmp(&(lined.as_deref() != Some(b.uid.as_str()), b.created, &b.uid)));
    open.first().copied().or_else(|| tasks.iter().find(named)).or_else(|| {
        let uid = lined?;
        tasks.iter().find(|t| t.uid == uid && !t.untied.contains(&event.uid) && appointment_of(t).is_none_or(|other| other == event.uid))
    })
}

/// What a task's link to its appointment says: the event's UID, the day the
/// pairing wrote as DUE, the task's own date it replaced.
struct Tie {
    written: Option<Date>,
    was: Option<String>,
}

fn tie_of(text: &str) -> Option<Tie> {
    let source = crate::lines::unfold(text);
    let line = source.iter().find(|l| pair_line(l, APPOINTMENT))?;
    let written = crate::lines::param(line, DUE_WRITTEN).and_then(|d| Date::strptime("%Y%m%d", d.trim()).ok());
    let was = crate::lines::param(line, DUE_WAS).map(|d| d.trim().to_string()).filter(|d| d.get(..10).is_some_and(|day| day.parse::<Date>().is_ok()));
    Some(Tie { written, was })
}

/// Whether the task's DUE is the day the pairing wrote, exactly: a bare
/// date, the same. A time given to it, or another day, is yours.
fn sioul_set(task: &Task, tie: Option<&Tie>) -> bool {
    tie.and_then(|t| t.written).is_some_and(|w| task.due == w.to_string())
}

/// The task's own date: the one Sioul's replaced, else its DUE when Sioul did not write it.
fn own_due(task: &Task, tie: Option<&Tie>) -> String {
    if sioul_set(task, tie) { tie.and_then(|t| t.was.clone()).unwrap_or_default() } else { task.due.clone() }
}

/// The task's link to its appointment, with the day it wrote as DUE and the date it replaced.
fn appointment_line(event_uid: &str, due: Option<Date>, was: Option<&str>) -> String {
    let due = due.map(|d| format!(";{DUE_WRITTEN}={}", d.strftime("%Y%m%d"))).unwrap_or_default();
    let was = was.filter(|w| !w.is_empty()).map(|w| format!(";{DUE_WAS}={}", crate::lines::param_value(w))).unwrap_or_default();
    format!("LINK;LINKREL=\"{APPOINTMENT}\"{due}{was};VALUE=URI:{}", links::event_uri(event_uid))
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
    pub fn takes_lines(&self) -> bool {
        self.writable && !agenda::with_others(self.text)
    }
}

/// The task `uid` paired with an event (the module's rules): the task's link
/// and, when it can be written, the event's; the task's DUE the appointment's
/// day when it has none or a later one (never for a repeating appointment).
/// Refused for a task you only read or done, one whose list keeps no link
/// (`keeps_links`, by its list's "account/id": Google Tasks), an event that
/// is a time block or another appointment's leg, an event another open task
/// prepares already.
pub fn attach(all: &[Task], uid: &str, event: &EventFile, refs: &[EventRef], keeps_links: &dyn Fn(&str) -> bool, now: &Zoned) -> Result<Change, Refused> {
    let task = all.iter().find(|t| t.uid == uid).ok_or(Refused("task-gone"))?;
    let current = std::fs::read_to_string(&task.key).map_err(|_| Refused("task-gone"))?;
    let (text, event_text) = paired(all, task, &current, event, refs, keeps_links, now)?;
    let mut change = Change { written: vec![(PathBuf::from(&task.key), text)], ..Change::default() };
    change.written.extend(event_text.map(|t| (event.path.to_path_buf(), t)));
    Ok(change)
}

/// The task's text paired with the event, and the event's when it changes (`attach`).
fn paired(all: &[Task], task: &Task, current: &str, event: &EventFile, refs: &[EventRef], keeps_links: &dyn Fn(&str) -> bool, now: &Zoned) -> Result<(String, Option<String>), Refused> {
    let zone = TimeZone::system();
    if task.read_only {
        return Err(Refused("task-read-only"));
    }
    if !task.status.is_open() {
        return Err(Refused("composed-done"));
    }
    if !keeps_links(&task.list_id) {
        return Err(Refused("pair-google"));
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
    let day = (known.start > 0 && !known.recurring).then(|| jiff::Timestamp::from_second(known.start).ok()).flatten().map(|t| t.to_zoned(zone.clone()).date());
    // Its date: the appointment's day, when its own date is none or later; a
    // date Sioul wrote for another tie (or this one, tied again) is Sioul's own.
    let tie = tie_of(current);
    let own = own_due(task, tie.as_ref());
    let own_day = own.get(..10).and_then(|d| d.parse::<Date>().ok());
    let mut edit = TaskEdit::of(task);
    let (written, was) = match day {
        Some(day) if own_day.is_none_or(|o| o > day) => {
            edit.due = day.to_string();
            (Some(day), Some(own))
        }
        _ => {
            edit.due = own;
            (None, None)
        }
    };
    let text = tasks::apply(current, &edit, &zone, now).map_err(|_| Refused("task-gone"))?;
    let mark = |l: &str| crate::lines::name(l) == UNTIED && crate::lines::value(l).trim() == reference.uid;
    let text = tasks::replace_lines(&text, |l| pair_line(l, APPOINTMENT) || mark(l), vec![appointment_line(&reference.uid, written, was.as_deref())], now).map_err(|_| Refused("task-gone"))?;
    let event_text = if event.takes_lines() {
        let without = agenda::remove_lines(event.text, |l| pair_line(l, PREPARATION)).ok_or(Refused("agenda-gone"))?;
        let with = agenda::add_lines(&without, &[preparation_line(&task.uid)]).ok_or(Refused("agenda-gone"))?;
        (with != event.text).then_some(with)
    } else {
        None
    };
    Ok((text, event_text))
}

/// The pair undone: the task's link out, its DUE back to the date it
/// replaced when it still says the day the pairing wrote; the event's line
/// out when it holds one and takes lines (`EventFile::takes_lines`: never a
/// meeting with others). An event that keeps its line, or is not here
/// (`event` None), leaves the task a mark (`UNTIED`) so that it never ties it
/// back. The legs stay with the event.
pub fn detach(task: &Task, event: Option<&EventFile>, now: &Zoned) -> Result<Change, Refused> {
    if task.read_only {
        return Err(Refused("task-read-only"));
    }
    let zone = TimeZone::system();
    let current = std::fs::read_to_string(&task.key).map_err(|_| Refused("task-gone"))?;
    let Some(event_uid) = appointment_of(task) else { return Err(Refused("pair-none")) };
    let tie = tie_of(&current);
    let mut text = current.clone();
    if sioul_set(task, tie.as_ref()) {
        text = tasks::apply(&text, &TaskEdit { due: own_due(task, tie.as_ref()), ..TaskEdit::of(task) }, &zone, now).map_err(|_| Refused("task-gone"))?;
    }
    let names_it = |l: &str| pair_line(l, PREPARATION) && tasks::link_of(l).uri == links::task_uri(&task.uid);
    let mut change = Change::default();
    let mut cleared = false;
    if let Some(event) = event.filter(|e| e.takes_lines()) {
        cleared = true;
        if crate::lines::unfold(event.text).iter().any(|l| names_it(l)) {
            let without = agenda::remove_lines(event.text, names_it).ok_or(Refused("agenda-gone"))?;
            change.written.push((event.path.to_path_buf(), without));
        }
    }
    // The event's line stays where it cannot be written: the mark keeps it from tying the task back.
    let marked = if cleared { Vec::new() } else { vec![format!("{UNTIED}:{event_uid}")] };
    let text = tasks::replace_lines(&text, |l| pair_line(l, APPOINTMENT), marked, now).map_err(|_| Refused("task-gone"))?;
    change.written.insert(0, (PathBuf::from(&task.key), text));
    Ok(change)
}

/// The task's text with its DUE moved to its appointment's day, when the
/// pairing wrote the DUE it has and the appointment's day differs: None when
/// nothing is to change (its DUE yours since, a repeating or undated
/// appointment, the same day). A date of its own the pairing replaced comes
/// back when it is earlier than the new day. `upkeep` asks it only of an
/// appointment this device saw move.
pub fn followed(task: &Task, text: &str, appointment: &Appointment, now: &Zoned) -> Option<String> {
    if appointment.recurring || appointment.cancelled || task.read_only || !task.status.is_open() {
        return None;
    }
    let day = appointment.day?;
    let tie = tie_of(text)?;
    if !sioul_set(task, Some(&tie)) || tie.written == Some(day) {
        return None;
    }
    let zone = TimeZone::system();
    let uid = appointment_of(task)?;
    let was = tie.was.clone().unwrap_or_default();
    let (due, written, keep) = match was.get(..10).and_then(|d| d.parse::<Date>().ok()) {
        Some(own) if own <= day => (was.clone(), None, None),
        _ => (day.to_string(), Some(day), tie.was.as_deref()),
    };
    let moved = tasks::apply(text, &TaskEdit { due, ..TaskEdit::of(task) }, &zone, now).ok()?;
    tasks::replace_lines(&moved, |l| pair_line(l, APPOINTMENT), vec![appointment_line(&uid, written, keep)], now).ok()
}

/// The day each appointment had when this device last saw it, by UID: kept
/// here alone (`$XDG_STATE_HOME/sioul/pairs-seen.toml`), so that a date
/// follows only an appointment this device saw move.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    #[serde(default)]
    pub days: BTreeMap<String, Date>,
}

impl Seen {
    pub fn path() -> PathBuf {
        crate::config::state_dir().join("pairs-seen.toml")
    }

    pub fn load() -> Seen {
        std::fs::read_to_string(Seen::path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Seen::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        crate::vdir::write_item(&path, &toml::to_string(self).map_err(|e| e.to_string())?)
    }
}

/// Each paired task's DUE following its appointment (`followed`), for the
/// appointments this device saw move since it last looked (`seen`, updated
/// for every tied appointment; one seen for the first time is only noted):
/// written, and the files written that way returned. A file that could not be
/// written is not returned, so that nothing is tried again at once.
pub fn upkeep(all: &[Task], appointments: &BTreeMap<String, Appointment>, seen: &mut Seen, now: &Zoned) -> Vec<PathBuf> {
    let tied: BTreeSet<String> = all.iter().filter(|t| t.status.is_open()).filter_map(appointment_of).collect();
    let mut moved: BTreeSet<String> = BTreeSet::new();
    for uid in &tied {
        let Some(day) = appointments.get(uid).and_then(|a| a.day) else { continue };
        if seen.days.insert(uid.clone(), day).is_some_and(|before| before != day) {
            moved.insert(uid.clone());
        }
    }
    seen.days.retain(|uid, _| tied.contains(uid));
    let mut written = Vec::new();
    for task in all {
        let Some(uid) = appointment_of(task).filter(|u| moved.contains(u)) else { continue };
        let Some(appointment) = appointments.get(&uid) else { continue };
        let Ok(text) = std::fs::read_to_string(&task.key) else { continue };
        if let Some(followed) = followed(task, &text, appointment, now)
            && crate::vdir::write_item(Path::new(&task.key), &followed).is_ok()
        {
            written.push(PathBuf::from(&task.key));
        }
    }
    written
}

/// A task to prepare the event (its steps and its list): titled as the
/// event, paired with it, in `list`. Its UID with the change.
pub fn new_for(all: &[Task], event: &EventFile, refs: &[EventRef], list: &crate::vdir::Collection, keeps_links: &dyn Fn(&str) -> bool, now: &Zoned) -> Result<(Change, String), Refused> {
    let zone = TimeZone::system();
    let edit = agenda::edit_of_text(event.text, &zone).ok_or(Refused("agenda-gone"))?;
    let uid = crate::vdir::new_name();
    let text = tasks::new_task(&TaskEdit { title: edit.title.clone(), status: Status::NeedsAction, ..TaskEdit::default() }, &uid, &zone, now).map_err(|_| Refused("task-gone"))?;
    let path = list.dir.join(format!("{uid}.ics"));
    let task = Task { key: path.display().to_string(), list_id: format!("{}/{}", list.account, list.id), ..tasks::task_of_text(&text, &zone).ok_or(Refused("task-gone"))? };
    let (text, event_text) = paired(all, &task, &text, event, refs, keeps_links, now)?;
    let mut change = Change { made: vec![(path, text)], ..Change::default() };
    change.written.extend(event_text.map(|t| (event.path.to_path_buf(), t)));
    Ok((change, uid))
}

/// An event made from a task that has steps (`appointments::task_to_event`
/// refuses to take such a task out): the event `edit` in `calendar`, paired
/// with the task, which stays with its steps. The event is made first
/// (`Change::write` writes what is new first): cut short, the task is never
/// left tied to nothing. The event's file with the change.
pub fn appointment_for(all: &[Task], uid: &str, edit: &agenda::EventEdit, calendar: &crate::vdir::Collection, keeps_links: &dyn Fn(&str) -> bool, now: &Zoned) -> Result<(Change, PathBuf), Refused> {
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
    let (task_text, event_text) = paired(all, task, &current, &event, std::slice::from_ref(&reference), keeps_links, now)?;
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

    /// Every list keeps links.
    fn any(_: &str) -> bool {
        true
    }

    fn task_of(desk: &Desk, uid: &str) -> Task {
        desk.tasks().into_iter().find(|t| t.uid == uid).unwrap()
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn day(text: &str) -> Option<Date> {
        Some(text.parse().unwrap())
    }

    fn at(on: &str) -> Appointment {
        Appointment { day: day(on), cancelled: false, recurring: false }
    }

    /// F1: untied while the event has guests: the event is never written, and
    /// its stale line never ties the task back.
    #[test]
    fn untied_from_a_meeting_with_others_writes_the_task_alone() {
        let desk = Desk::new("f1");
        desk.task("prep", TaskEdit::default());
        let path = desk.event("meet", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        // Guests added since, its line kept by the editor.
        let with_guests = read(&path).replace("SUMMARY:Doctor", "SUMMARY:Doctor\r\nORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:jane@example.org");
        std::fs::write(&path, &with_guests).unwrap();
        let change = detach(&task_of(&desk, "prep"), Some(&file(&path, &with_guests, true)), &now()).unwrap();
        assert!(change.written.iter().all(|(p, _)| *p != path), "the meeting is never written");
        change.write().unwrap();
        assert_eq!(read(&path), with_guests);
        let task = task_of(&desk, "prep");
        assert_eq!((appointment_of(&task), task.untied.clone()), (None, vec!["meet".to_string()]));
        assert!(preparation_of(&desk.tasks(), &desk.refs()[0]).is_none(), "the stale line ties nothing back");
        // Tied again: the mark goes.
        attach(&desk.tasks(), "prep", &file(&path, &with_guests, true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        assert!(task_of(&desk, "prep").untied.is_empty());
        assert_eq!(preparation_of(&desk.tasks(), &desk.refs()[0]).map(|t| t.uid.as_str()), Some("prep"));
    }

    /// F3: tied twice, the written day stays, and the date still follows.
    #[test]
    fn tied_twice_the_date_still_follows() {
        let desk = Desk::new("f3");
        desk.task("prep", TaskEdit::default());
        let path = desk.event("visit", "");
        for _ in 0..2 {
            attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        }
        let task = task_of(&desk, "prep");
        assert!(crate::lines::unfold(&read(Path::new(&task.key))).join("\n").contains("X-SIOUL-DUE=20261029"), "{}", read(Path::new(&task.key)));
        assert!(followed(&task, &read(Path::new(&task.key)), &at("2026-11-03"), &now()).is_some());
    }

    /// F5: tied again after the appointment went, Sioul's old date is replaced;
    /// a later date of its own comes back when untied; F8: a time you gave it is yours.
    #[test]
    fn dates_of_its_own_come_back_and_old_ones_are_replaced() {
        let desk = Desk::new("f5");
        desk.task("prep", TaskEdit::default());
        let old = desk.event("old", "");
        attach(&desk.tasks(), "prep", &file(&old, &read(&old), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        std::fs::remove_file(&old).unwrap();
        let new = desk.calendar.dir.join("new.ics");
        std::fs::write(&new, read_event("new", "20261103T130000Z")).unwrap();
        attach(&desk.tasks(), "prep", &file(&new, &read(&new), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let task = task_of(&desk, "prep");
        assert_eq!(task.due, "2026-11-03", "the old day was Sioul's own");
        assert!(followed(&task, &read(Path::new(&task.key)), &at("2026-11-05"), &now()).is_some(), "and the new one follows");
        // A later date of its own: replaced while tied, back when untied.
        desk.task("later", TaskEdit { due: "2026-11-30".into(), ..TaskEdit::default() });
        let visit = desk.event("visit-later", "");
        attach(&desk.tasks(), "later", &file(&visit, &read(&visit), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        assert_eq!(task_of(&desk, "later").due, "2026-10-29");
        detach(&task_of(&desk, "later"), Some(&file(&visit, &read(&visit), true)), &now()).unwrap().write().unwrap();
        assert_eq!(task_of(&desk, "later").due, "2026-11-30");
        // Moved past its own date: its own date, earlier, comes back.
        attach(&desk.tasks(), "later", &file(&visit, &read(&visit), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let task = task_of(&desk, "later");
        let moved = followed(&task, &read(Path::new(&task.key)), &at("2026-12-05"), &now()).unwrap();
        assert_eq!(tasks::task_of_text(&moved, &TimeZone::system()).unwrap().due, "2026-11-30");
        // A time given to the date: yours, never moved.
        let task = task_of(&desk, "prep");
        let timed = tasks::apply(&read(Path::new(&task.key)), &TaskEdit { due: "2026-11-03T09:00".into(), ..TaskEdit::of(&task) }, &TimeZone::system(), &now()).unwrap();
        let task = Task { key: task.key.clone(), ..tasks::task_of_text(&timed, &TimeZone::system()).unwrap() };
        assert!(followed(&task, &timed, &at("2026-11-07"), &now()).is_none());
    }

    fn read_event(uid: &str, start: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261020T100000Z\r\nDTSTART:{start}\r\nSUMMARY:Doctor\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")
    }

    /// F4: the date follows only an appointment this device saw move: a device
    /// that has the task's new date before the moved event never writes the
    /// old one back. F12: a file that cannot be written is not returned.
    #[test]
    fn a_date_follows_only_a_move_seen_here() {
        let desk = Desk::new("f4");
        desk.task("prep", TaskEdit::default());
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let mut seen = Seen::default();
        let here = |on: &str| BTreeMap::from([("visit".to_string(), at(on))]);
        // First seen: noted, nothing written.
        assert!(upkeep(&desk.tasks(), &here("2026-10-29"), &mut seen, &now()).is_empty());
        // Another device moved it to 3 November and wrote the task: this device has the task first.
        let task = task_of(&desk, "prep");
        let theirs = followed(&task, &read(Path::new(&task.key)), &at("2026-11-03"), &now()).unwrap();
        std::fs::write(&task.key, &theirs).unwrap();
        assert!(upkeep(&desk.tasks(), &here("2026-10-29"), &mut seen, &now()).is_empty(), "the old day never written back");
        assert_eq!(task_of(&desk, "prep").due, "2026-11-03");
        // The moved event comes: nothing more to write. Moved here again: written once.
        assert!(upkeep(&desk.tasks(), &here("2026-11-03"), &mut seen, &now()).is_empty());
        assert_eq!(upkeep(&desk.tasks(), &here("2026-11-10"), &mut seen, &now()).len(), 1);
        assert_eq!(task_of(&desk, "prep").due, "2026-11-10");
        assert!(upkeep(&desk.tasks(), &here("2026-11-10"), &mut seen, &now()).is_empty(), "the same twice");
        // F12: its folder read-only, the write fails: nothing returned, nothing tried again at once.
        use std::os::unix::fs::PermissionsExt;
        let dir = desk.list.dir.clone();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let written = upkeep(&desk.tasks(), &here("2026-11-12"), &mut seen, &now());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(written.is_empty(), "{written:?}");
    }

    /// F7: an event whose start cannot be read has no day: nothing written.
    #[test]
    fn an_unreadable_start_writes_no_date() {
        let undated = EventRef { uid: "x".into(), start: 0, ..EventRef::default() };
        assert_eq!(appointments_of(&[undated], &TimeZone::UTC)["x"].day, None);
        let desk = Desk::new("f7");
        desk.task("prep", TaskEdit::default());
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let task = task_of(&desk, "prep");
        assert!(followed(&task, &read(Path::new(&task.key)), &Appointment { day: None, cancelled: false, recurring: false }, &now()).is_none());
    }

    /// F10: a task of a Google Tasks list is never tied: Google keeps no link.
    #[test]
    fn a_google_task_is_never_tied() {
        let desk = Desk::new("f10");
        desk.task("prep", TaskEdit::default());
        let path = desk.event("visit", "");
        let google = |_: &str| false;
        assert_eq!(attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &google, &now()), Err(Refused("pair-google")));
    }

    /// F11: two tasks tied to one event on two devices: the one the event's line names.
    #[test]
    fn two_tasks_for_one_event_the_event_s_line_decides() {
        let desk = Desk::new("f11");
        desk.task("aaa", TaskEdit::default());
        desk.task("zzz", TaskEdit::default());
        let path = desk.event("visit", "");
        for uid in ["aaa", "zzz"] {
            // Each device tied its own, offline: written one after the other, the test of the other skipped.
            let alone: Vec<Task> = desk.tasks().into_iter().filter(|t| t.uid == uid).collect();
            attach(&alone, uid, &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        }
        assert_eq!(preparation_of(&desk.tasks(), &desk.refs()[0]).map(|t| t.uid.as_str()), Some("zzz"));
    }

    /// F14: the event made first: cut short (its calendar cannot be written), the task is untouched.
    #[test]
    fn an_event_made_for_a_task_is_made_first() {
        use std::os::unix::fs::PermissionsExt;
        let desk = Desk::new("f14");
        desk.task("vet", TaskEdit::default());
        let before = read(&desk.list.dir.join("vet.ics"));
        let edit = agenda::EventEdit { title: "Vet".into(), start: "2026-10-30T10:00".into(), end: "2026-10-30T10:30".into(), ..agenda::EventEdit::default() };
        let (change, _) = appointment_for(&desk.tasks(), "vet", &edit, &desk.calendar, &any, &now()).unwrap();
        std::fs::set_permissions(&desk.calendar.dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let written = change.write();
        std::fs::set_permissions(&desk.calendar.dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(written.is_err());
        assert_eq!(read(&desk.list.dir.join("vet.ics")), before, "the task never tied to nothing");
    }

    /// F13: an appointment made a task: the tasks that prepared it keep no tie to a task.
    #[test]
    fn a_conversion_keeps_no_pair_link() {
        let task = "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:prep\r\nLINK;LINKREL=\"tag:aurelienpierre.com,2026:sioul/rel/appointment\";X-SIOUL-DUE=20261029;VALUE=URI:sioul:event/visit\r\nLINK;LINKREL=related;VALUE=UID:visit\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let text = crate::appointments::retarget(task, "visit", "new-task", false, &now()).unwrap();
        let read = tasks::task_of_text(&text, &TimeZone::UTC).unwrap();
        assert_eq!(appointment_of(&read), None);
        assert!(!text.contains("rel/appointment") && text.contains("VALUE=UID:new-task"), "{text}");
    }

    #[test]
    fn a_task_paired_with_its_appointment_both_ways() {
        let desk = Desk::new("pair");
        desk.task("doctor", TaskEdit::default());
        desk.task("doctor-step", TaskEdit { parent: "doctor".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        let text = std::fs::read_to_string(&path).unwrap();
        // A task with steps is welcome; its date becomes the appointment's day.
        attach(&desk.tasks(), "doctor", &file(&path, &text, true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let task = desk.tasks().into_iter().find(|t| t.uid == "doctor").unwrap();
        assert_eq!((appointment_of(&task).as_deref(), task.due.as_str()), (Some("visit"), "2026-10-29"));
        let event = std::fs::read_to_string(&path).unwrap();
        assert!(event.contains("rel/preparation") && event.contains("sioul:task/doctor") && event.contains("DTSTAMP:20261020T100000Z"), "the event's link, its stamps as they were: {event}");
        assert_eq!(preparation_of(&desk.tasks(), &desk.refs()[0]).map(|t| t.uid.as_str()), Some("doctor"));
        // Another task for the same appointment: refused; a time block: refused.
        desk.task("other", TaskEdit::default());
        assert_eq!(attach(&desk.tasks(), "other", &file(&path, &event, true), &desk.refs(), &any, &now()), Err(Refused("pair-taken")));
        let block = desk.event("block", "X-SIOUL-TASK:other\r\n");
        let block_text = std::fs::read_to_string(&block).unwrap();
        assert_eq!(attach(&desk.tasks(), "other", &file(&block, &block_text, true), &desk.refs(), &any, &now()), Err(Refused("pair-block")));
        // The appointment moves: the date follows while it says the day written; not once you set yours.
        let moved = Appointment { day: Some("2026-11-03".parse().unwrap()), cancelled: false, recurring: false };
        let task = desk.tasks().into_iter().find(|t| t.uid == "doctor").unwrap();
        let text = std::fs::read_to_string(&task.key).unwrap();
        let followed_text = followed(&task, &text, &moved, &now()).unwrap();
        vdir::write_item(Path::new(&task.key), &followed_text).unwrap();
        let task = desk.tasks().into_iter().find(|t| t.uid == "doctor").unwrap();
        assert_eq!(task.due, "2026-11-03");
        assert!(followed(&task, &followed_text, &moved, &now()).is_none(), "the same twice");
        let mine = tasks::apply(&followed_text, &TaskEdit { due: "2026-11-01".into(), ..TaskEdit::of(&task) }, &TimeZone::system(), &now()).unwrap();
        let task_mine = tasks::task_of_text(&mine, &TimeZone::system()).map(|t| Task { key: task.key.clone(), ..t }).unwrap();
        assert!(followed(&task_mine, &mine, &Appointment { day: Some("2026-11-10".parse().unwrap()), ..moved }, &now()).is_none(), "your date stays");
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
        let change = attach(&desk.tasks(), "prepare", &file(&path, &text, true), &desk.refs(), &any, &now()).unwrap();
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
        let (change, path) = appointment_for(&desk.tasks(), "vet", &edit, &desk.calendar, &any, &now()).unwrap();
        change.write().unwrap();
        let event = std::fs::read_to_string(&path).unwrap();
        assert!(event.contains("sioul:task/vet"), "{event}");
        let task = desk.tasks().into_iter().find(|t| t.uid == "vet").unwrap();
        assert_eq!(appointment_of(&task), Some(crate::appointments::uid_in(&path)));
        assert_eq!(desk.tasks().len(), 2, "the task and its step stay");
        // A new task for an event: titled as it, paired.
        let other = desk.event("dentist", "");
        let other_text = std::fs::read_to_string(&other).unwrap();
        let (change, uid) = new_for(&desk.tasks(), &file(&other, &other_text, true), &desk.refs(), &desk.list, &any, &now()).unwrap();
        change.write().unwrap();
        let made = desk.tasks().into_iter().find(|t| t.uid == uid).unwrap();
        assert_eq!((made.title.as_str(), appointment_of(&made).as_deref(), made.due.as_str()), ("Doctor", Some("dentist"), "2026-10-29"));
    }
}
