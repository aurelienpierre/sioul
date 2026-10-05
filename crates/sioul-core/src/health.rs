// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Health and well-being: prescriptions, when to fetch their medicines and
//! when to renew them; medicines and when to take them; a pause to move during
//! long focus; a daily limit on chats. Sent nowhere, except sealed to your
//! other devices when you share with them (docs/database.md).
//!
//! Nothing here counts what was missed. A dose not marked taken is simply not
//! marked; the next one comes as planned. One marked late says when it was
//! taken, and a medicine taken every few hours may move its next doses by as much.

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
    /// A folder your watch's files come to (Gadgetbridge's exports, Garmin's
    /// export ZIPs); "" for none. A watch the desktop shows is read besides.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub watch_folder: String,
    /// Offers from the watch (a walk, a pause): on unless you say.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub watch_offers: bool,
    /// Meals, naps and the night: times kept free, set first (`needs`).
    #[serde(default)]
    pub needs: crate::needs::Needs,
}

fn is_true(value: &bool) -> bool {
    *value
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
}

/// When a medicine is taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "every", rename_all = "kebab-case")]
pub enum Schedule {
    /// At these times each day: "12:00", "18:00".
    Day { times: Vec<String> },
    /// Every `days` days at `time`, counted from `from` ("every other day from tomorrow").
    Days {
        days: u32,
        time: String,
        #[serde(deserialize_with = "crate::budget::dates::required")]
        from: Date,
    },
    /// Every `hours` hours from `from`, Unix seconds ("every 6 hours from now").
    /// `follows`: a dose taken late moved the next ones by as much, last time
    /// (`Health::taken_late`); asked again each time, this the answer offered.
    Hours {
        hours: u32,
        from: i64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        follows: bool,
    },
}

/// "18:00" → 18:00.
fn time_of(text: &str) -> Option<Time> {
    let (hour, minute) = text.trim().split_once(':')?;
    Time::new(hour.trim().parse().ok()?, minute.trim().parse().ok()?, 0, 0).ok()
}

impl Schedule {
    /// The doses from `start` (included) to `end` (excluded), in order.
    pub fn doses(&self, start: &Zoned, end: &Zoned) -> Vec<Zoned> {
        let zone = start.time_zone().clone();
        let mut out = Vec::new();
        match self {
            Schedule::Day { times } => {
                let mut times: Vec<Time> = times.iter().filter_map(|t| time_of(t)).collect();
                times.sort();
                let mut day = start.date();
                while day <= end.date() {
                    for time in &times {
                        if let Ok(at) = day.to_datetime(*time).to_zoned(zone.clone())
                            && &at >= start
                            && &at < end
                        {
                            out.push(at);
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
                        out.push(at);
                    }
                    let Ok(next) = day.checked_add(Span::new().days(i64::from(*days))) else { break };
                    day = next;
                }
            }
            Schedule::Hours { hours, from, .. } => {
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
                        out.push(t.to_zoned(zone.clone()));
                    }
                    at += step;
                }
            }
        }
        out
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
        match std::fs::read_to_string(path).map(|t| toml::from_str::<Health>(&t)) {
            Ok(Ok(health)) => health,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Health::default(),
            _ => {
                let _ = std::fs::copy(path, path.with_extension("toml.unreadable"));
                Health::default()
            }
        }
    }

    /// Written next to its place, then moved: never half a file. Yours alone.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)?;
        keep_private(&temporary);
        std::fs::rename(&temporary, path).map_err(fail)
    }

    /// A new id among the others, from a name.
    pub fn new_id(&self, name: &str) -> String {
        let taken: Vec<String> = self.prescriptions.iter().map(|p| p.id.clone()).chain(self.medicines.iter().map(|m| m.id.clone())).collect();
        crate::cases::new_id(name, &taken)
    }

    /// Every dose from `start` to `end`, of the medicines taken then.
    pub fn doses(&self, start: &Zoned, end: &Zoned) -> Vec<Dose> {
        let mut out: Vec<Dose> = self
            .medicines
            .iter()
            .filter(|m| !m.paused)
            .flat_map(|m| {
                m.schedule
                    .doses(start, end)
                    .into_iter()
                    .filter(|at| m.until.is_none_or(|until| at.date() <= until))
                    .map(|at| Dose { key: format!("{}@{}", m.id, at.timestamp().as_second()), medicine: m.id.clone(), name: m.name.clone(), dose: m.dose.clone(), at })
            })
            .collect();
        out.sort_by(|a, b| a.at.cmp(&b.at).then(a.name.cmp(&b.name)));
        out
    }

    /// A dose taken late, at `at` (Unix seconds), as you say. For a medicine
    /// taken every few hours, when you ask, the next doses move by as much:
    /// the hours between two doses are kept (an antibiotic every 8 hours).
    /// The answer is kept, to be offered next time. Returns where the doses
    /// started before and after they moved, to put them back if the mark is
    /// taken back; none when nothing moved.
    pub fn taken_late(&mut self, key: &str, at: i64, move_next: bool) -> Option<(i64, i64)> {
        let (id, due) = key.rsplit_once('@')?;
        let due: i64 = due.parse().ok()?;
        let medicine = self.medicines.iter_mut().find(|m| m.id == id)?;
        let Schedule::Hours { hours, from, follows } = &mut medicine.schedule else { return None };
        *follows = move_next;
        let step = i64::from(*hours) * 3600;
        if !move_next || step == 0 || at == due {
            return None;
        }
        let before = *from;
        *from = at + step;
        Some((before, *from))
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

/// Another computer sharing with this one, as known here, for the doses
/// (docs/health.md, "Knowing"): until when everything it wrote is read here,
/// whether it said it closed, how late its news comes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    /// Its name, to say where: the host's.
    pub name: String,
    /// Everything it wrote until then (Unix seconds) is read here; 0: never sure.
    #[serde(default)]
    pub known_until: i64,
    /// At `known_until`, it said it closed (quit, or put away on a phone):
    /// it marks nothing until it says otherwise.
    #[serde(default)]
    pub closed: bool,
    /// The longest its news took to come here in the last day, in seconds;
    /// none measured yet.
    #[serde(default)]
    pub delay: Option<i64>,
    /// When a line it wrote was found unreadable here: what it said is lost.
    #[serde(default)]
    pub broken: Option<i64>,
    /// When it was last heard at all.
    #[serde(default)]
    pub heard: i64,
}

