// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your availability, shared several ways for several audiences
//! (docs/client.md, "Sharing your availability"; docs/research.md, finding
//! 53; research/availability-sharing.md): your busy times, never what fills
//! them.
//!
//! An **availability** is what you give out of your time: the agendas that
//! count as busy, the margins kept around their events, one word per agenda
//! at most ("Work", for a partner, who reads a bare "busy" as secrecy:
//! Thayer et al. 2012, research/agendas.md §2.3), how coarse the blocks are
//! and how far ahead they go. Each has **shares**, one per audience: a
//! person, a group, a link, a booking tool. A share uses its server's own
//! way where one exists (Google's busy-only share, a server's free/busy
//! link, a booking page on your own server), which needs no Sioul running;
//! elsewhere, a **mirror**: a calendar of plain blocks Sioul keeps in an
//! account, one per availability and account, shared as often as needed.
//!
//! Kept in `[[agenda.availabilities]]`, travelling with the settings as one
//! list (the later list winning, as for the lists edited on one page). The
//! single share of before (`[agenda.availability]`, `Single`) is read once
//! and becomes one availability (`config::migrate_availability`).

use crate::agenda::Occurrence;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A day, in seconds.
const DAY: i64 = 86_400;
/// Days ahead by default, and at most.
pub const HORIZON: u32 = 60;
pub const HORIZON_MAX: u32 = 366;
/// How far past the horizon the closing block "Not known yet" reaches: a year.
const CLOSING_DAYS: i64 = 366;

/// One availability (`[[agenda.availabilities]]`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Availability {
    /// Stable, made once: random for a new one (`ids::new`), derived from
    /// what it was for one the migration made, so that two devices migrating
    /// at once write the same bytes.
    #[serde(default)]
    pub id: String,
    /// Your own name for it ("For Sam"), shown to you alone.
    #[serde(default)]
    pub name: String,
    /// What counts as busy, by place ("cloud/personal").
    #[serde(default)]
    pub agendas: Vec<String>,
    /// A word for an agenda's blocks instead of "Busy", by place.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub words: BTreeMap<String, String>,
    /// Minutes, outward: each block's start and end rounded to it, gaps shorter than it merged; 0 none.
    #[serde(default)]
    pub rounding: u32,
    /// Days ahead; unset (0), sixty.
    #[serde(default)]
    pub horizon: u32,
    /// A last block "Not known yet" from the horizon on (a booking tool's
    /// mirror), so that a tool offers nothing Sioul cannot vouch for.
    #[serde(default)]
    pub closing: bool,
    /// The calendars Sioul keeps for it, one per account, only where a share needs one.
    #[serde(default)]
    pub mirrors: Vec<Mirror>,
    /// To whom, and how.
    #[serde(default)]
    pub shares: Vec<Share>,
}

/// A calendar of blocks Sioul keeps for an availability in an account.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirror {
    pub account: String,
    /// The folder id Sioul made (`vdir::create`).
    pub calendar: String,
    /// Its server marked it transparent (`schedule-calendar-transp`): not counted as your own busy time.
    #[serde(default)]
    pub transparent: bool,
}

/// One share of an availability. Its words are kept as written, so that a
/// newer Sioul's audience or way never stops this one reading the settings
/// (`Audience::parse`, `Way::parse`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    /// person | group | link | booking.
    #[serde(default)]
    pub audience: String,
    /// A user, `user@instance`, an address, `group:<name>`; "" for a link or a booking page.
    #[serde(default)]
    pub who: String,
    /// How (`Way`).
    #[serde(default)]
    pub way: String,
    /// The account it lives on.
    #[serde(default)]
    pub account: String,
    /// The link, when the way has one (shown, with Copy).
    #[serde(default)]
    pub link: String,
    /// The day the server said yes ("2026-11-05"); "" while waiting.
    #[serde(default)]
    pub made: String,
}

impl Share {
    /// Its audience, when this Sioul knows the word.
    pub fn audience(&self) -> Option<Audience> {
        Audience::parse(&self.audience)
    }

    /// Its way, when this Sioul knows the word.
    pub fn way(&self) -> Option<Way> {
        Way::parse(&self.way)
    }

    /// Whether it names the same share as `other`: the same way, to the same, on the same account.
    pub fn same(&self, other: &Share) -> bool {
        self.way == other.way && self.account == other.account && self.who.trim().eq_ignore_ascii_case(other.who.trim())
    }
}

