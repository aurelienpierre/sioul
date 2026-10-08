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
    /// It stopped sharing: it counts no more.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub left: bool,
    /// Its size changed at each writing (`share::pad_for`): read by nobody.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pad: String,
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
fn written(entry: &Entry) -> (i64, i64, i64, i64, i64) {
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

/// This device's own entry changed by `change`, under its lock, read just
/// before: written here, then, sharing on (`vault`: the folder and its key),
/// into the folder. Returns it as written.
pub fn change(path: &Path, vault: Option<(&Path, &[u8; 32])>, change: impl FnOnce(&mut Entry)) -> Result<Entry, String> {
    sioul_core::filelock::with_lock(path, || {
        let mut entry = own(path);
        change(&mut entry);
        entry.pad.clear();
        let text = toml::to_string(&entry).map_err(|e| e.to_string())?;
        crate::share::write_atomically(path, format!("# This device, as it tells your other devices (docs/database.md, \"Devices\").\n{text}").as_bytes())?;
        if let Some((folder, key)) = vault
            && !entry.id.is_empty()
        {
            publish(folder, key, &entry)?;
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
