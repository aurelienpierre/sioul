// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Calls, per phone (docs/android.md, "Calls, per phone"): what one phone
//! lets ring, said on its line in Settings ▸ Devices, from any device.
//!
//! Kept in the shared configuration beside the device's name and areas,
//! `[device."<id>".calls]` (`device_said`), read by that phone alone, which
//! builds its calls' table from it (`calls::build`):
//! - **a profile** (`profile`): professional, personal, both, as-android
//!   (Sioul screens nothing on that phone), or `none` once a cell or a switch
//!   was changed by hand, with the profile the change started from (`base`),
//!   whose cells the unsaid ones keep;
//! - **one small table** (`rows.<row>.<when>`, "rings" or "voicemail"): the
//!   rows Safe, Your contacts, Restricted, numbers nobody knows, hidden
//!   numbers, and any group of your contacts given a row (`group:<name>`, a
//!   category as your cards write it); the columns work hours, the rest of
//!   the day, the night, while busy. Always through is a row of its own that
//!   always rings, never written;
//! - **three switches**: commercial and automated callers never ring
//!   (`commercial`), a second call within 15 minutes rings (`twice`), numbers
//!   called in the last 14 days ring as your contacts do (`called`).
//!
//! Each cell and each switch is a field of its own, so that the sharing
//! carries them one by one, as it carries the rest of the device's line: two
//! cells changed on two devices both stay. Read leniently: a field or a word
//! this Sioul does not read is passed over and kept in the file, never an
//! error. **A line that says nothing readable leaves the phone as it was**:
//! its table is the shared `[screening]` and the matrix's Calls rows, as
//! before (`calls::build`).

use crate::areas::Area;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// The key of the calls' table in a device's line.
pub const KEY: &str = "calls";
/// The rows' table in it.
pub const ROWS: &str = "rows";
/// What starts a group's row: `group:School`.
pub const GROUP: &str = "group:";
/// The three switches' keys, in the order shown.
pub const SWITCHES: [&str; 3] = ["commercial", "twice", "called"];

/// Where a phone's table starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// A phone for work: numbers nobody knows ring in work hours, commercial
    /// callers never, everything but Always through to voicemail outside.
    Professional,
    /// A phone of your own: your contacts ring, numbers nobody knows go to
    /// voicemail unless they call twice.
    Personal,
    /// One phone for both: work's rows in work hours, the personal ones outside.
    Both,
    /// As Android does: Sioul screens nothing on that phone (the blocked aside).
    AsAndroid,
    /// Cells or switches changed by hand ("none" in the file).
    Own,
}

impl Profile {
    /// The profiles offered, in the order shown.
    pub const OFFERED: [Profile; 4] = [Profile::Professional, Profile::Personal, Profile::Both, Profile::AsAndroid];

    pub fn id(self) -> &'static str {
        match self {
            Profile::Professional => "professional",
            Profile::Personal => "personal",
            Profile::Both => "both",
            Profile::AsAndroid => "as-android",
            Profile::Own => "none",
        }
    }

    /// A word of the file; none for any other.
    pub fn read(word: &str) -> Option<Profile> {
        Some(match word.trim().to_lowercase().as_str() {
            "professional" | "pro" | "work" => Profile::Professional,
            "personal" | "perso" => Profile::Personal,
            "both" => Profile::Both,
            "as-android" | "android" => Profile::AsAndroid,
            "none" | "own" => Profile::Own,
            _ => return None,
        })
    }

    /// The table it fills (design of 18 November 2026, section 2; the owner's three examples are
    /// the tests). `Own` has no table of its own: everything rings.
    pub fn grid(self) -> Grid {
        const R: bool = true;
        const V: bool = false;
        // Columns: work hours, the rest of the day, the night, while busy.
        // Rows: Safe, Your contacts, Restricted, numbers nobody knows, hidden numbers.
        let (fixed, switches) = match self {
            Profile::Professional => (
                [[R, V, V, V], [R, V, V, V], [R, V, V, V], [R, V, V, V], [R, V, V, V]],
                Switches { commercial: true, twice: false, called: true },
            ),
            Profile::Personal => (
                [[R, R, V, R], [R, R, V, V], [R, V, V, V], [V, V, V, V], [R, V, V, V]],
                Switches { commercial: true, twice: true, called: true },
            ),
            Profile::Both => (
                [[R, R, V, R], [R, R, V, V], [R, V, V, V], [R, V, V, V], [R, V, V, V]],
                Switches { commercial: true, twice: true, called: true },
            ),
            Profile::AsAndroid => ([[R; 4]; 5], Switches { commercial: false, twice: true, called: true }),
            Profile::Own => return Grid::ringing(),
        };
        Grid { profile: self, fixed, groups: Vec::new(), switches }
    }

    /// The profile offered first to a phone said for `area`: work alone,
    /// Professional; admin or leisure without work, Personal; else Both.
    pub fn offered(area: Option<Area>) -> Profile {
        match area {
            Some(area) if area.work && !area.admin && !area.leisure => Profile::Professional,
            Some(area) if !area.work && (area.admin || area.leisure) => Profile::Personal,
            _ => Profile::Both,
        }
    }
}

