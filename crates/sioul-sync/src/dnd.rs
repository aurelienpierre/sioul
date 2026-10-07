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
//!
//! Both ways (docs/do-not-disturb.md): the server's `Inhibited` followed
//! (`watch_inhibited`), each change told once it held a second and Sioul's
//! own change settled; dconf's changes of a key told as they come
//! (`watch_dconf_told`), for GNOME's switch.

use dbus::arg::PropMap;
use dbus::blocking::LocalConnection;
use dbus::message::MatchRule;
use std::cell::{Cell, RefCell};
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
/// How long a change of `Inhibited` must hold before it is read again and told.
const HELD: Duration = Duration::from_secs(1);
/// D-Bus's properties, whose `PropertiesChanged` says a change of `Inhibited`.
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

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

/// The connection that owns a name now (":1.42"); none when nobody does.
fn owner_of(bus: &LocalConnection, name: &str) -> Option<String> {
    bus.with_proxy(BUS, BUS_PATH, CALL_WAIT).method_call(BUS, "GetNameOwner", (name,)).ok().map(|(owner,): (String,)| owner)
}

/// What the notification server says of `Inhibited` now: Plasma's do-not-disturb
/// as its notifications applet works it out (the person's own, any
/// application's inhibition, a full-screen window, mirrored screens). None
/// without a server, or one that does not say it.
pub fn inhibited() -> Option<bool> {
    let bus = LocalConnection::new_session().ok()?;
    if !has_owner(&bus, SERVER) {
        return None;
    }
    read_inhibited(&bus)
}

fn read_inhibited(bus: &LocalConnection) -> Option<bool> {
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
    bus.with_proxy(SERVER, SERVER_PATH, CALL_WAIT).get::<bool>(SERVER, "Inhibited").ok()
}

/// The session's do-not-disturb followed, as the notification server says it
/// (`Inhibited`): each change that held a second, read again then, is told
/// once Sioul's own change settled (`quiet`: a time Sioul sets before each of
/// its own changes). A new server (plasmashell started again) is read without
/// telling: what it says then is nobody's act. Dropped, it stops.
pub struct Watch {
    stop: Sender<()>,
    seen: Arc<Mutex<Option<bool>>>,
    alive: Arc<AtomicBool>,
}

impl Watch {
    /// The state last read; none before a server answered.
    pub fn seen(&self) -> Option<bool> {
        self.seen.lock().ok().and_then(|seen| *seen)
    }

