// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The switch to format 2, the second review's tests (9 October 2026, S1 to
//! S9): who counts, a newer format met, preset places, sessions corrected in
//! the mixed window, a later removal, the switch stopped half way, and many
//! simulated upgrades under eDrive, with two and three devices. Each was first
//! written to prove a finding, and now holds what the fix gives. Fiction only.

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

fn change_words(computer: &Computer, which: &str, change: impl FnOnce(&mut Vec<String>)) {
    let mut doc: toml_edit::DocumentMut = computer.read("config/config.toml").parse().unwrap();
    let mut list = words(computer, which);
    change(&mut list);
    doc["words"]["codes"]["code"][which] = toml_edit::value(list.iter().map(String::as_str).collect::<toml_edit::Array>());
    computer.write("config/config.toml", &doc.to_string());
}

fn sites(computer: &Computer) -> Vec<String> {
    let mut out: Vec<String> = list_of(&computer.read("config/config.toml"), "site").iter().filter_map(|s| s.get("id").and_then(toml::Value::as_str).map(str::to_string)).collect();
    out.sort();
    out
}

fn bread() -> sioul_core::budget::Line {
    sioul_core::budget::Line { budget: "home".into(), date: "2026-09-05".parse().unwrap(), amount: sioul_core::money::Money(-1200), label: "Boulangerie".into(), planned: false, reserve: None, links: Vec::new(), preset: None }
}

// ---------------------------------------------------------------- S1: a device known by its notes alone

/// A device known by its plain notes alone in the folder (no sealed entry,
/// no record: a stray file, or what a sync app left) is no device: it holds
/// format 2 back neither now nor later (second review, S1).
#[test]
fn a_device_known_by_its_notes_alone_holds_nothing_back() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw2-s1-phantom");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let gone = uuid::Uuid::new_v4().to_string();
    std::fs::write(folder.join(format!("{gone}.toml")), format!("at = {}\nround = 1\n", (NOW - 400 * DAY) / 1000)).unwrap();
    let now = rounds_announced(&folder, &key, &both, NOW, 4);
    let held = formats(&folder, &key, &desk.memory, &desk.id, now).holders;
    let far = now + 400 * DAY;
    rounds_announced(&folder, &key, &both, far, 3);
    let still = formats(&folder, &key, &desk.memory, &desk.id, far + 10 * MINUTE).holders;
    assert_eq!(writes(&desk), 2, "a device of which nothing is known for 800 days holds format 2 back: {held:?} then {still:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// A device whose records were read before this version kept when it last
/// wrote (`heard_at`), and that has no entry: when it last wrote is read
/// from its last record (sealed), and 180 days after it, it holds nothing
/// back (S1).
#[test]
fn a_device_read_before_the_update_is_silent_after_180_days() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw2-s1b-old-records");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let gone = uuid::Uuid::new_v4().to_string();
    let change = Change { k: "config/safe-senders.txt#b@example.org".into(), v: Some(String::new()), b: String::new(), build: String::new(), f: 0 };
    super::tests::append_to(&round_file(&folder, &gone, 1), &super::tests::record_of(&key, &gone, 1, ((NOW - 300 * DAY) as u64) << 16, &change));
    std::fs::write(folder.join(format!("{gone}.toml")), format!("at = {}\nround = 1\n", (NOW - 300 * DAY) / 1000)).unwrap();
    let now = rounds(&folder, &key, &both, NOW, 2);
    // Both memories as an older Sioul left them: the records read, nothing of `heard_at`.
    for computer in [&desk, &phone] {
        let mut memory: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&computer.memory).unwrap()).unwrap();
        memory.as_object_mut().unwrap().remove("heard_at");
        std::fs::write(&computer.memory, serde_json::to_string(&memory).unwrap()).unwrap();
    }
    rounds_announced(&folder, &key, &both, now + 400 * DAY, 4);
    let holders = formats(&folder, &key, &desk.memory, &desk.id, now + 400 * DAY + 10 * MINUTE).holders;
    assert_eq!(writes(&desk), 2, "a device last heard 700 days ago (its last record) holds format 2 back: {holders:?}");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- S2: a newer device's records dropped

