// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Meetings you organize (decision 7 of the event card, 5 November 2026):
//! an event's guests set from the form's "Invite…" field (`with_guests`),
//! what they must hear of a change (`told`), the invitation and the
//! cancellation sent to them when the calendar's server does not send them
//! itself (iTIP over iMIP, RFC 5546 and RFC 6047: `request`, `cancel`), and
//! a guest's answer read from their REPLY (`replied`).
//!
//! **Who sends: per meeting, per guest, read from its file** (the owner's
//! word, 6 November 2026: "per meeting"). A guest marked
//! `SCHEDULE-AGENT=CLIENT` (RFC 6638 §7.1) is left to you: Sioul drafts their
//! invitation, update or cancellation (iTIP over iMIP), which you send from
//! Mail, and a server schedules nothing for them. A guest without it is told
//! by the calendar's server when it schedules by itself for the organizer's
//! address (RFC 6638, `calendar-auto-schedule`, as Nextcloud does); else
//! Sioul drafts for them too (`teller`). The mark of a guest already in the
//! file is never added nor taken out: added, sabre would cancel the meeting
//! for them; taken out, nobody would tell them again (review of 1e8551e, F1,
//! F2). A meeting made in Sioul marks every guest, those added later too; one
//! made elsewhere without the mark (Nextcloud's web page, Thunderbird) has
//! its new guests unmarked, told as the others are (`marks_new_guests`). A
//! change its server would not send (sabre sends a change of time, rule or
//! status alone: `significant`) reaches its server-told guests by
//! `SCHEDULE-FORCE-SEND=REQUEST` for that write (RFC 6638 §7.3, which sabre
//! honours and takes out of the stored copy once delivered:
//! `with_force_send`; F3). Never both for one guest and one change.
//!
//! These texts are written through the organizer's door alone
//! (`vdir::write_organized`): every other writer refuses a meeting with others.
//!
//! **What guests read.** Every message Sioul drafts for its guests holds
//! only what they need (`SHARED`, an allow-list of RFC 5546's REQUEST and
//! CANCEL properties): its times, how it repeats, its title, place and
//! notes, its status and who comes (review of 1e8551e, F4). A meeting's
//! file, which its server sends whole to the guests it tells, holds none of
//! Sioul's own lines, ties, alarms nor addresses (`for_guests`) and keeps
//! every other program's (its second review, N6): the door sets Sioul's
//! aside (`set_aside`), into your own files: the time kept around it, its
//! costs and gain and your reminder (`own_time`), your ties (`links`), and
//! the rest (`meeting_lines`: who takes a child's event, its kind).
//!
//! **Its guests' messages after a set-aside change** (`reconcile`): what
//! each guest still needs to hear, worked out from the server's version.

use crate::lines;
use serde::{Deserialize, Serialize};

/// A guest, or you as organizer: an address and the name to show.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guest {
    pub address: String,
    #[serde(default)]
    pub name: String,
}

impl Guest {
    fn is(&self, address: &str) -> bool {
        self.address.trim().eq_ignore_ascii_case(address.trim())
    }

    /// Its name when it has one, else its address: what a sentence says.
    pub fn said(&self) -> String {
        if self.name.trim().is_empty() { self.address.trim().to_string() } else { self.name.trim().to_string() }
    }
}

/// Who tells a guest of a meeting: its calendar's server, or you, from a
/// draft; for the guests a write adds, how they are marked (`with_guests`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sender {
    /// The server schedules by itself (RFC 6638): Sioul drafts nothing for them.
    Server,
    /// Sioul drafts the iTIP message; you send it from Mail (`SCHEDULE-AGENT=CLIENT`).
    Draft,
}

/// Whether a guest's line leaves them to you (`SCHEDULE-AGENT=CLIENT`).
pub fn left_to_you_line(line: &str) -> bool {
    lines::param(line, "SCHEDULE-AGENT").is_some_and(|a| a.trim().eq_ignore_ascii_case("CLIENT"))
}

/// Each guest of a meeting with how their lines mark them, read as a server
/// reads them: every ATTENDEE line of every event (the series and each time
/// of it changed), each alone (sabre/vobject's ITip Broker skips a line
/// marked `SCHEDULE-AGENT=CLIENT` per event and per line, `parseEventInfo`,
/// read 6 November 2026; review of 1e8551e's second review, N10). By address
/// in lower case, in order: (address, some line left to you, some line not).
fn marks(text: &str) -> Vec<(String, bool, bool)> {
    let mut out: Vec<(String, bool, bool)> = Vec::new();
    let organizer = organizer_of(text).map(|o| o.address.to_lowercase()).unwrap_or_default();
    let (mut inside, mut nested) = (false, 0usize);
    for line in lines::unfold(text) {
        match lines::name(&line).as_str() {
            "BEGIN" if !inside => inside = lines::value(&line).trim().eq_ignore_ascii_case("VEVENT"),
            "BEGIN" => nested += 1,
            "END" if inside && nested > 0 => nested -= 1,
            "END" if inside => inside = false,
            "ATTENDEE" if inside && nested == 0 => {
                let who = address_of(lines::value(&line));
                if who.is_empty() || who == organizer {
                    continue;
                }
                let client = left_to_you_line(&line);
                match out.iter_mut().find(|(a, _, _)| *a == who) {
                    Some(seen) => {
                        seen.1 |= client;
                        seen.2 |= !client;
                    }
                    None => out.push((who, client, !client)),
                }
            }
            _ => {}
        }
    }
    out
}

/// The guests of a meeting left to you (`left_to_you_line`) on every line
/// naming them, in every event of it: no server tells them anything of it.
/// By address in lower case.
pub fn clients(text: &str) -> Vec<String> {
    marks(text).into_iter().filter(|(_, client, other)| *client && !*other).map(|(a, _, _)| a).collect()
}

/// The guests a meeting leaves to you on some of its lines and not on others
/// (another program copied a time of it with its own marks): a server tells
/// them of the times where they are not left to you, and nothing of the
/// others. A change that would tell them is refused where a server
/// schedules (`organized::plan`): Sioul cannot tell them once.
pub fn mixed(text: &str) -> Vec<String> {
    marks(text).into_iter().filter(|(_, client, other)| *client && *other).map(|(a, _, _)| a).collect()
}

/// Who tells `address`, a guest of the meeting `text`: your drafts when
/// every line of theirs leaves them to you, else `server` (its calendar's
/// server when it schedules for the organizer, else your drafts too).
pub fn teller(text: &str, address: &str, server: Sender) -> Sender {
    if clients(text).contains(&address.trim().to_lowercase()) { Sender::Draft } else { server }
}

/// A meeting's every guest left to you (`SCHEDULE-AGENT=CLIENT`), for the
/// file Sioul makes of your own invitation seen in your mail: your message
/// told them, so no server tells them again (`organized::adopt`). Never for a
/// file already there: a guest's mark is never changed in a file (F1, F2).
pub fn every_guest_left_to_you(text: &str) -> String {
    let organizer = organizer_of(text).map(|o| o.address.to_lowercase()).unwrap_or_default();
    let source = lines::unfold(text);
    let (mut inside, mut nested) = (false, 0usize);
    let mut out = Vec::with_capacity(source.len());
    for line in &source {
        match lines::name(line).as_str() {
            "BEGIN" if !inside => inside = lines::value(line).trim().eq_ignore_ascii_case("VEVENT"),
            "BEGIN" => nested += 1,
            "END" if inside && nested > 0 => nested -= 1,
            "END" if inside => inside = false,
            "ATTENDEE" if inside && nested == 0 && !left_to_you_line(line) && address_of(lines::value(line)) != organizer => {
                out.push(with_params(line, &["SCHEDULE-AGENT"], &[("SCHEDULE-AGENT", "CLIENT")]));
                continue;
            }
            _ => {}
        }
        out.push(line.clone());
    }
    if out == source { text.to_string() } else { lines::fold(&out) }
}

/// How the guests a write adds to `text` are marked (the owner's word, 6
/// November 2026): left to you (`Sender::Draft`, `SCHEDULE-AGENT=CLIENT`)
/// in a meeting made in Sioul, or an event of no guest yet; unmarked, told
/// as its others are (`Sender::Server`), in a meeting made elsewhere, whose
/// guests none is left to you.
pub fn marks_new_guests(text: &str) -> Sender {
    let marks = marks(text);
    if !marks.is_empty() && !marks.iter().any(|(_, client, _)| *client) { Sender::Server } else { Sender::Draft }
}

/// "mailto:Jane@Example.org" → "jane@example.org".
fn address_of(value: &str) -> String {
    let value = value.trim();
    value.get(..7).filter(|p| p.eq_ignore_ascii_case("mailto:")).map_or(value, |_| &value[7..]).trim().to_lowercase()
}

/// The guests an event names (its main VEVENT, `agenda::participants`), its
/// organizer left out, each address once, as the form lists them.
pub fn guests_of(text: &str) -> Vec<Guest> {
    let people = crate::agenda::participants(text);
    let organizer = people.iter().find(|p| p.organizer).map(|p| p.address.to_lowercase());
    people.into_iter().filter(|p| !p.organizer && Some(p.address.to_lowercase()) != organizer).map(|p| Guest { address: p.address, name: p.name }).collect()
}

/// The organizer an event names (its main VEVENT), if any.
pub fn organizer_of(text: &str) -> Option<Guest> {
    crate::agenda::participants(text).into_iter().find(|p| p.organizer).map(|p| Guest { address: p.address, name: p.name })
}

/// Every guest of a meeting, in any of its events (the series and each time
/// of it changed), each once, in order, its organizer left out: the series'
/// guests, then those another program asked to one time of it alone.
pub fn every_guest(text: &str) -> Vec<Guest> {
    let mut out = guests_of(text);
    let organizer = organizer_of(text).map(|o| o.address.to_lowercase()).unwrap_or_default();
    let source = lines::unfold(text);
    for range in events(&source) {
        let mut nested = 0usize;
        for line in &source[range.start + 1..range.end - 1] {
            match lines::name(line).as_str() {
                "BEGIN" => nested += 1,
                "END" => nested = nested.saturating_sub(1),
                "ATTENDEE" if nested == 0 => {
                    let address = address_of(lines::value(line));
                    if address.is_empty() || address == organizer || out.iter().any(|g| g.is(&address)) {
                        continue;
                    }
                    let name = lines::param(line, "CN").unwrap_or_default();
                    out.push(Guest { address, name });
                }
                _ => {}
            }
        }
    }
    out
}

/// The guests asked to some times of a meeting alone, not to its series
/// (another program put them there): `every_guest` less `guests_of`.
pub fn one_time_guests(text: &str) -> Vec<Guest> {
    let series = guests_of(text);
    every_guest(text).into_iter().filter(|g| !series.iter().any(|s| s.is(&g.address))).collect()
}

/// Whether one of an event's lines names `address` as a guest.
fn names_guest(block: &[String], address: &str) -> bool {
    let mut nested = 0usize;
    for line in &block[1..block.len().saturating_sub(1)] {
        match lines::name(line).as_str() {
            "BEGIN" => nested += 1,
            "END" => nested = nested.saturating_sub(1),
            "ATTENDEE" if nested == 0 && address_of(lines::value(line)).eq_ignore_ascii_case(address.trim()) => return true,
            _ => {}
        }
    }
    false
}

/// The invitation of a meeting for guests asked to some times of it alone
/// (`one_time_guests`): only the events that name one of `to`, as RFC 5546
/// §3.2.2 sends one time of a series (its RECURRENCE-ID). None when no event
/// names them.
pub fn request_for(text: &str, to: &[String]) -> Option<String> {
    let source = lines::unfold(text);
    let ranges: Vec<_> = events(&source).into_iter().filter(|r| to.iter().any(|a| names_guest(&source[r.clone()], a))).collect();
    if ranges.is_empty() {
        return None;
    }
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "METHOD:REQUEST".to_string()];
    out.extend(zones(&source));
    for range in ranges {
        out.extend(as_sent(&source[range]));
    }
    out.push("END:VCALENDAR".to_string());
    Some(lines::fold(&out))
}

/// A line's head and value apart, at its colon (`lines::value`).
fn split(line: &str) -> (&str, &str) {
    let value = lines::value(line);
    let at = line.len().saturating_sub(value.len() + 1);
    if line.as_bytes().get(at) == Some(&b':') { (&line[..at], value) } else { (line, "") }
}

