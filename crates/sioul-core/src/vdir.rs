// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Address books and calendars on disk, as vdir: one folder per collection,
//! one file per contact (`.vcf`) or per event (`.ics`), the collection's name
//! and colour in the files `displayname` and `color`. pimsync, vdirsyncer,
//! khal and khard read the same folders
//! (<https://vdirsyncer.pimutils.org/en/stable/vdir.html>).
//!
//! What sync needs to remember, where each file sits on the server and its
//! ETag, lives apart in the state folder: the data folders hold only your
//! contacts and events.

use crate::config::{data_dir, state_dir};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Contacts,
    Calendars,
}

impl Kind {
    /// `~/.local/share/sioul/contacts`, `…/calendars`.
    pub fn root(self) -> PathBuf {
        data_dir().join(self.folder())
    }

    fn folder(self) -> &'static str {
        match self {
            Kind::Contacts => "contacts",
            Kind::Calendars => "calendars",
        }
    }

    /// The extension of its items.
    pub fn extension(self) -> &'static str {
        match self {
            Kind::Contacts => "vcf",
            Kind::Calendars => "ics",
        }
    }

    /// The media type a server expects for its items.
    pub fn media_type(self) -> &'static str {
        match self {
            Kind::Contacts => "text/vcard; charset=utf-8",
            Kind::Calendars => "text/calendar; charset=utf-8",
        }
    }
}

/// One address book or calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub kind: Kind,
    pub account: String,
    /// Its folder's name, made from the server's address of it.
    pub id: String,
    pub dir: PathBuf,
    pub name: String,
    /// "#4c6b5c", when the server gives one.
    pub color: Option<String>,
    /// The server lets you read it only.
    pub read_only: bool,
    /// For calendars: what it may hold ("VEVENT", "VTODO"); empty when the server does not say.
    pub components: Vec<String>,
}

impl Collection {
    /// Where it lives, in a word: "Google", its server's domain ("example.org");
    /// "" for one kept on this computer only.
    pub fn place(&self, config: &crate::config::Config) -> String {
        if self.account == LOCAL {
            return String::new();
        }
        let account = config.account(&self.account);
        if matches!(crate::capabilities::provider_of_collection(account, self), crate::capabilities::Provider::Google | crate::capabilities::Provider::GoogleTasks) {
            return "Google".into();
        }
        let address = account.and_then(|a| a.url.clone().or_else(|| a.host.clone())).unwrap_or_default();
        let host = crate::sites::host_of(&if address.contains("://") { address } else { format!("https://{address}") });
        let domain = crate::sites::domain_of(&host);
        if domain.is_empty() { self.account.clone() } else { domain }
    }

    /// Its name and where it lives: "Agenda · example.org", "Maison · on this computer only";
    /// a calendar taking events and tasks, "Personnel · example.org · events and tasks".
    pub fn label(&self, config: &crate::config::Config, tr: &crate::i18n::Translator) -> String {
        let place = self.place(config);
        let place = if place.is_empty() { tr.text("collection-here", None) } else { place };
        // One that takes events and tasks says so, wherever it is offered (21 October 2026).
        if self.takes_both() { format!("{} · {place} · {}", self.name, tr.text("collection-events-and-tasks", None)) } else { format!("{} · {place}", self.name) }
    }

    /// Whether it takes items of this kind ("VEVENT", "VTODO"): when the server says nothing, it does.
    pub fn holds(&self, component: &str) -> bool {
        self.components.is_empty() || self.components.iter().any(|c| c.eq_ignore_ascii_case(component))
    }

    /// A calendar that takes both events and tasks: it shows in the agenda
    /// and among the task lists, and says so (`label`).
    pub fn takes_both(&self) -> bool {
        self.kind == Kind::Calendars && self.holds("VEVENT") && self.holds("VTODO")
    }
}

impl Collection {
    /// Its contacts or events, by file.
    pub fn items(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let mut items: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == self.kind.extension()))
            .collect();
        items.sort();
        items
    }

    /// Where sync keeps what it knows of it.
    pub fn state_path(&self) -> PathBuf {
        state_path(&self.account, self.kind, &self.id)
    }
}

