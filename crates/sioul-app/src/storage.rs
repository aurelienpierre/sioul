// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What lives where (docs/notes-folder.md, "What lives where"): this
//! device's move of Sioul's lists out of the notes folder, once every device
//! that counts knows the new places, on your word (`move_now`); the old
//! files taken out once every device moved, and the dated copies after their
//! thirty days (`tidy`). The move itself is `sioul_core::storage`'s; the
//! gate, the sharing's (`sioul_sync::share::storage_stand`).

use crate::backend::{QtThread, Shared, load_config, say, tr};
use cxx_qt_lib::QString;
use serde::Serialize;
use sioul_core::storage::{self, Folders, Storage};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// A move running on this device (`begin`, `move_now`).
static MOVING: AtomicBool = AtomicBool::new(false);
/// What the last move said, when it did not happen: its one sentence.
static SAID: Mutex<String> = Mutex::new(String::new());

/// A move begins: the window says so at once.
pub(crate) fn begin() {
    MOVING.store(true, Ordering::SeqCst);
    if let Ok(mut said) = SAID.lock() {
        said.clear();
    }
}

/// The move, as the window asks it, with the papers' and letters' folders
/// chosen there ("" for the default) and whether this device's part
/// "projects" goes on, so that its lists reach your other devices. The
/// chosen folders are written first (this device's own settings, read only
/// once moved); then the copies, checks, the dated copy and the paths
/// named, nothing held; then the switch with every exchange held. Returns
/// what went wrong in one sentence, else "".
pub(crate) fn move_now(papers: &str, letters: &str, carry: bool) -> String {
    let problem = moved_now(papers, letters, carry);
    if let Ok(mut said) = SAID.lock() {
        said.clone_from(&problem);
    }
    MOVING.store(false, Ordering::SeqCst);
    problem
}

fn moved_now(papers: &str, letters: &str, carry: bool) -> String {
    let config = load_config();
    if config.storage.is_moved() {
        return String::new();
    }
    if let Some(stand) = crate::share::storage_stand()
        && !stand.open
    {
        return say("storage-waiting", &[("devices", names(&stand.waiting))]);
    }
    let pick = |chosen: &str, default: PathBuf| if chosen.trim().is_empty() { default } else { crate::backend::local_path(chosen.trim()) };
    let (papers, letters) = (pick(papers, config.papers_folder()), pick(letters, config.letters_folder()));
    let notes = config.notes_root_path().filter(|notes| notes.is_dir()).unwrap_or_default();
    let mut folders = Folders::here(&notes, &papers, &letters);
    folders.notes = notes;
    if let Err(fault) = storage::check_folders(&folders) {
        return tr().text(&format!("storage-{}", fault.code), None);
    }
    let path = crate::backend::config_path();
    for (key, folder) in [("papers.folder", &papers), ("letters.folder", &letters)] {
        if let Err(e) = sioul_core::config::set_value(&path, key, &sioul_core::config::SettingValue::Text(folder.display().to_string())) {
            return e;
        }
    }
    let now = jiff::Timestamp::now().as_second();
    if let Err(fault) = storage::move_now(&folders, now, crate::share::hold_exchanges) {
        return tr().text(&format!("storage-{}", fault.code), None);
    }
    if carry && !crate::share::carries_lists() {
        let problem = crate::share::carry_lists();
        if !problem.is_empty() {
            return problem;
        }
    }
    crate::devices::storage_now();
    for kind in [sioul_core::changes::Kind::Papers, sioul_core::changes::Kind::Letters] {
        sioul_core::changes::said(kind);
    }
    String::new()
}

/// The devices named, as a list in words.
fn names(devices: &[sioul_sync::share::Named]) -> String {
    devices.iter().map(|d| if d.name.trim().is_empty() { d.kind.clone() } else { d.name.clone() }).collect::<Vec<_>>().join(", ")
}

/// After each exchange: the dated copies past their days taken out; once
/// every device that counts moved, the old files of Sioul's lists out of
/// this device's notes folder, by the device every device agrees on
/// (`StorageStand::cleaner`) where a sync app carries that folder (it
/// carries the removal to the others), else by each device for its own.
/// Without sharing, only a notes folder no sync app carries: another device
/// of yours reading it would not be known here.
pub(crate) fn tidy() {
    let now = jiff::Timestamp::now().as_second();
    storage::sweep_kept(&sioul_core::config::data_dir().join(storage::KEPT), now);
    let here = Storage::here();
    if !here.is_moved() || here.cleaned > 0 || here.from.is_empty() {
        return;
    }
    let carried = crate::share::notes_carried_now();
    let due = match crate::share::storage_stand() {
        Some(stand) if !stand.alone => stand.all_moved && (stand.cleaner || !carried),
        _ => !carried,
    };
    if due {
        match storage::clean(&sioul_core::config::state_dir(), now) {
            Ok(cleaned) if !cleaned.changed.is_empty() => eprintln!("sioul: storage: kept in the notes folder, changed since the move: {}", cleaned.changed.join(", ")),
            Ok(_) => {}
            Err(e) => eprintln!("sioul: storage: {e}"),
        }
    }
}

