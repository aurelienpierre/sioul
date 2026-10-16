// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A device that no longer counts never holds the folder's files back
//! (docs/database.md, "The log" and "Old contents go"; 16 October 2026): the
//! rounds' floor counts only the devices that count by sealed facts
//! (`counted`, `readers`), never one that left, that you forgot, that another
//! replaces, nor one silent for `SILENT_DAYS`; a device that counts and whose
//! notes the folder does not show holds everything back; a device forgotten
//! that comes back reads from the oldest round kept and finds every entry; the
//! past rounds of a device that no longer counts go, below its last full round
//! only, once every device that counts read past them (`departed_rounds`); the
//! sealed files it wrote that no record names any more go in time, never a
//! content a current record names, the texts' media included (`sweep_theirs`).
//! Fiction only.

use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;

const DAY_MS: i64 = 24 * 60 * MINUTE;

fn put(dir: &Path, relative: &str, bytes: &[u8]) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// An exchange with `notes` as this computer's notes folder, the parts `on`
/// shared besides those shared by default.
fn with_parts(c: &Computer, folder: &Path, key: &[u8; 32], now: i64, notes: &Path, on: &[&str]) -> Outcome {
    let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
    let stores = stores_of(&config, &c.roots, &|part| on.contains(&part) || shared_by_default(part, &config));
    exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: true, hurry: None }, &stores, now).unwrap()
}

/// A computer's rounds in the folder, oldest first.
fn kept(folder: &Path, id: &str) -> Vec<u32> {
    rounds(folder).get(id).cloned().unwrap_or_default()
}

fn full_round(c: &Computer) -> u32 {
    Memory::load(&c.memory, &c.id).full_round
}

/// A large draft changed each minute on `c` until a full round opens (a
/// megabyte appended), `along` run after each of its exchanges (a reader
/// reading along). The time reached.
fn grow(c: &Computer, folder: &Path, key: &[u8; 32], mut now: i64, along: &dyn Fn(i64)) -> i64 {
    // A device's first round counts as full: the next one, past it.
    let before = full_round(c).max(1);
    while full_round(c) <= before {
        now += MINUTE;
        c.write("data/drafts/2026-10-01.json", &format!("{{\"steps\": {now}, \"pad\": \"{}\"}}", "x".repeat(100_000)));
        c.exchange(folder, key, now);
        along(now);
    }
    now
}

/// A device's sealed entry, as the window writes it (`devices::change`).
fn entry(c: &Computer, folder: &Path, key: &[u8; 32], kind: &str, f: impl FnOnce(&mut crate::devices::Entry)) {
    let id = c.id.clone();
    crate::devices::change(&crate::devices::own_path(&c.roots.state), Some((folder, key)), |e| {
        e.id = id;
        e.kind = kind.into();
        e.working = true;
        f(e);
    })
    .unwrap();
}

/// How far a device's notes say it read another's.
fn read_of(folder: &Path, reader: &Computer, writer: &Computer) -> u32 {
    read_seen(folder, &reader.id).and_then(|s| s.read.get(&writer.id).copied()).unwrap_or(0)
}

/// A desk writing much, a phone reading it, and an old phone that read the
/// desk's first round, wrote one of its own, and was never heard of again;
/// the desk past a full round, the phone having read past it.
struct Scene {
    base: PathBuf,
    folder: PathBuf,
    key: [u8; 32],
    desk: Computer,
    phone: Computer,
    old: Computer,
    now: i64,
    full: u32,
}

