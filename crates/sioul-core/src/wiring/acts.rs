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
//! - **Taken back**: Undo's line also says `back` and names the result it
//!   took back; every device that wrote that same result takes its own copy
//!   back once, and writes a line `taken` (`Acts::to_take_back`). "Not for
//!   this one" alone takes nothing back: what was made stays where it is.
//! - **Chains**: what a rule made is known by its act (`result`), with how
//!   many steps it is from the first object (`depth`): a rule never acts on
//!   what it made itself, and a chain stops at `declare::CHAIN` steps.
//!
//! A line names things by their addresses (`mid:…`, `sioul:task/…`); never
//! a subject nor a word of the mail.
//!
//! ```text
//! {"at":1795435200123,"rule":"0b6c…","object":"mid:a@example.org","act":"task","result":"sioul:task/5f…","wrote":true,"depth":1,"list":"local/work","digest":"9e1c…"}
//! {"at":1795435260000,"rule":"0b6c…","object":"mid:a@example.org","result":"sioul:task/5f…","undone":true,"back":true}
//! {"at":1795435290000,"rule":"0b6c…","object":"mid:a@example.org","result":"sioul:task/5f…","taken":true}
//! ```

#[allow(unused_imports)]
use crate::files::Probe as _;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The logs' folder, in the rules' state folder.
pub const FOLDER: &str = "acts";

/// The logs' folder on this device: `$XDG_STATE_HOME/sioul/rules/acts`.
pub fn folder() -> PathBuf {
    crate::config::state_dir().join("rules").join(FOLDER)
}

/// The name a device's lines are read under (`Act::device`): its log's.
pub fn device_name(device: &str) -> String {
    let safe: String = device.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if safe.is_empty() { "this-device".to_string() } else { safe }
}

/// This device's own log in `folder`; `device` is its name in the sharing.
pub fn own_log(folder: &Path, device: &str) -> PathBuf {
    folder.join(format!("{}.jsonl", device_name(device)))
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
    /// The act undone, or "not for this one": the rule never acts on that
    /// object again, on any device.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub undone: bool,
    /// Undo took back `result`: every device that wrote that same result
    /// takes its own copy back (`engine::Engine::take_back`). Without it, an
    /// `undone` line ("not for this one", an Undo too late) leaves what was
    /// made where it is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub back: bool,
    /// The device whose log holds this line took back its copy of `result`
    /// (or found it changed and left it): never asked again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub taken: bool,
    /// A paper kept where no antivirus could read its file
    /// (`papers::Paper::unchecked`): its mark is put back from here where a
    /// device still on 0.1.0 dropped it (`engine::Engine::restore_marks`),
    /// until a line `MARK_OFF` names the paper.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unchecked: bool,
    /// The rule matched but could not act, for a reason that may pass (a
    /// message id, "rules-refused-…"): tried again at each pass for a week
    /// (`PENDING_MS`), said once. Such a line does not count as done.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pending: String,
    /// What this act took out and why, a message id: a paper whose file an
    /// antivirus refused (`result` names it).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub refused: String,
    /// What the rule does, as a short digest of its "then" (`engine::same`):
    /// a rule made again with another id does not make twice what one with
    /// the same "then" made ("also for the last 30 days").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub same: String,
    /// The device whose log it is in (its file's name), as read; never written.
    #[serde(skip)]
    pub device: String,
}

/// The act of a line that says a paper's mark "not checked" came off for
/// good (`result` names the paper): an antivirus read it clean, or its file
/// was replaced.
pub const MARK_OFF: &str = "mark-off";

/// How long a pending act is tried again (milliseconds): a week.
pub const PENDING_MS: i64 = 7 * 86_400_000;

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

/// Every device's acts, oldest first, with the pairs each rule acted on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Acts {
    pub all: Vec<Act>,
    /// (rule, object) of every line but the pending ones: done, found done, undone.
    touched: std::collections::HashSet<(String, String)>,
    /// (rule, object) undone or said "not for this one".
    undone: std::collections::HashSet<(String, String)>,
    /// (rule, object, result) Undo took back.
    back: std::collections::HashSet<(String, String, String)>,
    /// (device, rule, object, result) a device took back already.
    taken: std::collections::HashSet<(String, String, String, String)>,
}

