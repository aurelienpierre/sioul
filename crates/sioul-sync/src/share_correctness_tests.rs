// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sharing's correctness, finding by finding, after the reviews of 5
//! October 2026: each test fails on the code before its fix and passes after
//! (docs/database.md, docs/health.md "Knowing"). Fiction only.

use super::tests::{Computer, MINUTE, NOW, copy_dir, quick_key, scratch};
use super::*;

/// Copies a file of the folder, whole or its first `bytes`, as a sync app
/// brings it, late or cut on its way.
fn carry(from: &Path, to: &Path, name: &str, bytes: Option<usize>) {
    let content = std::fs::read(from.join(name)).unwrap();
    let content = bytes.map_or(content.clone(), |n| content[..n.min(content.len())].to_vec());
    std::fs::create_dir_all(to.join(name).parent().unwrap()).unwrap();
    std::fs::write(to.join(name), content).unwrap();
}

/// Where the last whole line of a file ends, before `at`.
fn line_end_before(path: &Path, at: usize) -> usize {
    let bytes = std::fs::read(path).unwrap();
    bytes[..at.min(bytes.len())].iter().rposition(|b| *b == b'\n').map_or(0, |n| n + 1)
}

/// N1 (a regression of 8 October 2026, when rounds began to continue one
/// another, `SEGMENT`): a sync app brings a computer's next round before the
/// end of the one before. The reader moved on to the next round and never read
/// that end again, though it held a dose marked taken; the gap at the turn of
/// the rounds went unseen (a next round's first line "follows" any line), so
/// the doses took what that computer wrote as all read: a dose taken, known not taken.
#[test]
fn a_round_brought_before_the_end_of_the_one_before_loses_nothing() {
    let base = scratch("round-turn");
    let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
    let key = quick_key(&server, "four words make a passphrase").unwrap();
    carry(&server, &phone_folder, "seal.toml", None);
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let senders = |from: usize, to: usize| (from..to).map(|n| format!("sender{n}@example.org\n")).collect::<String>();
    // The desk's opening, far past a segment.
    desk.write("config/known-senders.txt", &senders(0, 1500));
    desk.exchange(&server, &key, NOW);
    let first = format!("{}-1.jsonl", desk.id);
    carry(&server, &phone_folder, &first, None);
    phone.exchange(&phone_folder, &key, NOW + 30_000);
    // Then much more, and a dose marked taken: round 2, continuing round 1, its dose last.
    desk.write("config/known-senders.txt", &senders(0, 3000));
    desk.write("state/health-state.toml", "[taken]\n\"levothyroxine@1790000600\" = 1790000600\n");
    desk.exchange(&server, &key, NOW + MINUTE);
    let second = format!("{}-2.jsonl", desk.id);
    assert!(server.join(&second).exists(), "round 2 continues round 1");
    // Then another dose: round 3, as round 2 is past a segment.
    desk.write("state/health-state.toml", "[taken]\n\"levothyroxine@1790000600\" = 1790000600\n\"levothyroxine@1790000700\" = 1790000700\n");
    desk.exchange(&server, &key, NOW + 2 * MINUTE);
    let third = format!("{}-3.jsonl", desk.id);
    assert!(server.join(&third).exists(), "round 3 continues round 2");
    // The phone's sync app brings round 3 whole, and only the first half of round 2.
    let half = line_end_before(&server.join(&second), std::fs::metadata(server.join(&second)).unwrap().len() as usize / 2);
    carry(&server, &phone_folder, &second, Some(half));
    carry(&server, &phone_folder, &third, None);
    phone.exchange(&phone_folder, &key, NOW + 2 * MINUTE + 30_000);
    let wrote = written(&desk.memory, &desk.id).unwrap();
    let record = phone.read("state/health-state.toml");
    assert!(
        record.contains("levothyroxine@1790000600") || !heard(&phone.memory, &phone.id).complete(&desk.id, wrote),
        "the phone takes all the desk wrote as read, and lacks the dose at the end of round 2:\n{record}"
    );
    // The rest of round 2 comes: read, the dose there.
    carry(&server, &phone_folder, &second, None);
    phone.exchange(&phone_folder, &key, NOW + 3 * MINUTE);
    let record = phone.read("state/health-state.toml");
    assert!(record.contains("levothyroxine@1790000600") && record.contains("levothyroxine@1790000700"), "{record}");
    assert!(heard(&phone.memory, &phone.id).complete(&desk.id, wrote));
    let _ = std::fs::remove_dir_all(&base);
}

