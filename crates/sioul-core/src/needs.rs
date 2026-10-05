// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The day's needs, set first: meals, naps and night sleep (docs/health.md,
//! "Meals, rest and sleep"). Their times are kept free: the plan lays no task
//! in them (`plan::Settings::with_needs`), the day shows meals and naps, and
//! two notices come for each, at most: a heads-up about the work a quarter of
//! an hour before ("No new big task"), then one at the time. Nothing about
//! eating or sleeping is recorded, and nothing is said when one passes.
//!
//! From the literature on eating disorders and irregular eating (notes in
//! docs/research/meal-prompts.md): by the clock, at times the person sets,
//! never by hunger (CBT-E, Murphy et al. 2010; NICE NG69); every part set by
//! the person, per block (Lindgreen, Lomborg & Clausen 2018); neutral words,
//! no praise, no numbers, no "missed" (Eikey 2021; Levinson et al. 2017); the
//! heads-up about the work, so it can be left on time (Leroy & Glomb 2018);
//! naps with time to wake up after (Hilditch et al. 2017); sleep with a
//! wind-down before it (Harvey et al. 2021).

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

/// Today's changes to the blocks, on this device only, never shared
/// (`$XDG_STATE_HOME/sioul/needs-today.toml`): those moved ("Later"), those
/// skipped, the notices given. Nothing about eating or sleeping itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Today {
    #[serde(default, deserialize_with = "crate::budget::dates::optional", skip_serializing_if = "Option::is_none")]
    pub day: Option<Date>,
    /// Blocks moved, by key, in minutes.
    #[serde(default)]
    pub shifts: BTreeMap<String, i64>,
    /// Blocks skipped today: no notice, kept free all the same.
    #[serde(default)]
    pub skipped: BTreeSet<String>,
    /// Notices given: "<key>:heads-up", "<key>:start".
    #[serde(default)]
    pub sent: BTreeSet<String>,
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
    /// What is kept free on `date`, in order: each meal and nap on that
    /// weekday, and the nights touching it (the one ending that morning, the
    /// one starting that evening). `shift` moves a block that day only, in
    /// minutes, by its key ("Later").
    pub fn kept_on(&self, date: Date, zone: &TimeZone, shift: &dyn Fn(&str) -> i64) -> Vec<Kept> {
        let mut out = Vec::new();
        let weekday = date.weekday().to_monday_zero_offset() as usize;
        let mut blocks = |kind: &'static str, on: bool, list: &[Block]| {
            if !on {
                return;
            }
            for (index, block) in list.iter().enumerate().filter(|(_, b)| b.on && b.days[weekday]) {
                let Some(time) = time_of(&block.at) else { continue };
                let key = format!("{kind}:{index}");
                let Some(start_at) = at(date, time, zone) else { continue };
                let start_at = start_at + shift(&key) * 60;
                out.push(Kept {
                    start: start_at - i64::from(block.before) * 60,
                    at: start_at,
                    end: start_at + i64::from(block.minutes + block.after) * 60,
                    key,
                    kind,
                    index,
                    name: block.name.clone(),
                    notices: block.notices,
                });
            }
        };
        blocks("meal", self.meals_on, &self.meals);
        blocks("nap", self.naps_on, &self.naps);
        if self.sleep_on
            && let (Some(bed), Some(wake)) = (time_of(&self.sleep.bed), time_of(&self.sleep.wake))
        {
            // The night of a day starts that evening; a bedtime after midnight is the next morning's.
            for night in [date.yesterday().ok(), Some(date)].into_iter().flatten() {
                let bed_day = if bed.hour() < 12 { night.tomorrow().unwrap_or(night) } else { night };
                let (Some(bed_at), Some(mut wake_at)) = (at(bed_day, bed, zone), at(bed_day, wake, zone)) else { continue };
                if wake_at <= bed_at {
                    wake_at = at(bed_day.tomorrow().unwrap_or(bed_day), wake, zone).unwrap_or(wake_at + 86_400);
                }
                let shifted = if night == date { shift("sleep") * 60 } else { 0 };
                let (bed_at, wake_at) = (bed_at + shifted, wake_at + shifted);
                let midnight = at(date, Time::midnight(), zone).unwrap_or(bed_at);
                let next = date.tomorrow().ok().and_then(|d| at(d, Time::midnight(), zone)).unwrap_or(midnight + 86_400);
                let start = bed_at - i64::from(self.sleep.wind_down) * 60;
                if wake_at > midnight && start < next {
                    out.push(Kept { key: "sleep".into(), kind: "sleep", index: 0, name: String::new(), start, at: bed_at, end: wake_at, notices: self.sleep.notices && night == date });
                }
            }
        }
        out.sort_by_key(|k| k.start);
        out
    }

    /// The spans kept free on `date`, for the plan: no task in them.
    pub fn busy_on(&self, date: Date, zone: &TimeZone, shift: &dyn Fn(&str) -> i64) -> Vec<(i64, i64)> {
        self.kept_on(date, zone, shift).into_iter().map(|k| (k.start, k.end)).collect()
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
