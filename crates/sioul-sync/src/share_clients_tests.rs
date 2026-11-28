// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The part "Clients' files" (docs/clients.md, "Between devices"): a
//! client's file, sealed on each disk through the store's door, travels
//! sealed between the devices that hold it; it waits while a device does not
//! know it (an older Sioul), never reaches a device whose files are not
//! sealed, and a client erased on one device is purged on the others and
//! never comes back. Fiction only.

use super::lists_tests::says_every_store;
use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;
use sioul_core::clients::door::{Door, Sealed};
use sioul_core::clients::template::Settings;
use sioul_core::clients::{file, keep, record};
use sioul_core::sealed::{Folders, Options, RootKey, Store, files};
use std::sync::Arc;

/// An exchange of `c` with `notes` as its notes folder, every part shared (`PARTS`).
fn exchange_in(c: &Computer, folder: &Path, key: &[u8; 32], now: i64, notes: &Path) -> Outcome {
    let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
    let stores = stores_of(&config, &c.roots, &|part| PARTS.contains(&part));
    exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: true, hurry: None }, &stores, now).unwrap()
}

/// A device's sealed entry saying what its Sioul does with clients' files:
/// 0 an older Sioul, `CLIENT_FILES_SHARED` it knows them, `CLIENT_FILES_HELD` it holds them.
fn says_client_files(c: &Computer, folder: &Path, key: &[u8; 32], value: u32) {
    crate::devices::change(&crate::devices::own_path(&c.roots.state), Some((folder, key)), |e| e.client_files = value).unwrap();
}

/// A device whose files are sealed: its store over its own folders, reached
/// through their paths as the window reaches its profile's, and the clients'
/// door on it.
fn sealed(base: &Path, name: &str, byte: u8) -> (files::Mounted, Sealed) {
    let store = Arc::new(Store::create(&Folders::under(&base.join(name)), &RootKey::from_bytes([byte; 32]), Options::default()).unwrap());
    (files::mount(store.clone()), Sealed::new(store))
}

/// Every plain file under `dir`, by its path below it: what a disk shows without the session.
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

/// The first plain file under `dir` that holds one of `words`.
fn says_any(dir: &Path, words: &[&str]) -> Option<String> {
    plain_files(dir).into_iter().find(|f| {
        let bytes = std::fs::read(dir.join(f)).unwrap_or_default();
        words.iter().any(|w| bytes.windows(w.len()).any(|x| x == w.as_bytes()))
    })
}

/// A client with a file: its record, a description and an attachment.
fn a_client_with_a_file(door: &dyn Door) -> String {
    let settings = Settings::default();
    let client = record::make(door, "Studio Exemple", &[], record::Billing::default()).unwrap();
    file::write_description(door, &client.id, "Prefers mornings. Allergic to latex.", None, &settings).unwrap();
    file::attach(door, &client.id, "Consent.pdf", "", "", "application/pdf", b"%PDF-1.7 consent signed", &settings).unwrap();
    client.id
}

