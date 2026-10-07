// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Training again by itself (docs/spam-filter.md, "Training by itself"): on
//! the computer that made the table in use, once a week at most after the
//! last training (by hand or by itself), when it is on mains power, not
//! saving power, idle for 15 minutes or locked, and no focus session runs;
//! on a metered connection, without downloading first. A job run apart
//! (`sioul spam train --by-itself`), at the lowest priority, on half the
//! cores, with the usual rule for replacing the table; stopped through its
//! stop file when the computer is unplugged or starts saving power, to try
//! again later. Never a notification: the settings say when it last trained
//! by itself, and whether its table replaced the one in use.
//!
//! [`decide`] is the rule, a pure function of what is known ([`Facts`]); an
//! unknown fact counts as not holding. [`facts`] reads them, the cheap ones
//! first: the system is asked about its power and its session only when
//! nothing else holds a training back. What the last run by itself did is
//! kept beside the last training's summary ([`Run`], `auto.toml`). The
//! window asks at its minute, `sioul watch` at its own; the command line
//! starts the job.

use crate::Dirs;
use serde::{Deserialize, Serialize};
use sioul_sync::power::Power;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The least time between two trainings, by hand or by itself.
pub const WEEK: i64 = 7 * 86_400;
/// After a run by itself stopped (unplugged, saving power): the soonest it tries again.
pub const AFTER_STOP: i64 = 15 * 60;
/// After one that failed, or whose process vanished: the soonest it tries again.
pub const AFTER_FAILURE: i64 = 86_400;
/// How often a run by itself asks whether its power still allows it.
pub const SUPPLY_EVERY: Duration = Duration::from_secs(30);

/// Who made the table in use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trainer {
    /// This computer: it trains by itself.
    Here,
    /// Another device, by its name: that one does.
    Elsewhere(String),
    /// No table yet, or none this Sioul can read: a training by hand comes first.
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
    /// The setting: "Train again by itself" (`[spam] train_by_itself`, on unless said).
    pub enabled: bool,
    pub trainer: Trainer,
    /// When the last training ended, by hand or by itself (`trained.toml`).
    pub last_trained: Option<i64>,
    /// The last run by itself.
    pub last_run: Option<Run>,
    /// A training runs: the window's, or a job's (`a_job_runs`).
    pub running: bool,
    /// A focus session runs.
    pub focus: Option<bool>,
    /// What the computer says of itself (`sioul_sync::power`).
    pub power: Power,
}

/// Why it does not train now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// The setting is off.
    Off,
    /// No table yet: a training by hand comes first.
    NoTable,
    /// Another computer made the table in use: that one trains.
    Elsewhere,
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
}

/// What to do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Train, downloading first or not (a metered connection, or one it cannot tell).
    Train { fetch: bool },
    Wait(Wait),
}

