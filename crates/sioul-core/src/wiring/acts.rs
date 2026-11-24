// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The record of acts (docs/rules.md, "Once, on every device"): what the
//! rules did, one JSON line per act, added at the end of a file and never
//! rewritten. Each device writes its own log,
//! `$XDG_STATE_HOME/sioul/rules/acts/<device>.jsonl` (`own_log`), and reads
//! every device's (sioul-sync's `share::RULES_ACTS` carries them); the engine
//! reads them all before it acts:
//!
//! - **Once**: a rule never acts twice on one object (`Acts::touched`),
//!   whichever device acted first; what it makes has an identifier worked out
//!   from the rule and the object besides, so that two devices acting before
//!   either read the other's log write the same thing.
//! - **Not for this one**: an act undone, or said "not for this one", writes
//!   a line `undone`; the rule never acts on that object again, anywhere. A
//!   tie taken apart by hand, a task deleted: their act stays recorded, so
//!   they are never made again either.
//! - **Chains**: what a rule made is known by its act (`result`), with how
//!   many steps it is from the first object (`depth`): a rule never acts on
//!   what it made itself, and a chain stops at `declare::CHAIN` steps.
//!
//! A line names things by their addresses (`mid:…`, `sioul:task/…`); never
//! a subject nor a word of the mail.
//!
//! ```text
//! {"at":1795435200123,"rule":"0b6c…","object":"mid:a@example.org","act":"task","result":"sioul:task/5f…","wrote":true,"depth":1,"list":"local/work","digest":"9e1c…"}
//! {"at":1795435260000,"rule":"0b6c…","object":"mid:a@example.org","undone":true}
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The logs' folder, in the rules' state folder.
pub const FOLDER: &str = "acts";

/// The logs' folder on this device: `$XDG_STATE_HOME/sioul/rules/acts`.
pub fn folder() -> PathBuf {
    crate::config::state_dir().join("rules").join(FOLDER)
}

/// This device's own log in `folder`; `device` is its name in the sharing.
pub fn own_log(folder: &Path, device: &str) -> PathBuf {
    let safe: String = device.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    folder.join(format!("{}.jsonl", if safe.is_empty() { "this-device" } else { &safe }))
}

/// One line: an act of a rule on an object, or its undoing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Act {
    /// When, Unix milliseconds.
    pub at: i64,
    pub rule: String,
    /// The object that triggered it, by its address.
    pub object: String,
    /// What it did: "tie", "task", "topic", "tell"; "" for an undoing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub act: String,
    /// What it made or changed, by its address (`sioul:project/…` for a tie,
    /// `sioul:task/<UID>`, a topic's id); "" for a notice.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub result: String,
    /// It wrote something; false: it found it done (another device, by
    /// hand, a route), and wrote nothing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrote: bool,
    /// How many steps from the first object (1: the rule acted on something
    /// no rule made).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub depth: u8,
    /// A task made: its list, "account/id", where Undo finds it on any device.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub list: String,
    /// A task made: its text's digest as written, so that Undo takes it out
    /// only while it is as the rule left it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub digest: String,
    /// The act undone, or "not for this one": the rule never acts on that object again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub undone: bool,
    /// The device whose log it is in (its file's name), as read; never written.
    #[serde(skip)]
    pub device: String,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// Lines added at the end of the log at `path`, all at once under the
