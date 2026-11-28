// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The two templates of the clients' files, versioned (docs/clients.md,
//! "Templates"): the folder tree and the databases (kinds of record, their
//! fields, a list's choices). Both are settings, `[clients]` in
//! `config.toml`, one for every file, carried by the sharing like every
//! setting. Each folder, kind, field and choice is a table keyed by its id,
//! so that each value travels on its own (an element of an array of tables
//! would travel whole, and two devices changing two fields of one kind would
//! lose one).
//!
//! Everything in a file is named by these ids: a template's change rewrites
//! nothing a file holds. A removal is a version number, not a deletion, so
//! that what a file holds keeps its name and comes back with it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `[clients]`: the templates and the options that protect. Read leniently:
/// a part that does not read is passed over, never the configuration refused.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Settings {
    /// "What you call them": "" is "clients". The view's word.
    pub word: String,
    /// The start by trade picked, if any (informative).
    pub start: String,
    /// The folder tree's version, raised at each of its changes.
    pub folders_version: u32,
    /// The kinds' version, raised at each change of a kind, a field or a choice.
    pub kinds_version: u32,
    pub folder: BTreeMap<String, FolderDef>,
    pub kind: BTreeMap<String, KindDef>,
    pub protect: super::protect::Protect,
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Settings, D::Error> {
        let found: toml::Value = Deserialize::deserialize(d)?;
        Ok(Settings::of(&found))
    }
}

impl Settings {
    /// `[clients]` as written, each part that reads kept, the others passed over.
    pub fn of(value: &toml::Value) -> Settings {
        let Some(t) = value.as_table() else { return Settings::default() };
        let text = |k: &str| t.get(k).and_then(toml::Value::as_str).unwrap_or("").to_string();
        let number = |k: &str| t.get(k).and_then(toml::Value::as_integer).and_then(|n| u32::try_from(n).ok()).unwrap_or(0);
        Settings {
            word: text("word"),
            start: text("start"),
            folders_version: number("folders_version"),
            kinds_version: number("kinds_version"),
            folder: t.get("folder").map(table_of).unwrap_or_default(),
            kind: t.get("kind").map(table_of).unwrap_or_default(),
            protect: t.get("protect").map(super::protect::Protect::of).unwrap_or_default(),
        }
    }

    /// The kinds in their order, archived ones included.
    pub fn kinds(&self) -> Vec<(&String, &KindDef)> {
        let mut out: Vec<_> = self.kind.iter().collect();
        out.sort_by(|a, b| a.1.place.cmp(&b.1.place).then(a.0.cmp(b.0)));
        out
    }

    /// The folders in their order, removed ones included.
    pub fn folders(&self) -> Vec<(&String, &FolderDef)> {
        let mut out: Vec<_> = self.folder.iter().collect();
        out.sort_by(|a, b| a.1.place.cmp(&b.1.place).then(a.0.cmp(b.0)));
        out
    }
}

/// A table of tables keyed by id, each element that reads kept.
fn table_of<T: serde::de::DeserializeOwned>(value: &toml::Value) -> BTreeMap<String, T> {
    value.as_table().map(|t| t.iter().filter(|(k, _)| super::address::valid_id(k)).filter_map(|(k, v)| Some((k.clone(), v.clone().try_into::<T>().ok()?))).collect()).unwrap_or_default()
}

/// A folder of the tree.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderDef {
    pub name: String,
    /// "" for a top folder, else a top folder's id: one level.
    pub parent: String,
    pub place: u32,
    pub health: bool,
    /// The folders' version it was removed at; 0 while in the template.
    pub removed: u32,
}

/// A kind of record: one database per kind, a row a record.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KindDef {
    pub name: String,
    pub place: u32,
    pub health: bool,
    /// Appointments make it ("Write the session").
    pub appointments: bool,
    /// The kind appointments make by default.
    pub default: bool,
    /// The words of a row's line, its fields by id in braces: "{services} · {price}".
    pub line: String,
    /// The kinds' version it was archived at; 0 while in use.
    pub archived: u32,
    /// The kinds' version it was added at.
    pub since: u32,
    #[serde(deserialize_with = "lenient_map")]
    pub field: BTreeMap<String, FieldDef>,
}

