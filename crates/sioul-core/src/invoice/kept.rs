// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The issued file, kept beside its record (docs/projects.md, "The issued
//! file"): `invoices/<number>.pdf` next to `invoices/<number>.toml`, the
//! record naming it (`file`) with its SHA-256 (`file_sha256`). Written once,
//! when the invoice is issued, in the same step as its record and its
//! number (`life::issue`: a step that fails undoes the others); it travels
//! with the records, in the sharing's part for drafts and invoices.
//!
//! Printing again gives back this file, byte for byte: a copy where your
//! invoices go (`copy_to`), the kept one untouched, so that a PDF reader
//! saving into its copy never changes what was issued. Nothing renders an
//! issued invoice again. A kept file changed or gone since is said, calmly
//! (`State`). Format-1 records, from before November 2026, have none: the
//! window prints them as it always did.

use super::pdf::{self, Options};
use super::record::Invoice;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The kept file's name: the record's, as a PDF.
pub fn file_name(number: &str) -> String {
    let record = super::file_name(number);
    format!("{}.pdf", record.strip_suffix(".toml").unwrap_or(&record))
}

/// Where `invoice`'s kept file is in `dir`.
pub fn path(dir: &Path, invoice: &Invoice) -> PathBuf {
    // The record's own word, a plain name in its folder; else from its number.
    let named = Path::new(&invoice.file).file_name().filter(|n| !invoice.file.is_empty() && n.to_string_lossy() == invoice.file).map(|n| n.to_string_lossy().into_owned());
    dir.join(named.unwrap_or_else(|| file_name(&invoice.number)))
}

/// The SHA-256 of `bytes`, in hexadecimal.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
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
    /// A record without one: from before November 2026, or a draft.
    None,
}

/// Where `invoice`'s kept file stands in `dir`.
pub fn state(dir: &Path, invoice: &Invoice) -> State {
    if invoice.file.is_empty() || invoice.number.is_empty() {
        return State::None;
    }
    let path = path(dir, invoice);
    match std::fs::read(&path) {
        Ok(bytes) if sha256(&bytes).eq_ignore_ascii_case(invoice.file_sha256.trim()) => State::Kept(path),
        Ok(_) => State::Changed(path),
        Err(_) => State::Missing(path),
    }
}

/// Makes and keeps the file of `invoice`, numbered and dated, in `dir`, and
/// names it on the record (`file`, `file_sha256`), before the record is
/// written. Refused over a file already there: an issued file is never
/// written over.
pub(crate) fn write(dir: &Path, invoice: &mut Invoice, options: &Options) -> Result<pdf::Rendered, String> {
    let rendered = pdf::render(invoice, options)?;
    let name = file_name(&invoice.number);
    let path = dir.join(&name);
    if path.exists() {
        return Err(format!("{}: kept already", path.display()));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Whole, through a file beside it that the sharing leaves alone.
    let temporary = dir.join(format!("{name}.new"));
    std::fs::write(&temporary, &rendered.pdf).map_err(|e| format!("{}: {e}", temporary.display()))?;
    if let Err(e) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("{}: {e}", path.display()));
    }
    invoice.file = name;
    invoice.file_sha256 = sha256(&rendered.pdf);
    Ok(rendered)
}

/// The kept file taken away: only to undo an issue that failed after writing it.
pub(crate) fn remove(dir: &Path, invoice: &Invoice) {
    if !invoice.file.is_empty() {
        let _ = std::fs::remove_file(path(dir, invoice));
    }
}

