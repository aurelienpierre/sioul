// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sealed store's tests: invented data only, in folders of their own, the fixed test key.

use super::disk::{CRASH, FULL_AFTER, ROOM, Step};
use super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("sioul-sealed-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn made(name: &str) -> (PathBuf, Store) {
    let base = scratch(name);
    let store = Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap();
    (base, store)
}

fn again(base: &Path) -> Store {
    Store::open(&Folders::under(base), &RootKey::test(), Options::default()).unwrap()
}

fn p(path: &str) -> LogicalPath {
    LogicalPath::new(path).unwrap()
}

fn text(store: &Store, path: &str) -> Option<String> {
    store.read_text(&p(path)).unwrap().map(|(text, _)| text)
}

/// Bytes that do not compress: as large sealed as plain.
fn noise(n: usize, seed: u32) -> Vec<u8> {
    let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

/// Every file under `base`, with its bytes: what a refused open must leave as it was.
fn snapshot(base: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
            if entry.file_type().unwrap().is_dir() {
                stack.push(entry.path());
            } else {
                out.push((entry.path(), std::fs::read(entry.path()).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// The blob of a file's copy on the disk.
fn blob(store: &Store, path: &str, generation: u64, copy: u8) -> PathBuf {
    store.blob_place(&p(path), format::Role::File, generation, copy)
}

#[test]
fn written_read_listed_and_nothing_plain_on_the_disk() {
    let (base, store) = made("basics");
    assert_eq!(store.read(&p("config/config.toml")).unwrap(), None, "never written: absent, said as such");
    let written = store.write(&p("config/config.toml"), b"[store]\nversions = 2\n# Lise Martin\n", How::default()).unwrap();
    assert_eq!(written.generation, 1);
    store.write(&p("data/mail/acc/INBOX/1700000000.M1.host"), b"Subject: Lettre de la mutuelle\n\nBonjour", How::default()).unwrap();
    assert_eq!(text(&store, "config/config.toml").unwrap(), "[store]\nversions = 2\n# Lise Martin\n");
    let listed = store.list(&p("data/mail/acc/INBOX")).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path.as_str(), "data/mail/acc/INBOX/1700000000.M1.host");
    assert_eq!(store.folders(&p("data")).unwrap().iter().map(LogicalPath::as_str).collect::<Vec<_>>(), ["data/mail", "data/mail/acc", "data/mail/acc/INBOX"]);
    assert!(store.list(&p("data/mail/other")).unwrap().is_empty(), "a folder never written: empty");
    // Nothing on the disk says a name, a word, a folder.
    for (path, bytes) in snapshot(&base) {
        let name = path.to_string_lossy().to_string();
        for word in ["INBOX", "config.toml", "mutuelle", "Lise"] {
            assert!(!name.contains(word), "{name}");
            assert!(!bytes.windows(word.len()).any(|w| w == word.as_bytes()), "{word} plain in {name}");
        }
    }
    drop(store);
    let store = again(&base);
    assert_eq!(text(&store, "config/config.toml").unwrap(), "[store]\nversions = 2\n# Lise Martin\n");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_wrong_key_is_told_and_nothing_is_touched() {
    let (base, store) = made("wrong-key");
    store.write(&p("config/config.toml"), b"a = 1\n", How::default()).unwrap();
    drop(store);
    let before = snapshot(&base);
    let refused = Store::open(&Folders::under(&base), &RootKey::from_bytes([9u8; 32]), Options::default());
    assert_eq!(refused.unwrap_err(), Error::WrongKey);
    assert_eq!(snapshot(&base), before, "nothing read as damage, nothing written");
    let empty = scratch("no-store");
    assert_eq!(Store::open(&Folders::under(&empty), &RootKey::test(), Options::default()).unwrap_err(), Error::NotCreated);
    assert_eq!(Store::create(&Folders::under(&base), &RootKey::test(), Options::default()).unwrap_err(), Error::Exists);
    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&empty);
}

#[test]
fn not_open_is_its_own_answer() {
    uninstall();
    assert_eq!(opened().unwrap_err(), Error::NotOpen);
}

#[test]
fn a_power_cut_at_every_step_leaves_the_old_content_or_the_new() {
    for step in [Step::BeforeFlush, Step::BeforeRename, Step::AfterRename, Step::BeforeJournal, Step::AfterJournal] {
        let (base, store) = made(&format!("cut-{step:?}"));
        let path = p("state/porch.toml");
        store.write(&path, b"old", How::default()).unwrap();
        CRASH.with(|c| c.set(Some((step, 0))));
        assert!(store.write(&path, b"new", How::default()).is_err(), "{step:?}");
        CRASH.with(|c| c.set(None));
        drop(store);
        let store = again(&base);
        let expected = if step == Step::AfterJournal { "new" } else { "old" };
        assert_eq!(text(&store, "state/porch.toml").unwrap(), expected, "{step:?}");
        // The store goes on: the next write, and a check that finds nothing lost.
        store.write(&path, b"after", How::default()).unwrap();
        assert_eq!(text(&store, "state/porch.toml").unwrap(), "after");
        let report = store.check(true).unwrap();
        assert!(report.folders.iter().all(|f| f.lost.is_empty() && f.whole == f.files), "{step:?}: {report:?}");
        assert_eq!(store.check(false).unwrap().unlisted, 0, "{step:?}: the interrupted write's blob taken out");
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[test]
fn a_power_cut_while_a_list_is_folded_loses_nothing() {
    for step in [Step::BetweenManifests, Step::BeforeNewJournal] {
        let (base, store) = made(&format!("fold-{step:?}"));
        let mut last: BTreeMap<String, u64> = BTreeMap::new();
        // Up to the change that folds: the cut stops the fold, never the change.
        for i in 0..manifest::FOLD_LINES - 1 {
            let path = format!("config/f{}.toml", i % 7);
            store.write(&p(&path), format!("{i}").as_bytes(), How::default()).unwrap();
            last.insert(path, i);
        }
        CRASH.with(|c| c.set(Some((step, 0))));
        for i in manifest::FOLD_LINES - 1..manifest::FOLD_LINES + 40 {
            let path = format!("config/f{}.toml", i % 7);
            store.write(&p(&path), format!("{i}").as_bytes(), How::default()).unwrap();
            last.insert(path, i);
        }
        assert_eq!(CRASH.with(std::cell::Cell::get), None, "{step:?}: the cut happened");
        drop(store);
        let store = again(&base);
        for (path, i) in &last {
            assert_eq!(text(&store, path).unwrap(), i.to_string(), "{step:?}: {path}");
        }
        // And the same with the list's first copy damaged: its second and the journal agree.
        let folder = p("config");
        std::fs::write(store.manifest_place(&folder, 0), b"broken").unwrap();
        let store = again(&base);
        for (path, i) in &last {
            assert_eq!(text(&store, path).unwrap(), i.to_string(), "{step:?}, first copy damaged: {path}");
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[test]
fn the_disk_full_keeps_the_old_file_and_versions_go_first() {
    let (base, store) = made("full");
    let alone = p("data/records/health.toml");
    store.write(&alone, b"before", How::default()).unwrap();
    // A disk really full (the system says so while the blob is written): refused, the old content intact.
    FULL_AFTER.with(|f| f.set(Some(100)));
    assert_eq!(store.write(&alone, &noise(5000, 1), How::default()).unwrap_err(), Error::NoRoom);
    FULL_AFTER.with(|f| f.set(None));
    assert_eq!(text(&store, "data/records/health.toml").unwrap(), "before", "the old content intact");
    // Short of the room kept free, but not full: written all the same (review-finish F4), as the system's files are.
    ROOM.with(|r| r.set(Some(10)));
    store.write(&alone, b"after", How::default()).unwrap();
    ROOM.with(|r| r.set(None));
    assert_eq!(text(&store, "data/records/health.toml").unwrap(), "after");
    // With earlier versions about: the oldest goes, and the write is made.
    let path = p("data/records/budgets.toml");
    for seed in 1..=3 {
        store.write(&path, &noise(5000, seed), How::default()).unwrap();
    }
    ROOM.with(|r| r.set(Some(100)));
    let written = store.write(&path, &noise(5000, 4), How::default());
    ROOM.with(|r| r.set(None));
    assert!(written.is_ok(), "{written:?}");
    assert_eq!(store.read(&path).unwrap().unwrap().bytes, noise(5000, 4));
    // Generations are counted per folder: health.toml took 1 and 2, budgets.toml 3, 4 and 5.
    assert_eq!(store.read_version(&path, 3).unwrap_err(), Error::Absent, "the oldest version went first");
    assert_eq!(store.read_version(&path, 4).unwrap().bytes, noise(5000, 2));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_flipped_bit_reads_the_newest_good_version_said_once() {
    let (base, store) = made("bit");
    let path = "state/share/memory.toml";
    for n in 1..=3 {
        store.write(&p(path), format!("version {n}\n").repeat(100).as_bytes(), How::default()).unwrap();
    }
    let place = blob(&store, path, 3, 0);
    let mut bytes = std::fs::read(&place).unwrap();
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0x10;
    std::fs::write(&place, &bytes).unwrap();
    let read = store.read(&p(path)).unwrap().unwrap();
    assert_eq!(read.bytes, "version 2\n".repeat(100).into_bytes());
    assert!(matches!(read.from, Source::Earlier { generation: 2, .. }));
    assert_eq!(store.take_notices().len(), 1);
    store.read(&p(path)).unwrap();
    assert!(store.take_notices().is_empty(), "said once");
    assert!(store.stat(&p(path)).unwrap().unwrap().damaged);
    // The check says it; the repair puts the newest good version back, the damaged copy set aside.
    let report = store.check(false).unwrap();
    assert_eq!(report.folders.iter().map(|f| f.thin.len()).sum::<usize>(), 1, "{report:?}");
    let report = store.check(true).unwrap();
    assert_eq!(report.folders.iter().map(|f| f.mended.len()).sum::<usize>(), 1, "{report:?}");
    let read = store.read(&p(path)).unwrap().unwrap();
    assert_eq!((read.from, read.bytes), (Source::Current, "version 2\n".repeat(100).into_bytes()));
    let report = store.check(false).unwrap();
    assert!(report.folders.iter().all(|f| f.thin.is_empty() && f.lost.is_empty()), "{report:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_file_with_no_good_copy_is_damaged_never_empty() {
    let (base, store) = made("lost");
    let path = "state/porch.toml";
    store.write(&p(path), b"done = 1", How::default()).unwrap();
    std::fs::write(blob(&store, path, 1, 0), b"garbage").unwrap();
    assert!(matches!(store.read(&p(path)), Err(Error::Damaged { .. })));
    let changed = store.update(&p(path), How::default(), &mut |old| Ok(Some(old.unwrap_or_default().to_vec())));
    assert!(matches!(changed, Err(Error::Damaged { .. })), "a change is never given nothing for a damaged file: {changed:?}");
    std::fs::remove_file(blob(&store, path, 1, 0)).unwrap();
    assert!(matches!(store.read(&p(path)), Err(Error::Damaged { .. })), "a blob gone is damage too");
    let report = store.check(false).unwrap();
    assert_eq!(report.folders.iter().map(|f| f.lost.len()).sum::<usize>(), 1, "{report:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_twin_mends_a_damaged_current_copy_and_goes_when_released() {
    let (base, store) = made("twin");
    let path = "data/drafts/reply.eml";
    let written = store.write(&p(path), b"Subject: Re: devis\n\nMerci.", How::only_here()).unwrap();
    assert!(store.stat(&p(path)).unwrap().unwrap().twin);
    // A twin is sealed apart: its own hash in the record. Whole for the check, and never set aside by a repair.
    let report = store.check(false).unwrap();
    assert!(report.folders.iter().all(|f| f.whole == f.files && f.thin.is_empty()), "{report:?}");
    store.check(true).unwrap();
    assert!(blob(&store, path, written.generation, 1).exists(), "the twin stays where it is");
    std::fs::write(blob(&store, path, written.generation, 0), b"").unwrap();
    let read = store.read(&p(path)).unwrap().unwrap();
    assert_eq!((read.from, read.bytes.as_slice()), (Source::Twin, &b"Subject: Re: devis\n\nMerci."[..]));
    let report = store.check(true).unwrap();
    assert!(report.folders.iter().any(|f| f.mended.iter().any(|(_, from)| *from == MendedFrom::SecondCopy)), "{report:?}");
    assert_eq!(store.read(&p(path)).unwrap().unwrap().from, Source::Current);
    // Travelled: the twin goes; a release of another generation changes nothing.
    store.released(&p(path), written.generation + 5).unwrap();
    assert!(store.stat(&p(path)).unwrap().unwrap().twin);
    store.released(&p(path), written.generation).unwrap();
    assert!(!store.stat(&p(path)).unwrap().unwrap().twin);
    assert!(!blob(&store, path, written.generation, 1).exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_lost_list_is_made_again_from_the_blobs() {
    let (base, store) = made("rebuilt");
    for i in 0..20 {
        store.write(&p(&format!("data/calendars/perso/{i}.ics")), format!("BEGIN:VEVENT {i}").as_bytes(), How::default()).unwrap();
    }
    store.write(&p("data/calendars/perso/3.ics"), b"BEGIN:VEVENT 3 again", How::default()).unwrap();
    store.set_attrs(&p("data/calendars/perso/4.ics"), &[("flags", Some("S"))]).unwrap();
    store.remove(&p("data/calendars/perso/5.ics"), Expect::Any).unwrap();
    let folder = p("data/calendars/perso");
    // Under 512 changes the folder has a journal and no manifest yet: whatever is there goes.
    for place in [store.manifest_place(&folder, 0), store.manifest_place(&folder, 1), store.journal_place(&folder)] {
        let _ = std::fs::remove_file(place);
    }
    drop(store);
    let store = again(&base);
    assert_eq!(store.list(&folder).unwrap().len(), 20, "the removed one comes back: nothing is left that says it was removed");
    assert_eq!(text(&store, "data/calendars/perso/3.ics").unwrap(), "BEGIN:VEVENT 3 again");
    assert_eq!(store.versions(&p("data/calendars/perso/3.ics")).unwrap().len(), 2);
    assert!(store.take_notices().iter().any(|n| matches!(n, Notice::Rebuilt { attrs_lost: true, .. })), "said once, attributes lost said");
    // Both copies of a list damaged, its journal whole: made again, what the journal says kept.
    store.set_attrs(&p("data/calendars/perso/4.ics"), &[("flags", Some("RS"))]).unwrap();
    store.remove(&p("data/calendars/perso/5.ics"), Expect::Any).unwrap();
    for copy in [0u8, 1] {
        std::fs::write(store.manifest_place(&folder, copy), b"sioul-sealed\n broken").unwrap();
    }
    drop(store);
    let store = again(&base);
    assert_eq!(store.list(&folder).unwrap().len(), 19);
    assert_eq!(store.stat(&p("data/calendars/perso/4.ics")).unwrap().unwrap().attrs.get("flags").map(String::as_str), Some("RS"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_blob_under_another_name_or_an_older_generation_does_not_open() {
    let (base, store) = made("swapped");
    let one = store.write(&p("config/one.toml"), b"one", How::default()).unwrap().generation;
    store.write(&p("config/two.toml"), b"two", How::default()).unwrap();
    let two = store.write(&p("config/two.toml"), b"two again", How::default()).unwrap().generation;
    // Another file's blob put in place of this one's: refused, its earlier version read.
    std::fs::copy(blob(&store, "config/one.toml", one, 0), blob(&store, "config/two.toml", two, 0)).unwrap();
    let read = store.read(&p("config/two.toml")).unwrap().unwrap();
    assert_eq!(read.bytes, b"two");
    assert!(matches!(read.from, Source::Earlier { .. }));
    // An older generation put back in place of the newer: refused there.
    let again = store.write(&p("config/one.toml"), b"one again", How::default()).unwrap().generation;
    std::fs::copy(blob(&store, "config/one.toml", one, 0), blob(&store, "config/one.toml", again, 0)).unwrap();
    let read = store.read(&p("config/one.toml")).unwrap().unwrap();
    assert!(matches!(read.from, Source::Earlier { generation, .. } if generation == one), "{:?}", read.from);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn two_processes_write_one_folder() {
    let (base, store) = made("two-writers");
    drop(store);
    let writers: Vec<_> = (0..2)
        .map(|w| {
            let base = base.clone();
            std::thread::spawn(move || {
                let store = again(&base);
                for i in 0..120 {
                    store.write(&p(&format!("state/reminded/{w}-{}", i % 10)), format!("{w} {i}").as_bytes(), How::default()).unwrap();
                    store.write(&p("state/reminded/shared"), format!("{w} {i}").as_bytes(), How::default()).unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let store = again(&base);
    assert_eq!(store.list(&p("state/reminded")).unwrap().len(), 21);
    for w in 0..2 {
        for i in 110..120 {
            assert_eq!(text(&store, &format!("state/reminded/{w}-{}", i % 10)).unwrap(), format!("{w} {i}"));
        }
    }
    // Each write a generation of its own in the folder, none given twice, none lost: 480 in all.
    let generations: Vec<u64> = store.list(&p("state/reminded")).unwrap().iter().map(|e| e.generation).collect();
    assert!(generations.iter().all(|g| *g <= 480));
    assert_eq!(generations.iter().collect::<std::collections::BTreeSet<_>>().len(), generations.len());
    let shared = text(&store, "state/reminded/shared").unwrap();
    assert!(shared == "0 119" || shared == "1 119", "{shared}");
    let report = store.check(false).unwrap();
    assert!(report.folders.iter().all(|f| f.whole == f.files), "{report:?}");
    assert_eq!(report.unlisted, 0, "{report:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_listing_while_another_process_folds_is_whole() {
    let (base, store) = made("fold-list");
    drop(store);
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let writer = {
        let base = base.clone();
        std::thread::spawn(move || {
            let store = again(&base);
            for i in 0..(manifest::FOLD_LINES * 3) {
                store.write(&p(&format!("data/mail/acc/INBOX/m{i}")), b"x", How::default()).unwrap();
            }
        })
    };
    let lister = {
        let (base, stop) = (base.clone(), stop.clone());
        std::thread::spawn(move || {
            let store = again(&base);
            let (mut last, mut listings) = (0, 0);
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let listed = store.list(&p("data/mail/acc/INBOX")).unwrap();
                assert!(listed.len() >= last, "a listing never goes back: {} after {last}", listed.len());
                last = listed.len();
                listings += 1;
            }
            listings
        })
    };
    writer.join().unwrap();
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(lister.join().unwrap() > 0);
    assert_eq!(again(&base).list(&p("data/mail/acc/INBOX")).unwrap().len(), manifest::FOLD_LINES as usize * 3);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn n_from_two_to_none_and_to_five() {
    let (base, store) = made("n");
    let path = p("config/dnd-people.toml");
    for i in 0..4 {
        store.write(&path, format!("{i}").as_bytes(), How::default()).unwrap();
    }
    assert_eq!(store.versions(&path).unwrap().len(), 3, "the current one and n = 2");
    store.set_versions(0).unwrap();
    store.write(&path, b"4", How::default()).unwrap();
    assert_eq!(store.versions(&path).unwrap().len(), 1, "n = 0: the current one alone");
    assert!(store.remove(&path, Expect::Any).unwrap());
    assert_eq!(store.versions(&path).unwrap_err(), Error::Absent, "n = 0: a removal is final");
    store.set_versions(5).unwrap();
    for i in 0..8 {
        store.write(&path, format!("{i}").as_bytes(), How::default()).unwrap();
    }
    assert_eq!(store.versions(&path).unwrap().len(), 6, "the current one and n = 5");
    store.set_versions(2).unwrap();
    let collected = store.collect().unwrap();
    assert_eq!(collected.versions, 3, "{collected:?}");
    assert_eq!(store.versions(&path).unwrap().len(), 3);
    let report = store.check(false).unwrap();
    assert_eq!(report.unlisted, 0, "{report:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_restore_and_its_undo() {
    let (base, store) = made("restore");
    let path = p("data/records/persons.toml");
    for i in 1..=3 {
        store.write(&path, format!("persons {i}").as_bytes(), How::default()).unwrap();
    }
    assert_eq!(store.versions(&path).unwrap().iter().map(|v| v.generation).collect::<Vec<_>>(), [3, 2, 1]);
    assert_eq!(store.read_version(&path, 1).unwrap().bytes, b"persons 1");
    let restored = store.restore(&path, 1).unwrap();
    assert_eq!(text(&store, "data/records/persons.toml").unwrap(), "persons 1");
    // Undone: what it replaced is an earlier version now, and is put back the same way.
    assert_eq!(store.versions(&path).unwrap().iter().map(|v| v.generation).collect::<Vec<_>>(), [restored.generation, 3, 2]);
    store.restore(&path, 3).unwrap();
    assert_eq!(text(&store, "data/records/persons.toml").unwrap(), "persons 3");
    // A removal undone.
    store.remove(&path, Expect::Any).unwrap();
    assert_eq!(store.read(&path).unwrap(), None);
    let last = store.versions(&path).unwrap()[0].generation;
    store.restore(&path, last).unwrap();
    assert_eq!(text(&store, "data/records/persons.toml").unwrap(), "persons 3");
    assert_eq!(store.restore(&path, 999).unwrap_err(), Error::Absent);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_purge_leaves_nothing_of_the_file() {
    let (base, store) = made("purge");
    let path = p("data/clients/c1/notes.md");
    for i in 0..3 {
        store.write(&path, format!("Allergie {i}").as_bytes(), How::only_here()).unwrap();
    }
    store.write(&p("data/clients/c1/other.md"), b"stays", How::default()).unwrap();
    let purged = store.purge(&path).unwrap();
    assert_eq!((purged.removed, purged.left), (4, 0), "the current copy, its twin, two versions");
    assert_eq!(store.read(&path).unwrap(), None);
    assert_eq!(store.versions(&path).unwrap_err(), Error::Absent);
    // No list on the disk names it any more, in either copy.
    let folder = p("data/clients/c1");
    for copy in [0u8, 1] {
        let mut bytes = Vec::new();
        let name = store.name(format::Role::Manifest, folder.as_str(), 0, copy);
        let mut file = std::fs::File::open(store.manifest_place(&folder, copy)).unwrap();
        format::open(&mut file, &store.keys.sealing, &name, &format::Expected { role: format::Role::Manifest, copy, generation: None, path: None }, &mut bytes, u64::MAX).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("notes.md"));
    }
    assert_eq!(text(&store, "data/clients/c1/other.md").unwrap(), "stays");
    let report = store.check(false).unwrap();
    assert_eq!(report.unlisted, 0, "{report:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_file_that_only_grows_with_a_damaged_line() {
    let (base, store) = made("lines");
    let path = p("state/mcp/said.jsonl");
    for i in 1..=5 {
        assert_eq!(store.append_line(&path, &format!("{{\"n\":{i}}}"), How::default()).unwrap(), i);
    }
    let place = store.blob_place(&path, format::Role::Lines, 1, 0);
    let on_disk = std::fs::read_to_string(&place).unwrap();
    assert!(!on_disk.contains("\"n\""), "sealed line by line");
    assert!(on_disk.lines().nth(4).unwrap().starts_with("3 "), "each line's number in clear");
    // The clear line, line 0, then lines 1 to 5: line 3 damaged, the others read.
    let mut lines: Vec<String> = on_disk.lines().map(str::to_string).collect();
    let mut chars: Vec<char> = lines[4].chars().collect();
    let middle = chars.len() / 2;
    chars[middle] = if chars[middle] == 'A' { 'B' } else { 'A' };
    lines[4] = chars.into_iter().collect();
    std::fs::write(&place, lines.join("\n") + "\n").unwrap();
    let read = store.read_lines(&path).unwrap().unwrap();
    assert_eq!(read.damaged, [3]);
    assert_eq!(read.lines.iter().map(|(n, _)| *n).collect::<Vec<_>>(), [1, 2, 4, 5]);
    // A line cut by a power cut at the end: dropped by the next append.
    std::fs::write(&place, lines.join("\n") + "\nAAAAAAAAAA").unwrap();
    assert_eq!(again(&base).append_line(&path, "{\"n\":6}", How::default()).unwrap(), 6);
    let read = again(&base).read_lines(&path).unwrap().unwrap();
    assert_eq!(read.lines.last().unwrap().1, "{\"n\":6}");
    // Never read or written whole.
    assert!(matches!(store.read(&path), Err(Error::Refused(_))));
    assert!(matches!(store.write(&path, b"whole", How::default()), Err(Error::Refused(_))));
    let rewritten = store.rewrite_lines(&path, &["{\"n\":5}".to_string(), "{\"n\":6}".to_string()], How::default()).unwrap();
    assert_eq!(rewritten.generation, 2);
    assert_eq!(store.read_lines(&path).unwrap().unwrap().lines.len(), 2);
    assert!(!place.exists(), "the old generation taken out");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn long_and_hostile_names() {
    let (base, store) = made("names");
    let long = "é".repeat(127) + "x";
    assert_eq!(long.len(), 255);
    let names = [long.as_str(), "CON", "a:b*c?.txt ", "trailing.", "<>|\"", "\u{1f}\u{7f}", "名前.toml", ".hidden", "..."];
    for name in names {
        let path = p(&format!("data/odd/{name}"));
        store.write(&path, name.as_bytes(), How::default()).unwrap();
        assert_eq!(store.read(&path).unwrap().unwrap().bytes, name.as_bytes());
    }
    assert_eq!(store.list(&p("data/odd")).unwrap().len(), names.len());
    let too_long = format!("data/{}", "x".repeat(256));
    for refused in ["data/../config/x", "data/./x", "data//x", "data/a\\b", "data/a\0b", "elsewhere/x", "", "store/moved", too_long.as_str()] {
        assert!(matches!(LogicalPath::new(refused), Err(Error::Refused(_))), "{refused:?}");
    }
    assert!(matches!(store.write(&p("config"), b"x", How::default()), Err(Error::Refused(_))), "a root is a folder");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_store_in_a_synced_folder_is_refused() {
    let base = scratch("synced");
    let carried = base.join("Nextcloud");
    std::fs::create_dir_all(&carried).unwrap();
    let options = Options { synced_roots: vec![carried.clone()], ..Options::default() };
    assert!(matches!(Store::create(&Folders::under(&carried.join("sioul")), &RootKey::test(), options), Err(Error::Synced(_))));
    // Wherever it is, a folder with a sync app's own mark.
    let marked = base.join("elsewhere");
    std::fs::create_dir_all(marked.join(".stfolder")).unwrap();
    assert!(matches!(Store::create(&Folders::under(&marked.join("profile")), &RootKey::test(), Options::default()), Err(Error::Synced(_))));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn update_expect_rename_and_attributes() {
    let (base, store) = made("update");
    let path = p("config/senders/allowed.toml");
    let made_now = store.update(&path, How { expect: Expect::Absent, ..How::default() }, &mut |old| Ok(old.is_none().then(|| b"a@b".to_vec()))).unwrap();
    assert_eq!(made_now.unwrap().generation, 1);
    assert_eq!(store.write(&path, b"x", How { expect: Expect::Absent, ..How::default() }).unwrap_err(), Error::Changed { now: Some(1) });
    assert_eq!(store.write(&path, b"x", How::over(7)).unwrap_err(), Error::Changed { now: Some(1) });
    store.write(&path, b"a@b\nc@d", How::over(1)).unwrap();
    assert!(matches!(store.update(&path, How::default(), &mut |_| Err("no".to_string())), Err(Error::Change(_))));
    // Inside a change, the store refuses its own folder: never a write under itself.
    let inner = store.update(&path, How::default(), &mut |_| store.read(&path).map(|_| None).map_err(|e| e.to_string()));
    assert!(matches!(inner, Err(Error::Change(_))), "{inner:?}");
    // Mail's flags: one line, the blob untouched.
    let message = p("data/mail/acc/INBOX/1700000001.M2.host");
    store.write(&message, b"Subject: x", How::default()).unwrap();
    let blobs = |base: &Path| snapshot(base).into_iter().filter(|(path, _)| path.components().any(|c| c.as_os_str() == "sealed") && !path.to_string_lossy().contains(".locks")).count();
    let before = blobs(&base);
    store.set_attrs(&message, &[("flags", Some("RS")), ("new", None)]).unwrap();
    let entry = store.stat(&message).unwrap().unwrap();
    assert_eq!((entry.generation, entry.attrs.get("flags").map(String::as_str)), (1, Some("RS")));
    assert_eq!(blobs(&base), before, "no blob written");
    // Renamed in its folder, then to another, its flags with it.
    let moved = p("data/mail/acc/INBOX/renamed");
    store.rename(&message, &moved, Expect::Any).unwrap();
    assert_eq!(store.read(&message).unwrap(), None);
    assert_eq!(store.stat(&moved).unwrap().unwrap().attrs.get("flags").map(String::as_str), Some("RS"));
    let trash = p("data/mail/acc/Trash/renamed");
    store.rename(&moved, &trash, Expect::Any).unwrap();
    assert_eq!(store.read(&trash).unwrap().unwrap().bytes, b"Subject: x");
    assert_eq!(store.stat(&trash).unwrap().unwrap().attrs.get("flags").map(String::as_str), Some("RS"));
    assert!(store.list(&p("data/mail/acc/INBOX")).unwrap().is_empty());
    assert_eq!(store.rename(&moved, &trash, Expect::Any).unwrap_err(), Error::Absent);
    let report = store.check(false).unwrap();
    assert_eq!(report.unlisted, 0, "{report:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_large_file_streams_and_the_move_never_goes_back() {
    let (base, store) = made("stream");
    let big = noise(3_000_000, 7);
    store.write_from(&p("cache/spam/corpus"), &mut big.as_slice(), How::default()).unwrap();
    let mut out = Vec::new();
    assert_eq!(store.read_into(&p("cache/spam/corpus"), &mut out).unwrap(), Some(big.len() as u64));
    assert_eq!(out, big);
    store.mark_moved("config").unwrap();
    store.mark_moved("lists").unwrap();
    assert_eq!(store.moved().unwrap().into_iter().collect::<Vec<_>>(), ["config", "lists"]);
    // The newest record lost: an older copy says less, and what was moved stays moved as far as any copy says.
    let record = LogicalPath::internal("moved");
    assert!(store.blob_place(&record, format::Role::File, 1, 0).exists());
    std::fs::remove_file(store.blob_place(&record, format::Role::File, 2, 0)).unwrap();
    std::fs::remove_file(store.blob_place(&record, format::Role::File, 2, 1)).unwrap();
    assert!(store.moved().unwrap().contains("config"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_generation_never_comes_back_after_a_purge() {
    let (base, store) = made("no-replay");
    let path = p("data/clients/c1/notes.md");
    let old = store.write(&path, b"old words", How::default()).unwrap().generation;
    let kept = std::fs::read(blob(&store, path.as_str(), old, 0)).unwrap();
    store.purge(&path).unwrap();
    let new = store.write(&path, b"new words", How::default()).unwrap().generation;
    assert!(new > old, "the folder's highest generation is past every one it gave");
    // The purged blob put back under its own name: never the file, never taken by a repair, swept as a leftover.
    std::fs::write(blob(&store, path.as_str(), old, 0), &kept).unwrap();
    assert_eq!(text(&store, path.as_str()).unwrap(), "new words");
    let report = store.check(true).unwrap();
    assert!(report.newer.is_empty(), "{report:?}");
    assert_eq!(text(&store, path.as_str()).unwrap(), "new words");
    assert!(!blob(&store, path.as_str(), old, 0).exists(), "taken out by the repair: a blob no list names");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_purge_cut_short_is_finished_by_a_second() {
    let (base, store) = made("purge-cut");
    let path = p("data/clients/c2/row");
    for i in 0..3 {
        store.write(&path, format!("row {i}").as_bytes(), How::only_here()).unwrap();
    }
    CRASH.with(|c| c.set(Some((Step::BeforeJournal, 0))));
    assert!(store.purge(&path).is_err());
    drop(store);
    let store = again(&base);
    assert!(matches!(store.read(&path), Err(Error::Damaged { .. })), "its blobs went first; its record says it was there");
    let purged = store.purge(&path).unwrap();
    assert_eq!(purged.left, 0);
    assert_eq!(store.read(&path).unwrap(), None);
    assert_eq!(store.check(false).unwrap().unlisted, 0);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn everything_under_a_folder_goes_with_purge_under() {
    let (base, store) = made("purge-under");
    let client = p("data/clients/c3");
    store.write(&p("data/clients/c3/description.md"), b"one", How::default()).unwrap();
    store.write(&p("data/clients/c3/description.md"), b"two", How::default()).unwrap();
    store.write(&p("data/clients/c3/rows/r1"), b"a session", How::default()).unwrap();
    // A file removed last week: its versions still there.
    store.write(&p("data/clients/c3/rows/r2"), b"a session removed", How::default()).unwrap();
    store.remove(&p("data/clients/c3/rows/r2"), Expect::Any).unwrap();
    assert_eq!(store.versions(&p("data/clients/c3/rows/r2")).unwrap().len(), 1);
    store.append_line(&p("data/clients/c3/log"), "seen", How::only_here()).unwrap();
    store.write(&p("data/clients/c4/description.md"), b"stays", How::default()).unwrap();
    let purged = store.purge_under(&client).unwrap();
    assert_eq!(purged.left, 0);
    assert!(purged.removed >= 6, "{purged:?}");
    for path in ["data/clients/c3/description.md", "data/clients/c3/rows/r1", "data/clients/c3/rows/r2"] {
        assert_eq!(store.read(&p(path)).unwrap(), None, "{path}");
        assert_eq!(store.versions(&p(path)).unwrap_err(), Error::Absent, "{path}");
    }
    assert!(store.folders(&p("data/clients")).unwrap().iter().all(|f| !f.is_within(&client)));
    // No blob on the disk is of it any more, nor its lists.
    assert!(store.scan_blobs(Root::Data).iter().all(|f| !LogicalPath::checked_any(&f.inner.path).is_some_and(|p| p.is_within(&client))));
    assert_eq!(text(&store, "data/clients/c4/description.md").unwrap(), "stays");
    assert_eq!(store.check(false).unwrap().unlisted, 0);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn pieces_swapped_or_cut_at_their_boundary_do_not_open() {
    let (base, store) = made("pieces");
    let path = "cache/big";
    let first = store.write(&p(path), &noise(300_000, 3), How { uncompressed: true, ..How::default() }).unwrap().generation;
    let current = store.write(&p(path), &noise(300_000, 4), How { uncompressed: true, ..How::default() }).unwrap().generation;
    let place = blob(&store, path, current, 0);
    let whole = std::fs::read(&place).unwrap();
    // The pieces: a clear header, then a length, a nonce and the sealed bytes each.
    let mut bounds = vec![format::HEADER];
    while *bounds.last().unwrap() < whole.len() {
        let at = *bounds.last().unwrap();
        let length = (u32::from_be_bytes(whole[at..at + 4].try_into().unwrap()) & 0x7fff_ffff) as usize;
        bounds.push(at + 4 + length);
    }
    assert!(bounds.len() > 5, "several pieces");
    let (a, b, c) = (bounds[2], bounds[3], bounds[4]);
    assert_eq!(b - a, c - b, "two full pieces");
    let mut swapped = whole.clone();
    swapped[a..b].copy_from_slice(&whole[b..c]);
    swapped[b..c].copy_from_slice(&whole[a..b]);
    for broken in [swapped, whole[..c].to_vec()] {
        std::fs::write(&place, &broken).unwrap();
        let read = again(&base).read(&p(path)).unwrap().unwrap();
        assert!(matches!(read.from, Source::Earlier { generation, .. } if generation == first));
        assert_eq!(read.bytes, noise(300_000, 3));
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn lists_put_back_older_are_caught_up_from_the_blobs() {
    let (base, store) = made("older-lists");
    store.write(&p("data/records/a.toml"), b"a one", How::default()).unwrap();
    let folder = p("data/records");
    store.fold(&folder, &mut match store.load(&folder, true).unwrap() { manifest::Loaded::Ready(s) => s, manifest::Loaded::NeedsRepair => unreachable!() }).unwrap();
    let saved: Vec<(PathBuf, Option<Vec<u8>>)> = [store.manifest_place(&folder, 0), store.manifest_place(&folder, 1), store.journal_place(&folder)].into_iter().map(|place| { let bytes = std::fs::read(&place).ok(); (place, bytes) }).collect();
    store.write(&p("data/records/a.toml"), b"a two", How::default()).unwrap();
    store.write(&p("data/records/b.toml"), b"b one", How::default()).unwrap();
    drop(store);
    // A backup put back: the lists as they were, the blobs as they are.
    for (place, bytes) in &saved {
        match bytes {
            Some(bytes) => std::fs::write(place, bytes).unwrap(),
            None => {
                let _ = std::fs::remove_file(place);
            }
        }
    }
    let store = again(&base);
    assert_eq!(text(&store, "data/records/a.toml").unwrap(), "a one", "the list put back says so, until caught up");
    let report = store.check(false).unwrap();
    assert_eq!(report.newer.len(), 2, "{report:?}");
    let collected = store.collect().unwrap();
    assert_eq!(collected.adopted, 2, "{collected:?}");
    assert_eq!(collected.unlisted, 0, "a blob newer than its list is taken, never removed");
    assert_eq!(text(&store, "data/records/a.toml").unwrap(), "a two");
    assert_eq!(text(&store, "data/records/b.toml").unwrap(), "b one");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_journal_cut_in_a_line_loses_no_written_content() {
    let (base, store) = made("journal-cut");
    store.write(&p("state/porch.toml"), b"one", How::default()).unwrap();
    store.write(&p("state/porch.toml"), b"two", How::default()).unwrap();
    let journal = store.journal_place(&p("state"));
    drop(store);
    let bytes = std::fs::read(&journal).unwrap();
    std::fs::write(&journal, &bytes[..bytes.len() - 20]).unwrap();
    let store = again(&base);
    assert_eq!(text(&store, "state/porch.toml").unwrap(), "one", "the cut line was the change: not made");
    store.collect().unwrap();
    assert_eq!(text(&store, "state/porch.toml").unwrap(), "two", "its blob, newer than its list, taken");
    store.write(&p("state/porch.toml"), b"three", How::default()).unwrap();
    assert_eq!(text(&again(&base), "state/porch.toml").unwrap(), "three", "the journal goes on past the cut");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_file_that_only_grows_cut_at_a_line_end_with_a_broken_head_and_a_twin() {
    let (base, store) = made("lines-more");
    let path = p("state/calls/log");
    for i in 1..=4 {
        store.append_line(&path, &format!("call {i}"), How::only_here()).unwrap();
    }
    let first = store.blob_place(&path, format::Role::Lines, 1, 0);
    let second = store.blob_place(&path, format::Role::Lines, 1, 1);
    assert!(second.exists(), "it exists nowhere else: two copies");
    // The first copy cut at a line's end, its head broken: the second gives every line.
    let text_now = std::fs::read_to_string(&first).unwrap();
    let mut lines: Vec<String> = text_now.lines().map(str::to_string).collect();
    lines.truncate(lines.len() - 1);
    lines[1] = format!("0 {}", "A".repeat(80));
    std::fs::write(&first, lines.join("\n") + "\n").unwrap();
    let read = store.read_lines(&path).unwrap().unwrap();
    assert!(read.damaged.is_empty(), "{read:?}");
    assert_eq!(read.lines.len(), 4);
    // Without its second copy, the cut is seen by the count the record keeps.
    std::fs::remove_file(&second).unwrap();
    let read = again(&base).read_lines(&path).unwrap().unwrap();
    assert_eq!(read.damaged, [4]);
    assert_eq!(again(&base).append_line(&path, "call 5", How::default()).unwrap(), 5, "a broken head blocks no append");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn cuts_in_a_rename_leave_the_file_in_one_place_or_both() {
    for (step, cross) in [(Step::BeforeJournal, false), (Step::AfterJournal, true)] {
        let (base, store) = made(&format!("rename-cut-{step:?}"));
        let from = p("data/mail/acc/INBOX/m1");
        store.write(&from, b"Subject: x", How::unversioned()).unwrap();
        let to = if cross { p("data/mail/acc/Trash/m1") } else { p("data/mail/acc/INBOX/m2") };
        CRASH.with(|c| c.set(Some((step, 0))));
        assert!(store.rename(&from, &to, Expect::Any).is_err());
        CRASH.with(|c| c.set(None));
        drop(store);
        let store = again(&base);
        store.collect().unwrap();
        let there = [store.read(&from).unwrap().is_some(), store.read(&to).unwrap().is_some()];
        assert!(there.iter().any(|t| *t), "{step:?}: never in neither");
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[test]
fn the_disk_full_in_the_middle_of_a_write() {
    let (base, store) = made("full-mid");
    let path = p("data/records/budgets.toml");
    store.write(&path, b"before", How::default()).unwrap();
    FULL_AFTER.with(|f| f.set(Some(2_000)));
    assert_eq!(store.write(&path, &noise(50_000, 9), How { uncompressed: true, ..How::default() }).unwrap_err(), Error::NoRoom);
    FULL_AFTER.with(|f| f.set(None));
    assert_eq!(text(&store, "data/records/budgets.toml").unwrap(), "before");
    // Nothing half written left behind.
    let leftovers = snapshot(&base).into_iter().filter(|(path, _)| path.file_name().is_some_and(|n| n.to_string_lossy().starts_with(".tmp-"))).count();
    assert_eq!(leftovers, 0);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_change_of_one_folder_may_not_call_the_store_on_another() {
    let (base, store) = made("two-folders");
    store.write(&p("config/a.toml"), b"a", How::default()).unwrap();
    let inner = store.update(&p("config/b.toml"), How::default(), &mut |_| store.read(&p("data/x")).map(|_| None).map_err(|e| e.to_string()));
    assert!(matches!(inner, Err(Error::Change(_))), "{inner:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn n_is_the_store_s_own_for_every_process() {
    let (base, store) = made("n-kept");
    store.set_versions(4).unwrap();
    assert_eq!(again(&base).versions_kept(), 4);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_removed_message_keeps_no_version() {
    let (base, store) = made("unversioned");
    let path = p("data/mail/acc/INBOX/m9");
    store.write(&path, b"Subject: once", How::unversioned()).unwrap();
    store.remove(&path, Expect::Any).unwrap();
    assert_eq!(store.versions(&path).unwrap_err(), Error::Absent, "its source is its backup");
    let _ = std::fs::remove_dir_all(&base);
}


#[test]
fn a_flipped_bit_in_a_log_s_clear_first_line_loses_no_line() {
    let (base, store) = made("lines-head");
    let path = p("state/mcp/said.jsonl");
    for i in 1..=3 {
        store.append_line(&path, &format!("said {i}"), How::default()).unwrap();
    }
    let place = store.blob_place(&path, format::Role::Lines, 1, 0);
    drop(store);
    // One byte of the clear first line changed: its generation no longer reads.
    let mut bytes = std::fs::read(&place).unwrap();
    let end = bytes.iter().position(|&b| b == b'\n').unwrap();
    bytes[end - 1] = b'x';
    std::fs::write(&place, &bytes).unwrap();
    let store = again(&base);
    let read = store.read_lines(&path).unwrap().unwrap();
    assert_eq!(read.lines.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>(), ["said 1", "said 2", "said 3"], "{read:?}");
    assert_eq!(store.append_line(&path, "said 4", How::default()).unwrap(), 4, "the next append keeps every line");
    assert_eq!(again(&base).read_lines(&path).unwrap().unwrap().lines.len(), 4);
    // Its only copy gone: the append is refused, never a log begun again empty.
    std::fs::remove_file(&place).unwrap();
    let store = again(&base);
    assert!(matches!(store.append_line(&path, "said 5", How::default()), Err(Error::Damaged { .. })));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_log_newer_than_its_list_is_taken_never_swept() {
    let (base, store) = made("lines-older-list");
    let folder = p("state/calls");
    store.write(&p("state/calls/table.toml"), b"t", How::default()).unwrap();
    store.fold(&folder, &mut match store.load(&folder, true).unwrap() { manifest::Loaded::Ready(s) => s, manifest::Loaded::NeedsRepair => unreachable!() }).unwrap();
    let saved: Vec<(PathBuf, Option<Vec<u8>>)> = [store.manifest_place(&folder, 0), store.manifest_place(&folder, 1), store.journal_place(&folder)].into_iter().map(|place| { let bytes = std::fs::read(&place).ok(); (place, bytes) }).collect();
    let path = p("state/calls/log");
    for i in 1..=3 {
        store.append_line(&path, &format!("call {i}"), How::default()).unwrap();
    }
    drop(store);
    // A backup of the lists put back, older than the log.
    for (place, bytes) in &saved {
        match bytes {
            Some(bytes) => std::fs::write(place, bytes).unwrap(),
            None => {
                let _ = std::fs::remove_file(place);
            }
        }
    }
    let store = again(&base);
    let report = store.check(false).unwrap();
    assert_eq!(report.unlisted, 0, "named as newer, never a leftover: {report:?}");
    let collected = store.collect().unwrap();
    assert_eq!(collected.unlisted, 0, "{collected:?}");
    assert_eq!(collected.adopted, 1, "{collected:?}");
    let read = store.read_lines(&path).unwrap().unwrap();
    assert_eq!(read.lines.len(), 3, "{read:?}");
    assert!(read.damaged.is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_creation_cut_before_its_store_toml_is_begun_again() {
    let base = scratch("create-cut");
    let folders = Folders::under(&base);
    CRASH.with(|c| c.set(Some((Step::BeforeJournal, 0))));
    assert!(Store::create(&folders, &RootKey::test(), Options::default()).is_err(), "cut before store.toml");
    CRASH.with(|c| c.set(None));
    assert_eq!(Store::open(&folders, &RootKey::test(), Options::default()).unwrap_err(), Error::NotCreated, "nothing was ever kept: not damage");
    let store = Store::create(&folders, &RootKey::test(), Options::default()).unwrap();
    store.write(&p("config/config.toml"), b"x = 1", How::default()).unwrap();
    drop(store);
    assert_eq!(text(&again(&base), "config/config.toml").unwrap(), "x = 1");
    // Once made, a store.toml lost beside its files is damage, never a new store.
    std::fs::remove_file(folders.store_file()).unwrap();
    std::fs::remove_file(folders.sealed(Root::Data).join("store.toml")).unwrap();
    assert_eq!(Store::create(&folders, &RootKey::test(), Options::default()).unwrap_err(), Error::Exists);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_write_over_a_list_put_back_older_keeps_the_newer_content() {
    let (base, store) = made("write-over-older");
    let folder = p("config");
    store.write(&p("config/config.toml"), b"one", How::default()).unwrap();
    store.fold(&folder, &mut match store.load(&folder, true).unwrap() { manifest::Loaded::Ready(s) => s, manifest::Loaded::NeedsRepair => unreachable!() }).unwrap();
    let saved: Vec<(PathBuf, Option<Vec<u8>>)> = [store.manifest_place(&folder, 0), store.manifest_place(&folder, 1), store.journal_place(&folder)].into_iter().map(|place| { let bytes = std::fs::read(&place).ok(); (place, bytes) }).collect();
    store.write(&p("config/config.toml"), b"two", How::default()).unwrap();
    drop(store);
    for (place, bytes) in &saved {
        match bytes {
            Some(bytes) => std::fs::write(place, bytes).unwrap(),
            None => {
                let _ = std::fs::remove_file(place);
            }
        }
    }
    let store = again(&base);
    store.write(&p("config/config.toml"), b"three", How::default()).unwrap();
    assert_eq!(text(&store, "config/config.toml").unwrap(), "three");
    let versions = store.versions(&p("config/config.toml")).unwrap();
    assert_eq!(store.read_version(&p("config/config.toml"), versions[1].generation).unwrap().bytes, b"two", "the newer content became the earlier version, never lost: {versions:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_write_over_a_damaged_copy_keeps_its_good_twin_as_the_earlier_version() {
    let (base, store) = made("twin-kept");
    let path = p("data/drafts/d1.toml");
    let first = store.write(&path, b"draft one", How::only_here()).unwrap();
    // Its first copy damaged: read once, the damage noted.
    std::fs::write(blob(&store, "data/drafts/d1.toml", first.generation, 0), b"sioul-sealed broken").unwrap();
    assert_eq!(store.read(&path).unwrap().unwrap().from, Source::Twin);
    store.write(&path, b"draft two", How::default()).unwrap();
    let versions = store.versions(&path).unwrap();
    assert_eq!(versions.len(), 2, "{versions:?}");
    assert_eq!(store.read_version(&path, first.generation).unwrap().bytes, b"draft one", "the twin's content kept: an undo, never lost");
    let _ = std::fs::remove_dir_all(&base);
}
