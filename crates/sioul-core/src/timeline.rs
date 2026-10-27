// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! One model for everything that has a time (the unified agenda): an
//! appointment, a task's time block, a step the plan lays, a task done, a
//! meal, a nap, the night, a dose, a date asked, a day off, the time kept
//! for you. Each is an `Item`, made by an adapter beside its store
//! (`agenda::items` for the calendars, `dayview::lay_on` and `from_day` for
//! the plan's days, `days_off` for the configuration's days off; the window
//! adds the needs and the doses, which live in its own code). Every view only
//! reads items (`days`); an agent reads them through `ForAgents`, never a
//! view's layout. Nothing here is written: the timeline is computed, and the
//! files stay as other calendar apps know them.
//!
//! Each item says what may change it and through which path (`Can`), decided
//! by its writer's own rule: a view never offers what the writer would
//! refuse. Health items carry no address, and never reach an agent.

use crate::dayview::DayView;
use crate::i18n::Translator;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use serde::Serialize;

/// Writes of a task's, an event's or a day's file in this program, counted:
/// a layout made before a write is older than it, whatever its clock says
/// (`written`). A view made from an older layout reads the files again.
static WRITTEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many writes of tasks, events and days this program made so far.
pub fn written() -> u64 {
    WRITTEN.load(std::sync::atomic::Ordering::SeqCst)
}

/// One more write of a task, an event or a day.
pub fn note_written() {
    WRITTEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// One thing that has a time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// Stable across reloads and re-plans (`event_id`, `step_id`…): what
    /// keeps a card open, a thing chosen, a row in place.
    pub id: String,
    pub kind: Kind,
    pub time: Time,
    pub certainty: Certainty,
    /// Where it lives: what its card reads, and what its own path takes.
    pub source: Source,
    /// What may be done to it here, decided by its writer's own rule.
    pub can: Can,
    pub privacy: Privacy,
    /// Its words on the timeline: an event's title, a task's, a meal's name.
    pub title: String,
    /// One line under it, worded in Rust; "" when it has none.
    pub line: String,
    /// Its whole name for a screen reader; "" until a view words it.
    pub said: String,
    pub tone: Tone,
    /// A calendar's or a list's colour; None for Sioul's own kinds.
    pub colour: Option<String>,
    /// Its `sioul:` address, for ties and for agents; "" for every health kind.
    pub address: String,
    /// What it belongs to: a block's or a step's task, by UID; "" for none.
    pub owner: String,
    /// It takes time from the plan, the hours and the meals (`Occurrence::holds_time`).
    pub holds_time: bool,
}

/// What a thing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// An appointment, from a calendar.
    Event,
    /// A task's time block, not laid as its step that day.
    Block,
    /// A step the plan lays (or a pinned one, laid in its block).
    Step,
    /// A task done, where its work was.
    Done,
    Meal,
    Nap,
    Night,
    Dose,
    /// A date asked of you.
    Dated,
    DayOff,
    /// Time kept for you.
    Gain,
    /// The free time kept after the last step.
    Slack,
    /// A ride or a walk around an appointment (composed tasks, part E).
    Leg,
}

impl Kind {
    /// Whether it is health's: never named on the timeline, never given to an agent.
    pub fn is_health(self) -> bool {
        matches!(self, Kind::Meal | Kind::Nap | Kind::Night | Kind::Dose)
    }
}

/// When a thing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Time {
    /// Getting ready (or getting there) from `start`, the thing itself from
    /// `at` to `until`, coming back (or winding down) to `end`:
    /// start ≤ at ≤ until ≤ end, Unix seconds.
    Span { start: i64, at: i64, until: i64, end: i64 },
    /// A dose; a date asked with a time.
    At { at: i64 },
    /// A whole-day event, a date asked, days off (`until` the last day); a
    /// step of the day's plan that is laid at no time.
    Day { date: Date, until: Date },
}

impl Time {
    /// A span with no lighter ends.
    pub fn span(at: i64, until: i64) -> Time {
        Time::Span { start: at, at, until, end: until }
    }

    /// When the thing itself starts and ends, Unix seconds (a day's: its
    /// midnights, in `zone`).
    pub fn proper(&self, zone: &TimeZone) -> (i64, i64) {
        match *self {
            Time::Span { at, until, .. } => (at, until),
            Time::At { at } => (at, at),
            Time::Day { date, until } => (midnight(date, zone), until.tomorrow().map_or(midnight(until, zone) + 86_400, |d| midnight(d, zone))),
        }
    }

    /// From its lighter start to its lighter end, Unix seconds.
    pub fn whole(&self, zone: &TimeZone) -> (i64, i64) {
        match *self {
            Time::Span { start, end, .. } => (start, end),
            _ => self.proper(zone),
        }
    }
}

