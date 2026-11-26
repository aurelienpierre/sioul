// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The agendas' door (docs/extensions.md, "Where a timetable goes"): an
//! agenda an extension brought, a pupil's timetable, written into a calendar
//! of the person's own that can be written, chosen by them, which its server
//! then carries to every device and to whoever it is shared with.
//!
//! - **One writer**: every event goes through the agenda's own door for
//!   events (`vdir::write_item`, `vdir::remove_item`, under `agenda::guard`),
//!   the same a person's own change takes; the ordinary sync sends it. No
//!   file of an event is written any other way.
//! - **Marked three ways**: its UID made from the extension's id, the
//!   agenda's and the lesson's own (`super::event_uid`: hashes, safe
//!   whatever those ids hold, the same at every refresh and on every
//!   device); the category of the extension's name (`super::category`);
//!   a last line of its description saying where it comes from. Sioul also
//!   keeps on it a hash of the lesson as the extension gave it (`HASH_LINE`),
//!   and in this device's state (`noted_path`) in case a server drops that line.
//! - **Only marked events are touched**: an event is the extension's by the
//!   form of its UID alone (`super::uid_mark`), never by its title, its time
//!   or its category; whatever else the calendar holds is never read as the
//!   extension's, never changed, never removed.
//! - **At each refresh**, for the period the extension says it read
//!   (`AgendaReply::read`), its unread days aside: a lesson not there is
//!   added (one deleted by hand comes back); one whose hash changed is
//!   written again; one unchanged is left as it is, edits made by someone
//!   else included; a cancelled one stays, said cancelled in its title and by
//!   `STATUS:CANCELLED`; a marked event of that period no longer in the
//!   answer is removed. A period not read, or an answer without any event,
//!   removes nothing (absence proves nothing). Nothing is written when nothing differs.
//! - **Busy or free**: a lesson is written `TRANSP:OPAQUE` when the person
//!   counts this agenda as their own busy time, `TRANSP:TRANSPARENT`
//!   otherwise. The extension only suggests a default (`AgendaReply::busy`:
//!   Planity true, a pupil's Skolengo false); the person's per-agenda choice
//!   (`[extensions.busy]`, resolved before the door is called; written by its
//!   calendar's switch in Your agendas) decides. A lesson held with the other
//!   mark is written again at the next refresh, whatever its hash says, so
//!   that a choice made on a device that does not run the extension reaches
//!   it. An own event written OPAQUE reaches Sioul's real busy
//!   (`busy::holds_you`); nothing in `busy.rs` changes.
//! - **Its categories**: the extension's name first (`super::category`), then
//!   the event's own as the extension gave them (a salon's services), on one
//!   `CATEGORIES` line. Nothing recognises an extension's event by its
//!   categories: its UID alone does.
//! - **Tied to the card of the person it is with** (`contact_line`): when the
//!   extension names, on an event, a person of a set of contacts it hands
//!   back in the same reply (`AgendaReply::ties`), and that person has a card
//!   (the contacts' door's, `cards::uids_by_mark`, read after the sets are
//!   written), the event carries
//!   `CONTACT;ALTREP="urn:uuid:<UID>":<name>` (RFC 5545 §3.8.4.2: the
//!   card's UID, the person's name), never a guest. The tie
//!   is part of the lesson's hash, so a card bound again, merged into another
//!   or removed writes the event again, re-pointed or without the line.

use super::reply::AgendaReply;
use crate::{agenda, lines, vdir};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The line Sioul keeps on each event it wrote for an extension: a hash of
/// the lesson as the extension gave it, its stamps aside.
pub const HASH_LINE: &str = "X-SIOUL-EXTENSION-HASH";
/// The most one event's file weighs once marked.
pub const MOST_FILE_BYTES: usize = 256 << 10;
/// The most lessons of one extension this device notes (`noted_path`).
pub const MOST_NOTED: usize = 100_000;
/// A file of a calendar larger than this is never read as one of an extension's.
const READ_MOST: u64 = 1 << 20;
/// The stamps a lesson's hash leaves aside: they may change when nothing else does.
const STAMPS: [&str; 4] = ["DTSTAMP", "LAST-MODIFIED", "CREATED", agenda::ORGANIZER_DTSTAMP];

/// Why an agenda was not written: one word, said by the window in its own
/// sentence and in the extension's log, never with anything the extension wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// The calendar chosen is gone, or can no longer be written (read only,
    /// for tasks alone, a subscribed one): nothing written; choose another.
    CalendarGone,
    /// An event, or the whole agenda once marked, weighs more than it may.
    TooLarge,
    /// The agenda's calendar does not read as one.
    Unreadable,
}

impl Problem {
    pub fn word(self) -> &'static str {
        match self {
            Self::CalendarGone => "calendar-gone",
            Self::TooLarge => "too-large",
            Self::Unreadable => "unreadable",
        }
    }
}

/// What the door writes in the person's language: a cancelled lesson's
/// title made from its own, and the description's last line.
pub struct Words<'a> {
    pub cancelled: &'a dyn Fn(&str) -> String,
    pub from: String,
}

/// A lesson ready to be written: its UID and file name (Sioul's), its text
/// marked, its hash as the extension gave it, its first times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lesson {
    pub uid: String,
    pub file: String,
    pub text: String,
    pub hash: String,
    pub start: i64,
    pub end: i64,
}

/// The lessons of an agenda, each an event of its own with the time zones it
/// needs (`agenda::calendar_objects`, the writer of the subscribed calendars
/// and of a calendar file opened, its whole weighed against what came:
/// `agenda::kept_most`), marked, each file at most `MOST_FILE_BYTES`. All
/// counted and weighed before anything is written: past a bound, none.
/// `busy`: whether the person counts this agenda as their own busy time (the
/// transparency each lesson is written with), resolved before the call.
/// `cards`: the UID of the card each person of the extension is bound to, by
/// their mark (`cards::uids_by_mark`): an event whose person
/// (`AgendaReply::ties`) has a card is tied to it (`contact_line`); one whose
/// person has none is tied to nobody.
pub fn lessons(extension: &str, agenda: &AgendaReply, words: &Words, busy: bool, cards: &BTreeMap<String, String>) -> Result<Vec<Lesson>, Problem> {
    // Every event has its UID already (`reply::clean` refuses one without).
    let objects = match agenda::calendar_objects(&agenda.calendar, &|_: &[String]| String::new(), agenda::kept_most(agenda.calendar.len())) {
        Ok(objects) => objects,
        Err(agenda::FileProblem::NoEvent) => return Ok(Vec::new()),
        Err(agenda::FileProblem::TooLarge) => return Err(Problem::TooLarge),
        Err(_) => return Err(Problem::Unreadable),
    };
    let category = super::category(extension);
    let mut out = Vec::with_capacity(objects.len());
    for (own, object) in objects {
        if own.is_empty() {
            return Err(Problem::Unreadable);
        }
        let contact = agenda.ties.get(&own).and_then(|tied| cards.get(&super::person_mark(extension, &tied.set, &tied.person)).map(|card| contact_line(card, &tied.name)));
        let hash = lesson_hash(&object, contact.as_deref());
        let uid = super::event_uid(extension, &agenda.id, &own);
        let text = marked(&object, &uid, &category, &hash, words, busy, contact.as_deref());
        if text.len() > MOST_FILE_BYTES {
            return Err(Problem::TooLarge);
        }
        let Some((start, end)) = agenda::first_times(&text, &TimeZone::UTC) else { return Err(Problem::Unreadable) };
        out.push(Lesson { file: super::event_file(&uid), uid, text, hash, start, end });
    }
    Ok(out)
}

/// The line that ties an event to the card of the person it is with, as any
/// calendar program reads it (RFC 5545 §3.8.4.2): `CONTACT`, the person's name
/// as the service writes it (never the card's own name, which may be the
/// person's private word for them, in a calendar others may read), and
/// `ALTREP`, the card's address by its UID (`contacts::uid_uri`). Never an
/// `ATTENDEE`: that would make the event a meeting and send invitations.
pub fn contact_line(card_uid: &str, name: &str) -> String {
    format!("CONTACT;ALTREP=\"{}\":{}", crate::contacts::uid_uri(card_uid), lines::escape(name))
}

