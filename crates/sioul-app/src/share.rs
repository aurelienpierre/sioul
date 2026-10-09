// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sharing with your other computers, for the window (docs/database.md): set
//! up on the Parameters page, one exchange a minute off the window's thread,
//! the pages read again when another computer's changes came in. Each part
//! switched on or off on this device; the versions kept before other devices'
//! changes listed, and put back.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use cxx_qt_lib::QString;
use serde::Serialize;
use sioul_core::config::{Config, expand_home, state_dir};
use sioul_sync::share;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The key made from the passphrase, once read from the keyring.
static KEY: Mutex<Option<[u8; 32]>> = Mutex::new(None);
/// One exchange at a time.
static BUSY: Mutex<()> = Mutex::new(());
/// Set while a hurried exchange (a dose's alarm, a button pressed, a switch
/// changed) waits for the one running: its work on notes and papers stops
/// where it is, and goes on at the next.
static HURRY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// This device's sharing file written one change at a time.
static HERE: Mutex<()> = Mutex::new(());
/// The last exchange: when it ended (Unix seconds), what went wrong.
static LAST: Mutex<Option<(i64, Vec<String>)>> = Mutex::new(None);
/// What an exchange did that you should know for longer than a minute, said
/// for a day: files two devices changed, both versions kept (Unix seconds, the problem).
static SAID: Mutex<Vec<(i64, String)>> = Mutex::new(Vec::new());

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

/// This device's sharing (`share::Here`). The projects' switch was a setting
/// every device followed (`share_projects`): the first time sharing is on
/// here, what it said becomes this device's own choice.
fn here() -> share::Here {
    let state = state_dir();
    let mut here = share::Here::load(&state);
    if here.folder_path().is_some() && !here.parts.contains_key("projects") {
        let _writing = HERE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        here = share::Here::load(&state);
        if !here.parts.contains_key("projects") {
            here.parts.insert("projects".into(), load_config().share_projects);
            let _ = here.save(&state);
        }
    }
    here
}

/// Whether notes and papers can be read whole here: on Android, only with
/// "All files access" (without it, a folder lists only what Sioul made, and
/// the rest would seem taken out).
fn files_readable() -> bool {
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            /// android/main.cpp's: whether Sioul may reach your files by their path.
            fn sioul_android_files_access() -> bool;
        }
        // SAFETY: android/main.cpp's, asking Android through Java.
        unsafe { sioul_android_files_access() }
    }
    #[cfg(not(target_os = "android"))]
    true
}

/// `work` run while no exchange of this Sioul runs: the one running told to
/// hurry (its notes and papers wait for the next), then waited for.
fn quietly<T>(work: impl FnOnce() -> T) -> T {
    use std::sync::atomic::Ordering;
    HURRY.store(true, Ordering::Relaxed);
    let busy = BUSY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    HURRY.store(false, Ordering::Relaxed);
    let done = work();
    drop(busy);
    done
}

/// The same, while no exchange runs on this computer at all (another Sioul,
/// the command line): `share::exchange_lock` held too. Never around what takes that lock itself.
fn between_exchanges<T>(work: impl FnOnce() -> T) -> T {
    quietly(|| {
        let running = share::exchange_lock(&memory_path(), true);
        let done = work();
        drop(running);
        done
    })
}

/// The notes folder, when a sync app carries it already: notes and papers then
/// never travel through the sharing too, the two carriers would undo each
/// other's changes. On a phone, a notes folder in the same top folder of its
/// storage as the sharing folder (Documents…), which the sync app carries whole.
fn notes_carried(config: &Config, folder: Option<&Path>) -> Option<PathBuf> {
    let store = config.notes_root_path()?;
    let carried = if cfg!(target_os = "android") { folder.is_some_and(|folder| same_top(&on_storage(&store), &on_storage(folder), Path::new(STORAGE))) } else { carried(&store, &synced_roots()) };
    carried.then_some(store)
}

/// A phone's shared storage.
const STORAGE: &str = "/storage/emulated/0";

/// A path of a phone's shared storage under its one name: "/sdcard/…",
/// "/storage/self/primary/…", "/mnt/sdcard/…" are "/storage/emulated/0/…".
fn on_storage(path: &Path) -> PathBuf {
    ["/sdcard", "/storage/self/primary", "/mnt/sdcard", "/mnt/user/0/primary"].iter().find_map(|alias| path.strip_prefix(alias).ok()).map_or_else(|| path.to_path_buf(), |rest| Path::new(STORAGE).join(rest))
}

/// Two folders inside one folder at the top of `storage`.
fn same_top(a: &Path, b: &Path, storage: &Path) -> bool {
    let top = |path: &Path| path.strip_prefix(storage).ok().and_then(|rest| rest.components().next()).map(|c| c.as_os_str().to_owned());
    top(a).is_some() && top(a) == top(b)
}

/// Why a part cannot be shared from here, when it cannot: notes and papers while a sync app carries the notes folder.
fn refused(part: &str, config: &Config, here: &share::Here) -> Option<String> {
    if !matches!(part, "notes" | "papers") {
        return None;
    }
    notes_carried(config, here.folder_path().as_deref()).map(|store| say("share-part-carried", &[("store", shorten(&store))]))
}

/// What this device shares: each part as switched here, else as before parts had switches.
fn stores_here(here: &share::Here) -> Vec<share::Store> {
    let config = load_config();
    let carried = notes_carried(&config, here.folder_path().as_deref()).is_some();
    share::stores_of(&config, &share::Roots::here(), &|part| here.shares(part, &config) && !(carried && matches!(part, "notes" | "papers")))
}

/// A part's name and what it carries, in a line.
fn part_words(part: &str) -> (String, String) {
    let text = |id: &str| tr().text(id, None);
    match part {
        "settings" => (text("share-part-settings"), text("share-part-settings-carries")),
        "senders" => (text("share-part-senders"), text("share-part-senders-carries")),
        "calls" => (text("share-part-calls"), text("share-part-calls-carries")),
        "phone-messages" => (text("share-part-phone-messages"), text("share-part-phone-messages-carries")),
        // texts: SMS phase (b).
        "texts" => (text("share-part-texts"), text("share-part-texts-carries")),
        "spam" => (text("share-part-spam"), text("share-part-spam-carries")),
        "health" => (text("share-part-health"), text("share-part-health-carries")),
        "time" => (text("share-part-time"), text("share-part-time-carries")),
        "drafts" => (text("share-part-drafts"), text("share-part-drafts-carries")),
        "projects" => (text("share-part-projects"), text("share-part-projects-carries")),
        "lists" => (text("share-part-lists"), text("share-part-lists-carries")),
        "notes" => (text("share-part-notes"), text("share-part-notes-carries")),
        "papers" => (text("share-part-papers"), text("share-part-papers-carries")),
        _ => (part.to_string(), String::new()),
    }
}

/// Folders a sync carries: those named so in your home, and Nextcloud's own list.
fn synced_roots() -> Vec<PathBuf> {
    let home = expand_home("~");
    let mut roots: Vec<PathBuf> = ["Nextcloud", "Dropbox", "ownCloud", "Sync", "Syncthing", "OneDrive", "pCloudDrive", "Seafile"].iter().map(|n| home.join(n)).filter(|p| p.is_dir()).collect();
    let data = |name: &str, file: &str| std::env::var_os(name).map(|a| PathBuf::from(a).join(file)).unwrap_or_default();
    let read = |paths: &[PathBuf]| paths.iter().filter_map(|c| std::fs::read_to_string(c).ok()).collect::<Vec<_>>();
    let mut found: Vec<PathBuf> = Vec::new();
    for text in read(&[home.join(".config/Nextcloud/nextcloud.cfg"), home.join("Library/Preferences/Nextcloud/nextcloud.cfg"), data("APPDATA", "Nextcloud/nextcloud.cfg")]) {
        found.extend(text.lines().filter_map(|line| line.split_once("localPath=")).map(|(_, path)| PathBuf::from(path.trim().trim_end_matches('/'))));
    }
    // Syncthing's folders, wherever they are (its config.xml).
    for text in read(&[home.join(".config/syncthing/config.xml"), home.join(".local/state/syncthing/config.xml"), home.join("Library/Application Support/Syncthing/config.xml"), data("LOCALAPPDATA", "Syncthing/config.xml")]) {
        found.extend(syncthing_folders(&text).iter().map(|path| expand_home(path)));
    }
    // Dropbox, wherever it was put (its info.json: the personal and business folders).
    for text in read(&[home.join(".dropbox/info.json"), data("APPDATA", "Dropbox/info.json"), data("LOCALAPPDATA", "Dropbox/info.json")]) {
        found.extend(dropbox_folders(&text).into_iter().map(PathBuf::from));
    }
    for path in found {
        if path.is_dir() && !roots.contains(&path) {
            roots.push(path);
        }
    }
    roots
}