/// A field of a kind.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldDef {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: FieldType,
    /// A number's unit ("ml", "min").
    pub unit: String,
    pub place: u32,
    /// Shown in the row's line.
    pub shown: bool,
    pub health: bool,
    /// The kinds' version it was removed at; 0 while shown.
    pub removed: u32,
    /// The kinds' version it was added at.
    pub since: u32,
    /// The kinds' version its type last changed at; 0 never.
    pub retyped: u32,
    /// A list's choices, by id.
    #[serde(deserialize_with = "lenient_map")]
    pub choice: BTreeMap<String, ChoiceDef>,
}

/// A choice of a list.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChoiceDef {
    pub name: String,
    pub place: u32,
    /// The kinds' version it was archived at; 0 while offered.
    pub archived: u32,
}

/// A field's type. A type a later Sioul adds reads as `Unknown`, kept as
/// written; a row holding a value of one is read only here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FieldType {
    #[default]
    Line,
    Text,
    Number,
    Amount,
    Day,
    Duration,
    Choice,
    Choices,
    YesNo,
    Supply,
    Attachment,
    Unknown(String),
}

impl FieldType {
    /// Every type this Sioul knows.
    pub const KNOWN: [FieldType; 11] = [FieldType::Line, FieldType::Text, FieldType::Number, FieldType::Amount, FieldType::Day, FieldType::Duration, FieldType::Choice, FieldType::Choices, FieldType::YesNo, FieldType::Supply, FieldType::Attachment];

    pub fn id(&self) -> &str {
        match self {
            FieldType::Line => "line",
            FieldType::Text => "text",
            FieldType::Number => "number",
            FieldType::Amount => "amount",
            FieldType::Day => "day",
            FieldType::Duration => "duration",
            FieldType::Choice => "choice",
            FieldType::Choices => "choices",
            FieldType::YesNo => "yes-no",
            FieldType::Supply => "supply",
            FieldType::Attachment => "attachment",
            FieldType::Unknown(id) => id,
        }
    }

    pub fn parse(id: &str) -> FieldType {
        FieldType::KNOWN.into_iter().find(|t| t.id() == id).unwrap_or_else(|| FieldType::Unknown(id.to_string()))
    }
}

impl Serialize for FieldType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.id())
    }
}

impl<'de> Deserialize<'de> for FieldType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<FieldType, D::Error> {
        let found: toml::Value = Deserialize::deserialize(d)?;
        Ok(found.as_str().map(FieldType::parse).unwrap_or_else(|| FieldType::Unknown(found.to_string())))
    }
}

/// A table of tables keyed by id, each element that reads kept, the others
/// (and any key that is no id) passed over.
fn lenient_map<'de, D: serde::Deserializer<'de>, T: serde::de::DeserializeOwned>(d: D) -> Result<BTreeMap<String, T>, D::Error> {
    let found: toml::Value = Deserialize::deserialize(d)?;
    let Some(table) = found.as_table() else { return Ok(BTreeMap::new()) };
    Ok(table.iter().filter(|(k, _)| super::address::valid_id(k)).filter_map(|(k, v)| Some((k.clone(), v.clone().try_into::<T>().ok()?))).collect())
}

// ---------------------------------------------------------------- evolutions

use std::path::Path;
use toml_edit::{DocumentMut, Item, Table};

/// What an evolution of a template changed: the id it made or changed, and
/// the part as it was before, to undo it (`undo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evolved {
    pub id: String,
    /// `"folder.<id>"`, or `"kind.<id>"` (a field or a choice changes its kind).
    pub part: String,
    /// The part's text before (None: it was not there).
    pub before: Option<String>,
}

/// An id made of a name, free among `taken`: "Acte de soin" → "acte-de-soin".
pub fn id_from(name: &str, fallback: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let folded: String = crate::text::fold(name).into_iter().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let mut base: String = folded.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    base.truncate(40);
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() { fallback.to_string() } else { base };
    if !taken(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|id| !taken(id)).expect("an id is free")
}

/// `[clients]` of the configuration at `path`, changed by `change` under the
/// file's lock and written in place, comments kept.
fn edit(path: &Path, change: impl FnOnce(&mut Table) -> Result<Evolved, String>) -> Result<Evolved, String> {
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let clients = sub(doc.as_table_mut(), "clients");
        let evolved = change(clients)?;
        crate::config::write_document(path, &doc)?;
        Ok(evolved)
    })
}

