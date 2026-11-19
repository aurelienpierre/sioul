// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your days off, and why (docs/areas.md, "Vacations and holidays"; 18
//! November 2026): one answer for every reader that knows days off, the time
//! now (leisure), the plan (no room), "two working days before" (skipped),
//! work's reminders (they wait), the agenda (shaded), the pauses and the
//! day's review.
//!
//! Two sources:
//! - **your vacations**, the periods you type in Settings ▸ Hours
//!   (`[[time_off]]`, as before, a kind added: vacation, sick leave, other);
//! - **the whole days of each subscribed calendar of holidays that counts as
//!   your days off** (`subscribed::Subscription::days_off`): public holidays
//!   by default, school holidays when you say so. Each device knows them
//!   from its own copy of the calendar: until it has read it once, it does
//!   not know them.
//!
//! Nothing else reads the list of `[[time_off]]`: a test of the sources says so
//! (`nothing_reads_the_days_off_list_but_this`).

use crate::config::{Config, TimeOff};
use crate::subscribed::Holidays;
use jiff::civil::Date;
use std::collections::BTreeSet;

/// Why a day is a day off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// A vacation you typed.
    Vacation,
    /// Sick leave you typed.
    Sick,
    /// Another period you typed.
    Other,
    /// A holiday of a subscribed calendar that counts as your days off: its
    /// id and name, and which holidays it holds.
    Holiday { id: String, name: String, holidays: Holidays },
}

impl Why {
    /// A typed period's kind ("vacation", "sick", "other"); unsaid or unknown, a vacation.
    pub fn of_kind(kind: &str) -> Why {
        match kind.trim() {
            "sick" => Why::Sick,
            "other" => Why::Other,
            _ => Why::Vacation,
        }
    }

    /// Its word: "vacation", "sick", "other", "public", "school".
    pub fn id(&self) -> &'static str {
        match self {
            Why::Vacation => "vacation",
            Why::Sick => "sick",
            Why::Other => "other",
            Why::Holiday { holidays, .. } => holidays.id(),
        }
    }

    /// Typed by you, rather than read from a calendar.
    pub fn typed(&self) -> bool {
        !matches!(self, Why::Holiday { .. })
    }
}

/// One period of days off, both ends included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Period {
    pub from: Date,
    pub until: Date,
    /// Its word ("Summer", "Noël"); "" none.
    pub label: String,
    pub why: Why,
    /// A typed period's id (`TimeOff::id`), "" without one; a holiday's
    /// `<calendar id>:<its first day>`.
    pub id: String,
}

/// Your days off: every period that counts, typed ones first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DaysOff {
    periods: Vec<Period>,
}

impl DaysOff {
    /// Your days off as the configuration and this device's copies of your
    /// calendars of holidays say them now (`subscribed::holidays`), in the
    /// time zone shown.
    pub fn of(config: &Config) -> DaysOff {
        DaysOff::with_holidays(&config.time_off, &crate::subscribed::holidays(config, &crate::zone::current()))
    }

    /// Your typed periods alone, from the configuration: what a series pausing
    /// during "my vacations" reads (`series::Days`).
    pub fn typed_of(config: &Config) -> DaysOff {
        DaysOff::typed(&config.time_off)
    }

    /// Your typed periods alone (a list read elsewhere, the tests).
    pub fn typed(list: &[TimeOff]) -> DaysOff {
        DaysOff::with_holidays(list, &[])
    }

    /// Your typed periods, then the holidays of the calendars that count as your days off.
    pub fn with_holidays(list: &[TimeOff], holidays: &[crate::subscribed::HolidayDays]) -> DaysOff {
        let mut periods: Vec<Period> = list.iter().filter(|t| t.until >= t.from).map(|t| Period { from: t.from, until: t.until, label: t.label.clone(), why: Why::of_kind(&t.kind), id: t.id.clone() }).collect();
        for calendar in holidays.iter().filter(|h| h.days_off) {
            for (from, until, name) in &calendar.periods {
                periods.push(Period { from: *from, until: *until, label: name.clone(), why: Why::Holiday { id: calendar.id.clone(), name: calendar.name.clone(), holidays: calendar.holidays }, id: format!("{}:{from}", calendar.id) });
            }
        }
        DaysOff { periods }
    }

    /// Whether `date` is a day off, and why: the first period that covers
    /// it, a typed one before a holiday.
    pub fn on(&self, date: Date) -> Option<&Period> {
        self.periods.iter().find(|p| p.from <= date && date <= p.until)
    }

    /// No day off at all.
    pub fn is_empty(&self) -> bool {
        self.periods.is_empty()
    }

    /// Every period, typed ones first.
    pub fn periods(&self) -> &[Period] {
        &self.periods
    }

