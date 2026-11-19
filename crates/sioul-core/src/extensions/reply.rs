// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An extension's replies read (docs/extensions.md, "The calls"), and an
//! agenda it handed back made the files Sioul keeps: one file per event
//! (`<hash of its UID>.ics`, the same name on every device), what it could
//! not read kept from the last copy. Anything that does not read as the
//! interface says is nonsense: the call ends, nothing is taken in.

use crate::agenda;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Why a reply was not taken: said in the log, the call ended with one sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nonsense(pub String);

fn nonsense<T>(what: impl Into<String>) -> Result<T, Nonsense> {
    Err(Nonsense(what.into()))
}

/// What a problem is, for Sioul to say it in its own words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemKind {
    SignIn,
    Unreachable,
    Refused,
    Changed,
}

impl ProblemKind {
    pub fn word(self) -> &'static str {
        match self {
            Self::SignIn => "sign-in",
            Self::Unreachable => "unreachable",
            Self::Refused => "refused",
            Self::Changed => "changed",
        }
    }
}

/// A problem: its kind, and its detail for the log alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub kind: ProblemKind,
    pub detail: String,
}

/// The reply to a `sign-in` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInReply {
    SignedIn,
    /// Ids and names to choose among.
    Choose(Vec<(String, String)>),
    Cancelled,
    Problem(Problem),
}

/// One event of an agenda, as the file Sioul keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventFile {
    pub uid: String,
    /// `<32 hexadecimal digits of the UID's SHA-256>.ics`.
    pub name: String,
    pub text: String,
}

/// An agenda handed back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaReply {
    pub id: String,
    pub name: String,
    pub events: Vec<EventFile>,
    /// Periods not read this time (seconds since 1970, from and until).
    pub unread: Vec<(i64, i64)>,
}

/// The reply to a `refresh` call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshReply {
    pub agendas: Vec<AgendaReply>,
    pub problem: Option<Problem>,
}

/// The most events an agenda holds.
pub const MOST_EVENTS: usize = 20_000;
const MOST_CHOICES: usize = 200;
const MOST_PERIODS: usize = 400;

fn parse(text: &str) -> Result<serde_json::Map<String, Value>, Nonsense> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => nonsense("a reply that is not a JSON object"),
    }
}

/// A plain text of at most `most` characters, no control character; none when empty.
fn plain(value: &Value, most: usize) -> Option<String> {
    let text: String = value.as_str()?.chars().filter(|c| !c.is_control()).collect();
    let text = text.trim();
    (!text.is_empty() && text.chars().count() <= most).then(|| text.to_string())
}

fn problem(value: &Value) -> Result<Problem, Nonsense> {
    let kind = match value["kind"].as_str() {
        Some("sign-in") => ProblemKind::SignIn,
        Some("unreachable") => ProblemKind::Unreachable,
        Some("refused") => ProblemKind::Refused,
        Some("changed") => ProblemKind::Changed,
        _ => return nonsense("a problem of no known kind"),
    };
    let detail: String = value["detail"].as_str().unwrap_or("").chars().filter(|c| !c.is_control()).take(300).collect();
    Ok(Problem { kind, detail })
}

