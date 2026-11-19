// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What lives where (docs/notes-folder.md, "What lives where"): Sioul's own
//! lists (projects, client profiles, budgets, the bank's lines, movements,
//! contracts, the papers' list, the letters' list and their texts) in
//! Sioul's private data folder, `records/`; the papers' files in a papers
//! folder, the letters' scans in a letters folder, each set on its device;
//! the notes folder holding the person's notes alone.
//!
//! Until a device moves, every list stays where it always was, at the root
//! of the notes folder, with `papers/` and `letters/` there (`Places::under`).
//! The move (`move_now`) is made once on each device, on the person's word,
//! once every device that counts knows the new places (the sharing's gate,
//! `sioul_sync::share::storage_stand`): it copies, checks every copy byte for
//! byte, keeps a dated copy of the whole in the data folder for
//! `KEEP_DAYS`, names the papers' and letters' files from their own folders
//! in the lists, and only then writes `storage.toml` in the state folder,
//! the one step after which this device reads the new places. The old files
//! stay in the notes folder, untouched, until every device has moved
//! (`clean`).
//!
//! Each step is safe to stop at any point: nothing is read from the new
//! places before `storage.toml` says so, every copy is made under a hidden
//! name and renamed into place once whole and checked, and the journal
//! (`storage-move.toml`, beside it) names what this move made, written
//! before each file takes its name, so that running it again finishes it
//! or makes it again, never twice, and never over a file it did not make.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Sioul's lists, in the data folder, once moved: `<data>/records/`.
pub const RECORDS: &str = "records";
/// Where this device stands (`Storage`), in the state folder: written once, at the switch.
pub const STATE_FILE: &str = "storage.toml";
/// The move's journal, in the state folder beside `STATE_FILE`.
pub const JOURNAL: &str = "storage-move.toml";
/// The dated copies kept by moves, in the data folder: `<data>/moved/<day>/`.
pub const KEPT: &str = "moved";
/// How long a dated copy is kept.
pub const KEEP_DAYS: i64 = 30;
/// The papers' folder in the notes folder's layout, and the name a
/// relative path of a paper started with there (`papers/<name>`).
pub const OLD_PAPERS: &str = "papers";
/// The letters' folder in the notes folder's layout (`letters/<year>/…`).
pub const OLD_LETTERS: &str = "letters";

/// Where this device keeps Sioul's lists now (`Config::storage`), read from
/// the state folder by `Config::load`, never from the configuration file:
/// nothing that travels may move it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Storage {
    /// When the lists left the notes folder (Unix seconds); 0: they did not,
    /// and the notes folder holds them, `papers/` and `letters/` as before.
    #[serde(default)]
    pub moved: i64,
    /// The notes folder they left: the place the sharing's memory knew them by.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from: String,
    /// When the old files were taken out of that folder (`clean`); 0: not yet.
    #[serde(default)]
    pub cleaned: i64,
    /// What is missing to know where the lists are (`Storage::read`): the
    /// state file there and unreadable, or gone while the journal says the
    /// move was made and its copies are not all there; none when known.
    /// While it is said, no list is read nor written anywhere, no move
    /// starts and nothing is deleted (absence proves nothing).
    #[serde(skip)]
    pub lost: Option<&'static str>,
    /// Read from the closed journal, the state file being gone
    /// (`Journal::switched`): the move was made, its copies checked there;
    /// written again by `recover`.
    #[serde(skip)]
    pub adopted: bool,
}

impl Storage {
    /// Whether this device reads its lists from the data folder.
    pub fn is_moved(&self) -> bool {
        self.moved > 0
    }

    /// The notes folder the lists left, once moved.
    pub fn from_path(&self) -> Option<PathBuf> {
        (self.is_moved() && !self.from.is_empty()).then(|| PathBuf::from(&self.from))
    }

    /// As `state` says (`STATE_FILE`). Gone: as the journal says, when it
    /// is closed (the switch was written) and every copy it names is there
    /// (`adopted`); else not moved, when no journal says a switch. A state
    /// file that does not read, or gone while a closed journal's copies are
    /// not all there, or a journal that does not read: `lost`, never "not
    /// moved" (the old files may have been taken out long ago), never
    /// "moved" either.
    pub fn read(state: &Path) -> Storage {
        let lost = |what: &'static str| Storage { lost: Some(what), ..Storage::default() };
        match std::fs::read_to_string(state.join(STATE_FILE)) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|_| lost("state-unreadable")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => match Journal::load(state) {
                Ok(Some(journal)) if journal.switched > 0 => {
                    if journal.copies_there() {
                        Storage { moved: journal.switched, from: journal.notes.clone(), cleaned: journal.cleaned, lost: None, adopted: true }
                    } else {
                        lost("state-missing")
                    }
                }
                Ok(_) => Storage::default(),
                Err(_) => lost("journal-unreadable"),
            },
            Err(_) => lost("state-unreadable"),
        }
    }

    /// The state file written again from a closed journal whose copies are
    /// all there (`Storage::read`, `adopted`): the move made is taken as
    /// made, nothing copied again, nothing deleted. Whether it was.
    pub fn recover(state: &Path) -> Result<bool, String> {
        let storage = Storage::read(state);
        if !storage.adopted {
            return Ok(false);
        }
        Storage { adopted: false, ..storage }.write(state)?;
        Ok(true)
    }

    /// This device's, in its own state folder.
    pub fn here() -> Storage {
        Storage::read(&crate::config::state_dir())
    }

    /// Written whole, its bytes on the disk before it takes its name: the
    /// switch is this one write.
    pub fn write(&self, state: &Path) -> Result<(), String> {
        std::fs::create_dir_all(state).map_err(|e| format!("{}: {e}", state.display()))?;
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::health::write_synced(&state.join(STATE_FILE), text.as_bytes())
    }
}

/// A folder of documents a list names (the papers', the letters'): where a
/// relative `file` of the list is read from, and where Sioul puts a file it
/// keeps. In the notes folder's layout the first is the notes folder and the
/// second its `papers/` or `letters/`; once moved, both are the folder
/// chosen for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Docs {
    /// What a relative `file` of a record is read from.
    pub base: PathBuf,
    /// Where Sioul copies a paper, or files a scan.
    pub folder: PathBuf,
    /// The folder's name in the notes folder's layout (`OLD_PAPERS`, `OLD_LETTERS`).
    pub old: &'static str,
    /// The notes folder: a path written by hand from it is still found there.
    pub notes: Option<PathBuf>,
}

impl Docs {
    /// Where a record's `file` is on this device. Absolute: there. Relative:
    /// from `base`; else in the other layout's form, so that a device not
    /// moved yet and one moved read each other's lists (`papers/x.pdf` read
    /// in a papers folder of its own, `x.pdf` in a notes folder's `papers/`);
    /// else from the notes folder; else where it would be from `base`.
    pub fn path_of(&self, file: &str) -> PathBuf {
        let path = Path::new(file);
        if path.is_absolute() {
            return path.to_path_buf();
        }
        let literal = join(&self.base, file);
        if literal.exists() {
            return literal;
        }
        let moved = self.base == self.folder;
        let other = if moved { file.strip_prefix(&format!("{}/", self.old)).map(|rest| join(&self.folder, rest)) } else { Some(join(&self.folder, file)) };
        other.filter(|p| p.exists()).or_else(|| self.notes.as_ref().map(|notes| join(notes, file)).filter(|p| p.exists())).unwrap_or(literal)
    }

    /// How a record names a file Sioul put in `folder`: from `base`, with `/`.
    pub fn named(&self, path: &Path) -> String {
        path.strip_prefix(&self.base).map_or_else(|_| path.display().to_string(), |rest| rest.to_string_lossy().replace('\\', "/"))
    }
}

/// `relative` (with `/`) under `base`, a part at a time.
fn join(base: &Path, relative: &str) -> PathBuf {
    relative.split('/').filter(|part| !part.is_empty()).fold(base.to_path_buf(), |path, part| path.join(part))
}

/// The places Sioul's stores read and write on this device (`Config::places`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// Sioul's lists: projects, client profiles, budgets, the bank's lines,
    /// movements, contracts, the papers' list; the letters' list and texts in
    /// its `letters/`.
    pub records: PathBuf,
    /// The notes folder, which a project names its notes from; none unset.
    pub notes: Option<PathBuf>,
    pub papers: Docs,
    pub letters: Docs,
    /// The lists left the notes folder.
    pub moved: bool,
}

impl Places {
    /// Everything under one folder, as before the move: the lists at its
    /// root, `papers/` and `letters/` in it.
    pub fn under(root: &Path) -> Places {
        let notes = Some(root.to_path_buf());
        Places {
            records: root.to_path_buf(),
            notes: notes.clone(),
            papers: Docs { base: root.to_path_buf(), folder: root.join(OLD_PAPERS), old: OLD_PAPERS, notes: notes.clone() },
            letters: Docs { base: root.to_path_buf(), folder: root.join(OLD_LETTERS), old: OLD_LETTERS, notes },
            moved: false,
        }
    }