/// The table `key` of `parent`, made (implicit) when absent.
fn sub<'a>(parent: &'a mut Table, key: &str) -> &'a mut Table {
    if !parent.get(key).is_some_and(Item::is_table) {
        let mut t = Table::new();
        t.set_implicit(true);
        parent.insert(key, Item::Table(t));
    }
    parent.get_mut(key).and_then(Item::as_table_mut).expect("a table")
}

/// The version `key` raised by one; the new one.
fn bump(clients: &mut Table, key: &str) -> u32 {
    let now = clients.get(key).and_then(Item::as_integer).unwrap_or(0).max(0) as u32 + 1;
    clients.insert(key, toml_edit::value(i64::from(now)));
    now
}

/// A part's text, to put back.
fn snapshot(table: &Table, id: &str) -> Option<String> {
    let item = table.get(id)?;
    let mut doc = DocumentMut::new();
    doc.insert("x", item.clone());
    Some(doc.to_string())
}

fn largest_place(table: &Table) -> i64 {
    table.iter().filter_map(|(_, v)| v.as_table()?.get("place")?.as_integer()).max().unwrap_or(0)
}

fn table_mut<'a>(parent: &'a mut Table, id: &str, what: &str) -> Result<&'a mut Table, String> {
    parent.get_mut(id).and_then(Item::as_table_mut).ok_or_else(|| format!("no such {what}: {id}"))
}

fn kind_of_known(kind: &str) -> Result<FieldType, String> {
    match FieldType::parse(kind) {
        FieldType::Unknown(k) => Err(format!("no such field type: {k}")),
        t => Ok(t),
    }
}

fn clean(name: &str) -> Result<String, String> {
    let name = name.replace(['\n', '\r'], " ").trim().to_string();
    if name.is_empty() { Err("a name is needed".into()) } else { Ok(name) }
}

/// A folder added to the tree (a top folder, or under one): it appears in every file, empty.
pub fn add_folder(path: &Path, name: &str, parent: &str) -> Result<Evolved, String> {
    let name = clean(name)?;
    edit(path, |clients| {
        let version = bump(clients, "folders_version");
        let folders = sub(clients, "folder");
        if !parent.is_empty() && !folders.get(parent).and_then(Item::as_table).is_some_and(|t| t.get("parent").and_then(Item::as_str).unwrap_or("").is_empty()) {
            return Err("a folder goes under a top folder".into());
        }
        let id = id_from(&name, "folder", &|id| folders.contains_key(id));
        let mut t = Table::new();
        t.insert("name", toml_edit::value(name.as_str()));
        if !parent.is_empty() {
            t.insert("parent", toml_edit::value(parent));
        }
        t.insert("place", toml_edit::value(largest_place(folders) + 1));
        let _ = version;
        folders.insert(&id, Item::Table(t));
        Ok(Evolved { part: format!("folder.{id}"), id, before: None })
    })
}

/// A folder renamed everywhere; its id stays.
pub fn rename_folder(path: &Path, id: &str, name: &str) -> Result<Evolved, String> {
    let name = clean(name)?;
    change_folder(path, id, |t, _| {
        t.insert("name", toml_edit::value(name.as_str()));
    })
}

/// A folder out of the template: it stays in each file that holds something
/// there, "No longer in your template" (`folders_of`).
pub fn remove_folder(path: &Path, id: &str) -> Result<Evolved, String> {
    change_folder(path, id, |t, version| {
        t.insert("removed", toml_edit::value(i64::from(version)));
    })
}

/// A folder removed brought back.
pub fn bring_back_folder(path: &Path, id: &str) -> Result<Evolved, String> {
    change_folder(path, id, |t, _| {
        t.remove("removed");
    })
}

/// A folder's health mark.
pub fn mark_folder(path: &Path, id: &str, health: bool) -> Result<Evolved, String> {
    change_folder(path, id, |t, _| {
        if health {
            t.insert("health", toml_edit::value(true));
        } else {
            t.remove("health");
        }
    })
}

fn change_folder(path: &Path, id: &str, change: impl FnOnce(&mut Table, u32)) -> Result<Evolved, String> {
    edit(path, |clients| {
        let version = bump(clients, "folders_version");
        let folders = sub(clients, "folder");
        let before = snapshot(folders, id);
        change(table_mut(folders, id, "folder")?, version);
        Ok(Evolved { id: id.to_string(), part: format!("folder.{id}"), before })
    })
}

