// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Every device of yours in one table (docs/database.md, "Devices"): a row
//! per device, this one first, for Settings ▸ Devices and `sioul devices`.
//!
//! A row says the three hands a device's record is in:
//! - **what the device says of itself**, in its sealed entry
//!   (`devices::Entry`): its system's name, what it is (a phone, a tablet, a
//!   computer), when it last opened, when it last read the others and wrote
//!   its own, its Sioul, the parts it shares;
//! - **what you said of it**, in the shared settings (`[device."<id>"]`,
//!   `sioul_core::device_said`): the name you give it, what it is for;
//! - **what this device knows of it** (`peers::Peers`, the window's
//!   `health::know`): in use, closed, silent, off as you said, an older
//!   Sioul, an entry that does not read, a clock ahead.
//!
//! `table` makes the rows from facts its caller read (`Facts`): the window
//! gives what it just learned, the command line what the window last kept
//! (`read`), said as of then. With sharing off the table is this device
//! alone. Every sentence is worded here, so that both say the same.
//!
//! Nothing is decided by a device's area here: it is shown, as you said it.

use crate::devices::{COMPUTER, Entry, PHONE, TABLET};
use crate::peers::Peers;
use jiff::tz::TimeZone;
use serde::Serialize;
use sioul_core::device_said::Said;
use sioul_core::health::{Named, Peer};
use sioul_core::i18n::Translator;
use std::collections::BTreeMap;
use std::path::Path;

/// What a device listed by the sharing holds back (`share::formats`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holding {
    /// The newer form of what travels: it is known by what it shared alone.
    Format,
    /// An older Sioul, once the newer form travels.
    Older,
    /// Your own time around meetings, kept on each device until it is
    /// updated or forgotten (re-check of 857c39e, §3.2).
    OwnTime,
}

/// How long a phone says nothing before a computer offers to forget it now
/// (`Row::forget_now`): a day, where a living phone restates its entry
/// every hour (`devices::RESTATED`).
pub const FORGET_NOW_AFTER: i64 = 86_400;

/// A device's row. The words are for people, in the caller's language; the
/// codes, times and lists beside them are for programs (`sioul devices --json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Row {
    /// Its id in the sharing (`share::Here::id`).
    pub id: String,
    /// This device.
    pub here: bool,
    /// The name to show: yours when given, else as its system names it.
    pub name: String,
    /// The name you gave it; "" while unsaid.
    pub given: String,
    /// As its system names it, in words: a computer's host name, "The phone
    /// (its model)", "Another device of yours" when nothing says.
    pub system: String,
    /// What it is: "phone", "tablet", "computer"; "" when its entry does not say.
    pub form: String,
    /// The same in words: "A phone".
    pub form_words: String,
    /// How it is now, as a code: "in-use", "in-use-then", "closed", "quiet",
    /// "silent", "off", "ahead", "older", "unread", "holds".
    pub code: String,
    /// How it is now, in words.
    pub state: String,
    /// Said under it, quietly: it does not share its doses; its clock ahead.
    pub notes: Vec<String>,
    /// When it last opened (its entry's `started`, its clock, Unix seconds); 0 when not known.
    pub started: i64,
    /// The same in words; "" when not known.
    pub opened: String,
    /// When it last read the others, and last wrote its own (its clock); 0 when not known.
    pub imported: i64,
    pub exported: i64,
    /// When it last shared, in words; "" when its entry does not say.
    pub shared: String,
    /// Its Sioul as its entry says it: "0.0.3 (eff8661abcde)"; "" when it does not say.
    pub version: String,
    /// Its Sioul is older than this device's.
    pub older: bool,
    /// Its Sioul in words: "Sioul 0.0.3 (eff8661abcde).", said calmly when
    /// older than this device's; "" when its entry does not say.
    pub build: String,
    /// What you said it is for: "work", "admin+leisure"; "" while unsaid (everything).
    pub area: String,
    /// The same in words: "Work · Leisure"; unsaid, "Everything comes here".
    pub area_words: String,
    /// The parts it shares, as its entry says them (`share::PARTS`); none
    /// when it shares none, or when its Sioul does not say.
    pub parts: Vec<String>,
    /// The same in words; when it says none, why.
    pub parts_words: String,
    /// Counted as off, as you said: "Count it again".
    pub off: bool,
    /// Silent for a week: "Forget this device".
    pub forget: bool,
    /// On a computer, a phone silent for a day or more, still counted:
    /// "Forget it now", said with what it changes.
    pub forget_now: bool,
    /// Its Sioul follows what devices are for (`devices::Entry::areas`):
    /// its cells are in force. This device always does (`device_grid::grid`).
    pub follows: bool,
    /// Its five cells, one per time: what comes forward on it then, in
    /// words (`device_grid::grid` sets them; none before, and none for a
    /// device whose Sioul is older).
    pub cells: Vec<crate::device_grid::Cell>,
    /// Said in the cells' place for a device whose Sioul is older: "Its
    /// Sioul is older: everything still comes there."; "" otherwise.
    pub cells_older: String,
}

/// What the table is made from: read by its caller, at `now`.
#[derive(Debug, Clone)]
pub struct Facts<'a> {
    /// This device's id in the sharing.
    pub id: &'a str,
    /// This device's own entry, as it last wrote it (`devices::own`), its
    /// name and form filled by the caller when it never wrote one.
    pub own: &'a Entry,
    /// Sharing is on here: a folder set, its key held.
    pub sharing: bool,
    /// The parts this device shares now (`share::PARTS`), sharing on.
    pub parts: &'a [String],
    /// The other devices as known here, for the doses; none with sharing off.
    pub peers: &'a [Peer],
    /// What was kept of each (`peers.json`): its newest entry, its claims, your word on it here.
    pub kept: &'a Peers,
    /// The devices the sharing lists as holding something back, with when
    /// each was last heard (seconds, 0 when nothing says it).
    pub holders: &'a [(String, i64, Holding)],
    /// What you said of each device (`Config::devices`).
    pub said: &'a BTreeMap<String, Said>,
    /// This device is a computer by its system (`devices::COMPUTER`): it offers "Forget it now".
    pub computer: bool,
    /// Now, this device's clock (Unix seconds).
    pub now: i64,
    /// When this device last looked at the others, when that is not now: the
    /// command line reading what the window kept. The others are then said
    /// as of that moment, never as silent since.
    pub looked: Option<i64>,
    /// The time zone times are said in.
    pub zone: TimeZone,
}

