// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Time blocks: a task pinned to a time is an event in a calendar, tied to the
//! task both ways (docs/tasks.md, "Pinned to a time"). An event shows in every
//! calendar application, a phone's too, and moves there like any other; a line
//! of the task's own would be Sioul's alone, and a time on the task's start
//! breaks when its date asked is a date (RFC 5545 §3.8.2.3: servers that
//! check, Sabre for Nextcloud, refuse the task).
//!
//! - **Written**: the task's title, kept in step; its start and end in UTC;
//!   busy (`TRANSP:OPAQUE`); `X-SIOUL-TASK:<the task's UID>` and an RFC 9253
//!   link to it (`LINK;LINKREL="…/block-of";VALUE=UID`). The task links back
//!   (`…/time-block`), to its current block only: a task that comes back every
//!   week would gather a link a week. No alarm unless Tasks ⚙ asks for one:
//!   Sioul reminds of a block as of any event, and a phone would remind twice.
//! - **Read**: the block of a task known here is the task's time, never an
//!   event of its own to the plan, the day or the record (`without_blocks`). A
//!   task is pinned to its first block still to come (`Blocks::of`); over, a
//!   block pins nothing and stays as it was, nothing said of it. A block whose
//!   task is not known here (not come yet, deleted elsewhere) is an event like
//!   any other.
//! - **A repeating task**: a block pins one turn (`X-SIOUL-TURN`, that turn's
//!   date when it was pinned); once that turn is done, it pins no other.
//! - **Set right** (`upkeep`), only when something differs: a block not begun
//!   of a task done or dropped goes, its time freed; one under way ends then;
//!   one over stays, the record of when the work was done. One not begun of a
//!   turn done elsewhere goes. One to come takes its task's title again.

use crate::agenda::{self, Occurrence};
use crate::lines;
use crate::tasks::{self, Link, Status, Task};
use crate::vdir::{self, Collection, Kind};
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// The task a block is the time of, by its UID, in the block.
pub const TASK: &str = "X-SIOUL-TASK";
/// The turn of a repeating task a block pins: its date asked, else its day to
/// start, as they were when it was pinned ("2026-10-09").
pub const TURN: &str = "X-SIOUL-TURN";
/// The link from a block to its task (RFC 9253 §6.3: LINKREL takes a
/// registered relation or a URI; a tag URI, RFC 4151, as task kinds are).
pub const BLOCK_OF: &str = "tag:aurelienpierre.com,2026:sioul/rel/block-of";
/// The link from a task to its block.
pub const TIME_BLOCK: &str = "tag:aurelienpierre.com,2026:sioul/rel/time-block";
/// The calendar made for blocks at first use, the same on every device: its
/// folder here and its address on the server (made with `MKCALENDAR` at the
/// next sync; one another device made already counts as made).
pub const CALENDAR: &str = "planned-tasks";
/// A block's alarm, when Tasks ⚙ asks for one: five minutes before it starts.
pub const ALARM_BEFORE: &str = "-PT5M";
/// How far ahead blocks are read in their own calendars: as far as the plan lays tasks.
pub const AHEAD_DAYS: i64 = 3 * 366;
/// The least a block lasts, in minutes.
pub const SHORTEST: u32 = 5;

/// Where a task is pinned: its block, and the day it is on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pin {
    /// The block's file and UID.
    pub key: String,
    pub uid: String,
    /// Unix seconds.
    pub start: i64,
    pub end: i64,
    /// Its start's day, in your time zone.
    pub date: Date,
    /// The task's margins, kept around the block (getting there and back, getting ready).
    pub before: u32,
    pub after: u32,
    /// Its calendar can only be read here: it moves elsewhere only.
    pub read_only: bool,
}

impl Pin {
    /// Its minutes, its margins aside.
    pub fn minutes(&self) -> u32 {
        u32::try_from((self.end - self.start).max(0) / 60).unwrap_or(u32::MAX)
    }

    /// The time it holds: the block, its margins around it.
    pub fn span(&self) -> (i64, i64) {
        (self.start - i64::from(self.before) * 60, self.end + i64::from(self.after) * 60)
    }
}

/// The blocks read, sorted out: where each task is pinned, and what held the
/// work of a task done.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Blocks {
    /// By task UID: its first block still to come.
    pub pins: BTreeMap<String, Pin>,
    /// By task UID, a task done: the block that held its work (begun before it
    /// was done, that day), (start, end) in Unix seconds, to show it there.
    pub records: BTreeMap<String, (i64, i64)>,
}

/// The turn of a repeating task a block made now pins: its date asked, else
/// its day to start ("2026-10-09"); "" for a task that does not come back.
pub fn turn_of(task: &Task) -> String {
    if task.repeat.is_empty() {
        return String::new();
    }
    task.due.get(..10).or_else(|| task.start.get(..10)).unwrap_or("").to_string()
}

