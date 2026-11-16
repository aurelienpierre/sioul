// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Invoices you issue: invoices, credit notes, deposit invoices
//! (docs/projects.md, "Invoices").
//!
//! An invoice is one record in EN 16931's vocabulary (`record`): a draft
//! until it is issued, kept in `$XDG_DATA_HOME/sioul/invoices/drafts/`; then
//! numbered (`series`), its issuer and client frozen as they were, kept as
//! `invoices/<number>.toml`, never changed but for its life: its payments,
//! the day it was sent. A wrong invoice is answered by a credit note citing
//! it. The rules of a country (its mentions, identifiers, rates, thresholds)
//! are data (`presets`); the life of an invoice is `life`; its page to print
//! is `html`. The date or period of the service is said on it, as French law
//! asks (service-public.gouv.fr, sheet F31808).

pub mod html;
pub mod ids;
pub mod life;
pub mod presets;
pub mod record;
pub mod series;
pub mod totals;

pub use html::{html, page};
pub use record::{Address, Bank, Cited, Client, Identifier, Invoice, Issuer, Kind, Line, Nature, Note, Payment, Status, TaxBreak, Terms, Totals};

use crate::config::data_dir;
use std::path::{Path, PathBuf};

/// Where invoices are kept.
pub fn folder() -> PathBuf {
    data_dir().join("invoices")
}

/// Where the window writes an invoice's PDF: the folder of the settings,
/// else `~/Documents/<default_folder>` ("Invoices", "Factures"); the file
/// named by its number and its client, with nothing a system refuses in a
/// name (a prefix such as "F/2026-" makes no folder).
pub fn pdf_path(invoice: &Invoice, folder_setting: &str, default_folder: &str) -> PathBuf {
    let folder = if folder_setting.trim().is_empty() { crate::config::expand_home(&format!("~/Documents/{default_folder}")) } else { crate::config::expand_home(folder_setting.trim()) };
    let client: String = invoice.client.first_line().chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-').collect();
    let number: String = invoice.number.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '-' }).collect();
    folder.join(format!("{} {}.pdf", number.trim_matches('.'), client.trim()).trim().to_string())
}

/// Drafts, in the invoices' folder: they travel with them.
pub const DRAFTS: &str = "drafts";

/// A number's file name: what Windows refuses in a name (":" would even
/// write into a hidden stream) becomes "-".
pub fn file_name(number: &str) -> String {
    let name: String = number.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c }).collect();
    format!("{}.toml", name.trim_end_matches(['.', ' ']))
}

/// A record's text read, or what holds its number when it no longer reads
/// (edited by hand, written by a newer Sioul): the number is never given
/// again, nor its file written over.
fn read_file(path: &Path) -> Option<Invoice> {
    let text = std::fs::read_to_string(path).ok();
    if let Some(invoice) = text.as_deref().and_then(record::read) {
        return Some(invoice);
    }
    let number = text.and_then(|t| t.parse::<toml::Table>().ok()).and_then(|t| t.get("number")?.as_str().map(str::to_string));
    let number = number.or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
    Some(Invoice { number, unreadable: true, ..Invoice::default() })
}

/// Every invoice issued and kept in `dir`, by number (2026-999 before
/// 2026-1000). The series journal and the drafts are not invoices.
pub fn all_in(dir: &Path) -> Vec<Invoice> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Invoice> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "toml") && p.file_name().is_some_and(|n| n != series::JOURNAL))
        .filter_map(|p| read_file(&p))
        .collect();
    out.sort_by(|a, b| natural(&a.number, &b.number));
    out
}

/// One issued invoice of `dir`, by number.
pub fn find_in(dir: &Path, number: &str) -> Option<Invoice> {
    all_in(dir).into_iter().find(|i| i.number == number && !i.unreadable)
}

/// The drafts kept in `dir`, the newest first.
pub fn drafts_in(dir: &Path) -> Vec<Invoice> {
    let Ok(entries) = std::fs::read_dir(dir.join(DRAFTS)) else { return Vec::new() };
    let mut out: Vec<Invoice> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .filter_map(|p| std::fs::read_to_string(&p).ok().and_then(|t| record::read(&t)))
        .filter(|d| d.number.is_empty() && !d.draft.is_empty())
        .collect();
    out.sort_by(|a, b| b.draft.cmp(&a.draft));
    out
}

/// One draft of `dir`, by its id.
pub fn draft_in(dir: &Path, id: &str) -> Option<Invoice> {
    drafts_in(dir).into_iter().find(|d| d.draft == id)
}

/// Numbers in their natural order: digits as numbers, the rest as text.
pub fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let digit = c.is_ascii_digit();
            match out.last_mut() {
                Some((d, text)) if *d == digit => text.push(c),
                _ => out.push((digit, c.to_string())),
            }
        }
        out
    }
    let (x, y) = (chunks(a), chunks(b));
    for (p, q) in x.iter().zip(&y) {
        let order = match (p.0, q.0) {
            (true, true) => {
                let (m, n) = (p.1.trim_start_matches('0'), q.1.trim_start_matches('0'));
                m.len().cmp(&n.len()).then_with(|| m.cmp(n))
            }
            _ => p.1.cmp(&q.1),
        };
        if order.is_ne() {
            return order;
        }
    }
    x.len().cmp(&y.len())
}

