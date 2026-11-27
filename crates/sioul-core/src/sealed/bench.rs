// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sealed store's bench (docs/store.md, "Bench"), beside the sharing's
//! idle bench: a made-up profile of a real one's size (about 6 000 files and
//! 600 MB, one folder of 3 700 messages), sealed, opened, listed, changed and
//! checked. Invented data only. Run on demand, in a folder given by
//! `SIOUL_STORE_BENCH_DIR` (else the system's temporary folder: on Linux
//! often memory, which leaves the disk out of the figures):
//!
//!     SIOUL_STORE_BENCH_DIR=/some/disk/folder cargo test --release -p sioul-core --lib sealed::bench -- --ignored --nocapture

use super::*;
use std::path::Path;
use std::time::Instant;

const WORDS: &[&str] = &["bonjour", "votre", "facture", "rendez-vous", "merci", "le", "la", "de", "pour", "dossier", "mutuelle", "remboursement", "semaine", "prochaine", "cordialement", "the", "your", "invoice", "meeting", "thanks", "attached", "please", "find", "account", "payment"];

/// Bytes like a file's: text of words (compresses as mail's text does), or noise (as attachments and media).
fn content(n: usize, seed: u64, text: bool) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let mut out = Vec::with_capacity(n + 16);
    if text {
        while out.len() < n {
            out.extend_from_slice(WORDS[(next() % WORDS.len() as u64) as usize].as_bytes());
            out.push(if next() % 12 == 0 { b'\n' } else { b' ' });
        }
    } else {
        // Base64-like noise: an attachment as mail carries it.
        const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        while out.len() < n {
            out.push(B64[(next() % 64) as usize]);
            if out.len() % 77 == 76 {
                out.push(b'\n');
            }
        }
    }
    out.truncate(n);
    out
}

/// The made-up profile: (path, size, text or not), about 6 000 files and 600 MB.
fn profile() -> Vec<(String, usize, bool)> {
    let mut files = Vec::new();
    let mut seed = 1u64;
    let mut pick = |lo: usize, hi: usize| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        lo + (seed >> 33) as usize % (hi - lo)
    };
    // One folder of 3 700 messages, 46 KB on average (171 MB): most short, some with attachments.
    for i in 0..3700 {
        let attached = i % 5 == 0;
        let size = if attached { pick(80_000, 300_000) } else { pick(2_000, 22_000) };
        files.push((format!("data/mail/acc/INBOX/17000{i:05}.M{i}P1.host"), size, !attached));
    }
    for i in 0..40 {
        files.push((format!("data/texts/media/{i}.bin"), 3_900_000, false));
    }
    for i in 0..4 {
        files.push((format!("data/spam/corpus-{i}.txt"), 25_500_000, true));
    }
    for i in 0..585 {
        files.push((format!("data/calendars/perso/{i:04}.ics"), pick(1_500, 6_500), true));
    }
    for i in 0..268 {
        files.push((format!("data/contacts/main/{i:04}.vcf"), pick(2_000, 10_000), true));
    }
    for i in 0..300 {
        files.push((format!("state/share/history/notes/{i:03}"), pick(200_000, 740_000), true));
    }
    for i in 0..300 {
        files.push((format!("state/misc/{i:03}.toml"), pick(200, 4_000), true));
    }
    for i in 0..300 {
        files.push((format!("cache/index/{i:03}"), pick(50_000, 410_000), i % 2 == 0));
    }
    for i in 0..488 {
        files.push((format!("data/records/r{i:03}.toml"), pick(500, 3_000), true));
    }
    for i in 0..15 {
        files.push((format!("config/c{i:02}.toml"), pick(500, 12_000), true));
    }
    files
}

fn rate(bytes: u64, seconds: f64) -> String {
    format!("{:.0} MB/s", bytes as f64 / 1e6 / seconds.max(1e-9))
}

