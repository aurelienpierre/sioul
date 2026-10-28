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

/// The file as it is now, read again only when it changed (its time and
/// size): the expander asks it once per calendar file.
pub fn current() -> Arc<OwnTime> {
    type Seen = Option<(PathBuf, Option<std::time::SystemTime>, u64, Arc<OwnTime>)>;
    static SEEN: Mutex<Seen> = Mutex::new(None);
    let path = default_path();
    let meta = std::fs::metadata(&path).ok();
    let stamp = (meta.as_ref().and_then(|m| m.modified().ok()), meta.as_ref().map_or(0, |m| m.len()));
    let mut seen = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((was, modified, len, own)) = seen.as_ref()
        && *was == path
        && (*modified, *len) == stamp
    {
        return Arc::clone(own);
    }
    let own = Arc::new(if meta.is_some() { load(&path) } else { OwnTime::default() });
    *seen = Some((path, stamp.0, stamp.1, Arc::clone(&own)));
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
        Ok(true)
    })
}

/// Each occurrence whose event this file has: its margins, its costs and
/// gain, and your reminder, as you set them here (this file wins).
pub fn apply(occurrences: &mut [crate::agenda::Occurrence], own: &OwnTime) {
    if own.events.is_empty() {
        return;
    }
    for o in occurrences.iter_mut() {
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
        let mut seen = vec![crate::agenda::Occurrence { uid: "board-1".into(), margins: Margins { before: 5, after: 0 }, remind: "NONE".into(), ..Default::default() }, crate::agenda::Occurrence { uid: "other".into(), margins: Margins { before: 5, after: 0 }, ..Default::default() }];
        apply(&mut seen, &load(&path));
        assert_eq!((seen[0].margins, seen[0].remind.as_str()), (Margins { before: 15, after: 10 }, "30"), "this file wins");
        assert_eq!(seen[1].margins, Margins { before: 5, after: 0 }, "the others as their files say");
        assert!(set(&path, "board-1", None).unwrap());
        assert!(load(&path).events.is_empty());
        std::fs::write(&path, "not toml = [").unwrap();
        assert!(load(&path).events.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
