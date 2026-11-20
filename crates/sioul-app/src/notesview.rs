// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Notes page's lists behind the window (docs/tasks.md, "Notes"): Show
//! (by folder, by when each was last changed, by when each was made) and
//! Type (every type, text, sound, PDF, other), both kept on this device as
//! they were left; and the search, the notes' names answered at once, their
//! texts read off the window's thread after a pause in typing. Each answer
//! carries the number of the question it answers, and fills the list only
//! while that question is the last one asked and no answer made after it is
//! shown (`Gate`): a slower earlier answer never fills a later list.

use crate::backend::{QtThread, Shared};
use cxx_qt_lib::QString;
use serde::Serialize;
use sioul_core::links::Loaded;
use sioul_core::notes::{self, Dated, Found, FoundBy, Kinds, Note, NoteKind, Vault};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// What the left column lists when nothing is searched.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Show {
    /// The tree of folders, folded until opened: the page as it starts.
    #[default]
    Folders,
    /// One list by when each note was last changed, under quiet headings.
    Changed,
    /// One list by when each note was made (`Note::created`).
    Created,
}

impl Show {
    pub(crate) fn named(name: &str) -> Show {
        match name {
            "changed" => Show::Changed,
            "created" => Show::Created,
            _ => Show::Folders,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Show::Folders => "folder",
            Show::Changed => "changed",
            Show::Created => "created",
        }
    }
}

/// One question of the page: the words typed, Show and Type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Asked {
    pub query: String,
    pub show: Show,
    pub kinds: Kinds,
}

impl Asked {
    fn searching(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// Whether the texts have anything to add: words typed, of a type with text notes.
    fn reads_texts(&self) -> bool {
        self.searching() && matches!(self.kinds, Kinds::All | Kinds::Text)
    }
}

/// A note as a row of the list.
#[derive(Debug, Serialize)]
pub(crate) struct NoteRow {
    pub path: String,
    pub title: String,
    /// "text", "image", "pdf", "audio", "document".
    pub kind: NoteKind,
    /// Its folder in the vault, "" at the root.
    pub folder: String,
    pub tags: Vec<String>,
    pub modified: i64,
    /// When it was made, when known.
    pub created: Option<i64>,
    /// Found by a search: by its "name" (title, path, tags) or its "text".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<FoundBy>,
    /// Found by its text: the line that holds the words, plain text.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub line: String,
}

impl NoteRow {
    fn of(n: &Note) -> NoteRow {
        NoteRow { path: n.path.clone(), title: n.title.clone(), kind: n.kind, folder: n.folder().to_string(), tags: n.tags.clone(), modified: n.modified, created: n.created, by: None, line: String::new() }
    }