/// A column of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum When {
    /// Your work hours.
    Work,
    /// The rest of the day: your admin, leisure, meals, Free time.
    Day,
    /// The night, from winding down to waking, and naps.
    Night,
    /// While you are busy: an event or a task marked busy (`busy::stretches`).
    Busy,
}

impl When {
    pub const ALL: [When; 4] = [When::Work, When::Day, When::Night, When::Busy];

    pub fn id(self) -> &'static str {
        match self {
            When::Work => "work",
            When::Day => "day",
            When::Night => "night",
            When::Busy => "busy",
        }
    }

    pub fn read(word: &str) -> Option<When> {
        When::ALL.into_iter().find(|w| w.id() == word.trim())
    }

    /// Its place in a row's cells.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// A row of the table, by its key in the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Row {
    Safe,
    Contacts,
    Restricted,
    /// Numbers nobody knows.
    Unknown,
    Hidden,
    /// A group of your contacts (a category of their cards), as written.
    Group(String),
}

impl Row {
    /// The rows every table has, Always through aside.
    pub const FIXED: [Row; 5] = [Row::Safe, Row::Contacts, Row::Restricted, Row::Unknown, Row::Hidden];

    pub fn id(&self) -> String {
        match self {
            Row::Safe => "safe".into(),
            Row::Contacts => "contacts".into(),
            Row::Restricted => "restricted".into(),
            Row::Unknown => "unknown".into(),
            Row::Hidden => "hidden".into(),
            Row::Group(name) => format!("{GROUP}{name}"),
        }
    }

    /// A row's key; none for one this Sioul does not read (kept in the file).
    pub fn read(key: &str) -> Option<Row> {
        Some(match key.trim() {
            "safe" => Row::Safe,
            "contacts" => Row::Contacts,
            "restricted" => Row::Restricted,
            "unknown" => Row::Unknown,
            "hidden" => Row::Hidden,
            other => {
                let name = other.strip_prefix(GROUP)?.trim();
                if name.is_empty() || name.chars().any(char::is_control) {
                    return None;
                }
                Row::Group(name.to_string())
            }
        })
    }

    fn fixed_index(&self) -> Option<usize> {
        Row::FIXED.iter().position(|r| r == self)
    }
}

/// A row's four cells, by `When`: rings (true) or voicemail.
pub type Cells = [bool; 4];

/// A group's row when added: rings but at night.
pub const GROUP_CELLS: Cells = [true, true, false, true];

/// The three switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Switches {
    /// Commercial and automated callers never ring (ARCEP's roots for
    /// machines, numbers that cannot be a caller's, a failed verification).
    pub commercial: bool,
    /// A second call within 15 minutes rings.
    pub twice: bool,
    /// A number you called in the last 14 days rings as your contacts do.
    pub called: bool,
}

impl Switches {
    fn set(&mut self, key: &str, on: bool) {
        match key {
            "commercial" => self.commercial = on,
            "twice" => self.twice = on,
            "called" => self.called = on,
            _ => {}
        }
    }
}

/// A phone's table as it holds: what it reads as, every row, the switches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Grid {
    /// The profile it reads as: the one said while every fixed cell and
    /// switch is that profile's, else `Own`.
    pub profile: Profile,
    /// The fixed rows, in `Row::FIXED`'s order.
    pub fixed: [Cells; 5],
    /// The groups given a row, by name as written, in their names' order.
    pub groups: Vec<(String, Cells)>,
    pub switches: Switches,
}

impl Grid {
    /// Everything rings; the switches as the shared screening's first values.
    pub fn ringing() -> Grid {
        Grid { profile: Profile::Own, fixed: [[true; 4]; 5], groups: Vec::new(), switches: Switches { commercial: true, twice: true, called: true } }
    }

    /// A row's cells; none for a group with no row.
    pub fn cells(&self, row: &Row) -> Option<Cells> {
        match row {
            Row::Group(name) => {
                let key = crate::porch::category_key(name);
                self.groups.iter().find(|(n, _)| crate::porch::category_key(n) == key).map(|(_, c)| *c)
            }
            fixed => fixed.fixed_index().map(|i| self.fixed[i]),
        }
    }

    /// Whether a row rings in a column; a group with no row does not say (false).
    pub fn rings(&self, row: &Row, when: When) -> bool {
        self.cells(row).is_some_and(|c| c[when.index()])
    }

    /// Sioul screens nothing on that phone.
    pub fn screens_nothing(&self) -> bool {
        self.profile == Profile::AsAndroid
    }
}

