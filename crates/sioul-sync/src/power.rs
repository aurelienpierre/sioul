// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What this computer says of itself, for work it may do while you are away
//! (the spam filter's training by itself: docs/spam-filter.md, "Training by
//! itself"): on mains power or on its battery, saving power or not, idle or
//! in use, its connection metered or not. Each fact is `None` when this
//! computer cannot tell it (no such service, no answer in time, a system not
//! read here): whoever decides counts an unknown fact as not holding.
//!
//! Also whether a person is at it now (`session`, `present_of`): input within
//! ten minutes, its session not locked. A computer says so in its entry among
//! your devices, and a phone's background steps slow down while nobody is at
//! any computer (docs/android.md, "In the background").
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

use std::time::Duration;

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

/// Every fact, read now: a few questions to the system, a second each at most.
pub fn read() -> Power {
    imp::read(true)
}

/// Its power supply alone (mains, saving power): what a training running asks
/// again and again, without the session's questions.
pub fn supply() -> Power {
    imp::read(false)
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
    /// stale hint of logind's, another session's): what it says of the lock
    /// is not taken, and whether someone is at it stays unknown (`present_of`).
    pub disputed: bool,
}

/// The session's facts alone (`Session`): a few questions to the system, a
/// second each at most once connected. Nothing said on a system not read here
/// (a phone). The connection's own setup has no limit: a bus that takes it
/// and never answers holds the caller for good; `session_within` bounds it.
pub fn session() -> Session {
    imp::session()
}

/// How long without input a person no longer counts as at this computer (`present_of`).
pub const PRESENT: Duration = Duration::from_secs(10 * 60);

/// Whether a person is at this computer now: its sources disagreeing on the
/// lock, unknown (a press in Sioul less than `PRESENT` ago aside, which says
/// someone); its session locked, no; else a press in Sioul (its window,
/// do-not-disturb's switch) less than `PRESENT` ago (`pressed`), yes, whatever
/// the system says of input; else input less than `PRESENT` ago, as the
/// session says; nothing said of input, unknown. Unknown is never "nobody":
/// Sioul running then counts as someone at it, as before (`devices::others_in_use`).
pub fn present_of(session: Session, pressed: Option<Duration>) -> Option<bool> {
    let pressed = pressed.is_some_and(|ago| ago < PRESENT);
    if session.disputed {
        return pressed.then_some(true);
    }
    if session.locked {
        return Some(false);
    }
    if pressed {
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

/// How often the session is read on its own thread (`watch_session`).
pub const EVERY: Duration = Duration::from_secs(30);
/// How long a reading may take (`session_within`): a few questions to the
/// system, a second each at most, and the connections' setup.
pub const WAIT: Duration = Duration::from_secs(10);
/// How long a reading is kept for (`kept_session`): past it, none (a bus that
/// stopped answering), and Sioul running counts, as when nothing is said.
pub const KEPT_FOR: Duration = Duration::from_secs(120);

/// The session as last read on its own thread, and when.
static KEPT: std::sync::Mutex<Option<(std::time::Instant, Session)>> = std::sync::Mutex::new(None);
/// A reading of the session not come back yet (`session_within`).
static READING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Starts, once in this process, the thread that reads the session every
/// `EVERY`, each reading waited for `WAIT` at most (`session_within`), and
/// keeps the last (`kept_session`). Whoever needs it reads the kept value and
/// never waits on the system: an exchange, Sioul closing.
pub fn watch_session() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new().name("sioul-session".into()).spawn(|| {
            loop {
                if let Some(session) = session_within(WAIT)
                    && let Ok(mut kept) = KEPT.lock()
                {
                    *kept = Some((std::time::Instant::now(), session));
                }
                std::thread::sleep(EVERY);
            }
        });
    });
}

/// The session as last read on its own thread (`watch_session`) and how long
/// ago; none before the first reading, or once it is `KEPT_FOR` old (a bus
/// that stopped answering). Never waits on the system.
pub fn kept_session() -> Option<(Session, Duration)> {
    let kept = *KEPT.lock().ok()?;
    kept.map(|(at, session)| (session, at.elapsed())).filter(|(_, age)| *age < KEPT_FOR)
}

/// `session`, waited for `limit` at most: read on a thread of its own, so
/// that a bus that takes the connection and never answers (its setup has no
/// limit of its own) holds that thread alone, never the caller; none when it
/// does not come back in time, and none at once while an earlier reading has
/// not come back (one thread at most held by a stuck bus).
pub fn session_within(limit: Duration) -> Option<Session> {
    within(limit, &READING, session)
}

