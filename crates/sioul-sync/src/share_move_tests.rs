// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sioul's lists leaving the notes folder, device by device
//! (`sioul_core::storage`, docs/notes-folder.md, "What lives where"): the
//! gate (`storage_stand`), two devices at different steps, three with an
//! older build, one offline through the move, a notes sync carrying the old
//! files' removal, and the sharing between two devices moved. Fiction only.

use super::lists_tests::put;
use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;
use sioul_core::storage::{Folders, Storage};

/// The parts every list, paper and letter travels in.
const PARTS: &[&str] = &["projects", "papers", "notes"];

/// A device of the test: its Sioul's folders, its notes folder, and the
/// papers' and letters' folders it chose.
struct Device {
    c: Computer,
    notes: PathBuf,
    papers: PathBuf,
    letters: PathBuf,
}

impl Device {
    fn new(base: &Path, name: &str) -> Device {
        let c = Computer::new(base, name);
        let root = base.join(name);
        Device { notes: root.join("Notes"), papers: root.join("Documents").join("Papers"), letters: root.join("Documents").join("Letters"), c }
    }

    fn records(&self) -> PathBuf {
        self.c.roots.data.join(sioul_core::storage::RECORDS)
    }

    fn storage(&self) -> Storage {
        Storage::read(&self.c.roots.state)
    }

    fn config(&self) -> Config {
        let mut config = Config { notes_root: Some(self.notes.display().to_string()), storage: self.storage(), ..Config::default() };
        config.papers.folder = Some(self.papers.display().to_string());
        config.letters.folder = Some(self.letters.display().to_string());
        config
    }

    /// Where its stores read now (`Config::places`, with its own data folder).
    fn places(&self) -> sioul_core::storage::Places {
        if self.storage().is_moved() { sioul_core::storage::Places::apart(&self.records(), Some(&self.notes), &self.papers, &self.letters) } else { sioul_core::storage::Places::under(&self.notes) }
    }

    /// Where its lists are now.
    fn lists(&self) -> PathBuf {
        if self.storage().is_moved() { self.records() } else { self.notes.clone() }
    }

    fn exchange(&self, folder: &Path, key: &[u8; 32], now: i64) -> Outcome {
        let config = self.config();
        let stores = stores_of(&config, &self.c.roots, &|part| PARTS.contains(&part));
        exchange(&Sharing { folder, computer: &self.c.id, key, memory: &self.c.memory, files: true, hurry: None }, &stores, now).unwrap()
    }

    /// Its sealed entry, written at the tests' clock: every store a version
    /// added, and where its lists stand.
    fn says(&self, folder: &Path, key: &[u8; 32], storage: u32) {
        let id = self.c.id.clone();
        crate::devices::change_at(&crate::devices::own_path(&self.c.roots.state), Some((folder, key)), NOW / 1000, |e| {
            e.id = id;
            e.kind = crate::devices::COMPUTER.into();
            e.working = true;
            e.format = FORMAT;
            e.movements = MOVEMENTS_SHARED;
            e.clients = CLIENTS_SHARED;
            e.persons = PERSONS_SHARED;
            e.storage = storage;
        })
        .unwrap();
    }

    /// The move, as the window makes it: under the exchanges' lock; then its entry says so.
    fn moves(&self, folder: &Path, key: &[u8; 32], now: i64) {
        let folders = Folders { notes: self.notes.clone(), records: self.records(), papers: self.papers.clone(), letters: self.letters.clone(), kept: self.c.roots.data.join(sioul_core::storage::KEPT), state: self.c.roots.state.clone() };
        sioul_core::storage::move_now(&folders, now / 1000, || exchange_lock(&self.c.memory, true)).unwrap();
        self.says(folder, key, crate::devices::STORAGE_MOVED);
    }

    fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.lists().join(file)).unwrap_or_default()
    }

    fn stand(&self, folder: &Path, key: &[u8; 32], now: i64) -> StorageStand {
        storage_stand(folder, key, &self.c.memory, &self.c.id, now)
    }
}

