// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The switch to format 2, and what crosses it (docs/database.md, "The format
//! of what travels"): a memory put back, a whole list from a device not
//! switched yet, a change made just before the switch, sessions without ids,
//! an older Sioul back, a hostile folder, a newer format met, a device
//! joining, records read again, earlier versions put back, and the switch
//! under eDrive. Each test was first written by the review of 9 October
//! 2026 to prove a finding (R1 to R14), and now holds what the fix gives.
//! Fiction only: example.org, invented amounts.

use super::format_tests::{announce, into_format_2, writes};
use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;

const PASSPHRASE: &str = "four words make a passphrase";

const BUDGET: &str = "[[budget]]\nid = \"home\"\ntitle = \"Home\"\nperiod = \"month\"\ntarget = 0\n";

fn two_devices(name: &str) -> (PathBuf, PathBuf, [u8; 32], Computer, Computer, PathBuf, PathBuf) {
    let base = scratch(name);
    let folder = base.join("Sioul");
    let key = quick_key(&folder, PASSPHRASE).unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    std::fs::create_dir_all(&desk_notes).unwrap();
    std::fs::create_dir_all(&phone_notes).unwrap();
    (base, folder, key, desk, phone, desk_notes, phone_notes)
}

fn rounds(folder: &Path, key: &[u8; 32], devices: &[(&Computer, &Path)], mut now: i64, rounds: usize) -> i64 {
    for _ in 0..rounds {
        for (computer, notes) in devices {
            computer.exchange_notes(folder, key, now, notes);
            now += MINUTE;
        }
    }
    now
}

/// Rounds where each device says after each exchange that it reads format 2 (as the window does).
fn rounds_announced(folder: &Path, key: &[u8; 32], devices: &[(&Computer, &Path)], mut now: i64, rounds: usize) -> i64 {
    for _ in 0..rounds {
        for (computer, notes) in devices {
            computer.exchange_notes(folder, key, now, notes);
            announce(folder, key, computer, 2, now);
            now += MINUTE;
        }
    }
    now
}

fn ledger_path(notes: &Path) -> PathBuf {
    notes.join("sioul-budgets.toml")
}

fn list_of(text: &str, name: &str) -> Vec<toml::Table> {
    let table: toml::Table = toml::from_str(text).unwrap_or_default();
    table.get(name).and_then(toml::Value::as_array).map(|list| list.iter().filter_map(|v| v.as_table().cloned()).collect()).unwrap_or_default()
}

fn lines(notes: &Path) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = list_of(&std::fs::read_to_string(ledger_path(notes)).unwrap_or_default(), "line")
        .iter()
        .map(|line| {
            let amount = line.get("amount").map_or(0, |a| a.as_float().map_or_else(|| a.as_integer().unwrap_or(0) * 100, |f| (f * 100.0).round() as i64));
            (line.get("label").and_then(toml::Value::as_str).unwrap_or_default().to_string(), amount)
        })
        .collect();
    out.sort();
    out
}

fn count(lines: &[(String, i64)], label: &str) -> usize {
    lines.iter().filter(|(l, _)| l == label).count()
}

fn sessions(computer: &Computer) -> Vec<(String, i64, i64, String)> {
    let mut out: Vec<(String, i64, i64, String)> = list_of(&computer.read("data/time/2026-09.toml"), "session")
        .iter()
        .map(|s| {
            let what = s.get("project").or_else(|| s.get("task")).and_then(toml::Value::as_str).unwrap_or_default().to_string();
            (what, s.get("start").and_then(toml::Value::as_integer).unwrap_or(0), s.get("minutes").and_then(toml::Value::as_integer).unwrap_or(0), s.get("invoice").and_then(toml::Value::as_str).unwrap_or_default().to_string())
        })
        .collect();
    out.sort();
    out
}

