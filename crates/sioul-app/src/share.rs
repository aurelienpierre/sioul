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

/// Folders already shared through, by this device's other devices: those holding a seal
/// (`seal.toml`) in the folders a sync carries here, or on Android in your files
/// (where the sync app, eDrive, Syncthing, puts them). A few levels down, the big ones skipped.
pub(crate) fn candidates() -> Vec<String> {
    let roots: Vec<PathBuf> = if cfg!(target_os = "android") { vec![PathBuf::from("/storage/emulated/0")] } else { synced_roots() };
    let mut found = Vec::new();
    for root in &roots {
        look_for_seals(root, 0, &mut found);
    }
    found.sort();
    found.dedup();
    found.into_iter().map(|path| shorten(&path)).collect()
}

/// A folder's own folders, for Sioul's folder browser (Android's picker
/// refuses the phone's storage, Murena's among others): {"path", "parent",
/// "folders": [{"name", "path", "sealed"}], "readable"}. Hidden folders left out;
/// "" starts in the phone's storage (Android), else your home.
pub(crate) fn folders_in(path: &str) -> String {
    #[derive(Serialize)]
    struct Folder {
        name: String,
        path: String,
        /// Already shared through, by your other devices (it holds a seal).
        sealed: bool,
    }
    #[derive(Serialize)]
    struct Listing {
        path: String,
        parent: String,
        folders: Vec<Folder>,
        readable: bool,
    }
    let start = if cfg!(target_os = "android") { PathBuf::from("/storage/emulated/0") } else { expand_home("~") };
    let dir = if path.trim().is_empty() { start } else { expand_home(path.trim()) };
    let entries = std::fs::read_dir(&dir);
    let readable = entries.is_ok();
    let mut folders: Vec<Folder> = entries
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
        .filter(|(name, _)| !name.starts_with('.'))
        .map(|(name, path)| Folder { sealed: path.join("seal.toml").is_file(), path: path.display().to_string(), name })
        .collect();
    folders.sort_by_key(|f| f.name.to_lowercase());
    // Up to the storage's top on Android: above it, nothing an app may read.
    let top = cfg!(target_os = "android") && dir == Path::new("/storage/emulated/0");
    let parent = dir.parent().filter(|_| !top).map(|p| p.display().to_string()).unwrap_or_default();
    json(&Listing { path: dir.display().to_string(), parent, folders, readable })
}

fn look_for_seals(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    // Deep enough for the Nextcloud app's own folder (Android/media/com.nextcloud.client/nextcloud/<account>/…).
    if depth > 6 || found.len() >= 10 {
        return;
    }
    if dir.join("seal.toml").is_file() {
        found.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        let skip = name.starts_with('.') || matches!(name.as_str(), "DCIM" | "Pictures" | "Movies" | "Music" | "Podcasts" | "Ringtones" | "Alarms" | "Notifications" | "data" | "obb" | "node_modules" | "target");
        if !skip && entry.file_type().is_ok_and(|t| t.is_dir()) {
            look_for_seals(&entry.path(), depth + 1, found);
        }
    }
}

fn carried(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

/// Where to share by default: inside a Documents folder a sync carries when
/// there is one, since a phone's sync app may carry only some folders (Murena's
/// eDrive: Documents, Pictures, Music…, not the cloud's root); else in the
/// case store when a sync carries it, else in a synced folder.
fn suggested(roots: &[PathBuf]) -> String {
    let store = load_config().case_store_path();
    let documents = roots.iter().map(|root| root.join("Documents")).find(|d| d.is_dir());
    let folder = match (&store, &documents, roots.first()) {
        (Some(store), _, _) if carried(store, roots) && in_documents(store) => store.join("sioul-shared"),
        (_, Some(documents), _) => documents.join("Sioul"),
        (Some(store), _, _) if carried(store, roots) => store.join("sioul-shared"),
        (_, _, Some(root)) => root.join("Sioul"),
        (Some(store), _, None) => store.join("sioul-shared"),
        (None, _, None) => return String::new(),
    };
    shorten(&folder)
}

/// Whether a folder is inside one named Documents, which phones' sync apps
/// carry: Murena's eDrive carries it, and not the rest of the cloud.
fn in_documents(path: &Path) -> bool {
    path.components().any(|part| part.as_os_str().eq_ignore_ascii_case("Documents"))
}

/// A folder a sync carries, outside the Documents folder that sync has: a phone
/// may not see it.
fn beside_documents(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root) && root.join("Documents").is_dir()) && !in_documents(path)
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
    /// Projects and budgets travel here too (`share_projects`).
    projects: bool,
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
    // A phone's sync app may carry only its cloud's Documents folder (Murena's eDrive).
    if !cfg!(target_os = "android") && beside_documents(&path, &roots) {
        lines.push(tr().text("share-phones", None));
    }
    // Projects and notes travel by their own folder: say so when no sync seems to carry it.
    if let Some(store) = load_config().case_store_path()
        && carried(&path, &roots)
        && !carried(&store, &roots)
    {
        problems.push(say("share-outside", &[("store", shorten(&store))]));
    }
    json(&Status { on, folder: chosen, sealed: share::sealed(&path), projects: load_config().share_projects, lines, problems })
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

