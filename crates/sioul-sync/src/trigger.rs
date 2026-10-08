// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Shared as soon as Sioul saves (docs/database.md, "Sent as soon as it is
//! saved"): when a file a sharing part carries changes here (a dose answered,
//! a setting, `health.toml`, a project, a budget), an exchange runs a few
//! seconds later, then this device's files go to the server. One mechanism
//! for every change, whoever wrote it: the window looks at the shared files
//! every two seconds (`Stamps`), and a change seen, like an exchange asked
//! (a button, a dose answered, the minute's tick), wants one (`Soon`). Saves
//! that follow each other wait for a pause: two seconds after the last, five
//! after the first at most, so that a page saving each field as you type
//! sends once, and soon.
//!
//! The notes and papers folders are not looked at (thousands of files, every
//! two seconds): the minute's exchange reads them, as before.

use crate::share::{Shape, Store};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A save is followed by an exchange this long after the last one, if none follows (milliseconds)…
pub const QUIET_MS: i64 = 2_000;
/// …and never later than this after the first (milliseconds).
pub const MOST_MS: i64 = 5_000;
/// Exchanges for saves, and the minute's scheduled one, never start closer
/// than this to the last (milliseconds): a file Sioul writes each minute (a
/// session's time, a plan) costs one exchange a minute, not two, and a quiet
/// minute only the scheduled one. An exchange asked (a button, a dose
/// answered) goes at once all the same. A save is thus carried within a
/// minute, within seconds when the last exchange is older.
pub const MIN_GAP_MS: i64 = 50_000;
/// A folder of shared files is not looked through past this many files.
const LARGEST_WATCH: usize = 20_000;

/// When an exchange is wanted: asked now, or saves seen, waiting for a pause.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Soon {
    /// An exchange asked for this moment (milliseconds).
    asked: Option<i64>,
    /// The first save seen since the last exchange, and the last one (milliseconds).
    first: Option<i64>,
    last: i64,
    /// When the last exchange started (milliseconds).
    ran: i64,
}

impl Soon {
    /// Nothing wanted yet.
    pub const fn new() -> Soon {
        Soon { asked: None, first: None, last: 0, ran: 0 }
    }

    /// An exchange asked at `now_ms` (a button, a dose answered, the minute's tick): due at once.
    pub fn ask(&mut self, now_ms: i64) {
        self.asked = Some(self.asked.map_or(now_ms, |at| at.min(now_ms)));
    }

    /// The minute's scheduled exchange, at `now_ms`: at once, unless one
    /// started less than `MIN_GAP_MS` ago (a save's), which did its work.
    pub fn tick(&mut self, now_ms: i64) {
        if now_ms - self.ran >= MIN_GAP_MS {
            self.ask(now_ms);
        }
    }

    /// A shared file saved at `now_ms`: due after a pause.
    pub fn saved(&mut self, now_ms: i64) {
        self.first.get_or_insert(now_ms);
        self.last = self.last.max(now_ms);
    }

    /// When the exchange is due (milliseconds); none when nothing is wanted.
    pub fn due(&self) -> Option<i64> {
        let after_saves = self.first.map(|first| (self.last + QUIET_MS).min(first + MOST_MS).max(self.ran + MIN_GAP_MS));
        match (self.asked, after_saves) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Taken by the exchange starting at `now_ms`: what comes after wants the next one.
    pub fn take(&mut self, now_ms: i64) {
        *self = Soon { ran: now_ms, ..Soon::default() };
    }
}

/// How the shared files stand: each one's size and time, by its path. A
/// change of either is a save (or a file the exchange wrote, taken in after
/// it with `refresh`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stamps(BTreeMap<PathBuf, (u64, u128)>);

impl Stamps {
    /// The files `stores` carry, as they are now: each file, each folder's
    /// files below it (hidden ones left out: locks, files half written); the
    /// notes and papers folders left out.
    pub fn of(stores: &[Store]) -> Stamps {
        let mut stamps = Stamps::default();
        for store in stores.iter().filter(|s| watched(s)) {
            stamps.add(&store.path);
        }
        stamps
    }