fn words(computer: &Computer, which: &str) -> Vec<String> {
    let config: toml::Table = toml::from_str(&computer.read("config/config.toml")).unwrap_or_default();
    let mut out: Vec<String> = config
        .get("words")
        .and_then(|w| w.get("codes"))
        .and_then(|w| w.get("code"))
        .and_then(|w| w.get(which))
        .and_then(toml::Value::as_array)
        .map(|list| list.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// A list of words changed as Settings ▸ Words writes it: the list rewritten whole.
fn change_words(computer: &Computer, which: &str, change: impl FnOnce(&mut Vec<String>)) {
    let mut doc: toml_edit::DocumentMut = computer.read("config/config.toml").parse().unwrap();
    let mut list = words(computer, which);
    change(&mut list);
    doc["words"]["codes"]["code"][which] = toml_edit::value(list.iter().map(String::as_str).collect::<toml_edit::Array>());
    computer.write("config/config.toml", &doc.to_string());
}

fn bread() -> sioul_core::budget::Line {
    sioul_core::budget::Line { budget: "home".into(), date: "2026-09-05".parse().unwrap(), amount: sioul_core::money::Money(-1200), label: "Boulangerie".into(), planned: false, reserve: None, links: Vec::new(), preset: None }
}

// ---------------------------------------------------------------- R1: a memory put back from before the switch

/// A memory put back from before the switch (a backup, or a switching
/// exchange stopped before its memory was saved): this device's own format-2
/// records, read again while its memory says format 1, are never written with
/// format 1's rules; they wait until it takes format 2 again, at its next
/// exchange. One line stays one on both devices (review R1).
#[test]
fn a_memory_put_back_from_before_the_switch_never_doubles_a_line() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r1-backup");
    std::fs::write(ledger_path(&desk_notes), BUDGET).unwrap();
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    // A backup of the desk's memory, while it writes format 1.
    let backup = base.join("backup");
    std::fs::create_dir_all(&backup).unwrap();
    for name in ["memory.json", "files.json"] {
        let _ = std::fs::copy(desk.memory.with_file_name(name), backup.join(name));
    }
    assert!(writes(&desk) < 2);
    now = into_format_2(&folder, &key, &both, now);
    sioul_core::budget::record_line(&ledger_path(&desk_notes), &bread(), "by hand").unwrap();
    now = rounds_announced(&folder, &key, &both, now, 2);
    // The phone writes something too: the restored memory has not read it, so the gate stays shut at first.
    let market = sioul_core::budget::Line { label: "Marché".into(), amount: sioul_core::money::Money(-2300), ..bread() };
    sioul_core::budget::record_line(&ledger_path(&phone_notes), &market, "by hand").unwrap();
    now = rounds_announced(&folder, &key, &both, now, 2);
    assert_eq!(count(&lines(&desk_notes), "Boulangerie"), 1);
    assert_eq!(count(&lines(&phone_notes), "Boulangerie"), 1);
    // The desk's memory put back.
    for name in ["memory.json", "files.json"] {
        let _ = std::fs::copy(backup.join(name), desk.memory.with_file_name(name));
    }
    let back = desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    let after_back = lines(&desk_notes);
    rounds_announced(&folder, &key, &both, now, 4);
    let (on_desk, on_phone) = (lines(&desk_notes), lines(&phone_notes));
    assert_eq!((count(&after_back, "Boulangerie"), count(&on_desk, "Boulangerie"), count(&on_phone, "Boulangerie")), (1, 1, 1), "right after the memory was put back ({back:?}): {after_back:?}; then desk {on_desk:?} phone {on_phone:?}");
    assert_eq!((count(&on_desk, "Marché"), count(&on_phone, "Marché")), (1, 1), "desk {on_desk:?} phone {on_phone:?}");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R2: a whole list from a device not switched yet

/// A device not switched yet sends a list of words whole (format 1) while the
/// other, switched, added a word one at a time: the whole list takes out
/// only what the previous whole list held and it no longer holds, never a
/// word added since. Both words stay, the same list on both devices (R2).
#[test]
fn a_whole_list_from_a_device_not_switched_yet_takes_out_only_what_it_held() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r2-whole-late");
    desk.write("config/config.toml", "[words.codes.code]\nadd = [\"Bestätigungscode\"]\n");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    assert_eq!(words(&phone, "add"), ["Bestätigungscode"]);
    // The phone updated: its entry says format 2; the desk switches.
    announce(&folder, &key, &phone, 2, now);
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    announce(&folder, &key, &desk, 2, now);
    now += MINUTE;
    assert_eq!(writes(&desk), 2);
    // The desk adds a word; its entry says how far it wrote.
    change_words(&desk, "add", |list| list.push("Sicherheitscode".into()));
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    announce(&folder, &key, &desk, 2, now);
    now += MINUTE;
    // The phone adds another before it has read that: format 1 for this exchange.
    change_words(&phone, "add", |list| list.push("Einmalcode".into()));
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    announce(&folder, &key, &phone, 2, now);
    now += MINUTE;
    rounds_announced(&folder, &key, &both, now, 4);
    let (on_desk, on_phone) = (words(&desk, "add"), words(&phone, "add"));
    assert_eq!(on_desk, on_phone, "both devices hold the same list (desk, phone)");
    assert_eq!(on_desk, ["Bestätigungscode", "Einmalcode", "Sicherheitscode"], "both words added stay (format 2's promise)");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R3: a removal made just before the switch

/// A word taken away just before the exchange that switches the device: that
/// exchange sends it in format 1 first, then switches. The word goes from the
/// other device too (R3).
#[test]
fn a_word_taken_away_just_before_the_switch_goes_from_the_other_device() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r3-removal");
    desk.write("config/config.toml", "[words.codes.code]\nadd = [\"Bestätigungscode\", \"Sicherheitscode\"]\n");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    assert_eq!(words(&phone, "add"), ["Bestätigungscode", "Sicherheitscode"]);
    announce(&folder, &key, &phone, 2, now);
    // Taken away on the desk, then its next exchange, the one that switches.
    change_words(&desk, "add", |list| list.retain(|w| w != "Sicherheitscode"));
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    announce(&folder, &key, &desk, 2, now);
    now += MINUTE;
    assert_eq!(writes(&desk), 2);
    rounds_announced(&folder, &key, &both, now, 4);
    assert_eq!(words(&desk, "add"), ["Bestätigungscode"]);
    assert_eq!(words(&phone, "add"), ["Bestätigungscode"], "the word taken away on the desk goes from the phone too");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R4: a session without an id changed on two devices

/// A session without an id (every session 0.0.4 noted) changed on two devices
/// at once: the switch wrote into it, on both, the id derived from its start,
/// task and project, so it stays one session, the later change winning, as
/// format 1 kept one (R4).
#[test]
fn a_session_without_id_changed_on_two_devices_stays_one() {
    for format in [1, 2] {
        let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices(&format!("sw-r4-legacy-session-{format}"));
        let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
        // As 0.0.4 wrote it: no id.
        desk.write("data/time/2026-09.toml", "[[session]]\nproject = \"garden\"\nstart = 1789912800\nminutes = 30\ndone = false\n");
        let mut now = rounds(&folder, &key, &both, NOW, 2);
        if format == 2 {
            now = into_format_2(&folder, &key, &both, now);
        }
        assert_eq!(sessions(&phone).len(), 1);
        let key_of = |computer: &Computer| sioul_core::timelog::sessions_in(&computer.path("data/time"))[0].key();
        sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.invoice = "2026-001".into()).unwrap();
        sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of(&phone)], |s| s.minutes = 50).unwrap();
        rounds(&folder, &key, &both, now, 3);
        let (here, there) = (sessions(&desk), sessions(&phone));
        assert_eq!((here.len(), there.len()), (1, 1), "format {format}: one session on each device: desk {here:?} phone {there:?}");
        let _ = std::fs::remove_dir_all(&base);
    }
}

// ---------------------------------------------------------------- R5: an older Sioul back after the switch

