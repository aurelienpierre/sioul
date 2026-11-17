// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Invoices you issue: invoices, credit notes, deposit invoices
//! (docs/projects.md, "Invoices").
//!
//! An invoice is one record in EN 16931's vocabulary (`record`): a draft
//! until it is issued, kept in `$XDG_DATA_HOME/sioul/invoices/drafts/`; then
//! numbered (`series`), its issuer and client frozen as they were, kept as
//! `invoices/<number>.toml` and never written again. Its life (its
//! payments, the day it was sent) is kept apart, each device in its own file
//! (`lives`). A wrong invoice is answered by a credit note citing it. The rules of a country (its mentions, identifiers, rates, thresholds)
//! are data (`presets`); the life of an invoice is `life`; its page to print
//! is `html`, for drafts and records from before November 2026. Issued, an
//! invoice is a PDF/A-3 (`pdf`) carrying its Factur-X XML (`cii`) and, in
//! euros, the EPC payment QR code (`epc`), kept beside its record with its
//! SHA-256 (`kept`) and given back as it is when printed again. The date or
//! period of the service is said on it, as French law asks
//! (service-public.gouv.fr, sheet F31808).

pub mod cii;
pub mod edit;
pub mod epc;
pub mod html;
pub mod ids;
pub mod kept;
pub mod life;
pub mod lives;
pub mod pdf;
mod pdfa;
pub mod presets;
pub mod record;
#[cfg(test)]
pub(crate) mod samples;
pub mod series;
pub mod totals;
pub mod units;

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

/// A number two files hold: a sync app's conflicted copy of a record, which
/// only two devices giving the same number make now (records are never
/// written again). Said in `sioul invoices` and on the Porch, until one of
/// them is answered by a credit note and invoiced again.
#[derive(Debug, Clone, PartialEq)]
pub struct Twice {
    pub number: String,
    /// The file its number names, which Sioul reads, and the other.
    pub kept: PathBuf,
    pub copy: PathBuf,
    /// Whom each bills, as their first lines say.
    pub clients: (String, String),
}

/// The issued records of `dir`, one per number, their life not read, and
/// the numbers two files hold (`Twice`): the record kept is the file its
/// number names; a copy that is the same invoice (an older Sioul's Paid
/// aside, which counts if either says it) is no second one.
fn records_and_twice(dir: &Path) -> (Vec<Invoice>, Vec<Twice>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return (Vec::new(), Vec::new()) };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "toml") && p.file_name().is_some_and(|n| n != series::OLD_JOURNAL)).collect();
    paths.sort();
    let mut by: std::collections::BTreeMap<String, (PathBuf, Invoice)> = std::collections::BTreeMap::new();
    let mut twice = Vec::new();
    let own = |p: &Path, number: &str| p.file_name().is_some_and(|n| n.to_string_lossy() == file_name(number));
    for path in paths {
        let Some(invoice) = read_file(&path) else { continue };
        if !by.contains_key(&invoice.number) {
            by.insert(invoice.number.clone(), (path, invoice));
            continue;
        }
        let Some((kept_path, kept)) = by.get_mut(&invoice.number) else { continue };
        let unpaid = |i: &Invoice| Invoice { older: i.older.clone().map(|o| record::Older { paid: false, ..o }), ..i.clone() };
        if unpaid(kept) == unpaid(&invoice) {
            if let (Some(a), Some(b)) = (kept.older.as_mut(), invoice.older.as_ref()) {
                a.paid |= b.paid;
            }
            continue;
        }
        let (kept_one, copy) = if own(kept_path, &invoice.number) || !own(&path, &invoice.number) { ((kept_path.clone(), kept.clone()), (path, invoice)) } else { ((path, invoice), (kept_path.clone(), kept.clone())) };
        eprintln!("Sioul: {} holds invoice {} again, as {} does: two invoices under one number", copy.0.display(), copy.1.number, kept_one.0.display());
        twice.push(Twice { number: copy.1.number.clone(), kept: kept_one.0.clone(), copy: copy.0.clone(), clients: (kept_one.1.client.first_line().to_string(), copy.1.client.first_line().to_string()) });
        by.insert(kept_one.1.number.clone(), kept_one);
    }
    (by.into_values().map(|(_, invoice)| invoice).collect(), twice)
}

/// The issued records of `dir`, one per number, their life not read
/// (`records_and_twice`). The series journal of before (`series.toml`) is
/// no invoice.
pub fn records_in(dir: &Path) -> Vec<Invoice> {
    records_and_twice(dir).0
}

