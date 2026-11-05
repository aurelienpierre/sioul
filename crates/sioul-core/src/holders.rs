// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Who takes a child's event or task, and its stand-in in the holder's own
//! agenda (several agendas, part 3; docs/client.md, "Children's events";
//! research/agendas.md §6, recommendations 4 to 9, 12, 13 and 16).
//!
//! **The holder line**, on the child's VEVENT or VTODO, never an ATTENDEE
//! (an event with a guest is a meeting with others, which Sioul never
//! writes, and a scheduling server would mail its guests on its own):
//!
//! ```text
//! X-SIOUL-TAKEN-BY;CN=Sam;PARTSTAT=ACCEPTED;SINCE=20261103T090000Z:mailto:sam@example.org
//! X-SIOUL-TAKEN-BY;CN=Alex;PART=BACK;PARTSTAT=ACCEPTED;SINCE=20261103T090000Z:mailto:alex@example.org
//! ```
//!
//! PARTSTAT says where the asking stands: NEEDS-ACTION proposed, ACCEPTED
//! taken, DECLINED not this time. SINCE is when that was said, an act, never
//! when anything was read. `PART=BACK`: the one who brings the child back,
//! when another drops them off. A repeating event's line holds the series; a
//! changed occurrence (RECURRENCE-ID) carries its own, and one without any
//! reads the series'. Every other line is kept as it was, through every edit
//! (Sioul changes events line by line), sync and merge.
//!
//! **The kind** of a child's event or task is a concept (RFC 9253, under
//! Sioul's own name `X-SIOUL-CONCEPT`), as a task's kind:
//! `tag:aurelienpierre.com,2026:sioul/child-kind/dentist`.
//!
//! **The stand-in**, in the holder's own agenda, copies the child event's
//! title, times and place, and says what it stands for:
//!
//! ```text
//! UID:<a name-based UUID (RFC 9562 §5.5) of the child's UID and the target agenda>
//! RELATED-TO;RELTYPE=PARENT:<the child event's UID>
//! URL:sioul:event/<the child event's UID>
//! X-SIOUL-LINK;LINKREL=latest-version;VALUE=URI:sioul:event/<the same>
//! DESCRIPTION:Details: Tom’s agenda\nsioul:event/<the same>
//! ```
//!
//! Its UID alone finds it again where a server dropped the other lines
//! (Google's calendars), so that it is never made twice; its file is named
//! after it, and its stamp is the child event's, so that two devices making
//! it write the same bytes. In a work agenda, a health or school event's
//! stand-in says "Away" and is CLASS:CONFIDENTIAL.

use crate::lines;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The holder line's name.
pub const TAKEN_BY: &str = "X-SIOUL-TAKEN-BY";
/// A child's event's kind, as a concept under this tag (RFC 4151).
pub const KIND_CONCEPT: &str = "tag:aurelienpierre.com,2026:sioul/child-kind/";
/// Sioul's own namespace for the UIDs it derives (RFC 9562 §5.5): the
/// name-based UUID, version 5, of `https://aurelienpierre.github.io/sioul/`
/// in the URL namespace (RFC 9562 §6.6).
pub const NAMESPACE: [u8; 16] = [0x4e, 0x66, 0xfa, 0x6a, 0xc3, 0x06, 0x5f, 0xbd, 0x9d, 0xd4, 0x35, 0x14, 0xf3, 0x55, 0x30, 0xcf];
/// The URL namespace of RFC 9562 §6.6 (6ba7b811-9dad-11d1-80b4-00c04fd430c8): what `NAMESPACE` is checked against.
#[cfg(test)]
const URL_NAMESPACE: [u8; 16] = [0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8];

// ---------------------------------------------------------------- kinds

/// What a child's event or task is (part 3's kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Health,
    Dentist,
    School,
    Activity,
    Clothes,
    Other,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::Health, Kind::Dentist, Kind::School, Kind::Activity, Kind::Clothes, Kind::Other];

    pub fn id(self) -> &'static str {
        match self {
            Kind::Health => "health",
            Kind::Dentist => "dentist",
            Kind::School => "school",
            Kind::Activity => "activity",
            Kind::Clothes => "clothes",
            Kind::Other => "other",
        }
    }

    pub fn parse(word: &str) -> Option<Kind> {
        let word = word.trim().to_ascii_lowercase();
        Kind::ALL.into_iter().find(|k| k.id() == word)
    }

    /// Its concept's URI.
    pub fn concept(self) -> String {
        format!("{KIND_CONCEPT}{}", self.id())
    }

    /// What a work agenda never names (research/agendas.md §6,
    /// recommendation 16): health, the dentist's, school.
    pub fn private(self) -> bool {
        matches!(self, Kind::Health | Kind::Dentist | Kind::School)
    }

    /// Its rhythm by default, in months: the dentist's every six, a check-up
    /// once a year; none for the others (`follow_up`).
    pub fn rhythm(self) -> Option<u32> {
        match self {
            Kind::Dentist => Some(6),
            Kind::Health => Some(12),
            _ => None,
        }
    }
}

/// The kind of a child's event or task, from its series' concept.
pub fn kind_of(text: &str) -> Option<Kind> {
    let source = lines::unfold(text);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    own(&source, &parts[at]).filter(|l| lines::name(l) == "CONCEPT").find_map(|l| lines::value(l).trim().strip_prefix(KIND_CONCEPT).and_then(Kind::parse))
}

/// Its kind written on the series, another kind's concept taken out, every
/// other concept kept; none takes it out. None when it holds no event or task.
pub fn with_kind(text: &str, kind: Option<Kind>) -> Option<String> {
    let mut source = lines::unfold(text);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    let part = parts[at].clone();
    let ours = |line: &str| lines::name(line) == "CONCEPT" && lines::value(line).trim().starts_with(KIND_CONCEPT);
    let drop: Vec<usize> = own_indices(&source, &part).into_iter().filter(|&i| ours(&source[i])).collect();
    let insert_at = insertion(&source, &part);
    let added = kind.map(|k| crate::tasks::concept_line(&k.concept()));
    let insert_at = insert_at - drop.iter().filter(|&&i| i < insert_at).count();
    for i in drop.into_iter().rev() {
        source.remove(i);
    }
    if let Some(line) = added {
        source.insert(insert_at, line);
    }
    Some(lines::fold(&source))
}

// ---------------------------------------------------------------- the holder line

/// Where the asking stands (PARTSTAT).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Answer {
    /// Proposed, not answered yet.
    #[default]
    NeedsAction,
    Accepted,
    /// Not this time: the event is open again.
    Declined,
}

impl Answer {
    pub fn word(self) -> &'static str {
        match self {
            Answer::NeedsAction => "NEEDS-ACTION",
            Answer::Accepted => "ACCEPTED",
            Answer::Declined => "DECLINED",
        }
    }

    pub fn parse(word: &str) -> Answer {
        match word.trim().to_ascii_uppercase().as_str() {
            "ACCEPTED" => Answer::Accepted,
            "DECLINED" => Answer::Declined,
            _ => Answer::NeedsAction,
        }
    }
}

/// One holder line.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holder {
    /// "Sam", as CN says it; "" unsaid.
    pub name: String,
    /// "sam@example.org", without "mailto:".
    pub address: String,
    pub answer: Answer,
    /// When that was said (Unix seconds); none unsaid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<i64>,
    /// `PART=BACK`: who brings the child back.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub back: bool,
}

impl Holder {
    /// Whether it names this address (letters' case aside).
    pub fn is(&self, address: &str) -> bool {
        !address.trim().is_empty() && self.address.trim().eq_ignore_ascii_case(address.trim())
    }

    /// Whether it names one of these addresses.
    pub fn among(&self, addresses: &[String]) -> bool {
        addresses.iter().any(|a| self.is(a))
    }

    /// Whether this holder takes it: they said yes.
    pub fn takes(&self) -> bool {
        self.answer == Answer::Accepted
    }
}

/// The line written for a holder.
pub fn holder_line(holder: &Holder) -> String {
    let mut out = String::from(TAKEN_BY);
    if !holder.name.trim().is_empty() {
        out.push_str(";CN=");
        out.push_str(&lines::param_value(holder.name.trim()));
    }
    if holder.back {
        out.push_str(";PART=BACK");
    }
    out.push_str(";PARTSTAT=");
    out.push_str(holder.answer.word());
    if let Some(since) = holder.since {
        out.push_str(";SINCE=");
        out.push_str(&crate::moves::utc(since));
    }
    out.push_str(":mailto:");
    out.push_str(holder.address.trim());
    out
}

/// A holder line read; none for another line or one without an address.
pub fn parse_line(line: &str) -> Option<Holder> {
    if lines::raw_name(line) != TAKEN_BY {
        return None;
    }
    let value = lines::value(line).trim();
    let address = value.get(..7).filter(|head| head.eq_ignore_ascii_case("mailto:")).map_or(value, |_| &value[7..]).trim().to_string();
    if address.is_empty() {
        return None;
    }
    let params = lines::params(line);
    let param = |name: &str| params.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
    Some(Holder {
        name: param("CN").unwrap_or_default(),
        address,
        answer: param("PARTSTAT").map(|p| Answer::parse(&p)).unwrap_or_default(),
        since: param("SINCE").and_then(|s| crate::moves::read_utc(&s)),
        back: param("PART").is_some_and(|p| p.eq_ignore_ascii_case("BACK")),
    })
}

/// Who takes it and who brings the child back, at one time of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Taken {
    /// Who takes it (the first holder line without `PART=BACK`).
    pub to: Option<Holder>,
    /// Who brings the child back (`PART=BACK`), when another does.
    pub back: Option<Holder>,
}

