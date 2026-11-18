// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contacts imported from a file (wave D, 18 November 2026;
//! `ContactImport.qml`): a vCard or a CSV file chosen with "Import…" (Contacts
//! ⚙ ▸ Address books, and the Contacts page), opened with Sioul on a computer
//! (`sioul-app <file>`, handed to the Sioul already open as a calendar file
//! is: `outside::hand_over`), or opened in Sioul on a phone (Android's VIEW,
//! ShareActivity.java). A handed file waits in `<data>/sioul/handed/contacts/`
//! until the window takes it. Every file is read off the window's thread,
//! 32 MB at most (`contactimport::MAX_BYTES`: cards may carry photos), on
//! this device alone. Three steps, their state the `contactsImport`
//! property: what the file holds (for a CSV file, which column is what,
//! guessed, kept under a name); where the cards go and what about those
//! already here, said as a count before anything is written; then one batch
//! (`contactbatch::start`), undone as a whole.

use crate::backend::{QtThread, Shared, load_config, say, tr};
use crate::backend::qobject::Sioul;
use cxx_qt_lib::QString;
use serde::Deserialize;
use sioul_core::contactimport::{self, Cards, Field, Imported, MAX_BYTES, Mappings};
use sioul_core::csv::{Encoding, Table};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// What was read of the file shown, and the import planned from it.
struct Shown {
    name: String,
    file: Kind,
    planned: Option<Planned>,
}

/// An import counted, kept for its run: the columns are kept under their
/// name only then, never at the count (review F23).
struct Planned {
    imported: Imported,
    target: Target,
    keep: Option<(String, Vec<String>, Vec<Field>)>,
}

#[derive(Clone)]
enum Kind {
    Vcf(Cards, Encoding),
    /// The table, its encoding, and the file's text: read again with another separator when asked.
    Csv(Table, Encoding, Arc<String>),
}

/// Where the cards go: a book there, or one made in the same step.
#[derive(Clone)]
enum Target {
    Book,
    New { account: String, name: String },
}

static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);

/// A file being read: the next one waiting is taken only once it is shown.
static READING: AtomicBool = AtomicBool::new(false);

/// Where files handed wait for the window, one each: `<data>/sioul/handed/contacts/`.
pub(crate) fn folder() -> PathBuf {
    sioul_core::config::data_dir().join("handed").join("contacts")
}

/// Whether a file is one this import reads, by its name: `.vcf`, `.vcard`, `.csv`.
pub(crate) fn is_contacts_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "vcf" | "vcard" | "csv"))
}