/// What a phone's line says of calls, as written: each field unsaid until it
/// reads. A cell is `rows.<row>.<when>`; only those that read are here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Said {
    pub profile: Option<Profile>,
    /// For `none`: the profile the cells were changed from.
    pub base: Option<Profile>,
    /// Row key → column → rings.
    pub cells: BTreeMap<String, BTreeMap<When, bool>>,
    pub commercial: Option<bool>,
    pub twice: Option<bool>,
    pub called: Option<bool>,
}

impl Said {
    /// Whether the line says anything this Sioul reads: else the phone keeps
    /// the shared screening, as before.
    pub fn says(&self) -> bool {
        self.profile.is_some() || !self.cells.is_empty() || self.commercial.is_some() || self.twice.is_some() || self.called.is_some()
    }

    fn switch(&self, key: &str) -> Option<bool> {
        match key {
            "commercial" => self.commercial,
            "twice" => self.twice,
            "called" => self.called,
            _ => None,
        }
    }

    /// The table it holds: the profile's (`none`: its base's; no base:
    /// everything rings), each cell and switch said over it, the groups'
    /// rows (a cell unsaid in one: `GROUP_CELLS`'s).
    pub fn grid(&self) -> Grid {
        let start = match self.profile {
            Some(Profile::Own) | None => self.base.filter(|b| *b != Profile::Own).map_or_else(Grid::ringing, Profile::grid),
            Some(profile) => profile.grid(),
        };
        let mut grid = Grid { profile: Profile::Own, ..start.clone() };
        let mut groups: BTreeMap<String, (String, Cells)> = BTreeMap::new();
        for (key, cells) in &self.cells {
            match Row::read(key) {
                Some(Row::Group(name)) => {
                    let entry = groups.entry(crate::porch::category_key(&name)).or_insert_with(|| (name.clone(), GROUP_CELLS));
                    for (when, rings) in cells {
                        entry.1[when.index()] = *rings;
                    }
                }
                Some(row) => {
                    if let Some(i) = row.fixed_index() {
                        for (when, rings) in cells {
                            grid.fixed[i][when.index()] = *rings;
                        }
                    }
                }
                None => {}
            }
        }
        for key in SWITCHES {
            if let Some(on) = self.switch(key) {
                grid.switches.set(key, on);
            }
        }
        grid.groups = groups.into_values().collect();
        // It reads as the profile said while nothing differs from it.
        if let Some(profile) = self.profile.filter(|p| *p != Profile::Own)
            && grid.fixed == start.fixed
            && grid.switches == start.switches
        {
            grid.profile = profile;
        }
        grid
    }
}

/// `[device."<id>".calls]` read leniently: a value that is no table, a word
/// that names nothing, a cell that is no word, are passed over (and kept in
/// the file by every writing, which edits it in place). None when nothing
/// reads.
pub fn read(value: &toml::Value) -> Option<Said> {
    let table = value.as_table()?;
    let word = |key: &str| table.get(key).and_then(toml::Value::as_str);
    let flag = |key: &str| table.get(key).and_then(toml::Value::as_bool);
    let mut said = Said {
        profile: word("profile").and_then(Profile::read),
        base: word("base").and_then(Profile::read),
        commercial: flag("commercial"),
        twice: flag("twice"),
        called: flag("called"),
        cells: BTreeMap::new(),
    };
    if let Some(rows) = table.get(ROWS).and_then(toml::Value::as_table) {
        for (key, cells) in rows {
            let (Some(row), Some(cells)) = (Row::read(key), cells.as_table()) else { continue };
            let read: BTreeMap<When, bool> = cells.iter().filter_map(|(when, cell)| Some((When::read(when)?, cell_word(cell.as_str()?)?))).collect();
            if !read.is_empty() {
                said.cells.insert(row.id(), read);
            }
        }
    }
    said.says().then_some(said)
}

/// "rings" or "voicemail" (and their near words); none for any other.
fn cell_word(word: &str) -> Option<bool> {
    match word.trim().to_lowercase().as_str() {
        "rings" | "ring" => Some(true),
        "voicemail" => Some(false),
        _ => None,
    }
}

fn cell_text(rings: bool) -> &'static str {
    if rings { "rings" } else { "voicemail" }
}

// ---------------------------------------------------------------- in words

/// What a phone's card needs to say beside its line.
#[derive(Clone, Copy)]
pub struct Card<'a> {
    /// What the phone is for, as you said it (`device_said::Said::area`).
    pub area: Option<Area>,
    /// Whether the phone screens calls, as it says (its `screens` in
    /// do-not-disturb's switch file); none: it says nothing yet.
    pub screens: Option<bool>,
    /// Every category of your contacts' cards, as first written: the groups a row may be given to.
    pub categories: &'a [String],
}

