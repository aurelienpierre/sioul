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

/// When a layout was read, as the files say it: this program's own writes
/// (`written`), and the calendars' folders' last changes, whichever program
/// made them (`vdir::calendars_stamp`). Taken before anything is read: a
/// layout whose stamp differs from now's is older than some write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub written: u64,
    pub folders: u64,
}

impl Stamp {
    pub fn now() -> Stamp {
        Stamp { written: written(), folders: crate::vdir::calendars_stamp() }
    }
}

/// One thing that has a time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// Stable across reloads and re-plans (`event_id`, `step_id`…): what
    /// keeps a card open, a thing chosen, a row in place.
    pub id: String,
    pub kind: Kind,
    pub time: Time,
    /// Fixed unless said (left out of the JSON then).
    #[serde(skip_serializing_if = "Certainty::is_fixed")]
    pub certainty: Certainty,
    /// Where it lives: what its card reads, and what its own path takes.
    pub source: Source,
    /// What may be done to it here, decided by its writer's own rule.
    pub can: Can,
    /// Admin unless said (left out of the JSON then).
    #[serde(skip_serializing_if = "Privacy::is_admin")]
    pub privacy: Privacy,
    /// Its words on the timeline: an event's title, a task's, a meal's name.
    pub title: String,
    /// One line under it, worded in Rust; "" when it has none (left out of the JSON then).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub line: String,
    /// Its whole name for a screen reader; "" until a view words it.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub said: String,
    /// Plain unless said (left out of the JSON then).
    #[serde(skip_serializing_if = "Tone::is_plain")]
    pub tone: Tone,
    /// A calendar's or a list's colour; None for Sioul's own kinds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// Its `sioul:` address, for ties and for agents; "" for every health kind.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub address: String,
    /// What it belongs to: a block's or a step's task, by UID (an event's, a
    /// meeting with others that is a task's time block); "" for none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub owner: String,
    /// It takes time from the plan, the hours and the meals (`Occurrence::holds_time`):
    /// the layouts' own, never drawn (left out of the JSON).
    #[serde(skip)]
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

impl Certainty {
    fn is_fixed(&self) -> bool {
        *self == Certainty::Fixed
    }
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
    /// Its end can be dragged (each of these left out of the JSON when false).
    #[serde(skip_serializing_if = "is_false")]
    pub ends: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub delete: bool,
    /// Its checklist can be ticked here.
    #[serde(skip_serializing_if = "is_false")]
    pub tick: bool,
    /// Why it cannot be changed here, in words; "" when it can.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub why: String,
}

fn is_false(value: &bool) -> bool {
    !*value
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

impl Privacy {
    fn is_admin(&self) -> bool {
        *self == Privacy::Admin
    }
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

impl Tone {
    fn is_plain(&self) -> bool {
        *self == Tone::Plain
    }
}

/// Where a thing lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Source {
    /// An event, a time block or a leg: its own file; its calendar by its name
    /// and by its place, "account/id" (`collection_of`): two calendars may share a name.
    Calendar { key: String, uid: String, start: i64, recurring: bool, calendar: String, collection: String },
    Task { uid: String, list: String },
    /// A step of the plan, on `date`; `part` from 1 for a step cut in parts, 0 when whole.
    Plan { uid: String, date: Date, part: u32 },
    /// "meal:1", "nap:0", "sleep" (on its evening), "added-…".
    Needs { date: Date, key: String },
    /// `<medicine>@<Unix seconds>`, the key every dose path uses.
    Dose { key: String },
    DayOff { id: String },
    /// A date asked of you (`dated`): its kind and what its card reads.
    Dated { kind: crate::dated::Kind, key: String },
    /// Your hours, the time kept for you, the time kept free.
    Frame,
}

/// An event's id: its file (below the calendars' folder: the same file,
/// a shorter name), and for one time of a repeating one, its start.
pub fn event_id(key: &str, start: i64, recurring: bool) -> String {
    let file = below_calendars(key);
    if recurring { format!("event:{file}@{start}") } else { format!("event:{file}") }
}

/// A time block's id: its own file, which its drag moves.
pub fn block_id(key: &str, start: i64, recurring: bool) -> String {
    let file = below_calendars(key);
    if recurring { format!("block:{file}@{start}") } else { format!("block:{file}") }
}

/// The calendar a file lies in, "account/id" ("local/work"), as the
/// calendars' choices name it (`pim::calendars`); its folder as it is when it
/// lies elsewhere.
pub fn collection_of(key: &str) -> String {
    below_calendars(key).rsplit_once('/').map_or_else(String::new, |(folder, _)| folder.to_string())
}

/// A calendar's file named below the calendars' folder ("local/work/a.ics"),
/// as it is when it lies elsewhere.
fn below_calendars(key: &str) -> &str {
    static ROOT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let root = ROOT.get_or_init(|| format!("{}/", crate::vdir::Kind::Calendars.root().display()));
    key.strip_prefix(root.as_str()).unwrap_or(key)
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
    /// Its hours by what they are for (work, admin, leisure), as the plan
    /// has them (`dress`); none in the views that do not draw them.
    pub hours: Vec<crate::dayview::Stretch>,
    /// In quiet time, a coming day's steps of what is hidden now, one band per
    /// stretch of hours ("Work, 09:00 – 12:00"); those steps in `hidden`, for
    /// "Show anyway" (decisions g and 11 of the unified agenda).
    pub bands: Vec<Placed>,
    pub hidden: Vec<Placed>,
    /// Its doses: today's by key and time (their state is the window's, from
    /// the Health job's rows), a day to come's plainly; never a name.
    pub doses: Vec<Placed>,
    /// The night's parts on that day: the morning's, to waking, and tonight's,
    /// from winding down.
    pub edges: Vec<Placed>,
    /// Within the plan's reach (`plan::HORIZON_DAYS`): its steps are laid.
    pub planned: bool,
    /// The lists' line: "The plan: Call the CAF first"; "" with no step.
    pub plan: String,
    /// Today: the steps of today's plan that find no time before the day ends, said.
    pub more: String,
    /// Today: why it holds what it holds (`capacity`), in words.
    pub said: Vec<String>,
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
    /// Whole day, or spanning the whole of that day: at the head (left out when not).
    #[serde(skip_serializing_if = "is_false")]
    pub all_day: bool,
    /// Its column among `columns`, beside what overlaps it (left out when alone: 0 of 1).
    #[serde(skip_serializing_if = "is_zero")]
    pub column: u32,
    #[serde(skip_serializing_if = "is_one")]
    pub columns: u32,
    /// Outside the hours the view shows (a dose or a meal inside the night):
    /// drawn at their top or bottom edge, with its time; "" inside them (`dress`).
    #[serde(skip_serializing_if = "str::is_empty")]
    pub edge: &'static str,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

fn is_one(value: &u32) -> bool {
    *value == 1
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
            // The night on each day it touches, from its winding down: a bedtime after
            // midnight keeps its evening (review of 3a2fbd4, later 2).
            let (on, off) = if item.kind == Kind::Night { item.time.whole(zone) } else { (at, until) };
            // Within the day: an instant at its start counts, one ending as it starts does not.
            if !(on < end && off > start && !(off == start && on < start)) && !(on == off && on >= start && on < end) {
                continue;
            }
            let whole = matches!(item.time, Time::Day { .. }) || (at <= start && until >= end);
            placed.push(Placed {
                item: item.clone(),
                when: when_on(at, until, whole, start, end, zone, tr),
                // A date asked sits at the head of its day, its hour said when it has one
                // ("17:00"): a mark, never a card that would seem to take time.
                // The thing itself within the day: a bedtime past midnight at its end.
                from_minute: i32::try_from((at.max(start).min(end) - start) / 60).unwrap_or(0),
                to_minute: i32::try_from((until.min(end).max(start) - start) / 60).unwrap_or(1440),
                all_day: whole || item.kind == Kind::Dated,
                column: 0,
                columns: 1,
                edge: "",
            });
        }
        // A date asked first in its day's head: never a "1 more" behind holidays or a trip
        // (review of part 4, later 2); the rest by when they start.
        placed.sort_by(|a, b| (a.item.kind != Kind::Dated, a.item.time.proper(zone).0, !a.all_day, &a.item.title).cmp(&(b.item.kind != Kind::Dated, b.item.time.proper(zone).0, !b.all_day, &b.item.title)));
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
        out.push(Day {
            date: date.to_string(),
            title,
            today: date == today,
            past: date < today,
            start,
            hours_from: -1,
            hours_to: -1,
            head,
            timed,
            hours: Vec::new(),
            bands: Vec::new(),
            hidden: Vec::new(),
            doses: Vec::new(),
            edges: Vec::new(),
            planned: false,
            plan: String::new(),
            more: String::new(),
            said: Vec::new(),
        });
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

/// What `dress` needs beyond the items: each day's hours, what is hidden
/// now, the plan's reach and today's own lines.
pub struct Dressing<'a> {
    pub today: Date,
    /// Now, Unix seconds: today's plan line names the next step from now.
    pub now: i64,
    pub zone: &'a TimeZone,
    pub tr: &'a Translator,
    /// A day's hours by what they are for (`plan::Settings::hours_on`).
    pub hours: &'a dyn Fn(Date) -> Vec<crate::dayview::Stretch>,
    /// The last day the plan lays steps on.
    pub horizon: Date,
    /// Whether a task, by UID, is hidden now (quiet time): a coming day's steps of it make bands.
    pub hidden: &'a dyn Fn(&str) -> bool,
    /// Why today holds what it holds, as the Tasks job dressed it (`capacity`).
    pub said: Vec<String>,
    /// With no night set and no hours: the hour the days start at (`agenda.day_start`).
    pub day_start: i32,
}

/// The hours a view shows, in minutes from each day's midnight, whole hours:
/// the same for every day it shows, so that the hour lines stay in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Frame {
    pub from: i32,
    pub to: i32,
}

