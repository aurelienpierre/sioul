// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A series that ends, and pauses (docs/client.md, "A series that ends, and
//! pauses"; 18 November 2026).
//!
//! **Ends**: the event form writes the standard rule's end, a day (`UNTIL`)
//! or a number of times (`COUNT`), which every calendar program reads; a
//! rule's days and other parts written by another program are kept
//! (`agenda::rule_line`).
//!
//! **Pauses during** any of these, ticked together on the series: your
//! vacations (the periods you type), public holidays (every subscribed
//! calendar of public holidays), each school-holidays calendar by its name.
//! Each time of the series that falls in a pause is left out by the standard
//! means (`EXDATE`), so the phone's calendar and anyone the agenda is shared
//! with see the same; Sioul notes on the series, in lines of its own, what it
//! pauses for and which times it left out for which source:
//!
//! - `X-SIOUL-PAUSE-DURING:vacations` (`public`, `school/<id>`), one line each;
//! - `X-SIOUL-PAUSED;X-SOURCE=public:20261225T090000Z,20270101T090000Z`, one
//!   line per source, each time as an instant in UTC (a day for a whole-day series);
//! - `X-SIOUL-PAUSED-AS-OF:20261118T100000Z`: how fresh the copy of the
//!   holidays was that last wrote them.
//!
//! A time left out by hand (an EXDATE Sioul did not note) is never put back.
//! The pauses are kept for the eighteen months ahead (`keep`), when a
//! vacation is typed, moved or removed, when a calendar of holidays changes,
//! and as the days pass; the times before today stay as they were. Between
//! two devices the one whose copy of the holidays is the freshest writes, and
//! an older copy never undoes a newer one (`may_write`, the busy calendars'
//! rule, `availability::may_write`).
//!
//! A CalDAV server that drops lines it does not know (none of those Sioul
//! names in docs/compatibility.md does) keeps the EXDATEs, which are
//! standard: the pauses written stay, but Sioul no longer knows they are
//! its own (it would then never put them back) nor what the series pauses
//! for (it stops adding new ones).

use crate::config::Config;
use crate::subscribed::Holidays;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use std::collections::{BTreeMap, BTreeSet};

/// What a series pauses for, one line each.
pub const PAUSE_DURING: &str = "X-SIOUL-PAUSE-DURING";
/// The times Sioul left out, one line per source.
pub const PAUSED: &str = "X-SIOUL-PAUSED";
/// How fresh the copy of the holidays was that last wrote them.
pub const PAUSED_AS_OF: &str = "X-SIOUL-PAUSED-AS-OF";
/// How far ahead the pauses are kept: eighteen months.
pub const AHEAD_DAYS: i64 = 548;

/// What a series pauses during.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// The periods you type (vacations, sick leave, other).
    Vacations,
    /// Every subscribed calendar of public holidays.
    Public,
    /// One subscribed calendar of school holidays, by its id.
    School(String),
}

impl Source {
    /// Its word in the series' lines and the form: "vacations", "public", `school/<id>`.
    pub fn id(&self) -> String {
        match self {
            Source::Vacations => "vacations".into(),
            Source::Public => "public".into(),
            Source::School(id) => format!("school/{id}"),
        }
    }

    pub fn parse(text: &str) -> Option<Source> {
        match text.trim() {
            "vacations" => Some(Source::Vacations),
            "public" => Some(Source::Public),
            other => other.strip_prefix("school/").filter(|id| crate::subscribed::valid_id(id)).map(|id| Source::School(id.to_string())),
        }
    }
}

/// The days each source pauses, as this device knows them now, and how fresh each copy is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Days {
    days: BTreeMap<Source, BTreeSet<Date>>,
    /// When this device last found each calendar of holidays current (`subscribed::Fetched::checked`);
    /// none while it never read it. Your vacations: when the settings last changed here.
    as_of: BTreeMap<Source, Option<i64>>,
    /// The sources this device has no word of at all (a school calendar stopped): no day, known.
    now: i64,
}

impl Days {
    /// What this device knows of each source, around `today`, in `zone`.
    pub fn of(config: &Config, zone: &TimeZone, today: Date) -> Days {
        let settings = std::fs::metadata(crate::config::default_path()).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
        Days::with(config, &crate::subscribed::holidays(config, zone, today), settings, Timestamp::now().as_second())
    }

