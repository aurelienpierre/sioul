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
//! - in the task: `X-SIOUL-LINK;LINKREL=related;X-SIOUL-REL="tag:aurelienpierre.com,2026:sioul/rel/appointment";VALUE=URI:sioul:event/<UID>`
//! - in the event: `X-SIOUL-LINK;LINKREL=related;X-SIOUL-REL="tag:aurelienpierre.com,2026:sioul/rel/preparation";VALUE=URI:sioul:task/<UID>`
//!
//! (RFC 9253's LINK under Sioul's own name, its relation in an X- parameter:
//! `tasks::link_line`; the form `LINK;LINKREL="tag:…"` written before 27
//! October 2026 reads the same.)
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
//!   time you gave it is yours), or the date of its own it was given back,
//!   always the earlier of the two; a detach puts back the date it replaced.
//!   Tied again to the same appointment, neither the date nor the instant
//!   changes. The link keeps the instant of the appointment it was set for
//!   (`X-SIOUL-AT`, its start in UTC), and the date follows by the one rule
//!   of what follows an appointment (`moves`): only a move this device saw,
//!   so that a device that has the task's new date before the moved event
//!   never writes the old one back. The day is the appointment's in the time
//!   zone of the moment asked (`now`). Never a DUE a
//!   server refuses (Sabre, so Murena: RFC 5545 §3.8.2.3): none before the
//!   day the task starts (that day then), none when its start has a time;
//!   the date it replaced comes back in the start's form (`put_back`). A
//!   date of its own the tie never replaced stays its own: nothing on the
//!   link, nothing follows.
//! - **Its steps** (`plan`): one with a start of its own goes by it.
//!   Otherwise the nearest date of its own up the chain decides (the step's,
//!   a bigger step's, the tied task's own, never the day the pairing wrote,
//!   `own_date`): before the appointment's day, the step is free now, to be
//!   done by then; on that day or after, or none, it waits for the day, as a
//!   follow-up cannot be done before the appointment.
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
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The relation of the task's link to its appointment.
pub const APPOINTMENT: &str = "tag:aurelienpierre.com,2026:sioul/rel/appointment";
/// The relation of the event's link to the task that prepares it.
pub const PREPARATION: &str = "tag:aurelienpierre.com,2026:sioul/rel/preparation";
/// The day the pairing wrote as the task's DUE, on its link: what follows the appointment.
pub const DUE_WRITTEN: &str = "X-SIOUL-DUE";
/// The task's own DUE the pairing replaced, as the task wrote it ("2026-11-30"), on its link.
/// A time kept as the task wrote it, in its zone: an older Sioul (982d6e5, 679f96a) reads
/// only that form, so a UTC instant here waits until every device reads it.
pub const DUE_WAS: &str = "X-SIOUL-DUE-WAS";
/// The appointment's start the tie was last set for, in UTC ("20261029T130000Z"), on its
/// link (`moves`): an older Sioul keeps the line as it is, and drops it when it writes the link.
pub const AT: &str = "X-SIOUL-AT";
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
/// cannot be read), whether it was cancelled, whether it comes back, and its
/// start (Unix seconds; 0 when it cannot be read).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Appointment {
    pub day: Option<Date>,
    pub cancelled: bool,
    pub recurring: bool,
    pub start: i64,
}

/// Every event, by its UID, as the plan knows appointments (`plan::Settings::appointments`).
pub fn appointments_of(events: &[EventRef], zone: &TimeZone) -> BTreeMap<String, Appointment> {
    events
        .iter()
        .map(|e| {
            let day = (e.start > 0).then(|| jiff::Timestamp::from_second(e.start).ok()).flatten().map(|t| t.to_zoned(zone.clone()).date());
            (e.uid.clone(), Appointment { day, cancelled: e.cancelled, recurring: e.recurring, start: e.start })
        })
        .collect()
}

/// The appointments there are, as the plan parks what prepares one gone:
/// `events` (those the pages show), then those of accounts switched off,
/// which are still here and park nothing (`switched_off_events`). A copy the
/// pages show wins over one of an account switched off. The date follows
/// the pages' events alone (`upkeep`): a copy left in an account switched
/// off never moves it.
pub fn appointments_here(events: &[EventRef], zone: &TimeZone) -> BTreeMap<String, Appointment> {
    let mut all = appointments_of(events, zone);
    for (uid, appointment) in appointments_of(&switched_off_events(zone), zone) {
        all.entry(uid).or_insert(appointment);
    }
    all
}

/// The events of accounts switched off (`vdir::switched_off_dirs`), read
/// again only when what they are changes: which accounts are switched off,
/// and each event file's name, length and time (a listing, on every system:
/// a folder's own time is not enough on Windows), or the time zone.
pub fn switched_off_events(zone: &TimeZone) -> Vec<EventRef> {
    type Key = (Vec<(PathBuf, u64, Option<std::time::SystemTime>)>, Option<String>);
    static READ: std::sync::Mutex<Option<(Key, Vec<EventRef>)>> = std::sync::Mutex::new(None);
    let mut listing: Vec<(PathBuf, u64, Option<std::time::SystemTime>)> = crate::vdir::switched_off_dirs(crate::vdir::Kind::Calendars)
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok))
        .filter(|e| e.path().extension().is_some_and(|x| x == "ics"))
        .map(|e| {
            let meta = e.metadata().ok();
            (e.path(), meta.as_ref().map_or(0, std::fs::Metadata::len), meta.and_then(|m| m.modified().ok()))
        })
        .collect();
    listing.sort();
    let key: Key = (listing, zone.iana_name().map(str::to_string));
    let mut cache = READ.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((known, events)) = cache.as_ref()
        && *known == key
    {
        return events.clone();
    }
    let events: Vec<EventRef> = key.0.iter().filter_map(|(p, _, _)| std::fs::read_to_string(p).ok().and_then(|t| agenda::event_ref(&t, &p.display().to_string(), zone))).collect();
    *cache = Some((key, events.clone()));
    events
}

/// The tasks of accounts switched off (`vdir::switched_off_dirs`), read
/// again only when their files change, as `switched_off_events`: what keeps
/// an appointment taken while its task's account is off (part E0.10).
pub fn switched_off_tasks(zone: &TimeZone) -> Vec<Task> {
    type Key = (Vec<(PathBuf, u64, Option<std::time::SystemTime>)>, Option<String>);
    static READ: std::sync::Mutex<Option<(Key, Vec<Task>)>> = std::sync::Mutex::new(None);
    let mut listing: Vec<(PathBuf, u64, Option<std::time::SystemTime>)> = crate::vdir::switched_off_dirs(crate::vdir::Kind::Calendars)
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok))
        .filter(|e| e.path().extension().is_some_and(|x| x == "ics"))
        .map(|e| {
            let meta = e.metadata().ok();
            (e.path(), meta.as_ref().map_or(0, std::fs::Metadata::len), meta.and_then(|m| m.modified().ok()))
        })
        .collect();
    listing.sort();
    let key: Key = (listing, zone.iana_name().map(str::to_string));
    let mut cache = READ.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((known, found)) = cache.as_ref()
        && *known == key
    {
        return found.clone();
    }
    let found: Vec<Task> = key.0.iter().filter_map(|(p, _, _)| std::fs::read_to_string(p).ok().filter(|t| t.contains("BEGIN:VTODO")).and_then(|t| tasks::task_of_text(&t, zone)).map(|t| Task { key: p.display().to_string(), read_only: true, ..t })).collect();
    *cache = Some((key, found.clone()));
    found
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
/// it, the one the event's own line names, else the oldest; else, by the same
/// order, a done one whose link names it (never the first one read: two
/// devices find the same, part E0.10); else the task the event's line names,
/// when that line still ties it (`line_ties`).
pub fn preparation_of<'a>(tasks: &'a [Task], event: &EventRef) -> Option<&'a Task> {
    let lined = event.links.iter().find(|l| l.rel == PREPARATION && l.uri.starts_with("sioul:task/")).map(|l| links::id_of(&l.uri));
    let named = |t: &&Task| appointment_of(t).as_deref() == Some(event.uid.as_str());
    let order = |a: &&Task, b: &&Task| (lined.as_deref() != Some(a.uid.as_str()), a.created, &a.uid).cmp(&(lined.as_deref() != Some(b.uid.as_str()), b.created, &b.uid));
    let mut open: Vec<&Task> = tasks.iter().filter(named).filter(|t| t.status.is_open()).collect();
    open.sort_by(order);
    let mut done: Vec<&Task> = tasks.iter().filter(named).filter(|t| !t.status.is_open()).collect();
    done.sort_by(order);
    open.first().copied().or_else(|| done.first().copied()).or_else(|| {
        let uid = lined?;
        tasks.iter().find(|t| t.uid == uid && line_ties(t, &event.uid))
    })
}

/// Whether an event's own line (`PREPARATION`) still ties `task` to it
/// (`event_uid`): the task was not untied from it (`UNTIED`), and names no
/// other appointment. The one test of a stale line, wherever a pair is read
/// (`preparation_of`, `links::World::edges`).
pub fn line_ties(task: &Task, event_uid: &str) -> bool {
    !task.untied.iter().any(|u| u == event_uid) && appointment_of(task).is_none_or(|other| other == event_uid)
}

/// A tied task about to go into a list that keeps no link (Google Tasks,
/// part E0.9): its tie undone by the one rule (`untie`), the event's line out
/// where it may be written, found among `events`. Empty for a task tied to
/// nothing.
pub fn untied_for_list(task: &Task, events: &[EventRef], now: &Zoned) -> Change {
    let Some(uid) = appointment_of(task) else { return Change::default() };
    let found = events.iter().find(|e| e.uid == uid && e.anchor.is_none()).and_then(|e| {
        let path = PathBuf::from(&e.key);
        let text = std::fs::read_to_string(&path).ok()?;
        let writable = path.parent().is_some_and(|dir| crate::vdir::collections(crate::vdir::Kind::Calendars).iter().any(|c| c.dir == dir && !c.read_only));
        Some((path, text, writable))
    });
    let event = found.as_ref().map(|(path, text, writable)| EventFile { path, text, writable: *writable });
    untie(task, &uid, event.as_ref(), now).unwrap_or_default()
}

/// A task and its steps (`family`) moved into the list at `target` through
/// the move journal, in one change; into a list that keeps no link (`untie`:
/// Google Tasks), a tie to an appointment undone in that same change
/// (`untied_for_list`), so that a move that fails unties nothing (review of
/// ab7daa4, later 5). Those already there stay.
pub fn moved_into(family: &[&Task], target: &Path, events: &[EventRef], untie: bool, now: &Zoned) -> Result<Change, String> {
    let mut change = Change::default();
    for task in family {
        let from = PathBuf::from(&task.key);
        if from.parent() == Some(target) {
            continue;
        }
        let mut text = std::fs::read_to_string(&from).map_err(|e| e.to_string())?;
        if untie {
            for (path, untied) in untied_for_list(task, events, now).written {
                if path == from {
                    text = untied;
                } else {
                    change.written.push((path, untied));
                }
            }
        }
        change.moved.push((from, target.join(format!("{}.ics", crate::vdir::new_name())), text));
    }
    Ok(change)
}

/// The task's own date, as the plan's rule for its steps reads it (`plan`):
/// its DUE's day, unless that is the day the pairing wrote (`Task::due_written`).
pub fn own_date(task: &Task) -> Option<Date> {
    if task.due.is_empty() || (!task.due_written.is_empty() && task.due == task.due_written) {
        return None;
    }
    task.due.get(..10)?.parse().ok()
}

/// The day the pairing may write as the DUE of `task` for an appointment on
/// `day`: none when the task starts at a time (a DUE of another value type,
/// which Sabre, so Murena, refuses), and never before the day it starts
/// (refused too, RFC 5545 §3.8.2.3): that day then.
fn writable_day(task: &Task, day: Date) -> Option<Date> {
    if task.start.contains('T') {
        return None;
    }
    let start = task.start.get(..10).and_then(|d| d.parse::<Date>().ok());
    Some(start.map_or(day, |s| s.max(day)))
}

/// What a task's link to its appointment says: the day the pairing wrote
/// as DUE, the task's own date it replaced, and the appointment's start it
/// was last set for (`AT`; none on a link written before 30 October 2026).
pub(crate) struct Tie {
    pub(crate) written: Option<Date>,
    pub(crate) was: Option<String>,
    pub(crate) at: Option<i64>,
}

pub(crate) fn tie_of(text: &str) -> Option<Tie> {
    let source = crate::lines::unfold(text);
    let line = source.iter().find(|l| pair_line(l, APPOINTMENT))?;
    let written = crate::lines::param(line, DUE_WRITTEN).and_then(|d| Date::strptime("%Y%m%d", d.trim()).ok());
    let was = crate::lines::param(line, DUE_WAS).map(|d| d.trim().to_string()).filter(|d| d.get(..10).is_some_and(|day| day.parse::<Date>().is_ok()));
    let at = crate::lines::param(line, AT).and_then(|a| crate::moves::read_utc(&a));
    Some(Tie { written, was, at })
}

/// Whether the task's DUE is the day the pairing wrote, exactly: a bare
/// date, the same. A time given to it, or another day, is yours.
fn sioul_set(task: &Task, tie: Option<&Tie>) -> bool {
    tie.and_then(|t| t.written).is_some_and(|w| task.due == w.to_string())
}

/// The task's own date as kept: the one the tie replaced (`DUE_WAS`), else its DUE when the tie did not write it.
fn own_kept(task: &Task, tie: Option<&Tie>) -> String {
    if sioul_set(task, tie) { tie.and_then(|t| t.was.clone()).unwrap_or_default() } else { task.due.clone() }
}

/// The task's own date to write: the one the tie replaced, put back beside
/// its start (`put_back`), else its DUE as it is.
fn own_due(task: &Task, tie: Option<&Tie>) -> String {
    if sioul_set(task, tie) { put_back(task, tie.and_then(|t| t.was.as_deref()).unwrap_or_default()) } else { task.due.clone() }
}

/// The date the tie replaced, as kept on its link, put back in a form a
/// server takes beside the task's start (Sabre, so Murena, wants DUE of
/// DTSTART's value type and not before it, RFC 5545 §3.8.2.3): as kept when
/// the task has no start; its bare day when the start is a date, never before
/// the start day (that day then); none when the start has a time.
fn put_back(task: &Task, kept: &str) -> String {
    if kept.is_empty() || task.start.is_empty() {
        return kept.to_string();
    }
    kept.get(..10).and_then(|d| d.parse::<Date>().ok()).and_then(|d| writable_day(task, d)).map(|d| d.to_string()).unwrap_or_default()
}

/// Whether the DUE is the date of its own an earlier move gave back
/// (`followed`), kept on the link: still the pairing's to follow.
fn given_back(task: &Task, tie: &Tie) -> bool {
    tie.written.is_none() && tie.was.as_deref().is_some_and(|w| !w.is_empty() && task.due == put_back(task, w))
}

