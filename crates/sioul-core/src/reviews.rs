// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! How each day went, as you said it (docs/reviews.md): the morning's
//! weather, the review at the end of the work day ("Done for today") and the
//! one before sleep, each with how the day felt, its mix and a note, every
//! answer optional. Kept readable in `$XDG_DATA_HOME/sioul/reviews/YYYY-MM.toml`,
//! one table per date, and shared with the sharing's "time" part, with the
//! plan's other files: every device learns the same.
//!
//! The plan learns from the day's outcome (`Day::outcome`, criteria CB13–14
//! and G7 of docs/research/): a day without one counts for nothing. Nothing
//! here is counted, scored or compared: the words are shown as said.

use crate::i18n::{Translator, args};
use crate::quiet::Mode;
use crate::today::Weather;
use jiff::civil::Date;
use jiff::{Timestamp, Zoned};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A weather said again within this long of the first is a correction, not a change.
pub const CORRECTION: i64 = 30 * 60;
/// The evening: this long before the night's wind-down (an informed guess).
pub const EVENING: i64 = 3 * 3600;
/// Without a night set, the evening runs from this hour to `EVENING_ENDS` (guesses).
pub const EVENING_FROM: i8 = 20;
pub const EVENING_ENDS: i8 = 2;
/// Without a night set, a day closed before this hour is the day before.
pub const DAY_TURNS: i8 = 5;

/// How the day felt: the words a task has for what it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Felt {
    Light,
    Usual,
    Heavy,
    GaveBack,
}

impl Felt {
    pub const ALL: [Felt; 4] = [Felt::Light, Felt::Usual, Felt::Heavy, Felt::GaveBack];

    /// As files write it: "light", "usual", "heavy", "gave_back"; a word it does not know is none.
    pub fn parse(text: &str) -> Option<Felt> {
        match text.trim() {
            "light" => Some(Felt::Light),
            "usual" => Some(Felt::Usual),
            "heavy" => Some(Felt::Heavy),
            "gave_back" | "gave-back" => Some(Felt::GaveBack),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Felt::Light => "light",
            Felt::Usual => "usual",
            Felt::Heavy => "heavy",
            Felt::GaveBack => "gave_back",
        }
    }
}

/// The day's mix of duty and what is yours (G7): too much, about right, too empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mix {
    TooMuch,
    AboutRight,
    TooEmpty,
}

impl Mix {
    pub const ALL: [Mix; 3] = [Mix::TooMuch, Mix::AboutRight, Mix::TooEmpty];

    /// As files write it: "too_much", "about_right", "too_empty"; a word it does not know is none.
    pub fn parse(text: &str) -> Option<Mix> {
        match text.trim() {
            "too_much" | "too-much" => Some(Mix::TooMuch),
            "about_right" | "about-right" => Some(Mix::AboutRight),
            "too_empty" | "too-empty" => Some(Mix::TooEmpty),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Mix::TooMuch => "too_much",
            Mix::AboutRight => "about_right",
            Mix::TooEmpty => "too_empty",
        }
    }

    /// A day that was too much: a bad day for the budgets.
    pub fn bad(self) -> bool {
        self == Mix::TooMuch
    }

    /// About right or too empty: the costs went fine.
    pub fn fine_for_costs(self) -> bool {
        matches!(self, Mix::AboutRight | Mix::TooEmpty)
    }

    /// About right alone: a good day, for the gain's minimum.
    pub fn good_for_gain(self) -> bool {
        self == Mix::AboutRight
    }
}

/// Which review: at the end of the work day, or before sleep (the whole day).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Work,
    Night,
}

impl Kind {
    pub fn parse(text: &str) -> Option<Kind> {
        match text.trim() {
            "work" => Some(Kind::Work),
            "night" => Some(Kind::Night),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Kind::Work => "work",
            Kind::Night => "night",
        }
    }
}

/// One review: when it was given, and what was said, each answer optional.
/// Given with nothing said, it says the day was closed without a word.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Review {
    /// When it was given (Unix seconds); written as RFC 3339 text.
    pub at: i64,
    pub felt: Option<Felt>,
    pub mix: Option<Mix>,
    /// Markdown; "" when none.
    pub memo: String,
}

impl Review {
    /// Something was said: a word or a note.
    pub fn said(&self) -> bool {
        self.felt.is_some() || self.mix.is_some() || !self.memo.trim().is_empty()
    }
}

