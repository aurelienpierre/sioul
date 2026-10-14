// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Every D-Bus connection Sioul makes on Linux and the BSDs, made here alone:
//! the system's reading (power.rs), the notifications (notify.rs), the
//! desktop's do-not-disturb (dnd.rs), colord's word on the screens
//! (sioul-app's colour.rs).
//!
//! libdbus's own ways to reach a bus (`dbus_bus_get`, `dbus_bus_get_private`,
//! `dbus_bus_register`, behind the `dbus` crate's `new_session`, `new_system`
//! and `new_address`) hold its process-wide locks across the connection's
//! `Hello`, which they wait for without limit: a bus that takes a connection
//! and never answers held every other D-Bus connection of the process with
//! it, being made or closed, and Qt's own at Sioul's exit (review of battery
//! part C, G1). Here the connection is opened as a plain one
//! (`dbus_connection_open_private`, which takes none of those locks), its
//! `Hello` said by Sioul, and the whole setup (the socket, the
//! authentication, the `Hello`'s answer) waited for on a thread of its own,
//! `SETUP` at most: such a bus then holds that one thread, never the caller
//! and never another connection. While a setup to a bus has been under way
//! longer than its limit, none more is tried there: the next is refused at
//! once, until the bus answers it, or another bus answers at the same place
//! (a socket file made anew: `socket_of`).
//!
//! The connection made is the same as before in every other way: private,
//! not closing the program when the bus goes, its name given by the bus.
//! Only libdbus's own record of that name is not kept (`unique_name` says
//! none), which nothing in Sioul reads.

use dbus::Message;
use dbus::channel::Channel;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How long a connection's setup may take: its socket, its authentication
/// and its `Hello`'s answer.
pub const SETUP: Duration = Duration::from_secs(5);

/// The setups under way: the bus's address, when it began, its limit, and its number.
static UNDER_WAY: Mutex<Vec<Setup>> = Mutex::new(Vec::new());

/// A setup under way (`UNDER_WAY`).
#[derive(Debug, Clone)]
struct Setup {
    address: String,
    /// The socket file it went to, as it was then (`socket_of`): another bus
    /// started at the same place is another socket file.
    socket: Option<(u64, u64)>,
    began: Instant,
    limit: Duration,
    number: u64,
}

/// Whether a new setup to `address`, whose socket file is `socket` now, is
/// refused at `now`: one under way to the same bus (the same address, the
/// same socket file) has been so longer than its limit (the bus takes
/// connections and does not answer them). A bus started again at that path,
/// the old one hung but alive, is another bus, tried at once.
fn refused(under_way: &[Setup], address: &str, socket: Option<(u64, u64)>, now: Instant) -> bool {
    under_way.iter().any(|setup| setup.address == address && setup.socket == socket && now.saturating_duration_since(setup.began) >= setup.limit)
}

/// The socket file of a `unix:path=` address (its first one), as it is now:
/// its device and inode; none for another kind of address (an abstract
/// socket's name, TCP), or no file there.
fn socket_of(address: &str) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let found = std::fs::metadata(unix_path(address)?).ok()?;
    Some((found.dev(), found.ino()))
}

