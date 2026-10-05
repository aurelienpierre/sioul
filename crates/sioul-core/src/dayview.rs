// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The day, seen (docs/tasks.md, "The day"): today's events at their times,
//! and the steps the plan gives today laid into your hours, each into hours
//! of its own kind (work, admin, free time), a pause between them and around
//! the events; a line where now is. A step up to an hour is never cut; a
//! longer one goes on after a break. Visual supports cut transition time
//! (Dettmer et al. 2000); the current step is shown inside the whole day,
//! with what comes next (Brili, Tiimo). A layout to look at, never a schedule
//! to keep: nothing is written into the tasks, and a step that runs over
//! only moves the next ones.

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
    /// "event", "task", "meal", "nap".
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
    let hours = crate::plan::stretches(&settings.windows, today, &zone, settings.anything);
    let (opening, closing) = match (hours.first(), hours.last()) {
        (Some(first), Some(last)) => (first.0, last.1),
        _ => (at(9), at(17)),
    };
    // Today, and the end of hours that run past midnight: what lasts longer is cut there.
    let day_end = next_midnight.max(closing);
    let mut view = DayView { now: stamp, hours: hours.iter().map(|&(start, end, open)| Stretch { start, end, kind: open.id() }).collect(), ..DayView::default() };
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
        view.blocks.push(Block { start, end, kind: "event", title: event.summary.clone(), key: event.key.clone(), energy: String::new(), location: event.location.clone(), column: 0, columns: 1, part: 0 });
    }
    // Meals and naps, kept free (`needs`): shown, a step never laid over them.
    // The night is kept free too, but not drawn: the day would run from midnight.
    for kept in settings.needs.kept_on(today, &zone, &|key: &str| settings.shifts.get(key).copied().unwrap_or(0)).into_iter().filter(|k| k.kind != "sleep") {
        let (start, end) = (kept.start.max(midnight), kept.end.min(day_end));
        if end > start {
            view.blocks.push(Block { start, end, kind: kept.kind, title: kept.name.clone(), key: kept.key.clone(), energy: String::new(), location: String::new(), column: 0, columns: 1, part: 0 });
        }
    }
    // Held: the events with a pause around each, what is kept free, what passed, and each step laid, with its pause.
    let pause = i64::from(settings.pause) * 60;
    let mut busy: Vec<(i64, i64)> = crate::plan::event_spans(events, settings.pause);
    busy.extend(settings.kept_on(today, &zone, today));
    busy.push((i64::MIN, rounded(stamp)));
    let mut laid: BTreeMap<&str, i64> = BTreeMap::new();
    for uid in &plan.order {
        let (Some(task), Some(planned)) = (tasks.iter().find(|t| &t.uid == uid), plan.items.get(uid)) else { continue };
        if planned.optional || planned.open_steps > 0 || planned.start != Some(today) || planned.on_start == 0 {
            continue;
        }
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
        if runs.is_empty() {
            view.more += 1;
            continue;
        }
        // After what it waits for, laid today.
        let after = planned.waits_for.iter().filter_map(|w| laid.get(w.as_str())).max().map_or(i64::MIN, |end| end + pause);
        let free: Vec<(i64, i64)> = crate::plan::less(&runs, &busy).into_iter().map(|(from, to, _)| (rounded(from.max(after)), to)).filter(|(from, to)| to > from).collect();
        let length = i64::from(planned.on_start) * 60;
        // A step up to an hour in one go, in the first gap that holds it; a longer
        // one (or one the plan already cut) from the first gap on, in parts of a
        // quarter of an hour at least.
        let cut = planned.left > WHOLE || planned.on_start < planned.left.max(SHORTEST);
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
            view.blocks.push(Block { start: from, end: to, kind: "task", title: task.title.clone(), key: task.uid.clone(), energy: task.energy.clone(), location: task.location.clone(), column: 0, columns: 1, part: if count > 1 { k as u32 + 1 } else { 0 } });
            busy.push((from, to + pause));
        }
        laid.insert(uid, pieces.last().map_or(0, |p| p.1));
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
        // The tax form in admin hours, the report in work hours, friends in free time.
        let mut seen = laid(&view);
        seen.sort();
        assert_eq!(seen, vec![("friends", 18 * 60 + 30), ("report", 9 * 60), ("tax", 17 * 60)]);
        assert_eq!(view.hours.iter().map(|h| h.kind.as_str()).collect::<Vec<_>>(), vec!["work", "admin", "leisure"]);
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
    fn overlapping_things_side_by_side() {
        // 9–10 and 9:30–11 overlap; 10:30–11 fits under the first; 12–13 alone.
        let spans = [(540, 600), (570, 660), (630, 660), (720, 780)];
        assert_eq!(side_by_side(&spans), vec![(0, 2), (1, 2), (0, 2), (0, 1)]);
        // Touching is not overlapping.
        assert_eq!(side_by_side(&[(540, 600), (600, 660)]), vec![(0, 1), (0, 1)]);
    }
}
