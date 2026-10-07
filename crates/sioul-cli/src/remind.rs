// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul remind`: reminders before dates (docs/reminders.md), listed, or told
//! with Sioul's window closed (`--watch`, started with the session when asked
//! on the Parameters page). The window tells the same ones; each is marked
//! when told, so it is told once, whichever runs.

use crate::Session;
use jiff::Zoned;
use sioul_core::config::{self, Config};
use sioul_core::reminders;

/// What comes in the next two weeks, with what was told already.
pub(crate) fn list(s: &Session) -> Result<(), String> {
    let now = Zoned::now();
    let (all, _) = reminders::gather(&s.config, &s.tr, &now);
    let stamp = now.timestamp().as_second();
    let dir = reminders::told_dir();
    let coming: Vec<&reminders::Reminder> = all.iter().filter(|r| r.until > stamp && r.at < stamp + 14 * 86_400).collect();
    if coming.is_empty() {
        println!("{}", s.tr.text("remind-nothing", None));
    }
    for reminder in coming {
        let at = jiff::Timestamp::from_second(reminder.at).map(|t| t.to_zoned(now.time_zone().clone())).map_err(|e| e.to_string())?;
        let told = if reminders::told(&dir, &reminder.key) { format!(" ({})", s.tr.text("remind-told", None)) } else { String::new() };
        // Titles of tasks, events and mail: one line each, never a terminal's escape sequences.
        println!("{}{told}\n    {}{}", s.tr.date(&at, true), crate::one_line(&reminder.title), if reminder.body.is_empty() { String::new() } else { format!(" — {}", crate::one_line(&reminder.body)) });
    }
    Ok(())
}

/// Stays open: each minute, what is due becomes one quiet notification. One
/// at a time: a second `sioul remind --watch` leaves at once.
pub(crate) fn watch(s: &Session) -> Result<(), String> {
    let lock_path = config::state_dir().join("remind.lock");
    std::fs::create_dir_all(config::state_dir()).map_err(|e| e.to_string())?;
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&lock_path).map_err(|e| format!("{}: {e}", lock_path.display()))?;
    // Held as long as the process lives.
    if lock.try_lock().is_err() {
        println!("{}", s.tr.text("remind-running", None));
        return Ok(());
    }
    // Its number, for the window to stop it when asked.
    let _ = std::fs::write(config::state_dir().join("remind.pid"), std::process::id().to_string());
    println!("{}", s.tr.text("remind-watching", None));
    let dir = reminders::told_dir();
    let mut forgotten_on = None;
    loop {
        // Read again each time: settings, tasks and events change while it runs.
        let config = Config::load(&s.config_path).unwrap_or_default();
        let now = Zoned::now();
        if forgotten_on != Some(now.date()) {
            reminders::forget_old(&dir);
            forgotten_on = Some(now.date());
        }
        // What holds now, as the notification matrix says (a slot of time for you is the window's alone).
        let (all, holds) = reminders::gather(&config, &s.tr, &now);
        for reminder in reminders::to_tell(all, &dir, now.timestamp().as_second(), &holds) {
            if let Err(e) = sioul_sync::notify::remind(&reminder.title, &reminder.body, None) {
                eprintln!("{e}");
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