/// The kept file of `invoice`, copied byte for byte to `destination` (its
/// folder made); a copy there already the same is left as it is. Returns
/// where the kept file stands: a changed one is copied too, as it is now,
/// and said; a missing one is not copied.
pub fn copy_to(dir: &Path, invoice: &Invoice, destination: &Path) -> Result<State, String> {
    let state = state(dir, invoice);
    let source = match &state {
        State::Kept(path) | State::Changed(path) => path.clone(),
        State::Missing(_) | State::None => return Ok(state),
    };
    let bytes = std::fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?;
    if std::fs::read(destination).is_ok_and(|there| there == bytes) {
        return Ok(state);
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let temporary = destination.with_extension("pdf.new");
    std::fs::write(&temporary, &bytes).map_err(|e| format!("{}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, destination).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        format!("{}: {e}", destination.display())
    })?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Translator;
    use crate::invoice::life::{self, Numbering, Steps};
    use crate::invoice::{all_in, drafts_in, find_in, samples, save_draft, series};

    fn place(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("sioul-kept-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// Issued, an invoice keeps its file beside its record, named with its
    /// SHA-256 on it; printed again, that very file comes back, byte for
    /// byte; a file changed or gone since is said; a file already where the
    /// next one would go refuses the issue, nothing written.
    #[test]
    fn the_issued_file_is_kept_and_given_back() {
        let root = place("issue");
        let (dir, time) = (root.join("invoices"), root.join("time"));
        let sample = samples::without_xml();
        let s = samples::french("franchise", true);
        let mut draft = sample.invoice.clone();
        draft.number.clear();
        draft.date.clear();
        let saved = save_draft(&dir, &draft).unwrap();
        // A file already where 2026-001's would go: refused, nothing given.
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("2026-001.pdf"), b"not ours").unwrap();
        let day: jiff::civil::Date = "2026-11-16".parse().unwrap();
        let steps = Steps { time: Some(&time), ledger: None };
        let refused = life::issue(&dir, &saved.draft, &s, day, &Numbering::default(), None, &steps, &Translator::new("en"));
        assert!(refused.is_err_and(|e| e.contains("kept already")));
        assert!(all_in(&dir).is_empty() && series::Journal::load(&dir).series.is_empty() && drafts_in(&dir).len() == 1, "nothing issued");
        assert_eq!(std::fs::read(dir.join("2026-001.pdf")).unwrap(), b"not ours", "never written over");
        std::fs::remove_file(dir.join("2026-001.pdf")).unwrap();

        let issued = life::issue(&dir, &saved.draft, &s, day, &Numbering::default(), None, &steps, &Translator::new("en")).unwrap();
        assert_eq!((issued.number.as_str(), issued.file.as_str()), ("2026-001", "2026-001.pdf"));
        let bytes = std::fs::read(dir.join("2026-001.pdf")).unwrap();
        assert_eq!(issued.file_sha256, sha256(&bytes));
        let kept = find_in(&dir, "2026-001").unwrap();
        assert_eq!((kept.file.as_str(), kept.file_sha256.as_str()), ("2026-001.pdf", issued.file_sha256.as_str()), "the record names it");
        assert_eq!(state(&dir, &kept), State::Kept(dir.join("2026-001.pdf")));
        assert_eq!(crate::einvoice::from_pdf(&bytes).map(|e| e.number), Some("2026-001".into()), "its XML in it");
        // Printed again: a copy where invoices go, the same bytes; again, left as it is.
        let copy = root.join("Documents").join("Invoices").join("2026-001 Studio Lumen.pdf");
        assert_eq!(copy_to(&dir, &kept, &copy).unwrap(), State::Kept(dir.join("2026-001.pdf")));
        assert_eq!(std::fs::read(&copy).unwrap(), bytes);
        assert!(copy_to(&dir, &kept, &copy).is_ok());
        // Its life written after: the file and its hash stay on the record.
        life::add_payment(&dir, "2026-001", crate::invoice::Payment { date: "2026-11-20".into(), amount: 100, means: "transfer".into(), movement: String::new() }).unwrap();
        assert_eq!(find_in(&dir, "2026-001").unwrap().file_sha256, issued.file_sha256);
        // Changed outside Sioul: said, given as it is now; gone: said, nothing copied.
        std::fs::write(dir.join("2026-001.pdf"), b"changed").unwrap();
        assert_eq!(state(&dir, &kept), State::Changed(dir.join("2026-001.pdf")));
        assert_eq!(copy_to(&dir, &kept, &copy).unwrap(), State::Changed(dir.join("2026-001.pdf")));
        assert_eq!(std::fs::read(&copy).unwrap(), b"changed");
        std::fs::remove_file(dir.join("2026-001.pdf")).unwrap();
        assert_eq!(state(&dir, &kept), State::Missing(dir.join("2026-001.pdf")));
        // A record naming a file elsewhere is read as its own number's.
        let elsewhere = Invoice { file: "../2026-001.pdf".into(), ..kept.clone() };
        assert_eq!(path(&dir, &elsewhere), dir.join("2026-001.pdf"));
        assert_eq!(file_name("F:2026/001"), "F-2026-001.pdf");
        let _ = std::fs::remove_dir_all(&root);
    }
}
