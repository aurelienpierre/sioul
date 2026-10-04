// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What only this computer keeps, shared with your other computers through a
//! folder your sync already carries: Nextcloud, Dropbox, Syncthing
//! (docs/database.md, "The log").
//!
//! Mail, contacts, the agenda and tasks travel by their servers; projects and
//! notes by the folder that holds them. The rest lives in small files here:
//! settings, who may write to you, ties, time, drafts, health, the watch,
//! lists kept on this computer. Each computer has a name, a UUID made once,
//! and appends each change it finds in them, as one sealed record, to its own
//! file in the folder (`<computer>-<round>.jsonl`): one writer per file, so
//! the sync never makes conflicted copies.
//!
//! Files stay the truth: Sioul reads and writes them as before. A change is
//! found by comparing them with what they held at the last look; a record from
//! another computer is written into them. For each entry (a setting, a line of
//! a list, a session, a whole file) the record with the later clock wins: a
//! hybrid logical clock, the time made strictly increasing here and never
//! behind a record already read. A change is dated by its file's time, so a
//! change made here before another computer's, but found after, still loses.
//!
//! Sealed: each record is encrypted here (XChaCha20-Poly1305) with a key made
//! from a passphrase (Argon2id) and kept in each computer's keyring. The
//! folder and its server see which computer wrote, when and how much, never
//! what.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use sioul_core::config::Config;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The keyring's name for the key made from the passphrase.
pub const KEY_NAME: &str = "Sharing key";

/// Between the parts of a setting's name.
const SEP: char = '\u{1f}';
/// Before an element's identity, in a list split by elements.
const MARK: char = '\u{1e}';
/// Files larger than this stay here.
const LARGEST: u64 = 16 << 20;
/// A computer's file past this size starts a new round.
const ROUND_SIZE: u64 = 1 << 20;
/// Entries taken out are remembered this long, so an old copy does not bring them back.
const TOMBSTONE_DAYS: i64 = 90;
/// A computer silent this long no longer holds back the removal of old rounds.
const SILENT_DAYS: i64 = 180;

/// How a file divides into entries.
#[derive(Debug, Clone, Copy)]
pub enum Shape {
    /// The file whole: a draft, an invoice, a day of the watch.
    Whole,
    /// One entry per line: the sender lists.
    Lines,
    /// One entry per setting of a TOML file.
    Toml(&'static Rules),
}

/// How a TOML file divides, beyond one entry per setting.
#[derive(Debug)]
pub struct Rules {
    /// Lists whose elements are entries.
    pub keyed: &'static [Keyed],
    /// Tables shared whole, their fields together; `*` stands for any name.
    pub whole: &'static [&'static str],
    /// Settings that stay on each computer.
    pub local: &'static [&'static str],
}

/// A list whose elements are entries, each named by some of its fields (none:
/// the element itself), less the fields that stay on each computer.
#[derive(Debug)]
pub struct Keyed {
    pub list: &'static str,
    pub by: &'static [&'static str],
    pub local: &'static [&'static str],
}

static CONFIG_RULES: Rules = Rules {
    keyed: &[Keyed { list: "account", by: &["id"], local: &["maildir", "history_weeks"] }],
    whole: &[],
    // Where things are on this computer, and how text reads on its screen.
    local: &["case_store", "known_senders", "blocked_senders", "reading", "history_weeks", "letters.inbox"],
};
static HEALTH_RULES: Rules = Rules {
    keyed: &[Keyed { list: "prescription", by: &["id"], local: &[] }, Keyed { list: "medicine", by: &["id"], local: &[] }],
    whole: &[],
    local: &["watch_folder"],
};
static LINKS_RULES: Rules = Rules { keyed: &[Keyed { list: "link", by: &[], local: &[] }], whole: &[], local: &[] };
static PORCH_RULES: Rules = Rules { keyed: &[], whole: &["done.*"], local: &[] };
static MONEY_RULES: Rules = Rules { keyed: &[Keyed { list: "ignored", by: &[], local: &[] }], whole: &[], local: &[] };
static TODAY_RULES: Rules = Rules { keyed: &[Keyed { list: "aside", by: &[], local: &[] }], whole: &[], local: &[] };
static TIME_RULES: Rules = Rules { keyed: &[Keyed { list: "session", by: &["start", "task", "project"], local: &[] }], whole: &[], local: &[] };
static PLAIN_RULES: Rules = Rules { keyed: &[], whole: &[], local: &[] };

/// A file, or a folder of files, that is shared.
#[derive(Debug)]
pub struct Store {
    /// Its name in the records, the same on every computer: "config/config.toml", "data/drafts/".
    pub name: String,
    pub path: PathBuf,
    /// A folder: each file in it, and below, on its own.
    pub folder: bool,
    pub shape: Shape,
    /// In a folder: names left out.
    pub skip: &'static [&'static str],
}

/// The folders Sioul keeps its files in.
pub struct Roots {
    pub config: PathBuf,
    pub data: PathBuf,
    pub state: PathBuf,
}

impl Roots {
    pub fn here() -> Roots {
        Roots { config: sioul_core::config::config_dir(), data: sioul_core::config::data_dir(), state: sioul_core::config::state_dir() }
    }
}

/// What is shared. Not shared: caches, how far each sync got, how pages are
/// shown on this screen, the site notices of this computer's browser, your own
/// PGP keys (secret keys never leave the computer they were made on).
pub fn stores(config: &Config, roots: &Roots) -> Vec<Store> {
    let (c, d, s) = (&roots.config, &roots.data, &roots.state);
    let file = |name: &str, path: PathBuf, shape: Shape| Store { name: name.into(), path, folder: false, shape, skip: &[] };
    let folder = |name: &str, path: PathBuf, shape: Shape, skip: &'static [&'static str]| Store { name: name.into(), path, folder: true, shape, skip };
    let senders = |chosen: &Option<String>, name: &str| chosen.as_deref().map_or_else(|| c.join(name), sioul_core::config::expand_home);
    let local = sioul_core::vdir::LOCAL;
    vec![
        file("config/config.toml", c.join("config.toml"), Shape::Toml(&CONFIG_RULES)),
        file("config/known-senders.txt", senders(&config.known_senders, "known-senders.txt"), Shape::Lines),
        file("config/blocked-senders.txt", senders(&config.blocked_senders, "blocked-senders.txt"), Shape::Lines),
        file("config/safe-senders.txt", c.join("safe-senders.txt"), Shape::Lines),
        file("config/neutral-senders.txt", c.join("neutral-senders.txt"), Shape::Lines),
        file("data/links.toml", d.join("links.toml"), Shape::Toml(&LINKS_RULES)),
        file("data/health.toml", d.join("health.toml"), Shape::Toml(&HEALTH_RULES)),
        file("data/time/running.toml", d.join("time").join("running.toml"), Shape::Whole),
        folder("data/time/", d.join("time"), Shape::Toml(&TIME_RULES), &["running.toml"]),
        folder("data/drafts/", d.join("drafts"), Shape::Whole, &[]),
        folder("data/invoices/", d.join("invoices"), Shape::Whole, &[]),
        folder("data/pgp/others/", d.join("pgp").join("others"), Shape::Whole, &[]),
        folder("data/watch/", d.join("watch"), Shape::Whole, &["imported.json"]),
        folder("data/calendars/local/", d.join("calendars").join(local), Shape::Whole, &[]),
        folder("data/contacts/local/", d.join("contacts").join(local), Shape::Whole, &[]),
        file("state/porch.toml", s.join("porch.toml"), Shape::Toml(&PORCH_RULES)),
        file("state/money.toml", s.join("money.toml"), Shape::Toml(&MONEY_RULES)),
        file("state/today.toml", s.join("today.toml"), Shape::Toml(&TODAY_RULES)),
        file("state/quiet.toml", s.join("quiet.toml"), Shape::Toml(&PLAIN_RULES)),
        file("state/health-state.toml", s.join("health-state.toml"), Shape::Toml(&PLAIN_RULES)),
        file("state/watch-offers.json", s.join("watch-offers.json"), Shape::Whole),
        folder("state/shield/", s.join("shield"), Shape::Whole, &[]),
        folder("state/dav/local/", s.join("dav").join(local), Shape::Whole, &[]),
    ]
}

