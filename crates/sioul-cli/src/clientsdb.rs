// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A client's records written out as databases (docs/clients.md, "Out in
//! clear"): `sioul store export --clients` writes, beside each client's
//! folder of TOML rows, `records.sqlite` (a table per kind of record, a
//! column per field, named as the template names them) and one CSV per kind
//! under `records/`, which any spreadsheet opens. Built from the store
//! through the clients' door, into a folder outside Sioul's: "once
//! decrypted, they stay .md and databases".

use sioul_core::clients::door::Door;
use sioul_core::clients::file::{self, Row};
use sioul_core::clients::record;
use sioul_core::clients::template::{FieldDef, FieldType, KindDef, Settings};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What was written: clients with records, tables in all, and the rows
/// left out because they do not open (said with the export's counts; the
/// TOML beside holds what did open, `sioul store check` names the rest).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    pub clients: usize,
    pub tables: usize,
    pub left_out: usize,
}

/// What a cell names by its name rather than its id: the supplies' list, a
/// client's attachments (id → its name and its place in the export).
#[derive(Debug, Default, Clone)]
pub struct Names {
    pub supplies: BTreeMap<String, String>,
    pub attachments: BTreeMap<String, String>,
}

impl Names {
    fn supply(&self, id: &str) -> String {
        self.supplies.get(id).cloned().unwrap_or_else(|| id.to_string())
    }
}

/// A supply used, as a cell says it: its name, × its quantity.
fn used(names: &Names, id: &str, quantity: f64) -> String {
    format!("{} × {}", names.supply(id), quantity)
}

/// One table: its name, its columns' names, its rows' cells.
struct Table {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<rusqlite::types::Value>>,
}

/// A name made unique among those taken ("Treatment", "Treatment (2)"), never empty.
fn unique(taken: &mut BTreeSet<String>, name: &str, fallback: &str) -> String {
    let base = if name.trim().is_empty() { fallback.to_string() } else { name.trim().to_string() };
    let mut name = base.clone();
    let mut n = 2;
    while !taken.insert(name.to_lowercase()) {
        name = format!("{base} ({n})");
        n += 1;
    }
    name
}

/// A field's value as a cell: a choice by its name, a list of choices by
/// their names, a date and a time as written, anything with parts as JSON.
fn cell(def: Option<&FieldDef>, value: &toml::Value, names: &Names) -> rusqlite::types::Value {
    use rusqlite::types::Value as V;
    let choice_name = |id: &str| def.and_then(|d| d.choice.get(id)).map_or_else(|| id.to_string(), |c| c.name.clone());
    match (def.map(|d| &d.kind), value) {
        // A supply by its name and quantity; an attachment by its name and where the export put it.
        (Some(FieldType::Supply), toml::Value::Table(t)) => V::Text(used(names, t.get("id").and_then(toml::Value::as_str).unwrap_or_default(), t.get("quantity").and_then(|q| q.as_float().or_else(|| q.as_integer().map(|n| n as f64))).unwrap_or(0.0))),
        (Some(FieldType::Attachment), toml::Value::String(id)) => V::Text(names.attachments.get(id).cloned().unwrap_or_else(|| id.clone())),
        (Some(FieldType::Choice), toml::Value::String(id)) => V::Text(choice_name(id)),
        (Some(FieldType::Choices), toml::Value::Array(ids)) => V::Text(ids.iter().map(|v| v.as_str().map_or_else(|| v.to_string(), choice_name)).collect::<Vec<_>>().join(", ")),
        (_, toml::Value::String(s)) => V::Text(s.clone()),
        (_, toml::Value::Integer(n)) => V::Integer(*n),
        (_, toml::Value::Float(f)) => V::Real(*f),
        (_, toml::Value::Boolean(b)) => V::Integer(i64::from(*b)),
        (_, toml::Value::Datetime(d)) => V::Text(d.to_string()),
        (_, other) => V::Text(serde_json::to_string(other).unwrap_or_default()),
    }
}

