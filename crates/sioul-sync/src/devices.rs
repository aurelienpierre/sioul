// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Each device says how it is, in the sharing folder (docs/database.md,
//! "Devices"): when it last started, when it last closed cleanly, whether it
//! is working now, when it last read the others and when it last wrote its
//! own. The doses rely on it (docs/health.md, "Knowing"): a device that closed
//! cleanly after its last export holds no answer the others have not read; one
//! still working may.
//!
//! One file per device, `<folder>/devices/<id>.device`, written by that device
//! only, sealed like the claims (bound to the device it names), its size
//! changed at each writing (`share::pad_for`: some sync apps tell a change by
//! its size alone), a few hundred bytes, never read past 64 KiB. The same,
//! privately, in `$XDG_STATE_HOME/sioul/share/device.toml`, never shared,
//! changed under a lock and read just before (`change`): two processes of
//! Sioul (the window, a phone's background service) never write an older
//! state back over the other's.
//!
//! Written into the folder only when the others need it (`change`): at each
//! export while the device is in use (`working`: the doses need its exports
//! fresh, `sioul_core::health::FRESH`), when anything else it says changes
//! (its session, how far it wrote, whether it shares its doses, its build),
//! when the folder's copy is not the one it wrote, and once an hour
//! (`RESTATED`). A device put away whose exchanges find nothing new writes
//! nothing there: a phone's background step every few minutes would
//! otherwise rewrite it, and its sync app send it, each time.
//!
//! The identity is the sharing's own (`share::Here::id`, a UUID made once per
//! machine), never the host name, which can change or be another's too.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A device's entry: what it says of itself, its times on its own clock (Unix seconds).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    /// To say where: a computer's host name, a phone's model.
    #[serde(default)]
    pub name: String,
    /// `PHONE` or `COMPUTER`.
    #[serde(default)]
    pub kind: String,
    /// The Sioul that wrote it: its version ("0.0.3") and the commit it was
    /// built from (`sioul_core::build::COMMIT`); an older Sioul's entry says
    /// its version alone, or neither, and reads all the same.
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub commit: String,
    /// The format of what travels that its Sioul reads and writes
    /// (`share::FORMAT`); 0 from an older Sioul, which reads format 1 only.
    /// The sharing writes format 2 once every device says 2 (`share::format_holders`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub format: u32,
    /// Its last start: Sioul opened; on a phone, Sioul back on the screen, or
    /// one of its reminders handled while it was not.
    #[serde(default)]
    pub started: i64,
    /// Its last clean close, after its last export; 0 never.
    #[serde(default)]
    pub closed: i64,
    /// Up from its start to the last step of a clean close: a crash leaves it
    /// up. Required: an entry without it is no entry, never one taken as closed.
    pub working: bool,
    /// When it last read the others.
    #[serde(default)]
    pub imported: i64,
    /// When its files were last read for an export: everything captured there
    /// before is in its records, up to `wrote`.
    #[serde(default)]
    pub exported: i64,
    /// How far its records went then: round and number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrote: Option<(u32, u64)>,
    /// It shares the health part: the answers it captures travel.
    #[serde(default)]
    pub doses: bool,
    /// A phone: whether Android lets Sioul hold other apps' notifications
    /// there (notification access), as it last looked; None when unsaid (a
    /// computer, an older Sioul). A computer's "What reaches you" counts a
    /// phone's messages unless it says no (an older Sioul's, unsaid, as before).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notifications: Option<bool>,
    /// It stopped sharing: it counts no more.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub left: bool,
    /// Its size changed at each writing (`share::pad_for`): read by nobody.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pad: String,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

pub const PHONE: &str = "phone";
pub const COMPUTER: &str = "computer";

impl Entry {
    /// A session begins: Sioul opened, brought back, or a receiver at work.
    pub fn start(&mut self, now: i64) {
        self.started = now;
        self.working = true;
        self.left = false;
    }

    /// The last step of a clean session, after its last export: closed then
    /// (never before its own start, whatever the clock did meanwhile).
    pub fn close(&mut self, now: i64) {
        self.closed = now.max(self.started);
        self.working = false;
    }

    /// An exchange done (`share::Outcome`): the others read `now`, this
    /// device's files read for its export at `looked` (milliseconds; 0 when it
    /// did not run), its records up to `wrote`. Never its session.
    pub fn exported(&mut self, looked_ms: i64, wrote: Option<(u32, u64)>, now: i64) {
        if looked_ms <= 0 {
            return;
        }
        self.imported = now;
        self.exported = self.exported.max(looked_ms / 1000);
        if wrote.is_some() {
            self.wrote = wrote;
        }
    }

    /// The build that wrote it, in words: "0.0.3 (eff8661abcde)"; its version
    /// alone from an older Sioul; "" when unsaid.
    pub fn build(&self) -> String {
        sioul_core::build::described(&self.version, &self.commit)
    }

    /// What the doses read of it (`sioul_core::health::Said`).
    pub fn said(&self) -> sioul_core::health::Said {
        sioul_core::health::Said { started: self.started, closed: self.closed, working: self.working, exported: self.exported, imported: self.imported, doses: self.doses, left: self.left, phone: self.kind == PHONE }
    }
}

fn folder_of(folder: &Path) -> PathBuf {
    folder.join("devices")
}

fn file_of(folder: &Path, id: &str) -> PathBuf {
    folder_of(folder).join(format!("{id}.device"))
}

fn bound(id: &str) -> String {
    format!("device\u{1f}{id}")
}

