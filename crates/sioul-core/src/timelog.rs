// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Time spent on tasks, and the focus session running now.
//!
//! A session is a stretch of time given to one task, usually through the focus
//! timer. Sessions are kept in plain files, one per month
//! (`$XDG_DATA_HOME/sioul/time/2026-10.toml`), not in CalDAV: they are your
//! record, other devices need not carry them, and a task's file stays small.
//! The session running now is kept on disk too (`time/running.toml`), so
//! closing the window does not lose it.
//!
//! Time can also be noted by hand, for a task or for a project alone (a
//! meeting, a call); billable time of a project carries, once billed, the
//! number of its invoice, so it is never billed twice.

use crate::config::data_dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Time given to one task, or to a project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// The task's UID; "" for time given to a project alone.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task: String,
    /// The project (a case's id) it counts for; "" for the task's own case.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub project: String,
    /// Not to be billed, though its project is billable.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unbilled: bool,
    /// The invoice it was billed on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub invoice: String,
    /// When it began, Unix seconds.
    pub start: i64,
    pub minutes: u32,
    /// The task was marked done at its end.
    #[serde(default)]
    pub done: bool,
    /// A word on what was done, if you left one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Month {
    #[serde(default, rename = "session")]
    sessions: Vec<Session>,
}

/// Where the sessions are kept.
pub fn folder() -> PathBuf {
    data_dir().join("time")
}

fn month_file(dir: &Path, start: i64) -> PathBuf {
    let month = jiff::Timestamp::from_second(start).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m").to_string()).unwrap_or_else(|_| "unknown".into());
    dir.join(format!("{month}.toml"))
}

fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let temporary = path.with_extension("toml.new");
    std::fs::write(&temporary, text).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)
}

/// Adds a session to its month's file, in `dir`.
pub fn record_in(dir: &Path, session: &Session) -> Result<(), String> {
    let path = month_file(dir, session.start);
    // A month not begun yet starts empty; one there that cannot be read is left
    // alone: written over, its time (and what was billed of it) would be lost.
    let mut month: Month = match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Month::default(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    month.sessions.push(session.clone());
    write_atomically(&path, &toml::to_string(&month).map_err(|e| e.to_string())?)
}

/// Adds a session to its month's file.
pub fn record(session: &Session) -> Result<(), String> {
    record_in(&folder(), session)
}

/// Every session kept in `dir`, oldest first.
pub fn sessions_in(dir: &Path) -> Vec<Session> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml") && p.file_stem().is_some_and(|s| s.to_string_lossy().chars().next().is_some_and(|c| c.is_ascii_digit())))
        .collect();
    files.sort();
    let mut all: Vec<Session> = files.iter().filter_map(|p| std::fs::read_to_string(p).ok()).filter_map(|t| toml::from_str::<Month>(&t).ok()).flat_map(|m| m.sessions).collect();
    all.sort_by_key(|s| s.start);
    all
}

/// Every session, oldest first.
pub fn sessions() -> Vec<Session> {
    sessions_in(&folder())
}

impl Session {
    /// What tells it from the others: when it began, and for what.
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.start, if self.task.is_empty() { &self.project } else { &self.task }, self.minutes)
    }
}

/// Changes the sessions `keys` names with `change`, in their month files in
/// `dir`; returns how many were changed.
pub fn update_in(dir: &Path, keys: &[String], change: impl Fn(&mut Session)) -> Result<usize, String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(0) };
    let mut changed = 0;
    for path in entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml") && p.file_name().is_some_and(|n| n != "running.toml")) {
        let Some(mut month) = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str::<Month>(&t).ok()) else { continue };
        let mut here = 0;
        for session in month.sessions.iter_mut().filter(|s| keys.contains(&s.key())) {
            change(session);
            here += 1;
        }
        if here > 0 {
            write_atomically(&path, &toml::to_string(&month).map_err(|e| e.to_string())?)?;
            changed += here;
        }
    }
    Ok(changed)
}