/// The gates: the part travels once every device knows it; drafts and
/// profiles move once every device holds it; both stay open for good.
#[test]
fn the_gates_wait_for_every_device_then_stay_open() {
    let base = scratch("client-files-gate");
    let folder = base.join("Sync").join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (_desk_mount, _) = sealed(&base, "desk", 3);
    let (_laptop_mount, _) = sealed(&base, "laptop", 4);
    let notes = base.join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    says_every_store(&desk, &folder, &key);
    says_every_store(&laptop, &folder, &key);
    says_client_files(&desk, &folder, &key, CLIENT_FILES_HELD);
    let alone = exchange_in(&desk, &folder, &key, NOW, &notes);
    assert!(!alone.client_files_open && !alone.client_files_held, "no other device yet");
    exchange_in(&laptop, &folder, &key, NOW + MINUTE, &notes);
    let older = exchange_in(&desk, &folder, &key, NOW + 2 * MINUTE, &notes);
    assert!(!older.client_files_open, "the laptop runs a Sioul that does not know them");
    says_client_files(&laptop, &folder, &key, CLIENT_FILES_SHARED);
    exchange_in(&laptop, &folder, &key, NOW + 3 * MINUTE, &notes);
    let knows = exchange_in(&desk, &folder, &key, NOW + 4 * MINUTE, &notes);
    assert!(knows.client_files_open && !knows.client_files_held, "known everywhere, held by the desk alone: they travel, nothing moves");
    assert!(!knows.client_files_elsewhere, "no other device holds them: two copies here (the twin)");
    says_client_files(&laptop, &folder, &key, CLIENT_FILES_HELD);
    exchange_in(&laptop, &folder, &key, NOW + 5 * MINUTE, &notes);
    let held = exchange_in(&desk, &folder, &key, NOW + 6 * MINUTE, &notes);
    assert!(held.client_files_open && held.client_files_held, "held everywhere");
    assert!(held.client_files_elsewhere, "another device holds them: one copy here");
    says_client_files(&laptop, &folder, &key, 0);
    exchange_in(&laptop, &folder, &key, NOW + 7 * MINUTE, &notes);
    let after = exchange_in(&desk, &folder, &key, NOW + 8 * MINUTE, &notes);
    assert!(after.client_files_open && after.client_files_held, "open for good");
    assert!(client_files_held_in(&desk.memory, &desk.id), "read at the window's start");
    let _ = std::fs::remove_dir_all(&base);
}

