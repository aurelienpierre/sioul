// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The issued file, kept beside its record (docs/projects.md, "The issued
//! file"): `invoices/<number>.pdf` next to `invoices/<number>.toml`, the
//! record naming it (`file`) with its SHA-256 (`file_sha256`). Written once,
//! when the invoice is issued, in the same step as its record and its
//! number (`life::issue`: a step that fails undoes the others); it travels
//! with the records, in the sharing's part for drafts and invoices.
//!
//! The record is the issue's commit: the file is written first under a name
//! the sharing leaves alone (`<number>.pdf.new`), and takes its own once the
//! record is written. An issue stopped halfway (a crash, a phone closing the
//! app) never blocks the next number: before numbering, the device that
//! numbers finishes a file whose record came, and sets aside in
//! `invoices/leftovers/` any other file no record names, said once (`tidy`).
//!
//! Printing again gives back the file issued, byte for byte: a copy where
//! your invoices go (`copy_to`), the kept one untouched, so that a PDF
//! reader saving into its copy never changes what was issued. A kept file
//! gone or changed since is made again from the frozen record and kept again
//! when this Sioul makes the same bytes (a changed one first set aside);
//! when it cannot, nothing is handed out, and where the record and the file
//! are is said. A record naming another file than its number's is refused.
//! Format-1 records, from before November 2026, have none: the window
//! prints them as it always did.

use super::pdf::{self, Options};
use super::record::Invoice;
use crate::i18n::Translator;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The folder, among the invoices, where what an issue stopped halfway left is set aside.
pub const LEFTOVERS: &str = "leftovers";

/// The kept file's name: the record's, as a PDF.
pub fn file_name(number: &str) -> String {
    let record = super::file_name(number);
    format!("{}.pdf", record.strip_suffix(".toml").unwrap_or(&record))
}

/// Where `invoice`'s kept file is in `dir`: its number's, and only it.
pub fn path(dir: &Path, invoice: &Invoice) -> PathBuf {
    dir.join(file_name(&invoice.number))
}

/// The file written while its record is not yet: a name the sharing leaves alone.
fn pending(dir: &Path, number: &str) -> PathBuf {
    dir.join(format!("{}.new", file_name(number)))
}

/// The SHA-256 of `bytes`, in hexadecimal.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn say(tr: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = crate::i18n::args();
    for (k, v) in pairs {
        args.set(k.to_string(), v.clone());
    }
    tr.text(id, Some(&args))
}

/// Where an issued invoice's file stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// As issued: its SHA-256 the record's.
    Kept(PathBuf),
    /// There, but not the bytes issued: changed outside Sioul.
    Changed(PathBuf),
    /// Not on this device (not come yet, or taken away).
    Missing(PathBuf),
    /// The record names another file than its number's: refused.
    Misnamed(String),
    /// A record without one: from before November 2026, or a draft.
    None,
}

/// Where `invoice`'s kept file stands in `dir`.
pub fn state(dir: &Path, invoice: &Invoice) -> State {
    if invoice.file.is_empty() || invoice.number.is_empty() {
        return State::None;
    }
    if invoice.file != file_name(&invoice.number) {
        return State::Misnamed(invoice.file.clone());
    }
    let path = path(dir, invoice);
    match std::fs::read(&path) {
        Ok(bytes) if sha256(&bytes).eq_ignore_ascii_case(invoice.file_sha256.trim()) => State::Kept(path),
        Ok(_) => State::Changed(path),
        Err(_) => State::Missing(path),
    }
}

