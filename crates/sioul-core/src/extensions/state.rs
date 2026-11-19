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
    /// The SHA-256 of the manifest accepted under `serial`: another one under
    /// the same serial is refused. Empty before the first.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manifest: String,
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
    /// Removed here: no call of it runs, a call under way ends at its next
    /// ask, and its sign-in is still to forget while the keyring refuses.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leaving: bool,
}

impl Installed {
    pub fn new(offer: Offer) -> Self {
        Self { offer, enabled: true, waiting: None, signed_in: false, last_run: 0, last_brought: 0, agendas: Vec::new(), problem: None, leaving: false }
    }
}

impl State {
    pub fn get(&self, id: &str) -> Option<&Installed> {
        self.extensions.iter().find(|e| e.offer.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Installed> {
        self.extensions.iter_mut().find(|e| e.offer.id == id)
    }

    /// A manifest taken, by its serial and the SHA-256 of its bytes: refused
    /// when its serial is lower than the last accepted here or than the
    /// lowest this release of Sioul accepts (`LOWEST_SERIAL`, so that a
    /// fresh device, or one whose record was lost, cannot be served an older
    /// signed manifest either), and when it is another manifest under the
    /// serial accepted last (two manifests signed with one number cannot be
    /// alternated). The same manifest read again is taken.
    pub fn accept(&mut self, serial: u64, sha256: &str) -> Result<(), String> {
        let lowest = self.serial.max(super::LOWEST_SERIAL);
        if serial < lowest {
            return Err(format!("an older manifest (serial {serial}, {lowest} at least)"));
        }
        if serial == self.serial && !self.manifest.is_empty() && self.manifest != sha256 {
            return Err(format!("another manifest under serial {serial}"));
        }
        self.serial = serial;
        self.manifest = sha256.to_string();
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

/// The state at `path`; an absent or unreadable one is empty. Each offer it
/// keeps is checked as a manifest's are (its id above all, which names a
/// file, a log and a keyring entry): one that fails is left out, as if never
/// installed (M2: nothing signs this record).
pub fn load(path: &Path) -> State {
    let mut state: State = std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default();
    state.extensions.retain(|i| super::manifest::check(&i.offer).is_ok());
    for installed in &mut state.extensions {
        if installed.waiting.as_ref().is_some_and(|w| super::manifest::check(w).is_err() || w.id != installed.offer.id) {
            installed.waiting = None;
        }
    }
    state
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

/// The most lines an extension's log keeps, and the most bytes.
pub const LOG_LINES: usize = 400;
pub const LOG_BYTES: usize = 128 << 10;
/// The most characters a line of the log holds (the engine cuts its own the same).
pub const LOG_WIDTH: usize = 300;

/// A line as the log keeps it: no control character, `LOG_WIDTH` characters at most.
fn log_line(line: &str) -> String {
    let plain: String = line.chars().filter(|c| !c.is_control()).collect();
    let mut cut: String = plain.chars().take(LOG_WIDTH).collect();
    if cut.len() < plain.len() {
        cut.push('…');
    }
    cut
}

/// Lines added to an extension's log at `path`, its oldest lines dropped
/// beyond `LOG_LINES` and `LOG_BYTES`. The engine's lines come hidden and
/// cut; every line is cut here again, whoever wrote it (X6), and the window's
/// own lines name nothing an extension said.
pub fn append_log(path: &Path, lines: &[String]) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }
    crate::filelock::with_lock(path, || {
        let before = std::fs::read_to_string(path).unwrap_or_default();
        let added: Vec<String> = lines.iter().map(|l| log_line(l)).collect();
        let mut all: Vec<&str> = before.lines().collect();
        all.extend(added.iter().map(String::as_str));
        let mut keep = all.len().saturating_sub(LOG_LINES);
        let mut bytes: usize = all[keep..].iter().map(|l| l.len() + 1).sum();
        while bytes > LOG_BYTES && keep < all.len() {
            bytes -= all[keep].len() + 1;
            keep += 1;
        }
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
            state.accept(7, "a7").unwrap();
            state.extensions.push(Installed::new(offer.clone()));
        })
        .unwrap();
        let mut state = load(&path);
        assert_eq!((state.serial, state.manifest.as_str()), (7, "a7"));
        assert!(state.get("timetable").unwrap().enabled);
        assert!(state.accept(6, "a6").is_err(), "an older manifest");
        assert!(state.accept(7, "a7").is_ok(), "the same one again");
        assert!(state.accept(7, "b7").is_err(), "another manifest under the same serial");
        assert!(state.accept(8, "a8").is_ok());
        let log = folder.join("timetable.log");
        for round in 0..3 {
            append_log(&log, &(0..200).map(|i| format!("{round}-{i}")).collect::<Vec<_>>()).unwrap();
        }
        let kept = std::fs::read_to_string(&log).unwrap();
        assert_eq!(kept.lines().count(), LOG_LINES);
        assert_eq!(kept.lines().last(), Some("2-199"));
        assert_eq!(kept.lines().next(), Some("1-0"));
        // X6: every line cut, whoever wrote it; the log bounded in bytes too.
        append_log(&log, &[format!("long {}\u{7}", "x".repeat(100_000))]).unwrap();
        let last = std::fs::read_to_string(&log).unwrap().lines().last().unwrap().to_string();
        assert!(last.chars().count() <= LOG_WIDTH + 1 && !last.contains('\u{7}'), "{}", last.len());
        for _ in 0..3 {
            append_log(&log, &(0..400).map(|i| format!("{i} {}", "y".repeat(290))).collect::<Vec<_>>()).unwrap();
        }
        assert!(std::fs::metadata(&log).unwrap().len() as usize <= LOG_BYTES);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// M2: an offer kept in the record is checked as a manifest's are when the record is read.
    #[test]
    fn a_kept_offer_that_is_not_one_is_left_out() {
        let path = std::env::temp_dir().join(format!("sioul-extensions-kept-{}.toml", std::process::id()));
        let offer = crate::extensions::manifest::parse(crate::extensions::manifest::SAMPLE).unwrap().offers.remove(0);
        let wrong = crate::extensions::manifest::Offer { id: "../x".into(), ..offer.clone() };
        let state = State { extensions: vec![Installed::new(offer.clone()), Installed::new(wrong)], ..State::default() };
        std::fs::write(&path, toml::to_string(&state).unwrap()).unwrap();
        let read = load(&path);
        assert_eq!(read.extensions.iter().map(|i| i.offer.id.as_str()).collect::<Vec<_>>(), vec!["timetable"]);
        let _ = std::fs::remove_file(&path);
    }

    /// S2: a fresh device, or one whose record was lost or no longer reads,
    /// still refuses a manifest older than the lowest this release accepts.
    #[test]
    fn an_older_manifest_is_refused_on_a_fresh_device_too() {
        let fresh = || State::default();
        assert!(fresh().accept(super::super::LOWEST_SERIAL - 1, "old").is_err(), "below the floor compiled in");
        assert!(fresh().accept(super::super::LOWEST_SERIAL, "now").is_ok());
        let unreadable = std::env::temp_dir().join(format!("sioul-extensions-unreadable-{}.toml", std::process::id()));
        std::fs::write(&unreadable, "serial = [").unwrap();
        assert!(load(&unreadable).accept(super::super::LOWEST_SERIAL - 1, "old").is_err(), "a record that no longer reads gives no way back");
        let _ = std::fs::remove_file(&unreadable);
    }
}
