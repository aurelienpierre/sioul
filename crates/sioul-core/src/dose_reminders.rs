// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What each device did to remind each dose (docs/health.md, "What became of
//! a reminder"): `$XDG_STATE_HOME/sioul/health-reminders.toml`, in the
//! sharing's health part.
//!
//! The rule it serves: a dose's reminder is plain only on the device that
//! knows it is the dose's first; every other reminder says the doubt first,
//! "Check first". A device knows it only from what the others wrote here, so
//! each writes, for each dose, before each notice of it goes (`maybe`), when
//! one was shown (`shown`), when its first try surely did not go out
//! (`unshown`), and what it promises (`quiet`: no notice of the dose from it
//! until then). Each device writes only its own entry, which travels whole
//! (`dose.<key>.<device>`): the sharing, where the later word wins entry by
//! entry, never has two devices fighting over one.
//!
//! A file of its own, as `health-doses.toml` is: an older Sioul (0.0.3 to
//! 0.0.5) does not know it, keeps its changes waiting and writes nothing in
//! it; a table of `health-state.toml` or a field of `health-doses.toml`
//! would be dropped by its next save, and the sharing would then take it out
//! on every device.

use crate::health::{Problem, Unsound};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The file's name, in Sioul's state folder.
pub const FILE: &str = "health-reminders.toml";

/// How long an entry is kept after its dose was due: the day's doses, and one more.
pub const KEPT_DAYS: i64 = 2;

/// A promise without end (`Tried::quiet`): no notice of the dose from this device again.
pub const NEVER: i64 = i64::MAX;

/// What each device did to remind each dose, by the dose's key, then the device's id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DoseReminders {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dose: BTreeMap<String, BTreeMap<String, Tried>>,
}

/// What one device did to remind one dose (Unix seconds, its own clock; 0: never).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tried {
    /// When its last notice of the dose was about to go: written before it
    /// went, so that no notice ever goes unrecorded. It may be on the screen
    /// since, until `shown` says it is.
    #[serde(default)]
    pub maybe: i64,
    /// When one was known shown: its notification server answered (or, on a
    /// phone, its alarm showed it). A reminder.
    #[serde(default)]
    pub shown: i64,
    /// When its first try surely did not go out: nothing was sent.
    #[serde(default)]
    pub unshown: i64,
    /// A promise: from the moment this was written until then, no notice of
    /// the dose goes from this device; `NEVER`, none again.
    #[serde(default)]
    pub quiet: i64,
}

impl DoseReminders {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join(FILE)
    }

    /// The entries as written, or why they cannot be trusted. Never empty in
    /// place of a file that does not read, nor of one lost after it was
    /// written here (`health::witnessed`): a device's word that it sent a
    /// notice would look never said.
    pub fn read(path: &Path) -> Result<DoseReminders, Unsound> {
        match std::fs::read_to_string(path) {
            Ok(text) if text.trim().is_empty() && crate::health::witnessed(path) => Err(Unsound::Lost),
            Ok(text) => toml::from_str(&text).map_err(|_| Unsound::Unreadable),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if crate::health::witnessed(path) {
                    Err(Unsound::Lost)
                } else {
                    Ok(DoseReminders::default())
                }
            }
            Err(_) => Err(Unsound::Unreadable),
        }
    }

    /// The entries changed by `change`, one writer at a time, read again just
    /// before, so that nothing written meanwhile is written over.
    pub fn update<T>(path: &Path, now: i64, change: impl FnOnce(&mut DoseReminders) -> T) -> Result<T, Problem> {
        crate::filelock::with_lock(path, || {
            let mut all = DoseReminders::read(path).map_err(Problem::Unsound)?;
            let out = change(&mut all);
            all.save(path, now).map_err(Problem::Write)?;
            Ok(out)
        })
    }

    /// Saved, entries past `KEPT_DAYS` dropped. Use `update`: it holds the lock.
    fn save(&mut self, path: &Path, now: i64) -> Result<(), String> {
        let oldest = now - KEPT_DAYS * 86_400;
        self.dose.retain(|key, _| due_of(key) >= oldest);
        let text = format!("# What each device did to remind each dose (docs/health.md, \"What became of a reminder\").\n{}", toml::to_string(self).map_err(|e| e.to_string())?);
        crate::health::write_witnessed(path, text.as_bytes())
    }

    /// `device`'s entry for the dose `key`, if it wrote one.
    pub fn of(&self, key: &str, device: &str) -> Option<Tried> {
        self.dose.get(key)?.get(device).copied()
    }

    /// `device`'s entry for the dose `key`, made when missing.
    pub fn entry(&mut self, key: &str, device: &str) -> &mut Tried {
        self.dose.entry(key.to_string()).or_default().entry(device.to_string()).or_default()
    }

    /// Whether a device's notice of the dose was shown, as far as read here.
    pub fn shown(&self, key: &str) -> bool {
        self.dose.get(key).is_some_and(|devices| devices.values().any(|t| t.shown > 0))
    }

    /// When the latest notice of the dose another device than `me` sent went,
    /// not known shown; none.
    pub fn maybe_elsewhere(&self, key: &str, me: &str) -> Option<i64> {
        self.dose.get(key)?.iter().filter(|(device, t)| device.as_str() != me && t.maybe > 0 && t.shown == 0).map(|(_, t)| t.maybe).max()
    }

    /// When a device's notice of the dose went without being known shown: a
    /// reminder that may not have shown (its question on the past says so).
    pub fn maybe_anywhere(&self, key: &str) -> Option<i64> {
        self.maybe_elsewhere(key, "")
    }
}

