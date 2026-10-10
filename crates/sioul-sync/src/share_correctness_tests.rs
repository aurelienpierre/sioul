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

// ---------------------------------------------------------------- the review of 10 October 2026

fn senders(from: usize, to: usize) -> String {
    (from..to).map(|n| format!("sender{n}@example.org\n")).collect()
}

fn doses(marked: &[&str]) -> String {
    let mut text = String::from("[taken]\n");
    for dose in marked {
        text.push_str(&format!("\"{dose}\" = 1790000600\n"));
    }
    text
}

/// R1: rounds 2, 3 and 4 continue round 1, a dose in each; the phone's sync
/// app brings round 3 first, then 2 and 4. Round 2 missing while round 1 is
/// kept had not come yet (rounds go oldest first): waited for, never taken
/// for one gone. Every dose comes once every round is there; the doubt is
/// said meanwhile.
#[test]
fn continuing_rounds_brought_in_another_order_all_come() {
    let base = scratch("order");
    let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
    let key = quick_key(&server, "four words make a passphrase").unwrap();
    carry(&server, &phone_folder, "seal.toml", None);
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/known-senders.txt", &senders(0, 1500));
    desk.exchange(&server, &key, NOW);
    carry(&server, &phone_folder, &format!("{}-1.jsonl", desk.id), None);
    phone.exchange(&phone_folder, &key, NOW + 30_000);
    desk.write("config/known-senders.txt", &senders(0, 3000));
    desk.write("state/health-state.toml", &doses(&["a@1790000600"]));
    desk.exchange(&server, &key, NOW + MINUTE);
    desk.write("config/known-senders.txt", &senders(0, 4500));
    desk.write("state/health-state.toml", &doses(&["a@1790000600", "b@1790000700"]));
    desk.exchange(&server, &key, NOW + 2 * MINUTE);
    desk.write("state/health-state.toml", &doses(&["a@1790000600", "b@1790000700", "c@1790000800"]));
    desk.exchange(&server, &key, NOW + 3 * MINUTE);
    for r in 2..=4 {
        assert!(round_file(&server, &desk.id, r).exists(), "round {r}");
    }
    let wrote = written(&desk.memory, &desk.id).unwrap();
    carry(&server, &phone_folder, &format!("{}-3.jsonl", desk.id), None);
    phone.exchange(&phone_folder, &key, NOW + 3 * MINUTE + 30_000);
    assert!(!heard(&phone.memory, &phone.id).complete(&desk.id, wrote), "round 2 not here: not all known");
    carry(&server, &phone_folder, &format!("{}-2.jsonl", desk.id), None);
    carry(&server, &phone_folder, &format!("{}-4.jsonl", desk.id), None);
    for m in 4..6 {
        phone.exchange(&phone_folder, &key, NOW + m * MINUTE + 30_000);
    }
    let record = phone.read("state/health-state.toml");
    let h = heard(&phone.memory, &phone.id);
    assert!(record.contains("a@1790000600") && record.contains("b@1790000700") && record.contains("c@1790000800"), "every dose once every round is here (read {:?}):\n{record}", h.read.get(&desk.id));
    assert!(h.complete(&desk.id, wrote), "and known");
    let _ = std::fs::remove_dir_all(&base);
}

