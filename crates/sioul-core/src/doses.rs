// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Each dose that fell due, as a record your devices share (docs/health.md,
//! "Doses as records"): `$XDG_STATE_HOME/sioul/health-doses.toml`, in the
//! sharing's health part.
//!
//! A dose falls due: a device that sees it opens its record, not taken yet
//! (the canary), and says so (`opened`: each device that did, and when). The
//! first device that captures your answer turns it into taken (when you took
//! it, earlier if you say so) or skipped, noting itself and when. Each device
//! writes only its own opening and its own answer, so the sharing, where the
//! later word wins entry by entry, never has two devices fighting over one
//! entry: answers merge here, when read (`answered`), by the earliest captured;
//! two that differ are both kept, both said, and you choose. A merge takes
//! nothing out. Records go nine days after their dose (`KEPT_DAYS`).
//!
//! A file of its own, not a table of `health-state.toml`: an older Sioul saves
//! that one through what it knows and would drop the table, which the sharing
//! would then take out on every device. An older Sioul keeps this file's
//! changes waiting, unknown to it, and writes nothing in it. The marks of
//! `health-state.toml` (`taken`, `not_taken`) are still written beside the
//! answers, for it, and read here as answers of a device not named.

use crate::health::{HealthState, Problem, Unsound};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How long a record is kept after its dose was due: the Health page's week
/// back (shown, read only), and two days.
pub const KEPT_DAYS: i64 = 9;

/// The file's name, in Sioul's state folder.
pub const FILE: &str = "health-doses.toml";

/// The doses that fell due, by key (`<medicine>@<Unix seconds>`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DoseRecords {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dose: BTreeMap<String, Record>,
}

/// One dose that fell due.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub medicine: String,
    /// When it was due (Unix seconds).
    #[serde(default)]
    pub due: i64,
    /// The canary: each device that saw it fall due, by its id, and when (its clock).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub opened: BTreeMap<String, i64>,
    /// Each device's answer, by its id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub answer: BTreeMap<String, Answer>,
}

/// One device's answer, shared whole (`whole = ["dose.*.answer.*"]` in the sharing).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// "taken" or "skipped"; another word (a later Sioul's) is an answer not understood here.
    pub state: String,
    /// Taken: when, as said (earlier than `noted` when asked). Skipped: when it was said.
    #[serde(default)]
    pub at: i64,
    /// When the answer was captured, on that device's clock.
    #[serde(default)]
    pub noted: i64,
    /// That device's name and kind then ("phone", "computer"), to say where.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
}

pub const TAKEN: &str = "taken";
pub const SKIPPED: &str = "skipped";

/// What an answer says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Taken,
    Skipped,
    /// A word this Sioul does not know (a later version's): an answer all the same.
    Other(String),
}

impl State {
    fn of(word: &str) -> State {
        match word {
            TAKEN => State::Taken,
            SKIPPED => State::Skipped,
            other => State::Other(other.to_string()),
        }
    }
}

/// An answer as merged: what, by which device, when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Given {
    pub state: State,
    /// The device that gave it; "" for a mark of `health-state.toml` (an older
    /// Sioul's, or this device's before records were kept).
    pub device: String,
    pub name: String,
    pub kind: String,
    /// Taken: when, as said. Skipped: when it was said.
    pub at: i64,
    /// When it was captured; 0 when not known (a mark of `health-state.toml`).
    pub noted: i64,
}

impl Given {
    /// When it was captured, as near as known: a mark of `health-state.toml`
    /// says only when it was taken, which is never after it was captured.
    fn captured(&self) -> i64 {
        if self.noted > 0 { self.noted } else { self.at }
    }
}

/// A dose's answers, merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    /// No answer anywhere this device has read: not taken yet, as far as the
    /// records go (whether another device may hold one is `health::doubts_now`'s).
    Open,
    /// Taken, everywhere it was answered: the earliest answer captured.
    Taken(Given),
    /// Said not taken, everywhere it was answered: the earliest.
    Skipped(Given),
    /// Answers that differ, each kept, the earliest captured first: you choose.
    Differ(Vec<Given>),
}

/// "levo@1800000000" → ("levo", 1800000000).
fn parts(key: &str) -> (String, i64) {
    match key.rsplit_once('@') {
        Some((medicine, due)) => (medicine.to_string(), due.parse().unwrap_or(0)),
        None => (key.to_string(), 0),
    }
}