/// One phone's section Calls, in words, for its card in Settings ▸ Devices:
/// {set, profile, offered, profiles: [{id, label, chosen}], said, columns:
/// [{id, label}], rows: [{id, label, fixed, group, cells: [{when, rings,
/// word}]}], switches: [{key, label, on}], groups (categories with no row
/// yet), screens, sims, always}. A line that says nothing shows the profile
/// offered, its table as a start, nothing chosen.
pub fn view(said: Option<&Said>, card: &Card, tr: &crate::i18n::Translator) -> serde_json::Value {
    use serde_json::json;
    let set = said.is_some_and(Said::says);
    let offered = Profile::offered(card.area);
    let grid = said.filter(|s| s.says()).map_or_else(|| offered.grid(), Said::grid);
    let chosen = set.then_some(grid.profile);
    let mut profiles: Vec<serde_json::Value> = Profile::OFFERED.iter().map(|p| json!({ "id": p.id(), "label": tr.text(&format!("phone-calls-profile-{}", p.id()), None), "chosen": chosen == Some(*p) })).collect();
    if chosen == Some(Profile::Own) {
        profiles.push(json!({ "id": "none", "label": tr.text("phone-calls-profile-none", None), "chosen": true }));
    }
    let said_words = match chosen {
        None => tr.text("phone-calls-unsaid", None),
        Some(Profile::AsAndroid) => tr.text("phone-calls-android", None),
        Some(_) => String::new(),
    };
    let word = |rings: bool| tr.text(if rings { "phone-calls-rings" } else { "phone-calls-voicemail" }, None);
    let cells = |cells: Cells| -> Vec<serde_json::Value> { When::ALL.iter().map(|w| json!({ "when": w.id(), "rings": cells[w.index()], "word": word(cells[w.index()]) })).collect() };
    let row = |id: String, label: String, fixed: bool, group: bool, c: Cells| json!({ "id": id, "label": label, "fixed": fixed, "group": group, "cells": cells(c) });
    let fixed_row = |r: Row| {
        let id = r.id();
        let c = grid.cells(&r).unwrap_or([true; 4]);
        row(id.clone(), tr.text(&format!("phone-calls-row-{id}"), None), false, false, c)
    };
    let mut rows = vec![row("always".into(), tr.text("phone-calls-row-always", None), true, false, [true; 4]), fixed_row(Row::Safe), fixed_row(Row::Contacts)];
    rows.extend(grid.groups.iter().map(|(name, c)| row(Row::Group(name.clone()).id(), name.clone(), false, true, *c)));
    rows.extend([fixed_row(Row::Restricted), fixed_row(Row::Unknown), fixed_row(Row::Hidden)]);
    let switches: Vec<serde_json::Value> = SWITCHES.iter().map(|key| json!({ "key": key, "label": tr.text(&format!("phone-calls-{key}"), None), "on": match *key { "commercial" => grid.switches.commercial, "twice" => grid.switches.twice, _ => grid.switches.called } })).collect();
    let taken: Vec<String> = grid.groups.iter().map(|(n, _)| crate::porch::category_key(n)).collect();
    let groups: Vec<&String> = card.categories.iter().filter(|c| !taken.contains(&crate::porch::category_key(c))).collect();
    json!({
        "set": set,
        "profile": chosen.map_or("", Profile::id),
        "offered": offered.id(),
        "profiles": profiles,
        "said": said_words,
        "columns": When::ALL.iter().map(|w| json!({ "id": w.id(), "label": tr.text(&format!("phone-calls-when-{}", w.id()), None) })).collect::<Vec<_>>(),
        "rows": rows,
        "switches": switches,
        "groups": groups,
        "screens": card.screens,
        "not_screening": if card.screens == Some(true) { String::new() } else { tr.text("phone-calls-not-screening", None) },
        "sims": tr.text("phone-calls-sims", None),
        "always": tr.text("phone-calls-always", None),
    })
}

// ---------------------------------------------------------------- writing

