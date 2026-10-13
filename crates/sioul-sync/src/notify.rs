// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The one exception to the admin windows: a quiet desktop notification for a
//! verified code, with the code in it (docs/porch.md, "Right now"); a gentle
//! reminder; new mail at the times it may come, once per batch
//! (docs/porch.md, "Notifications"); and the time running, while a focus
//! session runs (docs/tasks.md).
//!
//! On Linux and the BSDs each notification goes to the session's
//! notification server over a D-Bus connection of its own, made by
//! `crate::bus` within `bus::SETUP`: a session bus that takes it and never
//! answers holds one thread of its own, never the caller nor libdbus's
//! process-wide locks (review of battery part C, G1). Its call is the one
//! notify-rust's `Notification` made before 13 October 2026, argument for
//! argument (`send`), but for its urgency, said once (notify-rust's hints are
//! a set, where a mail's normal urgency and the critical one it started from
//! were two, sent in no fixed order). Windows and macOS: notify-rust.
//!
//! A dose's reminder (`remind_dose`) says what became of its first try, so
//! that one not shown is recorded `unshown`, never as a reminder (the phone's
//! alarm and the question on the past then take it up: docs/health.md); and
//! while the bus or the server could not take it, it is tried again until
//! the dose's grace ends, so that a bus that stalls shows it late.

#[cfg(any(windows, target_os = "macos"))]
use notify_rust::{Notification, Timeout};
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

/// How long the notification stays: about the life of a code.
#[cfg(not(target_os = "android"))]
const SHOWN: u32 = 10 * 60 * 1000;

/// How long a notice's first try may take, its connection and all: the
/// connection's setup (`bus::SETUP`, five seconds), the server's two answers
/// (`ASKED`, two seconds each), and a second more. A dose's reminder is
/// waited for this long at most (`remind_dose`, `Sending::outcome`).
pub const FIRST_TRY: Duration = Duration::from_secs(10);

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const _: () = assert!(FIRST_TRY.as_secs() > crate::bus::SETUP.as_secs() + 2 * ASKED.as_secs());

/// What a dose's reminder does when its first try did not go out
/// (`remind_dose`): tried again on its own thread until `until` (the end of
/// the dose's grace), five seconds apart, then half a minute (`again_in`),
/// while the bus or the notification server could not take it; never once
/// the server was asked and did not answer, which may show it late (a dose
/// reminded twice could be taken twice). Before each try, `still` gives the
/// words to show then, or none: no longer to remind (marked, or reminded
/// since by another device: a phone's alarm). Once shown that late, `shown`.
/// Linux and the BSDs; elsewhere the first try is the only one.
pub struct Again {
    pub until: std::time::Instant,
    pub still: Box<dyn FnMut() -> Option<(String, String)> + Send>,
    pub shown: Box<dyn FnOnce() + Send>,
}

/// A dose's reminder under way (`remind_dose`): what became of its first try.
pub struct Sending(std::sync::mpsc::Receiver<Result<(), String>>);

impl Sending {
    /// A first try whose outcome is known at once.
    #[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
    fn known(outcome: Result<(), String>) -> Sending {
        let (said, sending) = std::sync::mpsc::channel();
        let _ = said.send(outcome);
        Sending(sending)
    }

    /// Whether the first try went out (the notification server took the
    /// notice), waited for `wait` at most: not said by then, it did not; its
    /// thread may still show it later (`Again::shown`).
    pub fn outcome(&self, wait: Duration) -> Result<(), String> {
        self.0.recv_timeout(wait).unwrap_or_else(|_| Err("the notification was not taken in time".to_string()))
    }
}

/// Sioul's bundle id on macOS, as `packaging/macos/Info.plist` names it
/// (`CFBundleIdentifier`). macOS shows a notification as coming from the
/// application whose id it is given; without one, notify-rust's macOS side
/// (mac-notification-sys) names another application.
pub const BUNDLE_ID: &str = "com.aurelienpierre.Sioul";

/// macOS: Sioul's notifications said to come from Sioul (`BUNDLE_ID`), once
/// per process, before the first is shown: macOS takes the application only
/// once.
#[cfg(target_os = "macos")]
fn as_sioul() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = notify_rust::set_application(BUNDLE_ID);
    });
}

/// Shows a code: `title` says what and from whom, `body` the code and how long
/// it lasts. No sound. With `copy`, the notification offers a button labelled
/// with its text, and its action runs when the button is pressed; the
/// notification then lives on its own thread, which waits for the button.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn code(title: &str, body: &str, copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    one_button(Notice::new(title, body, "dialog-password", CRITICAL), copy)
}

/// A gentle reminder (a dose to take): one notification, no sound, with one
/// button whose action runs when pressed.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn remind(title: &str, body: &str, action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    one_button(Notice::new(title, body, "appointment-soon", CRITICAL), action)
}

/// A dose's reminder (docs/health.md, "Knowing"): what `remind` shows with
/// its button ("Taken"), on a thread of its own; what became of its first
/// try is said back (`Sending::outcome`, `FIRST_TRY` at most), so that one
/// not shown is never recorded as a reminder; and while the bus or the
/// server could not take it, it is tried again (`Again`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn remind_dose(title: &str, body: &str, button: (String, Box<dyn FnOnce() + Send>), again: Again) -> Sending {
    let mut notice = Notice::new(title, body, "appointment-soon", CRITICAL);
    let (label, run) = button;
    notice.buttons.push(("copy".to_string(), label));
    let pressed: Box<dyn FnOnce(&str) + Send> = Box::new(move |key: &str| {
        if key == "copy" {
            run();
        }
    });
    let (said, sending) = std::sync::mpsc::channel();
    // The D-Bus connection cannot change threads, so it is made where it waits.
    std::thread::spawn(move || {
        let failure = match show(&notice, true) {
            Ok(shown) => {
                let _ = said.send(Ok(()));
                return wait_for_press(&shown, pressed);
            }
            Err(failure) => failure,
        };
        let _ = said.send(Err(failure.said));
        if failure.again {
            show_again(notice, again, pressed);
        }
    });
    Sending(sending)
}

/// Elsewhere, the reminder as `remind` shows it, its outcome known at once;
/// no try after the first.
#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub fn remind_dose(title: &str, body: &str, button: (String, Box<dyn FnOnce() + Send>), _again: Again) -> Sending {
    Sending::known(remind(title, body, Some(button)))
}