/// The folders a Syncthing configuration carries: each `<folder … path="…">`.
fn syncthing_folders(xml: &str) -> Vec<String> {
    xml.split("<folder ")
        .skip(1)
        .filter_map(|element| {
            let tag = element.split('>').next()?;
            let (_, rest) = tag.split_once(" path=\"").or_else(|| tag.strip_prefix("path=\"").map(|rest| ("", rest)))?;
            let path = rest.split('"').next()?;
            Some(path.replace("&amp;", "&").replace("&apos;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">"))
        })
        .collect()
}

/// The folders Dropbox's info.json names: `{"personal": {"path": …}, "business": {"path": …}}`.
fn dropbox_folders(json: &str) -> Vec<String> {
    let Ok(serde_json::Value::Object(accounts)) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    accounts.values().filter_map(|account| account.get("path").and_then(serde_json::Value::as_str).map(str::to_string)).collect()
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
/// notes folder when a sync carries it, else in a synced folder.
fn suggested(roots: &[PathBuf]) -> String {
    let store = load_config().notes_root_path();
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
    /// Projects and budgets travel here too.
    projects: bool,
    lines: Vec<String>,
    problems: Vec<String>,
    /// What travels from this device, part by part.
    parts: Vec<Part>,
    /// Folders of notes or papers whose files went at once: held, said, to take out everywhere on a word.
    vanished: Vec<Vanished>,
    /// Your other devices, as this one knows them: in use, closed, silent, off as you said.
    devices: Vec<crate::health::DeviceRow>,
    /// The other devices' files fetched from the server too: where it stands, the switch, the place given by hand.
    backup: Backup,
    /// The accounts Sioul can keep the folder in step with itself (`remote::MIRROR`).
    servers: Vec<ServerAccount>,
    /// Sioul keeps the folder in step with a server itself here.
    mirrored: bool,
    /// This device's build, in words: "This device: Sioul 0.0.3 (eff8661abcde)."
    build: String,
    /// "Send everything again" shown: sharing is on (not found on a server, it says why nothing went).
    can_again: bool,
    /// What it did last, in words; "" before it was pressed.
    again: String,
}

/// Files gone at once from a folder of notes or papers, held until you say.
#[derive(Serialize)]
struct Vanished {
    /// The folder's part in the records: "files/notes/", "files/papers/".
    store: String,
    text: String,
}

/// A part of what is shared, as this device has it.
#[derive(Serialize)]
struct Part {
    id: &'static str,
    name: String,
    /// What it carries, in a line.
    carries: String,
    on: bool,
    /// Why it cannot be switched on here; "" when it can.
    refused: String,
    /// When it last sent and received a change here, said shortly; "" when not shared.
    last: String,
}

/// What the Parameters page shows, as JSON; `folder` is the one being chosen, if any.
pub(crate) fn status(folder: &str) -> String {
    let here = here();
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
    let mut vanished = Vec::new();
    let mirror = mirrored();
    // The format of what travels, as this device knows it (its devices listed below, every one holding it back among them).
    let formats = if on { key().map(|key| share::formats(&path, &key, &memory_path(), &here.id, now_ms())) } else { None };
    if on {
        // Kept in step with a server by Sioul itself: its folder there said, not its copy here.
        match &mirror {
            Some(state) => {
                lines.push(say("share-on-server", &[("host", state.host()), ("place", state.place.clone())]));
                lines.extend(Some(mirror_line(state)).filter(|l| !l.is_empty()));
            }
            None => lines.push(say("share-on", &[("folder", chosen.clone())])),
        }
        attached(&here);
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
            problems.extend(said.iter().filter(|p| !kept_a_day(p) && !p.starts_with("share-vanished:") && !p.starts_with("share-busy") && !p.starts_with("share-newer:")).map(|p| problem_text(p)));
            vanished.extend(said.iter().filter_map(|p| vanished_of(p)));
        }
        // The format of what travels (docs/database.md, "The format of what
        // travels"): a part your other devices share in a newer form than
        // this Sioul reads, said as long as it lasts; a device on an older
        // Sioul holding the newer form back, or one appearing after it.
        if let Some(formats) = &formats {
            for (part, _, build) in &formats.newer {
                let name = part_words(part).0;
                problems.push(if build.is_empty() { say("share-newer-plain", &[("part", name)]) } else { say("share-newer", &[("part", name), ("build", build.clone())]) });
            }
            if formats.holders.iter().any(|(_, holds)| *holds != share::Holds::Unheard) {
                lines.push(tr().text("share-format-held", None));
            }
            // A device that still counts (heard of in the last 180 days, not forgotten) on an older Sioul.
            if !formats.older.is_empty() {
                problems.push(tr().text("share-format-older", None));
            }
        }
        // Files two devices changed, both kept: said for a day.
        let day_ago = jiff::Timestamp::now().as_second() - 86_400;
        if let Ok(mut said) = SAID.lock() {
            said.retain(|(at, _)| *at > day_ago);
            problems.extend(said.iter().map(|(_, p)| problem_text(p)));
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
    // Notes travel by their own folder, unless this device shares them here:
    // say so when no sync seems to carry it.
    let config = load_config();
    if let Some(store) = config.notes_root_path()
        && !here.shares("notes", &config)
        && carried(&path, &roots)
        && !carried(&store, &roots)
    {
        problems.push(say("share-outside", &[("store", shorten(&store))]));
    }
    let traffic = share::traffic(&memory_path(), &here.id);
    let parts = share::PARTS
        .iter()
        .map(|&id| {
            let (name, carries) = part_words(id);
            let refused = refused(id, &config, &here).unwrap_or_default();
            let shared = here.shares(id, &config) && refused.is_empty();
            let last = match traffic.get(id).copied().unwrap_or_default() {
                _ if !on || !shared => String::new(),
                (0, 0) => tr().text("share-part-quiet", None),
                (sent, received) => [(sent, "share-part-sent"), (received, "share-part-received")].iter().filter(|(at, _)| *at > 0).map(|(at, id)| say(id, &[("when", when(*at))])).collect::<Vec<_>>().join(" "),
            };
            // texts: what this device keeps of them, as other parts say what they carry.
            let last = match (id, last.is_empty()) {
                ("texts", false) => [last, crate::texts::kept_words()].join(" ").trim().to_string(),
                _ => last,
            };
            Part { id, name, carries, on: shared, refused, last }
        })
        .collect();
    // Every device holding the newer form back, or on an older Sioul once it travels, has its line.
    let holders: Vec<(String, i64, bool)> = formats.as_ref().map(|f| f.holders.iter().map(|(id, _)| (id, true)).chain(f.older.iter().map(|id| (id, false))).map(|(id, holds)| (id.clone(), f.heard.get(id).copied().unwrap_or(0) / 1000, holds)).collect()).unwrap_or_default();
    let devices = if on { crate::health::device_rows(&holders) } else { Vec::new() };
    let backup = backup(on && mirror.is_none(), &path);
    let build = say("share-build", &[("build", sioul_core::build::DESCRIBED.to_string())]);
    // Pressed while the folder is not found on a server, it says why nothing went.
    let can_again = on;
    let again = if on { again_line(&sioul_sync::remote::Sending::load(&memory_path()).again, &sioul_sync::remote::State::load(&memory_path()).host()) } else { String::new() };
    json(&Status { on, folder: chosen, sealed: share::sealed(&path), projects: here.shares("projects", &config), lines, problems, parts, vanished, devices, backup, servers: server_accounts(&config), mirrored: mirror.is_some(), build, can_again, again })
}

fn problem_text(code: &str) -> String {
    let file = |file: &str| share::shown(file).to_string();
    match code.split_once(':') {
        Some(("share-other-seal", _)) => tr().text("share-other-seal", None),
        Some(("share-unreadable", path)) => say("share-unreadable", &[("file", path.to_string())]),
        Some(("share-conflict", copy)) => say("share-conflict", &[("copy", copy.to_string())]),
        Some(("share-conflict-gone", copy)) => say("share-conflict-gone", &[("copy", copy.to_string())]),
        Some(("share-older-copy", copy)) => say("share-older-copy", &[("copy", copy.to_string())]),
        Some(("share-damaged", name)) => say("share-damaged", &[("file", file(name))]),
        Some(("share-too-big", name)) => say("share-too-big", &[("file", file(name))]),
        Some(("share-missing", name)) => say("share-missing", &[("file", file(name))]),
        Some(("share-no-room", name)) => say("share-no-room", &[("file", file(name))]),
        Some(("share-name-clash", name)) => say("share-name-clash", &[("file", file(name))]),
        Some(("share-refused", name)) => say("share-refused", &[("file", file(name))]),
        Some(("share-not-text", name)) => say("share-not-text", &[("file", file(name))]),
        Some(("share-emptied", name)) => say("share-emptied", &[("file", file(name))]),
        Some(("share-format-wait", name)) => say("share-format-wait", &[("file", file(name))]),
        None if code == "share-files-unreadable" => tr().text("share-files-unreadable", None),
        _ => code.to_string(),
    }
}

/// What is said for a day, not only until the next exchange: files two devices changed, an older copy put back by hand.
fn kept_a_day(code: &str) -> bool {
    code.starts_with("share-conflict") || code.starts_with("share-older-copy")
}

/// `share-vanished:<folder>:<count>`, said with a button: the folder of notes or papers it is in, and the sentence.
fn vanished_of(code: &str) -> Option<Vanished> {
    let (folder, count) = code.strip_prefix("share-vanished:")?.rsplit_once(':')?;
    let store = if folder.starts_with("files/papers/") { "files/papers/" } else { "files/notes/" };
    let shown = match share::shown(folder) {
        "" => load_config().notes_root_path().map(|notes| shorten(&notes)).unwrap_or_default(),
        inside => inside.trim_end_matches('/').to_string(),
    };
    Some(Vanished { store: store.to_string(), text: say("share-vanished", &[("folder", shown), ("count", count.to_string())]) })
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

/// Projects and money shared from this device, or not; "" when kept, else why not.
pub(crate) fn set_projects(on: bool) -> String {
    set_part("projects", on)
}

/// A part shared from this device, or not (`share::PARTS`): this device's own
/// choice, never shared. Notes and papers are refused while a sync app
/// carries the notes folder. "" when kept, else why not.
pub(crate) fn set_part(part: &str, on: bool) -> String {
    let Some(part) = share::PARTS.iter().find(|p| **p == part) else { return format!("{part}?") };
    if on && let Some(why) = refused(part, &load_config(), &here()) {
        return why;
    }
    // Between two exchanges: one never reads half the parts as they were.
    between_exchanges(|| {
        let _writing = HERE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut here = here();
        here.parts.insert((*part).to_string(), on);
        here.save(&state_dir()).err().unwrap_or_default()
    })
}

/// A version kept, as the window lists it.
#[derive(Serialize)]
struct Kept {
    part: &'static str,
    name: String,
    files: Vec<KeptFile>,
    /// The files not shown (a hundred at most are), said; "" when all are.
    more: String,
}

#[derive(Serialize)]
struct KeptFile {
    file: String,
    shown: String,
    count: String,
    versions: Vec<KeptVersion>,
}

#[derive(Serialize)]
struct KeptVersion {
    stamp: String,
    when: String,
    size: String,
}

/// The versions kept here before other devices' changes (`sioul_sync::history`),
/// as JSON: each part's files whose name holds `filter` (any case), the one
/// changed last first, a hundred at most (the others said), their versions.
pub(crate) fn history(filter: &str) -> String {
    let root = sioul_sync::history::root(&memory_path());
    let needle = filter.trim().to_lowercase();
    let kept: Vec<Kept> = share::PARTS
        .iter()
        .filter_map(|&part| {
            let all: Vec<_> = sioul_sync::history::files(&root, part).into_iter().filter(|(file, _)| share::shown(file).to_lowercase().contains(&needle)).collect();
            let more = if all.len() > 100 { say("share-history-more", &[("count", (all.len() - 100).to_string())]) } else { String::new() };
            let files: Vec<KeptFile> = all
                .into_iter()
                .take(100)
                .map(|(file, versions)| KeptFile {
                    shown: share::shown(&file).to_string(),
                    count: say("share-versions", &[("count", versions.len().to_string())]),
                    versions: versions.into_iter().map(|v| KeptVersion { when: when(v.at / 1000), size: sioul_core::view::size(tr(), usize::try_from(v.size).unwrap_or(usize::MAX)), stamp: v.stamp }).collect(),
                    file,
                })
                .collect();
            (!files.is_empty()).then(|| Kept { part, name: part_words(part).0, files, more })
        })
        .collect();
    json(&kept)
}

/// A file put back as it was (`stamp`), the one there now kept in the list
/// too; the next exchange sends it. Waits for an exchange running. Returns
/// what went wrong, else "".
pub(crate) fn put_back(part: &str, file: &str, stamp: &str) -> String {
    let here = here();
    let (folder, key) = (here.folder_path(), key());
    let vault = folder.as_deref().zip(key.as_ref());
    // Every part, those switched off too: put back here only.
    let stores = share::stores_of(&load_config(), &share::Roots::here(), &|_| true);
    quietly(|| share::put_back(&memory_path(), vault, &stores, part, file, stamp, jiff::Timestamp::now().as_millisecond()).err().map(|e| problem_text(&e)).unwrap_or_default())
}

/// The store a file of the records is in, by name ("files/notes/", "config/config.toml"): for the pages to read again.
pub(crate) fn store_of(file: &str) -> String {
    share::stores_of(&load_config(), &share::Roots::here(), &|_| true).into_iter().filter(|s| if s.folder { file.starts_with(s.name.as_str()) } else { s.name == file }).map(|s| s.name).max_by_key(String::len).unwrap_or_default()
}

/// What putting back would do, said before it is done (`share::put_back_preview`).
pub(crate) fn put_back_preview(part: &str, file: &str, stamp: &str) -> String {
    let stores = share::stores_of(&load_config(), &share::Roots::here(), &|_| true);
    let kept = sioul_sync::history::kept_at(stamp).map(|at| when(at / 1000)).unwrap_or_default();
    let shown = share::shown(file).to_string();
    match share::put_back_preview(&memory_path(), &stores, part, file, stamp) {
        Ok(back) if back.whole => say("share-put-back-whole", &[("file", shown), ("when", kept)]),
        Ok(back) if back.changed + back.returning == 0 => say("share-put-back-nothing", &[("file", shown)]),
        Ok(back) => say("share-put-back-entries", &[("file", shown), ("when", kept), ("changed", back.changed.to_string()), ("returning", back.returning.to_string()), ("kept", back.kept.to_string())]),
        Err(e) => e,
    }
}

/// Files gone at once from a folder of notes or papers ("files/notes/") taken out everywhere at the next exchange, as you said; "" when kept, else why not.
pub(crate) fn confirm_gone(store: &str) -> String {
    if !matches!(store, "files/notes/" | "files/papers/") {
        return format!("{store}?");
    }
    let here = here();
    quietly(|| share::confirm_gone(&memory_path(), &here.id, store).err().unwrap_or_default())
}

/// What switching Notes or Papers on would send from here, as JSON {"part", "text"}: how many files, how big, how many stay (too big).
pub(crate) fn estimate(part: &str) -> String {
    let stores = share::stores_of(&load_config(), &share::Roots::here(), &|p| p == part);
    let (count, bytes, big) = share::estimate(&stores);
    let size = sioul_core::view::size(tr(), usize::try_from(bytes).unwrap_or(usize::MAX));
    let text = say("share-estimate", &[("count", count.to_string()), ("size", size), ("big", big.to_string())]);
    json(&serde_json::json!({ "part": part, "text": text }))
}

/// What putting back did, for the status line: the file, and when the version was kept.
pub(crate) fn put_back_said(file: &str, stamp: &str) -> String {
    let kept = sioul_sync::history::kept_at(stamp).map(|at| when(at / 1000)).unwrap_or_default();
    say("share-put-back-done", &[("file", share::shown(file).to_string()), ("when", kept)])
}

/// Stops sharing here: the key forgotten, the folder left as it is for the others.
pub(crate) fn stop() -> String {
    // Between two exchanges: one running never writes its memory back after.
    between_exchanges(|| {
        // The others count this device no more: said while its key is still known.
        crate::devices::left();
        if let Ok(mut held) = KEY.lock() {
            *held = None;
        }
        let _ = sioul_sync::secret::forget_named(share::KEY_NAME);
        let state = state_dir();
        let _writing = HERE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut here = share::Here::load(&state);
        // What was fetched from the server, and where: forgotten with the folder.
        // Sioul keeping the folder itself: this device's last word (it left)
        // goes up first, off the window's thread, then its own copy goes.
        if let Some(folder) = here.folder_path() {
            if mirrored().is_some() {
                let (memory, id) = (memory_path(), here.id.clone());
                std::thread::spawn(move || {
                    mirror_step_for(&folder, &id, false, false);
                    sioul_sync::remote::forget(&memory, &folder);
                    let _ = std::fs::remove_dir_all(sioul_sync::remote::mirror_of(&memory));
                });
            } else if sending_on(&here) {
                // Beside a sync app: its entry saying it left sent too, then forgotten.
                let (memory, leaving) = (memory_path(), here.clone());
                std::thread::spawn(move || {
                    send_to_server(&leaving, false);
                    sioul_sync::remote::forget(&memory, &folder);
                });
            } else {
                sioul_sync::remote::forget(&memory_path(), &folder);
            }
        }
        here.folder = None;
        // Starting again later reads the others first, as the first time.
        let memory = memory_path();
        let _ = std::fs::remove_file(memory.with_file_name("files.json"));
        let _ = std::fs::remove_file(&memory);
        here.save(&state).err().unwrap_or_default()
    })
}

/// A device you said is gone (Forget this device), for the sharing too: it
/// no longer holds the newer form back (`share::forget_device`), until it says
/// anything newer. Sharing off: nothing to do.
pub(crate) fn forget_device(id: &str) {
    let here = here();
    if let (Some(folder), Some(key)) = (attached(&here), key())
        && let Err(e) = share::forget_device(&folder, &key, &memory_path(), &here.id, id)
    {
        eprintln!("sioul: sharing: {e}");
    }
}

/// Whether the doses' part is held here, met in a newer format than this
/// Sioul reads (`share::holds`, from the sharing's memory on the disk: the
/// same after a restart).
pub(crate) fn holds_doses() -> bool {
    on() && share::holds(&memory_path(), "health")
}

/// Whether this device's sharing writes format 2 (`share::FORMAT`), where
/// each pinned site travels apart, and their order as its own setting.
pub(crate) fn writes_format_2() -> bool {
    on() && share::writes(&memory_path()) >= 2
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
    let _claiming = CLAIMING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let here = share::Here::load(&state_dir());
    let alone = sioul_sync::lease::Keeper::alone(&here.id);
    let (Some(folder), Some(key)) = (attached(&here), key()) else { return (alone, false) };
    // The claim says how far this computer wrote its records: read that far, the others know all it marked.
    let wrote = share::written(&memory_path(), &here.id);
    match sioul_sync::lease::renew(&folder, &key, part, &here.id, jiff::Timestamp::now().as_second(), active, take, rule, wrote) {
        Ok(keeper) => (keeper, false),
        Err(_) => (alone, true),
    }
}

/// Who keeps a part, as the claims read now, without claiming it. Sharing off: this computer alone.
pub(crate) fn looked(part: &str, rule: sioul_sync::lease::Rule) -> sioul_sync::lease::Keeper {
    let here = share::Here::load(&state_dir());
    let (Some(folder), Some(key)) = (attached(&here), key()) else { return sioul_sync::lease::Keeper::alone(&here.id) };
    sioul_sync::lease::look(&folder, &key, part, &here.id, jiff::Timestamp::now().as_second(), rule)
}

/// Claims renewed or closed one at a time: a phone put away and back at once
/// never ends with a "closed" written after it came back.
static CLAIMING: Mutex<()> = Mutex::new(());

/// The parts this computer claims (`keeper`): closed together.
const PARTS: [&str; 3] = ["health", "notices", crate::projects::INVOICES];

/// Sioul closes, or is put away on a phone: what was marked goes out at once
/// (an exchange, waiting for one running), then its claims say it closed and
/// how far it wrote, so the others know it marks nothing until it is back
/// (docs/health.md, "Knowing").
pub(crate) fn closing() {
    // Over, unless a phone's Sioul is back already (put away a moment).
    let ended = || !cfg!(target_os = "android") || crate::backend::AWAY.load(std::sync::atomic::Ordering::SeqCst);
    let here = here();
    let (Some(folder), Some(key)) = (here.folder_path(), key()) else {
        crate::devices::window_closed(ended);
        return;
    };
    let memory = memory_path();
    {
        // What was marked goes out; notes and papers wait for the next start.
        HURRY.store(true, std::sync::atomic::Ordering::Relaxed);
        let _busy = BUSY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        HURRY.store(false, std::sync::atomic::Ordering::Relaxed);
        let stores = stores_here(&here);
        let sharing = share::Sharing { folder: &folder, computer: &here.id, key: &key, memory: &memory, files: false, hurry: None };
        if let Ok(outcome) = share::exchange(&sharing, &stores, jiff::Timestamp::now().as_millisecond()) {
            crate::devices::exported(&outcome);
        }
    }
    {
        let _claiming = CLAIMING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // Back already (a phone put away a moment): its claims stay open.
        if ended() {
            let wrote = share::written(&memory, &here.id);
            for part in PARTS {
                let _ = sioul_sync::lease::close(&folder, &key, part, &here.id, jiff::Timestamp::now().as_second(), wrote);
            }
        }
    }
    // The last step of the session, after its final export: down (docs/database.md, "Devices").
    crate::devices::window_closed(ended);
    // Sioul keeping the folder itself: that last word goes up, off the window's
    // thread; a computer quitting waits a few seconds for it, a phone none
    // (its background step sends it).
    let going = if mirrored().is_some() {
        Some(std::thread::spawn(move || mirror_step(&here, false, false)))
    } else if sending_on(&here) {
        // Beside a sync app: the same, sent to the server too.
        Some(std::thread::spawn(move || {
            send_to_server(&here, false);
        }))
    } else {
        None
    };
    if let Some(going) = going {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(if cfg!(target_os = "android") { 0 } else { 5 });
        while !going.is_finished() && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// What this device reads of the others, for the doses (`health::know`).
pub(crate) struct Others {
    /// Their claims on the doses (older Sioul say no more).
    pub claims: Vec<sioul_sync::lease::Claim>,
    /// How far their records were read here.
    pub heard: share::Heard,
    /// Every device sharing through the folder, and when it last exchanged.
    pub others: Vec<share::Other>,
    /// What each says of itself (`sioul_sync::devices`), and the ids of those whose entry does not read.
    pub entries: Vec<sioul_sync::devices::Entry>,
    pub unread: Vec<String>,
}

/// The others' claims on the doses, their entries in the devices' registry,
/// and what was read of their records: what this device knows of the doses
/// they marked (`health::know`). None when sharing is off.
pub(crate) fn others_on_health() -> Option<Others> {
    let here = share::Here::load(&state_dir());
    let (Some(folder), Some(key)) = (attached(&here), key()) else { return None };
    let claims = sioul_sync::lease::claims(&folder, &key, "health").into_iter().filter(|c| c.computer != here.id).collect();
    let (mut entries, mut unread) = sioul_sync::devices::all(&folder, &key);
    entries.retain(|e| e.id != here.id);
    unread.retain(|id| *id != here.id);
    Some(Others { claims, heard: share::heard(&memory_path(), &here.id), others: share::others(&folder, &here.id), entries, unread })
}

/// This device's id, and the folder and key when sharing is on: where its
/// entry in the devices' registry goes (`devices`).
pub(crate) fn vault() -> Option<(String, Option<(PathBuf, [u8; 32])>)> {
    let here = share::Here::load(&state_dir());
    if here.id.is_empty() {
        return None;
    }
    let vault = attached(&here).zip(key());
    Some((here.id, vault))
}

/// Whether this device shares a part (`share::PARTS`), as chosen here.
pub(crate) fn shares(part: &str) -> bool {
    here().shares(part, &load_config())
}

/// The sync app asked to look now (`CARRIERS`), on a phone: after a reminder's
/// session went down, so that its lowered entry goes up (`devices::receiver`).
pub(crate) fn nudge_carriers() {
    ask_carriers();
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

/// The same for the doses' records (`sioul_core::doses`): one lost or broken
/// here is read again from every device's records, nothing of it taken out elsewhere.
pub(crate) fn rebuild_dose_records() -> String {
    let here = share::Here::load(&state_dir());
    let _busy = BUSY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    share::rebuild(&memory_path(), &here.id, &["state/health-doses.toml"]).err().unwrap_or_default()
}

/// Sync apps that can be asked to look for changes now, on a phone: their
/// package, receiver and action. The sharing never depends on it (docs/database.md,
/// "What the sync app must do"): without one, what the other devices wrote
/// comes at the sync app's own pace, later, and doubts say so meanwhile.
/// Each is asked; one not installed hears nothing. Add others the same way,
/// with the source that shows their receiver (and its package in the
/// manifest's <queries>).
#[cfg(target_os = "android")]
const CARRIERS: &[(&str, &str, &str)] = &[
    // Murena's eDrive (/e/OS) looks every half hour; its "force scan" receiver
    // (receivers/DebugCmdReceiver.java, exported) looks now.
    ("foundation.e.drive", "foundation.e.drive.receivers.DebugCmdReceiver", "foundation.e.drive.action.FORCE_SCAN"),
];

/// Every known sync app asked to look now (`CARRIERS`); elsewhere than a phone, nothing.
fn ask_carriers() {
    // Sioul keeping the folder itself: what changed here goes up now, off the caller's thread.
    if mirrored().is_some() {
        std::thread::spawn(|| mirror_step(&here(), false, false));
        return;
    }
    // Beside a sync app: this device's files sent first, then the sync app
    // asked, off the caller's thread. Asked before, it could find the server
    // a version behind the phone's file and bring that older copy down
    // (eDrive: a size that differs, docs/database.md, "Sent to the server too").
    let here = here();
    if sending_on(&here) {
        std::thread::spawn(move || {
            send_to_server(&here, false);
            ask_carriers_only();
        });
        return;
    }
    ask_carriers_only();
}

/// The sync apps asked to look now, and nothing else.
fn ask_carriers_only() {
    #[cfg(target_os = "android")]
    for (package, receiver, action) in CARRIERS {
        let text = |s: &str| std::ffi::CString::new(s).unwrap_or_default();
        let (package, receiver, action) = (text(package), text(receiver), text(action));
        unsafe { crate::backend::sioul_android_broadcast(package.as_ptr(), receiver.as_ptr(), action.as_ptr()) };
    }
}

/// When the sync app was last asked to look (Unix seconds), and whether Sioul is on the screen.
static NUDGED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Sioul on the phone's screen, or put away: the sync app is asked to look only while it is shown.
pub(crate) fn set_shown(shown: bool) {
    SHOWN.store(shown, std::sync::atomic::Ordering::Relaxed);
}

/// On a phone: the sync app asked to look now rather than at its own pace
/// (`ask_carriers`), at most once in `every` seconds (once a minute at least);
/// with `then_read`, an exchange twenty seconds later reads what came down.
/// Elsewhere, nothing: the desktop's sync apps look by themselves within seconds.
pub(crate) fn nudge(qt: &QtThread, shared: &Arc<Shared>, every: i64, then_read: bool) {
    use std::sync::atomic::Ordering;
    let now = jiff::Timestamp::now().as_second();
    let last = NUDGED.load(Ordering::Relaxed);
    if !cfg!(target_os = "android") || now - last < every.max(60) || NUDGED.compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed).is_err() {
        return;
    }
    if share::Here::load(&state_dir()).folder_path().is_none() {
        return;
    }
    ask_carriers();
    if then_read {
        let (qt, shared) = (qt.clone(), Arc::clone(shared));
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(20));
            exchange(&qt, &shared);
        });
    }
}

/// Each minute: while Sioul is shown on a phone, the sync app asked to look every five minutes.
pub(crate) fn nudge_tick(qt: &QtThread, shared: &Arc<Shared>) {
    if SHOWN.load(std::sync::atomic::Ordering::Relaxed) {
        nudge(qt, shared, 5 * 60, true);
    }
}

/// An exchange, when sharing is on: off the window's thread; the pages read
/// again when changes came in.
/// One exchange here and now, on the calling thread, without the window (an
/// Android alarm wakes Sioul with nothing on the screen): with `fetch_first`,
/// the sync app is asked to bring what the other devices wrote, and given
/// twenty seconds (the background service, its pull working: the pull alone,
/// waited for). None when sharing is off. Waits for an exchange running.
pub(crate) fn exchange_here(fetch_first: bool) -> Option<Result<share::Outcome, String>> {
    let here = here();
    let (Some(folder), Some(key)) = (here.folder_path(), key()) else { return None };
    // A phone's calls copied into its own log first (the background step's, a dose's alarm's).
    crate::calls::before_exchange();
    crate::phonemsgs::before_exchange();
    // texts: the phone's step (read, decide, send), the clock, the trims.
    crate::texts::before_exchange();
    let mirror = mirrored().is_some();
    // Sioul keeping the folder itself: the server looked through now, no sync app to wait for.
    if mirror && fetch_first {
        mirror_step(&here, true, true);
    }
    // The background service, its own pull working (`remote::State::pulls_well`):
    // the others' news come from the server alone, the sync app neither asked
    // to look nor waited for, before or after a send (docs/android.md, "In the
    // background"). A dose's or a waking's alarm, in Sioul's own process, asks it as before.
    let pull_alone = cfg!(target_os = "android") && crate::steps::in_service() && !mirror && sioul_sync::remote::State::load(&memory_path()).pulls_well(&folder);
    // The server asked too, meanwhile, when the folder is found there (`fetch_from_server`).
    let fetching = (fetch_first && !mirror).then(|| {
        let here = here.clone();
        std::thread::spawn(move || fetch_from_server(&here, true))
    });
    // A background step that asks the sync app nothing (you asleep and nobody
    // at another device, a file the sync app just wrote) still reads the
    // server at its pace: whether another device is in use is known from there.
    if !fetch_first && crate::steps::in_service() {
        if mirror { mirror_step(&here, true, false) } else { fetch_from_server(&here, false) }
    }
    // The sync app asked to bring the others' news, and given twenty seconds;
    // asked a moment ago already (two doses due at once), only what is left of them.
    if fetch_first && cfg!(target_os = "android") && !mirror && !pull_alone {
        use std::sync::atomic::Ordering;
        let now = jiff::Timestamp::now().as_second();
        let last = NUDGED.load(Ordering::Relaxed);
        let waited = if now - last < 20 {
            now - last
        } else {
            NUDGED.store(now, Ordering::Relaxed);
            ask_carriers();
            0
        };
        std::thread::sleep(std::time::Duration::from_secs((20 - waited).max(0) as u64));
    }
    // What the server brings, waited for a few seconds more at most: never
    // the whole of a slow network's (a dose's alarm has a minute in all). The
    // pull alone, up to the twenty-five seconds it had beside the sync app's
    // wait, never later than before, the step going on as soon as it is done.
    if let Some(fetching) = fetching {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(match (cfg!(target_os = "android"), pull_alone) {
            (true, true) => 25,
            (true, false) => 5,
            (false, _) => 15,
        });
        while !fetching.is_finished() && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    // The one running told to hurry, then waited for: notes and papers wait,
    // what is marked never does (a dose, a button pressed).
    HURRY.store(true, std::sync::atomic::Ordering::Relaxed);
    let _busy = BUSY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    HURRY.store(false, std::sync::atomic::Ordering::Relaxed);
    let stores = stores_here(&here);
    let memory = memory_path();
    let sharing = share::Sharing { folder: &folder, computer: &here.id, key: &key, memory: &memory, files: false, hurry: None };
    let outcome = share::exchange(&sharing, &stores, jiff::Timestamp::now().as_millisecond());
    if let Ok(outcome) = &outcome {
        crate::devices::exported(outcome);
        // Sioul keeping the folder itself: what it wrote goes up now; off this
        // thread for a quick exchange ("Taken" pressed: Android waits eight
        // seconds for its answer), the background step waiting for it.
        if mirror {
            if fetch_first || crate::steps::in_service() {
                mirror_step(&here, false, false);
            } else {
                let here = here.clone();
                std::thread::spawn(move || mirror_step(&here, false, false));
            }
        }
        remember(&outcome.problems, jiff::Timestamp::now().as_second());
        // Inside a reminder's own session, the sync app is asked once it is down (`devices::receiver`).
        let ask = outcome.sent > 0 && !crate::devices::in_receiver();
        // Beside a sync app, this device's files sent to the server too, then
        // the sync app asked to look (it finds them there as here): the
        // background step and an alarm wait for it; a button pressed does not
        // (Android waits eight seconds for its answer).
        if sending_on(&here) {
            let here = here.clone();
            let go = move || {
                send_to_server(&here, false);
                if ask && !pull_alone {
                    ask_carriers_only();
                }
            };
            if fetch_first || crate::steps::in_service() {
                go();
            } else {
                std::thread::spawn(go);
            }
        } else if ask {
            ask_carriers();
        }
    }
    Some(outcome)
}

/// Files two devices changed, both kept: said for a day, not only until the next exchange.
fn remember(problems: &[String], now: i64) {
    if let Ok(mut kept) = SAID.lock() {
        for problem in problems.iter().filter(|p| kept_a_day(p)) {
            kept.retain(|(_, p)| p != problem);
            kept.push((now, problem.clone()));
        }
    }
}

/// An exchange wanted now (a button, a dose answered, a switch, the
/// minute's tick), run by the one worker off the window's thread
/// (`worker`); asked while one runs, another follows it. A shared file
/// saved, seen by the watcher (`watcher`), wants one too, after a pause.
pub(crate) fn exchange(qt: &QtThread, shared: &Arc<Shared>) {
    start_worker(qt, shared);
    want(|soon, now| soon.ask(now));
}

/// The minute's scheduled exchange: left out when one started less than 50
/// seconds ago (a save's), which did its work (`trigger::Soon::tick`).
pub(crate) fn exchange_tick(qt: &QtThread, shared: &Arc<Shared>) {
    start_worker(qt, shared);
    want(|soon, now| soon.tick(now));
}

/// One exchange, on the worker's thread: the server's news first (Sioul
/// keeping the folder, or the backup), then the exchange, then this
/// device's files sent to the server (Sioul keeping the folder, or beside
/// the sync app), the pages read again when changes came in.
fn exchange_now(qt: &QtThread, shared: &Arc<Shared>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    {
        // Sioul keeping the folder itself: the server looked through first; else
        // the other devices' files from the server too, when due, before they are read.
        let mirror = mirrored().is_some();
        if mirror {
            mirror_step(&here(), true, false);
        } else {
            fetch_from_server(&here(), false);
        }
        // A metered or slow connection: full rounds wait (`share::set_frugal`).
        share::set_frugal(&memory_path(), frugal_now());
        // Waits for one running (a dose's alarm, Sioul closing): what was asked is never dropped.
        let busy = BUSY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let here = here();
        let (Some(folder), Some(key)) = (here.folder_path(), key()) else { return };
        // A phone's calls copied into its own log first, so that they go now (`calls`).
        crate::calls::before_exchange();
        crate::phonemsgs::before_exchange();
        // texts: the phone's step (read, decide, send), the clock, the trims.
        crate::texts::before_exchange();
        let stores = stores_here(&here);
        let now = jiff::Timestamp::now();
        let memory = memory_path();
        // Notes and papers only when they can be read whole (Android: "All files access").
        let readable = files_readable();
        let sharing = share::Sharing { folder: &folder, computer: &here.id, key: &key, memory: &memory, files: readable, hurry: Some(&HURRY) };
        let (mut received, mut pending) = (0, 0);
        let (written, accounts, problems, sent) = match share::exchange(&sharing, &stores, now.as_millisecond()) {
            Ok(outcome) => {
                // What it read and wrote, said in this device's entry (docs/database.md, "Devices").
                crate::devices::exported(&outcome);
                (received, pending) = (outcome.received, outcome.pending);
                if !outcome.written.is_empty() {
                    let names = outcome.written.iter().cloned().collect::<Vec<_>>().join("\n");
                    let _ = qt.queue(move |mut sioul| sioul.as_mut().shared_in(QString::from(&names)));
                }
                // The night, its mornings or a day's change come from another device: a phone's alarm at waking follows now (`wake`).
                if outcome.written.contains("data/health.toml") || outcome.written.contains("data/health-days.toml") {
                    crate::wake::schedule();
                }
                // Do-not-disturb pressed, a pause, a focus session or its settings from another device: applied here now.
                if ["state/do-not-disturb.toml", "state/quiet.toml", "data/time/running.toml", "config/config.toml"].iter().any(|f| outcome.written.contains(*f)) {
                    std::thread::spawn(crate::everywhere::apply);
                }
                (!outcome.written.is_empty(), outcome.written.contains("config/config.toml"), outcome.problems, outcome.sent)
            }
            Err(e) => (false, false, vec![e], 0),
        };
        // Whatever was written while the exchange ran (the exchange itself,
        // what runs before it, the minute's other work) is never taken for a
        // save (`watcher`): a change it did not carry goes at the next one.
        if let Ok(mut watched) = WATCHED.lock() {
            *watched = Some(sioul_sync::trigger::Stamps::of(&stores));
        }
        drop(busy);
        // What was marked here goes up now, not at the sync app's next look:
        // Sioul keeping the folder itself sends it, its entry with it; beside
        // a sync app, Sioul sends this device's files too, then the sync app
        // is asked to look: it finds them there as they are here.
        if mirror {
            send_texts(&here);
            mirror_step(&here, false, false);
        } else {
            send_to_server(&here, false);
            if sent > 0 {
                nudge(&qt, &shared, 60, false);
            }
        }
        // On a phone nobody reads the status line: what each exchange did goes to its
        // log (adb logcat), counts and problems only, never what was exchanged.
        if cfg!(target_os = "android") {
            // Codes alone: a note's name is what was exchanged.
            let codes: Vec<&str> = problems.iter().map(|p| p.split(':').next().unwrap_or_default()).collect();
            eprintln!("sioul: sharing: {sent} sent, {received} received, {pending} waiting{}{}", if codes.is_empty() { "" } else { "; " }, codes.join("; "));
        }
        // A part met in a newer form than this Sioul reads: said once in the
        // status line, calmly, the first time this Sioul meets it (Settings
        // says it as long as it lasts).
        for part in problems.iter().filter_map(|p| p.strip_prefix("share-newer:")) {
            crate::backend::set_status(&qt, say("share-newer-status", &[("part", part_words(part).0)]));
        }
        let mut said = problems;
        // Notes and papers wait while Android does not let Sioul read them all.
        if !readable && stores.iter().any(|s| matches!(s.shape, share::Shape::Files)) {
            said.push("share-files-unreadable".into());
        }
        said.dedup();
        remember(&said, now.as_second());
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
    }
}

// ---------------------------------------------------------------- shared as soon as it is saved

/// What wants an exchange (`sioul_sync::trigger::Soon`), and the worker waiting on it.
static SOON: Mutex<sioul_sync::trigger::Soon> = Mutex::new(sioul_sync::trigger::Soon::new());
static SOON_WAKE: std::sync::Condvar = std::sync::Condvar::new();
/// The window's handles, once an exchange was first asked: the worker and the watcher run from then on.
static WORKER: std::sync::OnceLock<(QtThread, Arc<Shared>)> = std::sync::OnceLock::new();
/// The shared files as the watcher last saw them (`sioul_sync::trigger::Stamps`).
static WATCHED: Mutex<Option<sioul_sync::trigger::Stamps>> = Mutex::new(None);

fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

/// An exchange wanted, as `change` says (asked now, or a save seen): the worker woken.
fn want(change: impl FnOnce(&mut sioul_sync::trigger::Soon, i64)) {
    let mut soon = SOON.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    change(&mut soon, now_ms());
    SOON_WAKE.notify_all();
}

/// The worker and the watcher, started once, with the window's handles.
fn start_worker(qt: &QtThread, shared: &Arc<Shared>) {
    if WORKER.set((qt.clone(), Arc::clone(shared))).is_ok() {
        std::thread::spawn(worker);
        std::thread::spawn(watcher);
    }
}

/// The one thread that runs the window's exchanges, each when it is due
/// (docs/database.md, "Sent as soon as it is saved"): asked, at once; saves
/// seen, two seconds after the last, five after the first at most. What is
/// wanted while one runs makes the next. One that fails (a panic) never stops
/// the next ones.
fn worker() {
    let Some((qt, shared)) = WORKER.get() else { return };
    loop {
        {
            let mut soon = SOON.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            loop {
                let now = now_ms();
                match soon.due() {
                    Some(due) if due <= now => {
                        soon.take(now);
                        break;
                    }
                    Some(due) => soon = SOON_WAKE.wait_timeout(soon, std::time::Duration::from_millis((due - now) as u64)).unwrap_or_else(std::sync::PoisonError::into_inner).0,
                    None => soon = SOON_WAKE.wait(soon).unwrap_or_else(std::sync::PoisonError::into_inner),
                }
            }
        }
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exchange_now(qt, shared)));
        give_back_memory();
    }
}

/// What an exchange held a moment (the memory of every line it knows, read
/// and parsed: tens of megabytes with thousands of texts) given back to the
/// system after it, where the C library keeps freed memory for itself
/// (glibc): the window's footprint stays its own, not its largest exchange's.
fn give_back_memory() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> std::ffi::c_int;
        }
        // SAFETY: glibc's own, asked to return the free pages of its heaps; nothing of Rust's is touched.
        unsafe {
            malloc_trim(0);
        }
    }
}

