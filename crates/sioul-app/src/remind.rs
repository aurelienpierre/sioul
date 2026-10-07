// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Reminders before dates, for the window (docs/reminders.md): each minute,
//! what is due becomes one quiet notification with "Open"; with the window
//! closed, `sioul remind --watch` tells them, started with the session when
//! asked on the Parameters page. Each is marked when told: told once, whichever runs.
//! On a phone, the events' reminders only, through Android's notifications
//! (`eventalarms`, whose alarms tell them while Sioul is away); the others
//! wait for a computer.

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
        // What holds now is the notification matrix's to say (Settings ▸ Reminders and
        // notifications): asleep, as usual, nothing but what you chose to happen then.
        let (mut all, holds) = reminders::gather(&load_config(), tr(), &now);
        let holds = crate::hours::with_layers(holds);
        let phone = cfg!(target_os = "android");
        if phone {
            all.retain(|r| matches!(r.kind, Kind::Event | Kind::Before | Kind::Alarm));
        }
        let events = if phone { all.clone() } else { Vec::new() };
        for reminder in reminders::to_tell(all, &reminders::told_dir(), now.timestamp().as_second(), &holds) {
            if phone {
                crate::eventalarms::show(&reminder);
                continue;
            }
            let (kind, uri, key) = match reminder.kind {
                Kind::Asked | Kind::Wait => ("task", reminder.target.clone(), reminder.target.trim_start_matches("sioul:task/").to_string()),
                Kind::Event | Kind::Before | Kind::Alarm => ("event", String::new(), reminder.target.clone()),
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
        // On a phone, the coming ones (those just told left out) given to
        // Android's alarm clock when they changed: they come with Sioul away too.
        if phone {
            crate::eventalarms::schedule_from(&events, &holds, &now);
        }
    });
}

/// An event's own reminder as the form and the details say it: "" as usual,
/// "none", or minutes ("15").
fn remind_word(value: &str) -> String {
    match reminders::Remind::read(value) {
        reminders::Remind::Usual => String::new(),
        reminders::Remind::Never => "none".to_string(),
        reminders::Remind::Minutes(minutes) => minutes.to_string(),
    }
}

/// The choices of an event's reminder before it, as its form and its
/// details offer them, [{value, label}]: "As usual (15 minutes)", "Not this
/// one", "5 minutes before"… "2 hours before"; `current` among them when an
/// event says another time (set elsewhere).
pub(crate) fn choices(current: &str) -> String {
    choices_with(tr(), load_config().reminders.before_event, current).to_string()
}

/// `choices`, said by `tr`, the usual time `usual` minutes.
fn choices_with(tr: &sioul_core::i18n::Translator, usual: u32, current: &str) -> serde_json::Value {
    let lead = |id: &str, minutes: u32| {
        let mut args = sioul_core::i18n::args();
        args.set("lead", reminders::lead_text(tr, minutes));
        tr.text(id, Some(&args))
    };
    let mut minutes: Vec<u32> = reminders::LEADS.iter().copied().filter(|m| *m > 0).collect();
    if let reminders::Remind::Minutes(own) = reminders::Remind::read(current)
        && !minutes.contains(&own)
    {
        minutes.push(own);
        minutes.sort_unstable();
    }
    let mut out = vec![
        serde_json::json!({ "value": "", "label": lead("event-remind-usual", usual) }),
        serde_json::json!({ "value": "none", "label": tr.text("event-remind-none", None) }),
    ];
    out.extend(minutes.into_iter().map(|m| serde_json::json!({ "value": m.to_string(), "label": lead("event-remind-before", m) })));
    serde_json::Value::Array(out)
}

/// An event's reminder before it, as its details say it (AgendaPage.qml),
/// the occurrence starting at `start`: {line, remind, writable}: "Reminder:
/// 13:15" (with its day when another), or "No reminder before it"; its own
/// choice; whether it can be changed here. "" when the event is not found.
pub(crate) fn of_event(key: &str, start: i64) -> String {
    let Some(path) = crate::pim::ours(key) else { return String::new() };
    let Some(calendar) = crate::pim::collection_of(&path) else { return String::new() };
    let zone = jiff::tz::TimeZone::system();
    let found = sioul_core::agenda::file_occurrences(&path, &calendar, start, start + 1, &zone);
    // A whole day is reminded the working day before only: nothing to say or choose here.
    let Some(event) = found.iter().find(|o| o.start == start && !o.all_day) else { return String::new() };
    let config = load_config();
    let line = match reminders::before_at(event, config.reminders.before_event).and_then(|at| jiff::Timestamp::from_second(at).ok()) {
        Some(at) => {
            let at = at.to_zoned(zone.clone());
            let day = jiff::Timestamp::from_second(event.start).map(|s| s.to_zoned(zone.clone()).date()).ok();
            let when = if day == Some(at.date()) { at.strftime("%H:%M").to_string() } else { tr().date(&at, true) };
            crate::backend::say("event-reminds", &[("when", when)])
        }
        None => tr().text("event-reminds-none", None),
    };
    serde_json::json!({ "line": line, "remind": remind_word(&event.remind), "writable": !calendar.read_only }).to_string()
}

/// An event's own reminder changed from its details, for every time it
/// comes (`value`: "" as usual, "none", "15"): written into its file and
/// sent, as its form saves it.
pub(crate) fn set_event(qt: &QtThread, shared: &std::sync::Arc<crate::backend::Shared>, key: &str, value: &str) -> Result<(), String> {
    let path = crate::pim::ours(key).ok_or_else(|| tr().text("agenda-gone", None))?;
    let mut edit = sioul_core::agenda::edit_of(&path).ok_or_else(|| tr().text("agenda-gone", None))?;
    edit.remind = remind_word(value);
    let json = serde_json::to_string(&edit).map_err(|e| e.to_string())?;
    crate::pim::save_event(qt, shared, key, &json, "")
}

/// `sioul`, next to this program, else on the PATH: the reminders' watcher, the spam filter's training by itself.
pub(crate) fn command() -> Option<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_events_reminder_choices() {
        let en = sioul_core::i18n::Translator::new("en");
        let offered = choices_with(&en, 15, "");
        let values: Vec<&str> = offered.as_array().unwrap().iter().map(|c| c["value"].as_str().unwrap()).collect();
        assert_eq!(values, ["", "none", "5", "10", "15", "30", "60", "120"]);
        let labels: Vec<&str> = offered.as_array().unwrap().iter().map(|c| c["label"].as_str().unwrap()).collect();
        assert_eq!(labels, ["As usual (15 minutes)", "Not this one", "5 minutes before", "10 minutes before", "15 minutes before", "30 minutes before", "1 hour before", "2 hours before"]);
        // Another time, set elsewhere, kept among them; the usual time none.
        let other = choices_with(&sioul_core::i18n::Translator::new("fr"), 0, "20");
        let values: Vec<&str> = other.as_array().unwrap().iter().map(|c| c["value"].as_str().unwrap()).collect();
        assert_eq!(values, ["", "none", "5", "10", "15", "20", "30", "60", "120"]);
        assert_eq!(other[0]["label"], "Comme d’habitude (aucun)");
        assert_eq!(other[5]["label"], "20 minutes avant");
        assert_eq!((remind_word("NONE"), remind_word("15"), remind_word("")), ("none".to_string(), "15".to_string(), String::new()));
    }
}
