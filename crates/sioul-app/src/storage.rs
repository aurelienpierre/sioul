// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What lives where (docs/notes-folder.md, "What lives where"): this
//! device's move of Sioul's lists out of the notes folder, once every device
//! that counts knows the new places, on your word (`move_now`); the old
//! files taken out once every device moved, and the dated copies after their
//! thirty days (`tidy`). The move itself is `sioul_core::storage`'s; the
//! gate, the sharing's (`sioul_sync::share::storage_stand`).

use crate::backend::{load_config, say, tr};
use sioul_core::storage::{self, Folders, Storage};
use std::path::PathBuf;

/// The move, as the window asks it, with the papers' and letters' folders
/// chosen there ("" for the default) and whether this device's part
/// "projects" goes on, so that its lists reach your other devices. The
/// chosen folders are written first (this device's own settings, read only
/// once moved); then the copies, checks, the dated copy and the paths
/// named, nothing held; then the switch with every exchange held. Returns
/// what went wrong in one sentence, else "".
pub(crate) fn move_now(papers: &str, letters: &str, carry: bool) -> String {
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