/// Midnight of `date` in `zone`, Unix seconds.
pub fn midnight(date: Date, zone: &TimeZone) -> i64 {
    date.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second())
}

/// How sure a thing's time is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Certainty {
    /// A calendar or a file says it.
    Fixed,
    /// Pinned by you: a time block.
    Held,
    /// The plan proposes it; it moves at each re-plan.
    Planned,
    /// A meal, a nap, the night, as usual.
    Usual,
    /// As changed for that day.
    ThatDay,
    /// Read by Sioul (a letter's date from its scan): it counts until corrected.
    Read,
    /// A leg moved with its appointment, its timetable not checked.
    Unconfirmed,
    Dose(DoseState),
}

/// What is known of a dose. No state says "not taken": a dose said not
/// taken is open, with the doubt whenever another device may know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum DoseState {
    /// A day to come: its time, plainly.
    Ahead,
    /// No reading of its day yet, none since Sioul came back, or the reading does not hold it.
    Reading,
    /// Known not marked on any device; `come`: its time has come; `late`: past its half hour.
    Open { come: bool, late: bool },
    Taken { at: i64 },
    /// Another device may know, this device's record does not read, or the reading is old.
    Check { why: String },
    /// Your devices' answers differ: you choose.
    Differ,
    /// Due while Sioul was closed, answered nowhere: the question on the past.
    Asked,
}

/// What may be done to a thing here, by its writer's own rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Can {
    /// What Edit opens.
    pub edit: Form,
    /// What a drag does.
    pub drag: Move,
    /// Its end can be dragged.
    pub ends: bool,
    pub delete: bool,
    /// Its checklist can be ticked here.
    pub tick: bool,
    /// Why it cannot be changed here, in words; "" when it can.
    pub why: String,
}

impl Can {
    /// Nothing to change here, and no word for it.
    pub fn nothing() -> Can {
        Can { edit: Form::None, drag: Move::None, ends: false, delete: false, tick: false, why: String::new() }
    }
}

/// The form Edit opens: each kind's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Form {
    None,
    Event,
    OwnTime,
    Task,
    Need,
    Medicine,
    Prescription,
    Paper,
    Contract,
    Line,
    Letter,
    DayOff,
}

/// What a drag calls: each kind's own path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Move {
    None,
    /// `pim::move_event`, on the event's (or the block's) own file.
    Event,
    /// `blocks::pin`: dragged, a step is pinned there.
    Pin,
    /// `health::drag_need`.
    Need,
}

/// Who may see a thing beyond your window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Privacy {
    /// Under each project's consent, for agents.
    Admin,
    /// Never beyond the window: meals, naps, the night, doses, errands.
    Health,
    /// Your hours and the time kept for you: never beyond the window.
    Frame,
}

/// How a thing is drawn: never red.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tone {
    Plain,
    Quiet,
    Warm,
    Muted,
}

/// Where a thing lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Source {
    /// An event, a time block or a leg: its own file.
    Calendar { key: String, uid: String, start: i64, recurring: bool, calendar: String },
    Task { uid: String, list: String },
    /// A step of the plan, on `date`; `part` from 1 for a step cut in parts, 0 when whole.
    Plan { uid: String, date: Date, part: u32 },
    /// "meal:1", "nap:0", "sleep" (on its evening), "added-…".
    Needs { date: Date, key: String },
    /// `<medicine>@<Unix seconds>`, the key every dose path uses.
    Dose { key: String },
    DayOff { id: String },
    /// Your hours, the time kept for you, the time kept free.
    Frame,
}

/// An event's id: its file, and for one time of a repeating one, its start.
pub fn event_id(key: &str, start: i64, recurring: bool) -> String {
    if recurring { format!("event:{key}@{start}") } else { format!("event:{key}") }
}

/// A time block's id: its own file, which its drag moves.
pub fn block_id(key: &str, start: i64, recurring: bool) -> String {
    if recurring { format!("block:{key}@{start}") } else { format!("block:{key}") }
}

/// A step's id: its task and its day, never its time, so that a re-lay that
/// moves it keeps its id, its card and its row; a part's number after `#`.
pub fn step_id(uid: &str, date: Date, part: u32) -> String {
    if part == 0 { format!("step:{uid}@{date}") } else { format!("step:{uid}@{date}#{part}") }
}

/// A task done's id, on the day it was done.
pub fn done_id(uid: &str, date: Date) -> String {
    format!("done:{uid}@{date}")
}

/// A meal's, a nap's or a night's id: the key `Needs::change_day` takes, on its day.
pub fn need_id(date: Date, key: &str) -> String {
    format!("need:{date}/{key}")
}