/// When a dose was due, from its key ("levo@1800000000"); 0 when it does not say.
fn due_of(key: &str) -> i64 {
    key.rsplit_once('@').and_then(|(_, due)| due.parse().ok()).unwrap_or(0)
}

/// Why a phone's alarm cannot know it is a dose's first reminder: a device
/// that may have reminded it before (`others`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Elsewhere {
    /// It sent a notice of it then, not known shown: it may be on the screen.
    Sent { device: crate::health::Named, at: i64 },
    /// Its first try did not go out then, and it may have tried again since.
    Unshown { device: crate::health::Named, at: i64 },
    /// In use around the dose's time, and no word of its own on this dose read here.
    Unknown { device: crate::health::Named },
    /// Another phone: its own alarm may remind it too.
    Alarm { device: crate::health::Named },
    /// The reminders' file does not read here: what the others did is not known.
    Unread,
}

/// What the other devices did to remind a dose, as a phone's alarm reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Others {
    /// One was shown somewhere: reminded.
    Reminded,
    /// Each other device that could have reminded it is known not to have,
    /// until `at`: this alarm is its first reminder, plainly.
    First,
    /// Not known: why, each device said, the doubt first.
    Doubt(Vec<Elsewhere>),
}

/// How far after `since` (the dose's time, or the waking it waited for) a
/// computer that did not keep the doses then sends no notice of it: past a
/// phone's alarm (`health::ALARM_AFTER`), its pull, and clocks apart. Its
/// promise says so (`Tried::quiet`); a computer closed then has it too.
pub const QUIET_AFTER: i64 = crate::health::ALARM_AFTER + 5 * 60;

