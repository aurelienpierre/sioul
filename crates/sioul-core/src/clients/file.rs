// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A client's file: a folder and a set of databases (docs/clients.md, "The
//! file"). A timeless description, notes in folders, one database per kind
//! of record (a row a file), and attachments. **Every part is a file of its
//! own** in the store, so that a damaged file costs one row, one note or one
//! attachment:
//!
//! ```text
//! data/clients/<id>/description.md
//! data/clients/<id>/notes/<note>.md            front matter: folder, row, ties, title, health, made
//! data/clients/<id>/rows.toml                  each row's kind and time: rebuilt from the rows
//! data/clients/<id>/db/<kind>/<row>.toml       one row
//! data/clients/<id>/files/<file>               an attachment's bytes
//! data/clients/<id>/files/<file>.meta.toml     its name, folder, row, type, size, health mark
//! data/clients/<id>/files/<file>.text          a PDF's text, once read (a later branch reads it)
//! ```
//!
//! Until a description, a note, a row or an attachment is written, a client
//! has no file (`has_file`).

use super::door::{Door, Expect, folder_of, name_of};
use super::template::{FieldDef, FieldType, KindDef, Settings};
use super::{Error, Read};
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use toml_edit::{DocumentMut, Item};

fn client_folder(id: &str) -> Result<String, Error> {
    if super::address::valid_id(id) { Ok(super::record::folder(id)) } else { Err(Error::Refused("id")) }
}

fn checked(id: &str) -> Result<&str, Error> {
    if super::address::valid_id(id) { Ok(id) } else { Err(Error::Refused("id")) }
}

/// The description's path.
pub fn description_path(id: &str) -> String {
    format!("{}/description.md", super::record::folder(id))
}

/// Whether a client has a file: a description, a note, a row or an attachment.
pub fn has_file(door: &dyn Door, id: &str) -> Result<bool, Error> {
    let folder = client_folder(id)?;
    if door.read(&description_path(id))?.is_some() {
        return Ok(true);
    }
    Ok(door.folders(&folder)?.iter().any(|f| {
        let rest = &f[folder.len()..];
        rest.starts_with("/notes") || rest.starts_with("/db/") || rest.starts_with("/files")
    }))
}

/// The client's record notes the folder template's version its file was made with, once.
fn file_used(door: &dyn Door, id: &str, settings: &Settings) -> Result<(), Error> {
    if settings.folders_version == 0 {
        return Ok(());
    }
    match super::record::read(door, id)? {
        Some(c) if c.folders_version == 0 => {
            super::record::change(door, id, &super::record::Change { folders_version: Some(settings.folders_version), ..Default::default() })?;
            Ok(())
        }
        Some(_) => Ok(()),
        None => Err(Error::NotFound),
    }
}

fn exists(door: &dyn Door, id: &str) -> Result<(), Error> {
    super::record::read(door, id)?.map(|_| ()).ok_or(Error::NotFound)
}

// ---------------------------------------------------------------- the description

/// The description: one Markdown note, timeless.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Description {
    pub text: String,
    pub generation: u64,
}

pub fn description(door: &dyn Door, id: &str) -> Result<Option<Description>, Error> {
    client_folder(id)?;
    Ok(door.read_text(&description_path(id))?.map(|(text, generation)| Description { text, generation }))
}

/// The description written (`expect`: the generation read, so that a change
/// made elsewhere meanwhile is `Error::Changed`, never written over); an
/// empty text takes it out.
pub fn write_description(door: &dyn Door, id: &str, text: &str, expect: Option<u64>, settings: &Settings) -> Result<u64, Error> {
    exists(door, id)?;
    let path = description_path(id);
    if text.trim().is_empty() {
        door.remove(&path)?;
        crate::changes::said(crate::changes::Kind::Clients);
        return Ok(0);
    }
    let generation = door.write(&path, text.as_bytes(), expect.map_or(Expect::Any, Expect::Generation))?;
    file_used(door, id, settings)?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(generation)
}

// ---------------------------------------------------------------- notes

/// A note of the file, its front matter read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Note {
    pub id: String,
    /// Its folder's id; "" at the file's root.
    pub folder: String,
    /// The row it is about: `"<kind>/<row>"`, "" none.
    pub row: String,
    /// What else it is tied to, by address.
    pub ties: Vec<String>,
    pub title: String,
    pub health: bool,
    /// When it was made, RFC 3339; "" when not said.
    pub made: String,
    /// Its Markdown body.
    pub text: String,
    pub generation: u64,
}

/// A note's path.
pub fn note_path(id: &str, note: &str) -> String {
    format!("{}/notes/{note}.md", super::record::folder(id))
}

