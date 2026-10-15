// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sharing folder stays small (docs/database.md, "Old contents go";
//! 15 October 2026): the names the others' records gave lately are dated by
//! the first reading of those records only, never by this device's own
//! records nor by a rebuild (`Sealed::named_at`); replaced contents stay two
//! weeks once every device keeps them so, 90 days while an older Sioul shares
//! (`unused_days`), counted from when they were replaced, after days away
//! too; a device back after weeks keeps its own last versions; a full round
//! waits until as much as the last one restated was appended since
//! (`full_round_after`); a large note or paper is sealed once it has settled
//! (`LARGE_SETTLES`). Fiction only.

use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;

const DAY_MS: i64 = 24 * 60 * MINUTE;

fn put(dir: &Path, relative: &str, bytes: &[u8]) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Bytes that do not compress, the same for the same seed: a recording, a scan.
fn noise(length: usize, seed: u32) -> Vec<u8> {
    let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..length)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

fn dated(path: &Path, at: std::time::SystemTime) {
    std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
}

/// An exchange with `notes` as this computer's notes folder, notes and papers shared.
fn with_notes(c: &Computer, folder: &Path, key: &[u8; 32], now: i64, notes: &Path) -> Outcome {
    let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
    let stores = stores_of(&config, &c.roots, &|part| matches!(part, "notes" | "papers") || shared_by_default(part, &config));
    exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: true, hurry: None }, &stores, now).unwrap()
}

/// A file's content, by its sealed file's name in the folder.
fn sealed_name(key: &[u8; 32], path: &Path) -> String {
    crate::blobs::name(key, &crate::blobs::hash_file(path).unwrap().0)
}

fn named_at(c: &Computer) -> BTreeMap<String, i64> {
    Memory::load_all(&c.memory, &c.id).sealed.named_at
}

fn sealed_files(folder: &Path) -> BTreeSet<String> {
    std::fs::read_dir(folder.join("blobs")).into_iter().flatten().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| !n.starts_with('.')).collect()
}

/// A device's sealed entry, as the window writes it (`devices::change`):
/// `keeping` how it keeps sealed files (`blobs::KEEPING`; 0, an older Sioul's).
fn says_it_keeps(c: &Computer, folder: &Path, key: &[u8; 32], keeping: u32) {
    let id = c.id.clone();
    crate::devices::change(&crate::devices::own_path(&c.roots.state), Some((folder, key)), |e| {
        e.id = id;
        e.kind = crate::devices::COMPUTER.into();
        e.version = "0.0.5".into();
        e.working = true;
        e.keeping = keeping;
    })
    .unwrap();
}