fn scene(name: &str) -> Scene {
    let base = scratch(name);
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone, old) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"), Computer::new(&base, "old"));
    desk.write("config/safe-senders.txt", "friend@example.org\n");
    desk.exchange(&folder, &key, NOW);
    old.write("data/drafts/2026-09-30.json", "{\"steps\": 1}");
    old.exchange(&folder, &key, NOW + MINUTE);
    phone.write("data/drafts/2026-09-29.json", "{\"steps\": 2}");
    phone.exchange(&folder, &key, NOW + 2 * MINUTE);
    let now = grow(&desk, &folder, &key, NOW + 2 * MINUTE, &|now| {
        phone.exchange(&folder, &key, now + 30_000);
    });
    desk.exchange(&folder, &key, now + MINUTE);
    let full = full_round(&desk);
    assert!(full > 2, "a full round opened past the first ones: {full}");
    assert_eq!(read_of(&folder, &old, &desk), 1, "the old phone read the desk's first round only");
    assert!(read_of(&folder, &phone, &desk) >= full, "the phone read past the full round");
    let held = kept(&folder, &desk.id);
    assert_eq!(held[0], 1, "held back by the old phone's notes while it counts: {held:?}");
    assert_eq!(crate::share::rounds_past(&folder, &desk.memory, &desk.id, now + MINUTE), Some(1), "the sending judges as the exchange did");
    Scene { base, folder, key, desk, phone, old, now: now + MINUTE, full }
}

/// The desk's rounds past its last full round go at its next exchange: the
/// device that held them back no longer counts.
fn the_desk_s_past_rounds_go(s: &Scene, at: i64, why: &str) {
    s.desk.exchange(&s.folder, &s.key, at);
    let held = kept(&s.folder, &s.desk.id);
    assert_eq!(held[0], s.full, "{why}: only the last full round and those continuing it: {held:?} (full {})", s.full);
    assert_eq!(crate::share::rounds_past(&s.folder, &s.desk.memory, &s.desk.id, at), Some(s.full), "{why}: the sending judges as the exchange did");
    let memory = Memory::load(&s.desk.memory, &s.desk.id);
    assert!(memory.counting.as_ref().is_some_and(|c| c.contains(&s.phone.id) && !c.contains(&s.old.id)), "{why}: {:?}", memory.counting);
    assert!(memory.uncounted.contains(&s.old.id), "{why}: {:?}", memory.uncounted);
}

/// A device you forgot: its notes, frozen since it died, no longer hold the
/// others' rounds back. Until 16 October 2026 every device whose plain notes
/// were heard within 180 days held them, and a dead phone's marks kept every
/// other device's past rounds in the folder for half a year.
#[test]
fn a_forgotten_device_s_stale_marks_no_longer_hold_the_others_rounds_back() {
    let s = scene("rounds-forgotten");
    forget_device(&s.folder, &s.key, &s.desk.memory, &s.desk.id, &s.old.id).unwrap();
    the_desk_s_past_rounds_go(&s, s.now + MINUTE, "forgotten");
    // Its own first round stays: its last full round.
    assert_eq!(kept(&s.folder, &s.old.id), vec![1]);
    let _ = std::fs::remove_dir_all(&s.base);
}

/// The same for a device whose entry says it left, and for one another
/// device's entry says it replaces (a new phone that took over from it).
#[test]
fn a_device_that_left_or_was_replaced_holds_nothing_back() {
    let s = scene("rounds-left");
    entry(&s.old, &s.folder, &s.key, crate::devices::PHONE, |e| {
        e.working = false;
        e.left = true;
    });
    the_desk_s_past_rounds_go(&s, s.now + MINUTE, "left");
    let _ = std::fs::remove_dir_all(&s.base);

    let s = scene("rounds-replaced");
    let heard = heard_of(&s.folder, &s.key, &s.phone.memory, &s.phone.id, &s.old.id);
    assert!(heard > 0, "the old phone was heard of by its record");
    entry(&s.phone, &s.folder, &s.key, crate::devices::PHONE, |e| {
        e.replaces = vec![crate::devices::Replaced { id: s.old.id.clone(), started: 0, heard }];
    });
    the_desk_s_past_rounds_go(&s, s.now + MINUTE, "replaced");
    let _ = std::fs::remove_dir_all(&s.base);
}

