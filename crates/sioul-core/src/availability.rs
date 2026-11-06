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

// ---------------------------------------------------------------- who keeps a mirror: the freshest view

/// A device's view of an availability's sources: when it last knew each of
/// them whole and current (`vdir::State::pulled`: a pull gone through whole,
/// or a token check that found nothing changed), as (oldest, newest), Unix
/// seconds on its clock; none while one was never pulled whole here.
pub fn view(pulled: &[Option<i64>]) -> Option<(i64, i64)> {
    if pulled.is_empty() || pulled.iter().any(Option::is_none) {
        return None;
    }
    let times = pulled.iter().flatten().copied();
    Some((times.clone().min()?, times.max()?))
}

/// What a device notes in its sealed entry once it wrote a mirror from `view`
/// (`devices::Entry::mirrors`): the freshest pull it relied on.
pub fn as_of(view: (i64, i64)) -> i64 {
    view.1
}

/// The freshest-view rule (design-availability-sharing §3): a device writes
/// an availability's mirror only when every source of it was known current
/// here after the freshest pull any other device last wrote it from
/// (`others`: their `as_of`). So a device opened after days away, or a phone
/// whose steps were held, writes nothing until its own pulls are newer than
/// what the mirror already says; then it writes the same bytes as the others
/// would. Its own earlier writes do not count: its view only moves forward.
/// Comparing the oldest pull here with the newest there, rather than oldest
/// with oldest, keeps a device whose sources were pulled unevenly from
/// writing an older copy of one of them over a newer one.
pub fn may_write(view: Option<(i64, i64)>, others: impl IntoIterator<Item = i64>) -> bool {
    let Some((oldest, _)) = view else { return false };
    others.into_iter().all(|theirs| oldest >= theirs)
}

// ---------------------------------------------------------------- the way per audience and server

/// What an account's server is, as Sioul knows it: by its sign-in (Google),
/// by what its DAV header says (Nextcloud's sharing, `calendars::Sharer`), by its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerKind {
    Google,
    Nextcloud,
    Fastmail,
    Other,
}

/// One account a share can live on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub account: String,
    pub kind: ServerKind,
    /// Its host, "cloud.example.org"; for Google, the account's address.
    pub host: String,
    /// It shares a calendar with its own users (Nextcloud's DAV sharing, Google's ACL).
    pub shares: bool,
    /// It publishes a calendar by a read-only link.
    pub publishes: bool,
}

/// A way Sioul would use for an audience, on an account, and why, in one
/// sentence (`why`, a message id taking `who` and `server`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub way: Way,
    pub account: String,
    /// Whom, as the server will name them: a user, `group:<name>`, a cloud id, an address; "" for a link.
    pub who: String,
    pub why: &'static str,
}

impl Choice {
    /// The share it makes, waiting for its server's yes.
    pub fn share(&self, audience: Audience) -> Share {
        Share { audience: audience.id().into(), who: self.who.clone(), way: self.way.id().into(), account: self.account.clone(), link: String::new(), made: String::new() }
    }
}

/// What Sioul says when no way fits: "This server cannot share busy times.",
/// "Only a Nextcloud shares with a group."
pub const NO_WAY: &str = "availability-way-none";
pub const NO_GROUP: &str = "availability-way-no-group";