/// A file's bytes, `MAX_BYTES` and one at most (what is more is too large).
fn read_at_most(path: &Path) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("not a file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Files handed (`sioul-app <file>`, another Sioul's): each kept for the
/// window under a name of its own, its kind kept in its extension.
pub(crate) fn keep_all(paths: &[PathBuf]) {
    for path in paths {
        let kept = read_at_most(path).and_then(|bytes| {
            let dir = folder();
            std::fs::create_dir_all(&dir)?;
            let extension = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv")) { "csv" } else { "vcf" };
            sioul_core::filelock::replace(&dir.join(format!("{}.{extension}", sioul_core::vdir::new_name())), &bytes)
        });
        if let Err(e) = kept {
            eprintln!("Sioul: {}: {e}", path.display());
        }
    }
}

/// On the window's thread (`outside::take`): the next file waiting read and
/// shown, unless one is shown or being read already. The file is taken out
/// of the waiting ones before its thread starts, so that two hand-overs at
/// once never read one file twice nor lose one (review F23): renamed
/// `<name>.<kind>.taken`, removed once read.
pub(crate) fn take(sioul: Pin<&mut Sioul>) {
    use cxx_qt::Threading;
    if SHOWN.lock().map_or(true, |shown| shown.is_some()) || READING.load(Ordering::Acquire) {
        return;
    }
    let mut waiting: Vec<PathBuf> = std::fs::read_dir(folder()).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| is_contacts_file(p)).collect();
    waiting.sort();
    let Some(next) = waiting.into_iter().next() else { return };
    let taken = next.with_file_name(format!("{}.taken", next.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    if std::fs::rename(&next, &taken).is_err() {
        return;
    }
    read(&sioul.qt_thread(), &sioul.shared(), &taken.display().to_string(), true);
}

/// The step shown, as the page reads it.
fn shown(qt: &QtThread, view: serde_json::Value) {
    let text = view.to_string();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_contacts_import(QString::from(&text)));
}

/// The reading ended (shown, or said why not): the next file waiting may come.
fn read_ended(qt: &QtThread, view: serde_json::Value) {
    READING.store(false, Ordering::Release);
    shown(qt, view);
}

/// A file read off the window's thread (`handed`: a file waiting, taken out
/// once read): what it holds shown, the first step.
pub(crate) fn read(qt: &QtThread, shared: &Arc<Shared>, path: &str, handed: bool) {
    // A phone's picker gives a content: address, which only Android reads: its own
    // way in is "Open with Sioul" from where the file is (ShareActivity.java).
    if path.starts_with("content:") {
        return shown(qt, serde_json::json!({ "step": "problem", "line": tr().text("import-on-a-phone", None) }));
    }
    READING.store(true, Ordering::Release);
    let (qt, _shared, path) = (qt.clone(), Arc::clone(shared), crate::backend::local_path(path));
    shown(&qt, serde_json::json!({ "step": "reading", "line": tr().text("import-reading", None) }));
    std::thread::spawn(move || {
        let bytes = read_at_most(&path);
        // A handed file's name, as it was handed: "people.csv.taken" → "people.csv".
        let named = if handed { path.with_extension("") } else { path.clone() };
        if handed {
            let _ = std::fs::remove_file(&path);
        }
        let name = named.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let bytes = match bytes {
            Ok(bytes) if bytes.len() as u64 > MAX_BYTES => return read_ended(&qt, serde_json::json!({ "step": "problem", "line": say("import-too-big", &[("mb", (MAX_BYTES / 1024 / 1024).to_string())]) })),
            Ok(bytes) => bytes,
            Err(e) => return read_ended(&qt, serde_json::json!({ "step": "problem", "line": say("import-unreadable", &[("error", e.to_string())]) })),
        };
        let (text, encoding) = sioul_core::csv::decode_with(&bytes);
        drop(bytes);
        let vcard = text.trim_start().get(..11).is_some_and(|s| s.eq_ignore_ascii_case("BEGIN:VCARD"));
        let csv = named.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"));
        let file = if vcard {
            Kind::Vcf(contactimport::read_vcf(&text), encoding)
        } else if csv {
            Kind::Csv(sioul_core::csv::read(&text), encoding, Arc::new(text))
        } else {
            return read_ended(&qt, serde_json::json!({ "step": "problem", "line": tr().text("import-kind-not", None) }));
        };
        let view = first_view(&name, &file);
        if let Ok(mut kept) = SHOWN.lock() {
            *kept = Some(Shown { name, file, planned: None });
        }
        read_ended(&qt, view);
    });
}

/// A CSV file shown read again with another separator (";", ",", "\t"), off
/// the window's thread: its columns offered again.
pub(crate) fn separate(qt: &QtThread, separator: &str) {
    let Some(separator) = separator.chars().next().filter(|c| matches!(c, ';' | ',' | '\t')) else { return };
    let qt = qt.clone();
    std::thread::spawn(move || {
        let Some((name, encoding, text)) = SHOWN.lock().ok().and_then(|kept| match kept.as_ref() {
            Some(Shown { name, file: Kind::Csv(_, encoding, text), .. }) => Some((name.clone(), *encoding, Arc::clone(text))),
            _ => None,
        }) else {
            return;
        };
        let file = Kind::Csv(sioul_core::csv::read_with(&text, separator), encoding, text);
        let view = first_view(&name, &file);
        if let Ok(mut kept) = SHOWN.lock()
            && let Some(shown_now) = kept.as_mut().filter(|f| f.name == name)
        {
            shown_now.file = file;
            shown_now.planned = None;
        }
        shown(&qt, view);
    });
}

/// At most this many line numbers are written out in one sentence.
const MOST_LINES_SAID: usize = 12;

/// "18, 77 and 204", or the first twelve and how many more.
fn lines_said(lines: &[usize]) -> String {
    let named: Vec<String> = lines.iter().take(MOST_LINES_SAID).map(usize::to_string).collect();
    if lines.len() > MOST_LINES_SAID {
        say("import-lines-more", &[("lines", named.join(", ")), ("more", (lines.len() - MOST_LINES_SAID).to_string())])
    } else {
        named.join(", ")
    }
}

/// What the file is, how it was read and what of it was not, each in one
/// calm sentence (review F7, F22): the encoding, the limits reached, the
/// quote never closed, the rows with more cells than headings.
fn reading_lines(file: &Kind) -> Vec<String> {
    let mut lines = Vec::new();
    let encoding = match file {
        Kind::Vcf(_, e) | Kind::Csv(_, e, _) => *e,
    };
    lines.push(say("import-encoding", &[("encoding", encoding.word().to_string())]));
    match file {
        Kind::Vcf(cards, _) => {
            if !cards.too_big.is_empty() {
                lines.push(say("import-limit-card", &[("n", cards.too_big.len().to_string()), ("mb", (contactimport::MOST_CARD / 1024 / 1024).to_string()), ("lines", lines_said(&cards.too_big))]));
            }
            if let Some(line) = cards.cut {
                lines.push(say("import-limit-cards", &[("most", contactimport::MOST_CARDS.to_string()), ("line", line.to_string())]));
            }
        }
        Kind::Csv(table, _, _) => {
            if let Some(n) = table.wide {
                lines.push(say("import-limit-columns", &[("n", n.to_string()), ("most", sioul_core::csv::MOST_COLUMNS.to_string())]));
            }
            if let Some(line) = table.cut {
                lines.push(say("import-limit-rows", &[("most", sioul_core::csv::MOST_ROWS.to_string()), ("line", line.to_string())]));
            }
            if !table.broken.is_empty() {
                lines.push(say("import-limit-cell", &[("n", table.broken.len().to_string()), ("kb", (sioul_core::csv::MOST_CELL / 1024).to_string()), ("lines", lines_said(&table.broken))]));
            }
            if let Some((line, lost)) = table.unclosed {
                lines.push(say("import-unclosed", &[("line", line.to_string()), ("n", lost.to_string())]));
            }
            if !table.overflow.is_empty() {
                lines.push(say("import-overflow", &[("n", table.overflow.len().to_string()), ("lines", lines_said(&table.overflow))]));
            }
        }
    }
    lines
}

/// What the file holds: for vCard, its count; for CSV, each column with its
/// heading, its field (kept, else guessed) and its first three values, and
/// the separator it was read with.
fn first_view(name: &str, file: &Kind) -> serde_json::Value {
    let config = load_config();
    let books: Vec<serde_json::Value> = sioul_core::vdir::collections(sioul_core::vdir::Kind::Contacts)
        .into_iter()
        .filter(|c| !c.read_only && !sioul_core::vdir::State::load(&c.state_path()).refused)
        .map(|c| serde_json::json!({ "place": format!("{}/{}", c.account, c.id), "name": c.label(&config, tr()) }))
        .collect();
    let homes = sioul_core::addressbooks::homes(&config, tr());
    let fields: Vec<serde_json::Value> = Field::ALL.iter().map(|f| serde_json::json!({ "field": f.word(), "label": tr().text(&format!("import-field-{}", f.word()), None) })).collect();
    let region = crate::duplicates::region();
    let international = match region {
        Some(r) => say("import-international", &[("country", tr().text(&format!("country-{}", r.code.to_ascii_lowercase()), None))]),
        None => tr().text("import-international-none", None),
    };
    let reading = reading_lines(file);
    match file {
        Kind::Vcf(cards, _) => serde_json::json!({
            "step": "read", "kind": "vcf", "name": name, "books": books, "homes": homes, "fields": fields,
            "line": say("import-found-vcf", &[("n", cards.cards.len().to_string())]),
            "broken": broken_line(&cards.broken), "reading": reading, "columns": [], "kept": "", "export": "",
            "separator": "", "international": international, "region": region.is_some(),
        }),
        Kind::Csv(table, _, _) => {
            let kept = Mappings::load(&Mappings::default_path()).found(&table.headings).cloned();
            let guessed = kept.as_ref().map(|k| k.fields.clone()).unwrap_or_else(|| contactimport::guess(&table.headings));
            let columns: Vec<serde_json::Value> = table
                .headings
                .iter()
                .enumerate()
                .map(|(i, heading)| {
                    let samples: Vec<String> = table.rows.iter().map(|r| r.cell(i).replace('\n', " ").chars().take(80).collect::<String>()).filter(|c| !c.is_empty()).take(3).collect();
                    serde_json::json!({ "heading": heading.chars().take(120).collect::<String>(), "field": guessed.get(i).copied().unwrap_or_default().word(), "samples": samples.join(" · ") })
                })
                .collect();
            serde_json::json!({
                "step": "read", "kind": "csv", "name": name, "books": books, "homes": homes, "fields": fields,
                "line": say("import-found-csv", &[("n", table.rows.len().to_string())]),
                "broken": "", "reading": reading, "columns": columns,
                "kept": kept.map(|k| say("import-mapping-kept", &[("name", k.name)])).unwrap_or_default(),
                "export": contactimport::export_of(&table.headings).map(|e| say("import-export-of", &[("name", e)])).unwrap_or_default(),
                "separator": table.separator.to_string(), "international": international, "region": region.is_some(),
            })
        }
    }
}

/// "Lines 18, 77 and 204 do not read.", or "".
fn broken_line(lines: &[usize]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    say("import-broken", &[("n", lines.len().to_string()), ("lines", lines_said(lines))])
}

#[derive(Debug, Default, Deserialize)]
struct Asked {
    /// For a CSV file: each column's field, by its word.
    #[serde(default)]
    fields: Vec<Field>,
    /// The book's place; empty with `new_account`: a new book.
    #[serde(default)]
    book: String,
    #[serde(default)]
    new_account: String,
    #[serde(default)]
    new_name: String,
    /// Groups added to every card imported, separated by commas.
    #[serde(default)]
    groups: String,
    /// Those groups given to the cards already here too (off by default).
    #[serde(default)]
    groups_here: bool,
    /// The cards already here completed with what the file adds (off by default).
    #[serde(default)]
    complete: bool,
    /// A title at the start of a name moved into its field, where it cannot be anything else.
    #[serde(default)]
    titles: bool,
    /// Numbers written in their international form, by the country of Contacts ⚙.
    #[serde(default)]
    international: bool,
    /// The columns kept under this name ("" not kept), at the run.
    #[serde(default)]
    keep_as: String,
}

/// The word of what a card already here would gain: "e-mail address jeanne@example.org".
fn gain(kind: &str, value: &str) -> String {
    say(&format!("import-gain-{kind}"), &[("value", value.chars().take(120).collect())])
}

/// The cards found again, one line each with what the file would add, the
/// first hundred (the count says the rest).
const MOST_MATCHES_SHOWN: usize = 100;

/// The import planned off the window's thread (`text`, JSON {fields, book,
/// new_account, new_name, groups, groups_here, complete, titles,
/// international, keep_as}): the count said before anything is written,
/// with each card already here and what the file would add to it.
pub(crate) fn plan(qt: &QtThread, shared: &Arc<Shared>, text: &str) {
    let asked: Asked = serde_json::from_str(text).unwrap_or_default();
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        // What was read, taken out of the lock: the window's thread never waits for a plan.
        let Some((name, file)) = SHOWN.lock().ok().and_then(|kept| kept.as_ref().map(|f| (f.name.clone(), f.file.clone()))) else {
            return shown(&qt, serde_json::json!({ "step": "problem", "line": tr().text("import-gone", None) }));
        };
        let config = load_config();
        let (target, collection) = if asked.book.is_empty() {
            if asked.new_name.trim().is_empty() {
                return shown(&qt, serde_json::json!({ "step": "problem-plan", "line": tr().text("book-needs-name", None) }));
            }
            if asked.new_account != sioul_core::vdir::LOCAL && !sioul_core::addressbooks::homes(&config, tr()).iter().any(|h| h.id == asked.new_account) {
                return shown(&qt, serde_json::json!({ "step": "problem-plan", "line": tr().text("book-no-home", None) }));
            }
            // Planned into a stand-in of the book to make: its cards' paths are made again at the run.
            let stand_in = sioul_core::vdir::Collection { kind: sioul_core::vdir::Kind::Contacts, account: asked.new_account.clone(), id: String::new(), dir: PathBuf::new(), name: asked.new_name.trim().to_string(), color: None, read_only: false, components: Vec::new() };
            (Target::New { account: asked.new_account.clone(), name: asked.new_name.trim().to_string() }, stand_in)
        } else {
            match sioul_core::addressbooks::find(&asked.book).filter(|b| !b.read_only && !sioul_core::vdir::State::load(&b.state_path()).refused) {
                Some(book) => (Target::Book, book),
                None => return shown(&qt, serde_json::json!({ "step": "problem-plan", "line": tr().text("dav-no-book", None) })),
            }
        };
        let (cards, rows, keep) = match &file {
            Kind::Vcf(cards, _) => (cards.clone(), None, None),
            Kind::Csv(table, _, _) => {
                let keep = Some(asked.keep_as.trim().to_string()).filter(|n| !n.is_empty()).map(|n| (n, table.headings.clone(), asked.fields.clone()));
                (contactimport::read_csv(table, &asked.fields, tr()), Some(table.rows.len()), keep)
            }
        };
        let groups: Vec<String> = asked.groups.split(',').map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect();
        let (removed, _) = crate::mail::hidden_pim(&shared);
        let existing: Vec<sioul_core::contacts::Contact> = sioul_core::contacts::all().into_iter().filter(|c| !removed.contains(Path::new(&c.key))).collect();
        let options = contactimport::Options { groups, groups_here: asked.groups_here, complete: asked.complete, titles: asked.titles, international: asked.international };
        let imported = contactimport::plan(&cards, &existing, &collection, &options, crate::duplicates::region());
        let n = |count: usize| count.to_string();
        let summary = say("import-summary", &[("new", n(imported.new)), ("book", collection.name.clone())]);
        let more: Vec<String> = [
            rows.filter(|&rows| rows > imported.read).map(|rows| say("import-read-of", &[("read", n(imported.read)), ("rows", n(rows))])),
            Some(broken_line(&imported.broken)).filter(|l| !l.is_empty()),
            (imported.here > 0).then(|| say("import-here", &[("n", n(imported.here))])),
            (imported.twice > 0).then(|| say("import-twice", &[("n", n(imported.twice))])),
            (imported.apart > 0).then(|| say("import-apart", &[("n", n(imported.apart))])),
            (imported.read_only > 0).then(|| say("import-read-only", &[("n", n(imported.read_only))])),
            (imported.completed > 0).then(|| say("import-completed", &[("n", n(imported.completed))])),
            (imported.titled > 0 && !asked.titles).then(|| say("import-titled", &[("n", n(imported.titled))])),
            (imported.rewritten > 0).then(|| say("import-rewritten", &[("n", n(imported.rewritten))])),
            (cards.birthdays_as_written > 0).then(|| say("import-birthdays", &[("n", n(cards.birthdays_as_written))])),
        ]
        .into_iter()
        .flatten()
        .collect();
        // Those already here, card by card, with what the file would add: read before "Complete them" is ticked.
        let gaining: Vec<&contactimport::Match> = imported.matches.iter().filter(|m| !m.adds.is_empty()).collect();
        let matches: Vec<serde_json::Value> = gaining
            .iter()
            .take(MOST_MATCHES_SHOWN)
            .map(|m| {
                let adds: Vec<String> = m.adds.iter().map(|(kind, value)| gain(kind, value)).collect();
                serde_json::json!({ "name": m.name, "adds": adds.join(" · "), "in_file": m.in_file })
            })
            .collect();
        let matches_more = gaining.len().saturating_sub(MOST_MATCHES_SHOWN);
        let count = imported.plan.steps.len();
        let view = serde_json::json!({
            "step": "planned", "name": name, "line": summary, "more": more, "count": count,
            "matches": matches, "matches_more": if matches_more > 0 { say("import-gains-more", &[("n", n(matches_more))]) } else { String::new() },
            "gaining": gaining.len(), "complete": asked.complete,
        });
        // Kept for its run, while that file is still the one shown.
        if let Ok(mut kept) = SHOWN.lock()
            && let Some(shown_now) = kept.as_mut().filter(|f| f.name == name)
        {
            shown_now.planned = Some(Planned { imported, target, keep });
        }
        shown(&qt, view);
    });
}

