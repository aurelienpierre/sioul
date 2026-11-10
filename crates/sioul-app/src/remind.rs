// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Reminders before dates, for the window (docs/reminders.md): each minute,
//! what is due becomes one quiet notification with "Open"; with the window
//! closed, `sioul remind --watch` tells them, started with the session when
//! asked on the Parameters page. Each is marked when told: told once, whichever runs.
//! On a phone, the events' reminders only, through Android's notifications
//! (`eventalarms`, whose alarms tell them while Sioul is away); the others
//! wait for a computer.

use crate::backend::{QtThread, load_config, say, tr};
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
        let now = sioul_core::zone::now();
        // What holds now is the notification matrix's to say (Settings ▸ Reminders and
        // notifications): asleep, as usual, nothing but what you chose to happen then.
        let (mut all, holds) = reminders::gather(&load_config(), tr(), &now);
        // The phone's card says the dates, waits, payments and papers due (homecard.rs).
        crate::homecard::reminders_seen(&all);
        let holds = crate::hours::with_layers(holds);
        let phone = cfg!(target_os = "android");
        if phone {
            all.retain(|r| matches!(r.kind, Kind::Event | Kind::Before | Kind::Alarm));
        }
        let events = if phone { all.clone() } else { Vec::new() };
        let mut told = reminders::to_tell(all, &reminders::told_dir(), now.timestamp().as_second(), &holds);
        // Waits over in the same minute (the start of work after a weekend):
        // one notice for them all, as new mail comes, never one each (the
        // check of ef8c0ca, low 10).
        let waits: Vec<reminders::Reminder> = if phone { Vec::new() } else { told.iter().filter(|r| r.kind == Kind::Wait).cloned().collect() };
        if waits.len() > 1 {
            told.retain(|r| r.kind != Kind::Wait);
            let tasks: Vec<String> = waits.iter().map(|r| format!("“{}”", r.title)).collect();
            let title = say("reminder-waits-over", &[("n", waits.len().to_string()), ("tasks", tasks.join(", "))]);
            let qt = qt.clone();
            let open: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = qt.queue(move |mut sioul| sioul.as_mut().reminder_opened(QString::from("task"), QString::default(), QString::default()));
            });
            let _ = sioul_sync::notify::remind(&title, "", Some((tr().text("reminder-open", None), open)));
        }
        for reminder in told {
            if phone {
                crate::eventalarms::show(&reminder);
                continue;
            }
            let (kind, uri, key) = opens(&reminder);
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

/// What a reminder's "Open" opens, as the window's `openThing` takes it: its
/// kind, its address, its key.
fn opens(reminder: &reminders::Reminder) -> (&'static str, String, String) {
    match reminder.kind {
        Kind::Asked | Kind::Wait => ("task", reminder.target.clone(), reminder.target.trim_start_matches("sioul:task/").to_string()),
        Kind::Event | Kind::Before | Kind::Alarm => ("event", String::new(), event_time(&reminder.target, reminder.starts)),
        Kind::Payment => ("budget", reminder.target.clone(), String::new()),
        Kind::Paper => ("paper", reminder.target.clone(), reminder.target.trim_start_matches("sioul:paper/").to_string()),
        Kind::Contract => ("contract", reminder.target.clone(), reminder.target.trim_start_matches("sioul:contract/").to_string()),
        Kind::Money => ("budget", reminder.target.clone(), String::new()),
    }
}

/// What an event's reminder opens: its file and the time it reminds of
/// ("…/yoga.ics#1793520000"), so that its card shows that time of a series,
/// not its next, the meeting over or not (review of 590eb18, later 6); its
/// file alone when no time is known. The window reads it back
/// (`CardHost.eventTime`); a file's name never ends in "#" and digits (".ics").
pub(crate) fn event_time(target: &str, starts: Option<i64>) -> String {
    match starts {
        Some(start) if start > 0 => format!("{target}#{start}"),
        _ => target.to_string(),
    }
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
    let zone = sioul_core::zone::current();
    let found = sioul_core::agenda::file_occurrences(&path, &calendar, start, start + 1, &zone);
    // A whole day is reminded the working day before only: nothing to say or choose here. A
    // child's event reminds nobody itself: whoever takes it is reminded by their stand-in (part 3).
    let Some(event) = found.iter().find(|o| o.start == start && !o.all_day && !o.child) else { return String::new() };
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
    // A meeting with others: from your own time around it, where its reminder is kept (`pim::write_event`).
    let mut edit = crate::pim::edit_with_own(&path).ok_or_else(|| tr().text("agenda-gone", None))?;
    edit.remind = remind_word(value);
    // A meeting, yours to organize too: your own file alone, never its file nor its door (review of 1e8551e, F13).
    if std::fs::read_to_string(&path).is_ok_and(|text| crate::pim::with_others(&text)) {
        return crate::pim::save_own_time(qt, shared, key, &edit);
    }
    let json = serde_json::to_string(&edit).map_err(|e| e.to_string())?;
    crate::pim::save_event(qt, shared, key, &json, "")
}

/// `sioul`, next to this program, else on the PATH: the reminders' watcher, the spam filter's training by itself.
pub(crate) fn command() -> Option<PathBuf> {
    let name = format!("sioul{}", std::env::consts::EXE_SUFFIX);
    let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join(&name))).filter(|p| p.is_file());
    beside.or_else(|| std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|dir| dir.join(&name)).find(|p| p.is_file())))
}

