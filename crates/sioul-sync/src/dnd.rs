// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The desktop's do-not-disturb during Sioul's pauses (sioul-app's dnd.rs,
//! docs/pauses.md): who serves the notifications, Plasma's inhibition held
//! for as long as a pause lasts, and dconf's word that a GNOME setting
//! changed. Linux and the BSDs; the session's D-Bus.
//!
//! Plasma lifts an inhibition when the connection that asked for it closes:
//! each one is held by a thread of its own, which keeps that connection open
//! until it is released or Sioul ends (a crash never leaves the desktop
//! silenced), and asks again if Plasma's notification server starts again.

use dbus::arg::PropMap;
use dbus::blocking::LocalConnection;
use dbus::message::MatchRule;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The notification server, on the session bus.
const SERVER: &str = "org.freedesktop.Notifications";
const SERVER_PATH: &str = "/org/freedesktop/Notifications";
/// The bus itself.
const BUS: &str = "org.freedesktop.DBus";
const BUS_PATH: &str = "/org/freedesktop/DBus";
/// dconf's writer, which tells every change to GNOME's settings.
const DCONF_WRITER: &str = "ca.desrt.dconf.Writer";

/// How long a call to the server may take.
const CALL_WAIT: Duration = Duration::from_secs(3);
/// How long a holding thread listens before it looks at what Sioul asked.
const LISTENED: Duration = Duration::from_millis(250);
/// How long a holding thread may take to start.
const START_WAIT: Duration = Duration::from_secs(5);

/// Who serves the session's notifications, as it says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Server {
    /// "Plasma", "gnome-shell", "dunst"…
    pub name: String,
    /// "KDE", "GNOME"…
    pub vendor: String,
    pub version: String,
    pub capabilities: Vec<String>,
    /// Whether it lets an application inhibit notifications (Plasma's
    /// `Inhibit`, listed as the capability "inhibitions").
    pub inhibits: bool,
}

/// The session's notification server; none without a session bus, or when no
/// server is running (asking would start one: D-Bus activation).
pub fn server() -> Option<Server> {
    let bus = LocalConnection::new_session().ok()?;
    if !has_owner(&bus, SERVER) {
        return None;
    }
    let proxy = bus.with_proxy(SERVER, SERVER_PATH, CALL_WAIT);
    let (name, vendor, version, _spec): (String, String, String, String) = proxy.method_call(SERVER, "GetServerInformation", ()).ok()?;
    let capabilities = proxy.method_call(SERVER, "GetCapabilities", ()).map(|(found,): (Vec<String>,)| found).unwrap_or_default();
    let mut inhibits = capabilities.iter().any(|c| c == "inhibitions");
    // Plasma before it listed the capability: its methods say it.
    if !inhibits && vendor == "KDE" {
        use dbus::blocking::stdintf::org_freedesktop_dbus::Introspectable;
        inhibits = proxy.introspect().is_ok_and(|xml| xml.contains("name=\"Inhibit\""));
    }
    Some(Server { name, vendor, version, capabilities, inhibits })
}

/// Whether a name has an owner on the bus, without starting one.
fn has_owner(bus: &LocalConnection, name: &str) -> bool {
    bus.with_proxy(BUS, BUS_PATH, CALL_WAIT).method_call(BUS, "NameHasOwner", (name,)).is_ok_and(|(owned,): (bool,)| owned)
}

/// An inhibition of the desktop's notifications (Plasma's do-not-disturb,
/// shown "While Sioul is active (reason)"), held until released or dropped.
pub struct Inhibition {
    orders: Sender<Sender<Result<(), String>>>,
    alive: Arc<AtomicBool>,
    cookie: u32,
}

impl Inhibition {
    /// Whether its thread still holds the connection.
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// The server's number for it, as first given.
    pub fn cookie(&self) -> u32 {
        self.cookie
    }

    /// Lifted (`UnInhibit`), waiting at most `wait` for the server's answer.
    /// An inhibition the person ended meanwhile (from Plasma's applet) is
    /// lifted already: the server's refusal then says so, and is harmless.
    pub fn release(self, wait: Duration) -> Result<(), String> {
        let (done, answer) = std::sync::mpsc::channel();
        if self.orders.send(done).is_err() {
            return Ok(());
        }
        answer.recv_timeout(wait).unwrap_or_else(|_| Err("no answer from the notification server".to_string()))
    }
}

