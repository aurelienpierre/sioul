// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A watch's own record of the body (docs/health.md, "Your watch"): Garmin's
//! FIT files, read here, from a folder you choose (Gadgetbridge's exports,
//! Garmin's export ZIPs) and from the watch's `GARMIN` folder when the desktop
//! shows it. No account, no server: nothing Garmin changes can break it, and
//! nothing leaves this computer.
//!
//! Kept, one small file a day (`$XDG_DATA_HOME/sioul/watch/<day>.json`):
//! heart rate, stress as Garmin scores it, Body Battery (an undocumented
//! field, kept only when plausible), steps, resting heart rate, sleep (its
//! levels, its score), the night's HRV. Read with `fitparser` (MIT), never
//! Garmin's SDK, whose licence forbids copyleft.
//!
//! What Sioul makes of it (research: `docs/research/wearables.md` §7):
//! offers, never alarms; at a task's end, never in its middle; no score, no
//! number in a notification, no "goal missed"; weekly before daily; sleep as
//! its length, not its stages. Nothing on quiet days or once the day is done.

use fitparser::Value;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Seconds between the Unix epoch and FIT's (1989-12-31 00:00 UTC).
const FIT_EPOCH: i64 = 631_065_600;

/// One day of the body, as the watch measured it; times in Unix seconds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Day {
    #[serde(default)]
    pub heart_rate: BTreeMap<i64, u8>,
    /// Stress as Garmin scores it, 0–100 (heart-rate variability while still).
    #[serde(default)]
    pub stress: BTreeMap<i64, u8>,
    /// Body Battery, 0–100.
    #[serde(default)]
    pub body_battery: BTreeMap<i64, u8>,
    /// Steps taken in the interval ending then.
    #[serde(default)]
    pub steps: BTreeMap<i64, u32>,
    #[serde(default)]
    pub resting_hr: Option<u8>,
    /// Sleep levels from then: 1 awake, 2 light, 3 deep, 4 REM.
    #[serde(default)]
    pub sleep: BTreeMap<i64, u8>,
    #[serde(default)]
    pub sleep_score: Option<u8>,
    /// The night's HRV average, in milliseconds.
    #[serde(default)]
    pub hrv_night: Option<f32>,
}

impl Day {
    fn merge(&mut self, other: Day) {
        self.heart_rate.extend(other.heart_rate);
        self.stress.extend(other.stress);
        self.body_battery.extend(other.body_battery);
        self.steps.extend(other.steps);
        self.sleep.extend(other.sleep);
        self.resting_hr = other.resting_hr.or(self.resting_hr);
        self.sleep_score = other.sleep_score.or(self.sleep_score);
        self.hrv_night = other.hrv_night.or(self.hrv_night);
    }

    pub fn steps_total(&self) -> u32 {
        self.steps.values().sum()
    }
}

/// What a file brought: its identity (serial and time made, to read it once
/// whatever way it came), and its samples by day.
pub struct Reading {
    pub id: String,
    pub days: BTreeMap<jiff::civil::Date, Day>,
}

fn seconds(value: &Value) -> Option<i64> {
    match value {
        Value::Timestamp(t) => Some(t.timestamp()),
        Value::UInt32(s) => Some(i64::from(*s) + FIT_EPOCH),
        _ => None,
    }
}

/// An enumerated field, by its number or by the name the parser gives it.
fn enumerated(value: &Value, names: &[(&str, u8)]) -> Option<u8> {
    match value {
        Value::String(name) => names.iter().find(|(n, _)| name.eq_ignore_ascii_case(n)).map(|(_, v)| *v),
        other => number(other).map(|n| n as u8),
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Byte(v) | Value::Enum(v) | Value::UInt8(v) | Value::UInt8z(v) => Some(f64::from(*v)),
        Value::SInt8(v) => Some(f64::from(*v)),
        Value::SInt16(v) => Some(f64::from(*v)),
        Value::UInt16(v) | Value::UInt16z(v) => Some(f64::from(*v)),
        Value::SInt32(v) => Some(f64::from(*v)),
        Value::UInt32(v) | Value::UInt32z(v) => Some(f64::from(*v)),
        Value::Float32(v) => Some(f64::from(*v)),
        Value::Float64(v) => Some(*v),
        _ => None,
    }
}

