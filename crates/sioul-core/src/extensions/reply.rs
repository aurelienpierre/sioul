// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An extension's replies read (docs/extensions.md, "The calls"). An agenda
//! it handed back is parsed and counted, every bound applied before anything
//! is built (agendas a reply, bytes and lines an agenda, events an agenda),
//! and handed on whole: its files are written by the door of read-only
//! agendas alone, one file per event with the time zones it needs, through
//! the subscribed calendars' writer; nothing here copies a calendar's head
//! into each event. What the agenda could not read is kept from the last
//! copy (`files_after`, which the door applies). Anything that does not read
//! as the interface says is nonsense: the call ends, nothing is taken in.

use crate::agenda;
use calcard::icalendar::ICalendarComponentType;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde_json::Value;
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

/// An agenda handed back, read and counted; its files are the door's to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaReply {
    pub id: String,
    pub name: String,
    /// Its calendar, one text: its events and their time zones, each once.
    pub calendar: String,
    /// How many events it holds.
    pub events: usize,
    /// Periods not read this time (seconds since 1970, from and until).
    pub unread: Vec<(i64, i64)>,
}

/// The reply to a `refresh` call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshReply {
    pub agendas: Vec<AgendaReply>,
    pub problem: Option<Problem>,
}

/// The most agendas a reply names.
pub const MOST_AGENDAS: usize = 32;
/// The most events an agenda holds.
pub const MOST_EVENTS: usize = 20_000;
/// The most an agenda's calendar weighs, and its most lines: counted on the
/// text before it is parsed, so that the parser's tree stays bounded too.
pub const MOST_BYTES: usize = 8 << 20;
pub const MOST_LINES: usize = 250_000;
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
    let listed: &[Value] = match reply.get("agendas") {
        None | Some(Value::Null) => &[],
        Some(Value::Array(all)) => all,
        Some(_) => return nonsense("agendas that are not a list"),
    };
    // Counted before anything is read further.
    if listed.len() > MOST_AGENDAS {
        return nonsense("more agendas than a reply holds");
    }
    for agenda in listed {
        let id = agenda["id"].as_str().unwrap_or("");
        if id.is_empty() || id.len() > 48 || id.starts_with('-') || !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return nonsense("an agenda's id that is not one");
        }
        if agendas.iter().any(|a| a.id == id) {
            return nonsense("an agenda handed twice");
        }
        let Some(name) = plain(&agenda["name"], 80) else { return nonsense("an agenda without a name") };
        let Some(calendar) = agenda["calendar"].as_str() else { return nonsense("an agenda without its calendar") };
        if calendar.len() > MOST_BYTES || calendar.bytes().filter(|b| *b == b'\n').count() > MOST_LINES {
            return nonsense("an agenda larger than an agenda may be");
        }
        let events = count(calendar)?;
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
        agendas.push(AgendaReply { id: id.to_string(), name, calendar: calendar.to_string(), events, unread });
    }
    Ok(RefreshReply { agendas, problem })
}

/// An agenda's calendar read as Sioul reads every calendar, and its events
/// counted: events only at its top (each with a UID and a start), and no more
/// than an agenda holds. Nothing is built from it here.
pub fn count(calendar: &str) -> Result<usize, Nonsense> {
    let Some(read) = agenda::parse(calendar) else { return nonsense("a calendar that is not one") };
    let Some(root) = read.components.first().filter(|c| c.component_type == ICalendarComponentType::VCalendar) else { return nonsense("a calendar that is not one") };
    let mut events = 0usize;
    for component in root.component_ids.iter().filter_map(|i| read.components.get(*i as usize)) {
        match component.component_type {
            ICalendarComponentType::VEvent => {
                events += 1;
                if events > MOST_EVENTS {
                    return nonsense("more events than an agenda holds");
                }
                if component.uid().is_none_or(|uid| uid.trim().is_empty() || uid.len() > 255) || component.property(&calcard::icalendar::ICalendarProperty::Dtstart).is_none() {
                    return nonsense("an event without its UID or its start");
                }
            }
            ICalendarComponentType::VTimezone => {}
            _ => return nonsense("a calendar that holds more than events"),
        }
    }
    Ok(events)
}

