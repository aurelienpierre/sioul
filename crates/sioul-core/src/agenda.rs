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

/// Occurrences of a repeating event looked at, at most, per file: a safety
/// net. Expansion starts just before the stretch asked for (`from_window`),
/// so an event repeating every hour for years still reaches today.
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
    /// TRANSP:TRANSPARENT: shown as any event, but holds no time (a reminder
    /// of a holiday, "working from home"): the plan lays tasks over it (`holds_time`).
    pub transparent: bool,
    /// You declined it: the guest that is you (one of your accounts' addresses)
    /// answered PARTSTAT=DECLINED (`occurrences`, `mark_declined`). Shown as
    /// before; holds no time (`holds_time`).
    pub declined: bool,
    pub organizer: String,
    pub attendees: Vec<Attendee>,
    pub read_only: bool,
    /// Its alarms (VALARM), in seconds from its start: -900 for a quarter of an hour before.
    pub alarms: Vec<i64>,
    /// Minutes kept before and after it: getting there and back (`demands`).
    pub margins: crate::demands::Margins,
    /// What it costs and gives back, rated 0 to 10 (`demands`).
    pub demands: crate::demands::Demands,
    /// The task it is the time block of (`X-SIOUL-TASK`, `blocks`), by UID; "" for an event of its own.
    pub task: String,
    /// The turn of a repeating task that block pins (`X-SIOUL-TURN`): "2026-10-09"; "" otherwise.
    pub turn: String,
    /// Its reminder before, as it says it (`reminders::REMIND`): "" as usual,
    /// "NONE", "15"; a changed occurrence's own, else its series'.
    pub remind: String,
}

/// Someone invited, and what they answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attendee {
    pub name: String,
    pub address: String,
    /// "accepted", "declined", "tentative", "needs-action".
    pub answer: String,
}

impl Occurrence {
    /// Whether it takes time from the plan, your hours and your meals
    /// (`plan::event_spans`, `capacity`): not cancelled, not transparent, not
    /// declined by you. Whole days are left out where times are counted.
    pub fn holds_time(&self) -> bool {
        !self.cancelled && !self.transparent && !self.declined
    }
}