/// The phone `id`'s calls' table changed in the configuration, in place,
/// under the file's lock (which the sharing takes too): `change` gets the
/// line as it reads now and its `calls` table to edit; tables left empty go,
/// the device's line with its last field, `[device]` with its last line.
fn edit(path: &Path, id: &str, change: impl FnOnce(Option<Said>, &mut dyn toml_edit::TableLike) -> Result<(), String>) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() || id.chars().any(char::is_control) {
        return Err("a device's id is needed".to_string());
    }
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let now: Option<Said> = doc.to_string().parse::<toml::Table>().ok().and_then(|t| t.get("device")?.get(id)?.get(KEY).and_then(read));
        if doc.get("device").is_none_or(|item| !item.is_table_like()) {
            let mut devices = toml_edit::Table::new();
            devices.set_implicit(true);
            doc.insert("device", toml_edit::Item::Table(devices));
        }
        let devices = doc.get_mut("device").and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{}: device is not a table", path.display()))?;
        if devices.get(id).is_none_or(|line| !line.is_table_like()) {
            let mut line = toml_edit::Table::new();
            line.set_implicit(true);
            devices.insert(id, toml_edit::Item::Table(line));
        }
        let line = devices.get_mut(id).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: not a table"))?;
        if line.get(KEY).is_none_or(|calls| !calls.is_table_like()) {
            line.insert(KEY, toml_edit::Item::Table(toml_edit::Table::new()));
        }
        let calls = line.get_mut(KEY).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: calls is not a table"))?;
        change(now, calls)?;
        // What is left empty goes.
        if let Some(rows) = calls.get_mut(ROWS).and_then(toml_edit::Item::as_table_like_mut) {
            let empty: Vec<String> = rows.iter().filter(|(_, row)| row.as_table_like().is_some_and(toml_edit::TableLike::is_empty)).map(|(key, _)| key.to_string()).collect();
            for key in empty {
                rows.remove(&key);
            }
        }
        if calls.get(ROWS).and_then(toml_edit::Item::as_table_like).is_some_and(toml_edit::TableLike::is_empty) {
            calls.remove(ROWS);
        }
        if calls.is_empty() {
            line.remove(KEY);
        }
        if line.is_empty() {
            devices.remove(id);
        }
        if devices.is_empty() {
            doc.remove("device");
        }
        crate::config::write_document(path, &doc)
    })
}

/// A value written in place: a key already there keeps its place and the comment above it.
fn put(table: &mut dyn toml_edit::TableLike, key: &str, value: toml_edit::Item) {
    match table.get_mut(key) {
        Some(slot) => *slot = value,
        None => {
            table.insert(key, value);
        }
    }
}

/// The rows' table of `calls`, made when missing.
fn rows_of(calls: &mut dyn toml_edit::TableLike) -> Result<&mut dyn toml_edit::TableLike, String> {
    if calls.get(ROWS).is_none_or(|rows| !rows.is_table_like()) {
        let mut rows = toml_edit::Table::new();
        rows.set_implicit(true);
        calls.insert(ROWS, toml_edit::Item::Table(rows));
    }
    calls.get_mut(ROWS).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| "calls.rows is not a table".to_string())
}

/// A profile chosen for the phone `id`: its word written; the fixed rows'
/// cells, the switches and the base go, as the profile says them; the
/// groups' rows stay. `Own` is no choice: a cell or a switch makes it.
pub fn set_profile(path: &Path, id: &str, profile: Profile) -> Result<(), String> {
    if profile == Profile::Own {
        return Err("none: change a cell or a switch".to_string());
    }
    edit(path, id, |_, calls| {
        put(calls, "profile", toml_edit::value(profile.id()));
        calls.remove("base");
        for key in SWITCHES {
            calls.remove(key);
        }
        if let Some(rows) = calls.get_mut(ROWS).and_then(toml_edit::Item::as_table_like_mut) {
            for row in Row::FIXED {
                rows.remove(&row.id());
            }
        }
        Ok(())
    })
}

/// Changed by hand from a profile: "none", the profile it started from kept
/// as the base its unsaid cells read from; a line that said none yet starts
/// from `offered` (the profile the card shows it from).
fn by_hand(now: Option<&Said>, calls: &mut dyn toml_edit::TableLike, offered: Profile) {
    let (profile, base) = (now.and_then(|s| s.profile), now.and_then(|s| s.base));
    // Already "none": its base stays as it is (none: everything rings but what is said).
    if profile != Some(Profile::Own) {
        let from = profile.or(base).unwrap_or(offered);
        if from != Profile::Own {
            put(calls, "base", toml_edit::value(from.id()));
        }
        put(calls, "profile", toml_edit::value(Profile::Own.id()));
    }
}

/// One cell of the phone `id`'s table: a fixed row's (the profile becomes
/// "none", the cell alone written; a line that said nothing yet starts from
/// `offered`) or a group's (that group's row made with its first cells when
/// it had none). Always through is no cell.
pub fn set_cell(path: &Path, id: &str, row: &Row, when: When, rings: bool, offered: Profile) -> Result<(), String> {
    edit(path, id, |now, calls| {
        let group = matches!(row, Row::Group(_));
        if !group {
            by_hand(now.as_ref(), calls, offered);
        } else if now.is_none() {
            // A group's row on a line that said nothing: the line starts from the profile offered.
            put(calls, "profile", toml_edit::value(offered.id()));
        }
        let rows = rows_of(calls)?;
        // A group's row under the key it was written with (another case), else its own.
        let key = match row {
            Row::Group(name) => {
                let wanted = crate::porch::category_key(name);
                rows.iter().map(|(k, _)| k.to_string()).find(|k| matches!(Row::read(k), Some(Row::Group(n)) if crate::porch::category_key(&n) == wanted)).unwrap_or_else(|| row.id())
            }
            fixed => fixed.id(),
        };
        let fresh = rows.get(&key).is_none_or(|r| !r.is_table_like());
        if fresh {
            let mut table = toml_edit::Table::new();
            if group {
                for w in When::ALL {
                    table.insert(w.id(), toml_edit::value(cell_text(GROUP_CELLS[w.index()])));
                }
            }
            rows.insert(&key, toml_edit::Item::Table(table));
        }
        let cells = rows.get_mut(&key).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{key}: not a table"))?;
        put(cells, when.id(), toml_edit::value(cell_text(rings)));
        Ok(())
    })
}