/// The numbers two different records of `dir` hold (`Twice`).
pub fn twice_in(dir: &Path) -> Vec<Twice> {
    records_and_twice(dir).1
}

/// Every invoice issued and kept in `dir`, with its life (`lives`), by
/// number (2026-999 before 2026-1000). The series journal and the drafts
/// are not invoices.
pub fn all_in(dir: &Path) -> Vec<Invoice> {
    let mut out = records_in(dir);
    lives::Lives::load(dir).apply(&mut out);
    out.sort_by(|a, b| natural(&a.number, &b.number));
    out
}

/// One issued invoice of `dir`, by number.
pub fn find_in(dir: &Path, number: &str) -> Option<Invoice> {
    all_in(dir).into_iter().find(|i| i.number == number && !i.unreadable)
}

/// What changed outside Sioul in an issued invoice (`outside`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Outside {
    /// Its record's bytes are not those written at issue.
    pub record: bool,
    /// Its kept file is not the file issued, or the record names another file.
    pub file: bool,
}

impl Outside {
    pub fn any(self) -> bool {
        self.record || self.file
    }
}

/// Whether an issued record of `dir`, or its kept file, was changed
/// outside Sioul since it was issued: the record's bytes against the
/// SHA-256 its journal's entry keeps (`series::Journal::records`, written
/// from phase 3 on: a record issued before has none and is not judged), the
/// kept file against the record's `file_sha256` (`kept::state`; a file not
/// on this device yet is not "changed"). A record from before November 2026
/// (format 1) has neither, and is never judged.
pub fn outside(dir: &Path, invoice: &Invoice, journal: &series::Journal) -> Outside {
    if invoice.number.is_empty() || invoice.unreadable || invoice.older.is_some() {
        return Outside::default();
    }
    let record = journal.records.get(&invoice.number).is_some_and(|kept| std::fs::read(dir.join(file_name(&invoice.number))).is_ok_and(|bytes| !kept::sha256(&bytes).eq_ignore_ascii_case(kept.trim())));
    let file = matches!(kept::state(dir, invoice), kept::State::Changed(_) | kept::State::Misnamed(_));
    Outside { record, file }
}

/// Whether a draft's id is one: letters, digits and dashes, as Sioul gives
/// them, never a path.
pub fn draft_id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// A draft's file name made its id: as it is when it is one; a sync app's
/// conflicted copy of a draft ("<id> (conflicted copy …)", "<id>.sync-
/// conflict-…") given one of its own, letters, digits and dashes, with the
/// id it is a copy of.
fn draft_file_id(stem: &str) -> Option<(String, String)> {
    if draft_id_ok(stem) {
        return Some((stem.to_string(), String::new()));
    }
    let made: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
    let made: String = made.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-").chars().take(64).collect();
    let made = made.trim_end_matches('-').to_string();
    let original: String = stem.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    draft_id_ok(&made).then(|| (made, if draft_id_ok(&original) { original } else { stem.to_string() }))
}

/// The drafts kept in `dir`, the newest first: each one's id is its file's
/// name (never what the file says). A sync app's conflicted copy of a draft
/// is listed too, never hidden, as a draft of its own that says what it is
/// a copy of (`Invoice::copy_of`).
pub fn drafts_in(dir: &Path) -> Vec<Invoice> {
    let Ok(entries) = std::fs::read_dir(dir.join(DRAFTS)) else { return Vec::new() };
    let mut out: Vec<Invoice> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .filter_map(|p| {
            let (id, copy_of) = draft_file_id(&p.file_stem()?.to_string_lossy())?;
            let mut draft = std::fs::read_to_string(&p).ok().and_then(|t| record::read(&t))?;
            draft.draft = id;
            draft.copy_of = copy_of;
            draft.number.clear();
            Some(draft)
        })
        .collect();
    out.sort_by(|a, b| b.draft.cmp(&a.draft).then_with(|| a.copy_of.cmp(&b.copy_of)));
    out.dedup_by(|a, b| a.draft == b.draft);
    out
}

/// One draft of `dir`, by its id.
pub fn draft_in(dir: &Path, id: &str) -> Option<Invoice> {
    if !draft_id_ok(id) {
        return None;
    }
    drafts_in(dir).into_iter().find(|d| d.draft == id)
}

