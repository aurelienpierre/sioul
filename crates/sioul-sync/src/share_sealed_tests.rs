// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A device whose files are sealed (the sealed store, docs/store.md) beside a
//! device whose files are not (a Sioul before the store, or a phone, where it
//! is not turned on yet), sharing through the same folder: who reads what,
//! and nothing lost. Invented data only.

use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;
use sioul_core::sealed::{Folders, Options, RootKey, Store, files};
use std::sync::Arc;

/// Every plain file under `dir`, by its path below it (what a disk shows to someone without the session).
fn plain_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(entry.path());
            } else {
                out.push(entry.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

fn sessions(computer: &Computer) -> Vec<(String, i64, u32)> {
    let mut out: Vec<(String, i64, u32)> = sioul_core::timelog::sessions_in(&computer.path("data/time")).into_iter().map(|s| (s.project, s.start, s.minutes)).collect();
    out.sort();
    out
}

#[test]
fn a_sealed_device_and_one_in_clear_share_everything_and_lose_nothing() {
    let base = scratch("sealed-beside-plain");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    std::fs::create_dir_all(&desk_notes).unwrap();
    std::fs::create_dir_all(&phone_notes).unwrap();
    // The desk's own folders sealed, reached through their paths; the phone's in clear, as before the store.
    let store = Arc::new(Store::create(&Folders::under(&base.join("desk")), &RootKey::from_bytes([7; 32]), Options::default()).unwrap());
    let _mounted = files::mount(store.clone());
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let rounds = |mut now: i64, n: usize| {
        for _ in 0..n {
            for (computer, notes) in both {
                let outcome = computer.exchange_notes(&folder, &key, now, notes);
                // What each exchange did, said when the test fails.
                eprintln!("{}: sent {} received {} pending {} problems {:?} waits {:?}", if std::ptr::eq(computer, &desk) { "desk" } else { "phone" }, outcome.sent, outcome.received, outcome.pending, outcome.problems, outcome.waits);
                now += MINUTE;
            }
        }
        now
    };
    // Each device records time of its own, and a note in its notes folder.
    let start = 1_789_912_800;
    sioul_core::timelog::record_in(&desk.path("data/time"), &sioul_core::timelog::Session { project: "garden".into(), start, minutes: 30, ..Default::default() }).unwrap();
    sioul_core::timelog::record_in(&phone.path("data/time"), &sioul_core::timelog::Session { project: "orchard".into(), start: start + 3600, minutes: 45, ..Default::default() }).unwrap();
    files::write(desk_notes.join("Loyer.md"), "# Loyer\n").unwrap();
    std::fs::write(phone_notes.join("Mutuelle.md"), "# Mutuelle\n").unwrap();
    let now = rounds(NOW, 3);
    // Nothing lost: each device holds both sessions, whatever it seals.
    let want = vec![("garden".to_string(), start, 30), ("orchard".to_string(), start + 3600, 45)];
    assert_eq!(sessions(&desk), want, "the sealed device reads what the other wrote");
    assert_eq!(sessions(&phone), want, "the device in clear reads what the sealed one wrote");
    // A change on the sealed device reaches the other, and back.
    let key_of = |computer: &Computer| sioul_core::timelog::sessions_in(&computer.path("data/time")).into_iter().find(|s| s.project == "garden").unwrap().key();
    sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.minutes = 50).unwrap();
    rounds(now, 3);
    assert_eq!(sessions(&phone)[0], ("garden".to_string(), start, 50));
    assert_eq!(sessions(&desk), sessions(&phone));
    // Who reads what. On the sealed device's disk: no file of its own in clear,
    // nothing that names a project, only sealed blobs, the store's own file and locks.
    for file in plain_files(&base.join("desk")) {
        assert!(file.contains("/sealed/") || file.ends_with("store.toml") || file.ends_with(".lock"), "{file} is in clear on the sealed device");
        if !file.ends_with("store.toml") {
            let bytes = std::fs::read(base.join("desk").join(&file)).unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("garden") && !String::from_utf8_lossy(&bytes).contains("orchard"), "{file} names a project");
        }
    }
    // On the device in clear: its files, as before, plain.
    assert!(plain_files(&base.join("phone")).iter().any(|f| f.starts_with("data/time/")), "{:?}", plain_files(&base.join("phone")));
    // The notes stay in clear on both, outside Sioul's folders, for every other program.
    assert_eq!(std::fs::read_to_string(desk_notes.join("Loyer.md")).unwrap(), "# Loyer\n");
    assert_eq!(std::fs::read_to_string(phone_notes.join("Mutuelle.md")).unwrap(), "# Mutuelle\n");
    // The sharing folder: sealed records, as between any two devices; no project's name in clear.
    for file in plain_files(&folder) {
        let bytes = std::fs::read(folder.join(&file)).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("orchard"), "{file} in the sharing folder names a project");
    }
    // The sealed device's files written out in clear: the same sessions, in the same layout.
    let out = base.join("export");
    let (written, damaged) = files::export(&store, &out, &|_| true).unwrap();
    assert!(written > 0 && damaged.is_empty());
    assert!(plain_files(&out).iter().any(|f| f.starts_with("data/time/")), "{:?}", plain_files(&out));
    let _ = std::fs::remove_dir_all(&base);
}
