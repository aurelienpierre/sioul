// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Address books and calendars on disk, as vdir: one folder per collection,
//! one file per contact (`.vcf`) or per event (`.ics`), the collection's name
//! and colour in the files `displayname` and `color`. pimsync, vdirsyncer,
//! khal and khard read the same folders
//! (https://vdirsyncer.pimutils.org/en/stable/vdir.html).
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

    /// Its name and where it lives: "Agenda · example.org", "Maison · on this computer only".
    pub fn label(&self, config: &crate::config::Config, tr: &crate::i18n::Translator) -> String {
        let place = self.place(config);
        format!("{} · {}", self.name, if place.is_empty() { tr.text("collection-here", None) } else { place })
    }

    /// Whether it takes items of this kind ("VEVENT", "VTODO"): when the server says nothing, it does.
    pub fn holds(&self, component: &str) -> bool {
        self.components.is_empty() || self.components.iter().any(|c| c.eq_ignore_ascii_case(component))
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
            let state = State::load(&state_path(&account_id, kind, &id));
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

/// A collection renamed here; the next sync tells the server.
pub fn rename_collection(collection: &Collection, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(format!("{}: a name is needed", collection.name));
    }
    if collection.read_only {
        return Err(format!("{}: read only", collection.name));
    }
    std::fs::write(collection.dir.join("displayname"), name).map_err(|e| e.to_string())?;
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
    std::fs::write(dir.join("displayname"), name)?;
    match color {
        Some(color) => std::fs::write(dir.join("color"), color)?,
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
    let dir = prepare(kind, account, &id, name, color).map_err(|e| e.to_string())?;
    // "local": kept here only, no server to create it on.
    let state = State { pending: account != LOCAL, components: components.iter().map(|c| c.to_string()).collect(), ..State::default() };
    state.save(&state_path(account, kind, &id))?;
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
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, text).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
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