/// The task's link to its appointment, with the day it wrote as DUE, the
/// date it replaced, and the appointment's start it is set for (`AT`).
fn appointment_line(event_uid: &str, due: Option<Date>, was: Option<&str>, at: Option<i64>) -> String {
    let due = due.map(|d| format!(";{DUE_WRITTEN}={}", d.strftime("%Y%m%d"))).unwrap_or_default();
    let was = was.filter(|w| !w.is_empty()).map(|w| format!(";{DUE_WAS}={}", crate::lines::param_value(w))).unwrap_or_default();
    let at = at.filter(|a| *a > 0).map(|a| format!(";{AT}={}", crate::moves::utc(a))).unwrap_or_default();
    crate::tasks::link_line_with(&crate::tasks::Link { uri: links::event_uri(event_uid), label: String::new(), rel: APPOINTMENT.to_string() }, &format!("{due}{was}{at}"))
}

/// The event's link to the task that prepares it.
fn preparation_line(task_uid: &str) -> String {
    crate::tasks::link_line(&crate::tasks::Link { uri: links::task_uri(task_uid), label: String::new(), rel: PREPARATION.to_string() })
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
    // Tied again as it is: nothing written (the MCP's tool is idempotent).
    let mut change = Change { written: (text != current).then(|| (PathBuf::from(&task.key), text)).into_iter().collect(), ..Change::default() };
    change.written.extend(event_text.map(|t| (event.path.to_path_buf(), t)));
    // Tied to another event: the line the old one keeps naming this task goes in the
    // same change, where it may be written (never a meeting with others, nor a calendar
    // you only read: `line_ties` reads it as stale there).
    let zone = now.time_zone();
    let new_uid = agenda::event_ref(event.text, &event.path.display().to_string(), zone).map(|r| r.uid).unwrap_or_default();
    if let Some(old) = tasks::task_of_text(&current, zone).and_then(|t| appointment_of(&t)).filter(|old| *old != new_uid)
        && let Some(old_ref) = refs.iter().find(|e| e.uid == old)
        && let Ok(old_text) = std::fs::read_to_string(&old_ref.key)
    {
        let path = PathBuf::from(&old_ref.key);
        let writable = path.parent().is_some_and(|dir| crate::vdir::collections(crate::vdir::Kind::Calendars).iter().any(|c| c.dir == dir && !c.read_only));
        let old_file = EventFile { path: &path, text: &old_text, writable };
        let names_it = |l: &str| pair_line(l, PREPARATION) && tasks::link_of(l).uri == links::task_uri(&task.uid);
        if old_file.takes_lines()
            && crate::lines::unfold(&old_text).iter().any(|l| names_it(l))
            && let Some(without) = agenda::remove_lines(&old_text, names_it)
        {
            change.written.push((path.clone(), without));
        }
    }
    Ok(change)
}

/// The task's text paired with the event, and the event's when it changes (`attach`).
fn paired(all: &[Task], task: &Task, current: &str, event: &EventFile, refs: &[EventRef], keeps_links: &dyn Fn(&str) -> bool, now: &Zoned) -> Result<(String, Option<String>), Refused> {
    let zone = now.time_zone().clone();
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
    // Taken by another open task, here or in a list of an account switched off (part E0.10).
    let off = switched_off_tasks(&zone);
    let everywhere: Vec<Task> = if off.is_empty() { Vec::new() } else { all.iter().cloned().chain(off.into_iter().filter(|t| !all.iter().any(|a| a.uid == t.uid))).collect() };
    let among: &[Task] = if everywhere.is_empty() { all } else { &everywhere };
    if preparation_of(among, &reference).is_some_and(|t| t.uid != task.uid && t.status.is_open()) {
        return Err(Refused("pair-taken"));
    }
    let known = refs.iter().find(|e| e.uid == reference.uid).cloned().unwrap_or(reference.clone());
    let day = (known.start > 0 && !known.recurring).then(|| jiff::Timestamp::from_second(known.start).ok()).flatten().map(|t| t.to_zoned(zone.clone()).date());
    // The instant it is tied for: what follows it next starts from there (`moves`).
    let at = (known.start > 0 && !known.recurring).then_some(known.start);
    let event_text = || -> Result<Option<String>, Refused> {
        if !event.takes_lines() {
            return Ok(None);
        }
        let without = agenda::remove_lines(event.text, |l| pair_line(l, PREPARATION)).ok_or(Refused("agenda-gone"))?;
        let with = agenda::add_lines(&without, &[preparation_line(&task.uid)]).ok_or(Refused("agenda-gone"))?;
        Ok((with != event.text).then_some(with))
    };
    // Tied again to the same appointment: neither its date nor its instant changes, whatever
    // this device's copy of the event says (one behind, undated, repeating: E0.4).
    if tasks::task_of_text(current, &zone).and_then(|t| appointment_of(&t)).as_deref() == Some(reference.uid.as_str()) {
        return Ok((current.to_string(), event_text()?));
    }
    // Its date: the appointment's day, when its own date is none or later; a
    // date Sioul wrote for another tie (or this one, tied again) is Sioul's own.
    let tie = tie_of(current);
    let own = own_kept(task, tie.as_ref());
    let own_day = own.get(..10).and_then(|d| d.parse::<Date>().ok());
    let mut edit = TaskEdit::of(task);
    // Never a DUE a server refuses (`writable_day`, `put_back`).
    let (written, was) = match day.and_then(|d| writable_day(task, d)) {
        Some(day) if own_day.is_none_or(|o| o > day) => {
            edit.due = day.to_string();
            (Some(day), Some(own))
        }
        // An earlier date of its own stays its own, nothing on the link; the date
        // the tie replaced keeps its mark, whether the DUE still says the day the
        // tie wrote or a move gave it back: it follows on, and tying again changes nothing.
        Some(_) => {
            edit.due = own_due(task, tie.as_ref());
            (None, tie.as_ref().filter(|t| sioul_set(task, Some(t)) || given_back(task, t)).and_then(|t| t.was.clone()))
        }
        None => {
            edit.due = own_due(task, tie.as_ref());
            (None, None)
        }
    };
    let line = appointment_line(&reference.uid, written, was.as_deref(), at);
    let mark = |l: &str| crate::lines::name(l) == UNTIED && crate::lines::value(l).trim() == reference.uid;
    let source = crate::lines::unfold(current);
    // Tied again as it is: nothing written.
    let text = if edit.due == task.due && source.iter().filter(|l| pair_line(l, APPOINTMENT)).eq([&line]) && !source.iter().any(|l| mark(l)) {
        current.to_string()
    } else {
        let text = tasks::apply(current, &edit, &zone, now).map_err(|_| Refused("task-gone"))?;
        tasks::replace_lines(&text, |l| pair_line(l, APPOINTMENT) || mark(l), vec![line], now).map_err(|_| Refused("task-gone"))?
    };
    Ok((text, event_text()?))
}

/// The pair undone (`untie`) from the task's end: its appointment, as its link names it.
pub fn detach(task: &Task, event: Option<&EventFile>, now: &Zoned) -> Result<Change, Refused> {
    if task.read_only {
        return Err(Refused("task-read-only"));
    }
    let Some(event_uid) = appointment_of(task) else { return Err(Refused("pair-none")) };
    untie(task, &event_uid, event, now)
}

/// The pair of `task` and the event `event_uid` undone, by one rule
/// wherever it is asked (the task panel's ×, the MCP, a Related list's "Undo
/// this link", `links::Loaded::untie`): the task's link out, its DUE back
/// (`untied_text`); the event's line out when it holds one and takes lines
/// (`EventFile::takes_lines`: never a meeting with others). An event that
/// keeps its line, or is not here (`event` None), leaves the task a mark
/// (`UNTIED`) so that it never ties it back. The legs stay with the event.
/// Refused for a task you only read, and when neither end names the other.
pub fn untie(task: &Task, event_uid: &str, event: Option<&EventFile>, now: &Zoned) -> Result<Change, Refused> {
    if task.read_only {
        return Err(Refused("task-read-only"));
    }
    let current = std::fs::read_to_string(&task.key).map_err(|_| Refused("task-gone"))?;
    let tied = tasks::task_of_text(&current, now.time_zone()).and_then(|t| appointment_of(&t)).as_deref() == Some(event_uid);
    let names_it = |l: &str| pair_line(l, PREPARATION) && tasks::link_of(l).uri == links::task_uri(&task.uid);
    let lined = event.is_some_and(|e| crate::lines::unfold(e.text).iter().any(|l| names_it(l)));
    if !tied && !lined {
        return Err(Refused("pair-none"));
    }
    let mut change = Change::default();
    let cleared = event.is_some_and(|e| e.takes_lines());
    if let Some(event) = event.filter(|e| e.takes_lines() && lined) {
        let without = agenda::remove_lines(event.text, names_it).ok_or(Refused("agenda-gone"))?;
        change.written.push((event.path.to_path_buf(), without));
    }
    // The event's line stays where it cannot be written: the mark keeps it from tying the task back.
    let marked = (!cleared).then_some(event_uid);
    let text = if tied {
        untied_text(&current, marked, now).ok_or(Refused("task-gone"))?
    } else {
        let mark = |l: &str| crate::lines::name(l) == UNTIED && crate::lines::value(l).trim() == event_uid;
        match marked {
            Some(uid) => tasks::replace_lines(&current, mark, vec![format!("{UNTIED}:{uid}")], now).map_err(|_| Refused("task-gone"))?,
            None => current.clone(),
        }
    };
    if text != current {
        change.written.insert(0, (PathBuf::from(&task.key), text));
    }
    Ok(change)
}

/// A task's text with its tie to its appointment undone, by the one date
/// rule wherever a tie is undone (`untie`, `appointments::retarget`): its
/// DUE back to the date it replaced when it still says the day the pairing
/// wrote, its link out, and the mark `UNTIED` for `marked` (an event that
/// keeps its line). None when the text holds no task.
pub fn untied_text(text: &str, marked: Option<&str>, now: &Zoned) -> Option<String> {
    let zone = now.time_zone().clone();
    let task = tasks::task_of_text(text, &zone)?;
    let tie = tie_of(text);
    let mut out = text.to_string();
    if sioul_set(&task, tie.as_ref()) {
        out = tasks::apply(&out, &TaskEdit { due: own_due(&task, tie.as_ref()), ..TaskEdit::of(&task) }, &zone, now).ok()?;
    }
    let mark = |l: &str| crate::lines::name(l) == UNTIED && marked.is_some_and(|uid| crate::lines::value(l).trim() == uid);
    let marks = marked.map(|uid| vec![format!("{UNTIED}:{uid}")]).unwrap_or_default();
    tasks::replace_lines(&out, |l| pair_line(l, APPOINTMENT) || mark(l), marks, now).ok()
}

/// The task's text with its DUE following its appointment's day: the
/// earlier of that day and the date of its own the pairing replaced, kept on
/// the link either way (`X-SIOUL-DUE-WAS`), while the DUE says the day the
/// pairing wrote or that date of its own; the link then set for the
/// appointment's start (`AT`). None when nothing is to change (its DUE yours
/// since, a repeating or undated appointment, the same day), and never a DUE
/// a server refuses (`writable_day`). The task is read from `text`, the file
/// as it is now, never from what was read before (`task` gives its file and
/// whether its list can be written): a title changed meanwhile stays.
/// `moves::upkeep` asks it only of an appointment this device saw move.
pub fn followed(task: &Task, text: &str, appointment: &Appointment, now: &Zoned) -> Option<String> {
    let zone = now.time_zone().clone();
    let task = &Task { key: task.key.clone(), list_id: task.list_id.clone(), list: task.list.clone(), read_only: task.read_only, ..tasks::task_of_text(text, &zone)? };
    if appointment.recurring || appointment.cancelled || task.read_only || !task.status.is_open() {
        return None;
    }
    let tie = tie_of(text)?;
    // Its own date, given back by an earlier move: still the pairing's to follow.
    if !sioul_set(task, Some(&tie)) && !given_back(task, &tie) {
        return None;
    }
    let day = writable_day(task, appointment.day?)?;
    let uid = appointment_of(task)?;
    // Its own date when earlier, as it may be written beside its start (`put_back`).
    let back = put_back(task, tie.was.as_deref().unwrap_or_default());
    let (due, written) = match back.get(..10).and_then(|d| d.parse::<Date>().ok()) {
        Some(o) if o <= day => (back.clone(), None),
        _ => (day.to_string(), Some(day)),
    };
    if due == task.due && written == tie.written {
        return None;
    }
    let at = (appointment.start > 0).then_some(appointment.start);
    let moved = tasks::apply(text, &TaskEdit { due, ..TaskEdit::of(task) }, &zone, now).ok()?;
    tasks::replace_lines(&moved, |l| pair_line(l, APPOINTMENT), vec![appointment_line(&uid, written, tie.was.as_deref(), at)], now).ok()
}

/// Where a paired task stands to its appointment for the one rule of what
/// follows (`moves::verdict`): whether its date is as the appointment is,
/// and whether this device saw the appointment at the instant the task was
/// set for (`seen`). A task whose date the tie no longer holds (its own,
/// set since) follows nothing: consistent. A link written before its
/// instant was kept (`AT`) stands by the day it wrote, or, with no day of its
/// own on it, by what this device saw last. With the instant it was set for
/// (`AT`, else 0): its doubt's key (`moves::Seen::since`).
pub fn stand(task: &Task, text: &str, appointment: &Appointment, seen: Option<crate::moves::Instants>, zone: &TimeZone) -> (bool, bool, i64) {
    let Some(tie) = tie_of(text) else { return (true, false, 0) };
    let at = tie.at.unwrap_or(0);
    let Some(fresh) = tasks::task_of_text(text, zone) else { return (true, false, at) };
    let fresh = Task { read_only: task.read_only, ..fresh };
    if appointment.recurring || appointment.cancelled || fresh.read_only || !fresh.status.is_open() || (!sioul_set(&fresh, Some(&tie)) && !given_back(&fresh, &tie)) {
        return (true, false, at);
    }
    let Some(day) = appointment.day else { return (true, false, at) };
    let day_of = |at: i64| jiff::Timestamp::from_second(at).ok().map(|t| t.to_zoned(zone.clone()).date());
    let seen_day = seen.and_then(|s| day_of(s.start));
    let set_for = tie.at.and_then(day_of).or(tie.written);
    // A link written before its instant was kept says a day in the zone it was written in,
    // which is not known: the appointment's day in some zone holds (review of ab7daa4, later 13).
    let some_zone = |set: Date| {
        (-12..=14).any(|hours: i32| jiff::tz::Offset::from_hours(hours as i8).ok().is_some_and(|o| jiff::Timestamp::from_second(appointment.start).is_ok_and(|t| t.to_zoned(jiff::tz::TimeZone::fixed(o)).date() == set)))
    };
    match set_for {
        Some(set) if tie.at.is_none() && appointment.start > 0 => (set == day || some_zone(set), seen_day == Some(set), at),
        Some(set) => (set == day, seen_day == Some(set), at),
        None => (seen_day.is_none_or(|s| s == day), seen_day.is_some(), at),
    }
}

/// On a step or a task set aside by itself once its appointment was over
/// (`closing`): `X-SIOUL-SET-ASIDE;END=<the appointment's end, UTC>:<its
/// UID>`, and `X-SIOUL-SET-ASIDE;BROUGHT-BACK=TRUE:<its UID>` once brought
/// back (`brought_back`), after which that appointment never sets it aside
/// again (another one it is tied to since may: review of 4d7bab4, later 10). Only on a
/// CANCELLED one: a task the closing completes is done, never set aside. An
/// older Sioul (679f96a) keeps the line as it is, parameters with it (its
/// writers keep a line they do not know), and sees a dropped step.
pub const SET_ASIDE: &str = "X-SIOUL-SET-ASIDE";

