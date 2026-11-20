// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul 0.0.12's `crates/sioul-core/src/device_said.rs` (tag v0.0.12, commit
// ef6f3ab2), its tests left out, every other line as released but one: the
// reader and the writers of `[device."<id>"]` that a device on that release
// runs. The line changed is `use crate::config::Config;`, in place of which
// stands that release's `Config` as far as its devices' lines go (the field
// as 0.0.12's config.rs declares it), so that the three readers of the
// configuration below read what 0.0.12 reads. Compiled into sioul-core's
// tests alone (`device_said::tests`), so that what that release does with a
// field it does not know stays tested whatever this Sioul's own code becomes.

//! What you said of each of your devices (docs/database.md, "Devices"): the
//! name you give it ("Work laptop"), what it is for, work, your admin,
//! leisure, any of them together, and the cells of its line you changed:
//! what comes forward on it at one of the five times (docs/areas.md, "What
//! each device is for").
//!
//! Kept in the configuration, `[device."<id>"]`, `<id>` being the device's
//! id in the sharing (sioul-sync's `share::Here::id`): `name`, `area`, and
//! under it `[device."<id>".at]`, a key per cell changed (`work`, `admin`,
//! `leisure`, `meals`, `sleep`): areas as everywhere, or `none` for
//! nothing. Only what you said: a device with no `area` is for everything,
//! as every device was before; one with no `name` is named as its system
//! names it; a time with no cell is as the device's areas say.
//! The table travels with the settings, one entry per field, so that a name
//! given on one device and an area set on another both stay, and so that
//! any device may say what each of the others is for. A Sioul that does not
//! know the table keeps it as it is: it edits the file in place, and the
//! sharing carries its fields like any other setting.
//!
//! What a device says of itself (its system's name, its kind, its build,
//! its sessions) is not here: it writes that alone, sealed, in the sharing
//! folder (sioul-sync's `devices::Entry`).
//!
//! What folds or waits by it is the rule's to say (`areas::Here::brings`),
//! which reads a device's line as `Said::line` gives it.

use crate::areas::{Area, Line, Time};
// (0.0.12: `use crate::config::Config;`.) Its `Config`, the devices' lines alone.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct Config {
    #[serde(rename = "device", default, deserialize_with = "lenient")]
    pub devices: std::collections::BTreeMap<String, Said>,
}
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// What you said of one device, each field unsaid until you say it. Words,
/// read leniently: an area with no word this Sioul knows is unsaid, never
/// an error that would stop the configuration from reading; a field a newer
/// Sioul wrote is passed over, and kept in the file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Said {
    /// The name you give it; unsaid, its system's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What it is for: "work", "admin", "leisure", joined by "+"; unsaid, everything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    /// The cells you changed, by their time ("work", "meals"): the areas
    /// that come forward on the device then ("work+leisure"), or the word
    /// for nothing (`areas::NOTHING`). Read leniently, as the rest: a time
    /// or words this Sioul does not know, or a value that is no text, are
    /// passed over and kept in the file.
    #[serde(default, deserialize_with = "cells", skip_serializing_if = "BTreeMap::is_empty")]
    pub at: BTreeMap<String, String>,
    /// When the person last said what it is for or changed a cell (Unix
    /// seconds, by the clock of the device it was said on), written with
    /// them (`CHANGED`): the keeper of the notices judges an idle device that
    /// shares your settings by this line once it is later than what that
    /// device last said of itself (sioul-sync's `dispatch::Known`; review of
    /// phase 2, N3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed: Option<i64>,
    /// What this phone lets ring (`calls_line`, `[device."<id>".calls]`),
    /// when it says something this Sioul reads; none: the phone keeps the
    /// shared call screening, as before.
    #[serde(default, deserialize_with = "calls", skip_serializing_if = "Option::is_none")]
    pub calls: Option<crate::calls_line::Said>,
}

/// The key of `Said::changed` in `[device."<id>"]`.
pub const CHANGED: &str = "changed";

/// The cells of `[device."<id>".at]`: each key whose value is a text; any
/// other value, and an `at` that is no table, read as nothing said, never as
/// an error that would stop the configuration from reading.
fn cells<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error> {
    Ok(toml::Value::deserialize(deserializer).map(|value| cells_of(&value)).unwrap_or_default())
}

/// `[device."<id>".calls]`, leniently (`calls_line::read`): anything that does not read is no line.
fn calls<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<crate::calls_line::Said>, D::Error> {
    Ok(toml::Value::deserialize(deserializer).ok().as_ref().and_then(crate::calls_line::read))
}

/// The same from a value already read: the keys of a table whose values are texts.
fn cells_of(value: &toml::Value) -> BTreeMap<String, String> {
    let Some(at) = value.as_table() else { return BTreeMap::new() };
    at.iter().filter_map(|(time, cell)| cell.as_str().map(|cell| (time.clone(), cell.to_string()))).collect()
}