/// Takes out the sessions `keys` names (time noted by mistake).
pub fn remove_in(dir: &Path, keys: &[String]) -> Result<usize, String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(0) };
    let mut removed = 0;
    for path in entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml") && p.file_name().is_some_and(|n| n != "running.toml")) {
        let Some(mut month) = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str::<Month>(&t).ok()) else { continue };
        let before = month.sessions.len();
        month.sessions.retain(|s| !keys.contains(&s.key()));
        if month.sessions.len() != before {
            removed += before - month.sessions.len();
            write_atomically(&path, &toml::to_string(&month).map_err(|e| e.to_string())?)?;
        }
    }
    Ok(removed)
}

/// Minutes spent on each task, by UID, between two instants (Unix seconds).
pub fn spent(sessions: &[Session], from: i64, to: i64) -> BTreeMap<String, u32> {
    let mut out: BTreeMap<String, u32> = BTreeMap::new();
    for s in sessions.iter().filter(|s| s.start >= from && s.start < to) {
        let minutes = out.entry(s.task.clone()).or_default();
        *minutes = minutes.saturating_add(s.minutes);
    }
    out
}

/// The focus session running now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Running {
    pub task: String,
    /// When it began, Unix seconds.
    pub start: i64,
    /// The minutes you chose.
    pub planned: u32,
    /// When it was paused, if it is.
    #[serde(default)]
    pub paused_at: Option<i64>,
    /// Seconds spent paused, before the current pause.
    #[serde(default)]
    pub paused: i64,
}

impl Running {
    /// Seconds of focus so far, pauses left out.
    pub fn elapsed(&self, now: i64) -> i64 {
        let until = self.paused_at.unwrap_or(now);
        (until - self.start - self.paused).max(0)
    }

    /// Seconds left of the time chosen; negative past it.
    pub fn remaining(&self, now: i64) -> i64 {
        i64::from(self.planned) * 60 - self.elapsed(now)
    }

    /// Paused, or going again: the time paused is set aside.
    pub fn toggle_pause(&mut self, now: i64) {
        match self.paused_at.take() {
            Some(since) => self.paused += now - since,
            None => self.paused_at = Some(now),
        }
    }

    /// The session as recorded when it ends: whole minutes of focus, at least one.
    pub fn finished(&self, now: i64, done: bool) -> Session {
        let minutes = u32::try_from((self.elapsed(now) + 30) / 60).unwrap_or(u32::MAX).max(1);
        Session { task: self.task.clone(), start: self.start, minutes, done, ..Session::default() }
    }
}

fn running_path(dir: &Path) -> PathBuf {
    dir.join("running.toml")
}

/// The session running now, if any.
pub fn running_in(dir: &Path) -> Option<Running> {
    std::fs::read_to_string(running_path(dir)).ok().and_then(|t| toml::from_str(&t).ok())
}

pub fn running() -> Option<Running> {
    running_in(&folder())
}

/// Keeps the running session, or forgets it (None).
pub fn keep_running_in(dir: &Path, running: Option<&Running>) -> Result<(), String> {
    let path = running_path(dir);
    match running {
        Some(r) => write_atomically(&path, &toml::to_string(r).map_err(|e| e.to_string())?),
        None => match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
            _ => Ok(()),
        },
    }
}

pub fn keep_running(running: Option<&Running>) -> Result<(), String> {
    keep_running_in(&folder(), running)
}

/// Minutes an open-ended session counts at most: one left running all night is not a night's work.
const OPEN_ENDED_CEILING: u32 = 180;

/// Ends the running session: recorded, then forgotten. A session left running
/// for hours (the window closed, a night passed) counts only the time chosen;
/// an open-ended one (`planned` 0), three hours at most.
pub fn finish_in(dir: &Path, now: i64, done: bool) -> Result<Option<Session>, String> {
    finish_noted_in(dir, now, done, "")
}