/// A dose's reminder tried again after its first try (`Again`): the bus
/// first (a stalled one refuses at once, `bus`, and nothing else is read
/// meanwhile), then its words as they are now, then the server.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn show_again(mut notice: Notice, mut again: Again, pressed: Box<dyn FnOnce(&str) + Send>) {
    let mut tries = 0;
    loop {
        let wait = again_in(tries);
        tries += 1;
        if std::time::Instant::now() + wait >= again.until {
            return;
        }
        std::thread::sleep(wait);
        let Ok(bus) = session() else { continue };
        let Some((title, body)) = (again.still)() else { return };
        (notice.title, notice.body) = (title, body);
        match show_on(bus, &notice, true) {
            Ok(shown) => {
                (again.shown)();
                return wait_for_press(&shown, pressed);
            }
            Err(failure) if failure.again => {}
            Err(_) => return,
        }
    }
}

/// How long a dose's reminder waits before its try after `tries` others:
/// five seconds for two minutes (a bus that stalled a while: shown soon after
/// it answers), then half a minute.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn again_in(tries: u32) -> Duration {
    if tries < 24 { Duration::from_secs(5) } else { Duration::from_secs(30) }
}

/// New mail at the times it may come (docs/porch.md, "Notifications"): one
/// notification for the batch, no sound, with one button ("Open": the
/// Porch) whose action runs when pressed. At normal urgency, unlike codes
/// and reminders: the desktop's own do-not-disturb holds it; but at critical
/// urgency when someone Always through is in it (`through`), which passes it
/// (docs/attention.md, Q10).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn mail(title: &str, body: &str, through: bool, action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    one_button(Notice::new(title, body, "mail-unread", if through { CRITICAL } else { NORMAL }), action)
}

/// Windows and macOS: the mail's words alone; their notifications carry no button here.
#[cfg(any(windows, target_os = "macos"))]
pub fn mail(title: &str, body: &str, _through: bool, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    as_sioul();
    Notification::new().appname("Sioul").summary(title).body(body).timeout(Timeout::Milliseconds(SHOWN)).show().map(drop).map_err(|e| e.to_string())
}

/// Android: made on the window's side, through Java (sioul-app's `eventalarms`).
#[cfg(target_os = "android")]
pub fn mail(_title: &str, _body: &str, _through: bool, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Err("notifications are made through Java on Android".to_string())
}

/// A gentle reminder with several buttons, each its label and its action:
/// the one pressed runs (the night's notice: "Close the day" and "Options…").
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn remind_choices(title: &str, body: &str, choices: Vec<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    if choices.is_empty() {
        return remind(title, body, None);
    }
    let mut notice = Notice::new(title, body, "appointment-soon", CRITICAL);
    let mut runs: Vec<Option<Box<dyn FnOnce() + Send>>> = Vec::new();
    for (index, (label, run)) in choices.into_iter().enumerate() {
        notice.buttons.push((format!("choice-{index}"), label));
        runs.push(Some(run));
    }
    notify(
        notice,
        Some(Box::new(move |key: &str| {
            if let Some(run) = key.strip_prefix("choice-").and_then(|n| n.parse::<usize>().ok()).and_then(|n| runs.get_mut(n)).and_then(Option::take) {
                run();
            }
        })),
    )
}

/// Elsewhere, as a reminder without buttons (`remind`).
#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub fn remind_choices(title: &str, body: &str, _choices: Vec<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    remind(title, body, None)
}

/// Windows and macOS: the reminder is in the text; no button here.
#[cfg(any(windows, target_os = "macos"))]
pub fn remind(title: &str, body: &str, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    as_sioul();
    Notification::new().appname("Sioul").summary(title).body(body).timeout(Timeout::Milliseconds(SHOWN)).show().map(drop).map_err(|e| e.to_string())
}

/// Windows and macOS: the code is in the text; their notifications carry no button here.
#[cfg(any(windows, target_os = "macos"))]
pub fn code(title: &str, body: &str, _copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    as_sioul();
    Notification::new().appname("Sioul").summary(title).body(body).timeout(Timeout::Milliseconds(SHOWN)).show().map(drop).map_err(|e| e.to_string())
}

/// Android: no notifications yet (Android's own are reached through Java);
/// the code shows in the window only.
#[cfg(target_os = "android")]
pub fn code(_title: &str, _body: &str, _copy: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Err("notifications are not made on Android yet".to_string())
}

/// Android: no notifications yet; a reminder is not shown (docs/android.md).
#[cfg(target_os = "android")]
pub fn remind(_title: &str, _body: &str, _action: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    Err("notifications are not made on Android yet".to_string())
}

/// The notification server, on the session bus.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const SERVER: &str = "org.freedesktop.Notifications";
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const SERVER_PATH: &str = "/org/freedesktop/Notifications";

/// Sioul's desktop file, named on each notification ("desktop-entry"): the
/// desktop knows them as Sioul's, and lists Sioul in its notification
/// settings (Plasma's "Show in do not disturb mode", for the doses during a
/// pause: docs/pauses.md).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const DESKTOP_ENTRY: &str = "com.aurelienpierre.Sioul";

/// How long a lasting notification's thread listens to the server before it
/// looks at what Sioul asked: a change shows within it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const LISTENED: Duration = Duration::from_millis(500);

/// A notification the server did not take (one starting with the session,
/// after Sioul) is asked again this often.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const ASKED_AGAIN: Duration = Duration::from_secs(60);

/// How long the server may take to answer about a notification (notify-rust's own wait).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const ASKED: Duration = Duration::from_secs(2);

/// Urgencies, as the specification numbers them: normal, and critical (what
/// Sioul sends always gets through: it filters its own notifications already).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const NORMAL: u8 = 1;
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
const CRITICAL: u8 = 2;

/// A notification as Sioul sends it on Linux and the BSDs (`send`): its
/// words, its icon (a theme's name), its urgency, its buttons as (key, label).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
struct Notice {
    title: String,
    body: String,
    icon: &'static str,
    urgency: u8,
    buttons: Vec<(String, String)>,
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
impl Notice {
    fn new(title: &str, body: &str, icon: &'static str, urgency: u8) -> Notice {
        Notice { title: title.to_string(), body: body.to_string(), icon, urgency, buttons: Vec::new() }
    }
}

/// `notice` with one button at most, keyed "copy" (`code`, `remind`,
/// `mail`): its action runs when it is pressed.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn one_button(mut notice: Notice, button: Option<(String, Box<dyn FnOnce() + Send>)>) -> Result<(), String> {
    let Some((label, run)) = button else { return notify(notice, None) };
    notice.buttons.push(("copy".to_string(), label));
    notify(
        notice,
        Some(Box::new(move |key: &str| {
            if key == "copy" {
                run();
            }
        })),
    )
}