// ---------------------------------------------------------------- entries

/// What the files hold now, entry by entry.
#[derive(Debug, Default)]
struct Found {
    /// Each entry's value, hashed: `<store><file>#<entry>` → its hash.
    hashes: BTreeMap<String, String>,
    /// The values of the entries in files read this time; the others' files did not change.
    values: BTreeMap<String, String>,
    /// When each file last changed, in milliseconds: dates the changes found in it.
    changed: BTreeMap<String, i64>,
    /// Files that could not be read (a TOML file half written, a folder not
    /// there): their entries are neither taken out nor written to.
    unknown: BTreeSet<String>,
    /// Each file's size, time and entries: a file unchanged is not read again.
    files: BTreeMap<String, Stat>,
}

/// A file as last read: its size, when it changed (nanoseconds), its entries' hashes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Stat {
    size: u64,
    modified: u64,
    entries: Vec<(String, String)>,
}

impl Found {
    fn is_unknown(&self, key: &str) -> bool {
        let file = file_of(key);
        self.unknown.iter().any(|u| file == u || (u.ends_with('/') && file.starts_with(u.as_str())))
    }
}

/// `<store><file>`: the part of a key before its entry.
fn file_of(key: &str) -> &str {
    key.split_once('#').map_or(key, |(file, _)| file)
}

fn modified_ns(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64)
}

/// What the stores hold; a file whose size and time did not change since
/// `known` is taken from there, not read.
fn gather(stores: &[Store], known: &BTreeMap<String, Stat>) -> Found {
    let mut found = Found::default();
    for store in stores {
        if store.folder {
            if !store.path.is_dir() {
                found.unknown.insert(store.name.clone());
                continue;
            }
            let mut files = Vec::new();
            list_files(&store.path, &store.path, store.skip, &mut files);
            for (relative, path) in files {
                read_file(store, &format!("{}{relative}", store.name), &path, known, &mut found);
            }
        } else if store.path.is_file() {
            read_file(store, &store.name, &store.path, known, &mut found);
        } else if !matches!(store.shape, Shape::Whole) {
            // A settings file not there is not a file emptied.
            found.unknown.insert(store.name.clone());
        }
    }
    found
}

/// Every file below a folder, by its path from it with `/`; temporary and hidden files left out.
fn list_files(root: &Path, dir: &Path, skip: &[&str], out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.ends_with(".new") || name.ends_with(".tmp") || name.ends_with('~') || skip.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            list_files(root, &path, skip, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push((relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"), path));
        }
    }
}

fn read_file(store: &Store, file: &str, path: &Path, known: &BTreeMap<String, Stat>, found: &mut Found) {
    let Ok(meta) = std::fs::metadata(path) else {
        found.unknown.insert(file.to_string());
        return;
    };
    if meta.len() > LARGEST {
        found.unknown.insert(file.to_string());
        return;
    }
    let modified = modified_ns(&meta);
    found.changed.insert(file.to_string(), (modified / 1_000_000) as i64);
    if let Some(stat) = known.get(file).filter(|s| s.size == meta.len() && s.modified == modified) {
        for (entry, h) in &stat.entries {
            found.hashes.insert(format!("{file}#{entry}"), h.clone());
        }
        found.files.insert(file.to_string(), stat.clone());
        return;
    }
    let Some(entries) = std::fs::read(path).ok().and_then(|bytes| entries_of(store, &bytes)) else {
        found.unknown.insert(file.to_string());
        return;
    };
    let mut stat = Stat { size: meta.len(), modified, entries: Vec::with_capacity(entries.len()) };
    for (entry, value) in entries {
        let key = format!("{file}#{entry}");
        let h = hash(&value);
        stat.entries.push((entry, h.clone()));
        found.hashes.insert(key.clone(), h);
        found.values.insert(key, value);
    }
    found.files.insert(file.to_string(), stat);
}

/// A file's entries and their values; none when it cannot be read as its shape says.
fn entries_of(store: &Store, bytes: &[u8]) -> Option<Vec<(String, String)>> {
    match store.shape {
        Shape::Whole => Some(vec![(String::new(), B64.encode(bytes))]),
        Shape::Lines => Some(String::from_utf8_lossy(bytes).lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| (l.to_string(), String::new())).collect()),
        Shape::Toml(rules) => toml_entries(&String::from_utf8_lossy(bytes), rules).ok(),
    }
}

/// An entry's value, read from its file now.
fn value_of(stores: &[Store], key: &str) -> Option<String> {
    let (file, entry) = key.split_once('#')?;
    let (store, path) = locate(stores, file)?;
    let bytes = std::fs::read(path).ok()?;
    entries_of(store, &bytes)?.into_iter().find(|(e, _)| e == entry).map(|(_, value)| value)
}

/// A value as TOML text, `v = …`: one form for one value, whatever the file's layout.
fn leaf_text(value: &toml::Value) -> String {
    let mut table = toml::Table::new();
    table.insert("v".into(), value.clone());
    toml::to_string(&table).unwrap_or_default()
}

fn leaf_value(text: &str) -> Result<toml::Value, String> {
    let mut table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    table.remove("v").ok_or_else(|| "no value".to_string())
}

fn matches_rule(path: &[String], rule: &str) -> bool {
    let parts: Vec<&str> = rule.split('.').collect();
    parts.len() == path.len() && parts.iter().zip(path).all(|(r, p)| *r == "*" || r == p)
}

