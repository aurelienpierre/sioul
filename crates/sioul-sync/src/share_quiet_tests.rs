// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A phone's background step finds nothing new most of the time: what an
//! exchange then reads, does and writes (docs/android.md, "Its cost"). An
//! exchange with nothing to do (`quiet`), the memory and the logs of lines
//! kept between exchanges (`Kept`, `LINES`), this device's notes restated
//! rarely while it is put away (`write_seen`), no full round during a bulk
//! import (`set_importing`), the folder's leftovers cleaned once a day.
//! Fiction only.

use super::format_tests::announce;
use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;

const HOUR_MS: i64 = 60 * MINUTE;

/// An exchange as a phone's background step runs it: notes and papers not read.
fn background(c: &Computer, folder: &Path, key: &[u8; 32], now: i64) -> Outcome {
    // Two milliseconds apart at least: what an exchange wrote is never "changed a moment ago" for the next.
    std::thread::sleep(std::time::Duration::from_millis(2));
    exchange(&Sharing { folder, computer: &c.id, key, memory: &c.memory, files: false, hurry: None }, &stores(&Config::default(), &c.roots), now).unwrap()
}

fn quieted() -> u32 {
    QUIETED.with(std::cell::Cell::get)
}

fn lines_read() -> u32 {
    LINES_READ.with(std::cell::Cell::get)
}

/// A device's own entry changed (`devices::change`), as its window or its receivers do.
fn entry(c: &Computer, folder: &Path, key: &[u8; 32], f: impl FnOnce(&mut crate::devices::Entry)) {
    let id = c.id.clone();
    crate::devices::change(&crate::devices::own_path(&c.roots.state), Some((folder, key)), |e| {
        e.id = id;
        e.kind = crate::devices::COMPUTER.into();
        e.doses = true;
        f(e);
    })
    .unwrap();
}

/// Its memory's two files, as they stand.
fn memory_files(c: &Computer) -> (Option<FileStamp>, Option<FileStamp>) {
    (stamp_at(&c.memory), stamp_at(&sealed_path(&c.memory)))
}

fn dated(path: &Path, ago: std::time::Duration) {
    std::fs::File::options().write(true).open(path).unwrap().set_modified(std::time::SystemTime::now() - ago).unwrap();
}

/// Nothing changed anywhere since an exchange that left everything settled:
/// the next reads nothing, writes nothing and says what a whole one would.
/// Each thing it reads, changed, makes the next one whole again; what only
/// tells the time (another device's entry restating its exchanges, its notes)
/// does not.
#[test]
fn an_exchange_with_nothing_new_reads_nothing_and_says_the_same() {
    let base = scratch("quiet");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/safe-senders.txt", "a@example.org\n");
    desk.write("state/health-state.toml", "[taken]\n\"a@100\" = 100\n");
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| e.start(now / 1000));
    background(&desk, &folder, &key, now);
    now += MINUTE;
    background(&phone, &folder, &key, now);
    assert!(phone.read("state/health-state.toml").contains("a@100"));
    // The files it wrote, read once more: then settled.
    now += MINUTE;
    let settled = background(&phone, &folder, &key, now);
    assert_eq!((settled.sent, settled.received), (0, 0), "{settled:?}");
    let (before, kept) = (quieted(), memory_files(&phone));
    let quiet = |now: i64| {
        let n = quieted();
        let outcome = background(&phone, &folder, &key, now);
        (quieted() > n, outcome)
    };
    now += 2 * MINUTE;
    let (was, idle) = quiet(now);
    assert!(was && quieted() == before + 1 && idle.quiet && !settled.quiet, "nothing new: nothing done");
    assert_eq!((idle.sent, idle.received, idle.pending, idle.wrote, idle.problems.len(), idle.written.len()), (0, 0, settled.pending, settled.wrote, 0, 0));
    assert!(idle.looked > 0, "its files looked at now: everything captured here is in its records");
    assert_eq!(memory_files(&phone), kept, "its memory not written");
    // The desk in use restates its entry at each of its exchanges: only times, nothing to do.
    entry(&desk, &folder, &key, |e| e.exported(now, None, now / 1000));
    now += 2 * MINUTE;
    assert!(quiet(now).0, "only the desk's exchanges' times changed");
    // Its notes, restated (their time alone): nothing to do either.
    let notes = seen_path(&folder, &desk.id);
    let text = std::fs::read_to_string(&notes).unwrap();
    std::fs::write(&notes, text.replace("at = ", "at = 1")).unwrap();
    now += 2 * MINUTE;
    assert!(quiet(now).0, "another device's notes alone");
    // The desk closes: what its entry says changed, read whole.
    entry(&desk, &folder, &key, |e| e.close(now / 1000));
    now += 2 * MINUTE;
    assert!(!quiet(now).0, "the desk's session changed");
    now += 2 * MINUTE;
    assert!(quiet(now).0);
    // A change from the desk: read, written here.
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
    background(&desk, &folder, &key, now);
    now += MINUTE;
    let (was, came) = quiet(now);
    assert!(!was && came.received == 1, "{came:?}");
    assert!(phone.read("config/safe-senders.txt").contains("b@example.org"));
    now += MINUTE;
    background(&phone, &folder, &key, now);
    // A change here: sent.
    phone.write("state/health-state.toml", "[taken]\n\"a@100\" = 100\n\"b@200\" = 200\n");
    now += MINUTE;
    let (was, went) = quiet(now);
    assert!(!was && went.sent == 1, "{went:?}");
    now += MINUTE;
    background(&phone, &folder, &key, now);
    now += MINUTE;
    assert!(quiet(now).0);
    // An hour on: the hour's tidying, then nothing to do again.
    now += HOUR_MS;
    assert!(!quiet(now).0, "the hour's tidying runs");
    now += MINUTE;
    assert!(quiet(now).0);
    // Full rounds told to wait (a slow connection): read whole once.
    set_frugal(&phone.memory, true);
    now += MINUTE;
    assert!(!quiet(now).0, "the rounds' rule changed");
    set_frugal(&phone.memory, false);
    now += MINUTE;
    assert!(!quiet(now).0);
    // This device's own file cut short by a sync app: never passed over.
    now += MINUTE;
    assert!(quiet(now).0);
    let own = round_file(&folder, &phone.id, Memory::load(&phone.memory, &phone.id).round);
    let bytes = std::fs::read(&own).unwrap();
    std::fs::write(&own, &bytes[..bytes.len() / 2]).unwrap();
    now += MINUTE;
    let (was, cut) = quiet(now);
    assert!(!was && cut.problems.iter().any(|p| p == "share-own-cut"), "{cut:?}");
    now += MINUTE;
    background(&phone, &folder, &key, now);
    now += MINUTE;
    assert!(quiet(now).0);
    // Its memory written by another process meanwhile: read again, never the one kept here.
    let mut text: serde_json::Value = serde_json::from_slice(&std::fs::read(&phone.memory).unwrap()).unwrap();
    text["broken"] = serde_json::json!({ "someone-else": 5 });
    write_synced(&phone.memory, text.to_string().as_bytes()).unwrap();
    now += MINUTE;
    assert!(!quiet(now).0, "its memory changed elsewhere");
    assert_eq!(heard(&phone.memory, &phone.id).broken.get("someone-else"), Some(&5), "what the other process wrote, kept");
    // A clock gone back: whole.
    assert!(!quiet(now - 10 * MINUTE).0);
    let _ = std::fs::remove_dir_all(&base);
}

