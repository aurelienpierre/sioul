// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Training by itself (docs/spam-filter.md, "Every device trains its own"):
//! on every device whose switch is on ("Train here", `[spam] train_here`,
//! this device's own, on unless said), phones included, once a week at most
//! after the last training (by hand or by itself), when it is on mains
//! power, not saving power, idle for 15 minutes or locked (a phone: its
//! screen off), no focus session runs, its battery not above 40 °C where
//! the system tells it, and on a network the download setting allows (any,
//! unless kept to unmetered ones; a network the system does not call metered
//! counts as unmetered). It downloads first, then learns, with the usual
//! rule for replacing the table. On a computer, a job run apart (`sioul spam
//! train --by-itself`), at the lowest priority, on half the cores; on a
//! phone, a thread of the background service's process. Stopped when the
//! device is unplugged or starts saving power, to try again later.
//!
//! **The guard** ([`watch_supply`]): a training that runs past 30 minutes
//! (the download aside, which goes on where it stopped) stops, and this
//! device no longer counts as learning ([`guard`], [`Guard`]): it reads the
//! tables of the devices that learn, as a device that never trained does,
//! and the Porch says so once ([`Notice`]). Its switch stays on: it tries
//! again only once its mail has changed a lot (a quarter more or less in its
//! corpus, [`changed_a_lot`], looked at by its weekly download), or when you
//! ask (Train now, or the Porch's "Try again"). A device too slow for it
//! never needs a check beforehand: a phone of 2025 trains about 12,000
//! messages in 30 minutes.
//!
//! **Heat** never turns training off: above 40 °C the run does not start, or
//! stops for this time, and tries again at the next chance. Nothing is said.
//!
//! **The first training** on a device with no table says once that it is
//! learning, then how long it took ([`Notice`]).
//!
//! [`decide`] is the rule, a pure function of what is known ([`Facts`]); an
//! unknown fact counts as not holding. [`facts`] reads them, the cheap ones
//! first: the system is asked about its power and its session only when
//! nothing else holds a training back. What the last run by itself did is
//! kept beside the last training's summary ([`Run`], `auto.toml`). Never a
//! notification.

use crate::Dirs;
use serde::{Deserialize, Serialize};
use sioul_sync::power::Power;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The least time between two trainings, by hand or by itself.
pub const WEEK: i64 = 7 * 86_400;
/// A training running longer than this (its download aside) stops, and the
/// device no longer counts as learning (`guard`): measured, 67.5 seconds on a
/// computer and 4.5 minutes on a slow phone for 3,457 messages
/// (docs/research.md), about 12,000 messages on such a phone in this time.
pub const LONGEST: Duration = Duration::from_secs(30 * 60);
/// A battery warmer than this, in tenths of a degree Celsius, as Android and
/// Linux's power supplies say it: no training starts, and one running stops
/// for this time (`Halt::Hot`), to try again at the next chance.
pub const HOT_TENTHS: i32 = 400;
/// After a run by itself stopped (unplugged, saving power): the soonest it tries again.
pub const AFTER_STOP: i64 = 15 * 60;
/// After one that failed, or whose process vanished: the soonest it tries again.
pub const AFTER_FAILURE: i64 = 86_400;
/// How often a run by itself asks whether its power still allows it.
pub const SUPPLY_EVERY: Duration = Duration::from_secs(30);

/// Who made the table in use: said in the settings, never a reason to wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trainer {
    /// This device.
    Here,
    /// Another device, by its name (a table that came before the tables were retired).
    Elsewhere(String),
    /// No table yet, or none this Sioul can read: the next training here is the first.
    Nobody,
}

/// How a run by itself ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Trained: its table replaced the one in use.
    Replaced,
    /// Trained: the table in use stayed (the new one no better, or nothing new).
    Kept,
    /// Stopped: unplugged, saving power, or asked to.
    Stopped,
    /// Failed: too few messages, the disk, an error.
    Failed,
    /// Downloaded, then waited: the guard stopped an earlier training here,
    /// and the mail has not changed a lot since (`Guard`).
    Waited,
}

/// The last run by itself, as `auto.toml` keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// When it started (Unix seconds), and its job's id.
    pub started: i64,
    pub job: String,
    /// It downloaded first (a connection not metered).
    #[serde(default)]
    pub fetched: bool,
    /// When it ended, and how; none while it runs, or when its process vanished.
    #[serde(default)]
    pub ended: Option<i64>,
    #[serde(default)]
    pub outcome: Option<Outcome>,
}

impl Run {
    fn path(dirs: &Dirs) -> PathBuf {
        dirs.state.join("auto.toml")
    }

    /// The last run by itself, if one was.
    pub fn load(dirs: &Dirs) -> Option<Run> {
        toml::from_str(&std::fs::read_to_string(Run::path(dirs)).ok()?).ok()
    }

    /// Kept, written whole.
    pub fn save(&self, dirs: &Dirs) -> Result<(), String> {
        std::fs::create_dir_all(&dirs.state).map_err(|e| format!("{}: {e}", dirs.state.display()))?;
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::corpus::write_whole(&Run::path(dirs), text.as_bytes()).map_err(|e| e.to_string())
    }