/// The files an agenda holds once `new` (by file name, as the door's writer
/// made them) is taken in: every new event, and each event of the last copy
/// (`old`, by file name) that the new ones do not replace and that overlaps a
/// period not read this time. For the door to apply.
pub fn files_after(new: &BTreeMap<String, String>, old: &BTreeMap<String, String>, unread: &[(i64, i64)]) -> BTreeMap<String, String> {
    let mut files = new.clone();
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

    fn one(calendar: &str) -> String {
        serde_json::json!({ "agendas": [{ "id": "a", "name": "A", "calendar": calendar }] }).to_string()
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
        assert_eq!(read.agendas[0].events, 1);
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
    fn an_agenda_is_events_alone_each_with_its_uid_and_start() {
        let text = calendar(&format!("{}{}", event("L1", "20261119T080000Z", "STATUS:CANCELLED\r\n"), event("L2", "20261120T080000Z", "")));
        assert_eq!(count(&text), Ok(2));
        for wrong in [calendar("BEGIN:VTODO\r\nUID:x\r\nEND:VTODO\r\n"), calendar("BEGIN:VEVENT\r\nDTSTART:20261119T080000Z\r\nEND:VEVENT\r\n"), calendar("BEGIN:VEVENT\r\nUID:x\r\nEND:VEVENT\r\n"), "not a calendar".to_string()] {
            assert!(count(&wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn what_was_not_read_stays_from_the_last_copy() {
        let file = |uid: &str, start: &str| (format!("{uid}.ics"), calendar(&event(uid, start, "")));
        let old: BTreeMap<String, String> = [file("A", "20261119T080000Z"), file("B", "20261120T080000Z"), file("C", "20261121T080000Z")].into();
        // The new copy: A moved, B's day unread, C gone.
        let new: BTreeMap<String, String> = [file("A", "20261119T090000Z")].into();
        let unread = [(1_795_129_200, 1_795_215_600)];
        let after = files_after(&new, &old, &unread);
        assert_eq!(after.len(), 2);
        assert!(after["A.ics"].contains("DTSTART:20261119T090000Z"), "the new A");
        assert!(after.contains_key("B.ics"), "B, on the day not read, stays");
        assert!(!after.contains_key("C.ics"), "C, on a day read, goes");
        assert_eq!(files_after(&new, &old, &[]).len(), 1);
    }

    /// X2 (the review's scratch test, through the reply): a calendar's head is
    /// never copied into each event here; what is kept stays near what was handed.
    #[test]
    fn a_time_zone_is_not_copied_into_every_event_without_a_limit() {
        // 64 KiB of time zone, 500 events: 32 MiB of files before, from a calendar of about 100 KiB.
        // (The real case: 6.8 MiB and 20,000 events, about 136 GB, from a reply of 8 MiB.)
        let filler: String = (0..800).map(|i| format!("X-FILL-{i}:{}\r\n", "z".repeat(64))).collect();
        let zone = format!("BEGIN:VTIMEZONE\r\nTZID:X\r\n{filler}END:VTIMEZONE\r\n");
        let events: String = (0..500).map(|i| event(&format!("L{i}"), "20261119T080000Z", "")).collect();
        let text = calendar(&format!("{zone}{events}"));
        match refresh_reply(&one(&text)) {
            // Refusing such a calendar is one right answer.
            Err(_) => {}
            // Keeping it is another, as long as what is kept stays near what was handed.
            Ok(read) => {
                let kept: usize = read.agendas.iter().map(|a| a.calendar.len()).sum();
                assert!(kept <= 4 * text.len(), "{} bytes handed, {kept} kept", text.len());
            }
        }
    }

    #[test]
    fn a_reply_names_a_bounded_number_of_agendas() {
        // No cap before: a reply of 8 MiB could name some 50,000 agendas, each a calendar folder
        // that then travels to every device.
        let one = |i: usize| serde_json::json!({ "id": format!("a-{i}"), "name": "A", "calendar": calendar(&event("L1", "20261119T080000Z", "")) });
        let reply = serde_json::json!({ "agendas": (0..1000).map(one).collect::<Vec<_>>() });
        assert!(refresh_reply(&reply.to_string()).is_err(), "a thousand agendas in one reply");
        let reply = serde_json::json!({ "agendas": (0..MOST_AGENDAS).map(one).collect::<Vec<_>>() });
        assert_eq!(refresh_reply(&reply.to_string()).unwrap().agendas.len(), MOST_AGENDAS);
    }

    /// Bytes and lines are counted on the text before it is parsed; events once parsed, before anything is built.
    #[test]
    fn an_agenda_is_bounded_before_anything_is_built() {
        let long = calendar(&event("L1", "20261119T080000Z", &format!("DESCRIPTION:{}\r\n", "d".repeat(MOST_BYTES))));
        assert!(refresh_reply(&one(&long)).is_err(), "more bytes than an agenda holds");
        let lines = calendar(&event("L1", "20261119T080000Z", &"X-A:1\r\n".repeat(MOST_LINES)));
        assert!(refresh_reply(&one(&lines)).is_err(), "more lines than an agenda holds");
        let many: String = (0..=MOST_EVENTS).map(|i| format!("BEGIN:VEVENT\r\nUID:{i}\r\nDTSTART:20261119T080000Z\r\nEND:VEVENT\r\n")).collect();
        assert!(refresh_reply(&one(&calendar(&many))).is_err(), "more events than an agenda holds");
    }
}
