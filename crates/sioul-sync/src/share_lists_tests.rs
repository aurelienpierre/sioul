// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's own lists, gone or blank on one device, are never read as "the
//! person removed everything" (`LISTS`, `lists_lost`; docs/database.md,
//! "Sioul's own lists"): each list, a file of no bytes and a file removed,
//! on a device that read it before; and a list gone when a change comes.
//! Fiction only.

use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;

/// The parts that carry the lists in the notes folder's layout.
const PARTS: &[&str] = &["projects", "papers", "notes"];

/// An exchange of `c` with `notes` as its notes folder, the lists' parts shared.
pub(super) fn exchange_in(c: &Computer, folder: &Path, key: &[u8; 32], now: i64, notes: &Path) -> Outcome {
    let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
    let stores = stores_of(&config, &c.roots, &|part| PARTS.contains(&part));
    exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: true, hurry: None }, &stores, now).unwrap()
}

/// A device's sealed entry saying it has every store a version added: the
/// gated lists (client profiles, movements) travel.
pub(super) fn says_every_store(c: &Computer, folder: &Path, key: &[u8; 32]) {
    let id = c.id.clone();
    crate::devices::change(&crate::devices::own_path(&c.roots.state), Some((folder, key)), |e| {
        e.id = id;
        e.kind = crate::devices::COMPUTER.into();
        e.working = true;
        e.format = FORMAT;
        e.movements = MOVEMENTS_SHARED;
        e.clients = CLIENTS_SHARED;
        e.persons = PERSONS_SHARED;
        e.supplies = SUPPLIES_SHARED;
    })
    .unwrap();
}

/// A file written, its time set a minute back: never "still being written".
pub(super) fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    let back = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
    std::fs::File::options().write(true).open(path).unwrap().set_modified(back).unwrap();
}

