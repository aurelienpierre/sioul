// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each device is for, on this device (docs/areas.md, "Which device
//! tells"): the devices that count as every process of this device reads
//! them (`known`: the window, a phone's background step and its listener of
//! other apps), this device among them (`lines`), and the keeper of the
//! notices read as it reads itself (`as_keeper`).
//!
//! What now is for on this device is `hours::mode_at`, which gives every
//! time these lines (`quiet::Mode::on`): the pages and the notifications
//! then read this device's cell through `Mode::brings` and `Now::brings`,
//! and stay as they were while nothing is said of this device. One device
//! tells, the keeper of the notices (`share::keeper`, as before devices): it
//! tells what comes forward on it now; the others read each thing as it
//! would (`as_keeper`), claim what it tells, and keep what it leaves waiting
//! (`mailnote`). The plan and agents read the hours alone; the doses, Health,
//! calls and texts too (`hours::hours_mode_at`), and nothing here.

use crate::backend::load_config;
use sioul_core::areas::Lines;
use sioul_sync::dispatch::Known;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

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

/// This device among the devices that count: what a time is given to
/// become this device's (`quiet::Mode::on`).
pub(crate) fn lines() -> Lines {
    known().lines()
}

/// The same for the date reminders, which a phone never tells
/// (`reminders::Holds::on`, `hours::with_layers`).
pub(crate) fn lines_for_dates() -> Lines {
    known().lines_for_dates()
}

/// The keeper of the notices (`keeper`: its id; this device's own, or ""
/// with sharing off) read as it reads itself (`Known::as_keeper`): what it
/// tells now, this device claims without a word.
pub(crate) fn as_keeper(keeper: &str) -> Lines {
    known().as_keeper(keeper)
}

/// What this device says in its entry: its own cells, in its week now, and
/// the cells it acts on of each other device (`devices::Entry::cells`,
/// `known`), so that the keeper reads them (docs/areas.md, "Which device tells").
pub(crate) fn said_in_entry() -> (std::collections::BTreeMap<String, String>, std::collections::BTreeMap<String, String>) {
    let known = known();
    let week = crate::hours::hours_mode_at(&sioul_core::zone::now()).week;
    (sioul_core::areas::Cells::Line(known.mine).words(week), known.acts_on())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With nothing said and no sharing (a test's own folders): this device
    /// alone, its line saying nothing, read as itself when it keeps the
    /// notices, and its entry says no cells.
    #[test]
    fn alone_with_nothing_said_this_device_narrows_nothing() {
        let known = known();
        assert!(known.lines().say_nothing() && known.older.is_empty() && known.others.is_empty());
        assert_eq!((as_keeper(&known.me), as_keeper("")), (lines(), lines()));
        assert!(as_keeper("another").say_nothing(), "a keeper this device knows nothing of: the hours");
        assert_eq!(said_in_entry(), (Default::default(), Default::default()));
    }
}
