// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sharing's format, and a sharing written by an older Sioul
//! (docs/database.md, "The format of what travels").
//!
//! `tests/fixtures/format-1/` holds a sharing as Sioul 0.0.3 and 0.0.4 wrote
//! it (format 1): two devices, a desk and a phone, their files, their
//! memories and their records, made by `make_format_1_fixtures` at ed7c3f1,
//! before format 2 was written. Stage `a`: the desk's settings, sessions and
//! ledger shared, the phone joined. Stage `b`: the phone, still on that
//! Sioul, changed some of them afterwards (a line edited, one taken out, a
//! word added, a site pinned, sessions rewritten without their ids, as that
//! Sioul rewrites a month). Fiction only: example.org, invented amounts.

use super::tests::{Computer, MINUTE, NOW, copy_dir, quick_key, scratch};
use super::*;

/// The fixed names of the fixtures' two devices.
pub(super) const DESK: &str = "11111111-1111-4111-8111-111111111111";
pub(super) const PHONE: &str = "22222222-2222-4222-8222-222222222222";
pub(super) const PASSPHRASE: &str = "four words make a passphrase";

/// What stands for the folder the fixtures were made in, in the memories' paths.
pub(super) const BASE: &str = "@BASE@";

const CONFIG_A: &str = r#"language = "fr"

[words.codes.code]
add = ["Bestätigungscode"]
remove = ["code temporaire"]

[words.brands.brands]
add = { "Ma Banque" = ["mabanque.example.org"] }
remove = ["Banque Exemple"]

[[site]]
id = "poste"
name = "La Poste"
url = "https://poste.example.org"
site = "admin"

[[site]]
id = "impots"
name = "Impôts"
url = "https://impots.example.org"
site = "admin"
"#;

/// Sessions of September 2026: two noted for the garden at the same start
/// (an older Sioul keeps one of them only), one with an id (as a newer
/// Sioul makes them).
const SESSIONS_A: &str = r#"[[session]]
task = "task-letters"
start = 1789900000
minutes = 30
done = true

[[session]]
project = "garden"
start = 1789912800
minutes = 45
done = false
note = "Pruning"

[[session]]
project = "garden"
start = 1789912800
minutes = 20
done = false

[[session]]
id = "5e550000-0000-4000-8000-000000000004"
project = "admin"
start = 1789920000
minutes = 25
done = true
"#;

/// A ledger: two presets without ids (an older Sioul keeps one of them
/// only), lines without ids (two the same, which it keeps as one), lines
/// with ids (as a newer Sioul makes them, two the same but for their ids).
const LEDGER_A: &str = r#"[[budget]]
id = "home"
title = "Home"
period = "month"
target = 0

[[preset]]
budget = "home"
label = "Rent"
amount = -600
every = "month"
day = 1

[[preset]]
budget = "home"
label = "Insurance"
amount = -150
every = "year"
month = 3
day = 15

[[line]]
budget = "home"
date = 2026-09-03
amount = -42
label = "Pharmacie"

[[line]]
budget = "home"
date = 2026-09-05
amount = -12
label = "Boulangerie"

[[line]]
budget = "home"
date = 2026-09-05
amount = -12
label = "Boulangerie"

[[line]]
id = "aaaaaaaa-0000-4000-8000-000000000001"
budget = "home"
date = 2026-09-10
amount = -60
label = "Essence"

[[line]]
id = "bbbbbbbb-0000-4000-8000-000000000002"
budget = "home"
date = 2026-09-12
amount = -8
label = "Café"

[[line]]
id = "cccccccc-0000-4000-8000-000000000003"
budget = "home"
date = 2026-09-12
amount = -8
label = "Café"

[[reserve]]
id = "savings"
title = "Savings account"
balance = 2000
as_of = 2026-09-01

[[cover]]
reserve = "savings"
budget = "home"

[[split]]
account = "checking"
words = ["supermarché"]
budget = "home"
"#;

/// A computer of the fixtures, under its fixed name.
pub(super) fn fixed(base: &Path, name: &str, id: &str) -> Computer {
    let mut computer = Computer::new(base, name);
    computer.id = id.to_string();
    computer
}

/// A stage of the fixtures copied from `base` to `out`: the folder, both
/// devices' files and notes, their memories (the folder they were made in
/// written `BASE`); not the copies kept before sharing, nor the history.
fn dump(base: &Path, out: &Path) {
    let _ = std::fs::remove_dir_all(out);
    std::fs::create_dir_all(out).unwrap();
    copy_dir(base, out);
    for device in ["desk", "phone"] {
        let share = out.join(device).join("state").join("share");
        for entry in std::fs::read_dir(&share).unwrap().filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() {
                std::fs::remove_dir_all(entry.path()).unwrap();
            } else if !matches!(name.as_str(), "memory.json" | "files.json") {
                std::fs::remove_file(entry.path()).unwrap();
            } else {
                let text = std::fs::read_to_string(entry.path()).unwrap();
                std::fs::write(entry.path(), text.replace(&base.display().to_string(), BASE)).unwrap();
            }
        }
    }
}