/// A lesson's hash as the extension gave it: its lines, its stamps and
/// Sioul's own lines aside, in their order. An event with categories of its
/// own or tied to a card (`contact`) adds what this Sioul writes of them, so
/// that an event an older Sioul wrote without them is written again here,
/// and a tie that changes (another card bound, the card gone) writes the
/// event again; any other event's hash is the one an older Sioul computes.
fn lesson_hash(object: &str, contact: Option<&str>) -> String {
    let mut said = String::new();
    let mut own_categories = false;
    for line in lines::unfold(object) {
        if !STAMPS.contains(&lines::name(&line).as_str()) && !lines::raw_name(&line).starts_with("X-SIOUL-") {
            own_categories |= lines::name(&line) == "CATEGORIES";
            said.push_str(&line);
            said.push('\n');
        }
    }
    if own_categories || contact.is_some() {
        said.push_str("sioul door: categories and contact\n");
        said.push_str(contact.unwrap_or(""));
        said.push('\n');
    }
    vdir::content_hash(said.as_bytes())
}

/// An event's object marked as the extension's: its UID Sioul's, its
/// categories the extension's name first, then the event's own (a salon's
/// services), on one line (a phone's account manager reads the first
/// `CATEGORIES` line alone); tied to the card of the person it is with
/// (`contact`, `contact_line`), when it is; busy or free as the person counts
/// this agenda (`busy`), its description ending with where it comes from, a
/// cancelled one said so in its title, and its hash.
fn marked(object: &str, uid: &str, category: &str, hash: &str, words: &Words, busy: bool, contact: Option<&str>) -> String {
    let source = lines::unfold(object);
    let cancelled = source.iter().any(|l| lines::name(l) == "STATUS" && lines::value(l).trim().eq_ignore_ascii_case("CANCELLED"));
    let mut out: Vec<String> = Vec::with_capacity(source.len() + 5);
    let (mut inside, mut titled, mut described) = (false, false, false);
    // The extension's name first, then the event's own categories, each once (case and accents aside).
    let mut categories = vec![category.to_string()];
    for line in source {
        let name = lines::name(&line);
        let value = lines::value(&line).trim().to_string();
        match name.as_str() {
            "BEGIN" if value.eq_ignore_ascii_case("VEVENT") => {
                inside = true;
                out.push(line);
            }
            "END" if inside && value.eq_ignore_ascii_case("VEVENT") => {
                if !titled && cancelled {
                    out.push(format!("SUMMARY:{}", lines::escape(&(words.cancelled)(""))));
                }
                if !described {
                    out.push(format!("DESCRIPTION:{}", lines::escape(&words.from)));
                }
                out.push(crate::contacts::categories_line(&categories));
                if let Some(contact) = contact {
                    out.push(contact.to_string());
                }
                out.push(if busy { "TRANSP:OPAQUE" } else { "TRANSP:TRANSPARENT" }.into());
                out.push(format!("{HASH_LINE}:{hash}"));
                out.push(line);
                inside = false;
            }
            "UID" if inside => out.push(format!("UID:{uid}")),
            "CATEGORIES" if inside => {
                for name in crate::contacts::categories_of(std::slice::from_ref(&line)) {
                    if !categories.iter().any(|c| crate::contacts::category_key(c) == crate::contacts::category_key(&name)) {
                        categories.push(name);
                    }
                }
            }
            // Its busy mark and its tie are the door's to write (an extension's CONTACT never comes: `reply::clean`).
            "CONTACT" | "TRANSP" if inside => {}
            _ if inside && lines::raw_name(&line) == HASH_LINE => {}
            "SUMMARY" if inside => {
                titled = true;
                if cancelled {
                    out.push(format!("SUMMARY:{}", lines::escape(&(words.cancelled)(lines::unescape(&value).trim()))));
                } else {
                    out.push(line);
                }
            }
            "DESCRIPTION" if inside => {
                described = true;
                let own = lines::unescape(&value);
                let own = own.trim_end();
                out.push(format!("DESCRIPTION:{}", lines::escape(&if own.is_empty() { words.from.clone() } else { format!("{own}\n\n{}", words.from) })));
            }
            _ => out.push(line),
        }
    }
    lines::fold(&out)
}

/// An event of the extension's found in a calendar: its file, the hash it
/// carries (none when a server dropped the line), its first times, and
/// whether it is written free (`TRANSP:TRANSPARENT`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub path: PathBuf,
    pub hash: Option<String>,
    pub times: Option<(i64, i64)>,
    pub free: bool,
}

/// The events of `dir` whose UID is of the form `super::event_uid` gives for
/// this extension and this agenda, by UID; nothing else is looked at further.
/// Two files with one UID: the first by name alone.
pub fn held(dir: &Path, extension: &str, agenda: &str) -> BTreeMap<String, Held> {
    let key = super::agenda_key(extension, agenda);
    let mut found = BTreeMap::new();
    for (path, uid, text) in marked_files(dir) {
        let Some(mark) = super::uid_mark(&uid) else { continue };
        if mark.extension != extension || mark.agenda != key || found.contains_key(&uid) {
            continue;
        }
        let unfolded = lines::unfold(&text);
        let hash = unfolded.iter().find(|l| lines::raw_name(l) == HASH_LINE).map(|l| lines::value(l).trim().to_string()).filter(|h| !h.is_empty());
        let free = unfolded.iter().any(|l| lines::name(l) == "TRANSP" && lines::value(l).trim().eq_ignore_ascii_case("TRANSPARENT"));
        found.insert(uid, Held { path, hash, times: agenda::first_times(&text, &TimeZone::UTC), free });
    }
    found
}

/// The files of a calendar's folder whose event's UID has the form of an
/// extension's (`super::uid_mark`): their path, that UID, their text. A file
/// too large to be one, or that does not read, is passed over.
fn marked_files(dir: &Path) -> Vec<(PathBuf, String, String)> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "ics")).collect();
    paths.sort();
    let mut out = Vec::new();
    for path in paths {
        if std::fs::metadata(&path).map_or(true, |m| !m.is_file() || m.len() > READ_MOST) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let unfolded = lines::unfold(&text);
        let mut inside = false;
        let mut uid = None;
        for line in &unfolded {
            match lines::name(line).as_str() {
                "BEGIN" if lines::value(line).trim().eq_ignore_ascii_case("VEVENT") => inside = true,
                "END" if lines::value(line).trim().eq_ignore_ascii_case("VEVENT") => inside = false,
                "UID" if inside => {
                    uid = Some(lines::value(line).trim().to_string());
                    break;
                }
                _ => {}
            }
        }
        if let Some(uid) = uid.filter(|u| super::uid_mark(u).is_some()) {
            out.push((path, uid, text));
        }
    }
    out
}

/// What a refresh does to the calendar: files to write (path, text, UID,
/// hash) and to remove (path, UID); lessons left as they are; a lesson
/// whose file name is taken by an event not the extension's (left out).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub write: Vec<(PathBuf, String, String, String)>,
    pub remove: Vec<(PathBuf, String)>,
    pub unchanged: usize,
    pub clash: usize,
}

/// The changes `lessons` bring to the calendar in `dir`, given what it holds
/// of this agenda (`held`) and the hashes this device noted (`noted`): see
/// the module's rules. `read`: the period the extension says it read; none,
/// and nothing is removed; `unread`: its days not read, never touched.
pub fn plan(dir: &Path, lessons: &[Lesson], held: &BTreeMap<String, Held>, noted: &BTreeMap<String, String>, read: Option<(i64, i64)>, unread: &[(i64, i64)]) -> Plan {
    let mut plan = Plan::default();
    let mut answered = BTreeSet::new();
    for lesson in lessons {
        answered.insert(lesson.uid.as_str());
        match held.get(&lesson.uid) {
            Some(there) => {
                let known = there.hash.as_deref().or_else(|| noted.get(&lesson.uid).map(String::as_str));
                // Its busy mark is not part of its hash: one written the other way (the agenda's
                // switch turned since, on any device) is written again, so that the device that
                // runs the extension follows the shared choice at its next refresh.
                let free = lesson.text.contains("TRANSP:TRANSPARENT");
                if known == Some(lesson.hash.as_str()) && there.free == free {
                    plan.unchanged += 1;
                } else {
                    plan.write.push((there.path.clone(), lesson.text.clone(), lesson.uid.clone(), lesson.hash.clone()));
                }
            }
            None => {
                let path = dir.join(&lesson.file);
                // A file of that name that is no event of this agenda's: never written over.
                if path.exists() {
                    plan.clash += 1;
                } else {
                    plan.write.push((path, lesson.text.clone(), lesson.uid.clone(), lesson.hash.clone()));
                }
            }
        }
    }
    // An answer without any event removes nothing; neither does one that says no period.
    let Some((from, until)) = read.filter(|_| !lessons.is_empty()) else { return plan };
    for (uid, there) in held {
        if answered.contains(uid.as_str()) {
            continue;
        }
        let Some((start, end)) = there.times else { continue };
        let not_read = unread.iter().any(|(f, u)| start < *u && end.max(start + 1) > *f);
        if start >= from && start < until && !not_read {
            plan.remove.push((there.path.clone(), uid.clone()));
        }
    }
    plan
}