/// The ways Sioul would use to share `availability` with `audience` (`who`
/// for a person or a group), best first (design-availability-sharing §1):
/// its server's own way where one fits, which needs no Sioul running and
/// never goes stale; a mirror only where none does. The window offers the
/// first and names it with its reason; the others stay a choice away.
///
/// - **A person**, by a name alone: a user of a server that shares (a
///   Nextcloud), the mirror shared with them there. By `name@host` of one
///   of your Nextclouds: that user, the same. By another address: on Google,
///   your calendars shared busy only when the availability is all in that
///   Google account and has no word (Google then shows the events' own
///   times, without your margins), else a mirror there shared busy only;
///   from a Nextcloud, the mirror federated to that cloud id (Nextcloud 32
///   and later); else a mirror's link to give them. An address of Google's
///   own mail, or of your Google account's domain, takes Google first;
///   another, a Nextcloud's federation first.
/// - **A group**: a Nextcloud's group, the mirror shared with it.
/// - **Anyone with a link**: Google's public free/busy, or Fastmail's own
///   link, when the availability is that one calendar and has no word; else
///   a mirror's link on a server that publishes.
/// - **A booking tool**: your booking page on your own server (Google's
///   appointment schedules, Nextcloud Appointments) where your agendas are;
///   else a mirror's link, which the tool reads as a feed.
pub fn choose(availability: &Availability, audience: Audience, who: &str, servers: &[Server]) -> Result<Vec<Choice>, &'static str> {
    let who = who.trim();
    let accounts: BTreeSet<&str> = availability.agendas.iter().filter_map(|place| place.split_once('/').map(|(account, _)| account)).collect();
    let plain = availability.words.values().all(|w| w.trim().is_empty());
    let all_in = |server: &Server| !accounts.is_empty() && accounts.iter().all(|a| *a == server.account);
    let holds = |server: &Server| accounts.contains(server.account.as_str());
    let of = |kind: ServerKind| servers.iter().filter(move |s| s.kind == kind);
    let choice = |way: Way, server: &Server, who: &str, why: &'static str| Choice { way, account: server.account.clone(), who: who.to_string(), why };
    let link_on = |out: &mut Vec<Choice>, why: &'static str| {
        // A Nextcloud first, then any server that publishes, Google's mirror made public last.
        let mut publishing: Vec<&Server> = servers.iter().filter(|s| s.publishes && s.kind != ServerKind::Google).collect();
        publishing.sort_by_key(|s| s.kind != ServerKind::Nextcloud);
        publishing.extend(of(ServerKind::Google));
        out.extend(publishing.into_iter().map(|s| choice(Way::MirrorLink, s, "", why)));
    };
    let mut out: Vec<Choice> = Vec::new();
    match audience {
        Audience::Person if who.is_empty() => return Err(NO_WAY),
        Audience::Person => match who.rsplit_once('@') {
            // A name alone: a user of a server that shares, where your agendas are first.
            None => {
                let mut sharing: Vec<&Server> = servers.iter().filter(|s| s.shares && s.kind != ServerKind::Google).collect();
                sharing.sort_by_key(|s| (s.kind != ServerKind::Nextcloud, !holds(s)));
                out.extend(sharing.into_iter().map(|s| choice(Way::MirrorShare, s, who, "availability-way-mirror-share")));
            }
            Some((user, host)) => {
                if let Some(server) = servers.iter().find(|s| s.shares && s.kind == ServerKind::Nextcloud && s.host.eq_ignore_ascii_case(host)) {
                    out.push(choice(Way::MirrorShare, server, user, "availability-way-mirror-share"));
                }
                let host = host.to_ascii_lowercase();
                let mut google: Vec<Choice> = Vec::new();
                for server in of(ServerKind::Google) {
                    if plain && all_in(server) {
                        google.push(choice(Way::GoogleFreeBusy, server, who, "availability-way-google-freebusy"));
                    }
                    google.push(choice(Way::MirrorShare, server, who, "availability-way-google-mirror"));
                }
                let federated: Vec<Choice> = if valid_cloud_id(who) { of(ServerKind::Nextcloud).filter(|s| s.shares).map(|s| choice(Way::MirrorFederated, s, who, "availability-way-mirror-federated")).collect() } else { Vec::new() };
                let google_mail = ["gmail.com", "googlemail.com"].contains(&host.as_str()) || of(ServerKind::Google).any(|s| s.host.rsplit_once('@').is_some_and(|(_, own)| own.eq_ignore_ascii_case(&host)));
                if google_mail {
                    out.extend(google);
                    out.extend(federated);
                } else {
                    out.extend(federated);
                    out.extend(google);
                }
                link_on(&mut out, "availability-way-mirror-link-person");
            }
        },
        Audience::Group => {
            let name = who.strip_prefix("group:").unwrap_or(who).trim();
            if name.is_empty() {
                return Err(NO_GROUP);
            }
            out.extend(of(ServerKind::Nextcloud).filter(|s| s.shares).map(|s| choice(Way::MirrorShare, s, &format!("group:{name}"), "availability-way-mirror-group")));
            if out.is_empty() {
                return Err(NO_GROUP);
            }
        }
        Audience::Link => {
            if plain && availability.agendas.len() == 1 {
                for server in servers.iter().filter(|s| all_in(s)) {
                    match server.kind {
                        ServerKind::Google => out.push(choice(Way::GooglePublic, server, "", "availability-way-google-public")),
                        ServerKind::Fastmail => out.push(choice(Way::FastmailLink, server, "", "availability-way-fastmail-link")),
                        _ => {}
                    }
                }
            }
            link_on(&mut out, "availability-way-mirror-link");
        }
        Audience::Booking => {
            for server in servers.iter().filter(|s| holds(s)) {
                match server.kind {
                    ServerKind::Google => out.push(choice(Way::GoogleSchedule, server, "", "availability-way-google-schedule")),
                    ServerKind::Nextcloud => out.push(choice(Way::Appointments, server, "", "availability-way-appointments")),
                    _ => {}
                }
            }
            link_on(&mut out, "availability-way-mirror-link-booking");
        }
    }
    out.dedup_by(|a, b| a.way == b.way && a.account == b.account && a.who == b.who);
    if out.is_empty() { Err(NO_WAY) } else { Ok(out) }
}