/// One switch of the phone `id` (`SWITCHES`): the profile becomes "none"
/// (a line that said nothing yet starts from `offered`).
pub fn set_switch(path: &Path, id: &str, key: &str, on: bool, offered: Profile) -> Result<(), String> {
    if !SWITCHES.contains(&key) {
        return Err(format!("{key}: {}", SWITCHES.join(", ")));
    }
    edit(path, id, |now, calls| {
        by_hand(now.as_ref(), calls, offered);
        put(calls, key, toml_edit::value(on));
        Ok(())
    })
}

/// A group of your contacts given a row on the phone `id`, its first cells
/// `GROUP_CELLS`; nothing when it has one (as categories compare). A line
/// that said nothing yet starts from `offered`.
pub fn add_group(path: &Path, id: &str, name: &str, offered: Profile) -> Result<(), String> {
    let name: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some(Row::Group(name)) = Row::read(&format!("{GROUP}{name}")) else {
        return Err("a group's name is needed".to_string());
    };
    edit(path, id, |now, calls| {
        if now.is_none() {
            put(calls, "profile", toml_edit::value(offered.id()));
        }
        let rows = rows_of(calls)?;
        let wanted = crate::porch::category_key(&name);
        if rows.iter().any(|(k, _)| matches!(Row::read(k), Some(Row::Group(n)) if crate::porch::category_key(&n) == wanted)) {
            return Ok(());
        }
        let mut table = toml_edit::Table::new();
        for w in When::ALL {
            table.insert(w.id(), toml_edit::value(cell_text(GROUP_CELLS[w.index()])));
        }
        rows.insert(&Row::Group(name.clone()).id(), toml_edit::Item::Table(table));
        Ok(())
    })
}

/// A group's row taken off the phone `id` (as categories compare).
pub fn remove_group(path: &Path, id: &str, name: &str) -> Result<(), String> {
    let wanted = crate::porch::category_key(name);
    edit(path, id, |_, calls| {
        if let Some(rows) = calls.get_mut(ROWS).and_then(toml_edit::Item::as_table_like_mut) {
            let keys: Vec<String> = rows.iter().map(|(k, _)| k.to_string()).filter(|k| matches!(Row::read(k), Some(Row::Group(n)) if crate::porch::category_key(&n) == wanted)).collect();
            for key in keys {
                rows.remove(&key);
            }
        }
        Ok(())
    })
}