/// Reads one FIT file: monitoring (heart rate, steps), stress and Body
/// Battery, resting heart rate, sleep levels and score, the night's HRV.
pub fn read_fit(bytes: &[u8], zone: &jiff::tz::TimeZone) -> Result<Reading, String> {
    let records = fitparser::from_bytes(bytes).map_err(|e| e.to_string())?;
    let mut id = String::new();
    let mut days: BTreeMap<jiff::civil::Date, Day> = BTreeMap::new();
    let day_of = |at: i64| jiff::Timestamp::from_second(at).ok().map(|t| t.to_zoned(zone.clone()).date());
    // The last full timestamp, for the messages that give only its 16 low bits.
    let mut last: Option<i64> = None;
    // Steps are counted up per kind of activity: what each record adds.
    let mut counted: BTreeMap<u8, u32> = BTreeMap::new();
    for record in &records {
        let field = |name: &str| record.fields().iter().find(|f| f.name() == name).map(|f| f.value().clone());
        let full = field("timestamp").as_ref().and_then(seconds);
        if let Some(at) = full {
            last = Some(at);
        }
        let at = full.or_else(|| {
            let low = field("timestamp_16").as_ref().and_then(number)? as i64;
            let base = last? - FIT_EPOCH;
            Some(base + ((low - (base & 0xFFFF)) & 0xFFFF) + FIT_EPOCH)
        });
        match record.kind().as_u16() {
            // file_id: who made it, when.
            0 => {
                let serial = field("serial_number").as_ref().and_then(number).map(|n| n as u64).unwrap_or(0);
                let made = field("time_created").as_ref().and_then(seconds).unwrap_or(0);
                if serial != 0 || made != 0 {
                    id = format!("{serial}-{made}");
                }
            }
            // monitoring: heart rate, steps.
            55 => {
                let Some(at) = at else { continue };
                let Some(day) = day_of(at) else { continue };
                if let Some(hr) = field("heart_rate").as_ref().and_then(number).filter(|v| (25.0..=230.0).contains(v)) {
                    days.entry(day).or_default().heart_rate.insert(at, hr as u8);
                }
                let kind = field("activity_type").as_ref().and_then(|v| enumerated(v, &[("running", 1), ("walking", 6)]));
                let cycles = field("steps").or_else(|| field("cycles")).as_ref().and_then(number);
                if let (Some(kind), Some(cycles)) = (kind, cycles)
                    && matches!(kind, 1 | 6)
                {
                    let total = cycles as u32;
                    let before = counted.insert(kind, total).unwrap_or(0);
                    let added = if total >= before { total - before } else { total };
                    if added > 0 {
                        *days.entry(day).or_default().steps.entry(at).or_insert(0) += added;
                    }
                }
            }
            // monitoring_hr_data: the day's resting heart rate.
            211 => {
                let Some(day) = at.and_then(day_of) else { continue };
                if let Some(rest) = field("resting_heart_rate").or_else(|| field("current_day_resting_heart_rate")).as_ref().and_then(number).filter(|v| (25.0..=120.0).contains(v)) {
                    days.entry(day).or_default().resting_hr = Some(rest as u8);
                }
            }
            // stress_level: stress (negative: no measurement), and Body Battery (field 3).
            227 => {
                let Some(at) = field("stress_level_time").as_ref().and_then(seconds).or(at) else { continue };
                let Some(day) = day_of(at) else { continue };
                if let Some(stress) = field("stress_level_value").as_ref().and_then(number).filter(|v| (0.0..=100.0).contains(v)) {
                    days.entry(day).or_default().stress.insert(at, stress as u8);
                }
                let battery = record.fields().iter().find(|f| f.number() == 3).map(|f| f.value().clone());
                if let Some(level) = battery.as_ref().and_then(number).filter(|v| (0.0..=100.0).contains(v)) {
                    days.entry(day).or_default().body_battery.insert(at, level as u8);
                }
            }
            // sleep_level: the level from then on.
            275 => {
                let Some(at) = at else { continue };
                let Some(day) = day_of(at) else { continue };
                if let Some(level) = field("sleep_level").as_ref().and_then(|v| enumerated(v, &[("awake", 1), ("light", 2), ("deep", 3), ("rem", 4)])).filter(|v| (1..=4).contains(v)) {
                    days.entry(day).or_default().sleep.insert(at, level);
                }
            }
            // sleep_assessment: the night's score.
            346 => {
                let Some(day) = at.and_then(day_of) else { continue };
                if let Some(score) = field("overall_sleep_score").as_ref().and_then(number).filter(|v| (0.0..=100.0).contains(v)) {
                    days.entry(day).or_default().sleep_score = Some(score as u8);
                }
            }
            // hrv_status_summary: the night's HRV average.
            370 => {
                let Some(day) = at.and_then(day_of) else { continue };
                if let Some(ms) = field("last_night_average").as_ref().and_then(number).filter(|v| *v > 5.0 && *v < 300.0) {
                    days.entry(day).or_default().hrv_night = Some(ms as f32);
                }
            }
            _ => {}
        }
    }
    if id.is_empty() {
        id = format!("sha-{}", crate::vdir::content_hash(bytes));
    }
    Ok(Reading { id, days })
}