impl Taken {
    pub fn is_empty(&self) -> bool {
        self.to.is_none() && self.back.is_none()
    }

    fn of(holders: Vec<Holder>) -> Taken {
        Taken { to: holders.iter().find(|h| !h.back).cloned(), back: holders.into_iter().find(|h| h.back) }
    }

    fn lines(&self) -> Vec<String> {
        self.to.iter().chain(self.back.iter()).map(holder_line).collect()
    }
}

/// Who takes the time of it that starts at `start` (Unix seconds): a changed
/// occurrence's own lines, else the series'; `start` none, the series'.
pub fn taken(text: &str, start: Option<i64>, zone: &TimeZone) -> Taken {
    let source = lines::unfold(text);
    let parts = components(&source);
    let read = |at: usize| own(&source, &parts[at]).filter_map(|l| parse_line(l)).collect::<Vec<_>>();
    if let Some(at) = start.and_then(|s| changed_at(&source, &parts, s, zone)) {
        let own = read(at);
        if !own.is_empty() {
            return Taken::of(own);
        }
    }
    series(&source, &parts).map(|at| Taken::of(read(at))).unwrap_or_default()
}

/// Who takes it written: on the changed occurrence that starts at `start`
/// when there is one, else on the series; the holder lines there replaced,
/// every other line kept. None when it holds no event or task.
pub fn with_taken(text: &str, start: Option<i64>, taken: &Taken, zone: &TimeZone) -> Option<String> {
    let mut source = lines::unfold(text);
    let parts = components(&source);
    let at = start.and_then(|s| changed_at(&source, &parts, s, zone)).or_else(|| series(&source, &parts))?;
    let part = parts[at].clone();
    let drop: Vec<usize> = own_indices(&source, &part).into_iter().filter(|&i| lines::raw_name(&source[i]) == TAKEN_BY).collect();
    let insert_at = insertion(&source, &part) - drop.iter().filter(|&&i| i < insertion(&source, &part)).count();
    for i in drop.into_iter().rev() {
        source.remove(i);
    }
    for (n, line) in taken.lines().into_iter().enumerate() {
        source.insert(insert_at + n, line);
    }
    Some(lines::fold(&source))
}

// ---------------------------------------------------------------- the proposal

/// A child's holders, as its agenda's settings name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Who {
    /// The person.
    Me,
    /// The other parent the child's agenda names.
    Other,
}

impl Who {
    pub fn id(self) -> &'static str {
        match self {
            Who::Me => "me",
            Who::Other => "other",
        }
    }

    pub fn parse(word: &str) -> Option<Who> {
        match word.trim().to_ascii_lowercase().as_str() {
            "me" => Some(Who::Me),
            "other" => Some(Who::Other),
            _ => None,
        }
    }
}

/// Who has the child on a day, by the child agenda's custody days: a whole
/// day titled `With <name>` (in French, « `Avec <name>` »), the name one of
/// yours (`mine`) or the other parent's (`other`). None without one, or with
/// both on that day.
pub fn custody(whole_days: &[&str], mine: &[String], other: &str) -> Option<Who> {
    custody_of(whole_days, mine, &[other.to_string()])
}

/// `custody` with the other parent by every name they go by (their person's
/// name and full name: docs/persons.md), names compared case and accents aside.
pub fn custody_of(whole_days: &[&str], mine: &[String], others: &[String]) -> Option<Who> {
    let named = |title: &str| -> Option<String> {
        let title = title.trim();
        let lower = title.to_lowercase();
        ["with ", "avec "].iter().find(|p| lower.starts_with(*p)).map(|p| title[p.len()..].trim().to_string())
    };
    let same = |a: &str, b: &str| !crate::persons::key(a).is_empty() && crate::persons::key(a) == crate::persons::key(b);
    let found: std::collections::BTreeSet<Who> = whole_days
        .iter()
        .filter_map(|t| named(t))
        .filter_map(|name| if others.iter().any(|o| same(&name, o)) { Some(Who::Other) } else if mine.iter().any(|m| same(&name, m)) { Some(Who::Me) } else { None })
        .collect();
    (found.len() == 1).then(|| found.into_iter().next()).flatten()
}

/// Who Sioul proposes for an event (research/agendas.md §6, recommendation
/// 9): the standing holder for that child and kind; else whoever has the
/// child that day by its custody days; else nobody.
pub fn propose(standing: Option<Who>, custody: Option<Who>) -> Option<Who> {
    standing.or(custody)
}

// ---------------------------------------------------------------- the stand-in

/// A name-based UUID, version 5 (RFC 9562 §5.5), of `name` in `namespace`.
pub fn uuid_v5(namespace: &[u8; 16], name: &str) -> String {
    let mut hash = Sha1::new();
    hash.update(namespace);
    hash.update(name.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// The stand-in's UID in an agenda (`target`, its place "account/id"): the
/// same for the same child event and agenda on every device.
pub fn stand_in_uid(child_uid: &str, target: &str) -> String {
    uuid_v5(&NAMESPACE, &format!("stand-in\n{child_uid}\n{target}"))
}

/// The follow-up task's UID, the same on every device (`follow_up`).
pub fn follow_up_uid(child_uid: &str) -> String {
    uuid_v5(&NAMESPACE, &format!("follow-up\n{child_uid}"))
}

/// What a stand-in is made from.
#[derive(Debug, Clone)]
pub struct Of<'a> {
    /// The child event's file, as it is.
    pub child: &'a str,
    /// Its agenda's place, where the stand-in goes ("account/id").
    pub target: &'a str,
    /// The stand-in goes into an agenda for work alone.
    pub work: bool,
    /// The child event's kind, when said.
    pub kind: Option<Kind>,
    /// "Details: Tom’s agenda": the description's last line, before the address.
    pub details: &'a str,
    /// "Away": a private kind's title in a work agenda.
    pub away: &'a str,
}

/// The child event's lines a stand-in copies: its times, how it repeats, its title, place and status.
const COPIED: [&str; 9] = ["DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "LOCATION", "STATUS"];

/// The stand-in's lines that follow its child: the copied ones, and CLASS.
const FOLLOWING: [&str; 10] = ["DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "LOCATION", "STATUS", "CLASS"];

/// The UID of the child event a stand-in stands for: its parent, when its
/// latest-version link names that event's address. None for any other event.
pub fn stands_for(text: &str) -> Option<String> {
    let source = lines::unfold(text);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    let parent = own(&source, &parts[at]).find(|l| lines::name(l) == "RELATED-TO" && lines::param(l, "RELTYPE").is_none_or(|r| r.eq_ignore_ascii_case("PARENT")) && lines::raw_name(l) == "RELATED-TO").map(|l| lines::value(l).trim().to_string())?;
    let address = crate::links::event_uri(&parent);
    let linked = own(&source, &parts[at]).any(|l| lines::name(l) == "LINK" && lines::param(l, "LINKREL").is_some_and(|r| r.eq_ignore_ascii_case("latest-version")) && lines::value(l).trim() == address);
    linked.then_some(parent)
}

/// The child event's lines, as the stand-in says them: what it copies, its
/// title hidden in a work agenda for a private kind, its place then too.
fn following_lines(of: &Of) -> Option<(Vec<String>, Vec<String>, String)> {
    let source = lines::unfold(of.child);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    if parts[at].kind != "VEVENT" {
        return None;
    }
    let uid = own(&source, &parts[at]).find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()).filter(|u| !u.is_empty())?;
    let hidden = of.work && of.kind.is_some_and(Kind::private);
    let mut out: Vec<String> = Vec::new();
    for line in own(&source, &parts[at]) {
        let name = lines::name(line);
        if !COPIED.contains(&name.as_str()) || lines::raw_name(line) != name {
            continue;
        }
        match name.as_str() {
            "SUMMARY" if hidden => out.push(format!("SUMMARY:{}", lines::escape(of.away))),
            "LOCATION" if hidden => {}
            _ => out.push(line.clone()),
        }
    }
    if hidden {
        out.push("CLASS:CONFIDENTIAL".to_string());
    }
    Some((out, zones(&source), uid))
}

/// The VTIMEZONE blocks of a calendar's head, in order.
fn zones(source: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = 0usize;
    for line in source {
        let (name, value) = (lines::name(line), lines::value(line).trim().to_ascii_uppercase());
        if name == "BEGIN" && value == "VTIMEZONE" {
            inside += 1;
        }
        if inside > 0 {
            out.push(line.clone());
        }
        if name == "END" && value == "VTIMEZONE" {
            inside = inside.saturating_sub(1);
        }
    }
    out
}

/// The child event's stamp, so that two devices write the same bytes: its
/// LAST-MODIFIED, else its DTSTAMP, else `now`.
fn stamp_of(child: &str, now: i64) -> String {
    let source = lines::unfold(child);
    let parts = components(&source);
    series(&source, &parts)
        .and_then(|at| {
            let mine = || own(&source, &parts[at]);
            mine().find(|l| lines::name(l) == "LAST-MODIFIED").or_else(|| mine().find(|l| lines::name(l) == "DTSTAMP")).map(|l| lines::value(l).trim().to_string())
        })
        .filter(|s| crate::moves::read_utc(s).is_some())
        .unwrap_or_else(|| crate::moves::utc(now))
}

/// A child event's stand-in in the holder's agenda (`of.target`): its UID
/// derived, its title, times and place copied (in a work agenda a private
/// kind's title "Away", no place, CLASS:CONFIDENTIAL), what it stands for
/// said in its lines and its description. None for no event.
pub fn stand_in(of: &Of, now: i64) -> Option<String> {
    let (following, zones, child_uid) = following_lines(of)?;
    let address = crate::links::event_uri(&child_uid);
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string()];
    out.extend(zones);
    out.push("BEGIN:VEVENT".to_string());
    out.push(format!("UID:{}", stand_in_uid(&child_uid, of.target)));
    out.push(format!("DTSTAMP:{}", stamp_of(of.child, now)));
    out.extend(following);
    out.push(format!("DESCRIPTION:{}", lines::escape(&format!("{}\n{address}", of.details))));
    out.push(format!("RELATED-TO;RELTYPE=PARENT:{child_uid}"));
    out.push(format!("URL:{address}"));
    out.push(format!("{};LINKREL=latest-version;VALUE=URI:{address}", crate::tasks::LINK));
    out.extend(["END:VEVENT".to_string(), "END:VCALENDAR".to_string()]);
    Some(lines::fold(&out))
}

/// The stand-in as it follows its child event (moved, renamed, cancelled,
/// its kind made private in a work agenda): the lines it copies replaced,
/// and its zones, every other line kept (the holder's margins, reminder and
/// alarms); none when it already says what the child does.
pub fn followed(stand_in_text: &str, of: &Of, now: i64) -> Option<String> {
    let (following, child_zones, _) = following_lines(of)?;
    let source = lines::unfold(stand_in_text);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    let part = parts[at].clone();
    let mine: Vec<String> = own(&source, &part).filter(|l| FOLLOWING.contains(&lines::name(l).as_str())).cloned().collect();
    let sorted = |mut v: Vec<String>| {
        v.sort();
        v
    };
    if sorted(mine) == sorted(following.clone()) && zones(&source) == child_zones {
        return None;
    }
    // The event's lines: those it follows taken out, the child's put where the first was.
    let indices = own_indices(&source, &part);
    let mut event: Vec<String> = Vec::new();
    let mut placed = false;
    for i in part.begin..=part.end {
        let line = &source[i];
        let name = lines::name(line);
        if indices.contains(&i) && (FOLLOWING.contains(&name.as_str()) || name == "DTSTAMP") {
            if !placed {
                event.push(format!("DTSTAMP:{}", stamp_of(of.child, now)));
                event.extend(following.iter().cloned());
                placed = true;
            }
            continue;
        }
        if !placed && name == "END" && i == part.end {
            event.extend(following.iter().cloned());
            placed = true;
        }
        event.push(line.clone());
    }
    // The calendar around it: its zones the child's.
    let mut out: Vec<String> = Vec::new();
    let mut inside = 0usize;
    let mut zones_written = false;
    for (i, line) in source.iter().enumerate() {
        if i == part.begin {
            if !zones_written {
                out.extend(child_zones.iter().cloned());
                zones_written = true;
            }
            out.extend(event.iter().cloned());
            continue;
        }
        if i > part.begin && i <= part.end {
            continue;
        }
        let (name, value) = (lines::name(line), lines::value(line).trim().to_ascii_uppercase());
        if name == "BEGIN" && value == "VTIMEZONE" {
            inside += 1;
        }
        let zone_line = inside > 0;
        if name == "END" && value == "VTIMEZONE" {
            inside = inside.saturating_sub(1);
        }
        if !zone_line {
            out.push(line.clone());
        }
    }
    Some(lines::fold(&out))
}

// ---------------------------------------------------------------- follow-ups

/// A follow-up task for a held event done, of a kind with a rhythm
/// (research/agendas.md §6, recommendation 6, after Rodsky 2019's cards):
/// "Book Tom's next dentist appointment", in the child's list, held by the
/// same parent, due a month before the rhythm comes round (`months` after
/// the event's day), its kind kept. `title` is said in the person's
/// language; `child_day` the event's day; `held` its holder line.
pub fn follow_up(child_uid: &str, title: &str, kind: Kind, child_day: jiff::civil::Date, months: u32, held: &Holder, stamp: i64) -> Option<String> {
    let due = child_day.checked_add(jiff::Span::new().months(i64::from(months.max(1)) - 1)).ok()?;
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "BEGIN:VTODO".to_string()];
    out.push(format!("UID:{}", follow_up_uid(child_uid)));
    out.push(format!("DTSTAMP:{}", crate::moves::utc(stamp)));
    out.push(format!("SUMMARY:{}", lines::escape(title)));
    out.push(format!("DUE;VALUE=DATE:{}", due.strftime("%Y%m%d")));
    out.push("STATUS:NEEDS-ACTION".to_string());
    out.push(crate::tasks::concept_line(&kind.concept()));
    out.push(crate::tasks::link_line(&crate::tasks::Link { uri: crate::links::event_uri(child_uid), label: String::new(), rel: "related".into() }));
    out.push(holder_line(&Holder { back: false, ..held.clone() }));
    out.extend(["END:VTODO".to_string(), "END:VCALENDAR".to_string()]);
    Some(lines::fold(&out))
}