    /// Each in its own place, once moved.
    pub fn apart(records: &Path, notes: Option<&Path>, papers: &Path, letters: &Path) -> Places {
        let notes = notes.map(Path::to_path_buf);
        Places {
            records: records.to_path_buf(),
            notes: notes.clone(),
            papers: Docs { base: papers.to_path_buf(), folder: papers.to_path_buf(), old: OLD_PAPERS, notes: notes.clone() },
            letters: Docs { base: letters.to_path_buf(), folder: letters.to_path_buf(), old: OLD_LETTERS, notes },
            moved: true,
        }
    }

    /// The letters' list and texts: `letters/` of the records.
    pub fn letters_records(&self) -> PathBuf {
        self.records.join(OLD_LETTERS)
    }
}

/// The person's documents folder, as each system names it (XDG's
/// `user-dirs.dirs` on Linux, the system's own elsewhere); on a phone, the
/// folder the notes are in when they are on its shared storage, else Sioul's
/// own storage, as the notes folder's default is.
pub fn documents(notes: Option<&Path>) -> PathBuf {
    if cfg!(target_os = "android") {
        return notes.filter(|n| n.starts_with("/storage") || n.starts_with("/sdcard")).and_then(Path::parent).map_or_else(|| crate::config::expand_home("~"), Path::to_path_buf);
    }
    directories::UserDirs::new().and_then(|dirs| dirs.document_dir().map(Path::to_path_buf)).unwrap_or_else(|| crate::config::expand_home("~/Documents"))
}

/// The papers' folder by default: `Papers` in the documents folder.
pub fn default_papers(notes: Option<&Path>) -> PathBuf {
    documents(notes).join("Papers")
}

/// The letters' folder by default: `Letters` in the documents folder.
pub fn default_letters(notes: Option<&Path>) -> PathBuf {
    documents(notes).join("Letters")
}

// ---------------------------------------------------------------- the move

/// The folders of a move: where each thing is, and where it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folders {
    /// The notes folder the lists leave.
    pub notes: PathBuf,
    /// Where the lists go: `<data>/records`.
    pub records: PathBuf,
    pub papers: PathBuf,
    pub letters: PathBuf,
    /// Where the dated copies go: `<data>/moved`.
    pub kept: PathBuf,
    /// Where `STATE_FILE` and the journal are: the state folder.
    pub state: PathBuf,
}

impl Folders {
    /// This device's: its notes folder, its data and state folders, and the
    /// papers' and letters' folders chosen.
    pub fn here(notes: &Path, papers: &Path, letters: &Path) -> Folders {
        let data = crate::config::data_dir();
        Folders { notes: notes.to_path_buf(), records: data.join(RECORDS), papers: papers.to_path_buf(), letters: letters.to_path_buf(), kept: data.join(KEPT), state: crate::config::state_dir() }
    }
}

/// What a file of the notes folder is in a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// One of Sioul's lists at the root, the projects' file's leftovers too.
    List,
    /// The letters' list or a letter's text, at the top of `letters/`.
    LetterRecord,
    /// A paper's file, in `papers/`.
    Paper,
    /// A scan, or anything else in `letters/` (its years, its inbox).
    Letter,
}

/// A file of the move, as the journal keeps it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Its path in the notes folder, with `/`.
    pub from: String,
    /// Where it goes.
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    /// The source's size, time (nanoseconds) and bytes' SHA-256 as copied.
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub time: i64,
    #[serde(default)]
    pub sha: String,
    /// This move made `to` (written before it took its name); false: it was
    /// there already with the same bytes, and is never taken back.
    #[serde(default)]
    pub made: bool,
    /// The bytes of `to` once its paths were named from their own folders
    /// (the papers' and letters' lists); empty when not rewritten.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rewritten: String,
    /// The dated copy of the source.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kept: String,
    /// No longer among what the notes folder holds to move, at a later try
    /// (its source removed, by hand or by a sync, or no longer named by its
    /// list): its copy stays where it is, the only one perhaps, never taken
    /// back, and its old file, if any, is never taken out.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub alone: bool,
    /// Made by the run that holds this journal in memory, never read from
    /// the disk: the only copies a failure may take back (`undo_run`).
    #[serde(skip)]
    pub this_run: bool,
    /// Its dated copy made by this run, the same.
    #[serde(skip)]
    pub kept_this_run: bool,
}

/// The journal of a move (`JOURNAL`): its folders and its files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    #[serde(default)]
    pub started: i64,
    /// The day of the dated copy (`YYYY-MM-DD`).
    #[serde(default)]
    pub day: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub records: String,
    #[serde(default)]
    pub papers: String,
    #[serde(default)]
    pub letters: String,
    /// Where its dated copies are: `<kept>/<day>`.
    #[serde(default)]
    pub kept: String,
    /// When the switch was written (Unix seconds), noted before the state
    /// file: from then on the journal is closed, the record of a move made,
    /// never work to finish or to undo (`open`), and the state file can be
    /// written again from it (`Storage::read`, `recover`).
    #[serde(default)]
    pub switched: i64,
    /// When the old files were taken out of the notes folder (`clean`).
    #[serde(default)]
    pub cleaned: i64,
    /// Copies an earlier try made in folders chosen otherwise since: left
    /// where they are (only the run that made a copy may take it back), said.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub earlier: Vec<String>,
    #[serde(default, rename = "file")]
    pub files: Vec<Entry>,
}

impl Journal {
    fn path(state: &Path) -> PathBuf {
        state.join(JOURNAL)
    }

    /// As written; none, or one that does not read: none.
    pub fn read(state: &Path) -> Option<Journal> {
        Journal::load(state).ok().flatten()
    }

    /// As written; none when there is none; one there that does not read: a
    /// fault, never "none" (it may say the move was made).
    pub fn load(state: &Path) -> Result<Option<Journal>, Fault> {
        match std::fs::read_to_string(Journal::path(state)) {
            Ok(text) => toml::from_str(&text).map(Some).map_err(|e| Fault::new("journal", format!("{}: {e}", Journal::path(state).display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Fault::new("journal", format!("{}: {e}", Journal::path(state).display()))),
        }
    }

    /// Every copy it names at its place, a plain file: what a closed journal
    /// must show before its move is taken as made without the state file.
    fn copies_there(&self) -> bool {
        self.files.iter().all(|entry| std::fs::symlink_metadata(&entry.to).is_ok_and(|meta| meta.is_file()))
    }

    /// Whether a copy this move put in one of its folders (records, papers,
    /// letters) is there, a plain file reached through no link below that folder.
    fn reached(&self, copy: &Path) -> bool {
        [&self.records, &self.papers, &self.letters].into_iter().filter(|root| !root.is_empty()).map(PathBuf::from).find(|root| copy.starts_with(root)).is_some_and(|root| {
            let rest = copy.strip_prefix(&root).map(|rest| rest.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            plain_below(&root, &rest).unwrap_or(false)
        })
    }

    fn write(&self, state: &Path) -> Result<(), String> {
        std::fs::create_dir_all(state).map_err(|e| format!("{}: {e}", state.display()))?;
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        crate::health::write_synced(&Journal::path(state), text.as_bytes())
    }

    fn of(folders: &Folders) -> (String, String, String, String) {
        let show = |p: &Path| p.display().to_string();
        (show(&folders.notes), show(&folders.records), show(&folders.papers), show(&folders.letters))
    }

    fn for_folders(&self, folders: &Folders) -> bool {
        (self.notes.clone(), self.records.clone(), self.papers.clone(), self.letters.clone()) == Journal::of(folders)
    }
}

/// Why a move did not happen, in one calm sentence's terms: a code and what
/// it names (`storage-<code>` in the messages).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub code: &'static str,
    pub what: String,
}

impl Fault {
    fn new(code: &'static str, what: impl std::fmt::Display) -> Fault {
        Fault { code, what: what.to_string() }
    }
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.what)
    }
}

/// What a move did, for the window's sentence and the trial.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Moved {
    pub lists: usize,
    pub papers: usize,
    pub letters: usize,
    /// Bytes copied.
    pub bytes: u64,
    /// The lists' paths named from their own folders: how many.
    pub named: usize,
    /// Copies kept whose source is no longer to move (removed since an
    /// earlier try): never taken back, said.
    pub alone: Vec<String>,
    /// Copies an earlier try left in folders chosen otherwise since: said.
    pub earlier: Vec<String>,
}

/// The lists' names at the root of a notes folder, the projects' file's
/// first name and the copy its renaming left among them.
fn list_names() -> Vec<&'static str> {
    vec![
        crate::projects::FILE,
        crate::projects::OLD_FILE,
        crate::clients::FILE,
        crate::budget::LEDGER,
        crate::bank::MANIFEST,
        crate::contracts::MANIFEST,
        crate::movements::STORE,
        crate::papers::MANIFEST,
    ]
}

/// What the notes folder holds of Sioul's to move, and where each goes
/// (`survey`'s entries).
pub fn plan(folders: &Folders) -> Result<Vec<Entry>, Fault> {
    Ok(survey(folders)?.entries)
}

