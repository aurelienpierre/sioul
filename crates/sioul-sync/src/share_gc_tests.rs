// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What you remove leaves the folder by itself, and a device's files can be
//! taken out with it (docs/database.md, "Removed notes" and "Removing a
//! device's files"; 17 October 2026): notes and papers gone many at once
//! leave your other devices two weeks after they went (`GONE_GRACE`),
//! unless they come back, said meanwhile (`withheld`) and put back on a
//! word (`put_back_withheld`); a folder missing or unreadable whole never
//! leaves; the content of a file taken out leaves the folder as soon as
//! every device that counts read so, once each keeps such a file as a copy
//! (`removed_due`, `blobs::KEEPING_MOVED`), else in its time; a note edited
//! many times keeps its earlier versions by reference twelve days, their
//! contents two weeks; "Remove its files" takes out every round of a device
//! that no longer counts, its claims, its note and its entry once nothing
//! needs them, and the contents of its last words no other record names,
//! never a text's medium, never for a device that counts, and a device
//! still in use shares again. Fiction only.

use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;

const DAY_MS: i64 = 24 * 60 * MINUTE;

fn put(dir: &Path, relative: &str, bytes: &[u8]) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A computer's stores with `notes` as its notes folder: notes and papers
/// shared besides the parts shared by default, the texts too when `texts`.
fn stores_with(c: &Computer, notes: &Path, texts: bool) -> Vec<Store> {
    let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
    stores_of(&config, &c.roots, &|part| matches!(part, "notes" | "papers") || (texts && part == sioul_core::texts::PART) || shared_by_default(part, &config))
}

fn with_notes(c: &Computer, folder: &Path, key: &[u8; 32], now: i64, notes: &Path) -> Outcome {
    exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: true, hurry: None }, &stores_with(c, notes, false), now).unwrap()
}

fn with_texts(c: &Computer, folder: &Path, key: &[u8; 32], now: i64, notes: &Path) -> Outcome {
    exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: true, hurry: None }, &stores_with(c, notes, true), now).unwrap()
}

/// A device's sealed entry, as the window writes it: a computer (or `kind`)
/// that keeps sealed files as `keeping` says (`blobs::KEEPING`: this Sioul).
fn says(c: &Computer, folder: &Path, key: &[u8; 32], kind: &str, keeping: u32) {
    let id = c.id.clone();
    crate::devices::change(&crate::devices::own_path(&c.roots.state), Some((folder, key)), |e| {
        e.id = id;
        e.kind = kind.into();
        e.working = true;
        e.keeping = keeping;
    })
    .unwrap();
}

