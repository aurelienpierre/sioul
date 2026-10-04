// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Working hours: the days and hours work can reach you, each day on or off,
//! from a start to an end. Mail keeps arriving in the background; the Porch
//! shows it only in these hours and always says when they open again; outside
//! them, quiet time (`quiet.rs`). Predictable looks, not silence: in a field
//! trial, people given no notifications at all felt more anxious, while
//! batching them three times a day helped (Fitz et al. 2019; docs/research.md).
//! The same rows say when offices are open, for tasks that need one.

use jiff::civil::{Date, Weekday};
use jiff::{SignedDuration, Span, Zoned};
use serde::Deserialize;

/// One day's hours, as written in the configuration: `day`, `start`, `end`
/// ("tuesday", "09:00", "17:00"); older files say `minutes` instead of `end`.
#[derive(Debug, Clone, Deserialize)]
pub struct AdminWindow {
    /// "tuesday", "tue" or "mardi".
    pub day: String,
    /// "09:00".
    pub start: String,
    /// "17:00"; before the start, it ends the next day.
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default)]
    pub minutes: u32,
    /// What the hours are for: "work" when unsaid, "admin" or "leisure" (docs/areas.md).
    #[serde(default)]
    pub kind: Option<String>,
}

impl AdminWindow {
    /// What its hours are for: "work", "admin" or "leisure".
    pub fn kind(&self) -> &str {
        match self.kind.as_deref().map(str::trim) {
            Some("admin") => "admin",
            Some("leisure") => "leisure",
            _ => "work",
        }
    }

    /// This window's opening on `date`, if `date` is its weekday.
    fn opening_on(&self, date: Date, now: &Zoned) -> Option<Zoned> {
        let (hour, minute) = parse_time(&self.start)?;
        if parse_weekday(&self.day)? != date.weekday() {
            return None;
        }
        // "24:00" ends a day; it opens none.
        let time = jiff::civil::Time::new(hour, minute, 0, 0).ok()?;
        date.to_datetime(time).to_zoned(now.time_zone().clone()).ok()
    }

    /// When it closes after `opening`: its end as the clock says it, the next
    /// day when it comes before the start ("24:00" is midnight), so a change
    /// of hour in between moves nothing; older windows, `minutes` after.
    fn closing(&self, opening: &Zoned) -> Option<Zoned> {
        let Some((hour, minute)) = self.end.as_deref().and_then(parse_time) else {
            return opening.checked_add(SignedDuration::from_mins(i64::from(self.minutes))).ok();
        };
        let (from_hour, from_minute) = parse_time(&self.start)?;
        let later = i32::from(hour) * 60 + i32::from(minute) > i32::from(from_hour) * 60 + i32::from(from_minute);
        let day = if later { opening.date() } else { opening.date().tomorrow().ok()? };
        let (day, hour) = if hour == 24 { (day.tomorrow().ok()?, 0) } else { (day, hour) };
        let closing = day.to_datetime(jiff::civil::Time::new(hour, minute, 0, 0).ok()?).to_zoned(opening.time_zone().clone()).ok()?;
        (closing.timestamp() > opening.timestamp()).then_some(closing)
    }

    /// Its weekday, when the day reads as one.
    pub fn weekday(&self) -> Option<Weekday> {
        parse_weekday(&self.day)
    }

    /// Its end, "17:00", from `end` or `minutes`.
    pub fn end_text(&self) -> String {
        if let Some(end) = self.end.clone().filter(|e| parse_time(e).is_some()) {
            return end;
        }
        let Some((hour, minute)) = parse_time(&self.start) else { return String::new() };
        let total = (i64::from(hour) * 60 + i64::from(minute) + i64::from(self.minutes)) % (24 * 60);
        format!("{:02}:{:02}", total / 60, total % 60)
    }
}

/// Whether `windows` have any hours on `date`'s weekday.
pub fn open_day(windows: &[AdminWindow], date: Date) -> bool {
    windows.iter().any(|w| w.weekday() == Some(date.weekday()))
}