    /// From the typed periods of `config`, the holidays' copies given, the
    /// settings' last change and now (the tests').
    pub fn with(config: &Config, holidays: &[crate::subscribed::HolidayDays], settings: i64, now: i64) -> Days {
        let mut out = Days { now, ..Days::default() };
        // Your typed periods, every kind (`vacations::DaysOff::typed_of`), whatever their years.
        out.days.insert(Source::Vacations, crate::vacations::DaysOff::typed_of(config).days(Date::MIN, Date::MAX));
        out.as_of.insert(Source::Vacations, Some(settings));
        let mut public = BTreeSet::new();
        let mut public_as_of: Vec<Option<i64>> = Vec::new();
        for calendar in holidays {
            let mut days = BTreeSet::new();
            for (from, until, _) in &calendar.periods {
                let mut day = *from;
                while day <= *until {
                    days.insert(day);
                    let Ok(next) = day.tomorrow() else { break };
                    day = next;
                }
            }
            match calendar.holidays {
                Holidays::Public => {
                    public.extend(days);
                    public_as_of.push(calendar.as_of);
                }
                Holidays::School => {
                    let source = Source::School(calendar.id.clone());
                    out.days.insert(source.clone(), days);
                    out.as_of.insert(source, calendar.as_of);
                }
            }
        }
        out.days.insert(Source::Public, public);
        // The oldest copy among them, none if one was never read here; no
        // calendar of public holidays at all: none of their days, known now.
        let public_as_of = if public_as_of.is_empty() { Some(now) } else if public_as_of.iter().any(Option::is_none) { None } else { public_as_of.iter().flatten().copied().min() };
        out.as_of.insert(Source::Public, public_as_of);
        out
    }

    /// Whether `source` pauses `date`.
    pub fn pauses(&self, source: &Source, date: Date) -> bool {
        self.days.get(source).is_some_and(|days| days.contains(&date))
    }

    /// This device's view of `sources` (`availability::view`): their copies'
    /// oldest and newest times found current; none while one was never read
    /// here. A school calendar this device has no word of (stopped) is no day, known now.
    pub fn view(&self, sources: &[Source]) -> Option<(i64, i64)> {
        let times: Vec<Option<i64>> = sources.iter().map(|s| self.as_of.get(s).copied().unwrap_or(Some(self.now))).collect();
        crate::availability::view(&times)
    }
}

/// What a series pauses for, as its lines say (`PAUSE_DURING`), in their order, once each.
pub fn sources_of(text: &str) -> Vec<Source> {
    let mut out = Vec::new();
    for line in master_lines(text) {
        if crate::lines::name(&line) == PAUSE_DURING
            && let Some(source) = Source::parse(&crate::lines::unescape(crate::lines::value(&line)))
            && !out.contains(&source)
        {
            out.push(source);
        }
    }
    out
}

/// The times Sioul left out, by source (`PAUSED`).
pub fn paused_of(text: &str, zone: &TimeZone) -> BTreeMap<Source, BTreeSet<i64>> {
    let mut out: BTreeMap<Source, BTreeSet<i64>> = BTreeMap::new();
    for line in master_lines(text) {
        if crate::lines::name(&line) != PAUSED {
            continue;
        }
        let Some(source) = crate::lines::param(&line, "X-SOURCE").and_then(|s| Source::parse(&s)) else { continue };
        let value = crate::lines::value(&line).replace("\\,", ",");
        out.entry(source).or_default().extend(value.split(',').filter_map(|v| instant_of(v, zone)));
    }
    out
}

/// How fresh the copy was that last wrote a series' pauses (`PAUSED_AS_OF`).
pub fn as_of(text: &str) -> Option<i64> {
    master_lines(text).iter().find(|l| crate::lines::name(l) == PAUSED_AS_OF).and_then(|l| instant_of(crate::lines::value(l), &TimeZone::UTC))
}

/// An instant as Sioul writes it here: `20261225T090000Z`, or a day `20261225` (at midnight in `zone`).
fn instant_of(value: &str, zone: &TimeZone) -> Option<i64> {
    let value = value.trim();
    if value.len() == 8 {
        return Date::strptime("%Y%m%d", value).ok()?.to_zoned(zone.clone()).ok().map(|z| z.timestamp().as_second());
    }
    let utc = value.strip_suffix(['Z', 'z'])?;
    jiff::civil::DateTime::strptime("%Y%m%dT%H%M%S", utc).ok()?.to_zoned(TimeZone::UTC).ok().map(|z| z.timestamp().as_second())
}

/// An instant written as Sioul notes it: a day for a whole-day series.
fn written(at: i64, whole_day: bool, zone: &TimeZone) -> String {
    let Ok(t) = Timestamp::from_second(at) else { return String::new() };
    if whole_day { t.to_zoned(zone.clone()).strftime("%Y%m%d").to_string() } else { t.strftime("%Y%m%dT%H%M%SZ").to_string() }
}

