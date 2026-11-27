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
//! pauses for, which times it left out for which source, and from which
//! version of each source:
//!
//! - `X-SIOUL-PAUSE-DURING:vacations/<owner>` (`public`, `school/<id>`), one
//!   line each; `<owner>` is an opaque id of the settings whose vacations
//!   they are (`config::VacationsMark::owner`), so that another person's
//!   Sioul that can write the same calendar sees a source it does not know;
//! - `X-SIOUL-PAUSED;X-SOURCE=public:20261225T090000Z,20270101T090000Z`, one
//!   line per source, each time as an instant in UTC (a day for a whole-day
//!   series, a local time for a floating one);
//! - `X-SIOUL-PAUSED-VERSION;X-SOURCE=public:<stamp>`, one line per source:
//!   the version of that source its left-out times were computed from.
//!
//! **Each source is judged by itself** (19 November 2026, the review of the
//! first rule): a source has a stamp of its own, the same on every device
//! that holds the same version of it (`Version`): your vacations, when they
//! were last edited (written in the shared settings with them) and the days
//! they hold; a calendar of holidays, the days it gives. A device acts on a
//! source only when it knows it, at a version at least as new as the one the
//! series notes: it leaves out a time the source covers, and puts back a
//! time only when that source left it out and no longer covers it. A device
//! that does not know a source (no copy of that calendar here, someone
//! else's vacations, settings that do not read), or knows an older version,
//! touches none of that source's times, and still keeps those of the
//! sources it knows. Nothing is written when nothing differs: two devices
//! holding the same versions compute the same times and write nothing.
//!
//! A time left out by hand (an EXDATE Sioul did not note) is never put back.
//! The pauses are kept for the eighteen months ahead (`keep`), when a
//! vacation is typed, moved or removed, when a calendar of holidays changes,
//! and as the days pass; the times before today stay as they were.
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
/// The version of each source the times it left out were computed from, one line per source.
pub const PAUSED_VERSION: &str = "X-SIOUL-PAUSED-VERSION";
/// How far ahead the pauses are kept: eighteen months.
pub const AHEAD_DAYS: i64 = 548;
/// The form's word for your own vacations, before they are named yours
/// (`Source::Vacations` with your settings' owner id, `config::vacations_owner`).
pub const MY_VACATIONS: &str = "vacations";
/// How many former versions of each calendar of holidays a device remembers (`Days::remember`).
const HELD_KEPT: usize = 16;

/// What a series pauses during.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// The periods typed in the settings whose owner id this is (vacations,
    /// sick leave, other): yours when it is your settings' own.
    Vacations(String),
    /// Every subscribed calendar of public holidays.
    Public,
    /// One subscribed calendar of school holidays, by its id.
    School(String),
}

impl Source {
    /// Its word in the series' lines and the form: `vacations/<owner>`, "public", `school/<id>`.
    pub fn id(&self) -> String {
        match self {
            Source::Vacations(owner) => format!("vacations/{owner}"),
            Source::Public => "public".into(),
            Source::School(id) => format!("school/{id}"),
        }
    }

    pub fn parse(text: &str) -> Option<Source> {
        match text.trim() {
            "public" => Some(Source::Public),
            other => {
                if let Some(owner) = other.strip_prefix("vacations/") {
                    return crate::subscribed::valid_id(owner).then(|| Source::Vacations(owner.to_string()));
                }
                other.strip_prefix("school/").filter(|id| crate::subscribed::valid_id(id)).map(|id| Source::School(id.to_string()))
            }
        }
    }

    /// Your own vacations, as `config` names them; none before an owner id is made.
    pub fn mine(config: &Config) -> Option<Source> {
        let owner = config.vacations.owner.trim();
        crate::subscribed::valid_id(owner).then(|| Source::Vacations(owner.to_string()))
    }
}

/// The days a source pauses, as spans of days, both ends included, sorted,
/// apart and not touching (`spans`).
pub type Spans = Vec<(Date, Date)>;

/// Spans of days made one: sorted, those that overlap or touch joined, so
/// that the same days give the same spans whatever periods they came from.
pub fn spans(periods: impl IntoIterator<Item = (Date, Date)>) -> Spans {
    let mut all: Vec<(Date, Date)> = periods.into_iter().filter(|(from, until)| until >= from).collect();
    all.sort();
    let mut out: Spans = Vec::with_capacity(all.len());
    for (from, until) in all {
        match out.last_mut() {
            Some(last) if last.1.tomorrow().map_or(true, |next| from <= next) => last.1 = last.1.max(until),
            _ => out.push((from, until)),
        }
    }
    out
}