/// Every reason `choose` may give, and its refusals: each a message in every language.
pub const REASONS: [&str; 14] = [
    "availability-way-mirror-share",
    "availability-way-google-freebusy",
    "availability-way-google-mirror",
    "availability-way-mirror-federated",
    "availability-way-mirror-link-person",
    "availability-way-mirror-group",
    "availability-way-google-public",
    "availability-way-fastmail-link",
    "availability-way-mirror-link",
    "availability-way-google-schedule",
    "availability-way-appointments",
    "availability-way-mirror-link-booking",
    NO_WAY,
    NO_GROUP,
];

/// Whether a text reads as a Nextcloud cloud id ("sam@cloud.example.org"):
/// a name and a server, no space, no slash (`calendars::federated_principal`).
fn valid_cloud_id(text: &str) -> bool {
    text.rsplit_once('@').is_some_and(|(user, server)| !user.is_empty() && server.contains(['.', ':']) && !text.chars().any(|c| c.is_whitespace() || c.is_control() || c == '/'))
}

// ---------------------------------------------------------------- your working hours, for a server's free/busy

/// "09:00" as hours and minutes; "24:00" ends a day.
fn clock(text: &str) -> Option<(u32, u32)> {
    let (hour, minute) = text.trim().split_once(':')?;
    let (hour, minute): (u32, u32) = (hour.trim().parse().ok()?, minute.trim().parse().ok()?);
    (minute < 60 && (hour < 24 || (hour == 24 && minute == 0))).then_some((hour, minute))
}