/// A message with its words given.
fn said(words: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = sioul_core::i18n::args();
    for (key, value) in pairs {
        args.set(key.to_string(), value.clone());
    }
    words.text(id, Some(&args))
}

/// "Wednesday": a word's first letter in capitals, as a title starts.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// When things are said: now, and the zone their times are said in.
struct Clock<'a> {
    now: i64,
    zone: &'a TimeZone,
    words: &'a Translator,
}

impl Clock<'_> {
    fn zoned(&self, at: i64) -> Option<jiff::Zoned> {
        jiff::Timestamp::from_second(at).ok().map(|t| t.to_zoned(self.zone.clone()))
    }

    fn today(&self, at: &jiff::Zoned) -> bool {
        self.zoned(self.now).is_some_and(|now| now.date() == at.date())
    }

    /// A moment inside a sentence: "at 07:45" today, "on Monday 5 October at 07:45" before.
    fn sentence(&self, at: i64) -> String {
        let Some(at) = self.zoned(at) else { return String::new() };
        if self.today(&at) { said(self.words, "when-at", &[("time", at.strftime("%H:%M").to_string())]) } else { self.words.when(&at) }
    }

    /// A moment in a cell: "14:05" today, "Fri 2 Oct 14:05" before; "" for none.
    fn short(&self, at: i64) -> String {
        if at <= 0 {
            return String::new();
        }
        let Some(at) = self.zoned(at) else { return String::new() };
        if self.today(&at) { at.strftime("%H:%M").to_string() } else { self.words.date(&at, true) }
    }
}

/// A device as the doses name it: "the phone", else its name; one whose
/// name is not known (an older Sioul on a phone says "localhost"), "another
/// device of yours"; at a sentence's start.
fn device_start(device: &Named, words: &Translator) -> String {
    let word = if device.phone {
        words.text("device-the-phone", None)
    } else if device.name.trim().is_empty() || device.name == "localhost" || device.name == "?" {
        words.text("share-other-device", None)
    } else {
        device.name.clone()
    };
    if word == device.name { word } else { capitalized(&word) }
}

/// A device as its entry names it: "The phone (its model)", "The tablet
/// (its model)" for an Android that says its model; none for any other.
fn android_named(entry: &Entry, words: &Translator) -> Option<String> {
    (entry.kind == PHONE && !entry.name.is_empty() && entry.name != PHONE).then(|| format!("{} ({})", capitalized(&words.text(if entry.form == TABLET { "device-the-tablet" } else { "device-the-phone" }, None)), entry.name))
}

/// What a device is, in words: "A phone"; "" for a word that names no kind.
fn form_words(form: &str, words: &Translator) -> String {
    match form {
        PHONE | TABLET | COMPUTER => words.text(&format!("set-device-kind-{form}"), None),
        _ => String::new(),
    }
}

/// "Closed since 14:02.", from an entry that says it closed; "Closed." from
/// one that says no time at all (written by something else than Sioul).
fn closed(entry: &Entry, clock: &Clock) -> String {
    // Its close when it says one (a phone put away still exchanges after it), else the latest it says.
    let since = if entry.closed > 0 { entry.closed } else { entry.exported.max(entry.started) };
    match clock.short(since) {
        when if when.is_empty() => clock.words.text("devices-now-closed-unknown", None),
        when => said(clock.words, "devices-now-closed", &[("when", when)]),
    }
}

/// What an entry says of itself, set into its row: what it is, when it last
/// opened and shared, its Sioul, its parts.
fn of_entry(row: &mut Row, entry: &Entry, clock: &Clock) {
    let words = clock.words;
    row.form = entry.form_or_kind().to_string();
    row.form_words = form_words(&row.form, words);
    row.started = entry.started;
    row.opened = clock.short(entry.started);
    row.imported = entry.imported;
    row.exported = entry.exported;
    let (read, wrote) = (clock.short(entry.imported), clock.short(entry.exported));
    row.shared = match (read.is_empty(), wrote.is_empty()) {
        (true, true) => words.text("devices-shared-never", None),
        (false, false) if read == wrote => read,
        (false, false) => said(words, "devices-shared-both", &[("read", read), ("wrote", wrote)]),
        (false, true) => said(words, "devices-shared-read", &[("read", read)]),
        (true, false) => said(words, "devices-shared-wrote", &[("wrote", wrote)]),
    };
    // The build it runs (docs/database.md, "Devices"): an older one said, calmly.
    row.version = entry.build();
    row.older = sioul_core::build::older(&entry.version, sioul_core::build::VERSION);
    row.build = match (row.version.is_empty(), row.older) {
        (true, _) => String::new(),
        (false, true) => said(words, "share-device-build-older", &[("build", row.version.clone()), ("here", sioul_core::build::DESCRIBED.to_string())]),
        (false, false) => said(words, "share-device-build", &[("build", row.version.clone())]),
    };
    row.parts = entry.parts.clone();
    row.parts_words = parts_words(&entry.parts, words).unwrap_or_else(|| words.text("devices-parts-unsaid", None));
    row.follows = entry.areas >= crate::devices::AREAS;
}

/// The parts' names, in the order the sharing lists them; none when the list is empty.
fn parts_words(parts: &[String], words: &Translator) -> Option<String> {
    let named: Vec<String> = crate::share::PARTS.iter().filter(|part| parts.iter().any(|p| p == *part)).map(|part| words.text(&format!("share-part-{part}"), None)).collect();
    (!named.is_empty()).then(|| named.join(", "))
}

