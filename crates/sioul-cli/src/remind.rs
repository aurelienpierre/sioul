// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul remind`: reminders before dates (docs/reminders.md), listed, or told
//! with Sioul's window closed (`--watch`, started with the session when asked
//! on the Parameters page). The window tells the same ones; each is marked
//! when told, so it is told once, whichever runs.

use crate::Session;
use sioul_core::config::{self, Config};
use sioul_core::reminders;

/// What comes in the next two weeks, with what was told already.
pub(crate) fn list(s: &Session) -> Result<(), String> {
    let now = sioul_core::zone::now();
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

/// Stays open: each minute, what is due becomes one notice, as the window
/// tells it (docs/notifications.md), with neither buttons nor a click of its
/// own (a click brings Sioul's window forward, where one runs). One
/// at a time: a second `sioul remind --watch` leaves at once. Started for
/// the session (`--session`: the entry of reminders with the window closed),
/// it leaves once that entry is gone, with 0, which nothing starts again.
/// Its program followed (mcp/follow.rs): an install's new program takes its
/// place within a minute, the same process going on (TODO §4, 10 November
/// 2026: an install left the old program running until the session ended).
pub(crate) fn watch(s: &Session, for_session: bool) -> Result<(), String> {
    if for_session && !wanted() {
        return Ok(());
    }
    let lock_path = config::state_dir().join("remind.lock");
    std::fs::create_dir_all(config::state_dir()).map_err(|e| e.to_string())?;
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&lock_path).map_err(|e| format!("{}: {e}", lock_path.display()))?;
    // Held as long as the process lives; let go when it runs a new program
    // in its place (the descriptor closes on exec), which takes it again.
    if lock.try_lock().is_err() {
        println!("{}", s.tr.text("remind-running", None));
        return Ok(());
    }
    // Its number, for the window to stop it when asked.
    let _ = std::fs::write(config::state_dir().join("remind.pid"), std::process::id().to_string());
    println!("{}", s.tr.text("remind-watching", None));
    let dir = reminders::told_dir();
    let mut forgotten_on = None;
    let mut follow = crate::mcp::follow::Follow::start();
    loop {
        // Read again each time: settings, tasks and events change while it runs.
        let config = Config::load(&s.config_path).unwrap_or_default();
        let now = sioul_core::zone::now();
        if forgotten_on != Some(now.date()) {
            reminders::forget_old(&dir);
            forgotten_on = Some(now.date());
        }
        // What holds now, as the matrix of what reaches you says, its layers read
        // from their files: today's slots of time for you, do-not-disturb.
        let (all, holds) = reminders::gather(&config, &s.tr, &now);
        let stamp = now.timestamp().as_second();
        for reminder in reminders::to_tell(all, &dir, stamp, &holds) {
            // As the window tells it (docs/notifications.md), without its buttons: nothing here opens a thing.
            let notice = reminder.notice("");
            let told = sioul_core::notice::told(Some(reminders::row_of(reminder.kind)));
            if let sioul_core::notice::Decision::Post(shape) = sioul_core::notice::decide(&notice, &told, sioul_core::notice::moment_from_files(&config, &now), config.notices.minutes(), stamp) {
                sioul_sync::notify::teller().post(notice, shape, "", None);
            }
        }
        // A minute, its program and its entry looked at every five seconds.
        for _ in 0..12 {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if for_session && !wanted() {
                return Ok(());
            }
            if let Some(follow) = follow.as_mut() {
                follow.step(None, |said| eprintln!("sioul remind: {said}."));
            }
        }
    }
}

/// Whether reminders still come with the window closed: their entry is
/// there, and the desktop did not turn it off (`reminders::watcher_wanted`).
fn wanted() -> bool {
    reminders::watcher_wanted()
}
