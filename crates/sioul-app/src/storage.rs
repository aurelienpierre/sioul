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
use sioul_core::config::NotesFolder;
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
/// the sentence for the status line, or what went wrong in one sentence.
pub(crate) fn move_now(papers: &str, letters: &str, carry: bool) -> Result<String, String> {
    let done = moved_now(papers, letters, carry);
    if let Ok(mut said) = SAID.lock() {
        said.clone_from(done.as_ref().err().unwrap_or(&String::new()));
    }
    MOVING.store(false, Ordering::SeqCst);
    done
}

fn moved_now(papers: &str, letters: &str, carry: bool) -> Result<String, String> {
    let config = load_config();
    if config.storage.is_moved() {
        return Ok(tr().text("storage-moved-already", None));
    }
    move_gate(crate::share::storage_stand().as_ref())?;
    let pick = |chosen: &str, default: PathBuf| if chosen.trim().is_empty() { default } else { crate::backend::local_path(chosen.trim()) };
    let (papers, letters) = (pick(papers, config.papers_folder()), pick(letters, config.letters_folder()));
    // The notes folder in its three states (`Config::notes_folder`): set and not read now
    // is never "no notes folder" (rule 3); none set, nothing to move.
    let notes = match config.notes_folder() {
        NotesFolder::Ready(path) => sioul_core::config::expand_home(&path),
        NotesFolder::Unreadable(path) => return Err(say("storage-notes-unreadable", &[("what", path)])),
        NotesFolder::Unset => PathBuf::new(),
    };
    let folders = Folders::here(&notes, &papers, &letters);
    storage::check_folders(&folders).map_err(said)?;
    let path = crate::backend::config_path();
    for (key, folder) in [("papers.folder", &papers), ("letters.folder", &letters)] {
        sioul_core::config::set_value(&path, key, &sioul_core::config::SettingValue::Text(folder.display().to_string()))?;
    }
    let now = jiff::Timestamp::now().as_second();
    let moved = storage::move_now(&folders, now, crate::share::hold_exchanges).map_err(said)?;
    if carry && !crate::share::carries_lists() {
        let problem = crate::share::carry_lists();
        if !problem.is_empty() {
            return Err(problem);
        }
    }
    crate::devices::storage_now();
    for kind in [sioul_core::changes::Kind::Papers, sioul_core::changes::Kind::Letters] {
        sioul_core::changes::said(kind);
    }
    // What stays besides: copies whose source was gone at this try, copies of an earlier try elsewhere.
    let mut line = vec![tr().text("storage-moved", None)];
    if !moved.alone.is_empty() {
        line.push(say("storage-move-alone", &[("count", moved.alone.len().to_string())]));
    }
    if !moved.earlier.is_empty() {
        let folders: std::collections::BTreeSet<String> = moved.earlier.iter().filter_map(|to| std::path::Path::new(to).parent().map(|p| p.display().to_string())).collect();
        line.push(say("storage-move-earlier", &[("folders", folders.into_iter().collect::<Vec<_>>().join(", "))]));
    }
    Ok(line.join(" "))
}

/// A move's fault in one sentence, naming what it is about (`storage-<code>`, `$what`).
pub(crate) fn said(fault: storage::Fault) -> String {
    say(&format!("storage-{}", fault.code), &[("what", fault.what)])
}

/// The devices named, as a list in words.
fn names(devices: &[sioul_sync::share::Named]) -> String {
    devices.iter().map(|d| if d.name.trim().is_empty() { d.kind.clone() } else { d.name.clone() }).collect::<Vec<_>>().join(", ")
}

/// After each exchange: the dated copies past their days taken out. The old
/// files of Sioul's lists are never taken out of the notes folder by
/// themselves: on the person's press alone (`take_out`).
pub(crate) fn tidy() {
    let now = jiff::Timestamp::now().as_second();
    storage::sweep_kept(&sioul_core::config::data_dir().join(storage::KEPT), Storage::here().cleaned, now);
}