/// The rule: every condition must hold, an unknown one counting as not
/// holding; the cheap ones are looked at first (`facts` asks the system
/// only when a training waits on its power alone).
pub fn decide(f: &Facts, now: i64) -> Decision {
    let wait = |why| Decision::Wait(why);
    if !f.enabled {
        return wait(Wait::Off);
    }
    match f.trainer {
        Trainer::Nobody => return wait(Wait::NoTable),
        Trainer::Elsewhere(_) => return wait(Wait::Elsewhere),
        Trainer::Here => {}
    }
    if f.running {
        return wait(Wait::Running);
    }
    if f.last_trained.is_some_and(|at| now - at < WEEK) {
        return wait(Wait::Recent);
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
    Decision::Train { fetch: f.power.metered == Some(false) }
}

/// What is known now: the setting (`enabled`), the table in use and the
/// files of the last trainings, the jobs, a focus session (`focus`:
/// `sioul_core::timelog::running`); then, only when nothing else holds a
/// training back, what the system says (`power`, asked once). `here`: this
/// computer's name, as a table records it (`sioul_sync::lease::host_name`);
/// `window_trains`: the window's own training runs ("Train now").
pub fn facts(dirs: &Dirs, enabled: bool, here: &str, window_trains: bool, focus: bool, now: i64, power: impl FnOnce() -> Power) -> Facts {
    let mut facts = Facts {
        enabled,
        trainer: trainer(dirs, here),
        last_trained: crate::train::last(dirs).map(|s| s.trained_at).filter(|at| *at > 0),
        last_run: Run::load(dirs),
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

/// fastText's threads for a run by itself: half the cores, one at least.
pub fn threads() -> u32 {
    std::thread::available_parallelism().map_or(1, |n| u32::try_from(n.get() / 2).unwrap_or(1).max(1))
}

/// While a run by itself goes on: its power asked every `every` (`supply`);
/// unplugged or saving power, or twice in a row unable to tell, `stop` is
/// called once and the watch ends; it ends too as soon as `done` says the
/// run did.
pub fn watch_supply(supply: impl Fn() -> Power, stop: impl FnOnce(), done: impl Fn() -> bool, every: Duration) {
    let mut unknown = false;
    loop {
        if done() {
            return;
        }
        let power = supply();
        let away = power.on_mains == Some(false) || power.power_saver == Some(true);
        let unsure = !away && !power.supplied();
        if away || (unsure && unknown) {
            stop();
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
        Power { on_mains, power_saver, idle, metered }
    }

    /// Everything allows it: the training computer, set on, a week and more
    /// since the last training, nothing running, no focus, on mains power,
    /// not saving power, idle, a connection not metered.
    fn ready() -> Facts {
        Facts {
            enabled: true,
            trainer: Trainer::Here,
            last_trained: Some(NOW - WEEK - 1),
            last_run: None,
            running: false,
            focus: Some(false),
            power: power(Some(true), Some(false), Some(true), Some(false)),
        }
    }

    #[test]
    fn it_trains_when_everything_allows_it() {
        assert_eq!(decide(&ready(), NOW), Decision::Train { fetch: true });
        // Never trained here (no `trained.toml`), the table this computer's: it may.
        assert_eq!(decide(&Facts { last_trained: None, ..ready() }, NOW), Decision::Train { fetch: true });
    }

    /// Each condition alone holds it back, its reason said.
    #[test]
    fn each_condition_holds_it_back() {
        let cases: Vec<(Facts, Wait)> = vec![
            (Facts { enabled: false, ..ready() }, Wait::Off),
            (Facts { trainer: Trainer::Nobody, ..ready() }, Wait::NoTable),
            (Facts { trainer: Trainer::Elsewhere("desk".into()), ..ready() }, Wait::Elsewhere),
            (Facts { running: true, ..ready() }, Wait::Running),
            (Facts { last_trained: Some(NOW - WEEK + 60), ..ready() }, Wait::Recent),
            (Facts { focus: Some(true), ..ready() }, Wait::Focus),
            (Facts { power: power(Some(false), Some(false), Some(true), Some(false)), ..ready() }, Wait::Battery),
            (Facts { power: power(Some(true), Some(true), Some(true), Some(false)), ..ready() }, Wait::PowerSaver),
            (Facts { power: power(Some(true), Some(false), Some(false), Some(false)), ..ready() }, Wait::InUse),
        ];
        for (facts, why) in cases {
            assert_eq!(decide(&facts, NOW), Decision::Wait(why), "{facts:?}");
        }
    }

    /// An unknown fact counts as not holding; a metered connection, or one it
    /// cannot tell, trains on the corpus as it is.
    #[test]
    fn unknown_counts_as_no() {
        let with = |p: Power| Facts { power: p, ..ready() };
        assert_eq!(decide(&Facts { focus: None, ..ready() }, NOW), Decision::Wait(Wait::Focus));
        assert_eq!(decide(&with(power(None, Some(false), Some(true), Some(false))), NOW), Decision::Wait(Wait::Battery));
        assert_eq!(decide(&with(power(Some(true), None, Some(true), Some(false))), NOW), Decision::Wait(Wait::PowerSaver));
        assert_eq!(decide(&with(power(Some(true), Some(false), None, Some(false))), NOW), Decision::Wait(Wait::InUse));
        assert_eq!(decide(&with(power(Some(true), Some(false), Some(true), Some(true))), NOW), Decision::Train { fetch: false });
        assert_eq!(decide(&with(power(Some(true), Some(false), Some(true), None)), NOW), Decision::Train { fetch: false });
        assert_eq!(decide(&with(Power::default()), NOW), Decision::Wait(Wait::Battery), "nothing known: no");
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
        // No table here: nothing to ask.
        let f = facts(&dirs, true, "desk", false, false, NOW, || {
            asked.set(asked.get() + 1);
            Power::default()
        });
        assert_eq!((f.trainer.clone(), asked.get(), decide(&f, NOW)), (Trainer::Nobody, 0, Decision::Wait(Wait::NoTable)));
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
    /// and the watch ends with the run.
    #[test]
    fn the_run_stops_when_the_power_changes() {
        let mains = power(Some(true), Some(false), None, None);
        let watched = |readings: Vec<Power>| {
            let left = RefCell::new(readings);
            let (stopped, asked) = (Cell::new(false), Cell::new(0));
            let next = || {
                asked.set(asked.get() + 1);
                let mut left = left.borrow_mut();
                if left.is_empty() { mains } else { left.remove(0) }
            };
            // The run ends after ten readings, unless stopped before.
            watch_supply(next, || stopped.set(true), || asked.get() >= 10, Duration::from_millis(1));
            (stopped.get(), asked.get())
        };
        assert_eq!(watched(vec![mains; 3]), (false, 10), "plugged in all along");
        assert_eq!(watched(vec![mains, mains, power(Some(false), Some(false), None, None)]), (true, 3), "unplugged");
        assert_eq!(watched(vec![mains, power(Some(true), Some(true), None, None)]), (true, 2), "saving power");
        assert_eq!(watched(vec![mains, Power::default(), mains, Power::default(), Power::default()]), (true, 5), "twice unknown in a row");
        assert_eq!(watched(vec![Power::default(), mains, Power::default(), mains]), (false, 10), "once unknown, then known again");
    }
}