    /// Every day off from `from` to `until`, both included.
    pub fn days(&self, from: Date, until: Date) -> BTreeSet<Date> {
        let mut out = BTreeSet::new();
        for period in &self.periods {
            let mut day = period.from.max(from);
            while day <= period.until && day <= until {
                out.insert(day);
                let Ok(next) = day.tomorrow() else { break };
                day = next;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    /// A typed period counts before a holiday on the same day; a calendar
    /// that does not count as days off gives none.
    #[test]
    fn one_answer_for_a_day_off_and_why() {
        let typed = vec![TimeOff { id: "a".into(), from: day("2026-12-24"), until: day("2026-12-26"), label: "Family".into(), kind: "other".into() }];
        let public = crate::subscribed::HolidayDays { id: "fr".into(), name: "Jours fériés".into(), holidays: Holidays::Public, days_off: true, periods: vec![(day("2026-12-25"), day("2026-12-25"), "Noël".into()), (day("2027-01-01"), day("2027-01-01"), "Jour de l'an".into())], as_of: Some(1) };
        let school = crate::subscribed::HolidayDays { id: "zone".into(), name: "Zone A".into(), holidays: Holidays::School, days_off: false, periods: vec![(day("2026-12-20"), day("2027-01-03"), "Noël".into())], as_of: Some(1) };
        let off = DaysOff::with_holidays(&typed, &[public, school]);
        assert_eq!(off.on(day("2026-12-25")).map(|p| p.why.clone()), Some(Why::Other), "typed first");
        let new_year = off.on(day("2027-01-01")).unwrap();
        assert_eq!((new_year.label.as_str(), new_year.why.id()), ("Jour de l'an", "public"));
        assert!(off.on(day("2026-12-21")).is_none(), "school holidays count only when said");
        assert_eq!(off.days(day("2026-12-23"), day("2027-01-02")).len(), 4);
        assert_eq!(Why::of_kind("sick"), Why::Sick);
        assert_eq!(Why::of_kind("anything"), Why::Vacation);
    }

    /// Each reader of a day off asks the one function: a public holiday of
    /// a calendar that counts is leisure all day with its name, work comes
    /// back past it, a reminder's working days skip it, the agenda draws it,
    /// the plan closes it; one of a calendar that does not count is none of these.
    #[test]
    fn each_reader_knows_a_holiday_through_the_one_function() {
        use crate::window::AdminWindow;
        let week: Vec<AdminWindow> = ["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: None }).collect();
        let public = crate::subscribed::HolidayDays { id: "fr".into(), name: "Jours fériés".into(), holidays: Holidays::Public, days_off: true, periods: vec![(day("2030-12-25"), day("2030-12-25"), "Noël".into())], as_of: Some(1) };
        let off = DaysOff::with_holidays(&[], std::slice::from_ref(&public));
        let at = |text: &str| text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(jiff::tz::TimeZone::get("Europe/Paris").unwrap()).unwrap();
        let none = crate::quiet::Overrides::default();
        let mode = crate::quiet::mode(&week, &off, &none, &crate::quiet::Blocks::default(), &at("2030-12-25T10:00"));
        assert_eq!((mode.reason, mode.label.as_str(), mode.time), (crate::quiet::Reason::TimeOff, "Noël", crate::areas::Time::Leisure));
        assert_eq!(crate::quiet::next_work(&week, &off, &at("2030-12-24T18:00")).unwrap().date(), day("2030-12-26"), "work comes back past it");
        assert_eq!(crate::quiet::end_of_next_workday(&week, &off, &at("2030-12-24T18:00")).date(), day("2030-12-26"));
        let items = crate::timeline::days_off(&off);
        assert_eq!((items.len(), items[0].title.as_str()), (1, "Noël"));
        assert_eq!(off.days(day("2030-12-01"), day("2030-12-31")).len(), 1, "the plan's closed days");
        // The same calendar, not counted: a working day like any.
        let not_counted = DaysOff::with_holidays(&[], &[crate::subscribed::HolidayDays { days_off: false, ..public }]);
        assert_eq!(crate::quiet::mode(&week, &not_counted, &none, &crate::quiet::Blocks::default(), &at("2030-12-25T10:00")).reason, crate::quiet::Reason::Working);
        assert!(crate::timeline::days_off(&not_counted).is_empty());
    }

    /// Over the programs' sources, tests aside: nothing reads the list of
    /// `[[time_off]]` but this module (and the configuration that holds it,
    /// with Settings ▸ Hours, which edits it): every reader of a day off asks
    /// `DaysOff`, which knows the holidays that count too. A new reader of
    /// the list fails here, with its line.
    #[test]
    fn nothing_reads_the_days_off_list_but_this() {
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let allowed = ["sioul-core/src/vacations.rs", "sioul-core/src/config.rs", "sioul-core/src/settings.rs"];
        let mut stack = vec![crates.clone()];
        let mut astray = Vec::new();
        let mut read = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if path.is_dir() && name != "target" && name != "tests" {
                    stack.push(path);
                    continue;
                }
                if !name.ends_with(".rs") || name.contains("tests") || !path.components().any(|c| c.as_os_str() == "src") {
                    continue;
                }
                let file = path.strip_prefix(&crates).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                // The program's own lines: a file's test module, at its end, left out.
                let program = text.split("\n#[cfg(test)]\nmod tests").next().unwrap_or_default();
                read += 1;
                for (n, line) in program.lines().enumerate() {
                    let code = line.split("//").next().unwrap_or_default();
                    if (code.contains(".time_off") || code.contains("time_off_on(")) && !allowed.contains(&file.as_str()) {
                        astray.push(format!("{file}:{}: {}", n + 1, line.trim()));
                    }
                }
            }
        }
        assert!(read > 100, "the sources were read: {read}");
        assert!(astray.is_empty(), "read through vacations::DaysOff instead:\n{}", astray.join("\n"));
    }
}