/// A client's file made on one sealed device reaches the other, sealed on
/// both disks and in the sharing folder, and a change comes back; erased on
/// one, it is purged on the other and never comes back, even when an older
/// copy is written there again.
#[test]
fn a_client_s_file_travels_sealed_and_an_erase_reaches_every_device() {
    let base = scratch("client-files-travel");
    let folder = base.join("Sync").join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (_desk_mount, desk_door) = sealed(&base, "desk", 5);
    let (_laptop_mount, laptop_door) = sealed(&base, "laptop", 6);
    let (desk_notes, laptop_notes) = (base.join("desk-notes"), base.join("laptop-notes"));
    std::fs::create_dir_all(&desk_notes).unwrap();
    std::fs::create_dir_all(&laptop_notes).unwrap();
    for c in [&desk, &laptop] {
        says_every_store(c, &folder, &key);
        says_client_files(c, &folder, &key, CLIENT_FILES_HELD);
    }
    let both = [(&desk, desk_notes.as_path()), (&laptop, laptop_notes.as_path())];
    let rounds = |mut now: i64, n: usize| {
        for _ in 0..n {
            for (computer, notes) in both {
                let outcome = exchange_in(computer, &folder, &key, now, notes);
                eprintln!("sent {} received {} pending {} open {} held {} problems {:?} waits {:?}", outcome.sent, outcome.received, outcome.pending, outcome.client_files_open, outcome.client_files_held, outcome.problems, outcome.waits);
                now += MINUTE;
            }
        }
        now
    };
    let id = a_client_with_a_file(&desk_door);
    let now = rounds(NOW, 3);
    // On the laptop, through its own door: the same client, its description and its attachment.
    let there = record::list(&laptop_door).unwrap().items;
    assert_eq!(there.iter().map(|c| (c.id.as_str(), c.name.as_str())).collect::<Vec<_>>(), vec![(id.as_str(), "Studio Exemple")]);
    assert_eq!(file::description(&laptop_door, &id).unwrap().unwrap().text, "Prefers mornings. Allergic to latex.");
    let attached = file::attachments(&laptop_door, &id).unwrap().items;
    assert_eq!(file::attachment_bytes(&laptop_door, &id, &attached[0].id).unwrap(), b"%PDF-1.7 consent signed");
    // Sealed everywhere: no client's word in clear on either disk nor in the sharing folder.
    let words = ["Studio Exemple", "latex", "Consent.pdf", "consent signed", id.as_str()];
    for dir in [base.join("desk"), base.join("laptop"), folder.clone()] {
        assert_eq!(says_any(&dir, &words), None, "a client's words in clear under {}", dir.display());
    }
    assert!(!plain_files(&base.join("desk")).iter().any(|f| f.contains("clients")), "no clients' folder in clear on the desk");
    // A change on the laptop comes back.
    file::write_description(&laptop_door, &id, "Prefers afternoons now.", None, &Settings::default()).unwrap();
    let now = rounds(now, 3);
    assert_eq!(file::description(&desk_door, &id).unwrap().unwrap().text, "Prefers afternoons now.");
    // The record as the laptop holds it before the erase, as a backup or an older copy would bring it back.
    let old_record = laptop_door.read(&record::record_path(&id)).unwrap().unwrap().bytes;
    // Erased on the desk: everything of it purged there, its mark travels.
    keep::erase(&desk_door, &id, &Settings::default(), &Default::default(), "2026-11-28".parse().unwrap(), None).unwrap();
    assert!(record::list(&desk_door).unwrap().items.is_empty());
    let now = rounds(now, 3);
    // On the laptop the move's pass reads the mark (`moving::run` calls it): nothing of it left.
    assert_eq!(keep::apply_erased(&laptop_door).unwrap(), vec![id.clone()]);
    assert!(record::list(&laptop_door).unwrap().items.is_empty(), "erased on every device");
    // And what the laptop's sharing kept of its files as earlier versions (one kept here as an exchange keeps it).
    let history = crate::history::root(&laptop.memory);
    let before = base.join("an-earlier-description.md");
    std::fs::write(&before, "Prefers mornings.").unwrap();
    crate::history::keep(&history, CLIENT_FILES_PART, &format!("{CLIENT_FILES_STORE}{id}/description.md"), &before, now).unwrap();
    assert!(crate::history::files(&history, CLIENT_FILES_PART).iter().any(|(f, _)| f.contains(&id)), "an earlier version kept");
    forget_erased_client(&laptop.memory, &id).unwrap();
    assert!(!crate::history::files(&history, CLIENT_FILES_PART).iter().any(|(f, _)| f.contains(&id)), "nothing of it kept");
    assert!(file::description(&laptop_door, &id).unwrap().is_none());
    // An older copy written there again is purged again, and never travels back as the client.
    laptop_door.write(&record::record_path(&id), &old_record, sioul_core::clients::door::Expect::Any).unwrap();
    assert_eq!(keep::apply_erased(&laptop_door).unwrap(), vec![id.clone()]);
    rounds(now, 2);
    keep::apply_erased(&desk_door).unwrap();
    assert!(record::list(&desk_door).unwrap().items.is_empty() && record::list(&laptop_door).unwrap().items.is_empty(), "nothing resurrected");
    assert!(record::make(&desk_door, "Studio Exemple", &[], record::Billing::default()).is_ok_and(|c| c.id != id), "a client made again takes an id of its own");
    let _ = std::fs::remove_dir_all(&base);
}