/// A newer device's records come before its new entry (its old one still
/// says format 2): the part stays held and its changes wait, since that
/// device still counts; only a device that no longer counts (gone,
/// forgotten, silent 180 days) releases a part (S2).
#[test]
fn a_newer_device_s_records_ahead_of_its_entry_stay_held() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw2-s2-release");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\nid = \"aa\"\nproject = \"garden\"\nstart = 1789912800\nminutes = 30\n");
    let mut now = into_format_2(&folder, &key, &both, rounds(&folder, &key, &both, NOW, 2));
    let newer = uuid::Uuid::new_v4().to_string();
    // Its entry as it was before it was updated (format 2).
    let entry = crate::devices::Entry { id: newer.clone(), version: "0.0.5".into(), format: 2, working: true, started: now / 1000, imported: now / 1000, ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &entry).unwrap();
    // Its first records in format 3, come ahead of its new entry.
    let change = Change { k: format!("data/time/2026-09.toml#session{SEP}{MARK}aa"), v: Some("[v]\nid = \"aa\"\nproject = \"garden\"\nstart = 1789912800\nminutes = 77\n".into()), b: String::new(), build: "0.1.0 (abcdef123456)".into(), f: 3 };
    super::tests::append_to(&round_file(&folder, &newer, 1), &super::tests::record_of(&key, &newer, 1, ((now as u64) << 16) | 1, &change));
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    let met = formats(&folder, &key, &phone.memory, &phone.id, now).newer;
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    // Its new entry arrives.
    crate::devices::publish(&folder, &key, &crate::devices::Entry { format: 3, version: "0.1.0".into(), started: now / 1000, ..entry }).unwrap();
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    let state = Memory::load(&phone.memory, &phone.id);
    let held = formats(&folder, &key, &phone.memory, &phone.id, now).newer;
    assert!(!held.is_empty() && !state.waiting.is_empty(), "a device that writes format 3 and still counts: the part stays held and its change waits (met {met:?}, held now {held:?}, waiting {:?}, released {:?})", state.waiting.keys().collect::<Vec<_>>(), state.released);
    // As the memory on the disk says it, read by a process started afresh (the doses' rule reads it so).
    assert!(holds(&phone.memory, "time") && !holds(&phone.memory, "health"));
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- S3: `again` takes out another session

/// Two sessions at one start, task and project: a format-1 change of the
/// one with its own id changes it alone; nothing is sent again for the
/// other, which stays on every device (S3).
#[test]
fn a_format_1_change_of_one_session_leaves_its_twin() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw2-s3-again");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\nproject = \"garden\"\nstart = 1789912800\nminutes = 45\ndone = false\nnote = \"Pruning\"\n\n[[session]]\nproject = \"garden\"\nstart = 1789912800\nminutes = 20\ndone = false\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    now = rounds_announced(&folder, &key, &both, now, 2);
    assert_eq!(sessions(&desk).len(), 2, "{:?}", sessions(&desk));
    assert_eq!(sessions(&phone).len(), 2, "{:?}", sessions(&phone));
    // The second session (random id), changed by a device still writing format 1.
    let all = sioul_core::timelog::sessions_in(&desk.path("data/time"));
    let twin = all.iter().find(|s| s.id != derived("session", "1789912800\u{1d}\u{1d}garden")).unwrap().clone();
    let mut element = toml::Table::new();
    element.insert("id".into(), twin.id.clone().into());
    element.insert("project".into(), "garden".into());
    element.insert("start".into(), toml::Value::Integer(1_789_912_800));
    element.insert("minutes".into(), toml::Value::Integer(i64::from(twin.minutes) + 1));
    element.insert("done".into(), toml::Value::Boolean(false));
    if !twin.note.is_empty() {
        element.insert("note".into(), twin.note.clone().into());
    }
    let other = uuid::Uuid::new_v4().to_string();
    let change = Change { k: format!("data/time/2026-09.toml#session{SEP}{MARK}1789912800\u{1d}\u{1d}garden"), v: Some(leaf_text(&toml::Value::Table(element))), b: String::new(), build: "0.0.5 (abcdef123456)".into(), f: 0 };
    super::tests::append_to(&round_file(&folder, &other, 1), &super::tests::record_of(&key, &other, 1, ((now as u64) << 16) | 1, &change));
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    // The phone exchanges right after: what it holds between the desk's exchanges.
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    let on_phone = sessions(&phone);
    now += MINUTE;
    rounds(&folder, &key, &both, now, 2);
    assert_eq!((on_phone.len(), sessions(&desk).len(), sessions(&phone).len()), (2, 2, 2), "both garden sessions stay everywhere: phone after the desk's exchange {on_phone:?}; desk {:?}; phone {:?}", sessions(&desk), sessions(&phone));
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- S4: presets' places