/// A kind's rows as a table: `id`, `when`, `health`, then each field of the
/// template in its place (removed ones too, when a row holds them), then the
/// fields a row holds that this template does not know, by their ids.
fn table(kind: &str, def: Option<&KindDef>, rows: &[Row], taken: &mut BTreeSet<String>, names: &Names) -> Table {
    let name = unique(taken, def.map_or(kind, |k| k.name.as_str()), kind);
    let held: BTreeSet<&String> = rows.iter().flat_map(|r| r.fields.keys()).collect();
    let mut fields: Vec<(&String, Option<&FieldDef>)> = def.map(|k| k.field.iter().filter(|(id, f)| f.removed == 0 || held.contains(id)).map(|(id, f)| (id, Some(f))).collect()).unwrap_or_default();
    fields.sort_by_key(|(id, f)| (f.map_or(u32::MAX, |f| f.place), (*id).clone()));
    for id in held {
        if !fields.iter().any(|(known, _)| *known == id) {
            fields.push((id, None));
        }
    }
    let mut columns_taken: BTreeSet<String> = ["id", "when", "health", "supplies"].iter().map(|s| s.to_string()).collect();
    let mut columns: Vec<String> = vec!["id".into(), "when".into(), "health".into(), "supplies".into()];
    columns.extend(fields.iter().map(|(id, f)| unique(&mut columns_taken, f.map_or(id.as_str(), |f| f.name.as_str()), id)));
    let rows = rows
        .iter()
        .map(|r| {
            // What the row used (a treatment's products), by their names: "Cream × 2; Mask × 1".
            let supplies = r.supplies.iter().map(|s| used(names, &s.id, s.quantity)).collect::<Vec<_>>().join("; ");
            let mut cells = vec![rusqlite::types::Value::Text(r.id.clone()), rusqlite::types::Value::Text(r.when.clone()), rusqlite::types::Value::Integer(i64::from(r.health)), if supplies.is_empty() { rusqlite::types::Value::Null } else { rusqlite::types::Value::Text(supplies) }];
            cells.extend(fields.iter().map(|(id, f)| r.fields.get(*id).map_or(rusqlite::types::Value::Null, |v| cell(*f, v, names))));
            cells
        })
        .collect();
    Table { name, columns, rows }
}

/// An identifier quoted for SQL: double quotes, a quote inside doubled.
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A cell as CSV writes it (RFC 4180): quoted when it holds a comma, a quote or a line end.
fn csv_cell(value: &rusqlite::types::Value) -> String {
    use rusqlite::types::Value as V;
    let text = match value {
        V::Null => String::new(),
        V::Integer(n) => n.to_string(),
        V::Real(f) => f.to_string(),
        V::Text(s) => s.clone(),
        V::Blob(_) => String::new(),
    };
    if text.contains([',', '"', '\n', '\r']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text }
}

/// A file name safe on every system, never empty.
fn file_name(name: &str, fallback: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c }).collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    if cleaned.is_empty() { fallback.to_string() } else { cleaned }
}

