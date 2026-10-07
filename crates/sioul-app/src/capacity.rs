// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a day holds, as the window uses it (docs/capacity.md): the record
//! gathered for the plan each time it is made (`planned`) and kept for what
//! the pages show beside it; today's slots of time for you, named, with a
//! suggestion from your own well-rated items, and written to
//! `state/slots.toml` for every process of this device to hold what waits in
//! them (`sioul_core::attention::Slots`, `crate::hours::in_slot`); the day's
//! balance, in words, for the evening's review (`day_balance`).

use crate::backend::{load_config, say, tr};
use jiff::Zoned;
use jiff::civil::Date;
use sioul_core::agenda::Occurrence;
use sioul_core::capacity::{self, DayBalance, Record};
use sioul_core::dayview::DayView;
use sioul_core::plan::Settings;
use sioul_core::tasks::Task;
use sioul_core::timelog::Session;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Events between two instants, read again at most every five minutes for the
/// same span: the plan is made often, the calendars change rarely.
fn events(from: i64, to: i64) -> Vec<Occurrence> {
    type Kept = BTreeMap<(i64, i64), (i64, Vec<Occurrence>)>;
    static KEPT: Mutex<Kept> = Mutex::new(BTreeMap::new());
    let now = jiff::Timestamp::now().as_second();
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    kept.retain(|_, (at, _)| (0..300).contains(&(now - *at)));
    if let Some((_, events)) = kept.get(&(from, to)) {
        return events.clone();
    }
    let events = sioul_core::agenda::occurrences(from, to);
    kept.insert((from, to), (now, events.clone()));
    events
}

/// The record as last gathered.
static LAST: Mutex<Option<Arc<Record>>> = Mutex::new(None);

/// The plan's settings with what your record says: the ratings said after
/// tasks, corrected lengths, what a day holds, the free time kept, the gain
/// slot (`capacity::gather`). The record is kept for the pages.
pub(crate) fn planned(mut settings: Settings, tasks: &[Task], sessions: &[Session]) -> Settings {
    // A task's time block is the task's time: never an event of its own to the record (`blocks`).
    let own = |from: i64, to: i64| sioul_core::blocks::without_blocks(events(from, to), tasks);
    let record = capacity::gather(tasks, sessions, &settings, &load_config().planning, &own, &Zoned::now());
    settings.capacity = record.planning.clone();
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(record));
    settings
}

/// The record as last gathered, if the plan was made.
pub(crate) fn last() -> Option<Arc<Record>> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The line a task's panel shows on request: how tasks like it usually go
/// against the first guess; "" when there is nothing worth saying.
pub(crate) fn ratio_line(task: &Task) -> String {
    last().map(|record| record.ratio_line(task, tr())).unwrap_or_default()
}

/// Today's layout in words: the slots of time for you named, one suggestion
/// from your own items when there is one (G19), the free time kept named, and
/// why today holds what it holds. Its slots are written down, when they
/// changed, for every process of this device: what can wait, waits (G18b),
/// whichever process decides it (the window, the listener of other apps, a
/// phone's background step, `sioul remind --watch`, the calls' table).
pub(crate) fn dress(day: &mut DayView, tasks: &[Task]) {
    let now = Zoned::now();
    let record = last();
    let suggestion = record.as_ref().and_then(|r| capacity::suggestion(tasks, &r.index, now.date()));
    for block in &mut day.blocks {
        match block.kind {
            "gain" => {
                block.title = tr().text("capacity-gain-slot", None);
                block.note = suggestion.as_ref().map(|title| say("capacity-gain-suggestion", &[("title", title.clone())])).unwrap_or_default();
            }
            "slack" => block.title = tr().text("capacity-slack", None),
            _ => {}
        }
    }
    day.said = record.map(|r| r.reasons(now.date(), tr())).unwrap_or_default();
    let slots = sioul_core::attention::Slots { day: Some(now.date()), slots: day.blocks.iter().filter(|b| b.kind == "gain").map(|b| (b.start, b.end)).collect() };
    match slots.save(&sioul_core::attention::Slots::default_path()) {
        // Calls ring or go to voicemail in them too: the phone's table follows.
        Ok(true) => {
            std::thread::spawn(|| crate::calls::refresh(true));
        }
        Ok(false) => {}
        Err(e) => eprintln!("sioul: {e}"),
    }
}

/// A day's balance, in words, for the evening's review: each cost and the
/// total against what a day holds for you (light, usual, heavy; "" when nothing
/// is known), what gave back against its minimum (below, around, above; ""),
/// and at most two lines on what the plan does with it. Never a number to show.
pub(crate) fn day_balance(date: Date) -> DayBalance {
    let zone = jiff::tz::TimeZone::system();
    let tasks = sioul_core::tasks::all(&zone);
    let sessions = sioul_core::timelog::sessions();
    let record = last().unwrap_or_else(|| {
        let config = load_config();
        let settings = Settings::of_hours(&config.week_hours(), sioul_core::areas::TaskAreas::usual());
        let own = |from: i64, to: i64| sioul_core::blocks::without_blocks(events(from, to), &tasks);
        Arc::new(capacity::gather(&tasks, &sessions, &settings, &config.planning, &own, &Zoned::now()))
    });
    // The day's outcome as said now: the review may be newer than the record.
    let mut record = (*record).clone();
    record.outcomes.remove(&date);
    record.outcomes.extend(capacity::outcomes_of(&sioul_core::reviews::Reviews::load_between(&sioul_core::reviews::Reviews::default_path(), date, date)));
    let midnight = |d: Date| d.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second());
    let next = date.tomorrow().map_or(midnight(date) + 86_400, midnight);
    // A task's time block is the task's time, counted with the task (`blocks`).
    let events = sioul_core::blocks::without_blocks(events(midnight(date), next), &tasks);
    record.day_balance(&tasks, &sessions, &events, date, &zone, tr())
}
