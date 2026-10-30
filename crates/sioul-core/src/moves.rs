// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! One follow machinery for what follows an appointment (composed tasks,
//! part E0): a paired task's date (`pairs`) and the legs around it
//! (`anchors`). Each follower keeps, in its own file, the instant of the
//! appointment it was last set for (the task's link: `X-SIOUL-AT`; a leg's
//! anchor: `WAS`), and this device keeps the instants it saw each appointment
//! at (`Seen`, its own state, never shared). One rule then says what to do
//! with a follower (`verdict`):
//!
//! - **consistent** with the appointment as it is: nothing, noted;
//! - set for the instant this device saw the appointment at, and the
//!   appointment is elsewhere now: it moved, seen here: **follow**;
//! - set for an instant this device never saw (another device's write came
//!   first, the appointment's copy not here yet): **wait**, writing nothing;
//!   still so once a pull of the appointment's calendar that began after the
//!   doubt was first seen has gone through whole (at once for a calendar that
//!   does not sync), the appointment's copy is the current one: follow. One
//!   round at most.
//!
//! A moved instant is noted only once each of its followers was written or
//! needed nothing: one that could not be read or written is tried again at
//! the next pass. An appointment is forgotten only once nothing here follows
//! it any more. Travel is not a move: instants are absolute, so a time zone
//! changed moves nothing.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The form of `Seen` written now: one before it (pairs-seen.toml, days only)
/// is not read, and every appointment is seen for the first time again.
pub const VERSION: u32 = 2;

/// An appointment's start and end, in Unix seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instants {
    pub start: i64,
    pub end: i64,
}

/// What this device saw of each appointment that something here follows,
/// by its UID (`$XDG_STATE_HOME/sioul/moves-seen.toml`, version 2): its
/// instants when last noted, and since when (Unix seconds) a follower has
/// been set for an instant it never saw, by that instant (`"<UID>@<instant>"`:
/// a follower written later for another instant has a doubt of its own,
/// never judged by a pull begun before it was written: review of ab7daa4,
/// later 6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub at: BTreeMap<String, Instants>,
    #[serde(default)]
    pub doubt: BTreeMap<String, i64>,
    /// When this device first set aside what an appointment over leaves open
    /// (`pairs::closing`, Unix seconds): written once, never moved back; an
    /// appointment that ended before it sets nothing aside (the past at the
    /// install is no preparation forgotten).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closing_from: Option<i64>,
}

impl Seen {
    pub fn path() -> PathBuf {
        crate::config::state_dir().join("moves-seen.toml")
    }

