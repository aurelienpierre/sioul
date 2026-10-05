// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Calendars: events from iCalendar files (RFC 5545), one event and its
//! exceptions per file as CalDAV keeps them (RFC 4791 §4.1).
//!
//! What comes is computed for a stretch of days: repeating events are
//! expanded (RRULE, RDATE, EXDATE and changed occurrences, calcard's
//! `expand_dates`), each in its own time zone, then shown in yours. Nothing is
//! "late" or "overdue": the past is simply not shown (docs/client.md, rule 9).
//!
//! Events are changed line by line (`lines`): what Sioul does not edit (alarms,
//! guests, another application's fields) is kept exactly. New events are
//! written in UTC; a repeating one keeps its local time across the change of
//! hour with its time zone, written out in a VTIMEZONE.

use crate::lines;
use crate::vdir::{self, Collection, Kind};
use calcard::icalendar::dates::TimeOrDelta;
use calcard::icalendar::{ICalendar, ICalendarComponent, ICalendarComponentType, ICalendarParameterName, ICalendarParameterValue, ICalendarParticipationStatus, ICalendarProperty, ICalendarValue};
use calcard::{Entry, Parser};
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan, Zoned};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Occurrences of a repeating event looked at, at most, per file.
const EXPANSION_LIMIT: usize = 20_000;

/// Occurrences looked at to find an event's first one still there: occurrences
/// left out (EXDATE) count against the limit, so one would find none when the
/// first was removed, and the event could no longer be opened.
const FIRST_LOOKED_AT: usize = 100;

/// One occurrence of an event, as the agenda shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Occurrence {
    /// The file, to open and change the event.
    pub key: String,
    pub uid: String,
    pub summary: String,
    pub location: String,
    pub notes: String,
    /// Unix seconds; an all-day event starts at midnight where you are.
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub recurring: bool,
    pub calendar: String,
    pub color: Option<String>,
    pub cancelled: bool,
    pub tentative: bool,
    pub organizer: String,
    pub attendees: Vec<Attendee>,
    pub read_only: bool,
    /// Its alarms (VALARM), in seconds from its start: -900 for a quarter of an hour before.
    pub alarms: Vec<i64>,
    /// Minutes kept before and after it: getting there and back (`demands`).
    pub margins: crate::demands::Margins,
    /// What it costs and gives back, rated 0 to 10 (`demands`).
    pub demands: crate::demands::Demands,
}

/// Someone invited, and what they answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attendee {
    pub name: String,
    pub address: String,
    /// "accepted", "declined", "tentative", "needs-action".
    pub answer: String,
}

/// Parses an iCalendar object.
pub fn parse(text: &str) -> Option<ICalendar> {
    match Parser::new(text).entry() {
        Entry::ICalendar(calendar) => Some(calendar),
        _ => None,
    }
}

/// A zone as calcard knows it: times written without a zone are taken in it.
fn calcard_zone(zone: &TimeZone) -> calcard::common::timezone::Tz {
    zone.iana_name().and_then(|name| name.parse::<chrono_tz::Tz>().ok()).map_or(calcard::common::timezone::Tz::Floating, Into::into)
}

fn text_of(component: &ICalendarComponent, property: &ICalendarProperty) -> String {
    component.property(property).and_then(|e| e.values.first()).and_then(ICalendarValue::as_text).unwrap_or_default().trim().to_string()
}

/// Its margins, costs and gain, from Sioul's own properties (`demands`).
fn demands_of(component: &ICalendarComponent) -> (crate::demands::Margins, crate::demands::Demands) {
    let other = |name: &str| text_of(component, &ICalendarProperty::Other(name.to_string()));
    let margins = crate::demands::Margins { before: crate::demands::minutes_of(&other(crate::demands::BEFORE)), after: crate::demands::minutes_of(&other(crate::demands::AFTER)) };
    let mut demands = crate::demands::Demands::default();
    demands.read_cost(&other(crate::demands::COST));
    let gain = other(crate::demands::GAIN);
    if !gain.is_empty() {
        demands.read_gain(&gain);
    }
    (margins, demands)
}

/// A date without a time: the event lasts whole days.
fn is_all_day(component: &ICalendarComponent) -> bool {
    component.property(&ICalendarProperty::Dtstart).is_some_and(|e| match e.values.first() {
        Some(ICalendarValue::PartialDateTime(date)) => date.hour.is_none(),
        _ => false,
    })
}

/// "mailto:jane@example.org" → "jane@example.org".
fn address(value: &str) -> String {
    let value = value.trim();
    value.get(..7).filter(|p| p.eq_ignore_ascii_case("mailto:")).map_or(value, |_| &value[7..]).to_string()
}

fn people(component: &ICalendarComponent) -> (String, Vec<Attendee>) {
    let person = |e: &calcard::icalendar::ICalendarEntry| {
        let value = e.values.first().and_then(ICalendarValue::as_text).map(address).unwrap_or_default();
        let name = e.params.iter().find(|p| p.name == ICalendarParameterName::Cn).and_then(|p| p.value.as_text()).unwrap_or_default().to_string();
        let answer = e
            .params
            .iter()
            .find(|p| p.name == ICalendarParameterName::Partstat)
            .map(|p| match &p.value {
                ICalendarParameterValue::Partstat(ICalendarParticipationStatus::Accepted) => "accepted",
                ICalendarParameterValue::Partstat(ICalendarParticipationStatus::Declined) => "declined",
                ICalendarParameterValue::Partstat(ICalendarParticipationStatus::Tentative) => "tentative",
                _ => "needs-action",
            })
            .unwrap_or("needs-action");
        Attendee { name, address: value, answer: answer.to_string() }
    };
    let organizer = component.property(&ICalendarProperty::Organizer).map(person).map(|p| if p.name.is_empty() { p.address } else { p.name }).unwrap_or_default();
    let attendees = component.properties(&ICalendarProperty::Attendee).map(person).collect();
    (organizer, attendees)
}