/// A desk holding lists, papers and letters, and a phone with a notes
/// folder of its own, both sharing them, both knowing the new places.
fn desk_and_phone(base: &Path) -> (PathBuf, [u8; 32], Device, Device) {
    let folder = base.join("Sync").join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Device::new(base, "desk"), Device::new(base, "phone"));
    std::fs::create_dir_all(&phone.notes).unwrap();
    for d in [&desk, &phone] {
        d.says(&folder, &key, crate::devices::STORAGE_KNOWS);
    }
    put(&desk.notes.join("sioul-projects.toml"), "[[project]]\nid = \"taxes\"\ntitle = \"Taxes\"\nfiles = [\"admin/taxes.md\"]\n");
    put(&desk.notes.join("admin").join("taxes.md"), "# Taxes\n");
    put(&desk.notes.join("sioul-budgets.toml"), "[[budget]]\nid = \"home\"\ntitle = \"Home\"\nperiod = \"month\"\ntarget = 0\n\n[[line]]\nid = \"rent\"\nbudget = \"home\"\ndate = \"2026-10-01\"\namount = -620.0\nlabel = \"Rent\"\nlinks = [\"mid:rent@landlord.example.org\"]\n");
    put(&desk.notes.join("sioul-papers.toml"), "[[paper]]\nid = \"passport\"\nkind = \"passport\"\ntitle = \"Passport\"\nfile = \"papers/Passport.pdf\"\n");
    put(&desk.notes.join("papers").join("Passport.pdf"), "%PDF passport");
    put(&desk.notes.join("letters").join("letters.toml"), "[[letter]]\nid = \"caf\"\nfile = \"letters/2026/caf.pdf\"\nkind = \"other\"\nstatus = \"new\"\n");
    put(&desk.notes.join("letters").join("caf.txt"), "Madame,\n");
    put(&desk.notes.join("letters").join("2026").join("caf.pdf"), "%PDF caf");
    desk.exchange(&folder, &key, NOW);
    phone.exchange(&folder, &key, NOW + MINUTE);
    desk.exchange(&folder, &key, NOW + 2 * MINUTE);
    assert!(phone.read("sioul-budgets.toml").contains("Rent") && phone.read("sioul-papers.toml").contains("passport"));
    assert!(phone.notes.join("papers").join("Passport.pdf").is_file() && phone.notes.join("letters").join("2026").join("caf.pdf").is_file());
    (folder, key, desk, phone)
}

