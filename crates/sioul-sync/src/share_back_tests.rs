// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A device that ends before its memory is saved never sends back what it
//! received, nor its own change again with a later clock (docs/database.md,
//! "What an exchange wrote, noted first"). Two stand-in devices exchanging
//! through one folder; "ends" stops an exchange just before its memory is
//! saved (`ENDS_BEFORE_SAVE`), everything else written, as the system ending
//! the app (or a crash, or a save that failed) leaves a device. On 0.1.0,
//! which has no such hook, the same tests put the memory's two files back as
//! they were before the exchange, which leaves the same state there (no
//! journal), and failed as the fault does (24 November 2026). Fiction only.

use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;

fn put(dir: &Path, relative: &str, bytes: &[u8]) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A file's time set to `at` (milliseconds), as an editor dates a save.
fn dated(path: &Path, at: i64) {
    std::fs::File::options().write(true).open(path).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_millis(at as u64)).unwrap();
}

fn at(minutes: i64) -> i64 {
    NOW + minutes * MINUTE
}

/// A computer's stores with `notes` as its notes folder, notes shared besides the parts shared by default.
fn stores_with(c: &Computer, notes: &Path) -> Vec<Store> {
    let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
    stores_of(&config, &c.roots, &|part| part == "notes" || shared_by_default(part, &config))
}

fn with_notes(c: &Computer, folder: &Path, key: &[u8; 32], now: i64, notes: &Path) -> Outcome {
    exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: true, hurry: None }, &stores_with(c, notes), now).unwrap()
}

/// One exchange that ends just before its memory is saved
/// (`ENDS_BEFORE_SAVE`): the others' changes written into the files here, its
/// own records appended, its notes to the others written, its memory as it
/// was before, as the system ending the app leaves a device.
fn ends(c: &Computer, folder: &Path, key: &[u8; 32], now: i64, stores: &[Store]) {
    ENDS_BEFORE_SAVE.with(|e| e.set(true));
    let ended = exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: true, hurry: None }, stores, now);
    ENDS_BEFORE_SAVE.with(|e| e.set(false));
    assert_eq!(ended.err().as_deref(), Some("ended before its memory was saved"));
}

/// A computer's stores, the parts shared by default.
fn plain(c: &Computer) -> Vec<Store> {
    stores(&Config::default(), &c.roots)
}

/// The names in a notes folder beginning with `stem`, but the note itself.
fn copies(notes: &Path, stem: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(notes).unwrap().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with(stem) && *n != format!("{stem}.md")).collect();
    out.sort();
    out
}

