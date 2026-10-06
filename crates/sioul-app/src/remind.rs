// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Reminders before dates, for the window (docs/reminders.md): each minute,
//! what is due becomes one quiet notification with "Open"; with the window
//! closed, `sioul remind --watch` tells them, started with the session when
//! asked on the Parameters page. Each is marked when told: told once, whichever runs.

use crate::backend::{QtThread, load_config, tr};
use cxx_qt_lib::QString;
use sioul_core::reminders::{self, Kind};
use std::path::PathBuf;
use std::sync::Mutex;

static BUSY: Mutex<()> = Mutex::new(());

/// What is due, told; off the window's thread.
pub(crate) fn tick(qt: &QtThread) {
    let qt = qt.clone();
    std::thread::spawn(move || {
        let Some(_busy) = crate::backend::one_at_a_time(&BUSY) else { return };
        let now = jiff::Zoned::now();
        // Asleep, nothing is told: it waits for waking (`Wait::Everything`).
        let (all, wait) = reminders::gather(&load_config(), tr(), &now);
        for reminder in reminders::to_tell(all, &reminders::told_dir(), now.timestamp().as_second(), wait) {
            let (kind, uri, key) = match reminder.kind {
                Kind::Asked | Kind::Wait => ("task", reminder.target.clone(), reminder.target.trim_start_matches("sioul:task/").to_string()),
                Kind::Event | Kind::Alarm => ("event", String::new(), reminder.target.clone()),
                Kind::Payment => ("budget", reminder.target.clone(), String::new()),
                Kind::Paper => ("paper", reminder.target.clone(), reminder.target.trim_start_matches("sioul:paper/").to_string()),
                Kind::Contract => ("contract", reminder.target.clone(), reminder.target.trim_start_matches("sioul:contract/").to_string()),
                Kind::Money => ("budget", reminder.target.clone(), String::new()),
            };
            let qt = qt.clone();
            let open: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = qt.queue(move |mut sioul| sioul.as_mut().reminder_opened(QString::from(kind), QString::from(&uri), QString::from(&key)));
            });
            let _ = sioul_sync::notify::remind(&reminder.title, &reminder.body, Some((tr().text("reminder-open", None), open)));
        }
    });
}

/// `sioul`, next to this program, else on the PATH.
fn command() -> Option<PathBuf> {
    let name = format!("sioul{}", std::env::consts::EXE_SUFFIX);
    let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join(&name))).filter(|p| p.is_file());
    beside.or_else(|| std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|dir| dir.join(&name)).find(|p| p.is_file())))
}

/// Where the session is told to start `sioul remind --watch`: the desktop's
/// autostart folder, or macOS's launch agents. None on Windows, not done yet.
fn entry() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return Some(sioul_core::config::expand_home("~/Library/LaunchAgents/org.sioul.reminders.plist"));
    }
    if cfg!(windows) {
        return None;
    }
    sioul_core::config::config_dir().parent().map(|dir| dir.join("autostart").join("sioul-reminders.desktop"))
}

/// Whether reminders come with the window closed.
pub(crate) fn background() -> bool {
    entry().is_some_and(|e| e.is_file())
}

/// Reminders with the window closed, or not: the entry written or taken out, the watcher started or stopped now.
pub(crate) fn set_background(on: bool) -> Result<(), String> {
    let entry = entry().ok_or_else(|| tr().text("reminders-closed-unavailable", None))?;
    if !on {
        let _ = std::fs::remove_file(&entry);
        stop();
        return Ok(());
    }
    let sioul = command().ok_or_else(|| tr().text("reminders-closed-no-command", None))?;
    let text = if cfg!(target_os = "macos") {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key><string>org.sioul.reminders</string>\n  <key>ProgramArguments</key>\n  <array><string>{}</string><string>remind</string><string>--watch</string></array>\n  <key>RunAtLoad</key><true/>\n</dict>\n</plist>\n",
            sioul.display()
        )
    } else {
        // Its name and comment show in the desktop's list of what starts with the session.
        let line = |id: &str| tr().text(id, None).replace(['\n', '\r'], " ");
        format!(
            "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec=\"{}\" remind --watch\nIcon=appointment-soon\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
            line("reminders-entry-name"),
            line("reminders-entry-comment"),
            sioul.display()
        )
    };
    if let Some(parent) = entry.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(&entry, text).map_err(|e| format!("{}: {e}", entry.display()))?;
    // Now too, not only from the next session; a second one leaves at once.
    std::process::Command::new(&sioul)
        .args(["remind", "--watch"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(drop)
        .map_err(|e| format!("{}: {e}", sioul.display()))
}

/// Stops the watcher started earlier, by its number: only while a watcher
/// holds its lock (`sioul remind --watch`), since a number left by one gone
/// may be another program's by now.
fn stop() {
    let state = sioul_core::config::state_dir();
    let path = state.join("remind.pid");
    let running = std::fs::OpenOptions::new().write(true).open(state.join("remind.lock")).is_ok_and(|lock| matches!(lock.try_lock(), Err(std::fs::TryLockError::WouldBlock)));
    let pid = std::fs::read_to_string(&path).ok().and_then(|t| t.trim().parse::<u32>().ok()).filter(|_| running);
    if let Some(pid) = pid {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
        #[cfg(windows)]
        let _ = std::process::Command::new("taskkill").args(["/PID", &pid.to_string()]).status();
    }
    let _ = std::fs::remove_file(path);
}