/// A device silent for `SILENT_DAYS` by sealed facts holds nothing back,
/// whatever its plain notes say: a hostile folder writing them fresh changes
/// nothing. A file of notes alone (a device whose entry has not come yet)
/// holds back by its notes, as before: plain notes only ever hold more back.
#[test]
fn a_silent_device_holds_nothing_back_whatever_its_notes_say() {
    let s = scene("rounds-silent");
    let later = s.now + (SILENT_DAYS + 1) * DAY_MS;
    // Notes in the old phone's name, said fresh: plain, and passed over.
    let forged = format!("at = {}\nround = 1\n\n[read]\n\"{}\" = 1\n", later / 1000, s.desk.id);
    put(&s.folder, &format!("{}.toml", s.old.id), forged.as_bytes());
    // The phone heard of lately (its records), and a file of notes alone that says it read nothing.
    s.phone.write("config/safe-senders.txt", "neighbour@example.org\n");
    s.phone.exchange(&s.folder, &s.key, later - MINUTE);
    let stray = uuid::Uuid::new_v4().to_string();
    put(&s.folder, &format!("{stray}.toml"), format!("at = {}\nround = 1\n\n[read]\n\"{}\" = 1\n", later / 1000, s.desk.id).as_bytes());
    s.desk.exchange(&s.folder, &s.key, later);
    assert_eq!(kept(&s.folder, &s.desk.id)[0], 1, "the notes alone hold everything back");
    std::fs::remove_file(s.folder.join(format!("{stray}.toml"))).unwrap();
    the_desk_s_past_rounds_go(&s, later + MINUTE, "silent");
    let _ = std::fs::remove_dir_all(&s.base);
}

/// A live phone's rounds wait for every device that counts: one that read
/// less holds the rest back, and one whose notes the folder does not show
/// (a sync app late, a hostile server hiding them) holds them all back.
/// Until 16 October 2026 a device without notes in the folder counted for
/// nothing, and its rounds went before it read them.
#[test]
fn a_live_phone_s_rounds_wait_for_every_device_that_counts() {
    let base = scratch("rounds-live-phone");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (phone, desk) = (Computer::new(&base, "phone"), Computer::new(&base, "desk"));
    desk.write("config/safe-senders.txt", "friend@example.org\n");
    desk.exchange(&folder, &key, NOW);
    phone.exchange(&folder, &key, NOW + MINUTE);
    // The desk reads the phone along for the first rounds only.
    let read_until = NOW + 5 * MINUTE;
    let now = grow(&phone, &folder, &key, NOW + MINUTE, &|now| {
        if now <= read_until {
            desk.exchange(&folder, &key, now + 30_000);
        }
    });
    phone.exchange(&folder, &key, now + MINUTE);
    let full = full_round(&phone);
    let desk_read = read_of(&folder, &desk, &phone);
    assert!(desk_read > 1 && desk_read < full, "{desk_read}, full {full}");
    assert_eq!(kept(&folder, &phone.id)[0], desk_read, "rounds the desk has not read past stay");
    // Its notes hidden: the desk still counts (its records), and holds every round back.
    let notes = folder.join(format!("{}.toml", desk.id));
    let hidden = std::fs::read(&notes).unwrap();
    std::fs::remove_file(&notes).unwrap();
    phone.exchange(&folder, &key, now + 2 * MINUTE);
    assert_eq!(kept(&folder, &phone.id)[0], desk_read, "nothing more goes while the desk's notes are not here");
    assert_eq!(crate::share::rounds_past(&folder, &phone.memory, &phone.id, now + 2 * MINUTE), Some(0));
    // Back, and the desk reads on: the rounds before the full one go.
    std::fs::write(&notes, hidden).unwrap();
    desk.exchange(&folder, &key, now + 3 * MINUTE);
    phone.exchange(&folder, &key, now + 4 * MINUTE);
    assert_eq!(kept(&folder, &phone.id)[0], full);
    let _ = std::fs::remove_dir_all(&base);
}

