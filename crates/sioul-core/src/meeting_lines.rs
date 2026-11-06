// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's own lines of a meeting with others, kept out of its file (second
//! review of the organizer's door, N6): once an event names guests, its file
//! holds only what they may read (`organizer::for_guests`), since its server
//! sends them that file. What Sioul wrote there beside the time kept around
//! it, its costs, gain and your reminder (`own_time`) and its ties (`links`)
//! is kept here, by the event's UID and by its time ("" the series, else
//! that time's RECURRENCE-ID line), each line as it was written: who takes a
//! child's event and its kind (`holders`), a block's task, one time's own
//! margins. `$XDG_DATA_HOME/sioul/meeting-lines.toml`, this device's own:
//! the sharing does not carry it yet.
//!
//! Its readers take the meeting's text with these lines put back
//! (`merged`), as an older Sioul wrote them into the file, so that every
//! reader of those lines (the children's upkeep, its cards and questions)
//! reads a meeting as any event; its writers write here, never into the
//! meeting (`set_from`).

use crate::lines;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The file's name, in Sioul's data folder.
pub const FILE: &str = "meeting-lines.toml";

/// One meeting's own lines, by its times: "" the series, else that time's RECURRENCE-ID line.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    #[serde(default)]
    pub times: BTreeMap<String, Vec<String>>,
}

impl Kept {
    pub fn is_empty(&self) -> bool {
        self.times.values().all(Vec::is_empty)
    }
}

/// The whole file: each meeting's own lines by its UID.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeetingLines {
    #[serde(default, rename = "event")]
    pub events: BTreeMap<String, Kept>,
}

/// Where it is kept.
pub fn default_path() -> PathBuf {
    #[cfg(any(test, feature = "test-hooks"))]
    if let Some(path) = HERE.with(|here| here.borrow().clone()) {
        return path;
    }
    crate::config::data_dir().join(FILE)
}

