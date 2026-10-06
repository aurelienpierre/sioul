// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The day's needs, set first: meals, naps and night sleep (docs/health.md,
//! "Meals, rest and sleep"). Their times are kept free: the plan lays no task
//! in them (`plan::Settings::with_needs`), the day shows meals and naps, and
//! two notices come for each, at most: a heads-up about the work a quarter of
//! an hour before ("No new big task"), then one at the time. Any day can
//! differ from the usual one: a block moved, its times changed, quiet, taken
//! out, or one added for that day only (`Days`, shared with your devices).
//! Nothing about eating or sleeping is recorded, and nothing is said when one passes.
//!
//! From the literature on eating disorders and irregular eating (notes in
//! docs/research/meal-prompts.md): by the clock, at times the person sets,
//! never by hunger (CBT-E, Murphy et al. 2010; NICE NG69); every part set by
//! the person, per block (Lindgreen, Lomborg & Clausen 2018); neutral words,
//! no praise, no numbers, no "missed" (Eikey 2021; Levinson et al. 2017); the
//! heads-up about the work, so it can be left on time (Leroy & Glomb 2018);
//! naps with time to wake up after (Hilditch et al. 2017); sleep with a
//! wind-down before it (Harvey et al. 2021).

use jiff::Zoned;
use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn yes() -> bool {
    true
}

fn every_day() -> [bool; 7] {
    [true; 7]
}

fn quarter() -> u32 {
    15
}

fn an_hour() -> u32 {
    60
}

/// A time kept free, again and again: a meal or a nap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// What it is called, as you like ("Lunch", "Break"); "" for the usual name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// When it starts proper: eating, or the nap ("12:30").
    pub at: String,
    /// Minutes it lasts.
    pub minutes: u32,
    /// Minutes before, to get it ready (a meal); kept free too.
    #[serde(default)]
    pub before: u32,
    /// Minutes after, to come back (waking up from a nap); kept free too.
    #[serde(default)]
    pub after: u32,
    /// The weekdays it comes, Monday first.
    #[serde(default = "every_day")]
    pub days: [bool; 7],
    /// Off: neither kept free nor said.
    #[serde(default = "yes")]
    pub on: bool,
    /// Its two notices; off, it is kept free all the same.
    #[serde(default = "yes")]
    pub notices: bool,
}

impl Block {
    fn new(at: &str, minutes: u32, before: u32, after: u32) -> Block {
        Block { name: String::new(), at: at.into(), minutes, before, after, days: every_day(), on: true, notices: true }
    }
}

/// The night: from winding down to waking up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sleep {
    /// Bedtime ("23:00").
    pub bed: String,
    /// Waking up ("07:00").
    pub wake: String,
    /// Minutes of winding down before bed, kept free: about an hour in the
    /// programmes studied (Harvey et al. 2021).
    #[serde(default = "an_hour")]
    pub wind_down: u32,
    /// Its two notices.
    #[serde(default = "yes")]
    pub notices: bool,
}

impl Default for Sleep {
    fn default() -> Sleep {
        Sleep { bed: "23:00".into(), wake: "07:00".into(), wind_down: an_hour(), notices: true }
    }
}

fn three_meals() -> Vec<Block> {
    vec![Block::new("08:00", 20, 10, 0), Block::new("12:30", 30, 20, 0), Block::new("19:30", 30, 30, 0)]
}

fn a_nap() -> Vec<Block> {
    vec![Block::new("14:00", 20, 0, 15)]
}

/// Meals, naps and the night, each kind on or off; three meals when meals
/// are first turned on, one of them in the evening (Murphy et al. 2010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needs {
    #[serde(default)]
    pub meals_on: bool,
    #[serde(default = "three_meals")]
    pub meals: Vec<Block>,
    #[serde(default)]
    pub naps_on: bool,
    #[serde(default = "a_nap")]
    pub naps: Vec<Block>,
    #[serde(default)]
    pub sleep_on: bool,
    #[serde(default)]
    pub sleep: Sleep,
    /// Minutes between the heads-up and the time it starts.
    #[serde(default = "quarter")]
    pub heads_up: u32,
    /// Minutes "Later" moves a block, today only.
    #[serde(default = "quarter")]
    pub later: u32,
}

impl Default for Needs {
    fn default() -> Needs {
        Needs { meals_on: false, meals: three_meals(), naps_on: false, naps: a_nap(), sleep_on: false, sleep: Sleep::default(), heads_up: quarter(), later: quarter() }
    }
}

/// One time kept free on a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Kept {
    /// "meal:0", "nap:1", "sleep": what "Later" and "Skip today" name.
    pub key: String,
    /// "meal", "nap", "sleep".
    pub kind: &'static str,
    /// Its place among its kind, from 0.
    pub index: usize,
    /// Its name as set; "" for the usual one.
    pub name: String,
    /// Kept free from getting it ready, or winding down (Unix seconds)…
    pub start: i64,
    /// …through when it starts proper (eating, the nap, bed)…
    pub at: i64,
    /// …to its end, waking up included.
    pub end: i64,
    pub notices: bool,
}

/// The notices given today, on this device only, never shared
/// (`$XDG_STATE_HOME/sioul/needs-today.toml`). Nothing about eating or
/// sleeping itself. The day's moves and skips live with each day's changes
/// now (`Days`); those an older Sioul wrote here are read once, to move them there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Today {
    #[serde(default, deserialize_with = "crate::budget::dates::optional", skip_serializing_if = "Option::is_none")]
    pub day: Option<Date>,
    /// Blocks moved, by key, in minutes (an older Sioul's).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shifts: BTreeMap<String, i64>,
    /// Blocks skipped today (an older Sioul's): no notice, kept free all the same.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub skipped: BTreeSet<String>,
    /// Notices given: "<key>:heads-up", "<key>:start".
    #[serde(default)]
    pub sent: BTreeSet<String>,
}

/// One block on one day: a usual one changed that day only, by its key
/// ("meal:1", "nap:0"; "sleep", the night starting that evening), or one
/// added that day only ("added-…"), its kind said (docs/health.md, "Meals,
/// rest and sleep"). Times on the clock and as set: a usual time changed
/// later moves the usual days, never this one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayBlock {
    /// One added that day: "meal" or "nap"; "" for a usual one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// One added that day: its name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// When it starts proper that day: eating, the nap, bed ("13:30"); "" as usual.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub at: String,
    /// The night: waking ("08:30"); "" as usual.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub wake: String,
    /// That day's minutes: it lasts, to get it ready (the night: to wind
    /// down), to come back after it; none as usual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u32>,
    /// "Not that day": no notice; kept free all the same.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quiet: bool,
    /// Taken out that day: neither kept free nor said.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
}