/// Writes a file whole, through a file beside it.
fn write_whole(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let temporary = path.with_extension("toml.new");
    std::fs::write(&temporary, text).map_err(|e| format!("{}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, path).map_err(|e| format!("{}: {e}", path.display()))?;
    crate::changes::said(crate::changes::Kind::Projects);
    Ok(())
}

/// Keeps a draft in `dir` (its id given when it has none); returns it.
pub fn save_draft(dir: &Path, draft: &Invoice) -> Result<Invoice, String> {
    if !draft.number.is_empty() {
        return Err(format!("{}: an issued invoice is no draft", draft.number));
    }
    let mut draft = draft.clone();
    if draft.draft.is_empty() {
        draft.draft = new_draft_id();
    }
    draft.format = record::FORMAT;
    let text = toml::to_string(&draft).map_err(|e| e.to_string())?;
    write_whole(&dir.join(DRAFTS).join(format!("{}.toml", draft.draft)), &text)?;
    Ok(draft)
}

/// A draft taken out of `dir`; Ok(false) when it was not there.
pub fn remove_draft(dir: &Path, id: &str) -> Result<bool, String> {
    let path = dir.join(DRAFTS).join(format!("{id}.toml"));
    match std::fs::remove_file(&path) {
        Ok(()) => {
            crate::changes::said(crate::changes::Kind::Projects);
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// A draft's own id: when it was made, and a few letters; never a number.
fn new_draft_id() -> String {
    let now = jiff::Zoned::now();
    let tail: String = crate::ids::new().chars().filter(char::is_ascii_alphanumeric).take(6).collect();
    format!("{}-{tail}", now.strftime("%Y%m%d-%H%M%S"))
}

/// An issued record kept as it is, under its number: for tests and imports;
/// issuing is `life::issue`. Refused over another record (`write_new`).
pub fn keep_issued(dir: &Path, invoice: &Invoice) -> Result<PathBuf, String> {
    write_new(dir, invoice)
}

/// Keeps a newly issued record under its number. Refused when its file is
/// there already: another number that makes the same file name ("F:1" and
/// "F/1"), or a record not to be written over.
pub(crate) fn write_new(dir: &Path, invoice: &Invoice) -> Result<PathBuf, String> {
    if invoice.number.is_empty() {
        return Err("an invoice is kept under its number".into());
    }
    let path = dir.join(file_name(&invoice.number));
    if path.exists() {
        let there = read_file(&path).map(|i| i.number).unwrap_or_default();
        return Err(format!("{}: {}", path.display(), if there == invoice.number { "kept already".to_string() } else { format!("holds {there}, whose file name is the same") }));
    }
    let mut invoice = invoice.clone();
    invoice.format = record::FORMAT;
    let text = toml::to_string(&invoice).map_err(|e| e.to_string())?;
    write_whole(&path, &text)?;
    Ok(path)
}

/// An issued record's life written in place: its payments and the day it
/// was sent, every other line of its file kept as it is (a format-1 record's
/// `paid` too, for an older Sioul). Refused for a record that does not read,
/// or whose file holds another number.
pub fn write_life(dir: &Path, invoice: &Invoice) -> Result<(), String> {
    use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};
    let path = dir.join(file_name(&invoice.number));
    crate::filelock::with_lock(&path, || {
        let fail = |e: String| format!("{}: {e}", path.display());
        let text = std::fs::read_to_string(&path).map_err(|e| fail(e.to_string()))?;
        let kept = record::read(&text).ok_or_else(|| fail("this record no longer reads: Sioul leaves it as it is".into()))?;
        if kept.number != invoice.number {
            return Err(fail(format!("holds {}", kept.number)));
        }
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        if invoice.payments.is_empty() {
            doc.remove("payments");
        } else {
            let mut list = ArrayOfTables::new();
            for p in &invoice.payments {
                let mut t = Table::new();
                t["date"] = value(p.date.as_str());
                t["amount"] = value(p.amount);
                t["means"] = value(p.means.as_str());
                t["movement"] = value(p.movement.as_str());
                list.push(t);
            }
            doc.insert("payments", Item::ArrayOfTables(list));
        }
        if invoice.sent.is_empty() && kept.format == 1 {
            doc.remove("sent");
        } else {
            doc.insert("sent", value(invoice.sent.as_str()));
        }
        if kept.format == 1 {
            // An older Sioul reads `paid`: said with the payments.
            let all = std::slice::from_ref(invoice);
            doc.insert("paid", value(invoice.older.as_ref().is_some_and(|o| o.paid) || invoice.status(all).settled()));
        }
        let changed = doc.to_string();
        if changed != text {
            write_whole(&path, &changed)?;
        }
        Ok(())
    })
}

/// An issued record taken out: only to undo an issue that failed halfway.
pub(crate) fn remove_issued(dir: &Path, number: &str) -> Result<(), String> {
    let path = dir.join(file_name(number));
    std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-invoices-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A record that no longer reads still holds its number, and is never
    /// written over; two numbers making one file name: the second refused.
    #[test]
    fn a_number_is_never_given_twice() {
        let dir = dir("kept");
        std::fs::write(dir.join("2026-003.toml"), "number = \"2026-003\"\ndate = oops\n").unwrap();
        std::fs::write(dir.join("2026-004.toml"), "not toml at all [").unwrap();
        let kept = all_in(&dir);
        assert_eq!(kept.iter().map(|i| (i.number.as_str(), i.unreadable)).collect::<Vec<_>>(), [("2026-003", true), ("2026-004", true)]);
        let pattern = series::Pattern::of_prefix("");
        assert_eq!(series::next(&pattern, "2026-11-16".parse().unwrap(), &series::Journal::default(), &kept, &Default::default()).0, "2026-005");
        assert!(write_life(&dir, &Invoice { number: "2026-003".into(), ..Invoice::default() }).is_err(), "never written over");
        // A number with what Windows refuses in a file name is kept under a name it takes.
        let path = write_new(&dir, &Invoice { number: "F:2026/001".into(), currency: "EUR".into(), ..Invoice::default() }).unwrap();
        assert_eq!(path.file_name().unwrap(), "F-2026-001.toml");
        let again = write_new(&dir, &Invoice { number: "F/2026:001".into(), currency: "EUR".into(), ..Invoice::default() });
        assert!(again.is_err_and(|e| e.contains("F:2026/001")), "the second refused, the first kept");
        assert_eq!(find_in(&dir, "F:2026/001").map(|i| i.format), Some(2));
        // The journal is no invoice; 2026-1000 comes after 2026-999.
        series::Journal::default().save(&dir).unwrap();
        write_new(&dir, &Invoice { number: "2026-1000".into(), currency: "EUR".into(), ..Invoice::default() }).unwrap();
        write_new(&dir, &Invoice { number: "2026-999".into(), currency: "EUR".into(), ..Invoice::default() }).unwrap();
        let numbers: Vec<String> = all_in(&dir).into_iter().map(|i| i.number).collect();
        assert_eq!(numbers, ["2026-003", "2026-004", "2026-999", "2026-1000", "F:2026/001"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A format-1 record (before November 2026) reads as an issued invoice
    /// of kind 380 with time lines; Paid writes its life in place, every
    /// other line kept, `paid` said for an older Sioul.
    #[test]
    fn a_format_1_record_reads_and_stays_as_it_is() {
        let dir = dir("older");
        let older = "number = \"2026-001\"\ndate = \"2026-09-01\"\ndue = \"2026-10-01\"\nproject = \"lumen\"\nproject_title = \"Studio Lumen\"\nclient = \"Studio Lumen\\nAnna Exemple\"\nclient_address = \"1 rue de l’Exemple\\n75002 Paris\"\nsessions = [\"10:t:90\", \"20:t:30\"]\ntotal_cents = 12000\ncurrency = \"EUR\"\n\n[[lines]]\nlabel = \"Build the site\"\nminutes = 120\nrate = 60.0\ncents = 12000\n\n[issuer]\nname = \"Camille Exemple EI\"\naddress = \"2 place de l’Exemple\\n69001 Lyon\"\nsiret = \"732 829 320 00017\"\nvat = \"TVA non applicable, art. 293 B du CGI\"\npayment = \"IBAN FR76 0000\"\n";
        std::fs::write(dir.join("2026-001.toml"), older).unwrap();
        let read = find_in(&dir, "2026-001").unwrap();
        assert_eq!((read.format, read.kind, read.totals.due, read.client.first_line()), (1, Kind::Invoice, 12000, "Studio Lumen"));
        assert_eq!((read.lines[0].minutes(), read.lines[0].price, read.lines[0].amount), (Some(120), 6000, 12000));
        assert_eq!(read.sessions(), ["10:t:90", "20:t:30"]);
        assert_eq!(read.status(&[]), Status::Issued);
        let mut paid = read.clone();
        paid.payments.push(Payment { date: "2026-10-02".into(), amount: 12000, means: "transfer".into(), movement: String::new() });
        write_life(&dir, &paid).unwrap();
        let text = std::fs::read_to_string(dir.join("2026-001.toml")).unwrap();
        let mut rest = text.as_str();
        for line in older.lines().filter(|l| !l.is_empty()) {
            let at = rest.find(line).unwrap_or_else(|| panic!("{line} kept, in its place:\n{text}"));
            rest = &rest[at + line.len()..];
        }
        assert!(text.contains("paid = true") && !text.contains("format"), "{text}");
        let again = find_in(&dir, "2026-001").unwrap();
        assert_eq!((again.format, again.status(&[]), again.paid()), (1, Status::Paid, 12000));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
