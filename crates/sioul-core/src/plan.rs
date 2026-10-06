// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The plan: what can start, the one next step, and when each task could
//! happen. It is computed from the tasks each time, never stored, so it
//! always starts from today (docs/tasks.md).
//!
//! - **Order**: a task waits for others (`tasks`). Kahn's algorithm orders
//!   them (Kahn 1962, "Topological sorting of large networks"). Tasks that
//!   wait for each other in a loop are found (Tarjan 1972) and said, calmly:
//!   one of them has to go first.
//! - **The next step**: among the tasks free to start, the one already
//!   started; else the one whose latest start comes first (its date, minus the
//!   chain of work behind it); then the more important, as you ranked it
//!   (PRIORITY); then the one that frees the most others; then the smaller
//!   (docs/design.md, "Tasks").
//! - **When**: each task goes into the first days with room, in that order,
//!   after what it waits for, never before today. A date that passed moves a
//!   task forward; it never makes it "late". Room is your hours, each kind for
//!   its own tasks: work hours for work, admin hours for your admin
//!   (`areas::in_view` lends what has no hours of its own); leisure has no
//!   hours, so what is only for it takes no room and waits for none; less
//!   the times kept for meals, naps and sleep (`needs`: set first, the work
//!   planned around them), the events in them and a pause after each step. Sioul can run your work
//!   too, so the plan may fill your hours: no second budget to keep.
//! - **Waiting on others**: a task done starts the clock of what waits for it
//!   with a gap ("the answer comes within two weeks"): the next one waits
//!   until then, and says so.
//! - **Optional**: a task tagged `joy` (only if you want to, never a duty) or
//!   `someday` (parked: when you say so), or a step of one, is never proposed
//!   as the next step and takes no room in the plan. Nor does what gives
//!   back (energy "rest": a walk, music): it is offered after a heavy step.
//! - **Heavy**: a day takes so many heavy tasks (two when clear, one in haze,
//!   none in fog, today's weather saying for today); the next one goes to a
//!   day that can take it, and in fog a heavy task is not the next step unless
//!   nothing else is ready (energy accounting: fewer demands, Raymaker et al. 2020).
//!   Heavy is the highest cost when any is rated, else the word (`capacity::level_of`);
//!   the days before and after a heavy event take one fewer.
//! - **What a day holds** (`capacity`, docs/capacity.md): with budgets, each day
//!   is filled to 85 % of each cost's budget and of the total, after its events;
//!   lighter around a heavy event, today by its weather; kept even if you
//!   asked. Each task is laid at its corrected length (your estimate × how
//!   long tasks like it take you), each day keeps free time for steps running
//!   long (the 85th percentile of the total of the steps laid there, fitted
//!   as the day fills, a third of its room at most) and half an hour for a
//!   gain slot; the next step keeps part of today all the same.
//!   A task with margins is never cut: laid whole on one day, or given a day
//!   of its own. A step given a time today by hand stays today.

use crate::agenda::Occurrence;
use crate::areas::{Area, TaskAreas, Time, Week, in_view};
use crate::tasks::{Status, Task};
use crate::window::AdminWindow;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Zoned};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

/// Where a task stands, as the board shows it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Column {
    /// Free to start.
    #[default]
    Ready,
    /// Started.
    Doing,
    /// Waits for another task, or for a day.
    Waiting,
    /// Done, or dropped.
    Done,
}

/// The least a step takes in a day, in minutes: a short one gets air around it.
pub const SHORTEST: u32 = 15;

/// A pause after each step and around each event, in minutes: switching costs (monotropism).
pub const PAUSE: u32 = 5;

/// A step up to an hour is done in one go, never cut over two days or around a break.
pub const WHOLE: u32 = 60;

/// A day's hours for tasks: minutes, by the kinds of hours open then
/// (several where windows overlap: admin hours within working hours).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Room(pub Vec<(Area, u32)>);

impl Room {
    /// Minutes open to something of `kinds`.
    pub fn for_kinds(&self, kinds: Area) -> u32 {
        self.0.iter().filter(|(open, _)| open.meets(kinds)).map(|(_, minutes)| minutes).sum()
    }

    /// Every minute of it.
    pub fn total(&self) -> u32 {
        self.0.iter().map(|(_, minutes)| minutes).sum()
    }

    /// The minutes of `stretches`, gathered by the kinds open in each.
    pub fn of(stretches: &[(i64, i64, Area)]) -> Room {
        let mut room: Vec<(Area, u32)> = Vec::new();
        for &(from, to, open) in stretches {
            let minutes = u32::try_from((to - from) / 60).unwrap_or(0);
            match room.iter_mut().find(|(kinds, _)| *kinds == open) {
                Some(found) => found.1 += minutes,
                None => room.push((open, minutes)),
            }
        }
        Room(room)
    }
}

/// The stretches of `date`'s hours, as (start, end, kinds open), Unix seconds,
/// in order: where windows of several kinds overlap, one stretch open to each;
/// with `anything`, every window is open to every kind.
pub fn stretches(windows: &[AdminWindow], date: Date, zone: &TimeZone, anything: bool) -> Vec<(i64, i64, Area)> {
    let spans: Vec<(i64, i64, Area)> = crate::window::kinds_on(windows, date, zone)
        .into_iter()
        .map(|(opening, closing, kind)| (opening.timestamp().as_second(), closing.timestamp().as_second(), if anything { Area::ALL } else { Area::parse(kind).unwrap_or(Area::WORK) }))
        .collect();
    let mut cuts: Vec<i64> = spans.iter().flat_map(|s| [s.0, s.1]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<(i64, i64, Area)> = Vec::new();
    for pair in cuts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let open = spans.iter().filter(|s| s.0 <= from && to <= s.1).fold(Area::default(), |kinds, s| kinds.with(s.2));
        if open.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.1 == from && last.2 == open => last.1 = to,
            _ => out.push((from, to, open)),
        }
    }
    out
}

/// `stretches` outside every `busy` span.
pub fn less(stretches: &[(i64, i64, Area)], busy: &[(i64, i64)]) -> Vec<(i64, i64, Area)> {
    let mut out = Vec::new();
    for &(from, to, open) in stretches {
        let mut pieces = vec![(from, to)];
        for &(taken_from, taken_to) in busy {
            pieces = pieces
                .into_iter()
                .flat_map(|(a, b)| {
                    if taken_to <= a || taken_from >= b {
                        return vec![(a, b)];
                    }
                    [(a, taken_from), (taken_to, b)].into_iter().filter(|(x, y)| y > x).collect()
                })
                .collect();
        }
        out.extend(pieces.into_iter().map(|(a, b)| (a, b, open)));
    }
    out
}

/// The time events hold: their margins (getting there and back, getting
/// ready: `demands`), then a pause before and after; timed ones, not
/// cancelled; shorter than a quarter of an hour, a quarter of an hour.
pub fn event_spans(events: &[Occurrence], pause: u32) -> Vec<(i64, i64)> {
    let pause = i64::from(pause) * 60;
    events
        .iter()
        .filter(|e| !e.cancelled && !e.all_day)
        .map(|e| (e.start - i64::from(e.margins.before) * 60 - pause, e.end.max(e.start + 15 * 60) + i64::from(e.margins.after) * 60 + pause))
        .collect()
}

/// How much room the days have for tasks: your hours, each for what it is for.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The week's hours as room, Monday first.
    pub week: [Room; 7],
    /// Days whose room is not their weekday's: today from now on, the days
    /// with events, the events taken out (`with_events`).
    pub days: BTreeMap<Date, Room>,
    /// The week's hours as written, for the day's layout (`dayview`).
    pub windows: Vec<AdminWindow>,
    /// No hours set at all: offices' usual hours, open to every task.
    pub anything: bool,
    /// Which kinds of hours the week sets: what has none of its own goes where `areas::in_view` says.
    pub hours: Week,
    /// What each task is for: its own area, else its categories and projects.
    pub areas: TaskAreas,
    /// Minutes of pause after each step, taken from the room too.
    pub pause: u32,
    /// Minutes counted for a task without an estimate.
    pub default_estimate: u32,
    /// Today's share of its usual room, in percent: the day's weather (`today`).
    pub today_percent: u32,
    /// Days without room, whatever their weekday: time off, a day stopped early.
    pub closed: BTreeSet<Date>,
    /// The weekdays offices open, Monday first: tasks that need one go only there.
    pub office_days: [bool; 7],
    /// Offices' usual hours (`Config::office_hours`): the day's layout puts a
    /// task that needs an office, without hours of its own, only while they are open.
    pub office_hours: Vec<AdminWindow>,
    /// Heavy tasks today takes (its weather), and any other day.
    pub heavy_today: u32,
    pub heavy_per_day: u32,
    /// Meals, naps and the night: kept free of tasks (`with_needs`).
    pub needs: crate::needs::Needs,
    /// Today's meals pushed past its events, by key, in minutes (`Needs::past_events_on`).
    pub shifts: BTreeMap<String, i64>,
    /// Each day's own meals, naps and nights: moved, changed, taken out, added (`with_days`).
    pub needs_days: crate::needs::Days,
    /// What your record says (`capacity`): ratings, corrected lengths, budgets,
    /// free time, the gain slot. Its default changes nothing.
    pub capacity: crate::capacity::Planning,
}

impl Default for Settings {
    /// Offices' usual hours, Monday to Friday 9:00 to 17:00, open to every task, until yours are set.
    fn default() -> Settings {
        Settings::of_hours(&[], TaskAreas::usual())
    }
}

impl Settings {
    /// The room your week's hours give (`Config::week_hours`), each kind for
    /// its own tasks; none set: offices' usual hours, for everything.
    pub fn of_hours(windows: &[AdminWindow], areas: TaskAreas) -> Settings {
        // Leisure is every other time: an older Sioul's free-time hours give no room.
        let windows: Vec<AdminWindow> = windows.iter().filter(|w| w.kind() != "leisure").cloned().collect();
        let anything = windows.is_empty();
        let windows = if anything { crate::window::default_office_hours() } else { windows };
        let hours = Week { work_hours: windows.iter().any(|w| w.kind() == "work"), admin_hours: windows.iter().any(|w| w.kind() == "admin"), ..Week::default() };
        // Any week will do: Monday 1 January 2024 to Sunday the 7th.
        let week = std::array::from_fn(|d| Room::of(&stretches(&windows, Date::constant(2024, 1, 1 + d as i8), &TimeZone::UTC, anything)));
        Settings {
            week,
            days: BTreeMap::new(),
            windows,
            anything,
            hours,
            areas,
            pause: PAUSE,
            default_estimate: 30,
            today_percent: 100,
            closed: BTreeSet::new(),
            office_days: [true, true, true, true, true, false, false],
            office_hours: crate::window::default_office_hours(),
            heavy_today: 2,
            heavy_per_day: 2,
            needs: crate::needs::Needs::default(),
            shifts: BTreeMap::new(),
            needs_days: crate::needs::Days::default(),
            capacity: crate::capacity::Planning::default(),
        }
    }