/// Your working hours (Settings ▸ Hours: the windows for work) as a
/// server's free/busy reads them (RFC 7953): one VAVAILABILITY in your time
/// zone, one AVAILABLE per span of the day repeating weekly on its days, as
/// Nextcloud's own settings page groups them (`slotsToVavailability`,
/// @nextcloud/calendar-availability-vue, read 5 November 2026), so that
/// its page shows them back. Its dates fall in one fixed week and its UIDs
/// derive from what it says, so that two devices write the same bytes.
/// None without working hours: the server's property is then taken out,
/// never written empty, which RFC 7953 reads as never available.
pub fn working_hours(windows: &[crate::window::AdminWindow], zone_name: &str, zone: &jiff::tz::TimeZone) -> Option<String> {
    const DAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
    let mut spans: BTreeMap<((u32, u32), (u32, u32)), BTreeSet<usize>> = BTreeMap::new();
    for window in windows.iter().filter(|w| w.kind() == "work") {
        let (Some(day), Some(start), Some(end)) = (window.weekday(), clock(&window.start), clock(&window.end_text())) else { continue };
        if start.0 == 24 || start == end {
            continue;
        }
        spans.entry((start, end)).or_default().insert(day.to_monday_zero_offset() as usize);
    }
    if spans.is_empty() {
        return None;
    }
    // The week of Monday 1 January 2024.
    let day = |offset: usize| jiff::civil::date(2024, 1, 1).checked_add(jiff::Span::new().days(offset as i64)).unwrap_or(jiff::civil::date(2024, 1, 1)).strftime("%Y%m%d").to_string();
    let utc = zone_name.is_empty() || zone_name.eq_ignore_ascii_case("UTC");
    let at = |offset: usize, (hour, minute): (u32, u32)| {
        let (offset, hour) = if hour == 24 { (offset + 1, 0) } else { (offset, hour) };
        if utc { format!(":{}T{hour:02}{minute:02}00Z", day(offset)) } else { format!(";TZID={zone_name}:{}T{hour:02}{minute:02}00", day(offset)) }
    };
    let stamp = "DTSTAMP:20240101T000000Z".to_string();
    let mut lines = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string()];
    if !utc {
        lines.extend(crate::agenda::vtimezone(zone, zone_name, 2024));
    }
    let said: String = spans.iter().map(|((s, e), days)| format!("{s:?}-{e:?}-{days:?};")).collect();
    lines.extend(["BEGIN:VAVAILABILITY".to_string(), format!("UID:{}", crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("working-hours\n{zone_name}\n{said}"))), stamp.clone()]);
    for ((start, end), days) in &spans {
        let first = *days.iter().next().unwrap_or(&0);
        // An end before its start ends the next day.
        let end_day = if end <= start { first + 1 } else { first };
        let byday: Vec<&str> = days.iter().map(|d| DAYS[*d]).collect();
        lines.extend([
            "BEGIN:AVAILABLE".to_string(),
            format!("UID:{}", crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("working-hours\n{zone_name}\n{start:?}\n{end:?}\n{}", byday.join(",")))),
            stamp.clone(),
            format!("DTSTART{}", at(first, *start)),
            format!("DTEND{}", at(end_day, *end)),
            format!("RRULE:FREQ=WEEKLY;BYDAY={}", byday.join(",")),
            "END:AVAILABLE".to_string(),
        ]);
    }
    lines.extend(["END:VAVAILABILITY".to_string(), "END:VCALENDAR".to_string()]);
    Some(crate::lines::fold(&lines))
}