// ---------------------------------------------------------------- the invitation (iTIP over iMIP)

/// Who an invitation is from and to: a name, an address.
#[derive(Debug, Clone, Copy)]
pub struct Person<'a> {
    pub name: &'a str,
    pub address: &'a str,
}

/// An invitation to take a child's event (`invitation`).
#[derive(Debug, Clone, Copy)]
pub struct Invite<'a> {
    /// "REQUEST", or "CANCEL" once it is cancelled or given to someone else.
    pub method: &'a str,
    /// You.
    pub organizer: Person<'a>,
    pub holder: Person<'a>,
    /// What the holder needs to know, in DESCRIPTION.
    pub details: &'a str,
    /// The address back, in URL.
    pub url: &'a str,
    /// The invitations sent for it: 0 the first, then each update one more.
    pub sequence: u32,
    /// When it is made (Unix seconds), its DTSTAMP.
    pub now: i64,
}

fn person_line(property: &str, person: Person, more: &str) -> String {
    let name = if person.name.trim().is_empty() { String::new() } else { format!(";CN={}", lines::param_value(person.name.trim())) };
    format!("{property}{name}{more}:mailto:{}", person.address.trim())
}

/// The invitation to take a child's event, when Sioul cannot write the
/// holder's agenda (research/agendas.md §6, recommendation 13): an iTIP
/// REQUEST (RFC 5546 §3.2.2), the child event's UID kept, from you as its
/// ORGANIZER, to the holder as a required ATTENDEE asked to answer
/// (`ROLE=REQ-PARTICIPANT;RSVP=TRUE`), with its times, title and place, the
/// details in DESCRIPTION and the address back in URL; its SEQUENCE the
/// invitations sent before it; a CANCEL (RFC 5546 §3.2.5) once it is
/// cancelled or given to someone else. None for no event.
pub fn invitation(child: &str, invite: &Invite) -> Option<String> {
    let Invite { method, organizer, holder, details, url, sequence, now } = *invite;
    let source = lines::unfold(child);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    if parts[at].kind != "VEVENT" {
        return None;
    }
    let uid = own(&source, &parts[at]).find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()).filter(|u| !u.is_empty())?;
    let cancel = method.eq_ignore_ascii_case("CANCEL");
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), format!("METHOD:{}", if cancel { "CANCEL" } else { "REQUEST" })];
    out.extend(zones(&source));
    out.push("BEGIN:VEVENT".to_string());
    out.push(format!("UID:{uid}"));
    out.push(format!("SEQUENCE:{sequence}"));
    out.push(format!("DTSTAMP:{}", crate::moves::utc(now)));
    for line in own(&source, &parts[at]) {
        let name = lines::name(line);
        if ["DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "LOCATION"].contains(&name.as_str()) && lines::raw_name(line) == name {
            out.push(line.clone());
        }
    }
    out.push(person_line("ORGANIZER", organizer, ""));
    out.push(person_line("ATTENDEE", holder, if cancel { ";ROLE=REQ-PARTICIPANT" } else { ";ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE" }));
    if cancel {
        out.push("STATUS:CANCELLED".to_string());
    }
    if !details.trim().is_empty() {
        out.push(format!("DESCRIPTION:{}", lines::escape(details)));
    }
    if !url.trim().is_empty() {
        out.push(format!("URL:{}", url.trim()));
    }
    out.extend(["END:VEVENT".to_string(), "END:VCALENDAR".to_string()]);
    Some(lines::fold(&out))
}

/// What an invitation says of its event, as one text: its series' times,
/// how it repeats, its title, place and status (`COPIED`). Another than
/// the last invitation's: an update is due.
pub fn fingerprint(text: &str) -> String {
    let source = lines::unfold(text);
    let parts = components(&source);
    let Some(at) = series(&source, &parts) else { return String::new() };
    own(&source, &parts[at]).filter(|l| COPIED.contains(&lines::name(l).as_str())).cloned().collect::<Vec<_>>().join("\n")
}

/// What a REPLY (RFC 5546 §3.2.3) says of a child's event: its UID, who
/// answered (the ATTENDEE's address), their answer and when they gave it
/// (its DTSTAMP). None for another method, or a REPLY without them.
pub fn reply_of(text: &str) -> Option<(String, String, Answer, Option<i64>)> {
    let source = lines::unfold(text);
    let method = source.iter().find(|l| lines::name(l) == "METHOD").map(|l| lines::value(l).trim().to_ascii_uppercase())?;
    if method != "REPLY" {
        return None;
    }
    let parts = components(&source);
    let part = parts.iter().find(|p| p.kind == "VEVENT")?;
    let mine: Vec<&String> = own(&source, part).collect();
    let uid = mine.iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string())?;
    let attendee = mine.iter().find(|l| lines::name(l) == "ATTENDEE")?;
    let value = lines::value(attendee).trim();
    let address = value.get(..7).filter(|head| head.eq_ignore_ascii_case("mailto:")).map_or(value, |_| &value[7..]).trim().to_string();
    let answer = lines::param(attendee, "PARTSTAT").map(|p| Answer::parse(&p))?;
    let since = mine.iter().find(|l| lines::name(l) == "DTSTAMP").and_then(|l| crate::moves::read_utc(lines::value(l)));
    Some((uid, address, answer, since))
}