/// Your other devices, as this one knows them: how each is, in words, with
/// what can be done; nothing red, no counts. `at`: the moment they are said
/// as of (now, or this device's last look); `live`: that moment is now;
/// `computer`: this device is one, and offers "Forget it now".
fn others(peers: &[Peer], kept: &Peers, at: i64, live: bool, computer: bool, clock: &Clock) -> Vec<Row> {
    let words = clock.words;
    let when = |at: i64| clock.sentence(at);
    peers
        .iter()
        // Forgotten here, or replaced by another device (`apply_replaced`): not listed until it says anything newer.
        .filter(|peer| !peer.id.is_empty() && !peer.forgotten && kept.peers.get(&peer.id).is_none_or(|seen| seen.forgotten.is_none()))
        .map(|peer| {
            let seen = kept.peers.get(&peer.id);
            let entry = seen.and_then(|s| s.entry.as_ref()).filter(|_| peer.said.is_some());
            let named = Named { id: peer.id.clone(), name: peer.name.clone(), phone: peer.said.as_ref().is_some_and(|s| s.phone) };
            let system = entry.and_then(|e| android_named(e, words)).unwrap_or_else(|| device_start(&named, words));
            let off = peer.off;
            let counted = sioul_core::health::counts(&Peer { off: false, ..peer.clone() }, at);
            // Its entry from a clock far ahead of this one's, set aside (`FUTURE`): said, so that its clock is set right.
            let future = peer.set_aside;
            let (code, state) = if off {
                ("off", words.text("share-device-off", None))
            } else if future && entry.is_none() {
                ("ahead", words.text("share-device-future", None))
            } else if !counted {
                ("silent", said(words, "share-device-silent", &[("when", when(peer.heard))]))
            } else {
                match entry {
                    // Not looked at since: in use when this device last looked, never "silent since".
                    Some(e) if e.working && !live => ("in-use-then", words.text("devices-now-in-use-then", None)),
                    Some(e) if e.working && at - peer.seen > sioul_core::health::QUIET => ("quiet", said(words, "devices-now-quiet", &[("when", clock.short(e.exported.max(e.started)))])),
                    Some(e) if e.working => ("in-use", words.text("devices-now-in-use", None)),
                    Some(e) => ("closed", closed(e, clock)),
                    None if seen.is_some_and(|s| s.renewed > 0) => ("older", said(words, "share-device-older", &[("when", when(peer.heard))])),
                    None => ("unread", words.text("share-device-unread", None)),
                }
            };
            let mut notes = Vec::new();
            if entry.is_some_and(|e| !e.doses) {
                notes.push(words.text("share-device-apart", None));
            }
            if future && (off || entry.is_some()) {
                notes.push(words.text("share-device-future", None));
            } else if peer.ahead >= 60 {
                let mut args = sioul_core::i18n::args();
                args.set("minutes", peer.ahead / 60);
                notes.push(words.text("share-device-ahead", Some(&args)));
            }
            // Forgotten once silent a week, or once said off (a phone that never gives its word holds the computers until then: L2).
            let forget = !counted || off;
            // On a computer, a phone silent a day (its entry not changed here since: a living
            // phone restates it each hour): "Forget it now", rather than a week's wait.
            let phone = peer.said.as_ref().is_some_and(|s| s.phone) || entry.is_some_and(|e| e.kind == PHONE);
            let forget_now = !forget && phone && computer && at - peer.seen.max(peer.heard) >= FORGET_NOW_AFTER;
            let mut row = Row { id: peer.id.clone(), name: system.clone(), system, code: code.to_string(), state, notes, off, forget, forget_now, ..Row::default() };
            match entry {
                Some(entry) => of_entry(&mut row, entry, clock),
                None => row.parts_words = words.text("devices-parts-unsaid", None),
            }
            row
        })
        .collect()
}

/// Every device holding the newer form back has its line, so that the window
/// says which (docs/database.md, "The format of what travels"): one the rows
/// do not list (known to the sharing alone, by what it shared before, or
/// forgotten here while the sharing heard it since) is added, with "Forget
/// this device": it is not counted for your doses, so forgetting it makes no
/// dose look known. One listed keeps its own line: forgotten only once silent
/// for a week, as any device, since a device counted for your doses and
/// forgotten could make a dose it took look not taken. One that keeps your
/// own time around meetings from travelling says so in one sentence, under
/// its own line when it has one (re-check of 857c39e, §3.2).
fn with_holders(rows: &mut Vec<Row>, holders: &[(String, i64, Holding)], clock: &Clock) {
    let words = clock.words;
    let when = |at: i64| clock.sentence(at);
    for (id, heard, holds) in holders {
        if id.is_empty() {
            continue;
        }
        if let Some(row) = rows.iter_mut().find(|row| row.id == *id) {
            let note = words.text("share-device-own-time", None);
            if *holds == Holding::OwnTime && !row.notes.contains(&note) {
                row.notes.push(note);
            }
            continue;
        }
        let state = match (holds, *heard > 0) {
            (Holding::Format, true) => said(words, "share-device-holds", &[("when", when(*heard))]),
            (Holding::Format, false) => words.text("share-device-holds-unknown", None),
            (Holding::Older, true) => said(words, "share-device-older", &[("when", when(*heard))]),
            (Holding::Older, false) => words.text("share-device-unread", None),
            (Holding::OwnTime, true) => said(words, "share-device-own-time-heard", &[("when", when(*heard))]),
            (Holding::OwnTime, false) => words.text("share-device-own-time-unknown", None),
        };
        let system = capitalized(&words.text("share-other-device", None));
        rows.push(Row { id: id.clone(), name: system.clone(), system, code: "holds".into(), state, forget: true, parts_words: words.text("devices-parts-unsaid", None), ..Row::default() });
    }
}