/// Every two seconds, the shared files looked at (their size and time, the
/// notes and papers folders left out): one changed since, by whatever wrote
/// it, wants an exchange after a pause. Not while an exchange runs (what it
/// writes is taken in after it, `exchange_now`); on a phone, only while Sioul
/// is on its screen: in the background, what writes exchanges itself
/// (`exchange_here`).
fn watcher() {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let shown = SHOWN.load(std::sync::atomic::Ordering::Relaxed);
        let here = share::Here::load(&state_dir());
        if !shown || here.folder_path().is_none() || key().is_none() {
            if let Ok(mut watched) = WATCHED.lock() {
                *watched = None;
            }
            continue;
        }
        let _quiet = match BUSY.try_lock() {
            Ok(held) => held,
            Err(std::sync::TryLockError::Poisoned(held)) => held.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => continue,
        };
        let now = sioul_sync::trigger::Stamps::of(&stores_here(&here));
        let Ok(mut watched) = WATCHED.lock() else { continue };
        if watched.as_ref().is_some_and(|before| !now.changed(before).is_empty()) {
            want(|soon, at| soon.saved(at));
        }
        *watched = Some(now);
    }
}

// ---------------------------------------------------------------- sent beside the sync app

/// Sending this device's own files beside the sync app is on here: the
/// backup found the folder on a server, and neither switch is off.
fn sending_on(here: &share::Here) -> bool {
    let Some(folder) = here.folder_path() else { return false };
    let state = sioul_sync::remote::State::load(&memory_path());
    state.mode != sioul_sync::remote::MIRROR && state.confirmed_for(&folder) && state.sending()
}