/// The import planned, run as one batch: a new book made first, the columns
/// kept under their name. "" once started, else why not.
pub(crate) fn run(qt: &QtThread, shared: &Arc<Shared>) -> String {
    let planned = SHOWN.lock().ok().and_then(|mut kept| kept.as_mut().and_then(|f| f.planned.take()));
    let Some(Planned { imported, target, keep }) = planned else { return tr().text("import-gone", None) };
    let mut steps = imported.plan.steps;
    if let Target::New { account, name } = &target {
        let made = match sioul_core::addressbooks::make(&load_config(), account, name, tr()) {
            Ok(made) => made,
            Err(problem) => return problem,
        };
        // Its cards' places, in the book made now.
        for (_, step) in steps.iter_mut() {
            if let sioul_core::contactbatch::Step::Write { path, before: None, .. } = step {
                *path = sioul_core::contacts::new_path(&made);
            }
        }
        crate::pim::nudge(shared, &made.account);
    }
    if let Some((name, headings, fields)) = keep
        && let Err(problem) = Mappings::keep(&Mappings::default_path(), &name, &headings, &fields)
    {
        eprintln!("Sioul: the columns kept as {name}: {problem}");
    }
    if let Ok(mut kept) = SHOWN.lock() {
        *kept = None;
    }
    shown(qt, serde_json::json!({ "step": "closed" }));
    let started = crate::contactbatch::start(qt, shared, "import", steps, None, Box::new(|_: &sioul_core::contactbatch::Outcome| String::new()));
    let qt = qt.clone();
    let _ = qt.queue(take);
    started
}