/// What a move would do with the notes folder: what it moves, and what it
/// leaves where it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Survey {
    pub entries: Vec<Entry>,
    /// Files under `papers/` and `letters/` that no list of Sioul's names (a
    /// note kept there, a scan not read yet, a file dropped there by hand):
    /// left in the notes folder, said in the window, never taken out.
    pub left: Vec<String>,
}

/// What the notes folder holds of Sioul's, and where each goes: only what
/// Sioul's lists name (review S4). The lists at its root; the papers' list's
/// files under `papers/`; the letters' list, each letter's text `<id>.txt`
/// and each letter's scan under `letters/`. Anything else under `papers/`
/// and `letters/` stays (`Survey::left`); no note is ever moved. A link on
/// the way to a file to move is refused: its target could be anywhere, and
/// a copy of it would not be the file a record names.
pub fn survey(folders: &Folders) -> Result<Survey, Fault> {
    let notes = &folders.notes;
    let mut planned = Vec::new();
    // No notes folder set on this device: nothing to move, the lists begin in the records.
    if notes.as_os_str().is_empty() {
        return Ok(Survey::default());
    }
    if !notes.is_absolute() {
        return Err(Fault::new("folder-not-absolute", notes.display()));
    }
    // A notes folder set and not there now (a drive not plugged in, a vault not
    // opened, a network folder offline) is never "nothing to move" (rule 3), nor
    // one that holds nothing at all, as a drive's mount point looks unmounted.
    let mut listing = std::fs::read_dir(notes).map_err(|e| Fault::new("notes-unreadable", format!("{}: {e}", notes.display())))?;
    if listing.next().is_none() {
        return Err(Fault::new("notes-empty", notes.display()));
    }
    let regular = |relative: &str| plain_below(notes, relative);
    let mut names: Vec<String> = list_names().into_iter().map(str::to_string).collect();
    // The copies `projects::rename_file` kept: `sioul-cases.toml.before-rename`, `-2`…
    if let Ok(dir) = std::fs::read_dir(notes) {
        let mut kept: Vec<String> = dir.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with(crate::projects::BEFORE_RENAME)).collect();
        kept.sort();
        names.extend(kept);
    }
    for name in names {
        if regular(&name)? {
            planned.push(Entry { from: name.clone(), to: folders.records.join(&name).display().to_string(), kind: Some(Kind::List), ..Entry::default() });
        }
    }
    let add = |planned: &mut Vec<Entry>, from: String, to: PathBuf, kind: Kind| {
        if !planned.iter().any(|e: &Entry| e.from == from) {
            planned.push(Entry { from, to: to.display().to_string(), kind: Some(kind), ..Entry::default() });
        }
    };
    // The papers the papers' list names under `papers/`, each to the papers' folder by its path there.
    if regular(crate::papers::MANIFEST)? {
        for item in items_of(&notes.join(crate::papers::MANIFEST), "paper")? {
            let Some(from) = item.get("file").and_then(toml::Value::as_str).and_then(|file| old_place(file, notes, OLD_PAPERS)) else { continue };
            if regular(&from)? {
                let rest = from[OLD_PAPERS.len() + 1..].to_string();
                add(&mut planned, from, join(&folders.papers, &rest), Kind::Paper);
            }
        }
    }
    // The letters' list and each letter's text to Sioul's records; each letter's scan under
    // `letters/` (its years, its inbox) to the letters' folder; a scan linked where it is stays.
    let list = format!("{OLD_LETTERS}/{}", crate::letters::MANIFEST);
    if regular(&list)? {
        add(&mut planned, list.clone(), folders.records.join(OLD_LETTERS).join(crate::letters::MANIFEST), Kind::LetterRecord);
        for item in items_of(&notes.join(&list), "letter")? {
            if let Some(id) = item.get("id").and_then(toml::Value::as_str).filter(|id| !id.is_empty() && !id.starts_with('.') && !id.contains(['/', '\\'])) {
                let text = format!("{OLD_LETTERS}/{id}.txt");
                if regular(&text)? {
                    add(&mut planned, text, folders.records.join(OLD_LETTERS).join(format!("{id}.txt")), Kind::LetterRecord);
                }
            }
            if item.get("linked").and_then(toml::Value::as_bool).unwrap_or(false) {
                continue;
            }
            let Some(from) = item.get("file").and_then(toml::Value::as_str).and_then(|file| old_place(file, notes, OLD_LETTERS)) else { continue };
            if regular(&from)? {
                let rest = from[OLD_LETTERS.len() + 1..].to_string();
                add(&mut planned, from, join(&folders.letters, &rest), Kind::Letter);
            }
        }
    }
    // Whatever else is under `papers/` and `letters/`: left where it is, said.
    let mut left = Vec::new();
    for old in [OLD_PAPERS, OLD_LETTERS] {
        let root = notes.join(old);
        if !std::fs::symlink_metadata(&root).is_ok_and(|meta| meta.is_dir()) {
            continue;
        }
        let mut found = Vec::new();
        below(&root, &root, &mut found)?;
        left.extend(found.into_iter().map(|relative| format!("{old}/{relative}")).filter(|from| !planned.iter().any(|e| e.from == *from)));
    }
    Ok(Survey { entries: planned, left })
}

/// The elements of a list's array of tables (`[[paper]]`, `[[letter]]`), read
/// as plain TOML; a list that does not read refuses the move: what it names
/// cannot be told.
fn items_of(path: &Path, table: &str) -> Result<Vec<toml::Table>, Fault> {
    let text = std::fs::read_to_string(path).map_err(|e| Fault::new("unreadable", format!("{}: {e}", path.display())))?;
    let parsed: toml::Table = toml::from_str(&text).map_err(|e| Fault::new("list-unreadable", format!("{}: {e}", path.display())))?;
    Ok(parsed.get(table).and_then(toml::Value::as_array).map(|items| items.iter().filter_map(toml::Value::as_table).cloned().collect()).unwrap_or_default())
}

/// Where a record's `file` is in the notes folder's `<old>/` (`papers/…`,
/// `letters/…`), with `/`: relative under that name, absolute inside it
/// (as the notes folder is spelled, or resolved), or a bare name, as a
/// device that moved writes it, found there and not at the notes folder's
/// root (`Docs::path_of`'s order). None: elsewhere, it does not move.
fn old_place(file: &str, notes: &Path, old: &str) -> Option<String> {
    let path = Path::new(file);
    let rest = if path.is_absolute() {
        let inside = |root: &Path| path.strip_prefix(root.join(old)).ok().map(|rest| rest.to_string_lossy().replace('\\', "/"));
        inside(notes).or_else(|| inside(&resolved(notes)))?
    } else if let Some(rest) = file.strip_prefix(&format!("{old}/")) {
        rest.to_string()
    } else if !join(notes, file).exists() && join(&notes.join(old), file).exists() {
        file.to_string()
    } else {
        return None;
    };
    let parts: Vec<&str> = rest.split('/').filter(|part| !part.is_empty() && *part != ".").collect();
    (!parts.is_empty() && !parts.contains(&"..")).then(|| format!("{old}/{}", parts.join("/")))
}