/// Shows `notice`. Without `pressed`, on the caller's thread, the server's
/// refusal said. With it, on a thread of its own, which keeps the connection
/// that showed it (the server tells a press to that connection: Plasma
/// addresses it) and waits there (`wait_for_press`).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn notify(notice: Notice, pressed: Option<Box<dyn FnOnce(&str) + Send>>) -> Result<(), String> {
    let Some(pressed) = pressed else {
        return show(&notice, false).map(drop).map_err(|failure| failure.said);
    };
    // The D-Bus connection cannot change threads, so it is made where it waits.
    std::thread::spawn(move || {
        if let Ok(shown) = show(&notice, true) {
            wait_for_press(&shown, pressed);
        }
    });
    Ok(())
}

/// A notice shown: the connection that showed it (kept: the server tells a
/// press to it), what it heard (when `listen`), the server's number for it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
struct Shown {
    bus: dbus::blocking::LocalConnection,
    heard: std::rc::Rc<std::cell::RefCell<Vec<Heard>>>,
    id: u32,
}

/// Why a notice did not go out, said in a few words, and whether a later try
/// may show it (`again`): no connection, no notification server (Plasma's
/// starting again); never once the server was asked and did not answer in
/// time (it may show it late) or refused it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
struct Failure {
    said: String,
    again: bool,
}

/// `notice` shown on a connection of its own, made now; heard on it when `listen`.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn show(notice: &Notice, listen: bool) -> Result<Shown, Failure> {
    let bus = session().map_err(|e| Failure { said: said(&e), again: true })?;
    show_on(bus, notice, listen)
}

/// `notice` shown on `bus`, heard there first when `listen` (a press is not
/// missed while the call is under way).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn show_on(bus: dbus::blocking::LocalConnection, notice: &Notice, listen: bool) -> Result<Shown, Failure> {
    let heard = std::rc::Rc::default();
    if listen {
        listened(&bus, &heard).map_err(|e| Failure { said: said(&e), again: true })?;
    }
    let id = send(&bus, notice).map_err(|e| Failure { said: said(&e), again: nobody_took(&e) })?;
    Ok(Shown { bus, heard, id })
}

/// Whether a call refused never reached a notification server: none on the
/// bus (it may start again: plasmashell), or the connection gone before it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn nobody_took(e: &dbus::Error) -> bool {
    matches!(e.name(), Some("org.freedesktop.DBus.Error.ServiceUnknown" | "org.freedesktop.DBus.Error.NameHasNoOwner" | "org.freedesktop.DBus.Error.Disconnected"))
}

/// A shown notice's button waited for on its connection: `pressed` runs with
/// the key of the first one pressed, and the wait ends then, or when the
/// notification closes, or the connection goes.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn wait_for_press(shown: &Shown, pressed: Box<dyn FnOnce(&str) + Send>) {
    loop {
        for item in std::mem::take(&mut *shown.heard.borrow_mut()) {
            match item {
                Heard::Pressed(at, key) if at == shown.id => {
                    pressed(&key);
                    return;
                }
                Heard::Closed(at) if at == shown.id => return,
                _ => {}
            }
        }
        if shown.bus.process(Duration::from_secs(1)).is_err() {
            return;
        }
    }
}

/// The call notify-rust's `Notification` made, argument for argument:
/// "Sioul", no notification replaced, the icon, the title as it is, the body
/// as the server reads it (`body_text`, after asking what it reads), the
/// buttons, the hints (no sound, Sioul's desktop file, the urgency, each
/// once), ten minutes (`SHOWN`). The server's number for it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn send(bus: &dbus::blocking::LocalConnection, notice: &Notice) -> Result<u32, dbus::Error> {
    use dbus::arg::{PropMap, RefArg, Variant};
    let server = bus.with_proxy(SERVER, SERVER_PATH, ASKED);
    let capabilities: Vec<String> = server.method_call(SERVER, "GetCapabilities", ()).map(|(found,): (Vec<String>,)| found).unwrap_or_default();
    let mut hints = PropMap::new();
    hints.insert("suppress-sound".into(), Variant(Box::new(true) as Box<dyn RefArg>));
    hints.insert("desktop-entry".into(), Variant(Box::new(DESKTOP_ENTRY.to_string()) as Box<dyn RefArg>));
    hints.insert("urgency".into(), Variant(Box::new(notice.urgency) as Box<dyn RefArg>));
    let buttons: Vec<&str> = notice.buttons.iter().flat_map(|(key, label)| [key.as_str(), label.as_str()]).collect();
    let timeout = i32::try_from(SHOWN).unwrap_or(i32::MAX);
    let (id,): (u32,) = server.method_call(SERVER, "Notify", ("Sioul", 0u32, notice.icon, notice.title.as_str(), body_text(&notice.body, &capabilities), buttons, hints, timeout))?;
    Ok(id)
}

/// What the server says of notifications, heard on `bus` from now on:
/// buttons pressed, notifications closed (`Heard`), kept in `heard`.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn listened(bus: &dbus::blocking::LocalConnection, heard: &std::rc::Rc<std::cell::RefCell<Vec<Heard>>>) -> Result<(), dbus::Error> {
    use dbus::blocking::LocalConnection;
    use dbus::message::MatchRule;
    let (pressed, closed) = (std::rc::Rc::clone(heard), std::rc::Rc::clone(heard));
    bus.add_match(MatchRule::new_signal(SERVER, "ActionInvoked").with_path(SERVER_PATH), move |(id, key): (u32, String), _: &LocalConnection, _: &dbus::Message| {
        pressed.borrow_mut().push(Heard::Pressed(id, key));
        true
    })?;
    bus.add_match(MatchRule::new_signal(SERVER, "NotificationClosed").with_path(SERVER_PATH), move |(id, _why): (u32, u32), _: &LocalConnection, _: &dbus::Message| {
        closed.borrow_mut().push(Heard::Closed(id));
        true
    })?;
    Ok(())
}

/// A connection to the session's bus, made by `crate::bus` (`bus::SETUP` at most).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn session() -> Result<dbus::blocking::LocalConnection, dbus::Error> {
    crate::bus::session().map(dbus::blocking::LocalConnection::from)
}

/// A D-Bus error in a few words: its message, else its name.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn said(e: &dbus::Error) -> String {
    e.message().or(e.name()).unwrap_or("D-Bus").to_string()
}

