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