    /// As kept; empty (every appointment seen for the first time) when it
    /// does not read or is of another form.
    pub fn load() -> Seen {
        let read: Seen = std::fs::read_to_string(Seen::path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
        if read.version == VERSION { read } else { Seen { version: VERSION, ..Seen::default() } }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Seen::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let kept = Seen { version: VERSION, ..self.clone() };
        crate::vdir::write_item(&path, &toml::to_string(&kept).map_err(|e| e.to_string())?)
    }

    /// Only the appointments something here still follows (`named`).
    pub fn keep_only(&mut self, named: &BTreeSet<String>) {
        self.at.retain(|uid, _| named.contains(uid));
        self.doubt.retain(|key, _| named.contains(appointment_of(key)));
    }

    /// The appointment `uid` noted at `instants`, its doubts gone.
    pub fn note(&mut self, uid: &str, instants: Instants) {
        self.at.insert(uid.to_string(), instants);
        self.doubt.retain(|key, _| appointment_of(key) != uid);
    }

    /// Since when a follower of `uid` set for `instant` has been in doubt; `now` when it is not.
    pub fn since(&self, uid: &str, instant: i64, now: i64) -> i64 {
        self.doubt.get(&doubt_key(uid, instant)).copied().unwrap_or(now)
    }

    /// A follower of `uid` set for `instant`, an instant never seen here: since when, kept from the first time.
    pub fn doubted(&mut self, uid: &str, instant: i64, now: i64) -> i64 {
        *self.doubt.entry(doubt_key(uid, instant)).or_insert(now)
    }

    /// From when what closes by itself sets aside (`closing_from`): `now` at
    /// the first pass, kept after. None, and nothing kept, while the clock
    /// says a time before this build was made (`closing_floor`): such a clock
    /// is wrong, and taking its time would let the past at the install be set
    /// aside once it is set right; that pass closes nothing (review of
    /// 4d7bab4, later 7).
    pub fn closing_since(&mut self, now: i64) -> Option<i64> {
        if let Some(from) = self.closing_from {
            return Some(from);
        }
        if now < closing_floor() {
            return None;
        }
        self.closing_from = Some(now);
        Some(now)
    }
}

/// No clock right says a time before this build: its commit's time
/// (`build::committed`), and never before what closes by itself was written
/// (29 October 2026, 00:00 UTC), for a build that could not read its commit.
pub fn closing_floor() -> i64 {
    const WRITTEN: i64 = 1_793_232_000;
    crate::build::committed().max(WRITTEN)
}

/// A doubt's key: the appointment's UID and the instant its follower was set for.
fn doubt_key(uid: &str, instant: i64) -> String {
    format!("{uid}@{instant}")
}

/// The appointment a doubt's key names: the UID before its instant ("uid@1793232000"); an
/// older key, the UID alone (ab7daa4 wrote such), names it whole, an '@' in it or not
/// (review of 4d7bab4, later 15).
fn appointment_of(key: &str) -> &str {
    match key.rsplit_once('@') {
        Some((uid, instant)) if !instant.is_empty() && instant.bytes().all(|b| b.is_ascii_digit() || b == b'-') => uid,
        _ => key,
    }
}

/// What to do with one follower.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// As the appointment is: nothing to write.
    Consistent,
    /// The appointment moved: the follower follows it.
    Follow,
    /// Set for an instant this device has not seen: nothing written yet.
    Wait,
}

/// The pull of a calendar as this device last had it: whether it syncs at
/// all, and when (Unix seconds) the last pull that went through whole began.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pull {
    pub syncs: bool,
    pub began: Option<i64>,
}

impl Pull {
    /// The pull of the calendar holding `path` (`vdir::State::pulled`); a
    /// calendar of this device's own never syncs (`of_with` for the window's).
    pub fn of(path: &Path) -> Pull {
        Pull::of_with(path, Pull::default())
    }

    /// The pull of the calendar holding `path`; one of this device's own
    /// (`vdir::LOCAL`) reads as `local`: the window gives the sharing's last
    /// exchange that went through whole while the sharing carries such
    /// calendars (part "lists", `share::local_pull`), so that a leg whose file
    /// came before its appointment's waits for it rather than moving back
    /// (review of ab7daa4, later 14); else it follows at once.
    pub fn of_with(path: &Path, local: Pull) -> Pull {
        let Some(dir) = path.parent() else { return Pull::default() };
        let Some(calendar) = crate::vdir::collections(crate::vdir::Kind::Calendars).into_iter().find(|c| c.dir == dir) else { return Pull::default() };
        if calendar.account == crate::vdir::LOCAL {
            return local;
        }
        let state = crate::vdir::State::load(&calendar.state_path());
        Pull { syncs: !state.url.is_empty(), began: state.pulled }
    }
}

/// The one rule for a follower (the module's): `consistent` with the
/// appointment as it is; `saw_follower`, this device saw the appointment at
/// the instant the follower was set for; `since`, when this device first
/// found it set for another (`Seen::doubted`); `pull`, its calendar's.
pub fn verdict(consistent: bool, saw_follower: bool, since: i64, pull: Pull) -> Verdict {
    if consistent {
        Verdict::Consistent
    } else if saw_follower || !pull.syncs || pull.began.is_some_and(|began| began >= since) {
        Verdict::Follow
    } else {
        Verdict::Wait
    }
}

/// An instant as iCalendar writes it in UTC: "20261029T130000Z".
pub fn utc(at: i64) -> String {
    jiff::Timestamp::from_second(at).map(|t| t.strftime("%Y%m%dT%H%M%SZ").to_string()).unwrap_or_default()
}

