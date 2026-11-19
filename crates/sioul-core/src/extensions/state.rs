// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What this device keeps of its extensions, never shared
//! (`$XDG_STATE_HOME/sioul/extensions.toml`): the last manifest's serial it
//! accepted, when it last read the manifest, and for each extension it
//! enabled the version in use (as accepted, with what it is given), an
//! update waiting for the person's yes, whether it holds a sign-in, when it
//! last ran and what it brought, and its last problem. The window and the
//! phone's background step both change it, one at a time (`change`).

use super::manifest::Offer;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// This device's extensions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// The highest serial of a manifest accepted here: a lower one is refused.
    #[serde(default)]
    pub serial: u64,
    /// When the manifest was last read (seconds since 1970).
    #[serde(default)]
    pub checked: i64,
    #[serde(default, rename = "extension")]
    pub extensions: Vec<Installed>,
}

/// An agenda an extension brought, as last taken in here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brought {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub events: usize,
}

/// An extension this device enabled once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    /// The version in use, as accepted: its hosts and sign-in are what it is given.
    pub offer: Offer,
    #[serde(default)]
    pub enabled: bool,
    /// A newer version that asks for more, waiting for the person's yes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<Offer>,
    /// Whether a sign-in is kept in this device's keyring.
    #[serde(default)]
    pub signed_in: bool,
    /// When it last ran, and last brought something (seconds since 1970).
    #[serde(default)]
    pub last_run: i64,
    #[serde(default)]
    pub last_brought: i64,
    #[serde(default)]
    pub agendas: Vec<Brought>,
    /// Its last problem, one word Sioul says in a sentence ("sign-in",
    /// "unreachable", "refused", "changed", "stopped-time", …); none when the last run went well.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

impl Installed {
    pub fn new(offer: Offer) -> Self {
        Self { offer, enabled: true, waiting: None, signed_in: false, last_run: 0, last_brought: 0, agendas: Vec::new(), problem: None }
    }
}

impl State {
    pub fn get(&self, id: &str) -> Option<&Installed> {
        self.extensions.iter().find(|e| e.offer.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Installed> {
        self.extensions.iter_mut().find(|e| e.offer.id == id)
    }

    /// A manifest's serial taken: refused when lower than the last accepted
    /// here, so that an older signed manifest cannot be served again.
    pub fn accept_serial(&mut self, serial: u64) -> Result<(), String> {
        if serial < self.serial {
            return Err(format!("an older manifest (serial {serial}, {} accepted before)", self.serial));
        }
        self.serial = serial;
        Ok(())
    }
}

/// Where this device keeps it.
pub fn path() -> PathBuf {
    crate::config::state_dir().join("extensions.toml")
}

/// Where an extension's file is kept, by its hash: Sioul's private data.
pub fn file_path(offer: &Offer) -> PathBuf {
    crate::config::data_dir().join("extensions").join(format!("{}-{}.wasm", offer.id, &offer.sha256[..offer.sha256.len().min(16)]))
}

/// Where an extension's log is kept, on this device only.
pub fn log_path(id: &str) -> PathBuf {
    crate::config::state_dir().join("extensions").join(format!("{id}.log"))
}

/// The state at `path`; an absent or unreadable one is empty.
pub fn load(path: &Path) -> State {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// The state at `path` changed by `f`, read and written under its lock (the
/// window and the phone's background process both change it); what `f` returns.
pub fn change<T>(path: &Path, f: impl FnOnce(&mut State) -> T) -> Result<T, String> {
    crate::filelock::with_lock(path, || {
        let mut state = load(path);
        let out = f(&mut state);
        let text = toml::to_string(&state).map_err(|e| e.to_string())?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(out)
    })
}

/// The most lines an extension's log keeps.
pub const LOG_LINES: usize = 400;

/// Lines added to an extension's log at `path`, its oldest lines dropped
/// beyond `LOG_LINES`. The lines come already cut and hidden (sioul-sync's engine).
pub fn append_log(path: &Path, lines: &[String]) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }
    crate::filelock::with_lock(path, || {
        let before = std::fs::read_to_string(path).unwrap_or_default();
        let mut all: Vec<&str> = before.lines().collect();
        all.extend(lines.iter().map(String::as_str));
        let keep = all.len().saturating_sub(LOG_LINES);
        let text = all[keep..].join("\n") + "\n";
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        }
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_keeps_its_serial_and_its_extensions() {
        let folder = std::env::temp_dir().join(format!("sioul-extensions-state-{}", std::process::id()));
        let path = folder.join("extensions.toml");
        let offer = crate::extensions::manifest::parse(crate::extensions::manifest::SAMPLE).unwrap().offers.remove(0);
        change(&path, |state| {
            state.accept_serial(7).unwrap();
            state.extensions.push(Installed::new(offer.clone()));
        })
        .unwrap();
        let mut state = load(&path);
        assert_eq!(state.serial, 7);
        assert!(state.get("timetable").unwrap().enabled);
        assert!(state.accept_serial(6).is_err(), "an older manifest");
        assert!(state.accept_serial(7).is_ok(), "the same one again");
        assert!(state.accept_serial(8).is_ok());
        let log = folder.join("timetable.log");
        for round in 0..3 {
            append_log(&log, &(0..200).map(|i| format!("{round}-{i}")).collect::<Vec<_>>()).unwrap();
        }
        let kept = std::fs::read_to_string(&log).unwrap();
        assert_eq!(kept.lines().count(), LOG_LINES);
        assert_eq!(kept.lines().last(), Some("2-199"));
        assert_eq!(kept.lines().next(), Some("1-0"));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