/// A content's sealed file's name in the folder.
fn content_name(base: &Path, key: &[u8; 32], bytes: &[u8]) -> String {
    let path = base.join("content");
    std::fs::write(&path, bytes).unwrap();
    let (hash, _) = crate::blobs::hash_file(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    crate::blobs::name(key, &hash)
}

/// A sealed file of the folder, dated a hundred days ago (the sweep's care:
/// one made fresh lately is another device's use).
fn sealed_long_ago(folder: &Path, name: &str) {
    let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(100 * 86_400);
    std::fs::File::options().write(true).open(folder.join("blobs").join(name)).unwrap().set_modified(long_ago).unwrap();
}

fn sealed(folder: &Path, name: &str) -> bool {
    folder.join("blobs").join(name).exists()
}

/// The files of a notes folder, hidden ones left out, by their path from it.
fn names_in(dir: &Path) -> BTreeSet<String> {
    let mut found = Vec::new();
    list_files(dir, dir, &[], &mut found);
    found.into_iter().map(|(name, _)| name).collect()
}

/// Changes on `c` until its records go on in a round past `round` (each
/// round goes on in a new one past `SEGMENT`); the time reached.
fn past_round(c: &Computer, folder: &Path, key: &[u8; 32], notes: &Path, mut now: i64, round: u32) -> i64 {
    let mut n = 0;
    while Memory::load(&c.memory, &c.id).round <= round {
        n += 1;
        assert!(n < 10, "a round past {round}");
        now += MINUTE;
        c.write("data/drafts/2026-10-01.json", &format!("{{\"steps\": {n}, \"pad\": \"{}\"}}", "x".repeat(40_000)));
        with_notes(c, folder, key, now, notes);
    }
    now
}

/// How far a device's notes in the folder say it read another's.
fn read_of(folder: &Path, reader: &Computer, writer: &Computer) -> u32 {
    read_seen(folder, &reader.id).and_then(|s| s.read.get(&writer.id).copied()).unwrap_or(0)
}

/// Notes deleted in bulk are held two weeks from when they went, said
/// meanwhile with the folders most were in, then leave your other devices
/// by themselves, nothing pressed: each kept there among its earlier
/// versions, as a copy.
#[test]
fn notes_gone_at_once_leave_by_themselves_after_two_weeks() {
    let base = scratch("gc-grace");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    for c in [&desk, &laptop] {
        says(c, &folder, &key, crate::devices::COMPUTER, crate::blobs::KEEPING);
    }
    put(&dn, "today.md", b"# Today\n");
    for n in 0..20 {
        put(&dn, &format!("archive/2026/{n}.md"), format!("# {n}\n").as_bytes());
    }
    for n in 0..3 {
        put(&dn, &format!("old/{n}.md"), format!("# old {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    assert_eq!(names_in(&ln).len(), 24);
    // Deleted in bulk here: held, said.
    std::fs::remove_dir_all(dn.join("archive")).unwrap();
    std::fs::remove_dir_all(dn.join("old")).unwrap();
    let went = NOW + 2 * MINUTE;
    let outcome = with_notes(&desk, &folder, &key, went, &dn);
    assert!(outcome.sent == 0 && outcome.problems.iter().any(|p| p == "share-vanished:files/notes/:23"), "{outcome:?}");
    let held = outcome.withheld.clone().unwrap();
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!((held[0].store.as_str(), held[0].count, held[0].first, held[0].last, held[0].leaves), ("files/notes/", 23, went, went, went + 14 * DAY_MS));
    assert_eq!(held[0].folders, vec!["archive/2026".to_string()], "the deepest folder that holds four in five of them (`bulk_folders`)");
    assert_eq!(withheld(&desk.memory, &desk.id), held, "kept for the window");
    // Until two weeks have passed: nothing taken out there.
    for at in [went + MINUTE, went + 13 * DAY_MS, went + 14 * DAY_MS - MINUTE] {
        let outcome = with_notes(&desk, &folder, &key, at, &dn);
        assert_eq!(outcome.sent, 0, "{outcome:?}");
        assert_eq!(outcome.withheld.as_ref().map(|w| w[0].count), Some(23));
    }
    with_notes(&laptop, &folder, &key, went + 14 * DAY_MS - 30_000, &ln);
    assert_eq!(names_in(&ln).len(), 24, "held two weeks");
    // Two weeks on: they go by themselves, nothing pressed, nothing said.
    let outcome = with_notes(&desk, &folder, &key, went + 14 * DAY_MS, &dn);
    assert!(outcome.sent >= 23 && !outcome.problems.iter().any(|p| p.starts_with("share-vanished")), "{outcome:?}");
    assert_eq!(outcome.withheld, Some(Vec::new()));
    assert!(withheld(&desk.memory, &desk.id).is_empty());
    with_notes(&laptop, &folder, &key, went + 14 * DAY_MS + MINUTE, &ln);
    assert_eq!(names_in(&ln), BTreeSet::from(["today.md".to_string()]));
    // Each kept on the laptop among its earlier versions, as a copy (`history::keep_moved`).
    let kept = crate::history::files(&crate::history::root(&laptop.memory), "notes");
    assert_eq!(kept.len(), 23, "{kept:?}");
    assert!(kept.iter().all(|(_, versions)| versions.len() == 1 && versions[0].sealed.is_none()), "{kept:?}");
    // Their contents leave the folder once the laptop read past the round that took them out.
    let names: Vec<String> = (0..20).map(|n| content_name(&base, &key, format!("# {n}\n").as_bytes())).chain((0..3).map(|n| content_name(&base, &key, format!("# old {n}\n").as_bytes()))).collect();
    for name in &names {
        sealed_long_ago(&folder, name);
    }
    let round = Memory::load(&desk.memory, &desk.id).round;
    let now = past_round(&desk, &folder, &key, &dn, went + 14 * DAY_MS + 2 * MINUTE, round);
    with_notes(&laptop, &folder, &key, now + 10_000, &ln);
    with_notes(&desk, &folder, &key, now + MINUTE, &dn);
    assert!(names.iter().all(|n| !sealed(&folder, n)), "every one taken out: their contents gone");
    assert!(sealed(&folder, &content_name(&base, &key, b"# Today\n")), "the note that stays keeps its content");
    let _ = std::fs::remove_dir_all(&base);
}

/// "Put them back": the files held come back where they were, from their
/// sealed contents, dated as they were; nothing leaves. One whose place is
/// taken meanwhile is left as it is.
#[test]
fn notes_held_come_back_when_put_back() {
    let base = scratch("gc-put-back");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    put(&dn, "today.md", b"# Today\n");
    for n in 0..12 {
        put(&dn, &format!("trip/{n}.md"), format!("# day {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    let before: Vec<std::time::SystemTime> = (0..12).map(|n| std::fs::metadata(dn.join(format!("trip/{n}.md"))).unwrap().modified().unwrap()).collect();
    std::fs::remove_dir_all(dn.join("trip")).unwrap();
    let outcome = with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn);
    assert_eq!(outcome.withheld.as_ref().map(|w| w[0].count), Some(12), "{outcome:?}");
    // One place taken meanwhile.
    put(&dn, "trip/0.md", b"# written anew\n");
    let stores = stores_with(&desk, &dn, false);
    assert_eq!(put_back_withheld(&folder, &key, &desk.memory, &desk.id, &stores, "files/notes/"), Ok(11));
    assert_eq!(std::fs::read(dn.join("trip/0.md")).unwrap(), b"# written anew\n", "its place taken: left as it is");
    for n in 1..12 {
        assert_eq!(std::fs::read(dn.join(format!("trip/{n}.md"))).unwrap(), format!("# day {n}\n").as_bytes());
        assert_eq!(std::fs::metadata(dn.join(format!("trip/{n}.md"))).unwrap().modified().unwrap(), before[n], "dated as it was");
    }
    // Found back: nothing leaves, nothing held; the one written anew goes as a change.
    let outcome = with_notes(&desk, &folder, &key, NOW + 3 * MINUTE, &dn);
    assert!(outcome.sent == 1 && !outcome.problems.iter().any(|p| p.starts_with("share-vanished")), "{outcome:?}");
    assert_eq!(outcome.withheld, Some(Vec::new()));
    with_notes(&laptop, &folder, &key, NOW + 15 * DAY_MS, &ln);
    assert_eq!(names_in(&ln).len(), 13);
    assert_eq!(std::fs::read(ln.join("trip/0.md")).unwrap(), b"# written anew\n");
    let _ = std::fs::remove_dir_all(&base);
}

/// "Put them back" never writes through a link put where a folder was (a
/// link to elsewhere would carry the files out of the notes), as nothing the
/// sharing writes does (`writable`).
#[cfg(unix)]
#[test]
fn notes_held_are_never_put_back_through_a_link() {
    let base = scratch("gc-put-back-link");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln, elsewhere) = (base.join("desk-notes"), base.join("laptop-notes"), base.join("elsewhere"));
    put(&dn, "today.md", b"# Today\n");
    for n in 0..12 {
        put(&dn, &format!("trip/{n}.md"), format!("# day {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    std::fs::remove_dir_all(dn.join("trip")).unwrap();
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn).withheld.map(|w| w[0].count), Some(12));
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dn.join("trip")).unwrap();
    let stores = stores_with(&desk, &dn, false);
    assert_eq!(put_back_withheld(&folder, &key, &desk.memory, &desk.id, &stores, "files/notes/"), Ok(0));
    assert!(names_in(&elsewhere).is_empty(), "nothing written through the link");
    let _ = std::fs::remove_dir_all(&base);
}

/// A notes folder missing whole (a disk not mounted, a folder moved away) is
/// not a deletion: its files never leave, two weeks on or two months on,
/// and nothing is said to leave.
#[test]
fn a_notes_folder_missing_whole_never_leaves() {
    let base = scratch("gc-missing");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    for n in 0..6 {
        put(&dn, &format!("note-{n}.md"), format!("# {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    std::fs::remove_dir_all(&dn).unwrap();
    for at in [NOW + 2 * MINUTE, NOW + 15 * DAY_MS, NOW + 60 * DAY_MS] {
        let outcome = with_notes(&desk, &folder, &key, at, &dn);
        assert_eq!(outcome.sent, 0, "{outcome:?}");
        assert_eq!(outcome.withheld, Some(Vec::new()), "unknown, not gone: nothing to leave");
    }
    // Emptied whole, the same.
    std::fs::create_dir_all(&dn).unwrap();
    for at in [NOW + 61 * DAY_MS, NOW + 80 * DAY_MS] {
        assert_eq!(with_notes(&desk, &folder, &key, at, &dn).sent, 0);
    }
    with_notes(&laptop, &folder, &key, NOW + 81 * DAY_MS, &ln);
    assert_eq!(names_in(&ln).len(), 6);
    let _ = std::fs::remove_dir_all(&base);
}

/// A note and a paper deleted alone go once gone a while (`EMPTIED_WAIT`);
/// their contents leave the folder as soon as every device that counts read
/// the round that took them out (strictly past it), not two weeks later,
/// once each says it keeps a file taken out as a copy; the laptop keeps
/// both among its earlier versions, as copies, which put back without the
/// folder.
#[test]
fn a_note_and_a_paper_taken_out_free_their_contents_once_every_device_read_so() {
    let base = scratch("gc-early");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    for c in [&desk, &laptop] {
        says(c, &folder, &key, crate::devices::COMPUTER, crate::blobs::KEEPING);
    }
    let paper = b"%PDF-1.4 an invoice, fiction\n".repeat(20);
    put(&dn, "a.md", b"# A note to delete\n");
    put(&dn, "b.md", b"# A note that stays\n");
    put(&dn, "papers/invoice.pdf", &paper);
    // Not the only paper: a folder found empty is not a deletion (`share-vanished`).
    put(&dn, "papers/lease.pdf", b"%PDF-1.4 a lease, fiction\n");
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    let (a, p, b) = (content_name(&base, &key, b"# A note to delete\n"), content_name(&base, &key, &paper), content_name(&base, &key, b"# A note that stays\n"));
    for name in [&a, &p, &b] {
        sealed_long_ago(&folder, name);
    }
    std::fs::remove_file(dn.join("a.md")).unwrap();
    std::fs::remove_file(dn.join("papers/invoice.pdf")).unwrap();
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn).sent, 0, "held a while");
    let outcome = with_notes(&desk, &folder, &key, NOW + 13 * MINUTE, &dn);
    assert_eq!(outcome.sent, 2, "{outcome:?}");
    let round = Memory::load(&desk.memory, &desk.id).round;
    let removed = Memory::load_all(&desk.memory, &desk.id).sealed.removed;
    assert_eq!(removed.get(&a), Some(&(desk.id.clone(), round)), "{removed:?}");
    assert_eq!(removed.get(&p), Some(&(desk.id.clone(), round)));
    // The laptop reads the take-outs: each kept as a copy there.
    with_notes(&laptop, &folder, &key, NOW + 14 * MINUTE, &ln);
    assert_eq!(names_in(&ln), BTreeSet::from(["b.md".to_string(), "papers/lease.pdf".to_string()]));
    let root = crate::history::root(&laptop.memory);
    for (part, file) in [("notes", "files/notes/a.md"), ("papers", "files/papers/invoice.pdf")] {
        let versions = crate::history::versions(&root, part, file);
        assert!(versions.len() == 1 && versions[0].sealed.is_none(), "{file}: {versions:?}");
    }
    // Read into that round only: kept (a reader enters the next round once it read this one to its end).
    assert_eq!(read_of(&folder, &laptop, &desk), round);
    with_notes(&desk, &folder, &key, NOW + 15 * MINUTE, &dn);
    assert!(sealed(&folder, &a) && sealed(&folder, &p), "not read past yet");
    // The desk's records go on in a later round, which the laptop reads: the contents leave the folder.
    let now = past_round(&desk, &folder, &key, &dn, NOW + 15 * MINUTE, round);
    with_notes(&laptop, &folder, &key, now + 10_000, &ln);
    assert!(read_of(&folder, &laptop, &desk) > round);
    assert!(sealed(&folder, &a) && sealed(&folder, &p), "only the writer sweeps its own");
    with_notes(&desk, &folder, &key, now + MINUTE, &dn);
    assert!(!sealed(&folder, &a) && !sealed(&folder, &p), "every device read so: gone, days early");
    assert!(sealed(&folder, &b), "a current note's content stays");
    assert!(Memory::load_all(&desk.memory, &desk.id).sealed.removed.is_empty());
    // Their copies put back on the laptop without the folder.
    let version = crate::history::versions(&root, "notes", "files/notes/a.md")[0].stamp.clone();
    match crate::history::version(&root, "notes", "files/notes/a.md", &version) {
        Some(crate::history::Kept::Copy(path)) => assert_eq!(std::fs::read(path).unwrap(), b"# A note to delete\n"),
        other => panic!("a copy: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// While a device that counts keeps a file taken out by reference (a Sioul
/// before 17 October 2026: `blobs::KEEPING_TWO_WEEKS`), the content of one
/// taken out stays as replaced contents do, two weeks from its last use,
/// even once every device read so.
#[test]
fn a_content_taken_out_stays_its_time_while_an_earlier_sioul_counts() {
    let base = scratch("gc-early-older");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    says(&desk, &folder, &key, crate::devices::COMPUTER, crate::blobs::KEEPING);
    says(&laptop, &folder, &key, crate::devices::COMPUTER, crate::blobs::KEEPING_TWO_WEEKS);
    put(&dn, "a.md", b"# A note to delete\n");
    put(&dn, "b.md", b"# A note that stays\n");
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    let a = content_name(&base, &key, b"# A note to delete\n");
    sealed_long_ago(&folder, &a);
    std::fs::remove_file(dn.join("a.md")).unwrap();
    with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn);
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 13 * MINUTE, &dn).sent, 1);
    let round = Memory::load(&desk.memory, &desk.id).round;
    let now = past_round(&desk, &folder, &key, &dn, NOW + 13 * MINUTE, round);
    with_notes(&laptop, &folder, &key, now + 10_000, &ln);
    assert!(read_of(&folder, &laptop, &desk) > round);
    with_notes(&desk, &folder, &key, now + MINUTE, &dn);
    assert!(sealed(&folder, &a), "an earlier Sioul may keep it by reference: not early");
    with_notes(&desk, &folder, &key, NOW + 12 * DAY_MS, &dn);
    assert!(sealed(&folder, &a), "within two weeks of its last use");
    with_notes(&desk, &folder, &key, NOW + 15 * DAY_MS, &dn);
    assert!(!sealed(&folder, &a), "two weeks on: swept as any replaced content");
    let _ = std::fs::remove_dir_all(&base);
}

/// A note edited many times: the laptop keeps its earlier versions by
/// reference, twelve days (`history::SEALED_DAYS`); their contents leave
/// the folder two weeks after each was replaced, after those references:
/// never a reference whose content went. The current one stays.
#[test]
fn a_note_edited_many_times_keeps_its_versions_then_their_contents_go() {
    let base = scratch("gc-versions");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    for c in [&desk, &laptop] {
        says(c, &folder, &key, crate::devices::COMPUTER, crate::blobs::KEEPING);
    }
    let text = |v: usize| format!("# Plans\n\nversion {v}\n");
    let mut now = NOW;
    for v in 1..=5 {
        put(&dn, "plans.md", text(v).as_bytes());
        now += MINUTE;
        with_notes(&desk, &folder, &key, now, &dn);
        with_notes(&laptop, &folder, &key, now + 10_000, &ln);
    }
    assert_eq!(std::fs::read_to_string(ln.join("plans.md")).unwrap(), text(5));
    let root = crate::history::root(&laptop.memory);
    let versions = crate::history::versions(&root, "notes", "files/notes/plans.md");
    assert_eq!(versions.len(), 4, "{versions:?}");
    assert!(versions.iter().all(|v| v.sealed.is_some()), "kept by reference: {versions:?}");
    let names: Vec<String> = (1..=5).map(|v| content_name(&base, &key, text(v).as_bytes())).collect();
    for name in &names {
        sealed_long_ago(&folder, name);
    }
    // Twelve days on and a little: the references go first.
    let later = now + 12 * DAY_MS + 2 * 60 * MINUTE;
    with_notes(&laptop, &folder, &key, later, &ln);
    assert!(crate::history::versions(&root, "notes", "files/notes/plans.md").is_empty(), "references past twelve days");
    with_notes(&desk, &folder, &key, later + MINUTE, &dn);
    assert!(names.iter().all(|n| sealed(&folder, n)), "contents within two weeks of their replacement");
    // Two weeks on: the replaced contents go, the current one stays.
    with_notes(&desk, &folder, &key, now + 15 * DAY_MS, &dn);
    assert!(names[..4].iter().all(|n| !sealed(&folder, n)), "replaced: swept");
    assert!(sealed(&folder, &names[4]), "current: kept");
    let _ = std::fs::remove_dir_all(&base);
}

/// Three computers: the desk, a laptop, and an old one that wrote a note
/// still current, replaced another, claimed the doses and asked a text to
/// be sent, then stopped.
struct Old {
    base: PathBuf,
    folder: PathBuf,
    key: [u8; 32],
    desk: Computer,
    laptop: Computer,
    old: Computer,
    dn: PathBuf,
    ln: PathBuf,
    on: PathBuf,
    /// Its last word on a note no other device changed; its first version of another, replaced by its second.
    last_word: String,
    replaced: String,
    now: i64,
}

fn old_one(name: &str) -> Old {
    let base = scratch(name);
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop, old) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"), Computer::new(&base, "old"));
    let (dn, ln, on) = (base.join("desk-notes"), base.join("laptop-notes"), base.join("old-notes"));
    for c in [&desk, &laptop, &old] {
        says(c, &folder, &key, crate::devices::COMPUTER, crate::blobs::KEEPING);
    }
    put(&dn, "desk.md", b"# The desk's note\n");
    with_notes(&desk, &folder, &key, NOW, &dn);
    put(&on, "kept.md", b"# The old one's last word\n");
    put(&on, "plans.md", b"# Plans, first\n");
    with_notes(&old, &folder, &key, NOW + MINUTE, &on);
    put(&on, "plans.md", b"# Plans, second\n");
    with_notes(&old, &folder, &key, NOW + 2 * MINUTE, &on);
    crate::lease::renew(&folder, &key, "health", &old.id, (NOW + 2 * MINUTE) / 1000, (NOW + 2 * MINUTE) / 1000, false, crate::lease::Rule::FollowsYou, None).unwrap();
    put(&folder, &format!("texts/send/{}.jsonl", old.id), b"a text to send, sealed\n");
    with_notes(&desk, &folder, &key, NOW + 3 * MINUTE, &dn);
    with_notes(&laptop, &folder, &key, NOW + 4 * MINUTE, &ln);
    with_notes(&old, &folder, &key, NOW + 5 * MINUTE, &on);
    let (last_word, replaced) = (content_name(&base, &key, b"# The old one's last word\n"), content_name(&base, &key, b"# Plans, first\n"));
    for name in [&last_word, &replaced, &content_name(&base, &key, b"# Plans, second\n")] {
        sealed_long_ago(&folder, name);
    }
    Old { base, folder, key, desk, laptop, old, dn, ln, on, last_word, replaced, now: NOW + 6 * MINUTE }
}

fn device_paths(o: &Old) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let id = &o.old.id;
    (o.folder.join("devices").join(format!("{id}.device")), seen_path(&o.folder, id), o.folder.join("leases").join("health").join(format!("{id}.lease")), o.folder.join("texts").join("send").join(format!("{id}.jsonl")))
}

/// "Remove its files" for a computer that left: every round of its goes,
/// its last full round too, its claims, its texts to send, its note of how
/// far it read (no device counts it), and the content of its last word that
/// no other record names; its entry stays (it says it left: without it,
/// your other devices would know it by its claims, the doses first); its
/// replaced content goes in time as any departed device's. Each device keeps
/// what it read; a device joining later gets none of its records. Refused
/// while it counts.
#[test]
fn remove_its_files_for_a_device_that_left() {
    let o = old_one("gc-remove-left");
    let stores = stores_with(&o.desk, &o.dn, false);
    assert_eq!(remove_device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now), Err("share-device-counts".to_string()), "it counts: refused");
    assert!(rounds(&o.folder).get(&o.old.id).is_some_and(|r| !r.is_empty()));
    // It stops sharing: its entry says it left.
    crate::devices::change(&crate::devices::own_path(&o.old.roots.state), Some((&o.folder, &o.key)), |e| {
        e.close(o.now / 1000);
        e.left = true;
    })
    .unwrap();
    with_notes(&o.desk, &o.folder, &o.key, o.now, &o.dn);
    let gone = departed_devices(&o.folder, &o.key, &o.desk.memory, &o.desk.id, o.now + 1_000);
    assert_eq!(gone.iter().map(|g| (g.id.as_str(), g.why)).collect::<Vec<_>>(), vec![(o.old.id.as_str(), "left")]);
    assert!(gone[0].bytes > 0);
    let said = device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + 1_000).unwrap();
    let rounds_there = rounds(&o.folder).get(&o.old.id).cloned().unwrap_or_default();
    assert!(!said.counts && said.rounds == rounds_there.len() && said.round_bytes > 0, "{said:?}");
    assert!(said.sealed_known && said.sealed == 2 && said.sealed_later == 0 && said.entry_stays && !said.notes_stay && said.unread_by.is_empty(), "its two last words: {said:?}");
    let removed = remove_device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + 1_000).unwrap();
    assert_eq!(removed, said, "as said before the press");
    let (entry, notes, claim, texts) = device_paths(&o);
    assert!(rounds(&o.folder).get(&o.old.id).is_none_or(|r| r.is_empty()), "every round, its last full one too");
    assert!(entry.exists(), "its entry says it left: it stays");
    assert!(!notes.exists() && !claim.exists() && !texts.exists());
    assert!(!sealed(&o.folder, &o.last_word) && !sealed(&o.folder, &content_name(&o.base, &o.key, b"# Plans, second\n")), "its last words' contents, named by no other record");
    assert!(sealed(&o.folder, &o.replaced), "its replaced content goes in time, as any departed device's");
    assert!(sealed(&o.folder, &content_name(&o.base, &o.key, b"# The desk's note\n")), "another device's: never");
    assert_eq!(removed_devices(&o.desk.memory, &o.desk.id).get(&o.old.id).map(|r| (r.last, r.entry, r.notes)), Some((*rounds_there.iter().max().unwrap(), false, true)));
    // Each device keeps what it read; the desk and the laptop go on as before.
    for (c, notes) in [(&o.desk, &o.dn), (&o.laptop, &o.ln)] {
        let outcome = with_notes(c, &o.folder, &o.key, o.now + 2 * MINUTE, notes);
        assert!(!outcome.problems.iter().any(|p| p.starts_with("share-other")), "{outcome:?}");
        assert_eq!(std::fs::read(notes.join("kept.md")).unwrap(), b"# The old one's last word\n");
        assert_eq!(std::fs::read(notes.join("plans.md")).unwrap(), b"# Plans, second\n");
    }
    assert!(departed_devices(&o.folder, &o.key, &o.desk.memory, &o.desk.id, o.now + 3 * MINUTE).is_empty(), "nothing of it left to remove");
    // The replaced content goes in its time.
    with_notes(&o.desk, &o.folder, &o.key, o.now + 15 * DAY_MS, &o.dn);
    assert!(!sealed(&o.folder, &o.replaced));
    // A device joining later gets none of its records.
    let late = Computer::new(&o.base, "late");
    let late_notes = o.base.join("late-notes");
    says(&late, &o.folder, &o.key, crate::devices::COMPUTER, crate::blobs::KEEPING);
    with_notes(&late, &o.folder, &o.key, o.now + 15 * DAY_MS + MINUTE, &late_notes);
    assert_eq!(names_in(&late_notes), BTreeSet::from(["desk.md".to_string()]));
    let _ = std::fs::remove_dir_all(&o.base);
}

/// Forgotten here only, a device may still count on your other devices:
/// its entry (how it closed, for their doses) and its note of how far it
/// read (else it would hold every round back there) stay. Another device
/// replacing it, every device stops counting it by that sealed word: both
/// go too. A content sealed lately goes later, swept in its time.
#[test]
fn a_forgotten_device_s_entry_and_note_stay_a_replaced_one_s_go() {
    for replaced in [false, true] {
        let o = old_one(&format!("gc-remove-{replaced}"));
        let stores = stores_with(&o.desk, &o.dn, false);
        if replaced {
            // The laptop says it replaces it (a new phone in its place, in life).
            let r = crate::devices::Replaced { id: o.old.id.clone(), started: Memory::load(&o.desk.memory, &o.desk.id).seen.get(&o.old.id).map_or(0, |e| e.started), heard: heard_of(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &o.old.id) };
            crate::devices::change(&crate::devices::own_path(&o.laptop.roots.state), Some((&o.folder, &o.key)), |e| e.replaces.push(r)).unwrap();
        } else {
            forget_device(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &o.old.id).unwrap();
        }
        // Its last word sealed again lately (another device's use): later.
        std::fs::File::options().write(true).open(o.folder.join("blobs").join(&o.last_word)).unwrap().set_modified(std::time::SystemTime::now()).unwrap();
        with_notes(&o.desk, &o.folder, &o.key, o.now, &o.dn);
        let claim_before = std::fs::read(o.folder.join("leases").join("health").join(format!("{}.lease", o.old.id))).unwrap();
        let said = device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + 1_000).unwrap();
        assert!(!said.counts && said.entry_stays != replaced && said.notes_stay != replaced, "{replaced}: {said:?}");
        assert!(said.sealed == 1 && said.sealed_later == 1, "{said:?}");
        remove_device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + 1_000).unwrap();
        let (entry, notes, claim, texts) = device_paths(&o);
        assert_eq!((entry.exists(), notes.exists()), (!replaced, !replaced), "{replaced}");
        assert!(!claim.exists() && !texts.exists());
        assert!(sealed(&o.folder, &o.last_word), "sealed lately: later");
        assert_eq!(Memory::load_all(&o.desk.memory, &o.desk.id).sealed.theirs.get(&o.last_word).map(|(w, _)| w.as_str()), Some(o.old.id.as_str()), "swept in its time");
        // The laptop, which may still count it, goes on: nothing held back by its note gone.
        with_notes(&o.laptop, &o.folder, &o.key, o.now + MINUTE, &o.ln);
        // A sync app bringing its claim back meanwhile: taken out again; one it wrote since stays.
        std::fs::write(&claim, &claim_before).unwrap();
        with_notes(&o.desk, &o.folder, &o.key, o.now + 2 * MINUTE, &o.dn);
        assert!(!claim.exists(), "{replaced}: an old copy brought back goes again");
        put(&o.folder, &format!("leases/health/{}.lease", o.old.id), b"a claim written since, of another size\n");
        with_notes(&o.desk, &o.folder, &o.key, o.now + 3 * MINUTE, &o.dn);
        assert!(claim.exists(), "{replaced}: one it wrote since stays (review of 17 October 2026, L7)");
        // One written again from nothing has the size its first had, never its bytes: it stays too (L-5).
        let mut renewed = claim_before.clone();
        renewed[0] ^= 1;
        std::fs::write(&claim, &renewed).unwrap();
        with_notes(&o.desk, &o.folder, &o.key, o.now + 4 * MINUTE, &o.dn);
        assert!(claim.exists(), "{replaced}: one of the same size written since stays (review of 18 October 2026, L-5)");
        std::fs::remove_file(&claim).unwrap();
        // Two weeks on, as old as the sweep wants (once it is dated so): its content goes.
        sealed_long_ago(&o.folder, &o.last_word);
        with_notes(&o.desk, &o.folder, &o.key, o.now + 15 * DAY_MS, &o.dn);
        assert!(!sealed(&o.folder, &o.last_word), "{replaced}: swept in its time");
        assert_eq!(std::fs::read(o.dn.join("kept.md")).unwrap(), b"# The old one's last word\n", "the file itself stays everywhere");
        let _ = std::fs::remove_dir_all(&o.base);
    }
}

