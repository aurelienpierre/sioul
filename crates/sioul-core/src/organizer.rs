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
//! **What guests read.** A meeting's file and every message to its guests
//! hold only what they need (`SHARED`, an allow-list of RFC 5546's REQUEST
//! and CANCEL properties): its times, how it repeats, its title, place and
//! notes, its status and who comes. Sioul's own lines (the time kept around
//! it, its costs and gain, your reminder), your ties (links, projects,
//! RELATED-TO) and your alarms never leave the device that way: the door
//! sets them aside (`set_aside`), into your own files (review of 1e8551e,
//! F4, F5).

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

/// The guests of a meeting left to you (`left_to_you_line`), by address in
/// lower case, as their first line in the file says it (the series' first).
pub fn clients(text: &str) -> Vec<String> {
    let (mut seen, mut out): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let (mut inside, mut nested) = (false, 0usize);
    for line in lines::unfold(text) {
        match lines::name(&line).as_str() {
            "BEGIN" if !inside => inside = lines::value(&line).trim().eq_ignore_ascii_case("VEVENT"),
            "BEGIN" => nested += 1,
            "END" if inside && nested > 0 => nested -= 1,
            "END" if inside => inside = false,
            "ATTENDEE" if inside && nested == 0 => {
                let who = address_of(lines::value(&line));
                if seen.contains(&who) {
                    continue;
                }
                if left_to_you_line(&line) {
                    out.push(who.clone());
                }
                seen.push(who);
            }
            _ => {}
        }
    }
    out
}

/// Who tells `address`, a guest of the meeting `text`: your drafts when its
/// line leaves them to you, else `server` (its calendar's server when it
/// schedules for the organizer, else your drafts too).
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
    let guests = guests_of(text);
    if !guests.is_empty() && clients(text).is_empty() { Sender::Server } else { Sender::Draft }
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

/// What a meeting's guests may read of each of its events, by property
/// name as written (`lines::raw_name`, so that none of Sioul's own names
/// reads as one of these): RFC 5546's REQUEST and CANCEL properties (§3.2.2,
/// §3.2.5) that say the meeting, never ATTACH (files), CATEGORIES (your own
/// filing), CONTACT nor RELATED-TO (your ties), nor any `X-` line; LINK,
/// REFID and CONCEPT are no iTIP properties. An allow-list: whatever another
/// program adds stays out too (review of 1e8551e, F4).
pub const SHARED: [&str; 26] = [
    "UID", "RECURRENCE-ID", "SEQUENCE", "DTSTAMP", "CREATED", "LAST-MODIFIED", "DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "DESCRIPTION", "LOCATION", "GEO", "STATUS", "CLASS",
    "TRANSP", "PRIORITY", "URL", "COMMENT", "RESOURCES", "REQUEST-STATUS", "ORGANIZER", "ATTENDEE",
];

/// The head of a calendar a meeting's file keeps (RFC 5545 §3.6): its
/// version, the program that made it, its calendar scale; no METHOD (RFC
/// 4791 §4.1), no `X-` line.
const HEAD: [&str; 3] = ["VERSION", "PRODID", "CALSCALE"];

/// Whether a line of an event's own level is one its guests may read (`SHARED`).
fn shared(line: &str) -> bool {
    SHARED.contains(&lines::raw_name(line).as_str())
}

/// A meeting's file as its guests may read it (review of 1e8551e, F5): each
/// VEVENT with its `SHARED` lines alone, no alarm (yours, in Sioul's own
/// files); the calendar's head (`HEAD`) and its zones whole; another kind of
/// component (a task) as it is. The same text when nothing goes.
pub fn shared_only(text: &str) -> String {
    let source = lines::unfold(text);
    let mut out: Vec<String> = Vec::with_capacity(source.len());
    // Where each line is: the calendar's own level, an event, a zone or another component, how deep.
    let (mut component, mut depth): (Option<String>, usize) = (None, 0);
    for line in &source {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        match (&component, name.as_str()) {
            (None, "BEGIN") if value != "VCALENDAR" => {
                component = Some(value);
                depth = 0;
                out.push(line.clone());
            }
            (None, "BEGIN" | "END") => out.push(line.clone()),
            (None, _) => {
                if HEAD.contains(&lines::raw_name(line).as_str()) {
                    out.push(line.clone());
                }
            }
            (Some(_), "BEGIN") => {
                depth += 1;
                if component.as_deref() != Some("VEVENT") {
                    out.push(line.clone());
                }
            }
            (Some(_), "END") if depth > 0 => {
                depth -= 1;
                if component.as_deref() != Some("VEVENT") {
                    out.push(line.clone());
                }
            }
            (Some(_), "END") => {
                component = None;
                out.push(line.clone());
            }
            (Some(kind), _) if kind == "VEVENT" => {
                if depth == 0 && shared(line) {
                    out.push(line.clone());
                }
            }
            (Some(_), _) => out.push(line.clone()),
        }
    }
    if out == source { text.to_string() } else { lines::fold(&out) }
}

