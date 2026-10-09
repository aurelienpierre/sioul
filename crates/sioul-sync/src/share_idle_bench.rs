// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What an idle exchange costs on a phone the size of a real one, all of it
//! invented: 6,000 sealed files in the folder, a texts log of 10 MB and a
//! calls log beside it, so that `files.json` weighs about 15 MB
//! (docs/android.md, "Its cost"). Ignored: it writes about 100 MB and takes a
//! minute. Run it with a folder of its own, outside a RAM disk:
//!
//! `SIOUL_IDLE_BENCH=<an empty folder> cargo test --release -p sioul-sync idle_bench -- --ignored --nocapture`
//!
//! It prints, for each case, the median CPU time of the exchange's thread
//! (Linux's `schedstat`, in milliseconds), its wall time, and what it wrote
//! into the shared folder.

use super::*;
use crate::devices::{COMPUTER, Entry, PHONE};

const KEY: [u8; 32] = [7u8; 32];
const MINUTE_MS: i64 = 60_000;

/// This thread's time on a CPU so far, in nanoseconds (Linux).
fn cpu_ns() -> u64 {
    std::fs::read_to_string("/proc/thread-self/schedstat").ok().and_then(|t| t.split_whitespace().next().and_then(|n| n.parse().ok())).unwrap_or(0)
}

/// A small generator: the same invented data at each run.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n.div_ceil(8)).flat_map(|_| self.next().to_le_bytes()).take(n).collect()
    }

    /// A line as a sealed one looks: `n` random bytes in base64.
    fn line(&mut self, n: usize) -> String {
        B64.encode(self.bytes(n))
    }
}

struct Dev {
    roots: Roots,
    id: String,
    memory: PathBuf,
    kind: &'static str,
}

impl Dev {
    fn new(base: &Path, name: &str, kind: &'static str, id: &str) -> Dev {
        let root = base.join(name);
        let roots = Roots { config: root.join("config"), data: root.join("data"), state: root.join("state") };
        for dir in [&roots.config, &roots.data, &roots.state] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let memory = roots.state.join("share").join("memory.json");
        Dev { roots, id: id.to_string(), memory, kind }
    }

    fn stores(&self) -> Vec<Store> {
        stores_of(&Config::default(), &self.roots, &|_| true)
    }

    fn path_of(&self, store: &str) -> PathBuf {
        self.stores().into_iter().find(|s| s.name == store).unwrap().path
    }

    /// An exchange as the background step runs it, then its entry says so.
    fn exchange(&self, folder: &Path, now_ms: i64) -> Outcome {
        let outcome = exchange(&Sharing { folder, computer: &self.id, key: &KEY, memory: &self.memory, files: false, hurry: None }, &self.stores(), now_ms).unwrap();
        self.exported(folder, &outcome, now_ms);
        outcome
    }

    fn exported(&self, folder: &Path, outcome: &Outcome, now_ms: i64) {
        crate::devices::change(&crate::devices::own_path(&self.roots.state), Some((folder, &KEY)), |e| e.exported(outcome.looked, outcome.wrote, now_ms / 1000)).unwrap();
    }

    fn session(&self, folder: &Path, working: bool, now_ms: i64) {
        let (kind, id) = (self.kind, self.id.clone());
        crate::devices::change(&crate::devices::own_path(&self.roots.state), Some((folder, &KEY)), |e: &mut Entry| {
            e.id = id;
            e.kind = kind.into();
            e.doses = true;
            e.format = FORMAT;
            e.start(now_ms / 1000);
            if !working {
                e.close(now_ms / 1000);
            }
        })
        .unwrap();
    }
}

/// The folder's files, each by its path, with its size and time: what an exchange wrote there.
fn listing(folder: &Path) -> BTreeMap<String, (u64, u64)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(meta) = entry.metadata() {
                out.insert(path.strip_prefix(folder).unwrap().to_string_lossy().to_string(), (meta.len(), modified_ns(&meta)));
            }
        }
    }
    out
}

fn written(before: &BTreeMap<String, (u64, u64)>, after: &BTreeMap<String, (u64, u64)>) -> usize {
    after.iter().filter(|(name, stamp)| before.get(*name) != Some(stamp)).count()
}

struct Case {
    name: &'static str,
    cpu: Vec<f64>,
    wall: Vec<f64>,
    writes: usize,
    /// Said as a mean rather than a median.
    mean: bool,
}

impl Case {
    fn new(name: &'static str) -> Case {
        Case { name, cpu: Vec::new(), wall: Vec::new(), writes: 0, mean: false }
    }

    fn time<T>(&mut self, run: impl FnOnce() -> T) -> T {
        let (cpu, wall) = (cpu_ns(), std::time::Instant::now());
        let out = run();
        self.cpu.push((cpu_ns() - cpu) as f64 / 1e6);
        self.wall.push(wall.elapsed().as_secs_f64() * 1e3);
        out
    }