/// The fixtures, made by the code of the day: run once, at ed7c3f1, with
/// `SIOUL_FORMAT1_OUT=<folder> cargo test --release -p sioul-sync make_format_1_fixtures -- --ignored`,
/// and copied into `tests/fixtures/format-1/`. Run again with a newer Sioul,
/// it would make that Sioul's sharing, not 0.0.3's: the fixtures are kept as made.
#[test]
#[ignore]
fn make_format_1_fixtures() {
    let Some(out) = std::env::var_os("SIOUL_FORMAT1_OUT").map(PathBuf::from) else { return };
    let base = scratch("format1");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, PASSPHRASE).unwrap();
    let (desk, phone) = (fixed(&base, "desk", DESK), fixed(&base, "phone", PHONE));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    std::fs::create_dir_all(&desk_notes).unwrap();
    std::fs::create_dir_all(&phone_notes).unwrap();
    desk.write("config/config.toml", CONFIG_A);
    desk.write("data/time/2026-09.toml", SESSIONS_A);
    std::fs::write(desk_notes.join("sioul-budgets.toml"), LEDGER_A).unwrap();
    desk.exchange_notes(&folder, &key, NOW, &desk_notes);
    phone.exchange_notes(&folder, &key, NOW + MINUTE, &phone_notes);
    desk.exchange_notes(&folder, &key, NOW + 2 * MINUTE, &desk_notes);
    phone.exchange_notes(&folder, &key, NOW + 3 * MINUTE, &phone_notes);
    dump(&base, &out.join("a"));

    // The phone, on that Sioul, changes things afterwards.
    let ledger_path = phone_notes.join("sioul-budgets.toml");
    let mut ledger: toml_edit::DocumentMut = std::fs::read_to_string(&ledger_path).unwrap().parse().unwrap();
    let lines = ledger["line"].as_array_of_tables_mut().unwrap();
    for line in lines.iter_mut() {
        if line.get("id").and_then(toml_edit::Item::as_str) == Some("aaaaaaaa-0000-4000-8000-000000000001") {
            line["amount"] = toml_edit::value(-65);
        }
    }
    lines.retain(|line| line.get("label").and_then(toml_edit::Item::as_str) != Some("Pharmacie"));
    let mut market = toml_edit::Table::new();
    market["budget"] = toml_edit::value("home");
    market["date"] = toml_edit::value("2026-09-20".parse::<toml_edit::Datetime>().unwrap());
    market["amount"] = toml_edit::value(-23);
    market["label"] = toml_edit::value("Marché");
    lines.push(market);
    std::fs::write(&ledger_path, ledger.to_string()).unwrap();

    let mut config: toml_edit::DocumentMut = phone.read("config/config.toml").parse().unwrap();
    config["words"]["codes"]["code"]["add"].as_array_mut().unwrap().push("Sicherheitscode");
    let mut caf = toml_edit::Table::new();
    caf["id"] = toml_edit::value("caf");
    caf["name"] = toml_edit::value("CAF");
    caf["url"] = toml_edit::value("https://caf.example.org");
    caf["site"] = toml_edit::value("admin");
    config["site"].as_array_of_tables_mut().unwrap().push(caf);
    phone.write("config/config.toml", &config.to_string());

    // A month rewritten as that Sioul rewrites it: every session without its id.
    let mut month: toml::Table = toml::from_str(&phone.read("data/time/2026-09.toml")).unwrap();
    let sessions = month.get_mut("session").and_then(toml::Value::as_array_mut).unwrap();
    for session in sessions.iter_mut() {
        let table = session.as_table_mut().unwrap();
        table.remove("id");
        match (table.get("project").and_then(toml::Value::as_str), table.get("start").and_then(toml::Value::as_integer)) {
            (Some("garden"), Some(1789912800)) => {
                table.insert("minutes".into(), toml::Value::Integer(50));
            }
            (Some("admin"), _) => {
                table.insert("minutes".into(), toml::Value::Integer(35));
            }
            _ => {}
        }
    }
    let mut weeding = toml::Table::new();
    weeding.insert("project".into(), "garden".into());
    weeding.insert("start".into(), toml::Value::Integer(1789999200));
    weeding.insert("minutes".into(), toml::Value::Integer(15));
    weeding.insert("done".into(), toml::Value::Boolean(false));
    sessions.push(toml::Value::Table(weeding));
    phone.write("data/time/2026-09.toml", &toml::to_string(&month).unwrap());

    phone.exchange_notes(&folder, &key, NOW + 10 * MINUTE, &phone_notes);
    dump(&base, &out.join("b"));
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- helpers

/// A device's entry in the folder (`devices`), as the window writes it after
/// each exchange: the format its Sioul reads, how far it wrote.
pub(super) fn announce(folder: &Path, key: &[u8; 32], computer: &Computer, format: u32, now: i64) {
    let entry = crate::devices::Entry {
        id: computer.id.clone(),
        version: if format >= 2 { "0.0.5".into() } else { "0.0.3".into() },
        format,
        working: true,
        started: now / 1000,
        imported: now / 1000,
        exported: now / 1000,
        wrote: written(&computer.memory, &computer.id),
        ..crate::devices::Entry::default()
    };
    crate::devices::publish(folder, key, &entry).unwrap();
}

/// The format a device writes, as its memory says.
pub(super) fn writes(computer: &Computer) -> u32 {
    format_of(&computer.memory)
}

/// Devices that each exchange then say they read format 2, round after
/// round, until each writes it. Returns the time after.
pub(super) fn into_format_2(folder: &Path, key: &[u8; 32], devices: &[(&Computer, &Path)], mut now: i64) -> i64 {
    for _ in 0..4 {
        for (computer, notes) in devices {
            computer.exchange_notes(folder, key, now, notes);
            announce(folder, key, computer, 2, now);
            now += MINUTE;
        }
        if devices.iter().all(|(computer, _)| writes(computer) == 2) {
            return now;
        }
    }
    panic!("never wrote format 2");
}

/// Each device exchanges, `rounds` times over.
fn rounds(folder: &Path, key: &[u8; 32], devices: &[(&Computer, &Path)], mut now: i64, rounds: usize) -> i64 {
    for _ in 0..rounds {
        for (computer, notes) in devices {
            computer.exchange_notes(folder, key, now, notes);
            now += MINUTE;
        }
    }
    now
}