/// A device still in use whose files you removed (forgotten first): its next
/// exchange finds its own records gone and opens a full round, restating its
/// last words; the desk reads it as any device's, and it counts again.
#[test]
fn a_device_still_in_use_shares_again_after_its_files_were_removed() {
    let o = old_one("gc-remove-live");
    let stores = stores_with(&o.desk, &o.dn, false);
    forget_device(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &o.old.id).unwrap();
    remove_device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now).unwrap();
    assert!(rounds(&o.folder).get(&o.old.id).is_none_or(|r| r.is_empty()));
    assert!(!sealed(&o.folder, &o.last_word));
    // Still in use: it writes again (its sealed files looked after, as every ten minutes).
    put(&o.on, "new.md", b"# Written after\n");
    says(&o.old, &o.folder, &o.key, crate::devices::COMPUTER, crate::blobs::KEEPING);
    let outcome = with_notes(&o.old, &o.folder, &o.key, o.now + 11 * MINUTE, &o.on);
    assert!(outcome.problems.iter().any(|p| p == "share-own-cut"), "its records gone: a full round: {outcome:?}");
    assert!(rounds(&o.folder).get(&o.old.id).is_some_and(|r| !r.is_empty()));
    with_notes(&o.desk, &o.folder, &o.key, o.now + 12 * MINUTE, &o.dn);
    assert_eq!(std::fs::read(o.dn.join("new.md")).unwrap(), b"# Written after\n", "read as any device's");
    assert!(removed_devices(&o.desk.memory, &o.desk.id).is_empty(), "it counts again: no longer removed");
    assert!(sealed(&o.folder, &o.last_word), "its full round restates its last words, sealed again");
    let _ = std::fs::remove_dir_all(&o.base);
}