/// Where sync keeps what it knows of a collection.
pub fn state_path(account: &str, kind: Kind, id: &str) -> PathBuf {
    state_dir().join("dav").join(account).join(kind.folder()).join(format!("{id}.toml"))
}

/// Every collection of a kind, from every account, by account then name.
pub fn collections(kind: Kind) -> Vec<Collection> {
    every_collection(kind).into_iter().filter(|c| !State::load(&c.state_path()).deleted).collect()
}

/// The mark of an account switched off, in its folder of each kind.
const OFF: &str = ".sioul-off";

/// An account's calendars and address books out of the pages (`off`), or back.
pub fn set_account_off(account: &str, off: bool) {
    for kind in [Kind::Contacts, Kind::Calendars] {
        let dir = kind.root().join(account);
        if off {
            if dir.is_dir() {
                let _ = std::fs::write(dir.join(OFF), "Switched off in Sioul's Accounts: these collections wait here.\n");
            }
        } else {
            let _ = std::fs::remove_file(dir.join(OFF));
        }
    }
}

/// The marks of switched-off accounts made to follow the configuration, on
/// this device: the mark is this device's own, while the configuration
/// travels; an account switched on again elsewhere (or here before, in a
/// window that died) kept its mark here, and its calendars and address
/// books, synced all the same, never showed (the phone of 21 October 2026).
/// `on`: the accounts switched on; `off`: those switched off. Whether any mark changed.
pub fn follow_switches(on: &[String], off: &[String]) -> bool {
    let mut changed = false;
    for kind in [Kind::Contacts, Kind::Calendars] {
        for (accounts, wanted) in [(on, false), (off, true)] {
            for account in accounts.iter().filter(|a| one_folder(a)) {
                let dir = kind.root().join(account);
                let marked = dir.join(OFF).exists();
                if marked != wanted && (wanted || marked) && dir.is_dir() {
                    set_account_off(account, wanted);
                    changed = true;
                }
            }
        }
    }
    changed
}

/// Items being moved from one collection to another (an event to another
/// calendar: `move_item`), each `from` → `to` written before the move and
/// taken out after it, so that a move cut short (a crash, Android stopping
/// Sioul) is finished at the next look (`finish_moves`): never twice, never lost.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Moves {
    #[serde(default, rename = "move")]
    moves: Vec<Move>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Move {
    from: PathBuf,
    to: PathBuf,
    /// Taken out with `from` once `to` is there (an item made another: its
    /// task's time blocks, `replace_items`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    also: Vec<PathBuf>,
    /// Written before `to`, taken back when `to` never came (a task's time
    /// block, made with the task an appointment becomes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    made: Vec<PathBuf>,
}

fn moves_path() -> PathBuf {
    crate::config::state_dir().join("dav").join("moves.toml")
}