/// The desk moved, the phone not yet: their lists stay in step through the
/// sharing, each reading its own places; a change made on the desk just
/// before its move, never sent, goes out after it (the move is no store
/// new here to rebuild from the records: `Store::was`).
#[test]
fn two_devices_at_different_steps_stay_in_step() {
    let base = scratch("move-steps");
    let (folder, key, desk, phone) = desk_and_phone(&base);
    // Changed on the desk, not sent yet, then moved.
    let ledger = desk.notes.join("sioul-budgets.toml");
    put(&ledger, &format!("{}\n[[line]]\nid = \"bread\"\nbudget = \"home\"\ndate = \"2026-10-02\"\namount = -2.5\nlabel = \"Bread\"\n", std::fs::read_to_string(&ledger).unwrap()));
    assert!(desk.stand(&folder, &key, NOW + 3 * MINUTE).open, "every device knows the new places");
    desk.moves(&folder, &key, NOW + 3 * MINUTE);
    // The lists whose paths the move named are dated then; the tests' files are dated a minute back (`put`).
    for named in [desk.records().join("sioul-papers.toml"), desk.records().join("letters").join("letters.toml")] {
        let back = std::time::SystemTime::now() - std::time::Duration::from_secs(120);
        std::fs::File::options().write(true).open(&named).unwrap().set_modified(back).unwrap();
    }
    let outcome = desk.exchange(&folder, &key, NOW + 4 * MINUTE);
    assert!(!outcome.problems.iter().any(|p| p.starts_with("share-list-back")), "{outcome:?}");
    assert!(desk.read("sioul-budgets.toml").contains("Bread"), "the change made before the move kept: {}", desk.read("sioul-budgets.toml"));
    phone.exchange(&folder, &key, NOW + 5 * MINUTE);
    assert!(phone.read("sioul-budgets.toml").contains("Bread"), "{}", phone.read("sioul-budgets.toml"));
    // Each way, while at different steps.
    let on_phone = phone.notes.join("sioul-budgets.toml");
    put(&on_phone, &format!("{}\n[[budget]]\nid = \"car\"\ntitle = \"Car\"\n", std::fs::read_to_string(&on_phone).unwrap()));
    put(&desk.records().join("sioul-contracts.toml"), "[[contract]]\nid = \"power\"\nparty = \"Power\"\n");
    phone.exchange(&folder, &key, NOW + 6 * MINUTE);
    desk.exchange(&folder, &key, NOW + 7 * MINUTE);
    phone.exchange(&folder, &key, NOW + 8 * MINUTE);
    assert!(desk.read("sioul-budgets.toml").contains("\"car\"") && phone.read("sioul-contracts.toml").contains("power"));
    // The desk's lists in its data folder alone; its notes folder's left as they were.
    assert!(desk.records().join("sioul-budgets.toml").is_file());
    assert!(!std::fs::read_to_string(desk.notes.join("sioul-budgets.toml")).unwrap().contains("\"car\""), "the old file is no longer written");
    // A paper's file named from its folder on the desk, read on the phone in its notes folder's layout.
    let phone_places = phone.places();
    let wallet = sioul_core::papers::Wallet::load_at(&phone_places).unwrap();
    assert_eq!(wallet.get("passport").unwrap().file, "Passport.pdf", "the desk's word came");
    assert!(wallet.file_path(wallet.get("passport").unwrap()).unwrap().is_file());
    // A paper added on the desk reaches the phone's papers/ by the same record.
    put(&desk.papers.join("Card.png"), "card");
    desk.exchange(&folder, &key, NOW + 9 * MINUTE);
    phone.exchange(&folder, &key, NOW + 10 * MINUTE);
    assert!(phone.notes.join("papers").join("Card.png").is_file());
    // The letters' list and texts come; a scan of the desk's waits for the phone's move.
    let letters = desk.records().join("letters").join("letters.toml");
    put(&letters, &format!("{}\n[[letter]]\nid = \"tax\"\nfile = \"2026/tax.pdf\"\nkind = \"other\"\nstatus = \"new\"\n", std::fs::read_to_string(&letters).unwrap()));
    put(&desk.records().join("letters").join("tax.txt"), "Monsieur,\n");
    put(&desk.letters.join("2026").join("tax.pdf"), "%PDF tax");
    desk.exchange(&folder, &key, NOW + 11 * MINUTE);
    phone.exchange(&folder, &key, NOW + 12 * MINUTE);
    assert!(phone.read("letters/letters.toml").contains("\"tax\"") && phone.notes.join("letters").join("tax.txt").is_file());
    assert!(!desk.notes.join("letters").join("2026").join("tax.pdf").exists() && !desk.records().join("letters").join("2026").exists(), "never a scan among the records");
    // Then the phone moves: every scan on both.
    phone.moves(&folder, &key, NOW + 13 * MINUTE);
    for at in 14..18 {
        phone.exchange(&folder, &key, NOW + at * MINUTE);
        desk.exchange(&folder, &key, NOW + (at + 1) * MINUTE);
    }
    for d in [&desk, &phone] {
        assert!(d.letters.join("2026").join("caf.pdf").is_file() && d.letters.join("2026").join("tax.pdf").is_file(), "{}", d.letters.display());
        assert!(d.records().join("letters").join("tax.txt").is_file() && d.records().join("letters").join("caf.txt").is_file());
        assert!(d.read("sioul-budgets.toml").contains("Bread") && d.read("sioul-budgets.toml").contains("\"car\""));
        let places = d.places();
        let letters = sioul_core::letters::Letters::load_at(&places).unwrap();
        assert!(letters.list.iter().all(|l| letters.file_path(l).is_file()), "{}", d.read("letters/letters.toml"));
        let projects = sioul_core::projects::ProjectStore::open(&places).unwrap();
        assert!(projects.get("taxes").is_some());
    }
}