/// What a meeting's file held that is yours alone (`set_aside`): Sioul's own
/// lines of its main event (the time kept around it, its costs and gain, your
/// reminder), and its ties, each as your own store of ties keeps one
/// (`links::LocalLinks`, `from` the event).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Aside {
    /// None when the file held none of those lines.
    pub own: Option<crate::own_time::Own>,
    pub ties: Vec<crate::links::Edge>,
}

impl Aside {
    pub fn is_empty(&self) -> bool {
        self.own.is_none() && self.ties.is_empty()
    }
}

/// Sioul's own lines and the ties of a meeting's main event (`Aside`),
/// read from its file: what the door moves into your own files when it writes
/// the meeting as its guests may read it (`shared_only`). A tie a task holds
/// already (the task that prepares it, `pairs`) is the task's.
pub fn aside_of(text: &str) -> Aside {
    let source = lines::unfold(text);
    let Some(main) = events(&source).into_iter().find(|r| !source[r.clone()].iter().any(|l| lines::name(l) == "RECURRENCE-ID")) else { return Aside::default() };
    let block = &source[main];
    let uid = block.iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()).unwrap_or_default();
    let from = crate::links::event_uri(&uid);
    let mut aside = Aside::default();
    let mut nested = 0usize;
    let mut own_lines = false;
    for line in &block[1..block.len().saturating_sub(1)] {
        let name = lines::name(line);
        match name.as_str() {
            "BEGIN" => nested += 1,
            "END" => nested = nested.saturating_sub(1),
            _ if nested > 0 => {}
            "LINK" => {
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
            "RELATED-TO" => {
                let other = lines::value(line).trim();
                if !other.is_empty() {
                    aside.ties.push(crate::links::Edge { from: from.clone(), to: format!("uid:{other}"), how: "link".into() });
                }
            }
            "REFID" => {
                let project = lines::unescape(lines::value(line).trim());
                if !project.is_empty() {
                    aside.ties.push(crate::links::Edge { from: from.clone(), to: crate::links::project_uri(&project), how: "project".into() });
                }
            }
            name if [crate::demands::BEFORE, crate::demands::AFTER, crate::demands::COST, crate::demands::GAIN, crate::reminders::REMIND].contains(&name) => own_lines = true,
            _ => {}
        }
    }
    if own_lines && let Some(edit) = crate::agenda::edit_of_text(text, &crate::zone::current()) {
        aside.own = Some(crate::own_time::Own { before: edit.margins.before, after: edit.margins.after, demands: edit.demands, remind: edit.remind });
    }
    aside
}

/// A meeting's file as its guests may read it (`shared_only`), and what it
/// held of yours (`aside_of`), for your own files.
pub fn set_aside(text: &str) -> (String, Aside) {
    (shared_only(text), aside_of(text))
}

/// A meeting's file without what Sioul wrote into it (review of 1e8551e,
/// F5, the once-only pass over meetings you organize already): Sioul's own
/// lines (`X-SIOUL-…`) and its ties (LINK, REFID, CONCEPT, RELATED-TO, either
/// spelling) of each event; what other programs wrote (an alarm) is left to
/// the next write through the door, which keeps only `SHARED`. The same text
/// when nothing goes.
pub fn without_own_lines(text: &str) -> String {
    let source = lines::unfold(text);
    let mut out: Vec<String> = Vec::with_capacity(source.len());
    let (mut inside, mut nested) = (false, 0usize);
    for line in &source {
        let name = lines::name(line);
        match name.as_str() {
            "BEGIN" if !inside => inside = lines::value(line).trim().eq_ignore_ascii_case("VEVENT"),
            "BEGIN" => nested += 1,
            "END" if inside && nested > 0 => nested -= 1,
            "END" if inside => inside = false,
            _ if inside && nested == 0 && (lines::raw_name(line).starts_with("X-SIOUL-") || ["LINK", "REFID", "CONCEPT", "RELATED-TO"].contains(&name.as_str())) => continue,
            _ => {}
        }
        out.push(line.clone());
    }
    if out == source { text.to_string() } else { lines::fold(&out) }
}

