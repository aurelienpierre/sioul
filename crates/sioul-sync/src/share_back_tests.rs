// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A device that ends before its memory is saved never sends back what it
//! received, nor its own change again with a later clock (docs/database.md,
//! "What an exchange wrote, noted first"); and the sync apps' copies of a
//! device's own files leave the folder once their content is said elsewhere,
//! and files being written are named as no sync app sends them ("What the
//! sync app must do"). Two stand-in devices exchanging
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

/// The case of 21 November 2026: a note written on the desk, that version
/// written on the phone by an exchange the phone did not live to save (the
/// app ended); the desk's next version a quarter of an hour later; the
/// phone started again a minute after that, something of its own changed at
/// its start (a setting, dated then) found before the note in its look.
/// Until the journal, the phone took the received version for a change of
/// its own, sent it with a clock past its setting's, later than the desk's
/// next version: the desk kept its newer version aside as "plans (conflict
/// …).md" and put the older one back.
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
    // The desk's next version.
    put(&desk_notes, "plans.md", v1);
    dated(&desk_notes.join("plans.md"), at(2));
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    // The phone writes it, then ends before its memory reaches the disk.
    ends(&phone, &folder, &key, at(3), &stores_with(&phone, &phone_notes));
    assert_eq!(std::fs::read(phone_notes.join("plans.md")).unwrap(), v1);
    // The desk's version after it.
    put(&desk_notes, "plans.md", v2);
    dated(&desk_notes.join("plans.md"), at(5));
    with_notes(&desk, &folder, &key, at(5), &desk_notes);
    // The phone starts again: a setting of its own changed then.
    phone.write("config/safe-senders.txt", "a@example.org\n");
    dated(&phone.path("config/safe-senders.txt"), at(6));
    let restart = with_notes(&phone, &folder, &key, at(6), &phone_notes);
    // The desk reads, then the phone.
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

/// The sync apps' copies of a device's own files (the Nextcloud desktop
/// client's "(conflicted copy …)", Syncthing's ".sync-conflict-…", Dropbox's
/// in French) leave the folder once nothing in them is unsaid elsewhere: its
/// entry's, its claim's and its notes' at once; a copy of one of its rounds
/// that holds nothing past the round's file at once, one that holds more once
/// a full round restated all (the cut that made it starts one at once). A
/// copy of another device's round stays, as a copy holding more of a round
/// before the full round came; nothing else is touched.
#[test]
fn copies_of_a_device_s_own_files_leave_once_known() {
    let base = scratch("back-copies");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    let senders = |n: usize| (0..n).map(|i| format!("sender{i}@example.org\n")).collect::<String>();
    desk.write("config/known-senders.txt", &senders(40));
    desk.exchange(&folder, &key, at(0));
    let round = folder.join(format!("{}-1.jsonl", desk.id));
    let bytes = std::fs::read(&round).unwrap();
    let half = bytes[..bytes.len() / 2].iter().rposition(|b| *b == b'\n').unwrap() + 1;
    let other = uuid::Uuid::new_v4().to_string();
    let id = desk.id.clone();
    let named = |relative: &str| folder.join(relative);
    let mut longer = bytes.clone();
    longer.extend_from_slice(b"{\"n\":999,\"c\":1,\"s\":\"AAAA\"}\n");
    let going = [
        (format!("{id}-1 (conflicted copy 2026-10-23 175457).jsonl"), bytes[..half].to_vec()),
        (format!("devices/{id} (conflicted copy 2026-10-23 175457).device"), b"entry".to_vec()),
        (format!("leases/health/{id} (conflicted copy 2026-10-23 175457).lease"), b"claim".to_vec()),
        (format!("{id}.sync-conflict-20261023-175457-ABCDEFG.toml"), b"notes".to_vec()),
    ];
    let staying = [(format!("{other}-1 (conflicted copy 2026-10-23 175457).jsonl"), bytes.clone()), (format!("{id}-1 (copie en conflit de bureau 2026-10-23).jsonl"), longer.clone())];
    for (relative, content) in going.iter().chain(staying.iter()) {
        put(&folder, relative, content);
    }
    // A file of the desk's that is no copy, and the desk's own files, stay.
    put(&folder, &format!("devices/{id}.device"), b"entry");
    desk.exchange(&folder, &key, at(11));
    for (relative, _) in &going {
        assert!(!named(relative).exists(), "{relative} goes");
    }
    for (relative, _) in &staying {
        assert!(named(relative).exists(), "{relative} stays");
    }
    assert!(round.exists() && named(&format!("devices/{id}.device")).exists());
    // The client puts back an older copy of the round: a cut, a full round at once; then the longer copy goes too.
    std::fs::write(&round, &bytes[..half]).unwrap();
    let outcome = desk.exchange(&folder, &key, at(22));
    assert!(outcome.problems.iter().any(|p| p == "share-own-cut"), "{:?}", outcome.problems);
    assert!(!named(&staying[1].0).exists(), "restated by the full round: it goes");
    assert!(named(&staying[0].0).exists(), "another device's copy stays");
    let _ = std::fs::remove_dir_all(&base);
}

