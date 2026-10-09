// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The switch to format 2, the third review's tests (9 October 2026, T1 to
//! T9): the window's editors against the sharing, a device read long ago,
//! a whole list's floor against restated elements, the merge of a state
//! emptied, and more simulated upgrades. Each was first written to prove a
//! finding, and now holds what the fix gives. Fiction only.

use super::format_tests::{announce, into_format_2, writes};
use super::switch_more_tests::{change_words, converge, converge3, list_of, rounds, rounds_announced, sessions, sites, two_devices};
use super::tests::{Computer, MINUTE, NOW, scratch};
use super::*;

/// The window records sessions into a month (`timelog::record_in`, under
/// the month's lock) on one thread while the sharing exchanges on another,
/// writing the phone's sessions into the same month: nothing lost, nothing
/// stuck (the re-entrant lock, two threads; third review, T3).
#[test]
fn the_window_and_the_sharing_writing_one_month_at_once_lose_nothing() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw3-t3-threads");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\nproject = \"garden\"\nstart = 1789912800\nminutes = 30\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    let started = std::time::Instant::now();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for i in 0..60i64 {
                sioul_core::timelog::record_in(&desk.path("data/time"), &sioul_core::timelog::Session { project: "desk".into(), start: 1_789_920_000 + i * 60, minutes: 1, ..Default::default() }).unwrap();
                std::thread::yield_now();
            }
        });
        scope.spawn(|| {
            for i in 0..60i64 {
                sioul_core::timelog::record_in(&phone.path("data/time"), &sioul_core::timelog::Session { project: "phone".into(), start: 1_789_930_000 + i * 60, minutes: 1, ..Default::default() }).unwrap();
                phone.exchange_notes(&folder, &key, now + i * 1000, &phone_notes);
                desk.exchange_notes(&folder, &key, now + i * 1000 + 500, &desk_notes);
            }
        });
    });
    assert!(started.elapsed().as_secs() < 120, "nothing stuck");
    rounds(&folder, &key, &both, now + 120_000, 3);
    let count = |c: &Computer, what: &str| sessions(c).iter().filter(|s| s.0 == what).count();
    assert_eq!((count(&desk, "desk"), count(&desk, "phone"), count(&phone, "desk"), count(&phone, "phone")), (60, 60, 60, 60));
    let _ = std::fs::remove_dir_all(&base);
}