/// An event's alarms, in seconds from its start (`length` turns those set from
/// its end); one set at a fixed time counts only for an event that does not repeat.
fn alarms_of(ical: &ICalendar, component: &ICalendarComponent, start: i64, length: i64) -> Vec<i64> {
    let mut out = Vec::new();
    for child in component.component_ids.iter().filter_map(|id| ical.components.get(*id as usize)) {
        if child.component_type != ICalendarComponentType::VAlarm {
            continue;
        }
        let Some(trigger) = child.property(&ICalendarProperty::Trigger) else { continue };
        let from_end = trigger.params.iter().any(|p| matches!(p.value, ICalendarParameterValue::Related(calcard::icalendar::ICalendarRelated::End)));
        match trigger.values.first() {
            Some(ICalendarValue::Duration(duration)) => out.push(duration.as_seconds() + if from_end { length } else { 0 }),
            Some(ICalendarValue::PartialDateTime(at)) if !component.is_recurrent_or_override() => {
                if let Some(at) = at.to_timestamp() {
                    out.push(at - start);
                }
            }
            _ => {}
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The occurrences of one file's events between `from` and `to` (Unix seconds);
/// times written without a zone are taken in `zone`, yours.
pub fn file_occurrences(path: &Path, calendar: &Collection, from: i64, to: i64, zone: &TimeZone) -> Vec<Occurrence> {
    let Some(ical) = std::fs::read_to_string(path).ok().as_deref().and_then(parse) else { return Vec::new() };
    let expanded = ical.expand_dates(calcard_zone(zone), EXPANSION_LIMIT);
    let mut found = Vec::new();
    for event in expanded.events {
        let Some(component) = ical.components.get(event.comp_id as usize) else { continue };
        if component.component_type != ICalendarComponentType::VEvent {
            continue;
        }
        let start = event.start.timestamp();
        let end = match event.end {
            TimeOrDelta::Time(end) => end.timestamp(),
            TimeOrDelta::Delta(delta) => start + delta.num_seconds(),
        };
        let all_day = is_all_day(component);
        // An instant event still takes its minute; an all-day one its day.
        let end = if end > start { end } else if all_day { start + 86_400 } else { start };
        if end <= from || start >= to {
            continue;
        }
        let status = text_of(component, &ICalendarProperty::Status).to_ascii_uppercase();
        let (organizer, attendees) = people(component);
        let (margins, demands) = demands_of(component);
        found.push(Occurrence {
            key: path.display().to_string(),
            uid: component.uid().unwrap_or_default().to_string(),
            summary: lines::unescape(&text_of(component, &ICalendarProperty::Summary)),
            location: lines::unescape(&text_of(component, &ICalendarProperty::Location)),
            notes: lines::unescape(&text_of(component, &ICalendarProperty::Description)),
            start,
            end,
            all_day,
            recurring: component.is_recurrent_or_override(),
            calendar: calendar.name.clone(),
            color: calendar.color.clone(),
            cancelled: status == "CANCELLED",
            tentative: status == "TENTATIVE",
            organizer,
            attendees,
            read_only: calendar.read_only,
            alarms: alarms_of(&ical, component, start, end - start),
            margins,
            demands,
        });
    }
    found
}

/// Everything between `from` and `to`, from every calendar, by start.
pub fn occurrences(from: i64, to: i64) -> Vec<Occurrence> {
    let zone = TimeZone::system();
    let mut all: Vec<Occurrence> = vdir::collections(Kind::Calendars)
        .iter()
        .flat_map(|calendar| calendar.items().into_iter().flat_map(|path| file_occurrences(&path, calendar, from, to, &zone)).collect::<Vec<_>>())
        .collect();
    all.sort_by(|a, b| (a.start, a.all_day == false, &a.summary).cmp(&(b.start, b.all_day == false, &b.summary)));
    all
}

/// What the event form gives back. Times are local, in your time zone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEdit {
    pub title: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub notes: String,
    /// "2026-10-05T09:00", or "2026-10-05" for a whole day.
    pub start: String,
    /// The same; for whole days, the last day it covers.
    pub end: String,
    #[serde(default)]
    pub all_day: bool,
    /// "", "daily", "weekly", "monthly", "yearly".
    #[serde(default)]
    pub repeat: String,
    /// Minutes kept before and after it.
    #[serde(default)]
    pub margins: crate::demands::Margins,
    /// What it costs and gives back, 0 to 10 each; none unsaid.
    #[serde(default)]
    pub demands: crate::demands::Demands,
}

/// An event as the form shows it, from its file, in your time zone.
pub fn edit_of(path: &Path) -> Option<EventEdit> {
    edit_of_text(&std::fs::read_to_string(path).ok()?, &TimeZone::system())
}

/// An event as the form shows it, from its text, in `zone`.
pub fn edit_of_text(text: &str, zone: &TimeZone) -> Option<EventEdit> {
    let ical = parse(text)?;
    let (zone, tz) = (zone.clone(), calcard_zone(zone));
    let (index, master) = ical.components.iter().enumerate().find(|(_, c)| c.component_type == ICalendarComponentType::VEvent && c.property(&ICalendarProperty::RecurrenceId).is_none())?;
    // Its first occurrence still there: the very first may have been left out (EXDATE) or changed.
    let first = ical.expand_dates(tz, FIRST_LOOKED_AT).events.into_iter().find(|e| e.comp_id as usize == index)?;
    let start = Timestamp::from_second(first.start.timestamp()).ok()?.to_zoned(zone.clone());
    let end_seconds = match first.end {
        TimeOrDelta::Time(end) => end.timestamp(),
        TimeOrDelta::Delta(delta) => first.start.timestamp() + delta.num_seconds(),
    };
    let end = Timestamp::from_second(end_seconds).ok()?.to_zoned(zone);
    let all_day = is_all_day(master);
    let rule = master
        .property(&ICalendarProperty::Rrule)
        .and_then(|e| e.values.first())
        .map(|v| match v {
            ICalendarValue::RecurrenceRule(rule) => rule.to_string(),
            other => other.as_text().unwrap_or_default().to_string(),
        })
        .unwrap_or_default()
        .to_ascii_uppercase();
    let repeat = ["DAILY", "WEEKLY", "MONTHLY", "YEARLY"].into_iter().find(|f| rule.contains(&format!("FREQ={f}"))).map(str::to_lowercase).unwrap_or_default();
    let (margins, demands) = demands_of(master);
    Some(EventEdit {
        title: lines::unescape(&text_of(master, &ICalendarProperty::Summary)),
        location: lines::unescape(&text_of(master, &ICalendarProperty::Location)),
        notes: lines::unescape(&text_of(master, &ICalendarProperty::Description)),
        start: if all_day { start.date().to_string() } else { start.strftime("%Y-%m-%dT%H:%M").to_string() },
        // A whole-day event ends the morning after its last day.
        end: if all_day { end.date().checked_sub(1.day()).unwrap_or(end.date()).to_string() } else { end.strftime("%Y-%m-%dT%H:%M").to_string() },
        all_day,
        repeat,
        margins,
        demands,
    })
}

/// The start and end the form means, in your time zone.
fn instants(edit: &EventEdit, zone: &TimeZone) -> Result<(Zoned, Zoned), String> {
    let bad = |what: &str| format!("{what}: ?");
    if edit.all_day {
        let start: Date = edit.start.get(..10).unwrap_or(&edit.start).parse().map_err(|_| bad(&edit.start))?;
        let last: Date = edit.end.get(..10).unwrap_or(&edit.end).parse().unwrap_or(start);
        let last = if last < start { start } else { last };
        let start = start.to_zoned(zone.clone()).map_err(|e| e.to_string())?;
        let end = last.checked_add(1.day()).map_err(|e| e.to_string())?.to_zoned(zone.clone()).map_err(|e| e.to_string())?;
        return Ok((start, end));
    }
    let start: DateTime = edit.start.parse().map_err(|_| bad(&edit.start))?;
    let end: DateTime = edit.end.parse().unwrap_or(start);
    let start = start.to_zoned(zone.clone()).map_err(|e| e.to_string())?;
    let end = end.to_zoned(zone.clone()).map_err(|e| e.to_string())?;
    // An end before the start means an hour.
    let end = if end <= start { start.checked_add(1.hour()).map_err(|e| e.to_string())? } else { end };
    Ok((start, end))
}

fn utc(at: &Zoned) -> String {
    at.timestamp().strftime("%Y%m%dT%H%M%SZ").to_string()
}

/// DTSTART and DTEND lines for the form's times: dates for whole days; UTC for
/// one-off events; local time with the zone for repeating ones.
fn time_lines(edit: &EventEdit, zone: &TimeZone) -> Result<(Vec<String>, Option<String>), String> {
    let (start, end) = instants(edit, zone)?;
    if edit.all_day {
        return Ok((vec![format!("DTSTART;VALUE=DATE:{}", start.date().strftime("%Y%m%d")), format!("DTEND;VALUE=DATE:{}", end.date().strftime("%Y%m%d"))], None));
    }
    match zone.iana_name().filter(|_| !edit.repeat.is_empty()) {
        Some(name) => Ok((
            vec![format!("DTSTART;TZID={name}:{}", start.strftime("%Y%m%dT%H%M%S")), format!("DTEND;TZID={name}:{}", end.strftime("%Y%m%dT%H%M%S"))],
            Some(name.to_string()),
        )),
        None => Ok((vec![format!("DTSTART:{}", utc(&start)), format!("DTEND:{}", utc(&end))], None)),
    }
}

fn rule_line(repeat: &str) -> Option<String> {
    match repeat {
        "daily" | "weekly" | "monthly" | "yearly" => Some(format!("RRULE:FREQ={}", repeat.to_ascii_uppercase())),
        _ => None,
    }
}

/// The VTIMEZONE of a zone, as its changes of hour go this year: one STANDARD
/// and one DAYLIGHT part, each repeating yearly on the same weekday of the
/// same month ("last Sunday of October"); a single part when it has no summer time.
pub fn vtimezone(zone: &TimeZone, name: &str, year: i16) -> Vec<String> {
    let mut out = vec!["BEGIN:VTIMEZONE".to_string(), format!("TZID:{name}")];
    let Some(from) = Date::new(year, 1, 1).ok().and_then(|d| d.to_zoned(zone.clone()).ok()) else { return out };
    let transitions: Vec<_> = zone.following(from.timestamp()).take_while(|t| t.timestamp().to_zoned(zone.clone()).year() == year).take(2).collect();
    let offset = |o: jiff::tz::Offset| {
        let seconds = o.seconds();
        let sign = if seconds < 0 { '-' } else { '+' };
        format!("{sign}{:02}{:02}", seconds.abs() / 3600, (seconds.abs() % 3600) / 60)
    };
    if transitions.len() < 2 {
        let now = zone.to_offset(from.timestamp());
        out.extend(["BEGIN:STANDARD".to_string(), "DTSTART:19700101T000000".to_string(), format!("TZOFFSETFROM:{}", offset(now)), format!("TZOFFSETTO:{}", offset(now)), "END:STANDARD".to_string()]);
    } else {
        for transition in transitions {
            let before = zone.to_offset(transition.timestamp().checked_sub(1.second()).unwrap_or(transition.timestamp()));
            let after = transition.offset();
            // The local time of the change, as clocks showed it just before.
            let local = transition.timestamp().to_zoned(TimeZone::fixed(before));
            let weekday = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"][local.weekday().to_monday_zero_offset() as usize];
            let days_in_month = local.date().days_in_month();
            let nth = if local.day() + 7 > days_in_month { "-1".to_string() } else { ((local.day() - 1) / 7 + 1).to_string() };
            let part = if transition.dst().is_dst() { "DAYLIGHT" } else { "STANDARD" };
            out.extend([
                format!("BEGIN:{part}"),
                format!("DTSTART:{}", local.strftime("%Y%m%dT%H%M%S")),
                format!("RRULE:FREQ=YEARLY;BYMONTH={};BYDAY={nth}{weekday}", local.month()),
                format!("TZOFFSETFROM:{}", offset(before)),
                format!("TZOFFSETTO:{}", offset(after)),
                format!("TZNAME:{}", transition.abbreviation()),
                format!("END:{part}"),
            ]);
        }
    }
    out.push("END:VTIMEZONE".to_string());
    out
}

/// A new event from the form, its times in `zone` (yours), with its own UID.
pub fn new_event(edit: &EventEdit, zone: &TimeZone) -> Result<String, String> {
    let zone = zone.clone();
    let (times, tzid) = time_lines(edit, &zone)?;
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "CALSCALE:GREGORIAN".to_string()];
    if let Some(name) = &tzid {
        out.extend(vtimezone(&zone, name, Zoned::now().with_time_zone(zone.clone()).year()));
    }
    out.extend(["BEGIN:VEVENT".to_string(), format!("UID:{}", vdir::new_name()), format!("DTSTAMP:{}", utc(&Zoned::now()))]);
    out.extend(times);
    out.extend(content_lines(edit, Changed::ALL));
    out.extend(["END:VEVENT".to_string(), "END:VCALENDAR".to_string()]);
    Ok(lines::fold(&out))
}

/// What the form changed, field by field: a field unchanged keeps its line.
#[derive(Clone, Copy)]
struct Changed {
    title: bool,
    location: bool,
    notes: bool,
    rule: bool,
    margins: bool,
    demands: bool,
}

impl Changed {
    const ALL: Changed = Changed { title: true, location: true, notes: true, rule: true, margins: true, demands: true };
}

/// SUMMARY, LOCATION, DESCRIPTION and the repeat rule, those that changed.
fn content_lines(edit: &EventEdit, changed: Changed) -> Vec<String> {
    let mut out = Vec::new();
    if changed.title {
        out.push(format!("SUMMARY:{}", lines::escape(edit.title.trim())));
    }
    if changed.location && !edit.location.trim().is_empty() {
        out.push(format!("LOCATION:{}", lines::escape(edit.location.trim())));
    }
    if changed.notes && !edit.notes.trim().is_empty() {
        out.push(format!("DESCRIPTION:{}", lines::escape(edit.notes.trim())));
    }
    if changed.rule {
        out.extend(rule_line(&edit.repeat));
    }
    if changed.margins {
        if edit.margins.before > 0 {
            out.push(format!("{}:{}", crate::demands::BEFORE, edit.margins.before.min(24 * 60)));
        }
        if edit.margins.after > 0 {
            out.push(format!("{}:{}", crate::demands::AFTER, edit.margins.after.min(24 * 60)));
        }
    }
    if changed.demands {
        if edit.demands.has_cost() {
            out.push(format!("{}:{}", crate::demands::COST, edit.demands.cost_value()));
        }
        if let Some(gain) = edit.demands.gain {
            out.push(format!("{}:{}", crate::demands::GAIN, gain.min(10)));
        }
    }
    out
}

/// Lines of the main event (not a changed occurrence) rewritten at every change.
const EDITED: &[&str] = &["DTSTAMP", "LAST-MODIFIED", "SEQUENCE"];
/// Rewritten only when the form changed the time: an event written in another
/// zone keeps it, and so does its time.
const TIMES: &[&str] = &["DTSTART", "DTEND", "DURATION"];

/// The event's text with what the form edits changed in its main VEVENT;
/// the rest, changed occurrences and alarms included, kept as it was. The
/// times and the repeat rule are written again only when they changed: a rule
/// the form cannot show ("every other Monday and Wednesday") stays whole while
/// the title changes. The SEQUENCE goes up, as RFC 5545 §3.8.7.4 asks.
pub fn apply(text: &str, edit: &EventEdit, zone: &TimeZone) -> Result<String, String> {
    let zone = zone.clone();
    let before = edit_of_text(text, &zone);
    // Nothing changed: nothing is written, not even the stamps.
    if before.as_ref() == Some(edit) {
        return Ok(text.to_string());
    }
    let times_changed = before.as_ref().is_none_or(|b| (&b.start, &b.end, b.all_day) != (&edit.start, &edit.end, edit.all_day));
    let rule_changed = before.as_ref().is_none_or(|b| b.repeat != edit.repeat);
    let changed = match &before {
        Some(b) => Changed { title: b.title.trim() != edit.title.trim(), location: b.location.trim() != edit.location.trim(), notes: b.notes.trim() != edit.notes.trim(), rule: rule_changed, margins: b.margins != edit.margins, demands: b.demands != edit.demands },
        None => Changed::ALL,
    };
    let (times, tzid) = if times_changed || rule_changed { time_lines(edit, &zone)? } else { (Vec::new(), None) };
    let rewritten = |name: &str| {
        EDITED.contains(&name)
            || ((times_changed || rule_changed) && TIMES.contains(&name))
            || (rule_changed && name == "RRULE")
            || (changed.title && name == "SUMMARY")
            || (changed.location && name == "LOCATION")
            || (changed.notes && name == "DESCRIPTION")
            || (changed.margins && (name == crate::demands::BEFORE || name == crate::demands::AFTER))
            || (changed.demands && (name == crate::demands::COST || name == crate::demands::GAIN))
    };
    let source = lines::unfold(text);
    let mut out: Vec<String> = Vec::with_capacity(source.len() + 8);
    let mut depth_in_event = false;
    let mut nested = 0usize;
    let mut is_override = false;
    let mut event: Vec<String> = Vec::new();
    let mut has_zone = false;
    for line in source {
        let name = lines::name(&line);
        let value = lines::value(&line).trim().to_ascii_uppercase();
        if name == "BEGIN" && value == "VTIMEZONE" && tzid.as_deref().is_some_and(|z| text.contains(&format!("TZID:{z}"))) {
            has_zone = true;
        }
        if !depth_in_event {
            if name == "BEGIN" && value == "VEVENT" {
                depth_in_event = true;
                nested = 0;
                is_override = false;
                event = vec![line];
            } else {
                out.push(line);
            }
            continue;
        }
        // Inside a VEVENT: its alarms are nested components, kept as they are.
        if name == "BEGIN" {
            nested += 1;
        }
        if name == "END" && nested > 0 {
            nested -= 1;
            event.push(line);
            continue;
        }
        if name == "END" && value == "VEVENT" {
            depth_in_event = false;
            if is_override {
                event.push(line);
                out.append(&mut event);
                continue;
            }
            let sequence = event.iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse::<u32>().ok()).unwrap_or(0);
            let mut kept: Vec<String> = event.drain(..).filter(|l| !rewritten(&lines::name(l))).collect();
            let at = kept.iter().position(|l| lines::name(l) == "BEGIN" && lines::value(l).trim().eq_ignore_ascii_case("VALARM")).unwrap_or(kept.len());
            let mut added = vec![format!("DTSTAMP:{}", utc(&Zoned::now())), format!("LAST-MODIFIED:{}", utc(&Zoned::now())), format!("SEQUENCE:{}", sequence.saturating_add(1))];
            added.extend(times.clone());
            added.extend(content_lines(edit, changed));
            kept.splice(at..at, added);
            kept.push(line);
            out.append(&mut kept);
            continue;
        }
        if nested == 0 && name == "RECURRENCE-ID" {
            is_override = true;
        }
        event.push(line);
    }
    // A repeating event in your zone needs the zone written once.
    if let (Some(name), false) = (&tzid, has_zone) {
        let at = out.iter().position(|l| lines::name(l) == "BEGIN" && lines::value(l).trim().eq_ignore_ascii_case("VEVENT")).unwrap_or(out.len());
        let zone_lines = vtimezone(&zone, name, Zoned::now().year());
        out.splice(at..at, zone_lines);
    }
    Ok(lines::fold(&out))
}