/// An older Sioul back after the switch takes out its copy of a line as it was
/// before a change made since: the removal names a version no device holds
/// under that name any more, and is passed over. The current line stays on
/// every device (R5).
#[test]
fn an_older_sioul_taking_out_a_stale_copy_leaves_the_current_line() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r5-older-back");
    std::fs::write(ledger_path(&desk_notes), BUDGET).unwrap();
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    let fuel = sioul_core::budget::Line { label: "Essence".into(), amount: sioul_core::money::Money(-6000), ..bread() };
    sioul_core::budget::record_line(&ledger_path(&desk_notes), &fuel, "by hand").unwrap();
    now = rounds_announced(&folder, &key, &both, now, 2);
    // The line as it was first (what an older Sioul holds of it), named as format 1 names it.
    let first = list_of(&std::fs::read_to_string(ledger_path(&desk_notes)).unwrap(), "line").into_iter().next().unwrap();
    let stale = leaf_text(&toml::Value::Table(first));
    // Edited on the desk: -60 → -65.
    let text = std::fs::read_to_string(ledger_path(&desk_notes)).unwrap().replace("amount = -60", "amount = -65");
    std::fs::write(ledger_path(&desk_notes), text).unwrap();
    now = rounds_announced(&folder, &key, &both, now, 2);
    assert_eq!(lines(&phone_notes), [("Essence".to_string(), -6500)]);
    // An older Sioul (0.0.3) back: it holds both versions and takes out the stale one.
    let older = uuid::Uuid::new_v4().to_string();
    let change = Change { k: format!("notes/sioul-budgets.toml#line{SEP}{MARK}{stale}"), v: None, b: String::new(), build: "0.0.3 (f054336abcde)".into(), f: 0 };
    super::tests::append_to(&round_file(&folder, &older, 1), &super::tests::record_of(&key, &older, 1, ((now as u64) << 16) | 1, &change));
    now = rounds_announced(&folder, &key, &both, now + MINUTE, 2);
    let _ = now;
    assert_eq!(lines(&desk_notes), [("Essence".to_string(), -6500)], "the current line stays on the desk");
    assert_eq!(lines(&phone_notes), [("Essence".to_string(), -6500)], "and on the phone");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R6: a hostile folder forces the switch

/// A hostile folder takes a live 0.0.3 device's sealed entry away and dates
/// its plain notes a year back: that device still counts, from what is sealed
/// (its records read here), and still holds format 2 back (R6).
#[test]
fn a_hostile_folder_never_opens_the_switch_past_a_live_older_device() {
    let (base, folder, key, desk, _, desk_notes, _) = two_devices("sw-r6-hostile");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange_notes(&folder, &key, NOW, &desk_notes);
    announce(&folder, &key, &desk, 2, NOW);
    // A live 0.0.3 device: records, an entry saying no format, recent notes.
    let older = uuid::Uuid::new_v4().to_string();
    let change = Change { k: "config/safe-senders.txt#b@example.org".into(), v: Some(String::new()), b: String::new(), build: "0.0.3 (f054336abcde)".into(), f: 0 };
    super::tests::append_to(&round_file(&folder, &older, 1), &super::tests::record_of(&key, &older, 1, ((NOW + MINUTE) as u64) << 16, &change));
    let entry = crate::devices::Entry { id: older.clone(), version: "0.0.3".into(), working: true, started: (NOW + MINUTE) / 1000, wrote: Some((1, 1)), ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &entry).unwrap();
    std::fs::write(folder.join(format!("{older}.toml")), format!("at = {}\nround = 1\n", (NOW + MINUTE) / 1000)).unwrap();
    // A third device, updated, that reads format 2.
    let laptop = Computer::new(&base, "laptop");
    let laptop_notes = base.join("laptop-notes");
    std::fs::create_dir_all(&laptop_notes).unwrap();
    laptop.exchange_notes(&folder, &key, NOW + 2 * MINUTE, &laptop_notes);
    announce(&folder, &key, &laptop, 2, NOW + 2 * MINUTE);
    desk.exchange_notes(&folder, &key, NOW + 3 * MINUTE, &desk_notes);
    assert!(writes(&desk) < 2, "the 0.0.3 device holds format 2 back");
    // The server: the older device's entry taken away, its plain notes dated a year back.
    std::fs::remove_file(folder.join("devices").join(format!("{older}.device"))).unwrap();
    std::fs::write(folder.join(format!("{older}.toml")), format!("at = {}\nround = 1\n", (NOW - 365 * 86_400_000) / 1000)).unwrap();
    desk.exchange_notes(&folder, &key, NOW + 4 * MINUTE, &desk_notes);
    desk.exchange_notes(&folder, &key, NOW + 5 * MINUTE, &desk_notes);
    assert!(writes(&desk) < 2, "a device whose records this desk read in the last minutes still holds format 2 back, whatever its plain notes say");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R7: doses under a newer format

/// The doses' part met in a newer format: the newer device's dose records,
/// waiting unwritten, are not counted as heard (`heard`), and this device's
/// exchange claims no export while its own answers wait: the others doubt,
/// never take a dose as known not taken (R7, docs/health.md "Knowing").
#[test]
fn a_held_doses_part_is_neither_heard_nor_claimed_as_exported() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r7-doses");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("state/health-doses.toml", "[dose.d1]\ndue = 1789900000\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    // A newer Sioul elsewhere writes the health part in a newer format: an answer, taken.
    let newer = uuid::Uuid::new_v4().to_string();
    let theirs = Change { k: format!("state/health-doses.toml#dose{SEP}d1{SEP}answer{SEP}{newer}"), v: Some("[v]\ntaken = true\n".into()), b: String::new(), build: "0.1.0 (abcdef123456)".into(), f: 3 };
    super::tests::append_to(&round_file(&folder, &newer, 1), &super::tests::record_of(&key, &newer, 1, ((now as u64) << 16) | 1, &theirs));
    let met = phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    assert!(met.problems.iter().any(|p| p == "share-newer:health"), "{met:?}");
    let heard_newer = heard(&phone.memory, &phone.id).complete(&newer, (1, 1));
    let written_here = phone.read("state/health-doses.toml").contains(&newer);
    let first_holds = !heard_newer || written_here;
    // The phone's own answer, held.
    let mine = format!("{}[dose.d1.answer.{}]\ntaken = true\n", phone.read("state/health-doses.toml"), phone.id);
    phone.write("state/health-doses.toml", &mine);
    let out = phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    let came = desk.read("state/health-doses.toml").contains(&phone.id);
    let second_holds = came || out.looked == 0;
    assert!(first_holds && second_holds, "heard but never written here: {}; own answer held while the exchange says it looked (its entry's export): {} ({out:?})", !first_holds, !second_holds);
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R8: held changes win over later ones once updated

/// A change made here while its part was held (a newer format met), then a
/// later change of the newer device: once this Sioul reads that format, the
/// held change goes out at the time it was found, and the newer device's
/// later change wins over it everywhere (R8). The update is simulated by
/// editing the memory as an updated Sioul reads it.
#[test]
fn a_change_held_under_a_newer_format_never_beats_its_later_ones() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r8-held");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\nid = \"aa\"\nproject = \"garden\"\nstart = 1789912800\nminutes = 30\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    let newer = uuid::Uuid::new_v4().to_string();
    let session = format!("data/time/2026-09.toml#session{SEP}{MARK}aa");
    let said = |minutes: u32, n: u64, at: i64| {
        let change = Change { k: session.clone(), v: Some(format!("[v]\nid = \"aa\"\nproject = \"garden\"\nstart = 1789912800\nminutes = {minutes}\n")), b: String::new(), build: if n == 1 { "0.1.0 (abcdef123456)".into() } else { String::new() }, f: 3 };
        super::tests::append_to(&round_file(&folder, &newer, 1), &super::tests::record_of(&key, &newer, n, ((at as u64) << 16) | n, &change));
    };
    // The newer device's first change: the phone meets format 3 and holds the time part.
    said(31, 1, now);
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    // Corrected on the phone (held), then changed later on the newer device.
    let key_of = sioul_core::timelog::sessions_in(&phone.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of], |s| s.minutes = 45).unwrap();
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    said(77, 2, now);
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += 10 * MINUTE;
    // The phone updated: the part no longer newer than its Sioul, the waiting change in a format it knows.
    let mut memory: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&phone.memory).unwrap()).unwrap();
    memory.as_object_mut().unwrap().remove("newer");
    for (_, waiting) in memory["waiting"].as_object_mut().unwrap().iter_mut() {
        waiting[3] = serde_json::json!(2);
    }
    std::fs::write(&phone.memory, serde_json::to_string(&memory).unwrap()).unwrap();
    rounds(&folder, &key, &both, now, 2);
    let minutes = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time")).iter().map(|s| s.minutes).collect::<Vec<_>>();
    assert_eq!(minutes(&phone), [77], "the newer device's later change, not the phone's earlier held one");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R9: a device joining after a translation