/// A device forgotten that comes back counts again at once (it says
/// something newer), reads the others from the oldest round kept, whether
/// that round is full or continues one gone, and finds every entry there:
/// the doses' record, a list, a draft, a note changed twice while it was
/// away. Nothing is said lost, and its next exchange holds the others'
/// rounds back again.
#[test]
fn a_forgotten_device_that_comes_back_finds_every_entry() {
    let base = scratch("rounds-back");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"), Computer::new(&base, "laptop"));
    let (desk_notes, laptop_notes, phone_notes) = (base.join("desk-notes"), base.join("laptop-notes"), base.join("phone-notes"));
    let notes = ["notes"];
    desk.write("config/safe-senders.txt", "friend@example.org\n");
    desk.write("state/health-state.toml", "[taken]\n\"levo@1790000000\" = 1790000040\n");
    put(&desk_notes, "lease.md", b"the lease, first draft\n");
    with_parts(&desk, &folder, &key, NOW, &desk_notes, &notes);
    // The laptop reads it all, writes a draft of its own, and is put away.
    laptop.write("data/drafts/2026-09-30.json", "{\"steps\": 1}");
    with_parts(&laptop, &folder, &key, NOW + MINUTE, &laptop_notes, &notes);
    assert_eq!(std::fs::read_to_string(laptop_notes.join("lease.md")).unwrap(), "the lease, first draft\n");
    with_parts(&phone, &folder, &key, NOW + 2 * MINUTE, &phone_notes, &notes);
    // Forgotten once each has heard all it said: its draft's record.
    with_parts(&desk, &folder, &key, NOW + 2 * MINUTE + 10_000, &desk_notes, &notes);
    forget_device(&folder, &key, &desk.memory, &desk.id, &laptop.id).unwrap();
    forget_device(&folder, &key, &phone.memory, &phone.id, &laptop.id).unwrap();
    // Meanwhile: the note changed twice, a dose marked, a sender added.
    put(&desk_notes, "lease.md", b"the lease, second draft\n");
    with_parts(&desk, &folder, &key, NOW + 3 * MINUTE, &desk_notes, &notes);
    put(&desk_notes, "lease.md", b"the lease, signed\n");
    desk.write("state/health-state.toml", "[taken]\n\"levo@1790000000\" = 1790000040\n\"levo@1790086400\" = 1790086460\n");
    desk.write("config/safe-senders.txt", "friend@example.org\nneighbour@example.org\n");
    with_parts(&desk, &folder, &key, NOW + 4 * MINUTE, &desk_notes, &notes);
    // The desk past a full round; the phone reads along, then only to a round before it.
    let read_until = NOW + 8 * MINUTE;
    let now = grow(&desk, &folder, &key, NOW + 4 * MINUTE, &|now| {
        if now <= read_until {
            with_parts(&phone, &folder, &key, now + 30_000, &phone_notes, &notes);
        }
    });
    with_parts(&desk, &folder, &key, now + MINUTE, &desk_notes, &notes);
    let full = full_round(&desk);
    let oldest = kept(&folder, &desk.id)[0];
    assert!(oldest > 1 && oldest < full, "the oldest kept continues a round gone, before the full one: {oldest}, full {full}");
    // The laptop comes back.
    let outcome = with_parts(&laptop, &folder, &key, now + 2 * MINUTE, &laptop_notes, &notes);
    assert!(!outcome.problems.iter().any(|p| p.starts_with("share-other")), "nothing said lost: {:?}", outcome.problems);
    // Its entry, as its window writes it after each exchange: something newer than when it was forgotten.
    entry(&laptop, &folder, &key, crate::devices::COMPUTER, |e| e.exported(now + 2 * MINUTE, outcome.wrote, (now + 2 * MINUTE) / 1000));
    for file in ["config/safe-senders.txt", "state/health-state.toml", "data/drafts/2026-10-01.json"] {
        assert_eq!(laptop.read(file), desk.read(file), "{file}");
    }
    assert_eq!(std::fs::read_to_string(laptop_notes.join("lease.md")).unwrap(), "the lease, signed\n");
    assert!(heard(&laptop.memory, &laptop.id).complete(&desk.id, written(&desk.memory, &desk.id).unwrap()), "everything the desk wrote is known there");
    let copies: Vec<String> = std::fs::read_dir(&laptop_notes).unwrap().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.contains("conflict")).collect();
    assert!(copies.is_empty(), "its own last version was the desk's: no conflicted copy: {copies:?}");
    // It counts again on the desk, and holds the desk's rounds back from where its notes say.
    with_parts(&desk, &folder, &key, now + 3 * MINUTE, &desk_notes, &notes);
    let memory = Memory::load(&desk.memory, &desk.id);
    assert!(memory.counting.as_ref().is_some_and(|c| c.contains(&laptop.id)), "{:?}", memory.counting);
    assert_eq!(kept(&folder, &desk.id)[0], oldest, "the phone still holds the rest back");
    let _ = std::fs::remove_dir_all(&base);
}