/// The main VEVENT's own lines (its alarms left out).
fn master_lines(text: &str) -> Vec<String> {
    let source = crate::lines::unfold(text);
    let mut out = Vec::new();
    let (mut inside, mut nested, mut is_override) = (false, 0usize, false);
    let mut event = Vec::new();
    for line in source {
        let name = crate::lines::name(&line);
        let value = crate::lines::value(&line).trim().to_ascii_uppercase();
        if !inside {
            if name == "BEGIN" && value == "VEVENT" {
                (inside, nested, is_override) = (true, 0, false);
                event.clear();
            }
            continue;
        }
        match name.as_str() {
            "BEGIN" => nested += 1,
            "END" if nested > 0 => nested -= 1,
            "END" => {
                inside = false;
                if !is_override {
                    out = std::mem::take(&mut event);
                    break;
                }
            }
            "RECURRENCE-ID" if nested == 0 => is_override = true,
            _ if nested == 0 => event.push(line.clone()),
            _ => {}
        }
    }
    out
}

/// Whether the series' DTSTART is a whole day.
fn whole_day(text: &str) -> bool {
    master_lines(text).iter().find(|l| crate::lines::name(l) == "DTSTART").is_some_and(|l| crate::lines::value(l).trim().len() == 8)
}

/// A series' pause lines written as `sources` say (none: taken out), the
/// rest as it was; the times it left out are kept until `keep` settles them.
pub fn with_sources(text: &str, sources: &[Source]) -> Option<String> {
    let without = crate::agenda::remove_lines(text, |l| crate::lines::name(l) == PAUSE_DURING)?;
    let lines: Vec<String> = sources.iter().map(|s| format!("{PAUSE_DURING}:{}", s.id())).collect();
    if lines.is_empty() { Some(without) } else { crate::agenda::add_lines(&without, &lines) }
}

/// A series' pauses settled for the times from `from` to `to` (Unix
/// seconds; today and the eighteen months ahead): each time that falls in a
/// day of a source it pauses for, and is not left out yet, is left out and
/// noted; each time Sioul left out that no longer falls in one is put back;
/// a time left out by hand is never put back, nor noted; a time with a
/// changed occurrence of its own is left as it is; what lies outside the
/// stretch is left as it was. `as_of`: how fresh this device's copy of the
/// holidays is, noted. The new text, or none when nothing changes.
pub fn keep(text: &str, days: &Days, from: i64, to: i64, zone: &TimeZone, as_of: i64) -> Option<String> {
    let wanted = sources_of(text);
    let paused = paused_of(text, zone);
    if wanted.is_empty() && paused.is_empty() {
        return None;
    }
    let times = crate::agenda::rule_times(text, from, to, zone).unwrap_or_default();
    let left_out: BTreeSet<i64> = crate::agenda::left_out_times(text, zone).into_iter().collect();
    let ours: BTreeSet<i64> = paused.values().flatten().copied().collect();
    let in_stretch = |at: &i64| *at >= from && *at < to;
    // What stays noted: the times outside the stretch as they were.
    let mut noted: BTreeMap<Source, BTreeSet<i64>> = paused.iter().map(|(s, at)| (s.clone(), at.iter().copied().filter(|t| !in_stretch(t)).collect())).collect();
    let (mut add, mut back) = (Vec::new(), Vec::new());
    let given: BTreeSet<i64> = times.iter().map(|(at, _)| *at).collect();
    for (at, changed) in &times {
        let Ok(when) = Timestamp::from_second(*at) else { continue };
        let date = when.to_zoned(zone.clone()).date();
        let by = wanted.iter().find(|s| days.pauses(s, date));
        match by {
            // A time changed by hand is the person's own: left as it is, noted as it was.
            _ if *changed => {
                if let Some((source, _)) = paused.iter().find(|(_, set)| set.contains(at)) {
                    noted.entry(source.clone()).or_default().insert(*at);
                }
            }
            Some(source) if ours.contains(at) => {
                noted.entry(source.clone()).or_default().insert(*at);
            }
            // Left out by hand already: the person's own, never noted as Sioul's.
            Some(_) if left_out.contains(at) => {}
            Some(source) => {
                add.push(*at);
                noted.entry(source.clone()).or_default().insert(*at);
            }
            None if ours.contains(at) => back.push(*at),
            None => {}
        }
    }
    // Noted in the stretch, but no time of the rule now (the series moved, or ended): forgotten, its EXDATE with it.
    back.extend(ours.iter().copied().filter(|at| in_stretch(at) && !given.contains(at)));
    noted.retain(|_, set| !set.is_empty());
    let whole = whole_day(text);
    let lines: Vec<String> = noted.iter().map(|(source, set)| format!("{PAUSED};X-SOURCE={}:{}", source.id(), set.iter().map(|at| written(*at, whole, zone)).collect::<Vec<_>>().join(","))).collect();
    let unchanged = add.is_empty() && back.is_empty() && noted == paused;
    if unchanged {
        return None;
    }
    let left = crate::agenda::leave_out(text, &add, &back, zone)?;
    let cleared = crate::agenda::remove_lines(&left, |l| matches!(crate::lines::name(l).as_str(), PAUSED | PAUSED_AS_OF))?;
    let mut extra = lines;
    if !extra.is_empty() {
        extra.push(format!("{PAUSED_AS_OF}:{}", Timestamp::from_second(as_of).map(|t| t.strftime("%Y%m%dT%H%M%SZ").to_string()).unwrap_or_default()));
    }
    let out = if extra.is_empty() { cleared } else { crate::agenda::add_lines(&cleared, &extra)? };
    (out != text).then_some(out)
}