/// Who a share is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Audience {
    Person,
    Group,
    Link,
    Booking,
}

impl Audience {
    pub const ALL: [Audience; 4] = [Audience::Person, Audience::Group, Audience::Link, Audience::Booking];

    pub fn id(self) -> &'static str {
        match self {
            Audience::Person => "person",
            Audience::Group => "group",
            Audience::Link => "link",
            Audience::Booking => "booking",
        }
    }

    pub fn parse(text: &str) -> Option<Audience> {
        let text = text.trim().to_ascii_lowercase();
        Audience::ALL.into_iter().find(|a| a.id() == text)
    }
}

/// How a share is made (design-availability-sharing §1): its server's own
/// way first, a mirror only where no server can do it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Way {
    /// Google: each source calendar shared `freeBusyReader` with a Google user.
    GoogleFreeBusy,
    /// Google: the calendar's public free/busy.
    GooglePublic,
    /// A mirror, shared read only with a user or a group of its server.
    MirrorShare,
    /// A mirror, shared with `user@instance` of another Nextcloud (32 and later).
    MirrorFederated,
    /// A mirror's read-only public link.
    MirrorLink,
    /// Fastmail's own free/busy link of one calendar, which you paste.
    FastmailLink,
    /// Nextcloud Appointments, your booking page on your own server.
    Appointments,
    /// Google's appointment schedules.
    GoogleSchedule,
}

impl Way {
    pub const ALL: [Way; 8] = [Way::GoogleFreeBusy, Way::GooglePublic, Way::MirrorShare, Way::MirrorFederated, Way::MirrorLink, Way::FastmailLink, Way::Appointments, Way::GoogleSchedule];

    pub fn id(self) -> &'static str {
        match self {
            Way::GoogleFreeBusy => "google-freebusy",
            Way::GooglePublic => "google-public",
            Way::MirrorShare => "mirror-share",
            Way::MirrorFederated => "mirror-federated",
            Way::MirrorLink => "mirror-link",
            Way::FastmailLink => "fastmail-link",
            Way::Appointments => "appointments",
            Way::GoogleSchedule => "google-schedule",
        }
    }

    pub fn parse(text: &str) -> Option<Way> {
        let text = text.trim().to_ascii_lowercase();
        Way::ALL.into_iter().find(|w| w.id() == text)
    }

    /// Whether a mirror Sioul keeps carries it; else its server keeps it up to date.
    pub fn mirrored(self) -> bool {
        matches!(self, Way::MirrorShare | Way::MirrorFederated | Way::MirrorLink)
    }
}

/// How coarse and how far ahead, by the first audience (design §2, after
/// research §6.3: the wider the audience, the coarser the view); not shown
/// as settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    pub rounding: u32,
    pub horizon: u32,
    pub closing: bool,
}

/// A person or a group: exact, sixty days; a link: the quarter hour, two
/// weeks; a booking tool: the quarter hour, sixty days, and the closing block.
pub fn defaults(audience: Audience) -> Defaults {
    match audience {
        Audience::Person | Audience::Group => Defaults { rounding: 0, horizon: HORIZON, closing: false },
        Audience::Link => Defaults { rounding: 15, horizon: 14, closing: false },
        Audience::Booking => Defaults { rounding: 15, horizon: HORIZON, closing: true },
    }
}

impl Availability {
    /// A new availability for its first audience, its defaults taken (`defaults`), its id random.
    pub fn new(name: &str, agendas: Vec<String>, first: Audience) -> Availability {
        let d = defaults(first);
        Availability { id: crate::ids::new(), name: name.trim().to_string(), agendas, rounding: d.rounding, horizon: d.horizon, closing: d.closing, ..Availability::default() }
    }

    /// Days ahead: the horizon, sixty unset, a year at most.
    pub fn days_ahead(&self) -> u32 {
        if self.horizon == 0 { HORIZON } else { self.horizon.min(HORIZON_MAX) }
    }

    /// The rounding in seconds, a day at most.
    fn step(&self) -> i64 {
        i64::from(self.rounding.min(24 * 60)) * 60
    }

    /// Its mirror in an account.
    pub fn mirror(&self, account: &str) -> Option<&Mirror> {
        self.mirrors.iter().find(|m| m.account == account)
    }