    /// The end of the run of `job`, kept: when, and how.
    pub fn finish(dirs: &Dirs, job: &str, outcome: Outcome, now: i64) -> Result<(), String> {
        let Some(run) = Run::load(dirs).filter(|r| r.job == job) else { return Ok(()) };
        Run { ended: Some(now), outcome: Some(outcome), ..run }.save(dirs)
    }
}

/// What is known now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The setting: "Train here" (`[spam] train_here`, this device's own, on unless said).
    pub enabled: bool,
    /// The download kept to unmetered networks (`[spam] train_network`).
    pub unmetered_only: bool,
    /// When the last training here ended, by hand or by itself (`trained.toml`).
    pub last_trained: Option<i64>,
    /// That training decided its labels by older rules (`labels::RULES`):
    /// the next one does not wait for the week.
    pub outdated: bool,
    /// The last run by itself.
    pub last_run: Option<Run>,
    /// The guard stopped a training here (`Guard`): read only once nothing else holds a training back.
    pub guard: Option<Guard>,
    /// A training runs: the window's, or a job's (`a_job_runs`).
    pub running: bool,
    /// A focus session runs.
    pub focus: Option<bool>,
    /// What the device says of itself (`sioul_sync::power`; a phone's, from Android).
    pub power: Power,
}

/// Why it does not train now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// The setting is off.
    Off,
    /// A training runs already.
    Running,
    /// Trained less than a week ago.
    Recent,
    /// A run by itself stopped or failed not long ago.
    Retry,
    /// A focus session runs, or it cannot tell.
    Focus,
    /// On its battery, or it cannot tell.
    Battery,
    /// Saving power, or it cannot tell.
    PowerSaver,
    /// Someone at it, or it cannot tell.
    InUse,
    /// Its battery above 40 °C, as the system says.
    Hot,
    /// A metered network, the download kept to unmetered ones.
    Metered,
    /// The guard stopped a training here, and its weekly download since
    /// found the mail not changed a lot (`Guard`).
    Guarded,
}

/// What to do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Train, downloading first: always, on a network the setting allows.
    Train { fetch: bool },
    Wait(Wait),
}

/// The rule: every condition must hold, an unknown one counting as not
/// holding, but the network's cost and the battery's warmth, which count as
/// fine when the system does not say them; the cheap ones are looked at first
/// (`facts` asks the system only when a training waits on its power alone).
pub fn decide(f: &Facts, now: i64) -> Decision {
    let wait = |why| Decision::Wait(why);
    if !f.enabled {
        return wait(Wait::Off);
    }
    if f.running {
        return wait(Wait::Running);
    }
    if !f.outdated && f.last_trained.is_some_and(|at| now - at < WEEK) {
        return wait(Wait::Recent);
    }
    // Guarded: one download a week, its training only once the mail changed a lot (`waits_for_mail`).
    if let Some(guard) = &f.guard
        && f.last_run.as_ref().is_some_and(|run| run.ended.unwrap_or(run.started) >= guard.at && now - run.started < WEEK)
    {
        return wait(Wait::Guarded);
    }
    // A run by itself that ended without a training since (stopped, failed, vanished): not again too soon.
    if let Some(run) = &f.last_run
        && !f.last_trained.is_some_and(|at| at >= run.started)
    {
        let pause = if run.outcome == Some(Outcome::Stopped) { AFTER_STOP } else { AFTER_FAILURE };
        if now - run.ended.unwrap_or(run.started) < pause {
            return wait(Wait::Retry);
        }
    }
    if f.focus != Some(false) {
        return wait(Wait::Focus);
    }
    if f.power.on_mains != Some(true) {
        return wait(Wait::Battery);
    }
    if f.power.power_saver != Some(false) {
        return wait(Wait::PowerSaver);
    }
    if f.power.idle != Some(true) {
        return wait(Wait::InUse);
    }
    if hot(&f.power) {
        return wait(Wait::Hot);
    }
    if f.unmetered_only && f.power.metered == Some(true) {
        return wait(Wait::Metered);
    }
    Decision::Train { fetch: true }
}

/// The battery above `HOT_TENTHS`, as the system says it (unsaid: not).
pub fn hot(power: &Power) -> bool {
    power.battery_tenths.is_some_and(|t| t > HOT_TENTHS)
}

/// What is known now: the settings (`spam`), the files of the last
/// trainings, the jobs, a focus session (`focus`:
/// `sioul_core::timelog::running`); then, only when nothing else holds a
/// training back, what the system says (`power`, asked once).
/// `window_trains`: the window's own training runs ("Train now").
pub fn facts(dirs: &Dirs, spam: &sioul_core::config::SpamSettings, window_trains: bool, focus: bool, now: i64, power: impl FnOnce() -> Power) -> Facts {
    let mut facts = Facts {
        enabled: spam.trains_here(),
        unmetered_only: spam.unmetered_only(),
        last_trained: crate::train::last(dirs).map(|s| s.trained_at).filter(|at| *at > 0),
        outdated: crate::train::last(dirs).is_some_and(|s| s.labels.rules < crate::labels::RULES),
        last_run: Run::load(dirs),
        guard: Guard::load(dirs),
        running: window_trains || a_job_runs(dirs),
        focus: Some(focus),
        power: Power::default(),
    };
    if decide(&facts, now) == Decision::Wait(Wait::Battery) {
        facts.power = power();
    }
    facts
}