/// A line's parameters as written, quotes kept, split where no quote holds them.
fn written_params(head: &str) -> Vec<&str> {
    let (mut parts, mut quoted, mut start) = (Vec::new(), false, 0);
    for (i, c) in head.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                parts.push(&head[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&head[start..]);
    parts
}

/// A line with the parameters named in `drop` taken out and `add` written after the others.
fn with_params(line: &str, drop: &[&str], add: &[(&str, &str)]) -> String {
    let (head, value) = split(line);
    let parts = written_params(head);
    let mut out = parts.first().copied().unwrap_or_default().to_string();
    for part in parts.iter().skip(1) {
        let name = part.split('=').next().unwrap_or_default().trim().to_ascii_uppercase();
        if !drop.contains(&name.as_str()) && !add.iter().any(|(n, _)| *n == name) {
            out.push(';');
            out.push_str(part);
        }
    }
    for (name, value) in add {
        out.push_str(&format!(";{name}={value}"));
    }
    format!("{out}:{value}")
}

fn person_line(property: &str, who: &Guest, more: &str) -> String {
    let name = if who.name.trim().is_empty() { String::new() } else { format!(";CN={}", lines::param_value(who.name.trim())) };
    format!("{property}{name}{more}:mailto:{}", who.address.trim())
}

/// A new guest's line: asked to answer (RFC 5546 §3.2.2), and left to you to
/// tell when you send the invitation (`Sender::Draft`: `marks_new_guests`).
fn guest_line(guest: &Guest, sender: Sender) -> String {
    let agent = if sender == Sender::Draft { ";SCHEDULE-AGENT=CLIENT" } else { "" };
    person_line("ATTENDEE", guest, &format!(";ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE{agent}"))
}

/// The ranges of the calendar's VEVENTs (BEGIN to END, both included).
fn events(source: &[String]) -> Vec<std::ops::Range<usize>> {
    let (mut out, mut start, mut depth) = (Vec::new(), None, 0usize);
    for (i, line) in source.iter().enumerate() {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        match (name.as_str(), start) {
            ("BEGIN", None) if value == "VEVENT" => {
                start = Some(i);
                depth = 0;
            }
            ("BEGIN", Some(_)) => depth += 1,
            ("END", Some(_)) if depth > 0 => depth -= 1,
            ("END", Some(begin)) => {
                out.push(begin..i + 1);
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// Whether an ORGANIZER line names `organizer` as a server reads it: the
/// `mailto:` URI of its address (review of 1e8551e, F15).
fn names_organizer(line: &str, organizer: &Guest) -> bool {
    let value = lines::value(line).trim();
    value.get(..7).is_some_and(|scheme| scheme.eq_ignore_ascii_case("mailto:")) && organizer.is(&address_of(value))
}

/// The event with you as its organizer and these guests, in every VEVENT
/// of it (the series and each time of it changed): a guest kept keeps its
/// line as it is (its answer, its role), a new one asked to answer, one no
/// longer listed taken out; your own line among the guests, if any, kept.
/// A time of it changed keeps its own guests (another program excused one
/// from it), less those taken off the series, plus those added to it
/// (review of 1e8551e, F11). An ORGANIZER line is written as the `mailto:`
/// URI of your address (F15). A guest's line kept is kept as it is, its
/// SCHEDULE-AGENT never added nor taken out (F1, F2); `added` says how the
/// new ones are marked (`marks_new_guests`). Its stamps are renewed where it
/// changed. The same text when nothing changes; None for no event.
pub fn with_guests(text: &str, organizer: &Guest, guests: &[Guest], added: Sender) -> Option<String> {
    let source = lines::unfold(text);
    let ranges = events(&source);
    if ranges.is_empty() {
        return None;
    }
    let now = crate::moves::utc(crate::zone::now().timestamp().as_second());
    let added_marks = added;
    let wanted: Vec<&Guest> = guests.iter().filter(|g| !g.address.trim().is_empty() && !organizer.is(&g.address)).collect();
    // The series' guests as they are: who is taken off it, and who added, for each time changed.
    let had: Vec<String> = guests_of(text).into_iter().map(|g| g.address.trim().to_lowercase()).filter(|a| !organizer.is(a)).collect();
    let taken_off: Vec<&String> = had.iter().filter(|a| !wanted.iter().any(|g| g.is(a))).collect();
    let added_to: Vec<&Guest> = wanted.iter().copied().filter(|g| !had.iter().any(|a| g.is(a))).collect();
    let mut out: Vec<String> = Vec::with_capacity(source.len() + guests.len() * 2);
    let mut at = 0;
    for range in ranges {
        out.extend(source[at..range.start].iter().cloned());
        at = range.end;
        let block = &source[range];
        let one_time = block.iter().any(|l| lines::name(l) == "RECURRENCE-ID");
        let mut kept: Vec<String> = Vec::with_capacity(block.len() + wanted.len() + 1);
        let (mut nested, mut has_organizer, mut changed) = (0usize, false, false);
        let mut present: Vec<String> = Vec::new();
        // Where its own stamps are among what is kept: renewed if it changes.
        let mut stamps: Vec<usize> = Vec::new();
        for (i, line) in block.iter().enumerate() {
            if i == 0 || i + 1 == block.len() {
                kept.push(line.clone());
                continue;
            }
            let name = lines::name(line);
            match name.as_str() {
                "BEGIN" => nested += 1,
                "END" => nested = nested.saturating_sub(1),
                "DTSTAMP" | "LAST-MODIFIED" if nested == 0 => stamps.push(kept.len()),
                "ORGANIZER" if nested == 0 => {
                    has_organizer = true;
                    if !names_organizer(line, organizer) {
                        kept.push(person_line("ORGANIZER", organizer, ""));
                        changed = true;
                        continue;
                    }
                }
                "ATTENDEE" if nested == 0 => {
                    let who = address_of(lines::value(line));
                    if organizer.is(&who) {
                        kept.push(line.clone());
                        continue;
                    }
                    // The series: as the form lists them; a time changed: its own, less those taken off.
                    let gone = if one_time { taken_off.iter().any(|a| **a == who) } else { !wanted.iter().any(|g| g.is(&who)) };
                    if gone || present.contains(&who) {
                        changed = true;
                        continue;
                    }
                    present.push(who);
                }
                _ => {}
            }
            kept.push(line.clone());
        }
        // Before its alarms, else before its end.
        let mut added: Vec<String> = Vec::new();
        let marks = added_marks;
        if !has_organizer {
            added.push(person_line("ORGANIZER", organizer, ""));
        }
        let new_here: &[&Guest] = if one_time { &added_to } else { &wanted };
        added.extend(new_here.iter().filter(|g| !present.contains(&g.address.trim().to_lowercase())).map(|g| guest_line(g, marks)));
        changed |= !added.is_empty();
        if changed {
            for at in stamps.into_iter().rev() {
                kept.remove(at);
            }
            added.extend([format!("DTSTAMP:{now}"), format!("LAST-MODIFIED:{now}")]);
        }
        let place = first_nested(&kept);
        kept.splice(place..place, added);
        out.extend(kept);
    }
    out.extend(source[at..].iter().cloned());
    let written = lines::fold(&out);
    Some(if lines::unfold(&written) == source { text.to_string() } else { written })
}

/// Where a VEVENT block's alarms begin, else its END: where its own lines go.
fn first_nested(block: &[String]) -> usize {
    block.iter().enumerate().skip(1).find(|(_, l)| lines::name(l) == "BEGIN").map_or(block.len().saturating_sub(1), |(i, _)| i)
}

/// What a meeting's guests may read of each of its events in the messages
/// Sioul drafts for them, by property name as written (`lines::raw_name`,
/// so that none of Sioul's own names reads as one of these): RFC 5546's
/// REQUEST and CANCEL properties (§3.2.2, §3.2.5) that say the meeting,
/// never ATTACH (files), CATEGORIES (your own filing), CONTACT nor
/// RELATED-TO (your ties), nor any `X-` line; LINK, REFID and CONCEPT are no
/// iTIP properties. An allow-list: whatever another program adds stays out of
/// the drafts too (review of 1e8551e, F4); a `sioul:` address never goes
/// (`without_sioul_links`).
pub const SHARED: [&str; 26] = [
    "UID", "RECURRENCE-ID", "SEQUENCE", "DTSTAMP", "CREATED", "LAST-MODIFIED", "DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "DESCRIPTION", "LOCATION", "GEO", "STATUS", "CLASS",
    "TRANSP", "PRIORITY", "URL", "COMMENT", "RESOURCES", "REQUEST-STATUS", "ORGANIZER", "ATTENDEE",
];

/// Whether a line of an event's own level is one its guests may read in a draft (`SHARED`).
fn shared(line: &str) -> bool {
    SHARED.contains(&lines::raw_name(line).as_str())
}

/// The text properties whose `sioul:` addresses are taken out, the rest of
/// their text kept (a child's stand-in's details); every other line holding
/// one goes whole.
const TEXTS: [&str; 4] = ["SUMMARY", "LOCATION", "DESCRIPTION", "COMMENT"];

/// Whether a line holds one of Sioul's own addresses (`sioul:…`), in its
/// value or in a parameter.
fn carries_sioul_link(line: &str) -> bool {
    line.to_ascii_lowercase().contains("sioul:")
}

/// A line as its guests may read it as far as Sioul's own addresses go
/// (second review of the organizer's door, N6: a child's stand-in names its
/// child's event, `sioul:event/…`, in its URL and its details): a text line
/// (`TEXTS`) with each word holding a `sioul:` address taken out of its
/// value, and each parameter holding one; none when nothing is left of it,
/// and none for any other line holding one. The line as it is when it holds none.
fn without_sioul_links(line: &str) -> Option<String> {
    if !carries_sioul_link(line) {
        return Some(line.to_string());
    }
    if !TEXTS.contains(&lines::raw_name(line).as_str()) {
        return None;
    }
    let (head, value) = split(line);
    let parts = written_params(head);
    let mut kept_head = parts.first().copied().unwrap_or_default().to_string();
    for part in parts.iter().skip(1).filter(|p| !carries_sioul_link(p)) {
        kept_head.push(';');
        kept_head.push_str(part);
    }
    let text = lines::unescape(value);
    let mut rows: Vec<String> = text
        .split('\n')
        .filter_map(|row| {
            if !carries_sioul_link(row) {
                return Some(row.to_string());
            }
            let left = row.split_whitespace().filter(|w| !carries_sioul_link(w)).collect::<Vec<_>>().join(" ");
            (!left.trim().is_empty()).then_some(left)
        })
        .collect();
    while rows.last().is_some_and(|r| r.trim().is_empty()) {
        rows.pop();
    }
    let kept = rows.join("\n");
    (!kept.trim().is_empty()).then(|| format!("{kept_head}:{}", lines::escape(&kept)))
}

/// Whether a line of an event's own level is Sioul's own, never kept in a
/// meeting's file once it names guests: Sioul's own lines (`X-SIOUL-…`: the
/// time kept around it, its costs and gain, your reminder, who takes a
/// child's event and its kind, a block's task…) and its ties under their
/// standard names too (LINK, REFID, CONCEPT, RFC 9253's, which Sioul wrote
/// before 27 October 2026: `lines::OWN_NAMES`).
pub fn sioul_own(line: &str) -> bool {
    lines::raw_name(line).starts_with("X-SIOUL-") || ["LINK", "REFID", "CONCEPT"].contains(&lines::name(line).as_str())
}

/// Whether a component nested in an event (an alarm) is Sioul's own: it
/// holds one of Sioul's lines (`X-SIOUL-…`; a time block's alarm says
/// `blocks::ALARM_MARK`) or one of its addresses. Another program's alarm
/// holds neither, and stays.
fn sioul_component(block: &[String]) -> bool {
    block.iter().any(|l| lines::raw_name(l).starts_with("X-SIOUL-") || carries_sioul_link(l))
}

/// A meeting's file once it names guests, as they may read it (review of
/// 1e8551e, F5, and its second review, N6): out of each event, Sioul's own
/// lines and ties (`sioul_own`), Sioul's own alarms (`sioul_component`) and
/// every `sioul:` address (`without_sioul_links`), which its server would
/// send them; every other program's lines kept as they are, the calendar's
/// and each event's: their alarms, attachments, categories, colour, video
/// link and the rest. At the calendar's own level, no METHOD (RFC 4791
/// §4.1) and no `X-SIOUL-…` line. The same text when nothing goes.
pub fn for_guests(text: &str) -> String {
    let source = lines::unfold(text);
    let mut out: Vec<String> = Vec::with_capacity(source.len());
    // Where each line is: the calendar's own level, a component and how deep in it; an
    // event's nested component (an alarm) held until its END says whether it is Sioul's.
    let (mut component, mut depth): (Option<String>, usize) = (None, 0);
    let mut nested: Vec<String> = Vec::new();
    for line in &source {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        let event = component.as_deref() == Some("VEVENT");
        match (&component, name.as_str()) {
            (None, "BEGIN") if value != "VCALENDAR" => {
                component = Some(value);
                depth = 0;
                out.push(line.clone());
            }
            (None, "BEGIN" | "END") => out.push(line.clone()),
            (None, _) => {
                if name != "METHOD" && !lines::raw_name(line).starts_with("X-SIOUL-") {
                    out.push(line.clone());
                }
            }
            (Some(_), "BEGIN") if event => {
                depth += 1;
                nested.push(line.clone());
            }
            (Some(_), "END") if event && depth > 0 => {
                depth -= 1;
                nested.push(line.clone());
                if depth == 0 {
                    if !sioul_component(&nested) {
                        out.append(&mut nested);
                    }
                    nested.clear();
                }
            }
            (Some(_), _) if event && depth > 0 => nested.push(line.clone()),
            (Some(_), "BEGIN") => {
                depth += 1;
                out.push(line.clone());
            }
            (Some(_), "END") if depth > 0 => {
                depth -= 1;
                out.push(line.clone());
            }
            (Some(_), "END") => {
                component = None;
                out.push(line.clone());
            }
            (Some(_), _) if event => {
                if !sioul_own(line)
                    && let Some(kept) = without_sioul_links(line)
                {
                    out.push(kept);
                }
            }
            (Some(_), _) => out.push(line.clone()),
        }
    }
    if out == source { text.to_string() } else { lines::fold(&out) }
}

/// What a meeting's file held that is yours alone (`set_aside`): Sioul's own
/// lines of its main event (the time kept around it, its costs and gain,
/// your reminder), its ties, each as your own store of ties keeps one
/// (`links::LocalLinks`, `from` the event), and Sioul's other lines of each
/// of its events, for their own store (`meeting_lines`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Aside {
    /// None when the file held none of those lines.
    pub own: Option<crate::own_time::Own>,
    pub ties: Vec<crate::links::Edge>,
    /// Sioul's other lines (who takes a child's event and its kind, a
    /// block's task, one time's own margins…), each with the time it is of:
    /// "" the series, else that time's RECURRENCE-ID line (second review of
    /// the organizer's door, N6).
    pub lines: Vec<(String, String)>,
}

impl Aside {
    pub fn is_empty(&self) -> bool {
        self.own.is_none() && self.ties.is_empty() && self.lines.is_empty()
    }
}

/// What a meeting's file holds of yours (`Aside`): what the door moves into
/// your own files when it writes the meeting as its guests may read it
/// (`for_guests`). Of its main event, the time kept around it, its costs,
/// gain and your reminder (`own_time`) and its ties (`links`; a tie a task
/// holds already, the task that prepares it, `pairs`, is the task's); every
/// other line of Sioul's of each event (`sioul_own`), and every other line
/// that goes for one of Sioul's addresses it holds (`without_sioul_links`:
/// a stand-in's URL), as it is.
pub fn aside_of(text: &str) -> Aside {
    let source = lines::unfold(text);
    let mut aside = Aside::default();
    let mut own_lines = false;
    let mut main_seen = false;
    for range in events(&source) {
        let block = &source[range];
        let time = block.iter().find(|l| lines::name(l) == "RECURRENCE-ID").cloned().unwrap_or_default();
        let main = time.is_empty() && !main_seen;
        main_seen |= main;
        let uid = block.iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()).unwrap_or_default();
        let from = crate::links::event_uri(&uid);
        let mut nested = 0usize;
        for line in &block[1..block.len().saturating_sub(1)] {
            match lines::name(line).as_str() {
                "BEGIN" => nested += 1,
                "END" => nested = nested.saturating_sub(1),
                _ if nested > 0 => {}
                // Another line naming one of Sioul's addresses (a contact by its address): kept as it is.
                _ if !sioul_own(line) => {
                    if without_sioul_links(line).is_none() {
                        aside.lines.push((time.clone(), line.clone()));
                    }
                }
                "LINK" if main => {
                    let link = crate::tasks::link_of(line);
                    if link.rel == crate::pairs::PREPARATION || link.uri.trim().is_empty() {
                        continue;
                    }
                    let how = match link.rel.as_str() {
                        "describedby" => "note",
                        "via" => "source",
                        _ => "link",
                    };
                    aside.ties.push(crate::links::Edge { from: from.clone(), to: link.uri, how: how.into() });
                }
                "RELATED-TO" if main => {
                    let other = lines::value(line).trim();
                    if !other.is_empty() {
                        aside.ties.push(crate::links::Edge { from: from.clone(), to: format!("uid:{other}"), how: "link".into() });
                    }
                }
                "REFID" if main => {
                    let project = lines::unescape(lines::value(line).trim());
                    if !project.is_empty() {
                        aside.ties.push(crate::links::Edge { from: from.clone(), to: crate::links::project_uri(&project), how: "project".into() });
                    }
                }
                name if main && [crate::demands::BEFORE, crate::demands::AFTER, crate::demands::COST, crate::demands::GAIN, crate::reminders::REMIND].contains(&name) => own_lines = true,
                _ => aside.lines.push((time.clone(), line.clone())),
            }
        }
    }
    if own_lines && let Some(edit) = crate::agenda::edit_of_text(text, &crate::zone::current()) {
        aside.own = Some(crate::own_time::Own { before: edit.margins.before, after: edit.margins.after, demands: edit.demands, remind: edit.remind });
    }
    aside
}

/// A meeting's file as its guests may read it (`for_guests`), and what it
/// held of yours (`aside_of`), for your own files.
pub fn set_aside(text: &str) -> (String, Aside) {
    (for_guests(text), aside_of(text))
}

/// What the guests see of an event: each VEVENT's times, rule, title,
/// place, notes and status, in order; the events naming `guest` alone when
/// one is given (a guest asked to some times of it alone). A title, a place
/// or notes as a form gives them back (`agenda::normalised`: no-break
/// spaces, line ends, trailing spaces) and without Sioul's own addresses
/// (`without_sioul_links`), which its guests never read: a save that changes
/// none of these tells nobody (second review of the organizer's door, N2).
fn seen_by_guests(text: &str, guest: Option<&str>) -> Vec<String> {
    const SEEN: [&str; 12] = ["UID", "RECURRENCE-ID", "DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "LOCATION", "DESCRIPTION", "STATUS"];
    let source = lines::unfold(text);
    let mut out = Vec::new();
    for range in events(&source) {
        if guest.is_some_and(|g| !names_guest(&source[range.clone()], g)) {
            continue;
        }
        let mut nested = 0usize;
        out.push("BEGIN:VEVENT".to_string());
        for line in &source[range.start + 1..range.end - 1] {
            match lines::name(line).as_str() {
                "BEGIN" => nested += 1,
                "END" => nested = nested.saturating_sub(1),
                name if nested == 0 && TEXTS.contains(&name) && SEEN.contains(&name) => {
                    if let Some(kept) = without_sioul_links(line) {
                        out.push(format!("{name}:{}", crate::agenda::normalised(&lines::unescape(lines::value(&kept)))));
                    }
                }
                name if nested == 0 && SEEN.contains(&name) => out.push(line.clone()),
                _ => {}
            }
        }
    }
    out
}

/// Whether the guests must hear of a change from `before` to `after`: its
/// times, how it repeats, its title, place, notes or status. Sioul's own
/// lines, your reminder and the stamps are yours alone.
pub fn told(before: &str, after: &str) -> bool {
    seen_by_guests(before, None) != seen_by_guests(after, None)
}

/// Whether `guest` must hear of a change from `before` to `after`: the
/// events naming them changed, or they were asked to more times or fewer
/// (a guest asked to some times of a series alone, `one_time_guests`).
pub fn told_guest(before: &str, after: &str, guest: &str) -> bool {
    seen_by_guests(before, Some(guest)) != seen_by_guests(after, Some(guest))
}

/// What a scheduling server sends a change of (sabre/vobject's ITip Broker,
/// `significantChangeProperties`, read 6 November 2026): each VEVENT's times,
/// rule, dates added or left out, and status; a title, a place or notes
/// changed alone are not sent (Nextcloud's iMIP plugin mails only a
/// significant change).
const SIGNIFICANT: [&str; 10] = ["UID", "RECURRENCE-ID", "DTSTART", "DTEND", "DURATION", "DUE", "RRULE", "RDATE", "EXDATE", "STATUS"];

/// Whether a server that schedules sends the change from `before` to `after`
/// to the guests it tells (`SIGNIFICANT`); a guest added or taken off is
/// sent their invitation or cancellation whatever this says.
pub fn significant(before: &str, after: &str) -> bool {
    let of = |text: &str| {
        let source = lines::unfold(text);
        let mut out = Vec::new();
        for range in events(&source) {
            let mut nested = 0usize;
            out.push("BEGIN:VEVENT".to_string());
            for line in &source[range.start + 1..range.end - 1] {
                match lines::name(line).as_str() {
                    "BEGIN" => nested += 1,
                    "END" => nested = nested.saturating_sub(1),
                    name if nested == 0 && SIGNIFICANT.contains(&name) => out.push(line.clone()),
                    _ => {}
                }
            }
        }
        out
    };
    of(before) != of(after)
}

/// A meeting with no `SCHEDULE-FORCE-SEND` left on any line: one asked for a
/// write before (`with_force_send`) is never asked again by the next. The
/// same text when there is none.
pub fn without_force_send(text: &str) -> String {
    let source = lines::unfold(text);
    let out: Vec<String> = source.iter().map(|line| if matches!(lines::name(line).as_str(), "ATTENDEE" | "ORGANIZER") && lines::param(line, "SCHEDULE-FORCE-SEND").is_some() { with_params(line, &["SCHEDULE-FORCE-SEND"], &[]) } else { line.clone() }).collect();
    if out == source { text.to_string() } else { lines::fold(&out) }
}

/// The guests a meeting asks its server to tell of its last change
/// (`SCHEDULE-FORCE-SEND` on their lines of its first VEVENT, where sabre
/// reads and takes it out: `with_force_send`), by address in lower case:
/// asked by a write its server has not had yet.
pub fn forced_of(text: &str) -> Vec<String> {
    let source = lines::unfold(text);
    let Some(first) = events(&source).into_iter().next() else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for line in &source[first] {
        if lines::name(line) == "ATTENDEE" && lines::param(line, "SCHEDULE-FORCE-SEND").is_some() {
            let who = address_of(lines::value(line));
            if !out.contains(&who) {
                out.push(who);
            }
        }
    }
    out
}

/// A meeting with `SCHEDULE-FORCE-SEND=REQUEST` on the lines of the guests
/// `to` (addresses) in its first VEVENT (RFC 6638 §7.3): its server sends
/// them this write's change, which it would not send by itself
/// (`significant`), then takes the parameter out of the copy it keeps
/// (sabre/dav's Schedule plugin, `processICalendarChange`, on the first
/// VEVENT's guests, which is where sabre's broker reads it; read 6 November
/// 2026). Those of `to` it names there, by address in lower case.
pub fn with_force_send(text: &str, to: &[String]) -> (String, Vec<String>) {
    let mut source = lines::unfold(text);
    let Some(first) = events(&source).into_iter().next() else { return (text.to_string(), Vec::new()) };
    let mut forced = Vec::new();
    let mut nested = 0usize;
    for i in first.start + 1..first.end - 1 {
        match lines::name(&source[i]).as_str() {
            "BEGIN" => nested += 1,
            "END" => nested = nested.saturating_sub(1),
            "ATTENDEE" if nested == 0 => {
                let who = address_of(lines::value(&source[i]));
                if to.iter().any(|t| t.trim().eq_ignore_ascii_case(&who)) && !forced.contains(&who) {
                    source[i] = with_params(&source[i], &["SCHEDULE-FORCE-SEND"], &[("SCHEDULE-FORCE-SEND", "REQUEST")]);
                    forced.push(who);
                }
            }
            _ => {}
        }
    }
    if forced.is_empty() { (text.to_string(), forced) } else { (lines::fold(&source), forced) }
}

/// The VTIMEZONEs of a calendar's text, whole.
fn zones(source: &[String]) -> Vec<String> {
    let (mut out, mut inside) = (Vec::new(), false);
    for line in source {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        if name == "BEGIN" && value == "VTIMEZONE" {
            inside = true;
        }
        if inside {
            out.push(line.clone());
        }
        if name == "END" && value == "VTIMEZONE" {
            inside = false;
        }
    }
    out
}

/// One VEVENT as the guests receive it: its lines they may read alone
/// (`SHARED`, an allow-list), with no alarm (yours), its people's lines
/// without what only a server reads (RFC 6638's SCHEDULE-AGENT,
/// SCHEDULE-STATUS, SCHEDULE-FORCE-SEND), never one of Sioul's own
/// addresses (`without_sioul_links`: a child's stand-in's URL and details).
fn as_sent(block: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(block.len());
    let mut nested = 0usize;
    for (i, line) in block.iter().enumerate() {
        if i == 0 || i + 1 == block.len() {
            out.push(line.clone());
            continue;
        }
        let name = lines::name(line);
        match name.as_str() {
            "BEGIN" => {
                nested += 1;
                continue;
            }
            "END" => {
                nested = nested.saturating_sub(1);
                continue;
            }
            _ if nested > 0 => continue,
            _ if !shared(line) => continue,
            "ATTENDEE" | "ORGANIZER" => out.push(with_params(line, &["SCHEDULE-AGENT", "SCHEDULE-STATUS", "SCHEDULE-FORCE-SEND"], &[])),
            _ => out.extend(without_sioul_links(line)),
        }
    }
    out
}

/// The invitation to a meeting you organize (an iTIP REQUEST, RFC 5546
/// §3.2.2), made from its file: every VEVENT of it as the guests receive
/// it (`as_sent`), its zones, every guest named, so that each knows who
/// else is asked. Its SEQUENCE is the file's: an update says a later one.
/// None for no event.
pub fn request(text: &str) -> Option<String> {
    let source = lines::unfold(text);
    let ranges = events(&source);
    if ranges.is_empty() {
        return None;
    }
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "METHOD:REQUEST".to_string()];
    out.extend(zones(&source));
    for range in ranges {
        out.extend(as_sent(&source[range]));
    }
    out.push("END:VCALENDAR".to_string());
    Some(lines::fold(&out))
}

/// The cancellation of a meeting you organize (an iTIP CANCEL, RFC 5546
/// §3.2.5): its main VEVENT's UID, times, rule, title and place, its
/// organizer, the guests in `to` (all of them when empty), and
/// STATUS:CANCELLED; `whole`, the meeting cancelled, its SEQUENCE one more
/// than the file's, else those guests taken off it, at the file's SEQUENCE.
/// None for no event.
pub fn cancel(text: &str, to: &[String], whole: bool) -> Option<String> {
    const KEPT: [&str; 9] = ["UID", "DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "LOCATION"];
    let source = lines::unfold(text);
    let ranges = events(&source);
    let main = ranges.iter().find(|r| !source[(*r).clone()].iter().any(|l| lines::name(l) == "RECURRENCE-ID")).or(ranges.first())?.clone();
    let block = as_sent(&source[main]);
    let sequence: i64 = block.iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse().ok()).unwrap_or(0);
    let every = to.is_empty();
    let now = crate::moves::utc(crate::zone::now().timestamp().as_second());
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "METHOD:CANCEL".to_string()];
    out.extend(zones(&source));
    out.push("BEGIN:VEVENT".to_string());
    for line in &block[1..block.len() - 1] {
        let name = lines::name(line);
        if KEPT.contains(&name.as_str()) || name == "ORGANIZER" {
            out.push(line.clone());
        } else if name == "ATTENDEE" && (every || to.iter().any(|t| t.trim().eq_ignore_ascii_case(&address_of(lines::value(line))))) {
            out.push(with_params(line, &["PARTSTAT", "RSVP"], &[]));
        }
    }
    // A guest taken off the list: named, though the file no longer names them.
    for gone in to {
        if !out.iter().any(|l| lines::name(l) == "ATTENDEE" && address_of(lines::value(l)) == gone.trim().to_lowercase()) {
            out.push(format!("ATTENDEE:mailto:{}", gone.trim()));
        }
    }
    out.extend([format!("SEQUENCE:{}", if whole { sequence + 1 } else { sequence }), format!("DTSTAMP:{now}"), "STATUS:CANCELLED".to_string(), "END:VEVENT".to_string(), "END:VCALENDAR".to_string()]);
    Some(lines::fold(&out))
}

/// A guest's answer to a meeting you organize, read from their REPLY
/// (`replied`): the meeting with it written in (`written`; none when the
/// meeting says it already, or when that time's answer is its server's to
/// keep), the guest, their answer ("accepted", "declined", "tentative") and,
/// for one time of a series, that time's RECURRENCE-ID line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub written: Option<String>,
    pub guest: Guest,
    pub answer: String,
    pub time: Option<String>,
    /// That time's start, Unix seconds.
    pub at: Option<i64>,
}

/// Whether every guest of a meeting is left to you to tell
/// (`SCHEDULE-AGENT=CLIENT` on each ATTENDEE line but your own, RFC 6638
/// §7.1): no server schedules a change to it, nor sends anything of it.
pub fn left_to_you(text: &str) -> bool {
    let organizer = organizer_of(text).map(|o| o.address.to_lowercase()).unwrap_or_default();
    let (mut inside, mut nested, mut any) = (false, 0usize, false);
    for line in lines::unfold(text) {
        match lines::name(&line).as_str() {
            "BEGIN" if !inside => inside = lines::value(&line).trim().eq_ignore_ascii_case("VEVENT"),
            "BEGIN" => nested += 1,
            "END" if inside && nested > 0 => nested -= 1,
            "END" if inside => inside = false,
            "ATTENDEE" if inside && nested == 0 => {
                if address_of(lines::value(&line)) == organizer {
                    continue;
                }
                any = true;
                if !lines::param(&line, "SCHEDULE-AGENT").is_some_and(|a| a.eq_ignore_ascii_case("CLIENT")) {
                    return false;
                }
            }
            _ => {}
        }
    }
    any
}

/// A guest's answer to a meeting you organize, from their REPLY (RFC 5546
/// §3.2.3): that guest's PARTSTAT set (and their RSVP done, as a server sets
/// it), in the event of the time it names (RECURRENCE-ID, matched by its
/// instant) or its series, and nothing else. A time of a series with no event
/// of its own yet gets one, made from the series at its own times
/// (`agenda::time_made_own`), when no server schedules the meeting
/// (`left_to_you`): one made where a server schedules it would have the
/// server send every guest that time again; there, the answer is said and
/// left to the server (review of 1e8551e, F13). None when the REPLY answers
/// another event or no time of it, names no guest of it (nobody is added by
/// an answer), or is older than the event (a lower SEQUENCE: it answers an
/// earlier version).
pub fn replied(text: &str, reply: &str) -> Option<Reply> {
    let theirs = lines::unfold(reply);
    if !theirs.iter().any(|l| lines::name(l) == "METHOD" && lines::value(l).trim().eq_ignore_ascii_case("REPLY")) {
        return None;
    }
    let range = events(&theirs).into_iter().next()?;
    let event = &theirs[range];
    let find = |name: &str| event.iter().find(|l| lines::name(l) == name).map(|l| lines::value(l).trim().to_string());
    let uid = find("UID")?;
    let time = event.iter().find(|l| lines::name(l) == "RECURRENCE-ID").cloned();
    let sequence: i64 = find("SEQUENCE").and_then(|s| s.parse().ok()).unwrap_or(0);
    let attendee = event.iter().find(|l| lines::name(l) == "ATTENDEE")?;
    let who = address_of(lines::value(attendee));
    let answer = lines::param(attendee, "PARTSTAT")?.to_ascii_uppercase();
    if !["ACCEPTED", "DECLINED", "TENTATIVE"].contains(&answer.as_str()) {
        return None;
    }
    let zone = crate::zone::current();
    let instant = |line: &str| crate::agenda::instant_in(line, &zone);
    let target_of = |source: &[String]| {
        events(source).into_iter().find(|r| {
            let block = &source[r.clone()];
            let own_uid = block.iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string());
            let own_time = block.iter().find(|l| lines::name(l) == "RECURRENCE-ID");
            own_uid.as_deref() == Some(uid.as_str())
                && match (&time, own_time) {
                    (None, None) => true,
                    (Some(asked), Some(own)) => instant(asked).is_some() && instant(asked) == instant(own),
                    _ => false,
                }
        })
    };
    let mut source = lines::unfold(text);
    let guest_of = |source: &[String], target: &std::ops::Range<usize>| -> Option<(usize, bool)> {
        let mut nested = 0usize;
        for i in target.start + 1..target.end - 1 {
            match lines::name(&source[i]).as_str() {
                "BEGIN" => nested += 1,
                "END" => nested = nested.saturating_sub(1),
                "ATTENDEE" if nested == 0 && address_of(lines::value(&source[i])) == who => return Some((i, lines::param(&source[i], "PARTSTAT").is_some_and(|p| p.eq_ignore_ascii_case(&answer)))),
                _ => {}
            }
        }
        None
    };
    let said = |written: Option<String>, line: Option<&String>| {
        let name = line.and_then(|l| lines::param(l, "CN")).unwrap_or_default();
        Some(Reply { written, guest: Guest { address: who.clone(), name }, answer: answer.to_ascii_lowercase(), time: time.clone(), at: time.as_deref().and_then(instant) })
    };
    let target = match target_of(&source) {
        Some(target) => target,
        // One time of a series with no event of its own: made, where no server schedules it.
        None => {
            let asked = time.as_deref()?;
            let master = events(&source).into_iter().find(|r| !source[r.clone()].iter().any(|l| lines::name(l) == "RECURRENCE-ID"))?;
            let ours: i64 = source[master.clone()].iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse().ok()).unwrap_or(0);
            if sequence < ours {
                return None;
            }
            let made = crate::agenda::time_made_own(text, asked, &zone)?;
            if !left_to_you(text) {
                let line = guest_of(&source, &master).map(|(i, _)| source[i].clone());
                return said(None, line.as_ref());
            }
            source = lines::unfold(&made);
            target_of(&source)?
        }
    };
    let ours: i64 = source[target.clone()].iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse().ok()).unwrap_or(0);
    if sequence < ours {
        return None;
    }
    // Asked to the series, taken off that time by another program: no answer of theirs for it.
    let (i, same) = guest_of(&source, &target)?;
    if same {
        return said(None, Some(&source[i]));
    }
    source[i] = with_params(&source[i], &["PARTSTAT", "RSVP"], &[("PARTSTAT", &answer)]);
    let line = source[i].clone();
    said(Some(lines::fold(&source)), Some(&line))
}