    /// The same minutes each weekday, Monday first, open to every task, no pause: for tests and simple plans.
    pub fn flat(minutes: [u32; 7]) -> Settings {
        let week = minutes.map(|m| if m == 0 { Room::default() } else { Room(vec![(Area::ALL, m)]) });
        Settings { week, pause: 0, ..Settings::default() }
    }

    /// Meals, naps and the night kept free of tasks, every day (`shifts`:
    /// today's moved by its events, by key, in minutes). Before `with_events`,
    /// which keeps them free on the days it lays out too.
    pub fn with_needs(mut self, needs: &crate::needs::Needs, shifts: BTreeMap<String, i64>) -> Settings {
        let none = |_: &str| 0;
        // Any week will do, as for the hours: Monday 1 January 2024, on the clock.
        self.week = std::array::from_fn(|d| {
            let date = Date::constant(2024, 1, 1 + d as i8);
            Room::of(&less(&stretches(&self.windows, date, &TimeZone::UTC, self.anything), &needs.busy_on(date, &TimeZone::UTC, &none)))
        });
        self.needs = needs.clone();
        self.shifts = shifts;
        self
    }

    /// Each day's own meals, naps and nights (`needs::Days`): before
    /// `with_events`, which lays out each day changed on its own.
    pub fn with_days(mut self, days: crate::needs::Days) -> Settings {
        self.needs_days = days;
        self
    }

    /// The times kept free on `date`, as that day has them; today's meals
    /// moved past its events as said (`shifts`).
    pub fn kept_on(&self, date: Date, zone: &TimeZone, today: Date) -> Vec<(i64, i64)> {
        self.kept_held(date, zone, today, &[])
    }

    /// The same, another day's meals moved past the events it holds (`held`:
    /// their times with their margins), as today's are.
    fn kept_held(&self, date: Date, zone: &TimeZone, today: Date, held: &[(i64, i64)]) -> Vec<(i64, i64)> {
        let pushed = if date == today { self.shifts.clone() } else { self.needs.past_events_on(date, zone, &self.needs_days, held) };
        self.needs.kept_with(date, zone, &self.needs_days, &|key: &str| pushed.get(key).copied().unwrap_or(0)).into_iter().map(|k| (k.start, k.end)).collect()
    }

    /// Today from `now` on, and the coming days with events: the events taken
    /// out of their hours, a pause before and after each (appointments, meetings).
    pub fn with_events(mut self, now: &Zoned, events: &[Occurrence]) -> Settings {
        let zone = now.time_zone().clone();
        let today = now.date();
        let busy = event_spans(events, self.pause);
        let day_of = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()).date());
        // Every day an event covers (a trip, three days of a conference), and
        // the day before, whose hours may run past midnight; as far as the plan goes.
        let horizon = add_days(today, 3 * 366);
        let mut dates: BTreeSet<Date> = BTreeSet::from([today]);
        for &(from, to) in &busy {
            let (Some(first), Some(last)) = (day_of(from), day_of(to)) else { continue };
            let mut date = first.yesterday().unwrap_or(first).max(today);
            while date <= last.min(horizon) {
                dates.insert(date);
                let Ok(next) = date.tomorrow() else { break };
                date = next;
            }
        }
        // Each day with changes of its own, and the next (its night ends that morning).
        for &date in self.needs_days.0.keys().filter(|d| **d >= today.yesterday().unwrap_or(today) && **d <= horizon) {
            dates.extend([date, date.tomorrow().unwrap_or(date)].into_iter().filter(|d| *d >= today));
        }
        // Meals move past the events with their margins, as today's (`Needs::past_events_on`).
        let held = event_spans(events, 0);
        for date in dates {
            let mut taken = busy.clone();
            taken.extend(self.kept_held(date, &zone, today, &held));
            if date == today {
                // From the next five minutes, as the day's layout starts.
                taken.push((i64::MIN, (now.timestamp().as_second() + 299) / 300 * 300));
            }
            let room = Room::of(&less(&stretches(&self.windows, date, &zone, self.anything), &taken));
            self.days.insert(date, room);
        }
        self
    }

    /// A day's room: its own when it has events or is today, else its weekday's.
    pub fn room_on(&self, date: Date) -> &Room {
        self.days.get(&date).unwrap_or(&self.week[date.weekday().to_monday_zero_offset() as usize])
    }

    /// The kinds of hours a task can take: those of its area, and those
    /// `areas::in_view` lends what has no hours of its own. A call to an
    /// office, as `quiet::QuietTasks::keeps` shows it: work and admin hours,
    /// never leisure, as offices keep business hours (docs/areas.md).
    pub fn usable(&self, task: &Task) -> Area {
        if self.anything {
            return Area::ALL;
        }
        let area = self.areas.of(task);
        if task.office_hours && area.admin && !area.work {
            return Area::MIXED;
        }
        Area { work: in_view(area, Time::Work, self.hours), admin: in_view(area, Time::Admin, self.hours), leisure: in_view(area, Time::Leisure, self.hours) }
    }

    /// The weekdays without any hours, Monday first.
    pub fn rest_days(&self) -> [bool; 7] {
        std::array::from_fn(|d| self.week[d].total() == 0)
    }
}

/// One task in the plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Planned {
    pub uid: String,
    pub column: Column,
    /// The open tasks it waits for: its own, and those its bigger task waits for.
    pub waits_for: Vec<String>,
    /// It cannot start before this day: its start date, or the end of a gap after a task done.
    pub not_before: Option<Date>,
    /// The open tasks that wait for it, directly or further down the chain.
    pub unblocks: usize,
    /// The last day it can start for every date after it to hold.
    pub latest_start: Option<Date>,
    /// The days the plan gives it.
    pub start: Option<Date>,
    pub finish: Option<Date>,
    /// At this pace it ends after its date: the plan says so early.
    pub tight: bool,
    /// It waits for a task that waits for it.
    pub in_loop: bool,
    /// Its steps not done yet.
    pub open_steps: usize,
    /// How many bigger tasks it is a step of.
    pub depth: usize,
    /// Minutes left: its estimate, less the time already spent, its margins added.
    pub left: u32,
    /// Minutes the plan lays for it: the same from its corrected length (`capacity::Ratios`).
    pub laid: u32,
    /// How heavy the plan takes it: "light", "usual", "heavy", "rest" (`capacity::Rates::level`).
    pub level: &'static str,
    /// Minutes the plan gives it on its first day: all of it, or the part that fits.
    pub on_start: u32,
    /// Tagged `joy` or `someday`, or a step of such a task: never proposed, never scheduled.
    pub optional: bool,
}

/// The tags that make a task optional (`Planned::optional`).
pub const OPTIONAL_TAGS: &[&str] = &["joy", "someday"];

/// Whether a task's own tags make it optional, or it gives back rather than
/// takes (its word, or its costs rated light with a gain of 5 or more). A
/// date asked makes it a duty all the same: planned, as a light step.
pub fn is_optional(task: &Task) -> bool {
    (crate::capacity::level_of(task) == crate::demands::Level::Rest && task.due.is_empty()) || task.categories.iter().any(|c| OPTIONAL_TAGS.iter().any(|t| c.eq_ignore_ascii_case(t)))
}

/// The whole plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Plan {
    /// Every task, by UID.
    pub items: BTreeMap<String, Planned>,
    /// The open tasks, in the order the plan takes them.
    pub order: Vec<String>,
    /// The tasks free to start, the next one first.
    pub ready: Vec<String>,
    /// The next step: the first of `ready` not set aside for today, from
    /// another stream than those set aside when there is one.
    pub next: Option<String>,
    /// The stream of each task, by UID: tasks tied by what they wait for or by
    /// being steps of the same bigger task share one. "Not now" puts a whole
    /// stream aside for the day: what comes next is not tied to it.
    pub streams: BTreeMap<String, usize>,
    /// Tasks that wait for each other, loop by loop.
    pub loops: Vec<Vec<String>>,
    /// What each day holds: each task's UID and its minutes there, in the plan's order.
    pub days: BTreeMap<Date, Vec<(String, u32)>>,
    /// Minutes each day keeps free for steps running long (`capacity::slack`).
    pub slack: BTreeMap<Date, u32>,
}

/// The time budget until a date asked (Shovel's "cushion"): what must be done
/// by then, against the room the days have until then. Shown only near a date:
/// further away it is noise (docs/tasks.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Cushion {
    pub date: Date,
    /// Minutes left of every open task asked by then, of what each waits for, of its steps.
    pub need: u32,
    /// Minutes of room from today to that day, today's weather and days off counted.
    pub room: u32,
}

/// The time budget until `date`: the tasks asked by then (optional ones aside),
/// with what they wait for and their steps, against each day's room until then.
pub fn cushion(tasks: &[Task], plan: &Plan, settings: &Settings, today: Date, date: Date) -> Cushion {
    let open = |uid: &str| plan.items.get(uid).filter(|p| p.column != Column::Done && !p.optional);
    let mut wanted: Vec<String> = tasks.iter().filter(|t| t.due_date().is_some_and(|d| d <= date) && open(&t.uid).is_some()).map(|t| t.uid.clone()).collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut need = 0u32;
    // The hours these tasks can take.
    let mut kinds = Area::default();
    while let Some(uid) = wanted.pop() {
        if !seen.insert(uid.clone()) {
            continue;
        }
        let Some(planned) = open(&uid) else { continue };
        need = need.saturating_add(planned.laid);
        if let Some(task) = tasks.iter().find(|t| t.uid == uid) {
            kinds = kinds.with(settings.usable(task));
        }
        wanted.extend(planned.waits_for.iter().cloned());
        wanted.extend(tasks.iter().filter(|t| t.parent() == Some(uid.as_str())).map(|t| t.uid.clone()));
    }
    let mut room = 0u32;
    let mut day = today;
    while day <= date {
        let usual = if settings.closed.contains(&day) { 0 } else { settings.room_on(day).for_kinds(kinds) };
        room = room.saturating_add(if day == today { usual * settings.today_percent / 100 } else { usual });
        let Ok(next) = day.tomorrow() else { break };
        day = next;
    }
    Cushion { date, need, room }
}

/// `days` later (earlier when negative); a span too long for any date (a gap
/// or an estimate from someone else's file) moves nothing.
fn add_days(date: Date, days: i64) -> Date {
    Span::new().try_days(days).ok().and_then(|span| date.checked_add(span).ok()).unwrap_or(date)
}