/// A dose's answers, from the records and the marks of `health-state.toml`,
/// merged: the same answer everywhere is that answer, the earliest captured;
/// answers that differ are all kept. A mark of `health-state.toml` that says
/// what an answer already says is that answer, written beside it for an older Sioul.
pub fn answered(key: &str, state: &HealthState, records: &DoseRecords) -> Answered {
    let mut given: Vec<Given> = records
        .dose
        .get(key)
        .map(|record| record.answer.iter().map(|(device, a)| Given { state: State::of(&a.state), device: device.clone(), name: a.name.clone(), kind: a.kind.clone(), at: a.at, noted: a.noted }).collect())
        .unwrap_or_default();
    let has = |given: &[Given], wanted: &State| given.iter().any(|g| g.state == *wanted);
    if let Some(at) = state.taken.get(key).copied().filter(|_| !has(&given, &State::Taken)) {
        given.push(Given { state: State::Taken, device: String::new(), name: String::new(), kind: String::new(), at, noted: 0 });
    }
    if let Some(at) = state.not_taken.get(key).copied().filter(|_| !has(&given, &State::Skipped)) {
        given.push(Given { state: State::Skipped, device: String::new(), name: String::new(), kind: String::new(), at, noted: at });
    }
    given.sort_by(|a, b| a.captured().cmp(&b.captured()).then_with(|| a.device.cmp(&b.device)));
    let Some(first) = given.first().cloned() else { return Answered::Open };
    if given.iter().any(|g| g.state != first.state) {
        return Answered::Differ(given);
    }
    match first.state {
        State::Taken => Answered::Taken(first),
        State::Skipped => Answered::Skipped(first),
        State::Other(_) => Answered::Differ(given),
    }
}

