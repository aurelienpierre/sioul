// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! This device's sessions, as your other devices learn them through the
//! sharing folder (docs/database.md, "Devices"; `sioul_sync::devices`): its
//! start, its clean close, and between them `working`, up.
//!
//! - On a computer, a session is the window's life: up when Sioul starts,
//!   down as the last step of a clean close (the window closed, or the stop
//!   signal: cpp/signals.cpp quits Qt), after the final export (`share::closing`).
//! - On a phone, a session is the time on the screen: up when Sioul comes
//!   back, down when it goes to the background, after its export (Android may
//!   end a process in the background without a word, so it goes down then).
//! - A reminder handled while no session is open here (a dose's alarm,
//!   "Taken" pressed on its notification) is a session of its own: up, what
//!   it records, its export, down (`receiver`). Only when the last one ends,
//!   the window's or a receiver's, does it go down.
//! - Each exchange says what it read and wrote (`exported`), never touching
//!   the session: a phone's background service exporting in its own process
//!   never undoes the window's.
//!
//! - A computer also says whether a person is at it (`active`): input within
//!   ten minutes and its session unlocked, as its system says, or a press in
//!   Sioul's window or of do-not-disturb's switch, which counts at once. A
//!   phone's background steps slow down while nobody is at any computer
//!   (`others_in_use`); where the system says nothing of it, Sioul running
//!   counts, as before.
//!
//! A crash leaves it up: your other devices then say they cannot tell what
//! was marked here, until it starts again or you say it is off.

use sioul_sync::devices::{COMPUTER, Entry, PHONE};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};

/// The sessions open in this process: the window's, and receivers at work.
struct Sessions {
    window: bool,
    receivers: u32,
}

static SESSIONS: Mutex<Sessions> = Mutex::new(Sessions { window: false, receivers: 0 });

/// When this process last recorded something of the doses (a mark, a record,
/// a reminder), and when its last export read the files (milliseconds, this
/// device's clock). A session goes down only once an export read the files
/// after the last of them: else an answer captured here may not have gone
/// out, and saying "closed" would make the others sure of what they lack.
static RECORDED: AtomicI64 = AtomicI64::new(0);
static EXPORTED: AtomicI64 = AtomicI64::new(0);

/// Something of the doses recorded here (`health::change`, `change_records`):
/// owed to the others until an export reads it.
pub(crate) fn recorded() {
    RECORDED.fetch_max(jiff::Timestamp::now().as_millisecond(), Ordering::SeqCst);
}

/// Whether everything recorded here went out with an export since.
fn all_out() -> bool {
    went_out(RECORDED.load(Ordering::SeqCst), EXPORTED.load(Ordering::SeqCst))
}

/// Whether what was recorded (`RECORDED`) went out with an export since
/// (`EXPORTED`): both 0 in a process just started, which knows nothing of an
/// earlier one's.
fn went_out(recorded: i64, exported: i64) -> bool {
    recorded < exported || recorded == 0
}

fn sessions() -> std::sync::MutexGuard<'static, Sessions> {
    SESSIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// This device's name, to say where: a computer's host name; a phone's
/// model (its host name is "localhost"), said "the phone" by the others.
pub(crate) fn name() -> String {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        if cfg!(target_os = "android") {
            let model = std::process::Command::new("/system/bin/getprop").arg("ro.product.model").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            model.filter(|m| !m.is_empty()).unwrap_or_else(|| PHONE.to_string())
        } else {
            sioul_sync::lease::host_name()
        }
    })
    .clone()
}

/// A phone, or a computer.
pub(crate) fn kind() -> &'static str {
    if cfg!(target_os = "android") { PHONE } else { COMPUTER }
}

/// This process's opening, its entry's `started`, once written into the
/// folder (Unix seconds); 0 while not (sharing off, the key not read, the
/// folder not writable). Until every phone said it read it, this computer
/// is never a dose's first reminder (`health::first_here`).
static PUBLISHED: AtomicI64 = AtomicI64::new(0);

/// This process's opening as written into the folder; 0 while it is not.
pub(crate) fn published_start() -> i64 {
    PUBLISHED.load(Ordering::SeqCst)
}

