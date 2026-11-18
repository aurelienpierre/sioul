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
use sioul_core::csv::Table;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// What was read of the file shown, and the import planned from it.
struct Shown {
    name: String,
    file: Kind,
    planned: Option<(Imported, Target)>,
}

#[derive(Clone)]
enum Kind {
    Vcf(Cards),
    Csv(Table),
}

/// Where the cards go: a book there, or one made in the same step.
#[derive(Clone)]
enum Target {
    Book,
    New { account: String, name: String },
}

static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);

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
/// shown, unless one is shown already; its file taken out once read.
pub(crate) fn take(sioul: Pin<&mut Sioul>) {
    use cxx_qt::Threading;
    if SHOWN.lock().map_or(true, |shown| shown.is_some()) {
        return;
    }
    let mut waiting: Vec<PathBuf> = std::fs::read_dir(folder()).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| is_contacts_file(p)).collect();
    waiting.sort();
    let Some(next) = waiting.into_iter().next() else { return };
    read(&sioul.qt_thread(), &sioul.shared(), &next.display().to_string(), true);
}

/// The step shown, as the page reads it.
fn shown(qt: &QtThread, view: serde_json::Value) {
    let text = view.to_string();
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_contacts_import(QString::from(&text)));
}

/// A file read off the window's thread (`handed`: a file waiting, taken out
/// once read): what it holds shown, the first step.
pub(crate) fn read(qt: &QtThread, shared: &Arc<Shared>, path: &str, handed: bool) {
    // A phone's picker gives a content: address, which only Android reads: its own
    // way in is "Open with Sioul" from where the file is (ShareActivity.java).
    if path.starts_with("content:") {
        return shown(qt, serde_json::json!({ "step": "problem", "line": tr().text("import-on-a-phone", None) }));
    }
    let (qt, _shared, path) = (qt.clone(), Arc::clone(shared), crate::backend::local_path(path));
    shown(&qt, serde_json::json!({ "step": "reading", "line": tr().text("import-reading", None) }));
    std::thread::spawn(move || {
        let bytes = read_at_most(&path);
        if handed {
            let _ = std::fs::remove_file(&path);
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let bytes = match bytes {
            Ok(bytes) if bytes.len() as u64 > MAX_BYTES => return shown(&qt, serde_json::json!({ "step": "problem", "line": say("import-too-big", &[("mb", (MAX_BYTES / 1024 / 1024).to_string())]) })),
            Ok(bytes) => bytes,
            Err(e) => return shown(&qt, serde_json::json!({ "step": "problem", "line": say("import-unreadable", &[("error", e.to_string())]) })),
        };
        let text = sioul_core::csv::decode(&bytes);
        let vcard = text.trim_start().get(..11).is_some_and(|s| s.eq_ignore_ascii_case("BEGIN:VCARD"));
        let csv = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"));
        let file = if vcard {
            Kind::Vcf(contactimport::read_vcf(&text))
        } else if csv {
            Kind::Csv(sioul_core::csv::read(&text))
        } else {
            return shown(&qt, serde_json::json!({ "step": "problem", "line": tr().text("import-kind-not", None) }));
        };
        let view = first_view(&name, &file);
        if let Ok(mut kept) = SHOWN.lock() {
            *kept = Some(Shown { name, file, planned: None });
        }
        shown(&qt, view);
    });
}

/// What the file holds: for vCard, its count; for CSV, each column with its
/// heading, its field (kept, else guessed) and its first three values.
fn first_view(name: &str, file: &Kind) -> serde_json::Value {
    let config = load_config();
    let books: Vec<serde_json::Value> = sioul_core::vdir::collections(sioul_core::vdir::Kind::Contacts).into_iter().filter(|c| !c.read_only).map(|c| serde_json::json!({ "place": format!("{}/{}", c.account, c.id), "name": c.label(&config, tr()) })).collect();
    let homes = sioul_core::addressbooks::homes(&config, tr());
    let fields: Vec<serde_json::Value> = Field::ALL.iter().map(|f| serde_json::json!({ "field": f.word(), "label": tr().text(&format!("import-field-{}", f.word()), None) })).collect();
    match file {
        Kind::Vcf(cards) => serde_json::json!({
            "step": "read", "kind": "vcf", "name": name, "books": books, "homes": homes, "fields": fields,
            "line": say("import-found-vcf", &[("n", cards.cards.len().to_string())]),
            "broken": broken_line(&cards.broken), "columns": [], "kept": "", "export": "",
        }),
        Kind::Csv(table) => {
            let kept = Mappings::load(&Mappings::default_path()).found(&table.headings).cloned();
            let guessed = kept.as_ref().map(|k| k.fields.clone()).unwrap_or_else(|| contactimport::guess(&table.headings));
            let columns: Vec<serde_json::Value> = table
                .headings
                .iter()
                .enumerate()
                .map(|(i, heading)| {
                    let samples: Vec<String> = table.rows.iter().map(|r| r.cell(i).replace('\n', " ")).filter(|c| !c.is_empty()).take(3).collect();
                    serde_json::json!({ "heading": heading, "field": guessed.get(i).copied().unwrap_or_default().word(), "samples": samples.join(" · ") })
                })
                .collect();
            serde_json::json!({
                "step": "read", "kind": "csv", "name": name, "books": books, "homes": homes, "fields": fields,
                "line": say("import-found-csv", &[("n", table.rows.len().to_string())]),
                "broken": broken_line(&table.broken), "columns": columns,
                "kept": kept.map(|k| say("import-mapping-kept", &[("name", k.name)])).unwrap_or_default(),
                "export": contactimport::export_of(&table.headings).map(|e| say("import-export-of", &[("name", e)])).unwrap_or_default(),
            })
        }
    }
}

/// "Lines 18, 77 and 204 do not read.", or "".
fn broken_line(lines: &[usize]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    say("import-broken", &[("n", lines.len().to_string()), ("lines", lines.iter().map(usize::to_string).collect::<Vec<_>>().join(", "))])
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
    /// A title at the start of a name moved into its field.
    #[serde(default)]
    titles: bool,
    /// The columns kept under this name ("" not kept).
    #[serde(default)]
    keep_as: String,
}

/// The import planned off the window's thread (`text`, JSON {fields, book,
/// new_account, new_name, groups, titles, keep_as}): the count said before
/// anything is written, the second step.
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
            match sioul_core::addressbooks::find(&asked.book).filter(|b| !b.read_only) {
                Some(book) => (Target::Book, book),
                None => return shown(&qt, serde_json::json!({ "step": "problem-plan", "line": tr().text("dav-no-book", None) })),
            }
        };
        let cards = match &file {
            Kind::Vcf(cards) => cards.clone(),
            Kind::Csv(table) => {
                if !asked.keep_as.trim().is_empty() {
                    let _ = Mappings::keep(&Mappings::default_path(), &asked.keep_as, &table.headings, &asked.fields);
                }
                contactimport::read_csv(table, &asked.fields)
            }
        };
        let groups: Vec<String> = asked.groups.split(',').map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect();
        let (removed, _) = crate::mail::hidden_pim(&shared);
        let existing: Vec<sioul_core::contacts::Contact> = sioul_core::contacts::all().into_iter().filter(|c| !removed.contains(Path::new(&c.key))).collect();
        let imported = contactimport::plan(&cards, &existing, &collection, &groups, asked.titles, crate::duplicates::region());
        let mut summary = say("import-summary", &[("new", imported.new.to_string()), ("completed", imported.completed.to_string())]);
        if !imported.broken.is_empty() {
            summary = format!("{summary} {}", broken_line(&imported.broken));
        }
        let more: Vec<String> = [
            (imported.same > 0).then(|| say("import-same", &[("n", imported.same.to_string())])),
            (imported.read_only > 0).then(|| say("import-read-only", &[("n", imported.read_only.to_string())])),
            (imported.titled > 0).then(|| say("import-titled", &[("n", imported.titled.to_string())])),
        ]
        .into_iter()
        .flatten()
        .collect();
        let count = imported.plan.steps.len();
        let view = serde_json::json!({ "step": "planned", "name": name, "line": summary, "more": more, "count": count });
        // Kept for its run, while that file is still the one shown.
        if let Ok(mut kept) = SHOWN.lock()
            && let Some(shown_now) = kept.as_mut().filter(|f| f.name == name)
        {
            shown_now.planned = Some((imported, target));
        }
        shown(&qt, view);
    });
}

/// The import planned, run as one batch: a new book made first. "" once started, else why not.
pub(crate) fn run(qt: &QtThread, shared: &Arc<Shared>) -> String {
    let planned = SHOWN.lock().ok().and_then(|mut kept| kept.as_mut().and_then(|f| f.planned.take()));
    let Some((imported, target)) = planned else { return tr().text("import-gone", None) };
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