/// An element's name in its list: its naming fields, or the element itself.
fn identity(element: &toml::Value, by: &[&str]) -> String {
    if by.is_empty() {
        return leaf_text(element);
    }
    by.iter()
        .map(|field| match element.get(field) {
            Some(toml::Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\u{1d}")
}

fn strip_local(element: &mut toml::Value, local: &[&str]) {
    if let toml::Value::Table(table) = element {
        for field in local {
            table.remove(*field);
        }
    }
}

/// A TOML file's entries: one per setting, one per element of a keyed list,
/// one per table kept whole; the settings that stay here left out.
fn toml_entries(text: &str, rules: &Rules) -> Result<Vec<(String, String)>, String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let mut out = Vec::new();
    for (name, value) in &table {
        if let Some(keyed) = rules.keyed.iter().find(|k| k.list == name)
            && let toml::Value::Array(elements) = value
        {
            for element in elements {
                let mut element = element.clone();
                strip_local(&mut element, keyed.local);
                out.push((format!("{name}{SEP}{MARK}{}", identity(&element, keyed.by)), leaf_text(&element)));
            }
            continue;
        }
        flatten(vec![name.clone()], value, rules, &mut out);
    }
    Ok(out)
}

fn flatten(path: Vec<String>, value: &toml::Value, rules: &Rules, out: &mut Vec<(String, String)>) {
    if rules.local.iter().any(|rule| matches_rule(&path, rule)) {
        return;
    }
    match value {
        toml::Value::Table(table) if !rules.whole.iter().any(|rule| matches_rule(&path, rule)) => {
            for (name, value) in table {
                let mut deeper = path.clone();
                deeper.push(name.clone());
                flatten(deeper, value, rules, out);
            }
        }
        _ => out.push((path.join(&SEP.to_string()), leaf_text(value))),
    }
}

// ---------------------------------------------------------------- writing entries

/// Writes entries into one file: each `(entry, value)`, `None` taking it out.
fn write_entries(store: &Store, path: &Path, changes: &[(&str, Option<&str>)]) -> Result<(), String> {
    match store.shape {
        Shape::Whole => {
            let (_, value) = changes.last().ok_or("nothing")?;
            match value {
                Some(value) => write_atomically(path, &B64.decode(value).map_err(|e| e.to_string())?),
                None => match std::fs::remove_file(path) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
                    _ => Ok(()),
                },
            }
        }
        Shape::Lines => {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
            for (entry, value) in changes {
                let at = lines.iter().position(|l| l.trim() == *entry);
                match (value, at) {
                    (Some(_), None) => lines.push(entry.to_string()),
                    (None, Some(at)) => {
                        lines.remove(at);
                    }
                    _ => {}
                }
            }
            let mut text = lines.join("\n");
            if !text.is_empty() {
                text.push('\n');
            }
            write_atomically(path, text.as_bytes())
        }
        Shape::Toml(rules) => {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let text = toml_write(&text, rules, changes)?;
            write_atomically(path, text.as_bytes())
        }
    }
}

pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".shared.tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, bytes).map_err(fail)?;
    std::fs::rename(&temporary, path).map_err(fail)
}

/// A TOML value as toml_edit holds it: a table stays a table, a list of tables a list of tables.
fn edit_item(text: &str) -> Result<toml_edit::Item, String> {
    let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    let mut item = doc.as_table().get("v").cloned().ok_or("no value")?;
    match &mut item {
        toml_edit::Item::Value(value) => value.decor_mut().clear(),
        toml_edit::Item::Table(table) => table.decor_mut().clear(),
        _ => {}
    }
    Ok(item)
}

/// Sets or takes out entries in a TOML file's text, keeping the rest as written.
fn toml_write(text: &str, rules: &Rules, changes: &[(&str, Option<&str>)]) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    for (entry, value) in changes {
        let path: Vec<&str> = entry.split(SEP).collect();
        if let [list, element] = path.as_slice()
            && let Some(id) = element.strip_prefix(MARK)
        {
            let keyed = rules.keyed.iter().find(|k| k.list == *list).ok_or_else(|| format!("{list}: not a list"))?;
            write_element(&mut doc, keyed, id, *value)?;
        } else {
            let item = value.map(edit_item).transpose()?;
            set_path(doc.as_table_mut(), &path, item.as_ref())?;
        }
    }
    Ok(doc.to_string())
}

fn set_path(table: &mut toml_edit::Table, path: &[&str], value: Option<&toml_edit::Item>) -> Result<(), String> {
    match path {
        [] => Ok(()),
        [last] => {
            match value {
                // A value replaced keeps its comments and spacing.
                Some(toml_edit::Item::Value(new)) if table.get(last).is_some_and(toml_edit::Item::is_value) => {
                    if let Some(old) = table.get_mut(last).and_then(toml_edit::Item::as_value_mut) {
                        let decor = old.decor().clone();
                        *old = new.clone();
                        *old.decor_mut() = decor;
                    }
                }
                Some(item) => {
                    table.insert(last, item.clone());
                }
                None => {
                    table.remove(last);
                }
            }
            Ok(())
        }
        [first, rest @ ..] => {
            if value.is_none() && !table.contains_key(first) {
                return Ok(());
            }
            let item = table.entry(first).or_insert_with(|| {
                let mut new = toml_edit::Table::new();
                new.set_implicit(true);
                toml_edit::Item::Table(new)
            });
            if let Some(inline) = item.as_inline_table().cloned() {
                *item = toml_edit::Item::Table(inline.into_table());
            }
            let inner = item.as_table_mut().ok_or_else(|| format!("{first}: not a table"))?;
            set_path(inner, rest, value)
        }
    }
}

/// The element named `id` of a keyed list: replaced (its local fields kept), added, or taken out.
fn write_element(doc: &mut toml_edit::DocumentMut, keyed: &Keyed, id: &str, value: Option<&str>) -> Result<(), String> {
    let normal = |text: String| -> Option<toml::Value> {
        let mut element = leaf_value(&text).ok()?;
        strip_local(&mut element, keyed.local);
        Some(element)
    };
    let named = |element: Option<toml::Value>| element.is_some_and(|e| identity(&e, keyed.by) == id);
    let new = value.map(edit_item).transpose()?;
    let table = doc.as_table_mut();
    if !table.contains_key(keyed.list) {
        match &new {
            Some(toml_edit::Item::Table(_)) => {
                table.insert(keyed.list, toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()));
            }
            Some(_) => {
                table.insert(keyed.list, toml_edit::value(toml_edit::Array::new()));
            }
            None => return Ok(()),
        }
    }
    match table.get_mut(keyed.list) {
        Some(toml_edit::Item::ArrayOfTables(list)) => {
            let at = (0..list.len()).find(|&i| named(list.get(i).and_then(|t| normal(format!("[v]\n{t}")))));
            match (new, at) {
                (Some(toml_edit::Item::Table(mut element)), at) => {
                    if let Some(old) = at.and_then(|i| list.get(i)) {
                        for field in keyed.local {
                            if let Some(kept) = old.get(field) {
                                element.insert(field, kept.clone());
                            }
                        }
                    }
                    match at.and_then(|i| list.get_mut(i)) {
                        Some(place) => *place = element,
                        None => list.push(element),
                    }
                }
                (None, Some(at)) => {
                    list.remove(at);
                }
                _ => {}
            }
        }
        Some(toml_edit::Item::Value(toml_edit::Value::Array(list))) => {
            let at = (0..list.len()).find(|&i| named(list.get(i).and_then(|v| normal(format!("v = {v}")))));
            match (new, at) {
                (Some(toml_edit::Item::Value(element)), Some(at)) => {
                    list.replace(at, element);
                }
                (Some(toml_edit::Item::Value(element)), None) => list.push(element),
                (None, Some(at)) => {
                    list.remove(at);
                }
                _ => {}
            }
            list.fmt();
        }
        _ => return Err(format!("{}: not a list", keyed.list)),
    }
    Ok(())
}

// ---------------------------------------------------------------- the seal

/// The folder's seal: how the key is made from the passphrase, and a value to
/// check it with. The same on every computer; the passphrase never written.
#[derive(Debug, Serialize, Deserialize)]
struct SealFile {
    version: u32,
    salt: String,
    memory_kib: u32,
    passes: u32,
    check: String,
}