/// The path a `unix:path=` address names, its escapes undone (`escaped`).
pub(crate) fn unix_path(address: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let first = address.split(';').next()?.strip_prefix("unix:")?;
    let value = first.split(',').find_map(|pair| pair.strip_prefix("path="))?;
    let mut bytes = Vec::with_capacity(value.len());
    let mut rest = value.as_bytes();
    while let Some((&byte, after)) = rest.split_first() {
        if byte == b'%' {
            let hex = std::str::from_utf8(after.get(..2)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = &after[2..];
        } else {
            bytes.push(byte);
            rest = after;
        }
    }
    Some(std::path::PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// A connection to the bus at `address` (a D-Bus address, as
/// `DBUS_SESSION_BUS_ADDRESS` gives one), made within `SETUP`.
pub fn connect(address: &str) -> Result<Channel, dbus::Error> {
    connect_within(address, SETUP)
}

/// `connect`, made within `limit`: on a thread of its own, its answer waited
/// for `limit` at most; refused at once while an earlier setup to the same
/// bus has been under way longer than its own limit (`refused`).
pub fn connect_within(address: &str, limit: Duration) -> Result<Channel, dbus::Error> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    let socket = socket_of(address);
    {
        let mut under_way = UNDER_WAY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if refused(&under_way, address, socket, Instant::now()) {
            return Err(dbus::Error::new_failed("the bus did not answer a connection in time"));
        }
        under_way.push(Setup { address: address.to_string(), socket, began: Instant::now(), limit, number });
    }
    let (sender, answer) = std::sync::mpsc::channel();
    let target = address.to_string();
    let started = std::thread::Builder::new().name("sioul-bus-setup".into()).spawn(move || {
        let made = set_up(&target, limit);
        finished(number);
        // Given up meanwhile: dropped here, and the connection with it.
        let _ = sender.send(made);
    });
    if started.is_err() {
        finished(number);
        return Err(dbus::Error::new_failed("no thread for the connection"));
    }
    answer.recv_timeout(limit).unwrap_or_else(|_| Err(dbus::Error::new_failed("the bus did not answer the connection in time")))
}

/// Setup `number` no longer under way.
fn finished(number: u64) {
    UNDER_WAY.lock().unwrap_or_else(std::sync::PoisonError::into_inner).retain(|setup| setup.number != number);
}

/// The setup itself: the socket, then this connection's `Hello`, said here
/// and its answer waited for `limit` at most once it went out (it goes out
/// only once the bus answered the authentication).
fn set_up(address: &str, limit: Duration) -> Result<Channel, dbus::Error> {
    let channel = Channel::open_private(address)?;
    let hello = Message::new_method_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "Hello").map_err(|e| dbus::Error::new_failed(&e))?;
    channel.send_with_reply_and_block(hello, limit)?;
    Ok(channel)
}

/// A connection to the session's bus (`session_address`), made within `SETUP`.
pub fn session() -> Result<Channel, dbus::Error> {
    let address = session_address().ok_or_else(|| dbus::Error::new_failed("no session bus"))?;
    connect(&address)
}

/// A connection to the system's bus (`system_address`), made within `SETUP`.
pub fn system() -> Result<Channel, dbus::Error> {
    connect(&system_address())
}

/// The session's bus as libdbus finds it: `DBUS_SESSION_BUS_ADDRESS`, else
/// the user's bus in `XDG_RUNTIME_DIR` when there is one; none, no session bus.
pub fn session_address() -> Option<String> {
    if let Some(address) = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok().filter(|address| !address.is_empty()) {
        return Some(address);
    }
    let bus = std::path::PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("bus");
    bus.exists().then(|| format!("unix:path={}", escaped(&bus)))
}

/// The system's bus as libdbus finds it: `DBUS_SYSTEM_BUS_ADDRESS`, else its usual socket.
pub fn system_address() -> String {
    std::env::var("DBUS_SYSTEM_BUS_ADDRESS").ok().filter(|address| !address.is_empty()).unwrap_or_else(|| "unix:path=/var/run/dbus/system_bus_socket".to_string())
}

/// A path as a D-Bus address writes it: each byte but letters, digits and
/// `-_/.\*` as `%` and its two hex digits.
pub fn escaped(path: &std::path::Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().iter().map(|&byte| if byte.is_ascii_alphanumeric() || b"-_/.\\*".contains(&byte) { char::from(byte).to_string() } else { format!("%{byte:02x}") }).collect()
}

/// The tests that stand in for the session's notification server, on the
/// test's own bus (dbus-run-session), take turns: one name, one owner.
#[cfg(test)]
pub(crate) static NOTIFICATION_SERVER: Mutex<()> = Mutex::new(());

/// Two stand-in buses for tests: private `dbus-daemon`s in a scratch folder,
/// never the session or the system of whoever runs them; ended and removed
/// however the test ends.
#[cfg(test)]
pub(crate) struct StandIns {
    dir: std::path::PathBuf,
    daemons: Vec<std::process::Child>,
}

#[cfg(test)]
impl StandIns {
    /// A scratch folder named after `name` and this process.
    pub(crate) fn new(name: &str) -> StandIns {
        let dir = std::env::temp_dir().join(format!("sioul-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        StandIns { dir, daemons: Vec::new() }
    }

    /// A private bus named `name`: its pid and address; none of the system's
    /// services on it, nothing started for a name asked, as the session's own
    /// policy otherwise (session.conf).
    pub(crate) fn start(&mut self, name: &str) -> (u32, String) {
        use std::process::{Command, Stdio};
        let socket = self.dir.join(name);
        let _ = std::fs::remove_file(&socket);
        let address = format!("unix:path={}", escaped(&socket));
        let config = self.dir.join(format!("{name}.conf"));
        let policy = "<allow send_destination=\"*\" eavesdrop=\"true\"/><allow eavesdrop=\"true\"/><allow own=\"*\"/>";
        std::fs::write(&config, format!("<!DOCTYPE busconfig PUBLIC \"-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN\" \"http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd\">\n<busconfig><type>session</type><listen>{address}</listen><auth>EXTERNAL</auth><policy context=\"default\">{policy}</policy></busconfig>\n")).unwrap();
        let daemon = Command::new("dbus-daemon").arg(format!("--config-file={}", config.display())).arg("--nofork").stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("dbus-daemon");
        let pid = daemon.id();
        self.daemons.push(daemon);
        let until = Instant::now() + Duration::from_secs(5);
        while !socket.exists() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(socket.exists(), "{name}: no socket");
        (pid, address)
    }

    /// Sends a signal ("-STOP", "-CONT", "-KILL") to a stand-in.
    pub(crate) fn signal(pid: u32, signal: &str) {
        let sent = std::process::Command::new("kill").args([signal, &pid.to_string()]).status().is_ok_and(|status| status.success());
        assert!(sent, "kill {signal} {pid}");
    }
}

#[cfg(test)]
impl Drop for StandIns {
    fn drop(&mut self) {
        for daemon in &mut self.daemons {
            let _ = daemon.kill();
            let _ = daemon.wait();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A setup under way longer than its limit refuses the next ones to the
    /// same bus at once (the bus takes connections and does not answer them:
    /// one thread held, never one more); within its limit, to another bus, or
    /// to another bus started at the same path (another socket file), they go
    /// ahead (review of battery part C, L1).
    #[test]
    fn a_setup_stuck_past_its_limit_refuses_the_next_ones() {
        let began = Instant::now();
        let old = Some((44, 1001));
        let stuck = Setup { address: "unix:path=/run/a".into(), socket: old, began, limit: SETUP, number: 1 };
        let under_way = [stuck];
        assert!(!refused(&under_way, "unix:path=/run/a", old, began + SETUP - Duration::from_millis(1)), "within its limit: another may go");
        assert!(refused(&under_way, "unix:path=/run/a", old, began + SETUP), "past it: refused");
        assert!(!refused(&under_way, "unix:path=/run/b", None, began + SETUP * 3), "another bus goes ahead");
        assert!(!refused(&under_way, "unix:path=/run/a", Some((44, 2002)), began + SETUP * 3), "a bus started again at the same path: tried");
        assert!(!refused(&under_way, "unix:path=/run/a", None, began + SETUP * 3), "no socket there now: tried (and fails at once)");
        assert!(!refused(&[], "unix:path=/run/a", old, began));
        // An address without a file (an abstract socket): refused while stuck, as before.
        let abstract_bus = [Setup { address: "unix:abstract=sioul".into(), socket: None, began, limit: SETUP, number: 2 }];
        assert!(refused(&abstract_bus, "unix:abstract=sioul", None, began + SETUP));
    }

    /// The buses' addresses as libdbus finds them, a path written as an
    /// address writes it, and read back from one.
    #[test]
    fn addresses_are_written_as_d_bus_writes_them() {
        assert_eq!(escaped(std::path::Path::new("/run/user/1000/bus")), "/run/user/1000/bus");
        assert_eq!(escaped(std::path::Path::new("/tmp/a b,c;é")), "/tmp/a%20b%2cc%3b%c3%a9");
        let odd = std::path::Path::new("/tmp/a b,c;é");
        assert_eq!(unix_path(&format!("unix:path={}", escaped(odd))).as_deref(), Some(odd));
        assert_eq!(unix_path("unix:path=/run/user/1000/bus,guid=0123abcd;unix:path=/other").as_deref(), Some(std::path::Path::new("/run/user/1000/bus")));
        assert_eq!(unix_path("unix:abstract=/tmp/dbus-abc"), None);
        assert_eq!(unix_path("tcp:host=localhost,port=4444"), None);
        assert_eq!(unix_path("unix:path=/tmp/broken%2"), None, "a broken escape reads as no path");
    }

    /// A bus that does not exist is refused at once, nothing held.
    #[test]
    fn a_bus_that_is_not_there_is_refused_at_once() {
        let start = Instant::now();
        assert!(connect("unix:path=/nonexistent/sioul-no-bus").is_err());
        assert!(start.elapsed() < Duration::from_secs(2), "{:?}", start.elapsed());
        assert!(UNDER_WAY.lock().unwrap().iter().all(|setup| setup.address != "unix:path=/nonexistent/sioul-no-bus"), "nothing left under way");
    }

    /// Whether `text` names `name` as code does: a whole word, not a part of
    /// a longer name, wherever it stands (a call, a function used as a value,
    /// a space before its parenthesis, a comment).
    fn named(text: &str, name: &str) -> bool {
        let part = |c: char| c.is_alphanumeric() || c == '_';
        text.match_indices(name).any(|(at, _)| !text[..at].chars().next_back().is_some_and(part) && !text[at + name.len()..].chars().next().is_some_and(part))
    }

    /// Whether `text` reaches `name` as a method or a path's last part
    /// (`.name`, `::name`, spaces allowed before it).
    fn reached(text: &str, name: &str) -> bool {
        let part = |c: char| c.is_alphanumeric() || c == '_';
        text.match_indices(name).any(|(at, _)| {
            let before = text[..at].trim_end();
            !text[at + name.len()..].chars().next().is_some_and(part) && (before.ends_with('.') || before.ends_with("::"))
        })
    }

    /// No source of Sioul's outside this file reaches a bus through libdbus's
    /// own registration, nor opens a connection itself: no file under crates/
    /// (whatever its kind, folders named "target" there included; Qt's
    /// compiled resources aside) names the `dbus` crate's `new_session`,
    /// `new_system`, `new_address`, `get_private`, `open_private` or calls a
    /// `register`, nor libdbus's own functions, nor the older `ffidisp` side,
    /// nor another way to a bus (`zbus`, the Secret Service's crates: the
    /// keyring reaches it inside the `keyring` crate alone, as it does until
    /// the owner decides: docs/android.md), nor notify-rust (which registers
    /// so) but notify.rs, in any way of writing them; no C or C++ uses Qt's
    /// D-Bus, and no build script asks for its module. The manifests, read
    /// as TOML (tables, renamed packages, targets): notify-rust for Windows
    /// and macOS alone, the `dbus` crate in sioul-sync alone, no other D-Bus
    /// crate; and the lock has no other crate reaching libdbus than those and
    /// the keyring's (review of battery part C, L2 of the third and fourth
    /// reviews).
    #[test]
    fn nothing_but_this_file_reaches_a_bus() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (mut files, mut manifests) = (Vec::new(), Vec::new());
        let mut folders = vec![root.join("crates")];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    folders.push(path);
                } else if path.ends_with("Cargo.toml") {
                    manifests.push(path);
                } else if !path.to_string_lossy().ends_with("_rc.py") {
                    files.push(path);
                }
            }
        }
        assert!(files.len() > 500 && manifests.len() >= 5, "{} files, {} manifests", files.len(), manifests.len());
        let words = ["new_session", "new_system", "new_address", "get_private", "open_private", "dbus_bus_get", "dbus_bus_get_private", "dbus_bus_register", "dbus_connection_open", "dbus_connection_open_private", "ffidisp", "libdbus_sys", "dbus_secret_service", "secret_service", "zbus"];
        for file in &files {
            if file.ends_with("sioul-sync/src/bus.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(file) else { continue };
            for word in words {
                assert!(!named(&text, word), "{}: {word} (connections through sioul_sync::bus alone)", file.display());
            }
            assert!(!reached(&text, "register"), "{}: a register call (libdbus's own registration)", file.display());
            if !file.ends_with("sioul-sync/src/notify.rs") {
                assert!(!named(&text, "notify_rust"), "{}: notify-rust (registers through libdbus)", file.display());
            }
            if !file.extension().is_some_and(|e| e == "rs") {
                assert!(!text.contains("QDBus") && !text.contains("QtDBus"), "{}: Qt's D-Bus", file.display());
            }
            if file.ends_with("build.rs") {
                assert!(!text.contains("\"DBus\""), "{}: Qt's D-Bus module", file.display());
            }
        }
        // The manifests, as TOML: each dependency by its package's name, wherever it is declared.
        manifests.push(root.join("Cargo.toml"));
        for manifest in &manifests {
            let table: toml::Table = std::fs::read_to_string(manifest).unwrap().parse().unwrap();
            let own = manifest.ends_with("sioul-sync/Cargo.toml");
            let workspace = *manifest == root.join("Cargo.toml");
            let mut declared: Vec<(String, String)> = Vec::new();
            let mut sections: Vec<(String, &toml::Table)> = Vec::new();
            for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(deps) = table.get(kind).and_then(toml::Value::as_table) {
                    sections.push((format!("[{kind}]"), deps));
                }
                if let Some(deps) = table.get("workspace").and_then(|w| w.get(kind)).and_then(toml::Value::as_table) {
                    sections.push((format!("[workspace.{kind}]"), deps));
                }
            }
            if let Some(targets) = table.get("target").and_then(toml::Value::as_table) {
                for (target, inner) in targets {
                    for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
                        if let Some(deps) = inner.get(kind).and_then(toml::Value::as_table) {
                            sections.push((format!("[target.'{target}'.{kind}]"), deps));
                        }
                    }
                }
            }
            for (section, deps) in sections {
                for (name, spec) in deps {
                    let package = spec.get("package").and_then(toml::Value::as_str).unwrap_or(name);
                    declared.push((section.clone(), package.to_string()));
                }
            }
            for (section, package) in &declared {
                match package.as_str() {
                    "notify-rust" => assert!((own && section == "[target.'cfg(any(windows, target_os = \"macos\"))'.dependencies]") || (workspace && section == "[workspace.dependencies]"), "{}: notify-rust under {section}", manifest.display()),
                    "dbus" => assert!(own && section.contains("not(any(target_os = \"macos\", target_os = \"android\"))"), "{}: dbus under {section}", manifest.display()),
                    "zbus" | "secret-service" | "dbus-secret-service" | "libdbus-sys" | "dbus-tokio" | "dbus-crossroads" | "zbus_names" | "ashpd" => panic!("{}: {package} under {section} (connections through sioul_sync::bus alone)", manifest.display()),
                    _ => {}
                }
            }
        }
        // The lock: who reaches libdbus's crates, and the other ways to a bus.
        let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
        let mut reaching: Vec<(String, String)> = Vec::new();
        for package in lock.split("[[package]]").skip(1) {
            let name = package.lines().find_map(|line| line.strip_prefix("name = \"")).and_then(|rest| rest.strip_suffix('"')).unwrap_or_default().to_string();
            for line in package.lines() {
                let dependency = line.trim().trim_end_matches(',').trim_matches('"');
                let dependency = dependency.split(' ').next().unwrap_or_default();
                if ["dbus", "libdbus-sys"].contains(&dependency) {
                    reaching.push((dependency.to_string(), name.clone()));
                }
            }
        }
        reaching.sort();
        let expected: Vec<(String, String)> = [("dbus", "dbus-secret-service"), ("dbus", "notify-rust"), ("dbus", "sioul-sync"), ("libdbus-sys", "dbus")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        assert_eq!(reaching, expected, "the crates that reach libdbus (the keyring's, notify-rust's for Windows and macOS, and this one)");
    }

    /// A setup stuck on a bus that hangs alive (its socket open, never
    /// answering) refuses the next ones; another bus started at the same
    /// path (the socket file made anew) is tried, and answers, while the old
    /// one still hangs (review of battery part C, L1). Starts `dbus-daemon`:
    /// run alone, by name:
    /// `cargo test --release -p sioul-sync bus::tests::a_bus_started_again_at_the_same_path_is_tried -- --ignored --exact`.
    #[test]
    #[ignore = "starts a stand-in dbus-daemon: run alone, by name"]
    fn a_bus_started_again_at_the_same_path_is_tried() {
        let mut stand_ins = StandIns::new("stand-in-again");
        let path = stand_ins.dir.join("bus");
        let address = format!("unix:path={}", escaped(&path));
        // The old bus: takes connections, never answers, stays alive.
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let held: std::sync::Arc<Mutex<Vec<std::os::unix::net::UnixStream>>> = std::sync::Arc::default();
        let keep = std::sync::Arc::clone(&held);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                keep.lock().unwrap().push(stream);
            }
        });
        let limit = Duration::from_secs(1);
        assert!(connect_within(&address, limit).is_err(), "the old bus never answers");
        assert!(connect_within(&address, limit).is_err(), "and, its setup stuck past its limit, refuses the next at once");
        // Another bus at the same path (its file made anew), the old one still hanging.
        std::fs::remove_file(&path).unwrap();
        let (_, again) = stand_ins.start("bus");
        assert_eq!(again, address);
        let start = Instant::now();
        let made = connect_within(&address, limit);
        assert!(made.is_ok(), "the bus started again is tried, and answers: {:?}", made.err());
        assert!(start.elapsed() < limit, "{:?}", start.elapsed());
        held.lock().unwrap().clear();
    }

    /// Two stand-in buses (`StandIns`), the session's stopped (SIGSTOP)
    /// before anything connected to it, as a bus that takes connections and
    /// never answers (review of battery part C, G1):
    /// - a connection to it comes back refused within its limit, its thread
    ///   left held there alone;
    /// - meanwhile, a connection to the other bus through libdbus's own
    ///   registration (its process-wide locks) is made and dropped at once;
    /// - past the limit, the next connection to the stopped bus is refused at
    ///   once, no thread more;
    /// - once it answers again, connections to it are made again.
    /// Starts `dbus-daemon`: run alone, by name:
    /// `cargo test --release -p sioul-sync bus::tests::a_bus_stuck_at_setup_holds_nothing_else -- --ignored --exact`.
    #[test]
    #[ignore = "starts two stand-in dbus-daemons: run alone, by name"]
    fn a_bus_stuck_at_setup_holds_nothing_else() {
        use dbus::blocking::Connection;
        let mut stand_ins = StandIns::new("stand-in-setup");
        let (stopped_pid, stopped) = stand_ins.start("stopped");
        let (_, other) = stand_ins.start("other");
        StandIns::signal(stopped_pid, "-STOP");
        let watched = |limit: Duration, work: Box<dyn FnOnce() + Send>| -> Option<Duration> {
            let (sender, came) = std::sync::mpsc::channel();
            let start = Instant::now();
            std::thread::spawn(move || {
                work();
                let _ = sender.send(start.elapsed());
            });
            came.recv_timeout(limit).ok()
        };
        // A connection to the stopped bus, on a thread, and libdbus's own to the other meanwhile.
        let limit = Duration::from_secs(2);
        let target = stopped.clone();
        let first = std::thread::spawn(move || {
            let start = Instant::now();
            let made = connect_within(&target, limit);
            (made.is_ok(), start.elapsed())
        });
        std::thread::sleep(Duration::from_millis(500));
        let elsewhere = other.clone();
        let registered = watched(Duration::from_secs(3), Box::new(move || drop(Connection::new_address(&elsewhere).unwrap())));
        let (made, took) = first.join().unwrap();
        assert!(registered.is_some(), "libdbus's own connection to the other bus waited behind the stopped one");
        assert!(!made && took < limit + Duration::from_secs(1), "refused within its limit: {made} {took:?}");
        // Past its limit, still under way: the next is refused at once.
        let start = Instant::now();
        assert!(connect_within(&stopped, limit).is_err());
        assert!(start.elapsed() < Duration::from_millis(200), "{:?}", start.elapsed());
        // The bus answers again: what was under way ends, and connections are made again.
        StandIns::signal(stopped_pid, "-CONT");
        let until = Instant::now() + Duration::from_secs(5);
        while UNDER_WAY.lock().unwrap().iter().any(|setup| setup.address == stopped) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
        let again = connect(&stopped).map(Connection::from).expect("made again");
        let (names,): (Vec<String>,) = again.with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", Duration::from_secs(2)).method_call("org.freedesktop.DBus", "ListNames", ()).unwrap();
        assert!(names.iter().any(|name| name.starts_with(':')), "{names:?}");
    }
}
