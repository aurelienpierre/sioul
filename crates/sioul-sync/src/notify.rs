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

#[cfg(any(windows, target_os = "macos"))]
use notify_rust::{Notification, Timeout};
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

/// How long the notification stays: about the life of a code.
#[cfg(not(target_os = "android"))]
const SHOWN: u32 = 10 * 60 * 1000;

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
/// addresses it) and waits there: `pressed` runs with the key of the first
/// button pressed, and the wait ends then, or when the notification closes.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn notify(notice: Notice, pressed: Option<Box<dyn FnOnce(&str) + Send>>) -> Result<(), String> {
    use std::cell::RefCell;
    use std::rc::Rc;
    let Some(pressed) = pressed else {
        let bus = session().map_err(|e| said(&e))?;
        return send(&bus, &notice).map(drop).map_err(|e| said(&e));
    };
    // The D-Bus connection cannot change threads, so it is made where it waits.
    std::thread::spawn(move || {
        let Ok(bus) = session() else { return };
        let heard: Rc<RefCell<Vec<Heard>>> = Rc::default();
        if listened(&bus, &heard).is_err() {
            return;
        }
        let Ok(id) = send(&bus, &notice) else { return };
        loop {
            for item in std::mem::take(&mut *heard.borrow_mut()) {
                match item {
                    Heard::Pressed(at, key) if at == id => {
                        pressed(&key);
                        return;
                    }
                    Heard::Closed(at) if at == id => return,
                    _ => {}
                }
            }
            if bus.process(Duration::from_secs(1)).is_err() {
                return;
            }
        }
    });
    Ok(())
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
/// says heard, until it is taken away.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn serve(mut said: Said, asked: &Receiver<Change>, on_action: &dyn Fn(&str)) {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc::TryRecvError;
    let Ok(connection) = session() else { return };
    let heard: Rc<RefCell<Vec<Heard>>> = Rc::default();
    if listened(&connection, &heard).is_err() {
        return;
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
                Ok(Change::Show(next)) => (said, fresh) = (next, true),
                Ok(Change::Close(done)) => {
                    close_lasting(&server, id);
                    if let Some(done) = done {
                        let _ = done.send(());
                    }
                    return;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return close_lasting(&server, id),
            }
        }
        if fresh || refused.is_some_and(|at| at.elapsed() >= ASKED_AGAIN) {
            if capabilities.is_empty() {
                capabilities = server.method_call(SERVER, "GetCapabilities", ()).map(|(found,): (Vec<String>,)| found).unwrap_or_default();
            }
            match show_lasting(&server, id, &said, &capabilities) {
                Ok(shown) => (id, refused) = (shown, None),
                Err(_) => refused = Some(std::time::Instant::now()),
            }
            fresh = false;
        }
        if connection.process(LISTENED).is_err() {
            return;
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