fn ledger_path(notes: &Path) -> PathBuf {
    notes.join("sioul-budgets.toml")
}

/// A list of a TOML file, its elements as tables.
fn list_of(text: &str, name: &str) -> Vec<toml::Table> {
    let table: toml::Table = toml::from_str(text).unwrap_or_default();
    table.get(name).and_then(toml::Value::as_array).map(|list| list.iter().filter_map(|v| v.as_table().cloned()).collect()).unwrap_or_default()
}

/// The ledger's lines in a notes folder: label and amount (in cents), sorted.
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

/// A device's sessions of September 2026: project or task, start, minutes, sorted.
fn sessions(computer: &Computer) -> Vec<(String, i64, i64)> {
    let mut out: Vec<(String, i64, i64)> = list_of(&computer.read("data/time/2026-09.toml"), "session")
        .iter()
        .map(|s| {
            let what = s.get("project").or_else(|| s.get("task")).and_then(toml::Value::as_str).unwrap_or_default().to_string();
            (what, s.get("start").and_then(toml::Value::as_integer).unwrap_or(0), s.get("minutes").and_then(toml::Value::as_integer).unwrap_or(0))
        })
        .collect();
    out.sort();
    out
}

/// A device's pinned sites, by id, sorted.
fn sites(computer: &Computer) -> Vec<String> {
    let mut out: Vec<String> = list_of(&computer.read("config/config.toml"), "site").iter().filter_map(|s| s.get("id").and_then(toml::Value::as_str).map(str::to_string)).collect();
    out.sort();
    out
}

/// A list of words in a device's configuration (`words.codes.code.<which>`), sorted.
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

// ---------------------------------------------------------------- the bugs

/// Two lines the same (date, amount, words), made on one device, stay two
/// after going through another device and back, and a third device joining
/// reads two: each has its own id since it was made (`sioul_core::ids`). In
/// format 1 too, the ids telling them apart. Before (ed7c3f1), lines had no
/// id, were named by what they hold, and became one.
#[test]
fn two_identical_lines_made_on_one_device_stay_two() {
    for format in [1, 2] {
        let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices(&format!("identical-lines-{format}"));
        std::fs::write(ledger_path(&desk_notes), "[[budget]]\nid = \"home\"\ntitle = \"Home\"\nperiod = \"month\"\ntarget = 0\n").unwrap();
        let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
        let mut now = rounds(&folder, &key, &both, NOW, 1);
        if format == 2 {
            now = into_format_2(&folder, &key, &both, now);
        }
        let bread = sioul_core::budget::Line { budget: "home".into(), date: "2026-09-05".parse().unwrap(), amount: sioul_core::money::Money(-1200), label: "Boulangerie".into(), planned: false, reserve: None, links: Vec::new(), preset: None };
        sioul_core::budget::record_line(&ledger_path(&desk_notes), &bread, "by hand").unwrap();
        sioul_core::budget::record_line(&ledger_path(&desk_notes), &bread, "by hand").unwrap();
        now = rounds(&folder, &key, &both, now, 2);
        // The phone adds one of its own; it goes round to the desk.
        let fuel = sioul_core::budget::Line { label: "Essence".into(), amount: sioul_core::money::Money(-6000), ..bread.clone() };
        sioul_core::budget::record_line(&ledger_path(&phone_notes), &fuel, "by hand").unwrap();
        now = rounds(&folder, &key, &both, now, 2);
        for notes in [&desk_notes, &phone_notes] {
            assert_eq!(lines(notes), [("Boulangerie".to_string(), -1200), ("Boulangerie".to_string(), -1200), ("Essence".to_string(), -6000)], "format {format}, {}", notes.display());
        }
        let ids: BTreeSet<String> = list_of(&std::fs::read_to_string(ledger_path(&phone_notes)).unwrap(), "line").iter().filter_map(|l| l.get("id").and_then(toml::Value::as_str).map(str::to_string)).collect();
        assert_eq!(ids.len(), 3, "each line its own id");
        // Two lines the same without ids (written by hand, or by Sioul up to
        // 0.0.4, which gave lines none): format 1 names them by what they
        // hold, and the other device gets one, as before; format 2 names the
        // second apart, and both travel.
        let mut ledger = std::fs::read_to_string(ledger_path(&desk_notes)).unwrap();
        ledger.push_str("\n[[line]]\nbudget = \"home\"\ndate = 2026-09-06\namount = -3\nlabel = \"Journal\"\n\n[[line]]\nbudget = \"home\"\ndate = 2026-09-06\namount = -3\nlabel = \"Journal\"\n");
        std::fs::write(ledger_path(&desk_notes), ledger).unwrap();
        now = rounds(&folder, &key, &both, now, 2);
        let without_ids = if format == 2 { 2 } else { 1 };
        assert_eq!(count(&lines(&phone_notes), "Journal"), without_ids, "format {format}: {:?}", lines(&phone_notes));
        assert_eq!(count(&lines(&desk_notes), "Journal"), 2);
        // A third device joining reads two.
        let laptop = Computer::new(&base, "laptop");
        let laptop_notes = base.join("laptop-notes");
        std::fs::create_dir_all(&laptop_notes).unwrap();
        if format == 2 {
            announce(&folder, &key, &laptop, 2, now);
        }
        laptop.exchange_notes(&folder, &key, now, &laptop_notes);
        laptop.exchange_notes(&folder, &key, now + MINUTE, &laptop_notes);
        assert_eq!(count(&lines(&laptop_notes), "Boulangerie"), 2, "format {format}: the device joining");
        assert_eq!(count(&lines(&laptop_notes), "Journal"), without_ids, "format {format}: the device joining");
        let _ = std::fs::remove_dir_all(&base);
    }
}