/// Half an hour, around the night: the hours shown start before waking and end after bedtime.
const AROUND_NIGHT: i32 = 30;

/// A night gone to bed before noon is the morning's part (it wakes that day);
/// from noon on, the evening's (`frame_of`, and the window's timeline alike).
const NOON: i32 = 12 * 60;

/// The days laid by `days`, dressed for the window's Day and Week (the
/// unified agenda, §4.3, §4.4, §4.7): each day's hours by what they are for;
/// the night's parts as its edges and the doses on their lane, apart from
/// the cards; a coming day's steps of what quiet time hides now folded into
/// one band per stretch of hours, those steps kept apart for "Show anyway";
/// a step at no time said in today's line, never drawn; the lists' line;
/// today's lines; side by side again where what is left overlaps. Returns
/// the hours shown (Health's rule, decision 5): from half an hour before
/// waking to half an hour after bedtime, whole hours, the same for every day,
/// widened to the appointments and the blocks outside them; with no night
/// set, the hours given to something; else from the day's start to 22:00.
/// What else falls outside (a dose, a meal inside the night) is marked to be
/// drawn at the edge with its time, and never stretches the day.
pub fn dress(days: &mut [Day], d: &Dressing) -> Frame {
    for day in days.iter_mut() {
        let Ok(date) = day.date.parse::<Date>() else { continue };
        day.planned = date >= d.today && date <= d.horizon;
        day.hours = (d.hours)(date);
        let mut head = Vec::new();
        let mut timed = Vec::new();
        let mut unlaid = Vec::new();
        for mut placed in std::mem::take(&mut day.head).into_iter().chain(std::mem::take(&mut day.timed)) {
            if matches!(placed.item.kind, Kind::Night | Kind::Dose) {
                (placed.column, placed.columns) = (0, 1);
            }
            match placed.item.kind {
                Kind::Night => day.edges.push(placed),
                Kind::Dose => day.doses.push(placed),
                Kind::Step if matches!(placed.item.time, Time::Day { .. }) => unlaid.push(placed),
                Kind::Step if date > d.today && (d.hidden)(&placed.item.owner) => day.hidden.push(placed),
                _ if placed.all_day => head.push(placed),
                _ => timed.push(placed),
            }
        }
        // Side by side again, what overlaps among what is drawn as cards.
        let spans: Vec<(i64, i64)> = timed.iter().map(|p| (i64::from(p.from_minute), i64::from(p.to_minute.max(p.from_minute + 15)))).collect();
        for (p, (column, columns)) in timed.iter_mut().zip(crate::dayview::side_by_side(&spans)) {
            p.column = column;
            p.columns = columns;
        }
        day.head = head;
        day.timed = timed;
        day.bands = bands_of(day, d);
        // The lists' line: the day's first step (today's first from now), never one hidden now.
        let first = day.timed.iter().filter(|p| p.item.kind == Kind::Step && p.item.time.proper(d.zone).1 > d.now).min_by_key(|p| p.item.time.proper(d.zone).0);
        if let Some(step) = first {
            let mut args = crate::i18n::args();
            args.set("step", step.item.title.clone());
            day.plan = d.tr.text("agenda-plan-first", Some(&args));
        }
        if date == d.today {
            if !unlaid.is_empty() {
                let mut args = d.tr.counted(unlaid.len());
                args.set("steps", unlaid.iter().map(|p| p.item.title.clone()).collect::<Vec<_>>().join(SEPARATOR));
                day.more = d.tr.text("agenda-more", Some(&args));
            }
            day.said = d.said.clone();
        }
    }
    let frame = frame_of(days, d.day_start);
    // What falls outside the hours shown: at their edge, with its time.
    for day in days.iter_mut() {
        for placed in day.timed.iter_mut().chain(day.doses.iter_mut()) {
            let instant = placed.from_minute == placed.to_minute;
            placed.edge = if placed.to_minute <= frame.from && !(instant && placed.from_minute == frame.from) {
                "top"
            } else if placed.from_minute >= frame.to {
                "bottom"
            } else {
                ""
            };
        }
    }
    frame
}

