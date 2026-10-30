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
//! the link `/etc/localtime` (Linux, macOS), else the zone Android said at
//! its change while that word is fresh (six minutes, as the doses trust it:
//! `Recorded::fresh`), else jiff's own detection, else the zone Android said
//! last however old. jiff keeps the system's zone for five minutes and cannot
//! be asked again sooner; what names it is looked at again each second, so a
//! change while Sioul runs (a phone landing elsewhere, `timedatectl
//! set-timezone`) is seen within a second. A record left from before a change
//! Sioul did not hear (stopped, or not started since an update) gives way to
//! the system's after its six minutes. On Windows, jiff's own detection
//! alone: within five minutes.

use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// The zone Android said last, and when: the dose path's record
/// (`$XDG_STATE_HOME/sioul/zone.toml`, the window's `alarms::ZoneSaid`),
/// read only here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recorded {
    /// "Asia/Tokyo".
    pub zone: String,
    /// When Android said it (Unix seconds); 0 for a record seeded from the
    /// system's zone, never said.
    pub said: i64,
}

impl Recorded {
    /// The record as the dose path keeps it; none without one, or without a zone in it.
    pub fn read() -> Option<Recorded> {
        let text = std::fs::read_to_string(crate::config::state_dir().join("zone.toml")).ok()?;
        let table: toml::Table = toml::from_str(&text).ok()?;
        let zone = table.get("zone").and_then(toml::Value::as_str).map(str::trim).filter(|zone| !zone.is_empty())?.to_string();
        let said = table.get("said").and_then(toml::Value::as_integer).unwrap_or(0);
        Some(Recorded { zone, said })
    }

    /// Whether it still names this device's zone at `now` (Unix seconds):
    /// said within the last six minutes, as the doses trust it
    /// (`alarms::now_in`; jiff keeps the system's zone five); a seeded
    /// record, never said, never.
    pub fn fresh(&self, now: i64) -> bool {
        self.said > 0 && (0..6 * 60).contains(&(now - self.said))
    }
}

/// What names this device's zone, as read now: a change of any is a change of zone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// `TZ`, when set.
    pub tz: Option<String>,
    /// Where `/etc/localtime` links to ("/usr/share/zoneinfo/Europe/Paris").
    pub localtime: Option<String>,
    /// The zone Android said last, and when (`Recorded`), on a phone.
    pub recorded: Option<Recorded>,
}

impl Sources {
    /// What names this device's zone now.
    pub fn read() -> Sources {
        let localtime = if cfg!(all(unix, not(target_os = "android"))) { std::fs::read_link("/etc/localtime").ok().map(|p| p.display().to_string()) } else { None };
        let recorded = if cfg!(target_os = "android") { Recorded::read() } else { None };
        Sources { tz: std::env::var("TZ").ok(), localtime, recorded }
    }
}

/// This device's zone, as `sources` name it at `now` (Unix seconds): `TZ`,
/// the link, the zone Android said while it is fresh (`Recorded::fresh`),
/// else what `detect` finds (jiff's own detection), else the zone Android
/// said last however old, else UTC.
pub fn system_of(sources: &Sources, now: i64, detect: &dyn Fn() -> Option<TimeZone>) -> TimeZone {
    found(sources, now, detect).0
}

/// `system_of`'s zone, and whether what names it was read here (`TZ`, the
/// link, a fresh record) rather than detected: a zone detected, or a record
/// gone stale, is found again at each look, never kept for good.
fn found(sources: &Sources, now: i64, detect: &dyn Fn() -> Option<TimeZone>) -> (TimeZone, bool) {
    if let Some(tz) = sources.tz.as_deref() {
        // As Unix tools read it: empty is UTC; a name, else a POSIX rule ("DEMO-11").
        let name = tz.trim().trim_start_matches(':');
        if name.is_empty() {
            return (TimeZone::UTC, true);
        }
        if let Ok(zone) = TimeZone::get(name).or_else(|_| TimeZone::posix(name)) {
            return (zone, true);
        }
    }
    if let Some(name) = sources.localtime.as_deref().and_then(|link| link.split_once("zoneinfo/").map(|(_, name)| name))
        && let Ok(zone) = TimeZone::get(name)
    {
        return (zone, true);
    }
    let recorded = sources.recorded.as_ref().and_then(|r| TimeZone::get(&r.zone).ok().map(|zone| (r.fresh(now), zone)));
    if let Some((true, zone)) = &recorded {
        return (zone.clone(), true);
    }
    if let Some(zone) = detect() {
        return (zone, false);
    }
    (recorded.map_or(TimeZone::UTC, |(_, zone)| zone), false)
}

/// jiff's own detection of the system's zone; none when it finds none.
fn detected() -> Option<TimeZone> {
    TimeZone::try_system().ok()
}

/// This device's zone now (Settings' "Follow this device").
pub fn device() -> TimeZone {
    system_of(&Sources::read(), Timestamp::now().as_second(), &detected)
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
    /// Named by what was read (`found`), the record fresh or not then.
    named: bool,
    fresh: bool,
}