const CHECK: &[u8] = b"sioul: the passphrase is right";

/// Why sharing cannot start.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not the passphrase chosen on the first computer.
    WrongPassphrase,
    Other(String),
}

fn derive(passphrase: &str, salt: &[u8], memory_kib: u32, passes: u32) -> Result<[u8; 32], String> {
    let params = argon2::Params::new(memory_kib, passes, 1, Some(32)).map_err(|e| e.to_string())?;
    let mut key = [0u8; 32];
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params).hash_password_into(passphrase.as_bytes(), salt, &mut key).map_err(|e| e.to_string())?;
    Ok(key)
}

/// The key for a folder: made from the passphrase and the folder's seal, which
/// the first computer writes (64 MiB, 3 passes: half a second, once per computer).
pub fn key_for(folder: &Path, passphrase: &str) -> Result<[u8; 32], Refused> {
    key_with(folder, passphrase, 64 * 1024, 3)
}

fn key_with(folder: &Path, passphrase: &str, memory_kib: u32, passes: u32) -> Result<[u8; 32], Refused> {
    let path = folder.join("seal.toml");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let seal: SealFile = toml::from_str(&text).map_err(|e| Refused::Other(format!("{}: {e}", path.display())))?;
        // The folder passes through others' servers: a seal asking far more than
        // Sioul writes (64 MiB, 3 passes) would only exhaust this computer.
        if seal.memory_kib > 1 << 20 || seal.passes > 16 {
            return Err(Refused::Other(format!("{}: {} KiB, {} passes", path.display(), seal.memory_kib, seal.passes)));
        }
        let salt = B64.decode(&seal.salt).map_err(|e| Refused::Other(e.to_string()))?;
        let key = derive(passphrase, &salt, seal.memory_kib, seal.passes).map_err(Refused::Other)?;
        return match open(&key, "check", &seal.check) {
            Some(plain) if plain == CHECK => Ok(key),
            _ => Err(Refused::WrongPassphrase),
        };
    }
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let key = derive(passphrase, &salt, memory_kib, passes).map_err(Refused::Other)?;
    let seal = SealFile { version: 1, salt: B64.encode(salt), memory_kib, passes, check: seal(&key, "check", CHECK) };
    let text = format!("# Sioul: how your computers' shared records are sealed (docs/database.md).\n{}", toml::to_string(&seal).map_err(|e| Refused::Other(e.to_string()))?);
    std::fs::create_dir_all(folder).and_then(|()| std::fs::write(&path, text)).map_err(|e| Refused::Other(format!("{}: {e}", path.display())))?;
    Ok(key)
}

pub(crate) fn seal(key: &[u8; 32], bound: &str, plain: &[u8]) -> String {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, Payload { msg: plain, aad: bound.as_bytes() }).unwrap_or_default();
    B64.encode([nonce.as_slice(), &sealed].concat())
}

pub(crate) fn open(key: &[u8; 32], bound: &str, text: &str) -> Option<Vec<u8>> {
    let bytes = B64.decode(text).ok()?;
    if bytes.len() < 24 {
        return None;
    }
    let (nonce, sealed) = bytes.split_at(24);
    XChaCha20Poly1305::new(key.into()).decrypt(XNonce::from_slice(nonce), Payload { msg: sealed, aad: bound.as_bytes() }).ok()
}

// ---------------------------------------------------------------- this computer

/// This computer, as sharing knows it: its name, the machine it was made on
/// (a copied configuration on another machine gets a name of its own), the
/// folder it shares through.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Here {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub machine: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

impl Here {
    pub fn path(state: &Path) -> PathBuf {
        state.join("share").join("here.toml")
    }

    /// This computer's name, made once.
    pub fn load(state: &Path) -> Here {
        let path = Here::path(state);
        let mut here: Here = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        let machine = machine();
        if here.id.is_empty() || (!machine.is_empty() && here.machine != machine) {
            here.id = uuid::Uuid::new_v4().to_string();
            here.machine = machine;
            let _ = here.save(state);
        }
        here
    }

    pub fn save(&self, state: &Path) -> Result<(), String> {
        write_atomically(&Here::path(state), toml::to_string(self).map_err(|e| e.to_string())?.as_bytes())
    }

    /// The folder shared through, `~` expanded.
    pub fn folder_path(&self) -> Option<PathBuf> {
        self.folder.as_deref().filter(|f| !f.is_empty()).map(sioul_core::config::expand_home)
    }
}

/// A hash of the machine's own id (`/etc/machine-id`), else of its name.
fn machine() -> String {
    let id = ["/etc/machine-id", "/var/lib/dbus/machine-id"].iter().find_map(|p| std::fs::read_to_string(p).ok()).or_else(|| std::env::var("COMPUTERNAME").ok()).or_else(|| std::fs::read_to_string("/etc/hostname").ok()).unwrap_or_default();
    let id = id.trim();
    if id.is_empty() { String::new() } else { format!("{:016x}", fnv(id.as_bytes())) }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

fn hash(value: &str) -> String {
    format!("{:016x}", fnv(value.as_bytes()))
}

// ---------------------------------------------------------------- memory

/// What this computer remembers between two exchanges.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Memory {
    #[serde(default)]
    computer: String,
    /// The last clock given or read.
    #[serde(default)]
    clock: u64,
    /// This computer's round (its file) and the records in it.
    #[serde(default)]
    round: u32,
    #[serde(default)]
    seq: u64,
    /// Others' records already read in: done once, when sharing starts.
    #[serde(default)]
    joined: bool,
    /// How far each other computer's records were read: its round and the bytes read in it.
    #[serde(default)]
    read: BTreeMap<String, (u32, u64)>,
    /// Others' changes not written yet (their file could not be read, or is
    /// one this version does not know): tried again at each exchange.
    #[serde(default)]
    pending: BTreeMap<String, (Option<String>, u64, String)>,
    /// Each entry's last change: its clock, the computer that made it, its value's hash ("" once taken out).
    #[serde(default)]
    entries: BTreeMap<String, Known>,
    /// The files as last read.
    #[serde(default)]
    files: BTreeMap<String, Stat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Known {
    c: u64,
    w: String,
    h: String,
}

impl Memory {
    fn load(path: &Path, computer: &str) -> Memory {
        let memory: Memory = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if memory.computer == computer { memory } else { Memory { computer: computer.to_string(), round: 1, ..Memory::default() } }
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        write_atomically(path, serde_json::to_string(self).map_err(|e| e.to_string())?.as_bytes())
    }

    /// A new clock: after every clock given or read, and not before `physical` (milliseconds).
    fn tick(&mut self, physical: i64) -> u64 {
        self.clock = (self.clock + 1).max((physical.max(0) as u64) << 16);
        self.clock
    }
}

/// One line of a computer's file: its number in the round, its clock, the sealed change.
#[derive(Debug, Serialize, Deserialize)]
struct Line {
    n: u64,
    c: u64,
    s: String,
}

/// A change, sealed in a line: the entry and its new value, none once taken out.
#[derive(Debug, Serialize, Deserialize)]
struct Change {
    k: String,
    v: Option<String>,
}

/// A computer's own notes in the folder: when it last exchanged, its round, how far it read the others.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Seen {
    #[serde(default)]
    at: i64,
    #[serde(default)]
    round: u32,
    #[serde(default)]
    read: BTreeMap<String, u32>,
}

