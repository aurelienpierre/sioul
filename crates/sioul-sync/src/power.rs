// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What this computer says of itself, for work it may do while you are away
//! (the spam filter's training by itself: docs/spam-filter.md, "Training by
//! itself"): on mains power or on its battery, saving power or not, idle or
//! in use, its connection metered or not. Each fact is `None` when this
//! computer cannot tell it (no such service, no answer in time, a system not
//! read here): whoever decides counts an unknown fact as not holding.
//!
//! Also whether a person is at it now (`kept_session`, `present_of`): input
//! within ten minutes, its session not locked. A computer says so in its entry
//! among your devices, and while you sleep a phone's background steps slow
//! down when nobody is at any computer (docs/android.md, "In the background").
//!
//! All of it is read on one thread of its own (`watch`), every thirty
//! seconds while it is asked for, on one connection per bus kept for the
//! process (made again only once the bus dropped it), and kept: whoever asks
//! reads the kept values (`read`, `supply`, `kept_power`, `kept_session`) and
//! never opens a bus nor asks it anything. A bus that stops answering after
//! the thread connected only gives its questions up, a second each; one that
//! takes a new connection and never answers is given up after `bus::SETUP`
//! and holds one thread of its own, never this one nor libdbus's
//! process-wide locks (the connections are `crate::bus`'s).
//!
//! - **Linux and the BSDs**, over D-Bus: UPower's `OnBattery`;
//!   power-profiles-daemon's `ActiveProfile`, under both of its names
//!   (`org.freedesktop.UPower.PowerProfiles`, the older
//!   `net.hadess.PowerProfiles`); the session's idle time (the desktop's
//!   `org.freedesktop.ScreenSaver`, in milliseconds as KDE gives it, GNOME's
//!   `org.gnome.Mutter.IdleMonitor`) and whether it is locked (both screen
//!   savers, logind's `LockedHint`; logind's `IdleHint` only as a word for
//!   idle, since some desktops never set it); NetworkManager's `Metered`.
//! - **Windows**: `GetSystemPowerStatus` (its AC line, its battery saver),
//!   `GetLastInputInfo`, and whether the session is locked
//!   (`WTSQuerySessionInformationW`'s `WTSSessionInfoEx`). The connection's
//!   cost is not read: unknown.
//! - **macOS**: IOKit's power sources, Low Power Mode (as `pmset -g` says
//!   it), `CGEventSourceSecondsSinceLastEventType`, and whether the screen is
//!   locked (`CGSessionCopyCurrentDictionary`'s `CGSSessionScreenIsLocked`).
//!   The connection's cost is not read: unknown.
//! - **Android**: nothing; a phone never trains, and its own entry says it
//!   is in use while Sioul is on its screen.

use std::sync::{Condvar, Mutex, Once};
use std::time::{Duration, Instant};

/// What this computer says of itself now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Power {
    /// On mains power, not on its battery.
    pub on_mains: Option<bool>,
    /// Saving power: a power-saver profile, Windows' battery saver, Low Power Mode.
    pub power_saver: Option<bool>,
    /// Nobody's hands on it for `IDLE` at least, or its session locked.
    pub idle: Option<bool>,
    /// Its connection metered (NetworkManager: yes, or guessed yes).
    pub metered: Option<bool>,
}

impl Power {
    /// Its power supply allows long work: on mains power, and not saving
    /// power, both known.
    pub fn supplied(&self) -> bool {
        self.on_mains == Some(true) && self.power_saver == Some(false)
    }
}

/// How long without anyone's hands on the computer counts as idle.
pub const IDLE: Duration = Duration::from_secs(15 * 60);

/// How long one question to the system may take.
#[cfg_attr(not(all(unix, not(any(target_os = "macos", target_os = "android")))), allow(dead_code))]
const ASK: Duration = Duration::from_secs(1);

/// Every fact, as the reading thread read it (`watch`, started and asked):
/// its last reading while younger than `KEPT_FOR`, else the next one, waited
/// for `WAIT` at most (the first of this process, or the thread asked again
/// after a rest: `ASKED_FOR`); none by then, nothing said. Never opens a bus
/// nor asks it anything itself; for work that may wait on a thread of its
/// own (the spam filter's training), never for an exchange (`kept_power`).
pub fn read() -> Power {
    if !imp::READS {
        return Power::default();
    }
    watch();
    fresh_within(&SHARED, &BELL, WAIT).map(|reading| reading.power).unwrap_or_default()
}

/// Its power supply alone (mains, saving power), as `read` gives it: what a
/// training running asks every half minute.
pub fn supply() -> Power {
    let power = read();
    Power { on_mains: power.on_mains, power_saver: power.power_saver, ..Power::default() }
}

/// NetworkManager's `Metered` (`NMMetered`): yes or guessed yes, metered; no
/// or guessed no, not; unknown (0) or anything else, unknown.
pub fn metered_of(value: u32) -> Option<bool> {
    match value {
        1 | 3 => Some(true),
        2 | 4 => Some(false),
        _ => None,
    }
}

/// Idle, from what the session said: locked, idle; else the shortest time
/// without input any source gave (the newest touch wins); nothing said, unknown.
pub fn idle_of(locked: bool, shortest: Option<Duration>) -> Option<bool> {
    if locked {
        return Some(true);
    }
    shortest.map(|d| d >= IDLE)
}

/// What the session says of the person at this computer now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Session {
    /// How long since the last input (keyboard, pointer, touch): the shortest
    /// any source gave (the newest touch wins); none when no source said.
    pub since_input: Option<Duration>,
    /// Its session locked (a screen locker on): a source says so, and none
    /// says it is not.
    pub locked: bool,
    /// Its sources disagree on the lock (one says locked, another not: a
    /// stale hint of logind's, another session's, GNOME's shield up with no
    /// lock behind it): the lock is not taken, input alone says whether
    /// someone is at it (`present_of`).
    pub disputed: bool,
}

/// How long without input a person no longer counts as at this computer (`present_of`).
pub const PRESENT: Duration = Duration::from_secs(10 * 60);

/// Whether a person is at this computer now: its session locked, no; else a
/// press in Sioul (its window, do-not-disturb's switch) less than `PRESENT`
/// ago (`pressed`), yes, whatever the system says of input; else input less
/// than `PRESENT` ago, as the session says, also when its sources disagree on
/// the lock (`Session::disputed`: input alone says it); nothing said of
/// input, unknown. Unknown is never "nobody": Sioul running then counts as
/// someone at it, as before (`devices::others_in_use`).
pub fn present_of(session: Session, pressed: Option<Duration>) -> Option<bool> {
    if session.locked {
        return Some(false);
    }
    if pressed.is_some_and(|ago| ago < PRESENT) {
        return Some(true);
    }
    session.since_input.map(|since| since < PRESENT)
}