/// Each event `own` (your addresses) declined marked so (`Occurrence::declined`).
pub fn mark_declined(events: &mut [Occurrence], own: &[String]) {
    for event in events.iter_mut() {
        event.declined = event.attendees.iter().any(|a| a.answer == "declined" && own.iter().any(|o| o.trim().eq_ignore_ascii_case(a.address.trim())));
    }
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

/// The text as calcard should expand it. calcard puts a changed occurrence
/// (RECURRENCE-ID) in place of its time in the series only when both have the
/// same SEQUENCE, where RFC 5545 matches them by RECURRENCE-ID alone: a series
/// edited since, or an occurrence changed by an application that counts its own
/// changes, would show twice, at its old time and its new one. So each changed
/// occurrence is read with the series' SEQUENCE; the file is left as it is.
fn for_expansion(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains("RECURRENCE-ID") {
        return std::borrow::Cow::Borrowed(text);
    }
    let source = lines::unfold(text);
    let Some(master) = master_range(&source) else { return std::borrow::Cow::Borrowed(text) };
    let own = |range: &std::ops::Range<usize>| -> Vec<usize> {
        let mut nested = 0usize;
        let mut out = Vec::new();
        for i in range.start + 1..range.end.saturating_sub(1) {
            match lines::name(&source[i]).as_str() {
                "BEGIN" => nested += 1,
                "END" => nested = nested.saturating_sub(1),
                "SEQUENCE" if nested == 0 => out.push(i),
                _ => {}
            }
        }
        out
    };
    let series = own(&master).first().map(|&i| source[i].clone());
    let mut out: Vec<String> = Vec::with_capacity(source.len() + 4);
    let mut next = 0;
    for range in event_ranges(&source).into_iter().filter(|r| *r != master) {
        let theirs = own(&range);
        out.extend(source[next..range.start + 1].iter().cloned());
        out.extend(series.clone());
        out.extend((range.start + 1..range.end).filter(|i| !theirs.contains(i)).map(|i| source[i].clone()));
        next = range.end;
    }
    out.extend(source[next..].iter().cloned());
    std::borrow::Cow::Owned(lines::fold(&out))
}

/// The text as calcard should expand it for occurrences from `from` (Unix
/// seconds) on. calcard expands a repeating event from its start, at most
/// `EXPANSION_LIMIT` occurrences, so an event repeating every hour begun years
/// ago would never reach today. Its start (and its end) moved forward by whole
/// periods of its rule, to a couple of days and a period before `from`, give
/// the same occurrences from there on: done for rules repeating by the second,
/// minute, hour, day or week (their periods are all as long; a month or a year
/// reaches far enough from the start), without COUNT (counted from the true
/// start), and without a change to "this and those after" (RANGE, whose
/// offset starts at its own occurrence). A start without a time, or one that
/// does not read, is left as it is. The file is left as it is.
fn from_window(text: &str, from: i64) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    if !text.contains("RRULE") {
        return Cow::Borrowed(text);
    }
    let mut source = lines::unfold(text);
    if source.iter().any(|l| lines::name(l) == "RECURRENCE-ID" && lines::param(l, "RANGE").is_some()) {
        return Cow::Borrowed(text);
    }
    let Some(master) = master_range(&source) else { return Cow::Borrowed(text) };
    // The master's own lines, its alarms left out.
    let mut own = Vec::new();
    let mut nested = 0usize;
    for i in master.start + 1..master.end.saturating_sub(1) {
        match lines::name(&source[i]).as_str() {
            "BEGIN" => nested += 1,
            "END" => nested = nested.saturating_sub(1),
            _ if nested == 0 => own.push(i),
            _ => {}
        }
    }
    let find = |name: &str| own.iter().copied().find(|&i| lines::name(&source[i]) == name);
    let (Some(rule), Some(start)) = (find("RRULE"), find("DTSTART")) else { return Cow::Borrowed(text) };
    let parts: Vec<(String, String)> = lines::value(&source[rule]).split(';').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim().to_ascii_uppercase())).collect();
    let part = |name: &str| parts.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    let unit = match part("FREQ") {
        Some("SECONDLY") => 1,
        Some("MINUTELY") => 60,
        Some("HOURLY") => 3_600,
        Some("DAILY") => 86_400,
        Some("WEEKLY") => 7 * 86_400,
        _ => return Cow::Borrowed(text),
    };
    if part("COUNT").is_some() {
        return Cow::Borrowed(text);
    }
    let step = unit * part("INTERVAL").and_then(|n| n.parse::<i64>().ok()).filter(|n| *n > 0).unwrap_or(1);
    // A time as written, on its own clock (TZID, UTC or none): moved as calcard counts, on that clock.
    let civil = |line: &str| DateTime::strptime("%Y%m%dT%H%M%S", lines::value(line).trim().trim_end_matches(['Z', 'z'])).ok();
    let Some(begins) = civil(&source[start]) else { return Cow::Borrowed(text) };
    let end = find("DTEND");
    let length = match (end.and_then(|i| civil(&source[i])), find("DURATION")) {
        (Some(ends), _) => begins.duration_until(ends).as_secs().max(0),
        (None, Some(i)) => lines::value(&source[i]).trim().parse::<jiff::Span>().ok().and_then(|span| span.total((jiff::Unit::Second, begins)).ok()).map_or(0, |s| s.max(0.0) as i64),
        (None, None) => 0,
    };
    // `from` on any clock: within fourteen hours of UTC; two days spare.
    let Some(window) = Timestamp::from_second(from).ok().map(|t| t.to_zoned(TimeZone::UTC).datetime()) else { return Cow::Borrowed(text) };
    let before = window.checked_sub(jiff::SignedDuration::from_secs(2 * 86_400 + length + step)).unwrap_or(window);
    let ahead = begins.duration_until(before).as_secs();
    let periods = ahead.div_euclid(step);
    if periods <= 0 {
        return Cow::Borrowed(text);
    }
    let moved = jiff::SignedDuration::from_secs(periods.saturating_mul(step));
    let rewrite = |line: &str, at: DateTime| {
        let value = lines::value(line).trim();
        let utc = if value.ends_with(['Z', 'z']) { "Z" } else { "" };
        format!("{}:{}{utc}", head_of(line), at.strftime("%Y%m%dT%H%M%S"))
    };
    let Ok(new_start) = begins.checked_add(moved) else { return Cow::Borrowed(text) };
    source[start] = rewrite(&source[start], new_start);
    if let Some(i) = end
        && let Some(ends) = civil(&source[i])
        && let Ok(new_end) = ends.checked_add(moved)
    {
        source[i] = rewrite(&source[i], new_end);
    }
    Cow::Owned(lines::fold(&source))
}

/// The occurrences of one file's events between `from` and `to` (Unix seconds);
/// times written without a zone are taken in `zone`, yours. Expanded from just
/// before `from` (`from_window`), not from each event's start.
pub fn file_occurrences(path: &Path, calendar: &Collection, from: i64, to: i64, zone: &TimeZone) -> Vec<Occurrence> {
    let Some(text) = std::fs::read_to_string(path).ok() else { return Vec::new() };
    let Some(ical) = parse(&for_expansion(&from_window(&text, from))) else { return Vec::new() };
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
            transparent: text_of(component, &ICalendarProperty::Transp).eq_ignore_ascii_case("TRANSPARENT"),
            declined: false,
            organizer,
            attendees,
            read_only: calendar.read_only,
            alarms: alarms_of(&ical, component, start, end - start),
            margins,
            demands,
            task: text_of(component, &ICalendarProperty::Other(crate::blocks::TASK.to_string())),
            turn: text_of(component, &ICalendarProperty::Other(crate::blocks::TURN.to_string())),
            remind: remind_of(&ical, component),
        });
    }
    found
}

/// Everything between `from` and `to`, from every calendar, by start; those
/// you declined marked (`declined`), by your accounts' addresses.
pub fn occurrences(from: i64, to: i64) -> Vec<Occurrence> {
    let zone = TimeZone::system();
    let mut all: Vec<Occurrence> = vdir::collections(Kind::Calendars)
        .iter()
        .flat_map(|calendar| calendar.items().into_iter().flat_map(|path| file_occurrences(&path, calendar, from, to, &zone)).collect::<Vec<_>>())
        .collect();
    if all.iter().any(|e| e.attendees.iter().any(|a| a.answer == "declined")) {
        let own: Vec<String> = crate::config::Config::load(&crate::config::default_path()).map(|c| c.every_account().filter_map(|a| a.address.clone()).collect()).unwrap_or_default();
        mark_declined(&mut all, &own);
    }
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
    /// Its reminder before: "" as usual, "none" not this one, "15" minutes before (`reminders::REMIND`).
    #[serde(default)]
    pub remind: String,
}