/// A meeting's text with each event's SEQUENCE set by `next` (given its
/// own, 0 when unsaid) and its stamps renewed. None for no event.
fn resequenced(text: &str, next: impl Fn(i64) -> i64) -> Option<String> {
    let source = lines::unfold(text);
    let ranges = events(&source);
    if ranges.is_empty() {
        return None;
    }
    let now = crate::moves::utc(crate::zone::now().timestamp().as_second());
    let mut out: Vec<String> = Vec::with_capacity(source.len() + 3);
    let mut at = 0;
    for range in ranges {
        out.extend(source[at..range.start].iter().cloned());
        at = range.end;
        let block = &source[range];
        let sequence: i64 = block.iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse().ok()).unwrap_or(0);
        let mut nested = 0usize;
        let mut kept: Vec<String> = Vec::with_capacity(block.len() + 3);
        for (i, line) in block.iter().enumerate() {
            if i > 0 && i + 1 < block.len() {
                match lines::name(line).as_str() {
                    "BEGIN" => nested += 1,
                    "END" => nested = nested.saturating_sub(1),
                    "SEQUENCE" | "DTSTAMP" | "LAST-MODIFIED" if nested == 0 => continue,
                    _ => {}
                }
            }
            kept.push(line.clone());
        }
        let place = first_nested(&kept);
        kept.splice(place..place, [format!("SEQUENCE:{}", next(sequence)), format!("DTSTAMP:{now}"), format!("LAST-MODIFIED:{now}")]);
        out.extend(kept);
    }
    out.extend(source[at..].iter().cloned());
    Some(lines::fold(&out))
}