/// Three devices, one on an older Sioul: nothing may move while it counts;
/// forgotten, or silent past the days a device counts, it holds nobody.
#[test]
fn the_gate_holds_while_an_older_build_counts() {
    let base = scratch("move-gate");
    let (folder, key, desk, phone) = desk_and_phone(&base);
    let laptop = Device::new(&base, "laptop");
    std::fs::create_dir_all(&laptop.notes).unwrap();
    laptop.says(&folder, &key, 0);
    laptop.exchange(&folder, &key, NOW + 3 * MINUTE);
    let stand = desk.stand(&folder, &key, NOW + 4 * MINUTE);
    assert!(!stand.open && !stand.alone, "{stand:?}");
    assert_eq!(stand.waiting.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), [laptop.c.id.as_str()]);
    assert_eq!(stand.unmoved.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), [phone.c.id.as_str()]);
    // Updated: it knows.
    laptop.says(&folder, &key, crate::devices::STORAGE_KNOWS);
    assert!(desk.stand(&folder, &key, NOW + 5 * MINUTE).open);
    laptop.says(&folder, &key, 0);
    assert!(!desk.stand(&folder, &key, NOW + 5 * MINUTE).open);
    // Forgotten here: it holds nothing.
    forget_device(&folder, &key, &desk.c.memory, &desk.c.id, &laptop.c.id).unwrap();
    assert!(desk.stand(&folder, &key, NOW + 6 * MINUTE).open);
    // Not forgotten on the phone: it still waits there.
    phone.exchange(&folder, &key, NOW + 7 * MINUTE);
    assert!(!phone.stand(&folder, &key, NOW + 7 * MINUTE).open);
}

/// A phone offline through the whole move of the desk: back, it reads what
/// the desk changed since into its own old places, then moves in its turn.
#[test]
fn a_device_offline_through_the_move_catches_up() {
    let base = scratch("move-offline");
    let (folder, key, desk, phone) = desk_and_phone(&base);
    desk.moves(&folder, &key, NOW + 3 * MINUTE);
    let ledger = desk.records().join("sioul-budgets.toml");
    for at in 4..10 {
        put(&ledger, &format!("{}\n[[budget]]\nid = \"b{at}\"\ntitle = \"B{at}\"\nperiod = \"month\"\ntarget = 0\n", std::fs::read_to_string(&ledger).unwrap()));
        desk.exchange(&folder, &key, NOW + at * MINUTE);
    }
    // Days later the phone is back, still in the old layout.
    let back = NOW + 3 * 24 * 60 * MINUTE;
    phone.exchange(&folder, &key, back);
    assert!((4..10).all(|at| phone.read("sioul-budgets.toml").contains(&format!("\"b{at}\""))), "{}", phone.read("sioul-budgets.toml"));
    phone.moves(&folder, &key, back + MINUTE);
    phone.exchange(&folder, &key, back + 2 * MINUTE);
    desk.exchange(&folder, &key, back + 3 * MINUTE);
    assert_eq!(sioul_core::budget::Ledger::load(&phone.records()).unwrap().budgets.len(), sioul_core::budget::Ledger::load(&desk.records()).unwrap().budgets.len());
    assert!(desk.stand(&folder, &key, back + 4 * MINUTE).all_moved);
}