/// An instant written in UTC ("20261029T130000Z"); None for anything else.
pub fn read_utc(value: &str) -> Option<i64> {
    let value = value.trim().trim_matches('"');
    let local = value.strip_suffix('Z').or_else(|| value.strip_suffix('z'))?;
    let at = jiff::civil::DateTime::strptime("%Y%m%dT%H%M%S", local).ok()?;
    at.to_zoned(jiff::tz::TimeZone::UTC).ok().map(|z| z.timestamp().as_second())
}

/// What follows each appointment, set right once (the module's rule): the
/// dates of the open tasks tied to it (`pairs::stand`, `pairs::followed`)
/// and its legs (`anchors::follow_legs`), for the appointments of `events`
/// (the pages' events: a copy left in an account switched off never moves
/// anything), each read from its file as it is now. Written only what
/// differs; `seen` noted for an appointment once each follower was written
/// or needed nothing, its doubt kept while one waits, and pruned to the
/// appointments something here still follows. The files written.
pub fn upkeep(tasks: &[crate::tasks::Task], events: &[crate::agenda::EventRef], seen: &mut Seen, now: &jiff::Zoned) -> Vec<PathBuf> {
    upkeep_with(tasks, events, seen, now, Pull::default())
}

/// `upkeep`, this device's own calendars read as `local` (`Pull::of_with`).
pub fn upkeep_with(tasks: &[crate::tasks::Task], events: &[crate::agenda::EventRef], seen: &mut Seen, now: &jiff::Zoned, local: Pull) -> Vec<PathBuf> {
    let zone = now.time_zone();
    let stamp = now.timestamp().as_second();
    let mut followers: BTreeMap<String, (Vec<&crate::tasks::Task>, Vec<&crate::agenda::EventRef>)> = BTreeMap::new();
    for task in tasks.iter().filter(|t| t.status.is_open()) {
        if let Some(uid) = crate::pairs::appointment_of(task) {
            followers.entry(uid).or_default().0.push(task);
        }
    }
    for leg in events {
        if let Some(anchor) = &leg.anchor {
            followers.entry(anchor.appointment.clone()).or_default().1.push(leg);
        }
    }
    seen.keep_only(&followers.keys().cloned().collect());
    let mut written = Vec::new();
    for (uid, (tied, legs)) in &followers {
        // The appointment itself, never one of its legs.
        let Some(reference) = events.iter().find(|e| e.uid == *uid && e.anchor.is_none()) else { continue };
        let path = Path::new(&reference.key);
        let Some(appointment) = crate::anchors::Followed::read(path, zone) else { continue };
        let pull = Pull::of_with(path, local);
        let at = seen.at.get(uid).copied();
        let (mut settled, mut waits) = (true, Vec::new());
        let day = jiff::Timestamp::from_second(appointment.instants.start).ok().map(|t| t.to_zoned(zone.clone()).date());
        let dated = crate::pairs::Appointment { day, cancelled: reference.cancelled, recurring: reference.recurring, start: appointment.instants.start };
        for task in tied {
            let Ok(text) = std::fs::read_to_string(&task.key) else {
                settled = false;
                continue;
            };
            let (consistent, saw, set_for) = crate::pairs::stand(task, &text, &dated, at, zone);
            match verdict(consistent, saw, seen.since(uid, set_for, stamp), pull) {
                Verdict::Consistent => {}
                Verdict::Wait => waits.push(set_for),
                Verdict::Follow => {
                    if let Some(followed) = crate::pairs::followed(task, &text, &dated, now) {
                        match crate::vdir::write_item(Path::new(&task.key), &followed) {
                            Ok(()) => written.push(PathBuf::from(&task.key)),
                            Err(_) => settled = false,
                        }
                    }
                }
            }
        }
        let (legs_written, legs_settled, legs_wait) = crate::anchors::follow_legs(legs, &appointment, at, &|instant| seen.since(uid, instant, stamp), pull, now);
        written.extend(legs_written);
        settled &= legs_settled;
        waits.extend(legs_wait);
        if !waits.is_empty() {
            for instant in waits {
                seen.doubted(uid, instant, stamp);
            }
        } else if settled {
            seen.note(uid, appointment.instants);
        }
    }
    written
}