    /// Whether a share in this account needs its mirror there.
    pub fn needs_mirror(&self, account: &str) -> bool {
        self.shares.iter().any(|s| s.account == account && s.way().is_some_and(Way::mirrored))
    }
}

/// What it was shared with before, by one person or by link (`[agenda.availability]`,
/// until 5 November 2026): read once, then made an availability (`migrated`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Single {
    /// The account it is shared from.
    pub account: String,
    /// The agendas it is made from, by place; on Google, the calendars shared busy only.
    #[serde(default)]
    pub agendas: Vec<String>,
    /// Whom: a user of the same Nextcloud, or an address (Google); "" by link alone.
    #[serde(default)]
    pub with: String,
    /// Shared by a read-only public link (Nextcloud).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub link: bool,
    /// A word for an agenda's blocks instead of "Busy", by place.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub words: BTreeMap<String, String>,
    /// The calendar Sioul made for the blocks, its folder's id; "" on Google.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub calendar: String,
    /// The share was made on the server.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shared: bool,
}

impl Single {
    /// Its id as an availability, the same on every device: a name-based
    /// UUID of what it was (`holders::uuid_v5`).
    pub fn migrated_id(&self) -> String {
        crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("availability\n{}\n{}\n{}\n{}", self.account, self.calendar, self.with.trim(), self.link))
    }

    /// The availability it becomes: named `name` ("For Sam", "By link"),
    /// its agendas and words kept, exact and sixty days; its calendar the
    /// mirror as it is (same folder, same name); one share, as it was made:
    /// on Google each calendar shared busy only, elsewhere the mirror shared
    /// with the person, or by its link (an empty `with`, as the window of
    /// before shared it). `today` is said as the day it was made when its
    /// server had said yes.
    pub fn migrated(&self, name: &str, today: &str) -> Availability {
        let google = self.calendar.is_empty();
        let with = self.with.trim().to_string();
        let (audience, way, who) = if google {
            (Audience::Person, Way::GoogleFreeBusy, with)
        } else if with.is_empty() {
            (Audience::Link, Way::MirrorLink, String::new())
        } else {
            (Audience::Person, Way::MirrorShare, with)
        };
        let mirrors = if google { Vec::new() } else { vec![Mirror { account: self.account.clone(), calendar: self.calendar.clone(), transparent: false }] };
        let share = Share { audience: audience.id().into(), who, way: way.id().into(), account: self.account.clone(), link: String::new(), made: if self.shared { today.to_string() } else { String::new() } };
        Availability {
            id: self.migrated_id(),
            name: name.trim().to_string(),
            agendas: self.agendas.clone(),
            words: self.words.iter().filter(|(_, w)| !w.trim().is_empty()).map(|(p, w)| (p.clone(), w.trim().to_string())).collect(),
            rounding: 0,
            horizon: HORIZON,
            closing: false,
            mirrors,
            shares: vec![share],
        }
    }

    /// The calendar's files as the window keeps them until its availability
    /// moves to the list (every block it is given, exact, margins counted):
    /// `mirror_files` for it read as one availability.
    pub fn wanted(&self, events: &[Occurrence], busy: &str) -> BTreeMap<String, String> {
        let availability = self.migrated("", "");
        let words = Words { busy, not_known: busy };
        blocks(events, &availability, &words, &BTreeSet::new(), (i64::MIN, i64::MAX)).iter().map(|b| (format!("{}.ics", block_uid(b)), block_text(b))).collect()
    }
}

/// The calendars of blocks Sioul keeps, as (account, folder id): derived from
/// your own events, they are no agendas of yours to show, remind, plan around
/// or meet in a conflict, so the agenda's events leave them out
/// (`agenda::occurrences`); Agenda ⚙ ▸ Your agendas still lists them.
pub fn mirror_places() -> Vec<(String, String)> {
    let config = crate::agendas::read_config();
    let mut places: Vec<(String, String)> = config.agenda.availabilities.iter().flat_map(|a| a.mirrors.iter()).filter(|m| !m.calendar.is_empty()).map(|m| (m.account.clone(), m.calendar.clone())).collect();
    // The single share of before, until it is read as an availability.
    if let Some(single) = config.agenda.availability.as_ref().filter(|a| !a.calendar.is_empty()) {
        places.push((single.account.clone(), single.calendar.clone()));
    }
    places.sort();
    places.dedup();
    places
}