/// A file being written is named beside its place, so that its rename stays
/// within one file system, and begins with ".~", which the sync apps' own
/// lists of what never travels leave out whatever they do with hidden files
/// (the Nextcloud and ownCloud desktop clients' `*.~*`, Dropbox's `.~*`); a
/// crash's leftover of it, or of the earlier form, is cleaned all the same.
#[test]
fn a_file_being_written_is_named_as_no_sync_app_sends_it() {
    let path = Path::new("/somewhere/Sioul/devices/0f8fad5b-d9cb-469f-a165-70867728950e.device");
    let (first, second) = (temporary(path), temporary(path));
    assert_ne!(first, second, "never two writers in one");
    for written in [&first, &second] {
        assert_eq!(written.parent(), path.parent(), "beside its place: one file system");
        let name = written.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with(".~0f8fad5b-d9cb-469f-a165-70867728950e.device.") && name.ends_with(TEMPORARY), "{name}");
        assert!(is_leftover(&name) && made_by_tools(&name), "{name}");
    }
    assert!(is_leftover(".lease.md.4211-7.sioul.tmp"), "the earlier form is still cleaned");
}

/// A change made here that lost to another device's later one in the same
/// exchange, the device then ended: the file holds the later change, so the
/// journal never says it holds this device's; the person choosing the same
/// value again later makes a new change, at a new clock, which goes and
/// stays (the review of 24 November 2026, must-fix 2).
#[test]
fn a_value_chosen_again_after_it_lost_goes_at_a_new_clock() {
    let base = scratch("back-again");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/config.toml", "theme = \"dark\"\n");
    desk.exchange(&folder, &key, at(0));
    phone.exchange(&folder, &key, at(1));
    phone.write("config/config.toml", "theme = \"light\"\n");
    dated(&phone.path("config/config.toml"), at(2));
    desk.write("config/config.toml", "theme = \"system\"\n");
    desk.exchange(&folder, &key, at(3));
    // The phone's exchange sends "light" and writes the desk's later "system" over it; it ends.
    ends(&phone, &folder, &key, at(4), &plain(&phone));
    assert!(phone.read("config/config.toml").contains("system"));
    // The person chooses "light" again.
    phone.write("config/config.toml", "theme = \"light\"\n");
    dated(&phone.path("config/config.toml"), at(5));
    phone.exchange(&folder, &key, at(6));
    desk.exchange(&folder, &key, at(7));
    phone.exchange(&folder, &key, at(8));
    for c in [&desk, &phone] {
        assert!(c.read("config/config.toml").contains("light"), "{}", c.read("config/config.toml"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// A setting taken out on the desk, the take-out written on the phone, the
/// phone ended; the desk sets it again. The phone started again never sends
/// that take-out back as its own (it would take the setting out everywhere).
#[test]
fn a_setting_taken_out_elsewhere_then_ended_never_takes_out_its_return() {
    let base = scratch("back-taken");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/config.toml", "theme = \"dark\"\nlanguage = \"fr\"\n");
    desk.exchange(&folder, &key, at(0));
    phone.exchange(&folder, &key, at(1));
    desk.write("config/config.toml", "language = \"fr\"\n");
    desk.exchange(&folder, &key, at(2));
    ends(&phone, &folder, &key, at(3), &plain(&phone));
    assert!(!phone.read("config/config.toml").contains("theme"));
    desk.write("config/config.toml", "theme = \"light\"\nlanguage = \"fr\"\n");
    desk.exchange(&folder, &key, at(5));
    assert_eq!(phone.exchange(&folder, &key, at(6)).sent, 0);
    desk.exchange(&folder, &key, at(7));
    for c in [&desk, &phone] {
        assert!(c.read("config/config.toml").contains("light"), "{}", c.read("config/config.toml"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// This device's own take-out (a sender let in again), appended, the device
/// ended; the desk blocks the sender again later. Started again, the phone
/// sends its take-out at its own clock, which loses to the desk's later word.
#[test]
fn an_own_take_out_appended_then_ended_goes_again_at_its_own_clock() {
    let base = scratch("back-own-out");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/blocked-senders.txt", "a@example.org\nnews@example.org\n");
    desk.exchange(&folder, &key, at(0));
    phone.exchange(&folder, &key, at(1));
    phone.write("config/blocked-senders.txt", "a@example.org\n");
    ends(&phone, &folder, &key, at(2), &plain(&phone));
    desk.exchange(&folder, &key, at(3));
    assert!(!desk.read("config/blocked-senders.txt").contains("news@"));
    desk.write("config/blocked-senders.txt", "a@example.org\nnews@example.org\n");
    desk.exchange(&folder, &key, at(4));
    phone.exchange(&folder, &key, at(5));
    desk.exchange(&folder, &key, at(6));
    phone.exchange(&folder, &key, at(7));
    for c in [&desk, &phone] {
        assert!(c.read("config/blocked-senders.txt").contains("news@example.org"), "{}", c.read("config/blocked-senders.txt"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The doses' record and a draft received, the device ended: neither goes
/// back as its own; the desk's later word on both stays.
#[test]
fn a_dose_and_a_draft_received_then_ended_never_go_back() {
    let base = scratch("back-dose-draft");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("state/health-state.toml", "[taken]\n\"levothyroxine@1790000600\" = 1790000600\n");
    desk.write("data/drafts/reply.eml", "Subject: Lease\n\nFirst words.\n");
    desk.exchange(&folder, &key, at(0));
    phone.exchange(&folder, &key, at(1));
    desk.write("state/health-state.toml", "[taken]\n\"levothyroxine@1790000600\" = 1790000600\n\"levothyroxine@1790086400\" = 1790086400\n");
    desk.write("data/drafts/reply.eml", "Subject: Lease\n\nFirst words, and more.\n");
    desk.exchange(&folder, &key, at(2));
    ends(&phone, &folder, &key, at(3), &plain(&phone));
    // The draft sent on the desk (taken out), a dose taken back there.
    std::fs::remove_file(desk.path("data/drafts/reply.eml")).unwrap();
    desk.write("state/health-state.toml", "[taken]\n\"levothyroxine@1790000600\" = 1790000600\n");
    desk.exchange(&folder, &key, at(5));
    assert_eq!(phone.exchange(&folder, &key, at(6)).sent, 0);
    desk.exchange(&folder, &key, at(7));
    for c in [&desk, &phone] {
        assert!(!c.path("data/drafts/reply.eml").exists(), "the draft sent stays sent");
        assert!(!c.read("state/health-state.toml").contains("1790086400"), "{}", c.read("state/health-state.toml"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// A quick exchange (a dose's alarm: no notes read) between the end and the
/// next whole one: it saves its memory, and the note's line stays in the
/// journal while the memory does not hold it; the whole exchange then passes
/// over the received note as well.
#[test]
fn a_quick_exchange_between_the_end_and_the_next_keeps_the_note_s_line() {
    let base = scratch("back-quick");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    put(&desk_notes, "lease.md", b"Rent: 600\n");
    with_notes(&desk, &folder, &key, at(0), &desk_notes);
    with_notes(&phone, &folder, &key, at(1), &phone_notes);
    put(&desk_notes, "lease.md", b"Rent: 620\n");
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    ends(&phone, &folder, &key, at(3), &stores_with(&phone, &phone_notes));
    put(&desk_notes, "lease.md", b"Rent: 640\n");
    with_notes(&desk, &folder, &key, at(4), &desk_notes);
    let quick = exchange(&Sharing { folder: &folder, computer: &phone.id, key: &key, memory: &phone.memory, files: false, hurry: None }, &stores_with(&phone, &phone_notes), at(5)).unwrap();
    assert_eq!(quick.sent, 0);
    assert_eq!(with_notes(&phone, &folder, &key, at(6), &phone_notes).sent, 0, "the received note never goes back");
    with_notes(&desk, &folder, &key, at(7), &desk_notes);
    for notes in [&desk_notes, &phone_notes] {
        assert_eq!(std::fs::read(notes.join("lease.md")).unwrap(), b"Rent: 640\n", "{}", notes.display());
        assert!(copies(notes, "lease").is_empty(), "{}", notes.display());
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The journal's own edges: after two ended exchanges, only the last done
/// line of a key counts, so a note put back to the first one's version is a
/// change of the person's, sent (the review of 24 November 2026, 3); a line
/// left half written by the end of a process is passed over and cut back.
#[test]
fn only_the_last_line_marked_done_counts_and_a_half_line_is_passed_over() {
    let base = scratch("back-marks");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    let (v0, v1, v2): (&[u8], &[u8], &[u8]) = (b"Rent: 600\n", b"Rent: 620\n", b"Rent: 640\n");
    put(&desk_notes, "lease.md", v0);
    with_notes(&desk, &folder, &key, at(0), &desk_notes);
    with_notes(&phone, &folder, &key, at(1), &phone_notes);
    // Two versions received, each by an exchange that ended.
    put(&desk_notes, "lease.md", v1);
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    ends(&phone, &folder, &key, at(3), &stores_with(&phone, &phone_notes));
    put(&desk_notes, "lease.md", v2);
    with_notes(&desk, &folder, &key, at(4), &desk_notes);
    ends(&phone, &folder, &key, at(5), &stores_with(&phone, &phone_notes));
    assert_eq!(std::fs::read(phone_notes.join("lease.md")).unwrap(), v2);
    // Half a line, as a process ended while it wrote.
    let journal = journal::journal_path(&phone.memory);
    let mut text = std::fs::read(&journal).unwrap();
    text.extend(b"{\"v\":2,\"k\":\"files/notes/le");
    std::fs::write(&journal, &text).unwrap();
    // The person puts the first received version back by hand.
    put(&phone_notes, "lease.md", v1);
    let restart = with_notes(&phone, &folder, &key, at(6), &phone_notes);
    assert_eq!(restart.sent, 1, "the person's change goes: {restart:?}");
    let lines = std::fs::read(&journal).unwrap_or_default();
    assert!(lines.split(|b| *b == b'\n').filter(|l| !l.is_empty()).all(|l| serde_json::from_slice::<serde_json::Value>(l).is_ok()), "no half line kept");
    with_notes(&desk, &folder, &key, at(7), &desk_notes);
    assert_eq!(std::fs::read(desk_notes.join("lease.md")).unwrap(), v1, "the person's word is the later one");
    let _ = std::fs::remove_dir_all(&base);
}

/// The journal's lines as an exchange ended between a write and its mark
/// leaves them (the process ended in the microseconds after a file's rename,
/// or a power cut lost the marks, which are not waited for on the disk): a
/// change noted after the key's last done one counts all the same where the
/// file holds exactly what it wrote, so the note received is never sent
/// back (the third review of 26 November 2026, A2).
#[test]
fn a_write_whose_mark_was_lost_still_counts_where_the_file_holds_it() {
    let base = scratch("back-unmarked");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    put(&desk_notes, "lease.md", b"Rent: 600\n");
    with_notes(&desk, &folder, &key, at(0), &desk_notes);
    with_notes(&phone, &folder, &key, at(1), &phone_notes);
    put(&desk_notes, "lease.md", b"Rent: 620\n");
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    ends(&phone, &folder, &key, at(3), &stores_with(&phone, &phone_notes));
    // Every mark lost: the lines noted remain.
    let journal = journal::journal_path(&phone.memory);
    let kept: Vec<u8> = std::fs::read(&journal).unwrap().split(|b| *b == b'\n').filter(|l| !l.is_empty() && !serde_json::from_slice::<Landed>(l).is_ok_and(|l| l.d)).flat_map(|l| l.iter().copied().chain([b'\n'])).collect();
    assert!(!kept.is_empty());
    std::fs::write(&journal, &kept).unwrap();
    put(&desk_notes, "lease.md", b"Rent: 640\n");
    with_notes(&desk, &folder, &key, at(4), &desk_notes);
    assert_eq!(with_notes(&phone, &folder, &key, at(5), &phone_notes).sent, 0, "the received note never goes back");
    with_notes(&desk, &folder, &key, at(6), &desk_notes);
    for notes in [&desk_notes, &phone_notes] {
        assert_eq!(std::fs::read(notes.join("lease.md")).unwrap(), b"Rent: 640\n", "{}", notes.display());
        assert!(copies(notes, "lease").is_empty(), "{}", notes.display());
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The update from a build whose journal had no marks (format 1, the
/// install of 25 November 2026): its lines count where the file holds
/// exactly what they wrote, which they meant there. A note received by such
/// a build, the app then ended by the install before its memory was saved,
/// is never sent back as this device's own change by the new build (the
/// third review of 26 November 2026, A1: read as never written, it went
/// back, the incident of 21 November at the moment of the install).
#[test]
fn a_line_of_the_journal_s_first_format_counts_where_the_file_holds_it() {
    let base = scratch("back-format-1");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    put(&desk_notes, "lease.md", b"Rent: 600\n");
    with_notes(&desk, &folder, &key, at(0), &desk_notes);
    with_notes(&phone, &folder, &key, at(1), &phone_notes);
    put(&desk_notes, "lease.md", b"Rent: 620\n");
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    ends(&phone, &folder, &key, at(3), &stores_with(&phone, &phone_notes));
    // The journal as the build before the marks wrote it: format 1, no "d".
    let journal = journal::journal_path(&phone.memory);
    let older: Vec<u8> = std::fs::read(&journal)
        .unwrap()
        .split(|b| *b == b'\n')
        .filter_map(|l| serde_json::from_slice::<Landed>(l).ok())
        .filter(|l| !l.d)
        .flat_map(|l| {
            let mut line = serde_json::to_vec(&Landed { v: 1, ..l }).unwrap();
            line.push(b'\n');
            line
        })
        .collect();
    assert!(!older.is_empty());
    std::fs::write(&journal, &older).unwrap();
    put(&desk_notes, "lease.md", b"Rent: 640\n");
    with_notes(&desk, &folder, &key, at(4), &desk_notes);
    assert_eq!(with_notes(&phone, &folder, &key, at(5), &phone_notes).sent, 0, "nothing sent back");
    with_notes(&desk, &folder, &key, at(6), &desk_notes);
    for notes in [&desk_notes, &phone_notes] {
        assert_eq!(std::fs::read(notes.join("lease.md")).unwrap(), b"Rent: 640\n", "{}", notes.display());
        assert!(copies(notes, "lease").is_empty(), "{}", notes.display());
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// Two lines of the journal's first format for one note (the installed
/// build counted every line of a key by its content), from two exchanges
/// that ended in a row, the second between noting the newer version and
/// writing it, so that the file still holds the earlier: at the update,
/// nothing is sent back, and both devices end on the newer version with no
/// conflicted copy (the fourth review of 27 November 2026, F2: each line read
/// as done, the second hid the first, and the earlier version went back over
/// the newer one, the incident of 21 November at the update).
#[test]
fn two_first_format_lines_the_file_holding_the_earlier_send_nothing_back() {
    let base = scratch("back-format-1-two");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    put(&desk_notes, "lease.md", b"Rent: 600\n");
    with_notes(&desk, &folder, &key, at(0), &desk_notes);
    with_notes(&phone, &folder, &key, at(1), &phone_notes);
    put(&desk_notes, "lease.md", b"Rent: 620\n");
    with_notes(&desk, &folder, &key, at(2), &desk_notes);
    ends(&phone, &folder, &key, at(3), &stores_with(&phone, &phone_notes));
    put(&desk_notes, "lease.md", b"Rent: 640\n");
    with_notes(&desk, &folder, &key, at(4), &desk_notes);
    ends(&phone, &folder, &key, at(5), &stores_with(&phone, &phone_notes));
    assert_eq!(std::fs::read(phone_notes.join("lease.md")).unwrap(), b"Rent: 640\n");
    // The second end came between noting 640 and writing it: the file still holds 620.
    put(&phone_notes, "lease.md", b"Rent: 620\n");
    let journal = journal::journal_path(&phone.memory);
    let older: Vec<u8> = std::fs::read(&journal)
        .unwrap()
        .split(|b| *b == b'\n')
        .filter_map(|l| serde_json::from_slice::<Landed>(l).ok())
        .filter(|l| !l.d)
        .flat_map(|l| {
            let mut line = serde_json::to_vec(&Landed { v: 1, ..l }).unwrap();
            line.push(b'\n');
            line
        })
        .collect();
    std::fs::write(&journal, &older).unwrap();
    let out = with_notes(&phone, &folder, &key, at(6), &phone_notes);
    with_notes(&desk, &folder, &key, at(7), &desk_notes);
    with_notes(&phone, &folder, &key, at(8), &phone_notes);
    assert_eq!(out.sent, 0, "nothing sent back");
    for notes in [&desk_notes, &phone_notes] {
        assert_eq!(std::fs::read(notes.join("lease.md")).unwrap(), b"Rent: 640\n");
        assert!(copies(notes, "lease").is_empty());
    }
    let _ = std::fs::remove_dir_all(&base);
}

