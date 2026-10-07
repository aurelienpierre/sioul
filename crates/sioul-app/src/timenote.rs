// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The time running, in the system's notifications (docs/tasks.md): while a
//! focus session runs, one notification says its task and since when, with
//! Pause (Go on, while paused) and Stop, which do what the focus window's own
//! buttons do. It follows the session from wherever it changes: the window,
//! the notification itself, another device (the sharing brings
//! `time/running.toml`), the command line. On a computer, the desktop's
//! (`sioul_sync::notify::ongoing`), while Sioul runs. On a phone, Android's
//! own, with a chronometer (android/main.cpp, TimeNote.java): it stays while
//! Sioul is away, and its buttons reach here without the window
//! (`sioul_time_action`, docs/android.md).

use crate::backend::{QtThread, Shared, load_config, tr};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::Serialize;
use sioul_core::cases::{Case, CaseStore};
use sioul_core::i18n::Translator;
use sioul_core::stopped::Stopped;
use sioul_core::tasks::{self, Task};
use sioul_core::timelog::{self, Running};
use std::ffi::c_char;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

/// What the notification says while a session runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Note {
    /// The task's title, else its project's, else "Focus".
    title: String,
    /// "Since 14:02, 25 min chosen"; paused, "Paused, 12 min so far".
    text: String,
    paused: bool,
    /// Where Android's chronometer counts from, Unix milliseconds: the start,
    /// the time paused left out; 0 while paused, the chronometer stopped.
    since: i64,
    /// The first button's word: "Pause", or "Go on" while paused.
    pause: String,
    stop: String,
    /// Android's channel for it, named in your language.
    channel: String,
}

/// The notification for the session running, if one is: `title` names its
/// task, `zone` tells the time it started.
fn note_of(running: Option<&Running>, title: &str, now: i64, tr: &Translator, zone: &TimeZone) -> Option<Note> {
    let running = running?;
    let paused = running.paused_at.is_some();
    let mut args = sioul_core::i18n::args();
    let text = if paused {
        args.set("minutes", running.elapsed(now) / 60);
        tr.text("focus-notification-paused", Some(&args))
    } else {
        args.set("time", Timestamp::from_second(running.start).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default());
        if running.planned == 0 {
            tr.text("focus-notification-open", Some(&args))
        } else {
            args.set("minutes", running.planned);
            tr.text("focus-notification-planned", Some(&args))
        }
    };
    Some(Note {
        title: if title.trim().is_empty() { tr.text("focus-title", None) } else { title.to_string() },
        text,
        paused,
        since: if paused { 0 } else { (running.start + running.paused) * 1000 },
        pause: tr.text(if paused { "focus-resume" } else { "focus-pause" }, None),
        stop: tr.text("focus-stop", None),
        channel: tr.text("focus-notification-channel", None),
    })
}

/// The task's title, else its first project's; "" when neither is known here
/// (a task not synced to this device yet).
fn title_of(uid: &str, tasks: &[Task], cases: &[Case]) -> String {
    let Some(task) = tasks.iter().find(|t| t.uid == uid) else { return String::new() };
    if !task.title.trim().is_empty() {
        return task.title.clone();
    }
    task.cases.iter().find_map(|id| cases.iter().find(|c| c.id == *id)).map(|c| c.title.clone()).unwrap_or_default()
}

/// What this process shows.
struct Shown {
    /// The note put up last; None until one was, in this process: a phone's
    /// may be left from an earlier one, the session over since.
    note: Option<Option<Note>>,
    /// The task named last, and its title, for a moment without the tasks read.
    named: (String, String),
    /// Sioul quits: nothing more is shown.
    over: bool,
    /// The desktop's notification, while one shows.
    #[cfg(not(target_os = "android"))]
    ongoing: Option<sioul_sync::notify::Ongoing>,
}

static SHOWN: Mutex<Shown> = Mutex::new(Shown {
    note: None,
    named: (String::new(), String::new()),
    over: false,
    #[cfg(not(target_os = "android"))]
    ongoing: None,
});

/// Sioul's window, when it runs in this process: a phone's buttons then act
/// as its own, and it follows.
static WINDOW: Mutex<Option<(QtThread, Arc<Shared>)>> = Mutex::new(None);

/// Buttons pressed one at a time: two in a row act in their order.
static ACTING: Mutex<()> = Mutex::new(());