/// One busy block: from, until (Unix seconds), what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub start: i64,
    pub end: i64,
    pub word: String,
}

/// The words a block may say, in the person's language: "Busy" (« Occupé »),
/// and the closing block's "Not known yet" (« Pas encore connu »).
#[derive(Debug, Clone, Copy)]
pub struct Words<'a> {
    pub busy: &'a str,
    pub not_known: &'a str,
}

/// The time an availability's blocks cover now: from the start of yesterday
/// to its horizon, in whole UTC days, so that two devices on the same day
/// read the same events and write the same blocks.
pub fn window(availability: &Availability, now: i64) -> (i64, i64) {
    let today = now.div_euclid(DAY) * DAY;
    (today - DAY, today + i64::from(availability.days_ahead()) * DAY)
}

/// The blocks of `events` within `window` (those of the agendas picked, that
/// take time: not cancelled, free or declined, never a whole day), each
/// widened by the margins kept around it (getting there is your time, and no
/// server knows it), rounded outward to the availability's rounding; each
/// says its agenda's word or "Busy", and "Busy" always for health
/// (`health`, by UID: `health_events`), whatever the agenda's word. Blocks
/// of one word that touch, overlap or lie closer than the rounding are made
/// one, so that nothing says how many things fill a time. With `closing`,
/// one last block "Not known yet" from the window's end on.
pub fn blocks(events: &[Occurrence], availability: &Availability, words: &Words, health: &BTreeSet<String>, window: (i64, i64)) -> Vec<Block> {
    let (from, to) = window;
    let step = availability.step();
    let outward = |start: i64, end: i64| if step > 0 { (start.div_euclid(step) * step, (end + step - 1).div_euclid(step) * step) } else { (start, end) };
    let word_of = |e: &Occurrence| {
        if health.contains(&e.uid) {
            return words.busy.to_string();
        }
        availability.words.get(&e.agenda).map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).unwrap_or_else(|| words.busy.to_string())
    };
    let mut found: Vec<Block> = events
        .iter()
        .filter(|e| availability.agendas.iter().any(|a| *a == e.agenda) && !e.cancelled && !e.transparent && !e.declined && !e.all_day && e.end > e.start && e.end > from && e.start < to)
        .map(|e| {
            let (start, end) = outward(e.start - i64::from(e.margins.before.min(24 * 60)) * 60, e.end + i64::from(e.margins.after.min(24 * 60)) * 60);
            Block { start, end, word: word_of(e) }
        })
        .collect();
    found.sort_by(|a, b| (&a.word, a.start, a.end).cmp(&(&b.word, b.start, b.end)));
    let mut merged: Vec<Block> = Vec::new();
    for block in found {
        match merged.last_mut() {
            // Touching or overlapping; closer than the rounding.
            Some(last) if last.word == block.word && (block.start <= last.end || block.start - last.end < step) => last.end = last.end.max(block.end),
            _ => merged.push(block),
        }
    }
    merged.sort_by(|a, b| (a.start, a.end, &a.word).cmp(&(b.start, b.end, &b.word)));
    if availability.closing {
        merged.push(Block { start: to, end: to + CLOSING_DAYS * DAY, word: words.not_known.to_string() });
    }
    merged
}

/// The events Sioul knows as health, by UID, which never take an agenda's
/// word (research §6.3): a child's event of the health or dentist kind
/// (`holders::kind_of`, as `consent::Consent::keep_agendas` reads it), your
/// stand-in of one (`stands_for`), an errand's time block or appointment
/// (its task among `errands`: `errand_tasks`; a title Sioul gives an errand,
/// `health::is_errand_title`).
pub fn health_events(events: &[Occurrence], errands: &BTreeSet<String>) -> BTreeSet<String> {
    let mut read: BTreeMap<&str, bool> = BTreeMap::new();
    let mut child_health: BTreeSet<String> = BTreeSet::new();
    for event in events.iter().filter(|e| e.child && !e.key.is_empty()) {
        let health = *read.entry(event.key.as_str()).or_insert_with(|| {
            std::fs::read_to_string(&event.key).ok().and_then(|text| crate::holders::kind_of(&text)).is_some_and(|k| matches!(k, crate::holders::Kind::Health | crate::holders::Kind::Dentist))
        });
        if health {
            child_health.insert(event.uid.clone());
        }
    }
    let mut out = child_health.clone();
    for event in events {
        let errand = (!event.task.is_empty() && errands.contains(&event.task)) || crate::health::is_errand_title(&event.summary);
        if errand || (!event.stands_for.is_empty() && child_health.contains(&event.stands_for)) {
            out.insert(event.uid.clone());
        }
    }
    out
}

