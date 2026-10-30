// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The time zone Sioul shows and enters times in, and decides "today" by
//! (Settings ▸ Agenda ▸ Time zone, `agenda.time_zone`, this device's own):
//! this device's zone, followed as it changes ("Follow this device", the
//! default), else one chosen by hand from the time zone database, for a
//! computer whose clock stays at home while you travel. Every view, the plan,
//! the reminders, the home card, the agents and the command line read it
//! here (`current`, `now`). Times are instants; only how they are said, and
//! which day is today, follow it.
//!
//! On the system's clock still (`Zoned::now()`, `TimeZone::system()`): the
//! dose path and the Health page (`health`, the window's and the core's),
//! their alarms and the zone said with them (the window's `alarms`), the
//! waking alarm (`wake`), an event's reminder that Android hands at a change
//! of zone unless one is chosen (`eventalarms`, `chosen_or`), the home
//! card's doses line, KDE Plasma's own "Until" (`dnd`), and a conflict
//! copy's file name (sioul-sync's `share`).
//!
//! This device's zone is found here from what names it: `TZ` when set, else
//! the link `/etc/localtime` (Linux, macOS), else the zone Android said last
//! (`agenda::recorded_zone`, written as it changes), else jiff's own
//! detection. jiff keeps the system's zone for five minutes and cannot be
//! asked again sooner; what names it is looked at again each second, so a
//! change while Sioul runs (a phone landing elsewhere, `timedatectl
//! set-timezone`) is seen within a second. On Windows, jiff's own detection
//! alone: within five minutes.

use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// What names this device's zone, as read now: a change of any is a change of zone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// `TZ`, when set.
    pub tz: Option<String>,
    /// Where `/etc/localtime` links to ("/usr/share/zoneinfo/Europe/Paris").
    pub localtime: Option<String>,
    /// The zone Android said last (`agenda::recorded_zone`), on a phone.
    pub recorded: Option<String>,
}

impl Sources {
    /// What names this device's zone now.
    pub fn read() -> Sources {
        let localtime = if cfg!(all(unix, not(target_os = "android"))) { std::fs::read_link("/etc/localtime").ok().map(|p| p.display().to_string()) } else { None };
        let recorded = if cfg!(target_os = "android") { crate::agenda::recorded_zone() } else { None };
        Sources { tz: std::env::var("TZ").ok(), localtime, recorded }
    }
}

/// This device's zone, as `sources` name it; jiff's own detection when they name none.
pub fn system_of(sources: &Sources) -> TimeZone {
    if let Some(tz) = sources.tz.as_deref() {
        // As Unix tools read it: empty is UTC; a name, else a POSIX rule ("DEMO-11").
        let name = tz.trim().trim_start_matches(':');
        if name.is_empty() {
            return TimeZone::UTC;
        }
        if let Ok(zone) = TimeZone::get(name).or_else(|_| TimeZone::posix(name)) {
            return zone;
        }
    }
    if let Some(name) = sources.localtime.as_deref().and_then(|link| link.split_once("zoneinfo/").map(|(_, name)| name))
        && let Ok(zone) = TimeZone::get(name)
    {
        return zone;
    }
    if let Some(zone) = sources.recorded.as_deref().and_then(|name| TimeZone::get(name).ok()) {
        return zone;
    }
    TimeZone::system()
}

/// The zone chosen by hand, by its name, when it reads; none for "Follow this device".
pub fn chosen_of(setting: Option<&str>) -> Option<TimeZone> {
    setting.map(str::trim).filter(|name| !name.is_empty()).and_then(|name| TimeZone::get(name).ok())
}

/// What `current` keeps: what it was found from, and when it last looked.
#[derive(Debug, Clone)]
pub struct Kept {
    sources: Sources,
    setting: Option<String>,
    config_changed: Option<SystemTime>,
    zone: TimeZone,
    looked: Instant,
}