/// A meeting you organize as it comes back after its cancellation went to
/// its guests (Undo of its deletion, review of 1e8551e, F14): each event's
/// SEQUENCE two past its own, so one past the cancellation's
/// (`cancel`), which guests' programs take as a later version; its stamps
/// renewed. None for no event.
pub fn after_cancel(text: &str) -> Option<String> {
    resequenced(text, |own| own + 2)
}

/// The SEQUENCE of a meeting's main event (the series, else its first event); 0 unsaid.
pub fn sequence_of(text: &str) -> i64 {
    let source = lines::unfold(text);
    let ranges = events(&source);
    let main = ranges.iter().find(|r| !source[(*r).clone()].iter().any(|l| lines::name(l) == "RECURRENCE-ID")).or(ranges.first());
    main.and_then(|r| source[r.clone()].iter().find(|l| lines::name(l) == "SEQUENCE")).and_then(|l| lines::value(l).trim().parse().ok()).unwrap_or(0)
}

/// A meeting's earlier version put back once its guests heard of the change
/// made over it (Undo, second review of the organizer's door, N7): each
/// event's SEQUENCE one past the highest of `heard`, the version they
/// received, so that their programs take it as a later one (RFC 5546
/// §2.1.5), never as an older; its stamps renewed. None for no event.
pub fn past(text: &str, heard: &str) -> Option<String> {
    let source = lines::unfold(heard);
    let top = events(&source).into_iter().filter_map(|r| source[r].iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse::<i64>().ok())).max().unwrap_or(0);
    resequenced(text, |own| own.max(top) + 1)
}