/// A text's medium stays whoever's note holds the same bytes: "Remove its
/// files" takes out an old computer's last word's content only when no
/// text names it, as the texts' logs held here say.
#[test]
fn a_text_s_medium_stays_when_a_device_s_files_are_removed() {
    use sioul_core::texts;
    let o = old_one("gc-remove-medium");
    let phone = Computer::new(&o.base, "phone");
    let pn = o.base.join("phone-notes");
    says(&phone, &o.folder, &o.key, crate::devices::PHONE, crate::blobs::KEEPING);
    // The phone's text with a picture whose bytes are the old computer's last word.
    let seal = crate::textseal::TextSeal::new(&o.key);
    let own = texts::own_file(&phone.roots.data.join(texts::FOLDER), texts::LOG, &phone.id);
    let path = o.base.join("picture");
    std::fs::write(&path, b"# The old one's last word\n").unwrap();
    let (hash, size) = crate::blobs::hash_file(&path).unwrap();
    let part = texts::Part { seq: 0, ct: "image/jpeg".into(), hash, size, state: "here".into(), ..texts::Part::default() };
    let text = texts::Text { id: "mms-1@1790000000000".into(), at: NOW, direction: "in".into(), with: vec!["+33199001234".into()], parts: vec![part], ..texts::Text::default() };
    let id = |t: &texts::Text| t.line_id();
    texts::append(&own, std::slice::from_ref(&text), &seal, Some(&id)).unwrap();
    with_texts(&phone, &o.folder, &o.key, o.now, &pn);
    with_texts(&o.desk, &o.folder, &o.key, o.now + MINUTE, &o.dn);
    forget_device(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &o.old.id).unwrap();
    let stores = stores_with(&o.desk, &o.dn, true);
    let said = device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + 2 * MINUTE).unwrap();
    assert!(said.sealed_known && said.sealed == 1 && said.sealed_later == 0, "its other last word only, never a medium: {said:?}");
    remove_device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + 2 * MINUTE).unwrap();
    assert!(sealed(&o.folder, &o.last_word), "named by the phone's text");
    assert!(!sealed(&o.folder, &content_name(&o.base, &o.key, b"# Plans, second\n")), "its other last word, named by no text: gone");
    let _ = std::fs::remove_dir_all(&o.base);
    // Without the texts here, a phone seen: the media are not known, and no sealed file goes.
    let o = old_one("gc-remove-media-unknown");
    let phone = Computer::new(&o.base, "phone");
    says(&phone, &o.folder, &o.key, crate::devices::PHONE, crate::blobs::KEEPING);
    with_notes(&o.desk, &o.folder, &o.key, o.now, &o.dn);
    forget_device(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &o.old.id).unwrap();
    let stores = stores_with(&o.desk, &o.dn, false);
    let said = remove_device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + MINUTE).unwrap();
    assert!(!said.sealed_known && said.sealed == 0, "{said:?}");
    assert!(sealed(&o.folder, &o.last_word));
    assert!(rounds(&o.folder).get(&o.old.id).is_none_or(|r| r.is_empty()), "its rounds go all the same");
    let _ = std::fs::remove_dir_all(&o.base);
}


