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
//! once, until the bus answers it.
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
    began: Instant,
    limit: Duration,
    number: u64,
}

/// Whether a new setup to `address` is refused at `now`: one under way there
/// has been so longer than its limit (the bus takes connections and does not
/// answer them).
fn refused(under_way: &[Setup], address: &str, now: Instant) -> bool {
    under_way.iter().any(|setup| setup.address == address && now.saturating_duration_since(setup.began) >= setup.limit)
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
    {
        let mut under_way = UNDER_WAY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if refused(&under_way, address, Instant::now()) {
            return Err(dbus::Error::new_failed("the bus did not answer a connection in time"));
        }
        under_way.push(Setup { address: address.to_string(), began: Instant::now(), limit, number });
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
    /// one thread held, never one more); within its limit, or to another
    /// bus, they go ahead.
    #[test]
    fn a_setup_stuck_past_its_limit_refuses_the_next_ones() {
        let began = Instant::now();
        let stuck = Setup { address: "unix:path=/run/a".into(), began, limit: SETUP, number: 1 };
        let under_way = [stuck];
        assert!(!refused(&under_way, "unix:path=/run/a", began + SETUP - Duration::from_millis(1)), "within its limit: another may go");
        assert!(refused(&under_way, "unix:path=/run/a", began + SETUP), "past it: refused");
        assert!(!refused(&under_way, "unix:path=/run/b", began + SETUP * 3), "another bus goes ahead");
        assert!(!refused(&[], "unix:path=/run/a", began));
    }

    /// The buses' addresses as libdbus finds them, a path written as an address writes it.
    #[test]
    fn addresses_are_written_as_d_bus_writes_them() {
        assert_eq!(escaped(std::path::Path::new("/run/user/1000/bus")), "/run/user/1000/bus");
        assert_eq!(escaped(std::path::Path::new("/tmp/a b,c;é")), "/tmp/a%20b%2cc%3b%c3%a9");
    }

    /// A bus that does not exist is refused at once, nothing held.
    #[test]
    fn a_bus_that_is_not_there_is_refused_at_once() {
        let start = Instant::now();
        assert!(connect("unix:path=/nonexistent/sioul-no-bus").is_err());
        assert!(start.elapsed() < Duration::from_secs(2), "{:?}", start.elapsed());
        assert!(UNDER_WAY.lock().unwrap().iter().all(|setup| setup.address != "unix:path=/nonexistent/sioul-no-bus"), "nothing left under way");
    }

    /// No file of Sioul's outside this one reaches a bus through libdbus's own
    /// registration (`dbus_bus_get*`, `dbus_bus_register`, the `dbus`
    /// crate's `new_session`, `new_system`, `new_address`, `get_private`,
    /// `register`) nor opens a connection itself; notify-rust, which does, is
    /// built for Windows and macOS alone; and Sioul's C++ uses no Qt D-Bus
    /// connection (Qt reaches the buses through the same registration).
    #[test]
    fn nothing_but_this_file_reaches_a_bus() {
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        let mut folders = vec![crates.clone()];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if !path.ends_with("target") {
                        folders.push(path);
                    }
                } else if path.extension().is_some_and(|e| e == "rs" || e == "cpp" || e == "h") {
                    files.push(path);
                }
            }
        }
        assert!(files.len() > 100, "{} files", files.len());
        let own = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bus.rs");
        // Written apart, so that this test does not find them in itself.
        let forbidden = [["new_", "session("], ["new_", "system("], ["new_", "address("], ["get_", "private("], [".regis", "ter()"], ["dbus_bus_", "get"], ["dbus_bus_", "register"], ["open_", "private("], ["QDBusConn", "ection"], ["notify_rust::", "get_capabilities"]];
        for file in &files {
            if file.ends_with("sioul-sync/src/bus.rs") || *file == own {
                continue;
            }
            let text = std::fs::read_to_string(file).unwrap_or_default();
            for [first, rest] in forbidden {
                let word = format!("{first}{rest}");
                assert!(!text.contains(&word), "{}: {word} (connections through sioul_sync::bus alone)", file.display());
            }
        }
        let manifest = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
        let notify = manifest.split("notify-rust.workspace = true").next().and_then(|before| before.lines().rev().find(|line| line.starts_with("[target."))).unwrap();
        assert_eq!(notify, "[target.'cfg(any(windows, target_os = \"macos\"))'.dependencies]", "notify-rust for Windows and macOS alone");
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
