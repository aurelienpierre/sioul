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
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

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

/// This device's entry changed by `change`, its identity set: written here,
/// and into the folder when sharing is on. Never fails loudly: the doses of
/// the others say what they cannot know when it is not written.
fn write(change: impl FnOnce(&mut Entry)) {
    let Some((id, vault)) = crate::share::vault() else { return };
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
    if let Err(e) = written {
        eprintln!("sioul: devices: {e}");
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
    write(|entry| {
        if entry.working {
            recorded();
        }
        entry.start(now());
        entry.active = active;
    });
}

/// When a press was last made in Sioul on this computer: in its window, or
/// of do-not-disturb's switch (Unix seconds; 0, none in this process).
static PRESSED: AtomicI64 = AtomicI64::new(0);
/// This computer's entry last said nobody is at it (`Entry::active` false).
static AWAY: AtomicBool = AtomicBool::new(false);

/// A press in Sioul's window (cpp/application.cpp), or of do-not-disturb's
/// switch (`everywhere::press`), on this computer: a person is at it, whatever
/// its system says of input, for ten minutes, and whatever the session's
/// last reading said of its lock when the press is newer than it
/// (`sioul_sync::power::present_since`). Its entry said nobody was: an
/// exchange now says someone is, at once (`share::exchange_soon`).
pub(crate) fn pressed() {
    PRESSED.fetch_max(now(), Ordering::SeqCst);
    if AWAY.swap(false, Ordering::SeqCst) {
        crate::share::exchange_soon();
    }
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
/// press in Sioul. The session is read on a thread of its own every thirty
/// seconds (`sioul_sync::power::watch_session`): an exchange, a start or a
/// close only read the value kept, and never wait on the system's bus (a bus
/// that never answers once held the exchanges for good: review of battery
/// part C, F3). No reading yet, or one two minutes old: unsaid (Sioul running
/// counts), a recent press aside.
fn active_here() -> Option<bool> {
    if cfg!(target_os = "android") {
        return None;
    }
    sioul_sync::power::watch_session();
    active_of(false, sioul_sync::power::kept_session(), PRESSED.load(Ordering::SeqCst), now())
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
    AWAY.store(active == Some(false), Ordering::SeqCst);
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
        // `pressed` keeps the time, nothing else at hand (no window, no exchange asked).
        pressed();
        assert!(super::now() - PRESSED.load(Ordering::SeqCst) < 5);
        // Do-not-disturb's switch and the window's presses both say it (read from the sources).
        let source = |path: &str| std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
        let everywhere = source("src/everywhere.rs");
        let press_via = everywhere.split("fn press_via(").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert!(press_via.contains("crate::devices::pressed();"), "a press of the switch counts at once");
        let application = source("cpp/application.cpp");
        assert!(application.contains("installEventFilter(new Presses(application))") && application.contains("sioul_window_pressed();"), "a press in the window counts at once");
    }

    /// An exchange, a start or a close never wait on the system's bus: this
    /// computer's activity is read from the value the session's own thread
    /// keeps, at once, whatever the bus does (review of battery part C, F3:
    /// a bus that never answered once held the exchanges for good).
    #[test]
    fn the_activity_is_read_without_waiting_on_the_bus() {
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = active_here();
        }
        assert!(start.elapsed() < std::time::Duration::from_millis(200), "{:?}", start.elapsed());
        let source = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/devices.rs")).unwrap();
        let body = source.split("fn active_here() -> Option<bool> {").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert!(body.contains("kept_session()") && !body.contains("power::session()"), "the kept reading, never one of its own");
    }
}