/// A kind of record added, with no field yet.
pub fn add_kind(path: &Path, name: &str) -> Result<Evolved, String> {
    let name = clean(name)?;
    edit(path, |clients| {
        let version = bump(clients, "kinds_version");
        let kinds = sub(clients, "kind");
        let id = id_from(&name, "kind", &|id| kinds.contains_key(id));
        let mut t = Table::new();
        t.insert("name", toml_edit::value(name.as_str()));
        t.insert("place", toml_edit::value(largest_place(kinds) + 1));
        t.insert("since", toml_edit::value(i64::from(version)));
        kinds.insert(&id, Item::Table(t));
        Ok(Evolved { part: format!("kind.{id}"), id, before: None })
    })
}

fn change_kind(path: &Path, kind: &str, change: impl FnOnce(&mut Table, u32) -> Result<String, String>) -> Result<Evolved, String> {
    edit(path, |clients| {
        let version = bump(clients, "kinds_version");
        let kinds = sub(clients, "kind");
        let before = snapshot(kinds, kind);
        let id = change(table_mut(kinds, kind, "kind")?, version)?;
        Ok(Evolved { id, part: format!("kind.{kind}"), before })
    })
}

pub fn rename_kind(path: &Path, kind: &str, name: &str) -> Result<Evolved, String> {
    let name = clean(name)?;
    change_kind(path, kind, |t, _| {
        t.insert("name", toml_edit::value(name.as_str()));
        Ok(kind.to_string())
    })
}

/// A kind removed is archived: its rows stay, shown, and no new one is made.
pub fn archive_kind(path: &Path, kind: &str) -> Result<Evolved, String> {
    change_kind(path, kind, |t, version| {
        t.insert("archived", toml_edit::value(i64::from(version)));
        t.remove("default");
        Ok(kind.to_string())
    })
}

pub fn bring_back_kind(path: &Path, kind: &str) -> Result<Evolved, String> {
    change_kind(path, kind, |t, _| {
        t.remove("archived");
        Ok(kind.to_string())
    })
}

/// What a kind's settings changed, each when it changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KindChange {
    pub health: Option<bool>,
    pub appointments: Option<bool>,
    /// The kind appointments make by default: one at most.
    pub default: Option<bool>,
    pub line: Option<String>,
    pub place: Option<u32>,
}

pub fn set_kind(path: &Path, kind: &str, change: &KindChange) -> Result<Evolved, String> {
    let evolved = change_kind(path, kind, |t, _| {
        let flag = |t: &mut Table, key: &str, on: Option<bool>| match on {
            Some(true) => {
                t.insert(key, toml_edit::value(true));
            }
            Some(false) => {
                t.remove(key);
            }
            None => {}
        };
        flag(t, "health", change.health);
        flag(t, "appointments", change.appointments);
        flag(t, "default", change.default);
        if let Some(line) = &change.line {
            t.insert("line", toml_edit::value(line.replace(['\n', '\r'], " ").trim()));
        }
        if let Some(place) = change.place {
            t.insert("place", toml_edit::value(i64::from(place)));
        }
        Ok(kind.to_string())
    })?;
    if change.default == Some(true) {
        // One default: the others lose it.
        edit(path, |clients| {
            for (id, item) in sub(clients, "kind").iter_mut() {
                if id.get() != kind
                    && let Some(t) = item.as_table_mut()
                {
                    t.remove("default");
                }
            }
            Ok(evolved.clone())
        })?;
    }
    Ok(evolved)
}

/// A field added to a kind: rows written before show it empty; **no
/// default is written into them**.
pub fn add_field(path: &Path, kind: &str, name: &str, field_type: &str) -> Result<Evolved, String> {
    let name = clean(name)?;
    let field_type = kind_of_known(field_type)?;
    change_kind(path, kind, |t, version| {
        let fields = sub(t, "field");
        let id = id_from(&name, "field", &|id| fields.contains_key(id));
        let mut f = Table::new();
        f.insert("name", toml_edit::value(name.as_str()));
        f.insert("type", toml_edit::value(field_type.id()));
        f.insert("place", toml_edit::value(largest_place(fields) + 1));
        f.insert("since", toml_edit::value(i64::from(version)));
        fields.insert(&id, Item::Table(f));
        Ok(id)
    })
}