/// The phone `id`'s calls' table taken out whole: it keeps the shared
/// screening again, as before any was said. What this Sioul does not read
/// in it goes too.
pub fn clear(path: &Path, id: &str) -> Result<(), String> {
    edit(path, id, |_, calls| {
        let keys: Vec<String> = calls.iter().map(|(k, _)| k.to_string()).collect();
        for key in keys {
            calls.remove(&key);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHONE: &str = "7b1c9e04-52a8-4c0d-8f0e-93c1b2a4d5e6";

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-calls-line-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    fn said_in(path: &Path) -> Option<Said> {
        crate::config::Config::load(path).unwrap().devices.get(PHONE).and_then(|s| s.calls.clone())
    }

    /// Read leniently: a word, a cell or a field this Sioul does not read is
    /// passed over, the rest read; a table of nothing readable is no line,
    /// and the configuration always reads.
    #[test]
    fn the_line_reads_leniently() {
        let text = format!(
            "[device.\"{PHONE}\"]\nname = \"Pocket\"\n\n[device.\"{PHONE}\".calls]\nprofile = \"personal\"\nfollows = 3\ntwice = \"maybe\"\ncalled = false\n\n[device.\"{PHONE}\".calls.rows.safe]\nnight = \"rings\"\nlater = \"rings\"\nbusy = 4\n\n[device.\"{PHONE}\".calls.rows.\"group:School\"]\nnight = \"rings\"\n\n[device.\"{PHONE}\".calls.rows.lunch]\nwork = \"rings\"\n"
        );
        let said = read(&text.parse::<toml::Table>().unwrap()["device"][PHONE]["calls"]).unwrap();
        assert_eq!(said.profile, Some(Profile::Personal));
        assert_eq!((said.twice, said.called, said.commercial), (None, Some(false), None), "a switch that is no flag is unsaid");
        assert_eq!(said.cells.get("safe").map(|c| c.len()), Some(1), "only the cell that reads: {said:?}");
        assert!(!said.cells.contains_key("lunch"), "a row this Sioul does not know");
        let grid = said.grid();
        assert_eq!(grid.profile, Profile::Own, "a cell said apart from the profile: by hand");
        assert!(grid.rings(&Row::Safe, When::Night) && !grid.switches.called);
        assert_eq!(grid.cells(&Row::Group("school".into())), Some([true, true, true, true]), "a group's row, its unsaid cells as a group's first");
        // Nothing readable: no line, the phone as before; the rest of the configuration reads.
        for nothing in ["calls = 3", "[device.\"P\".calls]\nprofile = \"loud\"\n[device.\"P\".calls.rows.safe]\nwork = true\n"] {
            let text = if nothing.starts_with('[') { nothing.to_string() } else { format!("[device.\"P\"]\n{nothing}\n") };
            let config: crate::config::Config = toml::from_str(&format!("language = \"en\"\n{text}")).unwrap();
            assert_eq!(config.devices.get("P").and_then(|s| s.calls.clone()), None, "{nothing}");
        }
    }

    /// The card's words, in both languages: a line that says nothing shows
    /// the profile offered, nothing chosen; a line by hand reads "Your own";
    /// As Android does says Sioul screens nothing; the rows in the design's
    /// order, Always through fixed; the groups not given a row offered;
    /// a phone that does not screen says so.
    #[test]
    fn the_card_says_the_line_in_words() {
        for language in ["en", "fr"] {
            let tr = crate::i18n::Translator::new(language);
            let categories = ["School".to_string(), "Clients".to_string()];
            let card = Card { area: Some(Area::WORK), screens: Some(false), categories: &categories };
            let unsaid = view(None, &card, &tr);
            assert_eq!((unsaid["set"].as_bool(), unsaid["profile"].as_str(), unsaid["offered"].as_str()), (Some(false), Some(""), Some("professional")));
            assert!(unsaid["profiles"].as_array().unwrap().iter().all(|p| p["chosen"] == false));
            assert!(!unsaid["not_screening"].as_str().unwrap().is_empty());
            let mut own = Said { profile: Some(Profile::Own), base: Some(Profile::Personal), ..Said::default() };
            own.cells.insert("group:School".into(), [(When::Night, true)].into_iter().collect());
            let view_own = view(Some(&own), &Card { screens: Some(true), ..card }, &tr);
            assert_eq!(view_own["profiles"].as_array().unwrap().last().unwrap()["id"], "none");
            let ids: Vec<&str> = view_own["rows"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();
            assert_eq!(ids, ["always", "safe", "contacts", "group:School", "restricted", "unknown", "hidden"]);
            assert_eq!(view_own["rows"][0]["fixed"], true);
            assert_eq!(view_own["groups"], serde_json::json!(["Clients"]), "School has its row");
            assert_eq!(view_own["not_screening"], "");
            let android = view(Some(&Said { profile: Some(Profile::AsAndroid), ..Said::default() }), &card, &tr);
            assert!(!android["said"].as_str().unwrap().is_empty());
            let mut words: Vec<String> = Vec::new();
            for v in [&unsaid, &view_own, &android] {
                for key in ["said", "sims", "always", "not_screening"] {
                    words.push(v[key].as_str().unwrap().to_string());
                }
                for list in ["profiles", "columns", "rows", "switches"] {
                    words.extend(v[list].as_array().unwrap().iter().filter_map(|x| x["label"].as_str().map(str::to_string)));
                }
                words.extend(v["rows"].as_array().unwrap().iter().flat_map(|r| r["cells"].as_array().unwrap().iter().map(|c| c["word"].as_str().unwrap().to_string())));
            }
            for w in words.iter().filter(|w| !w.is_empty()) {
                assert!(!w.starts_with("phone-calls-"), "{language}: {w}");
                if language == "fr" {
                    assert!(![" :", " ;", " ?", " !", "'"].iter().any(|bad| w.contains(bad)), "{w}");
                }
            }
        }
    }

    /// The owner's three examples, and the other profiles.
    #[test]
    fn each_profile_fills_the_table() {
        use When::*;
        let pro = Profile::Professional.grid();
        assert!(pro.rings(&Row::Unknown, Work), "numbers nobody knows ring in work hours");
        assert!(pro.switches.commercial, "commercial callers never");
        for when in [Day, Night, Busy] {
            assert!(Row::FIXED.iter().all(|row| !pro.rings(row, when)), "everything but Always through to voicemail outside: {when:?}");
        }
        assert!(!pro.switches.twice, "outside, not even twice");
        let personal = Profile::Personal.grid();
        assert!(personal.rings(&Row::Contacts, Work) && personal.rings(&Row::Contacts, Day), "contacts ring");
        assert!(When::ALL.iter().all(|w| !personal.rings(&Row::Unknown, *w)) && personal.switches.twice, "numbers nobody knows to voicemail unless they call twice");
        let both = Profile::Both.grid();
        for (i, row) in Row::FIXED.iter().enumerate() {
            assert_eq!(both.fixed[i][0], pro.fixed[i][0], "work's rows in work hours: {row:?}");
            assert_eq!(both.fixed[i][1..3], personal.fixed[i][1..3], "the personal ones outside: {row:?}");
        }
        let android = Profile::AsAndroid.grid();
        assert!(android.fixed.iter().flatten().all(|r| *r) && android.screens_nothing() && !android.switches.commercial);
        // Offered by what the phone is for.
        assert_eq!(Profile::offered(Some(Area::WORK)), Profile::Professional);
        assert_eq!(Profile::offered(Some(Area::PERSONAL)), Profile::Personal);
        assert_eq!(Profile::offered(Some(Area::LEISURE)), Profile::Personal);
        assert_eq!((Profile::offered(Some(Area::ALL)), Profile::offered(None)), (Profile::Both, Profile::Both));
    }

    /// Written in place: a profile; a cell by hand makes it "none" with its
    /// base, the other cells still the base's; a switch alike; a group's row
    /// added once, changed, taken off; a profile again leaves the groups; the
    /// whole line taken out. The device's name and a newer Sioul's field stay.
    #[test]
    fn the_line_is_written_in_place() {
        let path = scratch("write");
        std::fs::write(&path, format!("language = \"en\"\n\n[device.\"{PHONE}\"]\nname = \"Pocket\"\n\n[device.\"{PHONE}\".calls]\nfollows = 3\n")).unwrap();
        assert_eq!(said_in(&path), None, "a newer field alone says nothing");
        set_profile(&path, PHONE, Profile::Professional).unwrap();
        assert_eq!(said_in(&path).unwrap().grid(), Profile::Professional.grid());
        set_cell(&path, PHONE, &Row::Safe, When::Night, true, Profile::Personal).unwrap();
        let said = said_in(&path).unwrap();
        assert_eq!((said.profile, said.base), (Some(Profile::Own), Some(Profile::Professional)));
        let grid = said.grid();
        assert!(grid.rings(&Row::Safe, When::Night) && grid.rings(&Row::Unknown, When::Work) && !grid.switches.twice, "the rest as the base: {grid:?}");
        set_switch(&path, PHONE, "twice", true, Profile::Personal).unwrap();
        assert_eq!(said_in(&path).unwrap().base, Some(Profile::Professional), "the base kept");
        assert!(said_in(&path).unwrap().grid().switches.twice);
        assert!(set_switch(&path, PHONE, "loud", true, Profile::Personal).is_err());
        add_group(&path, PHONE, "  School ", Profile::Personal).unwrap();
        add_group(&path, PHONE, "school", Profile::Personal).unwrap();
        set_cell(&path, PHONE, &Row::Group("SCHOOL".into()), When::Night, true, Profile::Personal).unwrap();
        let grid = said_in(&path).unwrap().grid();
        assert_eq!(grid.groups, vec![("School".to_string(), [true, true, true, true])], "one row, under the name first written");
        set_profile(&path, PHONE, Profile::Personal).unwrap();
        let said = said_in(&path).unwrap();
        assert_eq!((said.profile, said.base, said.twice), (Some(Profile::Personal), None, None));
        let grid = said.grid();
        assert_eq!((grid.profile, grid.fixed), (Profile::Personal, Profile::Personal.grid().fixed));
        assert_eq!(grid.groups.len(), 1, "a profile leaves the groups");
        remove_group(&path, PHONE, "school").unwrap();
        assert!(said_in(&path).unwrap().grid().groups.is_empty());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("name = \"Pocket\"") && text.contains("follows = 3"), "{text}");
        clear(&path, PHONE).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("name = \"Pocket\"") && !text.contains("calls"), "{text}");
        assert_eq!(said_in(&path), None);
        clear(&path, PHONE).unwrap();
        // A cell changed on a line that said nothing: from the profile offered, never from "everything rings".
        set_cell(&path, PHONE, &Row::Unknown, When::Day, true, Profile::Professional).unwrap();
        let said = said_in(&path).unwrap();
        assert_eq!((said.profile, said.base), (Some(Profile::Own), Some(Profile::Professional)));
        assert!(!said.grid().rings(&Row::Contacts, When::Day) && said.grid().rings(&Row::Unknown, When::Day));
        clear(&path, PHONE).unwrap();
        add_group(&path, PHONE, "Clients", Profile::Professional).unwrap();
        assert_eq!(said_in(&path).unwrap().profile, Some(Profile::Professional), "a group added on a line that said nothing");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