/// What closes by itself once an appointment is over (composed tasks §7.5,
/// the unified agenda's part 6): the open steps of what prepares it that come
/// before it are set aside (`CANCELLED`, never `COMPLETED`: Sioul never says you
/// did them; marked `SET_ASIDE` with the appointment's end, so that its card
/// and the done tasks list them and bring each back). A task with steps whose
/// steps are all closed then closes too, done when every step was done, else
/// set aside the same way; never while it has a time block not begun (`blocks::upkeep`
/// would take out the block's time to come). Nothing is said, and nothing
/// counted. Only a step that is plainly a preparation: never one that comes
/// after it (`X-SIOUL-ANCHOR:AFTER`, read from its file as it is), nor one with
/// a start, a date of its own on or after the appointment's day, or a time
/// block (each a follow-up by the plan's rule), nor one made or made a step
/// once the appointment was over (a follow-up, never a preparation forgotten),
/// nor one changed since (its LAST-MODIFIED after the end), nor one with open
/// steps of its own, nor one that carries that appointment's mark while open
/// (brought back, here or by an agent or another program). Only once the
/// appointment's end has passed and, for a calendar or a list that syncs, a
/// pull begun after it went through whole (`moves::Pull`: the appointment's
/// calendar, the task's list and each step's, so that a step changed on
/// another device after the end is read first: review of 4d7bab4, later 12);
/// never for an appointment that
/// ended before this device first ran this (`from`, kept in
/// `moves::Seen::closing_from`: the past at the install is no preparation
/// forgotten), that repeats, is cancelled, lasts the whole day, is a meeting
/// with others (whose time may have moved in a message not answered yet), or
/// is not here. Stamped with the appointment's end, so that two devices write
/// the same bytes. In the same change, what an appointment moved later gives
/// back (`given_back_by_moves`). The change, for one write.
pub fn closing(tasks: &[Task], events: &[EventRef], from: i64, now: &Zoned) -> Change {
    closing_with(tasks, events, from, now, crate::moves::Pull::default())
}

/// `closing`, this device's own calendars and lists read as `local` (`moves::Pull::of_with`).
pub fn closing_with(tasks: &[Task], events: &[EventRef], from: i64, now: &Zoned, local: crate::moves::Pull) -> Change {
    let zone = now.time_zone();
    let stamp = now.timestamp().as_second();
    let mut change = Change { written: given_back_by_moves(tasks, events, now), ..Change::default() };
    for preparation in tasks.iter().filter(|t| t.status.is_open() && !t.read_only) {
        let Some(uid) = appointment_of(preparation) else { continue };
        let Some(event) = events.iter().find(|e| e.uid == uid && e.anchor.is_none() && !e.recurring && !e.cancelled) else { continue };
        let path = Path::new(&event.key);
        let Ok(event_text) = std::fs::read_to_string(path) else { continue };
        if agenda::with_others(&event_text) || agenda::edit_of_text(&event_text, zone).is_none_or(|e| e.all_day) {
            continue;
        }
        let Some((start, end)) = agenda::first_times(&event_text, zone) else { continue };
        // Read whole since the end: a calendar or a list that syncs, by a pull begun after it.
        let pulled = |path: &Path| {
            let pull = crate::moves::Pull::of_with(path, local);
            !pull.syncs || pull.began.is_some_and(|began| began > end)
        };
        if end > stamp || end < from || !pulled(path) || !pulled(Path::new(&preparation.key)) {
            continue;
        }
        let Ok(at) = jiff::Timestamp::from_second(end).map(|t| t.to_zoned(zone.clone())) else { continue };
        let day = jiff::Timestamp::from_second(start).map_or(at.date(), |t| t.to_zoned(zone.clone()).date());
        let steps = tasks::steps_of(tasks, &preparation.uid);
        // That appointment's mark on it while open: brought back, whatever reopened it (review of
        // 4d7bab4, later 2); one of another appointment holds nothing here (later 10).
        if steps.is_empty() || preparation.set_aside == uid {
            continue;
        }
        let mark = format!("{SET_ASIDE};END={}:{uid}", crate::moves::utc(end));
        let set_aside = |text: &str| -> Option<String> {
            let dropped = tasks::set_status(text, Status::Cancelled, zone, &at).ok()?;
            tasks::replace_lines(&dropped, |l| crate::lines::name(l) == SET_ASIDE, vec![mark.clone()], &at).ok()
        };
        let (mut open_left, mut all_done) = (false, true);
        for step in &steps {
            // Its file as it is now: a mark set since the tasks were read counts.
            let Some((text, fresh)) = std::fs::read_to_string(&step.key).ok().and_then(|t| tasks::task_of_text(&t, zone).map(|f| (t, f))) else {
                open_left = true;
                continue;
            };
            if !fresh.status.is_open() {
                all_done &= fresh.status == Status::Completed;
                continue;
            }
            // Its list read whole since the end: a change made elsewhere after it comes first.
            if !pulled(Path::new(&step.key)) {
                open_left = true;
                continue;
            }
            // Made once the appointment was over (review of 4d7bab4, M-a), or made a step since
            // (`composed::demote` keeps CREATED; its place says when, later 9), or changed since
            // (opened again, moved to the top of the steps, renamed: review of 468a123, §3.1;
            // the closing's own writes are stamped with the end, never after it): a follow-up.
            let made_after = fresh.created > end || fresh.placed_at().is_some_and(|placed| placed > end) || fresh.modified > end;
            // Its own open steps would stay laid under a step set aside (later 11).
            let has_open_steps = tasks::steps_of(tasks, &fresh.uid).iter().any(|s| s.status.is_open());
            let follow_up = fresh.after || !fresh.start.is_empty() || own_date(&fresh).is_some_and(|d| d >= day) || fresh.links.iter().any(crate::blocks::is_block_link) || fresh.set_aside == uid || made_after || has_open_steps;
            if follow_up || step.read_only {
                open_left = true;
                continue;
            }
            match set_aside(&text) {
                Some(dropped) => {
                    change.written.push((PathBuf::from(&step.key), dropped));
                    all_done = false;
                }
                None => open_left = true,
            }
        }
        // With a time block not begun, it stays open: that block is never taken out for it
        // (review of 4d7bab4, later 4); one over pins nothing (review of 468a123, §3.2).
        let pinned = events.iter().any(|e| e.task == preparation.uid && e.anchor.is_none() && !e.cancelled && e.start > stamp) || pinned_beyond(preparation, events, stamp, zone);
        if !open_left
            && !pinned
            && let Ok(text) = std::fs::read_to_string(&preparation.key)
        {
            // Done when every step was done: completed, never marked (later 1).
            let closed = if all_done { tasks::set_status(&text, Status::Completed, zone, &at).ok() } else { set_aside(&text) };
            change.written.extend(closed.map(|t| (PathBuf::from(&preparation.key), t)));
        }
    }
    change
}

/// Whether a task's time block lies beyond the events read (`events`, the
/// coming days alone): its block links' own files, named by their UID in a
/// calendar (`blocks` writes `<uid>.ics`), one not begun keeping it pinned
/// (review of eb6f660, later 4: a block three months ahead).
fn pinned_beyond(task: &Task, events: &[EventRef], stamp: i64, zone: &TimeZone) -> bool {
    let uids: Vec<String> = task.links.iter().filter(|l| crate::blocks::is_block_link(l)).map(|l| l.uri.trim_start_matches("uid:").to_string()).filter(|u| !u.is_empty() && !events.iter().any(|e| e.uid == *u)).collect();
    if uids.is_empty() {
        return false;
    }
    let calendars = crate::vdir::collections(crate::vdir::Kind::Calendars);
    uids.iter().any(|uid| {
        calendars.iter().filter_map(|c| std::fs::read_to_string(c.dir.join(format!("{uid}.ics"))).ok()).any(|text| {
            let block = agenda::event_ref(&text, "", zone);
            block.is_some_and(|b| b.task == task.uid && b.anchor.is_none() && !b.cancelled) && agenda::first_times(&text, zone).is_some_and(|(start, _)| start > stamp)
        })
    })
}

/// What an appointment moved later gives back (review of 4d7bab4, M-b): each
/// task or step it set aside (`SET_ASIDE`, CANCELLED, not brought back), once
/// the appointment ends after the end its mark keeps and after now: open
/// again, its mark taken out, so that it is set aside again only once the
/// appointment is over again. The appointment's end is compared with the one
/// kept, never the clock alone: two devices whose clocks differ near its end
/// never undo each other. Stamped with the set-aside's own stamp (END, else
/// the item's last change): the same bytes on every device, whatever a server
/// did to the appointment's own lines, never a stamp to come, and never one
/// after the end that would keep it from being set aside again (review of
/// 468a123, §3.1). Never on its deletion, its cancellation or its repeating: an appointment
/// over, then deleted, happened. A step comes back only while its task is open, or is set
/// aside with the same mark and comes back with it: never an open step under a task done
/// since (§3.5). The task tied to the appointment comes back with its date following it,
/// as the follow would write it (`followed`), not one pull late (§3.7). Its files are read
/// only when the appointment, as read, ends after the mark's end and after now (§3.8).
fn given_back_by_moves(tasks: &[Task], events: &[EventRef], now: &Zoned) -> Vec<(PathBuf, String)> {
    let zone = now.time_zone();
    let stamp = now.timestamp().as_second();
    let mut out = Vec::new();
    for task in tasks.iter().filter(|t| t.status == Status::Cancelled && !t.set_aside.is_empty() && !t.brought_back && !t.read_only) {
        let Some(event) = events.iter().find(|e| e.uid == task.set_aside && e.anchor.is_none() && !e.recurring && !e.cancelled) else { continue };
        if event.end > 0 && (event.end <= task.set_aside_end || event.end <= stamp) {
            continue;
        }
        if let Some(whole) = task.parent().and_then(|p| tasks.iter().find(|t| t.uid == p))
            && !(whole.status.is_open() || (whole.status == Status::Cancelled && whole.set_aside == task.set_aside && !whole.brought_back))
        {
            continue;
        }
        let Some(appointment) = crate::anchors::Followed::read(Path::new(&event.key), zone) else { continue };
        if appointment.cancelled || appointment.recurring {
            continue;
        }
        // Its file as it is now.
        let Some((text, fresh)) = std::fs::read_to_string(&task.key).ok().and_then(|t| tasks::task_of_text(&t, zone).map(|f| (t, f))) else { continue };
        let end = appointment.instants.end;
        if fresh.status != Status::Cancelled || fresh.set_aside != task.set_aside || fresh.brought_back || end <= fresh.set_aside_end || end <= stamp {
            continue;
        }
        // The set-aside's own stamp (END): the same bytes whatever the server did to the
        // appointment's, and "moved later, then back" consistent (review of 468a123, §3.1).
        let changed = if fresh.set_aside_end > 0 { fresh.set_aside_end } else { fresh.modified };
        let Ok(at) = jiff::Timestamp::from_second(changed).map(|t| t.to_zoned(zone.clone())) else { continue };
        let Some(back) = tasks::set_status(&text, Status::NeedsAction, zone, &at).ok().and_then(|t| tasks::remove_lines(&t, |l| crate::lines::name(l) == SET_ASIDE, &at).ok()) else { continue };
        // Tied to it: its date follows the appointment as it is now, in the same write.
        let day = jiff::Timestamp::from_second(appointment.instants.start).ok().map(|t| t.to_zoned(zone.clone()).date());
        let tied = Appointment { day, cancelled: false, recurring: false, start: appointment.instants.start };
        let back = if appointment_of(&fresh).as_deref() == Some(task.set_aside.as_str()) { followed(task, &back, &tied, &at).unwrap_or(back) } else { back };
        out.push((PathBuf::from(&task.key), back));
    }
    out
}

/// What bringing back `uid` writes (Bring back, and a set-aside row ticked
/// open: review of 468a123, §3.8): it open again, never set aside again, and
/// its task when that was set aside with it (the same mark, not brought
/// back). Empty when it is not set aside so, or is read-only.
pub fn bringing_back(all: &[Task], uid: &str, now: &Zoned) -> Vec<(PathBuf, String)> {
    let back = |task: &Task| std::fs::read_to_string(&task.key).ok().and_then(|text| brought_back(&text, now)).map(|t| (PathBuf::from(&task.key), t));
    let Some(task) = all.iter().find(|t| t.uid == uid).filter(|t| !t.read_only) else { return Vec::new() };
    let mut written: Vec<(PathBuf, String)> = back(task).into_iter().collect();
    if written.is_empty() {
        return written;
    }
    if let Some(whole) = task.parent().and_then(|p| all.iter().find(|t| t.uid == p)).filter(|w| !w.read_only && w.set_aside == task.set_aside && !w.brought_back) {
        written.extend(back(whole));
    }
    written
}

/// Whether a task is one an appointment over set aside by itself, not
/// brought back: ticked open, it is brought back (`bringing_back`).
pub fn is_set_aside(task: &Task) -> bool {
    task.status == Status::Cancelled && !task.set_aside.is_empty() && !task.brought_back
}

/// A step or a task set aside by itself (`closing`) brought back: open
/// again, its mark kept with `BROUGHT-BACK=TRUE`, so that it is never set
/// aside again. None when it was not set aside so, or is no longer (done or
/// open again since).
pub fn brought_back(text: &str, now: &Zoned) -> Option<String> {
    let zone = now.time_zone();
    let task = tasks::task_of_text(text, zone)?;
    if task.set_aside.is_empty() || task.brought_back || task.status != Status::Cancelled {
        return None;
    }
    let open = tasks::set_status(text, Status::NeedsAction, zone, now).ok()?;
    tasks::replace_lines(&open, |l| crate::lines::name(l) == SET_ASIDE, vec![format!("{SET_ASIDE};BROUGHT-BACK=TRUE:{}", task.set_aside)], now).ok()
}

/// A task to prepare the event (its steps and its list): titled as the
/// event, paired with it, in `list`. Its UID with the change.
pub fn new_for(all: &[Task], event: &EventFile, refs: &[EventRef], list: &crate::vdir::Collection, keeps_links: &dyn Fn(&str) -> bool, now: &Zoned) -> Result<(Change, String), Refused> {
    let zone = now.time_zone().clone();
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
    let zone = now.time_zone().clone();
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

/// A thing named in an appointment's view: a task or a step, by its UID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Named {
    pub uid: String,
    pub title: String,
}

/// An appointment as the task views say it (composed tasks §7.2): its
/// title, its first start, and its earliest leg before it (its title and its
/// start as written).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outing {
    pub title: String,
    pub start: i64,
    pub leg: Option<(String, i64)>,
}