impl Said {
    /// The name you gave it, when it says something.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref().map(str::trim).filter(|name| !name.is_empty())
    }

    /// What you said it is for; none while unsaid, or said in no word that reads.
    pub fn area(&self) -> Option<Area> {
        self.area.as_deref().and_then(Area::parse)
    }

    /// The cell you changed for one of the five times; none while it is as
    /// the device's areas say, or said in no word that reads.
    pub fn cell(&self, time: Time) -> Option<Area> {
        self.at.iter().find(|(at, _)| cell_time(at) == Some(time)).and_then(|(_, cell)| Line::read_cell(cell))
    }

    /// The device's line, as the rule reads it (`areas::Line`): what it is
    /// for, and each cell you changed.
    pub fn line(&self) -> Line {
        Time::STATES.into_iter().fold(Line { area: self.area(), at: [None; 5] }, |line, time| line.with_cell(time, self.cell(time)))
    }
}

/// A cell as someone says it (the window's boxes, the command line): the
/// device's areas' own again ("" or "usual"), nothing (`areas::NOTHING`), or
/// areas joined by "+"; none for words that say none of these.
pub fn said_cell(value: &str) -> Option<Option<Area>> {
    match value.trim().to_lowercase().as_str() {
        "" | "usual" | "habituel" => Some(None),
        other => Line::read_cell(other).map(Some),
    }
}

/// One of the five times, as a cell's key names it ("work", "meals"); none
/// for any other word (working and admin hours at once have no cell of their own).
pub fn cell_time(word: &str) -> Option<Time> {
    Time::parse(word).filter(|time| Time::STATES.contains(time))
}

/// `[device]` as the configuration reads it (`Config::devices`), leniently:
/// a value of the wrong type (a number for `name`, a list for `area`, a
/// device's line that is not a table, `[device]` itself not a table),
/// brought by a hand edit or by another device through the sharing, is
/// passed over, its device's other fields and the rest of the file still
/// read. A strict reading would make the whole configuration unreadable on
/// every device the sharing carries it to.
pub fn lenient<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, Said>, D::Error> {
    // Whatever TOML holds reads as a value; one that does not, as nothing said.
    Ok(toml::Value::deserialize(deserializer).map(|value| read(&value)).unwrap_or_default())
}

/// Each device's line that is a table, its fields that are words, and the
/// cells of its `at` that are words (`cells_of`).
fn read(value: &toml::Value) -> BTreeMap<String, Said> {
    let Some(devices) = value.as_table() else { return BTreeMap::new() };
    let word = |line: &toml::Table, field: &str| line.get(field).and_then(toml::Value::as_str).map(str::to_string);
    let cells = |line: &toml::Table| line.get("at").map(cells_of).unwrap_or_default();
    let changed = |line: &toml::Table| line.get(CHANGED).and_then(toml::Value::as_integer);
    devices.iter().filter_map(|(id, line)| line.as_table().map(|line| (id.clone(), Said { name: word(line, "name"), area: word(line, "area"), at: cells(line), changed: changed(line), calls: line.get(crate::calls_line::KEY).and_then(crate::calls_line::read) }))).collect()
}

/// The longest name a device is given, in characters: a line of a card.
pub const NAME_LENGTH: usize = 60;

/// What you said the device `id` is for; none while unsaid.
pub fn said_area(config: &Config, id: &str) -> Option<Area> {
    config.devices.get(id).and_then(Said::area)
}

/// What the device `id` is for: as you said, else everything, as every
/// device was before any was said.
pub fn area_of(config: &Config, id: &str) -> Area {
    said_area(config, id).unwrap_or(Area::ALL)
}

/// The name you gave the device `id`; none while unsaid.
pub fn name_of<'a>(config: &'a Config, id: &str) -> Option<&'a str> {
    config.devices.get(id).and_then(Said::name)
}

/// The name you give the device `id`, written into the configuration in
/// place, its comments and every other line kept; "" takes it out (its
/// system's name again). One line, `NAME_LENGTH` characters at most.
pub fn set_name(path: &Path, id: &str, name: &str) -> Result<(), String> {
    let name: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name: String = name.chars().take(NAME_LENGTH).collect();
    set(path, id, "name", (!name.is_empty()).then_some(name))
}

/// What the device `id` is for, written into the configuration in place:
/// "work", "admin", "leisure", joined by "+"; "" takes it out (everything
/// again). Words that name no area are refused, nothing written.
pub fn set_area(path: &Path, id: &str, area: &str) -> Result<(), String> {
    let area = match area.trim() {
        "" => None,
        said => Some(Area::parse(said).ok_or_else(|| format!("{said}: work, admin, leisure, joined by +"))?.id()),
    };
    set(path, id, "area", area)
}