/// Whether this device may move now: every other device that counts knows
/// the new places (or none is known), and the sharing, set up, reads (a
/// sharing that cannot be read holds the move as a device missing would).
fn move_gate(stand: Option<&sioul_sync::share::StorageStand>) -> Result<(), String> {
    match stand {
        Some(stand) if stand.unreadable => Err(tr().text("storage-sharing-unreadable", None)),
        Some(stand) if !stand.open => Err(say("storage-waiting", &[("devices", names(&stand.waiting))])),
        _ => Ok(()),
    }
}

/// Whether the old files may be taken out now: every other device that
/// counts moved (or none is known), the sharing read. Else the sentence
/// that says who is still to move, or what cannot be read.
fn take_out_gate(stand: Option<&sioul_sync::share::StorageStand>) -> Result<(), String> {
    match stand {
        None => Ok(()),
        Some(stand) if stand.unreadable => Err(tr().text("storage-sharing-unreadable", None)),
        Some(stand) if stand.alone || stand.all_moved => Ok(()),
        Some(stand) => Err(say("storage-old-files-waiting", &[("devices", names(&stand.waiting.iter().chain(&stand.unmoved).cloned().collect::<Vec<_>>()))])),
    }
}

/// "Take them out" (Settings ▸ Sharing): the old files of Sioul's lists out
/// of this device's notes folder, each against its copy (`storage::clean`),
/// once every device that counts moved; what stays, said by why. Off the
/// window's thread.
pub(crate) fn take_out() -> String {
    let here = Storage::here();
    if !here.is_moved() {
        return String::new();
    }
    if let Err(line) = take_out_gate(crate::share::storage_stand().as_ref()) {
        return line;
    }
    let now = jiff::Timestamp::now().as_second();
    let line = match storage::clean(&sioul_core::config::state_dir(), now) {
        Ok(cleaned) => cleaned_line(&cleaned),
        Err(e) => e,
    };
    if let Ok(mut taken) = TAKEN.lock() {
        taken.clone_from(&line);
    }
    line
}

/// What a take-out did, in sentences: how many went, and what stays, by why.
fn cleaned_line(cleaned: &storage::Cleaned) -> String {
    let mut line = vec![say("storage-taken-out", &[("count", cleaned.removed.to_string())])];
    let mut by: std::collections::BTreeMap<&str, Vec<String>> = std::collections::BTreeMap::new();
    by.entry("changed").or_default().extend(cleaned.changed.iter().cloned());
    for (file, why) in &cleaned.kept {
        by.entry(*why).or_default().push(file.clone());
    }
    for (why, files) in by.into_iter().filter(|(_, files)| !files.is_empty()) {
        line.push(say(&format!("storage-kept-{why}"), &[("files", files.join(", "))]));
    }
    line.join(" ")
}

/// What the last take-out said.
static TAKEN: Mutex<String> = Mutex::new(String::new());

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
    /// Why the move waits, when it does: the devices on an older Sioul, or the sharing not read now.
    held: String,
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
    /// The notes folder's state (`Config::notes_folder`): "ready", "unreadable" (set, not read
    /// now: no move, the Notes page asks for it) or "unset" (nothing to move).
    notes_state: String,
    papers: String,
    letters: String,
    /// What would move: lists (with the letters' list and texts), papers' files, letters' scans, bytes.
    lists: usize,
    papers_files: usize,
    letters_files: usize,
    bytes: u64,
    /// Where this device's lists are is not known now (`Storage::lost`): its sentence.
    lost: String,
    /// Old files of Sioul's still in the notes folder after the move (`storage::old_files`);
    /// whether "Take them out" may be pressed now, else who is still to move; what the last press said.
    old_files: usize,
    take_out_open: bool,
    take_out_waiting: String,
    taken: String,
    /// A move begun here and not finished (a failure, Sioul stopped during it): the Porch offers the window again.
    begun: bool,
    /// Old files of the notes folder changed since the lists left them (`storage::changed_since`), named
    /// on the Porch once (`changed_key`: what was said), merged with nothing.
    changed_old: Vec<String>,
    changed_key: String,
    /// The window of the move may show itself (not moved, every device knows).
    offer: bool,
}

