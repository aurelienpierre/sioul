// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

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
use crate::config::Config;
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
}

/// The cells of `[device."<id>".at]`: each key whose value is a text; any
/// other value, and an `at` that is no table, read as nothing said, never as
/// an error that would stop the configuration from reading.
fn cells<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error> {
    Ok(match toml::Value::deserialize(deserializer) {
        Ok(toml::Value::Table(table)) => table.into_iter().filter_map(|(time, cell)| cell.as_str().map(|cell| (time, cell.to_string()))).collect(),
        _ => BTreeMap::new(),
    })
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
        if !doc.contains_key("device") {
            let mut devices = toml_edit::Table::new();
            devices.set_implicit(true);
            doc.insert("device", toml_edit::Item::Table(devices));
        }
        let devices = doc.get_mut("device").and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{}: device is not a table", path.display()))?;
        if !devices.contains_key(id) {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            devices.insert(id, toml_edit::Item::Table(table));
        }
        let device = devices.get_mut(id).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: not a table"))?;
        if !device.contains_key("at") {
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
        if devices.is_empty() {
            doc.remove("device");
        }
        crate::config::write_document(path, &doc)
    })
}

/// One field of `[device."<id>"]` written, or taken out with `None`; the
/// table goes with its last field, `[device]` with its last table. Read,
/// changed and written under the file's lock, which the sharing takes too.
fn set(path: &Path, id: &str, field: &str, value: Option<String>) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() || id.chars().any(char::is_control) {
        return Err("a device's id is needed".to_string());
    }
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        if value.is_none() && !doc.contains_key("device") {
            return Ok(());
        }
        if !doc.contains_key("device") {
            let mut devices = toml_edit::Table::new();
            devices.set_implicit(true);
            doc.insert("device", toml_edit::Item::Table(devices));
        }
        let devices = doc.get_mut("device").and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{}: device is not a table", path.display()))?;
        match value {
            Some(text) => {
                if !devices.contains_key(id) {
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
        if devices.is_empty() {
            doc.remove("device");
        }
        crate::config::write_document(path, &doc)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESK: &str = "0d0f3a52-6f0c-4f4e-9b62-2f0f6f1f7a11";
    const PHONE: &str = "7b1c9e04-52a8-4c0d-8f0e-93c1b2a4d5e6";

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-device-said-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// What you said of a device reads from `[device."<id>"]`: its name, what
    /// it is for; unsaid, no name and everything; an area in no word this
    /// Sioul knows is unsaid, and what a newer Sioul wrote beside is passed
    /// over, the cells it knows read all the same.
    #[test]
    fn what_you_said_of_a_device_reads_from_the_settings() {
        let config: Config = toml::from_str(&format!("language = \"en\"\n\n[device.\"{DESK}\"]\nname = \"Work laptop\"\narea = \"work\"\n\n[device.\"{PHONE}\"]\narea = \"admin+leisure\"\n")).unwrap();
        assert_eq!(name_of(&config, DESK), Some("Work laptop"));
        assert_eq!((said_area(&config, DESK), area_of(&config, DESK)), (Some(Area::WORK), Area::WORK));
        assert_eq!((name_of(&config, PHONE), area_of(&config, PHONE)), (None, Area::PERSONAL));
        // A device nothing was said of: no name, everything.
        assert_eq!((name_of(&config, "another"), said_area(&config, "another"), area_of(&config, "another")), (None, None, Area::ALL));
        // No table at all, as every configuration before.
        let older: Config = toml::from_str("language = \"en\"\n").unwrap();
        assert!(older.devices.is_empty() && area_of(&older, DESK) == Area::ALL);
        // A word that names no area, an empty name: unsaid. A newer Sioul's field beside: passed over; its cell read.
        let newer: Config = toml::from_str(&format!("[device.\"{DESK}\"]\nname = \"  \"\narea = \"elsewhere\"\nfollows = 2\n\n[device.\"{DESK}\".at]\nwork = \"work+leisure\"\n")).unwrap();
        assert_eq!((name_of(&newer, DESK), said_area(&newer, DESK), area_of(&newer, DESK)), (None, None, Area::ALL));
        assert_eq!(newer.devices[DESK].cell(Time::Work), Some(Area::WORK.with(Area::LEISURE)));
    }

    /// The cells you changed read from `[device."<id>".at]`, one key per
    /// time: areas, or the word for nothing; a time with no cell is as the
    /// device's areas say. A time or words this Sioul does not know, a value
    /// that is no text, an `at` that is no table: passed over, never an
    /// error that stops the configuration from reading.
    #[test]
    fn the_cells_you_changed_read_from_the_settings() {
        let config: Config = toml::from_str(&format!(
            "[device.\"{PHONE}\"]\narea = \"admin+leisure\"\n\n[device.\"{PHONE}\".at]\nwork = \"work+leisure\"\nmeals = \"none\"\nsleep = 3\nweekend = \"work\"\nadmin = \"elsewhere\"\n\n[device.\"{DESK}\"]\nat = \"work\"\n"
        ))
        .unwrap();
        let phone = &config.devices[PHONE];
        assert_eq!((phone.cell(Time::Work), phone.cell(Time::Meals)), (Some(Area::WORK.with(Area::LEISURE)), Some(Area::default())));
        assert_eq!((phone.cell(Time::Sleep), phone.cell(Time::Admin), phone.cell(Time::Leisure)), (None, None, None), "no text, no word that reads, unsaid");
        let line = phone.line();
        assert_eq!(line, Line::for_area(Area::PERSONAL).with_cell(Time::Work, Some(Area::WORK.with(Area::LEISURE))).with_cell(Time::Meals, Some(Area::default())));
        let week = crate::areas::Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        assert_eq!(Time::STATES.map(|time| line.cell(time, week).id()), ["work+leisure", "admin", "leisure", "", "leisure"]);
        // An `at` that is no table: nothing said, and the rest of the file reads.
        assert!(config.devices[DESK].at.is_empty() && config.devices[DESK].line().says_nothing());
        // Nothing said of a device: a line that says nothing.
        assert!(Said::default().line().says_nothing());
        assert_eq!((cell_time("work"), cell_time("Repas"), cell_time("work+admin"), cell_time("any"), cell_time("weekend")), (Some(Time::Work), Some(Time::Meals), None, None, None));
        // As someone says a cell: its areas' own again, nothing, areas; anything else, refused.
        assert_eq!((said_cell(""), said_cell(" Usual "), said_cell("none"), said_cell("work+leisure"), said_cell("often")), (Some(None), Some(None), Some(Some(Area::default())), Some(Some(Area::WORK.with(Area::LEISURE))), None));
    }

    /// A cell is written in place under `[device."<id>".at]`, read back, and
    /// taken out: nothing is your word for nothing, kept as such; the `at`
    /// table goes with its last cell, the device's with its last field; a
    /// time that is none of the five is refused; the name and the area
    /// written beside keep the cells.
    #[test]
    fn a_cell_is_written_in_place_and_taken_out() {
        let dir = scratch("cells");
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\nlanguage = \"fr\"\n\n[agenda]\nbetween = 10\n").unwrap();
        // A cell alone, on a device nothing else is said of.
        set_cell(&path, PHONE, Time::Work, Some(Area::WORK.with(Area::LEISURE))).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("work = \"work+leisure\"") && text.contains("# mine") && text.contains("between = 10"), "{text}");
        let read = Config::load(&path).unwrap();
        assert_eq!((read.devices[PHONE].cell(Time::Work), said_area(&read, PHONE), name_of(&read, PHONE)), (Some(Area::WORK.with(Area::LEISURE)), None, None));
        // Nothing at meals; the area and the name said beside.
        set_cell(&path, PHONE, Time::Meals, Some(Area::default())).unwrap();
        set_area(&path, PHONE, "admin+leisure").unwrap();
        set_name(&path, PHONE, "Pocket").unwrap();
        let read = Config::load(&path).unwrap();
        assert_eq!(read.devices[PHONE].line(), Line::for_area(Area::PERSONAL).with_cell(Time::Work, Some(Area::WORK.with(Area::LEISURE))).with_cell(Time::Meals, Some(Area::default())));
        assert!(std::fs::read_to_string(&path).unwrap().contains("meals = \"none\""));
        assert_eq!(name_of(&read, PHONE), Some("Pocket"));
        // Changed again in place; refused for a time that is none of the five, nothing written.
        set_cell(&path, PHONE, Time::Work, Some(Area::LEISURE)).unwrap();
        assert_eq!(Config::load(&path).unwrap().devices[PHONE].cell(Time::Work), Some(Area::LEISURE));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(set_cell(&path, PHONE, Time::Any, Some(Area::WORK)).is_err() && set_cell(&path, PHONE, Time::Several(Area::MIXED), None).is_err() && set_cell(&path, " ", Time::Work, None).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        // As its areas say again: the cell goes; taking out one never said writes nothing.
        set_cell(&path, PHONE, Time::Work, None).unwrap();
        assert_eq!(Config::load(&path).unwrap().devices[PHONE].line(), Line::for_area(Area::PERSONAL).with_cell(Time::Meals, Some(Area::default())));
        let text = std::fs::read_to_string(&path).unwrap();
        set_cell(&path, PHONE, Time::Sleep, None).unwrap();
        set_cell(&path, DESK, Time::Sleep, None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        // The last cell out: the `at` table goes, the name and the area stay; they out, the device's table and `[device]` go.
        set_cell(&path, PHONE, Time::Meals, None).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains(".at]") && text.contains("name = \"Pocket\"") && text.contains("area = \"admin+leisure\""), "{text}");
        set_cell(&path, PHONE, Time::Sleep, Some(Area::default())).unwrap();
        set_name(&path, PHONE, "").unwrap();
        set_area(&path, PHONE, "").unwrap();
        assert!(Config::load(&path).unwrap().devices[PHONE].cell(Time::Sleep) == Some(Area::default()), "its cell keeps the table");
        set_cell(&path, PHONE, Time::Sleep, None).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("device") && text.contains("# mine") && text.contains("between = 10"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A device's name and what it is for are written in place, one field at
    /// a time, comments and the other lines kept, and read back; "" takes a
    /// field out, the last one its table, the last table `[device]`; an area
    /// that names none is refused, nothing written; a name is one line.
    #[test]
    fn a_device_s_name_and_area_are_written_in_place() {
        let dir = scratch("written");
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\nlanguage = \"fr\"\ndevice_kind = \"tablet\"\n\n[agenda]\nbetween = 10\n").unwrap();
        set_name(&path, DESK, "  Work\n laptop ").unwrap();
        set_area(&path, DESK, "work").unwrap();
        set_area(&path, PHONE, "leisure, admin").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# mine") && text.contains("language = \"fr\"") && text.contains("device_kind = \"tablet\"") && text.contains("between = 10"), "{text}");
        assert!(text.contains(&format!("[device.{DESK}]")) || text.contains(&format!("[device.\"{DESK}\"]")), "{text}");
        let read = Config::load(&path).unwrap();
        assert_eq!((name_of(&read, DESK), said_area(&read, DESK)), (Some("Work laptop"), Some(Area::WORK)));
        assert_eq!(read.devices[PHONE].area.as_deref(), Some("admin+leisure"), "written in the files' order of words");
        assert_eq!(read.device_kind(), Some(crate::device_kind::DeviceKind::Tablet), "this device's kind is another key, untouched");
        // Refused: nothing written.
        assert!(set_area(&path, DESK, "elsewhere").is_err());
        assert!(set_name(&path, " ", "Nobody").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        // Every area said is kept as said, not read as unsaid.
        set_area(&path, PHONE, "work+admin+leisure").unwrap();
        assert_eq!(said_area(&Config::load(&path).unwrap(), PHONE), Some(Area::ALL));
        // A long name is cut to a line of a card.
        set_name(&path, PHONE, &"n".repeat(200)).unwrap();
        assert_eq!(name_of(&Config::load(&path).unwrap(), PHONE).map(|n| n.chars().count()), Some(NAME_LENGTH));
        // Taken out one by one: the table goes with its last field, `[device]` with its last table.
        set_name(&path, DESK, "").unwrap();
        let read = Config::load(&path).unwrap();
        assert_eq!((name_of(&read, DESK), said_area(&read, DESK)), (None, Some(Area::WORK)));
        set_area(&path, DESK, "").unwrap();
        assert!(!Config::load(&path).unwrap().devices.contains_key(DESK));
        set_name(&path, PHONE, "").unwrap();
        set_area(&path, PHONE, "").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("device.") && !text.contains("[device]") && text.contains("device_kind = \"tablet\"") && text.contains("# mine"), "{text}");
        // Taking out what was never said writes nothing.
        set_area(&path, DESK, "").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A Sioul that does not know the table keeps it as it is when it writes
    /// the settings: every writer edits the file in place (`toml_edit`), so a
    /// setting changed, a list written and an agenda's field said leave
    /// `[device]` untouched, comments and a newer Sioul's cells included;
    /// and this Sioul's own writers keep what a newer one wrote in a
    /// device's table.
    #[test]
    fn writing_other_settings_keeps_the_devices_table_untouched() {
        let dir = scratch("kept");
        let path = dir.join("config.toml");
        let table = format!("[device.\"{DESK}\"]\n# the laptop on the desk\nname = \"Work laptop\"\narea = \"work\"\nfollows = 2\n\n[device.\"{DESK}\".at]\nwork = \"work+leisure\"\n");
        std::fs::write(&path, format!("language = \"en\"\n\n{table}\n[agenda]\nbetween = 10\n")).unwrap();
        // As a Sioul before this table writes: a setting, a list, a table of its own.
        crate::settings::change(&path, "theme", None, &crate::config::SettingValue::Text("dark".into())).unwrap();
        crate::config::set_history(&path, 12).unwrap();
        crate::agendas::set_field(&path, "home/personal", "area", "admin+leisure").unwrap();
        crate::config::set_table(&path, "attention", &[("mail.safe".to_string(), Some(vec!["work".to_string()]))], &[]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(&table), "the table as it was, byte for byte: {text}");
        assert!(text.contains("theme = \"dark\"") && text.contains("history_weeks = 12") && text.contains("[attention]"), "{text}");
        // This Sioul's own writers keep the cells it does not know.
        set_area(&path, DESK, "work+admin").unwrap();
        set_name(&path, DESK, "Desk").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("follows = 2") && text.contains("work = \"work+leisure\"") && text.contains("# the laptop on the desk"), "{text}");
        let read = Config::load(&path).unwrap();
        assert_eq!((name_of(&read, DESK), said_area(&read, DESK)), (Some("Desk"), Some(Area::MIXED)));
        // Its name and area taken out, the table stays while a newer Sioul's cells are in it.
        set_area(&path, DESK, "").unwrap();
        set_name(&path, DESK, "").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("follows = 2") && text.contains("work = \"work+leisure\"") && !text.contains("name ="), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