    fn found(found: Found) -> NoteRow {
        NoteRow { by: Some(found.by), line: found.line, ..NoteRow::of(found.note) }
    }
}

/// A group of a dated list: "today", "week", "month", "year", a year
/// ("2025"), "after", "unknown" (`notes::When::key`), and its notes.
#[derive(Debug, Serialize)]
pub(crate) struct Group {
    pub when: String,
    pub notes: Vec<NoteRow>,
}

/// The page's lists, as JSON for the window (the property `notes`).
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NotesShown {
    /// No notes folder: none is set.
    pub missing: bool,
    /// "folder", "changed", "created".
    pub show: &'static str,
    /// "all", "text", "sound", "pdf", "other".
    #[serde(rename = "type")]
    pub kinds: &'static str,
    /// The words this answer is for ("" when nothing is searched).
    pub query: String,
    /// The number of the question it answers.
    pub question: u64,
    /// Words typed and the notes' texts not looked in yet: the list says it is still looking.
    pub looking: bool,
    /// Something searched, and a PDF or a sound of the type is in the notes:
    /// they are found by their names only, and the list says so.
    pub names_only: bool,
    /// Searching: what was found, by name then by text. By folder: every
    /// note of the type, by title (the tree). Else empty.
    pub notes: Vec<NoteRow>,
    /// By folder: the ten notes of the type changed last, for the top of the page.
    pub recent: Vec<NoteRow>,
    /// By folder: every folder, empty ones too; with a type, only those holding a note of it.
    pub folders: Vec<String>,
    /// Last changed and Created: the dated list's groups, in order.
    pub groups: Vec<Group>,
}

/// What the page lists for `asked`: what pure work makes, from the notes as
/// read; `texts` reads a text note's text, when the search may look in it.
pub(crate) fn listing(vault: Option<&Vault>, asked: &Asked, texts: Option<&dyn Fn(&Note) -> Option<String>>, now: &jiff::Zoned) -> NotesShown {
    let mut shown = NotesShown { show: asked.show.name(), kinds: asked.kinds.name(), query: asked.query.clone(), ..NotesShown::default() };
    let Some(vault) = vault else {
        shown.missing = true;
        return shown;
    };
    if asked.searching() {
        shown.notes = vault.find(&asked.query, asked.kinds, texts).into_iter().map(NoteRow::found).collect();
        shown.names_only = vault.names_only(asked.kinds);
        return shown;
    }
    let kept: Vec<&Note> = vault.notes.iter().filter(|n| asked.kinds.keeps(n.kind)).collect();
    match asked.show {
        Show::Folders => {
            let mut recent = kept.clone();
            recent.sort_by_key(|n| std::cmp::Reverse(n.modified));
            shown.recent = recent.into_iter().take(10).map(NoteRow::of).collect();
            let mut by_title = kept.clone();
            by_title.sort_by_cached_key(|n| (n.title.to_lowercase(), n.path.to_lowercase()));
            shown.notes = by_title.into_iter().map(NoteRow::of).collect();
            shown.folders = if asked.kinds == Kinds::All {
                vault.folders.clone()
            } else {
                // A folder left with nothing of the type in it is not shown.
                let mut holding: Vec<String> = Vec::new();
                for note in &kept {
                    let mut folder = note.folder();
                    while !folder.is_empty() && !holding.iter().any(|f| f == folder) {
                        holding.push(folder.to_string());
                        folder = folder.rsplit_once('/').map_or("", |(parent, _)| parent);
                    }
                }
                holding.sort_by_key(|f| f.to_lowercase());
                holding
            };
        }
        Show::Changed | Show::Created => {
            let by = if asked.show == Show::Changed { Dated::Changed } else { Dated::Created };
            shown.groups = notes::dated(kept, by, now).into_iter().map(|(when, notes)| Group { when: when.key(), notes: notes.into_iter().map(NoteRow::of).collect() }).collect();
        }
    }
    shown
}

/// The page's questions and the answers that may fill its list.
pub(crate) struct Gate {
    /// The last question asked.
    question: AtomicU64,
    /// The last answer begun: each answer's ticket is later than every one before it.
    ticket: AtomicU64,
    /// The latest ticket shown.
    shown: AtomicU64,
}

impl Gate {
    pub(crate) const fn new() -> Gate {
        Gate { question: AtomicU64::new(0), ticket: AtomicU64::new(0), shown: AtomicU64::new(0) }
    }