/// What a refresh did to the calendar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Done {
    pub written: usize,
    pub removed: usize,
    pub unchanged: usize,
    /// Changes the agenda's door refused (a meeting with others now, the
    /// calendar read only meanwhile) or that could not be written.
    pub refused: usize,
    pub clash: usize,
    /// The lessons the answer held.
    pub lessons: usize,
}

/// Whether a calendar may receive an extension's lessons: one that takes
/// events and that its server lets the person write, never a subscribed one
/// nor a calendar of busy blocks Sioul keeps (`closed`). The person's own
/// word on what they may do with that agenda (read only, its availability
/// only) is no bar (26 November 2026: whose an agenda is decides what
/// reaches the person; who may write it is apart): the door writes past it
/// (`agenda::as_extension_door`), and the picker says so under the calendar chosen.
pub fn writable(calendar: &vdir::Collection) -> bool {
    calendar.kind == vdir::Kind::Calendars && calendar.holds("VEVENT") && closed(calendar, &crate::agendas::read_config()).is_none()
}

/// Why the door cannot write into a calendar that holds events, as the
/// calendar picker says it beside it (26 November 2026: a calendar was
/// left out of the picker without a word, and the person made a second
/// one). The first that holds, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    /// Subscribed to by its address: read, never written.
    Subscribed,
    /// A calendar of busy blocks Sioul keeps for an availability
    /// (`agendas::is_mirror`): its events would reach whoever reads those
    /// blocks, titles and all.
    BusyTimes,
    /// Its server lets the person read it only (a share read only).
    ServerReadOnly,
    /// Its folder is not on this device.
    NotHere,
}

impl Closed {
    /// Its word, for the window's sentence (`extension-calendar-closed-<word>`).
    pub fn word(self) -> &'static str {
        match self {
            Self::Subscribed => "subscribed",
            Self::BusyTimes => "busy-times",
            Self::ServerReadOnly => "server-read-only",
            Self::NotHere => "not-here",
        }
    }
}

/// Why the door cannot write into `calendar`; none when it can. Only what
/// its server, its kind or this device decide: never the person's own word
/// on the agenda (`writable`).
pub fn closed(calendar: &vdir::Collection, config: &crate::config::Config) -> Option<Closed> {
    if calendar.account == vdir::SUBSCRIBED {
        return Some(Closed::Subscribed);
    }
    if crate::agendas::is_mirror(config, &calendar.account, &calendar.id) {
        return Some(Closed::BusyTimes);
    }
    if vdir::server_read_only(&calendar.account, &calendar.id) {
        return Some(Closed::ServerReadOnly);
    }
    if !calendar.dir.is_dir() {
        return Some(Closed::NotHere);
    }
    None
}

/// What the person keeps an agenda to (`[agenda.calendars."<place>"]
/// access`): read only, or its availability only; none when they keep it
/// read and write. The door writes past it (`writable`); the picker says it.
pub fn kept_from_writers(calendar: &vdir::Collection, config: &crate::config::Config) -> Option<crate::agendas::Access> {
    use crate::agendas::Access;
    config.agenda.calendars.get(&crate::agendas::place(calendar)).and_then(|said| said.access.as_deref()).and_then(Access::parse).filter(|access| *access != Access::Write)
}

/// The calendar a place names ("account/id"), when it is one this device holds.
pub fn calendar_at(place: &str) -> Option<vdir::Collection> {
    vdir::collections(vdir::Kind::Calendars).into_iter().find(|c| crate::agendas::place(c) == place)
}

/// An agenda written into `calendar` (the module's rules). Bounds and the
/// calendar are checked before anything is written; past one, nothing is.
/// Nothing at all is written, this device's notes included, when nothing differs.
/// `busy`: whether the person counts this agenda as their own busy time
/// (OPAQUE), else free (TRANSPARENT), resolved before the call.
pub fn take(extension: &str, agenda: &AgendaReply, calendar: &vdir::Collection, words: &Words, busy: bool) -> Result<Done, Problem> {
    if !writable(calendar) {
        return Err(Problem::CalendarGone);
    }
    // The cards its events' people are bound to, as the address books stand now (their sets written first).
    // A card in an address book on this device only names nothing other devices hold: none in a calendar its server carries.
    let cards = if agenda.ties.is_empty() { BTreeMap::new() } else { super::cards::uids_by_mark(extension, calendar.account == vdir::LOCAL) };
    let lessons = lessons(extension, agenda, words, busy, &cards)?;
    let held = held(&calendar.dir, extension, &agenda.id);
    let mut noted = load_noted(extension);
    let plan = plan(&calendar.dir, &lessons, &held, &noted.events, agenda.read, &agenda.unread);
    let mut done = Done { unchanged: plan.unchanged, clash: plan.clash, lessons: lessons.len(), ..Done::default() };
    if plan.write.is_empty() && plan.remove.is_empty() {
        // The hashes found on the events stand; a note missing is not worth a write.
        return Ok(done);
    }
    for (path, text, uid, hash) in &plan.write {
        match crate::agenda::as_extension_door(|| vdir::write_item(path, text)) {
            Ok(()) => {
                done.written += 1;
                noted.events.insert(uid.clone(), hash.clone());
            }
            Err(why) => {
                eprintln!("sioul: extensions: {extension}: a lesson not written: {why}");
                done.refused += 1;
            }
        }
    }
    for (path, uid) in &plan.remove {
        match crate::agenda::as_extension_door(|| vdir::remove_item(path)) {
            Ok(()) => {
                done.removed += 1;
                noted.events.remove(uid);
            }
            Err(why) => {
                eprintln!("sioul: extensions: {extension}: a lesson not taken out: {why}");
                done.refused += 1;
            }
        }
    }
    if let Err(why) = save_noted(extension, &noted) {
        eprintln!("sioul: extensions: {extension}: its notes of the lessons: {why}");
    }
    Ok(done)
}

/// Every event of `extension` taken out of the person's calendars (Remove,
/// "take its lessons out"), whatever agenda and calendar: by the form of
/// their UID alone, through the agenda's door; the calendars themselves stay.
/// Past the person's own word on an agenda (read only, its availability
/// only) only in the calendars chosen for this extension, which it wrote
/// past that word (`as_extension_door`); any other calendar they keep read
/// only keeps its events. How many were taken out, and how many the door refused.
pub fn take_out(extension: &str) -> (usize, usize) {
    let (mut removed, mut refused) = (0, 0);
    // The calendars chosen for it (`[extensions.calendars]`, read before Remove takes them out of
    // the settings): the door goes past the person's own word there alone, as it wrote there.
    let config = crate::agendas::read_config();
    let prefix = format!("{extension}/");
    let chosen: BTreeSet<&str> = config.extensions.calendars.iter().filter(|(key, _)| key.starts_with(&prefix)).map(|(_, place)| place.as_str()).collect();
    for calendar in vdir::collections(vdir::Kind::Calendars).into_iter().filter(|c| c.account != vdir::SUBSCRIBED) {
        let door = chosen.contains(crate::agendas::place(&calendar).as_str());
        // Any other calendar the person keeps read only (another person's shared one, where
        // their own Sioul writes the same extension) keeps its events, as their word says.
        if (door && vdir::server_read_only(&calendar.account, &calendar.id)) || (!door && calendar.read_only) {
            continue;
        }
        for (path, uid, _) in marked_files(&calendar.dir) {
            if super::uid_mark(&uid).is_none_or(|m| m.extension != extension) {
                continue;
            }
            let taken = if door { crate::agenda::as_extension_door(|| vdir::remove_item(&path)) } else { vdir::remove_item(&path) };
            match taken {
                Ok(()) => removed += 1,
                Err(why) => {
                    eprintln!("sioul: extensions: {extension}: a lesson not taken out: {why}");
                    refused += 1;
                }
            }
        }
    }
    let _ = std::fs::remove_file(noted_path(extension));
    (removed, refused)
}