/// One day: the morning's weather, as said, and its two reviews.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Day {
    /// The morning's weather: the first said that day (a change within
    /// `CORRECTION` corrects it). None: never said; the plan's default is not a perception.
    pub weather: Option<Weather>,
    /// When it was first said (Unix seconds).
    pub weather_at: Option<i64>,
    /// Another weather said later that day, the last one.
    pub later: Option<Weather>,
    pub work: Option<Review>,
    pub night: Option<Review>,
}

impl Day {
    /// The day's outcome, for the plan to learn from: the night's mix if
    /// given, else the work's; none, and the day counts for nothing.
    pub fn outcome(&self) -> Option<Mix> {
        self.night.as_ref().and_then(|r| r.mix).or_else(|| self.work.as_ref().and_then(|r| r.mix))
    }

    pub fn review(&self, kind: Kind) -> Option<&Review> {
        match kind {
            Kind::Work => self.work.as_ref(),
            Kind::Night => self.night.as_ref(),
        }
    }

    /// Words were said that day, in either review.
    pub fn said(&self) -> bool {
        self.work.as_ref().is_some_and(Review::said) || self.night.as_ref().is_some_and(Review::said)
    }
}

/// Every day kept, by date.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reviews {
    pub days: BTreeMap<Date, Day>,
}

impl Reviews {
    /// The folder of the month files, `$XDG_DATA_HOME/sioul/reviews`.
    pub fn default_path() -> PathBuf {
        crate::config::data_dir().join("reviews")
    }

    /// Every month kept in `dir`. A file that does not read is left out, and
    /// said on the error output; it is never written over (`save`).
    pub fn load(dir: &Path) -> Reviews {
        let mut months: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| month_of(p).is_some()).collect())
            .unwrap_or_default();
        months.sort();
        let mut reviews = Reviews::default();
        for path in months {
            reviews.take(&path);
        }
        reviews
    }

    /// The months covering `from` to `to` only: what a few weeks need.
    pub fn load_between(dir: &Path, from: Date, to: Date) -> Reviews {
        let mut reviews = Reviews::default();
        let (mut year, mut month) = (from.year(), from.month());
        while (year, month) <= (to.year(), to.month()) {
            reviews.take(&dir.join(format!("{year:04}-{month:02}.toml")));
            (year, month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
        }
        reviews.days.retain(|date, _| (from..=to).contains(date));
        reviews
    }

    /// One month file's days added; a date in its own month's file comes first.
    fn take(&mut self, path: &Path) {
        match read_month(path) {
            Ok(days) => {
                let own = month_of(path);
                for (date, day) in days {
                    if own == Some((date.year(), date.month())) || !self.days.contains_key(&date) {
                        self.days.insert(date, day);
                    }
                }
            }
            Err(e) => eprintln!("{e}"),
        }
    }

    pub fn day(&self, date: Date) -> Option<&Day> {
        self.days.get(&date)
    }

    /// The days with words before `before`, the newest first, at most `days` days back.
    pub fn recent(&self, before: Date, days: i64) -> Vec<(Date, &Day)> {
        let from = before.checked_sub(jiff::Span::new().days(days)).unwrap_or(before);
        self.days.range(from..before).rev().filter(|(_, day)| day.said()).map(|(date, day)| (*date, day)).collect()
    }
}

/// "2026-10.toml" → (2026, 10); hidden and other files are none.
fn month_of(path: &Path) -> Option<(i16, i8)> {
    let name = path.file_name()?.to_str()?.strip_suffix(".toml")?;
    let (year, month) = name.split_once('-')?;
    let (year, month): (i16, i8) = (year.parse().ok()?, month.parse().ok()?);
    (name.len() == 7 && (1..=12).contains(&month)).then_some((year, month))
}

/// The file a date is kept in.
pub fn month_path(dir: &Path, date: Date) -> PathBuf {
    dir.join(format!("{:04}-{:02}.toml", date.year(), date.month()))
}

fn weather_of(text: &str) -> Option<Weather> {
    match text.trim() {
        "clear" => Some(Weather::Clear),
        "haze" => Some(Weather::Haze),
        "fog" => Some(Weather::Fog),
        _ => None,
    }
}

fn weather_id(weather: Weather) -> &'static str {
    match weather {
        Weather::Clear => "clear",
        Weather::Haze => "haze",
        Weather::Fog => "fog",
    }
}