/// A device joining after a translation (a session 0.0.3 wrote back without
/// its id, kept here under its id) holds that session once: the device that
/// kept the id sent it again under that id, and took out the name a device
/// holding none of it would give; a later change reaches the joiner as one
/// session (R9).
#[test]
fn a_device_joining_after_a_translation_holds_each_session_once() {
    let base = scratch("sw-r9-join-after");
    super::format_tests::from_fixtures("a", &base);
    let folder = base.join("Sioul");
    let key = quick_key(&folder, PASSPHRASE).unwrap();
    let (desk, phone) = (super::format_tests::fixed(&base, "desk", super::format_tests::DESK), super::format_tests::fixed(&base, "phone", super::format_tests::PHONE));
    let desk_notes = base.join("desk-notes");
    announce(&folder, &key, &phone, 2, NOW + 15 * MINUTE);
    desk.exchange_notes(&folder, &key, NOW + 20 * MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 2);
    // The phone's 0.0.3 records (stage b), arriving late.
    let later = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("format-1").join("b").join("Sioul");
    for name in [format!("{}-1.jsonl", super::format_tests::PHONE), format!("{}.toml", super::format_tests::PHONE)] {
        std::fs::copy(later.join(&name), folder.join(&name)).unwrap();
    }
    desk.exchange_notes(&folder, &key, NOW + 21 * MINUTE, &desk_notes);
    announce(&folder, &key, &desk, 2, NOW + 21 * MINUTE);
    // A laptop joins.
    let laptop = Computer::new(&base, "laptop");
    let laptop_notes = base.join("laptop-notes");
    std::fs::create_dir_all(&laptop_notes).unwrap();
    announce(&folder, &key, &laptop, 2, NOW + 22 * MINUTE);
    laptop.exchange_notes(&folder, &key, NOW + 22 * MINUTE, &laptop_notes);
    announce(&folder, &key, &laptop, 2, NOW + 22 * MINUTE);
    let admin = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time")).into_iter().filter(|s| s.project == "admin").map(|s| s.minutes).collect::<Vec<_>>();
    assert_eq!(admin(&laptop), [35], "the laptop reads the admin session once");
    // The desk corrects it.
    let on_desk = sioul_core::timelog::sessions_in(&desk.path("data/time")).into_iter().find(|s| s.project == "admin").unwrap();
    sioul_core::timelog::update_in(&desk.path("data/time"), &[on_desk.key()], |s| s.minutes = 40).unwrap();
    desk.exchange_notes(&folder, &key, NOW + 23 * MINUTE, &desk_notes);
    laptop.exchange_notes(&folder, &key, NOW + 24 * MINUTE, &laptop_notes);
    desk.exchange_notes(&folder, &key, NOW + 25 * MINUTE, &desk_notes);
    laptop.exchange_notes(&folder, &key, NOW + 26 * MINUTE, &laptop_notes);
    assert_eq!(admin(&desk), [40]);
    assert_eq!(admin(&laptop), [40], "one admin session on the laptop, corrected");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R10: convergence through the switch, eDrive between

/// eDrive as `tests::Edrive` models it: a file changed on the phone goes up;
/// one changed on the server comes down only when its size differs; nothing
/// is ever deleted.
#[derive(Default)]
struct SizeOnly {
    known: BTreeMap<String, (String, String)>,
}

impl SizeOnly {
    fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut found = Vec::new();
        list_files(root, root, &[], &mut found);
        found.into_iter().filter(|(name, _)| !name.split('/').any(|p| p.starts_with('.'))).filter_map(|(name, path)| std::fs::read(path).ok().map(|b| (name, b))).collect()
    }

    fn carry(&mut self, server: &Path, phone: &Path) {
        let digest = |bytes: &[u8]| hash(&String::from_utf8_lossy(bytes));
        let (up, down) = (Self::files(phone), Self::files(server));
        for (name, bytes) in &up {
            if self.known.get(name).is_none_or(|(_, local)| *local != digest(bytes)) {
                let to = server.join(name);
                std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                std::fs::write(&to, bytes).unwrap();
                self.known.insert(name.clone(), (digest(bytes), digest(bytes)));
            }
        }
        for (name, bytes) in &down {
            let local = phone.join(name);
            match (self.known.get(name), std::fs::read(&local)) {
                (Some((remote, _)), _) if *remote == digest(bytes) => {}
                (Some(_), Ok(here)) if here.len() == bytes.len() => {
                    self.known.insert(name.clone(), (digest(bytes), digest(&here)));
                }
                _ => {
                    std::fs::create_dir_all(local.parent().unwrap()).unwrap();
                    std::fs::write(&local, bytes).unwrap();
                    self.known.insert(name.clone(), (digest(bytes), digest(bytes)));
                }
            }
        }
    }
}