/// A line as an older or a newer Sioul may have written it: when it does
/// not read whole (a field of another type), its rule and its object still
/// count, so that a device never does again what another did.
fn lenient(line: &str) -> Option<Act> {
    if let Ok(act) = serde_json::from_str::<Act>(line) {
        return Some(act);
    }
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let text = |key: &str| value.get(key).and_then(serde_json::Value::as_str).map(str::to_string);
    let (rule, object) = (text("rule")?, text("object")?);
    Some(Act { at: value.get("at").and_then(serde_json::Value::as_i64).unwrap_or(0), rule, object, act: "unknown".into(), undone: value.get("undone").and_then(serde_json::Value::as_bool).unwrap_or(false), ..Act::default() })
}

impl Acts {
    /// Every log in `folder`, each line that reads (one cut by a crash, or
    /// written by a later Sioul, left out), oldest first.
    pub fn read(folder: &Path) -> Acts {
        let mut files: Vec<PathBuf> = crate::files::read_dir(folder).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "jsonl") && p.present_file()).collect();
        files.sort();
        let mut all = Vec::new();
        for file in files {
            let device = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let Ok(text) = crate::files::read_to_string(&file) else { continue };
            all.extend(text.lines().filter_map(lenient).map(|act| Act { device: device.clone(), ..act }));
        }
        Acts::of(all)
    }

    /// From lines already read, oldest first.
    pub fn of(mut all: Vec<Act>) -> Acts {
        all.sort_by_key(|a| a.at);
        let pair = |a: &Act| (a.rule.clone(), a.object.clone());
        let touched = all.iter().filter(|a| a.pending.is_empty()).map(pair).collect();
        let undone = all.iter().filter(|a| a.undone).map(pair).collect();
        let back = all.iter().filter(|a| a.undone && a.back).map(|a| (a.rule.clone(), a.object.clone(), a.result.clone())).collect();
        let taken = all.iter().filter(|a| a.taken).map(|a| (a.device.clone(), a.rule.clone(), a.object.clone(), a.result.clone())).collect();
        Acts { all, touched, undone, back, taken }
    }

    /// Whether `rule` acted on `object`, found it done, or was told not to,
    /// on any device: then it never acts on it again. A pending line does
    /// not count: the act is tried again.
    pub fn touched(&self, rule: &str, object: &str) -> bool {
        self.touched.contains(&(rule.to_string(), object.to_string()))
    }

    /// Whether `rule`'s act on `object` was undone, or said "not for this one".
    pub fn undone(&self, rule: &str, object: &str) -> bool {
        self.undone.contains(&(rule.to_string(), object.to_string()))
    }

    /// Whether any rule doing the same (`same`) acted on `object`.
    pub fn same_done(&self, object: &str, same: &str) -> bool {
        !same.is_empty() && self.all.iter().any(|a| a.object == object && a.same == same && a.pending.is_empty())
    }

    /// The lines waiting to be tried again: pending, younger than a week at
    /// `now_ms`, their pair not done since; the newest of each pair.
    pub fn pending(&self, now_ms: i64) -> Vec<&Act> {
        let mut out: Vec<&Act> = Vec::new();
        for act in self.all.iter().rev().filter(|a| !a.pending.is_empty() && now_ms - a.at < PENDING_MS && !self.touched(&a.rule, &a.object)) {
            if !out.iter().any(|o| o.rule == act.rule && o.object == act.object) {
                out.push(act);
            }
        }
        out
    }

    /// Whether a pending line waits for this pair already.
    pub fn is_pending(&self, rule: &str, object: &str) -> bool {
        self.all.iter().any(|a| a.rule == rule && a.object == object && !a.pending.is_empty())
    }

    /// What `device` (its log's name) wrote that an Undo on any device took
    /// back since, the very result it names, and that this device has not
    /// taken back yet: what it must take back (`engine::Engine::take_back`).
    /// "Not for this one" asks nothing back.
    pub fn to_take_back(&self, device: &str) -> Vec<&Act> {
        let key = |a: &Act| (a.rule.clone(), a.object.clone(), a.result.clone());
        self.all
            .iter()
            .filter(|a| a.device == device && a.wrote && !a.undone && !a.taken && !a.act.is_empty() && a.pending.is_empty() && self.back.contains(&key(a)))
            .filter(|a| !self.taken.contains(&(device.to_string(), a.rule.clone(), a.object.clone(), a.result.clone())))
            .collect()
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
        self.all.iter().rev().filter(|a| a.rule == rule && !a.undone && !a.act.is_empty() && a.pending.is_empty()).take(limit).map(|a| (a, self.undone(rule, &a.object))).collect()
    }

    /// A rule's refusals waiting to be tried again, newest first: what the
    /// view shows beside "what it did lately", said once.
    pub fn waiting(&self, rule: &str, now_ms: i64) -> Vec<&Act> {
        self.pending(now_ms).into_iter().filter(|a| a.rule == rule).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two devices' logs read together: once, undone, depth, lately, what
    /// to take back and what to leave; a line a later Sioul wrote or a crash
    /// cut left out; no word of the mail.
    #[test]
    fn every_device_s_acts_are_read_together() {
        let folder = std::env::temp_dir().join(format!("sioul-wiring-acts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let made = Act { at: 1, rule: "r1".into(), object: "mid:a@example.org".into(), act: "task".into(), result: "sioul:task/t1".into(), wrote: true, depth: 1, ..Act::default() };
        let ((), said) = crate::changes::heard(|| append(&own_log(&folder, "desk/1"), std::slice::from_ref(&made)).unwrap());
        assert_eq!(said, [crate::changes::Kind::Rules]);
        assert!(folder.join("desk_1.jsonl").is_file());
        append(&own_log(&folder, "phone"), &[Act { at: 2, rule: "r1".into(), object: "mid:a@example.org".into(), undone: true, ..Act::default() }]).unwrap();
        std::fs::write(folder.join("later.jsonl"), "{\"at\":3,\"rule\":\"r9\",\"object\":\n{\"at\":\n{\"at\":4,\"rule\":\"r8\",\"object\":\"mid:b@example.org\",\"depth\":\"deep\"}\n").unwrap();
        let acts = Acts::read(&folder);
        assert_eq!(acts.all.len(), 3, "a cut line left out, a line of another shape kept by its rule and object");
        assert!(acts.touched("r8", "mid:b@example.org"), "what a newer Sioul did is never done again here");
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
        // A refusal that may pass waits, does not count as done, and is tried for a week.
        let waiting = Acts::of(vec![Act { at: 10, rule: "r3".into(), object: "mid:c@example.org".into(), act: "task".into(), pending: "rules-refused-write".into(), ..Act::default() }]);
        assert!(!waiting.touched("r3", "mid:c@example.org") && waiting.is_pending("r3", "mid:c@example.org"));
        assert_eq!(waiting.pending(20).len(), 1);
        assert!(waiting.pending(10 + PENDING_MS).is_empty(), "a week later, no more");
        // Two papers of one message, written on the desk: "not for this one" takes nothing back;
        // an Undo takes back the one it names, once.
        let paper = |at: i64, id: &str| Act { at, rule: "r4".into(), object: "mid:d@example.org".into(), act: "paper".into(), result: format!("sioul:paper/{id}"), wrote: true, device: "desk".into(), ..Act::default() };
        let mark = |at: i64, id: &str, back: bool, taken: bool, device: &str| Act { at, rule: "r4".into(), object: "mid:d@example.org".into(), result: format!("sioul:paper/{id}"), undone: !taken, back, taken, device: device.into(), ..Act::default() };
        let left = Acts::of(vec![paper(1, "p1"), paper(1, "p2"), mark(2, "p1", false, false, "phone")]);
        assert!(left.undone("r4", "mid:d@example.org") && left.to_take_back("desk").is_empty(), "not for this one: left where it is");
        let back = Acts::of(vec![paper(1, "p1"), paper(1, "p2"), mark(2, "p1", true, false, "phone")]);
        assert_eq!(back.to_take_back("desk").iter().map(|a| a.result.as_str()).collect::<Vec<_>>(), ["sioul:paper/p1"], "the paper undone, not the other");
        assert!(back.to_take_back("phone").is_empty(), "the phone wrote nothing");
        let done = Acts::of(vec![paper(1, "p1"), paper(1, "p2"), mark(2, "p1", true, false, "phone"), mark(3, "p1", false, true, "desk")]);
        assert!(done.to_take_back("desk").is_empty(), "taken back once");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