/// The notification of something going on, the time running while a focus
/// session runs (docs/tasks.md): shown until it is closed, changed in place,
/// its buttons pressed any number of times. Its own thread holds the D-Bus
/// connection that shows it, and listens there: the server tells a press to
/// that connection alone (Plasma addresses it). Resident, kept when a button
/// is pressed; at normal urgency, for Plasma shows a low one nowhere unless
/// told to; without sound. A server that keeps what it shows ("persistence":
/// its history, its tray) takes it there after its popup, buttons alive;
/// elsewhere it does not expire. No session bus, no server: nothing shows,
/// nothing breaks. Dropped, it is taken away.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub struct Ongoing {
    changes: Sender<Change>,
}

/// What a lasting notification says: its buttons as (key, label), "default"
/// for a click on it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
struct Said {
    title: String,
    body: String,
    actions: Vec<(String, String)>,
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
enum Change {
    Show(Said),
    /// Taken away; said done on the sender, if any.
    Close(Option<Sender<()>>),
}

/// What the server said, read between two looks.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
enum Heard {
    Pressed(u32, String),
    Closed(u32),
}

/// Shows the notification of something going on. `actions` are its buttons
/// as (key, label), "default" for a click on it; `on_action` runs with the
/// key of each one pressed, on the notification's own thread.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn ongoing(title: &str, body: &str, actions: Vec<(String, String)>, on_action: Box<dyn Fn(&str) + Send>) -> Ongoing {
    let (changes, asked) = std::sync::mpsc::channel();
    let said = Said { title: title.to_string(), body: body.to_string(), actions };
    // The D-Bus connection cannot change threads, so it is made where it listens.
    std::thread::spawn(move || serve(said, &asked, &*on_action));
    Ongoing { changes }
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
impl Ongoing {
    /// Changes it in place; shown again if it was closed meanwhile.
    pub fn update(&self, title: &str, body: &str, actions: Vec<(String, String)>) {
        let _ = self.changes.send(Change::Show(Said { title: title.to_string(), body: body.to_string(), actions }));
    }