/// Leaves one occurrence of a repeating event out (EXDATE), in the form DTSTART is written.
pub fn skip_occurrence(text: &str, start: i64) -> Option<String> {
    let at = Timestamp::from_second(start).ok()?;
    let mut source = lines::unfold(text);
    let master = master_range(&source)?;
    let dtstart = source[master.clone()].iter().find(|l| lines::name(l) == "DTSTART")?.clone();
    let params = dtstart.split(':').next().unwrap_or("DTSTART").trim_start_matches("DTSTART").to_string();
    let value = lines::value(&dtstart).trim();
    let written = if value.len() == 8 {
        at.to_zoned(TimeZone::system()).strftime("%Y%m%d").to_string()
    } else if value.ends_with('Z') {
        at.strftime("%Y%m%dT%H%M%SZ").to_string()
    } else {
        let tzid = params.split(';').find_map(|p| p.strip_prefix("TZID=")).map(|t| t.trim_matches('"').to_string());
        let zone = tzid.and_then(|t| TimeZone::get(&t).ok()).unwrap_or_else(TimeZone::system);
        at.to_zoned(zone).strftime("%Y%m%dT%H%M%S").to_string()
    };
    let end = master.end - 1;
    source.insert(end, format!("EXDATE{params}:{written}"));
    Some(lines::fold(&source))
}