/// The past rounds of a device that no longer counts (a phone dead for
/// good, forgotten) go, taken out by a device that counts: only those below
/// its last full round, never that round nor those continuing it, and only
/// once every device that counts read it past them. A device joining later
/// reads every entry of it from what is kept.
#[test]
fn a_dead_device_s_past_rounds_go_never_its_last_full_round() {
    let base = scratch("rounds-dead");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone, old) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"), Computer::new(&base, "old"));
    desk.write("config/safe-senders.txt", "friend@example.org\n");
    desk.exchange(&folder, &key, NOW);
    phone.exchange(&folder, &key, NOW + 30_000);
    old.write("config/known-senders.txt", "shop@example.org\n");
    // The old phone past a full round; the desk reads along; the phone to a round before it.
    let read_until = NOW + 6 * MINUTE;
    let now = grow(&old, &folder, &key, NOW + MINUTE, &|now| {
        desk.exchange(&folder, &key, now + 20_000);
        if now <= read_until {
            phone.exchange(&folder, &key, now + 40_000);
        }
    });
    let full = full_round(&old);
    // One more line past it, in a round continuing it, then it dies.
    old.write("config/known-senders.txt", "shop@example.org\nbaker@example.org\n");
    old.exchange(&folder, &key, now + MINUTE);
    desk.exchange(&folder, &key, now + 2 * MINUTE);
    // Alive, it took out its own rounds below the phone's mark; the rest wait for the phone.
    let all = kept(&folder, &old.id);
    let phone_read = read_of(&folder, &phone, &old);
    assert!(phone_read > 1 && phone_read < full, "{phone_read}");
    assert!(all[0] == phone_read && all.contains(&full) && *all.last().unwrap() > full, "{all:?}, full {full}");
    // While it counts, nothing of it goes (it never takes its own out again).
    desk.exchange(&folder, &key, now + 3 * MINUTE);
    assert_eq!(kept(&folder, &old.id), all, "nothing taken out of a device that counts");
    // Forgotten on the desk: its rounds below the phone's mark may go, and none is there; the rest wait.
    forget_device(&folder, &key, &desk.memory, &desk.id, &old.id).unwrap();
    desk.exchange(&folder, &key, now + 4 * MINUTE);
    assert_eq!(kept(&folder, &old.id), all, "the phone has not read past the rest");
    assert_eq!(Memory::load(&desk.memory, &desk.id).departed.get(&old.id), Some(&phone_read));
    // The phone reads on: everything below the last full round goes, never that round nor the one continuing it.
    phone.exchange(&folder, &key, now + 5 * MINUTE);
    desk.exchange(&folder, &key, now + 6 * MINUTE);
    let held = kept(&folder, &old.id);
    assert_eq!(held, all.iter().copied().filter(|r| *r >= full).collect::<Vec<_>>(), "full {full}");
    assert_eq!(crate::share::departed_past(&desk.memory, &desk.id).get(&old.id), Some(&full));
    // A device joining now reads every entry of it from what is kept.
    let late = Computer::new(&base, "late");
    let outcome = late.exchange(&folder, &key, now + 7 * MINUTE);
    assert!(!outcome.problems.iter().any(|p| p.starts_with("share-other")), "{:?}", outcome.problems);
    // Each entry, whatever the order of a list's lines.
    let lines = |text: String| text.lines().map(str::to_string).collect::<BTreeSet<String>>();
    for file in ["config/known-senders.txt", "data/drafts/2026-10-01.json", "config/safe-senders.txt"] {
        assert!(!late.read(file).is_empty() && lines(late.read(file)) == lines(desk.read(file)), "{file}: {:?} there, {:?} on the desk", late.read(file), desk.read(file));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The sealed files of a device that no longer counts, that no record names
/// any more, go `unused_days` after their last use, swept by a device that
/// counts: never one a current record names, a texts' medium included
/// (`texts_media`), and nothing at all on a device that cannot know the
/// texts' media (it does not share the texts, and a phone may). Each one
/// swept is said for the server (`Memory::taken`).
#[test]
fn a_dead_device_s_unused_sealed_files_go_never_a_content_named() {
    use sioul_core::texts;
    let base = scratch("rounds-dead-sealed");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let seal = crate::textseal::TextSeal::new(&key);
    let (desk, laptop, phone, old) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"), Computer::new(&base, "phone"), Computer::new(&base, "old"));
    let (dn, ln, pn, on) = (base.join("desk-notes"), base.join("laptop-notes"), base.join("phone-notes"), base.join("old-notes"));
    let (desk_on, notes_on, texts_on): (&[&str], &[&str], &[&str]) = (&["notes", texts::PART], &["notes"], &[texts::PART]);
    let picture = b"a picture sent by text, the same bytes as a note of the old computer's\n".repeat(40);
    let (first, second, stays, medium) = (content_name(&base, &key, b"first version\n"), content_name(&base, &key, b"second version\n"), content_name(&base, &key, b"a note that stays\n"), content_name(&base, &key, &picture));
    // The old computer writes three notes, replaces one, and takes out the one that is also a picture.
    put(&on, "a.md", b"first version\n");
    put(&on, "b.md", b"a note that stays\n");
    put(&on, "c.md", &picture);
    with_parts(&old, &folder, &key, NOW, &on, notes_on);
    with_parts(&desk, &folder, &key, NOW + MINUTE, &dn, desk_on);
    with_parts(&laptop, &folder, &key, NOW + MINUTE + 10_000, &ln, notes_on);
    put(&on, "a.md", b"second version\n");
    std::fs::remove_file(on.join("c.md")).unwrap();
    with_parts(&old, &folder, &key, NOW + 2 * MINUTE, &on, notes_on);
    // A file gone is taken out once gone a while (`EMPTIED_WAIT`).
    with_parts(&old, &folder, &key, NOW + 13 * MINUTE, &on, notes_on);
    with_parts(&desk, &folder, &key, NOW + 14 * MINUTE, &dn, desk_on);
    with_parts(&laptop, &folder, &key, NOW + 14 * MINUTE + 10_000, &ln, notes_on);
    assert!(!dn.join("c.md").exists() && std::fs::read_to_string(dn.join("a.md")).unwrap() == "second version\n");
    // The phone's text with that picture: named by a current record.
    let own = texts::own_file(&phone.roots.data.join(texts::FOLDER), texts::LOG, &phone.id);
    let path = base.join("picture");
    std::fs::write(&path, &picture).unwrap();
    let (hash, size) = crate::blobs::hash_file(&path).unwrap();
    assert_eq!(medium, crate::blobs::name(&key, &hash));
    let part = texts::Part { seq: 0, ct: "image/jpeg".into(), hash, size, state: "here".into(), ..texts::Part::default() };
    let text = texts::Text { id: "mms-1@1790000000000".into(), at: NOW, direction: "in".into(), with: vec!["+33199001234".into()], parts: vec![part], ..texts::Text::default() };
    let id = |t: &texts::Text| t.line_id();
    texts::append(&own, std::slice::from_ref(&text), &seal, Some(&id)).unwrap();
    with_parts(&phone, &folder, &key, NOW + 15 * MINUTE, &pn, texts_on);
    with_parts(&desk, &folder, &key, NOW + 16 * MINUTE, &dn, desk_on);
    for name in [&first, &second, &stays, &medium] {
        assert!(folder.join("blobs").join(name).exists(), "sealed by the old computer: {name}");
    }
    let theirs = Memory::load_all(&desk.memory, &desk.id).sealed.theirs;
    assert!(theirs.get(&first).is_some_and(|(w, _)| *w == old.id) && theirs.get(&medium).is_some_and(|(w, _)| *w == old.id), "{theirs:?}");
    assert!(!theirs.contains_key(&second) && !theirs.contains_key(&stays), "current: not noted");
    // The old computer dies; forgotten on the desk and the laptop. Its files sealed long ago.
    for c in [&desk, &laptop] {
        forget_device(&folder, &key, &c.memory, &c.id, &old.id).unwrap();
    }
    for name in [&first, &second, &stays, &medium] {
        sealed_long_ago(&folder, name);
    }
    // Ninety-one days on (the devices say nothing of how they keep sealed files: 90 days).
    let later = NOW + 91 * DAY_MS;
    with_parts(&phone, &folder, &key, later, &pn, texts_on);
    // The laptop does not share the texts, and a phone may name a medium: it sweeps nothing.
    with_parts(&laptop, &folder, &key, later + MINUTE, &ln, notes_on);
    assert!(folder.join("blobs").join(&first).exists(), "the laptop cannot know the texts' media");
    assert!(Memory::load(&laptop.memory, &laptop.id).taken.is_empty());
    // The desk shares them: the replaced version goes, never the medium nor a current note.
    with_parts(&desk, &folder, &key, later + 2 * MINUTE, &dn, desk_on);
    assert!(!folder.join("blobs").join(&first).exists(), "replaced, named by no record: swept");
    for (name, why) in [(&second, "current"), (&stays, "current"), (&medium, "a text's medium")] {
        assert!(folder.join("blobs").join(name).exists(), "{why}: kept");
    }
    let memory = Memory::load_all(&desk.memory, &desk.id);
    assert_eq!(memory.taken.keys().collect::<Vec<_>>(), vec![&first], "said for the server");
    assert_eq!(crate::share::swept_theirs(&desk.memory, &desk.id).len(), 1);
    assert!(!memory.sealed.theirs.contains_key(&first) && memory.sealed.theirs.contains_key(&medium));
    let _ = std::fs::remove_dir_all(&base);
}

/// A content's sealed file's name in the folder.
fn content_name(base: &Path, key: &[u8; 32], bytes: &[u8]) -> String {
    let path = base.join("content");
    std::fs::write(&path, bytes).unwrap();
    let (hash, _) = crate::blobs::hash_file(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    crate::blobs::name(key, &hash)
}

/// A sealed file of the folder, dated a hundred days ago (`blobs::sweep`'s
/// care: a fresh one is another device's use).
fn sealed_long_ago(folder: &Path, name: &str) {
    let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(100 * 86_400);
    std::fs::File::options().write(true).open(folder.join("blobs").join(name)).unwrap().set_modified(long_ago).unwrap();
}

/// Among computers only, a phone never seen (every device known by its
/// entry, none a phone), the texts hold no media: a computer that does not
/// share the texts sweeps a dead computer's replaced contents all the same.
/// Not before it is forgotten, and not while it is used again by a record.
#[test]
fn among_computers_a_dead_one_s_replaced_contents_go_when_forgotten() {
    let base = scratch("rounds-dead-computer");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, old) = (Computer::new(&base, "desk"), Computer::new(&base, "old"));
    let (dn, on) = (base.join("desk-notes"), base.join("old-notes"));
    let notes = ["notes"];
    for c in [&desk, &old] {
        entry(c, &folder, &key, crate::devices::COMPUTER, |_| {});
    }
    put(&on, "a.md", b"first version\n");
    with_parts(&old, &folder, &key, NOW, &on, &notes);
    with_parts(&desk, &folder, &key, NOW + MINUTE, &dn, &notes);
    put(&on, "a.md", b"second version\n");
    with_parts(&old, &folder, &key, NOW + 2 * MINUTE, &on, &notes);
    with_parts(&desk, &folder, &key, NOW + 3 * MINUTE, &dn, &notes);
    let first = content_name(&base, &key, b"first version\n");
    sealed_long_ago(&folder, &first);
    // Not forgotten: its writer's to sweep.
    with_parts(&desk, &folder, &key, NOW + 91 * DAY_MS, &dn, &notes);
    assert!(folder.join("blobs").join(&first).exists(), "a writer that counts sweeps its own");
    forget_device(&folder, &key, &desk.memory, &desk.id, &old.id).unwrap();
    with_parts(&desk, &folder, &key, NOW + 91 * DAY_MS + MINUTE, &dn, &notes);
    assert!(!folder.join("blobs").join(&first).exists(), "forgotten: its replaced content swept");
    assert!(dn.join("a.md").exists() && std::fs::read_to_string(dn.join("a.md")).unwrap() == "second version\n");
    let _ = std::fs::remove_dir_all(&base);
}