/// An unsent message to a meeting's guests waiting in your Drafts (`reconcile`): its id, to whom, its iTIP object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub id: String,
    /// Its guests' addresses.
    pub to: Vec<String>,
    pub ics: String,
}

/// What becomes of a meeting's unsent messages once its server's version
/// was kept over yours (`reconcile`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reconciled {
    /// Still what their guests need to hear: kept, by id.
    pub keep: Vec<String>,
    /// Saying a version the calendar no longer holds: withdrawn, by id.
    pub withdraw: Vec<String>,
    /// Those who still need to hear the server's version, and whose message
    /// went: an invitation of that version drafted for them.
    pub invite: Vec<Guest>,
    /// Its guests its server tells (not left to you): they heard nothing of
    /// your change, which the server never had.
    pub served: Vec<Guest>,
}

/// A meeting you organize whose change here was set aside by the sync for
/// its server's version, `theirs` (second review of the organizer's door,
/// N3): what each guest still needs to hear, worked out from `theirs`, and
/// so which of its unsent messages (`pending`, its REQUESTs and CANCELs in
/// your Drafts) stay, which go, and to whom an invitation of `theirs` is
/// drafted. `server_tells`: its server tells the guests not left to you.
///
/// - An invitation stays when every guest it goes to is told by your drafts
///   in `theirs` and it says `theirs` (the times, title, place, notes and
///   status of the events naming them, and its SEQUENCE); a cancellation
///   stays when its guests are no longer asked in `theirs`, or `theirs` is
///   cancelled. Every other one goes.
/// - A guest told by your drafts in `theirs` whose message went, and whom
///   no message kept tells, gets an invitation of `theirs` when they have
///   not heard it: an invitation that went was for a version at least as
///   recent as theirs (`SEQUENCE`), or a cancellation for an earlier one.
///   One of theirs older than what went they heard already: nothing more.
pub fn reconcile(theirs: &str, pending: &[Pending], server_tells: bool) -> Reconciled {
    let server = if server_tells { Sender::Server } else { Sender::Draft };
    let guests = every_guest(theirs);
    let drafted: Vec<&Guest> = guests.iter().filter(|g| teller(theirs, &g.address, server) == Sender::Draft).collect();
    let is_drafted = |address: &str| drafted.iter().any(|g| g.is(address));
    let asked = |address: &str| guests.iter().any(|g| g.is(address));
    let cancelled = {
        let source = lines::unfold(theirs);
        let ranges = events(&source);
        let main = ranges.iter().find(|r| !source[(*r).clone()].iter().any(|l| lines::name(l) == "RECURRENCE-ID")).or(ranges.first()).cloned();
        main.is_some_and(|r| source[r].iter().any(|l| lines::name(l) == "STATUS" && lines::value(l).trim().eq_ignore_ascii_case("CANCELLED")))
    };
    let ours = sequence_of(theirs);
    let method = |ics: &str| lines::unfold(ics).iter().find(|l| lines::name(l) == "METHOD").map(|l| lines::value(l).trim().to_ascii_uppercase()).unwrap_or_default();
    let mut out = Reconciled::default();
    let mut gone: Vec<&Pending> = Vec::new();
    for p in pending {
        let valid = match method(&p.ics).as_str() {
            "REQUEST" => !cancelled && sequence_of(&p.ics) == ours && p.to.iter().all(|r| is_drafted(r) && seen_by_guests(&p.ics, Some(r)) == seen_by_guests(theirs, Some(r))),
            "CANCEL" => cancelled || p.to.iter().all(|r| !asked(r)),
            _ => true,
        };
        if valid {
            out.keep.push(p.id.clone());
        } else {
            out.withdraw.push(p.id.clone());
            gone.push(p);
        }
    }
    let kept_request = |address: &str| pending.iter().any(|p| out.keep.contains(&p.id) && method(&p.ics) == "REQUEST" && p.to.iter().any(|r| r.trim().eq_ignore_ascii_case(address)));
    for guest in drafted.iter().filter(|_| !cancelled) {
        let unheard = gone.iter().filter(|p| p.to.iter().any(|r| guest.is(r))).any(|p| match method(&p.ics).as_str() {
            "REQUEST" => ours >= sequence_of(&p.ics),
            "CANCEL" => ours > sequence_of(&p.ics),
            _ => false,
        });
        if unheard && !kept_request(&guest.address) {
            out.invite.push((*guest).clone());
        }
    }
    if server_tells {
        out.served = guests.iter().filter(|g| !is_drafted(&g.address)).cloned().collect();
    }
    out
}

/// A sentence of the translator `tr` with its arguments.
fn say(tr: &crate::i18n::Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = crate::i18n::args();
    for (key, value) in pairs {
        args.set(key.to_string(), value.clone());
    }
    tr.text(id, Some(&args))
}

/// A meeting's title and its first time, as a message says them:
/// ("Planning", "Tuesday 10 November at 10:00"); a whole day's day alone.
pub fn named(text: &str, tr: &crate::i18n::Translator) -> (String, String) {
    let zone = crate::zone::current();
    let source = lines::unfold(text);
    let title = source.iter().find(|l| lines::name(l) == "SUMMARY").map(|l| lines::unescape(lines::value(l).trim())).filter(|t| !t.is_empty()).unwrap_or_else(|| tr.text("agenda-untitled", None));
    let when = crate::agenda::first_times(text, &zone)
        .and_then(|(start, _)| jiff::Timestamp::from_second(start).ok())
        .map(|t| t.to_zoned(zone.clone()))
        .map(|start| if text.contains("DTSTART;VALUE=DATE:") { tr.day(start.date()) } else { tr.date(&start, true) })
        .unwrap_or_default();
    (title, when)
}

/// A draft to `to`, guests of the meeting `text`, carrying `ics` (iMIP, RFC
/// 6047), from `account` with its `signature`: an invitation or an update,
/// or a cancellation (`cancel`), its words in `tr`'s language; tied to the
/// meeting (`links::event_uri`).
pub fn draft(account: &str, signature: Option<&str>, to: &[Guest], text: &str, ics: String, cancel: bool, tr: &crate::i18n::Translator) -> crate::compose::Draft {
    let (title, when) = named(text, tr);
    let place = lines::unfold(text).iter().find(|l| lines::name(l) == "LOCATION").map(|l| lines::unescape(lines::value(l).trim())).unwrap_or_default();
    let mut draft = crate::compose::Draft::new(account);
    draft.to = to.iter().map(|g| if g.name.trim().is_empty() { g.address.clone() } else { format!("{} <{}>", g.name.trim(), g.address) }).collect();
    let pairs = [("title", title), ("when", when), ("place", place.clone())];
    let (subject, body) = if cancel { ("invite-cancel-subject", "invite-cancel-body") } else if place.is_empty() { ("invite-subject", "invite-body") } else { ("invite-subject", "invite-body-place") };
    draft.subject = say(tr, subject, &pairs);
    draft.body = say(tr, body, &pairs);
    draft.add_signature(signature);
    draft.calendar = Some(ics);
    if let Some(uid) = lines::unfold(text).iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()) {
        draft.links = vec![crate::links::event_uri(&uid)];
    }
    draft
}

