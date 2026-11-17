// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul devices`: your devices in one table (docs/database.md, "Devices"),
//! the one Settings ▸ Devices shows (`sioul_sync::device_table`): this
//! device first, then each device Sioul shares with: its name, yours when
//! you gave one, what it is, how it is now, when it last opened and last
//! shared, its Sioul, what you say it is for, what it shares.
//!
//! Under each device, what comes forward on it at each of the five times,
//! then what now brings on this one, the areas no device is for, and what a
//! device's areas never keep back (`sioul_sync::device_grid`; docs/areas.md,
//! "What each device is for"). `sioul devices for` says what a device is
//! for, `sioul devices at` changes one cell of its line: both write the
//! shared settings, as Settings ▸ Devices does.
//!
//! The table is read from this device's own files: its entry as its window
//! last wrote it, what you said of each device in the settings, and what the
//! window last kept of the others, said as of then when no window looks
//! now. With the sharing's key (asked of the keyring only when a sharing
//! folder is set), the devices the sharing alone knows are listed too. With
//! sharing off, this device alone.

use crate::Session;
use sioul_core::areafilter::areas_in_words;
use sioul_core::areas::Area;
use sioul_core::device_said;
use sioul_sync::device_table::{self, Row};
use sioul_sync::{device_grid, dispatch};

/// What you say of a device, from the command line.
#[derive(clap::Subcommand)]
pub(crate) enum DevicesCommand {
    /// Says what a device is for: work, admin, leisure, any of them together. Each device then brings forward only what it is for.
    For {
        /// The device: its name as `sioul devices` shows it, or its id.
        device: String,
        /// work, admin, leisure, joined by + (work+leisure); everything, as before anything was said.
        areas: String,
    },
    /// Changes what comes forward on a device at one time: a cell of its line.
    At {
        /// The device: its name as `sioul devices` shows it, or its id.
        device: String,
        /// The time: work, admin, leisure, meals or sleep.
        time: String,
        /// work, admin, leisure, joined by +; none, for nothing then; usual, as the device's areas say.
        areas: String,
    },
}

/// The table's rows as read now, with what they were read from.
fn rows(s: &Session) -> (device_table::Read, Vec<Row>, i64) {
    let state = sioul_core::config::state_dir();
    let now = jiff::Timestamp::now().as_second();
    let shared = sioul_sync::share::Here::load(&state).folder_path().is_some();
    let key = if shared { sioul_sync::secret::named(sioul_sync::share::KEY_NAME).and_then(|key| crate::invoices::unhex(&key)) } else { None };
    let read = device_table::read(&state, &s.config, key.as_ref(), now);
    let rows = device_table::table(&read.facts(&s.config.devices, now, sioul_core::zone::current()), &s.tr);
    (read, rows, now)
}

pub(crate) fn run(s: &Session, json: bool, command: Option<DevicesCommand>) -> Result<(), String> {
    match command {
        Some(DevicesCommand::For { device, areas }) => say_for(s, &device, &areas),
        Some(DevicesCommand::At { device, time, areas }) => say_at(s, &device, &time, &areas),
        None => list(s, json),
    }
}

fn list(s: &Session, json: bool) -> Result<(), String> {
    let (read, mut rows, now) = rows(s);
    let zone = sioul_core::zone::current();
    // What comes forward on each device at each time, and now on this one: the hours as this
    // device's files say them, with what you said each device is for.
    let known = dispatch::Known::read(&s.config, &sioul_core::config::state_dir(), now);
    let at = sioul_core::zone::now();
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let mode = sioul_core::quiet::mode(&s.config.week_hours(), &s.config.time_off, &overrides, &sioul_core::quiet::Blocks::read_now(&at), &at).on(&known.lines());
    let grid = device_grid::grid(&mut rows, &s.config.devices, &known, &mode, &at, &s.tr);
    if json {
        println!("{}", serde_json::json!({ "sharing": read.sharing, "looked": read.looked, "devices": rows, "grid": grid }));
        return Ok(());
    }
    // No window looks now: the others are said as it last saw them.
    if let Some(looked) = read.looked.filter(|_| rows.len() > 1)
        && let Ok(at) = jiff::Timestamp::from_second(looked)
    {
        println!("{}\n", s.say("devices-as-of", &[("when", s.tr.date(&at.to_zoned(zone), true))]));
    }
    for line in device_table::plain(&rows, &s.tr) {
        // A device's name is its own, or yours: no control character reaches the terminal.
        println!("{}", crate::plain_lines(&line));
    }
    println!();
    for line in device_grid::plain(&grid) {
        println!("{line}");
    }
    if !read.sharing {
        println!("\n{}", s.tr.text("devices-alone", None));
    }
    Ok(())
}

