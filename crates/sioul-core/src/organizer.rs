// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Meetings you organize (decision 7 of the event card, 5 November 2026):
//! an event's guests set from the form's "Invite…" field (`with_guests`),
//! what they must hear of a change (`told`), the invitation and the
//! cancellation sent to them when the calendar's server does not send them
//! itself (iTIP over iMIP, RFC 5546 and RFC 6047: `request`, `cancel`), and
//! a guest's answer read from their REPLY (`replied`).
//!
//! **Who sends.** A server that schedules by itself (RFC 6638,
//! `calendar-auto-schedule` in its DAV header, as Nextcloud does) emails the
//! guests when it receives the event, as your organizer's copy; Sioul then
//! drafts nothing. Otherwise (a plain CalDAV server, a calendar of this
//! device's own) Sioul drafts the invitation, which you send from Mail, and
//! marks each guest `SCHEDULE-AGENT=CLIENT` (RFC 6638 §7.1), so that a
//! server that would schedule after all leaves it to you: never both.
//!
//! These texts are written through the organizer's door alone
//! (`vdir::write_organized`): every other writer refuses a meeting with others.

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

/// Who sends the invitations of a meeting: its calendar's server, or you, from a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sender {
    /// The server schedules by itself (RFC 6638): Sioul drafts nothing.
    Server,
    /// Sioul drafts the iTIP message; you send it from Mail.
    Draft,
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
/// tell when you send the invitation (`Sender::Draft`).
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

/// The event with you as its organizer and these guests, in every VEVENT
/// of it (the series and each time of it changed): a guest kept keeps its
/// line as it is (its answer, its role), a new one asked to answer, one no
/// longer listed taken out; your own line among the guests, if any, kept.
/// Its stamps are renewed where it changed. The same text when nothing
/// changes; None for no event.
pub fn with_guests(text: &str, organizer: &Guest, guests: &[Guest], sender: Sender) -> Option<String> {
    let source = lines::unfold(text);
    let ranges = events(&source);
    if ranges.is_empty() {
        return None;
    }
    let now = crate::moves::utc(crate::zone::now().timestamp().as_second());
    let wanted: Vec<&Guest> = guests.iter().filter(|g| !g.address.trim().is_empty() && !organizer.is(&g.address)).collect();
    let mut out: Vec<String> = Vec::with_capacity(source.len() + guests.len() * 2);
    let mut at = 0;
    for range in ranges {
        out.extend(source[at..range.start].iter().cloned());
        at = range.end;
        let block = &source[range];
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
                    if !organizer.is(&address_of(lines::value(line))) {
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
                    if !wanted.iter().any(|g| g.is(&who)) || present.contains(&who) {
                        changed = true;
                        continue;
                    }
                    present.push(who);
                    // Left to you to tell: said on every guest, so that no server tells them too.
                    let agent = lines::param(line, "SCHEDULE-AGENT").unwrap_or_default();
                    if sender == Sender::Draft && !agent.eq_ignore_ascii_case("CLIENT") {
                        kept.push(with_params(line, &["SCHEDULE-AGENT"], &[("SCHEDULE-AGENT", "CLIENT")]));
                        changed = true;
                        continue;
                    }
                }
                _ => {}
            }
            kept.push(line.clone());
        }
        // Before its alarms, else before its end.
        let mut added: Vec<String> = Vec::new();
        if !has_organizer {
            added.push(person_line("ORGANIZER", organizer, ""));
        }
        added.extend(wanted.iter().filter(|g| !present.contains(&g.address.trim().to_lowercase())).map(|g| guest_line(g, sender)));
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