/// A note read from its text.
pub fn read_note(id: &str, text: &str, generation: u64) -> Note {
    let (front, start) = crate::notes::front_matter(text);
    let one = |key: &str| front.get(key).and_then(|v| v.first()).cloned().unwrap_or_default();
    let body: String = text.lines().skip(start).collect::<Vec<_>>().join("\n");
    Note {
        id: id.to_string(),
        folder: one("folder"),
        row: one("row"),
        ties: front.get("ties").cloned().unwrap_or_default(),
        title: one("title"),
        health: one("health") == "true",
        made: one("made"),
        text: if text.ends_with('\n') && !body.is_empty() { format!("{body}\n") } else { body },
        generation,
    }
}

/// A note's text as written: its front matter, then its body.
pub fn note_text(note: &Note) -> String {
    let line = |s: &str| s.replace(['\n', '\r'], " ").trim().to_string();
    let mut out = String::from("---\n");
    if !note.title.trim().is_empty() {
        out.push_str(&format!("title: \"{}\"\n", line(&note.title)));
    }
    if !note.folder.is_empty() {
        out.push_str(&format!("folder: {}\n", line(&note.folder)));
    }
    if !note.row.is_empty() {
        out.push_str(&format!("row: {}\n", line(&note.row)));
    }
    if !note.ties.is_empty() {
        out.push_str("ties:\n");
        for tie in &note.ties {
            out.push_str(&format!("  - {}\n", line(tie)));
        }
    }
    if note.health {
        out.push_str("health: true\n");
    }
    if !note.made.is_empty() {
        out.push_str(&format!("made: {}\n", line(&note.made)));
    }
    out.push_str("---\n");
    out.push_str(&note.text);
    out
}

/// A note without its body: what a list shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteHead {
    pub id: String,
    pub folder: String,
    pub row: String,
    pub ties: Vec<String>,
    pub title: String,
    pub health: bool,
    pub made: String,
}

/// Every note of a file, without its body; those that do not open counted.
pub fn notes(door: &dyn Door, id: &str) -> Result<Read<Vec<NoteHead>>, Error> {
    let folder = format!("{}/notes", client_folder(id)?);
    let mut out: Read<Vec<NoteHead>> = Read::default();
    for path in door.list(&folder)? {
        let Some(note) = name_of(&path).strip_suffix(".md") else { continue };
        match door.read_text(&path) {
            Ok(Some((text, generation))) => {
                let n = read_note(note, &text, generation);
                out.items.push(NoteHead { id: n.id, folder: n.folder, row: n.row, ties: n.ties, title: n.title, health: n.health, made: n.made });
            }
            Ok(None) => {}
            Err(Error::Damaged(_)) => out.damaged += 1,
            Err(e) => return Err(e),
        }
    }
    out.items.sort_by(|a, b| b.made.cmp(&a.made).then(a.id.cmp(&b.id)));
    Ok(out)
}

pub fn note(door: &dyn Door, id: &str, note: &str) -> Result<Option<Note>, Error> {
    client_folder(id)?;
    checked(note)?;
    Ok(door.read_text(&note_path(id, note))?.map(|(text, generation)| read_note(note, &text, generation)))
}

/// A note written: a new one when its id is empty (made now), else over
/// the one read (its generation expected). The note as written.
pub fn write_note(door: &dyn Door, id: &str, note: &Note, settings: &Settings) -> Result<Note, Error> {
    exists(door, id)?;
    let mut note = note.clone();
    let expect = if note.id.is_empty() {
        note.id = crate::ids::new();
        if note.made.is_empty() {
            note.made = jiff::Timestamp::now().to_string();
        }
        Expect::Absent
    } else if note.generation == 0 {
        Expect::Any
    } else {
        Expect::Generation(note.generation)
    };
    checked(&note.id)?;
    note.generation = door.write(&note_path(id, &note.id), note_text(&note).as_bytes(), expect)?;
    file_used(door, id, settings)?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(note)
}

pub fn remove_note(door: &dyn Door, id: &str, note: &str) -> Result<bool, Error> {
    client_folder(id)?;
    let removed = door.remove(&note_path(id, checked(note)?))?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(removed)
}

// ---------------------------------------------------------------- rows

/// A supply a row used.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SupplyUse {
    pub id: String,
    pub quantity: f64,
}

/// One record of a kind.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub id: String,
    pub kind: String,
    /// The kinds' template version it was written with.
    pub schema: u32,
    /// Its day, or its day and time, RFC 3339.
    pub when: String,
    pub health: bool,
    /// Its fields' values by field id, as written.
    pub fields: BTreeMap<String, toml::Value>,
    pub supplies: Vec<SupplyUse>,
    pub generation: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct RowFile {
    id: String,
    kind: String,
    schema: u32,
    #[serde(deserialize_with = "lenient_when")]
    when: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    health: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    supplies: Vec<SupplyUse>,
    fields: BTreeMap<String, toml::Value>,
}