/// A content another device's record named is protected from when this
/// device first read that record, never again from a reading of the same
/// record (a rebuild, a store joining, a memory from before rounds said where
/// they stand), never from its own records. Until 15 October 2026 every
/// rebuild dated each superseded content of this device's own kept rounds
/// anew, and those never aged out while new versions kept coming.
#[test]
fn a_rebuild_leaves_the_times_others_named_as_they_were() {
    let base = scratch("named-at");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (dn, pn) = (base.join("desk-notes"), base.join("phone-notes"));
    put(&dn, "lease.md", b"the first lease\n");
    with_notes(&desk, &folder, &key, NOW, &dn);
    let first = sealed_name(&key, &dn.join("lease.md"));
    put(&dn, "lease.md", b"the lease, signed\n");
    with_notes(&desk, &folder, &key, NOW + MINUTE, &dn);
    let second = sealed_name(&key, &dn.join("lease.md"));
    assert!(named_at(&desk).is_empty(), "never from its own records");
    // The phone reads the desk's records for the first time: both contents named then.
    with_notes(&phone, &folder, &key, NOW + 2 * MINUTE, &pn);
    assert_eq!(named_at(&phone), [(first.clone(), NOW + 2 * MINUTE), (second.clone(), NOW + 2 * MINUTE)].into());
    // A note of the phone's, read by the desk the first time.
    put(&pn, "from-the-phone.md", b"written on the phone\n");
    with_notes(&phone, &folder, &key, NOW + 3 * MINUTE, &pn);
    with_notes(&desk, &folder, &key, NOW + 4 * MINUTE, &dn);
    let phones = sealed_name(&key, &pn.join("from-the-phone.md"));
    let desk_named = named_at(&desk);
    assert_eq!(desk_named, [(phones.clone(), NOW + 4 * MINUTE)].into(), "the others' record only, when first read");
    let phone_named = named_at(&phone);
    // Both keep sealed files two weeks (`unused_days`).
    for c in [&desk, &phone] {
        says_it_keeps(c, &folder, &key, crate::blobs::KEEPING);
    }

    // Days later, every round read again, this device's own too, each way
    // that happens: the doses' record rebuilt (the window's repair), a
    // memory from before rounds said where they stand, a store joining (a
    // notes folder moved; a part switched on, or a store a new version
    // adds, as on 14 October 2026). The same times.
    for c in [&desk, &phone] {
        rebuild(&c.memory, &c.id, &["state/health-state.toml"]).unwrap();
    }
    with_notes(&desk, &folder, &key, NOW + 5 * DAY_MS, &dn);
    with_notes(&phone, &folder, &key, NOW + 5 * DAY_MS + MINUTE, &pn);
    assert_eq!((named_at(&desk), named_at(&phone)), (desk_named.clone(), phone_named.clone()), "a rebuild leaves the times as they were");
    for c in [&desk, &phone] {
        let mut m = Memory::load_all(&c.memory, &c.id);
        m.turns = false;
        m.save(&c.memory).unwrap();
    }
    with_notes(&desk, &folder, &key, NOW + 6 * DAY_MS, &dn);
    with_notes(&phone, &folder, &key, NOW + 6 * DAY_MS + MINUTE, &pn);
    assert_eq!((named_at(&desk), named_at(&phone)), (desk_named.clone(), phone_named.clone()), "every round read again: the same times");
    let (dn, pn) = (base.join("desk-notes-moved"), base.join("phone-notes-moved"));
    copy_tree(&base.join("desk-notes"), &dn);
    copy_tree(&base.join("phone-notes"), &pn);
    with_notes(&desk, &folder, &key, NOW + 7 * DAY_MS, &dn);
    with_notes(&phone, &folder, &key, NOW + 7 * DAY_MS + MINUTE, &pn);
    assert_eq!((named_at(&desk), named_at(&phone)), (desk_named.clone(), phone_named.clone()), "a store joining reads the rounds again: the same times");

    // So the desk's superseded content goes two weeks after it was replaced,
    // whatever reads its rounds again meanwhile; the current one and the phone's stay.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(100 * 86_400);
    for name in sealed_files(&folder) {
        dated(&folder.join("blobs").join(name), old);
    }
    rebuild(&desk.memory, &desk.id, &["state/health-state.toml"]).unwrap();
    with_notes(&desk, &folder, &key, NOW + 10 * DAY_MS, &dn);
    assert!(sealed_files(&folder).contains(&first), "ten days: kept");
    with_notes(&desk, &folder, &key, NOW + MINUTE + crate::blobs::UNUSED_DAYS * DAY_MS + MINUTE, &dn);
    let left = sealed_files(&folder);
    assert!(!left.contains(&first), "two weeks after it was replaced: gone, the rebuild at ten days notwithstanding");
    assert!(left.contains(&second) && left.contains(&phones), "{left:?}");
    // A record of the phone's read for the first time names its content then.
    put(&pn, "later.md", b"later\n");
    with_notes(&phone, &folder, &key, NOW + 20 * DAY_MS, &pn);
    with_notes(&desk, &folder, &key, NOW + 20 * DAY_MS + MINUTE, &dn);
    let later = sealed_name(&key, &pn.join("later.md"));
    assert_eq!(named_at(&desk).get(&later), Some(&(NOW + 20 * DAY_MS + MINUTE)));
    let _ = std::fs::remove_dir_all(&base);
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap().filter_map(Result::ok) {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::create_dir_all(to).unwrap();
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A full round restates everything this device holds: the next one waits
/// until as much was appended since (a megabyte at least), so restating never
/// costs more than what was appended. A memory from before reads that size
/// from the full round's file.
#[test]
fn a_full_round_waits_for_as_much_as_the_last_one_restated() {
    let base = scratch("full-size");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    let lines = |from: usize, count: usize| -> String { (from..from + count).map(|n| format!("sender-{n:06}@example.org\n")).collect() };
    // Much held: its first round, full, well past a megabyte.
    let mut list = lines(0, 20_000);
    desk.write("config/known-senders.txt", &list);
    desk.exchange(&folder, &key, NOW);
    let opening = std::fs::metadata(round_file(&folder, &desk.id, 1)).unwrap().len();
    assert!(opening > 2 * ROUND_SIZE, "{opening}");
    let memory = || Memory::load(&desk.memory, &desk.id);
    assert_eq!(memory().full_size, opening, "the first round counts as full: its size noted");
    // Appended past a megabyte, below what it restated: no full round.
    let mut n = 20_000;
    let mut at = NOW;
    let mut before = 0;
    loop {
        list.push_str(&lines(n, 600));
        n += 600;
        desk.write("config/known-senders.txt", &list);
        at += MINUTE;
        desk.exchange(&folder, &key, at);
        let m = memory();
        if m.full_round > 1 {
            break;
        }
        assert_eq!(m.full_size, opening);
        before = m.grown;
        assert!(before <= opening + 200_000, "no full round past what it restated: {before} appended, {opening} restated");
    }
    assert!(before > ROUND_SIZE && before <= opening, "past a megabyte and within what the first round restated: {before}, {opening}");
    let m = memory();
    let restated = std::fs::metadata(round_file(&folder, &desk.id, m.full_round)).unwrap().len();
    assert_eq!(m.full_size, restated, "what the new full round restated, noted");
    assert!(restated > opening, "it restates all, the appended lines too");
    // A memory from before: the size read once from that round's file, the same.
    let mut kept = Memory::load_all(&desk.memory, &desk.id);
    kept.full_size = 0;
    kept.save(&desk.memory).unwrap();
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, at + MINUTE);
    assert_eq!(memory().full_size, restated);
    let _ = std::fs::remove_dir_all(&base);
}

/// A note or a paper of a megabyte or more is sealed once it has not changed
/// for ten minutes: a recording saved take after take goes once, its last
/// take; meanwhile it is unknown, as a file being written is (the others'
/// changes to it wait, nothing of it is taken out). Smaller notes and the
/// spam filter's table go at once, as before; one that never stops changing
/// goes an hour after its last version did.
#[test]
fn a_large_note_is_sealed_once_it_has_settled() {
    LARGE_SETTLE.with(|s| s.set(LARGE_SETTLING));
    let base = scratch("settles");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    let now = std::time::SystemTime::now;
    let song = dn.join("scenes").join("song.mp3");
    put(&dn, "today.md", b"today\n");
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    let empty = sealed_files(&folder);
    // Three takes within minutes: none sealed while it changes; a small note goes at once.
    for take in 1..=3u32 {
        put(&dn, "scenes/song.mp3", &noise(3 << 19, take));
        put(&dn, "today.md", format!("today, take {take}\n").as_bytes());
        let outcome = with_notes(&desk, &folder, &key, NOW + i64::from(take) * 2 * MINUTE, &dn);
        assert_eq!(outcome.sent, 1, "the small note alone: {outcome:?}");
        assert_eq!(sealed_files(&folder).len(), empty.len() + take as usize, "one sealed file a take of the small note, none of the recording");
    }
    assert!(!ln.join("scenes/song.mp3").exists());
    // Settled ten minutes: its last take sealed, once.
    dated(&song, now() - std::time::Duration::from_secs(11 * 60));
    let outcome = with_notes(&desk, &folder, &key, NOW + 20 * MINUTE, &dn);
    assert_eq!(outcome.sent, 1, "{outcome:?}");
    let take = sealed_name(&key, &song);
    assert!(sealed_files(&folder).contains(&take));
    assert_eq!(sealed_files(&folder).len(), empty.len() + 4, "the three small notes and the recording's last take");
    with_notes(&laptop, &folder, &key, NOW + 21 * MINUTE, &ln);
    assert_eq!(std::fs::read(ln.join("scenes/song.mp3")).unwrap(), noise(3 << 19, 3));
    // A paper as large waits the same way.
    let papers = dn.join(sioul_core::papers::FOLDER);
    put(&papers, "scan.pdf", &noise(2 << 20, 9));
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 22 * MINUTE, &dn).sent, 0, "a large paper changed a moment ago waits");
    dated(&papers.join("scan.pdf"), now() - std::time::Duration::from_secs(11 * 60));
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 23 * MINUTE, &dn).sent, 1);
    // The spam filter's table is no note: sealed at once, whatever its size.
    desk.write("data/spam/table.bin", &String::from_utf8(vec![b't'; 2 << 20]).unwrap());
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 24 * MINUTE, &dn).sent, 1, "the table at once");

    // The laptop changes the recording while the desk records again: the
    // laptop's take waits on the desk until the desk's settles; both are kept.
    put(&ln, "scenes/song.mp3", &noise(3 << 19, 20));
    dated(&ln.join("scenes/song.mp3"), now() - std::time::Duration::from_secs(11 * 60));
    assert_eq!(with_notes(&laptop, &folder, &key, NOW + 30 * MINUTE, &ln).sent, 1);
    put(&dn, "scenes/song.mp3", &noise(3 << 19, 21));
    let outcome = with_notes(&desk, &folder, &key, NOW + 31 * MINUTE, &dn);
    assert_eq!(outcome.waits.get("unknown"), Some(&1), "the laptop's take waits: {outcome:?}");
    assert_eq!(std::fs::read(&song).unwrap(), noise(3 << 19, 21), "the take being recorded here is never written over");
    dated(&song, now() - std::time::Duration::from_secs(11 * 60));
    with_notes(&desk, &folder, &key, NOW + 45 * MINUTE, &dn);
    assert_eq!(std::fs::read(&song).unwrap(), noise(3 << 19, 21), "settled, the later take stays and goes");
    with_notes(&laptop, &folder, &key, NOW + 46 * MINUTE, &ln);
    let kept: Vec<Vec<u8>> = std::fs::read_dir(ln.join("scenes")).unwrap().filter_map(Result::ok).map(|e| std::fs::read(e.path()).unwrap()).collect();
    assert_eq!(std::fs::read(ln.join("scenes/song.mp3")).unwrap(), noise(3 << 19, 21));
    assert!(kept.contains(&noise(3 << 19, 20)), "the laptop's take kept beside: {} files", kept.len());

    // Saved again after a quiet hour (its last version read long ago): the
    // first save waits as any other. Until the review of 15 October 2026
    // (RV-P1) the hour counted from the last read, and it went at once.
    let song = "files/notes/scenes/song.mp3";
    let mut m = Memory::load_all(&desk.memory, &desk.id);
    m.files.get_mut(song).unwrap().seen -= 2 * HOUR;
    m.save(&desk.memory).unwrap();
    put(&dn, "scenes/song.mp3", &noise(3 << 19, 30));
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 50 * MINUTE, &dn).sent, 0, "a quiet hour, then a first take: it waits");
    // One that never stops changing still goes, an hour after it was first found changed.
    let mut m = Memory::load_all(&desk.memory, &desk.id);
    *m.sealed.changing.get_mut(song).expect("found changing, and since when") -= HOUR;
    m.save(&desk.memory).unwrap();
    put(&dn, "scenes/song.mp3", &noise(3 << 19, 31));
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 51 * MINUTE, &dn).sent, 1, "changing for an hour: sealed though it just changed");
    assert!(Memory::load_all(&desk.memory, &desk.id).sealed.changing.is_empty(), "read: no longer changing");
    put(&dn, "scenes/song.mp3", &noise(3 << 19, 32));
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 52 * MINUTE, &dn).sent, 0, "then it waits again");
    // A new large file, changing more often than every ten minutes: sealed at the hour.
    let new = "files/notes/scenes/new.mp3";
    put(&dn, "scenes/new.mp3", &noise(3 << 19, 40));
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 53 * MINUTE, &dn).sent, 0);
    let mut m = Memory::load_all(&desk.memory, &desk.id);
    *m.sealed.changing.get_mut(new).expect("a new file found changing") -= HOUR;
    m.save(&desk.memory).unwrap();
    put(&dn, "scenes/new.mp3", &noise(3 << 19, 41));
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 54 * MINUTE, &dn).sent, 1, "a new one changing for an hour: sealed");
    assert_eq!(Memory::load_all(&desk.memory, &desk.id).sealed.changing.keys().collect::<Vec<_>>(), [song], "the recording still settling");
    LARGE_SETTLE.with(|s| s.set(0));
    let _ = std::fs::remove_dir_all(&base);
}