/// The unsent messages of the meeting with this UID that you organize, in
/// your Drafts: each draft carrying an iTIP REQUEST or CANCEL of it whose
/// organizer is one of `own`, with its guests' addresses.
pub fn pending_of(uid: &str, own: &[String]) -> Vec<Pending> {
    let mut out = Vec::new();
    for draft in crate::compose::Draft::all() {
        let Some(ics) = draft.calendar.as_deref() else { continue };
        let source = lines::unfold(ics);
        let named = |name: &str| source.iter().find(|l| lines::name(l) == name).map(|l| lines::value(l).trim().to_string()).unwrap_or_default();
        let method = named("METHOD").to_ascii_uppercase();
        let organizer = address_of(&named("ORGANIZER"));
        if !(method == "REQUEST" || method == "CANCEL") || named("UID") != uid || !own.contains(&organizer) {
            continue;
        }
        let to = draft.to.iter().map(|t| crate::compose::address_of(t).unwrap_or_else(|| t.trim().to_string()).to_lowercase()).collect();
        out.push(Pending { id: draft.id.clone(), to, ics: ics.to_string() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(extra: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:plan-1\r\nDTSTAMP:20261001T000000Z\r\nSEQUENCE:2\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Planning\r\nX-SIOUL-BEFORE:15\r\n{extra}BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")
    }

    /// A text with its long lines unfolded, as the tests read it.
    fn un(text: &str) -> String {
        lines::unfold(text).join("\r\n") + "\r\n"
    }

    fn me() -> Guest {
        Guest { address: "me@example.org".into(), name: "Camille".into() }
    }

    fn guest(address: &str, name: &str) -> Guest {
        Guest { address: address.into(), name: name.into() }
    }

    /// Guests set: you as organizer, a new guest asked to answer before the
    /// alarm, a kept guest's line as it was (its answer, its mark), one taken
    /// out; a new guest left to you (`SCHEDULE-AGENT=CLIENT`) when the
    /// meeting's guests are, unmarked when they are not; nothing changed,
    /// the same text.
    #[test]
    fn guests_are_set_as_the_form_lists_them() {
        let own = event("");
        let invited = un(&with_guests(&own, &me(), &[guest("anne@example.net", "Anne"), guest("bob@example.net", "")], Sender::Server).unwrap());
        assert!(invited.contains("ORGANIZER;CN=Camille:mailto:me@example.org\r\n"), "{invited}");
        assert!(invited.contains("ATTENDEE;CN=Anne;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:anne@example.net\r\n"), "{invited}");
        assert!(invited.find("mailto:bob@example.net").unwrap() < invited.find("BEGIN:VALARM").unwrap(), "before its alarm");
        assert!(!invited.contains("SCHEDULE-AGENT") && !invited.contains("DTSTAMP:20261001T000000Z"), "the server tells them; the stamp renewed");
        assert_eq!(guests_of(&invited).iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["anne@example.net", "bob@example.net"]);
        assert!(crate::agenda::organized_meeting(&invited, &["me@example.org".into()]));
        let answered = invited.replace("PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:anne", "PARTSTAT=ACCEPTED:mailto:anne");
        let changed = un(&with_guests(&answered, &me(), &[guest("anne@example.net", "Anne"), guest("carl@example.net", "Carl")], Sender::Draft).unwrap());
        assert!(changed.contains("ATTENDEE;CN=Anne;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n"), "Anne's line kept as it is, no mark added (F1): {changed}");
        assert!(!changed.contains("bob@example.net") && changed.contains("CN=Carl;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE;SCHEDULE-AGENT=CLIENT:mailto:carl@example.net"));
        assert_eq!(changed.matches("ORGANIZER").count(), 1);
        assert_eq!(un(&with_guests(&changed, &me(), &[guest("Anne@Example.net", ""), guest("carl@example.net", "")], Sender::Draft).unwrap()), changed, "nothing changed");
        let alone = with_guests(&changed, &me(), &[], Sender::Draft).unwrap();
        assert!(!alone.contains("ATTENDEE") && alone.contains("ORGANIZER"), "every guest taken out");
        assert!(!crate::agenda::meeting_with_others(&alone, &["me@example.org".into()]));
    }

    /// The invitation as the guests receive it: METHOD:REQUEST, no alarm, no
    /// line of Sioul's, no SCHEDULE parameter, every guest named; told only
    /// of what they see; the cancellation to those taken off, and to all.
    #[test]
    fn the_invitation_and_its_cancellation() {
        let invited = un(&with_guests(&event("RELATED-TO;RELTYPE=PARENT:other\r\n"), &me(), &[guest("anne@example.net", "Anne"), guest("bob@example.net", "")], Sender::Draft).unwrap());
        let asked = un(&request(&invited).unwrap());
        assert!(asked.contains("METHOD:REQUEST\r\n") && asked.contains("UID:plan-1") && asked.contains("SEQUENCE:2"), "{asked}");
        assert!(!asked.contains("VALARM") && !asked.contains("X-SIOUL") && !asked.contains("RELATED-TO") && !asked.contains("SCHEDULE-AGENT"), "{asked}");
        assert!(asked.contains("mailto:anne@example.net") && asked.contains("mailto:bob@example.net"));
        assert!(!told(&invited, &invited.replace("X-SIOUL-BEFORE:15", "X-SIOUL-BEFORE:30").replace("TRIGGER:-PT15M", "TRIGGER:-PT5M")), "yours alone");
        assert!(told(&invited, &invited.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T080000Z")) && told(&invited, &invited.replace("SUMMARY:Planning", "SUMMARY:Plans")));
        let off = un(&cancel(&invited, &["bob@example.net".into()], false).unwrap());
        assert!(off.contains("METHOD:CANCEL") && off.contains("STATUS:CANCELLED") && off.contains("SEQUENCE:2") && off.contains("mailto:bob@example.net") && !off.contains("anne@example.net"), "{off}");
        let gone = with_guests(&invited, &me(), &[guest("anne@example.net", "")], Sender::Draft).unwrap();
        assert!(un(&cancel(&gone, &["bob@example.net".into()], false).unwrap()).contains("ATTENDEE:mailto:bob@example.net"), "named though the file no longer names them");
        let all = un(&cancel(&invited, &[], true).unwrap());
        assert!(all.contains("SEQUENCE:3") && all.contains("anne@example.net") && all.contains("bob@example.net") && !all.contains("PARTSTAT"), "{all}");
    }

    /// A guest's REPLY sets their answer, once; an older one, another
    /// event's, or one from someone not asked changes nothing.
    #[test]
    fn a_guest_s_reply_sets_their_answer() {
        let invited = un(&with_guests(&event(""), &me(), &[guest("anne@example.net", "Anne")], Sender::Draft).unwrap());
        let reply = |sequence: u32, who: &str, uid: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REPLY\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nSEQUENCE:{sequence}\r\nDTSTAMP:20991101T090000Z\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE;PARTSTAT=ACCEPTED;CN=Anne:mailto:{who}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
        let got = replied(&invited, &reply(2, "Anne@example.net", "plan-1")).unwrap();
        assert_eq!((got.guest.address.as_str(), got.answer.as_str(), got.time.as_deref()), ("anne@example.net", "accepted", None));
        let answered = un(&got.written.unwrap());
        assert!(answered.contains("PARTSTAT=ACCEPTED") && !answered.contains("RSVP=TRUE") && answered.contains("SCHEDULE-AGENT=CLIENT"), "{answered}");
        assert_eq!(lines::unfold(&answered).into_iter().filter(|l| !l.starts_with("ATTENDEE")).collect::<Vec<_>>(), lines::unfold(&invited).into_iter().filter(|l| !l.starts_with("ATTENDEE")).collect::<Vec<_>>(), "its PARTSTAT alone (F13)");
        let again = replied(&answered, &reply(2, "anne@example.net", "plan-1")).unwrap();
        assert!(again.written.is_none() && again.answer == "accepted", "said already: said, nothing written");
        assert_eq!(replied(&invited, &reply(1, "anne@example.net", "plan-1")), None, "an earlier version's answer");
        assert_eq!(replied(&invited, &reply(2, "eve@example.net", "plan-1")), None, "nobody added by an answer");
        assert_eq!(replied(&invited, &reply(2, "anne@example.net", "other")), None);
        assert_eq!(replied(&invited, &request(&invited).unwrap()), None, "no REPLY");
    }

    // ---- The review of 1e8551e: each probe turned into what now holds.

    /// A meeting holding the person's links (a landlord's message, a scanned
    /// letter's path, a note), a project, Sioul's own lines in both
    /// spellings, another program's line, an attachment, categories, an
    /// alarm: the invitation and the cancellation carry none of them, only
    /// what the guests need (F4, the probe `probe_request_carries_links_and_projects`).
    fn private_meeting() -> String {
        let private = "X-SIOUL-LINK;LINKREL=via;LABEL=Landlord:mid:abc@mail.example\r\n\
            X-SIOUL-LINK;LINKREL=related;VALUE=URI:file:///srv/papers/lease-scan.pdf\r\n\
            LINK;LINKREL=describedby;VALUE=URI:sioul:note/Tenancy/notes.md\r\n\
            X-SIOUL-REFID:tenancy\r\nREFID:deposit\r\nX-SIOUL-CONCEPT:sioul:label/family\r\nCONCEPT:sioul:label/law\r\n\
            X-SIOUL-RELATED-TO;RELTYPE=DEPENDS-ON:task-1\r\nRELATED-TO;RELTYPE=PARENT:task-2\r\n\
            X-SIOUL-COST:EMOTIONAL=7\r\nX-SIOUL-GAIN:2\r\nX-SIOUL-REMIND:30\r\nX-SIOUL-AFTER:10\r\n\
            X-MOZ-GENERATION:3\r\nATTACH:file:///srv/papers/scan.pdf\r\nCATEGORIES:Tenancy\r\nCONTACT;ALTREP=\"sioul:contact/landlord\":Landlord\r\n\
            LOCATION:3 rue de l'Exemple\r\nDESCRIPTION:Bring the papers\r\nURL:https://visio.example.org/r/abc\r\n\
            ORGANIZER;CN=Camille:mailto:me@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=NEEDS-ACTION;RSVP=TRUE;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\n";
        un(&event(private))
    }

    const PRIVATE: [&str; 18] = [
        "X-SIOUL", "mid:abc", "file:///", "sioul:note", "tenancy", "deposit", "sioul:label", "CONCEPT", "REFID", "RELATED-TO", "task-1", "task-2", "X-MOZ", "ATTACH", "CATEGORIES", "CONTACT", "VALARM", "TRIGGER",
    ];

    #[test]
    fn the_invitation_carries_what_its_guests_need_alone() {
        let text = private_meeting();
        for sent in [request(&text).unwrap(), cancel(&text, &[], true).unwrap(), cancel(&text, &["anne@example.net".into()], false).unwrap()] {
            let sent = un(&sent);
            for private in PRIVATE {
                assert!(!sent.contains(private), "{private} in:\n{sent}");
            }
            assert!(sent.contains("UID:plan-1") && sent.contains("SEQUENCE:") && sent.contains("ORGANIZER;CN=Camille:mailto:me@example.org") && sent.contains("mailto:anne@example.net") && !sent.contains("SCHEDULE-AGENT"), "{sent}");
        }
        let asked = un(&request(&text).unwrap());
        for needed in ["SUMMARY:Planning", "DTSTART:20991110T090000Z", "DTEND:20991110T100000Z", "LOCATION:3 rue de l'Exemple", "DESCRIPTION:Bring the papers", "URL:https://visio.example.org/r/abc", "METHOD:REQUEST"] {
            assert!(asked.contains(needed), "{needed} missing:\n{asked}");
        }
    }

    /// The meeting's own file, once it names a guest (F5, and the second
    /// review's N6): Sioul's own lines, its ties under either name, every
    /// `sioul:` address (a contact named by Sioul's address) out of it; every
    /// other program's line kept (its generation, its attachment, its
    /// categories, its alarm, a parent it names); the rest set aside for the
    /// person's own files: the time kept after it, its costs and gain, the
    /// reminder, each tie (the message it came from, the letter's file, the
    /// note, the projects, the task it waits on), never the task that
    /// prepares it (the task's own), and Sioul's other lines (its concepts)
    /// as they are. The once-only pass is the same rule.
    #[test]
    fn a_meeting_s_file_holds_what_its_guests_may_read() {
        let text = private_meeting().replace("ORGANIZER;", &format!("LINK;LINKREL=\"{}\";VALUE=UID:task-3\r\nORGANIZER;", crate::pairs::PREPARATION));
        let (file, aside) = set_aside(&text);
        let file = un(&file);
        for private in ["X-SIOUL", "mid:abc", "lease-scan", "sioul:note", "tenancy", "deposit", "sioul:label", "CONCEPT", "REFID", "task-1", "task-3", "LINK", "CONTACT", "sioul:"] {
            assert!(!file.contains(private), "{private} in:\n{file}");
        }
        for kept in ["X-MOZ-GENERATION:3", "ATTACH:file:///srv/papers/scan.pdf", "CATEGORIES:Tenancy", "BEGIN:VALARM", "TRIGGER:-PT15M", "RELATED-TO;RELTYPE=PARENT:task-2", "SCHEDULE-AGENT=CLIENT", "URL:https://visio", "VERSION:2.0"] {
            assert!(file.contains(kept), "{kept} kept: {file}");
        }
        assert_eq!(set_aside(&file).0, file, "said once: nothing more goes");
        let own = aside.own.expect("Sioul's own lines");
        assert_eq!((own.before, own.after, own.demands.emotional, own.demands.gain, own.remind.as_str()), (15, 10, Some(7), Some(2), "30"));
        let ties: Vec<(String, String)> = aside.ties.iter().map(|e| (e.to.clone(), e.how.clone())).collect();
        for (to, how) in [("mid:abc@mail.example", "source"), ("file:///srv/papers/lease-scan.pdf", "link"), ("sioul:note/Tenancy/notes.md", "note"), ("sioul:project/tenancy", "project"), ("sioul:project/deposit", "project"), ("uid:task-1", "link")] {
            assert!(ties.contains(&(to.to_string(), how.to_string())), "{to} ({how}) in {ties:?}");
        }
        assert!(aside.ties.iter().all(|e| e.from == "sioul:event/plan-1") && !ties.iter().any(|(to, _)| to.contains("task-3") || to.contains("task-2")), "{ties:?}");
        let lines: Vec<&str> = aside.lines.iter().map(|(t, l)| {
            assert!(t.is_empty(), "the series' own");
            l.as_str()
        }).collect();
        assert_eq!(lines, ["X-SIOUL-CONCEPT:sioul:label/family", "CONCEPT:sioul:label/law", "CONTACT;ALTREP=\"sioul:contact/landlord\":Landlord"], "Sioul's other lines, as they are");
        assert_eq!(aside_of(&event("")).own.map(|o| o.before), Some(15), "the lines a test's event holds");
        assert!(aside_of(&event("").replace("X-SIOUL-BEFORE:15\r\n", "")).is_empty());
    }

    /// One time of a weekly meeting from which another program excused Bob:
    /// moving the series or adding Carl keeps Bob out of that time, Carl
    /// asked to it too; Anne taken off the series leaves that time as well (F11).
    #[test]
    fn a_time_of_a_series_keeps_its_own_guests() {
        let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:w-1\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Weekly\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\nATTENDEE:mailto:bob@example.net\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:w-1\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991117T090000Z\r\nDTEND:20991117T100000Z\r\nSUMMARY:Weekly\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let over = |written: &str| {
            let written = un(written);
            written[written.find("RECURRENCE-ID").unwrap()..].to_string()
        };
        let same = with_guests(text, &me(), &guests_of(text), Sender::Server).unwrap();
        assert!(!over(&same).contains("bob@example.net"), "Bob stays out of that time: {}", over(&same));
        let carl = with_guests(text, &me(), &[guest("anne@example.net", ""), guest("bob@example.net", ""), guest("carl@example.net", "")], Sender::Server).unwrap();
        assert!(!over(&carl).contains("bob@example.net") && over(&carl).contains("carl@example.net"), "{}", over(&carl));
        assert!(un(&carl)[..un(&carl).find("RECURRENCE-ID").unwrap()].contains("bob@example.net"), "the series keeps Bob");
        let no_anne = with_guests(text, &me(), &[guest("bob@example.net", "")], Sender::Server).unwrap();
        assert!(!un(&no_anne).contains("anne@example.net"), "taken off the series, and that time: {no_anne}");
    }

    /// An ORGANIZER written without `mailto:` is no one a server reads as
    /// the person: never theirs (F15); a VTODO's people count as an event's
    /// (F16); two ORGANIZER lines, either order, or two events naming two
    /// organizers: malformed, never theirs, a meeting with others (F7).
    #[test]
    fn who_organizes_reads_as_a_server_reads_it() {
        use crate::agenda::{may_organize, meeting_with_others, organized_by_you, organized_meeting, participants};
        let own = vec!["me@example.org".to_string()];
        let no_scheme = event("ORGANIZER:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\n");
        assert!(!organized_meeting(&no_scheme, &own) && meeting_with_others(&no_scheme, &own), "F15");
        let alone = with_guests(&event("ORGANIZER:me@example.org\r\n"), &me(), &[guest("anne@example.net", "")], Sender::Server).unwrap();
        assert!(un(&alone).contains("ORGANIZER;CN=Camille:mailto:me@example.org\r\n") && !un(&alone).contains("ORGANIZER:me@"), "written as a URI: {alone}");
        let task = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:e-1\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Mine\r\nEND:VEVENT\r\nBEGIN:VTODO\r\nUID:t-1\r\nSUMMARY:Assigned\r\nORGANIZER:mailto:boss@example.org\r\nATTENDEE:mailto:anne@example.net\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        assert!(meeting_with_others(task, &own) && !organized_by_you(task, &own), "F16");
        for both in ["ORGANIZER:mailto:boss@example.org\r\nORGANIZER:mailto:me@example.org\r\n", "ORGANIZER:mailto:me@example.org\r\nORGANIZER:mailto:boss@example.org\r\n"] {
            let text = event(&format!("{both}ATTENDEE:mailto:anne@example.net\r\n"));
            assert!(!organized_by_you(&text, &own) && meeting_with_others(&text, &own), "F7: {both}");
            assert!(may_organize(Some(&text), Some(&text.replace("Planning", "Moved")), false, &own).is_err());
            assert_eq!(participants(&text).iter().filter(|p| p.organizer).count(), 1);
        }
        let first = event("ORGANIZER:mailto:boss@example.org\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\n");
        assert_eq!(organizer_of(&first).map(|o| o.address), Some("boss@example.org".to_string()), "the first line, as sabre reads it");
        let split = format!("{}BEGIN:VEVENT\r\nUID:plan-1\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTART:20991117T090000Z\r\nDTEND:20991117T100000Z\r\nORGANIZER:mailto:boss@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", event("ORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\n").trim_end_matches("END:VCALENDAR\r\n"));
        assert!(!organized_by_you(&split, &own), "two events, two organizers");
        assert!(organized_by_you(&event("ORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\n"), &own));
    }

    /// Anne declines one time of a weekly meeting that has no event of its
    /// own for it: when every guest is left to the person (Sioul drafts), an
    /// event is made for that time with Anne's answer, the series untouched;
    /// when a server tells the guests, the answer is said and nothing is
    /// written, since a new time there would have the server send it to
    /// every guest (F13, the probe `probe_reply_for_an_unchanged_occurrence`).
    #[test]
    fn an_answer_for_one_time_is_kept_on_that_time() {
        let series = |agent: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:w-2\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Weekly\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE;PARTSTAT=NEEDS-ACTION{agent}:mailto:anne@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
        let reply = |when: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REPLY\r\nBEGIN:VEVENT\r\nUID:w-2\r\nRECURRENCE-ID:{when}\r\nDTSTAMP:20991101T090000Z\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE;PARTSTAT=DECLINED:mailto:anne@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
        let drafted = series(";SCHEDULE-AGENT=CLIENT");
        let got = replied(&drafted, &reply("20991117T090000Z")).unwrap();
        assert_eq!((got.answer.as_str(), got.at), ("declined", Some(4_098_589_200)));
        let written = un(&got.written.expect("kept on that time"));
        let (master, own) = written.split_at(written.rfind("BEGIN:VEVENT").unwrap());
        assert!(master.contains("PARTSTAT=NEEDS-ACTION") && own.contains("PARTSTAT=DECLINED") && own.contains("DTSTART:20991117T090000Z") && !own.contains("RRULE"), "{written}");
        assert!(replied(&written, &reply("20991117T090000Z")).unwrap().written.is_none(), "said already");
        assert_eq!(replied(&drafted, &reply("20991118T090000Z")), None, "no such time");
        let served = replied(&series(""), &reply("20991117T090000Z")).unwrap();
        assert!(served.written.is_none() && served.answer == "declined", "its server's to keep: said, not written");
        assert!(left_to_you(&drafted) && !left_to_you(&series("")));
    }

    /// A meeting back after its cancellation went to its guests: each event
    /// one SEQUENCE past the cancellation's (F14).
    #[test]
    fn a_meeting_back_after_its_cancellation_is_a_later_version() {
        let invited = un(&with_guests(&event(""), &me(), &[guest("anne@example.net", "")], Sender::Draft).unwrap());
        let cancelled = un(&cancel(&invited, &[], true).unwrap());
        assert!(cancelled.contains("SEQUENCE:3"), "{cancelled}");
        let back = un(&after_cancel(&invited).unwrap());
        assert!(back.contains("SEQUENCE:4") && !back.contains("SEQUENCE:2") && un(&request(&back).unwrap()).contains("SEQUENCE:4"), "{back}");
    }

    /// A calendar file opened in Sioul: a booking naming the person a guest,
    /// and someone else's invitation, are added as before; one naming the
    /// person the organizer of guests is refused, with its own reason (F8,
    /// the probe `probe_calendar_file_with_a_guest_refused`).
    #[test]
    fn an_opened_file_of_someone_else_s_meeting_is_added() {
        crate::config::stand_ins_only();
        crate::agenda::own_addresses_here(Some(&["me@example.org"]));
        let calendar = crate::vdir::create(crate::vdir::Kind::Calendars, crate::vdir::LOCAL, "Opened files", None, &["VEVENT"]).unwrap();
        let booking = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:PUBLISH\r\nBEGIN:VEVENT\r\nUID:booking-1@booking.example\r\nDTSTAMP:20261101T090000Z\r\nDTSTART:20991104T150000Z\r\nDTEND:20991104T151500Z\r\nSUMMARY:Dentist\r\nORGANIZER;CN=Booking:mailto:noreply@booking.example\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:me@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let request = booking.replace("METHOD:PUBLISH", "METHOD:REQUEST").replace("booking-1", "board-1").replace("ATTENDEE;PARTSTAT=ACCEPTED:mailto:me@example.org", "ATTENDEE:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net");
        let mine = request.replace("board-1", "mine-1").replace("ORGANIZER;CN=Booking:mailto:noreply@booking.example", "ORGANIZER:mailto:me@example.org");
        let record = |file: &str| crate::agenda::calendar_file(file.as_bytes(), &jiff::tz::TimeZone::UTC).unwrap().record(&calendar, &[]);
        let recorded = (record(booking), record(&request), record(&mine));
        // The booking's own update (the same UID, a later SEQUENCE, a new time): it takes the
        // first one's place, never doubled; an older one changes nothing; another
        // organizer's of the same UID is refused (the second review's N9, its probe turned).
        let updated = booking.replace("DTSTAMP:20261101T090000Z", "DTSTAMP:20261102T090000Z\r\nSEQUENCE:1").replace("DTSTART:20991104T150000Z", "DTSTART:20991105T150000Z").replace("DTEND:20991104T151500Z", "DTEND:20991105T151500Z");
        let path = recorded.0.as_ref().map(|w| w[0].0.clone()).unwrap();
        let replaced = record(&updated);
        let after_update = std::fs::read_to_string(&path).unwrap_or_default();
        let older = record(booking);
        let after_older = std::fs::read_to_string(&path).unwrap_or_default();
        let theirs = record(&updated.replace("noreply@booking.example", "eve@example.net").replace("SEQUENCE:1", "SEQUENCE:2"));
        let files = calendar.items().len();
        crate::agenda::own_addresses_here(None);
        let _ = std::fs::remove_dir_all(&calendar.dir);
        assert!(recorded.0.is_ok() && recorded.1.is_ok(), "{recorded:?}");
        assert_eq!(recorded.2, Err(crate::agenda::said(crate::agenda::Refusal::Organizes, &crate::agenda::Change::Write(""))), "its own reason");
        assert_ne!(crate::agenda::Refusal::Organizes.word(&crate::agenda::Change::Write("")), crate::agenda::Refusal::WithOthers.word(&crate::agenda::Change::Write("")));
        assert_eq!(replaced.map(|w| w.into_iter().map(|(p, there)| (p == path, there)).collect::<Vec<_>>()), Ok(vec![(true, true)]), "the same file, written over");
        assert!(after_update.contains("DTSTART:20991105T150000Z") && !after_update.contains("DTSTART:20991104T150000Z"), "the update's time: {after_update}");
        assert!(older.is_ok() && after_older == after_update, "an older one changes nothing");
        assert!(theirs.is_err(), "another organizer's: {theirs:?}");
        assert_eq!(files, 2, "never doubled: the booking and the request");
    }

    // ---- Per meeting (the owner's word, 6 November 2026): F1, F2, F3.

    /// A meeting made elsewhere (Anne unmarked), written by Sioul however its
    /// device's check stands: Anne's line kept byte for byte, never marked
    /// (sabre would cancel the meeting for Anne: F1, the probe
    /// `probe_server_then_draft_adds_client_untold`), Bob added unmarked as
    /// Anne is; a meeting made in Sioul (Anne left to you): Anne's mark never
    /// taken out (nobody would tell Anne again: F2, the probe
    /// `probe_draft_then_server_keeps_client`), Bob added left to you too.
    #[test]
    fn a_guest_s_mark_is_never_changed() {
        let theirs = event("ORGANIZER;CN=Camille:mailto:me@example.org\r\nATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n");
        let anne_line = "ATTENDEE;CN=Anne;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n";
        assert_eq!(marks_new_guests(&theirs), Sender::Server, "made elsewhere");
        for marks in [Sender::Server, Sender::Draft] {
            assert_eq!(with_guests(&theirs, &me(), &guests_of(&theirs), marks).unwrap(), theirs, "nothing changed, nothing written: {marks:?}");
            let more = un(&with_guests(&theirs, &me(), &[guest("anne@example.net", "Anne"), guest("bob@example.net", "")], marks).unwrap());
            assert!(more.contains(anne_line), "{more}");
        }
        let bob = un(&with_guests(&theirs, &me(), &[guest("anne@example.net", "Anne"), guest("bob@example.net", "")], marks_new_guests(&theirs)).unwrap());
        assert!(!bob.contains("SCHEDULE-AGENT"), "Bob told as Anne is: {bob}");
        assert_eq!((teller(&bob, "anne@example.net", Sender::Server), teller(&bob, "bob@example.net", Sender::Server)), (Sender::Server, Sender::Server));
        let made = theirs.replace("PARTSTAT=ACCEPTED:mailto:anne", "PARTSTAT=ACCEPTED;SCHEDULE-AGENT=CLIENT:mailto:anne");
        assert_eq!(marks_new_guests(&made), Sender::Draft, "made in Sioul");
        assert_eq!(marks_new_guests(&event("")), Sender::Draft, "no guest yet: made in Sioul");
        let moved = made.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T110000Z");
        for marks in [Sender::Server, Sender::Draft] {
            let kept = un(&with_guests(&moved, &me(), &[guest("anne@example.net", "Anne"), guest("bob@example.net", "")], marks).unwrap());
            assert!(kept.contains("ATTENDEE;CN=Anne;PARTSTAT=ACCEPTED;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\n"), "{kept}");
        }
        let both = un(&with_guests(&moved, &me(), &[guest("anne@example.net", "Anne"), guest("bob@example.net", "")], marks_new_guests(&made)).unwrap());
        assert!(both.contains("SCHEDULE-AGENT=CLIENT:mailto:bob@example.net"));
        assert_eq!((teller(&both, "Anne@example.net", Sender::Server), clients(&both)), (Sender::Draft, vec!["anne@example.net".to_string(), "bob@example.net".to_string()]));
    }

    /// What a scheduling server sends by itself (sabre's
    /// `significantChangeProperties`): a new time, rule or status, a time of
    /// the series added; never a new title, place or notes alone. Those
    /// reach its guests by `SCHEDULE-FORCE-SEND=REQUEST` on the first
    /// event's lines, which the server takes out of the copy it keeps
    /// (`without_force_send` says that copy) (F3).
    #[test]
    fn a_change_the_server_would_not_send_is_asked_of_it_once() {
        let meeting = event("LOCATION:Room 2\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:bob@example.net\r\n");
        assert!(significant(&meeting, &meeting.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T080000Z")));
        assert!(significant(&meeting, &meeting.replace("SUMMARY:Planning", "STATUS:CANCELLED\r\nSUMMARY:Planning")));
        for alone in [meeting.replace("Room 2", "Hall"), meeting.replace("SUMMARY:Planning", "SUMMARY:Plans"), meeting.replace("SUMMARY:Planning", "SUMMARY:Planning\r\nDESCRIPTION:Bring the plans")] {
            assert!(told(&meeting, &alone) && !significant(&meeting, &alone), "{alone}");
        }
        let (forced, named) = with_force_send(&meeting, &["Anne@example.net".into(), "eve@example.net".into()]);
        assert_eq!(named, ["anne@example.net"], "those it names");
        assert!(un(&forced).contains("ATTENDEE;SCHEDULE-FORCE-SEND=REQUEST:mailto:anne@example.net\r\n") && !un(&request(&forced).unwrap()).contains("FORCE"), "{forced}");
        assert_eq!(without_force_send(&forced), lines::fold(&lines::unfold(&meeting)), "the copy its server keeps");
        assert_eq!(without_force_send(&meeting), meeting);
        let mine = every_guest_left_to_you(&meeting);
        assert_eq!(clients(&mine), ["anne@example.net", "bob@example.net"], "your own invitation's file: each left to you");
    }

    // ---- The second review of the organizer's door (6 November 2026): each probe turned.

    /// N2: notes another program wrote with no-break spaces, a line
    /// separator and CRLF line ends, given back by the form as Qt gives them
    /// (spaces, LF), a margin changed: the file's DESCRIPTION kept byte for
    /// byte, and no guest told (the probe
    /// `probe2_app_nbsp_in_notes_tells_every_guest`, its core half).
    #[test]
    fn notes_as_the_form_gives_them_back_tell_nobody() {
        let notes = "DESCRIPTION:Salle\u{a0}2 : apportez «\u{a0}les plans\u{a0}»\\nEtage 3\u{2028}porte B  \r\n";
        let text = event(&format!("{notes}ORGANIZER:mailto:me@example.org\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\n"));
        let zone = jiff::tz::TimeZone::UTC;
        let mut edit = crate::agenda::edit_of_text(&text, &zone).unwrap();
        edit.notes = edit.notes.replace('\u{a0}', " ").replace('\u{2028}', "\n");
        assert!(!told(&text, &crate::agenda::apply(&text, &edit, &zone).unwrap()), "the notes as Qt gives them back: nobody told");
        edit.margins.before = 40;
        let applied = un(&crate::agenda::apply(&text, &edit, &zone).unwrap());
        assert!(applied.contains(notes.trim_end()), "the file's own bytes kept: {applied}");
        assert!(!told(&text, &applied) && applied.contains("SEQUENCE:2"), "nobody told, no revision: {applied}");
        edit.notes = "Salle 3".into();
        assert!(told(&text, &crate::agenda::apply(&text, &edit, &zone).unwrap()), "a real change is told");
        assert_eq!(crate::agenda::normalised("a\u{a0}b \r\nc\u{202f}d\u{2028}e  "), "a b\nc d\ne");
    }

    /// N6: a meeting made in Nextcloud with its attachment, categories,
    /// colour, video link, Apple's map point and the person's own alarm: the
    /// file keeps every one of them through the door; a time block's alarm,
    /// marked as Sioul's, goes; the drafts carry none of them (the probe
    /// `probe2_other_programs_lines_leave_the_meeting` turned).
    #[test]
    fn other_programs_lines_stay_in_the_meeting() {
        let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Nextcloud calendar v4//EN\r\nX-WR-CALNAME:Work\r\nBEGIN:VEVENT\r\nUID:nc-1\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Board\r\nATTACH;FMTTYPE=application/pdf;FILENAME=/agenda.pdf;X-NC-FILE-ID=42:/f/42\r\nCATEGORIES:Work\r\nCOLOR:tomato\r\nCONFERENCE;VALUE=URI;FEATURE=VIDEO:https://meet.example.org/abc\r\nX-APPLE-STRUCTURED-LOCATION;VALUE=URI:geo:48.85,2.35\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:anne@example.net\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT1H\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!(for_guests(text), text, "nothing of Sioul's: the file as it is");
        let block = text.replace("END:VALARM\r\nEND:VEVENT", &format!("END:VALARM\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:{}\r\n{}\r\nEND:VALARM\r\nX-SIOUL-TASK:t-1\r\nEND:VEVENT", crate::blocks::ALARM_BEFORE, crate::blocks::ALARM_MARK));
        let file = un(&for_guests(&block));
        assert_eq!(file.matches("BEGIN:VALARM").count(), 1, "the block's alarm, Sioul's, out; the person's stays: {file}");
        assert!(file.contains("TRIGGER:-PT1H") && !file.contains("X-SIOUL"), "{file}");
        for kept in ["ATTACH;", "CATEGORIES:Work", "COLOR:tomato", "CONFERENCE;", "X-APPLE-STRUCTURED-LOCATION", "X-WR-CALNAME:Work"] {
            assert!(file.contains(kept), "{kept}: {file}");
        }
        let asked = un(&request(&block).unwrap());
        for gone in ["ATTACH", "CATEGORIES", "COLOR", "CONFERENCE", "X-APPLE", "VALARM", "X-SIOUL"] {
            assert!(!asked.contains(gone), "{gone} in the draft: {asked}");
        }
        assert!(!significant(text, &for_guests(&block)) && !told(text, &for_guests(&block)), "nothing a server mails for");
    }

    /// N6: a child's event given a guest through the door: who takes it
    /// and its kind out of the file, kept aside for the meeting's own lines
    /// (`meeting_lines`), put back for the children's upkeep to read; the
    /// stand-in of a child's event given a guest sends no `sioul:` address,
    /// in its file nor in a draft, its details kept (the probe
    /// `probe2_child_event_loses_holder_and_kind` turned).
    #[test]
    fn a_child_s_holder_and_kind_are_kept_aside() {
        let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:child-1\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Tom: dentist\r\nX-SIOUL-CONCEPT:tag:aurelienpierre.com,2026:sioul/child-kind/health\r\nX-SIOUL-TAKEN-BY;CN=Sam;PARTSTAT=ACCEPTED;SINCE=20261103T090000Z:mailto:sam@example.org\r\nORGANIZER;CN=Camille:mailto:me@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:dr@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let (file, aside) = set_aside(text);
        assert!(!un(&file).contains("TAKEN-BY") && !un(&file).contains("child-kind"), "{file}");
        let kept = crate::meeting_lines::Kept { times: [(String::new(), aside.lines.iter().map(|(_, l)| l.clone()).collect())].into() };
        let read = crate::meeting_lines::merged_with(&file, &kept);
        assert_eq!(crate::holders::kind_of(&read), Some(crate::holders::Kind::Health));
        assert_eq!(crate::holders::taken(&read, None, &jiff::tz::TimeZone::UTC).to.map(|h| h.address), Some("sam@example.org".to_string()));
        // Its stand-in in your own agenda, given a guest.
        let of = crate::holders::Of { child: text, target: "camille/personal", work: false, kind: Some(crate::holders::Kind::Health), details: "Details: Tom’s agenda", away: "Away" };
        let stand_in = crate::holders::stand_in(&of, 0).unwrap().replace("END:VEVENT", "ORGANIZER:mailto:me@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nEND:VEVENT");
        let (file, aside) = set_aside(&stand_in);
        let file = un(&file);
        assert!(!file.to_ascii_lowercase().contains("sioul:") && file.contains("DESCRIPTION:Details: Tom’s agenda\r\n"), "{file}");
        assert!(aside.lines.iter().any(|(_, l)| l.starts_with("URL:sioul:event/child-1")), "its address kept aside: {aside:?}");
        assert!(!un(&request(&stand_in).unwrap()).to_ascii_lowercase().contains("sioul:"), "never in a draft");
    }

    /// N10: marks read per event and per line, as sabre reads them: Anne
    /// left to you in the series and not in one time of it another program
    /// copied is told two ways (`mixed`), never counted as left to you; Bob
    /// asked to that time alone is its guest (`one_time_guests`), told of a
    /// change to it, never of the series', and Bob's invitation holds that
    /// time alone (`request_for`).
    #[test]
    fn marks_and_guests_are_read_per_event() {
        let series = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:w-3\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Weekly\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:carl@example.net\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:w-3\r\nRECURRENCE-ID:20991117T090000Z\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20991117T090000Z\r\nDTEND:20991117T100000Z\r\nSUMMARY:Weekly\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:carl@example.net\r\nATTENDEE:mailto:bob@example.net\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!((clients(series), mixed(series)), (vec!["carl@example.net".to_string()], vec!["anne@example.net".to_string()]));
        assert_eq!(teller(series, "anne@example.net", Sender::Server), Sender::Server, "never left to you alone");
        assert_eq!(one_time_guests(series).iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["bob@example.net"]);
        assert_eq!(every_guest(series).len(), 3);
        let renamed = series.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T080000Z");
        assert!(!told_guest(series, &renamed, "bob@example.net") && told_guest(series, &renamed, "anne@example.net"), "the series moved: not Bob's time");
        let that_time = series.replace("DTSTART:20991117T090000Z", "DTSTART:20991117T080000Z");
        assert!(told_guest(series, &that_time, "bob@example.net"), "Bob's time moved");
        let asked = un(&request_for(series, &["bob@example.net".into()]).unwrap());
        assert!(asked.contains("RECURRENCE-ID:20991117T090000Z") && !asked.contains("RRULE") && asked.matches("BEGIN:VEVENT").count() == 1, "{asked}");
        assert_eq!(marks_new_guests(series), Sender::Draft);
    }

    /// N3: a meeting made in Sioul whose change here (Anne's update,
    /// drafted) was set aside for its server's version (Bob, inside the
    /// server, answered meanwhile): Anne's invitation of the version the
    /// server keeps, unsent, stays; the update goes; nothing more is drafted
    /// for Anne. Sent already, nothing is left: Anne heard the server's
    /// version. A version changed by another app since (a later SEQUENCE):
    /// Anne gets an invitation of it. The guest the server tells is named.
    #[test]
    fn a_set_aside_change_keeps_the_messages_still_true() {
        let agreed = event("ORGANIZER:mailto:me@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT;PARTSTAT=NEEDS-ACTION:mailto:anne@example.net\r\nATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:bob@example.net\r\n");
        let theirs = agreed.replace("PARTSTAT=NEEDS-ACTION:mailto:bob", "PARTSTAT=ACCEPTED:mailto:bob");
        let mine = agreed.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T110000Z").replace("SEQUENCE:2", "SEQUENCE:3");
        let invitation = Pending { id: "invited".into(), to: vec!["anne@example.net".into()], ics: request(&agreed).unwrap() };
        let update = Pending { id: "moved".into(), to: vec!["anne@example.net".into()], ics: request(&mine).unwrap() };
        let got = reconcile(&theirs, &[invitation.clone(), update.clone()], true);
        assert_eq!((got.keep.as_slice(), got.withdraw.as_slice(), got.invite.len()), (&["invited".to_string()][..], &["moved".to_string()][..], 0), "{got:?}");
        assert_eq!(got.served.iter().map(|g| g.address.as_str()).collect::<Vec<_>>(), ["bob@example.net"], "the server tells Bob");
        let sent = reconcile(&theirs, std::slice::from_ref(&update), true);
        assert!(sent.withdraw == ["moved"] && sent.invite.is_empty(), "Anne heard the server's version: {sent:?}");
        let elsewhere = theirs.replace("DTSTART:20991110T090000Z", "DTSTART:20991110T100000Z").replace("SEQUENCE:2", "SEQUENCE:3");
        let other = reconcile(&elsewhere, &[invitation, update], true);
        assert_eq!((other.withdraw.len(), other.invite.iter().map(|g| g.address.as_str()).collect::<Vec<_>>()), (2, vec!["anne@example.net"]), "{other:?}");
        let off = Pending { id: "off".into(), to: vec!["carl@example.net".into()], ics: cancel(&agreed, &["carl@example.net".into()], false).unwrap() };
        assert_eq!(reconcile(&theirs, &[off], true).keep, ["off"], "Carl is off the server's version too");
        let plain = reconcile(&theirs, &[], false);
        assert!(plain.served.is_empty(), "a server that tells nobody");
    }

    /// N7: the version put back once its guests heard the change made over
    /// it: one SEQUENCE past what they received, never the old one.
    #[test]
    fn a_change_taken_back_is_a_later_version() {
        let before = event("ORGANIZER:mailto:me@example.org\r\nATTENDEE:mailto:anne@example.net\r\n");
        let moved = before.replace("SEQUENCE:2", "SEQUENCE:3").replace("DTSTART:20991110T090000Z", "DTSTART:20991110T110000Z");
        let back = un(&past(&before, &moved).unwrap());
        assert!(back.contains("SEQUENCE:4") && back.contains("DTSTART:20991110T090000Z") && !back.contains("DTSTAMP:20261001T000000Z"), "{back}");
        assert_eq!(sequence_of(&back), 4);
    }
}