/// This device's own row: its entry as it last wrote it, the parts it
/// shares now, this build.
fn here(facts: &Facts, clock: &Clock) -> Row {
    let words = clock.words;
    let own = facts.own;
    let named = Named { id: facts.id.to_string(), name: own.name.clone(), phone: false };
    let system = android_named(own, words).unwrap_or_else(|| if own.name.trim().is_empty() { words.text("devices-this", None) } else { device_start(&named, words) });
    let (code, state) = if own.working || own.started == 0 { ("in-use", words.text("devices-now-in-use", None)) } else { ("closed", closed(own, clock)) };
    let mut row = Row { id: facts.id.to_string(), here: true, name: system.clone(), system, code: code.to_string(), state, ..Row::default() };
    of_entry(&mut row, own, clock);
    // This build, whatever its entry last said.
    row.version = sioul_core::build::DESCRIBED.to_string();
    row.older = false;
    row.build = said(words, "share-device-build", &[("build", row.version.clone())]);
    if facts.sharing {
        row.parts = crate::share::PARTS.iter().filter(|part| facts.parts.iter().any(|p| p == *part)).map(|part| part.to_string()).collect();
        row.parts_words = parts_words(&row.parts, words).unwrap_or_else(|| words.text("devices-parts-none", None));
    } else {
        (row.imported, row.exported) = (0, 0);
        row.shared = words.text("devices-shared-off", None);
        row.parts = Vec::new();
        row.parts_words = String::new();
    }
    row
}

/// What a device is for, in words: "Work · Leisure"; unsaid, "Everything comes here".
fn area_words(area: Option<sioul_core::areas::Area>, words: &Translator) -> String {
    match area {
        None => words.text("devices-for-all", None),
        Some(area) => [(area.work, "area-work"), (area.admin, "area-admin"), (area.leisure, "area-leisure")].iter().filter(|(on, _)| *on).map(|(_, id)| words.text(id, None)).collect::<Vec<_>>().join(" · "),
    }
}

/// The table: this device first, then the others by the name shown. With
/// sharing off, this device alone.
pub fn table(facts: &Facts, words: &Translator) -> Vec<Row> {
    let clock = Clock { now: facts.now, zone: &facts.zone, words };
    let mut rest = if facts.sharing {
        let at = facts.looked.unwrap_or(facts.now);
        let mut rows = others(facts.peers, facts.kept, at, facts.looked.is_none(), facts.computer, &clock);
        with_holders(&mut rows, facts.holders, &clock);
        rows.retain(|row| row.id != facts.id);
        rows
    } else {
        Vec::new()
    };
    let mut rows = vec![here(facts, &clock)];
    rows.append(&mut rest);
    for row in &mut rows {
        let word = facts.said.get(&row.id);
        if let Some(given) = word.and_then(Said::name) {
            row.given = given.to_string();
            row.name = given.to_string();
        }
        let area = word.and_then(Said::area);
        row.area = area.map(sioul_core::areas::Area::id).unwrap_or_default();
        row.area_words = area_words(area, words);
    }
    rows[1..].sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
    rows
}

/// The table in plain text, for the command line: each device a few lines,
/// its name first; an empty line between two.
pub fn plain(rows: &[Row], words: &Translator) -> Vec<String> {
    let mut out = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push(String::new());
        }
        // Its name, with what its system calls it when you named it otherwise.
        let renamed = !row.given.is_empty() && row.system != row.given;
        let names = [("name", row.name.clone()), ("system", row.system.clone())];
        out.push(match (row.here, renamed) {
            (true, true) => said(words, "devices-plain-here-system", &names),
            (true, false) => said(words, "devices-plain-here", &names),
            (false, true) => said(words, "devices-plain-system", &names),
            (false, false) => row.name.clone(),
        });
        let line = |label: &str, value: &str| (!value.is_empty()).then(|| format!("  {}", said(words, "devices-plain-line", &[("label", words.text(label, None)), ("value", value.to_string())])));
        let now = [row.state.clone()].into_iter().chain(row.notes.iter().cloned()).collect::<Vec<_>>().join(" ");
        out.extend(line("devices-col-form", &row.form_words));
        out.extend(line("devices-col-now", &now));
        out.extend(line("devices-col-opened", &row.opened));
        out.extend(line("devices-col-shared", &row.shared));
        // Its Sioul: the build alone, or the sentence that says it is older.
        out.extend(if row.older { Some(format!("  {}", row.build)) } else { line("devices-col-sioul", &row.version) });
        out.extend(line("devices-col-for", &row.area_words));
        // What comes forward on it at each time (`device_grid::grid`), a line per time; an older Sioul, one sentence.
        out.extend(row.cells.iter().map(|cell| format!("    {}", cell.line)));
        out.extend((!row.cells_older.is_empty()).then(|| format!("    {}", row.cells_older)));
        out.extend(line("devices-col-parts", &row.parts_words));
        out.extend(line("devices-col-id", &row.id));
    }
    out
}

/// Whether Settings names the devices that keep your own time around
/// meetings from travelling: while this device shares its time, and either
/// some is set here, or another device says it reads it, so may hold some to
/// send here (review of e5bd27b, later 1: the device that needs the sentence
/// is the one that has none of its own).
pub fn own_time_listed(shares_time: bool, set_here: bool, f: &crate::share::Formats) -> bool {
    shares_time && (set_here || f.others_share_own_time)
}

/// The devices the sharing lists as holding something back (`share::formats`),
/// for the table's rows: the format's holders, those on an older Sioul once
/// it travels, and, while some of your own time around meetings is set
/// (`own_time_set`), those that keep it from travelling; each with when it
/// was last heard (seconds).
pub fn holding(f: &crate::share::Formats, own_time_set: bool) -> Vec<(String, i64, Holding)> {
    let own_time = f.own_time.iter().filter(|_| own_time_set).map(|id| (id, Holding::OwnTime));
    f.holders.iter().map(|(id, _)| (id, Holding::Format)).chain(f.older.iter().map(|id| (id, Holding::Older))).chain(own_time).map(|(id, holds)| (id.clone(), f.heard.get(id).copied().unwrap_or(0) / 1000, holds)).collect()
}