/// This device's own files sent to the server beside the sync app
/// (`sioul_sync::remote::send`, docs/database.md, "Sent to the server too"):
/// those changed since they went, each over what is there; with `again`,
/// each compared with the server's copy, whatever was sent before. Never on
/// the window's thread. Said in a phone's log, counts and a code only.
fn send_to_server(here: &share::Here, again: bool) -> Option<sioul_sync::remote::Pulled> {
    let folder = here.folder_path()?;
    if mirrored().is_some() || here.id.is_empty() {
        return None;
    }
    let memory = memory_path();
    let now = jiff::Timestamp::now().as_second();
    let state = sioul_sync::remote::State::load(&memory);
    if !state.confirmed_for(&folder) {
        if again {
            sioul_sync::remote::Sending::note_again(&memory, now, "not-confirmed");
        }
        return None;
    }
    let config = load_config();
    let Some(login) = config.accounts.iter().find(|a| a.id == state.account).and_then(login_of) else {
        if again {
            sioul_sync::remote::Sending::note_again(&memory, now, &format!("no-account-for:{}", state.host()));
        }
        return None;
    };
    let sealed = share::own_sealed(&memory, &here.id);
    // The texts to send first, on their own lane: never behind a large file.
    send_texts(here);
    let sent = sioul_sync::remote::send(&memory, &folder, &here.id, &login, &sealed, now, again, metered());
    if sent.sent > 0 || sent.held > 0 || again || sent.problem.as_deref().is_some_and(|p| p != "off" && !p.starts_with("waiting:")) {
        let at = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M:%S").to_string();
        let detail = sent.failure.as_ref().map(|f| format!("; {}", failure_words(f))).unwrap_or_default();
        log_send(match &sent.problem {
            None => format!("{at} sent to {}: {} sent, {} there already, {} records longer there left as they are, {} paced, {} bytes{}", state.host(), sent.sent, sent.same, sent.longer, sent.paced, sent.bytes, if sent.differ.is_empty() { String::new() } else { format!("; otherwise there: {}", sent.differ.join("; ")) }),
            Some(why) => format!("{at} send to {} stopped: {}{detail}", state.host(), backup_words(why)),
        });
    }
    Some(sent)
}