/// R2: one damaged line in the middle of a round (the phone's copy): said,
/// its time in doubt for the doses, and what the desk writes after it, in
/// the next round continuing that one, still comes.
#[test]
fn a_damaged_line_does_not_stop_the_rounds_after_it() {
    let base = scratch("damaged-middle");
    let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
    let key = quick_key(&server, "four words make a passphrase").unwrap();
    carry(&server, &phone_folder, "seal.toml", None);
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/known-senders.txt", &senders(0, 1500));
    desk.exchange(&server, &key, NOW);
    let first = format!("{}-1.jsonl", desk.id);
    carry(&server, &phone_folder, &first, None);
    // A line in its middle damaged on the phone (a letter of its seal changed).
    let path = phone_folder.join(&first);
    let mut bytes = std::fs::read(&path).unwrap();
    let start = line_end_before(&path, bytes.len() / 2);
    let seal_at = start + std::str::from_utf8(&bytes[start..]).unwrap().find("\"s\":\"").unwrap() + 10;
    bytes[seal_at] = if bytes[seal_at] == b'A' { b'B' } else { b'A' };
    std::fs::write(&path, &bytes).unwrap();
    let outcome = phone.exchange(&phone_folder, &key, NOW + 30_000);
    assert!(outcome.problems.iter().any(|p| p.starts_with("share-other-seal")), "{:?}", outcome.problems);
    assert!(heard(&phone.memory, &phone.id).broken.contains_key(&desk.id), "said, for the doses");
    desk.write("state/health-state.toml", &doses(&["a@1790000600"]));
    desk.exchange(&server, &key, NOW + MINUTE);
    assert!(round_file(&server, &desk.id, 2).exists());
    carry(&server, &phone_folder, &format!("{}-2.jsonl", desk.id), None);
    for m in 2..4 {
        phone.exchange(&phone_folder, &key, NOW + m * MINUTE);
    }
    let record = phone.read("state/health-state.toml");
    assert!(record.contains("a@1790000600"), "the dose of round 2 comes after one damaged line in round 1:\n{record}");
    let _ = std::fs::remove_dir_all(&base);
}

/// R1 and R14: the "late" carrier (half the changed files at each look, in
/// any order) while the desk writes past a segment each minute and marks a
/// dose each five: no dose is ever known wrongly, and after quiet minutes
/// with every file brought, every dose is there and known.
#[test]
fn doses_come_whatever_a_late_carrier_brings_across_segments() {
    let base = scratch("late-segments");
    let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
    let key = quick_key(&server, "four words make a passphrase").unwrap();
    carry(&server, &phone_folder, "seal.toml", None);
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut marked: Vec<String> = Vec::new();
    let mut claims: Vec<((u32, u64), usize)> = Vec::new();
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut brought: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut bring = |all: bool, seed: &mut u64| {
        let mut found = Vec::new();
        list_files(&server, &server, &[], &mut found);
        for (name, path) in found {
            let bytes = std::fs::read(&path).unwrap();
            if brought.get(&name) == Some(&bytes) {
                continue;
            }
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            if !all && *seed % 2 == 0 {
                continue;
            }
            std::fs::create_dir_all(phone_folder.join(&name).parent().unwrap()).unwrap();
            std::fs::write(phone_folder.join(&name), &bytes).unwrap();
            brought.insert(name, bytes);
        }
    };
    let mut wrong = Vec::new();
    for minute in 0..60i64 {
        let now = NOW + minute * MINUTE;
        if minute % 5 == 0 {
            marked.push(format!("dose{minute}@1790000600"));
        }
        let refs: Vec<&str> = marked.iter().map(String::as_str).collect();
        desk.write("state/health-state.toml", &doses(&refs));
        desk.write("config/known-senders.txt", &senders(0, 300 * (minute as usize + 1)));
        desk.exchange(&server, &key, now);
        claims.push((written(&desk.memory, &desk.id).unwrap(), marked.len()));
        bring(false, &mut seed);
        phone.exchange(&phone_folder, &key, now + 30_000);
        let h = heard(&phone.memory, &phone.id);
        let record = phone.read("state/health-state.toml");
        for (wrote, count) in &claims {
            if h.complete(&desk.id, *wrote) {
                for dose in &marked[..*count] {
                    if !record.contains(dose.as_str()) {
                        wrong.push(format!("minute {minute}: {dose} known not taken (wrote {wrote:?}, read {:?})", h.read.get(&desk.id)));
                    }
                }
            }
        }
    }
    assert!(rounds(&server).get(&desk.id).is_some_and(|r| r.len() > 10), "the desk's rounds continue one another");
    for minute in 60..70i64 {
        let now = NOW + minute * MINUTE;
        desk.exchange(&server, &key, now);
        bring(true, &mut seed);
        phone.exchange(&phone_folder, &key, now + 30_000);
    }
    assert!(wrong.is_empty(), "known wrongly: {wrong:?}");
    let record = phone.read("state/health-state.toml");
    let missing: Vec<&String> = marked.iter().filter(|d| !record.contains(d.as_str())).collect();
    let h = heard(&phone.memory, &phone.id);
    assert!(missing.is_empty(), "never came with every file here: {missing:?}; read {:?}", h.read.get(&desk.id));
    assert!(h.complete(&desk.id, written(&desk.memory, &desk.id).unwrap()), "and known");
    let _ = std::fs::remove_dir_all(&base);
}