/// The case of 21 November 2026: a note written on the desk at 17:40, its
/// version written on the phone by an exchange the phone did not live to
/// save (the app ended); the desk's next version at 17:55; the phone started
/// again at 17:56, something of its own changed at its start (a setting,
/// dated then) found before the note in its look. Until the journal, the
/// phone took the 17:40 version for a change of its own, sent it with a
/// clock past its setting's, later than the desk's 17:55 version: the desk
/// kept 17:55 aside as "plans (conflict … 17.55).md" and put 17:40 back.
#[test]
fn a_note_received_then_ended_never_goes_back_over_a_newer_one() {
    let base = scratch("back-note");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    let (v0, v1, v2): (&[u8], &[u8], &[u8]) = (b"# Plans\n- one\n", b"# Plans\n- one\n- two\n", b"# Plans\n- one\n- two\n- three\n");
    put(&desk_notes, "plans.md", v0);
    dated(&desk_notes.join("plans.md"), at(0));
    with_notes(&desk, &folder, &key, at(0), &desk_notes);
    with_notes(&phone, &folder, &key, at(1), &phone_notes);
    assert_eq!(std::fs::read(phone_notes.join("plans.md")).unwrap(), v0);
    // The desk's next version ("17:40").
    put(&desk_notes, "plans.md", v1);
    dated(&desk_notes.join("plans.md"), at(2));
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    // The phone writes it, then ends before its memory reaches the disk.
    ends(&phone, &folder, &key, at(3), &stores_with(&phone, &phone_notes));
    assert_eq!(std::fs::read(phone_notes.join("plans.md")).unwrap(), v1);
    // The desk's next version ("17:55").
    put(&desk_notes, "plans.md", v2);
    dated(&desk_notes.join("plans.md"), at(5));
    with_notes(&desk, &folder, &key, at(5), &desk_notes);
    // The phone starts again ("17:56"): a setting of its own changed then.
    phone.write("config/safe-senders.txt", "a@example.org\n");
    dated(&phone.path("config/safe-senders.txt"), at(6));
    let restart = with_notes(&phone, &folder, &key, at(6), &phone_notes);
    // The desk reads ("17:57"), then the phone.
    let read = with_notes(&desk, &folder, &key, at(7), &desk_notes);
    with_notes(&phone, &folder, &key, at(8), &phone_notes);
    for notes in [&desk_notes, &phone_notes] {
        let shown = |name: &str| String::from_utf8_lossy(&std::fs::read(notes.join(name)).unwrap_or_default()).to_string();
        let kept: Vec<(String, String)> = copies(notes, "plans").into_iter().map(|n| (shown(&n), n)).collect();
        assert_eq!(shown("plans.md").as_bytes(), v2, "{}: the newer version stays; kept beside: {kept:?}; the desk said {:?}", notes.display(), read.problems);
        assert!(kept.is_empty(), "{}: no conflict where there was none: {kept:?}", notes.display());
    }
    assert!(!read.problems.iter().any(|p| p.starts_with("share-conflict")), "{:?}", read.problems);
    assert_eq!(restart.sent, 1, "the setting alone goes, never the note it received: {restart:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// The same for a setting: received, the device ended, changed again
/// elsewhere; the device started again sends nothing of it, and the later
/// setting stays everywhere.
#[test]
fn a_setting_received_then_ended_never_goes_back_over_a_newer_one() {
    let base = scratch("back-setting");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/config.toml", "theme = \"dark\"\n");
    desk.exchange(&folder, &key, at(0));
    phone.exchange(&folder, &key, at(1));
    desk.write("config/config.toml", "theme = \"light\"\n");
    desk.exchange(&folder, &key, at(2));
    ends(&phone, &folder, &key, at(3), &plain(&phone));
    assert!(phone.read("config/config.toml").contains("light"));
    desk.write("config/config.toml", "theme = \"system\"\n");
    desk.exchange(&folder, &key, at(5));
    let restart = phone.exchange(&folder, &key, at(6));
    assert_eq!(restart.sent, 0, "{restart:?}");
    desk.exchange(&folder, &key, at(7));
    phone.exchange(&folder, &key, at(8));
    for c in [&desk, &phone] {
        assert!(c.read("config/config.toml").contains("system"), "{}", c.read("config/config.toml"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// A sender blocked on the desk, received on the phone, the phone ended; the
/// desk lets the sender in again. The phone started again never blocks it
/// anew everywhere.
#[test]
fn a_sender_let_in_again_never_comes_back_blocked_from_a_device_that_ended() {
    let base = scratch("back-sender");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/blocked-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, at(0));
    phone.exchange(&folder, &key, at(1));
    desk.write("config/blocked-senders.txt", "a@example.org\nnews@example.org\n");
    desk.exchange(&folder, &key, at(2));
    ends(&phone, &folder, &key, at(3), &plain(&phone));
    assert!(phone.read("config/blocked-senders.txt").contains("news@example.org"));
    desk.write("config/blocked-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, at(5));
    let restart = phone.exchange(&folder, &key, at(6));
    assert_eq!(restart.sent, 0, "{restart:?}");
    desk.exchange(&folder, &key, at(7));
    phone.exchange(&folder, &key, at(8));
    for c in [&desk, &phone] {
        assert!(!c.read("config/blocked-senders.txt").contains("news@example.org"), "{}", c.read("config/blocked-senders.txt"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The phone's own change, appended to its records, the phone ended before
/// its memory was saved; the desk read it and changed the setting again
/// later. The phone started again (`share-own-ahead`) finds its change
/// again: it goes again at its own clock, and the desk's later one stays.
#[test]
fn a_change_appended_then_ended_goes_again_at_its_own_clock() {
    let base = scratch("back-own");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/config.toml", "theme = \"dark\"\n");
    desk.exchange(&folder, &key, at(0));
    phone.exchange(&folder, &key, at(1));
    phone.write("config/config.toml", "theme = \"light\"\n");
    ends(&phone, &folder, &key, at(2), &plain(&phone));
    desk.exchange(&folder, &key, at(3));
    assert!(desk.read("config/config.toml").contains("light"));
    desk.write("config/config.toml", "theme = \"system\"\n");
    desk.exchange(&folder, &key, at(4));
    let restart = phone.exchange(&folder, &key, at(5));
    assert!(restart.problems.iter().any(|p| p == "share-own-ahead"), "{:?}", restart.problems);
    desk.exchange(&folder, &key, at(6));
    phone.exchange(&folder, &key, at(7));
    for c in [&desk, &phone] {
        assert!(c.read("config/config.toml").contains("system"), "{}", c.read("config/config.toml"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// Many notes taken out on the desk at once, the take-outs written on the
/// phone, the phone ended: started again, it finds them gone while its memory
/// still holds them. Never held as gone many at once (the Porch would ask
/// about notes the phone did not take out, and the take-outs would wait
/// behind that question): they are the desk's, written already.
#[test]
fn notes_taken_out_elsewhere_then_ended_are_never_held_as_gone_here() {
    let base = scratch("back-bulk");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    for n in 0..20 {
        put(&desk_notes, &format!("day-{n:02}.md"), format!("# Day {n}\n").as_bytes());
    }
    with_notes(&desk, &folder, &key, at(0), &desk_notes);
    with_notes(&phone, &folder, &key, at(1), &phone_notes);
    for n in 0..12 {
        std::fs::remove_file(desk_notes.join(format!("day-{n:02}.md"))).unwrap();
    }
    // Gone ten minutes, and you said to take them out: they go.
    let folder_said = "files/notes/".to_string();
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    confirm_gone(&desk.memory, &desk.id, &folder_said).unwrap();
    let sent = with_notes(&desk, &folder, &key, at(15), &desk_notes);
    assert_eq!(sent.sent, 12, "{sent:?}");
    ends(&phone, &folder, &key, at(16), &stores_with(&phone, &phone_notes));
    assert_eq!(std::fs::read_dir(&phone_notes).unwrap().count(), 8);
    let restart = with_notes(&phone, &folder, &key, at(17), &phone_notes);
    assert!(!restart.problems.iter().any(|p| p.starts_with("share-vanished")), "{:?}", restart.problems);
    assert!(restart.withheld.as_ref().is_some_and(Vec::is_empty), "{:?}", restart.withheld);
    assert_eq!(restart.sent, 0, "{restart:?}");
    assert_eq!(std::fs::read_dir(&phone_notes).unwrap().count(), 8);
    let _ = std::fs::remove_dir_all(&base);
}

/// What the person changed after a received version was written, the device
/// ended in between, still goes: the journal only ever passes over a file
/// that holds what another device's change wrote. Nothing is lost: the
/// desk's version is kept beside.
#[test]
fn a_change_made_over_a_received_note_after_the_end_still_goes() {
    let base = scratch("back-edit");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    let (v0, v1, mine): (&[u8], &[u8], &[u8]) = (b"Rent: 600\n", b"Rent: 620\n", b"Rent: 620\nKeys: two\n");
    put(&desk_notes, "lease.md", v0);
    with_notes(&desk, &folder, &key, at(0), &desk_notes);
    with_notes(&phone, &folder, &key, at(1), &phone_notes);
    put(&desk_notes, "lease.md", v1);
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    ends(&phone, &folder, &key, at(3), &stores_with(&phone, &phone_notes));
    put(&phone_notes, "lease.md", mine);
    assert_eq!(with_notes(&phone, &folder, &key, at(4), &phone_notes).sent, 1);
    with_notes(&desk, &folder, &key, at(5), &desk_notes);
    with_notes(&phone, &folder, &key, at(6), &phone_notes);
    for notes in [&desk_notes, &phone_notes] {
        assert_eq!(std::fs::read(notes.join("lease.md")).unwrap(), mine, "{}", notes.display());
    }
    let kept: Vec<Vec<u8>> = copies(&desk_notes, "lease").iter().map(|n| std::fs::read(desk_notes.join(n)).unwrap()).collect();
    assert!(kept.iter().all(|k| k == v1), "only the desk's version kept beside: {kept:?}");
    let _ = std::fs::remove_dir_all(&base);
}