/// A device read before `heard_at` was kept, whose files were since taken
/// out of the folder: when it last wrote is read from its changes this
/// memory still holds (each the clock of a record read here), and 180 days
/// after it, it holds nothing back (third review, T4).
#[test]
fn a_device_read_long_ago_whose_files_are_gone_stops_counting() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw3-t4-read-gone");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let gone = uuid::Uuid::new_v4().to_string();
    let change = Change { k: "config/safe-senders.txt#b@example.org".into(), v: Some(String::new()), b: String::new(), build: String::new(), f: 0 };
    super::tests::append_to(&round_file(&folder, &gone, 1), &super::tests::record_of(&key, &gone, 1, ((NOW - 300 * DAY) as u64) << 16, &change));
    let entry = crate::devices::Entry { id: gone.clone(), version: "0.0.2".into(), working: false, left: true, started: (NOW - 300 * DAY) / 1000, ..crate::devices::Entry::default() };
    let now = rounds(&folder, &key, &both, NOW, 2);
    // Its files go from the folder; the memories as an older Sioul kept them (no `heard_at`, no `seen`).
    std::fs::remove_file(round_file(&folder, &gone, 1)).unwrap();
    let _ = entry;
    for computer in [&desk, &phone] {
        let mut memory: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&computer.memory).unwrap()).unwrap();
        memory.as_object_mut().unwrap().remove("heard_at");
        memory.as_object_mut().unwrap().remove("seen");
        std::fs::write(&computer.memory, serde_json::to_string(&memory).unwrap()).unwrap();
    }
    rounds_announced(&folder, &key, &both, now + 400 * DAY, 4);
    let holders = formats(&folder, &key, &desk.memory, &desk.id, now + 400 * DAY + 10 * MINUTE).holders;
    assert_eq!(writes(&desk), 2, "a device of which nothing has been heard for 700 days holds format 2 back: {holders:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// More seeds than `many_seeds_converge_through_the_switch`, two devices and
/// three: every run converges (third review, T5; seed 77 found T7).
#[test]
fn more_seeds_converge_with_two_and_three_devices() {
    let mut failed = Vec::new();
    let mut runs = 0;
    for seed in 41..=100u64 {
        for lag in [7, 30] {
            for phone_from in [0, 20, 45] {
                runs += 1;
                if let Err(e) = converge(seed, lag, phone_from) {
                    failed.push(e);
                }
            }
        }
    }
    for seed in 26..=60u64 {
        for lag in [7, 30] {
            for (phone_from, laptop_at) in [(20, 10), (45, 30), (0, 1)] {
                runs += 1;
                if let Err(e) = converge3(seed, lag, phone_from, Some(laptop_at)) {
                    failed.push(e);
                }
            }
        }
    }
    let summary: Vec<String> = failed.iter().map(|e| e.lines().next().unwrap_or_default().to_string()).collect();
    assert!(failed.is_empty(), "{} of {runs} runs fail:\n{}\n\nfirst in full:\n{}", failed.len(), summary.join("\n"), failed.first().cloned().unwrap_or_default());
}

/// A site pinned on the phone in the exchange that switches it, unpinned on
/// the desk in format 1: unpinned on both once both switch (third review, T6).
#[test]
fn a_site_unpinned_before_the_switch_stays_unpinned() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw3-t6-site");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("config/config.toml", "[words.codes.code]\nadd = [\"Bestätigungscode\"]\n\n[[site]]\nid = \"poste\"\nurl = \"https://poste.example.org\"\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    announce(&folder, &key, &desk, 2, now);
    let _ = sioul_core::config::add_site(&phone.path("config/config.toml"), "s0", "s0", "https://s0.example.org");
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    announce(&folder, &key, &phone, 0, now);
    now += MINUTE;
    let phone_switched = writes(&phone);
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    announce(&folder, &key, &desk, 2, now);
    now += MINUTE;
    let desk_got = sites(&desk);
    change_words(&phone, "add", |list| list.push("Einmalcode".into()));
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    announce(&folder, &key, &phone, 0, now);
    now += MINUTE;
    let _ = sioul_core::config::remove_site(&desk.path("config/config.toml"), "s0");
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    let after_unpin = (writes(&desk), sites(&desk));
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    let after_switch = (writes(&desk), sites(&desk));
    rounds_announced(&folder, &key, &both, now, 3);
    assert_eq!((sites(&desk), sites(&phone)), (vec!["poste".to_string()], vec!["poste".to_string()]), "phone switched: {phone_switched}; desk got {desk_got:?}; after its unpin {after_unpin:?}; after its switch {after_switch:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// The phone switches while a site it pinned goes out; the desk, still on
/// format 1, gets it, then unpins it (the whole list without it). The
/// phone's round is cut (`share-own-cut`) and it restates what it has the
/// last word on, in format 2, at their old clocks. The desk switches and
/// passes the restated site over (`below_whole_floor`): its whole list took
/// it out later. Unpinned on both (third review, T7).
#[test]
fn a_restated_site_stays_unpinned_on_a_device_that_unpinned_it_before_its_switch() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw3-t7-restated");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("config/config.toml", "[[site]]\nid = \"poste\"\nurl = \"https://poste.example.org\"\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    announce(&folder, &key, &desk, 2, now);
    let _ = sioul_core::config::add_site(&phone.path("config/config.toml"), "s0", "s0", "https://s0.example.org");
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    assert_eq!(writes(&phone), 2);
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    assert_eq!(sites(&desk), ["poste", "s0"]);
    // The phone's round cut (an older copy put back): it opens a new round, restating what it has the last word on.
    let phone_round = round_file(&folder, &phone.id, 1);
    let text = std::fs::read_to_string(&phone_round).unwrap();
    let cut: String = text.lines().take(text.lines().count().saturating_sub(1)).map(|l| format!("{l}\n")).collect();
    std::fs::write(&phone_round, cut).unwrap();
    let restated = phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    // The desk unpins it, then reads the phone's new round in the same exchange.
    let _ = sioul_core::config::remove_site(&desk.path("config/config.toml"), "s0");
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    let desk_after_switch = (writes(&desk), sites(&desk));
    announce(&folder, &key, &phone, 2, now);
    rounds_announced(&folder, &key, &both, now, 3);
    assert_eq!((sites(&desk), sites(&phone)), (vec!["poste".to_string()], vec!["poste".to_string()]), "unpinned on both (phone restated: {:?}; desk after its switch {desk_after_switch:?})", restated.problems);
    let _ = std::fs::remove_dir_all(&base);
}

/// The window takes its one lighter day away (the list emptied, a key the
/// state skips) while the sharing has added another device's: that day
/// stays (`filelock::save_merged`, third review, T9).
#[test]
fn emptying_the_lighter_days_keeps_one_another_device_added_meanwhile() {
    let base = scratch("sw3-t9-merge");
    let path = base.join("quiet.toml");
    std::fs::write(&path, "lighter = [2026-10-12]\n").unwrap();
    let mut overrides = sioul_core::quiet::Overrides::load(&path);
    // The sharing writes another device's lighter day meanwhile.
    std::fs::write(&path, "lighter = [2026-10-12, 2026-10-14]\n").unwrap();
    // The window takes its one lighter day away.
    overrides.lighter.clear();
    overrides.save(&path).unwrap();
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(now.contains("2026-10-14"), "the other device's lighter day stays: {now:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// The Words tab shows a list; meanwhile the phone adds a word, which the
/// sharing writes into the desk's configuration. The desk then adds its own
/// word from the tab still open: the tab's change (one word added, from what
/// it showed) is set over the list as it is now (`settings::change`), so both
/// words stay on both devices (third review, T1).
#[test]
fn the_words_tab_keeps_a_word_that_arrived_while_it_was_open() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw3-t1-words-tab");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("config/config.toml", "[words.codes.code]\nadd = [\"Bestätigungscode\"]\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    let path_of = |c: &Computer| c.path("config/config.toml");
    let shown = |c: &Computer| sioul_core::words::compared(&sioul_core::config::Config::load(&path_of(c)).unwrap()).yours.list("codes.code").to_vec();
    // The desk's tab opens.
    let open_on_desk = shown(&desk);
    // The phone adds a word from its own tab, and it reaches the desk.
    let open_on_phone = shown(&phone);
    let mut phone_list = open_on_phone.clone();
    phone_list.push("Einmalcode".into());
    sioul_core::settings::change(&path_of(&phone), "words.codes.code", Some(&sioul_core::config::SettingValue::Texts(open_on_phone)), &sioul_core::config::SettingValue::Texts(phone_list)).unwrap();
    now = rounds_announced(&folder, &key, &both, now, 2);
    let arrived = shown(&desk).contains(&"Einmalcode".to_string());
    // The desk adds its own word from the tab still open.
    let mut desk_list = open_on_desk.clone();
    desk_list.push("Sicherheitscode".into());
    // As the tab sends it: what it showed, and the list with its word added (`settings::change`).
    sioul_core::settings::change(&path_of(&desk), "words.codes.code", Some(&sioul_core::config::SettingValue::Texts(open_on_desk)), &sioul_core::config::SettingValue::Texts(desk_list)).unwrap();
    rounds_announced(&folder, &key, &both, now, 3);
    let (on_desk, on_phone) = (shown(&desk), shown(&phone));
    let has = |list: &[String], w: &str| list.iter().any(|x| x == w);
    assert!(arrived && has(&on_desk, "Einmalcode") && has(&on_phone, "Einmalcode") && has(&on_desk, "Sicherheitscode") && has(&on_phone, "Sicherheitscode"), "both words stay (arrived on the desk before its save: {arrived}); desk {on_desk:?}; phone {on_phone:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// Days off, each given an id at the switch. The editor, open on the desk,
/// adds Easter while Christmas arrived from the phone: its change is set over
/// the list as it is now (`settings::change`), each day already there keeps
/// its table and its id, a new one takes an id of its own: three days off on
/// both, each with its id (third review, T2).
#[test]
fn the_days_off_editor_keeps_ids_and_a_day_that_arrived() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw3-t2-time-off");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("config/config.toml", "[[time_off]]\nfrom = 2026-08-01\nuntil = 2026-08-15\nlabel = \"Summer\"\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    now = rounds_announced(&folder, &key, &both, now, 1);
    let path_of = |c: &Computer| c.path("config/config.toml");
    let days = |c: &Computer| list_of(&c.read("config/config.toml"), "time_off").iter().map(|t| (t.get("label").and_then(toml::Value::as_str).unwrap_or_default().to_string(), t.get("id").is_some())).collect::<Vec<_>>();
    let shown = |c: &Computer| sioul_core::config::Config::load(&path_of(c)).unwrap().time_off.iter().map(|t| sioul_core::config::TimeOffValue { from: t.from.to_string(), until: t.until.to_string(), label: t.label.clone() }).collect::<Vec<_>>();
    let with_ids_after_switch = days(&desk);
    // The desk's editor opens.
    let open_on_desk = shown(&desk);
    // The phone adds Christmas; it reaches the desk.
    let open_on_phone = shown(&phone);
    let mut phone_days = open_on_phone.clone();
    phone_days.push(sioul_core::config::TimeOffValue { from: "2026-12-24".into(), until: "2026-12-26".into(), label: "Christmas".into() });
    sioul_core::settings::change(&path_of(&phone), "time_off", Some(&sioul_core::config::SettingValue::TimeOff(open_on_phone)), &sioul_core::config::SettingValue::TimeOff(phone_days)).unwrap();
    now = rounds_announced(&folder, &key, &both, now, 2);
    let arrived = days(&desk);
    // The desk adds Easter from the editor still open.
    let mut desk_days = open_on_desk.clone();
    desk_days.push(sioul_core::config::TimeOffValue { from: "2027-03-26".into(), until: "2027-03-29".into(), label: "Easter".into() });
    // As the editor sends it: what it showed, and the list with its day added (`settings::change`).
    sioul_core::settings::change(&path_of(&desk), "time_off", Some(&sioul_core::config::SettingValue::TimeOff(open_on_desk)), &sioul_core::config::SettingValue::TimeOff(desk_days)).unwrap();
    rounds_announced(&folder, &key, &both, now, 3);
    let (on_desk, on_phone) = (days(&desk), days(&phone));
    let labels = |d: &[(String, bool)]| { let mut l: Vec<String> = d.iter().map(|(l, _)| l.clone()).collect(); l.sort(); l };
    assert!(labels(&on_desk) == ["Christmas", "Easter", "Summer"] && labels(&on_phone) == labels(&on_desk) && on_desk.iter().all(|(_, id)| *id), "three days off on both, ids kept (after the switch {with_ids_after_switch:?}; desk before its save {arrived:?}); desk {on_desk:?}; phone {on_phone:?}");
    let _ = std::fs::remove_dir_all(&base);
}
