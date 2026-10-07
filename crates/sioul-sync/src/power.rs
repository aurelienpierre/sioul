// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What this computer says of itself, for work it may do while you are away
//! (the spam filter's training by itself: docs/spam-filter.md, "Training by
//! itself"): on mains power or on its battery, saving power or not, idle or
//! in use, its connection metered or not. Each fact is `None` when this
//! computer cannot tell it (no such service, no answer in time, a system not
//! read here): whoever decides counts an unknown fact as not holding.
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
//!   `GetLastInputInfo`. The connection's cost is not read: unknown.
//! - **macOS**: IOKit's power sources, Low Power Mode (as `pmset -g` says
//!   it), `CGEventSourceSecondsSinceLastEventType`. The connection's cost is
//!   not read: unknown.
//! - **Android**: nothing; a phone never trains.

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

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
mod imp {
    use super::{ASK, Power, idle_of, metered_of};
    use dbus::blocking::Connection;
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
    use std::time::Duration;

    pub(super) fn read(all: bool) -> Power {
        let Ok(system) = Connection::new_system() else { return Power::default() };
        let mut power = Power { on_mains: on_mains(&system), power_saver: power_saver(&system), ..Power::default() };
        if all {
            power.metered = metered(&system);
            power.idle = idle(&system);
        }
        power
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

    fn idle(system: &Connection) -> Option<bool> {
        let mut locked = false;
        let mut times: Vec<Duration> = Vec::new();
        if let Ok(session) = Connection::new_session() {
            let saver = session.with_proxy("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver", ASK);
            let active: Result<(bool,), _> = saver.method_call("org.freedesktop.ScreenSaver", "GetActive", ());
            locked |= active.is_ok_and(|(on,)| on);
            // Milliseconds, as KDE gives them: a desktop giving seconds reads as never idle, never as idle too soon.
            let since: Result<(u32,), _> = saver.method_call("org.freedesktop.ScreenSaver", "GetSessionIdleTime", ());
            times.extend(since.ok().map(|(ms,)| Duration::from_millis(u64::from(ms))));
            let mutter: Result<(u64,), _> = session.with_proxy("org.gnome.Mutter.IdleMonitor", "/org/gnome/Mutter/IdleMonitor/Core", ASK).method_call("org.gnome.Mutter.IdleMonitor", "GetIdletime", ());
            times.extend(mutter.ok().map(|(ms,)| Duration::from_millis(ms)));
            let gnome: Result<(bool,), _> = session.with_proxy("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver", ASK).method_call("org.gnome.ScreenSaver", "GetActive", ());
            locked |= gnome.is_ok_and(|(on,)| on);
        }
        let login = system.with_proxy("org.freedesktop.login1", "/org/freedesktop/login1/session/auto", ASK);
        locked |= login.get::<bool>("org.freedesktop.login1.Session", "LockedHint").unwrap_or(false);
        // Idle since a time (microseconds of the real clock): a word for idle only.
        if login.get::<bool>("org.freedesktop.login1.Session", "IdleHint").unwrap_or(false)
            && let Ok(since) = login.get::<u64>("org.freedesktop.login1.Session", "IdleSinceHint")
            && let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            times.push(now.saturating_sub(Duration::from_micros(since)));
        }
        idle_of(locked, times.into_iter().min())
    }
}

#[cfg(windows)]
mod imp {
    use super::{IDLE, Power};

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
            let mut input = LastInput { size: std::mem::size_of::<LastInput>() as u32, time: 0 };
            // SAFETY: it fills the structure, its size set; the tick count reads a counter.
            power.idle = (unsafe { GetLastInputInfo(&mut input) } != 0).then(|| {
                let now = unsafe { GetTickCount() };
                u128::from(now.wrapping_sub(input.time)) >= IDLE.as_millis()
            });
        }
        power
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{IDLE, Power};
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
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }

    /// `kCFStringEncodingUTF8`.
    const UTF8: u32 = 0x0800_0100;

    pub(super) fn read(all: bool) -> Power {
        let mut power = Power { on_mains: on_mains(), power_saver: low_power_mode(), ..Power::default() };
        if all {
            // The session's every input (`kCGEventSourceStateCombinedSessionState`, `kCGAnyInputEventType`).
            // SAFETY: it only reads the session's state.
            let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
            power.idle = (seconds.is_finite() && seconds >= 0.0).then(|| seconds >= IDLE.as_secs_f64());
        }
        power
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
}