/// One agenda's events taken out of one calendar (its lessons going into
/// another the person chose instead): by the form of their UID alone,
/// through the agenda's door. How many, and how many the door refused.
pub fn take_out_of(calendar: &vdir::Collection, extension: &str, agenda: &str) -> (usize, usize) {
    let (mut removed, mut refused) = (0, 0);
    if calendar.account == vdir::SUBSCRIBED || vdir::server_read_only(&calendar.account, &calendar.id) {
        return (0, 0);
    }
    for there in held(&calendar.dir, extension, agenda).values() {
        match crate::agenda::as_extension_door(|| vdir::remove_item(&there.path)) {
            Ok(()) => removed += 1,
            Err(why) => {
                eprintln!("sioul: extensions: {extension}: a lesson not taken out: {why}");
                refused += 1;
            }
        }
    }
    (removed, refused)
}

/// The hashes this device noted of the lessons it wrote, by UID, in case a
/// server drops `HASH_LINE`: this device's own (`$XDG_STATE_HOME/sioul/extensions/<id>.lessons.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Noted {
    #[serde(default)]
    pub events: BTreeMap<String, String>,
}

/// Where this device notes an extension's lessons.
pub fn noted_path(extension: &str) -> PathBuf {
    crate::config::state_dir().join("extensions").join(format!("{extension}.lessons.toml"))
}