/// A child event's holder lines as a REPLY sets them: the holder it names
/// (by address) gets its answer and when it was given; nothing for a REPLY
/// of another event, from someone it does not name, or older than what the
/// line says already. None when nothing changes.
pub fn replied(child: &str, reply: &str, zone: &TimeZone) -> Option<String> {
    let (uid, address, answer, since) = reply_of(reply)?;
    let source = lines::unfold(child);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    let child_uid = own(&source, &parts[at]).find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string())?;
    if child_uid != uid {
        return None;
    }
    let mut now = taken(child, None, zone);
    let slot = [now.to.as_mut(), now.back.as_mut()].into_iter().flatten().find(|h| h.is(&address))?;
    if slot.answer == answer || slot.since.zip(since).is_some_and(|(was, said)| said < was) {
        return None;
    }
    slot.answer = answer;
    slot.since = since.or(slot.since);
    with_taken(child, None, &now, zone)
}

// ---------------------------------------------------------------- what the holder lines ask of your agendas

/// A child agenda's item, as the upkeep reads it: an event or a task.
#[derive(Debug, Clone, Copy)]
pub struct ChildItem<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub agenda: &'a crate::agendas::Agenda,
}

/// An event of your own agendas that may stand in for a child's.
#[derive(Debug, Clone, Copy)]
pub struct Mine<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    /// Its agenda's place, and whether that agenda is for work alone.
    pub place: &'a str,
    pub work: bool,
}

/// Where a stand-in goes: its agenda's place, its folder, whether it is for work alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub place: String,
    pub dir: PathBuf,
    pub work: bool,
}

/// What a follow-up needs, for a child agenda and a kind: its title, the
/// months after the event, the list it goes into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowUp {
    pub title: String,
    pub months: u32,
    pub dir: PathBuf,
}

/// What the upkeep reads beside the files.
pub struct Context<'a> {
    /// Your addresses (`agenda::own_addresses`).
    pub own: &'a [String],
    /// Where your stand-in of a kind goes; none: none is made.
    pub target: &'a dyn Fn(Option<Kind>) -> Option<Target>,
    /// "Details: Tom’s agenda", for a child's agenda.
    pub details: &'a dyn Fn(&crate::agendas::Agenda) -> String,
    /// "Away", a private kind's title in a work agenda.
    pub away: &'a str,
    /// A kind's follow-up in a child's agenda; none when follow-ups are off,
    /// the kind has no rhythm, or the child has no list.
    pub follow_up: &'a dyn Fn(&crate::agendas::Agenda, Kind) -> Option<FollowUp>,
    pub now: i64,
    pub zone: &'a TimeZone,
}

/// What the holder lines ask be written, each once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Your stand-in, made: you take it (`child`, its UID) and none was there.
    Made { path: PathBuf, text: String, child: String },
    /// Your stand-in, following its child event (moved, renamed, cancelled).
    Followed { path: PathBuf, text: String, child: String },
    /// Your stand-in, taken out: another is asked or takes it, or you said not this time.
    Given { path: PathBuf, child: String },
    /// A follow-up task, added in the child's list.
    FollowUp { path: PathBuf, text: String, child: String },
}

/// A follow-up is made for an event over within this long (seconds): never
/// for those of before, the day follow-ups came, or a list read late.
const FOLLOW_UP_WITHIN: i64 = 30 * 86_400;

/// A child event's series' UID; none for a task or no UID.
fn event_uid(text: &str) -> Option<String> {
    let source = lines::unfold(text);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    (parts[at].kind == "VEVENT").then(|| own(&source, &parts[at]).find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string())).flatten().filter(|u| !u.is_empty())
}

/// Any item's UID, its series' (an event's or a task's).
pub fn uid_of(text: &str) -> Option<String> {
    let source = lines::unfold(text);
    let parts = components(&source);
    let at = series(&source, &parts)?;
    own(&source, &parts[at]).find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()).filter(|u| !u.is_empty())
}

/// Whether its series comes back (RRULE, RDATE).
fn repeats(text: &str) -> bool {
    let source = lines::unfold(text);
    let parts = components(&source);
    series(&source, &parts).is_some_and(|at| own(&source, &parts[at]).any(|l| matches!(lines::name(l).as_str(), "RRULE" | "RDATE")))
}

/// What the holder lines ask of your agendas, as they are now (part 3; the
/// rides' rule, docs/tasks.md, "Getting there and coming back"): you take a
/// child's event and no stand-in of it is in your agendas, one is made where
/// its kind's go; one is there, it follows its child; another is asked or
/// takes it, or you said not this time, it is taken out; none is made for
/// an event over (one that repeats always counts). A stand-in whose
/// child names nobody stays: a server may have dropped the line (Google's),
/// and its UID still says you take it. A child event gone writes nothing,
/// nothing is lost. Once over, a held event of a kind with a rhythm gets
/// its follow-up in the child's list, once (its UID derived).
pub fn upkeep(children: &[ChildItem], mine: &[Mine], cx: &Context) -> Vec<Act> {
    let known: std::collections::BTreeSet<String> = children.iter().filter_map(|c| uid_of(c.text)).collect();
    let mut acts = Vec::new();
    for child in children {
        let Some(uid) = event_uid(child.text) else { continue };
        let taken = taken(child.text, None, cx.zone);
        let kind = kind_of(child.text);
        let standing_in: Vec<&Mine> = mine.iter().filter(|m| stands_for(m.text).as_deref() == Some(uid.as_str()) || uid_of(m.text).is_some_and(|u| u == stand_in_uid(&uid, m.place))).collect();
        let yours = taken.to.as_ref().is_some_and(|h| h.among(cx.own) && h.takes());
        let given = taken.to.as_ref().is_some_and(|h| !h.among(cx.own) || h.answer == Answer::Declined);
        if given {
            acts.extend(standing_in.iter().map(|m| Act::Given { path: m.path.to_path_buf(), child: uid.clone() }));
            continue;
        }
        let details = (cx.details)(child.agenda);
        // An event over (not repeating) gets no stand-in: nothing to hold any more.
        let over = !repeats(child.text) && crate::agenda::first_times(child.text, cx.zone).is_some_and(|(_, end)| end <= cx.now);
        if standing_in.is_empty() {
            if yours && !over && let Some(target) = (cx.target)(kind) {
                let of = Of { child: child.text, target: &target.place, work: target.work, kind, details: &details, away: cx.away };
                if let Some(text) = stand_in(&of, cx.now) {
                    acts.push(Act::Made { path: target.dir.join(format!("{}.ics", stand_in_uid(&uid, &target.place))), text, child: uid.clone() });
                }
            }
        } else {
            for m in &standing_in {
                let of = Of { child: child.text, target: m.place, work: m.work, kind, details: &details, away: cx.away };
                if let Some(text) = followed(m.text, &of, cx.now) {
                    acts.push(Act::Followed { path: m.path.to_path_buf(), text, child: uid.clone() });
                }
            }
        }
        // Its follow-up, once it is over: whoever took it holds it too.
        let Some(holder) = taken.to.as_ref().filter(|h| h.takes()) else { continue };
        let Some(kind) = kind else { continue };
        let follow = follow_up_uid(&uid);
        if known.contains(&follow) || repeats(child.text) {
            continue;
        }
        let Some((start, end)) = crate::agenda::first_times(child.text, cx.zone) else { continue };
        if end > cx.now || end <= cx.now - FOLLOW_UP_WITHIN {
            continue;
        }
        let Some(wanted) = (cx.follow_up)(child.agenda, kind) else { continue };
        let Ok(day) = jiff::Timestamp::from_second(start).map(|t| t.to_zoned(cx.zone.clone()).date()) else { continue };
        if let Some(text) = follow_up(&uid, &wanted.title, kind, day, wanted.months, holder, end) {
            acts.push(Act::FollowUp { path: wanted.dir.join(format!("{follow}.ics")), text, child: uid.clone() });
        }
    }
    acts
}

/// Each task of a child's list (`child_lists`, by place) marked as held
/// elsewhere (`Task::elsewhere`) unless you take it (research/agendas.md
/// §6, recommendation 7): it sits in its holder's plan and Next, never in
/// another's; nobody's yet, in nobody's.
pub fn mark_tasks(tasks: &mut [crate::tasks::Task], child_lists: &std::collections::BTreeSet<String>, own: &[String]) {
    for task in tasks.iter_mut().filter(|t| child_lists.contains(&t.list_id)) {
        task.elsewhere = !task.holder.as_ref().is_some_and(|h| h.among(own) && h.takes());
    }
}

// ---------------------------------------------------------------- asking once

/// What a child's event asks of you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Asking {
    /// Nobody takes it: "Who takes it? Me, Sam, later."
    Nobody,
    /// The other parent proposes that you take it: "take it, or not this time".
    Proposed,
    /// The one you asked said not this time: open again, said once.
    Reopened,
}