/// Who made the table in use (the Porch's: this one, else the one before
/// when this one is refused), by the name it records.
pub fn trainer(dirs: &Dirs, here: &str) -> Trainer {
    match sioul_core::spam::table::Table::cached(&dirs.table()) {
        None => Trainer::Nobody,
        Some(table) if table.meta.device.is_empty() => Trainer::Nobody,
        Some(table) if table.meta.device == here => Trainer::Here,
        Some(table) => Trainer::Elsewhere(table.meta.device.clone()),
    }
}

/// Why a training running was stopped (`watch_supply`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Halt {
    /// Unplugged, saving power, or twice unable to tell: it tries again later.
    Supply,
    /// Its training ran past `LONGEST`: the device no longer counts as learning (`guard`).
    Long,
    /// Its battery above `HOT_TENTHS`: stopped for this time, tried again at
    /// the next chance, nothing said.
    Hot,
}

impl Halt {
    /// The guard's (`guard`): only a training too long. Heat never is.
    pub fn guards(self) -> bool {
        matches!(self, Halt::Long)
    }
}

/// The guard's mark (`guarded.toml` beside the corpus, this device's own):
/// a training here ran past `LONGEST`, when, and how many messages the
/// corpus held then. While it stands, this device does not count as learning
/// (`learns`): it reads the others' tables. Its weekly download goes on; its
/// training comes back once the mail changed a lot (`changed_a_lot`), or
/// when you ask (`Guard::clear`: Train now, the Porch's "Try again", the
/// switch turned on).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guard {
    pub at: i64,
    pub messages: u64,
}

impl Guard {
    fn path(dirs: &Dirs) -> PathBuf {
        dirs.state.join("guarded.toml")
    }

    /// The mark, if the guard stopped a training here.
    pub fn load(dirs: &Dirs) -> Option<Guard> {
        toml::from_str(&std::fs::read_to_string(Guard::path(dirs)).ok()?).ok()
    }

    /// Kept, in place of the one before.
    pub fn save(&self, dirs: &Dirs) -> Result<(), String> {
        std::fs::create_dir_all(&dirs.state).map_err(|e| format!("{}: {e}", dirs.state.display()))?;
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::corpus::write_whole(&Guard::path(dirs), text.as_bytes()).map_err(|e| e.to_string())
    }

    /// Gone: you asked, or the mail changed a lot. The training comes back.
    pub fn clear(dirs: &Dirs) {
        let _ = std::fs::remove_file(Guard::path(dirs));
    }
}

/// The messages the corpus holds (every folder, every UIDVALIDITY): the
/// measure of "its mail" for the guard (`changed_a_lot`).
pub fn messages(dirs: &Dirs) -> u64 {
    crate::corpus::status(dirs).accounts.iter().flat_map(|a| a.folders.iter()).map(|f| f.records).sum()
}

/// A quarter more, or less, than `then`: mail changed a lot.
pub fn changed_a_lot(then: u64, now: u64) -> bool {
    now.abs_diff(then).saturating_mul(4) >= then.max(1)
}

/// At the start of a run by itself's training, its download done: whether
/// it waits, the guard standing and the mail not changed a lot since
/// (`Outcome::Waited`). Changed a lot: the mark goes, and it trains.
pub fn waits_for_mail(dirs: &Dirs) -> bool {
    let Some(guard) = Guard::load(dirs) else { return false };
    if changed_a_lot(guard.messages, messages(dirs)) {
        Guard::clear(dirs);
        return false;
    }
    true
}

/// Whether this device learns, as its entry tells the others
/// (`sioul_sync::share::LEARNS_HERE`): its switch on, the guard not
/// standing, the spam filter's table in use made here, and the topics' table
/// (`topics_table`) made here or none yet. Else the tables travel to it.
pub fn learns(dirs: &Dirs, topics_table: &std::path::Path, spam: &sioul_core::config::SpamSettings, here: &str) -> bool {
    let topics_here = || sioul_core::topics::judging::cached(topics_table).is_none_or(|(table, _)| table.meta.device == here);
    spam.trains_here() && Guard::load(dirs).is_none() && trainer(dirs, here) == Trainer::Here && topics_here()
}

/// What the Porch says once of this device's training (`notice.toml` beside
/// the corpus, this device's own), until you put it away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub kind: NoticeKind,
    /// When it happened (Unix seconds).
    pub at: i64,
    /// How long the first training took, in seconds (`NoticeKind::FirstDone`).
    #[serde(default)]
    pub seconds: u64,
    /// You put it away: said no more.
    #[serde(default)]
    pub put_away: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoticeKind {
    /// The first training on this device runs: "Sioul is learning from your mail…".
    FirstRunning,
    /// It ended, and took this long.
    FirstDone,
    /// The guard stopped a training too long: this device reads the others'
    /// tables until its mail changed a lot or you ask. (`Halt::Hot` was said
    /// by an earlier build; heat is no longer said.)
    Guarded(Halt),
}

impl Notice {
    fn path(dirs: &Dirs) -> PathBuf {
        dirs.state.join("notice.toml")
    }

    /// The notice kept, if any.
    pub fn load(dirs: &Dirs) -> Option<Notice> {
        toml::from_str(&std::fs::read_to_string(Notice::path(dirs)).ok()?).ok()
    }