fn edit_ledger(notes: &Path, change: impl FnOnce(&mut toml_edit::ArrayOfTables)) {
    let path = ledger_path(notes);
    let mut doc: toml_edit::DocumentMut = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    if let Some(lines) = doc.get_mut("line").and_then(toml_edit::Item::as_array_of_tables_mut) {
        change(lines);
    }
    std::fs::write(&path, doc.to_string()).unwrap();
}

/// R10. Desk (Nextcloud: the server's folder) and phone (eDrive: size-only,
/// carried every `lag` minutes), both updated at minute 0, editing money
/// lines and sessions (legacy ones without ids, and new ones) during the
/// switch, then left alone until everything is carried: both must hold the
/// same lines and sessions. `with_words`: words added and taken away too.
fn converge(seed: u64, lag: i64, with_words: bool) -> Result<(), String> {
    converge_with("once", seed, lag, with_words, 0, false)
}

/// `phone_from`: the minute the phone is updated (its entry says format 1 before, as 0.0.3's does); `both`: each device may act each minute.
fn converge_with(run: &str, seed: u64, lag: i64, with_words: bool, phone_from: i64, both_act: bool) -> Result<(), String> {
    let (base, server, key, desk, phone, desk_notes, phone_notes) = two_devices(&format!("sw-{run}-{seed}-{lag}-{with_words}-{phone_from}-{both_act}"));
    let phone_folder = base.join("phone-sync").join("Sioul");
    std::fs::create_dir_all(&phone_folder).unwrap();
    desk.write("config/config.toml", "[words.codes.code]\nadd = [\"Bestätigungscode\", \"Sicherheitscode\"]\n");
    desk.write("data/time/2026-09.toml", "[[session]]\nproject = \"garden\"\nstart = 1789912800\nminutes = 30\ndone = false\n\n[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
    std::fs::write(ledger_path(&desk_notes), format!("{BUDGET}\n[[line]]\nbudget = \"home\"\ndate = 2026-09-03\namount = -42\nlabel = \"Pharmacie\"\n\n[[line]]\nbudget = \"home\"\ndate = 2026-09-05\namount = -12\nlabel = \"Boulangerie\"\n")).unwrap();
    let mut carrier = SizeOnly::default();
    let mut now = NOW;
    for _ in 0..4 {
        desk.exchange_notes(&server, &key, now, &desk_notes);
        carrier.carry(&server, &phone_folder);
        phone.exchange_notes(&phone_folder, &key, now + 30_000, &phone_notes);
        carrier.carry(&server, &phone_folder);
        now += MINUTE;
    }
    let mut rng = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let mut next = || {
        rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (rng >> 33) as usize
    };
    let mut log = Vec::new();
    for minute in 0..90i64 {
      for turn in 0..if both_act { 2 } else { 1 } {
        let (who, notes) = if (if both_act { turn } else { next() % 2 }) == 0 { (&desk, &desk_notes) } else { (&phone, &phone_notes) };
        let name = if std::ptr::eq(who, &desk) { "desk" } else { "phone" };
        match next() % 12 {
            0 => {
                let line = sioul_core::budget::Line { label: format!("L{minute}"), amount: sioul_core::money::Money(-100 * (minute + 1)), ..bread() };
                sioul_core::budget::record_line(&ledger_path(notes), &line, "by hand").unwrap();
                log.push(format!("{minute} {name} add line"));
            }
            1 => {
                let amount = -((next() % 90 + 1) as i64);
                edit_ledger(notes, |lines| {
                    if let Some(line) = lines.get_mut(1) {
                        line["amount"] = toml_edit::value(amount);
                    }
                });
                log.push(format!("{minute} {name} edit line 1 → {amount}"));
            }
            2 => {
                edit_ledger(notes, |lines| {
                    if lines.len() > 3 {
                        lines.remove(lines.len() - 1);
                    }
                });
                log.push(format!("{minute} {name} remove last line"));
            }
            3 => {
                let all = sioul_core::timelog::sessions_in(&who.path("data/time"));
                let minutes = (next() % 90 + 1) as u32;
                if let Some(first) = all.first() {
                    sioul_core::timelog::update_in(&who.path("data/time"), &[first.key()], |s| s.minutes = minutes).unwrap();
                    log.push(format!("{minute} {name} session {} → {minutes}", first.key()));
                }
            }
            4 => {
                sioul_core::timelog::record_in(&who.path("data/time"), &sioul_core::timelog::Session { project: "admin".into(), start: 1_789_920_000 + minute * 3600, minutes: 10, ..Default::default() }).unwrap();
                log.push(format!("{minute} {name} add session"));
            }
            5 if with_words => {
                change_words(who, "add", |list| list.push(format!("W{minute}")));
                log.push(format!("{minute} {name} add word"));
            }
            6 if with_words => {
                change_words(who, "add", |list| {
                    if list.len() > 2 {
                        list.remove(list.len() - 1);
                    }
                });
                log.push(format!("{minute} {name} remove a word"));
            }
            _ => {}
        }
      }
        desk.exchange_notes(&server, &key, now, &desk_notes);
        announce(&server, &key, &desk, 2, now);
        if minute % lag == 0 {
            carrier.carry(&server, &phone_folder);
        }
        phone.exchange_notes(&phone_folder, &key, now + 30_000, &phone_notes);
        announce(&phone_folder, &key, &phone, if minute >= phone_from { 2 } else { 0 }, now + 30_000);
        if minute % lag == 0 {
            carrier.carry(&server, &phone_folder);
        }
        now += MINUTE;
    }
    for _ in 0..12 {
        desk.exchange_notes(&server, &key, now, &desk_notes);
        announce(&server, &key, &desk, 2, now);
        carrier.carry(&server, &phone_folder);
        phone.exchange_notes(&phone_folder, &key, now + 30_000, &phone_notes);
        announce(&phone_folder, &key, &phone, 2, now + 30_000);
        carrier.carry(&server, &phone_folder);
        now += MINUTE;
    }
    let both = (writes(&desk), writes(&phone));
    let (dl, pl) = (lines(&desk_notes), lines(&phone_notes));
    let (ds, ps) = (sessions(&desk), sessions(&phone));
    let (dw, pw) = (words(&desk, "add"), words(&phone, "add"));
    let _ = std::fs::remove_dir_all(&base);
    if both != (2, 2) || dl != pl || ds != ps || dw != pw {
        return Err(format!("seed {seed} lag {lag} words {with_words}: formats {both:?}\n lines desk {dl:?}\n lines phone {pl:?}\n sessions desk {ds:?}\n sessions phone {ps:?}\n words desk {dw:?}\n words phone {pw:?}\n log {log:?}"));
    }
    Ok(())
}

/// Desk on the server's folder, phone behind eDrive (size only, never
/// deleting), both updated at once, one of them editing lines and sessions
/// each minute through the switch: once everything is carried, both hold the
/// same lines and sessions (R10).
#[test]
fn lines_and_sessions_converge_through_the_switch_under_edrive() {
    let failed: Vec<String> = (1..=12u64).flat_map(|seed| [1, 7, 30].map(|lag| converge(seed, lag, false))).filter_map(Result::err).collect();
    assert!(failed.is_empty(), "{} runs parted the devices:\n{}", failed.len(), failed.join("\n\n"));
}

/// The same, words added and taken away too (R10).
#[test]
fn words_converge_through_the_switch_under_edrive() {
    let failed: Vec<String> = (1..=12u64).flat_map(|seed| [1, 7, 30].map(|lag| converge(seed, lag, true))).filter_map(Result::err).collect();
    assert!(failed.is_empty(), "{} runs parted the devices:\n{}", failed.len(), failed.join("\n\n"));
}

/// The same, both devices editing each minute, the phone updated at minute 20
/// or 45 (the mixed window of an upgrade): the same lines and sessions (R10).
#[test]
fn both_devices_editing_each_minute_converge_when_the_phone_is_updated_late() {
    let mut failed = Vec::new();
    for seed in 1..=10u64 {
        for lag in [7, 30] {
            for phone_from in [20, 45] {
                if let Err(e) = converge_with("late", seed, lag, false, phone_from, true) {
                    failed.push(e);
                }
            }
        }
    }
    assert!(failed.is_empty(), "{} runs parted the devices:\n{}", failed.len(), failed.join("\n\n"));
}

/// The same with words (R10, R2, R3).
#[test]
fn words_edited_on_both_each_minute_converge_when_the_phone_is_updated_late() {
    let mut failed = Vec::new();
    for seed in 1..=10u64 {
        for lag in [7, 30] {
            for phone_from in [20, 45] {
                if let Err(e) = converge_with("late-words", seed, lag, true, phone_from, true) {
                    failed.push(e);
                }
            }
        }
    }
    assert!(failed.is_empty(), "{} runs parted the devices:\n{}", failed.len(), failed.join("\n\n"));
}

/// No phase of the switch parts the devices on lines and sessions: the phone
/// updated at once, at minute 20, or at 45 (R10, R11).
#[test]
fn no_phase_of_the_switch_parts_the_devices() {
    let mut report = Vec::new();
    let mut any = false;
    for phone_from in [0, 20, 45] {
        let mut parted = Vec::new();
        for seed in 1..=10u64 {
            for lag in [7, 30] {
                if converge_with("phases", seed, lag, false, phone_from, true).is_err() {
                    parted.push(format!("{seed}/{lag}"));
                }
            }
        }
        any |= !parted.is_empty();
        report.push(format!("phone updated at {phone_from}: {} of 20 parted {parted:?}", parted.len()));
    }
    assert!(!any, "{}", report.join("\n"));
}

// ---------------------------------------------------------------- R11: the mixed window, a session without id

/// The mixed window of an upgrade (the phone writes format 2, the desk still
/// format 1 until its sync carries the phone's news): a session without an
/// id corrected on both stays one session, the same on both devices (R11).
#[test]
fn a_session_corrected_on_both_during_the_mixed_window_stays_one() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r11-mixed");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    // The desk's entry says format 2 (its Sioul reads it); the phone's news has not reached the desk.
    announce(&folder, &key, &desk, 2, now);
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    assert_eq!((writes(&desk), writes(&phone)), (0, 2), "the mixed window");
    let key_of = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of(&phone)], |s| s.minutes = 36).unwrap();
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.minutes = 45).unwrap();
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    // The phone's news reaches the desk; both go on.
    announce(&folder, &key, &phone, 2, now);
    rounds_announced(&folder, &key, &both, now, 4);
    let (on_desk, on_phone) = (sessions(&desk), sessions(&phone));
    assert_eq!(on_desk, on_phone, "the same sessions on both devices");
    assert_eq!(on_desk.len(), 1, "one session (the later correction), as format 1 kept: {on_desk:?}");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R12: an older copy of a round file after the switch