/// A memory put back from a backup (`share-own-ahead`) reads the size of the
/// last full round again from the folder, never the size it knew when it was
/// put away: an older, larger one held the next full round back for that
/// many bytes, every round kept meanwhile (review of 15 October 2026, L3).
#[test]
fn a_memory_put_back_reads_the_full_round_s_size_again() {
    let base = scratch("full-size-back");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    // A backup of the memory, from when its last full round was 50 MB.
    let mut kept = Memory::load_all(&desk.memory, &desk.id);
    kept.full_size = 50 << 20;
    kept.save(&desk.memory).unwrap();
    let backup = std::fs::read(&desk.memory).unwrap();
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
    desk.exchange(&folder, &key, NOW + MINUTE);
    // Put back: its own records go further.
    std::fs::write(&desk.memory, &backup).unwrap();
    let outcome = desk.exchange(&folder, &key, NOW + 2 * MINUTE);
    assert!(outcome.problems.iter().any(|p| p == "share-own-ahead"), "{outcome:?}");
    let first = std::fs::metadata(round_file(&folder, &desk.id, 1)).unwrap().len();
    assert_eq!(Memory::load(&desk.memory, &desk.id).full_size, first, "read again from round 1, not 50 MB");
    let _ = std::fs::remove_dir_all(&base);
}