/// One of the lists: its place under the notes folder, two entries, and
/// what tells each of them in the file.
struct List {
    file: &'static str,
    text: &'static str,
    marks: [&'static str; 2],
    third: &'static str,
}

const ALL: [List; 9] = [
    List { file: "sioul-cases.toml", text: "[[case]]\nid = \"acme\"\ntitle = \"Acme site\"\n\n[[case]]\nid = \"garden\"\ntitle = \"Garden\"\n", marks: ["acme", "garden"], third: "\n[[case]]\nid = \"roof\"\ntitle = \"Roof\"\n" },
    List { file: "sioul-clients.toml", text: "[[client]]\nid = \"lumen\"\nname = \"Studio Lumen\"\n\n[[client]]\nid = \"ferro\"\nname = \"Ferro\"\n", marks: ["lumen", "ferro"], third: "\n[[client]]\nid = \"nord\"\nname = \"Nord\"\n" },
    List { file: "sioul-budgets.toml", text: "[[budget]]\nid = \"home\"\ntitle = \"Home\"\nperiod = \"month\"\ntarget = 0\n\n[[budget]]\nid = \"car\"\ntitle = \"Car\"\nperiod = \"month\"\ntarget = 0\n", marks: ["home", "car"], third: "\n[[budget]]\nid = \"cat\"\ntitle = \"Cat\"\n" },
    List { file: "sioul-bank.toml", text: "[[account]]\nid = \"main\"\ntitle = \"Main\"\n\n[[movement]]\naccount = \"main\"\nid = \"fitid-1\"\nlabel = \"Rent\"\n\n[[movement]]\naccount = \"main\"\nid = \"fitid-2\"\nlabel = \"Bread\"\n", marks: ["fitid-1", "fitid-2"], third: "\n[[movement]]\naccount = \"main\"\nid = \"fitid-3\"\nlabel = \"Coffee\"\n" },
    List { file: "sioul-contracts.toml", text: "[[contract]]\nid = \"power\"\nparty = \"Power\"\n\n[[contract]]\nid = \"phone\"\nparty = \"Phone\"\n", marks: ["power", "phone"], third: "\n[[contract]]\nid = \"water\"\nparty = \"Water\"\n" },
    List { file: "sioul-movements.toml", text: "[[name]]\nbank = \"ACME SA\"\nname = \"Acme\"\n\n[[name]]\nbank = \"FERRO SARL\"\nname = \"Ferro\"\n", marks: ["ACME SA", "FERRO SARL"], third: "\n[[name]]\nbank = \"NORD SAS\"\nname = \"Nord\"\n" },
    List { file: "sioul-papers.toml", text: "[[paper]]\nid = \"passport\"\nkind = \"passport\"\ntitle = \"Passport\"\n\n[[paper]]\nid = \"lease\"\nkind = \"other\"\ntitle = \"Lease\"\n", marks: ["passport", "lease"], third: "\n[[paper]]\nid = \"card\"\nkind = \"identity\"\ntitle = \"Card\"\n" },
    List { file: "letters/letters.toml", text: "[[letter]]\nid = \"caf\"\nfile = \"letters/2026/caf.pdf\"\nstatus = \"new\"\nkind = \"other\"\n\n[[letter]]\nid = \"tax\"\nfile = \"letters/2026/tax.pdf\"\nstatus = \"new\"\nkind = \"other\"\n", marks: ["caf", "tax"], third: "\n[[letter]]\nid = \"bank\"\nfile = \"letters/2026/bank.pdf\"\nstatus = \"new\"\nkind = \"other\"\n" },
    List { file: "sioul-supplies.toml", text: "[[supply]]\nid = \"stamps\"\nname = \"Stamps\"\n\n[[supply]]\nid = \"envelopes\"\nname = \"Envelopes\"\n", marks: ["stamps", "envelopes"], third: "\n[[supply]]\nid = \"cards\"\nname = \"Gift cards\"\n" },
];

/// Two devices that both hold `list`, read and sent.
fn two_holding(base: &Path, list: &List) -> (PathBuf, [u8; 32], Computer, Computer, PathBuf, PathBuf) {
    let folder = base.join("Sync").join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(base, "desk"), Computer::new(base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    std::fs::create_dir_all(&phone_notes).unwrap();
    says_every_store(&desk, &folder, &key);
    says_every_store(&phone, &folder, &key);
    put(&desk_notes.join(list.file), list.text);
    exchange_in(&desk, &folder, &key, NOW, &desk_notes);
    exchange_in(&phone, &folder, &key, NOW + MINUTE, &phone_notes);
    exchange_in(&desk, &folder, &key, NOW + 2 * MINUTE, &desk_notes);
    let held = std::fs::read_to_string(place(&phone_notes, list)).unwrap_or_default();
    assert!(list.marks.iter().all(|m| held.contains(m)), "{}: the phone holds the list first: {held}", list.file);
    (folder, key, desk, phone, desk_notes, phone_notes)
}

/// Where a list is under a notes folder: the projects' file under the name it has there (`file_in`).
fn place(notes: &Path, list: &List) -> PathBuf {
    if list.file == sioul_core::projects::OLD_FILE { sioul_core::projects::file_in(notes) } else { notes.join(list.file) }
}

fn holds_both(notes: &Path, list: &List) -> bool {
    let text = std::fs::read_to_string(place(notes, list)).unwrap_or_default();
    list.marks.iter().all(|m| text.contains(m))
}

/// Each list, of no bytes or removed on the desk: nothing leaves the phone,
/// past the ten minutes of `EMPTIED_WAIT` too, and the desk's file comes back
/// whole, said.
#[test]
fn a_list_of_no_bytes_or_removed_takes_nothing_out() {
    for (n, list) in ALL.iter().enumerate() {
        for removed in [false, true] {
            let base = scratch(&format!("lists-{n}-{removed}"));
            let (folder, key, desk, phone, desk_notes, phone_notes) = two_holding(&base, list);
            let path = place(&desk_notes, list);
            if removed {
                std::fs::remove_file(&path).unwrap();
            } else {
                put(&path, "");
            }
            let outcome = exchange_in(&desk, &folder, &key, NOW + 3 * MINUTE, &desk_notes);
            assert!(outcome.problems.iter().any(|p| p == &format!("share-list-back:{}", record_of(list.file))), "{} ({removed}): said: {outcome:?}", list.file);
            for at in [14, 15, 30] {
                exchange_in(&desk, &folder, &key, NOW + at * MINUTE, &desk_notes);
                exchange_in(&phone, &folder, &key, NOW + (at + 1) * MINUTE, &phone_notes);
            }
            assert!(holds_both(&phone_notes, list), "{} ({removed}): the phone keeps it: {}", list.file, std::fs::read_to_string(place(&phone_notes, list)).unwrap_or_default());
            assert!(holds_both(&desk_notes, list), "{} ({removed}): the desk's comes back: {}", list.file, std::fs::read_to_string(place(&desk_notes, list)).unwrap_or_default());
        }
    }
}

/// A list gone on the desk while the phone adds an entry to it: the desk
/// never writes the one change alone (every other entry would then leave
/// everywhere), but the whole list, the new entry with it.
#[test]
fn a_list_gone_when_a_change_comes_is_written_whole() {
    for (n, list) in ALL.iter().enumerate() {
        let base = scratch(&format!("lists-change-{n}"));
        let (folder, key, desk, phone, desk_notes, phone_notes) = two_holding(&base, list);
        let on_phone = place(&phone_notes, list);
        put(&on_phone, &format!("{}{}", std::fs::read_to_string(&on_phone).unwrap(), list.third));
        exchange_in(&phone, &folder, &key, NOW + 3 * MINUTE, &phone_notes);
        std::fs::remove_file(place(&desk_notes, list)).unwrap();
        for at in [4, 20, 40] {
            exchange_in(&desk, &folder, &key, NOW + at * MINUTE, &desk_notes);
            exchange_in(&phone, &folder, &key, NOW + (at + 1) * MINUTE, &phone_notes);
        }
        let third = list.third.lines().find(|l| l.contains('"')).unwrap();
        for notes in [&desk_notes, &phone_notes] {
            let text = std::fs::read_to_string(place(notes, list)).unwrap_or_default();
            assert!(holds_both(notes, list) && text.contains(third), "{}: whole on {}: {text}", list.file, notes.display());
        }
    }
}

/// A list's name in the records.
fn record_of(file: &str) -> String {
    match file {
        "sioul-cases.toml" => "notes/sioul-cases.toml".into(),
        "letters/letters.toml" => LETTERS_LIST.into(),
        other => format!("notes/{other}"),
    }
}

/// A list caught of no bytes for an instant (a save in place between its
/// truncation and its writing): looked at again at the next exchange, never
/// rebuilt over what its editor writes then (review S34).
#[test]
fn a_list_blank_for_an_instant_is_looked_at_again() {
    let list = &ALL[2];
    let base = scratch("lists-instant");
    let (folder, key, desk, _phone, desk_notes, _phone_notes) = two_holding(&base, list);
    let path = place(&desk_notes, list);
    std::fs::write(&path, "").unwrap();
    let outcome = exchange_in(&desk, &folder, &key, NOW + 3 * MINUTE, &desk_notes);
    assert!(!outcome.problems.iter().any(|p| p.starts_with("share-list-back")), "{outcome:?}");
    let saved = format!("{}{}", list.text, list.third);
    put(&path, &saved);
    exchange_in(&desk, &folder, &key, NOW + 4 * MINUTE, &desk_notes);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), saved, "the editor's save stays as written");
}