/// What the table is made from, read from this device's own files, without
/// a window: its id, its entry as it last wrote it, what the window last
/// kept of the others (`peers.json`), and, the folder's key given, the
/// devices the sharing lists besides. Nothing is written but this device's
/// id, made once (`share::Here::load`).
#[derive(Debug, Default)]
pub struct Read {
    pub id: String,
    pub own: Entry,
    /// A sharing folder is set on this device.
    pub sharing: bool,
    pub peers: Vec<Peer>,
    pub kept: Peers,
    pub holders: Vec<(String, i64, Holding)>,
    /// When the others were last looked at, when that is more than two
    /// minutes ago: no window looks now.
    pub looked: Option<i64>,
}

/// How long after a look what was kept still counts as seen now: the
/// window looks each minute while it runs.
const LOOKS_NOW: i64 = 2 * 60;

/// `Read`, from Sioul's state folder; `key`: the sharing folder's, when the
/// keyring gives it. Without it, a folder being set, the others are listed
/// as the window kept them, without the devices the sharing alone knows.
pub fn read(state: &Path, config: &sioul_core::config::Config, key: Option<&[u8; 32]>, now: i64) -> Read {
    let here = crate::share::Here::load(state);
    let mut own = crate::devices::own(&crate::devices::own_path(state));
    // Never opened in a window here: named and said as its system does.
    if own.name.is_empty() {
        own.name = crate::lease::host_name();
    }
    if own.kind.is_empty() {
        own.kind = if cfg!(target_os = "android") { PHONE } else { COMPUTER }.to_string();
    }
    if own.form.is_empty() {
        own.form = sioul_core::device_kind::DeviceKind::of(config.device_kind.as_deref(), cfg!(target_os = "android"), None).word().to_string();
    }
    let Some(folder) = here.folder_path() else { return Read { id: here.id, own, ..Read::default() } };
    let kept = Peers::load(&Peers::path(state));
    let mut peers: Vec<Peer> = kept.peers.values().map(|seen| seen.peer.clone()).collect();
    let mut holders = Vec::new();
    if let Some(key) = key {
        let memory = state.join("share").join("memory.json");
        // A device in the folder this last week that the window never learned (its
        // seal not arrived, a damaged file): listed all the same, as the window lists it.
        let silent = sioul_core::health::SILENT_DAYS * 86_400;
        for other in crate::share::others_heard(&folder, key, &here.id).into_iter().filter(|o| !kept.peers.contains_key(&o.id) && now - o.heard < silent) {
            peers.push(Peer { id: other.id, heard: other.heard, ..Peer::default() });
        }
        let (_, unread) = crate::devices::all(&folder, key);
        for id in unread.into_iter().filter(|id| *id != here.id && !kept.peers.contains_key(id)) {
            if !peers.iter().any(|p| p.id == id) {
                peers.push(Peer { id, heard: now, ..Peer::default() });
            }
        }
        let formats = crate::share::formats(&folder, key, &memory, &here.id, now.saturating_mul(1000));
        let listed = own_time_listed(here.shares("time", config), !sioul_core::own_time::current().events.is_empty(), &formats);
        holders = holding(&formats, listed);
    }
    let looked = (kept.looked > 0 && now - kept.looked > LOOKS_NOW).then_some(kept.looked);
    Read { id: here.id, own, sharing: true, peers, kept, holders, looked }
}