/// A time as files may hold it: RFC 3339 text (as written here), a TOML
/// date-time written by hand, or Unix seconds.
fn stamp_of(value: &toml::Value) -> Option<i64> {
    let text = match value {
        toml::Value::Integer(seconds) => return Some(*seconds),
        toml::Value::String(text) => text.clone(),
        toml::Value::Datetime(datetime) => datetime.to_string(),
        _ => return None,
    };
    text.parse::<Timestamp>().ok().or_else(|| text.parse::<Zoned>().ok().map(|z| z.timestamp())).map(Timestamp::as_second)
}

/// A time as written here: "2026-10-06T17:04:12+02:00", in `zone`.
fn stamp_text(at: i64, zone: &jiff::tz::TimeZone) -> String {
    let stamp = Timestamp::from_second(at).unwrap_or(Timestamp::UNIX_EPOCH);
    stamp.display_with_offset(zone.to_offset(stamp)).to_string()
}

fn review_of(table: &toml::Table) -> Review {
    Review {
        at: table.get("at").and_then(stamp_of).unwrap_or(0),
        felt: table.get("felt").and_then(toml::Value::as_str).and_then(Felt::parse),
        mix: table.get("mix").and_then(toml::Value::as_str).and_then(Mix::parse),
        memo: table.get("memo").and_then(toml::Value::as_str).unwrap_or_default().to_string(),
    }
}

fn day_of(table: &toml::Table) -> Day {
    let weather = |name: &str| table.get(name).and_then(toml::Value::as_str).and_then(weather_of);
    let review = |name: &str| table.get(name).and_then(toml::Value::as_table).map(review_of);
    Day { weather: weather("weather"), weather_at: table.get("weather_at").and_then(stamp_of), later: weather("later"), work: review("work"), night: review("night") }
}

/// One month's days; none when the file is not there.
fn read_month(path: &Path) -> Result<BTreeMap<Date, Day>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| format!("{}: {e}", path.display()))?;
    Ok(table.iter().filter_map(|(key, value)| Some((key.parse::<Date>().ok()?, day_of(value.as_table()?)))).collect())
}

/// Changes one date's table in its month file, the rest of the file kept as
/// written (comments, what a newer Sioul adds); under the file's lock, as the
/// sharing writes in it too. A file that does not read is never written over.
fn change(dir: &Path, date: Date, edit: impl FnOnce(&mut toml_edit::Table)) -> Result<(), String> {
    let path = month_path(dir, date);
    crate::filelock::with_lock(&path, || {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(fail(e)),
        };
        let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| format!("{}: {e}", path.display()))?;
        let key = date.to_string();
        let item = doc.entry(&key).or_insert_with(|| {
            let mut table = toml_edit::Table::new();
            // No empty "[2026-10-06]" above "[2026-10-06.work]".
            table.set_implicit(true);
            toml_edit::Item::Table(table)
        });
        if let Some(inline) = item.as_inline_table().cloned() {
            *item = toml_edit::Item::Table(inline.into_table());
        }
        let table = item.as_table_mut().ok_or_else(|| format!("{}: {key} is not a table", path.display()))?;
        edit(table);
        std::fs::create_dir_all(dir).map_err(fail)?;
        // Hidden while written: the sharing and sync apps leave it alone.
        let temporary = dir.join(format!(".{}.new", path.file_name().and_then(|n| n.to_str()).unwrap_or("month.toml")));
        std::fs::write(&temporary, doc.to_string()).map_err(fail)?;
        std::fs::rename(&temporary, &path).map_err(fail)
    })
}

/// Keeps a review of `date`, whole: given again, it replaces the one before.
pub fn save(dir: &Path, date: Date, kind: Kind, review: &Review, zone: &jiff::tz::TimeZone) -> Result<(), String> {
    let mut table = toml_edit::Table::new();
    table.insert("at", toml_edit::value(stamp_text(review.at, zone)));
    if let Some(felt) = review.felt {
        table.insert("felt", toml_edit::value(felt.id()));
    }
    if let Some(mix) = review.mix {
        table.insert("mix", toml_edit::value(mix.id()));
    }
    let memo = review.memo.trim_end();
    if !memo.trim().is_empty() {
        table.insert("memo", toml_edit::value(memo));
    }
    change(dir, date, |day| {
        day.insert(kind.id(), toml_edit::Item::Table(table));
    })
}