/// A desk whose rounds 2 and 3 continue its first, a dose marked in each,
/// and a phone that read round 1 and holds only the first half of round 2
/// with round 3 whole: the case of `a_round_brought_before_the_end_of_the_one_before_loses_nothing`.
struct Turned {
    base: PathBuf,
    server: PathBuf,
    phone_folder: PathBuf,
    key: [u8; 32],
    desk: Computer,
    phone: Computer,
}

fn turned(name: &str) -> Turned {
    let base = scratch(name);
    let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
    let key = quick_key(&server, "four words make a passphrase").unwrap();
    carry(&server, &phone_folder, "seal.toml", None);
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let senders = |to: usize| (0..to).map(|n| format!("sender{n}@example.org\n")).collect::<String>();
    desk.write("config/known-senders.txt", &senders(1500));
    desk.exchange(&server, &key, NOW);
    carry(&server, &phone_folder, &format!("{}-1.jsonl", desk.id), None);
    phone.exchange(&phone_folder, &key, NOW + 30_000);
    desk.write("config/known-senders.txt", &senders(3000));
    desk.write("state/health-state.toml", "[taken]\n\"iron@1790000600\" = 1790000600\n");
    desk.exchange(&server, &key, NOW + MINUTE);
    desk.write("state/health-state.toml", "[taken]\n\"iron@1790000600\" = 1790000600\n\"iron@1790000700\" = 1790000700\n");
    desk.exchange(&server, &key, NOW + 2 * MINUTE);
    let second = format!("{}-2.jsonl", desk.id);
    let half = line_end_before(&server.join(&second), std::fs::metadata(server.join(&second)).unwrap().len() as usize / 2);
    carry(&server, &phone_folder, &second, Some(half));
    carry(&server, &phone_folder, &format!("{}-3.jsonl", desk.id), None);
    Turned { base, server, phone_folder, key, desk, phone }
}

/// N1: the end of a round never comes (an older copy of it kept on the phone
/// for good): the phone waits, in doubt, until its computer opens a full
/// round, then reads on from there, and has every dose.
#[test]
fn a_round_whose_end_never_comes_is_read_past_from_a_full_round() {
    let t = turned("round-turn-full");
    t.phone.exchange(&t.phone_folder, &t.key, NOW + 3 * MINUTE);
    assert!(!heard(&t.phone.memory, &t.phone.id).complete(&t.desk.id, written(&t.desk.memory, &t.desk.id).unwrap()), "waiting for the end of round 2: not known");
    // The desk's own round cut on the server (a sync app's older copy): it opens a full round, restating all.
    let third = round_file(&t.server, &t.desk.id, 3);
    let cut = line_end_before(&third, 1);
    std::fs::write(&third, &std::fs::read(&third).unwrap()[..cut]).unwrap();
    let outcome = t.desk.exchange(&t.server, &t.key, NOW + 4 * MINUTE);
    assert!(outcome.problems.iter().any(|p| p == "share-own-cut"), "{outcome:?}");
    carry(&t.server, &t.phone_folder, &format!("{}-4.jsonl", t.desk.id), None);
    let outcome = t.phone.exchange(&t.phone_folder, &t.key, NOW + 5 * MINUTE);
    let record = t.phone.read("state/health-state.toml");
    assert!(record.contains("iron@1790000600") && record.contains("iron@1790000700"), "{record}\n{outcome:?}");
    assert!(heard(&t.phone.memory, &t.phone.id).complete(&t.desk.id, written(&t.desk.memory, &t.desk.id).unwrap()));
    assert!(!outcome.problems.iter().any(|p| p.starts_with("share-other-gap")), "{outcome:?}");
    let _ = std::fs::remove_dir_all(&t.base);
}