/// What the guests see of an event: each VEVENT's times, rule, title,
/// place, notes and status, in order.
fn seen_by_guests(text: &str) -> Vec<String> {
    const SEEN: [&str; 12] = ["UID", "RECURRENCE-ID", "DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "LOCATION", "DESCRIPTION", "STATUS"];
    let source = lines::unfold(text);
    let mut out = Vec::new();
    for range in events(&source) {
        let mut nested = 0usize;
        out.push("BEGIN:VEVENT".to_string());
        for line in &source[range.start + 1..range.end - 1] {
            match lines::name(line).as_str() {
                "BEGIN" => nested += 1,
                "END" => nested = nested.saturating_sub(1),
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
    seen_by_guests(before) != seen_by_guests(after)
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
/// SCHEDULE-STATUS, SCHEDULE-FORCE-SEND).
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
            _ => out.push(line.clone()),
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

/// A meeting you organize as it comes back after its cancellation went to
/// its guests (Undo of its deletion, review of 1e8551e, F14): each event's
/// SEQUENCE two past its own, so one past the cancellation's
/// (`cancel`), which guests' programs take as a later version; its stamps
/// renewed. None for no event.
pub fn after_cancel(text: &str) -> Option<String> {
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
        kept.splice(place..place, [format!("SEQUENCE:{}", sequence + 2), format!("DTSTAMP:{now}"), format!("LAST-MODIFIED:{now}")]);
        out.extend(kept);
    }
    out.extend(source[at..].iter().cloned());
    Some(lines::fold(&out))
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

    /// The meeting's own file, once it names a guest: what its guests may
    /// read alone (`shared_only`); the rest set aside for the person's own
    /// files: the time kept after it, its costs and gain, the reminder, and
    /// each tie (the message it came from, the letter's file, the note, the
    /// projects, the tasks it is related to), never the task that prepares
    /// it (the task's own); another program's alarm stays out too (F5).
    /// The once-only pass takes Sioul's own lines and ties alone.
    #[test]
    fn a_meeting_s_file_holds_what_its_guests_may_read() {
        let text = private_meeting().replace("ORGANIZER;", &format!("LINK;LINKREL=\"{}\";VALUE=UID:task-3\r\nORGANIZER;", crate::pairs::PREPARATION));
        let (file, aside) = set_aside(&text);
        let file = un(&file);
        for private in PRIVATE.iter().chain(&["task-3"]) {
            assert!(!file.contains(private), "{private} in:\n{file}");
        }
        assert!(file.contains("SCHEDULE-AGENT=CLIENT") && file.contains("URL:https://visio") && file.contains("VERSION:2.0"), "a server's parameter and the head kept: {file}");
        assert_eq!(set_aside(&file).0, file, "said once: nothing more goes");
        let own = aside.own.expect("Sioul's own lines");
        assert_eq!((own.before, own.after, own.demands.emotional, own.demands.gain, own.remind.as_str()), (15, 10, Some(7), Some(2), "30"));
        let ties: Vec<(String, String)> = aside.ties.iter().map(|e| (e.to.clone(), e.how.clone())).collect();
        for (to, how) in [("mid:abc@mail.example", "source"), ("file:///srv/papers/lease-scan.pdf", "link"), ("sioul:note/Tenancy/notes.md", "note"), ("sioul:project/tenancy", "project"), ("sioul:project/deposit", "project"), ("uid:task-1", "link"), ("uid:task-2", "link")] {
            assert!(ties.contains(&(to.to_string(), how.to_string())), "{to} ({how}) in {ties:?}");
        }
        assert!(aside.ties.iter().all(|e| e.from == "sioul:event/plan-1") && !ties.iter().any(|(to, _)| to.contains("task-3")), "{ties:?}");
        let pass = un(&without_own_lines(&text));
        assert!(!pass.contains("X-SIOUL") && !pass.contains("REFID") && !pass.contains("RELATED-TO") && !pass.contains("LINK"), "{pass}");
        assert!(pass.contains("X-MOZ-GENERATION:3") && pass.contains("BEGIN:VALARM") && pass.contains("CATEGORIES:Tenancy"), "another program's lines left to the next write: {pass}");
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
        crate::agenda::own_addresses_here(None);
        let _ = std::fs::remove_dir_all(&calendar.dir);
        assert!(recorded.0.is_ok() && recorded.1.is_ok(), "{recorded:?}");
        assert_eq!(recorded.2, Err(crate::agenda::said(crate::agenda::Refusal::Organizes, &crate::agenda::Change::Write(""))), "its own reason");
        assert_ne!(crate::agenda::Refusal::Organizes.word(&crate::agenda::Change::Write("")), crate::agenda::Refusal::WithOthers.word(&crate::agenda::Change::Write("")));
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
    /// event's lines, asked once: the next write takes it out (F3).
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
        assert_eq!(without_force_send(&forced), lines::fold(&lines::unfold(&meeting)), "taken out at the next write");
        assert_eq!(without_force_send(&meeting), meeting);
        let mine = every_guest_left_to_you(&meeting);
        assert_eq!(clients(&mine), ["anne@example.net", "bob@example.net"], "your own invitation's file: each left to you");
    }
}
