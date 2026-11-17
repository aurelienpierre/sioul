// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What you said of each of your devices (docs/database.md, "Devices"): the
//! name you give it ("Work laptop") and what it is for, work, your admin,
//! leisure, any of them together (docs/areas.md).
//!
//! Kept in the configuration, `[device."<id>"]`, `<id>` being the device's
//! id in the sharing (sioul-sync's `share::Here::id`): `name`, `area`. Only
//! what you said: a device with no `area` is for everything, as every
//! device was before; one with no `name` is named as its system names it.
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
//! Nothing folds or waits by a device's area yet: `area_of` is where that
//! rule will read it.

use crate::areas::Area;
use crate::config::Config;
use serde::{Deserialize, Serialize};
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
    /// Sioul knows is unsaid, and what a newer Sioul wrote beside is passed over.
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
        // A word that names no area, an empty name: unsaid. A newer Sioul's cells beside: passed over.
        let newer: Config = toml::from_str(&format!("[device.\"{DESK}\"]\nname = \"  \"\narea = \"elsewhere\"\nfollows = 2\n\n[device.\"{DESK}\".at]\nwork = \"work+leisure\"\n")).unwrap();
        assert_eq!((name_of(&newer, DESK), said_area(&newer, DESK), area_of(&newer, DESK)), (None, None, Area::ALL));
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