/// Whether a block's key names one added for a day only.
pub fn is_added(key: &str) -> bool {
    key.starts_with("added-")
}

/// How long a past day's changes are kept: two weeks.
const KEPT_DAYS: i64 = 14;

/// What a person reads first in the days' file.
const DAYS_HEADER: &str = "# Meals, naps and nights changed for one day only (Sioul's Health page).\n# The usual ones are in health.toml. Times on the clock; \"off\": not that day;\n# \"quiet\": no notice, still kept free.\n";

/// Each day's changes, by day: `$XDG_DATA_HOME/sioul/health-days.toml`,
/// shared with your own devices as the health file is (the sharing's health
/// part). Days more than two weeks past are dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Days(pub BTreeMap<Date, BTreeMap<String, DayBlock>>);

impl Days {
    pub fn default_path() -> PathBuf {
        crate::config::data_dir().join("health-days.toml")
    }

    /// The days' changes; none when there is no file. One that no longer
    /// reads (edited by hand) is copied aside first (`health-days.toml.unreadable`):
    /// saving over it then loses nothing.
    pub fn load(path: &Path) -> Days {
        match std::fs::read_to_string(path).map(|t| toml::from_str::<Days>(&t)) {
            Ok(Ok(days)) => days,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Days::default(),
            _ => {
                let _ = std::fs::copy(path, path.with_extension("toml.unreadable"));
                Days::default()
            }
        }
    }

    /// Written next to its place, then moved: never half a file. Yours alone.
    /// Days more than two weeks before `today` go, and blocks left as usual.
    pub fn save(&self, path: &Path, today: Date) -> Result<(), String> {
        let mut kept = self.clone();
        let oldest = today.checked_sub(jiff::Span::new().days(KEPT_DAYS)).unwrap_or(today);
        kept.0.retain(|day, blocks| {
            blocks.retain(|_, block| *block != DayBlock::default());
            *day >= oldest && !blocks.is_empty()
        });
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let text = format!("{DAYS_HEADER}\n{}", toml::to_string(&kept).map_err(|e| e.to_string())?);
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, text).map_err(fail)?;
        crate::health::keep_private(&temporary);
        std::fs::rename(&temporary, path).map_err(fail)
    }

    /// A block's change on a day, if any.
    pub fn get(&self, date: Date, key: &str) -> Option<&DayBlock> {
        self.0.get(&date)?.get(key)
    }

    /// The block `key` of `date` changed by `change`; back to usual when
    /// nothing is left of the change.
    pub fn change(&mut self, date: Date, key: &str, change: impl FnOnce(&mut DayBlock)) {
        let blocks = self.0.entry(date).or_default();
        let block = blocks.entry(key.to_string()).or_default();
        change(block);
        if *block == DayBlock::default() {
            blocks.remove(key);
        }
        if blocks.is_empty() {
            self.0.remove(&date);
        }
    }

    /// A block's change taken back: as usual that day (an added one: gone).
    pub fn forget(&mut self, date: Date, key: &str) {
        if let Some(blocks) = self.0.get_mut(&date) {
            blocks.remove(key);
            if blocks.is_empty() {
                self.0.remove(&date);
            }
        }
    }

    /// A usual block taken out of the settings (its kind, its place): its
    /// changes go, and those of the blocks after it follow them to their new
    /// place ("meal:2" becomes "meal:1").
    pub fn removed_usual(&mut self, kind: &str, index: usize) {
        for blocks in self.0.values_mut() {
            *blocks = std::mem::take(blocks)
                .into_iter()
                .filter_map(|(key, block)| match key.split_once(':').filter(|(k, _)| *k == kind).map(|(_, i)| i.parse::<usize>()) {
                    Some(Ok(i)) if i == index => None,
                    Some(Ok(i)) if i > index => Some((format!("{kind}:{}", i - 1), block)),
                    _ => Some((key, block)),
                })
                .collect();
        }
        self.0.retain(|_, blocks| !blocks.is_empty());
    }

    /// A key for a block added on `date`, from now in milliseconds: two devices
    /// adding one the same day never name theirs alike.
    pub fn new_key(&self, date: Date, now_ms: i64) -> String {
        let mut n = now_ms;
        while self.get(date, &format!("added-{n}")).is_some() {
            n += 1;
        }
        format!("added-{n}")
    }
}

/// What a day's block is asked to become, that day only, as the Health page
/// asks it (`Needs::change_day`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DayEdit {
    /// "meal:1", "nap:0", "sleep", "added-…"; none for "add".
    pub key: String,
    /// "later", "move", "times", "quiet", "loud", "off", "on", "usual", "add".
    pub action: String,
    /// "12:10", "13:00": from getting it ready (winding down), to its end (waking).
    pub from: String,
    pub to: String,
    /// The night: bedtime.
    pub bed: String,
    /// A meal's minutes to get it ready; a nap's to come back.
    pub before: Option<u32>,
    pub after: Option<u32>,
    /// "later" by these minutes; 0: as many as "Later" moves by (`Needs::later`).
    pub minutes: i64,
    /// One added: "meal" or "nap", and its name.
    pub kind: String,
    pub name: String,
}

/// Why a day's block was left as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayProblem {
    /// The day has passed: it stays as it was.
    Past,
    /// Times that do not read, or an end before the start.
    Times,
    /// Shorter than five minutes.
    TooShort,
    /// A meal or a nap moved out of its day.
    OtherDay,
    /// An action this version does not know.
    Unknown,
}

/// `at` (Unix seconds) on the clock in `zone`: "12:30".
fn clock(at: i64, zone: &TimeZone) -> String {
    jiff::Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default()
}

/// "8:5" as typed, "08:05" as kept.
fn hh_mm(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

impl Today {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("needs-today.toml")
    }

    /// Today's, or nothing when it was written another day.
    pub fn load(path: &Path, date: Date) -> Today {
        let today: Today = std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        if today.day == Some(date) { today } else { Today { day: Some(date), ..Today::default() } }
    }

    /// The notices due `now` (Unix seconds) among `kept`, each marked given:
    /// (block, true) for the heads-up, `heads_up` minutes before, within ten
    /// minutes of its time and never once the block began; (block, false) at
    /// its time, within a quarter of an hour. Each once; none for a block
    /// skipped today or whose notices are off.
    pub fn due<'a>(&mut self, kept: &'a [Kept], heads_up: u32, now: i64) -> Vec<(&'a Kept, bool)> {
        let mut out = Vec::new();
        for block in kept.iter().filter(|k| k.notices && !self.skipped.contains(&k.key)) {
            let ahead = block.start - i64::from(heads_up) * 60;
            if heads_up > 0 && now >= ahead && now < block.start && now - ahead < 10 * 60 && self.sent.insert(format!("{}:heads-up", block.key)) {
                out.push((block, true));
            }
            if now >= block.start && now - block.start < 15 * 60 && self.sent.insert(format!("{}:start", block.key)) {
                out.push((block, false));
            }
        }
        out
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        std::fs::write(path, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)
    }
}