/// A block of a turn done since: the task's turn is later than the one it
/// pinned (a turn done moves the dates on, never back).
fn of_a_turn_done(block: &Occurrence, task: &Task) -> bool {
    let turn = turn_of(task);
    !block.turn.is_empty() && !turn.is_empty() && turn.as_str() > block.turn.as_str()
}

/// The UIDs of the tasks known here.
fn known(tasks: &[Task]) -> BTreeSet<&str> {
    tasks.iter().map(|t| t.uid.as_str()).collect()
}

/// The events without the blocks of the tasks known here: what holds time as
/// an event, to the plan, the day and the record. A block is its task's time.
pub fn without_blocks(mut events: Vec<Occurrence>, tasks: &[Task]) -> Vec<Occurrence> {
    let known = known(tasks);
    events.retain(|e| e.task.is_empty() || !known.contains(e.task.as_str()));
    events
}

impl Blocks {
    /// The blocks among `blocks` (events, read once or twice: each counts
    /// once), as of `now`, their days in `zone`.
    pub fn of(blocks: &[Occurrence], tasks: &[Task], now: i64, zone: &TimeZone) -> Blocks {
        let by_uid: BTreeMap<&str, &Task> = tasks.iter().map(|t| (t.uid.as_str(), t)).collect();
        let day = |seconds: i64| Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()).date());
        let mut seen = BTreeSet::new();
        let mut sorted: Vec<&Occurrence> = blocks.iter().filter(|b| !b.task.is_empty() && !b.cancelled && !b.all_day && b.end > b.start).filter(|b| seen.insert((b.key.clone(), b.start))).collect();
        sorted.sort_by(|a, b| (a.start, a.end, &a.key).cmp(&(b.start, b.end, &b.key)));
        let mut out = Blocks::default();
        for block in sorted {
            let Some(task) = by_uid.get(block.task.as_str()) else { continue };
            if task.status.is_open() {
                if block.end > now && !of_a_turn_done(block, task) && !out.pins.contains_key(&task.uid) {
                    let Some(date) = day(block.start) else { continue };
                    out.pins.insert(task.uid.clone(), Pin { key: block.key.clone(), uid: block.uid.clone(), start: block.start, end: block.end, date, before: task.margins.before, after: task.margins.after, read_only: block.read_only });
                }
            } else if task.status == Status::Completed
                && let Some(done) = task.completed
                && block.start <= done
                && day(block.start) == day(done)
            {
                // The latest begun before it was done, that day.
                out.records.insert(task.uid.clone(), (block.start, block.end));
            }
        }
        out
    }
}

/// The calendars blocks are looked for in, beyond the four weeks of events the
/// plan reads: the one chosen for them (`chosen`, "account/id"), and those
/// made for them, on every account.
pub fn calendars(chosen: &str) -> Vec<Collection> {
    vdir::collections(Kind::Calendars).into_iter().filter(|c| c.id == CALENDAR || (!chosen.is_empty() && format!("{}/{}", c.account, c.id) == chosen)).collect()
}

/// The blocks of `calendars` between `from` and `to` (Unix seconds); times
/// without a zone in `zone`.
pub fn read(calendars: &[Collection], from: i64, to: i64, zone: &TimeZone) -> Vec<Occurrence> {
    calendars.iter().flat_map(|c| c.items().into_iter().flat_map(|path| agenda::file_occurrences(&path, c, from, to, zone)).collect::<Vec<_>>()).filter(|o| !o.task.is_empty()).collect()
}

/// A value written as it is: no line break nor other control character.
fn one_line(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
}

fn utc(seconds: i64) -> Result<String, String> {
    Timestamp::from_second(seconds).map(|t| t.strftime("%Y%m%dT%H%M%SZ").to_string()).map_err(|e| e.to_string())
}

/// A new block for `task`, `start` to `end` (Unix seconds), with its own `uid`;
/// an alarm five minutes before it when asked (`alarm`), else none.
pub fn new_block(task: &Task, uid: &str, start: i64, end: i64, alarm: bool, now: &Zoned) -> Result<String, String> {
    if end <= start {
        return Err(format!("{start}–{end}?"));
    }
    let stamp = utc(now.timestamp().as_second())?;
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "CALSCALE:GREGORIAN".to_string(), "BEGIN:VEVENT".to_string()];
    out.extend([format!("UID:{}", one_line(uid)), format!("DTSTAMP:{stamp}"), format!("CREATED:{stamp}"), format!("LAST-MODIFIED:{stamp}")]);
    out.extend([format!("DTSTART:{}", utc(start)?), format!("DTEND:{}", utc(end)?)]);
    let title = task.title.trim();
    if !title.is_empty() {
        out.push(format!("SUMMARY:{}", lines::escape(title)));
    }
    out.push("TRANSP:OPAQUE".to_string());
    out.push(format!("{TASK}:{}", one_line(&task.uid)));
    let turn = turn_of(task);
    if !turn.is_empty() {
        out.push(format!("{TURN}:{turn}"));
    }
    out.push(tasks::link_line(&Link { uri: format!("uid:{}", task.uid), label: String::new(), rel: BLOCK_OF.to_string() }));
    if alarm {
        out.extend(["BEGIN:VALARM".to_string(), "ACTION:DISPLAY".to_string(), format!("DESCRIPTION:{}", lines::escape(title)), format!("TRIGGER:{ALARM_BEFORE}"), "END:VALARM".to_string()]);
    }
    out.extend(["END:VEVENT".to_string(), "END:VCALENDAR".to_string()]);
    Ok(lines::fold(&out))
}