    /// Kept, in place of the one before.
    pub fn save(&self, dirs: &Dirs) -> Result<(), String> {
        std::fs::create_dir_all(&dirs.state).map_err(|e| format!("{}: {e}", dirs.state.display()))?;
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::corpus::write_whole(&Notice::path(dirs), text.as_bytes()).map_err(|e| e.to_string())
    }

    /// What the Porch shows now: a notice not put away (a first training
    /// still running only while one runs, else it vanished: nothing).
    pub fn shown(dirs: &Dirs, running: bool) -> Option<Notice> {
        Notice::load(dirs).filter(|n| !n.put_away && (n.kind != NoticeKind::FirstRunning || running))
    }

    /// Put away: said no more.
    pub fn put_away(dirs: &Dirs) -> Result<(), String> {
        match Notice::load(dirs) {
            Some(notice) => Notice { put_away: true, ..notice }.save(dirs),
            None => Ok(()),
        }
    }
}

/// A training begins: on a device with no table yet (`first`), the Porch
/// says once that it is learning. What it said before stays otherwise.
pub fn begin(dirs: &Dirs, first: bool, now: i64) {
    if first {
        let _ = Notice { kind: NoticeKind::FirstRunning, at: now, seconds: 0, put_away: false }.save(dirs);
    }
}

/// A training ended: the first one (its notice running) says how long it
/// took once it made a table (`made`), and is forgotten otherwise. A table
/// made here: the guard's mark, if any, goes too.
pub fn end(dirs: &Dirs, made: bool, seconds: u64, now: i64) {
    if made {
        Guard::clear(dirs);
    }
    if let Some(notice) = Notice::load(dirs).filter(|n| n.kind == NoticeKind::FirstRunning) {
        let _ = if made { Notice { kind: NoticeKind::FirstDone, at: now, seconds, put_away: notice.put_away }.save(dirs) } else { std::fs::remove_file(Notice::path(dirs)).map_err(|e| e.to_string()) };
    }
}

/// The guard (`Halt::guards`): the run stopped, this device marked as not
/// learning (`Guard`, the corpus's size kept), its switch left as it is, and
/// the Porch's notice kept to say so once. Heat: nothing.
pub fn guard(dirs: &Dirs, halt: Halt, now: i64) -> Result<(), String> {
    if !halt.guards() {
        return Ok(());
    }
    Guard { at: now, messages: messages(dirs) }.save(dirs)?;
    Notice { kind: NoticeKind::Guarded(halt), at: now, seconds: 0, put_away: false }.save(dirs)
}

/// Whether a job of the spam filter runs, a download or a training: its
/// process holds its lock (`$XDG_STATE_HOME/sioul/spam/jobs/<id>.lock`).
pub fn a_job_runs(dirs: &Dirs) -> bool {
    let jobs = dirs.state.join("jobs");
    std::fs::read_dir(&jobs).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "lock")).any(|path| {
        let Ok(lock) = std::fs::OpenOptions::new().read(true).open(&path) else { return false };
        match lock.try_lock() {
            Ok(()) => {
                let _ = lock.unlock();
                false
            }
            Err(_) => true,
        }
    })
}

/// The run by itself under way, asked to stop (F5, review of 7 November
/// 2026): its job's stop file (`jobs/<job>.stop`), which a computer's job
/// reads as `sioul spam stop` writes it, and a phone's background training
/// reads every few tenths of a second (`stop_asked`). "Train here" turned
/// off, or Stop in the window. Whether one was under way.
pub fn ask_stop(dirs: &Dirs) -> bool {
    let Some(run) = Run::load(dirs).filter(|r| r.ended.is_none()) else { return false };
    let jobs = dirs.state.join("jobs");
    let _ = std::fs::create_dir_all(&jobs);
    std::fs::write(jobs.join(format!("{}.stop", run.job)), b"").is_ok()
}

/// Whether the run of `job` was asked to stop (`ask_stop`).
pub fn stop_asked(dirs: &Dirs, job: &str) -> bool {
    dirs.state.join("jobs").join(format!("{job}.stop")).exists()
}

/// The stop file of `job` gone (its run ended, or a new one starts under its name).
pub fn stop_forgotten(dirs: &Dirs, job: &str) {
    let _ = std::fs::remove_file(dirs.state.join("jobs").join(format!("{job}.stop")));
}

/// fastText's threads for a run by itself: half the cores, one at least.
pub fn threads() -> u32 {
    std::thread::available_parallelism().map_or(1, |n| u32::try_from(n.get() / 2).unwrap_or(1).max(1))
}