fn load_noted(extension: &str) -> Noted {
    std::fs::read_to_string(noted_path(extension)).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// The notes written, `MOST_NOTED` at most (beyond, the events' own lines stand alone).
fn save_noted(extension: &str, noted: &Noted) -> Result<(), String> {
    let path = noted_path(extension);
    let kept = Noted { events: noted.events.iter().take(MOST_NOTED).map(|(k, v)| (k.clone(), v.clone())).collect() };
    let text = toml::to_string(&kept).map_err(|e| e.to_string())?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    crate::filelock::replace(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::reply::refresh_reply;
    use crate::vdir::{Kind, LOCAL};

    /// Each test its own extension: they run side by side, and Remove looks in every calendar.
    const EXT: &str = "skolengo";

    fn words() -> Words<'static> {
        Words { cancelled: &|title: &str| format!("Cancelled: {title}"), from: "From Skolengo, through Sioul.".into() }
    }

    /// A lesson as the extension writes it.
    fn lesson(uid: &str, start: &str, end: &str, title: &str, extra: &str) -> String {
        format!("BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:{start}\r\nDTSTART:{start}\r\nDTEND:{end}\r\nSUMMARY:{title}\r\n{extra}STATUS:CONFIRMED\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\n")
    }

    /// An answer of the extension, read as Sioul reads it: an agenda of
    /// `lessons`, the week of 16 November 2026 read (when `period`), `unread` days.
    fn answer(agenda: &str, lessons: &[String], period: bool, unread: &[(&str, &str)]) -> AgendaReply {
        let calendar = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\n{}END:VCALENDAR\r\n", lessons.concat());
        let mut agenda = serde_json::json!({ "id": agenda, "name": "Timetable: Alex", "calendar": calendar, "unread": unread.iter().map(|(f, u)| serde_json::json!({ "from": f, "until": u })).collect::<Vec<_>>() });
        if period {
            agenda["period"] = serde_json::json!({ "from": "2026-11-16T00:00:00Z", "until": "2026-11-23T00:00:00Z" });
        }
        refresh_reply(&serde_json::json!({ "agendas": [agenda] }).to_string()).unwrap().agendas.remove(0)
    }

    fn calendar(name: &str) -> vdir::Collection {
        crate::config::stand_ins_only();
        vdir::create(Kind::Calendars, LOCAL, name, None, &["VEVENT"]).unwrap()
    }

    /// Every file of a folder, by name, with its bytes.
    fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.path().is_file()).map(|e| (e.file_name().to_string_lossy().to_string(), std::fs::read(e.path()).unwrap())).collect()
    }

    /// A lesson's file as written, its lines unfolded; "" when there is none.
    fn text_of(calendar: &vdir::Collection, uid: &str) -> String {
        let text = std::fs::read_to_string(calendar.dir.join(crate::extensions::event_file(uid))).unwrap_or_default();
        lines::unfold(&text).iter().map(|l| format!("{l}\r\n")).collect()
    }

    /// The rules of a refresh, each in turn: added, marked three ways and
    /// free; left as it is when unchanged at the school (an edit by someone
    /// else kept), written again when changed; cancelled said so; deleted by
    /// hand, back; gone from what was read, removed; on a day not read, kept;
    /// an answer without a period or without any event removes nothing.
    #[test]
    fn lessons_are_added_changed_left_and_removed_in_what_was_read() {
        const EXT: &str = "rules-test";
        let calendar = calendar("Door rules");
        let maths = lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "LOCATION:Room 2\r\n");
        let history = lesson("L2@skolengo", "20261118T080000Z", "20261118T090000Z", "History", "STATUS:CANCELLED\r\n").replace("STATUS:CONFIRMED\r\n", "");
        let music = lesson("L3@skolengo", "20261119T080000Z", "20261119T090000Z", "Music", "DESCRIPTION:Bring a recorder\r\n");
        let first = answer("pupil-1", &[maths.clone(), history.clone(), music.clone()], true, &[]);
        let done = take(EXT, &first, &calendar, &words(), false).unwrap();
        assert_eq!((done.written, done.removed, done.lessons), (3, 0, 3));
        let (l1, l2, l3) = (crate::extensions::event_uid(EXT, "pupil-1", "L1@skolengo"), crate::extensions::event_uid(EXT, "pupil-1", "L2@skolengo"), crate::extensions::event_uid(EXT, "pupil-1", "L3@skolengo"));
        let text = text_of(&calendar, &l1);
        for line in [format!("UID:{l1}"), format!("CATEGORIES:{}", crate::extensions::category(EXT)), "TRANSP:TRANSPARENT".into(), "DESCRIPTION:From Skolengo\\, through Sioul.".into(), format!("{HASH_LINE}:"), "SUMMARY:Maths".into(), "LOCATION:Room 2".into()] {
            assert!(text.contains(&line), "{line} missing:\n{text}");
        }
        assert!(!text.contains("L1@skolengo"), "the extension's own UID is not kept");
        let cancelled = text_of(&calendar, &l2);
        assert!(cancelled.contains("SUMMARY:Cancelled: History") && cancelled.contains("STATUS:CANCELLED"), "{cancelled}");
        assert!(text_of(&calendar, &l3).contains("DESCRIPTION:Bring a recorder\\n\\nFrom Skolengo\\, through Sioul."), "the line comes last");

        // The same answer again: nothing written, not even this device's notes.
        let before = files(&calendar.dir);
        let noted = std::fs::read(noted_path(EXT)).unwrap();
        let again = take(EXT, &first, &calendar, &words(), false).unwrap();
        assert_eq!((again.written, again.removed, again.unchanged), (0, 0, 3));
        assert_eq!(files(&calendar.dir), before);
        assert_eq!(std::fs::read(noted_path(EXT)).unwrap(), noted);

        // Someone else's note on a lesson, the school unchanged: kept.
        let edited = text_of(&calendar, &l1).replace("SUMMARY:Maths", "SUMMARY:Maths (bring the compass)");
        std::fs::write(calendar.dir.join(crate::extensions::event_file(&l1)), &edited).unwrap();
        assert_eq!(take(EXT, &first, &calendar, &words(), false).unwrap().written, 0);
        assert_eq!(text_of(&calendar, &l1), edited, "an edit made by someone else stays while the school does not change the lesson");

        // The school moves the music lesson to another room: written again.
        let moved = answer("pupil-1", &[maths.clone(), history.clone(), music.replace("SUMMARY:Music", "SUMMARY:Music\r\nLOCATION:Hall")], true, &[]);
        assert_eq!(take(EXT, &moved, &calendar, &words(), false).unwrap().written, 1);
        assert!(text_of(&calendar, &l3).contains("LOCATION:Hall"));

        // Deleted by hand: back at the next refresh.
        std::fs::remove_file(calendar.dir.join(crate::extensions::event_file(&l2))).unwrap();
        assert_eq!(take(EXT, &moved, &calendar, &words(), false).unwrap().written, 1);
        assert!(text_of(&calendar, &l2).contains("STATUS:CANCELLED"));

        // History's day not read, music gone from the answer, its day read: music removed, history kept.
        let partial = answer("pupil-1", &[maths.clone()], true, &[("2026-11-18T00:00:00Z", "2026-11-19T00:00:00Z")]);
        let done = take(EXT, &partial, &calendar, &words(), false).unwrap();
        assert_eq!((done.written, done.removed), (0, 1));
        assert!(text_of(&calendar, &l3).is_empty() && !text_of(&calendar, &l2).is_empty());

        // No period said, or no event at all: absence proves nothing.
        let no_period = answer("pupil-1", &[maths.clone()], false, &[]);
        assert_eq!(take(EXT, &no_period, &calendar, &words(), false).unwrap().removed, 0);
        let nothing = answer("pupil-1", &[], true, &[]);
        assert_eq!(take(EXT, &nothing, &calendar, &words(), false).unwrap().removed, 0);
        assert!(!text_of(&calendar, &l2).is_empty() && !text_of(&calendar, &l1).is_empty());

        // A lesson outside the period read, gone from the answer: left.
        let later = answer("pupil-1", &[lesson("L9@skolengo", "20261201T080000Z", "20261201T090000Z", "Art", "")], false, &[]);
        take(EXT, &later, &calendar, &words(), false).unwrap();
        let l9 = crate::extensions::event_uid(EXT, "pupil-1", "L9@skolengo");
        assert_eq!(take(EXT, &answer("pupil-1", &[maths], true, &[]), &calendar, &words(), false).unwrap().removed, 1, "history's day read now: it goes, Art's day is not in the period");
        assert!(!text_of(&calendar, &l9).is_empty());
    }

    /// The person's own busy time: an agenda the person counts as busy is
    /// written `TRANSP:OPAQUE`, one they do not `TRANSP:TRANSPARENT`; and an
    /// OPAQUE lesson, the person's own and needing no answer, reaches Sioul's
    /// real busy (`busy::holds_you`), so two can clash, while a free one does
    /// not. The busy mark is Sioul's, not part of the lesson's hash, and a
    /// lesson held with the other mark is written again (`plan`), so that a
    /// choice turned on a device that does not run the extension reaches it.
    #[test]
    fn a_busy_agenda_is_written_opaque_and_holds_you() {
        const EXT: &str = "busy-test";
        let free_cal = calendar("Door free");
        let busy_cal = calendar("Door busy");
        let maths = lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "");
        let reply = answer("pupil-1", &[maths], true, &[]);
        let uid = crate::extensions::event_uid(EXT, "pupil-1", "L1@skolengo");
        // Not counted as busy: free time, as a pupil's lesson is by default.
        take(EXT, &reply, &free_cal, &words(), false).unwrap();
        assert!(text_of(&free_cal, &uid).contains("TRANSP:TRANSPARENT"), "free by default");
        // Counted as busy: opaque, nothing else of the mark changed.
        assert_eq!(take(EXT, &reply, &busy_cal, &words(), true).unwrap().written, 1);
        let text = text_of(&busy_cal, &uid);
        assert!(text.contains("TRANSP:OPAQUE") && !text.contains("TRANSP:TRANSPARENT"), "{text}");
        assert!(text.contains(&format!("UID:{uid}")) && text.contains("SUMMARY:Maths"), "nothing else of the mark changes:\n{text}");
        // The opaque lesson reaches Sioul's real busy; the free one does not.
        let read = |cal: &vdir::Collection| agenda::file_occurrences(&cal.dir.join(crate::extensions::event_file(&uid)), cal, 1_794_000_000, 1_796_000_000, &TimeZone::UTC);
        let busy_read = read(&busy_cal);
        let event = busy_read.iter().find(|o| o.summary == "Maths").expect("the busy lesson read back");
        assert!(crate::busy::own_event(event), "the door writes into a calendar of the person's own");
        assert!(crate::busy::event_marked(event), "an OPAQUE timed event is busy");
        assert!(event.needs_you(), "no ORGANIZER/ATTENDEE: the person's own, not an invitation");
        assert!(crate::busy::holds_you(event), "an OPAQUE own lesson holds you busy");
        let free_read = read(&free_cal);
        let free = free_read.iter().find(|o| o.summary == "Maths").expect("the free lesson read back");
        assert!(crate::busy::own_event(free) && !crate::busy::event_marked(free), "a TRANSPARENT lesson is own but not busy");
        assert!(!crate::busy::holds_you(free), "a free lesson does not hold you busy");
        // The choice turned since, on any device (its calendar's switch, review of edef0091, F6):
        // the same lesson written again with its new mark at the next refresh, then left as it is.
        assert_eq!(take(EXT, &reply, &free_cal, &words(), true).unwrap().written, 1, "its mark differs: written again");
        assert!(text_of(&free_cal, &uid).contains("TRANSP:OPAQUE"));
        assert_eq!(take(EXT, &reply, &free_cal, &words(), true).unwrap().unchanged, 1, "the same mark: nothing written");
    }

    /// Only events whose UID has the extension's form are touched: the
    /// person's own event with the same title and time as a lesson, another
    /// carrying the category by hand, and a file of the very name Sioul would
    /// give a lesson but not of its UID stay byte for byte, through a refresh
    /// that removes and a Remove that takes the lessons out.
    #[test]
    fn only_marked_events_are_touched() {
        let calendar = calendar("Door shared");
        let own = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//person//EN\r\nBEGIN:VEVENT\r\nUID:own-1@example.org\r\nDTSTAMP:20261117T080000Z\r\nDTSTART:20261117T080000Z\r\nDTEND:20261117T090000Z\r\nSUMMARY:Maths\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let by_hand = own.replace("own-1@example.org", "own-2@example.org").replace("SUMMARY:Maths", "SUMMARY:Dentist\r\nCATEGORIES:Skolengo\r\nDESCRIPTION:From Skolengo\\, through Sioul.");
        let clashing_uid = crate::extensions::event_uid(EXT, "pupil-1", "L4@skolengo");
        let clash_name = crate::extensions::event_file(&clashing_uid);
        let clash = own.replace("own-1@example.org", "own-3@example.org").replace("SUMMARY:Maths", "SUMMARY:Piano");
        std::fs::write(calendar.dir.join("own-1.ics"), own).unwrap();
        std::fs::write(calendar.dir.join("own-2.ics"), &by_hand).unwrap();
        std::fs::write(calendar.dir.join(&clash_name), &clash).unwrap();
        let mine = |calendar: &vdir::Collection| files(&calendar.dir).into_iter().filter(|(n, _)| n.starts_with("own-") || *n == clash_name).collect::<BTreeMap<_, _>>();
        let before = mine(&calendar);
        assert_eq!(before.len(), 3);
        let first = answer("pupil-1", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", ""), lesson("L4@skolengo", "20261119T080000Z", "20261119T090000Z", "Piano", "")], true, &[]);
        let done = take(EXT, &first, &calendar, &words(), false).unwrap();
        assert_eq!((done.written, done.clash), (1, 1), "the lesson whose file name is taken by an event not its own is left out");
        // A refresh where nothing is held: every marked lesson of the week goes, nothing else.
        let empty_week = answer("pupil-1", &[lesson("L5@skolengo", "20261124T080000Z", "20261124T090000Z", "Maths", "")], true, &[]);
        assert_eq!(take(EXT, &empty_week, &calendar, &words(), false).unwrap().removed, 1);
        assert_eq!(mine(&calendar), before, "the person's own events are never read as the school's");
        // Remove, its lessons taken out: the person's own events stay.
        take(EXT, &first, &calendar, &words(), false).unwrap();
        let (removed, refused) = take_out(EXT);
        assert!(removed >= 2 && refused == 0, "{removed} {refused}");
        assert_eq!(mine(&calendar), before);
        assert!(files(&calendar.dir).keys().all(|n| n.starts_with("own-") || *n == clash_name || !n.ends_with(".ics")));
    }

    /// An answer whose agenda's events name the people of a set handed back beside it.
    fn answer_with(agenda: &str, lessons: &[String], people: serde_json::Value) -> AgendaReply {
        let calendar = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\n{}END:VCALENDAR\r\n", lessons.concat());
        let reply = serde_json::json!({
            "agendas": [{ "id": agenda, "name": "Salon", "calendar": calendar, "period": { "from": "2026-11-16T00:00:00Z", "until": "2026-11-23T00:00:00Z" } }],
            "contacts": [{ "id": "clients", "name": "Clients", "people": people }],
        });
        refresh_reply(&reply.to_string()).unwrap().agendas.remove(0)
    }

    /// The `CONTACT` lines of a lesson as written; none when it has no file.
    fn contacts_of(calendar: &vdir::Collection, uid: &str) -> Vec<String> {
        text_of(calendar, uid).split("\r\n").filter(|l| lines::name(l) == "CONTACT").map(str::to_string).collect()
    }

    /// Appointments tied to their client's card (invented clients): the event
    /// carries `CONTACT` with the client's name as the service gives it and
    /// `ALTREP` the card's address by its UID, never a guest; two
    /// appointments of one client name the same card; a client without a
    /// card, and an appointment naming no one, carry none; a card made later
    /// ties its client's appointments then; a card removed drops the line, a
    /// card bound again or merged into another re-points it; nothing is
    /// written when the tie is the same.
    #[test]
    fn appointments_are_tied_to_their_client_s_card() {
        const EXT: &str = "ties-test";
        let calendar = calendar("Door ties");
        let book = vdir::create(Kind::Contacts, LOCAL, "Door ties clients", None, &[]).unwrap();
        let mark = |person: &str| crate::extensions::person_mark(EXT, "clients", person);
        let person = |id: &str| format!("X-SIOUL-PERSON;X-SIOUL-SET=clients:{id}\r\n");
        let coupe = lesson("B1@salon", "20261117T080000Z", "20261117T090000Z", "Marie Exemple — Coupe", &format!("{}CATEGORIES:Coupe,Couleur\\, mèches\r\n", person("client-41")));
        let again = lesson("B2@salon", "20261118T080000Z", "20261118T090000Z", "Marie Exemple — Brushing", &person("client-41"));
        let jean = lesson("B3@salon", "20261119T080000Z", "20261119T090000Z", "Jean Exemple — Coupe", &person("client-42"));
        let pause = lesson("B4@salon", "20261119T120000Z", "20261119T130000Z", "Pause", "");
        let people = serde_json::json!([{ "id": "client-41", "name": "Marie Exemple" }, { "id": "client-42", "name": "Jean Exemple" }]);
        let reply = answer_with("salon", &[coupe, again, jean, pause], people);
        assert_eq!(reply.ties.len(), 3);
        let uid = |own: &str| crate::extensions::event_uid(EXT, "salon", own);
        let (b1, b2, b3, b4) = (uid("B1@salon"), uid("B2@salon"), uid("B3@salon"), uid("B4@salon"));
        let card = |file: &str, uid: &str, name: &str, marks: &[String]| {
            let marks: String = marks.iter().map(|m| format!("X-SIOUL-EXTENSION:{m}\r\n")).collect();
            let path = book.dir.join(file);
            std::fs::write(&path, format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\nFN:{name}\r\n{marks}END:VCARD\r\n")).unwrap();
            path
        };

        // Marie's card is one of the person's own, bound by the door (its mark), its own name kept.
        let own = card("marie.vcf", "f81d4fae-7dec-11d0-a765-00a0c91e6bf6", "Marie (neighbour)", &[mark("client-41")]);
        let done = take(EXT, &reply, &calendar, &words(), true).unwrap();
        assert_eq!((done.written, done.lessons), (4, 4));
        let tie = "CONTACT;ALTREP=\"urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6\":Marie Exemple".to_string();
        assert_eq!(contacts_of(&calendar, &b1), vec![tie.clone()], "the service's name, never the card's own");
        assert_eq!(contacts_of(&calendar, &b2), vec![tie.clone()], "two appointments of one client, one card");
        assert!(contacts_of(&calendar, &b3).is_empty(), "a client without a card: no tie");
        assert!(contacts_of(&calendar, &b4).is_empty(), "an appointment naming no one: no tie");
        let text = text_of(&calendar, &b1);
        assert!(text.contains(&format!("CATEGORIES:{},Coupe,Couleur\\, mèches\r\n", crate::extensions::category(EXT))), "the extension's name first, the services after, on one line:\n{text}");
        assert!(text_of(&calendar, &b2).contains(&format!("CATEGORIES:{}\r\n", crate::extensions::category(EXT))));
        for gone in ["ATTENDEE", "ORGANIZER", "X-SIOUL-PERSON", "client-41"] {
            assert!(!text.contains(gone), "{gone}:\n{text}");
        }
        assert!(!crate::agenda::with_others(&text), "a tie is no meeting");
        assert_eq!(take(EXT, &reply, &calendar, &words(), true).unwrap().written, 0, "the same ties: nothing written");

        // Jean's card made by the door later (its UID the mark): Jean's appointment is tied then.
        let jean_alone = crate::extensions::reply::ContactSet { id: "clients".into(), name: "Clients".into(), people: vec![crate::extensions::reply::Person { id: "client-42".into(), name: "Jean Exemple".into(), ..Default::default() }] };
        assert_eq!(crate::extensions::cards::take(EXT, &jean_alone, &book, None, &crate::extensions::cards::Trust::default()).unwrap().made, 1);
        assert_eq!(take(EXT, &reply, &calendar, &words(), true).unwrap().written, 1);
        assert_eq!(contacts_of(&calendar, &b3), vec![format!("CONTACT;ALTREP=\"urn:uuid:{}\":Jean Exemple", mark("client-42"))]);

        // Marie's card removed: the line dropped from both, nothing left dangling.
        std::fs::remove_file(&own).unwrap();
        assert_eq!(take(EXT, &reply, &calendar, &words(), true).unwrap().written, 2);
        assert!(contacts_of(&calendar, &b1).is_empty() && contacts_of(&calendar, &b2).is_empty());
        assert!(text_of(&calendar, &b1).contains("Coupe,Couleur"), "its services stay");

        // Bound again to another card of the person's: re-pointed.
        card("marie-2.vcf", "sioul-marie-2", "Marie Exemple", &[mark("client-41")]);
        assert_eq!(take(EXT, &reply, &calendar, &words(), true).unwrap().written, 2);
        assert_eq!(contacts_of(&calendar, &b2), vec!["CONTACT;ALTREP=\"urn:uuid:sioul-marie-2\":Marie Exemple".to_string()]);

        // Jean's card merged into one of the person's own (which keeps the mark): re-pointed to the one kept.
        let lead = card("jean-lead.vcf", "urn:uuid:0e2a9c4b-1111-4222-8333-444455556666", "Jean", &[]);
        let made = book.dir.join(crate::extensions::card_file(&mark("client-42")));
        crate::duplicates::merge_files(&lead, &made, None, &crate::config::state_dir().join("ties-test-merges"), 1_795_000_000, false).unwrap();
        assert!(!made.exists());
        assert_eq!(take(EXT, &reply, &calendar, &words(), true).unwrap().written, 1);
        assert_eq!(contacts_of(&calendar, &b3), vec!["CONTACT;ALTREP=\"urn:uuid:0e2a9c4b-1111-4222-8333-444455556666\":Jean Exemple".to_string()], "a UID that is a URN already, as it is");
        assert_eq!(take(EXT, &reply, &calendar, &words(), true).unwrap().written, 0);
    }

    /// A card in an address book on this device only ties nothing in a
    /// calendar its server carries (the review of 4f268ddb, 3): it would name
    /// a card no other device holds, and the tie is part of each event's hash,
    /// so two devices running the extension would rewrite each other's
    /// appointments at each turn. Seen from both devices, the event is the same
    /// and nothing is written again; in a calendar of this device only, this
    /// device's card ties; a card on the account's own address book ties anywhere.
    #[test]
    fn a_card_on_this_device_alone_ties_nothing_in_a_calendar_others_read() {
        const EXT: &str = "here-only-test";
        crate::config::stand_ins_only();
        let shared = vdir::create(Kind::Calendars, "cloud-ties", "Door ties shared", None, &["VEVENT"]).unwrap();
        let here_book = vdir::create(Kind::Contacts, LOCAL, "Door ties this device", None, &[]).unwrap();
        let mark = crate::extensions::person_mark(EXT, "clients", "client-41");
        let card = format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:sioul-here-only\r\nFN:Marie Exemple\r\nX-SIOUL-EXTENSION:{mark}\r\nEND:VCARD\r\n");
        let card_here = here_book.dir.join("marie.vcf");
        std::fs::write(&card_here, &card).unwrap();
        let reply = answer_with("salon", &[lesson("B1@salon", "20261117T080000Z", "20261117T090000Z", "Marie Exemple — Coupe", "X-SIOUL-PERSON;X-SIOUL-SET=clients:client-41\r\n")], serde_json::json!([{ "id": "client-41", "name": "Marie Exemple" }]));
        let uid = crate::extensions::event_uid(EXT, "salon", "B1@salon");
        // Device A holds the card in a book of its own: no tie in the shared calendar.
        assert_eq!(take(EXT, &reply, &shared, &words(), true).unwrap().written, 1);
        assert!(contacts_of(&shared, &uid).is_empty(), "{}", text_of(&shared, &uid));
        // Device B holds no such book: the same event, nothing written again.
        std::fs::remove_file(&card_here).unwrap();
        assert_eq!(take(EXT, &reply, &shared, &words(), true).unwrap().written, 0, "the other device's view writes nothing");
        // A calendar of this device only: this device's card ties.
        std::fs::write(&card_here, &card).unwrap();
        let here = calendar("Door ties this device only");
        take(EXT, &reply, &here, &words(), true).unwrap();
        assert_eq!(contacts_of(&here, &uid), vec!["CONTACT;ALTREP=\"urn:uuid:sioul-here-only\":Marie Exemple".to_string()]);
        // The card in the account's own address book, which every device holds: tied in the shared calendar too.
        let cloud_book = vdir::create(Kind::Contacts, "cloud-ties", "Door ties clients", None, &[]).unwrap();
        std::fs::rename(&card_here, cloud_book.dir.join("marie.vcf")).unwrap();
        assert_eq!(take(EXT, &reply, &shared, &words(), true).unwrap().written, 1);
        assert_eq!(contacts_of(&shared, &uid), vec!["CONTACT;ALTREP=\"urn:uuid:sioul-here-only\":Marie Exemple".to_string()]);
    }

    /// Remove goes past the person's own word on an agenda only in the
    /// calendars chosen for the extension (the review of 4f268ddb, 1): a
    /// calendar they keep read only that was never chosen for it (another
    /// person's shared one, where their own Sioul writes the same extension)
    /// keeps its events; the one chosen, kept read only too, is emptied.
    #[test]
    fn remove_goes_past_the_person_s_word_only_where_the_extension_was_chosen() {
        const EXT: &str = "remove-kept-test";
        let config_path = crate::config::default_path();
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let chosen = calendar("Door remove chosen");
        let other = calendar("Door remove another's");
        take(EXT, &answer("pupil-1", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "")], true, &[]), &chosen, &words(), false).unwrap();
        take(EXT, &answer("pupil-2", &[lesson("L2@skolengo", "20261117T080000Z", "20261117T090000Z", "Art", "")], true, &[]), &other, &words(), false).unwrap();
        for calendar in [&chosen, &other] {
            crate::agendas::set_field(&config_path, &crate::agendas::place(calendar), "access", "read").unwrap();
        }
        crate::config::set_extension_calendar(&config_path, EXT, "pupil-1", &crate::agendas::place(&chosen)).unwrap();
        assert_eq!(take_out(EXT), (1, 0), "the chosen one alone");
        assert!(text_of(&chosen, &crate::extensions::event_uid(EXT, "pupil-1", "L1@skolengo")).is_empty());
        assert!(!text_of(&other, &crate::extensions::event_uid(EXT, "pupil-2", "L2@skolengo")).is_empty(), "a calendar kept read only, never chosen for it, keeps its events");
    }

    /// The hash an older Sioul computes stays for an event with no category
    /// of its own and no tie (nothing written again after an update), and
    /// differs once either comes (an event an older Sioul wrote without them
    /// is written again here).
    #[test]
    fn the_hash_changes_only_with_categories_or_a_tie() {
        let object = |extra: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:L1\r\nDTSTAMP:20261117T080000Z\r\nDTSTART:20261117T080000Z\r\nSUMMARY:Maths\r\n{extra}END:VEVENT\r\nEND:VCALENDAR\r\n");
        // The hash as a Sioul before 26 November 2026 computes it.
        let older = |text: &str| {
            let said: String = lines::unfold(text).into_iter().filter(|l| !STAMPS.contains(&lines::name(l).as_str()) && !lines::raw_name(l).starts_with("X-SIOUL-")).map(|l| format!("{l}\n")).collect();
            vdir::content_hash(said.as_bytes())
        };
        assert_eq!(lesson_hash(&object(""), None), older(&object("")));
        let categorised = object("CATEGORIES:Coupe\r\n");
        assert_ne!(lesson_hash(&categorised, None), older(&categorised));
        let tie = contact_line("sioul-1", "Marie Exemple");
        assert_ne!(lesson_hash(&object(""), Some(&tie)), lesson_hash(&object(""), None));
        assert_ne!(lesson_hash(&object(""), Some(&tie)), lesson_hash(&object(""), Some(&contact_line("sioul-2", "Marie Exemple"))));
    }

    /// The door writes inside the chosen calendar's folder alone, whatever
    /// ids an extension hands back: an agenda's or a lesson's id holding
    /// separators, dots, a dash first, a path upwards.
    #[test]
    fn the_door_cannot_write_outside_its_calendar_whatever_the_ids() {
        const EXT: &str = "ids-test";
        let calendar = calendar("Door ids");
        for (agenda, uid) in [("../../calendars", "../../../escape@x"), ("-dash", "a/b\\c.ics"), ("a.b.c", "..\\..\\x"), ("pupil/../../x", "x/../../../../y")] {
            let mut reply = answer("pupil-1", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "")], true, &[]);
            reply.id = agenda.to_string();
            reply.calendar = reply.calendar.replace("L1@skolengo", uid);
            // Every write the door plans is a file of the calendar's own folder, of a safe name.
            let lessons = lessons(EXT, &reply, &words(), false, &BTreeMap::new()).unwrap();
            let planned = plan(&calendar.dir, &lessons, &BTreeMap::new(), &BTreeMap::new(), reply.read, &[]);
            for (path, ..) in &planned.write {
                assert_eq!(path.parent(), Some(calendar.dir.as_path()), "{agenda} {uid}");
                let name = path.file_name().unwrap().to_string_lossy();
                assert!(name.starts_with("ids-test-") && name.trim_end_matches(".ics").bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'), "{name}");
            }
            assert_eq!(take(EXT, &reply, &calendar, &words(), false).unwrap().written, 1, "{agenda} {uid}");
        }
        // Nothing of this extension anywhere else in the data folder, nor where the ids point.
        let mut stack = vec![crate::config::data_dir()];
        while let Some(at) = stack.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if !path.starts_with(&calendar.dir) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    assert!(!name.starts_with("ids-test-") && !name.contains("escape") && name != "y", "{}", path.display());
                }
            }
        }
        assert_eq!(files(&calendar.dir).keys().filter(|n| n.ends_with(".ics")).count(), 4);
    }

    /// Bounds counted before anything is written: an event heavier than a
    /// file may be, and nothing of the agenda is written.
    #[test]
    fn bounds_come_before_any_write() {
        const EXT: &str = "bounds-test";
        let calendar = calendar("Door bounds");
        let big = lesson("L2@skolengo", "20261118T080000Z", "20261118T090000Z", "History", &format!("DESCRIPTION:{}\r\n", "d".repeat(MOST_FILE_BYTES)));
        let reply = answer("pupil-1", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", ""), big], true, &[]);
        assert_eq!(take(EXT, &reply, &calendar, &words(), false), Err(Problem::TooLarge));
        assert!(files(&calendar.dir).keys().all(|n| !n.ends_with(".ics")), "nothing written, the small lesson neither");
    }

    /// The person's own word on an agenda (read only, its availability only) neither
    /// closes it to the door nor stops its writes (26 November 2026: whose
    /// an agenda is decides what reaches the person, who may write it is apart):
    /// the calendar chosen is written, changed and emptied by the door, while
    /// every other writer is still refused there. Only its server, a
    /// subscription, Sioul's busy blocks or its absence close it, each with
    /// its reason.
    #[test]
    fn the_person_s_own_word_on_an_agenda_does_not_close_it_to_the_door() {
        const EXT: &str = "kept-test";
        let config_path = crate::config::default_path();
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let maths = lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "");
        let art = lesson("L2@skolengo", "20261118T080000Z", "20261118T090000Z", "Art", "");
        for (name, access) in [("Door kept for busy times", crate::agendas::Access::Availability), ("Door kept read only", crate::agendas::Access::Read)] {
            let made = calendar(name);
            let place = crate::agendas::place(&made);
            crate::agendas::set_field(&config_path, &place, "access", access.id()).unwrap();
            let kept = calendar_at(&place).unwrap();
            assert!(kept.read_only, "{name}: Sioul's other writers see it read only");
            let config = crate::agendas::read_config();
            assert_eq!((closed(&kept, &config), kept_from_writers(&kept, &config)), (None, Some(access)), "{name}");
            assert!(writable(&kept), "{name}: offered and written");
            let done = take(EXT, &answer("pupil-1", &[maths.clone(), art.clone()], true, &[]), &kept, &words(), true).unwrap();
            assert_eq!((done.written, done.refused), (2, 0), "{name}");
            let changed = take(EXT, &answer("pupil-1", &[maths.replace("SUMMARY:Maths", "SUMMARY:Maths\r\nLOCATION:Hall")], true, &[]), &kept, &words(), true).unwrap();
            assert_eq!((changed.written, changed.removed, changed.refused), (1, 1, 0), "{name}: changed and taken out by the door");
            // The person's own rule holds for every other writer.
            let own = kept.dir.join("own.ics");
            assert!(vdir::write_item(&own, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:own@example.org\r\nDTSTART:20261117T080000Z\r\nSUMMARY:Mine\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").is_err(), "{name}: another writer refused");
            let lesson_file = kept.dir.join(crate::extensions::event_file(&crate::extensions::event_uid(EXT, "pupil-1", "L1@skolengo")));
            assert!(vdir::remove_item(&lesson_file).is_err(), "{name}: another remover refused");
            assert_eq!(take_out_of(&kept, EXT, "pupil-1"), (1, 0), "{name}: its lessons taken out of it on another choice");
        }
        // What closes a calendar to the door, each with its reason.
        let config = crate::agendas::read_config();
        let shared = calendar("Door shared read only");
        vdir::State { read_only: true, components: vec!["VEVENT".into()], ..vdir::State::default() }.save(&shared.state_path()).unwrap();
        let shared = calendar_at(&crate::agendas::place(&shared)).unwrap();
        assert_eq!(closed(&shared, &config), Some(Closed::ServerReadOnly));
        let subscribed = vdir::Collection { account: vdir::SUBSCRIBED.into(), ..shared.clone() };
        assert_eq!(closed(&subscribed, &config), Some(Closed::Subscribed));
        let gone = calendar("Door gone for good");
        std::fs::remove_dir_all(&gone.dir).unwrap();
        assert_eq!(closed(&gone, &config), Some(Closed::NotHere));
        assert!(!writable(&shared) && !writable(&subscribed) && !writable(&gone));
    }

    /// A calendar that can no longer be written (read only by its server,
    /// for tasks alone, a subscribed one, gone): nothing written, and the
    /// problem says to choose again.
    #[test]
    fn a_calendar_that_cannot_be_written_takes_nothing() {
        const EXT: &str = "gone-test";
        let reply = answer("pupil-1", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "")], true, &[]);
        let read_only = calendar("Door read only");
        let state = vdir::State { read_only: true, components: vec!["VEVENT".into()], ..vdir::State::default() };
        state.save(&read_only.state_path()).unwrap();
        let read_only = calendar_at(&crate::agendas::place(&read_only)).unwrap();
        assert_eq!(take(EXT, &reply, &read_only, &words(), false), Err(Problem::CalendarGone));
        let tasks = vdir::create(Kind::Calendars, LOCAL, "Door tasks", None, &["VTODO"]).unwrap();
        assert_eq!(take(EXT, &reply, &tasks, &words(), false), Err(Problem::CalendarGone));
        let gone = calendar("Door gone");
        std::fs::remove_dir_all(&gone.dir).unwrap();
        assert_eq!(take(EXT, &reply, &gone, &words(), false), Err(Problem::CalendarGone));
        assert!(files(&read_only.dir).keys().all(|n| !n.ends_with(".ics")) && files(&tasks.dir).keys().all(|n| !n.ends_with(".ics")));
    }

    /// A server that drops Sioul's own lines: the hash this device noted
    /// stands for the one the event lost, and nothing is written; another
    /// device that never wrote the lesson reads the hash on the event itself.
    #[test]
    fn a_hash_a_server_dropped_is_read_from_this_device_s_notes() {
        const EXT: &str = "dropped-test";
        let calendar = calendar("Door dropped");
        let reply = answer("pupil-2", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "")], true, &[]);
        take(EXT, &reply, &calendar, &words(), false).unwrap();
        let uid = crate::extensions::event_uid(EXT, "pupil-2", "L1@skolengo");
        let path = calendar.dir.join(crate::extensions::event_file(&uid));
        let stripped: String = lines::fold(&lines::unfold(&std::fs::read_to_string(&path).unwrap()).into_iter().filter(|l| !l.starts_with("X-SIOUL")).collect::<Vec<_>>());
        std::fs::write(&path, &stripped).unwrap();
        assert_eq!(take(EXT, &reply, &calendar, &words(), false).unwrap().written, 0, "the note stands for the line");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), stripped);
        // Another device: no note of its own, the line on the event as the first wrote it.
        let written = take(EXT, &answer("pupil-3", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "")], true, &[]), &calendar, &words(), false).unwrap();
        assert_eq!(written.written, 1);
        let mut notes = load_noted(EXT);
        notes.events.retain(|uid, _| !uid.contains(&crate::extensions::agenda_key(EXT, "pupil-3")));
        save_noted(EXT, &notes).unwrap();
        assert_eq!(take(EXT, &answer("pupil-3", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "")], true, &[]), &calendar, &words(), false).unwrap().written, 0, "the hash on the event agrees");
    }

    /// Remove's "take its lessons out" takes this extension's events alone,
    /// from every calendar, never another extension's nor the person's own.
    #[test]
    fn taking_out_takes_one_extension_s_events_alone() {
        let first = calendar("Door out one");
        let second = calendar("Door out two");
        let reply = answer("pupil-1", &[lesson("L1@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "")], true, &[]);
        take("timetable-out", &reply, &first, &words(), false).unwrap();
        take("timetable-out", &answer("pupil-2", &[lesson("L7@skolengo", "20261117T080000Z", "20261117T090000Z", "Maths", "")], true, &[]), &second, &words(), false).unwrap();
        take("other-out", &reply, &second, &words(), false).unwrap();
        // One agenda moved to another calendar: its lessons leave the first, the other agenda's stay.
        let third = calendar("Door out three");
        take("timetable-out", &answer("pupil-3", &[lesson("L8@skolengo", "20261117T080000Z", "20261117T090000Z", "Art", "")], true, &[]), &third, &words(), false).unwrap();
        take("timetable-out", &answer("pupil-1", &[lesson("L9@skolengo", "20261118T080000Z", "20261118T090000Z", "Art", "")], true, &[]), &third, &words(), false).unwrap();
        assert_eq!(take_out_of(&third, "timetable-out", "pupil-3"), (1, 0));
        assert_eq!(files(&third.dir).keys().filter(|n| n.ends_with(".ics")).count(), 1);
        assert_eq!(take_out("timetable-out"), (3, 0));
        assert!(files(&first.dir).keys().all(|n| !n.ends_with(".ics")));
        assert_eq!(files(&second.dir).keys().filter(|n| n.ends_with(".ics")).count(), 1, "the other extension's lesson stays");
        assert!(!noted_path("timetable-out").exists());
    }
}