/// Every file below `dir` that is no hidden one, a link among them (never
/// followed), by its path from `root` with `/`: what stays. A folder that
/// does not list is never taken for an empty one.
fn below(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), Fault> {
    let read = std::fs::read_dir(dir).map_err(|e| Fault::new("unreadable", format!("{}: {e}", dir.display())))?;
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            below(root, &path, out)?;
        } else {
            out.push(path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Every file below `dir`, by its path from `root` with `/`; hidden ones
/// (sync apps', editors', Sioul's own locks and passing files) left out; a
/// link refused (`plan`). A folder that is not there holds nothing; one
/// there that does not list is never taken for an empty one.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), Fault> {
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(Fault::new("unreadable", format!("{}: {e}", dir.display()))),
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| Fault::new("unreadable", format!("{}: {e}", path.display())))?;
        if kind.is_symlink() {
            return Err(Fault::new("link", path.display()));
        }
        if kind.is_dir() {
            walk(root, &path, out)?;
        } else if kind.is_file() {
            out.push(path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// The folders of a move, checked as the system resolves them (rule 4: the
/// same folder under another spelling, through a link or another case, is
/// the same folder): each its own, none inside another nor holding it, none
/// inside the notes folder or holding it, none inside Sioul's own records,
/// state or dated copies nor holding them.
pub fn check_folders(folders: &Folders) -> Result<(), Fault> {
    let real = |path: &Path| if path.as_os_str().is_empty() { PathBuf::new() } else { resolved(path) };
    let places = [("notes", real(&folders.notes)), ("records", real(&folders.records)), ("kept", real(&folders.kept)), ("state", real(&folders.state))];
    let (papers, letters) = (real(&folders.papers), real(&folders.letters));
    for (what, given, folder) in [("papers", &folders.papers, &papers), ("letters", &folders.letters, &letters)] {
        if given.as_os_str().is_empty() || !given.is_absolute() {
            return Err(Fault::new("folder-not-absolute", what));
        }
        for (other, place) in &places {
            if place.as_os_str().is_empty() {
                continue;
            }
            if folder.starts_with(place) || place.starts_with(folder) {
                return Err(Fault::new(if *other == "notes" { "folder-in-notes" } else { "folder-in-sioul" }, given.display()));
            }
        }
    }
    if papers.starts_with(&letters) || letters.starts_with(&papers) {
        return Err(Fault::new("folders-same", folders.papers.display()));
    }
    let (notes, records) = (&places[0].1, &places[1].1);
    if !notes.as_os_str().is_empty() && (records.starts_with(notes) || notes.starts_with(records)) {
        return Err(Fault::new("folder-in-notes", folders.records.display()));
    }
    Ok(())
}

/// A folder as the system resolves it: its longest part that exists made
/// real (links followed, `..` and the case a folder that folds it keeps),
/// the rest as written. Two spellings of one folder resolve alike.
pub fn resolved(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&existing) {
            return rest.iter().rev().fold(real, |path, part| path.join(part));
        }
        match (existing.file_name().map(std::ffi::OsStr::to_os_string), existing.parent().map(Path::to_path_buf)) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Whether two names lead to one file (a folder that folds case, a link
/// above them, a hard link): never copied over itself, never removed as
/// its own copy.
pub fn same_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        matches!((std::fs::metadata(a), std::fs::metadata(b)), (Ok(x), Ok(y)) if x.dev() == y.dev() && x.ino() == y.ino())
    }
    #[cfg(not(unix))]
    {
        matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
    }
}

/// The first of a trial's folders that is, holds or sits inside one of
/// `own` (a device's own configuration, data, state, notes, papers or
/// letters folder), as each resolves: (the folder's name in `folders`,
/// what it clashes with). A trial of the move (`sioul move-lists`) runs on
/// copies only (review S20).
pub fn clash(folders: &Folders, own: &[(&'static str, PathBuf)]) -> Option<(&'static str, &'static str)> {
    let given = [("notes", &folders.notes), ("records", &folders.records), ("papers", &folders.papers), ("letters", &folders.letters), ("kept", &folders.kept), ("state", &folders.state)];
    let own: Vec<(&'static str, PathBuf)> = own.iter().filter(|(_, place)| !place.as_os_str().is_empty()).map(|(what, place)| (*what, resolved(place))).collect();
    for (name, folder) in given.into_iter().filter(|(_, folder)| !folder.as_os_str().is_empty()) {
        let real = resolved(folder);
        if let Some((what, _)) = own.iter().find(|(_, place)| real.starts_with(place) || place.starts_with(&real)) {
            return Some((name, *what));
        }
    }
    None
}

/// A file's bytes' SHA-256, read a piece at a time.
pub fn sha_of(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether two files hold the same bytes, read side by side.
pub fn same_bytes(a: &Path, b: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let (mut x, mut y) = (std::fs::File::open(a)?, std::fs::File::open(b)?);
    if x.metadata()?.len() != y.metadata()?.len() {
        return Ok(false);
    }
    let (mut p, mut q) = (vec![0u8; 256 * 1024], vec![0u8; 256 * 1024]);
    loop {
        let n = read_full(&mut x, &mut p)?;
        let m = read_full(&mut y, &mut q)?;
        if n != m || p[..n] != q[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
    fn read_full(file: &mut std::fs::File, buffer: &mut [u8]) -> std::io::Result<usize> {
        let mut filled = 0;
        while filled < buffer.len() {
            let n = file.read(&mut buffer[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        Ok(filled)
    }
}

/// A file's size and time (nanoseconds since 1970), as a move notes its source.
fn stamp(path: &Path) -> Option<(u64, i64)> {
    let meta = std::fs::metadata(path).ok()?;
    let time = i64::try_from(meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos()).ok()?;
    Some((meta.len(), time))
}

#[cfg(test)]
thread_local! {
    /// A test's stop (`point`): the move stops as if killed at the point
    /// this counts down to, in this test's thread; 0, never.
    pub(crate) static STOP_AT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// A point a move may be killed at (a test stops it there: `STOP_AT`): what
/// is on the disk stays as it is, nothing taken back.
fn point(name: &str) -> Result<(), Fault> {
    #[cfg(test)]
    {
        let left = STOP_AT.with(std::cell::Cell::get);
        if left > 0 {
            STOP_AT.with(|stop| stop.set(left - 1));
            if left == 1 {
                return Err(Fault::new("stopped", name));
            }
        }
    }
    let _ = name;
    Ok(())
}

/// The hidden name a copy is made under, beside its place.
fn passing(to: &Path) -> PathBuf {
    let name = to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    to.with_file_name(format!(".{name}.sioul-move"))
}

/// `dir` made with the folders above it, each folder made flushed in its
/// parent, up to the first one that was there (a power cut after the switch
/// must not lose a folder the copies are in).
fn made_dirs(dir: &Path) -> std::io::Result<()> {
    let missing: Vec<PathBuf> = dir.ancestors().take_while(|d| !d.as_os_str().is_empty() && !d.exists()).map(Path::to_path_buf).collect();
    std::fs::create_dir_all(dir)?;
    for made in missing.iter().rev() {
        if let Some(parent) = made.parent() {
            sync_dir(parent);
        }
    }
    Ok(())
}

/// The folder's change made to last (a phone's file system may keep a name
/// whose bytes a crash lost); nothing to do where folders cannot be opened.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(folder) = std::fs::File::open(dir) {
        let _ = folder.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// `from` copied to `to` through a hidden name beside it: its bytes on the
/// disk, checked against the source byte for byte, dated as the source, then
/// renamed into place. The journal says `to` is this move's before the name
/// is taken (`note`). Returns the bytes copied.
fn copy_into(from: &Path, to: &Path, note: &mut dyn FnMut() -> Result<(), Fault>) -> Result<u64, Fault> {
    let fail = |e: std::io::Error| Fault::new("copy", format!("{}: {e}", to.display()));
    let parent = to.parent().ok_or_else(|| Fault::new("copy", to.display()))?;
    made_dirs(parent).map_err(fail)?;
    let temporary = passing(to);
    // A full disk leaves no half copy behind (review S6).
    let copied = crate::filelock::copy_whole(from, &temporary).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        fail(e)
    })?;
    let written = (|| -> std::io::Result<bool> {
        let file = std::fs::File::options().write(true).open(&temporary)?;
        // The source's own mode, not the private one a copy starts with: a papers' folder
        // may be read by other programs, a family's shared folder by other people (review S10).
        if let Ok(meta) = std::fs::metadata(from) {
            let _ = file.set_permissions(meta.permissions());
        }
        file.sync_all()?;
        if let Ok(time) = std::fs::metadata(from).and_then(|m| m.modified()) {
            let _ = file.set_modified(time);
        }
        same_bytes(from, &temporary)
    })();
    match written {
        Ok(true) => point("copied-aside")?,
        Ok(false) => {
            let _ = std::fs::remove_file(&temporary);
            return Err(Fault::new("changed", from.display()));
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            return Err(fail(e));
        }
    }
    if let Err(fault) = note() {
        let _ = std::fs::remove_file(&temporary);
        return Err(fault);
    }
    point("noted")?;
    std::fs::rename(&temporary, to).map_err(fail)?;
    sync_dir(parent);
    point("renamed")?;
    Ok(copied)
}

/// Steps 1 to 4 of a move, nothing switched: the lists copied to the
/// records, the papers and letters to their folders, each checked byte for
/// byte, a dated copy kept, the lists' paths named from their folders.
/// Safe to stop anywhere and to run again (the journal). Stopped here, as a
/// kill would: nothing taken back.
pub fn prepare(folders: &Folders, now: i64) -> Result<(Journal, Moved), Fault> {
    let mut journal = open(folders, now)?;
    let moved = fill(folders, &mut journal)?;
    Ok((journal, moved))
}

/// The journal of this move, as an earlier try left it or new. A closed
/// one (`Journal::switched`) is a move made, never work to do again. Folders
/// chosen otherwise since an earlier try: its copies outside the folders of
/// now stay where they are, named in `earlier` (only the run that made a
/// copy may take it back, and that run is over).
fn open(folders: &Folders, now: i64) -> Result<Journal, Fault> {
    match Journal::load(&folders.state)? {
        Some(journal) if journal.switched > 0 => Err(Fault::new("moved-already", folders.state.display())),
        Some(mut journal) => {
            if !journal.for_folders(folders) {
                let places = [&folders.records, &folders.papers, &folders.letters];
                let (stay, left): (Vec<Entry>, Vec<Entry>) = journal.files.drain(..).partition(|e| places.iter().any(|place| Path::new(&e.to).starts_with(place)));
                journal.earlier.extend(left.into_iter().filter(|e| e.made && Path::new(&e.to).exists()).map(|e| e.to));
                journal.files = stay;
                (journal.notes, journal.records, journal.papers, journal.letters) = Journal::of(folders);
                journal.write(&folders.state).map_err(|e| Fault::new("journal", e))?;
            }
            Ok(journal)
        }
        None => {
            let (notes, records, papers, letters) = Journal::of(folders);
            let day = day_of(now);
            let kept = folders.kept.join(&day).display().to_string();
            let journal = Journal { started: now, day, notes, records, papers, letters, kept, ..Journal::default() };
            journal.write(&folders.state).map_err(|e| Fault::new("journal", e))?;
            Ok(journal)
        }
    }
}

/// Steps 1 to 4 on an open journal (`prepare`).
fn fill(folders: &Folders, journal: &mut Journal) -> Result<Moved, Fault> {
    check_folders(folders)?;
    check_empty(folders, journal)?;
    let planned = plan(folders)?;
    let mut moved = Moved::default();
    for mut entry in planned {
        let from = join(&folders.notes, &entry.from);
        let known = journal.files.iter().position(|e| e.from == entry.from);
        if let Some(i) = known {
            entry.to.clone_from(&journal.files[i].to);
            entry.made = journal.files[i].made;
            entry.sha.clone_from(&journal.files[i].sha);
            entry.rewritten.clone_from(&journal.files[i].rewritten);
            entry.kept.clone_from(&journal.files[i].kept);
            entry.this_run = journal.files[i].this_run;
            entry.kept_this_run = journal.files[i].kept_this_run;
        }
        let (size, time) = stamp(&from).ok_or_else(|| Fault::new("unreadable", from.display()))?;
        let source = sha_of(&from).map_err(|e| Fault::new("unreadable", format!("{}: {e}", from.display())))?;
        let to = PathBuf::from(&entry.to);
        let _ = std::fs::remove_file(passing(&to));
        // The place it goes is the file itself under another name (a link above it, a
        // folder that folds case): never copied over itself, never cleaned as its own copy.
        if to.exists() && same_file(&from, &to) {
            return Err(Fault::new("folder-in-notes", to.display()));
        }
        // What is at its place: this move's, as it wrote it or named it (the journal of this
        // move says so), or the person's, whatever its bytes: never adopted on a guess.
        let there = if to.exists() { Some(sha_of(&to).map_err(|e| Fault::new("unreadable", format!("{}: {e}", to.display())))?) } else { None };
        let (made, copied, named) = (entry.made, entry.sha.clone(), entry.rewritten.clone());
        let ours = |sha: &str| made && (sha == copied || (!named.is_empty() && sha == named));
        match there.as_deref() {
            Some(sha) if made && (sha == source || ours(sha)) => {}
            // Another file at its place, never written over (the papers' and letters' folders
            // were checked empty but for this move's: a list's place in Sioul's records).
            Some(_) => return Err(Fault::new("records-taken", to.display())),
            None => {}
        }
        let needs_copy = !to.exists() || there.as_deref().is_some_and(|sha| sha != source && ours(sha));
        entry.to = to.display().to_string();
        entry.kind = entry.kind.or(Some(Kind::Letter));
        entry.alone = false;
        (entry.size, entry.time, entry.sha) = (size, time, source);
        if needs_copy {
            entry.made = true;
            entry.this_run = true;
            entry.rewritten.clear();
            entry.kept.clear();
            entry.kept_this_run = false;
            let noted = entry.clone();
            let journal_now = &mut *journal;
            let state = folders.state.clone();
            let mut note = || -> Result<(), Fault> {
                upsert(journal_now, noted.clone());
                journal_now.write(&state).map_err(|e| Fault::new("journal", e))
            };
            moved.bytes += copy_into(&from, &to, &mut note)?;
        }
        match entry.kind {
            Some(Kind::List | Kind::LetterRecord) => moved.lists += 1,
            Some(Kind::Paper) => moved.papers += 1,
            _ => moved.letters += 1,
        }
        upsert(journal, entry);
    }
    // No longer among what is to move since an earlier try (its source removed
    // by hand or by a sync, a folder that did not list): its copy stays, perhaps
    // the only one, and its dated copy with it; said, never taken back.
    let planned_now: Vec<String> = plan(folders)?.into_iter().map(|e| e.from).collect();
    for entry in journal.files.iter_mut().filter(|e| !planned_now.contains(&e.from)) {
        if !entry.alone {
            entry.alone = true;
            moved.alone.push(entry.to.clone());
        }
    }
    journal.files.retain(|e| !e.alone || Path::new(&e.to).exists());
    moved.earlier.clone_from(&journal.earlier);
    journal.write(&folders.state).map_err(|e| Fault::new("journal", e))?;
    point("copied")?;
    check(folders, journal)?;
    point("checked")?;
    keep(folders, journal)?;
    journal.write(&folders.state).map_err(|e| Fault::new("journal", e))?;
    point("kept")?;
    moved.named = name_paths(folders, journal)?;
    journal.write(&folders.state).map_err(|e| Fault::new("journal", e))?;
    point("named")?;
    Ok(moved)
}

/// The papers' and the letters' folders: empty, or not there yet, but for
/// what this move wrote there at an earlier try (its journal says so).
/// Everything in them is shared by Sioul, sealed, with every device: a
/// folder that holds anything else is refused with one sentence, never
/// filled beside it (review S3). Hidden files (a sync app's marks, a copy
/// on its way) are passed over.
fn check_empty(folders: &Folders, journal: &Journal) -> Result<(), Fault> {
    for folder in [&folders.papers, &folders.letters] {
        let mut there = Vec::new();
        walk(folder, folder, &mut there)?;
        let ours = |relative: &String| journal.files.iter().any(|e| e.made && Path::new(&e.to) == join(folder, relative));
        if there.iter().any(|relative| !ours(relative)) {
            return Err(Fault::new("folder-not-empty", folder.display()));
        }
    }
    Ok(())
}

/// The journal's entry for `entry.from` replaced, or added.
fn upsert(journal: &mut Journal, entry: Entry) {
    match journal.files.iter_mut().find(|e| e.from == entry.from) {
        Some(place) => *place = entry,
        None => journal.files.push(entry),
    }
}

/// Step 2: every copy against its source, byte for byte (a list against its
/// source before its paths are named: the bytes then are those noted).
fn check(folders: &Folders, journal: &Journal) -> Result<(), Fault> {
    for entry in journal.files.iter().filter(|e| !e.alone) {
        let (from, to) = (join(&folders.notes, &entry.from), PathBuf::from(&entry.to));
        // As copied; or, a list whose paths were named, its source as copied and its copy as named.
        let same = same_bytes(&from, &to).map_err(|e| Fault::new("unreadable", format!("{}: {e}", to.display())))?
            || (!entry.rewritten.is_empty() && sha_of(&from).is_ok_and(|sha| sha == entry.sha) && sha_of(&to).is_ok_and(|sha| sha == entry.rewritten));
        if !same {
            return Err(Fault::new("changed", from.display()));
        }
    }
    Ok(())
}

/// Step 3: a dated copy of every source in `<kept>/<day>/`, its path from
/// the notes folder kept: a copy of its own, never a second name of the old
/// file (an editor or a sync that writes the old file in place would write
/// it too), a copy-on-write clone where the file system gives one (the
/// system's copy: Btrfs and XFS on Linux, APFS on a Mac), its bytes on the
/// disk before it counts.
fn keep(folders: &Folders, journal: &mut Journal) -> Result<(), Fault> {
    let day = PathBuf::from(&journal.kept);
    for entry in journal.files.iter_mut().filter(|e| !e.alone) {
        let from = join(&folders.notes, &entry.from);
        let to = join(&day, &entry.from);
        if to.exists() && !same_file(&from, &to) && same_bytes(&from, &to).unwrap_or(false) {
            entry.kept = to.display().to_string();
            continue;
        }
        let fail = |e: std::io::Error| Fault::new("keep", format!("{}: {e}", to.display()));
        let parent = to.parent().unwrap_or(&day).to_path_buf();
        made_dirs(&parent).map_err(fail)?;
        let _ = std::fs::remove_file(&to);
        std::fs::copy(&from, &to).map_err(fail)?;
        std::fs::File::open(&to).and_then(|file| file.sync_all()).map_err(fail)?;
        sync_dir(&parent);
        entry.kept_this_run = true;
        if same_file(&from, &to) || !same_bytes(&from, &to).map_err(fail)? {
            return Err(Fault::new("changed", from.display()));
        }
        entry.kept = to.display().to_string();
    }
    Ok(())
}

/// Step 4: the papers' and letters' lists, at their new place, naming each
/// file from its own folder: `papers/x.pdf` becomes `x.pdf`, a path in the
/// notes folder's `papers/` the same, a file given another name its new one;
/// a path elsewhere stays as it is. Written beside, checked to change
/// nothing but those paths, then renamed into place. Returns how many paths
/// were named anew.
fn name_paths(folders: &Folders, journal: &mut Journal) -> Result<usize, Fault> {
    // Only the files the move copied (or kept, their sources gone since): a record whose file
    // was not moved keeps its path as it is (review S5).
    let renamed: BTreeMap<String, String> = journal.files.iter().filter(|e| matches!(e.kind, Some(Kind::Paper | Kind::Letter))).map(|e| (e.from.clone(), e.to.clone())).collect();
    let mut named = 0;
    for (list, table, old, base) in [(crate::papers::MANIFEST.to_string(), "paper", OLD_PAPERS, &folders.papers), (format!("{OLD_LETTERS}/{}", crate::letters::MANIFEST), "letter", OLD_LETTERS, &folders.letters)] {
        let Some(i) = journal.files.iter().position(|e| e.from == list) else { continue };
        let to = PathBuf::from(&journal.files[i].to);
        let unreadable = |e: std::io::Error| Fault::new("unreadable", format!("{}: {e}", to.display()));
        if !journal.files[i].rewritten.is_empty() && sha_of(&to).is_ok_and(|sha| sha == journal.files[i].rewritten) {
            continue;
        }
        let text = std::fs::read_to_string(&to).map_err(unreadable)?;
        let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e| Fault::new("list-unreadable", format!("{}: {e}", to.display())))?;
        let mut count = 0;
        if let Some(items) = doc.get_mut(table).and_then(toml_edit::Item::as_array_of_tables_mut) {
            for item in items.iter_mut() {
                let Some(file) = item.get("file").and_then(toml_edit::Item::as_str).map(str::to_string) else { continue };
                if let Some(new) = named_from(&file, &folders.notes, old, base, &renamed) {
                    item["file"] = toml_edit::value(new);
                    count += 1;
                }
            }
        }
        let written = if count > 0 { doc.to_string() } else { text.clone() };
        same_but_files(&text, &written, table).map_err(|e| Fault::new("list-unreadable", format!("{}: {e}", to.display())))?;
        // The journal first: a move stopped once the file took its new bytes knows them for its own.
        journal.files[i].rewritten = hex(&Sha256::digest(written.as_bytes()));
        journal.write(&folders.state).map_err(|e| Fault::new("journal", e))?;
        if count > 0 {
            point("naming")?;
            // Flushed before it counts (review S8): nothing but the move writes there yet.
            crate::health::write_synced(&to, written.as_bytes()).map_err(|e| Fault::new("copy", e))?;
            point("renamed-paths")?;
        }
        if !sha_of(&to).is_ok_and(|sha| sha == journal.files[i].rewritten) {
            return Err(Fault::new("changed", to.display()));
        }
        named += count;
    }
    Ok(named)
}

/// A record's `file` named from its folder's new place, when the move
/// copied that file there: read as the plan reads it (`old_place`), named
/// by its copy's path in that folder. None: it was not moved, and stays as
/// it is (review S5).
fn named_from(file: &str, notes: &Path, old: &str, base: &Path, renamed: &BTreeMap<String, String>) -> Option<String> {
    let from = old_place(file, notes, old)?;
    let to = PathBuf::from(renamed.get(&from)?);
    to.strip_prefix(base).ok().map(|rest| rest.to_string_lossy().replace('\\', "/"))
}

/// Whether two texts of a list hold the same tables, the same values, but
/// for each table's `file`.
fn same_but_files(before: &str, after: &str, table: &str) -> Result<(), String> {
    let strip = |text: &str| -> Result<toml::Table, String> {
        let mut parsed: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        if let Some(toml::Value::Array(items)) = parsed.get_mut(table) {
            for item in items.iter_mut() {
                if let Some(item) = item.as_table_mut() {
                    item.remove("file");
                }
            }
        }
        Ok(parsed)
    };
    if strip(before)? == strip(after)? { Ok(()) } else { Err("the list would change beyond its paths".into()) }
}

/// The day of a time (UTC), for the dated copy's folder.
fn day_of(now: i64) -> String {
    jiff::Timestamp::from_second(now).map(|t| t.to_zoned(jiff::tz::TimeZone::UTC).date().to_string()).unwrap_or_else(|_| "unknown".into())
}

/// Step 5, the switch, with `hold` held (the sharing's exchanges' lock: no
/// exchange writes the old lists meanwhile) and every old list's own lock
/// (Sioul's writers wait): each source looked at once more, a list or a
/// file changed since its copy copied again (a list's paths named again),
/// then `STATE_FILE` written. From then on this device reads the new places.
pub fn switch(folders: &Folders, journal: &mut Journal, now: i64) -> Result<(), Fault> {
    let locks: Vec<PathBuf> = journal.files.iter().filter(|e| matches!(e.kind, Some(Kind::List | Kind::LetterRecord))).map(|e| join(&folders.notes, &e.from)).filter(|p| crate::filelock::has_lock(p)).collect();
    with_locks(&locks, &mut || {
        let mut changed = false;
        // Planned again: what came since the plan (an exchange, the person, a sync: a paper,
        // a scan, a letter's text, a first list) is copied and checked here too, never left
        // behind while the lists are named from the new folders (review S2).
        let came: Vec<Entry> = plan(folders)?.into_iter().filter(|planned| !journal.files.iter().any(|e| e.from == planned.from)).collect();
        for entry in came {
            let from = join(&folders.notes, &entry.from);
            let to = PathBuf::from(&entry.to);
            if to.exists() {
                return Err(Fault::new("records-taken", to.display()));
            }
            let (size, time) = stamp(&from).ok_or_else(|| Fault::new("unreadable", from.display()))?;
            let sha = sha_of(&from).map_err(|e| Fault::new("unreadable", format!("{}: {e}", from.display())))?;
            let fresh = Entry { size, time, sha, made: true, this_run: true, ..entry };
            let state = folders.state.clone();
            let noted = fresh.clone();
            let mut note = || -> Result<(), Fault> {
                upsert(journal, noted.clone());
                journal.write(&state).map_err(|e| Fault::new("journal", e))
            };
            copy_into(&from, &to, &mut note)?;
            upsert(journal, fresh);
            changed = true;
        }
        for i in 0..journal.files.len() {
            let entry = journal.files[i].clone();
            if entry.alone {
                continue;
            }
            let from = join(&folders.notes, &entry.from);
            // A list read whole (small, and what an exchange writes); a paper or a scan by its size and time, read whole before.
            let list = matches!(entry.kind, Some(Kind::List | Kind::LetterRecord));
            if stamp(&from) == Some((entry.size, entry.time)) && (!list || sha_of(&from).is_ok_and(|sha| sha == entry.sha)) {
                continue;
            }
            // Changed since its copy (an exchange, a writer, a sync app): copied again, here.
            let (size, time) = stamp(&from).ok_or_else(|| Fault::new("unreadable", from.display()))?;
            let sha = sha_of(&from).map_err(|e| Fault::new("unreadable", format!("{}: {e}", from.display())))?;
            let to = PathBuf::from(&entry.to);
            // Only over this move's own copy, as it wrote or named it (review S7).
            let ours = entry.made && sha_of(&to).is_ok_and(|now| now == entry.sha || (!entry.rewritten.is_empty() && now == entry.rewritten));
            if to.exists() && !ours {
                return Err(Fault::new("records-taken", to.display()));
            }
            let mut fresh = Entry { size, time, sha, made: true, this_run: true, rewritten: String::new(), ..entry };
            let state = folders.state.clone();
            let noted = fresh.clone();
            let mut note = || -> Result<(), Fault> {
                upsert(journal, noted.clone());
                journal.write(&state).map_err(|e| Fault::new("journal", e))
            };
            copy_into(&from, &to, &mut note)?;
            fresh.kept.clear();
            upsert(journal, fresh);
            changed = true;
        }
        if changed {
            check(folders, journal)?;
            keep(folders, journal)?;
            name_paths(folders, journal)?;
            journal.write(&folders.state).map_err(|e| Fault::new("journal", e))?;
        }
        point("switching")?;
        // The journal closed first: from now on it is the record of a move made,
        // never work to undo, and the state file can be written again from it.
        journal.switched = now;
        journal.write(&folders.state).map_err(|e| Fault::new("journal", e))?;
        point("closed")?;
        let storage = Storage { moved: now, from: folders.notes.display().to_string(), ..Storage::default() };
        storage.write(&folders.state).map_err(|e| Fault::new("switch", e))
    })
}

/// `f` with the locks of `paths` held, one after the other.
fn with_locks<T>(paths: &[PathBuf], f: &mut dyn FnMut() -> T) -> T {
    match paths.split_first() {
        None => f(),
        Some((first, rest)) => crate::filelock::with_lock(first, || with_locks(rest, f)),
    }
}

/// The whole move, without the window: `prepare`, then `switch` with
/// `hold` held (the app holds the sharing's exchanges' lock; the command
/// line holds nothing), the whole under the move's own lock (two Sioul on
/// one device never move at once). Before the switch, a failure takes back
/// what this run made, and only that (`undo_run`): the old places stay as
/// they were. Already moved, or a closed journal whose state file is gone
/// (written again from it, `recover`): nothing is done, nothing deleted.
pub fn move_now<G>(folders: &Folders, now: i64, hold: impl FnOnce() -> Result<G, String>) -> Result<Moved, Fault> {
    crate::filelock::with_lock(&Journal::path(&folders.state), || {
        let _ = Storage::recover(&folders.state);
        let here = Storage::read(&folders.state);
        if here.is_moved() {
            return Err(Fault::new("moved-already", folders.state.display()));
        }
        if let Some(lost) = here.lost {
            return Err(Fault::new(lost, folders.state.display()));
        }
        let mut journal = open(folders, now)?;
        let outcome = (|| {
            let moved = fill(folders, &mut journal)?;
            let held = hold().map_err(|e| Fault::new("busy", e))?;
            let switched = switch(folders, &mut journal, now);
            drop(held);
            switched.map(|()| moved)
        })();
        if outcome.is_err() && journal.switched == 0 && !Storage::read(&folders.state).is_moved() {
            undo_run(folders, &mut journal);
        }
        outcome
    })
}

/// A profile that holds nothing of Sioul's in the old places starts in the
/// new layout (review S29): no state file, no journal, a notes folder set
/// that reads and holds notes but no list, no `papers/` nor `letters/` (or
/// none set), nothing in Sioul's records. Written as a move made with
/// nothing in it. A notes folder that does not read, or holds nothing at all
/// (a drive's mount point unmounted), never starts so: absence proves
/// nothing. Whether it was.
pub fn start_moved(folders: &Folders, now: i64) -> Result<bool, Fault> {
    crate::filelock::with_lock(&Journal::path(&folders.state), || {
        if std::fs::symlink_metadata(folders.state.join(STATE_FILE)).is_ok() || std::fs::symlink_metadata(Journal::path(&folders.state)).is_ok() {
            return Ok(false);
        }
        let Ok(survey) = survey(folders) else { return Ok(false) };
        let old = |name: &str| !folders.notes.as_os_str().is_empty() && std::fs::symlink_metadata(folders.notes.join(name)).is_ok();
        if !survey.entries.is_empty() || !survey.left.is_empty() || old(OLD_PAPERS) || old(OLD_LETTERS) {
            return Ok(false);
        }
        let mut there = Vec::new();
        if walk(&folders.records, &folders.records, &mut there).is_err() || !there.is_empty() {
            return Ok(false);
        }
        Storage { moved: now, from: folders.notes.display().to_string(), ..Storage::default() }.write(&folders.state).map_err(|e| Fault::new("switch", e))?;
        Ok(true)
    })
}

/// What this run made, taken back after its failure, before any switch:
/// each copy it made that still holds the bytes it wrote while its source is
/// still there with those bytes (a copy whose source is gone is the only one
/// left: it stays), the dated copies it made, the folders it left empty.
/// Never a copy an earlier run made, never a file the move did not write;
/// the journal keeps what stays, and goes when nothing does.
fn undo_run(folders: &Folders, journal: &mut Journal) {
    let roots: Vec<PathBuf> = [&folders.records, &folders.papers, &folders.letters].into_iter().map(PathBuf::clone).collect();
    let mut stays = Vec::new();
    for entry in journal.files.drain(..) {
        let to = PathBuf::from(&entry.to);
        let _ = std::fs::remove_file(passing(&to));
        if entry.kept_this_run && !entry.kept.is_empty() {
            let kept = PathBuf::from(&entry.kept);
            if kept.starts_with(&journal.kept) {
                let _ = std::fs::remove_file(&kept);
            }
        }
        let source_there = sha_of(&join(&folders.notes, &entry.from)).is_ok_and(|sha| sha == entry.sha);
        let as_written = sha_of(&to).is_ok_and(|sha| sha == entry.sha || (!entry.rewritten.is_empty() && sha == entry.rewritten));
        if entry.this_run && entry.made && source_there && as_written && std::fs::remove_file(&to).is_ok() {
            if let Some(root) = roots.iter().find(|root| to.starts_with(root)) {
                for dir in to.ancestors().skip(1).take_while(|dir| *dir != root.as_path() && dir.starts_with(root)) {
                    if std::fs::remove_dir(dir).is_err() {
                        break;
                    }
                }
            }
            continue;
        }
        if to.exists() {
            stays.push(Entry { kept: if entry.kept_this_run { String::new() } else { entry.kept.clone() }, ..entry });
        }
    }
    journal.files = stays;
    remove_empty(Path::new(&journal.kept));
    if journal.files.is_empty() && journal.earlier.is_empty() {
        let _ = std::fs::remove_file(Journal::path(&folders.state));
    } else {
        let _ = journal.write(&folders.state);
    }
}

/// A papers' or letters' folder chosen again once moved (Papers ▸ ⚙,
/// Letters ▸ ⚙): every file of `from` copied to `to` and checked byte for
/// byte, never over a file already there that differs (refused, said); then
/// `switch` (the caller writes the setting, the sharing's exchanges held);
/// then the list's paths into `from` written whole named from `to`. A
/// relative path stays as it is: the same name in the new folder. The old
/// folder stays as it was. Before the switch, a failure takes back the
/// copies made; stopped there, the next run finds them, the same bytes.
pub fn move_documents(from: &Path, to: &Path, list: &Path, table: &str, switch: impl FnOnce() -> Result<(), String>) -> Result<Moved, Fault> {
    if !to.is_absolute() {
        return Err(Fault::new("folder-not-absolute", to.display()));
    }
    let (real_from, real_to) = (resolved(from), resolved(to));
    if real_to.starts_with(&real_from) || real_from.starts_with(&real_to) {
        return Err(Fault::new("folders-same", to.display()));
    }
    // Empty, or not there yet: everything in it is shared by Sioul (review S3, S25).
    let mut there = Vec::new();
    walk(to, to, &mut there)?;
    if !there.is_empty() {
        return Err(Fault::new("folder-not-empty", to.display()));
    }
    let mut found = Vec::new();
    walk(from, from, &mut found)?;
    let mut moved = Moved::default();
    let mut made: Vec<PathBuf> = Vec::new();
    let copied = (|| -> Result<(), Fault> {
        for relative in &found {
            let (source, target) = (join(from, relative), join(to, relative));
            // Another file at its place: never written over, never taken for the copy (review S3, S23).
            if target.exists() {
                return Err(Fault::new("records-taken", target.display()));
            }
            moved.bytes += copy_into(&source, &target, &mut || Ok(()))?;
            made.push(target);
            moved.papers += 1;
        }
        for relative in &found {
            if !same_bytes(&join(from, relative), &join(to, relative)).unwrap_or(false) {
                return Err(Fault::new("changed", join(from, relative).display()));
            }
        }
        switch().map_err(|e| Fault::new("switch", e))
    })();
    if let Err(fault) = copied {
        for path in made {
            let _ = std::fs::remove_file(path);
        }
        return Err(fault);
    }
    // Paths written whole into the old folder, named from the new one.
    crate::filelock::with_lock(list, || -> Result<(), Fault> {
        let Ok(text) = std::fs::read_to_string(list) else { return Ok(()) };
        let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e| Fault::new("list-unreadable", format!("{}: {e}", list.display())))?;
        let mut count = 0;
        if let Some(items) = doc.get_mut(table).and_then(toml_edit::Item::as_array_of_tables_mut) {
            for item in items.iter_mut() {
                let Some(file) = item.get("file").and_then(toml_edit::Item::as_str).filter(|f| Path::new(f).is_absolute()).map(str::to_string) else { continue };
                if let Ok(rest) = Path::new(&file).strip_prefix(from) {
                    item["file"] = toml_edit::value(rest.to_string_lossy().replace('\\', "/"));
                    count += 1;
                }
            }
        }
        if count > 0 {
            let written = doc.to_string();
            same_but_files(&text, &written, table).map_err(|e| Fault::new("list-unreadable", format!("{}: {e}", list.display())))?;
            crate::health::write_synced(list, written.as_bytes()).map_err(|e| Fault::new("copy", e))?;
        }
        moved.named = count;
        Ok(())
    })?;
    Ok(moved)
}

/// What taking the old files out left.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cleaned {
    pub removed: usize,
    /// Old files changed since they were copied (someone wrote there after the move): kept, said.
    pub changed: Vec<String>,
    /// Old files kept for another reason, each with its code (`storage-kept-<code>`):
    /// "link" (a link on its way, rule 4), "same-file" (its copy is the same
    /// file under another name), "copy-missing", "copy-differs" (rule 1),
    /// "not-removed" (the system refused: a file open elsewhere, a permission).
    pub kept: Vec<(String, &'static str)>,
}

impl Cleaned {
    /// Nothing of Sioul's left in the notes folder by this take-out.
    pub fn whole(&self) -> bool {
        self.changed.is_empty() && self.kept.is_empty()
    }
}

/// Whether `relative` (with `/`) below `root` is a plain file reached
/// through no link: `Ok(false)` when it is not there; a link on its way, or
/// a `..` that would leave `root`, refused (rule 4: a link anywhere in a
/// path below a folder). `root` itself may be reached through links: it is
/// the folder as set.
pub fn plain_below(root: &Path, relative: &str) -> Result<bool, Fault> {
    let parts: Vec<&str> = relative.split('/').filter(|part| !part.is_empty() && *part != ".").collect();
    let mut path = root.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        if *part == ".." {
            return Err(Fault::new("link", join(root, relative).display()));
        }
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(Fault::new("link", path.display())),
            Ok(meta) if i + 1 == parts.len() => return Ok(meta.is_file()),
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(Fault::new("unreadable", format!("{}: {e}", path.display()))),
        }
    }
    Ok(false)
}

/// The old files taken out of the notes folder, on the person's press once
/// every device has moved (Settings ▸ Sharing, "Take them out"; never by
/// itself, never on a guess about who else reads that folder): each only
/// against its copy (rule 1), one written there since stays, said; the
/// lists' lock files beside them, and the `papers/` and `letters/` folders
/// left empty. Said done in `STATE_FILE` and the journal once nothing is
/// left; the move's own lock held, as for the move.
pub fn clean(state: &Path, now: i64) -> Result<Cleaned, String> {
    crate::filelock::with_lock(&Journal::path(state), || clean_held(state, now))
}

fn clean_held(state: &Path, now: i64) -> Result<Cleaned, String> {
    let mut storage = Storage::read(state);
    let Some(notes) = storage.from_path() else { return Ok(Cleaned::default()) };
    let journal = Journal::read(state).ok_or("no journal")?;
    // A notes folder not there now (a drive not plugged in, a permission taken back) is no
    // folder whose old files are gone: nothing is taken out, nothing said done (rule 3).
    std::fs::read_dir(&notes).map_err(|e| format!("{}: {e}", notes.display()))?;
    let mut cleaned = Cleaned::default();
    for entry in journal.files.iter().filter(|e| !e.alone) {
        let from = join(&notes, &entry.from);
        // Reached through a link (a `papers/` replaced by a link to the papers' folder, a sync
        // that carries links): never read nor removed through it, the copy may be behind it.
        match plain_below(&notes, &entry.from) {
            Ok(true) => {}
            // Gone already (taken out by hand): its lock file, Sioul's own and hidden, goes too.
            Ok(false) if std::fs::symlink_metadata(&from).is_err() => {
                let name = from.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let _ = std::fs::remove_file(from.with_file_name(format!(".{name}.lock")));
                continue;
            }
            Ok(false) => {
                cleaned.kept.push((entry.from.clone(), "not-removed"));
                continue;
            }
            Err(fault) if fault.code == "link" => {
                cleaned.kept.push((entry.from.clone(), "link"));
                continue;
            }
            Err(_) => {
                cleaned.kept.push((entry.from.clone(), "not-removed"));
                continue;
            }
        }
        // Its copy the same file under another name: it is the only one.
        if same_file(&from, Path::new(&entry.to)) {
            cleaned.kept.push((entry.from.clone(), "same-file"));
            continue;
        }
        // Rule 1: an old file goes only against its copy. Itself as it was copied (one
        // written there since is someone's change: kept, said)…
        match sha_of(&from) {
            Ok(sha) if sha == entry.sha => {}
            Ok(_) => {
                cleaned.changed.push(entry.from.clone());
                continue;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                cleaned.kept.push((entry.from.clone(), "not-removed"));
                continue;
            }
        }
        // …its copy at the new place, a plain file reached through no link…
        let copy = PathBuf::from(&entry.to);
        if !journal.reached(&copy) {
            cleaned.kept.push((entry.from.clone(), "copy-missing"));
            continue;
        }
        // …and the same bytes at this moment: a paper's or a scan's copy itself; a list's, which
        // Sioul's writers change by use since the move (and whose paths were named anew), its
        // dated copy, a copy of its own in Sioul's folder, as the copy of what it held then.
        let against = if matches!(entry.kind, Some(Kind::List | Kind::LetterRecord)) { PathBuf::from(&entry.kept) } else { copy };
        let dated_plain = against.starts_with(&journal.kept) && std::fs::symlink_metadata(&against).is_ok_and(|meta| meta.is_file());
        if (against != PathBuf::from(&entry.to) && !dated_plain) || against.as_os_str().is_empty() {
            cleaned.kept.push((entry.from.clone(), "copy-missing"));
            continue;
        }
        if same_file(&from, &against) || !same_bytes(&from, &against).unwrap_or(false) {
            cleaned.kept.push((entry.from.clone(), "copy-differs"));
            continue;
        }
        let removed = std::fs::remove_file(&from).is_ok();
        if removed {
            cleaned.removed += 1;
        } else {
            cleaned.kept.push((entry.from.clone(), "not-removed"));
        }
        // Its lock, beside it, once it is gone: Sioul's own, hidden.
        if removed {
            let name = from.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let _ = std::fs::remove_file(from.with_file_name(format!(".{name}.lock")));
        }
    }
    for old in [OLD_PAPERS, OLD_LETTERS] {
        remove_empty(&notes.join(old));
    }
    if cleaned.whole() {
        storage.cleaned = now;
        storage.adopted = false;
        storage.write(state)?;
        let mut journal = journal;
        journal.cleaned = now;
        journal.write(state)?;
    }
    Ok(cleaned)
}

/// The old files of the notes folder changed since this device's lists
/// left them (written by hand, by an agent, by a notes sync from a device
/// not moved yet): each with its bytes' SHA-256 now, for the Porch to name
/// it once; nothing is merged by itself, and the take-out leaves them
/// (rule 1). Looked at by size and time first, read only when they moved.
pub fn changed_since(state: &Path) -> Vec<(String, String)> {
    let storage = Storage::read(state);
    let (Some(notes), Ok(Some(journal))) = (storage.from_path(), Journal::load(state)) else { return Vec::new() };
    journal
        .files
        .iter()
        .filter(|e| !e.alone)
        .filter_map(|e| {
            let old = join(&notes, &e.from);
            let moved = stamp(&old).is_some_and(|now| now != (e.size, e.time));
            moved.then(|| sha_of(&old).ok()).flatten().filter(|sha| *sha != e.sha).map(|sha| (e.from.clone(), sha))
        })
        .collect()
}

/// Whether a move was begun on this device and not finished (its journal
/// open: a failure that left copies, Sioul stopped during it): the Porch
/// offers the window of the move again.
pub fn begun(state: &Path) -> bool {
    !Storage::read(state).is_moved() && Journal::load(state).ok().flatten().is_some_and(|journal| journal.switched == 0)
}

/// Where a paper or a scan the move copied is now, for a link that still
/// names its old place in the notes folder's `papers/` or `letters/` (a
/// task's or an event's `file://` link, never rewritten: an event may be a
/// meeting, and they live on their servers): read from the move's own
/// journal when the link is opened. None: not one of them, or its copy is
/// no longer there.
pub fn moved_to(state: &Path, old: &Path) -> Option<PathBuf> {
    let notes = Storage::read(state).from_path()?;
    let journal = Journal::load(state).ok().flatten()?;
    let relative = old.strip_prefix(&notes).ok().map(Path::to_path_buf).or_else(|| resolved(old).strip_prefix(resolved(&notes)).ok().map(Path::to_path_buf))?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    let entry = journal.files.iter().find(|e| e.from == relative && matches!(e.kind, Some(Kind::Paper | Kind::Letter)))?;
    let to = PathBuf::from(&entry.to);
    std::fs::symlink_metadata(&to).is_ok_and(|meta| meta.is_file()).then_some(to)
}

/// How many old files of Sioul's this device's notes folder still holds, by
/// the journal of its move: what the line of Settings ▸ Sharing offers to
/// take out (`clean`, on the person's press, never by itself).
pub fn old_files(state: &Path) -> usize {
    let storage = Storage::read(state);
    let (Some(notes), Ok(Some(journal))) = (storage.from_path(), Journal::load(state)) else { return 0 };
    journal.files.iter().filter(|e| !e.alone).filter(|e| std::fs::symlink_metadata(join(&notes, &e.from)).is_ok()).count()
}

/// A folder and those below it, each taken out once empty; never a file,
/// never through a link (a link put at `papers/` leads to the papers' folder).
fn remove_empty(dir: &Path) {
    if !std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            remove_empty(&entry.path());
        }
    }
    let _ = std::fs::remove_dir(dir);
}

/// The dated copies taken out of `kept` `KEEP_DAYS` after the old files
/// left the notes folder (`cleaned`, the take-out that left nothing): never
/// while they are still there (0), never counted from the move itself
/// (the take-out may come months later). Only folders named as a day,
/// never through a link.
pub fn sweep_kept(kept: &Path, cleaned: i64, now: i64) {
    if cleaned <= 0 || now - cleaned <= KEEP_DAYS * 86_400 {
        return;
    }
    let Ok(read) = std::fs::read_dir(kept) else { return };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.parse::<jiff::civil::Date>().is_ok() && entry.file_type().is_ok_and(|t| t.is_dir()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