/// A coming day's steps hidden now, one band per stretch of hours that holds
/// them, named by what the hours are for and their time; those of no stretch
/// in one band of their own.
fn bands_of(day: &Day, d: &Dressing) -> Vec<Placed> {
    let mut groups: Vec<(String, Vec<&Placed>)> = Vec::new();
    for placed in &day.hidden {
        let (at, _) = placed.item.time.proper(d.zone);
        let area = day.hours.iter().find(|h| h.start <= at && at < h.end).map(|h| h.kind.split('+').next().unwrap_or("").to_string()).unwrap_or_default();
        match groups.iter_mut().find(|(a, _)| *a == area) {
            Some((_, steps)) => steps.push(placed),
            None => groups.push((area, vec![placed])),
        }
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(n, (area, steps))| {
            let from = steps.iter().map(|p| p.item.time.proper(d.zone).0).min().unwrap_or(day.start);
            let to = steps.iter().map(|p| p.item.time.proper(d.zone).1).max().unwrap_or(from);
            let mut args = crate::i18n::args();
            args.set("time", range(from, to, d.zone));
            let title = if area.is_empty() {
                d.tr.text("agenda-band-other", Some(&args))
            } else {
                args.set("area", d.tr.text(&format!("area-{area}"), None));
                d.tr.text("agenda-band", Some(&args))
            };
            let minute = |t: i64| i32::try_from((t - day.start) / 60).unwrap_or(0).clamp(0, 1440);
            Placed {
                item: Item {
                    id: format!("band:{}/{n}", day.date),
                    kind: Kind::Step,
                    time: Time::span(from, to),
                    certainty: Certainty::Planned,
                    source: Source::Frame,
                    can: Can::nothing(),
                    privacy: Privacy::Frame,
                    title: title.clone(),
                    line: String::new(),
                    said: title,
                    tone: Tone::Quiet,
                    colour: None,
                    address: String::new(),
                    owner: String::new(),
                    holds_time: false,
                },
                when: range(from, to, d.zone),
                from_minute: minute(from),
                to_minute: minute(to),
                all_day: false,
                column: 0,
                columns: 1,
                edge: "",
            }
        })
        .collect()
}

/// The hours shown for `days` (`dress`).
fn frame_of(days: &[Day], day_start: i32) -> Frame {
    let (mut from, mut to) = (i32::MAX, i32::MIN);
    for day in days {
        let minute = |t: i64| i32::try_from((t - day.start) / 60).unwrap_or(0);
        // Waking: a night gone to bed before noon (last evening, or after
        // midnight) ends that day; bedtime: one gone to bed from noon on begins it.
        let wake = day.edges.iter().filter_map(|p| match p.item.time {
            Time::Span { at, until, .. } if minute(at) < NOON && until > day.start && minute(until) <= 25 * 60 => Some(minute(until)),
            _ => None,
        }).min();
        let bed = day.edges.iter().filter_map(|p| match p.item.time {
            Time::Span { at, .. } if minute(at) >= NOON => Some(minute(at)),
            _ => None,
        }).max();
        let night = wake.is_some() || bed.is_some();
        if let Some(wake) = wake {
            from = from.min((wake - AROUND_NIGHT).div_euclid(60) * 60);
        }
        if let Some(bed) = bed {
            to = to.max(((bed + AROUND_NIGHT) as f64 / 60.0).ceil() as i32 * 60);
        }
        if !night {
            for stretch in &day.hours {
                from = from.min(minute(stretch.start).div_euclid(60) * 60);
                to = to.max((minute(stretch.end) as f64 / 60.0).ceil() as i32 * 60);
            }
        }
    }
    // Nothing said of a day: from its start to 22:00.
    let first = day_start.clamp(0, 23) * 60;
    if from == i32::MAX {
        from = first;
    }
    if to == i32::MIN {
        to = 22 * 60;
    }
    if from >= to {
        (from, to) = (from.min(first), to.max(22 * 60));
    }
    // Widened to the appointments and blocks outside them, a pinned step's included.
    for day in days {
        for placed in day.timed.iter().filter(|p| matches!(p.item.kind, Kind::Event | Kind::Block) || (p.item.kind == Kind::Step && p.item.certainty == Certainty::Held)) {
            from = from.min(placed.from_minute.div_euclid(60) * 60);
            to = to.max((placed.to_minute as f64 / 60.0).ceil() as i32 * 60);
        }
    }
    Frame { from: from.clamp(0, 23 * 60), to: to.clamp(from.clamp(0, 23 * 60) + 60, 24 * 60) }
}

/// One frame of the head line (the unified agenda, §4.2): its words hold from
/// `from` until `until` (Unix seconds), by the window's clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeadFrame {
    pub from: i64,
    pub until: i64,
    pub text: String,
    /// Its words for a screen reader.
    pub said: String,
}

/// What the head line is made from.
pub struct HeadInputs<'a> {
    pub now: i64,
    /// The next midnight, Unix seconds: the line is given until then.
    pub next_midnight: i64,
    /// What each moment is for, from now on (`quiet::frames`), in Unix seconds.
    pub moments: &'a [(i64, i64, crate::quiet::Mode)],
    /// Today's things and tomorrow's (events, time blocks, steps, meals, naps, the night, time for you).
    pub today: &'a [Item],
    pub tomorrow: &'a [Item],
    /// Tomorrow morning's alarm, when it rings.
    pub alarm: Option<i64>,
    /// The dates asked of you this week, today and after (`dated::item`): in
    /// work and admin time, the nearest is said in a second sentence.
    pub dates: &'a [Item],
    pub zone: &'a TimeZone,
    pub tr: &'a Translator,
}