/// A dose's id: routing only, never logged, never written anywhere.
pub fn dose_id(key: &str) -> String {
    format!("dose:{key}")
}

/// "09:00 – 10:30", on the clock in `zone`: every time range, one form.
pub fn range(from: i64, to: i64, zone: &TimeZone) -> String {
    format!("{} – {}", clock(from, zone), clock(to, zone))
}

/// The separator between the parts of a line.
pub const SEPARATOR: &str = " · ";

/// `at` (Unix seconds) on the clock in `zone`: "09:30".
pub fn clock(at: i64, zone: &TimeZone) -> String {
    Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default()
}

/// The plan's days, as items: each step laid (its margins its lighter ends;
/// held when pinned, else planned), each step of the day's plan that found
/// no time that day (a `Day` time, said apart, never drawn at a time), each
/// task done, the time kept for you and the free time kept after the last
/// step. An event's blocks, and their margins, are `agenda::items`'; a
/// meal's and a nap's are the window's (its needs adapter, which names them).
pub fn from_day(day: &DayView, date: Date, tasks: &[crate::tasks::Task]) -> Vec<Item> {
    let events: std::collections::BTreeSet<&str> = day.blocks.iter().filter(|b| b.kind == "event").map(|b| b.key.as_str()).collect();
    let task_of = |uid: &str| tasks.iter().find(|t| t.uid == uid);
    let mut out: Vec<Item> = Vec::new();
    let mut gains = 0;
    let blocks = &day.blocks;
    let mut i = 0;
    while i < blocks.len() {
        let block = &blocks[i];
        match block.kind {
            "task" | "margin" if !events.contains(block.key.as_str()) => {
                // A step and its margins, at both ends: one item, its margins its lighter ends.
                let same = |b: &crate::dayview::Block| b.key == block.key && b.part == block.part && (b.kind == "task" || b.kind == "margin");
                let mut parts: Vec<&crate::dayview::Block> = blocks.iter().filter(|b| same(b)).collect();
                parts.sort_by_key(|b| b.start);
                let Some(proper) = parts.iter().find(|b| b.kind == "task") else {
                    i += 1;
                    continue;
                };
                let id = step_id(&block.key, date, block.part);
                if out.iter().any(|item| item.id == id) {
                    i += 1;
                    continue;
                }
                let start = parts.first().map_or(proper.start, |b| b.start).min(proper.start);
                let end = parts.last().map_or(proper.end, |b| b.end).max(proper.end);
                let task = task_of(&block.key);
                out.push(Item {
                    id,
                    kind: Kind::Step,
                    time: Time::Span { start, at: proper.start, until: proper.end, end },
                    certainty: if proper.pinned { Certainty::Held } else { Certainty::Planned },
                    source: Source::Plan { uid: block.key.clone(), date, part: block.part },
                    can: step_can(task),
                    privacy: Privacy::Admin,
                    title: proper.title.clone(),
                    line: String::new(),
                    said: String::new(),
                    tone: Tone::Plain,
                    colour: None,
                    address: crate::links::task_uri(&block.key),
                    owner: block.key.clone(),
                    holds_time: true,
                });
            }
            "done" => out.push(Item {
                id: done_id(&block.key, date),
                kind: Kind::Done,
                time: Time::span(block.start, block.end),
                certainty: Certainty::Fixed,
                source: Source::Task { uid: block.key.clone(), list: task_of(&block.key).map(|t| t.list.clone()).unwrap_or_default() },
                can: Can { edit: Form::Task, drag: Move::None, ends: false, delete: false, tick: false, why: String::new() },
                privacy: Privacy::Admin,
                title: block.title.clone(),
                line: String::new(),
                said: String::new(),
                tone: Tone::Muted,
                colour: None,
                address: crate::links::task_uri(&block.key),
                owner: block.key.clone(),
                holds_time: false,
            }),
            "gain" | "slack" => {
                let gain = block.kind == "gain";
                if gain {
                    gains += 1;
                }
                out.push(Item {
                    id: if gain { format!("gain:{date}/{gains}") } else { format!("slack:{date}") },
                    kind: if gain { Kind::Gain } else { Kind::Slack },
                    time: Time::span(block.start, block.end),
                    certainty: Certainty::Planned,
                    source: Source::Frame,
                    can: Can::nothing(),
                    privacy: Privacy::Frame,
                    title: block.title.clone(),
                    line: block.note.clone(),
                    said: String::new(),
                    tone: Tone::Quiet,
                    colour: None,
                    address: String::new(),
                    owner: String::new(),
                    holds_time: true,
                });
            }
            _ => {}
        }
        i += 1;
    }
    // The day's steps that found no time that day: in its plan all the same, at no time.
    for uid in &day.unlaid {
        let task = task_of(uid);
        out.push(Item {
            id: step_id(uid, date, 0),
            kind: Kind::Step,
            time: Time::Day { date, until: date },
            certainty: Certainty::Planned,
            source: Source::Plan { uid: uid.clone(), date, part: 0 },
            can: step_can(task),
            privacy: Privacy::Admin,
            title: task.map(|t| t.title.clone()).unwrap_or_default(),
            line: String::new(),
            said: String::new(),
            tone: Tone::Quiet,
            colour: None,
            address: crate::links::task_uri(uid),
            owner: uid.clone(),
            holds_time: false,
        });
    }
    out
}