/// The memory kept between exchanges in a process (`Kept`) is taken only
/// while its two files are as that process wrote them: written elsewhere
/// (the window's process, a phone's background service), they are read
/// again. What a send asks of them (`own_sealed`) is the same either way.
#[test]
fn the_memory_kept_between_exchanges_is_never_older_than_its_files() {
    let base = scratch("kept");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    desk.write("config/safe-senders.txt", "a@example.org\n");
    background(&desk, &folder, &key, NOW);
    background(&phone, &folder, &key, NOW + MINUTE);
    let parsed = |c: &Computer| -> BTreeMap<String, u64> {
        let sealed: Sealed = serde_json::from_slice(&std::fs::read(sealed_path(&c.memory)).unwrap()).unwrap();
        sealed.mine.into_iter().map(|(name, (_, size))| (name, size)).collect()
    };
    assert!(KEPT.lock().unwrap().contains_key(&phone.memory), "kept after its exchange");
    assert_eq!(own_sealed(&phone.memory, &phone.id), parsed(&phone));
    // Another process seals a file of this device's and says so in `files.json`.
    let mut sealed: Sealed = serde_json::from_slice(&std::fs::read(sealed_path(&phone.memory)).unwrap()).unwrap();
    sealed.mine.insert("a-sealed-file".into(), (NOW + MINUTE, 42));
    write_synced(&sealed_path(&phone.memory), serde_json::to_string(&sealed).unwrap().as_bytes()).unwrap();
    assert_eq!(own_sealed(&phone.memory, &phone.id).get("a-sealed-file"), Some(&42), "read from the file, not the memory kept");
    assert_eq!(own_sealed(&phone.memory, "another-device"), BTreeMap::new(), "another device's memory says nothing of this one's");
    // The next exchange reads both files again, and keeps what the other process wrote.
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
    background(&desk, &folder, &key, NOW + 2 * MINUTE);
    background(&phone, &folder, &key, NOW + 3 * MINUTE);
    assert_eq!(own_sealed(&phone.memory, &phone.id).get("a-sealed-file"), Some(&42));
    assert_eq!(own_sealed(&phone.memory, &phone.id), parsed(&phone), "from the memory kept: the same as the file says");
    // `memory.json` put back from a backup (its bytes differ): read again, its numbering goes on.
    let backup = std::fs::read(&phone.memory).unwrap();
    phone.write("state/health-state.toml", "[taken]\n\"c@300\" = 300\n");
    background(&phone, &folder, &key, NOW + 4 * MINUTE);
    std::fs::write(&phone.memory, &backup).unwrap();
    let outcome = background(&phone, &folder, &key, NOW + 5 * MINUTE);
    assert!(outcome.problems.iter().any(|p| p == "share-own-ahead"), "the memory put back is the one read: {outcome:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// A log of lines (the senders' lists here; on a phone, the texts' log of
/// megabytes) is read through once, then again only once it changed.
#[test]
fn a_log_of_lines_is_read_through_only_when_it_changed() {
    let base = scratch("lines");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let phone = Computer::new(&base, "phone");
    let list = phone.path("config/known-senders.txt");
    phone.write("config/known-senders.txt", "a@example.org\nb@example.org\n");
    dated(&list, std::time::Duration::from_secs(60));
    let first = lines_read();
    background(&phone, &folder, &key, NOW);
    assert_eq!(lines_read(), first + 1);
    // Another file changes: the exchange runs whole, the list is not read again.
    phone.write("state/health-state.toml", "[taken]\n\"a@100\" = 100\n");
    let outcome = background(&phone, &folder, &key, NOW + MINUTE);
    assert_eq!((outcome.sent, lines_read()), (1, first + 1), "{outcome:?}");
    // A line added: read again, sent.
    phone.write("config/known-senders.txt", "a@example.org\nb@example.org\nc@example.org\n");
    dated(&list, std::time::Duration::from_secs(50));
    let outcome = background(&phone, &folder, &key, NOW + 2 * MINUTE);
    assert_eq!((outcome.sent, lines_read()), (1, first + 2), "{outcome:?}");
    // Rewritten in place at the same size, at another time: read again, both changes sent.
    phone.write("config/known-senders.txt", "a@example.org\nb@example.org\nd@example.org\n");
    dated(&list, std::time::Duration::from_secs(40));
    let outcome = background(&phone, &folder, &key, NOW + 3 * MINUTE);
    assert_eq!((outcome.sent, lines_read()), (2, first + 3), "{outcome:?}");
    // Changed a moment before it was read (a coarse clock may hide a change at the same size and time): read again next time.
    let n = lines_read();
    phone.write("config/known-senders.txt", "a@example.org\nb@example.org\ne@example.org\n");
    SETTLE.with(|s| s.set(60_000));
    background(&phone, &folder, &key, NOW + 4 * MINUTE);
    phone.write("state/health-state.toml", "[taken]\n\"a@100\" = 100\n\"b@200\" = 200\n");
    background(&phone, &folder, &key, NOW + 5 * MINUTE);
    SETTLE.with(|s| s.set(0));
    assert_eq!(lines_read(), n + 2, "read within the settling time: read again");
    let _ = std::fs::remove_dir_all(&base);
}

/// This device's notes in the folder (how far it read the others) are
/// written when they change; restated each quarter of an hour while it is in
/// use, once a day while it is put away (a phone in the background).
#[test]
fn a_device_put_away_restates_its_notes_once_a_day() {
    let base = scratch("notes-daily");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let phone = Computer::new(&base, "phone");
    let notes = seen_path(&folder, &phone.id);
    let at = || read_seen_in(&folder, &phone.id).map(|s| s.at);
    let mut n = 0;
    // A change at each exchange, so that each runs whole.
    let mut exchange_at = |now: i64| {
        n += 1;
        phone.write("state/health-state.toml", &format!("[taken]\n\"a@{n}\" = {n}\n"));
        background(&phone, &folder, &key, now);
    };
    entry(&phone, &folder, &key, |e| {
        e.start(NOW / 1000);
        e.close(NOW / 1000);
    });
    exchange_at(NOW);
    assert_eq!(at(), Some(NOW / 1000), "written once");
    exchange_at(NOW + 20 * MINUTE);
    exchange_at(NOW + 23 * HOUR_MS);
    assert_eq!(at(), Some(NOW / 1000), "put away: not restated within the day");
    exchange_at(NOW + 25 * HOUR_MS);
    assert_eq!(at(), Some((NOW + 25 * HOUR_MS) / 1000), "a day on: restated");
    // In use: each quarter of an hour, as before.
    entry(&phone, &folder, &key, |e| e.start((NOW + 25 * HOUR_MS) / 1000));
    exchange_at(NOW + 25 * HOUR_MS + 10 * MINUTE);
    assert_eq!(at(), Some((NOW + 25 * HOUR_MS) / 1000));
    exchange_at(NOW + 25 * HOUR_MS + 16 * MINUTE);
    assert_eq!(at(), Some((NOW + 25 * HOUR_MS + 16 * MINUTE) / 1000), "in use: a quarter of an hour on, restated");
    assert!(notes.exists());
    let _ = std::fs::remove_dir_all(&base);
}

/// While a bulk import goes on (`set_importing`: a phone's first import of
/// its texts), no full round starts, past what a slow connection would
/// allow too; the first exchange after it opens one.
#[test]
fn no_full_round_starts_while_a_bulk_import_goes_on() {
    let base = scratch("importing");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let desk = Computer::new(&base, "desk");
    let full = || Memory::load(&desk.memory, &desk.id).full_round;
    let lines = |from: usize, count: usize| -> String { (from..from + count).map(|n| format!("imported-{n:06}@example.org\n")).collect() };
    set_importing(&desk.memory, true);
    let mut list = String::new();
    for batch in 0..4 {
        list.push_str(&lines(batch * 10_000, 10_000));
        desk.write("config/known-senders.txt", &list);
        background(&desk, &folder, &key, NOW + batch as i64 * MINUTE);
    }
    let grown = Memory::load(&desk.memory, &desk.id).grown;
    assert!(grown > FRUGAL_ROUND_SIZE && full() <= 1, "importing: no full round, past what a slow connection allows ({grown} bytes, round {})", full());
    set_importing(&desk.memory, false);
    list.push_str(&lines(40_000, 10));
    desk.write("config/known-senders.txt", &list);
    background(&desk, &folder, &key, NOW + 10 * MINUTE);
    assert!(full() > 1, "the import over: one full round");
    let _ = std::fs::remove_dir_all(&base);
}

/// What a crash left half written among the folder's sealed files is
/// cleaned once a day, not after each change received: listing `blobs/` reads
/// thousands of names.
#[test]
fn leftovers_among_the_sealed_files_are_cleaned_once_a_day() {
    let base = scratch("leftovers");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let blobs = folder.join("blobs");
    std::fs::create_dir_all(&blobs).unwrap();
    let leftover = |name: &str| {
        let path = blobs.join(format!(".{name}.1-1{TEMPORARY}"));
        std::fs::write(&path, b"half").unwrap();
        dated(&path, std::time::Duration::from_secs(7_200));
        path
    };
    let first = leftover("first");
    desk.write("config/safe-senders.txt", "a@example.org\n");
    background(&desk, &folder, &key, NOW);
    background(&phone, &folder, &key, NOW + MINUTE);
    assert!(!first.exists(), "the first exchange cleans");
    let second = leftover("second");
    // A change received two hours later (a copy kept in the history): not listed again yet.
    desk.write("config/safe-senders.txt", "a@example.org\nb@example.org\n");
    background(&desk, &folder, &key, NOW + 2 * HOUR_MS);
    assert_eq!(background(&phone, &folder, &key, NOW + 2 * HOUR_MS + MINUTE).received, 1);
    assert!(second.exists(), "within the day: left");
    background(&phone, &folder, &key, NOW + 25 * HOUR_MS);
    assert!(!second.exists(), "a day on: cleaned");
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------- against a whole exchange

const DESK: &str = "11111111-1111-4111-8111-111111111111";
const PHONE: &str = "22222222-2222-4222-8222-222222222222";
const DIFF_FILES: [&str; 3] = ["state/health-state.toml", "config/safe-senders.txt", "config/known-senders.txt"];

/// One of two identical worlds: a desk and a phone sharing a folder; in one, the quiet exchange allowed.
struct Twin {
    folder: PathBuf,
    key: [u8; 32],
    devices: [Computer; 2],
    quiet: bool,
}

impl Twin {
    fn new(base: &Path, name: &str, quiet: bool) -> Twin {
        let root = base.join(name);
        let folder = root.join("Sioul");
        let key = quick_key(&folder, "four words make a passphrase").unwrap();
        let mut desk = Computer::new(&root, "desk");
        desk.id = DESK.into();
        let mut phone = Computer::new(&root, "phone");
        phone.id = PHONE.into();
        Twin { folder, key, devices: [desk, phone], quiet }
    }

    fn exchange(&self, d: usize, files: bool, now: i64) -> Outcome {
        let c = &self.devices[d];
        if !self.quiet {
            QUIET.lock().unwrap().remove(&c.memory);
        }
        std::thread::sleep(std::time::Duration::from_millis(3));
        exchange(&Sharing { folder: &self.folder, computer: &c.id, key: &self.key, memory: &c.memory, files, hurry: None }, &stores(&Config::default(), &c.roots), now).unwrap()
    }

    fn session(&self, d: usize, up: bool, now: i64) {
        let c = &self.devices[d];
        let id = c.id.clone();
        crate::devices::change(&crate::devices::own_path(&c.roots.state), Some((&self.folder, &self.key)), |e| {
            e.id = id;
            e.kind = if d == 0 { crate::devices::COMPUTER.into() } else { crate::devices::PHONE.into() };
            e.doses = true;
            e.format = 2;
            if up { e.start(now / 1000) } else { e.close(now / 1000) }
        })
        .unwrap();
    }

    fn state(&self) -> Vec<String> {
        let mut out = Vec::new();
        for c in &self.devices {
            for f in DIFF_FILES {
                out.push(format!("{}:{f}={:?}", c.id, c.read(f)));
            }
            let m = Memory::load(&c.memory, &c.id);
            out.push(format!("{}: round {} seq {} format {} read_n {:?} newer {:?} waiting {}", c.id, m.round, m.seq, m.format, m.read_n, m.newer.keys().collect::<Vec<_>>(), m.waiting.len()));
        }
        out
    }
}

/// A small generator: the same script for both worlds, at each run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// One seed's script, `steps` long, in both worlds: what each did and holds, compared at every step.
fn run(seed: u64, steps: usize) -> (u32, usize) {
    let base = scratch(&format!("quiet-diff-{seed}"));
    let worlds = [Twin::new(&base, "a", true), Twin::new(&base, "b", false)];
    let mut rng = Rng(seed * 2_654_435_761 + 7);
    let mut now = NOW;
    let mut lines: [[Vec<String>; 3]; 2] = Default::default();
    let mut long_jumps = 0;
    let quiet_before = QUIETED.with(std::cell::Cell::get);
    let mut exchanges = 0;
    for w in &worlds {
        w.session(0, true, now);
        w.session(1, true, now);
    }
    for step in 0..steps {
        let action = rng.below(100);
        match action {
            // An edit on one device.
            0..=17 => {
                let d = rng.below(2) as usize;
                let f = rng.below(3) as usize;
                let shared_key = rng.below(5) == 0;
                let n = rng.below(40);
                let entry = if f == 0 { format!("\"{}@{n}\" = {n}", if shared_key { "both" } else if d == 0 { "desk" } else { "phone" }) } else { format!("{}{n}@example.org", if d == 0 { "d" } else { "p" }) };
                let list = &mut lines[d][f];
                if let Some(at) = list.iter().position(|l| *l == entry) {
                    list.remove(at);
                } else {
                    list.push(entry);
                }
                let text = if f == 0 { format!("[taken]\n{}\n", list.join("\n")) } else { format!("{}\n", list.join("\n")) };
                for w in &worlds {
                    w.devices[d].write(DIFF_FILES[f], &text);
                }
            }
            // Background exchanges (the quiet path), many.
            18..=69 => {
                let d = rng.below(2) as usize;
                let outs: Vec<Outcome> = worlds.iter().map(|w| w.exchange(d, false, now)).collect();
                exchanges += 1;
                let key = |o: &Outcome| (o.sent, o.received, o.pending, o.wrote, o.problems.clone(), o.written.clone());
                assert_eq!(key(&outs[0]), key(&outs[1]), "seed {seed} step {step}: background exchange of device {d} differs (quiet {})", outs[0].quiet);
                // What it read is now in the stores: the local lists follow what came in.
                for (f, file) in DIFF_FILES.iter().enumerate() {
                    let text = worlds[0].devices[d].read(file);
                    lines[d][f] = text.lines().map(str::trim).filter(|l| !l.is_empty() && *l != "[taken]").map(str::to_string).collect();
                }
            }
            // A window's exchange (notes and papers read: never quiet).
            70..=77 => {
                let d = rng.below(2) as usize;
                let outs: Vec<Outcome> = worlds.iter().map(|w| w.exchange(d, true, now)).collect();
                exchanges += 1;
                assert_eq!((outs[0].sent, outs[0].received, outs[0].wrote), (outs[1].sent, outs[1].received, outs[1].wrote), "seed {seed} step {step}: window exchange differs");
                for (f, file) in DIFF_FILES.iter().enumerate() {
                    let text = worlds[0].devices[d].read(file);
                    lines[d][f] = text.lines().map(str::trim).filter(|l| !l.is_empty() && *l != "[taken]").map(str::to_string).collect();
                }
            }
            // A session up or down.
            78..=83 => {
                let (d, up) = (rng.below(2) as usize, rng.below(2) == 0);
                for w in &worlds {
                    w.session(d, up, now);
                }
            }
            // A device says it reads format 2 (as announce does in the switch tests).
            84..=86 => {
                let d = rng.below(2) as usize;
                for w in &worlds {
                    announce(&w.folder, &w.key, &w.devices[d], 2, now);
                }
            }
            // Full rounds' rules.
            87..=89 => {
                let (d, on, which) = (rng.below(2) as usize, rng.below(2) == 0, rng.below(2));
                for w in &worlds {
                    if which == 0 { set_frugal(&w.devices[d].memory, on) } else { set_importing(&w.devices[d].memory, on) }
                }
            }
            // A sync app puts back a shorter copy of the other device's latest round (cut at a line), in both worlds.
            90..=91 => {
                let d = rng.below(2) as usize;
                let other = &worlds[0].devices[1 - d].id;
                let cut = rng.below(3);
                for w in &worlds {
                    let round = Memory::load(&w.devices[1 - d].memory, other).round.max(1);
                    let file = round_file(&w.folder, other, round);
                    if let Ok(bytes) = std::fs::read(&file) {
                        let ends: Vec<usize> = bytes.iter().enumerate().filter(|(_, b)| **b == b'\n').map(|(i, _)| i + 1).collect();
                        if ends.len() > 2 {
                            let keep = ends[ends.len() - 2 - (cut as usize).min(ends.len() - 3)];
                            std::fs::write(&file, &bytes[..keep]).unwrap();
                        }
                    }
                }
            }
            // Time.
            _ => {
                let jump = match rng.below(10) {
                    0 if long_jumps < 2 => {
                        long_jumps += 1;
                        25 * 60 * MINUTE
                    }
                    1..=2 => 61 * MINUTE,
                    _ => (1 + rng.below(5) as i64) * MINUTE,
                };
                now += jump;
            }
        }
        now += 1_000;
        let (a, b) = (worlds[0].state(), worlds[1].state());
        if a != b {
            for (x, y) in a.iter().zip(&b) {
                if x != y {
                    eprintln!("A: {x}\nB: {y}");
                }
            }
            panic!("seed {seed} step {step} (action {action}): the worlds differ");
        }
    }
    for w in &worlds {
        for c in &w.devices {
            set_frugal(&c.memory, false);
            set_importing(&c.memory, false);
        }
    }
    let quieted = QUIETED.with(std::cell::Cell::get) - quiet_before;
    let formats: Vec<u32> = worlds[0].devices.iter().map(|c| Memory::load(&c.memory, &c.id).format).collect();
    eprintln!("seed {seed}: formats {formats:?}, quiet {quieted} of {exchanges}");
    let _ = std::fs::remove_dir_all(&base);
    (quieted, exchanges)
}

/// The quiet exchange against a whole one (review of battery part B, F7):
/// two identical worlds driven by the same random script, edits on both
/// devices, background (`files: false`) and window exchanges, sessions up and
/// down, devices announcing format 2 (the switch included), full rounds held
/// back or not, time jumps of minutes, an hour and a day, a sync app cutting
/// the other device's latest round at a line. In one world the quiet exchange
/// is allowed, in the other never used: every outcome and every store the
/// same, at every step.
#[test]
fn a_quiet_exchange_is_the_same_as_a_whole_one_through_the_switch() {
    let (mut quiet, mut total) = (0, 0);
    for seed in 1..=12 {
        let (q, n) = run(seed, 300);
        quiet += q;
        total += n;
    }
    eprintln!("quiet against whole: {quiet} quiet exchanges in world A out of {total} exchanges, outcomes and stores the same");
    assert!(quiet > 20, "the quiet path ran too seldom to mean anything: {quiet}");
}

/// A leg as a step's time (the unified agenda's part 7): offered only once
/// every other device that counts says its Sioul keeps such a leg
/// (`devices::Entry::legs`); a phone on an older Sioul, whose entry says
/// nothing of it, keeps it from being offered; alone, nothing is harmed.
#[test]
fn a_leg_as_a_step_s_time_waits_for_every_device() {
    let base = scratch("legs-gate");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    entry(&desk, &folder, &key, |e| {
        e.start(NOW / 1000);
        e.legs = LEGS_AS_STEPS;
    });
    assert!(legs_as_steps(&folder, &key, &desk.memory, &desk.id, NOW), "no other device: none harmed");
    entry(&phone, &folder, &key, |e| e.start(NOW / 1000));
    assert!(!legs_as_steps(&folder, &key, &desk.memory, &desk.id, NOW + MINUTE), "the phone on an older Sioul");
    entry(&phone, &folder, &key, |e| e.legs = LEGS_AS_STEPS);
    assert!(legs_as_steps(&folder, &key, &desk.memory, &desk.id, NOW + 2 * MINUTE), "both keep such a leg");
}

/// Your own time around meetings (`data/own-time.toml`, the unified agenda's
/// part 4) beside a phone on 982d6e5, which has no such store: never offered
/// while the phone's entry does not say it has it, so the phone holds no
/// record it could not place and its background steps go quiet again (review
/// of part 4, M1); the desk's file kept to itself. Once the phone runs a build
/// that says so, the desk's next exchange sends it, once, and both read the
/// same own time.
#[test]
fn your_own_time_waits_for_a_device_that_has_its_store() {
    let base = scratch("own-time-gate");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.own_time = OWN_TIME_SHARED;
    });
    entry(&phone, &folder, &key, |e| e.start(now / 1000));
    desk.write("data/own-time.toml", "[event.\"board-1\"]\nbefore = 15\n\n[event.\"review-2\"]\nafter = 10\nremind = \"30\"\n");
    // The desk's own exchanges: its file kept to itself, nothing sent; quiet from its third
    // (re-check of 857c39e, §3.6: said before, never asserted).
    let mut desk_steps = Vec::new();
    for _ in 0..3 {
        let n = quieted();
        let outcome = background(&desk, &folder, &key, now);
        desk_steps.push((quieted() > n, outcome.sent));
        now += MINUTE;
    }
    assert!(desk_steps.iter().all(|(_, sent)| *sent == 0) && desk_steps[2].0, "{desk_steps:?}");
    // The phone as 982d6e5: no such store, none known.
    let older = stores(&Config::default(), &phone.roots).into_iter().filter(|s| s.name != OWN_TIME).collect::<Vec<_>>();
    super::tests::OLDER.with(|o| *o.borrow_mut() = vec![OWN_TIME.to_string()]);
    let older_step = |now: i64| {
        std::thread::sleep(std::time::Duration::from_millis(2));
        let n = quieted();
        let outcome = exchange(&Sharing { folder: &folder, computer: &phone.id, key: &key, memory: &phone.memory, files: false, hurry: None }, &older, now).unwrap();
        (quieted() > n, outcome)
    };
    // Its first steps read the desk's records and then its own once (as any device's: settled
    // at the second, `an_exchange_with_nothing_new_reads_nothing_and_says_the_same`); the next are quiet.
    let mut steps = Vec::new();
    for _ in 0..4 {
        now += MINUTE;
        steps.push(older_step(now));
    }
    assert!(steps.iter().all(|(_, o)| o.pending == 0), "nothing it cannot place: {steps:?}");
    assert!(steps[2].0 && steps[3].0, "its background steps quiet again: {steps:?}");
    assert!(!phone.path("data/own-time.toml").exists(), "nothing written there");
    // The desk's again, after the phone's: its file still kept to itself.
    now += MINUTE;
    assert_eq!(background(&desk, &folder, &key, now).sent, 0);
    // Settings says which device keeps it from travelling (§3.2).
    assert_eq!(formats(&folder, &key, &desk.memory, &desk.id, now).own_time, [phone.id.clone()]);
    // On the phone, which has none of its own: the desk says it reads own time, so it may hold some
    // to send there; the phone's Settings names itself nowhere (review of e5bd27b, later 1).
    let on_phone = formats(&folder, &key, &phone.memory, &phone.id, now);
    assert!(on_phone.others_share_own_time && on_phone.own_time.is_empty(), "{:?}", on_phone.own_time);
    // The phone updated: its entry says it has the store; the desk's next exchange sends it, once.
    super::tests::OLDER.with(|o| o.borrow_mut().clear());
    entry(&phone, &folder, &key, |e| e.own_time = OWN_TIME_SHARED);
    now += MINUTE;
    let sent = background(&desk, &folder, &key, now);
    now += MINUTE;
    let came = background(&phone, &folder, &key, now);
    assert!(came.written.contains(OWN_TIME), "{sent:?} {came:?}");
    assert_eq!(phone.read("data/own-time.toml").parse::<toml::Table>().unwrap(), desk.read("data/own-time.toml").parse::<toml::Table>().unwrap(), "the same own time on both");
    now += MINUTE;
    let again = background(&desk, &folder, &key, now);
    assert_eq!(again.sent, 0, "sent once: {again:?}");
    assert!(formats(&folder, &key, &desk.memory, &desk.id, now).own_time.is_empty(), "nothing held back any more");
    let _ = std::fs::remove_dir_all(&base);
}

/// Sioul's own lines of meetings with others (`data/meeting-lines.toml`:
/// who takes a child's event, its kind) beside a phone whose entry does not
/// say it has the store in the sharing, as an older Sioul's never does:
/// neither sent nor read, so the older phone holds no record of it and writes
/// none over it, and each device knows what it knows. Once the phone says
/// so, the desk's next exchange sends every entry, a time's lines and an entry
/// of no line included, as the desk's own writers wrote them. An entry taken
/// out (its lines back in its file) travels as a removal, and does not come
/// back.
#[test]
fn a_meeting_s_own_lines_wait_for_a_device_that_has_their_store_then_travel() {
    use sioul_core::meeting_lines as lines;
    let base = scratch("meeting-lines-gate");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.meeting_lines = MEETING_LINES_SHARED;
    });
    entry(&phone, &folder, &key, |e| e.start(now / 1000));
    let (on_desk, on_phone) = (desk.path(MEETING_LINES), phone.path(MEETING_LINES));
    let kind = "X-SIOUL-CONCEPT:tag:aurelienpierre.com,2026:sioul/child-kind/health".to_string();
    let held = "X-SIOUL-TAKEN-BY;CN=Alex;PARTSTAT=ACCEPTED:mailto:alex@example.org".to_string();
    let that_time = "RECURRENCE-ID;TZID=Europe/Paris:20991117T090000".to_string();
    lines::add(&on_desk, "psy.1@example.org", &[(String::new(), kind.clone()), (String::new(), held.clone()), (that_time.clone(), held.clone())]).unwrap();
    lines::ensure(&on_desk, "known-empty").unwrap();
    assert_eq!(stores(&Config::default(), &desk.roots).iter().find(|s| s.name == MEETING_LINES).map(|s| (s.part, matches!(s.shape, Shape::Files))), Some(("time", false)), "a TOML file of the part \"time\": no sealed file for the sweeps (`FILE_PARTS`)");
    for _ in 0..2 {
        assert_eq!(background(&desk, &folder, &key, now).sent, 0, "kept to itself");
        now += MINUTE;
    }
    // The phone as an older Sioul: no such store, none known. It places all it reads, and writes nothing there.
    let older = stores(&Config::default(), &phone.roots).into_iter().filter(|s| s.name != MEETING_LINES).collect::<Vec<_>>();
    super::tests::OLDER.with(|o| *o.borrow_mut() = vec![MEETING_LINES.to_string()]);
    for _ in 0..3 {
        now += MINUTE;
        std::thread::sleep(std::time::Duration::from_millis(2));
        let outcome = exchange(&Sharing { folder: &folder, computer: &phone.id, key: &key, memory: &phone.memory, files: false, hurry: None }, &older, now).unwrap();
        assert_eq!(outcome.pending, 0, "nothing it cannot place: {outcome:?}");
    }
    assert!(!on_phone.exists(), "nothing written there");
    // The phone updated: the whole file comes, as it was written.
    super::tests::OLDER.with(|o| o.borrow_mut().clear());
    entry(&phone, &folder, &key, |e| e.meeting_lines = MEETING_LINES_SHARED);
    now += MINUTE;
    background(&desk, &folder, &key, now);
    now += MINUTE;
    let came = background(&phone, &folder, &key, now);
    assert!(came.written.contains(MEETING_LINES), "{came:?}");
    assert_eq!(lines::load(&on_phone), lines::load(&on_desk), "the same entries on both: {}", phone.read(MEETING_LINES));
    assert_eq!(lines::load(&on_phone).events["psy.1@example.org"].lines(&that_time), [held.clone()], "a time's own lines");
    assert!(lines::load(&on_phone).events.get("known-empty").is_some_and(lines::Kept::is_empty), "known there too, of no line");
    now += MINUTE;
    assert_eq!(background(&desk, &folder, &key, now).sent, 0, "sent once");
    // Its lines back in its file on the desk: the entry goes on the phone too, and stays gone.
    lines::forget(&on_desk, "psy.1@example.org").unwrap();
    for c in [&desk, &phone, &desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert!(!lines::load(&on_phone).events.contains_key("psy.1@example.org") && !lines::load(&on_desk).events.contains_key("psy.1@example.org"), "gone on both: {}\n{}", desk.read(MEETING_LINES), phone.read(MEETING_LINES));
    assert!(lines::load(&on_phone).events.contains_key("known-empty"), "the other entry stays");
    assert_eq!(lines::load(&on_phone), lines::load(&on_desk), "{}\n{}", desk.read(MEETING_LINES), phone.read(MEETING_LINES));
    let _ = std::fs::remove_dir_all(&base);
}

