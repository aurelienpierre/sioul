// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What now is for, as the window asks it (docs/areas.md): work, admin,
//! leisure, a meal, sleep, the pauses; and what reaches you then, as the one
//! matrix says (`attention_now`, `comes`; the model is
//! `sioul_core::attention`, docs/attention.md). One place for the pages and
//! the notifications; the rule itself is `sioul_core::quiet::mode`. Health's
//! meals, naps and nights are read again when its files change, today's
//! events at most every five minutes, today's slots of time for you when
//! their file changes: this is asked often (every list of sites, every
//! notification).

use crate::backend::load_config;
use jiff::Zoned;
use sioul_core::attention::{Attention, Column, Kind, Level, Now, Row, Slots};
use sioul_core::quiet::{self, Blocks, Mode, Overrides};
use sioul_core::reminders::Holds;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

/// When a file last changed; none when it is not there.
fn changed(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Today's events as the time they hold (their margins counted), read again
/// every five minutes at most, and on a new day.
fn held(now: &Zoned) -> Vec<(i64, i64)> {
    static KEPT: Mutex<Option<(i64, jiff::civil::Date, Vec<(i64, i64)>)>> = Mutex::new(None);
    let stamp = now.timestamp().as_second();
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, day, held)) = kept.as_ref()
        && *day == now.date()
        && (0..300).contains(&(stamp - at))
    {
        return held.clone();
    }
    let midnight = now.date().to_zoned(now.time_zone().clone()).map_or(stamp, |z| z.timestamp().as_second());
    // From yesterday (a meeting past midnight) to tomorrow night.
    let held = sioul_core::plan::event_spans(&sioul_core::agenda::occurrences(midnight - 86_400, midnight + 2 * 86_400), 0);
    *kept = Some((stamp, now.date(), held.clone()));
    held
}

/// Health's meals, naps and nights around now, each day's changes applied
/// and the meals pushed past events, as the Health page has them.
pub(crate) fn blocks(now: &Zoned) -> Blocks {
    type Key = (i64, Option<SystemTime>, Option<SystemTime>);
    static KEPT: Mutex<Option<(Key, Blocks)>> = Mutex::new(None);
    let (health, days) = (sioul_core::health::Health::default_path(), sioul_core::needs::Days::default_path());
    let key = (now.timestamp().as_second() / 60, changed(&health), changed(&days));
    if let Some((known, blocks)) = KEPT.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
        && *known == key
    {
        return blocks.clone();
    }
    let needs = sioul_core::health::Health::load(&health).needs;
    let blocks = Blocks::of(&needs, &sioul_core::needs::Days::load(&days), &held(now), now);
    *KEPT.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, blocks.clone()));
    blocks
}

/// What now is for, and until when.
pub(crate) fn mode_at(now: &Zoned) -> Mode {
    let config = load_config();
    let overrides = Overrides::load(&Overrides::default_path());
    quiet::mode(&config.week_hours(), &config.time_off, &overrides, &blocks(now), now)
}

pub(crate) fn mode_now() -> Mode {
    mode_at(&Zoned::now())
}

/// Today's slots of time for you (docs/capacity.md, G18b), as the window
/// that laid out the day wrote them (`state/slots.toml`, `capacity::dress`):
/// every process of this device reads the same, so that Time for you holds
/// everywhere. Read again when the file changes.
pub(crate) fn slots() -> Slots {
    static KEPT: Mutex<Option<(Option<SystemTime>, Slots)>> = Mutex::new(None);
    let path = Slots::default_path();
    let stamp = changed(&path);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((known, slots)) = kept.as_ref()
        && *known == stamp
        && stamp.is_some()
    {
        return slots.clone();
    }
    let slots = Slots::load(&path);
    *kept = Some((stamp, slots.clone()));
    slots
}

/// Whether `at` is in one of today's slots of time for you: a layer of the
/// matrix, which says what waits there (as usual, new mail's telling, the
/// sites' notifications, other apps' automatons, the pause to move).
pub(crate) fn in_slot(at: &Zoned) -> bool {
    slots().at(at)
}