/// `present_of`, from the session as last read (`kept_session`: the reading
/// and how long ago it was made; none when there is none fresh enough) and
/// the last press in Sioul (how long ago): a press made since the reading
/// counts whatever the reading said, its lock included (someone unlocked the
/// computer to make it); an older one as `present_of` reads it. No reading, a
/// press within `PRESENT` says someone, and nothing else says anything
/// (unknown: Sioul running counts).
pub fn present_since(reading: Option<(Session, Duration)>, pressed: Option<Duration>) -> Option<bool> {
    let recent = pressed.filter(|ago| *ago < PRESENT);
    match reading {
        Some((_, read)) if recent.is_some_and(|ago| ago <= read) => Some(true),
        Some((session, _)) => present_of(session, pressed),
        None => recent.map(|_| true),
    }
}

/// How often the reading thread reads everything while it is asked for (`watch`).
pub const EVERY: Duration = Duration::from_secs(30);
/// How long `read` waits for a fresh reading: a few questions to the system,
/// a second each at most, and the connections' setup.
pub const WAIT: Duration = Duration::from_secs(10);
/// How long a reading is kept for (`kept_session`, `kept_power`, `read`):
/// past it, none (the thread held by a bus, or resting), and Sioul running
/// counts, as when nothing is said.
pub const KEPT_FOR: Duration = Duration::from_secs(120);
/// How long the reading thread goes on after it was last asked (`watch`,
/// `read`); past it, it rests until the next ask. The window asks at each
/// exchange, every minute; a process that asked once (`sioul watch`, whose
/// training asks only when nothing else holds it back) is not read every
/// half minute for days after.
pub const ASKED_FOR: Duration = Duration::from_secs(15 * 60);

/// One reading of the thread: when it began (a press in Sioul made while it
/// was read is newer than it: `present_since`), the session, the power
/// supply and the connection's cost.
#[derive(Debug, Clone, Copy)]
struct Reading {
    began: Instant,
    session: Session,
    power: Power,
}

/// What the reading thread and those who ask share: its last reading, and
/// when it was last asked for readings.
struct Shared {
    reading: Option<Reading>,
    asked: Option<Instant>,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared { reading: None, asked: None });
/// Rung at each reading kept and at each ask: whoever waits on it looks again.
static BELL: Condvar = Condvar::new();
/// The reading thread started (`start`).
static STARTED: Once = Once::new();

/// A reading by `read`, timed from when it began (`began`), not from when it
/// came back: a press made while a slow reading said locked is newer than it
/// (review of battery part C, G6).
fn reading_of(began: Instant, read: impl FnOnce() -> (Session, Power)) -> Reading {
    let (session, power) = read();
    Reading { began, session, power }
}

/// Starts, once in this process, the thread that reads everything on its own
/// connections to this process's buses (`imp::Reader`) every `EVERY` while
/// asked within `ASKED_FOR`, and keeps the last reading; and asks it.
/// Nothing else in the process asks the system for these: an exchange, a
/// start, Sioul closing and the training read what is kept.
pub fn watch() {
    start(imp::Reader::here);
    ask(&SHARED, &BELL, Instant::now());
}

/// `watch`, on the buses at these D-Bus addresses (none: that bus not read):
/// stand-ins, so that a test never reads the session or the system of
/// whoever runs it. Only the first start in a process counts; elsewhere than
/// on Linux and the BSDs, nothing is read over D-Bus and they are not used.
pub fn watch_on(session: Option<String>, system: Option<String>) {
    start(move || imp::Reader::at(session, system));
    ask(&SHARED, &BELL, Instant::now());
}

/// The reading thread, started once, its `imp::Reader` made on it by
/// `reader`: a reading, kept and rung; `EVERY` on, the next, once someone
/// asked within `ASKED_FOR` (`wanted`), else at the next ask. Nothing
/// started where nothing is read (a phone).
fn start(reader: impl FnOnce() -> imp::Reader + Send + 'static) {
    if !imp::READS {
        return;
    }
    STARTED.call_once(move || {
        let _ = std::thread::Builder::new().name("sioul-system".into()).spawn(move || {
            let mut reader = reader();
            loop {
                let reading = reading_of(Instant::now(), || reader.read());
                let Ok(mut shared) = SHARED.lock() else { return };
                shared.reading = Some(reading);
                BELL.notify_all();
                drop(shared);
                std::thread::sleep(EVERY);
                let Ok(shared) = SHARED.lock() else { return };
                if BELL.wait_while(shared, |shared| !wanted(shared.asked, Instant::now())).is_err() {
                    return;
                }
            }
        });
    });
}

/// Readings asked for at `now`: the thread resting wakes (`wanted`).
fn ask(shared: &Mutex<Shared>, bell: &Condvar, now: Instant) {
    if let Ok(mut shared) = shared.lock() {
        shared.asked = Some(now);
        bell.notify_all();
    }
}

/// Whether the reading thread goes on reading at `now`: asked within `ASKED_FOR`.
fn wanted(asked: Option<Instant>, now: Instant) -> bool {
    asked.is_some_and(|at| now.saturating_duration_since(at) < ASKED_FOR)
}

/// `reading` while it is younger than `KEPT_FOR` at `now`.
fn fresh(reading: Option<Reading>, now: Instant) -> Option<Reading> {
    reading.filter(|reading| now.saturating_duration_since(reading.began) < KEPT_FOR)
}

/// The reading in `shared` once fresh (`fresh`), waited for `limit` at most,
/// looked at again at each ring of `bell`; none after it.
fn fresh_within(shared: &Mutex<Shared>, bell: &Condvar, limit: Duration) -> Option<Reading> {
    let guard = shared.lock().ok()?;
    let (guard, _) = bell.wait_timeout_while(guard, limit, |shared| fresh(shared.reading, Instant::now()).is_none()).ok()?;
    fresh(guard.reading, Instant::now())
}

/// The session as last read (`watch`) and how long ago that reading began;
/// none before the first, or once it is `KEPT_FOR` old (a bus that stopped
/// answering, the thread resting). Never asks the system, never waits.
pub fn kept_session() -> Option<(Session, Duration)> {
    let now = Instant::now();
    let reading = fresh(SHARED.lock().ok()?.reading, now)?;
    Some((reading.session, now.saturating_duration_since(reading.began)))
}

/// The power supply and the connection's cost as last read (`watch`); none
/// before the first reading, or once it is `KEPT_FOR` old. Never asks the
/// system, never waits: for an exchange (the window's `metered`).
pub fn kept_power() -> Option<Power> {
    fresh(SHARED.lock().ok()?.reading, Instant::now()).map(|reading| reading.power)
}

/// Whether the reading thread kept a reading yet in this process, its first
/// waited for `limit` at most while there is none (the window's first
/// exchange, `metered`: a connection not read yet counts as metered).
pub fn read_once_within(limit: Duration) -> bool {
    ever_read_within(&SHARED, &BELL, limit)
}