    /// Whether it still follows.
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

/// Follows the server's `Inhibited`, on a thread of its own: `told(on)` hears
/// each change, on that thread (it should hand any long work on).
pub fn watch_inhibited(quiet: Arc<Mutex<Instant>>, told: Box<dyn Fn(bool) + Send>) -> Result<Watch, String> {
    let (stop, stopped) = std::sync::mpsc::channel::<()>();
    let (ready, started) = std::sync::mpsc::channel();
    let seen: Arc<Mutex<Option<bool>>> = Arc::default();
    let alive = Arc::new(AtomicBool::new(true));
    let (noted, living) = (Arc::clone(&seen), Arc::clone(&alive));
    std::thread::spawn(move || {
        follow_inhibited(&noted, &quiet, &*told, &stopped, &ready);
        living.store(false, Ordering::Relaxed);
    });
    match started.recv_timeout(START_WAIT) {
        Ok(Ok(())) => Ok(Watch { stop, seen, alive }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("the session bus did not answer".to_string()),
    }
}

fn follow_inhibited(seen: &Mutex<Option<bool>>, quiet: &Mutex<Instant>, told: &dyn Fn(bool), stopped: &Receiver<()>, ready: &Sender<Result<(), String>>) {
    let bus = match LocalConnection::new_session() {
        Ok(bus) => bus,
        Err(e) => {
            let _ = ready.send(Err(said(&e)));
            return;
        }
    };
    // When a change was heard, to read it again once it held; and whether it
    // came with a new server, read then without telling.
    let heard: Rc<Cell<Option<(Instant, bool)>>> = Rc::default();
    let owner = Rc::new(RefCell::new(owner_of(&bus, SERVER).unwrap_or_default()));
    let (new_owner, server_changed) = (Rc::clone(&owner), Rc::clone(&heard));
    let owners = bus.add_match(MatchRule::new_signal(BUS, "NameOwnerChanged").with_sender(BUS), move |(name, _old, new): (String, String, String), _: &LocalConnection, _: &dbus::Message| {
        if name == SERVER {
            *new_owner.borrow_mut() = new;
            server_changed.set(Some((Instant::now(), true)));
        }
        true
    });
    let (from, property_changed) = (Rc::clone(&owner), Rc::clone(&heard));
    let changes = bus.add_match(MatchRule::new_signal(PROPERTIES, "PropertiesChanged").with_path(SERVER_PATH), move |(interface, changed, _gone): (String, PropMap, Vec<String>), _: &LocalConnection, message: &dbus::Message| {
        let ours = message.sender().is_some_and(|sender| *from.borrow() == sender.to_string());
        if ours && interface == SERVER && changed.contains_key("Inhibited") {
            let fresh = property_changed.get().is_some_and(|(_, fresh)| fresh);
            property_changed.set(Some((Instant::now(), fresh)));
        }
        true
    });
    if let Err(e) = owners.and(changes) {
        let _ = ready.send(Err(said(&e)));
        return;
    }
    // The state now, read without telling: found, not heard.
    if let Ok(mut noted) = seen.lock() {
        *noted = read_inhibited(&bus);
    }
    let _ = ready.send(Ok(()));
    loop {
        match stopped.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {}
        }
        if let Some((at, fresh)) = heard.get() {
            let settled = quiet.lock().map_or(true, |quiet| Instant::now() >= *quiet);
            if at.elapsed() >= HELD && settled {
                heard.set(None);
                let now = read_inhibited(&bus);
                let before = seen.lock().ok().and_then(|mut noted| std::mem::replace(&mut *noted, now));
                if !fresh
                    && let (Some(before), Some(now)) = (before, now)
                    && before != now
                {
                    told(now);
                }
            }
        }
        if bus.process(LISTENED).is_err() {
            return;
        }
    }
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
    watch(key, None)
}

/// Listens to dconf's changes of `key`, and calls `told` at each, on the
/// listening thread (it should hand any long work on).
pub fn watch_dconf_told(key: &str, told: Box<dyn Fn() + Send>) -> Result<Changes, String> {
    watch(key, Some(told))
}

fn watch(key: &str, told: Option<Box<dyn Fn() + Send>>) -> Result<Changes, String> {
    let (stop, stopped) = std::sync::mpsc::channel::<()>();
    let (ready, started) = std::sync::mpsc::channel();
    let heard: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let alive = Arc::new(AtomicBool::new(true));
    let (key, noted, living) = (key.to_string(), Arc::clone(&heard), Arc::clone(&alive));
    std::thread::spawn(move || {
        listen(&key, &noted, told, &stopped, &ready);
        living.store(false, Ordering::Relaxed);
    });
    match started.recv_timeout(START_WAIT) {
        Ok(Ok(())) => Ok(Changes { stop, heard, alive }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("the session bus did not answer".to_string()),
    }
}

fn listen(key: &str, heard: &Arc<Mutex<Vec<Instant>>>, told: Option<Box<dyn Fn() + Send>>, stopped: &Receiver<()>, ready: &Sender<Result<(), String>>) {
    let bus = match LocalConnection::new_session() {
        Ok(bus) => bus,
        Err(e) => {
            let _ = ready.send(Err(said(&e)));
            return;
        }
    };
    let (key, noted) = (key.to_string(), Arc::clone(heard));
    let matched = bus.add_match(MatchRule::new_signal(DCONF_WRITER, "Notify"), move |(prefix, changes, _tag): (String, Vec<String>, String), _: &LocalConnection, _: &dbus::Message| {
        if touches(&key, &prefix, &changes) {
            if let Ok(mut noted) = noted.lock() {
                noted.push(Instant::now());
            }
            if let Some(told) = &told {
                told();
            }
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
        stand_in_saying(first, told, stop, life, None)
    }

    /// What a stand-in says of `Inhibited`, and the values it is given to
    /// announce (`PropertiesChanged`, as Plasma's server does).
    struct Says {
        value: Arc<AtomicBool>,
        set: Receiver<bool>,
    }

    /// `stand_in`, answering `Inhibited` too when `says` is given.
    fn stand_in_saying(first: u32, told: Sender<Asked>, stop: Arc<AtomicBool>, life: Duration, says: Option<Says>) -> Option<()> {
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
            let value = says.as_ref().map(|says| Arc::clone(&says.value));
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
                        Some("Get") if call.interface().as_deref() == Some(PROPERTIES) => {
                            let (_interface, name): (String, String) = call.read2().unwrap();
                            match &value {
                                Some(value) if name == "Inhibited" => call.method_return().append1(dbus::arg::Variant(value.load(Ordering::Relaxed))),
                                _ => return true,
                            }
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
                // Each value given, announced as Plasma's server does.
                while let Some(on) = says.as_ref().and_then(|says| says.set.try_recv().ok()) {
                    if let Some(says) = &says {
                        says.value.store(on, Ordering::Relaxed);
                    }
                    let mut changed = PropMap::new();
                    changed.insert("Inhibited".to_string(), dbus::arg::Variant(Box::new(on) as Box<dyn dbus::arg::RefArg>));
                    let signal = dbus::Message::new_signal(SERVER_PATH, PROPERTIES, "PropertiesChanged").unwrap().append3(SERVER, changed, Vec::<String>::new());
                    let _ = bus.send(signal);
                }
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

    /// The session's do-not-disturb followed (Plasma's `Inhibited`), against a
    /// stand-in server on a bus of the test's own: a change that holds a second
    /// is told; a flicker is not; nothing before Sioul's own change settled; a
    /// new server is read without telling.
    #[test]
    fn the_session_s_do_not_disturb_is_followed() {
        let _alone = ON_THE_BUS.lock().unwrap_or_else(|e| e.into_inner());
        let (told, _asked) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let (set, announced) = channel();
        let says = Says { value: Arc::new(AtomicBool::new(false)), set: announced };
        if stand_in_saying(1, told.clone(), Arc::clone(&stop), Duration::from_secs(40), Some(says)).is_none() {
            eprintln!("No bus of the test's own (dbus-run-session), or a notification server on it already: skipped.");
            return;
        }
        assert_eq!(inhibited(), Some(false));
        let quiet = Arc::new(Mutex::new(Instant::now()));
        let (heard, hears) = channel();
        let watch = watch_inhibited(Arc::clone(&quiet), Box::new(move |on| {
            let _ = heard.send(on);
        }))
        .expect("following");
        assert_eq!(watch.seen(), Some(false), "read at the start, not told");
        // Turned on (the applet, a full-screen window…): told once it held.
        set.send(true).unwrap();
        assert_eq!(hears.recv_timeout(Duration::from_secs(5)), Ok(true));
        assert_eq!(watch.seen(), Some(true));
        // A flicker within the second: read again after, unchanged, not told.
        set.send(false).unwrap();
        set.send(true).unwrap();
        assert!(hears.recv_timeout(Duration::from_millis(2_500)).is_err(), "a flicker is no change");
        // Sioul's own change: nothing before it settled, then what holds.
        *quiet.lock().unwrap() = Instant::now() + Duration::from_secs(2);
        set.send(false).unwrap();
        assert!(hears.recv_timeout(Duration::from_millis(1_500)).is_err(), "not before Sioul's change settled");
        assert_eq!(hears.recv_timeout(Duration::from_secs(4)), Ok(false));
        // On again within the settle after Sioul's own change turned it off: on
        // before, on after, nothing told; no press, no loop.
        set.send(true).unwrap();
        assert_eq!(hears.recv_timeout(Duration::from_secs(5)), Ok(true));
        *quiet.lock().unwrap() = Instant::now() + Duration::from_secs(2);
        set.send(false).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        set.send(true).unwrap();
        assert!(hears.recv_timeout(Duration::from_millis(4_000)).is_err(), "on before, on after: nothing");
        assert_eq!(watch.seen(), Some(true));
        // plasmashell started again, its do-not-disturb on: read, not told.
        stop.store(true, Ordering::Relaxed);
        let wait = Duration::from_secs(5);
        let second_stop = Arc::new(AtomicBool::new(false));
        let (_set_again, announced_again) = channel();
        let says_again = Says { value: Arc::new(AtomicBool::new(true)), set: announced_again };
        // It waits for the first to leave the name (two seconds at most).
        assert!(stand_in_saying(50, told.clone(), Arc::clone(&second_stop), Duration::from_secs(30), Some(says_again)).is_some(), "the second stand-in took the name");
        let until = Instant::now() + wait;
        while watch.seen() != Some(true) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(watch.seen(), Some(true), "the new server read");
        assert!(hears.recv_timeout(Duration::from_millis(500)).is_err(), "and not told");
        assert!(watch.alive());
        second_stop.store(true, Ordering::Relaxed);
    }

    /// dconf's changes of a key told as they come, on a bus of the test's own.
    #[test]
    fn dconf_tells_each_change_of_its_key() {
        let _alone = ON_THE_BUS.lock().unwrap_or_else(|e| e.into_inner());
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
        let (heard, hears) = channel();
        let _changes = watch_dconf_told(key, Box::new(move || {
            let _ = heard.send(());
        }))
        .expect("listening");
        let notify = |prefix: &str, changed: Vec<&str>| {
            let signal = dbus::Message::new_signal("/ca/desrt/dconf/Writer/user", DCONF_WRITER, "Notify").unwrap().append3(prefix, changed, "tag");
            let _ = bus.send(signal);
            bus.process(Duration::from_millis(10)).ok();
        };
        notify("/org/gnome/desktop/interface/", vec!["color-scheme"]);
        notify(key, vec![""]);
        assert_eq!(hears.recv_timeout(Duration::from_secs(5)), Ok(()));
        assert!(hears.recv_timeout(Duration::from_millis(300)).is_err(), "another key's change is not told");
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