/// N1: a device joining reads from the oldest round kept, which may continue
/// one gone: nothing is said lost, and it knows all once it read the full round.
#[test]
fn a_device_joining_from_a_continuing_round_says_nothing_lost() {
    let t = turned("round-turn-join");
    // The phone's notes on the server: the desk keeps the rounds it has not read past.
    carry(&t.phone_folder, &t.server, &format!("{}.toml", t.phone.id), None);
    // Round 3 grows by a line; a sync app cuts it back to its first: the desk opens a full round 4.
    t.desk.write("config/safe-senders.txt", "a@example.org\n");
    t.desk.exchange(&t.server, &t.key, NOW + 3 * MINUTE);
    let third = round_file(&t.server, &t.desk.id, 3);
    let first = std::fs::read(&third).unwrap().iter().position(|b| *b == b'\n').unwrap() + 1;
    std::fs::write(&third, &std::fs::read(&third).unwrap()[..first]).unwrap();
    let outcome = t.desk.exchange(&t.server, &t.key, NOW + 4 * MINUTE);
    assert!(outcome.problems.iter().any(|p| p == "share-own-cut"), "{outcome:?}");
    // Rounds 1 and 2 gone from the server's folder; 3 (continuing 2) and 4 (full) kept.
    for round in [1, 2] {
        std::fs::remove_file(round_file(&t.server, &t.desk.id, round)).unwrap();
    }
    assert!(third.exists() && round_file(&t.server, &t.desk.id, 4).exists());
    let laptop = Computer::new(&t.base, "laptop");
    let outcome = laptop.exchange(&t.server, &t.key, NOW + 5 * MINUTE);
    assert!(!outcome.problems.iter().any(|p| p.starts_with("share-other")), "{outcome:?}");
    let record = laptop.read("state/health-state.toml");
    assert!(record.contains("iron@1790000600") && record.contains("iron@1790000700"), "{record}");
    assert!(heard(&laptop.memory, &laptop.id).complete(&t.desk.id, written(&t.desk.memory, &t.desk.id).unwrap()));
    let _ = std::fs::remove_dir_all(&t.base);
}

/// N1: a memory kept before rounds said where they stand, which had moved
/// past the end of round 2 unread: read again once, the dose there comes in.
#[test]
fn an_older_memory_that_moved_past_a_rounds_end_reads_it_again() {
    let t = turned("round-turn-older");
    // As the code before 10 October 2026 left it: in round 3, all "read", round 2's end never.
    let third = round_file(&t.phone_folder, &t.desk.id, 3);
    let mut memory: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&t.phone.memory).unwrap()).unwrap();
    memory["read"][&t.desk.id] = serde_json::json!([3, std::fs::metadata(&third).unwrap().len()]);
    memory["read_n"][&t.desk.id] = serde_json::json!([3, 1]);
    memory.as_object_mut().unwrap().remove("turns");
    std::fs::write(&t.phone.memory, memory.to_string()).unwrap();
    carry(&t.server, &t.phone_folder, &format!("{}-2.jsonl", t.desk.id), None);
    t.phone.exchange(&t.phone_folder, &t.key, NOW + 3 * MINUTE);
    let record = t.phone.read("state/health-state.toml");
    assert!(record.contains("iron@1790000600"), "{record}");
    let _ = std::fs::remove_dir_all(&t.base);
}