    fn add(&mut self, path: &Path) {
        let Ok(meta) = std::fs::metadata(path) else { return };
        if meta.is_file() {
            self.0.insert(path.to_path_buf(), stamp(&meta));
            return;
        }
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
                if self.0.len() >= LARGEST_WATCH {
                    return;
                }
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                match entry.file_type() {
                    Ok(kind) if kind.is_dir() => stack.push(entry.path()),
                    Ok(kind) if kind.is_file() => {
                        if let Ok(meta) = entry.metadata() {
                            self.0.insert(entry.path(), stamp(&meta));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The files that differ from `before`: changed, new, gone.
    pub fn changed(&self, before: &Stamps) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self.0.iter().filter(|(path, stamp)| before.0.get(*path) != Some(stamp)).map(|(path, _)| path.clone()).collect();
        out.extend(before.0.keys().filter(|path| !self.0.contains_key(*path)).cloned());
        out
    }

    /// What the exchange wrote (`share::Outcome::written`, by store name),
    /// taken in as it is now: never taken for a save.
    pub fn refresh(&mut self, stores: &[Store], written: &std::collections::BTreeSet<String>) {
        for store in stores.iter().filter(|s| watched(s) && written.contains(&s.name)) {
            self.0.retain(|path, _| !path.starts_with(&store.path));
            self.add(&store.path);
        }
    }

    /// How many files are looked at.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Looked at every two seconds: every store but the folders sealed apart file by file (notes, papers).
fn watched(store: &Store) -> bool {
    !(store.folder && matches!(store.shape, Shape::Files))
}

fn stamp(meta: &std::fs::Metadata) -> (u64, u128) {
    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    (meta.len(), modified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::Roots;
    use sioul_core::config::Config;

    const T: i64 = 1_800_000_000_000;

    /// Saves wait for a pause, two seconds, never more than five after the
    /// first; an exchange asked goes at once; each exchange takes what came before it.
    #[test]
    fn the_trigger_waits_for_a_pause_and_never_long() {
        let mut soon = Soon::default();
        assert_eq!(soon.due(), None, "nothing wanted");
        // One save: two seconds later.
        soon.saved(T);
        assert_eq!(soon.due(), Some(T + QUIET_MS));
        // Saves every second (a page saving as you type): due five seconds after the first.
        for n in 1..=8 {
            soon.saved(T + n * 1_000);
        }
        assert_eq!(soon.due(), Some(T + MOST_MS));
        // A pause after two saves, the last exchange a minute old: two seconds after the last.
        soon.take(T - 60_000);
        soon.saved(T + 10_000);
        soon.saved(T + 11_000);
        assert_eq!(soon.due(), Some(T + 11_000 + QUIET_MS));
        // Asked meanwhile (a dose answered): at once.
        soon.ask(T + 11_500);
        assert_eq!(soon.due(), Some(T + 11_500));
        // Taken by the exchange: a save during it wants the next one, never closer than `MIN_GAP_MS`.
        soon.take(T + 11_500);
        assert_eq!(soon.due(), None);
        soon.saved(T + 12_000);
        assert_eq!(soon.due(), Some(T + 11_500 + MIN_GAP_MS));
        soon.ask(T + 13_000);
        assert_eq!(soon.due(), Some(T + 13_000), "asked: at once all the same");
    }

    /// A quiet minute runs the scheduled exchange alone; a file saved each
    /// minute (a session's time) costs one exchange a minute, not two; a save
    /// right after the scheduled one makes one more, and the next scheduled
    /// one, then too recent, is left out.
    #[test]
    fn a_quiet_minute_runs_the_scheduled_exchange_alone() {
        // The worker as the window runs it: due, taken, run; counted.
        let run = |events: &[(i64, &str)], until: i64| -> Vec<i64> {
            let mut soon = Soon::new();
            let mut ran = Vec::new();
            let mut events = events.to_vec();
            events.sort();
            let mut next = 0;
            for now in (T..=until).step_by(500) {
                while next < events.len() && events[next].0 <= now {
                    match events[next].1 {
                        "tick" => soon.tick(events[next].0),
                        "save" => soon.saved(events[next].0),
                        _ => soon.ask(events[next].0),
                    }
                    next += 1;
                }
                if soon.due().is_some_and(|due| due <= now) {
                    soon.take(now);
                    ran.push(now);
                }
            }
            ran
        };
        let ticks: Vec<(i64, &str)> = (0..10).map(|m| (T + m * 60_000, "tick")).collect();
        // Ten quiet minutes: ten exchanges, the scheduled ones.
        assert_eq!(run(&ticks, T + 10 * 60_000).len(), 10);
        // A file written each minute, whenever in the minute: about one a minute.
        for offset in [3_000, 12_000, 30_000, 50_000, 58_000] {
            let mut busy = ticks.clone();
            busy.extend((0..10).map(|m| (T + m * 60_000 + offset, "save")));
            let ran = run(&busy, T + 10 * 60_000);
            // At most one each `MIN_GAP_MS`: twelve in ten minutes, never twenty.
            assert!(ran.len() <= 12, "saved {offset} ms after each tick: {} exchanges in ten minutes: {ran:?}", ran.len());
            assert!(ran.windows(2).all(|w| w[1] - w[0] >= MIN_GAP_MS), "{ran:?}");
        }
        // One save in a quiet hour: carried within the minute, no more exchanges than the ticks and it.
        let mut once = ticks.clone();
        once.push((T + 3 * 60_000 + 20_000, "save"));
        let ran = run(&once, T + 10 * 60_000);
        assert!(ran.contains(&(T + 3 * 60_000 + MIN_GAP_MS)), "{ran:?}");
        assert!(ran.len() <= 11, "{ran:?}");
        // After a quiet stretch, a save goes within seconds.
        let mut late = vec![(T, "tick")];
        late.push((T + 70_000, "save"));
        assert!(run(&late, T + 80_000).contains(&(T + 70_000 + QUIET_MS)));
        // An exchange asked (a button) always runs.
        let mut asked = ticks.clone();
        asked.push((T + 5_000, "ask"));
        assert!(run(&asked, T + 60_000).contains(&(T + 5_000)));
    }

    /// A save is seen in any shared file, not in notes nor in hidden files;
    /// what the exchange wrote is taken in, never taken for a save.
    #[test]
    fn saves_are_seen_in_the_shared_files() {
        let base = std::env::temp_dir().join(format!("sioul-trigger-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let roots = Roots { config: base.join("config"), data: base.join("data"), state: base.join("state") };
        let notes = base.join("notes");
        for dir in [&roots.config, &roots.data.join("drafts"), &roots.state, &notes] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let config = Config { case_store: Some(notes.display().to_string()), ..Config::default() };
        let stores = crate::share::stores_of(&config, &roots, &|_| true);
        std::fs::write(roots.config.join("config.toml"), "language = \"fr\"\n").unwrap();
        std::fs::write(notes.join("lease.md"), "a note\n").unwrap();
        let before = Stamps::of(&stores);
        assert!(!before.is_empty() && before.0.keys().all(|p| !p.starts_with(&notes) || p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("sioul-"))), "{before:?}");
        // A note, a lock, a file half written: no save.
        std::fs::write(notes.join("lease.md"), "a note, longer\n").unwrap();
        std::fs::write(roots.state.join(".health-state.toml.lock"), "").unwrap();
        std::fs::write(roots.data.join("drafts").join(".draft.eml.123-1.sioul.tmp"), "half").unwrap();
        assert!(Stamps::of(&stores).changed(&before).is_empty());
        // A setting, a draft, a medicine: saves.
        std::fs::write(roots.config.join("config.toml"), "language = \"en\"\n").unwrap();
        std::fs::write(roots.data.join("drafts").join("draft.eml"), "Subject: lease\n").unwrap();
        std::fs::write(roots.data.join("health.toml"), "[[medicine]]\nid = \"levo\"\n").unwrap();
        let mut now = Stamps::of(&stores);
        let changed = now.changed(&before);
        assert_eq!(changed.len(), 3, "{changed:?}");
        // The exchange writes the doses' record: taken in, not a save after it.
        let seen = now.clone();
        std::fs::write(roots.state.join("health-state.toml"), "[taken]\n").unwrap();
        now.refresh(&stores, &["state/health-state.toml".to_string()].into());
        assert!(Stamps::of(&stores).changed(&now).is_empty());
        assert_eq!(now.changed(&seen).len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }
}