impl DoseRecords {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join(FILE)
    }

    /// The records as written, or why they cannot be trusted. Never empty in
    /// place of a file that does not read: an answer given would look not
    /// given, and written back empty, the sharing would take it out everywhere.
    /// Of no bytes, or gone, though written before (`health::witnessed`): lost.
    pub fn read(path: &Path) -> Result<DoseRecords, Unsound> {
        match std::fs::read_to_string(path) {
            Ok(text) if text.trim().is_empty() && crate::health::witnessed(path) => Err(Unsound::Lost),
            Ok(text) => toml::from_str(&text).map_err(|_| Unsound::Unreadable),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if crate::health::witnessed(path) {
                    Err(Unsound::Lost)
                } else {
                    Ok(DoseRecords::default())
                }
            }
            Err(_) => Err(Unsound::Unreadable),
        }
    }

    /// The records changed by `change`, one writer at a time (the window, the
    /// sharing: `filelock`), read again just before, so that nothing written
    /// meanwhile is written over.
    pub fn update<T>(path: &Path, now: i64, change: impl FnOnce(&mut DoseRecords) -> T) -> Result<T, Problem> {
        crate::filelock::with_lock(path, || {
            let mut records = DoseRecords::read(path).map_err(Problem::Unsound)?;
            let out = change(&mut records);
            records.save(path, now).map_err(Problem::Write)?;
            Ok(out)
        })
    }

    /// Records that cannot be trusted, kept aside as `<name>.unreadable-<time>`
    /// with their witness gone: new ones can start. Only once nothing they
    /// held can be taken out elsewhere (`share::rebuild`).
    pub fn set_aside(path: &Path, now: i64) -> Result<Option<PathBuf>, String> {
        crate::filelock::with_lock(path, || {
            for witness in crate::health::witnesses(path) {
                let _ = std::fs::remove_file(witness);
            }
            if !path.exists() {
                return Ok(None);
            }
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let aside = path.with_file_name(format!("{name}.unreadable-{now}"));
            std::fs::rename(path, &aside).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(Some(aside))
        })
    }

    /// Saved, records past `KEPT_DAYS` dropped. Use `update`: it holds the lock.
    fn save(&mut self, path: &Path, now: i64) -> Result<(), String> {
        let oldest = now - KEPT_DAYS * 86_400;
        self.dose.retain(|key, record| (if record.due > 0 { record.due } else { parts(key).1 }) >= oldest);
        let text = format!("# Each dose that fell due, and the answers your devices captured (docs/health.md).\n{}", toml::to_string(self).map_err(|e| e.to_string())?);
        crate::health::write_witnessed(path, text.as_bytes())
    }

    fn record(&mut self, key: &str) -> &mut Record {
        self.dose.entry(key.to_string()).or_insert_with(|| {
            let (medicine, due) = parts(key);
            Record { medicine, due, ..Record::default() }
        })
    }

    /// The canary: `device` saw the dose fall due at `now`. Opened already
    /// here, by any device, it is left as it is. Returns whether it changed.
    pub fn open(&mut self, key: &str, device: &str, now: i64) -> bool {
        if self.dose.get(key).is_some_and(|r| !r.opened.is_empty() || !r.answer.is_empty()) {
            return false;
        }
        self.record(key).opened.insert(device.to_string(), now);
        true
    }

    /// The device that opened the record first, and when; none when no device did.
    pub fn opened_first(&self, key: &str) -> Option<(String, i64)> {
        self.dose.get(key)?.opened.iter().min_by_key(|(device, at)| (**at, (*device).clone())).map(|(device, at)| (device.clone(), *at))
    }

    /// `device`'s answer, given now: its own, replacing what it said before.
    /// Another device's answer stays, the same or not: answers that differ are
    /// both kept, and said (`answered`).
    pub fn answer(&mut self, key: &str, device: &str, answer: Answer) {
        self.record(key).answer.insert(device.to_string(), answer);
    }

    /// You chose, seeing the answers that differ: `device`'s answer, and every
    /// other answer that says otherwise taken out, on purpose.
    pub fn choose(&mut self, key: &str, device: &str, answer: Answer) {
        let state = answer.state.clone();
        let record = self.record(key);
        record.answer.retain(|other, given| other == device || given.state == state);
        record.answer.insert(device.to_string(), answer);
    }

    /// The mark taken back ("one click takes it back"): every answer saying it
    /// was taken goes, on purpose; the dose is open again.
    pub fn take_back(&mut self, key: &str) {
        if let Some(record) = self.dose.get_mut(key) {
            record.answer.retain(|_, given| given.state != TAKEN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-doses-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn said(state: &str, at: i64, noted: i64, name: &str) -> Answer {
        Answer { state: state.into(), at, noted, name: name.into(), kind: if name == "phone" { "phone".into() } else { "computer".into() } }
    }

    const KEY: &str = "levo@1800000000";

    #[test]
    fn records_that_cannot_be_trusted_are_never_taken_for_empty() {
        let dir = scratch("trust");
        let path = dir.join("health-doses.toml");
        assert_eq!(DoseRecords::read(&path), Ok(DoseRecords::default()), "never written: empty, honestly");
        DoseRecords::update(&path, 1_800_000_100, |r| r.answer(KEY, "phone-id", said(TAKEN, 1_800_000_060, 1_800_000_090, "phone"))).unwrap();
        assert!(DoseRecords::read(&path).unwrap().dose[KEY].answer.contains_key("phone-id"));
        // Broken: unreadable, and nothing written over it.
        std::fs::write(&path, "[dose.\"levo@1800000000\"\nstate =").unwrap();
        assert_eq!(DoseRecords::read(&path), Err(Unsound::Unreadable));
        assert_eq!(DoseRecords::update(&path, 1_800_000_200, |r| r.dose.clear()), Err(Problem::Unsound(Unsound::Unreadable)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[dose.\"levo@1800000000\"\nstate =", "left as it was");
        // Gone after being written: lost, not new.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(DoseRecords::read(&path), Err(Unsound::Lost));
        assert_eq!(DoseRecords::set_aside(&path, 1_800_000_300), Ok(None));
        assert_eq!(DoseRecords::read(&path), Ok(DoseRecords::default()), "set aside: new ones can start");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn answers_merge_by_the_earliest_and_answers_that_differ_both_stay() {
        let empty = HealthState::default();
        let mut records = DoseRecords::default();
        assert_eq!(answered(KEY, &empty, &records), Answered::Open, "nothing anywhere: the canary");
        records.open(KEY, "desk-id", 1_800_000_010);
        assert_eq!(answered(KEY, &empty, &records), Answered::Open, "opened, not answered");
        // Taken on the phone at 08:02, then on the desktop later: one answer, the earliest captured.
        records.answer(KEY, "desk-id", said(TAKEN, 1_800_000_600, 1_800_000_600, "desk"));
        records.answer(KEY, "phone-id", said(TAKEN, 1_800_000_120, 1_800_000_125, "phone"));
        match answered(KEY, &empty, &records) {
            Answered::Taken(given) => assert_eq!((given.device.as_str(), given.at), ("phone-id", 1_800_000_120)),
            other => panic!("{other:?}"),
        }
        // Skipped on the desktop instead: both kept, both said, the earliest first; never one picked.
        records.answer(KEY, "desk-id", said(SKIPPED, 1_800_000_600, 1_800_000_600, "desk"));
        match answered(KEY, &empty, &records) {
            Answered::Differ(given) => {
                assert_eq!(given.iter().map(|g| (g.device.as_str(), g.state.clone())).collect::<Vec<_>>(), [("phone-id", State::Taken), ("desk-id", State::Skipped)]);
            }
            other => panic!("{other:?}"),
        }
        // You choose "taken": the desktop's answer is yours now, the other kept.
        records.choose(KEY, "desk-id", said(TAKEN, 1_800_000_700, 1_800_000_700, "desk"));
        assert!(matches!(answered(KEY, &empty, &records), Answered::Taken(ref g) if g.device == "phone-id"));
        // You choose "not taken": the answers saying otherwise go, on purpose.
        records.choose(KEY, "desk-id", said(SKIPPED, 1_800_000_800, 1_800_000_800, "desk"));
        assert!(matches!(answered(KEY, &empty, &records), Answered::Skipped(ref g) if g.device == "desk-id"));
        assert_eq!(records.dose[KEY].answer.len(), 1);
        // A word this Sioul does not know (a later version's): an answer that differs, said, never dropped.
        records.answer(KEY, "tablet-id", said("half", 1_800_000_900, 1_800_000_900, "tablet"));
        assert!(matches!(answered(KEY, &empty, &records), Answered::Differ(ref g) if g.len() == 2));
        // Taken back: every "taken" goes; what said otherwise stays.
        let mut back = DoseRecords::default();
        back.answer(KEY, "phone-id", said(TAKEN, 1_800_000_120, 1_800_000_125, "phone"));
        back.take_back(KEY);
        assert_eq!(answered(KEY, &empty, &back), Answered::Open);
    }

    #[test]
    fn the_marks_of_older_sioul_are_answers_too() {
        let records = DoseRecords::default();
        // An older phone marked it taken in health-state.toml only: taken, by a device not named.
        let mut state = HealthState::default();
        state.taken.insert(KEY.into(), 1_800_000_120);
        assert!(matches!(answered(KEY, &state, &records), Answered::Taken(ref g) if g.device.is_empty() && g.at == 1_800_000_120));
        // The same mark beside a record's answer (a new device writes both): one answer, the record's.
        let mut both = DoseRecords::default();
        both.answer(KEY, "phone-id", said(TAKEN, 1_800_000_120, 1_800_000_125, "phone"));
        assert!(matches!(answered(KEY, &state, &both), Answered::Taken(ref g) if g.device == "phone-id"));
        // An older device said it was not taken; the desktop's record says taken: both said.
        let mut not = HealthState::default();
        not.not_taken.insert(KEY.into(), 1_800_000_300);
        assert!(matches!(answered(KEY, &not, &both), Answered::Differ(ref g) if g.len() == 2));
        // Both marks in health-state.toml (the later word did not win between two maps): both said.
        state.not_taken.insert(KEY.into(), 1_800_000_300);
        assert!(matches!(answered(KEY, &state, &records), Answered::Differ(_)));
    }

    #[test]
    fn a_canary_opened_on_two_devices_is_one_record() {
        let mut desk = DoseRecords::default();
        assert!(desk.open(KEY, "desk-id", 1_800_000_030));
        assert!(!desk.open(KEY, "desk-id", 1_800_000_090), "opened once");
        // The phone's opening arrives (the sharing writes its entry beside the desktop's): one record, both kept.
        desk.dose.get_mut(KEY).unwrap().opened.insert("phone-id".into(), 1_800_000_020);
        assert_eq!(desk.dose.len(), 1);
        assert_eq!(desk.opened_first(KEY), Some(("phone-id".into(), 1_800_000_020)));
        assert_eq!(desk.dose[KEY].medicine, "levo");
        assert_eq!(desk.dose[KEY].due, 1_800_000_000);
    }

    #[test]
    fn records_go_nine_days_after_their_dose() {
        let dir = scratch("prune");
        let path = dir.join("health-doses.toml");
        let due = 1_800_000_000;
        DoseRecords::update(&path, due + 60, |r| {
            r.open(KEY, "desk-id", due + 30);
            r.open("iron@1800600000", "desk-id", 1_800_600_030);
        })
        .unwrap();
        // Eight days on: both kept (the Health page shows a week back).
        DoseRecords::update(&path, due + 8 * 86_400, |_| ()).unwrap();
        assert_eq!(DoseRecords::read(&path).unwrap().dose.len(), 2);
        // Ten days on: the first goes, the later one stays.
        DoseRecords::update(&path, due + 10 * 86_400, |_| ()).unwrap();
        assert_eq!(DoseRecords::read(&path).unwrap().dose.keys().collect::<Vec<_>>(), ["iron@1800600000"]);
        // As the sharing reads it: one table per answer, kept whole.
        DoseRecords::update(&path, due + 10 * 86_400, |r| r.answer("iron@1800600000", "phone-id", said(TAKEN, 1_800_600_100, 1_800_600_110, "phone"))).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[dose.\"iron@1800600000\".answer.phone-id]"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