/// A device on a Sioul that does not know clients' files (as main before
/// them, 608d8521) beside one that holds them: nothing of a client is written
/// in the sharing folder meanwhile, so the older device keeps nothing waiting
/// and loses nothing; the client stays whole where it was made; once the
/// older device is updated, the file travels to it. A device whose files are
/// not sealed (a phone today) never takes the part in.
#[test]
fn an_older_device_holds_the_part_shut_and_a_device_in_clear_never_takes_it() {
    let base = scratch("client-files-older");
    let folder = base.join("Sync").join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"), Computer::new(&base, "phone"));
    let (_desk_mount, desk_door) = sealed(&base, "desk", 7);
    let (_laptop_mount, laptop_door) = sealed(&base, "laptop", 8);
    let notes = |name: &str| {
        let dir = base.join(format!("{name}-notes"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    };
    let (desk_notes, laptop_notes, phone_notes) = (notes("desk"), notes("laptop"), notes("phone"));
    for c in [&desk, &laptop, &phone] {
        says_every_store(c, &folder, &key);
    }
    says_client_files(&desk, &folder, &key, CLIENT_FILES_HELD);
    // The laptop's entry says nothing of clients' files: an older Sioul's.
    says_client_files(&laptop, &folder, &key, 0);
    says_client_files(&phone, &folder, &key, CLIENT_FILES_SHARED);
    let all = [(&desk, desk_notes.as_path()), (&laptop, laptop_notes.as_path()), (&phone, phone_notes.as_path())];
    // The laptop runs main before clients' files (608d8521) until it is updated: its
    // engine knows no store `files/clients/` (`tests::OLDER`: a record of it would be
    // kept waiting at every exchange) and its exchanges carry no such store.
    let older = std::cell::Cell::new(true);
    let as_older = |c: &Computer, notes: &Path, now: i64| -> Outcome {
        let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
        let stores: Vec<super::Store> = stores_of(&config, &c.roots, &|part| PARTS.contains(&part)).into_iter().filter(|s| s.name != CLIENT_FILES_STORE).collect();
        super::tests::OLDER.with(|o| *o.borrow_mut() = vec![CLIENT_FILES_STORE.to_string()]);
        let outcome = exchange(&Sharing { folder: &folder, computer: &c.id, key: &key, memory: &c.memory, files: true, hurry: None }, &stores, now).unwrap();
        super::tests::OLDER.with(|o| o.borrow_mut().clear());
        outcome
    };
    let rounds = |mut now: i64, n: usize| {
        let mut last = Vec::new();
        for _ in 0..n {
            last.clear();
            for (computer, notes) in all {
                last.push(if std::ptr::eq(computer, &laptop) && older.get() { as_older(computer, notes, now) } else { exchange_in(computer, &folder, &key, now, notes) });
                now += MINUTE;
            }
        }
        (now, last)
    };
    let id = a_client_with_a_file(&desk_door);
    let (now, last) = rounds(NOW, 3);
    assert!(!last[0].client_files_open, "the older device holds the part shut");
    assert_eq!(last[1].pending, 0, "the older device keeps nothing waiting: {:?}", last[1]);
    // Nothing of a client in the folder: the older device's memory keeps nothing waiting, nothing of the part is known there.
    let older_memory = Memory::load(&laptop.memory, &laptop.id);
    assert!(!older_memory.waiting.keys().chain(older_memory.pending.keys()).chain(older_memory.entries.keys()).any(|k| k.starts_with(CLIENT_FILES_STORE)), "nothing of a client reached the older device");
    assert!(record::list(&laptop_door).unwrap().items.is_empty());
    // Nothing lost where it was made.
    assert_eq!(file::description(&desk_door, &id).unwrap().unwrap().text, "Prefers mornings. Allergic to latex.");
    // The older device updated: it knows and holds them now; the file travels to it, never to the phone.
    older.set(false);
    says_client_files(&laptop, &folder, &key, CLIENT_FILES_HELD);
    let (_, last) = rounds(now, 3);
    assert!(last[0].client_files_open && !last[0].client_files_held, "the phone knows them but does not hold them: nothing moves");
    assert_eq!(record::list(&laptop_door).unwrap().items.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), vec![id.clone()]);
    assert_eq!(file::description(&laptop_door, &id).unwrap().unwrap().text, "Prefers mornings. Allergic to latex.");
    assert!(!phone.path("data/clients").exists() && says_any(&base.join("phone"), &["Studio Exemple", "latex"]).is_none(), "nothing of a client on the device in clear");
    let _ = std::fs::remove_dir_all(&base);
}