/// This device's entry changed by `change`, its identity set: written here,
/// and into the folder when sharing is on. Never fails loudly: the doses of
/// the others say what they cannot know when it is not written. Whether it
/// went into the folder.
fn write(change: impl FnOnce(&mut Entry)) -> bool {
    let Some((id, vault)) = crate::share::vault() else { return false };
    let path = sioul_sync::devices::own_path(&sioul_core::config::state_dir());
    // Its doses said known only while their part travels: held under a newer
    // format (`share::holds`), they are not, whatever this process recorded.
    let doses = doses_said(crate::share::shares("health"), crate::share::holds_doses());
    // A phone's notification access, as Android says it now: what it may hold of other apps.
    let notifications = crate::appnotes::access_here();
    let written = sioul_sync::devices::change(&path, vault.as_ref().map(|(folder, key)| (folder.as_path(), key)), |entry| {
        entry.id = id;
        entry.name = name();
        entry.kind = kind().into();
        // The build that writes it (docs/database.md, "Devices"): which code each device runs.
        entry.version = sioul_core::build::VERSION.into();
        entry.commit = sioul_core::build::COMMIT.into();
        entry.format = sioul_sync::share::FORMAT;
        entry.doses = doses;
        if notifications.is_some() {
            entry.notifications = notifications;
        }
        change(entry);
    });
    match written {
        Ok(entry) if vault.is_some() => {
            // Its opening, as the folder now says it: up since then, or down.
            PUBLISHED.store(if entry.working { entry.started } else { 0 }, Ordering::SeqCst);
            true
        }
        Ok(_) => false,
        Err(e) => {
            eprintln!("sioul: devices: {e}");
            false
        }
    }
}

/// The window opened, or a phone's Sioul back on the screen: up. Found up
/// already (the last session ended without closing: a crash), what it left
/// is owed an export before the next close.
pub(crate) fn window_opened() {
    let mut open = sessions();
    open.window = true;
    raise();
}

/// Up, said; a session found up already owes an export. A computer says
/// too whether someone is at it (`active_here`).
fn raise() {
    let active = active_here();
    // Not written into the folder: no opening of this process said there yet.
    if !write(|entry| {
        if entry.working {
            recorded();
        }
        entry.start(now());
        entry.active = active;
    }) {
        PUBLISHED.store(0, Ordering::SeqCst);
    }
}

/// When a press was last made in Sioul on this computer: in its window, or
/// of do-not-disturb's switch (Unix seconds; 0, none in this process).
static PRESSED: AtomicI64 = AtomicI64::new(0);

/// A press in Sioul's window (cpp/application.cpp), or of do-not-disturb's
/// switch (`everywhere::press`), on this computer: a person is at it, whatever
/// its system says of input, for ten minutes, and whatever the session's
/// last reading said of its lock when the press is newer than it
/// (`sioul_sync::power::present_since`). Kept for the next export, at the
/// minute's cadence: never an exchange of its own, which would tell the
/// server when someone came back to this computer (review of battery part C, G2).
pub(crate) fn pressed() {
    PRESSED.fetch_max(now(), Ordering::SeqCst);
}

/// A press in Sioul's window, from its event filter (cpp/application.cpp),
/// a few seconds apart at most: `pressed`.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub extern "C" fn sioul_window_pressed() {
    let _ = std::panic::catch_unwind(pressed);
}

/// Whether a person is at this device now, for its entry (`Entry::active`):
/// on a phone, unsaid (in use only while Sioul is on its screen); on a
/// computer, as `active_of` reads the session's last reading and the last
/// press in Sioul. The system is read on a thread of its own every thirty
/// seconds (`watch_system`): an exchange, a start or a close only read the
/// value kept, and never wait on the system's bus (a bus that never answers
/// once held the exchanges for good: review of battery part C, F3 and G1).
/// No reading yet, or one two minutes old: unsaid (Sioul running counts), a
/// recent press aside.
fn active_here() -> Option<bool> {
    if cfg!(target_os = "android") {
        return None;
    }
    watch_system();
    active_of(false, sioul_sync::power::kept_session(), PRESSED.load(Ordering::SeqCst), now())
}

/// The system's reading thread started and asked for readings
/// (`sioul_sync::power::watch`): this computer's session, its power supply,
/// its connection's cost, read on connections of its own and kept; the one
/// way this program has them read. In this crate's tests, on a stand-in bus
/// of their own that never answers (`tests::stand_in_bus`): never the
/// session or the system of whoever runs them.
pub(crate) fn watch_system() {
    #[cfg(test)]
    tests::stand_in_bus();
    sioul_sync::power::watch();
}