/// Whole days of a gap: "P14D" waits fourteen days; less than a day, none.
fn gap_days(minutes: i64) -> i64 {
    minutes / 1440
}

/// A day as a number, to rank by.
fn day_number(date: Date) -> i64 {
    date.since(Date::constant(1970, 1, 1)).map_or(0, |s| i64::from(s.get_days()))
}

/// The ties between tasks, by index: who waits for whom, and who is a step of whom.
struct Graph {
    /// predecessor → (successor, gap in minutes): dependencies only.
    after: Vec<Vec<(usize, i64)>>,
    /// The bigger task of each.
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
}

impl Graph {
    fn new(tasks: &[&Task], index: &BTreeMap<&str, usize>) -> Graph {
        let n = tasks.len();
        let mut graph = Graph { after: vec![Vec::new(); n], parent: vec![None; n], children: vec![Vec::new(); n] };
        let edge = |from: usize, to: usize, gap: i64, graph: &mut Graph| {
            if from != to && !graph.after[from].iter().any(|(t, _)| *t == to) {
                graph.after[from].push((to, gap));
            }
        };
        for (i, task) in tasks.iter().enumerate() {
            for relation in &task.relations {
                let Some(&other) = index.get(relation.uid.as_str()) else { continue };
                match relation.kind.as_str() {
                    // In the one that waits.
                    "DEPENDS-ON" => edge(other, i, relation.gap, &mut graph),
                    // In the one that comes first (RFC 9253 §4).
                    "FINISHTOSTART" | "NEXT" => edge(i, other, relation.gap, &mut graph),
                    "PARENT" if graph.parent[i].is_none() && other != i => graph.parent[i] = Some(other),
                    "CHILD" if graph.parent[other].is_none() && other != i => graph.parent[other] = Some(i),
                    _ => {}
                }
            }
        }
        // A step of itself, further up: the tie is dropped.
        for i in 0..n {
            let mut seen = BTreeSet::from([i]);
            let mut at = graph.parent[i];
            while let Some(p) = at {
                if !seen.insert(p) {
                    graph.parent[i] = None;
                    break;
                }
                at = graph.parent[p];
            }
        }
        for i in 0..n {
            if let Some(p) = graph.parent[i] {
                graph.children[p].push(i);
            }
        }
        graph
    }

    fn ancestors(&self, i: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = self.parent[i];
        while let Some(p) = at {
            out.push(p);
            at = self.parent[p];
        }
        out
    }

    /// What a task waits for: its own predecessors and those of its bigger tasks.
    fn predecessors(&self, i: usize) -> Vec<(usize, i64)> {
        let mut out = Vec::new();
        for holder in std::iter::once(i).chain(self.ancestors(i)) {
            for (from, list) in self.after.iter().enumerate() {
                for &(to, gap) in list {
                    if to == holder && !out.iter().any(|(f, _)| *f == from) {
                        out.push((from, gap));
                    }
                }
            }
        }
        out
    }
}

/// Strongly connected components with more than one task (Tarjan 1972): the loops.
fn loops(n: usize, edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct State<'a> {
        edges: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        found: Vec<Vec<usize>>,
    }
    fn visit(v: usize, s: &mut State) {
        s.index[v] = Some(s.next);
        s.low[v] = s.next;
        s.next += 1;
        s.stack.push(v);
        s.on_stack[v] = true;
        for &w in &s.edges[v] {
            match s.index[w] {
                None => {
                    visit(w, s);
                    s.low[v] = s.low[v].min(s.low[w]);
                }
                Some(index) if s.on_stack[w] => s.low[v] = s.low[v].min(index),
                Some(_) => {}
            }
        }
        if Some(s.low[v]) == s.index[v] {
            let mut component = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on_stack[w] = false;
                component.push(w);
                if w == v {
                    break;
                }
            }
            if component.len() > 1 {
                component.sort_unstable();
                s.found.push(component);
            }
        }
    }
    let mut state = State { edges, index: vec![None; n], low: vec![0; n], on_stack: vec![false; n], stack: Vec::new(), next: 0, found: Vec::new() };
    for v in 0..n {
        if state.index[v].is_none() {
            visit(v, &mut state);
        }
    }
    state.found
}

/// How the plan ranks two tasks: smaller comes first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    set_aside: bool,
    waiting: bool,
    /// Heavy, on a day that takes none: after the others.
    too_heavy: bool,
    not_started: bool,
    latest_start: i64,
    priority: u8,
    unblocks: Reverse<usize>,
    left: u32,
    created: i64,
    title: String,
    index: usize,
}

