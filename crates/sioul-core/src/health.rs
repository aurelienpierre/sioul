// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Health and well-being: prescriptions, when to fetch their medicines and
//! when to renew them; medicines and when to take them; a pause to move during
//! long focus; a daily limit on chats. Sent nowhere, except sealed to your
//! other devices when you share with them (docs/database.md).
//!
//! Nothing here counts what was missed. A dose not marked taken is simply not
//! marked; the next one comes as planned. Two kinds of medicines: those taken
//! at set times of the day keep their times (being ready for something);
//! those taken every few hours keep the hours between two doses, which the
//! body (the liver, the kidneys) needs to clear one before the next: each dose
//! taken, early or late, sets the next one that many hours after it.

use jiff::civil::{Date, Time};
use jiff::{Span, Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Everything the health page keeps: `$XDG_DATA_HOME/sioul/health.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Health {
    #[serde(rename = "prescription", default)]
    pub prescriptions: Vec<Prescription>,
    #[serde(rename = "medicine", default)]
    pub medicines: Vec<Medicine>,
    #[serde(default)]
    pub movement: Movement,
    #[serde(default)]
    pub chats: ChatLimit,
    /// Where the pharmacy and the renewals go as tasks: a list on your server
    /// ("account/id"), so your phone has them; "" for your usual list.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub errands_list: String,
    // `watch_folder` and `watch_offers`, which Sioul wrote until 8 October 2026
    // (a Garmin watch's files, taken out: docs/health.md), are read and left
    // aside, as any key it does not know.
    /// Meals, naps and the night: times kept free, set first (`needs`).
    #[serde(default)]
    pub needs: crate::needs::Needs,
}

/// A prescription: what it is for, who wrote it, until when it is valid,
/// and how often the pharmacy gives its medicines.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Prescription {
    pub id: String,
    /// "Vitamin D 1000 IU".
    pub title: String,
    #[serde(default)]
    pub prescriber: String,
    /// Its last valid day: renewed before it.
    #[serde(default, deserialize_with = "crate::budget::dates::optional", skip_serializing_if = "Option::is_none")]
    pub until: Option<Date>,
    /// The pharmacy gives this many days at a time (28, 30, 90).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refill_days: Option<u32>,
    /// The last time it was fetched at the pharmacy.
    #[serde(default, deserialize_with = "crate::budget::dates::optional", skip_serializing_if = "Option::is_none")]
    pub last_refill: Option<Date>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A medicine to take, and when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Medicine {
    pub id: String,
    /// Its brand name, or your own word for it: what reminders say. Never
    /// empty once saved (its generic name stands in), as Sioul 0.0.3 reads it.
    pub name: String,
    /// "1000 IU", "1 tablet".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dose: String,
    pub schedule: Schedule,
    /// The prescription it comes with, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prescription: Option<String>,
    /// The last day it is taken; none for as long as it goes.
    #[serde(default, deserialize_with = "crate::budget::dates::optional", skip_serializing_if = "Option::is_none")]
    pub until: Option<Date>,
    /// Stopped for now: no reminder.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
    /// The generic name of its molecule, as typed (the INN, "levothyroxine"):
    /// what a doctor or a pharmacist reads, whatever the brand. Written only
    /// when given; Sioul 0.0.3 drops it at its next save.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub generic: String,
    /// Its strength ("75 µg", "500 mg per tablet"), apart from a take's
    /// amount ("1 tablet"). Written only when given, as `generic`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub strength: String,
    /// The day it was first taken, when known: how long it has been taken.
    /// Written only when given, as `generic`.
    #[serde(default, deserialize_with = "crate::budget::dates::optional", skip_serializing_if = "Option::is_none")]
    pub since: Option<Date>,
}

/// When a medicine is taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "every", rename_all = "kebab-case")]
pub enum Schedule {
    /// At these times each day: "12:00", "18:00", each a take.
    Day {
        times: Vec<String>,
        /// Each take's own amount, by its time ("20:00" = "2 tablets"), only
        /// when the takes' amounts differ: then it holds every take's, and the
        /// medicine's `dose` holds them all in one line for an older Sioul,
        /// which reads the times alone (`Medicine::set_takes`). Empty: every
        /// take is the medicine's `dose`, and the file reads as before.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        amounts: BTreeMap<String, String>,
    },
    /// Every `days` days at `time`, counted from `from` ("every other day from tomorrow").
    Days {
        days: u32,
        time: String,
        #[serde(deserialize_with = "crate::budget::dates::required")]
        from: Date,
    },
    /// Every `hours` hours from `from`, Unix seconds ("every 6 hours from now"):
    /// each dose taken sets the next one that many hours after it (`Health::taken_at`).
    Hours { hours: u32, from: i64 },
}

/// "18:00" → 18:00.
fn time_of(text: &str) -> Option<Time> {
    let (hour, minute) = text.trim().split_once(':')?;
    Time::new(hour.trim().parse().ok()?, minute.trim().parse().ok()?, 0, 0).ok()
}

/// "8:00" → "08:00": a take's time as `amounts` keys it; none when it does not read.
pub fn hhmm(text: &str) -> Option<String> {
    time_of(text).map(|t| format!("{:02}:{:02}", t.hour(), t.minute()))
}

impl Schedule {
    /// The doses from `start` (included) to `end` (excluded), in order.
    pub fn doses(&self, start: &Zoned, end: &Zoned) -> Vec<Zoned> {
        self.timed(start, end).into_iter().map(|(at, _)| at).collect()
    }

    /// The same, each with the time of day it is set for, at set times each
    /// day (its take: a change of hour can move the dose itself, 02:30 to
    /// 03:30); none for the others.
    fn timed(&self, start: &Zoned, end: &Zoned) -> Vec<(Zoned, Option<Time>)> {
        let zone = start.time_zone().clone();
        let mut out = Vec::new();
        match self {
            Schedule::Day { times, .. } => {
                let mut times: Vec<Time> = times.iter().filter_map(|t| time_of(t)).collect();
                times.sort();
                let mut day = start.date();
                while day <= end.date() {
                    for time in &times {
                        if let Ok(at) = day.to_datetime(*time).to_zoned(zone.clone())
                            && &at >= start
                            && &at < end
                        {
                            out.push((at, Some(*time)));
                        }
                    }
                    let Ok(next) = day.tomorrow() else { break };
                    day = next;
                }
            }
            Schedule::Days { days, time, from } => {
                let (Some(time), true) = (time_of(time), *days > 0) else { return out };
                let mut day = *from;
                // The first turn on or after the start.
                if day < start.date()
                    && let Ok(behind) = start.date().since(day)
                {
                    let turns = i64::from(behind.get_days()) / i64::from(*days);
                    day = day.checked_add(Span::new().days(turns * i64::from(*days))).unwrap_or(day);
                }
                while day <= end.date() {
                    if let Ok(at) = day.to_datetime(time).to_zoned(zone.clone())
                        && &at >= start
                        && &at < end
                    {
                        out.push((at, None));
                    }
                    let Ok(next) = day.checked_add(Span::new().days(i64::from(*days))) else { break };
                    day = next;
                }
            }
            Schedule::Hours { hours, from } => {
                if *hours == 0 {
                    return out;
                }
                let step = i64::from(*hours) * 3600;
                let first = start.timestamp().as_second();
                let mut at = *from;
                if at < first {
                    at += (first - at + step - 1) / step * step;
                }
                while at < end.timestamp().as_second() {
                    if let Ok(t) = Timestamp::from_second(at) {
                        out.push((t.to_zoned(zone.clone()), None));
                    }
                    at += step;
                }
            }
        }
        out
    }
}

/// A take of a medicine at set times each day: its time ("08:00") and its
/// amount ("1 tablet"; "" when none is said).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Take {
    pub time: String,
    #[serde(default)]
    pub amount: String,
}

/// Why takes could not be set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TakeProblem {
    /// No take at all: a medicine at set times would never fall due.
    None,
    /// A time that does not read, as written.
    Unreadable(String),
    /// Two takes at the same time: one dose, whose amount would be lost.
    Twice(String),
}

impl Medicine {
    /// What reminders, the day's list and a phone's card call it: its name,
    /// else (a file written by hand) its generic name.
    pub fn short_name(&self) -> String {
        if self.name.trim().is_empty() { self.generic.trim().to_string() } else { self.name.trim().to_string() }
    }

    /// What it is, precisely, for whoever prescribes or hands it out: its
    /// name, then its generic name and strength ("Thyrolan — levothyroxine
    /// 75 µg"); the generic name alone when it is the name ("levothyroxine
    /// 75 µg"); the name and strength without a generic name.
    pub fn precise(&self) -> String {
        let (name, generic, strength) = (self.name.trim(), self.generic.trim(), self.strength.trim());
        let molecule = [generic, strength].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
        if generic.is_empty() {
            [name, strength].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
        } else if name.is_empty() || name.to_lowercase() == generic.to_lowercase() {
            molecule
        } else {
            format!("{name} — {molecule}")
        }
    }

    /// The amount said with a dose set for `time` of the day: the take's own
    /// when the takes differ (none when that take has none), else the
    /// medicine's dose.
    pub fn amount_at(&self, time: Time) -> String {
        match &self.schedule {
            Schedule::Day { amounts, .. } if !amounts.is_empty() => amounts.iter().find(|(at, _)| time_of(at) == Some(time)).map(|(_, amount)| amount.clone()).unwrap_or_default(),
            _ => self.dose.clone(),
        }
    }

    /// Its takes, at set times each day, in time order, each with its amount
    /// (`amount_at`); a time that does not read is left out, as it never falls
    /// due. None for the other kinds.
    pub fn takes(&self) -> Vec<Take> {
        let Schedule::Day { times, .. } = &self.schedule else { return Vec::new() };
        let mut set: Vec<Time> = times.iter().filter_map(|t| time_of(t)).collect();
        set.sort();
        set.dedup();
        set.into_iter().map(|time| Take { time: format!("{:02}:{:02}", time.hour(), time.minute()), amount: self.amount_at(time) }).collect()
    }

    /// The amount most of its takes have, the earliest first on a tie: what
    /// its form shows as its dose, each take saying its own only when it
    /// differs. Its dose when the takes do not differ.
    pub fn usual(&self) -> String {
        let takes = self.takes();
        if !matches!(&self.schedule, Schedule::Day { amounts, .. } if !amounts.is_empty()) || takes.is_empty() {
            return self.dose.clone();
        }
        let count = |amount: &str| takes.iter().filter(|t| t.amount == amount).count();
        let most = takes.iter().map(|t| count(&t.amount)).max().unwrap_or(0);
        takes.iter().find(|t| count(&t.amount) == most).map(|t| t.amount.clone()).unwrap_or_default()
    }

    /// Its takes in a line, in time order: "08:00 · 1 tablet, 20:00 · 2
    /// tablets" ("12:00" alone for a take with no amount). What an older
    /// Sioul says with each dose when the takes differ (`set_takes`).
    pub fn takes_line(&self) -> String {
        self.takes().into_iter().map(|t| if t.amount.is_empty() { t.time } else { format!("{} · {}", t.time, t.amount) }).collect::<Vec<_>>().join(", ")
    }

    /// Takes it at set times each day: `takes`, each with its own amount, or
    /// `usual` when it says none. The same amount for all: that is its dose,
    /// and `amounts` stays empty (the file as an older Sioul writes it).
    /// Amounts that differ: `amounts` holds every take's, and its dose is the
    /// whole line (`takes_line`), so that an older Sioul, which reads the
    /// times alone, says every take's amount with each dose rather than one
    /// that is wrong for some. The times are what they say, whatever the amounts.
    pub fn set_takes(&mut self, usual: &str, takes: &[Take]) -> Result<(), TakeProblem> {
        let mut set: Vec<(Time, String)> = Vec::new();
        for take in takes {
            let time = time_of(&take.time).ok_or_else(|| TakeProblem::Unreadable(take.time.trim().to_string()))?;
            if set.iter().any(|(t, _)| *t == time) {
                return Err(TakeProblem::Twice(format!("{:02}:{:02}", time.hour(), time.minute())));
            }
            let own = take.amount.trim();
            set.push((time, if own.is_empty() { usual.trim() } else { own }.to_string()));
        }
        if set.is_empty() {
            return Err(TakeProblem::None);
        }
        set.sort();
        let times: Vec<String> = set.iter().map(|(t, _)| format!("{:02}:{:02}", t.hour(), t.minute())).collect();
        if set.iter().all(|(_, amount)| *amount == set[0].1) {
            self.dose = set[0].1.clone();
            self.schedule = Schedule::Day { times, amounts: BTreeMap::new() };
        } else {
            let amounts = times.iter().zip(&set).filter(|(_, (_, amount))| !amount.is_empty()).map(|(time, (_, amount))| (time.clone(), amount.clone())).collect();
            self.schedule = Schedule::Day { times, amounts };
            self.dose = self.takes_line();
        }
        Ok(())
    }
}

/// The pause to move: while a focus session runs, after this many minutes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Movement {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "forty_five")]
    pub minutes: u32,
}

impl Default for Movement {
    fn default() -> Movement {
        Movement { enabled: true, minutes: 45 }
    }
}

fn yes() -> bool {
    true
}

fn forty_five() -> u32 {
    45
}

/// Chats, a limit a day: after `minutes` of use they are covered, silent
/// and muted, for `locked_minutes`; then they come back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatLimit {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub minutes: u32,
    #[serde(default)]
    pub locked_minutes: u32,
}

impl Health {
    pub fn default_path() -> PathBuf {
        crate::config::data_dir().join("health.toml")
    }

    /// The health file; empty when there is none. One that no longer reads
    /// (edited by hand, written by a newer Sioul on another computer) is
    /// copied aside first, as `health.toml.unreadable`: saving over it then
    /// loses no medicine.
    pub fn load(path: &Path) -> Health {
        match Health::read(path) {
            Ok(health) => health,
            // Lost (of no bytes, or gone though written here): nothing to copy aside.
            Err(Unsound::Lost) => Health::default(),
            Err(Unsound::Unreadable) => {
                let _ = std::fs::copy(path, path.with_extension("toml.unreadable"));
                Health::default()
            }
        }
    }

    /// What `change` answers when the file does not read: nothing was written.
    /// The window says it in your words.
    pub const UNREADABLE: &'static str = "health.toml does not read: nothing written over it";

    /// The health file as it reads, or why not: empty when there is none;
    /// `Unsound::Unreadable` when it is there and does not read, which the
    /// window says rather than read it as no medicines (`health::load_said`).
    /// Nothing copied aside here (`load` does).
    pub fn read(path: &Path) -> Result<Health, Unsound> {
        Health::read_bytes(path).map(|(health, _)| health)
    }

    /// `read`, with the very bytes that read: what is kept as its last read
    /// (`health::keep_last_read` in the window), never a second look at the
    /// file. The doses' records' rule (review of 36002ea's check, A2): Sioul
    /// never writes it empty (its tables are always written), so a file of no
    /// bytes, or one gone though written here before (`witnessed`), was lost
    /// (a crash before its bytes reached the disk, a full disk, a folder
    /// wiped), never read as no medicines.
    pub fn read_bytes(path: &Path) -> Result<(Health, Vec<u8>), Unsound> {
        match std::fs::read(path) {
            Ok(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Err(Unsound::Lost),
            Ok(bytes) => std::str::from_utf8(&bytes).ok().and_then(|text| toml::from_str::<Health>(text).ok()).map(|health| (health, bytes)).ok_or(Unsound::Unreadable),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if witnessed(path) {
                    Err(Unsound::Lost)
                } else {
                    Ok((Health::default(), Vec::new()))
                }
            }
            Err(_) => Err(Unsound::Unreadable),
        }
    }

    /// Written as the doses' records are (`write_witnessed`): next to its
    /// place, its bytes on the disk before it takes the name, then its
    /// witnesses made, so that a crash after a save leaves a file lost, said,
    /// never one of no bytes read as no medicines (review of 36002ea's check,
    /// A2). Yours alone. Under the file's lock, which the sharing takes too.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        crate::filelock::with_lock(path, || write_witnessed(path, toml::to_string(self).map_err(|e| e.to_string())?.as_bytes()))
    }

    /// Read, changed by `change` and written back, all under the file's lock,
    /// which the sharing takes too: a change is set over the file as it is
    /// then, never over a copy read earlier. Returns what `change` returns. A
    /// file that does not read is never written over (`Health::UNREADABLE`): written
    /// empty, the sharing would take every medicine out on every device
    /// (review of 36002ea, can-wait 2); it is copied aside (`load`) and left.
    pub fn change<R>(path: &Path, change: impl FnOnce(&mut Health) -> Result<R, String>) -> Result<R, String> {
        crate::filelock::with_lock(path, || {
            let Ok(mut health) = Health::read(path) else {
                let _ = Health::load(path);
                return Err(Self::UNREADABLE.to_string());
            };
            let out = change(&mut health)?;
            health.save(path)?;
            Ok(out)
        })
    }

    /// A new id among the others, from a name.
    pub fn new_id(&self, name: &str) -> String {
        let taken: Vec<String> = self.prescriptions.iter().map(|p| p.id.clone()).chain(self.medicines.iter().map(|m| m.id.clone())).collect();
        crate::projects::new_id(name, &taken)
    }

    /// Every dose from `start` to `end`, of the medicines taken then, each
    /// with its take's amount (`Medicine::amount_at`).
    pub fn doses(&self, start: &Zoned, end: &Zoned) -> Vec<Dose> {
        let mut out: Vec<Dose> = self
            .medicines
            .iter()
            .filter(|m| !m.paused)
            .flat_map(|m| {
                m.schedule.timed(start, end).into_iter().filter(|(at, _)| m.until.is_none_or(|until| at.date() <= until)).map(|(at, set)| Dose {
                    key: format!("{}@{}", m.id, at.timestamp().as_second()),
                    medicine: m.id.clone(),
                    name: m.short_name(),
                    dose: set.map_or_else(|| m.dose.clone(), |time| m.amount_at(time)),
                    at,
                })
            })
            .collect();
        out.sort_by(|a, b| a.at.cmp(&b.at).then(a.name.cmp(&b.name)));
        out
    }

    /// The amount of the dose `key` (`<medicine>@<Unix seconds>`), as its
    /// reminder said it: its take's, found at its time; else (a dose the
    /// schedule no longer has, moved or changed since) the amount at its time
    /// of day on this clock. None for a medicine no longer here.
    pub fn amount_of(&self, key: &str, zone: &jiff::tz::TimeZone) -> Option<String> {
        let (id, due) = key.rsplit_once('@')?;
        let due = Timestamp::from_second(due.parse().ok()?).ok()?.to_zoned(zone.clone());
        let medicine = self.medicines.iter().find(|m| m.id == id)?;
        let end = due.checked_add(Span::new().seconds(1)).ok()?;
        match medicine.schedule.timed(&due, &end).into_iter().next() {
            Some((_, Some(time))) => Some(medicine.amount_at(time)),
            Some((_, None)) => Some(medicine.dose.clone()),
            None => Some(match medicine.schedule {
                Schedule::Day { .. } => medicine.amount_at(Time::new(due.hour(), due.minute(), 0, 0).ok()?),
                _ => medicine.dose.clone(),
            }),
        }
    }

    /// A dose taken at `at` (Unix seconds): for a medicine taken every few
    /// hours, the next doses come that many hours after it, early or late, so
    /// that the hours between two doses are kept. Medicines at set times keep
    /// theirs. Returns where the doses started before and after they moved,
    /// to put them back if the mark is taken back; none when nothing moved.
    pub fn taken_at(&mut self, key: &str, at: i64) -> Option<(i64, i64)> {
        let (id, due) = key.rsplit_once('@')?;
        let due: i64 = due.parse().ok()?;
        let medicine = self.medicines.iter_mut().find(|m| m.id == id)?;
        let Schedule::Hours { hours, from } = &mut medicine.schedule else { return None };
        let step = i64::from(*hours) * 3600;
        if step == 0 || at == due {
            return None;
        }
        // Only a dose the schedule still has moves the next ones: once moved, a
        // dose taken earlier is no longer one of them, and taking it again (a
        // late time typed after its Taken) never moves them a second time
        // (review of 36002ea, can-wait 1).
        if due < *from || (due - *from) % step != 0 {
            return None;
        }
        let before = *from;
        *from = at + step;
        Some((before, *from))
    }

    /// The moves of medicines taken every few hours, mended: a dose taken
    /// late moves the next ones (`taken_at`), inside the medicine, which
    /// travels whole between devices; changed meanwhile on another device (its
    /// note, its dose's text), the medicine there, its start as it was, can win
    /// everywhere, and the next dose would come early, the hours between doses
    /// broken (review of 5 October 2026, F23). Each move is kept with its mark
    /// (`HealthState::moved`), which travels apart: a medicine whose start is
    /// one a mark moved it from goes again where that mark moved it, its
    /// other changes kept. Two moves from one start (the second marked on a
    /// device the first had not reached yet): the latest mark's, by when it
    /// was taken (review of 10 October 2026, R10). Returns whether one moved.
    pub fn mend_shifts(&mut self, state: &HealthState) -> bool {
        let mut mended = false;
        for medicine in &mut self.medicines {
            let Schedule::Hours { from, .. } = &mut medicine.schedule else { continue };
            // Each move with when its mark was taken (none known: the earliest).
            let moves: Vec<([i64; 2], i64)> = state.moved.iter().filter(|(key, _)| key.rsplit_once('@').is_some_and(|(id, _)| id == medicine.id)).map(|(key, moved)| (*moved, state.taken.get(key).copied().unwrap_or(i64::MIN))).collect();
            // Moved several times since: each from where the one before left it.
            for _ in 0..moves.len() {
                let Some(([_, after], _)) = moves.iter().filter(|([before, after], _)| *before == *from && after != before).max_by_key(|(_, taken)| *taken) else { break };
                *from = *after;
                mended = true;
            }
            // A move taken back (`HealthState::back`): its start put back, unless it moved again since.
            let backs = state.back.iter().filter(|(key, _)| key.rsplit_once('@').is_some_and(|(id, _)| id == medicine.id));
            if let Some((_, [before, _, _])) = backs.filter(|(_, [before, after, _])| *after == *from && after != before).max_by_key(|(_, [_, _, at])| *at) {
                *from = *before;
                mended = true;
            }
        }
        mended
    }

    /// A mark taken back: the doses it moved go back where they were, unless
    /// they moved again since. Returns whether they did.
    pub fn taken_back(&mut self, key: &str, before: i64, after: i64) -> bool {
        let Some((id, _)) = key.rsplit_once('@') else { return false };
        let Some(medicine) = self.medicines.iter_mut().find(|m| m.id == id) else { return false };
        match &mut medicine.schedule {
            Schedule::Hours { from, .. } if *from == after => {
                *from = before;
                true
            }
            _ => false,
        }
    }

    /// The pharmacy and the doctor, as errands: when a prescription's
    /// medicines run out (two days before) and when it ends (two weeks before).
    pub fn errands(&self) -> Vec<Errand> {
        let mut out = Vec::new();
        for p in &self.prescriptions {
            if let (Some(days), Some(last)) = (p.refill_days, p.last_refill)
                && let Ok(out_of) = last.checked_add(Span::new().days(i64::from(days)))
                && p.until.is_none_or(|until| out_of <= until)
            {
                let day = out_of.checked_sub(Span::new().days(2)).unwrap_or(out_of);
                out.push(Errand { key: format!("refill:{}:{}", p.id, out_of), prescription: p.id.clone(), kind: ErrandKind::Refill, day, title: p.title.clone() });
            }
            if let Some(until) = p.until {
                let day = until.checked_sub(Span::new().days(14)).unwrap_or(until);
                out.push(Errand { key: format!("renew:{}:{until}", p.id), prescription: p.id.clone(), kind: ErrandKind::Renew, day, title: p.title.clone() });
            }
        }
        out.sort_by_key(|e| e.day);
        out
    }
}

/// Another device sharing with this one, as known here, for the doses
/// (docs/health.md, "Knowing"): what it last said of itself in the sharing
/// folder (its entry in the devices' registry, `said`), and, for an older
/// Sioul that writes none, what its claims said (until when everything it
/// wrote is read here, whether it said it closed, how late its news comes).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    /// Its name, to say where: the host's; a phone's is said "the phone".
    pub name: String,
    /// From its claims (an older Sioul): everything it wrote until then (Unix
    /// seconds) is read here; 0: never sure.
    #[serde(default)]
    pub known_until: i64,
    /// From its claims: at `known_until`, it said it closed (quit, or put away
    /// on a phone): it marks nothing until it says otherwise.
    #[serde(default)]
    pub closed: bool,
    /// From its claims: the longest its news took to come here lately, in
    /// seconds; none measured yet.
    #[serde(default)]
    pub delay: Option<i64>,
    /// When a line it wrote was found unreadable here: what it said is lost.
    #[serde(default)]
    pub broken: Option<i64>,
    /// When it was last heard at all.
    #[serde(default)]
    pub heard: i64,
    /// Its id in the sharing; "" for a device whose claim does not read here.
    #[serde(default)]
    pub id: String,
    /// What it last said of itself in the folder, as read here; none from an
    /// older Sioul, which says nothing of the kind, or before it is read.
    #[serde(default)]
    pub said: Option<Said>,
    /// Everything it wrote up to its last export (`Said::exported`) is read here.
    #[serde(default)]
    pub complete: bool,
    /// When this device last saw its entry change: this device's clock (Unix seconds).
    #[serde(default)]
    pub seen: i64,
    /// How far its clock is ahead of this device's, at least, in seconds:
    /// measured here (`Said::exported` against when it was seen), 0 when not
    /// seen ahead. A clock behind only makes Sioul wait longer.
    #[serde(default)]
    pub ahead: i64,
    /// You said it is off: not counted until it shows life again.
    #[serde(default)]
    pub off: bool,
    /// How late the news of its opening comes here: the longest a start of
    /// its was seen after it began, of its last starts seen while this device
    /// looked (seconds, this device's clock, its clock's lead taken off);
    /// none measured yet. A device closed before a dose may open around it to
    /// mark it: known closed only once that while went by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wake: Option<i64>,
    /// When its claim on the doses was last renewed while open, as read here
    /// (its clock; 0 none): a computer whose entry says it closed but whose
    /// claim came later is open, its entry not here yet (a sync app carrying
    /// files one by one: the sixth review's R1d).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub claimed: i64,
    /// You forgot it here (it left the list of your devices): it owes this
    /// device no word on its openings (`off` counts it out of the doses too).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forgotten: bool,
    /// The opening of it (its entry's `started`) this device said it read, in
    /// its own sealed entry (`devices::Entry::acked`; a phone's word on a
    /// computer); 0 none. A phone takes that computer's "closed" as its word
    /// only for this opening or a later one it read (the seventh review's M1).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub acknowledged: i64,
    /// The latest opening of it ever read here (its entry's `started`), never
    /// lowered: an older copy of its entry put back cannot make a phone's word
    /// on it go back (the eighth review's N3).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub latest_opening: i64,
    /// Its entry read now is dated more than ten minutes ahead of this
    /// device's clock (sioul-app's `FUTURE`): set aside while it reads so (a
    /// copy read later, or arriving late, within ten minutes by then, is
    /// taken). Nothing it says counts, nor a word it gives; its doubt says
    /// why (`Doubt::Ahead`), never "an older Sioul" (the final review before
    /// 0.0.6, F1H and its words; the re-check's LOW-1).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub set_aside: bool,
}

fn is_zero_i64(n: &i64) -> bool {
    *n == 0
}

/// What a device says of itself in the sharing folder: its entry in the
/// devices' registry (`sioul_sync::devices`), its times on its own clock (Unix seconds).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Said {
    /// Its last start: Sioul opened; on a phone, Sioul back on the screen, or
    /// one of its reminders handled while it was not.
    pub started: i64,
    /// Its last clean close, after its last export; 0 never.
    pub closed: i64,
    /// From its start until the last step of a clean close: a crash leaves it so.
    pub working: bool,
    /// When its files were last read for an export: every answer captured
    /// there before is in its records (read here when `Peer::complete`).
    pub exported: i64,
    /// When it last read the others.
    pub imported: i64,
    /// It shares the health part: the answers it captures travel.
    pub doses: bool,
    /// It stopped sharing: it counts no more.
    pub left: bool,
    /// A phone, said "the phone"; else a computer, said by its name.
    pub phone: bool,
}

/// A device a doubt names: its id (for "This device is off"), its name, a phone or not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub name: String,
    pub phone: bool,
}

/// Why a dose is not known here: whether it was taken, Sioul cannot tell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doubt {
    /// This device's record of doses could not be read, or was lost, then.
    Record { since: i64 },
    /// An older Sioul, which writes no entry in the registry, may have marked
    /// it: everything it wrote is known until then (0: never), and it was open
    /// or closed then.
    Unheard { device: Named, until: i64, closed: bool },
    /// A line another device wrote could not be read here.
    Broken { device: Named },
    /// In use, and it has not shared since the dose was due, or not lately:
    /// `shared`, when it last did (its clock); `quiet`, not seen changing for
    /// over an hour (`QUIET`): it may have stopped without closing.
    Working { device: Named, shared: i64, quiet: bool },
    /// What it last shared, then (`shared`), has not all come here yet.
    Coming { device: Named, shared: i64 },
    /// It does not share its doses (its Health part switched off), and it was
    /// not closed before the dose was due.
    Apart { device: Named },
    /// Its clock runs more than ten minutes ahead of this device's: what it
    /// says is set aside here (`Peer::set_aside`), so nothing of it is known.
    Ahead { device: Named },
}

/// The longest a closed computer's news may take to come here for it to count
/// as closed, for an older Sioul known by its claims only: had it opened
/// again, it would be known by now.
pub const NEWS_IN: i64 = 3 * 60;

/// How long a device in use stays known after its last export, as of now:
/// within it, an answer it captured is one you have just given. Beyond it,
/// what it captured since may still be on its way.
pub const FRESH: i64 = 5 * 60;

/// How far apart two devices' clocks are taken to be, beyond what is measured
/// here (`Peer::ahead`): a guess; devices set their clocks from the network,
/// to within seconds.
pub const SKEW: i64 = 2 * 60;

/// How long after a dose's time a phone's alarm rings when a computer keeps
/// the doses (its wait for news, sioul-app's `WAIT_FOR_NEWS`): a phone put
/// away is opened then by its own alarm (docs/health.md, "On a phone").
pub const ALARM_AFTER: i64 = 10 * 60;

/// When a phone put away before a dose may last have been opened for it, at
/// `now`: by its own alarm (`ALARM_AFTER`) once that rang, else at the dose's
/// time (picked up to mark it); a computer's opening is the dose's time.
fn opened_for(due: i64, now: i64, phone: bool) -> i64 {
    if phone && now >= due + ALARM_AFTER { due + ALARM_AFTER } else { due }
}

/// A device in use not seen changing for this long may have stopped without
/// closing (a crash): said so. Only the words change.
pub const QUIET: i64 = 60 * 60;

/// How long before a dose's time a line lost from another device's records
/// may have held its answer (a dose marked ahead of its time), in seconds.
pub const BROKEN_SPAN: i64 = 12 * 3600;

/// A device silent this long stops counting for the doses, and Settings ▸ Your
/// folder and sharing offers to forget it: a guess (a computer away for a long
/// weekend comes back within it).
pub const SILENT_DAYS: i64 = 7;

/// How many phones count among `peers` at `now` (`counts`).
fn phones_counted(peers: &[Peer], now: i64) -> usize {
    peers.iter().filter(|p| counts(p, now) && p.said.as_ref().is_some_and(|s| s.phone)).count()
}

/// A device as a doubt names it: "the phone" only when it is the one phone
/// among `phones` counted; two or more, each by its name. Words only: what
/// a phone does (its alarm) is read from `Said::phone` itself (review of
/// battery part C, the fifth review's C1).
fn named(peer: &Peer, phones: usize) -> Named {
    Named { id: peer.id.clone(), name: peer.name.clone(), phone: peer.said.as_ref().is_some_and(|s| s.phone) && phones == 1 }
}

/// Whether `peer` counts for the doses at `now`: not said off by you, still
/// sharing, and heard within `SILENT_DAYS`, by this device's clock (its entry
/// seen changing) or its own (its last export).
pub fn counts(peer: &Peer, now: i64) -> bool {
    if peer.off {
        return false;
    }
    let silent = SILENT_DAYS * 86_400;
    match &peer.said {
        Some(said) if said.left => false,
        Some(said) => now - peer.seen <= silent || now - said.exported <= silent,
        None => now - peer.heard < silent,
    }
}

/// Why a dose due at `due` (Unix seconds), not answered here, may have been
/// answered on another device, as of `now`; none when it is known not taken
/// (docs/health.md, "Knowing"). This device's record must read, and each
/// other device that counts (`counts`) be one of:
/// - closed cleanly (`working` down, `closed` after `started`, both on its
///   own clock), everything it wrote up to its last export read here: closed
///   after the dose, or before it and not started since. Its last export holds
///   every answer it captured, and it captures none until it starts again;
/// - in use, having exported after the dose (its clock, allowing `SKEW` and
///   how far ahead its clock was seen) and lately (seen changing here within
///   `FRESH`, and dated within it), everything read: an answer captured since
///   goes out at once and comes within the sync's time;
/// - an older Sioul, which writes no registry: as its claims say, heard in
///   full after the dose and within `FRESH`, or its last word, read in full,
///   that it closed, its news quick (`NEWS_IN`). Never closed for want of a word.
///
/// Otherwise the doubt names it. One rule for reminders, near the dose's time,
/// and for what stays in view hours after it (the Porch, the Health page, a
/// phone's home screen). A dose taken twice can harm: what is not known is
/// said, never guessed.
pub fn doubts_now(due: i64, now: i64, record_lost: Option<i64>, peers: &[Peer]) -> Vec<Doubt> {
    doubts_at(due, due, now, record_lost, peers)
}

/// `doubts_now` for a dose that waited for `waking` (sleep or a pause held
/// it: its time, else): what looks back counts from the dose's time (a
/// record lost after it, a device apart closed before it, a line lost
/// around it, a device closed or exported after it), and only the moment a
/// device may have been opened for it counts from waking (picked up then,
/// or its alarm, held until waking, rung after: review of battery part C,
/// the sixth review's M1).
pub fn doubts_at(due: i64, waking: i64, now: i64, record_lost: Option<i64>, peers: &[Peer]) -> Vec<Doubt> {
    let mut out = Vec::new();
    // Doses due before the record was found broken or gone, in the day before.
    if let Some(since) = record_lost.filter(|since| due <= *since && *since - due < 86_400) {
        out.push(Doubt::Record { since });
    }
    let phones = phones_counted(peers, now);
    for peer in peers.iter().filter(|p| counts(p, now)) {
        let device = named(peer, phones);
        // Its entry set aside, its clock far ahead of this one's: nothing it says is known.
        if peer.set_aside {
            out.push(Doubt::Ahead { device });
            continue;
        }
        // A line lost from around the dose's time may have been its answer:
        // one written before it, or within `BROKEN_SPAN` after (a dose marked
        // ahead of its time). The line is dated by its own time, never by when
        // it was found (a round read again finds an old one again).
        if peer.broken.is_some_and(|at| at >= due - BROKEN_SPAN) {
            out.push(Doubt::Broken { device });
            continue;
        }
        // Closed before the dose, it may open again around it to mark it: the
        // news of its opening comes after a while, as measured on its last
        // openings (`Peer::wake`), and it is known closed only once that while
        // went by, and `SKEW` more (a lead of its clock measured here is that
        // lead less the quickest news seen: its openings timed short by as
        // much; review of 10 October 2026, R8). None measured yet: as before,
        // at once. A phone put away is opened by its own alarm too, after the
        // dose (`ALARM_AFTER`): from then on, counted from its alarm, never
        // from the dose's time (review of battery part C, H2).
        let phone = peer.said.as_ref().is_some_and(|s| s.phone);
        let woke = peer.wake.is_none_or(|wake| now - opened_for(waking, now, phone) >= wake + SKEW);
        let Some(said) = &peer.said else {
            let closed = peer.closed && peer.heard <= peer.known_until;
            let quiet = closed && peer.delay.is_some_and(|d| d <= NEWS_IN) && (peer.known_until >= due + SKEW || woke);
            let fresh = peer.known_until >= due && now - peer.known_until <= FRESH;
            if !quiet && !fresh {
                out.push(Doubt::Unheard { device, until: peer.known_until, closed });
            }
            continue;
        };
        // Its entry came ahead of its records: what it last shared may hold the answer.
        if !peer.complete {
            out.push(Doubt::Coming { device, shared: said.exported });
            continue;
        }
        let closed = !said.working && said.closed >= said.started;
        let margin = SKEW + peer.ahead.max(0);
        if !said.doses {
            // Its answers never travel: known only if it was closed before the
            // dose was due, its clock maybe `SKEW` behind this one's, and the
            // news of an opening since had the time to come.
            if !(closed && said.closed + SKEW < due) {
                out.push(Doubt::Apart { device });
            } else if !woke {
                out.push(Doubt::Unheard { device, until: said.closed, closed: true });
            }
            continue;
        }
        if closed {
            // Closed after the dose (its clock, allowing `SKEW` and its lead):
            // its last export holds every answer it captured until then. Closed
            // before: it may have opened again since to mark it (a phone picked
            // up for that, its sync slow to wake: review of 5 October 2026, F21),
            // known only once the news of an opening had the time to come.
            if said.closed < due + margin && !woke {
                out.push(Doubt::Unheard { device, until: said.closed, closed: true });
            }
            continue;
        }
        let after = said.exported >= due + margin;
        let fresh = now - peer.seen <= FRESH && now - said.exported <= FRESH + margin;
        if !(after && fresh) {
            out.push(Doubt::Working { device, shared: said.exported, quiet: now - peer.seen > QUIET });
        }
    }
    out
}

/// The same, as a dose's reminder asks it, near its time: one rule, kept by
/// its name for its callers.
pub fn doubts(due: i64, now: i64, record_lost: Option<i64>, peers: &[Peer]) -> Vec<Doubt> {
    doubts_now(due, now, record_lost, peers)
}

/// `doubts_now`, at a phone's alarm whose own pull did not go through since
/// it began (`since`, Unix seconds): failed, or too slow, the sync app asked
/// instead. Of the devices in `heard`, an entry or a health claim saying more
/// came since: what they say is as fresh as the sync app makes it. Any other known
/// only by what it said before then, closed (or an older Sioul whose claims
/// say so), may have opened since and answered: said in doubt, as it was last
/// heard, never known (docs/health.md, "Knowing"). One in use stays as `FRESH`
/// has it: known only while its news is minutes old, which the alarm's own
/// wait already spends.
pub fn doubts_unread(due: i64, now: i64, record_lost: Option<i64>, peers: &[Peer], since: i64, heard: &std::collections::BTreeSet<String>) -> Vec<Doubt> {
    doubts_unread_at(due, due, now, record_lost, peers, since, heard)
}

/// `doubts_unread` for a dose that waited for `waking` (`doubts_at`).
pub fn doubts_unread_at(due: i64, waking: i64, now: i64, record_lost: Option<i64>, peers: &[Peer], since: i64, heard: &std::collections::BTreeSet<String>) -> Vec<Doubt> {
    let mut out = doubts_at(due, waking, now, record_lost, peers);
    let phones = phones_counted(peers, now);
    for peer in peers.iter().filter(|p| counts(p, now) && !heard.contains(&p.id)) {
        let device = named(peer, phones);
        let names = |doubt: &Doubt| match doubt {
            Doubt::Unheard { device: d, .. } | Doubt::Broken { device: d } | Doubt::Working { device: d, .. } | Doubt::Coming { device: d, .. } | Doubt::Apart { device: d } | Doubt::Ahead { device: d } => *d == device,
            Doubt::Record { .. } => false,
        };
        if out.iter().any(names) {
            continue;
        }
        let until = match &peer.said {
            Some(said) if said.working => continue,
            Some(said) => said.closed,
            None if peer.closed => peer.known_until,
            None => continue,
        };
        out.push(Doubt::Unheard { device, until: until.clamp(1, since.max(1)), closed: true });
    }
    out
}

/// The first moment from `from` on, before `until`, at which a dose due at
/// `due` is known not taken (`doubts_now`) while nothing new is read: a
/// device closed before it becomes known so once the news of an opening had
/// the time to come (`Peer::wake`), a device in use only goes stale. None when
/// it is not known by `until`. For what is shown ahead of time (a phone's home
/// screen card): "check" until then.
pub fn known_from(due: i64, from: i64, until: i64, record_lost: Option<i64>, peers: &[Peer]) -> Option<i64> {
    known_from_at(due, due, from, until, record_lost, peers)
}

/// `known_from` for a dose that waited for `waking` (`doubts_at`).
pub fn known_from_at(due: i64, waking: i64, from: i64, until: i64, record_lost: Option<i64>, peers: &[Peer]) -> Option<i64> {
    let phone = |p: &Peer| p.said.as_ref().is_some_and(|s| s.phone);
    let after_alarm = peers.iter().filter(|p| phone(p)).filter_map(|p| p.wake).map(|w| waking + ALARM_AFTER + w + SKEW);
    let mut moments: Vec<i64> = std::iter::once(from).chain(peers.iter().filter_map(|p| p.wake).map(|w| waking + w + SKEW)).chain(after_alarm).filter(|t| *t >= from && *t < until).collect();
    moments.sort_unstable();
    moments.dedup();
    moments.into_iter().find(|t| doubts_at(due, waking, *t, record_lost, peers).is_empty())
}

/// Until when a dose due at `due`, known not taken at `from` (`doubts_now`),
/// stays known while nothing new is read (a device in use goes stale after
/// `FRESH`): the first moment it is not, else `until`; 0 when it is not known
/// at `from`. For what is shown ahead of time (a phone's home screen card).
pub fn known_until(due: i64, from: i64, until: i64, record_lost: Option<i64>, peers: &[Peer]) -> i64 {
    let doubted = |at: i64| !doubts_now(due, at, record_lost, peers).is_empty();
    if doubted(from) {
        return 0;
    }
    let mut ends: Vec<i64> = Vec::new();
    for peer in peers {
        match &peer.said {
            Some(said) => {
                let margin = SKEW + peer.ahead.max(0);
                ends.push(peer.seen + FRESH + 1);
                ends.push(said.exported + FRESH + margin + 1);
                // A phone put away, opened by its alarm: unknown again from then.
                if said.phone && peer.wake.is_some() {
                    ends.push(due + ALARM_AFTER);
                }
            }
            None => ends.push(peer.known_until + FRESH + 1),
        }
    }
    ends.retain(|t| *t > from && *t < until);
    ends.sort_unstable();
    ends.dedup();
    ends.into_iter().find(|t| doubted(*t)).unwrap_or(until)
}

/// What the body's files hold is yours alone: on Unix, readable by you only
/// (0600), whatever the system's default for new files.
pub(crate) fn keep_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// The witnesses of a record of the doses (the marks, `health-state.toml`;
/// each dose's record, `health-doses.toml`), made the first time it is
/// written: gone, or of no bytes, with a witness there, it was lost, not
/// never written. One beside it; one apart, in the data folder with the
/// medicines, for a record in its usual place, the state folder: a state
/// folder wiped whole (a reset, a backup put back without it) takes the
/// record and the witness beside it, never that one (review of 5 October
/// 2026, F22).
pub(crate) fn witnesses(path: &Path) -> Vec<PathBuf> {
    let name = path.file_name().map(|n| format!(".{}.written", n.to_string_lossy())).unwrap_or_default();
    let mut all = vec![path.with_file_name(&name)];
    #[cfg(test)]
    let apart = APART.with(|apart| apart.borrow().as_ref().map(|dir| dir.join(&name)));
    // Kept in the other folder too: a record of the state folder's in the data
    // folder's, the health file of the data folder's in the state folder's.
    #[cfg(not(test))]
    let apart = if path.parent() == Some(crate::config::state_dir().as_path()) {
        Some(crate::config::data_dir().join(&name))
    } else if path.parent() == Some(crate::config::data_dir().as_path()) {
        Some(crate::config::state_dir().join(&name))
    } else {
        None
    };
    all.extend(apart.filter(|apart| !all.contains(apart)));
    all
}

#[cfg(test)]
thread_local! {
    /// Where the witness apart is kept, in this test's thread (else none).
    pub(crate) static APART: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Whether a record of the doses was written here before (`witnesses`).
pub fn witnessed(path: &Path) -> bool {
    witnesses(path).iter().any(|witness| witness.exists())
}

/// A record of the doses written next to its place, its bytes on the disk
/// before it takes its name (a phone's file system may keep the name of a
/// file whose bytes a crash lost: a record of no bytes), then its folder's
/// change made to last; its witnesses made when missing.
pub fn write_witnessed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_synced(path, bytes)?;
    // Once the bytes have the name, made when missing: a witness that could
    // not be made (a full disk) never says the write failed, which it did not
    // (review of 36002ea's check, low 23: a mark written, its move put back).
    for witness in witnesses(path) {
        if !witness.exists() {
            if let Some(folder) = witness.parent() {
                let _ = std::fs::create_dir_all(folder);
            }
            let _ = std::fs::write(&witness, "");
        }
    }
    Ok(())
}

/// Written next to its place, its bytes on the disk before it takes the
/// name, then its folder's change made to last: never half a file, never a
/// name over bytes a crash lost. Yours alone.
pub fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(fail)?;
    // Next to its place, then moved: a dose marked is never lost to half a file.
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let temporary = path.with_file_name(format!("{name}.new"));
    let mut file = std::fs::File::create(&temporary).map_err(fail)?;
    file.write_all(bytes).and_then(|()| file.sync_all()).map_err(fail)?;
    drop(file);
    keep_private(&temporary);
    std::fs::rename(&temporary, path).map_err(fail)?;
    #[cfg(unix)]
    let _ = std::fs::File::open(parent).and_then(|dir| dir.sync_all());
    Ok(())
}

/// One dose to take.
#[derive(Debug, Clone, PartialEq)]
pub struct Dose {
    /// `<medicine>@<Unix seconds>`: what marks it taken.
    pub key: String,
    pub medicine: String,
    pub name: String,
    pub dose: String,
    pub at: Zoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrandKind {
    /// Fetch the medicines at the pharmacy.
    Refill,
    /// See the doctor for a new prescription.
    Renew,
}

/// Something to do for a prescription, from a day on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Errand {
    /// Unique to its turn: the task made for it is made once.
    pub key: String,
    pub prescription: String,
    pub kind: ErrandKind,
    /// From when the task can start.
    pub day: Date,
    pub title: String,
}

/// What tells the task of an errand, in each language Sioul writes: the
/// category it is given ("health", « santé »), and its title's words around
/// the prescription's ("Pharmacy: …", "Doctor: renew the prescription for …").
struct ErrandWords {
    categories: Vec<String>,
    titles: Vec<(String, String)>,
}

fn errand_words() -> &'static ErrandWords {
    static WORDS: std::sync::OnceLock<ErrandWords> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| {
        // Where the prescription's title goes: a character no title holds.
        const MARK: &str = "\u{1}";
        let mut words = ErrandWords { categories: Vec::new(), titles: Vec::new() };
        for (language, _) in crate::i18n::LANGUAGES {
            let tr = crate::i18n::Translator::new(language);
            words.categories.push(tr.text("health-category", None).trim().to_lowercase());
            for id in ["health-errand-refill", "health-errand-renew"] {
                let mut args = crate::i18n::args();
                args.set("title", MARK);
                // Words around it, never the title alone: that would be every task.
                if let Some((before, after)) = tr.text(id, Some(&args)).split_once(MARK).filter(|(before, after)| !before.trim().is_empty() || !after.trim().is_empty()) {
                    words.titles.push((before.to_string(), after.to_string()));
                }
            }
        }
        words
    })
}

/// Whether a task is a pharmacy or renewal errand (`Health::errands`):
/// health, which never reaches an AI agent (docs/mcp.md). Known by the
/// category it was made with, in any language Sioul writes; by its UID among
/// the errands' tasks the record keeps (`recorded`: `HealthState::errand_uids`);
/// or by its title, one Sioul gives an errand in any language: a list that
/// keeps no category (Google Tasks) still keeps the title.
pub fn is_errand(task: &crate::tasks::Task, recorded: &std::collections::BTreeSet<String>) -> bool {
    if recorded.contains(&task.uid) {
        return true;
    }
    let words = errand_words();
    task.categories.iter().any(|c| words.categories.contains(&c.trim().to_lowercase())) || is_errand_title(&task.title)
}

/// Whether a title is one Sioul gives an errand, in any language it writes
/// ("Pharmacy: …", "Doctor: renew the prescription for …"): an errand's
/// task, its time block (titled as its task) or the appointment it was made
/// (`consent::Consent::keep_errands`; review of 36002ea, can-wait 7 and 8).
pub fn is_errand_title(title: &str) -> bool {
    let title = title.trim();
    errand_words().titles.iter().any(|(before, after)| title.len() > before.len() + after.len() && title.starts_with(before.as_str()) && title.ends_with(after.as_str()))
}

/// What is marked of the doses and done of the errands, kept apart from the
/// health file: `$XDG_STATE_HOME/sioul/health-state.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HealthState {
    /// Doses marked taken, by key, when (Unix seconds); kept a week.
    #[serde(default)]
    pub taken: BTreeMap<String, i64>,
    /// Doses already reminded, by key; kept a week.
    #[serde(default)]
    pub reminded: BTreeMap<String, i64>,
    /// Doses whose reminder was not shown, or not known shown, while Sioul
    /// ran, by key, when it was tried; kept a week. Not a reminder: such a
    /// dose is still asked about afterwards, under its own heading, since
    /// Sioul ran (`unanswered`), and another device reminds it (a phone's
    /// alarm). What each device did, and whether it may have shown, is in
    /// each dose's reminders (`dose_reminders`, a file of its own); this table
    /// says it to an older Sioul (0.0.4, 0.0.5), which reads it as a reminder
    /// not shown, never as one given: its alarm reminds the dose, plainly, as
    /// it always did. 0.0.3 knows no such table: the dose is not reminded
    /// there either. Written only when there is one: an older Sioul's file
    /// stays as it was.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unshown: BTreeMap<String, i64>,
    /// Doses answered "not taken" when asked afterwards, by key; kept a week.
    #[serde(default)]
    pub not_taken: BTreeMap<String, i64>,
    /// Doses whose take moved the next ones, by key: where they started
    /// before and after (`Health::taken_at`); kept with the mark.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub moved: BTreeMap<String, [i64; 2]>,
    /// Moves taken back, by key: where the next doses started before and
    /// after the take, and when it was taken back (Unix seconds); kept a
    /// week. Every device puts them back from here (`Health::mend_shifts`),
    /// its health file read or not where it was taken back (review of
    /// 36002ea's check, A2: a take back there once dropped its restore).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub back: BTreeMap<String, [i64; 3]>,
    /// Errands whose task was made, by key.
    #[serde(default)]
    pub errands: BTreeMap<String, String>,
    /// Chats: the day counted, the minutes used that day, and covered until when.
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub chat_day: Option<Date>,
    /// Never written while 0, as none: a record started again after a repair
    /// holds no minutes of its own, so it sends none over what the others'
    /// records hold (the day's chat minutes reset on every device after a
    /// repair: review of 15 October 2026, RV-P4 and RF-J). Read as before, 0
    /// when missing; an older Sioul reads it the same.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub chat_minutes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chats_locked_until: Option<i64>,
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

/// Why the doses' record cannot be trusted: what it held is not known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsound {
    /// It does not read: half written, broken by hand, or by a disk.
    Unreadable,
    /// It is gone, though it was written here before (its witness says so).
    Lost,
}

/// A change to the doses' record that could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The record cannot be trusted: nothing is written over it.
    Unsound(Unsound),
    /// It could not be written (a full disk): what it says.
    Write(String),
}