/// Whether reminders come with the window closed: the entry is there and
/// the desktop did not turn it off (`reminders::watcher_wanted`).
pub(crate) fn background() -> bool {
    reminders::watcher_wanted()
}

/// Reminders with the window closed, or not: the entry written or taken out, the watcher started or stopped now.
pub(crate) fn set_background(on: bool) -> Result<(), String> {
    let entry = reminders::watcher_entry().ok_or_else(|| tr().text("reminders-closed-unavailable", None))?;
    if !on {
        let _ = std::fs::remove_file(&entry);
        if let Some(restart) = reminders::watcher_restart() {
            let _ = std::fs::remove_file(&restart);
            let _ = restart.parent().map(std::fs::remove_dir);
        }
        stop();
        return Ok(());
    }
    let sioul = command().ok_or_else(|| tr().text("reminders-closed-no-command", None))?;
    // Turned on here: the whole entry, any "off" a desktop wrote in it gone.
    write_entries(&entry, &sioul, true)?;
    // Now too, not only from the next session; a second one leaves at once.
    start_watcher(&sioul)
}

/// The session's entry for the watcher, and on Linux systemd's word to start
/// it again. `whole` (the setting turned on here): the entry as Sioul writes
/// it. Else (the window's start) an entry is changed only where its program
/// or its arguments say otherwise (a Sioul before 10 November 2026 wrote no
/// `--session`; the program moved): its `Exec=` line alone, or its
/// `ProgramArguments`, every other line kept, a desktop's "off" among them
/// (review of wt/mail, F2).
fn write_entries(entry: &std::path::Path, sioul: &std::path::Path, whole: bool) -> Result<(), String> {
    let line = |id: &str| tr().text(id, None).replace(['\n', '\r'], " ");
    let made = entry_text(sioul, &line("reminders-entry-name"), &line("reminders-entry-comment"));
    let text = match std::fs::read_to_string(entry) {
        Ok(kept) if !whole => with_program_of(&kept, &made),
        _ => made,
    };
    let mut files = vec![(entry.to_path_buf(), text)];
    files.extend(reminders::watcher_restart().map(|restart| (restart, RESTART.to_string())));
    for (path, text) in files {
        if std::fs::read_to_string(&path).is_ok_and(|kept| kept == text) {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// `kept`, an entry as it is, with the program and arguments of `made`, the
/// entry Sioul would write: its `Exec=` line (or its `ProgramArguments`
/// array) taken from `made`, nothing else changed.
fn with_program_of(kept: &str, made: &str) -> String {
    if kept.contains("<plist") {
        let array = |text: &str| -> Option<(usize, usize)> {
            let key = text.find("<key>ProgramArguments</key>")?;
            let start = key + text[key..].find("<array>")?;
            let end = start + text[start..].find("</array>")? + "</array>".len();
            Some((start, end))
        };
        return match (array(kept), array(made)) {
            (Some((a, b)), Some((c, d))) => format!("{}{}{}", &kept[..a], &made[c..d], &kept[b..]),
            _ => kept.to_string(),
        };
    }
    let Some(exec) = made.lines().find(|line| line.starts_with("Exec=")) else { return kept.to_string() };
    let mut replaced = false;
    let mut lines: Vec<&str> = kept
        .lines()
        .map(|line| {
            if line.starts_with("Exec=") && !replaced {
                replaced = true;
                exec
            } else {
                line
            }
        })
        .collect();
    if !replaced {
        lines.push(exec);
    }
    lines.join("\n") + "\n"
}

/// The entry that starts `sioul remind --watch --session` with the session:
/// a launch agent on macOS, started again after a crash (`Crashed`: never
/// after an exit of its own, so an older program refusing `--session` does
/// not loop), an XDG autostart entry elsewhere, whose name and
/// comment show in the desktop's list of what starts with the session.
fn entry_text(sioul: &std::path::Path, name: &str, comment: &str) -> String {
    if cfg!(target_os = "macos") {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key><string>org.sioul.reminders</string>\n  <key>ProgramArguments</key>\n  <array><string>{}</string><string>remind</string><string>--watch</string><string>--session</string></array>\n  <key>RunAtLoad</key><true/>\n  <key>KeepAlive</key><dict><key>Crashed</key><true/></dict>\n  <key>ThrottleInterval</key><integer>60</integer>\n</dict>\n</plist>\n",
            xml_escaped(&sioul.display().to_string())
        )
    } else {
        format!("[Desktop Entry]\nType=Application\nName={name}\nComment={comment}\nExec=\"{}\" remind --watch --session\nIcon=appointment-soon\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n", sioul.display())
    }
}

fn xml_escaped(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// systemd's word for the unit it makes of the entry (Linux): started again
/// ten seconds after it ends otherwise than by systemd or by its own choice:
/// killed (a `killall sioul` typed to start Sioul again ends it too), crashed
/// (docs/reminders.md, "It keeps running"). A watcher that leaves on its own (one
/// runs already; reminders with the window closed turned off) exits with 0,
/// which is not started again; the session's end stops it through systemd.
/// Five starts in ten minutes at most (`StartLimitBurst`, `StartLimitIntervalSec`):
/// a program that cannot run (an older one, rolled back to, refusing
/// `--session`; one removed) is tried five times, then left until the next
/// session, rather than every ten seconds (review of wt/mail, F3).
const RESTART: &str = "# Sioul: the reminders' watcher started again when it ends otherwise than by systemd or by its own choice, five times in ten minutes at most (docs/reminders.md).\n[Unit]\nStartLimitIntervalSec=600\nStartLimitBurst=5\n[Service]\nRestart=on-failure\nRestartForceExitStatus=SIGTERM SIGINT SIGHUP SIGPIPE\nRestartSec=10\n";

/// The watcher started now, apart from the window: its own process group,
/// so that the end of a terminal the window was started from does not end it.
fn start_watcher(sioul: &std::path::Path) -> Result<(), String> {
    let mut command = std::process::Command::new(sioul);
    command.args(["remind", "--watch", "--session"]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command.spawn().map(drop).map_err(|e| format!("{}: {e}", sioul.display()))
}

/// At the window's start, with reminders with the window closed on: the
/// entries brought up to date (their program and arguments alone), and the
/// watcher started when none runs, only where no service manager runs it:
/// a unit systemd made of the entry, stopped or masked by you, and a
/// launch agent you turned off, stay so (review of wt/mail, F2); an XDG
/// autostart run by the desktop itself has no one else to start it again.
pub(crate) fn keep_watching() {
    if !background() || cfg!(target_os = "android") {
        return;
    }
    std::thread::spawn(|| {
        let Some(entry) = reminders::watcher_entry() else { return };
        // The program the entry names while it is there: a window started from
        // elsewhere (a build tried) never moves the session's watcher to it.
        let Some(sioul) = named_program(&entry).filter(|p| p.is_file()).or_else(command) else { return };
        if let Err(e) = write_entries(&entry, &sioul, false) {
            eprintln!("Sioul: the reminders' watcher: {e}");
        }
        if !managed() && !watching() && let Err(e) = start_watcher(&sioul) {
            eprintln!("Sioul: the reminders' watcher: {e}");
        }
    });
}

/// Whether a service manager runs the watcher, and so decides alone when it
/// starts: launchd on macOS; systemd where its XDG autostart generator made
/// a unit of the entry (loaded, or masked by you).
fn managed() -> bool {
    if cfg!(target_os = "macos") {
        return true;
    }
    if !cfg!(target_os = "linux") {
        return false;
    }
    let out = std::process::Command::new("systemctl").args(["--user", "show", "--property=LoadState", "--value", UNIT]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output();
    out.is_ok_and(|out| out.status.success() && matches!(String::from_utf8_lossy(&out.stdout).trim(), "loaded" | "masked"))
}

/// The unit systemd's XDG autostart generator makes of the entry.
const UNIT: &str = "app-sioul\\x2dreminders@autostart.service";

/// The program an entry starts: its `Exec`'s first word, quoted as Sioul
/// writes it; on macOS, the first of its `ProgramArguments`.
fn named_program(entry: &std::path::Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(entry).ok()?;
    let program = if cfg!(target_os = "macos") {
        text.split("<key>ProgramArguments</key>").nth(1)?.split("<string>").nth(1)?.split("</string>").next()?.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
    } else {
        let exec = text.lines().find_map(|line| line.strip_prefix("Exec="))?.trim();
        match exec.strip_prefix('"') {
            Some(quoted) => quoted.split('"').next()?.to_string(),
            None => exec.split_whitespace().next()?.to_string(),
        }
    };
    (!program.is_empty()).then(|| PathBuf::from(program))
}

/// Whether a watcher holds its lock (`sioul remind --watch`).
fn watching() -> bool {
    let lock = sioul_core::config::state_dir().join("remind.lock");
    std::fs::OpenOptions::new().write(true).open(lock).is_ok_and(|lock| matches!(lock.try_lock(), Err(std::fs::TryLockError::WouldBlock)))
}

/// Stops the watcher started earlier, by its number: only while a watcher
/// holds its lock (`sioul remind --watch`), since a number left by one gone
/// may be another program's by now.
fn stop() {
    let state = sioul_core::config::state_dir();
    let path = state.join("remind.pid");
    let pid = std::fs::read_to_string(&path).ok().and_then(|t| t.trim().parse::<u32>().ok()).filter(|_| watching());
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

    /// TODO §4 (10 November 2026): the watcher's entry starts it for the
    /// session (`--session`) and names its program, quoted, as
    /// `named_program` reads it back, an older entry's too; systemd's word
    /// starts it again after a kill, never after its own clean exit.
    #[test]
    fn the_watcher_s_entry_and_its_restart() {
        let program = PathBuf::from("/opt/Sioul & co/bin/sioul");
        let text = entry_text(&program, "Sioul reminders", "Reminders with the window closed");
        assert!(text.contains("remind --watch --session") || text.contains("<string>--session</string>"), "{text}");
        let dir = std::env::temp_dir().join(format!("sioul-remind-entry-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let entry = dir.join("entry");
        std::fs::write(&entry, &text).unwrap();
        assert_eq!(named_program(&entry), Some(program));
        if !cfg!(target_os = "macos") {
            std::fs::write(&entry, "[Desktop Entry]\nType=Application\nExec=\"/usr/bin/sioul\" remind --watch\n").unwrap();
            assert_eq!(named_program(&entry), Some(PathBuf::from("/usr/bin/sioul")), "an entry from before");
        }
        assert!(RESTART.contains("\nRestart=on-failure\n") && RESTART.contains("RestartForceExitStatus=SIGTERM SIGINT SIGHUP SIGPIPE\n"));
        // Review of wt/mail, F3: five starts in ten minutes at most.
        assert!(RESTART.contains("[Unit]\nStartLimitIntervalSec=600\nStartLimitBurst=5\n[Service]"));
        // F2: a desktop's "off" kept; only the program and its arguments brought up to date.
        if !cfg!(target_os = "macos") {
            let kept = "[Desktop Entry]\nType=Application\nName=Sioul reminders\nExec=\"/usr/bin/sioul\" remind --watch\nHidden=true\nX-GNOME-Autostart-enabled=false\n";
            let made = entry_text(&PathBuf::from("/opt/sioul/bin/sioul"), "Sioul reminders", "Reminders");
            let changed = with_program_of(kept, &made);
            assert_eq!(changed, "[Desktop Entry]\nType=Application\nName=Sioul reminders\nExec=\"/opt/sioul/bin/sioul\" remind --watch --session\nHidden=true\nX-GNOME-Autostart-enabled=false\n");
            std::fs::write(&entry, &changed).unwrap();
            assert!(!reminders::entry_on(&entry), "turned off by the desktop: off");
            std::fs::write(&entry, &made).unwrap();
            assert!(reminders::entry_on(&entry));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An event's reminder opens that time of it, its card on that time of a
    /// series, not its next (review of 590eb18, later 6); a task's its task.
    #[test]
    fn an_event_s_reminder_opens_its_time() {
        let event = reminders::Reminder { key: "e".into(), kind: Kind::Before, at: 1_793_519_100, until: 1_793_520_000, title: "Yoga".into(), body: String::new(), target: "/c/yoga.ics".into(), work: false, starts: Some(1_793_520_000), names: None, event: None };
        assert_eq!(opens(&event), ("event", String::new(), "/c/yoga.ics#1793520000".to_string()));
        assert_eq!(opens(&reminders::Reminder { starts: None, ..event.clone() }).2, "/c/yoga.ics", "no time known: its file");
        let task = reminders::Reminder { kind: Kind::Asked, target: "sioul:task/t1".into(), starts: None, ..event };
        assert_eq!(opens(&task), ("task", "sioul:task/t1".to_string(), "t1".to_string()));
    }

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