/// Each client's records, from `door`, written as `<out>/<id>/records.sqlite`
/// and `<out>/<id>/records/<kind>.csv` (`out`: the export's `data/clients/`).
/// A client without records gets neither; a row that does not open is left
/// out, as the export leaves out a file that does not open.
pub fn write(door: &dyn Door, out: &Path, settings: &Settings, supplies: &BTreeMap<String, String>) -> Result<Written, String> {
    let mut written = Written::default();
    for id in record::ids(door).map_err(|e| e.to_string())? {
        let heads = file::rows(door, &id).map_err(|e| e.to_string())?;
        written.left_out += heads.damaged;
        let kinds: BTreeSet<String> = heads.items.iter().map(|h| h.kind.clone()).collect();
        if kinds.is_empty() {
            continue;
        }
        // Each attachment by its name and its place in the export, beside the database.
        let attachments = file::attachments(door, &id).map_err(|e| e.to_string())?.items.into_iter().map(|a| (a.id.clone(), format!("{} (files/{})", a.name, a.id))).collect();
        let names = Names { supplies: supplies.clone(), attachments };
        let mut taken = BTreeSet::new();
        let mut tables = Vec::new();
        for kind in &kinds {
            let rows = file::rows_of_kind(door, &id, kind).map_err(|e| e.to_string())?;
            written.left_out += rows.damaged;
            tables.push(table(kind, settings.kind.get(kind), &rows.items, &mut taken, &names));
        }
        let folder = out.join(&id);
        crate::files::create_dir_all(folder.join("records")).map_err(|e| format!("{}: {e}", folder.display()))?;
        let database = folder.join("records.sqlite");
        let mut db = rusqlite::Connection::open(&database).map_err(|e| format!("{}: {e}", database.display()))?;
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let mut csv_names = BTreeSet::new();
        for t in &tables {
            let columns: Vec<String> = t.columns.iter().map(|c| quoted(c)).collect();
            tx.execute(&format!("CREATE TABLE {} ({})", quoted(&t.name), columns.join(", ")), []).map_err(|e| e.to_string())?;
            let marks = vec!["?"; t.columns.len()].join(", ");
            let mut insert = tx.prepare(&format!("INSERT INTO {} VALUES ({marks})", quoted(&t.name))).map_err(|e| e.to_string())?;
            for row in &t.rows {
                insert.execute(rusqlite::params_from_iter(row.iter())).map_err(|e| e.to_string())?;
            }
            let mut csv = t.columns.iter().map(|c| csv_cell(&rusqlite::types::Value::Text(c.clone()))).collect::<Vec<_>>().join(",");
            csv.push_str("\r\n");
            for row in &t.rows {
                csv.push_str(&row.iter().map(csv_cell).collect::<Vec<_>>().join(","));
                csv.push_str("\r\n");
            }
            let name = unique(&mut csv_names, &file_name(&t.name, "records"), "records");
            let path = folder.join("records").join(format!("{name}.csv"));
            crate::files::write(&path, csv).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        written.clients += 1;
        written.tables += tables.len();
    }
    Ok(written)
}

/// The settings' kinds and fields, as the export's own copy of the configuration says them.
pub fn settings_of(config: &Path) -> Settings {
    sioul_core::config::Config::load(config).map(|c| c.clients).unwrap_or_default()
}

/// The supplies' names by id, from the list where this profile keeps it
/// (its records, or the notes folder before the move); none when it does not read.
pub fn supply_names(config: &Path) -> BTreeMap<String, String> {
    let Some(root) = sioul_core::config::Config::load(config).ok().and_then(|c| c.records_root()) else { return BTreeMap::new() };
    sioul_core::supplies::List::load(&root).map(|list| list.supplies.iter().map(|s| (s.id.clone(), s.name_shown().to_string())).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::clients::door::Memory;
    use sioul_core::clients::template::ChoiceDef;

    /// An invented client's two kinds of records, written as a database
    /// that opens and holds each row's fields by their names and values, and
    /// as CSV tables.
    #[test]
    fn a_client_s_records_open_as_a_database_and_as_tables() {
        let door = Memory::new();
        let mut settings = Settings { kinds_version: 1, ..Settings::default() };
        let mut treatment = KindDef { name: "Treatment".into(), place: 1, ..KindDef::default() };
        treatment.field.insert("care".into(), FieldDef { name: "Care given".into(), kind: FieldType::Line, place: 1, ..FieldDef::default() });
        treatment.field.insert("minutes".into(), FieldDef { name: "Minutes".into(), kind: FieldType::Number, place: 2, ..FieldDef::default() });
        let mut skin = FieldDef { name: "Skin, today".into(), kind: FieldType::Choice, place: 3, ..FieldDef::default() };
        skin.choice.insert("dry".into(), ChoiceDef { name: "Dry".into(), place: 1, ..ChoiceDef::default() });
        treatment.field.insert("skin".into(), skin);
        treatment.field.insert("cream".into(), FieldDef { name: "Cream".into(), kind: FieldType::Supply, place: 4, ..FieldDef::default() });
        treatment.field.insert("photo".into(), FieldDef { name: "Photo".into(), kind: FieldType::Attachment, place: 5, ..FieldDef::default() });
        settings.kind.insert("treatment".into(), treatment);
        let mut visit = KindDef { name: "Visit".into(), place: 2, ..KindDef::default() };
        visit.field.insert("note".into(), FieldDef { name: "Note".into(), kind: FieldType::Text, place: 1, ..FieldDef::default() });
        settings.kind.insert("visit".into(), visit);
        let client = record::make(&door, "Studio Exemple", &[], record::Billing::default()).unwrap();
        let fields = |pairs: &[(&str, toml::Value)]| pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<BTreeMap<_, _>>();
        let photo = file::attach(&door, &client.id, "Before.jpg", "", "", "image/jpeg", b"jpeg", &settings).unwrap();
        let cream: toml::Value = toml::from_str::<toml::Table>("v = { id = \"cream-1\", quantity = 2 }").unwrap()["v"].clone();
        let row = file::add_row(&door, &client.id, "treatment", "2026-11-20T10:00:00+01:00", fields(&[("care", "Facial, gentle".into()), ("minutes", 45.into()), ("skin", "dry".into()), ("cream", cream), ("photo", photo.id.clone().into())]), &settings).unwrap();
        let used = vec![sioul_core::clients::file::SupplyUse { id: "mask-1".into(), quantity: 1.0 }];
        file::change_row(&door, &client.id, "treatment", &row.id, &file::RowChange { supplies: Some(used), ..Default::default() }, &settings).unwrap();
        file::add_row(&door, &client.id, "visit", "2026-11-21", fields(&[("note", "Asked \"when\" next".into())]), &settings).unwrap();
        let out = std::env::temp_dir().join(format!("sioul-clientsdb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let supplies: BTreeMap<String, String> = [("cream-1".to_string(), "Rich cream".to_string()), ("mask-1".to_string(), "Clay mask".to_string())].into_iter().collect();
        assert_eq!(write(&door, &out, &settings, &supplies).unwrap(), Written { clients: 1, tables: 2, left_out: 0 });
        let db = rusqlite::Connection::open(out.join(&client.id).join("records.sqlite")).unwrap();
        let (care, minutes, skin): (String, f64, String) = db.query_row("SELECT \"Care given\", \"Minutes\", \"Skin, today\" FROM \"Treatment\"", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!((care.as_str(), minutes, skin.as_str()), ("Facial, gentle", 45.0, "Dry"), "each field by its name, a choice by its name");
        let (used, cream, photo_cell): (String, String, String) = db.query_row("SELECT \"supplies\", \"Cream\", \"Photo\" FROM \"Treatment\"", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(used, "Clay mask × 1", "what the row used, by its name");
        assert_eq!(cream, "Rich cream × 2", "a supply field by the supply's name");
        assert_eq!(photo_cell, format!("Before.jpg (files/{})", photo.id), "an attachment by its name and its place in the export");
        let note: String = db.query_row("SELECT \"Note\" FROM \"Visit\"", [], |r| r.get(0)).unwrap();
        assert_eq!(note, "Asked \"when\" next");
        let csv = std::fs::read_to_string(out.join(&client.id).join("records").join("Treatment.csv")).unwrap();
        assert_eq!(csv.lines().next(), Some("id,when,health,supplies,Care given,Minutes,\"Skin, today\",Cream,Photo"));
        assert!(csv.contains("Clay mask × 1,\"Facial, gentle\",45,Dry,Rich cream × 2,"), "{csv}");
        // A row that does not open is left out, and counted.
        let rows = file::rows_of_kind(&door, &client.id, "visit").unwrap().items;
        door.damage(&format!("data/clients/{}/db/visit/{}.toml", client.id, rows[0].id));
        let again = std::env::temp_dir().join(format!("sioul-clientsdb-left-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&again);
        std::fs::create_dir_all(&again).unwrap();
        assert_eq!(write(&door, &again, &settings, &supplies).unwrap().left_out, 1);
        let _ = std::fs::remove_dir_all(&again);
        let _ = std::fs::remove_dir_all(&out);
    }
}