/// `read_once_within`, on `shared` and its `bell`.
fn ever_read_within(shared: &Mutex<Shared>, bell: &Condvar, limit: Duration) -> bool {
    let Ok(guard) = shared.lock() else { return false };
    bell.wait_timeout_while(guard, limit, |shared| shared.reading.is_none()).is_ok_and(|(guard, _)| guard.reading.is_some())
}

/// What a Linux or BSD desktop's services answered of its session
/// (`imp::session`), each none when it did not answer: the desktop's screen
/// saver (KDE's, and others of the freedesktop name), GNOME's idle monitor
/// and screen shield, logind's hints.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(not(all(unix, not(any(target_os = "macos", target_os = "android")))), allow(dead_code))]
struct LinuxAnswers {
    /// `org.freedesktop.ScreenSaver.GetActive`.
    saver_active: Option<bool>,
    /// `org.freedesktop.ScreenSaver.GetSessionIdleTime`: milliseconds, as KDE
    /// gives them (a desktop giving seconds reads as never idle, never as idle too soon).
    saver_idle_ms: Option<u32>,
    /// `org.gnome.Mutter.IdleMonitor.GetIdletime`, milliseconds.
    mutter_idle_ms: Option<u64>,
    /// `org.gnome.ScreenSaver.GetActive`.
    gnome_active: Option<bool>,
    /// logind's `LockedHint`.
    locked_hint: Option<bool>,
    /// logind's `IdleSinceHint` (microseconds of the real clock), read only
    /// while its `IdleHint` holds: a word for idle only.
    idle_since_us: Option<u64>,
}

/// The session as a Linux or BSD desktop's answers say it, `now` the real
/// clock since the epoch: locked when a locker says so and none says
/// otherwise, disputed when they disagree (logind's hint left locked by a
/// locker that died, while the desktop's own screen saver says unlocked); the
/// shortest time since input any of them gave.
#[cfg_attr(not(all(unix, not(any(target_os = "macos", target_os = "android")))), allow(dead_code))]
fn linux_session(answers: &LinuxAnswers, now: Duration) -> Session {
    let lockers = [answers.saver_active, answers.gnome_active, answers.locked_hint];
    let (locked, unlocked) = (lockers.contains(&Some(true)), lockers.contains(&Some(false)));
    let times = [answers.saver_idle_ms.map(|ms| Duration::from_millis(u64::from(ms))), answers.mutter_idle_ms.map(Duration::from_millis), answers.idle_since_us.map(|us| now.saturating_sub(Duration::from_micros(us)))];
    Session { since_input: times.into_iter().flatten().min(), locked: locked && !unlocked, disputed: locked && unlocked }
}

/// The session as Windows' answers say it: the tick counts now and at the
/// last input (`GetTickCount`, `GetLastInputInfo`; none when it failed; both
/// wrap alike every 49.7 days), and `WTSSessionInfoEx`'s answer (none when
/// Windows gave none).
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_session(ticks: Option<(u32, u32)>, wts: Option<&[u8]>) -> Session {
    let since_input = ticks.map(|(now, input)| Duration::from_millis(u64::from(now.wrapping_sub(input))));
    Session { since_input, locked: wts.and_then(windows_flags).and_then(windows_locked) == Some(true), disputed: false }
}

/// Windows' word on a session's lock (`WTSINFOEX_LEVEL1_W::SessionFlags`):
/// `WTS_SESSIONSTATE_LOCK` (0), locked; `WTS_SESSIONSTATE_UNLOCK` (1), not;
/// anything else (`WTS_SESSIONSTATE_UNKNOWN`), unknown.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_locked(flags: i32) -> Option<bool> {
    match flags {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}

/// The flags of `WTSSessionInfoEx`'s answer, as Windows lays it out
/// (`WTSINFOEXW`): its level (a `DWORD`, 1), then, 8-aligned for the times
/// that follow, its session's id, state and flags (each 4 bytes): the flags
/// at byte 16. None for another level, or an answer too short to hold them.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_flags(answer: &[u8]) -> Option<i32> {
    let word = |at: usize| answer.get(at..at + 4).and_then(|b| <[u8; 4]>::try_from(b).ok());
    let level = u32::from_le_bytes(word(0)?);
    (level == 1).then(|| word(16).map(i32::from_le_bytes)).flatten()
}

/// The session as macOS says it: the seconds since its last input
/// (`CGEventSourceSecondsSinceLastEventType`; none from a value that is no
/// time), and whether its screen is locked.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn macos_session(seconds: f64, locked: bool) -> Session {
    let since_input = if seconds.is_finite() && seconds >= 0.0 { Duration::try_from_secs_f64(seconds).ok() } else { None };
    Session { since_input, locked, disputed: false }
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
mod imp {
    use super::{ASK, LinuxAnswers, Power, Session, idle_of, linux_session, metered_of};
    use dbus::blocking::Connection;
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
    use std::time::Duration;

    /// Something is read here.
    pub(super) const READS: bool = true;

    /// What the reading thread keeps between its readings: where each bus is
    /// (none: that bus not read), and the connection made to each, kept while
    /// the bus keeps it: made again only once the bus dropped it, never after
    /// a question given up (`held`).
    pub(super) struct Reader {
        session_at: Option<String>,
        system_at: Option<String>,
        session: Option<Connection>,
        system: Option<Connection>,
    }

    impl Reader {
        /// On the buses this process's environment names (`bus::session_address`, `bus::system_address`).
        pub(super) fn here() -> Reader {
            Reader::at(crate::bus::session_address(), Some(crate::bus::system_address()))
        }

        /// On the buses at these addresses; nothing connected yet.
        pub(super) fn at(session: Option<String>, system: Option<String>) -> Reader {
            Reader { session_at: session, system_at: system, session: None, system: None }
        }

        /// Everything, read now: the power supply and the connection's cost
        /// on the system's bus, the session on both (`session_of`), a few
        /// questions each given up after `ASK`.
        pub(super) fn read(&mut self) -> (Session, Power) {
            let system = held(&mut self.system, self.system_at.as_deref());
            let mut power = Power { on_mains: system.and_then(on_mains), power_saver: system.and_then(power_saver), metered: system.and_then(metered), idle: None };
            let bus = held(&mut self.session, self.session_at.as_deref());
            let session = session_of(bus, system);
            power.idle = idle_of(session.locked, session.since_input);
            (session, power)
        }
    }

