// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What now is for, as the window asks it (docs/areas.md): work, admin,
//! leisure, a meal, sleep; and do-not-disturb while you sleep
//! (docs/health.md). One place for the pages and the notifications; the rule
//! itself is `sioul_core::quiet::mode`. Health's meals, naps and nights are
//! read again when its files change, and today's events at most every five
//! minutes: this is asked often (every list of sites, every notification).

use crate::backend::load_config;
use jiff::Zoned;
use sioul_core::quiet::{self, Blocks, Mode, Overrides};
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

/// Asleep now: the night from winding down to waking, or a nap.
pub(crate) fn asleep() -> bool {
    mode_now().sleeps()
}

/// Whether a notification may come now: none while you sleep (doses: `doses_silent`).
pub(crate) fn may_notify() -> bool {
    !asleep()
}

/// Whether a dose's reminder waits now: asleep, and "Doses during sleep: stay
/// silent" chosen. Never without that choice: a dose at 05:00 is meant to wake you.
pub(crate) fn doses_silent() -> bool {
    !load_config().reminders.doses_in_sleep && asleep()
}

/// When this sleep ends (Unix seconds), while asleep: waking, or a nap's end.
pub(crate) fn waking() -> Option<i64> {
    let mode = mode_now();
    let sleeps = mode.sleeps();
    mode.until.filter(|_| sleeps).map(|until| until.timestamp().as_second())
}

/// When the sleep that ended within the last half hour began (Unix seconds),
/// when doses stay silent during sleep: the doses due since then waited for
/// waking, and are reminded now. None otherwise.
pub(crate) fn woke_from() -> Option<i64> {
    if load_config().reminders.doses_in_sleep {
        return None;
    }
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let blocks = blocks(&now);
    // Not while a next one goes on (a nap right after the night).
    if blocks.at(stamp, &["sleep", "nap"]).is_some() {
        return None;
    }
    blocks.kept.iter().filter(|k| (k.kind == "sleep" || k.kind == "nap") && k.end <= stamp && stamp - k.end < 30 * 60).map(|k| k.start).min()
}

/// Whether a night is set (Health): without one, nothing keeps notifications away at night.
pub(crate) fn night_set() -> bool {
    blocks(&Zoned::now()).night
}