/// A thing the head line names: an appointment, a pinned task, a meal, a
/// nap, the winding down; never a proposal of the plan, never a dose.
fn fixed(item: &Item) -> bool {
    matches!(item.kind, Kind::Event | Kind::Block | Kind::Meal | Kind::Nap | Kind::Night) && item.tone != Tone::Muted || (item.kind == Kind::Step && item.certainty == Certainty::Held)
}

/// The head line from now to midnight, as frames given ahead (the unified
/// agenda, §4.2), each holding while what it says holds: at the end of what
/// is under way, at a leaving time, when the next thing starts, when the
/// moment changes, at midnight. In work and admin time: what is under way,
/// then the next fixed thing with its leaving time, then one date asked this
/// week, the nearest, never a contract's notice day nor health's (one thing at
/// a time). At a meal, or in leisure while today still holds something: what
/// is under way and the next fixed thing, no date. Once today holds nothing
/// more (no work or admin time and no appointment or pinned task left; meals,
/// naps and the night do not count, so the evening with a night set speaks of
/// tomorrow: decided in part 4, nothing pressing in the evening): tomorrow's
/// first appointment and its leaving time, and the alarm. In sleep, a pause
/// or Free time: the moment alone, until when. No dose, no proposal of the plan.
pub fn head_line(i: &HeadInputs) -> Vec<HeadFrame> {
    let mut bounds: Vec<i64> = vec![i.now, i.next_midnight];
    for item in i.today.iter().filter(|item| fixed(item) || item.kind == Kind::Gain) {
        if let Time::Span { start, at, end, .. } = item.time {
            bounds.extend([start, at, end]);
        }
    }
    for (from, until, _) in i.moments {
        bounds.extend([*from, *until]);
    }
    // With no hours set, the day's work ends at 18:00 for the line (`head_at`).
    if i.moments.iter().any(|(_, _, mode)| mode.time == crate::areas::Time::Any) {
        bounds.extend(evening_of(i.now, i.zone));
    }
    bounds.retain(|b| *b >= i.now && *b <= i.next_midnight);
    bounds.sort_unstable();
    bounds.dedup();
    let mut out: Vec<HeadFrame> = Vec::new();
    for pair in bounds.windows(2) {
        let (from, until) = (pair[0], pair[1]);
        let text = head_at(i, from);
        match out.last_mut() {
            Some(last) if last.text == text && last.until == from => last.until = until,
            _ => out.push(HeadFrame { from, until, said: text.clone(), text }),
        }
    }
    out.retain(|frame| !frame.text.is_empty());
    out
}

/// 18:00 of `t`'s day: with no hours set (`Time::Any`, everything comes),
/// the end of the day's work for the head line (review of part 4, later 8).
fn evening_of(t: i64, zone: &TimeZone) -> Option<i64> {
    let day = jiff::Timestamp::from_second(t).ok()?.to_zoned(zone.clone()).date();
    Some(day.at(18, 0, 0, 0).to_zoned(zone.clone()).ok()?.timestamp().as_second())
}

/// Work and admin for the head line: their hours, and with no hours set
/// (`Time::Any`) the whole day, its work until 18:00 (`evening_of`).
fn works(time: crate::areas::Time) -> bool {
    matches!(time, crate::areas::Time::Work | crate::areas::Time::Admin | crate::areas::Time::Any)
}