/// The errands' tasks and their steps however deep, by UID (`health::is_errand`,
/// `recorded` the UIDs the record keeps): whose time blocks are health.
pub fn errand_tasks(tasks: &[crate::tasks::Task], recorded: &BTreeSet<String>) -> BTreeSet<String> {
    let errands: BTreeSet<&str> = tasks.iter().filter(|t| crate::health::is_errand(t, recorded)).map(|t| t.uid.as_str()).collect();
    let parent: BTreeMap<&str, &str> = tasks.iter().filter_map(|t| t.relations.iter().find(|r| r.kind.eq_ignore_ascii_case("PARENT")).map(|r| (t.uid.as_str(), r.uid.as_str()))).collect();
    let mut out: BTreeSet<String> = recorded.clone();
    for task in tasks {
        let mut seen = BTreeSet::new();
        let mut at = task.uid.as_str();
        loop {
            if errands.contains(at) {
                out.insert(task.uid.clone());
                break;
            }
            match parent.get(at).copied().filter(|up| seen.insert(*up)) {
                Some(up) => at = up,
                None => break,
            }
        }
    }
    out
}

/// A block's UID, the same on every device (a name-based UUID, `holders::uuid_v5`).
pub fn block_uid(block: &Block) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("busy\n{}\n{}\n{}", block.start, block.end, block.word))
}

/// A block's file: its time in UTC, its word, busy, public; its stamp its start, so that two devices write the same bytes.
pub fn block_text(block: &Block) -> String {
    let utc = crate::moves::utc;
    crate::lines::fold(&[
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Sioul//Sioul//EN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{}", block_uid(block)),
        format!("DTSTAMP:{}", utc(block.start)),
        format!("DTSTART:{}", utc(block.start)),
        format!("DTEND:{}", utc(block.end)),
        format!("SUMMARY:{}", crate::lines::escape(&block.word)),
        "TRANSP:OPAQUE".to_string(),
        "CLASS:PUBLIC".to_string(),
        "END:VEVENT".to_string(),
        "END:VCALENDAR".to_string(),
    ])
}

