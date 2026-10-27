// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The moves of medicines taken every few hours that are to be put back
//! (docs/health.md, "Taken back"): `$XDG_STATE_HOME/sioul/health-back.toml`,
//! in the sharing's health part. A dose's mark taken back puts back the start
//! its take moved; a time corrected later replaces its move, and the start it
//! had moved to is put back where it was before moving again. Each entry,
//! `back."<dose key>~<after>" = [before, after, at]`: the medicine's start
//! before the move, after it, and when this was decided (Unix seconds); kept
//! a week. Every device puts the starts back from here
//! (`Health::mend_shifts`), its health file read or not where it was decided.
//!
//! A file of its own, as `health-reminders.toml` is: an older Sioul does not
//! know it and leaves it alone; a table of `health-state.toml` would be
//! dropped by its next save, and the sharing would then take it out on every
//! device (the check of 1b6795e). Each entry by its own name travels whole,
//! and two devices write two names.

use crate::health::{Problem, Unsound};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The file's name, in Sioul's state folder.
pub const FILE: &str = "health-back.toml";

/// How long an entry is kept after it was decided.
pub const KEPT_DAYS: i64 = 7;

/// The starts to put back, by `"<dose key>~<after>"`: `[before, after, at]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoseBacks {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub back: BTreeMap<String, [i64; 3]>,
}

impl DoseBacks {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join(FILE)
    }

    /// The file; none yet, nothing to put back. One gone though written here
    /// before, or that does not read: not known, said so (`Unsound`).
    pub fn read(path: &Path) -> Result<DoseBacks, Unsound> {
        match std::fs::read_to_string(path) {
            Ok(text) if text.trim().is_empty() && crate::health::witnessed(path) => Err(Unsound::Lost),
            Ok(text) => toml::from_str(&text).map_err(|_| Unsound::Unreadable),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if crate::health::witnessed(path) {
                    Err(Unsound::Lost)
                } else {
                    Ok(DoseBacks::default())
                }
            }
            Err(_) => Err(Unsound::Unreadable),
        }
    }

    /// The entries changed by `change`, one writer at a time, read again just
    /// before; those past `KEPT_DAYS` dropped. Never written over when it
    /// does not read.
    pub fn update<T>(path: &Path, now: i64, change: impl FnOnce(&mut DoseBacks) -> T) -> Result<T, Problem> {
        crate::filelock::with_lock(path, || {
            let mut all = DoseBacks::read(path).map_err(Problem::Unsound)?;
            let before = all.clone();
            let out = change(&mut all);
            all.back.retain(|_, [_, _, at]| *at >= now - KEPT_DAYS * 86_400);
            if all != before {
                let text = format!("# Doses' moves to put back (docs/health.md, \"Taken back\").\n{}", toml::to_string(&all).map_err(|e| Problem::Write(e.to_string()))?);
                crate::health::write_witnessed(path, text.as_bytes()).map_err(Problem::Write)?;
            }
            Ok(out)
        })
    }

    /// A move of the dose `key`, from `before` to `after`, to put back where it
    /// was: taken back, or replaced by a time corrected (`at`: when).
    pub fn restore(&mut self, key: &str, before: i64, after: i64, at: i64) {
        if before != after {
            self.back.insert(format!("{key}~{after}"), [before, after, at]);
        }
    }

    /// The dose `key` taken anew: what was to be put back of it is passed.
    pub fn passed(&mut self, key: &str) {
        self.back.retain(|name, _| name.rsplit_once('~').is_none_or(|(dose, _)| dose != key));
    }

    /// The starts to put back of a medicine (its id), each [before, after, at].
    pub fn of<'a>(&'a self, id: &'a str) -> impl Iterator<Item = [i64; 3]> + 'a {
        self.back.iter().filter(move |(name, _)| name.rsplit_once('~').and_then(|(dose, _)| dose.rsplit_once('@')).is_some_and(|(medicine, _)| medicine == id)).map(|(_, entry)| *entry)
    }
}