#[cfg(test)]
mod tests {
    /// A doubt's key names its appointment: the UID before its instant; an older key, the UID
    /// alone, whole, an '@' in it or not (review of 4d7bab4, later 15).
    #[test]
    fn a_doubt_key_names_its_appointment() {
        assert_eq!(super::appointment_of(&super::doubt_key("visit@example.org", 1_793_232_000)), "visit@example.org");
        assert_eq!(super::appointment_of("visit@example.org"), "visit@example.org", "an older key, the UID alone");
        assert_eq!(super::appointment_of("visit"), "visit");
    }

    use super::*;

    #[test]
    fn one_rule_for_every_follower() {
        let synced = Pull { syncs: true, began: Some(100) };
        assert_eq!(verdict(true, false, 50, synced), Verdict::Consistent);
        assert_eq!(verdict(false, true, 500, Pull { syncs: true, began: None }), Verdict::Follow, "a move seen here");
        assert_eq!(verdict(false, false, 500, synced), Verdict::Wait, "the pull began before the doubt");
        assert_eq!(verdict(false, false, 50, synced), Verdict::Follow, "a pull begun after it went through: the copy here is current");
        assert_eq!(verdict(false, false, 500, Pull { syncs: false, began: None }), Verdict::Follow, "a calendar that does not sync: at once");
    }

    #[test]
    fn instants_in_utc_both_ways() {
        let at = jiff::Timestamp::from_second(1_793_278_800).unwrap().as_second();
        assert_eq!(read_utc(&utc(at)), Some(at));
        assert_eq!(read_utc("2026-10-29"), None);
    }

    /// A doubt is its follower's instant's: one written later for another
    /// instant starts its own (review of ab7daa4, later 6); noted, all go;
    /// the closing's start is kept across a save.
    #[test]
    fn each_follower_s_instant_has_its_own_doubt() {
        let mut seen = Seen::default();
        assert_eq!(seen.doubted("visit", 1000, 100), 100);
        assert_eq!((seen.since("visit", 1000, 500), seen.since("visit", 2000, 500)), (100, 500), "the later follower's own doubt");
        seen.doubted("visit", 2000, 500);
        seen.note("visit", Instants { start: 1, end: 2 });
        assert!(seen.doubt.is_empty());
        seen.doubt.insert("visit".into(), 7);
        seen.keep_only(&["visit".to_string()].into());
        assert!(seen.doubt.contains_key("visit"), "an older key, the UID alone, still the appointment's");
        seen.closing_from = Some(42);
        assert_eq!(toml::from_str::<Seen>(&toml::to_string(&seen).unwrap()).unwrap().closing_from, Some(42));
    }

    /// The closing's start is taken only from a clock at or after this
    /// build's time; before it, that pass closes nothing and keeps nothing
    /// (review of 4d7bab4, later 7). Then kept, whatever the clock says.
    #[test]
    fn a_clock_behind_the_build_starts_no_closing() {
        let floor = closing_floor();
        assert!(floor >= 1_793_232_000 && floor >= crate::build::committed());
        let mut seen = Seen::default();
        assert_eq!(seen.closing_since(floor - 400 * 86_400), None, "a year behind: nothing");
        assert_eq!(seen.closing_from, None, "and nothing kept");
        assert_eq!(seen.closing_since(floor + 60), Some(floor + 60));
        assert_eq!(seen.closing_since(floor + 3_600), Some(floor + 60), "kept");
        assert_eq!(seen.closing_since(floor - 86_400), Some(floor + 60), "kept, the clock set back since");
    }

    /// An older form (days only) is not read: first sight again, harmless.
    #[test]
    fn an_older_form_is_seen_afresh() {
        crate::config::stand_ins_only();
        let path = Seen::path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[days]\nvisit = 2026-10-29\n").unwrap();
        assert_eq!(Seen::load(), Seen { version: VERSION, ..Seen::default() });
        let mut seen = Seen::load();
        seen.note("visit", Instants { start: 1, end: 2 });
        seen.save().unwrap();
        assert_eq!(Seen::load().at["visit"], Instants { start: 1, end: 2 });
        let _ = std::fs::remove_file(&path);
    }
}
