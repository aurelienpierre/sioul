// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The spam filter's training by itself, from the command line's side
//! (`sioul_learn::auto` decides; docs/spam-filter.md, "Training by itself"):
//! its job started apart, at once (`start`: for the window's minute, through
//! `sioul spam train --by-itself`, and for `sioul watch`'s own minute,
//! `every_minute`), then run (`run`: the lowest priority, its power watched,
//! its end kept).

use super::{jobs, long, report};
use crate::Session;
use sioul_learn::Dirs;
use sioul_learn::auto::{self, Decision, Outcome, Run};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A run by itself, started apart (`jobs`): downloading first or not, on
/// `threads` (half the cores unless said), its start kept (`auto::Run`)
/// before its process starts.
pub(crate) fn start(s: &Session, fetch: bool, threads: Option<u32>) -> Result<jobs::Job, String> {
    let threads = threads.unwrap_or_else(auto::threads);
    let mut args = vec!["--by-itself".to_string(), "--threads".to_string(), threads.to_string()];
    if !fetch {
        args.push("--no-fetch".to_string());
    }
    let job = jobs::create(s, "train", args)?;
    Run { started: job.created, job: job.id.clone(), fetched: fetch, ended: None, outcome: None }.save(&Dirs::standard())?;
    jobs::spawn(s, job)
}

/// The run itself, as its job: the lowest priority for every thread it
/// starts, its power asked every half minute (unplugged or saving power: its
/// own stop file, and it stops at its next step), its end kept for the
/// settings and the next decision.
pub(crate) fn run(s: &Session, dirs: &Dirs, id: &str, ask: &report::TrainAsk, json: bool) -> Result<(), String> {
    lowest_priority();
    let done = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (done, id) = (Arc::clone(&done), id.to_string());
        std::thread::Builder::new().name("spam-supply".into()).spawn(move || {
            auto::watch_supply(
                sioul_sync::power::supply,
                || {
                    let _ = jobs::stop(&id);
                },
                || done.load(Ordering::Relaxed),
                auto::SUPPLY_EVERY,
            );
        })
    };
    let mut replaced = None;
    let result = long(s, json, Some(id), |progress, cancel| {
        let report = report::train(s, dirs, ask, progress, cancel)?;
        replaced = report.data["summary"]["replaced"].as_bool();
        Ok(report)
    });
    done.store(true, Ordering::Relaxed);
    if let Ok(watcher) = watcher {
        let _ = watcher.join();
    }
    let stopped = jobs::look(id).is_ok_and(|job| job.state == jobs::State::Stopped);
    let _ = Run::finish(dirs, id, outcome(&result, replaced, stopped), jiff::Timestamp::now().as_second());
    result
}

/// How a run by itself ended, from its result, whether its table replaced
/// the one in use, and whether its job says it stopped.
pub(crate) fn outcome(result: &Result<(), String>, replaced: Option<bool>, stopped: bool) -> Outcome {
    match (result, replaced) {
        (Ok(()), Some(true)) => Outcome::Replaced,
        (Ok(()), _) => Outcome::Kept,
        (Err(_), _) if stopped => Outcome::Stopped,
        (Err(_), _) => Outcome::Failed,
    }
}

/// `sioul watch`'s own minute, as the window's: a run by itself started
/// when everything allows it (`auto::decide`), the system asked only when
/// nothing else holds it back. Its settings read again each minute; silent.
pub(crate) fn every_minute(s: &Session) {
    let (path, language) = (s.config_path.clone(), s.tr.language().to_string());
    let _ = std::thread::Builder::new().name("spam-by-itself".into()).spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(60));
            let config = sioul_core::config::Config::load(&path).unwrap_or_default();
            let session = Session { config, config_path: path.clone(), tr: sioul_core::i18n::Translator::new(&language) };
            let (dirs, now) = (Dirs::standard(), jiff::Timestamp::now().as_second());
            let focus = sioul_core::timelog::running().is_some();
            let facts = auto::facts(&dirs, session.config.spam.trains_by_itself(), &sioul_sync::lease::host_name(), false, focus, now, sioul_sync::power::read);
            if let Decision::Train { fetch } = auto::decide(&facts, now) {
                let _ = start(&session, fetch, None);
            }
        }
    });
}

/// The lowest priority this system has, for this process and every thread
/// it starts from now (fastText's): nice 19, and the idle I/O class on
/// Linux; background mode on Windows (its processor, its disk, its memory);
/// a throttled disk on a Mac.
fn lowest_priority() {
    #[cfg(unix)]
    // SAFETY: setpriority only reads its arguments; 0 is this process (on
    // Linux this thread, whose new threads take it).
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 19);
    }
    #[cfg(target_os = "linux")]
    // SAFETY: ioprio_set only reads its arguments: this thread
    // (IOPRIO_WHO_PROCESS, 0), the idle class (3 << 13), which its new threads take.
    unsafe {
        libc::syscall(libc::SYS_ioprio_set, 1, 0, 3 << 13);
    }
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> isize;
            fn SetPriorityClass(process: isize, class: u32) -> i32;
        }
        // SAFETY: this process's pseudo-handle, and PROCESS_MODE_BACKGROUND_BEGIN.
        unsafe {
            SetPriorityClass(GetCurrentProcess(), 0x0010_0000);
        }
    }
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
        }
        // SAFETY: it only reads its arguments: the disk (IOPOL_TYPE_DISK), this
        // process (IOPOL_SCOPE_PROCESS), throttled (IOPOL_THROTTLE).
        unsafe {
            setiopolicy_np(0, 0, 3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trained (its table in use, or kept), stopped, or failed.
    #[test]
    fn how_a_run_by_itself_ended() {
        let (ok, err) = (Ok(()), Err("x".to_string()));
        assert_eq!(outcome(&ok, Some(true), false), Outcome::Replaced);
        assert_eq!(outcome(&ok, Some(false), false), Outcome::Kept);
        assert_eq!(outcome(&ok, None, false), Outcome::Kept);
        assert_eq!(outcome(&err, None, true), Outcome::Stopped);
        assert_eq!(outcome(&err, None, false), Outcome::Failed);
    }
}