/// "18:30" → 18:30.
fn time_of(text: &str) -> Option<Time> {
    let (hour, minute) = text.trim().split_once(':')?;
    Time::new(hour.trim().parse().ok()?, minute.trim().parse().ok()?, 0, 0).ok()
}

/// `date` at `time` in `zone`, Unix seconds (a time skipped by a change of hour, after it).
fn at(date: Date, time: Time, zone: &TimeZone) -> Option<i64> {
    date.to_datetime(time).to_zoned(zone.clone()).ok().map(|z| z.timestamp().as_second())
}

impl Needs {
    /// What is kept free around `now`, in order of start: today's and
    /// tomorrow's meals and naps (those added for the day too), and the
    /// nights ending this morning, starting tonight and tomorrow night; each
    /// as its day has it (`days`: moved, times changed), meals pushed past
    /// the events in `held` (their times with their margins, Unix seconds).
    /// A block taken out that day is not there; a quiet one is (it is still
    /// kept: only its notices are off). For whatever asks whether a meal, a
    /// nap or the night is on at some time (the hours' states): kept stable.
    pub fn kept_around(&self, now: &Zoned, days: &Days, held: &[(i64, i64)]) -> Vec<Kept> {
        let zone = now.time_zone();
        let today = now.date();
        let midnight = at(today, Time::midnight(), zone).unwrap_or(0);
        let mut out = Vec::new();
        for date in [Some(today), today.tomorrow().ok()].into_iter().flatten() {
            let pushed = self.past_events_on(date, zone, days, held);
            out.extend(self.day_blocks(date, zone, days, &|key: &str| pushed.get(key).copied().unwrap_or(0)));
        }
        if self.sleep_on {
            for night in [today.yesterday().ok(), Some(today), today.tomorrow().ok()].into_iter().flatten() {
                if let Some(mut kept) = self.night_of(night, zone, days, 0) {
                    // The night ending this morning said its notices yesterday, unless it began after midnight.
                    kept.notices = kept.notices && kept.start >= midnight;
                    out.push(kept);
                }
            }
        }
        out.sort_by_key(|k| k.start);
        out
    }

    /// What is kept free on `date`, in order: each meal and nap on that
    /// weekday, and the nights touching it (the one ending that morning, the
    /// one starting that evening). `shift` moves a block that day only, in
    /// minutes, by its key ("Later").
    pub fn kept_on(&self, date: Date, zone: &TimeZone, shift: &dyn Fn(&str) -> i64) -> Vec<Kept> {
        self.kept_with(date, zone, &Days::default(), shift)
    }

    /// The same with each day's changes (`days`): a block moved, its times
    /// changed, quiet (no notice) or off that day, and those added that day.
    /// Notices are said by the day a block starts on: the night ending this
    /// morning said its own yesterday, unless its bedtime was after midnight.
    pub fn kept_with(&self, date: Date, zone: &TimeZone, days: &Days, shift: &dyn Fn(&str) -> i64) -> Vec<Kept> {
        let mut out = self.day_blocks(date, zone, days, shift);
        if self.sleep_on {
            let midnight = at(date, Time::midnight(), zone).unwrap_or(0);
            let next = date.tomorrow().ok().and_then(|d| at(d, Time::midnight(), zone)).unwrap_or(midnight + 86_400);
            for night in [date.yesterday().ok(), Some(date)].into_iter().flatten() {
                let Some(mut kept) = self.night_of(night, zone, days, if night == date { shift("sleep") } else { 0 }) else { continue };
                if kept.end > midnight && kept.start < next {
                    kept.notices = kept.notices && kept.start >= midnight;
                    out.push(kept);
                }
            }
        }
        out.sort_by_key(|k| k.start);
        out
    }

    /// The blocks that are `date`'s own, in order: its meals and naps, those
    /// added that day, and the night starting that evening (its bedtime after
    /// midnight: the next morning). What the Health page lists for a day.
    pub fn blocks_of(&self, date: Date, zone: &TimeZone, days: &Days, shift: &dyn Fn(&str) -> i64) -> Vec<Kept> {
        let mut out = self.day_blocks(date, zone, days, shift);
        if self.sleep_on
            && let Some(night) = self.night_of(date, zone, days, shift("sleep"))
        {
            out.push(night);
        }
        out.sort_by_key(|k| k.start);
        out
    }