/// The appointments of `events` (one time, not cancelled), by their UID,
/// each with its earliest leg before it (`anchors`), for the task views.
pub fn outings(events: &[EventRef]) -> BTreeMap<String, Outing> {
    let mut out: BTreeMap<String, Outing> = events.iter().filter(|e| e.anchor.is_none() && !e.cancelled && !e.recurring && e.start > 0).map(|e| (e.uid.clone(), Outing { title: e.summary.clone(), start: e.start, leg: None })).collect();
    for leg in events.iter().filter(|e| !e.cancelled && e.start > 0) {
        let Some(anchor) = leg.anchor.as_ref().filter(|a| a.side == crate::anchors::Side::Before) else { continue };
        if let Some(outing) = out.get_mut(&anchor.appointment)
            && outing.leg.as_ref().is_none_or(|(_, at)| leg.start < *at)
        {
            outing.leg = Some((leg.summary.clone(), leg.start));
        }
    }
    out
}

/// The appointment `task` prepares, or its bigger task does, as the task
/// views say it (`outings`); None for one not here.
pub fn outing_of<'a>(task: &Task, tasks: &[Task], outings: &'a BTreeMap<String, Outing>) -> Option<&'a Outing> {
    let mut at = Some(task);
    let mut seen = std::collections::BTreeSet::new();
    while let Some(t) = at {
        if !seen.insert(t.uid.clone()) {
            return None;
        }
        if let Some(outing) = appointment_of(t).and_then(|uid| outings.get(&uid)) {
            return Some(outing);
        }
        at = t.parent().and_then(|p| tasks.iter().find(|x| x.uid == p));
    }
    None
}

/// One item to bring or to check, from the preparation's list (`checklist`):
/// its task, its line there (what `checkItem` ticks), its words, its mark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bring {
    pub task: String,
    pub line: usize,
    pub text: String,
    pub done: bool,
}

/// A group of the preparation's list ("Bring", "Check"), five items at most,
/// and how many more it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BringGroup {
    pub name: String,
    pub items: Vec<Bring>,
    pub more: usize,
}

/// A leg as an appointment's view says it: where it will be once followed
/// (`anchors::effective`), and whether its timetable was not checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegView {
    pub uid: String,
    pub key: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    /// "13:10 – 13:35".
    pub when: String,
    pub unconfirmed: bool,
}

/// An appointment as composed tasks' part F says it (the unified agenda,
/// parts 4 and 6), typed, the one thing the task's card and the event's card
/// read: the appointment, when to leave (its first leg, else the time kept
/// before it), what prepares it, its next steps (Before), its legs before it
/// (Getting there), what to bring, its legs and steps after it (Coming
/// back), and its line on the timeline ("Leave at 13:10 · Bus 24 · Next:
/// check the bus schedule"). For a leg: what it is to its appointment. Read only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppointmentView {
    pub uid: String,
    pub key: String,
    pub title: String,
    /// When it is, as the agenda says it.
    pub when: String,
    /// Its file is here and it is not cancelled.
    pub here: bool,
    /// When to leave, Unix seconds: the start of its earliest leg before it
    /// (part E), else its start less the time kept before it (your own time
    /// around a meeting with others, `own_time`); none without either.
    pub leave_at: Option<i64>,
    /// The task that prepares it.
    pub preparation: Option<Named>,
    /// Its open steps, the next first, two at most, and how many more.
    pub before: Vec<Named>,
    pub more: usize,
    /// The preparation's lists by group, five items each at most.
    pub bring: Vec<BringGroup>,
    /// Its legs before it, by time (Getting there).
    pub getting_there: Vec<LegView>,
    /// Its legs after it, by time, then its open steps that come after it (Coming back).
    pub coming_back: Vec<LegView>,
    pub steps_after: Vec<Named>,
    /// Other open tasks tied to it besides its preparation ("Also tied").
    pub also: Vec<Named>,
    /// What it set aside once over (`closing`), each to bring back ("Set aside after it").
    pub set_aside: Vec<Named>,
    /// The event is itself a leg: what it is to its appointment.
    pub leg_of: Option<crate::anchors::LegOf>,
    /// "Leave at 13:10 · Bus 24 · Next: check the bus schedule"; "" when none is known.
    pub line: String,
    /// "Leave at 13:10 · Bus 24": the line without its next step, which the
    /// task's card says under its appointment (composed tasks §7.2).
    pub leave: String,
}