/// Makes the file of `invoice`, numbered and dated, and writes it under its
/// waiting name in `dir`, naming it on the record (`file`, `file_sha256`)
/// before the record is written; `commit` gives it its name after. Refused
/// over a kept file already there: an issued file is never written over.
pub(crate) fn write(dir: &Path, invoice: &mut Invoice, options: &Options) -> Result<pdf::Rendered, String> {
    let rendered = pdf::render(invoice, options)?;
    let name = file_name(&invoice.number);
    let kept = dir.join(&name);
    if kept.exists() {
        return Err(format!("{}: kept already", kept.display()));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let waiting = pending(dir, &invoice.number);
    std::fs::write(&waiting, &rendered.pdf).map_err(|e| format!("{}: {e}", waiting.display()))?;
    invoice.file = name;
    invoice.file_sha256 = sha256(&rendered.pdf);
    Ok(rendered)
}

/// The file given its own name, once its record is written.
pub(crate) fn commit(dir: &Path, invoice: &Invoice) -> Result<(), String> {
    let kept = path(dir, invoice);
    std::fs::rename(pending(dir, &invoice.number), &kept).map_err(|e| format!("{}: {e}", kept.display()))
}

/// The file taken away, waiting or named: only to undo an issue that failed after writing it.
pub(crate) fn remove(dir: &Path, invoice: &Invoice) {
    let _ = std::fs::remove_file(pending(dir, &invoice.number));
    if !invoice.file.is_empty() {
        let _ = std::fs::remove_file(path(dir, invoice));
    }
}

/// `file` moved into `dir/leftovers/` as `name`, or a free name beside it: never deleted.
fn set_aside(dir: &Path, file: &Path, name: &str) -> Result<PathBuf, String> {
    let folder = dir.join(LEFTOVERS);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let stem = name.strip_suffix(".pdf").unwrap_or(name);
    let mut target = folder.join(name);
    let mut n = 2;
    while target.exists() {
        target = folder.join(format!("{stem} ({n}).pdf"));
        n += 1;
    }
    std::fs::rename(file, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(target)
}

/// Before a number is given, on the device that numbers (under the series
/// journal's lock): what an issue stopped halfway left. A waiting file
/// whose record came with its SHA-256 (the stop fell after the record)
/// takes its name; any other waiting file, and a kept file no record holds,
/// is set aside in `leftovers/`. Returns what to say, once, in `tr`'s words.
pub(crate) fn tidy(dir: &Path, tr: &Translator) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut said = Vec::new();
    let mut files: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    for file in files {
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let (kept_name, waiting) = match name.strip_suffix(".pdf.new") {
            Some(stem) => (format!("{stem}.pdf"), true),
            None if name.ends_with(".pdf") => (name.clone(), false),
            None => continue,
        };
        let stem = kept_name.strip_suffix(".pdf").unwrap_or(&kept_name).to_string();
        let record = std::fs::read_to_string(dir.join(format!("{stem}.toml"))).ok();
        if waiting {
            let bytes = std::fs::read(&file).unwrap_or_default();
            let named = record.as_deref().and_then(super::record::read).is_some_and(|r| r.file == kept_name && r.file_sha256.eq_ignore_ascii_case(&sha256(&bytes)));
            if named && !dir.join(&kept_name).exists() && std::fs::rename(&file, dir.join(&kept_name)).is_ok() {
                continue;
            }
        } else if record.is_some() {
            continue;
        }
        if let Ok(target) = set_aside(dir, &file, &kept_name) {
            let folder = target.parent().map(|p| p.display().to_string()).unwrap_or_default();
            said.push(say(tr, "invoice-set-aside", &[("number", stem), ("folder", folder)]));
        }
    }
    said
}

/// The kept file made again from its frozen record, when this Sioul makes
/// the bytes issued (the QR code on or off, as it was).
fn again(invoice: &Invoice) -> Option<Vec<u8>> {
    [true, false]
        .into_iter()
        .filter_map(|qr| pdf::render(invoice, &Options { qr }).ok())
        .find(|r| sha256(&r.pdf).eq_ignore_ascii_case(invoice.file_sha256.trim()))
        .map(|r| r.pdf)
}

/// Why an invoice's file was not handed out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotGiven {
    /// The kept file is not the one issued, or not here, and cannot be made
    /// again as it was; or the record names another file: what to say, with
    /// where the record and the file are.
    Kept(String),
    /// Its copy could not be written there.
    Copy(PathBuf, String),
}

/// The bytes issued of `invoice`'s kept file in `dir`: as kept; gone or
/// changed, made again and kept again when this Sioul makes the same bytes
/// (a changed file first set aside in `leftovers/`); else refused.
pub fn issued(dir: &Path, invoice: &Invoice, tr: &Translator) -> Result<Vec<u8>, NotGiven> {
    let record = dir.join(super::file_name(&invoice.number)).display().to_string();
    match state(dir, invoice) {
        State::Kept(path) => std::fs::read(&path).map_err(|e| NotGiven::Kept(format!("{}: {e}", path.display()))),
        State::Misnamed(file) => Err(NotGiven::Kept(say(tr, "invoice-kept-misnamed", &[("number", invoice.number.clone()), ("file", file), ("record", record)]))),
        State::None => Err(NotGiven::Kept(say(tr, "invoice-kept-none", &[("number", invoice.number.clone())]))),
        State::Changed(path) | State::Missing(path) => {
            let changed = path.exists();
            let Some(bytes) = again(invoice) else {
                let id = if changed { "invoice-kept-refused" } else { "invoice-kept-lost" };
                return Err(NotGiven::Kept(say(tr, id, &[("number", invoice.number.clone()), ("record", record), ("file", path.display().to_string())])));
            };
            if changed {
                set_aside(dir, &path, &file_name(&invoice.number)).map_err(NotGiven::Kept)?;
            }
            let waiting = pending(dir, &invoice.number);
            std::fs::write(&waiting, &bytes).map_err(|e| NotGiven::Kept(format!("{}: {e}", waiting.display())))?;
            std::fs::rename(&waiting, &path).map_err(|e| NotGiven::Kept(format!("{}: {e}", path.display())))?;
            Ok(bytes)
        }
    }
}

