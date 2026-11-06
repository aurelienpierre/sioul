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
//! (`merged`, `read`), as an older Sioul wrote them into the file, so that
//! every reader of those lines (the children's upkeep, its cards and
//! questions) reads a meeting as any event; its writers write here, never
//! into the meeting (`set_from`).
//!
//! **An entry is what this device knows** (third review of the organizer's
//! door, R1). Until the sharing carries this file, a meeting with no entry
//! here may have its lines on another device: who takes it and its kind are
//! unknown here (`Read::Unknown`), and the children's upkeep leaves it and
//! its stand-ins as they are. An entry may hold no line: the door made the
//! meeting here from an event that held none (`ensure`). When an event is no
//! meeting any more, its lines go back into its file and its entry goes
//! (`forget`).

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

/// `uid`'s entry changed by `change` over the file as it is then (None: no
/// entry, nothing known here), under its lock; written only when it changes,
/// its bytes on the disk before its name. Whether it changed.
fn change(path: &Path, uid: &str, change: impl FnOnce(&mut Option<Kept>)) -> Result<bool, String> {
    crate::filelock::with_lock(path, || {
        let mut all = load(path);
        let before = all.clone();
        let mut kept = all.events.remove(uid);
        change(&mut kept);
        if let Some(mut kept) = kept {
            kept.times.retain(|_, lines| !lines.is_empty());
            all.events.insert(uid.to_string(), kept);
        }
        if all == before {
            return Ok(false);
        }
        write(path, &all)?;
        Ok(true)
    })
}

/// The whole file written, under its lock (held by the caller).
fn write(path: &Path, all: &MeetingLines) -> Result<(), String> {
    let text = toml::to_string(all).map_err(|e| e.to_string())?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    let bytes = format!("# Sioul's own lines of meetings with others, by their UID and time (docs/client.md): kept here, never in the meeting.\n{text}");
    crate::filelock::replace(path, bytes.as_bytes()).map_err(|e| e.to_string())
}

/// An entry for `uid`, empty when none is there: this device knows the
/// meeting's lines, none of them (the door made it a meeting here from an
/// event that held none). Whether it changed.
pub fn ensure(path: &Path, uid: &str) -> Result<bool, String> {
    if uid.is_empty() {
        return Ok(false);
    }
    change(path, uid, |kept| {
        kept.get_or_insert_with(Kept::default);
    })
}

/// `uid`'s entry taken out: its lines went back into its file (it is no
/// meeting any more), or its file holds its own. Whether it changed.
pub fn forget(path: &Path, uid: &str) -> Result<bool, String> {
    change(path, uid, |kept| *kept = None)
}

/// Whether `path` is this file (`default_path`, or a file of its name):
/// what Undo puts back by entry (`put_back`), never whole.
pub fn is_store(path: &Path) -> bool {
    path == default_path() || path.file_name().is_some_and(|name| name == FILE)
}

/// The entries that differ between two versions of the file's text (None:
/// no file), each as it was in `before` and in `after`.
fn changed_entries(before: Option<&str>, after: Option<&str>) -> Vec<(String, Option<Kept>, Option<Kept>)> {
    let read = |text: Option<&str>| text.and_then(|t| toml::from_str::<MeetingLines>(t).ok()).unwrap_or_default();
    let (before, after) = (read(before), read(after));
    let uids: std::collections::BTreeSet<&String> = before.events.keys().chain(after.events.keys()).collect();
    uids.into_iter().filter(|uid| before.events.get(*uid) != after.events.get(*uid)).map(|uid| ((*uid).clone(), before.events.get(uid).cloned(), after.events.get(uid).cloned())).collect()
}

/// Whether the entries a change made (the file's text `before` and `after`
/// it) are still as it left them, whatever happened to the others since.
pub fn as_left(path: &Path, before: Option<&str>, after: Option<&str>) -> bool {
    let now = load(path);
    changed_entries(before, after).iter().all(|(uid, _, left)| now.events.get(uid) == left.as_ref())
}