/// M1 (review of 17 October 2026): "Take them out now" takes out the files
/// held after going many at once, those alone; a folder that lists empty
/// beside them (a disk not mounted at its place in the notes) stays unknown
/// until its own line says so, and then it alone goes.
#[test]
fn take_them_out_now_takes_out_the_bulk_alone_never_a_folder_listing_empty() {
    let base = scratch("gc-confirm-bulk");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    put(&dn, "today.md", b"# Today\n");
    for n in 0..20 {
        put(&dn, &format!("old/{n}.md"), format!("# old {n}\n").as_bytes());
    }
    for n in 0..4 {
        put(&dn, &format!("disk/{n}.md"), format!("# disk {n}\n").as_bytes());
        put(&dn, &format!("card/{n}.md"), format!("# card {n}\n").as_bytes());
    }
    // Many notes that stay: the two folders' eight are not many at once.
    for n in 0..60 {
        put(&dn, &format!("keep/{n}.md"), format!("# keep {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    std::fs::remove_dir_all(dn.join("old")).unwrap();
    for n in 0..4 {
        std::fs::remove_file(dn.join(format!("disk/{n}.md"))).unwrap();
        std::fs::remove_file(dn.join(format!("card/{n}.md"))).unwrap();
    }
    let outcome = with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn);
    assert!(["disk", "card"].iter().all(|f| outcome.problems.iter().any(|p| *p == format!("share-vanished:files/notes/{f}/:4"))) && outcome.problems.iter().any(|p| p == "share-vanished:files/notes/:20"), "{outcome:?}");
    assert_eq!(confirm_withheld(&desk.memory, &desk.id, "files/notes/"), Ok(20));
    let outcome = with_notes(&desk, &folder, &key, NOW + 3 * MINUTE, &dn);
    assert_eq!(outcome.sent, 20, "the bulk alone: {outcome:?}");
    with_notes(&laptop, &folder, &key, NOW + 4 * MINUTE, &ln);
    let left = names_in(&ln);
    assert!(left.iter().all(|f| !f.starts_with("old/")) && left.iter().filter(|f| f.starts_with("disk/") || f.starts_with("card/")).count() == 8, "{left:?}");
    // Two weeks on, the folders listing empty still never leave.
    with_notes(&desk, &folder, &key, NOW + 15 * DAY_MS, &dn);
    with_notes(&laptop, &folder, &key, NOW + 15 * DAY_MS + MINUTE, &ln);
    assert_eq!(names_in(&ln).iter().filter(|f| f.starts_with("disk/") || f.starts_with("card/")).count(), 8);
    // One's own line pressed: it alone goes, never the folder beside it.
    confirm_gone(&desk.memory, &desk.id, "files/notes/disk/").unwrap();
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 15 * DAY_MS + 2 * MINUTE, &dn).sent, 4);
    with_notes(&laptop, &folder, &key, NOW + 15 * DAY_MS + 3 * MINUTE, &ln);
    let left = names_in(&ln);
    assert!(left.iter().all(|f| !f.starts_with("disk/")) && left.iter().filter(|f| f.starts_with("card/")).count() == 4, "{left:?}");
    // Nor later: the folder beside it stays unknown.
    with_notes(&desk, &folder, &key, NOW + 15 * DAY_MS + 20 * MINUTE, &dn);
    with_notes(&laptop, &folder, &key, NOW + 15 * DAY_MS + 21 * MINUTE, &ln);
    assert_eq!(names_in(&ln).iter().filter(|f| f.starts_with("card/")).count(), 4);
    let _ = std::fs::remove_dir_all(&base);
}