/// What may be done to a step: its form, and a drag that pins it there
/// (`blocks::pin`), never in a list you only read (its writer's rule).
fn step_can(task: Option<&crate::tasks::Task>) -> Can {
    let read_only = task.is_some_and(|t| t.read_only);
    Can { edit: Form::Task, drag: if read_only { Move::None } else { Move::Pin }, ends: false, delete: false, tick: !read_only, why: String::new() }
}

/// A task once a day (the unified agenda, §3.4): a time block that holds
/// its task's step, or its task done, gives way to it; a block over whose
/// task is still open is not shown (it pins nothing: its task is laid again
/// from now); any other block stays, a block of the same task still to come
/// included. `open` says whether a task (by UID) is still to do.
pub fn merge(items: Vec<Item>, now: i64, open: &dyn Fn(&str) -> bool) -> Vec<Item> {
    let held: Vec<(String, i64, i64)> = items
        .iter()
        .filter(|item| matches!(item.kind, Kind::Step | Kind::Done))
        .filter_map(|item| match item.time {
            Time::Span { at, until, .. } => Some((item.owner.clone(), at, until)),
            _ => None,
        })
        .collect();
    items
        .into_iter()
        .filter(|item| {
            if item.kind != Kind::Block {
                return true;
            }
            let Time::Span { at, until, .. } = item.time else { return true };
            let taken = held.iter().any(|(owner, from, to)| *owner == item.owner && *from < until && *to > at);
            let over_and_open = until <= now && open(&item.owner);
            !taken && !over_and_open
        })
        .collect()
}

/// The configuration's days off, as items: from their first day to their last.
pub fn days_off(time_off: &[crate::config::TimeOff]) -> Vec<Item> {
    time_off
        .iter()
        .filter(|off| off.until >= off.from)
        .map(|off| {
            let id = if off.id.is_empty() { off.from.to_string() } else { off.id.clone() };
            Item {
                id: format!("off:{id}"),
                kind: Kind::DayOff,
                time: Time::Day { date: off.from, until: off.until },
                certainty: Certainty::Fixed,
                source: Source::DayOff { id: off.id.clone() },
                can: Can { edit: Form::DayOff, drag: Move::None, ends: false, delete: false, tick: false, why: String::new() },
                privacy: Privacy::Admin,
                title: off.label.clone(),
                line: String::new(),
                said: String::new(),
                tone: Tone::Quiet,
                colour: None,
                address: String::new(),
                owner: String::new(),
                holds_time: false,
            }
        })
        .collect()
}

/// What may reach an AI agent: admin items that consent opens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForAgents(Vec<Item>);

impl ForAgents {
    /// Every health and frame item left out (meals, naps, the night, doses,
    /// errands, your hours, the time kept for you), then what consent keeps
    /// back (`allows`: false for an item an agent may not see); each item's
    /// line and spoken name emptied, which Rust words from other things'
    /// titles. With how many things consent kept back: distinct owners (a
    /// task's step and its block count once), never a health item.
    pub fn of(items: Vec<Item>, allows: impl Fn(&Item) -> bool) -> (ForAgents, usize) {
        let mut kept_back = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for mut item in items.into_iter().filter(|item| item.privacy == Privacy::Admin && !item.kind.is_health()) {
            if !allows(&item) {
                kept_back.insert(if item.owner.is_empty() { item.address.clone() } else { item.owner.clone() });
                continue;
            }
            item.line.clear();
            item.said.clear();
            out.push(item);
        }
        (ForAgents(out), kept_back.len())
    }

    /// The items an agent may see.
    pub fn items(&self) -> &[Item] {
        &self.0
    }
}