impl Read {
    /// The facts of the table, with what you said of each device.
    pub fn facts<'a>(&'a self, said: &'a BTreeMap<String, Said>, now: i64, zone: TimeZone) -> Facts<'a> {
        Facts { id: &self.id, own: &self.own, sharing: self.sharing, parts: &self.own.parts, peers: &self.peers, kept: &self.kept, holders: &self.holders, said, computer: self.own.kind == COMPUTER, now, looked: self.looked, zone }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::{Mark, PeerSeen};
    use sioul_core::health::Said as Session;

    const NOW: i64 = 1_800_000_000;

    fn utc() -> TimeZone {
        TimeZone::UTC
    }

    fn entry(id: &str, kind: &str, name: &str, working: bool, exported: i64, doses: bool) -> Entry {
        Entry { id: id.into(), name: name.into(), kind: kind.into(), started: NOW - 7_200, closed: if working { 0 } else { exported + 5 }, working, imported: exported, exported, doses, ..Entry::default() }
    }

    fn add(kept: &mut Peers, entry: Entry, seen: i64, ahead: i64) {
        let peer = Peer { id: entry.id.clone(), name: entry.name.clone(), said: Some(entry.said()), complete: true, seen, heard: entry.exported, ahead, ..Peer::default() };
        kept.peers.insert(entry.id.clone(), PeerSeen { peer, entry: Some(entry), ..PeerSeen::default() });
    }

    fn peers_of(kept: &Peers) -> Vec<Peer> {
        kept.peers.values().map(|s| s.peer.clone()).collect()
    }

    fn own() -> Entry {
        Entry { id: "here".into(), name: "desk".into(), kind: COMPUTER.into(), form: COMPUTER.into(), version: sioul_core::build::VERSION.into(), started: NOW - 3_600, working: true, imported: NOW - 60, exported: NOW - 60, doses: true, ..Entry::default() }
    }

    /// Your other devices in words: how each is, with what can be done;
    /// nothing red, no counts. Every case the sharing panel worded: in use,
    /// closed, in use and silent since, silent a week, off as you said, an
    /// older Sioul known by its claims, and the notes under them.
    #[test]
    fn your_other_devices_in_words() {
        let english = Translator::new("en");
        let zone = utc();
        let clock = Clock { now: NOW, zone: &zone, words: &english };
        let mut kept = Peers::default();
        add(&mut kept, entry("a-phone", "phone", "Moss", false, NOW - 600, true), NOW - 590, 240);
        add(&mut kept, entry("b-desk", "computer", "desk", true, NOW - 30, true), NOW - 25, 0);
        add(&mut kept, entry("c-laptop", "computer", "laptop", true, NOW - 4 * 3_600, false), NOW - 4 * 3_600, 0);
        add(&mut kept, entry("d-old", "computer", "old", true, NOW - 9 * 86_400, true), NOW - 9 * 86_400, 0);
        add(&mut kept, entry("j-tower", "computer", "tower", true, NOW - 30, true), NOW - 25, 0);
        kept.peers.insert("e-older".into(), PeerSeen { peer: Peer { id: "e-older".into(), name: "tower".into(), heard: NOW - 120, ..Peer::default() }, renewed: NOW - 120, ..PeerSeen::default() });
        kept.peers.insert("k-unread".into(), PeerSeen { peer: Peer { id: "k-unread".into(), heard: NOW - 120, ..Peer::default() }, ..PeerSeen::default() });
        kept.peers.get_mut("b-desk").unwrap().peer.off = true;
        let rows = others(&peers_of(&kept), &kept, NOW, true, true, &clock);
        let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!((row("a-phone").name.as_str(), row("a-phone").form.as_str(), row("a-phone").form_words.as_str()), ("The phone (Moss)", "phone", "A phone"));
        assert!(row("a-phone").code == "closed" && row("a-phone").state.starts_with("Closed since ") && row("a-phone").notes == ["Its clock is 4 minutes ahead of this one's, at least."], "{:?}", row("a-phone"));
        assert!(row("b-desk").off && row("b-desk").code == "off" && row("b-desk").state == "Counted as off, as you said, until it shares again.");
        assert!(row("c-laptop").code == "quiet" && row("c-laptop").state.starts_with("Silent since ") && row("c-laptop").state.ends_with(": it may have stopped without closing.") && row("c-laptop").notes == ["It does not share its doses."], "{:?}", row("c-laptop"));
        assert!(row("d-old").forget && row("d-old").code == "silent" && row("d-old").state.starts_with("Silent since ") && row("d-old").state.ends_with("no longer counted for your doses."), "{:?}", row("d-old"));
        assert!(row("e-older").code == "older" && row("e-older").state.starts_with("An older Sioul: last heard "), "{:?}", row("e-older"));
        assert_eq!((row("j-tower").code.as_str(), row("j-tower").state.as_str()), ("in-use", "In use now."));
        assert_eq!((row("k-unread").code.as_str(), row("k-unread").state.as_str(), row("k-unread").name.as_str()), ("unread", "What it says does not read here yet.", "Another device of yours"));
        assert!(rows.iter().all(|r| !r.state.contains("not taken")));
        // What each entry says of itself: when it opened and shared, its Sioul.
        assert_eq!((row("a-phone").started, row("a-phone").exported, row("a-phone").shared.as_str()), (NOW - 7_200, NOW - 600, clock.short(NOW - 600).as_str()));
        assert!(!row("a-phone").opened.is_empty() && row("e-older").opened.is_empty() && row("e-older").shared.is_empty());
        // The devices holding the newer form of what travels back: each has its
        // line; one not listed is added, with Forget; one listed keeps its own.
        let mut rows = rows;
        with_holders(&mut rows, &[("e-older".into(), NOW - 120, Holding::Format), ("f-gone".into(), NOW - 90 * 86_400, Holding::Format), ("g-unknown".into(), 0, Holding::Format), ("h-after".into(), NOW - 3_600, Holding::Older)], &clock);
        assert_eq!(rows.iter().filter(|r| r.id == "e-older").count(), 1);
        let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert!(!row("e-older").forget, "counted for the doses: forgotten once silent a week only");
        assert!(row("f-gone").forget && row("f-gone").name == "Another device of yours" && row("f-gone").state.starts_with("Known here by what it shared, last "), "{:?}", row("f-gone"));
        assert!(row("h-after").forget && row("h-after").state.starts_with("An older Sioul: last heard "), "{:?}", row("h-after"));
        assert!(row("g-unknown").forget && row("g-unknown").state == "Known here by what it shared: until it is updated there, or forgotten here, your devices keep sharing in the form an older Sioul reads.", "{:?}", row("g-unknown"));
        // The devices keeping your own time around meetings from travelling (re-check of 857c39e,
        // §3.2): one listed says it under its own line, keeping its own Forget; one not listed is
        // added with Forget; one already added for the format says it once more, under it.
        let mut rows = rows;
        with_holders(&mut rows, &[("d-old".into(), NOW - 9 * 86_400, Holding::OwnTime), ("i-tablet".into(), NOW - 3_600, Holding::OwnTime), ("f-gone".into(), NOW - 90 * 86_400, Holding::OwnTime)], &clock);
        let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        let sentence = "Until it is updated there, or forgotten here, the time you keep around meetings stays on each of your devices.";
        assert!(row("d-old").forget && row("d-old").notes == [sentence], "{:?}", row("d-old"));
        let tablet = row("i-tablet");
        assert!(tablet.forget && tablet.name == "Another device of yours" && tablet.state.starts_with("Known here by what it shared, last heard ") && tablet.state.ends_with(": until it is updated there, or forgotten here, the time you keep around meetings stays on each of your devices."), "{tablet:?}");
        assert!(row("f-gone").notes == [sentence] && rows.iter().filter(|r| r.id == "f-gone").count() == 1, "{:?}", row("f-gone"));
        for r in &rows {
            let words = format!("{} {}", r.state, r.notes.join(" ")).to_lowercase();
            assert!(!words.contains("error") && !words.contains("blocked"), "calm: {r:?}");
        }
        // A clock far ahead, its entry set aside and none kept: said, so that it is set right; one kept says it under its line.
        let mut ahead = Peers::default();
        ahead.peers.insert("m-ahead".into(), PeerSeen { peer: Peer { id: "m-ahead".into(), name: "ahead".into(), said: Some(Session::default()), set_aside: true, seen: NOW, heard: NOW, ..Peer::default() }, ..PeerSeen::default() });
        add(&mut ahead, entry("n-ahead", "computer", "later", false, NOW - 600, true), NOW - 590, 0);
        ahead.peers.get_mut("n-ahead").unwrap().peer.set_aside = true;
        let rows = others(&peers_of(&ahead), &ahead, NOW, true, true, &clock);
        let future = "Its clock runs more than ten minutes ahead of this one's: what it says is set aside here until its clock is set right.";
        assert_eq!((rows[0].code.as_str(), rows[0].state.as_str()), ("ahead", future));
        assert!(rows[1].code == "closed" && rows[1].notes == [future], "{:?}", rows[1]);
        // Forgotten here: not listed until it says anything newer.
        ahead.peers.get_mut("n-ahead").unwrap().forgotten = Some(Mark::default());
        assert!(others(&peers_of(&ahead), &ahead, NOW, true, true, &clock).iter().all(|r| r.id != "n-ahead"));
    }

    /// On a computer, a phone silent for a day or more is offered "Forget it
    /// now" (a dead phone could hold every dose back a week); one heard
    /// within the day, or already offered "Forget this device", is not; nor
    /// is a computer; nor any on a phone.
    #[test]
    fn a_phone_silent_a_day_is_offered_to_be_forgotten_now() {
        let english = Translator::new("en");
        let zone = utc();
        let clock = Clock { now: NOW, zone: &zone, words: &english };
        let peer = |id: &str, phone: bool, seen: i64| Peer {
            id: id.into(),
            name: id.into(),
            said: Some(Session { started: seen - 60, closed: seen, working: false, exported: seen, imported: seen, doses: true, left: false, phone }),
            complete: true,
            seen,
            heard: seen,
            ..Peer::default()
        };
        let peers = [peer("silent-phone", true, NOW - FORGET_NOW_AFTER - 60), peer("fresh-phone", true, NOW - 3_600), peer("silent-desk", false, NOW - FORGET_NOW_AFTER - 60), peer("week-phone", true, NOW - 8 * 86_400)];
        let rows = others(&peers, &Peers::default(), NOW, true, true, &clock);
        let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert!(row("silent-phone").forget_now && !row("silent-phone").forget);
        assert!(!row("fresh-phone").forget_now);
        assert!(!row("silent-desk").forget_now, "a computer is not offered it");
        assert!(row("week-phone").forget && !row("week-phone").forget_now, "silent a week: Forget this device, as before");
        let on_a_phone = others(&peers, &Peers::default(), NOW, true, false, &clock);
        assert!(on_a_phone.iter().all(|r| !r.forget_now), "offered on a computer only");
    }

    /// The table: this device first, whatever its name, then the others by
    /// the name shown; each with the name you gave it and what you said it
    /// is for, unsaid being everything; the parts each entry says, an entry
    /// that says none read as an older Sioul's; with sharing off, this
    /// device alone, its line still holding what you said of it.
    #[test]
    fn the_table_lists_this_device_first_with_what_you_said_of_each() {
        let english = Translator::new("en");
        let mut kept = Peers::default();
        let phone = Entry { form: PHONE.into(), version: "0.0.1".into(), parts: vec!["health".into(), "settings".into()], ..entry("a-phone", "phone", "Moss", false, NOW - 600, true) };
        add(&mut kept, phone, NOW - 590, 0);
        add(&mut kept, Entry { imported: NOW - 90_000, ..entry("b-laptop", "computer", "zeta", true, NOW - 30, true) }, NOW - 25, 0);
        let peers = peers_of(&kept);
        let own = own();
        let parts = vec!["time".to_string(), "settings".to_string()];
        let said: BTreeMap<String, Said> = [
            ("here".to_string(), Said { name: Some("Work desk".into()), area: Some("work".into()), ..Said::default() }),
            ("b-laptop".to_string(), Said { name: Some("Attic laptop".into()), area: Some("admin+leisure".into()), ..Said::default() }),
            ("gone".to_string(), Said { name: Some("Old".into()), area: None, ..Said::default() }),
        ]
        .into_iter()
        .collect();
        let facts = Facts { id: "here", own: &own, sharing: true, parts: &parts, peers: &peers, kept: &kept, holders: &[], said: &said, computer: true, now: NOW, looked: None, zone: utc() };
        let rows = table(&facts, &english);
        assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["here", "b-laptop", "a-phone"], "this device first, then by the name shown");
        let (here, laptop, phone) = (&rows[0], &rows[1], &rows[2]);
        assert!(here.here && !laptop.here && !phone.here);
        assert_eq!((here.name.as_str(), here.given.as_str(), here.system.as_str(), here.form_words.as_str()), ("Work desk", "Work desk", "desk", "A computer"));
        assert_eq!((here.area.as_str(), here.area_words.as_str(), here.code.as_str(), here.state.as_str()), ("work", "Work", "in-use", "In use now."));
        assert_eq!((here.parts.clone(), here.parts_words.as_str()), (vec!["settings".to_string(), "time".to_string()], "Settings and accounts, Time"), "in the sharing's own order");
        assert_eq!((here.version.as_str(), here.older), (sioul_core::build::DESCRIBED, false));
        assert_eq!((laptop.name.as_str(), laptop.given.as_str(), laptop.system.as_str()), ("Attic laptop", "Attic laptop", "zeta"));
        assert_eq!((laptop.area.as_str(), laptop.area_words.as_str()), ("admin+leisure", "Your admin · Leisure"));
        assert!(laptop.parts.is_empty() && laptop.parts_words == "Its Sioul does not say what it shares.", "{laptop:?}");
        assert!(laptop.shared.starts_with("Read the others ") && laptop.shared.contains("; wrote its own "), "two times when they differ: {}", laptop.shared);
        assert_eq!((phone.name.as_str(), phone.given.as_str(), phone.area.as_str(), phone.area_words.as_str()), ("The phone (Moss)", "", "", "Everything comes here"));
        assert_eq!((phone.parts.clone(), phone.parts_words.as_str()), (vec!["health".to_string(), "settings".to_string()], "Settings and accounts, Health"));
        assert!(phone.older && phone.build.starts_with("Sioul 0.0.1, older than this device's"), "{phone:?}");
        assert!(rows.iter().all(|r| r.id != "gone"), "a word on a device not known here makes no row");
        // As JSON, for the window and `sioul devices --json`: the names of the fields.
        let json = serde_json::to_value(&rows).unwrap();
        assert_eq!((json[0]["here"].as_bool(), json[0]["area"].as_str(), json[2]["parts"][0].as_str(), json[1]["code"].as_str()), (Some(true), Some("work"), Some("health"), Some("in-use")));
        // In plain text: each device a few lines, its name first.
        let text = plain(&rows, &english).join("\n");
        assert!(text.starts_with("Work desk (this device, named desk by its system)\n  What it is: A computer\n  Now: In use now.\n"), "{text}");
        assert!(text.contains("\n\nAttic laptop (named zeta by its system)\n") && text.contains("\n  For: Your admin · Leisure\n") && text.contains("\n\nThe phone (Moss)\n") && text.contains("\n  For: Everything comes here\n") && text.contains("\n  Shares: Settings and accounts, Health\n") && text.contains("\n  Id: a-phone") && text.contains("\n  Sioul 0.0.1, older than this device's"), "{text}");
        assert!(text.contains(&format!("\n  Sioul: {}\n", sioul_core::build::DESCRIBED)), "{text}");
        // Sharing off: this device alone, what you said of it kept, nothing shared said so.
        let alone = table(&Facts { sharing: false, ..facts.clone() }, &english);
        assert_eq!(alone.len(), 1);
        assert_eq!((alone[0].name.as_str(), alone[0].area.as_str(), alone[0].shared.as_str(), alone[0].parts.len(), alone[0].parts_words.as_str()), ("Work desk", "work", "This device does not share.", 0, ""));
        // Sharing on with every part switched off here: said, not taken for an older Sioul.
        let none = table(&Facts { parts: &[], ..facts.clone() }, &english);
        assert_eq!(none[0].parts_words, "Nothing travels from this device.");
        // From the command line with no window looking: the others as of the
        // last look, one in use then never said silent since; this device closed.
        let closed = Entry { working: false, closed: NOW - 1_800, ..own.clone() };
        let later = NOW + 5 * 3_600;
        let stale = table(&Facts { own: &closed, now: later, looked: Some(NOW), ..facts.clone() }, &english);
        assert_eq!(stale[0].code, "closed");
        assert!(stale[0].state.starts_with("Closed since "), "{:?}", stale[0]);
        assert_eq!((stale[1].code.as_str(), stale[1].state.as_str()), ("in-use-then", "In use when this device last looked."));
        let live = table(&Facts { now: later, ..facts.clone() }, &english);
        assert_eq!(live[1].code, "quiet", "looked at now, hours without a change: it may have stopped");
        // In French: a kind is « type », the words there.
        let french = Translator::new("fr");
        let rows = table(&facts, &french);
        assert_eq!((rows[0].state.as_str(), rows[2].area_words.as_str(), rows[0].form_words.as_str()), ("Utilisé en ce moment.", "Tout arrive ici", "Un ordinateur"));
        assert!(plain(&rows, &french).iter().all(|line| !line.to_lowercase().contains("genre")));
    }