/// Offices' usual hours when the configuration says none: Monday to Friday, 9:00 to 17:00.
pub fn default_office_hours() -> Vec<AdminWindow> {
    ["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: None }).collect()
}

/// The days a set of hours opens, Monday first.
pub fn days_open(windows: &[AdminWindow]) -> [bool; 7] {
    let mut days = [false; 7];
    for day in windows.iter().filter_map(AdminWindow::weekday) {
        days[day.to_monday_zero_offset() as usize] = true;
    }
    days
}

const SHORT: [&str; 7] = ["mo", "tu", "we", "th", "fr", "sa", "su"];
const NAMES: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

/// "mo", "lu", "monday", "lundi": a day, Monday 0.
fn day_index(text: &str) -> Option<usize> {
    let text = text.trim().to_lowercase();
    let french = ["lu", "ma", "me", "je", "ve", "sa", "di"];
    SHORT.iter().position(|d| *d == text).or_else(|| french.iter().position(|d| *d == text)).or_else(|| parse_weekday(&text).map(|w| w.to_monday_zero_offset() as usize))
}

/// "09:00", from "9:00" or "9h00" or "9".
fn tidy_time(text: &str) -> Option<String> {
    let text = text.trim().to_lowercase().replace('h', ":");
    let (hour, minute) = match text.split_once(':') {
        Some((h, m)) => (h.parse::<u8>().ok()?, if m.is_empty() { 0 } else { m.parse::<u8>().ok()? }),
        None => (text.parse::<u8>().ok()?, 0),
    };
    (hour <= 24 && minute < 60).then(|| format!("{hour:02}:{minute:02}"))
}

/// Opening hours written short, as a task keeps its office's: "mo-fr 09:00-17:00",
/// "mo-fr 09:00-12:00, 14:00-17:00; sa 09:00-12:00". Days in two letters,
/// English or French ("lu-ve"), or named; a range, or a list with commas.
pub fn parse_ranges(text: &str) -> Vec<AdminWindow> {
    let mut out = Vec::new();
    for part in text.split(';') {
        let Some((days, hours)) = part.trim().split_once(char::is_whitespace) else { continue };
        let mut chosen = Vec::new();
        for group in days.split(',') {
            match group.split_once('-') {
                Some((from, to)) => {
                    if let (Some(from), Some(to)) = (day_index(from), day_index(to)) {
                        let mut day = from;
                        loop {
                            chosen.push(day);
                            if day == to {
                                break;
                            }
                            day = (day + 1) % 7;
                        }
                    }
                }
                None => chosen.extend(day_index(group)),
            }
        }
        for range in hours.split(',') {
            let Some((start, end)) = range.trim().split_once('-') else { continue };
            let (Some(start), Some(end)) = (tidy_time(start), tidy_time(end)) else { continue };
            for &day in &chosen {
                out.push(AdminWindow { day: NAMES[day].into(), start: start.clone(), end: Some(end.clone()), minutes: 0, kind: None });
            }
        }
    }
    out
}

/// Hours as `parse_ranges` reads them, days with the same hours together: "mo-fr 09:00-17:00".
pub fn ranges_text(windows: &[AdminWindow]) -> String {
    let mut hours: Vec<Vec<String>> = vec![Vec::new(); 7];
    for w in windows {
        if let Some(day) = w.weekday() {
            let range = format!("{}-{}", w.start.trim(), w.end_text());
            let slot = &mut hours[day.to_monday_zero_offset() as usize];
            if !slot.contains(&range) {
                slot.push(range);
            }
        }
    }
    for slot in &mut hours {
        slot.sort();
    }
    let mut parts = Vec::new();
    let mut day = 0;
    while day < 7 {
        if hours[day].is_empty() {
            day += 1;
            continue;
        }
        let mut last = day;
        while last + 1 < 7 && hours[last + 1] == hours[day] {
            last += 1;
        }
        let days = if last == day { SHORT[day].to_string() } else { format!("{}-{}", SHORT[day], SHORT[last]) };
        parts.push(format!("{days} {}", hours[day].join(", ")));
        day = last + 1;
    }
    parts.join("; ")
}

/// English and French day names, full or in three letters.
fn parse_weekday(day: &str) -> Option<Weekday> {
    match day.trim().to_lowercase().as_str() {
        "monday" | "mon" | "lundi" => Some(Weekday::Monday),
        "tuesday" | "tue" | "mardi" => Some(Weekday::Tuesday),
        "wednesday" | "wed" | "mercredi" => Some(Weekday::Wednesday),
        "thursday" | "thu" | "jeudi" => Some(Weekday::Thursday),
        "friday" | "fri" | "vendredi" => Some(Weekday::Friday),
        "saturday" | "sat" | "samedi" => Some(Weekday::Saturday),
        "sunday" | "sun" | "dimanche" => Some(Weekday::Sunday),
        _ => None,
    }
}

/// "09:30" → (9, 30); "24:00" ends a day; anything past it, or not a time, is none.
fn parse_time(text: &str) -> Option<(i8, i8)> {
    let (hour, minute) = text.trim().split_once(':')?;
    let (hour, minute): (i8, i8) = (hour.trim().parse().ok()?, minute.trim().parse().ok()?);
    ((0..24).contains(&hour) && (0..60).contains(&minute) || (hour, minute) == (24, 0)).then_some((hour, minute))
}

/// Each window of `date`, as (opening, closing), the earliest first.
pub fn spans_on(windows: &[AdminWindow], date: Date, zone: &jiff::tz::TimeZone) -> Vec<(Zoned, Zoned)> {
    let Ok(midnight) = date.to_zoned(zone.clone()) else { return Vec::new() };
    let mut spans: Vec<(Zoned, Zoned)> = windows
        .iter()
        .filter_map(|w| {
            let opening = w.opening_on(date, &midnight)?;
            let closing = w.closing(&opening)?;
            Some((opening, closing))
        })
        .collect();
    spans.sort_by_key(|(opening, _)| opening.timestamp());
    spans
}

/// Each window of `date` with what its hours are for, as (opening, closing,
/// kind: "work", "admin", "leisure"), the earliest first.
pub fn kinds_on<'a>(windows: &'a [AdminWindow], date: Date, zone: &jiff::tz::TimeZone) -> Vec<(Zoned, Zoned, &'a str)> {
    let Ok(midnight) = date.to_zoned(zone.clone()) else { return Vec::new() };
    let mut spans: Vec<(Zoned, Zoned, &str)> = windows
        .iter()
        .filter_map(|w| {
            let opening = w.opening_on(date, &midnight)?;
            let closing = w.closing(&opening)?;
            Some((opening, closing, w.kind()))
        })
        .collect();
    spans.sort_by_key(|(opening, _, _)| opening.timestamp());
    spans
}

