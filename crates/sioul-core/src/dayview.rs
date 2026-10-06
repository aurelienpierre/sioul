// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The day, seen (docs/tasks.md, "The day"): today's events at their times,
//! and the steps the plan gives today laid into your hours, each into hours
//! of its own kind (work, admin), a pause between them and around
//! the events; a line where now is. A step up to an hour is never cut; a
//! longer one goes on after a break; one with margins never (going there
//! twice is not the same step). Visual supports cut transition time
//! (Dettmer et al. 2000); the current step is shown inside the whole day,
//! with what comes next (Brili, Tiimo). A layout to look at, never a schedule
//! to keep: nothing is written into the tasks, and a step that runs over
//! only moves the next ones.
//!
//! What a day asks is spread (docs/capacity.md): never two heavy steps in a
//! row, a light step or a break of a quarter of an hour after a heavy one,
//! two steps weighing on the same cost apart when another can come between
//! (criteria 29–30). A step given a time by hand keeps it. Two slots of time
//! for you are kept, silent: one after the day's costliest block, one in the
//! evening (G11, G15, G18). Free time is kept after the last step for steps
//! running long (`plan::Plan::slack`).

use crate::agenda::Occurrence;
use crate::areas::Area;
use crate::plan::{Plan, SHORTEST, Settings, WHOLE};
use crate::tasks::Task;
use jiff::Zoned;
use serde::Serialize;
use std::collections::BTreeMap;

/// One block of the day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Block {
    /// Unix seconds.
    pub start: i64,
    pub end: i64,
    /// "event", "task", "meal", "nap", "margin" (getting there and back, getting
    /// ready), "done" (a task done today, kept in view).
    pub kind: &'static str,
    pub title: String,
    /// A task's UID, an event's file.
    pub key: String,
    /// A task "heavy", "light" or "rest"; "" otherwise.
    pub energy: String,
    pub location: String,
    /// Side by side with what overlaps it (two events at once): its column among `columns`.
    pub column: u32,
    pub columns: u32,
    /// A long step cut by a break: its part, from 1; 0 when whole.
    pub part: u32,
    /// A step given this time by hand: laid there, whatever else is.
    pub pinned: bool,
    /// A line under the title: a gain slot's suggestion; "" otherwise.
    pub note: String,
    /// An event (or its margins) whose calendar cannot be written here; false for anything else.
    pub read_only: bool,
    /// An event (or its margins) that comes back: one occurrence of it; false for anything else.
    pub recurring: bool,
}

/// A stretch of the day's hours and what it is for: "work", "admin",
/// "leisure", several joined by "+" where hours overlap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stretch {
    pub start: i64,
    pub end: i64,
    pub kind: String,
}

/// Places spans side by side where they overlap, as (column, columns): each
/// takes the first column free at its start, and every span of a group of
/// overlapping ones learns how many columns the group needs. `spans` are
/// (start, end), in any unit, an end no earlier than its start.
pub fn side_by_side(spans: &[(i64, i64)]) -> Vec<(u32, u32)> {
    let mut out = vec![(0u32, 1u32); spans.len()];
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|&i| (spans[i].0, -spans[i].1));
    let mut group: Vec<usize> = Vec::new();
    let mut ends: Vec<i64> = Vec::new();
    let mut group_end = i64::MIN;
    let close = |out: &mut Vec<(u32, u32)>, group: &mut Vec<usize>, ends: &mut Vec<i64>| {
        let columns = u32::try_from(ends.len()).unwrap_or(1).max(1);
        for &i in group.iter() {
            out[i].1 = columns;
        }
        group.clear();
        ends.clear();
    };
    for i in order {
        let (from, to) = spans[i];
        if from >= group_end {
            close(&mut out, &mut group, &mut ends);
            group_end = i64::MIN;
        }
        let column = match ends.iter().position(|&end| end <= from) {
            Some(free) => {
                ends[free] = to;
                free
            }
            None => {
                ends.push(to);
                ends.len() - 1
            }
        };
        out[i].0 = u32::try_from(column).unwrap_or(0);
        group.push(i);
        group_end = group_end.max(to);
    }
    close(&mut out, &mut group, &mut ends);
    out
}

/// The day: from its first hour to its last, its hours, its blocks, and now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DayView {
    pub from: i64,
    pub to: i64,
    pub now: i64,
    /// Today's hours, by what they are for.
    pub hours: Vec<Stretch>,
    pub blocks: Vec<Block>,
    /// Events of the whole day, said above.
    pub all_day: Vec<String>,
    /// Steps of today that do not fit before the day ends: they keep their place in the plan.
    pub more: usize,
    /// Why today holds what it holds, in at most two lines (`capacity::reasons`); set by the caller.
    pub said: Vec<String>,
}

/// After a heavy step, this long before anything but a light one (criterion 29: more than 10 min).
pub const BREAK_AFTER_HEAVY: i64 = 15;
/// How far ahead the day looks for a step that keeps two heavy ones, or two
/// of the same cost, apart. A guess.
pub const LOOK_AHEAD: usize = 3;
/// A gain slot: this long, a quarter of an hour when that is all a gap holds (G11).
pub const GAIN_LEAST: i64 = 15;
/// The free time kept for steps running long is shown from this long.
pub const SLACK_SHOWN: i64 = 10;

/// A step of today, as the day orders it.
#[derive(Debug, Clone)]
struct Step<'a> {
    task: &'a Task,
    planned: &'a crate::plan::Planned,
    level: crate::demands::Level,
    dominant: Option<usize>,
}

/// Today's steps in the plan's order, moved only to keep two heavy ones, or
/// two weighing on the same cost, apart, when one of the next few can come
/// between; never before what it waits for.
fn spread_out<'a>(mut rest: Vec<Step<'a>>) -> Vec<Step<'a>> {
    use crate::demands::Level;
    let mut out: Vec<Step<'a>> = Vec::with_capacity(rest.len());
    while !rest.is_empty() {
        let ahead = rest.len().min(LOOK_AHEAD);
        let ready = |s: &Step, rest: &[Step], out: &[Step]| s.planned.waits_for.iter().all(|w| out.iter().any(|o| &o.task.uid == w) || !rest.iter().any(|r| &r.task.uid == w));
        let first = (0..ahead).find(|&k| ready(&rest[k], &rest, &out)).unwrap_or(0);
        let pick = match out.last() {
            Some(prev) => {
                let heavy_twice = |s: &Step| prev.level == Level::Heavy && s.level == Level::Heavy;
                let same_cost = |s: &Step| prev.dominant.is_some() && s.dominant == prev.dominant;
                if heavy_twice(&rest[first]) || same_cost(&rest[first]) {
                    (0..ahead)
                        .find(|&k| ready(&rest[k], &rest, &out) && !heavy_twice(&rest[k]) && !same_cost(&rest[k]))
                        .or_else(|| (0..ahead).find(|&k| ready(&rest[k], &rest, &out) && !heavy_twice(&rest[k])))
                        .unwrap_or(first)
                } else {
                    first
                }
            }
            None => first,
        };
        out.push(rest.remove(pick));
    }
    out
}