/// After the switch, an older copy of a round file put back by the sync app
/// brings no older version back: format 1's records at or below what format 1
/// last said under their entry here are passed over (R12).
#[test]
fn an_older_copy_of_a_round_file_brings_nothing_back_after_the_switch() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r12-stale");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    let key_of = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of(&phone)], |s| s.minutes = 30).unwrap();
    now = rounds(&folder, &key, &both, now, 2);
    // The phone's file as it is now: what an older copy put back would hold.
    let phone_round = round_file(&folder, &phone.id, 1);
    let older_copy = std::fs::read(&phone_round).unwrap();
    sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.minutes = 40).unwrap();
    now = rounds(&folder, &key, &both, now, 2);
    assert_eq!(sessions(&phone), [("letters".to_string(), 1_789_900_000, 40, String::new())]);
    now = into_format_2(&folder, &key, &both, now);
    // The phone records a little more, then the sync app puts the older copy back.
    sioul_core::timelog::record_in(&phone.path("data/time"), &sioul_core::timelog::Session { project: "admin".into(), start: 1_789_920_000, minutes: 10, ..Default::default() }).unwrap();
    now = rounds_announced(&folder, &key, &both, now, 2);
    std::fs::write(&phone_round, &older_copy).unwrap();
    let cut = desk.exchange_notes(&folder, &key, now, &desk_notes);
    let listing: Vec<String> = std::fs::read_dir(&folder).unwrap().filter_map(Result::ok).map(|e| format!("{} {}", e.file_name().to_string_lossy(), e.metadata().map(|m| m.len()).unwrap_or(0))).collect();
    assert!(cut.problems.iter().any(|p| p.starts_with("share-other-cut")), "{cut:?} phone {} older copy {} bytes; {listing:?}", phone.id, older_copy.len());
    let letters: Vec<_> = sessions(&desk).into_iter().filter(|s| s.0 == "letters").collect();
    assert_eq!(letters, [("letters".to_string(), 1_789_900_000, 40, String::new())], "an older copy read again brings nothing back");
    let _ = std::fs::remove_dir_all(&base);
}