/// The appointment `event` (one time of it) as part F says it, from the
/// tasks as read: what prepares it (`preparation_of`), its open steps in their
/// order, before it or after it (`Task::after`), its lists, its legs (`legs`,
/// each where it will be once followed); when to leave from its first leg,
/// else the time kept before it; `leg_of` when the event is itself a leg.
/// Nothing is written: a step left open costs nothing, and nothing asks to tick.
pub fn appointment_view(event: &agenda::Occurrence, links: &[tasks::Link], tasks: &[Task], legs: &[crate::anchors::LegSeen], leg_of: Option<crate::anchors::LegOf>, tr: &crate::i18n::Translator, zone: &TimeZone) -> AppointmentView {
    let reference = EventRef { key: event.key.clone(), uid: event.uid.clone(), links: links.to_vec(), ..EventRef::default() };
    let preparation = preparation_of(tasks, &reference).filter(|t| t.status.is_open());
    let open: Vec<&Task> = preparation.map(|p| tasks::steps_of(tasks, &p.uid).into_iter().filter(|s| s.status.is_open()).collect()).unwrap_or_default();
    let steps: Vec<&Task> = open.iter().copied().filter(|s| !s.after).collect();
    let steps_after: Vec<Named> = open.iter().filter(|s| s.after).map(|s| Named { uid: s.uid.clone(), title: s.title.clone() }).collect();
    let before: Vec<Named> = steps.iter().take(2).map(|s| Named { uid: s.uid.clone(), title: s.title.clone() }).collect();
    let more = steps.len().saturating_sub(before.len());
    let also: Vec<Named> = tasks.iter().filter(|t| t.status.is_open() && appointment_of(t).as_deref() == Some(event.uid.as_str()) && preparation.is_none_or(|p| p.uid != t.uid)).map(|t| Named { uid: t.uid.clone(), title: t.title.clone() }).collect();
    // Set aside, never done: a task the closing completed, or a step done since, is not listed (review of 4d7bab4, later 1).
    let set_aside: Vec<Named> = tasks.iter().filter(|t| t.set_aside == event.uid && !t.brought_back && t.status == Status::Cancelled).map(|t| Named { uid: t.uid.clone(), title: t.title.clone() }).collect();
    let span = |a: i64, b: i64| {
        let hm = |at: i64| jiff::Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default();
        format!("{} – {}", hm(a), hm(b))
    };
    let mut views: Vec<(crate::anchors::Side, LegView)> = legs
        .iter()
        .filter(|l| l.anchor.appointment == event.uid)
        .map(|l| {
            let at = crate::anchors::effective(crate::moves::Instants { start: l.start, end: l.end }, &l.anchor, crate::moves::Instants { start: event.start, end: event.end });
            (l.anchor.side, LegView { uid: l.uid.clone(), key: l.key.clone(), title: l.title.clone(), start: at.start, end: at.end, when: span(at.start, at.end), unconfirmed: l.anchor.unconfirmed.is_some() })
        })
        .collect();
    views.sort_by_key(|(_, v)| v.start);
    let getting_there: Vec<LegView> = views.iter().filter(|(s, _)| *s == crate::anchors::Side::Before).map(|(_, v)| v.clone()).collect();
    let coming_back: Vec<LegView> = views.iter().filter(|(s, _)| *s == crate::anchors::Side::After).map(|(_, v)| v.clone()).collect();
    let mut bring: Vec<BringGroup> = Vec::new();
    for task in preparation.into_iter().chain(steps.iter().copied()) {
        for item in crate::checklist::items(&task.notes) {
            let name = item.group.clone();
            let entry = Bring { task: task.uid.clone(), line: item.line, text: item.text.clone(), done: item.mark != crate::checklist::Mark::Open };
            match bring.iter_mut().find(|g| g.name == name) {
                Some(group) if group.items.len() < 5 => group.items.push(entry),
                Some(group) => group.more += 1,
                None => bring.push(BringGroup { name, items: vec![entry], more: 0 }),
            }
        }
    }
    let leave_at = getting_there.first().map(|l| l.start).or_else(|| (event.margins.before > 0 && !event.all_day).then(|| event.start - i64::from(event.margins.before) * 60));
    let clock = |at: i64| jiff::Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    let say = |id: &str, name: &str, value: String| {
        let mut args = crate::i18n::args();
        args.set(name, value);
        tr.text(id, Some(&args))
    };
    let mut parts = Vec::new();
    if let Some(at) = leave_at {
        parts.push(say("appointment-leave-at", "time", clock(at)));
    }
    // Its legs on its line (the unified agenda, §4.7): titled, a doubt said.
    if !getting_there.is_empty() {
        parts.push(getting_there.iter().map(|l| if l.unconfirmed { say("legs-unconfirmed", "leg", l.title.clone()) } else { l.title.clone() }).collect::<Vec<_>>().join(", "));
    }
    // When to leave and how, the task's card's line under its appointment (§7.2): no next step there.
    let leave = parts.join(" · ");
    if let Some(next) = before.first() {
        parts.push(say("appointment-next", "step", next.title.clone()));
    }
    let when = jiff::Timestamp::from_second(event.start).map(|t| tr.when(&t.to_zoned(zone.clone()))).unwrap_or_default();
    AppointmentView {
        uid: event.uid.clone(),
        key: event.key.clone(),
        title: event.summary.clone(),
        when,
        here: !event.cancelled,
        leave_at,
        preparation: preparation.map(|p| Named { uid: p.uid.clone(), title: p.title.clone() }),
        before,
        more,
        bring,
        getting_there,
        coming_back,
        steps_after,
        also,
        set_aside,
        leg_of,
        line: parts.join(" · "),
        leave,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
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
            // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
            crate::config::stand_ins_only();
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
        Appointment { day: day(on), cancelled: false, recurring: false, start: 0 }
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
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        // Tied again as it is: nothing written (check of 96b3cd8, item 10).
        assert!(attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().is_empty());
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
        // Check of 96b3cd8, item 6: that date kept on the link, a later move before it is followed,
        // and one past it gives it back again: always the earlier.
        std::fs::write(&task.key, &moved).unwrap();
        let task = task_of(&desk, "later");
        let earlier = followed(&task, &moved, &at("2026-11-20"), &now()).unwrap();
        assert_eq!(tasks::task_of_text(&earlier, &TimeZone::system()).unwrap().due, "2026-11-20");
        std::fs::write(&task.key, &earlier).unwrap();
        let task = task_of(&desk, "later");
        let again = followed(&task, &earlier, &at("2026-12-10"), &now()).unwrap();
        assert_eq!(tasks::task_of_text(&again, &TimeZone::system()).unwrap().due, "2026-11-30");
        assert!(followed(&Task { key: task.key.clone(), ..tasks::task_of_text(&again, &TimeZone::system()).unwrap() }, &again, &at("2026-12-12"), &now()).is_none(), "the same, nothing written");
        // A time given to the date: yours, never moved.
        let task = task_of(&desk, "prep");
        let timed = tasks::apply(&read(Path::new(&task.key)), &TaskEdit { due: "2026-11-03T09:00".into(), ..TaskEdit::of(&task) }, &TimeZone::system(), &now()).unwrap();
        let task = Task { key: task.key.clone(), ..tasks::task_of_text(&timed, &TimeZone::system()).unwrap() };
        assert!(followed(&task, &timed, &at("2026-11-07"), &now()).is_none());
    }

    fn read_event(uid: &str, start: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261020T100000Z\r\nDTSTART:{start}\r\nSUMMARY:Doctor\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")
    }

    /// A calendar that syncs (its state names a server), as the follow rule reads it
    /// (`moves::Pull`), with the visit on 29 October; its pull's start set by `pulled`.
    fn synced_visit(account: &str, pulled: Option<i64>) -> (PathBuf, PathBuf) {
        let dir = vdir::prepare(Kind::Calendars, account, "agenda", "Synced", None).unwrap();
        let state = vdir::state_path(account, Kind::Calendars, "agenda");
        let mut known = vdir::State::load(&state);
        known.url = "https://dav.example.org/calendars/agenda/".into();
        known.components = vec!["VEVENT".into()];
        known.pulled = pulled;
        known.save(&state).unwrap();
        let path = dir.join("visit.ics");
        std::fs::write(&path, read_event("visit", "20261029T130000Z")).unwrap();
        (dir, state)
    }

    fn refs_in(dir: &Path) -> Vec<EventRef> {
        let zone = TimeZone::system();
        std::fs::read_dir(dir).unwrap().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ics")).filter_map(|p| agenda::event_ref(&read(&p), &p.display().to_string(), &zone)).collect()
    }

    /// F4, by the one follow rule (part E0): the date follows only an appointment
    /// this device saw move: a device that has the task's new date before the moved
    /// event never writes the old one back; once a pull begun since went through
    /// whole and the event still says the old day, the copy here is current: it
    /// follows. F12: a file that cannot be written is not returned, and is tried
    /// again. The link keeps the instant it was set for (`AT`).
    #[test]
    fn a_date_follows_only_a_move_seen_here() {
        let desk = Desk::new("f4");
        desk.task("prep", TaskEdit::default());
        let (dir, state) = synced_visit("pairs-f4", None);
        let path = dir.join("visit.ics");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &refs_in(&dir), &any, &now()).unwrap().write().unwrap();
        assert!(read(Path::new(&task_of(&desk, "prep").key)).contains(&format!("{AT}=20261029T130000Z")), "the instant it is set for");
        let mut seen = crate::moves::Seen::default();
        let pass = |seen: &mut crate::moves::Seen| crate::moves::upkeep(&desk.tasks(), &refs_in(&dir), seen, &now());
        // First seen: noted, nothing written.
        assert!(pass(&mut seen).is_empty());
        // Another device moved it to 3 November and wrote the task: this device has the task first.
        let task = task_of(&desk, "prep");
        let theirs = followed(&task, &read(Path::new(&task.key)), &Appointment { start: "2026-11-03T13:00:00Z".parse::<jiff::Timestamp>().unwrap().as_second(), ..at("2026-11-03") }, &now()).unwrap();
        std::fs::write(&task.key, &theirs).unwrap();
        assert!(pass(&mut seen).is_empty(), "the old day never written back");
        assert_eq!(task_of(&desk, "prep").due, "2026-11-03");
        assert!(seen.doubt.keys().any(|k| k.starts_with("visit@")), "a doubt kept while it waits, by the instant it was set for");
        // The moved event comes: nothing more to write. Moved here again: written once.
        std::fs::write(&path, read_event("visit", "20261103T130000Z")).unwrap();
        assert!(pass(&mut seen).is_empty());
        assert!(seen.doubt.is_empty());
        std::fs::write(&path, read_event("visit", "20261110T130000Z")).unwrap();
        assert_eq!(pass(&mut seen).len(), 1);
        assert_eq!(task_of(&desk, "prep").due, "2026-11-10");
        assert!(pass(&mut seen).is_empty(), "the same twice");
        // A doubt again: the task says 20 November, the event 10; a pull begun after the doubt went through.
        let task = task_of(&desk, "prep");
        let ahead = followed(&task, &read(Path::new(&task.key)), &Appointment { start: "2026-11-20T13:00:00Z".parse::<jiff::Timestamp>().unwrap().as_second(), ..at("2026-11-20") }, &now()).unwrap();
        std::fs::write(&task.key, &ahead).unwrap();
        assert!(pass(&mut seen).is_empty(), "waits");
        let mut known = vdir::State::load(&state);
        known.pulled = Some(now().timestamp().as_second() + 60);
        known.save(&state).unwrap();
        assert_eq!(pass(&mut seen).len(), 1, "the copy here is current: it follows, once");
        assert_eq!(task_of(&desk, "prep").due, "2026-11-10");
        // F12: its folder read-only, the write fails: nothing returned, nothing tried again at once.
        // Unix alone: a folder's write permission is what fails the write there.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&path, read_event("visit", "20261112T130000Z")).unwrap();
            let list = desk.list.dir.clone();
            std::fs::set_permissions(&list, std::fs::Permissions::from_mode(0o555)).unwrap();
            let written = pass(&mut seen);
            std::fs::set_permissions(&list, std::fs::Permissions::from_mode(0o755)).unwrap();
            assert!(written.is_empty(), "{written:?}");
            // Check of 96b3cd8, item 5: the move not noted, so the next pass, the folder writable again, writes it.
            assert_eq!(pass(&mut seen).len(), 1);
            assert_eq!(task_of(&desk, "prep").due, "2026-11-12");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// E0.3 and E0.4: a follow writes the task as its file is now (a title changed
    /// since the list was read stays); tied again to the same appointment, from a
    /// copy of the event behind, neither its date nor its instant changes; tied to
    /// another, the old event's line naming the task goes in the same change.
    #[test]
    fn a_follow_and_a_re_tie_take_the_file_as_it_is() {
        let desk = Desk::new("fresh");
        desk.task("prep", TaskEdit::default());
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let stale = task_of(&desk, "prep");
        let renamed = tasks::apply(&read(Path::new(&stale.key)), &TaskEdit { title: "Renamed".into(), ..TaskEdit::of(&stale) }, &TimeZone::system(), &now()).unwrap();
        std::fs::write(&stale.key, &renamed).unwrap();
        let moved = followed(&stale, &renamed, &at("2026-11-03"), &now()).unwrap();
        assert_eq!(tasks::task_of_text(&moved, &TimeZone::system()).unwrap().title, "Renamed", "the title changed meanwhile stays");
        // A copy of the event behind (5 November here, as this device reads it): tied again, nothing written.
        let behind = read(&path).replace("20261029T13", "20261105T13");
        let behind_refs = vec![agenda::event_ref(&behind, &path.display().to_string(), &TimeZone::system()).unwrap()];
        assert!(attach(&desk.tasks(), "prep", &file(&path, &behind, true), &behind_refs, &any, &now()).unwrap().written.iter().all(|(p, _)| *p != PathBuf::from(&stale.key)), "neither its date nor its instant");
        // Tied to another event: the old one's line goes with it.
        let other = desk.event("other", "");
        let change = attach(&desk.tasks(), "prep", &file(&other, &read(&other), true), &desk.refs(), &any, &now()).unwrap();
        let old = change.written.iter().find(|(p, _)| *p == path).map(|(_, t)| t.clone()).expect("the old event's line out");
        assert!(!old.contains("sioul:task/prep"), "{old}");
        change.write().unwrap();
        assert_eq!(appointment_of(&task_of(&desk, "prep")).as_deref(), Some("other"));
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
        assert!(followed(&task, &read(Path::new(&task.key)), &Appointment { day: None, cancelled: false, recurring: false, start: 0 }, &now()).is_none());
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
    /// Unix alone: a folder's write permission is what cuts the write short there.
    #[cfg(unix)]
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

    /// F13: an appointment made a task: the tasks that prepared it keep no tie to a task;
    /// check of 96b3cd8, item 7: untied by the pair's rule, the date the tie replaced back.
    #[test]
    fn a_conversion_keeps_no_pair_link() {
        let task = "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:prep\r\nDUE;VALUE=DATE:20261029\r\nLINK;LINKREL=\"tag:aurelienpierre.com,2026:sioul/rel/appointment\";X-SIOUL-DUE=20261029;X-SIOUL-DUE-WAS=2026-11-30;VALUE=URI:sioul:event/visit\r\nLINK;LINKREL=related;VALUE=UID:visit\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let text = crate::appointments::retarget(task, "visit", "new-task", false, &now()).unwrap();
        let read = tasks::task_of_text(&text, &TimeZone::UTC).unwrap();
        assert_eq!((appointment_of(&read), read.due.as_str()), (None, "2026-11-30"));
        assert!(!text.contains("rel/appointment") && !text.contains(DUE_WAS) && text.contains("VALUE=UID:new-task"), "{text}");
    }

    /// Check of 96b3cd8, items 1 and 2: "Undo this link" in a Related list unties a
    /// pair by the ×'s rule: its date back, a meeting with others never written, the
    /// pair gone from both Related lists; and no message's tie goes into the meeting.
    #[test]
    fn undoing_the_link_unties_by_the_pair_s_rule() {
        let desk = Desk::new("undo-link");
        desk.task("prep", TaskEdit { due: "2026-11-30".into(), ..TaskEdit::default() });
        let path = desk.event("meet", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        assert_eq!(task_of(&desk, "prep").due, "2026-10-29");
        // Guests added since, its line kept by the editor.
        let with_guests = read(&path).replace("SUMMARY:Doctor", "SUMMARY:Doctor\r\nORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:jane@example.org");
        std::fs::write(&path, &with_guests).unwrap();
        let loaded = links::Loaded { tasks: desk.tasks(), events: desk.refs(), ..links::Loaded::default() };
        let undone = loaded.untie(&links::task_uri("prep"), &links::event_uri("meet"), &now());
        assert_eq!(undone.len(), 1, "the task alone, never the meeting: {undone:?}");
        assert_ne!(undone[0].path, path);
        let task = tasks::task_of_text(&undone[0].text, &TimeZone::system()).unwrap();
        assert_eq!((task.due.as_str(), appointment_of(&task), task.untied.clone()), ("2026-11-30", None, vec!["meet".to_string()]));
        std::fs::write(&undone[0].path, &undone[0].text).unwrap();
        let loaded = links::Loaded { tasks: desk.tasks(), events: desk.refs(), ..links::Loaded::default() };
        let edges = loaded.world().edges();
        assert!(!edges.iter().any(|e| e.how == "appointment"), "gone from both Related lists: {edges:?}");
        assert!(loaded.tie("mid:agenda@example.org", &links::event_uri("meet"), &now()).is_none(), "the file of ties holds it, not the meeting");
    }

    /// Check of 96b3cd8, item 3: never a DUE a server refuses. A task that starts on
    /// 27 October, whose appointment moves to 26 October, is due on its start day;
    /// one whose start has a time gets no DUE at all, nor a day to follow.
    #[test]
    fn never_a_due_before_the_start_nor_of_another_kind() {
        let desk = Desk::new("murena");
        desk.task("prep", TaskEdit { start: "2026-10-27".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let task = task_of(&desk, "prep");
        assert_eq!(task.due, "2026-10-29");
        let moved = followed(&task, &read(Path::new(&task.key)), &at("2026-10-26"), &now()).unwrap();
        std::fs::write(&task.key, &moved).unwrap();
        let task = task_of(&desk, "prep");
        assert_eq!((task.start.as_str(), task.due.as_str()), ("2026-10-27", "2026-10-27"));
        // Moved after its start again: it follows.
        let back = followed(&task, &moved, &at("2026-10-30"), &now()).unwrap();
        assert_eq!(tasks::task_of_text(&back, &TimeZone::system()).unwrap().due, "2026-10-30");
        // Tied when it starts after the appointment: due on its start day.
        desk.task("late", TaskEdit { start: "2026-10-31".into(), ..TaskEdit::default() });
        let other = desk.event("visit-late", "");
        attach(&desk.tasks(), "late", &file(&other, &read(&other), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        assert_eq!(task_of(&desk, "late").due, "2026-10-31");
        // A start with a time (another program's): no DUE, no day written to follow.
        desk.task("timed", TaskEdit { start: "2026-10-27T09:00".into(), ..TaskEdit::default() });
        let third = desk.event("visit-timed", "");
        attach(&desk.tasks(), "timed", &file(&third, &read(&third), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let timed = task_of(&desk, "timed");
        assert_eq!((timed.due.as_str(), appointment_of(&timed).as_deref()), ("", Some("visit-timed")));
        assert!(!read(Path::new(&timed.key)).contains(DUE_WRITTEN), "{}", read(Path::new(&timed.key)));
        assert!(followed(&timed, &read(Path::new(&timed.key)), &at("2026-11-03"), &now()).is_none());
    }

    /// Check of 60ad872, item 1: the date the tie replaced comes back in the start's
    /// form. Due 30 November at 17:00, tied to an appointment on 29 October, then
    /// given a start of 27 October: moved to 5 December, and untied, its date comes
    /// back as a day beside the start's day, never `DUE:…Z` beside `DTSTART;VALUE=DATE`.
    #[test]
    fn a_date_put_back_takes_the_start_s_form() {
        let zone = TimeZone::system();
        let desk = Desk::new("put-back");
        desk.task("prep", TaskEdit { due: "2026-11-30T17:00".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let task = task_of(&desk, "prep");
        assert_eq!(task.due, "2026-10-29");
        let started = tasks::apply(&read(Path::new(&task.key)), &TaskEdit { start: "2026-10-27".into(), ..TaskEdit::of(&task) }, &zone, &now()).unwrap();
        std::fs::write(&task.key, &started).unwrap();
        let task = task_of(&desk, "prep");
        let date_lines = |text: &str| crate::lines::unfold(text).into_iter().filter(|l| l.starts_with("DUE") || l.starts_with("DTSTART")).collect::<Vec<_>>();
        let moved = followed(&task, &started, &at("2026-12-05"), &now()).unwrap();
        assert_eq!(date_lines(&moved), ["DTSTART;VALUE=DATE:20261027", "DUE;VALUE=DATE:20261130"], "{moved}");
        let change = detach(&task, Some(&file(&path, &read(&path), true)), &now()).unwrap();
        let untied = &change.written.iter().find(|(p, _)| *p == PathBuf::from(&task.key)).unwrap().1;
        assert_eq!(date_lines(untied), ["DTSTART;VALUE=DATE:20261027", "DUE;VALUE=DATE:20261130"], "{untied}");
        // Check of c792bd7, item 7: given back beside its start, it follows on; tied again as it is, nothing is
        // written; tied to a second event, that one's day is written.
        std::fs::write(&task.key, &moved).unwrap();
        let given = task_of(&desk, "prep");
        let earlier = followed(&given, &moved, &at("2026-11-20"), &now()).unwrap();
        assert_eq!(date_lines(&earlier), ["DTSTART;VALUE=DATE:20261027", "DUE;VALUE=DATE:20261120"], "{earlier}");
        // The appointment moved there, its line kept.
        std::fs::write(&path, read(&path).replace("20261029T13", "20261205T13")).unwrap();
        assert!(attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().is_empty(), "a date given back, tied again: nothing");
        let second = desk.calendar.dir.join("second.ics");
        std::fs::write(&second, read_event("second", "20261103T130000Z")).unwrap();
        attach(&desk.tasks(), "prep", &file(&second, &read(&second), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let tied = task_of(&desk, "prep");
        assert_eq!((tied.due.as_str(), appointment_of(&tied).as_deref()), ("2026-11-03", Some("second")));
        // A start with a time: no date put back beside it.
        let timed = Task { start: "2026-10-27T09:00".into(), ..task.clone() };
        assert_eq!(put_back(&timed, "2026-11-30T17:00"), "");
        // No start: as kept; before the start day: that day.
        assert_eq!(put_back(&Task { start: String::new(), ..task.clone() }, "2026-11-30T17:00"), "2026-11-30T17:00");
        assert_eq!(put_back(&task, "2026-10-20"), "2026-10-27");
    }

    /// Check of c792bd7, item 7: a re-tie keeps the replaced date following. Due 30
    /// November, tied to 29 October, tied again to 5 December, which moves to 20
    /// November: due 20 November; a third tie writes nothing.
    #[test]
    fn a_re_tie_keeps_the_replaced_date_following() {
        let desk = Desk::new("re-tie");
        desk.task("prep", TaskEdit { due: "2026-11-30".into(), ..TaskEdit::default() });
        let first = desk.event("first", "");
        attach(&desk.tasks(), "prep", &file(&first, &read(&first), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        assert_eq!(task_of(&desk, "prep").due, "2026-10-29");
        let later = desk.calendar.dir.join("later.ics");
        std::fs::write(&later, read_event("later", "20261205T130000Z")).unwrap();
        attach(&desk.tasks(), "prep", &file(&later, &read(&later), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let task = task_of(&desk, "prep");
        assert_eq!(task.due, "2026-11-30", "its own date back, earlier than 5 December");
        let moved = followed(&task, &read(Path::new(&task.key)), &at("2026-11-20"), &now()).unwrap();
        std::fs::write(&task.key, &moved).unwrap();
        assert_eq!(task_of(&desk, "prep").due, "2026-11-20");
        // The appointment moved there, its line kept.
        std::fs::write(&later, read(&later).replace("DTSTART:20261205T130000Z", "DTSTART:20261120T130000Z")).unwrap();
        assert!(attach(&desk.tasks(), "prep", &file(&later, &read(&later), true), &desk.refs(), &any, &now()).unwrap().is_empty(), "a third tie: nothing written");
    }

    /// Check of 60ad872, item 3: a date of your own the tie never replaced stays yours:
    /// due 20 November, or 20 November at 09:00, tied to an appointment on 29
    /// November that then moves to 15 November: nothing is written for either.
    #[test]
    fn a_date_of_your_own_the_tie_never_replaced_stays_yours() {
        let desk = Desk::new("yours");
        for (uid, due) in [("plain", "2026-11-20"), ("timed", "2026-11-20T09:00")] {
            desk.task(uid, TaskEdit { due: due.into(), ..TaskEdit::default() });
            let event = desk.calendar.dir.join(format!("{uid}-visit.ics"));
            std::fs::write(&event, read_event(&format!("{uid}-visit"), "20261129T130000Z")).unwrap();
            attach(&desk.tasks(), uid, &file(&event, &read(&event), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
            let task = task_of(&desk, uid);
            let text = read(Path::new(&task.key));
            assert_eq!(task.due, due);
            assert!(!text.contains(DUE_WRITTEN), "neither the day written nor the date replaced: {text}");
            assert!(followed(&task, &text, &at("2026-11-15"), &now()).is_none(), "{uid}: nothing written");
            assert!(attach(&desk.tasks(), uid, &file(&event, &read(&event), true), &desk.refs(), &any, &now()).unwrap().is_empty(), "{uid}: tied again, nothing written");
        }
    }

    /// Check of 60ad872, items 2 and 5: Add ▸ Note writes the event's tie only where
    /// a tie may go: a meeting with others and an event of a calendar you only read
    /// stay byte for byte as they were; a tie an older build left in a meeting is
    /// said to be kept there, a pair's own line is not.
    #[test]
    fn a_note_s_tie_goes_into_no_meeting_nor_a_calendar_you_only_read() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
        let desk = Desk::new("note-ties");
        let meeting = desk.event("meeting", "ORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:jane@example.org\r\n");
        let plain = desk.event("plain", "");
        let account = "note-ties-read-only";
        vdir::prepare(Kind::Calendars, account, "holidays", "Holidays", None).unwrap();
        let state = vdir::state_path(account, Kind::Calendars, "holidays");
        let mut read_only_state = vdir::State::load(&state);
        read_only_state.read_only = true;
        read_only_state.save(&state).unwrap();
        let holidays = vdir::collections(Kind::Calendars).into_iter().find(|c| c.account == account).unwrap();
        let holiday = holidays.dir.join("holiday.ics");
        std::fs::write(&holiday, read_event("holiday", "20261111T000000Z")).unwrap();
        let before = [read(&meeting), read(&holiday)];
        let mut events = desk.refs();
        events.push(agenda::event_ref(&read(&holiday), &holiday.display().to_string(), &TimeZone::system()).unwrap());
        let loaded = links::Loaded { events, ..links::Loaded::default() };
        assert!(loaded.tie_into_event("meeting", "sioul:note/notes/meeting.md", &now()).is_none());
        assert!(loaded.tie_into_event("holiday", "sioul:note/notes/holiday.md", &now()).is_none());
        assert_eq!([read(&meeting), read(&holiday)], before, "byte for byte");
        let into_plain = loaded.tie_into_event("plain", "sioul:note/notes/plain.md", &now()).expect("a plain event of yours takes it");
        assert!(into_plain.path == plain && into_plain.text.contains("LINK;LINKREL=describedby;VALUE=URI:sioul:note/notes/plain.md"), "{}", into_plain.text);
        // A tie an older build wrote into the meeting: kept there, and said so.
        let old_tie = read(&meeting).replace("SUMMARY:Doctor", "SUMMARY:Doctor\r\nLINK;LINKREL=describedby;VALUE=URI:sioul:note/notes/meeting.md\r\nLINK;LINKREL=\"tag:aurelienpierre.com,2026:sioul/rel/preparation\";VALUE=URI:sioul:task/prep");
        std::fs::write(&meeting, &old_tie).unwrap();
        assert!(loaded.kept_in_meeting("sioul:note/notes/meeting.md", &links::event_uri("meeting")));
        assert!(!loaded.kept_in_meeting(&links::task_uri("prep"), &links::event_uri("meeting")), "a pair's line: the mark undoes it");
        assert!(!loaded.kept_in_meeting("sioul:note/notes/plain.md", &links::event_uri("plain")));
    }

    /// Check of 96b3cd8, item 9: an appointment of an account switched off is here
    /// for parking, a copy the pages show wins, and the pages' events alone move a date.
    #[test]
    fn appointments_of_accounts_switched_off_are_here() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
        let zone = TimeZone::system();
        let off = Kind::Calendars.root().join("pairs-off-test");
        std::fs::create_dir_all(off.join("agenda")).unwrap();
        std::fs::write(off.join("agenda/away.ics"), read_event("away", "20261103T130000Z")).unwrap();
        std::fs::write(off.join("agenda/both.ics"), read_event("both", "20261104T130000Z")).unwrap();
        vdir::set_account_off("pairs-off-test", true);
        let shown = EventRef { uid: "both".into(), start: "2026-11-10T09:00:00Z".parse::<jiff::Timestamp>().unwrap().as_second(), ..EventRef::default() };
        let here = appointments_here(std::slice::from_ref(&shown), &zone);
        assert_eq!(here.get("away").and_then(|a| a.day), day("2026-11-03"));
        assert_eq!(here["both"].day, day("2026-11-10"), "the pages' copy wins");
        assert!(switched_off_events(&zone).iter().any(|e| e.uid == "away"));
        // Read once while the folder stays as it is; again once it changes.
        std::fs::remove_file(off.join("agenda/away.ics")).unwrap();
        assert!(!appointments_here(&[], &zone).contains_key("away"));
        vdir::set_account_off("pairs-off-test", false);
        let _ = std::fs::remove_dir_all(&off);
    }

    /// What closes by itself (§7.5): an appointment over, on a calendar that
    /// does not sync, sets aside its steps before it (`CANCELLED`, marked), keeps
    /// the steps after it; with none after, the task closes, set aside unless
    /// every step was done. Stamped with the appointment's end: another device,
    /// later, writes the same bytes. One to come, or on a calendar whose pull did
    /// not go through since its end, closes nothing (review of ab7daa4, M1).
    #[test]
    fn what_closes_by_itself_once_the_appointment_is_over() {
        let from = now().timestamp().as_second();
        let later: Zoned = "2026-10-29T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        let desk = Desk::new("closing");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        desk.task("done-one", TaskEdit { parent: "prep".into(), status: Status::Completed, ..TaskEdit::default() });
        desk.task("follow-up", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let after = crate::anchors::step_side(&read(&desk.list.dir.join("follow-up.ics")), true, &now()).unwrap();
        std::fs::write(desk.list.dir.join("follow-up.ics"), after).unwrap();
        // Before the appointment's end: nothing.
        assert!(closing(&desk.tasks(), &desk.refs(), from, &now()).is_empty());
        let first = closing(&desk.tasks(), &desk.refs(), from, &later);
        let elsewhere: Zoned = "2026-10-30T09:00:00+01:00[Europe/Paris]".parse().unwrap();
        assert_eq!(first.written, closing(&desk.tasks(), &desk.refs(), from, &elsewhere).written, "the same bytes on another device, later");
        first.write().unwrap();
        let status = |uid: &str| task_of(&desk, uid).status;
        assert_eq!([status("early"), status("done-one"), status("follow-up"), status("prep")], [Status::Cancelled, Status::Completed, Status::NeedsAction, Status::NeedsAction], "the one before set aside, the one after kept");
        assert_eq!(task_of(&desk, "early").set_aside, "visit");
        assert!(closing(&desk.tasks(), &desk.refs(), from, &later).is_empty(), "twice: nothing");
        // The step after done: no step left, the task closes, set aside (one was not done).
        let done = tasks::set_status(&read(&desk.list.dir.join("follow-up.ics")), Status::Completed, &TimeZone::system(), &later).unwrap();
        std::fs::write(desk.list.dir.join("follow-up.ics"), done).unwrap();
        closing(&desk.tasks(), &desk.refs(), from, &later).write().unwrap();
        assert_eq!(status("prep"), Status::Cancelled);
        // A calendar that syncs, its pull not gone through since the end: nothing closes.
        let other = Desk::new("closing-synced");
        other.task("prep2", TaskEdit::default());
        other.task("step2", TaskEdit { parent: "prep2".into(), ..TaskEdit::default() });
        let (dir, _) = synced_visit("pairs-closing", Some(now().timestamp().as_second()));
        let visit = dir.join("visit.ics");
        // Over at 13:30 UTC on 29 October, as the first one.
        std::fs::write(&visit, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:visit\r\nDTSTAMP:20261020T100000Z\r\nDTSTART:20261029T130000Z\r\nDTEND:20261029T133000Z\r\nSUMMARY:Doctor\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let mut refs = refs_in(&dir);
        attach(&other.tasks(), "prep2", &file(&visit, &read(&visit), true), &refs, &any, &now()).unwrap().write().unwrap();
        refs = refs_in(&dir);
        assert!(closing(&other.tasks(), &refs, from, &later).is_empty(), "an older copy closes nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One prepared appointment on 29 October, 13:00–13:30 UTC, with one step;
    /// `event` its lines beyond the usual, or its whole text; `step` the step's
    /// own; `after_load` a change to the step's file once the tasks were read.
    /// Whether `closing` sets the step aside, the appointment over.
    fn sets_aside(name: &str, event: Result<&str, &str>, step: TaskEdit, after_load: Option<&dyn Fn(&str) -> String>, from: i64) -> bool {
        let desk = Desk::new(name);
        desk.task("prep", TaskEdit::default());
        desk.task("step", TaskEdit { parent: "prep".into(), ..step });
        let path = match event {
            Ok(lines) => desk.event("visit", lines),
            Err(text) => {
                let p = desk.calendar.dir.join("visit.ics");
                std::fs::write(&p, text).unwrap();
                p
            }
        };
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let loaded = desk.tasks();
        if let Some(change) = after_load {
            let step = desk.list.dir.join("step.ics");
            std::fs::write(&step, change(&read(&step))).unwrap();
        }
        let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&loaded, &desk.refs(), from, &later).written.iter().any(|(p, _)| p.ends_with("step.ics"))
    }

    fn closing_from() -> i64 {
        now().timestamp().as_second()
    }

    /// What closes by itself sets aside a plain step before an appointment over
    /// (the guards' tests below each keep one follow-up or one appointment).
    #[test]
    fn a_plain_step_before_is_set_aside() {
        assert!(sets_aside("aside-plain", Ok(""), TaskEdit::default(), None, closing_from()));
    }

    /// The past at the install (review of ab7daa4, M1): over before this device first closed anything.
    #[test]
    fn the_past_at_the_install_is_never_set_aside() {
        assert!(!sets_aside("aside-past", Ok(""), TaskEdit::default(), None, "2026-10-29T15:00:00+01:00[Europe/Paris]".parse::<Zoned>().unwrap().timestamp().as_second()));
    }

    #[test]
    fn a_whole_day_sets_nothing_aside() {
        let whole = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:visit\r\nDTSTAMP:20261020T100000Z\r\nDTSTART;VALUE=DATE:20261029\r\nDTEND;VALUE=DATE:20261030\r\nSUMMARY:Doctor\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert!(!sets_aside("aside-whole-day", Err(whole), TaskEdit::default(), None, closing_from()));
    }

    /// A meeting with others, whose time may have moved in a message not answered yet.
    #[test]
    fn a_meeting_with_others_sets_nothing_aside() {
        assert!(!sets_aside("aside-meeting", Ok("ORGANIZER:mailto:chair@example.org\r\nATTENDEE:mailto:jane@example.org\r\n"), TaskEdit::default(), None, closing_from()));
    }

    #[test]
    fn an_appointment_that_repeats_sets_nothing_aside() {
        assert!(!sets_aside("aside-repeating", Ok("RRULE:FREQ=WEEKLY;COUNT=2\r\n"), TaskEdit::default(), None, closing_from()));
    }

    #[test]
    fn an_appointment_cancelled_sets_nothing_aside() {
        assert!(!sets_aside("aside-cancelled", Ok("STATUS:CANCELLED\r\n"), TaskEdit::default(), None, closing_from()));
    }

    /// Follow-ups by the plan's rule: a step with a start, a date of its own on
    /// or after the day, a time block, or said to come after once the tasks were read.
    #[test]
    fn a_step_with_a_start_stays() {
        assert!(!sets_aside("aside-start", Ok(""), TaskEdit { start: "2026-11-05".into(), ..TaskEdit::default() }, None, closing_from()));
    }

    #[test]
    fn a_step_with_its_own_later_date_stays() {
        assert!(!sets_aside("aside-due", Ok(""), TaskEdit { due: "2026-11-05".into(), ..TaskEdit::default() }, None, closing_from()));
    }

    #[test]
    fn a_step_with_a_time_block_stays() {
        assert!(!sets_aside("aside-block", Ok(""), TaskEdit { links: vec![crate::blocks::task_link("step-block")], ..TaskEdit::default() }, None, closing_from()));
    }

    #[test]
    fn a_step_said_after_since_the_read_stays() {
        let mark = |text: &str| crate::anchors::step_side(text, true, &now()).unwrap();
        assert!(!sets_aside("aside-after", Ok(""), TaskEdit::default(), Some(&mark), closing_from()));
    }

    /// A step added once the appointment was over is a follow-up (review of
    /// 4d7bab4, M-a), even put first among the steps by hand (its place older).
    #[test]
    fn a_step_made_once_the_appointment_was_over_stays() {
        let made: Zoned = "2026-10-29T15:00:00+01:00[Europe/Paris]".parse().unwrap();
        let new_step = |_: &str| tasks::new_task(&TaskEdit { title: "Book the follow-up visit".into(), parent: "prep".into(), sort_order: Some(1), ..TaskEdit::default() }, "step", &TimeZone::system(), &made).unwrap();
        assert!(!sets_aside("aside-made-after", Ok(""), TaskEdit::default(), Some(&new_step), closing_from()));
    }

    /// A task made a step of the preparation once the appointment was over
    /// keeps its CREATED; its place says when it joined (review of 4d7bab4, later 9).
    #[test]
    fn a_task_made_a_step_once_the_appointment_was_over_stays() {
        let desk = Desk::new("aside-demoted");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        desk.task("loose", TaskEdit::default());
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let made: Zoned = "2026-10-29T15:00:00+01:00[Europe/Paris]".parse().unwrap();
        crate::composed::demote(&desk.tasks(), "loose", "prep", std::slice::from_ref(&desk.list), &|_| crate::capabilities::Provider::Dav, true, &made).unwrap().write().unwrap();
        assert!(task_of(&desk, "loose").created < 1_793_280_600, "made before the end");
        let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        let written: Vec<PathBuf> = closing(&desk.tasks(), &desk.refs(), closing_from(), &later).written.into_iter().map(|(p, _)| p).collect();
        assert!(written.iter().any(|p| p.ends_with("early.ics")), "the step before set aside");
        assert!(!written.iter().any(|p| p.ends_with("loose.ics")), "the one made a step since stays: {written:?}");
        // Then dragged to the top of the steps: its place older than the end, its change not
        // (review of 468a123, §3.1).
        let steps = desk.tasks();
        let ordered: Vec<&Task> = tasks::steps_of(&steps, "prep");
        let moved: Zoned = "2026-10-29T15:05:00+01:00[Europe/Paris]".parse().unwrap();
        for (uid, place) in tasks::reorder(&ordered, "loose", Some("early")).unwrap() {
            let key = desk.list.dir.join(format!("{uid}.ics"));
            std::fs::write(&key, tasks::set_sequence(&read(&key), place, &moved).unwrap()).unwrap();
        }
        assert!(task_of(&desk, "loose").placed_at().is_some_and(|placed| placed < 1_793_280_600), "its place now older than the end");
        let written: Vec<PathBuf> = closing(&desk.tasks(), &desk.refs(), closing_from(), &later).written.into_iter().map(|(p, _)| p).collect();
        assert!(!written.iter().any(|p| p.ends_with("loose.ics")), "moved to the top since, it stays: {written:?}");
    }

    /// A step ticked before the appointment and opened again once it was
    /// over stays: changed after the end (review of 468a123, §3.1).
    #[test]
    fn a_step_opened_again_once_the_appointment_was_over_stays() {
        let opened: Zoned = "2026-10-29T15:00:00+01:00[Europe/Paris]".parse().unwrap();
        let open_again = |text: &str| tasks::set_status(text, Status::NeedsAction, &TimeZone::system(), &opened).unwrap();
        assert!(!sets_aside("aside-opened-again", Ok(""), TaskEdit { status: Status::Completed, ..TaskEdit::default() }, Some(&open_again), closing_from()));
    }

    /// A step whose own steps are open stays, with them (review of 4d7bab4,
    /// later 11): they would be laid as steps of a step set aside.
    #[test]
    fn a_step_with_open_steps_of_its_own_stays() {
        let desk = Desk::new("aside-nested");
        desk.task("prep", TaskEdit::default());
        desk.task("forms", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        desk.task("form-a", TaskEdit { parent: "forms".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        assert!(closing(&desk.tasks(), &desk.refs(), closing_from(), &later).is_empty());
    }

    /// A preparation pinned to a time keeps its block: it stays open, its
    /// steps before set aside (review of 4d7bab4, later 4).
    #[test]
    fn a_preparation_with_a_time_block_stays_open() {
        // Its block to come (2 November): it stays open; over (29 October, 10:00 UTC), it closes
        // (review of 468a123, §3.2).
        for (name, block_at, stays) in [("aside-pinned", "20261102T090000Z", true), ("aside-pinned-over", "20261029T100000Z", false)] {
            let desk = Desk::new(name);
            desk.task("prep", TaskEdit { links: vec![crate::blocks::task_link("prep-block")], ..TaskEdit::default() });
            desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
            let path = desk.event("visit", "");
            std::fs::write(desk.calendar.dir.join("prep-block.ics"), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:prep-block\r\nDTSTAMP:20261020T100000Z\r\nDTSTART:{block_at}\r\nDURATION:PT30M\r\nSUMMARY:prep\r\nX-SIOUL-TASK:prep\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")).unwrap();
            attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
            let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
            closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
            let prep = if stays { Status::NeedsAction } else { Status::Cancelled };
            assert_eq!((task_of(&desk, "early").status, task_of(&desk, "prep").status), (Status::Cancelled, prep), "{name}");
        }
    }

    /// A preparation pinned to a block beyond the events read (three months
    /// ahead, in the calendar for blocks) stays open: the block's own file is
    /// read (review of eb6f660, later 4).
    #[test]
    fn a_block_months_ahead_keeps_its_preparation_open() {
        let desk = Desk::new("aside-pinned-far");
        let planned = vdir::create(Kind::Calendars, vdir::LOCAL, "aside-pinned-far blocks", None, &["VEVENT"]).unwrap();
        desk.task("prep", TaskEdit { links: vec![crate::blocks::task_link("far-block")], ..TaskEdit::default() });
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        std::fs::write(planned.dir.join("far-block.ics"), "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:far-block\r\nDTSTAMP:20261020T100000Z\r\nDTSTART:20270129T090000Z\r\nDURATION:PT30M\r\nSUMMARY:prep\r\nX-SIOUL-TASK:prep\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        assert!(!desk.refs().iter().any(|e| e.uid == "far-block"), "not among the events read");
        closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
        assert_eq!((task_of(&desk, "early").status, task_of(&desk, "prep").status), (Status::Cancelled, Status::NeedsAction));
        let _ = std::fs::remove_dir_all(&planned.dir);
    }

    /// Done is never "set aside" (review of 4d7bab4, later 1): a task the
    /// closing completes carries no mark, and a step set aside then done
    /// elsewhere leaves the card's list.
    #[test]
    fn what_is_done_is_never_listed_as_set_aside() {
        let desk = Desk::new("aside-done");
        desk.task("prep", TaskEdit::default());
        desk.task("done-one", TaskEdit { parent: "prep".into(), status: Status::Completed, ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
        let prep = task_of(&desk, "prep");
        assert_eq!((prep.status, prep.set_aside.as_str()), (Status::Completed, ""), "done, no mark");
        // A step set aside, then ticked done by an agent (its mark kept): not on the card.
        let other = Desk::new("aside-done-later");
        other.task("prep", TaskEdit::default());
        other.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        other.task("after", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let visit = other.event("visit", "");
        attach(&other.tasks(), "prep", &file(&visit, &read(&visit), true), &other.refs(), &any, &now()).unwrap().write().unwrap();
        let key = other.list.dir.join("after.ics");
        std::fs::write(&key, crate::anchors::step_side(&read(&key), true, &now()).unwrap()).unwrap();
        closing(&other.tasks(), &other.refs(), closing_from(), &later).write().unwrap();
        let early = other.list.dir.join("early.ics");
        std::fs::write(&early, tasks::set_status(&read(&early), Status::Completed, &TimeZone::system(), &later).unwrap()).unwrap();
        assert_eq!(task_of(&other, "early").set_aside, "visit", "its mark kept by the tick");
        assert!(brought_back(&read(&early), &later).is_none(), "done: nothing to bring back");
        let o = agenda::Occurrence { key: visit.display().to_string(), uid: "visit".into(), summary: "Doctor".into(), start: 1_793_278_800, end: 1_793_280_600, ..agenda::Occurrence::default() };
        let tr = crate::i18n::Translator::new("en");
        assert!(appointment_view(&o, &[], &other.tasks(), &[], None, &tr, &TimeZone::system()).set_aside.is_empty());
    }

    /// Reopened by any other way than Bring back (an agent, another
    /// program), a step or a task that carries a mark counts as brought back
    /// (review of 4d7bab4, later 2).
    #[test]
    fn what_was_reopened_elsewhere_is_never_set_aside_again() {
        let desk = Desk::new("aside-reopened");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
        let reopen = |uid: &str| {
            let key = desk.list.dir.join(format!("{uid}.ics"));
            std::fs::write(&key, tasks::set_status(&read(&key), Status::NeedsAction, &TimeZone::system(), &later).unwrap()).unwrap();
        };
        reopen("prep");
        assert!(closing(&desk.tasks(), &desk.refs(), closing_from(), &later).is_empty(), "the task reopened, its mark on it");
        reopen("early");
        assert_eq!(task_of(&desk, "early").set_aside, "visit");
        assert!(closing(&desk.tasks(), &desk.refs(), closing_from(), &later).is_empty(), "the step reopened, its mark on it");
        // Its task kept open by a step after it: the step reopened alone stays open too.
        let open = Desk::new("aside-reopened-step");
        open.task("prep", TaskEdit::default());
        open.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        open.task("later-on", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let visit = open.event("visit", "");
        attach(&open.tasks(), "prep", &file(&visit, &read(&visit), true), &open.refs(), &any, &now()).unwrap().write().unwrap();
        let after = open.list.dir.join("later-on.ics");
        std::fs::write(&after, crate::anchors::step_side(&read(&after), true, &now()).unwrap()).unwrap();
        closing(&open.tasks(), &open.refs(), closing_from(), &later).write().unwrap();
        let early = open.list.dir.join("early.ics");
        std::fs::write(&early, tasks::set_status(&read(&early), Status::NeedsAction, &TimeZone::system(), &later).unwrap()).unwrap();
        assert_eq!((task_of(&open, "early").status, task_of(&open, "prep").status), (Status::NeedsAction, Status::NeedsAction));
        assert!(closing(&open.tasks(), &open.refs(), closing_from(), &later).is_empty(), "the step reopened elsewhere, never set aside again");
    }

    /// An appointment moved later once over gives back what it set aside
    /// (review of 4d7bab4, M-b): open again, unmarked, off its card, the same
    /// bytes on another device; its end unchanged, nothing is written; over
    /// again, set aside again, once.
    #[test]
    fn an_appointment_moved_later_gives_back_what_it_set_aside() {
        let desk = Desk::new("aside-moved-later");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-29T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
        assert_eq!((task_of(&desk, "early").status, task_of(&desk, "early").set_aside_end), (Status::Cancelled, 1_793_280_600), "set aside, the end kept");
        let evening: Zoned = "2026-10-29T18:00:00+01:00[Europe/Paris]".parse().unwrap();
        assert!(closing(&desk.tasks(), &desk.refs(), closing_from(), &evening).is_empty(), "its end unchanged: nothing");
        // At 18:00, moved to 5 November.
        let text = read(&path).replace("DTSTART:20261029T130000Z", "DTSTART:20261105T130000Z").replace("DTEND:20261029T133000Z", "DTEND:20261105T133000Z").replace("DTSTAMP:20261020T100000Z", "DTSTAMP:20261029T170000Z\r\nLAST-MODIFIED:20261029T170000Z");
        std::fs::write(&path, text).unwrap();
        let back = closing(&desk.tasks(), &desk.refs(), closing_from(), &evening);
        let elsewhere: Zoned = "2026-10-30T08:00:00+01:00[Europe/Paris]".parse().unwrap();
        assert_eq!(back.written, closing(&desk.tasks(), &desk.refs(), closing_from(), &elsewhere).written, "the same bytes on another device, later");
        // The appointment's own stamps rewritten by a server on the way: the same bytes still
        // (stamped with the set-aside's END, review of 468a123, §3.1).
        let moved = read(&path);
        std::fs::write(&path, moved.replace("LAST-MODIFIED:20261029T170000Z", "LAST-MODIFIED:20261029T170512Z")).unwrap();
        assert_eq!(back.written, closing(&desk.tasks(), &desk.refs(), closing_from(), &elsewhere).written, "whatever the server did to its stamps");
        std::fs::write(&path, moved).unwrap();
        back.write().unwrap();
        for uid in ["early", "prep"] {
            let task = task_of(&desk, uid);
            assert_eq!((task.status, task.set_aside.as_str()), (Status::NeedsAction, ""), "{uid} given back");
        }
        // Its date follows the appointment in the same write, not one pull late (review of 468a123, §3.7).
        let prep = task_of(&desk, "prep");
        assert!(prep.due.starts_with("2026-11-05"), "{}", prep.due);
        assert_eq!(tie_of(&read(&desk.list.dir.join("prep.ics"))).and_then(|t| t.at), Some(1_793_883_600), "set for the appointment's start"); 
        let o = agenda::Occurrence { key: path.display().to_string(), uid: "visit".into(), summary: "Doctor".into(), start: 1_793_883_600, end: 1_793_885_400, ..agenda::Occurrence::default() };
        let tr = crate::i18n::Translator::new("en");
        assert!(appointment_view(&o, &[], &desk.tasks(), &[], None, &tr, &TimeZone::system()).set_aside.is_empty(), "off its card");
        assert!(closing(&desk.tasks(), &desk.refs(), closing_from(), &evening).is_empty(), "given back once");
        // Over again: set aside again, with its new end, once.
        let after: Zoned = "2026-11-05T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &after).write().unwrap();
        assert_eq!((task_of(&desk, "early").status, task_of(&desk, "early").set_aside_end), (Status::Cancelled, 1_793_885_400));
        assert!(closing(&desk.tasks(), &desk.refs(), closing_from(), &after).is_empty());
    }

    /// A step set aside is never given back under its task done since (review
    /// of 468a123, §3.5): the steps after the appointment ticked, the task
    /// done, the appointment moved later: the step stays set aside.
    #[test]
    fn a_step_is_never_given_back_under_a_task_done() {
        let desk = Desk::new("aside-under-done");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        desk.task("later-on", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let after = desk.list.dir.join("later-on.ics");
        std::fs::write(&after, crate::anchors::step_side(&read(&after), true, &now()).unwrap()).unwrap();
        let later: Zoned = "2026-10-29T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
        assert_eq!((task_of(&desk, "early").status, task_of(&desk, "prep").status), (Status::Cancelled, Status::NeedsAction));
        // The step after it ticked, the task done with it.
        for uid in ["later-on", "prep"] {
            let key = desk.list.dir.join(format!("{uid}.ics"));
            std::fs::write(&key, tasks::set_status(&read(&key), Status::Completed, &TimeZone::system(), &later).unwrap()).unwrap();
        }
        let evening: Zoned = "2026-10-29T18:00:00+01:00[Europe/Paris]".parse().unwrap();
        std::fs::write(&path, read(&path).replace("DTSTART:20261029T130000Z", "DTSTART:20261105T130000Z").replace("DTEND:20261029T133000Z", "DTEND:20261105T133000Z")).unwrap();
        let back = closing(&desk.tasks(), &desk.refs(), closing_from(), &evening);
        assert!(!back.written.iter().any(|(p, _)| p.ends_with("early.ics")), "never an open step under a task done: {:?}", back.written);
    }

    /// The give-back reads its files only when the appointment, as read,
    /// ends later than the mark's end (review of 468a123, §3.8): one moved in
    /// its file but not read again gives back at the next read.
    #[test]
    fn a_give_back_waits_for_the_appointment_read_again() {
        let desk = Desk::new("aside-read-again");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-29T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
        let read_before = desk.refs();
        std::fs::write(&path, read(&path).replace("DTSTART:20261029T130000Z", "DTSTART:20261105T130000Z").replace("DTEND:20261029T133000Z", "DTEND:20261105T133000Z")).unwrap();
        let evening: Zoned = "2026-10-29T18:00:00+01:00[Europe/Paris]".parse().unwrap();
        assert!(closing(&desk.tasks(), &read_before, closing_from(), &evening).is_empty(), "as read, not moved: no file read, nothing");
        assert!(!closing(&desk.tasks(), &desk.refs(), closing_from(), &evening).is_empty(), "read again: given back");
    }

    /// Bringing back a step reopens its task set aside with it (the same
    /// mark), never one set aside by another, nor one that is open; a task
    /// not set aside brings nothing (review of 468a123, §3.8: a set-aside row
    /// ticked open goes this way).
    #[test]
    fn bringing_back_reopens_its_task_set_aside_with_it() {
        let desk = Desk::new("aside-bring-both");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-29T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
        assert!(is_set_aside(&task_of(&desk, "early")) && is_set_aside(&task_of(&desk, "prep")));
        let written: Vec<PathBuf> = bringing_back(&desk.tasks(), "early", &later).into_iter().map(|(p, _)| p).collect();
        assert_eq!(written, [desk.list.dir.join("early.ics"), desk.list.dir.join("prep.ics")], "the step and its task");
        assert!(bringing_back(&desk.tasks(), "nowhere", &later).is_empty());
        let mut all = desk.tasks();
        all.iter_mut().filter(|t| t.uid == "prep").for_each(|t| t.set_aside = "another".into());
        assert_eq!(bringing_back(&all, "early", &later).len(), 1, "a task set aside by another appointment stays");
    }

    /// A link written before its instant was kept (no AT) stands by the day it
    /// wrote, in a zone not known: the appointment's day in some zone is
    /// consistent, never followed back a day (review of ab7daa4, later 13).
    #[test]
    fn a_link_without_its_instant_holds_across_zones() {
        let zone = TimeZone::get("Europe/Paris").unwrap();
        // 23:30 in Paris on 29 October: 30 October already from UTC+2 on.
        let start = "2026-10-29T23:30:00+01:00[Europe/Paris]".parse::<Zoned>().unwrap().timestamp().as_second();
        let appointment = Appointment { day: Some(jiff::civil::date(2026, 10, 29)), cancelled: false, recurring: false, start };
        let task = |day: Date| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:pack\r\nDTSTAMP:20261020T100000Z\r\nSUMMARY:Pack\r\nDUE;VALUE=DATE:{}\r\n{}\r\nEND:VTODO\r\nEND:VCALENDAR\r\n", day.strftime("%Y%m%d"), appointment_line("late-visit", Some(day), None, None));
        let written = task(jiff::civil::date(2026, 10, 30));
        let pack = tasks::task_of_text(&written, &zone).unwrap();
        assert!(stand(&pack, &written, &appointment, None, &zone).0, "its day where it was written");
        let far = task(jiff::civil::date(2026, 11, 2));
        assert!(!stand(&tasks::task_of_text(&far, &zone).unwrap(), &far, &appointment, None, &zone).0, "a day no zone gives: it follows");
    }

    /// Brought back for one appointment, a step tied again to another is set
    /// aside once that one is over (review of 4d7bab4, later 10): the mark
    /// holds for the appointment it names.
    #[test]
    fn brought_back_holds_for_that_appointment_alone() {
        let desk = Desk::new("aside-other-appointment");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let first = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&first, &read(&first), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-29T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &later).write().unwrap();
        for (path, text) in bringing_back(&desk.tasks(), "early", &later) {
            std::fs::write(path, text).unwrap();
        }
        assert!(task_of(&desk, "early").brought_back && task_of(&desk, "prep").brought_back);
        assert!(closing(&desk.tasks(), &desk.refs(), closing_from(), &later).is_empty(), "that appointment never sets it aside again");
        // Tied again to another appointment, on 2 November, once it is over.
        let second = desk.event("second", "");
        std::fs::write(&second, read(&second).replace("20261029T13", "20261102T13")).unwrap();
        attach(&desk.tasks(), "prep", &file(&second, &read(&second), true), &desk.refs(), &any, &later).unwrap().write().unwrap();
        let over: Zoned = "2026-11-02T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), closing_from(), &over).write().unwrap();
        let early = task_of(&desk, "early");
        assert_eq!((early.status, early.set_aside.as_str()), (Status::Cancelled, "second"), "the other one sets it aside");
    }

    /// A list of this device's own that the sharing carries reads as the
    /// sharing's last exchange gone through whole (review of ab7daa4, later
    /// 14): begun before the end, nothing closes yet; after, it closes; not
    /// carried, at once.
    #[test]
    fn a_local_list_the_sharing_carries_waits_for_an_exchange() {
        let desk = Desk::new("aside-local-carried");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        let carried = |began: i64| crate::moves::Pull { syncs: true, began: Some(began) };
        assert!(closing_with(&desk.tasks(), &desk.refs(), closing_from(), &later, carried(1_793_280_000)).is_empty(), "the last exchange began before the end");
        assert!(!closing_with(&desk.tasks(), &desk.refs(), closing_from(), &later, carried(1_793_282_400)).is_empty(), "one begun after it");
        assert!(!closing(&desk.tasks(), &desk.refs(), closing_from(), &later).is_empty(), "not carried: at once");
    }

    /// A task list that syncs, its last pull begun before the end: a step
    /// changed on another device after it may not be here yet, so nothing is
    /// set aside until a pull begun after the end went through (review of
    /// 4d7bab4, later 12).
    #[test]
    fn a_list_that_syncs_is_read_whole_first() {
        let account = "pairs-list-pull";
        let dir = vdir::prepare(Kind::Calendars, account, "tasks", "Synced tasks", None).unwrap();
        let state = vdir::state_path(account, Kind::Calendars, "tasks");
        let mut known = vdir::State::load(&state);
        known.url = "https://dav.example.org/calendars/tasks/".into();
        known.components = vec!["VTODO".into()];
        // 29 October, 13:20 UTC: begun before the appointment's end (13:30).
        known.pulled = Some(1_793_280_000);
        known.save(&state).unwrap();
        let list = vdir::collections(Kind::Calendars).into_iter().find(|c| c.dir == dir).unwrap();
        let zone = TimeZone::system();
        for (uid, parent) in [("prep", ""), ("early", "prep")] {
            vdir::write_item(&dir.join(format!("{uid}.ics")), &tasks::new_task(&TaskEdit { title: uid.into(), parent: parent.into(), ..TaskEdit::default() }, uid, &zone, &now()).unwrap()).unwrap();
        }
        let all = || list.items().into_iter().filter_map(|p| tasks::read(&p, &list, &zone)).collect::<Vec<_>>();
        let desk = Desk::new("aside-list-pull");
        let path = desk.event("visit", "");
        attach(&all(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-30T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        assert!(closing(&all(), &desk.refs(), closing_from(), &later).is_empty(), "its list not read whole since the end");
        // 14:00 UTC: a pull begun after the end went through.
        known.pulled = Some(1_793_282_400);
        known.save(&state).unwrap();
        assert!(!closing(&all(), &desk.refs(), closing_from(), &later).is_empty(), "read whole since: set aside");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The way back (review of ab7daa4, M1): what an appointment set aside is
    /// on its card; brought back, it is open again, and that appointment never
    /// sets it aside again.
    #[test]
    fn a_step_set_aside_is_brought_back() {
        let from = now().timestamp().as_second();
        let desk = Desk::new("brought-back");
        desk.task("prep", TaskEdit::default());
        desk.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let later: Zoned = "2026-10-29T16:00:00+01:00[Europe/Paris]".parse().unwrap();
        closing(&desk.tasks(), &desk.refs(), from, &later).write().unwrap();
        let zone = TimeZone::system();
        let o = agenda::Occurrence { key: path.display().to_string(), uid: "visit".into(), summary: "Doctor".into(), start: 1_793_278_800, end: 1_793_280_600, ..agenda::Occurrence::default() };
        let tr = crate::i18n::Translator::new("en");
        let shown = appointment_view(&o, &[], &desk.tasks(), &[], None, &tr, &zone);
        assert_eq!(shown.set_aside.iter().map(|n| n.uid.as_str()).collect::<BTreeSet<_>>(), ["early", "prep"].into_iter().collect::<BTreeSet<_>>(), "on its card");
        for uid in ["early", "prep"] {
            let key = desk.list.dir.join(format!("{uid}.ics"));
            std::fs::write(&key, brought_back(&read(&key), &later).unwrap()).unwrap();
        }
        assert_eq!((task_of(&desk, "early").status, task_of(&desk, "early").brought_back), (Status::NeedsAction, true));
        assert!(closing(&desk.tasks(), &desk.refs(), from, &later).is_empty(), "never set aside again by it");
        assert!(appointment_view(&o, &[], &desk.tasks(), &[], None, &tr, &zone).set_aside.is_empty());
        assert!(brought_back(&read(&desk.list.dir.join("early.ics")), &later).is_none(), "brought back once");
        // Its task still open (a step after it remains): the step brought back alone stays open.
        let open = Desk::new("brought-back-open");
        open.task("prep", TaskEdit::default());
        open.task("early", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        open.task("later-on", TaskEdit { parent: "prep".into(), ..TaskEdit::default() });
        let visit = open.event("visit", "");
        attach(&open.tasks(), "prep", &file(&visit, &read(&visit), true), &open.refs(), &any, &now()).unwrap().write().unwrap();
        let after = crate::anchors::step_side(&read(&open.list.dir.join("later-on.ics")), true, &now()).unwrap();
        std::fs::write(open.list.dir.join("later-on.ics"), after).unwrap();
        closing(&open.tasks(), &open.refs(), from, &later).write().unwrap();
        assert_eq!((task_of(&open, "early").status, task_of(&open, "prep").status), (Status::Cancelled, Status::NeedsAction));
        let key = open.list.dir.join("early.ics");
        std::fs::write(&key, brought_back(&read(&key), &later).unwrap()).unwrap();
        assert!(closing(&open.tasks(), &open.refs(), from, &later).is_empty(), "the step brought back, never set aside again");
    }

    /// The appointment's view (parts 4 and 6): Leave at from its first leg, its
    /// legs before and after where they will be once followed, a doubt said on
    /// its line; its steps before and after; another task tied to it.
    #[test]
    fn an_appointment_s_view_says_its_legs_and_its_steps_after() {
        let zone = TimeZone::get("Europe/Paris").unwrap();
        let t = |at: &str| at.parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let event = agenda::Occurrence { key: "/c/doctor.ics".into(), uid: "doctor".into(), summary: "Doctor".into(), start: t("2026-10-29T15:00"), end: t("2026-10-29T15:30"), ..agenda::Occurrence::default() };
        let anchor = |side, gap, was, unconfirmed: bool| crate::anchors::Anchor { appointment: "doctor".into(), side, gap, timetable: unconfirmed, was: Some(was), unconfirmed: unconfirmed.then_some(was), cancelled_with: false };
        let legs = [
            // Set for 14:00, not followed yet: said where it will be (14:10).
            crate::anchors::LegSeen { uid: "bus".into(), key: "/c/bus.ics".into(), title: "Bus 24".into(), start: t("2026-10-29T13:10"), end: t("2026-10-29T13:35"), anchor: anchor(crate::anchors::Side::Before, 25 * 60, t("2026-10-29T14:00"), true) },
            crate::anchors::LegSeen { uid: "home".into(), key: "/c/home.ics".into(), title: "Walk home".into(), start: t("2026-10-29T15:40"), end: t("2026-10-29T16:00"), anchor: anchor(crate::anchors::Side::After, 10 * 60, t("2026-10-29T15:30"), false) },
        ];
        let link = |uid: &str| vec![tasks::Link { uri: format!("sioul:event/{uid}"), label: String::new(), rel: APPOINTMENT.into() }];
        let all = [
            Task { uid: "prep".into(), title: "Going to the doctor".into(), links: link("doctor"), created: 1, ..Task::default() },
            Task { uid: "card".into(), title: "Find the card".into(), relations: vec![tasks::Relation { kind: "PARENT".into(), uid: "prep".into(), gap: 0 }], created: 2, ..Task::default() },
            Task { uid: "next".into(), title: "Book the next one".into(), relations: vec![tasks::Relation { kind: "PARENT".into(), uid: "prep".into(), gap: 0 }], after: true, created: 3, ..Task::default() },
            Task { uid: "papers".into(), title: "The papers".into(), links: link("doctor"), created: 4, ..Task::default() },
        ];
        let tr = crate::i18n::Translator::new("en");
        let view = appointment_view(&event, &[], &all, &legs, None, &tr, &zone);
        assert_eq!(view.leave_at, Some(t("2026-10-29T14:10")), "from its first leg");
        assert_eq!(view.getting_there.iter().map(|l| (l.title.as_str(), l.when.as_str(), l.unconfirmed)).collect::<Vec<_>>(), [("Bus 24", "14:10 – 14:35", true)]);
        assert_eq!(view.coming_back.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), ["Walk home"]);
        assert_eq!((view.before.iter().map(|s| s.uid.as_str()).collect::<Vec<_>>(), view.steps_after.iter().map(|s| s.uid.as_str()).collect::<Vec<_>>()), (vec!["card"], vec!["next"]));
        assert_eq!(view.also.iter().map(|s| s.uid.as_str()).collect::<Vec<_>>(), ["papers"]);
        assert_eq!(view.line, "Leave at 14:10 · Bus 24 (timetable not checked) · Next: Find the card");
    }

    /// E0.9 and E0.10: a tied task bound for Google Tasks says it loses its tie,
    /// and is untied by the one rule; two done tasks naming one event, the same
    /// one is found whatever the order read; an open task of an account switched
    /// off keeps the appointment taken.
    #[test]
    fn google_lists_untie_and_a_taken_appointment_stays_taken() {
        let desk = Desk::new("e0-9");
        desk.task("prep", TaskEdit::default());
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let task = task_of(&desk, "prep");
        assert!(crate::capabilities::task_losses(&task, crate::capabilities::Provider::GoogleTasks).contains(&"appointment"));
        let change = untied_for_list(&task, &desk.refs(), &now());
        change.write().unwrap();
        assert_eq!(appointment_of(&task_of(&desk, "prep")), None);
        assert!(!read(&path).contains("sioul:task/prep"), "the event's line out");
        // Two done tasks for one event: the oldest, whatever the order read.
        let link = vec![tasks::Link { uri: "sioul:event/visit".into(), label: String::new(), rel: APPOINTMENT.into() }];
        let older = Task { uid: "older".into(), links: link.clone(), status: Status::Completed, created: 1, ..Task::default() };
        let newer = Task { uid: "newer".into(), links: link, status: Status::Completed, created: 2, ..Task::default() };
        let event = EventRef { uid: "visit".into(), ..EventRef::default() };
        assert_eq!(preparation_of(&[newer.clone(), older.clone()], &event).map(|t| t.uid.as_str()), Some("older"));
        assert_eq!(preparation_of(&[older, newer], &event).map(|t| t.uid.as_str()), Some("older"));
        // An open task of an account switched off, tied to the event: another task's tie is refused.
        let off = crate::vdir::Kind::Calendars.root().join("pairs-off-tasks");
        std::fs::create_dir_all(off.join("tasks")).unwrap();
        let theirs = tasks::new_task(&TaskEdit { title: "Theirs".into(), ..TaskEdit::default() }, "theirs", &TimeZone::system(), &now()).unwrap();
        // Its own event's UID, which no other test ties to (the account switched off is seen by all of them).
        let theirs = tasks::replace_lines(&theirs, |_| false, vec![appointment_line("visit-e010", None, None, None)], &now()).unwrap();
        std::fs::write(off.join("tasks/theirs.ics"), theirs).unwrap();
        let taken = desk.event("visit-e010", "");
        crate::vdir::set_account_off("pairs-off-tasks", true);
        assert_eq!(attach(&desk.tasks(), "prep", &file(&taken, &read(&taken), true), &desk.refs(), &any, &now()), Err(Refused("pair-taken")));
        crate::vdir::set_account_off("pairs-off-tasks", false);
        let _ = std::fs::remove_dir_all(&off);
    }

    /// A tied task moved into a list that keeps no link: untied in the same
    /// change as the move, so that a move cut short unties nothing (review of
    /// ab7daa4, later 5). Unix alone: a folder's write permission is what cuts
    /// the move short there.
    #[cfg(unix)]
    #[test]
    fn a_move_into_a_list_without_links_unties_in_one_change() {
        use std::os::unix::fs::PermissionsExt;
        let desk = Desk::new("moved-into");
        desk.task("prep", TaskEdit::default());
        let path = desk.event("visit", "");
        attach(&desk.tasks(), "prep", &file(&path, &read(&path), true), &desk.refs(), &any, &now()).unwrap().write().unwrap();
        let (task_before, event_before) = (read(&desk.list.dir.join("prep.ics")), read(&path));
        let target = vdir::create(Kind::Calendars, vdir::LOCAL, "moved-into google", None, &["VTODO"]).unwrap();
        let task = task_of(&desk, "prep");
        let change = moved_into(&[&task], &target.dir, &desk.refs(), true, &now()).unwrap();
        std::fs::set_permissions(&target.dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let cut = change.write();
        std::fs::set_permissions(&target.dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(cut.is_err());
        assert_eq!((read(&desk.list.dir.join("prep.ics")), read(&path)), (task_before, event_before), "nothing untied");
        moved_into(&[&task], &target.dir, &desk.refs(), true, &now()).unwrap().write().unwrap();
        let moved: Vec<Task> = target.items().into_iter().filter_map(|p| tasks::read(&p, &target, &TimeZone::system())).collect();
        assert_eq!(moved.iter().map(|t| (t.uid.as_str(), appointment_of(t))).collect::<Vec<_>>(), [("prep", None)], "moved, untied");
        assert!(!read(&path).contains("sioul:task/prep"), "the event's line out with it");
        let _ = std::fs::remove_dir_all(&target.dir);
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
        let moved = Appointment { day: Some("2026-11-03".parse().unwrap()), cancelled: false, recurring: false, start: 0 };
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