/// file's lock; the file yours alone, in a folder of yours alone (Unix).
/// Every view of rules told.
pub fn append(path: &Path, acts: &[Act]) -> Result<(), String> {
    if acts.is_empty() {
        return Ok(());
    }
    let mut text = String::new();
    for act in acts {
        text.push_str(&serde_json::to_string(act).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    crate::filelock::with_lock(path, || crate::calls::append_private(path, &text))?;
    crate::changes::said(crate::changes::Kind::Rules);
    Ok(())
}

/// Every device's acts, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Acts {
    pub all: Vec<Act>,
}

impl Acts {
    /// Every log in `folder`, each line that reads (one cut by a crash, or
    /// written by a later Sioul, left out), oldest first.
    pub fn read(folder: &Path) -> Acts {
        let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "jsonl") && p.is_file()).collect();
        files.sort();
        let mut all = Vec::new();
        for file in files {
            let device = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            all.extend(crate::spam::labels::read_from::<Act>(&file).into_iter().map(|act| Act { device: device.clone(), ..act }));
        }
        all.sort_by_key(|a| a.at);
        Acts { all }
    }

    /// From lines already read, oldest first.
    pub fn of(mut all: Vec<Act>) -> Acts {
        all.sort_by_key(|a| a.at);
        Acts { all }
    }

    /// Whether `rule` acted on `object`, or was told not to, on any device:
    /// then it never acts on it again.
    pub fn touched(&self, rule: &str, object: &str) -> bool {
        self.all.iter().any(|a| a.rule == rule && a.object == object)
    }

    /// Whether `rule`'s act on `object` was undone, or said "not for this one".
    pub fn undone(&self, rule: &str, object: &str) -> bool {
        self.all.iter().any(|a| a.rule == rule && a.object == object && a.undone)
    }

    /// The acts whose result is `object`: what made it, if a rule did, its
    /// act undone since or not (what a rule made counts for a chain either way).
    pub fn making(&self, object: &str) -> impl Iterator<Item = &Act> {
        self.all.iter().filter(move |a| !a.undone && !a.result.is_empty() && a.result == object && matches!(a.act.as_str(), "task"))
    }

    /// How many steps of rules `object` is from something no rule made: 0
    /// for that, else the deepest act that made it.
    pub fn depth_of(&self, object: &str) -> u8 {
        self.making(object).map(|a| a.depth).max().unwrap_or(0)
    }

    /// Whether `rule` itself made `object`.
    pub fn made_by(&self, object: &str, rule: &str) -> bool {
        self.making(object).any(|a| a.rule == rule)
    }

    /// A rule's acts, newest first, `limit` at most, each with whether it
    /// was undone since: what it did lately.
    pub fn lately(&self, rule: &str, limit: usize) -> Vec<(&Act, bool)> {
        self.all.iter().rev().filter(|a| a.rule == rule && !a.undone && !a.act.is_empty()).take(limit).map(|a| (a, self.undone(rule, &a.object))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two devices' logs read together: once, undone, depth, lately; a line
    /// a later Sioul wrote or a crash cut left out; no word of the mail.
    #[test]
    fn every_device_s_acts_are_read_together() {
        let folder = std::env::temp_dir().join(format!("sioul-wiring-acts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let made = Act { at: 1, rule: "r1".into(), object: "mid:a@example.org".into(), act: "task".into(), result: "sioul:task/t1".into(), wrote: true, depth: 1, ..Act::default() };
        let ((), said) = crate::changes::heard(|| append(&own_log(&folder, "desk/1"), std::slice::from_ref(&made)).unwrap());
        assert_eq!(said, [crate::changes::Kind::Rules]);
        assert!(folder.join("desk_1.jsonl").is_file());
        append(&own_log(&folder, "phone"), &[Act { at: 2, rule: "r1".into(), object: "mid:a@example.org".into(), undone: true, ..Act::default() }]).unwrap();
        std::fs::write(folder.join("later.jsonl"), "{\"at\":3,\"rule\":\"r9\",\"object\":\n{\"at\":").unwrap();
        let acts = Acts::read(&folder);
        assert_eq!(acts.all.len(), 2);
        assert_eq!(acts.all[0].device, "desk_1");
        assert!(acts.touched("r1", "mid:a@example.org") && !acts.touched("r2", "mid:a@example.org"));
        assert!(acts.undone("r1", "mid:a@example.org"));
        assert_eq!(acts.depth_of("sioul:task/t1"), 1, "made by a rule, undone or not: a chain counts it");
        let line = serde_json::to_string(&made).unwrap();
        assert_eq!(line, r#"{"at":1,"rule":"r1","object":"mid:a@example.org","act":"task","result":"sioul:task/t1","wrote":true,"depth":1}"#);
        let kept = Acts::of(vec![made.clone(), Act { at: 5, rule: "r2".into(), object: "sioul:task/t1".into(), act: "tie".into(), result: "sioul:project/p".into(), wrote: true, depth: 2, ..Act::default() }]);
        assert_eq!(kept.depth_of("sioul:task/t1"), 1);
        assert!(kept.made_by("sioul:task/t1", "r1") && !kept.made_by("sioul:task/t1", "r2"));
        assert_eq!(kept.lately("r1", 5).len(), 1);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