/// The first stretch of `free` from `from` on, `least` long at least, `most` at most.
fn first_gap(free: &[(i64, i64)], from: i64, least: i64, most: i64) -> Option<(i64, i64)> {
    free.iter().map(|&(a, b)| (rounded(a.max(from)), b)).find(|(a, b)| b - a >= least).map(|(a, b)| (a, b.min(a + most)))
}

/// The next five minutes: blocks start on readable times.
fn rounded(seconds: i64) -> i64 {
    (seconds + 299).div_euclid(300) * 300
}

/// Today laid out: `events` of today, and the plan's steps for today in its
/// order, from now into the hours of their kind (`settings`), around the
/// events and what is laid already.
pub fn day(now: &Zoned, events: &[Occurrence], plan: &Plan, tasks: &[Task], settings: &Settings) -> DayView {
    let zone = now.time_zone().clone();
    let today = now.date();
    let stamp = now.timestamp().as_second();
    let midnight = today.to_zoned(zone.clone()).map_or(stamp, |z| z.timestamp().as_second());
    // The next midnight on the clock: the day of a change of hour lasts 23 or 25 hours.
    let next_midnight = today.tomorrow().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map_or(midnight + 86_400, |z| z.timestamp().as_second());
    let at = |hour: i8| jiff::civil::Time::new(hour, 0, 0, 0).ok().and_then(|t| today.to_datetime(t).to_zoned(zone.clone()).ok()).map_or(midnight + i64::from(hour) * 3600, |z| z.timestamp().as_second());
    // The day's hours, the end of work moved by free time included (light steps only there).
    let hours = settings.hours_on(today, &zone);
    let moved = settings.extension_on(today, &zone);
    let (opening, closing) = match (hours.first(), hours.last()) {
        (Some(first), Some(last)) => (first.0, last.1),
        _ => (at(9), at(17)),
    };
    // Today, and the end of hours that run past midnight: what lasts longer is cut there.
    let day_end = next_midnight.max(closing);
    let mut view = DayView { now: stamp, hours: hours.iter().map(|&(start, end, open)| Stretch { start, end, kind: open.id() }).collect(), ..DayView::default() };
    // A task's time block is the task's time, laid as the task (`blocks`): never an event of its own here.
    let own: Vec<Occurrence> = events.iter().filter(|e| e.task.is_empty() || !plan.items.contains_key(&e.task)).cloned().collect();
    let events = own.as_slice();
    // A task pinned to a block today: laid there, its margins around it, whatever else is.
    let block_today = |uid: &str| plan.pins.get(uid).filter(|pin| pin.end > midnight && pin.start < next_midnight);
    let mut timed: Vec<&Occurrence> = Vec::new();
    for event in events.iter().filter(|e| !e.cancelled && e.end > midnight && e.start < next_midnight) {
        if event.all_day {
            view.all_day.push(event.summary.clone());
        } else {
            timed.push(event);
        }
    }
    timed.sort_by_key(|e| e.start);
    for event in &timed {
        // An event from yesterday evening, or of several days, shows its part of today.
        let (start, end) = (event.start.max(midnight), event.end.max(event.start + 15 * 60).min(day_end));
        view.blocks.push(Block { start, end, kind: "event", title: event.summary.clone(), key: event.key.clone(), energy: String::new(), location: event.location.clone(), column: 0, columns: 1, part: 0, pinned: false, note: String::new(), read_only: event.read_only, recurring: event.recurring });
        // Its margins, shown apart: getting there and back is not the event, nor a pause.
        let margin = |from: i64, to: i64| Block { start: from.max(midnight), end: to.min(day_end), kind: "margin", title: event.summary.clone(), key: event.key.clone(), energy: String::new(), location: String::new(), column: 0, columns: 1, part: 0, pinned: false, note: String::new(), read_only: event.read_only, recurring: event.recurring };
        for block in [margin(start - i64::from(event.margins.before) * 60, start), margin(end, end + i64::from(event.margins.after) * 60)] {
            if block.end > block.start {
                view.blocks.push(block);
            }
        }
    }
    // What was done today stays in view, where it ended: its length, a quarter of an hour at least, an hour at most.
    for task in tasks.iter().filter(|t| t.status == crate::tasks::Status::Completed) {
        let Some(done) = task.completed.filter(|&at| at >= midnight && at < next_midnight) else { continue };
        let length = i64::from(task.estimate.clamp(15, 60)) * 60;
        // Done in the block that held its work: there, the record of when it was done.
        let (start, end) = match settings.records.get(&task.uid) {
            Some(&(from, to)) => (from.max(midnight), to.min(day_end).max(from.max(midnight) + 15 * 60)),
            None => ((done - length).max(midnight), done.max(midnight + 15 * 60)),
        };
        view.blocks.push(Block { start, end, kind: "done", title: task.title.clone(), key: task.uid.clone(), energy: String::new(), location: String::new(), column: 0, columns: 1, part: 0, pinned: false, note: String::new(), read_only: false, recurring: false });
    }
    // Meals and naps, kept free (`needs`), as today has them (its own changes):
    // shown, a step never laid over them. The night is kept free too, but not
    // drawn: the day would run from midnight.
    for kept in settings.needs.kept_with(today, &zone, &settings.needs_days, &|key: &str| settings.shifts.get(key).copied().unwrap_or(0)).into_iter().filter(|k| k.kind != "sleep") {
        let (start, end) = (kept.start.max(midnight), kept.end.min(day_end));
        if end > start {
            view.blocks.push(Block { start, end, kind: kept.kind, title: kept.name.clone(), key: kept.key.clone(), energy: String::new(), location: String::new(), column: 0, columns: 1, part: 0, pinned: false, note: String::new(), read_only: false, recurring: false });
        }
    }
    // Held: the events with a pause around each, what is kept free, what passed, and each step laid, with its pause.
    let pause = i64::from(settings.pause) * 60;
    let capacity = &settings.capacity;
    let mut busy: Vec<(i64, i64)> = crate::plan::event_spans(events, settings.pause);
    busy.extend(settings.kept_on(today, &zone, today));
    busy.push((i64::MIN, rounded(stamp)));
    let start_at = rounded(stamp);
    // Today's steps, in the plan's order.
    let steps: Vec<Step> = plan
        .order
        .iter()
        .filter_map(|uid| Some((tasks.iter().find(|t| &t.uid == uid)?, plan.items.get(uid)?)))
        .filter(|(task, planned)| (block_today(&task.uid).is_some() || (!planned.optional && planned.open_steps == 0)) && planned.start == Some(today) && planned.on_start > 0)
        .map(|(task, planned)| {
            let rates = capacity.rates_of(task);
            Step { task, planned, level: rates.level(), dominant: rates.dominant() }
        })
        .collect();
    let block_of = |task: &Task, level: crate::demands::Level, start: i64, end: i64, kind: &'static str, part: u32, pinned: bool| Block {
        start,
        end,
        kind,
        title: task.title.clone(),
        key: task.uid.clone(),
        energy: level.energy().to_string(),
        location: task.location.clone(),
        column: 0,
        columns: 1,
        part,
        pinned,
        note: String::new(),
        read_only: false,
        recurring: false,
    };
    // A step laid: its margins at both ends, shown apart (getting there and back is not the step, nor a pause).
    let lay = |view: &mut DayView, step: &Step, from: i64, to: i64, part: u32, pinned: bool| {
        let (before, after) = (i64::from(step.task.margins.before) * 60, i64::from(step.task.margins.after) * 60);
        let (inner_from, inner_to) = if to - from > before + after { (from + before, to - after) } else { (from, to) };
        if inner_from > from {
            view.blocks.push(block_of(step.task, step.level, from, inner_from, "margin", part, pinned));
        }
        view.blocks.push(block_of(step.task, step.level, inner_from, inner_to, "task", part, pinned));
        if to > inner_to {
            view.blocks.push(block_of(step.task, step.level, inner_to, to, "margin", part, pinned));
        }
    };
    // The costliest block of the day (task or event, by what it weighs): the gain slot comes after it.
    let gain_on = capacity.gain_slot > 0;
    let gain_length = i64::from(capacity.gain_slot) * 60;
    let event_weight = |e: &Occurrence| crate::capacity::Rates::of_event(e).load(u32::try_from((e.end - e.start) / 60).unwrap_or(0))[crate::capacity::TOTAL];
    let step_weight = |s: &Step| capacity.rates_of(s.task).load(s.planned.on_start)[crate::capacity::TOTAL];
    let costliest_event = timed.iter().filter(|e| e.end > stamp).map(|e| (event_weight(e), e)).fold(None, |best: Option<(f32, &&Occurrence)>, (w, e)| if best.is_some_and(|(b, _)| b >= w) { best } else { Some((w, e)) });
    let costliest_step = steps.iter().map(|s| (step_weight(s), s.task.uid.as_str())).fold(None, |best: Option<(f32, &str)>, (w, u)| if best.is_some_and(|(b, _)| b >= w) { best } else { Some((w, u)) });
    let after_step = match (costliest_event, costliest_step) {
        (Some((we, _)), Some((ws, uid))) if ws > we => Some(uid),
        (None, Some((_, uid))) => Some(uid),
        _ => None,
    };
    let mut gains: Vec<(i64, i64, &'static str)> = Vec::new();
    // Free time of the day, whatever its hours, from `from` on.
    let anywhere = |busy: &[(i64, i64)]| -> Vec<(i64, i64)> { crate::plan::less(&[(midnight, day_end, Area::ALL)], busy).into_iter().map(|(a, b, _)| (a, b)).collect() };
    if gain_on
        && after_step.is_none()
        && let Some((_, event)) = costliest_event
    {
        let end = event.end + i64::from(event.margins.after) * 60 + pause;
        if let Some((from, to)) = first_gap(&anywhere(&busy), end, GAIN_LEAST * 60, gain_length).filter(|(from, _)| *from < end + 2 * 3600) {
            gains.push((from, to, "gain:after"));
            busy.push((from, to + pause));
        }
    }
    let mut laid: BTreeMap<&str, i64> = BTreeMap::new();
    let mut heavy_laid: Vec<(i64, i64)> = Vec::new();
    // Pinned to a block today, or given a time today by a drag before blocks (`Task::at`): laid there first,
    // its margins before and after, whatever else is.
    let (pinned, others): (Vec<Step>, Vec<Step>) = steps.into_iter().partition(|s| block_today(&s.task.uid).is_some() || s.task.at_on(today, &zone).is_some());
    for step in &pinned {
        let (from, to) = match block_today(&step.task.uid) {
            // Its block, as the calendar has it: under way, it shows from its start.
            Some(pin) => (pin.span().0.max(midnight), pin.span().1.min(day_end)),
            None => {
                let Some(at) = step.task.at_on(today, &zone) else { continue };
                let length = i64::from(step.planned.on_start.max(step.planned.laid)) * 60;
                let (before, after) = (i64::from(step.task.margins.before) * 60, i64::from(step.task.margins.after) * 60);
                // The step's own start is the time given; one already past is laid from now.
                let inner = at.max(start_at + before);
                (inner - before, inner + (length - before - after).max(5 * 60) + after)
            }
        };
        lay(&mut view, step, from, to, 0, true);
        busy.push((from - if block_today(&step.task.uid).is_some() { pause } else { 0 }, to + pause));
        if step.level == crate::demands::Level::Heavy {
            heavy_laid.push((from, to));
        }
        laid.insert(step.task.uid.as_str(), to);
    }
    for step in spread_out(others) {
        let (task, planned) = (step.task, step.planned);
        // What it waits for comes first: when that is not laid today, neither is it.
        if planned.waits_for.iter().any(|w| !laid.contains_key(w.as_str())) {
            view.more += 1;
            continue;
        }
        // The hours open to it, one run where they follow each other.
        let kinds = settings.usable(task);
        let mut runs: Vec<(i64, i64, Area)> = Vec::new();
        for &(from, to, _) in hours.iter().filter(|h| h.2.meets(kinds)) {
            match runs.last_mut() {
                Some(last) if last.1 == from => last.1 = to,
                _ => runs.push((from, to, Area::ALL)),
            }
        }
        // A call to an office only while it is open: its own hours, else offices' usual ones.
        if task.office_hours {
            let own = crate::window::parse_ranges(&task.office_times);
            let open: Vec<(i64, i64)> = crate::window::spans_on(if own.is_empty() { &settings.office_hours } else { &own }, today, &zone).iter().map(|(o, c)| (o.timestamp().as_second(), c.timestamp().as_second())).collect();
            runs = runs.iter().flat_map(|&(from, to, area)| open.iter().filter(move |&&(o, c)| from.max(o) < to.min(c)).map(move |&(o, c)| (from.max(o), to.min(c), area))).collect();
        }
        // The end of work moved by free time: light steps only, and those there first (docs/pauses.md).
        let light = matches!(step.level, crate::demands::Level::Light | crate::demands::Level::Rest);
        if !light && !moved.is_empty() {
            runs = crate::plan::less(&runs, &moved);
        }
        if runs.is_empty() {
            view.more += 1;
            continue;
        }
        // After what it waits for, laid today.
        let after = planned.waits_for.iter().filter_map(|w| laid.get(w.as_str())).max().map_or(i64::MIN, |end| end + pause);
        // A heavy step is followed by a light one or a quarter of an hour's break, and two heavy ones never touch.
        let mut held = busy.clone();
        if step.level != crate::demands::Level::Light && step.level != crate::demands::Level::Rest {
            held.extend(heavy_laid.iter().map(|&(_, end)| (end, end + BREAK_AFTER_HEAVY * 60)));
        }
        if step.level == crate::demands::Level::Heavy {
            held.extend(heavy_laid.iter().map(|&(start, _)| (start - BREAK_AFTER_HEAVY * 60, start)));
        }
        let mut free: Vec<(i64, i64)> = crate::plan::less(&runs, &held).into_iter().map(|(from, to, _)| (rounded(from.max(after)), to)).filter(|(from, to)| to > from).collect();
        if light && !moved.is_empty() {
            let inside: Vec<(i64, i64)> = free.iter().flat_map(|&(a, b)| moved.iter().filter_map(move |&(m, n)| (a.max(m) < b.min(n)).then_some((a.max(m), b.min(n))))).collect();
            let outside = crate::plan::less(&free.iter().map(|&(a, b)| (a, b, Area::ALL)).collect::<Vec<_>>(), &moved).into_iter().map(|(a, b, _)| (a, b));
            free = inside.into_iter().chain(outside).collect();
        }
        let length = i64::from(planned.on_start) * 60;
        // A step up to an hour in one go, in the first gap that holds it; a longer
        // one (or one the plan already cut) from the first gap on, in parts of a
        // quarter of an hour at least.
        // Never cut when it has margins: going there twice is not the same step.
        let cut = task.margins.is_empty() && (planned.laid > WHOLE || planned.on_start < planned.laid.max(SHORTEST));
        let pieces: Vec<(i64, i64)> = if cut {
            let (mut pieces, mut owed, shortest) = (Vec::new(), length, i64::from(SHORTEST) * 60);
            for &(from, to) in &free {
                let mut take = owed.min(to - from);
                // As the plan cuts: never less than a quarter of an hour left for later
                // (40 minutes in gaps of 35 and 20 go 25 + 15, not 35 + 5); else not this gap.
                if take < owed && owed - take < shortest {
                    take = owed - shortest;
                }
                if take >= owed.min(shortest) {
                    pieces.push((from, from + take));
                    owed -= take;
                }
                if owed == 0 {
                    break;
                }
            }
            if owed > 0 { Vec::new() } else { pieces }
        } else {
            free.iter().find(|(from, to)| to - from >= length).map(|&(from, _)| vec![(from, from + length)]).unwrap_or_default()
        };
        if pieces.is_empty() {
            view.more += 1;
            continue;
        }
        let count = pieces.len();
        for (k, &(from, to)) in pieces.iter().enumerate() {
            lay(&mut view, &step, from, to, if count > 1 { k as u32 + 1 } else { 0 }, false);
            busy.push((from, to + pause));
            if step.level == crate::demands::Level::Heavy {
                heavy_laid.push((from, to));
            }
        }
        let end = pieces.last().map_or(0, |p| p.1);
        laid.insert(task.uid.as_str(), end);
        // Right after the day's costliest step: time for you, within its hours when they hold it.
        if gain_on && after_step == Some(task.uid.as_str()) {
            let in_hours: Vec<(i64, i64)> = crate::plan::less(&runs, &busy).into_iter().map(|(a, b, _)| (a, b)).collect();
            let near = |gap: &(i64, i64)| gap.0 < end + 2 * 3600;
            let slot = first_gap(&in_hours, end + pause, GAIN_LEAST * 60, gain_length).filter(near).or_else(|| first_gap(&anywhere(&busy), end + pause, GAIN_LEAST * 60, gain_length).filter(near));
            if let Some((from, to)) = slot {
                gains.push((from, to, "gain:after"));
                busy.push((from, to + pause));
            }
        }
    }
    // Free time kept for steps running long, after the last one laid, within the
    // day's hours: as the plan kept it for the steps it laid today (TE16).
    let last_step = view.blocks.iter().filter(|b| b.kind == "task" || (b.kind == "margin" && laid.contains_key(b.key.as_str()))).map(|b| b.end).max();
    if let (Some(last), Some(&slack)) = (last_step, plan.slack.get(&today)) {
        let wanted = i64::from(slack) * 60;
        let in_hours: Vec<(i64, i64)> = crate::plan::less(&hours, &busy).into_iter().map(|(a, b, _)| (a, b)).collect();
        if let Some((from, to)) = first_gap(&in_hours, last + pause, SLACK_SHOWN * 60, wanted).filter(|_| wanted >= SLACK_SHOWN * 60) {
            view.blocks.push(Block { start: from, end: to, kind: "slack", title: String::new(), key: "slack".into(), energy: String::new(), location: String::new(), column: 0, columns: 1, part: 0, pinned: false, note: String::new(), read_only: false, recurring: false });
            busy.push((from, to));
        }
    }
    // In the evening, once today's hours are over, before the night: time for you (G15).
    if gain_on {
        let evening = if hours.is_empty() { at(18) } else { closing };
        let night = settings.needs.kept_with(today, &zone, &settings.needs_days, &|key: &str| settings.shifts.get(key).copied().unwrap_or(0)).into_iter().filter(|k| k.kind == "sleep" && k.start > evening).map(|k| k.start).min().unwrap_or(at(23));
        if let Some((from, to)) = first_gap(&anywhere(&busy), evening.max(start_at), GAIN_LEAST * 60, gain_length).filter(|(_, to)| *to <= night) {
            gains.push((from, to, "gain:evening"));
        }
    }
    for (from, to, key) in gains {
        view.blocks.push(Block { start: from, end: to, kind: "gain", title: String::new(), key: key.into(), energy: String::new(), location: String::new(), column: 0, columns: 1, part: 0, pinned: false, note: String::new(), read_only: false, recurring: false });
    }
    view.blocks.sort_by_key(|b| b.start);
    // Two events at once: side by side, never one over the other.
    let spans: Vec<(i64, i64)> = view.blocks.iter().map(|b| (b.start, b.end)).collect();
    for (block, (column, columns)) in view.blocks.iter_mut().zip(side_by_side(&spans)) {
        block.column = column;
        block.columns = columns;
    }
    // From the first hour to the last, and now, within today.
    view.from = view.blocks.iter().map(|b| b.start).min().unwrap_or(opening).min(opening).min(stamp.max(midnight));
    view.to = view.blocks.iter().map(|b| b.end).max().unwrap_or(closing).max(closing).max(stamp.min(day_end));
    // Whole hours at each end, on the clock: in a zone half an hour off UTC too.
    view.from = whole_hour(view.from, &zone, false);
    view.to = whole_hour(view.to, &zone, true);
    view
}