/// Why a dose is not known here: whether it was taken, Sioul cannot tell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doubt {
    /// This computer's record of doses could not be read, or was lost, then.
    Record { since: i64 },
    /// Another computer may have marked it: everything it wrote is known
    /// until then (0: never), and it was open or closed then.
    Unheard { name: String, until: i64, closed: bool },
    /// A line another computer wrote could not be read here.
    Broken { name: String },
}

/// The longest a closed computer's news may take to come here for it to count
/// as closed: had it opened again, it would be known by now.
pub const NEWS_IN: i64 = 3 * 60;

/// Why a dose due at `due` (Unix seconds) is not known here now; none when it
/// is. Known means: this computer's record reads, and every other computer
/// sharing with it, heard in the last month, was heard after the dose was due
/// with everything it wrote until then read here, or said it closed before
/// and its news comes within minutes. A dose taken twice can harm: whatever
/// is not known is said, never guessed.
pub fn doubts(due: i64, now: i64, record_lost: Option<i64>, peers: &[Peer]) -> Vec<Doubt> {
    let mut out = Vec::new();
    // Doses due before the record was found broken or gone, in the day before.
    if let Some(since) = record_lost.filter(|since| due <= *since && *since - due < 86_400) {
        out.push(Doubt::Record { since });
    }
    for peer in peers.iter().filter(|p| now - p.heard < 30 * 86_400) {
        // A line lost from around the dose's time may have been its mark.
        if peer.broken.is_some_and(|at| at >= due - 6 * 3600) {
            out.push(Doubt::Broken { name: peer.name.clone() });
            continue;
        }
        let known = peer.known_until >= due || (peer.closed && peer.delay.is_some_and(|d| d <= NEWS_IN));
        if !known {
            out.push(Doubt::Unheard { name: peer.name.clone(), until: peer.known_until, closed: peer.closed });
        }
    }
    out
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

/// One dose to take.
#[derive(Debug, Clone, PartialEq)]
pub struct Dose {
    /// "<medicine>@<Unix seconds>": what marks it taken.
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
    /// Doses answered "not taken" when asked afterwards, by key; kept a week.
    #[serde(default)]
    pub not_taken: BTreeMap<String, i64>,
    /// Doses whose take moved the next ones, by key: where they started
    /// before and after (`Health::taken_at`); kept with the mark.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub moved: BTreeMap<String, [i64; 2]>,
    /// Errands whose task was made, by key.
    #[serde(default)]
    pub errands: BTreeMap<String, String>,
    /// Chats: the day counted, the minutes used that day, and covered until when.
    #[serde(default, deserialize_with = "crate::budget::dates::optional")]
    pub chat_day: Option<Date>,
    #[serde(default)]
    pub chat_minutes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chats_locked_until: Option<i64>,
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

impl HealthState {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("health-state.toml")
    }

    /// A hidden file beside the record, made the first time it is written:
    /// the record gone with its witness there was lost, not never written.
    fn witness(path: &Path) -> PathBuf {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        path.with_file_name(format!(".{name}.written"))
    }

    /// The doses' record as written, or why it cannot be trusted. Never an
    /// empty record in place of one that does not read: a dose taken would
    /// look not taken, and written back empty, the sharing would take every
    /// mark out on your other devices too.
    pub fn read(path: &Path) -> Result<HealthState, Unsound> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|_| Unsound::Unreadable),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if Self::witness(path).exists() {
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
            let _ = std::fs::remove_file(Self::witness(path));
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
        self.not_taken.retain(|_, at| *at >= week);
        self.moved.retain(|key, _| self.taken.contains_key(key));
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        // Next to its place, then moved: a dose marked is never lost to half a file.
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)?;
        keep_private(&temporary);
        std::fs::rename(&temporary, path).map_err(fail)?;
        let witness = Self::witness(path);
        if !witness.exists() {
            std::fs::write(&witness, "").map_err(fail)?;
        }
        Ok(())
    }

    /// The doses to remind now: due in the last `minutes`, not taken, not reminded yet.
    pub fn to_remind(&self, health: &Health, now: &Zoned, minutes: i64) -> Vec<Dose> {
        let start = now.checked_sub(Span::new().minutes(minutes)).unwrap_or_else(|_| now.clone());
        let end = now.checked_add(Span::new().seconds(1)).unwrap_or_else(|_| now.clone());
        health.doses(&start, &end).into_iter().filter(|d| !self.taken.contains_key(&d.key) && !self.reminded.contains_key(&d.key)).collect()
    }

    /// The doses of the last `hours`, past their time by `grace` minutes,
    /// neither marked nor reminded: due while Sioul ran nowhere. Asked about
    /// afterwards, as a question on the past; never reminded to take now.
    pub fn unanswered(&self, health: &Health, now: &Zoned, hours: i64, grace: i64) -> Vec<Dose> {
        let start = now.checked_sub(Span::new().hours(hours)).unwrap_or_else(|_| now.clone());
        let end = now.checked_sub(Span::new().minutes(grace)).unwrap_or_else(|_| now.clone());
        health.doses(&start, &end).into_iter().filter(|d| !self.taken.contains_key(&d.key) && !self.reminded.contains_key(&d.key) && !self.not_taken.contains_key(&d.key)).collect()
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
        let noon_and_six = Schedule::Day { times: vec!["18:00".into(), "12:00".into()] };
        let doses = noon_and_six.doses(&at("2026-10-03T13:00[Europe/Paris]"), &at("2026-10-05T00:00[Europe/Paris]"));
        let shown: Vec<String> = doses.iter().map(|z| z.strftime("%d %H:%M").to_string()).collect();
        assert_eq!(shown, vec!["03 18:00", "04 12:00", "04 18:00"]);
        // Every other day from tomorrow.
        let other = Schedule::Days { days: 2, time: "08:00".into(), from: "2026-10-04".parse().unwrap() };
        let shown: Vec<String> = other.doses(&at("2026-10-03T09:00[Europe/Paris]"), &at("2026-10-09T00:00[Europe/Paris]")).iter().map(|z| z.strftime("%d").to_string()).collect();
        assert_eq!(shown, vec!["04", "06", "08"]);
        // Every six hours from 18:30.
        let six = Schedule::Hours { hours: 6, from: at("2026-10-03T18:30[Europe/Paris]").timestamp().as_second(), follows: false };
        let shown: Vec<String> = six.doses(&at("2026-10-04T00:00[Europe/Paris]"), &at("2026-10-04T13:00[Europe/Paris]")).iter().map(|z| z.strftime("%H:%M").to_string()).collect();
        assert_eq!(shown, vec!["00:30", "06:30", "12:30"]);
    }

    #[test]
    fn a_late_dose_moves_the_next_ones_when_asked() {
        let eight = || Health {
            medicines: vec![Medicine { id: "antibiotic".into(), name: "Antibiotic".into(), dose: String::new(), schedule: Schedule::Hours { hours: 8, from: at("2026-10-05T08:00[Europe/Paris]").timestamp().as_second(), follows: false }, prescription: None, until: None, paused: false }],
            ..Health::default()
        };
        let times = |health: &Health| -> Vec<String> { health.doses(&at("2026-10-05T12:00[Europe/Paris]"), &at("2026-10-06T12:00[Europe/Paris]")).iter().map(|d| d.at.strftime("%H:%M").to_string()).collect() };
        let key = format!("antibiotic@{}", at("2026-10-05T08:00[Europe/Paris]").timestamp().as_second());
        let late = at("2026-10-05T09:30[Europe/Paris]").timestamp().as_second();
        // Not asked: the next doses keep their times.
        let mut kept = eight();
        assert_eq!(kept.taken_late(&key, late, false), None);
        assert_eq!(times(&kept), ["16:00", "00:00", "08:00"]);
        // Asked: an hour and a half late, the next ones too; the answer kept for next time.
        let mut health = eight();
        let (before, after) = health.taken_late(&key, late, true).unwrap();
        assert_eq!(times(&health), ["17:30", "01:30", "09:30"]);
        assert!(matches!(health.medicines[0].schedule, Schedule::Hours { follows: true, .. }));
        // The mark taken back: where they were.
        assert!(health.taken_back(&key, before, after));
        assert_eq!(times(&health), ["16:00", "00:00", "08:00"]);
        // Moved again since: a mark taken back later leaves them.
        let (before, after) = health.taken_late(&key, late, true).unwrap();
        let later = format!("antibiotic@{}", at("2026-10-05T17:30[Europe/Paris]").timestamp().as_second());
        health.taken_late(&later, at("2026-10-05T18:00[Europe/Paris]").timestamp().as_second(), true).unwrap();
        assert!(!health.taken_back(&key, before, after));
        assert_eq!(times(&health), ["02:00", "10:00"], "8 hours after the one taken at 18:00");
        // Medicines at set times keep them.
        let mut daily = Health { medicines: vec![Medicine { schedule: Schedule::Day { times: vec!["08:00".into()] }, ..eight().medicines[0].clone() }], ..Health::default() };
        assert_eq!(daily.taken_late(&key, late, true), None);
        // Written as kept, read back the same; unsaid when off.
        let again: Health = toml::from_str(&toml::to_string(&health).unwrap()).unwrap();
        assert_eq!(again, health);
        assert!(!toml::to_string(&eight()).unwrap().contains("follows"));
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

    #[test]
    fn a_dose_is_known_or_said_unknown() {
        let due = 1_800_000_000;
        let peer = |known_until: i64, closed: bool, delay: Option<i64>| Peer { name: "laptop".into(), known_until, closed, delay, broken: None, heard: known_until.max(due - 3_600) };
        // Alone, with a record that reads: known.
        assert!(doubts(due, due + 60, None, &[]).is_empty());
        // The laptop heard after the dose was due, everything it wrote read: known.
        assert!(doubts(due, due + 600, None, &[peer(due + 120, false, Some(30))]).is_empty());
        // Last heard before the dose, open: it may have marked it.
        assert_eq!(doubts(due, due + 600, None, &[peer(due - 300, false, Some(30))]), vec![Doubt::Unheard { name: "laptop".into(), until: due - 300, closed: false }]);
        // Closed before, and its news comes within minutes: known.
        assert!(doubts(due, due + 600, None, &[peer(due - 3_600, true, Some(60))]).is_empty());
        // Closed before, but its news can take half an hour (a phone's sync): not known.
        assert_eq!(doubts(due, due + 600, None, &[peer(due - 3_600, true, Some(1_800))]).len(), 1);
        // Closed, its delay never measured: not known.
        assert_eq!(doubts(due, due + 600, None, &[peer(due - 3_600, true, None)]).len(), 1);
        // Never heard complete (an older Sioul that does not say what it wrote): not known.
        assert_eq!(doubts(due, due + 600, None, &[Peer { name: "phone".into(), heard: due, ..Peer::default() }]).len(), 1);
        // A line of it lost around the dose's time: not known, whatever else.
        let broken = Peer { broken: Some(due - 60), ..peer(due + 120, false, Some(30)) };
        assert_eq!(doubts(due, due + 600, None, &[broken]), vec![Doubt::Broken { name: "laptop".into() }]);
        // Gone for over a month: no longer counted.
        let gone = Peer { heard: due - 40 * 86_400, ..peer(0, false, None) };
        assert!(doubts(due, due + 600, None, &[gone]).is_empty());
        // This computer's record was found broken after the dose was due: not known; doses due after are.
        assert_eq!(doubts(due, due + 600, Some(due + 300), &[]), vec![Doubt::Record { since: due + 300 }]);
        assert!(doubts(due + 900, due + 1_000, Some(due + 300), &[]).is_empty());
    }

    #[test]
    fn reminded_once_never_counted() {
        let health = Health {
            medicines: vec![Medicine { id: "vitamin-d".into(), name: "Vitamin D".into(), dose: "1000 IU".into(), schedule: Schedule::Day { times: vec!["12:00".into()] }, prescription: None, until: None, paused: false }],
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

    #[test]
    fn read_as_written_by_hand_and_by_sioul() {
        let by_hand = "[[prescription]]\nid = \"d\"\ntitle = \"Vitamin D\"\nuntil = 2026-12-31\n\n[[medicine]]\nid = \"iron\"\nname = \"Iron\"\n[medicine.schedule]\nevery = \"days\"\ndays = 2\ntime = \"12:00\"\nfrom = 2026-10-03\n";
        let health: Health = toml::from_str(by_hand).unwrap();
        assert_eq!(health.prescriptions[0].until, Some("2026-12-31".parse().unwrap()));
        let again: Health = toml::from_str(&toml::to_string(&health).unwrap()).unwrap();
        assert_eq!(again, health);
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
}
