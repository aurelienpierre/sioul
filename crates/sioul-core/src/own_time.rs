// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your own time around a meeting with others (the unified agenda's decision
//! 18, part 4): the time kept before and after it, what it costs and gives
//! back, and your reminder for it, kept in a file of yours by the event's
//! UID (`$XDG_DATA_HOME/sioul/own-time.toml`), never in the meeting's own
//! file: its server tells every guest of any change to it (RFC 6638), so no
//! writer of Sioul's writes into it any more (`agenda::may_change`).
//!
//! Read in the one expander of every event (`agenda::file_occurrences_as`),
//! so the plan, quiet time, the reminders, the home card, the agenda and
//! "Leave at" all follow it; for an event it has, this file wins over the
//! lines an older Sioul wrote into the meeting (read until then). Read with a
//! default: no file, or one that does not read, changes nothing. The file
//! travels with the sharing's time part as a store of its own (sync
//! share.rs), each event's entry whole; a Sioul that does not know the
//! store keeps its changes waiting, writes nothing and takes nothing out.

use crate::demands::{Demands, Margins};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The file's name, in Sioul's data folder.
pub const FILE: &str = "own-time.toml";

/// Your own time around one meeting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Own {
    /// Minutes kept before and after it.
    #[serde(default)]
    pub before: u32,
    #[serde(default)]
    pub after: u32,
    /// What it costs and gives back, as you rate them; none unsaid.
    #[serde(default, skip_serializing_if = "is_unsaid")]
    pub demands: Demands,
    /// Your reminder before it, as `reminders::REMIND` says it: "" as usual, "NONE", "15".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub remind: String,
}

fn is_unsaid(demands: &Demands) -> bool {
    *demands == Demands::default()
}

impl Own {
    pub fn margins(&self) -> Margins {
        Margins { before: self.before, after: self.after }
    }
}

/// The whole file: each meeting's own time by its UID.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnTime {
    #[serde(default, rename = "event")]
    pub events: BTreeMap<String, Own>,
}

/// Where it is kept.
pub fn default_path() -> PathBuf {
    crate::config::data_dir().join(FILE)
}

/// The file as it is; empty when there is none, or it does not read.
pub fn load(path: &Path) -> OwnTime {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// This process's writes of the file (`set`): a write here is never read stale.
static WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// What says the file changed: this process's writes, and the file's time (to
/// the nanosecond), size and inode (each write replaces it, a new inode: two
/// writes by another program within one tick of the clock still differ;
/// review of part 4, later 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    writes: u64,
    modified: Option<std::time::SystemTime>,
    len: u64,
    inode: u64,
}

fn stamp_of(meta: Option<&std::fs::Metadata>) -> Stamp {
    #[cfg(unix)]
    let inode = meta.map_or(0, std::os::unix::fs::MetadataExt::ino);
    #[cfg(not(unix))]
    let inode = 0;
    Stamp { writes: WRITES.load(std::sync::atomic::Ordering::SeqCst), modified: meta.and_then(|m| m.modified().ok()), len: meta.map_or(0, std::fs::Metadata::len), inode }
}

/// The file as it is now, read again only when it changed (`Stamp`): the
/// expander asks it once per calendar file, a stat each time.
pub fn current() -> Arc<OwnTime> {
    current_at(&default_path())
}

fn current_at(path: &Path) -> Arc<OwnTime> {
    type Seen = Option<(PathBuf, Stamp, Arc<OwnTime>)>;
    static SEEN: Mutex<Seen> = Mutex::new(None);
    let meta = std::fs::metadata(path).ok();
    let stamp = stamp_of(meta.as_ref());
    let mut seen = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((was, at, own)) = seen.as_ref()
        && was == path
        && *at == stamp
    {
        return Arc::clone(own);
    }
    let own = Arc::new(if meta.is_some() { load(path) } else { OwnTime::default() });
    *seen = Some((path.to_path_buf(), stamp, Arc::clone(&own)));
    own
}