    /// The connection kept in `slot` to the bus at `address`: what came on it
    /// read first (`drained`: late answers, and the bus gone), then the one
    /// kept while the bus keeps it, else a new one (`crate::bus::connect`,
    /// `bus::SETUP` at most). None without such a bus, or while it cannot be
    /// reached (tried again at the next reading).
    fn held<'a>(slot: &'a mut Option<Connection>, address: Option<&str>) -> Option<&'a Connection> {
        if let Some(connection) = slot.as_ref() {
            drained(connection);
        }
        if slot.as_ref().is_some_and(|connection| !connection.channel().is_connected()) {
            *slot = None;
        }
        if slot.is_none() {
            *slot = address.and_then(|address| crate::bus::connect(address).ok()).map(Connection::from);
        }
        slot.as_ref()
    }

    /// What came on a kept connection and nobody waits for (an answer come
    /// after its question was given up, the bus's own word on its name), read
    /// and dropped, a few at most at each reading: a kept connection never
    /// piles them up. A bus gone is learnt here (`held`).
    fn drained(connection: &Connection) {
        for _ in 0..32 {
            if !connection.process(Duration::ZERO).unwrap_or(false) {
                return;
            }
        }
    }

    fn on_mains(system: &Connection) -> Option<bool> {
        let on_battery: bool = system.with_proxy("org.freedesktop.UPower", "/org/freedesktop/UPower", ASK).get("org.freedesktop.UPower", "OnBattery").ok()?;
        Some(!on_battery)
    }

    /// power-profiles-daemon under its name, else under its older one.
    fn power_saver(system: &Connection) -> Option<bool> {
        [("org.freedesktop.UPower.PowerProfiles", "/org/freedesktop/UPower/PowerProfiles"), ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles")]
            .into_iter()
            .find_map(|(name, path)| system.with_proxy(name, path, ASK).get::<String>(name, "ActiveProfile").ok())
            .map(|profile| profile == "power-saver")
    }

    fn metered(system: &Connection) -> Option<bool> {
        let value: u32 = system.with_proxy("org.freedesktop.NetworkManager", "/org/freedesktop/NetworkManager", ASK).get("org.freedesktop.NetworkManager", "Metered").ok()?;
        metered_of(value)
    }

    /// The session's screen savers and idle monitors (on its own bus), then
    /// logind's word on it (on the system's), read as `linux_session` says.
    fn session_of(bus: Option<&Connection>, system: Option<&Connection>) -> Session {
        let mut answers = LinuxAnswers::default();
        if let Some(session) = bus {
            let saver = session.with_proxy("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver", ASK);
            let active: Result<(bool,), _> = saver.method_call("org.freedesktop.ScreenSaver", "GetActive", ());
            answers.saver_active = active.ok().map(|(on,)| on);
            let since: Result<(u32,), _> = saver.method_call("org.freedesktop.ScreenSaver", "GetSessionIdleTime", ());
            answers.saver_idle_ms = since.ok().map(|(ms,)| ms);
            let mutter: Result<(u64,), _> = session.with_proxy("org.gnome.Mutter.IdleMonitor", "/org/gnome/Mutter/IdleMonitor/Core", ASK).method_call("org.gnome.Mutter.IdleMonitor", "GetIdletime", ());
            answers.mutter_idle_ms = mutter.ok().map(|(ms,)| ms);
            let gnome: Result<(bool,), _> = session.with_proxy("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver", ASK).method_call("org.gnome.ScreenSaver", "GetActive", ());
            answers.gnome_active = gnome.ok().map(|(on,)| on);
        }
        if let Some(system) = system {
            let login = system.with_proxy("org.freedesktop.login1", "/org/freedesktop/login1/session/auto", ASK);
            answers.locked_hint = login.get::<bool>("org.freedesktop.login1.Session", "LockedHint").ok();
            if login.get::<bool>("org.freedesktop.login1.Session", "IdleHint").unwrap_or(false) {
                answers.idle_since_us = login.get::<u64>("org.freedesktop.login1.Session", "IdleSinceHint").ok();
            }
        }
        // logind's time is the real clock's: with the clock before 1970, not read.
        let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(now) => now,
            Err(_) => {
                answers.idle_since_us = None;
                Duration::ZERO
            }
        };
        linux_session(&answers, now)
    }
}

#[cfg(windows)]
mod imp {
    use super::{Power, Session, idle_of, windows_session};

    /// `SYSTEM_POWER_STATUS`.
    #[repr(C)]
    #[derive(Default)]
    struct PowerStatus {
        ac_line_status: u8,
        battery_flag: u8,
        battery_life_percent: u8,
        /// 1 while the battery saver is on.
        system_status_flag: u8,
        battery_life_time: u32,
        battery_full_life_time: u32,
    }

    /// `LASTINPUTINFO`.
    #[repr(C)]
    struct LastInput {
        size: u32,
        time: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetSystemPowerStatus(status: *mut PowerStatus) -> i32;
        fn GetTickCount() -> u32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetLastInputInfo(info: *mut LastInput) -> i32;
    }

    #[link(name = "wtsapi32")]
    unsafe extern "system" {
        fn WTSQuerySessionInformationW(server: *mut std::ffi::c_void, session: u32, class: i32, buffer: *mut *mut u16, bytes: *mut u32) -> i32;
        fn WTSFreeMemory(memory: *mut std::ffi::c_void);
    }

    /// `WTS_CURRENT_SESSION`: the session this program runs in.
    const CURRENT_SESSION: u32 = u32::MAX;
    /// `WTSSessionInfoEx`, among the classes of `WTS_INFO_CLASS`.
    const SESSION_INFO_EX: i32 = 25;

    /// Something is read here.
    pub(super) const READS: bool = true;

    /// Nothing kept between readings: Windows answers each question at once.
    pub(super) struct Reader;

    impl Reader {
        pub(super) fn here() -> Reader {
            Reader
        }

        /// No D-Bus here: the addresses are not used.
        pub(super) fn at(_session: Option<String>, _system: Option<String>) -> Reader {
            Reader
        }

        /// Everything, read now: the power supply, then the session.
        pub(super) fn read(&mut self) -> (Session, Power) {
            let session = session();
            let power = Power { idle: idle_of(session.locked, session.since_input), ..supply() };
            (session, power)
        }
    }

    /// The AC line and the battery saver (`GetSystemPowerStatus`).
    fn supply() -> Power {
        let mut status = PowerStatus::default();
        // SAFETY: it fills the structure it is given, laid out as Windows declares it.
        let read = unsafe { GetSystemPowerStatus(&mut status) } != 0;
        let on_mains = if read {
            match status.ac_line_status {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            }
        } else {
            None
        };
        let power_saver = read.then_some(status.system_status_flag == 1);
        Power { on_mains, power_saver, ..Power::default() }
    }