/// SS-F14: a line lost is dated by when its computer wrote it (the line
/// before it), never by when it was found; read again (a record rebuilt), it
/// is not stamped anew, so a years-old damaged line never doubts today's doses.
#[test]
fn a_lost_line_is_dated_by_its_own_time_and_stamped_once() {
    let base = scratch("lost-line");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    let own = round_file(&folder, &desk.id, 1);
    let mut bytes = std::fs::read(&own).unwrap();
    bytes.extend_from_slice(b"\xff\xfe broken\n");
    std::fs::write(&own, &bytes).unwrap();
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
    desk.exchange(&folder, &key, NOW + MINUTE);
    // Found three days later.
    let later = NOW + 3 * 86_400_000;
    let outcome = phone.exchange(&folder, &key, later);
    assert!(outcome.problems.iter().any(|p| p.starts_with("share-other-line")), "{outcome:?}");
    let at = heard(&phone.memory, &phone.id).broken[&desk.id];
    assert!((NOW / 1000..=NOW / 1000 + 120).contains(&at), "dated by the desk's lines around it, not when found: {at}");
    // Read again (the doses' record rebuilt): the same line, not stamped anew.
    rebuild(&phone.memory, &phone.id, &["state/health-state.toml"]).unwrap();
    phone.exchange(&folder, &key, later + 86_400_000);
    assert_eq!(heard(&phone.memory, &phone.id).broken[&desk.id], at);
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F20 and SS-F5: the sharing folder gone (a disk not mounted, a sync
/// app's folder moved) is never made again, empty, for the real one to meet
/// later as conflicted copies: nothing is written, it is said, and all goes
/// on once it is back. A device whose key does not open the folder's seal
/// (it sealed a folder of its own before the other device's seal came)
/// writes nothing there under a key the others lack.
#[test]
fn a_folder_gone_is_never_made_again_and_a_seal_that_does_not_open_takes_nothing() {
    let base = scratch("folder-gone");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    let away = base.join("Sioul-away");
    std::fs::rename(&folder, &away).unwrap();
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
    let outcome = desk.exchange(&folder, &key, NOW + MINUTE);
    assert_eq!(outcome.problems, ["share-no-seal"], "{outcome:?}");
    assert!(!folder.exists(), "never made again, empty");
    std::fs::rename(&away, &folder).unwrap();
    // Its seal gone while the folder stays (a sync app that made it again, or an empty placeholder of the seal): the same.
    let seal = std::fs::read(folder.join("seal.toml")).unwrap();
    std::fs::write(folder.join("seal.toml"), b"").unwrap();
    let outcome = desk.exchange(&folder, &key, NOW + 90_000);
    assert_eq!(outcome.problems, ["share-no-seal"], "{outcome:?}");
    std::fs::write(folder.join("seal.toml"), seal).unwrap();
    let outcome = desk.exchange(&folder, &key, NOW + 2 * MINUTE);
    assert!(outcome.problems.is_empty() && outcome.sent == 1, "{outcome:?}");
    // The phone sealed its copy of the folder first (the desk's seal not brought yet), then its sync
    // app brought the desk's seal in its place: the phone's key no longer opens it.
    let mine = base.join("phone-copy");
    let other = quick_key(&mine, "another passphrase, typed apart").unwrap();
    let phone = Computer::new(&base, "phone");
    phone.exchange(&mine, &other, NOW + 150_000);
    std::fs::copy(folder.join("seal.toml"), mine.join("seal.toml")).unwrap();
    phone.write("config/safe-senders.txt", "c@example.org\n");
    let before = std::fs::read(round_file(&mine, &phone.id, 1)).unwrap_or_default();
    let outcome = phone.exchange(&mine, &other, NOW + 3 * MINUTE);
    assert_eq!(outcome.problems, ["share-sealed-otherwise"], "{outcome:?}");
    assert_eq!(std::fs::read(round_file(&mine, &phone.id, 1)).unwrap_or_default(), before, "nothing written under a key the others lack");
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F25: a list kept by another tool in another encoding is not text: it is
/// left as it is here, said, what comes for it waits, and nothing of it is
/// taken out on the other devices (it was read lossily, then written back
/// holding the incoming line alone).
#[test]
fn a_list_that_is_not_text_is_left_as_it_is_and_takes_nothing_out() {
    let base = scratch("not-text");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    desk.write("config/blocked-senders.txt", "spam@example.org\nother@example.org\n");
    desk.exchange(&folder, &key, NOW);
    laptop.exchange(&folder, &key, NOW + MINUTE);
    // On the laptop, saved by another tool in Latin-1: "é" as one byte.
    let latin = b"spam@example.org\nr\xe9sum\xe9@example.org\nother@example.org\n".to_vec();
    std::fs::write(laptop.path("config/blocked-senders.txt"), &latin).unwrap();
    desk.write("config/blocked-senders.txt", "spam@example.org\nother@example.org\nmore@example.org\n");
    desk.exchange(&folder, &key, NOW + 2 * MINUTE);
    let outcome = laptop.exchange(&folder, &key, NOW + 3 * MINUTE);
    assert_eq!(std::fs::read(laptop.path("config/blocked-senders.txt")).unwrap(), latin, "left as it is");
    assert!(outcome.problems.iter().any(|p| p.starts_with("share-unreadable")), "{outcome:?}");
    desk.exchange(&folder, &key, NOW + 4 * MINUTE);
    let list = desk.read("config/blocked-senders.txt");
    assert!(list.contains("spam@example.org") && list.contains("other@example.org") && list.contains("more@example.org") && !list.contains('\u{fffd}'), "{list}");
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F22: the copy of every shared file kept before sharing starts is never
/// written over: sharing stopped and started again the same day (or a first
/// exchange tried again) keeps the files as they were before sharing.
#[test]
fn the_copy_kept_before_sharing_is_never_written_over() {
    let base = scratch("before-sharing");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "before@example.org\n");
    desk.exchange(&folder, &key, NOW);
    let day = jiff::Timestamp::from_millisecond(NOW).unwrap().strftime("%Y-%m-%d").to_string();
    let kept = desk.memory.parent().unwrap().join(format!("before-sharing-{day}")).join("config/safe-senders.txt");
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), "before@example.org\n");
    // Stop sharing (the memory forgotten), the file changed, sharing again the same day.
    std::fs::remove_file(&desk.memory).unwrap();
    let _ = std::fs::remove_file(desk.memory.with_file_name("files.json"));
    desk.write("config/safe-senders.txt", "after@example.org\n");
    desk.exchange(&folder, &key, NOW + MINUTE);
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), "before@example.org\n", "as it was before sharing");
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F23: on the phone a dose of an antibiotic taken every eight hours is
/// taken at 09:30, not 08:00: its next doses move (the medicine's start), and
/// the move is kept with the mark. Before the phone's change reached it, the
/// desk changed the medicine's dose text: the medicine travels whole, the
/// desk's is the later word, and its start, as it was, wins everywhere: the
/// next dose would come an hour and a half early. Each device's minute mends
/// it from the mark (`Health::mend_shifts`, as the window's tick runs it):
/// both end with the move and the desk's text.
#[test]
fn a_late_dose_s_move_survives_an_edit_of_the_medicine_elsewhere() {
    use sioul_core::health::{Health, HealthState, Medicine, Schedule};
    let base = scratch("medicine-move");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let start = NOW / 1000 + 3600;
    let medicine = Medicine { id: "antibiotic".into(), name: "Antibiotic".into(), dose: "1 tablet".into(), schedule: Schedule::Hours { hours: 8, from: start }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None };
    Health { medicines: vec![medicine], ..Health::default() }.save(&desk.path("data/health.toml")).unwrap();
    desk.exchange(&folder, &key, NOW);
    phone.exchange(&folder, &key, NOW + MINUTE);
    // On the phone: taken an hour and a half late.
    let key_of = format!("antibiotic@{start}");
    let late = start + 90 * 60;
    let moved = Health::change(&phone.path("data/health.toml"), |h| Ok(h.taken_at(&key_of, late))).unwrap().unwrap();
    HealthState::update(&phone.path("state/health-state.toml"), late, |s| {
        s.taken.insert(key_of.clone(), late);
        s.moved.insert(key_of.clone(), [moved.0, moved.1]);
    })
    .unwrap();
    // On the desk, before the phone's news: the dose's text changed.
    Health::change(&desk.path("data/health.toml"), |h| {
        h.medicines[0].dose = "1 tablet, with food".into();
        Ok(())
    })
    .unwrap();
    phone.exchange(&folder, &key, NOW + 2 * MINUTE);
    desk.exchange(&folder, &key, NOW + 3 * MINUTE);
    phone.exchange(&folder, &key, NOW + 4 * MINUTE);
    let from = |c: &Computer| match Health::load(&c.path("data/health.toml")).medicines[0].schedule {
        Schedule::Hours { from, .. } => from,
        _ => 0,
    };
    assert_eq!((from(&desk), from(&phone)), (start, start), "the move lost to the edit, before any mend");
    // The window's minute on each device, then the sharing.
    let mend = |c: &Computer| {
        let state = HealthState::read(&c.path("state/health-state.toml")).unwrap();
        Health::change(&c.path("data/health.toml"), |h| Ok(h.mend_shifts(&state))).unwrap()
    };
    for (n, c) in [&desk, &phone, &desk, &phone].into_iter().enumerate() {
        mend(c);
        c.exchange(&folder, &key, NOW + (5 + n as i64) * MINUTE);
    }
    for c in [&desk, &phone] {
        let health = Health::load(&c.path("data/health.toml"));
        assert_eq!((from(c), health.medicines[0].dose.as_str()), (moved.1, "1 tablet, with food"), "{}", c.read("data/health.toml"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F5: a new device's records come before its notes and its entry (each
/// file travels on its own): it is heard of all the same, by its last record,
/// so that the doses count it, in doubt, until its entry comes. Before, only
/// its plain notes dated it: none there, never heard, not counted.
#[test]
fn a_device_known_by_its_records_alone_is_heard_of() {
    let base = scratch("records-alone");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    phone.write("config/safe-senders.txt", "a@example.org\n");
    phone.exchange(&folder, &key, NOW);
    std::fs::remove_file(folder.join(format!("{}.toml", phone.id))).unwrap();
    let heard = others_heard(&folder, &key, &desk.id);
    let found = heard.iter().find(|o| o.id == phone.id).map(|o| o.heard);
    assert_eq!(found, Some(NOW / 1000), "{heard:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F22: sharing set up again with another folder while the memory stayed
/// (the keyring's key lost, the passphrase typed again with another folder):
/// the memory of the first sharing means nothing there. Before, it counted as
/// joined, and what the desk held never went to the second folder.
#[test]
fn set_up_again_with_another_folder_the_memory_starts_anew() {
    let base = scratch("another-folder");
    let (first, second) = (base.join("First"), base.join("Second"));
    let key = quick_key(&first, "the first passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&first, &key, NOW);
    let other = quick_key(&second, "a second passphrase").unwrap();
    let laptop = Computer::new(&base, "laptop");
    laptop.exchange(&second, &other, NOW + MINUTE);
    desk.exchange(&second, &other, NOW + 2 * MINUTE);
    laptop.exchange(&second, &other, NOW + 3 * MINUTE);
    assert!(laptop.read("config/safe-senders.txt").contains("a@example.org"), "what the desk holds went to the second folder");
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F5: the phone sealed its copy of the folder before the desk's seal came,
/// and wrote there under its own key; its records came into the desk's folder,
/// which no one can open. Set up again with the desk's passphrase, the phone
/// starts a new full round under it, and the desk reads on from there.
#[test]
fn two_devices_that_sealed_apart_meet_once_one_is_set_up_again() {
    let base = scratch("sealed-apart");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "the desk's passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "desk@example.org\n");
    desk.exchange(&folder, &key, NOW);
    let copy = base.join("phone-copy");
    let own = quick_key(&copy, "the phone's own passphrase").unwrap();
    let phone = Computer::new(&base, "phone");
    phone.write("config/safe-senders.txt", "phone@example.org\n");
    phone.exchange(&copy, &own, NOW + MINUTE);
    std::fs::copy(round_file(&copy, &phone.id, 1), round_file(&folder, &phone.id, 1)).unwrap();
    let outcome = desk.exchange(&folder, &key, NOW + 2 * MINUTE);
    assert!(outcome.problems.iter().any(|p| p.starts_with("share-other-seal")), "{outcome:?}");
    let outcome = phone.exchange(&folder, &key, NOW + 3 * MINUTE);
    assert!(!outcome.problems.iter().any(|p| p.starts_with("share-sealed")), "{outcome:?}");
    desk.exchange(&folder, &key, NOW + 4 * MINUTE);
    assert!(desk.read("config/safe-senders.txt").contains("phone@example.org"), "the desk reads the phone's new round");
    assert!(phone.read("config/safe-senders.txt").contains("desk@example.org"));
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F22: a draft sent on the desk (taken out) while the laptop, not knowing
/// yet, changed it twice, later: before, the later word won, and the draft
/// came back on the desk, to be sent again. It stays sent on every device; the
/// laptop's changes are kept among its earlier versions.
#[test]
fn a_draft_sent_never_comes_back_from_changes_made_meanwhile() {
    let base = scratch("draft-sent");
    let (desk_folder, laptop_folder) = (base.join("desk").join("Sioul"), base.join("laptop").join("Sioul"));
    let key = quick_key(&desk_folder, "four words make a passphrase").unwrap();
    carry(&desk_folder, &laptop_folder, "seal.toml", None);
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let sync = |from: &Path, to: &Path, who: &Computer| {
        for entry in std::fs::read_dir(from).unwrap().filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&who.id) && name.ends_with(".jsonl") {
                carry(from, to, &name, None);
            }
        }
    };
    desk.write("data/drafts/reply.eml", "Subject: Rent\n\nFirst words.\n");
    desk.exchange(&desk_folder, &key, NOW);
    sync(&desk_folder, &laptop_folder, &desk);
    laptop.exchange(&laptop_folder, &key, NOW + MINUTE);
    assert!(laptop.read("data/drafts/reply.eml").contains("First words."));
    // Sent from the desk at minute 3; the laptop, its sync late, changes it at minutes 4 and 5.
    std::fs::remove_file(desk.path("data/drafts/reply.eml")).unwrap();
    desk.exchange(&desk_folder, &key, NOW + 3 * MINUTE);
    laptop.write("data/drafts/reply.eml", "Subject: Rent\n\nFirst words, and more.\n");
    laptop.exchange(&laptop_folder, &key, NOW + 4 * MINUTE);
    laptop.write("data/drafts/reply.eml", "Subject: Rent\n\nFirst words, and more, and more.\n");
    laptop.exchange(&laptop_folder, &key, NOW + 5 * MINUTE);
    // Each reads the other.
    sync(&laptop_folder, &desk_folder, &laptop);
    sync(&desk_folder, &laptop_folder, &desk);
    desk.exchange(&desk_folder, &key, NOW + 6 * MINUTE);
    laptop.exchange(&laptop_folder, &key, NOW + 7 * MINUTE);
    assert!(!desk.path("data/drafts/reply.eml").exists(), "sent: never back on the desk");
    assert!(!laptop.path("data/drafts/reply.eml").exists(), "sent: taken out on the laptop too");
    // A device joining reads it all: sent there too.
    let tablet = Computer::new(&base, "tablet");
    sync(&laptop_folder, &desk_folder, &laptop);
    tablet.exchange(&desk_folder, &key, NOW + 8 * MINUTE);
    assert!(!tablet.path("data/drafts/reply.eml").exists());
    let kept = crate::history::versions(&crate::history::root(&laptop.memory), "drafts", "data/drafts/reply.eml");
    assert!(!kept.is_empty(), "the laptop's change kept among its earlier versions");
    let _ = std::fs::remove_dir_all(&base);
}

/// SS-F22: two devices under one name (a disk or a state folder copied to a
/// new machine): each finds the other's lines in its own file, again and
/// again, and it is said (`share-twin`); a memory restored once is not.
#[test]
fn two_devices_under_one_name_are_said() {
    let base = scratch("twins");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    // Its state copied to a laptop, with its name.
    let laptop = Computer { id: desk.id.clone(), ..Computer::new(&base, "laptop") };
    copy_dir(&desk.roots.state, &laptop.roots.state);
    copy_dir(&desk.roots.config, &laptop.roots.config);
    let mut said = Vec::new();
    for n in 1..6 {
        let (one, now) = if n % 2 == 0 { (&desk, NOW + n * MINUTE) } else { (&laptop, NOW + n * MINUTE) };
        one.write("config/safe-senders.txt", &format!("a@example.org\nn{n}@example.org\n"));
        said.extend(one.exchange(&folder, &key, now).problems);
    }
    assert!(said.iter().any(|p| p == "share-twin"), "{said:?}");
    let _ = std::fs::remove_dir_all(&base);
}