fn bound(computer: &str, round: u32, n: u64, clock: u64) -> String {
    format!("{computer}:{round}:{n}:{clock}")
}

fn round_file(folder: &Path, computer: &str, round: u32) -> PathBuf {
    folder.join(format!("{computer}-{round}.jsonl"))
}

/// Every computer sharing through the folder: those that wrote records, and
/// those that only read so far (their notes).
fn computers(folder: &Path) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = rounds(folder).into_keys().collect();
    for entry in std::fs::read_dir(folder).into_iter().flatten().filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".toml").filter(|id| id.len() == 36 && uuid::Uuid::parse_str(id).is_ok()) {
            out.insert(id.to_string());
        }
    }
    out
}

/// The computers' files in the folder: computer → its rounds.
fn rounds(folder: &Path) -> BTreeMap<String, Vec<u32>> {
    let mut out: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for entry in std::fs::read_dir(folder).into_iter().flatten().filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".jsonl") else { continue };
        // "<uuid>-<round>": a UUID is 36 characters; a conflicted copy's name is longer.
        let Some((computer, round)) = stem.rsplit_once('-') else { continue };
        if computer.len() != 36 || uuid::Uuid::parse_str(computer).is_err() {
            continue;
        }
        if let Ok(round) = round.parse::<u32>() {
            out.entry(computer.to_string()).or_default().push(round);
        }
    }
    for list in out.values_mut() {
        list.sort_unstable();
    }
    out
}

// ---------------------------------------------------------------- exchange

/// Sharing as set up on this computer.
pub struct Sharing<'a> {
    pub folder: &'a Path,
    pub computer: &'a str,
    pub key: &'a [u8; 32],
    /// Where this computer keeps what it remembers: `<state>/share/memory.json`.
    pub memory: &'a Path,
}

/// What an exchange did.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The stores written to, by name: what to read again.
    pub written: BTreeSet<String>,
    /// Changes found here and sent.
    pub sent: usize,
    /// Changes from the others written here.
    pub received: usize,
    /// What went wrong, in a word each; the rest went on.
    pub problems: Vec<String>,
}

/// Before the first exchange, a copy of every shared file, in case.
fn keep_copies(stores: &[Store], memory: &Path, today: &str) {
    let Some(base) = memory.parent() else { return };
    let aside = base.join(format!("before-sharing-{today}"));
    for store in stores {
        let target = aside.join(store.name.trim_end_matches('/'));
        if store.folder {
            let mut files = Vec::new();
            list_files(&store.path, &store.path, store.skip, &mut files);
            for (relative, path) in files {
                let to = target.join(relative);
                let _ = to.parent().map(std::fs::create_dir_all);
                let _ = std::fs::copy(&path, to);
            }
        } else if store.path.is_file() {
            let _ = target.parent().map(std::fs::create_dir_all);
            let _ = std::fs::copy(&store.path, target);
        }
    }
}

/// One exchange: the changes found here go out, the others' come in.
pub fn exchange(sharing: &Sharing, stores: &[Store], now_ms: i64) -> Result<Outcome, String> {
    std::fs::create_dir_all(sharing.folder).map_err(|e| format!("{}: {e}", sharing.folder.display()))?;
    let mut memory = Memory::load(sharing.memory, sharing.computer);
    let mut outcome = Outcome::default();
    if !memory.joined {
        let today = jiff::Timestamp::from_millisecond(now_ms).map(|t| t.strftime("%Y-%m-%d").to_string()).unwrap_or_default();
        keep_copies(stores, sharing.memory, &today);
    }
    let found = gather(stores, &memory.files);
    let mut out: Vec<(String, Option<String>, u64)> = Vec::new();
    let value = |key: &str| found.values.get(key).cloned().or_else(|| value_of(stores, key));

    // Changes made here since the last look, dated by their file. When joining,
    // the others' records come first, and only what they do not hold goes out.
    if memory.joined {
        for (key, h) in &found.hashes {
            if memory.entries.get(key).is_some_and(|k| k.h == *h) {
                continue;
            }
            let Some(value) = value(key) else { continue };
            let clock = memory.tick(found.changed.get(file_of(key)).copied().unwrap_or(now_ms).min(now_ms));
            memory.entries.insert(key.clone(), Known { c: clock, w: sharing.computer.to_string(), h: h.clone() });
            out.push((key.clone(), Some(value), clock));
        }
        let gone: Vec<String> = memory.entries.iter().filter(|(key, known)| !known.h.is_empty() && !found.hashes.contains_key(*key) && !found.is_unknown(key)).map(|(key, _)| key.clone()).collect();
        for key in gone {
            let clock = memory.tick(now_ms);
            memory.entries.insert(key.clone(), Known { c: clock, w: sharing.computer.to_string(), h: String::new() });
            out.push((key, None, clock));
        }
    }

    // The others' records, from where each was left; those not written last time first.
    let newer = |c: u64, w: &str, than: Option<(u64, &str)>| than.is_none_or(|(tc, tw)| (c, w) > (tc, tw));
    let mut winners: BTreeMap<String, (Option<String>, u64, String)> = std::mem::take(&mut memory.pending)
        .into_iter()
        .filter(|(key, (_, c, w))| newer(*c, w, memory.entries.get(key).map(|k| (k.c, k.w.as_str()))))
        .collect();
    let all = rounds(sharing.folder);
    for (computer, their_rounds) in all.iter().filter(|(c, _)| *c != sharing.computer) {
        let (mut start, mut skip) = memory.read.get(computer).copied().unwrap_or((0, 0));
        if !their_rounds.contains(&start) {
            // Not begun, or that round was removed: the oldest kept starts with all that counts.
            (start, skip) = (their_rounds[0], 0);
        }
        'rounds: for &round in their_rounds.iter().filter(|r| **r >= start) {
            let mut offset = if round == start { skip } else { 0 };
            let mut text = String::new();
            if let Ok(mut file) = std::fs::File::open(round_file(sharing.folder, computer, round)) {
                use std::io::{Read, Seek};
                let _ = file.seek(std::io::SeekFrom::Start(offset)).and_then(|_| file.read_to_string(&mut text));
            }
            // Only whole lines: one still arriving is read next time.
            for line in text.split_inclusive('\n').filter(|l| l.ends_with('\n')) {
                let Ok(record) = serde_json::from_str::<Line>(line) else {
                    offset += line.len() as u64;
                    continue;
                };
                let Some(change) = open(sharing.key, &bound(computer, round, record.n, record.c), &record.s).and_then(|p| serde_json::from_slice::<Change>(&p).ok()) else {
                    outcome.problems.push(format!("share-other-seal:{computer}"));
                    memory.read.insert(computer.clone(), (round, offset));
                    break 'rounds;
                };
                offset += line.len() as u64;
                memory.clock = memory.clock.max(record.c);
                let known = memory.entries.get(&change.k).map(|k| (k.c, k.w.as_str()));
                let pending = winners.get(&change.k).map(|(_, c, w)| (*c, w.as_str()));
                if newer(record.c, computer, known) && newer(record.c, computer, pending) {
                    winners.insert(change.k, (change.v, record.c, computer.clone()));
                }
            }
            memory.read.insert(computer.clone(), (round, offset));
        }
    }

    // The others' changes, written into the files.
    let mut by_file: BTreeMap<String, Vec<(String, Option<String>)>> = BTreeMap::new();
    for (key, (value, _, _)) in &winners {
        by_file.entry(file_of(key).to_string()).or_default().push((key.clone(), value.clone()));
    }
    let mut written_keys: BTreeSet<String> = BTreeSet::new();
    for (file, changes) in &by_file {
        let Some((store, path)) = locate(stores, file) else { continue };
        if found.unknown.contains(file) && !matches!(store.shape, Shape::Whole) && path.exists() {
            // Half written, or written by hand and broken: left as it is.
            outcome.problems.push(format!("share-unreadable:{}", path.display()));
            continue;
        }
        let entries: Vec<(&str, Option<&str>)> = changes.iter().map(|(key, value)| (key.split_once('#').map_or("", |(_, e)| e), value.as_deref())).collect();
        match write_entries(store, &path, &entries) {
            Ok(()) => {
                outcome.written.insert(store.name.clone());
                outcome.received += changes.len();
                written_keys.extend(changes.iter().map(|(k, _)| k.clone()));
            }
            Err(e) => outcome.problems.push(e),
        }
    }
    for (key, (value, clock, computer)) in winners {
        if written_keys.contains(&key) {
            memory.entries.insert(key, Known { c: clock, w: computer, h: value.as_deref().map(hash).unwrap_or_default() });
        } else {
            memory.pending.insert(key, (value, clock, computer));
        }
    }
    // Written files read again: what they hold now is what the next look compares with.
    let files = if outcome.written.is_empty() {
        found.files.clone()
    } else {
        let again = gather(stores, &found.files);
        for (key, known) in memory.entries.iter_mut() {
            if written_keys.contains(key) {
                known.h = again.hashes.get(key).cloned().unwrap_or_default();
            }
        }
        again.files
    };

    // Joining: what only this computer holds goes out.
    if !memory.joined {
        for (key, h) in &found.hashes {
            if memory.entries.contains_key(key) {
                continue;
            }
            let Some(value) = value(key) else { continue };
            let clock = memory.tick(found.changed.get(file_of(key)).copied().unwrap_or(now_ms).min(now_ms));
            memory.entries.insert(key.clone(), Known { c: clock, w: sharing.computer.to_string(), h: h.clone() });
            out.push((key.clone(), Some(value), clock));
        }
        memory.joined = true;
    }

    outcome.sent = out.len();
    if memory.round == 0 {
        memory.round = 1;
    }
    append(sharing, &mut memory, &out)?;
    if std::fs::metadata(round_file(sharing.folder, sharing.computer, memory.round)).is_ok_and(|m| m.len() > ROUND_SIZE) {
        new_round(sharing, &mut memory, stores, &found, now_ms)?;
    }
    memory.files = files;
    write_seen(sharing, &memory, &all, now_ms);
    remove_old_rounds(sharing, &memory, now_ms);
    memory.save(sharing.memory)?;
    Ok(outcome)
}