/// The files a mirror of `availability` holds, by name (`<uid>.ics`): what it
/// should hold at `now`, from `events` (read over `window`), the same on
/// every device that read the same events the same day.
pub fn mirror_files(events: &[Occurrence], availability: &Availability, words: &Words, health: &BTreeSet<String>, now: i64) -> BTreeMap<String, String> {
    blocks(events, availability, words, health, window(availability, now)).iter().map(|b| (format!("{}.ics", block_uid(b)), block_text(b))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDS: Words = Words { busy: "Busy", not_known: "Not known yet" };
    const ALL: (i64, i64) = (i64::MIN, i64::MAX);

    fn event(agenda: &str, uid: &str, start: i64, end: i64, title: &str) -> Occurrence {
        Occurrence { agenda: agenda.into(), uid: uid.into(), start, end, summary: title.into(), location: "Rue Blanche".into(), ..Occurrence::default() }
    }

    fn mine(agendas: &[&str]) -> Availability {
        Availability { id: "a1".into(), agendas: agendas.iter().map(|a| a.to_string()).collect(), ..Availability::default() }
    }

    /// The blocks: the agendas picked alone, what takes time alone, each
    /// "Busy" or its agenda's word, those of one word that touch made one;
    /// their files say nothing of what fills them.
    #[test]
    fn busy_blocks_say_nothing_of_what_fills_them() {
        let events = [
            event("me/personal", "d", 1_000, 2_000, "Dentist"),
            event("me/personal", "l", 2_000, 3_000, "Lunch with Tom"),
            event("work/main", "b", 2_500, 4_000, "Board"),
            Occurrence { cancelled: true, ..event("me/personal", "c", 5_000, 6_000, "Cancelled") },
            Occurrence { transparent: true, ..event("me/personal", "f", 7_000, 8_000, "Free") },
            Occurrence { all_day: true, ..event("me/personal", "h", 0, 86_400, "Holiday") },
            Occurrence { declined: true, ..event("me/personal", "n", 10_000, 11_000, "Declined") },
            event("me/hidden", "p", 9_000, 9_500, "Not picked"),
        ];
        let availability = Availability { words: [("work/main".to_string(), "Work".to_string())].into(), ..mine(&["me/personal", "work/main"]) };
        let found = blocks(&events, &availability, &WORDS, &BTreeSet::new(), ALL);
        assert_eq!(found, [Block { start: 1_000, end: 3_000, word: "Busy".into() }, Block { start: 2_500, end: 4_000, word: "Work".into() }]);
        let files = mirror_files(&events, &availability, &WORDS, &BTreeSet::new(), 0);
        assert_eq!(files.len(), 2);
        let all: String = files.values().cloned().collect();
        for kept in ["Dentist", "Lunch", "Board", "Rue Blanche"] {
            assert!(!all.contains(kept), "{kept}: {all}");
        }
        assert!(all.contains("SUMMARY:Busy") && all.contains("SUMMARY:Work") && all.contains("CLASS:PUBLIC"));
    }

    /// The margins kept around an event are busy time; the rounding widens
    /// each block outward to the quarter hour and merges what lies closer
    /// than it, so that a fifty-minute session reads as an hour among others.
    #[test]
    fn margins_count_and_rounding_widens_outward() {
        let at = |h: i64, m: i64| 1_800_000_000 / DAY * DAY + h * 3600 + m * 60;
        let session = Occurrence { margins: crate::demands::Margins { before: 20, after: 10 }, ..event("me/personal", "s", at(17, 0), at(17, 50), "Session") };
        let exact = mine(&["me/personal"]);
        assert_eq!(blocks(std::slice::from_ref(&session), &exact, &WORDS, &BTreeSet::new(), ALL), [Block { start: at(16, 40), end: at(18, 0), word: "Busy".into() }], "the margins, exact");
        let coarse = Availability { rounding: 15, ..exact.clone() };
        let plain = event("me/personal", "x", at(9, 7), at(9, 57), "Fifty minutes");
        assert_eq!(blocks(std::slice::from_ref(&plain), &coarse, &WORDS, &BTreeSet::new(), ALL), [Block { start: at(9, 0), end: at(10, 0), word: "Busy".into() }], "outward to the quarter hour");
        // Ten minutes apart: one block once rounded; exact, two.
        let near = [event("me/personal", "a", at(11, 0), at(11, 50), "A"), event("me/personal", "b", at(12, 0), at(12, 30), "B")];
        assert_eq!(blocks(&near, &coarse, &WORDS, &BTreeSet::new(), ALL), [Block { start: at(11, 0), end: at(12, 30), word: "Busy".into() }]);
        assert_eq!(blocks(&near, &exact, &WORDS, &BTreeSet::new(), ALL).len(), 2);
    }

    /// Health is never named: an event known as health says "Busy" in an
    /// agenda that has a word; a child's health event, a stand-in of it, an
    /// errand's block and an errand's steps are known so, nothing else.
    #[test]
    fn health_takes_busy_never_the_agendas_word() {
        let availability = Availability { words: [("work/main".to_string(), "Work".to_string())].into(), ..mine(&["work/main"]) };
        let events = [event("work/main", "meeting", 1_000, 2_000, "Board"), event("work/main", "therapy", 5_000, 6_000, "Session")];
        let health: BTreeSet<String> = ["therapy".to_string()].into();
        assert_eq!(blocks(&events, &availability, &WORDS, &health, ALL), [Block { start: 1_000, end: 2_000, word: "Work".into() }, Block { start: 5_000, end: 6_000, word: "Busy".into() }]);
        crate::config::stand_ins_only();
        let dir = crate::config::state_dir().join("availability-health-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("tom-dentist.ics");
        std::fs::write(&file, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:tom-dentist\r\nDTSTART:20991104T140000Z\r\nDTEND:20991104T150000Z\r\nSUMMARY:Dentist\r\nX-SIOUL-CONCEPT;VALUE=URI:tag:aurelienpierre.com,2026:sioul/child-kind/dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let child = Occurrence { child: true, key: file.display().to_string(), ..event("me/tom", "tom-dentist", 1, 2, "Dentist") };
        let stand_in = Occurrence { stands_for: "tom-dentist".into(), ..event("me/personal", "stand-in", 1, 2, "Dentist") };
        let block = Occurrence { task: "errand".into(), ..event("me/personal", "block", 3, 4, "Pick up") };
        let other = Occurrence { task: "groceries".into(), ..event("me/personal", "other", 5, 6, "Groceries") };
        let known = health_events(&[child, stand_in, block, other], &["errand".to_string()].into());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(known, ["block", "stand-in", "tom-dentist"].iter().map(|s| s.to_string()).collect::<BTreeSet<_>>());
        // A step of an errand, however deep: its block is health too.
        let task = |uid: &str, parent: &str| crate::tasks::Task { uid: uid.into(), title: uid.into(), relations: if parent.is_empty() { Vec::new() } else { vec![crate::tasks::Relation { kind: "PARENT".into(), uid: parent.into(), gap: 0 }] }, ..crate::tasks::Task::default() };
        let tasks = [task("renew", ""), task("call", "renew"), task("ask", "call"), task("shop", "")];
        let errands = errand_tasks(&tasks, &["renew".to_string()].into());
        assert_eq!(errands, ["ask", "call", "renew"].iter().map(|s| s.to_string()).collect::<BTreeSet<_>>());
    }

    /// The horizon and the closing block: a link's two weeks, a booking
    /// tool's "Not known yet" from its sixty days on; the window in whole UTC
    /// days, so that two devices the same day, reading the events in any
    /// order, write the same bytes.
    #[test]
    fn the_horizon_closes_and_two_devices_write_the_same_bytes() {
        let now = 1_800_000_000;
        let today = now / DAY * DAY;
        let link = Availability { horizon: defaults(Audience::Link).horizon, rounding: 15, ..mine(&["me/personal"]) };
        assert_eq!(window(&link, now), (today - DAY, today + 14 * DAY));
        let events = vec![event("me/personal", "near", today + 3600, today + 7200, "Near"), event("me/personal", "far", today + 20 * DAY, today + 20 * DAY + 3600, "Far"), event("me/personal", "past", today - 3 * DAY, today - 3 * DAY + 60, "Past")];
        let files = mirror_files(&events, &link, &WORDS, &BTreeSet::new(), now);
        assert_eq!(files.len(), 1, "beyond the two weeks and before yesterday: left out");
        let booking = Availability { rounding: 15, horizon: 60, closing: true, ..mine(&["me/personal"]) };
        let found = blocks(&events, &booking, &WORDS, &BTreeSet::new(), window(&booking, now));
        let last = found.last().unwrap();
        assert_eq!((last.start, last.word.as_str()), (today + 60 * DAY, "Not known yet"), "closed from the horizon on");
        // Two devices: the same day, the events in another order.
        let mut reversed = events.clone();
        reversed.reverse();
        assert_eq!(mirror_files(&reversed, &booking, &WORDS, &BTreeSet::new(), today + 80_000), mirror_files(&events, &booking, &WORDS, &BTreeSet::new(), now));
        assert_eq!(defaults(Audience::Person), Defaults { rounding: 0, horizon: 60, closing: false });
        assert_eq!(defaults(Audience::Booking), Defaults { rounding: 15, horizon: 60, closing: true });
    }

    /// The words of a share read back as this Sioul knows them; a newer one is kept, never read as another.
    #[test]
    fn ways_and_audiences_read_back_and_a_newer_word_is_kept() {
        for way in Way::ALL {
            assert_eq!(Way::parse(way.id()), Some(way));
        }
        for audience in Audience::ALL {
            assert_eq!(Audience::parse(audience.id()), Some(audience));
        }
        let newer = Share { audience: "family".into(), way: "carrier-pigeon".into(), ..Share::default() };
        assert_eq!((newer.audience(), newer.way()), (None, None));
        let availability = Availability { shares: vec![Share { way: "mirror-link".into(), account: "cloud".into(), ..Share::default() }, Share { way: "google-freebusy".into(), account: "google".into(), ..Share::default() }], ..mine(&[]) };
        assert!(availability.needs_mirror("cloud") && !availability.needs_mirror("google"));
    }
}