/// The device a word names among the table's rows: its id, else the one
/// device shown under that name (yours, or its system's), capitals aside.
fn named<'a>(s: &Session, rows: &'a [Row], word: &str) -> Result<&'a Row, String> {
    let word = word.trim();
    if let Some(row) = rows.iter().find(|row| row.id == word) {
        return Ok(row);
    }
    let same = |name: &str| !name.is_empty() && name.to_lowercase() == word.to_lowercase();
    let found: Vec<&Row> = rows.iter().filter(|row| same(&row.name) || same(&row.given) || same(&row.system)).collect();
    match found.as_slice() {
        [one] => Ok(one),
        [] => Err(s.say("devices-unknown", &[("device", crate::plain_lines(word))])),
        _ => Err(s.say("devices-several", &[("device", crate::plain_lines(word))])),
    }
}

/// `sioul devices for <device> <areas>`: what the device is for, written
/// into the shared settings; "everything" takes it out.
fn say_for(s: &Session, device: &str, areas: &str) -> Result<(), String> {
    let (_, rows, _) = rows(s);
    let row = named(s, &rows, device)?;
    let everything = matches!(areas.trim().to_lowercase().as_str(), "" | "everything" | "all" | "tout");
    // What a device is for takes no "none" (nothing ticked is everything): its own refusal says so (review of phase 2, L4).
    let area = if everything { None } else { Some(Area::parse(areas).ok_or_else(|| s.say("devices-for-refused", &[("said", crate::plain_lines(areas.trim()))]))?) };
    device_said::set_area(&s.config_path, &row.id, &area.map(Area::id).unwrap_or_default())?;
    println!("{}", s.say("devices-said-for", &[("device", crate::plain_lines(&row.name)), ("areas", areas_in_words(&s.tr, "device-areas", area.unwrap_or(Area::ALL)))]));
    Ok(())
}

/// `sioul devices at <device> <time> <areas>`: one cell of the device's
/// line, written into the shared settings; "usual" puts it back as the
/// device's areas say, "none" lets nothing come then.
fn say_at(s: &Session, device: &str, time: &str, areas: &str) -> Result<(), String> {
    let (_, rows, _) = rows(s);
    let row = named(s, &rows, device)?;
    // The five times by their own words only, as the refusal names them (review of phase 2, L4):
    // the settings still read an older word for a time (`device_said::cell_time`).
    let at = sioul_core::areas::Time::STATES.into_iter().find(|t| t.id() == time.trim().to_lowercase()).ok_or_else(|| s.say("devices-time-refused", &[("said", crate::plain_lines(time.trim()))]))?;
    let cell = device_said::said_cell(areas).ok_or_else(|| s.say("devices-cell-refused", &[("said", crate::plain_lines(areas.trim()))]))?;
    device_said::set_cell(&s.config_path, &row.id, at, cell)?;
    // What comes forward there then, as the settings now say it.
    let line = sioul_core::config::Config::load(&s.config_path).ok().and_then(|config| config.devices.get(&row.id).map(device_said::Said::line)).unwrap_or_default();
    // The week's hours, for what a cell left as its areas say brings (admin at work while it has no hours of its own).
    let week = sioul_core::quiet::mode(&s.config.week_hours(), &s.config.time_off, &sioul_core::quiet::Overrides::default(), &sioul_core::quiet::Blocks::default(), &sioul_core::zone::now()).week;
    let pairs = [("device", crate::plain_lines(&row.name)), ("time", device_grid::time_title(at, &s.tr)), ("areas", device_grid::cell_label(line.cell(at, week), &s.tr))];
    println!("{}", s.say(if cell.is_some() { "devices-said-cell" } else { "devices-said-cell-usual" }, &pairs));
    Ok(())
}