/// A time written as a text or as a TOML date-time.
fn lenient_when<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let found: toml::Value = Deserialize::deserialize(d)?;
    Ok(match found {
        toml::Value::String(s) => s,
        toml::Value::Datetime(dt) => dt.to_string(),
        _ => String::new(),
    })
}

/// A row's path.
pub fn row_path(id: &str, kind: &str, row: &str) -> String {
    format!("{}/db/{kind}/{row}.toml", super::record::folder(id))
}

/// The rows' list's path.
pub fn rows_path(id: &str) -> String {
    format!("{}/rows.toml", super::record::folder(id))
}

/// What a field's value reads as, through its template.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Nothing written (a field added after the row: no default is ever written).
    Empty,
    Line(String),
    Text(String),
    Number { value: f64, unit: String },
    Amount(crate::money::Money),
    Day(Date),
    /// Minutes.
    Duration(i64),
    /// A list's choices, by id: one for a choice, any for several.
    Choices(Vec<String>),
    YesNo(bool),
    Supply(SupplyUse),
    /// An attachment of the file, by id.
    Attachment(String),
    /// A value that no longer fits its field's type: kept as written, "as written before".
    AsWrittenBefore(String),
    /// A field this device's template does not know, or of a type it does not: "fields of a newer template".
    Unknown(toml::Value),
}

/// An amount as written: a number of units ("65.00", "65,5", 65).
fn amount(raw: &toml::Value) -> Option<crate::money::Money> {
    match raw {
        toml::Value::Integer(n) => Some(crate::money::Money(n.checked_mul(100)?)),
        toml::Value::Float(f) if f.is_finite() => Some(crate::money::Money::from_units(*f)),
        toml::Value::String(s) => s.trim().replace(',', ".").parse::<f64>().ok().filter(|f| f.is_finite()).map(crate::money::Money::from_units),
        _ => None,
    }
}

