// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each device is for, on this device (docs/areas.md, "What each device
//! is for"): the lines of the devices that count, as every process of this
//! device reads them (`lines`, `known`: the window, a phone's background step
//! and its listener of other apps), and which device tells a thing among
//! them (`Tellers`: the "notices" lease read per area,
//! `sioul_sync::lease::told_by`).
//!
//! What now is for on this device is `hours::mode_at`, which gives every
//! time these lines (`quiet::Mode::on`): the pages and the notifications
//! then read this device's cell through `Mode::brings` and `Now::brings`,
//! and stay as they were while nothing is said of this device. The plan and
//! agents read the hours alone. The doses and Health read nothing here.

use crate::backend::load_config;
use sioul_core::areas::{Area, Lines};
use sioul_sync::dispatch::Known;
use sioul_sync::lease::{Claim, Dispatch, Keeper, Rule};
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

/// The part of the work the notices' claims are on (`share::keeper`).
const NOTICES: &str = "notices";

fn changed(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The devices that count, as this device knows them now
/// (`sioul_sync::dispatch::Known::read`): read again when the settings,
/// this device's place in the sharing or what it kept of the others change,
/// and each hour at least, since a device silent for a week stops counting.
pub(crate) fn known() -> Known {
    type Key = ([std::path::PathBuf; 3], i64, [Option<SystemTime>; 3]);
    static KEPT: Mutex<Option<(Key, Known)>> = Mutex::new(None);
    let state = sioul_core::config::state_dir();
    let now = jiff::Timestamp::now().as_second();
    let files = [crate::backend::config_path(), sioul_sync::share::Here::path(&state), sioul_sync::peers::Peers::path(&state)];
    let stamps = files.each_ref().map(|file| changed(file));
    let key: Key = (files, now / 3600, stamps);
    if let Ok(kept) = KEPT.lock()
        && let Some((known_key, known)) = kept.as_ref()
        && *known_key == key
    {
        return known.clone();
    }
    let known = Known::read(&load_config(), &state, now);
    if let Ok(mut kept) = KEPT.lock() {
        *kept = Some((key, known.clone()));
    }
    known
}

/// What the person said each device that counts is for, this device's line
/// first: what a time is given to become this device's (`quiet::Mode::on`).
pub(crate) fn lines() -> Lines {
    known().lines()
}

/// Which device tells, read once for several things: this device's id, the
/// claims on the notices as they read now, and what each device is for at
/// this moment.
pub(crate) struct Tellers {
    me: String,
    claims: Vec<Claim>,
    dispatch: Dispatch,
    now: i64,
}

impl Tellers {
    /// Read now. From the window (`active`: when the person was last at this
    /// device), its claim is renewed first, as before (`share::keeper`);
    /// from a process without the window, the claims are only read.
    pub(crate) fn now(active: Option<i64>) -> Tellers {
        // Its claim could not be written: the others may not know of it, and it tells as a device alone, as before.
        let unwritten = active.is_some_and(|active| crate::share::keeper(NOTICES, Rule::FollowsYou, active, false).1);
        let known = known();
        let mode = crate::hours::mode_now();
        // The claims as the folder holds them now; none with sharing off, or its key not held: this device alone.
        let claims = match crate::share::vault() {
            Some((_, Some((folder, key)))) if !unwritten => sioul_sync::lease::claims(&folder, &key, NOTICES),
            _ => Vec::new(),
        };
        Tellers { me: known.me.clone(), claims, dispatch: known.dispatch(mode.time, mode.week), now: jiff::Timestamp::now().as_second() }
    }

    /// Whether this device tells a thing for `area` (none: a thing no device
    /// narrows, told by the device used last of all, as before): among the
    /// devices it comes forward on now, the one used last
    /// (`sioul_sync::lease::tells`). Sharing off: this device.
    pub(crate) fn tells(&self, area: Option<Area>) -> bool {
        sioul_sync::lease::tells(&self.claims, &self.me, &self.dispatch, area, self.now)
    }

    /// The keeper of the notices for a thing for `area`, with whether this
    /// device has been the one long enough to act (`lease::keeper_for`).
    pub(crate) fn keeper(&self, area: Option<Area>) -> Keeper {
        sioul_sync::lease::keeper_for(&self.claims, NOTICES, &self.me, &self.dispatch, area, self.now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With nothing said and no sharing (a test's own folders): this device
    /// alone, its line saying nothing, and it tells whatever a thing is for.
    #[test]
    fn alone_with_nothing_said_this_device_tells_everything() {
        let known = known();
        assert!(known.lines().say_nothing() && known.older.is_empty());
        let tellers = Tellers::now(None);
        for area in [None, Some(Area::WORK), Some(Area::PERSONAL), Some(Area::ALL)] {
            assert!(tellers.tells(area), "{area:?}");
            let keeper = tellers.keeper(area);
            assert!(keeper.mine && keeper.settled, "{area:?}: {keeper:?}");
        }
    }
}