/// The event with `added` lines in its main VEVENT (a LINK, a REFID), those
/// not there yet, before its alarms; None when it holds no event. Nothing
/// else changes: the stamps stay, as a link is not a change of the event.
pub fn add_lines(text: &str, added: &[String]) -> Option<String> {
    let mut source = lines::unfold(text);
    let master = master_range(&source)?;
    let new: Vec<String> = added.iter().filter(|l| !source[master.clone()].contains(l)).cloned().collect();
    if new.is_empty() {
        return Some(text.to_string());
    }
    let at = source[master.clone()].iter().position(|l| lines::name(l) == "BEGIN" && !lines::value(l).trim().eq_ignore_ascii_case("VEVENT")).map_or(master.end - 1, |i| master.start + i);
    source.splice(at..at, new);
    Some(lines::fold(&source))
}

/// The main VEVENT without the lines `drop` picks (its alarms untouched);
/// None when the text holds no event.
pub fn remove_lines(text: &str, drop: impl Fn(&str) -> bool) -> Option<String> {
    let mut source = lines::unfold(text);
    let master = master_range(&source)?;
    let last = master.len() - 1;
    let mut nested = 0usize;
    let kept: Vec<String> = source[master.clone()]
        .iter()
        .enumerate()
        .filter(|(i, line)| {
            if *i == 0 || *i == last {
                return true;
            }
            match lines::name(line).as_str() {
                "BEGIN" => nested += 1,
                "END" => nested = nested.saturating_sub(1),
                _ if nested == 0 => return !drop(line),
                _ => {}
            }
            true
        })
        .map(|(_, line)| line.clone())
        .collect();
    if kept.len() == master.len() {
        return Some(text.to_string());
    }
    source.splice(master, kept);
    Some(lines::fold(&source))
}

