// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul devices`: your devices in one table (docs/database.md, "Devices"),
//! the one Settings ▸ Devices shows (`sioul_sync::device_table`): this
//! device first, then each device Sioul shares with: its name, yours when
//! you gave one, what it is, how it is now, when it last opened and last
//! shared, its Sioul, what you say it is for, what it shares.
//!
//! Read only, from this device's own files: its entry as its window last
//! wrote it, what you said of each device in the settings, and what the
//! window last kept of the others, said as of then when no window looks
//! now. With the sharing's key (asked of the keyring only when a sharing
//! folder is set), the devices the sharing alone knows are listed too. With
//! sharing off, this device alone.

use crate::Session;
use sioul_sync::device_table;

pub(crate) fn run(s: &Session, json: bool) -> Result<(), String> {
    let state = sioul_core::config::state_dir();
    let now = jiff::Timestamp::now().as_second();
    let shared = sioul_sync::share::Here::load(&state).folder_path().is_some();
    let key = if shared { sioul_sync::secret::named(sioul_sync::share::KEY_NAME).and_then(|key| crate::invoices::unhex(&key)) } else { None };
    let read = device_table::read(&state, &s.config, key.as_ref(), now);
    let zone = sioul_core::zone::current();
    let rows = device_table::table(&read.facts(&s.config.devices, now, zone.clone()), &s.tr);
    if json {
        println!("{}", serde_json::json!({ "sharing": read.sharing, "looked": read.looked, "devices": rows }));
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
    if !read.sharing {
        println!("\n{}", s.tr.text("devices-alone", None));
    }
    Ok(())
}