/// A `sign-in` call's reply.
pub fn sign_in_reply(text: &str) -> Result<SignInReply, Nonsense> {
    let reply = parse(text)?;
    if reply.get("signed_in") == Some(&Value::Bool(true)) {
        return Ok(SignInReply::SignedIn);
    }
    if reply.get("cancelled") == Some(&Value::Bool(true)) {
        return Ok(SignInReply::Cancelled);
    }
    if let Some(found) = reply.get("problem") {
        return Ok(SignInReply::Problem(problem(found)?));
    }
    let Some(Value::Array(choices)) = reply.get("choose") else { return nonsense("a sign-in's reply of no known form") };
    if choices.is_empty() || choices.len() > MOST_CHOICES {
        return nonsense("no choice, or too many");
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for choice in choices {
        let (Some(id), Some(name)) = (plain(&choice["id"], 64), plain(&choice["name"], 120)) else { return nonsense("a choice without its id or name") };
        if out.iter().any(|(i, _)| *i == id) {
            return nonsense("a choice offered twice");
        }
        out.push((id, name));
    }
    Ok(SignInReply::Choose(out))
}

/// An instant as the interface writes it (RFC 3339).
fn instant(value: &Value) -> Option<i64> {
    value.as_str()?.parse::<Timestamp>().ok().map(Timestamp::as_second)
}

/// A `refresh` call's reply, each agenda checked and made files.
pub fn refresh_reply(text: &str) -> Result<RefreshReply, Nonsense> {
    let reply = parse(text)?;
    let problem = match reply.get("problem") {
        None | Some(Value::Null) => None,
        Some(found) => Some(problem(found)?),
    };
    let mut agendas: Vec<AgendaReply> = Vec::new();
    let listed = match reply.get("agendas") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(all)) => all.clone(),
        Some(_) => return nonsense("agendas that are not a list"),
    };
    for agenda in &listed {
        let id = agenda["id"].as_str().unwrap_or("");
        if id.is_empty() || id.len() > 48 || id.starts_with('-') || !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return nonsense("an agenda's id that is not one");
        }
        if agendas.iter().any(|a| a.id == id) {
            return nonsense("an agenda handed twice");
        }
        let Some(name) = plain(&agenda["name"], 80) else { return nonsense("an agenda without a name") };
        let Some(calendar) = agenda["calendar"].as_str() else { return nonsense("an agenda without its calendar") };
        let events = split(calendar)?;
        let mut unread = Vec::new();
        for period in agenda["unread"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let (Some(from), Some(until)) = (instant(&period["from"]), instant(&period["until"])) else { return nonsense("an unread period that does not read") };
            if until <= from || until - from > 31 * 86_400 {
                return nonsense("an unread period out of bounds");
            }
            unread.push((from, until));
        }
        if unread.len() > MOST_PERIODS {
            return nonsense("too many unread periods");
        }
        agendas.push(AgendaReply { id: id.to_string(), name, events, unread });
    }
    Ok(RefreshReply { agendas, problem })
}

/// The content lines of a text, each with its folded continuations: (its
/// name in upper case, its physical lines).
fn logical_lines(text: &str) -> Vec<(String, Vec<&str>)> {
    let mut lines: Vec<(String, Vec<&str>)> = Vec::new();
    for line in text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) {
        if (line.starts_with(' ') || line.starts_with('\t')) && let Some(last) = lines.last_mut() {
            last.1.push(line);
        } else if !line.is_empty() {
            let name = line.split([':', ';']).next().unwrap_or("").to_ascii_uppercase();
            lines.push((name, vec![line]));
        }
    }
    lines
}

/// The value of a logical line, unfolded.
fn value_of(physical: &[&str]) -> String {
    let joined: String = physical.iter().enumerate().map(|(i, l)| if i == 0 { *l } else { &l[1..] }).collect();
    // The value follows the first `:` that is not inside a quoted parameter.
    let mut quoted = false;
    for (i, c) in joined.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => return joined[i + 1..].to_string(),
            _ => {}
        }
    }
    String::new()
}

/// What is dropped from an extension's event: who takes part (an agenda an
/// extension brings is no meeting), and alarms (reminders are the person's own).
const DROPPED: [&str; 2] = ["ATTENDEE", "ORGANIZER"];