/// Whether this device writes a series' pauses (`availability::may_write`):
/// its view of what the series pauses for is newer than what any other
/// device last wrote pauses from (`others`, their sealed entries) and than
/// what the series says it was written from; an older copy never undoes a newer one.
pub fn may_write(view: Option<(i64, i64)>, others: &[i64], text: &str) -> bool {
    crate::availability::may_write(view, others.iter().copied().chain(as_of(text)))
}

/// Every source a series names, those it pauses for and those it noted times for.
pub fn all_sources(text: &str, zone: &TimeZone) -> Vec<Source> {
    let mut out = sources_of(text);
    for source in paused_of(text, zone).into_keys() {
        if !out.contains(&source) {
            out.push(source);
        }
    }
    out
}

/// What the card says of a series' next pause (18 November 2026): its
/// first time left out by Sioul from `now` on, and when the series resumes
/// (its next time after it that is not left out): "Paused from Friday 19
/// December (public holidays), back on Monday 5 January." "" when none comes.
pub fn pause_line(text: &str, now: &Zoned, names: &dyn Fn(&Source) -> String, tr: &crate::i18n::Translator) -> String {
    let zone = now.time_zone().clone();
    let from = now.timestamp().as_second();
    let Some(times) = crate::agenda::rule_times(text, from, from + AHEAD_DAYS * 86_400, &zone) else { return String::new() };
    let paused = paused_of(text, &zone);
    let left_out: BTreeSet<i64> = crate::agenda::left_out_times(text, &zone).into_iter().collect();
    let Some((first, source)) = times.iter().find_map(|(at, _)| paused.iter().find(|(_, set)| set.contains(at)).map(|(s, _)| (*at, s.clone()))) else { return String::new() };
    let back = times.iter().map(|(at, _)| *at).find(|at| *at > first && !left_out.contains(at));
    let day = |at: i64| Timestamp::from_second(at).map(|t| tr.day_in(t.to_zoned(zone.clone()).date(), now.date())).unwrap_or_default();
    let mut args = crate::i18n::args();
    args.set("from", day(first));
    args.set("why", names(&source));
    match back {
        Some(back) => {
            args.set("back", day(back));
            tr.text("series-paused-back", Some(&args))
        }
        None => tr.text("series-paused", Some(&args)),
    }
}

/// How a series repeats, in words: "Every week", "Every 2 weeks until
/// Friday 26 June", "Every day, 10 times"; "" for an event that does not
/// repeat as the form says it.
pub fn rule_words(edit: &crate::agenda::EventEdit, today: Date, tr: &crate::i18n::Translator) -> String {
    if edit.repeat.is_empty() {
        return String::new();
    }
    let mut args = crate::i18n::args();
    args.set("unit", edit.repeat.clone());
    args.set("every", edit.every.max(1));
    let every = tr.text("series-every", Some(&args));
    let mut args = crate::i18n::args();
    args.set("rule", every.clone());
    match edit.ends.as_str() {
        "until" => match edit.until.parse::<Date>() {
            Ok(day) => {
                args.set("date", tr.day_in(day, today));
                tr.text("series-until", Some(&args))
            }
            Err(_) => every,
        },
        "count" if edit.count > 0 => {
            args.set("count", edit.count);
            tr.text("series-count", Some(&args))
        }
        _ => every,
    }
}

#[cfg(test)]
mod tests;