impl Kept {
    /// The zone for these sources and this setting, found again only when one of them changed.
    pub fn resolve(kept: &mut Option<Kept>, sources: Sources, setting: Option<String>, config_changed: Option<SystemTime>, looked: Instant) -> TimeZone {
        if let Some(k) = kept.as_mut().filter(|k| k.sources == sources && k.setting == setting) {
            k.looked = looked;
            k.config_changed = config_changed;
            return k.zone.clone();
        }
        let zone = chosen_of(setting.as_deref()).unwrap_or_else(|| system_of(&sources));
        *kept = Some(Kept { sources, setting, config_changed, zone: zone.clone(), looked });
        zone
    }
}

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

/// How long what names the zone is trusted before it is looked at again.
const LOOK_AGAIN: Duration = Duration::from_secs(1);

/// The setting as the configuration says it, and when the file last changed.
fn setting_read(kept: Option<&Kept>) -> (Option<String>, Option<SystemTime>) {
    let path = crate::config::default_path();
    let changed = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    // The file unchanged since it was last read: the setting it held.
    if let Some(k) = kept.filter(|k| changed.is_some() && k.config_changed == changed) {
        return (k.setting.clone(), changed);
    }
    let setting = crate::config::Config::load(&path).ok().and_then(|c| c.agenda.time_zone).filter(|z| !z.trim().is_empty());
    (setting, changed)
}

/// The zone Sioul shows and enters times in, and decides today by.
pub fn current() -> TimeZone {
    let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = Instant::now();
    if let Some(k) = kept.as_ref().filter(|k| now.duration_since(k.looked) < LOOK_AGAIN) {
        return k.zone.clone();
    }
    let (setting, changed) = setting_read(kept.as_ref());
    Kept::resolve(&mut kept, Sources::read(), setting, changed, now)
}

/// The zone chosen by hand when one is; else `device`, the zone this device
/// just said (Android's own word at its change, before anything here names it).
pub fn chosen_or(device: TimeZone) -> TimeZone {
    let zone = current();
    let kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if kept.as_ref().is_some_and(|k| chosen_of(k.setting.as_deref()).is_some()) { zone } else { device }
}

/// Now, on the current zone's clock.
pub fn now() -> Zoned {
    Timestamp::now().to_zoned(current())
}