/// The same, now.
pub(crate) fn quiet_slot() -> bool {
    in_slot(&Zoned::now())
}

/// The matrix as set (Settings ▸ What reaches you).
pub(crate) fn attention() -> Attention {
    Attention::of(&load_config())
}

/// The moment as the matrix reads it at `now`, in `mode`: the time, a slot
/// of time for you, do-not-disturb from its switch or a focus session
/// (docs/do-not-disturb.md), Free time's "Nothing at all", the Porch
/// resting after a pause.
pub(crate) fn attention_at(mode: &Mode, now: &Zoned) -> Now {
    let config = load_config();
    let overrides = Overrides::load(&Overrides::default_path());
    let mut moment = Now::of(mode).layers(in_slot(now), crate::everywhere::holds_others());
    moment.nothing = mode.free() && sioul_core::pause::nothing_now(&overrides, &config.free_time);
    moment.holds.porch_rests = overrides.porch_rests(now.timestamp().as_second());
    moment
}

/// The moment now.
pub(crate) fn attention_now() -> Now {
    let now = Zoned::now();
    attention_at(&mode_at(&now), &now)
}

/// What the matrix says now of one kind of Sioul's own.
pub(crate) fn level(kind: Kind) -> Level {
    attention().level(Row::Own(kind), &attention_now())
}

/// Whether one kind comes now, at once.
pub(crate) fn comes(kind: Kind) -> bool {
    level(kind) == Level::Now
}

/// Reminders' holds (`reminders::gather`) with this process's layers, the
/// do-not-disturb it keeps a few seconds rather than reading its files again.
pub(crate) fn with_layers(mut holds: Holds) -> Holds {
    holds.now.slot = quiet_slot();
    holds.now.dnd = crate::everywhere::holds_others();
    holds
}

/// Whether a dose's reminder waits now: the matrix's doses row, whose only
/// cells that may hold it are sleep's and the pause's, your choice. Never
/// without it: a dose at 05:00 is meant to wake you.
pub(crate) fn doses_silent() -> bool {
    !comes(Kind::Doses)
}

/// When this sleep ends (Unix seconds), while asleep: waking, or a nap's end;
/// while paused, in five minutes: the pause ends when you come back.
pub(crate) fn waking() -> Option<i64> {
    let mode = mode_now();
    if mode.paused() {
        return Some(Zoned::now().timestamp().as_second() + 5 * 60);
    }
    let sleeps = mode.sleeps();
    mode.until.filter(|_| sleeps).map(|until| until.timestamp().as_second())
}

/// When the sleep that ended within the last half hour began (Unix seconds),
/// when doses stay silent during sleep: the doses due since then waited for
/// waking, and are reminded now; the same for a pause holding doses, at
/// coming back (docs/pauses.md). None otherwise.
pub(crate) fn woke_from() -> Option<i64> {
    let attention = attention();
    let held_in = |column: Column| attention.cell(Row::Own(Kind::Doses), column) != Level::Now;
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let overrides = Overrides::load(&Overrides::default_path());
    let paused = overrides.paused_since.zip(overrides.paused_ended).filter(|(since, ended)| held_in(Column::Pause) && ended > since && *ended <= stamp && stamp - ended < 30 * 60).map(|(since, _)| since);
    if !held_in(Column::Sleep) {
        return paused;
    }
    let blocks = blocks(&now);
    // Not while a next one goes on (a nap right after the night).
    if blocks.at(stamp, &["sleep", "nap"]).is_some() {
        return None;
    }
    blocks.kept.iter().filter(|k| (k.kind == "sleep" || k.kind == "nap") && k.end <= stamp && stamp - k.end < 30 * 60).map(|k| k.start).chain(paused).min()
}

/// Whether a night is set (Health): without one, nothing keeps notifications away at night.
pub(crate) fn night_set() -> bool {
    blocks(&Zoned::now()).night
}