/// The block moved to `start`..`end`, as a drag in the agenda moves an event
/// (`agenda::moved`): what Sioul does not edit is kept (an alarm another
/// application added). A block another application made repeat moves its
/// first time only.
pub fn moved(text: &str, start: i64, end: i64, zone: &TimeZone) -> Result<String, String> {
    let (current, _) = agenda::first_times(text, zone).ok_or("no event")?;
    let repeats = lines::unfold(text).iter().any(|l| matches!(lines::name(l).as_str(), "RRULE" | "RDATE"));
    agenda::moved(text, current, start, end, repeats, zone).map_err(|e| format!("{e:?}"))
}

/// The block ending at `end`, its start as it is.
pub fn ended(text: &str, end: i64, zone: &TimeZone) -> Result<String, String> {
    let (start, _) = agenda::first_times(text, zone).ok_or("no event")?;
    moved(text, start, end, zone)
}

/// The block titled as its task (`SUMMARY`, its stamps and `SEQUENCE` moved
/// on); None when it is already (its line as Sioul writes it, or read the
/// same), or holds no event: never written twice for one title.
pub fn titled(text: &str, title: &str, zone: &TimeZone) -> Option<String> {
    let wanted = format!("SUMMARY:{}", lines::escape(title.trim()));
    if lines::unfold(text).iter().any(|l| *l == wanted) {
        return None;
    }
    let mut edit = agenda::edit_of_text(text, zone)?;
    edit.title = title.trim().to_string();
    agenda::apply(text, &edit, zone).ok().filter(|written| written != text)
}

/// The block pinning another turn of its repeating task (`X-SIOUL-TURN`: its
/// dates changed in Sioul, the block follows them); "" for none.
pub fn turned(text: &str, turn: &str) -> Option<String> {
    let without = agenda::remove_lines(text, |l| lines::name(l) == TURN)?;
    if turn.is_empty() {
        return Some(without);
    }
    agenda::add_lines(&without, &[format!("{TURN}:{}", one_line(turn))])
}

/// What becomes of a block once its task is done, dropped or left to the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closing {
    /// Over by then: kept, the record of when the work was done.
    Keep,
    /// Under way then: it ends then (to the minute, five minutes at least).
    EndAt(i64),
    /// Not begun: it goes, its time freed.
    Delete,
}

/// What becomes of a block from `start` to `end` when its task is done,
/// dropped or left to the plan at `at` (Unix seconds).
pub fn closing(start: i64, end: i64, at: i64) -> Closing {
    if end <= at {
        return Closing::Keep;
    }
    if start >= at {
        return Closing::Delete;
    }
    let ends = ((at + 59).div_euclid(60) * 60).max(start + i64::from(SHORTEST) * 60);
    if ends >= end { Closing::Keep } else { Closing::EndAt(ends) }
}

/// Something to set right in a block (`upkeep`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// Not begun, its task done or dropped, or its turn done: it goes.
    Delete,
    /// Under way when its task was done or dropped: it ends then.
    EndAt(i64),
    /// To come, titled otherwise than its task: its task's title.
    Title(String),
}

/// What to set right in `blocks` as of `now`, by file: the blocks of a task
/// done (from when it was done) or dropped (from now), as `closing` says; a
/// block not begun of a turn done elsewhere goes; one to come of an open task
/// takes its title again. Nothing for a block of a task unknown here, nor for
/// one another application made repeat. Each file once.
pub fn upkeep(blocks: &[Occurrence], tasks: &[Task], now: i64) -> Vec<(String, Fix)> {
    let by_uid: BTreeMap<&str, &Task> = tasks.iter().map(|t| (t.uid.as_str(), t)).collect();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for block in blocks.iter().filter(|b| !b.task.is_empty() && !b.all_day && !b.recurring) {
        if !seen.insert(block.key.clone()) {
            continue;
        }
        let Some(task) = by_uid.get(block.task.as_str()) else { continue };
        let fix = match task.status {
            Status::Completed | Status::Cancelled => {
                let at = if task.status == Status::Completed { task.completed.unwrap_or(now) } else { now };
                match closing(block.start, block.end, at) {
                    Closing::Keep => None,
                    Closing::EndAt(end) => Some(Fix::EndAt(end)),
                    Closing::Delete => Some(Fix::Delete),
                }
            }
            _ if of_a_turn_done(block, task) => (block.start > now).then_some(Fix::Delete),
            _ if block.end > now && !task.title.trim().is_empty() && block.summary.trim() != task.title.trim() => Some(Fix::Title(task.title.trim().to_string())),
            _ => None,
        };
        out.extend(fix.map(|fix| (block.key.clone(), fix)));
    }
    out
}