/// An event's reminder before (`reminders::REMIND`), as the form says it:
/// "" as usual, "none", or minutes ("15").
fn remind_word(value: &str) -> String {
    match crate::reminders::Remind::read(value) {
        crate::reminders::Remind::Usual => String::new(),
        crate::reminders::Remind::Never => "none".to_string(),
        crate::reminders::Remind::Minutes(minutes) => minutes.to_string(),
    }
}

/// What an occurrence says of its reminder: its own, else, a changed one
/// (RECURRENCE-ID) saying nothing, its series'.
fn remind_of(ical: &ICalendar, component: &ICalendarComponent) -> String {
    let name = ICalendarProperty::Other(crate::reminders::REMIND.to_string());
    let own = text_of(component, &name);
    if !own.is_empty() || component.property(&ICalendarProperty::RecurrenceId).is_none() {
        return own;
    }
    ical.components
        .iter()
        .find(|c| c.component_type == ICalendarComponentType::VEvent && c.property(&ICalendarProperty::RecurrenceId).is_none() && c.uid() == component.uid())
        .map(|series| text_of(series, &name))
        .unwrap_or_default()
}

/// An event as the form shows it, from its file, in your time zone.
pub fn edit_of(path: &Path) -> Option<EventEdit> {
    edit_of_text(&std::fs::read_to_string(path).ok()?, &TimeZone::system())
}

/// An event as the form shows it, from its text, in `zone`.
pub fn edit_of_text(text: &str, zone: &TimeZone) -> Option<EventEdit> {
    let ical = parse(&for_expansion(text))?;
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
        remind: remind_word(&text_of(master, &ICalendarProperty::Other(crate::reminders::REMIND.to_string()))),
    })
}

/// The main event's first occurrence still there, as the agenda shows it:
/// (start, end) in Unix seconds, to the second; None when the text holds no
/// event. Times without a zone are in `zone`.
pub fn first_times(text: &str, zone: &TimeZone) -> Option<(i64, i64)> {
    let ical = parse(&for_expansion(text))?;
    let (index, _) = ical.components.iter().enumerate().find(|(_, c)| c.component_type == ICalendarComponentType::VEvent && c.property(&ICalendarProperty::RecurrenceId).is_none())?;
    let first = ical.expand_dates(calcard_zone(zone), FIRST_LOOKED_AT).events.into_iter().find(|e| e.comp_id as usize == index)?;
    let start = first.start.timestamp();
    let end = match first.end {
        TimeOrDelta::Time(end) => end.timestamp(),
        TimeOrDelta::Delta(delta) => start + delta.num_seconds(),
    };
    Some((start, end))
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
    remind: bool,
}