/// The weather said for `date` at `at` (Unix seconds): the morning's when it
/// is the first, or a correction within `CORRECTION` of it; else a change
/// later in the day, forgotten when it comes back to the morning's.
pub fn note_weather(dir: &Path, date: Date, weather: Weather, at: i64, zone: &jiff::tz::TimeZone) -> Result<(), String> {
    change(dir, date, |day| {
        let first = day.get("weather").and_then(toml_edit::Item::as_str).and_then(weather_of);
        let first_at = day.get("weather_at").and_then(toml_edit::Item::as_str).and_then(|t| stamp_of(&toml::Value::String(t.to_string())));
        let correcting = first_at.is_some_and(|t| (0..=CORRECTION).contains(&(at - t)));
        match first {
            None => {
                day.insert("weather", toml_edit::value(weather_id(weather)));
                day.insert("weather_at", toml_edit::value(stamp_text(at, zone)));
                day.remove("later");
            }
            Some(_) if correcting => {
                day.insert("weather", toml_edit::value(weather_id(weather)));
                day.remove("later");
            }
            Some(morning) if morning == weather => {
                day.remove("later");
            }
            Some(_) => {
                day.insert("later", toml_edit::value(weather_id(weather)));
            }
        }
    })
}

/// The day a night belongs to: the evening it starts, or, its bedtime (`bed`,
/// Unix seconds) before noon, the evening before (Health's `night_of`).
pub fn night_day(bed: i64, zone: &jiff::tz::TimeZone) -> Option<Date> {
    let bed = Timestamp::from_second(bed).ok()?.to_zoned(zone.clone());
    if bed.hour() < 12 { bed.date().yesterday().ok() } else { Some(bed.date()) }
}

/// What the status line offers now (docs/reviews.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    pub kind: Kind,
    /// The day it closes.
    pub date: Date,
}

/// Where the day stands, for the offer.
pub struct Moment<'a> {
    pub now: &'a Zoned,
    pub mode: &'a Mode,
    /// When today's last hours of work or admin end; none on a day without
    /// them (a day off, time off, no hours set).
    pub work_end: Option<Zoned>,
    /// The night now or next, as Health keeps it: (winding down from, bed at), Unix seconds.
    pub night: Option<(i64, i64)>,
    /// The work day closed today with "Done for today".
    pub closed_today: bool,
}

impl Moment<'_> {
    /// Whether it is the evening, and the day it closes: the last `EVENING`
    /// before the night's wind-down; without a night set, from `EVENING_FROM`
    /// to `EVENING_ENDS`, a day closed before `DAY_TURNS` being the day before.
    pub fn evening(&self) -> (bool, Date) {
        let stamp = self.now.timestamp().as_second();
        let today = self.now.date();
        match self.night {
            Some((from, bed)) if from - EVENING <= stamp && stamp < from => (true, night_day(bed, self.now.time_zone()).unwrap_or(today)),
            // In the night itself the day is the night's; after it, today.
            Some((from, bed)) if from <= stamp => (false, night_day(bed, self.now.time_zone()).unwrap_or(today)),
            Some(_) => (false, today),
            None => {
                let hour = self.now.hour();
                let day = if hour < DAY_TURNS { today.yesterday().unwrap_or(today) } else { today };
                (hour >= EVENING_FROM || hour < EVENING_ENDS, day)
            }
        }
    }
}

/// What the status line offers now: closing the work day once today's last
/// hours of work or admin are over, until the evening; then closing the day,
/// once the work day is over. Nothing while you sleep (the wind-down
/// included) or work, nor once that review is given; the next morning never
/// opens on yesterday.
pub fn offer(moment: &Moment, reviews: &Reviews) -> Option<Offer> {
    // Nothing offered at work, asleep or paused, nor in Free time (docs/pauses.md).
    if moment.mode.sleeps() || moment.mode.time.works() || moment.mode.free() {
        return None;
    }
    let stamp = moment.now.timestamp().as_second();
    let ended = moment.work_end.as_ref().is_some_and(|end| stamp >= end.timestamp().as_second());
    let work_over = moment.closed_today || moment.work_end.is_none() || ended;
    let (evening, day) = moment.evening();
    if evening {
        let given = reviews.day(day).is_some_and(|d| d.night.is_some());
        return (work_over && !given).then_some(Offer { kind: Kind::Night, date: day });
    }
    let today = moment.now.date();
    let given = reviews.day(today).is_some_and(|d| d.work.is_some());
    (ended && !moment.closed_today && !given).then_some(Offer { kind: Kind::Work, date: today })
}

// ---------------------------------------------------------------- words

