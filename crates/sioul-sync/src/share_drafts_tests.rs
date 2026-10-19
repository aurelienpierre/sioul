// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The copies beside drafts through the sharing (batch B, 19 October 2026;
//! docs/client.md, "Encrypted messages you answer"): a copy that holds an
//! encrypted message's text in clear, as Sioul wrote it until then, is
//! encrypted again on each device that holds it, before its exchanges; a
//! device that cannot judge it (its message not in its mail, its draft not
//! encrypted) keeps it until the encrypted copy comes, and the version it
//! kept in clear among its earlier ones is then encrypted too; nothing of
//! the text stays in clear on either device (its earlier versions and what
//! it kept before sharing began included) nor in the folder; the draft
//! itself, edited on both devices meanwhile, keeps both versions; a copy
//! encrypted as it came travels as it is. Fiction only, stand-in keys.

use super::tests::{Computer, MINUTE, NOW, quick_key, scratch};
use super::*;
use crate::history;
use sioul_core::compose::Draft;
use sioul_core::draftcopies::{self, Judged, Sealed};
use sioul_core::pgp::{self, Keys, SessionKeys, Unlock};

const SECRET: &str = "The secret clause is on page four.";

fn lease(id: &str) -> String {
    format!("From: Jane <jane@example.org>\r\nTo: Noa <noa@example.org>\r\nSubject: The lease\r\nMessage-ID: <{id}@example.org>\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{SECRET}\r\n")
}

fn draft(id: &str, encrypt: bool, body: &str) -> String {
    toml::to_string(&Draft { id: id.into(), account: "perso".into(), encrypt, body: body.into(), ..Draft::default() }).unwrap()
}

/// Every file under `dir` whose bytes hold `text`.
fn holding(dir: &Path, text: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(here) = stack.pop() {
        for entry in std::fs::read_dir(&here).into_iter().flatten().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if std::fs::read(&path).is_ok_and(|bytes| bytes.windows(text.len()).any(|w| w == text.as_bytes())) {
                out.push(path);
            }
        }
    }
    out
}

/// What a copy reads once opened with your key.
fn opened(raw: &[u8], keys: &Keys) -> String {
    let sessions = SessionKeys::new();
    let (opened, _) = pgp::open(raw, keys, &Unlock::new(&|_| None, &sessions)).expect("encrypted");
    String::from_utf8_lossy(&opened.expect("your key opens it")).to_string()
}