/// Every fact of this computer's (`sioul_sync::power::read`, through
/// `watch_system`): may wait for a fresh reading ten seconds at most, so only
/// on a thread of its own (the spam filter's training by itself), never in
/// an exchange (`share::metered` reads what is kept). A computer's alone: a
/// phone never trains.
#[cfg(not(target_os = "android"))]
pub(crate) fn power_now() -> sioul_sync::power::Power {
    watch_system();
    sioul_sync::power::read()
}

/// `active_here`, from what was read: a phone; the session's last reading
/// and how long ago it was made (none: none fresh); when Sioul was last
/// pressed here (Unix seconds, 0 never); now.
fn active_of(phone: bool, reading: Option<(sioul_sync::power::Session, std::time::Duration)>, pressed: i64, now: i64) -> Option<bool> {
    if phone {
        return None;
    }
    let ago = (pressed > 0).then(|| std::time::Duration::from_secs(now.saturating_sub(pressed).max(0) as u64));
    sioul_sync::power::present_since(reading, ago)
}

/// Down, said, the last step of a session, once all it recorded went out;
/// else it stays up, and the others say what they cannot know until the
/// next session's export.
fn lower() -> bool {
    if !may_lower(all_out(), crate::share::holds_doses()) {
        eprintln!("sioul: devices: what was recorded here has not gone out; this device stays up");
        return false;
    }
    write(|entry| entry.close(now()));
    true
}

/// Whether this device's entry may say it shares its doses: it shares the
/// health part, and that part is not held here under a newer format (its
/// answers then wait unsent, and the others must not count them as known).
fn doses_said(shares: bool, held: bool) -> bool {
    shares && !held
}

/// Whether a session may go down (closed: everything it captured went out):
/// all it recorded went out, and the doses' part is not held here, which
/// this process's own counts cannot tell after a restart.
fn may_lower(all_out: bool, held: bool) -> bool {
    all_out && !held
}

/// The window's session ended, its final export done (`share::closing`):
/// down, unless a receiver is still at work, or `ended` says it is not over
/// after all (a phone's Sioul brought back meanwhile).
pub(crate) fn window_closed(ended: impl Fn() -> bool) {
    let mut open = sessions();
    if !ended() {
        return;
    }
    open.window = false;
    if open.receivers == 0 {
        lower();
    }
}

/// `work` (what a reminder records, then its export) as a session of its own
/// when none is open here: up first, so that a process ended in its midst
/// leaves the others in doubt rather than sure; down after, then the sync app
/// asked to carry it (`share::nudge_carriers`): the lowered entry goes up,
/// not the raised one.
pub(crate) fn receiver<T>(work: impl FnOnce() -> T) -> T {
    {
        let mut open = sessions();
        if !open.window && open.receivers == 0 {
            raise();
        }
        open.receivers += 1;
    }
    let out = work();
    let lowered = {
        let mut open = sessions();
        open.receivers = open.receivers.saturating_sub(1);
        !open.window && open.receivers == 0 && lower()
    };
    if lowered {
        crate::share::nudge_carriers();
    }
    out
}

/// Whether a reminder's own session is at work in this process (no window open):
/// the sync app is then asked once it is down, not by each exchange inside it.
pub(crate) fn in_receiver() -> bool {
    let open = sessions();
    !open.window && open.receivers > 0
}

/// An exchange done: what it read and wrote, said; the session as it is.
/// The first export of a session is carried at once (a phone's sync app
/// asked to look): the others see this device in use with fresh news, not
/// with what it last shared before it was put away.
pub(crate) fn exported(outcome: &sioul_sync::share::Outcome) {
    if outcome.looked <= 0 {
        return;
    }
    EXPORTED.fetch_max(outcome.looked, Ordering::SeqCst);
    // A computer says whether someone is at it now (sealed in its entry, written at
    // the same cadence either way): while you sleep, a phone steps slower when nobody is.
    let active = active_here();
    let first = std::sync::atomic::AtomicBool::new(false);
    write(|entry| {
        first.store(entry.working && entry.exported < entry.started, std::sync::atomic::Ordering::Relaxed);
        entry.exported(outcome.looked, outcome.wrote, now());
        entry.active = active;
    });
    if first.load(std::sync::atomic::Ordering::Relaxed) && !in_receiver() {
        crate::share::nudge_carriers();
    }
}