/// What a source's days are, as a stamp: the same days, the same stamp.
pub fn held(spans: &Spans) -> String {
    let said: String = spans.iter().map(|(from, until)| format!("{from}/{until}\n")).collect();
    crate::vdir::content_hash(said.as_bytes())
}

/// A source's version, as this device knows it (19 November 2026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Version {
    /// Your vacations: when they were last edited (the shared settings'
    /// stamp, `config::VacationsMark::stamp`, 0 unsaid) and the days they hold (`held`).
    Edited { at: i64, held: String },
    /// A calendar of holidays: the days this device's copy gives (`held`),
    /// and the versions it held before, oldest first.
    Held { held: String, before: Vec<String> },
}

impl Version {
    /// Its stamp, as a series notes it: `20261119T100000Z.<held>` for your
    /// vacations, `<held>` for a calendar of holidays.
    pub fn stamp(&self) -> String {
        match self {
            Version::Edited { at, held } => format!("{}.{held}", Timestamp::from_second(*at).unwrap_or(Timestamp::UNIX_EPOCH).strftime("%Y%m%dT%H%M%SZ")),
            Version::Held { held, .. } => held.clone(),
        }
    }

    /// Whether this version is at least as new as `noted`, a series' stamp:
    /// the same days; your vacations edited later; a calendar's version
    /// this device held before the one it holds. Anything else (a version
    /// it never held, a stamp that does not read) is not known to be older.
    pub fn at_least(&self, noted: &str) -> bool {
        let noted = noted.trim();
        match self {
            Version::Edited { at, held } => {
                let Some((when, days)) = noted.split_once('.') else { return false };
                days == held || jiff::civil::DateTime::strptime("%Y%m%dT%H%M%S", when.trim_end_matches(['Z', 'z'])).ok().and_then(|d| d.to_zoned(TimeZone::UTC).ok()).is_some_and(|z| *at > z.timestamp().as_second())
            }
            Version::Held { held, before } => noted == held || before.iter().any(|b| b == noted),
        }
    }
}

/// What this device knows of one source: the days it pauses, and their version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    pub spans: Spans,
    pub version: Version,
}

impl Known {
    /// Whether it pauses `date`.
    pub fn pauses(&self, date: Date) -> bool {
        let at = self.spans.partition_point(|(from, _)| *from <= date);
        at > 0 && self.spans[at - 1].1 >= date
    }
}

/// The sources this device knows, each with its days and version; a source
/// it does not know is not here (no copy of that calendar read here, someone
/// else's vacations).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Days {
    known: BTreeMap<Source, Known>,
}

/// The versions of each calendar of holidays this device held, by source
/// (`state/series-held.toml`): what tells it that a version a series notes
/// is older than its own.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HeldBefore {
    #[serde(default)]
    pub held: BTreeMap<String, Vec<String>>,
}

impl HeldBefore {
    fn path() -> std::path::PathBuf {
        crate::config::state_dir().join("series-held.toml")
    }

    pub fn load() -> HeldBefore {
        crate::files::read_to_string(HeldBefore::path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }
}

impl Days {
    /// What this device knows of each source, in `zone`.
    pub fn of(config: &Config, zone: &TimeZone) -> Days {
        Days::with(config, &crate::subscribed::holidays(config, zone), &HeldBefore::load())
    }

    /// From the typed periods of `config`, the holidays' copies given and
    /// the versions held before (the tests').
    pub fn with(config: &Config, holidays: &[crate::subscribed::HolidayDays], before: &HeldBefore) -> Days {
        let mut out = Days::default();
        // Your typed periods, every kind (`vacations::DaysOff::typed_of`), once your settings are named.
        if let Some(mine) = Source::mine(config) {
            let typed = spans(crate::vacations::DaysOff::typed_of(config).periods().iter().map(|p| (p.from, p.until)));
            let version = Version::Edited { at: config.vacations.edited(), held: held(&typed) };
            out.known.insert(mine, Known { spans: typed, version });
        }
        let former = |source: &Source| before.held.get(&source.id()).cloned().unwrap_or_default();
        // A calendar never read here gives nothing known; public holidays, only when each of theirs was read here.
        let mut public: Vec<(Date, Date)> = Vec::new();
        let mut public_known = true;
        let mut any_public = false;
        for calendar in holidays {
            let periods = calendar.periods.iter().map(|(from, until, _)| (*from, *until));
            match calendar.holidays {
                Holidays::Public => {
                    any_public = true;
                    public_known &= calendar.as_of.is_some();
                    public.extend(periods);
                }
                Holidays::School if calendar.as_of.is_some() => {
                    let source = Source::School(calendar.id.clone());
                    let days = spans(periods);
                    let version = Version::Held { held: held(&days), before: former(&source) };
                    out.known.insert(source, Known { spans: days, version });
                }
                Holidays::School => {}
            }
        }
        if any_public && public_known {
            let days = spans(public);
            let version = Version::Held { held: held(&days), before: former(&Source::Public) };
            out.known.insert(Source::Public, Known { spans: days, version });
        }
        out
    }