/// The task's link to its block.
pub fn task_link(block_uid: &str) -> Link {
    Link { uri: format!("uid:{block_uid}"), label: String::new(), rel: TIME_BLOCK.to_string() }
}

/// Whether a task's link is to a block of its.
pub fn is_block_link(link: &Link) -> bool {
    link.rel == TIME_BLOCK
}

/// The task's text pinned to the block `uid`, or to none: its links to blocks
/// made that one alone, and a time given by a drag before blocks (`X-SIOUL-AT`)
/// taken out; the text as it was when nothing changes.
pub fn task_pinned(text: &str, uid: Option<&str>, now: &Zoned) -> Result<String, String> {
    let source = lines::unfold(text);
    let is_block_line = |l: &str| lines::name(l) == "LINK" && is_block_link(&tasks::link_of(l));
    let drop = |l: &str| is_block_line(l) || lines::name(l) == tasks::AT;
    let wanted = uid.map(task_link);
    let current: Vec<Link> = source.iter().filter(|l| is_block_line(l)).map(|l| tasks::link_of(l)).collect();
    let at = source.iter().any(|l| lines::name(l) == tasks::AT);
    if !at && current.len() == usize::from(wanted.is_some()) && wanted.as_ref().is_none_or(|w| current.contains(w)) {
        return Ok(text.to_string());
    }
    tasks::replace_lines(text, drop, wanted.iter().map(tasks::link_line).collect(), now)
}

/// A block's length by default, in minutes: what the plan lays for the task
/// (`laid`: its corrected estimate, less the time spent, with its margins),
/// its margins aside; else its estimate, else `default_estimate`. By whole
/// five minutes, a quarter of an hour at least.
pub fn default_minutes(task: &Task, laid: Option<u32>, default_estimate: u32) -> u32 {
    let margins = task.margins.before.saturating_add(task.margins.after);
    let minutes = laid.map(|l| l.saturating_sub(margins)).filter(|m| *m > 0).unwrap_or(if task.estimate > 0 { task.estimate } else { default_estimate });
    minutes.max(15).div_ceil(5).saturating_mul(5)
}

/// Where a task's blocks go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Found(Collection),
    /// The calendar for blocks, to make on this account ("local": kept on this device).
    Make(String),
}

/// Where a task's blocks go: the calendar chosen in Tasks ⚙ (`chosen`,
/// "account/id") while it can be written and takes events; else the one made
/// for them on the account of the task's list (`list_account`), or on this
/// device when that list is kept here or its account makes no calendar from
/// Sioul (`makes`: Google's makes none).
pub fn place_for(chosen: &str, list_account: &str, calendars: &[Collection], makes: bool) -> Place {
    if let Some(found) = calendars.iter().find(|c| !chosen.is_empty() && format!("{}/{}", c.account, c.id) == chosen && !c.read_only && c.holds("VEVENT")) {
        return Place::Found(found.clone());
    }
    let account = if list_account.is_empty() || list_account == vdir::LOCAL || !makes { vdir::LOCAL } else { list_account };
    match calendars.iter().find(|c| c.account == account && c.id == CALENDAR && !c.read_only) {
        Some(found) => Place::Found(found.clone()),
        None => Place::Make(account.to_string()),
    }
}

/// The calendar for blocks made on `account` ("local": this device only),
/// named `name` in your language; the next sync makes it on the server.
pub fn make_calendar(account: &str, name: &str) -> Result<Collection, String> {
    let dir = vdir::prepare(Kind::Calendars, account, CALENDAR, name, None).map_err(|e| e.to_string())?;
    let state = vdir::State { pending: account != vdir::LOCAL, components: vec!["VEVENT".to_string()], ..vdir::State::default() };
    state.save(&vdir::state_path(account, Kind::Calendars, CALENDAR))?;
    Ok(Collection { kind: Kind::Calendars, account: account.to_string(), id: CALENDAR.to_string(), dir, name: name.to_string(), color: None, read_only: false, components: state.components })
}