fn moves_load() -> Moves {
    std::fs::read_to_string(moves_path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

fn moves_save(moves: &Moves) -> Result<(), String> {
    let path = moves_path();
    if moves.moves.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    crate::filelock::replace(&path, toml::to_string(moves).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
thread_local! {
    /// Tests only: the step after which a move stops, as a crash would (1: the
    /// move noted, 2: what goes with it made, 3: written there, 4: taken out
    /// here). Never in a build.
    static CRASH_AFTER: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// Whether a move stops here, as a crash would (tests only).
fn crashed(step: u8) -> bool {
    #[cfg(test)]
    if CRASH_AFTER.with(std::cell::Cell::get) == step {
        return true;
    }
    let _ = step;
    false
}

/// An item moved to another collection's folder as `to` (21 October 2026):
/// noted first, then written there, then taken out here, then the note
/// gone; a step cut short is finished by `finish_moves`. Its sync sends the
/// new item and the deletion, whatever their accounts (this device's own
/// calendar to a server's: the event reaches your other devices).
pub fn move_item(from: &Path, to: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(from).map_err(|e| e.to_string())?;
    replace_items(&[from.to_path_buf()], &[], to, &text)
}

/// Items made another (a task made an appointment, an appointment a task,
/// 21 October 2026): `gone` (the first, then what goes with it: a task's
/// time blocks) replaced by `to`, written as `text`, with `made` written
/// just before it (a time block for the task an event becomes). The same
/// steps as a move: noted, `made` then `to` written, `gone` taken out, the
/// note gone. Cut short before `to` is whole, `made` is taken back and
/// `gone` kept; after, `gone` is taken out (`finish_moves`): never both,
/// never neither.
pub fn replace_items(gone: &[PathBuf], made: &[(PathBuf, String)], to: &Path, text: &str) -> Result<(), String> {
    let Some((from, also)) = gone.split_first() else { return Err("nothing to replace".into()) };
    // One move at a time in this program: a sync finishing moves meanwhile
    // never takes back what one under way has made (22 October 2026).
    let _moving = MOVING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    finish_moves_now();
    let mut moves = moves_load();
    moves.moves.push(Move { from: from.clone(), to: to.to_path_buf(), also: also.to_vec(), made: made.iter().map(|(p, _)| p.clone()).collect() });
    moves_save(&moves)?;
    if crashed(1) {
        return Ok(());
    }
    for (path, text) in made {
        write_item(path, text)?;
    }
    if crashed(2) {
        return Ok(());
    }
    write_item(to, text)?;
    if crashed(3) {
        return Ok(());
    }
    std::fs::remove_file(from).map_err(|e| e.to_string())?;
    for path in also {
        let _ = std::fs::remove_file(path);
    }
    if crashed(4) {
        return Ok(());
    }
    finish_moves_now();
    Ok(())
}

/// Held while a move runs or moves are finished (`replace_items`, `finish_moves`).
static MOVING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Every move cut short, finished: the item there and still here, taken out
/// here (written whole before the note said so), with what goes with it; not
/// there, the move forgotten, the item kept here and what was made for it
/// taken back; gone from here, the move done. Called before each move and
/// each sync of contacts and calendars.
pub fn finish_moves() {
    let _moving = MOVING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    finish_moves_now();
}

/// `finish_moves`, `MOVING` held.
fn finish_moves_now() {
    let moves = moves_load();
    if moves.moves.is_empty() {
        return;
    }
    for m in &moves.moves {
        if m.to.exists() {
            for path in std::iter::once(&m.from).chain(&m.also) {
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                }
            }
        } else {
            for path in &m.made {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let _ = moves_save(&Moves::default());
}

/// Every collection kept here, those deleted and not yet gone from the server included.
pub fn every_collection(kind: Kind) -> Vec<Collection> {
    let mut found = Vec::new();
    let Ok(accounts) = std::fs::read_dir(kind.root()) else { return found };
    // An account switched off keeps its folders, out of the pages (`set_account_off`).
    for account in accounts.filter_map(Result::ok).filter(|e| e.path().is_dir() && !e.path().join(OFF).exists()) {
        let account_id = account.file_name().to_string_lossy().to_string();
        let Ok(dirs) = std::fs::read_dir(account.path()) else { continue };
        for dir in dirs.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()) {
            let id = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
            let mut state = State::load(&state_path(&account_id, kind, &id));
            if kind == Kind::Calendars && account_id == LOCAL && state.components.is_empty() {
                settle_kinds(&dir, &mut state, &state_path(&account_id, kind, &id));
            }
            found.push(Collection {
                kind,
                account: account_id.clone(),
                name: read("displayname").unwrap_or_else(|| id.clone()),
                color: read("color"),
                read_only: state.read_only,
                components: state.components.clone(),
                id,
                dir,
            });
        }
    }
    found.sort_by(|a, b| (&a.account, a.name.to_lowercase()).cmp(&(&b.account, b.name.to_lowercase())));
    found
}

/// A calendar of this device's own whose kinds were never written (made
/// before 21 October 2026 without them, when "unsaid" meant both): what it
/// holds says them, written once with it (its state, `dav/local/`): events
/// alone, tasks alone, or both when it holds both or nothing. A server's says
/// its own at each sync (`supported-calendar-component-set`; unsaid, both, as
/// RFC 4791 §5.2.3 has it).
fn settle_kinds(dir: &Path, state: &mut State, path: &Path) {
    let (mut events, mut tasks) = (false, false);
    for item in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "ics")) {
        let text = std::fs::read_to_string(&item).unwrap_or_default().to_ascii_uppercase();
        events |= text.contains("BEGIN:VEVENT");
        tasks |= text.contains("BEGIN:VTODO");
    }
    // Said meanwhile (a collection being made): what was said stays.
    let now = State::load(path);
    if !now.components.is_empty() {
        *state = now;
        return;
    }
    state.components = match (events, tasks) {
        (true, false) => vec!["VEVENT".into()],
        (false, true) => vec!["VTODO".into()],
        _ => vec!["VEVENT".into(), "VTODO".into()],
    };
    let _ = state.save(path);
}

/// A collection renamed here; the next sync tells the server.
pub fn rename_collection(collection: &Collection, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(format!("{}: a name is needed", collection.name));
    }
    if collection.read_only {
        return Err(format!("{}: read only", collection.name));
    }
    crate::filelock::replace(&collection.dir.join("displayname"), name.as_bytes()).map_err(|e| e.to_string())?;
    let path = collection.state_path();
    let mut state = State::load(&path);
    if collection.account != LOCAL && !state.pending {
        state.renamed = true;
        state.save(&path)?;
    }
    Ok(())
}

/// The items a collection holds here: events, tasks or contacts.
pub fn holds_items(collection: &Collection) -> bool {
    std::fs::read_dir(&collection.dir).into_iter().flatten().filter_map(Result::ok).any(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        name.ends_with(".ics") || name.ends_with(".vcf")
    })
}

/// An empty collection deleted: at once when it was never sent, else hidden
/// until the next sync deletes it on the server. One holding anything stays.
pub fn delete_collection(collection: &Collection) -> Result<(), String> {
    if holds_items(collection) {
        return Err(format!("{}: not empty", collection.name));
    }
    if collection.read_only {
        return Err(format!("{}: read only", collection.name));
    }
    let path = collection.state_path();
    let mut state = State::load(&path);
    if collection.account == LOCAL || state.pending {
        let _ = std::fs::remove_dir_all(&collection.dir);
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    state.deleted = true;
    state.save(&path)
}

/// Whether a name (an account's, a collection's) is one folder of its own:
/// not ".", "..", nor a path ("../x", "C:"), which would put files elsewhere.
fn one_folder(name: &str) -> bool {
    !matches!(name, "" | "." | "..") && !name.contains(['/', '\\', '\0']) && !(cfg!(windows) && name.contains(':'))
}

/// A collection's folder, made if needed, with its name and colour written for other tools.
pub fn prepare(kind: Kind, account: &str, id: &str, name: &str, color: Option<&str>) -> std::io::Result<PathBuf> {
    if !one_folder(account) || !one_folder(id) {
        return Err(std::io::Error::other(format!("{account}/{id}: not a folder's name")));
    }
    let dir = kind.root().join(account).join(id);
    std::fs::create_dir_all(&dir)?;
    // Written beside, then renamed: the sharing (local lists) never reads them empty.
    crate::filelock::replace(&dir.join("displayname"), name.as_bytes())?;
    match color {
        Some(color) => crate::filelock::replace(&dir.join("color"), color.as_bytes())?,
        None => {
            let _ = std::fs::remove_file(dir.join("color"));
        }
    }
    Ok(dir)
}

/// The account of collections kept on this computer only.
pub const LOCAL: &str = "local";

/// A new collection made here: its folder, its name and colour, and a state
/// that asks the next sync to create it on the server (MKCALENDAR or an
/// extended MKCOL). `components` says what a calendar holds ("VTODO" for a
/// task list). Returns it; an id already taken gets a number.
pub fn create(kind: Kind, account: &str, name: &str, color: Option<&str>, components: &[&str]) -> Result<Collection, String> {
    // The account comes from the window or an agent (the MCP): never a path.
    if !one_folder(account) {
        return Err(format!("{account}: not an account"));
    }
    let base = crate::config::slug(name);
    let base = if base.is_empty() { "list".to_string() } else { base };
    let taken = |id: &str| kind.root().join(account).join(id).exists() || state_path(account, kind, id).exists();
    let id = std::iter::once(base.clone()).chain((2..100).map(|n| format!("{base}-{n}"))).find(|id| !taken(id)).ok_or_else(|| format!("{name}: no free name"))?;
    // "local": kept here only, no server to create it on. Its state first,
    // what it takes with it: a folder never shows without them, which a
    // look meanwhile would settle as both (`settle_kinds`; 22 October 2026).
    let state = State { pending: account != LOCAL, components: components.iter().map(|c| c.to_string()).collect(), ..State::default() };
    state.save(&state_path(account, kind, &id))?;
    let dir = prepare(kind, account, &id, name, color).map_err(|e| e.to_string())?;
    Ok(Collection { kind, account: account.to_string(), id, dir, name: name.to_string(), color: color.map(str::to_string), read_only: false, components: state.components })
}

/// "personal", "contacts", "Calendrier%20perso" → a folder name that holds on
/// every system: letters, digits, `-` and `_`, the rest as `_`.
pub fn folder_id(href: &str) -> String {
    let last = href.trim_end_matches('/').rsplit('/').next().unwrap_or("collection");
    let id: String = last.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if id.is_empty() { "collection".into() } else { id }
}

/// What sync knows of a collection: where it is, how far it got, and each item.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    /// The collection's address on the server.
    #[serde(default)]
    pub url: String,
    /// RFC 6578: what the server gave at the last sync, to ask what changed since.
    #[serde(default)]
    pub sync_token: Option<String>,
    /// The collection's own tag (getctag): unchanged, nothing changed.
    #[serde(default)]
    pub ctag: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    /// For calendars: what the server lets it hold ("VEVENT", "VTODO"); empty when it does not say.
    #[serde(default)]
    pub components: Vec<String>,
    /// Made here: the next sync creates it on the server, then sends its items.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pending: bool,
    /// Renamed here: the next sync gives the server its new name.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub renamed: bool,
    /// Deleted here, empty: hidden, and the next sync deletes it on the
    /// server, unless something was put in it there meanwhile.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    #[serde(default, rename = "item")]
    pub items: Vec<ItemState>,
}

/// One item, as last agreed with the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemState {
    /// Its address on the server, as the server writes it.
    pub href: String,
    /// Its file name in the collection's folder.
    pub file: String,
    pub etag: String,
    /// A hash of the file as last synced: a different one means it was changed here.
    pub hash: String,
    /// The file's size and modification time (nanoseconds since 1970) when
    /// last found as synced: both the same, it is unchanged, and not read.
    /// A time of 0 is unknown: the file is read and its hash compared.
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub mtime: i64,
}

impl State {
    pub fn load(path: &Path) -> State {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written next to its place, then moved: an interruption never leaves half a state.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        // A name of its own beside it: two writers at once never share one (22 October 2026).
        crate::filelock::replace(path, text.as_bytes()).map_err(fail)
    }
}

/// A short hash of a file's content, to tell whether it changed since sync.
pub fn content_hash(bytes: &[u8]) -> String {
    // FNV-1a, 64 bits: stable across versions of Rust, unlike `DefaultHasher`.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Writes an item next to its place, then moves it in.
pub fn write_item(path: &Path, text: &str) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, text).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)
}