/// Projects and budgets carried through the sharing too, or not (a setting
/// every device then follows); "" when kept, else why not.
pub(crate) fn set_projects(on: bool) -> String {
    sioul_core::config::set_value(&crate::backend::config_path(), "share_projects", &sioul_core::config::SettingValue::Bool(on)).err().unwrap_or_default()
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
    // The claim says how far this computer wrote its records: read that far, the others know all it marked.
    let wrote = share::written(&memory_path(), &here.id);
    match sioul_sync::lease::renew(&folder, &key, part, &here.id, jiff::Timestamp::now().as_second(), active, take, rule, wrote) {
        Ok(keeper) => (keeper, false),
        Err(_) => (alone, true),
    }
}

/// The parts this computer claims (`keeper`): closed together.
const PARTS: [&str; 3] = ["health", "notices", crate::projects::INVOICES];

/// Sioul closes, or is put away on a phone: what was marked goes out at once
/// (an exchange, waiting for one running), then its claims say it closed and
/// how far it wrote, so the others know it marks nothing until it is back
/// (docs/health.md, "Knowing").
pub(crate) fn closing() {
    let here = share::Here::load(&state_dir());
    let (Some(folder), Some(key)) = (here.folder_path(), key()) else { return };
    let memory = memory_path();
    {
        let _busy = BUSY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let stores = share::stores(&load_config(), &share::Roots::here());
        let sharing = share::Sharing { folder: &folder, computer: &here.id, key: &key, memory: &memory };
        let _ = share::exchange(&sharing, &stores, jiff::Timestamp::now().as_millisecond());
    }
    let wrote = share::written(&memory, &here.id);
    for part in PARTS {
        let _ = sioul_sync::lease::close(&folder, &key, part, &here.id, jiff::Timestamp::now().as_second(), wrote);
    }
}

/// The others' claims on the doses, and what was read of their records: what
/// this computer knows of the doses they marked (`health::know`). None when
/// sharing is off.
pub(crate) fn others_on_health() -> Option<(String, Vec<sioul_sync::lease::Claim>, share::Heard)> {
    let here = share::Here::load(&state_dir());
    let (Some(folder), Some(key)) = (here.folder_path(), key()) else { return None };
    let claims = sioul_sync::lease::claims(&folder, &key, "health").into_iter().filter(|c| c.computer != here.id).collect();
    Some((here.id.clone(), claims, share::heard(&memory_path(), &here.id)))
}

/// The doses' record read again from every computer's records at the next
/// exchange, this computer forgetting it held it: one lost or broken here,
/// started again empty, takes nothing out elsewhere. Waits for an exchange
/// running. Returns what went wrong, else "".
pub(crate) fn rebuild_health_record() -> String {
    let here = share::Here::load(&state_dir());
    let _busy = BUSY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    share::rebuild(&memory_path(), &here.id, &["state/health-state.toml"]).err().unwrap_or_default()
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
        let (written, accounts, problems) = match share::exchange(&sharing, &stores, now.as_millisecond()) {
            Ok(outcome) => (!outcome.written.is_empty(), outcome.written.contains("config/config.toml"), outcome.problems),
            Err(e) => (false, false, vec![e]),
        };
        let mut said = problems;
        said.dedup();
        if let Ok(mut last) = LAST.lock() {
            *last = Some((now.as_second(), said));
        }
        // Accounts come with another device's settings: their syncs start (asking
        // their passwords, which never travel).
        if accounts {
            crate::backend::start_new_watchers(&qt, &shared);
        }
        if written {
            crate::backend::show(&qt, &shared);
            crate::pim::show_pim(&qt, &shared);
            crate::work::show_work(&qt, &shared);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_folders() {
        let base = std::env::temp_dir().join(format!("sioul-share-documents-{}", std::process::id()));
        let root = base.join("Nextcloud");
        std::fs::create_dir_all(root.join("Documents")).unwrap();
        let roots = vec![root.clone()];
        assert!(in_documents(&root.join("Documents").join("Sioul")));
        assert!(in_documents(&root.join("documents").join("Sioul")));
        assert!(!in_documents(&root.join("Sioul")));
        assert!(beside_documents(&root.join("Sioul"), &roots));
        assert!(!beside_documents(&root.join("Documents").join("Sioul"), &roots));
        // A sync without a Documents folder (Syncthing, Dropbox): nothing to say.
        let other = base.join("Sync");
        std::fs::create_dir_all(&other).unwrap();
        assert!(!beside_documents(&other.join("Sioul"), &[other.clone()]));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn folders_listed() {
        let base = std::env::temp_dir().join(format!("sioul-folders-in-{}", std::process::id()));
        for dir in ["Documents/Sioul", "Documents/.hidden", "Documents/archive", "Documents/Bills"] {
            std::fs::create_dir_all(base.join(dir)).unwrap();
        }
        std::fs::write(base.join("Documents/Sioul/seal.toml"), "").unwrap();
        std::fs::write(base.join("Documents/notes.txt"), "").unwrap();
        let listed: serde_json::Value = serde_json::from_str(&folders_in(&base.join("Documents").display().to_string())).unwrap();
        let names: Vec<&str> = listed["folders"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["archive", "Bills", "Sioul"]);
        assert_eq!(listed["folders"][2]["sealed"], true);
        assert_eq!(listed["folders"][0]["sealed"], false);
        assert_eq!(listed["parent"], base.display().to_string());
        assert_eq!(listed["readable"], true);
        let missing: serde_json::Value = serde_json::from_str(&folders_in(&base.join("nowhere").display().to_string())).unwrap();
        assert_eq!(missing["readable"], false);
        let _ = std::fs::remove_dir_all(&base);
    }
}