fn change_field(path: &Path, kind: &str, field: &str, change: impl FnOnce(&mut Table, u32) -> Result<(), String>) -> Result<Evolved, String> {
    change_kind(path, kind, |t, version| {
        let fields = sub(t, "field");
        change(table_mut(fields, field, "field")?, version)?;
        Ok(field.to_string())
    })
}

/// A field renamed: its id, and so every value, stays.
pub fn rename_field(path: &Path, kind: &str, field: &str, name: &str) -> Result<Evolved, String> {
    let name = clean(name)?;
    change_field(path, kind, field, |t, _| {
        t.insert("name", toml_edit::value(name.as_str()));
        Ok(())
    })
}

/// A field's type changed: values that no longer fit are kept as written
/// and read "as written before" (`file::Value::AsWrittenBefore`).
pub fn retype_field(path: &Path, kind: &str, field: &str, field_type: &str) -> Result<Evolved, String> {
    let field_type = kind_of_known(field_type)?;
    change_field(path, kind, field, |t, version| {
        t.insert("type", toml_edit::value(field_type.id()));
        t.insert("retyped", toml_edit::value(i64::from(version)));
        Ok(())
    })
}

/// A field removed: hidden, its values kept; `bring_back_field` shows them again.
pub fn remove_field(path: &Path, kind: &str, field: &str) -> Result<Evolved, String> {
    change_field(path, kind, field, |t, version| {
        t.insert("removed", toml_edit::value(i64::from(version)));
        Ok(())
    })
}

pub fn bring_back_field(path: &Path, kind: &str, field: &str) -> Result<Evolved, String> {
    change_field(path, kind, field, |t, _| {
        t.remove("removed");
        Ok(())
    })
}

/// What a field's settings changed, each when it changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldChange {
    pub health: Option<bool>,
    pub shown: Option<bool>,
    pub unit: Option<String>,
    pub place: Option<u32>,
}

pub fn set_field(path: &Path, kind: &str, field: &str, change: &FieldChange) -> Result<Evolved, String> {
    change_field(path, kind, field, |t, _| {
        for (key, on) in [("health", change.health), ("shown", change.shown)] {
            match on {
                Some(true) => {
                    t.insert(key, toml_edit::value(true));
                }
                Some(false) => {
                    t.remove(key);
                }
                None => {}
            }
        }
        if let Some(unit) = &change.unit {
            t.insert("unit", toml_edit::value(unit.replace(['\n', '\r'], " ").trim()));
        }
        if let Some(place) = change.place {
            t.insert("place", toml_edit::value(i64::from(place)));
        }
        Ok(())
    })
}

/// A choice added to a list.
pub fn add_choice(path: &Path, kind: &str, field: &str, name: &str) -> Result<Evolved, String> {
    let name = clean(name)?;
    let mut made = String::new();
    let evolved = change_field(path, kind, field, |t, _| {
        let choices = sub(t, "choice");
        let id = id_from(&name, "choice", &|id| choices.contains_key(id));
        let mut c = Table::new();
        c.insert("name", toml_edit::value(name.as_str()));
        c.insert("place", toml_edit::value(largest_place(choices) + 1));
        choices.insert(&id, Item::Table(c));
        made = id;
        Ok(())
    })?;
    Ok(Evolved { id: made, ..evolved })
}

pub fn rename_choice(path: &Path, kind: &str, field: &str, choice: &str, name: &str) -> Result<Evolved, String> {
    let name = clean(name)?;
    change_field(path, kind, field, |t, _| {
        table_mut(sub(t, "choice"), choice, "choice")?.insert("name", toml_edit::value(name.as_str()));
        Ok(())
    })
}

/// A choice archived: rows that hold it keep it, shown; it is no longer offered.
pub fn archive_choice(path: &Path, kind: &str, field: &str, choice: &str) -> Result<Evolved, String> {
    change_field(path, kind, field, |t, version| {
        table_mut(sub(t, "choice"), choice, "choice")?.insert("archived", toml_edit::value(i64::from(version)));
        Ok(())
    })
}