/// One session changed on two devices in the same exchange (moved to another
/// project on one, its minutes corrected on the other) stays one in format
/// 2, the later change winning whole, as every entry's. Format 1, which names
/// sessions by their start, task and project, keeps two: what an older
/// sharing does still, and what Sioul did before (ed7c3f1).
#[test]
fn one_session_changed_on_two_devices_stays_one() {
    for format in [1, 2] {
        let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices(&format!("one-session-{format}"));
        let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
        let start = 1_789_912_800;
        sioul_core::timelog::record_in(&desk.path("data/time"), &sioul_core::timelog::Session { project: "garden".into(), start, minutes: 30, ..Default::default() }).unwrap();
        let mut now = rounds(&folder, &key, &both, NOW, 2);
        if format == 2 {
            now = into_format_2(&folder, &key, &both, now);
        }
        assert_eq!(sessions(&phone), [("garden".to_string(), start, 30)]);
        let key_of = |computer: &Computer| sioul_core::timelog::sessions_in(&computer.path("data/time"))[0].key();
        sioul_core::timelog::update_in(&desk.path("data/time"), &[key_of(&desk)], |s| s.project = "orchard".into()).unwrap();
        sioul_core::timelog::update_in(&phone.path("data/time"), &[key_of(&phone)], |s| s.minutes = 50).unwrap();
        rounds(&folder, &key, &both, now, 2);
        if format == 2 {
            let (here, there) = (sessions(&desk), sessions(&phone));
            assert_eq!(here.len(), 1, "format 2: one session: {here:?}");
            assert_eq!(here, there, "the same on both");
            assert_eq!(here, [("garden".to_string(), start, 50)], "the later change, whole");
            let ids = |c: &Computer| sioul_core::timelog::sessions_in(&c.path("data/time")).into_iter().map(|s| s.id).collect::<Vec<_>>();
            assert_eq!(ids(&desk), ids(&phone));
        } else {
            assert_eq!(sessions(&desk).len(), 2, "format 1 doubles it, as before: {:?}", sessions(&desk));
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}

/// Two devices each pin a site in the same exchange: both sites stay; each
/// adds a word to the same list: both words stay; one takes a word away while
/// the other adds another: both changes hold. Format 1 sends each list
/// whole, and the later list wins: one change is lost, as before (ed7c3f1).
#[test]
fn sites_and_words_changed_on_two_devices_at_once_all_hold() {
    for format in [1, 2] {
        let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices(&format!("sites-words-{format}"));
        let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
        desk.write("config/config.toml", CONFIG_A);
        let mut now = rounds(&folder, &key, &both, NOW, 2);
        if format == 2 {
            now = into_format_2(&folder, &key, &both, now);
        }
        assert_eq!(sites(&phone), ["impots", "poste"]);
        sioul_core::config::add_site(&desk.path("config/config.toml"), "caf", "CAF", "https://caf.example.org").unwrap();
        sioul_core::config::add_site(&phone.path("config/config.toml"), "msa", "MSA", "https://msa.example.org").unwrap();
        change_words(&desk, "add", |list| list.push("Sicherheitscode".into()));
        change_words(&phone, "add", |list| list.push("Einmalcode".into()));
        now = rounds(&folder, &key, &both, now, 2);
        let pinned = [sites(&desk), sites(&phone)];
        let added = [words(&desk, "add"), words(&phone, "add")];
        if format == 2 {
            for sites in &pinned {
                assert_eq!(sites, &["caf", "impots", "msa", "poste"], "both sites stay");
            }
            for words in &added {
                assert_eq!(words, &["Bestätigungscode", "Einmalcode", "Sicherheitscode"], "both words stay");
            }
        } else {
            assert!(pinned.iter().all(|s| s.len() == 3) && added.iter().all(|w| w.len() == 2), "format 1: the later list wins, one change lost: {pinned:?} {added:?}");
        }
        // One takes a word away while the other adds another.
        change_words(&desk, "add", |list| list.retain(|w| w != "Bestätigungscode"));
        change_words(&phone, "add", |list| list.push("Prüfcode".into()));
        rounds(&folder, &key, &both, now, 2);
        if format == 2 {
            for device in [&desk, &phone] {
                assert_eq!(words(device, "add"), ["Einmalcode", "Prüfcode", "Sicherheitscode"], "both changes hold");
            }
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}

// ---------------------------------------------------------------- an older sharing

/// A stage of the fixtures, copied into `base`, the memories saying where they are now.
fn from_fixtures(stage: &str, base: &Path) {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("format-1").join(stage);
    copy_dir(&from, base);
    for device in ["desk", "phone"] {
        for name in ["memory.json", "files.json"] {
            let path = base.join(device).join("state").join("share").join(name);
            if let Ok(text) = std::fs::read_to_string(&path) {
                // Each path made where the fixtures were, made here: parsed, not
                // pasted into the text (a Windows path's backslashes are escapes
                // in JSON), its separators this system's.
                let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
                rebase(&mut value, base);
                std::fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
            }
        }
    }
}

/// Every string of `value` naming a path below `BASE` (where the fixtures
/// were made), and every key that does, named below `base` instead.
fn rebase(value: &mut serde_json::Value, base: &Path) {
    let moved = |s: &str| {
        s.strip_prefix(BASE).map(|rest| rest.split('/').filter(|part| !part.is_empty()).fold(base.to_path_buf(), |path, part| path.join(part)).display().to_string())
    };
    match value {
        serde_json::Value::String(s) => {
            if let Some(path) = moved(s) {
                *s = path;
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|item| rebase(item, base)),
        serde_json::Value::Object(map) => {
            let entries: Vec<(String, serde_json::Value)> = std::mem::take(map).into_iter().collect();
            for (key, mut item) in entries {
                rebase(&mut item, base);
                map.insert(moved(&key).unwrap_or(key), item);
            }
        }
        _ => {}
    }
}

/// The phone's later records (stage b), arriving in the folder: its file and its notes.
fn phone_writes_later(base: &Path) {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("format-1").join("b").join("Sioul");
    for name in [format!("{PHONE}-1.jsonl"), format!("{PHONE}.toml")] {
        std::fs::copy(from.join(&name), base.join("Sioul").join(&name)).unwrap();
    }
}

/// What the desk's ledger, sessions, sites and words hold after the phone's
/// later changes, whichever came first: its own changes or the desk's
/// switch to format 2.
fn the_desk_after_the_phone(desk: &Computer, notes: &Path) {
    let lines = lines(notes);
    assert_eq!(count(&lines, "Essence"), 1, "one line edited stays one: {lines:?}");
    assert!(lines.contains(&("Essence".to_string(), -6500)), "{lines:?}");
    assert_eq!(count(&lines, "Pharmacie"), 0, "taken out: {lines:?}");
    assert_eq!(count(&lines, "Marché"), 1, "{lines:?}");
    assert_eq!(count(&lines, "Café"), 2, "two the same, each with its id: {lines:?}");
    assert_eq!(count(&lines, "Boulangerie"), 2, "the desk's own two: {lines:?}");
    assert_eq!(list_of(&std::fs::read_to_string(ledger_path(notes)).unwrap(), "preset").len(), 2, "both presets");
    assert_eq!(sites(desk), ["caf", "impots", "poste"]);
    assert_eq!(words(desk, "add"), ["Bestätigungscode", "Sicherheitscode"]);
    let sessions = sessions(desk);
    for one in [("task-letters", 1_789_900_000, 30), ("admin", 1_789_920_000, 35), ("garden", 1_789_999_200, 15), ("garden", 1_789_912_800, 50)] {
        assert_eq!(sessions.iter().filter(|(what, start, minutes)| (what.as_str(), *start, *minutes) == one).count(), 1, "{one:?} once: {sessions:?}");
    }
    assert!(!sessions.iter().any(|(what, _, minutes)| what == "admin" && *minutes == 25), "the admin session changed, not doubled: {sessions:?}");
}

/// A sharing written by Sioul 0.0.3 (the fixtures, format 1), read by this
/// Sioul on the desk while the phone still runs 0.0.3: the desk writes
/// format 1 (the phone's entry says no format), its memory read as it is,
/// nothing sent again; the phone's later changes come in as 0.0.3 would
/// write them. The phone updated, both write format 2: the desk's memory is
/// said in format 2's names once, and the only changes it then sends are
/// those format 1 had lost (a second line the same, a second preset without
/// an id, a session noted at the same start as another); a second switch
/// changes nothing. The phone, updated too, reads them: nothing doubled,
/// nothing lost. A device joining afterwards reads the same.
#[test]
fn an_older_sharing_is_read_then_upgraded_without_doubling_or_losing() {
    let base = scratch("upgrade-later");
    from_fixtures("b", &base);
    let folder = base.join("Sioul");
    let key = quick_key(&folder, PASSPHRASE).unwrap();
    let (desk, phone) = (fixed(&base, "desk", DESK), fixed(&base, "phone", PHONE));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    let before = std::fs::read_to_string(ledger_path(&desk_notes)).unwrap();
    // The desk updated, the phone on 0.0.3 (its entry says no format): format 1.
    announce(&folder, &key, &phone, 0, NOW + 15 * MINUTE);
    let first = desk.exchange_notes(&folder, &key, NOW + 20 * MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 0, "format 1 while the phone runs 0.0.3");
    assert_eq!(first.sent, 0, "its memory read as it is: nothing sent again, {first:?}");
    assert!(first.received > 0 && first.problems.is_empty(), "{first:?}");
    assert_ne!(std::fs::read_to_string(ledger_path(&desk_notes)).unwrap(), before);
    announce(&folder, &key, &desk, 2, NOW + 20 * MINUTE);
    the_desk_after_the_phone(&desk, &desk_notes);
    // The phone updated: both write format 2.
    announce(&folder, &key, &phone, 2, NOW + 25 * MINUTE);
    let switched = desk.exchange_notes(&folder, &key, NOW + 30 * MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 2);
    // What format 1 lost goes out: the second Boulangerie, the second preset, the second garden session at one start.
    assert_eq!(switched.sent, 3, "{switched:?}");
    the_desk_after_the_phone(&desk, &desk_notes);
    // Once only: the next exchanges send nothing, and the memory says format 2.
    for minute in [31, 32] {
        assert_eq!(desk.exchange_notes(&folder, &key, NOW + minute * MINUTE, &desk_notes).sent, 0);
    }
    announce(&folder, &key, &desk, 2, NOW + 32 * MINUTE);
    // It meets the desk's format 2 before it has read all the desk wrote: it writes format 2 from its next exchange.
    let updated = phone.exchange_notes(&folder, &key, NOW + 33 * MINUTE, &phone_notes);
    phone.exchange_notes(&folder, &key, NOW + 34 * MINUTE, &phone_notes);
    assert_eq!(writes(&phone), 2, "{updated:?}");
    let held = lines(&phone_notes);
    assert_eq!((count(&held, "Boulangerie"), count(&held, "Essence"), count(&held, "Café"), count(&held, "Pharmacie"), count(&held, "Marché")), (2, 1, 2, 0, 1), "{held:?}");
    assert_eq!(list_of(&std::fs::read_to_string(ledger_path(&phone_notes)).unwrap(), "preset").len(), 2, "the preset 0.0.3 lost comes");
    assert_eq!(sites(&phone), ["caf", "impots", "poste"]);
    assert_eq!(words(&phone, "add"), ["Bestätigungscode", "Sicherheitscode"]);
    let on_phone = sessions(&phone);
    assert_eq!(on_phone.iter().filter(|(what, start, _)| what == "garden" && *start == 1_789_912_800).count(), 1, "{on_phone:?}");
    // A device joining now reads the same lines.
    let laptop = Computer::new(&base, "laptop");
    let laptop_notes = base.join("laptop-notes");
    std::fs::create_dir_all(&laptop_notes).unwrap();
    announce(&folder, &key, &laptop, 2, NOW + 35 * MINUTE);
    laptop.exchange_notes(&folder, &key, NOW + 35 * MINUTE, &laptop_notes);
    assert_eq!(writes(&laptop), 2);
    assert_eq!(lines(&laptop_notes), lines(&phone_notes));
    assert_eq!(sites(&laptop), sites(&phone));
    assert_eq!(words(&laptop, "add"), words(&phone, "add"));
    let _ = std::fs::remove_dir_all(&base);
}

/// The same sharing, the desk writing format 2 before the phone's later
/// changes reach it (written by 0.0.3 before the phone was updated, carried
/// late): they come in translated, nothing doubled, nothing lost, the desk
/// as in the order above. An admin session that 0.0.3 wrote back without its
/// id keeps it. A device joining afterwards reads the same lines.
#[test]
fn an_older_device_s_write_after_the_switch_doubles_nothing() {
    let base = scratch("upgrade-first");
    from_fixtures("a", &base);
    let folder = base.join("Sioul");
    let key = quick_key(&folder, PASSPHRASE).unwrap();
    let (desk, phone) = (fixed(&base, "desk", DESK), fixed(&base, "phone", PHONE));
    let desk_notes = base.join("desk-notes");
    announce(&folder, &key, &phone, 2, NOW + 15 * MINUTE);
    let switched = desk.exchange_notes(&folder, &key, NOW + 20 * MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 2, "{switched:?}");
    assert_eq!(switched.sent, 3, "the second Boulangerie, the second preset, the second garden session: {switched:?}");
    // The phone's 0.0.3 records, written before its update, arrive now.
    phone_writes_later(&base);
    let late = desk.exchange_notes(&folder, &key, NOW + 21 * MINUTE, &desk_notes);
    assert!(late.received > 0 && late.problems.is_empty(), "{late:?}");
    the_desk_after_the_phone(&desk, &desk_notes);
    let admin = sioul_core::timelog::sessions_in(&desk.path("data/time")).into_iter().find(|s| s.project == "admin").unwrap();
    assert_eq!(admin.id, "5e550000-0000-4000-8000-000000000004", "its id kept, though 0.0.3 wrote it back without");
    assert_eq!(desk.exchange_notes(&folder, &key, NOW + 22 * MINUTE, &desk_notes).sent, 0, "nothing echoed back");
    the_desk_after_the_phone(&desk, &desk_notes);
    let laptop = Computer::new(&base, "laptop");
    let laptop_notes = base.join("laptop-notes");
    std::fs::create_dir_all(&laptop_notes).unwrap();
    announce(&folder, &key, &desk, 2, NOW + 22 * MINUTE);
    announce(&folder, &key, &laptop, 2, NOW + 23 * MINUTE);
    laptop.exchange_notes(&folder, &key, NOW + 23 * MINUTE, &laptop_notes);
    assert_eq!(lines(&laptop_notes), lines(&desk_notes));
    assert_eq!(sites(&laptop), sites(&desk));
    assert_eq!(words(&laptop, "add"), words(&desk, "add"));
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- a newer format

/// A part met in a format newer than this Sioul knows (a record saying
/// format 3 for the time part, from a device on a newer Sioul): its changes
/// are read but not written here, they wait; nothing of that part goes out
/// from here, its changes kept in its files until this Sioul is updated;
/// this is said once (`share-newer:time`), the part's format and the build
/// that wrote it kept for the window; the other parts go on.
#[test]
fn a_newer_format_is_read_never_written_over_and_said_once() {
    let (base, folder, key, desk, phone, desk_notes, phone_notes) = two_devices("newer-format");
    let both = [(&desk, desk_notes.as_path()), (&phone, phone_notes.as_path())];
    desk.write("data/time/2026-09.toml", "[[session]]\nid = \"aa\"\nproject = \"garden\"\nstart = 1789912800\nminutes = 30\n");
    let mut now = rounds(&folder, &key, &both, NOW, 2);
    now = into_format_2(&folder, &key, &both, now);
    // A newer Sioul elsewhere writes the time part in format 3.
    let newer = uuid::Uuid::new_v4().to_string();
    let record = |n: u64, change: Change| super::tests::record_of(&key, &newer, n, ((now as u64) << 16) | n, &change);
    let session = "data/time/2026-09.toml#session\u{1f}\u{1e}aa".to_string();
    let first = Change { k: session.clone(), v: Some("[v]\nid = \"aa\"\nproject = \"garden\"\nstart = 1789912800\nminutes = 99\nshape = \"new\"\n".into()), b: String::new(), build: "0.1.0 (abcdef123456)".into(), f: 3 };
    let other = Change { k: "config/safe-senders.txt#c@example.org".into(), v: Some(String::new()), b: String::new(), build: String::new(), f: 0 };
    super::tests::append_to(&round_file(&folder, &newer, 1), &(record(1, first) + &record(2, other)));
    let met = phone.exchange_notes(&folder, &key, now, &phone_notes);
    assert_eq!(met.problems.iter().filter(|p| *p == "share-newer:time").count(), 1, "{met:?}");
    assert!(sessions(&phone).iter().all(|(_, _, minutes)| *minutes == 30), "not written here: {:?}", sessions(&phone));
    assert!(phone.read("config/safe-senders.txt").contains("c@example.org"), "the other parts go on");
    let state = formats(&folder, &key, &phone.memory, &phone.id, now);
    assert_eq!(state.newer, [("time".to_string(), 3, "0.1.0 (abcdef123456)".to_string())]);
    // Said once; a change made here to that part does not go out, and stays in its file.
    sioul_core::timelog::update_in(&phone.path("data/time"), &[sioul_core::timelog::sessions_in(&phone.path("data/time"))[0].key()], |s| s.minutes = 45).unwrap();
    let again = phone.exchange_notes(&folder, &key, now + MINUTE, &phone_notes);
    assert!(!again.problems.iter().any(|p| p.starts_with("share-newer")), "said once: {again:?}");
    assert_eq!(again.sent, 0, "nothing of the time part goes out: {again:?}");
    assert!(sessions(&phone).iter().any(|(_, _, minutes)| *minutes == 45), "kept in its file");
    desk.exchange_notes(&folder, &key, now + 2 * MINUTE, &desk_notes);
    assert!(sessions(&desk).iter().all(|(_, _, minutes)| *minutes == 30), "the desk never got the older device's change: {:?}", sessions(&desk));
    // The other parts still go out from it.
    phone.write("config/safe-senders.txt", "c@example.org\nd@example.org\n");
    assert!(phone.exchange_notes(&folder, &key, now + 3 * MINUTE, &phone_notes).sent > 0);
    let _ = std::fs::remove_dir_all(&base);
}

/// The sharing writes format 2 only once every device says it reads it, and
/// all it wrote before is read here: a device known by its records alone, or
/// whose entry says no format (0.0.3), or does not read here, holds it back,
/// and so does one whose records up to its entry are not all read here; one
/// that left, or was silent for 180 days, does not. Alone in the folder, a
/// device writes format 1 (one joining later may be older).
#[test]
fn format_2_waits_for_every_device() {
    let (base, folder, key, desk, _, desk_notes, _) = two_devices("format-gate");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.exchange_notes(&folder, &key, NOW, &desk_notes);
    announce(&folder, &key, &desk, 2, NOW);
    desk.exchange_notes(&folder, &key, NOW + MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 0, "alone: format 1");
    // A device on 0.0.3: its records (format 1, as that Sioul writes them).
    let older = uuid::Uuid::new_v4().to_string();
    let said = |n: u64, address: &str| super::tests::record_of(&key, &older, n, ((NOW + n as i64 * MINUTE) as u64) << 16, &Change { k: format!("config/safe-senders.txt#{address}"), v: Some(String::new()), b: String::new(), build: if n == 1 { "0.0.3 (f054336abcde)".into() } else { String::new() }, f: 0 });
    super::tests::append_to(&round_file(&folder, &older, 1), &said(1, "b@example.org"));
    desk.exchange_notes(&folder, &key, NOW + 2 * MINUTE, &desk_notes);
    assert!(desk.read("config/safe-senders.txt").contains("b@example.org"), "format 1 read as ever");
    let holders = |at: i64| formats(&folder, &key, &desk.memory, &desk.id, at).holders;
    assert_eq!(holders(NOW + 2 * MINUTE), [(older.clone(), Holds::Older)], "known by its records alone");
    // Its entry, saying no format.
    let entry = |format: u32, wrote: Option<(u32, u64)>, left: bool| crate::devices::Entry { id: older.clone(), version: if format >= 2 { "0.0.5".into() } else { "0.0.3".into() }, format, working: !left, left, started: (NOW + 3 * MINUTE) / 1000, wrote, ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &entry(0, Some((1, 1)), false)).unwrap();
    desk.exchange_notes(&folder, &key, NOW + 3 * MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 0);
    assert_eq!(holders(NOW + 3 * MINUTE), [(older.clone(), Holds::Older)], "0.0.3's entry");
    // Its entry not readable here.
    std::fs::write(folder.join("devices").join(format!("{older}.device")), "not sealed").unwrap();
    assert_eq!(holders(NOW + 3 * MINUTE), [(older.clone(), Holds::Unread)]);
    // Updated: it reads format 2, and wrote a second record, not read here yet.
    crate::devices::publish(&folder, &key, &entry(2, Some((1, 2)), false)).unwrap();
    super::tests::append_to(&round_file(&folder, &older, 1), &said(2, "c@example.org"));
    assert_eq!(holders(NOW + 3 * MINUTE), [(older.clone(), Holds::Unheard)]);
    desk.exchange_notes(&folder, &key, NOW + 4 * MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 0, "decided before reading");
    assert!(holders(NOW + 4 * MINUTE).is_empty());
    desk.exchange_notes(&folder, &key, NOW + 5 * MINUTE, &desk_notes);
    assert_eq!(writes(&desk), 2, "every device reads it, all it wrote read here");
    // One that left, or silent for 180 days, holds nothing back.
    let gone = uuid::Uuid::new_v4().to_string();
    let holding = |at: i64| format_holders(&folder, &key, &desk.id, &Memory::load(&desk.memory, &desk.id).read_n, at).into_iter().filter(|(id, _)| *id == gone).count();
    let gone_entry = |left: bool| crate::devices::Entry { id: gone.clone(), version: "0.0.3".into(), working: !left, left, started: (NOW + 5 * MINUTE) / 1000, ..crate::devices::Entry::default() };
    crate::devices::publish(&folder, &key, &gone_entry(false)).unwrap();
    assert_eq!(holding(NOW + 5 * MINUTE), 1, "its Sioul says no format");
    crate::devices::publish(&folder, &key, &gone_entry(true)).unwrap();
    assert_eq!(holding(NOW + 5 * MINUTE), 0, "it left");
    crate::devices::publish(&folder, &key, &gone_entry(false)).unwrap();
    assert_eq!(holding(NOW + 5 * MINUTE + (SILENT_DAYS + 1) * 86_400_000), 0, "silent 180 days");
    let _ = std::fs::remove_dir_all(&base);
}

/// Format 2 divides a configuration's lists element by element, within
/// tables too, and writes an element back where it stands: words added or
/// taken away one each (a named list's words too), pinned sites by their
/// ids, task kinds by theirs, days off by what they hold; the lighter days
/// one each. Format 1 keeps each list whole.
#[test]
fn format_2_divides_lists_element_by_element_and_writes_them_back() {
    let config = "language = \"fr\"\n\n[tasks]\n[[tasks.kind]]\nid = \"call\"\nlabel = \"Call\"\n\n[words.codes.code]\nadd = [\"Bestätigungscode\"]\nremove = [\"code temporaire\"]\n\n[words.brands.brands]\nadd = { \"Ma Banque\" = [\"mabanque.example.org\"] }\n\n[[site]]\nid = \"poste\"\nurl = \"https://poste.example.org\"\n\n[[time_off]]\nfrom = 2026-08-01\nuntil = 2026-08-15\nlabel = \"Summer\"\n";
    let before: Vec<String> = toml_entries(config, &CONFIG_RULES).unwrap().into_iter().map(|(entry, _)| entry).collect();
    assert!(before.iter().any(|e| e == "words\u{1f}codes\u{1f}code\u{1f}add") && before.iter().any(|e| e == "site") && before.iter().any(|e| e == "tasks\u{1f}kind"), "format 1: whole lists, {before:?}");
    let after: BTreeMap<String, String> = toml_entries(config, &CONFIG_RULES_2).unwrap().into_iter().collect();
    for entry in ["words\u{1f}codes\u{1f}code\u{1f}add\u{1f}\u{1e}v = \"Bestätigungscode\"\n", "words\u{1f}codes\u{1f}code\u{1f}remove\u{1f}\u{1e}v = \"code temporaire\"\n", "words\u{1f}brands\u{1f}brands\u{1f}add\u{1f}Ma Banque\u{1f}\u{1e}v = \"mabanque.example.org\"\n", "site\u{1f}\u{1e}poste", "tasks\u{1f}kind\u{1f}\u{1e}call", "language"] {
        assert!(after.contains_key(entry), "{entry:?} in {:?}", after.keys().collect::<Vec<_>>());
    }
    let time_off = after.keys().find(|e| e.starts_with("time_off\u{1f}\u{1e}")).unwrap();
    assert_eq!(time_off.len(), "time_off\u{1f}\u{1e}".len() + 36, "named by an id derived from it");
    // Written back: a word added, one taken away, a domain added to a named list, a kind and a site added.
    let written = toml_write(
        config,
        &CONFIG_RULES_2,
        &[
            ("words\u{1f}codes\u{1f}code\u{1f}add\u{1f}\u{1e}v = \"Einmalcode\"\n", Some("v = \"Einmalcode\"\n")),
            ("words\u{1f}codes\u{1f}code\u{1f}remove\u{1f}\u{1e}v = \"code temporaire\"\n", None),
            ("words\u{1f}brands\u{1f}brands\u{1f}add\u{1f}Ma Banque\u{1f}\u{1e}v = \"banque.example.org\"\n", Some("v = \"banque.example.org\"\n")),
            ("tasks\u{1f}kind\u{1f}\u{1e}visit", Some("[v]\nid = \"visit\"\nlabel = \"Visit\"\n")),
            ("site\u{1f}\u{1e}caf", Some("[v]\nid = \"caf\"\nurl = \"https://caf.example.org\"\n")),
        ],
        false,
    )
    .unwrap();
    let table: toml::Table = toml::from_str(&written).unwrap();
    assert_eq!(table["words"]["codes"]["code"]["add"].as_array().unwrap().len(), 2, "{written}");
    assert!(table["words"]["codes"]["code"]["remove"].as_array().unwrap().is_empty(), "{written}");
    assert_eq!(table["words"]["brands"]["brands"]["add"]["Ma Banque"].as_array().unwrap().len(), 2, "{written}");
    assert_eq!(table["tasks"]["kind"].as_array().unwrap().len(), 2, "{written}");
    assert_eq!(table["site"].as_array().unwrap().len(), 2, "{written}");
    assert!(written.starts_with("language = \"fr\""), "the rest kept as written: {written}");
    // The lighter days, one each.
    let quiet = "lighter = [2026-09-10, 2026-09-12]\n";
    let days: Vec<String> = toml_entries(quiet, &QUIET_RULES_2).unwrap().into_iter().map(|(e, _)| e).collect();
    assert_eq!(days.len(), 2, "{days:?}");
    let written = toml_write(quiet, &QUIET_RULES_2, &[("lighter\u{1f}\u{1e}v = 2026-09-14\n", Some("v = 2026-09-14\n"))], false).unwrap();
    assert_eq!(toml::from_str::<toml::Table>(&written).unwrap()["lighter"].as_array().unwrap().len(), 3, "{written}");
}