/// Replaced contents stay 90 days while a device of the sharing runs an older
/// Sioul (its entry says nothing of how it keeps them, or it is known by its
/// records alone): that Sioul makes a content it uses again fresh only past
/// 30 days, and a phone under eDrive keeps every sealed file it brought, so a
/// version it puts back could name a content gone from the server. Once
/// every device says it keeps them two weeks, two weeks; what was kept past
/// that goes then.
#[test]
fn replaced_contents_stay_ninety_days_while_an_older_sioul_shares() {
    let base = scratch("keeping");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (dn, pn) = (base.join("desk-notes"), base.join("phone-notes"));
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(400 * 86_400);
    let age = |folder: &Path| {
        for name in sealed_files(folder) {
            dated(&folder.join("blobs").join(name), old);
        }
    };
    put(&dn, "lease.md", b"the first lease\n");
    with_notes(&desk, &folder, &key, NOW, &dn);
    let first = sealed_name(&key, &dn.join("lease.md"));
    put(&dn, "lease.md", b"the lease, signed\n");
    with_notes(&desk, &folder, &key, NOW + MINUTE, &dn);
    let second = sealed_name(&key, &dn.join("lease.md"));
    put(&pn, "from-the-phone.md", b"written on the phone\n");
    with_notes(&phone, &folder, &key, NOW + 2 * MINUTE, &pn);
    with_notes(&desk, &folder, &key, NOW + 3 * MINUTE, &dn);
    says_it_keeps(&desk, &folder, &key, crate::blobs::KEEPING);
    age(&folder);
    // The phone known by its records alone: as an older Sioul.
    with_notes(&desk, &folder, &key, NOW + 15 * DAY_MS, &dn);
    assert!(sealed_files(&folder).contains(&first), "a device known by its records alone: kept past two weeks");
    // Its entry says nothing of how it keeps them (an older Sioul's).
    says_it_keeps(&phone, &folder, &key, 0);
    with_notes(&desk, &folder, &key, NOW + 60 * DAY_MS, &dn);
    assert!(sealed_files(&folder).contains(&first), "an older Sioul shares: kept");
    with_notes(&desk, &folder, &key, NOW + MINUTE + crate::blobs::UNUSED_DAYS_BEFORE * DAY_MS + MINUTE, &dn);
    assert!(!sealed_files(&folder).contains(&first), "90 days: gone");
    // Replaced again; then the phone is updated and says it keeps them two weeks.
    let at = NOW + 100 * DAY_MS;
    put(&dn, "lease.md", b"the lease, renewed\n");
    with_notes(&desk, &folder, &key, at, &dn);
    age(&folder);
    says_it_keeps(&phone, &folder, &key, crate::blobs::KEEPING);
    with_notes(&desk, &folder, &key, at + 10 * DAY_MS, &dn);
    assert!(sealed_files(&folder).contains(&second), "ten days: kept");
    with_notes(&desk, &folder, &key, at + crate::blobs::UNUSED_DAYS * DAY_MS + MINUTE, &dn);
    assert!(!sealed_files(&folder).contains(&second), "every device keeps them two weeks: gone after two weeks");
    assert!(sealed_files(&folder).contains(&sealed_name(&key, &dn.join("lease.md"))), "the current one stays");
    let _ = std::fs::remove_dir_all(&base);
}