    /// The tick counts now and at the last input, and `WTSSessionInfoEx`'s
    /// answer, read as `windows_session` says.
    fn session() -> Session {
        let mut input = LastInput { size: std::mem::size_of::<LastInput>() as u32, time: 0 };
        // SAFETY: it fills the structure, its size set.
        let given = unsafe { GetLastInputInfo(&mut input) } != 0;
        // SAFETY: it reads a counter.
        let ticks = given.then(|| (unsafe { GetTickCount() }, input.time));
        let mut buffer: *mut u16 = std::ptr::null_mut();
        let mut bytes: u32 = 0;
        // SAFETY: the current server (null) and session; Windows allocates the answer and says its size.
        let asked = unsafe { WTSQuerySessionInformationW(std::ptr::null_mut(), CURRENT_SESSION, SESSION_INFO_EX, &mut buffer, &mut bytes) } != 0;
        if !asked || buffer.is_null() {
            return windows_session(ticks, None);
        }
        // SAFETY: Windows gave `bytes` bytes at `buffer`: read as bytes, within
        // them, before it is given back, once.
        unsafe {
            let session = windows_session(ticks, Some(std::slice::from_raw_parts(buffer.cast::<u8>(), bytes as usize)));
            WTSFreeMemory(buffer.cast());
            session
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{Power, Session, idle_of, macos_session};
    use std::ffi::{CStr, c_char, c_void};
    use std::time::{Duration, Instant};

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOPSCopyPowerSourcesInfo() -> *const c_void;
        fn IOPSGetProvidingPowerSourceType(snapshot: *const c_void) -> *const c_void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringGetCString(string: *const c_void, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
        fn CFRelease(object: *const c_void);
        fn CFStringCreateWithCString(allocator: *const c_void, text: *const c_char, encoding: u32) -> *const c_void;
        fn CFDictionaryGetValue(dictionary: *const c_void, key: *const c_void) -> *const c_void;
        fn CFGetTypeID(object: *const c_void) -> usize;
        fn CFBooleanGetTypeID() -> usize;
        fn CFBooleanGetValue(boolean: *const c_void) -> u8;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
        fn CGSessionCopyCurrentDictionary() -> *const c_void;
    }

    /// `kCFStringEncodingUTF8`.
    const UTF8: u32 = 0x0800_0100;

    /// Something is read here.
    pub(super) const READS: bool = true;

    /// How often Low Power Mode is asked (`pmset`, a program started each time).
    const LOW_POWER_EVERY: Duration = Duration::from_secs(5 * 60);

    /// What the reading thread keeps between its readings: Low Power Mode as
    /// `pmset` last said it, and when.
    pub(super) struct Reader {
        low_power: Option<(Instant, Option<bool>)>,
    }

    impl Reader {
        pub(super) fn here() -> Reader {
            Reader { low_power: None }
        }

        /// No D-Bus here: the addresses are not used.
        pub(super) fn at(_session: Option<String>, _system: Option<String>) -> Reader {
            Reader::here()
        }

        /// Everything, read now: the power source, Low Power Mode (asked
        /// again every `LOW_POWER_EVERY`), the session.
        pub(super) fn read(&mut self) -> (Session, Power) {
            let power_saver = match self.low_power {
                Some((at, said)) if at.elapsed() < LOW_POWER_EVERY => said,
                _ => {
                    let said = low_power_mode();
                    self.low_power = Some((Instant::now(), said));
                    said
                }
            };
            let session = session();
            let power = Power { on_mains: on_mains(), power_saver, idle: idle_of(session.locked, session.since_input), metered: None };
            (session, power)
        }
    }

    /// The time since the session's last input, and whether its screen is
    /// locked, read as `macos_session` says.
    fn session() -> Session {
        // The session's every input (`kCGEventSourceStateCombinedSessionState`, `kCGAnyInputEventType`).
        // SAFETY: it only reads the session's state.
        let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
        macos_session(seconds, screen_locked())
    }

    /// Whether the screen is locked, as the window server says it of this
    /// session (`CGSSessionScreenIsLocked`, there only while it is).
    fn screen_locked() -> bool {
        // SAFETY: the session's dictionary and the key are ours, each released
        // once; the value is borrowed from the dictionary and read before it is.
        unsafe {
            let session = CGSessionCopyCurrentDictionary();
            if session.is_null() {
                return false;
            }
            let key = CFStringCreateWithCString(std::ptr::null(), c"CGSSessionScreenIsLocked".as_ptr(), UTF8);
            let value = if key.is_null() { std::ptr::null() } else { CFDictionaryGetValue(session, key) };
            let locked = !value.is_null() && CFGetTypeID(value) == CFBooleanGetTypeID() && CFBooleanGetValue(value) != 0;
            if !key.is_null() {
                CFRelease(key);
            }
            CFRelease(session);
            locked
        }
    }

    /// What provides the power now, as IOKit names it: "AC Power", "Battery Power", "UPS Power".
    fn on_mains() -> Option<bool> {
        // SAFETY: the snapshot is ours, released once; the type it names is
        // borrowed from it and read before that, into a buffer of ours.
        unsafe {
            let snapshot = IOPSCopyPowerSourcesInfo();
            if snapshot.is_null() {
                return None;
            }
            let kind = IOPSGetProvidingPowerSourceType(snapshot);
            let mut buffer = [0 as c_char; 64];
            let read = !kind.is_null() && CFStringGetCString(kind, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) != 0;
            let name = read.then(|| CStr::from_ptr(buffer.as_ptr()).to_string_lossy().into_owned());
            CFRelease(snapshot);
            name.map(|n| n == "AC Power")
        }
    }

    /// Low Power Mode, as `pmset -g` says it ("lowpowermode 1"); a Mac without it says nothing of it: off.
    fn low_power_mode() -> Option<bool> {
        let out = std::process::Command::new("pmset").arg("-g").output().ok().filter(|o| o.status.success())?;
        let text = String::from_utf8_lossy(&out.stdout);
        Some(text.lines().any(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some("lowpowermode") && words.next() == Some("1")
        }))
    }
}

#[cfg(any(target_os = "android", not(any(unix, windows))))]
mod imp {
    /// Nothing is read here: no reading thread (`start`), `read` says nothing at once.
    pub(super) const READS: bool = false;

    pub(super) struct Reader;

    impl Reader {
        pub(super) fn here() -> Reader {
            Reader
        }

        pub(super) fn at(_session: Option<String>, _system: Option<String>) -> Reader {
            Reader
        }