/// The lines of the main VEVENT, BEGIN to END.
fn master_range(source: &[String]) -> Option<std::ops::Range<usize>> {
    let mut start = None;
    let mut nested = 0;
    let mut is_override = false;
    for (i, line) in source.iter().enumerate() {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        match (name.as_str(), start) {
            ("BEGIN", None) if value == "VEVENT" => {
                start = Some(i);
                is_override = false;
            }
            ("BEGIN", Some(_)) => nested += 1,
            ("END", Some(_)) if nested > 0 => nested -= 1,
            ("RECURRENCE-ID", Some(_)) if nested == 0 => is_override = true,
            ("END", Some(begin)) if value == "VEVENT" => {
                if !is_override {
                    return Some(begin..i + 1);
                }
                start = None;
            }
            _ => {}
        }
    }
    None
}

/// What an event file ties to: its links (LINK, RELATED-TO), its cases
/// (REFID) and its guests, read from its main VEVENT.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventRef {
    pub key: String,
    pub uid: String,
    pub summary: String,
    /// Its first start, Unix seconds; 0 when unreadable.
    pub start: i64,
    pub links: Vec<crate::tasks::Link>,
    pub related: Vec<crate::tasks::Relation>,
    pub cases: Vec<String>,
    /// The guests' and organizer's addresses.
    pub people: Vec<String>,
}