/// The notification made to follow the session as it is now: put up, changed
/// or taken away. After each reading of the tasks, and at the window's minute
/// (what changed elsewhere: another device, the command line).
pub(crate) fn follow(qt: &QtThread, shared: &Arc<Shared>) {
    if let Ok(mut window) = WINDOW.lock()
        && window.is_none()
    {
        *window = Some((qt.clone(), Arc::clone(shared)));
    }
    post(Some((qt, shared)));
}

/// The note made again from the session on disk, and put up if it changed;
/// one at a time, so that the one put up last is the latest. Returns it.
fn post(window: Option<(&QtThread, &Arc<Shared>)>) -> Option<Note> {
    let mut shown = SHOWN.lock().unwrap_or_else(PoisonError::into_inner);
    if shown.over {
        return None;
    }
    let running = timelog::running();
    let title = running.as_ref().map(|r| title_now(&mut shown.named, &r.task, window.map(|(_, shared)| shared))).unwrap_or_default();
    let note = note_of(running.as_ref(), &title, Timestamp::now().as_second(), tr(), &TimeZone::system());
    // At a time the notification matrix says "Not at all" for it (as usual, never), the
    // note is taken away until that time ends; the session runs on.
    let note = note.filter(|_| crate::hours::comes(sioul_core::notify::Kind::Time));
    if shown.note.as_ref() != Some(&note) {
        put(&mut shown, note.as_ref(), window);
        shown.note = Some(note.clone());
        // Do-not-disturb while you focus follows the session at once (docs/do-not-disturb.md).
        std::thread::spawn(crate::everywhere::apply);
    }
    note
}

/// The title of the task `uid`: from the tasks the window read last; without
/// them (a phone's button, the window not running), from the task lists
/// themselves, once per task.
fn title_now(named: &mut (String, String), uid: &str, shared: Option<&Arc<Shared>>) -> String {
    let read = shared.and_then(|s| s.loaded.lock().ok().and_then(|l| l.clone()));
    let title = match read {
        Some(loaded) => title_of(uid, &loaded.tasks, &loaded.cases),
        None if named.0 == uid => return named.1.clone(),
        None => {
            let cases = load_config().case_store_path().and_then(|root| CaseStore::load(&root).ok()).map(|store| store.cases).unwrap_or_default();
            title_of(uid, &tasks::all(&TimeZone::system()), &cases)
        }
    };
    *named = (uid.to_string(), title.clone());
    title
}

/// The note put up, changed, or taken away: the desktop's notification, made
/// with the window, whose buttons it reaches.
#[cfg(not(target_os = "android"))]
fn put(shown: &mut Shown, note: Option<&Note>, window: Option<(&QtThread, &Arc<Shared>)>) {
    let Some(note) = note else {
        // Dropped, it is taken away.
        shown.ongoing = None;
        return;
    };
    let buttons = vec![
        (if note.paused { "resume" } else { "pause" }.to_string(), note.pause.clone()),
        ("stop".to_string(), note.stop.clone()),
        ("default".to_string(), tr().text("reminder-open", None)),
    ];
    if let Some(ongoing) = &shown.ongoing {
        ongoing.update(&note.title, &note.text, buttons);
    } else if let Some((qt, shared)) = window {
        let (qt, shared) = (qt.clone(), Arc::clone(shared));
        shown.ongoing = Some(sioul_sync::notify::ongoing(&note.title, &note.text, buttons, Box::new(move |key: &str| pressed(&qt, &shared, key))));
    }
}

#[cfg(target_os = "android")]
unsafe extern "C" {
    /// The note in Android's notifications, put up or changed; "" takes it
    /// away (android/main.cpp). Any thread.
    fn sioul_android_time_note(json: *const c_char);
}

/// The note put up, changed, or taken away: Android's ongoing notification.
#[cfg(target_os = "android")]
fn put(_shown: &mut Shown, note: Option<&Note>, _window: Option<(&QtThread, &Arc<Shared>)>) {
    let Ok(text) = std::ffi::CString::new(note.map_or_else(String::new, |n| crate::backend::json(n))) else { return };
    // SAFETY: a zero-terminated text, valid for the call.
    unsafe { sioul_android_time_note(text.as_ptr()) };
}

/// A button of the desktop's notification, on its thread: as the focus
/// window's own; a click on it brings Sioul forward, on the task.
#[cfg(not(target_os = "android"))]
fn pressed(qt: &QtThread, shared: &Arc<Shared>, key: &str) {
    if key != "default" {
        act(key, Some((qt, shared)));
        post(Some((qt, shared)));
        return;
    }
    let Some(uid) = timelog::running().map(|r| r.task).filter(|t| !t.is_empty()) else { return };
    let uri = sioul_core::links::task_uri(&uid);
    let _ = qt.queue(move |mut sioul| sioul.as_mut().reminder_opened(cxx_qt_lib::QString::from("task"), cxx_qt_lib::QString::from(&uri), cxx_qt_lib::QString::from(&uid)));
}

