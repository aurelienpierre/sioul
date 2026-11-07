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
//! margins. `$XDG_DATA_HOME/sioul/meeting-lines.toml`. The sharing carries
//! it among your devices, sealed, by UID, time and line name
//! (`share::MEETING_LINES`, the part "time"), once every device says it has
//! the store: a kind set on one device and a holder on another both stay
//! (review of f85f504, M1), and two holders written apart keep the later. An
//! entry taken out travels as a removal. It never reaches a guest.
//!
//! Its readers take the meeting's text with these lines put back
//! (`merged`, `read`), as an older Sioul wrote them into the file, so that
//! every reader of those lines (the children's upkeep, its cards and
//! questions) reads a meeting as any event; its writers write here, never
//! into the meeting (`set_from`).
//!
//! **An entry is what this device knows** (third review of the organizer's
//! door, R1). Until an entry reaches this device (an older device held the
//! store back, the part "time" is off, the exchange has not come), a meeting
//! with no entry here may have its lines on another device: who takes it and
//! its kind are unknown here (`Read::Unknown`), and the children's upkeep
//! leaves it and its stand-ins as they are. An entry may hold no line: the door made the
//! meeting here from an event that held none (`ensure`). When an event is no
//! meeting any more, its lines go back into its file and its entry goes
//! (`back_into_file`, `forget`).
//!
//! **An entry can be ahead of its file** (review of f85f504, H1): the sharing
//! and the calendar's sync have separate triggers, so an entry may reach a
//! device before the meeting that made it. Each entry keeps that meeting's
//! DTSTAMP (`Kept::stamp`), and a file no later than it says nothing of guests
//! gone: nothing is written and nothing forgotten.

use crate::lines;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The file's name, in Sioul's data folder.
pub const FILE: &str = "meeting-lines.toml";

/// One meeting's own lines: by its times ("" the series, else that time's
/// RECURRENCE-ID line), then by the line's name (`lines::raw_name`), each name
/// its own entry in the sharing (a holder's two lines, who takes the child and
/// who brings them back, are one name).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "KeptAsWritten")]
pub struct Kept {
    /// The DTSTAMP of the meeting that made this entry (`stamp_of`): a file no
    /// later than it may simply be behind the entry (`later_than_entry`). ""
    /// an entry of no known age. Written even then, since it is what carries
    /// an entry of no line to your other devices. None only when another
    /// device's removal took it while a line was added here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stamp: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub times: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// A time's lines as the file holds them: by name, or a plain list, as the
/// store was written before its lines travelled one by one (6 November 2026).
#[derive(Deserialize)]
#[serde(untagged)]
enum TimeAsWritten {
    ByName(BTreeMap<String, Vec<String>>),
    Listed(Vec<String>),
}

/// An entry as the file holds it (`Kept`).
#[derive(Deserialize)]
struct KeptAsWritten {
    #[serde(default)]
    stamp: Option<String>,
    #[serde(default)]
    times: Option<BTreeMap<String, TimeAsWritten>>,
}

impl From<KeptAsWritten> for Kept {
    /// An entry of the older form has no age, and is an entry even of no line.
    fn from(written: KeptAsWritten) -> Kept {
        let older = written.stamp.is_none() && written.times.as_ref().is_some_and(|times| times.is_empty() || times.values().any(|t| matches!(t, TimeAsWritten::Listed(_))));
        let mut kept = Kept { stamp: if older { Some(String::new()) } else { written.stamp }, times: BTreeMap::new() };
        for (time, lines) in written.times.unwrap_or_default() {
            let names = kept.times.entry(time).or_default();
            match lines {
                TimeAsWritten::ByName(by_name) => names.extend(by_name),
                TimeAsWritten::Listed(listed) => {
                    for line in listed {
                        names.entry(lines::raw_name(&line)).or_default().push(line);
                    }
                }
            }
        }
        kept
    }
}

impl Kept {
    /// An entry of no line and no known age: this device knows the meeting holds none.
    pub fn known() -> Kept {
        Kept { stamp: Some(String::new()), times: BTreeMap::new() }
    }