/// M2 (review of 17 October 2026): two notes holding the same bytes; one
/// edited (the laptop keeps its earlier version by reference), the other
/// taken out: that content is replaced lately, and never leaves the folder
/// early; the earlier version puts back.
#[test]
fn a_content_replaced_lately_never_leaves_early_and_its_version_puts_back() {
    let base = scratch("gc-early-dup");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    for c in [&desk, &laptop] {
        says(c, &folder, &key, crate::devices::COMPUTER, crate::blobs::KEEPING);
    }
    let same = b"# A template, the same in two notes\n";
    put(&dn, "x.md", same);
    put(&dn, "y.md", same);
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    let c = content_name(&base, &key, same);
    sealed_long_ago(&folder, &c);
    put(&dn, "y.md", b"# y, written over the template\n");
    with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn);
    with_notes(&laptop, &folder, &key, NOW + 3 * MINUTE, &ln);
    let root = crate::history::root(&laptop.memory);
    let versions = crate::history::versions(&root, "notes", "files/notes/y.md");
    assert!(versions.len() == 1 && versions[0].sealed.is_some(), "kept by reference: {versions:?}");
    std::fs::remove_file(dn.join("x.md")).unwrap();
    with_notes(&desk, &folder, &key, NOW + 4 * MINUTE, &dn);
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 15 * MINUTE, &dn).sent, 1);
    let round = Memory::load(&desk.memory, &desk.id).round;
    let now = past_round(&desk, &folder, &key, &dn, NOW + 15 * MINUTE, round);
    with_notes(&laptop, &folder, &key, now + 10_000, &ln);
    assert!(read_of(&folder, &laptop, &desk) > round);
    with_notes(&desk, &folder, &key, now + MINUTE, &dn);
    assert!(sealed(&folder, &c), "replaced lately: not early");
    let stamp = versions[0].stamp.clone();
    match crate::history::version(&root, "notes", "files/notes/y.md", &stamp) {
        Some(crate::history::Kept::Sealed { hash, .. }) => {
            let target = base.join("put-back");
            crate::blobs::get(&folder, &key, &hash, &target).unwrap();
            assert_eq!(std::fs::read(&target).unwrap(), same, "its earlier version opens");
        }
        other => panic!("a reference: {other:?}"),
    }
    // In its time, as any replaced content.
    with_notes(&desk, &folder, &key, NOW + 15 * DAY_MS, &dn);
    assert!(!sealed(&folder, &c));
    let _ = std::fs::remove_dir_all(&base);
}

/// M3 (review of 17 October 2026): notes an old computer wrote, deleted
/// here in bulk and held; that computer leaves; "Remove its files" leaves
/// their contents: "Put them back" brings them all back.
#[test]
fn remove_its_files_leaves_what_put_them_back_needs() {
    let o = old_one("gc-remove-held");
    let stores = stores_with(&o.desk, &o.dn, false);
    for n in 0..12 {
        put(&o.on, &format!("trip/{n}.md"), format!("# day {n}, written on the old one\n").as_bytes());
    }
    with_notes(&o.old, &o.folder, &o.key, o.now, &o.on);
    with_notes(&o.desk, &o.folder, &o.key, o.now + MINUTE, &o.dn);
    for n in 0..12 {
        sealed_long_ago(&o.folder, &content_name(&o.base, &o.key, format!("# day {n}, written on the old one\n").as_bytes()));
    }
    std::fs::remove_dir_all(o.dn.join("trip")).unwrap();
    assert_eq!(with_notes(&o.desk, &o.folder, &o.key, o.now + 2 * MINUTE, &o.dn).withheld.map(|w| w[0].count), Some(12));
    crate::devices::change(&crate::devices::own_path(&o.old.roots.state), Some((&o.folder, &o.key)), |e| {
        e.close((o.now + 3 * MINUTE) / 1000);
        e.left = true;
    })
    .unwrap();
    with_notes(&o.desk, &o.folder, &o.key, o.now + 3 * MINUTE, &o.dn);
    let removed = remove_device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + 4 * MINUTE).unwrap();
    assert_eq!(removed.sealed, 2, "its two older last words, never the held notes': {removed:?}");
    assert_eq!(put_back_withheld(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, "files/notes/"), Ok(12));
    assert_eq!(names_in(&o.dn).iter().filter(|f| f.starts_with("trip/")).count(), 12);
    let _ = std::fs::remove_dir_all(&o.base);
}