/// One day of a view: what it holds, placed (the window's agenda, version 2).
#[derive(Debug, Clone, Serialize)]
pub struct Day {
    /// "2026-10-05".
    pub date: String,
    /// "Today", "Tomorrow", "Monday 5 October".
    pub title: String,
    pub today: bool,
    pub past: bool,
    /// The day's midnight, Unix seconds.
    pub start: i64,
    /// Its hours given to work or admin, in minutes from midnight; -1 without.
    pub hours_from: i32,
    pub hours_to: i32,
    /// Whole days and what spans the day: its head.
    pub head: Vec<Placed>,
    /// What happens at a time, side by side where things overlap.
    pub timed: Vec<Placed>,
}

/// An item as a day places it.
#[derive(Debug, Clone, Serialize)]
pub struct Placed {
    pub item: Item,
    /// "09:00 – 10:30", "All day", "Until 12:00": its time on that day.
    pub when: String,
    /// The thing itself in that day's hours, in minutes from midnight.
    pub from_minute: i32,
    pub to_minute: i32,
    /// Whole day, or spanning the whole of that day: at the head.
    pub all_day: bool,
    /// Its column among `columns`, beside what overlaps it.
    pub column: u32,
    pub columns: u32,
}

/// The days from `from`, `count` of them, each with the items it holds,
/// placed: those of a whole day (or spanning it) at its head, the others at
/// their times, side by side where they overlap (`dayview::side_by_side`; one
/// shorter than a quarter of an hour takes a quarter of an hour, as drawn).
/// A thing over several days shows on each. `today` names today and tomorrow.
pub fn days(items: &[Item], from: Date, count: i64, today: Date, zone: &TimeZone, tr: &Translator) -> Vec<Day> {
    let mut out = Vec::new();
    for offset in 0..count {
        let Ok(date) = from.checked_add(jiff::Span::new().days(offset)) else { break };
        let start = midnight(date, zone);
        let end = date.tomorrow().map_or(start + 86_400, |d| midnight(d, zone));
        let mut placed: Vec<Placed> = Vec::new();
        for item in items {
            let (at, until) = item.time.proper(zone);
            // Within the day: an instant at its start counts, one ending as it starts does not.
            if !(at < end && until > start && !(until == start && at < start)) && !(at == until && at >= start && at < end) {
                continue;
            }
            let whole = matches!(item.time, Time::Day { .. }) || (at <= start && until >= end);
            placed.push(Placed {
                item: item.clone(),
                when: when_on(at, until, whole, start, end, zone, tr),
                from_minute: i32::try_from((at.max(start) - start) / 60).unwrap_or(0),
                to_minute: i32::try_from((until.min(end) - start) / 60).unwrap_or(1440),
                all_day: whole,
                column: 0,
                columns: 1,
            });
        }
        placed.sort_by(|a, b| (a.item.time.proper(zone).0, !a.all_day, &a.item.title).cmp(&(b.item.time.proper(zone).0, !b.all_day, &b.item.title)));
        let (head, mut timed): (Vec<Placed>, Vec<Placed>) = placed.into_iter().partition(|p| p.all_day);
        let spans: Vec<(i64, i64)> = timed.iter().map(|p| (i64::from(p.from_minute), i64::from(p.to_minute.max(p.from_minute + 15)))).collect();
        for (p, (column, columns)) in timed.iter_mut().zip(crate::dayview::side_by_side(&spans)) {
            p.column = column;
            p.columns = columns;
        }
        let title = if date == today {
            tr.text("agenda-today", None)
        } else if Some(date) == today.tomorrow().ok() {
            tr.text("agenda-tomorrow", None)
        } else {
            tr.day(date)
        };
        out.push(Day { date: date.to_string(), title, today: date == today, past: date < today, start, hours_from: -1, hours_to: -1, head, timed });
    }
    out
}

/// A thing's time on one day (`day_start`..`day_end`): its hours, "All day",
/// or how it goes on from the day before or into the next.
pub fn when_on(at: i64, until: i64, whole: bool, day_start: i64, day_end: i64, zone: &TimeZone, tr: &Translator) -> String {
    let starts_today = at >= day_start;
    let ends_today = until <= day_end;
    if whole || (!starts_today && !ends_today) {
        return tr.text("agenda-all-day", None);
    }
    let mut args = crate::i18n::args();
    match (starts_today, ends_today) {
        (true, true) if until == at => clock(at, zone),
        (true, true) => range(at, until, zone),
        (true, false) => {
            args.set("time", clock(at, zone));
            tr.text("agenda-from", Some(&args))
        }
        (false, _) => {
            args.set("time", clock(until, zone));
            tr.text("agenda-until", Some(&args))
        }
    }
}

