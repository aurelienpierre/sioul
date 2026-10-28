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
//! fresh, `sioul_core::health::FRESH`), whether someone is at it or not, so
//! that the server, which sees when the file changes, cannot tell when
//! someone sits at a computer (`active` is sealed with the rest); when
//! anything else it says changes (its session, how far it wrote, whether it
//! shares its doses, its build), when the folder's copy is not the one it
//! wrote, and once an hour (`RESTATED`). A device put away whose exchanges
//! find nothing new writes nothing there: a phone's background step every few
//! minutes would otherwise rewrite it, and its sync app send it, each time.
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
    /// How its Sioul keeps the sealed files of notes and papers that no record
    /// points to any more (`blobs::KEEPING`): 1, two weeks, a content used
    /// again made fresh past a week, earlier versions by reference 12 days;
    /// 2, the same, and a file another device took out kept as a copy among
    /// its earlier versions (`blobs::KEEPING_MOVED`); 3, the same, its notes
    /// saying each line read, and a content another device sealed that its
    /// own change names again made fresh whatever its age and taken as its
    /// own (`blobs::KEEPING_FRESH`); 0 from an older Sioul
    /// (90 days, 30, 60). A device keeps its own two weeks only once every
    /// device that counts says 1 at least (`share::unused_days`).
    /// An older Sioul reads the entry as before, the field unknown to it
    /// passed over.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub keeping: u32,
    /// Whether its Sioul shares your own time around meetings with others
    /// (`share::OWN_TIME`, the unified agenda's part 4): 1 from a build that
    /// has that store, 0 from an older one. The store travels only once every
    /// other device that counts says 1 (`share::own_time_shared`): an older
    /// Sioul, which has no such store, would keep its records waiting at
    /// every exchange, never quiet again. An older Sioul reads the entry as
    /// before, the field unknown to it passed over (982d6e5 reads entries with
    /// serde, unknown fields left aside, and writes its own entry alone).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub own_time: u32,
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
    /// A computer in use: whether a person is at it, as it last looked (input
    /// within ten minutes, its session unlocked, or a press in Sioul:
    /// `power::present_of`). None when unsaid: a phone (in use only while
    /// Sioul is on its screen), a computer whose system says nothing of it, an
    /// older Sioul; then Sioul running counts as someone at it, as before.
    /// While you sleep, a phone's background steps slow down when nobody is
    /// at any computer (`others_in_use`); the doses never read it (`said`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    /// It stopped sharing: it counts no more.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub left: bool,
    /// A phone's word on each computer it counts now: the opening (`started`)
    /// of that computer's entry it read, by the computer's id. A computer
    /// reminds a dose plainly as its first only once every phone's entry says
    /// its current opening here, sealed with the rest (docs/health.md, "What
    /// became of a reminder"; review of battery part C, the seventh review's
    /// C1 and M1). Written by phones only, of computers only; a computer the
    /// phone stops counting (said off, forgotten, silent a week) goes from it.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub acked: std::collections::BTreeMap<String, i64>,
    /// The devices this one replaces, as you said when it joined (a phone
    /// that will never share again: dead, lost; docs/database.md, "Devices"),
    /// each with what it had last said of itself then. Every device then
    /// treats it as forgotten (it no longer holds the doses' reminders back,
    /// nor the newer form of what travels) until it says anything newer:
    /// a phone wrongly said replaced comes back by itself. Nothing of it is
    /// deleted. Sealed with the rest; an older Sioul passes over it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replaces: Vec<Replaced>,
    /// Its size changed at each writing (`share::pad_for`): read by nobody.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pad: String,
}

/// A device replaced (`Entry::replaces`): its id, and the latest it had said
/// when you said so, by its own clock: its entry's last start (Unix seconds),
/// and the latest of its entry's times, its claims' renewals and its records'
/// clocks read (`heard`, milliseconds). Anything it says later (a new start,
/// a later time) makes it count again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replaced {
    pub id: String,
    #[serde(default)]
    pub started: i64,
    #[serde(default)]
    pub heard: i64,
}