/// An agenda's calendar made one file per UID (its events, its overrides),
/// each with the calendar's time zones: events only, each with a UID and a start.
pub fn split(calendar: &str) -> Result<Vec<EventFile>, Nonsense> {
    fn begins(l: &(String, Vec<&str>), what: &str) -> bool {
        l.0 == "BEGIN" && value_of(&l.1).eq_ignore_ascii_case(what)
    }
    fn ends(l: &(String, Vec<&str>), what: &str) -> bool {
        l.0 == "END" && value_of(&l.1).eq_ignore_ascii_case(what)
    }
    let lines = logical_lines(calendar);
    if lines.first().is_none_or(|l| !begins(l, "VCALENDAR")) || lines.last().is_none_or(|l| !ends(l, "VCALENDAR")) {
        return nonsense("a calendar that is not one");
    }
    let mut zones: Vec<&str> = Vec::new();
    let mut by_uid: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut count = 0usize;
    let mut i = 1;
    while i + 1 < lines.len() {
        let line = &lines[i];
        if line.0 == "BEGIN" {
            let what = value_of(&line.1).to_ascii_uppercase();
            let Some(end) = (i + 1..lines.len()).find(|&j| ends(&lines[j], &what)) else { return nonsense("a component that does not end") };
            let block = &lines[i..=end];
            match what.as_str() {
                "VTIMEZONE" => zones.extend(block.iter().flat_map(|l| l.1.iter().copied())),
                "VEVENT" => {
                    count += 1;
                    if count > MOST_EVENTS {
                        return nonsense("more events than an agenda holds");
                    }
                    let uid = block.iter().find(|l| l.0 == "UID").map(|l| value_of(&l.1)).unwrap_or_default();
                    if uid.trim().is_empty() || uid.len() > 255 || !block.iter().any(|l| l.0 == "DTSTART") {
                        return nonsense("an event without its UID or its start");
                    }
                    let kept = by_uid.entry(uid).or_default();
                    let mut alarm = false;
                    for l in block {
                        if begins(l, "VALARM") {
                            alarm = true;
                        }
                        if !alarm && !DROPPED.contains(&l.0.as_str()) {
                            kept.extend(l.1.iter().copied());
                        }
                        if ends(l, "VALARM") {
                            alarm = false;
                        }
                    }
                }
                _ => return nonsense(format!("a calendar that holds a {what}, not events alone")),
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    let head = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Sioul//Extension//EN\r\n";
    let mut files = Vec::with_capacity(by_uid.len());
    for (uid, lines) in by_uid {
        let mut text = String::from(head);
        for line in zones.iter().chain(lines.iter()) {
            text.push_str(line);
            text.push_str("\r\n");
        }
        text.push_str("END:VCALENDAR\r\n");
        if agenda::parse(&text).is_none() || agenda::first_times(&text, &TimeZone::UTC).is_none() {
            return nonsense("an event that does not read");
        }
        files.push(EventFile { name: file_name(&uid), uid, text });
    }
    Ok(files)
}

/// The file of an event by its UID, the same on every device.
pub fn file_name(uid: &str) -> String {
    let digest = Sha256::digest(uid.as_bytes());
    format!("{}.ics", digest.iter().take(16).map(|b| format!("{b:02x}")).collect::<String>())
}

/// The files an agenda holds once `new` is taken in: every new event, and
/// each event of the last copy (`old`, by file name) that the new ones do not
/// replace and that overlaps a period not read this time.
pub fn files_after(new: &[EventFile], old: &BTreeMap<String, String>, unread: &[(i64, i64)]) -> BTreeMap<String, String> {
    let mut files: BTreeMap<String, String> = new.iter().map(|e| (e.name.clone(), e.text.clone())).collect();
    for (name, text) in old {
        if files.contains_key(name) {
            continue;
        }
        if let Some((start, end)) = agenda::first_times(text, &TimeZone::UTC)
            && unread.iter().any(|(from, until)| start < *until && end.max(start + 1) > *from)
        {
            files.insert(name.clone(), text.clone());
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calendar(events: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//t//EN\r\n{events}END:VCALENDAR\r\n")
    }

    fn event(uid: &str, start: &str, extra: &str) -> String {
        format!("BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:{start}\r\nDTSTART:{start}\r\nDTEND:{start}\r\nSUMMARY:Lesson\r\n{extra}END:VEVENT\r\n")
    }

    #[test]
    fn replies_read_or_are_nonsense() {
        assert_eq!(sign_in_reply(r#"{"signed_in": true}"#), Ok(SignInReply::SignedIn));
        assert_eq!(sign_in_reply(r#"{"cancelled": true}"#), Ok(SignInReply::Cancelled));
        assert_eq!(sign_in_reply(r#"{"choose": [{"id": "a", "name": "A"}]}"#), Ok(SignInReply::Choose(vec![("a".into(), "A".into())])));
        assert!(matches!(sign_in_reply(r#"{"problem": {"kind": "sign-in", "detail": "x"}}"#), Ok(SignInReply::Problem(Problem { kind: ProblemKind::SignIn, .. }))));
        for wrong in ["", "[]", "{}", r#"{"choose": []}"#, r#"{"choose": [{"id": "a"}]}"#, r#"{"choose": [{"id": "a", "name": "A"}, {"id": "a", "name": "B"}]}"#, r#"{"problem": {"kind": "fire"}}"#] {
            assert!(sign_in_reply(wrong).is_err(), "{wrong}");
        }
        let good = serde_json::json!({ "agendas": [{ "id": "p-1", "name": "Timetable: Alex", "calendar": calendar(&event("L1", "20261119T080000Z", "")), "unread": [{ "from": "2026-11-19T23:00:00Z", "until": "2026-11-20T23:00:00Z" }] }], "problem": null });
        let read = refresh_reply(&good.to_string()).unwrap();
        assert_eq!(read.agendas[0].unread, vec![(1_795_129_200, 1_795_215_600)]);
        assert_eq!(read.agendas[0].events.len(), 1);
        for (field, value) in [("id", serde_json::json!("P 1")), ("id", serde_json::json!("")), ("name", serde_json::json!("")), ("calendar", serde_json::json!("BEGIN:VEVENT")), ("unread", serde_json::json!([{ "from": "2026-11-20T00:00:00Z", "until": "2026-11-19T00:00:00Z" }]))] {
            let mut wrong = good.clone();
            wrong["agendas"][0][field] = value.clone();
            assert!(refresh_reply(&wrong.to_string()).is_err(), "{field}: {value}");
        }
        let twice = serde_json::json!({ "agendas": [good["agendas"][0].clone(), good["agendas"][0].clone()] });
        assert!(refresh_reply(&twice.to_string()).is_err());
        assert_eq!(refresh_reply(r#"{"agendas": [], "problem": {"kind": "unreachable", "detail": "down"}}"#).unwrap().problem.map(|p| p.kind), Some(ProblemKind::Unreachable));
    }

    #[test]
    fn an_agenda_is_events_alone_one_file_each() {
        let alarm = "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nATTENDEE;CN=\"A: B\":mailto:a@example.org\r\nORGANIZER:mailto:\r\n b@example.org\r\n";
        let text = calendar(&format!("{}{}", event("L1", "20261119T080000Z", "STATUS:CANCELLED\r\n"), event("L2", "20261120T080000Z", alarm)));
        let files = split(&text).unwrap();
        assert_eq!(files.len(), 2);
        let second = files.iter().find(|f| f.uid == "L2").unwrap();
        assert!(!second.text.contains("VALARM") && !second.text.contains("ATTENDEE") && !second.text.contains("b@example.org"), "{}", second.text);
        assert_eq!(second.name, file_name("L2"));
        assert_eq!(file_name("L2").len(), 36);
        assert!(files.iter().find(|f| f.uid == "L1").unwrap().text.contains("STATUS:CANCELLED\r\n"));
        for wrong in [calendar("BEGIN:VTODO\r\nUID:x\r\nEND:VTODO\r\n"), calendar("BEGIN:VEVENT\r\nDTSTART:20261119T080000Z\r\nEND:VEVENT\r\n"), calendar("BEGIN:VEVENT\r\nUID:x\r\nEND:VEVENT\r\n"), "not a calendar".to_string()] {
            assert!(split(&wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn what_was_not_read_stays_from_the_last_copy() {
        let old_files = split(&calendar(&format!("{}{}{}", event("A", "20261119T080000Z", ""), event("B", "20261120T080000Z", ""), event("C", "20261121T080000Z", "")))).unwrap();
        let old: BTreeMap<String, String> = old_files.iter().map(|f| (f.name.clone(), f.text.clone())).collect();
        // The new copy: A moved, B's day unread, C gone.
        let new = split(&calendar(&event("A", "20261119T090000Z", ""))).unwrap();
        let unread = [(1_795_129_200, 1_795_215_600)];
        let after = files_after(&new, &old, &unread);
        assert_eq!(after.len(), 2);
        assert!(after[&file_name("A")].contains("DTSTART:20261119T090000Z"), "the new A");
        assert!(after.contains_key(&file_name("B")), "B, on the day not read, stays");
        assert!(!after.contains_key(&file_name("C")), "C, on a day read, goes");
        assert_eq!(files_after(&new, &old, &[]).len(), 1);
    }
}