/// The plan for `tasks`, from `today`. `spent` gives the minutes already
/// spent on each task (by UID); `set_aside` the tasks put off for today ("Not now").
pub fn plan(tasks: &[Task], today: Date, settings: &Settings, spent: &BTreeMap<String, u32>, set_aside: &BTreeSet<String>) -> Plan {
    // One task per UID: the first one read.
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    let mut unique: Vec<&Task> = Vec::new();
    for task in tasks {
        if !task.uid.is_empty() && !index.contains_key(task.uid.as_str()) {
            index.insert(&task.uid, unique.len());
            unique.push(task);
        }
    }
    let tasks = unique;
    let n = tasks.len();
    let graph = Graph::new(&tasks, &index);
    let open: Vec<bool> = tasks.iter().map(|t| t.status.is_open()).collect();
    let open_steps: Vec<usize> = (0..n).map(|i| graph.children[i].iter().filter(|&&c| open[c]).count()).collect();
    let optional: Vec<bool> = (0..n).map(|i| is_optional(tasks[i]) || graph.ancestors(i).iter().any(|&a| is_optional(tasks[a]))).collect();
    let preds: Vec<Vec<(usize, i64)>> = (0..n).map(|i| graph.predecessors(i)).collect();

    // The order's ties, among open tasks: what each waits for, and a step before its bigger task.
    let mut order_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in (0..n).filter(|&i| open[i]) {
        for &(p, _) in &preds[i] {
            if open[p] && !order_edges[p].contains(&i) {
                order_edges[p].push(i);
            }
        }
        if let Some(p) = graph.parent[i].filter(|&p| open[p])
            && !order_edges[i].contains(&p)
        {
            order_edges[i].push(p);
        }
    }
    let found_loops = loops(n, &order_edges);
    let loop_of: BTreeMap<usize, usize> = found_loops.iter().enumerate().flat_map(|(k, l)| l.iter().map(move |&i| (i, k))).collect();
    // Inside a loop, the ties are set aside to order the rest.
    for (from, list) in order_edges.iter_mut().enumerate() {
        list.retain(|to| loop_of.get(&from).is_none_or(|k| loop_of.get(to) != Some(k)));
    }

    // Minutes left, and the day each can start from: as you estimated (shown,
    // the focus timer's), and as the plan lays it (its corrected length).
    let minutes_left = |i: usize, corrected: bool| -> u32 {
        if open_steps[i] > 0 {
            return 0;
        }
        // Every step done: only the bigger task's own tick is left.
        if !graph.children[i].is_empty() && tasks[i].estimate == 0 {
            return 5;
        }
        let own = if tasks[i].estimate > 0 { tasks[i].estimate } else { settings.default_estimate };
        let estimate = if corrected { settings.capacity.corrected.get(&tasks[i].uid).copied().unwrap_or(own) } else { own };
        // Getting there and back, getting ready: room taken too, never a pause.
        estimate.saturating_sub(spent.get(&tasks[i].uid).copied().unwrap_or(0)).max(5) + tasks[i].margins.before + tasks[i].margins.after
    };
    let left: Vec<u32> = (0..n).map(|i| minutes_left(i, false)).collect();
    let laid: Vec<u32> = (0..n).map(|i| minutes_left(i, true)).collect();
    let not_before: Vec<Option<Date>> = (0..n)
        .map(|i| {
            let mut day = tasks[i].start_date().filter(|d| *d > today);
            for &(p, gap) in &preds[i] {
                let done_on = tasks[p].completed.and_then(|t| jiff::Timestamp::from_second(t).ok()).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).date());
                if tasks[p].status == Status::Completed
                    && let Some(done) = done_on
                {
                    let free = add_days(done, gap_days(gap));
                    if free > today && day.is_none_or(|d| free > d) {
                        day = Some(free);
                    }
                }
            }
            day
        })
        .collect();

    // A plain order first, to compute latest starts backwards.
    let topological = kahn(n, &order_edges, &open, |i| (tasks[i].created, tasks[i].title.clone(), i));
    // The hours each task can take, and a day's room of them on average.
    let usable: Vec<Area> = tasks.iter().map(|t| settings.usable(t)).collect();
    let weekly: Vec<u32> = usable.iter().map(|&kinds| settings.week.iter().map(|r| r.for_kinds(kinds)).sum()).collect();
    let mut latest: Vec<Option<Date>> = vec![None; n];
    for &i in topological.iter().rev() {
        let mut finish = tasks[i].due_date();
        for &s in &order_edges[i] {
            let gap = preds[s].iter().find(|(p, _)| *p == i).map_or(0, |(_, g)| gap_days(*g));
            if let Some(start) = latest[s] {
                let bound = add_days(start, -gap);
                finish = Some(finish.map_or(bound, |f| f.min(bound)));
            }
        }
        // The days its work takes at the pace of its hours; without hours for it, none
        // (not a minute a day, which made such a task the most pressing of all).
        let days = if weekly[i] == 0 { 0 } else { i64::from(laid[i]) * 7 / i64::from(weekly[i]) };
        latest[i] = finish.map(|f| add_days(f, -days));
    }

    // How many open tasks wait for each, further down too.
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in (0..n).filter(|&i| open[i]) {
        for &(p, _) in &preds[i] {
            if open[p] {
                dependents[p].push(i);
            }
        }
    }
    let unblocks: Vec<usize> = (0..n)
        .map(|i| {
            let mut seen = BTreeSet::new();
            let mut stack = dependents[i].clone();
            while let Some(j) = stack.pop() {
                if j != i && seen.insert(j) {
                    stack.extend(dependents[j].iter().copied());
                }
            }
            seen.len()
        })
        .collect();

    let waits_for: Vec<Vec<usize>> = (0..n).map(|i| preds[i].iter().map(|&(p, _)| p).filter(|&p| open[p]).collect()).collect();
    // How heavy each task is planned: its ratings (said after it, else forecast), else its word.
    let capacity = &settings.capacity;
    let rates: Vec<crate::capacity::Rates> = tasks.iter().map(|t| capacity.rates_of(t)).collect();
    let heavy: Vec<bool> = rates.iter().map(|r| r.level() == crate::demands::Level::Heavy).collect();
    let column = |i: usize| match tasks[i].status {
        Status::Completed | Status::Cancelled => Column::Done,
        Status::InProcess => Column::Doing,
        Status::NeedsAction if !waits_for[i].is_empty() || not_before[i].is_some() => Column::Waiting,
        Status::NeedsAction => Column::Ready,
    };
    let rank = |i: usize| Rank {
        set_aside: set_aside.contains(&tasks[i].uid),
        waiting: column(i) == Column::Waiting,
        too_heavy: heavy[i] && settings.heavy_today == 0,
        not_started: tasks[i].status != Status::InProcess,
        latest_start: latest[i].map_or(i64::MAX, day_number),
        unblocks: Reverse(unblocks[i]),
        priority: if tasks[i].priority == 0 { 5 } else { tasks[i].priority },
        left: left[i],
        created: tasks[i].created,
        title: tasks[i].title.to_lowercase(),
        index: i,
    };

    // Free to do now: nothing it waits for still open, no day ahead to wait for;
    // a step started too soon waits like the others.
    let free_now = |i: usize| waits_for[i].is_empty() && not_before[i].is_none();
    let mut ready: Vec<usize> = (0..n).filter(|&i| open[i] && open_steps[i] == 0 && free_now(i) && (column(i) == Column::Doing || (column(i) == Column::Ready && !optional[i]))).collect();
    ready.sort_by_key(|&i| rank(i));
    // Streams: tasks tied by waiting or by being steps of one bigger task (union-find).
    let mut root: Vec<usize> = (0..n).collect();
    fn find(root: &mut [usize], mut i: usize) -> usize {
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    for i in 0..n {
        let tied: Vec<usize> = preds[i].iter().map(|&(p, _)| p).chain(graph.parent[i]).collect();
        for j in tied {
            let (a, b) = (find(&mut root, i), find(&mut root, j));
            root[a.max(b)] = a.min(b);
        }
    }
    let stream: Vec<usize> = (0..n).map(|i| find(&mut root, i)).collect();
    let aside_streams: BTreeSet<usize> = (0..n).filter(|&i| set_aside.contains(&tasks[i].uid)).map(|i| stream[i]).collect();
    let not_aside = |i: &&usize| !set_aside.contains(&tasks[**i].uid);
    // The next step: it keeps part of today (below), so that Now and the day agree.
    let next: Option<usize> = ready.iter().filter(not_aside).find(|&&i| !aside_streams.contains(&stream[i])).or_else(|| ready.iter().find(not_aside)).copied();

    // The plan's order, then the days: first days with room, after what each waits for.
    let order = kahn(n, &order_edges, &open, rank);
    // The days each task's office opens: its own hours, else offices' usual days.
    let office_days: Vec<[bool; 7]> = tasks
        .iter()
        .map(|t| {
            let own = crate::window::parse_ranges(&t.office_times);
            if own.is_empty() { settings.office_days } else { crate::window::days_open(&own) }
        })
        .collect();
    // A step given a time today by hand (dragged in the day, `Task::at`): today, whatever the room.
    let pinned: Vec<bool> = tasks.iter().map(|t| t.at_on(today, &TimeZone::system()).is_some()).collect();
    // A day's room for tasks, in minutes: today's by its weather.
    let room_for_tasks = |day: Date| settings.room_on(day).total() * if day == today { settings.today_percent } else { 100 } / 100;
    // The half hour kept on a day for the slot of time for you after its costliest block.
    let gain_kept = |day: Date| if settings.room_on(day).total() >= 120 { capacity.gain_slot } else { 0 };
    // How a day's total spreads (`capacity::Spread`): today's as its finished tasks say it goes.
    let spread_on = |day: Date| -> Option<(crate::capacity::Spread, f32)> {
        let spread = capacity.spread?;
        Some(if day == today { (crate::capacity::Spread { day: if capacity.today_sigma > 0.0 { capacity.today_sigma } else { spread.day }, ..spread }, capacity.today_effect) } else { (spread, 0.0) })
    };
    // Each task into the first days with room, each day keeping free time for its
    // steps running long as it fills; `even`, the most a day holds.
    let layout = |even: Option<&Load>| -> Layout {
        // Minutes taken from each day's room, kind by kind (as `Settings::room_on` lists them).
        let mut used: BTreeMap<Date, Vec<u32>> = BTreeMap::new();
        let mut heavy_used: BTreeMap<Date, u32> = BTreeMap::new();
        // What the tasks laid each day weigh (`capacity::Load`).
        let mut loaded: BTreeMap<Date, Load> = BTreeMap::new();
        // The minutes laid each day, summed and squared: what its free time grows with.
        let mut sums: BTreeMap<Date, (f32, f32)> = BTreeMap::new();
        let mut out = Layout { start: vec![None; n], finish: vec![None; n], on_start: vec![0; n], days: BTreeMap::new(), left: BTreeMap::new() };
        for &i in &order {
            let mut earliest = not_before[i].unwrap_or(today).max(today);
            for &(p, gap) in &preds[i] {
                if let Some(end) = out.finish[p] {
                    earliest = earliest.max(add_days(end, gap_days(gap)));
                }
            }
            if optional[i] && tasks[i].status != Status::InProcess {
                continue;
            }
            if open_steps[i] > 0 {
                // A bigger task spans its steps.
                let steps = || graph.children[i].iter().filter(|&&c| open[c]);
                out.start[i] = steps().filter_map(|&c| out.start[c]).min().or(Some(earliest));
                out.finish[i] = steps().filter_map(|&c| out.finish[c]).max().or(Some(earliest));
                continue;
            }
            let minutes_all = laid[i].max(SHORTEST);
            // Given a time today by hand: today, whole, whatever the room, the budgets or what it waits for.
            if pinned[i] {
                let room = settings.room_on(today);
                let taken = used.entry(today).or_insert_with(|| vec![0; room.0.len()]);
                let mut owed = minutes_all + settings.pause;
                for k in (0..room.0.len()).filter(|&k| room.0[k].0.meets(usable[i])) {
                    let take = room.0[k].1.saturating_sub(taken[k]).min(owed);
                    taken[k] += take;
                    owed -= take;
                }
                if heavy[i] {
                    *heavy_used.entry(today).or_insert(0) += 1;
                }
                crate::capacity::add(loaded.entry(today).or_insert([0.0; 5]), &rates[i].load(minutes_all));
                let sum = sums.entry(today).or_insert((0.0, 0.0));
                (sum.0, sum.1) = (sum.0 + minutes_all as f32, sum.1 + (minutes_all as f32).powi(2));
                out.days.entry(today).or_default().push((i, minutes_all));
                (out.start[i], out.finish[i], out.on_start[i]) = (Some(today), Some(today), minutes_all);
                continue;
            }
            // The most room a day it can go on has for it: its kinds' hours, on its office's days.
            let biggest = (0..7).filter(|&d| !tasks[i].office_hours || office_days[i][d]).map(|d| settings.week[d].for_kinds(usable[i])).max().unwrap_or(0);
            // No hours ever for what it is for (on days its office opens): it waits for none.
            if biggest == 0 {
                (out.start[i], out.finish[i]) = (Some(earliest), Some(earliest));
                continue;
            }
            let (mut day, mut minutes) = (earliest, minutes_all);
            let margins = !tasks[i].margins.is_empty();
            // Done in one go when it is short, or has margins (going there twice is
            // not the same step), and some day can hold it.
            let whole = (minutes <= WHOLE || margins) && minutes <= biggest;
            // With margins and longer than any day's room: a day of its own.
            let alone = margins && minutes > biggest;
            let per_hour = rates[i].per_hour();
            for _ in 0..3 * 366 {
                let weekday = day.weekday().to_monday_zero_offset() as usize;
                // A day full of heavy tasks takes no other; the days around a heavy event, one fewer.
                let lighter = u32::from(capacity.lightened.contains(&day));
                let heavy_limit = if day == today { settings.heavy_today } else { settings.heavy_per_day }.saturating_sub(lighter);
                let heavy_full = heavy[i] && settings.heavy_per_day > 0 && heavy_used.get(&day).copied().unwrap_or(0) >= heavy_limit;
                if !(heavy_full || settings.closed.contains(&day) || (tasks[i].office_hours && !office_days[i][weekday])) {
                    let room = settings.room_on(day);
                    let percent = if day == today { settings.today_percent } else { 100 };
                    let taken = used.entry(day).or_insert_with(|| vec![0; room.0.len()]);
                    // The hours of its kinds, those open to fewer kinds first: overlaps stay for either.
                    let mut kinds: Vec<usize> = (0..room.0.len()).filter(|&k| room.0[k].0.meets(usable[i])).collect();
                    kinds.sort_by_key(|&k| room.0[k].0.count());
                    let free_in = |k: usize, taken: &[u32]| (room.0[k].1 * percent / 100).saturating_sub(taken[k]);
                    // Room for the step and the pause after it; the day's first step may fill its
                    // hours without one (a step of an hour in an hour's window is not cut).
                    let first = kinds.iter().all(|&k| taken[k] == 0);
                    let pause = if first { 0 } else { settings.pause };
                    let open_here: u32 = kinds.iter().map(|&k| free_in(k, taken)).sum::<u32>().saturating_sub(pause);
                    // Kept free: the slot of time for you after the day's costliest block, and time for
                    // steps running long: the day, this part laid, must still hold the 85th percentile
                    // of its total (`capacity::slack_estimate`), a third of its room at most.
                    let mut free = open_here.saturating_sub(gain_kept(day));
                    if let Some((spread, effect)) = spread_on(day) {
                        let total = room_for_tasks(day);
                        let cap = crate::capacity::SLACK_SHARE * total as f32;
                        let day_left = total.saturating_sub(taken.iter().sum::<u32>()).saturating_sub(pause).saturating_sub(gain_kept(day)) as f32;
                        let (sum, squares) = sums.get(&day).copied().unwrap_or((0.0, 0.0));
                        let fits = |p: u32| p as f32 + crate::capacity::slack_estimate(sum + p as f32, squares + (p as f32).powi(2), spread, effect).min(cap) <= day_left + 1e-3;
                        let upper = free.min(minutes);
                        free = if fits(upper) {
                            upper
                        } else if !fits(0) {
                            0
                        } else {
                            let (mut lo, mut hi) = (0, upper);
                            while hi - lo > 1 {
                                let mid = (lo + hi) / 2;
                                if fits(mid) { lo = mid } else { hi = mid }
                            }
                            lo
                        };
                    }
                    let mut part = shaped(free, minutes, whole, alone, first);
                    // The next step keeps part of today, as before free time was kept: what is left of
                    // today's room is its when the time kept free would leave it none. Only a step that
                    // cannot be cut (its margins) goes whole to the first day that holds it.
                    if part < minutes.min(SHORTEST) && !alone && next == Some(i) && day == today {
                        part = shaped(open_here, minutes, whole, alone, first);
                    }
                    // What the day's costs still hold (docs/capacity.md, "Planning").
                    if part > 0
                        && let Some(mut limit) = capacity.day_limit(day, percent as f32 / 100.0)
                    {
                        if let Some(even) = even {
                            for s in 0..5 {
                                limit[s] = limit[s].min(even[s]);
                            }
                        }
                        let tasks_here = loaded.get(&day).copied().unwrap_or([0.0; 5]);
                        let mut held = capacity.held.get(&day).copied().unwrap_or([0.0; 5]);
                        crate::capacity::add(&mut held, &tasks_here);
                        let fits_under = |m: u32, base: &Load, limit: &Load| {
                            let load = rates[i].load(m);
                            (0..5).all(|s| load[s] <= 0.0 || base[s] + load[s] <= limit[s] + 1e-3)
                        };
                        let fits = |m: u32, base: &Load| fits_under(m, base, &limit);
                        if !fits(part, &held) {
                            // Too heavy for an empty ordinary day (no weather, no event around it):
                            // the first day with no other task takes it all the same.
                            let ordinary = capacity.limit.map_or(limit, |l| l.map(|b| b * crate::capacity::FILL));
                            let never = !fits_under(if whole || alone { minutes } else { SHORTEST }, &[0.0; 5], &ordinary);
                            part = if never {
                                if tasks_here[crate::capacity::TOTAL] <= 0.0 { part } else { 0 }
                            } else if whole || alone {
                                0
                            } else {
                                // The part the budgets let in, a quarter of an hour at least, a quarter left for later.
                                let most = (0..5).filter(|&s| per_hour[s] > 0.0).map(|s| ((limit[s] - held[s]).max(0.0) / per_hour[s] * 60.0) as u32).min().unwrap_or(part);
                                let mut fitting = most.min(part);
                                if fitting < minutes && minutes - fitting < SHORTEST {
                                    fitting = fitting.min(minutes.saturating_sub(SHORTEST));
                                }
                                if fitting >= SHORTEST && fits(fitting, &held) { fitting } else { 0 }
                            };
                        }
                    }
                    if part > 0 && (part >= minutes.min(SHORTEST) || alone) {
                        let mut owed = part + settings.pause;
                        for &k in &kinds {
                            let take = free_in(k, taken).min(owed);
                            taken[k] += take;
                            owed -= take;
                        }
                        if heavy[i] {
                            *heavy_used.entry(day).or_insert(0) += 1;
                        }
                        crate::capacity::add(loaded.entry(day).or_insert([0.0; 5]), &rates[i].load(part));
                        let sum = sums.entry(day).or_insert((0.0, 0.0));
                        (sum.0, sum.1) = (sum.0 + part as f32, sum.1 + (part as f32).powi(2));
                        out.days.entry(day).or_default().push((i, part));
                        minutes -= part;
                        if out.start[i].is_none() {
                            (out.start[i], out.on_start[i]) = (Some(day), part);
                        }
                        if minutes == 0 {
                            break;
                        }
                    }
                }
                day = add_days(day, 1);
            }
            out.finish[i] = Some(day);
            out.start[i] = out.start[i].or(Some(day));
        }
        // What each day has left once its steps and its slot for you are laid.
        out.left = used.iter().map(|(&day, taken)| (day, room_for_tasks(day).saturating_sub(taken.iter().sum::<u32>()).saturating_sub(gain_kept(day)))).collect();
        out
    };
    // Laid once, each day keeping free time for its steps running long as it
    // fills (TE16–18); days kept even, again, each day held to the coming
    // week's mean (criterion 35).
    let mut placed = layout(None);
    if capacity.even && capacity.limit.is_some() {
        let week: Vec<Date> = (0..7).map(|k| add_days(today, k)).filter(|d| !settings.closed.contains(d) && settings.room_on(*d).total() > 0).collect();
        let mut sum: Load = [0.0; 5];
        for date in &week {
            if let Some(held) = capacity.held.get(date) {
                crate::capacity::add(&mut sum, held);
            }
            for &(i, m) in placed.days.get(date).into_iter().flatten() {
                crate::capacity::add(&mut sum, &rates[i].load(m));
            }
        }
        if !week.is_empty() {
            // A cost nothing weighs on this week stays unbounded.
            let target: Load = sum.map(|v| if v > 0.0 { v / week.len() as f32 } else { f32::INFINITY });
            placed = layout(Some(&target));
        }
    }
    // The free time each day keeps: the 85th percentile of the total of the steps
    // laid there, by the seeded simulation (`capacity::slack`), a third of its
    // room at most, and never more than it has left.
    let mut slack: BTreeMap<Date, u32> = BTreeMap::new();
    for (&date, steps) in &placed.days {
        let Some((spread, effect)) = spread_on(date) else { break };
        let medians: Vec<u32> = steps.iter().map(|&(_, m)| m).collect();
        let uids: Vec<&str> = steps.iter().map(|&(i, _)| tasks[i].uid.as_str()).collect();
        let cap = (crate::capacity::SLACK_SHARE * room_for_tasks(date) as f32) as u32;
        let kept = crate::capacity::slack(&medians, spread, effect, crate::capacity::seed(date, &uids)).min(cap).min(placed.left.get(&date).copied().unwrap_or(0));
        if kept > 0 {
            slack.insert(date, kept);
        }
    }
    let Layout { start, finish, on_start, days: placed_days, .. } = placed;

    let depth = |i: usize| graph.ancestors(i).len();
    let mut items = BTreeMap::new();
    for i in 0..n {
        let tight = open[i] && matches!((finish[i], tasks[i].due_date()), (Some(f), Some(d)) if f > d);
        items.insert(
            tasks[i].uid.clone(),
            Planned {
                uid: tasks[i].uid.clone(),
                column: column(i),
                waits_for: waits_for[i].iter().map(|&p| tasks[p].uid.clone()).collect(),
                not_before: not_before[i],
                unblocks: unblocks[i],
                latest_start: latest[i],
                start: start[i],
                finish: finish[i],
                tight,
                in_loop: loop_of.contains_key(&i),
                open_steps: open_steps[i],
                depth: depth(i),
                left: left[i],
                laid: laid[i],
                level: rates[i].level().id(),
                on_start: on_start[i],
                optional: optional[i],
            },
        );
    }
    Plan {
        items,
        order: order.iter().map(|&i| tasks[i].uid.clone()).collect(),
        ready: ready.iter().map(|&i| tasks[i].uid.clone()).collect(),
        next: next.map(|i| tasks[i].uid.clone()),
        loops: found_loops.iter().map(|l| l.iter().map(|&i| tasks[i].uid.clone()).collect()).collect(),
        streams: (0..n).map(|i| (tasks[i].uid.clone(), stream[i])).collect(),
        days: placed_days.into_iter().map(|(date, steps)| (date, steps.into_iter().map(|(i, m)| (tasks[i].uid.clone(), m)).collect())).collect(),
        slack,
    }
}