    /// Takes it away, waiting at most `wait` for the server to have done it:
    /// as Sioul quits, before the connection goes with the process.
    pub fn close(self, wait: Duration) {
        let (done, closed) = std::sync::mpsc::channel();
        if self.changes.send(Change::Close(Some(done))).is_ok() {
            let _ = closed.recv_timeout(wait);
        }
    }
}

/// A lasting notification's thread: what Sioul asks shown, what the server
/// says heard, until it is taken away. Its connection is made again while
/// the bus does not give one (a stall: `bus` refuses at once meanwhile, five
/// seconds apart, `bus::SETUP`) and once it is lost, rather than the
/// notification being lost for the session (review of battery part C, L4);
/// what Sioul asks meanwhile is kept, the last of it, and a close ends it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn serve(mut said: Said, asked: &Receiver<Change>, on_action: &dyn Fn(&str)) {
    use std::sync::mpsc::RecvTimeoutError;
    loop {
        if let Ok(connection) = session()
            && let Served::Done = served(&mut said, asked, on_action, &connection)
        {
            return;
        }
        match asked.recv_timeout(crate::bus::SETUP) {
            Ok(Change::Show(next)) => said = next,
            Ok(Change::Close(done)) => {
                if let Some(done) = done {
                    let _ = done.send(());
                }
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// What became of a lasting notification on one connection (`served`):
/// taken away, or its connection lost (made again, the notification shown
/// again).
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
enum Served {
    Done,
    Lost,
}

/// A lasting notification on `connection`: what Sioul asks shown, what the
/// server says heard, until it is taken away or the connection is lost.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn served(said: &mut Said, asked: &Receiver<Change>, on_action: &dyn Fn(&str), connection: &dbus::blocking::LocalConnection) -> Served {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc::TryRecvError;
    let heard: Rc<RefCell<Vec<Heard>>> = Rc::default();
    if listened(connection, &heard).is_err() {
        return Served::Lost;
    }
    let server = connection.with_proxy(SERVER, SERVER_PATH, Duration::from_secs(2));
    // Asked again while not known: a server starting after Sioul answers later.
    let mut capabilities: Vec<String> = Vec::new();
    // The server's number for it, 0 while none shows; what is to show; when the server last refused it.
    let (mut id, mut fresh, mut refused) = (0u32, true, None::<std::time::Instant>);
    loop {
        // What the server said meanwhile, all of it: buttons pressed, notifications closed.
        while connection.process(Duration::ZERO).unwrap_or(false) {}
        let news = std::mem::take(&mut *heard.borrow_mut());
        for item in news {
            match item {
                Heard::Pressed(at, key) if at == id && id != 0 => on_action(&key),
                // Closed by you, or by the server: shown again at the next change.
                Heard::Closed(at) if at == id => id = 0,
                _ => {}
            }
        }
        // What Sioul asked meanwhile: the last of it counts.
        loop {
            match asked.try_recv() {
                Ok(Change::Show(next)) => (*said, fresh) = (next, true),
                Ok(Change::Close(done)) => {
                    close_lasting(&server, id);
                    if let Some(done) = done {
                        let _ = done.send(());
                    }
                    return Served::Done;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    close_lasting(&server, id);
                    return Served::Done;
                }
            }
        }
        if fresh || refused.is_some_and(|at| at.elapsed() >= ASKED_AGAIN) {
            if capabilities.is_empty() {
                capabilities = server.method_call(SERVER, "GetCapabilities", ()).map(|(found,): (Vec<String>,)| found).unwrap_or_default();
            }
            match show_lasting(&server, id, said, &capabilities) {
                Ok(shown) => (id, refused) = (shown, None),
                Err(_) => refused = Some(std::time::Instant::now()),
            }
            fresh = false;
        }
        if connection.process(LISTENED).is_err() {
            return Served::Lost;
        }
    }
}

/// Shows it, in place of `id` if not 0: the server's number for it.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn show_lasting(server: &dbus::blocking::Proxy<'_, &dbus::blocking::LocalConnection>, id: u32, said: &Said, capabilities: &[String]) -> Result<u32, dbus::Error> {
    use dbus::arg::{PropMap, RefArg, Variant};
    let mut hints = PropMap::new();
    hints.insert("resident".into(), Variant(Box::new(true) as Box<dyn RefArg>));
    hints.insert("suppress-sound".into(), Variant(Box::new(true) as Box<dyn RefArg>));
    hints.insert("urgency".into(), Variant(Box::new(1u8) as Box<dyn RefArg>));
    hints.insert("desktop-entry".into(), Variant(Box::new(DESKTOP_ENTRY.to_string()) as Box<dyn RefArg>));
    let actions: Vec<&str> = said.actions.iter().flat_map(|(key, label)| [key.as_str(), label.as_str()]).collect();
    // Kept by the server once its popup goes, or never expiring where nothing keeps it.
    let timeout: i32 = if capabilities.iter().any(|c| c == "persistence") { -1 } else { 0 };
    let (shown,): (u32,) = server.method_call(SERVER, "Notify", ("Sioul", id, "chronometer", said.title.as_str(), body_text(&said.body, capabilities), actions, hints, timeout))?;
    Ok(shown)
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn close_lasting(server: &dbus::blocking::Proxy<'_, &dbus::blocking::LocalConnection>, id: u32) {
    if id != 0 {
        let _: Result<(), _> = server.method_call(SERVER, "CloseNotification", (id,));
    }
}

/// Windows and macOS: no lasting notification yet, the focus window shows
/// the time. Android: Android's own, made on the window's side (sioul-app).
#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub struct Ongoing;

#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
pub fn ongoing(_title: &str, _body: &str, _actions: Vec<(String, String)>, _on_action: Box<dyn Fn(&str) + Send>) -> Ongoing {
    Ongoing
}

#[cfg(not(all(unix, not(any(target_os = "macos", target_os = "android")))))]
impl Ongoing {
    pub fn update(&self, _title: &str, _body: &str, _actions: Vec<(String, String)>) {}

    pub fn close(self, _wait: Duration) {}
}

/// A body as the notification server reads it. Most read markup there
/// ("body-markup": `<b>`, `<a href>`, `<img src>`), and bodies carry a mail's
/// or a site's words: there, `&`, `<` and `>` are written as entities, so
/// that the words show as they came and nothing in them is a link or a
/// picture to fetch. A server that reads no markup shows the text as it is.
/// The summary is plain text for every server (the specification), and goes as it is.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn body_text(body: &str, capabilities: &[String]) -> String {
    // A server that could not be asked is taken to read markup: entities at worst.
    if capabilities.is_empty() || capabilities.iter().any(|c| c == "body-markup") {
        body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    } else {
        body.to_string()
    }
}

/// The bundle id macOS is given is the one Sioul's macOS bundle declares:
/// any other would attribute its notifications to another application.
#[cfg(test)]
mod bundle {
    #[test]
    fn macos_is_given_the_bundles_own_id() {
        let plist = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/macos/Info.plist")).unwrap();
        let declared = plist.split("<key>CFBundleIdentifier</key>").nth(1).and_then(|rest| rest.split("<string>").nth(1)).and_then(|rest| rest.split("</string>").next()).unwrap();
        assert_eq!(super::BUNDLE_ID, declared.trim());
    }
}

#[cfg(all(test, unix, not(any(target_os = "macos", target_os = "android"))))]
mod tests {
    use super::*;

    #[test]
    fn bodies_are_words_not_markup() {
        let markup = vec!["actions".to_string(), "body-markup".to_string()];
        let body = "Code 482913 <img src=\"https://example.org/t.png\"> & <a href=\"https://example.org\">here</a>";
        let shown = body_text(body, &markup);
        assert!(!shown.contains('<') && !shown.contains('>'), "{shown}");
        assert!(shown.starts_with("Code 482913 &lt;img") && shown.contains(" &amp; "), "{shown}");
        // Not asked, read as markup; a server without markup gets the words as they are.
        assert_eq!(body_text("a < b", &[]), "a &lt; b");
        assert_eq!(body_text("a < b & c", &["actions".to_string()]), "a < b & c");
    }

    /// A connection to the test's own session bus (dbus-run-session) owning
    /// the notification server's name, a stand-in of a test before given two
    /// seconds to leave it; none where a server is there already (the
    /// desktop's): never in a server's place, no replacing it, no waiting in line.
    fn stand_in_bus() -> Option<dbus::blocking::LocalConnection> {
        use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
        let until = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let bus = session().ok().filter(|bus| matches!(bus.request_name(SERVER, false, false, true), Ok(RequestNameReply::PrimaryOwner)));
            if bus.is_some() || std::time::Instant::now() >= until {
                return bus;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The hints' names a call carries, each as many times as it is there
    /// (a dictionary read as such would keep one of each).
    fn hint_names(call: &dbus::Message) -> Vec<String> {
        use dbus::arg::ArgType;
        let mut args = call.iter_init();
        for _ in 0..6 {
            args.next();
        }
        let mut names = Vec::new();
        if let Some(mut entries) = args.recurse(ArgType::Array) {
            while entries.arg_type() == ArgType::DictEntry {
                if let Some(mut entry) = entries.recurse(ArgType::DictEntry) {
                    names.extend(entry.get::<String>());
                }
                entries.next();
            }
        }
        names.sort();
        names
    }

    /// A stand-in notification server on the bus at `address`, on a thread of
    /// its own until `stop`: "answers" answers and presses "copy" on each
    /// notice that has it, to the connection that showed it; "silent" takes
    /// each call and never answers. Each Notify's title said on `told`, when
    /// it came. Whether it got the name.
    fn notice_server(address: &str, mode: &'static str, told: std::sync::mpsc::Sender<(std::time::Instant, String)>, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> bool {
        use dbus::blocking::LocalConnection;
        use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
        use dbus::channel::{MatchingReceiver, Sender as _};
        use dbus::message::MatchRule;
        let (ready, started) = std::sync::mpsc::channel::<bool>();
        let address = address.to_string();
        std::thread::spawn(move || {
            let Some(bus) = crate::bus::connect(&address).ok().map(LocalConnection::from).filter(|bus| matches!(bus.request_name(SERVER, false, false, true), Ok(RequestNameReply::PrimaryOwner))) else {
                let _ = ready.send(false);
                return;
            };
            let mut next = 40u32;
            bus.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |call: dbus::Message, bus: &LocalConnection| {
                    let notify = call.member().as_deref() == Some("Notify");
                    if notify && let Ok((_, _, _, title)) = call.read4::<String, u32, String, String>() {
                        let _ = told.send((std::time::Instant::now(), title));
                    }
                    if mode == "silent" {
                        return true;
                    }
                    let reply = if notify {
                        next += 1;
                        call.method_return().append1(next)
                    } else if call.member().as_deref() == Some("GetCapabilities") {
                        call.method_return().append1(vec!["actions", "body"])
                    } else {
                        return true;
                    };
                    let _ = bus.send(reply);
                    let buttons: Vec<String> = if notify { call.read_all::<(String, u32, String, String, String, Vec<String>, dbus::arg::PropMap, i32)>().map(|all| all.5).unwrap_or_default() } else { Vec::new() };
                    if buttons.iter().any(|key| key == "copy") {
                        let mut press = dbus::Message::new_signal(SERVER_PATH, SERVER, "ActionInvoked").unwrap().append2(next, "copy");
                        press.set_destination(call.sender().map(|s| s.into_static()));
                        let _ = bus.send(press);
                    }
                    true
                }),
            );
            let _ = ready.send(true);
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = bus.process(Duration::from_millis(100));
            }
        });
        started.recv_timeout(Duration::from_secs(5)).unwrap_or(false)
    }

    /// Dose reminders and the focus session's lasting notification against a
    /// stand-in session bus (`bus::StandIns`, never the session of whoever
    /// runs this) and a stand-in notification server on it (review of
    /// battery part C, H1 and L4, the review's own scenarios):
    /// - the bus stopped (SIGSTOP) at the dose's minute, let go 8 s later: the
    ///   first try says it did not go out, within `FIRST_TRY`; tried again,
    ///   the reminder shows once the bus answers, said shown late, and its
    ///   button works;
    /// - stopped, reminders at 1, 10 and 20 s, let go at 30 s: each says it
    ///   did not go out, and all three show once the bus answers;
    /// - a server that never answers: not gone out once its two questions
    ///   were given up, and never sent again (it may show it late);
    /// - no server, then one (plasmashell starting again): shown once it is
    ///   there; and none at all once the dose is no longer to remind;
    /// - the lasting notification, its connection stalled at first: shown
    ///   once the bus answers, not lost for the session.
    /// Sets this process's session bus address: run alone, by name:
    /// `cargo test --release -p sioul-sync notify::tests::reminders_say_their_outcome_and_show_late_after_a_stall -- --ignored --exact --nocapture`.
    #[test]
    #[ignore = "starts a stand-in dbus-daemon and sets the session bus address: run alone, by name"]
    fn reminders_say_their_outcome_and_show_late_after_a_stall() {
        use crate::bus::StandIns;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc::channel;
        use std::sync::{Arc, Mutex};
        use std::time::Instant;
        let mut stand_ins = StandIns::new("stand-in-notices");
        let (pid, address) = stand_ins.start("session");
        // SAFETY: run alone, by name; nothing else in this process reads it meanwhile.
        unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", &address) };
        let (told, notices) = channel::<(Instant, String)>();
        let stop = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "answers", told.clone(), Arc::clone(&stop)), "the stand-in server");
        // A dose's reminder: its button's action and its late show said on channels.
        let (pressed, presses) = channel::<String>();
        let (late, lates) = channel::<String>();
        // Whether the doses are still to remind (none once marked).
        let to_remind = Arc::new(Mutex::new(true));
        let dose = |name: &str, until: Duration| {
            let (pressed, late, to_remind) = (pressed.clone(), late.clone(), Arc::clone(&to_remind));
            let (pressed_name, still_name, shown_name) = (name.to_string(), name.to_string(), name.to_string());
            remind_dose(
                name,
                "22:00",
                ("Taken".into(), Box::new(move || drop(pressed.send(pressed_name.clone())))),
                Again { until: Instant::now() + until, still: Box::new(move || to_remind.lock().unwrap().then(|| (still_name.clone(), "22:00".to_string()))), shown: Box::new(move || drop(late.send(shown_name.clone()))) },
            )
        };
        let titles = |within: Duration| -> Vec<String> {
            let until = Instant::now() + within;
            let mut seen = Vec::new();
            while let Ok((_, title)) = notices.recv_timeout(until.saturating_duration_since(Instant::now())) {
                seen.push(title);
            }
            seen
        };