/// Asks the notification server to hold notifications back while Sioul
/// pauses: `desktop_entry` names the application (its desktop file, which
/// Plasma shows by its name), `reason` the pause. Fails when the server has
/// no such call or refuses; the error is the server's.
pub fn inhibit(desktop_entry: &str, reason: &str) -> Result<Inhibition, String> {
    let (orders, asked) = std::sync::mpsc::channel();
    let (ready, started) = std::sync::mpsc::channel();
    let alive = Arc::new(AtomicBool::new(true));
    let (entry, reason, living) = (desktop_entry.to_string(), reason.to_string(), Arc::clone(&alive));
    // The D-Bus connection cannot change threads, so it is made where it waits.
    std::thread::spawn(move || {
        hold(&entry, &reason, &asked, &ready);
        living.store(false, Ordering::Relaxed);
    });
    match started.recv_timeout(START_WAIT) {
        Ok(Ok(cookie)) => Ok(Inhibition { orders, alive, cookie }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("no answer from the notification server".to_string()),
    }
}

/// An inhibition's thread: asked for, held, asked again when the server
/// starts again (a new server holds none), lifted when Sioul says or goes.
fn hold(entry: &str, reason: &str, asked: &Receiver<Sender<Result<(), String>>>, ready: &Sender<Result<u32, String>>) {
    let bus = match LocalConnection::new_session() {
        Ok(bus) => bus,
        Err(e) => {
            let _ = ready.send(Err(said(&e)));
            return;
        }
    };
    // Plasma's notification server started again (plasmashell restarted): it
    // knows no inhibition. The person ending this one from the applet does
    // not change the name's owner, and is never undone here.
    let restarted: Rc<Cell<bool>> = Rc::default();
    let heard = Rc::clone(&restarted);
    let _ = bus.add_match(MatchRule::new_signal(BUS, "NameOwnerChanged").with_sender(BUS), move |(name, _old, new): (String, String, String), _: &LocalConnection, _: &dbus::Message| {
        if name == SERVER && !new.is_empty() {
            heard.set(true);
        }
        true
    });
    let server = bus.with_proxy(SERVER, SERVER_PATH, CALL_WAIT);
    let ask = || server.method_call(SERVER, "Inhibit", (entry, reason, PropMap::new())).map(|(cookie,): (u32,)| cookie);
    let mut cookie = match ask() {
        Ok(cookie) => cookie,
        Err(e) => {
            let _ = ready.send(Err(said(&e)));
            return;
        }
    };
    let _ = ready.send(Ok(cookie));
    let lift = |cookie: u32| -> Result<(), String> {
        if cookie == 0 {
            return Ok(());
        }
        server.method_call(SERVER, "UnInhibit", (cookie,)).map_err(|e| said(&e))
    };
    loop {
        match asked.try_recv() {
            Ok(done) => {
                let _ = done.send(lift(cookie));
                return;
            }
            Err(TryRecvError::Disconnected) => {
                let _ = lift(cookie);
                return;
            }
            Err(TryRecvError::Empty) => {}
        }
        if restarted.replace(false) {
            // Asked again; 0 when the new server refuses: nothing to lift then.
            cookie = ask().unwrap_or(0);
        }
        if bus.process(LISTENED).is_err() {
            return;
        }
    }
}

/// A D-Bus error in a few words: its message, else its name.
fn said(e: &dbus::Error) -> String {
    e.message().or(e.name()).unwrap_or("D-Bus").to_string()
}

/// The changes dconf tells to one key, heard while it lives: GNOME's
/// settings (and the person's switches in its top bar) write through dconf,
/// which says each change on the session bus. Dropped, it stops listening.
pub struct Changes {
    stop: Sender<()>,
    heard: Arc<Mutex<Vec<Instant>>>,
    alive: Arc<AtomicBool>,
}

impl Changes {
    /// How many changes were heard after `at`.
    pub fn since(&self, at: Instant) -> usize {
        self.heard.lock().map_or(0, |heard| heard.iter().filter(|when| **when > at).count())
    }

    /// Whether it still listens.
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}

impl Drop for Changes {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

/// Listens to dconf's changes of `key` ("/org/gnome/desktop/notifications/show-banners").
pub fn watch_dconf(key: &str) -> Result<Changes, String> {
    let (stop, stopped) = std::sync::mpsc::channel::<()>();
    let (ready, started) = std::sync::mpsc::channel();
    let heard: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let alive = Arc::new(AtomicBool::new(true));
    let (key, noted, living) = (key.to_string(), Arc::clone(&heard), Arc::clone(&alive));
    std::thread::spawn(move || {
        listen(&key, &noted, &stopped, &ready);
        living.store(false, Ordering::Relaxed);
    });
    match started.recv_timeout(START_WAIT) {
        Ok(Ok(())) => Ok(Changes { stop, heard, alive }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("the session bus did not answer".to_string()),
    }
}

fn listen(key: &str, heard: &Arc<Mutex<Vec<Instant>>>, stopped: &Receiver<()>, ready: &Sender<Result<(), String>>) {
    let bus = match LocalConnection::new_session() {
        Ok(bus) => bus,
        Err(e) => {
            let _ = ready.send(Err(said(&e)));
            return;
        }
    };
    let (key, noted) = (key.to_string(), Arc::clone(heard));
    let matched = bus.add_match(MatchRule::new_signal(DCONF_WRITER, "Notify"), move |(prefix, changes, _tag): (String, Vec<String>, String), _: &LocalConnection, _: &dbus::Message| {
        if touches(&key, &prefix, &changes)
            && let Ok(mut noted) = noted.lock()
        {
            noted.push(Instant::now());
        }
        true
    });
    if let Err(e) = matched {
        let _ = ready.send(Err(said(&e)));
        return;
    }
    let _ = ready.send(Ok(()));
    loop {
        match stopped.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {}
        }
        if bus.process(LISTENED).is_err() {
            return;
        }
    }
}

/// Whether dconf's change (`prefix`, and the paths under it that changed, ""
/// for the prefix itself) touches `key`: the key itself, or a folder holding
/// it reset whole.
fn touches(key: &str, prefix: &str, changes: &[String]) -> bool {
    let reaches = |path: &str| path == key || (path.ends_with('/') && key.starts_with(path));
    if changes.is_empty() {
        return reaches(prefix);
    }
    changes.iter().any(|change| reaches(&format!("{prefix}{change}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
    use dbus::channel::{MatchingReceiver, Sender as _};
    use std::sync::mpsc::channel;

    #[test]
    fn a_dconf_change_touches_its_key_or_a_folder_reset() {
        let key = "/org/gnome/desktop/notifications/show-banners";
        assert!(touches(key, key, &[String::new()]));
        assert!(touches(key, "/org/gnome/desktop/notifications/", &["show-banners".into(), "show-in-lock-screen".into()]));
        assert!(touches(key, "/org/gnome/desktop/notifications/", &[String::new()]), "the folder reset whole");
        assert!(touches(key, "/org/gnome/", &["desktop/notifications/".into()]));
        assert!(!touches(key, "/org/gnome/desktop/notifications/", &["show-in-lock-screen".into()]));
        assert!(!touches(key, "/org/gnome/desktop/notifications/show-banners-x", &[String::new()]));
        assert!(!touches(key, "/org/gnome/desktop/interface/", &["color-scheme".into()]));
    }

    /// What the stand-in notification server was asked.
    #[derive(Debug, PartialEq)]
    enum Asked {
        Inhibit { entry: String, reason: String, caller: String },
        UnInhibit(u32),
    }

    /// The tests that use the bus, one at a time: each stand-in needs the
    /// server's name to itself.
    static ON_THE_BUS: Mutex<()> = Mutex::new(());

    /// A stand-in for Plasma's notification server, on a bus of the test's
    /// own (dbus-run-session), recording what it is asked; its cookies start
    /// at `first`. None when the name keeps an owner (the desktop's own
    /// server): nothing here may reach it. It ends when `stop` is set, or
    /// after `life`, and gives up the name then.
    fn stand_in(first: u32, told: Sender<Asked>, stop: Arc<AtomicBool>, life: Duration) -> Option<()> {
        let (ready, started) = channel::<bool>();
        std::thread::spawn(move || {
            // Never in a server's place: no replacing it, no waiting in line;
            // a stand-in of the test before may take a moment to leave.
            let until = Instant::now() + Duration::from_secs(2);
            let owner = loop {
                let bus = LocalConnection::new_session().ok().filter(|bus| matches!(bus.request_name(SERVER, false, false, true), Ok(RequestNameReply::PrimaryOwner)));
                if bus.is_some() || Instant::now() >= until {
                    break bus;
                }
                std::thread::sleep(Duration::from_millis(50));
            };
            let Some(bus) = owner else {
                let _ = ready.send(false);
                return;
            };
            let next = Rc::new(Cell::new(first));
            bus.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |call: dbus::Message, bus: &LocalConnection| {
                    let reply = match call.member().as_deref() {
                        Some("GetServerInformation") => call.method_return().append3("Plasma", "KDE", "6.7.5").append1("1.2"),
                        Some("GetCapabilities") => call.method_return().append1(vec!["body", "actions", "persistence", "inhibitions"]),
                        Some("Inhibit") => {
                            let (entry, reason, _hints): (String, String, PropMap) = call.read3().unwrap();
                            let caller = call.sender().map(|s| s.to_string()).unwrap_or_default();
                            let _ = told.send(Asked::Inhibit { entry, reason, caller });
                            let cookie = next.get();
                            next.set(cookie + 1);
                            call.method_return().append1(cookie)
                        }
                        Some("UnInhibit") => {
                            let (cookie,): (u32,) = call.read_all().unwrap();
                            let _ = told.send(Asked::UnInhibit(cookie));
                            call.method_return()
                        }
                        _ => return true,
                    };
                    let _ = bus.send(reply);
                    true
                }),
            );
            let _ = ready.send(true);
            let until = Instant::now() + life;
            while !stop.load(Ordering::Relaxed) && Instant::now() < until {
                let _ = bus.process(Duration::from_millis(50));
            }
            let _ = bus.release_name(SERVER);
        });
        started.recv_timeout(Duration::from_secs(5)).unwrap_or(false).then_some(())
    }

    /// Whether a connection is still on the bus (the stand-in's view of Sioul's).
    fn connected(name: &str) -> bool {
        LocalConnection::new_session().is_ok_and(|bus| has_owner(&bus, name))
    }

    /// Plasma's inhibition, against a stand-in server on a bus of the test's
    /// own: run under dbus-run-session. Where a server is there already (the
    /// desktop's), skipped: nothing here may reach it.
    #[test]
    fn plasma_inhibition_is_held_while_the_pause_lasts_and_lifted_after() {
        let _alone = ON_THE_BUS.lock().unwrap_or_else(|e| e.into_inner());
        let (told, asked) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        if stand_in(42, told, Arc::clone(&stop), Duration::from_secs(30)).is_none() {
            eprintln!("No bus of the test's own (dbus-run-session), or a notification server on it already: skipped.");
            return;
        }
        let wait = Duration::from_secs(5);
        let found = server().expect("the stand-in answers");
        assert_eq!((found.name.as_str(), found.vendor.as_str(), found.inhibits), ("Plasma", "KDE", true));
        let held = inhibit("com.aurelienpierre.Sioul", "Pause").expect("inhibited");
        assert_eq!(held.cookie(), 42);
        let Asked::Inhibit { entry, reason, caller } = asked.recv_timeout(wait).unwrap() else { panic!("Inhibit first") };
        assert_eq!((entry.as_str(), reason.as_str()), ("com.aurelienpierre.Sioul", "Pause"));
        // The connection that asked stays on the bus while the pause lasts: Plasma
        // lifts an inhibition whose caller's connection closes.
        std::thread::sleep(Duration::from_millis(600));
        assert!(held.alive() && connected(&caller), "the inhibition's connection is kept");
        held.release(wait).expect("lifted");
        assert_eq!(asked.recv_timeout(wait).unwrap(), Asked::UnInhibit(42));
        // Then its connection goes.
        let until = Instant::now() + wait;
        while connected(&caller) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!connected(&caller), "the connection closed once lifted");
        // Dropped without a word (Sioul's pause state lost), it is lifted all the same.
        let dropped = inhibit("com.aurelienpierre.Sioul", "Free time").expect("inhibited");
        assert!(matches!(asked.recv_timeout(wait).unwrap(), Asked::Inhibit { reason, .. } if reason == "Free time"));
        drop(dropped);
        assert_eq!(asked.recv_timeout(wait).unwrap(), Asked::UnInhibit(43));
        stop.store(true, Ordering::Relaxed);
    }

    /// Plasma's server started again during a pause (plasmashell restarted)
    /// holds no inhibition: it is asked again, and lifted there at the end.
    #[test]
    fn plasma_restarted_is_asked_again() {
        let _alone = ON_THE_BUS.lock().unwrap_or_else(|e| e.into_inner());
        let (told, asked) = channel();
        let first_stop = Arc::new(AtomicBool::new(false));
        if stand_in(7, told.clone(), Arc::clone(&first_stop), Duration::from_secs(30)).is_none() {
            eprintln!("No bus of the test's own (dbus-run-session), or a notification server on it already: skipped.");
            return;
        }
        let wait = Duration::from_secs(5);
        let held = inhibit("com.aurelienpierre.Sioul", "Pause").expect("inhibited");
        assert!(matches!(asked.recv_timeout(wait).unwrap(), Asked::Inhibit { .. }));
        // The first server goes; another takes its name.
        first_stop.store(true, Ordering::Relaxed);
        let until = Instant::now() + wait;
        let second_stop = Arc::new(AtomicBool::new(false));
        while stand_in(100, told.clone(), Arc::clone(&second_stop), Duration::from_secs(30)).is_none() {
            assert!(Instant::now() < until, "the second stand-in could not take the name");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(matches!(asked.recv_timeout(wait).unwrap(), Asked::Inhibit { reason, .. } if reason == "Pause"), "asked again of the new server");
        held.release(wait).expect("lifted");
        assert_eq!(asked.recv_timeout(wait).unwrap(), Asked::UnInhibit(100));
        second_stop.store(true, Ordering::Relaxed);
    }

    /// dconf's word that a key changed, on a bus of the test's own. Skipped on
    /// a bus with a notification server (the desktop's): no signal of the
    /// test's may reach the person's session.
    #[test]
    fn dconf_changes_of_a_key_are_heard() {
        let _alone = ON_THE_BUS.lock().unwrap_or_else(|e| e.into_inner());
        // A stand-in of the test before may take a moment to leave the name.
        let free = |bus: &LocalConnection| {
            let until = Instant::now() + Duration::from_secs(1);
            while has_owner(bus, SERVER) && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(50));
            }
            !has_owner(bus, SERVER)
        };
        let Some(bus) = LocalConnection::new_session().ok().filter(|bus| free(bus)) else {
            eprintln!("No bus of the test's own (dbus-run-session): skipped.");
            return;
        };
        let key = "/org/gnome/desktop/notifications/show-banners";
        let changes = watch_dconf(key).expect("listening");
        let start = Instant::now();
        let notify = |prefix: &str, changed: Vec<&str>| {
            let signal = dbus::Message::new_signal("/ca/desrt/dconf/Writer/user", DCONF_WRITER, "Notify").unwrap().append3(prefix, changed, "tag");
            let _ = bus.send(signal);
            bus.process(Duration::from_millis(10)).ok();
        };
        notify("/org/gnome/desktop/interface/", vec!["color-scheme"]);
        notify(key, vec![""]);
        notify("/org/gnome/desktop/notifications/", vec!["show-in-lock-screen", "show-banners"]);
        let until = Instant::now() + Duration::from_secs(5);
        while changes.since(start) < 2 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(changes.since(start), 2, "two changes of the key, one of another");
        assert_eq!(changes.since(Instant::now()), 0);
        assert!(changes.alive());
    }
}