    /// What this device knows of `source`; none when it does not know it.
    pub fn known(&self, source: &Source) -> Option<&Known> {
        self.known.get(source)
    }

    /// Whether `source` pauses `date`, as this device knows it.
    pub fn pauses(&self, source: &Source, date: Date) -> bool {
        self.known(source).is_some_and(|k| k.pauses(date))
    }

    /// The versions of the calendars of holidays held now remembered on
    /// this device (`HeldBefore`), the latest last, a few each.
    pub fn remember(&self) -> Result<(), String> {
        let mut kept = HeldBefore::load();
        let mut changed = false;
        for (source, known) in &self.known {
            if let Version::Held { held, .. } = &known.version {
                let list = kept.held.entry(source.id()).or_default();
                if list.last() != Some(held) {
                    list.retain(|h| h != held);
                    list.push(held.clone());
                    let extra = list.len().saturating_sub(HELD_KEPT);
                    list.drain(..extra);
                    changed = true;
                }
            }
        }
        if !changed {
            return Ok(());
        }
        let path = HeldBefore::path();
        if let Some(parent) = path.parent() {
            crate::files::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        crate::filelock::replace(&path, toml::to_string(&kept).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
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

/// The version of each source a series' pauses were computed from (`PAUSED_VERSION`).
pub fn versions_of(text: &str) -> BTreeMap<Source, String> {
    let mut out = BTreeMap::new();
    for line in master_lines(text) {
        if crate::lines::name(&line) == PAUSED_VERSION
            && let Some(source) = crate::lines::param(&line, "X-SOURCE").and_then(|s| Source::parse(&s))
        {
            out.entry(source).or_insert_with(|| crate::lines::value(&line).trim().to_string());
        }
    }
    out
}

/// An instant as Sioul writes it here: `20261225T090000Z`; a day `20261225`
/// (at midnight in `zone`); a local time `20261225T090000` (a floating
/// series', in `zone`).
fn instant_of(value: &str, zone: &TimeZone) -> Option<i64> {
    let value = value.trim();
    if value.len() == 8 {
        return Date::strptime("%Y%m%d", value).ok()?.to_zoned(zone.clone()).ok().map(|z| z.timestamp().as_second());
    }
    let (local, clock) = match value.strip_suffix(['Z', 'z']) {
        Some(utc) => (utc, TimeZone::UTC),
        None => (value, zone.clone()),
    };
    jiff::civil::DateTime::strptime("%Y%m%dT%H%M%S", local).ok()?.to_zoned(clock).ok().map(|z| z.timestamp().as_second())
}

/// How a series' start is written, which its noted times follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// A whole day: a day.
    Day,
    /// A time without a zone: a local time, the same on every device.
    Floating,
    /// A time with a zone, or in UTC: an instant in UTC.
    Instant,
}

/// An instant written as Sioul notes it, in the form of the series' start.
fn written(at: i64, form: Form, zone: &TimeZone) -> String {
    let Ok(t) = Timestamp::from_second(at) else { return String::new() };
    match form {
        Form::Day => t.to_zoned(zone.clone()).strftime("%Y%m%d").to_string(),
        Form::Floating => t.to_zoned(zone.clone()).strftime("%Y%m%dT%H%M%S").to_string(),
        Form::Instant => t.strftime("%Y%m%dT%H%M%SZ").to_string(),
    }
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

/// The form of the series' DTSTART: a day, a local time, an instant.
fn start_form(text: &str) -> Form {
    let Some(line) = master_lines(text).into_iter().find(|l| crate::lines::name(l) == "DTSTART") else { return Form::Instant };
    let value = crate::lines::value(&line).trim().to_string();
    if value.len() == 8 {
        Form::Day
    } else if !value.ends_with(['Z', 'z']) && crate::lines::param(&line, "TZID").is_none() {
        Form::Floating
    } else {
        Form::Instant
    }
}

/// A series' pause lines written as `sources` say (none: taken out), the
/// rest as it was; the times it left out are kept until `keep` settles them.
pub fn with_sources(text: &str, sources: &[Source]) -> Option<String> {
    let without = crate::agenda::remove_lines(text, |l| crate::lines::name(l) == PAUSE_DURING)?;
    let lines: Vec<String> = sources.iter().map(|s| format!("{PAUSE_DURING}:{}", s.id())).collect();
    if lines.is_empty() { Some(without) } else { crate::agenda::add_lines(&without, &lines) }
}

/// A series' pauses settled for the times from `from` to `to` (Unix
/// seconds; today and the eighteen months ahead), each source by itself
/// (`Days::known`, `Version::at_least`):
///
/// - a source this device knows at a version at least as new as the
///   series notes (or that the series does not note yet) acts: each time it
///   covers is left out and noted for it; each time it left out that it no
///   longer covers, or that the series no longer pauses for, is put back
///   unless another source still holds it;
/// - a source this device does not know, or knows at an older version,
///   keeps its noted times and its version as they are, and its times stay
///   left out;
/// - a source the series no longer pauses for has its times put back by
///   any device: the series' own lines say so, the same everywhere;
/// - a time left out by hand is never put back, nor noted; a time with a
///   changed occurrence of its own is left as it is; what lies outside the
///   stretch is left as it was; a time the rule no longer gives (the series
///   moved, or ended) is forgotten with its EXDATE.
///
/// The new text, or none when nothing differs (nor when the rule does not
/// expand here: nothing is taken back on that account).
pub fn keep(text: &str, days: &Days, from: i64, to: i64, zone: &TimeZone) -> Option<String> {
    let wanted = sources_of(text);
    let paused = paused_of(text, zone);
    let versions = versions_of(text);
    if wanted.is_empty() && paused.is_empty() && versions.is_empty() {
        return None;
    }
    // The sources this device acts on: known here, at a version at least as new as the series notes.
    let acting = |source: &Source| days.known(source).filter(|k| versions.get(source).is_none_or(|noted| k.version.at_least(noted)));
    let times = crate::agenda::rule_times(text, from, to, zone)?;
    let left_out: BTreeSet<i64> = crate::agenda::left_out_times(text, zone).into_iter().collect();
    let ours: BTreeSet<i64> = paused.values().flatten().copied().collect();
    let in_stretch = |at: &i64| *at >= from && *at < to;
    let given: BTreeSet<i64> = times.iter().map(|(at, _)| *at).collect();
    // What stays noted: the times outside the stretch as they were; in it, as settled below.
    let mut noted: BTreeMap<Source, BTreeSet<i64>> = paused.iter().map(|(s, at)| (s.clone(), at.iter().copied().filter(|t| !in_stretch(t)).collect())).collect();
    let (mut add, mut back) = (Vec::new(), Vec::new());
    for (at, changed) in &times {
        let by: Vec<&Source> = paused.iter().filter(|(_, set)| set.contains(at)).map(|(s, _)| s).collect();
        // A time changed by hand is the person's own: left as it is, noted as it was.
        if *changed {
            for source in by {
                noted.entry(source.clone()).or_default().insert(*at);
            }
            continue;
        }
        // Left out by hand already: the person's own, never noted as Sioul's.
        let by_hand = left_out.contains(at) && by.is_empty();
        let Ok(when) = Timestamp::from_second(*at) else { continue };
        let date = when.to_zoned(zone.clone()).date();
        let mut holds = false;
        // Each source that noted it keeps it, unless it acts here and lets it go.
        for source in &by {
            let lets_go = !wanted.contains(source) || acting(source).is_some_and(|k| !k.pauses(date));
            if !lets_go {
                noted.entry((*source).clone()).or_default().insert(*at);
                holds = true;
            }
        }
        // Each source it pauses for that acts here and covers it takes it.
        if !by_hand {
            for source in wanted.iter().filter(|s| acting(s).is_some_and(|k| k.pauses(date))) {
                noted.entry(source.clone()).or_default().insert(*at);
                holds = true;
            }
        }
        match (holds, left_out.contains(at)) {
            // Noted already but not left out: put back by hand elsewhere, and left so.
            (true, false) if by.is_empty() => add.push(*at),
            (false, true) if !by.is_empty() => back.push(*at),
            _ => {}
        }
    }
    // Noted in the stretch, but no time of the rule now (the series moved, or ended): forgotten, its EXDATE with it.
    back.extend(ours.iter().copied().filter(|at| in_stretch(at) && !given.contains(at) && left_out.contains(at)));
    noted.retain(|_, set| !set.is_empty());
    // Each source's version, for those it pauses for or still notes times
    // for: this device's own where it acts; else kept as the series says it.
    let mut stamps: BTreeMap<Source, String> = BTreeMap::new();
    for source in wanted.iter().chain(noted.keys()) {
        if let Some(stamp) = acting(source).map(|k| k.version.stamp()).or_else(|| versions.get(source).cloned()) {
            stamps.insert(source.clone(), stamp);
        }
    }
    // Nothing differs: the same times left out and noted, and every source
    // it pauses for that acts here already has a version (a version alone
    // newer is not written: the times it gives are the same).
    let unversioned = wanted.iter().any(|s| acting(s).is_some() && !versions.contains_key(s));
    let dropped = versions.keys().any(|s| !stamps.contains_key(s));
    if add.is_empty() && back.is_empty() && noted == paused && !unversioned && !dropped {
        return None;
    }
    let form = start_form(text);
    let mut extra: Vec<String> = noted.iter().map(|(source, set)| format!("{PAUSED};X-SOURCE={}:{}", source.id(), set.iter().map(|at| written(*at, form, zone)).collect::<Vec<_>>().join(","))).collect();
    extra.extend(stamps.iter().map(|(source, stamp)| format!("{PAUSED_VERSION};X-SOURCE={}:{stamp}", source.id())));
    let left = crate::agenda::leave_out(text, &add, &back, zone)?;
    let cleared = crate::agenda::remove_lines(&left, |l| matches!(crate::lines::name(l).as_str(), PAUSED | PAUSED_VERSION))?;
    let out = if extra.is_empty() { cleared } else { crate::agenda::add_lines(&cleared, &extra)? };
    (out != text).then_some(out)
}

/// Every source a series names, those it pauses for and those it noted times or a version for.
pub fn all_sources(text: &str, zone: &TimeZone) -> Vec<Source> {
    let mut out = sources_of(text);
    for source in paused_of(text, zone).into_keys().chain(versions_of(text).into_keys()) {
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

/// A series that ends after a number of times, some of them in pauses
/// (the review's F36, the coordinator's choice): the number keeps the
/// standard meaning (RFC 5545's COUNT counts every date of the series, those
/// left out included), and this says it in plain words: "Every week: 10
/// dates, 2 of them in pauses: 8 take place, the last on Wednesday 4
/// December." "" when it does not end so, or none of its dates is in a pause.
pub fn count_words(text: &str, now: &Zoned, tr: &crate::i18n::Translator) -> String {
    let zone = now.time_zone().clone();
    let Some(edit) = crate::agenda::edit_of_text(text, &zone) else { return String::new() };
    if edit.ends != "count" || edit.count == 0 {
        return String::new();
    }
    let Some(times) = crate::agenda::rule_times(text, i64::MIN / 4, i64::MAX / 4, &zone) else { return String::new() };
    let paused: BTreeSet<i64> = paused_of(text, &zone).into_values().flatten().collect();
    let left_out: BTreeSet<i64> = crate::agenda::left_out_times(text, &zone).into_iter().collect();
    let in_pauses = times.iter().filter(|(at, _)| paused.contains(at)).count();
    if in_pauses == 0 {
        return String::new();
    }
    let held: Vec<i64> = times.iter().map(|(at, _)| *at).filter(|at| !left_out.contains(at)).collect();
    let mut every = crate::i18n::args();
    every.set("unit", edit.repeat.clone());
    every.set("every", edit.every.max(1));
    let mut args = crate::i18n::args();
    args.set("rule", tr.text("series-every", Some(&every)));
    args.set("count", times.len());
    args.set("paused", in_pauses);
    args.set("held", held.len());
    args.set("last", held.last().and_then(|at| Timestamp::from_second(*at).ok()).map(|t| tr.day_in(t.to_zoned(zone.clone()).date(), now.date())).unwrap_or_default());
    tr.text("series-count-paused", Some(&args))
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