const KIND_HEALTH: &str = "X-SIOUL-CONCEPT:tag:aurelienpierre.com,2026:sioul/child-kind/health";

fn holder_line(who: &str) -> String {
    format!("X-SIOUL-TAKEN-BY;CN={who};PARTSTAT=ACCEPTED:mailto:{}@example.org", who.to_lowercase())
}

/// A desk and a phone sharing `data/meeting-lines.toml`.
fn both_with_lines(name: &str) -> (std::path::PathBuf, std::path::PathBuf, [u8; 32], Computer, Computer) {
    let base = scratch(name);
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    for c in [&desk, &phone] {
        entry(c, &folder, &key, |e| {
            e.start(NOW / 1000);
            e.meeting_lines = MEETING_LINES_SHARED;
        });
    }
    (base, folder, key, desk, phone)
}

/// The review of f85f504, H1 (`probe_s3`, `probe_a1`, end to end on two
/// devices): Tom's psychiatrist made a meeting on the desk, its lines in the
/// entry. The sharing brings the entry to the phone before its calendar's sync
/// brings the meeting. The phone's upkeep, on its stale file: nothing written,
/// nothing forgotten, so no removal reaches the desk. The phone's file then
/// takes the server's meeting, and both read its kind and holder. Later its
/// guests go, on the server: a later file brings the lines back, and that
/// forget travels as any.
#[test]
fn an_entry_ahead_of_its_calendar_stays_on_both_devices() {
    use sioul_core::meeting_lines as lines;
    crate::dav::stand_in::home();
    let (base, folder, key, desk, phone) = both_with_lines("meeting-lines-stale");
    let (on_desk, on_phone) = (desk.path(MEETING_LINES), phone.path(MEETING_LINES));
    let plain = |stamp: &str, more: &str| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:h1-two\r\nDTSTAMP:{stamp}\r\nDTSTART:20991110T090000Z\r\nDTEND:20991110T100000Z\r\nSUMMARY:Tom: psychiatrist\r\n{more}END:VEVENT\r\nEND:VCALENDAR\r\n");
    let held = holder_line("Alex");
    let stale = plain("20261105T090000Z", &format!("{KIND_HEALTH}\r\n{held}\r\n"));
    let meeting = plain("20261106T120000Z", "ORGANIZER:mailto:alex@example.org\r\nATTENDEE;SCHEDULE-AGENT=CLIENT:mailto:office@example.net\r\n");
    // The desk's door: the lines out of the file, into the entry, with the meeting's stamp.
    lines::add(&on_desk, "h1-two", &[(String::new(), KIND_HEALTH.to_string()), (String::new(), held.clone())]).unwrap();
    lines::stamped(&on_desk, "h1-two", &lines::stamp_of(&meeting)).unwrap();
    let mut now = NOW;
    for c in [&desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert!(lines::load(&on_phone).events.contains_key("h1-two"), "it travelled: {}", phone.read(MEETING_LINES));
    // The phone's upkeep, its file behind the entry.
    let file = base.join("phone-h1-two.ics");
    std::fs::write(&file, &stale).unwrap();
    assert!(!lines::back_into_file(&file, &on_phone, &lines::load(&on_phone)), "nothing written");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stale);
    for c in [&phone, &desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    let known = |at: &std::path::Path, text: &str| match lines::read(text, &lines::load(at), true) {
        lines::Read::Known(read) => read.contains(KIND_HEALTH) && read.contains(&held),
        lines::Read::Unknown => false,
    };
    assert!(known(&on_desk, &meeting) && known(&on_phone, &meeting), "known on both: {}\n{}", desk.read(MEETING_LINES), phone.read(MEETING_LINES));
    // Its guests gone later, on the server: the later file takes the lines back on the phone, and the desk's entry goes too.
    std::fs::write(&file, plain("20261107T080000Z", "")).unwrap();
    assert!(lines::back_into_file(&file, &on_phone, &lines::load(&on_phone)));
    assert!(std::fs::read_to_string(&file).unwrap().contains(KIND_HEALTH) && std::fs::read_to_string(&file).unwrap().contains(&held));
    for c in [&phone, &desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert!(!lines::load(&on_desk).events.contains_key("h1-two") && !lines::load(&on_phone).events.contains_key("h1-two"), "{}", desk.read(MEETING_LINES));
    let _ = std::fs::remove_dir_all(&base);
}

/// The review of f85f504, M1 (`probe_s1`): each device kept its own lines of
/// one meeting before the store travelled: the desk its kind and a holder, the
/// phone a holder. Once it opens, whichever device goes first: the kind stays,
/// the two holders keep one, and both devices hold the same.
#[test]
fn a_kind_and_a_holder_kept_apart_both_stay_when_the_store_opens() {
    use sioul_core::meeting_lines as lines;
    for first in ["desk", "phone"] {
        let (base, folder, key, desk, phone) = both_with_lines(&format!("meeting-lines-open-{first}"));
        let (on_desk, on_phone) = (desk.path(MEETING_LINES), phone.path(MEETING_LINES));
        lines::add(&on_desk, "m1", &[(String::new(), KIND_HEALTH.to_string()), (String::new(), holder_line("Alex"))]).unwrap();
        lines::add(&on_phone, "m1", &[(String::new(), holder_line("Sam"))]).unwrap();
        let order: Vec<&Computer> = if first == "desk" { vec![&desk, &phone, &desk, &phone] } else { vec![&phone, &desk, &phone, &desk] };
        let mut now = NOW;
        for c in order {
            now += MINUTE;
            background(c, &folder, &key, now);
        }
        let (d, p) = (lines::load(&on_desk).events["m1"].clone(), lines::load(&on_phone).events["m1"].clone());
        assert_eq!(d, p, "{first}: one entry on both: {}\n{}", desk.read(MEETING_LINES), phone.read(MEETING_LINES));
        let read = d.lines("");
        assert!(read.contains(&KIND_HEALTH.to_string()), "{first}: the kind: {read:?}");
        assert_eq!(read.iter().filter(|l| l.starts_with("X-SIOUL-TAKEN-BY")).count(), 1, "{first}: one holder: {read:?}");
        let _ = std::fs::remove_dir_all(&base);
    }
}

/// The review of f85f504, M1 (`probe_s2`), after the store opened: between two
/// exchanges the desk sets the kind and the phone takes it: both stay, each
/// line on its own. The same holder written on both keeps the later, and a
/// kind taken out on one device goes on both, the holder staying.
#[test]
fn a_kind_and_a_holder_set_apart_between_two_exchanges_both_stay() {
    use sioul_core::meeting_lines as lines;
    let (base, folder, key, desk, phone) = both_with_lines("meeting-lines-apart");
    let (on_desk, on_phone) = (desk.path(MEETING_LINES), phone.path(MEETING_LINES));
    lines::ensure(&on_desk, "m1").unwrap();
    let mut now = NOW;
    for c in [&desk, &phone, &desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert!(lines::load(&on_phone).events.contains_key("m1"), "an entry of no line travels");
    lines::add(&on_desk, "m1", &[(String::new(), KIND_HEALTH.to_string())]).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    lines::add(&on_phone, "m1", &[(String::new(), holder_line("Alex"))]).unwrap();
    for c in [&desk, &phone, &desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    let read = lines::load(&on_desk).events["m1"].lines("");
    assert!(read.contains(&KIND_HEALTH.to_string()) && read.contains(&holder_line("Alex")), "{read:?}");
    assert_eq!(lines::load(&on_desk), lines::load(&on_phone));
    // A later holder on one device, then the other's exchange: the later word.
    lines::add(&on_phone, "m1", &[(String::new(), holder_line("Sam"))]).unwrap();
    for c in [&phone, &desk] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    let read = lines::load(&on_desk).events["m1"].lines("");
    assert!(read.contains(&holder_line("Sam")) && !read.contains(&holder_line("Alex")) && read.contains(&KIND_HEALTH.to_string()), "{read:?}");
    // The kind taken out on the desk (`set_from`, every line it read): that line goes on both.
    let meeting = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:m1\r\nDTSTAMP:20261106T120000Z\r\nDTSTART:20991110T090000Z\r\nSUMMARY:Tom\r\n{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", holder_line("Sam"));
    lines::set_from(&on_desk, "m1", &meeting).unwrap();
    for c in [&desk, &phone, &desk] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert_eq!(lines::load(&on_phone).events["m1"].lines(""), [holder_line("Sam")], "{}", phone.read(MEETING_LINES));
    assert_eq!(lines::load(&on_phone), lines::load(&on_desk));
    assert_eq!(lines::load(&on_phone).events["m1"].stamp.as_deref(), Some("20261106T120000Z"), "its stamp travelled");
    let _ = std::fs::remove_dir_all(&base);
}

/// `probe_s4`, kept for the store's new shape: nothing of the lines in the
/// clear in the folder, and UIDs with a quote, a hash, brackets, dots, a
/// space, a backslash and `*` travel whole, a time's name too, their removals too.
#[test]
fn a_meeting_s_lines_travel_sealed_whatever_their_uid() {
    use sioul_core::meeting_lines as lines;
    let (base, folder, key, desk, phone) = both_with_lines("meeting-lines-sealed");
    let (on_desk, on_phone) = (desk.path(MEETING_LINES), phone.path(MEETING_LINES));
    let odd = ["040000008200E0 \"x\".y#z[1]@exämple.org", "a.b.c", "with space", "back\\slash", "*"];
    for uid in odd {
        lines::add(&on_desk, uid, &[(String::new(), holder_line("Alex")), ("RECURRENCE-ID;TZID=Europe/Paris:20991117T090000".to_string(), KIND_HEALTH.to_string())]).unwrap();
    }
    let mut now = NOW;
    for c in [&desk, &phone, &desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert_eq!(lines::load(&on_phone), lines::load(&on_desk), "{}", phone.read(MEETING_LINES));
    assert_eq!(lines::load(&on_phone).events.len(), odd.len());
    let mut clear = Vec::new();
    let mut stack = vec![folder.clone()];
    while let Some(d) = stack.pop() {
        for f in std::fs::read_dir(&d).unwrap().flatten() {
            if f.path().is_dir() {
                stack.push(f.path());
                continue;
            }
            let bytes = std::fs::read(f.path()).unwrap_or_default();
            let text = String::from_utf8_lossy(&bytes);
            if text.contains("X-SIOUL") || text.contains("alex@example") || text.contains("040000008200E0") || text.contains("times") {
                clear.push(f.path());
            }
        }
    }
    assert!(clear.is_empty(), "in the clear: {clear:?}");
    lines::forget(&on_phone, odd[0]).unwrap();
    lines::forget(&on_phone, "*").unwrap();
    for c in [&phone, &desk] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    let d = lines::load(&on_desk);
    assert!(!d.events.contains_key(odd[0]) && !d.events.contains_key("*") && d.events.contains_key("a.b.c"), "{d:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// `probe_s5`, kept for the store's new shape: a removal against an edit, both
/// orders, ends the same on both devices, line by line by the exchange's clock.
#[test]
fn a_removal_against_an_edit_ends_the_same_on_both() {
    use sioul_core::meeting_lines as lines;
    let (base, folder, key, desk, phone) = both_with_lines("meeting-lines-removal");
    let (on_desk, on_phone) = (desk.path(MEETING_LINES), phone.path(MEETING_LINES));
    for uid in ["a", "b"] {
        lines::add(&on_desk, uid, &[(String::new(), holder_line("Alex"))]).unwrap();
    }
    let mut now = NOW;
    for c in [&desk, &phone, &desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    // a: the desk removes, then the phone edits; b: the phone edits, then the desk removes.
    lines::forget(&on_desk, "a").unwrap();
    lines::add(&on_phone, "b", &[(String::new(), KIND_HEALTH.to_string())]).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    lines::add(&on_phone, "a", &[(String::new(), KIND_HEALTH.to_string())]).unwrap();
    lines::forget(&on_desk, "b").unwrap();
    for c in [&desk, &phone, &desk, &phone, &desk] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert_eq!(lines::load(&on_desk), lines::load(&on_phone), "{}\n{}", desk.read(MEETING_LINES), phone.read(MEETING_LINES));
    let _ = std::fs::remove_dir_all(&base);
}

/// `probe_s6`, the known cost said (the same as your own time's): the store
/// open between desk and phone, an older Sioul coming back holds what it cannot
/// place until it is updated, writing nothing and losing nothing; updated, it
/// gets every entry.
#[test]
fn an_older_device_back_after_the_store_opened_gets_it_once_updated() {
    use sioul_core::meeting_lines as lines;
    let (base, folder, key, desk, phone) = both_with_lines("meeting-lines-tablet");
    lines::add(&desk.path(MEETING_LINES), "u1", &[(String::new(), KIND_HEALTH.to_string())]).unwrap();
    let mut now = NOW;
    for c in [&desk, &phone, &desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    let tablet = Computer::new(&base, "tablet");
    entry(&tablet, &folder, &key, |e| e.start(now / 1000));
    let older = stores(&Config::default(), &tablet.roots).into_iter().filter(|s| s.name != MEETING_LINES).collect::<Vec<_>>();
    super::tests::OLDER.with(|o| *o.borrow_mut() = vec![MEETING_LINES.to_string()]);
    for _ in 0..3 {
        now += MINUTE;
        std::thread::sleep(std::time::Duration::from_millis(2));
        exchange(&Sharing { folder: &folder, computer: &tablet.id, key: &key, memory: &tablet.memory, files: false, hurry: None }, &older, now).unwrap();
    }
    super::tests::OLDER.with(|o| o.borrow_mut().clear());
    assert!(!tablet.path(MEETING_LINES).exists(), "nothing written there");
    entry(&tablet, &folder, &key, |e| e.meeting_lines = MEETING_LINES_SHARED);
    for c in [&desk, &tablet, &desk, &tablet] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert!(lines::load(&tablet.path(MEETING_LINES)).events.contains_key("u1"));
    let _ = std::fs::remove_dir_all(&base);
}

/// The store as an older Sioul wrote it on this device, when it opens: read
/// whole, written in its present form, and sent as such: the other device
/// reads each line by its name, no entry lost, no exchange stopped on a time
/// that is a list in one file and a table in the other.
#[test]
fn the_older_form_of_a_meeting_s_lines_opens_in_its_present_form() {
    use sioul_core::meeting_lines as lines;
    let (base, folder, key, desk, phone) = both_with_lines("meeting-lines-older");
    let (on_desk, on_phone) = (desk.path(MEETING_LINES), phone.path(MEETING_LINES));
    let held = holder_line("Alex");
    std::fs::create_dir_all(on_desk.parent().unwrap()).unwrap();
    std::fs::write(&on_desk, format!("[event.\"old-1\".times]\n\"\" = [\"{KIND_HEALTH}\", \"{held}\"]\n\n[event.\"old-empty\".times]\n")).unwrap();
    lines::add(&on_phone, "old-1", &[(String::new(), holder_line("Sam"))]).unwrap();
    let mut now = NOW;
    for c in [&desk, &phone, &desk, &phone] {
        now += MINUTE;
        let outcome = background(c, &folder, &key, now);
        assert!(outcome.problems.is_empty(), "{outcome:?}");
    }
    assert_eq!(lines::load(&on_desk), lines::load(&on_phone), "{}\n{}", desk.read(MEETING_LINES), phone.read(MEETING_LINES));
    assert!(lines::load(&on_phone).events["old-1"].lines("").contains(&KIND_HEALTH.to_string()));
    assert!(lines::load(&on_phone).events.contains_key("old-empty"));
    let _ = std::fs::remove_dir_all(&base);
}

/// The notices' words wait for every device to have their store, as your
/// own time does: a phone still on a Sioul without it (705576a) would keep
/// their records waiting at every exchange. The desk's log stays on the desk
/// and the phone's steps place everything they read; once the phone says it
/// has the store, the desk's next exchange sends the log, and the phone
/// reads the desk's word.
#[test]
fn the_notices_words_wait_for_a_device_that_has_their_store() {
    let base = scratch("notices-gate");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.notices = NOTICES_SHARED;
    });
    entry(&phone, &folder, &key, |e| e.start(now / 1000));
    let said = "{\"at\":1793000000000,\"ids\":[\"mail:home:0000000000000001\"],\"state\":\"dismissed\"}\n";
    desk.write("state/notices/desk.jsonl", said);
    for _ in 0..2 {
        assert_eq!(background(&desk, &folder, &key, now).sent, 0, "kept to itself");
        now += MINUTE;
    }
    // The phone as 705576a: no such store, none known.
    let older = stores(&Config::default(), &phone.roots).into_iter().filter(|s| s.name != NOTICES).collect::<Vec<_>>();
    super::tests::OLDER.with(|o| *o.borrow_mut() = vec![NOTICES.to_string()]);
    for _ in 0..3 {
        now += MINUTE;
        std::thread::sleep(std::time::Duration::from_millis(2));
        let outcome = exchange(&Sharing { folder: &folder, computer: &phone.id, key: &key, memory: &phone.memory, files: false, hurry: None }, &older, now).unwrap();
        assert_eq!(outcome.pending, 0, "nothing it cannot place: {outcome:?}");
    }
    assert!(!phone.path("state/notices/desk.jsonl").exists(), "nothing written there");
    // The phone updated: the desk's next exchange sends the log.
    super::tests::OLDER.with(|o| o.borrow_mut().clear());
    entry(&phone, &folder, &key, |e| e.notices = NOTICES_SHARED);
    now += MINUTE;
    background(&desk, &folder, &key, now);
    now += MINUTE;
    let came = background(&phone, &folder, &key, now);
    assert!(came.written.contains(NOTICES), "{came:?}");
    assert_eq!(phone.read("state/notices/desk.jsonl"), said);
    let _ = std::fs::remove_dir_all(&base);
}

/// A device with no own time of its own (the phone, just updated), while a
/// device on an older Sioul still counts: another device says it reads own
/// time, so may hold some to send; Settings there names the device that keeps
/// it from travelling (review of e5bd27b, later 1).
#[test]
fn a_device_with_no_own_time_still_names_the_one_holding_it_back() {
    let base = scratch("own-time-named");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone, old) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"), Computer::new(&base, "old"));
    let mut now = NOW;
    for c in [&desk, &phone] {
        entry(c, &folder, &key, |e| {
            e.start(now / 1000);
            e.own_time = OWN_TIME_SHARED;
        });
    }
    entry(&old, &folder, &key, |e| e.start(now / 1000));
    desk.write("data/own-time.toml", "[event.\"board-1\"]\nbefore = 15\n");
    for c in [&desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert!(!phone.path("data/own-time.toml").exists(), "kept on the desk");
    let on_phone = formats(&folder, &key, &phone.memory, &phone.id, now);
    assert_eq!((on_phone.own_time, on_phone.others_share_own_time), (vec![old.id.clone()], true));
    let _ = std::fs::remove_dir_all(&base);
}

/// Once your own time around meetings has travelled from a device, it keeps
/// travelling there whoever joins (`Memory::own_time_open`): a device on an
/// older Sioul counted for a while, then forgotten, never makes the store
/// leave and join again, its records written back over what changed
/// meanwhile (re-check of 857c39e, §3.1).
#[test]
fn your_own_time_keeps_travelling_once_it_has() {
    let base = scratch("own-time-open");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone, old) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"), Computer::new(&base, "old"));
    let mut now = NOW;
    for c in [&desk, &phone] {
        entry(c, &folder, &key, |e| {
            e.start(now / 1000);
            e.own_time = OWN_TIME_SHARED;
        });
    }
    desk.write("data/own-time.toml", "[event.\"board-1\"]\nbefore = 15\n");
    let before = |c: &Computer| c.read("data/own-time.toml").parse::<toml::Table>().ok().and_then(|t| t["event"]["board-1"]["before"].as_integer());
    for _ in 0..2 {
        for c in [&desk, &phone] {
            now += MINUTE;
            background(c, &folder, &key, now);
        }
    }
    assert_eq!((before(&desk), before(&phone)), (Some(15), Some(15)), "travelled");
    // A third device whose entry does not say it reads own time counts now.
    entry(&old, &folder, &key, |e| e.start(now / 1000));
    for c in [&desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    assert!(formats(&folder, &key, &desk.memory, &desk.id, now).own_time.is_empty(), "open here: nothing said held back");
    // Changed on the desk meanwhile, then the third device forgotten on both.
    desk.write("data/own-time.toml", "[event.\"board-1\"]\nbefore = 25\n");
    for c in [&desk, &phone] {
        now += MINUTE;
        background(c, &folder, &key, now);
    }
    for c in [&desk, &phone] {
        forget_device(&folder, &key, &c.memory, &c.id, &old.id).unwrap();
    }
    for _ in 0..2 {
        for c in [&desk, &phone] {
            now += MINUTE;
            background(c, &folder, &key, now);
        }
    }
    assert_eq!((before(&desk), before(&phone)), (Some(25), Some(25)), "the change kept on both");
    let _ = std::fs::remove_dir_all(&base);
}

/// The money movements' store (`notes/sioul-movements.toml`, beside the
/// budget file) beside a phone whose entry does not say it has the store, as
/// 705576a's never does: never sent while it does not, so that an older
/// Sioul holds no record it would keep waiting; the desk's file kept to
/// itself. Once the phone's entry says so, the desk's next exchange sends
/// it; a correction made on the phone comes back, entry by entry.
#[test]
fn the_money_movements_store_waits_for_a_device_that_has_it() {
    let base = scratch("movements-gate");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    for notes in [&desk_notes, &phone_notes] {
        std::fs::create_dir_all(notes).unwrap();
    }
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.movements = MOVEMENTS_SHARED;
    });
    entry(&phone, &folder, &key, |e| e.start(now / 1000));
    let store = |notes: &Path| std::fs::read_to_string(notes.join(sioul_core::movements::STORE)).unwrap_or_default();
    std::fs::write(desk_notes.join(sioul_core::movements::STORE), "format = 1\n\n[[apart]]\na = \"bank:main/A1\"\nb = \"line:l1\"\n").unwrap();
    for _ in 0..2 {
        desk.exchange_notes(&folder, &key, now, &desk_notes);
        now += MINUTE;
        phone.exchange_notes(&folder, &key, now, &phone_notes);
        now += MINUTE;
    }
    assert!(store(&phone_notes).is_empty(), "kept on the desk: {}", store(&phone_notes));
    // The phone updated: its entry says it has the store; the desk's next exchange sends it.
    entry(&phone, &folder, &key, |e| e.movements = MOVEMENTS_SHARED);
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    now += MINUTE;
    let came = phone.exchange_notes(&folder, &key, now, &phone_notes);
    assert!(came.written.contains(MOVEMENTS), "{came:?}");
    let read = |notes: &Path| sioul_core::movements::Store::load(notes).unwrap();
    assert_eq!(read(&phone_notes), read(&desk_notes), "the same store on both");
    // A source joined on the phone travels back, beside what was there.
    sioul_core::movements::Store::edit(&phone_notes, |edit| edit.link("preset:rent/2026-11-01", "sioul:letter/2026-10-28-1a2b", "hand", None)).unwrap();
    now += MINUTE;
    phone.exchange_notes(&folder, &key, now, &phone_notes);
    now += MINUTE;
    desk.exchange_notes(&folder, &key, now, &desk_notes);
    let desk_store = read(&desk_notes);
    assert_eq!((desk_store.apart.len(), desk_store.links.len(), desk_store.format), (1, 1, 1), "{}", store(&desk_notes));
    let _ = std::fs::remove_dir_all(&base);
}

/// The money movements' new lists (5 November 2026: fees, payouts carried,
/// your words, removals, exports taken in) travel entry by entry, as links
/// do: a fee kept on the desk and another on the phone in the same
/// exchanges both stay on both, never one list winning over the other.
#[test]
fn the_money_movements_fees_travel_entry_by_entry() {
    use sioul_core::movements::{Fee, FeeFrom, FeeKind, FeeTold, Store};
    let base = scratch("movements-fees");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (desk_notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    for notes in [&desk_notes, &phone_notes] {
        std::fs::create_dir_all(notes).unwrap();
        std::fs::write(notes.join(sioul_core::movements::STORE), "format = 1\n").unwrap();
    }
    let mut now = NOW;
    for c in [&desk, &phone] {
        entry(c, &folder, &key, |e| {
            e.start(now / 1000);
            e.movements = MOVEMENTS_SHARED;
        });
    }
    for _ in 0..2 {
        desk.exchange_notes(&folder, &key, now, &desk_notes);
        now += MINUTE;
        phone.exchange_notes(&folder, &key, now, &phone_notes);
        now += MINUTE;
    }
    let told = |cents: i64| FeeTold { gross: Some(sioul_core::money::Money(cents)), fees: vec![Fee { kind: FeeKind::Processor, amount: sioul_core::money::Money(cents / 30), from: FeeFrom::Export, about: false }], ..FeeTold::default() };
    Store::edit(&desk_notes, |edit| {
        edit.fee("bank:paypal/T1", &told(52000));
        edit.gone("preset:rent/2026-10-01", "not-this-time");
    })
    .unwrap();
    Store::edit(&phone_notes, |edit| {
        edit.fee("bank:paypal/T3", &told(4000));
        edit.edit("bank:paypal/T3", Some("Client B, the October lesson"), None, None);
    })
    .unwrap();
    for _ in 0..3 {
        desk.exchange_notes(&folder, &key, now, &desk_notes);
        now += MINUTE;
        phone.exchange_notes(&folder, &key, now, &phone_notes);
        now += MINUTE;
    }
    for notes in [&desk_notes, &phone_notes] {
        let store = Store::load(notes).unwrap();
        let mut sources: Vec<&str> = store.fees.iter().map(|f| f.source.as_str()).collect();
        sources.sort();
        assert_eq!((sources, store.gone.len(), store.edits.len()), (vec!["bank:paypal/T1", "bank:paypal/T3"], 1, 1), "{}", std::fs::read_to_string(notes.join(sioul_core::movements::STORE)).unwrap());
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The persons' store (`data/persons.toml`) beside a phone whose entry does
/// not say it has the store, as 705576a's never does: kept on the desk; once
/// the phone says so, the next exchanges join the two. The persons each
/// device read from its old texts carry the same ids (`persons::name_id`),
/// so that they join as one, each field kept: a date of birth said on the
/// desk before the phone had the store stays. From then on each field
/// travels apart: a field added on the desk and another on the phone in the
/// same exchanges both stay. Whether the old texts were read stays on each
/// device (`names_read`).
#[test]
fn the_persons_store_waits_for_a_device_that_has_it() {
    use sioul_core::persons::{Persons, Relation, name_id};
    let base = scratch("persons-gate");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.persons = PERSONS_SHARED;
    });
    entry(&phone, &folder, &key, |e| e.start(now / 1000));
    let lea = name_id(Relation::Child, "Léa");
    let read_apart = format!("[person.{lea}]\nname = \"Léa\"\nrelation = \"child\"\nagents = false\n");
    desk.write(PERSONS, &format!("format = 1\nnames_read = 1\n\n{read_apart}born = 2014-05-12\n"));
    phone.write(PERSONS, &format!("format = 1\n\n{read_apart}"));
    let rounds = |now: &mut i64| {
        for _ in 0..2 {
            background(&desk, &folder, &key, *now);
            *now += MINUTE;
            background(&phone, &folder, &key, *now);
            *now += MINUTE;
        }
    };
    rounds(&mut now);
    let read = |c: &Computer| Persons::read(&c.path(PERSONS)).unwrap();
    assert_eq!(read(&phone).persons[0].born, None, "kept on the desk: {}", phone.read(PERSONS));
    // The phone updated: its entry says it has the store; the next exchanges join the two.
    entry(&phone, &folder, &key, |e| e.persons = PERSONS_SHARED);
    rounds(&mut now);
    assert_eq!((read(&desk).persons.len(), read(&phone).persons.len()), (1, 1), "one Léa on each: {}", phone.read(PERSONS));
    assert_eq!(read(&desk).persons, read(&phone).persons, "the same Léa on both: {}\n{}", desk.read(PERSONS), phone.read(PERSONS));
    assert_eq!(read(&phone).persons[0].born, Some("2014-05-12".parse().unwrap()), "the desk's date of birth joined: {}", phone.read(PERSONS));
    // From then on, field by field: lives with you said on the desk, a full name on the phone.
    desk.write(PERSONS, &desk.read(PERSONS).replace("relation = \"child\"", "relation = \"child\"\nlives_with = true"));
    phone.write(PERSONS, &phone.read(PERSONS).replace("name = \"Léa\"", "name = \"Léa\"\nfull_name = \"Léa Martin\""));
    rounds(&mut now);
    let (on_desk, on_phone) = (read(&desk), read(&phone));
    assert_eq!((on_phone.persons[0].lives_with, on_phone.persons[0].full_name.as_str()), (Some(true), "Léa Martin"), "phone: {}\ndesk: {}", phone.read(PERSONS), desk.read(PERSONS));
    assert_eq!(on_desk.persons, on_phone.persons, "the same Léa on both: {}", desk.read(PERSONS));
    assert_eq!((on_desk.names_read, on_phone.names_read), (1, 0), "read on each device apart");
    let _ = std::fs::remove_dir_all(&base);
}

/// Your word on topics (`state/topics/labels/<device>.jsonl`) beside a phone
/// whose entry does not say it has the store, as an older Sioul's never
/// does: kept on the desk; once the phone says so, the next exchanges bring
/// the desk's log there, beside the phone's own.
#[test]
fn your_word_on_topics_waits_for_a_device_that_has_it() {
    let base = scratch("topics-gate");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.topics = TOPICS_SHARED;
    });
    entry(&phone, &folder, &key, |e| e.start(now / 1000));
    let line = "{\"act\":\"message\",\"at\":1791360000,\"message_id\":\"a@example.org\",\"topic\":\"choir\"}\n";
    let desk_log = format!("{TOPICS}{}.jsonl", desk.id);
    desk.write(&desk_log, line);
    // The topics' table, the desk's own training: the same gate.
    desk.write("data/topics/table.bin", "SIOULTOP stand-in table");
    let rounds = |now: &mut i64| {
        for _ in 0..2 {
            background(&desk, &folder, &key, *now);
            *now += MINUTE;
            background(&phone, &folder, &key, *now);
            *now += MINUTE;
        }
    };
    rounds(&mut now);
    assert!(!phone.path(&desk_log).exists(), "kept on the desk");
    assert!(!phone.path("data/topics/table.bin").exists(), "the table kept on the desk too");
    entry(&phone, &folder, &key, |e| e.topics = TOPICS_SHARED);
    rounds(&mut now);
    assert_eq!(phone.read(&desk_log), line, "the desk's word, on the phone");
    assert_eq!(phone.read("data/topics/table.bin"), "SIOULTOP stand-in table", "and the desk's table");
    let said = sioul_core::topics::labels::Said::of(&sioul_core::topics::labels::read_all_in(&phone.roots.state.join("topics")));
    assert_eq!(said.message("a@example.org"), Some(Some("choir")));
    let _ = std::fs::remove_dir_all(&base);
}

/// A table trained on a device at `now` (its clock's time): its file written,
/// dated then; never after this computer's real clock, whose files dated
/// ahead would wait as still changing (a test that runs months ahead).
fn trained(c: &Computer, relative: &str, text: &str, now: i64) {
    c.write(relative, text);
    let at = now.min(clock_ms() - 10 * MINUTE);
    set_time(&c.path(relative), std::time::UNIX_EPOCH + std::time::Duration::from_millis(at as u64)).unwrap();
}

/// Whether a shared table's record is current (a value, not a take-out) in a device's memory.
fn shared_now(c: &Computer, file: &str) -> bool {
    Memory::load_all(&c.memory, &c.id).entries.iter().any(|(k, known)| file_of(k) == file && !known.h.is_empty())
}

/// An exchange run as an older Sioul's (`tests::OLDER_TABLES`): it never
/// learns, and a take-out of a table read there deletes its file.
fn older(c: &Computer, folder: &Path, key: &[u8; 32], now: i64) -> Outcome {
    super::tests::OLDER_TABLES.with(|b| b.set(true));
    let outcome = background(c, folder, key, now);
    super::tests::OLDER_TABLES.with(|b| b.set(false));
    outcome
}

/// An exchange whose view of the others' entries lags (`tests::GATE_CLOSED`).
fn lagging(c: &Computer, folder: &Path, key: &[u8; 32], now: i64) -> Outcome {
    super::tests::GATE_CLOSED.with(|b| b.set(true));
    let outcome = background(c, folder, key, now);
    super::tests::GATE_CLOSED.with(|b| b.set(false));
    outcome
}

/// Every device trains its own spam filter and topics (docs/spam-filter.md,
/// "Every device trains its own"). While a device does not learn (an older
/// Sioul, a new one not trained yet, one whose switch is off), the tables
/// travel: the newest goes, and that device reads it. Once every device
/// learns, nothing goes, the server's copies are taken out, and every device
/// keeps its own table; a device that learns never takes another's in. A
/// device whose switch turns off gets the tables again: retirement is
/// reversible.
#[test]
fn the_tables_travel_while_a_device_does_not_learn() {
    let base = scratch("tables-travel");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.topics = TOPICS_SHARED;
        e.learns = LEARNS_HERE;
    });
    entry(&phone, &folder, &key, |e| {
        e.start(now / 1000);
        e.topics = TOPICS_SHARED;
    });
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's first table", now);
    trained(&desk, "data/topics/table.bin", "SIOULTOP the desk's first table", now);
    let rounds = |now: &mut i64, phone_older: bool| {
        for _ in 0..3 {
            background(&desk, &folder, &key, *now);
            *now += MINUTE;
            if phone_older { older(&phone, &folder, &key, *now) } else { background(&phone, &folder, &key, *now) };
            *now += MINUTE;
        }
    };
    // An older phone reads the table another device made, as before.
    rounds(&mut now, true);
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's first table");
    assert_eq!(phone.read("data/topics/table.bin"), "SIOULTOP the desk's first table");
    // Updated, not trained yet: it still reads the newest the desk trains.
    entry(&phone, &folder, &key, |e| e.learns = KEEPS_TABLES);
    rounds(&mut now, false);
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's second table", now);
    rounds(&mut now, false);
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's second table", "the newest travels");
    assert!(shared_now(&desk, SPAM_TABLE) && shared_now(&phone, TOPICS_TABLE));
    // The phone trains its own: every device learns. Nothing goes, and the server's copies are taken out.
    trained(&phone, "data/spam/table.bin", "SIOULSPM the phone's own table", now);
    trained(&phone, "data/topics/table.bin", "SIOULTOP the phone's own table", now);
    entry(&phone, &folder, &key, |e| e.learns = LEARNS_HERE);
    rounds(&mut now, false);
    for c in [&desk, &phone] {
        for file in [SPAM_TABLE, TOPICS_TABLE] {
            assert!(!shared_now(c, file), "{file}: taken out of the sharing");
        }
    }
    assert_eq!(desk.read("data/spam/table.bin"), "SIOULSPM the desk's second table", "a device that learns never takes another's in");
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the phone's own table", "each keeps its own");
    assert_eq!(phone.read("data/topics/table.bin"), "SIOULTOP the phone's own table");
    // The desk trains again: nothing of it goes.
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's third table", now);
    rounds(&mut now, false);
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the phone's own table", "never sent");
    assert!(!shared_now(&desk, SPAM_TABLE) && !shared_now(&phone, SPAM_TABLE));
    // The phone's switch turned off: it no longer learns, and gets the desk's newest again.
    entry(&phone, &folder, &key, |e| e.learns = KEEPS_TABLES);
    rounds(&mut now, false);
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's third table", "reversible: the tables travel again");
    assert_eq!(desk.read("data/spam/table.bin"), "SIOULSPM the desk's third table");
    let _ = std::fs::remove_dir_all(&base);
}

/// R1 (review of 7 November 2026): a take-out of the tables never deletes a
/// table file, whatever the device that reads it: one that learns, whose
/// view of the others lags (it still sends its own meanwhile), and one that
/// no longer learns (its switch turned off just after the take-out went).
/// Each keeps the file it uses; the server's copy alone goes.
#[test]
fn a_take_out_of_the_tables_never_deletes_a_table_file() {
    let base = scratch("tables-kept");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"), Computer::new(&base, "phone"));
    let mut now = NOW;
    for c in [&desk, &laptop, &phone] {
        entry(c, &folder, &key, |e| {
            e.start(now / 1000);
            e.topics = TOPICS_SHARED;
            e.learns = KEEPS_TABLES;
        });
    }
    entry(&desk, &folder, &key, |e| e.learns = LEARNS_HERE);
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's table", now);
    trained(&desk, "data/topics/table.bin", "SIOULTOP the desk's table", now);
    for _ in 0..2 {
        for c in [&desk, &laptop, &phone] {
            background(c, &folder, &key, now);
            now += MINUTE;
        }
    }
    assert_eq!(laptop.read("data/spam/table.bin"), "SIOULSPM the desk's table");
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's table");
    // The laptop and the phone train their own and say so; the laptop's view lags behind.
    now += MINUTE;
    for c in [&laptop, &phone] {
        trained(c, "data/spam/table.bin", "SIOULSPM a table of its own", now);
        trained(c, "data/topics/table.bin", "SIOULTOP a table of its own", now);
        entry(c, &folder, &key, |e| e.learns = LEARNS_HERE);
    }
    for _ in 0..3 {
        background(&desk, &folder, &key, now);
        now += MINUTE;
        lagging(&laptop, &folder, &key, now);
        now += MINUTE;
        background(&phone, &folder, &key, now);
        now += MINUTE;
    }
    let taken = |c: &Computer| Memory::load_all(&c.memory, &c.id).entries.iter().any(|(k, known)| file_of(k) == SPAM_TABLE && known.h.is_empty());
    assert!(taken(&desk) || taken(&phone), "the desk or the phone took the server's copies out");
    for c in [&laptop, &phone] {
        assert_eq!(c.read("data/spam/table.bin"), "SIOULSPM a table of its own", "kept, in use");
        assert_eq!(c.read("data/topics/table.bin"), "SIOULTOP a table of its own");
    }
    // The laptop's view catches up: it takes them out too, its file kept.
    for _ in 0..2 {
        for c in [&desk, &laptop, &phone] {
            background(c, &folder, &key, now);
            now += MINUTE;
        }
    }
    for c in [&desk, &laptop, &phone] {
        assert!(!shared_now(c, SPAM_TABLE) && !shared_now(c, TOPICS_TABLE), "taken out, as each knows it");
    }
    assert_eq!(laptop.read("data/spam/table.bin"), "SIOULSPM a table of its own");
    assert_eq!(desk.read("data/spam/table.bin"), "SIOULSPM the desk's table");
    // A take-out read by a device that no longer learns: its switch turned
    // off once the desk's take-out went, before it read it.
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's next table", now);
    entry(&phone, &folder, &key, |e| e.learns = KEEPS_TABLES);
    for _ in 0..3 {
        for c in [&desk, &laptop, &phone] {
            background(c, &folder, &key, now);
            now += MINUTE;
        }
    }
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's next table", "it reads the newest");
    entry(&phone, &folder, &key, |e| e.learns = LEARNS_HERE);
    for _ in 0..2 {
        for c in [&desk, &laptop] {
            background(c, &folder, &key, now);
            now += MINUTE;
        }
    }
    assert!(!shared_now(&desk, SPAM_TABLE), "the desk took it out again");
    entry(&phone, &folder, &key, |e| e.learns = KEEPS_TABLES);
    background(&phone, &folder, &key, now);
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's next table", "a take-out read by a device that does not learn: its file kept");
    let _ = std::fs::remove_dir_all(&base);
}

/// R2 (review of 7 November 2026): an older Sioul deletes its table when it
/// reads a take-out. While any device ever known may come back an older
/// Sioul (a phone silent for `SILENT_DAYS` in a drawer), the tables are
/// retired among the others, nothing sent, but never taken out: back, the
/// phone keeps its last table, and gets the newest.
#[test]
fn an_older_device_back_late_keeps_its_table() {
    let base = scratch("tables-older-back");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"), Computer::new(&base, "phone"));
    let mut now = NOW;
    for c in [&desk, &laptop] {
        entry(c, &folder, &key, |e| {
            e.start(now / 1000);
            e.topics = TOPICS_SHARED;
            e.learns = LEARNS_HERE;
        });
    }
    entry(&phone, &folder, &key, |e| {
        e.start(now / 1000);
        e.topics = TOPICS_SHARED;
    });
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's table", now);
    trained(&laptop, "data/spam/table.bin", "SIOULSPM the laptop's table", now - MINUTE);
    for _ in 0..2 {
        background(&desk, &folder, &key, now);
        now += MINUTE;
        background(&laptop, &folder, &key, now);
        now += MINUTE;
        older(&phone, &folder, &key, now);
        now += MINUTE;
    }
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's table", "the newest");
    assert_eq!(laptop.read("data/spam/table.bin"), "SIOULSPM the laptop's table", "a device that learns keeps its own");
    // The phone in a drawer: silent for SILENT_DAYS on the desk and the laptop.
    now += (SILENT_DAYS + 1) * DAY;
    entry(&desk, &folder, &key, |e| e.start(now / 1000));
    entry(&laptop, &folder, &key, |e| e.start(now / 1000));
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's table of the spring", now);
    for _ in 0..3 {
        background(&desk, &folder, &key, now);
        now += MINUTE;
        background(&laptop, &folder, &key, now);
        now += MINUTE;
    }
    let judged = |c: &Computer, now: i64| tables_now(&Sharing { folder: &folder, computer: &c.id, key: &key, memory: &c.memory, files: false, hurry: None }, &Memory::load_all(&c.memory, &c.id), now);
    for c in [&desk, &laptop] {
        let tables = judged(c, now);
        assert!(tables.retired && !tables.removable, "retired, never taken out: {tables:?}");
        assert!(shared_now(c, SPAM_TABLE), "the desk's table of the autumn still on the server");
    }
    // The phone back, still an older Sioul: it keeps its table.
    entry(&phone, &folder, &key, |e| e.start(now / 1000));
    for _ in 0..3 {
        older(&phone, &folder, &key, now);
        now += MINUTE;
        background(&desk, &folder, &key, now);
        now += MINUTE;
        background(&laptop, &folder, &key, now);
        now += MINUTE;
    }
    assert!(phone.path("data/spam/table.bin").exists(), "the older phone back late kept its table");
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's table of the spring", "and got the newest once back");
    // Forgotten in the settings, it still holds the take-out back: it may come back.
    forget_device(&folder, &key, &desk.memory, &desk.id, &phone.id).unwrap();
    let tables = judged(&desk, now);
    assert!(!tables.removable, "{tables:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// R3 (review of 7 November 2026): a new device that has no table of its own
/// (just set up, its switch off, its training stopped by the guard) does not
/// learn, so the tables travel to it: it gets the desk's.
#[test]
fn a_device_without_a_table_of_its_own_gets_the_newest() {
    let base = scratch("tables-new-device");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.topics = TOPICS_SHARED;
        e.learns = LEARNS_HERE;
    });
    entry(&phone, &folder, &key, |e| {
        e.start(now / 1000);
        e.topics = TOPICS_SHARED;
        e.learns = KEEPS_TABLES;
    });
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's table", now);
    trained(&desk, "data/topics/table.bin", "SIOULTOP the desk's table", now);
    for _ in 0..4 {
        background(&desk, &folder, &key, now);
        now += MINUTE;
        background(&phone, &folder, &key, now);
        now += MINUTE;
    }
    assert_eq!(phone.read("data/spam/table.bin"), "SIOULSPM the desk's table");
    assert_eq!(phone.read("data/topics/table.bin"), "SIOULTOP the desk's table");
    let _ = std::fs::remove_dir_all(&base);
}

/// F3 (review of 7 November 2026): the tables are judged at every exchange,
/// one with nothing else new included. A desk whose exchanges went quiet
/// stops sending as soon as its own entry says it learns.
#[test]
fn the_tables_are_judged_in_a_quiet_exchange_too() {
    let base = scratch("tables-quiet");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let mut now = NOW;
    entry(&desk, &folder, &key, |e| {
        e.start(now / 1000);
        e.learns = KEEPS_TABLES;
    });
    entry(&phone, &folder, &key, |e| {
        e.start(now / 1000);
        e.learns = LEARNS_HERE;
    });
    trained(&desk, "data/spam/table.bin", "SIOULSPM the desk's table", now);
    for _ in 0..3 {
        background(&desk, &folder, &key, now);
        now += MINUTE;
        background(&phone, &folder, &key, now);
        now += MINUTE;
    }
    assert!(shared_now(&desk, SPAM_TABLE));
    let before = quieted();
    background(&desk, &folder, &key, now);
    assert_eq!(quieted(), before + 1, "nothing new: quiet");
    // Its own entry now says it learns, nothing else changed: the next exchange judges the tables.
    crate::devices::change(&crate::devices::own_path(&desk.roots.state), None, |e| e.learns = LEARNS_HERE).unwrap();
    now += MINUTE;
    background(&desk, &folder, &key, now);
    assert_eq!(quieted(), before + 1, "judged again");
    let sharing = Sharing { folder: &folder, computer: &desk.id, key: &key, memory: &desk.memory, files: false, hurry: None };
    assert!(tables_now(&sharing, &Memory::load_all(&desk.memory, &desk.id), now).retired);
    assert!(!shared_now(&desk, SPAM_TABLE), "retired and taken out at once");
    let _ = std::fs::remove_dir_all(&base);
}

/// Each `paths` dated a minute back: a folder's time then no longer "this
/// very moment" (`RACY_MS`), as between two exchanges a minute apart.
#[cfg(unix)]
fn a_minute_back(paths: &[&Path]) {
    let then = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
    for path in paths {
        std::fs::File::open(path).unwrap().set_modified(then).unwrap();
    }
}

fn asked() -> usize {
    ASKED.with(|n| n.replace(0))
}

/// The notes folder is listed only where it changed (review of 13 October
/// 2026, §4.8: the whole folder was listed every minute, each note's time
/// asked of the disk, which costs more as the folder grows). A folder whose
/// time did not change is taken as listed, its notes as last read: nothing
/// asked. A note made, renamed or removed changes its folder's time: that
/// folder alone is listed again, and the change goes at once. A note written
/// in place (no folder's time changes) goes at the next whole listing
/// (`RELIST_MS`). A folder's time is a Unix test's to set back (`a_minute_back`).
#[cfg(unix)]
#[test]
fn the_notes_folder_is_listed_only_where_it_changed() {
    let base = scratch("listed-notes");
    let folder = base.join("Sioul");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, phone) = (Computer::new(&base, "desk"), Computer::new(&base, "phone"));
    let (notes, phone_notes) = (base.join("desk-notes"), base.join("phone-notes"));
    let (admin, old) = (notes.join("admin"), notes.join("admin").join("old"));
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&phone_notes).unwrap();
    for (path, text) in [(notes.join("plan.md"), "the plan\n"), (admin.join("lease.md"), "the lease\n"), (old.join("2020.md"), "an old year\n")] {
        std::fs::write(&path, text).unwrap();
        a_minute_back(&[&path]);
    }
    a_minute_back(&[&notes, &admin, &old]);
    RELIST.with(|r| r.set(10 * MINUTE));
    let mut now = NOW;
    // The notes shared, as with the part "Notes" switched on.
    let mut turn = |c: &Computer, notes: &Path| {
        now += MINUTE;
        std::thread::sleep(std::time::Duration::from_millis(2));
        let config = Config { notes_root: Some(notes.display().to_string()), ..Config::default() };
        let stores = stores_of(&config, &c.roots, &|part| part == "notes" || shared_by_default(part, &config));
        exchange(&Sharing { folder: &folder, computer: &c.id, key: &key, memory: &c.memory, files: true, hurry: None }, &stores, now).unwrap()
    };
    turn(&desk, &notes);
    assert_eq!(asked(), 3, "listed whole the first time");
    turn(&phone, &phone_notes);
    assert_eq!(std::fs::read_to_string(phone_notes.join("admin/old/2020.md")).unwrap(), "an old year\n");
    asked();
    // Nothing changed: nothing asked.
    turn(&desk, &notes);
    assert_eq!(asked(), 0, "every folder as listed");
    // A note made in admin/: that folder alone listed again, and it goes at once.
    std::fs::write(admin.join("deposit.md"), "the deposit\n").unwrap();
    turn(&desk, &notes);
    assert_eq!(asked(), 2, "admin/'s two notes, nothing of the others");
    turn(&phone, &phone_notes);
    assert_eq!(std::fs::read_to_string(phone_notes.join("admin/deposit.md")).unwrap(), "the deposit\n");
    // Written in place: no folder's time changes; it goes at the next whole listing.
    a_minute_back(&[&admin, &admin.join("deposit.md")]);
    turn(&desk, &notes);
    asked();
    std::fs::write(notes.join("plan.md"), "the plan, changed in place\n").unwrap();
    turn(&desk, &notes);
    assert_eq!(asked(), 0, "taken as last read");
    turn(&phone, &phone_notes);
    assert_eq!(std::fs::read_to_string(phone_notes.join("plan.md")).unwrap(), "the plan\n", "not yet");
    RELIST.with(|r| r.set(0));
    asked();
    turn(&desk, &notes);
    assert_eq!(asked(), 4, "listed whole");
    turn(&phone, &phone_notes);
    assert_eq!(std::fs::read_to_string(phone_notes.join("plan.md")).unwrap(), "the plan, changed in place\n");
    // A note removed: its folder's time changes, and it is taken out at once.
    RELIST.with(|r| r.set(10 * MINUTE));
    turn(&desk, &notes);
    std::fs::remove_file(admin.join("lease.md")).unwrap();
    turn(&desk, &notes);
    let files = Memory::load_all(&desk.memory, &desk.id).files;
    assert!(!files.keys().any(|f| f.ends_with("admin/lease.md")) && files.keys().any(|f| f.ends_with("admin/deposit.md")), "{:?}", files.keys().collect::<Vec<_>>());
    RELIST.with(|r| r.set(0));
    let _ = std::fs::remove_dir_all(&base);
}