/// L1 and L2 (review of 17 October 2026): "Put them back" never writes over
/// a file there, a disk too full stops it, said; a bulk held by a Sioul that
/// kept no date of its files (up to 17 October 2026) comes back dated as
/// its last record dates it.
#[test]
fn put_back_keeps_its_care_and_dates_an_older_bulk_by_its_records() {
    let base = scratch("gc-put-back-care");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    let dn = base.join("desk-notes");
    put(&dn, "today.md", b"# Today\n");
    let when = std::time::UNIX_EPOCH + std::time::Duration::from_millis((NOW - 3 * DAY_MS) as u64);
    for n in 0..12 {
        put(&dn, &format!("trip/{n}.md"), format!("# day {n}\n").as_bytes());
        std::fs::File::options().write(true).open(dn.join(format!("trip/{n}.md"))).unwrap().set_modified(when).unwrap();
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    std::fs::remove_dir_all(dn.join("trip")).unwrap();
    with_notes(&desk, &folder, &key, NOW + MINUTE, &dn);
    // As a Sioul up to 17 October 2026 left it: no date kept of the files held.
    let sealed_path = desk.memory.with_file_name("files.json");
    let mut files: serde_json::Value = serde_json::from_slice(&std::fs::read(&sealed_path).unwrap()).unwrap();
    if let Some(stats) = files["files"].as_object_mut() {
        stats.retain(|name, _| !name.starts_with("files/notes/trip/"));
    }
    std::fs::write(&sealed_path, serde_json::to_vec(&files).unwrap()).unwrap();
    let stores = stores_with(&desk, &dn, false);
    // No room: nothing written, said.
    crate::disk::ROOM.with(|room| room.set(Some(0)));
    assert!(put_back_withheld(&folder, &key, &desk.memory, &desk.id, &stores, "files/notes/").is_err_and(|e| e.starts_with("share-no-room:")));
    crate::disk::ROOM.with(|room| room.set(Some(u64::MAX)));
    assert!(!dn.join("trip").join("0.md").exists());
    // A place taken: never written over.
    put(&dn, "trip/3.md", b"# written meanwhile\n");
    assert_eq!(put_back_withheld(&folder, &key, &desk.memory, &desk.id, &stores, "files/notes/"), Ok(11));
    assert_eq!(std::fs::read(dn.join("trip/3.md")).unwrap(), b"# written meanwhile\n");
    // Dated as its last record was made (its first exchange, here), not now.
    let dated = std::fs::metadata(dn.join("trip/5.md")).unwrap().modified().unwrap();
    let recorded = std::time::UNIX_EPOCH + std::time::Duration::from_millis(NOW as u64);
    let off = dated.duration_since(recorded).unwrap_or_else(|e| e.duration());
    assert!(off < std::time::Duration::from_secs(60), "dated as its last record was made: {dated:?} for {recorded:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// L3 (review of 17 October 2026): files held as gone many at once stay so
/// until their own two weeks are over, whatever leaves beside them.
#[test]
fn a_bulk_s_remainder_leaves_in_its_own_time() {
    let base = scratch("gc-remainder");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln) = (base.join("desk-notes"), base.join("laptop-notes"));
    put(&dn, "today.md", b"# Today\n");
    put(&dn, "old/stays.md", b"# stays\n");
    for n in 0..14 {
        put(&dn, &format!("old/{n}.md"), format!("# old {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    for n in 0..12 {
        std::fs::remove_file(dn.join(format!("old/{n}.md"))).unwrap();
    }
    with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn);
    for n in 12..14 {
        std::fs::remove_file(dn.join(format!("old/{n}.md"))).unwrap();
    }
    let five = NOW + 5 * DAY_MS;
    with_notes(&desk, &folder, &key, five, &dn);
    // Day 14: the first twelve leave; the two of day 5 stay until their own day 14.
    with_notes(&desk, &folder, &key, NOW + 14 * DAY_MS + 3 * MINUTE, &dn);
    with_notes(&desk, &folder, &key, NOW + 14 * DAY_MS + 4 * MINUTE, &dn);
    with_notes(&laptop, &folder, &key, NOW + 14 * DAY_MS + 5 * MINUTE, &ln);
    let left: Vec<String> = names_in(&ln).into_iter().filter(|f| f.starts_with("old/") && f != "old/stays.md").collect();
    assert_eq!(left, vec!["old/12.md".to_string(), "old/13.md".to_string()], "the remainder held");
    with_notes(&desk, &folder, &key, five + 14 * DAY_MS - MINUTE, &dn);
    with_notes(&laptop, &folder, &key, five + 14 * DAY_MS - 30_000, &ln);
    assert_eq!(names_in(&ln).len(), 4, "until its own two weeks are over");
    with_notes(&desk, &folder, &key, five + 14 * DAY_MS, &dn);
    with_notes(&laptop, &folder, &key, five + 14 * DAY_MS + MINUTE, &ln);
    assert_eq!(names_in(&ln), BTreeSet::from(["today.md".to_string(), "old/stays.md".to_string()]), "and then, in its own time");
    let _ = std::fs::remove_dir_all(&base);
}

/// L4 (review of 17 October 2026): a folder moved out of the notes and
/// linked back in its place is unknown, said, never taken out.
#[cfg(unix)]
#[test]
fn a_folder_replaced_by_a_link_never_leaves() {
    let base = scratch("gc-linked");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln, elsewhere) = (base.join("desk-notes"), base.join("laptop-notes"), base.join("elsewhere"));
    put(&dn, "today.md", b"# Today\n");
    for n in 0..20 {
        put(&dn, &format!("archive/{n}.md"), format!("# {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    std::fs::rename(dn.join("archive"), &elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dn.join("archive")).unwrap();
    let outcome = with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn);
    assert!(outcome.problems.iter().any(|p| p == "share-vanished:files/notes/archive/:20") && outcome.sent == 0, "{outcome:?}");
    assert_eq!(outcome.withheld, Some(Vec::new()), "unknown, not gone");
    for at in [NOW + 15 * DAY_MS, NOW + 30 * DAY_MS] {
        assert_eq!(with_notes(&desk, &folder, &key, at, &dn).sent, 0);
    }
    with_notes(&laptop, &folder, &key, NOW + 30 * DAY_MS + MINUTE, &ln);
    assert_eq!(names_in(&ln).len(), 21);
    let _ = std::fs::remove_dir_all(&base);
}

/// L5 (review of 17 October 2026): where a bulk was, said shortly: the
/// deepest folder that holds four in five; else the few that hold most;
/// else the folder above them all.
#[test]
fn a_bulk_s_folders_are_said_shortly() {
    let many: Vec<String> = (0..300).flat_map(|run| (0..20).map(move |n| format!("agents/{run:04}/{n}.md"))).collect();
    assert_eq!(bulk_folders(many.iter().map(String::as_str)), vec!["agents".to_string()]);
    let mostly: Vec<String> = (0..20).map(|n| format!("archive/2026/{n}.md")).chain((0..3).map(|n| format!("old/{n}.md"))).collect();
    assert_eq!(bulk_folders(mostly.iter().map(String::as_str)), vec!["archive/2026".to_string()]);
    let two: Vec<String> = (0..10).map(|n| format!("a/{n}.md")).chain((0..10).map(|n| format!("b/{n}.md"))).collect();
    assert_eq!(bulk_folders(two.iter().map(String::as_str)), vec!["a".to_string(), "b".to_string()]);
    let top: Vec<String> = (0..10).map(|n| format!("{n}.md")).collect();
    assert_eq!(bulk_folders(top.iter().map(String::as_str)), vec![String::new()]);
    let spread: Vec<String> = (0..10).map(|n| format!("d{n}/x.md")).collect();
    assert_eq!(bulk_folders(spread.iter().map(String::as_str)), vec![String::new()], "spread: the folder above them all");
}

/// L6 (review of 17 October 2026): a device forgotten here, its files
/// removed (its entry and note kept: it may still count elsewhere), then
/// replaced by another device's word: every device stops counting it, and
/// its entry and note go at the next exchange.
#[test]
fn forgotten_then_replaced_its_entry_and_note_go() {
    let o = old_one("gc-forgotten-replaced");
    let stores = stores_with(&o.desk, &o.dn, false);
    forget_device(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &o.old.id).unwrap();
    with_notes(&o.desk, &o.folder, &o.key, o.now, &o.dn);
    remove_device_files(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &stores, &o.old.id, o.now + 1_000).unwrap();
    let (entry, notes, _, _) = device_paths(&o);
    assert!(entry.exists() && notes.exists(), "forgotten here only: kept");
    let r = crate::devices::Replaced { id: o.old.id.clone(), started: Memory::load(&o.desk.memory, &o.desk.id).seen.get(&o.old.id).map_or(0, |e| e.started), heard: heard_of(&o.folder, &o.key, &o.desk.memory, &o.desk.id, &o.old.id) };
    crate::devices::change(&crate::devices::own_path(&o.laptop.roots.state), Some((&o.folder, &o.key)), |e| e.replaces.push(r)).unwrap();
    with_notes(&o.desk, &o.folder, &o.key, o.now + MINUTE, &o.dn);
    assert!(!entry.exists() && !notes.exists(), "replaced now: they go");
    let _ = std::fs::remove_dir_all(&o.base);
}

/// L9 (review of 17 October 2026): take-outs kept as copies (a file with
/// other names is copied, never moved: its other names keep their mode) are
/// bounded per exchange; the rest wait for the next.
#[cfg(unix)]
#[test]
fn copies_of_take_outs_are_bounded_per_exchange() {
    use std::os::unix::fs::PermissionsExt;
    let base = scratch("gc-copies");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln, links) = (base.join("desk-notes"), base.join("laptop-notes"), base.join("links"));
    for n in 0..3 {
        put(&dn, &format!("n{n}.md"), format!("# note {n}\n").as_bytes());
    }
    // Three of twenty: not many at once (they go ten minutes on).
    let stays: BTreeSet<String> = (0..17).map(|n| format!("stays-{n}.md")).collect();
    for name in &stays {
        put(&dn, name, b"# stays\n");
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    std::fs::create_dir_all(&links).unwrap();
    for n in 0..3 {
        std::fs::hard_link(ln.join(format!("n{n}.md")), links.join(format!("n{n}.md"))).unwrap();
        std::fs::set_permissions(links.join(format!("n{n}.md")), std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::remove_file(dn.join(format!("n{n}.md"))).unwrap();
    }
    with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn);
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 13 * MINUTE, &dn).sent, 3);
    // A copy at most, this exchange.
    TAKE_OUT_BUDGET.with(|budget| budget.set(1));
    let outcome = with_notes(&laptop, &folder, &key, NOW + 14 * MINUTE, &ln);
    assert_eq!(outcome.waits.get("copies"), Some(&2), "{outcome:?}");
    assert_eq!(names_in(&ln).len(), 19, "one out, two waiting");
    for _ in 0..2 {
        with_notes(&laptop, &folder, &key, NOW + 15 * MINUTE, &ln);
    }
    TAKE_OUT_BUDGET.with(|budget| budget.set(TAKE_OUT_COPIES));
    assert_eq!(names_in(&ln), stays);
    for n in 0..3 {
        let mode = std::fs::metadata(links.join(format!("n{n}.md"))).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644, "its other name keeps its mode");
        let versions = crate::history::versions(&crate::history::root(&laptop.memory), "notes", &format!("files/notes/n{n}.md"));
        assert!(versions.len() == 1 && versions[0].sealed.is_none(), "kept as a copy");
    }
    let _ = std::fs::remove_dir_all(&base);
}


/// L1 (review of 17 October 2026): a file put back takes its place only
/// while none is there, one written there meanwhile included; the file
/// beside it goes in any case.
#[test]
fn a_file_put_back_never_takes_a_place_taken() {
    let base = scratch("gc-place-new");
    let (beside, place) = (base.join(".x.md.tmp"), base.join("x.md"));
    std::fs::write(&beside, b"# put back\n").unwrap();
    std::fs::write(&place, b"# written meanwhile\n").unwrap();
    assert!(!place_new(&beside, &place));
    assert_eq!(std::fs::read(&place).unwrap(), b"# written meanwhile\n");
    assert!(!beside.exists());
    std::fs::write(&beside, b"# put back\n").unwrap();
    std::fs::remove_file(&place).unwrap();
    assert!(place_new(&beside, &place));
    assert_eq!(std::fs::read(&place).unwrap(), b"# put back\n");
    assert!(!beside.exists());
    let _ = std::fs::remove_dir_all(&base);
}


/// L-3 (review of 18 October 2026): "Put them back" of a large bulk never
/// keeps a dose's exchange waiting a second: the exchanges' lock is held a
/// quarter of a second at most, then left free a moment.
#[test]
fn put_back_never_keeps_a_dose_waiting() {
    let base = scratch("gc-put-back-lock");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    let dn = base.join("desk-notes");
    put(&dn, "today.md", b"# Today\n");
    for n in 0..1_500 {
        put(&dn, &format!("bulk/{n:04}.md"), format!("# note {n} of a bulk, with a few words in it\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    std::fs::remove_dir_all(dn.join("bulk")).unwrap();
    assert_eq!(with_notes(&desk, &folder, &key, NOW + MINUTE, &dn).withheld.map(|w| w[0].count), Some(1_500));
    let (stores, memory, id) = (stores_with(&desk, &dn, false), desk.memory.clone(), desk.id.clone());
    let putting = {
        let (folder, memory, id) = (folder.clone(), memory.clone(), id.clone());
        std::thread::spawn(move || put_back_withheld(&folder, &key, &memory, &id, &stores, "files/notes/"))
    };
    // A dose's exchange taking the lock as it does, again and again, while the bulk comes back.
    let (mut longest, mut taken) = (std::time::Duration::ZERO, 0);
    while !putting.is_finished() {
        let asked = std::time::Instant::now();
        let held = exchange_lock(&memory, true).unwrap();
        longest = longest.max(asked.elapsed());
        taken += 1;
        drop(held);
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    assert_eq!(putting.join().unwrap(), Ok(1_500));
    println!("put back of 1,500 notes: a dose's exchange waited {longest:?} at most, {taken} times");
    assert!(taken > 5 && longest < std::time::Duration::from_secs(1), "a dose's exchange waited {longest:?} at most, {taken} times");
    let _ = std::fs::remove_dir_all(&base);
}

/// L-a (review of 18 October 2026): a large note put back is opened beside
/// its place outside the exchanges' lock, taken only to place it: a dose's
/// exchange never waits for its time (until then, a 40 MB note held it
/// 0.62 s on this computer, seconds on a phone).
#[test]
fn a_large_note_put_back_never_holds_the_lock_for_its_time() {
    let base = scratch("gc-put-back-large");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    let dn = base.join("desk-notes");
    put(&dn, "today.md", b"# Today\n");
    // 24 MB of words that do not compress, and eleven small notes beside it.
    let mut seed: u64 = 0x5eed;
    let big: String = (0..24 * 1024 * 1024)
        .map(|_| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            char::from(b'a' + ((seed >> 33) % 26) as u8)
        })
        .collect();
    put(&dn, "bulk/big.md", big.as_bytes());
    for n in 0..11 {
        put(&dn, &format!("bulk/{n:02}.md"), format!("# note {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    std::fs::remove_dir_all(dn.join("bulk")).unwrap();
    assert_eq!(with_notes(&desk, &folder, &key, NOW + MINUTE, &dn).withheld.map(|w| w[0].count), Some(12));
    let (stores, memory, id) = (stores_with(&desk, &dn, false), desk.memory.clone(), desk.id.clone());
    let started = std::time::Instant::now();
    let putting = {
        let (folder, memory, id) = (folder.clone(), memory.clone(), id.clone());
        std::thread::spawn(move || put_back_withheld(&folder, &key, &memory, &id, &stores, "files/notes/"))
    };
    let mut longest = std::time::Duration::ZERO;
    while !putting.is_finished() {
        let asked = std::time::Instant::now();
        let held = exchange_lock(&memory, true).unwrap();
        longest = longest.max(asked.elapsed());
        drop(held);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(putting.join().unwrap(), Ok(12));
    let took = started.elapsed();
    assert_eq!(std::fs::read(dn.join("bulk/big.md")).unwrap(), big.as_bytes(), "the large note back whole");
    println!("put back with a 24 MB note: {took:?}, a dose's exchange waited {longest:?} at most");
    assert!(longest < std::time::Duration::from_millis(250), "a dose's exchange waited {longest:?} at most, the put back taking {took:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// L-9 (review of 18 October 2026): a note replaced by a link is said as
/// that note, never left on the others; "Take them out everywhere" on its
/// line takes it out alone, never a note whose name begins as its.
#[cfg(unix)]
#[test]
fn a_note_replaced_by_a_link_is_said_as_that_note() {
    let base = scratch("gc-note-linked");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let (dn, ln, elsewhere) = (base.join("desk-notes"), base.join("laptop-notes"), base.join("elsewhere.md"));
    for name in ["today.md", "single.md", "single.md.bak"] {
        put(&dn, name, format!("# {name}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, NOW, &dn);
    with_notes(&laptop, &folder, &key, NOW + MINUTE, &ln);
    // Both replaced by links: the one whose name begins as the other's stays unknown when the other is confirmed.
    for (name, away) in [("single.md", elsewhere.clone()), ("single.md.bak", base.join("elsewhere.bak"))] {
        std::fs::rename(dn.join(name), &away).unwrap();
        std::os::unix::fs::symlink(&away, dn.join(name)).unwrap();
    }
    let outcome = with_notes(&desk, &folder, &key, NOW + 2 * MINUTE, &dn);
    assert!(outcome.problems.iter().any(|p| p == "share-vanished:files/notes/single.md:1") && outcome.problems.iter().any(|p| p == "share-vanished:files/notes/single.md.bak:1") && outcome.sent == 0, "{outcome:?}");
    with_notes(&desk, &folder, &key, NOW + 30 * DAY_MS, &dn);
    with_notes(&laptop, &folder, &key, NOW + 30 * DAY_MS + MINUTE, &ln);
    assert_eq!(names_in(&ln).len(), 3, "never left");
    confirm_gone(&desk.memory, &desk.id, "files/notes/single.md").unwrap();
    assert_eq!(with_notes(&desk, &folder, &key, NOW + 30 * DAY_MS + 2 * MINUTE, &dn).sent, 1);
    with_notes(&laptop, &folder, &key, NOW + 30 * DAY_MS + 3 * MINUTE, &ln);
    assert_eq!(names_in(&ln), BTreeSet::from(["today.md".to_string(), "single.md.bak".to_string()]));
    let _ = std::fs::remove_dir_all(&base);
}