impl Changed {
    const ALL: Changed = Changed { title: true, location: true, notes: true, rule: true, margins: true, demands: true, remind: true };
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
    if changed.remind
        && let Some(value) = crate::reminders::Remind::read(&edit.remind).value()
    {
        out.push(format!("{}:{value}", crate::reminders::REMIND));
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
        Some(b) => Changed { title: b.title.trim() != edit.title.trim(), location: b.location.trim() != edit.location.trim(), notes: b.notes.trim() != edit.notes.trim(), rule: rule_changed, margins: b.margins != edit.margins, demands: b.demands != edit.demands, remind: crate::reminders::Remind::read(&b.remind) != crate::reminders::Remind::read(&edit.remind) },
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
            || (changed.remind && name == crate::reminders::REMIND)
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
/// One changed already (RECURRENCE-ID: moved by a drag in the agenda, or by another
/// application) goes with its change: its own VEVENT is taken out, and the time it
/// replaced in the series is left out.
pub fn skip_occurrence(text: &str, start: i64) -> Option<String> {
    let zone = TimeZone::system();
    let mut source = lines::unfold(text);
    if let Some((place, true)) = occurrence_at(text, start, &zone) {
        let range = event_ranges(&source).get(place)?.clone();
        let id = source[range.clone()].iter().find(|l| lines::name(l) == "RECURRENCE-ID")?.clone();
        // Its parameters but RANGE, which an EXDATE has not.
        let params: String = lines::params(&id).into_iter().filter(|(name, _)| name != "RANGE").map(|(name, value)| format!(";{name}={}", lines::param_value(&value))).collect();
        let value = lines::value(&id).trim().to_string();
        source.drain(range);
        let master = master_range(&source)?;
        source.insert(master.end - 1, format!("EXDATE{params}:{value}"));
        return Some(lines::fold(&source));
    }
    let at = Timestamp::from_second(start).ok()?;
    let master = master_range(&source)?;
    let dtstart = source[master.clone()].iter().find(|l| lines::name(l) == "DTSTART")?.clone();
    let end = master.end - 1;
    source.insert(end, written_like(&dtstart, "EXDATE", at, &zone));
    Some(lines::fold(&source))
}

/// Which occurrence of a file's event starts at `start` (Unix seconds), as the
/// agenda shows it: the place of the VEVENT holding it among the text's
/// VEVENTs (calcard keeps them in the text's order), and whether that one is a
/// changed occurrence (RECURRENCE-ID). Times without a zone are in `zone`.
fn occurrence_at(text: &str, start: i64, zone: &TimeZone) -> Option<(usize, bool)> {
    let ical = parse(&for_expansion(&from_window(text, start)))?;
    let events: Vec<usize> = ical.components.iter().enumerate().filter(|(_, c)| c.component_type == ICalendarComponentType::VEvent).map(|(i, _)| i).collect();
    let found = ical.expand_dates(calcard_zone(zone), EXPANSION_LIMIT).events.into_iter().find(|e| e.start.timestamp() == start && events.contains(&(e.comp_id as usize)))?;
    let place = events.iter().position(|&i| i == found.comp_id as usize)?;
    Some((place, ical.components[found.comp_id as usize].property(&ICalendarProperty::RecurrenceId).is_some()))
}

/// The text's VEVENTs, BEGIN to END, in order (their alarms inside them).
fn event_ranges(source: &[String]) -> Vec<std::ops::Range<usize>> {
    let (mut out, mut start, mut nested) = (Vec::new(), None, 0usize);
    for (i, line) in source.iter().enumerate() {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        match (name.as_str(), start) {
            ("BEGIN", None) if value == "VEVENT" => {
                start = Some(i);
                nested = 0;
            }
            ("BEGIN", Some(_)) => nested += 1,
            ("END", Some(_)) if nested > 0 => nested -= 1,
            ("END", Some(begin)) if value == "VEVENT" => {
                out.push(begin..i + 1);
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// A line's name and parameters, as written: "DTSTART;TZID=Europe/Paris".
fn head_of(line: &str) -> &str {
    line.strip_suffix(lines::value(line)).and_then(|h| h.strip_suffix(':')).unwrap_or(line)
}

/// `at` as `like` writes its time (a DTSTART line): a date, UTC, or the clock
/// in its TZID; a time without a zone, on the clock in `zone`.
fn written_value(like: &str, at: Timestamp, zone: &TimeZone) -> String {
    let value = lines::value(like).trim();
    if value.len() == 8 {
        at.to_zoned(zone.clone()).strftime("%Y%m%d").to_string()
    } else if value.ends_with(['Z', 'z']) {
        at.strftime("%Y%m%dT%H%M%SZ").to_string()
    } else {
        let own = lines::param(like, "TZID").and_then(|t| TimeZone::get(t.trim_start_matches('/')).ok()).unwrap_or_else(|| zone.clone());
        at.to_zoned(own).strftime("%Y%m%dT%H%M%S").to_string()
    }
}

/// A line `name` for `at`, with `like`'s parameters, written as `like` writes its time.
fn written_like(like: &str, name: &str, at: Timestamp, zone: &TimeZone) -> String {
    let head = head_of(like);
    let params = head.find(';').map_or("", |i| &head[i..]);
    format!("{name}{params}:{}", written_value(like, at, zone))
}

/// The moment one value of `line` means (a DATE-TIME list's item): in its
/// TZID, UTC with a Z, a date at midnight; a time without a zone, in `zone`.
fn moment_in(line: &str, value: &str, zone: &TimeZone) -> Option<Zoned> {
    let value = value.trim();
    if value.len() == 8 {
        return Date::strptime("%Y%m%d", value).ok()?.to_zoned(zone.clone()).ok();
    }
    if let Some(utc) = value.strip_suffix(['Z', 'z']) {
        return DateTime::strptime("%Y%m%dT%H%M%S", utc).ok()?.to_zoned(TimeZone::UTC).ok().map(|z| z.with_time_zone(zone.clone()));
    }
    let own = lines::param(line, "TZID").and_then(|t| TimeZone::get(t.trim_start_matches('/')).ok()).unwrap_or_else(|| zone.clone());
    DateTime::strptime("%Y%m%dT%H%M%S", value).ok()?.to_zoned(own).ok().map(|z| z.with_time_zone(zone.clone()))
}

/// A VEVENT's lines with its own times set: `from` to `to`, written as its
/// DTSTART was (`like`), DURATION replaced by DTEND; its stamps written again.
/// Its SEQUENCE goes one up (RFC 5545 §3.8.7.4), unless `recurrence` makes it a
/// new changed occurrence of the series: then it keeps the series' own, as
/// readers that match the two by it want (calcard). Its alarms are kept as
/// they are; `drop` names the other lines left out (a new changed occurrence
/// leaves the rule).
fn retimed(block: &[String], like: &str, from: Timestamp, to: Timestamp, recurrence: Option<String>, drop: &[&str], zone: &TimeZone) -> Vec<String> {
    let kept = recurrence.is_some();
    let sequence = block.iter().find(|l| lines::name(l) == "SEQUENCE").and_then(|l| lines::value(l).trim().parse::<u32>().ok());
    let last = block.len().saturating_sub(1);
    let mut nested = 0usize;
    let mut out: Vec<String> = Vec::with_capacity(block.len() + 6);
    for (i, line) in block.iter().enumerate() {
        if i == 0 || i == last {
            out.push(line.clone());
            continue;
        }
        match lines::name(line).as_str() {
            "BEGIN" => nested += 1,
            "END" => nested = nested.saturating_sub(1),
            name if nested == 0 && (matches!(name, "DTSTART" | "DTEND" | "DURATION" | "DTSTAMP" | "LAST-MODIFIED") || (name == "SEQUENCE" && !kept) || drop.contains(&name)) => continue,
            _ => {}
        }
        out.push(line.clone());
    }
    let now = utc(&Zoned::now());
    let mut own = Vec::new();
    own.extend(recurrence);
    own.extend([written_like(like, "DTSTART", from, zone), written_like(like, "DTEND", to, zone), format!("DTSTAMP:{now}"), format!("LAST-MODIFIED:{now}")]);
    if !kept {
        own.push(format!("SEQUENCE:{}", sequence.unwrap_or(0).saturating_add(1)));
    }
    // Before its alarms, else before its end.
    let at = out.iter().enumerate().skip(1).find(|(_, l)| lines::name(l) == "BEGIN").map_or(out.len().saturating_sub(1), |(i, _)| i);
    out.splice(at..at, own);
    out
}

/// A repeating event's rule, its occurrences moved by `span` on the clock: a
/// weekly rule's days (BYDAY=MO,WE) move with them, and its end (UNTIL) by as
/// much. None when the move would not keep what the rule says: days set
/// otherwise (a day of the month, "the first Monday") moved to another day,
/// or hours set in the rule (BYHOUR) moved to another time.
fn rule_moved(rule: &str, span: jiff::Span, zone: &TimeZone) -> Option<String> {
    let days = span.get_days();
    let timed = span.get_hours() != 0 || span.get_minutes() != 0 || span.get_seconds() != 0;
    let weekly = rule.to_ascii_uppercase().split(';').any(|p| p.trim() == "FREQ=WEEKLY");
    const WEEK: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
    let mut parts = Vec::new();
    for part in rule.split(';') {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        let upper = key.trim().to_ascii_uppercase();
        let moved = match upper.as_str() {
            "BYDAY" if days == 0 => part.to_string(),
            "BYDAY" if weekly => {
                let shifted: Option<Vec<&str>> = value.split(',').map(|d| WEEK.iter().position(|w| w.eq_ignore_ascii_case(d.trim())).map(|i| WEEK[(i as i64 + i64::from(days)).rem_euclid(7) as usize])).collect();
                format!("{key}={}", shifted?.join(","))
            }
            "BYDAY" | "BYMONTHDAY" | "BYYEARDAY" | "BYWEEKNO" | "BYMONTH" | "BYSETPOS" if days != 0 => return None,
            "BYHOUR" | "BYMINUTE" | "BYSECOND" if timed => return None,
            "UNTIL" => {
                let until = moment_in("", value, zone)?;
                let shifted = until.datetime().checked_add(span).ok()?.to_zoned(zone.clone()).ok()?;
                let written = if value.trim().len() == 8 { shifted.strftime("%Y%m%d").to_string() } else { shifted.timestamp().strftime("%Y%m%dT%H%M%SZ").to_string() };
                format!("{key}={written}")
            }
            _ => part.to_string(),
        };
        parts.push(moved);
    }
    Some(parts.join(";"))
}

/// Why an event stayed where it was (`moved`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveProblem {
    /// No occurrence starts at that time any more: changed meanwhile, here or elsewhere.
    Gone,
    /// Every time, to other days, for a rule that sets its days other than by
    /// the week ("the first Monday", "the 5th"): its form says how it repeats.
    SetDays,
    /// The file or a time does not read.
    Unreadable(String),
}

/// The event moved as a drag in the agenda asks: its occurrence starting at
/// `start` (Unix seconds, as shown) now from `new_start` to `new_end`. A single
/// event takes the new times. A repeating one moves that time alone
/// (`only_this`: a changed occurrence, RECURRENCE-ID, RFC 5545 §3.8.4.4; one
/// changed already is changed again), or every time by as much on the clock:
/// its first time, the times left out (EXDATE) and added (RDATE), and the
/// times its changed occurrences replace go together, a weekly rule's days
/// with them; the changed occurrences keep their own times, but the one moved.
/// What Sioul does not edit is kept (`apply`); times without a zone are in `zone`.
pub fn moved(text: &str, start: i64, new_start: i64, new_end: i64, only_this: bool, zone: &TimeZone) -> Result<String, MoveProblem> {
    let unreadable = |e: &dyn std::fmt::Display| MoveProblem::Unreadable(e.to_string());
    let (place, changed) = occurrence_at(text, start, zone).ok_or(MoveProblem::Gone)?;
    let (from, to) = (Timestamp::from_second(new_start).map_err(|e| unreadable(&e))?, Timestamp::from_second(new_end).map_err(|e| unreadable(&e))?);
    if to <= from {
        return Err(MoveProblem::Unreadable(format!("{new_start}–{new_end}")));
    }
    let mut source = lines::unfold(text);
    let master = master_range(&source).ok_or(MoveProblem::Gone)?;
    let repeats = source[master.clone()].iter().any(|l| matches!(lines::name(l).as_str(), "RRULE" | "RDATE"));
    let ranges = event_ranges(&source);
    // That time alone: its changed occurrence, made or changed again.
    if changed && (only_this || !repeats) {
        let range = ranges.get(place).cloned().ok_or(MoveProblem::Gone)?;
        let like = source[range.clone()].iter().find(|l| lines::name(l) == "DTSTART").cloned().ok_or(MoveProblem::Gone)?;
        let block = retimed(&source[range.clone()], &like, from, to, None, &[], zone);
        source.splice(range, block);
        return Ok(lines::fold(&source));
    }
    if repeats && only_this {
        let like = source[master.clone()].iter().find(|l| lines::name(l) == "DTSTART").cloned().ok_or(MoveProblem::Gone)?;
        let instance = Timestamp::from_second(start).map_err(|e| unreadable(&e))?;
        let recurrence = written_like(&like, "RECURRENCE-ID", instance, zone);
        let block = retimed(&source[master.clone()], &like, from, to, Some(recurrence), &["RRULE", "RDATE", "EXDATE", "EXRULE", "RECURRENCE-ID"], zone);
        source.splice(master.end..master.end, block);
        return Ok(lines::fold(&source));
    }
    let mut edit = edit_of_text(text, zone).ok_or(MoveProblem::Gone)?;
    let local = |at: Timestamp| at.to_zoned(zone.clone()).strftime("%Y-%m-%dT%H:%M").to_string();
    if !repeats {
        edit.start = local(from);
        edit.end = local(to);
        return apply(text, &edit, zone).map_err(|e| unreadable(&e));
    }
    // Every time: by as much on the clock as the one dragged (days, then hours).
    let was = Timestamp::from_second(start).map_err(|e| unreadable(&e))?.to_zoned(zone.clone()).datetime();
    let span = was.until((jiff::Unit::Day, from.to_zoned(zone.clone()).datetime())).map_err(|e| unreadable(&e))?;
    let shift = |at: &Zoned| -> Option<Timestamp> { at.datetime().checked_add(span).ok()?.to_zoned(zone.clone()).ok().map(|z| z.timestamp()) };
    // A rule that would not keep what it says stops here: nothing is written.
    for line in source[master.clone()].iter().filter(|l| lines::name(l) == "RRULE") {
        rule_moved(lines::value(line).trim(), span, zone).ok_or(MoveProblem::SetDays)?;
    }
    let first: DateTime = edit.start.parse().map_err(|e| unreadable(&e))?;
    let first = first.checked_add(span).map_err(|e| unreadable(&e))?;
    let last = first.checked_add(jiff::Span::new().seconds(new_end - new_start)).map_err(|e| unreadable(&e))?;
    edit.start = first.strftime("%Y-%m-%dT%H:%M").to_string();
    edit.end = last.strftime("%Y-%m-%dT%H:%M").to_string();
    // Its first time and its length, as the form writes them; then, around them,
    // the rule's days, the times left out and added, the times changed occurrences replace.
    let written = apply(text, &edit, zone).map_err(|e| unreadable(&e))?;
    let mut source = lines::unfold(&written);
    let master = master_range(&source).ok_or(MoveProblem::Gone)?;
    for range in event_ranges(&source) {
        let own = range.clone();
        let mut nested = 0usize;
        for i in own.clone() {
            let name = lines::name(&source[i]);
            match name.as_str() {
                "BEGIN" if i > own.start => nested += 1,
                "END" if i + 1 < own.end => nested = nested.saturating_sub(1),
                _ => {}
            }
            if nested > 0 {
                continue;
            }
            let line = source[i].clone();
            if range == master && name == "RRULE" {
                let rule = rule_moved(lines::value(&line).trim(), span, zone).ok_or(MoveProblem::SetDays)?;
                source[i] = format!("{}:{rule}", head_of(&line));
                continue;
            }
            let wanted = if range == master { matches!(name.as_str(), "EXDATE" | "RDATE") } else { name == "RECURRENCE-ID" };
            if !wanted {
                continue;
            }
            let values: Option<Vec<String>> = lines::value(&line).split(',').map(|v| moment_in(&line, v, zone).and_then(|z| shift(&z)).map(|at| written_value(&line, at, zone))).collect();
            if let Some(values) = values {
                source[i] = format!("{}:{}", head_of(&line), values.join(","));
            }
        }
    }
    // The one dragged, changed already: its own times too.
    if changed {
        let range = event_ranges(&source).get(place).cloned().ok_or(MoveProblem::Gone)?;
        let like = source[range.clone()].iter().find(|l| lines::name(l) == "DTSTART").cloned().ok_or(MoveProblem::Gone)?;
        let block = retimed(&source[range.clone()], &like, from, to, None, &[], zone);
        source.splice(range, block);
    }
    Ok(lines::fold(&source))
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

/// What an event file ties to: its links (LINK, RELATED-TO), its projects
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
    pub projects: Vec<String>,
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
            "REFID" => found.projects.push(lines::unescape(lines::value(line).trim())),
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

/// Where a new event goes: the first calendar that can be written to and takes
/// events, the one made for time blocks last (`blocks::CALENDAR`).
pub fn default_calendar() -> Option<Collection> {
    let writable: Vec<Collection> = vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only && c.holds("VEVENT")).collect();
    writable.iter().find(|c| c.id != crate::blocks::CALENDAR).or(writable.first()).cloned()
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

    /// A repeating event begun years ago reaches the stretch asked for: it is
    /// expanded from just before it (`from_window`), with the same occurrences
    /// there as an expansion from its true start without a cap would give:
    /// every hour, across the change of hour, every other week on two days,
    /// a day left out and a changed occurrence; a counted rule as it was.
    #[test]
    fn an_event_repeating_for_years_reaches_today() {
        let event = |extra: &str, start: &str, end: &str, rule: &str| {
            format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Demo//EN\r\nBEGIN:VEVENT\r\nUID:demo-repeat\r\nDTSTAMP:20190101T000000Z\r\nDTSTART;TZID=Europe/Paris:{start}\r\nDTEND;TZID=Europe/Paris:{end}\r\nRRULE:{rule}\r\nSUMMARY:Demo\r\n{extra}END:VEVENT\r\nEND:VCALENDAR\r\n")
        };
        // As calcard gives them from the true start, with no cap that matters here.
        let uncapped = |text: &str, from: i64, to: i64| -> Vec<(i64, i64)> {
            let ical = parse(&for_expansion(text)).unwrap();
            let mut out: Vec<(i64, i64)> = ical
                .expand_dates(calcard_zone(&paris()), 80_000)
                .events
                .into_iter()
                .map(|e| {
                    let start = e.start.timestamp();
                    let end = match e.end {
                        TimeOrDelta::Time(end) => end.timestamp(),
                        TimeOrDelta::Delta(delta) => start + delta.num_seconds(),
                    };
                    (start, end)
                })
                .filter(|(start, end)| *end > from && *start < to)
                .collect();
            out.sort_unstable();
            out
        };
        let read = |name: &str, text: &str, from: i64, to: i64| -> Vec<(i64, i64)> {
            let mut out: Vec<(i64, i64)> = file_occurrences(&write_temp(name, text), &calendar(), from, to, &paris()).into_iter().map(|o| (o.start, o.end)).collect();
            out.sort_unstable();
            out
        };
        // Every hour since 1 January 2019: 24 on Monday 5 October 2026 (68,000 hours on).
        let hourly = event("", "20190101T000000", "20190101T001500", "FREQ=HOURLY");
        let (monday, tuesday) = (at("2026-10-05T00:00"), at("2026-10-06T00:00"));
        let found = read("hourly.ics", &hourly, monday, tuesday);
        assert_eq!(found.len(), 24, "{found:?}");
        assert_eq!((found[0], found[23].0), ((monday, monday + 900), at("2026-10-05T23:00")));
        assert_eq!(found, uncapped(&hourly, monday, tuesday));
        // The night the clocks go back, 25 hours: as calcard counts them from the start.
        let (sunday, after) = (at("2026-10-25T00:00"), at("2026-10-26T00:00"));
        assert_eq!(read("hourly.ics", &hourly, sunday, after), uncapped(&hourly, sunday, after));
        // Every other week, Monday and Wednesday at 18:00, since 2019; one left out, one moved.
        let changed = "EXDATE;TZID=Europe/Paris:20261007T180000\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:demo-repeat\r\nRECURRENCE-ID;TZID=Europe/Paris:20261019T180000\r\nDTSTART;TZID=Europe/Paris:20261019T200000\r\nDTEND;TZID=Europe/Paris:20261019T210000\r\nSUMMARY:Demo later\r\n";
        let weekly = event(changed, "20190107T180000", "20190107T190000", "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE");
        let (october, november) = (at("2026-10-01T00:00"), at("2026-11-01T00:00"));
        let found = read("weekly.ics", &weekly, october, november);
        assert_eq!(found, uncapped(&weekly, october, november));
        assert!(found.contains(&(at("2026-10-19T20:00"), at("2026-10-19T21:00"))) && !found.iter().any(|o| o.0 == at("2026-10-07T18:00")), "{found:?}");
        // Every quarter of an hour: the cap alone would stop in 2019.
        let minutes = event("", "20190101T000000", "20190101T000500", "FREQ=MINUTELY;INTERVAL=15");
        let found = read("minutes.ics", &minutes, monday, tuesday);
        assert_eq!((found.len(), found.first().map(|o| o.0)), (96, Some(monday)));
        // Counted from its start: left as it was (all its 20 are long over).
        let counted = event("", "20190101T000000", "20190101T001500", "FREQ=HOURLY;COUNT=20");
        assert!(read("counted.ics", &counted, monday, tuesday).is_empty());
        assert_eq!(from_window(&counted, monday), counted.as_str());
        // A rule by the month reaches far enough from its start: left as it was.
        let monthly = event("", "19900115T090000", "19900115T100000", "FREQ=MONTHLY");
        assert_eq!(from_window(&monthly, monday), monthly.as_str());
        assert_eq!(read("monthly.ics", &monthly, october, november).len(), 1);
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

    /// Each occurrence between 1 and 27 October as (start, end, title).
    fn october(name: &str, text: &str) -> Vec<(i64, i64, String)> {
        let path = write_temp(name, text);
        file_occurrences(&path, &calendar(), at("2026-10-01T00:00"), at("2026-10-27T00:00"), &paris()).into_iter().map(|o| (o.start, o.end, o.summary)).collect()
    }

    #[test]
    fn a_single_event_moved_or_stretched() {
        let one = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:d-1\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:20261007T070000Z\r\n\
            DTEND:20261007T073000Z\r\nSUMMARY:Dentist\r\nX-APP-THING:keep\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        // Dragged from 9:00 to 10:15, its half hour kept; then its end to 11:00.
        let text = moved(one, at("2026-10-07T09:00"), at("2026-10-07T10:15"), at("2026-10-07T10:45"), false, &paris()).unwrap();
        assert!(text.contains("DTSTART:20261007T081500Z") && text.contains("DTEND:20261007T084500Z") && text.contains("SEQUENCE:1"), "{text}");
        assert!(text.contains("X-APP-THING:keep") && text.contains("SUMMARY:Dentist"), "{text}");
        let text = moved(&text, at("2026-10-07T10:15"), at("2026-10-07T10:15"), at("2026-10-07T11:00"), true, &paris()).unwrap();
        assert!(text.contains("DTEND:20261007T090000Z") && !text.contains("RECURRENCE-ID"), "{text}");
        // Changed meanwhile: no occurrence starts at 8:00 any more.
        assert_eq!(moved(one, at("2026-10-07T08:00"), at("2026-10-07T10:00"), at("2026-10-07T11:00"), false, &paris()), Err(MoveProblem::Gone));
    }

    #[test]
    fn one_time_of_a_repeating_event_moved() {
        // Monday 5 October's yoga to Tuesday 6 at 19:00, still an hour and a half; the others stay.
        let text = moved(WEEKLY, at("2026-10-05T18:00"), at("2026-10-06T19:00"), at("2026-10-06T20:30"), true, &paris()).unwrap();
        assert!(text.contains("RECURRENCE-ID;TZID=Europe/Paris:20261005T180000") && text.contains("DTSTART;TZID=Europe/Paris:20261006T190000"), "{text}");
        assert!(text.contains("RRULE:FREQ=WEEKLY;BYDAY=MO") && text.contains("DTSTART;TZID=Europe/Paris:20260105T180000"), "the series as it was: {text}");
        let yoga = |at_: &str, until: &str, title: &str| (at(at_), at(until), title.to_string());
        assert_eq!(october("moved-once.ics", &text), vec![yoga("2026-10-06T19:00", "2026-10-06T20:30", "Yoga"), yoga("2026-10-19T19:00", "2026-10-19T20:30", "Yoga (later)"), yoga("2026-10-26T18:00", "2026-10-26T19:30", "Yoga")]);
        // Its alarm, and what Sioul does not know, go with it.
        assert_eq!((text.matches("TRIGGER:-PT15M").count(), text.matches("X-APP-THING:keep").count()), (2, 2), "{text}");
        assert!(parse(&text).is_some());
        // Moved again: the same changed occurrence, not a second one.
        let again = moved(&text, at("2026-10-06T19:00"), at("2026-10-06T20:00"), at("2026-10-06T21:00"), true, &paris()).unwrap();
        assert_eq!(again.matches("RECURRENCE-ID").count(), 2, "the 5th's and the 19th's: {again}");
        assert!(again.contains("DTSTART;TZID=Europe/Paris:20261006T200000") && !again.contains("20261006T190000"), "{again}");
        // Left out: the changed occurrence goes, and the time it replaced is left out of the series.
        let skipped = skip_occurrence(&again, at("2026-10-06T20:00")).unwrap();
        assert!(skipped.contains("EXDATE;TZID=Europe/Paris:20261005T180000") && skipped.matches("RECURRENCE-ID").count() == 1, "{skipped}");
        assert_eq!(october("moved-skipped.ics", &skipped).first().map(|o| o.0), Some(at("2026-10-19T19:00")));
    }

    #[test]
    fn every_time_of_a_repeating_event_moved() {
        // Half an hour later every time: its first time, the time left out and the time the changed one replaces go along.
        let text = moved(WEEKLY, at("2026-10-05T18:00"), at("2026-10-05T18:30"), at("2026-10-05T20:00"), false, &paris()).unwrap();
        assert!(text.contains("DTSTART;TZID=Europe/Paris:20260105T183000") && text.contains("EXDATE;TZID=Europe/Paris:20261012T183000"), "{text}");
        assert!(text.contains("RECURRENCE-ID;TZID=Europe/Paris:20261019T183000") && text.contains("RRULE:FREQ=WEEKLY;BYDAY=MO"), "{text}");
        let starts = |name: &str, text: &str| october(name, text).into_iter().map(|o| (o.0, o.2)).collect::<Vec<_>>();
        assert_eq!(starts("moved-all.ics", &text), vec![(at("2026-10-05T18:30"), "Yoga".into()), (at("2026-10-19T19:00"), "Yoga (later)".into()), (at("2026-10-26T18:30"), "Yoga".into())]);
        // The one changed already, dragged: every time half an hour later, and it where it was dropped.
        let text = moved(WEEKLY, at("2026-10-19T19:00"), at("2026-10-19T19:30"), at("2026-10-19T21:00"), false, &paris()).unwrap();
        assert_eq!(starts("moved-all-changed.ics", &text), vec![(at("2026-10-05T18:30"), "Yoga".into()), (at("2026-10-19T19:30"), "Yoga (later)".into()), (at("2026-10-26T18:30"), "Yoga".into())]);
        // To Tuesdays: a weekly rule's day goes along.
        let text = moved(WEEKLY, at("2026-10-05T18:00"), at("2026-10-06T18:00"), at("2026-10-06T19:30"), false, &paris()).unwrap();
        assert!(text.contains("RRULE:FREQ=WEEKLY;BYDAY=TU") && text.contains("DTSTART;TZID=Europe/Paris:20260106T180000"), "{text}");
        assert_eq!(starts("moved-tuesdays.ics", &text).first().map(|o| o.0), Some(at("2026-10-06T18:00")));
        // The first Monday of the month, to another day: its form says how it repeats.
        let monthly = WEEKLY.replace("RRULE:FREQ=WEEKLY;BYDAY=MO", "RRULE:FREQ=MONTHLY;BYDAY=1MO");
        assert_eq!(moved(&monthly, at("2026-10-05T18:00"), at("2026-10-06T18:00"), at("2026-10-06T19:30"), false, &paris()), Err(MoveProblem::SetDays));
        assert!(moved(&monthly, at("2026-10-05T18:00"), at("2026-10-05T19:00"), at("2026-10-05T20:30"), false, &paris()).is_ok(), "another time the same day");
    }
}
