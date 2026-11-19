// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The move of Sioul's lists out of the notes folder (`storage`), on
//! stand-in folders made from the old layout's fixture
//! (`tests/fixtures/old-layout`). Fiction only.

use super::*;
use std::collections::BTreeMap;

const NOW: i64 = 1_795_000_000;

/// A stand-in device: the fixture's notes folder and the folders a move fills.
struct Stand {
    base: PathBuf,
    folders: Folders,
}

impl Drop for Stand {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            copy_tree(&path, &to.join(entry.file_name()));
        } else {
            std::fs::copy(&path, to.join(entry.file_name())).unwrap();
        }
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("old-layout")
}

/// The fixture copied to a stand-in folder of its own, its absolute paths
/// written in, a lock file beside two lists as Sioul's writers leave them.
fn stand(name: &str) -> Stand {
    let base = std::env::temp_dir().join(format!("sioul-storage-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    copy_tree(&fixture().join("notes"), &base.join("notes"));
    copy_tree(&fixture().join("elsewhere"), &base.join("elsewhere"));
    let notes = base.join("notes");
    std::fs::write(notes.join(".sioul-budgets.toml.lock"), "").unwrap();
    std::fs::write(notes.join("letters").join(".letters.toml.lock"), "").unwrap();
    let fill = |path: &Path, pairs: &[(&str, PathBuf)]| {
        let mut text = std::fs::read_to_string(path).unwrap();
        for (mark, value) in pairs {
            text = text.replace(mark, &value.display().to_string().replace('\\', "/"));
        }
        std::fs::write(path, text).unwrap();
    };
    fill(&notes.join("sioul-papers.toml"), &[("ABSOLUTE-ELSEWHERE", base.join("elsewhere").join("Tax-notice.pdf")), ("ABSOLUTE-INSIDE", notes.join("papers").join("Power.pdf"))]);
    fill(&notes.join("letters").join("letters.toml"), &[("ABSOLUTE-LINKED", base.join("elsewhere").join("Linked-scan.pdf"))]);
    let folders = Folders {
        notes,
        records: base.join("data").join(RECORDS),
        papers: base.join("Documents").join("Papers"),
        letters: base.join("Documents").join("Letters"),
        kept: base.join("data").join(KEPT),
        state: base.join("state"),
    };
    Stand { base, folders }
}

/// Every file below `dir`, hidden ones too, by its path from it, with its bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(read) = std::fs::read_dir(dir) else { return };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn places_of(stand: &Stand) -> Places {
    let f = &stand.folders;
    Places::apart(&f.records, Some(&f.notes), &f.papers, &f.letters)
}

/// What a moved device reads, checked whole: every list, every paper's and
/// letter's file found, a project's notes, a budget line's message, the
/// bank's lines by their keys.
fn reads_moved(stand: &Stand) {
    let f = &stand.folders;
    let places = places_of(stand);
    let wallet = crate::papers::Wallet::load_at(&places).unwrap();
    assert_eq!(wallet.papers.len(), 4);
    for paper in &wallet.papers {
        let file = wallet.file_path(paper).unwrap();
        assert!(file.is_file(), "{}: {}", paper.id, file.display());
    }
    assert_eq!(wallet.get("passport").unwrap().file, "Passport.pdf");
    assert_eq!(wallet.get("card").unwrap().file, "ids/Card.png");
    assert_eq!(wallet.get("power-contract").unwrap().file, "Power.pdf", "an absolute path in the old papers' folder, named from the new one");
    assert!(wallet.get("tax-notice").unwrap().file.ends_with("elsewhere/Tax-notice.pdf"), "a paper kept elsewhere stays where it is");
    let letters = crate::letters::Letters::load_at(&places).unwrap();
    assert_eq!(letters.list.len(), 2);
    for letter in &letters.list {
        assert!(letters.file_path(letter).is_file(), "{}: {}", letter.id, letters.file_path(letter).display());
    }
    assert_eq!(letters.get("2026-10-12-caf").unwrap().file, "2026/2026-10-12 caf other.pdf");
    let linked = letters.get("2026-10-20-linked").unwrap();
    assert!(linked.linked && linked.file.ends_with("elsewhere/Linked-scan.pdf"), "a linked scan stays where it is");
    assert_eq!(letters.text(letters.get("2026-10-12-caf").unwrap()), "Madame, votre dossier est complet.\n");
    let projects = crate::projects::ProjectStore::open(&places).unwrap();
    assert_eq!(projects.projects.len(), 2);
    assert!(projects.missing_files().is_empty(), "a project's notes named from the notes folder: {:?}", projects.missing_files());
    let ledger = crate::budget::Ledger::load(&f.records).unwrap();
    assert_eq!(ledger.lines[0].links, ["mid:rent-2026-10@landlord.example.org"]);
    let bank = crate::bank::Bank::load(&f.records).unwrap();
    assert_eq!(bank.movements.iter().map(|m| (m.account.as_str(), m.id.as_str())).collect::<Vec<_>>(), [("main", "fitid-2026-10-01")]);
    assert_eq!(crate::clients::Clients::load(&f.records).unwrap().list.len(), 1);
    assert_eq!(crate::contracts::Contracts::load(&f.records).unwrap().list[0].paper, "power-contract");
    assert_eq!(crate::movements::Store::load(&f.records).unwrap().names.len(), 1);
    // Each list byte for byte as it was, but the two whose paths were named anew.
    for list in [crate::projects::FILE, crate::clients::FILE, crate::budget::LEDGER, crate::bank::MANIFEST, crate::movements::STORE, crate::contracts::MANIFEST, "sioul-cases.toml.before-rename"] {
        if f.notes.join(list).exists() {
            assert_eq!(std::fs::read(f.records.join(list)).unwrap(), std::fs::read(f.notes.join(list)).unwrap(), "{list}");
        }
    }
    assert!(text(&f.records.join(crate::papers::MANIFEST)).starts_with("# Your papers"), "comments kept");
}

#[test]
fn a_move_copies_checks_keeps_names_and_switches() {
    let stand = stand("whole");
    let before = tree(&stand.folders.notes);
    let moved = move_now(&stand.folders, NOW, || Ok(())).unwrap();
    assert_eq!((moved.lists, moved.papers, moved.letters, moved.found), (10, 3, 2, 0), "{moved:?}");
    assert_eq!(moved.named, 4, "passport, card, the contract's and the letter's paths");
    assert!(Storage::read(&stand.folders.state).is_moved());
    assert_eq!(Storage::read(&stand.folders.state).from_path().unwrap(), stand.folders.notes);
    // The old files untouched, hidden ones too.
    assert_eq!(tree(&stand.folders.notes), before);
    reads_moved(&stand);
    // A dated copy of every file moved, in the data folder.
    let kept = stand.folders.kept.join(day_of(NOW));
    assert_eq!(std::fs::read(kept.join("papers/ids/Card.png")).unwrap(), std::fs::read(stand.folders.notes.join("papers/ids/Card.png")).unwrap());
    assert_eq!(std::fs::read(kept.join("letters/letters.toml")).unwrap(), std::fs::read(stand.folders.notes.join("letters/letters.toml")).unwrap());
    // The letters' inbox moved with its scan waiting; no lock, no hidden file copied.
    assert!(stand.folders.letters.join("inbox/scan-0001.pdf").is_file());
    assert!(!tree(&stand.folders.records).keys().any(|k| k.split('/').any(|p| p.starts_with('.'))), "{:?}", tree(&stand.folders.records).keys().collect::<Vec<_>>());
    // Moved already: nothing done again.
    assert_eq!(move_now(&stand.folders, NOW + 60, || Ok(())).unwrap_err().code, "moved-already");
}

/// Killed at each of its points, one after the other, then run again: it
/// finishes, every file once, nothing doubled, and before the switch the
/// device still reads its old places.
#[test]
fn each_point_stopped_then_run_again_finishes_once() {
    let mut k = 1;
    loop {
        let stand = stand(&format!("stop-{k}"));
        let before = tree(&stand.folders.notes);
        STOP_AT.with(|stop| stop.set(k));
        let stopped = prepare(&stand.folders, NOW).and_then(|(mut journal, _)| switch(&stand.folders, &mut journal, NOW));
        let left = STOP_AT.with(std::cell::Cell::get);
        STOP_AT.with(|stop| stop.set(0));
        if stopped.is_ok() {
            assert!(left > 0 && k > 20, "every point stopped at once: {k}");
            break;
        }
        let stopped = stopped.unwrap_err();
        assert_eq!(stopped.code, "stopped", "{k}: {stopped:?}");
        assert_eq!(tree(&stand.folders.notes), before, "{k}: the old places as they were");
        if stopped.what == "closed" {
            // The journal closed, the state file not written yet: the move made, adopted
            // from the journal once its copies are all there; nothing done again.
            assert!(Storage::read(&stand.folders.state).adopted, "{k}");
            assert_eq!(move_now(&stand.folders, NOW + 60, || Ok(())).unwrap_err().code, "moved-already", "{k}");
            assert!(!Storage::read(&stand.folders.state).adopted, "{k}: the state file written again");
            reads_moved(&stand);
            k += 1;
            continue;
        }
        assert!(!Storage::read(&stand.folders.state).is_moved(), "{k}: not switched");
        let moved = move_now(&stand.folders, NOW + 60, || Ok(())).unwrap_or_else(|e| panic!("{k}: {e}"));
        assert_eq!(moved.renamed, Vec::<(String, String)>::new(), "{k}: no file given a second name");
        reads_moved(&stand);
        let papers: Vec<String> = tree(&stand.folders.papers).into_keys().collect();
        assert_eq!(papers, ["Passport.pdf", "Power.pdf", "ids/Card.png"], "{k}: once each, nothing beside");
        let letters: Vec<String> = tree(&stand.folders.letters).into_keys().collect();
        assert_eq!(letters, ["2026/2026-10-12 caf other.pdf", "inbox/scan-0001.pdf"], "{k}");
        let records: Vec<String> = tree(&stand.folders.records).into_keys().collect();
        assert_eq!(records.len(), 10, "{k}: {records:?}");
        assert_eq!(Journal::read(&stand.folders.state).unwrap().files.len(), 15, "{k}");
        assert_eq!(tree(&stand.folders.notes), before, "{k}");
        k += 1;
    }
}

/// A failure before the switch takes back what the move made: the old
/// places alone are read, as before; nothing of the person's is touched.
#[test]
fn a_failure_before_the_switch_changes_nothing() {
    let stand = stand("fails");
    // Another file of Sioul's lists at its new place: refused, nothing switched.
    std::fs::create_dir_all(&stand.folders.records).unwrap();
    std::fs::write(stand.folders.records.join(crate::budget::LEDGER), "[[budget]]\nid = \"other\"\n").unwrap();
    std::fs::create_dir_all(&stand.folders.papers).unwrap();
    std::fs::write(stand.folders.papers.join("Mine.pdf"), "the person's own").unwrap();
    let before = tree(&stand.folders.notes);
    let fault = move_now(&stand.folders, NOW, || Ok(())).unwrap_err();
    assert_eq!(fault.code, "records-taken", "{fault}");
    assert!(!Storage::read(&stand.folders.state).is_moved());
    assert!(Journal::read(&stand.folders.state).is_none(), "its journal gone with what it made");
    assert_eq!(tree(&stand.folders.notes), before);
    assert_eq!(tree(&stand.folders.papers).into_keys().collect::<Vec<_>>(), ["Mine.pdf"], "the person's file alone stays");
    assert_eq!(tree(&stand.folders.records).into_keys().collect::<Vec<_>>(), [crate::budget::LEDGER]);
    // The hold refused (an exchange running): the same.
    std::fs::remove_file(stand.folders.records.join(crate::budget::LEDGER)).unwrap();
    let fault = move_now(&stand.folders, NOW, || Err::<(), _>("busy".to_string())).unwrap_err();
    assert_eq!(fault.code, "busy");
    assert!(!Storage::read(&stand.folders.state).is_moved());
    assert_eq!(tree(&stand.folders.papers).into_keys().collect::<Vec<_>>(), ["Mine.pdf"]);
    assert!(tree(&stand.folders.records).is_empty());
    // Then it goes.
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    reads_moved(&stand);
}

/// A file of the person's where a paper goes: kept, the paper's copy named
/// beside it, and the paper's record names that copy.
#[test]
fn a_file_of_the_persons_at_its_place_is_kept_and_the_copy_named_beside() {
    let stand = stand("taken");
    std::fs::create_dir_all(&stand.folders.papers).unwrap();
    std::fs::write(stand.folders.papers.join("Passport.pdf"), "the person's own passport scan").unwrap();
    let moved = move_now(&stand.folders, NOW, || Ok(())).unwrap();
    assert_eq!(moved.renamed.len(), 1, "{moved:?}");
    assert_eq!(text(&stand.folders.papers.join("Passport.pdf")), "the person's own passport scan");
    let wallet = crate::papers::Wallet::load_at(&places_of(&stand)).unwrap();
    assert_eq!(wallet.get("passport").unwrap().file, "Passport-2.pdf");
    assert_eq!(std::fs::read(wallet.file_path(wallet.get("passport").unwrap()).unwrap()).unwrap(), std::fs::read(stand.folders.notes.join("papers/Passport.pdf")).unwrap());
}

#[test]
fn a_link_is_refused_calmly() {
    #[cfg(unix)]
    {
        let stand = stand("link");
        std::os::unix::fs::symlink(stand.base.join("elsewhere").join("Tax-notice.pdf"), stand.folders.notes.join("papers").join("Linked.pdf")).unwrap();
        let fault = move_now(&stand.folders, NOW, || Ok(())).unwrap_err();
        assert_eq!(fault.code, "link");
        assert!(!Storage::read(&stand.folders.state).is_moved());
        assert!(tree(&stand.folders.papers).is_empty());
    }
}

/// The folders chosen: never inside the notes folder, nor one inside the other.
#[test]
fn folders_inside_the_notes_or_each_other_are_refused() {
    let stand = stand("folders");
    let inside = Folders { papers: stand.folders.notes.join("Papers"), ..stand.folders.clone() };
    assert_eq!(check_folders(&inside).unwrap_err().code, "folder-in-notes");
    let around = Folders { letters: stand.base.clone(), ..stand.folders.clone() };
    assert!(check_folders(&around).is_err());
    let same = Folders { letters: stand.folders.papers.clone(), ..stand.folders.clone() };
    assert_eq!(check_folders(&same).unwrap_err().code, "folders-same");
    let relative = Folders { papers: PathBuf::from("Papers"), ..stand.folders.clone() };
    assert_eq!(check_folders(&relative).unwrap_err().code, "folder-not-absolute");
    assert!(check_folders(&stand.folders).is_ok());
}

/// A list changed after its copy, before the switch (an exchange, Sioul's
/// own writer): copied again at the switch, its paths named again.
#[test]
fn a_list_changed_before_the_switch_is_copied_again() {
    let stand = stand("changed");
    let (mut journal, _) = prepare(&stand.folders, NOW).unwrap();
    let budgets = stand.folders.notes.join(crate::budget::LEDGER);
    std::fs::write(&budgets, format!("{}\n[[budget]]\nid = \"car\"\ntitle = \"Car\"\n", text(&budgets))).unwrap();
    let papers = stand.folders.notes.join(crate::papers::MANIFEST);
    std::fs::write(&papers, format!("{}\n[[paper]]\nid = \"lease\"\nkind = \"other\"\ntitle = \"Lease\"\nfile = \"papers/Power.pdf\"\n", text(&papers))).unwrap();
    switch(&stand.folders, &mut journal, NOW).unwrap();
    assert_eq!(std::fs::read(stand.folders.records.join(crate::budget::LEDGER)).unwrap(), std::fs::read(&budgets).unwrap());
    let wallet = crate::papers::Wallet::load_at(&places_of(&stand)).unwrap();
    assert_eq!(wallet.get("lease").unwrap().file, "Power.pdf");
    assert!(check(&stand.folders, &journal).is_ok());
}

/// A list gone during the move: nothing switched, the old places kept.
#[test]
fn a_list_gone_during_the_move_switches_nothing() {
    let stand = stand("gone");
    let (mut journal, _) = prepare(&stand.folders, NOW).unwrap();
    let bank = stand.folders.notes.join(crate::bank::MANIFEST);
    let kept = std::fs::read(&bank).unwrap();
    std::fs::remove_file(&bank).unwrap();
    assert!(switch(&stand.folders, &mut journal, NOW).is_err());
    assert!(!Storage::read(&stand.folders.state).is_moved());
    std::fs::write(&bank, kept).unwrap();
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    reads_moved(&stand);
}

/// A list of no bytes at the move: copied as it is, never taken for one to
/// fill or for one emptied (the sharing writes it again from its records:
/// `share::lists_tests`); a list absent: none at the new place either.
#[test]
fn a_list_of_no_bytes_or_absent_moves_as_it_is() {
    let stand = stand("blank");
    std::fs::write(stand.folders.notes.join(crate::contracts::MANIFEST), "").unwrap();
    std::fs::remove_file(stand.folders.notes.join(crate::clients::FILE)).unwrap();
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    assert_eq!(std::fs::read(stand.folders.records.join(crate::contracts::MANIFEST)).unwrap(), b"");
    assert!(!stand.folders.records.join(crate::clients::FILE).exists());
}

/// Once every device moved: the old files out, but one written there since
/// the move, kept and said; the folders left empty go.
#[test]
fn clean_takes_out_only_what_still_holds_the_bytes_copied() {
    let stand = stand("clean");
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    let late = stand.folders.notes.join(crate::budget::LEDGER);
    std::fs::write(&late, format!("{}\n[[budget]]\nid = \"late\"\n", text(&late))).unwrap();
    let cleaned = clean(&stand.folders.state, NOW + 86_400).unwrap();
    assert_eq!(cleaned.changed, [crate::budget::LEDGER.to_string()]);
    assert!(late.is_file(), "a change made there after the move stays");
    assert_eq!(Storage::read(&stand.folders.state).cleaned, 0, "not said done while one stays");
    let left: Vec<String> = tree(&stand.folders.notes).into_keys().collect();
    assert_eq!(left, [".sioul-budgets.toml.lock", "admin/lease.md", "admin/taxes-2025.md", crate::budget::LEDGER], "the notes alone, and the file changed since with its lock");
    assert!(!stand.folders.notes.join("papers").exists() && !stand.folders.notes.join("letters").exists());
    // Taken out by hand, then cleaned again: said done.
    std::fs::remove_file(&late).unwrap();
    assert!(clean(&stand.folders.state, NOW + 2 * 86_400).unwrap().changed.is_empty());
    assert_eq!(Storage::read(&stand.folders.state).cleaned, NOW + 2 * 86_400);
    assert_eq!(tree(&stand.folders.notes).into_keys().collect::<Vec<_>>(), ["admin/lease.md", "admin/taxes-2025.md"], "its lock too, once the file is gone");
    // A moved device still finds everything.
    let places = places_of(&stand);
    let wallet = crate::papers::Wallet::load_at(&places).unwrap();
    assert!(wallet.papers.iter().all(|p| wallet.file_path(p).is_some_and(|file| file.is_file())));
    let letters = crate::letters::Letters::load_at(&places).unwrap();
    assert!(letters.list.iter().all(|l| letters.file_path(l).is_file()));
}

/// Each layout reads the other's paths: a device not moved yet and one moved
/// share their lists while they move one after the other.
#[test]
fn both_layouts_read_each_others_paths() {
    let base = std::env::temp_dir().join(format!("sioul-storage-layouts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (notes, papers) = (base.join("notes"), base.join("Papers"));
    std::fs::create_dir_all(notes.join("papers")).unwrap();
    std::fs::create_dir_all(&papers).unwrap();
    std::fs::write(notes.join("papers").join("A.pdf"), "a").unwrap();
    std::fs::write(papers.join("B.pdf"), "b").unwrap();
    std::fs::write(notes.join("Note.pdf"), "a note's file").unwrap();
    let old = Places::under(&notes);
    let new = Places::apart(&base.join("records"), Some(&notes), &papers, &base.join("Letters"));
    assert_eq!(old.papers.path_of("papers/A.pdf"), notes.join("papers").join("A.pdf"));
    assert_eq!(old.papers.path_of("A.pdf"), notes.join("papers").join("A.pdf"), "a moved device's name read before the move");
    assert_eq!(new.papers.path_of("B.pdf"), papers.join("B.pdf"));
    assert_eq!(new.papers.path_of("papers/B.pdf"), papers.join("B.pdf"), "a device not moved yet's name read after");
    assert_eq!(new.papers.path_of("Note.pdf"), notes.join("Note.pdf"), "a path written by hand from the notes folder still found");
    assert_eq!(new.papers.path_of("Gone.pdf"), papers.join("Gone.pdf"));
    assert_eq!(new.papers.named(&papers.join("x").join("C.pdf")), "x/C.pdf");
    assert_eq!(old.papers.named(&notes.join("papers").join("C.pdf")), "papers/C.pdf");
    let _ = std::fs::remove_dir_all(&base);
}

/// Dated copies go after thirty days, never before.
#[test]
fn dated_copies_go_after_thirty_days() {
    let base = std::env::temp_dir().join(format!("sioul-storage-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    for day in ["2026-10-01", "2026-10-20", "not-a-day"] {
        std::fs::create_dir_all(base.join(day)).unwrap();
        std::fs::write(base.join(day).join("sioul-bank.toml"), "x").unwrap();
    }
    // 18 November 2026.
    sweep_kept(&base, 1_795_046_400);
    assert!(!base.join("2026-10-01").exists() && base.join("2026-10-20").exists() && base.join("not-a-day").exists());
    let _ = std::fs::remove_dir_all(&base);
}

/// The state file: unset, never moved; there and unreadable, never moved
/// and never "not moved" either: lost, no list read anywhere.
#[test]
fn a_state_that_does_not_read_is_never_moved() {
    let base = std::env::temp_dir().join(format!("sioul-storage-state-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    assert!(!Storage::read(&base).is_moved() && Storage::read(&base).lost.is_none());
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join(STATE_FILE), "moved = \"soon\"").unwrap();
    assert!(!Storage::read(&base).is_moved());
    assert_eq!(Storage::read(&base).lost, Some("state-unreadable"));
    let config = crate::config::Config { notes_root: Some(base.display().to_string()), storage: Storage::read(&base), ..crate::config::Config::default() };
    assert!(config.places().is_none(), "no list read nor written while it is lost");
    // A journal that does not read, the state file gone: lost too, never "not moved".
    std::fs::remove_file(base.join(STATE_FILE)).unwrap();
    std::fs::write(base.join(JOURNAL), "switched = \"yes\"").unwrap();
    assert_eq!(Storage::read(&base).lost, Some("journal-unreadable"));
    std::fs::remove_file(base.join(JOURNAL)).unwrap();
    Storage { moved: NOW, from: "/notes".into(), ..Storage::default() }.write(&base).unwrap();
    assert!(Storage::read(&base).is_moved());
    let _ = std::fs::remove_dir_all(&base);
}

/// The papers' folder chosen again once moved: every file copied and
/// checked, the setting switched, a path written whole into the old folder
/// named from the new one; the old folder as it was. A file already there
/// that differs: refused, the copies made taken back, nothing switched.
#[test]
fn a_folder_chosen_again_moves_its_files_with_the_same_care() {
    let base = std::env::temp_dir().join(format!("sioul-storage-again-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (from, to, records) = (base.join("Papers"), base.join("Elsewhere").join("Papers"), base.join("records"));
    std::fs::create_dir_all(from.join("ids")).unwrap();
    std::fs::create_dir_all(&records).unwrap();
    std::fs::write(from.join("Passport.pdf"), "passport").unwrap();
    std::fs::write(from.join("ids").join("Card.png"), "card").unwrap();
    let list = records.join(crate::papers::MANIFEST);
    std::fs::write(&list, format!("# kept\n[[paper]]\nid = \"passport\"\nkind = \"passport\"\ntitle = \"Passport\"\nfile = \"Passport.pdf\"\n\n[[paper]]\nid = \"card\"\nkind = \"identity\"\ntitle = \"Card\"\nfile = \"{}\"\n", from.join("ids").join("Card.png").display().to_string().replace('\\', "/"))).unwrap();
    let before = tree(&from);
    // A file of the person's in the way: nothing switched, nothing left.
    std::fs::create_dir_all(&to).unwrap();
    std::fs::write(to.join("Passport.pdf"), "another").unwrap();
    let mut switched = false;
    let fault = move_documents(&from, &to, &list, "paper", || {
        switched = true;
        Ok(())
    })
    .unwrap_err();
    assert_eq!(fault.code, "records-taken");
    assert!(!switched && !to.join("ids").join("Card.png").exists() && text(&to.join("Passport.pdf")) == "another");
    // Out of the way: it goes.
    std::fs::remove_file(to.join("Passport.pdf")).unwrap();
    let moved = move_documents(&from, &to, &list, "paper", || {
        switched = true;
        Ok(())
    })
    .unwrap();
    assert!(switched && moved.named == 1, "{moved:?}");
    assert_eq!(tree(&from), before, "the old folder as it was");
    assert_eq!(tree(&to), before);
    let places = Places::apart(&records, None, &to, &base.join("Letters"));
    let wallet = crate::papers::Wallet::load_at(&places).unwrap();
    assert_eq!(wallet.get("card").unwrap().file, "ids/Card.png");
    assert!(wallet.papers.iter().all(|p| wallet.file_path(p).is_some_and(|f| f.starts_with(&to) && f.is_file())));
    assert!(text(&list).starts_with("# kept"));
    let _ = std::fs::remove_dir_all(&base);
}

/// A move stopped after its copies, the old paper then removed from the
/// notes folder (by hand, by a sync), and the move run again: the copy in
/// the papers' folder and the dated copy stay (review S24, first sequence).
#[test]
fn a_source_gone_between_two_runs_never_takes_its_copies_with_it() {
    // Find the point "kept": every copy made, checked, and its dated copy made.
    let mut k = 1;
    let stand = loop {
        let stand = stand(&format!("gone-after-copy-{k}"));
        STOP_AT.with(|stop| stop.set(k));
        let stopped = prepare(&stand.folders, NOW);
        STOP_AT.with(|stop| stop.set(0));
        match stopped {
            Err(fault) if fault.code == "stopped" && fault.what == "kept" => break stand,
            Err(fault) if fault.code == "stopped" => k += 1,
            other => panic!("no point named kept: {other:?}"),
        }
    };
    let copy = stand.folders.papers.join("Passport.pdf");
    let bytes = std::fs::read(&copy).expect("the copy was made before the stop");
    let journal = Journal::read(&stand.folders.state).unwrap();
    let dated = PathBuf::from(&journal.files.iter().find(|e| e.from == "papers/Passport.pdf").unwrap().kept);
    assert!(dated.is_file(), "the dated copy was made before the stop");
    // The old file leaves the notes folder between the two runs.
    std::fs::remove_file(stand.folders.notes.join("papers").join("Passport.pdf")).unwrap();
    let moved = move_now(&stand.folders, NOW + 60, || Ok(()));
    assert_eq!(std::fs::read(&copy).ok(), Some(bytes.clone()), "the copy is the only one left: it stays");
    assert_eq!(std::fs::read(&dated).ok(), Some(bytes), "and so does the dated copy");
    assert_eq!(moved.unwrap().alone, [copy.display().to_string()], "said");
    // The device reads it at its new place.
    let wallet = crate::papers::Wallet::load_at(&places_of(&stand)).unwrap();
    assert!(wallet.file_path(wallet.get("passport").unwrap()).is_some_and(|f| f == copy));
    // Its old file, gone, is never looked for at the take-out.
    assert!(clean(&stand.folders.state, NOW + 86_400).unwrap().changed.is_empty());
    assert!(copy.is_file());
}

/// Moved and cleaned, then the state file lost (the journal still there): a
/// second "Move now" deletes nothing; the move is adopted as made from the
/// closed journal (review S24, second sequence).
#[test]
fn a_lost_state_file_after_the_clean_up_never_undoes_the_move() {
    let stand = stand("state-lost");
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    assert!(clean(&stand.folders.state, NOW + 86_400).unwrap().changed.is_empty());
    let (papers, letters, records, kept) = (tree(&stand.folders.papers), tree(&stand.folders.letters), tree(&stand.folders.records), tree(&stand.folders.kept));
    assert!(records.contains_key(crate::bank::MANIFEST));
    std::fs::remove_file(stand.folders.state.join(STATE_FILE)).unwrap();
    // Read from the closed journal, before anything writes it again.
    let adopted = Storage::read(&stand.folders.state);
    assert!(adopted.is_moved() && adopted.adopted && adopted.cleaned == NOW + 86_400, "{adopted:?}");
    assert_eq!(move_now(&stand.folders, NOW + 2 * 86_400, || Ok(())).unwrap_err().code, "moved-already");
    assert_eq!(tree(&stand.folders.papers), papers, "the papers stay");
    assert_eq!(tree(&stand.folders.letters), letters, "the scans stay");
    assert_eq!(tree(&stand.folders.records), records, "the lists stay, the bank's lines first");
    assert_eq!(tree(&stand.folders.kept), kept, "the dated copies stay");
    assert!(stand.folders.state.join(STATE_FILE).is_file(), "the state file written again");
    // A closed journal is never work to do again, whatever calls it.
    assert_eq!(prepare(&stand.folders, NOW + 3 * 86_400).unwrap_err().code, "moved-already");
    assert_eq!(tree(&stand.folders.papers), papers);
}

/// The state file lost and the copies not all there (a papers' folder on a
/// drive not plugged in): never "not moved", never moved; no move starts,
/// nothing is deleted, no list is read anywhere (rule 3).
#[test]
fn a_lost_state_file_with_copies_missing_stops_every_list() {
    let stand = stand("state-lost-copies");
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    std::fs::remove_file(stand.folders.state.join(STATE_FILE)).unwrap();
    let aside = stand.base.join("unplugged");
    std::fs::rename(&stand.folders.papers, &aside).unwrap();
    let before = (tree(&stand.folders.notes), tree(&stand.folders.records), tree(&stand.folders.letters));
    let storage = Storage::read(&stand.folders.state);
    assert!(!storage.is_moved() && storage.lost == Some("state-missing"), "{storage:?}");
    assert_eq!(move_now(&stand.folders, NOW + 60, || Ok(())).unwrap_err().code, "state-missing");
    assert_eq!((tree(&stand.folders.notes), tree(&stand.folders.records), tree(&stand.folders.letters)), before);
    // The drive back: the move adopted as made.
    std::fs::rename(&aside, &stand.folders.papers).unwrap();
    assert!(Storage::recover(&stand.folders.state).unwrap());
    assert!(Storage::read(&stand.folders.state).is_moved());
    reads_moved(&stand);
}

/// A failure takes back what its own run made, and only that: the copies of
/// an earlier run, killed, stay for the next try (review S24).
#[test]
fn a_failure_takes_back_only_what_its_own_run_made() {
    let mut k = 1;
    let stand = loop {
        let stand = stand(&format!("own-run-{k}"));
        STOP_AT.with(|stop| stop.set(k));
        let stopped = prepare(&stand.folders, NOW);
        STOP_AT.with(|stop| stop.set(0));
        match stopped {
            Err(fault) if fault.code == "stopped" && fault.what == "copied" => break stand,
            Err(fault) if fault.code == "stopped" => k += 1,
            other => panic!("no point named copied: {other:?}"),
        }
    };
    let (papers, records) = (tree(&stand.folders.papers), tree(&stand.folders.records));
    assert!(!papers.is_empty() && !records.is_empty());
    // A second run that fails at the switch (an exchange running): what the first made stays.
    assert_eq!(move_now(&stand.folders, NOW + 60, || Err::<(), _>("busy".to_string())).unwrap_err().code, "busy");
    assert_eq!(tree(&stand.folders.papers), papers);
    // The same files (the papers' list's copy has its paths named since: the same file still).
    assert_eq!(tree(&stand.folders.records).into_keys().collect::<Vec<_>>(), records.into_keys().collect::<Vec<_>>());
    assert!(Journal::read(&stand.folders.state).is_some_and(|j| j.files.len() == 15), "still named, for the next try");
    // Then it goes, nothing copied twice.
    move_now(&stand.folders, NOW + 120, || Ok(())).unwrap();
    reads_moved(&stand);
}

/// A notes folder set and not there now (a drive not plugged in) is no
/// "nothing to move": the move refuses and switches nothing; one that holds
/// nothing at all, as an unmounted drive's mount point, the same; a papers'
/// folder there that does not list is never taken for an empty one; none
/// set: nothing to move, nothing switched to empty (review S1).
#[test]
fn a_notes_folder_that_is_not_there_now_switches_nothing() {
    let stand = stand("no-notes-now");
    let gone = Folders { notes: stand.base.join("unplugged").join("Notes"), ..stand.folders.clone() };
    assert_eq!(move_now(&gone, NOW, || Ok(())).unwrap_err().code, "notes-unreadable");
    assert!(!Storage::read(&gone.state).is_moved());
    assert!(tree(&gone.records).is_empty() && tree(&gone.papers).is_empty());
    let mount = stand.base.join("mount-point");
    std::fs::create_dir_all(&mount).unwrap();
    let empty = Folders { notes: mount, ..stand.folders.clone() };
    assert_eq!(move_now(&empty, NOW, || Ok(())).unwrap_err().code, "notes-empty");
    assert!(!Storage::read(&empty.state).is_moved());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let papers = stand.folders.notes.join("papers");
        std::fs::set_permissions(&papers, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Read all the same by a user above permissions (root in a container): nothing to show then.
        if std::fs::read_dir(&papers).is_err() {
            assert_eq!(plan(&stand.folders).unwrap_err().code, "unreadable");
            assert_eq!(move_now(&stand.folders, NOW, || Ok(())).unwrap_err().code, "unreadable");
            assert!(!Storage::read(&stand.folders.state).is_moved());
        }
        std::fs::set_permissions(&papers, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // None set on this device: nothing to move, the lists begin in the records.
    let none = Folders { notes: PathBuf::new(), ..stand.folders.clone() };
    let moved = move_now(&none, NOW, || Ok(())).unwrap();
    assert_eq!((moved.lists, moved.papers, moved.letters), (0, 0, 0));
    assert!(Storage::read(&none.state).is_moved() && Storage::read(&none.state).from_path().is_none());
    assert!(tree(&none.records).is_empty(), "nothing written there");
}

/// After the move, `papers/` in the notes folder replaced by a link to the
/// new papers' folder (or a file of `letters/` by a link): the take-out
/// removes nothing through it, and says why (review S14).
#[cfg(unix)]
#[test]
fn clean_never_deletes_through_a_link_put_in_the_notes_folder() {
    let stand = stand("clean-link");
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    let old = stand.folders.notes.join("papers");
    std::fs::remove_dir_all(&old).unwrap();
    std::os::unix::fs::symlink(&stand.folders.papers, &old).unwrap();
    // A scan's old file replaced by a link to its copy, too.
    let scan = stand.folders.notes.join("letters").join("2026").join("2026-10-12 caf other.pdf");
    std::fs::remove_file(&scan).unwrap();
    std::os::unix::fs::symlink(stand.folders.letters.join("2026").join("2026-10-12 caf other.pdf"), &scan).unwrap();
    let (papers, letters) = (tree(&stand.folders.papers), tree(&stand.folders.letters));
    assert!(!papers.is_empty());
    let cleaned = clean(&stand.folders.state, NOW + 86_400).unwrap();
    assert_eq!(tree(&stand.folders.papers), papers, "the papers' folder as it was");
    assert_eq!(tree(&stand.folders.letters), letters, "the letters' folder as it was");
    assert!(cleaned.kept.iter().any(|(file, why)| file == "papers/Passport.pdf" && *why == "link"), "{cleaned:?}");
    assert!(cleaned.kept.iter().any(|(file, why)| file.ends_with("caf other.pdf") && *why == "link"), "{cleaned:?}");
    assert!(std::fs::symlink_metadata(&old).is_ok_and(|m| m.file_type().is_symlink()), "the link itself left alone");
    assert_eq!(Storage::read(&stand.folders.state).cleaned, 0, "not said done while something stays");
}

/// The papers' folder chosen is the notes folder's own `papers/` under
/// another spelling (the notes folder reached through a link): refused,
/// as the same folder spelled alike is (review S23).
#[cfg(unix)]
#[test]
fn the_same_folder_under_another_spelling_is_refused() {
    let stand = stand("same-folder");
    let link = stand.base.join("NotesLink");
    std::os::unix::fs::symlink(&stand.folders.notes, &link).unwrap();
    let folders = Folders { papers: link.join("papers"), ..stand.folders.clone() };
    let before = tree(&stand.folders.notes);
    assert_eq!(move_now(&folders, NOW, || Ok(())).unwrap_err().code, "folder-in-notes");
    assert!(!Storage::read(&folders.state).is_moved());
    assert_eq!(tree(&stand.folders.notes), before);
    // A folder around Sioul's own data, through a link: refused too.
    let data = stand.base.join("DataLink");
    std::os::unix::fs::symlink(stand.folders.records.parent().unwrap().parent().unwrap(), &data).unwrap();
    std::fs::create_dir_all(&stand.folders.records).unwrap();
    let around = Folders { letters: data, ..stand.folders.clone() };
    assert!(check_folders(&around).is_err());
}

/// A file found at its place that is the paper itself (a hard link, as a
/// folder that folds case or a link above it would make it): never copied
/// over itself, never taken for "found there already"; and a file of the
/// same bytes the journal does not name is never adopted (review S23).
#[cfg(unix)]
#[test]
fn a_file_at_its_place_is_the_moves_only_when_the_journal_says_so() {
    let stand = stand("same-file");
    std::fs::create_dir_all(&stand.folders.records).unwrap();
    std::fs::hard_link(stand.folders.notes.join(crate::bank::MANIFEST), stand.folders.records.join(crate::bank::MANIFEST)).unwrap();
    assert_eq!(move_now(&stand.folders, NOW, || Ok(())).unwrap_err().code, "folder-in-notes");
    assert!(!Storage::read(&stand.folders.state).is_moved());
    assert!(stand.folders.notes.join(crate::bank::MANIFEST).is_file());
    std::fs::remove_file(stand.folders.records.join(crate::bank::MANIFEST)).unwrap();
    // The same bytes, another file the journal does not name: never the move's own.
    std::fs::create_dir_all(&stand.folders.papers).unwrap();
    std::fs::copy(stand.folders.notes.join("papers").join("Passport.pdf"), stand.folders.papers.join("Passport.pdf")).unwrap();
    let _ = move_now(&stand.folders, NOW, || Ok(()));
    let adopted = Journal::read(&stand.folders.state).is_some_and(|j| j.files.iter().any(|e| Path::new(&e.to) == stand.folders.papers.join("Passport.pdf") && !e.made));
    assert!(!adopted, "never taken for the move's own");
    // Moved on a folder of its own, then the copy made the same file as the old one: the take-out leaves it.
    let _ = std::fs::remove_dir_all(&stand.folders.papers);
    let _ = std::fs::remove_dir_all(&stand.folders.records);
    let _ = std::fs::remove_dir_all(&stand.folders.letters);
    let _ = std::fs::remove_dir_all(&stand.folders.state);
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    let copy = stand.folders.papers.join("Power.pdf");
    std::fs::remove_file(&copy).unwrap();
    std::fs::hard_link(stand.folders.notes.join("papers").join("Power.pdf"), &copy).unwrap();
    let cleaned = clean(&stand.folders.state, NOW + 86_400).unwrap();
    assert!(cleaned.kept.contains(&("papers/Power.pdf".to_string(), "same-file")), "{cleaned:?}");
    assert!(stand.folders.notes.join("papers").join("Power.pdf").is_file());
}

/// A copy gone from the papers' folder before the take-out (a disk not
/// plugged in, a mistake), or a list's dated copy gone: the old file stays,
/// said (rule 1, review S15).
#[test]
fn clean_keeps_an_old_file_whose_copy_is_gone() {
    let stand = stand("copy-gone");
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    std::fs::remove_file(stand.folders.papers.join("Passport.pdf")).unwrap();
    std::fs::write(stand.folders.papers.join("Power.pdf"), "another file now").unwrap();
    let journal = Journal::read(&stand.folders.state).unwrap();
    std::fs::remove_file(&journal.files.iter().find(|e| e.from == crate::bank::MANIFEST).unwrap().kept).unwrap();
    let cleaned = clean(&stand.folders.state, NOW + 86_400).unwrap();
    assert!(stand.folders.notes.join("papers").join("Passport.pdf").is_file(), "the last copy in view stays");
    assert!(stand.folders.notes.join("papers").join("Power.pdf").is_file(), "its copy is another file now");
    assert!(stand.folders.notes.join(crate::bank::MANIFEST).is_file(), "the bank's lines, their dated copy gone");
    assert!(cleaned.kept.contains(&("papers/Passport.pdf".to_string(), "copy-missing")), "{cleaned:?}");
    assert!(cleaned.kept.contains(&("papers/Power.pdf".to_string(), "copy-differs")), "{cleaned:?}");
    assert!(cleaned.kept.contains(&(crate::bank::MANIFEST.to_string(), "copy-missing")), "{cleaned:?}");
    assert!(!stand.folders.notes.join(crate::budget::LEDGER).exists(), "the others went, each against its copy");
    assert_eq!(Storage::read(&stand.folders.state).cleaned, 0);
}

/// An old file written after the switch (by hand, a sync from a device not
/// moved): named with its bytes, never merged, never taken out; a move
/// begun and left unfinished is said too (review S11, and the window's way
/// back from the Porch).
#[test]
fn a_change_in_an_old_file_after_the_switch_is_told_and_kept() {
    let stand = stand("changed-after");
    STOP_AT.with(|stop| stop.set(3));
    let _ = prepare(&stand.folders, NOW);
    STOP_AT.with(|stop| stop.set(0));
    assert!(begun(&stand.folders.state), "a move begun, not finished");
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    assert!(!begun(&stand.folders.state));
    assert!(changed_since(&stand.folders.state).is_empty(), "nothing changed yet");
    let budgets = stand.folders.notes.join(crate::budget::LEDGER);
    std::fs::write(&budgets, format!("{}\n[[budget]]\nid = \"late\"\n", text(&budgets))).unwrap();
    let changed = changed_since(&stand.folders.state);
    assert_eq!(changed.iter().map(|(file, _)| file.as_str()).collect::<Vec<_>>(), [crate::budget::LEDGER]);
    assert_eq!(changed[0].1, sha_of(&budgets).unwrap());
    assert!(!text(&stand.folders.records.join(crate::budget::LEDGER)).contains("late"), "merged with nothing");
    let cleaned = clean(&stand.folders.state, NOW + 86_400).unwrap();
    assert_eq!(cleaned.changed, [crate::budget::LEDGER.to_string()]);
    assert!(budgets.is_file());
}

/// The dated copy is bytes of its own: an editor that writes an old list in
/// place (vim with a file of several names, rsync --inplace, some syncs)
/// never changes it (review S12).
#[test]
fn the_dated_copy_is_a_copy_of_its_own() {
    let stand = stand("dated-own");
    move_now(&stand.folders, NOW, || Ok(())).unwrap();
    let bank = stand.folders.notes.join(crate::bank::MANIFEST);
    let dated = PathBuf::from(&Journal::read(&stand.folders.state).unwrap().files.iter().find(|e| e.from == crate::bank::MANIFEST).unwrap().kept);
    let before = std::fs::read(&dated).unwrap();
    assert!(!same_file(&bank, &dated), "never a second name of the old file");
    // Written in place, the same file truncated and filled again.
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).truncate(true).open(&bank).unwrap();
    file.write_all(b"# written over in place\n").unwrap();
    drop(file);
    assert_eq!(std::fs::read(&dated).unwrap(), before, "the dated copy as it was");
}