/// The head line's words at `t` (`head_line`).
fn head_at(i: &HeadInputs, t: i64) -> String {
    let tr = i.tr;
    let clock = |at: i64| clock(at, i.zone);
    let moment = i.moments.iter().find(|(from, until, _)| *from <= t && t < *until).map(|(_, _, mode)| mode);
    let until = |mode: &crate::quiet::Mode| mode.until.as_ref().map(|z| z.timestamp().as_second()).filter(|u| *u > t);
    let say = |id: &str, pairs: &[(&str, String)]| {
        let mut args = crate::i18n::args();
        for (name, value) in pairs {
            args.set(*name, value.clone());
        }
        tr.text(id, Some(&args))
    };
    // The night by its winding down, as the needs name it (`need-sleep`).
    let named = |item: &Item| item.title.clone();
    if let Some(mode) = moment {
        use crate::quiet::Reason;
        let held = until(mode).map(clock);
        match mode.reason {
            Reason::FreeTime => return held.map_or_else(|| tr.text("head-free-time-now", None), |u| say("head-free-time", &[("until", u)])),
            Reason::Paused => return held.map_or_else(|| tr.text("head-paused-now", None), |u| say("head-paused", &[("until", u)])),
            Reason::WindingDown => {
                let bed = i.today.iter().find(|item| item.kind == Kind::Night && matches!(item.time, Time::Span { start, at, .. } if start <= t && t < at)).map(|item| item.time.proper(i.zone).0);
                return match (bed, i.alarm) {
                    (Some(bed), Some(alarm)) => say("head-wind-down-alarm", &[("bed", clock(bed)), ("alarm", clock(alarm))]),
                    (Some(bed), None) => say("head-wind-down", &[("bed", clock(bed))]),
                    _ => String::new(),
                };
            }
            Reason::Sleep => return held.map_or_else(String::new, |u| say("head-sleep", &[("until", u)])),
            Reason::Nap => return held.map_or_else(String::new, |u| say("head-nap", &[("until", u)])),
            _ => {}
        }
    }
    let span = |item: &Item| match item.time {
        Time::Span { start, end, .. } => Some((start, end)),
        _ => None,
    };
    // Under way: an appointment, a pinned task, a meal, a nap first; time for you else.
    let under = i.today.iter().filter(|item| fixed(item) && item.kind != Kind::Night).find(|item| span(item).is_some_and(|(s, e)| s <= t && t < e)).or_else(|| i.today.iter().filter(|item| item.kind == Kind::Gain).find(|item| span(item).is_some_and(|(s, e)| s <= t && t < e)));
    // The next fixed thing, by when it asks you to start: leaving, getting ready, winding down.
    let next = i.today.iter().filter(|item| fixed(item)).filter_map(|item| span(item).map(|(s, _)| (s, item))).filter(|(s, _)| *s > t).min_by_key(|(s, _)| *s).map(|(_, item)| item);
    let evening = evening_of(t, i.zone).unwrap_or(i.next_midnight);
    let work_left = i.moments.iter().any(|(from, until, mode)| {
        let until = if mode.time == crate::areas::Time::Any { (*until).min(evening) } else { *until };
        until > t && *from < i.next_midnight && works(mode.time)
    });
    // An appointment or a pinned task still to come or under way: meals, naps and the
    // night keep no evening on today (they show on the timeline all the same).
    let held_left = i.today.iter().filter(|item| fixed(item) && !matches!(item.kind, Kind::Meal | Kind::Nap | Kind::Night)).any(|item| span(item).is_some_and(|(_, e)| e > t));
    if !held_left && !work_left {
        // Today holds nothing more: tomorrow's first appointment or pinned task, and the alarm.
        let first = i.tomorrow.iter().filter(|item| matches!(item.kind, Kind::Event | Kind::Block) && item.tone != Tone::Muted || (item.kind == Kind::Step && item.certainty == Certainty::Held)).filter(|item| matches!(item.time, Time::Span { .. })).min_by_key(|item| item.time.proper(i.zone).0);
        let mut parts = Vec::new();
        if let Some(item) = first
            && let Time::Span { start, at, .. } = item.time
        {
            parts.push(if start < at { say("head-tomorrow-leave", &[("time", clock(at)), ("what", named(item)), ("leave", clock(start))]) } else { say("head-tomorrow", &[("time", clock(at)), ("what", named(item))]) });
        }
        if let Some(alarm) = i.alarm {
            parts.push(say("head-alarm", &[("time", clock(alarm))]));
        }
        return parts.join(" ");
    }
    let mut parts = Vec::new();
    match under {
        Some(item) => parts.push(say("head-until", &[("what", named(item)), ("time", clock(span(item).map_or(t, |(_, e)| e)))])),
        None => {
            if let Some(mode) = moment.filter(|m| matches!(m.time, crate::areas::Time::Work | crate::areas::Time::Admin))
                && let Some(u) = until(mode)
            {
                let what = if mode.time == crate::areas::Time::Work { tr.text("head-work", None) } else { tr.text("head-admin", None) };
                parts.push(say("head-until", &[("what", what), ("time", clock(u))]));
            }
        }
    }
    if let Some(item) = next {
        let (start, at) = match item.time {
            Time::Span { start, at, .. } => (start, at),
            _ => (t, t),
        };
        // An appointment or a pinned task: its time, and when to leave; your own times: when they begin.
        let leaves = matches!(item.kind, Kind::Event | Kind::Block | Kind::Step) && start < at;
        parts.push(if leaves { say("head-next-leave", &[("time", clock(at)), ("what", named(item)), ("leave", clock(start))]) } else { say("head-next", &[("time", clock(if matches!(item.kind, Kind::Event | Kind::Block | Kind::Step) { at } else { start })), ("what", named(item))]) });
    }
    // In work and admin time, the nearest date asked this week, one only.
    // With no hours set, until 18:00 only, as `work_left` (re-check of 857c39e, §3.3).
    let working = moment.is_some_and(|m| matches!(m.time, crate::areas::Time::Work | crate::areas::Time::Admin) || (m.time == crate::areas::Time::Any && t < evening));
    if working && let Some((date, item, kind)) = nearest_date(i.dates, t, i.zone) {
        let today = jiff::Timestamp::from_second(t).map_or(date, |s| s.to_zoned(i.zone.clone()).date());
        parts.push(say("head-date", &[("kind", tr.text(kind.word(), None)), ("what", item.title.clone()), ("date", tr.day(date)), ("left", crate::dated::left(tr, today, date))]));
    }
    parts.join(" ")
}