    /// A new question (words typed, Show or Type changed): its number.
    pub(crate) fn ask(&self) -> u64 {
        self.question.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// The last question asked.
    pub(crate) fn current(&self) -> u64 {
        self.question.load(Ordering::SeqCst)
    }

    /// An answer begins: its ticket.
    pub(crate) fn ticket(&self) -> u64 {
        self.ticket.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Whether the answer to `question` begun under `ticket` fills the list
    /// now: it answers the last question asked, and no answer begun after
    /// it is shown. Admitted, it is the latest shown.
    pub(crate) fn admits(&self, question: u64, ticket: u64) -> bool {
        question == self.current() && self.shown.fetch_max(ticket, Ordering::SeqCst) < ticket
    }
}

/// The window's one page of notes.
static GATE: Gate = Gate::new();

/// What the page asks now, as kept (`WorkState`).
fn asked(shared: &Shared) -> Asked {
    shared.work.lock().map(|state| Asked { query: state.notes_query.clone(), show: Show::named(&state.notes_show), kinds: Kinds::named(&state.notes_type) }).unwrap_or_default()
}

/// The answer to `question`, to the window, through the gate.
fn publish(qt: &QtThread, question: u64, ticket: u64, mut shown: NotesShown, looking: bool) {
    shown.question = question;
    shown.looking = looking;
    let json = crate::backend::json(&shown);
    let _ = qt.queue(move |mut sioul| {
        if GATE.admits(question, ticket) {
            sioul.as_mut().set_notes(QString::from(&json));
        }
    });
}

/// The question asked now answered by the notes' names, at once; the texts
/// to follow when words are typed (`search_texts`).
fn answer_names(qt: &QtThread, shared: &Arc<Shared>, question: u64) -> Asked {
    let asked = asked(shared);
    let ticket = GATE.ticket();
    let loaded = crate::work::loaded(shared);
    let shown = listing(loaded.vault.as_ref(), &asked, None, &sioul_core::zone::now());
    publish(qt, question, ticket, shown, asked.reads_texts());
    asked
}

/// The question asked now answered with the notes' texts, on a thread of
/// its own; it stops reading as soon as another question is asked. Texts
/// are read again at each search, never kept: the notes are read whole at
/// each load already, and a phone's memory would hold them twice
/// (docs/tasks.md, "Notes").
fn answer_texts(qt: &QtThread, shared: &Arc<Shared>, question: u64) {
    let asked = asked(shared);
    if !asked.reads_texts() {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let ticket = GATE.ticket();
        let loaded = crate::work::loaded(&shared);
        let root: PathBuf = loaded.vault.as_ref().map(|v| v.root.clone()).unwrap_or_default();
        let read = |n: &Note| (GATE.current() == question).then(|| notes::search_text(&root.join(&n.path))).flatten();
        let shown = listing(loaded.vault.as_ref(), &asked, Some(&read), &sioul_core::zone::now());
        if GATE.current() == question {
            publish(&qt, question, ticket, shown, false);
        }
    });
}

/// Words typed in the search: a new question, answered by the names at once.
pub(crate) fn search(qt: &QtThread, shared: &Arc<Shared>, query: &str) {
    if let Ok(mut state) = shared.work.lock() {
        state.notes_query = query.to_string();
    }
    answer_names(qt, shared, GATE.ask());
}

/// A pause in typing: the texts of the question asked last.
pub(crate) fn search_texts(qt: &QtThread, shared: &Arc<Shared>) {
    answer_texts(qt, shared, GATE.current());
}

/// Show and Type chosen: kept on this device, never shared, and the list
/// made again; with words typed, the texts looked in at once.
pub(crate) fn set_view(qt: &QtThread, shared: &Arc<Shared>, show: &str, kinds: &str) {
    if let Ok(mut state) = shared.work.lock() {
        state.notes_show = Show::named(show).name().to_string();
        state.notes_type = Kinds::named(kinds).name().to_string();
        state.save();
    }
    let question = GATE.ask();
    answer_names(qt, shared, question);
    answer_texts(qt, shared, question);
}

/// The notes read again (a change here or elsewhere): the list made again
/// for the question asked now, without asking a new one; with words typed,
/// the whole answer, texts with it, comes once made.
pub(crate) fn relist(qt: &QtThread, shared: &Arc<Shared>, loaded: &Loaded) {
    let asked = asked(shared);
    let question = GATE.current();
    if asked.reads_texts() {
        answer_texts(qt, shared, question);
        return;
    }
    let ticket = GATE.ticket();
    publish(qt, question, ticket, listing(loaded.vault.as_ref(), &asked, None, &sioul_core::zone::now()), false);
}

/// A heading of the outline as the window shows it.
#[derive(Debug, Serialize)]
pub(crate) struct OutlineRow {
    pub level: u8,
    pub text: String,
    /// Its line in the file, from 1: where the editor puts its cursor.
    pub line: usize,
    /// Drawn in the note as it is read; the title's heading is not (it is
    /// shown above the text): pressed, the note goes to its top.
    pub drawn: bool,
}

/// A text note's outline as it shows, its links made links first (`Vault::with_links`),
/// so that a heading's words are those drawn; `title_line` is the line of the
/// heading left out of the drawn text, 0 for none.
pub(crate) fn outline_of(vault: &Vault, folder: &str, text: &str, title_line: usize) -> Vec<OutlineRow> {
    notes::outline(&vault.with_links(folder, text)).into_iter().map(|h| OutlineRow { drawn: h.line != title_line, level: h.level, text: h.text, line: h.line }).collect()
}

/// The outline of the text being written in the note at `path`, as JSON.
pub(crate) fn outline_json(shared: &Shared, path: &str, text: &str) -> String {
    let loaded = crate::work::loaded(shared);
    let Some(vault) = loaded.vault.as_ref() else { return "[]".into() };
    let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
    crate::backend::json(&outline_of(vault, folder, text, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> Vault {
        let at = |s: &str| jiff::civil::date(2026, 11, s.parse().unwrap()).at(12, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp().as_second();
        let mut notes = vec![
            Note { path: "admin/Lease.md".into(), title: "Lease".into(), modified: at("10"), created: Some(at("1")), ..Note::default() },
            Note { path: "memos/Lease call.ogg".into(), title: "Lease call".into(), kind: NoteKind::Audio, modified: at("17"), ..Note::default() },
            Note { path: "home/Soup.md".into(), title: "Soup".into(), modified: at("18"), created: Some(at("18")), ..Note::default() },
            Note { path: "home/recipes/Bread.md".into(), title: "Bread".into(), modified: at("2"), ..Note::default() },
        ];
        notes.sort_by(|a, b| a.path.cmp(&b.path));
        Vault { root: PathBuf::from("/vault"), notes, folders: vec!["admin".into(), "empty".into(), "home".into(), "home/recipes".into(), "memos".into()] }
    }

    fn now() -> jiff::Zoned {
        jiff::civil::date(2026, 11, 18).at(15, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap()
    }

    /// The window's answers: the question's number, still looking while
    /// words are typed and the texts not looked in yet, the names-only line;
    /// by folder, a type leaves out the folders with nothing of it; the dated
    /// lists under their groups.
    #[test]
    fn notes_answers_the_window_receives() {
        let vault = vault();
        let mut asked = Asked { query: "lease".into(), ..Asked::default() };
        let names = listing(Some(&vault), &asked, None, &now());
        assert_eq!((names.query.as_str(), names.names_only), ("lease", true));
        assert_eq!(names.notes.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), vec!["memos/Lease call.ogg", "admin/Lease.md"], "newest first");
        assert!(asked.reads_texts(), "words typed: the texts follow");
        let text = |n: &Note| Some(if n.path == "home/Soup.md" { "Ask the landlord about the lease.".to_string() } else { String::new() });
        let whole = listing(Some(&vault), &asked, Some(&text), &now());
        let rows: Vec<(&str, Option<FoundBy>, &str)> = whole.notes.iter().map(|r| (r.path.as_str(), r.by, r.line.as_str())).collect();
        assert_eq!(rows[2], ("home/Soup.md", Some(FoundBy::Text), "Ask the landlord about the lease."));
        let json: serde_json::Value = serde_json::from_str(&crate::backend::json(&NotesShown { question: 7, looking: true, ..whole })).unwrap();
        assert_eq!((json["question"].as_u64(), json["looking"].as_bool(), json["namesOnly"].as_bool(), json["type"].as_str()), (Some(7), Some(true), Some(true), Some("all")));
        assert_eq!(json["notes"][2]["by"], "text");
        assert!(json["notes"][0].get("line").is_none(), "found by its name: no line");
        // A type with no text: nothing to read, nothing to say about the names.
        asked.kinds = Kinds::Sound;
        assert!(!asked.reads_texts());
        asked.kinds = Kinds::Text;
        assert!(!listing(Some(&vault), &asked, None, &now()).names_only);
        // By folder, every folder; with a type, those holding a note of it.
        asked.query.clear();
        asked.kinds = Kinds::All;
        let folders = listing(Some(&vault), &asked, None, &now());
        assert_eq!(folders.folders.len(), 5);
        assert_eq!(folders.recent[0].path, "home/Soup.md");
        asked.kinds = Kinds::Text;
        assert_eq!(listing(Some(&vault), &asked, None, &now()).folders, vec!["admin", "home", "home/recipes"]);
        // Dated: by when made, not known last.
        asked.show = Show::Created;
        let groups = listing(Some(&vault), &asked, None, &now()).groups;
        let shape: Vec<(&str, Vec<&str>)> = groups.iter().map(|g| (g.when.as_str(), g.notes.iter().map(|n| n.path.as_str()).collect())).collect();
        assert_eq!(shape, vec![("today", vec!["home/Soup.md"]), ("month", vec!["admin/Lease.md"]), ("unknown", vec!["home/recipes/Bread.md"])]);
        assert!(listing(None, &asked, None, &now()).missing);
    }

    /// A slower earlier answer never fills a later list; an answer begun
    /// before the one shown, to the same question, neither.
    #[test]
    fn notes_answers_belong_to_their_question() {
        let gate = Gate::new();
        let first = gate.ask();
        let (names, texts) = (gate.ticket(), gate.ticket());
        assert!(gate.admits(first, names), "the names, at once");
        let second = gate.ask();
        let later = gate.ticket();
        assert!(!gate.admits(first, texts), "the first question's texts, come late: dropped");
        assert!(gate.admits(second, later));
        let (begun, after) = (gate.ticket(), gate.ticket());
        assert!(gate.admits(second, after));
        assert!(!gate.admits(second, begun), "begun before the one shown");
    }

    /// The outline as the window shows it: a link's words, the title's heading not drawn.
    #[test]
    fn notes_outline_as_shown() {
        let vault = Vault { notes: vec![Note { path: "Overview.md".into(), title: "Overview".into(), ..Note::default() }], ..vault() };
        let rows = outline_of(&vault, "", "# Plan\n\n## See [[Overview|the overview]]\n\n## Money\n", 1);
        let shape: Vec<(u8, &str, usize, bool)> = rows.iter().map(|h| (h.level, h.text.as_str(), h.line, h.drawn)).collect();
        assert_eq!(shape, vec![(1, "Plan", 1, false), (2, "See the overview", 3, true), (2, "Money", 5, true)]);
    }
}