/// A send's failure in words for the log (English, as the log is): "records:
/// timeout after 216 s, 1081344 of 2530934 bytes sent (timeout: global)".
fn failure_words(f: &sioul_sync::remote::Failure) -> String {
    let file = if f.file.is_empty() { String::new() } else { format!("{}: ", f.file) };
    let bytes = if f.total > 0 { format!(", {} of {} bytes handed to the network", f.sent, f.total) } else { String::new() };
    format!("{file}{} after {} s{bytes} ({})", f.kind, f.seconds, f.words)
}

/// A send's words in the log, on every system (a computer's too: its
/// journal): what went, or why not, the same line never twice in a row.
fn log_send(line: String) {
    static SAID: Mutex<String> = Mutex::new(String::new());
    if let Ok(mut said) = SAID.lock() {
        if *said == line {
            return;
        }
        said.clone_from(&line);
    }
    eprintln!("sioul: backup: {line}");
}

/// The connection is metered (NetworkManager: yes, or guessed yes), asked at
/// most every five minutes; on a phone, not known here (measured instead:
/// `Sending::slow`).
fn metered() -> bool {
    static ASKED: Mutex<(i64, bool)> = Mutex::new((0, false));
    if cfg!(target_os = "android") {
        return false;
    }
    let now = jiff::Timestamp::now().as_second();
    let Ok(mut asked) = ASKED.lock() else { return false };
    if now - asked.0 >= 300 {
        *asked = (now, sioul_sync::power::read().metered == Some(true));
    }
    asked.1
}

