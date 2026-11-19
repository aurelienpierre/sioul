// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An extension's replies read (docs/extensions.md, "The calls"). An agenda
//! it handed back is parsed and counted, every bound applied before anything
//! is built (agendas a reply, bytes and lines an agenda, events an agenda),
//! then written again from what was parsed, through an allow-list (`clean`):
//! names are judged once parsed, as Sioul reads every calendar, never on the
//! extension's lines. It is handed on whole: its files are written by the door of read-only
//! agendas alone, one file per event with the time zones it needs, through
//! the subscribed calendars' writer; nothing here copies a calendar's head
//! into each event. What the agenda could not read is kept from the last
//! copy (`files_after`, which the door applies). Anything that does not read
//! as the interface says is nonsense: the call ends, nothing is taken in.

use crate::agenda;
use calcard::icalendar::{ICalendar, ICalendarComponent, ICalendarComponentType, ICalendarEntry, ICalendarParameterName, ICalendarProperty, ICalendarValue};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

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

/// An agenda handed back, read, counted and written again; its files are the door's to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaReply {
    pub id: String,
    pub name: String,
    /// Its calendar written again from what was parsed (`clean`): its events,
    /// with the properties of the allow-list alone, and the time zones they
    /// name, each once.
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
/// The most the time zones an agenda names weigh, written: a zone is a few
/// hundred bytes (an extension that writes times in UTC needs none).
pub const MOST_ZONE_BYTES: usize = 64 << 10;
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
        let (calendar, events) = clean(calendar)?;
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
        agendas.push(AgendaReply { id: id.to_string(), name, calendar, events, unread });
    }
    Ok(RefreshReply { agendas, problem })
}

/// What an extension's event keeps (X3, X4): its identifier, start, end or
/// duration, title, place, description, status, busy or free, categories,
/// its stamps and sequence. Dropped in this version: guests, organizer,
/// alarms, every `X-` line (Sioul's own included: a reminder, the time kept
/// around, costs, an area, ties to your things are yours, never an
/// extension's), repeat rules, links, attachments, conference lines, class.
const KEPT: [ICalendarProperty; 14] = [
    ICalendarProperty::Uid,
    ICalendarProperty::Dtstart,
    ICalendarProperty::Dtend,
    ICalendarProperty::Duration,
    ICalendarProperty::Summary,
    ICalendarProperty::Location,
    ICalendarProperty::Description,
    ICalendarProperty::Status,
    ICalendarProperty::Transp,
    ICalendarProperty::Categories,
    ICalendarProperty::Dtstamp,
    ICalendarProperty::Created,
    ICalendarProperty::LastModified,
    ICalendarProperty::Sequence,
];

/// What a time zone's own parts (its standard and daylight times) keep.
const ZONE_KEPT: [ICalendarProperty; 6] = [ICalendarProperty::Dtstart, ICalendarProperty::Tzoffsetfrom, ICalendarProperty::Tzoffsetto, ICalendarProperty::Tzname, ICalendarProperty::Rrule, ICalendarProperty::Rdate];

/// A text value as Sioul shows it: no control character (a new line kept in
/// a description alone), no mark that turns a line around.
fn plain_value(text: &str, lines: bool) -> String {
    let direction = |c: char| matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
    text.chars().filter(|c| !(c.is_control() && !(lines && *c == '\n')) && !direction(*c)).collect()
}

/// A line of an event or a zone, kept when its name is in `kept`: with its
/// time zone and its value's type alone among its parameters (no link, no
/// `X-` parameter), its texts plain.
fn kept_entry(entry: &ICalendarEntry, kept: &[ICalendarProperty]) -> Option<ICalendarEntry> {
    if !kept.contains(&entry.name) {
        return None;
    }
    let params = entry.params.iter().filter(|p| matches!(p.name, ICalendarParameterName::Tzid | ICalendarParameterName::Value)).cloned().collect();
    let lines = entry.name == ICalendarProperty::Description;
    let values = entry
        .values
        .iter()
        .map(|value| match value {
            ICalendarValue::Text(text) => ICalendarValue::Text(plain_value(text, lines)),
            other => other.clone(),
        })
        .collect();
    Some(ICalendarEntry { name: entry.name.clone(), params, values })
}