#[test]
fn clear_copies_are_encrypted_on_every_device_and_none_stays() {
    let base = scratch("draftcopies");
    let folder = base.join("Nextcloud").join("sioul-shared");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let keys = Keys { own: pgp::certificates_in(&pgp::stand_in_key("Noa <noa@example.org>").unwrap()), ..Keys::default() };
    // The desk's mail: two leases came encrypted; Sioul (until 19 October)
    // kept their decrypted text beside two answers, and the sharing carried it.
    let mail = base.join("desk-mail");
    std::fs::create_dir_all(&mail).unwrap();
    for id in ["lease", "lease2"] {
        std::fs::write(mail.join(id), pgp::seal_message(lease(id).as_bytes(), &[&keys.own[0]]).unwrap()).unwrap();
    }
    desk.write("data/drafts/a1.toml", &draft("a1", true, "Fine by me."));
    desk.write("data/drafts/a1.eml", &lease("lease"));
    desk.write("data/drafts/a2.toml", &draft("a2", false, "Let me think."));
    desk.write("data/drafts/a2.eml", &lease("lease2"));
    // And a copy encrypted as it came, as Sioul keeps them now.
    let as_it_came = pgp::seal_message(lease("lease3").as_bytes(), &[&keys.own[0]]).unwrap();
    std::fs::write(desk.path("data/drafts/a3.eml"), &as_it_came).unwrap();
    desk.write("data/drafts/a3.toml", &draft("a3", true, "Noted."));
    desk.exchange(&folder, &key, NOW);
    laptop.exchange(&folder, &key, NOW + MINUTE);
    assert_eq!(laptop.read("data/drafts/a2.eml"), lease("lease2"), "the leak: the text in clear on both");

    // The new Sioul on the laptop first, whose mail holds neither lease: a1's
    // draft is encrypted, its copy too; a2's is left to the desk. The laptop
    // edits a1 meanwhile.
    let first = draftcopies::seal_beside(&laptop.path("data/drafts"), &keys, &mut |_| None, &mut Judged::default(), &replacements_hold(&laptop.memory));
    assert_eq!(first, Sealed { copies: 1, unknown: 1, ..Sealed::default() });
    laptop.write("data/drafts/a1.toml", &draft("a1", true, "From the laptop."));
    laptop.exchange(&folder, &key, NOW + 2 * MINUTE);

    // Then the desk, before its exchange, whose mail has both: each copy
    // encrypted. The desk edits a1 too, later.
    let mut find = |id: &str| match id {
        "lease@example.org" => Some(mail.join("lease")),
        "lease2@example.org" => Some(mail.join("lease2")),
        _ => None,
    };
    let second = draftcopies::seal_beside(&desk.path("data/drafts"), &keys, &mut find, &mut Judged::default(), &replacements_hold(&desk.memory));
    assert_eq!(second, Sealed { copies: 2, ..Sealed::default() });
    desk.write("data/drafts/a1.toml", &draft("a1", true, "From the desk."));
    desk.exchange(&folder, &key, NOW + 3 * MINUTE);
    laptop.exchange(&folder, &key, NOW + 4 * MINUTE);
    desk.exchange(&folder, &key, NOW + 5 * MINUTE);

    // The laptop kept its clear a2 among its earlier versions when the
    // desk's encrypted copy came; the desk, its clear copies in what it kept
    // before sharing began: their next looks encrypt them.
    let versions = |computer: &Computer| {
        let mut kept = history::draft_copies(&history::root(&computer.memory));
        kept.extend(draft_copies_kept_before_sharing(&computer.memory));
        kept
    };
    assert!(versions(&laptop).iter().any(|v| std::fs::read(v).is_ok_and(|b| !pgp::is_encrypted(&b))), "the clear version kept on the laptop");
    let mut judged = Judged::default();
    let third = draftcopies::seal_versions(&versions(&laptop), &keys, &mut judged, &replacements_hold(&laptop.memory));
    assert_eq!(third, Sealed { versions: 1, ..Sealed::default() });
    let fourth = draftcopies::seal_versions(&versions(&desk), &keys, &mut Judged::default(), &replacements_hold(&desk.memory));
    assert_eq!(fourth, Sealed { versions: 2, ..Sealed::default() }, "a1 and a2 as they were before sharing");
    assert_eq!(draftcopies::seal_beside(&laptop.path("data/drafts"), &keys, &mut |_| None, &mut judged, &replacements_hold(&laptop.memory)), Sealed::default());

    // Nothing of the text in clear, on either device, in the folder, in the history.
    assert_eq!(holding(&base, SECRET), Vec::<PathBuf>::new());
    // Each copy encrypted on both, opening with your key to what it said;
    // the one encrypted as it came, as it came.
    for computer in [&desk, &laptop] {
        for id in ["a1", "a2", "a3"] {
            let copy = std::fs::read(computer.path(&format!("data/drafts/{id}.eml"))).unwrap();
            assert!(opened(&copy, &keys).contains(SECRET), "{id}");
        }
        assert_eq!(std::fs::read(computer.path("data/drafts/a3.eml")).unwrap(), as_it_came);
        // The drafts themselves: the later edit in place on both.
        assert!(computer.read("data/drafts/a1.toml").contains("From the desk."));
        assert!(computer.read("data/drafts/a2.toml").contains("Let me think."));
    }
    // The laptop's edit kept among its earlier versions: both kept.
    let root = history::root(&laptop.memory);
    let kept = history::versions(&root, "drafts", "data/drafts/a1.toml");
    assert!(
        kept.iter().any(|v| matches!(history::version(&root, "drafts", "data/drafts/a1.toml", &v.stamp), Some(history::Kept::Copy(path)) if std::fs::read_to_string(&path).unwrap().contains("From the laptop."))),
        "{kept:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Every file under `dir` that is all zeros (and not empty).
fn zeroed(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(here) = stack.pop() {
        for entry in std::fs::read_dir(&here).into_iter().flatten().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if std::fs::read(&path).is_ok_and(|bytes| !bytes.is_empty() && bytes.iter().all(|&b| b == 0)) {
                out.push(path);
            }
        }
    }
    out
}

/// Exchanges running while the migration replaces copies (main's final
/// pass, 19 October 2026: a reader holding a copy being replaced read the
/// zeros written over it, and an exchange that did so would seal and send
/// them as the shared version). Each copy takes its place under the exchange
/// lock, a short hold at a time, its old bytes overwritten once the lock is
/// let go: no exchange reads one meanwhile, and a reader outside the lock
/// (Send, the writing window) reads whole or not at all. At the end, nothing
/// anywhere is zeros, nothing holds the text in clear, the sharing folder
/// included, and the other device's copies open to what they said. The
/// meeting itself lasts microseconds, too brief to force from here (this
/// test passed even with both guards taken out): it is made to happen in
/// `filelock::tests::a_file_replaced_between_its_opening_and_its_reading_is_read_again`,
/// and the wait in `no_copy_is_replaced_while_an_exchange_holds_the_lock`.
#[test]
fn exchanges_running_while_copies_are_replaced_never_send_zeros() {
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    let base = scratch("draftcopies-race");
    let folder = base.join("Nextcloud").join("sioul-shared");
    let key = quick_key(&folder, "four words make a passphrase").unwrap();
    let (desk, laptop) = (Computer::new(&base, "desk"), Computer::new(&base, "laptop"));
    let keys = Keys { own: pgp::certificates_in(&pgp::stand_in_key("Noa <noa@example.org>").unwrap()), ..Keys::default() };
    const COPIES: usize = 24;
    // Large copies (an answer with its attachments): each read, and each
    // overwrite with zeros, takes long enough for the two to meet.
    let large = |id: &str| format!("{}{}\r\n", lease(id), "The lease runs three years. ".repeat(20_000));
    for n in 0..COPIES {
        desk.write(&format!("data/drafts/c{n}.toml"), &draft(&format!("c{n}"), true, "Fine by me."));
        desk.write(&format!("data/drafts/c{n}.eml"), &large(&format!("race{n}")));
    }
    desk.exchange(&folder, &key, NOW);
    laptop.exchange(&folder, &key, NOW + MINUTE);
    let (stop, made) = (AtomicBool::new(false), AtomicI64::new(0));
    let (done, during) = std::thread::scope(|scope| {
        // The desk's exchanges, one after the other, all along.
        let running = scope.spawn(|| {
            while !stop.load(Ordering::Relaxed) {
                let n = made.load(Ordering::Relaxed);
                desk.exchange(&folder, &key, NOW + (2 + n) * MINUTE);
                made.store(n + 1, Ordering::Relaxed);
            }
        });
        // The migration starts once an exchange went by, each copy dated
        // anew first, so that the exchanges read them all again meanwhile.
        while made.load(Ordering::Relaxed) == 0 {
            std::thread::yield_now();
        }
        for n in 0..COPIES {
            let copy = std::fs::File::options().write(true).open(desk.path(&format!("data/drafts/c{n}.eml"))).unwrap();
            copy.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(1)).unwrap();
        }
        // A reader outside the lock, as Send and the writing window read.
        let reader = scope.spawn(|| {
            while !stop.load(Ordering::Relaxed) {
                for n in 0..COPIES {
                    if let Ok(bytes) = sioul_core::filelock::read_whole(&desk.path(&format!("data/drafts/c{n}.eml"))) {
                        assert!(!bytes.is_empty() && bytes.iter().any(|&b| b != 0), "c{n} read as zeros");
                    }
                }
            }
        });
        let before = made.load(Ordering::Relaxed);
        let done = draftcopies::seal_beside(&desk.path("data/drafts"), &keys, &mut |_| None, &mut Judged::default(), &replacements_hold(&desk.memory));
        let during = made.load(Ordering::Relaxed) - before;
        stop.store(true, Ordering::Relaxed);
        reader.join().unwrap();
        running.join().unwrap();
        (done, during)
    });
    assert_eq!(done, Sealed { copies: COPIES, ..Sealed::default() });
    assert!(during >= 2, "{during} exchanges went by while the copies were replaced");
    // What came of it on both devices; the laptop's clear versions kept
    // among its earlier ones, and the desk's kept before sharing began,
    // encrypted at their next looks.
    desk.exchange(&folder, &key, NOW + 1000 * MINUTE);
    laptop.exchange(&folder, &key, NOW + 1001 * MINUTE);
    desk.exchange(&folder, &key, NOW + 1002 * MINUTE);
    for computer in [&desk, &laptop] {
        let mut kept = history::draft_copies(&history::root(&computer.memory));
        kept.extend(draft_copies_kept_before_sharing(&computer.memory));
        draftcopies::seal_versions(&kept, &keys, &mut Judged::default(), &replacements_hold(&computer.memory));
    }
    assert_eq!(zeroed(&base), Vec::<PathBuf>::new());
    assert_eq!(holding(&base, SECRET), Vec::<PathBuf>::new());
    for n in 0..COPIES {
        let copy = std::fs::read(laptop.path(&format!("data/drafts/c{n}.eml"))).unwrap();
        assert!(opened(&copy, &keys).contains(SECRET), "c{n} on the laptop");
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// No copy takes its place while an exchange runs: the migration waits for
/// the exchange lock, which an exchange holds from its start to its end, so
/// that no exchange reads, seals or sends a copy while it is replaced. Its
/// new bytes wait beside the copy meanwhile, under a hidden name the sharing
/// passes over; then each copy takes its place, and nothing is left beside.
#[test]
fn no_copy_is_replaced_while_an_exchange_holds_the_lock() {
    let base = scratch("draftcopies-lock");
    let desk = Computer::new(&base, "desk");
    std::fs::create_dir_all(desk.memory.parent().unwrap()).unwrap();
    let keys = Keys { own: pgp::certificates_in(&pgp::stand_in_key("Noa <noa@example.org>").unwrap()), ..Keys::default() };
    for n in 0..5 {
        desk.write(&format!("data/drafts/c{n}.toml"), &draft(&format!("c{n}"), true, "Fine by me."));
        desk.write(&format!("data/drafts/c{n}.eml"), &lease(&format!("lock{n}")));
    }
    let exchange = exchange_lock(&desk.memory, true).unwrap().expect("the exchange's lock");
    std::thread::scope(|scope| {
        let migration = scope.spawn(|| draftcopies::seal_beside(&desk.path("data/drafts"), &keys, &mut |_| None, &mut Judged::default(), &replacements_hold(&desk.memory)));
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(!migration.is_finished(), "waits for the exchange");
        for n in 0..5 {
            assert_eq!(desk.read(&format!("data/drafts/c{n}.eml")), lease(&format!("lock{n}")), "c{n} untouched while the exchange runs");
        }
        drop(exchange);
        assert_eq!(migration.join().unwrap(), Sealed { copies: 5, ..Sealed::default() });
    });
    for n in 0..5 {
        let copy = std::fs::read(desk.path(&format!("data/drafts/c{n}.eml"))).unwrap();
        assert!(opened(&copy, &keys).contains(SECRET), "c{n}");
    }
    let hidden: Vec<String> = std::fs::read_dir(desk.path("data/drafts")).unwrap().filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with('.')).collect();
    assert!(hidden.is_empty(), "{hidden:?}");
    let _ = std::fs::remove_dir_all(&base);
}