/// The old files leave a notes folder only once every device moved
/// (`StorageStand::all_moved`): the desk moved and the phone not, nothing
/// may go. A notes sync carrying their removal to the phone all the same:
/// the phone's lists are written again whole from the records, nothing lost.
#[test]
fn a_removal_carried_to_a_device_not_moved_loses_nothing() {
    let base = scratch("move-removal");
    let (folder, key, desk, phone) = desk_and_phone(&base);
    desk.moves(&folder, &key, NOW + 3 * MINUTE);
    desk.exchange(&folder, &key, NOW + 4 * MINUTE);
    let stand = desk.stand(&folder, &key, NOW + 5 * MINUTE);
    assert!(stand.open && !stand.all_moved, "the phone has not moved: nothing leaves {stand:?}");
    // A sync app removes the phone's old lists all the same.
    for file in ["sioul-budgets.toml", "sioul-projects.toml", "sioul-papers.toml"] {
        std::fs::remove_file(phone.notes.join(file)).unwrap();
    }
    std::fs::remove_file(phone.notes.join("letters").join("letters.toml")).unwrap();
    let outcome = phone.exchange(&folder, &key, NOW + 6 * MINUTE);
    assert!(outcome.problems.iter().any(|p| p.starts_with("share-list-back")), "{outcome:?}");
    for at in 7..9 {
        phone.exchange(&folder, &key, NOW + at * MINUTE);
        desk.exchange(&folder, &key, NOW + at * MINUTE + 30_000);
    }
    assert!(phone.read("sioul-budgets.toml").contains("Rent") && phone.read("sioul-projects.toml").contains("taxes") && phone.read("sioul-papers.toml").contains("passport"));
    assert!(phone.read("letters/letters.toml").contains("caf"));
    assert!(desk.read("sioul-budgets.toml").contains("Rent") && desk.read("sioul-projects.toml").contains("taxes"), "nothing left the desk");
    // Every device moved: then the old files may go, from the device all agree on.
    phone.moves(&folder, &key, NOW + 10 * MINUTE);
    let (on_desk, on_phone) = (desk.stand(&folder, &key, NOW + 11 * MINUTE), phone.stand(&folder, &key, NOW + 11 * MINUTE));
    assert!(on_desk.all_moved && on_phone.all_moved);
    assert!(on_desk.cleaner != on_phone.cleaner, "one of them takes the old files out");
}

/// Two devices moved: their lists, papers and letters travel between their
/// new places; a list of no bytes or gone there is written again whole.
#[test]
fn two_moved_devices_share_everything() {
    let base = scratch("move-both");
    let (folder, key, desk, phone) = desk_and_phone(&base);
    desk.moves(&folder, &key, NOW + 3 * MINUTE);
    phone.moves(&folder, &key, NOW + 3 * MINUTE + 1000);
    for at in 4..6 {
        desk.exchange(&folder, &key, NOW + at * MINUTE);
        phone.exchange(&folder, &key, NOW + at * MINUTE + 30_000);
    }
    let ledger = phone.records().join("sioul-budgets.toml");
    put(&ledger, &format!("{}\n[[budget]]\nid = \"cat\"\ntitle = \"Cat\"\n", std::fs::read_to_string(&ledger).unwrap()));
    put(&phone.papers.join("Lease.pdf"), "%PDF lease");
    put(&phone.letters.join("2026").join("bank.pdf"), "%PDF bank");
    phone.exchange(&folder, &key, NOW + 7 * MINUTE);
    desk.exchange(&folder, &key, NOW + 8 * MINUTE);
    assert!(desk.read("sioul-budgets.toml").contains("\"cat\""));
    assert!(desk.papers.join("Lease.pdf").is_file() && desk.letters.join("2026").join("bank.pdf").is_file());
    // A list of no bytes, then one gone, on the desk: back whole, nothing gone from the phone.
    for removed in [false, true] {
        let path = desk.records().join("sioul-budgets.toml");
        if removed {
            std::fs::remove_file(&path).unwrap();
        } else {
            put(&path, "");
        }
        let outcome = desk.exchange(&folder, &key, NOW + if removed { 20 } else { 10 } * MINUTE);
        assert!(outcome.problems.iter().any(|p| p.starts_with("share-list-back")), "{outcome:?}");
        desk.exchange(&folder, &key, NOW + if removed { 35 } else { 25 } * MINUTE);
        phone.exchange(&folder, &key, NOW + if removed { 36 } else { 26 } * MINUTE);
        for d in [&desk, &phone] {
            assert!(d.read("sioul-budgets.toml").contains("\"cat\"") && d.read("sioul-budgets.toml").contains("Rent"), "{removed}: {}", d.read("sioul-budgets.toml"));
        }
    }
    // Nothing of Sioul's written in either notes folder since the move.
    for d in [&desk, &phone] {
        assert!(!d.notes.join("sioul-budgets.toml").exists() || !std::fs::read_to_string(d.notes.join("sioul-budgets.toml")).unwrap().contains("\"cat\""));
    }
}