/// The connection metered or measured slow: the sharing's full rounds wait
/// (`share::set_frugal`), large files go at most every ten minutes.
fn frugal_now() -> bool {
    metered() || sioul_sync::remote::Sending::load(&memory_path()).slow()
}

/// This device's texts to send copied into the sharing folder as a small
/// file of their own, when the part "texts" is shared here, then sent to
/// the server on their own lane (Sioul keeping the folder itself, or beside
/// the sync app): a request reaches the phone within its quarter of an hour
/// whatever the records' size (docs/database.md, "Sent to the server too").
fn send_texts(here: &share::Here) {
    let Some(folder) = here.folder_path() else { return };
    if here.id.is_empty() || !here.shares(sioul_core::texts::PART, &load_config()) {
        return;
    }
    let source = sioul_core::texts::own_file(&sioul_core::texts::folder(), sioul_core::texts::SEND, &here.id);
    if let Err(e) = sioul_sync::remote::place_texts(&folder, &here.id, &source) {
        eprintln!("sioul: texts: {e}");
    }
    let memory = memory_path();
    let state = sioul_sync::remote::State::load(&memory);
    if !state.confirmed_for(&folder) || !(state.mode == sioul_sync::remote::MIRROR || state.sending()) {
        return;
    }
    let config = load_config();
    let Some(login) = config.accounts.iter().find(|a| a.id == state.account).and_then(login_of) else { return };
    let sent = sioul_sync::remote::send_urgent(&memory, &folder, &here.id, &login, jiff::Timestamp::now().as_second());
    if sent.sent > 0 || sent.failure.is_some() {
        let at = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M:%S").to_string();
        log_send(match &sent.failure {
            None => format!("{at} texts to send sent to {}", state.host()),
            Some(f) => format!("{at} texts to send not sent to {}: {}", state.host(), failure_words(f)),
        });
    }
}

/// A text was just written to send (`texts`): its small file placed and sent
/// now, off the caller's thread, not at the next exchange.
pub(crate) fn texts_now() {
    std::thread::spawn(|| send_texts(&here()));
}

/// Where the other devices' texts to send are found beside the records'
/// copy: the sharing folder's small files (`texts/send/<id>.jsonl`, carried
/// by the sync app, or kept in step by Sioul itself), and those fetched from
/// its server (`remote::overlay`). Each a root holding `send/`.
pub(crate) fn texts_copies() -> Vec<PathBuf> {
    let here = share::Here::load(&state_dir());
    let Some(folder) = attached(&here) else { return Vec::new() };
    let mut out = vec![folder.join(sioul_core::texts::FOLDER)];
    out.extend(sioul_sync::remote::overlay(&folder).map(|cache| cache.join(sioul_core::texts::FOLDER)));
    out
}

/// "Send everything again" (Settings ▸ Your folder and sharing): this
/// device's files sent to the server again, each one the server lacks or
/// holds otherwise, whatever was sent before; then the others' news fetched,
/// and an exchange. For when a sync app was late, or the server lost files.
/// Off the window's thread; what it did is said in the panel (`Status::again`).
pub(crate) fn send_again(qt: &QtThread, shared: &Arc<Shared>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let here = here();
        let memory = memory_path();
        let now = jiff::Timestamp::now().as_second();
        match (mirrored(), here.folder_path()) {
            (Some(state), Some(folder)) => match load_config().accounts.iter().find(|a| a.id == state.account).and_then(login_of) {
                Some(login) => {
                    sioul_sync::remote::again_mirror(&memory, &folder, &here.id, &login, now);
                }
                None => sioul_sync::remote::Sending::note_again(&memory, now, &format!("no-account-for:{}", state.host())),
            },
            (None, Some(_)) => {
                send_to_server(&here, true);
                fetch_from_server(&here, true);
            }
            _ => {}
        }
        exchange(&qt, &shared);
    });
}

/// This device's own files sent to the server beside the sync app, or not:
/// this device's own choice, never shared. "" when kept, else why not.
pub(crate) fn set_send(on: bool) -> String {
    sioul_sync::remote::State::choose(&memory_path(), |state| state.send = (!on).then_some(false)).err().unwrap_or_default()
}

// ---------------------------------------------------------------- kept in step by Sioul itself

/// Sioul keeping the folder in step with the server itself here (`remote::MIRROR`): its state.
fn mirrored() -> Option<sioul_sync::remote::State> {
    let state = sioul_sync::remote::State::load(&memory_path());
    (state.mode == sioul_sync::remote::MIRROR).then_some(state)
}

/// A step of Sioul keeping the folder itself (`sioul_sync::remote::step`):
/// `whole`, the server looked through first and what is new there brought
/// (at most every twenty seconds, unless `urgent`); then what changed here
/// sent. Never on the window's thread.
fn mirror_step(here: &share::Here, whole: bool, urgent: bool) {
    if let Some(folder) = here.folder_path() {
        mirror_step_for(&folder, &here.id, whole, urgent);
    }
}

fn mirror_step_for(folder: &Path, own: &str, whole: bool, urgent: bool) {
    let Some(state) = mirrored().filter(|s| s.confirmed_for(folder)) else { return };
    let now = jiff::Timestamp::now().as_second();
    let whole = whole && (urgent || now - state.tried >= 20);
    let config = load_config();
    let Some(login) = config.accounts.iter().find(|a| a.id == state.account).and_then(login_of) else { return };
    let stepped = sioul_sync::remote::step(&memory_path(), folder, own, &login, now, whole);
    // On a phone, its log (adb logcat): counts and a code, never a name nor an address.
    if cfg!(target_os = "android") && (stepped.fetched + stepped.sent > 0 || stepped.problem.is_some()) {
        let code = stepped.problem.as_deref().map(|p| format!("; {}", p.split(':').next().unwrap_or_default())).unwrap_or_default();
        eprintln!("sioul: sharing: {} fetched, {} sent by Sioul itself{code}", stepped.fetched, stepped.sent);
    }
}

/// An account Sioul can keep the folder with, for the panel: its id, and how to say it.
#[derive(Serialize)]
struct ServerAccount {
    id: String,
    label: String,
}

/// The contacts-and-calendars accounts whose server keeps files Nextcloud's
/// way, without their passwords (read on the window's thread: no keyring).
fn server_accounts(config: &Config) -> Vec<ServerAccount> {
    config
        .accounts
        .iter()
        .filter(|a| a.is_dav() && a.auth.as_deref() != Some("google") && a.url.as_deref().is_some_and(|u| u.contains("/remote.php/")))
        .map(|a| {
            let host = sioul_sync::remote::host_of(a.url.as_deref().unwrap_or_default());
            let who = a.address.clone().unwrap_or_else(|| a.id.clone());
            ServerAccount { id: a.id.clone(), label: if who.ends_with(&host) { who } else { format!("{who} ({host})") } }
        })
        .collect()
}

/// Where Sioul keeping the folder itself stands, in words: when last in step, or why not.
fn mirror_line(state: &sioul_sync::remote::State) -> String {
    let host = state.host();
    let (code, _) = state.said.split_once(':').unwrap_or((state.said.as_str(), ""));
    if state.confirmed == 0 {
        return say("share-server-unconfirmed", &[("host", host)]);
    }
    let why = || {
        tr().text(
            match code {
                "login" => "share-backup-why-login",
                "tls" => "share-backup-why-tls",
                "network" => "share-backup-why-network",
                "disk" => "share-backup-why-disk",
                "quota" => "share-backup-why-quota",
                _ => "share-backup-why-server",
            },
            None,
        )
    };
    let last = state.last.max(state.pushed);
    match (code, last) {
        ("", 0) => String::new(),
        ("", last) => say("share-server-last", &[("when", when(last))]),
        (_, 0) => say("share-server-failing-never", &[("why", why())]),
        (_, last) => say("share-server-failing", &[("when", when(last)), ("why", why())]),
    }
}

/// A start or a join that did not go, in words.
fn server_problem(code: &str) -> String {
    let (kind, about) = code.split_once(':').unwrap_or((code, ""));
    let about = || about.to_string();
    match kind {
        "no-place" => tr().text("share-server-no-place", None),
        "no-files" => say("share-server-no-files", &[("host", about())]),
        "no-password" => tr().text("share-server-no-password", None),
        "login" => say("share-server-refused", &[("host", about())]),
        "tls" => say("share-server-tls", &[("host", about())]),
        "network" => say("share-server-unreachable", &[("host", about())]),
        "sealed-meanwhile" => tr().text("share-server-sealed-meanwhile", None),
        _ => say("share-server-error", &[("host", about())]),
    }
}

/// Starts sharing through a folder of an account's files that Sioul keeps in
/// step itself (`remote::MIRROR`): joined when another device shares through
/// it already (its seal, the passphrase chosen there), else made there and
/// sealed (the passphrase typed twice). Off the window's thread: the result
/// comes through `share_started` ("" when it did, else why not).
pub(crate) fn start_on_server(qt: &QtThread, shared: &Arc<Shared>, account: String, place: String, passphrase: String, again: String) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let started = (|| -> Result<(), String> {
            if passphrase.chars().count() < 12 {
                return Err(tr().text("share-short", None));
            }
            let config = load_config();
            let login = config.accounts.iter().find(|a| a.id == account).and_then(login_of).ok_or_else(|| tr().text("share-server-none", None))?;
            if login.password.is_none() {
                return Err(tr().text("share-server-no-password", None));
            }
            let opened = sioul_sync::remote::open(&login, &place).map_err(|code| server_problem(&code))?;
            let memory = memory_path();
            let mirror = sioul_sync::remote::mirror_of(&memory);
            // A copy of its own, new: what an earlier sharing left there goes.
            let _ = std::fs::remove_dir_all(&mirror);
            share::private_dirs(&mirror).map_err(|e| format!("{}: {e}", mirror.display()))?;
            let key = match &opened.seal {
                // Joining: the folder's seal, the passphrase chosen there.
                Some(seal) => {
                    share::write_atomically(&mirror.join("seal.toml"), seal)?;
                    share::key_for(&mirror, &passphrase)
                }
                None if passphrase != again => return Err(tr().text("share-differ", None)),
                None => share::key_for(&mirror, &passphrase),
            };
            let key = match key {
                Ok(key) => key,
                Err(share::Refused::WrongPassphrase) => {
                    let _ = std::fs::remove_dir_all(&mirror);
                    return Err(tr().text("share-wrong", None));
                }
                Err(share::Refused::Other(e)) => return Err(e),
            };
            if opened.seal.is_none() {
                sioul_sync::remote::create(&login, &opened, &mirror).map_err(|code| {
                    let _ = std::fs::remove_dir_all(&mirror);
                    server_problem(&code)
                })?;
            }
            sioul_sync::secret::save_named(share::KEY_NAME, &hex(&key)).map_err(|e| e.sentence(tr(), "Sioul"))?;
            if let Ok(mut held) = KEY.lock() {
                *held = Some(key);
            }
            sioul_sync::remote::begin(&memory, &mirror, &login, &opened, jiff::Timestamp::now().as_second())?;
            let state = state_dir();
            let _writing = HERE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut here = share::Here::load(&state);
            here.folder = Some(mirror.display().to_string());
            here.save(&state)
        })();
        let problem = started.err().unwrap_or_default();
        // The first exchange brings what the others shared there.
        if problem.is_empty() {
            exchange(&qt, &shared);
        }
        let _ = qt.queue(move |mut sioul| sioul.as_mut().share_started(QString::from(&problem)));
    });
}