fn finish_noted_in(dir: &Path, now: i64, done: bool, note: &str) -> Result<Option<Session>, String> {
    let Some(running) = running_in(dir) else { return Ok(None) };
    let mut session = running.finished(now, done);
    session.note = note.to_string();
    let ceiling = if running.planned == 0 { OPEN_ENDED_CEILING } else { running.planned.saturating_mul(2).max(running.planned.saturating_add(30)) };
    if session.minutes > ceiling {
        session.minutes = if running.planned == 0 { OPEN_ENDED_CEILING } else { running.planned };
    }
    record_in(dir, &session)?;
    keep_running_in(dir, None)?;
    Ok(Some(session))
}

pub fn finish(now: i64, done: bool) -> Result<Option<Session>, String> {
    finish_in(&folder(), now, done)
}

/// Ends the running session with where you stopped.
pub fn finish_with_note(now: i64, done: bool, note: &str) -> Result<Option<Session>, String> {
    finish_noted_in(&folder(), now, done, note)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_by_hand_and_billed_once() {
        let dir = std::env::temp_dir().join(format!("sioul-time-hand-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let start = 1_791_000_000;
        let meeting = Session { project: "lumen".into(), start, minutes: 90, note: "Kick-off meeting".into(), ..Session::default() };
        let coffee = Session { project: "lumen".into(), start: start + 7200, minutes: 30, unbilled: true, ..Session::default() };
        record_in(&dir, &meeting).unwrap();
        record_in(&dir, &coffee).unwrap();
        assert_eq!(update_in(&dir, &[meeting.key()], |s| s.invoice = "2026-001".into()).unwrap(), 1);
        let all = sessions_in(&dir);
        assert_eq!((all[0].invoice.as_str(), all[1].invoice.as_str()), ("2026-001", ""));
        let text = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|e| std::fs::read_to_string(e.path()).unwrap()).collect::<String>();
        assert!(!text.contains("task = \"\""), "a session for a project alone says so without an empty task: {text}");
        assert_eq!(remove_in(&dir, &[coffee.key()]).unwrap(), 1);
        assert_eq!(sessions_in(&dir).len(), 1);
        // A month's file that cannot be read is never written over with one session.
        let file = month_file(&dir, start);
        std::fs::write(&file, "[[session]]\nstart = \"half written").unwrap();
        assert!(record_in(&dir, &meeting).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[[session]]\nstart = \"half written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sessions_by_month_and_by_task() {
        let dir = std::env::temp_dir().join(format!("sioul-time-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let start = 1_791_100_000; // 4 October 2026
        record_in(&dir, &Session { task: "a".into(), start, minutes: 25, ..Session::default() }).unwrap();
        record_in(&dir, &Session { task: "a".into(), start: start + 3600, minutes: 5, done: true, ..Session::default() }).unwrap();
        record_in(&dir, &Session { task: "b".into(), start: start - 40 * 86_400, minutes: 15, ..Session::default() }).unwrap();
        let all = sessions_in(&dir);
        assert_eq!(all.len(), 3);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2, "two months, two files");
        let spent = spent(&all, start - 86_400, start + 86_400);
        assert_eq!(spent.get("a"), Some(&30));
        assert_eq!(spent.get("b"), None);

        let mut running = Running { task: "c".into(), start, planned: 15, ..Running::default() };
        running.toggle_pause(start + 300);
        running.toggle_pause(start + 900);
        assert_eq!((running.elapsed(start + 1200), running.remaining(start + 1200)), (600, 300));
        keep_running_in(&dir, Some(&running)).unwrap();
        assert_eq!(running_in(&dir), Some(running));
        let session = finish_in(&dir, start + 1200, true).unwrap().unwrap();
        assert_eq!((session.minutes, session.done), (10, true));
        assert_eq!(running_in(&dir), None);
        // Forgotten overnight: the time chosen, not the night.
        keep_running_in(&dir, Some(&Running { task: "d".into(), start, planned: 25, ..Running::default() })).unwrap();
        assert_eq!(finish_in(&dir, start + 10 * 3600, false).unwrap().unwrap().minutes, 25);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