/// `seconds` down, or up, to a whole hour as the clock in `zone` shows it.
fn whole_hour(seconds: i64, zone: &jiff::tz::TimeZone, up: bool) -> i64 {
    let offset = jiff::Timestamp::from_second(seconds).map_or(0, |t| i64::from(zone.to_offset(t).seconds()));
    let local = seconds + offset;
    let down = local - local.rem_euclid(3600);
    (if up && down < local { down + 3600 } else { down }) - offset
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::TaskAreas;
    use crate::plan::plan;
    use crate::window::AdminWindow;
    use jiff::tz::TimeZone;
    use std::collections::{BTreeMap, BTreeSet};

    fn at(text: &str) -> Zoned {
        text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(TimeZone::get("Europe/Paris").unwrap()).unwrap()
    }

    fn window(day: &str, start: &str, end: &str, kind: &str) -> AdminWindow {
        AdminWindow { day: day.into(), start: start.into(), end: Some(end.into()), minutes: 0, kind: Some(kind.into()) }
    }

    /// Minutes from midnight of each block, with its title.
    fn laid(view: &DayView) -> Vec<(&str, i64)> {
        let midnight = at("2026-10-05T00:00").timestamp().as_second();
        view.blocks.iter().map(|b| (b.title.as_str(), (b.start - midnight) / 60)).collect()
    }

    fn task(uid: &str, estimate: u32, category: &str) -> Task {
        Task { uid: uid.into(), title: uid.into(), estimate, categories: if category.is_empty() { vec![] } else { vec![category.into()] }, ..Task::default() }
    }

    fn laid_out(now: &Zoned, windows: &[AdminWindow], events: &[Occurrence], tasks: &[Task]) -> DayView {
        let settings = Settings::of_hours(windows, TaskAreas::usual()).with_events(now, events);
        let made = plan(tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        day(now, events, &made, tasks, &settings)
    }

    #[test]
    fn steps_go_around_events() {
        let now = at("2026-10-05T09:58");
        let hours = vec![window("monday", "09:00", "12:00", "work")];
        let tasks = vec![task("form", 30, ""), task("call", 45, ""), task("long", 120, "")];
        let call = at("2026-10-05T10:30").timestamp().as_second();
        let meeting = Occurrence { key: "m.ics".into(), uid: "m".into(), summary: "Meeting".into(), start: call, end: call + 1800, ..Occurrence::default() };
        let view = laid_out(&now, &hours, &[meeting], &tasks);
        // 10:00–10:25 before the meeting (a pause before it), 11:05–12:00 after it: the form after
        // the meeting; the call does not fit whole any more today; the long one in two parts, 25 + 15.
        assert_eq!(laid(&view), vec![("long", 600), ("Meeting", 630), ("form", 665), ("long", 700)], "{:?}", laid(&view));
        assert_eq!(view.blocks.iter().filter(|b| b.part > 0).count(), 2);
        assert_eq!(view.more, 0, "the call waits for another day in the plan");
        assert_eq!(view.from, at("2026-10-05T09:00").timestamp().as_second());
        assert_eq!(view.to, at("2026-10-05T12:00").timestamp().as_second());
        assert!(view.blocks.iter().all(|b| (b.column, b.columns) == (0, 1)), "nothing overlaps");
    }

    #[test]
    fn no_step_over_a_meal() {
        // Work 9:00–17:00 straight; lunch kept from 12:10 (getting it ready) to 13:00.
        let now = at("2026-10-05T11:30");
        let hours = vec![window("monday", "09:00", "17:00", "work")];
        let needs = crate::needs::Needs { meals_on: true, ..crate::needs::Needs::default() };
        let settings = Settings::of_hours(&hours, TaskAreas::usual()).with_needs(&needs, BTreeMap::new()).with_events(&now, &[]);
        let tasks = vec![task("short", 20, ""), task("long", 60, "")];
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        let view = day(&now, &[], &made, &tasks, &settings);
        // The short one before lunch; the long one after it, never across it.
        let steps: Vec<(&str, i64)> = laid(&view).into_iter().filter(|(t, _)| *t == "short" || *t == "long").collect();
        assert_eq!(steps, vec![("short", 690), ("long", 780)], "{:?}", laid(&view));
        let lunch = view.blocks.iter().find(|b| b.key == "meal:1").unwrap();
        assert_eq!((lunch.start, lunch.end), (at("2026-10-05T12:10").timestamp().as_second(), at("2026-10-05T13:00").timestamp().as_second()));
    }

    #[test]
    fn no_step_in_the_break() {
        let now = at("2026-10-05T11:30");
        let hours = vec![window("monday", "09:00", "12:00", "work"), window("monday", "14:00", "17:00", "work")];
        let view = laid_out(&now, &hours, &[], &[task("short", 20, ""), task("long", 60, "")]);
        // 11:30 the short one (to 11:50); the long one would run into lunch: at 14:00.
        assert_eq!(laid(&view), vec![("short", 690), ("long", 840)], "{:?}", laid(&view));
    }

    #[test]
    fn each_step_in_hours_of_its_kind() {
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "12:00", "work"), window("monday", "17:00", "18:30", "admin"), window("monday", "18:30", "21:00", "leisure")];
        let tasks = vec![task("tax", 30, "admin"), task("report", 60, "work"), task("friends", 60, "friends")];
        let view = laid_out(&now, &hours, &[], &tasks);
        // The tax form in admin hours, the report in work hours; leisure has no
        // hours (an older Sioul's free time is left aside): friends take none.
        let mut seen = laid(&view);
        seen.sort();
        assert_eq!(seen, vec![("report", 9 * 60), ("tax", 17 * 60)]);
        assert_eq!(view.hours.iter().map(|h| h.kind.as_str()).collect::<Vec<_>>(), vec!["work", "admin"]);
    }

    #[test]
    fn a_long_step_goes_on_after_lunch() {
        let now = at("2026-10-05T10:00");
        let hours = vec![window("monday", "09:00", "12:00", "work"), window("monday", "14:00", "17:00", "work")];
        let view = laid_out(&now, &hours, &[], &[task("thesis", 180, "work")]);
        assert_eq!(laid(&view), vec![("thesis", 600), ("thesis", 840)], "{:?}", laid(&view));
        assert_eq!(view.blocks.iter().map(|b| (b.end - b.start) / 60).sum::<i64>(), 180);
        assert_eq!(view.blocks.iter().map(|b| b.part).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn no_piece_under_a_quarter_of_an_hour() {
        // Work 9:00–10:20, a visit 9:40–9:55: gaps of 35 and 20 minutes, its pauses around it. The day's weather
        // leaves the thesis 40 minutes today: 25 + 15, never 35 + 5.
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "10:20", "work")];
        let visit = Occurrence { key: "v.ics".into(), summary: "Visit".into(), start: at("2026-10-05T09:40").timestamp().as_second(), end: at("2026-10-05T09:55").timestamp().as_second(), ..Occurrence::default() };
        let mut settings = Settings::of_hours(&hours, TaskAreas::usual()).with_events(&now, std::slice::from_ref(&visit));
        settings.today_percent = 73;
        let tasks = vec![task("thesis", 100, "work")];
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.items["thesis"].on_start, 40);
        let view = day(&now, &[visit], &made, &tasks, &settings);
        assert_eq!(laid(&view), vec![("thesis", 540), ("Visit", 580), ("thesis", 600)], "{:?}", laid(&view));
        let lengths: Vec<i64> = view.blocks.iter().filter(|b| b.kind == "task").map(|b| (b.end - b.start) / 60).collect();
        assert_eq!(lengths, vec![25, 15]);
    }

    #[test]
    fn calls_while_offices_open_and_waits_kept() {
        // Work 9–10, admin hours 17–19: the report fills the morning; the call is not laid at 17:00, offices are closed.
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "10:00", "work"), window("monday", "17:00", "19:00", "admin")];
        let report = Task { due: "2026-10-05".into(), ..task("report", 60, "work") };
        let call = Task { office_hours: true, ..task("call", 30, "") };
        let view = laid_out(&now, &hours, &[], &[report, call]);
        assert_eq!((laid(&view), view.more), (vec![("report", 540)], 1));
        // A step waiting for one that does not fit today is not laid before it.
        let hours = vec![window("monday", "09:00", "11:00", "work")];
        let visit = Occurrence { key: "v.ics".into(), summary: "Visit".into(), start: at("2026-10-05T09:55").timestamp().as_second(), end: at("2026-10-05T10:05").timestamp().as_second(), ..Occurrence::default() };
        let small = Task { relations: vec![crate::tasks::Relation { kind: "DEPENDS-ON".into(), uid: "big".into(), gap: 0 }], ..task("small", 15, "") };
        let view = laid_out(&now, &hours, &[visit], &[task("big", 60, ""), small]);
        assert_eq!((laid(&view), view.more), (vec![("Visit", 595)], 2));
    }

    #[test]
    fn days_of_23_and_25_hours_and_of_several() {
        // Sunday 25 October 2026 lasts 25 hours: an event at 23:30 is still today's.
        let late = Occurrence { key: "l.ics".into(), summary: "Late".into(), start: at("2026-10-25T23:30").timestamp().as_second(), end: at("2026-10-25T23:45").timestamp().as_second(), ..Occurrence::default() };
        assert_eq!(laid_out(&at("2026-10-25T10:00"), &[], &[late], &[]).blocks.len(), 1);
        // Sunday 29 March 2026 lasts 23: Monday's whole-day event is not today's.
        let monday = at("2026-03-30T00:00").timestamp().as_second();
        let holiday = Occurrence { summary: "Holiday".into(), all_day: true, start: monday, end: monday + 86_400, ..Occurrence::default() };
        assert!(laid_out(&at("2026-03-29T10:00"), &[], &[holiday], &[]).all_day.is_empty());
        // A conference over three days: today's part of it, the view within today.
        let conference = Occurrence { key: "c.ics".into(), summary: "Conference".into(), start: at("2026-10-05T09:00").timestamp().as_second(), end: at("2026-10-07T17:00").timestamp().as_second(), ..Occurrence::default() };
        let view = laid_out(&at("2026-10-06T08:00"), &[], &[conference], &[]);
        let midnight = at("2026-10-06T00:00").timestamp().as_second();
        assert_eq!((view.blocks[0].start, view.blocks[0].end, view.from, view.to), (midnight, midnight + 86_400, midnight, midnight + 86_400));
    }

    #[test]
    fn done_today_stays_in_view() {
        let now = at("2026-10-05T15:00");
        let done_at = at("2026-10-05T11:00").timestamp().as_second();
        let letter = Task { status: crate::tasks::Status::Completed, completed: Some(done_at), ..task("letter", 30, "") };
        let yesterday = Task { status: crate::tasks::Status::Completed, completed: Some(done_at - 86_400), ..task("old", 30, "") };
        let view = laid_out(&now, &[], &[], &[letter, yesterday]);
        let done: Vec<(&str, &str, i64)> = view.blocks.iter().map(|b| (b.kind, b.title.as_str(), (b.end - b.start) / 60)).collect();
        assert_eq!(done, vec![("done", "letter", 30)]);
    }

    #[test]
    fn margins_kept_free_and_shown() {
        // The dentist 10–11, half an hour to get there, 20 minutes back; posting a
        // parcel, 30 minutes and a quarter of an hour each way: not before, there is no room.
        let now = at("2026-10-05T09:00");
        let hours = vec![window("monday", "09:00", "13:00", "work")];
        let margins = |before, after| crate::demands::Margins { before, after };
        let dentist = Occurrence { key: "d.ics".into(), summary: "Dentist".into(), start: at("2026-10-05T10:00").timestamp().as_second(), end: at("2026-10-05T11:00").timestamp().as_second(), margins: margins(30, 20), ..Occurrence::default() };
        let parcel = Task { margins: margins(15, 15), ..task("parcel", 30, "") };
        let view = laid_out(&now, &hours, &[dentist], &[parcel]);
        let pause = i64::from(crate::plan::PAUSE);
        let kinds: Vec<(&str, &str, i64)> = view.blocks.iter().map(|b| (b.kind, b.title.as_str(), (b.start - at("2026-10-05T00:00").timestamp().as_second()) / 60)).collect();
        assert_eq!(
            kinds,
            vec![("margin", "Dentist", 570), ("event", "Dentist", 600), ("margin", "Dentist", 660), ("margin", "parcel", 680 + pause), ("task", "parcel", 695 + pause), ("margin", "parcel", 725 + pause)]
        );
    }

    fn rated(uid: &str, estimate: u32, anxiety: u8, cognitive: u8) -> Task {
        Task { demands: crate::demands::Demands { anxiety: Some(anxiety), cognitive: Some(cognitive), ..crate::demands::Demands::default() }, ..task(uid, estimate, "") }
    }

    fn steps_of(view: &DayView) -> Vec<(&str, i64, i64)> {
        let midnight = at("2026-10-05T00:00").timestamp().as_second();
        view.blocks.iter().filter(|b| b.kind == "task").map(|b| (b.title.as_str(), (b.start - midnight) / 60, (b.end - midnight) / 60)).collect()
    }

    #[test]
    fn never_two_heavy_steps_in_a_row() {
        // Work 9–13: heavy, heavy, light, usual in the plan's order (all equal, so by title).
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "13:00", "work")];
        let tasks = vec![rated("a-heavy", 30, 8, 2), rated("b-heavy", 30, 2, 8), rated("c-light", 30, 2, 1), rated("d-usual", 30, 5, 2)];
        let view = laid_out(&now, &hours, &[], &tasks);
        let order: Vec<&str> = steps_of(&view).iter().map(|s| s.0).collect();
        assert_eq!(order, vec!["a-heavy", "c-light", "b-heavy", "d-usual"], "{:?}", steps_of(&view));
        // After a heavy step, a usual one waits a quarter of an hour; a light one only the pause.
        let s = steps_of(&view);
        assert_eq!(s[1].1 - s[0].2, 5, "light after heavy: the pause");
        assert!(s[3].1 - s[2].2 >= BREAK_AFTER_HEAVY, "usual after heavy: a break: {s:?}");
        assert!(view.blocks.iter().filter(|b| b.kind == "task" && b.title.ends_with("heavy")).all(|b| b.energy == "heavy"));
    }

    #[test]
    fn only_heavy_steps_get_breaks_between() {
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "12:00", "work")];
        let view = laid_out(&now, &hours, &[], &[rated("one", 30, 8, 0), rated("two", 30, 9, 0)]);
        let s = steps_of(&view);
        assert_eq!(s.len(), 2);
        assert!(s[1].1 - s[0].2 >= BREAK_AFTER_HEAVY, "{s:?}");
    }

    #[test]
    fn kinds_of_cost_alternate() {
        // Two anxious calls and a cognitive form: the form between them.
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "12:00", "work")];
        let tasks = vec![rated("a-call", 20, 5, 1), rated("b-call", 20, 6, 1), rated("c-form", 20, 1, 5)];
        let view = laid_out(&now, &hours, &[], &tasks);
        let order: Vec<&str> = steps_of(&view).iter().map(|s| s.0).collect();
        assert_eq!(order, vec!["a-call", "c-form", "b-call"]);
    }

    #[test]
    fn a_task_with_margins_is_never_cut() {
        // Work 9–10 and 11–13; posting a parcel: 50 minutes and a quarter of an hour each way.
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "10:00", "work"), window("monday", "11:00", "13:00", "work")];
        let parcel = Task { margins: crate::demands::Margins { before: 15, after: 15 }, ..task("parcel", 50, "") };
        let settings = Settings::of_hours(&hours, TaskAreas::usual()).with_events(&now, &[]);
        let made = plan(std::slice::from_ref(&parcel), now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.items["parcel"].on_start, 80, "the plan lays it whole");
        let view = day(&now, &[], &made, &[parcel], &settings);
        let blocks: Vec<(&str, i64)> = view.blocks.iter().map(|b| (b.kind, (b.start - at("2026-10-05T00:00").timestamp().as_second()) / 60)).collect();
        // Not in the first hour (it would be cut): whole at 11:00, margins at both ends.
        assert_eq!(blocks, vec![("margin", 660), ("task", 675), ("margin", 725)]);
        // Longer than any day: a day of its own, still whole.
        let trip = Task { margins: crate::demands::Margins { before: 60, after: 60 }, ..task("trip", 240, "") };
        let made = plan(std::slice::from_ref(&trip), now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!((made.items["trip"].start, made.items["trip"].finish), (Some(now.date()), Some(now.date())));
    }

    #[test]
    fn a_step_given_a_time_keeps_it() {
        // Dragged to 15:00, outside the morning's hours, over nothing: laid there, the rest around it.
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "12:00", "work")];
        let mut held = task("held", 30, "");
        held.at = "2026-10-05T15:00".into();
        let tasks = vec![held, task("free", 30, "")];
        let view = laid_out(&now, &hours, &[], &tasks);
        let pinned = view.blocks.iter().find(|b| b.title == "held").unwrap();
        assert!(pinned.pinned);
        assert_eq!((pinned.start - at("2026-10-05T00:00").timestamp().as_second()) / 60, 900);
        assert!(view.blocks.iter().any(|b| b.title == "free" && !b.pinned));
        // Yesterday's time: nothing pinned, the plan places it again.
        let mut old = task("old", 30, "");
        old.at = "2026-10-04T15:00".into();
        let view = laid_out(&now, &hours, &[], &[old]);
        assert!(view.blocks.iter().all(|b| !b.pinned));
    }

    /// A task's time block, as the calendar has it (an event naming its task).
    fn block_of(task: &str, from: &str, to: &str) -> Occurrence {
        Occurrence { key: format!("{task}-block.ics"), uid: format!("{task}-block"), summary: task.into(), start: at(from).timestamp().as_second(), end: at(to).timestamp().as_second(), task: task.into(), ..Occurrence::default() }
    }

    /// The day as the window lays it: the blocks sorted out of the events, then the plan, then the day.
    fn with_blocks(now: &Zoned, hours: &[AdminWindow], events: &[Occurrence], tasks: &[Task]) -> DayView {
        let blocks = crate::blocks::Blocks::of(events, tasks, now.timestamp().as_second(), now.time_zone());
        let own = crate::blocks::without_blocks(events.to_vec(), tasks);
        let settings = Settings::of_hours(hours, TaskAreas::usual()).with_pins(blocks).with_events(now, &own);
        let made = plan(tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        // The events as read, blocks among them: the day sets them apart itself too.
        day(now, events, &made, tasks, &settings)
    }

    #[test]
    fn a_task_pinned_to_its_block_is_laid_there() {
        // Work 9–12; "bank" pinned 10:00–10:45 by its block; "free", an hour and a half, placed by the plan.
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "12:00", "work")];
        let tasks = vec![task("bank", 30, ""), task("free", 90, "")];
        let view = with_blocks(&now, &hours, &[block_of("bank", "2026-10-05T10:00", "2026-10-05T10:45")], &tasks);
        // The task in its block, for the block's 45 minutes, held; never an event of its own.
        assert!(view.blocks.iter().all(|b| b.kind != "event"), "{:?}", laid(&view));
        let bank: Vec<&Block> = view.blocks.iter().filter(|b| b.title == "bank").collect();
        assert_eq!(bank.len(), 1);
        assert_eq!((bank[0].kind, bank[0].pinned, bank[0].start, (bank[0].end - bank[0].start) / 60), ("task", true, at("2026-10-05T10:00").timestamp().as_second(), 45));
        // The rest of the day goes around it, a pause on each side: 9:00–9:55, then from 10:50.
        let free: Vec<(i64, i64)> = steps_of(&view).into_iter().filter(|s| s.0 == "free").map(|s| (s.1, s.2)).collect();
        assert_eq!(free, vec![(540, 595), (650, 685)]);
    }

    #[test]
    fn a_past_block_leaves_its_task_to_the_plan() {
        // 11:00: this morning's block, 10:00–10:45, is over and the task not done: laid again from now, nothing said.
        let now = at("2026-10-05T11:00");
        let hours = vec![window("monday", "09:00", "12:00", "work")];
        let tasks = vec![task("bank", 30, "")];
        let view = with_blocks(&now, &hours, &[block_of("bank", "2026-10-05T10:00", "2026-10-05T10:45")], &tasks);
        let shown: Vec<(&str, &str, i64, bool)> = view.blocks.iter().map(|b| (b.kind, b.title.as_str(), (b.start - at("2026-10-05T00:00").timestamp().as_second()) / 60, b.pinned)).collect();
        assert_eq!(shown, vec![("task", "bank", 660, false)]);
        // Done in its block: shown done there, the record of when the work was done.
        let done = Task { status: crate::tasks::Status::Completed, completed: Some(at("2026-10-05T10:40").timestamp().as_second()), ..task("bank", 30, "") };
        let view = with_blocks(&now, &hours, &[block_of("bank", "2026-10-05T10:00", "2026-10-05T10:45")], &[done]);
        let shown: Vec<(&str, i64, i64)> = view.blocks.iter().map(|b| (b.kind, (b.start - at("2026-10-05T00:00").timestamp().as_second()) / 60, (b.end - b.start) / 60)).collect();
        assert_eq!(shown, vec![("done", 600, 45)]);
    }

    #[test]
    fn gain_slots_after_the_costliest_block_and_in_the_evening() {
        // Work 9–13; a heavy step and two light ones; the night from 22:30.
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "13:00", "work")];
        let needs = crate::needs::Needs::default();
        let mut settings = Settings::of_hours(&hours, TaskAreas::usual()).with_needs(&needs, BTreeMap::new()).with_events(&now, &[]);
        settings.capacity.gain_slot = 30;
        let tasks = vec![rated("a-light", 20, 1, 1), rated("b-heavy", 60, 8, 2), rated("c-light", 20, 2, 1)];
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        let view = day(&now, &[], &made, &tasks, &settings);
        let midnight = at("2026-10-05T00:00").timestamp().as_second();
        let gains: Vec<(&str, i64, i64)> = view.blocks.iter().filter(|b| b.kind == "gain").map(|b| (b.key.as_str(), (b.start - midnight) / 60, (b.end - b.start) / 60)).collect();
        let heavy = view.blocks.iter().find(|b| b.title == "b-heavy").unwrap();
        assert_eq!(gains.len(), 2, "{gains:?}");
        assert_eq!(gains[0].0, "gain:after");
        assert_eq!((gains[0].1, gains[0].2), ((heavy.end - midnight) / 60 + 5, 30), "right after the heavy step, its pause between");
        assert_eq!((gains[1].0, gains[1].1, gains[1].2), ("gain:evening", 13 * 60, 30), "once the day's hours are over");
        // Off: none.
        settings.capacity.gain_slot = 0;
        let view = day(&now, &[], &made, &tasks, &settings);
        assert!(view.blocks.iter().all(|b| b.kind != "gain"));
    }

    #[test]
    fn free_time_kept_after_the_last_step() {
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "17:00", "work")];
        let mut settings = Settings::of_hours(&hours, TaskAreas::usual()).with_events(&now, &[]);
        settings.capacity.spread = Some(crate::capacity::Spread::of(0.5));
        let tasks: Vec<Task> = (0..5).map(|k| task(&format!("t{k}"), 30, "")).collect();
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        let slack = made.slack.get(&now.date()).copied().unwrap_or(0);
        assert!(slack > 0, "{:?}", made.slack);
        let view = day(&now, &[], &made, &tasks, &settings);
        let kept = view.blocks.iter().find(|b| b.kind == "slack").unwrap();
        let last = view.blocks.iter().filter(|b| b.kind == "task").map(|b| b.end).max().unwrap();
        assert_eq!((kept.start, (kept.end - kept.start) / 60), (last + 5 * 60, i64::from(slack)));
        // The same day, the same steps: the same free time.
        assert_eq!(plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new()).slack, made.slack);
    }

    /// What `capacity::gather` gives with no history yet: the prior's spread, widened for thin data.
    fn thin_data(settings: &mut Settings) {
        let ratios = crate::capacity::ratios(&[], at("2026-10-06T00:00").date());
        settings.capacity.spread = Some(crate::capacity::Spread::of(ratios.sigma));
        settings.capacity.today_sigma = ratios.today_sigma;
        settings.capacity.gain_slot = crate::capacity::GAIN_SLOT_MINUTES;
    }

    #[test]
    fn a_started_long_step_keeps_part_of_today() {
        // Tuesday 6 October, 14:38: work until 18:00. A sketch of about four hours, started, and a
        // call of 17 minutes; no history (the widest free time). The sketch is the next step: it
        // keeps part of today, and the free time kept stays within a third of the day's room.
        let now = at("2026-10-06T14:38");
        let hours = vec![window("tuesday", "09:00", "18:00", "work")];
        let mut settings = Settings::of_hours(&hours, TaskAreas::usual()).with_events(&now, &[]);
        thin_data(&mut settings);
        let sketch = Task { status: crate::tasks::Status::InProcess, ..task("Esquisser deux mises en page", 240, "") };
        let call = task("Appeler la caisse santé", 17, "");
        let tasks = vec![sketch, call];
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert_eq!(made.next.as_deref(), Some("Esquisser deux mises en page"));
        let started = &made.items["Esquisser deux mises en page"];
        assert_eq!(started.start, Some(now.date()), "{started:?}");
        assert!(started.on_start >= crate::plan::SHORTEST, "{started:?}");
        let room = settings.room_on(now.date()).total();
        let slack = made.slack.get(&now.date()).copied().unwrap_or(0);
        assert!(slack as f32 <= crate::capacity::SLACK_SHARE * room as f32, "{slack} of {room}");
        // Now and the day agree: the day lays the started step today, and its free time within the hours.
        let view = day(&now, &[], &made, &tasks, &settings);
        assert!(view.blocks.iter().any(|b| b.kind == "task" && b.title == "Esquisser deux mises en page"), "{:?}", view.blocks);
        let closing = at("2026-10-06T18:00").timestamp().as_second();
        for kept in view.blocks.iter().filter(|b| b.kind == "slack") {
            assert!(kept.end <= closing && (kept.end - kept.start) / 60 <= i64::from(slack), "{kept:?}");
        }
    }

    #[test]
    fn a_day_of_short_steps_keeps_some_free_time() {
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "17:00", "work")];
        let mut settings = Settings::of_hours(&hours, TaskAreas::usual()).with_events(&now, &[]);
        thin_data(&mut settings);
        let tasks: Vec<Task> = (0..4).map(|k| task(&format!("short-{k}"), 15, "")).collect();
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        assert!(tasks.iter().all(|t| made.items[&t.uid].start == Some(now.date())));
        let slack = made.slack.get(&now.date()).copied().unwrap_or(0);
        assert!(slack > 0 && slack as f32 <= crate::capacity::SLACK_SHARE * 480.0, "{slack}");
        let view = day(&now, &[], &made, &tasks, &settings);
        let kept = view.blocks.iter().find(|b| b.kind == "slack").unwrap();
        let last = view.blocks.iter().filter(|b| b.kind == "task").map(|b| b.end).max().unwrap();
        assert!(kept.start >= last && kept.end <= at("2026-10-05T17:00").timestamp().as_second());
        assert_eq!((kept.end - kept.start) / 60, i64::from(slack));
    }

    #[test]
    fn free_time_never_empties_a_day() {
        // Twenty half hours, no history: the day keeps at most a third free, and still holds most of the rest.
        let now = at("2026-10-05T08:00");
        let hours = vec![window("monday", "09:00", "17:00", "work")];
        let mut settings = Settings::of_hours(&hours, TaskAreas::usual()).with_events(&now, &[]);
        thin_data(&mut settings);
        let tasks: Vec<Task> = (0..20).map(|k| task(&format!("t{k:02}"), 30, "")).collect();
        let made = plan(&tasks, now.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        let laid: u32 = made.days[&now.date()].iter().map(|(_, m)| m).sum();
        let slack = made.slack.get(&now.date()).copied().unwrap_or(0);
        assert!(slack as f32 <= crate::capacity::SLACK_SHARE * 480.0 + 1.0, "{slack}");
        assert!(laid >= 240, "half the day at least is laid: {laid}");
    }

    #[test]
    fn overlapping_things_side_by_side() {
        // 9–10 and 9:30–11 overlap; 10:30–11 fits under the first; 12–13 alone.
        let spans = [(540, 600), (570, 660), (630, 660), (720, 780)];
        assert_eq!(side_by_side(&spans), vec![(0, 2), (1, 2), (0, 2), (0, 1)]);
        // Touching is not overlapping.
        assert_eq!(side_by_side(&[(540, 600), (600, 660)]), vec![(0, 1), (0, 1)]);
    }
}