/// The setting changed here (Settings ▸ Agenda): read again at the next ask.
pub fn forget() {
    *KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// Whether the zone shown changed since this was last asked (the window's
/// minute, which then says every time again); never at the first ask.
pub fn turned() -> bool {
    static SAID: Mutex<Option<String>> = Mutex::new(None);
    let name = name_of(&current(), Timestamp::now());
    let mut said = SAID.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let changed = said.as_ref().is_some_and(|s| *s != name);
    *said = Some(name);
    changed
}

/// A zone's name as Sioul says it: "Europe/Paris"; one without a name, by its
/// offset at `at` ("UTC+01:00"); UTC, "UTC".
pub fn name_of(zone: &TimeZone, at: Timestamp) -> String {
    match zone.iana_name() {
        Some("UTC" | "Etc/UTC" | "Etc/GMT" | "Etc/Zulu" | "Zulu" | "GMT") => "UTC".to_string(),
        Some(name) => name.to_string(),
        None => {
            let seconds = zone.to_offset(at).seconds();
            if seconds == 0 {
                return "UTC".to_string();
            }
            let sign = if seconds < 0 { '-' } else { '+' };
            format!("UTC{sign}{:02}:{:02}", seconds.abs() / 3600, (seconds.abs() % 3600) / 60)
        }
    }
}

/// The zone's city, as a time beside another says it: "Paris" for
/// "Europe/Paris", "New York" for "America/New_York"; "UTC" for UTC.
pub fn place_of(name: &str) -> String {
    if name == "UTC" || name.starts_with("UTC+") || name.starts_with("UTC-") {
        return name.to_string();
    }
    name.rsplit('/').next().unwrap_or(name).replace('_', " ")
}

/// Every zone of the time zone database, by name, sorted (Settings, the event's clock).
pub fn all_names() -> Vec<String> {
    let mut names: Vec<String> = jiff::tz::db().available().map(|n| n.as_str().to_string()).filter(|n| n.contains('/') && !n.starts_with("Etc/") && !n.starts_with("posix/") && !n.starts_with("right/")).collect();
    names.sort();
    names.dedup();
    names
}

/// The zones whose name holds `query` (a city, a region, a part of either:
/// "new y", "paris", "utc"), those whose city starts with it first; at most `most`.
pub fn matching(query: &str, most: usize) -> Vec<String> {
    let simple = |text: &str| text.to_lowercase().replace('_', " ");
    let query = simple(query.trim());
    if query.is_empty() {
        return Vec::new();
    }
    let names: Vec<String> = std::iter::once("UTC".to_string()).chain(all_names()).filter(|n| simple(n).contains(&query)).collect();
    let (first, then): (Vec<String>, Vec<String>) = names.into_iter().partition(|n| simple(&place_of(n)).starts_with(&query));
    first.into_iter().chain(then).take(most).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    /// The zone follows what names it: `TZ` first, then the link, then what
    /// Android said; a change of any is seen at the next look, without a
    /// restart, and nothing is found again while they hold.
    #[test]
    fn a_change_of_the_system_s_zone_is_seen_while_running() {
        let paris = Sources { localtime: Some("/usr/share/zoneinfo/Europe/Paris".into()), ..Sources::default() };
        let new_york = Sources { localtime: Some("../usr/share/zoneinfo/America/New_York".into()), ..Sources::default() };
        let mut kept = None;
        let start = Instant::now();
        assert_eq!(Kept::resolve(&mut kept, paris.clone(), None, None, start).iana_name(), Some("Europe/Paris"));
        assert_eq!(Kept::resolve(&mut kept, new_york, None, None, start).iana_name(), Some("America/New_York"), "the link changed: seen at once");
        let tz = Sources { tz: Some("Asia/Tokyo".into()), ..paris };
        assert_eq!(Kept::resolve(&mut kept, tz, None, None, start).iana_name(), Some("Asia/Tokyo"), "TZ first");
        let phone = Sources { recorded: Some("Pacific/Auckland".into()), ..Sources::default() };
        assert_eq!(system_of(&phone).iana_name(), Some("Pacific/Auckland"), "what Android said");
        assert_eq!(system_of(&Sources { tz: Some("DEMO-11".into()), ..Sources::default() }).to_offset(at("2026-10-30T00:00:00Z")).seconds(), 11 * 3600, "a POSIX rule");
    }

    /// A trip: the setting changed, the instants unchanged, the times said
    /// shifted; a name that does not read follows this device.
    #[test]
    fn a_trip_shifts_the_times_said_never_the_instants() {
        let sources = Sources { localtime: Some("/usr/share/zoneinfo/Europe/Paris".into()), ..Sources::default() };
        let mut kept = None;
        let start = Instant::now();
        let meeting = at("2026-11-04T14:00:00Z");
        let home = Kept::resolve(&mut kept, sources.clone(), None, None, start);
        assert_eq!(meeting.to_zoned(home).strftime("%H:%M").to_string(), "15:00");
        let away = Kept::resolve(&mut kept, sources.clone(), Some("America/New_York".into()), None, start);
        assert_eq!(meeting.to_zoned(away.clone()).strftime("%H:%M").to_string(), "09:00", "the same instant, said in New York");
        assert_eq!(meeting.to_zoned(away).timestamp(), meeting);
        assert_eq!(Kept::resolve(&mut kept, sources, Some("Nowhere/Atlantis".into()), None, start).iana_name(), Some("Europe/Paris"), "a name that does not read: this device's");
    }

    #[test]
    fn a_zone_is_named_and_placed() {
        assert_eq!(name_of(&TimeZone::UTC, at("2026-10-30T00:00:00Z")), "UTC");
        assert_eq!(name_of(&TimeZone::fixed(jiff::tz::offset(-5)), at("2026-10-30T00:00:00Z")), "UTC-05:00");
        assert_eq!(place_of("America/New_York"), "New York");
        assert_eq!(place_of("UTC"), "UTC");
        assert!(all_names().iter().any(|n| n == "Europe/Paris") && !all_names().iter().any(|n| n.starts_with("Etc/")));
        assert_eq!(matching("new y", 3), ["America/New_York"]);
        assert_eq!(matching("utc", 1), ["UTC"]);
        assert_eq!(matching("PARIS", 5), ["Europe/Paris"]);
        assert!(matching("america/", 10).len() == 10 && matching("  ", 10).is_empty());
    }
}