impl Replaced {
    /// Whether the device it names said nothing newer since: its entry still
    /// that start (`started`), nothing of it later than `heard`
    /// (milliseconds, its clock): `latest_ms`, the latest this device read of it.
    pub fn holds(&self, started: Option<i64>, latest_ms: i64) -> bool {
        started.is_none_or(|started| started == self.started) && latest_ms <= self.heard
    }
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
/// (`remote::overlay`), the later of the two copies, its words as both say
/// them (`chosen`).
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
            (Some(here), Some(fetched)) => Some(chosen(here, fetched)),
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

/// Of an entry read in two copies, the folder's and the one fetched beside
/// it: the later by its own times (`written`), the folder's on a tie, with
/// its words (`Entry::acked`) only on the computers the other copy names too.
/// A phone that no longer names a computer in either copy has stopped
/// counting it (said off, forgotten, silent a week), whichever copy is dated
/// later: a phone's clock set back, or two copies written in one second, no
/// longer keep its word there from the lagging copy. One naming an older
/// opening of it is only behind, and takes nothing away. The cost: a phone
/// that counts a computer again, or is paired anew, gives its word there
/// once both copies name it (the re-check before 0.0.6, LOW-2).
pub fn chosen(here: Entry, fetched: Entry) -> Entry {
    let (mut chosen, other) = if written(&fetched) > written(&here) { (fetched, here) } else { (here, fetched) };
    chosen.acked.retain(|computer, _| other.acked.contains_key(computer));
    chosen
}

/// When an entry was written, as far as its times tell: each writing sets
/// one of them to its device's clock then (a start, a close, an exchange), and
/// none goes back.
pub fn written(entry: &Entry) -> (i64, i64, i64, i64, i64) {
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

/// Whether another device than `own` is in use, for the rhythm of a phone's
/// background steps (`steps::next_step`) and of its pulls: in use, still
/// sharing, its export dated lately (`FRESH`, `SKEW`). While you are awake, a
/// computer counts while its Sioul runs, whoever is at it, as before; while
/// you sleep (`asleep`), only with someone at it: one that says nobody is
/// (`Entry::active`) does not count, so that a computer left on at night with
/// Sioul in its tray no longer makes the phone step every two minutes. One
/// that says nothing of it (an older Sioul, a system that cannot tell) counts
/// by Sioul running, as before. The doses' knowing is `sioul_core::health`'s,
/// never this.
pub fn others_in_use(entries: &[Entry], own: &str, now: i64, asleep: bool) -> bool {
    use sioul_core::health::{FRESH, SKEW};
    entries.iter().any(|e| e.id != own && e.working && !e.left && now - e.exported <= FRESH + SKEW && (!asleep || e.active != Some(false)))
}

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
            // In use, every export goes out, someone at it or not: one cadence, which says nothing of who is there.
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

    /// A computer counts as in use for a phone's rhythm while its Sioul runs
    /// and its news is fresh; while you sleep, only with someone at it (review
    /// of battery part C: the sleep column alone). One that says nothing of it
    /// (an older Sioul, a system that cannot tell) counts by Sioul running, as
    /// before, asleep too.
    #[test]
    fn a_computer_counts_in_use_for_a_phones_rhythm_while_someone_is_at_it() {
        use sioul_core::health::{FRESH, SKEW};
        let now = DUE;
        let desk = |active: Option<bool>| Entry { id: "desk".into(), kind: COMPUTER.into(), working: true, started: now - 3_600, exported: now - 60, active, ..Entry::default() };
        for asleep in [false, true] {
            assert!(others_in_use(&[desk(Some(true))], "phone", now, asleep));
            assert!(others_in_use(&[desk(None)], "phone", now, asleep), "unsaid: Sioul running counts, as before");
            assert!(!others_in_use(&[Entry { exported: now - FRESH - SKEW - 1, ..desk(Some(true)) }], "phone", now, asleep), "its news stale");
            assert!(!others_in_use(&[Entry { working: false, ..desk(Some(true)) }], "phone", now, asleep), "closed");
            assert!(!others_in_use(&[Entry { left: true, ..desk(None) }], "phone", now, asleep), "no longer sharing");
            assert!(!others_in_use(&[desk(Some(true))], "desk", now, asleep), "itself");
            assert!(others_in_use(&[desk(Some(false)), Entry { id: "laptop".into(), ..desk(Some(true)) }], "phone", now, asleep), "someone at another computer");
            assert!(!others_in_use(&[], "phone", now, asleep), "no other device");
        }
        // Left on, nobody at it: in use while you are awake, as 0.0.5 had it; not while you sleep.
        assert!(others_in_use(&[desk(Some(false))], "phone", now, false), "awake: Sioul running counts");
        assert!(!others_in_use(&[desk(Some(false))], "phone", now, true), "asleep: nobody at it, not in use");
    }

    /// A computer in use writes its entry at each export, someone at it or
    /// not (review of battery part C, F5): the server, which sees when the
    /// file changes, cannot tell when someone sits at it; whether someone is
    /// travels sealed with the rest, its size padded as every writing's. A
    /// phone looking at any moment knows a dose due meanwhile once an export
    /// after it is read, as before, and a computer stopped without closing is
    /// doubted once `FRESH` has passed (docs/health.md, "Knowing").
    #[test]
    fn a_computer_writes_its_entry_at_one_cadence_whoever_is_at_it() {
        use sioul_core::health::{FRESH, SKEW};
        let dir = scratch("one-cadence");
        let (folder, state) = (dir.join("shared"), dir.join("state"));
        let own = own_path(&state);
        let vault = Some((folder.as_path(), &KEY));
        let id = uuid::Uuid::new_v4().to_string();
        let file = file_of(&folder, &id);
        let t = DUE - 3_600;
        change_at(&own, vault, t, |e| {
            e.id = id.clone();
            e.kind = COMPUTER.into();
            e.doses = true;
            e.active = Some(true);
            e.start(t);
        })
        .unwrap();
        // The window's export each minute, saying whether someone is at it; whether the folder's copy changed.
        let export = |at: i64, active: Option<bool>| -> bool {
            let before = std::fs::read(&file).unwrap();
            change_at(&own, vault, at, |e| {
                e.exported(at * 1000, Some((1, 4)), at);
                e.active = active;
            })
            .unwrap();
            std::fs::read(&file).unwrap() != before
        };
        // Someone at it, then nobody, then unsaid: every export goes out, the same.
        let mut written = Vec::new();
        let mut watched: Option<((i64, i64, bool, i64, bool), i64)> = None;
        for at in (t + 60..=t + 90 * 60).step_by(10) {
            let active = match (at - t) / 60 {
                0..=10 => Some(true),
                11..=80 => Some(false),
                _ => None,
            };
            if (at - t) % 60 == 0 && export(at, active) {
                written.push(at);
            }
            // A phone looking every seventy seconds: when it saw the entry change, as `health::know` keeps it.
            if (at - t) % 70 != 0 {
                continue;
            }
            let entry = all(&folder, &KEY).0.into_iter().find(|e| e.id == id).unwrap();
            assert!(at - entry.exported < 60, "the folder's copy from the last export: {} s at {at}", at - entry.exported);
            let state = (entry.started, entry.closed, entry.working, entry.exported, entry.left);
            if watched.is_none_or(|(last, _)| last != state) {
                watched = Some((state, at));
            }
            let peer = Peer { id: id.clone(), said: Some(entry.said()), complete: true, seen: watched.map_or(at, |(_, seen)| seen), heard: entry.exported, ..Peer::default() };
            let doubts = doubts_now(DUE, at, None, &[peer]);
            if entry.exported >= DUE + SKEW {
                assert!(doubts.is_empty(), "an export after the dose read: known at {at}: {doubts:?}");
            } else {
                assert!(matches!(doubts.as_slice(), [Doubt::Working { .. }]), "not shared since the dose: in doubt at {at}");
            }
        }
        assert_eq!(written, (1..=90).map(|n| t + n * 60).collect::<Vec<_>>(), "one writing a minute, whoever is at it");
        // Stopped without closing (a crash) after its last export: the doubt once FRESH has passed.
        let entry = all(&folder, &KEY).0.into_iter().find(|e| e.id == id).unwrap();
        let stopped = entry.exported + FRESH + 1;
        let peer = Peer { id: id.clone(), said: Some(entry.said()), complete: true, seen: entry.exported, heard: entry.exported, ..Peer::default() };
        assert!(matches!(doubts_now(DUE, stopped, None, &[peer]).as_slice(), [Doubt::Working { .. }]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The entry that says whether someone is at a computer reads on an older
    /// Sioul as before: `active` passed over (serde's way with a field it does
    /// not know), its other fields the same, so an older phone keeps its own
    /// rule (Sioul running counts) and its doses read the same. An older
    /// computer's entry, which says nothing of it, reads here as in use while
    /// its Sioul runs, as before. No field changed meaning: no new format.
    #[test]
    fn an_older_device_reads_the_entry_that_says_whether_someone_is_at_it() {
        use sioul_core::health::{FRESH, SKEW};
        /// The entry as Sioul 0.0.5 reads it: the fields it knows.
        #[derive(Deserialize)]
        struct Older {
            id: String,
            #[serde(default)]
            kind: String,
            #[serde(default)]
            started: i64,
            #[serde(default)]
            closed: i64,
            working: bool,
            #[serde(default)]
            exported: i64,
            #[serde(default)]
            wrote: Option<(u32, u64)>,
            #[serde(default)]
            doses: bool,
            #[serde(default)]
            left: bool,
        }
        let dir = scratch("older-reads");
        let (folder, state) = (dir.join("shared"), dir.join("state"));
        let id = uuid::Uuid::new_v4().to_string();
        let vault = Some((folder.as_path(), &KEY));
        change(&own_path(&state), vault, |e| {
            e.id = id.clone();
            e.kind = COMPUTER.into();
            e.doses = true;
            e.start(1_000);
            e.exported(1_060_000, Some((1, 2)), 1_061);
            e.active = Some(false);
        })
        .unwrap();
        let sealed = std::fs::read_to_string(file_of(&folder, &id)).unwrap();
        let plain = crate::share::open(&KEY, &bound(&id), sealed.trim()).unwrap();
        assert!(String::from_utf8_lossy(&plain).contains("\"active\":false"));
        let older: Older = serde_json::from_slice(&plain).expect("an older Sioul reads it");
        assert_eq!((older.id.as_str(), older.kind.as_str(), older.started, older.closed, older.working, older.exported, older.wrote, older.doses, older.left), (id.as_str(), COMPUTER, 1_000, 0, true, 1_060, Some((1, 2)), true, false));
        // Its own rule (0.0.5's `others_in_use`): Sioul running, its export fresh.
        assert!(older.working && !older.left && 1_100 - older.exported <= FRESH + SKEW, "an older phone steps as before");
        // What the doses read of it is the same, someone at it or not.
        let said = all(&folder, &KEY).0[0].said();
        change(&own_path(&state), vault, |e| e.active = Some(true)).unwrap();
        assert_eq!(all(&folder, &KEY).0[0].said(), said);
        // An older computer's entry, saying nothing of it: in use here while its Sioul runs.
        let plain = serde_json::json!({ "id": id, "kind": "computer", "started": 1_000, "working": true, "exported": 1_060 }).to_string();
        std::fs::write(file_of(&folder, &id), crate::share::seal(&KEY, &bound(&id), plain.as_bytes())).unwrap();
        let (entries, _) = all(&folder, &KEY);
        assert_eq!(entries[0].active, None);
        assert!(others_in_use(&entries, "phone", 1_100, false) && others_in_use(&entries, "phone", 1_100, true));
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

    /// The handshake (review of battery part C, the sixth and seventh
    /// reviews): a phone's word on a computer's opening travels in its own
    /// sealed entry (`Entry::acked`), read by the computer with the rest of
    /// it; a word written in the folder otherwise (a file not sealed with the
    /// key, another device's name) is no entry at all.
    /// Of two copies of a phone's entry (the folder's, the one fetched): the
    /// later by its times, the folder's on a tie; a computer either copy no
    /// longer names is withdrawn, whichever is later, while a copy naming an
    /// older opening of it takes nothing away (the re-check before 0.0.6, LOW-2).
    #[test]
    fn of_two_copies_the_later_and_a_word_both_name() {
        let copy = |exported: i64, word: Option<i64>| Entry { id: "phone".into(), kind: PHONE.into(), working: true, started: 1_000, exported, imported: exported, acked: word.map(|opening| ("desk".to_string(), opening)).into_iter().collect(), ..Entry::default() };
        // The phone's clock set back: its newest copy, dated earlier, no longer names the desk; the lagging one does.
        let picked = chosen(copy(9_000, Some(500)), copy(7_000, None));
        assert_eq!((picked.exported, picked.acked.get("desk")), (9_000, None), "withdrawn, though the copy naming it is dated later");
        // Two copies written in one second: the folder's, with the word; the fetched one, without.
        assert!(chosen(copy(9_000, Some(500)), copy(9_000, None)).acked.is_empty());
        // A lagging copy naming an older opening takes nothing away: the new word stands.
        assert_eq!(chosen(copy(7_000, Some(500)), copy(9_000, Some(800))).acked.get("desk"), Some(&800));
        assert_eq!(chosen(copy(9_000, Some(800)), copy(7_000, Some(500))).acked.get("desk"), Some(&800));
        // The cost: counted again (or paired anew) while the other copy lags without it, its word waits for that copy.
        assert!(chosen(copy(7_000, None), copy(9_000, Some(800))).acked.is_empty());
        // Both alike: the later as it is.
        assert_eq!(chosen(copy(7_000, Some(800)), copy(9_000, Some(800))).acked.get("desk"), Some(&800));
    }

    #[test]
    fn a_phones_word_on_an_opening_travels_sealed_in_its_entry() {
        let base = scratch("acknowledged");
        let folder = base.join("shared");
        std::fs::create_dir_all(&folder).unwrap();
        let desk = Device::new(&base, "desk", COMPUTER);
        let phone = Device::new(&base, "phone", PHONE);
        desk.session(&folder, |e| e.start(DUE - 30));
        let desk_id = desk.id.clone();
        phone.session(&folder, |e| {
            e.start(DUE - 3_600);
            e.acked.insert(desk_id.clone(), DUE - 30);
        });
        let read = |id: &str| all(&folder, &KEY).0.into_iter().find(|e| e.id == id);
        assert_eq!(read(&phone.id).and_then(|e| e.acked.get(&desk.id).copied()), Some(DUE - 30), "the phone's word, sealed");
        // Forged in the folder without the key: no entry read, so no word.
        let forged = Entry { id: phone.id.clone(), kind: PHONE.into(), working: true, acked: [(desk.id.clone(), i64::MAX)].into_iter().collect(), ..Entry::default() };
        std::fs::write(file_of(&folder, &phone.id), serde_json::to_vec(&forged).unwrap()).unwrap();
        assert!(read(&phone.id).is_none() && all(&folder, &KEY).1.contains(&phone.id), "unsealed: unread");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// What each device did to remind a dose (`health-reminders.toml`, review
    /// of battery part C, the fifth review): each device's entry comes whole
    /// to the others; an older Sioul, which knows no such file, takes none of
    /// it out, whatever it writes in the files it knows.
    #[test]
    fn what_each_device_did_to_remind_travels_and_an_older_sioul_leaves_it() {
        use sioul_core::dose_reminders::{DoseReminders, Tried};
        let base = scratch("reminders");
        let folder = base.join("shared");
        std::fs::create_dir_all(&folder).unwrap();
        let desk = Device::new(&base, "desk", COMPUTER);
        let phone = Device::new(&base, "phone", PHONE);
        let old = Device::new(&base, "old", PHONE);
        let old_stores = || old.stores().into_iter().filter(|s| s.name != "state/health-doses.toml" && s.name != "state/health-reminders.toml").collect::<Vec<_>>();
        let old_exchange = |now: i64| crate::share::exchange(&Sharing { folder: &folder, computer: &old.id, key: &KEY, memory: &old.memory, files: false, hurry: None }, &old_stores(), now * 1000).unwrap();
        let reminders = |device: &Device| device.roots.state.join(sioul_core::dose_reminders::FILE);
        desk.session(&folder, |e| e.start(DUE - 3_600));
        phone.session(&folder, |e| e.start(DUE - 3_600));
        desk.exchange(&folder, DUE - 3_500);
        phone.exchange(&folder, DUE - 3_450);
        old_exchange(DUE - 3_400);
        // The desk's first try did not go out, then it promised quiet past the phone's alarm.
        let promised = Tried { unshown: DUE + 5, quiet: DUE + 900, ..Tried::default() };
        DoseReminders::update(&reminders(&desk), DUE + 5, &desk.id, |all| *all.entry(DOSE, &desk.id) = promised).unwrap();
        desk.exchange(&folder, DUE + 420);
        for n in 0..3 {
            old_exchange(DUE + 430 + n);
        }
        HealthState::update(&old.marks(), DUE + 440, |s| {
            s.reminded.insert("iron@1800000000".into(), DUE + 440);
        })
        .unwrap();
        old_exchange(DUE + 450);
        phone.exchange(&folder, DUE + 600);
        assert_eq!(DoseReminders::read(&reminders(&phone)).unwrap().of(DOSE, &desk.id), Some(promised), "whole, as the desk wrote it");
        // The phone's alarm shows it: its own entry, read back on the desk.
        DoseReminders::update(&reminders(&phone), DUE + 601, &phone.id, |all| all.entry(DOSE, &phone.id).shown = DUE + 601).unwrap();
        phone.exchange(&folder, DUE + 610);
        desk.exchange(&folder, DUE + 620);
        let read = DoseReminders::read(&reminders(&desk)).unwrap();
        assert!(read.shown(DOSE) && read.of(DOSE, &desk.id) == Some(promised), "both entries, nothing taken out");
        let _ = std::fs::remove_dir_all(&base);
    }
}