/// A button pressed, "pause", "resume" or "stop": with the window, as its own
/// buttons (`work::focus_pause`, `work::focus_stop`); without, on the
/// session's file. A button a moment behind (another device paused the
/// session meanwhile) changes nothing: "pause" pauses only what runs,
/// "resume" goes on only with what is paused. Returns whether it changed.
fn act(action: &str, window: Option<(&QtThread, &Arc<Shared>)>) -> bool {
    let _acting = ACTING.lock().unwrap_or_else(PoisonError::into_inner);
    let Some((qt, shared)) = window else {
        return act_in(&timelog::folder(), &Stopped::default_path(), action, Timestamp::now().as_second()).unwrap_or_else(|e| {
            eprintln!("Sioul: the time running: {e}");
            false
        });
    };
    let Some(running) = timelog::running() else { return false };
    match action {
        "pause" | "resume" if running.paused_at.is_some() == (action == "resume") => crate::work::focus_pause(qt, shared),
        "stop" => drop(crate::work::focus_stop(qt, shared, false, "")),
        _ => return false,
    }
    true
}

/// A button pressed without the window: the session's file in `dir` changed
/// as the window's buttons change it; Stop keeps the line left at a pause of
/// this session (`stopped`), as the window's Stop with no word does.
fn act_in(dir: &Path, stopped: &Path, action: &str, now: i64) -> Result<bool, String> {
    let Some(mut running) = timelog::running_in(dir) else { return Ok(false) };
    match action {
        "pause" | "resume" if running.paused_at.is_some() == (action == "resume") => {
            running.toggle_pause(now);
            timelog::keep_running_in(dir, Some(&running)).map(|()| true)
        }
        "stop" => {
            let left = Stopped::load(stopped).filter(|s| s.task == running.task && s.at >= running.start).map(|s| s.text).unwrap_or_default();
            timelog::finish_noted_in(dir, now, false, &left).map(|session| session.is_some())
        }
        _ => Ok(false),
    }
}

/// A button of Android's notification, "pause", "resume" or "stop", pressed,
/// Sioul's window running or not (TimeReceiver.java): done here first, as the
/// window would, so that nothing slow can lose it (Android lets the button go
/// after a few seconds, and may stop Sioul then); then sent at once, with what
/// your other devices did, in an exchange without notes and papers; the
/// notification then put up as it follows, in the order the buttons were
/// pressed. Answers that note as JSON, "" when no session is left. Blocks
/// while the sharing exchanges: never on Android's main thread.
///
/// # Safety
/// `action` is null, or a zero-terminated text valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_time_action(action: *const c_char) -> *mut c_char {
    // SAFETY: as the caller promises.
    let action = unsafe { crate::alarms::key_of(action) };
    let answer = std::panic::catch_unwind(|| {
        let window = WINDOW.lock().ok().and_then(|w| w.clone());
        let window = window.as_ref().map(|(qt, shared)| (qt, shared));
        act(&action, window);
        let _ = crate::share::exchange_here(false);
        post(window).map(|note| crate::backend::json(&note)).unwrap_or_default()
    })
    .unwrap_or_else(|_| {
        eprintln!("Sioul: the time running: \"{action}\" broke");
        String::new()
    });
    crate::alarms::handed(answer)
}