/// The file holding the draft `id` of `dir`: its own, else the conflicted copy whose id it is.
fn draft_file(dir: &Path, id: &str) -> Option<PathBuf> {
    let own = dir.join(DRAFTS).join(format!("{id}.toml"));
    if own.is_file() {
        return Some(own);
    }
    std::fs::read_dir(dir.join(DRAFTS)).ok()?.filter_map(Result::ok).map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "toml") && p.file_stem().and_then(|s| draft_file_id(&s.to_string_lossy())).is_some_and(|(made, _)| made == id))
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

/// Writes a file whole, through a hidden file beside it renamed over it,
/// tried again on Windows while another program holds it open a moment
/// (`filelock::replace`, Sioul's shared writer).
pub(crate) fn write_whole(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
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
    if !draft_id_ok(&draft.draft) {
        return Err(format!("“{}” is no draft's id", draft.draft));
    }
    draft.format = record::FORMAT;
    let text = toml::to_string(&draft).map_err(|e| e.to_string())?;
    let copy = draft_file(dir, &draft.draft).filter(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() != draft.draft));
    write_whole(&dir.join(DRAFTS).join(format!("{}.toml", draft.draft)), &text)?;
    // A conflicted copy kept under its own id from now on: its file of before goes.
    if let Some(copy) = copy {
        let _ = std::fs::remove_file(copy);
    }
    Ok(draft)
}