/// R4: your version of an event set aside (a change made here and on its
/// server, `dav`) is kept as a shared file's versions are: the last 20, and
/// all those of 30 days; the sharing's hourly tidy took it for a file no
/// longer here and dropped it after 30 days.
#[test]
fn your_versions_of_an_event_stay_past_thirty_days() {
    let base = scratch("accounts-versions");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    let root = crate::history::root(&desk.memory);
    let event = "data/calendars/work/personal/dentist.ics";
    crate::history::keep_bytes(&root, crate::history::ACCOUNTS, event, b"BEGIN:VCALENDAR\r\nSUMMARY:Dentist\r\nEND:VCALENDAR\r\n", NOW - 31 * 86_400_000).unwrap();
    let gone = "data/calendars/work/personal/moved.ics";
    crate::history::keep_bytes(&root, crate::history::ACCOUNTS, gone, b"BEGIN:VCALENDAR\r\nSUMMARY:Moved\r\nEND:VCALENDAR\r\n", NOW - 31 * 86_400_000).unwrap();
    desk.write(event, "BEGIN:VCALENDAR\r\nSUMMARY:Dentist (server)\r\nEND:VCALENDAR\r\n");
    desk.exchange(&folder, &key, NOW);
    assert_eq!(crate::history::versions(&root, crate::history::ACCOUNTS, event).len(), 1, "your version, set aside 31 days ago, is kept");
    assert_eq!(crate::history::versions(&root, crate::history::ACCOUNTS, gone).len(), 1, "the item gone from its server: kept all the same");
    let _ = std::fs::remove_dir_all(&base);
}

