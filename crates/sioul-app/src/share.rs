// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sharing with your other computers, for the window (docs/database.md): set
//! up on the Parameters page, one exchange a minute off the window's thread,
//! the pages read again when another computer's changes came in.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use serde::Serialize;
use sioul_core::config::{expand_home, state_dir};
use sioul_sync::share;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The key made from the passphrase, once read from the keyring.
static KEY: Mutex<Option<[u8; 32]>> = Mutex::new(None);
/// One exchange at a time.
static BUSY: Mutex<()> = Mutex::new(());
/// The last exchange: when it ended (Unix seconds), what went wrong.
static LAST: Mutex<Option<(i64, Vec<String>)>> = Mutex::new(None);

fn hex(key: &[u8; 32]) -> String {
    key.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    let bytes: Option<Vec<u8>> = (0..text.len()).step_by(2).map(|i| text.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
    <[u8; 32]>::try_from(bytes?).ok()
}

fn key() -> Option<[u8; 32]> {
    let mut held = KEY.lock().ok()?;
    if held.is_none() {
        *held = sioul_sync::secret::named(share::KEY_NAME).and_then(|k| unhex(&k));
    }
    *held
}

fn memory_path() -> PathBuf {
    state_dir().join("share").join("memory.json")
}

/// Folders a sync carries: those named so in your home, and Nextcloud's own list.
fn synced_roots() -> Vec<PathBuf> {
    let home = expand_home("~");
    let mut roots: Vec<PathBuf> = ["Nextcloud", "Dropbox", "ownCloud", "Sync", "Syncthing", "OneDrive", "pCloudDrive", "Seafile"].iter().map(|n| home.join(n)).filter(|p| p.is_dir()).collect();
    let configs = [home.join(".config/Nextcloud/nextcloud.cfg"), home.join("Library/Preferences/Nextcloud/nextcloud.cfg"), std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Nextcloud").join("nextcloud.cfg")).unwrap_or_default()];
    for text in configs.iter().filter_map(|c| std::fs::read_to_string(c).ok()) {
        for line in text.lines() {
            if let Some((_, path)) = line.split_once("localPath=") {
                let path = PathBuf::from(path.trim().trim_end_matches('/'));
                if path.is_dir() && !roots.contains(&path) {
                    roots.push(path);
                }
            }
        }
    }
    roots
}

fn carried(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

/// Where to share by default: in the case store when a sync carries it, else in a synced folder.
fn suggested(roots: &[PathBuf]) -> String {
    let store = load_config().case_store_path();
    let folder = match (&store, roots.first()) {
        (Some(store), _) if carried(store, roots) => store.join("sioul-shared"),
        (_, Some(root)) => root.join("Sioul"),
        (Some(store), None) => store.join("sioul-shared"),
        (None, None) => return String::new(),
    };
    shorten(&folder)
}

/// A path with your home written `~`.
fn shorten(path: &Path) -> String {
    let home = expand_home("~");
    path.strip_prefix(&home).map_or_else(|_| path.display().to_string(), |rest| format!("~/{}", rest.display()))
}

/// A moment, said shortly: "14:05" today, "Fri 2 Oct 14:05" before.
fn when(seconds: i64) -> String {
    let Ok(at) = jiff::Timestamp::from_second(seconds) else { return String::new() };
    let at = at.to_zoned(jiff::tz::TimeZone::system());
    if at.date() == jiff::Zoned::now().date() { at.strftime("%H:%M").to_string() } else { tr().date(&at, true) }
}

#[derive(Serialize)]
struct Status {
    on: bool,
    folder: String,
    /// The folder already holds a seal: the passphrase is typed once, not twice.
    sealed: bool,
    lines: Vec<String>,
    problems: Vec<String>,
}

/// What the Parameters page shows, as JSON; `folder` is the one being chosen, if any.
pub(crate) fn status(folder: &str) -> String {
    let here = share::Here::load(&state_dir());
    let roots = synced_roots();
    let on = here.folder_path().is_some() && key().is_some();
    // Shared: the folder in use. Not yet: the one being chosen, else where it was, else a suggestion.
    let saved = here.folder.clone().filter(|f| !f.is_empty());
    let chosen = match (&saved, folder.is_empty()) {
        (Some(saved), _) if on => saved.clone(),
        (_, false) => folder.to_string(),
        (Some(saved), true) => saved.clone(),
        (None, true) => suggested(&roots),
    };
    let path = expand_home(&chosen);
    let mut lines = Vec::new();
    let mut problems = Vec::new();
    if on {
        lines.push(say("share-on", &[("folder", chosen.clone())]));
        let others = share::others(&path, &here.id);
        match others.iter().map(|o| o.heard).max() {
            Some(heard) => {
                let mut args = sioul_core::i18n::args();
                args.set("count", others.len() as i64);
                args.set("when", when(heard));
                lines.push(tr().text("share-others", Some(&args)));
            }
            None => lines.push(tr().text("share-alone", None)),
        }
        if let Some((at, said)) = LAST.lock().ok().and_then(|l| l.clone()) {
            lines.push(say("share-last", &[("when", when(at))]));
            problems.extend(said.iter().map(|p| problem_text(p)));
        }
    } else if here.folder_path().is_some() {
        problems.push(tr().text("share-key-missing", None));
    } else {
        lines.push(tr().text("share-off", None));
    }
    // Projects and notes travel by their own folder: say so when no sync seems to carry it.
    if let Some(store) = load_config().case_store_path()
        && carried(&path, &roots)
        && !carried(&store, &roots)
    {
        problems.push(say("share-outside", &[("store", shorten(&store))]));
    }
    json(&Status { on, folder: chosen, sealed: share::sealed(&path), lines, problems })
}

fn problem_text(code: &str) -> String {
    match code.split_once(':') {
        Some(("share-other-seal", _)) => tr().text("share-other-seal", None),
        Some(("share-unreadable", file)) => say("share-unreadable", &[("file", file.to_string())]),
        _ => code.to_string(),
    }
}

/// Starts sharing through a folder; "" when it did, else why not.
pub(crate) fn start(folder: &str, passphrase: &str, again: &str) -> String {
    let folder = folder.trim();
    if folder.is_empty() {
        return tr().text("share-no-folder", None);
    }
    let path = expand_home(folder);
    let first = !share::sealed(&path);
    if passphrase.chars().count() < 12 {
        return tr().text("share-short", None);
    }
    if first && passphrase != again {
        return tr().text("share-differ", None);
    }
    let key = match share::key_for(&path, passphrase) {
        Ok(key) => key,
        Err(share::Refused::WrongPassphrase) => return tr().text("share-wrong", None),
        Err(share::Refused::Other(e)) => return e,
    };
    if let Err(e) = sioul_sync::secret::save_named(share::KEY_NAME, &hex(&key)) {
        return e.sentence(tr(), "Sioul");
    }
    if let Ok(mut held) = KEY.lock() {
        *held = Some(key);
    }
    let state = state_dir();
    let mut here = share::Here::load(&state);
    here.folder = Some(folder.to_string());
    here.save(&state).err().unwrap_or_default()
}

/// Stops sharing here: the key forgotten, the folder left as it is for the others.
pub(crate) fn stop() -> String {
    if let Ok(mut held) = KEY.lock() {
        *held = None;
    }
    let _ = sioul_sync::secret::forget_named(share::KEY_NAME);
    let state = state_dir();
    let mut here = share::Here::load(&state);
    here.folder = None;
    // Starting again later reads the others first, as the first time.
    let _ = std::fs::remove_file(memory_path());
    here.save(&state).err().unwrap_or_default()
}

/// Whether sharing is on here: a folder and its key.
pub(crate) fn on() -> bool {
    share::Here::load(&state_dir()).folder_path().is_some() && key().is_some()
}

/// The last exchange, when one ran since Sioul started: when (Unix seconds), and whether it went well.
pub(crate) fn last_exchange() -> Option<(i64, bool)> {
    LAST.lock().ok().and_then(|l| l.as_ref().map(|(at, problems)| (*at, problems.is_empty())))
}

/// Who keeps a part of the work among your computers (`sioul_sync::lease`),
/// this computer's claim renewed; `active` is when you were last at it,
/// `take` takes the part here on purpose. Sharing off: this computer alone.
/// The second value: the folder could not be written, so the others may not know.
pub(crate) fn keeper(part: &str, rule: sioul_sync::lease::Rule, active: i64, take: bool) -> (sioul_sync::lease::Keeper, bool) {
    let here = share::Here::load(&state_dir());
    let alone = sioul_sync::lease::Keeper::alone(&here.id);
    let (Some(folder), Some(key)) = (here.folder_path(), key()) else { return (alone, false) };
    match sioul_sync::lease::renew(&folder, &key, part, &here.id, jiff::Timestamp::now().as_second(), active, take, rule) {
        Ok(keeper) => (keeper, false),
        Err(_) => (alone, true),
    }
}

/// An exchange, when sharing is on: off the window's thread; the pages read
/// again when changes came in.
pub(crate) fn exchange(qt: &QtThread, shared: &Arc<Shared>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let Some(_busy) = crate::backend::one_at_a_time(&BUSY) else { return };
        let here = share::Here::load(&state_dir());
        let (Some(folder), Some(key)) = (here.folder_path(), key()) else { return };
        let stores = share::stores(&load_config(), &share::Roots::here());
        let now = jiff::Timestamp::now();
        let memory = memory_path();
        let sharing = share::Sharing { folder: &folder, computer: &here.id, key: &key, memory: &memory };
        let (written, problems) = match share::exchange(&sharing, &stores, now.as_millisecond()) {
            Ok(outcome) => (!outcome.written.is_empty(), outcome.problems),
            Err(e) => (false, vec![e]),
        };
        let mut said = problems;
        said.dedup();
        if let Ok(mut last) = LAST.lock() {
            *last = Some((now.as_second(), said));
        }
        if written {
            crate::backend::show(&qt, &shared);
            crate::pim::show_pim(&qt, &shared);
            crate::work::show_work(&qt, &shared);
        }
    });
}