/// A split naming a preset without an id by its place (`#1`): the switch
/// names it by the preset's new id where that place named one, and sends
/// the split again, so that one version wins on every device (S4).
#[test]
fn a_split_naming_a_preset_by_place_ends_the_same_on_each_device() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw2-s4-presets");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    let ledger = format!("{BUDGET}\n[[preset]]\nbudget = \"home\"\nlabel = \"Rent\"\namount = -600\nevery = \"month\"\nday = 1\n\n[[preset]]\nbudget = \"home\"\nlabel = \"Insurance\"\namount = -150\nevery = \"year\"\nmonth = 3\nday = 15\n\n[[split]]\naccount = \"checking\"\nwords = [\"assurance\"]\npreset = \"#1\"\n");
    std::fs::write(ledger_path(&desk_notes), ledger).unwrap();
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    rounds_announced(&folder, &key, &both, now, 3);
    let split_preset = |notes: &Path| list_of(&std::fs::read_to_string(ledger_path(notes)).unwrap(), "split").iter().map(|s| s.get("preset").and_then(toml::Value::as_str).unwrap_or_default().to_string()).collect::<Vec<_>>();
    let presets = |notes: &Path| list_of(&std::fs::read_to_string(ledger_path(notes)).unwrap(), "preset").iter().map(|p| format!("{} {}", p.get("label").and_then(toml::Value::as_str).unwrap_or_default(), p.get("id").and_then(toml::Value::as_str).unwrap_or_default())).collect::<Vec<_>>();
    assert_eq!(split_preset(&desk_notes), split_preset(&phone_notes), "the split names one preset on both: desk presets {:?}, phone presets {:?}", presets(&desk_notes), presets(&phone_notes));
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- S5: simulations, more seeds

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
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let mut doc: toml_edit::DocumentMut = text.parse().unwrap();
    if let Some(lines) = doc.get_mut("line").and_then(toml_edit::Item::as_array_of_tables_mut) {
        change(lines);
    }
    std::fs::write(&path, doc.to_string()).unwrap();
}

/// Desk on the server's folder, phone behind a size-only, never-deleting
/// carrier; both devices act each minute (lines, sessions, words, sites);
/// the phone's entry says format 2 from `phone_from`. Then left alone and
/// fully carried. Must hold: both write format 2; the same lines, sessions,
/// words and sites; exactly one "letters" and one "garden" session (never
/// added nor removed, only corrected); as many "admin" sessions as were
/// recorded.
fn converge(seed: u64, lag: i64, phone_from: i64) -> Result<(), String> {
    converge3(seed, lag, phone_from, None)
}