/// A draft taken out of `dir`; Ok(false) when it was not there.
pub fn remove_draft(dir: &Path, id: &str) -> Result<bool, String> {
    if !draft_id_ok(id) {
        return Err(format!("“{id}” is no draft's id"));
    }
    let Some(path) = draft_file(dir, id) else { return Ok(false) };
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
    /// written over; two numbers making one file name: the second refused; a
    /// sync app's conflicted copy of a record is no second invoice (F12).
    #[test]
    fn a_number_is_never_given_twice() {
        let dir = dir("kept");
        std::fs::write(dir.join("2026-003.toml"), "number = \"2026-003\"\ndate = oops\n").unwrap();
        std::fs::write(dir.join("2026-004.toml"), "not toml at all [").unwrap();
        let kept = all_in(&dir);
        assert_eq!(kept.iter().map(|i| (i.number.as_str(), i.unreadable)).collect::<Vec<_>>(), [("2026-003", true), ("2026-004", true)]);
        let pattern = series::Pattern::of_prefix("");
        assert_eq!(series::next(&pattern, "2026-11-16".parse().unwrap(), &series::Journal::default(), &kept, &Default::default()).0, "2026-005");
        // A number with what Windows refuses in a file name is kept under a name it takes.
        let path = write_new(&dir, &Invoice { number: "F:2026/001".into(), currency: "EUR".into(), ..Invoice::default() }).unwrap();
        assert_eq!(path.file_name().unwrap(), "F-2026-001.toml");
        let again = write_new(&dir, &Invoice { number: "F/2026:001".into(), currency: "EUR".into(), ..Invoice::default() });
        assert!(again.is_err_and(|e| e.contains("F:2026/001")), "the second refused, the first kept");
        assert_eq!(find_in(&dir, "F:2026/001").map(|i| i.format), Some(2));
        // The journal of before is no invoice; 2026-1000 comes after 2026-999.
        std::fs::write(dir.join(series::OLD_JOURNAL), "format = 1\n").unwrap();
        write_new(&dir, &Invoice { number: "2026-1000".into(), currency: "EUR".into(), ..Invoice::default() }).unwrap();
        write_new(&dir, &Invoice { number: "2026-999".into(), currency: "EUR".into(), ..Invoice::default() }).unwrap();
        // A conflicted copy of 2026-999, as a sync app makes it: the same invoice, one.
        std::fs::copy(dir.join("2026-999.toml"), dir.join("2026-999 (conflict 2026-11-16 10.30).toml")).unwrap();
        let numbers: Vec<String> = all_in(&dir).into_iter().map(|i| i.number).collect();
        assert_eq!(numbers, ["2026-003", "2026-004", "2026-999", "2026-1000", "F:2026/001"]);
        assert!(twice_in(&dir).is_empty());
        // Two devices gave 2026-1000 at once, to two clients (G9): one record read, both said.
        let other = Invoice { number: "2026-1000".into(), currency: "EUR".into(), client: Client { name: "Studio Exemple".into(), ..Client::default() }, format: record::FORMAT, ..Invoice::default() };
        std::fs::write(dir.join("2026-1000 (conflicted copy 2026-11-20 103000).toml"), toml::to_string(&other).unwrap()).unwrap();
        let twice = twice_in(&dir);
        assert_eq!((twice.len(), twice[0].number.as_str(), twice[0].clients.1.as_str()), (1, "2026-1000", "Studio Exemple"));
        assert!(twice[0].kept.ends_with("2026-1000.toml"));
        assert_eq!(all_in(&dir).iter().filter(|i| i.number == "2026-1000").count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two older Sioul ticking Paid on one format-1 record make a conflicted
    /// copy of it that is the same invoice: one, paid if either says so (G9).
    #[test]
    fn an_older_copy_of_the_same_record_is_one() {
        let dir = dir("older-copy");
        let older = "number = \"2026-002\"\ndate = \"2026-09-01\"\ndue = \"2026-10-01\"\nproject = \"lumen\"\nproject_title = \"Studio Lumen\"\nclient = \"Studio Lumen\"\nsessions = []\ntotal_cents = 6000\ncurrency = \"EUR\"\nlines = []\n\n[issuer]\nname = \"Camille Exemple EI\"\naddress = \"Lyon\"\nsiret = \"\"\nvat = \"\"\npayment = \"\"\n";
        std::fs::write(dir.join("2026-002.toml"), older).unwrap();
        std::fs::write(dir.join("2026-002 (conflicted copy).toml"), older.replace("currency = \"EUR\"\n", "currency = \"EUR\"\npaid = true\n")).unwrap();
        assert!(twice_in(&dir).is_empty());
        let all = all_in(&dir);
        assert_eq!((all.len(), all[0].status(&all)), (1, Status::Paid));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A format-1 record (before November 2026) reads as an issued invoice
    /// of kind 380 with time lines, and is never written: Paid goes into this
    /// device's life file; its own `paid`, an older Sioul's word, reads as a
    /// payment from before, which an Undo here takes back (F18).
    #[test]
    fn a_format_1_record_reads_and_stays_as_it_is() {
        let dir = dir("older");
        let tr = crate::i18n::Translator::new("en");
        let older = "number = \"2026-001\"\ndate = \"2026-09-01\"\ndue = \"2026-10-01\"\nproject = \"lumen\"\nproject_title = \"Studio Lumen\"\nclient = \"Studio Lumen\\nAnna Exemple\"\nclient_address = \"1 rue de l’Exemple\\n75002 Paris\"\nsessions = [\"10:t:90\", \"20:t:30\"]\ntotal_cents = 12000\ncurrency = \"EUR\"\n\n[[lines]]\nlabel = \"Build the site\"\nminutes = 120\nrate = 60.0\ncents = 12000\n\n[issuer]\nname = \"Camille Exemple EI\"\naddress = \"2 place de l’Exemple\\n69001 Lyon\"\nsiret = \"732 829 320 00017\"\nvat = \"TVA non applicable, art. 293 B du CGI\"\npayment = \"IBAN FR76 0000\"\n";
        std::fs::write(dir.join("2026-001.toml"), older).unwrap();
        let read = find_in(&dir, "2026-001").unwrap();
        assert_eq!((read.format, read.kind, read.totals.due, read.client.first_line()), (1, Kind::Invoice, 12000, "Studio Lumen"));
        assert_eq!((read.lines[0].minutes(), read.lines[0].price, read.lines[0].amount), (Some(120), 6000, 12000));
        assert_eq!(read.sessions(), ["10:t:90", "20:t:30"]);
        assert_eq!(read.status(&[]), Status::Issued);
        let paid = life::pay_rest(&dir, "desk", "2026-001", "2026-10-02".parse().unwrap(), "transfer", &tr).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("2026-001.toml")).unwrap(), older, "never written");
        let again = find_in(&dir, "2026-001").unwrap();
        assert_eq!((again.format, again.status(&[]), again.paid()), (1, Status::Paid, 12000));
        life::take_back(&dir, "desk", std::slice::from_ref(&paid.id)).unwrap();
        assert_eq!(find_in(&dir, "2026-001").unwrap().status(&[]), Status::Issued);
        // Said paid by an older Sioul: a payment from before, taken back by "Not paid".
        std::fs::write(dir.join("2026-001.toml"), older.replace("total_cents = 12000\n", "total_cents = 12000\npaid = true\n")).unwrap();
        let said = find_in(&dir, "2026-001").unwrap();
        assert_eq!((said.status(&[]), said.payments[0].id.as_str()), (Status::Paid, "older:2026-001"));
        life::unpay(&dir, "desk", "2026-001", &tr).unwrap();
        assert_eq!(find_in(&dir, "2026-001").unwrap().status(&[]), Status::Issued);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
