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
    // Read under the journal's lock, never before it: a sync writing it in
    // between is moved as it wrote it (the review of 22 October 2026, F8).
    crate::filelock::with_lock(&moves_path(), || {
        let text = std::fs::read_to_string(from).map_err(|e| e.to_string())?;
        // An event's: through the one rule (`agenda::guard`), as it is now, before anything is noted.
        guarded(from, crate::agenda::Change::Move)?;
        guarded(to, crate::agenda::Change::Write(&text))?;
        replace_locked(from, &[], &[], to, &text)
    })
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
    // Each event among them through the one rule (`agenda::guard`) before anything is noted:
    // refused, nothing is done (a meeting with others never taken out, a calendar you only read never written).
    for path in gone.iter().filter(|p| p.as_path() != to) {
        guarded(path, crate::agenda::Change::Delete)?;
    }
    for (path, made) in made {
        guarded(path, crate::agenda::Change::Write(made))?;
    }
    guarded(to, crate::agenda::Change::Write(text))?;
    // One move at a time, in this program and between programs (the window,
    // `sioul`, a sync): another finishing moves meanwhile never takes back
    // what one under way has made, nor clears its note (22 October 2026; the
    // Windows run of `an_item_made_another_cut_at_each_step_ends_once`,
    // whose journal another test finished in between).
    crate::filelock::with_lock(&moves_path(), || replace_locked(from, also, made, to, text))
}

/// `replace_items`, the journal's lock held.
fn replace_locked(from: &Path, also: &[PathBuf], made: &[(PathBuf, String)], to: &Path, text: &str) -> Result<(), String> {
    finish_moves_now();
    let mut moves = moves_load();
    moves.moves.push(Move { from: from.to_path_buf(), to: to.to_path_buf(), also: also.to_vec(), made: made.iter().map(|(p, _)| p.clone()).collect() });
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
    // The old one taken out, unless the new one took its place: the same
    // collection, the same name (a plan's task become its event, the review
    // of 22 October 2026, F1: both were lost).
    if from != to {
        std::fs::remove_file(from).map_err(|e| e.to_string())?;
        counted(from);
    }
    for path in also.iter().filter(|p| p.as_path() != to) {
        let _ = std::fs::remove_file(path);
        counted(path);
    }
    if crashed(4) {
        return Ok(());
    }
    finish_moves_now();
    Ok(())
}

/// Every move cut short, finished: the item there and still here, taken out
/// here (written whole before the note said so), with what goes with it; not
/// there, the move forgotten, the item kept here and what was made for it
/// taken back; gone from here, the move done. Called before each move and
/// each sync of contacts and calendars.
pub fn finish_moves() {
    crate::filelock::with_lock(&moves_path(), finish_moves_now);
}