/// The nearest date asked at `t` or after it among `dates` that asks
/// something of you: a task's, a letter's, a payment, a paper to renew; never
/// a contract's notice day (stopping one is a choice, its reminder comes two
/// weeks before), an invoice owed to you, nor health's. Its day, the item, its kind.
fn nearest_date<'a>(dates: &'a [Item], t: i64, zone: &TimeZone) -> Option<(Date, &'a Item, crate::dated::Kind)> {
    use crate::dated::Kind as Dated;
    let today = jiff::Timestamp::from_second(t).ok()?.to_zoned(zone.clone()).date();
    dates
        .iter()
        .filter(|item| item.kind == Kind::Dated && item.privacy == Privacy::Admin)
        .filter_map(|item| match &item.source {
            Source::Dated { kind, .. } if !matches!(kind, Dated::Notice | Dated::Invoice | Dated::PrescriptionEnds | Dated::RunsOut) => {
                let (date, at) = match item.time {
                    Time::Day { date, .. } => (date, None),
                    Time::At { at } => (jiff::Timestamp::from_second(at).ok()?.to_zoned(zone.clone()).date(), Some(at)),
                    Time::Span { .. } => return None,
                };
                (date >= today && at.is_none_or(|a| a >= t)).then_some((date, at.unwrap_or(0), item, *kind))
            }
            _ => None,
        })
        .min_by_key(|(date, at, item, _)| (*date, *at, item.title.clone()))
        .map(|(date, _, item, kind)| (date, item, kind))
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
            source: Source::Calendar { key: format!("/c/{owner}.ics"), uid: owner.into(), start: at(from), recurring: false, calendar: "c".into(), collection: "c".into() },
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

    fn thing(id: &str, kind: Kind, from: &str, to: &str) -> Item {
        let (at, until) = (at(from), at(to));
        Item { id: id.into(), kind, time: Time::span(at, until), certainty: Certainty::Fixed, source: Source::Frame, can: Can::nothing(), privacy: Privacy::Admin, title: id.into(), line: String::new(), said: String::new(), tone: Tone::Plain, colour: None, address: String::new(), owner: id.into(), holds_time: true }
    }

    fn night(evening: &str, bed: &str, wake: &str) -> Item {
        let mut item = thing("night", Kind::Night, bed, wake);
        item.time = Time::Span { start: at(evening), at: at(bed), until: at(wake), end: at(wake) };
        item.title = "Winding down".into();
        item.privacy = Privacy::Health;
        item
    }

    fn dressing<'a>(today: Date, now: &str, hidden: &'a dyn Fn(&str) -> bool, hours: &'a dyn Fn(Date) -> Vec<crate::dayview::Stretch>, tr: &'a Translator, zone: &'a TimeZone) -> Dressing<'a> {
        Dressing { today, now: at(now), zone, tr, hours, horizon: today.checked_add(jiff::Span::new().days(27)).unwrap(), hidden, said: vec!["Today is a light day.".into()], day_start: 7 }
    }

    /// The hours a Day or a Week shows (decision 5): from half an hour before
    /// waking to half an hour after bedtime, whole hours, one frame for every
    /// day; widened to an appointment outside them; a dose or a meal inside
    /// the night is drawn at the edge with its time, never stretching the day.
    /// With no night set: the hours given to something; else from the day's
    /// start to 22:00.
    #[test]
    fn the_hours_shown_follow_the_night() {
        let (zone, tr) = (paris(), Translator::new("en"));
        let monday = day("2026-10-05");
        let mut dose = thing("dose:a@1", Kind::Dose, "2026-10-06T05:00", "2026-10-06T05:00");
        dose.time = Time::At { at: at("2026-10-06T05:00") };
        let items = vec![
            night("2026-10-04T22:30", "2026-10-04T23:00", "2026-10-05T07:00"),
            night("2026-10-05T22:30", "2026-10-05T23:00", "2026-10-06T07:30"),
            night("2026-10-06T22:30", "2026-10-06T23:10", "2026-10-07T07:00"),
            dose,
            thing("lunch", Kind::Meal, "2026-10-05T12:30", "2026-10-05T13:00"),
        ];
        let none = |_: &str| false;
        let no_hours = |_: Date| Vec::new();
        let mut shown = days(&items, monday, 2, monday, &zone, &tr);
        let frame = dress(&mut shown, &dressing(monday, "2026-10-05T10:00", &none, &no_hours, &tr, &zone));
        assert_eq!((frame.from, frame.to), (6 * 60, 24 * 60), "06:30 less half an hour, down to 06:00; 23:10 and half an hour, up to midnight");
        assert_eq!(shown[0].edges.len(), 2, "this morning's part and tonight's");
        assert!(shown[0].timed.iter().any(|p| p.item.id == "lunch") && shown[0].timed.iter().all(|p| p.item.kind != Kind::Night));
        let dose = shown[1].doses.iter().find(|p| p.item.kind == Kind::Dose).unwrap();
        assert_eq!(dose.edge, "top", "05:00, inside the night: at the top, with its time");
        // An appointment at 05:00 widens the hours; it is never at an edge.
        let mut early = items.clone();
        early.push(thing("train", Kind::Event, "2026-10-06T05:00", "2026-10-06T05:30"));
        let mut shown = days(&early, monday, 2, monday, &zone, &tr);
        let frame = dress(&mut shown, &dressing(monday, "2026-10-05T10:00", &none, &no_hours, &tr, &zone));
        assert_eq!(frame.from, 5 * 60);
        assert!(shown[1].timed.iter().all(|p| p.edge.is_empty()));
        // No night: the hours given to something; none: from the day's start to 22:00.
        let work = |date: Date| vec![crate::dayview::Stretch { start: midnight(date, &zone) + 9 * 3600, end: midnight(date, &zone) + 17 * 3600, kind: "work".into() }];
        let mut shown = days(&[], monday, 1, monday, &zone, &tr);
        assert_eq!(dress(&mut shown, &dressing(monday, "2026-10-05T10:00", &none, &work, &tr, &zone)), Frame { from: 9 * 60, to: 17 * 60 });
        let mut shown = days(&[], monday, 1, monday, &zone, &tr);
        assert_eq!(dress(&mut shown, &dressing(monday, "2026-10-05T10:00", &none, &no_hours, &tr, &zone)), Frame { from: 7 * 60, to: 22 * 60 });
        // Every bedtime at 00:30 (review of 3a2fbd4, later 2): each evening holds its winding
        // down at its edge, the hours run to midnight, and the morning still wakes the day.
        let late_week = [
            night("2026-10-04T23:45", "2026-10-05T00:30", "2026-10-05T08:00"),
            night("2026-10-05T23:45", "2026-10-06T00:30", "2026-10-06T08:00"),
            night("2026-10-06T23:45", "2026-10-07T00:30", "2026-10-07T08:00"),
        ];
        let mut shown = days(&late_week, monday, 2, monday, &zone, &tr);
        let frame = dress(&mut shown, &dressing(monday, "2026-10-05T10:00", &none, &no_hours, &tr, &zone));
        assert_eq!((frame.from, frame.to), (7 * 60, 24 * 60), "08:00 less half an hour, down to 07:00; a bedtime past midnight: to midnight");
        for day in &shown {
            assert_eq!(day.edges.len(), 2, "{}: this morning's part and tonight's", day.date);
            let evening = day.edges.iter().find(|p| p.from_minute >= 12 * 60).unwrap();
            assert_eq!(evening.from_minute, 24 * 60, "{}: tonight's bedtime past this day's end", day.date);
            assert!(matches!(evening.item.time, Time::Span { start, .. } if start == midnight(day.date.parse().unwrap(), &zone) + (23 * 60 + 45) * 60), "{}: its winding down from 23:45", day.date);
        }
        // Gone to bed after midnight: that night still wakes the day it lies in.
        let tuesday = day("2026-10-06");
        let late = [night("2026-10-05T23:45", "2026-10-06T00:30", "2026-10-06T09:30")];
        let mut shown = days(&late, tuesday, 1, tuesday, &zone, &tr);
        assert_eq!(dress(&mut shown, &dressing(tuesday, "2026-10-06T10:00", &none, &no_hours, &tr, &zone)), Frame { from: 9 * 60, to: 22 * 60 });
    }

    /// Quiet time on a coming day (decisions g and 11): the steps the task
    /// pages hide now fold into one band per stretch of hours, named by what
    /// they are for, those steps kept for "Show anyway"; today's never. A step
    /// of today's plan that found no time is said in today's line, never
    /// drawn; the lists' line names the day's first step.
    #[test]
    fn quiet_time_folds_a_coming_day_s_hidden_steps_into_bands() {
        let (zone, tr) = (paris(), Translator::new("en"));
        let monday = day("2026-10-05");
        let step = |uid: &str, from: &str, to: &str| {
            let mut item = thing(uid, Kind::Step, from, to);
            item.certainty = Certainty::Planned;
            item
        };
        let mut unlaid = step("form", "2026-10-05T00:00", "2026-10-05T00:00");
        unlaid.time = Time::Day { date: monday, until: monday };
        let items = vec![step("report", "2026-10-05T09:00", "2026-10-05T10:00"), step("report", "2026-10-06T09:00", "2026-10-06T10:00"), step("slides", "2026-10-06T10:15", "2026-10-06T11:00"), step("tidy", "2026-10-06T18:00", "2026-10-06T18:30"), unlaid];
        let hidden = |uid: &str| uid == "report" || uid == "slides";
        let work = |date: Date| vec![crate::dayview::Stretch { start: midnight(date, &zone) + 9 * 3600, end: midnight(date, &zone) + 12 * 3600, kind: "work".into() }];
        let mut shown = days(&items, monday, 2, monday, &zone, &tr);
        dress(&mut shown, &dressing(monday, "2026-10-05T08:00", &hidden, &work, &tr, &zone));
        assert!(shown[0].bands.is_empty() && shown[0].timed.iter().any(|p| p.item.owner == "report"), "today: as the Tasks job laid it");
        assert_eq!(shown[1].bands.len(), 1);
        assert_eq!(shown[1].bands[0].item.title, "Work, 09:00 – 11:00");
        assert_eq!(shown[1].bands[0].item.privacy, Privacy::Frame, "never to an agent");
        assert_eq!(shown[1].hidden.len(), 2);
        assert!(shown[1].timed.iter().all(|p| p.item.owner != "report" && p.item.owner != "slides"));
        assert!(shown[0].head.iter().chain(&shown[0].timed).all(|p| p.item.owner != "form"), "at no time: never drawn");
        assert!(shown[0].more.contains("form"), "{}", shown[0].more);
        assert_eq!(shown[0].said, vec!["Today is a light day.".to_string()]);
        assert_eq!(shown[1].plan, "The plan: tidy first");
    }

    fn moment(time: crate::areas::Time, reason: crate::quiet::Reason, until: &str) -> crate::quiet::Mode {
        let until = Timestamp::from_second(at(until)).unwrap().to_zoned(paris());
        crate::quiet::Mode { quiet: time != crate::areas::Time::Work, time, week: crate::areas::Week::default(), reason, until: Some(until), back: None, label: String::new() }
    }

    /// The head line (§4.2), given ahead by the moment: in work time what is
    /// under way and the next fixed thing with its leaving time; at a meal
    /// the same; once today holds nothing more, tomorrow's first appointment
    /// and the alarm; in Free time the moment alone. Never a dose, never a
    /// proposal of the plan; each frame holds until what it says changes.
    #[test]
    fn the_head_line_says_what_is_under_way_and_what_comes() {
        use crate::areas::Time as Hours;
        use crate::quiet::Reason;
        let (zone, tr) = (paris(), Translator::new("en"));
        let mut doctor = thing("Doctor", Kind::Event, "2026-10-05T14:00", "2026-10-05T14:30");
        doctor.time = Time::Span { start: at("2026-10-05T13:05"), at: at("2026-10-05T14:00"), until: at("2026-10-05T14:30"), end: at("2026-10-05T14:30") };
        let mut lunch = thing("Lunch", Kind::Meal, "2026-10-05T12:30", "2026-10-05T13:00");
        lunch.time = Time::Span { start: at("2026-10-05T12:10"), at: at("2026-10-05T12:30"), until: at("2026-10-05T13:00"), end: at("2026-10-05T13:00") };
        let mut proposal = thing("Sort the receipts", Kind::Step, "2026-10-05T11:15", "2026-10-05T11:45");
        proposal.certainty = Certainty::Planned;
        let mut dose = thing("Dose", Kind::Dose, "2026-10-05T11:30", "2026-10-05T11:30");
        dose.time = Time::At { at: at("2026-10-05T11:30") };
        let today = vec![thing("Time for you", Kind::Gain, "2026-10-05T10:30", "2026-10-05T11:00"), proposal, dose, lunch, doctor];
        let mut dentist = thing("Dentist", Kind::Event, "2026-10-06T09:30", "2026-10-06T10:00");
        dentist.time = Time::Span { start: at("2026-10-06T09:05"), at: at("2026-10-06T09:30"), until: at("2026-10-06T10:00"), end: at("2026-10-06T10:00") };
        let moments = vec![
            (at("2026-10-05T09:00"), at("2026-10-05T17:00"), moment(Hours::Work, Reason::Working, "2026-10-05T17:00")),
            (at("2026-10-05T17:00"), at("2026-10-06T00:00"), moment(Hours::Leisure, Reason::Evening, "2026-10-06T09:00")),
        ];
        let line = |now: &str| head_line(&HeadInputs { now: at(now), next_midnight: at("2026-10-06T00:00"), moments: &moments, today: &today, tomorrow: std::slice::from_ref(&dentist), alarm: Some(at("2026-10-06T07:00")), dates: &[], zone: &zone, tr: &tr });
        let frames = line("2026-10-05T10:40");
        assert_eq!(frames[0].text, "Time for you until 11:00. Next: 12:10 Lunch.");
        assert_eq!((frames[0].from, frames[0].until), (at("2026-10-05T10:40"), at("2026-10-05T11:00")), "it holds until time for you ends");
        let at_lunch = frames.iter().find(|f| f.from <= at("2026-10-05T12:40") && at("2026-10-05T12:40") < f.until).unwrap();
        assert_eq!(at_lunch.text, "Lunch until 13:00. Next: 14:00 Doctor, leave at 13:05.");
        let evening = frames.iter().find(|f| f.from <= at("2026-10-05T18:00") && at("2026-10-05T18:00") < f.until).unwrap();
        assert_eq!(evening.text, "Tomorrow: 09:30 Dentist, leave at 09:05. Alarm at 07:00.");
        assert!(frames.iter().all(|f| !f.text.contains("Dose") && !f.text.contains("Sort the receipts")), "{frames:?}");
        // Free time: the moment alone.
        let free = vec![(at("2026-10-05T09:00"), at("2026-10-05T16:00"), moment(Hours::Leisure, Reason::FreeTime, "2026-10-05T16:00"))];
        let frames = head_line(&HeadInputs { now: at("2026-10-05T10:40"), next_midnight: at("2026-10-06T00:00"), moments: &free, today: &today, tomorrow: &[], alarm: None, dates: &[], zone: &zone, tr: &tr });
        assert_eq!(frames[0].text, "Free time until 16:00.");
    }

    /// A date asked comes first in its day's head, before a trip begun the
    /// day before and a whole day (review of part 4, later 2): never a "1 more".
    #[test]
    fn a_date_asked_heads_its_day() {
        let (zone, tr) = (paris(), Translator::new("en"));
        let mut trip = thing("Trip", Kind::Event, "2026-10-04T00:00", "2026-10-07T00:00");
        trip.time = Time::Day { date: day("2026-10-04"), until: day("2026-10-06") };
        let mut birthday = thing("A birthday", Kind::Event, "2026-10-05T00:00", "2026-10-06T00:00");
        birthday.time = Time::Day { date: day("2026-10-05"), until: day("2026-10-05") };
        let caf = crate::dated::item(&crate::dated::Dated { kind: crate::dated::Kind::Task, key: "caf".into(), address: "sioul:task/caf".into(), title: "CAF form".into(), date: day("2026-10-05"), at: None }, day("2026-10-05"), &tr);
        let days = days(&[trip, birthday, caf], day("2026-10-05"), 1, day("2026-10-05"), &zone, &tr);
        assert_eq!(days[0].head.iter().map(|p| p.item.title.as_str()).collect::<Vec<_>>(), ["CAF form", "Trip", "A birthday"]);
    }

    /// The evening with a night set (part 2's open point, decided in part 4):
    /// once no work or admin time and no appointment is left, the line speaks
    /// of tomorrow, dinner and the night still to come; a date asked is said
    /// in work and admin time only, the nearest of the week, never a
    /// contract's notice day, never in the evening.
    #[test]
    fn the_head_line_s_evening_and_its_one_date() {
        use crate::areas::Time as Hours;
        use crate::quiet::Reason;
        let (zone, tr) = (paris(), Translator::new("en"));
        let mut dinner = thing("Dinner", Kind::Meal, "2026-10-05T19:30", "2026-10-05T20:00");
        dinner.time = Time::Span { start: at("2026-10-05T19:10"), at: at("2026-10-05T19:30"), until: at("2026-10-05T20:00"), end: at("2026-10-05T20:00") };
        let mut night = thing("Night", Kind::Night, "2026-10-05T23:00", "2026-10-06T07:00");
        night.time = Time::Span { start: at("2026-10-05T22:30"), at: at("2026-10-05T23:00"), until: at("2026-10-06T07:00"), end: at("2026-10-06T07:00") };
        let today = vec![dinner, night];
        let mut dentist = thing("Dentist", Kind::Event, "2026-10-06T09:30", "2026-10-06T10:00");
        dentist.time = Time::span(at("2026-10-06T09:30"), at("2026-10-06T10:00"));
        let moments = vec![
            (at("2026-10-05T09:00"), at("2026-10-05T17:00"), moment(Hours::Work, Reason::Working, "2026-10-05T17:00")),
            (at("2026-10-05T17:00"), at("2026-10-06T00:00"), moment(Hours::Leisure, Reason::Evening, "2026-10-06T09:00")),
        ];
        let caf = crate::dated::item(&crate::dated::Dated { kind: crate::dated::Kind::Task, key: "caf".into(), address: "sioul:task/caf".into(), title: "CAF form".into(), date: day("2026-10-09"), at: None }, day("2026-10-05"), &tr);
        let notice = crate::dated::item(&crate::dated::Dated { kind: crate::dated::Kind::Notice, key: "box".into(), address: "sioul:contract/box".into(), title: "Internet box".into(), date: day("2026-10-06"), at: None }, day("2026-10-05"), &tr);
        let dates = vec![notice, caf];
        let frames = head_line(&HeadInputs { now: at("2026-10-05T10:00"), next_midnight: at("2026-10-06T00:00"), moments: &moments, today: &today, tomorrow: std::slice::from_ref(&dentist), alarm: Some(at("2026-10-06T07:00")), dates: &dates, zone: &zone, tr: &tr });
        let at_ = |t: &str| frames.iter().find(|f| f.from <= at(t) && at(t) < f.until).map(|f| f.text.clone()).unwrap_or_default();
        assert_eq!(at_("2026-10-05T10:00"), "Work until 17:00. Next: 19:10 Dinner. Date asked: CAF form, Friday 9 October: four days left.", "the notice day never");
        assert_eq!(at_("2026-10-05T18:00"), "Tomorrow: 09:30 Dentist. Alarm at 07:00.", "dinner and the night to come: tomorrow, no date");
        assert_eq!(at_("2026-10-05T19:40"), "Tomorrow: 09:30 Dentist. Alarm at 07:00.", "at dinner too");
        // No hours set (everything comes, all day): the day's line until 18:00, its date
        // named; tomorrow's after (review of part 4, later 8).
        let any = vec![(at("2026-10-05T00:00"), at("2026-10-06T00:00"), crate::quiet::Mode { until: None, ..moment(Hours::Any, Reason::NoHours, "2026-10-06T00:00") })];
        let frames = head_line(&HeadInputs { now: at("2026-10-05T10:00"), next_midnight: at("2026-10-06T00:00"), moments: &any, today: &today, tomorrow: std::slice::from_ref(&dentist), alarm: Some(at("2026-10-06T07:00")), dates: &dates, zone: &zone, tr: &tr });
        let at_ = |t: &str| frames.iter().find(|f| f.from <= at(t) && at(t) < f.until).map(|f| f.text.clone()).unwrap_or_default();
        assert_eq!(at_("2026-10-05T10:00"), "Next: 19:10 Dinner. Date asked: CAF form, Friday 9 October: four days left.");
        assert_eq!(at_("2026-10-05T19:00"), "Tomorrow: 09:30 Dentist. Alarm at 07:00.");
        // A concert at 20:00 still to come: at 19:00 the line names it, and no date (re-check of
        // 857c39e, §3.3), as with hours set.
        let mut concert = thing("Concert", Kind::Event, "2026-10-05T20:00", "2026-10-05T22:00");
        concert.time = Time::span(at("2026-10-05T20:00"), at("2026-10-05T22:00"));
        let evening = vec![concert];
        let frames = head_line(&HeadInputs { now: at("2026-10-05T10:00"), next_midnight: at("2026-10-06T00:00"), moments: &any, today: &evening, tomorrow: std::slice::from_ref(&dentist), alarm: Some(at("2026-10-06T07:00")), dates: &dates, zone: &zone, tr: &tr });
        let at_ = |t: &str| frames.iter().find(|f| f.from <= at(t) && at(t) < f.until).map(|f| f.text.clone()).unwrap_or_default();
        assert_eq!(at_("2026-10-05T10:00"), "Next: 20:00 Concert. Date asked: CAF form, Friday 9 October: four days left.");
        assert_eq!(at_("2026-10-05T19:00"), "Next: 20:00 Concert.");
    }
}