/// The file issued of `invoice`, as `issued` gives it, copied byte for byte
/// to `destination` (its folder made); a copy there already the same is left
/// as it is.
pub fn copy_to(dir: &Path, invoice: &Invoice, destination: &Path, tr: &Translator) -> Result<(), NotGiven> {
    let bytes = issued(dir, invoice, tr)?;
    if std::fs::read(destination).is_ok_and(|there| there == bytes) {
        return Ok(());
    }
    let failed = |e: std::io::Error| NotGiven::Copy(destination.to_path_buf(), e.to_string());
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(failed)?;
    }
    let temporary = destination.with_extension("pdf.new");
    std::fs::write(&temporary, &bytes).map_err(failed)?;
    std::fs::rename(&temporary, destination).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        failed(e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoice::life::{self, Numbering, Steps};
    use crate::invoice::{all_in, drafts_in, find_in, samples, save_draft, series};

    fn place(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("sioul-kept-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn day() -> jiff::civil::Date {
        "2026-11-16".parse().unwrap()
    }

    /// A draft of the samples' French franchise, kept in `dir`, to issue.
    fn drafted(dir: &Path) -> String {
        let mut draft = samples::without_xml().invoice;
        draft.number.clear();
        draft.date.clear();
        save_draft(dir, &draft).unwrap().draft
    }

    fn issue(root: &Path, dir: &Path) -> Result<Invoice, String> {
        let id = drafted(dir);
        let time = root.join("time");
        life::issue(dir, &id, &samples::french("franchise", true), day(), &Numbering::default(), None, &Steps { time: Some(&time), ledger: None }, &Translator::new("en"))
    }

    /// Issued, an invoice keeps its file beside its record, named with its
    /// SHA-256 on it; printed again, that very file comes back, byte for
    /// byte; a record naming another file is refused.
    #[test]
    fn the_issued_file_is_kept_and_given_back() {
        let root = place("issue");
        let dir = root.join("invoices");
        let issued = issue(&root, &dir).unwrap();
        assert_eq!((issued.number.as_str(), issued.file.as_str()), ("2026-001", "2026-001.pdf"));
        let bytes = std::fs::read(dir.join("2026-001.pdf")).unwrap();
        assert_eq!(issued.file_sha256, sha256(&bytes));
        assert!(!dir.join("2026-001.pdf.new").exists(), "named once its record is written");
        let kept = find_in(&dir, "2026-001").unwrap();
        assert_eq!((kept.file.as_str(), kept.file_sha256.as_str()), ("2026-001.pdf", issued.file_sha256.as_str()), "the record names it");
        assert_eq!(state(&dir, &kept), State::Kept(dir.join("2026-001.pdf")));
        assert_eq!(crate::einvoice::from_pdf(&bytes).map(|e| e.number), Some("2026-001".into()), "its XML in it");
        let tr = Translator::new("en");
        let copy = root.join("Documents").join("Invoices").join("2026-001 Studio Lumen.pdf");
        copy_to(&dir, &kept, &copy, &tr).unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), bytes);
        copy_to(&dir, &kept, &copy, &tr).unwrap();
        // Its life written after: the file and its hash stay on the record.
        life::add_payment(&dir, "2026-001", crate::invoice::Payment { date: "2026-11-20".into(), amount: 100, means: "transfer".into(), movement: String::new() }).unwrap();
        assert_eq!(find_in(&dir, "2026-001").unwrap().file_sha256, issued.file_sha256);
        // A record naming another file than its number's: nothing handed out.
        let elsewhere = Invoice { file: "series.toml".into(), ..kept.clone() };
        assert_eq!(state(&dir, &elsewhere), State::Misnamed("series.toml".into()));
        assert!(matches!(copy_to(&dir, &elsewhere, &copy, &tr), Err(NotGiven::Kept(said)) if said.contains("series.toml")));
        assert_eq!(file_name("F:2026/001"), "F-2026-001.pdf");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// An issue stopped halfway never blocks the next: a kept file no record
    /// holds is set aside and said once, its number given again; a file
    /// whose record came takes its name; one with no record is set aside.
    #[test]
    fn an_issue_stopped_halfway_blocks_nothing() {
        let root = place("halfway");
        let dir = root.join("invoices");
        std::fs::create_dir_all(&dir).unwrap();
        // Stopped before its record: 2026-001's file alone, under its own name and waiting.
        std::fs::write(dir.join("2026-001.pdf"), b"orphan").unwrap();
        std::fs::write(dir.join("2026-009.pdf.new"), b"waiting, no record").unwrap();
        let issued = issue(&root, &dir).unwrap();
        assert_eq!(issued.number, "2026-001", "the number never committed, given again");
        assert_eq!(issued.said.len(), 2, "{:?}", issued.said);
        assert!(issued.said[0].contains("2026-001") && issued.said[0].contains(LEFTOVERS), "{:?}", issued.said);
        assert_eq!(std::fs::read(dir.join(LEFTOVERS).join("2026-001.pdf")).unwrap(), b"orphan", "set aside, never deleted");
        assert!(dir.join(LEFTOVERS).join("2026-009.pdf").exists() && !dir.join("2026-009.pdf.new").exists());
        assert_eq!(state(&dir, &find_in(&dir, "2026-001").unwrap()), State::Kept(dir.join("2026-001.pdf")));
        // Stopped after its record, before the file took its name: the next issue finishes it.
        std::fs::rename(dir.join("2026-001.pdf"), dir.join("2026-001.pdf.new")).unwrap();
        let next = issue(&root, &dir).unwrap();
        assert_eq!((next.number.as_str(), next.said.len()), ("2026-002", 0), "{:?}", next.said);
        assert_eq!(state(&dir, &find_in(&dir, "2026-001").unwrap()), State::Kept(dir.join("2026-001.pdf")));
        assert!(all_in(&dir).len() == 2 && drafts_in(&dir).is_empty() && series::Journal::load(&dir).series.len() == 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A kept file gone or changed is made again from its record, as it was
    /// issued (a changed one set aside first); one that cannot be made again
    /// is not handed out, and where the record and the file are is said.
    #[test]
    fn a_file_gone_or_changed_is_made_again_or_refused() {
        let root = place("again");
        let dir = root.join("invoices");
        issue(&root, &dir).unwrap();
        let bytes = std::fs::read(dir.join("2026-001.pdf")).unwrap();
        let kept = find_in(&dir, "2026-001").unwrap();
        let tr = Translator::new("en");
        let copy = root.join("copy.pdf");
        std::fs::remove_file(dir.join("2026-001.pdf")).unwrap();
        assert_eq!(state(&dir, &kept), State::Missing(dir.join("2026-001.pdf")));
        copy_to(&dir, &kept, &copy, &tr).unwrap();
        assert_eq!((std::fs::read(&copy).unwrap(), std::fs::read(dir.join("2026-001.pdf")).unwrap()), (bytes.clone(), bytes.clone()), "made again, the same bytes");
        std::fs::write(dir.join("2026-001.pdf"), b"annotated by a reader").unwrap();
        copy_to(&dir, &kept, &copy, &tr).unwrap();
        assert_eq!(std::fs::read(dir.join("2026-001.pdf")).unwrap(), bytes);
        assert_eq!(std::fs::read(dir.join(LEFTOVERS).join("2026-001.pdf")).unwrap(), b"annotated by a reader");
        // Issued by another Sioul (its bytes not made here): refused, the places said.
        std::fs::write(dir.join("2026-001.pdf"), b"changed again").unwrap();
        let other = Invoice { file_sha256: sha256(b"what another Sioul made"), ..kept.clone() };
        match copy_to(&dir, &other, &copy, &tr) {
            Err(NotGiven::Kept(said)) => assert!(said.contains("2026-001.toml") && said.contains("2026-001.pdf"), "{said}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(std::fs::read(&copy).unwrap(), bytes, "the copy left as it was");
        let _ = std::fs::remove_dir_all(&root);
    }
}