fn say(tr: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut a = args();
    for (name, value) in pairs {
        a.set(*name, value.clone());
    }
    tr.text(id, Some(&a))
}

fn weather_word(tr: &Translator, weather: Weather) -> String {
    tr.text(&format!("review-weather-{}", weather_id(weather)), None)
}

/// How the day started and what the plan did with it, in words, only what was
/// said: "This morning: haze. The plan kept a lighter day." Nothing when the
/// weather was never said.
pub fn morning_lines(day: Option<&Day>, tr: &Translator) -> Vec<String> {
    let Some(morning) = day.and_then(|d| d.weather) else { return Vec::new() };
    let later = day.and_then(|d| d.later).filter(|l| *l != morning);
    let mut line = match later {
        None => say(tr, "review-morning", &[("weather", weather_word(tr, morning))]),
        Some(later) => say(tr, "review-morning-later", &[("weather", weather_word(tr, morning)), ("later", weather_word(tr, later))]),
    };
    let plan = match (morning, later) {
        (_, Some(later)) if later.room() < morning.room() => Some("review-plan-lighter"),
        (_, Some(_)) => Some("review-plan-roomier"),
        (Weather::Haze, None) => Some("review-plan-haze"),
        (Weather::Fog, None) => Some("review-plan-fog"),
        (Weather::Clear, None) => None,
    };
    if let Some(plan) = plan {
        line.push(' ');
        line.push_str(&tr.text(plan, None));
    }
    vec![line]
}

/// What a review said, in words: "heavy; the mix: too much"; "a note" when
/// only a note was written; none when nothing was said.
pub fn said_words(review: &Review, tr: &Translator) -> Option<String> {
    let felt = review.felt.map(|f| tr.text(&format!("review-felt-word-{}", f.id().replace('_', "-")), None));
    let mix = review.mix.map(|m| tr.text(&format!("review-mix-word-{}", m.id().replace('_', "-")), None));
    match (felt, mix) {
        (Some(felt), Some(mix)) => Some(say(tr, "review-said-both", &[("felt", felt), ("mix", mix)])),
        (Some(felt), None) => Some(felt),
        (None, Some(mix)) => Some(say(tr, "review-said-mix", &[("mix", mix)])),
        (None, None) => (!review.memo.trim().is_empty()).then(|| tr.text("review-said-note", None)),
    }
}