/// The hours of `date`, from its first opening to its last closing, when it has any.
pub fn hours_on(windows: &[AdminWindow], date: Date, zone: &jiff::tz::TimeZone) -> Option<(Zoned, Zoned)> {
    let midnight = date.to_zoned(zone.clone()).ok()?;
    let spans: Vec<(Zoned, Zoned)> = windows
        .iter()
        .filter_map(|w| {
            let opening = w.opening_on(date, &midnight)?;
            let closing = w.closing(&opening)?;
            Some((opening, closing))
        })
        .collect();
    let opening = spans.iter().map(|(o, _)| o).min()?.clone();
    let closing = spans.iter().map(|(_, c)| c).max()?.clone();
    Some((opening, closing))
}

/// The window open right now, as (opening, closing).
pub fn current(windows: &[AdminWindow], now: &Zoned) -> Option<(Zoned, Zoned)> {
    // A window opened yesterday may still be open after midnight.
    let days = [now.date().yesterday().ok(), Some(now.date())];
    windows
        .iter()
        .flat_map(|w| days.iter().flatten().filter_map(move |d| {
            let opening = w.opening_on(*d, now)?;
            let closing = w.closing(&opening)?;
            Some((opening, closing))
        }))
        .find(|(opening, closing)| opening.timestamp() <= now.timestamp() && now.timestamp() < closing.timestamp())
}