/// The supplies' movements (docs/supplies.md): each device's own file
/// reaches the other whole, the two read together; nothing of them, nor of
/// the list, travels while another device's entry does not say its Sioul
/// has them (an older one would take them for notes).
#[test]
fn each_device_s_movements_of_supplies_travel_once_every_device_has_them() {
    let base = scratch("supply-moves");
    let folder = base.join("Sync").join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(base.as_path(), "desk"), Computer::new(base.as_path(), "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    std::fs::create_dir_all(&phone_notes).unwrap();
    let moves = |notes: &Path, device: &str| notes.join(sioul_core::supplies::MOVES).join(sioul_core::supplies::device_file(device));
    put(&desk_notes.join(sioul_core::supplies::FILE), "[[supply]]\nid = \"stamps\"\nname = \"Stamps\"\n");
    put(&moves(&desk_notes, "desk"), "[[move]]\nid = \"c1\"\nat = 100\nsupply = \"stamps\"\nkind = \"count\"\nquantity = 12\n");
    // The phone says nothing of supplies yet (an older Sioul): nothing goes.
    says_every_store(&desk, &folder, &key);
    crate::devices::change(&crate::devices::own_path(&phone.roots.state), Some((&folder, &key)), |e| {
        e.id = "phone".into();
        e.kind = crate::devices::COMPUTER.into();
        e.working = true;
        e.format = FORMAT;
    })
    .unwrap();
    for at in [0, 1, 2] {
        exchange_in(&desk, &folder, &key, NOW + at * 2 * MINUTE, &desk_notes);
        exchange_in(&phone, &folder, &key, NOW + (at * 2 + 1) * MINUTE, &phone_notes);
    }
    assert!(!phone_notes.join(sioul_core::supplies::FILE).exists() && !moves(&phone_notes, "desk").exists(), "nothing while the phone does not have them");
    // Every device has them: the list and the desk's movements reach the phone, the phone's reach the desk.
    says_every_store(&phone, &folder, &key);
    for at in [10, 11, 12] {
        exchange_in(&desk, &folder, &key, NOW + at * 2 * MINUTE, &desk_notes);
        exchange_in(&phone, &folder, &key, NOW + (at * 2 + 1) * MINUTE, &phone_notes);
    }
    assert!(std::fs::read_to_string(phone_notes.join(sioul_core::supplies::FILE)).unwrap_or_default().contains("stamps"));
    assert!(std::fs::read_to_string(moves(&phone_notes, "desk")).unwrap_or_default().contains("c1"), "the desk's file, whole");
    put(&moves(&phone_notes, "phone"), "[[move]]\nid = \"u1\"\nat = 200\nsupply = \"stamps\"\nkind = \"used\"\nquantity = 1\n");
    for at in [20, 21, 22] {
        exchange_in(&phone, &folder, &key, NOW + at * 2 * MINUTE, &phone_notes);
        exchange_in(&desk, &folder, &key, NOW + (at * 2 + 1) * MINUTE, &desk_notes);
    }
    for notes in [&desk_notes, &phone_notes] {
        let read = sioul_core::supplies::read_moves(notes, "desk");
        let latest = sioul_core::supplies::latest(&read.lines);
        assert_eq!(sioul_core::supplies::stock_of(&sioul_core::supplies::lines_of(&latest, "stamps")).text(), "11", "both files read together on {}", notes.display());
    }
    // Neither is a note.
    let notes_records: Vec<String> = stores_of(&Config { notes_root: Some(desk_notes.display().to_string()), ..Config::default() }, &desk.roots, &|_| true).into_iter().filter(|s| s.name == "files/notes/").flat_map(|s| s.skip.iter().map(|n| n.to_string()).collect::<Vec<_>>()).collect();
    assert!(notes_records.iter().any(|n| n == sioul_core::supplies::FILE) && notes_records.iter().any(|n| n == sioul_core::supplies::MOVES), "{notes_records:?}");
}