/// `laptop_at`: a third device (on the server's folder, as the desk) joining at that minute, acting too.
fn converge3(seed: u64, lag: i64, phone_from: i64, laptop_at: Option<i64>) -> Result<(), String> {
    let (base, server, key, desk, phone, desk_notes, phone_notes) = two_devices(&format!("sw2-sim-{seed}-{lag}-{phone_from}-{laptop_at:?}"));
    let laptop = Computer::new(&base, "laptop");
    let laptop_notes = base.join("laptop-notes");
    std::fs::create_dir_all(&laptop_notes).unwrap();
    let phone_folder = base.join("phone-sync").join("Sioul");
    std::fs::create_dir_all(&phone_folder).unwrap();
    desk.write("config/config.toml", "[words.codes.code]\nadd = [\"Bestätigungscode\", \"Sicherheitscode\"]\n\n[[site]]\nid = \"poste\"\nurl = \"https://poste.example.org\"\n");
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
    let mut admins = 0;
    for minute in 0..90i64 {
        let joined = laptop_at.is_some_and(|at| minute >= at);
        for turn in 0..if laptop_at.is_some_and(|at| minute > at + 1) { 3 } else { 2 } {
            let (who, notes) = match turn { 0 => (&desk, &desk_notes), 1 => (&phone, &phone_notes), _ => (&laptop, &laptop_notes) };
            let name = match turn { 0 => "desk", 1 => "phone", _ => "laptop" };
            match next() % 14 {
                0 => {
                    let line = sioul_core::budget::Line { label: format!("L{minute}{name}"), amount: sioul_core::money::Money(-100 * (minute + 1)), ..bread() };
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
                3 | 4 => {
                    let all = sioul_core::timelog::sessions_in(&who.path("data/time"));
                    let minutes = (next() % 90 + 1) as u32;
                    let pick = if next() % 2 == 0 { "letters" } else { "garden" };
                    if let Some(one) = all.iter().find(|s| s.task == pick || s.project == pick) {
                        sioul_core::timelog::update_in(&who.path("data/time"), &[one.key()], |s| s.minutes = minutes).unwrap();
                        log.push(format!("{minute} {name} {pick} → {minutes}"));
                    }
                }
                5 => {
                    sioul_core::timelog::record_in(&who.path("data/time"), &sioul_core::timelog::Session { project: "admin".into(), start: 1_789_920_000 + minute * 3600 + turn * 60, minutes: 10, ..Default::default() }).unwrap();
                    admins += 1;
                    log.push(format!("{minute} {name} add session"));
                }
                6 => {
                    change_words(who, "add", |list| list.push(format!("W{minute}{name}")));
                    log.push(format!("{minute} {name} add word"));
                }
                7 => {
                    change_words(who, "add", |list| {
                        if list.len() > 2 {
                            list.remove(list.len() - 1);
                        }
                    });
                    log.push(format!("{minute} {name} remove a word"));
                }
                8 => {
                    let id = format!("s{minute}{name}");
                    let _ = sioul_core::config::add_site(&who.path("config/config.toml"), &id, &id, &format!("https://{id}.example.org"));
                    log.push(format!("{minute} {name} pin {id}"));
                }
                9 => {
                    let pinned = sites(who);
                    if let Some(id) = pinned.iter().rev().find(|id| id.as_str() != "poste") {
                        let _ = sioul_core::config::remove_site(&who.path("config/config.toml"), id);
                        log.push(format!("{minute} {name} unpin {id}"));
                    }
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
        if joined {
            laptop.exchange_notes(&server, &key, now + 45_000, &laptop_notes);
            announce(&server, &key, &laptop, 2, now + 45_000);
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
        if laptop_at.is_some() {
            laptop.exchange_notes(&server, &key, now + 45_000, &laptop_notes);
            announce(&server, &key, &laptop, 2, now + 45_000);
        }
        now += MINUTE;
    }
    if laptop_at.is_some() {
        let (ll, ls, lw, lsite) = (lines(&laptop_notes), sessions(&laptop), words(&laptop, "add"), sites(&laptop));
        let (dl, ds, dw, dsite) = (lines(&desk_notes), sessions(&desk), words(&desk, "add"), sites(&desk));
        if (ll.clone(), ls.clone(), lw.clone(), lsite.clone()) != (dl.clone(), ds.clone(), dw.clone(), dsite.clone()) || writes(&laptop) != 2 {
            let _ = std::fs::remove_dir_all(&base);
            return Err(format!("seed {seed} lag {lag} phone_from {phone_from} laptop {laptop_at:?}: the laptop differs from the desk\n lines laptop {ll:?}\n lines desk {dl:?}\n sessions laptop {ls:?}\n sessions desk {ds:?}\n words laptop {lw:?}\n words desk {dw:?}\n sites laptop {lsite:?}\n sites desk {dsite:?}"));
        }
    }
    let formats_now = (writes(&desk), writes(&phone));
    let (dl, pl) = (lines(&desk_notes), lines(&phone_notes));
    let (ds, ps) = (sessions(&desk), sessions(&phone));
    let (dw, pw) = (words(&desk, "add"), words(&phone, "add"));
    let (dsite, psite) = (sites(&desk), sites(&phone));
    let _ = std::fs::remove_dir_all(&base);
    let one = |s: &[(String, i64, i64, String)], what: &str| s.iter().filter(|x| x.0 == what).count();
    let admin = |s: &[(String, i64, i64, String)]| s.iter().filter(|x| x.0 == "admin").count();
    let mut wrong = Vec::new();
    if formats_now != (2, 2) {
        wrong.push(format!("formats {formats_now:?}"));
    }
    if dl != pl {
        wrong.push("lines differ".into());
    }
    if ds != ps {
        wrong.push("sessions differ".into());
    }
    if dw != pw {
        wrong.push("words differ".into());
    }
    if dsite != psite {
        wrong.push("sites differ".into());
    }
    for (who, s) in [("desk", &ds), ("phone", &ps)] {
        if one(s, "letters") != 1 || one(s, "garden") != 1 {
            wrong.push(format!("{who}: letters {} garden {}", one(s, "letters"), one(s, "garden")));
        }
        if admin(s) != admins {
            wrong.push(format!("{who}: {} admin sessions for {admins} recorded", admin(s)));
        }
    }
    if wrong.is_empty() {
        return Ok(());
    }
    Err(format!("seed {seed} lag {lag} phone_from {phone_from}: {}\n lines desk {dl:?}\n lines phone {pl:?}\n sessions desk {ds:?}\n sessions phone {ps:?}\n words desk {dw:?}\n words phone {pw:?}\n sites desk {dsite:?}\n sites phone {psite:?}\n log {log:?}", wrong.join("; ")))
}

/// Many seeds under eDrive, both devices acting each minute (lines,
/// sessions, words, sites), the phone updated at once, at minute 20 or 45:
/// both switch and hold the same, one session of each kind noted, every
/// session recorded once (second review, S5).
#[test]
fn many_seeds_converge_through_the_switch() {
    let mut failed = Vec::new();
    let mut runs = 0;
    for seed in 1..=40u64 {
        for lag in [1, 7, 30] {
            for phone_from in [0, 20, 45] {
                runs += 1;
                if let Err(e) = converge(seed, lag, phone_from) {
                    failed.push(e);
                }
            }
        }
    }
    let summary: Vec<String> = failed.iter().map(|e| e.lines().next().unwrap_or_default().to_string()).collect();
    assert!(failed.is_empty(), "{} of {runs} runs fail:\n{}\n\nfirst in full:\n{}", failed.len(), summary.join("\n"), failed.first().cloned().unwrap_or_default());
}

// ---------------------------------------------------------------- S6: an id written at the switch, lost in the mixed window

/// A session corrected on a device still on format 1 (its value without
/// the id) keeps the id the switch wrote into it on the device that
/// switched; changed on both later, it stays one (S6).
#[test]
fn a_session_corrected_in_the_mixed_window_keeps_the_id_the_switch_wrote() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw2-s6-id-lost");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    announce(&folder, &key, &desk, 2, now);
    for _ in 0..2 {
        phone.exchange_notes(&folder, &key, now, &phone_notes);
        now += MINUTE;
    }
    assert_eq!((writes(&desk), writes(&phone)), (0, 2), "the mixed window");
    let id_on_phone = || sioul_core::timelog::sessions_in(&phone.path("data/time"))[0].id.clone();
    let given = id_on_phone();
    assert!(!given.is_empty(), "the switch wrote an id");
    let key_of = sioul_core::timelog::sessions_in(&desk.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of], |s| s.minutes = 45).unwrap();
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    assert_eq!(sessions(&phone), [("letters".to_string(), 1_789_900_000, 45, String::new())]);
    let lost = id_on_phone() != given;
    // Then both on format 2: the phone moves the session to a project while the desk corrects it.
    now = rounds_announced(&folder, &key, &both, now, 3);
    let key_of = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time"))[0].key();
    sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of(&phone)], |s| s.project = "orchard".into()).unwrap();
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.minutes = 50).unwrap();
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    rounds(&folder, &key, &both, now, 3);
    let all = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time")).into_iter().map(|s| (s.project, s.minutes, s.id.is_empty())).collect::<Vec<_>>();
    assert!(!lost && all(&desk).len() == 1 && all(&phone).len() == 1, "id lost in the mixed window: {lost}; then one session on each: desk {:?} phone {:?}", all(&desk), all(&phone));
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- S7: a hostile folder hides every file of a device

/// A hostile folder hides every file of a live 0.0.3 device (its entry, its
/// round, its notes): this device read its records, keeps what it knows of
/// it, and still holds format 2 back (S7).
#[test]
fn a_hostile_folder_hiding_every_file_of_a_device_keeps_the_switch_shut() {
    let (base, folder, key, desk, _, desk_notes, _) = two_devices("sw2-s7-hostile");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange_notes(&folder, &key, NOW, &desk_notes);
    announce(&folder, &key, &desk, 2, NOW);
    let older = uuid::Uuid::new_v4().to_string();
    let change = Change { k: "config/safe-senders.txt#b@example.org".into(), v: Some(String::new()), b: String::new(), build: "0.0.3 (f054336abcde)".into(), f: 0 };
    super::tests::append_to(&round_file(&folder, &older, 1), &super::tests::record_of(&key, &older, 1, ((NOW + MINUTE) as u64) << 16, &change));
    let entry = crate::devices::Entry { id: older.clone(), version: "0.0.3".into(), working: true, started: (NOW + MINUTE) / 1000, wrote: Some((1, 1)), ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &entry).unwrap();
    std::fs::write(folder.join(format!("{older}.toml")), format!("at = {}\nround = 1\n", (NOW + MINUTE) / 1000)).unwrap();
    let laptop = Computer::new(&base, "laptop");
    let laptop_notes = base.join("laptop-notes");
    std::fs::create_dir_all(&laptop_notes).unwrap();
    laptop.exchange_notes(&folder, &key, NOW + 2 * MINUTE, &laptop_notes);
    announce(&folder, &key, &laptop, 2, NOW + 2 * MINUTE);
    desk.exchange_notes(&folder, &key, NOW + 3 * MINUTE, &desk_notes);
    assert!(writes(&desk) < 2, "the 0.0.3 device holds format 2 back");
    // The server: every file of the older device taken away from the desk's view.
    std::fs::remove_file(folder.join("devices").join(format!("{older}.device"))).unwrap();
    std::fs::remove_file(round_file(&folder, &older, 1)).unwrap();
    std::fs::remove_file(folder.join(format!("{older}.toml"))).unwrap();
    desk.exchange_notes(&folder, &key, NOW + 4 * MINUTE, &desk_notes);
    desk.exchange_notes(&folder, &key, NOW + 5 * MINUTE, &desk_notes);
    assert!(writes(&desk) < 2, "a device whose records this desk read three minutes ago still holds format 2 back");
    // Listed in Settings all the same (Forget this device offered there), with when it was last heard.
    let known = formats(&folder, &key, &desk.memory, &desk.id, NOW + 5 * MINUTE);
    assert!(known.holders.iter().any(|(id, _)| *id == older) && known.heard.get(&older).is_some_and(|at| *at >= NOW + MINUTE), "{known:?}");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- S8: a switch stopped half way

/// The switch stopped half way (ids written into the configuration and the
/// months, the ledger not writable): the device stays on format 1, says so,
/// switches at the next exchange; nothing doubled or lost (S8).
#[cfg(unix)]
#[test]
fn a_switch_stopped_half_way_doubles_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw2-s8-half");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("config/config.toml", "[words.codes.code]\nadd = [\"Bestätigungscode\"]\n\n[[time_off]]\nfrom = 2026-08-01\nuntil = 2026-08-15\nlabel = \"Summer\"\n");
    desk.write("data/time/2026-09.toml", "[[session]]\ntask = \"letters\"\nstart = 1789900000\nminutes = 20\ndone = true\n");
    std::fs::write(ledger_path(&desk_notes), format!("{BUDGET}\n[[line]]\nbudget = \"home\"\ndate = 2026-09-05\namount = -12\nlabel = \"Boulangerie\"\n")).unwrap();
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    announce(&folder, &key, &phone, 2, now);
    std::fs::set_permissions(&desk_notes, std::fs::Permissions::from_mode(0o555)).unwrap();
    let half = desk.exchange_notes(&folder, &key, now, &desk_notes);
    std::fs::set_permissions(&desk_notes, std::fs::Permissions::from_mode(0o755)).unwrap();
    now += MINUTE;
    let stopped = writes(&desk) < 2 && half.problems.iter().any(|p| p.starts_with("share-format-wait"));
    let session_id = sioul_core::timelog::sessions_in(&desk.path("data/time"))[0].id.clone();
    assert!(stopped && !session_id.is_empty(), "the switch stopped half way, ids already in the months: {stopped} {session_id:?} {:?}", half.problems);
    announce(&folder, &key, &desk, 2, now);
    rounds_announced(&folder, &key, &both, now, 4);
    let time_off = |c: &Computer| list_of(&c.read("config/config.toml"), "time_off").len();
    let state = (writes(&desk), writes(&phone), sessions(&desk).len(), sessions(&phone).len(), lines(&desk_notes).len(), lines(&phone_notes).len(), time_off(&desk), time_off(&phone));
    assert_eq!(state, (2, 2, 1, 1, 1, 1, 1, 1), "stopped half way: {stopped} (problems {:?}, the session's id then {session_id:?}); formats, sessions, lines and days off on desk and phone", half.problems);
    let _ = std::fs::remove_dir_all(&base);
}

/// A third device joining in the middle of the switch converges with the
/// others (S5b).
#[test]
fn a_third_device_joining_mid_switch_converges() {
    let mut failed = Vec::new();
    let mut runs = 0;
    for seed in 1..=25u64 {
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

// ---------------------------------------------------------------- S9: a later removal passed over on one device only

/// The mixed window: the phone switched, the desk still writes format 1 (its
/// entry says it reads format 2). The phone corrects a line; a minute later
/// the desk takes it out: the later word, the removal, holds on both (S9).
#[test]
fn a_later_removal_from_format_1_is_taken_on_every_device() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("sw2-s9-removal");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    std::fs::write(ledger_path(&desk_notes), format!("{BUDGET}\n[[line]]\nbudget = \"home\"\ndate = 2026-09-03\namount = -42\nlabel = \"Pharmacie\"\n")).unwrap();
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    announce(&folder, &key, &desk, 2, now);
    for _ in 0..2 {
        phone.exchange_notes(&folder, &key, now, &phone_notes);
        now += MINUTE;
    }
    assert_eq!((writes(&desk), writes(&phone)), (0, 2), "the mixed window");
    // The phone corrects it.
    edit_ledger(&phone_notes, |lines| {
        lines.get_mut(0).unwrap()["amount"] = toml_edit::value(-33);
    });
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    // The desk, a minute later, takes it out (format 1).
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    edit_ledger(&desk_notes, |lines| {
        lines.remove(0);
    });
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    announce(&folder, &key, &phone, 2, now);
    rounds_announced(&folder, &key, &both, now, 4);
    assert_eq!((lines(&desk_notes), lines(&phone_notes)), (Vec::new(), Vec::new()), "the later word, the removal, holds on both");
    let _ = std::fs::remove_dir_all(&base);
}