/// What follows an errand's key in the record where the id a list that
/// keeps no UID gave its task is kept (`HealthState::note_elsewhere`).
pub const ELSEWHERE: &str = "#elsewhere";

impl HealthState {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("health-state.toml")
    }

    /// The UIDs of the tasks made for errands (`errands`), alone: what keeps
    /// them from agents (`is_errand`); with them, the ids a list that keeps
    /// no UID gave them (`ELSEWHERE`). Nothing else of the record is kept,
    /// the errands' keys either (they name prescriptions). None, never an
    /// empty set, when the record does not read or was lost: the doubt is
    /// said (review of 36002ea, can-wait 10); nothing set aside or repaired here.
    pub fn errand_uids(path: &Path) -> Option<std::collections::BTreeSet<String>> {
        #[derive(Deserialize)]
        struct Errands {
            #[serde(default)]
            errands: BTreeMap<String, String>,
        }
        match std::fs::read_to_string(path) {
            Ok(text) if text.trim().is_empty() && witnessed(path) => None,
            Ok(text) => toml::from_str::<Errands>(&text).ok().map(|e| e.errands.into_values().collect()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (!witnessed(path)).then(std::collections::BTreeSet::new),
            Err(_) => None,
        }
    }

    /// The errands' tasks as a list that keeps no UID knows them (Google
    /// Tasks: its own id, `ids` by Sioul's UID), recorded beside each errand
    /// under its key and `ELSEWHERE`, so that a device that knows the task by
    /// that id alone keeps it from agents too (`errand_uids`; review of
    /// 36002ea, can-wait 10). Written only when one is new; nothing when the
    /// record does not read. An older Sioul reads such a key as no errand of
    /// its own, and the id among the errands' UIDs.
    pub fn note_elsewhere(path: &Path, now: i64, ids: &BTreeMap<String, String>) -> Result<(), Problem> {
        let new = |state: &HealthState| -> Vec<(String, String)> {
            state
                .errands
                .iter()
                .filter(|(key, _)| !key.ends_with(ELSEWHERE))
                .filter_map(|(key, uid)| Some((format!("{key}{ELSEWHERE}"), ids.get(uid)?.clone())))
                .filter(|(key, id)| state.errands.get(key) != Some(id))
                .collect()
        };
        if new(&HealthState::read(path).map_err(Problem::Unsound)?).is_empty() {
            return Ok(());
        }
        HealthState::update(path, now, |state| {
            for (key, id) in new(state) {
                state.errands.insert(key, id);
            }
        })
    }

    /// The doses' record as written, or why it cannot be trusted. Never an
    /// empty record in place of one that does not read: a dose taken would
    /// look not taken, and written back empty, the sharing would take every
    /// mark out on your other devices too. Of no bytes at all, or gone,
    /// though written before (`witnessed`): lost (a crash after a write that
    /// had not reached the disk, a full disk, a state folder wiped).
    pub fn read(path: &Path) -> Result<HealthState, Unsound> {
        match std::fs::read_to_string(path) {
            Ok(text) if text.trim().is_empty() && witnessed(path) => Err(Unsound::Lost),
            Ok(text) => toml::from_str(&text).map_err(|_| Unsound::Unreadable),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if witnessed(path) {
                    Err(Unsound::Lost)
                } else {
                    Ok(HealthState::default())
                }
            }
            Err(_) => Err(Unsound::Unreadable),
        }
    }

    /// The record changed by `change`, one writer at a time (the window, the
    /// sharing: `filelock`): read again, changed, written, under the lock, so
    /// that no change made meanwhile is written over.
    pub fn update<T>(path: &Path, now: i64, change: impl FnOnce(&mut HealthState) -> T) -> Result<T, Problem> {
        crate::filelock::with_lock(path, || {
            let mut state = HealthState::read(path).map_err(Problem::Unsound)?;
            let out = change(&mut state);
            state.save(path, now).map_err(Problem::Write)?;
            Ok(out)
        })
    }

    /// A record that cannot be trusted, kept aside as `<name>.unreadable-<time>`
    /// for whoever wants to look, with its witness: a new one can start. Only
    /// once nothing it held can be taken out elsewhere (`share::rebuild`).
    pub fn set_aside(path: &Path, now: i64) -> Result<Option<PathBuf>, String> {
        crate::filelock::with_lock(path, || {
            for witness in witnesses(path) {
                let _ = std::fs::remove_file(witness);
            }
            if !path.exists() {
                return Ok(None);
            }
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let aside = path.with_file_name(format!("{name}.unreadable-{now}"));
            std::fs::rename(path, &aside).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(Some(aside))
        })
    }

    /// Saved, its marks older than a week dropped. Use `update`: it holds the lock.
    pub fn save(&mut self, path: &Path, now: i64) -> Result<(), String> {
        let week = now - 7 * 86_400;
        self.taken.retain(|_, at| *at >= week);
        self.reminded.retain(|_, at| *at >= week);
        self.unshown.retain(|_, at| *at >= week);
        self.not_taken.retain(|_, at| *at >= week);
        self.moved.retain(|key, _| self.taken.contains_key(key));
        self.back.retain(|_, [_, _, at]| *at >= week);
        write_witnessed(path, toml::to_string(self).map_err(|e| e.to_string())?.as_bytes())
    }

    /// The doses to remind now: due in the last `minutes`, not taken, not reminded yet.
    pub fn to_remind(&self, health: &Health, now: &Zoned, minutes: i64) -> Vec<Dose> {
        let start = now.checked_sub(Span::new().minutes(minutes)).unwrap_or_else(|_| now.clone());
        let end = now.checked_add(Span::new().seconds(1)).unwrap_or_else(|_| now.clone());
        health.doses(&start, &end).into_iter().filter(|d| !self.taken.contains_key(&d.key) && !self.reminded.contains_key(&d.key)).collect()
    }

    /// The doses of the last `hours`, past their time by `grace` minutes,
    /// neither marked nor reminded: due while Sioul ran nowhere, or while it
    /// ran but could not show their reminder (`unshown`, `reminder_unshown`).
    /// Asked about afterwards, as a question on the past; never reminded to
    /// take now.
    pub fn unanswered(&self, health: &Health, now: &Zoned, hours: i64, grace: i64) -> Vec<Dose> {
        let start = now.checked_sub(Span::new().hours(hours)).unwrap_or_else(|_| now.clone());
        let end = now.checked_sub(Span::new().minutes(grace)).unwrap_or_else(|_| now.clone());
        health.doses(&start, &end).into_iter().filter(|d| !self.taken.contains_key(&d.key) && !self.reminded.contains_key(&d.key) && !self.not_taken.contains_key(&d.key)).collect()
    }

    /// Whether Sioul ran when `key` fell due but did not show its reminder,
    /// or did not know it shown (no notification server, no connection, no
    /// answer in time), here or on another device that shares its doses: its
    /// question then says so, never "while Sioul was closed" (which of the
    /// two: `dose_reminders`).
    pub fn reminder_unshown(&self, key: &str) -> bool {
        self.unshown.contains_key(key) && !self.reminded.contains_key(key)
    }

    /// Today's doses from their time on, not marked yet (taken, or said not
    /// taken), for the Porch: reminded or not, since a reminder can go unseen;
    /// until marked, until the day ends, or `hours` after their time (as far
    /// back as a dose is asked about). Those due while Sioul ran nowhere
    /// (`unanswered`) are asked about apart, and left out here.
    pub fn due_today(&self, health: &Health, now: &Zoned, hours: i64, grace: i64) -> Vec<Dose> {
        let midnight = now.date().to_zoned(now.time_zone().clone()).unwrap_or_else(|_| now.clone());
        let back = now.checked_sub(Span::new().hours(hours)).unwrap_or_else(|_| now.clone());
        let start = if back > midnight { back } else { midnight };
        let end = now.checked_add(Span::new().seconds(1)).unwrap_or_else(|_| now.clone());
        let apart: Vec<String> = self.unanswered(health, now, hours, grace).into_iter().map(|d| d.key).collect();
        health.doses(&start, &end).into_iter().filter(|d| !self.taken.contains_key(&d.key) && !self.not_taken.contains_key(&d.key) && !apart.contains(&d.key)).collect()
    }

    /// One more minute of chats today; covered once the day's limit is reached.
    /// Returns whether chats are covered now.
    pub fn chat_minute(&mut self, limit: &ChatLimit, now: &Zoned) -> bool {
        if self.chat_day != Some(now.date()) {
            self.chat_day = Some(now.date());
            self.chat_minutes = 0;
        }
        if self.chats_covered(now) {
            return true;
        }
        self.chat_minutes += 1;
        if limit.enabled && limit.minutes > 0 && self.chat_minutes >= limit.minutes {
            self.chats_locked_until = Some(now.timestamp().as_second() + i64::from(limit.locked_minutes.max(1)) * 60);
            self.chat_minutes = 0;
            return true;
        }
        false
    }

    /// Whether chats are covered now.
    pub fn chats_covered(&self, now: &Zoned) -> bool {
        self.chats_locked_until.is_some_and(|until| until > now.timestamp().as_second())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    #[test]
    fn doses_by_the_day_the_days_and_the_hours() {
        let noon_and_six = Schedule::Day { times: vec!["18:00".into(), "12:00".into()], amounts: BTreeMap::new() };
        let doses = noon_and_six.doses(&at("2026-10-03T13:00[Europe/Paris]"), &at("2026-10-05T00:00[Europe/Paris]"));
        let shown: Vec<String> = doses.iter().map(|z| z.strftime("%d %H:%M").to_string()).collect();
        assert_eq!(shown, vec!["03 18:00", "04 12:00", "04 18:00"]);
        // Every other day from tomorrow.
        let other = Schedule::Days { days: 2, time: "08:00".into(), from: "2026-10-04".parse().unwrap() };
        let shown: Vec<String> = other.doses(&at("2026-10-03T09:00[Europe/Paris]"), &at("2026-10-09T00:00[Europe/Paris]")).iter().map(|z| z.strftime("%d").to_string()).collect();
        assert_eq!(shown, vec!["04", "06", "08"]);
        // Every six hours from 18:30.
        let six = Schedule::Hours { hours: 6, from: at("2026-10-03T18:30[Europe/Paris]").timestamp().as_second() };
        let shown: Vec<String> = six.doses(&at("2026-10-04T00:00[Europe/Paris]"), &at("2026-10-04T13:00[Europe/Paris]")).iter().map(|z| z.strftime("%H:%M").to_string()).collect();
        assert_eq!(shown, vec!["00:30", "06:30", "12:30"]);
    }

    #[test]
    fn every_few_hours_the_hours_between_doses_are_kept() {
        let eight = || Health {
            medicines: vec![Medicine { id: "antibiotic".into(), name: "Antibiotic".into(), dose: String::new(), schedule: Schedule::Hours { hours: 8, from: at("2026-10-05T08:00[Europe/Paris]").timestamp().as_second() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None }],
            ..Health::default()
        };
        let times = |health: &Health| -> Vec<String> { health.doses(&at("2026-10-05T12:00[Europe/Paris]"), &at("2026-10-06T12:00[Europe/Paris]")).iter().map(|d| d.at.strftime("%H:%M").to_string()).collect() };
        let key = format!("antibiotic@{}", at("2026-10-05T08:00[Europe/Paris]").timestamp().as_second());
        // Taken an hour and a half late: the next ones too, always.
        let mut health = eight();
        let (before, after) = health.taken_at(&key, at("2026-10-05T09:30[Europe/Paris]").timestamp().as_second()).unwrap();
        assert_eq!(times(&health), ["17:30", "01:30", "09:30"]);
        // The mark taken back: where they were.
        assert!(health.taken_back(&key, before, after));
        assert_eq!(times(&health), ["16:00", "00:00", "08:00"]);
        // Early: the next ones earlier, never closer than eight hours after it.
        health.taken_at(&key, at("2026-10-05T07:00[Europe/Paris]").timestamp().as_second()).unwrap();
        assert_eq!(times(&health), ["15:00", "23:00", "07:00"]);
        // Moved again since: a mark taken back later leaves them.
        let mut health = eight();
        let (before, after) = health.taken_at(&key, at("2026-10-05T09:30[Europe/Paris]").timestamp().as_second()).unwrap();
        let later = format!("antibiotic@{}", at("2026-10-05T17:30[Europe/Paris]").timestamp().as_second());
        health.taken_at(&later, at("2026-10-05T18:00[Europe/Paris]").timestamp().as_second()).unwrap();
        assert!(!health.taken_back(&key, before, after));
        assert_eq!(times(&health), ["02:00", "10:00"]);
        // On time to the second: nothing to move.
        assert_eq!(eight().taken_at(&key, at("2026-10-05T08:00[Europe/Paris]").timestamp().as_second()), None);
        // Medicines at set times keep them: being ready for something.
        let mut daily = Health { medicines: vec![Medicine { schedule: Schedule::Day { times: vec!["08:00".into()], amounts: BTreeMap::new() }, ..eight().medicines[0].clone() }], ..Health::default() };
        assert_eq!(daily.taken_at(&key, at("2026-10-05T09:30[Europe/Paris]").timestamp().as_second()), None);
    }

    #[test]
    fn a_record_that_cannot_be_trusted_is_never_taken_for_empty() {
        let dir = std::env::temp_dir().join(format!("sioul-health-record-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health-state.toml");
        // Never written: an empty record, honestly.
        assert_eq!(HealthState::read(&path), Ok(HealthState::default()));
        HealthState::update(&path, 1_000, |s| s.taken.insert("d@1".into(), 1_000)).unwrap();
        assert!(HealthState::read(&path).unwrap().taken.contains_key("d@1"));
        // Broken: not empty, unreadable; and nothing is written over it.
        std::fs::write(&path, "taken = { \"d@1\" = ").unwrap();
        assert_eq!(HealthState::read(&path), Err(Unsound::Unreadable));
        assert_eq!(HealthState::update(&path, 1_060, |s| s.taken.clear()), Err(Problem::Unsound(Unsound::Unreadable)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "taken = { \"d@1\" = ", "left as it was");
        // Gone after being written: lost, not new.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(HealthState::read(&path), Err(Unsound::Lost));
        // Set aside (here, nothing left to keep), a new record can start.
        assert_eq!(HealthState::set_aside(&path, 1_120), Ok(None));
        assert_eq!(HealthState::read(&path), Ok(HealthState::default()));
        // A broken one is kept aside for whoever wants to look.
        std::fs::write(&path, "nonsense = [").unwrap();
        let aside = HealthState::set_aside(&path, 1_180).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), "nonsense = [");
        assert_eq!(HealthState::read(&path), Ok(HealthState::default()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An older Sioul, which writes no entry in the devices' registry, is known
    /// as its claims say, as before: heard in full after the dose and lately,
    /// or its last word, read in full, that it closed, its news quick. Never
    /// closed for want of a word.
    #[test]
    fn an_older_sioul_is_known_as_its_claims_say() {
        let due = 1_800_000_000;
        let peer = |known_until: i64, closed: bool, delay: Option<i64>| Peer { name: "laptop".into(), id: "laptop-id".into(), known_until, closed, delay, broken: None, heard: known_until.max(due - 3_600), ..Peer::default() };
        let laptop = Named { id: "laptop-id".into(), name: "laptop".into(), phone: false };
        // Alone, with a record that reads: known.
        assert!(doubts(due, due + 60, None, &[]).is_empty());
        // Heard in full after the dose was due, a minute ago: known.
        assert!(doubts(due, due + 180, None, &[peer(due + 120, false, Some(30))]).is_empty());
        // Last heard before the dose, open: it may have marked it.
        assert_eq!(doubts(due, due + 600, None, &[peer(due - 300, false, Some(30))]), vec![Doubt::Unheard { device: laptop.clone(), until: due - 300, closed: false }]);
        // Closed before, and its news comes within minutes: known.
        assert!(doubts(due, due + 600, None, &[peer(due - 3_600, true, Some(60))]).is_empty());
        // Closed before, its news slow (a phone's sync) or never measured: not known, as before.
        assert_eq!(doubts(due, due + 600, None, &[peer(due - 3_600, true, Some(1_800))]).len(), 1);
        assert_eq!(doubts(due, due + 600, None, &[peer(due - 3_600, true, None)]).len(), 1);
        // Never said how far it wrote: not known.
        assert_eq!(doubts(due, due + 600, None, &[Peer { name: "phone".into(), heard: due, ..Peer::default() }]).len(), 1);
        // A line of it lost around the dose's time: not known, whatever else.
        let broken = Peer { broken: Some(due - 60), ..peer(due + 120, false, Some(30)) };
        assert_eq!(doubts(due, due + 180, None, &[broken]), vec![Doubt::Broken { device: laptop.clone() }]);
        // Silent for over a week: no longer counted (`SILENT_DAYS`).
        let gone = Peer { heard: due - 8 * 86_400, ..peer(0, false, None) };
        assert!(doubts(due, due + 600, None, &[gone]).is_empty());
        // This device's record was found broken after the dose was due: not known; doses due after are.
        assert_eq!(doubts(due, due + 600, Some(due + 300), &[]), vec![Doubt::Record { since: due + 300 }]);
        assert!(doubts(due + 900, due + 1_000, Some(due + 300), &[]).is_empty());
        // Heard in full just after the dose, not since (its sync slow, or stopped): known then, not an hour later.
        let then = peer(due + 60, false, Some(40));
        assert!(doubts_now(due, due + 120, None, std::slice::from_ref(&then)).is_empty());
        assert_eq!(doubts_now(due, due + 3_600, None, std::slice::from_ref(&then)), vec![Doubt::Unheard { device: laptop.clone(), until: due + 60, closed: false }]);
        // Closed, then spoke again, what it wrote not read here yet: it may have marked it.
        let back = Peer { heard: due + 3_000, ..peer(due - 600, true, Some(60)) };
        assert_eq!(doubts_now(due, due + 3_600, None, &[back]), vec![Doubt::Unheard { device: laptop, until: due - 600, closed: false }]);
    }

    /// The dose of the scenarios below: 08:00, Unix seconds.
    const DUE: i64 = 1_800_000_000;

    /// The phone, as this device knows it: its entry in the registry (a phone
    /// sharing its doses), seen changing here at `seen`, all it wrote read.
    fn phone(said: Said, seen: i64) -> Peer {
        Peer { name: "phone".into(), id: "phone-id".into(), heard: said.exported, said: Some(Said { phone: true, doses: true, ..said }), complete: true, seen, ..Peer::default() }
    }

    fn the_phone() -> Named {
        Named { id: "phone-id".into(), name: "phone".into(), phone: true }
    }

    #[test]
    fn the_phone_closed_cleanly_before_the_dose_is_known() {
        // Put away at 07:00, after its last export, read here in full; not started since.
        let p = phone(Said { started: DUE - 7_200, closed: DUE - 3_600, working: false, exported: DUE - 3_605, imported: DUE - 3_605, ..Said::default() }, DUE - 3_500);
        // At the dose, an hour later, half a day later: known not taken, with no wait for news.
        for now in [DUE, DUE + 3_600, DUE + 12 * 3_600] {
            assert!(doubts_now(DUE, now, None, std::slice::from_ref(&p)).is_empty(), "{now}");
        }
        assert_eq!(known_until(DUE, DUE + 60, DUE + 86_400, None, &[p]), DUE + 86_400, "nothing ends it but the day");
    }

    /// F21 of the review of 5 October 2026: a device closed before the dose
    /// may be picked up at its time to mark it, its sync slow to wake; how
    /// late the news of its opening comes here is measured on its last
    /// openings (`Peer::wake`): known closed only once that while went by
    /// after the dose, and `SKEW` more (R8 of the review of 10 October 2026);
    /// closed after the dose, at once; none measured, as before.
    #[test]
    fn a_device_closed_before_the_dose_is_known_once_its_opening_could_have_come() {
        let closed = phone(Said { started: DUE - 7_200, closed: DUE - 3_600, working: false, exported: DUE - 3_605, ..Said::default() }, DUE - 3_500);
        let doubt = vec![Doubt::Unheard { device: the_phone(), until: DUE - 3_600, closed: true }];
        assert!(doubts_now(DUE, DUE + 10, None, std::slice::from_ref(&closed)).is_empty(), "none measured: as before");
        // Its openings came here within 40 s lately: known 40 s and `SKEW` after the dose, not before.
        let quick = Peer { wake: Some(40), ..closed.clone() };
        assert_eq!(doubts_now(DUE, DUE + 10, None, std::slice::from_ref(&quick)), doubt);
        assert_eq!(doubts_now(DUE, DUE + 40 + SKEW - 1, None, std::slice::from_ref(&quick)), doubt);
        assert!(doubts_now(DUE, DUE + 40 + SKEW, None, std::slice::from_ref(&quick)).is_empty());
        // A phone whose sync woke four minutes late once: four minutes, and `SKEW`.
        let slow = Peer { wake: Some(240), ..closed.clone() };
        assert_eq!(doubts_now(DUE, DUE + 240, None, std::slice::from_ref(&slow)), doubt);
        assert!(doubts_now(DUE, DUE + 240 + SKEW, None, std::slice::from_ref(&slow)).is_empty());
        assert_eq!(known_from(DUE, DUE - 600, DUE + 86_400, None, std::slice::from_ref(&slow)), Some(DUE + 240 + SKEW), "a dose seen ahead: known from then");
        // Then its own alarm opens it (`ALARM_AFTER`): unknown again until the news of that opening had the time to come.
        assert_eq!(known_until(DUE, DUE + 240 + SKEW, DUE + 86_400, None, std::slice::from_ref(&slow)), DUE + ALARM_AFTER);
        assert_eq!(known_from(DUE, DUE + ALARM_AFTER, DUE + 86_400, None, std::slice::from_ref(&slow)), Some(DUE + ALARM_AFTER + 240 + SKEW));
        // Closed after the dose, all it wrote read: its last export holds its answer, at once.
        let after = Peer { wake: Some(240), ..phone(Said { started: DUE - 600, closed: DUE + 600, working: false, exported: DUE + 595, ..Said::default() }, DUE + 620) };
        assert!(doubts_now(DUE, DUE + 630, None, &[after]).is_empty());
        // An older Sioul known by its claims, closed before, its steady news quick: the same wait.
        let older = Peer { name: "laptop".into(), id: "laptop-id".into(), known_until: DUE - 3_600, closed: true, delay: Some(30), heard: DUE - 3_600, wake: Some(120), ..Peer::default() };
        assert_eq!(doubts_now(DUE, DUE + 60, None, std::slice::from_ref(&older)).len(), 1);
        assert!(doubts_now(DUE, DUE + 120 + SKEW, None, std::slice::from_ref(&older)).is_empty());
        // Not sharing its doses, closed before: the same wait, then known.
        let apart = Peer { said: slow.said.clone().map(|s| Said { doses: false, ..s }), ..slow };
        assert_eq!(doubts_now(DUE, DUE + 60, None, std::slice::from_ref(&apart)), doubt);
        assert!(doubts_now(DUE, DUE + 240 + SKEW, None, &[apart]).is_empty());
    }

    /// A phone put away before the dose is opened by its own alarm when a
    /// computer keeps the doses, `ALARM_AFTER` after the dose: it may mark the
    /// dose then, or remind it. Known closed from the dose's time until then
    /// (once the news of an opening at the dose had the time to come), then
    /// known again only once the news of its alarm's opening had the time to
    /// come: never from the dose's time (review of battery part C, H2: the
    /// computer, a minute after the phone's alarm, took the phone as known
    /// and reminded the dose plainly a second time). None measured: as before.
    #[test]
    fn a_phone_put_away_is_known_from_its_alarm_once_that_rang() {
        let put_away = Peer { wake: Some(30), ..phone(Said { started: DUE - 7_200, closed: DUE - 3_600, working: false, exported: DUE - 3_600, imported: DUE - 3_600, ..Said::default() }, DUE - 3_600) };
        let doubt = vec![Doubt::Unheard { device: the_phone(), until: DUE - 3_600, closed: true }];
        let at = |now: i64| doubts_now(DUE, now, None, std::slice::from_ref(&put_away));
        assert_eq!(at(DUE + 60), doubt, "picked up at the dose: its news not come yet");
        assert!(at(DUE + 30 + SKEW).is_empty(), "known before its alarm");
        assert_eq!(at(DUE + ALARM_AFTER), doubt, "its alarm rang");
        assert_eq!(at(DUE + ALARM_AFTER + 60), doubt, "a minute after its alarm: the review's moment");
        assert!(at(DUE + ALARM_AFTER + 30 + SKEW).is_empty(), "the news of its alarm's opening had the time to come");
        assert_eq!(known_until(DUE, DUE + 200, DUE + 86_400, None, std::slice::from_ref(&put_away)), DUE + ALARM_AFTER);
        assert_eq!(known_from(DUE, DUE + ALARM_AFTER, DUE + 86_400, None, std::slice::from_ref(&put_away)), Some(DUE + ALARM_AFTER + 30 + SKEW));
        // A computer put away has no alarm: as before.
        let laptop = Peer { name: "laptop".into(), id: "laptop-id".into(), said: put_away.said.clone().map(|s| Said { phone: false, ..s }), ..put_away.clone() };
        assert!(doubts_now(DUE, DUE + ALARM_AFTER + 60, None, &[laptop]).is_empty());
        // None measured yet: at once, its alarm or not.
        let unmeasured = Peer { wake: None, ..put_away };
        assert!(doubts_now(DUE, DUE + ALARM_AFTER + 1, None, &[unmeasured]).is_empty());
    }

    #[test]
    fn the_phone_closed_cleanly_after_the_dose_without_marking_is_known() {
        // In use from 07:50, closed at 08:20 after its last export (08:19:55), nothing marked there.
        let p = phone(Said { started: DUE - 600, closed: DUE + 1_200, working: false, exported: DUE + 1_195, ..Said::default() }, DUE + 1_260);
        assert!(doubts_now(DUE, DUE + 1_300, None, std::slice::from_ref(&p)).is_empty());
        // Its entry here, but its records not all read yet (a sync that brought one file first): it may hold the answer.
        let behind = Peer { complete: false, ..p };
        assert_eq!(doubts_now(DUE, DUE + 1_300, None, &[behind]), vec![Doubt::Coming { device: the_phone(), shared: DUE + 1_195 }]);
    }

    /// No news of a device read since a phone's alarm began (its pull failed,
    /// its sync app brought nothing of that device): known closed by what it
    /// said before, it is said in doubt, as last heard, never known; its own
    /// entry come since, it is as it says; one in use and fresh stays as
    /// `FRESH` has it; a doubt said already is not said twice.
    #[test]
    fn with_no_news_since_the_alarm_a_device_known_closed_is_a_doubt() {
        let closed = phone(Said { started: DUE - 7_200, closed: DUE - 3_600, working: false, exported: DUE - 3_605, imported: DUE - 3_605, ..Said::default() }, DUE - 3_500);
        assert!(doubts_now(DUE, DUE + 60, None, std::slice::from_ref(&closed)).is_empty(), "closed before the dose, its news read: known");
        let none = std::collections::BTreeSet::new();
        let unread = doubts_unread(DUE, DUE + 60, None, std::slice::from_ref(&closed), DUE + 30, &none);
        assert_eq!(unread, vec![Doubt::Unheard { device: the_phone(), until: DUE - 3_600, closed: true }], "nothing of it read since the alarm: it may have opened since");
        let heard: std::collections::BTreeSet<String> = ["phone-id".to_string()].into();
        assert!(doubts_unread(DUE, DUE + 60, None, std::slice::from_ref(&closed), DUE + 30, &heard).is_empty(), "its own entry came since the alarm: as it says");
        let working = phone(Said { started: DUE - 600, closed: DUE - 7_200, working: true, exported: DUE + 180, ..Said::default() }, DUE + 200);
        assert_eq!(doubts_unread(DUE, DUE + 240, None, std::slice::from_ref(&working), DUE + 230, &none), doubts_now(DUE, DUE + 240, None, std::slice::from_ref(&working)), "in use: as FRESH has it");
        let stale = phone(Said { started: DUE - 3_600, working: true, exported: DUE - 900, ..Said::default() }, DUE - 870);
        assert_eq!(doubts_unread(DUE, DUE + 60, None, std::slice::from_ref(&stale), DUE + 30, &none).len(), 1, "doubted already: once");
    }

    #[test]
    fn the_phone_in_use_and_exported_after_the_dose_is_known() {
        // In use since 07:50, sharing each minute: its 08:03 export seen here at 08:03:20, nothing marked.
        let p = phone(Said { started: DUE - 600, closed: DUE - 7_200, working: true, exported: DUE + 180, ..Said::default() }, DUE + 200);
        assert!(doubts_now(DUE, DUE + 240, None, std::slice::from_ref(&p)).is_empty());
        // Known while that export is fresh: five minutes on with nothing newer, no longer.
        assert_eq!(known_until(DUE, DUE + 240, DUE + 86_400, None, std::slice::from_ref(&p)), DUE + 200 + FRESH + 1);
        assert_eq!(doubts_now(DUE, DUE + 200 + FRESH + 1, None, &[p]), vec![Doubt::Working { device: the_phone(), shared: DUE + 180, quiet: false }]);
    }

    #[test]
    fn the_phone_in_use_not_exported_since_the_dose_is_uncertain_naming_it() {
        // In use; its last export at 07:45, seen at 07:45:30; the dose at 08:00.
        let p = phone(Said { started: DUE - 3_600, closed: DUE - 7_200, working: true, exported: DUE - 900, ..Said::default() }, DUE - 870);
        assert_eq!(doubts_now(DUE, DUE + 60, None, std::slice::from_ref(&p)), vec![Doubt::Working { device: the_phone(), shared: DUE - 900, quiet: false }]);
        let said = p.said.clone().unwrap();
        // An export dated 08:01, within the margin for clocks: not after the dose for sure.
        let early = phone(Said { exported: DUE + 60, ..said.clone() }, DUE + 70);
        assert_eq!(doubts_now(DUE, DUE + 80, None, &[early]).len(), 1);
        // Its 08:03 export comes: known; the doubt lifted by itself.
        let later = phone(Said { exported: DUE + 180, ..said }, DUE + 190);
        assert!(doubts_now(DUE, DUE + 200, None, &[later]).is_empty());
    }

    #[test]
    fn the_phone_that_crashed_is_uncertain_until_it_starts_again_or_is_said_off() {
        // In use from 07:30, last export at 07:45, then nothing: working still up, no close after its start.
        let crashed = phone(Said { started: DUE - 1_800, closed: DUE - 86_400, working: true, exported: DUE - 900, ..Said::default() }, DUE - 880);
        assert_eq!(doubts_now(DUE, DUE + 600, None, std::slice::from_ref(&crashed)), vec![Doubt::Working { device: the_phone(), shared: DUE - 900, quiet: false }]);
        // Hours later, still nothing: it may have stopped without closing, said so; never known.
        assert_eq!(doubts_now(DUE, DUE + 4 * 3_600, None, std::slice::from_ref(&crashed)), vec![Doubt::Working { device: the_phone(), shared: DUE - 900, quiet: true }]);
        // You say it is off: not counted; the dose known not taken, never answered for you.
        let off = Peer { off: true, ..crashed };
        assert!(doubts_now(DUE, DUE + 4 * 3_600, None, &[off]).is_empty());
        // It starts again, in use, exporting: counted again, and known once it exported after the dose.
        let back = phone(Said { started: DUE + 4 * 3_600, closed: DUE - 86_400, working: true, exported: DUE + 4 * 3_600 + 60, ..Said::default() }, DUE + 4 * 3_600 + 70);
        assert!(doubts_now(DUE, DUE + 4 * 3_600 + 90, None, &[back]).is_empty());
    }

    #[test]
    fn clocks_a_few_minutes_apart_either_way() {
        // The phone's clock 3 minutes ahead, as seen here (`ahead`). Its export at 07:59 real
        // time is dated 08:02: not after the dose for sure.
        let early = Peer { ahead: 180, ..phone(Said { started: DUE - 3_600, working: true, exported: DUE + 120, ..Said::default() }, DUE - 60) };
        assert_eq!(doubts_now(DUE, DUE, None, &[early]).len(), 1, "its 08:02 is 07:59 here");
        // Its export at 08:03 real time, dated 08:06: past the dose and the margin; known.
        let later = Peer { ahead: 180, ..phone(Said { started: DUE - 3_600, working: true, exported: DUE + 360, ..Said::default() }, DUE + 190) };
        assert!(doubts_now(DUE, DUE + 200, None, &[later]).is_empty());
        // Its clock 3 minutes behind: the export at 08:04 real time is dated 08:01: Sioul waits longer, never less.
        let behind = phone(Said { started: DUE - 3_600, working: true, exported: DUE + 60, ..Said::default() }, DUE + 250);
        assert_eq!(doubts_now(DUE, DUE + 260, None, &[behind]).len(), 1);
        let behind_later = phone(Said { started: DUE - 3_600, working: true, exported: DUE + 300, ..Said::default() }, DUE + 490);
        assert!(doubts_now(DUE, DUE + 500, None, &[behind_later]).is_empty());
        // Closed, whatever its clock: started and closed are both on its own clock.
        let closed = Peer { ahead: 600, ..phone(Said { started: DUE + 900, closed: DUE + 1_500, working: false, exported: DUE + 1_490, ..Said::default() }, DUE + 300) };
        assert!(doubts_now(DUE, DUE + 400, None, &[closed]).is_empty());
    }

    #[test]
    fn a_device_silent_for_a_week_no_longer_counts() {
        // Stopped without closing eight days ago, by its clock and as seen here: not counted.
        let gone = phone(Said { started: DUE - 10 * 86_400, working: true, exported: DUE - 8 * 86_400, ..Said::default() }, DUE - 8 * 86_400 + 30);
        assert!(doubts_now(DUE, DUE + 60, None, &[gone.clone()]).is_empty());
        // Six days: still counted, its doubt said.
        let six = phone(Said { started: DUE - 7 * 86_400, working: true, exported: DUE - 6 * 86_400, ..Said::default() }, DUE - 6 * 86_400 + 30);
        assert_eq!(doubts_now(DUE, DUE + 60, None, &[six]).len(), 1);
        // First seen here today (this device just joined), silent a month by its own date: counted a
        // week from now, never dropped for a date on another clock.
        let first = Peer { seen: DUE - 60, ..gone };
        assert_eq!(doubts_now(DUE, DUE + 60, None, std::slice::from_ref(&first)).len(), 1);
        assert!(!counts(&first, DUE + 8 * 86_400));
        // A device that stopped sharing counts no more.
        let left = phone(Said { started: DUE - 600, working: false, closed: DUE - 60, exported: DUE - 61, left: true, ..Said::default() }, DUE - 30);
        assert!(!counts(&left, DUE));
    }

    #[test]
    fn a_device_that_does_not_share_its_doses_is_known_only_closed_before() {
        let tablet = |said: Said| Peer { name: "tablet".into(), id: "tablet-id".into(), said: Some(Said { doses: false, ..said }), complete: true, seen: DUE + 310, ..Peer::default() };
        let named = Named { id: "tablet-id".into(), name: "tablet".into(), phone: false };
        // Health switched off there: what it marks never comes. In use: never known.
        let working = tablet(Said { started: DUE - 600, working: true, exported: DUE + 300, ..Said::default() });
        assert_eq!(doubts_now(DUE, DUE + 320, None, &[working]), vec![Doubt::Apart { device: named }]);
        // Closed before the dose: it could not have marked it.
        assert!(doubts_now(DUE, DUE + 320, None, &[tablet(Said { started: DUE - 7_200, closed: DUE - 3_600, exported: DUE - 3_605, ..Said::default() })]).is_empty());
        // Closed after it: it may have.
        assert_eq!(doubts_now(DUE, DUE + 700, None, &[tablet(Said { started: DUE - 600, closed: DUE + 600, exported: DUE + 595, ..Said::default() })]).len(), 1);
    }

    #[test]
    fn each_device_said_apart() {
        // The desktop closed, the phone in use and behind, an older laptop never heard in full: two doubts, each named.
        let desktop = Peer { name: "desk".into(), id: "desk-id".into(), said: Some(Said { started: DUE - 9_000, closed: DUE - 3_600, exported: DUE - 3_601, doses: true, ..Said::default() }), complete: true, seen: DUE - 3_500, ..Peer::default() };
        let p = phone(Said { started: DUE - 3_600, working: true, exported: DUE - 900, ..Said::default() }, DUE - 880);
        let older = Peer { name: "laptop".into(), id: "laptop-id".into(), heard: DUE - 60, ..Peer::default() };
        let doubts = doubts_now(DUE, DUE + 60, None, &[desktop, p, older]);
        assert_eq!(doubts.len(), 2, "{doubts:?}");
        assert!(matches!(&doubts[0], Doubt::Working { device, .. } if device.id == "phone-id"));
        assert!(matches!(&doubts[1], Doubt::Unheard { device, until: 0, .. } if device.id == "laptop-id"));
    }

    #[test]
    fn reminded_once_never_counted() {
        let health = Health {
            medicines: vec![Medicine { id: "vitamin-d".into(), name: "Vitamin D".into(), dose: "1000 IU".into(), schedule: Schedule::Day { times: vec!["12:00".into()], amounts: BTreeMap::new() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None }],
            ..Health::default()
        };
        let mut state = HealthState::default();
        let now = at("2026-10-03T12:03[Europe/Paris]");
        let due = state.to_remind(&health, &now, 10);
        assert_eq!(due.len(), 1);
        state.reminded.insert(due[0].key.clone(), now.timestamp().as_second());
        assert!(state.to_remind(&health, &now, 10).is_empty(), "reminded once");
        assert!(state.to_remind(&health, &at("2026-10-03T13:00[Europe/Paris]"), 10).is_empty(), "past its window, no second reminder");
    }

    /// A dose held by sleep (due 05:00, waking 07:00): what looks back counts
    /// from the dose's time, the opening for it from waking (the sixth
    /// review's M1, R4): a record lost at 06:00 and a device apart closed at
    /// 06:00 leave it not known, as at its time; a phone put away before the
    /// sleep is known only once the news of an opening at waking had the time to come.
    #[test]
    fn at_waking_what_looks_back_counts_from_the_doses_time() {
        let (due, waking) = (1_800_018_000, 1_800_025_200);
        let now = waking + 30;
        // This device's record found broken at 06:00, the dose due before it.
        assert_eq!(doubts_at(due, waking, now, Some(due + 3_600), &[]), [Doubt::Record { since: due + 3_600 }]);
        assert_eq!(doubts_at(waking, waking, now, Some(due + 3_600), &[]), [], "counted from waking, it would be lost");
        // A computer that does not share its doses, open at 05:00, closed at 06:00.
        let apart = Peer {
            name: "desk".into(),
            id: "desk-id".into(),
            said: Some(Said { started: due - 600, closed: due + 3_600, working: false, exported: due + 3_600, imported: due + 3_600, doses: false, left: false, phone: false }),
            complete: true,
            seen: due + 3_600,
            heard: due + 3_600,
            ..Peer::default()
        };
        assert!(matches!(&doubts_at(due, waking, now, None, std::slice::from_ref(&apart))[..], [Doubt::Apart { .. }]));
        // A phone put away at 22:30, its openings' news within 30 s: known from waking + 30 s + SKEW, not from its alarm at 05:10.
        let phone = Peer {
            name: "phone".into(),
            id: "phone-id".into(),
            said: Some(Said { started: due - 30_000, closed: due - 23_400, working: false, exported: due - 23_400, imported: due - 23_400, doses: true, left: false, phone: true }),
            complete: true,
            seen: due - 23_400,
            heard: due - 23_400,
            wake: Some(30),
            ..Peer::default()
        };
        assert!(!doubts_at(due, waking, now, None, std::slice::from_ref(&phone)).is_empty());
        assert!(doubts_at(due, waking, waking + 30 + SKEW, None, std::slice::from_ref(&phone)).is_empty());
        assert!(doubts_now(due, now, None, std::slice::from_ref(&phone)).is_empty(), "from the dose's time, it would be known by its alarm long rung");
    }

    /// A reminder not known shown is written in `unshown` for an older Sioul
    /// (0.0.4, 0.0.5), which reads it as a dose not reminded: its alarm
    /// reminds it, its question asks about it, never "reminded"; written back
    /// by it, `unshown` stays. 0.0.3, which knows no such table, does not read
    /// it reminded either. What each device did travels in a file of its own
    /// (`dose_reminders`), which no older Sioul rewrites.
    #[test]
    fn an_older_sioul_reads_a_reminder_that_may_have_shown_as_one_to_give() {
        /// The record's tables as 0.0.5 knows them.
        #[derive(Debug, Default, Serialize, Deserialize)]
        struct Older {
            #[serde(default)]
            taken: BTreeMap<String, i64>,
            #[serde(default)]
            reminded: BTreeMap<String, i64>,
            #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
            unshown: BTreeMap<String, i64>,
            #[serde(default)]
            not_taken: BTreeMap<String, i64>,
        }
        let key = "levo@1791360000".to_string();
        let mut new = HealthState::default();
        new.unshown.insert(key.clone(), 1_791_360_004);
        let text = toml::to_string(&new).unwrap();
        assert!(text.contains("[unshown]") && !text.contains("maybe"));
        let older: Older = toml::from_str(&text).unwrap();
        assert!(!older.reminded.contains_key(&key) && older.unshown.contains_key(&key), "to remind and to ask about, never reminded");
        let rewritten: HealthState = toml::from_str(&toml::to_string(&older).unwrap()).unwrap();
        assert!(rewritten.unshown.contains_key(&key) && rewritten.reminder_unshown(&key), "its rewrite keeps unshown");
        /// The record's tables as 0.0.3 knows them.
        #[derive(Debug, Default, Deserialize)]
        struct Oldest {
            #[serde(default)]
            reminded: BTreeMap<String, i64>,
        }
        let oldest: Oldest = toml::from_str(&text).unwrap();
        assert!(!oldest.reminded.contains_key(&key), "0.0.3: to remind, never reminded");
        // A file of 6c952fc's, with its `maybe` table: read, the table left out.
        let earlier: HealthState = toml::from_str("[unshown]\n\"levo@1791360000\" = 1791360004\n[maybe]\n\"levo@1791360000\" = 1791360002\n").unwrap();
        assert!(earlier.reminder_unshown(&key) && !toml::to_string(&earlier).unwrap().contains("maybe"));
    }

    /// A reminder that could not be shown (no notification server) is no
    /// reminder: the dose is still asked about afterwards, and told apart from
    /// one due while Sioul was closed; the record keeps it a week, and a file
    /// without it (an older Sioul's) reads and is written as before.
    #[test]
    fn a_reminder_not_shown_is_still_asked_about_and_told_apart() {
        let health = Health {
            medicines: vec![Medicine { id: "demo-pill".into(), name: "Demo pill".into(), dose: String::new(), schedule: Schedule::Day { times: vec!["08:00".into(), "09:00".into()], amounts: BTreeMap::new() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None }],
            ..Health::default()
        };
        let now = at("2026-10-03T12:00[Europe/Paris]");
        let key = |when: &str| format!("demo-pill@{}", at(when).timestamp().as_second());
        let mut state = HealthState::default();
        // 08:00: Sioul ran, its reminder failed; 09:00: Sioul ran nowhere.
        state.unshown.insert(key("2026-10-03T08:00[Europe/Paris]"), at("2026-10-03T08:01[Europe/Paris]").timestamp().as_second());
        let asked: Vec<String> = state.unanswered(&health, &now, 12, 30).into_iter().map(|d| d.key).collect();
        assert_eq!(asked, [key("2026-10-03T08:00[Europe/Paris]"), key("2026-10-03T09:00[Europe/Paris]")], "both still asked about");
        assert!(state.reminder_unshown(&key("2026-10-03T08:00[Europe/Paris]")));
        assert!(!state.reminder_unshown(&key("2026-10-03T09:00[Europe/Paris]")));
        // Reminded later (another device, or a notification server back): a reminder, not asked.
        let mut later = state.clone();
        later.reminded.insert(key("2026-10-03T08:00[Europe/Paris]"), at("2026-10-03T08:05[Europe/Paris]").timestamp().as_second());
        assert!(!later.reminder_unshown(&key("2026-10-03T08:00[Europe/Paris]")));
        assert_eq!(later.unanswered(&health, &now, 12, 30).len(), 1);
        // Sent, not known shown (written in `unshown` too): asked about; a late answer makes it a reminder.
        let mut sent = HealthState::default();
        let nine = key("2026-10-03T09:00[Europe/Paris]");
        sent.unshown.insert(nine.clone(), at("2026-10-03T09:00[Europe/Paris]").timestamp().as_second() + 4);
        assert!(sent.reminder_unshown(&nine));
        assert!(sent.unanswered(&health, &now, 12, 30).iter().any(|d| d.key == nine), "asked about");
        let mut answered_late = sent.clone();
        answered_late.reminded.insert(nine.clone(), at("2026-10-03T09:01[Europe/Paris]").timestamp().as_second());
        assert!(!answered_late.reminder_unshown(&nine));
        // Kept as written, a week; an older Sioul's file has no table, and gets none.
        let dir = std::env::temp_dir().join(format!("sioul-unshown-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("health-state.toml");
        state.save(&path, now.timestamp().as_second()).unwrap();
        let read = HealthState::read(&path).unwrap();
        assert!(read.reminder_unshown(&key("2026-10-03T08:00[Europe/Paris]")));
        let mut old = read.clone();
        old.save(&path, now.timestamp().as_second() + 8 * 86_400).unwrap();
        assert!(HealthState::read(&path).unwrap().unshown.is_empty(), "dropped after a week");
        std::fs::write(&path, "[taken]\n[reminded]\n").unwrap();
        let mut older = HealthState::read(&path).unwrap();
        assert!(older.unshown.is_empty());
        older.save(&path, now.timestamp().as_second()).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("unshown"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pharmacy_and_doctor_errands() {
        let health = Health {
            prescriptions: vec![Prescription {
                id: "vitamin-d".into(),
                title: "Vitamin D 1000 IU".into(),
                until: Some("2026-12-31".parse().unwrap()),
                refill_days: Some(28),
                last_refill: Some("2026-10-01".parse().unwrap()),
                ..Prescription::default()
            }],
            ..Health::default()
        };
        let errands: Vec<(ErrandKind, String)> = health.errands().into_iter().map(|e| (e.kind, e.day.to_string())).collect();
        assert_eq!(errands, vec![(ErrandKind::Refill, "2026-10-27".to_string()), (ErrandKind::Renew, "2026-12-17".to_string())]);
    }

    /// An errand is health wherever it went (docs/mcp.md): known by its
    /// category in either language, by the record's UID alone, or by Sioul's
    /// own title in either language (a Google list keeps no category); a task
    /// of your own about a pharmacy is not one.
    #[test]
    fn an_errand_is_known_by_its_category_its_record_or_its_title() {
        use crate::tasks::Task;
        let titled = |language: &str, id: &str| {
            let mut args = crate::i18n::args();
            args.set("title", "Vitamin D 1000 IU");
            crate::i18n::Translator::new(language).text(id, Some(&args))
        };
        let task = |uid: &str, title: &str, categories: &[&str]| Task { uid: uid.into(), title: title.into(), categories: categories.iter().map(|c| c.to_string()).collect(), ..Task::default() };
        let recorded: std::collections::BTreeSet<String> = ["renamed".to_string()].into();
        for errand in [
            task("fr", "Aller chercher la boîte", &["Santé"]),
            task("en", "Fetch the box", &["health"]),
            task("renamed", "Collect the order", &[]),
            task("google-en", &titled("en", "health-errand-refill"), &[]),
            task("google-fr", &titled("fr", "health-errand-renew"), &[]),
        ] {
            assert!(is_errand(&errand, &recorded), "{errand:?}");
        }
        for own in [task("call", "Call the pharmacy", &[]), task("bread", "Buy bread", &["shopping"]), task("bare", "Pharmacy:", &[])] {
            assert!(!is_errand(&own, &recorded), "{own:?}");
        }
        // The record's UIDs alone: its keys (they name prescriptions) and its marks are not kept.
        let dir = std::env::temp_dir().join(format!("sioul-errand-uids-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health-state.toml");
        std::fs::write(&path, "[taken]\n\"vitamin-d@1790000000\" = 1790000000\n\n[errands]\n\"refill:vitamin-d:2026-10-27\" = \"uid-1\"\n").unwrap();
        assert_eq!(HealthState::errand_uids(&path), Some(["uid-1".to_string()].into()));
        // Its id on a list that keeps no UID, recorded beside it once, read with it (can-wait 10).
        let ids: BTreeMap<String, String> = [("uid-1".to_string(), "google-1".to_string())].into();
        HealthState::note_elsewhere(&path, 1_790_000_000, &ids).unwrap();
        assert_eq!(HealthState::errand_uids(&path), Some(["uid-1".to_string(), "google-1".to_string()].into()));
        let written = std::fs::read(&path).unwrap();
        HealthState::note_elsewhere(&path, 1_790_000_100, &ids).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), written, "nothing new: nothing written");
        // A record that does not read: not known, never none.
        std::fs::write(&path, "not toml [[").unwrap();
        assert_eq!(HealthState::errand_uids(&path), None);
        assert!(HealthState::note_elsewhere(&path, 1_790_000_200, &ids).is_err());
        let names: Vec<String> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert!(names.iter().all(|n| !n.contains("unreadable") && !n.contains("lost")), "nothing set aside: {names:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not toml [[", "nor written over");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(HealthState::errand_uids(&path), None, "lost: written here before");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_as_written_by_hand_and_by_sioul() {
        let by_hand = "[[prescription]]\nid = \"d\"\ntitle = \"Vitamin D\"\nuntil = 2026-12-31\n\n[[medicine]]\nid = \"iron\"\nname = \"Iron\"\n[medicine.schedule]\nevery = \"days\"\ndays = 2\ntime = \"12:00\"\nfrom = 2026-10-03\n";
        let health: Health = toml::from_str(by_hand).unwrap();
        assert_eq!(health.prescriptions[0].until, Some("2026-12-31".parse().unwrap()));
        let again: Health = toml::from_str(&toml::to_string(&health).unwrap()).unwrap();
        assert_eq!(again, health);
    }

    #[test]
    fn the_watch_s_old_keys_are_read_and_left_aside() {
        // As Sioul wrote it while it read a Garmin watch's files (until 8 October 2026).
        let dir = std::env::temp_dir().join(format!("sioul-health-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        let older = "errands_list = \"local/errands\"\nwatch_folder = \"~/Garmin\"\nwatch_offers = false\n\n[[medicine]]\nid = \"iron\"\nname = \"Iron\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\"]\n";
        std::fs::write(&path, older).unwrap();
        let health = Health::load(&path);
        assert_eq!((health.errands_list.as_str(), health.medicines.len()), ("local/errands", 1), "read as before");
        assert!(!dir.join("health.toml.unreadable").exists(), "never taken for a file that does not read");
        health.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("watch"), "{written}");
        assert_eq!(Health::load(&path), health);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_no_longer_reads_is_kept_aside() {
        let dir = std::env::temp_dir().join(format!("sioul-health-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        assert_eq!(Health::load(&path), Health::default(), "none yet");
        let broken = "[[medicine]]\nid = \"iron\"\nname = \"Iron\"\n[medicine.schedule]\nevery = \"fortnight\"\n";
        std::fs::write(&path, broken).unwrap();
        assert_eq!(Health::load(&path), Health::default());
        Health::default().save(&path).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("health.toml.unreadable")).unwrap(), broken, "what was there is kept");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Porch's doses at a desktop: due now, Sioul running (shown, reminded
    /// or not); due earlier today, reminded but not answered (shown, until
    /// marked, twelve hours after, or the day's end); due while Sioul ran
    /// nowhere (asked apart, not here); marked either way (gone).
    #[test]
    fn todays_doses_until_marked() {
        let health = Health {
            medicines: vec![
                Medicine { id: "levo".into(), name: "Levothyroxine".into(), dose: "75 µg".into(), schedule: Schedule::Day { times: vec!["07:30".into()], amounts: BTreeMap::new() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None },
                Medicine { id: "iron".into(), name: "Iron".into(), dose: String::new(), schedule: Schedule::Day { times: vec!["12:00".into(), "21:00".into()], amounts: BTreeMap::new() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None },
                Medicine { id: "zinc".into(), name: "Zinc".into(), dose: String::new(), schedule: Schedule::Day { times: vec!["09:00".into(), "23:50".into()], amounts: BTreeMap::new() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None },
            ],
            ..Health::default()
        };
        let key = |id: &str, when: &str| format!("{id}@{}", at(when).timestamp().as_second());
        let keys = |doses: Vec<Dose>| doses.into_iter().map(|d| d.key).collect::<Vec<_>>();
        let mut state = HealthState::default();
        // 07:30 reminded while Sioul ran, never answered; 09:00 due while Sioul ran nowhere.
        state.reminded.insert(key("levo", "2026-10-06T07:30[Europe/Paris]"), at("2026-10-06T07:31[Europe/Paris]").timestamp().as_second());
        let noon = at("2026-10-06T12:00:30[Europe/Paris]");
        assert_eq!(keys(state.due_today(&health, &noon, 12, 30)), [key("levo", "2026-10-06T07:30[Europe/Paris]"), key("iron", "2026-10-06T12:00[Europe/Paris]")]);
        assert_eq!(keys(state.unanswered(&health, &noon, 12, 30)), [key("zinc", "2026-10-06T09:00[Europe/Paris]")]);
        // Before its time, a dose is not due; at its minute, it is.
        assert_eq!(keys(state.due_today(&health, &at("2026-10-06T11:59[Europe/Paris]"), 12, 30)), [key("levo", "2026-10-06T07:30[Europe/Paris]")]);
        // Within its half hour and not reminded (the reminder waits for news): shown all the same.
        let mut waiting = state.clone();
        waiting.reminded.clear();
        assert_eq!(keys(waiting.due_today(&health, &at("2026-10-06T07:45[Europe/Paris]"), 12, 30)), [key("levo", "2026-10-06T07:30[Europe/Paris]")]);
        // Past its half hour without a reminder anywhere: the question on doses due while closed instead.
        assert!(waiting.due_today(&health, &at("2026-10-06T08:10[Europe/Paris]"), 12, 30).is_empty());
        // Marked taken, or said not taken: gone.
        state.taken.insert(key("iron", "2026-10-06T12:00[Europe/Paris]"), noon.timestamp().as_second());
        state.not_taken.insert(key("zinc", "2026-10-06T09:00[Europe/Paris]"), noon.timestamp().as_second());
        assert_eq!(keys(state.due_today(&health, &noon, 12, 30)), [key("levo", "2026-10-06T07:30[Europe/Paris]")]);
        // Twelve hours after its time: no longer shown; the evening's iron instead.
        let evening = at("2026-10-06T21:05[Europe/Paris]");
        state.reminded.insert(key("iron", "2026-10-06T21:00[Europe/Paris]"), evening.timestamp().as_second());
        assert_eq!(keys(state.due_today(&health, &evening, 12, 30)), [key("iron", "2026-10-06T21:00[Europe/Paris]")]);
        // The day ends: yesterday's are not today's, even within twelve hours.
        let night = at("2026-10-07T00:30[Europe/Paris]");
        assert!(state.due_today(&health, &night, 12, 30).is_empty());
        // Yesterday's 23:50 zinc, reminded nowhere, past its half hour: the question's.
        assert_eq!(keys(state.unanswered(&health, &night, 12, 30)), [key("zinc", "2026-10-06T23:50[Europe/Paris]")]);
    }

    #[test]
    fn chats_covered_after_their_time() {
        let limit = ChatLimit { enabled: true, minutes: 2, locked_minutes: 30 };
        let mut state = HealthState::default();
        let now = at("2026-10-03T20:00[Europe/Paris]");
        assert!(!state.chat_minute(&limit, &now));
        assert!(state.chat_minute(&limit, &now), "the second minute reaches the limit");
        assert!(state.chats_covered(&at("2026-10-03T20:29[Europe/Paris]")));
        assert!(!state.chats_covered(&at("2026-10-03T20:31[Europe/Paris]")), "back after the time chosen");
    }

    /// A medicine as Sioul 0.0.3 reads and writes it, before takes had
    /// amounts of their own: the fields it knows, the others dropped at its
    /// next save.
    mod older {
        use jiff::civil::Date;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Health {
            #[serde(rename = "medicine", default)]
            pub medicines: Vec<Medicine>,
        }

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct Medicine {
            pub id: String,
            pub name: String,
            #[serde(default, skip_serializing_if = "String::is_empty")]
            pub dose: String,
            pub schedule: Schedule,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub prescription: Option<String>,
            #[serde(default, deserialize_with = "crate::budget::dates::optional", skip_serializing_if = "Option::is_none")]
            pub until: Option<Date>,
            #[serde(default, skip_serializing_if = "std::ops::Not::not")]
            pub paused: bool,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "every", rename_all = "kebab-case")]
        pub enum Schedule {
            Day {
                times: Vec<String>,
            },
            Days {
                days: u32,
                time: String,
                #[serde(deserialize_with = "crate::budget::dates::required")]
                from: Date,
            },
            Hours {
                hours: u32,
                from: i64,
            },
        }
    }

    fn take(time: &str, amount: &str) -> Take {
        Take { time: time.into(), amount: amount.into() }
    }

    fn at_set_times(id: &str, name: &str) -> Medicine {
        Medicine { id: id.into(), name: name.into(), dose: String::new(), schedule: Schedule::Day { times: Vec::new(), amounts: BTreeMap::new() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None }
    }

    /// The doses of a day: time, name and amount, in order.
    fn day_of(health: &Health, day: &str) -> Vec<(String, String, String)> {
        let start = at(&format!("{day}T00:00[Europe/Paris]"));
        let end = start.checked_add(Span::new().days(1)).unwrap();
        health.doses(&start, &end).into_iter().map(|d| (d.at.strftime("%H:%M").to_string(), d.name, d.dose)).collect()
    }

    fn said(rows: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
        rows.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect()
    }

    #[test]
    fn each_take_its_time_and_its_own_amount() {
        // The same amount at every take: the medicine's dose, the file as before.
        let mut same = at_set_times("zinc", "Zinc");
        same.set_takes("1 tablet", &[take("20:00", ""), take("8:00", "1 tablet")]).unwrap();
        assert_eq!(same.dose, "1 tablet");
        assert_eq!(same.schedule, Schedule::Day { times: vec!["08:00".into(), "20:00".into()], amounts: BTreeMap::new() });
        assert!(!toml::to_string(&Health { medicines: vec![same.clone()], ..Health::default() }).unwrap().contains("amounts"));
        // 1 tablet at 08:00, 2 at 20:00: each take its own; the form's dose, the most usual.
        let mut iron = at_set_times("iron", "Iron");
        iron.set_takes("1 tablet", &[take("08:00", ""), take("20:00", "2 tablets")]).unwrap();
        assert_eq!(iron.takes(), [take("08:00", "1 tablet"), take("20:00", "2 tablets")]);
        assert_eq!(iron.usual(), "1 tablet");
        assert_eq!(iron.takes_line(), "08:00 · 1 tablet, 20:00 · 2 tablets");
        assert_eq!(iron.dose, iron.takes_line(), "what an older Sioul says with each dose");
        let health = Health { medicines: vec![iron.clone(), same], ..Health::default() };
        assert_eq!(day_of(&health, "2026-10-08"), said(&[("08:00", "Iron", "1 tablet"), ("08:00", "Zinc", "1 tablet"), ("20:00", "Iron", "2 tablets"), ("20:00", "Zinc", "1 tablet")]));
        // The doses fall due as they would with one amount for all: the same keys, the same times.
        let plain = Health { medicines: vec![Medicine { schedule: Schedule::Day { times: vec!["08:00".into(), "20:00".into()], amounts: BTreeMap::new() }, ..iron.clone() }], ..Health::default() };
        let keys = |h: &Health| h.doses(&at("2026-10-08T00:00[Europe/Paris]"), &at("2026-10-10T00:00[Europe/Paris]")).into_iter().filter(|d| d.medicine == "iron").map(|d| (d.key, d.at)).collect::<Vec<_>>();
        assert_eq!(keys(&health), keys(&plain));
        // A take with no amount among others: none said for it, never another's.
        let mut some = at_set_times("d", "Vitamin D");
        some.set_takes("", &[take("08:00", ""), take("20:00", "2 drops")]).unwrap();
        assert_eq!(some.takes(), [take("08:00", ""), take("20:00", "2 drops")]);
        assert_eq!(some.dose, "08:00, 20:00 · 2 drops");
        assert_eq!(some.usual(), "", "the earliest on a tie");
        // What cannot be set: no take, a time that does not read, two takes at one time.
        assert_eq!(at_set_times("x", "X").set_takes("", &[]), Err(TakeProblem::None));
        assert_eq!(at_set_times("x", "X").set_takes("", &[take(" 8h ", "")]), Err(TakeProblem::Unreadable("8h".into())));
        assert_eq!(at_set_times("x", "X").set_takes("", &[take("08:00", "1"), take("8:00", "2")]), Err(TakeProblem::Twice("08:00".into())));
        // Set again alike: the amounts go, the dose is the one amount again.
        iron.set_takes("2 tablets", &[take("08:00", ""), take("20:00", "")]).unwrap();
        assert_eq!((iron.dose.as_str(), iron.takes()), ("2 tablets", vec![take("08:00", "2 tablets"), take("20:00", "2 tablets")]));
    }

    #[test]
    fn a_prescription_with_three_medicines_and_several_takes_each() {
        let mut levo = at_set_times("levo", "Levothyroxine");
        levo.set_takes("75 µg", &[take("07:30", "")]).unwrap();
        let mut iron = at_set_times("iron", "Iron");
        iron.set_takes("1 tablet", &[take("08:00", ""), take("13:00", ""), take("20:00", "2 tablets")]).unwrap();
        let mut magnesium = at_set_times("magnesium", "Magnesium");
        magnesium.set_takes("300 mg", &[take("12:30", "150 mg"), take("21:30", "")]).unwrap();
        let mut health = Health {
            prescriptions: vec![Prescription { id: "dr".into(), title: "Thyroid and iron".into(), refill_days: Some(30), last_refill: Some("2026-10-01".parse().unwrap()), ..Prescription::default() }],
            medicines: vec![levo, iron, magnesium],
            ..Health::default()
        };
        for medicine in &mut health.medicines {
            medicine.prescription = Some("dr".into());
        }
        assert_eq!(
            day_of(&health, "2026-10-08"),
            said(&[("07:30", "Levothyroxine", "75 µg"), ("08:00", "Iron", "1 tablet"), ("12:30", "Magnesium", "150 mg"), ("13:00", "Iron", "1 tablet"), ("20:00", "Iron", "2 tablets"), ("21:30", "Magnesium", "300 mg")])
        );
        // Written and read again: the same.
        let again: Health = toml::from_str(&toml::to_string(&health).unwrap()).unwrap();
        assert_eq!(again, health);
        // One taken out: the others fall due as before.
        health.medicines.retain(|m| m.id != "iron");
        assert_eq!(day_of(&health, "2026-10-08"), said(&[("07:30", "Levothyroxine", "75 µg"), ("12:30", "Magnesium", "150 mg"), ("21:30", "Magnesium", "300 mg")]));
        assert_eq!(health.errands().len(), 1, "its pharmacy, as before");
    }

    #[test]
    fn an_older_file_reads_as_before() {
        // As Sioul 0.0.3 writes a medicine: no amounts.
        let text = "[[medicine]]\nid = \"iron\"\nname = \"Iron\"\ndose = \"1 tablet\"\n\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"20:00\"]\n";
        let health: Health = toml::from_str(text).unwrap();
        assert_eq!(health.medicines[0].takes(), [take("08:00", "1 tablet"), take("20:00", "1 tablet")]);
        assert_eq!(day_of(&health, "2026-10-08"), said(&[("08:00", "Iron", "1 tablet"), ("20:00", "Iron", "1 tablet")]));
        // Written again by this Sioul: what 0.0.3 reads, field for field.
        let written = toml::to_string(&health).unwrap();
        assert!(!written.contains("amounts"), "{written}");
        assert_eq!(toml::from_str::<older::Health>(&written).unwrap(), toml::from_str::<older::Health>(text).unwrap());
    }

    #[test]
    fn an_older_sioul_reads_the_times_and_says_every_amount() {
        let mut iron = at_set_times("iron", "Iron");
        iron.set_takes("1 tablet", &[take("08:00", ""), take("20:00", "2 tablets")]).unwrap();
        let health = Health { medicines: vec![iron], ..Health::default() };
        let written = toml::to_string(&health).unwrap();
        // Sioul 0.0.3 reads the file: the same times, and the whole line as the dose of each.
        let old: older::Health = toml::from_str(&written).unwrap();
        assert_eq!(old.medicines[0].schedule, older::Schedule::Day { times: vec!["08:00".into(), "20:00".into()] });
        assert_eq!(old.medicines[0].dose, "08:00 · 1 tablet, 20:00 · 2 tablets");
        // It saves it again, dropping the amounts: the doses fall due as they did, each saying the whole line.
        let back: Health = toml::from_str(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!(back.medicines[0].schedule, Schedule::Day { times: vec!["08:00".into(), "20:00".into()], amounts: BTreeMap::new() });
        let (start, end) = (at("2026-10-08T00:00[Europe/Paris]"), at("2026-10-10T00:00[Europe/Paris]"));
        let due = |h: &Health| h.doses(&start, &end).into_iter().map(|d| (d.key, d.at)).collect::<Vec<_>>();
        assert_eq!(due(&back), due(&health));
        assert!(back.doses(&start, &end).iter().all(|d| d.dose == "08:00 · 1 tablet, 20:00 · 2 tablets"));
    }

    #[test]
    fn a_take_moved_by_the_change_of_hour_keeps_its_amount() {
        // 02:30 does not exist in Paris on 29 March 2026: the dose comes at 03:30, its amount with it.
        let mut night = at_set_times("night", "Night drops");
        night.set_takes("1 drop", &[take("02:30", "3 drops"), take("08:00", "")]).unwrap();
        let health = Health { medicines: vec![night], ..Health::default() };
        assert_eq!(day_of(&health, "2026-03-29"), said(&[("03:30", "Night drops", "3 drops"), ("08:00", "Night drops", "1 drop")]));
        let zone = jiff::tz::TimeZone::get("Europe/Paris").unwrap();
        let doses = health.doses(&at("2026-03-29T00:00[Europe/Paris]"), &at("2026-03-30T00:00[Europe/Paris]"));
        assert_eq!(health.amount_of(&doses[0].key, &zone).as_deref(), Some("3 drops"));
        assert_eq!(health.amount_of(&doses[1].key, &zone).as_deref(), Some("1 drop"));
        // A dose the schedule no longer has: the amount at its time of day; a medicine gone: none.
        assert_eq!(health.amount_of(&format!("night@{}", at("2026-03-30T02:30[Europe/Paris]").timestamp().as_second() + 60), &zone).as_deref(), Some(""));
        assert_eq!(health.amount_of("gone@1800000000", &zone), None);
    }

    /// A medicine's generic name, strength and first day: written only when
    /// given, read back, said precisely for a professional; reminders keep
    /// the short name. Sioul 0.0.3 reads the file, its doses the same.
    #[test]
    fn generic_name_and_strength_written_only_when_given() {
        let mut levo = at_set_times("levo", "Thyrolan");
        levo.set_takes("1 tablet", &[take("07:30", "")]).unwrap();
        // None given: the file as before.
        let plain = toml::to_string(&Health { medicines: vec![levo.clone()], ..Health::default() }).unwrap();
        assert!(!plain.contains("generic") && !plain.contains("strength") && !plain.contains("since"), "{plain}");
        assert_eq!(levo.precise(), "Thyrolan");
        levo.generic = "levothyroxine".into();
        levo.strength = "75 µg".into();
        levo.since = Some("2026-03-02".parse().unwrap());
        let health = Health { medicines: vec![levo.clone()], ..Health::default() };
        let written = toml::to_string(&health).unwrap();
        let again: Health = toml::from_str(&written).unwrap();
        assert_eq!(again, health, "read back as written");
        assert_eq!(levo.precise(), "Thyrolan — levothyroxine 75 µg");
        assert_eq!(levo.short_name(), "Thyrolan");
        // The name is the generic name: said once.
        assert_eq!(Medicine { name: "Levothyroxine".into(), ..levo.clone() }.precise(), "levothyroxine 75 µg");
        // Reminders, the day's list, the card: the short name, the take's amount.
        assert_eq!(day_of(&health, "2026-10-08"), said(&[("07:30", "Thyrolan", "1 tablet")]));
        // Sioul 0.0.3 reads it: the same name and times; the new fields dropped at its save, the doses the same.
        let old: older::Health = toml::from_str(&written).unwrap();
        assert_eq!((old.medicines[0].name.as_str(), old.medicines[0].dose.as_str()), ("Thyrolan", "1 tablet"));
        let back: Health = toml::from_str(&toml::to_string(&old).unwrap()).unwrap();
        assert_eq!((back.medicines[0].generic.as_str(), back.medicines[0].strength.as_str(), back.medicines[0].since), ("", "", None));
        let (start, end) = (at("2026-10-08T00:00[Europe/Paris]"), at("2026-10-10T00:00[Europe/Paris]"));
        let due = |h: &Health| h.doses(&start, &end).into_iter().map(|d| (d.key, d.at, d.name, d.dose)).collect::<Vec<_>>();
        assert_eq!(due(&back), due(&health));
        // Written by hand with a generic name and no name: the generic name stands in.
        let by_hand = "[[medicine]]\nid = \"m\"\nname = \"\"\ngeneric = \"metformin\"\nstrength = \"500 mg\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\"]\n";
        let hand: Health = toml::from_str(by_hand).unwrap();
        assert_eq!((hand.medicines[0].short_name().as_str(), hand.medicines[0].precise().as_str()), ("metformin", "metformin 500 mg"));
        assert_eq!(day_of(&hand, "2026-10-08")[0].1, "metformin");
    }

    /// The review of 5 October 2026, F10 and F22: a record of no bytes (a
    /// crash after a write that had not reached a phone's disk, a full disk)
    /// read as lost, never as a sound empty one, the doses due before then in
    /// doubt; the same once the state folder is wiped whole, its witness with
    /// it, the one kept apart in the data folder telling. Each dose's records
    /// alike.
    #[test]
    fn a_record_of_no_bytes_or_gone_with_its_folder_is_lost_never_empty() {
        let dir = std::env::temp_dir().join(format!("sioul-record-lost-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (state, data) = (dir.join("state"), dir.join("data"));
        APART.with(|apart| *apart.borrow_mut() = Some(data.clone()));
        let path = state.join("health-state.toml");
        assert_eq!(HealthState::read(&path), Ok(HealthState::default()), "never written: nothing lost");
        HealthState::update(&path, DUE, |s| {
            s.taken.insert("iron@1800000000".into(), DUE);
        })
        .unwrap();
        std::fs::write(&path, "").unwrap();
        assert_eq!(HealthState::read(&path), Err(Unsound::Lost), "of no bytes, written before");
        std::fs::remove_dir_all(&state).unwrap();
        assert_eq!(HealthState::read(&path), Err(Unsound::Lost), "the state folder wiped, the witness apart kept");
        HealthState::set_aside(&path, DUE).unwrap();
        assert_eq!(HealthState::read(&path), Ok(HealthState::default()), "set aside: a new record starts");
        let records = state.join("health-doses.toml");
        crate::doses::DoseRecords::update(&records, DUE, |r| r.open("iron@1800000000", "desk-id", DUE)).unwrap();
        std::fs::write(&records, "").unwrap();
        assert_eq!(crate::doses::DoseRecords::read(&records), Err(Unsound::Lost));
        std::fs::remove_dir_all(&state).unwrap();
        assert_eq!(crate::doses::DoseRecords::read(&records), Err(Unsound::Lost));
        APART.with(|apart| *apart.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The check of 36002ea, A2: a health file of no bytes, or gone though
    /// written here, was lost (Sioul never writes it empty): never read as no
    /// medicines, never written over; a move taken back (`HealthState::back`)
    /// puts the start back on every device, unless it moved again since; a
    /// witness that cannot be made never says the write failed.
    #[test]
    fn a_health_file_of_no_bytes_is_lost_and_a_move_taken_back_comes_back() {
        let dir = std::env::temp_dir().join(format!("sioul-health-lost-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("health.toml");
        let start = at("2026-10-05T08:00[Europe/Paris]").timestamp().as_second();
        let medicine = |from: i64| Medicine { id: "antibiotic".into(), name: "Antibiotic".into(), dose: "1 tablet".into(), schedule: Schedule::Hours { hours: 8, from }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None };
        Health { medicines: vec![medicine(start)], ..Health::default() }.save(&path).unwrap();
        assert!(witnessed(&path), "its witness made");
        assert_eq!(Health::read(&path).unwrap().medicines.len(), 1);
        std::fs::write(&path, "").unwrap();
        assert!(matches!(Health::read(&path), Err(Unsound::Lost)), "of no bytes: lost");
        assert!(matches!(Health::read_bytes(&path), Err(Unsound::Lost)));
        assert!(Health::change(&path, |h| Ok(h.medicines.clear())).is_err(), "never written over");
        assert_eq!(std::fs::read(&path).unwrap(), b"");
        assert!(!path.with_extension("toml.unreadable").exists(), "nothing to copy aside");
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(Health::read(&path), Err(Unsound::Lost)), "gone though written here");
        // A move taken back: the start put back, where it is where the take had moved it.
        let late = start + 90 * 60;
        let after = late + 8 * 3600;
        let mut health = Health { medicines: vec![medicine(after)], ..Health::default() };
        let mut state = HealthState::default();
        state.back.insert(format!("antibiotic@{start}"), [start, after, late + 600]);
        assert!(health.mend_shifts(&state));
        assert!(matches!(health.medicines[0].schedule, Schedule::Hours { from, .. } if from == start), "put back");
        assert!(!health.mend_shifts(&state), "once");
        // Moved again since (the next dose taken late, its move recorded): left where that move put it.
        let mut health = Health { medicines: vec![medicine(after)], ..Health::default() };
        let next = after + 3600 + 8 * 3600;
        state.moved.insert(format!("antibiotic@{after}"), [after, next]);
        state.taken.insert(format!("antibiotic@{after}"), after + 3600);
        assert!(health.mend_shifts(&state));
        assert!(matches!(health.medicines[0].schedule, Schedule::Hours { from, .. } if from == next), "the later move holds");
        // Kept a week, then gone with the marks' week.
        let mut saved = HealthState::default();
        saved.back.insert("antibiotic@1".into(), [1, 2, late]);
        saved.save(&dir.join("health-state.toml"), late + 8 * 86_400).unwrap();
        assert!(HealthState::read(&dir.join("health-state.toml")).unwrap().back.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F23: a medicine's start, moved by a dose taken late, put back by an
    /// edit made elsewhere meanwhile: moved again where the mark said, its
    /// edit kept; twice moved, both; nothing to mend, nothing changed.
    #[test]
    fn a_move_lost_to_an_edit_elsewhere_is_mended() {
        let start = at("2026-10-05T08:00[Europe/Paris]").timestamp().as_second();
        let mut health = Health {
            medicines: vec![Medicine { id: "antibiotic".into(), name: "Antibiotic".into(), dose: "1 tablet, with food".into(), schedule: Schedule::Hours { hours: 8, from: start }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None }],
            ..Health::default()
        };
        let mut state = HealthState::default();
        let late = start + 90 * 60;
        state.moved.insert(format!("antibiotic@{start}"), [start, late + 8 * 3600]);
        assert!(health.mend_shifts(&state));
        assert!(matches!(health.medicines[0].schedule, Schedule::Hours { from, .. } if from == late + 8 * 3600));
        assert_eq!(health.medicines[0].dose, "1 tablet, with food", "its other changes kept");
        assert!(!health.mend_shifts(&state), "mended once");
        let next = late + 8 * 3600;
        state.moved.insert(format!("antibiotic@{next}"), [next, next + 3600 + 8 * 3600]);
        health.medicines[0].schedule = Schedule::Hours { hours: 8, from: start };
        assert!(health.mend_shifts(&state));
        assert!(matches!(health.medicines[0].schedule, Schedule::Hours { from, .. } if from == next + 3600 + 8 * 3600), "both moves");
    }

    /// R10 of the review of 10 October 2026: two moves from one start (the
    /// second dose marked on a device the first move had not reached): the
    /// mend goes where the latest mark moved it, whatever the keys' order.
    #[test]
    fn two_moves_from_one_start_mend_to_the_latest_mark() {
        let start = at("2026-10-05T08:00[Europe/Paris]").timestamp().as_second();
        let medicine = Medicine { id: "antibiotic".into(), name: "Antibiotic".into(), dose: "1 tablet".into(), schedule: Schedule::Hours { hours: 8, from: start }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None };
        let mut state = HealthState::default();
        // The dose of 08:00 taken at 09:30 on the phone; the one of 16:00 taken at 16:45 on the desk, which still had 08:00 as its start.
        let (first, second) = (format!("antibiotic@{start}"), format!("antibiotic@{}", start + 8 * 3600));
        state.moved.insert(first.clone(), [start, start + 90 * 60 + 8 * 3600]);
        state.moved.insert(second.clone(), [start, start + 8 * 3600 + 45 * 60 + 8 * 3600]);
        state.taken.insert(first, start + 90 * 60);
        state.taken.insert(second, start + 8 * 3600 + 45 * 60);
        let mut health = Health { medicines: vec![medicine.clone()], ..Health::default() };
        assert!(health.mend_shifts(&state));
        assert!(matches!(health.medicines[0].schedule, Schedule::Hours { from, .. } if from == start + 8 * 3600 + 45 * 60 + 8 * 3600), "the latest mark's move");
        // Marked in the other order: the same.
        let (a, b) = (state.taken.values().copied().min().unwrap(), state.taken.values().copied().max().unwrap());
        for taken in state.taken.values_mut() {
            *taken = if *taken == a { b } else { a };
        }
        let mut health = Health { medicines: vec![medicine], ..Health::default() };
        assert!(health.mend_shifts(&state));
        assert!(matches!(health.medicines[0].schedule, Schedule::Hours { from, .. } if from == start + 90 * 60 + 8 * 3600), "the latest mark's move, whatever its key");
    }
}