        pub(super) fn read(&mut self) -> (super::Session, super::Power) {
            Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NetworkManager's five answers: a guess counts as what it guesses.
    #[test]
    fn metered_as_networkmanager_says() {
        assert_eq!([0, 1, 2, 3, 4, 9].map(metered_of), [None, Some(true), Some(false), Some(true), Some(false), None]);
    }

    /// Locked is idle; else the newest touch says it; nothing heard, unknown.
    #[test]
    fn idle_as_the_session_says() {
        let minutes = |m: u64| Some(Duration::from_secs(m * 60));
        assert_eq!(idle_of(true, None), Some(true));
        assert_eq!(idle_of(true, minutes(1)), Some(true));
        assert_eq!(idle_of(false, minutes(15)), Some(true));
        assert_eq!(idle_of(false, minutes(14)), Some(false));
        assert_eq!(idle_of(false, None), None);
    }

    /// Long work needs mains power and no power saving, both known.
    #[test]
    fn supplied_needs_both_known() {
        let p = |on_mains, power_saver| Power { on_mains, power_saver, ..Power::default() };
        assert!(p(Some(true), Some(false)).supplied());
        for (on_mains, saver) in [(Some(false), Some(false)), (Some(true), Some(true)), (None, Some(false)), (Some(true), None), (None, None)] {
            assert!(!p(on_mains, saver).supplied(), "{on_mains:?} {saver:?}");
        }
    }

    fn minutes(m: u64) -> Duration {
        Duration::from_secs(m * 60)
    }

    /// A person is at a computer while its session is unlocked and was
    /// touched within ten minutes; a press in Sioul counts at once, whatever
    /// the system says of input; locked, nobody is. A system that says nothing
    /// of input says nothing: Sioul running counts, as before.
    #[test]
    fn a_person_is_at_a_computer_as_its_session_says() {
        let session = |since: Option<u64>, locked: bool| Session { since_input: since.map(minutes), locked, disputed: false };
        assert_eq!(present_of(session(Some(0), false), None), Some(true));
        assert_eq!(present_of(session(Some(9), false), None), Some(true));
        assert_eq!(present_of(session(Some(10), false), None), Some(false), "ten minutes without input: nobody");
        assert_eq!(present_of(session(Some(120), false), None), Some(false));
        // A press in Sioul's window or of do-not-disturb's switch: at once, the system's idle time aside.
        assert_eq!(present_of(session(Some(120), false), Some(Duration::from_secs(5))), Some(true));
        assert_eq!(present_of(session(None, false), Some(minutes(9))), Some(true));
        assert_eq!(present_of(session(Some(120), false), Some(minutes(10))), Some(false), "a press ten minutes ago no longer counts");
        // Locked: nobody, a press or input a moment ago included.
        assert_eq!(present_of(session(Some(0), true), Some(Duration::ZERO)), Some(false));
        assert_eq!(present_of(session(None, true), None), Some(false));
        // Nothing said of input, no press: unknown (Sioul running counts, as before).
        assert_eq!(present_of(session(None, false), None), None);
        assert_eq!(present_of(session(None, false), Some(minutes(11))), None);
    }

    /// Linux and the BSDs, from stand-in answers of the session's services:
    /// a locker locks unless another says unlocked; the newest touch any of
    /// them gave wins; logind's idle time is the real clock less its hint;
    /// none answering, nothing said.
    #[test]
    fn a_linux_session_read_from_its_services() {
        let now = Duration::from_secs(1_800_000_000);
        let none = LinuxAnswers::default();
        assert_eq!(linux_session(&none, now), Session::default(), "no service answered: nothing said");
        // KDE: its screen saver alone.
        let kde = LinuxAnswers { saver_active: Some(false), saver_idle_ms: Some(30_000), ..none };
        assert_eq!(linux_session(&kde, now), Session { since_input: Some(Duration::from_secs(30)), locked: false, disputed: false });
        assert_eq!(present_of(linux_session(&kde, now), None), Some(true));
        assert_eq!(linux_session(&LinuxAnswers { saver_active: Some(true), ..kde }, now).locked, true, "KDE's locker on");
        // GNOME: Mutter's idle monitor and its shield; the freedesktop name not there.
        let gnome = LinuxAnswers { mutter_idle_ms: Some(11 * 60_000), gnome_active: Some(false), locked_hint: Some(false), ..none };
        assert_eq!(present_of(linux_session(&gnome, now), None), Some(false), "eleven minutes without input");
        assert!(linux_session(&LinuxAnswers { gnome_active: Some(true), locked_hint: Some(true), ..gnome }, now).locked, "GNOME locked: its shield and logind say so");
        assert!(linux_session(&LinuxAnswers { gnome_active: Some(true), ..gnome }, now).disputed, "its shield up, logind saying unlocked: disputed");
        // Several monitors: the newest touch wins.
        let both = LinuxAnswers { saver_idle_ms: Some(20 * 60_000), mutter_idle_ms: Some(60_000), ..none };
        assert_eq!(linux_session(&both, now).since_input, Some(minutes(1)));
        // logind alone (a desktop with neither): its hints; idle since a time of the real clock.
        let logind = LinuxAnswers { locked_hint: Some(false), idle_since_us: Some((now - minutes(25)).as_micros() as u64), ..none };
        assert_eq!(linux_session(&logind, now).since_input, Some(minutes(25)));
        assert!(linux_session(&LinuxAnswers { locked_hint: Some(true), ..none }, now).locked);
        assert_eq!(present_of(linux_session(&LinuxAnswers { locked_hint: Some(false), ..none }, now), None), None, "logind not idle, no monitor: unknown");
    }

    /// Windows, from stand-in answers: the tick counts (wrapping), and
    /// `WTSSessionInfoEx`'s answer laid out as Windows lays it.
    #[test]
    fn a_windows_session_read_from_its_answers() {
        let wts = |level: u32, flags: i32, length: usize| -> Vec<u8> {
            let mut answer = vec![0u8; 360];
            answer[0..4].copy_from_slice(&level.to_le_bytes());
            answer[8..12].copy_from_slice(&1u32.to_le_bytes());
            answer[12..16].copy_from_slice(&0u32.to_le_bytes());
            answer[16..20].copy_from_slice(&flags.to_le_bytes());
            answer.truncate(length);
            answer
        };
        let locked = wts(1, 0, 360);
        let unlocked = wts(1, 1, 360);
        assert_eq!(windows_flags(&locked), Some(0));
        assert_eq!(windows_session(Some((70_000, 10_000)), Some(&unlocked)), Session { since_input: Some(minutes(1)), locked: false, disputed: false });
        assert!(windows_session(Some((70_000, 10_000)), Some(&locked)).locked);
        assert_eq!(present_of(windows_session(Some((70_000, 10_000)), Some(&locked)), None), Some(false));
        // Unknown, another level, an answer too short, none at all: not said locked.
        for answer in [wts(1, -1, 360), wts(2, 0, 360), wts(1, 0, 18)] {
            assert!(!windows_session(None, Some(&answer)).locked, "{answer:?}");
        }
        assert_eq!(windows_session(None, None), Session::default(), "nothing answered: nothing said");
        // The tick count wrapped since the last input (49.7 days): still the time between.
        assert_eq!(windows_session(Some((5_000, u32::MAX - 54_999)), None).since_input, Some(minutes(1)));
    }

    /// macOS, from stand-in answers: the seconds since the last input (a value
    /// that is no time says nothing), and the screen's lock.
    #[test]
    fn a_macos_session_read_from_its_answers() {
        assert_eq!(macos_session(42.5, false), Session { since_input: Some(Duration::from_millis(42_500)), locked: false, disputed: false });
        assert_eq!(present_of(macos_session(42.5, false), None), Some(true));
        assert_eq!(present_of(macos_session(42.5, true), None), Some(false));
        assert_eq!(present_of(macos_session(900.0, false), None), Some(false));
        for odd in [f64::NAN, f64::INFINITY, -1.0, f64::MAX] {
            assert_eq!(macos_session(odd, false).since_input, None, "{odd}");
        }
    }

    /// Lock answers that disagree are not taken; input alone says whether
    /// someone is there (review of battery part C, F4 and G3): logind's hint
    /// left locked by a locker that died, the desktop's own screen saver
    /// saying unlocked and input five seconds ago: someone; GNOME's shield up
    /// without a lock behind it (its automatic lock off), two hours without
    /// input: nobody; no input said: unknown, Sioul running counts. A press in
    /// Sioul says someone.
    #[test]
    fn lock_answers_that_disagree_leave_input_to_say_it() {
        let now = Duration::from_secs(1_800_000_000);
        let stale = LinuxAnswers { saver_active: Some(false), saver_idle_ms: Some(5_000), locked_hint: Some(true), ..LinuxAnswers::default() };
        let session = linux_session(&stale, now);
        assert!(session.disputed && !session.locked);
        assert_eq!(present_of(session, None), Some(true), "input five seconds ago: someone, the stale lock aside");
        let shield = LinuxAnswers { mutter_idle_ms: Some(2 * 3_600_000), gnome_active: Some(true), locked_hint: Some(false), ..LinuxAnswers::default() };
        let night = linux_session(&shield, now);
        assert!(night.disputed);
        assert_eq!(present_of(night, None), Some(false), "GNOME's shield up, no lock, idle two hours: nobody");
        assert_eq!(present_of(Session { since_input: None, ..night }, None), None, "nothing said of input: unknown, Sioul running counts");
        assert_eq!(present_of(night, Some(Duration::from_secs(3))), Some(true), "a press says someone");
        // Agreeing lockers: locked; agreeing unlocked: by input, as before.
        let both_locked = LinuxAnswers { saver_active: Some(true), locked_hint: Some(true), ..LinuxAnswers::default() };
        assert_eq!(present_of(linux_session(&both_locked, now), None), Some(false));
        let both_unlocked = LinuxAnswers { saver_active: Some(false), saver_idle_ms: Some(5_000), locked_hint: Some(false), ..LinuxAnswers::default() };
        assert_eq!(present_of(linux_session(&both_unlocked, now), None), Some(true));
        // The spam training reads a disputed lock as unlocked, by input alone.
        assert_eq!(idle_of(session.locked, Some(minutes(20))), Some(true));
    }

    /// A press in Sioul made after the session was last read wins over what
    /// that reading said, its lock included: someone unlocked the computer
    /// to make it (review of battery part C, F4). A press older than the
    /// reading counts as `present_of` says; no reading, a recent press says
    /// someone and nothing else says anything.
    #[test]
    fn a_press_newer_than_the_reading_wins_over_its_lock() {
        let locked = Session { since_input: Some(minutes(40)), locked: true, disputed: false };
        let secs = Duration::from_secs;
        assert_eq!(present_since(Some((locked, secs(20))), Some(secs(2))), Some(true), "read locked 20 s ago, pressed 2 s ago");
        assert_eq!(present_since(Some((locked, secs(20))), Some(secs(30))), Some(false), "pressed before the reading that says locked");
        assert_eq!(present_since(Some((locked, secs(20))), None), Some(false));
        let away = Session { since_input: Some(minutes(40)), locked: false, disputed: false };
        assert_eq!(present_since(Some((away, secs(20))), Some(secs(30))), Some(true), "unlocked, pressed within ten minutes");
        assert_eq!(present_since(Some((away, secs(20))), Some(minutes(11))), Some(false));
        assert_eq!(present_since(None, Some(secs(30))), Some(true), "no reading: a press says someone");
        assert_eq!(present_since(None, Some(minutes(11))), None, "no reading: unknown");
        assert_eq!(present_since(None, None), None);
    }

    /// A reading is timed from when it began, not from when it came back: a
    /// press made while a slow reading said locked (someone unlocked the
    /// computer meanwhile, then pressed) is newer than it and wins (review of
    /// battery part C, G6).
    #[test]
    fn a_press_made_during_a_slow_reading_wins_over_its_lock() {
        let locked = Session { since_input: Some(minutes(40)), locked: true, disputed: false };
        let began = Instant::now();
        let mut pressed = None;
        let reading = reading_of(began, || {
            std::thread::sleep(Duration::from_millis(150));
            pressed = Some(Instant::now());
            std::thread::sleep(Duration::from_millis(150));
            (locked, Power::default())
        });
        assert_eq!(reading.began, began, "stamped when it began");
        let now = Instant::now();
        let (age, ago) = (now.duration_since(reading.began), now.duration_since(pressed.unwrap()));
        assert!(ago < age);
        assert_eq!(present_since(Some((reading.session, age)), Some(ago)), Some(true));
    }

    /// The kept reading: fresh, its own; once `KEPT_FOR` old, none (a bus
    /// that stopped answering, the thread resting), and Sioul running counts.
    #[test]
    fn the_kept_reading_ages_out() {
        let session = Session { since_input: Some(Duration::from_secs(4)), locked: false, disputed: false };
        let reading = Reading { began: Instant::now(), session, power: Power { metered: Some(true), ..Power::default() } };
        assert_eq!(fresh(Some(reading), reading.began + Duration::from_secs(1)).map(|r| (r.session, r.power.metered)), Some((session, Some(true))));
        assert!(fresh(Some(reading), reading.began + KEPT_FOR - Duration::from_millis(1)).is_some());
        assert!(fresh(Some(reading), reading.began + KEPT_FOR).is_none(), "too old: none");
        assert!(fresh(None, reading.began).is_none());
    }

    /// The thread reads while it is asked for, and rests `ASKED_FOR` after
    /// the last ask, until the next (a process that asked once is not read
    /// every half minute for days).
    #[test]
    fn the_thread_reads_while_asked_then_rests() {
        let asked = Instant::now();
        assert!(!wanted(None, asked), "never asked: rests");
        assert!(wanted(Some(asked), asked + Duration::from_secs(60)));
        assert!(wanted(Some(asked), asked + ASKED_FOR - Duration::from_millis(1)));
        assert!(!wanted(Some(asked), asked + ASKED_FOR), "asked fifteen minutes ago: rests");
        // An ask rings whoever waits, and says when.
        static SHARED_HERE: Mutex<Shared> = Mutex::new(Shared { reading: None, asked: None });
        static BELL_HERE: Condvar = Condvar::new();
        ask(&SHARED_HERE, &BELL_HERE, asked);
        assert_eq!(SHARED_HERE.lock().unwrap().asked, Some(asked));
    }

    /// `read` waits for a fresh reading `WAIT` at most: none, it comes back
    /// empty-handed at its limit; one rung meanwhile, at once.
    #[test]
    fn a_reading_is_waited_for_its_limit_at_most() {
        static SHARED_HERE: Mutex<Shared> = Mutex::new(Shared { reading: None, asked: None });
        static BELL_HERE: Condvar = Condvar::new();
        let start = Instant::now();
        assert!(fresh_within(&SHARED_HERE, &BELL_HERE, Duration::from_millis(200)).is_none());
        let took = start.elapsed();
        assert!(took >= Duration::from_millis(200) && took < Duration::from_secs(2), "{took:?}");
        let keeper = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            let session = Session { since_input: Some(Duration::from_secs(1)), locked: false, disputed: false };
            SHARED_HERE.lock().unwrap().reading = Some(Reading { began: Instant::now(), session, power: Power::default() });
            BELL_HERE.notify_all();
        });
        let start = Instant::now();
        let came = fresh_within(&SHARED_HERE, &BELL_HERE, Duration::from_secs(10));
        assert!(came.is_some_and(|reading| reading.session.since_input == Some(Duration::from_secs(1))));
        assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
        keeper.join().unwrap();
    }