#[cfg(any(test, feature = "test-hooks"))]
thread_local! {
    /// The file for one test's thread alone: tests beside it write theirs.
    static HERE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// The file for this test's thread alone (none: Sioul's data folder's).
#[cfg(any(test, feature = "test-hooks"))]
pub fn path_here(path: Option<&Path>) {
    HERE.with(|here| *here.borrow_mut() = path.map(Path::to_path_buf));
}

/// The file as it is; empty when there is none, or it does not read.
pub fn load(path: &Path) -> MeetingLines {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// `uid`'s lines changed by `change` over the file as it is then, under its
/// lock; written only when it changes, its bytes on the disk before its name.
/// Whether it changed.
fn change(path: &Path, uid: &str, change: impl FnOnce(&mut Kept)) -> Result<bool, String> {
    crate::filelock::with_lock(path, || {
        let mut all = load(path);
        let before = all.clone();
        let mut kept = all.events.remove(uid).unwrap_or_default();
        change(&mut kept);
        kept.times.retain(|_, lines| !lines.is_empty());
        if !kept.is_empty() {
            all.events.insert(uid.to_string(), kept);
        }
        if all == before {
            return Ok(false);
        }
        let text = toml::to_string(&all).map_err(|e| e.to_string())?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        let bytes = format!("# Sioul's own lines of meetings with others, by their UID and time (docs/client.md): kept here, never in the meeting.\n{text}");
        crate::filelock::replace(path, bytes.as_bytes()).map_err(|e| e.to_string())?;
        Ok(true)
    })
}

/// Lines a meeting's file held, added to what is kept of it (`lines`: each
/// with its time, as `organizer::Aside::lines` gives them): a line of a name
/// already kept for that time takes the place of every line of that name
/// there (a newer holder line, a kind). Whether it changed.
pub fn add(path: &Path, uid: &str, lines: &[(String, String)]) -> Result<bool, String> {
    if uid.is_empty() || lines.is_empty() {
        return Ok(false);
    }
    change(path, uid, |kept| {
        let mut given: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for (time, line) in lines {
            let at = given.entry(time.as_str()).or_default();
            if !at.contains(line) {
                at.push(line.clone());
            }
        }
        for (time, new) in given {
            let names: Vec<String> = new.iter().map(|l| lines::raw_name(l)).collect();
            let there = kept.times.entry(time.to_string()).or_default();
            let mut written: Vec<String> = there.iter().filter(|l| !names.contains(&lines::raw_name(l))).cloned().collect();
            written.extend(new);
            // The same lines in another order: kept as they were.
            let sorted = |mut v: Vec<String>| {
                v.sort();
                v
            };
            if sorted(written.clone()) != sorted(there.clone()) {
                *there = written;
            }
        }
    })
}

/// What is kept of `uid` replaced by the Sioul lines of `text` (its file
/// with them put back, then changed by a reader's writer: who takes it, its
/// kind), every time of it. Whether it changed.
pub fn set_from(path: &Path, uid: &str, text: &str) -> Result<bool, String> {
    let lines = crate::organizer::aside_of(text).lines;
    change(path, uid, |kept| {
        kept.times.clear();
        for (time, line) in lines {
            kept.times.entry(time).or_default().push(line);
        }
    })
}

/// The time a VEVENT's lines are kept under: "" the series, else its RECURRENCE-ID line.
fn time_of(block: &[String]) -> String {
    block.iter().find(|l| lines::name(l) == "RECURRENCE-ID").cloned().unwrap_or_default()
}

/// Whether two RECURRENCE-ID lines name the same time: the same line, or the same instant.
fn same_time(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let zone = crate::zone::current();
    crate::agenda::instant_in(a, &zone).is_some_and(|at| crate::agenda::instant_in(b, &zone) == Some(at))
}

/// A meeting's text with what is kept of it here put back (`kept`), each
/// line in its own time's event, before its alarms, unless that event holds
/// it already; the text as it is when nothing is kept.
pub fn merged_with(text: &str, kept: &Kept) -> String {
    if kept.is_empty() {
        return text.to_string();
    }
    let source = lines::unfold(text);
    let mut out: Vec<String> = Vec::with_capacity(source.len() + 8);
    let (mut block, mut inside, mut nested): (Vec<String>, bool, usize) = (Vec::new(), false, 0);
    for line in source.iter() {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        if !inside {
            if name == "BEGIN" && value == "VEVENT" {
                inside = true;
                nested = 0;
                block = vec![line.clone()];
            } else {
                out.push(line.clone());
            }
            continue;
        }
        if name == "BEGIN" {
            nested += 1;
        } else if name == "END" && nested > 0 {
            nested -= 1;
        } else if name == "END" {
            inside = false;
            let time = time_of(&block);
            let wanted: Vec<String> = kept.times.iter().filter(|(t, _)| same_time(t, &time)).flat_map(|(_, l)| l.iter().cloned()).filter(|l| !block.contains(l)).collect();
            let at = block.iter().enumerate().skip(1).find(|(_, l)| lines::name(l) == "BEGIN").map_or(block.len(), |(i, _)| i);
            block.splice(at..at, wanted);
            block.push(line.clone());
            out.append(&mut block);
            continue;
        }
        block.push(line.clone());
    }
    let written = lines::fold(&out);
    if lines::unfold(&written) == source { text.to_string() } else { written }
}

/// A meeting's text with what this device keeps of it put back
/// (`merged_with`, from `default_path`), by its series' UID.
pub fn merged(text: &str) -> String {
    let uid = lines::unfold(text).iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()).unwrap_or_default();
    if uid.is_empty() {
        return text.to_string();
    }
    match load(&default_path()).events.get(&uid) {
        Some(kept) => merged_with(text, kept),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A child's meeting's holder lines and kind, kept by time, put back
    /// into its text as an older Sioul wrote them: each in its own time's
    /// event, never twice; a newer holder line takes the old one's place;
    /// set from a text, the whole entry follows it.
    #[test]
    fn a_meeting_s_own_lines_are_kept_by_time_and_put_back() {
        crate::config::stand_ins_only();
        let path = crate::config::data_dir().join(format!("meeting-lines-{}.toml", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let meeting = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:kept-1\r\nDTSTART:20991110T090000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Swimming\r\nATTENDEE:mailto:coach@example.net\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT1H\r\nEND:VALARM\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:kept-1\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTART:20991117T100000Z\r\nSUMMARY:Swimming\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let held = "X-SIOUL-TAKEN-BY;CN=Sam;PARTSTAT=ACCEPTED:mailto:sam@example.org".to_string();
        let kind = "X-SIOUL-CONCEPT:tag:aurelienpierre.com,2026:sioul/child-kind/activity".to_string();
        let that_time = "X-SIOUL-TAKEN-BY;CN=Alex;PARTSTAT=ACCEPTED:mailto:alex@example.org".to_string();
        assert!(add(&path, "kept-1", &[(String::new(), held.clone()), (String::new(), kind.clone()), ("RECURRENCE-ID:20991117T090000Z".into(), that_time.clone())]).unwrap());
        assert!(!add(&path, "kept-1", &[(String::new(), held.clone())]).unwrap(), "the same: nothing written");
        let kept = load(&path).events.get("kept-1").cloned().unwrap();
        let text = lines::unfold(&merged_with(meeting, &kept)).join("\n");
        let (series, one) = text.split_at(text.rfind("BEGIN:VEVENT").unwrap());
        assert!(series.contains(&held) && series.contains(&kind) && !series.contains(&that_time) && one.contains(&that_time) && !one.contains(&held), "{text}");
        assert!(series.find(&held).unwrap() < series.find("BEGIN:VALARM").unwrap(), "before its alarm");
        assert_eq!(merged_with(&merged_with(meeting, &kept), &kept), merged_with(meeting, &kept), "never twice");
        let newer = held.replace("Sam", "Sam B");
        add(&path, "kept-1", &[(String::new(), newer.clone())]).unwrap();
        let series = load(&path).events["kept-1"].times[""].clone();
        assert!(series.contains(&newer) && !series.contains(&held) && series.contains(&kind), "{series:?}");
        set_from(&path, "kept-1", &merged_with(meeting, &Kept { times: [(String::new(), vec![kind.clone()])].into() })).unwrap();
        assert_eq!(load(&path).events["kept-1"].times, [(String::new(), vec![kind.clone()])].into(), "set from a text: the whole entry");
        set_from(&path, "kept-1", meeting).unwrap();
        assert!(!load(&path).events.contains_key("kept-1"), "none left: none kept");
        let _ = std::fs::remove_file(&path);
    }
}