pub fn bring_back_choice(path: &Path, kind: &str, field: &str, choice: &str) -> Result<Evolved, String> {
    change_field(path, kind, field, |t, _| {
        table_mut(sub(t, "choice"), choice, "choice")?.remove("archived");
        Ok(())
    })
}

/// An evolution undone: the part put back as it was, the version raised
/// again (a version never goes back: rows written meanwhile keep theirs).
pub fn undo(path: &Path, evolved: &Evolved) -> Result<Evolved, String> {
    let Some((which, id)) = evolved.part.split_once('.') else { return Err("nothing to undo".into()) };
    let (key, version) = match which {
        "folder" => ("folder", "folders_version"),
        "kind" => ("kind", "kinds_version"),
        _ => return Err("nothing to undo".into()),
    };
    edit(path, |clients| {
        bump(clients, version);
        let parts = sub(clients, key);
        let now = snapshot(parts, id);
        match &evolved.before {
            Some(text) => {
                let doc: DocumentMut = text.parse().map_err(|e| format!("{e}"))?;
                let item = doc.get("x").cloned().ok_or("nothing to undo")?;
                parts.insert(id, item);
            }
            None => {
                parts.remove(id);
            }
        }
        Ok(Evolved { id: evolved.id.clone(), part: evolved.part.clone(), before: now })
    })
}

// ---------------------------------------------------------------- starts by trade

/// The starts by trade (`data/client-starts.toml`): data, never types.
const STARTS: &str = include_str!("../../data/client-starts.toml");

/// A start by trade, as offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartInfo {
    pub id: String,
    pub name: String,
    pub about: String,
    /// The options that protect it proposes: "keep-tied", "stay-here", "keeping".
    pub proposes: Vec<String>,
}

fn starts_table() -> toml::Table {
    toml::from_str::<toml::Table>(STARTS).ok().and_then(|t| t.get("start").and_then(|s| s.as_table()).cloned()).unwrap_or_default()
}

/// A text of a start in `language` ("fr"…), English when it has none.
fn words(value: Option<&toml::Value>, language: &str) -> String {
    let Some(t) = value.and_then(toml::Value::as_table) else { return String::new() };
    let lang = if language.starts_with("fr") { "fr" } else { "en" };
    t.get(lang).or_else(|| t.get("en")).and_then(toml::Value::as_str).unwrap_or("").to_string()
}