/// Repairing the doses' record after the switch reads every device's records
/// again: no format-1 version of a session comes back (R12).
#[test]
fn repairing_the_doses_record_brings_no_old_version_back() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r12b-rebuild");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    let key_of = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of(&phone)], |s| s.minutes = 30).unwrap();
    now = rounds(&folder, &key, &both, now, 2);
    sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.minutes = 40).unwrap();
    now = rounds(&folder, &key, &both, now, 2);
    now = into_format_2(&folder, &key, &both, now);
    assert_eq!(sessions(&desk), [("letters".to_string(), 1_789_900_000, 40, String::new())]);
    // The doses' record found broken on the desk: the sharing reads it again from every record.
    rebuild(&desk.memory, &desk.id, &["state/health-state.toml"]).unwrap();
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    rounds_announced(&folder, &key, &both, now, 2);
    assert_eq!(sessions(&desk), [("letters".to_string(), 1_789_900_000, 40, String::new())], "the desk");
    assert_eq!(sessions(&phone), [("letters".to_string(), 1_789_900_000, 40, String::new())], "the phone");
    let _ = std::fs::remove_dir_all(&base);
}

/// Switching notes on after the switch (a store joining) reads every device's
/// records again: no older version of a session comes back (R12).
#[test]
fn switching_notes_on_brings_no_old_version_back() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r12c-notes-on");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    let key_of = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of(&phone)], |s| s.minutes = 30).unwrap();
    now = rounds(&folder, &key, &both, now, 2);
    sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.minutes = 40).unwrap();
    now = rounds(&folder, &key, &both, now, 2);
    now = into_format_2(&folder, &key, &both, now);
    // Notes switched on on the desk.
    std::fs::write(desk_notes.join("lease.md"), "A note.\n").unwrap();
    let config = Config { share_projects: true, notes_root: Some(desk_notes.display().to_string()), ..Config::default() };
    let stores = stores_of(&config, &desk.roots, &|part| part == "notes" || shared_by_default(part, &config));
    exchange(&Sharing { folder: &folder, computer: &desk.id, key: &key, memory: &desk.memory, files: true, hurry: None }, &stores, now).unwrap();
    assert_eq!(sessions(&desk), [("letters".to_string(), 1_789_900_000, 40, String::new())], "the desk, notes switched on");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R13: a session corrected just before the switching exchange

/// A session corrected just before the exchange that switches the device
/// reaches the other device once: that exchange sends it in format 1 first
/// (R13).
#[test]
fn a_session_corrected_just_before_the_switch_reaches_the_other_device_once() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r13-pending-session");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    announce(&folder, &key, &phone, 2, now);
    let key_of = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.minutes = 45).unwrap();
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    announce(&folder, &key, &desk, 2, now);
    now += MINUTE;
    assert_eq!(writes(&desk), 2);
    rounds_announced(&folder, &key, &both, now, 4);
    assert_eq!(sessions(&desk), [("letters".to_string(), 1_789_900_000, 45, String::new())]);
    assert_eq!(sessions(&phone), [("letters".to_string(), 1_789_900_000, 45, String::new())], "the phone: the corrected session, once");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- R14: an earlier version put back

/// Putting back an earlier version of a month (Settings ▸ Earlier versions)
/// puts the session back as it was, one session, in both formats (R14).
#[test]
fn putting_back_an_earlier_month_keeps_one_session() {
    for format in [1, 2] {
        let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices(&format!("sw-r14-put-back-{format}"));
        let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
        desk.write("data/time/2026-09.toml", "[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
        let mut now = rounds(&folder, &key, &both, NOW, 2);
        if format == 2 {
            now = into_format_2(&folder, &key, &both, now);
        }
        let key_of = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time"))[0].key();
        sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.minutes = 45).unwrap();
        now = rounds(&folder, &key, &both, now, 2);
        assert_eq!(sessions(&phone), [("letters".to_string(), 1_789_900_000, 45, String::new())]);
        let history = crate::history::root(&phone.memory);
        let kept = crate::history::versions(&history, "time", "data/time/2026-09.toml");
        assert!(!kept.is_empty(), "format {format}: a version kept");
        let config = Config { share_projects: true, notes_root: Some(phone_notes.display().to_string()), ..Config::default() };
        let every = stores(&config, &phone.roots);
        put_back(&phone.memory, Some((&folder, &key)), &every, "time", "data/time/2026-09.toml", &kept[0].stamp, now).unwrap();
        assert_eq!(sessions(&phone), [("letters".to_string(), 1_789_900_000, 20, String::new())], "format {format}: the session as it was, once");
        let _ = std::fs::remove_dir_all(&base);
    }
}


// ---------------------------------------------------------------- the gate: who counts

/// A device whose only companion stopped sharing (its sealed entry says it
/// left) is alone, and stays on format 1: a device joining later may run an
/// older Sioul (review L1).
#[test]
fn a_device_whose_only_companion_left_stays_on_format_1() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-l1-left");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let now = rounds(&folder, &key, &both, NOW, 2);
    let entry = crate::devices::Entry { id: phone.id.clone(), version: "0.0.5".into(), format: 2, working: false, left: true, started: now / 1000, wrote: written(&phone.memory, &phone.id), ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &entry).unwrap();
    announce(&folder, &key, &desk, 2, now);
    desk.exchange_notes(&folder, &key, now + MINUTE, &desk_notes);
    desk.exchange_notes(&folder, &key, now + 2 * MINUTE, &desk_notes);
    assert!(writes(&desk) < 2, "alone once its companion left");
    let _ = std::fs::remove_dir_all(&base);
}