/// R5: one device, its process stopped after appending and before saving its
/// memory (a phone's process killed mid-exchange), twice within six hours:
/// its own lines (`appended`), never said to be another device's under its
/// name; two devices under one name still are
/// (`two_devices_under_one_name_are_said`).
#[test]
fn one_device_stopped_twice_is_not_said_to_be_two() {
    let base = scratch("stopped-twice");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    let mut said = Vec::new();
    for n in 1..=2i64 {
        let kept = std::fs::read(&desk.memory).unwrap();
        desk.write("config/safe-senders.txt", &format!("a@example.org\nn{n}@example.org\n"));
        desk.exchange(&folder, &key, NOW + n * 60 * MINUTE);
        // Stopped before its memory reached the disk: the memory as it was.
        std::fs::write(&desk.memory, &kept).unwrap();
        said.extend(desk.exchange(&folder, &key, NOW + n * 60 * MINUTE + MINUTE).problems);
    }
    assert!(said.iter().any(|p| p == "share-own-ahead"), "{said:?}");
    assert!(!said.iter().any(|p| p == "share-twin"), "a single device said to be two: {said:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// R13: a round of this device's found without a line (its first writing
/// lost, a placeholder of a file kept on demand) ahead of its memory: its
/// next line says where it stands from the folder (it continues the round
/// before, as that round ends), never a turn kept from an older round; the
/// others read on.
#[test]
fn an_empty_own_round_found_ahead_says_where_it_stands() {
    let base = scratch("empty-own-round");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/known-senders.txt", &senders(0, 1500));
    desk.exchange(&folder, &key, NOW);
    desk.write("state/health-state.toml", &doses(&["a@1790000600"]));
    desk.exchange(&folder, &key, NOW + MINUTE);
    assert!(round_file(&folder, &desk.id, 2).exists(), "round 2 continues round 1");
    phone.exchange(&folder, &key, NOW + MINUTE + 30_000);
    // Round 3 there, empty.
    std::fs::write(round_file(&folder, &desk.id, 3), "").unwrap();
    desk.write("state/health-state.toml", &doses(&["a@1790000600", "b@1790000700"]));
    let outcome = desk.exchange(&folder, &key, NOW + 2 * MINUTE);
    assert!(outcome.problems.iter().any(|p| p == "share-own-ahead"), "{:?}", outcome.problems);
    for m in 3..5 {
        phone.exchange(&folder, &key, NOW + m * MINUTE);
    }
    let record = phone.read("state/health-state.toml");
    assert!(record.contains("b@1790000700"), "the dose after the empty round's turn:\n{record}");
    assert!(heard(&phone.memory, &phone.id).complete(&desk.id, written(&desk.memory, &desk.id).unwrap()));
    let _ = std::fs::remove_dir_all(&base);
}

/// R7: a full round restates all its computer holds, and a reader waiting
/// for the end of an earlier round reads on from it (`later_full`). One
/// written while the doses' record could not be read (held empty after a
/// crash) left the marks out: it is not full, the reader keeps the doubt,
/// and a full round follows once the record reads again.
#[test]
fn a_round_that_could_not_restate_all_is_not_full() {
    let base = scratch("not-full");
    let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
    let key = quick_key(&server, "four words make a passphrase").unwrap();
    carry(&server, &phone_folder, "seal.toml", None);
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/safe-senders.txt", "a@example.org\n");
    let marks = doses(&["a@1790000600"]);
    desk.write("state/health-state.toml", &marks);
    desk.exchange(&server, &key, NOW);
    // An older copy of round 1 put back without its last line, the dose's;
    // the phone reads it so.
    let first = round_file(&server, &desk.id, 1);
    let cut = line_end_before(&first, std::fs::metadata(&first).unwrap().len() as usize - 1);
    let bytes = std::fs::read(&first).unwrap();
    std::fs::write(&first, &bytes[..cut]).unwrap();
    carry(&server, &phone_folder, &format!("{}-1.jsonl", desk.id), None);
    phone.exchange(&phone_folder, &key, NOW + 30_000);
    // The desk finds its file cut while its doses' record is of no bytes (a crash).
    desk.write("state/health-state.toml", "");
    let outcome = desk.exchange(&server, &key, NOW + MINUTE);
    assert!(outcome.problems.iter().any(|p| p == "share-own-cut"), "{:?}", outcome.problems);
    carry(&server, &phone_folder, &format!("{}-2.jsonl", desk.id), None);
    phone.exchange(&phone_folder, &key, NOW + MINUTE + 30_000);
    let wrote = written(&desk.memory, &desk.id).unwrap();
    let record = phone.read("state/health-state.toml");
    assert!(record.contains("a@1790000600") || !heard(&phone.memory, &phone.id).complete(&desk.id, wrote), "a round that left the marks out read as full: the dose known not taken:\n{record}");
    // The record reads again: a full round, and the dose comes.
    desk.write("state/health-state.toml", &marks);
    desk.exchange(&server, &key, NOW + 2 * MINUTE);
    for name in [2, 3].map(|r| format!("{}-{r}.jsonl", desk.id)) {
        if server.join(&name).exists() {
            carry(&server, &phone_folder, &name, None);
        }
    }
    phone.exchange(&phone_folder, &key, NOW + 2 * MINUTE + 30_000);
    let record = phone.read("state/health-state.toml");
    assert!(record.contains("a@1790000600"), "{record}");
    assert!(heard(&phone.memory, &phone.id).complete(&desk.id, written(&desk.memory, &desk.id).unwrap()));
    let _ = std::fs::remove_dir_all(&base);
}

/// R6: the doses' records the sharing writes (a mark received) are written
/// as the window writes them: on the disk before they take their name, with
/// their witness; found of no bytes after a power cut, the record is lost,
/// said, never read as a sound empty one.
#[test]
fn the_doses_records_the_sharing_writes_are_witnessed() {
    let base = scratch("witnessed");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("state/health-state.toml", &doses(&["a@1790000600"]));
    desk.exchange(&folder, &key, NOW);
    phone.exchange(&folder, &key, NOW + 30_000);
    let record = phone.path("state/health-state.toml");
    assert!(std::fs::read_to_string(&record).unwrap().contains("a@1790000600"));
    assert!(sioul_core::health::witnessed(&record), "its witness made");
    std::fs::write(&record, "").unwrap();
    assert_eq!(sioul_core::health::HealthState::read(&record), Err(sioul_core::health::Unsound::Lost));
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- the review of 10 October 2026, second pass

/// S2-A: a damaged line between a line days older and one after it, all
/// read in one exchange: never counted as read (`read_n` stays before it),
/// dated by the line after it, never by the older line before it; the doses
/// doubt (not all known) where they took the dose it marked for not taken.
/// The reader asks the writer for a full round (`Seen::full`), which brings
/// the dose and ends the doubt.
#[test]
fn a_lost_line_is_never_counted_read_and_a_full_round_mends_it() {
    let base = scratch("lost-dated");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    phone.exchange(&folder, &key, NOW + 30_000);
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
    desk.exchange(&folder, &key, NOW + 10 * MINUTE);
    let day3 = NOW + 3 * 86_400_000;
    let due = day3 / 1000 - 300;
    let file = round_file(&folder, &desk.id, 1);
    let before = std::fs::metadata(&file).unwrap().len() as usize;
    desk.write("state/health-state.toml", &format!("[taken]\n\"iron@{due}\" = {}\n", due + 60));
    desk.exchange(&folder, &key, day3);
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\nc@example.org\n");
    desk.exchange(&folder, &key, day3 + MINUTE);
    assert!(!round_file(&folder, &desk.id, 2).exists());
    // The dose's line damaged (a letter of its seal changed), the line after it whole.
    let mut bytes = std::fs::read(&file).unwrap();
    let seal_at = before + std::str::from_utf8(&bytes[before..]).unwrap().find("\"s\":\"").unwrap() + 10;
    bytes[seal_at] = if bytes[seal_at] == b'A' { b'B' } else { b'A' };
    std::fs::write(&file, &bytes).unwrap();
    let now = day3 + 2 * MINUTE;
    phone.exchange(&folder, &key, now);
    let doubts = |phone: &Computer, now: i64| {
        let h = heard(&phone.memory, &phone.id);
        let complete = h.complete(&desk.id, written(&desk.memory, &desk.id).unwrap());
        let said = sioul_core::health::Said { started: due - 7_200, closed: now / 1000 - 30, working: false, exported: now / 1000 - 30, doses: true, ..Default::default() };
        let peer = sioul_core::health::Peer { id: desk.id.clone(), name: "desk".into(), said: Some(said), complete, seen: now / 1000 - 20, heard: now / 1000 - 30, broken: h.broken.get(&desk.id).copied(), ..Default::default() };
        sioul_core::health::doubts_now(due, now / 1000, None, &[peer])
    };
    let record = phone.read("state/health-state.toml");
    assert!(!record.contains(&format!("iron@{due}")), "the dose's line is lost here");
    assert!(!doubts(&phone, now).is_empty(), "a dose taken never known not taken");
    let h = heard(&phone.memory, &phone.id);
    assert!(h.broken[&desk.id] >= (day3 + MINUTE) / 1000 - 1, "dated by the line after it: {}", h.broken[&desk.id]);
    // Asked for a full round; the desk writes one at its next exchange.
    assert!(read_seen(&folder, &phone.id).is_some_and(|seen| seen.full.contains_key(&desk.id)), "the phone asks the desk for a full round");
    desk.exchange(&folder, &key, now + MINUTE);
    let m = Memory::load(&desk.memory, &desk.id);
    assert_eq!(m.full_round, m.round, "a full round, asked for");
    phone.exchange(&folder, &key, now + 2 * MINUTE);
    assert!(phone.read("state/health-state.toml").contains(&format!("iron@{due}")), "the dose comes with the full round");
    assert!(heard(&phone.memory, &phone.id).complete(&desk.id, written(&desk.memory, &desk.id).unwrap()), "and all is known again");
    assert!(read_seen(&folder, &phone.id).is_some_and(|seen| seen.full.is_empty()), "nothing more asked");
    let _ = std::fs::remove_dir_all(&base);
}

/// S2-B: a part switched off on this device leaves its entries in the
/// memory, which no round can restate: passed over, they stop no round from
/// being full.
#[test]
fn a_part_switched_off_stops_no_full_round() {
    let base = scratch("part-off-full");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/known-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    let config = Config::default();
    let off = stores_of(&config, &desk.roots, &|part| part != "senders" && shared_by_default(part, &config));
    let sharing = Sharing { folder: &folder, computer: &desk.id, key: &key, memory: &desk.memory, files: true, hurry: None };
    desk.write("data/drafts/big.toml", &format!("body = \"{}\"\n", "x".repeat(ROUND_SIZE as usize + 1000)));
    exchange(&sharing, &off, NOW + MINUTE).unwrap();
    let m = Memory::load(&desk.memory, &desk.id);
    assert_eq!(m.full_round, m.round, "past ROUND_SIZE, a full round");
    assert!(m.entries.keys().any(|key| key.starts_with("config/known-senders.txt")), "the part's entries kept");
    let _ = std::fs::remove_dir_all(&base);
}

/// S2-B: an entry of a store that left (`RETIRED`: the watch's days, the
/// texts' first folder) is forgotten, and stops no round from being full.
#[test]
fn a_retired_store_s_entry_is_forgotten_and_stops_no_full_round() {
    let base = scratch("retired-full");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/known-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    let mut memory: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&desk.memory).unwrap()).unwrap();
    let clock = memory["clock"].as_u64().unwrap();
    memory["entries"]["data/watch/2026-10-01.toml\u{1f}steps"] = serde_json::json!({ "c": clock, "w": desk.id, "h": "0123456789abcdef", "b": "" });
    std::fs::write(&desk.memory, memory.to_string()).unwrap();
    desk.write("data/drafts/big.toml", &format!("body = \"{}\"\n", "x".repeat(ROUND_SIZE as usize + 1000)));
    desk.exchange(&folder, &key, NOW + MINUTE);
    let m = Memory::load(&desk.memory, &desk.id);
    assert_eq!(m.full_round, m.round, "past ROUND_SIZE, a full round");
    assert!(!m.entries.keys().any(|key| key.starts_with("data/watch/")), "forgotten");
    let _ = std::fs::remove_dir_all(&base);
}

/// A round truly lost (taken out of the server, never to come) while the
/// round before it is kept: waited for an hour, the doses in doubt; then
/// passed over, the gap said: what comes after it flows (a dose marked
/// later is here), the doses still in doubt; the reader asks for a full
/// round, which brings what was lost and ends the doubt.
#[test]
fn a_round_truly_lost_is_passed_over_after_an_hour() {
    let base = scratch("round-lost");
    let (server, phone_folder) = (base.join("server").join("Sioul"), base.join("phone").join("Sioul"));
    let key = quick_key(&server, "four words make a passphrase").unwrap();
    carry(&server, &phone_folder, "seal.toml", None);
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/known-senders.txt", &senders(0, 1500));
    desk.exchange(&server, &key, NOW);
    carry(&server, &phone_folder, &format!("{}-1.jsonl", desk.id), None);
    phone.exchange(&phone_folder, &key, NOW + 30_000);
    desk.write("config/known-senders.txt", &senders(0, 3000));
    desk.write("state/health-state.toml", &doses(&["a@1790000600"]));
    desk.exchange(&server, &key, NOW + MINUTE);
    desk.write("state/health-state.toml", &doses(&["a@1790000600", "b@1790000700"]));
    desk.exchange(&server, &key, NOW + 2 * MINUTE);
    assert!(round_file(&server, &desk.id, 3).exists());
    std::fs::remove_file(round_file(&server, &desk.id, 2)).unwrap();
    carry(&server, &phone_folder, &format!("{}-3.jsonl", desk.id), None);
    let wrote = written(&desk.memory, &desk.id).unwrap();
    for m in 3..8 {
        phone.exchange(&phone_folder, &key, NOW + m * MINUTE);
    }
    assert!(!phone.read("state/health-state.toml").contains("b@1790000700"), "waited for round 2");
    assert!(!heard(&phone.memory, &phone.id).complete(&desk.id, wrote));
    // An hour on: passed over, the gap said; what came after flows, the doses still in doubt.
    let outcome = phone.exchange(&phone_folder, &key, NOW + 3 * MINUTE + AWAITED);
    assert!(outcome.problems.iter().any(|p| p.starts_with("share-other-gap")), "{:?}", outcome.problems);
    let record = phone.read("state/health-state.toml");
    assert!(record.contains("b@1790000700") && !record.contains("a@1790000600"), "{record}");
    assert!(!heard(&phone.memory, &phone.id).complete(&desk.id, wrote), "not all known: round 2 lost");
    // The phone asks the desk for a full round; the desk writes one.
    let note = seen_path(&phone_folder, &phone.id);
    assert!(read_seen(&phone_folder, &phone.id).is_some_and(|seen| seen.full.contains_key(&desk.id)));
    std::fs::copy(&note, seen_path(&server, &phone.id)).unwrap();
    desk.exchange(&server, &key, NOW + 70 * MINUTE);
    let m = Memory::load(&desk.memory, &desk.id);
    assert_eq!(m.full_round, m.round, "a full round, asked for");
    for r in rounds(&server).get(&desk.id).cloned().unwrap_or_default() {
        carry(&server, &phone_folder, &format!("{}-{r}.jsonl", desk.id), None);
    }
    phone.exchange(&phone_folder, &key, NOW + 71 * MINUTE);
    let record = phone.read("state/health-state.toml");
    assert!(record.contains("a@1790000600") && record.contains("b@1790000700"), "{record}");
    assert!(heard(&phone.memory, &phone.id).complete(&desk.id, written(&desk.memory, &desk.id).unwrap()));
    let _ = std::fs::remove_dir_all(&base);
}

/// A device that never held the doses' record and receives only removals
/// for it writes nothing: written empty, with its witness, it would read as
/// lost and be repaired again and again.
#[test]
fn removals_alone_write_no_doses_record() {
    let base = scratch("removals-alone");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("state/health-state.toml", &doses(&["a@1790000600"]));
    desk.exchange(&folder, &key, NOW);
    desk.write("state/health-state.toml", "[taken]\n");
    desk.exchange(&folder, &key, NOW + MINUTE);
    phone.exchange(&folder, &key, NOW + 2 * MINUTE);
    let record = phone.path("state/health-state.toml");
    assert!(!record.exists(), "{}", phone.read("state/health-state.toml"));
    assert!(sioul_core::health::HealthState::read(&record).is_ok(), "never read as lost");
    let _ = std::fs::remove_dir_all(&base);
}

/// Where its append ends is noted before each append (`appended`); a note
/// that cannot be written (a full disk) appends nothing, so that a line of
/// its own is never taken for another device's (`share-twin`).
#[test]
fn an_append_not_noted_is_not_made() {
    let base = scratch("not-noted");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange(&folder, &key, NOW);
    let first = round_file(&folder, &desk.id, 1);
    let size = std::fs::metadata(&first).unwrap().len();
    // The note's place taken (as a full disk refuses it).
    let note = desk.memory.with_file_name("appended.toml");
    std::fs::remove_file(&note).unwrap();
    std::fs::create_dir_all(note.join("taken")).unwrap();
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
    let stores = stores(&Config::default(), &desk.roots);
    let sharing = Sharing { folder: &folder, computer: &desk.id, key: &key, memory: &desk.memory, files: true, hurry: None };
    assert!(exchange(&sharing, &stores, NOW + MINUTE).is_err(), "said");
    assert_eq!(std::fs::metadata(&first).unwrap().len(), size, "nothing appended");
    std::fs::remove_dir_all(&note).unwrap();
    let mut said = Vec::new();
    for n in 2..5 {
        said.extend(desk.exchange(&folder, &key, NOW + n * MINUTE).problems);
    }
    assert!(!said.iter().any(|p| p == "share-own-ahead" || p == "share-twin"), "{said:?}");
    assert!(std::fs::metadata(&first).unwrap().len() > size, "appended once noted");
    let _ = std::fs::remove_dir_all(&base);
}