/// A device back after weeks away that replaces a note at once keeps the
/// content it replaced two weeks from then, not from its last use noted
/// before it left (which would sweep it in that very exchange): the version
/// the others keep by reference when they receive the change still opens.
#[test]
fn a_content_replaced_on_coming_back_stays_two_weeks_from_then() {
    let base = scratch("back");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (dn, pn) = (base.join("desk-notes"), base.join("phone-notes"));
    put(&dn, "lease.md", b"the first lease\n");
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&phone, &folder, &key, NOW + MINUTE, &pn);
    for c in [&desk, &phone] {
        says_it_keeps(c, &folder, &key, crate::blobs::KEEPING);
    }
    let first = sealed_name(&key, &dn.join("lease.md"));
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(400 * 86_400);
    dated(&folder.join("blobs").join(&first), old);
    // A month away, then the note changed at once.
    let back = NOW + 30 * DAY_MS;
    put(&dn, "lease.md", b"the lease, renewed\n");
    with_notes(&desk, &folder, &key, back, &dn);
    assert!(sealed_files(&folder).contains(&first), "replaced now: kept two weeks from now");
    // The phone keeps what it held by reference, and it opens.
    with_notes(&phone, &folder, &key, back + MINUTE, &pn);
    assert_eq!(std::fs::read(pn.join("lease.md")).unwrap(), b"the lease, renewed\n");
    let history = crate::history::root(&phone.memory);
    let kept = crate::history::versions(&history, "notes", "files/notes/lease.md");
    let Some(crate::history::Kept::Sealed { hash, .. }) = crate::history::version(&history, "notes", "files/notes/lease.md", &kept[0].stamp) else { panic!("{kept:?}: not by reference") };
    let opened = base.join("opened");
    crate::blobs::get(&folder, &key, &hash, &opened).unwrap();
    assert_eq!(std::fs::read(&opened).unwrap(), b"the first lease\n");
    // Two weeks on, less a minute: there; past them: gone.
    with_notes(&desk, &folder, &key, back + crate::blobs::UNUSED_DAYS * DAY_MS - MINUTE, &dn);
    assert!(sealed_files(&folder).contains(&first));
    with_notes(&desk, &folder, &key, back + crate::blobs::UNUSED_DAYS * DAY_MS + MINUTE, &dn);
    assert!(!sealed_files(&folder).contains(&first), "two weeks after it was replaced: gone");
    let _ = std::fs::remove_dir_all(&base);
}