/// Each day's hours given to work or your admin, in minutes from its
/// midnight (docs/areas.md): a planning shows those, and what happens
/// outside them, rather than the night.
pub fn set_hours(days: &mut [Day], windows: &[crate::window::AdminWindow], zone: &TimeZone) {
    for day in days {
        let Ok(date) = day.date.parse::<Date>() else { continue };
        if let Some((opening, closing)) = crate::window::hours_on(windows, date, zone) {
            let minute = |z: &Zoned| i32::try_from((z.timestamp().as_second() - day.start) / 60).unwrap_or(0).clamp(0, 1440);
            day.hours_from = minute(&opening);
            day.hours_to = minute(&closing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dayview::Block;

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> i64 {
        text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(paris()).unwrap().timestamp().as_second()
    }

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn block(kind: &'static str, key: &str, from: &str, to: &str, part: u32, pinned: bool) -> Block {
        Block { start: at(from), end: at(to), kind, title: format!("{key} title"), key: key.into(), energy: String::new(), location: String::new(), column: 0, columns: 1, part, pinned, note: String::new(), read_only: false, recurring: false, with_others: false }
    }

    fn laid(blocks: Vec<Block>, unlaid: Vec<String>) -> DayView {
        DayView { blocks, unlaid, ..DayView::default() }
    }

    /// A step's id names its task and its day, never its time: laid again
    /// elsewhere, it keeps its id (§3.4); an event's names its file, and
    /// for one time of a repeating one, that time.
    #[test]
    fn ids_stay_through_a_re_lay() {
        let date = day("2026-10-05");
        let first = from_day(&laid(vec![block("task", "call", "2026-10-05T09:00", "2026-10-05T09:30", 0, false)], vec![]), date, &[]);
        let again = from_day(&laid(vec![block("task", "call", "2026-10-05T14:00", "2026-10-05T14:30", 0, false)], vec![]), date, &[]);
        assert_eq!(first[0].id, again[0].id);
        assert_eq!(first[0].id, "step:call@2026-10-05");
        assert_eq!(step_id("long", date, 2), "step:long@2026-10-05#2");
        assert_eq!(event_id("/c/a.ics", 100, false), "event:/c/a.ics");
        assert_eq!(event_id("/c/a.ics", 100, true), "event:/c/a.ics@100");
    }

    /// A step and its margins are one item, its margins its lighter ends; an
    /// event's own blocks and margins are left to `agenda::items`; the steps
    /// that found no time stay in the day's plan at no time.
    #[test]
    fn a_step_takes_its_margins_as_its_ends() {
        let date = day("2026-10-05");
        let view = laid(
            vec![
                block("margin", "visit", "2026-10-05T09:00", "2026-10-05T09:15", 0, true),
                block("task", "visit", "2026-10-05T09:15", "2026-10-05T10:00", 0, true),
                block("margin", "visit", "2026-10-05T10:00", "2026-10-05T10:20", 0, true),
                block("event", "/c/m.ics", "2026-10-05T11:00", "2026-10-05T12:00", 0, false),
                block("margin", "/c/m.ics", "2026-10-05T12:00", "2026-10-05T12:10", 0, false),
                block("done", "letter", "2026-10-05T08:00", "2026-10-05T08:30", 0, false),
            ],
            vec!["form".into()],
        );
        let items = from_day(&view, date, &[]);
        let kinds: Vec<(Kind, &str)> = items.iter().map(|i| (i.kind, i.owner.as_str())).collect();
        assert_eq!(kinds, vec![(Kind::Step, "visit"), (Kind::Done, "letter"), (Kind::Step, "form")]);
        assert_eq!(items[0].time, Time::Span { start: at("2026-10-05T09:00"), at: at("2026-10-05T09:15"), until: at("2026-10-05T10:00"), end: at("2026-10-05T10:20") });
        assert_eq!(items[0].certainty, Certainty::Held, "pinned: held");
        assert_eq!(items[2].time, Time::Day { date, until: date }, "no time found: at no time");
        assert!(items.iter().all(|i| i.privacy == Privacy::Admin && i.address.starts_with("sioul:task/")));
    }

    fn block_item(owner: &str, from: &str, to: &str) -> Item {
        Item {
            id: block_id(&format!("/c/{owner}-{from}.ics"), 0, false),
            kind: Kind::Block,
            time: Time::span(at(from), at(to)),
            certainty: Certainty::Held,
            source: Source::Calendar { key: format!("/c/{owner}.ics"), uid: owner.into(), start: at(from), recurring: false, calendar: "c".into() },
            can: Can::nothing(),
            privacy: Privacy::Admin,
            title: owner.into(),
            line: String::new(),
            said: String::new(),
            tone: Tone::Plain,
            colour: None,
            address: crate::links::task_uri(owner),
            owner: owner.into(),
            holds_time: true,
        }
    }

    /// A task once a day (§3.4): the block its step is laid in gives way to
    /// the step, a block that held a task done to the task done; a block over
    /// whose task is still open is not shown; a second block still to come
    /// stays; no task shows twice over one span.
    #[test]
    fn a_task_shows_once_over_a_span() {
        let date = day("2026-10-05");
        let mut items = from_day(&laid(vec![block("task", "a", "2026-10-05T10:00", "2026-10-05T10:45", 0, true), block("done", "b", "2026-10-05T08:00", "2026-10-05T08:30", 0, false)], vec![]), date, &[]);
        items.push(block_item("a", "2026-10-05T10:00", "2026-10-05T10:45"));
        items.push(block_item("a", "2026-10-05T15:00", "2026-10-05T15:30"));
        items.push(block_item("b", "2026-10-05T08:00", "2026-10-05T08:30"));
        items.push(block_item("c", "2026-10-05T07:00", "2026-10-05T07:30"));
        let now = at("2026-10-05T09:00");
        let merged = merge(items, now, &|uid| uid != "b");
        let shown: Vec<(Kind, &str)> = merged.iter().map(|i| (i.kind, i.owner.as_str())).collect();
        assert_eq!(shown, vec![(Kind::Step, "a"), (Kind::Done, "b"), (Kind::Block, "a")], "the later block of a stays, c's over and open goes");
        for (n, one) in merged.iter().enumerate() {
            for other in &merged[n + 1..] {
                let (a, b) = (one.time.proper(&paris()), other.time.proper(&paris()));
                assert!(!(one.owner == other.owner && a.0 < b.1 && b.0 < a.1), "{one:?} and {other:?} overlap");
            }
        }
    }

    /// Agents get admin items that consent opens, never health or frame
    /// ones, their lines emptied; what consent keeps back is counted by
    /// owner, a health item never.
    #[test]
    fn agents_get_admin_items_that_consent_opens() {
        let date = day("2026-10-05");
        let mut items = from_day(&laid(vec![block("task", "open", "2026-10-05T10:00", "2026-10-05T10:30", 0, false), block("task", "closed", "2026-10-05T11:00", "2026-10-05T11:30", 0, false), block("gain", "gain:after", "2026-10-05T12:00", "2026-10-05T12:20", 0, false)], vec!["closed".into()]), date, &[]);
        items[0].line = "Next: something of another task".into();
        let mut meal = block_item("meal", "2026-10-05T12:30", "2026-10-05T13:00");
        meal.kind = Kind::Meal;
        meal.privacy = Privacy::Health;
        items.push(meal);
        let (agents, kept) = ForAgents::of(items, |item| item.owner != "closed");
        let given: Vec<&str> = agents.items().iter().map(|i| i.owner.as_str()).collect();
        assert_eq!(given, vec!["open"]);
        assert_eq!(kept, 1, "the closed task's laid step and its unlaid part count once");
        assert!(agents.items().iter().all(|i| i.line.is_empty() && i.said.is_empty()));
    }

    /// The days off: one item from their first day to their last, named by their label.
    #[test]
    fn days_off_span_their_days() {
        let off = crate::config::TimeOff { id: String::new(), from: day("2026-12-24"), until: day("2026-12-26"), label: "Holidays".into() };
        let items = days_off(&[off]);
        assert_eq!(items[0].id, "off:2026-12-24");
        assert_eq!(items[0].time, Time::Day { date: day("2026-12-24"), until: day("2026-12-26") });
        let tr = Translator::new("en");
        let shown = days(&items, day("2026-12-23"), 5, day("2026-12-23"), &paris(), &tr);
        let heads: Vec<usize> = shown.iter().map(|d| d.head.len()).collect();
        assert_eq!(heads, vec![0, 1, 1, 1, 0]);
    }

    /// Every time range reads one way.
    #[test]
    fn a_range_reads_one_way() {
        assert_eq!(range(at("2026-10-05T09:00"), at("2026-10-05T10:30"), &paris()), "09:00 – 10:30");
    }

    fn occurrence(key: &str, from: &str, to: &str) -> crate::agenda::Occurrence {
        crate::agenda::Occurrence { key: key.into(), uid: key.into(), summary: key.into(), start: at(from), end: at(to), calendar: "c".into(), ..Default::default() }
    }

    /// The agenda, made from the one model, places every event as it did
    /// (`view::agenda`): its days, its time, the whole days at the head, side
    /// by side where events overlap, a task's block as the task it is.
    #[test]
    fn the_agenda_places_events_as_before() {
        let zone = paris();
        let tr = Translator::new("en");
        let mut all_day = occurrence("holiday", "2026-10-06T00:00", "2026-10-08T00:00");
        all_day.all_day = true;
        let mut margins = occurrence("doctor", "2026-10-05T14:00", "2026-10-05T14:30");
        margins.margins = crate::demands::Margins { before: 30, after: 15 };
        let mut block = occurrence("block", "2026-10-06T10:00", "2026-10-06T10:45");
        block.task = "t1".into();
        let mut occurrences = vec![
            occurrence("a", "2026-10-05T09:00", "2026-10-05T10:00"),
            occurrence("b", "2026-10-05T09:30", "2026-10-05T11:00"),
            occurrence("c", "2026-10-05T10:00", "2026-10-05T10:30"),
            occurrence("instant", "2026-10-05T12:00", "2026-10-05T12:00"),
            margins,
            occurrence("trip", "2026-10-05T18:00", "2026-10-07T09:00"),
            all_day,
            block,
        ];
        occurrences.sort_by(|a, b| (a.start, !a.all_day, &a.summary).cmp(&(b.start, !b.all_day, &b.summary)));
        crate::agenda::mark_tasks(&mut occurrences, &[crate::tasks::Task { uid: "t1".into(), list: "L".into(), ..Default::default() }]);
        let from = day("2026-10-05");
        let before = crate::view::agenda(&occurrences, from, 4, &tr, &zone);
        let items = crate::agenda::items(&occurrences, &zone, &|_| false);
        // `view::agenda` names today by the clock.
        let now = days(&items, from, 4, Timestamp::now().to_zoned(zone.clone()).date(), &zone, &tr);
        for (old, new) in before.days.iter().zip(&now) {
            let was: Vec<(String, i64, String, bool, i32, i32, u32, u32)> = old.events.iter().map(|e| (e.key.clone(), e.start, e.when.clone(), e.all_day, e.from_minute, e.to_minute, if e.all_day { 0 } else { e.column }, if e.all_day { 1 } else { e.columns })).collect();
            let is: Vec<(String, i64, String, bool, i32, i32, u32, u32)> = new
                .head
                .iter()
                .chain(&new.timed)
                .map(|p| {
                    let Source::Calendar { key, start, .. } = &p.item.source else { panic!("{p:?}") };
                    (key.clone(), *start, p.when.clone(), p.all_day, p.from_minute, p.to_minute, p.column, p.columns)
                })
                .collect();
            let (mut was, mut is) = (was, is);
            was.sort();
            is.sort();
            assert_eq!(was, is, "{}", old.date);
            assert_eq!((old.title.clone(), old.date.clone(), old.today, old.start), (new.title.clone(), new.date.clone(), new.today, new.start));
        }
    }

    /// Each event says what its writer lets change it: nothing in a calendar
    /// only read, nothing of a meeting with others, no drag of a whole day;
    /// a task's block is changed as its task, never deleted here, health's
    /// when its task is an errand.
    #[test]
    fn each_event_says_what_may_change_it() {
        let zone = paris();
        let mut read_only = occurrence("read-only", "2026-10-05T09:00", "2026-10-05T10:00");
        read_only.read_only = true;
        let mut meeting = occurrence("meeting", "2026-10-05T11:00", "2026-10-05T12:00");
        meeting.with_others = true;
        let mut whole = occurrence("whole", "2026-10-05T00:00", "2026-10-06T00:00");
        whole.all_day = true;
        let mut block = occurrence("block", "2026-10-05T14:00", "2026-10-05T14:45");
        block.task = "t1".into();
        let mut errand = occurrence("errand", "2026-10-05T16:00", "2026-10-05T16:30");
        errand.task = "pharmacy".into();
        let own = occurrence("own", "2026-10-05T18:00", "2026-10-05T19:00");
        let items = crate::agenda::items(&[read_only, meeting, whole, block, errand, own], &zone, &|uid| uid == "pharmacy");
        let can: Vec<(Kind, Move, bool, bool, Privacy)> = items.iter().map(|i| (i.kind, i.can.drag, i.can.delete, i.can.why.is_empty(), i.privacy)).collect();
        assert_eq!(
            can,
            vec![
                (Kind::Event, Move::None, false, false, Privacy::Admin),
                (Kind::Event, Move::None, false, false, Privacy::Admin),
                (Kind::Event, Move::None, true, true, Privacy::Admin),
                (Kind::Block, Move::Event, false, true, Privacy::Admin),
                (Kind::Block, Move::Event, false, true, Privacy::Health),
                (Kind::Event, Move::Event, true, true, Privacy::Admin),
            ]
        );
        assert_eq!((items[3].can.edit, items[3].owner.as_str(), items[3].address.as_str()), (Form::Task, "t1", "sioul:task/t1"));
        assert_eq!(items[5].address, "sioul:event/own");
        assert_eq!(items[2].time, Time::Day { date: day("2026-10-05"), until: day("2026-10-05") });
    }
}