/// Sharing stopped here: down, and said to count no more.
pub(crate) fn left() {
    write(|entry| {
        entry.close(now());
        entry.left = true;
    });
}

/// This device's id and every device's entry, as the folder and the last
/// pull's copy hold them (`sioul_sync::devices::all`); none while sharing is off.
pub(crate) fn entries() -> Option<(String, Vec<Entry>)> {
    let Some((id, Some((folder, key)))) = crate::share::vault() else { return None };
    Some((id, sioul_sync::devices::all(&folder, &key).0))
}

/// Whether another device says it is in use, with an export dated lately
/// (`FRESH` and `SKEW`), and, while you sleep (`asleep`), a computer only with
/// someone at it (`sioul_sync::devices::others_in_use`): for the rhythm of a
/// phone's background service only; the doses' knowing is `health::know`'s.
pub(crate) fn others_in_use(asleep: bool) -> bool {
    entries().is_some_and(|(id, entries)| sioul_sync::devices::others_in_use(&entries, &id, now(), asleep))
}

/// A device's kind ("phone", "computer") and name, from its entry in the
/// folder; none when it has none (an older Sioul, or not arrived yet).
pub(crate) fn kind_and_name(id: &str) -> Option<(String, String)> {
    let (_, Some((folder, key))) = crate::share::vault()? else { return None };
    sioul_sync::devices::all(&folder, &key).0.into_iter().find(|e| e.id == id).map(|e| (e.kind, e.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A process started anew (an app restart; on Android, the background
    /// process killed and started again) knows nothing of what an earlier one
    /// recorded: its counts say all went out. While the doses' part is held
    /// under a newer format (`share::holds`, read from the disk), its entry
    /// still says its doses are not known, and its session never goes down as
    /// if they were: the others doubt, never take a dose as not taken
    /// (docs/health.md, "A newer Sioul's doses").
    #[test]
    fn a_held_doses_part_claims_no_knowledge_after_a_restart() {
        // The statics as a new process has them: nothing recorded, nothing exported.
        let (recorded, exported) = (0, 0);
        assert!(went_out(recorded, exported), "a new process: all went out, by its own counts");
        assert!(!may_lower(went_out(recorded, exported), true), "held: never down as if all went out");
        assert!(!doses_said(true, true), "held: its doses said not shared");
        assert!(may_lower(went_out(recorded, exported), false) && doses_said(true, false), "not held: as before");
        assert!(!may_lower(went_out(5, 3), false), "an answer recorded and not exported keeps it up, as before");
    }

    /// A press in Sioul (its window, do-not-disturb's switch) makes this
    /// computer active at once, whatever its system says of input, for ten
    /// minutes; its session locked, nobody; its system saying nothing,
    /// unsaid (Sioul running counts, as before); a phone says nothing of it.
    #[test]
    fn a_press_makes_this_computer_active_at_once() {
        use sioul_sync::power::Session;
        use std::time::Duration;
        let now = 1_800_000_000;
        // The session as read on its own thread, ten seconds ago.
        let read = |session: Session| Some((session, Duration::from_secs(10)));
        let idle = Session { since_input: Some(Duration::from_secs(2 * 3600)), locked: false, disputed: false };
        assert_eq!(active_of(false, read(idle), 0, now), Some(false), "two hours without input: nobody");
        assert_eq!(active_of(false, read(idle), now - 2, now), Some(true), "a press two seconds ago: someone, at once");
        assert_eq!(active_of(false, read(idle), now - 600, now), Some(false), "ten minutes on: the system's word again");
        assert_eq!(active_of(false, read(Session { since_input: Some(Duration::from_secs(30)), ..idle }), 0, now), Some(true));
        // Locked when read: nobody; a press made since the reading wins over its lock (review of
        // battery part C, F4: someone unlocked the computer to make it).
        let locked = Session { locked: true, ..idle };
        assert_eq!(active_of(false, read(locked), now - 60, now), Some(false), "pressed before the reading that says locked");
        assert_eq!(active_of(false, read(locked), now - 2, now), Some(true), "pressed after it: someone");
        // No reading kept (none yet, or it stopped coming): unsaid, Sioul running counts; a recent press still says someone.
        assert_eq!(active_of(false, None, 0, now), None, "nothing read: unsaid");
        assert_eq!(active_of(false, None, now - 30, now), Some(true));
        assert_eq!(active_of(false, read(Session::default()), 0, now), None, "the system says nothing: unsaid");
        assert_eq!(active_of(true, read(idle), now, now), None, "a phone says nothing of it");
        // `pressed` keeps the time, nothing else: no exchange of its own, which
        // would tell the server when someone came back (review of battery part C, G2).
        pressed();
        assert!(super::now() - PRESSED.load(Ordering::SeqCst) < 5);
        let source = |path: &str| std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        let devices = source("src/devices.rs");
        let press = devices.split("pub(crate) fn pressed() {").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert_eq!(press.trim(), "PRESSED.fetch_max(now(), Ordering::SeqCst);", "a press asks no exchange");
        assert!(!source("src/share.rs").contains("fn exchange_soon"), "nothing left to ask one");
        // Do-not-disturb's switch and the window's presses both say it (read from the sources).
        let everywhere = source("src/everywhere.rs");
        let press_via = everywhere.split("fn press_via(").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert!(press_via.contains("crate::devices::pressed();"), "a press of the switch counts at once");
        let application = source("cpp/application.cpp");
        assert!(application.contains("installEventFilter(new Presses(application))") && application.contains("sioul_window_pressed();"), "a press in the window counts at once");
    }

    /// A bus for this crate's tests that takes connections and never answers
    /// (a Linux abstract socket of their own, its listener holding each
    /// connection, nothing written to disk): the system's reading thread,
    /// started on it for both buses (`watch_system`), is held in its first
    /// connection, as by a wedged bus, and never reads the session or the
    /// system of whoever runs the tests. Elsewhere, no bus at all (Windows and
    /// macOS read no D-Bus).
    pub(super) fn stand_in_bus() {
        static MADE: std::sync::Once = std::sync::Once::new();
        MADE.call_once(|| {
            let address = silent_bus();
            sioul_sync::power::watch_on(address.clone(), address);
        });
    }

    /// `stand_in_bus`'s socket: its D-Bus address, none where there is none.
    fn silent_bus() -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            use std::os::linux::net::SocketAddrExt;
            let name = format!("sioul-app-tests-{}", std::process::id());
            let address = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).ok()?;
            let listener = std::os::unix::net::UnixListener::bind_addr(&address).ok()?;
            std::thread::spawn(move || {
                let mut held = Vec::new();
                for stream in listener.incoming().flatten() {
                    held.push(stream);
                }
            });
            Some(format!("unix:abstract={name}"))
        }
        #[cfg(not(target_os = "linux"))]
        None
    }

    /// An exchange, a start or a close never wait on the system's bus: this
    /// computer's activity is read from the value the system's own thread
    /// keeps, at once, whatever the bus does (review of battery part C, F3
    /// and G1: a bus that never answered once held the exchanges for good).
    /// Here that thread is held for good by a stand-in bus that never answers
    /// (`stand_in_bus`), as by a wedged one, and the activity comes at once,
    /// unsaid; never read from the real session or system of whoever runs it.
    #[test]
    fn the_activity_is_read_without_waiting_on_the_bus() {
        let _ = active_here();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = active_here();
        }
        assert!(start.elapsed() < std::time::Duration::from_millis(200), "{:?}", start.elapsed());
        assert!(sioul_sync::power::kept_session().is_none(), "the stand-in never answers: nothing read");
        let source = |path: &str| std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        let devices = source("src/devices.rs");
        let body = devices.split("fn active_here() -> Option<bool> {").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert!(body.contains("watch_system();") && body.contains("kept_session()"), "the kept reading, never one of its own");
        let watch = devices.split("pub(crate) fn watch_system() {").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert!(watch.contains("#[cfg(test)]\n    tests::stand_in_bus();\n    sioul_sync::power::watch();"), "tests start it on their stand-in first");
        // The system read through `watch_system` alone, and an exchange reads what is kept.
        for file in std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")).unwrap().flatten() {
            let path = file.path();
            if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("devices.rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for direct in ["power::read(", "power::supply(", "power::watch(", "power::watch_on("] {
                    assert!(!text.contains(direct), "{}: {direct} (through devices::watch_system)", path.display());
                }
            }
        }
        let share = source("src/share.rs");
        let metered = share.split("fn metered() -> bool {").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert!(metered.contains("kept_power()") && metered.contains("watch_system()"), "the cost of the connection as kept");
    }
}