/// What the devices other than `me` did to remind the dose `key`, to remind
/// from `since`, as a phone's alarm reads it at `at` (docs/health.md, "What
/// became of a reminder"). Of each other device that counts and shares its
/// doses (`health::counts`):
/// - its notice shown: reminded;
/// - a notice sent, not known shown: it may be on the screen;
/// - its promise covering `at` (no notice from it until then, its clock's
///   lead and `SKEW` allowed), nothing sent: it did not remind it;
/// - its first try not gone out, no promise covering `at`: it may have tried since;
/// - no word of its own on the dose: a computer closed cleanly, everything it
///   wrote read here, did not remind it, until `QUIET_AFTER` past `since`
///   (opened since, it promises that much before it may); a computer in use,
///   or an older Sioul, may have; another phone's own alarm may remind it.
///
/// `entries`: none when the file does not read here (nothing known).
/// `me_phone`: the device reading is a phone (a phone is then named, never
/// "the phone", when two count, it among them).
pub fn others(key: &str, since: i64, at: i64, me: &str, me_phone: bool, entries: Option<&DoseReminders>, peers: &[crate::health::Peer]) -> Others {
    use crate::health::{Named, SKEW, counts};
    let Some(entries) = entries else { return Others::Doubt(vec![Elsewhere::Unread]) };
    if entries.shown(key) {
        return Others::Reminded;
    }
    let counted: Vec<&crate::health::Peer> = peers.iter().filter(|p| p.id != me && counts(p, at) && p.said.as_ref().is_none_or(|s| s.doses)).collect();
    // Named for the words: "the phone" only when one phone counts.
    let phones = counted.iter().filter(|p| p.said.as_ref().is_some_and(|s| s.phone)).count() + usize::from(me_phone);
    let mut doubts = Vec::new();
    for peer in counted {
        let phone = peer.said.as_ref().is_some_and(|s| s.phone);
        let device = Named { id: peer.id.clone(), name: peer.name.clone(), phone: phone && phones == 1 };
        let margin = SKEW + peer.ahead.max(0);
        match entries.of(key, &peer.id) {
            Some(tried) if tried.maybe > 0 => doubts.push(Elsewhere::Sent { device, at: tried.maybe }),
            Some(tried) if tried.quiet == NEVER || tried.quiet.saturating_sub(margin) >= at => {}
            Some(tried) if tried.unshown > 0 => doubts.push(Elsewhere::Unshown { device, at: tried.unshown }),
            _ if phone => doubts.push(Elsewhere::Alarm { device }),
            _ => {
                let closed = peer.complete && peer.said.as_ref().is_some_and(|s| !s.working && s.closed >= s.started);
                if !(closed && at <= since + QUIET_AFTER - margin) {
                    doubts.push(Elsewhere::Unknown { device });
                }
            }
        }
    }
    if doubts.is_empty() { Others::First } else { Others::Doubt(doubts) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{Peer, Said};

    const DUE: i64 = 1_800_000_000;
    const KEY: &str = "levo@1800000000";

    fn computer(id: &str, exported: i64, working: bool) -> Peer {
        Peer {
            name: id.into(),
            id: id.into(),
            said: Some(Said { started: DUE - 3_600, closed: if working { 0 } else { DUE - 600 }, working, exported, imported: exported, doses: true, left: false, phone: false }),
            complete: true,
            seen: exported,
            heard: exported,
            ..Peer::default()
        }
    }

    fn phone(id: &str) -> Peer {
        Peer { said: Some(Said { phone: true, ..computer(id, DUE + 300, false).said.unwrap() }), ..computer(id, DUE + 300, false) }
    }

    fn with(device: &str, tried: Tried) -> DoseReminders {
        let mut all = DoseReminders::default();
        *all.entry(KEY, device) = tried;
        all
    }

    /// THE RULE at a phone's alarm, ten minutes after the dose, each case of
    /// the fifth review: the desk's word read, the doubt said unless it proves
    /// it did not remind; another phone always a doubt (its own alarm); the
    /// file unread, a doubt.
    #[test]
    fn a_phone_is_first_only_on_the_others_word() {
        let at = DUE + 600;
        let desk = computer("desk", DUE + 540, true);
        // Shown on the desk: reminded.
        assert_eq!(others(KEY, DUE, at, "me", true, Some(&with("desk", Tried { maybe: DUE + 1, shown: DUE + 2, ..Tried::default() })), std::slice::from_ref(&desk)), Others::Reminded);
        // Sent, not known shown (a server answering late, a bus stalled under it): may be on the screen.
        assert!(matches!(others(KEY, DUE, at, "me", true, Some(&with("desk", Tried { maybe: DUE + 1, ..Tried::default() })), std::slice::from_ref(&desk)), Others::Doubt(d) if matches!(d[..], [Elsewhere::Sent { at, .. }] if at == DUE + 1)));
        // Not gone out, then quiet past the alarm, promised: first.
        let quiet = Tried { unshown: DUE + 5, quiet: DUE + QUIET_AFTER, ..Tried::default() };
        assert_eq!(others(KEY, DUE, at, "me", true, Some(&with("desk", quiet)), std::slice::from_ref(&desk)), Others::First);
        // The same, the alarm late (a phone off, turned on at +20 min): past the promise, a doubt.
        assert!(matches!(others(KEY, DUE, DUE + 1_200, "me", true, Some(&with("desk", quiet)), std::slice::from_ref(&desk)), Others::Doubt(d) if matches!(d[..], [Elsewhere::Unshown { .. }])));
        // Not gone out, no promise read yet (its tries may go on): a doubt.
        assert!(matches!(others(KEY, DUE, at, "me", true, Some(&with("desk", Tried { unshown: DUE + 5, ..Tried::default() })), std::slice::from_ref(&desk)), Others::Doubt(_)));
        // A clock ahead by three minutes: its promise read three minutes shorter.
        let ahead = Peer { ahead: 180, ..desk.clone() };
        assert!(matches!(others(KEY, DUE, DUE + QUIET_AFTER - 200, "me", true, Some(&with("desk", quiet)), std::slice::from_ref(&ahead)), Others::Doubt(_)));
        // In use, no word on this dose: may have reminded it.
        assert!(matches!(others(KEY, DUE, at, "me", true, Some(&DoseReminders::default()), std::slice::from_ref(&desk)), Others::Doubt(d) if matches!(d[..], [Elsewhere::Unknown { .. }])));
        // Closed before the dose, everything read: did not, until QUIET_AFTER.
        let closed = computer("desk", DUE - 600, false);
        assert_eq!(others(KEY, DUE, at, "me", true, Some(&DoseReminders::default()), std::slice::from_ref(&closed)), Others::First);
        assert!(matches!(others(KEY, DUE, DUE + QUIET_AFTER, "me", true, Some(&DoseReminders::default()), std::slice::from_ref(&closed)), Others::Doubt(_)));
        // Never again, promised (its tries over): first, whenever.
        assert_eq!(others(KEY, DUE, DUE + 7_200, "me", true, Some(&with("desk", Tried { unshown: DUE + 5, quiet: NEVER, ..Tried::default() })), std::slice::from_ref(&desk)), Others::First);
        // Another phone, whatever it wrote of nothing: its own alarm may ring too.
        let peers = [desk.clone(), phone("tablet")];
        match others(KEY, DUE, at, "me", true, Some(&with("desk", quiet)), &peers) {
            Others::Doubt(d) => assert!(matches!(&d[..], [Elsewhere::Alarm { device }] if device.name == "tablet" && !device.phone), "{d:?}"),
            other => panic!("{other:?}"),
        }
        // Not read here: nothing known.
        assert_eq!(others(KEY, DUE, at, "me", true, None, std::slice::from_ref(&desk)), Others::Doubt(vec![Elsewhere::Unread]));
        // A device of mine is not another; one said off does not count.
        assert_eq!(others(KEY, DUE, at, "desk", true, Some(&DoseReminders::default()), std::slice::from_ref(&desk)), Others::First);
        assert_eq!(others(KEY, DUE, at, "me", true, Some(&DoseReminders::default()), &[Peer { off: true, ..desk }]), Others::First);
    }

    /// The file: each device's entry, written whole and kept two days; a file
    /// that does not read, or is lost after it was written, is never read empty.
    #[test]
    fn each_device_writes_its_own_entry_and_a_lost_file_is_not_empty() {
        let dir = std::env::temp_dir().join(format!("sioul-dose-reminders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        crate::health::APART.with(|apart| *apart.borrow_mut() = Some(dir.join("apart")));
        let path = dir.join(FILE);
        assert_eq!(DoseReminders::read(&path), Ok(DoseReminders::default()), "never written: empty");
        DoseReminders::update(&path, DUE + 1, |all| all.entry(KEY, "desk").maybe = DUE + 1).unwrap();
        DoseReminders::update(&path, DUE + 2, |all| all.entry(KEY, "phone").shown = DUE + 2).unwrap();
        let read = DoseReminders::read(&path).unwrap();
        assert_eq!(read.of(KEY, "desk"), Some(Tried { maybe: DUE + 1, ..Tried::default() }));
        assert!(read.shown(KEY), "shown on the phone: reminded");
        assert_eq!(read.maybe_elsewhere(KEY, "phone"), Some(DUE + 1), "the desk's, as the phone reads it");
        assert_eq!(read.maybe_elsewhere(KEY, "desk"), None, "none but its own, as the desk reads it");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[dose.\"levo@1800000000\".desk]"), "{text}");
        // Two days on, gone.
        DoseReminders::update(&path, DUE + 3 * 86_400, |_| ()).unwrap();
        assert!(DoseReminders::read(&path).unwrap().dose.is_empty());
        // Lost after it was written: never empty.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(DoseReminders::read(&path), Err(Unsound::Lost));
        std::fs::write(&path, "[dose.\"x\"").unwrap();
        assert_eq!(DoseReminders::read(&path), Err(Unsound::Unreadable));
        assert!(DoseReminders::update(&path, DUE, |_| ()).is_err(), "nothing written over it");
        crate::health::APART.with(|apart| *apart.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