    /// Read without a window: this device named and said as its system does
    /// when it never wrote its entry, alone while no folder is set; with a
    /// folder, the others as the window last kept them, said as of its last
    /// look when that is not now.
    #[test]
    fn the_command_line_reads_what_the_window_kept() {
        let dir = std::env::temp_dir().join(format!("sioul-device-table-read-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state = dir.join("state");
        std::fs::create_dir_all(state.join("share")).unwrap();
        let config = sioul_core::config::Config::default();
        let alone = read(&state, &config, None, NOW);
        assert!(!alone.id.is_empty() && !alone.sharing && alone.peers.is_empty() && alone.looked.is_none());
        assert!(!alone.own.name.is_empty() && alone.own.form == COMPUTER && alone.own.kind == COMPUTER, "{:?}", alone.own);
        let rows = table(&alone.facts(&config.devices, NOW, utc()), &Translator::new("en"));
        assert_eq!((rows.len(), rows[0].here, rows[0].shared.as_str()), (1, true, "This device does not share."));
        // A folder set, the window's knowledge kept ten minutes ago, its key held.
        let mut here = crate::share::Here::load(&state);
        here.folder = Some(dir.join("shared").display().to_string());
        here.save(&state).unwrap();
        std::fs::create_dir_all(dir.join("shared")).unwrap();
        let mut kept = Peers { looked: NOW - 600, ..Peers::default() };
        add(&mut kept, entry("a-phone", "phone", "Moss", true, NOW - 630, true), NOW - 620, 0);
        std::fs::write(Peers::path(&state), serde_json::to_string(&kept).unwrap()).unwrap();
        crate::devices::change(&crate::devices::own_path(&state), None, |e| {
            e.id = here.id.clone();
            e.name = "desk".into();
            e.kind = COMPUTER.into();
            e.parts = vec!["settings".into()];
            e.start(NOW - 3_600);
            e.close(NOW - 900);
        })
        .unwrap();
        let key = [7u8; 32];
        let with = read(&state, &config, Some(&key), NOW);
        assert!(with.sharing && with.looked == Some(NOW - 600) && with.peers.len() == 1 && with.holders.is_empty(), "{with:?}");
        let rows = table(&with.facts(&config.devices, NOW, utc()), &Translator::new("en"));
        assert_eq!(rows.iter().map(|r| (r.id.as_str(), r.code.as_str())).collect::<Vec<_>>(), [(here.id.as_str(), "closed"), ("a-phone", "in-use-then")]);
        assert_eq!(rows[0].parts, ["settings"], "this device's parts as it last wrote them");
        // Its key not read: the others as kept all the same.
        let without = read(&state, &config, None, NOW);
        assert!(without.sharing && without.peers.len() == 1 && without.holders.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