/// Times given by a drag before blocks (`X-SIOUL-AT`) still to honour, each
/// to become a block once: an open task's, for today or later, as (task, its
/// start in Unix seconds), in `zone`.
pub fn given_times<'a>(tasks: &'a [Task], today: Date, zone: &TimeZone) -> Vec<(&'a Task, i64)> {
    tasks
        .iter()
        .filter(|t| t.status.is_open() && !t.at.is_empty())
        .filter_map(|t| {
            let date: Date = t.at.get(..10)?.parse().ok()?;
            if date < today {
                return None;
            }
            let local: DateTime = t.at.parse().ok()?;
            Some((t, local.to_zoned(zone.clone()).ok()?.timestamp().as_second()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> i64 {
        text.parse::<DateTime>().unwrap().to_zoned(paris()).unwrap().timestamp().as_second()
    }

    fn now() -> Zoned {
        "2026-10-06T09:00:00+02:00[Europe/Paris]".parse().unwrap()
    }

    fn task(uid: &str, title: &str) -> Task {
        Task { uid: uid.into(), title: title.into(), estimate: 30, ..Task::default() }
    }

    /// A calendar of its own in a folder of its own, for files read back.
    fn calendar(name: &str) -> Collection {
        let dir = std::env::temp_dir().join(format!("sioul-blocks-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Collection { kind: Kind::Calendars, account: "a".into(), id: CALENDAR.into(), dir, name: "Planned tasks".into(), color: None, read_only: false, components: vec!["VEVENT".into()] }
    }

    /// The blocks of a calendar's files, as the plan reads them.
    fn read_back(calendar: &Collection) -> Vec<Occurrence> {
        read(std::slice::from_ref(calendar), at("2026-10-01T00:00"), at("2026-12-31T00:00"), &paris())
    }

    #[test]
    fn a_block_made_read_back_and_tied_both_ways() {
        let bank = task("bank-1", "Call the bank, about the loan");
        let text = new_block(&bank, "block-1", at("2026-10-06T14:00"), at("2026-10-06T14:45"), false, &now()).unwrap();
        // Busy, its task named twice: Sioul's own line and an RFC 9253 link; no alarm unless asked.
        assert!(text.contains("TRANSP:OPAQUE") && text.contains("X-SIOUL-TASK:bank-1") && text.contains("SUMMARY:Call the bank\\, about the loan"), "{text}");
        assert!(lines::unfold(&text).iter().any(|l| l == "LINK;LINKREL=\"tag:aurelienpierre.com,2026:sioul/rel/block-of\";VALUE=UID:bank-1"), "{text}");
        assert!(!text.contains("VALARM") && !text.contains("X-SIOUL-TURN"), "{text}");
        assert!(agenda::parse(&text).is_some());
        let with_alarm = new_block(&bank, "block-2", at("2026-10-06T14:00"), at("2026-10-06T14:45"), true, &now()).unwrap();
        assert!(with_alarm.contains("BEGIN:VALARM") && with_alarm.contains("TRIGGER:-PT5M"), "{with_alarm}");
        // Read back from its calendar: an event that is the task's time.
        let place = calendar("made");
        std::fs::write(place.dir.join("block-1.ics"), &text).unwrap();
        let found = read_back(&place);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].task.as_str(), found[0].uid.as_str(), found[0].start, found[0].end), ("bank-1", "block-1", at("2026-10-06T14:00"), at("2026-10-06T14:45")));
        assert_eq!(found[0].alarms, Vec::<i64>::new());
        // The task links back, to it alone; again, nothing changes.
        let task_text = tasks::new_task(&tasks::TaskEdit { title: bank.title.clone(), ..tasks::TaskEdit::default() }, "bank-1", &paris(), &now()).unwrap();
        let linked = task_pinned(&task_text, Some("block-1"), &now()).unwrap();
        let read = tasks::task_of_text(&linked, &paris()).unwrap();
        assert_eq!(read.links, vec![task_link("block-1")]);
        assert!(lines::unfold(&linked).iter().any(|l| l == "LINK;LINKREL=\"tag:aurelienpierre.com,2026:sioul/rel/time-block\";VALUE=UID:block-1"), "{linked}");
        assert_eq!(task_pinned(&linked, Some("block-1"), &now()).unwrap(), linked);
        // Pinned elsewhere: the old link goes; to none: no link to a block left.
        let other = tasks::task_of_text(&task_pinned(&linked, Some("block-9"), &now()).unwrap(), &paris()).unwrap();
        assert_eq!(other.links, vec![task_link("block-9")]);
        assert!(tasks::task_of_text(&task_pinned(&linked, None, &now()).unwrap(), &paris()).unwrap().links.is_empty());
        // Both ways, as the links read them: the task points at the event, the event at the task.
        let task = Task { links: read.links.clone(), ..bank.clone() };
        let event = agenda::event_ref(&text, "block-1.ics", &paris()).unwrap();
        let (empty_mail, empty_links) = (crate::mailindex::MailIndex::default(), crate::links::LocalLinks::default());
        let world = crate::links::World { tasks: std::slice::from_ref(&task), events: std::slice::from_ref(&event), contacts: &[], vault: None, drafts: &[], mail: &empty_mail, projects: &[], budget: &[], local: &empty_links, sites: &[] };
        let edges = world.edges();
        assert!(edges.iter().any(|e| e.from == crate::links::task_uri("bank-1") && e.to == crate::links::event_uri("block-1")), "{edges:?}");
        assert!(edges.iter().any(|e| e.from == crate::links::event_uri("block-1") && e.to == crate::links::task_uri("bank-1")), "{edges:?}");
    }

    #[test]
    fn a_task_pinned_to_its_first_block_to_come() {
        let zone = paris();
        let bank = task("bank", "Call the bank");
        let block = |key: &str, from: &str, to: &str| Occurrence { key: key.into(), uid: key.into(), summary: "Call the bank".into(), start: at(from), end: at(to), task: "bank".into(), ..Occurrence::default() };
        let blocks = [block("thursday", "2026-10-08T10:00", "2026-10-08T10:30"), block("morning", "2026-10-06T08:00", "2026-10-06T08:30"), block("tuesday", "2026-10-06T14:00", "2026-10-06T14:45")];
        let now = at("2026-10-06T09:00");
        let found = Blocks::of(&blocks, std::slice::from_ref(&bank), now, &zone);
        // This morning's is over: it pins nothing, stays as it was; this afternoon's pins it, Thursday's waits.
        let pin = &found.pins["bank"];
        assert_eq!((pin.key.as_str(), pin.minutes(), pin.date), ("tuesday", 45, Date::constant(2026, 10, 6)));
        // A block of a task unknown here is an event like any other; of a known one, never.
        let stranger = Occurrence { task: "elsewhere".into(), ..block("stranger", "2026-10-06T16:00", "2026-10-06T17:00") };
        let events = without_blocks(vec![blocks[2].clone(), stranger.clone(), Occurrence { key: "dentist".into(), ..Occurrence::default() }], std::slice::from_ref(&bank));
        assert_eq!(events.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["stranger", "dentist"]);
        // Done: it pins nothing; the block it was done in is its record.
        let done = Task { status: Status::Completed, completed: Some(at("2026-10-06T14:30")), ..bank.clone() };
        let found = Blocks::of(&blocks, std::slice::from_ref(&done), at("2026-10-06T15:00"), &zone);
        assert!(found.pins.is_empty());
        assert_eq!(found.records.get("bank"), Some(&(at("2026-10-06T14:00"), at("2026-10-06T14:45"))));
    }

    #[test]
    fn a_block_moved_then_deleted_in_another_application() {
        let zone = paris();
        let bank = task("bank", "Call the bank");
        let place = calendar("elsewhere");
        let path = place.dir.join("b.ics");
        let text = new_block(&bank, "b", at("2026-10-06T14:00"), at("2026-10-06T15:00"), false, &now()).unwrap();
        std::fs::write(&path, &text).unwrap();
        let now = at("2026-10-06T09:00");
        assert_eq!(Blocks::of(&read_back(&place), std::slice::from_ref(&bank), now, &zone).pins["bank"].start, at("2026-10-06T14:00"));
        // Another application moves it to Wednesday 16:00, writing it its own way (in its zone, a sequence of its own).
        let elsewhere = text.replace("DTSTART:20261006T120000Z", "DTSTART;TZID=Europe/Paris:20261007T160000").replace("DTEND:20261006T130000Z", "DTEND;TZID=Europe/Paris:20261007T170000\r\nSEQUENCE:3");
        std::fs::write(&path, &elsewhere).unwrap();
        let pin = Blocks::of(&read_back(&place), std::slice::from_ref(&bank), now, &zone).pins["bank"].clone();
        assert_eq!((pin.start, pin.date), (at("2026-10-07T16:00"), Date::constant(2026, 10, 7)));
        // Moved again here: its time as Sioul writes it, what the other wrote kept.
        let back = moved(&elsewhere, at("2026-10-07T10:00"), at("2026-10-07T10:45"), &zone).unwrap();
        assert_eq!(agenda::first_times(&back, &zone), Some((at("2026-10-07T10:00"), at("2026-10-07T10:45"))));
        assert!(back.contains("X-SIOUL-TASK:bank") && back.contains("SEQUENCE:4"), "{back}");
        // Deleted there: the task is no longer pinned.
        std::fs::remove_file(&path).unwrap();
        assert!(Blocks::of(&read_back(&place), std::slice::from_ref(&bank), now, &zone).pins.is_empty());
    }

    #[test]
    fn done_or_dropped_frees_what_is_to_come_and_keeps_the_record() {
        // A block from 14:00 to 15:00.
        let (start, end) = (at("2026-10-06T14:00"), at("2026-10-06T15:00"));
        assert_eq!(closing(start, end, at("2026-10-06T13:00")), Closing::Delete, "not begun: it goes");
        assert_eq!(closing(start, end, at("2026-10-06T14:20") + 30), Closing::EndAt(at("2026-10-06T14:21")), "under way: it ends then");
        assert_eq!(closing(start, end, start + 30), Closing::EndAt(start + 300), "five minutes at least");
        assert_eq!(closing(start, end, at("2026-10-06T16:00")), Closing::Keep, "over: the record of the work");
        // Done elsewhere at 14:20: seen later, from when it was done.
        let bank = task("bank", "Call the bank");
        let block = |key: &str, from: &str, to: &str| Occurrence { key: key.into(), summary: "Call the bank".into(), start: at(from), end: at(to), task: "bank".into(), ..Occurrence::default() };
        let blocks = [block("monday", "2026-10-05T10:00", "2026-10-05T11:00"), block("now", "2026-10-06T14:00", "2026-10-06T15:00"), block("friday", "2026-10-09T09:00", "2026-10-09T10:00")];
        let done = Task { status: Status::Completed, completed: Some(at("2026-10-06T14:20")), ..bank.clone() };
        let fixes = upkeep(&blocks, std::slice::from_ref(&done), at("2026-10-06T16:00"));
        assert_eq!(fixes, vec![("now".to_string(), Fix::EndAt(at("2026-10-06T14:20"))), ("friday".to_string(), Fix::Delete)]);
        // Dropped: from now; what is over stays.
        let dropped = Task { status: Status::Cancelled, ..bank.clone() };
        let fixes = upkeep(&blocks, std::slice::from_ref(&dropped), at("2026-10-06T14:30"));
        assert_eq!(fixes, vec![("now".to_string(), Fix::EndAt(at("2026-10-06T14:30"))), ("friday".to_string(), Fix::Delete)]);
        // The file itself: ended to the minute, its start as it was.
        let text = new_block(&bank, "now", start, end, false, &now()).unwrap();
        let shorter = ended(&text, at("2026-10-06T14:20"), &paris()).unwrap();
        assert_eq!(agenda::first_times(&shorter, &paris()), Some((start, at("2026-10-06T14:20"))));
    }

    #[test]
    fn the_title_kept_in_step() {
        let bank = task("bank", "Call the bank");
        let text = new_block(&bank, "b", at("2026-10-06T14:00"), at("2026-10-06T15:00"), false, &now()).unwrap();
        let renamed = Task { title: "Call the bank about the loan".into(), ..bank.clone() };
        let block = Occurrence { key: "b.ics".into(), summary: "Call the bank".into(), start: at("2026-10-06T14:00"), end: at("2026-10-06T15:00"), task: "bank".into(), ..Occurrence::default() };
        assert_eq!(upkeep(std::slice::from_ref(&block), std::slice::from_ref(&renamed), at("2026-10-06T09:00")), vec![("b.ics".to_string(), Fix::Title("Call the bank about the loan".into()))]);
        let titled = titled(&text, &renamed.title, &paris()).unwrap();
        assert!(titled.contains("SUMMARY:Call the bank about the loan") && titled.contains("SEQUENCE:1") && titled.contains("X-SIOUL-TASK:bank"), "{titled}");
        assert_eq!(super::titled(&titled, &renamed.title, &paris()), None, "already so: nothing written");
        // Over: left as it was.
        assert!(upkeep(std::slice::from_ref(&block), std::slice::from_ref(&renamed), at("2026-10-06T16:00")).is_empty());
    }

    #[test]
    fn a_block_pins_one_turn_of_a_repeating_task() {
        let zone = paris();
        let plants = Task { repeat: "weekly".into(), due: "2026-10-09".into(), ..task("plants", "Water the plants") };
        let text = new_block(&plants, "p", at("2026-10-08T18:00"), at("2026-10-08T18:30"), false, &now()).unwrap();
        assert!(text.contains("X-SIOUL-TURN:2026-10-09"), "{text}");
        let block = Occurrence { key: "p.ics".into(), summary: plants.title.clone(), start: at("2026-10-08T18:00"), end: at("2026-10-08T18:30"), task: "plants".into(), turn: "2026-10-09".into(), ..Occurrence::default() };
        let now = at("2026-10-06T09:00");
        assert!(Blocks::of(std::slice::from_ref(&block), std::slice::from_ref(&plants), now, &zone).pins.contains_key("plants"));
        // This turn done elsewhere: the task comes back on the 16th; the block pins that turn no more, and goes.
        let next = Task { due: "2026-10-16".into(), ..plants.clone() };
        assert!(Blocks::of(std::slice::from_ref(&block), std::slice::from_ref(&next), now, &zone).pins.is_empty());
        assert_eq!(upkeep(std::slice::from_ref(&block), std::slice::from_ref(&next), now), vec![("p.ics".to_string(), Fix::Delete)]);
        // Its dates changed in Sioul: the block follows (its turn written again).
        let followed = turned(&text, "2026-10-16").unwrap();
        assert!(followed.contains("X-SIOUL-TURN:2026-10-16") && !followed.contains("2026-10-09"), "{followed}");
        // A task that does not come back has no turn.
        assert_eq!(turn_of(&task("once", "Once")), "");
    }

    #[test]
    fn a_time_given_before_blocks_becomes_one() {
        let zone = paris();
        let today = Date::constant(2026, 10, 6);
        let given = Task { at: "2026-10-06T14:30".into(), ..task("given", "Write the letter") };
        let yesterday = Task { at: "2026-10-05T10:00".into(), ..task("old", "Old") };
        let done = Task { at: "2026-10-06T16:00".into(), status: Status::Completed, ..task("done", "Done") };
        let all = [given.clone(), yesterday, done];
        let found = given_times(&all, today, &zone);
        assert_eq!(found.iter().map(|(t, s)| (t.uid.as_str(), *s)).collect::<Vec<_>>(), [("given", at("2026-10-06T14:30"))]);
        // Its block, at that time, as long as its estimate: it pins the task there.
        let (task, start) = found[0];
        let block = new_block(task, "block-given", start, start + i64::from(default_minutes(task, None, 30)) * 60, false, &now()).unwrap();
        assert_eq!(agenda::first_times(&block, &zone), Some((at("2026-10-06T14:30"), at("2026-10-06T15:00"))));
        let read = Occurrence { key: "block-given.ics".into(), uid: "block-given".into(), start, end: at("2026-10-06T15:00"), task: "given".into(), ..Occurrence::default() };
        assert_eq!(Blocks::of(std::slice::from_ref(&read), std::slice::from_ref(&given), at("2026-10-06T09:00"), &zone).pins["given"].start, at("2026-10-06T14:30"));
        // Pinned to its block: the line goes from the task, the link comes.
        let text = tasks::new_task(&tasks::TaskEdit { title: given.title.clone(), ..tasks::TaskEdit::default() }, "given", &zone, &now()).unwrap();
        let old = text.replace("END:VTODO", "X-SIOUL-AT:20261006T123000Z\r\nEND:VTODO");
        assert_eq!(tasks::task_of_text(&old, &zone).unwrap().at, "2026-10-06T14:30");
        let pinned = task_pinned(&old, Some("block-given"), &now()).unwrap();
        let read = tasks::task_of_text(&pinned, &zone).unwrap();
        assert_eq!((read.at.as_str(), read.links.clone()), ("", vec![task_link("block-given")]));
        assert!(!pinned.contains("X-SIOUL-AT"), "{pinned}");
    }

    #[test]
    fn where_blocks_go() {
        let here = |account: &str, id: &str| Collection { kind: Kind::Calendars, account: account.into(), id: id.into(), dir: std::env::temp_dir(), name: id.into(), color: None, read_only: false, components: vec![] };
        let calendars = [here("cloud", "personal"), here("cloud", CALENDAR), here("other", "work")];
        // As usual: the one made for blocks on the list's account.
        assert_eq!(place_for("", "cloud", &calendars, true), Place::Found(here("cloud", CALENDAR)));
        // Another account without one yet: made there; a local list or Google's: on this device.
        assert_eq!(place_for("", "other", &calendars, true), Place::Make("other".into()));
        assert_eq!(place_for("", vdir::LOCAL, &calendars, true), Place::Make(vdir::LOCAL.into()));
        assert_eq!(place_for("", "google", &calendars, false), Place::Make(vdir::LOCAL.into()));
        // Chosen in Tasks ⚙: there, while it can be written.
        assert_eq!(place_for("other/work", "cloud", &calendars, true), Place::Found(here("other", "work")));
        let read_only = [Collection { read_only: true, ..here("other", "work") }, here("cloud", CALENDAR)];
        assert_eq!(place_for("other/work", "cloud", &read_only, true), Place::Found(here("cloud", CALENDAR)));
        // Its length by default: what the plan lays, margins aside, by five minutes.
        let errand = Task { margins: crate::demands::Margins { before: 10, after: 10 }, ..task("e", "Errand") };
        assert_eq!((default_minutes(&errand, Some(53), 30), default_minutes(&errand, None, 30), default_minutes(&task("t", "T"), Some(8), 30)), (35, 30, 15));
    }
}