/// This device's entry, sealed into its file in the folder, its size changed
/// from the one there.
pub fn publish(folder: &Path, key: &[u8; 32], entry: &Entry) -> Result<(), String> {
    let path = file_of(folder, &entry.id);
    let sealed = |pad: usize| serde_json::to_vec(&Entry { pad: ".".repeat(pad), ..entry.clone() }).map(|plain| crate::share::seal(key, &bound(&entry.id), &plain)).map_err(|e| e.to_string());
    let pad = crate::share::pad_for(&path, |pad| sealed(pad).map_or(0, |s| s.len() as u64));
    crate::share::write_atomically(&path, sealed(pad)?.as_bytes())
}

/// Every device's entry that reads, and the ids of the devices whose file
/// does not (its seal not arrived, damaged, another key's, written by an
/// older Sioul's sharing that knows no `working`): those are no entry, never
/// one read as closed. Files of other names (a sync app's conflicted copies,
/// half-written ones) are not read. Of an entry fetched from the server too
/// (`remote::overlay`), the later of the two copies (`written`); the
/// folder's on a tie.
pub fn all(folder: &Path, key: &[u8; 32]) -> (Vec<Entry>, Vec<String>) {
    let (mut entries, mut unread) = (Vec::new(), Vec::new());
    let fetched = crate::remote::overlay(folder);
    let ids = |dir: &Path| -> Vec<String> {
        let listing = std::fs::read_dir(folder_of(dir)).into_iter().flatten().filter_map(Result::ok);
        listing.filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".device")).map(str::to_string)).filter(|id| id.len() == 36 && uuid::Uuid::parse_str(id).is_ok()).collect()
    };
    let mut all: std::collections::BTreeSet<String> = ids(folder).into_iter().collect();
    if let Some(fetched) = &fetched {
        all.extend(ids(fetched));
    }
    let read = |dir: &Path, id: &str| crate::share::read_small(&file_of(dir, id)).and_then(|text| crate::share::open(key, &bound(id), text.trim())).and_then(|plain| serde_json::from_slice::<Entry>(&plain).ok()).filter(|entry| entry.id == id);
    for id in all {
        let entry = match (read(folder, &id), fetched.as_deref().and_then(|f| read(f, &id))) {
            (Some(here), Some(fetched)) => Some(if written(&fetched) > written(&here) { fetched } else { here }),
            (here, fetched) => here.or(fetched),
        };
        match entry {
            Some(entry) => entries.push(entry),
            None => unread.push(id),
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    unread.sort();
    (entries, unread)
}

/// When an entry was written, as far as its times tell: each writing sets
/// one of them to its device's clock then (a start, a close, an exchange), and
/// none goes back.
pub(crate) fn written(entry: &Entry) -> (i64, i64, i64, i64, i64) {
    (entry.started.max(entry.closed).max(entry.imported), entry.closed, entry.started, entry.imported, entry.exported)
}

/// Where this device keeps its own entry, never shared.
pub fn own_path(state: &Path) -> PathBuf {
    state.join("share").join("device.toml")
}

/// This device's own entry as kept here; a new one when none reads.
pub fn own(path: &Path) -> Entry {
    std::fs::read_to_string(path).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// Whether this device is in use, as its own entry says (`Entry::working`):
/// the entry kept beside the sharing's memory (`<state>/share/`, `own_path`).
pub(crate) fn in_use(memory: &Path) -> bool {
    own(&memory.with_file_name("device.toml")).working
}

/// How long a device not in use leaves its entry in the folder as it is, at
/// most, while only the times of its exchanges change (seconds): restated
/// then, so that the others still hear of it (Settings ▸ Your folder and
/// sharing, the texts' "last shared", `sioul_core::health::SILENT_DAYS`). The
/// doses never wait for it: a device put away is known by its close and how
/// far it wrote, both written at once.
pub const RESTATED: i64 = 60 * 60;

/// What this device last wrote into the folder, kept beside its own entry and
/// never shared (`published_path`): what it said then, its exchanges' times
/// left out (`steady`), the file as written (its size and time), where, under
/// which key (`sealed_with`: the folder sealed again, it is written again at
/// once), and when.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Published {
    said: String,
    folder: String,
    #[serde(default)]
    key: String,
    size: u64,
    modified: u64,
    at: i64,
}

/// Which key an entry was sealed with, as a mark that says nothing of the key
/// itself (a hash of it under a name of its own).
fn sealed_with(key: &[u8; 32]) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::new().chain_update(b"sioul: the key a device entry was sealed with\x1f").chain_update(key).finalize();
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn published_path(own: &Path) -> PathBuf {
    own.with_file_name("device-published.toml")
}

/// What an entry says but for its exchanges' times, as a hash: what the
/// others must hear at once when it changes.
pub(crate) fn steady(entry: &Entry) -> String {
    let steady = Entry { imported: 0, exported: 0, pad: String::new(), ..entry.clone() };
    serde_json::to_string(&steady).map(|text| crate::share::hash(&text)).unwrap_or_default()
}

/// The size and time of the folder's copy of an entry, when there is one.
fn stamp_of(path: &Path) -> Option<(u64, u64)> {
    std::fs::metadata(path).ok().map(|meta| (meta.len(), crate::share::modified_ns(&meta)))
}

/// This device's own entry changed by `change`, under its lock, read just
/// before: written here, then, sharing on (`vault`: the folder and its key),
/// into the folder when the others need it (see the module's words).
/// Returns it as written here.
pub fn change(path: &Path, vault: Option<(&Path, &[u8; 32])>, change: impl FnOnce(&mut Entry)) -> Result<Entry, String> {
    change_at(path, vault, jiff::Timestamp::now().as_second(), change)
}

/// `change`, at `now` (Unix seconds, this device's clock): when the folder's copy was last restated.
pub(crate) fn change_at(path: &Path, vault: Option<(&Path, &[u8; 32])>, now: i64, change: impl FnOnce(&mut Entry)) -> Result<Entry, String> {
    sioul_core::filelock::with_lock(path, || {
        let mut entry = own(path);
        change(&mut entry);
        entry.pad.clear();
        let text = toml::to_string(&entry).map_err(|e| e.to_string())?;
        crate::share::write_atomically(path, format!("# This device, as it tells your other devices (docs/database.md, \"Devices\").\n{text}").as_bytes())?;
        if let Some((folder, key)) = vault
            && !entry.id.is_empty()
        {
            let file = file_of(folder, &entry.id);
            let said = steady(&entry);
            let there = folder.display().to_string();
            let last: Option<Published> = std::fs::read_to_string(published_path(path)).ok().and_then(|t| toml::from_str(&t).ok());
            // As the others last read it: the same words, the folder's copy the one written, within the hour.
            let mark = sealed_with(key);
            let current = last.is_some_and(|last| last.said == said && last.folder == there && last.key == mark && stamp_of(&file) == Some((last.size, last.modified)) && (0..RESTATED).contains(&(now - last.at)));
            if entry.working || !current {
                publish(folder, key, &entry)?;
                let (size, modified) = stamp_of(&file).unwrap_or_default();
                let published = Published { said, folder: there, key: mark, size, modified, at: now };
                // Lost, the entry is only written again at the next change: nothing is said wrongly.
                let _ = toml::to_string(&published).map_err(|e| e.to_string()).and_then(|text| crate::share::write_atomically(&published_path(path), text.as_bytes()));
            }
        }
        Ok(entry)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::{Roots, Sharing, Store};
    use sioul_core::config::Config;
    use sioul_core::doses::{Answer, Answered, DoseRecords, SKIPPED, TAKEN};
    use sioul_core::health::{Doubt, HealthState, Peer, doubts_now};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-devices-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const KEY: [u8; 32] = [7u8; 32];

    #[test]
    fn a_device_says_how_it_is() {
        let dir = scratch("says");
        let (folder, state) = (dir.join("shared"), dir.join("state"));
        let id = uuid::Uuid::new_v4().to_string();
        let own = own_path(&state);
        let vault = Some((folder.as_path(), &KEY));
        // Started: up, said in the folder, sealed (nothing of it readable there).
        change(&own, vault, |e| {
            e.id = id.clone();
            e.kind = PHONE.into();
            e.doses = true;
            e.start(1_000);
        })
        .unwrap();
        let raw = std::fs::read_to_string(file_of(&folder, &id)).unwrap();
        assert!(!raw.contains("working") && !raw.contains(&id), "sealed");
        let size = raw.len();
        let (entries, unread) = all(&folder, &KEY);
        assert!(unread.is_empty());
        assert_eq!((entries[0].started, entries[0].working, entries[0].said().phone), (1_000, true, true));
        // An exchange: what it read and wrote, never its session.
        change(&own, vault, |e| e.exported(1_060_000, Some((1, 4)), 1_061)).unwrap();
        // Closed after its last export: down.
        change(&own, vault, |e| e.close(1_120)).unwrap();
        let (entries, _) = all(&folder, &KEY);
        assert_eq!((entries[0].exported, entries[0].wrote, entries[0].closed, entries[0].working), (1_060, Some((1, 4)), 1_120, false));
        assert_ne!(std::fs::metadata(file_of(&folder, &id)).unwrap().len() as usize, size, "its size changes at each writing");
        // Kept here too, never shared.
        assert_eq!(super::own(&own).closed, 1_120);
        // Another key reads nothing: no entry, said apart; never taken as closed.
        let (entries, unread) = all(&folder, &[1u8; 32]);
        assert!(entries.is_empty() && unread == [id.clone()]);
        // A file of another name (a conflicted copy) is not read; one sealed for another device is no entry.
        std::fs::copy(file_of(&folder, &id), folder_of(&folder).join(format!("{id} (conflicted copy).device"))).unwrap();
        let other = uuid::Uuid::new_v4().to_string();
        std::fs::copy(file_of(&folder, &id), file_of(&folder, &other)).unwrap();
        let (entries, unread) = all(&folder, &KEY);
        assert_eq!((entries.len(), unread), (1, vec![other]));
        // An entry without `working` (written by something else): no entry, never closed.
        let without = serde_json::json!({ "id": id, "started": 1_000, "closed": 2_000 }).to_string();
        std::fs::write(file_of(&folder, &id), crate::share::seal(&KEY, &bound(&id), without.as_bytes())).unwrap();
        assert!(all(&folder, &KEY).0.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Each entry says the build that wrote it (docs/database.md, "Devices");
    /// one written before the field came, by an older Sioul, reads all the same.
    #[test]
    fn an_entry_says_its_build_and_an_older_one_still_reads() {
        let dir = scratch("build");
        let (folder, state) = (dir.join("shared"), dir.join("state"));
        let id = uuid::Uuid::new_v4().to_string();
        let vault = Some((folder.as_path(), &KEY));
        change(&own_path(&state), vault, |e| {
            e.id = id.clone();
            e.version = sioul_core::build::VERSION.into();
            e.commit = sioul_core::build::COMMIT.into();
            e.start(1_000);
        })
        .unwrap();
        let (entries, _) = all(&folder, &KEY);
        assert_eq!((entries[0].version.as_str(), entries[0].commit.as_str()), (sioul_core::build::VERSION, sioul_core::build::COMMIT));
        assert_eq!(entries[0].build(), sioul_core::build::DESCRIBED);
        assert!(!std::fs::read_to_string(file_of(&folder, &id)).unwrap().contains(sioul_core::build::COMMIT), "sealed: the server never learns it");
        // An older Sioul's entry: its version alone, no commit; one older still, neither.
        for (plain, said) in [(serde_json::json!({ "id": id, "version": "0.0.2", "started": 1_000, "working": true }), "0.0.2"), (serde_json::json!({ "id": id, "started": 1_000, "working": true }), "")] {
            std::fs::write(file_of(&folder, &id), crate::share::seal(&KEY, &bound(&id), plain.to_string().as_bytes())).unwrap();
            let (entries, unread) = all(&folder, &KEY);
            assert!(unread.is_empty() && entries[0].working && entries[0].commit.is_empty(), "{entries:?}");
            assert_eq!(entries[0].build(), said);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A phone says whether Android lets Sioul hold other apps' notifications;
    /// a change that does not look again keeps what it said; an older Sioul
    /// says nothing, read as unsaid.
    #[test]
    fn a_phone_says_whether_it_holds_notifications() {
        let dir = scratch("notifications");
        let (folder, state) = (dir.join("shared"), dir.join("state"));
        let id = uuid::Uuid::new_v4().to_string();
        let vault = Some((folder.as_path(), &KEY));
        change(&own_path(&state), vault, |e| {
            e.id = id.clone();
            e.kind = PHONE.into();
            e.notifications = Some(true);
            e.start(1_000);
        })
        .unwrap();
        change(&own_path(&state), vault, |e| e.close(1_100)).unwrap();
        assert_eq!(all(&folder, &KEY).0[0].notifications, Some(true), "kept by a change that did not look");
        change(&own_path(&state), vault, |e| e.notifications = Some(false)).unwrap();
        assert_eq!(all(&folder, &KEY).0[0].notifications, Some(false));
        let older = serde_json::json!({ "id": id, "kind": "phone", "started": 1_000, "working": true });
        std::fs::write(file_of(&folder, &id), crate::share::seal(&KEY, &bound(&id), older.to_string().as_bytes())).unwrap();
        assert_eq!(all(&folder, &KEY).0[0].notifications, None, "an older Sioul's: unsaid");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A device put away (a phone's background step every few minutes) whose
    /// exchanges find nothing new leaves its entry in the folder as it is:
    /// what the others need goes there at once (its session, how far it
    /// wrote, whether it shares its doses), a copy there that is not the one
    /// it wrote is written again, and an hour on it is restated. In use, every
    /// export goes there, as the doses' knowing needs (`FRESH`).
    #[test]
    fn a_device_put_away_writes_its_entry_only_when_it_says_something_new() {
        let dir = scratch("put-away");
        let (folder, state) = (dir.join("shared"), dir.join("state"));
        let own = own_path(&state);
        let vault = Some((folder.as_path(), &KEY));
        let id = uuid::Uuid::new_v4().to_string();
        let file = file_of(&folder, &id);
        let bytes = || std::fs::read(&file).unwrap();
        let exported = |at: i64, wrote: (u32, u64)| change_at(&own, vault, at, |e| e.exported(at * 1000, Some(wrote), at)).unwrap();
        let t = 1_800_000_000;
        change_at(&own, vault, t, |e| {
            e.id = id.clone();
            e.kind = PHONE.into();
            e.doses = true;
            e.start(t);
        })
        .unwrap();
        exported(t, (1, 4));
        change_at(&own, vault, t + 5, |e| e.close(t + 5)).unwrap();
        let closed = bytes();
        // Exchanges that wrote nothing: kept here, nothing written there.
        for n in 1..=5 {
            exported(t + 5 + n * 120, (1, 4));
        }
        assert_eq!(bytes(), closed, "nothing new: the folder's copy as it was");
        assert_eq!(super::own(&own).exported, t + 605, "kept here");
        let seen = &all(&folder, &KEY).0[0];
        assert_eq!((seen.exported, seen.wrote, seen.working), (t, Some((1, 4)), false));
        // How far it wrote changed: there at once, with its time.
        exported(t + 700, (2, 1));
        let seen = all(&folder, &KEY).0[0].clone();
        assert_eq!((seen.exported, seen.wrote, seen.working), (t + 700, Some((2, 1)), false));
        // Within the hour, nothing new: as it is; an hour on: restated.
        let wrote = bytes();
        exported(t + 700 + RESTATED - 1, (2, 1));
        assert_eq!(bytes(), wrote);
        exported(t + 700 + RESTATED, (2, 1));
        assert_eq!(all(&folder, &KEY).0[0].exported, t + 700 + RESTATED, "restated after an hour");
        // An older copy put back by a sync app: written again at the next export.
        std::fs::write(&file, &closed).unwrap();
        exported(t + 800 + RESTATED, (2, 1));
        assert_eq!(all(&folder, &KEY).0[0].wrote, Some((2, 1)), "the copy put back is replaced");
        // Its doses no longer shared: there at once.
        change_at(&own, vault, t + 900 + RESTATED, |e| e.doses = false).unwrap();
        assert!(!all(&folder, &KEY).0[0].doses);
        // The folder sealed again (another passphrase, the same place): written under the new key at once.
        let other = [9u8; 32];
        exported(t + 950 + RESTATED, (2, 1));
        change_at(&own, Some((folder.as_path(), &other)), t + 960 + RESTATED, |e| e.exported((t + 960 + RESTATED) * 1000, Some((2, 1)), t + 960 + RESTATED)).unwrap();
        assert_eq!(all(&folder, &other).0.len(), 1, "readable under the new key at once");
        // In use: every export, each with its time.
        change_at(&own, vault, t + 1_000 + RESTATED, |e| e.start(t + 1_000 + RESTATED)).unwrap();
        for n in 1..=3 {
            let at = t + 1_000 + RESTATED + n * 60;
            exported(at, (2, 1));
            let seen = &all(&folder, &KEY).0[0];
            assert_eq!((seen.exported, seen.working), (at, true), "in use: export {n} there at once");
        }
        // Another folder (sharing moved): written there at once.
        let elsewhere = dir.join("elsewhere");
        change_at(&own, Some((elsewhere.as_path(), &KEY)), t + 2_000 + RESTATED, |e| e.close(t + 2_000 + RESTATED)).unwrap();
        exported(t + 2_100 + RESTATED, (2, 1));
        let moved = dir.join("moved");
        change_at(&own, Some((moved.as_path(), &KEY)), t + 2_200 + RESTATED, |e| e.exported((t + 2_200 + RESTATED) * 1000, Some((2, 1)), t + 2_200 + RESTATED)).unwrap();
        assert_eq!(all(&moved, &KEY).0.len(), 1, "the new folder has it at once");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_processes_never_undo_each_others_session() {
        let dir = scratch("processes");
        let (folder, state) = (dir.join("shared"), dir.join("state"));
        let own = own_path(&state);
        let vault = Some((folder.as_path(), &KEY));
        let id = uuid::Uuid::new_v4().to_string();
        change(&own, vault, |e| {
            e.id = id.clone();
            e.start(1_000);
        })
        .unwrap();
        // The window's process puts Sioul away (down); a background service then exports: still down.
        change(&own, vault, |e| e.close(1_100)).unwrap();
        change(&own, vault, |e| e.exported(1_200_000, Some((1, 9)), 1_201)).unwrap();
        let (entries, _) = all(&folder, &KEY);
        assert_eq!((entries[0].working, entries[0].closed, entries[0].exported), (false, 1_100, 1_200));
        // Brought back (up); the service exports again: still up.
        change(&own, vault, |e| e.start(1_300)).unwrap();
        change(&own, vault, |e| e.exported(1_310_000, Some((1, 9)), 1_311)).unwrap();
        assert!(all(&folder, &KEY).0[0].working);
        // An exchange that did not run (busy) changes nothing.
        change(&own, vault, |e| e.exported(0, None, 1_400)).unwrap();
        assert_eq!(all(&folder, &KEY).0[0].exported, 1_310);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------ two devices, one folder

    /// A device of its own, in a scratch folder: its folders, its id, its memory.
    struct Device {
        roots: Roots,
        id: String,
        memory: PathBuf,
        kind: &'static str,
    }

    impl Device {
        fn new(base: &Path, name: &str, kind: &'static str) -> Device {
            let root = base.join(name);
            let roots = Roots { config: root.join("config"), data: root.join("data"), state: root.join("state") };
            for dir in [&roots.config, &roots.data, &roots.state] {
                std::fs::create_dir_all(dir).unwrap();
            }
            let memory = roots.state.join("share").join("memory.json");
            Device { roots, id: uuid::Uuid::new_v4().to_string(), memory, kind }
        }

        fn stores(&self) -> Vec<Store> {
            crate::share::stores(&Config::default(), &self.roots)
        }

        /// An exchange as `crate::share::exchange` makes it, then its entry
        /// says so: what it read and wrote (`exported`).
        fn exchange(&self, folder: &Path, now: i64) -> crate::share::Outcome {
            let outcome = crate::share::exchange(&Sharing { folder, computer: &self.id, key: &KEY, memory: &self.memory, files: false, hurry: None }, &self.stores(), now * 1000).unwrap();
            // The exchange's own clock read the files; a test's clock is `now`.
            change(&own_path(&self.roots.state), Some((folder, &KEY)), |e| e.exported(now * 1000, outcome.wrote, now)).unwrap();
            outcome
        }

        fn session(&self, folder: &Path, f: impl FnOnce(&mut Entry)) {
            let kind = self.kind;
            let id = self.id.clone();
            change(&own_path(&self.roots.state), Some((folder, &KEY)), |e| {
                e.id = id;
                e.kind = kind.into();
                e.doses = true;
                f(e);
            })
            .unwrap();
        }

        fn records(&self) -> PathBuf {
            self.roots.state.join(sioul_core::doses::FILE)
        }

        fn marks(&self) -> PathBuf {
            self.roots.state.join("health-state.toml")
        }

        /// The person answers here, as the window or a receiver records it: the
        /// answer in the records, the mark beside it for an older Sioul.
        fn answer(&self, key: &str, state: &str, at: i64) {
            DoseRecords::update(&self.records(), at, |r| r.answer(key, &self.id, Answer { state: state.into(), at, noted: at, name: self.kind.into(), kind: self.kind.into() })).unwrap();
            HealthState::update(&self.marks(), at, |s| {
                if state == TAKEN {
                    s.taken.insert(key.to_string(), at);
                } else {
                    s.not_taken.insert(key.to_string(), at);
                }
            })
            .unwrap();
        }

        /// What this device knows of `other`, as the window's `know` builds it:
        /// its entry, and whether everything it wrote up to it is read here.
        fn sees(&self, folder: &Path, other: &Device, now: i64) -> Peer {
            let entry = all(folder, &KEY).0.into_iter().find(|e| e.id == other.id).expect("its entry");
            let heard = crate::share::heard(&self.memory, &self.id);
            Peer { id: other.id.clone(), name: entry.name.clone(), said: Some(entry.said()), complete: entry.wrote.is_none_or(|wrote| heard.complete(&other.id, wrote)), seen: now, heard: entry.exported, ..Peer::default() }
        }

        fn answered(&self, key: &str) -> Answered {
            sioul_core::doses::answered(key, &HealthState::read(&self.marks()).unwrap(), &DoseRecords::read(&self.records()).unwrap())
        }
    }

    const DUE: i64 = 1_800_000_000;
    const DOSE: &str = "levo@1800000000";

    /// Two devices sharing one folder, each joined.
    fn pair(name: &str) -> (PathBuf, PathBuf, Device, Device) {
        let base = scratch(name);
        let folder = base.join("shared");
        std::fs::create_dir_all(&folder).unwrap();
        let desk = Device::new(&base, "desk", COMPUTER);
        let phone = Device::new(&base, "phone", PHONE);
        desk.session(&folder, |e| e.start(DUE - 7_200));
        phone.session(&folder, |e| e.start(DUE - 7_200));
        desk.exchange(&folder, DUE - 7_100);
        phone.exchange(&folder, DUE - 7_090);
        desk.exchange(&folder, DUE - 7_080);
        (base, folder, desk, phone)
    }

    #[test]
    fn the_phone_marks_taken_in_the_background_and_the_desktop_shows_taken() {
        let (base, folder, desk, phone) = pair("background");
        // The phone put away at 07:00: its last export, then down.
        phone.exchange(&folder, DUE - 3_600);
        phone.session(&folder, |e| e.close(DUE - 3_595));
        desk.exchange(&folder, DUE - 3_500);
        // 08:00, before anything: the phone closed cleanly before the dose, read in full: known not taken.
        let at_dose = desk.sees(&folder, &phone, DUE + 30);
        assert!(doubts_now(DUE, DUE + 30, None, &[at_dose]).is_empty());
        assert_eq!(desk.answered(DOSE), Answered::Open);
        // 08:05, "Taken" pressed on the phone's notification, Sioul in the background:
        // the receiver raises its session, records, exports, then lowers it.
        phone.session(&folder, |e| e.start(DUE + 300));
        phone.answer(DOSE, TAKEN, DUE + 300);
        phone.exchange(&folder, DUE + 302);
        phone.session(&folder, |e| e.close(DUE + 303));
        // The desktop reads the folder: taken, on the phone, at 08:05; nothing in doubt.
        desk.exchange(&folder, DUE + 360);
        match desk.answered(DOSE) {
            Answered::Taken(given) => assert_eq!((given.device.as_str(), given.at), (phone.id.as_str(), DUE + 300)),
            other => panic!("{other:?}"),
        }
        assert!(doubts_now(DUE, DUE + 360, None, &[desk.sees(&folder, &phone, DUE + 360)]).is_empty());
        // The old mark came too, for an older Sioul.
        assert!(HealthState::read(&desk.marks()).unwrap().taken.contains_key(DOSE));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The phone put away, its background step exchanging every few minutes
    /// and finding nothing new: its entry in the folder stays as it was, and
    /// the desktop knows a dose due since as surely as before (closed after its
    /// last export, everything it wrote read), hours later too. "Taken"
    /// pressed on its reminder in the background goes at once, as before:
    /// raised, recorded, exported, lowered, each in the folder.
    #[test]
    fn the_phone_put_away_and_quiet_is_known_without_rewriting_its_entry() {
        let (base, folder, desk, phone) = pair("quiet-phone");
        phone.exchange(&folder, DUE - 3_600);
        phone.session(&folder, |e| e.close(DUE - 3_595));
        let entry = std::fs::read(file_of(&folder, &phone.id)).unwrap();
        for n in 0..12 {
            phone.exchange(&folder, DUE - 3_500 + n * 300);
            desk.exchange(&folder, DUE - 3_490 + n * 300);
        }
        assert_eq!(std::fs::read(file_of(&folder, &phone.id)).unwrap(), entry, "nothing new: its entry as it was");
        for at in [DUE + 30, DUE + 3_600, DUE + 6 * 3_600] {
            assert!(doubts_now(DUE, at, None, &[desk.sees(&folder, &phone, at)]).is_empty(), "known at {at}");
        }
        // Raised by a reminder: in the folder at once; the desk doubts until its export is read.
        phone.session(&folder, |e| e.start(DUE + 300));
        assert!(matches!(doubts_now(DUE, DUE + 310, None, &[desk.sees(&folder, &phone, DUE + 310)]).as_slice(), [Doubt::Working { .. }]));
        phone.answer(DOSE, TAKEN, DUE + 300);
        phone.exchange(&folder, DUE + 302);
        phone.session(&folder, |e| e.close(DUE + 303));
        desk.exchange(&folder, DUE + 360);
        assert!(matches!(desk.answered(DOSE), Answered::Taken(_)));
        assert!(doubts_now(DUE, DUE + 360, None, &[desk.sees(&folder, &phone, DUE + 360)]).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// What the desktop knows of the phone with `seen` as `health::know` keeps
    /// it (when the phone's entry was last seen changing: its session, its
    /// export), not the moment it looks (review of battery part B, F7). Put
    /// away and quiet for hours, its entry unchanged: `seen` hours old, the
    /// dose known all the same (a closed device's knowing never reads it). In
    /// use, exporting every minute: known after the dose while it goes on;
    /// stopped without closing (a crash), its entry still: the doubt said once
    /// `FRESH` has passed.
    #[test]
    fn a_dose_is_known_with_seen_as_the_desktop_keeps_it() {
        let (base, folder, desk, phone) = pair("seen-kept");
        let mut watched: Option<((i64, i64, bool, i64, bool), i64)> = None;
        let mut sees = |at: i64| -> Peer {
            let entry = all(&folder, &KEY).0.into_iter().find(|e| e.id == phone.id).expect("its entry");
            let state = (entry.started, entry.closed, entry.working, entry.exported, entry.left);
            if watched.is_none_or(|(last, _)| last != state) {
                watched = Some((state, at));
            }
            Peer { seen: watched.map_or(at, |(_, seen)| seen), ..desk.sees(&folder, &phone, at) }
        };
        phone.exchange(&folder, DUE - 3_600);
        phone.session(&folder, |e| e.close(DUE - 3_595));
        desk.exchange(&folder, DUE - 3_590);
        let closed_seen = sees(DUE - 3_590).seen;
        for n in 0..12 {
            phone.exchange(&folder, DUE - 3_500 + n * 300);
            desk.exchange(&folder, DUE - 3_490 + n * 300);
            assert_eq!(sees(DUE - 3_490 + n * 300).seen, closed_seen, "put away and quiet: its entry not seen changing");
        }
        for at in [DUE + 30, DUE + 3_600, DUE + 6 * 3_600] {
            let peer = sees(at);
            assert!(at - peer.seen > sioul_core::health::FRESH);
            assert!(doubts_now(DUE, at, None, &[peer]).is_empty(), "closed, its seen {} s old: known at {at}", at - closed_seen);
        }
        // In use from DUE + 400, exporting every minute: seen follows, the dose known after it.
        let later = DUE + 7 * 3_600;
        phone.session(&folder, |e| e.start(later));
        for n in 1..=5 {
            phone.exchange(&folder, later + n * 60);
            desk.exchange(&folder, later + n * 60 + 5);
            assert!(doubts_now(later, later + n * 60 + 10, None, &[sees(later + n * 60 + 10)]).is_empty() || n * 60 < sioul_core::health::SKEW, "in use, exporting: known at minute {n}");
        }
        // Stopped without closing (a crash): its entry still, the doubt said once FRESH has passed.
        let stopped = later + 5 * 60 + 10 + sioul_core::health::FRESH + 60;
        desk.exchange(&folder, stopped - 5);
        assert!(matches!(doubts_now(later, stopped, None, &[sees(stopped)]).as_slice(), [Doubt::Working { .. }]), "a phone in use gone quiet: the doubt said");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_phone_raised_but_not_exported_is_uncertain_and_its_records_behind_its_entry_too() {
        let (base, folder, desk, phone) = pair("raised");
        // The phone's receiver raised its session and recorded, then was stopped before its export.
        phone.session(&folder, |e| e.start(DUE + 300));
        phone.answer(DOSE, TAKEN, DUE + 300);
        desk.exchange(&folder, DUE + 400);
        assert_eq!(desk.answered(DOSE), Answered::Open, "its answer not exported");
        let doubts = doubts_now(DUE, DUE + 400, None, &[desk.sees(&folder, &phone, DUE + 400)]);
        assert!(matches!(doubts.as_slice(), [Doubt::Working { device, .. }] if device.id == phone.id), "{doubts:?}");
        // Its export done, its entry in the folder, but its new records only half arrived here (the
        // sync brought its entry first): what it last shared is coming, not known.
        let rounds = || -> Vec<PathBuf> { std::fs::read_dir(&folder).unwrap().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(&phone.id) && p.extension().is_some_and(|x| x == "jsonl")).collect() };
        let before: Vec<(PathBuf, usize)> = rounds().into_iter().map(|p| (p.clone(), std::fs::read(&p).unwrap().len())).collect();
        phone.exchange(&folder, DUE + 500);
        phone.session(&folder, |e| e.close(DUE + 501));
        let held: Vec<(PathBuf, Vec<u8>)> = rounds().into_iter().map(|p| (p.clone(), std::fs::read(&p).unwrap())).collect();
        for (path, bytes) in &held {
            let had = before.iter().find(|(p, _)| p == path).map_or(0, |(_, size)| *size);
            std::fs::write(path, &bytes[..had + (bytes.len() - had) / 2]).unwrap();
        }
        desk.exchange(&folder, DUE + 510);
        let behind = desk.sees(&folder, &phone, DUE + 520);
        assert!(!behind.complete);
        assert!(matches!(doubts_now(DUE, DUE + 520, None, &[behind]).as_slice(), [Doubt::Coming { .. }]));
        for (path, bytes) in &held {
            std::fs::write(path, bytes).unwrap();
        }
        desk.exchange(&folder, DUE + 560);
        assert!(matches!(desk.answered(DOSE), Answered::Taken(_)));
        assert!(doubts_now(DUE, DUE + 560, None, &[desk.sees(&folder, &phone, DUE + 560)]).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_same_dose_answered_on_both_devices_keeps_both() {
        let (base, folder, desk, phone) = pair("both");
        // Taken on the phone at 08:02 (its sync slow), skipped on the desktop at 08:10.
        phone.answer(DOSE, TAKEN, DUE + 120);
        desk.answer(DOSE, SKIPPED, DUE + 600);
        phone.exchange(&folder, DUE + 700);
        desk.exchange(&folder, DUE + 710);
        phone.exchange(&folder, DUE + 720);
        // Both answers kept on both devices, both said; none picked.
        for device in [&desk, &phone] {
            match device.answered(DOSE) {
                Answered::Differ(given) => {
                    let said: Vec<(String, sioul_core::doses::State)> = given.iter().map(|g| (g.device.clone(), g.state.clone())).collect();
                    assert_eq!(said, [(phone.id.clone(), sioul_core::doses::State::Taken), (desk.id.clone(), sioul_core::doses::State::Skipped)]);
                }
                other => panic!("{other:?}"),
            }
        }
        // You choose on the desktop, seeing both: "taken". Settled everywhere, the phone's answer kept.
        DoseRecords::update(&desk.records(), DUE + 800, |r| r.choose(DOSE, &desk.id, Answer { state: TAKEN.into(), at: DUE + 120, noted: DUE + 800, name: "desk".into(), kind: COMPUTER.into() })).unwrap();
        // The marks beside, for an older Sioul, as the window settles them.
        HealthState::update(&desk.marks(), DUE + 800, |s| {
            s.not_taken.remove(DOSE);
            s.taken.insert(DOSE.into(), DUE + 120);
        })
        .unwrap();
        desk.exchange(&folder, DUE + 810);
        phone.exchange(&folder, DUE + 820);
        assert!(matches!(phone.answered(DOSE), Answered::Taken(ref g) if g.device == phone.id));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_choice_or_a_take_back_on_one_device_settles_both() {
        let (base, folder, desk, phone) = pair("settle");
        // Taken on the phone, skipped on the desktop: both said on both devices.
        phone.answer(DOSE, TAKEN, DUE + 120);
        desk.answer(DOSE, SKIPPED, DUE + 600);
        phone.exchange(&folder, DUE + 700);
        desk.exchange(&folder, DUE + 710);
        phone.exchange(&folder, DUE + 720);
        assert!(matches!(phone.answered(DOSE), Answered::Differ(_)));
        // You choose "not taken" on the desktop: the phone's "taken" goes, on purpose, on both devices.
        DoseRecords::update(&desk.records(), DUE + 800, |r| r.choose(DOSE, &desk.id, Answer { state: SKIPPED.into(), at: DUE + 800, noted: DUE + 800, name: "desk".into(), kind: COMPUTER.into() })).unwrap();
        HealthState::update(&desk.marks(), DUE + 800, |s| {
            s.taken.remove(DOSE);
            s.not_taken.insert(DOSE.into(), DUE + 800);
        })
        .unwrap();
        desk.exchange(&folder, DUE + 810);
        phone.exchange(&folder, DUE + 820);
        desk.exchange(&folder, DUE + 830);
        for device in [&desk, &phone] {
            let answered = device.answered(DOSE);
            assert!(matches!(answered, Answered::Skipped(ref g) if g.device == desk.id), "{answered:?}");
        }
        // Another dose taken on the phone, taken back on the desktop: open again on both, never taken on one.
        let iron = "iron@1800000000";
        phone.answer(iron, TAKEN, DUE + 900);
        phone.exchange(&folder, DUE + 910);
        desk.exchange(&folder, DUE + 920);
        assert!(matches!(desk.answered(iron), Answered::Taken(_)));
        DoseRecords::update(&desk.records(), DUE + 1_000, |r| r.take_back(iron)).unwrap();
        HealthState::update(&desk.marks(), DUE + 1_000, |s| {
            s.taken.remove(iron);
        })
        .unwrap();
        desk.exchange(&folder, DUE + 1_010);
        phone.exchange(&folder, DUE + 1_020);
        desk.exchange(&folder, DUE + 1_030);
        for device in [&desk, &phone] {
            assert_eq!(device.answered(iron), Answered::Open);
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_canary_opened_on_two_devices_merges_into_one_record() {
        let (base, folder, desk, phone) = pair("canary");
        DoseRecords::update(&desk.records(), DUE + 30, |r| r.open(DOSE, &desk.id, DUE + 30)).unwrap();
        DoseRecords::update(&phone.records(), DUE + 20, |r| r.open(DOSE, &phone.id, DUE + 20)).unwrap();
        desk.exchange(&folder, DUE + 60);
        phone.exchange(&folder, DUE + 70);
        desk.exchange(&folder, DUE + 80);
        for device in [&desk, &phone] {
            let records = DoseRecords::read(&device.records()).unwrap();
            assert_eq!(records.dose.len(), 1);
            assert_eq!(records.dose[DOSE].opened.len(), 2, "both openings kept");
            assert_eq!(records.opened_first(DOSE), Some((phone.id.clone(), DUE + 20)));
            assert_eq!(device.answered(DOSE), Answered::Open, "not taken yet: the canary");
        }
        // An answer on one device lifts it on both; nothing taken out by the merge.
        phone.answer(DOSE, TAKEN, DUE + 90);
        phone.exchange(&folder, DUE + 100);
        desk.exchange(&folder, DUE + 110);
        assert!(matches!(desk.answered(DOSE), Answered::Taken(_)));
        assert_eq!(DoseRecords::read(&desk.records()).unwrap().dose[DOSE].opened.len(), 2);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn an_older_sioul_takes_nothing_out_and_its_marks_still_count() {
        let base = scratch("older");
        let folder = base.join("shared");
        std::fs::create_dir_all(&folder).unwrap();
        let desk = Device::new(&base, "desk", COMPUTER);
        let old = Device::new(&base, "old", PHONE);
        // The older Sioul: no records file among its stores, no entry in the registry.
        let old_stores = || old.stores().into_iter().filter(|s| s.name != "state/health-doses.toml").collect::<Vec<_>>();
        let old_exchange = |now: i64| crate::share::exchange(&Sharing { folder: &folder, computer: &old.id, key: &KEY, memory: &old.memory, files: false, hurry: None }, &old_stores(), now * 1000).unwrap();
        desk.session(&folder, |e| e.start(DUE - 3_600));
        desk.exchange(&folder, DUE - 3_500);
        old_exchange(DUE - 3_400);
        // The desktop opens and answers in its records; the older one exchanges again and again.
        DoseRecords::update(&desk.records(), DUE + 30, |r| r.open(DOSE, &desk.id, DUE + 30)).unwrap();
        desk.answer("iron@1800000000", SKIPPED, DUE + 60);
        desk.exchange(&folder, DUE + 90);
        for n in 0..3 {
            old_exchange(DUE + 100 + n);
        }
        desk.exchange(&folder, DUE + 200);
        let records = DoseRecords::read(&desk.records()).unwrap();
        assert!(records.dose.contains_key(DOSE) && records.dose.contains_key("iron@1800000000"), "nothing taken out by the older one");
        // Its own marks (health-state.toml) still read as answers on the desktop.
        HealthState::update(&old.marks(), DUE + 300, |s| {
            s.taken.insert(DOSE.into(), DUE + 300);
        })
        .unwrap();
        old_exchange(DUE + 310);
        desk.exchange(&folder, DUE + 320);
        assert!(matches!(desk.answered(DOSE), Answered::Taken(ref g) if g.device.is_empty()));
        // It wrote no entry: the desktop has none, and judges it by its claims, as before (never closed).
        assert!(all(&folder, &KEY).0.iter().all(|e| e.id != old.id));
        let _ = std::fs::remove_dir_all(&base);
    }
}