/// An agenda's calendar read as Sioul reads every calendar, its events
/// counted (no more than an agenda holds), then written again from what was
/// parsed: each event at the calendar's top with the lines of `KEPT` alone
/// and no component inside it; the time zones the kept events name, once
/// each, with their standard and daylight times alone; nothing else (an
/// override of a series, a task, a zone no event names). Each event has a
/// UID of its own and a start. The text, and how many events it holds.
pub fn clean(calendar: &str) -> Result<(String, usize), Nonsense> {
    let Some(read) = agenda::parse(calendar) else { return nonsense("a calendar that is not one") };
    let Some(root) = read.components.first().filter(|c| c.component_type == ICalendarComponentType::VCalendar) else { return nonsense("a calendar that is not one") };
    let top: Vec<&ICalendarComponent> = root.component_ids.iter().filter_map(|i| read.components.get(*i as usize)).collect();
    let listed = top.iter().filter(|c| c.component_type == ICalendarComponentType::VEvent).count();
    if listed > MOST_EVENTS {
        return nonsense("more events than an agenda holds");
    }
    let head = vec![
        ICalendarEntry { name: ICalendarProperty::Version, params: Vec::new(), values: vec![ICalendarValue::Text("2.0".into())] },
        ICalendarEntry { name: ICalendarProperty::Prodid, params: Vec::new(), values: vec![ICalendarValue::Text("-//Sioul//Extension//EN".into())] },
    ];
    let mut out = ICalendar { components: vec![ICalendarComponent { component_type: ICalendarComponentType::VCalendar, entries: head, component_ids: Vec::new() }] };
    let (mut events, mut uids, mut named) = (Vec::new(), BTreeSet::new(), BTreeSet::new());
    for event in top.iter().filter(|c| c.component_type == ICalendarComponentType::VEvent) {
        // An override of a series: no series in this version.
        if event.property(&ICalendarProperty::RecurrenceId).is_some() {
            continue;
        }
        let entries: Vec<ICalendarEntry> = event.entries.iter().filter_map(|e| kept_entry(e, &KEPT)).collect();
        let uid = entries.iter().find(|e| e.name == ICalendarProperty::Uid).and_then(|e| e.values.first()).and_then(|v| v.as_text()).map(str::trim).unwrap_or("");
        let starts = entries.iter().any(|e| e.name == ICalendarProperty::Dtstart && matches!(e.values.first(), Some(ICalendarValue::PartialDateTime(_))));
        if uid.is_empty() || uid.len() > 255 || !starts {
            return nonsense("an event without its UID or its start");
        }
        if !uids.insert(uid.to_string()) {
            return nonsense("an event's UID twice");
        }
        named.extend(entries.iter().filter_map(|e| e.tz_id().map(str::to_string)));
        events.push(ICalendarComponent { component_type: ICalendarComponentType::VEvent, entries, component_ids: Vec::new() });
    }
    // The zones the events name, each once, before the events.
    let mut zone_bytes = 0usize;
    let mut written = BTreeSet::new();
    for zone in top.iter().filter(|c| c.component_type == ICalendarComponentType::VTimezone) {
        let Some(tzid) = zone.property(&ICalendarProperty::Tzid).and_then(|e| e.values.first()).and_then(|v| v.as_text()).map(|t| plain_value(t, false)) else { continue };
        if !named.contains(&tzid) || !written.insert(tzid.clone()) {
            continue;
        }
        let at = out.components.len();
        out.components.push(ICalendarComponent {
            component_type: ICalendarComponentType::VTimezone,
            entries: vec![ICalendarEntry { name: ICalendarProperty::Tzid, params: Vec::new(), values: vec![ICalendarValue::Text(tzid)] }],
            component_ids: Vec::new(),
        });
        let mut parts = Vec::new();
        for part in zone.component_ids.iter().filter_map(|i| read.components.get(*i as usize)) {
            if matches!(part.component_type, ICalendarComponentType::Standard | ICalendarComponentType::Daylight) {
                parts.push(out.components.len() as u32);
                let entries = part.entries.iter().filter_map(|e| kept_entry(e, &ZONE_KEPT)).collect();
                out.components.push(ICalendarComponent { component_type: part.component_type.clone(), entries, component_ids: Vec::new() });
            }
        }
        out.components[at].component_ids = parts;
        out.components[0].component_ids.push(at as u32);
        zone_bytes += out.components[at..].iter().map(|c| c.entries.iter().map(|e| e.size()).sum::<usize>()).sum::<usize>();
        if zone_bytes > MOST_ZONE_BYTES {
            return nonsense("time zones larger than an agenda needs");
        }
    }
    let count = events.len();
    for event in events {
        let at = out.components.len() as u32;
        out.components[0].component_ids.push(at);
        out.components.push(event);
    }
    Ok((out.to_string(), count))
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
    fn an_agenda_is_written_again_from_what_was_parsed() {
        let alarm = "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nATTENDEE;CN=\"A: B\":mailto:a@example.org\r\nORGANIZER:mailto:\r\n b@example.org\r\nURL:https://elsewhere.example.org/\r\nRRULE:FREQ=DAILY\r\nCLASS:PRIVATE\r\n";
        let text = calendar(&format!("{}{}", event("L1", "20261119T080000Z", "STATUS:CANCELLED\r\nLOCATION;ALTREP=\"https://elsewhere.example.org/\":Room 2\r\n"), event("L2", "20261120T080000Z", alarm)));
        let (kept, events) = clean(&text).unwrap();
        assert_eq!(events, 2);
        let read = agenda::parse(&kept).unwrap();
        assert_eq!(read.components.iter().filter(|c| c.component_type == ICalendarComponentType::VEvent).count(), 2);
        for gone in ["VALARM", "ATTENDEE", "b@example.org", "elsewhere.example.org", "RRULE", "CLASS", "ALTREP"] {
            assert!(!kept.contains(gone), "{gone} came through:\n{kept}");
        }
        assert!(kept.contains("STATUS:CANCELLED\r\n") && kept.contains("LOCATION:Room 2\r\n"), "{kept}");
        // A text without control characters or marks that turn a line around.
        let (kept, _) = clean(&calendar(&event("L1", "20261119T080000Z", "DESCRIPTION:a\\nb\u{202E}c\u{7}d\r\n"))).unwrap();
        assert!(kept.contains("DESCRIPTION:a\\nbcd\r\n"), "{kept}");
        // A task at the top, an override of a series: left out, not taken for events.
        let (kept, events) = clean(&calendar(&format!("BEGIN:VTODO\r\nUID:t\r\nEND:VTODO\r\n{}{}", event("L1", "20261119T080000Z", ""), event("L1", "20261120T080000Z", "RECURRENCE-ID:20261120T080000Z\r\n")))).unwrap();
        assert_eq!(events, 1);
        assert!(!kept.contains("VTODO") && !kept.contains("RECURRENCE-ID"), "{kept}");
        for wrong in [calendar("BEGIN:VEVENT\r\nDTSTART:20261119T080000Z\r\nEND:VEVENT\r\n"), calendar("BEGIN:VEVENT\r\nUID:x\r\nEND:VEVENT\r\n"), calendar(&format!("{}{}", event("L1", "20261119T080000Z", ""), event("L1", "20261121T080000Z", ""))), "not a calendar".to_string()] {
            assert!(clean(&wrong).is_err(), "{wrong}");
        }
    }

    /// The time zones an agenda's events name, once each, with their standard and daylight times alone.
    #[test]
    fn a_time_zone_named_is_written_once_and_alone() {
        let paris = "BEGIN:VTIMEZONE\r\nTZID:Europe/Paris\r\nX-SIOUL-REMIND:30\r\nBEGIN:STANDARD\r\nDTSTART:19701025T030000\r\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\nTZOFFSETFROM:+0200\r\nTZOFFSETTO:+0100\r\nTZNAME:CET\r\nATTENDEE:mailto:a@example.org\r\nEND:STANDARD\r\nBEGIN:DAYLIGHT\r\nDTSTART:19700329T020000\r\nRRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0200\r\nTZNAME:CEST\r\nEND:DAYLIGHT\r\nEND:VTIMEZONE\r\n";
        let other = "BEGIN:VTIMEZONE\r\nTZID:Other/Zone\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\nTZOFFSETFROM:+0000\r\nTZOFFSETTO:+0000\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n";
        let events: String = (0..50).map(|i| format!("BEGIN:VEVENT\r\nUID:L{i}\r\nDTSTART;TZID=Europe/Paris:20261119T090000\r\nDTEND;TZID=Europe/Paris:20261119T100000\r\nEND:VEVENT\r\n")).collect();
        let (kept, events) = clean(&calendar(&format!("{paris}{other}{paris}{events}"))).unwrap();
        assert_eq!(events, 50);
        assert_eq!(kept.matches("BEGIN:VTIMEZONE").count(), 1, "once, whatever the number of events:\n{kept}");
        assert!(!kept.contains("Other/Zone") && !kept.contains("X-SIOUL") && !kept.contains("ATTENDEE"), "{kept}");
        assert!(kept.contains("TZOFFSETTO:+0200") && kept.contains("TZNAME:CET"), "{kept}");
        assert_eq!(agenda::first_times(&kept, &TimeZone::UTC), Some((1_795_075_200, 1_795_078_800)), "read in its zone, as written:\n{kept}");
        // A zone of more than an agenda needs: refused.
        let filler: String = (0..2000).map(|i| format!("TZNAME:{i}-{}\r\n", "z".repeat(40))).collect();
        let big = format!("BEGIN:VTIMEZONE\r\nTZID:Big\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\nTZOFFSETFROM:+0000\r\nTZOFFSETTO:+0000\r\n{filler}END:STANDARD\r\nEND:VTIMEZONE\r\n");
        assert!(clean(&calendar(&format!("{big}BEGIN:VEVENT\r\nUID:x\r\nDTSTART;TZID=Big:20261119T090000\r\nEND:VEVENT\r\n"))).is_err());
    }

    /// X3, X4 (the review's scratch test, through `clean`): what the
    /// allow-list leaves out does not come back another way. Each line passed
    /// the line filter before, and calcard, which reads the file everywhere
    /// else in Sioul, took it for what the filter meant to drop: it unfolds a
    /// name written on two lines, skips a blank before the colon and quotes
    /// around a name, and takes `\X` for `X` (calcard-0.3.14,
    /// src/common/tokenizer.rs, `token`).
    #[test]
    fn what_the_filter_drops_does_not_come_back_another_way() {
        use calcard::icalendar::ICalendarProperty as P;
        let read = |extra: &str| agenda::parse(&clean(&calendar(&event("L1", "20261119T080000Z", extra))).unwrap().0).unwrap();
        let guests = [
            ("a name folded on two lines", "ATT\r\n ENDEE:mailto:a@example.org\r\n"),
            ("a blank before the colon", "ATTENDEE :mailto:a@example.org\r\n"),
            ("quotes around the name", "\"ATTENDEE\":mailto:a@example.org\r\n"),
            ("a backslash inside the name", "ATT\\ENDEE:mailto:a@example.org\r\n"),
        ];
        for (what, extra) in guests {
            assert!(read(extra).components.iter().all(|c| c.property(&P::Attendee).is_none()), "a guest came through: {what}");
        }
        // ("ORG\ANIZER", not "ORGA\NIZER": calcard takes `\N` for a new line.)
        for (what, organizer) in [("folded", "ORGAN\r\n IZER:mailto:o@example.org\r\n"), ("a backslash", "ORG\\ANIZER:mailto:o@example.org\r\n")] {
            assert!(read(organizer).components.iter().all(|c| c.property(&P::Organizer).is_none()), "an organizer came through: {what}");
        }
        let alarms = [
            ("BEGIN and END folded", "BEG\r\n IN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT10M\r\nEN\r\n D:VALARM\r\n"),
            ("a backslash inside VALARM", "BEGIN:VAL\\ARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT10M\r\nEND:VAL\\ARM\r\n"),
        ];
        for (what, extra) in alarms {
            assert!(read(extra).components.iter().all(|c| c.component_type != ICalendarComponentType::VAlarm), "an alarm came through: {what}");
        }
        // With no trick at all: a whole event, its guest and its alarm, inside a time zone.
        let zone = "BEGIN:VTIMEZONE\r\nTZID:X\r\nBEGIN:VEVENT\r\nUID:hidden\r\nDTSTART:20261119T030000Z\r\nATTENDEE:mailto:a@example.org\r\nORGANIZER:mailto:o@example.org\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT10M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VTIMEZONE\r\n";
        let named = "BEGIN:VEVENT\r\nUID:L1\r\nDTSTART;TZID=X:20261119T080000\r\nEND:VEVENT\r\n";
        let read_zone = agenda::parse(&clean(&calendar(&format!("{zone}{named}"))).unwrap().0).unwrap();
        assert_eq!(read_zone.components.iter().filter(|c| c.component_type == ICalendarComponentType::VEvent).count(), 1, "an event came through inside a time zone");
        assert!(read_zone.components.iter().all(|c| c.component_type != ICalendarComponentType::VAlarm && c.property(&P::Attendee).is_none()));
        // A task inside an event (only the calendar's top level was checked).
        let task = read("BEGIN:VTODO\r\nUID:t\r\nSUMMARY:A task\r\nEND:VTODO\r\n");
        assert!(task.components.iter().all(|c| c.component_type != ICalendarComponentType::VTodo), "a task came through inside an event");
        // X4: Sioul's own properties are the person's, never an extension's.
        for own in ["X-SIOUL-REMIND:30", "X-SIOUL-BEFORE:PT30M", "X-SIOUL-AFTER:PT30M", "X-SIOUL-COST:9;9;9;9", "X-SIOUL-GAIN:9", "X-SIOUL-AREA:work", "X-SIOUL-LINK;LINKREL=related;VALUE=URI:sioul:note/admin/form.md", "X-SIOUL-BILLABLE:1", "X-\r\n SIOUL-REMIND:30"] {
            let (kept, _) = clean(&calendar(&event("L1", "20261119T080000Z", &format!("{own}\r\n")))).unwrap();
            assert!(!kept.to_ascii_uppercase().contains("X-SIOUL-") && !kept.contains("SIOUL-REMIND"), "{own} came through");
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