/// One VEVENT as the guests receive it: its own lines but Sioul's
/// (`X-SIOUL-…`) and its ties to your other items (RELATED-TO), with no
/// alarm (yours), its people's lines without what only a server reads
/// (RFC 6638's SCHEDULE-AGENT, SCHEDULE-STATUS, SCHEDULE-FORCE-SEND).
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
            "RELATED-TO" => continue,
            name if name.starts_with("X-SIOUL-") => continue,
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
/// organizer, the guests in `to` (all of them when empty: the whole meeting
/// cancelled, its SEQUENCE one more than the file's; else those taken off
/// it, at the file's SEQUENCE), and STATUS:CANCELLED. None for no event.
pub fn cancel(text: &str, to: &[String]) -> Option<String> {
    const KEPT: [&str; 9] = ["UID", "DTSTART", "DTEND", "DURATION", "RRULE", "RDATE", "EXDATE", "SUMMARY", "LOCATION"];
    let source = lines::unfold(text);
    let ranges = events(&source);
    let main = ranges.iter().find(|r| !source[(*r).clone()].iter().any(|l| lines::name(l) == "RECURRENCE-ID")).or(ranges.first())?.clone();
    let block = as_sent(&source[main]);
    let sequence: i64 = block.iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse().ok()).unwrap_or(0);
    let whole = to.is_empty();
    let now = crate::moves::utc(crate::zone::now().timestamp().as_second());
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "METHOD:CANCEL".to_string()];
    out.extend(zones(&source));
    out.push("BEGIN:VEVENT".to_string());
    for line in &block[1..block.len() - 1] {
        let name = lines::name(line);
        if KEPT.contains(&name.as_str()) || name == "ORGANIZER" {
            out.push(line.clone());
        } else if name == "ATTENDEE" && (whole || to.iter().any(|t| t.trim().eq_ignore_ascii_case(&address_of(lines::value(line))))) {
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

/// A guest's answer to a meeting you organize, from their REPLY (RFC 5546
/// §3.2.3): the event with that guest's PARTSTAT set, in the event of the
/// time it names (RECURRENCE-ID) or its series; the guest and their answer
/// ("accepted", "declined", "tentative"). None when the REPLY answers
/// another event, names no guest of it (nobody is added by an answer), is
/// older than the event (a lower SEQUENCE: it answers an earlier version),
/// or says what the event says already.
pub fn replied(text: &str, reply: &str) -> Option<(String, Guest, String)> {
    let theirs = lines::unfold(reply);
    if !theirs.iter().any(|l| lines::name(l) == "METHOD" && lines::value(l).trim().eq_ignore_ascii_case("REPLY")) {
        return None;
    }
    let range = events(&theirs).into_iter().next()?;
    let event = &theirs[range];
    let find = |name: &str| event.iter().find(|l| lines::name(l) == name).map(|l| lines::value(l).trim().to_string());
    let uid = find("UID")?;
    let recurrence = find("RECURRENCE-ID");
    let sequence: i64 = find("SEQUENCE").and_then(|s| s.parse().ok()).unwrap_or(0);
    let attendee = event.iter().find(|l| lines::name(l) == "ATTENDEE")?;
    let who = address_of(lines::value(attendee));
    let answer = lines::param(attendee, "PARTSTAT")?.to_ascii_uppercase();
    if !["ACCEPTED", "DECLINED", "TENTATIVE"].contains(&answer.as_str()) {
        return None;
    }
    let mut source = lines::unfold(text);
    let ranges = events(&source);
    let target = ranges.into_iter().find(|r| {
        let block = &source[r.clone()];
        let get = |name: &str| block.iter().find(|l| lines::name(l) == name).map(|l| lines::value(l).trim().to_string());
        get("UID").as_deref() == Some(uid.as_str()) && get("RECURRENCE-ID").map(|r| r.to_ascii_uppercase()) == recurrence.as_ref().map(|r| r.to_ascii_uppercase())
    })?;
    let ours: i64 = source[target.clone()].iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse().ok()).unwrap_or(0);
    if sequence < ours {
        return None;
    }
    let mut nested = 0usize;
    let mut guest = None;
    for i in target.start + 1..target.end - 1 {
        match lines::name(&source[i]).as_str() {
            "BEGIN" => nested += 1,
            "END" => nested = nested.saturating_sub(1),
            "ATTENDEE" if nested == 0 && address_of(lines::value(&source[i])) == who => {
                if lines::param(&source[i], "PARTSTAT").is_some_and(|p| p.eq_ignore_ascii_case(&answer)) {
                    return None;
                }
                source[i] = with_params(&source[i], &["PARTSTAT", "RSVP"], &[("PARTSTAT", &answer)]);
                guest = Some(Guest { address: who.clone(), name: lines::param(&source[i], "CN").unwrap_or_default() });
                break;
            }
            _ => {}
        }
    }
    let guest = guest?;
    Some((lines::fold(&source), guest, answer.to_ascii_lowercase()))
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
    /// alarm, a kept guest's line as it was (its answer), one taken out; by
    /// a draft, each guest left to you (`SCHEDULE-AGENT=CLIENT`), by the
    /// server, none; nothing changed, the same text.
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
        assert!(changed.contains("ATTENDEE;CN=Anne;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;SCHEDULE-AGENT=CLIENT:mailto:anne@example.net"), "Anne's answer kept: {changed}");
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
        let off = un(&cancel(&invited, &["bob@example.net".into()]).unwrap());
        assert!(off.contains("METHOD:CANCEL") && off.contains("STATUS:CANCELLED") && off.contains("SEQUENCE:2") && off.contains("mailto:bob@example.net") && !off.contains("anne@example.net"), "{off}");
        let gone = with_guests(&invited, &me(), &[guest("anne@example.net", "")], Sender::Draft).unwrap();
        assert!(un(&cancel(&gone, &["bob@example.net".into()]).unwrap()).contains("ATTENDEE:mailto:bob@example.net"), "named though the file no longer names them");
        let all = un(&cancel(&invited, &[]).unwrap());
        assert!(all.contains("SEQUENCE:3") && all.contains("anne@example.net") && all.contains("bob@example.net") && !all.contains("PARTSTAT"), "{all}");
    }

    /// A guest's REPLY sets their answer, once; an older one, another
    /// event's, or one from someone not asked changes nothing.
    #[test]
    fn a_guest_s_reply_sets_their_answer() {
        let invited = un(&with_guests(&event(""), &me(), &[guest("anne@example.net", "Anne")], Sender::Draft).unwrap());
        let reply = |sequence: u32, who: &str, uid: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REPLY\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nSEQUENCE:{sequence}\r\nDTSTAMP:20991101T090000Z\r\nORGANIZER:mailto:me@example.org\r\nATTENDEE;PARTSTAT=ACCEPTED;CN=Anne:mailto:{who}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
        let (answered, who, answer) = replied(&invited, &reply(2, "Anne@example.net", "plan-1")).unwrap();
        assert_eq!((who.address.as_str(), answer.as_str()), ("anne@example.net", "accepted"));
        let answered = un(&answered);
        assert!(answered.contains("PARTSTAT=ACCEPTED") && !answered.contains("RSVP=TRUE") && answered.contains("SCHEDULE-AGENT=CLIENT"), "{answered}");
        assert_eq!(replied(&answered, &reply(2, "anne@example.net", "plan-1")), None, "said already");
        assert_eq!(replied(&invited, &reply(1, "anne@example.net", "plan-1")), None, "an earlier version's answer");
        assert_eq!(replied(&invited, &reply(2, "eve@example.net", "plan-1")), None, "nobody added by an answer");
        assert_eq!(replied(&invited, &reply(2, "anne@example.net", "other")), None);
        assert_eq!(replied(&invited, &request(&invited).unwrap()), None, "no REPLY");
    }
}