/// Sioul quits: the computer's notification goes with it, its buttons having
/// nothing left to reach. A phone's stays: its buttons reach Sioul without
/// the window.
pub(crate) fn closing() {
    #[cfg(not(target_os = "android"))]
    {
        let ongoing = {
            let mut shown = SHOWN.lock().unwrap_or_else(PoisonError::into_inner);
            shown.over = true;
            shown.ongoing.take()
        };
        if let Some(ongoing) = ongoing {
            ongoing.close(std::time::Duration::from_secs(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> i64 {
        format!("{text}[Europe/Paris]").parse::<jiff::Zoned>().unwrap().timestamp().as_second()
    }

    #[test]
    fn the_note_says_the_task_since_when_and_its_buttons() {
        let (zone, tr) = (TimeZone::get("Europe/Paris").unwrap(), Translator::new("en"));
        let start = at("2026-10-05T14:02");
        let mut running = Running { task: "t1".into(), start, planned: 25, ..Running::default() };
        let note = note_of(Some(&running), "Write to the bank", start + 60, &tr, &zone).unwrap();
        assert_eq!((note.title.as_str(), note.text.as_str(), note.paused, note.since), ("Write to the bank", "Since 14:02, 25 min chosen", false, start * 1000));
        assert_eq!((note.pause.as_str(), note.stop.as_str(), note.channel.as_str()), ("Pause", "Stop", "Focus timer"));
        // Paused after twelve minutes: the time so far, no chronometer, "Go on".
        running.toggle_pause(start + 12 * 60);
        let note = note_of(Some(&running), "Write to the bank", start + 20 * 60, &tr, &zone).unwrap();
        assert_eq!((note.text.as_str(), note.paused, note.since, note.pause.as_str()), ("Paused, 12 min so far", true, 0, "Go on"));
        // Going on after eight minutes paused: the chronometer counts from eight minutes later.
        running.toggle_pause(start + 20 * 60);
        assert_eq!(note_of(Some(&running), "Write to the bank", start + 21 * 60, &tr, &zone).unwrap().since, (start + 8 * 60) * 1000);
        // No title known here, no end chosen: "Focus", without an end; in French too.
        running.planned = 0;
        let note = note_of(Some(&running), " ", start + 21 * 60, &Translator::new("fr"), &zone).unwrap();
        assert_eq!((note.title.as_str(), note.text.as_str(), note.pause.as_str(), note.stop.as_str()), ("Concentration", "Depuis 14:02, sans fin fixée", "Pause", "Arrêter"));
        // No session, no note.
        assert_eq!(note_of(None, "Write to the bank", start, &tr, &zone), None);
    }

    #[test]
    fn titles_from_the_task_else_its_project() {
        let task = |uid: &str, title: &str, cases: &[&str]| Task { uid: uid.into(), title: title.into(), cases: cases.iter().map(|c| c.to_string()).collect(), ..Task::default() };
        let tasks = vec![task("t1", "Write to the bank", &["lumen"]), task("t2", "", &["gone", "lumen"])];
        let cases = vec![Case { id: "lumen".into(), title: "Lumen, the website".into(), ..Case::default() }];
        assert_eq!(title_of("t1", &tasks, &cases), "Write to the bank");
        assert_eq!(title_of("t2", &tasks, &cases), "Lumen, the website");
        assert_eq!(title_of("t3", &tasks, &cases), "", "a task not here yet");
    }

    #[test]
    fn its_buttons_pause_go_on_and_stop_once() {
        let dir = std::env::temp_dir().join(format!("sioul-timenote-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let stopped = dir.join("stopped.toml");
        let (zone, tr) = (TimeZone::get("Europe/Paris").unwrap(), Translator::new("en"));
        let note = |now: i64| note_of(timelog::running_in(&dir).as_ref(), "Write to the bank", now, &tr, &zone);
        let start = at("2026-10-05T09:00");
        // Started: the note, with its task.
        timelog::keep_running_in(&dir, Some(&Running { task: "t1".into(), start, planned: 25, ..Running::default() })).unwrap();
        assert_eq!(note(start).map(|n| (n.title, n.paused)), Some(("Write to the bank".to_string(), false)));
        // Going on with what runs changes nothing; paused, the note says so.
        assert!(!act_in(&dir, &stopped, "resume", start + 60).unwrap());
        assert!(act_in(&dir, &stopped, "pause", start + 600).unwrap());
        assert_eq!(note(start + 700).map(|n| (n.text, n.paused)), Some(("Paused, 10 min so far".to_string(), true)));
        assert!(!act_in(&dir, &stopped, "pause", start + 700).unwrap(), "a button a moment behind changes nothing");
        assert_eq!(timelog::running_in(&dir).unwrap().paused_at, Some(start + 600));
        assert!(act_in(&dir, &stopped, "resume", start + 900).unwrap());
        assert!(note(start + 900).is_some_and(|n| !n.paused && n.since == (start + 300) * 1000));
        // Stopped: kept, with the line left at its pause; no note left.
        Stopped::keep(&stopped, "the second paragraph", "t1", start + 610).unwrap();
        assert!(act_in(&dir, &stopped, "stop", start + 1500).unwrap());
        assert_eq!(note(start + 1500), None);
        let kept = timelog::sessions_in(&dir);
        assert_eq!((kept.len(), kept[0].minutes, kept[0].note.as_str(), kept[0].done), (1, 20, "the second paragraph", false));
        assert!(!act_in(&dir, &stopped, "stop", start + 1600).unwrap(), "nothing left to stop");
        assert!(!act_in(&dir, &stopped, "jump", start + 1600).unwrap());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