    /// Whether a reading was ever kept: none, waited for its limit and said
    /// no; one rung meanwhile, yes at once; afterwards, yes without waiting,
    /// however old it is (the window's first exchange: review of battery part
    /// C, L3).
    #[test]
    fn the_first_reading_is_waited_for_once() {
        static SHARED_HERE: Mutex<Shared> = Mutex::new(Shared { reading: None, asked: None });
        static BELL_HERE: Condvar = Condvar::new();
        let start = Instant::now();
        assert!(!ever_read_within(&SHARED_HERE, &BELL_HERE, Duration::from_millis(150)));
        assert!(start.elapsed() >= Duration::from_millis(150));
        let keeper = std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            SHARED_HERE.lock().unwrap().reading = Some(Reading { began: Instant::now(), session: Session::default(), power: Power { metered: Some(true), ..Power::default() } });
            BELL_HERE.notify_all();
        });
        let start = Instant::now();
        assert!(ever_read_within(&SHARED_HERE, &BELL_HERE, Duration::from_secs(10)));
        assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
        keeper.join().unwrap();
        // Read once: at once from then on, the reading aged or not.
        SHARED_HERE.lock().unwrap().reading = Some(Reading { began: Instant::now() - KEPT_FOR.min(Duration::from_secs(1)), session: Session::default(), power: Power::default() });
        let start = Instant::now();
        assert!(ever_read_within(&SHARED_HERE, &BELL_HERE, Duration::from_secs(10)));
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    /// Nothing but the reading thread asks the system for these facts: the
    /// connections are made and the questions asked by `imp::Reader::read`
    /// alone, called on that thread only (`start`); what the others call
    /// reads what it kept (review of battery part C, G1).
    #[test]
    fn nothing_but_the_reading_thread_asks_the_system() {
        let source = include_str!("power.rs");
        let shared = source.split("#[cfg(all(unix, not(any(target_os = \"macos\", target_os = \"android\"))))]\nmod imp {").next().unwrap();
        assert_eq!(shared.matches("reader.read()").count(), 1, "read once, on the thread");
        let thread = shared.split("fn start(").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert!(thread.contains("spawn(move || {\n            let mut reader = reader();") && thread.contains("reader.read()"), "the reader made and read on the thread it starts");
        for public in ["pub fn read() -> Power {", "pub fn supply() -> Power {", "pub fn kept_session()", "pub fn kept_power()"] {
            let body = shared.split(public).nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
            assert!(!body.contains("Reader") && !body.contains("Connection"), "{public} reads what is kept");
        }
        let linux = source.split("mod imp {").nth(1).and_then(|rest| rest.split("#[cfg(windows)]").next()).unwrap();
        assert!(linux.contains("crate::bus::connect(") && !linux.contains("Channel::"), "its connections are crate::bus's (bus::tests::nothing_but_this_file_reaches_a_bus)");
    }

