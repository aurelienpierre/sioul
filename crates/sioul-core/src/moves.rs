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
/// been set for an instant it never saw.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seen {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub at: BTreeMap<String, Instants>,
    #[serde(default)]
    pub doubt: BTreeMap<String, i64>,
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
        self.doubt.retain(|uid, _| named.contains(uid));
    }

    /// The appointment `uid` noted at `instants`, its doubt gone.
    pub fn note(&mut self, uid: &str, instants: Instants) {
        self.at.insert(uid.to_string(), instants);
        self.doubt.remove(uid);
    }

    /// A follower of `uid` set for an instant never seen here: since when, kept from the first time.
    pub fn doubted(&mut self, uid: &str, now: i64) -> i64 {
        *self.doubt.entry(uid.to_string()).or_insert(now)
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
    /// The pull of the calendar holding `path` (`vdir::State::pulled`).
    pub fn of(path: &Path) -> Pull {
        let Some(dir) = path.parent() else { return Pull::default() };
        let Some(calendar) = crate::vdir::collections(crate::vdir::Kind::Calendars).into_iter().find(|c| c.dir == dir) else { return Pull::default() };
        let state = crate::vdir::State::load(&calendar.state_path());
        Pull { syncs: calendar.account != crate::vdir::LOCAL && !state.url.is_empty(), began: state.pulled }
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
        let pull = Pull::of(path);
        let at = seen.at.get(uid).copied();
        let since = seen.doubt.get(uid).copied().unwrap_or(stamp);
        let (mut settled, mut waits) = (true, false);
        let day = jiff::Timestamp::from_second(appointment.instants.start).ok().map(|t| t.to_zoned(zone.clone()).date());
        let dated = crate::pairs::Appointment { day, cancelled: reference.cancelled, recurring: reference.recurring, start: appointment.instants.start };
        for task in tied {
            let Ok(text) = std::fs::read_to_string(&task.key) else {
                settled = false;
                continue;
            };
            let (consistent, saw) = crate::pairs::stand(task, &text, &dated, at, zone);
            match verdict(consistent, saw, since, pull) {
                Verdict::Consistent => {}
                Verdict::Wait => waits = true,
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
        let (legs_written, legs_settled, legs_wait) = crate::anchors::follow_legs(legs, &appointment, at, since, pull, now);
        written.extend(legs_written);
        settled &= legs_settled;
        waits |= legs_wait;
        if waits {
            seen.doubted(uid, stamp);
        } else if settled {
            seen.note(uid, appointment.instants);
        }
    }
    written
}

#[cfg(test)]
mod tests {
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