/// `work` on a thread of its own, waited for `limit` at most: its answer, or
/// none when it does not come in time, and none at once, nothing started,
/// while an earlier one under the same `held` flag has not come back.
fn within<T: Send + 'static>(limit: Duration, held: &'static std::sync::atomic::AtomicBool, work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    use std::sync::atomic::Ordering;
    if held.swap(true, Ordering::SeqCst) {
        return None;
    }
    let (sender, answer) = std::sync::mpsc::channel();
    let started = std::thread::Builder::new().name("sioul-session-read".into()).spawn(move || {
        let out = work();
        held.store(false, Ordering::SeqCst);
        let _ = sender.send(out);
    });
    if started.is_err() {
        held.store(false, Ordering::SeqCst);
        return None;
    }
    answer.recv_timeout(limit).ok()
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

    pub(super) fn read(all: bool) -> Power {
        let Ok(system) = Connection::new_system() else { return Power::default() };
        let mut power = Power { on_mains: on_mains(&system), power_saver: power_saver(&system), ..Power::default() };
        if all {
            power.metered = metered(&system);
            let session = session_on(Some(&system));
            power.idle = idle_of(session.locked, session.since_input);
        }
        power
    }

    pub(super) fn session() -> Session {
        let system = Connection::new_system().ok();
        session_on(system.as_ref())
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

    /// The session's screen savers and idle monitors, then logind's word on
    /// it (the system bus, when it answers), read as `linux_session` says.
    fn session_on(system: Option<&Connection>) -> Session {
        let mut answers = LinuxAnswers::default();
        if let Ok(session) = Connection::new_session() {
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
                std::time::Duration::ZERO
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

    pub(super) fn read(all: bool) -> Power {
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
        let mut power = Power { on_mains, power_saver, ..Power::default() };
        if all {
            let session = session();
            power.idle = idle_of(session.locked, session.since_input);
        }
        power
    }

    /// The tick counts now and at the last input, and `WTSSessionInfoEx`'s
    /// answer, read as `windows_session` says.
    pub(super) fn session() -> Session {
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

    pub(super) fn read(all: bool) -> Power {
        let mut power = Power { on_mains: on_mains(), power_saver: low_power_mode(), ..Power::default() };
        if all {
            let session = session();
            power.idle = idle_of(session.locked, session.since_input);
        }
        power
    }

    /// The time since the session's last input, and whether its screen is
    /// locked, read as `macos_session` says.
    pub(super) fn session() -> Session {
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
    pub(super) fn read(_all: bool) -> super::Power {
        super::Power::default()
    }

    pub(super) fn session() -> super::Session {
        super::Session::default()
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

    /// Lock answers that disagree (logind's hint left locked by a locker that
    /// died, or naming another session, while the desktop's own screen saver
    /// says unlocked, input five seconds ago) are not taken: unknown, Sioul
    /// running counts, never "nobody" (review of battery part C, F4). A press
    /// in Sioul still says someone.
    #[test]
    fn lock_answers_that_disagree_say_nothing_of_who_is_there() {
        let now = Duration::from_secs(1_800_000_000);
        let answers = LinuxAnswers { saver_active: Some(false), saver_idle_ms: Some(5_000), locked_hint: Some(true), ..LinuxAnswers::default() };
        let session = linux_session(&answers, now);
        assert!(session.disputed && !session.locked);
        assert_eq!(present_of(session, None), None, "unknown: Sioul running counts");
        assert_eq!(present_of(Session { since_input: Some(minutes(120)), ..session }, None), None, "never nobody, idle or not");
        assert_eq!(present_of(session, Some(Duration::from_secs(3))), Some(true), "a press says someone");
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

    /// A reading that does not come back (a bus that takes the connection and
    /// never answers: its setup has no limit of its own) never holds its
    /// caller past the limit; while it is held, the next asks nothing and
    /// answers none at once (one thread held at most); once it comes back,
    /// readings go on (review of battery part C, F3).
    #[test]
    fn a_reading_that_does_not_come_back_never_holds_its_caller() {
        static HELD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let start = std::time::Instant::now();
        let first = within(Duration::from_millis(300), &HELD, move || {
            let _ = wait.recv();
            7
        });
        assert_eq!(first, None);
        assert!(start.elapsed() < Duration::from_secs(2), "{:?}", start.elapsed());
        let start = std::time::Instant::now();
        assert_eq!(within(Duration::from_secs(5), &HELD, || 8), None, "an earlier one still held: nothing asked");
        assert!(start.elapsed() < Duration::from_millis(100));
        release.send(()).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while HELD.load(std::sync::atomic::Ordering::SeqCst) && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(within(Duration::from_secs(5), &HELD, || 9), Some(9), "come back: readings go on");
    }

    /// The kept reading: none before the first, then the last and its age,
    /// none again once it is `KEPT_FOR` old (a reading that stopped coming).
    #[test]
    fn the_kept_reading_ages_out() {
        let reading = Session { since_input: Some(Duration::from_secs(4)), locked: false, disputed: false };
        *KEPT.lock().unwrap() = Some((std::time::Instant::now(), reading));
        assert_eq!(kept_session().map(|(session, _)| session), Some(reading));
        if let Some(old) = std::time::Instant::now().checked_sub(KEPT_FOR + Duration::from_secs(1)) {
            *KEPT.lock().unwrap() = Some((old, reading));
            assert_eq!(kept_session(), None, "too old: none, Sioul running counts");
        }
        *KEPT.lock().unwrap() = None;
    }

    /// A session bus that takes the connection and never answers (a wedged
    /// broker): `session_within` comes back within its limit (the review of
    /// battery part C saw `session` blocked over nine minutes). Sets the
    /// process's bus addresses: run alone, by name:
    /// `cargo test --release -p sioul-sync power::tests::a_wedged_session_bus -- --ignored --test-threads=1`.
    #[test]
    #[ignore = "sets the process's bus addresses: run alone, by name"]
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    fn a_wedged_session_bus_never_holds_the_reading_past_its_limit() {
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("sioul-wedged-bus-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bus");
        let listener = UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming().flatten() {
                held.push(stream);
            }
        });
        // SAFETY: run alone, by name, before any other use of D-Bus in this process.
        unsafe {
            std::env::set_var("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", path.display()));
            std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", format!("unix:path={}", dir.join("none").display()));
        }
        let start = std::time::Instant::now();
        let said = session_within(Duration::from_secs(2));
        let took = start.elapsed();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(said, None);
        assert!(took < Duration::from_secs(3), "{took:?}");
    }
}