// ---------------------------------------------------------------- fetched from the server too

/// One look or fetch at a time in this process: two exchanges' threads never fetch at once.
static FETCHING: Mutex<()> = Mutex::new(());

/// The sharing folder, and what was fetched from its server read beside it
/// from now on in this process (`sioul_sync::remote::attach`): cheap, no network.
fn attached(here: &share::Here) -> Option<PathBuf> {
    let folder = here.folder_path()?;
    sioul_sync::remote::attach(&folder, &memory_path());
    Some(folder)
}

/// The contacts-and-calendars accounts whose server may keep files
/// Nextcloud's way (found under `/remote.php/`: Nextcloud, Murena, ownCloud),
/// each with its password from the keyring as their sync has it. Google's never.
fn logins(config: &Config) -> Vec<sioul_sync::remote::Login> {
    config.accounts.iter().filter(|a| a.auth.as_deref() != Some("google")).filter_map(login_of).collect()
}

fn login_of(account: &sioul_core::config::Account) -> Option<sioul_sync::remote::Login> {
    let url = account.url.clone().filter(|u| account.is_dav() && u.contains("/remote.php/"))?;
    Some(sioul_sync::remote::Login { account: account.id.clone(), url, user: account.login()?.to_string(), password: sioul_sync::secret::password(account).ok() })
}

/// What the Nextcloud client says it carries, on a computer: its `nextcloud.cfg`.
fn nextcloud_configs() -> Vec<String> {
    if cfg!(target_os = "android") {
        return Vec::new();
    }
    let home = expand_home("~");
    let windows = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Nextcloud/nextcloud.cfg"));
    [Some(home.join(".config/Nextcloud/nextcloud.cfg")), Some(home.join("Library/Preferences/Nextcloud/nextcloud.cfg")), windows].into_iter().flatten().filter_map(|p| std::fs::read_to_string(p).ok()).collect()
}

/// The backup's words for a phone's log (`adb logcat -s sioul`): what it did
/// or why not, and when; never a password, a name of yours, an address or a
/// content. A line the same as the last one is not said again.
fn log_backup(line: String) {
    static SAID: Mutex<String> = Mutex::new(String::new());
    if !cfg!(target_os = "android") {
        return;
    }
    if let Ok(mut said) = SAID.lock() {
        if *said == line {
            return;
        }
        said.clone_from(&line);
    }
    eprintln!("sioul: backup: {line}");
}

/// A backup's code (`State::said`) in plain words, for the log: "network:murena.io" → "murena.io: could not be reached".
fn backup_words(said: &str) -> String {
    let (code, about) = said.split_once(':').unwrap_or((said, ""));
    let why = match code {
        "network" => "could not be reached",
        "login" => "refused the password kept for the account",
        "tls" => "its certificate could not be checked",
        "server" => "answered with an error",
        "quota" => "its space is full",
        "disk" => "could not be written here",
        "no-password" => "no password kept on this device for the account",
        "not-found" => "the folder is not there",
        "seal-differs" => "a folder sealed otherwise, never read",
        "seal-gone" => "the folder holds no seal any more",
        "no-account" => "no account with a Nextcloud server",
        "no-account-for" => "no account on that server",
        "no-seal" => "no seal in the folder here",
        "off" => "switched off here",
        other => other,
    };
    if about.is_empty() { why.to_string() } else { format!("{about}: {why}") }
}

/// The other devices' files fetched from the server too (docs/database.md,
/// "Fetched from the server too"), when the sharing folder is found on one of
/// your accounts' servers under the same seal, and a pull is due
/// (`sioul_sync::remote::due`: the first at once, then at the pace of how
/// this device stands). Looked for at once when asked, else every six hours.
/// Never on the window's thread; the switch off, not a request. Each look and
/// each pull said in a phone's log.
fn fetch_from_server(here: &share::Here, urgent: bool) {
    let Some(folder) = attached(here) else { return };
    if mirrored().is_some() {
        return;
    }
    // One at a time; one that broke (a panic) never stops the next ones.
    let _fetching = match FETCHING.try_lock() {
        Ok(held) => held,
        Err(std::sync::TryLockError::Poisoned(held)) => held.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return,
    };
    let memory = memory_path();
    let now = jiff::Timestamp::now().as_second();
    let mut state = sioul_sync::remote::State::load(&memory);
    if !state.fetching() || here.id.is_empty() {
        sioul_sync::remote::tidy(&memory, &folder, &here.id);
        log_backup("switched off here".into());
        return;
    }
    let config = load_config();
    let login = |account: &str| config.accounts.iter().find(|a| a.id == account).and_then(login_of);
    // Its account gone, or its password: looked for again.
    if state.look_due(&folder, now) || (state.confirmed_for(&folder) && login(&state.account).is_none_or(|l| l.password.is_none())) {
        let places = sioul_sync::remote::places(&folder, &nextcloud_configs());
        let logins = logins(&config);
        state = sioul_sync::remote::find(&memory, &folder, &logins, &places, now);
        log_backup(if state.confirmed_for(&folder) { format!("the folder found on {}", state.host()) } else { format!("the folder not found on {} account(s): {}", logins.len(), backup_words(&state.said)) });
    }
    if !state.confirmed_for(&folder) {
        return;
    }
    let phone = cfg!(target_os = "android");
    let asleep = phone && crate::everywhere::rest_now() == (true, false);
    let pace = sioul_sync::remote::Pace {
        phone,
        // On the phone's screen: the window's process only, the background service's never.
        shown: !crate::steps::in_service() && SHOWN.load(std::sync::atomic::Ordering::Relaxed),
        in_use: phone && crate::devices::others_in_use(),
        urgent,
        step: if asleep { crate::steps::STEP_ASLEEP } else { crate::steps::STEP_AWAKE },
    };
    if !sioul_sync::remote::due(&state, now, pace) {
        let pace_said = match (pace.shown, pace.in_use, asleep) {
            (true, _, _) => "Sioul on the screen: once a minute",
            (_, true, _) => "another device in use: once a minute",
            (_, false, true) => "in the background, nobody at another device, at night: every fifteen minutes",
            _ => "in the background, nobody at another device: every five minutes",
        };
        log_backup(format!("next pull at its pace ({pace_said})"));
        return;
    }
    let Some(login) = login(&state.account) else { return };
    let pulled = sioul_sync::remote::pull(&memory, &folder, &here.id, &login, now);
    let at = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M:%S").to_string();
    log_backup(match &pulled.problem {
        None => format!("{at} pulled from {}: {} listed, {} fetched, {} bytes", state.host(), pulled.listed, pulled.fetched, pulled.bytes),
        Some(why) => format!("{at} pull from {} stopped: {}", state.host(), backup_words(why)),
    });
}

/// Fetched from the server too, as the panel shows it.
#[derive(Serialize)]
struct Backup {
    /// Shown: sharing is on.
    shown: bool,
    /// Where things stand, in words.
    line: String,
    /// The switch: fetching from the server, on this device.
    on: bool,
    /// The place given by hand; "" when looked for by itself.
    given: String,
    /// Found under the same seal.
    found: bool,
    /// The second switch: this device's own files sent there too, beside the sync app.
    send: bool,
    /// Its words, with the server's name when it is known.
    send_switch: String,
    /// Where the sending stands, in words: when last, or why not.
    send_line: String,
}

fn backup(on: bool, folder: &Path) -> Backup {
    let state = sioul_sync::remote::State::load(&memory_path());
    let line = if on { backup_line(&state, folder) } else { String::new() };
    let host = state.host();
    let send_switch = if host.is_empty() { tr().text("share-send-switch-none", None) } else { say("share-send-switch", &[("host", host)]) };
    let send_line = if on && state.fetching() { send_line(&state, &sioul_sync::remote::Sending::load(&memory_path()), folder) } else { String::new() };
    Backup { shown: on, line, on: state.fetching(), given: state.given.clone(), found: state.confirmed_for(folder), send: state.sending(), send_switch, send_line }
}

/// A send's code in words, for the panel: "network" → "the server could not be reached".
fn why_words(code: &str) -> String {
    tr().text(
        match code {
            "login" => "share-backup-why-login",
            "tls" => "share-backup-why-tls",
            "network" => "share-backup-why-network",
            "disk" => "share-backup-why-disk",
            "quota" => "share-backup-why-quota",
            _ => "share-backup-why-server",
        },
        None,
    )
}

/// Where sending this device's files beside the sync app stands, in words.
fn send_line(state: &sioul_sync::remote::State, sending: &sioul_sync::remote::Sending, folder: &Path) -> String {
    let host = state.host();
    if !state.sending() {
        return if host.is_empty() { String::new() } else { say("share-send-off", &[("host", host)]) };
    }
    if !state.confirmed_for(folder) {
        return tr().text("share-send-waiting", None);
    }
    let (code, _) = sending.said.split_once(':').unwrap_or((sending.said.as_str(), ""));
    let line = match (code, sending.last) {
        ("", 0) => say("share-send-soon", &[("host", host)]),
        ("", last) => say("share-send-on", &[("host", host), ("when", when(last))]),
        (code, 0) => say("share-send-failing-never", &[("host", host), ("why", why_words(code))]),
        (code, last) => say("share-send-failing", &[("host", host), ("when", when(last)), ("why", why_words(code))]),
    };
    // What failed, in detail: which file, how far it went, when it is tried again.
    match sending.failure.as_ref().filter(|_| !sending.said.is_empty()) {
        Some(failure) => [line, failure_line(failure, sending.next_try())].join(" "),
        None => line,
    }
}

/// A failed send in detail, in words: "Your records did not go at 19:24:
/// 1.1 MB of 2.5 MB went in 216 s, slower than Sioul waits for. Tried again at 19:39."
fn failure_line(failure: &sioul_sync::remote::Failure, next: i64) -> String {
    let size = |bytes: u64| sioul_core::view::size(tr(), usize::try_from(bytes).unwrap_or(usize::MAX));
    let detail = match failure.kind.as_str() {
        "timeout" => say("share-send-detail-timeout", &[("total", size(failure.total)), ("seconds", failure.seconds.to_string())]),
        "stalled" => say("share-send-detail-stalled", &[("total", size(failure.total))]),
        "network" if failure.sent > 0 => say("share-send-detail-cut", &[("total", size(failure.total))]),
        "out-of-time" => tr().text("share-send-detail-out-of-time", None),
        "unreachable" | "network" => tr().text("share-send-detail-unreachable", None),
        kind => format!("{}.", why_words(kind)),
    };
    let file = match failure.file.as_str() {
        "records" => "share-send-file-records",
        "entry" => "share-send-file-entry",
        "notes" => "share-send-file-notes",
        "claim" => "share-send-file-claim",
        "texts" => "share-send-file-texts",
        "sealed" => "share-send-file-sealed",
        _ => "",
    };
    let next = if next > 0 { say("share-send-next", &[("when", when(next))]) } else { String::new() };
    let said = if file.is_empty() { say("share-send-failure", &[("when", when(failure.at)), ("detail", detail)]) } else { say("share-send-failure-file", &[("file", tr().text(file, None)), ("when", when(failure.at)), ("detail", detail)]) };
    [said, next].join(" ").trim().to_string()
}