/// `uid`'s own time set to `own` (None takes it out), under the file's lock,
/// over the file as it is then; written only when it changes, its bytes on
/// the disk before its name. Returns whether it changed.
pub fn set(path: &Path, uid: &str, own: Option<Own>) -> Result<bool, String> {
    crate::filelock::with_lock(path, || {
        let mut all = load(path);
        let before = all.clone();
        match own {
            Some(own) => {
                all.events.insert(uid.to_string(), own);
            }
            None => {
                all.events.remove(uid);
            }
        }
        if all == before {
            return Ok(false);
        }
        let text = toml::to_string(&all).map_err(|e| e.to_string())?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        let bytes = format!("# Your own time around meetings with others, by their UID (docs/client.md): kept here, never in the meeting.\n{text}");
        crate::filelock::replace(path, bytes.as_bytes()).map_err(|e| e.to_string())?;
        WRITES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(true)
    })
}

/// Each occurrence of a meeting with others whose event this file has: its
/// margins, its costs and gain, and your reminder, as you set them here (this
/// file wins). An event that is no meeting (its guests taken out elsewhere)
/// reads its own lines again, which its form writes (review of part 4, later 4).
pub fn apply(occurrences: &mut [crate::agenda::Occurrence], own: &OwnTime) {
    if own.events.is_empty() {
        return;
    }
    for o in occurrences.iter_mut().filter(|o| o.with_others) {
        if let Some(mine) = own.events.get(&o.uid) {
            o.margins = mine.margins();
            o.demands = mine.demands;
            o.remind = mine.remind.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set, read back, written only when it changes, taken out; a file that
    /// does not read is none.
    #[test]
    fn own_time_is_kept_by_uid_and_written_only_when_it_changes() {
        let dir = std::env::temp_dir().join(format!("sioul-own-time-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(FILE);
        let own = Own { before: 15, after: 10, remind: "30".into(), ..Own::default() };
        assert!(set(&path, "board-1", Some(own.clone())).unwrap());
        assert!(!set(&path, "board-1", Some(own.clone())).unwrap(), "the same again: nothing written");
        assert_eq!(load(&path).events.get("board-1"), Some(&own));
        let meeting = |uid: &str| crate::agenda::Occurrence { uid: uid.into(), margins: Margins { before: 5, after: 0 }, remind: "NONE".into(), with_others: true, ..Default::default() };
        let mut seen = vec![meeting("board-1"), meeting("other"), crate::agenda::Occurrence { with_others: false, ..meeting("board-1") }];
        apply(&mut seen, &load(&path));
        assert_eq!((seen[0].margins, seen[0].remind.as_str()), (Margins { before: 15, after: 10 }, "30"), "this file wins");
        assert_eq!(seen[1].margins, Margins { before: 5, after: 0 }, "the others as their files say");
        assert_eq!((seen[2].margins, seen[2].remind.as_str()), (Margins { before: 5, after: 0 }, "NONE"), "no meeting any more: its own lines (later 4)");
        assert!(set(&path, "board-1", None).unwrap());
        assert!(load(&path).events.is_empty());
        std::fs::write(&path, "not toml = [").unwrap();
        assert!(load(&path).events.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two writes of the same size within one tick of the clock: the second
    /// is read, never the first kept (later 15).
    #[test]
    fn the_file_read_again_after_each_write() {
        let dir = std::env::temp_dir().join(format!("sioul-own-time-tick-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(FILE);
        set(&path, "a", Some(Own { before: 15, ..Own::default() })).unwrap();
        let first = std::fs::metadata(&path).unwrap();
        assert_eq!(current_at(&path).events.get("a").map(|o| o.before), Some(15));
        set(&path, "a", Some(Own { before: 25, ..Own::default() })).unwrap();
        // The same size, and its time set back to the first one's, as within one tick.
        assert_eq!(std::fs::metadata(&path).unwrap().len(), first.len());
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(first.modified().unwrap()).unwrap();
        assert_eq!(current_at(&path).events.get("a").map(|o| o.before), Some(25), "never stale");
        // Another program's write (the sharing's writer, a phone's background process: no
        // count of this one's): a new file renamed over it, the same size, its time set back
        // (re-check of 857c39e, §3.6).
        let other = dir.join("own-time.toml.new");
        std::fs::write(&other, std::fs::read_to_string(&path).unwrap().replace("before = 25", "before = 35")).unwrap();
        std::fs::File::options().write(true).open(&other).unwrap().set_modified(first.modified().unwrap()).unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), first.len());
        assert_eq!(current_at(&path).events.get("a").map(|o| o.before), Some(35), "another program's write read too");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