/// What a child's item asks of you now, if anything: a coming one that
/// nobody takes, one proposed to you, one the other parent said not this
/// time to (open again). One you take, or that the other parent was asked
/// and has not answered, asks nothing.
pub fn asking(text: &str, own: &[String], zone: &TimeZone) -> Option<Asking> {
    let taken = taken(text, None, zone);
    match taken.to {
        None => Some(Asking::Nobody),
        Some(h) if h.among(own) && h.answer == Answer::NeedsAction => Some(Asking::Proposed),
        Some(h) if !h.among(own) && h.answer == Answer::Declined => Some(Asking::Reopened),
        Some(_) => None,
    }
}

/// "Later", said of one question: how many times, and from when it is asked again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Later {
    pub times: u8,
    /// Unix seconds: the next admin window's opening.
    pub from: i64,
}

/// What this device asked of the children's events (`holder-questions.toml`
/// in the state folder, this device's own): each event is asked once, when
/// it comes in without a holder (research/agendas.md §6, recommendation 9);
/// "Later" asks once more, at the next admin window, then never. The events
/// there at this device's first look are never asked: a question comes with
/// something new, never with what you come upon.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asked {
    /// The first look was made: what was there then is not asked.
    #[serde(default)]
    pub looked: bool,
    /// The children's items seen at the last look, by UID.
    #[serde(default)]
    pub seen: std::collections::BTreeSet<String>,
    /// Those to ask, by UID: each come since the first look.
    #[serde(default)]
    pub ask: std::collections::BTreeSet<String>,
    /// "Later", by UID.
    #[serde(default)]
    pub later: BTreeMap<String, Later>,
    /// What was said once: `<uid>|<since>` of an answer "not this time".
    #[serde(default)]
    pub said: std::collections::BTreeSet<String>,
}

impl Asked {
    pub fn path() -> PathBuf {
        crate::config::state_dir().join("holder-questions.toml")
    }

    pub fn load(path: &Path) -> Asked {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A look at the children's items now (their UIDs): what came since the
    /// last is to be asked, unless this is the first look; what is gone is
    /// forgotten. Whether anything changed.
    pub fn look(&mut self, uids: &std::collections::BTreeSet<String>) -> bool {
        let before = self.clone();
        if self.looked {
            self.ask.extend(uids.difference(&self.seen).cloned());
        }
        self.looked = true;
        self.seen = uids.clone();
        self.ask.retain(|u| uids.contains(u));
        self.later.retain(|u, _| uids.contains(u));
        self.said.retain(|key| key.split_once('|').is_some_and(|(u, _)| uids.contains(u)));
        *self != before
    }

    /// Whether a question about `uid` is put now: proposed to you, always
    /// until answered or put off; else one come since the first look, or
    /// open again; put off once, from the next admin window; twice, never.
    pub fn due(&self, uid: &str, asking: Asking, now: i64) -> bool {
        match self.later.get(uid) {
            Some(later) if later.times >= 2 => false,
            Some(later) if later.from > now => false,
            _ => asking == Asking::Proposed || asking == Asking::Reopened || self.ask.contains(uid),
        }
    }

    /// "Later": asked again from `from` (the next admin window), once.
    pub fn put_off(&mut self, uid: &str, from: i64) {
        let later = self.later.entry(uid.to_string()).or_default();
        later.times = later.times.saturating_add(1);
        later.from = from;
    }

    /// Answered: not asked again.
    pub fn answered(&mut self, uid: &str) {
        self.ask.remove(uid);
        self.later.remove(uid);
    }

    /// Says something once (`key`): whether it is the first time.
    pub fn say_once(&mut self, key: &str) -> bool {
        self.said.insert(key.to_string())
    }
}

// ---------------------------------------------------------------- the components of a file

/// One VEVENT or VTODO of a calendar file: its BEGIN's and END's lines.
#[derive(Debug, Clone)]
struct Part {
    begin: usize,
    end: usize,
    kind: &'static str,
}

/// The VEVENTs and VTODOs of a file's lines, in order.
fn components(source: &[String]) -> Vec<Part> {
    let mut out = Vec::new();
    let mut open: Option<(usize, &'static str)> = None;
    let mut nested = 0usize;
    for (i, line) in source.iter().enumerate() {
        let (name, value) = (lines::name(line), lines::value(line).trim().to_ascii_uppercase());
        match (open, name.as_str()) {
            (None, "BEGIN") if value == "VEVENT" || value == "VTODO" => {
                open = Some((i, if value == "VEVENT" { "VEVENT" } else { "VTODO" }));
                nested = 0;
            }
            (Some(_), "BEGIN") => nested += 1,
            (Some(_), "END") if nested > 0 => nested -= 1,
            (Some((begin, kind)), "END") => {
                out.push(Part { begin, end: i, kind });
                open = None;
            }
            _ => {}
        }
    }
    out
}

/// A component's own lines' places: neither its BEGIN and END nor a nested one's (an alarm's).
fn own_indices(source: &[String], part: &Part) -> Vec<usize> {
    let mut out = Vec::new();
    let mut nested = 0usize;
    for i in part.begin + 1..part.end {
        let name = lines::name(&source[i]);
        if name == "BEGIN" {
            nested += 1;
            continue;
        }
        if name == "END" && nested > 0 {
            nested -= 1;
            continue;
        }
        if nested == 0 {
            out.push(i);
        }
    }
    out
}

fn own<'a>(source: &'a [String], part: &Part) -> impl Iterator<Item = &'a String> {
    own_indices(source, part).into_iter().map(move |i| &source[i])
}

/// Where a line is added to a component: before its first nested component, else before its END.
fn insertion(source: &[String], part: &Part) -> usize {
    (part.begin + 1..part.end).find(|&i| lines::name(&source[i]) == "BEGIN").unwrap_or(part.end)
}

/// The series: the component without RECURRENCE-ID, else the first.
fn series(source: &[String], parts: &[Part]) -> Option<usize> {
    parts.iter().position(|p| !own(source, p).any(|l| lines::name(l) == "RECURRENCE-ID")).or(if parts.is_empty() { None } else { Some(0) })
}