/// The store and the file a key's file part names.
fn locate<'a>(stores: &'a [Store], file: &str) -> Option<(&'a Store, PathBuf)> {
    if let Some(store) = stores.iter().find(|s| !s.folder && s.name == file) {
        return Some((store, store.path.clone()));
    }
    let store = stores.iter().filter(|s| s.folder && file.starts_with(s.name.as_str())).max_by_key(|s| s.name.len())?;
    let relative = &file[store.name.len()..];
    // Never outside its folder, on Windows too, where "\" also separates and
    // "C:" starts another place.
    if relative.is_empty() || relative.split('/').any(|part| part.is_empty() || part == ".." || part == "." || part.contains(['\\', ':'])) {
        return None;
    }
    Some((store, relative.split('/').fold(store.path.clone(), |path, part| path.join(part))))
}

fn append(sharing: &Sharing, memory: &mut Memory, changes: &[(String, Option<String>, u64)]) -> Result<(), String> {
    if changes.is_empty() {
        return Ok(());
    }
    let path = round_file(sharing.folder, sharing.computer, memory.round);
    let mut text = String::new();
    for (key, value, clock) in changes {
        memory.seq += 1;
        let plain = serde_json::to_vec(&Change { k: key.clone(), v: value.clone() }).map_err(|e| e.to_string())?;
        let line = Line { n: memory.seq, c: *clock, s: seal(sharing.key, &bound(sharing.computer, memory.round, memory.seq, *clock), &plain) };
        text.push_str(&serde_json::to_string(&line).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(fail)?;
    file.write_all(text.as_bytes()).map_err(fail)?;
    file.sync_all().map_err(fail)
}

/// A new file for this computer, opening on every entry it holds the last
/// word on, with their clocks; entries taken out long ago are forgotten.
fn new_round(sharing: &Sharing, memory: &mut Memory, stores: &[Store], found: &Found, now_ms: i64) -> Result<(), String> {
    let oldest = ((now_ms - TOMBSTONE_DAYS * 86_400_000).max(0) as u64) << 16;
    memory.entries.retain(|_, known| !known.h.is_empty() || known.c >= oldest);
    let ours: Vec<(String, Option<String>, u64)> = memory
        .entries
        .iter()
        .filter(|(_, known)| known.w == sharing.computer)
        .filter_map(|(key, known)| {
            if known.h.is_empty() {
                return (!found.hashes.contains_key(key)).then(|| (key.clone(), None, known.c));
            }
            let value = found.values.get(key).cloned().or_else(|| value_of(stores, key))?;
            (hash(&value) == known.h).then(|| (key.clone(), Some(value), known.c))
        })
        .collect();
    memory.round += 1;
    memory.seq = 0;
    append(sharing, memory, &ours)
}

fn seen_path(folder: &Path, computer: &str) -> PathBuf {
    folder.join(format!("{computer}.toml"))
}

/// This computer's notes in the folder, when they change or a quarter of an
/// hour after the last: the sync is not asked to carry a file every minute.
fn write_seen(sharing: &Sharing, memory: &Memory, all: &BTreeMap<String, Vec<u32>>, now_ms: i64) {
    let seen = Seen {
        at: now_ms / 1000,
        round: memory.round,
        read: memory.read.iter().filter(|(c, _)| all.contains_key(*c)).map(|(c, (round, _))| (c.clone(), *round)).collect(),
    };
    if read_seen(sharing.folder, sharing.computer).is_some_and(|old| old.round == seen.round && old.read == seen.read && seen.at - old.at < 15 * 60) {
        return;
    }
    if let Ok(text) = toml::to_string(&seen) {
        let _ = write_atomically(&seen_path(sharing.folder, sharing.computer), text.as_bytes());
    }
}

fn read_seen(folder: &Path, computer: &str) -> Option<Seen> {
    std::fs::read_to_string(seen_path(folder, computer)).ok().and_then(|t| toml::from_str(&t).ok())
}

/// This computer's old files, once every other computer heard lately has read past them.
fn remove_old_rounds(sharing: &Sharing, memory: &Memory, now_ms: i64) {
    let all = rounds(sharing.folder);
    let others: Vec<Seen> = computers(sharing.folder).iter().filter(|c| *c != sharing.computer).filter_map(|c| read_seen(sharing.folder, c)).filter(|s| now_ms / 1000 - s.at < SILENT_DAYS * 86_400).collect();
    let read_up_to = others.iter().map(|s| s.read.get(sharing.computer).copied().unwrap_or(0)).min().unwrap_or(memory.round);
    for round in all.get(sharing.computer).into_iter().flatten().filter(|r| **r < memory.round && **r < read_up_to) {
        let _ = std::fs::remove_file(round_file(sharing.folder, sharing.computer, *round));
    }
}

/// Another computer sharing through the folder, and when it last exchanged (Unix seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Other {
    pub id: String,
    pub heard: i64,
}

/// The other computers sharing through the folder.
pub fn others(folder: &Path, computer: &str) -> Vec<Other> {
    computers(folder).into_iter().filter(|c| c != computer).map(|c| Other { heard: read_seen(folder, &c).map_or(0, |s| s.at), id: c }).collect()
}

/// Whether the folder already holds a seal: another computer shares through it.
pub fn sealed(folder: &Path) -> bool {
    folder.join("seal.toml").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A computer of its own: its folders, its name, its memory.
    struct Computer {
        roots: Roots,
        id: String,
        memory: PathBuf,
    }

    impl Computer {
        fn new(base: &Path, name: &str) -> Computer {
            let root = base.join(name);
            let roots = Roots { config: root.join("config"), data: root.join("data"), state: root.join("state") };
            for dir in [&roots.config, &roots.data, &roots.state] {
                std::fs::create_dir_all(dir).unwrap();
            }
            let memory = roots.state.join("share").join("memory.json");
            Computer { roots, id: uuid::Uuid::new_v4().to_string(), memory }
        }

        fn exchange(&self, folder: &Path, key: &[u8; 32], now: i64) -> Outcome {
            let stores = stores(&Config::default(), &self.roots);
            exchange(&Sharing { folder, computer: &self.id, key, memory: &self.memory }, &stores, now).unwrap()
        }

        fn write(&self, relative: &str, text: &str) {
            let path = self.path(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        fn read(&self, relative: &str) -> String {
            std::fs::read_to_string(self.path(relative)).unwrap_or_default()
        }

        fn path(&self, relative: &str) -> PathBuf {
            let (root, rest) = relative.split_once('/').unwrap();
            match root {
                "config" => self.roots.config.join(rest),
                "data" => self.roots.data.join(rest),
                _ => self.roots.state.join(rest),
            }
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-share-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn quick_key(folder: &Path, passphrase: &str) -> Result<[u8; 32], Refused> {
        key_with(folder, passphrase, 64, 1)
    }

    const MINUTE: i64 = 60_000;
    const NOW: i64 = 1_790_000_000_000;

    #[test]
    fn two_computers_agree() {
        let base = scratch("agree");
        let folder = base.join("Nextcloud").join("sioul-shared");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));

        // The desk shares first: its settings, its lists, a draft, a session.
        desk.write("config/config.toml", "language = \"fr\"\ncase_store = \"~/Notes\"\n\n[tasks]\nkind = \"plan\"\n\n[[account]]\nid = \"perso\"\nkind = \"imap\"\nmaildir = \"/desk/mail\"\n");
        desk.write("config/blocked-senders.txt", "spam@example.org\n");
        desk.write("data/drafts/a1.toml", "subject = \"Hello\"\n");
        desk.write("data/time/2026-10.toml", "[[session]]\ntask = \"t1\"\nstart = 100\nminutes = 25\n");
        assert_eq!(desk.exchange(&folder, &key, NOW).received, 0);

        // The laptop joins with its own: its case store stays, its blocked sender joins the desk's.
        laptop.write("config/config.toml", "language = \"en\"\ncase_store = \"~/Nextcloud/Notes\"\n");
        laptop.write("config/blocked-senders.txt", "*@pushy.example.com\n");
        let outcome = laptop.exchange(&folder, &key, NOW + MINUTE);
        assert!(outcome.received > 0 && outcome.sent > 0, "{outcome:?}");
        let config = laptop.read("config/config.toml");
        assert!(config.contains("language = \"fr\""), "the shared settings win when joining: {config}");
        assert!(config.contains("case_store = \"~/Nextcloud/Notes\""), "where things are stays here: {config}");
        assert!(config.contains("id = \"perso\"") && !config.contains("/desk/mail"), "accounts come, without this computer's paths: {config}");
        assert_eq!(laptop.read("data/drafts/a1.toml"), "subject = \"Hello\"\n");
        assert!(laptop.read("data/time/2026-10.toml").contains("task = \"t1\""));
        let blocked = laptop.read("config/blocked-senders.txt");
        assert!(blocked.contains("spam@example.org") && blocked.contains("*@pushy.example.com"), "{blocked}");

        // The desk hears the laptop's sender.
        desk.exchange(&folder, &key, NOW + 2 * MINUTE);
        assert!(desk.read("config/blocked-senders.txt").contains("*@pushy.example.com"));

        // Both work apart: a session each in the same month, a setting each, a draft sent on the laptop.
        desk.write("data/time/2026-10.toml", "[[session]]\ntask = \"t1\"\nstart = 100\nminutes = 25\n\n[[session]]\ntask = \"t2\"\nstart = 200\nminutes = 50\n");
        desk.write("config/config.toml", &desk.read("config/config.toml").replace("kind = \"plan\"", "kind = \"list\""));
        laptop.write("data/time/2026-10.toml", &format!("{}\n[[session]]\nproject = \"p\"\nstart = 300\nminutes = 10\n", laptop.read("data/time/2026-10.toml")));
        let config = laptop.read("config/config.toml").replace("language = \"fr\"", "language = \"de\"");
        laptop.write("config/config.toml", &config);
        std::fs::remove_file(laptop.path("data/drafts/a1.toml")).unwrap();
        desk.exchange(&folder, &key, NOW + 3 * MINUTE);
        laptop.exchange(&folder, &key, NOW + 4 * MINUTE);
        desk.exchange(&folder, &key, NOW + 5 * MINUTE);
        for computer in [&desk, &laptop] {
            let time = computer.read("data/time/2026-10.toml");
            assert!(time.contains("\"t1\"") && time.contains("\"t2\"") && time.contains("project = \"p\""), "{time}");
            let config = computer.read("config/config.toml");
            assert!(config.contains("language = \"de\"") && config.contains("kind = \"list\""), "{config}");
            assert!(!computer.path("data/drafts/a1.toml").exists(), "a draft sent on one is gone on both");
        }
        assert!(desk.read("config/config.toml").contains("maildir = \"/desk/mail\""), "the desk keeps its own path");

        // Nothing changed: nothing goes out.
        assert_eq!(desk.exchange(&folder, &key, NOW + 6 * MINUTE).sent, 0);
        assert_eq!(laptop.exchange(&folder, &key, NOW + 7 * MINUTE).sent, 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_later_word_wins() {
        let base = scratch("later");
        let folder = base.join("shared");
        let key = quick_key(&folder, "a passphrase of some length").unwrap();
        let (a, b) = (Computer::new(&base, "a"), Computer::new(&base, "b"));
        a.write("config/safe-senders.txt", "friend@example.org\n");
        a.exchange(&folder, &key, NOW);
        b.exchange(&folder, &key, NOW + MINUTE);
        assert!(b.read("config/safe-senders.txt").contains("friend@example.org"));

        // Safe on one, then blocked later on the other: blocked everywhere, and safe nowhere.
        b.write("config/safe-senders.txt", "");
        b.write("config/blocked-senders.txt", "friend@example.org\n");
        b.exchange(&folder, &key, NOW + 2 * MINUTE);
        a.exchange(&folder, &key, NOW + 3 * MINUTE);
        assert!(!a.read("config/safe-senders.txt").contains("friend@example.org"));
        assert!(a.read("config/blocked-senders.txt").contains("friend@example.org"));

        // A list that cannot be read is left alone, and nothing of it is taken out elsewhere.
        a.write("state/quiet.toml", "work_until = 5\n");
        a.exchange(&folder, &key, NOW + 4 * MINUTE);
        b.exchange(&folder, &key, NOW + 5 * MINUTE);
        assert!(b.read("state/quiet.toml").contains("work_until = 5"));
        b.write("state/quiet.toml", "work_until = [ broken");
        b.exchange(&folder, &key, NOW + 6 * MINUTE);
        a.exchange(&folder, &key, NOW + 7 * MINUTE);
        assert!(a.read("state/quiet.toml").contains("work_until = 5"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sealed_with_one_passphrase() {
        let base = scratch("seal");
        let folder = base.join("shared");
        let key = quick_key(&folder, "the right passphrase").unwrap();
        assert_eq!(quick_key(&folder, "the right passphrase"), Ok(key));
        assert_eq!(quick_key(&folder, "another passphrase"), Err(Refused::WrongPassphrase));

        // What travels says nothing of what it holds.
        let a = Computer::new(&base, "a");
        a.write("config/safe-senders.txt", "friend@example.org\n");
        a.exchange(&folder, &key, NOW);
        let file = std::fs::read_to_string(round_file(&folder, &a.id, 1)).unwrap();
        assert!(!file.contains("friend") && !file.contains("safe-senders"), "{file}");
        assert_eq!(others(&folder, "someone else").len(), 1);
        // A seal asking more than this computer can give is refused before it is tried.
        let text = std::fs::read_to_string(folder.join("seal.toml")).unwrap();
        std::fs::write(folder.join("seal.toml"), text.replace("memory_kib = 64", "memory_kib = 4294967295")).unwrap();
        assert!(matches!(quick_key(&folder, "the right passphrase"), Err(Refused::Other(_))));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn records_stay_in_their_folders() {
        let roots = Roots { config: PathBuf::from("config"), data: PathBuf::from("data"), state: PathBuf::from("state") };
        let stores = stores(&Config::default(), &roots);
        assert!(locate(&stores, "data/drafts/a1.toml").is_some());
        for file in ["data/drafts/../x", "data/drafts/", "data/drafts//x", "data/drafts/a\\..\\..\\x", "data/drafts/C:x"] {
            assert!(locate(&stores, file).is_none(), "{file}");
        }
    }

    #[test]
    fn rounds_start_over_and_old_ones_go() {
        let base = scratch("rounds");
        let folder = base.join("shared");
        let key = quick_key(&folder, "a passphrase of some length").unwrap();
        let (a, b) = (Computer::new(&base, "a"), Computer::new(&base, "b"));
        a.write("config/safe-senders.txt", "friend@example.org\n");
        a.exchange(&folder, &key, NOW);
        b.exchange(&folder, &key, NOW + MINUTE);
        // A large file, changed until a's file passes its size.
        let mut now = NOW + MINUTE;
        let mut round = 1;
        while round == 1 {
            now += MINUTE;
            a.write("data/watch/2026-10-01.json", &format!("{{\"steps\": {now}, \"pad\": \"{}\"}}", "x".repeat(100_000)));
            a.exchange(&folder, &key, now);
            round = Memory::load(&a.memory, &a.id).round;
        }
        assert!(round_file(&folder, &a.id, 1).exists(), "kept until b has read past it");
        // b reads past round 1.
        b.exchange(&folder, &key, now + MINUTE);
        assert!(b.read("data/watch/2026-10-01.json").contains(&format!("\"steps\": {now}")));
        a.exchange(&folder, &key, now + 2 * MINUTE);
        assert!(!round_file(&folder, &a.id, 1).exists(), "read by everyone: removed");
        // A third computer joins later: round 2 alone holds the day.
        let c = Computer::new(&base, "c");
        c.exchange(&folder, &key, now + 3 * MINUTE);
        assert!(c.read("data/watch/2026-10-01.json").contains(&format!("\"steps\": {now}")));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn toml_entries_and_back() {
        let rules = &CONFIG_RULES;
        let text = "# mine\nlanguage = \"fr\" # kept\ncase_store = \"~/Notes\"\n\n[quiet]\npersonal = [\"joy\", \"family\"]\n\n[[account]]\nid = \"a\"\nmaildir = \"/x\"\n";
        let entries = toml_entries(text, rules).unwrap();
        let names: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"language") && names.contains(&"quiet\u{1f}personal") && names.contains(&"account\u{1f}\u{1e}a"), "{names:?}");
        assert!(!names.iter().any(|n| n.starts_with("case_store")));
        // Writing keeps the rest of the file as written.
        let account = entries.iter().find(|(k, _)| k.starts_with("account")).unwrap().1.replace("id = \"a\"", "id = \"a\"\nmuted = true");
        let new = toml_write(text, rules, &[("language", Some("v = \"en\"\n")), ("account\u{1f}\u{1e}a", Some(&account)), ("quiet\u{1f}work", Some("v = [\"client\"]\n"))]).unwrap();
        assert!(new.starts_with("# mine\nlanguage = \"en\""), "{new}");
        assert!(new.contains("muted = true") && new.contains("maildir = \"/x\""), "{new}");
        assert!(new.contains("work = [\"client\"]"), "{new}");
        // A list of words, one at a time.
        let money = "ignored = [\"a\", \"b\"]\n";
        let new = toml_write(money, &MONEY_RULES, &[("ignored\u{1f}\u{1e}v = \"a\"\n", None), ("ignored\u{1f}\u{1e}v = \"c\"\n", Some("v = \"c\"\n"))]).unwrap();
        assert_eq!(new, "ignored = [\"b\", \"c\"]\n");
    }
}

#[cfg(test)]
mod probe {
    /// What a copy of real folders holds, in counts: `SIOUL_SHARE_PROBE=<dir with config, data, state>`.
    #[test]
    #[ignore]
    fn probe() {
        let Some(base) = std::env::var_os("SIOUL_SHARE_PROBE").map(std::path::PathBuf::from) else { return };
        let roots = super::Roots { config: base.join("config"), data: base.join("data"), state: base.join("state") };
        let config: sioul_core::config::Config = std::fs::read_to_string(roots.config.join("config.toml")).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        let stores = super::stores(&config, &roots);
        let found = super::gather(&stores, &Default::default());
        for store in &stores {
            let count = found.hashes.keys().filter(|k| super::file_of(k).starts_with(store.name.as_str()) && (store.folder || super::file_of(k) == store.name)).count();
            println!("{:32} {count}", store.name);
        }
        println!("unknown: {:?}", found.unknown);
    }
}