/// What "Send everything again" did, in words: "Sent 14 files to cloud.example.org at 10:42.", or why not.
fn again_line(again: &sioul_sync::remote::Again, host: &str) -> String {
    if again.at == 0 {
        return String::new();
    }
    let (code, about) = again.said.split_once(':').unwrap_or((again.said.as_str(), ""));
    let host = if about.is_empty() { host.to_string() } else { about.to_string() };
    let at = when(again.at);
    let counted = |id: &str, count: usize| {
        let mut args = sioul_core::i18n::args();
        args.set("count", count as i64);
        args.set("host", host.clone());
        args.set("when", at.clone());
        tr().text(id, Some(&args))
    };
    match code {
        "" if again.sent > 0 => counted("share-send-again-sent", again.sent),
        "" => counted("share-send-again-same", again.same),
        "off" => tr().text("share-send-again-off", None),
        "not-confirmed" | "mirror" | "no-seal" | "seal-gone" | "seal-differs" => tr().text("share-send-again-not-found", None),
        "no-account-for" => say("share-send-again-no-account", &[("host", host)]),
        code => say("share-send-again-failed", &[("host", host), ("when", at), ("why", why_words(code))]),
    }
}

/// Where the backup stands, in words: from where, and since when, or why not.
fn backup_line(state: &sioul_sync::remote::State, folder: &Path) -> String {
    let host = state.host();
    let (code, about) = state.said.split_once(':').unwrap_or((state.said.as_str(), ""));
    let about = || about.to_string();
    if !state.fetching() {
        return if host.is_empty() { tr().text("share-backup-off-none", None) } else { say("share-backup-off", &[("host", host)]) };
    }
    if state.confirmed_for(folder) {
        let why = |code: &str| {
            tr().text(
                match code {
                    "login" => "share-backup-why-login",
                    "tls" => "share-backup-why-tls",
                    "network" => "share-backup-why-network",
                    "disk" => "share-backup-why-disk",
                    _ => "share-backup-why-server",
                },
                None,
            )
        };
        return match (code, state.last) {
            ("", 0) => say("share-backup-soon", &[("host", host)]),
            ("", last) => say("share-backup-on", &[("host", host), ("when", when(last))]),
            (code, 0) => say("share-backup-failing-never", &[("host", host), ("why", why(code))]),
            (code, last) => say("share-backup-failing", &[("host", host), ("when", when(last)), ("why", why(code))]),
        };
    }
    if state.looked == 0 || state.folder != folder.display().to_string() || state.asked > state.looked {
        return tr().text("share-backup-looking", None);
    }
    match code {
        "no-account" => tr().text("share-backup-no-account", None),
        "no-account-for" => say("share-backup-no-account-for", &[("host", about())]),
        "not-found" => say("share-backup-not-found", &[("hosts", about())]),
        "seal-differs" => say("share-backup-seal-differs", &[("host", about())]),
        "seal-gone" => say("share-backup-seal-gone", &[("host", about())]),
        "login" => say("share-backup-login", &[("host", about())]),
        "no-password" => say("share-backup-no-password", &[("account", about())]),
        "no-seal" => String::new(),
        _ => say("share-backup-unreachable", &[("host", about())]),
    }
}

/// The other devices' files fetched from the server too, or not: this
/// device's own choice, never shared. "" when kept, else why not.
pub(crate) fn set_backup(on: bool) -> String {
    let now = jiff::Timestamp::now().as_second();
    let chosen = sioul_sync::remote::State::choose(&memory_path(), |state| {
        state.on = (!on).then_some(false);
        // On again: looked for at once.
        if on {
            state.asked = now;
        }
    });
    chosen.err().unwrap_or_default()
}

/// The sharing folder's place on the server, given by hand ("Documents/Sioul",
/// or its address; "" to look for it by itself): looked at with the next
/// exchange, that place alone. "" when kept, else why not.
pub(crate) fn set_backup_place(place: &str) -> String {
    let now = jiff::Timestamp::now().as_second();
    let chosen = sioul_sync::remote::State::choose(&memory_path(), |state| {
        state.given = place.trim().to_string();
        state.asked = now;
    });
    chosen.err().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeping_the_folder_says_in_words_where_it_stands() {
        use sioul_sync::remote::{MIRROR, State};
        let kept = State { mode: MIRROR.into(), place: "Documents/Sioul".into(), url: "https://murena.io/remote.php/dav/files/jane/Documents/Sioul/".into(), confirmed: 1, ..State::default() };
        assert_eq!(mirror_line(&kept), "", "nothing to say before its first step");
        let stepped = State { last: jiff::Timestamp::now().as_second(), ..kept.clone() };
        assert!(!mirror_line(&stepped).is_empty());
        for said in ["network:murena.io", "login:murena.io", "tls:murena.io", "server:murena.io", "quota:murena.io", "disk:murena.io"] {
            let line = mirror_line(&State { said: said.into(), ..stepped.clone() });
            assert!(!line.is_empty() && !line.contains("share-"), "{said}: {line}");
        }
        assert!(mirror_line(&State { confirmed: 0, said: "seal-differs:murena.io".into(), ..kept }).contains("murena.io"));
        for code in ["no-place", "no-files:murena.io", "no-password:murena", "login:murena.io", "tls:murena.io", "network:murena.io", "server:murena.io", "sealed-meanwhile"] {
            let words = server_problem(code);
            assert!(!words.is_empty() && !words.contains("share-"), "{code}: {words}");
        }
    }

    /// A send that failed, said in words, whichever way it failed: the file,
    /// how long it tried, when it is tried again; never a message's key.
    #[test]
    fn a_failed_send_is_said_in_words() {
        use sioul_sync::remote::Failure;
        let at = jiff::Timestamp::now().as_second();
        let timeout = Failure { kind: "timeout".into(), file: "records".into(), seconds: 216, sent: 1_081_344, total: 2_530_934, words: "timeout: global".into(), at };
        let line = failure_line(&timeout, at + 60);
        assert!(line.contains("216") && !line.contains("share-"), "{line}");
        for kind in ["stalled", "network", "unreachable", "out-of-time", "tls", "login", "quota", "server"] {
            for file in ["records", "entry", "notes", "claim", "texts", "sealed", ""] {
                let line = failure_line(&Failure { kind: kind.into(), file: file.into(), ..timeout.clone() }, 0);
                assert!(!line.is_empty() && !line.contains("share-"), "{kind} {file}: {line}");
            }
        }
        assert_eq!(failure_words(&timeout), "records: timeout after 216 s, 1081344 of 2530934 bytes handed to the network (timeout: global)");
    }

    #[test]
    fn the_backup_says_in_words_what_it_did() {
        assert_eq!(backup_words("network:murena.io"), "murena.io: could not be reached");
        assert_eq!(backup_words("no-account"), "no account with a Nextcloud server");
        assert_eq!(backup_words("seal-differs:murena.io"), "murena.io: a folder sealed otherwise, never read");
    }

    #[test]
    fn where_the_backup_stands_in_words() {
        use sioul_sync::remote::State;
        let folder = Path::new("/storage/emulated/0/Documents/Sioul");
        let here = folder.display().to_string();
        let found = State { folder: here.clone(), url: "https://murena.io/remote.php/dav/files/jane/Documents/Sioul/".into(), account: "murena".into(), confirmed: 1, looked: 1, ..State::default() };
        assert!(backup_line(&found, folder).contains("murena.io"));
        let fetched = State { last: jiff::Timestamp::now().as_second(), ..found.clone() };
        assert!(backup_line(&fetched, folder).contains("murena.io"));
        let failing = State { said: "network:murena.io".into(), ..fetched.clone() };
        assert_ne!(backup_line(&failing, folder), backup_line(&fetched, folder));
        let off = State { on: Some(false), ..fetched.clone() };
        assert!(backup_line(&off, folder).contains("murena.io"));
        let looking = State { folder: here.clone(), ..State::default() };
        assert_eq!(backup_line(&looking, folder), tr().text("share-backup-looking", None));
        for said in ["no-account", "not-found:murena.io", "seal-differs:murena.io", "seal-gone:murena.io", "login:murena.io", "no-password:murena", "network:murena.io", "no-account-for:cloud.example.org"] {
            let state = State { folder: here.clone(), looked: 1, said: said.into(), ..State::default() };
            let line = backup_line(&state, folder);
            assert!(!line.is_empty() && !line.contains("share-backup"), "{said}: {line}");
        }
    }

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
    fn notes_carried_twice_are_refused() {
        let home = std::env::temp_dir().join(format!("sioul-share-carried-{}", std::process::id()));
        let roots = vec![home.join("Nextcloud")];
        assert!(carried(&home.join("Nextcloud").join("Notes"), &roots));
        assert!(!carried(&home.join("Notes"), &roots));
        // On a phone: notes in the top folder of its storage the sharing folder is in, whatever name the storage goes by.
        let storage = Path::new(STORAGE);
        assert!(same_top(&storage.join("Documents/Notes"), &storage.join("Documents/Sioul"), storage));
        assert!(!same_top(&storage.join("Notes"), &storage.join("Documents/Sioul"), storage));
        assert!(!same_top(Path::new("/data/user/0/sioul/files/Notes"), &storage.join("Documents/Sioul"), storage));
        assert!(same_top(&on_storage(Path::new("/sdcard/Documents/Notes")), &on_storage(&storage.join("Documents/Sioul")), storage));
        assert!(same_top(&on_storage(Path::new("/storage/self/primary/Documents/Notes")), &on_storage(Path::new("/sdcard/Documents/Sioul")), storage));
        // Syncthing's folders and Dropbox's, wherever they are.
        let xml = "<configuration>\n<folder id=\"a\" label=\"Notes\" path=\"~/Writing &amp; Notes\" type=\"sendreceive\">\n<device id=\"X\"></device>\n</folder>\n<folder path=\"/data/Sync\" id=\"b\"></folder>\n<folderish/>\n</configuration>";
        assert_eq!(syncthing_folders(xml), ["~/Writing & Notes", "/data/Sync"]);
        let mut boxes = dropbox_folders(r#"{"personal": {"path": "/home/me/Boxes/Dropbox", "host": 1}, "business": {"path": "/home/me/Work"}}"#);
        boxes.sort();
        assert_eq!(boxes, ["/home/me/Boxes/Dropbox", "/home/me/Work"]);
        assert!(dropbox_folders("not json").is_empty());
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