/// The ties of one event's text; None when it holds no event.
pub fn event_ref(text: &str, key: &str, zone: &TimeZone) -> Option<EventRef> {
    let source = lines::unfold(text);
    let master = master_range(&source)?;
    let mut found = EventRef { key: key.to_string(), ..EventRef::default() };
    let mut nested = 0usize;
    for line in &source[master.start + 1..master.end - 1] {
        let name = lines::name(line);
        match name.as_str() {
            "BEGIN" => nested += 1,
            "END" => nested = nested.saturating_sub(1),
            _ if nested > 0 => {}
            "UID" => found.uid = lines::value(line).trim().to_string(),
            "SUMMARY" => found.summary = lines::unescape(lines::value(line).trim()),
            "LINK" => found.links.push(crate::tasks::link_of(line)),
            "RELATED-TO" => found.related.push(crate::tasks::relation_of(line)),
            "REFID" => found.cases.push(lines::unescape(lines::value(line).trim())),
            "ATTENDEE" | "ORGANIZER" => found.people.push(address(lines::value(line))),
            _ => {}
        }
    }
    found.start = edit_of_text(text, zone).and_then(|e| {
        let at: DateTime = if e.all_day { format!("{}T00:00", e.start).parse().ok()? } else { e.start.parse().ok()? };
        at.to_zoned(zone.clone()).ok().map(|z| z.timestamp().as_second())
    }).unwrap_or(0);
    Some(found)
}

/// The ties of every event, in every calendar.
pub fn event_refs() -> Vec<EventRef> {
    let zone = TimeZone::system();
    vdir::collections(Kind::Calendars)
        .iter()
        .flat_map(|c| c.items())
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            text.to_ascii_uppercase().contains("BEGIN:VEVENT").then(|| event_ref(&text, &path.display().to_string(), &zone)).flatten()
        })
        .collect()
}

/// Where a new event goes: the first calendar that can be written to and takes events.
pub fn default_calendar() -> Option<Collection> {
    vdir::collections(Kind::Calendars).into_iter().find(|c| !c.read_only && c.holds("VEVENT"))
}

/// The file a new event gets in a calendar.
pub fn new_path(calendar: &Collection) -> PathBuf {
    calendar.dir.join(format!("{}.ics", vdir::new_name()))
}

/// An invitation (iTIP, RFC 5546) carried by a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Invitation {
    /// "REQUEST", "CANCEL", "REPLY", "PUBLISH"…
    pub method: String,
    pub uid: String,
    pub summary: String,
    pub location: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub organizer: String,
    pub organizer_address: String,
    pub attendees: Vec<Attendee>,
}

/// What an invitation asks, from the text of its `text/calendar` part; times
/// without a zone are taken in `zone`, yours.
pub fn invitation(text: &str, zone: &TimeZone) -> Option<Invitation> {
    let ical = parse(text)?;
    let method = ical
        .components
        .iter()
        .find(|c| c.component_type == ICalendarComponentType::VCalendar)
        .and_then(|c| c.property(&ICalendarProperty::Method))
        .and_then(|e| e.values.first())
        .map(|v| match v {
            ICalendarValue::Method(m) => format!("{m:?}").to_ascii_uppercase(),
            other => other.as_text().unwrap_or_default().to_ascii_uppercase(),
        })
        .unwrap_or_else(|| "PUBLISH".into());
    let tz = calcard_zone(zone);
    let (index, event) = ical.components.iter().enumerate().find(|(_, c)| c.component_type == ICalendarComponentType::VEvent)?;
    let first = ical.expand_dates(tz, FIRST_LOOKED_AT).events.into_iter().find(|e| e.comp_id as usize == index)?;
    let start = first.start.timestamp();
    let end = match first.end {
        TimeOrDelta::Time(end) => end.timestamp(),
        TimeOrDelta::Delta(delta) => start + delta.num_seconds(),
    };
    let (organizer, attendees) = people(event);
    let organizer_address = event.property(&ICalendarProperty::Organizer).and_then(|e| e.values.first()).and_then(ICalendarValue::as_text).map(address).unwrap_or_default();
    Some(Invitation {
        method,
        uid: event.uid().unwrap_or_default().to_string(),
        summary: lines::unescape(&text_of(event, &ICalendarProperty::Summary)),
        location: lines::unescape(&text_of(event, &ICalendarProperty::Location)),
        start,
        end,
        all_day: is_all_day(event),
        organizer,
        organizer_address,
        attendees,
    })
}