    /// An entry holding these lines, each with its time, as `Aside::lines` gives them.
    pub fn of(given: impl IntoIterator<Item = (String, String)>) -> Kept {
        let mut kept = Kept::known();
        for (time, line) in given {
            kept.times.entry(time).or_default().entry(lines::raw_name(&line)).or_default().push(line);
        }
        kept
    }

    pub fn is_empty(&self) -> bool {
        self.times.values().all(|names| names.values().all(Vec::is_empty))
    }

    /// The lines kept for a time, by their names' order; none when it has none.
    pub fn lines(&self, time: &str) -> Vec<String> {
        self.times.get(time).map(|names| names.values().flatten().cloned().collect()).unwrap_or_default()
    }

    /// Without the names and times left empty (a removal that came leaves their tables behind).
    fn tidied(mut self) -> Kept {
        for names in self.times.values_mut() {
            names.retain(|_, lines| !lines.is_empty());
        }
        self.times.retain(|_, names| !names.is_empty());
        self
    }

    /// Its age raised to `stamp` ("" leaves it): a later meeting's, never an earlier one's.
    fn stamped(&mut self, stamp: &str) {
        match &self.stamp {
            Some(there) if there.as_str() >= stamp => {}
            _ => self.stamp = Some(stamp.to_string()),
        }
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

/// The file as it is; empty when there is none, or it does not read. What a
/// removal that came left behind (the tables of the lines it took out) is no entry.
pub fn load(path: &Path) -> MeetingLines {
    read_text(std::fs::read_to_string(path).ok().as_deref())
}

/// A text of the file (None: no file) as `load` reads it.
fn read_text(text: Option<&str>) -> MeetingLines {
    let mut all: MeetingLines = text.and_then(|text| toml::from_str(text).ok()).unwrap_or_default();
    all.events = std::mem::take(&mut all.events).into_iter().map(|(uid, kept)| (uid, kept.tidied())).filter(|(_, kept)| kept.stamp.is_some() || !kept.is_empty()).collect();
    all
}

/// The file written again in its present form, when it holds anything: what
/// the sharing reads once the store opens is never the older form
/// (`share::meeting_lines_shared`).
pub fn in_present_form(path: &Path) -> Result<(), String> {
    crate::filelock::with_lock(path, || {
        let all = load(path);
        if all.events.is_empty() { Ok(()) } else { write(path, &all) }
    })
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
        if let Some(kept) = kept {
            all.events.insert(uid.to_string(), kept.tidied());
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
        kept.get_or_insert_with(Kept::known);
    })
}

/// `uid`'s entry, when it has one, stamped with the DTSTAMP of the meeting it
/// is kept for (`stamp_of`, `Kept::stamp`). Whether it changed.
pub fn stamped(path: &Path, uid: &str, stamp: &str) -> Result<bool, String> {
    change(path, uid, |kept| {
        if let Some(kept) = kept {
            kept.stamped(stamp);
        }
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
    let (before, after) = (read_text(before), read_text(after));
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
        let kept = entry.get_or_insert_with(Kept::known);
        let mut given: BTreeMap<&str, BTreeMap<String, Vec<String>>> = BTreeMap::new();
        for (time, line) in lines {
            let named = given.entry(time.as_str()).or_default().entry(lines::raw_name(line)).or_default();
            if !named.contains(line) {
                named.push(line.clone());
            }
        }
        for (time, names) in given {
            let there = kept.times.entry(time.to_string()).or_default();
            for (name, new) in names {
                // The same lines in another order: kept as they were.
                let sorted = |mut v: Vec<String>| {
                    v.sort();
                    v
                };
                if there.get(&name).is_none_or(|old| sorted(old.clone()) != sorted(new.clone())) {
                    there.insert(name, new);
                }
            }
        }
    })
}

/// What is kept of `uid` replaced by the Sioul lines of `text` (its file
/// with them put back, then changed by a reader's writer: who takes it, its
/// kind), every time of it, stamped with its DTSTAMP; an entry with none left
/// stays, known empty. Whether it changed.
pub fn set_from(path: &Path, uid: &str, text: &str) -> Result<bool, String> {
    let lines = crate::organizer::aside_of(text).lines;
    let stamp = stamp_of(text);
    change(path, uid, |entry| {
        let kept = entry.get_or_insert_with(Kept::known);
        kept.times = Kept::of(lines).times;
        kept.stamped(&stamp);
    })
}

/// A meeting's stamp: the latest DTSTAMP of its events ("" none), as a file
/// is compared with the entry that meeting made (`later_than_entry`).
pub fn stamp_of(text: &str) -> String {
    let source = lines::unfold(text);
    source.iter().filter(|l| lines::name(l) == "DTSTAMP").map(|l| lines::value(l).trim().to_string()).max().unwrap_or_default()
}

/// Whether an event's file is later than the meeting its entry was kept for:
/// only then may its having no guest be read as its guests gone. A file no
/// later may simply be behind the entry, the sharing having come before its
/// calendar's sync (review of f85f504, H1); an entry of no known age says
/// nothing of it.
pub fn later_than_entry(text: &str, kept: &Kept) -> bool {
    kept.stamp.as_deref().is_some_and(|stamp| !stamp.is_empty() && stamp_of(text).as_str() > stamp)
}

/// Sioul's own lines of an item that is no meeting any more, kept in the
/// store at `at` (`store` as read from it), back into its file
/// (`read` reads them from there meanwhile), name by name: a line its file
/// holds of a name wins (a holder written on another device since), the entry
/// fills the names it lacks (the kind this device kept: review of round 4,
/// R1-a); then its entry is forgotten, never before, and that removal travels.
/// Nothing for a meeting, an item with no entry, a file no later than its
/// entry (`later_than_entry`), or a change here its server has not agreed on
/// yet, which was made on what this device held and says nothing of guests
/// gone. Whether its file was written.
pub fn back_into_file(path: &Path, at: &Path, store: &MeetingLines) -> bool {
    let Ok(file) = std::fs::read_to_string(path) else { return false };
    let uid = uid_of(&file);
    let Some(kept) = store.events.get(&uid) else { return false };
    if crate::agenda::with_others(&file) || !later_than_entry(&file, kept) {
        return false;
    }
    if crate::vdir::agreed_hash(path).is_some_and(|agreed| agreed != crate::vdir::content_hash(file.as_bytes())) {
        return false;
    }
    let back = filled(&file, kept);
    if back != file
        && let Err(e) = crate::vdir::write_item(path, &back)
    {
        eprintln!("Sioul: an event kept its lines apart: {e}");
        return false;
    }
    let _ = forget(at, &uid);
    back != file
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

/// An event that is no meeting any more with what is kept of it here put
/// back name by name (review of round 4, R1-a): its file's own line wins
/// where it has one of that name for that time, the entry fills the names
/// it lacks. A holder line written on another device meanwhile is kept, and
/// so is the kind this device kept.
pub fn filled(text: &str, kept: &Kept) -> String {
    let source = lines::unfold(text);
    let mut events: Vec<(String, Vec<String>)> = Vec::new();
    let (mut inside, mut nested, mut time, mut names): (bool, usize, String, Vec<String>) = (false, 0, String::new(), Vec::new());
    for line in &source {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        if !inside {
            if name == "BEGIN" && value == "VEVENT" {
                (inside, nested, time, names) = (true, 0, String::new(), Vec::new());
            }
            continue;
        }
        match name.as_str() {
            "BEGIN" => nested += 1,
            "END" if nested > 0 => nested -= 1,
            "END" => {
                inside = false;
                events.push((std::mem::take(&mut time), std::mem::take(&mut names)));
            }
            "RECURRENCE-ID" if nested == 0 => time = line.clone(),
            _ if nested == 0 => names.push(lines::raw_name(line)),
            _ => {}
        }
    }
    // What each time lacks of the entry's names: those lines alone, merged in.
    let mut lacking = Kept::default();
    for (t, wanted) in &kept.times {
        for (time, names) in events.iter().filter(|(time, _)| same_time(t, time)) {
            for (name, lines) in wanted.iter().filter(|(name, _)| !names.contains(*name)) {
                lacking.times.entry(time.clone()).or_default().entry(name.clone()).or_default().extend(lines.iter().cloned());
            }
        }
    }
    put_in(text, &lacking, false)
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
            let wanted: Vec<String> = kept.times.iter().filter(|(t, _)| same_time(t, &time)).flat_map(|(_, names)| names.values().flatten().cloned()).collect();
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
/// (`merged_with`); without one, as its file says when the file holds
/// Sioul's lines itself (who takes it, its kind: review of round 4, R1-b),
/// else `Read::Unknown` (R1). An event that is no meeting, its file as it
/// is, or with an entry here (taken off its last guest elsewhere) the
/// entry filling what its file lacks, name by name (`filled`, R1-a), until
/// it is back in its file (`forget`).
pub fn read(text: &str, store: &MeetingLines, meeting: bool) -> Read {
    let entry = store.events.get(&uid_of(text));
    match entry {
        Some(kept) if meeting => Read::Known(merged_with(text, kept)),
        None if meeting && !holds_own(text) => Read::Unknown,
        Some(kept) => Read::Known(filled(text, kept)),
        None => Read::Known(text.to_string()),
    }
}

/// Whether an event's file holds Sioul's other lines itself (who takes it,
/// its kind: `organizer::aside_of`).
pub fn holds_own(text: &str) -> bool {
    !crate::organizer::aside_of(text).lines.is_empty()
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
        let series = load(&path).events["kept-1"].lines("");
        assert!(series.contains(&newer) && !series.contains(&held) && series.contains(&kind), "{series:?}");
        set_from(&path, "kept-1", &merged_with(meeting, &Kept::of([(String::new(), kind.clone())]))).unwrap();
        assert!(load(&path).events["kept-1"].times.len() == 1 && load(&path).events["kept-1"].lines("") == [kind.clone()], "set from a text: the whole entry");
        set_from(&path, "kept-1", meeting).unwrap();
        assert!(load(&path).events.get("kept-1").is_some_and(Kept::is_empty), "none left: known here, empty");
        assert!(forget(&path, "kept-1").unwrap() && !load(&path).events.contains_key("kept-1"), "forgotten");
        let _ = std::fs::remove_file(&path);
    }

    /// The review of f85f504, H1 (`probe_a1`, turned here and in
    /// `children`): an entry that reached this device ahead of its calendar.
    /// The file, no later than the meeting that made the entry, says nothing
    /// of guests gone: nothing written, nothing forgotten. An entry of no known
    /// age, the same. Only a later file brings its lines back, and forgets it.
    #[test]
    fn an_entry_ahead_of_its_file_is_never_forgotten() {
        crate::config::stand_ins_only();
        let dir = crate::config::data_dir().join(format!("meeting-lines-h1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (store, file) = (dir.join(FILE), dir.join("psy.ics"));
        let kind = "X-SIOUL-CONCEPT:tag:aurelienpierre.com,2026:sioul/child-kind/health".to_string();
        let event = |stamp: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:h1-psy\r\nDTSTAMP:{stamp}\r\nDTSTART:20991110T090000Z\r\nSUMMARY:Tom: psychiatrist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
        add(&store, "h1-psy", &[(String::new(), kind.clone())]).unwrap();
        let stale = event("20261105T090000Z");
        std::fs::write(&file, &stale).unwrap();
        assert!(!back_into_file(&file, &store, &load(&store)), "an entry of no known age");
        assert!(load(&store).events.contains_key("h1-psy") && std::fs::read_to_string(&file).unwrap() == stale);
        stamped(&store, "h1-psy", "20261106T120000Z").unwrap();
        assert!(!stamped(&store, "h1-psy", "20261101T000000Z").unwrap(), "never an earlier one's");
        assert!(!back_into_file(&file, &store, &load(&store)), "older than the entry");
        std::fs::write(&file, event("20261106T120000Z")).unwrap();
        assert!(!back_into_file(&file, &store, &load(&store)), "the meeting's own stamp: no later");
        assert!(load(&store).events.contains_key("h1-psy") && !std::fs::read_to_string(&file).unwrap().contains(&kind));
        std::fs::write(&file, event("20261107T080000Z")).unwrap();
        assert!(back_into_file(&file, &store, &load(&store)), "later: its guests went");
        assert!(!load(&store).events.contains_key("h1-psy") && std::fs::read_to_string(&file).unwrap().contains(&kind));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The store as it was written before its lines travelled one by one
    /// reads whole, an entry of no line included, of no known age; written in
    /// its present form it reads the same. What a removal that came leaves
    /// behind (the tables of the lines it took out) is no entry.
    #[test]
    fn the_older_form_reads_and_hollow_tables_are_no_entry() {
        crate::config::stand_ins_only();
        let dir = crate::config::data_dir().join(format!("meeting-lines-form-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = dir.join(FILE);
        let kind = "X-SIOUL-CONCEPT:tag:aurelienpierre.com,2026:sioul/child-kind/health";
        let held = "X-SIOUL-TAKEN-BY;CN=Alex;PARTSTAT=ACCEPTED:mailto:alex@example.org";
        std::fs::write(&store, format!("[event.\"old-1\".times]\n\"\" = [\"{kind}\", \"{held}\"]\n\n[event.\"old-empty\".times]\n")).unwrap();
        let read = load(&store);
        assert_eq!(read.events["old-1"].lines("").len(), 2, "{read:?}");
        assert_eq!(read.events["old-1"].stamp.as_deref(), Some(""), "of no known age");
        assert!(read.events["old-empty"].is_empty() && read.events["old-empty"].stamp.is_some(), "an entry of no line: {read:?}");
        in_present_form(&store).unwrap();
        assert_eq!(load(&store), read, "{}", std::fs::read_to_string(&store).unwrap());
        assert!(std::fs::read_to_string(&store).unwrap().contains("X-SIOUL-CONCEPT = ["), "by name now");
        std::fs::write(&store, "[event.\"gone\".times.\"\"]\n\n[event.\"gone-too\"]\n\n[event.\"left\".times.\"\"]\nX-SIOUL-CONCEPT = [\"X-SIOUL-CONCEPT:x\"]\n").unwrap();
        let read = load(&store);
        assert_eq!(read.events.keys().map(String::as_str).collect::<Vec<_>>(), ["left"], "{read:?}");
        let _ = std::fs::remove_dir_all(&dir);
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
        let Read::Known(both) = read(&own_lines, &load(&path), false) else { panic!("known") };
        assert!(both.contains("sam@example.org") && !both.contains(&held) && both.contains(&kind), "its file's holder, the kind kept here (R1-a): {both}");
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
        assert!(now.events["psy-1"].lines("").contains(&held) && now.events.contains_key("other-meeting"), "{now:?}");
        set_from(&path, "psy-1", &merged.replace("CN=Alex;PARTSTAT=ACCEPTED", "CN=Alex;PARTSTAT=DECLINED")).unwrap();
        let after = std::fs::read_to_string(&path).ok();
        set_from(&path, "psy-1", &merged).unwrap();
        assert!(!as_left(&path, before.as_deref(), after.as_deref()) && !put_back(&path, before.as_deref(), after.as_deref()).unwrap(), "changed again since: too late");
        assert!(is_store(&path) && !is_store(&dir.join("other.toml")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- The review of round 4 (ab79db0): its probes turned.

    /// R1-a: device A keeps Tom's psychiatrist's kind (health) and holder
    /// (Alex), its stand-in masked in Alex's agenda for work; on device B the
    /// office's line went and Alex took it there (a holder line alone in the
    /// file). A reads it name by name: B's holder line, A's kind, so the
    /// stand-in stays masked; its lines back in the file the same way, the
    /// kind kept (the probe `review4_r1_a_take_on_the_other_device_…`).
    #[test]
    fn a_holder_written_elsewhere_keeps_the_kind_kept_here() {
        use crate::holders::{self, Act, Answer, ChildItem, Context, Holder, Kind, Mine, Taken, Target};
        let zone = jiff::tz::TimeZone::UTC;
        let own = vec!["alex@example.org".to_string()];
        let collection = crate::vdir::Collection { kind: crate::vdir::Kind::Calendars, account: "family".into(), id: "tom".into(), dir: std::path::PathBuf::from("/nowhere/family/tom"), name: "Tom".into(), color: None, read_only: false, components: vec!["VEVENT".into(), "VTODO".into()] };
        let said = crate::agendas::Said { whose: Some("child".into()), name: Some("Tom".into()), ..Default::default() };
        let tom = crate::agendas::Agenda::of(&collection, &said, &crate::config::Config::default(), false, &|| true);
        let work = Target { place: "me/work".into(), dir: std::path::PathBuf::from("/nowhere/me/work"), work: true };
        let target = |_: Option<Kind>| Some(work.clone());
        let details = |_: &crate::agendas::Agenda| "Details: Tom’s agenda".to_string();
        let follow_up = |_: &crate::agendas::Agenda, _: Kind| None;
        let now = 1_793_500_000;
        let cx = Context { own: &own, target: &target, details: &details, away: "Away", follow_up: &follow_up, now, zone: &zone };
        let child = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:psy-r4\r\nDTSTAMP:20261101T090000Z\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Tom: psychiatrist\r\nLOCATION:Dr Martin\\, 12 rue des Lilas\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let alex = Holder { name: "Alex".into(), address: "alex@example.org".into(), answer: Answer::Accepted, since: Some(now), back: false };
        let held = holders::with_kind(&holders::with_taken(child, None, &Taken { to: Some(alex.clone()), back: None }, &zone).unwrap(), Some(Kind::Health)).unwrap();
        let path = std::path::PathBuf::from("/nowhere/family/tom/psy-r4.ics");
        let acts = holders::upkeep(&[ChildItem { path: &path, text: &held, agenda: &tom }], &[], &cx);
        let [Act::Made { path: stand_in_path, text: stand_in, .. }] = acts.as_slice() else { panic!("{acts:?}") };
        let meeting = held.replace("END:VEVENT", "ORGANIZER;CN=Alex:mailto:alex@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:office@example.net\r\nEND:VEVENT");
        let (file, aside) = crate::organizer::set_aside(&meeting);
        let store_a = MeetingLines { events: [("psy-r4".to_string(), Kept::of(aside.lines.clone()))].into() };
        let me = crate::organizer::Guest { address: "alex@example.org".into(), name: "Alex".into() };
        let off = crate::organizer::with_guests(&file, &me, &[], crate::organizer::Sender::Draft).unwrap();
        let b_written = holders::with_taken(&off, None, &Taken { to: Some(alex), back: None }, &zone).unwrap();
        let Read::Known(on_a) = read(&b_written, &store_a, false) else { panic!("known") };
        assert_eq!((holders::kind_of(&on_a), holders::taken(&on_a, None, &zone).to.map(|h| h.address)), (Some(Kind::Health), Some("alex@example.org".to_string())));
        let mine = [Mine { path: stand_in_path.as_path(), text: stand_in.as_str(), place: "me/work", work: true }];
        assert_eq!(holders::upkeep(&[ChildItem { path: &path, text: &on_a, agenda: &tom }], &mine, &cx), [], "the stand-in stays masked");
        assert_eq!(filled(&b_written, &store_a.events["psy-r4"]), on_a, "back into its file the same way");
    }

    /// R1-b: a child's meeting whose file holds who takes it and its kind
    /// (another organizer's, written before) reads as its file says with no
    /// entry here; one holding neither, with no entry here, stays unknown.
    #[test]
    fn a_meeting_whose_file_holds_its_lines_is_known() {
        let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:office-1\r\nDTSTAMP:20261101T090000Z\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Tom: check-up\r\nORGANIZER:mailto:office@example.net\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:alex@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!(read(text, &MeetingLines::default(), true), Read::Unknown, "nothing here, nothing in its file");
        let zone = jiff::tz::TimeZone::UTC;
        let alex = crate::holders::Holder { name: "Alex".into(), address: "alex@example.org".into(), answer: crate::holders::Answer::Accepted, since: Some(1), back: false };
        let held = crate::holders::with_kind(&crate::holders::with_taken(text, None, &crate::holders::Taken { to: Some(alex), back: None }, &zone).unwrap(), Some(crate::holders::Kind::Health)).unwrap();
        assert_eq!(read(&held, &MeetingLines::default(), true), Read::Known(held.clone()), "its file says it");
    }
}