/// A change taken back (Undo of a take or a kind on a meeting, third review
/// of the organizer's door, R9): each entry it changed put back as it was,
/// under the file's lock, while it is still as the change left it; another
/// meeting's entry changed since never stops it, nor is touched. Whether it
/// was put back.
pub fn put_back(path: &Path, before: Option<&str>, after: Option<&str>) -> Result<bool, String> {
    let entries = changed_entries(before, after);
    crate::filelock::with_lock(path, || {
        let mut all = load(path);
        if !entries.iter().all(|(uid, _, left)| all.events.get(uid) == left.as_ref()) {
            return Ok(false);
        }
        for (uid, was, _) in entries {
            match was {
                Some(kept) => all.events.insert(uid, kept),
                None => all.events.remove(&uid),
            };
        }
        write(path, &all)?;
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
    change(path, uid, |entry| {
        let kept = entry.get_or_insert_with(Kept::default);
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
/// kind), every time of it; an entry with none left stays, known empty.
/// Whether it changed.
pub fn set_from(path: &Path, uid: &str, text: &str) -> Result<bool, String> {
    let lines = crate::organizer::aside_of(text).lines;
    change(path, uid, |entry| {
        let kept = entry.get_or_insert_with(Kept::default);
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
/// line in its own time's event, before its alarms, in place of the lines
/// of the same name that event holds (an older holder line, an older kind);
/// the text as it is when nothing is kept.
pub fn merged_with(text: &str, kept: &Kept) -> String {
    put_in(text, kept, false)
}

/// An event's text with Sioul's other lines it holds (`organizer::kept_apart`)
/// replaced by what is kept of it here, time by time: the entry's word over
/// the file's (Undo of a guest's invitation puts back a file older than
/// what was answered since: third review of the organizer's door, R1).
pub fn folded(text: &str, kept: &Kept) -> String {
    put_in(text, kept, true)
}

/// `merged_with`, or `folded` (`whole`: every line of Sioul's of each event
/// replaced, not only those of the names kept).
fn put_in(text: &str, kept: &Kept, whole: bool) -> String {
    if kept.is_empty() && !whole {
        return text.to_string();
    }
    let source = lines::unfold(text);
    let mut out: Vec<String> = Vec::with_capacity(source.len() + 8);
    let (mut block, mut inside, mut nested, mut main_seen): (Vec<String>, bool, usize, bool) = (Vec::new(), false, 0, false);
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
            let main = time.is_empty() && !main_seen;
            main_seen |= main;
            let wanted: Vec<String> = kept.times.iter().filter(|(t, _)| same_time(t, &time)).flat_map(|(_, l)| l.iter().cloned()).collect();
            let names: Vec<String> = wanted.iter().map(|l| lines::raw_name(l)).collect();
            // The event's own level: before its first nested component (its alarms).
            let own_end = block.iter().enumerate().skip(1).find(|(_, l)| lines::name(l) == "BEGIN").map_or(block.len(), |(i, _)| i);
            let mut kept_block: Vec<String> = Vec::with_capacity(block.len() + wanted.len());
            for (i, l) in block.iter().enumerate() {
                let own = i > 0 && i < own_end;
                if own && (names.contains(&lines::raw_name(l)) || (whole && crate::organizer::kept_apart(l, main))) {
                    continue;
                }
                kept_block.push(l.clone());
            }
            let at = kept_block.iter().enumerate().skip(1).find(|(_, l)| lines::name(l) == "BEGIN").map_or(kept_block.len(), |(i, _)| i);
            kept_block.splice(at..at, wanted);
            kept_block.push(line.clone());
            out.append(&mut kept_block);
            continue;
        }
        block.push(line.clone());
    }
    let written = lines::fold(&out);
    if lines::unfold(&written) == source { text.to_string() } else { written }
}

/// An event's series' UID ("" none).
pub fn uid_of(text: &str) -> String {
    lines::unfold(text).iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()).unwrap_or_default()
}

/// A meeting's text with what this device keeps of it put back
/// (`merged_with`, from `default_path`), by its series' UID.
pub fn merged(text: &str) -> String {
    let uid = uid_of(text);
    if uid.is_empty() {
        return text.to_string();
    }
    match load(&default_path()).events.get(&uid) {
        Some(kept) => merged_with(text, kept),
        None => text.to_string(),
    }
}

/// An event as a reader of Sioul's other lines reads it on this device
/// (`read`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    /// Its text, with what is kept of it here put back.
    Known(String),
    /// A meeting this device keeps nothing of: who takes it and its kind may
    /// be kept on another device, and are not known here.
    Unknown,
}

/// An event's text as this device reads it, `store` being what it keeps
/// and `meeting` whether the event is a meeting with others now
/// (`agenda::with_others`): a meeting, with its entry put back
/// (`merged_with`), or `Read::Unknown` without one (R1); an event that is
/// no meeting, its file as it is, or, when it holds none of Sioul's other
/// lines and an entry is here (taken off its last guest elsewhere), with
/// the entry put back until they go back into its file (`forget`).
pub fn read(text: &str, store: &MeetingLines, meeting: bool) -> Read {
    let entry = store.events.get(&uid_of(text));
    match entry {
        Some(kept) if meeting => Read::Known(merged_with(text, kept)),
        None if meeting => Read::Unknown,
        Some(kept) if crate::organizer::aside_of(text).lines.is_empty() => Read::Known(merged_with(text, kept)),
        _ => Read::Known(text.to_string()),
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
        assert_eq!(load(&path).events.get("kept-1"), Some(&Kept::default()), "none left: known here, empty");
        assert!(forget(&path, "kept-1").unwrap() && !load(&path).events.contains_key("kept-1"), "forgotten");
        let _ = std::fs::remove_file(&path);
    }

    /// The third review of the organizer's door, R1 and R9. A child's
    /// meeting this device keeps nothing of reads as unknown (its lines may
    /// be on another device); with an entry, even empty, as known; an event
    /// that is no meeting any more, its lines read from here until they are
    /// back in its file, never over lines its file holds. Folded, the entry's
    /// word replaces the file's. A change of one entry is put back alone,
    /// under the lock, whatever another entry became meanwhile; never once
    /// that entry changed again.
    #[test]
    fn what_this_device_keeps_says_what_it_knows() {
        crate::config::stand_ins_only();
        let dir = crate::config::data_dir().join(format!("meeting-lines-r1-{}", std::process::id()));
        let path = dir.join(FILE);
        let _ = std::fs::remove_dir_all(&dir);
        let kind = "X-SIOUL-CONCEPT:tag:aurelienpierre.com,2026:sioul/child-kind/health".to_string();
        let held = "X-SIOUL-TAKEN-BY;CN=Alex;PARTSTAT=ACCEPTED:mailto:alex@example.org".to_string();
        let plain = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:psy-1\r\nDTSTART:20991110T090000Z\r\nSUMMARY:Tom: psychiatrist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let meeting = plain.replace("END:VEVENT", "ORGANIZER:mailto:alex@example.org\r\nATTENDEE:mailto:office@example.net\r\nEND:VEVENT");
        let empty = MeetingLines::default();
        assert_eq!(read(&meeting, &empty, true), Read::Unknown, "a meeting kept nowhere here: unknown");
        assert_eq!(read(plain, &empty, false), Read::Known(plain.to_string()), "an event: its file");
        assert!(ensure(&path, "psy-1").unwrap() && !ensure(&path, "psy-1").unwrap());
        assert_eq!(read(&meeting, &load(&path), true), Read::Known(meeting.clone()), "known here: nobody takes it, no kind");
        add(&path, "psy-1", &[(String::new(), kind.clone()), (String::new(), held.clone())]).unwrap();
        let Read::Known(merged) = read(&meeting, &load(&path), true) else { panic!("known") };
        assert!(merged.contains(&kind) && merged.contains(&held));
        // No meeting any more, its lines still here: read from here, until back in its file.
        assert_eq!(read(plain, &load(&path), false), Read::Known(merged_with(plain, &load(&path).events["psy-1"])));
        let own_lines = plain.replace("END:VEVENT", "X-SIOUL-TAKEN-BY;CN=Sam;PARTSTAT=ACCEPTED:mailto:sam@example.org\r\nEND:VEVENT");
        assert_eq!(read(&own_lines, &load(&path), false), Read::Known(own_lines.clone()), "its file holds its own: the file's word");
        // Folded: the entry's lines in place of the file's.
        let entry_wins = folded(&own_lines, &load(&path).events["psy-1"]);
        assert!(entry_wins.contains(&held) && entry_wins.contains(&kind) && !entry_wins.contains("sam@example.org"), "{entry_wins}");
        assert_eq!(folded(&own_lines, &Kept::default()), lines::fold(&lines::unfold(plain)), "an empty entry: none of Sioul's");
        // Undo of a take on the meeting (R9): its entry alone, under the lock.
        let before = std::fs::read_to_string(&path).ok();
        set_from(&path, "psy-1", &merged.replace("CN=Alex;PARTSTAT=ACCEPTED", "CN=Alex;PARTSTAT=DECLINED")).unwrap();
        let after = std::fs::read_to_string(&path).ok();
        add(&path, "other-meeting", &[(String::new(), kind.clone())]).unwrap();
        assert!(as_left(&path, before.as_deref(), after.as_deref()), "another meeting's entry changed: still as left");
        assert!(put_back(&path, before.as_deref(), after.as_deref()).unwrap());
        let now = load(&path);
        assert!(now.events["psy-1"].times[""].contains(&held) && now.events.contains_key("other-meeting"), "{now:?}");
        set_from(&path, "psy-1", &merged.replace("CN=Alex;PARTSTAT=ACCEPTED", "CN=Alex;PARTSTAT=DECLINED")).unwrap();
        let after = std::fs::read_to_string(&path).ok();
        set_from(&path, "psy-1", &merged).unwrap();
        assert!(!as_left(&path, before.as_deref(), after.as_deref()) && !put_back(&path, before.as_deref(), after.as_deref()).unwrap(), "changed again since: too late");
        assert!(is_store(&path) && !is_store(&dir.join("other.toml")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