/// Where the tasks went: their first and last days, the minutes of their first,
/// what each day holds (task index, minutes), and what it has left.
struct Layout {
    start: Vec<Option<Date>>,
    finish: Vec<Option<Date>>,
    on_start: Vec<u32>,
    days: BTreeMap<Date, Vec<(usize, u32)>>,
    left: BTreeMap<Date, u32>,
}

/// The part of a step worth starting in `free` minutes: the whole rest; for a
/// long step, a quarter of an hour at least, leaving at least as much for
/// later; a step done in one go (`whole`), all or nothing; one that needs a
/// day of its own (`alone`), all of an untouched day.
fn shaped(free: u32, minutes: u32, whole: bool, alone: bool, first: bool) -> u32 {
    if alone {
        return if first { minutes } else { 0 };
    }
    let part = free.min(minutes);
    if part < minutes && (whole || minutes - part < SHORTEST) {
        return if whole { 0 } else { part.min(minutes.saturating_sub(SHORTEST)) };
    }
    part
}

/// Rating × hours, per cost and in total (`capacity::Load`).
type Load = crate::capacity::Load;

/// Kahn's algorithm over the open tasks, the smallest key first among those free.
fn kahn<K: Ord>(n: usize, edges: &[Vec<usize>], open: &[bool], key: impl Fn(usize) -> K) -> Vec<usize> {
    let mut incoming = vec![0usize; n];
    for (from, list) in edges.iter().enumerate() {
        if open[from] {
            for &to in list {
                incoming[to] += 1;
            }
        }
    }
    let mut free: BinaryHeap<Reverse<(K, usize)>> = (0..n).filter(|&i| open[i] && incoming[i] == 0).map(|i| Reverse((key(i), i))).collect();
    let mut out = Vec::with_capacity(n);
    while let Some(Reverse((_, i))) = free.pop() {
        out.push(i);
        for &to in &edges[i] {
            incoming[to] -= 1;
            if incoming[to] == 0 {
                free.push(Reverse((key(to), to)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offices_and_days_off() {
        // Saturday 3 October 2026.
        let today: Date = "2026-10-03".parse().unwrap();
        let call = Task { uid: "call".into(), title: "Call the office".into(), estimate: 15, office_hours: true, ..Task::default() };
        let mut settings = Settings::flat([60; 7]);
        let planned = plan(&[call.clone()], today, &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(planned.items["call"].start.map(|d| d.to_string()).as_deref(), Some("2026-10-05"), "on Monday, when offices open");
        // Monday off: Tuesday.
        settings.closed.insert("2026-10-05".parse().unwrap());
        let planned = plan(&[call], today, &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(planned.items["call"].start.map(|d| d.to_string()).as_deref(), Some("2026-10-06"));
    }

    #[test]
    fn a_step_started_too_soon_waits() {
        let mut started = waits(task("send", 10), "call");
        started.status = Status::InProcess;
        let tasks = vec![task("call", 10), started];
        let plan = plan(&tasks, today(), &Settings::default(), &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(plan.items["send"].column, Column::Doing, "still shown as started");
        assert!(!plan.ready.contains(&"send".to_string()), "never proposed before what it waits for");
        assert!(plan.ready.contains(&"call".to_string()));
    }

    #[test]
    fn an_office_with_its_own_hours() {
        // Saturday 3 October: an office open on Saturday mornings takes the call today.
        let call = Task { uid: "call".into(), estimate: 15, office_hours: true, office_times: "sa 09:00-12:00".into(), ..Task::default() };
        let usual = Task { uid: "usual".into(), estimate: 15, office_hours: true, ..Task::default() };
        let settings = Settings::flat([60; 7]);
        let plan = plan(&[call, usual], day("2026-10-03"), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(plan.items["call"].start, Some(day("2026-10-03")));
        assert_eq!(plan.items["usual"].start, Some(day("2026-10-05")), "the usual offices open Monday");
    }

    #[test]
    fn not_now_takes_its_stream_with_it() {
        let today: Date = "2026-10-03".parse().unwrap();
        let task = |uid: &str| Task { uid: uid.into(), title: uid.into(), estimate: 15, ..Task::default() };
        // A request with two steps: one free, one waiting for a certificate, which waits for a call.
        let call = Task { due: "2026-10-20".into(), ..task("call") };
        let arrives = Task { relations: vec![Relation { kind: "DEPENDS-ON".into(), uid: "call".into(), gap: 0 }], ..task("arrives") };
        let request = Task { due: "2026-10-30".into(), ..task("request") };
        let form = Task { relations: vec![Relation { kind: "PARENT".into(), uid: "request".into(), gap: 0 }], ..task("form") };
        let send = Task { relations: vec![Relation { kind: "PARENT".into(), uid: "request".into(), gap: 0 }, Relation { kind: "DEPENDS-ON".into(), uid: "arrives".into(), gap: 0 }], ..task("send") };
        // Something else entirely.
        let shelf = task("shelf");
        let tasks = vec![call, arrives, request, form, send, shelf];
        let none = BTreeSet::new();
        let planned = plan(&tasks, today, &Settings::default(), &BTreeMap::new(), &none);
        assert_eq!(planned.next.as_deref(), Some("call"));
        assert_eq!(planned.streams["call"], planned.streams["form"], "the form shares the call's stream through its bigger task");
        assert_ne!(planned.streams["call"], planned.streams["shelf"]);
        // The call put off: the free form is tied to it, the shelf is not.
        let aside: BTreeSet<String> = ["call".to_string()].into();
        assert_eq!(plan(&tasks, today, &Settings::default(), &BTreeMap::new(), &aside).next.as_deref(), Some("shelf"));
        // Nothing else: the tied step all the same.
        let aside: BTreeSet<String> = ["call".to_string(), "shelf".to_string()].into();
        assert_eq!(plan(&tasks, today, &Settings::default(), &BTreeMap::new(), &aside).next.as_deref(), Some("form"));
    }
    use crate::tasks::Relation;

    fn task(uid: &str, minutes: u32) -> Task {
        Task { uid: uid.into(), title: uid.into(), estimate: minutes, ..Task::default() }
    }

    fn waits(mut t: Task, on: &str) -> Task {
        t.relations.push(Relation { kind: "DEPENDS-ON".into(), uid: on.into(), gap: 0 });
        t
    }

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    /// Saturday 3 October 2026.
    fn today() -> Date {
        day("2026-10-03")
    }

    #[test]
    fn waits_in_order_and_says_loops() {
        let tasks = vec![waits(task("c", 15), "b"), waits(task("b", 15), "a"), task("a", 15), waits(task("x", 15), "y"), waits(task("y", 15), "x")];
        let plan = plan(&tasks, today(), &Settings::default(), &BTreeMap::new(), &BTreeSet::new());
        let position = |u: &str| plan.order.iter().position(|o| o == u).unwrap();
        assert!(position("a") < position("b") && position("b") < position("c"), "{:?}", plan.order);
        assert_eq!(plan.loops, vec![vec!["x".to_string(), "y".to_string()]]);
        assert_eq!(plan.items["c"].column, Column::Waiting);
        assert_eq!(plan.items["c"].waits_for, vec!["b"]);
        assert_eq!((plan.items["a"].column, plan.items["a"].unblocks), (Column::Ready, 2));
        assert!(plan.items["x"].in_loop && plan.order.len() == 5);
    }

    #[test]
    fn the_next_step() {
        let mut deadline = task("deadline", 60);
        deadline.due = "2026-10-09".into();
        let mut started = task("started", 30);
        started.status = Status::InProcess;
        let tasks = vec![task("small", 5), deadline.clone(), waits(task("after", 5), "unlocking"), task("unlocking", 30)];
        let plan_of = |tasks: &[Task], aside: &[&str]| plan(tasks, today(), &Settings::default(), &BTreeMap::new(), &aside.iter().map(|s| s.to_string()).collect());
        // A date first; then what frees the most; then the smaller.
        assert_eq!(plan_of(&tasks, &[]).ready, vec!["deadline", "unlocking", "small"]);
        assert_eq!(plan_of(&tasks, &["deadline"]).next.as_deref(), Some("unlocking"));
        let mut with_started = tasks.clone();
        with_started.push(started);
        assert_eq!(plan_of(&with_started, &[]).next.as_deref(), Some("started"));
    }

    #[test]
    fn days_with_room_after_what_comes_first() {
        // An hour on weekdays: Saturday the 3rd and Sunday the 4th have none.
        let mut letter = task("letter", 15);
        letter.status = Status::Completed;
        letter.completed = Some(day("2026-10-01").to_zoned(jiff::tz::TimeZone::system()).unwrap().timestamp().as_second());
        let mut answer = task("answer", 15);
        answer.relations.push(Relation { kind: "DEPENDS-ON".into(), uid: "letter".into(), gap: 14 * 1440 });
        let mut late = task("late", 45);
        late.due = "2026-10-05".into();
        let tasks = vec![task("one", 45), waits(task("two", 30), "one"), letter, answer, late];
        let plan = plan(&tasks, today(), &Settings::flat([60, 60, 60, 60, 60, 0, 0]), &BTreeMap::new(), &BTreeSet::new());
        let days = |u: &str| (plan.items[u].start.unwrap().to_string(), plan.items[u].finish.unwrap().to_string());
        // "late" ranks first (its date), and fills Monday's first 45 minutes.
        assert_eq!(days("late"), ("2026-10-05".into(), "2026-10-05".into()));
        assert!(!plan.items["late"].tight);
        // "one" is done in one go: not in Monday's last 15 minutes, on Tuesday; "two" follows on Wednesday.
        assert_eq!(days("one"), ("2026-10-06".into(), "2026-10-06".into()));
        assert_eq!(days("two"), ("2026-10-07".into(), "2026-10-07".into()));
        // Two weeks after the letter went: not before the 15th.
        assert_eq!(plan.items["answer"].not_before, Some(day("2026-10-15")));
        assert_eq!((plan.items["answer"].column, days("answer").0), (Column::Waiting, "2026-10-15".to_string()));
    }

    #[test]
    fn optional_tasks_are_never_proposed() {
        let mut music = task("music", 30);
        music.categories = vec!["JOY".into()];
        let mut parked = task("parked", 15);
        parked.categories = vec!["someday".into()];
        let mut step = task("parked-step", 5);
        step.relations.push(Relation { kind: "PARENT".into(), uid: "parked".into(), gap: 0 });
        let tasks = vec![music, parked, step, task("letter", 15)];
        let plan = plan(&tasks, today(), &Settings::default(), &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(plan.ready, vec!["letter"]);
        assert!(plan.items["parked-step"].optional && plan.items["music"].optional && !plan.items["letter"].optional);
        assert_eq!((plan.items["music"].start, plan.items["parked"].finish), (None, None));
    }

    #[test]
    fn steps_of_a_bigger_task() {
        let mut step_one = task("step-one", 20);
        step_one.relations.push(Relation { kind: "PARENT".into(), uid: "big".into(), gap: 0 });
        let mut step_two = waits(task("step-two", 20), "step-one");
        step_two.relations.push(Relation { kind: "PARENT".into(), uid: "big".into(), gap: 0 });
        let big = waits(task("big", 0), "papers");
        let tasks = vec![big, step_one, step_two, task("papers", 10), waits(task("then", 10), "big")];
        let plan = plan(&tasks, today(), &Settings::default(), &BTreeMap::new(), &BTreeSet::new());
        // The steps wait for what their bigger task waits for; what waits for it, for its steps.
        assert_eq!(plan.items["step-one"].waits_for, vec!["papers"]);
        assert_eq!((plan.items["big"].open_steps, plan.items["step-one"].depth), (2, 1));
        assert_eq!(plan.ready, vec!["papers"]);
        let position = |u: &str| plan.order.iter().position(|o| o == u).unwrap();
        assert!(position("papers") < position("step-one") && position("step-two") < position("big") && position("big") < position("then"), "{:?}", plan.order);
        assert_eq!(plan.items["big"].start, plan.items["step-one"].start);
    }
    #[test]
    fn time_budget_until_a_date() {
        let today = Date::constant(2026, 10, 5); // a Monday
        let task = |uid: &str, estimate: u32, due: &str| Task { uid: uid.into(), title: uid.into(), estimate, due: due.into(), ..Task::default() };
        // Asked for Wednesday: a form (1 h) that waits for a scan (30 min), and two steps of a letter (20 min each, the letter's own time not counted).
        let mut form = task("form", 60, "2026-10-07");
        form.relations = vec![Relation { kind: "DEPENDS-ON".into(), uid: "scan".into(), gap: 0 }];
        let scan = task("scan", 30, "");
        let letter = task("letter", 90, "2026-10-06");
        let mut step_a = task("a", 20, "");
        step_a.relations = vec![Relation { kind: "PARENT".into(), uid: "letter".into(), gap: 0 }];
        let mut step_b = task("b", 20, "");
        step_b.relations = vec![Relation { kind: "PARENT".into(), uid: "letter".into(), gap: 0 }];
        // Asked later, and a joy: neither counts.
        let later = task("later", 120, "2026-10-20");
        let mut joy = task("joy", 60, "2026-10-06");
        joy.categories = vec!["joy".into()];
        let tasks = vec![form, scan, letter, step_a, step_b, later, joy];
        let mut settings = Settings::flat([60, 60, 60, 60, 60, 0, 0]);
        settings.today_percent = 50;
        let plan = plan(&tasks, today, &settings, &BTreeMap::new(), &BTreeSet::new());
        let budget = cushion(&tasks, &plan, &settings, today, Date::constant(2026, 10, 7));
        assert_eq!(budget.need, 60 + 30 + 20 + 20);
        // Monday at half (the weather), Tuesday and Wednesday whole.
        assert_eq!(budget.room, 30 + 60 + 60);
        settings.closed.insert(Date::constant(2026, 10, 6));
        assert_eq!(cushion(&tasks, &plan, &settings, today, Date::constant(2026, 10, 7)).room, 30 + 60);
    }

    fn rated(uid: &str, minutes: u32, cognitive: u8) -> Task {
        Task { demands: crate::demands::Demands { cognitive: Some(cognitive), ..crate::demands::Demands::default() }, ..task(uid, minutes) }
    }

    fn with_budget(limit: f32) -> Settings {
        let mut settings = Settings::flat([480; 7]);
        settings.capacity.limit = Some([limit; 5]);
        settings
    }

    #[test]
    fn each_day_filled_to_its_budget() {
        // A budget of 10 a day: 8.5 planned. Three hours rated 8 (cognitive): one a day.
        let today = day("2026-10-05");
        let tasks = vec![rated("a", 60, 8), rated("b", 60, 8), rated("c", 60, 8)];
        let made = plan(&tasks, today, &with_budget(10.0), &BTreeMap::new(), &BTreeSet::new());
        let starts: BTreeSet<Option<Date>> = ["a", "b", "c"].iter().map(|u| made.items[*u].start).collect();
        assert_eq!(starts.len(), 3, "one a day: {starts:?}");
        // Without a budget, all today, as before.
        let free = plan(&tasks, today, &Settings::flat([480; 7]), &BTreeMap::new(), &BTreeSet::new());
        assert!(["a", "b", "c"].iter().filter(|u| free.items[**u].start == Some(today)).count() >= 2);
        // Light ones fill around: a long light step goes on with the budget's room, cut over days.
        let long = rated("long", 300, 2);
        let made = plan(std::slice::from_ref(&long), today, &with_budget(4.0), &BTreeMap::new(), &BTreeSet::new());
        assert!(made.items["long"].finish > Some(today), "2 × 5 h = 10 > 3.4: spread: {:?}", made.items["long"]);
        // Haze holds 60 %: the hour rated 8 waits for a day that holds it.
        let mut haze = with_budget(10.0);
        haze.today_percent = 60;
        let made = plan(&[rated("a", 60, 8)], today, &haze, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.items["a"].start, Some(day("2026-10-06")));
    }

    #[test]
    fn heavier_than_any_day_gets_a_day_of_its_own() {
        let today = day("2026-10-05");
        // A budget of 4 (3.4 planned): an hour rated 8 never fits; a small step first today.
        let small = Task { due: "2026-10-05".into(), ..task("small", 15) };
        let huge = Task { due: "2026-10-30".into(), ..rated("huge", 60, 8) };
        let made = plan(&[small, huge], today, &with_budget(4.0), &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.items["small"].start, Some(today));
        assert_eq!((made.items["huge"].start, made.items["huge"].finish), (Some(day("2026-10-06")), Some(day("2026-10-06"))), "alone, whole, the next day");
    }

    #[test]
    fn even_days_spread_the_week() {
        let today = day("2026-10-05");
        let tasks: Vec<Task> = (0..7).map(|k| task(&format!("t{k}"), 60)).collect();
        let mut settings = with_budget(100.0);
        let all_at_once = plan(&tasks, today, &settings, &BTreeMap::new(), &BTreeSet::new());
        assert!(tasks.iter().all(|t| all_at_once.items[&t.uid].start == Some(today)), "varied: the first day holds them all");
        settings.capacity.even = true;
        let even = plan(&tasks, today, &settings, &BTreeMap::new(), &BTreeSet::new());
        let days: BTreeSet<Option<Date>> = tasks.iter().map(|t| even.items[&t.uid].start).collect();
        assert_eq!(days.len(), 7, "one a day: {days:?}");
        // Off by default.
        assert!(!Settings::default().capacity.even);
    }

    #[test]
    fn a_heavy_event_lightens_the_days_around_it() {
        // Tomorrow is the day before a heavy appointment: one heavy task fewer.
        let today = day("2026-10-05");
        let heavy = |uid: &str| Task { energy: "heavy".into(), ..task(uid, 20) };
        let tasks = vec![heavy("h1"), heavy("h2"), heavy("h3"), heavy("h4")];
        let mut settings = Settings::default();
        settings.capacity.lightened = BTreeSet::from([day("2026-10-06")]);
        let made = plan(&tasks, today, &settings, &BTreeMap::new(), &BTreeSet::new());
        let on = |d: &str| tasks.iter().filter(|t| made.items[&t.uid].start == Some(day(d))).count();
        assert_eq!((on("2026-10-05"), on("2026-10-06"), on("2026-10-07")), (2, 1, 1));
        // With budgets, those days hold three quarters.
        let mut budget = with_budget(10.0);
        budget.capacity.lightened = BTreeSet::from([today]);
        let made = plan(&[rated("a", 60, 8)], today, &budget, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.items["a"].start, Some(day("2026-10-06")), "8 > 8.5 × 0.75");
    }

    #[test]
    fn margins_keep_a_task_whole_across_days() {
        // An hour a day: 40 minutes and a quarter of an hour each way (70) is never cut.
        let today = day("2026-10-05");
        let errand = Task { margins: crate::demands::Margins { before: 15, after: 15 }, ..task("errand", 40) };
        let made = plan(std::slice::from_ref(&errand), today, &Settings::flat([60; 7]), &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.items["errand"].start, made.items["errand"].finish, "a day of its own, whole");
        assert_eq!(made.items["errand"].on_start, 70);
        // Without margins, the same length goes on over two days.
        let made = plan(&[task("plain", 70)], today, &Settings::flat([60; 7]), &BTreeMap::new(), &BTreeSet::new());
        assert!(made.items["plain"].finish > made.items["plain"].start);
    }

    #[test]
    fn corrected_lengths_and_what_gives_back() {
        let today = day("2026-10-05");
        let mut settings = Settings::default();
        settings.capacity.corrected = BTreeMap::from([("a".to_string(), 45)]);
        let made = plan(&[task("a", 30)], today, &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!((made.items["a"].left, made.items["a"].laid, made.items["a"].on_start), (30, 45, 45), "shown as estimated, laid as corrected");
        // Rated light with a gain: it gives back, offered rather than planned; with a date asked, planned all the same.
        let gives = Task { demands: crate::demands::Demands { cognitive: Some(2), gain: Some(7), ..crate::demands::Demands::default() }, ..task("walk", 30) };
        assert!(is_optional(&gives));
        assert!(!is_optional(&Task { due: "2026-10-09".into(), ..gives.clone() }));
        assert_eq!(plan(&[gives], today, &settings, &BTreeMap::new(), &BTreeSet::new()).items["walk"].level, "rest");
    }

    #[test]
    fn free_time_is_the_final_placement_s() {
        // Three half hours on a day of eight hours: the free time kept is the simulation's, on the steps laid there.
        let today = day("2026-10-05");
        let spread = crate::capacity::Spread::of(0.5);
        let mut settings = Settings::flat([480; 7]);
        settings.capacity.spread = Some(spread);
        let tasks = vec![task("a", 30), task("b", 30), task("c", 30)];
        let made = plan(&tasks, today, &settings, &BTreeMap::new(), &BTreeSet::new());
        let steps = &made.days[&today];
        let medians: Vec<u32> = steps.iter().map(|(_, m)| *m).collect();
        let uids: Vec<&str> = steps.iter().map(|(u, _)| u.as_str()).collect();
        assert_eq!(medians, vec![30, 30, 30]);
        assert_eq!(made.slack[&today], crate::capacity::slack(&medians, spread, 0.0, crate::capacity::seed(today, &uids)));
        // Days kept even move steps: each day's free time follows what ends up there.
        settings.capacity.limit = Some([100.0; 5]);
        settings.capacity.even = true;
        let week: Vec<Task> = (0..7).map(|k| task(&format!("t{k}"), 60)).collect();
        let made = plan(&week, today, &settings, &BTreeMap::new(), &BTreeSet::new());
        assert!(made.days.len() > 1);
        for (date, steps) in &made.days {
            let medians: Vec<u32> = steps.iter().map(|(_, m)| *m).collect();
            let uids: Vec<&str> = steps.iter().map(|(u, _)| u.as_str()).collect();
            assert_eq!(made.slack.get(date).copied().unwrap_or(0), crate::capacity::slack(&medians, spread, 0.0, crate::capacity::seed(*date, &uids)), "{date}");
        }
        // No spread, no free time: as before.
        assert!(plan(&tasks, today, &Settings::flat([480; 7]), &BTreeMap::new(), &BTreeSet::new()).slack.is_empty());
    }

    #[test]
    fn the_next_step_keeps_today_when_little_is_left() {
        // Forty minutes left today, the widest free time: the next step still takes part of today.
        let today = day("2026-10-05");
        let mut settings = Settings::flat([40, 480, 480, 480, 480, 480, 480]);
        settings.capacity.spread = Some(crate::capacity::Spread::of(0.707));
        let started = Task { status: Status::InProcess, ..task("long", 240) };
        let made = plan(&[started], today, &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!((made.next.as_deref(), made.items["long"].start), (Some("long"), Some(today)));
        assert!(made.items["long"].on_start >= SHORTEST);
        // With margins it is never cut: whole, on the first day that holds it.
        let errand = Task { status: Status::InProcess, margins: crate::demands::Margins { before: 15, after: 15 }, ..task("errand", 60) };
        let made = plan(&[errand], today, &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!((made.items["errand"].start, made.items["errand"].finish), (Some(day("2026-10-06")), Some(day("2026-10-06"))));
    }

    #[test]
    fn heavy_days_and_rest() {
        let today = Date::constant(2026, 10, 5); // a Monday
        let task = |uid: &str, energy: &str| Task { uid: uid.into(), title: uid.into(), estimate: 20, energy: energy.into(), ..Task::default() };
        let tasks = vec![task("h1", "heavy"), task("h2", "heavy"), task("h3", "heavy"), task("light", "light"), task("walk", "rest")];
        // Clear: two heavy today, the third tomorrow; the walk takes no room and is never the next step.
        let clear = Settings::default();
        let made = plan(&tasks, today, &clear, &BTreeMap::new(), &BTreeSet::new());
        let starts: Vec<Option<Date>> = ["h1", "h2", "h3"].iter().map(|u| made.items[*u].start).collect();
        assert_eq!(starts.iter().filter(|d| **d == Some(today)).count(), 2, "{starts:?}");
        assert!(starts.contains(&Some(Date::constant(2026, 10, 6))));
        assert!(made.items["walk"].optional && made.next.as_deref() != Some("walk"));
        // Fog: no heavy task today, and the light one comes first.
        let fog = Settings { heavy_today: 0, today_percent: 30, ..Settings::default() };
        let made = plan(&tasks, today, &fog, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.next.as_deref(), Some("light"));
        assert!(["h1", "h2", "h3"].iter().all(|u| made.items[*u].start > Some(today)));
    }


    fn window(day: &str, start: &str, end: &str, kind: &str) -> AdminWindow {
        AdminWindow { day: day.into(), start: start.into(), end: Some(end.into()), minutes: 0, kind: Some(kind.into()) }
    }

    #[test]
    fn each_kind_in_its_own_hours() {
        // Monday 5 October: work 9–12, admin 17–18; an older Sioul's free time on Saturday, 10–12, left aside.
        let windows = vec![window("monday", "09:00", "12:00", "work"), window("monday", "17:00", "18:00", "admin"), window("saturday", "10:00", "12:00", "leisure")];
        let settings = Settings { pause: 0, ..Settings::of_hours(&windows, TaskAreas::usual()) };
        let monday = day("2026-10-05");
        let tagged = |uid: &str, minutes: u32, category: &str| Task { categories: vec![category.into()], ..task(uid, minutes) };
        let tasks = vec![tagged("report", 150, "work"), tagged("tax", 90, "admin"), tagged("friends", 60, "friends"), tagged("more-work", 60, "work")];
        let made = plan(&tasks, monday, &settings, &BTreeMap::new(), &BTreeSet::new());
        let days = |u: &str| (made.items[u].start.unwrap().to_string(), made.items[u].finish.unwrap().to_string());
        // Work fills Monday's work hours, the smaller step first; the report goes on the next Monday (Tuesday has no hours).
        assert_eq!(days("more-work"), ("2026-10-05".into(), "2026-10-05".into()));
        assert_eq!(days("report"), ("2026-10-05".into(), "2026-10-12".into()));
        // Admin in admin hours only: an hour on Monday, the rest the next Monday.
        assert_eq!(days("tax"), ("2026-10-05".into(), "2026-10-12".into()));
        // Leisure has no hours: it waits for none, and takes nothing from work.
        assert_eq!(days("friends"), ("2026-10-05".into(), "2026-10-05".into()));
        assert!(settings.rest_days()[5], "Saturday has no hours");
    }

    #[test]
    fn admin_without_hours_goes_to_work_hours() {
        let windows = vec![window("monday", "09:00", "10:00", "work")];
        let settings = Settings { pause: 0, ..Settings::of_hours(&windows, TaskAreas::usual()) };
        let tax = Task { categories: vec!["admin".into()], ..task("tax", 30) };
        let friends = Task { categories: vec!["friends".into()], ..task("friends", 30) };
        let made = plan(&[tax, friends], day("2026-10-05"), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.items["tax"].start, Some(day("2026-10-05")));
        // Leisure has no hours: it waits for none, and takes nothing from work.
        assert_eq!(made.items["friends"].start, Some(day("2026-10-05")));
        assert!(settings.rest_days()[1], "Tuesday has no hours");
    }

    #[test]
    fn meals_and_naps_are_kept_free() {
        let windows: Vec<AdminWindow> = ["monday"].iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: None }).collect();
        let plain = Settings::of_hours(&windows, TaskAreas::usual());
        assert_eq!(plain.week[0].total(), 8 * 60);
        // Lunch kept from 12:10 (getting it ready) to 13:00, a nap from 14:00 to 14:35: 85 minutes fewer.
        let needs = crate::needs::Needs { meals_on: true, naps_on: true, ..crate::needs::Needs::default() };
        let kept = Settings::of_hours(&windows, TaskAreas::usual()).with_needs(&needs, BTreeMap::new());
        assert_eq!(kept.week[0].total(), 8 * 60 - 50 - 35);
        // Today, lunch moved a quarter of an hour later: the same minutes kept, later.
        let now: Zoned = "2026-10-05T08:00[Europe/Paris]".parse().unwrap();
        let moved = Settings::of_hours(&windows, TaskAreas::usual()).with_needs(&needs, BTreeMap::from([("meal:1".to_string(), 15)])).with_events(&now, &[]);
        assert_eq!(moved.room_on(now.date()).total(), 8 * 60 - 50 - 35);
        let lunch = moved.kept_on(now.date(), now.time_zone(), now.date()).into_iter().find(|(from, _)| *from > now.timestamp().as_second() + 3 * 3600).unwrap();
        assert_eq!(jiff::Timestamp::from_second(lunch.0).unwrap().to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string(), "12:25");
    }

    #[test]
    fn a_day_s_own_meals_and_naps_in_the_plan() {
        // Work 9:00–17:00 on Tuesdays and Wednesdays; lunch kept 12:10–13:00, a nap 14:00–14:35.
        let windows: Vec<AdminWindow> = ["tuesday", "wednesday"].iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: None }).collect();
        let needs = crate::needs::Needs { meals_on: true, naps_on: true, ..crate::needs::Needs::default() };
        let now: Zoned = "2026-10-05T08:00[Europe/Paris]".parse().unwrap();
        let (tuesday, wednesday): (Date, Date) = ("2026-10-06".parse().unwrap(), "2026-10-07".parse().unwrap());
        // That Wednesday only: no nap, and an hour for lunch.
        let mut days = crate::needs::Days::default();
        days.change(wednesday, "nap:0", |b| b.off = true);
        days.change(wednesday, "meal:1", |b| b.minutes = Some(60));
        let settings = Settings::of_hours(&windows, TaskAreas::usual()).with_needs(&needs, BTreeMap::new()).with_days(days).with_events(&now, &[]);
        assert_eq!(settings.room_on(tuesday).total(), 8 * 60 - 50 - 35, "Tuesday as usual");
        assert_eq!(settings.room_on(wednesday).total(), 8 * 60 - 80, "the nap's time given back, lunch half an hour longer");
        let next: Date = "2026-10-14".parse().unwrap();
        assert_eq!(settings.room_on(next).total(), 8 * 60 - 50 - 35, "the Wednesday after as usual");
    }

    #[test]
    fn events_and_pauses_take_room() {
        // Monday 5 October, 8:00: work 9–12 (180 min), a meeting 10–11 with a pause before and after.
        let windows = vec![window("monday", "09:00", "12:00", "work")];
        let zone = TimeZone::get("Europe/Paris").unwrap();
        let now = "2026-10-05T08:00".parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap();
        let at = |time: &str| format!("2026-10-05T{time}").parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let meeting = Occurrence { start: at("10:00"), end: at("11:00"), ..Occurrence::default() };
        let settings = Settings::of_hours(&windows, TaskAreas::usual()).with_events(&now, &[meeting]);
        assert_eq!(settings.room_on(now.date()).total(), 180 - 70);
        // Two 50-minute steps fit with their pauses (55 + 55 = 110); a third goes to the next Monday.
        let tasks = vec![task("a", 50), task("b", 50), task("c", 50)];
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        let on_monday = ["a", "b", "c"].iter().filter(|u| made.items[**u].start == Some(now.date())).count();
        assert_eq!(on_monday, 2);
        // Later today, what passed is gone too.
        let later = now.with().hour(11).minute(30).build().unwrap();
        let settings = Settings::of_hours(&windows, TaskAreas::usual()).with_events(&later, &[]);
        assert_eq!(settings.room_on(later.date()).total(), 30);
    }

    #[test]
    fn a_step_that_fills_its_window_is_not_cut() {
        // Tuesday 14:00–15:00 and Wednesday 18:00–18:30 for your admin: an hour's step and a half hour's, each whole in its window.
        let windows = vec![window("tuesday", "14:00", "15:00", "admin"), window("wednesday", "18:00", "18:30", "admin")];
        let settings = Settings::of_hours(&windows, TaskAreas::usual());
        let form = Task { due: "2026-10-06".into(), ..task("form", 60) };
        let made = plan(&[form, task("letter", 30)], day("2026-10-05"), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!((made.items["form"].start, made.items["form"].finish, made.items["form"].on_start), (Some(day("2026-10-06")), Some(day("2026-10-06")), 60));
        assert_eq!((made.items["letter"].start, made.items["letter"].on_start), (Some(day("2026-10-07")), 30));
    }

    #[test]
    fn calls_to_offices_in_working_hours() {
        // Admin hours in the evening only: a call to an office still goes in working hours (docs/areas.md).
        let windows = vec![window("monday", "09:00", "12:00", "work"), window("monday", "18:00", "19:00", "admin")];
        let settings = Settings::of_hours(&windows, TaskAreas::usual());
        let call = Task { office_hours: true, ..task("call", 30) };
        assert_eq!(settings.usable(&call), Area::MIXED);
        assert_eq!(settings.usable(&task("form", 30)), Area::ADMIN);
        // A date asked, and a task with no hours for it (leisure here): the date comes first, not the one without hours.
        let deadline = Task { due: "2026-10-12".into(), ..task("deadline", 30) };
        let friends = Task { due: "2026-10-20".into(), categories: vec!["friends".into()], ..task("friends", 30) };
        let made = plan(&[friends, deadline], day("2026-10-05"), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.items["friends"].latest_start, Some(day("2026-10-20")));
        assert_eq!(made.ready.first().map(String::as_str), Some("deadline"));
    }

    #[test]
    fn events_of_several_days_and_odd_files() {
        // A trip from Monday 8:00 to Wednesday 20:00 takes Tuesday's hours too.
        let windows = vec![window("monday", "09:00", "17:00", "work"), window("tuesday", "09:00", "17:00", "work"), window("wednesday", "09:00", "17:00", "work")];
        let zone = TimeZone::get("Europe/Paris").unwrap();
        let now = "2026-10-04T12:00".parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap();
        let at = |text: &str| text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let trip = Occurrence { start: at("2026-10-05T08:00"), end: at("2026-10-07T20:00"), ..Occurrence::default() };
        let settings = Settings::of_hours(&windows, TaskAreas::usual()).with_events(&now, &[trip]);
        assert_eq!(settings.room_on(day("2026-10-06")).total(), 0);
        // A gap and an estimate too long for any date, from someone else's file: planned, no crash.
        let mut sent = task("sent", 15);
        sent.status = Status::Completed;
        sent.completed = Some(now.timestamp().as_second());
        let answer = Task { relations: vec![Relation { kind: "DEPENDS-ON".into(), uid: "sent".into(), gap: i64::MAX / 2 }], ..task("answer", 15) };
        let huge = Task { due: "2026-10-30".into(), ..task("huge", u32::MAX) };
        let made = plan(&[sent, answer, huge], now.date(), &Settings::flat([1, 1, 1, 1, 1, 0, 0]), &BTreeMap::new(), &BTreeSet::new());
        assert!(made.items["answer"].start.is_some() && made.items["huge"].latest_start.is_some());
    }

    #[test]
    fn overlapping_hours_open_to_both() {
        // Saturday work 10–18 with admin hours 14–16 inside.
        let windows = vec![window("saturday", "10:00", "18:00", "work"), window("saturday", "14:00", "16:00", "admin")];
        let settings = Settings::of_hours(&windows, TaskAreas::usual());
        let saturday = &settings.week[5];
        assert_eq!(saturday.for_kinds(Area::WORK), 480);
        assert_eq!(saturday.for_kinds(Area::ADMIN), 120);
        assert_eq!(saturday.total(), 480, "the overlap counted once");
        assert_eq!(saturday.for_kinds(Area::LEISURE), 0, "leisure has no hours");
    }
}