/// A new item's name: unique, and safe as a file name and in a URL.
pub fn new_name() -> String {
    use std::hash::{Hash, Hasher};
    // Two names asked in the same instant (an import; a clock that ticks by the
    // microsecond, as on macOS) still differ: a file would be written over.
    static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    now.hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed).hash(&mut hasher);
    format!("sioul-{:x}-{:016x}", now.as_secs(), hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An event moved to another calendar (21 October 2026), cut short after
    /// each step as a crash would cut it: the next look finishes it, never
    /// twice, never lost.
    #[test]
    fn a_move_cut_at_each_step_ends_once() {
        let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:moved-1\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        for step in 0..=4u8 {
            let account = format!("moves-{step}");
            let from = prepare(Kind::Calendars, LOCAL, &format!("from-{step}"), "From", None).unwrap();
            let to = prepare(Kind::Calendars, &account, "to", "To", None).unwrap();
            let (source, target) = (from.join("e.ics"), to.join("e.ics"));
            std::fs::write(&source, text).unwrap();
            CRASH_AFTER.with(|c| c.set(step));
            move_item(&source, &target).unwrap();
            CRASH_AFTER.with(|c| c.set(0));
            finish_moves();
            let (here, there) = (source.exists(), target.exists());
            match step {
                // Noted only: kept here, nothing there.
                1 | 2 => assert!(here && !there, "step {step}"),
                // Written there, then the crash: taken out here at the next look.
                _ => assert!(!here && there && std::fs::read_to_string(&target).unwrap() == text, "step {step}"),
            }
            assert!(here != there, "step {step}: never twice, never lost");
            assert!(!moves_path().exists(), "step {step}: nothing left to finish");
            let _ = std::fs::remove_dir_all(&from);
            let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(&account));
        }
    }

    /// A task made an appointment and back (21 October 2026), cut short
    /// after each step: the task and its block, or the event and what was
    /// made with it, one side whole, never both, never neither.
    #[test]
    fn an_item_made_another_cut_at_each_step_ends_once() {
        for step in 0..=4u8 {
            let list = prepare(Kind::Calendars, LOCAL, &format!("kinds-list-{step}"), "List", None).unwrap();
            let calendar = prepare(Kind::Calendars, LOCAL, &format!("kinds-calendar-{step}"), "Calendar", None).unwrap();
            let (task, block, made, event) = (list.join("t.ics"), calendar.join("b.ics"), calendar.join("m.ics"), calendar.join("e.ics"));
            std::fs::write(&task, "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:t\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
            std::fs::write(&block, "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:b\r\nX-SIOUL-TASK:t\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
            let text = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:e\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
            CRASH_AFTER.with(|c| c.set(step));
            replace_items(&[task.clone(), block.clone()], &[(made.clone(), "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".into())], &event, text).unwrap();
            CRASH_AFTER.with(|c| c.set(0));
            finish_moves();
            let before = task.exists() && block.exists() && !made.exists() && !event.exists();
            let after = !task.exists() && !block.exists() && made.exists() && std::fs::read_to_string(&event).ok().as_deref() == Some(text);
            match step {
                1 | 2 => assert!(before, "step {step}: the task kept, what was made taken back"),
                _ => assert!(after, "step {step}: the event and its block, the task gone"),
            }
            assert!(!moves_path().exists(), "step {step}: nothing left to finish");
            let _ = std::fs::remove_dir_all(&list);
            let _ = std::fs::remove_dir_all(&calendar);
        }
        assert!(replace_items(&[], &[], Path::new("nowhere.ics"), "").is_err());
    }

    /// What a calendar takes, events, tasks or both (21 October 2026): a
    /// server's says it (`supported-calendar-component-set`; unsaid, both);
    /// one of this device's own made before without it is settled once from
    /// what it holds, and kept with it. One that takes both says so.
    #[test]
    fn a_calendar_says_what_it_takes() {
        let tr = crate::i18n::Translator::new("en");
        let config = crate::config::Config::default();
        let made = |id: &str, item: Option<&str>| {
            let dir = prepare(Kind::Calendars, LOCAL, id, id, None).unwrap();
            if let Some(item) = item {
                std::fs::write(dir.join("x.ics"), format!("BEGIN:VCALENDAR\r\nBEGIN:{item}\r\nUID:x\r\nEND:{item}\r\nEND:VCALENDAR\r\n")).unwrap();
            }
            dir
        };
        let dirs = [made("kinds-old-events", Some("VEVENT")), made("kinds-old-tasks", Some("VTODO")), made("kinds-old-empty", None)];
        let find = |account: &str, id: &str| collections(Kind::Calendars).into_iter().find(|c| c.account == account && c.id == id).unwrap();
        assert_eq!(find(LOCAL, "kinds-old-events").components, ["VEVENT"]);
        assert_eq!(find(LOCAL, "kinds-old-tasks").components, ["VTODO"]);
        let empty = find(LOCAL, "kinds-old-empty");
        assert_eq!(empty.components, ["VEVENT", "VTODO"]);
        assert_eq!(State::load(&state_path(LOCAL, Kind::Calendars, "kinds-old-tasks")).components, ["VTODO"], "kept with it");
        assert!(empty.takes_both() && empty.label(&config, &tr).ends_with("events and tasks"));
        assert!(!find(LOCAL, "kinds-old-tasks").label(&config, &tr).contains("events and tasks"));
        // A server's says its own: unsaid, both, and nothing is written for it here.
        prepare(Kind::Calendars, "kinds-server", "personnel", "Personnel", None).unwrap();
        let unsaid = find("kinds-server", "personnel");
        assert!(unsaid.components.is_empty() && unsaid.holds("VEVENT") && unsaid.holds("VTODO") && unsaid.takes_both());
        let path = state_path("kinds-server", Kind::Calendars, "personnel");
        State { components: vec!["VEVENT".into()], ..State::default() }.save(&path).unwrap();
        let events = find("kinds-server", "personnel");
        assert!(events.holds("VEVENT") && !events.holds("VTODO") && !events.takes_both());
        for dir in dirs {
            let _ = std::fs::remove_dir_all(dir);
        }
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join("kinds-server"));
    }

    /// A calendar made while another thread looks at the calendars (a sync,
    /// the window) keeps what it takes: its folder showed before its state,
    /// and the look settled it as both (22 October 2026, found by the
    /// choosers' test run beside others).
    #[test]
    fn a_calendar_made_while_others_look_keeps_what_it_takes() {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let looking = {
            let stop = std::sync::Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = collections(Kind::Calendars);
                }
            })
        };
        let made: Vec<Collection> = (0..40).map(|n| create(Kind::Calendars, LOCAL, &format!("Raced events {n}"), None, &["VEVENT"]).unwrap()).collect();
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        looking.join().unwrap();
        for c in &made {
            assert_eq!(State::load(&c.state_path()).components, ["VEVENT"], "{}", c.id);
            let _ = std::fs::remove_dir_all(&c.dir);
            let _ = std::fs::remove_file(c.state_path());
        }
    }

    /// The phone of 21 October 2026: an account switched off once, then on
    /// again on another device, kept its mark here, and its calendars,
    /// synced all the same, never showed. The marks follow the configuration.
    #[test]
    fn an_account_on_again_elsewhere_shows_its_calendars_here() {
        let account = "marked-elsewhere";
        prepare(Kind::Calendars, account, "work", "Work", None).unwrap();
        prepare(Kind::Calendars, account, "family", "Family", None).unwrap();
        let shown = || collections(Kind::Calendars).into_iter().filter(|c| c.account == account).count();
        assert_eq!(shown(), 2);
        set_account_off(account, true);
        assert_eq!(shown(), 0, "off here");
        // Switched on again on another device: the configuration says on, the mark stays.
        assert!(follow_switches(&[account.to_string()], &[]), "the mark follows");
        assert_eq!(shown(), 2, "every calendar shown again");
        assert!(!follow_switches(&[account.to_string()], &[]), "nothing more to change");
        // Switched off elsewhere: its folders leave the pages here too.
        assert!(follow_switches(&[], &[account.to_string()]));
        assert_eq!(shown(), 0);
        set_account_off(account, false);
    }

    #[test]
    fn folder_names_hold_everywhere() {
        assert_eq!(folder_id("/remote.php/dav/calendars/jane/personal/"), "personal");
        assert_eq!(folder_id("/dav/Calendrier%20perso/"), "Calendrier_20perso");
        assert_eq!(folder_id("/"), "collection");
        assert_eq!(content_hash(b"BEGIN:VCARD"), content_hash(b"BEGIN:VCARD"));
        assert_ne!(content_hash(b"a"), content_hash(b"b"));
        // An account or a collection is one folder, never a path out of the data folders.
        assert!(one_folder("local") && one_folder("murmur-2") && !one_folder("..") && !one_folder("../x") && !one_folder("a\\b") && !one_folder(""));
        assert!(create(Kind::Calendars, "../elsewhere", "List", None, &["VTODO"]).is_err());
        let names: std::collections::BTreeSet<String> = (0..1000).map(|_| new_name()).collect();
        assert_eq!(names.len(), 1000, "never the same name twice");
    }
}