/// The cell of the device `id` for `time` (one of the five), written into
/// the configuration in place, `[device."<id>".at]`: the areas that come
/// forward on it then ("work+leisure"), an empty area being your word for
/// nothing (`areas::NOTHING`); `None` takes the cell out, as the device's
/// areas say again. The `at` table goes with its last cell, the device's
/// table with its last field.
pub fn set_cell(path: &Path, id: &str, time: Time, areas: Option<Area>) -> Result<(), String> {
    if !Time::STATES.contains(&time) {
        return Err(format!("{}: work, admin, leisure, meals, sleep", time.id()));
    }
    let id = id.trim();
    if id.is_empty() || id.chars().any(char::is_control) {
        return Err("a device's id is needed".to_string());
    }
    let value = areas.map(Line::cell_id);
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let there = doc.get("device").and_then(toml_edit::Item::as_table_like).and_then(|devices| devices.get(id)).and_then(toml_edit::Item::as_table_like).and_then(|device| device.get("at")).and_then(toml_edit::Item::as_table_like).is_some_and(|at| at.iter().any(|(key, _)| cell_time(key) == Some(time)));
        if value.is_none() && !there {
            return Ok(());
        }
        // A line that is not a table (nothing reads it: `lenient`) gives way to one, as in `set`.
        if doc.get("device").is_none_or(|item| !item.is_table_like()) {
            let mut devices = toml_edit::Table::new();
            devices.set_implicit(true);
            doc.insert("device", toml_edit::Item::Table(devices));
        }
        let devices = doc.get_mut("device").and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{}: device is not a table", path.display()))?;
        if devices.get(id).is_none_or(|line| !line.is_table_like()) {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            devices.insert(id, toml_edit::Item::Table(table));
        }
        let device = devices.get_mut(id).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: not a table"))?;
        if device.get("at").is_none_or(|at| !at.is_table_like()) {
            device.insert("at", toml_edit::Item::Table(toml_edit::Table::new()));
        }
        let at = device.get_mut("at").and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: at is not a table"))?;
        // The cell under the word it was written with (an older word for the time, kept), else the time's own.
        let key = at.iter().map(|(key, _)| key.to_string()).find(|key| cell_time(key) == Some(time)).unwrap_or_else(|| time.id().to_string());
        match value {
            Some(text) => match at.get_mut(&key) {
                Some(slot) => *slot = toml_edit::value(text),
                None => {
                    at.insert(&key, toml_edit::value(text));
                }
            },
            None => {
                at.remove(&key);
            }
        }
        if at.is_empty() {
            device.remove("at");
        }
        if device.is_empty() {
            devices.remove(id);
        }
        // A cell changed: when, for the keeper of the notices.
        stamp(devices, id, true);
        if devices.is_empty() {
            doc.remove("device");
        }
        crate::config::write_document(path, &doc)
    })
}

/// One field of `[device."<id>"]` written, or taken out with `None`; the
/// table goes with its last field, `[device]` with its last table. Read,
/// changed and written under the file's lock, which the sharing takes too.
/// A line that is not a table (nothing reads it: `lenient`) gives way to one.
fn set(path: &Path, id: &str, field: &str, value: Option<String>) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() || id.chars().any(char::is_control) {
        return Err("a device's id is needed".to_string());
    }
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        if doc.get("device").is_none_or(|item| !item.is_table_like()) {
            if value.is_none() {
                return Ok(());
            }
            let mut devices = toml_edit::Table::new();
            devices.set_implicit(true);
            doc.insert("device", toml_edit::Item::Table(devices));
        }
        let devices = doc.get_mut("device").and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{}: device is not a table", path.display()))?;
        match value {
            Some(text) => {
                if devices.get(id).is_none_or(|line| !line.is_table_like()) {
                    let mut table = toml_edit::Table::new();
                    table.set_implicit(false);
                    devices.insert(id, toml_edit::Item::Table(table));
                }
                let table = devices.get_mut(id).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: not a table"))?;
                // A field already there keeps its place and the comment above it.
                match table.get_mut(field) {
                    Some(slot) => *slot = toml_edit::value(text),
                    None => {
                        table.insert(field, toml_edit::value(text));
                    }
                }
            }
            None => {
                if let Some(table) = devices.get_mut(id).and_then(toml_edit::Item::as_table_like_mut) {
                    table.remove(field);
                    if table.is_empty() {
                        devices.remove(id);
                    }
                }
            }
        }
        // What it is for said: when, for the keeper of the notices; a name is no change of it.
        stamp(devices, id, field != "name");
        if devices.is_empty() {
            doc.remove("device");
        }
        crate::config::write_document(path, &doc)
    })
}

/// The device `id`'s line after a change: `changed` set to now when
/// `said` (its area or a cell changed), and the line taken out when nothing
/// but that time is left in it.
fn stamp(devices: &mut dyn toml_edit::TableLike, id: &str, said: bool) {
    let Some(line) = devices.get_mut(id).and_then(toml_edit::Item::as_table_like_mut) else { return };
    if line.iter().all(|(key, _)| key == CHANGED) {
        devices.remove(id);
        return;
    }
    if said {
        let now = jiff::Timestamp::now().as_second();
        match line.get_mut(CHANGED) {
            Some(slot) => *slot = toml_edit::value(now),
            None => {
                line.insert(CHANGED, toml_edit::value(now));
            }
        }
    }
}