/// Where Sioul's lists are and the move stands, for the window (`storageMove`).
#[derive(Debug, Default, Serialize)]
struct View {
    /// This device's lists left the notes folder; and its old files were taken out.
    moved: bool,
    cleaned: bool,
    /// A move runs now.
    moving: bool,
    /// Why the last move did not happen, in one sentence.
    said: String,
    /// Every other device that counts knows the new places (or none counts): the move may be made.
    open: bool,
    /// No other device counts; sharing set up here at all.
    alone: bool,
    sharing: bool,
    /// The devices waited for (an older Sioul), and those that have not moved, by name.
    waiting: String,
    unmoved: String,
    /// This device's part "projects" is on: its lists travel by the sharing after the move.
    carries: bool,
    /// A sync app carries the notes folder.
    notes_carried: bool,
    /// The notes folder, the papers' and the letters' folders (as set, or by default).
    notes: String,
    papers: String,
    letters: String,
    /// What would move: lists (with the letters' list and texts), papers' files, letters' scans, bytes.
    lists: usize,
    papers_files: usize,
    letters_files: usize,
    bytes: u64,
    /// The window of the move may show itself (not moved, every device knows).
    offer: bool,
}

/// `View`, as JSON. Reads the sharing folder's entries and lists the notes
/// folder: never on the window's thread (`backend::storage_move_now`).
pub(crate) fn view_json() -> String {
    let config = load_config();
    let here = Storage::here();
    let stand = crate::share::storage_stand();
    let notes = config.notes_root_path().filter(|notes| notes.is_dir());
    let (papers, letters) = (config.papers_folder(), config.letters_folder());
    let mut view = View {
        moved: here.is_moved(),
        cleaned: here.cleaned > 0,
        moving: MOVING.load(Ordering::SeqCst),
        said: SAID.lock().map(|s| s.clone()).unwrap_or_default(),
        open: stand.as_ref().is_none_or(|s| s.open),
        alone: stand.as_ref().is_none_or(|s| s.alone),
        sharing: stand.is_some(),
        waiting: stand.as_ref().map(|s| names(&s.waiting)).unwrap_or_default(),
        unmoved: stand.as_ref().map(|s| names(&s.unmoved)).unwrap_or_default(),
        carries: crate::share::carries_lists(),
        notes_carried: crate::share::notes_carried_now(),
        notes: notes.as_ref().map(|n| n.display().to_string()).unwrap_or_default(),
        papers: papers.display().to_string(),
        letters: letters.display().to_string(),
        ..View::default()
    };
    if !view.moved
        && let Some(notes) = &notes
        && let Ok(plan) = storage::plan(&Folders::here(notes, &papers, &letters))
    {
        for entry in &plan {
            match entry.kind {
                Some(storage::Kind::Paper) => view.papers_files += 1,
                Some(storage::Kind::Letter) => view.letters_files += 1,
                _ => view.lists += 1,
            }
            view.bytes += std::fs::metadata(notes.join(&entry.from)).map_or(0, |m| m.len());
        }
    }
    view.offer = !view.moved && view.open;
    serde_json::to_string(&view).unwrap_or_default()
}

/// The papers' or letters' folder chosen in its page's ⚙ (`papers.folder`,
/// `letters.folder`). Before the move, only kept: it is read once the lists
/// move. Once moved, its files follow with the same care as the move
/// (`storage::move_documents`: copied, checked, the setting switched with
/// every exchange held, the old folder left as it was), off the window's
/// thread, said in the status line.
pub(crate) fn change_folder(qt: &QtThread, shared: &Arc<Shared>, key: &str, folder: &str) -> Result<(), String> {
    let config = load_config();
    let chosen = crate::backend::local_path(folder.trim());
    let path = crate::backend::config_path();
    let text = |p: &std::path::Path| sioul_core::config::SettingValue::Text(p.display().to_string());
    if !config.storage.is_moved() || folder.trim().is_empty() {
        return sioul_core::config::set_value(&path, key, &text(&chosen));
    }
    let Some(records) = config.records_root() else { return sioul_core::config::set_value(&path, key, &text(&chosen)) };
    let (from, list, table) = if key == "papers.folder" {
        (config.papers_folder(), records.join(sioul_core::papers::MANIFEST), "paper")
    } else {
        (config.letters_folder(), sioul_core::letters::Letters::folder(&records).join(sioul_core::letters::MANIFEST), "letter")
    };
    if from == chosen {
        return Ok(());
    }
    let (qt, shared, key) = (qt.clone(), shared.clone(), key.to_string());
    std::thread::spawn(move || {
        let moved = storage::move_documents(&from, &chosen, &list, table, || {
            let _held = crate::share::hold_exchanges()?;
            sioul_core::config::set_value(&path, &key, &text(&chosen))
        });
        let line = match moved {
            Ok(_) => say("storage-folder-moved", &[("folder", chosen.display().to_string())]),
            Err(fault) => tr().text(&format!("storage-{}", fault.code), None),
        };
        let qt_again = qt.clone();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().set_status(QString::from(&line)));
        crate::backend::show(&qt_again, &shared);
        crate::share::exchange(&qt_again, &shared);
    });
    Ok(())
}