#[test]
#[ignore = "a bench: run on demand (docs/store.md, \"Bench\")"]
fn bench() {
    let root = std::env::var_os("SIOUL_STORE_BENCH_DIR").map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let base = root.join(format!("sioul-store-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let folders = Folders::under(&base);
    let files = profile();
    let total: u64 = files.iter().map(|(_, size, _)| *size as u64).sum();
    println!("profile: {} files, {:.0} MB, in {}", files.len(), total as f64 / 1e6, root.display());

    // Sealing: every file written once (each flushed, as a write is).
    let store = Store::create(&folders, &RootKey::test(), Options::default()).unwrap();
    let started = Instant::now();
    for (i, (path, size, text)) in files.iter().enumerate() {
        store.write(&LogicalPath::new(path).unwrap(), &content(*size, i as u64, *text), How::default()).unwrap();
    }
    let sealing = started.elapsed().as_secs_f64();
    let sealed_bytes: u64 = Root::ALL.iter().map(|r| du(&folders.sealed(*r))).sum();
    println!("sealing: {sealing:.1} s, {} plain, {:.0} files/s; on the disk {:.0} MB ({:.0} % of plain)", rate(total, sealing), files.len() as f64 / sealing, sealed_bytes as f64 / 1e6, sealed_bytes as f64 * 100.0 / total as f64);

    // Opening: every file read whole, in a store opened anew (manifests read from the disk; the files from the system's cache).
    let store = Store::open(&folders, &RootKey::test(), Options::default()).unwrap();
    let started = Instant::now();
    let mut read = 0u64;
    for (path, _, _) in &files {
        read += store.read(&LogicalPath::new(path).unwrap()).unwrap().unwrap().bytes.len() as u64;
    }
    let opening = started.elapsed().as_secs_f64();
    assert_eq!(read, total);
    println!("opening: {opening:.1} s, {}, {:.0} files/s", rate(total, opening), files.len() as f64 / opening);

    // Listing the folder of 3 700 messages from its manifest: first in a process (read from the disk), then again (kept).
    let inbox = LogicalPath::new("data/mail/acc/INBOX").unwrap();
    let fresh = Store::open(&folders, &RootKey::test(), Options::default()).unwrap();
    let started = Instant::now();
    let listed = fresh.list(&inbox).unwrap();
    let first = started.elapsed().as_secs_f64();
    let started = Instant::now();
    for _ in 0..20 {
        assert_eq!(fresh.list(&inbox).unwrap().len(), listed.len());
    }
    let again = started.elapsed().as_secs_f64() / 20.0;
    let manifest_size = std::fs::metadata(fresh.manifest_place(&inbox, 0)).map_or(0, |m| m.len());
    println!("listing {} messages: {:.1} ms first in a process, {:.2} ms once kept; its manifest {:.0} KB sealed", listed.len(), first * 1e3, again * 1e3, manifest_size as f64 / 1e3);

    // One flag changed through the journal, a hundred times (some folds among them).
    let started = Instant::now();
    for i in 0..100 {
        let message = &listed[i * 37 % listed.len()].path;
        fresh.set_attrs(message, &[("flags", Some(if i % 2 == 0 { "S" } else { "RS" }))]).unwrap();
    }
    let flag = started.elapsed().as_secs_f64() / 100.0;
    // And listed by another process after them: the journal's new lines alone are read.
    let started = Instant::now();
    store.list(&inbox).unwrap();
    let after = started.elapsed().as_secs_f64();
    println!("one flag changed: {:.2} ms (one journal line, flushed); listed after by another process: {:.1} ms", flag * 1e3, after * 1e3);

    // A write with n = 2: a configuration file and a message rewritten, a hundred times each.
    for (label, path, size) in [("a configuration file (4 KB)", "config/c00.toml", 4_000), ("a message (46 KB)", "data/mail/acc/INBOX/1700000001.M1P1.host", 46_000)] {
        let path = LogicalPath::new(path).unwrap();
        let started = Instant::now();
        for i in 0..100 {
            fresh.write(&path, &content(size, 10_000 + i, true), How::default()).unwrap();
        }
        println!("a write with n = 2, {label}: {:.2} ms", started.elapsed().as_secs_f64() * 10.0);
    }

    // The check's full pass: every piece of every copy opened.
    let started = Instant::now();
    let report = fresh.check(false).unwrap();
    let checking = started.elapsed().as_secs_f64();
    let whole: usize = report.folders.iter().map(|f| f.whole).sum();
    let counted: usize = report.folders.iter().map(|f| f.files).sum();
    assert_eq!(whole, counted, "{report:?}");
    println!("check, full pass: {checking:.1} s for {counted} files and their versions, {}", rate(sealed_bytes, checking));
    let _ = std::fs::remove_dir_all(&base);
}

/// What a folder takes on the disk, its files' sizes summed.
fn du(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok) {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(_) => total += entry.metadata().map_or(0, |m| m.len()),
                Err(_) => {}
            }
        }
    }
    total
}