impl Kept {
    /// The zone for these sources and this setting at `now` (Unix seconds),
    /// found again only when one of them changed, when the record turned
    /// stale, or when it was detected rather than named (`found`).
    pub fn resolve(kept: &mut Option<Kept>, sources: Sources, setting: Option<String>, config_changed: Option<SystemTime>, looked: Instant, now: i64, detect: &dyn Fn() -> Option<TimeZone>) -> TimeZone {
        let fresh = sources.recorded.as_ref().is_some_and(|r| r.fresh(now));
        if let Some(k) = kept.as_mut().filter(|k| k.sources == sources && k.setting == setting && k.named && k.fresh == fresh) {
            k.looked = looked;
            k.config_changed = config_changed;
            return k.zone.clone();
        }
        let (device, named) = found(&sources, now, detect);
        let zone = chosen_of(setting.as_deref()).unwrap_or(device);
        *kept = Some(Kept { sources, setting, config_changed, zone: zone.clone(), looked, named, fresh });
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
    Kept::resolve(&mut kept, Sources::read(), setting, changed, now, Timestamp::now().as_second(), &detected)
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

    /// No zone detected: what the sources name alone.
    fn none() -> Option<TimeZone> {
        None
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
        let now = at("2026-10-30T12:00:00Z").as_second();
        assert_eq!(Kept::resolve(&mut kept, paris.clone(), None, None, start, now, &none).iana_name(), Some("Europe/Paris"));
        assert_eq!(Kept::resolve(&mut kept, new_york, None, None, start, now, &none).iana_name(), Some("America/New_York"), "the link changed: seen at once");
        let tz = Sources { tz: Some("Asia/Tokyo".into()), ..paris };
        assert_eq!(Kept::resolve(&mut kept, tz, None, None, start, now, &none).iana_name(), Some("Asia/Tokyo"), "TZ first");
        let phone = Sources { recorded: Some(Recorded { zone: "Pacific/Auckland".into(), said: now - 60 }), ..Sources::default() };
        assert_eq!(system_of(&phone, now, &none).iana_name(), Some("Pacific/Auckland"), "what Android said");
        assert_eq!(system_of(&Sources { tz: Some("DEMO-11".into()), ..Sources::default() }, now, &none).to_offset(at("2026-10-30T00:00:00Z")).seconds(), 11 * 3600, "a POSIX rule");
    }

    /// On a phone, the zone Android said wins while it is fresh (six minutes,
    /// as the doses trust it); said seven minutes ago, or seeded (`said` 0),
    /// the system's as jiff detects it; that record however old only when
    /// nothing is detected; and a record kept while fresh gives way once stale.
    #[test]
    fn a_stale_record_gives_way_to_the_system_s_zone() {
        let now = at("2026-10-30T12:00:00Z").as_second();
        let phone = |said: i64| Sources { recorded: Some(Recorded { zone: "Asia/Tokyo".into(), said }), ..Sources::default() };
        let system = || TimeZone::get("Europe/Paris").ok();
        let name = |zone: TimeZone| zone.iana_name().unwrap_or_default().to_string();
        let said = |at: i64| name(system_of(&phone(at), now, &system));
        assert_eq!([said(now - 60), said(now - 7 * 60), said(0)], ["Asia/Tokyo", "Europe/Paris", "Europe/Paris"], "fresh: Android's word; said seven minutes ago, or seeded (never said): the system's");
        assert_eq!(name(system_of(&phone(now - 7 * 60), now, &none)), "Asia/Tokyo", "nothing detected: the record however old");
        let mut kept = None;
        let start = Instant::now();
        assert_eq!(name(Kept::resolve(&mut kept, phone(now - 5 * 60), None, None, start, now, &system)), "Asia/Tokyo");
        assert_eq!(name(Kept::resolve(&mut kept, phone(now - 5 * 60), None, None, start, now + 2 * 60, &system)), "Europe/Paris", "the same record, gone stale: the system's");
    }

    /// A trip: the setting changed, the instants unchanged, the times said
    /// shifted; a name that does not read follows this device.
    #[test]
    fn a_trip_shifts_the_times_said_never_the_instants() {
        let sources = Sources { localtime: Some("/usr/share/zoneinfo/Europe/Paris".into()), ..Sources::default() };
        let mut kept = None;
        let start = Instant::now();
        let meeting = at("2026-11-04T14:00:00Z");
        let now = meeting.as_second();
        let home = Kept::resolve(&mut kept, sources.clone(), None, None, start, now, &none);
        assert_eq!(meeting.to_zoned(home).strftime("%H:%M").to_string(), "15:00");
        let away = Kept::resolve(&mut kept, sources.clone(), Some("America/New_York".into()), None, start, now, &none);
        assert_eq!(meeting.to_zoned(away.clone()).strftime("%H:%M").to_string(), "09:00", "the same instant, said in New York");
        assert_eq!(meeting.to_zoned(away).timestamp(), meeting);
        assert_eq!(Kept::resolve(&mut kept, sources, Some("Nowhere/Atlantis".into()), None, start, now, &none).iana_name(), Some("Europe/Paris"), "a name that does not read: this device's");
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