/// `finish_moves`, the journal's lock held.
fn finish_moves_now() {
    let moves = moves_load();
    if moves.moves.is_empty() {
        return;
    }
    for m in &moves.moves {
        if m.to.exists() {
            // Never the item made, when it took the old one's place (one collection, one name: F1).
            for path in std::iter::once(&m.from).chain(&m.also).filter(|p| **p != m.to) {
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                }
                // Its deletion on its server waits for the new item to be there (F6).
                note_moved(path, &m.to);
            }
        } else {
            for path in &m.made {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let _ = moves_save(&Moves::default());
}

/// Items moved or made another here, whose old copy's deletion on its
/// server waits until the new one is there (the review of 22 October 2026,
/// F6: a refused PUT after the DELETE lost the event on every other device).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Moved {
    #[serde(default, rename = "moved")]
    moved: Vec<MovedItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MovedItem {
    old: PathBuf,
    new: PathBuf,
}

fn moved_path() -> PathBuf {
    crate::config::state_dir().join("dav").join("moved.toml")
}

fn moved_change<T>(f: impl FnOnce(&mut Moved) -> T) -> T {
    let path = moved_path();
    crate::filelock::with_lock(&path, || {
        let mut moved: Moved = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        let before = moved.moved.clone();
        let out = f(&mut moved);
        if moved.moved != before {
            if moved.moved.is_empty() {
                let _ = std::fs::remove_file(&path);
            } else if let Ok(text) = toml::to_string(&moved) {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = crate::filelock::replace(&path, text.as_bytes());
            }
        }
        out
    })
}

fn note_moved(old: &Path, new: &Path) {
    moved_change(|m| {
        m.moved.retain(|i| i.old != old);
        m.moved.push(MovedItem { old: old.to_path_buf(), new: new.to_path_buf() });
    });
}

/// The new item an old one's deletion on its server waits for, while the
/// new one is not there yet (`dav`: its PUT first, then the DELETE): None
/// once it is, or when nothing waits.
pub fn deletion_waits_for(old: &Path) -> Option<PathBuf> {
    let new = moved_change(|m| m.moved.iter().find(|i| i.old == old).map(|i| i.new.clone()))?;
    (!new_is_there(old, &new)).then_some(new)
}

/// Whether the new item of a move is on its server, or needs not be: gone
/// since, kept on this device alone, or in the old one's own collection.
fn new_is_there(old: &Path, new: &Path) -> bool {
    if !new.exists() || new.parent() == old.parent() {
        return true;
    }
    let found = [Kind::Calendars, Kind::Contacts].into_iter().find_map(|kind| {
        let rest = new.strip_prefix(kind.root()).ok()?;
        let mut parts = rest.components().map(|c| c.as_os_str().to_string_lossy().to_string());
        Some((kind, parts.next()?, parts.next()?, parts.next()?))
    });
    let Some((kind, account, id, file)) = found else { return true };
    if account == LOCAL {
        return true;
    }
    State::load(&state_path(&account, kind, &id)).items.iter().any(|i| i.file == file && (!i.etag.is_empty() || !i.hash.is_empty()))
}

/// The old copy's deletion done on its server: nothing waits for it any more.
pub fn deletion_done(old: &Path) {
    moved_change(|m| m.moved.retain(|i| i.old != old));
}

/// Whether `new` is an item moved or made here whose old copy may still be
/// on its server: never replaced by what the server holds under its UID.
pub fn moved_here(new: &Path) -> bool {
    moved_change(|m| m.moved.iter().any(|i| i.new == new))
}

/// The folders of the collections of accounts switched off (`set_account_off`):
/// out of the pages, their items still here.
pub fn switched_off_dirs(kind: Kind) -> Vec<PathBuf> {
    let Ok(accounts) = std::fs::read_dir(kind.root()) else { return Vec::new() };
    accounts
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join(OFF).exists())
        .flat_map(|account| std::fs::read_dir(account).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_dir()))
        .collect()
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
            // What it holds counts too, at each read: a task that came into a
            // calendar said to take events alone (another device's, an older
            // Sioul's, an import) is never hidden (the review of 22 October 2026, F3).
            if kind == Kind::Calendars && !state.components.is_empty() {
                for held in kinds_held(&dir) {
                    if !state.components.iter().any(|c| c.eq_ignore_ascii_case(held)) {
                        state.components.push(held.to_string());
                    }
                }
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

/// The kinds a calendar's folder holds ("VEVENT", "VTODO"), read again only
/// when its list of files changed (its time and count), else as last read.
fn kinds_held(dir: &Path) -> Vec<&'static str> {
    static SEEN: std::sync::Mutex<Option<std::collections::HashMap<PathBuf, (Option<std::time::SystemTime>, usize, Vec<&'static str>)>>> = std::sync::Mutex::new(None);
    let items: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "ics")).collect();
    let time = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    let mut seen = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let known = seen.get_or_insert_with(Default::default);
    if let Some((t, n, held)) = known.get(dir)
        && *t == time
        && *n == items.len()
    {
        return held.clone();
    }
    let (mut events, mut tasks) = (false, false);
    for item in &items {
        let text = std::fs::read_to_string(item).unwrap_or_default().to_ascii_uppercase();
        events |= text.contains("BEGIN:VEVENT");
        tasks |= text.contains("BEGIN:VTODO");
    }
    let held: Vec<&'static str> = [(events, "VEVENT"), (tasks, "VTODO")].into_iter().filter(|(h, _)| *h).map(|(_, k)| k).collect();
    known.insert(dir.to_path_buf(), (time, items.len(), held.clone()));
    held
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
    // Written beside, then renamed: the sharing (local lists) never reads them
    // empty. Only when they differ: each sync prepares every collection, and a
    // name written again would change its folder's time, which says a calendar
    // changed (`calendars_stamp`: the plan as last made would read as older
    // after every sync; review of 85da7f0, later 5).
    let same = |file: &str, value: &str| std::fs::read(dir.join(file)).is_ok_and(|bytes| bytes == value.as_bytes());
    if !same("displayname", name) {
        crate::filelock::replace(&dir.join("displayname"), name.as_bytes())?;
    }
    match color {
        Some(color) if !same("color", color) => crate::filelock::replace(&dir.join("color"), color.as_bytes())?,
        Some(_) => {}
        None if dir.join("color").exists() => {
            let _ = std::fs::remove_file(dir.join("color"));
        }
        None => {}
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
    /// The form its tasks and events were rewritten in, once
    /// (`interop::FORMAT`, `interop::migrate_folder`); 0 before.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub interop: u32,
    /// When the last pull that went through whole began (Unix seconds): what
    /// follows an appointment set for an instant this device never saw waits
    /// for one begun after (`moves`). This device's own, as the rest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulled: Option<i64>,
    #[serde(default, rename = "item")]
    pub items: Vec<ItemState>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
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

/// Writes an item next to its place, then moves it in. An event's file goes
/// through the one rule of every event's writer first (`agenda::guard`, on
/// the file as it is now): never into a calendar you only read, never a
/// meeting with others changed but for Sioul's own lines; anything else (a
/// task, a contact) as it comes.
pub fn write_item(path: &Path, text: &str) -> Result<(), String> {
    guarded(path, crate::agenda::Change::Write(text))?;
    store_item(path, text)
}

/// An item taken out of its collection; an event's through the one rule
/// first (`agenda::guard`): never one of a calendar you only read, never a
/// meeting with others (its server would cancel or decline it for them).
pub fn remove_item(path: &Path) -> Result<(), String> {
    guarded(path, crate::agenda::Change::Delete)?;
    std::fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    counted(path);
    Ok(())
}

/// A write of a task's or an event's file counted (`timeline::written`):
/// the agenda's layouts made before it are older.
fn counted(path: &Path) {
    if path.starts_with(Kind::Calendars.root()) {
        crate::timeline::note_written();
    }
}

/// `agenda::guard` for a file of a collection, its refusal said.
fn guarded(path: &Path, change: crate::agenda::Change) -> Result<(), String> {
    crate::agenda::guard(path, change).map_err(|refusal| crate::agenda::said(refusal, &change))
}

/// Whether the calendar whose folder holds `path` can only be read; None
/// when `path` is no file of a calendar's folder.
pub fn calendar_read_only(path: &Path) -> Option<bool> {
    calendar_marks(path).map(|(read_only, _)| read_only)
}

/// What the calendar whose folder holds `path` may hold ("VEVENT", "VTODO";
/// empty when its server does not say, or a calendar of this device's own
/// made without saying); None when `path` is no file of a calendar's folder.
pub fn calendar_components(path: &Path) -> Option<Vec<String>> {
    calendar_marks(path).map(|(_, components)| components)
}

fn calendar_marks(path: &Path) -> Option<(bool, Vec<String>)> {
    let rest = path.strip_prefix(Kind::Calendars.root()).ok()?;
    let parts: Vec<String> = rest.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    let [account, id, _file] = parts.as_slice() else { return None };
    Some(marks_of(&state_path(account, Kind::Calendars, id)))
}

/// A calendar's read-only mark and what it may hold, from its sync state
/// (`State::read_only`, `components`), parsed again only when that file
/// changed (its time or its size), never at each write: the state holds a
/// row per item its server has (review of 36002ea, can-wait 13).
fn marks_of(state: &Path) -> (bool, Vec<String>) {
    type Seen = std::collections::BTreeMap<PathBuf, (Option<std::time::SystemTime>, u64, bool, Vec<String>)>;
    static SEEN: std::sync::Mutex<Seen> = std::sync::Mutex::new(std::collections::BTreeMap::new());
    let meta = std::fs::metadata(state).ok();
    let (when, size) = (meta.as_ref().and_then(|m| m.modified().ok()), meta.as_ref().map_or(0, std::fs::Metadata::len));
    let mut seen = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((at, length, mark, components)) = seen.get(state)
        && (*at, *length) == (when, size)
    {
        return (*mark, components.clone());
    }
    // A state that does not read now is never kept for its stamp (the check
    // of ef8c0ca, low 21): the marks last read meanwhile, read again next time.
    match std::fs::read_to_string(state).ok().and_then(|text| toml::from_str::<State>(&text).ok()) {
        Some(read) => {
            seen.insert(state.to_path_buf(), (when, size, read.read_only, read.components.clone()));
            (read.read_only, read.components)
        }
        // No state at all: a calendar of this device's own, which no server makes read-only.
        None if meta.is_none() => {
            seen.insert(state.to_path_buf(), (when, size, false, Vec::new()));
            (false, Vec::new())
        }
        None => seen.get(state).map_or((false, Vec::new()), |(_, _, mark, components)| (*mark, components.clone())),
    }
}

/// An item written as it came, without `agenda::guard`: what a server sent
/// (the sync's copy of it), an invitation's event as your answer keeps it
/// (`pim::answer`, the one path that writes a meeting with others), and
/// `write_item` once the rule let it through. Next to its place, then moved in.
pub fn store_item(path: &Path, text: &str) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, text).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)?;
    counted(path);
    Ok(())
}