/// Forget this device (a retired phone on 0.0.3, never stopped): it no
/// longer holds format 2 back, and the others take it; it counts again as
/// soon as it says anything newer, a record read here (review L2).
#[test]
fn forgetting_a_device_releases_the_switch_until_it_says_anything_newer() {
    let (base, folder, key, desk, laptop, desk_notes, laptop_notes) = two_devices("sw-l2-forget");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    let both = [(&desk, desk_notes.as_path()), (&laptop, laptop_notes.as_path())];
    let mut now = rounds(&folder, &key, &both, NOW, 1);
    // A phone on 0.0.3: a record and an entry saying no format.
    let old = uuid::Uuid::new_v4().to_string();
    let said = |n: u64, at: i64, address: &str| super::tests::record_of(&key, &old, n, ((at as u64) << 16) | n, &Change { k: format!("config/safe-senders.txt#{address}"), v: Some(String::new()), b: String::new(), build: if n == 1 { "0.0.3 (f054336abcde)".into() } else { String::new() }, f: 0 });
    super::tests::append_to(&round_file(&folder, &old, 1), &said(1, now, "b@example.org"));
    let entry = crate::devices::Entry { id: old.clone(), version: "0.0.3".into(), working: true, started: now / 1000, wrote: Some((1, 1)), ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &entry).unwrap();
    for _ in 0..3 {
        for (computer, notes) in &both {
            computer.exchange_notes(&folder, &key, now, notes);
            announce(&folder, &key, computer, 2, now);
            now += MINUTE;
        }
    }
    assert!(writes(&desk) < 2, "the old phone holds format 2 back");
    assert!(formats(&folder, &key, &desk.memory, &desk.id, now).holders.iter().any(|(id, holds)| *id == old && *holds == Holds::Older));
    // Forgotten on the desk: it holds nothing back there; the desk takes format 2.
    forget_device(&folder, &key, &desk.memory, &desk.id, &old).unwrap();
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    assert_eq!(writes(&desk), 2, "forgotten, it holds nothing back");
    // It says something newer: it counts again.
    super::tests::append_to(&round_file(&folder, &old, 1), &said(2, now + MINUTE, "c@example.org"));
    desk.exchange_notes(&folder, &key, now + 2 * MINUTE, &desk_notes);
    let holding = format_holders(&folder, &key, &desk.id, &Memory::load(&desk.memory, &desk.id), now + 2 * MINUTE);
    assert!(holding.iter().any(|(id, _)| *id == old), "heard of again, it counts again: {holding:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// A device that still counts, on an older Sioul, is said once the others
/// write format 2 (`share-format-older`); one not heard of for 180 days (from
/// what is sealed) is not (review L4).
#[test]
fn an_older_device_is_said_only_while_it_counts() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-l4-older");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let now = into_format_2(&folder, &key, &both, rounds(&folder, &key, &both, NOW, 1));
    let old = uuid::Uuid::new_v4().to_string();
    let entry = |started: i64| crate::devices::Entry { id: old.clone(), version: "0.0.3".into(), working: true, started, ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &entry(now / 1000)).unwrap();
    assert_eq!(formats(&folder, &key, &desk.memory, &desk.id, now).older, [old.clone()]);
    crate::devices::publish(&folder, &key, &entry((now - 200 * DAY) / 1000)).unwrap();
    assert!(formats(&folder, &key, &desk.memory, &desk.id, now).older.is_empty(), "silent 200 days: no longer said");
    let _ = std::fs::remove_dir_all(&base);
}

/// A configuration that does not read (being written, broken by hand) at the
/// exchange that would switch keeps the device on format 1, said
/// (`share-format-wait`), nothing renamed; read again, the switch happens
/// then (review L3).
#[test]
fn a_file_that_does_not_read_keeps_format_1_and_says_so() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-l3-unreadable");
    desk.write("config/config.toml", "[words.codes.code]\nadd = [\"Bestätigungscode\"]\n");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let now = rounds(&folder, &key, &both, NOW, 2);
    announce(&folder, &key, &phone, 2, now);
    let good = desk.read("config/config.toml");
    desk.write("config/config.toml", "[words.codes.code\nadd = [\"Bestätigungscode\"\n");
    let waited = desk.exchange_notes(&folder, &key, now + MINUTE, &desk_notes);
    assert!(writes(&desk) < 2, "{waited:?}");
    assert!(waited.problems.iter().any(|p| p == "share-format-wait:config/config.toml"), "{waited:?}");
    desk.write("config/config.toml", &good);
    desk.exchange_notes(&folder, &key, now + 2 * MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 2);
    let _ = std::fs::remove_dir_all(&base);
}

/// A part held for a newer format that a test build wrote once: once that
/// device no longer counts (it left), the part is released, what waited of
/// it in that format dropped, what changed here goes out, and that build's
/// records read again do not hold it again (review R8, a part never held
/// for good).
#[test]
fn a_part_held_for_a_device_gone_is_released() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw-r8-released");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\nid = \"aa\"\nproject = \"garden\"\nstart = 1789912800\nminutes = 30\n");
    let mut now = into_format_2(&folder, &key, &both, rounds(&folder, &key, &both, NOW, 2));
    let test_build = uuid::Uuid::new_v4().to_string();
    let change = Change { k: format!("data/time/2026-09.toml#session{SEP}{MARK}aa"), v: Some("[v]\nid = \"aa\"\nproject = \"garden\"\nstart = 1789912800\nminutes = 99\n".into()), b: String::new(), build: "0.1.0-dev (abcdef123456)".into(), f: 3 };
    super::tests::append_to(&round_file(&folder, &test_build, 1), &super::tests::record_of(&key, &test_build, 1, ((now as u64) << 16) | 1, &change));
    let entry = crate::devices::Entry { id: test_build.clone(), version: "0.1.0".into(), format: 3, working: true, started: now / 1000, ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &entry).unwrap();
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    assert!(formats(&folder, &key, &phone.memory, &phone.id, now).newer.iter().any(|(part, _, _)| part == "time"));
    let key_of = sioul_core::timelog::sessions_in(&phone.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of], |s| s.minutes = 45).unwrap();
    assert_eq!(phone.exchange_notes(&folder, &key, now, &phone_notes).sent, 0, "held");
    now += MINUTE;
    // That device stops sharing: the part is released.
    crate::devices::publish(&folder, &key, &crate::devices::Entry { left: true, working: false, ..entry }).unwrap();
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    assert!(formats(&folder, &key, &phone.memory, &phone.id, now).newer.is_empty(), "released");
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    let minutes = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time")).iter().map(|s| s.minutes).collect::<Vec<_>>();
    assert_eq!(minutes(&desk), [45], "what changed on the phone meanwhile went out");
    // Read again (the doses' record repaired: every record read again), that build's record holds nothing.
    rebuild(&phone.memory, &phone.id, &["state/health-state.toml"]).unwrap();
    let again = phone.exchange_notes(&folder, &key, now, &phone_notes);
    assert!(!again.problems.iter().any(|p| p.starts_with("share-newer")), "{again:?}");
    assert!(formats(&folder, &key, &phone.memory, &phone.id, now).newer.is_empty());
    let _ = std::fs::remove_dir_all(&base);
}