/// A mirror's description, the reader's only word on how fresh it is: a
/// date, never a time, which would tell when your devices run
/// (research/availability-sharing.md §6.1): "From Sioul. As of 5 November."
pub fn mirror_description(tr: &crate::i18n::Translator, today: jiff::civil::Date) -> String {
    let mut args = crate::i18n::args();
    args.set("day", today.day());
    args.set("month", tr.text(&format!("month-{}", today.month()), None));
    tr.text("availability-mirror-description", Some(&args))
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

    /// Your working hours as Nextcloud's settings page writes them: one
    /// AVAILABLE per span of the day, its days together, in your zone, in a
    /// fixed week; an evening past midnight ending the next day; admin hours
    /// left out; the same bytes whatever the order; none without working hours.
    #[test]
    fn working_hours_read_as_nextcloud_s_settings_write_them() {
        use crate::window::AdminWindow;
        let w = |day: &str, start: &str, end: &str, kind: Option<&str>| AdminWindow { day: day.into(), start: start.into(), end: Some(end.into()), minutes: 0, kind: kind.map(str::to_string) };
        let paris = jiff::tz::TimeZone::get("Europe/Paris").unwrap();
        let windows = vec![w("monday", "09:00", "17:00", None), w("tuesday", "09:00", "17:00", None), w("friday", "09:00", "12:00", None), w("saturday", "10:00", "12:00", Some("admin")), w("wednesday", "22:00", "02:00", None)];
        let text = working_hours(&windows, "Europe/Paris", &paris).unwrap();
        let flat = text.replace("\r\n ", "");
        for line in ["BEGIN:VTIMEZONE", "TZID:Europe/Paris", "BEGIN:VAVAILABILITY", "DTSTART;TZID=Europe/Paris:20240101T090000", "DTEND;TZID=Europe/Paris:20240101T170000", "RRULE:FREQ=WEEKLY;BYDAY=MO,TU", "DTSTART;TZID=Europe/Paris:20240105T090000", "RRULE:FREQ=WEEKLY;BYDAY=FR", "DTSTART;TZID=Europe/Paris:20240103T220000", "DTEND;TZID=Europe/Paris:20240104T020000"] {
            assert!(flat.contains(line), "{line}: {flat}");
        }
        assert_eq!((flat.matches("BEGIN:AVAILABLE").count(), flat.contains("BYDAY=SA")), (3, false), "{flat}");
        let mut reversed = windows.clone();
        reversed.reverse();
        assert_eq!(working_hours(&reversed, "Europe/Paris", &paris), Some(text), "two devices, the same bytes");
        assert!(working_hours(&windows, "UTC", &jiff::tz::TimeZone::UTC).unwrap().contains("DTSTART:20240101T090000Z"));
        assert!(working_hours(&[w("monday", "18:00", "24:00", None)], "UTC", &jiff::tz::TimeZone::UTC).unwrap().contains("DTEND:20240102T000000Z"), "midnight ends the day");
        assert_eq!(working_hours(&windows[3..4], "Europe/Paris", &paris), None, "admin hours alone: none");
        assert_eq!(working_hours(&[], "Europe/Paris", &paris), None);
    }

    /// The way: the server's own first, a mirror only where none fits; by
    /// whom, by server, by how many agendas and whether a word is said; each
    /// reason a sentence in every language.
    #[test]
    fn the_way_is_the_server_s_own_first_and_a_mirror_only_where_none_fits() {
        let server = |account: &str, kind: ServerKind, host: &str, shares: bool, publishes: bool| Server { account: account.into(), kind, host: host.into(), shares, publishes };
        let cloud = server("cloud", ServerKind::Nextcloud, "cloud.example.org", true, true);
        let google = server("g", ServerKind::Google, "alex@example.org", true, true);
        let fastmail = server("fm", ServerKind::Fastmail, "caldav.fastmail.com", false, false);
        let icloud = server("ic", ServerKind::Other, "caldav.icloud.com", false, false);
        let ways = |a: &Availability, audience: Audience, who: &str, servers: &[Server]| choose(a, audience, who, servers).map(|c| c.into_iter().map(|c| (c.way, c.account, c.who)).collect::<Vec<_>>());
        let w = |way: Way, account: &str, who: &str| (way, account.to_string(), who.to_string());
        let mine = Availability { agendas: vec!["cloud/personal".into()], ..Availability::default() };
        let both = [cloud.clone(), google.clone()];
        // A person, by a name: a user of the Nextcloud.
        assert_eq!(ways(&mine, Audience::Person, "sam", &both).unwrap()[0], w(Way::MirrorShare, "cloud", "sam"));
        assert_eq!(ways(&mine, Audience::Person, "sam@cloud.example.org", &both).unwrap()[0], w(Way::MirrorShare, "cloud", "sam"), "a user of your own Nextcloud");
        // Someone elsewhere: federation first, Google next, a link last.
        assert_eq!(ways(&mine, Audience::Person, "lea@other.example.net", &both).unwrap(), [w(Way::MirrorFederated, "cloud", "lea@other.example.net"), w(Way::MirrorShare, "g", "lea@other.example.net"), w(Way::MirrorLink, "cloud", ""), w(Way::MirrorLink, "g", "")]);
        assert_eq!(ways(&mine, Audience::Person, "lea@gmail.com", &both).unwrap()[0], w(Way::MirrorShare, "g", "lea@gmail.com"), "Google's mail: Google first; your agendas elsewhere: a mirror there");
        assert_eq!(ways(&mine, Audience::Person, "lea@example.org", &both).unwrap()[0], w(Way::MirrorShare, "g", "lea@example.org"), "your Google account's domain");
        // All in Google, no word: Google's own share; a word: a mirror.
        let on_google = Availability { agendas: vec!["g/primary".into(), "g/work".into()], ..Availability::default() };
        assert_eq!(ways(&on_google, Audience::Person, "lea@gmail.com", &[google.clone()]).unwrap()[0], w(Way::GoogleFreeBusy, "g", "lea@gmail.com"));
        let worded = Availability { words: [("g/work".to_string(), "Work".to_string())].into(), ..on_google.clone() };
        assert_eq!(ways(&worded, Audience::Person, "lea@gmail.com", &[google.clone()]).unwrap()[0], w(Way::MirrorShare, "g", "lea@gmail.com"));
        // A group: a Nextcloud's alone.
        assert_eq!(ways(&mine, Audience::Group, "family", &both).unwrap(), [w(Way::MirrorShare, "cloud", "group:family")]);
        assert_eq!(choose(&mine, Audience::Group, "family", &[google.clone()]), Err(NO_GROUP));
        // A link: Google's or Fastmail's own for one calendar without a word; else a mirror's.
        let one_on_google = Availability { agendas: vec!["g/primary".into()], ..Availability::default() };
        assert_eq!(ways(&one_on_google, Audience::Link, "", &both).unwrap()[0], w(Way::GooglePublic, "g", ""));
        assert_eq!(ways(&on_google, Audience::Link, "", &both).unwrap()[0], w(Way::MirrorLink, "cloud", ""), "two calendars: a mirror");
        let one_on_fastmail = Availability { agendas: vec!["fm/home".into()], ..Availability::default() };
        assert_eq!(ways(&one_on_fastmail, Audience::Link, "", &[fastmail.clone()]).unwrap(), [w(Way::FastmailLink, "fm", "")]);
        // A booking tool: your own server's page first.
        let spread = Availability { agendas: vec!["cloud/personal".into(), "g/work".into()], ..Availability::default() };
        assert_eq!(ways(&spread, Audience::Booking, "", &both).unwrap()[..2], [w(Way::Appointments, "cloud", ""), w(Way::GoogleSchedule, "g", "")]);
        // A server that neither shares nor publishes: said.
        let on_icloud = Availability { agendas: vec!["ic/home".into()], ..Availability::default() };
        assert_eq!(choose(&on_icloud, Audience::Person, "sam", &[icloud.clone()]), Err(NO_WAY));
        assert_eq!(choose(&on_icloud, Audience::Link, "", &[icloud]), Err(NO_WAY));
        assert_eq!(choose(&mine, Audience::Person, " ", &both), Err(NO_WAY));
        assert_eq!(choose(&mine, Audience::Person, "sam", &both).unwrap()[0].share(Audience::Person), Share { audience: "person".into(), who: "sam".into(), way: "mirror-share".into(), account: "cloud".into(), ..Share::default() });
        for language in ["en", "fr"] {
            let tr = crate::i18n::Translator::new(language);
            for id in REASONS {
                assert_ne!(tr.text(id, None), id, "{language}: {id}");
            }
        }
    }

    /// A mirror's description says a day, never a time.
    #[test]
    fn a_mirror_says_its_day_never_its_time() {
        let en = crate::i18n::Translator::new("en");
        let fr = crate::i18n::Translator::new("fr");
        assert_eq!(mirror_description(&en, jiff::civil::date(2026, 11, 5)), "From Sioul. As of 5 November.");
        assert_eq!(mirror_description(&fr, jiff::civil::date(2026, 11, 5)), "De Sioul. Au 5 novembre.");
        assert_eq!(mirror_description(&fr, jiff::civil::date(2026, 12, 1)), "De Sioul. Au 1er décembre.");
    }

    /// Two devices keep one mirror from the freshest view: the computer,
    /// whose sources were pulled a minute ago, writes; the phone, a day
    /// behind, writes nothing over it; once its own check finds every source
    /// current, it writes again, the same bytes as the computer's. Sources
    /// pulled unevenly never let an older copy of one win; a source never
    /// pulled whole holds every write back.
    #[test]
    fn the_stale_device_writes_nothing_until_its_view_is_fresh() {
        let now = 1_800_000_000;
        let computer = view(&[Some(now - 60), Some(now - 30)]).unwrap();
        assert_eq!(computer, (now - 60, now - 30));
        assert!(may_write(Some(computer), []), "first to write");
        let written = as_of(computer);
        let phone = view(&[Some(now - 86_400), Some(now - 86_400)]);
        assert!(!may_write(phone, [written]), "a day behind: nothing written");
        let checked = view(&[Some(now + 300), Some(now + 300)]);
        assert!(may_write(checked, [written]), "its own check done: fresh again");
        // The same view on both: the same bytes.
        let events = [event("me/personal", "e1", now + 3_600, now + 7_200, "Dentist")];
        let a = mine(&["me/personal"]);
        assert_eq!(mirror_files(&events, &a, &WORDS, &BTreeSet::new(), now), mirror_files(&events, &a, &WORDS, &BTreeSet::new(), now + 300));
        // Uneven: one source newer than the last write, the other older.
        assert!(!may_write(view(&[Some(now + 600), Some(now - 120)]), [written]), "one source older than the mirror's: held");
        assert!(!may_write(view(&[Some(now), None]), []), "one source never pulled whole: held");
        assert!(!may_write(view(&[]), []), "no source: nothing to write");
    }
}