/// Every start by trade, in order, its words in `language`.
pub fn starts(language: &str) -> Vec<StartInfo> {
    let table = starts_table();
    let mut out: Vec<(i64, StartInfo)> = table
        .iter()
        .filter_map(|(id, s)| {
            let s = s.as_table()?;
            Some((
                s.get("place").and_then(toml::Value::as_integer).unwrap_or(0),
                StartInfo {
                    id: id.clone(),
                    name: words(s.get("name"), language),
                    about: words(s.get("about"), language),
                    proposes: s.get("proposes").and_then(toml::Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                },
            ))
        })
        .collect();
    out.sort_by_key(|(place, _)| *place);
    out.into_iter().map(|(_, s)| s).collect()
}

/// A start by trade written into the person's templates, its words in
/// `language`: each of its folders and kinds whose id is free, nothing over
/// what the person has. Returns what it proposes (never set here) and the
/// ids it wrote.
pub fn start_from(path: &Path, trade: &str, language: &str) -> Result<(StartInfo, Vec<String>), String> {
    let table = starts_table();
    let start = table.get(trade).and_then(toml::Value::as_table).ok_or_else(|| format!("no such start: {trade}"))?.clone();
    let info = starts(language).into_iter().find(|s| s.id == trade).ok_or("no such start")?;
    let mut written = Vec::new();
    edit(path, |clients| {
        let folders_version = bump(clients, "folders_version");
        let kinds_version = bump(clients, "kinds_version");
        let _ = folders_version;
        clients.insert("start", toml_edit::value(trade));
        if let Some(folders) = start.get("folder").and_then(toml::Value::as_table) {
            let into = sub(clients, "folder");
            for (id, f) in folders {
                if into.contains_key(id) || !super::address::valid_id(id) {
                    continue;
                }
                let mut t = Table::new();
                t.insert("name", toml_edit::value(words(f.get("name"), language)));
                t.insert("place", toml_edit::value(f.get("place").and_then(toml::Value::as_integer).unwrap_or(0) + largest_place(into)));
                if f.get("health").and_then(toml::Value::as_bool) == Some(true) {
                    t.insert("health", toml_edit::value(true));
                }
                into.insert(id, Item::Table(t));
                written.push(format!("folder.{id}"));
            }
        }
        if let Some(kinds) = start.get("kind").and_then(toml::Value::as_table) {
            let has_default = sub(clients, "kind").iter().any(|(_, k)| k.as_table().and_then(|t| t.get("default")).and_then(Item::as_bool) == Some(true));
            let into = sub(clients, "kind");
            for (id, k) in kinds {
                if into.contains_key(id) || !super::address::valid_id(id) {
                    continue;
                }
                let mut t = Table::new();
                t.insert("name", toml_edit::value(words(k.get("name"), language)));
                t.insert("place", toml_edit::value(largest_place(into) + 1));
                t.insert("since", toml_edit::value(i64::from(kinds_version)));
                for flag in ["health", "appointments"] {
                    if k.get(flag).and_then(toml::Value::as_bool) == Some(true) {
                        t.insert(flag, toml_edit::value(true));
                    }
                }
                if k.get("default").and_then(toml::Value::as_bool) == Some(true) && !has_default {
                    t.insert("default", toml_edit::value(true));
                }
                if let Some(line) = k.get("line").and_then(toml::Value::as_str) {
                    t.insert("line", toml_edit::value(line));
                }
                let mut fields = Table::new();
                for (fid, f) in k.get("field").and_then(toml::Value::as_table).into_iter().flatten() {
                    let mut ft = Table::new();
                    ft.insert("name", toml_edit::value(words(f.get("name"), language)));
                    ft.insert("type", toml_edit::value(f.get("type").and_then(toml::Value::as_str).unwrap_or("line")));
                    ft.insert("place", toml_edit::value(f.get("place").and_then(toml::Value::as_integer).unwrap_or(0)));
                    ft.insert("since", toml_edit::value(i64::from(kinds_version)));
                    for flag in ["health", "shown"] {
                        if f.get(flag).and_then(toml::Value::as_bool) == Some(true) {
                            ft.insert(flag, toml_edit::value(true));
                        }
                    }
                    fields.insert(fid, Item::Table(ft));
                }
                t.insert("field", Item::Table(fields));
                into.insert(id, Item::Table(t));
                written.push(format!("kind.{id}"));
            }
        }
        Ok(Evolved { id: trade.to_string(), part: String::new(), before: None })
    })?;
    Ok((info, written))
}

// ---------------------------------------------------------------- a file's folders

/// A folder as a file shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderView {
    pub id: String,
    pub name: String,
    pub parent: String,
    pub health: bool,
    pub state: FolderState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderState {
    /// In the template.
    Template,
    /// This file's own.
    Own,
    /// Removed from the template, kept here because it holds something: "No longer in your template".
    NoLongerInTemplate,
    /// Used by this file, unknown to this device's template: "Not in this device's template yet".
    NotInTemplateYet,
}

/// The folders a file shows: the template's (its removed ones only where
/// the file holds something), its own, and those it uses that this device
/// does not know yet. `used`: the folder ids its notes and attachments name.
pub fn folders_of(own: &BTreeMap<String, FolderDef>, settings: &Settings, used: &std::collections::BTreeSet<String>) -> Vec<FolderView> {
    let mut out = Vec::new();
    for (id, f) in settings.folders() {
        let state = if f.removed == 0 {
            FolderState::Template
        } else if used.contains(id) {
            FolderState::NoLongerInTemplate
        } else {
            continue;
        };
        out.push(FolderView { id: id.clone(), name: f.name.clone(), parent: f.parent.clone(), health: f.health, state });
    }
    for (id, f) in own {
        if !settings.folder.contains_key(id) {
            out.push(FolderView { id: id.clone(), name: f.name.clone(), parent: f.parent.clone(), health: f.health, state: FolderState::Own });
        }
    }
    for id in used.iter().filter(|id| !id.is_empty() && !settings.folder.contains_key(*id) && !own.contains_key(*id)) {
        out.push(FolderView { id: id.clone(), name: id.clone(), parent: String::new(), health: false, state: FolderState::NotInTemplateYet });
    }
    out
}