        // Healthy: shown at once, its button pressed.
        let start = Instant::now();
        assert_eq!(dose("Magnesium · 300 mg", Duration::from_secs(60)).outcome(FIRST_TRY), Ok(()));
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(presses.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Magnesium · 300 mg"));
        assert_eq!(titles(Duration::from_millis(300)), ["Magnesium · 300 mg"]);

        // One stall: stopped at 0, the reminder at 1 s, let go at 9 s.
        StandIns::signal(pid, "-STOP");
        std::thread::sleep(Duration::from_secs(1));
        let start = Instant::now();
        let sent = dose("Magnesium · 300 mg", Duration::from_secs(60));
        let outcome = sent.outcome(FIRST_TRY);
        let took = start.elapsed();
        assert!(outcome.is_err() && took <= FIRST_TRY, "not gone out, said within FIRST_TRY: {outcome:?} after {took:?}");
        std::thread::sleep(Duration::from_secs(8).saturating_sub(start.elapsed() + Duration::from_secs(1)));
        StandIns::signal(pid, "-CONT");
        assert_eq!(lates.recv_timeout(Duration::from_secs(20)).as_deref(), Ok("Magnesium · 300 mg"), "shown late once the bus answers");
        assert_eq!(presses.recv_timeout(Duration::from_secs(5)).as_deref(), Ok("Magnesium · 300 mg"), "its button works");
        assert_eq!(titles(Duration::from_millis(500)), ["Magnesium · 300 mg"], "once");

        // Three reminders over 30 s, the bus stopped all along.
        StandIns::signal(pid, "-STOP");
        let start = Instant::now();
        let mut sending = Vec::new();
        for (at, name) in [(1, "First"), (10, "Second"), (20, "Third")] {
            std::thread::sleep(Duration::from_secs(at).saturating_sub(start.elapsed()));
            let sent = dose(name, Duration::from_secs(90));
            assert!(sent.outcome(FIRST_TRY).is_err(), "{name}: not gone out");
            sending.push(sent);
        }
        std::thread::sleep(Duration::from_secs(30).saturating_sub(start.elapsed()));
        StandIns::signal(pid, "-CONT");
        let mut shown: Vec<String> = (0..3).filter_map(|_| lates.recv_timeout(Duration::from_secs(25)).ok()).collect();
        shown.sort();
        assert_eq!(shown, ["First", "Second", "Third"], "all three shown once the bus answers");
        let mut seen = titles(Duration::from_secs(2));
        seen.sort();
        assert_eq!(seen, ["First", "Second", "Third"]);
        for _ in 0..3 {
            let _ = presses.recv_timeout(Duration::from_secs(5));
        }

        // A server that never answers: not gone out, never sent again.
        stop.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));
        let quiet = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "silent", told.clone(), Arc::clone(&quiet)), "the silent server");
        let start = Instant::now();
        let outcome = dose("Magnesium · 300 mg", Duration::from_secs(60)).outcome(FIRST_TRY);
        assert!(outcome.is_err() && start.elapsed() < FIRST_TRY, "{outcome:?} after {:?}", start.elapsed());
        assert_eq!(titles(Duration::from_secs(15)), ["Magnesium · 300 mg"], "asked once, never again");
        assert!(lates.try_recv().is_err());

        // No server, then one: shown once it is there; none once no longer to remind.
        quiet.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));
        let outcome = dose("Magnesium · 300 mg", Duration::from_secs(60)).outcome(FIRST_TRY);
        assert!(outcome.is_err(), "no server: {outcome:?}");
        std::thread::sleep(Duration::from_secs(6));
        let back = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "answers", told.clone(), Arc::clone(&back)));
        assert_eq!(lates.recv_timeout(Duration::from_secs(15)).as_deref(), Ok("Magnesium · 300 mg"), "shown once the server is there");
        let _ = presses.recv_timeout(Duration::from_secs(5));
        let _ = titles(Duration::from_millis(500));
        back.store(true, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(500));
        let outcome = dose("Magnesium · 300 mg", Duration::from_secs(60)).outcome(FIRST_TRY);
        assert!(outcome.is_err());
        *to_remind.lock().unwrap() = false;
        let again = Arc::new(AtomicBool::new(false));
        assert!(notice_server(&address, "answers", told.clone(), Arc::clone(&again)));
        assert!(titles(Duration::from_secs(12)).is_empty(), "marked meanwhile: never shown");
        assert!(lates.try_recv().is_err());

        // The lasting notification, its connection stalled at first: shown once the bus answers.
        StandIns::signal(pid, "-STOP");
        let lasting = ongoing("Write to the bank", "Since 14:02, for 25 min", vec![("stop".to_string(), "Stop".to_string())], Box::new(|_: &str| {}));
        std::thread::sleep(Duration::from_secs(8));
        StandIns::signal(pid, "-CONT");
        assert_eq!(titles(Duration::from_secs(20)).first().map(String::as_str), Some("Write to the bank"), "not lost for the session");
        lasting.close(Duration::from_secs(3));
        again.store(true, Ordering::Relaxed);
    }

    /// Codes, reminders and mail, against a notification server of the test's
    /// own on a bus of its own (run under dbus-run-session; skipped where a
    /// server is there already: nothing here may reach the desktop's). Each
    /// goes as notify-rust's `Notification` sent it (`send`): "Sioul", the
    /// icon, the title, the body as a server reading markup takes it, the
    /// buttons, no sound, Sioul's desktop file, ten minutes; its urgency said
    /// once (a mail at normal urgency, unless someone Always through is in
    /// it). A button pressed at the server runs its action, here, once.
    #[test]
    fn notifications_go_as_before_and_hear_their_button() {
        use dbus::blocking::LocalConnection;
        use dbus::channel::{MatchingReceiver, Sender as _};
        use dbus::message::MatchRule;
        use std::sync::mpsc::channel;

        /// What the stand-in server was sent.
        #[derive(Debug, PartialEq)]
        struct Sent {
            app: String,
            replaces: u32,
            icon: String,
            title: String,
            body: String,
            buttons: Vec<String>,
            hints: Vec<String>,
            sound_off: bool,
            entry: String,
            urgency: u8,
            timeout: i32,
        }
        let _alone = crate::bus::NOTIFICATION_SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let (told, sent) = channel::<Sent>();
        let (ready, started) = channel::<bool>();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopped = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            let Some(bus) = stand_in_bus() else {
                let _ = ready.send(false);
                return;
            };
            let mut next = 20u32;
            bus.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |call: dbus::Message, bus: &LocalConnection| {
                    let mut press = None;
                    let reply = match call.member().as_deref() {
                        Some("GetCapabilities") => call.method_return().append1(vec!["actions", "body", "body-markup"]),
                        Some("Notify") => {
                            let (app, replaces, icon, title, body, buttons, hints, timeout): (String, u32, String, String, String, Vec<String>, dbus::arg::PropMap, i32) = call.read_all().unwrap();
                            next += 1;
                            // The last button, pressed once shown.
                            if buttons.len() >= 2 {
                                press = Some(buttons[buttons.len() - 2].clone());
                            }
                            let _ = told.send(Sent {
                                app,
                                replaces,
                                icon,
                                title,
                                body,
                                buttons,
                                hints: hint_names(&call),
                                sound_off: dbus::arg::prop_cast::<bool>(&hints, "suppress-sound").copied().unwrap_or(false),
                                entry: dbus::arg::prop_cast::<String>(&hints, "desktop-entry").cloned().unwrap_or_default(),
                                urgency: dbus::arg::prop_cast::<u8>(&hints, "urgency").copied().unwrap_or(9),
                                timeout,
                            });
                            call.method_return().append1(next)
                        }
                        _ => return true,
                    };
                    let _ = bus.send(reply);
                    if let Some(key) = press {
                        let mut signal = dbus::Message::new_signal(SERVER_PATH, SERVER, "ActionInvoked").unwrap().append2(next, key);
                        // To the connection that showed it alone, as Plasma does.
                        signal.set_destination(call.sender().map(|s| s.into_static()));
                        let _ = bus.send(signal);
                    }
                    true
                }),
            );
            let _ = ready.send(true);
            let until = std::time::Instant::now() + Duration::from_secs(30);
            while !stopped.load(std::sync::atomic::Ordering::Relaxed) && std::time::Instant::now() < until {
                let _ = bus.process(Duration::from_millis(100));
            }
        });
        if !started.recv_timeout(Duration::from_secs(5)).unwrap_or(false) {
            eprintln!("No bus of the test's own (dbus-run-session), or a notification server on it already: skipped.");
            return;
        }
        let wait = Duration::from_secs(5);
        let as_before = |icon: &str, title: &str, body: &str, buttons: &[&str], urgency: u8| Sent {
            app: "Sioul".into(),
            replaces: 0,
            icon: icon.into(),
            title: title.into(),
            body: body.into(),
            buttons: buttons.iter().map(|b| b.to_string()).collect(),
            hints: vec!["desktop-entry".into(), "suppress-sound".into(), "urgency".into()],
            sound_off: true,
            entry: DESKTOP_ENTRY.into(),
            urgency,
            timeout: 600_000,
        };
        // A code with its button: pressed at the server, copied here.
        let (copied, copies) = channel::<()>();
        code("Code from example.org", "482913 <b>now</b> & then", Some(("Copy".into(), Box::new(move || {
            let _ = copied.send(());
        })))).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("dialog-password", "Code from example.org", "482913 &lt;b&gt;now&lt;/b&gt; &amp; then", &["copy", "Copy"], CRITICAL));
        assert_eq!(copies.recv_timeout(wait), Ok(()));
        // Mail: normal urgency, said once; critical when someone Always through is in it.
        mail("New mail", "From Noa", false, None).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("mail-unread", "New mail", "From Noa", &[], NORMAL));
        mail("New mail", "From Noa", true, None).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("mail-unread", "New mail", "From Noa", &[], CRITICAL));
        // A reminder; and one with choices, the one pressed run alone.
        remind("Magnesium", "300 mg", None).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("appointment-soon", "Magnesium", "300 mg", &[], CRITICAL));
        let (chose, choices) = channel::<&str>();
        let other = chose.clone();
        remind_choices("The night", "Close the day?", vec![("Close the day".into(), Box::new(move || {
                let _ = chose.send("close");
            })), ("Options…".into(), Box::new(move || {
                let _ = other.send("options");
            }))]).unwrap();
        assert_eq!(sent.recv_timeout(wait).unwrap(), as_before("appointment-soon", "The night", "Close the day?", &["choice-0", "Close the day", "choice-1", "Options…"], CRITICAL));
        assert_eq!(choices.recv_timeout(wait), Ok("options"));
        assert!(choices.recv_timeout(Duration::from_millis(300)).is_err(), "one choice run");
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// The time running, against a notification server of the test's own on a
    /// bus of its own: run under dbus-run-session. Where a server is there
    /// already (the desktop's), skipped: nothing here may reach it.
    #[test]
    fn a_lasting_notification_changes_in_place_hears_its_buttons_and_goes() {
        use dbus::blocking::LocalConnection;
        use dbus::channel::{MatchingReceiver, Sender as _};
        use dbus::message::MatchRule;
        use std::sync::mpsc::channel;

        /// What the stand-in server was asked.
        #[derive(Debug, PartialEq)]
        enum Asked {
            Notify { replaces: u32, title: String, body: String, actions: Vec<String>, resident: bool, urgency: u8, timeout: i32 },
            Close(u32),
        }
        let _alone = crate::bus::NOTIFICATION_SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let (told, asked) = channel::<Asked>();
        let (ready, started) = channel::<bool>();
        std::thread::spawn(move || {
            // Never in a server's place: no replacing it, no waiting in line.
            let Some(bus) = stand_in_bus() else {
                let _ = ready.send(false);
                return;
            };
            let finished = std::rc::Rc::new(std::cell::Cell::new(false));
            let over = std::rc::Rc::clone(&finished);
            let mut notified = 0;
            bus.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |call: dbus::Message, bus: &LocalConnection| {
                    let mut presses: Vec<(u32, &str)> = Vec::new();
                    let reply = match call.member().as_deref() {
                        Some("GetCapabilities") => call.method_return().append1(vec!["actions", "body", "body-markup", "persistence"]),
                        Some("Notify") => {
                            let (_, replaces, _, title, body, actions, hints, timeout): (String, u32, String, String, String, Vec<String>, dbus::arg::PropMap, i32) = call.read_all().unwrap();
                            let resident = dbus::arg::prop_cast::<bool>(&hints, "resident").copied().unwrap_or(false);
                            let urgency = dbus::arg::prop_cast::<u8>(&hints, "urgency").copied().unwrap_or(9);
                            let _ = told.send(Asked::Notify { replaces, title, body, actions, resident, urgency, timeout });
                            notified += 1;
                            // Pressed once shown; then another notification's button, and this one's.
                            presses = match notified {
                                1 => vec![(7, "pause")],
                                2 => vec![(8, "stop"), (7, "resume")],
                                _ => Vec::new(),
                            };
                            call.method_return().append1(7u32)
                        }
                        Some("CloseNotification") => {
                            let (id,): (u32,) = call.read_all().unwrap();
                            let _ = told.send(Asked::Close(id));
                            over.set(true);
                            call.method_return()
                        }
                        _ => return true,
                    };
                    let _ = bus.send(reply);
                    for (id, key) in presses {
                        let mut press = dbus::Message::new_signal(SERVER_PATH, SERVER, "ActionInvoked").unwrap().append2(id, key);
                        // To the connection that showed it alone, as Plasma does.
                        press.set_destination(call.sender().map(|s| s.into_static()));
                        let _ = bus.send(press);
                    }
                    true
                }),
            );
            let _ = ready.send(true);
            let until = std::time::Instant::now() + Duration::from_secs(20);
            while !finished.get() && std::time::Instant::now() < until {
                let _ = bus.process(Duration::from_millis(100));
            }
        });
        if !started.recv_timeout(Duration::from_secs(5)).unwrap_or(false) {
            eprintln!("No bus of the test's own (dbus-run-session), or a notification server on it already: skipped.");
            return;
        }
        let wait = Duration::from_secs(5);
        let buttons = |first: &str, label: &str| vec![(first.to_string(), label.to_string()), ("stop".to_string(), "Stop".to_string()), ("default".to_string(), "Open".to_string())];
        let (pressed, presses) = channel::<String>();
        let shown = ongoing("Write to the bank", "Since 14:02, for 25 min", buttons("pause", "Pause"), Box::new(move |key: &str| drop(pressed.send(key.to_string()))));
        let first = Asked::Notify {
            replaces: 0,
            title: "Write to the bank".into(),
            body: "Since 14:02, for 25 min".into(),
            actions: ["pause", "Pause", "stop", "Stop", "default", "Open"].map(String::from).to_vec(),
            resident: true,
            urgency: 1,
            // The server keeps it once its popup goes: its own time.
            timeout: -1,
        };
        assert_eq!(asked.recv_timeout(wait).unwrap(), first);
        // A button pressed at the server, heard here.
        assert_eq!(presses.recv_timeout(wait).unwrap(), "pause");
        // Changed in place; another notification's button is not this one's.
        shown.update("Write to the bank", "Paused, 3 min so far", buttons("resume", "Go on"));
        match asked.recv_timeout(wait).unwrap() {
            Asked::Notify { replaces, body, actions, .. } => assert_eq!((replaces, body.as_str(), actions[0].as_str()), (7, "Paused, 3 min so far", "resume")),
            other => panic!("{other:?}"),
        }
        assert_eq!(presses.recv_timeout(wait).unwrap(), "resume");
        // Taken away before Sioul goes on.
        shown.close(wait);
        assert_eq!(asked.recv_timeout(wait).unwrap(), Asked::Close(7));
        assert!(presses.try_recv().is_err(), "nothing else was pressed");
    }
}