/// When the next window opens, within the coming week.
pub fn next_opening(windows: &[AdminWindow], now: &Zoned) -> Option<Zoned> {
    (0..8)
        .filter_map(|d| now.date().checked_add(Span::new().days(d)).ok())
        .flat_map(|date| windows.iter().filter_map(move |w| w.opening_on(date, now)))
        .filter(|opening| opening.timestamp() > now.timestamp())
        .min_by_key(Zoned::timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windows() -> Vec<AdminWindow> {
        vec![
            AdminWindow { day: "mardi".into(), start: "10:00".into(), end: None, minutes: 45, kind: None },
            AdminWindow { day: "friday".into(), start: "10:00".into(), end: Some("10:45".into()), minutes: 0, kind: None },
        ]
    }

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    #[test]
    fn opening_hours_written_short() {
        let hours = parse_ranges("lu-ve 9h-12h, 14:00-17:00; sa 09:00-12:00");
        assert_eq!(hours.len(), 11);
        assert_eq!(ranges_text(&hours), "mo-fr 09:00-12:00, 14:00-17:00; sa 09:00-12:00");
        assert_eq!(days_open(&hours), [true, true, true, true, true, true, false]);
        assert_eq!(ranges_text(&parse_ranges("mo,we,fr 10:00-11:30")), "mo 10:00-11:30; we 10:00-11:30; fr 10:00-11:30");
        assert_eq!(ranges_text(&parse_ranges("sa-mo 08:00-09:00")), "mo 08:00-09:00; sa-su 08:00-09:00", "across the week's end");
        assert!(parse_ranges("whenever").is_empty());
        let saturday = at("2026-10-03T10:00[Europe/Paris]");
        assert!(current(&hours, &saturday).is_some());
        assert!(current(&hours, &at("2026-10-03T13:00[Europe/Paris]")).is_none(), "closed at lunch");
    }

    #[test]
    fn next_and_current() {
        let w = windows();
        // 2 October 2026 is a Friday.
        let morning = at("2026-10-02T09:00[Europe/Paris]");
        assert_eq!(next_opening(&w, &morning).unwrap().datetime().to_string(), "2026-10-02T10:00:00");
        assert!(current(&w, &morning).is_none());
        let during = at("2026-10-02T10:20[Europe/Paris]");
        assert_eq!(current(&w, &during).unwrap().1.datetime().to_string(), "2026-10-02T10:45:00");
        let after = at("2026-10-02T11:00[Europe/Paris]");
        assert_eq!(next_opening(&w, &after).unwrap().datetime().to_string(), "2026-10-06T10:00:00");
    }

    #[test]
    fn odd_hours_and_changes_of_hour() {
        // A task's office hours come from its file: "24:00" as a start, or no time at all, opens nothing.
        let odd = parse_ranges("mo 24:00-23:00; tu 9:75-10:00");
        let monday = at("2026-10-05T10:00[Europe/Paris]");
        assert!(current(&odd, &monday).is_none() && next_opening(&odd, &monday).is_none());
        let bad = AdminWindow { day: "monday".into(), start: "25:00".into(), end: Some("26:00".into()), minutes: 0, kind: None };
        assert!(spans_on(&[bad], monday.date(), monday.time_zone()).is_empty());
        // A night across the change of hour (Sunday 25 October 2026) ends at 6:00 on the clock; "24:00" is midnight.
        let night = AdminWindow { day: "saturday".into(), start: "22:00".into(), end: Some("06:00".into()), minutes: 0, kind: None };
        let evening = AdminWindow { day: "saturday".into(), start: "20:00".into(), end: Some("24:00".into()), minutes: 0, kind: None };
        let spans = spans_on(&[night, evening], "2026-10-24".parse().unwrap(), monday.time_zone());
        let ends: Vec<String> = spans.iter().map(|(_, c)| c.datetime().to_string()).collect();
        assert_eq!(ends, vec!["2026-10-25T00:00:00", "2026-10-25T06:00:00"]);
    }
}