/// What the calendars' folders say of their last change, as one number:
/// each account's and each collection's folder's time. A file made, replaced
/// or taken out there changes it, whichever program did it (this window,
/// `sioul`, `sioul mcp`, the sync, another application), since Sioul writes
/// each item next to its place and moves it in (`store_item`). A layout made
/// before such a change is older than it (`timeline::Stamp`).
pub fn calendars_stamp() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let time = |path: &Path| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let sorted = |dir: &Path| {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        entries.sort();
        entries
    };
    let root = Kind::Calendars.root();
    time(&root).hash(&mut hasher);
    for account in sorted(&root) {
        account.hash(&mut hasher);
        time(&account).hash(&mut hasher);
        for collection in sorted(&account) {
            collection.hash(&mut hasher);
            time(&collection).hash(&mut hasher);
        }
    }
    hasher.finish()
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

    /// A collection prepared again as it is (each sync prepares every one)
    /// writes nothing: its folder's time, which says a calendar changed
    /// (`calendars_stamp`), stays; a new name or colour is written (review of
    /// 85da7f0, later 5).
    #[test]
    fn a_collection_prepared_as_it_is_writes_nothing() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
        let dir = prepare(Kind::Calendars, "prepared-again", "cal", "Name", Some("#123456")).unwrap();
        let time = || std::fs::metadata(&dir).and_then(|m| m.modified()).unwrap();
        let before = time();
        std::thread::sleep(std::time::Duration::from_millis(50));
        prepare(Kind::Calendars, "prepared-again", "cal", "Name", Some("#123456")).unwrap();
        assert_eq!(time(), before, "the same name and colour: nothing written");
        prepare(Kind::Calendars, "prepared-again", "cal", "Other name", Some("#123456")).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("displayname")).unwrap(), "Other name");
        assert_ne!(time(), before, "a new name: written, its folder's time with it");
        prepare(Kind::Calendars, "prepared-again", "cal", "Other name", None).unwrap();
        assert!(!dir.join("color").exists(), "no colour: none kept");
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join("prepared-again"));
    }

    /// An event moved to another calendar (21 October 2026), cut short after
    /// each step as a crash would cut it: the next look finishes it, never
    /// twice, never lost.
    #[test]
    fn a_move_cut_at_each_step_ends_once() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
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
            assert!(!noted(&source), "step {step}: nothing left to finish");
            let _ = std::fs::remove_dir_all(&from);
            let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(&account));
        }
    }

    /// Whether the journal still notes a move from `path` (another test's own may be under way).
    fn noted(path: &Path) -> bool {
        crate::filelock::with_lock(&moves_path(), || moves_load().moves.iter().any(|m| m.from == path))
    }

    /// A task made an appointment and back (21 October 2026), cut short
    /// after each step: the task and its block, or the event and what was
    /// made with it, one side whole, never both, never neither.
    #[test]
    fn an_item_made_another_cut_at_each_step_ends_once() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
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
            assert!(!noted(&task), "step {step}: nothing left to finish");
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
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
        let tr = crate::i18n::Translator::new("en");
        let config = crate::config::Config::default();
        // Each folder made whole beside, then put in place: a look from another
        // test meanwhile never sees it before its item (CI's Windows job of 22
        // October 2026 settled one while it was still empty).
        let made = |id: &str, item: Option<&str>| {
            let staged = Kind::Calendars.root().parent().unwrap().join(format!("staged-{id}"));
            let _ = std::fs::remove_dir_all(&staged);
            std::fs::create_dir_all(&staged).unwrap();
            std::fs::write(staged.join("displayname"), id).unwrap();
            if let Some(item) = item {
                std::fs::write(staged.join("x.ics"), format!("BEGIN:VCALENDAR\r\nBEGIN:{item}\r\nUID:x\r\nEND:{item}\r\nEND:VCALENDAR\r\n")).unwrap();
            }
            let dir = Kind::Calendars.root().join(LOCAL).join(id);
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::remove_file(state_path(LOCAL, Kind::Calendars, id));
            std::fs::create_dir_all(dir.parent().unwrap()).unwrap();
            std::fs::rename(&staged, &dir).unwrap();
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

    /// A task become an event in a calendar taking both, under the same
    /// name (a plan's key, 22 October 2026, F1): the event stays, whether the
    /// move ends or is cut short at any step.
    #[test]
    fn an_item_made_another_in_its_own_place_stays() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
        for step in 0..=4u8 {
            let id = format!("same-place-{step}");
            // Its kinds said first, both, as `create` says them: else another test's
            // look meanwhile settles them from what it holds, a task alone (`settle_kinds`).
            State { components: vec!["VEVENT".into(), "VTODO".into()], ..State::default() }.save(&state_path(LOCAL, Kind::Calendars, &id)).unwrap();
            let dir = prepare(Kind::Calendars, LOCAL, &id, "Both", None).unwrap();
            let path = dir.join("plan-x.ics");
            std::fs::write(&path, "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:plan-x\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
            let event = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:plan-x\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
            CRASH_AFTER.with(|c| c.set(step));
            replace_items(std::slice::from_ref(&path), &[], &path, event).unwrap();
            CRASH_AFTER.with(|c| c.set(0));
            finish_moves();
            let now = std::fs::read_to_string(&path).unwrap_or_default();
            match step {
                1 | 2 => assert!(now.contains("VTODO"), "step {step}: cut before the event: the task kept"),
                _ => assert_eq!(now, event, "step {step}: the event kept"),
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// Moves made side by side while another finishes moves in a loop (a
    /// sync, the window): each ends once, its block kept (22 October 2026: on
    /// Windows, another test's finishing took a block back between its
    /// writing and its event's).
    #[test]
    fn moves_side_by_side_each_end_once() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let finishing = {
            let stop = std::sync::Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    finish_moves();
                }
            })
        };
        let workers: Vec<_> = (0..4)
            .map(|n| {
                std::thread::spawn(move || {
                    let dir = prepare(Kind::Calendars, LOCAL, &format!("side-by-side-{n}"), "Side", None).unwrap();
                    for i in 0..25 {
                        let (task, block, event) = (dir.join(format!("t{i}.ics")), dir.join(format!("b{i}.ics")), dir.join(format!("e{i}.ics")));
                        std::fs::write(&task, "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:t\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
                        replace_items(std::slice::from_ref(&task), &[(block.clone(), "block".into())], &event, "event").unwrap();
                        assert!(!task.exists() && block.exists() && event.exists(), "worker {n}, move {i}");
                    }
                    let _ = std::fs::remove_dir_all(&dir);
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        finishing.join().unwrap();
    }

    /// A task that came into a calendar said to take events alone (another
    /// device's files arriving, an older Sioul, an import) is never hidden:
    /// what a folder holds counts at each read (the review of 22 October
    /// 2026, F3). Saved kinds are not changed by it.
    #[test]
    fn what_a_calendar_holds_always_shows() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
        for (account, id) in [(LOCAL, "held-events-local"), ("held-server", "held-events")] {
            let dir = prepare(Kind::Calendars, account, id, "Events", None).unwrap();
            State { components: vec!["VEVENT".into()], ..State::default() }.save(&state_path(account, Kind::Calendars, id)).unwrap();
            std::fs::write(dir.join("e.ics"), "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:e\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
            let find = || collections(Kind::Calendars).into_iter().find(|c| c.account == account && c.id == id).unwrap();
            assert!(!find().holds("VTODO"));
            std::fs::write(dir.join("t.ics"), "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:t\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
            assert!(find().holds("VTODO") && find().holds("VEVENT"), "{account}: the task that came shows");
            assert!(crate::tasks::lists().iter().any(|c| c.account == account && c.id == id), "{account}: among the task lists");
            assert_eq!(State::load(&state_path(account, Kind::Calendars, id)).components, ["VEVENT"], "what the server said stays said");
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::remove_file(state_path(account, Kind::Calendars, id));
        }
    }

    /// A calendar made while another thread looks at the calendars (a sync,
    /// the window) keeps what it takes: its folder showed before its state,
    /// and the look settled it as both (22 October 2026, found by the
    /// choosers' test run beside others).
    #[test]
    fn a_calendar_made_while_others_look_keeps_what_it_takes() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
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
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        crate::config::stand_ins_only();
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