    /// Two stand-in buses (`bus::StandIns`: private `dbus-daemon`s, never the
    /// session or the system of whoever runs this) as the reading thread
    /// meets them (review of battery part C, G1):
    /// - both healthy: three readings, one connection to each bus, kept;
    /// - the session's stopped (SIGSTOP) once connected: a reading comes back,
    ///   its questions given up a second each, and keeps its connection;
    /// - the session's stand-in ends and another takes its place: the next
    ///   reading connects again.
    ///
    /// A bus stuck at setup holding nothing else is `bus`'s own test
    /// (`bus::tests::a_bus_stuck_at_setup_holds_nothing_else`). Starts
    /// `dbus-daemon`: run alone, by name:
    /// `cargo test --release -p sioul-sync power::tests::the_reading_keeps_its_connections_through_a_stopped_bus -- --ignored --exact`.
    #[test]
    #[ignore = "starts two stand-in dbus-daemons: run alone, by name"]
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    fn the_reading_keeps_its_connections_through_a_stopped_bus() {
        use crate::bus::StandIns;
        use dbus::blocking::Connection;
        // The unique names on a bus: one per connection, never given twice.
        let names = |look: &Connection| -> Vec<String> {
            let (all,): (Vec<String>,) = look.with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", Duration::from_secs(2)).method_call("org.freedesktop.DBus", "ListNames", ()).unwrap();
            let mut unique: Vec<String> = all.into_iter().filter(|name| name.starts_with(':')).collect();
            unique.sort();
            unique
        };
        let look = |address: &str| Connection::from(crate::bus::connect(address).unwrap());
        let mut stand_ins = StandIns::new("stand-in-buses");
        let (session_pid, session) = stand_ins.start("session");
        let (_, system) = stand_ins.start("system");

        // Both healthy: one connection to each, kept.
        let (look_session, look_system) = (look(&session), look(&system));
        let mut reader = imp::Reader::at(Some(session.clone()), Some(system.clone()));
        for _ in 0..3 {
            let start = Instant::now();
            let _ = reader.read();
            assert!(start.elapsed() < Duration::from_secs(2), "{:?}", start.elapsed());
        }
        let before = names(&look_session);
        assert_eq!(before.len(), 2, "the test's and the reader's: {before:?}");
        assert_eq!(names(&look_system).len(), 2);

        // The session's bus stops answering: a reading comes back, its questions given up.
        StandIns::signal(session_pid, "-STOP");
        let start = Instant::now();
        let _ = reader.read();
        let took = start.elapsed();
        StandIns::signal(session_pid, "-CONT");
        assert!(took < Duration::from_secs(8), "its questions given up a second each: {took:?}");
        // The reader kept its connection through the questions given up.
        let _ = reader.read();
        assert_eq!(names(&look_session), before, "the same connection kept");

        // The session's stand-in ends, another takes its place: connected again.
        StandIns::signal(session_pid, "-KILL");
        std::thread::sleep(Duration::from_millis(200));
        let (_, again) = stand_ins.start("session");
        assert_eq!(again, session);
        let look_again = look(&session);
        let _ = reader.read();
        assert_eq!(names(&look_again).len(), 2, "the test's and the reader's, made again");
    }
}