/// The changed occurrence that starts at `start`, or that RECURRENCE-ID names there.
fn changed_at(source: &[String], parts: &[Part], start: i64, zone: &TimeZone) -> Option<usize> {
    let instant = |p: &Part, name: &str| own(source, p).find(|l| lines::name(l) == name).and_then(|l| crate::agenda::instant_in(l, zone));
    let changed = |p: &&Part| own(source, p).any(|l| lines::name(l) == "RECURRENCE-ID");
    parts.iter().enumerate().filter(|(_, p)| changed(p)).find(|(_, p)| instant(p, "DTSTART") == Some(start)).or_else(|| parts.iter().enumerate().filter(|(_, p)| changed(p)).find(|(_, p)| instant(p, "RECURRENCE-ID") == Some(start))).map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DENTIST: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Other//EN\r\nBEGIN:VEVENT\r\nUID:dentist-1\r\nDTSTAMP:20261101T080000Z\r\nDTSTART:20261110T090000Z\r\nDTEND:20261110T100000Z\r\nSUMMARY:Dentist\r\nLOCATION:12 rue des Lilas\r\nX-OTHER-APP:kept\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    /// A file's logical lines, one per line: what a check looks for, whatever the folding.
    fn flat(text: &str) -> String {
        lines::unfold(text).join("\n")
    }

    fn sam(answer: Answer) -> Holder {
        Holder { name: "Sam".into(), address: "sam@example.org".into(), answer, since: Some(1_793_000_000), back: false }
    }

    /// The holder line written, read, and kept through an edit and a sync
    /// (part 3's first test): every other line as it was; an edit that
    /// changes the title keeps it; a server's copy that comes back with it
    /// reads the same.
    #[test]
    fn the_holder_line_is_written_read_and_kept() {
        let zone = TimeZone::UTC;
        let taken = Taken { to: Some(sam(Answer::Accepted)), back: Some(Holder { name: "Alex Martin".into(), address: "alex@example.org".into(), answer: Answer::Accepted, since: None, back: true }) };
        let written = with_taken(DENTIST, None, &taken, &zone).unwrap();
        assert!(flat(&written).contains("X-SIOUL-TAKEN-BY;CN=Sam;PARTSTAT=ACCEPTED;SINCE=20261026T073320Z:mailto:sam@example.org"), "{written}");
        assert!(flat(&written).contains("X-SIOUL-TAKEN-BY;CN=Alex Martin;PART=BACK;PARTSTAT=ACCEPTED:mailto:alex@example.org"), "{written}");
        assert!(!written.contains("ATTENDEE"), "never a guest");
        assert_eq!(super::taken(&written, None, &zone), taken);
        // Every other line kept, the alarm whole, the lines before it.
        for kept in ["X-OTHER-APP:kept", "TRIGGER:-PT15M", "LOCATION:12 rue des Lilas", "PRODID:-//Other//EN"] {
            assert!(written.contains(kept), "{kept}");
        }
        assert!(written.find("X-SIOUL-TAKEN-BY").unwrap() < written.find("BEGIN:VALARM").unwrap());
        // An edit as Sioul makes it (line by line) keeps it.
        let edit = crate::agenda::EventEdit { title: "Dentist, check-up".into(), start: "2026-11-10T10:00".into(), end: "2026-11-10T11:00".into(), ..Default::default() };
        let edited = crate::agenda::apply(&written, &edit, &zone).unwrap();
        assert_eq!(super::taken(&edited, None, &zone), taken, "{edited}");
        // As a server sends it back (folded otherwise, a parameter quoted): the same holders.
        let synced = written.replace("CN=Sam", "CN=\"Sam\"").replace("\r\n", "\n");
        assert_eq!(super::taken(&synced, None, &zone), taken);
        // Written again with the same holders: the same bytes.
        assert_eq!(with_taken(&written, None, &taken, &zone).unwrap(), written);
        // Taken out: none, every other line back as it was.
        assert_eq!(with_taken(&written, None, &Taken::default(), &zone).unwrap(), lines::fold(&lines::unfold(DENTIST)));
    }

    /// A repeating event's holder holds the series; a changed occurrence
    /// carries its own, and one without any reads the series'.
    #[test]
    fn a_series_holds_and_a_changed_time_carries_its_own() {
        let zone = TimeZone::UTC;
        let weekly = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:swim-1\r\nDTSTART:20261104T160000Z\r\nDTEND:20261104T170000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Swimming\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:swim-1\r\nRECURRENCE-ID:20261111T160000Z\r\nDTSTART:20261111T170000Z\r\nDTEND:20261111T180000Z\r\nSUMMARY:Swimming\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let me = Holder { name: "Alex".into(), address: "alex@example.org".into(), answer: Answer::Accepted, since: None, back: false };
        let series_held = with_taken(weekly, None, &Taken { to: Some(me.clone()), back: None }, &zone).unwrap();
        let at = |text: &str| jiff::Timestamp::strptime("%Y%m%dT%H%M%S%z", text).unwrap().as_second();
        assert_eq!(super::taken(&series_held, Some(at("20261118T160000+0000")), &zone).to, Some(me.clone()));
        // The changed time (moved to 17:00) reads the series' until it has its own.
        let moved = at("20261111T170000+0000");
        assert_eq!(super::taken(&series_held, Some(moved), &zone).to, Some(me.clone()));
        let own = with_taken(&series_held, Some(moved), &Taken { to: Some(sam(Answer::NeedsAction)), back: None }, &zone).unwrap();
        assert_eq!(super::taken(&own, Some(moved), &zone).to, Some(sam(Answer::NeedsAction)));
        assert_eq!(super::taken(&own, None, &zone).to, Some(me), "the series keeps its own");
        assert_eq!(own.matches("X-SIOUL-TAKEN-BY").count(), 2);
    }

    /// The proposal's order (part 3): the standing holder for that child and
    /// kind; else who has the child that day ("With Sam", « Avec Alex »);
    /// else nobody, as with both on one day.
    #[test]
    fn the_proposal_is_the_standing_holder_then_the_custody_day_then_none() {
        let mine = vec!["Alex".to_string()];
        assert_eq!(propose(Some(Who::Me), custody(&["With Sam"], &mine, "Sam")), Some(Who::Me));
        assert_eq!(propose(None, custody(&["With Sam"], &mine, "Sam")), Some(Who::Other));
        assert_eq!(propose(None, custody(&["Avec alex"], &mine, "Sam")), Some(Who::Me));
        assert_eq!(propose(None, custody(&["School trip"], &mine, "Sam")), None);
        assert_eq!(propose(None, custody(&["With Sam", "With Alex"], &mine, "Sam")), None);
        assert_eq!(propose(None, custody(&[], &mine, "Sam")), None);
        assert_eq!(propose(Some(Who::Other), None), Some(Who::Other));
        // The other parent by their person's names too, accents aside (docs/persons.md).
        let sam = ["Sam".to_string(), "Samuel Étienne".to_string()];
        assert_eq!(custody_of(&["Avec samuel etienne"], &mine, &sam), Some(Who::Other));
        assert_eq!(custody_of(&["With Sam", "With Samuel Étienne"], &mine, &sam), Some(Who::Other), "one parent by two names is one");
    }

    /// Its kind is a concept, another application's concepts kept.
    #[test]
    fn a_kind_is_a_concept() {
        let other = DENTIST.replace("X-OTHER-APP:kept\r\n", "X-OTHER-APP:kept\r\nCONCEPT:https://example.org/health\r\n");
        let dentist = with_kind(&other, Some(Kind::Dentist)).unwrap();
        assert!(flat(&dentist).contains("X-SIOUL-CONCEPT;VALUE=URI:tag:aurelienpierre.com,2026:sioul/child-kind/dentist"), "{dentist}");
        assert_eq!(kind_of(&dentist), Some(Kind::Dentist));
        let school = with_kind(&dentist, Some(Kind::School)).unwrap();
        assert_eq!((kind_of(&school), flat(&school).matches("child-kind/").count()), (Some(Kind::School), 1));
        assert!(school.contains("CONCEPT:https://example.org/health"));
        assert_eq!(kind_of(&with_kind(&school, None).unwrap()), None);
    }

    /// The namespace is what the module says it is, and a derived UID is a
    /// version 5 UUID, the same for the same child and agenda.
    #[test]
    fn the_derived_uids_are_name_based() {
        let namespace = uuid_v5(&URL_NAMESPACE, "https://aurelienpierre.github.io/sioul/");
        let written: String = NAMESPACE.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(namespace.replace('-', ""), written);
        assert_eq!(namespace, "4e66fa6a-c306-5fbd-9dd4-3514f35530cf");
        // RFC 9562's own example (Appendix A.4): www.example.com in the DNS namespace.
        let dns = [0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8];
        assert_eq!(uuid_v5(&dns, "www.example.com"), "2ed6657d-e927-568b-95e1-2665a8aea6a2");
        let uid = stand_in_uid("dentist-1", "me/personal");
        assert_eq!(uid, stand_in_uid("dentist-1", "me/personal"));
        assert_ne!(uid, stand_in_uid("dentist-1", "work/main"));
        assert_eq!(&uid[14..15], "5");
    }

    fn of<'a>(child: &'a str, target: &'a str, work: bool) -> Of<'a> {
        Of { child, target, work, kind: kind_of(child), details: "Details: Tom’s agenda", away: "Away" }
    }

    /// The stand-in: the child's title, times and place, what it stands for
    /// in its lines, its UID derived; made twice, the same bytes, never a
    /// second copy; found again by its UID where a server dropped its lines.
    #[test]
    fn a_stand_in_is_made_once() {
        let made = stand_in(&of(DENTIST, "me/personal", false), 1_793_000_000).unwrap();
        let uid = stand_in_uid("dentist-1", "me/personal");
        for line in [
            format!("UID:{uid}"),
            "DTSTART:20261110T090000Z".into(),
            "SUMMARY:Dentist".into(),
            "LOCATION:12 rue des Lilas".into(),
            "RELATED-TO;RELTYPE=PARENT:dentist-1".into(),
            "URL:sioul:event/dentist-1".into(),
            "X-SIOUL-LINK;LINKREL=latest-version;VALUE=URI:sioul:event/dentist-1".into(),
            "DTSTAMP:20261101T080000Z".into(),
        ] {
            assert!(flat(&made).contains(&line), "{line}: {made}");
        }
        assert!(lines::unfold(&made).iter().any(|l| l == "DESCRIPTION:Details: Tom’s agenda\\nsioul:event/dentist-1"), "{made}");
        assert!(!made.contains("X-OTHER-APP") && !made.contains("VALARM"), "only what it copies");
        assert_eq!(stands_for(&made).as_deref(), Some("dentist-1"));
        // Made again, on another device: the same bytes, the same UID: one copy.
        assert_eq!(stand_in(&of(DENTIST, "me/personal", false), 1_799_999_999).unwrap(), made);
        // Its lines dropped by a server: its UID still says it.
        let dropped: String = lines::fold(&lines::unfold(&made).into_iter().filter(|l| !l.starts_with("RELATED-TO") && !l.starts_with("X-SIOUL-LINK") && !l.starts_with("URL")).collect::<Vec<_>>());
        assert_eq!(stands_for(&dropped), None);
        assert!(dropped.contains(&format!("UID:{}", stand_in_uid("dentist-1", "me/personal"))));
        // Never a stand-in of an event that is no event.
        assert!(stand_in(&of("BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:t\r\nEND:VTODO\r\nEND:VCALENDAR\r\n", "me/personal", false), 0).is_none());
    }

    /// A work stand-in carries no title for health (recommendation 16):
    /// "Away", no place, CLASS:CONFIDENTIAL; an activity keeps its title.
    #[test]
    fn a_work_stand_in_names_no_health() {
        let dentist = with_kind(DENTIST, Some(Kind::Dentist)).unwrap();
        let away = stand_in(&of(&dentist, "work/main", true), 0).unwrap();
        assert!(flat(&away).contains("SUMMARY:Away") && away.contains("CLASS:CONFIDENTIAL"), "{away}");
        assert!(!away.contains("Dentist") && !away.contains("rue des Lilas"), "{away}");
        assert!(stand_in(&of(&dentist, "me/personal", false), 0).unwrap().contains("SUMMARY:Dentist"));
        let football = with_kind(&DENTIST.replace("SUMMARY:Dentist", "SUMMARY:Football"), Some(Kind::Activity)).unwrap();
        let shown = stand_in(&of(&football, "work/main", true), 0).unwrap();
        assert!(shown.contains("SUMMARY:Football") && !shown.contains("CLASS:"), "{shown}");
    }

    /// The stand-in follows a move and a cancellation of its child event,
    /// the holder's own lines kept (margins, reminder, alarm); once it says
    /// what the child does, nothing more is written.
    #[test]
    fn a_stand_in_follows_a_move_and_a_cancellation() {
        let made = stand_in(&of(DENTIST, "me/personal", false), 0).unwrap();
        let mine = made.replace("END:VEVENT", "X-SIOUL-BEFORE:20\r\nX-SIOUL-REMIND:30\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT30M\r\nEND:VALARM\r\nEND:VEVENT");
        assert_eq!(followed(&mine, &of(DENTIST, "me/personal", false), 0), None, "nothing to follow");
        let moved_child = DENTIST.replace("DTSTART:20261110T090000Z", "DTSTART:20261112T140000Z").replace("DTEND:20261110T100000Z", "DTEND:20261112T150000Z").replace("DTSTAMP:20261101T080000Z", "DTSTAMP:20261102T080000Z");
        let moved = followed(&mine, &of(&moved_child, "me/personal", false), 0).unwrap();
        assert!(moved.contains("DTSTART:20261112T140000Z") && moved.contains("DTEND:20261112T150000Z") && !moved.contains("20261110T09"), "{moved}");
        for kept in ["X-SIOUL-BEFORE:20", "X-SIOUL-REMIND:30", "TRIGGER:-PT30M", "RELATED-TO;RELTYPE=PARENT:dentist-1", "DTSTAMP:20261102T080000Z"] {
            assert!(moved.contains(kept), "{kept}: {moved}");
        }
        assert_eq!(moved.matches("DTSTAMP").count(), 1);
        assert_eq!(followed(&moved, &of(&moved_child, "me/personal", false), 0), None, "once followed, nothing more");
        let cancelled_child = moved_child.replace("SUMMARY:Dentist", "SUMMARY:Dentist\r\nSTATUS:CANCELLED");
        let cancelled = followed(&moved, &of(&cancelled_child, "me/personal", false), 0).unwrap();
        assert!(cancelled.contains("STATUS:CANCELLED") && cancelled.contains("X-SIOUL-BEFORE:20"), "{cancelled}");
        // Its zones follow too: a child on Paris' clock brings its VTIMEZONE.
        let paris = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTIMEZONE\r\nTZID:Europe/Paris\r\nBEGIN:STANDARD\r\nDTSTART:19701025T030000\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:dentist-1\r\nDTSTART;TZID=Europe/Paris:20261112T150000\r\nDTEND;TZID=Europe/Paris:20261112T160000\r\nSUMMARY:Dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let zoned = followed(&moved, &of(paris, "me/personal", false), 0).unwrap();
        assert!(zoned.contains("TZID:Europe/Paris") && zoned.find("BEGIN:VTIMEZONE").unwrap() < zoned.find("BEGIN:VEVENT").unwrap(), "{zoned}");
        assert_eq!(followed(&zoned, &of(paris, "me/personal", false), 0), None);
    }

    /// The iTIP REQUEST's lines (part 3): the child's UID kept, you its
    /// organizer, the holder a required guest asked to answer, the details
    /// and the address back; an update counts one more; a cancellation says
    /// so. A REPLY sets the holder line's PARTSTAT and SINCE.
    #[test]
    fn the_invitation_s_lines_and_a_reply_setting_the_answer() {
        let zone = TimeZone::UTC;
        let alex = Person { name: "Alex", address: "alex@example.org" };
        let sam_person = Person { name: "Sam", address: "sam@example.org" };
        let invite = Invite { method: "REQUEST", organizer: alex, holder: sam_person, details: "Tom’s dentist. Details: Tom’s agenda", url: "sioul:event/dentist-1", sequence: 0, now: 1_793_000_000 };
        let request = invitation(DENTIST, &invite).unwrap();
        let lines_of = lines::unfold(&request);
        for line in [
            "METHOD:REQUEST",
            "UID:dentist-1",
            "SEQUENCE:0",
            "DTSTART:20261110T090000Z",
            "DTEND:20261110T100000Z",
            "SUMMARY:Dentist",
            "LOCATION:12 rue des Lilas",
            "ORGANIZER;CN=Alex:mailto:alex@example.org",
            "ATTENDEE;CN=Sam;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:sam@example.org",
            "URL:sioul:event/dentist-1",
            "DESCRIPTION:Tom’s dentist. Details: Tom’s agenda",
        ] {
            assert!(lines_of.iter().any(|l| l == line), "{line}: {request}");
        }
        assert!(!request.contains("X-OTHER-APP") && !request.contains("VALARM"), "only what it needs");
        let update = invitation(DENTIST, &Invite { sequence: 1, ..invite }).unwrap();
        assert!(update.contains("SEQUENCE:1"));
        let cancel = invitation(DENTIST, &Invite { method: "CANCEL", sequence: 2, ..invite }).unwrap();
        assert!(cancel.contains("METHOD:CANCEL") && cancel.contains("STATUS:CANCELLED") && !cancel.contains("RSVP"), "{cancel}");
        // Sam's REPLY, from their calendar: accepted, at its DTSTAMP.
        let asked = with_taken(DENTIST, None, &Taken { to: Some(sam(Answer::NeedsAction)), back: None }, &zone).unwrap();
        let reply = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REPLY\r\nBEGIN:VEVENT\r\nUID:dentist-1\r\nDTSTAMP:20261103T090000Z\r\nATTENDEE;PARTSTAT=ACCEPTED;CN=Sam:mailto:SAM@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let answered = replied(&asked, reply, &zone).unwrap();
        let holder = super::taken(&answered, None, &zone).to.unwrap();
        assert_eq!((holder.answer, holder.since), (Answer::Accepted, crate::moves::read_utc("20261103T090000Z")));
        assert_eq!(replied(&answered, reply, &zone), None, "said already");
        // Another event's, someone it does not name, an older answer: nothing.
        assert_eq!(replied(&asked, &reply.replace("UID:dentist-1", "UID:other"), &zone), None);
        assert_eq!(replied(&asked, &reply.replace("SAM@example.org", "eve@example.org"), &zone), None);
        let older = reply.replace("20261103T090000Z", "20261020T090000Z").replace("ACCEPTED", "DECLINED");
        assert_eq!(replied(&answered, &older, &zone), None);
        assert_eq!(reply_of(&request), None, "a REQUEST is no answer");
    }

    /// The follow-up task (part 3): "Book Tom's next dentist appointment",
    /// held by the same parent, due a month before the rhythm comes round,
    /// its UID the same on every device.
    #[test]
    fn a_follow_up_task_comes_after_a_held_event() {
        let alex = Holder { name: "Alex".into(), address: "alex@example.org".into(), answer: Answer::Accepted, since: Some(1_793_000_000), back: false };
        let task = follow_up("dentist-1", "Book Tom’s next dentist appointment", Kind::Dentist, jiff::civil::date(2026, 11, 10), Kind::Dentist.rhythm().unwrap(), &alex, 1_793_000_000).unwrap();
        for line in [
            "BEGIN:VTODO".to_string(),
            format!("UID:{}", follow_up_uid("dentist-1")),
            "SUMMARY:Book Tom’s next dentist appointment".into(),
            "DUE;VALUE=DATE:20270410".into(),
            "X-SIOUL-CONCEPT;VALUE=URI:tag:aurelienpierre.com,2026:sioul/child-kind/dentist".into(),
            "X-SIOUL-LINK;LINKREL=related;VALUE=URI:sioul:event/dentist-1".into(),
            holder_line(&alex),
        ] {
            assert!(lines::unfold(&task).contains(&line), "{line}: {task}");
        }
        assert_eq!(super::taken(&task, None, &TimeZone::UTC).to, Some(alex.clone()));
        let check_up = follow_up("health-1", "Book Tom’s next check-up", Kind::Health, jiff::civil::date(2026, 11, 10), 12, &alex, 0).unwrap();
        assert!(check_up.contains("DUE;VALUE=DATE:20271010"), "{check_up}");
        assert_eq!(Kind::Activity.rhythm(), None);
    }

    fn tom() -> crate::agendas::Agenda {
        let collection = crate::vdir::Collection { kind: crate::vdir::Kind::Calendars, account: "family".into(), id: "tom".into(), dir: PathBuf::from("/nowhere/family/tom"), name: "Tom".into(), color: None, read_only: false, components: vec!["VEVENT".into(), "VTODO".into()] };
        let said = crate::agendas::Said { whose: Some("child".into()), name: Some("Tom".into()), parent: Some("Sam".into()), parent_address: Some("sam@example.org".into()), ..Default::default() };
        crate::agendas::Agenda::of(&collection, &said, &crate::config::Config::default(), false, &|| true)
    }

    /// The upkeep (part 3): you take a child's event and your stand-in is
    /// made once, where its kind's go; moved, it follows; asked of Sam, it is
    /// taken out; a line a server dropped takes nothing out; once over, a
    /// held dentist's gets its follow-up, once; a child event gone writes nothing.
    #[test]
    fn the_upkeep_makes_follows_and_gives_stand_ins() {
        let zone = TimeZone::UTC;
        let own = vec!["alex@example.org".to_string()];
        let tom = tom();
        let personal = Target { place: "me/personal".into(), dir: PathBuf::from("/nowhere/me/personal"), work: false };
        let target = |_: Option<Kind>| Some(personal.clone());
        let details = |a: &crate::agendas::Agenda| format!("Details: {}’s agenda", a.person);
        let follow_up = |a: &crate::agendas::Agenda, kind: Kind| kind.rhythm().map(|months| FollowUp { title: format!("Book {}’s next dentist appointment", a.person), months, dir: PathBuf::from("/nowhere/family/tom") });
        let before_it = 1_793_500_000; // 1 November 2026, before the appointment
        let cx = |now: i64| Context { own: &own, target: &target, details: &details, away: "Away", follow_up: &follow_up, now, zone: &zone };
        let me = Holder { name: "Alex".into(), address: "alex@example.org".into(), answer: Answer::Accepted, since: Some(before_it), back: false };
        let held = with_kind(&with_taken(DENTIST, None, &Taken { to: Some(me.clone()), back: None }, &zone).unwrap(), Some(Kind::Dentist)).unwrap();
        let child_path = PathBuf::from("/nowhere/family/tom/dentist-1.ics");
        let children = [ChildItem { path: &child_path, text: &held, agenda: &tom }];
        // Taken, none there: made once, its file named after its UID.
        let acts = upkeep(&children, &[], &cx(before_it));
        let uid = stand_in_uid("dentist-1", "me/personal");
        let [Act::Made { path, text, child }] = acts.as_slice() else { panic!("{acts:?}") };
        assert_eq!((path.clone(), child.as_str()), (PathBuf::from(format!("/nowhere/me/personal/{uid}.ics")), "dentist-1"));
        assert!(flat(text).contains("Details: Tom’s agenda"), "{text}");
        // There already: nothing more, by its lines or by its UID alone.
        let stand_path = path.clone();
        let mine = [Mine { path: &stand_path, text, place: "me/personal", work: false }];
        assert_eq!(upkeep(&children, &mine, &cx(before_it)), []);
        let dropped: String = lines::fold(&lines::unfold(text).into_iter().filter(|l| !l.starts_with("RELATED-TO") && !l.starts_with("X-SIOUL-LINK")).collect::<Vec<_>>());
        let found_by_uid = [Mine { path: &stand_path, text: &dropped, place: "me/personal", work: false }];
        assert_eq!(upkeep(&children, &found_by_uid, &cx(before_it)), []);
        // Moved: it follows.
        let moved = held.replace("DTSTART:20261110T090000Z", "DTSTART:20261112T090000Z").replace("DTEND:20261110T100000Z", "DTEND:20261112T100000Z");
        let moved_children = [ChildItem { path: &child_path, text: &moved, agenda: &tom }];
        let acts = upkeep(&moved_children, &mine, &cx(before_it));
        assert!(matches!(acts.as_slice(), [Act::Followed { text, .. }] if text.contains("DTSTART:20261112T090000Z")), "{acts:?}");
        // Asked of Sam: taken out; Sam's line dropped by a server: kept.
        let asked = with_taken(&held, None, &Taken { to: Some(sam(Answer::NeedsAction)), back: None }, &zone).unwrap();
        let asked_children = [ChildItem { path: &child_path, text: &asked, agenda: &tom }];
        assert_eq!(upkeep(&asked_children, &mine, &cx(before_it)), [Act::Given { path: stand_path.clone(), child: "dentist-1".into() }]);
        let nobody = with_taken(&held, None, &Taken::default(), &zone).unwrap();
        let nobody_children = [ChildItem { path: &child_path, text: &nobody, agenda: &tom }];
        assert_eq!(upkeep(&nobody_children, &mine, &cx(before_it)), []);
        assert_eq!(upkeep(&nobody_children, &[], &cx(before_it)), [], "nobody takes it: nothing made");
        // You said not this time: taken out too.
        let declined = with_taken(&held, None, &Taken { to: Some(Holder { answer: Answer::Declined, ..me.clone() }), back: None }, &zone).unwrap();
        assert!(matches!(upkeep(&[ChildItem { path: &child_path, text: &declined, agenda: &tom }], &mine, &cx(before_it)).as_slice(), [Act::Given { .. }]));
        // Over (the next day): its follow-up, once, held by you, due a month before six months on.
        let after = 1_794_400_000; // 11 November 2026
        let acts = upkeep(&children, &mine, &cx(after));
        let Some(Act::FollowUp { path, text, .. }) = acts.iter().find(|a| matches!(a, Act::FollowUp { .. })) else { panic!("{acts:?}") };
        assert_eq!(path, &PathBuf::from(format!("/nowhere/family/tom/{}.ics", follow_up_uid("dentist-1"))));
        assert!(flat(text).contains("SUMMARY:Book Tom’s next dentist appointment") && text.contains("DUE;VALUE=DATE:20270410"), "{text}");
        assert_eq!(super::taken(text, None, &zone).to.map(|h| h.address), Some("alex@example.org".into()));
        let task_path = path.clone();
        let with_task = [ChildItem { path: &child_path, text: &held, agenda: &tom }, ChildItem { path: &task_path, text, agenda: &tom }];
        assert!(!upkeep(&with_task, &mine, &cx(after)).iter().any(|a| matches!(a, Act::FollowUp { .. })), "once");
        // A month and more after: none (follow-ups never come for the past).
        assert!(!upkeep(&children, &mine, &cx(after + 40 * 86_400)).iter().any(|a| matches!(a, Act::FollowUp { .. })));
        // The child event gone: nothing written, nothing lost.
        assert_eq!(upkeep(&[], &mine, &cx(before_it)), []);
        // Taken once over (another device's first look): no stand-in for the past.
        assert!(!upkeep(&children, &[], &cx(after)).iter().any(|a| matches!(a, Act::Made { .. })));
    }

    /// Asked once (recommendation 9): what was there at the first look is
    /// never asked; what comes after is; "Later" asks again from the next
    /// admin window, then never; a proposal to you is asked until answered;
    /// what is gone is forgotten.
    #[test]
    fn each_question_is_asked_once() {
        let own = vec!["alex@example.org".to_string()];
        let zone = TimeZone::UTC;
        let mut asked = Asked::default();
        let uids = |list: &[&str]| list.iter().map(|u| u.to_string()).collect::<std::collections::BTreeSet<_>>();
        assert!(asked.look(&uids(&["old"])));
        assert!(!asked.due("old", Asking::Nobody, 0), "there at the first look");
        asked.look(&uids(&["old", "new"]));
        assert!(asked.due("new", Asking::Nobody, 0));
        asked.put_off("new", 100);
        assert!(!asked.due("new", Asking::Nobody, 50) && asked.due("new", Asking::Nobody, 100));
        asked.put_off("new", 200);
        assert!(!asked.due("new", Asking::Nobody, 300), "twice: never");
        assert_eq!(asking(DENTIST, &own, &zone), Some(Asking::Nobody));
        let proposed = with_taken(DENTIST, None, &Taken { to: Some(Holder { name: "Alex".into(), address: "alex@example.org".into(), ..Holder::default() }), back: None }, &zone).unwrap();
        assert_eq!(asking(&proposed, &own, &zone), Some(Asking::Proposed));
        assert!(asked.due("old", Asking::Proposed, 0), "a proposal to you, even of what was there");
        let reopened = with_taken(DENTIST, None, &Taken { to: Some(sam(Answer::Declined)), back: None }, &zone).unwrap();
        assert_eq!(asking(&reopened, &own, &zone), Some(Asking::Reopened));
        let waiting = with_taken(DENTIST, None, &Taken { to: Some(sam(Answer::NeedsAction)), back: None }, &zone).unwrap();
        assert_eq!(asking(&waiting, &own, &zone), None, "Sam asked, not answered yet: nothing asks you");
        assert!(asked.say_once("dentist-1|5") && !asked.say_once("dentist-1|5"));
        asked.answered("new");
        assert!(!asked.due("new", Asking::Nobody, 1_000));
        asked.look(&uids(&["new"]));
        assert_eq!((asked.seen.len(), asked.said.len()), (1, 0), "what is gone is forgotten");
    }

    /// A child's task sits in its holder's plan and Next, never in another's
    /// (recommendation 7); nobody's yet, in nobody's; your own lists' as ever.
    #[test]
    fn a_child_s_task_is_planned_by_its_holder_alone() {
        use crate::tasks::Task;
        let own = vec!["alex@example.org".to_string()];
        let alex = Holder { name: "Alex".into(), address: "alex@example.org".into(), answer: Answer::Accepted, since: None, back: false };
        let task = |uid: &str, list: &str, holder: Option<Holder>| Task { uid: uid.into(), title: uid.into(), list_id: list.into(), holder, estimate: 30, ..Task::default() };
        let mut tasks = vec![
            task("coat", "family/tom", Some(alex.clone())),
            task("shoes", "family/tom", Some(sam(Answer::Accepted))),
            task("form", "family/tom", None),
            task("asked", "family/tom", Some(Holder { answer: Answer::NeedsAction, ..alex })),
            task("mine", "me/tasks", None),
        ];
        mark_tasks(&mut tasks, &["family/tom".to_string()].into(), &own);
        assert_eq!(tasks.iter().map(|t| t.elsewhere).collect::<Vec<_>>(), [false, true, true, true, false]);
        let plan = crate::plan::plan(&tasks, jiff::civil::date(2026, 11, 4), &crate::plan::Settings::default(), &BTreeMap::new(), &std::collections::BTreeSet::new());
        let ready: Vec<&str> = plan.ready.iter().map(String::as_str).collect();
        assert!(ready.contains(&"coat") && ready.contains(&"mine"), "{ready:?}");
        assert!(!ready.contains(&"shoes") && !ready.contains(&"form") && !ready.contains(&"asked"), "{ready:?}");
    }

    /// A holder line read leniently: no CN, a PARTSTAT this Sioul does not
    /// know (proposed), MAILTO in capitals; none without an address.
    #[test]
    fn a_holder_line_reads_leniently() {
        let read = parse_line("X-SIOUL-TAKEN-BY;PARTSTAT=TENTATIVE:MAILTO:sam@example.org").unwrap();
        assert_eq!((read.name.as_str(), read.address.as_str(), read.answer, read.since, read.back), ("", "sam@example.org", Answer::NeedsAction, None, false));
        assert_eq!(parse_line("X-SIOUL-TAKEN-BY;CN=Sam:"), None);
        assert_eq!(parse_line("ATTENDEE:mailto:sam@example.org"), None);
    }
}