    /// `date`'s meals and naps on that weekday, as changed that day, and those added that day.
    fn day_blocks(&self, date: Date, zone: &TimeZone, days: &Days, shift: &dyn Fn(&str) -> i64) -> Vec<Kept> {
        let mut out = Vec::new();
        let weekday = date.weekday().to_monday_zero_offset() as usize;
        let changes = days.0.get(&date);
        let kept = |key: String, kind: &'static str, index: usize, name: String, start_at: i64, (minutes, before, after): (u32, u32, u32), notices: bool| Kept {
            start: start_at - i64::from(before) * 60,
            at: start_at,
            end: start_at + i64::from(minutes + after) * 60,
            key,
            kind,
            index,
            name,
            notices,
        };
        let mut usual = |kind: &'static str, on: bool, list: &[Block]| {
            if !on {
                return;
            }
            for (index, block) in list.iter().enumerate().filter(|(_, b)| b.on && b.days[weekday]) {
                let key = format!("{kind}:{index}");
                let change = changes.and_then(|c| c.get(&key));
                if change.is_some_and(|c| c.off) {
                    continue;
                }
                let time = change.map(|c| c.at.as_str()).filter(|t| !t.is_empty()).unwrap_or(&block.at);
                let Some(time) = time_of(time) else { continue };
                let Some(start_at) = at(date, time, zone) else { continue };
                let lengths = (change.and_then(|c| c.minutes).unwrap_or(block.minutes), change.and_then(|c| c.before).unwrap_or(block.before), change.and_then(|c| c.after).unwrap_or(block.after));
                let notices = block.notices && !change.is_some_and(|c| c.quiet);
                let shifted = start_at + shift(&key) * 60;
                out.push(kept(key, kind, index, block.name.clone(), shifted, lengths, notices));
            }
        };
        usual("meal", self.meals_on, &self.meals);
        usual("nap", self.naps_on, &self.naps);
        // Added that day only, whatever the usual ones' switches say.
        for (key, block) in changes.into_iter().flatten().filter(|(key, b)| is_added(key) && !b.off) {
            let (kind, minutes) = match block.kind.as_str() {
                "meal" => ("meal", 30),
                "nap" => ("nap", 20),
                _ => continue,
            };
            let Some(start_at) = time_of(&block.at).and_then(|time| at(date, time, zone)) else { continue };
            let lengths = (block.minutes.unwrap_or(minutes), block.before.unwrap_or(0), block.after.unwrap_or(0));
            out.push(kept(key.clone(), kind, 0, block.name.clone(), start_at + shift(key) * 60, lengths, !block.quiet));
        }
        out
    }

    /// The night starting the evening of `night` (its bedtime after
    /// midnight: the next morning), as changed that day, moved `shift`
    /// minutes; none when off that day.
    fn night_of(&self, night: Date, zone: &TimeZone, days: &Days, shift: i64) -> Option<Kept> {
        let change = days.get(night, "sleep");
        if change.is_some_and(|c| c.off) {
            return None;
        }
        let set = |own: Option<&str>, usual: &str| own.filter(|t| !t.is_empty()).map_or_else(|| usual.to_string(), str::to_string);
        let bed = time_of(&set(change.map(|c| c.at.as_str()), &self.sleep.bed))?;
        let wake = time_of(&set(change.map(|c| c.wake.as_str()), &self.sleep.wake))?;
        let wind_down = change.and_then(|c| c.before).unwrap_or(self.sleep.wind_down);
        let bed_day = if bed.hour() < 12 { night.tomorrow().unwrap_or(night) } else { night };
        let bed_at = at(bed_day, bed, zone)?;
        let mut wake_at = at(bed_day, wake, zone)?;
        if wake_at <= bed_at {
            wake_at = at(bed_day.tomorrow().unwrap_or(bed_day), wake, zone).unwrap_or(wake_at + 86_400);
        }
        let (bed_at, wake_at) = (bed_at + shift * 60, wake_at + shift * 60);
        Some(Kept {
            key: "sleep".into(),
            kind: "sleep",
            index: 0,
            name: String::new(),
            start: bed_at - i64::from(wind_down) * 60,
            at: bed_at,
            end: wake_at,
            notices: self.sleep.notices && !change.is_some_and(|c| c.quiet),
        })
    }

    /// The day's meals pushed past the events they would fall in (`held`:
    /// the events' times with their margins, Unix seconds), as minutes by
    /// key: to the end of the event, by whole five minutes; a meal pushed
    /// into a next event goes past it too. Not a meal whose time you set for
    /// that day, nor one added that day: you chose it, the day in view.
    pub fn past_events_on(&self, date: Date, zone: &TimeZone, days: &Days, held: &[(i64, i64)]) -> BTreeMap<String, i64> {
        let mut shifts: BTreeMap<String, i64> = BTreeMap::new();
        let chosen = |key: &str| is_added(key) || days.get(date, key).is_some_and(|c| !c.at.is_empty());
        for _ in 0..8 {
            let current = shifts.clone();
            let mut pushed = false;
            for block in self.day_blocks(date, zone, days, &|key: &str| current.get(key).copied().unwrap_or(0)).iter().filter(|k| k.kind == "meal" && !chosen(&k.key)) {
                if let Some(end) = held.iter().filter(|&&(from, to)| from < block.end && to > block.start).map(|&(_, to)| to).max() {
                    *shifts.entry(block.key.clone()).or_insert(0) += (end - block.start + 299) / 300 * 5;
                    pushed = true;
                }
            }
            if !pushed {
                break;
            }
        }
        shifts
    }

    /// Moves given as minutes by key (`moved`), each meal pushed past the
    /// events it would fall in (`held`: their times with their margins, Unix
    /// seconds): from those moves, later only, to the end of the event, by
    /// whole five minutes; a meal pushed into a next event goes past it too.
    /// The days' changes go through `past_events_on`.
    pub fn past_events(&self, date: Date, zone: &TimeZone, moved: &BTreeMap<String, i64>, held: &[(i64, i64)]) -> BTreeMap<String, i64> {
        let mut shifts = moved.clone();
        for _ in 0..8 {
            let current = shifts.clone();
            let mut pushed = false;
            for block in self.kept_on(date, zone, &|key: &str| current.get(key).copied().unwrap_or(0)).iter().filter(|k| k.kind == "meal") {
                if let Some(end) = held.iter().filter(|&&(from, to)| from < block.end && to > block.start).map(|&(_, to)| to).max() {
                    *shifts.entry(block.key.clone()).or_insert(0) += (end - block.start + 299) / 300 * 5;
                    pushed = true;
                }
            }
            if !pushed {
                break;
            }
        }
        shifts
    }

    /// The spans kept free on `date`, for the plan: no task in them.
    pub fn busy_on(&self, date: Date, zone: &TimeZone, shift: &dyn Fn(&str) -> i64) -> Vec<(i64, i64)> {
        self.kept_on(date, zone, shift).into_iter().map(|k| (k.start, k.end)).collect()
    }

    /// A block's lengths on `date`, as that day has them, in minutes: it
    /// lasts, to get it ready (the night: to wind down), to come back after
    /// it. None for a key that names no block.
    pub fn lengths(&self, date: Date, key: &str, days: &Days) -> Option<(u32, u32, u32)> {
        let change = days.get(date, key);
        if is_added(key) {
            let c = change?;
            return Some((c.minutes.unwrap_or(if c.kind == "nap" { 20 } else { 30 }), c.before.unwrap_or(0), c.after.unwrap_or(0)));
        }
        if key == "sleep" {
            return Some((0, change.and_then(|c| c.before).unwrap_or(self.sleep.wind_down), 0));
        }
        let (kind, index) = key.split_once(':')?;
        let index: usize = index.parse().ok()?;
        let block = match kind {
            "meal" => self.meals.get(index)?,
            "nap" => self.naps.get(index)?,
            _ => return None,
        };
        Some((change.and_then(|c| c.minutes).unwrap_or(block.minutes), change.and_then(|c| c.before).unwrap_or(block.before), change.and_then(|c| c.after).unwrap_or(block.after)))
    }

    /// `edit` made on `days`, for `date` only, as the Health page asks:
    /// "later" (by `minutes`, else as many as "Later" moves by), "move" (to
    /// start at `from`), "times" (`from`–`to`; a meal's minutes to get it
    /// ready, a nap's to come back, the night's bedtime), "quiet" and "loud"
    /// (no notice that day, or back), "off" and "on" (taken out that day, or
    /// put back; one added that day goes), "usual" (as usual that day), "add"
    /// (one for that day only, a meal or a rest). A day before `now`'s stays
    /// as it was. Moves count from the block as the page shows it (meals
    /// pushed past the events in `held`), and are kept as times on the clock;
    /// lengths only where they differ from the usual ones.
    pub fn change_day(&self, days: &mut Days, date: Date, edit: &DayEdit, now: &Zoned, held: &[(i64, i64)]) -> Result<(), DayProblem> {
        if date < now.date() {
            return Err(DayProblem::Past);
        }
        let zone = now.time_zone().clone();
        let pushed = self.past_events_on(date, &zone, days, held);
        let blocks = self.blocks_of(date, &zone, days, &|key: &str| pushed.get(key).copied().unwrap_or(0));
        let current = blocks.iter().find(|k| k.key == edit.key);
        let key = edit.key.as_str();
        let night = key == "sleep";
        // A time on that day; the night's before noon, the next morning's.
        let on_day = |time: Time, night: bool| -> Option<i64> {
            let day = if night && time.hour() < 12 { date.tomorrow().ok()? } else { date };
            at(day, time, &zone)
        };
        let time = |text: &str, night: bool| time_of(text).and_then(|t| on_day(t, night)).ok_or(DayProblem::Times);
        // That day's lengths only where they differ from the usual ones; an added one's all.
        let added = is_added(key);
        let usual = if added { None } else { self.lengths(date, key, &Days::default()) };
        let own = |value: u32, usual: Option<u32>| if usual == Some(value) && !added { None } else { Some(value) };
        // Minutes from `start` to `end`, less what comes after: five at least.
        let lasting = |start: i64, end: i64, after: u32| u32::try_from((end - start) / 60 - i64::from(after)).ok().filter(|m| *m >= 5).ok_or(DayProblem::TooShort);
        match edit.action.as_str() {
            "later" | "move" => {
                let Some(k) = current else { return Ok(()) };
                let delta = if edit.action == "later" { 60 * if edit.minutes == 0 { i64::from(self.later) } else { edit.minutes } } else { time(&edit.from, night)? - k.start };
                // A meal or a nap stays on its day.
                if !night && jiff::Timestamp::from_second(k.at + delta).map(|t| t.to_zoned(zone.clone()).date()).ok() != Some(date) {
                    return Err(DayProblem::OtherDay);
                }
                days.change(date, key, |b| {
                    b.at = clock(k.at + delta, &zone);
                    if night {
                        b.wake = clock(k.end + delta, &zone);
                    }
                });
            }
            "times" => {
                let (Some(k), Some((_, before, after))) = (current, self.lengths(date, key, days)) else { return Ok(()) };
                let (from, to) = (time_of(&edit.from).ok_or(DayProblem::Times)?, time_of(&edit.to).ok_or(DayProblem::Times)?);
                if night {
                    let bed = time_of(&edit.bed).ok_or(DayProblem::Times)?;
                    let (from_at, bed_at) = (on_day(from, true).ok_or(DayProblem::Times)?, on_day(bed, true).ok_or(DayProblem::Times)?);
                    let wind_down = u32::try_from((bed_at - from_at) / 60).map_err(|_| DayProblem::Times)?;
                    days.change(date, key, |b| {
                        b.at = hh_mm(bed);
                        b.wake = hh_mm(to);
                        b.before = own(wind_down, usual.map(|u| u.1));
                    });
                } else {
                    let (from_at, to_at) = (on_day(from, false).ok_or(DayProblem::Times)?, on_day(to, false).ok_or(DayProblem::Times)?);
                    if to_at <= from_at {
                        return Err(DayProblem::Times);
                    }
                    let before = if k.kind == "meal" { edit.before.unwrap_or(before) } else { before };
                    let after = if k.kind == "nap" { edit.after.unwrap_or(after) } else { after };
                    let start = from_at + i64::from(before) * 60;
                    let minutes = lasting(start, to_at, after)?;
                    days.change(date, key, |b| {
                        b.at = clock(start, &zone);
                        b.minutes = own(minutes, usual.map(|u| u.0));
                        b.before = own(before, usual.map(|u| u.1));
                        b.after = own(after, usual.map(|u| u.2));
                    });
                }
            }
            "quiet" | "loud" => days.change(date, key, |b| b.quiet = edit.action == "quiet"),
            "off" if added => days.forget(date, key),
            "off" => days.change(date, key, |b| b.off = true),
            "on" => days.change(date, key, |b| b.off = false),
            "usual" => {
                if !added {
                    days.forget(date, key);
                }
            }
            "add" => {
                let kind = if edit.kind == "nap" { "nap" } else { "meal" };
                let (from_at, to_at) = (time(&edit.from, false)?, time(&edit.to, false)?);
                if to_at <= from_at {
                    return Err(DayProblem::Times);
                }
                let before = if kind == "meal" { edit.before.unwrap_or(0) } else { 0 };
                let after = if kind == "nap" { edit.after.unwrap_or(0) } else { 0 };
                let start = from_at + i64::from(before) * 60;
                let minutes = lasting(start, to_at, after)?;
                let new = days.new_key(date, now.timestamp().as_millisecond());
                days.change(date, &new, |b| {
                    b.kind = kind.to_string();
                    b.name = edit.name.trim().to_string();
                    b.at = clock(start, &zone);
                    b.minutes = Some(minutes);
                    b.before = Some(before);
                    b.after = Some(after);
                });
            }
            _ => return Err(DayProblem::Unknown),
        }
        Ok(())
    }

    /// Meals planned more than four hours apart, as (end of one, start of the
    /// next) on a usual day: said quietly where they are set, never in a
    /// notice (Murphy et al. 2010: rarely more than four hours between).
    pub fn long_gaps(&self) -> Vec<(String, String)> {
        if !self.meals_on {
            return Vec::new();
        }
        // Minutes of the day: when each meal starts, and ends.
        let minutes = |t: Time| i64::from(t.hour()) * 60 + i64::from(t.minute());
        let mut times: Vec<(i64, i64)> = self.meals.iter().filter(|m| m.on).filter_map(|m| time_of(&m.at).map(|t| (minutes(t), minutes(t) + i64::from(m.minutes)))).collect();
        times.sort_unstable();
        let clock = |m: i64| format!("{:02}:{:02}", m / 60 % 24, m % 60);
        times.windows(2).filter(|w| w[1].0 - w[0].1 > 4 * 60).map(|w| (clock(w[0].1), clock(w[1].0))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn hm(seconds: i64) -> String {
        jiff::Timestamp::from_second(seconds).unwrap().to_zoned(zone()).strftime("%d %H:%M").to_string()
    }

    fn none(_: &str) -> i64 {
        0
    }

    #[test]
    fn meals_pushed_past_events() {
        let monday: Date = "2026-10-05".parse().unwrap();
        let needs = Needs { meals_on: true, ..Needs::default() };
        let at = |text: &str| format!("2026-10-05T{text}").parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone()).unwrap().timestamp().as_second();
        // A meeting 12:00–13:15, a quarter of an hour to come back: lunch (made from 12:10) from 13:30.
        let shifts = needs.past_events(monday, &zone(), &BTreeMap::new(), &[(at("12:00"), at("13:30"))]);
        assert_eq!(shifts.get("meal:1"), Some(&80));
        assert_eq!(shifts.get("meal:0"), None, "breakfast untouched");
        // Moved already to 14:00 by you: past the meeting, left there.
        let mine = BTreeMap::from([("meal:1".to_string(), 110)]);
        assert_eq!(needs.past_events(monday, &zone(), &mine, &[(at("12:00"), at("13:30"))]).get("meal:1"), Some(&110));
        // Pushed into a second event: past it too, on whole five minutes.
        let shifts = needs.past_events(monday, &zone(), &BTreeMap::new(), &[(at("12:00"), at("13:30")), (at("13:40"), at("14:02"))]);
        let kept = needs.kept_on(monday, &zone(), &|key: &str| shifts.get(key).copied().unwrap_or(0));
        assert_eq!(hm(kept.iter().find(|k| k.key == "meal:1").unwrap().start), "05 14:05");
    }

    #[test]
    fn meals_naps_and_nights_kept_free() {
        let monday: Date = "2026-10-05".parse().unwrap();
        // Off, nothing kept.
        assert!(Needs::default().kept_on(monday, &zone(), &none).is_empty());
        let needs = Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() };
        let kept: Vec<(String, String, String, String)> = needs.kept_on(monday, &zone(), &none).iter().map(|k| (k.key.clone(), hm(k.start), hm(k.at), hm(k.end))).collect();
        assert_eq!(
            kept,
            [
                // Sunday's night, ending this morning; winding down from 22:00.
                ("sleep".to_string(), "04 22:00".to_string(), "04 23:00".to_string(), "05 07:00".to_string()),
                ("meal:0".into(), "05 07:50".into(), "05 08:00".into(), "05 08:20".into()),
                ("meal:1".into(), "05 12:10".into(), "05 12:30".into(), "05 13:00".into()),
                // A nap, and a quarter of an hour to come back after it.
                ("nap:0".into(), "05 14:00".into(), "05 14:00".into(), "05 14:35".into()),
                ("meal:2".into(), "05 19:00".into(), "05 19:30".into(), "05 20:00".into()),
                ("sleep".into(), "05 22:00".into(), "05 23:00".into(), "06 07:00".into()),
            ]
        );
        // Only the evening's night says its notices: the one ending this morning said them yesterday.
        assert!(needs.kept_on(monday, &zone(), &none).iter().filter(|k| k.kind == "sleep").map(|k| k.notices).eq([false, true]));
        // "Later": lunch moved a quarter of an hour, today only.
        let later = |key: &str| if key == "meal:1" { 15 } else { 0 };
        let lunch = needs.kept_on(monday, &zone(), &later).into_iter().find(|k| k.key == "meal:1").unwrap();
        assert_eq!((hm(lunch.start), hm(lunch.end)), ("05 12:25".into(), "05 13:15".into()));
        // A block off on Sundays, and one off altogether.
        let mut weekdays = needs.clone();
        weekdays.meals[0].days[6] = false;
        weekdays.meals[2].on = false;
        let sunday: Date = "2026-10-11".parse().unwrap();
        let keys: Vec<String> = weekdays.kept_on(sunday, &zone(), &none).iter().filter(|k| k.kind == "meal").map(|k| k.key.clone()).collect();
        assert_eq!(keys, ["meal:1"]);
        // A bedtime after midnight belongs to the night before.
        let late = Needs { sleep: Sleep { bed: "01:00".into(), wake: "09:00".into(), wind_down: 30, notices: true }, ..needs.clone() };
        let nights: Vec<(String, String)> = late.kept_on(monday, &zone(), &none).iter().filter(|k| k.kind == "sleep").map(|k| (hm(k.start), hm(k.end))).collect();
        assert_eq!(nights, [("05 00:30".to_string(), "05 09:00".to_string())], "the next one starts on the 6th: Tuesday's");
        // Winding down from before midnight: the evening keeps it free.
        let midnight = Needs { sleep: Sleep { bed: "00:30".into(), wake: "08:00".into(), wind_down: 60, notices: true }, ..needs.clone() };
        let evening = midnight.kept_on(monday, &zone(), &none).into_iter().filter(|k| k.kind == "sleep").last().unwrap();
        assert_eq!((hm(evening.start), hm(evening.at)), ("05 23:30".to_string(), "06 00:30".to_string()));
    }

    #[test]
    fn two_notices_each_once_near_their_time() {
        let monday: Date = "2026-10-05".parse().unwrap();
        let needs = Needs { meals_on: true, ..Needs::default() };
        let kept = needs.kept_on(monday, &zone(), &none);
        let at = |text: &str| -> i64 { format!("2026-10-05T{text}[Europe/Paris]").parse::<jiff::Zoned>().unwrap().timestamp().as_second() };
        let mut today = Today::default();
        // Lunch is got ready from 12:10: the heads-up at 11:55, once.
        let given = |today: &mut Today, time: &str| -> Vec<(String, bool)> { today.due(&kept, 15, at(time)).into_iter().map(|(k, h)| (k.key.clone(), h)).collect() };
        assert!(given(&mut today, "11:54").is_empty());
        assert_eq!(given(&mut today, "11:56"), [("meal:1".to_string(), true)]);
        assert!(given(&mut today, "11:57").is_empty(), "once");
        assert_eq!(given(&mut today, "12:11"), [("meal:1".to_string(), false)]);
        assert!(given(&mut today, "12:12").is_empty());
        // Sioul opened late: no heads-up once the block began, the one at its time within a quarter of an hour, none after.
        let mut late = Today::default();
        assert_eq!(given(&mut late, "19:05"), [("meal:2".to_string(), false)]);
        assert!(given(&mut Today::default(), "19:20").is_empty(), "past its quarter of an hour: nothing, never 'missed'");
        // Not today: nothing at all.
        let mut skipped = Today { skipped: ["meal:1".to_string()].into(), ..Today::default() };
        assert!(given(&mut skipped, "11:56").is_empty() && given(&mut skipped, "12:11").is_empty());
    }

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    /// Each block as "key start–end", " quiet" when it gives no notice.
    fn rows(kept: &[Kept]) -> Vec<String> {
        kept.iter().map(|k| format!("{} {}–{}{}", k.key, hm(k.start), hm(k.end), if k.notices { "" } else { " quiet" })).collect()
    }

    #[test]
    fn a_day_changed_is_that_day_only() {
        let (wednesday, thursday) = (day("2026-10-07"), day("2026-10-08"));
        let needs = Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() };
        let mut days = Days::default();
        days.change(wednesday, "meal:1", |b| b.at = "13:30".into());
        days.change(wednesday, "nap:0", |b| b.off = true);
        days.change(wednesday, "meal:0", |b| b.quiet = true);
        days.change(wednesday, "sleep", |b| {
            b.at = "00:30".into();
            b.wake = "08:30".into();
        });
        let picnic = days.new_key(wednesday, 1_791_300_000_000);
        days.change(wednesday, &picnic, |b| {
            b.kind = "meal".into();
            b.name = "Picnic".into();
            b.at = "16:00".into();
            b.minutes = Some(45);
        });
        // Wednesday: breakfast without notices, lunch at 13:30 (getting it ready from 13:10), no
        // nap, the picnic, and a bedtime after midnight: its notices are Wednesday's.
        let picnic_row = format!("{picnic} 07 16:00–07 16:45");
        assert_eq!(
            rows(&needs.kept_with(wednesday, &zone(), &days, &none)),
            ["sleep 06 22:00–07 07:00 quiet", "meal:0 07 07:50–07 08:20 quiet", "meal:1 07 13:10–07 14:00", picnic_row.as_str(), "meal:2 07 19:00–07 20:00", "sleep 07 23:30–08 08:30"]
        );
        // Thursday as usual; Wednesday's night ends its morning, its notices said the day before.
        assert_eq!(
            rows(&needs.kept_with(thursday, &zone(), &days, &none)),
            ["sleep 07 23:30–08 08:30 quiet", "meal:0 08 07:50–08 08:20", "meal:1 08 12:10–08 13:00", "nap:0 08 14:00–08 14:35", "meal:2 08 19:00–08 20:00", "sleep 08 22:00–09 07:00"]
        );
        // Without the days' changes, Wednesday is a usual day.
        assert_eq!(rows(&needs.kept_on(wednesday, &zone(), &none))[2], "meal:1 07 12:10–07 13:00");
        // A day's own blocks, as the page lists them: not the night ending that morning.
        let own: Vec<String> = needs.blocks_of(wednesday, &zone(), &days, &none).iter().map(|k| k.key.clone()).collect();
        assert_eq!(own, ["meal:0", "meal:1", picnic.as_str(), "meal:2", "sleep"]);
        // The usual lunch changed afterwards (12:00, forty minutes): Wednesday keeps its own time
        // and takes the new length, which it did not change; Thursday takes both.
        let mut later = needs.clone();
        later.meals[1].at = "12:00".into();
        later.meals[1].minutes = 40;
        let lunch = |date: Date| later.kept_with(date, &zone(), &days, &none).into_iter().find(|k| k.key == "meal:1").map(|k| format!("{}–{}", hm(k.start), hm(k.end)));
        assert_eq!(lunch(wednesday).as_deref(), Some("07 13:10–07 14:10"));
        assert_eq!(lunch(thursday).as_deref(), Some("08 11:40–08 12:40"));
        // Breakfast taken out of the settings: lunch's change follows lunch, now the first meal.
        let mut rekeyed = days.clone();
        rekeyed.removed_usual("meal", 0);
        assert!(rekeyed.get(wednesday, "meal:0").is_some_and(|b| b.at == "13:30"));
        assert!(rekeyed.get(wednesday, "meal:1").is_none());
        assert!(rekeyed.get(wednesday, "nap:0").is_some_and(|b| b.off), "naps untouched");
        // A change taken back leaves nothing behind.
        days.change(wednesday, "meal:0", |b| b.quiet = false);
        days.forget(wednesday, "meal:1");
        assert!(days.get(wednesday, "meal:0").is_none() && days.get(wednesday, "meal:1").is_none());
    }

    #[test]
    fn a_day_changed_from_the_page() {
        let needs = Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() };
        let now: jiff::Zoned = "2026-10-06T10:00[Europe/Paris]".parse().unwrap();
        let wednesday = day("2026-10-07");
        let mut days = Days::default();
        let asked = |action: &str, key: &str| DayEdit { action: action.into(), key: key.into(), ..DayEdit::default() };
        let mut change = |date: Date, edit: DayEdit| needs.change_day(&mut days, date, &edit, &now, &[]);
        // A past day stays as it was.
        assert_eq!(change(day("2026-10-05"), asked("off", "meal:1")), Err(DayProblem::Past));
        // "15 min later", twice: eating from 13:00, getting it ready from 12:40.
        change(wednesday, asked("later", "meal:1")).unwrap();
        change(wednesday, asked("later", "meal:1")).unwrap();
        // Moved to 11:30, from getting it ready: eating from 11:50.
        change(wednesday, DayEdit { from: "11:30".into(), ..asked("move", "meal:1") }).unwrap();
        // A meal moved past midnight would leave its day.
        assert_eq!(change(wednesday, DayEdit { from: "23:50".into(), ..asked("move", "meal:2") }), Err(DayProblem::OtherDay));
        // The nap quiet that day, then back; then taken out; dinner's times: 18:00 to 19:00, ten minutes to get it ready.
        change(wednesday, asked("quiet", "nap:0")).unwrap();
        change(wednesday, asked("loud", "nap:0")).unwrap();
        change(wednesday, asked("off", "nap:0")).unwrap();
        change(wednesday, DayEdit { from: "18:00".into(), to: "19:00".into(), before: Some(10), ..asked("times", "meal:2") }).unwrap();
        // The night: winding down from 23:00, bed at 00:30, up at 08:30.
        change(wednesday, DayEdit { from: "23:00".into(), bed: "00:30".into(), to: "08:30".into(), ..asked("times", "sleep") }).unwrap();
        // One added that day: a rest from 16:00 to 16:30, a quarter of an hour to come back after it.
        change(wednesday, DayEdit { kind: "nap".into(), name: " Rest ".into(), from: "16:00".into(), to: "16:30".into(), after: Some(15), ..asked("add", "") }).unwrap();
        assert_eq!(change(wednesday, DayEdit { from: "16:00".into(), to: "16:03".into(), ..asked("add", "") }), Err(DayProblem::TooShort));
        assert_eq!(change(wednesday, DayEdit { from: "16:00".into(), to: "15:00".into(), ..asked("times", "meal:0") }), Err(DayProblem::Times));
        assert_eq!(change(wednesday, asked("forget", "meal:0")), Err(DayProblem::Unknown));
        let shown = rows(&needs.blocks_of(wednesday, &zone(), &days, &none));
        let rest = days.0[&wednesday].keys().find(|k| is_added(k)).unwrap().clone();
        let rest_row = format!("{rest} 07 16:00–07 16:30");
        assert_eq!(shown, ["meal:0 07 07:50–07 08:20", "meal:1 07 11:30–07 12:20", rest_row.as_str(), "meal:2 07 18:00–07 19:00", "sleep 07 23:00–08 08:30"]);
        // Kept as times, lengths only where they differ from the usual ones (dinner: 50 minutes, ten to get it ready).
        let dinner = days.get(wednesday, "meal:2").unwrap();
        assert_eq!((dinner.at.as_str(), dinner.minutes, dinner.before), ("18:10", Some(50), Some(10)));
        let night = days.get(wednesday, "sleep").unwrap();
        assert_eq!((night.at.as_str(), night.wake.as_str(), night.before), ("00:30", "08:30", Some(90)));
        assert!(days.get(wednesday, "nap:0").is_some_and(|b| b.off && !b.quiet));
        assert_eq!(days.get(wednesday, &rest).map(|b| (b.name.as_str(), b.minutes)), Some(("Rest", Some(15))));
        // Back to usual, put back, the added one removed: Wednesday as any day.
        let mut change = |edit: DayEdit| needs.change_day(&mut days, wednesday, &edit, &now, &[]);
        for (action, key) in [("usual", "meal:1"), ("usual", "meal:2"), ("usual", "sleep"), ("on", "nap:0"), ("off", rest.as_str())] {
            change(asked(action, key)).unwrap();
        }
        assert!(days.0.is_empty(), "{days:?}");
    }

    #[test]
    fn kept_around_now() {
        let needs = Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() };
        let now: jiff::Zoned = "2026-10-07T11:00[Europe/Paris]".parse().unwrap();
        let mut days = Days::default();
        days.change(now.date(), "nap:0", |b| b.off = true);
        days.change(now.date(), "meal:0", |b| b.quiet = true);
        let at = |text: &str| format!("2026-10-07T{text}").parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone()).unwrap().timestamp().as_second();
        let kept = rows(&needs.kept_around(&now, &days, &[(at("12:00"), at("13:30"))]));
        // The night ending this morning, today (breakfast quiet, lunch after the meeting, no nap),
        // tonight, tomorrow, tomorrow night; each block once.
        assert_eq!(
            kept,
            [
                "sleep 06 22:00–07 07:00 quiet",
                "meal:0 07 07:50–07 08:20 quiet",
                "meal:1 07 13:30–07 14:20",
                "meal:2 07 19:00–07 20:00",
                "sleep 07 22:00–08 07:00",
                "meal:0 08 07:50–08 08:20",
                "meal:1 08 12:10–08 13:00",
                "nap:0 08 14:00–08 14:35",
                "meal:2 08 19:00–08 20:00",
                "sleep 08 22:00–09 07:00",
            ]
        );
    }

    #[test]
    fn meals_pushed_past_events_unless_set_that_day() {
        let monday = day("2026-10-05");
        let needs = Needs { meals_on: true, ..Needs::default() };
        let at = |text: &str| format!("2026-10-05T{text}").parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone()).unwrap().timestamp().as_second();
        let meeting = [(at("12:00"), at("13:30"))];
        assert_eq!(needs.past_events_on(monday, &zone(), &Days::default(), &meeting).get("meal:1"), Some(&80));
        // Set for that day (eating from 11:50, into the meeting): chosen with the day in view, it stays.
        let mut set = Days::default();
        set.change(monday, "meal:1", |b| b.at = "11:50".into());
        assert!(needs.past_events_on(monday, &zone(), &set, &meeting).is_empty());
        // Quiet only: its time is the usual one, pushed as ever.
        let mut quiet = Days::default();
        quiet.change(monday, "meal:1", |b| b.quiet = true);
        assert_eq!(needs.past_events_on(monday, &zone(), &quiet, &meeting).get("meal:1"), Some(&80));
    }

    #[test]
    fn the_days_file_reads_as_written() {
        let dir = std::env::temp_dir().join(format!("sioul-needs-days-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health-days.toml");
        assert_eq!(Days::load(&path), Days::default(), "none yet");
        let mut days = Days::default();
        days.change(day("2026-10-07"), "meal:1", |b| b.at = "13:30".into());
        days.change(day("2026-10-08"), "sleep", |b| {
            b.at = "00:30".into();
            b.wake = "08:30".into();
        });
        days.change(day("2026-09-01"), "nap:0", |b| b.off = true);
        days.save(&path, day("2026-10-06")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# Meals, naps and nights"), "{text}");
        assert!(text.contains("\"meal:1\"]\nat = \"13:30\"\n"), "{text}");
        let read = Days::load(&path);
        assert_eq!(read.get(day("2026-10-08"), "sleep").map(|b| b.wake.as_str()), Some("08:30"));
        assert!(!read.0.contains_key(&day("2026-09-01")), "more than two weeks past: dropped");
        // Written by hand: a table per block.
        let by_hand: Days = toml::from_str("[2026-10-09.\"nap:0\"]\nquiet = true\n\n[2026-10-09.added-5]\nkind = \"nap\"\nname = \"Rest\"\nat = \"16:00\"\n").unwrap();
        assert!(by_hand.get(day("2026-10-09"), "nap:0").is_some_and(|b| b.quiet));
        assert_eq!(by_hand.get(day("2026-10-09"), "added-5").map(|b| b.name.as_str()), Some("Rest"));
        // Broken by hand: kept aside, nothing read.
        std::fs::write(&path, "[2026-10-07\n").unwrap();
        assert_eq!(Days::load(&path), Days::default());
        assert!(dir.join("health-days.toml.unreadable").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn long_gaps_said_where_meals_are_set() {
        let needs = Needs { meals_on: true, ..Needs::default() };
        // 08:20 to 12:30 is 4 h 10; 13:00 to 19:30 is 6 h 30.
        assert_eq!(needs.long_gaps(), [("08:20".to_string(), "12:30".to_string()), ("13:00".to_string(), "19:30".to_string())]);
        let mut snack = needs.clone();
        snack.meals.push(Block::new("16:00", 15, 5, 0));
        snack.meals[0].at = "08:30".into();
        assert!(snack.long_gaps().is_empty(), "a snack between, breakfast later: no gap over four hours");
        // Read as written by hand: a block's unsaid parts take their usual values.
        let read: Needs = toml::from_str("meals_on = true\n[[meals]]\nat = \"12:00\"\nminutes = 30\n").unwrap();
        assert!(read.meals[0].on && read.meals[0].notices && read.meals[0].days == [true; 7] && !read.sleep_on && read.heads_up == 15);
    }
}