/// A past day in one line, as it was said: "Morning: haze · End of work:
/// heavy; the mix: about right · The day: usual".
pub fn back_line(day: &Day, tr: &Translator) -> String {
    let mut parts = Vec::new();
    if let Some(weather) = day.weather {
        parts.push(say(tr, "review-back-morning", &[("weather", weather_word(tr, weather))]));
    }
    if let Some(words) = day.work.as_ref().and_then(|r| said_words(r, tr)) {
        parts.push(say(tr, "review-back-work", &[("words", words)]));
    }
    if let Some(words) = day.night.as_ref().and_then(|r| said_words(r, tr)) {
        parts.push(say(tr, "review-back-night", &[("words", words)]));
    }
    parts.join("  ·  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{Time, Week};
    use crate::quiet::Reason;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-reviews-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn paris() -> jiff::tz::TimeZone {
        jiff::tz::TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    fn seconds(text: &str) -> i64 {
        at(text).timestamp().as_second()
    }

    #[test]
    fn a_review_kept_and_read_back() {
        let dir = folder("round");
        let date: Date = "2026-10-06".parse().unwrap();
        let review = Review { at: seconds("2026-10-06T17:04:12[Europe/Paris]"), felt: Some(Felt::Heavy), mix: Some(Mix::AboutRight), memo: "The bank called back.\n\n- the form *sent*\n- \"quotes\" and \\ kept".into() };
        save(&dir, date, Kind::Work, &review, &paris()).unwrap();
        let text = std::fs::read_to_string(dir.join("2026-10.toml")).unwrap();
        assert!(text.contains("[2026-10-06.work]") && text.contains("at = \"2026-10-06T17:04:12+02:00\"") && text.contains("felt = \"heavy\"") && text.contains("mix = \"about_right\""), "{text}");
        assert!(!text.contains("[2026-10-06]\n"), "no empty table above: {text}");
        assert!(text.contains("\"\"\"") || text.contains("'''"), "a note on several lines reads as lines: {text}");
        let read = Reviews::load(&dir);
        assert_eq!(read.day(date).unwrap().work.as_ref(), Some(&review));
        assert_eq!(read.day(date).unwrap().outcome(), Some(Mix::AboutRight));
        // Given again, it replaces the one before, whole.
        let again = Review { at: review.at + 60, felt: Some(Felt::Usual), mix: None, memo: String::new() };
        save(&dir, date, Kind::Work, &again, &paris()).unwrap();
        assert_eq!(Reviews::load(&dir).day(date).unwrap().work.as_ref(), Some(&again));
        // Closed without a word: kept as such, and counts for nothing.
        let silent = Review { at: review.at, ..Review::default() };
        save(&dir, "2026-10-07".parse().unwrap(), Kind::Work, &silent, &paris()).unwrap();
        let read = Reviews::load(&dir);
        let day = read.day("2026-10-07".parse().unwrap()).unwrap();
        assert!(day.work.is_some() && !day.said() && day.outcome().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn both_reviews_of_one_day() {
        let dir = folder("both");
        let date: Date = "2026-10-06".parse().unwrap();
        let work = Review { at: seconds("2026-10-06T17:00[Europe/Paris]"), felt: Some(Felt::Heavy), mix: Some(Mix::TooMuch), memo: String::new() };
        let night = Review { at: seconds("2026-10-06T22:10[Europe/Paris]"), felt: Some(Felt::Usual), mix: Some(Mix::AboutRight), memo: "Dinner with friends.".into() };
        save(&dir, date, Kind::Work, &work, &paris()).unwrap();
        save(&dir, date, Kind::Night, &night, &paris()).unwrap();
        let read = Reviews::load(&dir);
        let day = read.day(date).unwrap();
        assert_eq!((day.work.as_ref(), day.night.as_ref()), (Some(&work), Some(&night)));
        // The night's mix first; without it, the work's.
        assert_eq!(day.outcome(), Some(Mix::AboutRight));
        let only_work = Day { night: None, ..day.clone() };
        assert_eq!(only_work.outcome(), Some(Mix::TooMuch));
        assert!(Mix::TooMuch.bad() && !Mix::TooMuch.fine_for_costs() && Mix::TooEmpty.fine_for_costs() && !Mix::TooEmpty.good_for_gain() && Mix::AboutRight.good_for_gain());
        let without_mix = Day { work: Some(Review { mix: None, ..work.clone() }), night: Some(Review { mix: None, ..night }), ..Day::default() };
        assert_eq!(without_mix.outcome(), None, "a day without a mix counts for nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn older_and_hand_written_files() {
        let dir = folder("old");
        std::fs::create_dir_all(&dir).unwrap();
        // Unix seconds, a TOML date-time typed by hand, a word from a newer
        // Sioul, a field it adds, a comment: all read, and kept on writing.
        let text = "# My days.\n[2026-09-30]\nweather = \"fog\"\nmood_tags = [\"rain\"]\n\n[2026-09-30.work]\nat = 1790000000\nfelt = \"electric\"\nmix = \"too_much\"\nkind = \"newer\"\n\n[2026-09-29.night]\nat = 2026-09-29T22:30:00+02:00\nfelt = \"gave_back\"\n";
        std::fs::write(dir.join("2026-09.toml"), text).unwrap();
        // Hidden and stray files are no months.
        std::fs::write(dir.join(".2026-09.toml.new"), "broken [").unwrap();
        std::fs::write(dir.join("notes.toml"), "x = 1").unwrap();
        let read = Reviews::load(&dir);
        let day = read.day("2026-09-30".parse().unwrap()).unwrap();
        assert_eq!((day.weather, day.work.as_ref().map(|r| (r.at, r.felt, r.mix))), (Some(Weather::Fog), Some((1_790_000_000, None, Some(Mix::TooMuch)))));
        let night = read.day("2026-09-29".parse().unwrap()).unwrap().night.clone().unwrap();
        assert_eq!((night.at, night.felt), (seconds("2026-09-29T22:30[Europe/Paris]"), Some(Felt::GaveBack)));
        // A night review added: the rest stays as written.
        save(&dir, "2026-09-30".parse().unwrap(), Kind::Night, &Review { at: 1_790_010_000, felt: Some(Felt::Light), ..Review::default() }, &paris()).unwrap();
        let after = std::fs::read_to_string(dir.join("2026-09.toml")).unwrap();
        assert!(after.starts_with("# My days.") && after.contains("mood_tags = [\"rain\"]") && after.contains("kind = \"newer\"") && after.contains("felt = \"electric\""), "{after}");
        // A month that does not read is left out, and never written over.
        std::fs::write(dir.join("2026-08.toml"), "[2026-08-01\nbroken").unwrap();
        assert!(Reviews::load(&dir).day("2026-09-30".parse().unwrap()).is_some());
        assert!(save(&dir, "2026-08-02".parse().unwrap(), Kind::Work, &Review::default(), &paris()).is_err());
        assert_eq!(std::fs::read_to_string(dir.join("2026-08.toml")).unwrap(), "[2026-08-01\nbroken");
        // Only the months asked for.
        let between = Reviews::load_between(&dir, "2026-09-30".parse().unwrap(), "2026-10-31".parse().unwrap());
        assert_eq!(between.days.keys().map(Date::to_string).collect::<Vec<_>>(), ["2026-09-30"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_day_with_only_its_weather() {
        let dir = folder("weather");
        let date: Date = "2026-10-06".parse().unwrap();
        let morning = seconds("2026-10-06T08:40[Europe/Paris]");
        note_weather(&dir, date, Weather::Clear, morning, &paris()).unwrap();
        // Said again ten minutes later: a correction.
        note_weather(&dir, date, Weather::Haze, morning + 600, &paris()).unwrap();
        let read = Reviews::load(&dir);
        let day = read.day(date).unwrap();
        assert_eq!((day.weather, day.later, day.work.as_ref(), day.outcome()), (Some(Weather::Haze), None, None, None));
        assert!(!day.said());
        let text = std::fs::read_to_string(dir.join("2026-10.toml")).unwrap();
        assert!(text.contains("[2026-10-06]") && text.contains("weather = \"haze\"") && text.contains("weather_at = \"2026-10-06T08:40:00+02:00\""), "{text}");
        // In the afternoon, fog: the morning stays haze; back to haze, the change is forgotten.
        note_weather(&dir, date, Weather::Fog, morning + 6 * 3600, &paris()).unwrap();
        assert_eq!(Reviews::load(&dir).day(date).map(|d| (d.weather, d.later)), Some((Some(Weather::Haze), Some(Weather::Fog))));
        note_weather(&dir, date, Weather::Haze, morning + 7 * 3600, &paris()).unwrap();
        assert_eq!(Reviews::load(&dir).day(date).map(|d| (d.weather, d.later)), Some((Some(Weather::Haze), None)));
        // A review that day keeps the weather.
        save(&dir, date, Kind::Night, &Review { at: morning + 14 * 3600, mix: Some(Mix::TooEmpty), ..Review::default() }, &paris()).unwrap();
        let read = Reviews::load(&dir);
        assert_eq!(read.day(date).map(|d| (d.weather, d.outcome())), Some((Some(Weather::Haze), Some(Mix::TooEmpty))));
        assert_eq!(read.recent("2026-10-07".parse().unwrap(), 14).len(), 1);
        assert!(read.recent(date, 14).is_empty(), "the day closed is not among the days before it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_the_status_line_offers() {
        let mode = |time: Time, reason: Reason| Mode { quiet: !time.works(), time, week: Week::default(), reason, until: None, back: None, label: String::new() };
        let evening = mode(Time::Leisure, Reason::Evening);
        let none = Reviews::default();
        let friday = |time: &str| at(&format!("2026-10-02T{time}[Europe/Paris]"));
        let end = Some(friday("17:00"));
        // The night: winding down from 22:00, bed at 23:00.
        let night = Some((seconds("2026-10-02T22:00[Europe/Paris]"), seconds("2026-10-02T23:00[Europe/Paris]")));
        let offered = |now: &Zoned, mode: &Mode, reviews: &Reviews, closed: bool, night: Option<(i64, i64)>| offer(&Moment { now, mode, work_end: end.clone(), night, closed_today: closed }, reviews).map(|o| (o.kind, o.date.to_string()));
        // At work: nothing; once the hours are over: the work day, until the evening.
        assert_eq!(offered(&friday("16:30"), &mode(Time::Work, Reason::Working), &none, false, night), None);
        assert_eq!(offered(&friday("17:05"), &evening, &none, false, night), Some((Kind::Work, "2026-10-02".into())));
        assert_eq!(offered(&friday("18:30"), &mode(Time::Meals, Reason::Meal), &none, false, night), Some((Kind::Work, "2026-10-02".into())), "a meal is no sleep");
        // Working late, or the day closed with "Done for today": no work review offered.
        assert_eq!(offered(&friday("17:30"), &mode(Time::Work, Reason::WorkingLate), &none, false, night), None);
        assert_eq!(offered(&friday("17:30"), &mode(Time::Leisure, Reason::DoneForTheDay), &none, true, night), None);
        // Given: not offered again.
        let mut given = Reviews::default();
        given.days.insert("2026-10-02".parse().unwrap(), Day { work: Some(Review { at: 1, ..Review::default() }), ..Day::default() });
        assert_eq!(offered(&friday("17:30"), &evening, &given, false, night), None);
        // The evening, three hours before winding down: the day, whatever the work review.
        assert_eq!(offered(&friday("19:00"), &evening, &none, false, night), Some((Kind::Night, "2026-10-02".into())));
        assert_eq!(offered(&friday("21:59"), &evening, &given, false, night), Some((Kind::Night, "2026-10-02".into())));
        // Winding down is sleep's: nothing; nor the next morning.
        assert_eq!(offered(&friday("22:10"), &mode(Time::Sleep, Reason::WindingDown), &none, false, night), None);
        let saturday = at("2026-10-03T08:00[Europe/Paris]");
        let day_off = mode(Time::Leisure, Reason::DayOff);
        assert_eq!(offer(&Moment { now: &saturday, mode: &day_off, work_end: None, night: None, closed_today: false }, &none), None);
        // A night after midnight: the evening before is the day closed.
        let late = Some((seconds("2026-10-03T00:30[Europe/Paris]"), seconds("2026-10-03T01:30[Europe/Paris]")));
        assert_eq!(offered(&friday("23:45"), &evening, &none, false, late), Some((Kind::Night, "2026-10-02".into())));
        // Without a night set: from 20:00 to 02:00; before 05:00, the day before.
        assert_eq!(offered(&friday("19:30"), &evening, &given, false, None), None);
        assert_eq!(offered(&friday("20:00"), &evening, &given, false, None), Some((Kind::Night, "2026-10-02".into())));
        assert_eq!(offered(&at("2026-10-03T01:00[Europe/Paris]"), &evening, &none, false, None), Some((Kind::Night, "2026-10-02".into())));
        // A day without hours: the evening only.
        let day_off = |now: &Zoned| offer(&Moment { now, mode: &evening, work_end: None, night, closed_today: false }, &none).map(|o| o.kind);
        assert_eq!((day_off(&friday("15:00")), day_off(&friday("20:00"))), (None, Some(Kind::Night)));
        // Work until 21:00 in the evening: the day once work is over.
        let long = |now: &Zoned| offer(&Moment { now, mode: &evening, work_end: Some(friday("21:00")), night, closed_today: false }, &none).map(|o| o.kind);
        assert_eq!((long(&friday("20:30")), long(&friday("21:10"))), (None, Some(Kind::Night)));
    }

    #[test]
    fn said_in_words() {
        let tr = Translator::new("en");
        let haze = Day { weather: Some(Weather::Haze), ..Day::default() };
        assert_eq!(morning_lines(Some(&haze), &tr), ["This morning: haze. The plan kept a lighter day."]);
        let changed = Day { weather: Some(Weather::Clear), later: Some(Weather::Fog), ..Day::default() };
        assert_eq!(morning_lines(Some(&changed), &tr), ["This morning: clear; later, fog. The plan lightened the rest of the day."]);
        assert!(morning_lines(Some(&Day::default()), &tr).is_empty() && morning_lines(None, &tr).is_empty(), "never said: nothing");
        let work = Review { at: 1, felt: Some(Felt::Heavy), mix: Some(Mix::TooMuch), memo: String::new() };
        assert_eq!(said_words(&work, &tr).as_deref(), Some("heavy; the mix: too much"));
        assert_eq!(said_words(&Review { memo: "x".into(), ..Review::default() }, &tr).as_deref(), Some("a note"));
        assert_eq!(said_words(&Review::default(), &tr), None);
        let day = Day { weather: Some(Weather::Haze), work: Some(work), night: Some(Review { at: 2, felt: Some(Felt::GaveBack), ..Review::default() }), ..Day::default() };
        assert_eq!(back_line(&day, &tr), "Morning: haze  ·  End of work: heavy; the mix: too much  ·  The day: gave back");
        // French: the same words, its typography.
        let fr = Translator::new("fr");
        assert_eq!(morning_lines(Some(&haze), &fr), ["Ce matin\u{202f}: brume. Le plan a gardé une journée plus légère."]);
    }
}