/// A device back after weeks away reads all it missed and writes the latest
/// version of each file over its own. The content its file held left the
/// folder meanwhile (two weeks after another device replaced it): that
/// version is kept as a copy, not by a reference that would no longer open,
/// and puts back.
#[test]
fn a_device_back_after_weeks_keeps_its_own_last_version() {
    let base = scratch("away");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    put(&dn, "lease.md", b"the first lease\n");
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    for c in [&desk, &laptop] {
        says_it_keeps(c, &folder, &key, crate::blobs::KEEPING);
    }
    let first = sealed_name(&key, &ln.join("lease.md"));
    dated(&folder.join("blobs").join(&first), std::time::SystemTime::now() - std::time::Duration::from_secs(400 * 86_400));
    // The laptop away; the desk replaces the note, and two weeks later its old content goes.
    put(&dn, "lease.md", b"the lease, renewed\n");
    with_notes(&desk, &folder, &key, NOW + DAY_MS, &dn);
    with_notes(&desk, &folder, &key, NOW + 16 * DAY_MS, &dn);
    assert!(!sealed_files(&folder).contains(&first), "two weeks after it was replaced: gone");
    // Back after three weeks: the latest version, its own kept beside as a copy.
    let outcome = with_notes(&laptop, &folder, &key, NOW + 21 * DAY_MS, &ln);
    assert!(outcome.problems.is_empty(), "{outcome:?}");
    assert_eq!(std::fs::read(ln.join("lease.md")).unwrap(), b"the lease, renewed\n");
    let history = crate::history::root(&laptop.memory);
    let kept = crate::history::versions(&history, "notes", "files/notes/lease.md");
    assert!(kept.len() == 1 && kept[0].sealed.is_none(), "kept as a copy: {kept:?}");
    let config = Config { notes_root: Some(ln.display().to_string()), ..Config::default() };
    let stores = stores_of(&config, &laptop.roots, &|part| matches!(part, "notes" | "papers") || shared_by_default(part, &config));
    put_back(&laptop.memory, Some((&folder, &key)), &stores, "notes", "files/notes/lease.md", &kept[0].stamp, NOW + 21 * DAY_MS + MINUTE).unwrap();
    assert_eq!(std::fs::read(ln.join("lease.md")).unwrap(), b"the first lease\n", "put back");
    let _ = std::fs::remove_dir_all(&base);
}