/// Your answer to an invitation (iTIP REPLY, RFC 5546 §3.2.3): the event with
/// you alone among the guests, your answer in PARTSTAT. `answer`: "ACCEPTED",
/// "DECLINED" or "TENTATIVE".
pub fn reply(text: &str, me: &str, answer: &str) -> Option<String> {
    let source = lines::unfold(text);
    let master = master_range(&source)?;
    let mine = |line: &str| address(lines::value(line)).eq_ignore_ascii_case(me);
    let attendee = source[master.clone()].iter().find(|l| lines::name(l) == "ATTENDEE" && mine(l)).cloned().unwrap_or_else(|| format!("ATTENDEE:mailto:{me}"));
    // PARTSTAT replaced, RSVP dropped: an answer asks for none. A line without
    // its colon (a broken file) has no value: all of it is the head.
    let at = attendee.len().checked_sub(lines::value(&attendee).len() + 1).filter(|&at| attendee.as_bytes().get(at) == Some(&b':')).unwrap_or(attendee.len());
    let (head, value) = attendee.split_at(at);
    let params: Vec<&str> = head.split(';').skip(1).filter(|p| !p.to_ascii_uppercase().starts_with("PARTSTAT=") && !p.to_ascii_uppercase().starts_with("RSVP=")).collect();
    let mut attendee = String::from("ATTENDEE");
    for param in params {
        attendee.push(';');
        attendee.push_str(param);
    }
    attendee.push_str(&format!(";PARTSTAT={answer}{value}"));
    let kept = ["UID", "SEQUENCE", "DTSTART", "DTEND", "DURATION", "SUMMARY", "ORGANIZER", "RECURRENCE-ID"];
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "METHOD:REPLY".to_string()];
    // The zones the event's times name.
    let mut in_zone = false;
    for line in &source {
        let name = lines::name(line);
        let upper = lines::value(line).trim().to_ascii_uppercase();
        if name == "BEGIN" && upper == "VTIMEZONE" {
            in_zone = true;
        }
        if in_zone {
            out.push(line.clone());
        }
        if name == "END" && upper == "VTIMEZONE" {
            in_zone = false;
        }
    }
    out.push("BEGIN:VEVENT".to_string());
    out.extend(source[master].iter().filter(|l| kept.contains(&lines::name(l).as_str())).cloned());
    out.push(format!("DTSTAMP:{}", utc(&Zoned::now())));
    out.push(attendee);
    out.extend(["END:VEVENT".to_string(), "END:VCALENDAR".to_string()]);
    Some(lines::fold(&out))
}