/// Where the days are kept.
pub fn folder() -> PathBuf {
    crate::config::data_dir().join("watch")
}

/// What was imported already, by file id.
#[derive(Default, Serialize, Deserialize)]
struct Imported {
    #[serde(default)]
    files: BTreeSet<String>,
}

fn imported_path(folder: &Path) -> PathBuf {
    folder.join("imported.json")
}

pub fn load_day(folder: &Path, date: jiff::civil::Date) -> Day {
    std::fs::read_to_string(folder.join(format!("{date}.json"))).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_day(folder: &Path, date: jiff::civil::Date, day: &Day) -> Result<(), String> {
    let path = folder.join(format!("{date}.json"));
    let temporary = path.with_extension("json.new");
    std::fs::write(&temporary, serde_json::to_string(day).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", temporary.display()))?;
    crate::health::keep_private(&temporary);
    std::fs::rename(&temporary, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Brings in the FIT files under `source` (a folder, or a ZIP, nested ZIPs
/// too), each once; the watch's workouts (`Activity`) are left: their tracks
/// are not needed here. Returns how many files were new.
pub fn import(source: &Path, folder: &Path, zone: &jiff::tz::TimeZone) -> Result<usize, String> {
    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let mut imported: Imported = std::fs::read_to_string(imported_path(folder)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut files: Vec<Vec<u8>> = Vec::new();
    collect(source, &mut files, 0);
    let mut new = 0;
    let mut changed: BTreeMap<jiff::civil::Date, Day> = BTreeMap::new();
    for bytes in files {
        let Ok(reading) = read_fit(&bytes, zone) else { continue };
        if !imported.files.insert(reading.id) {
            continue;
        }
        new += 1;
        for (date, day) in reading.days {
            changed.entry(date).or_insert_with(|| load_day(folder, date)).merge(day);
        }
    }
    for (date, day) in &changed {
        save_day(folder, *date, day)?;
    }
    if new > 0 {
        std::fs::write(imported_path(folder), serde_json::to_string(&imported).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    Ok(new)
}

fn collect(path: &Path, out: &mut Vec<Vec<u8>>, depth: usize) {
    if depth > 6 {
        return;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if path.is_dir() {
        if name == "activity" || name.starts_with('.') {
            return;
        }
        for entry in std::fs::read_dir(path).into_iter().flatten().filter_map(Result::ok) {
            collect(&entry.path(), out, depth + 1);
        }
    } else if name.ends_with(".fit") {
        if let Ok(bytes) = std::fs::read(path) {
            out.push(bytes);
        }
    } else if name.ends_with(".zip")
        && let Ok(file) = std::fs::File::open(path)
    {
        unzip(file, out, depth);
    }
}

fn unzip<R: std::io::Read + std::io::Seek>(reader: R, out: &mut Vec<Vec<u8>>, depth: usize) {
    let Ok(mut archive) = zip::ZipArchive::new(reader) else { return };
    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else { continue };
        let name = entry.name().to_lowercase();
        if name.contains("activity") && !name.contains("wellness") {
            continue;
        }
        let mut bytes = Vec::new();
        if (name.ends_with(".fit") || name.ends_with(".zip")) && entry.read_to_end(&mut bytes).is_ok() {
            if name.ends_with(".zip") && depth < 4 {
                unzip(std::io::Cursor::new(bytes), out, depth + 1);
            } else {
                out.push(bytes);
            }
        }
    }
}

/// Where a watch shows itself when plugged in: a drive (older watches), or
/// the desktop's MTP mount (GNOME's gvfs), each holding a `GARMIN` folder.
pub fn mounted_watches() -> Vec<PathBuf> {
    let user = std::env::var("USER").unwrap_or_default();
    let mut roots: Vec<PathBuf> = vec![PathBuf::from(format!("/run/media/{user}")), PathBuf::from(format!("/media/{user}")), PathBuf::from("/Volumes")];
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        roots.push(Path::new(&dir).join("gvfs"));
        // KDE with kio-fuse: <runtime>/kio-fuse-XXXX/mtp/<device>/<storage>/GARMIN.
        for fuse in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().starts_with("kio-fuse")) {
            roots.push(fuse.path().join("mtp"));
        }
    }
    let mut found = Vec::new();
    for root in roots {
        for volume in std::fs::read_dir(&root).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()) {
            // A drive: <volume>/GARMIN; MTP: <device>/<storage>/GARMIN.
            for candidate in std::iter::once(volume.join("GARMIN")).chain(std::fs::read_dir(&volume).into_iter().flatten().filter_map(Result::ok).map(|e| e.path().join("GARMIN"))) {
                if candidate.is_dir() {
                    found.push(candidate);
                }
            }
        }
    }
    found
}

// What it says.

/// The body's day as the Health page shows it: times in words are the window's.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Summary {
    /// The newest sample, Unix seconds; 0: nothing yet.
    pub newest: i64,
    pub steps_today: u32,
    pub resting_hr: Option<u8>,
    /// The median of the last 14 mornings, without today.
    pub resting_hr_usual: Option<u8>,
    /// Last night: asleep (minutes), from, to.
    pub slept_minutes: u32,
    pub slept_from: i64,
    pub slept_to: i64,
    pub body_battery: Option<u8>,
    /// Today's curves: (time, value), every quarter hour at most.
    pub heart_rate_curve: Vec<(i64, u8)>,
    pub stress_curve: Vec<(i64, u8)>,
    pub battery_curve: Vec<(i64, u8)>,
    /// The week's averages: resting heart rate, sleep (minutes), steps a day.
    pub week_resting_hr: Option<u8>,
    pub week_sleep_minutes: u32,
    pub week_steps: u32,
}

fn thin(series: &BTreeMap<i64, u8>, every: i64) -> Vec<(i64, u8)> {
    let mut out: Vec<(i64, u8)> = Vec::new();
    for (&at, &value) in series {
        if out.last().is_none_or(|(last, _)| at - last >= every) {
            out.push((at, value));
        }
    }
    out
}

/// A night: the sleep levels from the evening before (18:00) to the day's noon.
fn night(folder: &Path, date: jiff::civil::Date, zone: &jiff::tz::TimeZone) -> (u32, i64, i64) {
    let evening = date.yesterday().ok().and_then(|d| d.at(18, 0, 0, 0).to_zoned(zone.clone()).ok()).map_or(0, |z| z.timestamp().as_second());
    let noon = date.at(12, 0, 0, 0).to_zoned(zone.clone()).map_or(i64::MAX, |z| z.timestamp().as_second());
    let mut levels: BTreeMap<i64, u8> = BTreeMap::new();
    for day in [date.yesterday().unwrap_or(date), date] {
        levels.extend(load_day(folder, day).sleep.into_iter().filter(|(at, _)| *at >= evening && *at <= noon));
    }
    let points: Vec<(i64, u8)> = levels.into_iter().collect();
    let mut asleep = 0i64;
    for pair in points.windows(2) {
        if pair[0].1 != 1 {
            asleep += pair[1].0 - pair[0].0;
        }
    }
    let first = points.iter().find(|(_, l)| *l != 1).map_or(0, |(at, _)| *at);
    let last = points.last().map_or(0, |(at, _)| *at);
    ((asleep / 60) as u32, first, last)
}

fn median(mut values: Vec<u8>) -> Option<u8> {
    values.sort_unstable();
    values.get(values.len() / 2).copied()
}

pub fn summary(folder: &Path, today: jiff::civil::Date, zone: &jiff::tz::TimeZone) -> Summary {
    let day = load_day(folder, today);
    let past: Vec<(jiff::civil::Date, Day)> = (1..=14).filter_map(|n| today.checked_sub(jiff::Span::new().days(n)).ok()).map(|d| (d, load_day(folder, d))).collect();
    let newest = std::iter::once(&day)
        .chain(past.iter().map(|(_, d)| d))
        .flat_map(|d| [d.heart_rate.keys().next_back(), d.stress.keys().next_back(), d.steps.keys().next_back(), d.body_battery.keys().next_back()])
        .flatten()
        .max()
        .copied()
        .unwrap_or(0);
    let (slept_minutes, slept_from, slept_to) = night(folder, today, zone);
    let week: Vec<&Day> = past.iter().take(7).map(|(_, d)| d).collect();
    let week_nights: Vec<u32> = past.iter().take(7).map(|(d, _)| night(folder, *d, zone).0).filter(|m| *m > 0).collect();
    let week_steps: Vec<u32> = week.iter().map(|d| d.steps_total()).filter(|s| *s > 0).collect();
    Summary {
        newest,
        steps_today: day.steps_total(),
        resting_hr: day.resting_hr,
        resting_hr_usual: median(past.iter().filter_map(|(_, d)| d.resting_hr).collect()),
        slept_minutes,
        slept_from,
        slept_to,
        body_battery: day.body_battery.values().next_back().copied(),
        heart_rate_curve: thin(&day.heart_rate, 900),
        stress_curve: thin(&day.stress, 900),
        battery_curve: thin(&day.body_battery, 900),
        week_resting_hr: median(week.iter().filter_map(|d| d.resting_hr).collect()),
        week_sleep_minutes: if week_nights.is_empty() { 0 } else { week_nights.iter().sum::<u32>() / week_nights.len() as u32 },
        week_steps: if week_steps.is_empty() { 0 } else { week_steps.iter().sum::<u32>() / week_steps.len() as u32 },
    }
}

/// A gentle offer, by the rule that makes it (research §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Offer {
    /// R1: sat long; five minutes on your feet before the next task.
    Move,
    /// R2: long arousal while still, during focus: a pause.
    Pause,
    /// R5: low reserve late in the day: done for the day?
    LowReserve,
    /// R6: room for a walk outside.
    Walk,
}

/// The morning's word (R3, R4): never a notification, a line in the day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Morning {
    /// A short night, or two of the last three.
    ShortNight,
    /// The body flags strain: resting heart rate well above usual.
    Strain,
}

pub fn morning(folder: &Path, today: jiff::civil::Date, zone: &jiff::tz::TimeZone) -> Option<Morning> {
    let summary = summary(folder, today, zone);
    if let (Some(rest), Some(usual)) = (summary.resting_hr, summary.resting_hr_usual) {
        let yesterday = today.yesterday().ok().map(|d| load_day(folder, d).resting_hr);
        if rest >= usual.saturating_add(7) || (rest >= usual.saturating_add(5) && yesterday.flatten().is_some_and(|y| y >= usual.saturating_add(5))) {
            return Some(Morning::Strain);
        }
    }
    let nights: Vec<u32> = (0..3).filter_map(|n| today.checked_sub(jiff::Span::new().days(n)).ok()).map(|d| night(folder, d, zone).0).collect();
    let last = nights.first().copied().unwrap_or(0);
    let short = (last > 0 && last < 360) || nights.iter().filter(|m| **m > 0 && **m < 390).count() >= 2;
    short.then_some(Morning::ShortNight)
}

/// What the rules know of the moment.
pub struct Moment {
    pub now: i64,
    /// Local hour, 0–23.
    pub hour: u8,
    /// Quiet time, or the day closed: nothing is offered.
    pub quiet: bool,
    /// A task was just finished or a focus session ended: a breakpoint.
    pub breakpoint: bool,
    /// How long you sat in the focus session that just ended, in minutes.
    pub focus_minutes: u32,
}

/// What the rules remember: when they last offered, how often today, what was declined.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub last_offer: i64,
    #[serde(default)]
    pub day: String,
    #[serde(default)]
    pub today: u32,
    /// Until when each rule rests, after being declined.
    #[serde(default)]
    pub resting: BTreeMap<Offer, i64>,
    /// Declines in a row, by rule.
    #[serde(default)]
    pub declined: BTreeMap<Offer, u32>,
}

/// The one offer for this moment, if any: at a breakpoint only; never in
/// quiet time; 45 minutes apart at least; six a day at most; a declined rule
/// rests (longer each time; three times in a row, three days). Watch data
/// older than an hour counts as none.
pub fn offer(folder: &Path, moment: &Moment, memory: &Memory, zone: &jiff::tz::TimeZone, date: jiff::civil::Date) -> Option<Offer> {
    if moment.quiet || !moment.breakpoint || moment.now - memory.last_offer < 45 * 60 {
        return None;
    }
    if memory.day == date.to_string() && memory.today >= 6 {
        return None;
    }
    let ready = |offer: Offer| memory.resting.get(&offer).is_none_or(|until| *until <= moment.now);
    let day = load_day(folder, date);
    let fresh = |series: &BTreeMap<i64, u8>| series.keys().next_back().is_some_and(|at| moment.now - at < 3600);
    let steps_since = |since: i64| day.steps.range(since..).map(|(_, s)| *s).sum::<u32>();
    let watched = !day.steps.is_empty() && day.steps.keys().next_back().is_some_and(|at| moment.now - at < 3600);
    // R5: low reserve after 15:00.
    if ready(Offer::LowReserve) && moment.hour >= 15 && fresh(&day.body_battery) && day.body_battery.values().next_back().is_some_and(|b| *b <= 25) {
        return Some(Offer::LowReserve);
    }
    // R2: aroused and still for most of the last half hour.
    if ready(Offer::Pause) && fresh(&day.stress) {
        let high = day.stress.range(moment.now - 1800..).filter(|(_, s)| **s >= 51).count();
        let all = day.stress.range(moment.now - 1800..).count().max(1);
        if high * 3 >= all * 2 && steps_since(moment.now - 1800) < 50 {
            return Some(Offer::Pause);
        }
    }
    // R1: sat 45 minutes or more: by the watch's steps, else by the session's length.
    let sat = if watched { (0..3).all(|k| day.steps.range(moment.now - (k + 1) * 900..moment.now - k * 900).map(|(_, s)| *s).sum::<u32>() < 100) } else { moment.focus_minutes >= 45 };
    if ready(Offer::Move) && sat {
        return Some(Offer::Move);
    }
    // R6: room for a walk, midday to 16:00.
    let battery = day.body_battery.values().next_back().copied();
    if ready(Offer::Walk) && (12..16).contains(&moment.hour) && fresh(&day.body_battery) && battery.is_some_and(|b| b >= 50) && day.steps_total() < 3000 && morning(folder, date, zone) != Some(Morning::Strain) {
        return Some(Offer::Walk);
    }
    None
}

/// An offer made: remembered for the limits.
pub fn offered(memory: &mut Memory, offer: Offer, now: i64, date: jiff::civil::Date) {
    if memory.day != date.to_string() {
        memory.day = date.to_string();
        memory.today = 0;
    }
    memory.today += 1;
    memory.last_offer = now;
    let _ = offer;
}

/// An offer declined ("Not now"): that rule rests, longer each time, three
/// days after the third decline in a row. Never counted against you.
pub fn declined(memory: &mut Memory, offer: Offer, now: i64) {
    let count = memory.declined.entry(offer).or_insert(0);
    *count += 1;
    let rest = if *count >= 3 { 3 * 86_400 } else { 45 * 60 * (1 << *count) };
    memory.resting.insert(offer, now + rest);
}

/// An offer taken: its rule's count of declines starts again.
pub fn accepted(memory: &mut Memory, offer: Offer) {
    memory.declined.remove(&offer);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A FIT file, written by hand: a header, then for each message its
    /// definition and its data, then the CRC (FIT protocol 2.0).
    fn fit(messages: &[(u16, Vec<(u8, u8, Vec<u8>)>)]) -> Vec<u8> {
        fn crc(bytes: &[u8]) -> u16 {
            const TABLE: [u16; 16] = [0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800, 0xB401, 0x5000, 0x9C01, 0x8801, 0x4400];
            let mut crc = 0u16;
            for &byte in bytes {
                for nibble in [byte & 0xF, byte >> 4] {
                    let tmp = TABLE[(crc & 0xF) as usize];
                    crc = (crc >> 4) & 0x0FFF;
                    crc = crc ^ tmp ^ TABLE[nibble as usize];
                }
            }
            crc
        }
        let mut data = Vec::new();
        for (local, (global, fields)) in messages.iter().enumerate() {
            let local = local as u8 & 0x0F;
            data.extend([0x40 | local, 0, 0]);
            data.extend(global.to_le_bytes());
            data.push(fields.len() as u8);
            for (number, base, value) in fields {
                data.extend([*number, value.len() as u8, *base]);
            }
            data.push(local);
            for (_, _, value) in fields {
                data.extend(value);
            }
        }
        let mut header = vec![14u8, 0x20];
        header.extend(2132u16.to_le_bytes());
        header.extend((data.len() as u32).to_le_bytes());
        header.extend(b".FIT");
        let header_crc = crc(&header);
        header.extend(header_crc.to_le_bytes());
        let mut out = header;
        out.extend(&data);
        let file_crc = crc(&out);
        out.extend(file_crc.to_le_bytes());
        out
    }

    #[test]
    fn a_watch_file_read() {
        let zone = jiff::tz::TimeZone::UTC;
        // 2026-10-03 10:00 UTC, in FIT seconds.
        let at = |minutes: i64| (1_791_021_600 - FIT_EPOCH + minutes * 60) as u32;
        let u32le = |v: u32| v.to_le_bytes().to_vec();
        let messages = vec![
            // file_id: serial 1234, made at the start.
            (0u16, vec![(3u8, 0x8C, u32le(1234)), (4, 0x86, u32le(at(0)))]),
            // monitoring: timestamp, heart rate 72, walking (6) with 500 steps counted so far.
            (55, vec![(253, 0x86, u32le(at(0))), (27, 0x02, vec![72]), (5, 0x00, vec![6]), (3, 0x86, u32le(500))]),
            // then 620 steps counted: 120 more.
            (55, vec![(253, 0x86, u32le(at(15))), (5, 0x00, vec![6]), (3, 0x86, u32le(620))]),
            // stress 40, Body Battery (field 3) 63; then "no measurement" (-1).
            (227, vec![(0, 0x83, 40i16.to_le_bytes().to_vec()), (1, 0x86, u32le(at(3))), (3, 0x02, vec![63])]),
            (227, vec![(0, 0x83, (-1i16).to_le_bytes().to_vec()), (1, 0x86, u32le(at(6)))]),
            // resting heart rate 58.
            (211, vec![(253, 0x86, u32le(at(1))), (0, 0x02, vec![58])]),
        ];
        let reading = read_fit(&fit(&messages), &zone).unwrap();
        assert_eq!(reading.id, format!("1234-{}", i64::from(at(0)) + FIT_EPOCH));
        let day = &reading.days[&"2026-10-03".parse().unwrap()];
        assert_eq!(day.heart_rate.values().copied().collect::<Vec<_>>(), vec![72]);
        assert_eq!(day.steps_total(), 620, "500 at first, then 120 more");
        assert_eq!(day.stress.values().copied().collect::<Vec<_>>(), vec![40], "no measurement is no value");
        assert_eq!(day.body_battery.values().copied().collect::<Vec<_>>(), vec![63]);
        assert_eq!(day.resting_hr, Some(58));
    }

    #[test]
    fn imported_once_and_offered_gently() {
        let zone = jiff::tz::TimeZone::UTC;
        let root = std::env::temp_dir().join(format!("sioul-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (inbox, folder) = (root.join("inbox"), root.join("watch"));
        std::fs::create_dir_all(inbox.join("Activity")).unwrap();
        let at = |minutes: i64| (1_791_021_600 - FIT_EPOCH + minutes * 60) as u32;
        let u32le = |v: u32| v.to_le_bytes().to_vec();
        // Body Battery low at 15:30, freshly measured.
        let low = vec![(0u16, vec![(3u8, 0x8C, u32le(1)), (4, 0x86, u32le(at(330)))]), (227, vec![(0, 0x83, 20i16.to_le_bytes().to_vec()), (1, 0x86, u32le(at(330))), (3, 0x02, vec![22])])];
        std::fs::write(inbox.join("a.fit"), fit(&low)).unwrap();
        std::fs::write(inbox.join("Activity/run.fit"), fit(&low)).unwrap();
        assert_eq!(import(&inbox, &folder, &zone).unwrap(), 1, "workouts left aside");
        assert_eq!(import(&inbox, &folder, &zone).unwrap(), 0, "each file once");
        let date: jiff::civil::Date = "2026-10-03".parse().unwrap();
        let now = 1_791_021_600 + 335 * 60;
        let moment = |quiet: bool, breakpoint: bool| Moment { now, hour: 15, quiet, breakpoint, focus_minutes: 30 };
        let mut memory = Memory::default();
        assert_eq!(offer(&folder, &moment(false, true), &memory, &zone, date), Some(Offer::LowReserve));
        assert_eq!(offer(&folder, &moment(true, true), &memory, &zone, date), None, "nothing in quiet time");
        assert_eq!(offer(&folder, &moment(false, false), &memory, &zone, date), None, "only at a breakpoint");
        offered(&mut memory, Offer::LowReserve, now, date);
        assert_eq!(offer(&folder, &moment(false, true), &memory, &zone, date), None, "45 minutes apart");
        declined(&mut memory, Offer::LowReserve, now);
        let later = Moment { now: now + 50 * 60, ..moment(false, true) };
        assert_eq!(offer(&folder, &later, &memory, &zone, date), None, "a declined rule rests");
        let _ = std::fs::remove_dir_all(&root);
    }
}