/// `View`, as JSON. Reads the sharing folder's entries and lists the notes
/// folder: never on the window's thread (`backend::storage_move_now`).
pub(crate) fn view_json() -> String {
    // The state file written again from a closed journal whose copies are all there (`Storage::recover`).
    if Storage::recover(&sioul_core::config::state_dir()).unwrap_or(false) {
        crate::devices::storage_now();
    }
    let config = load_config();
    let here = Storage::here();
    let stand = crate::share::storage_stand();
    let (notes, notes_state) = match config.notes_folder() {
        NotesFolder::Ready(path) => (Some(sioul_core::config::expand_home(&path)), "ready"),
        NotesFolder::Unreadable(path) => (Some(sioul_core::config::expand_home(&path)), "unreadable"),
        NotesFolder::Unset => (None, "unset"),
    };
    let (papers, letters) = (config.papers_folder(), config.letters_folder());
    let mut view = View {
        moved: here.is_moved(),
        cleaned: here.cleaned > 0,
        moving: MOVING.load(Ordering::SeqCst),
        said: SAID.lock().map(|s| s.clone()).unwrap_or_default(),
        open: move_gate(stand.as_ref()).is_ok(),
        held: move_gate(stand.as_ref()).err().unwrap_or_default(),
        alone: stand.as_ref().is_none_or(|s| s.alone && !s.unreadable),
        sharing: stand.is_some(),
        waiting: stand.as_ref().map(|s| names(&s.waiting)).unwrap_or_default(),
        unmoved: stand.as_ref().map(|s| names(&s.unmoved)).unwrap_or_default(),
        carries: crate::share::carries_lists(),
        notes_carried: crate::share::notes_carried_now(),
        notes: notes.as_ref().map(|n| n.display().to_string()).unwrap_or_default(),
        notes_state: notes_state.to_string(),
        papers: papers.display().to_string(),
        letters: letters.display().to_string(),
        lost: here.lost.map(|lost| say(&format!("storage-{lost}"), &[("what", sioul_core::config::state_dir().join(storage::STATE_FILE).display().to_string())])).unwrap_or_default(),
        ..View::default()
    };
    if !view.moved
        && notes_state == "ready"
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
    view.begun = !view.moved && (storage::begun(&sioul_core::config::state_dir()) || !view.said.is_empty());
    if view.moved {
        let changed = storage::changed_since(&sioul_core::config::state_dir());
        view.changed_key = if changed.is_empty() { String::new() } else { sioul_core::notices::digest(&changed.iter().map(|(file, sha)| format!("{file} {sha}")).collect::<Vec<_>>().join("\n")) };
        view.changed_old = changed.into_iter().map(|(file, _)| file).collect();
        view.old_files = storage::old_files(&sioul_core::config::state_dir());
        match take_out_gate(stand.as_ref()) {
            Ok(()) => view.take_out_open = true,
            Err(line) => view.take_out_waiting = line,
        }
        view.taken = TAKEN.lock().map(|t| t.clone()).unwrap_or_default();
    }
    view.offer = !view.moved && view.open && view.lost.is_empty() && notes_state != "unreadable";
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

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_sync::share::{Named, StorageStand};

    /// The gates of the move and of the take-out: no sharing, nothing to wait
    /// for; a sharing set up that cannot be read holds both, as a device
    /// missing would (review S22); a device not moved holds the take-out, named.
    #[test]
    fn a_sharing_that_cannot_be_read_holds_the_move_and_the_take_out() {
        assert!(move_gate(None).is_ok() && take_out_gate(None).is_ok());
        let unreadable = StorageStand { unreadable: true, ..StorageStand::default() };
        assert!(move_gate(Some(&unreadable)).is_err() && take_out_gate(Some(&unreadable)).is_err());
        let alone = StorageStand { alone: true, open: true, all_moved: true, ..StorageStand::default() };
        assert!(move_gate(Some(&alone)).is_ok() && take_out_gate(Some(&alone)).is_ok());
        let phone = Named { id: "p".into(), name: "Phone".into(), kind: "phone".into() };
        let one_to_move = StorageStand { open: true, unmoved: vec![phone], ..StorageStand::default() };
        assert!(move_gate(Some(&one_to_move)).is_ok());
        assert!(take_out_gate(Some(&one_to_move)).is_err_and(|line| line.contains("Phone")), "the device still to move named");
    }
}