/// While a run by itself goes on: its power asked every `every` (`supply`);
/// unplugged or saving power, or twice in a row unable to tell
/// (`Halt::Supply`), its battery above `HOT_TENTHS` (`Halt::Hot`), or its
/// training running past `LONGEST` (`Halt::Long`; `training` says how long
/// it has run, none while it downloads), `stop` is called once and the
/// watch ends; it ends too as soon as `done` says the run did.
pub fn watch_supply(supply: impl Fn() -> Power, stop: impl FnOnce(Halt), done: impl Fn() -> bool, training: impl Fn() -> Option<Duration>, every: Duration) {
    let mut unknown = false;
    loop {
        if done() {
            return;
        }
        if training().is_some_and(|ran| ran > LONGEST) {
            stop(Halt::Long);
            return;
        }
        let power = supply();
        if hot(&power) {
            stop(Halt::Hot);
            return;
        }
        let away = power.on_mains == Some(false) || power.power_saver == Some(true);
        let unsure = !away && !power.supplied();
        if away || (unsure && unknown) {
            stop(Halt::Supply);
            return;
        }
        unknown = unsure;
        let until = Instant::now() + every;
        while Instant::now() < until {
            if done() {
                return;
            }
            std::thread::sleep(every.min(Duration::from_millis(200)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    const NOW: i64 = 1_791_360_000;

    fn power(on_mains: Option<bool>, power_saver: Option<bool>, idle: Option<bool>, metered: Option<bool>) -> Power {
        Power { on_mains, power_saver, idle, metered, battery_tenths: None }
    }

    /// Everything allows it: set on, a week and more since the last training
    /// here, nothing running, no focus, on mains power, not saving power,
    /// idle, a connection not metered.
    fn ready() -> Facts {
        Facts {
            enabled: true,
            unmetered_only: false,
            last_trained: Some(NOW - WEEK - 1),
            outdated: false,
            last_run: None,
            guard: None,
            running: false,
            focus: Some(false),
            power: power(Some(true), Some(false), Some(true), Some(false)),
        }
    }

    /// Every device whose switch is on trains, whoever made the table in
    /// use, and one that never trained (no `trained.toml`) too: its first.
    #[test]
    fn it_trains_when_everything_allows_it() {
        assert_eq!(decide(&ready(), NOW), Decision::Train { fetch: true });
        assert_eq!(decide(&Facts { last_trained: None, ..ready() }, NOW), Decision::Train { fetch: true });
    }

    /// A training whose labels were decided by older rules (`labels::RULES`)
    /// is followed by one without waiting for the week; the rest still holds.
    #[test]
    fn new_label_rules_do_not_wait_for_the_week() {
        let recent = Facts { last_trained: Some(NOW - 3600), ..ready() };
        assert_eq!(decide(&recent, NOW), Decision::Wait(Wait::Recent));
        assert_eq!(decide(&Facts { outdated: true, ..recent.clone() }, NOW), Decision::Train { fetch: true });
        assert_eq!(decide(&Facts { outdated: true, power: power(Some(true), Some(false), Some(false), Some(false)), ..recent }, NOW), Decision::Wait(Wait::InUse));
    }

    /// Each condition alone holds it back, its reason said.
    #[test]
    fn each_condition_holds_it_back() {
        let warm = Power { battery_tenths: Some(HOT_TENTHS + 1), ..ready().power };
        let cases: Vec<(Facts, Wait)> = vec![
            (Facts { enabled: false, ..ready() }, Wait::Off),
            (Facts { running: true, ..ready() }, Wait::Running),
            (Facts { last_trained: Some(NOW - WEEK + 60), ..ready() }, Wait::Recent),
            (Facts { focus: Some(true), ..ready() }, Wait::Focus),
            (Facts { power: power(Some(false), Some(false), Some(true), Some(false)), ..ready() }, Wait::Battery),
            (Facts { power: power(Some(true), Some(true), Some(true), Some(false)), ..ready() }, Wait::PowerSaver),
            (Facts { power: power(Some(true), Some(false), Some(false), Some(false)), ..ready() }, Wait::InUse),
            (Facts { power: warm, ..ready() }, Wait::Hot),
            (Facts { unmetered_only: true, power: power(Some(true), Some(false), Some(true), Some(true)), ..ready() }, Wait::Metered),
        ];
        for (facts, why) in cases {
            assert_eq!(decide(&facts, NOW), Decision::Wait(why), "{facts:?}");
        }
    }

    /// An unknown fact counts as not holding, but the network's cost and the
    /// battery's warmth: unsaid, the network counts as unmetered, the battery
    /// as cool. Any network by default; kept to unmetered ones, a metered
    /// network waits and an unknown one does not.
    #[test]
    fn unknown_counts_as_no_but_for_the_network_and_the_warmth() {
        let with = |p: Power| Facts { power: p, ..ready() };
        assert_eq!(decide(&Facts { focus: None, ..ready() }, NOW), Decision::Wait(Wait::Focus));
        assert_eq!(decide(&with(power(None, Some(false), Some(true), Some(false))), NOW), Decision::Wait(Wait::Battery));
        assert_eq!(decide(&with(power(Some(true), None, Some(true), Some(false))), NOW), Decision::Wait(Wait::PowerSaver));
        assert_eq!(decide(&with(power(Some(true), Some(false), None, Some(false))), NOW), Decision::Wait(Wait::InUse));
        assert_eq!(decide(&with(power(Some(true), Some(false), Some(true), Some(true))), NOW), Decision::Train { fetch: true }, "any network: metered too");
        assert_eq!(decide(&with(power(Some(true), Some(false), Some(true), None)), NOW), Decision::Train { fetch: true });
        let unmetered = |p: Power| Facts { unmetered_only: true, power: p, ..ready() };
        assert_eq!(decide(&unmetered(power(Some(true), Some(false), Some(true), Some(true))), NOW), Decision::Wait(Wait::Metered));
        assert_eq!(decide(&unmetered(power(Some(true), Some(false), Some(true), None)), NOW), Decision::Train { fetch: true }, "unsaid: unmetered");
        assert_eq!(decide(&unmetered(power(Some(true), Some(false), Some(true), Some(false))), NOW), Decision::Train { fetch: true });
        assert_eq!(decide(&with(Power { battery_tenths: Some(HOT_TENTHS), ..ready().power }), NOW), Decision::Train { fetch: true }, "40 °C is not above");
        assert_eq!(decide(&with(Power::default()), NOW), Decision::Wait(Wait::Battery), "nothing known: no");
    }

    /// The settings as each device keeps them: on unless said, never an
    /// older device's shared `train_by_itself` (C1, review of 7 November
    /// 2026); any network unless "unmetered".
    #[test]
    fn the_device_s_own_settings() {
        use sioul_core::config::SpamSettings;
        assert!(SpamSettings::default().trains_here());
        assert!(SpamSettings { train_by_itself: Some(false), ..SpamSettings::default() }.trains_here(), "the shared switch never decides");
        assert!(SpamSettings { train_by_itself: Some(false), train_here: Some(true), ..SpamSettings::default() }.trains_here());
        assert!(!SpamSettings { train_here: Some(false), ..SpamSettings::default() }.trains_here());
        let any = SpamSettings::default();
        assert!(any.downloads_on(Some(true)) && any.downloads_on(None) && !any.unmetered_only());
        let unmetered = SpamSettings { train_network: Some("unmetered".into()), ..SpamSettings::default() };
        assert!(!unmetered.downloads_on(Some(true)) && unmetered.downloads_on(Some(false)) && unmetered.downloads_on(None));
        let facts = facts(&Dirs::under(&std::env::temp_dir().join("sioul-auto-none")), &unmetered, false, false, NOW, Power::default);
        assert!(facts.enabled && facts.unmetered_only);
    }

    /// Once a week at most after the last training, by hand or by itself;
    /// after a run that stopped, a quarter of an hour; after one that failed
    /// or vanished, a day; a run that trained since says nothing.
    #[test]
    fn once_a_week_and_not_again_too_soon() {
        let run = |started: i64, ended: Option<i64>, outcome: Option<Outcome>| Run { started, job: "j".into(), fetched: true, ended, outcome };
        let after = |r: Run| decide(&Facts { last_run: Some(r), ..ready() }, NOW);
        assert_eq!(after(run(NOW - 600, Some(NOW - 300), Some(Outcome::Stopped))), Decision::Wait(Wait::Retry));
        assert_eq!(after(run(NOW - 3600, Some(NOW - AFTER_STOP - 1), Some(Outcome::Stopped))), Decision::Train { fetch: true });
        assert_eq!(after(run(NOW - 3600, Some(NOW - 3000), Some(Outcome::Failed))), Decision::Wait(Wait::Retry));
        assert_eq!(after(run(NOW - AFTER_FAILURE - 1, Some(NOW - AFTER_FAILURE), Some(Outcome::Failed))), Decision::Train { fetch: true });
        assert_eq!(after(run(NOW - 3600, None, None)), Decision::Wait(Wait::Retry), "vanished: as failed");
        // Trained since that run started (by hand, or the run itself): the week counts.
        let trained = Facts { last_trained: Some(NOW - 3000), last_run: Some(run(NOW - 3600, Some(NOW - 3000), Some(Outcome::Replaced))), ..ready() };
        assert_eq!(decide(&trained, NOW), Decision::Wait(Wait::Recent));
        let old = Facts { last_trained: Some(NOW - WEEK - 10), last_run: Some(run(NOW - WEEK - 100, Some(NOW - WEEK - 10), Some(Outcome::Kept))), ..ready() };
        assert_eq!(decide(&old, NOW), Decision::Train { fetch: true });
    }

    /// The cheap facts first: the system is asked only when a training waits
    /// on its power alone; then once.
    #[test]
    fn the_system_is_asked_last() {
        let dir = std::env::temp_dir().join(format!("sioul-auto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dirs = Dirs::under(&dir);
        let asked = Cell::new(0);
        let spam = sioul_core::config::SpamSettings::default();
        // Switched off: nothing to ask.
        let off = sioul_core::config::SpamSettings { train_here: Some(false), ..spam.clone() };
        let f = facts(&dirs, &off, false, false, NOW, || {
            asked.set(asked.get() + 1);
            Power::default()
        });
        assert_eq!((asked.get(), decide(&f, NOW)), (0, Decision::Wait(Wait::Off)));
        // On, no table here: the first training, the system asked once.
        let f = facts(&dirs, &spam, false, false, NOW, || {
            asked.set(asked.get() + 1);
            power(Some(true), Some(false), Some(true), None)
        });
        assert_eq!((trainer(&dirs, "desk"), asked.get(), decide(&f, NOW)), (Trainer::Nobody, 1, Decision::Train { fetch: true }));
        // The record of a run kept and read back, its end written by its job.
        let run = Run { started: NOW - 100, job: "20261007-031200-train-abcd".into(), fetched: false, ended: None, outcome: None };
        run.save(&dirs).unwrap();
        Run::finish(&dirs, "another", Outcome::Failed, NOW).unwrap();
        assert_eq!(Run::load(&dirs), Some(run.clone()), "another job's end is not this run's");
        Run::finish(&dirs, &run.job, Outcome::Replaced, NOW).unwrap();
        assert_eq!(Run::load(&dirs).map(|r| (r.ended, r.outcome)), Some((Some(NOW), Some(Outcome::Replaced))));
        // A job whose process holds its lock runs; one whose lock nobody holds does not.
        let jobs = dirs.state.join("jobs");
        std::fs::create_dir_all(&jobs).unwrap();
        let lock = std::fs::File::create(jobs.join("x.lock")).unwrap();
        assert!(!a_job_runs(&dirs));
        lock.lock().unwrap();
        assert!(a_job_runs(&dirs));
        drop(lock);
        assert!(threads() >= 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A run by itself stops once unplugged or saving power, or after two
    /// readings in a row that cannot tell; never while its power allows it,
    /// and the watch ends with the run. Past `LONGEST` of training, or with
    /// the battery above 40 °C, the guard stops it.
    #[test]
    fn the_run_stops_when_the_power_changes_or_the_guard_says() {
        let mains = power(Some(true), Some(false), None, None);
        let watched = |readings: Vec<Power>, ran: Option<Duration>| {
            let left = RefCell::new(readings);
            let (stopped, asked) = (Cell::new(None), Cell::new(0));
            let next = || {
                asked.set(asked.get() + 1);
                let mut left = left.borrow_mut();
                if left.is_empty() { mains } else { left.remove(0) }
            };
            // The run ends after ten readings, unless stopped before.
            watch_supply(next, |halt| stopped.set(Some(halt)), || asked.get() >= 10, || ran, Duration::from_millis(1));
            (stopped.get(), asked.get())
        };
        assert_eq!(watched(vec![mains; 3], None), (None, 10), "plugged in all along");
        assert_eq!(watched(vec![mains, mains, power(Some(false), Some(false), None, None)], None), (Some(Halt::Supply), 3), "unplugged");
        assert_eq!(watched(vec![mains, power(Some(true), Some(true), None, None)], None), (Some(Halt::Supply), 2), "saving power");
        assert_eq!(watched(vec![mains, Power::default(), mains, Power::default(), Power::default()], None), (Some(Halt::Supply), 5), "twice unknown in a row");
        assert_eq!(watched(vec![Power::default(), mains, Power::default(), mains], None), (None, 10), "once unknown, then known again");
        assert_eq!(watched(vec![mains, Power { battery_tenths: Some(415), ..mains }], None), (Some(Halt::Hot), 2), "too warm");
        assert_eq!(watched(vec![mains; 3], Some(LONGEST + Duration::from_secs(1))), (Some(Halt::Long), 0), "too long");
        assert_eq!(watched(vec![mains; 3], Some(LONGEST - Duration::from_secs(1))), (None, 10), "within the time");
        assert!(Halt::Long.guards() && !Halt::Hot.guards() && !Halt::Supply.guards(), "heat stops the run, never guards");
    }

    /// The guard turns this device's training off in its own settings, and
    /// the Porch says so once, until put away. A first training says it is
    /// learning while it runs, then how long it took; one that made no table
    /// says nothing after.
    #[test]
    fn the_guard_and_the_first_training_are_said_once() {
        let dir = std::env::temp_dir().join(format!("sioul-auto-guard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dirs = Dirs::under(&dir);
        let config = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&config, "language = \"en\"\n").unwrap();
        // The first training, running, then done in 270 s.
        begin(&dirs, true, NOW);
        assert_eq!(Notice::shown(&dirs, true).map(|n| n.kind), Some(NoticeKind::FirstRunning));
        assert_eq!(Notice::shown(&dirs, false), None, "said only while it runs");
        end(&dirs, true, 270, NOW + 270);
        assert_eq!(Notice::shown(&dirs, false).map(|n| (n.kind, n.seconds)), Some((NoticeKind::FirstDone, 270)));
        Notice::put_away(&dirs).unwrap();
        assert_eq!(Notice::shown(&dirs, false), None, "put away: said no more");
        // Not the first: nothing said; a first that made no table: forgotten.
        begin(&dirs, false, NOW);
        assert_eq!(Notice::shown(&dirs, true), None);
        begin(&dirs, true, NOW);
        end(&dirs, false, 30, NOW + 30);
        assert_eq!(Notice::load(&dirs), None);
        // Heat: nothing kept, nothing said.
        guard(&dirs, Halt::Hot, NOW).unwrap();
        assert_eq!((Guard::load(&dirs), Notice::load(&dirs)), (None, None));
        // The guard: the switch left on, the device marked as not learning, said once.
        guard(&dirs, Halt::Long, NOW).unwrap();
        let written = sioul_core::config::Config::load(&config).unwrap();
        assert!(written.spam.trains_here(), "the switch stays on: {}", std::fs::read_to_string(&config).unwrap());
        assert_eq!(Guard::load(&dirs), Some(Guard { at: NOW, messages: 0 }));
        assert_eq!(Notice::shown(&dirs, false).map(|n| n.kind), Some(NoticeKind::Guarded(Halt::Long)));
        Notice::put_away(&dirs).unwrap();
        assert_eq!(Notice::shown(&dirs, false), None);
        // A table made here at last (Train now, or the mail changed a lot): the mark goes.
        end(&dirs, true, 30, NOW + 30);
        assert_eq!(Guard::load(&dirs), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// F2 (review of 7 November 2026): the guard stops a training too long
    /// and marks the device as not learning; it tries again once a week to
    /// download, and trains only once its mail changed a lot (a quarter more
    /// or less), or when you ask (the mark cleared).
    #[test]
    fn the_guard_waits_for_the_mail_to_change_a_lot() {
        assert!(!changed_a_lot(12_000, 12_500) && !changed_a_lot(12_000, 9_500));
        assert!(changed_a_lot(12_000, 15_000) && changed_a_lot(12_000, 9_000) && changed_a_lot(0, 1));
        let guard = Guard { at: NOW - 3600, messages: 12_000 };
        let run = |started: i64, ended: i64, outcome: Outcome| Run { started, job: "j".into(), fetched: true, ended: Some(ended), outcome: Some(outcome) };
        // The run the guard stopped ended as the guard said so: a week before the next download.
        let stopped = Facts { guard: Some(guard), last_run: Some(run(NOW - 4000, NOW - 3600, Outcome::Stopped)), ..ready() };
        assert_eq!(decide(&stopped, NOW), Decision::Wait(Wait::Guarded));
        // A week on: a run by itself again; its training waits for the mail (`waits_for_mail`).
        let week = Facts { guard: Some(guard), last_run: Some(run(NOW - WEEK - 10, NOW - WEEK, Outcome::Waited)), ..ready() };
        assert_eq!(decide(&week, NOW), Decision::Train { fetch: true });
        let dir = std::env::temp_dir().join(format!("sioul-auto-mail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dirs = Dirs::under(&dir);
        assert!(!waits_for_mail(&dirs), "no guard: it trains");
        assert_eq!(messages(&dirs), 0, "no corpus here");
        Guard { at: NOW, messages: 1_000 }.save(&dirs).unwrap();
        assert!(!waits_for_mail(&dirs) && Guard::load(&dirs).is_none(), "far less mail now: changed a lot, the mark gone");
        Guard { at: NOW, messages: messages(&dirs) }.save(&dirs).unwrap();
        assert!(waits_for_mail(&dirs) && Guard::load(&dirs).is_some(), "the same mail: it waits");
        Guard::clear(&dirs);
        assert!(!waits_for_mail(&dirs), "you asked: it trains");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// F2: a device learns, as its entry says, only with its switch on, a
    /// table it made itself in use, and the guard not standing.
    #[test]
    fn a_device_learns_with_a_table_of_its_own() {
        use sioul_core::config::SpamSettings;
        let dir = std::env::temp_dir().join(format!("sioul-auto-learns-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dirs = Dirs::under(&dir);
        let topics = dir.join("topics-table.bin");
        let on = SpamSettings::default();
        assert!(!learns(&dirs, &topics, &on, "desk"), "no table: it does not learn");
        let table = |device: &str| {
            use sioul_core::spam::{features, table, tokenize};
            let mut weights = vec![0.0; features::N];
            weights[0] = 2.0;
            let meta = table::Meta { device: device.into(), ..table::Meta::default() };
            let mut t = table::Table { tokenizer: tokenize::TOKENIZER, features: features::FEATURES, dim: 4, minn: 3, maxn: 4, bucket: 8, words: vec![(table::word_hash("gratuit"), 3.0)], buckets: vec![0.5; 8], weights, means: vec![0.0; features::N], bias: -0.5, text_mean: 0.5, platt_a: -2.0, platt_b: 0.0, meta };
            t.sort();
            t
        };
        std::fs::create_dir_all(&dirs.data).unwrap();
        std::fs::write(dirs.table(), table("phone").to_bytes()).unwrap();
        assert!(!learns(&dirs, &topics, &on, "desk"), "another device's table: it does not learn");
        std::fs::write(dirs.table(), table("desk").to_bytes()).unwrap();
        assert!(learns(&dirs, &topics, &on, "desk"));
        assert!(!learns(&dirs, &topics, &SpamSettings { train_here: Some(false), ..on.clone() }, "desk"), "its switch off");
        Guard { at: NOW, messages: 1 }.save(&dirs).unwrap();
        assert!(!learns(&dirs, &topics, &on, "desk"), "the guard standing");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// F5: Stop, or "Train here" turned off, reaches the run under way
    /// through its job's stop file; nothing when none runs.
    #[test]
    fn a_run_under_way_is_asked_to_stop() {
        let dir = std::env::temp_dir().join(format!("sioul-auto-stop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dirs = Dirs::under(&dir);
        assert!(!ask_stop(&dirs), "none runs");
        Run { started: NOW, job: "phone-1".into(), fetched: true, ended: None, outcome: None }.save(&dirs).unwrap();
        assert!(!stop_asked(&dirs, "phone-1"));
        assert!(ask_stop(&dirs) && stop_asked(&dirs, "phone-1"));
        stop_forgotten(&dirs, "phone-1");
        assert!(!stop_asked(&dirs, "phone-1"));
        Run::finish(&dirs, "phone-1", Outcome::Stopped, NOW + 1).unwrap();
        assert!(!ask_stop(&dirs), "ended: nothing to stop");
        let _ = std::fs::remove_dir_all(dir);
    }
}