fn as_text(raw: &toml::Value) -> String {
    match raw {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Whether a choice's value names a choice this device's list lacks: an id
/// (as Sioul makes them), never free text.
fn newer_choice(def: &FieldDef, value: &str) -> bool {
    super::address::valid_id(value) && !def.choice.contains_key(value)
}

impl Row {
    /// Field `field`'s value, read through its kind's template.
    pub fn value(&self, kind: Option<&KindDef>, field: &str) -> Value {
        let Some(raw) = self.fields.get(field) else { return Value::Empty };
        let Some(def) = kind.and_then(|k| k.field.get(field)) else { return Value::Unknown(raw.clone()) };
        let unfit = || Value::AsWrittenBefore(as_text(raw));
        match &def.kind {
            FieldType::Line => raw.as_str().map_or_else(unfit, |s| Value::Line(s.to_string())),
            FieldType::Text => raw.as_str().map_or_else(unfit, |s| Value::Text(s.to_string())),
            FieldType::Number => match raw {
                toml::Value::Integer(n) => Value::Number { value: *n as f64, unit: def.unit.clone() },
                toml::Value::Float(f) => Value::Number { value: *f, unit: def.unit.clone() },
                toml::Value::String(s) => s.trim().replace(',', ".").parse::<f64>().ok().filter(|f| f.is_finite()).map_or_else(unfit, |value| Value::Number { value, unit: def.unit.clone() }),
                _ => unfit(),
            },
            FieldType::Amount => amount(raw).map_or_else(unfit, Value::Amount),
            FieldType::Day => raw.as_str().and_then(|s| s.trim().get(..10)?.parse().ok()).or_else(|| raw.as_datetime().and_then(|d| d.to_string().get(..10)?.parse().ok())).map_or_else(unfit, Value::Day),
            FieldType::Duration => raw.as_integer().map_or_else(unfit, Value::Duration),
            // A text that names no choice of the list (a line before its type changed) reads as written before.
            FieldType::Choice => raw.as_str().filter(|c| def.choice.contains_key(*c)).map_or_else(unfit, |s| Value::Choices(vec![s.to_string()])),
            FieldType::Choices => raw.as_array().and_then(|a| a.iter().map(|v| v.as_str().filter(|c| def.choice.contains_key(*c)).map(str::to_string)).collect::<Option<Vec<_>>>()).map_or_else(unfit, Value::Choices),
            FieldType::YesNo => raw.as_bool().map_or_else(unfit, Value::YesNo),
            FieldType::Supply => raw.clone().try_into::<SupplyUse>().map_or_else(|_| unfit(), Value::Supply),
            FieldType::Attachment => raw.as_str().filter(|s| super::address::valid_id(s)).map_or_else(unfit, |s| Value::Attachment(s.to_string())),
            FieldType::Unknown(_) => Value::Unknown(raw.clone()),
        }
    }

    /// Whether this device may only read it: it holds a field this device's
    /// template lacks, a field of a type it does not know, or a choice its
    /// list lacks. Never because of a version number: the template's version
    /// travels as a setting and may go back when two devices change it at
    /// once. Rows are written field by field, so nothing is lost either way;
    /// this only keeps a form from offering what it cannot show.
    pub fn read_only(&self, settings: &Settings) -> bool {
        let kind = settings.kind.get(&self.kind);
        self.fields.iter().any(|(f, raw)| match kind.and_then(|k| k.field.get(f)) {
            None => true,
            Some(def) => match &def.kind {
                FieldType::Unknown(_) => true,
                // A choice's id the list lacks: a newer template's. Free text (a line
                // before the field became a choice) reads "as written before" and the
                // row stays editable (the second review, E).
                FieldType::Choice => raw.as_str().is_some_and(|c| newer_choice(def, c)),
                FieldType::Choices => raw.as_array().is_some_and(|a| a.iter().any(|c| c.as_str().is_some_and(|c| newer_choice(def, c)))),
                _ => false,
            },
        })
    }

    /// Its address.
    pub fn uri(&self, client: &str) -> String {
        super::address::row_uri(client, &self.kind, &self.id)
    }

    fn of(file: RowFile, generation: u64) -> Row {
        Row { id: file.id, kind: file.kind, schema: file.schema, when: file.when, health: file.health, fields: file.fields, supplies: file.supplies, generation }
    }
}

/// A row read, checked against where it lies.
fn parse_row(path: &str, kind: &str, row: &str, text: &str, generation: u64) -> Result<Row, Error> {
    let file: RowFile = toml::from_str(text).map_err(|_| Error::Damaged(path.to_string()))?;
    if file.id != row || file.kind != kind {
        return Err(Error::Damaged(path.to_string()));
    }
    Ok(Row::of(file, generation))
}

/// A row without its fields: what a list shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RowHead {
    #[serde(skip)]
    pub id: String,
    pub kind: String,
    pub when: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct RowsFile {
    row: BTreeMap<String, RowHead>,
}

pub fn row(door: &dyn Door, id: &str, kind: &str, row: &str) -> Result<Option<Row>, Error> {
    client_folder(id)?;
    let path = row_path(id, checked(kind)?, checked(row)?);
    match door.read_text(&path)? {
        None => Ok(None),
        Some((text, generation)) => parse_row(&path, kind, row, &text, generation).map(Some),
    }
}

/// Every row of a file, without its fields, newest first. The rows' list
/// gives their kind and time; a row it lacks (arrived from another device
/// before the list) is read, one it names that is gone is passed over, and
/// a list that does not open is rebuilt from the rows (said by `rebuilt`).
pub fn rows(door: &dyn Door, id: &str) -> Result<Read<Vec<RowHead>>, Error> {
    let folder = client_folder(id)?;
    let listed: Option<RowsFile> = match door.read_text(&rows_path(id)) {
        Ok(Some((text, _))) => toml::from_str(&text).ok(),
        Ok(None) | Err(Error::Damaged(_)) => None,
        Err(e) => return Err(e),
    };
    let listed = listed.map(|f| f.row).unwrap_or_default();
    let mut out: Read<Vec<RowHead>> = Read::default();
    let db = format!("{folder}/db");
    for kind_folder in door.folders(&db)? {
        let kind = name_of(&kind_folder).to_string();
        if folder_of(&kind_folder) != db || !super::address::valid_id(&kind) {
            continue;
        }
        for path in door.list(&kind_folder)? {
            let Some(row_id) = name_of(&path).strip_suffix(".toml") else { continue };
            match listed.get(row_id).filter(|h| h.kind == kind) {
                Some(head) => out.items.push(RowHead { id: row_id.to_string(), ..head.clone() }),
                None => match self::row(door, id, &kind, row_id) {
                    Ok(Some(r)) => out.items.push(RowHead { id: r.id, kind: r.kind, when: r.when }),
                    Ok(None) => {}
                    Err(Error::Damaged(_)) => out.damaged += 1,
                    Err(e) => return Err(e),
                },
            }
        }
    }
    out.items.sort_by(|a, b| b.when.cmp(&a.when).then(a.id.cmp(&b.id)));
    Ok(out)
}

/// The rows' list rebuilt from the rows themselves.
pub fn rebuild_rows(door: &dyn Door, id: &str) -> Result<usize, Error> {
    let mut file = RowsFile::default();
    // Read every row, not the list being rebuilt.
    let folder = client_folder(id)?;
    let db = format!("{folder}/db");
    for kind_folder in door.folders(&db)? {
        let kind = name_of(&kind_folder).to_string();
        for path in door.list(&kind_folder)? {
            let Some(row_id) = name_of(&path).strip_suffix(".toml") else { continue };
            if let Ok(Some(r)) = self::row(door, id, &kind, row_id) {
                file.row.insert(r.id.clone(), RowHead { id: String::new(), kind: r.kind, when: r.when });
            }
        }
    }
    let n = file.row.len();
    door.write(&rows_path(id), toml::to_string(&file).unwrap_or_default().as_bytes(), Expect::Any)?;
    Ok(n)
}

/// The rows' list with one row said (Some) or taken out (None).
fn list_row(door: &dyn Door, id: &str, row: &str, head: Option<&RowHead>) -> Result<(), Error> {
    let path = rows_path(id);
    door.update(&path, &mut |now| {
        let text = now.and_then(|b| std::str::from_utf8(b).ok()).unwrap_or("");
        let mut doc: DocumentMut = text.parse().unwrap_or_default();
        if !doc.contains_key("row") {
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            doc["row"] = Item::Table(t);
        }
        let table = doc["row"].as_table_mut().ok_or(Error::Damaged(path.clone()))?;
        match head {
            Some(h) => {
                let mut one = toml_edit::Table::new();
                one.insert("kind", toml_edit::value(h.kind.as_str()));
                one.insert("when", toml_edit::value(h.when.as_str()));
                table.insert(row, Item::Table(one));
            }
            None => {
                table.remove(row);
            }
        }
        Ok(Some(doc.to_string().into_bytes()))
    })?;
    Ok(())
}

/// Every row of one kind of a file, those that do not open counted.
pub fn rows_of_kind(door: &dyn Door, id: &str, kind: &str) -> Result<Read<Vec<Row>>, Error> {
    let folder = format!("{}/db/{}", client_folder(id)?, checked(kind)?);
    let mut out: Read<Vec<Row>> = Read::default();
    for path in door.list(&folder)? {
        let Some(row_id) = name_of(&path).strip_suffix(".toml") else { continue };
        match self::row(door, id, kind, row_id) {
            Ok(Some(r)) => out.items.push(r),
            Ok(None) => {}
            Err(Error::Damaged(_)) => out.damaged += 1,
            Err(e) => return Err(e),
        }
    }
    out.items.sort_by(|a, b| b.when.cmp(&a.when).then(a.id.cmp(&b.id)));
    Ok(out)
}

/// Every client's rows of one kind (what Count reads), each with its client's id.
pub fn rows_everywhere(door: &dyn Door, kind: &str) -> Result<Read<Vec<(String, Row)>>, Error> {
    let mut out: Read<Vec<(String, Row)>> = Read::default();
    for id in super::record::ids(door)? {
        let read = rows_of_kind(door, &id, kind)?;
        out.damaged += read.damaged;
        out.items.extend(read.items.into_iter().map(|r| (id.clone(), r)));
    }
    Ok(out)
}

/// A new row of `kind`, its fields as given (those left empty not written).
/// A row with no field filled is not kept (`Error::Empty`). A kind the
/// template does not hold, or holds archived, is refused.
pub fn add_row(door: &dyn Door, id: &str, kind: &str, when: &str, fields: BTreeMap<String, toml::Value>, settings: &Settings) -> Result<Row, Error> {
    exists(door, id)?;
    let def = settings.kind.get(checked(kind)?).filter(|k| k.archived == 0).ok_or(Error::NotFound)?;
    let fields: BTreeMap<String, toml::Value> = fields.into_iter().filter(|(k, v)| def.field.contains_key(k) && !blank(v)).collect();
    if fields.is_empty() {
        return Err(Error::Empty);
    }
    let health = def.health || fields.keys().any(|f| def.field.get(f).is_some_and(|d| d.health));
    let file = RowFile { id: crate::ids::new(), kind: kind.to_string(), schema: settings.kinds_version, when: when.trim().to_string(), health, supplies: Vec::new(), fields };
    let text = toml::to_string(&file).map_err(|_| Error::Refused("row"))?;
    let generation = door.write(&row_path(id, kind, &file.id), text.as_bytes(), Expect::Absent)?;
    list_row(door, id, &file.id, Some(&RowHead { id: String::new(), kind: file.kind.clone(), when: file.when.clone() }))?;
    file_used(door, id, settings)?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(Row::of(file, generation))
}

fn blank(v: &toml::Value) -> bool {
    match v {
        toml::Value::String(s) => s.trim().is_empty(),
        toml::Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

/// What a row's form changed: each field written (Some) or emptied (None),
/// its time, its health mark.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RowChange {
    pub fields: Vec<(String, Option<toml::Value>)>,
    pub when: Option<String>,
    pub health: Option<bool>,
    pub supplies: Option<Vec<SupplyUse>>,
}

/// A row changed field by field: every key it does not change stays as
/// written (by a newer device too), and a change made elsewhere meanwhile is
/// kept, the row read again. A row this device may only read is refused.
pub fn change_row(door: &dyn Door, id: &str, kind: &str, row_id: &str, change: &RowChange, settings: &Settings) -> Result<Row, Error> {
    client_folder(id)?;
    let path = row_path(id, checked(kind)?, checked(row_id)?);
    let mut found = false;
    let mut refused = false;
    door.update(&path, &mut |now| {
        let Some(now) = now else { return Ok(None) };
        found = true;
        let text = std::str::from_utf8(now).map_err(|_| Error::Damaged(path.clone()))?;
        let row = parse_row(&path, kind, row_id, text, 0)?;
        if row.read_only(settings) {
            refused = true;
            return Ok(None);
        }
        let mut doc: DocumentMut = text.parse().map_err(|_| Error::Damaged(path.clone()))?;
        if !doc.contains_key("fields") {
            doc["fields"] = Item::Table(toml_edit::Table::new());
        }
        let fields = doc["fields"].as_table_mut().ok_or(Error::Damaged(path.clone()))?;
        for (field, value) in &change.fields {
            if !super::address::valid_id(field) {
                continue;
            }
            match value.as_ref().filter(|v| !blank(v)) {
                Some(v) => {
                    // Written as the value it is; a table (a supply used) as an inline table.
                    let item: toml_edit::Value = toml::to_string(&toml::Table::from_iter([("v".to_string(), v.clone())])).ok().and_then(|t| t.parse::<DocumentMut>().ok()).and_then(|d| d.get("v").cloned()).and_then(|i| i.into_value().ok()).ok_or(Error::Refused("value"))?;
                    fields.insert(field, Item::Value(item));
                }
                None => {
                    fields.remove(field);
                }
            }
        }
        if let Some(when) = &change.when {
            doc["when"] = toml_edit::value(when.trim());
        }
        if let Some(h) = change.health {
            if h {
                doc["health"] = toml_edit::value(true);
            } else {
                doc.remove("health");
            }
        }
        if let Some(supplies) = &change.supplies {
            let mut array = toml_edit::Array::new();
            for s in supplies {
                let mut one = toml_edit::InlineTable::new();
                one.insert("id", s.id.as_str().into());
                one.insert("quantity", s.quantity.into());
                array.push(one);
            }
            doc["supplies"] = toml_edit::value(array);
        }
        // Written now with this device's template.
        doc["schema"] = toml_edit::value(i64::from(settings.kinds_version.max(row.schema)));
        Ok(Some(doc.to_string().into_bytes()))
    })?;
    if refused {
        return Err(Error::NewerTemplate);
    }
    if !found {
        return Err(Error::NotFound);
    }
    let row = self::row(door, id, kind, row_id)?.ok_or(Error::NotFound)?;
    if let Some(when) = &change.when {
        list_row(door, id, row_id, Some(&RowHead { id: String::new(), kind: kind.to_string(), when: when.trim().to_string() }))?;
    }
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(row)
}

/// A row taken out (its earlier versions kept by the store), and out of the
/// rows' list. Its ties move to the client (`ties::row_removed`).
pub fn remove_row(door: &dyn Door, id: &str, kind: &str, row_id: &str) -> Result<bool, Error> {
    client_folder(id)?;
    let removed = door.remove(&row_path(id, checked(kind)?, checked(row_id)?))?;
    list_row(door, id, row_id, None)?;
    super::ties::row_removed(door, id, &super::address::row_uri(id, kind, row_id))?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(removed)
}

// ---------------------------------------------------------------- attachments

/// An attachment of the file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Attachment {
    #[serde(skip)]
    pub id: String,
    /// As named: shown, never a file name on the disk.
    pub name: String,
    pub folder: String,
    /// `"<kind>/<row>"`, "" none.
    pub row: String,
    /// Its media type: "application/pdf".
    pub media: String,
    pub size: u64,
    pub health: bool,
    /// RFC 3339.
    pub made: String,
    /// A PDF's text was read and kept.
    #[serde(skip)]
    pub has_text: bool,
    /// What it is a copy of, when it has a source elsewhere (a message's
    /// address, for a mail's attachment): its bytes keep no earlier versions.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
}

fn files_folder(id: &str) -> String {
    format!("{}/files", super::record::folder(id))
}

/// An attachment's bytes' path, its description's, its text's.
pub fn attachment_paths(id: &str, file: &str) -> (String, String, String) {
    let at = format!("{}/{file}", files_folder(id));
    (at.clone(), format!("{at}.meta.toml"), format!("{at}.text"))
}

/// Every attachment of a file, those whose description does not open counted.
pub fn attachments(door: &dyn Door, id: &str) -> Result<Read<Vec<Attachment>>, Error> {
    let folder = format!("{}/files", client_folder(id)?);
    let all = door.list(&folder)?;
    let texts: std::collections::BTreeSet<&str> = all.iter().filter_map(|p| name_of(p).strip_suffix(".text")).collect();
    let mut out: Read<Vec<Attachment>> = Read::default();
    for path in &all {
        let Some(file) = name_of(path).strip_suffix(".meta.toml") else { continue };
        match door.read_text(path) {
            Ok(Some((text, _))) => match toml::from_str::<Attachment>(&text) {
                Ok(a) => out.items.push(Attachment { id: file.to_string(), has_text: texts.contains(file), ..a }),
                Err(_) => out.damaged += 1,
            },
            Ok(None) => {}
            Err(Error::Damaged(_)) => out.damaged += 1,
            Err(e) => return Err(e),
        }
    }
    out.items.sort_by(|a, b| b.made.cmp(&a.made).then(a.id.cmp(&b.id)));
    Ok(out)
}

/// An attachment added: its bytes, then its description. Its name stays
/// inside the seal: on the disk it is an id. Its bytes keep their earlier versions.
#[allow(clippy::too_many_arguments)]
pub fn attach(door: &dyn Door, id: &str, name: &str, folder: &str, row: &str, media: &str, bytes: &[u8], settings: &Settings) -> Result<Attachment, Error> {
    attach_from(door, id, name, folder, row, media, bytes, "", settings)
}

/// A copy of a message's attachment added (`source`: the message's
/// address): it never changes and its source is the message, so its bytes
/// keep no earlier versions (`Door::write_unversioned`).
#[allow(clippy::too_many_arguments)]
pub fn attach_copy(door: &dyn Door, id: &str, name: &str, folder: &str, row: &str, media: &str, bytes: &[u8], source: &str, settings: &Settings) -> Result<Attachment, Error> {
    if source.trim().is_empty() {
        return Err(Error::Refused("a copy names its source"));
    }
    attach_from(door, id, name, folder, row, media, bytes, source, settings)
}

#[allow(clippy::too_many_arguments)]
fn attach_from(door: &dyn Door, id: &str, name: &str, folder: &str, row: &str, media: &str, bytes: &[u8], source: &str, settings: &Settings) -> Result<Attachment, Error> {
    exists(door, id)?;
    let file = crate::ids::new();
    let (at, meta, _) = attachment_paths(id, &file);
    if source.is_empty() {
        door.write(&at, bytes, Expect::Absent)?;
    } else {
        door.write_unversioned(&at, bytes, Expect::Absent)?;
    }
    let attachment = Attachment {
        id: file,
        name: name.replace(['\n', '\r'], " ").trim().to_string(),
        folder: folder.to_string(),
        row: row.to_string(),
        media: media.to_string(),
        size: bytes.len() as u64,
        health: settings.folder.get(folder).is_some_and(|f| f.health),
        made: jiff::Timestamp::now().to_string(),
        has_text: false,
        source: source.replace(['\n', '\r'], " ").trim().to_string(),
    };
    door.write(&meta, toml::to_string(&attachment).map_err(|_| Error::Refused("attachment"))?.as_bytes(), Expect::Absent)?;
    file_used(door, id, settings)?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(attachment)
}

pub fn attachment_bytes(door: &dyn Door, id: &str, file: &str) -> Result<Vec<u8>, Error> {
    client_folder(id)?;
    let (at, ..) = attachment_paths(id, checked(file)?);
    door.read(&at)?.map(|o| o.bytes).ok_or(Error::NotFound)
}

/// A PDF's text kept beside its attachment (read by a later branch).
pub fn set_pdf_text(door: &dyn Door, id: &str, file: &str, text: &str) -> Result<(), Error> {
    client_folder(id)?;
    let (.., at) = attachment_paths(id, checked(file)?);
    door.write(&at, text.as_bytes(), Expect::Any)?;
    Ok(())
}

pub fn pdf_text(door: &dyn Door, id: &str, file: &str) -> Result<Option<String>, Error> {
    client_folder(id)?;
    let (.., at) = attachment_paths(id, checked(file)?);
    Ok(door.read_text(&at)?.map(|(t, _)| t))
}

/// An attachment's description changed (its name, folder, row or health mark).
pub fn change_attachment(door: &dyn Door, id: &str, file: &str, change: &dyn Fn(&mut Attachment)) -> Result<Attachment, Error> {
    client_folder(id)?;
    let (_, meta, _) = attachment_paths(id, checked(file)?);
    let mut out = None;
    door.update(&meta, &mut |now| {
        let Some(now) = now else { return Ok(None) };
        let text = std::str::from_utf8(now).map_err(|_| Error::Damaged(meta.clone()))?;
        let mut a: Attachment = toml::from_str(text).map_err(|_| Error::Damaged(meta.clone()))?;
        change(&mut a);
        a.name = a.name.replace(['\n', '\r'], " ").trim().to_string();
        out = Some(Attachment { id: file.to_string(), ..a.clone() });
        Ok(Some(toml::to_string(&a).map_err(|_| Error::Refused("attachment"))?.into_bytes()))
    })?;
    crate::changes::said(crate::changes::Kind::Clients);
    out.ok_or(Error::NotFound)
}

/// An attachment taken out: its bytes, its description and its text.
pub fn remove_attachment(door: &dyn Door, id: &str, file: &str) -> Result<bool, Error> {
    client_folder(id)?;
    let (at, meta, text) = attachment_paths(id, checked(file)?);
    let removed = door.remove(&at)?;
    door.remove(&meta)?;
    door.remove(&text)?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(removed)
}

/// Move its contents…: every note and attachment of a file in folder
/// `from` (a folder no longer in the template, say) put in folder `to`
/// ("" the file's root). How many moved.
pub fn move_folder_contents(door: &dyn Door, id: &str, from: &str, to: &str, settings: &Settings) -> Result<usize, Error> {
    let mut moved = 0;
    for head in notes(door, id)?.items.into_iter().filter(|n| n.folder == from) {
        if let Some(mut n) = note(door, id, &head.id)? {
            n.folder = to.to_string();
            write_note(door, id, &n, settings)?;
            moved += 1;
        }
    }
    for a in attachments(door, id)?.items.into_iter().filter(|a| a.folder == from) {
        change_attachment(door, id, &a.id, &|a| a.folder = to.to_string())?;
        moved += 1;
    }
    Ok(moved)
}

/// A health mark set on a note, a row or an attachment of a file.
pub fn set_health(door: &dyn Door, address: &super::address::Address, health: bool, settings: &Settings) -> Result<(), Error> {
    use super::address::Address as A;
    match address {
        A::Note { client, note } => {
            let mut n = self::note(door, client, note)?.ok_or(Error::NotFound)?;
            n.health = health;
            write_note(door, client, &n, settings).map(|_| ())
        }
        A::Row { client, kind, row } => change_row(door, client, kind, row, &RowChange { health: Some(health), ..Default::default() }, settings).map(|_| ()),
        A::File { client, file } => change_attachment(door, client, file, &|a| a.health = health).map(|_| ()),
        _ => Err(Error::Refused("a health mark goes on a note, a row or an attachment")),
    }
}

/// Whether anything of a file holds a health mark: a note, a row, an
/// attachment, or a part of the template its contents use (a kind, a field
/// filled, a folder holding something).
///
/// **Fails closed**: a note, an attachment or a row that does not open
/// cannot be told free of a health mark, so it counts as one (what a health
/// mark protects is then protected: an invoice named by its number alone).
pub fn holds_health(door: &dyn Door, id: &str, settings: &Settings) -> Result<bool, Error> {
    let notes = notes(door, id)?;
    if notes.damaged > 0 || notes.items.iter().any(|n| n.health || settings.folder.get(&n.folder).is_some_and(|f| f.health)) {
        return Ok(true);
    }
    let attachments = attachments(door, id)?;
    if attachments.damaged > 0 || attachments.items.iter().any(|a| a.health || settings.folder.get(&a.folder).is_some_and(|f| f.health)) {
        return Ok(true);
    }
    let rows = rows(door, id)?;
    if rows.damaged > 0 {
        return Ok(true);
    }
    for head in rows.items {
        let kind = settings.kind.get(&head.kind);
        if kind.is_some_and(|k| k.health) {
            return Ok(true);
        }
        match row(door, id, &head.kind, &head.id) {
            Ok(Some(r)) if r.health || r.fields.keys().any(|f| kind.and_then(|k| k.field.get(f)).is_some_and(|d| d.health)) => return Ok(true),
            Ok(_) => {}
            Err(_) => return Ok(true),
        }
    }
    Ok(false)
}