/// The file shown set aside, nothing written; the next one waiting shown.
pub(crate) fn close(qt: &QtThread) {
    if let Ok(mut kept) = SHOWN.lock() {
        *kept = None;
    }
    shown(qt, serde_json::json!({ "step": "closed" }));
    let _ = qt.queue(take);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file handed to a running Sioul (`sioul-app contacts.vcf`, or another
    /// Sioul's hand-over) waits for the window under a name of its own, its
    /// kind kept; what is larger than the limit is read one byte past it, and
    /// said too large, never cut into cards.
    #[test]
    fn a_file_handed_waits_for_the_window() {
        sioul_core::config::stand_ins_only();
        let dir = std::env::temp_dir().join(format!("sioul-contacts-handed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let card = dir.join("people.VCF");
        std::fs::write(&card, "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Lou\r\nEND:VCARD\r\n").unwrap();
        let sheet = dir.join("people.csv");
        std::fs::write(&sheet, "Nom;Prénom\nExemple;Lou\n").unwrap();
        assert!(is_contacts_file(&card) && is_contacts_file(&sheet) && !is_contacts_file(&dir.join("visit.ics")));
        let _ = std::fs::remove_dir_all(folder());
        keep_all(&[card, sheet]);
        let mut kept: Vec<String> = std::fs::read_dir(folder()).unwrap().filter_map(Result::ok).map(|e| e.path().extension().unwrap().to_string_lossy().to_string()).collect();
        kept.sort();
        assert_eq!(kept, ["csv", "vcf"]);
        let large = dir.join("large.vcf");
        std::fs::File::create(&large).unwrap().set_len(MAX_BYTES + 10).unwrap();
        assert_eq!(read_at_most(&large).unwrap().len() as u64, MAX_BYTES + 1);
        assert!(read_at_most(&dir).is_err(), "a folder is no file");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(folder());
    }
}