    fn line(&self) -> String {
        let median = |v: &[f64]| {
            let mut v = v.to_vec();
            v.sort_by(f64::total_cmp);
            v.get(v.len() / 2).copied().unwrap_or(0.0)
        };
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        let at = |v: &[f64]| if self.mean { mean(v) } else { median(v) };
        format!("{:58} runs {:3}  cpu {:9.1} ms  wall {:9.1} ms  folder writes {}", self.name, self.cpu.len(), at(&self.cpu), at(&self.wall), self.writes)
    }
}

#[test]
#[ignore]
fn an_idle_exchange_on_a_phone_the_size_of_a_real_one() {
    let Some(base) = std::env::var_os("SIOUL_IDLE_BENCH").map(PathBuf::from) else { return };
    let _ = std::fs::remove_dir_all(&base);
    let folder = base.join("folder");
    std::fs::create_dir_all(folder.join("blobs")).unwrap();
    let mut noise = Noise(0x9e37_79b9_7f4a_7c15);
    let desk = Dev::new(&base, "desk", COMPUTER, "6f1d3a52-0c4e-4b8e-9a51-2f7c8d9e0a11");
    let phone = Dev::new(&base, "phone", PHONE, "7c41d09e-5b2a-4c3d-8e6f-1a2b3c4d5e6f");

    // The desk's settings, a few of them.
    std::fs::write(desk.roots.config.join("config.toml"), "theme = \"light\"\nnotes_view = \"list\"\n").unwrap();
    // The phone's texts log (10 MB of sealed lines, about 1.1 kB each) and its calls log.
    let texts = phone.path_of(TEXTS_LOG);
    std::fs::create_dir_all(&texts).unwrap();
    let lines: Vec<String> = (0..9_100).map(|_| noise.line(822)).collect();
    std::fs::write(texts.join(format!("{}.jsonl", phone.id)), lines.join("\n") + "\n").unwrap();
    let calls = phone.path_of(CALLS_LOG);
    std::fs::create_dir_all(&calls).unwrap();
    let lines: Vec<String> = (0..8_500).map(|_| noise.line(225)).collect();
    std::fs::write(calls.join(format!("{}.jsonl", phone.id)), lines.join("\n") + "\n").unwrap();
    // 6,000 sealed files in the folder, as many as the notes' versions and the texts' media.
    for _ in 0..6_000 {
        let name: String = (0..4).map(|_| format!("{:016x}", noise.next())).collect();
        std::fs::write(folder.join("blobs").join(name), noise.bytes(2_048)).unwrap();
    }

    // Both join; the desk in use, the phone put away.
    let mut now = clock_ms();
    desk.session(&folder, true, now);
    phone.session(&folder, true, now);
    desk.exchange(&folder, now);
    now += MINUTE_MS;
    let joined = std::time::Instant::now();
    phone.exchange(&folder, now);
    println!("the phone joined in {:.1} s", joined.elapsed().as_secs_f64());
    phone.session(&folder, false, now);
    for _ in 0..3 {
        now += MINUTE_MS;
        desk.exchange(&folder, now);
        now += MINUTE_MS;
        phone.exchange(&folder, now);
    }
    let sizes = |name: &str| std::fs::metadata(phone.memory.with_file_name(name)).map_or(0, |m| m.len());
    println!("files.json {:.1} MB, memory.json {:.1} kB, format {}", sizes("files.json") as f64 / 1e6, sizes("memory.json") as f64 / 1e3, Memory::load(&phone.memory, &phone.id).format);

    let mut cases = Vec::new();
    // A. Nothing changed anywhere: the phone's step every two minutes, the desk away.
    let mut idle = Case::new("A. nothing changed");
    for _ in 0..12 {
        now += 2 * MINUTE_MS;
        let before = listing(&folder);
        let outcome = idle.time(|| exchange(&Sharing { folder: &folder, computer: &phone.id, key: &KEY, memory: &phone.memory, files: false, hurry: None }, &phone.stores(), now).unwrap());
        phone.exported(&folder, &outcome, now);
        idle.writes += written(&before, &listing(&folder));
    }
    cases.push(idle);
    // B. The desk in use rewrote its entry (each minute), nothing else.
    let mut entry = Case::new("B. only the desk's entry changed");
    for _ in 0..12 {
        now += 2 * MINUTE_MS;
        crate::devices::change(&crate::devices::own_path(&desk.roots.state), Some((&folder, &KEY)), |e| e.exported(now - MINUTE_MS, None, (now - MINUTE_MS) / 1000)).unwrap();
        let before = listing(&folder);
        let outcome = entry.time(|| exchange(&Sharing { folder: &folder, computer: &phone.id, key: &KEY, memory: &phone.memory, files: false, hurry: None }, &phone.stores(), now).unwrap());
        phone.exported(&folder, &outcome, now);
        entry.writes += written(&before, &listing(&folder));
    }
    cases.push(entry);
    // C. One setting changed on the desk: one change comes in.
    let mut change = Case::new("C. one change from the desk");
    for n in 0..6 {
        now += MINUTE_MS;
        std::fs::write(desk.roots.config.join("config.toml"), format!("theme = \"light\"\nnotes_view = \"list\"\nweek_start = {n}\n")).unwrap();
        desk.exchange(&folder, now);
        now += MINUTE_MS;
        let before = listing(&folder);
        let outcome = change.time(|| exchange(&Sharing { folder: &folder, computer: &phone.id, key: &KEY, memory: &phone.memory, files: false, hurry: None }, &phone.stores(), now).unwrap());
        assert_eq!(outcome.received, 1, "{outcome:?}");
        phone.exported(&folder, &outcome, now);
        change.writes += written(&before, &listing(&folder));
    }
    cases.push(change);
    // E. Two hours of steps every two minutes, the desk in use (its entry each minute): the hour's
    // tidying among them. The mean (the thread's time is counted coarsely: a few milliseconds).
    let mut hours = Case::new("E. two hours, the desk in use (mean)");
    for _ in 0..60 {
        now += MINUTE_MS;
        crate::devices::change(&crate::devices::own_path(&desk.roots.state), Some((&folder, &KEY)), |e| e.exported(now, None, now / 1000)).unwrap();
        now += MINUTE_MS;
        crate::devices::change(&crate::devices::own_path(&desk.roots.state), Some((&folder, &KEY)), |e| e.exported(now, None, now / 1000)).unwrap();
        let before = listing(&folder);
        let (c, w) = (cpu_ns(), std::time::Instant::now());
        let outcome = exchange(&Sharing { folder: &folder, computer: &phone.id, key: &KEY, memory: &phone.memory, files: false, hurry: None }, &phone.stores(), now).unwrap();
        hours.cpu.push((cpu_ns() - c) as f64 / 1e6);
        hours.wall.push(w.elapsed().as_secs_f64() * 1e3);
        phone.exported(&folder, &outcome, now);
        hours.writes += written(&before, &listing(&folder));
    }
    hours.mean = true;
    cases.push(hours);
    // D. What the send asks before it sends (`remote::send`): the files this phone sealed.
    let mut sealed = Case::new("D. own_sealed, before each send");
    for _ in 0..12 {
        sealed.time(|| own_sealed(&phone.memory, &phone.id));
    }
    cases.push(sealed);
    // What the process keeps between exchanges, counted from what it holds (about: allocations rounded).
    let kept: usize = KEPT.lock().unwrap().get(&phone.memory).map_or(0, |k| {
        let m = &k.memory;
        m.entries.iter().map(|(key, known)| key.len() + known.w.len() + known.h.len() + known.b.len() + 120).sum::<usize>() + m.files.keys().map(|f| f.len() + 120).sum::<usize>() + m.sealed.mine.len() * 120 + k.text.len()
    });
    let lines: usize = LINES.lock().unwrap().iter().filter(|(path, _)| path.starts_with(base.join("phone"))).map(|(_, (_, _, lines))| lines.iter().map(|l| l.len() + 40).sum::<usize>()).sum();
    // F. The same hours in a process that keeps nothing between exchanges (a window's: `set_keeping`).
    let mut lean = Case::new("F. as E, nothing kept between exchanges (mean)");
    set_keeping(false);
    for _ in 0..30 {
        now += MINUTE_MS;
        crate::devices::change(&crate::devices::own_path(&desk.roots.state), Some((&folder, &KEY)), |e| e.exported(now, None, now / 1000)).unwrap();
        now += MINUTE_MS;
        let outcome = lean.time(|| exchange(&Sharing { folder: &folder, computer: &phone.id, key: &KEY, memory: &phone.memory, files: false, hurry: None }, &phone.stores(), now).unwrap());
        phone.exported(&folder, &outcome, now);
    }
    // One change from the desk there, read whole.
    let mut whole = Case::new("G. as C, nothing kept between exchanges");
    for n in 0..4 {
        now += MINUTE_MS;
        std::fs::write(desk.roots.config.join("config.toml"), format!("theme = \"dark\"\nweek_start = {n}\n")).unwrap();
        desk.exchange(&folder, now);
        now += MINUTE_MS;
        let outcome = whole.time(|| exchange(&Sharing { folder: &folder, computer: &phone.id, key: &KEY, memory: &phone.memory, files: false, hurry: None }, &phone.stores(), now).unwrap());
        phone.exported(&folder, &outcome, now);
    }
    set_keeping(true);
    lean.mean = true;
    cases.push(lean);
    cases.push(whole);
    let mut report: Vec<String> = cases.iter().map(Case::line).collect();
    report.push(format!("kept in the process between exchanges: the memory {:.1} MB, the logs of lines {:.1} MB", kept as f64 / 1e6, lines as f64 / 1e6));
    println!("{}", report.join("\n"));
    let _ = std::fs::write(base.join("idle-bench.txt"), report.join("\n") + "\n");
}