/// Adds an invitation's event to a calendar file, as the organizer sent it,
/// METHOD removed (RFC 4791 §4.1 forbids it in stored objects).
pub fn stored(text: &str) -> String {
    let kept: Vec<String> = lines::unfold(text).into_iter().filter(|l| lines::name(l) != "METHOD").collect();
    lines::fold(&kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_leaves_an_event() {
        let text = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:e\r\nSUMMARY:Meeting\r\nLINK;LINKREL=related;VALUE=URI:sioul:task/a\r\nLINK;LINKREL=related;VALUE=URI:sioul:task/b\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:sioul:task/a\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let less = remove_lines(text, |l| l.ends_with("sioul:task/a")).unwrap();
        assert!(!less.contains("VALUE=URI:sioul:task/a"));
        assert!(less.contains("VALUE=URI:sioul:task/b"));
        assert!(less.contains("DESCRIPTION:sioul:task/a"), "the alarm is left alone");
        assert_eq!(remove_lines(text, |_| false).unwrap(), text);
    }

    fn calendar() -> Collection {
        Collection { kind: Kind::Calendars, account: "a".into(), id: "personal".into(), dir: PathBuf::from("/tmp"), name: "Personal".into(), color: Some("#4c6b5c".into()), read_only: false, components: vec![] }
    }

    const WEEKLY: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\nBEGIN:VEVENT\r\nUID:yoga-1\r\nDTSTAMP:20260101T000000Z\r\n\
        DTSTART;TZID=Europe/Paris:20260105T180000\r\nDTEND;TZID=Europe/Paris:20260105T193000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO\r\n\
        EXDATE;TZID=Europe/Paris:20261012T180000\r\nSUMMARY:Yoga\r\nLOCATION:Salle des fêtes\r\nX-APP-THING:keep\r\n\
        BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nDESCRIPTION:Yoga\r\nEND:VALARM\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nUID:yoga-1\r\nRECURRENCE-ID;TZID=Europe/Paris:20261019T180000\r\nDTSTART;TZID=Europe/Paris:20261019T190000\r\n\
        DTEND;TZID=Europe/Paris:20261019T203000\r\nSUMMARY:Yoga (later)\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    fn write_temp(name: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-agenda-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> i64 {
        let local: DateTime = text.parse().unwrap();
        local.to_zoned(TimeZone::get("Europe/Paris").unwrap()).unwrap().timestamp().as_second()
    }

    #[test]
    fn repeats_with_exceptions_and_changes() {
        let path = write_temp("yoga.ics", WEEKLY);
        let found = file_occurrences(&path, &calendar(), at("2026-10-01T00:00"), at("2026-10-27T00:00"), &paris());
        // 5, 12 (left out), 19 (moved to 19:00), 26.
        let starts: Vec<(i64, &str)> = found.iter().map(|o| (o.start, o.summary.as_str())).collect();
        assert_eq!(starts, vec![(at("2026-10-05T18:00"), "Yoga"), (at("2026-10-19T19:00"), "Yoga (later)"), (at("2026-10-26T18:00"), "Yoga")]);
        assert!(found[0].recurring && found[0].location == "Salle des fêtes" && found[0].end - found[0].start == 5400);
    }

    #[test]
    fn a_new_title_leaves_the_rule_and_the_zone_alone() {
        let mut edit = edit_of_text(WEEKLY, &paris()).unwrap();
        assert_eq!((edit.start.as_str(), edit.repeat.as_str()), ("2026-01-05T18:00", "weekly"));
        edit.title = "Yoga with Marie".into();
        let text = apply(WEEKLY, &edit, &paris()).unwrap();
        assert!(text.contains("RRULE:FREQ=WEEKLY;BYDAY=MO") && text.contains("DTSTART;TZID=Europe/Paris:20260105T180000"), "{text}");
        assert!(text.contains("SUMMARY:Yoga with Marie"), "{text}");
        // Its first occurrence left out: still opened, from the next one.
        let skipped = skip_occurrence(WEEKLY, at("2026-01-05T18:00")).unwrap();
        assert_eq!(edit_of_text(&skipped, &paris()).map(|e| e.start), Some("2026-01-12T18:00".to_string()));
    }

    #[test]
    fn edits_keep_alarms_overrides_and_unknown_lines() {
        let edit = EventEdit { title: "Yoga, new room".into(), location: "Gymnase".into(), start: "2026-01-05T18:30".into(), end: "2026-01-05T20:00".into(), repeat: "weekly".into(), ..EventEdit::default() };
        let text = apply(WEEKLY, &edit, &paris()).unwrap();
        assert!(text.contains("SUMMARY:Yoga\\, new room") && text.contains("LOCATION:Gymnase") && text.contains("SEQUENCE:1"), "{text}");
        assert!(text.contains("X-APP-THING:keep") && text.contains("TRIGGER:-PT15M") && text.contains("SUMMARY:Yoga (later)"), "{text}");
        // New times, the same rule: it was still "weekly".
        assert!(text.contains("DTSTART;TZID=Europe/Paris:20260105T183000") && text.contains("RRULE:FREQ=WEEKLY;BYDAY=MO"), "{text}");
        assert!(text.contains("EXDATE;TZID=Europe/Paris:20261012T180000"), "{text}");
        assert!(parse(&text).is_some());
        let skipped = skip_occurrence(WEEKLY, at("2026-10-05T18:00")).unwrap();
        assert!(skipped.contains("EXDATE;TZID=Europe/Paris:20261005T180000"), "{skipped}");
    }

    #[test]
    fn new_events_and_answers() {
        let one = new_event(&EventEdit { title: "Dentist".into(), start: "2026-10-07T09:00".into(), end: "2026-10-07T09:30".into(), ..EventEdit::default() }, &paris()).unwrap();
        assert!(one.contains("SUMMARY:Dentist") && one.contains("DTSTART:20261007T070000Z") && parse(&one).is_some(), "{one}");
        let weekly = new_event(&EventEdit { title: "Choir".into(), start: "2026-10-07T20:00".into(), end: "2026-10-07T22:00".into(), repeat: "weekly".into(), ..EventEdit::default() }, &paris()).unwrap();
        assert!(weekly.contains("DTSTART;TZID=Europe/Paris:20261007T200000") && weekly.contains("BEGIN:VTIMEZONE") && weekly.contains("RRULE:FREQ=WEEKLY"), "{weekly}");
        let day = new_event(&EventEdit { title: "Holidays".into(), start: "2026-12-24".into(), end: "2026-12-26".into(), all_day: true, ..EventEdit::default() }, &paris()).unwrap();
        assert!(day.contains("DTSTART;VALUE=DATE:20261224") && day.contains("DTEND;VALUE=DATE:20261227"), "{day}");
        let invite = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:meet-1\r\nSEQUENCE:0\r\nDTSTAMP:20261001T000000Z\r\n\
            DTSTART:20261008T080000Z\r\nDTEND:20261008T090000Z\r\nSUMMARY:Review\r\nORGANIZER;CN=Jane:mailto:jane@example.org\r\n\
            ATTENDEE;CN=Me;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:me@example.net\r\nATTENDEE;CN=Paul:mailto:paul@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let asked = invitation(invite, &paris()).unwrap();
        assert_eq!((asked.method.as_str(), asked.summary.as_str(), asked.organizer.as_str()), ("REQUEST", "Review", "Jane"));
        assert_eq!(asked.end - asked.start, 3600);
        let answer = reply(invite, "me@example.net", "ACCEPTED").unwrap();
        assert!(answer.contains("METHOD:REPLY") && answer.contains("ATTENDEE;CN=Me;PARTSTAT=ACCEPTED:mailto:me@example.net"), "{answer}");
        assert!(!answer.contains("paul@example.org") && answer.contains("ORGANIZER;CN=Jane:mailto:jane@example.org"), "{answer}");
        assert!(!stored(invite).contains("METHOD"));
    }

    #[test]
    fn the_zone_repeats_its_changes_of_hour() {
        let zone = TimeZone::get("Europe/Paris").unwrap();
        let text = vtimezone(&zone, "Europe/Paris", 2026).join("\n");
        assert!(text.contains("BEGIN:DAYLIGHT\nDTSTART:20260329T020000\nRRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\nTZOFFSETFROM:+0100\nTZOFFSETTO:+0200"), "{text}");
        assert!(text.contains("BEGIN:STANDARD\nDTSTART:20261025T030000\nRRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\nTZOFFSETFROM:+0200\nTZOFFSETTO:+0100"), "{text}");
    }
}
