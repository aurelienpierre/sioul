// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An issued invoice's life, kept apart from its record: the record is never
//! written again once issued (its file, and phase 2's kept PDF and their
//! hashes, stay as they were), and what happens to it later is written in
//! `invoices/life/<device>.toml`, each device's own file, which no other
//! device writes, so that the sync never makes a conflicted copy of it. An
//! older Sioul lists only the folder's top files: it never sees these.
//!
//! Each file holds entries, none naming another, so that nothing dangles
//! when one is taken back:
//! - `[[payment]]`: money received, with its amount: said by hand, its id
//!   made of what it says (`hand_id`: the same payment said on two devices
//!   is one); or a bank line's share for an invoice, its id made of the
//!   line's key, the same on every device (`movements::line_key`), and the
//!   invoice (`bank_id`: the same export taken in on two devices is one);
//! - `[[settled]]`: "Paid", said on a day: what the other payments leave,
//!   read each time, so that Paid said on two devices, or before or after
//!   the bank's line, never counts twice (`settled_id`);
//! - `[[removed]]`: an entry taken back by its id (an Undo, "Not paid",
//!   "Not the same payment"), whichever device wrote it;
//! - `[[sent]]`: the day an invoice was sent.
//!
//! Reading merges every device's file by union, an id once (the first
//! file's word on it), a removal winning; a bank line split across
//! invoices on two devices at once counts as the first file split it. Then
//! each invoice's payments are read (`Lives::apply`): the bank's first,
//! each taken in by a payment said by hand with room for it, of its very
//! amount, else the nearest in days (never one said in cash), which keeps
//! its own day and amount; then "Paid" covers what is left. On a credit
//! note the same entries say money paid back (`Invoice::to_pay_back`).
//!
//! A format-1 record (before November 2026) is never written at all: its
//! own `paid` reads as "Paid" on its due day (`older:<number>`), which an
//! older Sioul ticking Paid changes, and which a removal here takes back.
//!
//! A file of a newer format is read where it can be, never written over,
//! and said (`Lives::newer`).
//!
//! The books write here too (`books`): what a payment in another currency
//! came to in the books' own (`booked`), the books' notices told (a
//! threshold's step), each once whichever device told it, and the
//! declarations you said made (`declared`, the declaration view's
//! "Declared", taken back by a removal as any entry).

use super::record::{Invoice, Kind, Payment};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The life's folder, in the invoices' folder.
pub const FOLDER: &str = "life";
/// The format of the life's files this Sioul writes.
pub const FORMAT: u32 = 1;

/// The id of a format-1 record's own word that it was paid.
pub fn older_id(number: &str) -> String {
    format!("older:{number}")
}

/// A payment said by hand: what it says, and its rank among the same said
/// on this device (two payments alike, both said here, are two).
pub fn hand_id(invoice: &str, date: &str, amount: i64, means: &str, n: u32) -> String {
    format!("hand:{invoice}/{date}/{amount}/{means}/{n}")
}

/// A bank line's share for an invoice: the line's key and the invoice.
pub fn bank_id(line: &str, invoice: &str) -> String {
    format!("bank:{line}/{invoice}")
}

/// "Paid" on an invoice, the `n`th time it is said after being taken back.
pub fn settled_id(invoice: &str, n: u32) -> String {
    format!("settled:{invoice}/{n}")
}

/// What a payment in another currency came to, said at `at` (Unix
/// milliseconds) for the payment `payment`: the books' entry (`booked`).
pub fn booked_id(payment: &str, at: i64) -> String {
    format!("booked:{payment}/{at}")
}

/// One device's file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Book {
    #[serde(default = "one")]
    pub format: u32,
    #[serde(default, rename = "payment")]
    pub payments: Vec<Paid>,
    #[serde(default)]
    pub settled: Vec<Settled>,
    #[serde(default)]
    pub removed: Vec<Removed>,
    #[serde(default)]
    pub sent: Vec<Sent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub booked: Vec<Booked>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub told: Vec<Told>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared: Vec<Declared>,
}

fn one() -> u32 {
    1
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// Money received on an invoice, with its amount (on a credit note: paid back).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paid {
    pub id: String,
    pub invoice: String,
    /// The day it was paid (a processor's, for what came through one).
    pub date: String,
    /// What the client paid, in cents.
    pub amount: i64,
    #[serde(default)]
    pub means: String,
    /// The bank line it came from (`movements::line_key`); empty when said by hand.
    #[serde(default)]
    pub line: String,
    /// What a processor kept of it, in cents.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fee: i64,
}

/// "Paid", said on a day: what the other payments leave.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settled {
    pub id: String,
    pub invoice: String,
    pub date: String,
    #[serde(default)]
    pub means: String,
}

/// An entry taken back (by its id), whichever device wrote it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Removed {
    pub id: String,
}

/// The day an invoice was sent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    pub invoice: String,
    pub date: String,
}

/// What a payment in another currency than the books' came to in theirs
/// (the euros that reached you): the books count it so (`books`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Booked {
    pub id: String,
    pub payment: String,
    /// In cents of the books' currency.
    pub amount: i64,
    /// When it was said, Unix milliseconds: the last said counts.
    #[serde(default)]
    pub at: i64,
}

/// A notice of the books told (a threshold's step: `books::thresholds`), by
/// its key, on its day: told once, whichever device told it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Told {
    pub key: String,
    pub date: String,
}

/// A declaration said made, or said not made, by its key
/// ("declaration:2026-Q4", "des:2026-10"): the declaration view's
/// "Declared" and "Not declared", and their Undo. Each is a dated entry,
/// the latest said (`at`) winning on every device, so that an older word,
/// an Undo among them, never takes back a newer one (the review of phase 3, F16).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declared {
    pub id: String,
    pub key: String,
    /// The day it was declared, as said.
    pub date: String,
    /// Declared (true) or said not declared (false).
    #[serde(default = "yes")]
    pub on: bool,
    /// When it was said, Unix milliseconds: the latest wins.
    #[serde(default)]
    pub at: i64,
}

fn yes() -> bool {
    true
}

/// A word on a declaration, made of what it says: its key, the device
/// that said it and when, so that two devices never give one id.
pub fn declared_id(key: &str, device: &str, at: i64) -> String {
    format!("declared:{key}/{device}/{at}")
}

/// A device's name as a file name: letters, digits and dashes.
fn device_file(dir: &Path, device: &str) -> PathBuf {
    let name: String = device.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
    let name = if name.trim_matches('-').is_empty() { "here".to_string() } else { name };
    dir.join(FOLDER).join(format!("{name}.toml"))
}

/// A file's `format`, as written (1 when it says none).
pub(crate) fn format_of(text: &str) -> u32 {
    text.parse::<toml::Table>().ok().and_then(|t| t.get("format").and_then(toml::Value::as_integer)).map_or(1, |f| u32::try_from(f).unwrap_or(u32::MAX))
}

/// Every device's file of `dir`, in their names' order, and those a newer
/// Sioul wrote (read where they could be).
fn read_books(dir: &Path) -> (Vec<Book>, Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir.join(FOLDER)) else { return (Vec::new(), Vec::new()) };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
    paths.sort();
    let mut books = Vec::new();
    let mut newer = Vec::new();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if format_of(&text) > FORMAT {
            eprintln!("Sioul: {}: written by a newer Sioul, read where it can be, never written here", path.display());
            newer.push(path.clone());
        }
        match toml::from_str::<Book>(&text) {
            Ok(book) => books.push(book),
            Err(e) => eprintln!("Sioul: {}: an invoices' life file that does not read, left as it is: {e}", path.display()),
        }
    }
    (books, newer)
}

/// Every device's file of `dir`.
pub fn books(dir: &Path) -> Vec<Book> {
    read_books(dir).0
}

/// What every device's files say, merged.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lives {
    /// Payments with an amount, by invoice, each id once, a removal winning.
    pub payments: BTreeMap<String, Vec<Paid>>,
    /// "Paid" said and not taken back, by invoice.
    pub settled: BTreeMap<String, Vec<Settled>>,
    /// The day each invoice was sent (the first said).
    pub sent: BTreeMap<String, String>,
    /// Ids taken back.
    pub removed: BTreeSet<String>,
    /// Every id written, taken back or not: a new entry never takes one.
    pub ids: BTreeSet<String>,
    /// Files a newer Sioul wrote: read where they could be, never written here.
    pub newer: Vec<PathBuf>,
    /// What a payment in another currency came to in the books', by the payment's id (the last said).
    pub booked: BTreeMap<String, i64>,
    /// The books' notices told, by key.
    pub told: BTreeSet<String>,
    /// The declarations said made, by key: the first day it was said, its entries not taken back.
    pub declared: BTreeMap<String, String>,
}

impl Lives {
    /// Every device's file of `dir`, merged by union.
    pub fn load(dir: &Path) -> Lives {
        let (books, newer) = read_books(dir);
        Lives { newer, ..Lives::of(&books) }
    }

    pub fn of(books: &[Book]) -> Lives {
        let removed: BTreeSet<String> = books.iter().flat_map(|b| b.removed.iter().map(|r| r.id.clone())).collect();
        let mut ids: BTreeSet<String> = removed.clone();
        // A bank line's shares, as the first file that took it in split it.
        let mut line_book: BTreeMap<&str, usize> = BTreeMap::new();
        for (n, book) in books.iter().enumerate() {
            for p in book.payments.iter().filter(|p| !p.line.is_empty()) {
                line_book.entry(p.line.as_str()).or_insert(n);
            }
        }
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut payments: BTreeMap<String, Vec<Paid>> = BTreeMap::new();
        for (n, book) in books.iter().enumerate() {
            for p in &book.payments {
                ids.insert(p.id.clone());
                let theirs = p.line.is_empty() || line_book.get(p.line.as_str()) == Some(&n);
                if removed.contains(&p.id) || !theirs || !seen.insert(p.id.clone()) {
                    continue;
                }
                payments.entry(p.invoice.clone()).or_default().push(p.clone());
            }
        }
        let mut settled: BTreeMap<String, Vec<Settled>> = BTreeMap::new();
        for s in books.iter().flat_map(|b| &b.settled) {
            ids.insert(s.id.clone());
            if removed.contains(&s.id) || !seen.insert(s.id.clone()) {
                continue;
            }
            settled.entry(s.invoice.clone()).or_default().push(s.clone());
        }
        let mut sent: BTreeMap<String, String> = BTreeMap::new();
        for s in books.iter().flat_map(|b| &b.sent) {
            let at = sent.entry(s.invoice.clone()).or_insert_with(|| s.date.clone());
            if s.date < *at {
                *at = s.date.clone();
            }
        }
        // The last said of each payment.
        ids.extend(books.iter().flat_map(|b| &b.booked).map(|b| b.id.clone()));
        let mut said: Vec<&Booked> = books.iter().flat_map(|b| &b.booked).filter(|b| !removed.contains(&b.id)).collect();
        said.sort_by(|a, b| (a.at, &a.id).cmp(&(b.at, &b.id)));
        let booked: BTreeMap<String, i64> = said.into_iter().map(|b| (b.payment.clone(), b.amount)).collect();
        let told: BTreeSet<String> = books.iter().flat_map(|b| &b.told).map(|t| t.key.clone()).collect();
        // Each declaration's latest word, on every device: declared on its day, or not.
        let mut latest: BTreeMap<String, &Declared> = BTreeMap::new();
        for d in books.iter().flat_map(|b| &b.declared) {
            ids.insert(d.id.clone());
            if removed.contains(&d.id) {
                continue;
            }
            let newer = latest.get(&d.key).is_none_or(|kept| (d.at, &d.id) > (kept.at, &kept.id));
            if newer {
                latest.insert(d.key.clone(), d);
            }
        }
        let declared: BTreeMap<String, String> = latest.into_iter().filter(|(_, d)| d.on).map(|(k, d)| (k, d.date.clone())).collect();
        Lives { payments, settled, sent, removed, ids, newer: Vec::new(), booked, told, declared }
    }

    /// The records given their life: their payments as read (`counted`),
    /// the day they were sent. Invoices first, then credit notes, whose
    /// money to pay back follows what their invoice was paid.
    pub fn apply(&self, records: &mut [Invoice]) {
        let view: &[Invoice] = records;
        // Each invoice's credit notes, by day: "Paid" covers what it asked the day it was said.
        let mut credits: BTreeMap<String, Vec<(String, i64)>> = BTreeMap::new();
        for note in view.iter().filter(|c| c.kind == Kind::CreditNote && !c.number.is_empty()) {
            if let Some(cited) = &note.corrects {
                credits.entry(cited.number.clone()).or_default().push((note.date.clone(), note.totals.due));
            }
        }
        for invoice in records.iter_mut().filter(|i| !i.unreadable && i.kind != Kind::CreditNote) {
            let due = invoice.totals.due;
            let notes = credits.get(&invoice.number).cloned().unwrap_or_default();
            let asked = move |day: &str| due - notes.iter().filter(|(on, _)| on.as_str() <= day).map(|(_, amount)| amount).sum::<i64>();
            invoice.payments = self.counted(invoice, &asked);
            invoice.sent = self.sent.get(&invoice.number).cloned().unwrap_or_default();
        }
        let view: &[Invoice] = records;
        let shares: Vec<(usize, i64)> = view.iter().enumerate().filter(|(_, r)| !r.unreadable && r.kind == Kind::CreditNote).map(|(n, r)| (n, r.pay_back_share(view))).collect();
        for (n, share) in shares {
            let note = &mut records[n];
            note.payments = self.counted(note, &|_: &str| share);
            note.sent = self.sent.get(&note.number).cloned().unwrap_or_default();
        }
    }

    /// One invoice's payments as read: the bank's, each taken in by a
    /// payment said by hand with room for it (of its very amount first,
    /// else the nearest in days; never one in cash), whose day and amount
    /// stay; what is left of a bank line beyond them counts as its own;
    /// then "Paid" (a format-1 record's own word among them) covers what is
    /// left of what the invoice asked on the first day it was said
    /// (`asked`: its amount less the credit notes issued by then; on a
    /// credit note, what it pays back), on that day.
    fn counted(&self, invoice: &Invoice, asked: &dyn Fn(&str) -> i64) -> Vec<Payment> {
        let all: &[Paid] = self.payments.get(&invoice.number).map_or(&[], Vec::as_slice);
        let mut bank: Vec<&Paid> = all.iter().filter(|p| !p.line.is_empty()).collect();
        bank.sort_by(|a, b| (a.date.as_str(), a.id.as_str()).cmp(&(b.date.as_str(), b.id.as_str())));
        let hand: Vec<&Paid> = all.iter().filter(|p| p.line.is_empty()).collect();
        let mut room: Vec<i64> = hand.iter().map(|h| if h.means == "cash" { 0 } else { h.amount.max(0) }).collect();
        let mut found: Vec<String> = vec![String::new(); hand.len()];
        let mut out = Vec::new();
        for b in bank {
            let mut rest = b.amount;
            while rest > 0 {
                let open: Vec<usize> = (0..hand.len()).filter(|i| room[*i] > 0).collect();
                let near = |i: &&usize| (days_apart(&hand[**i].date, &b.date), hand[**i].id.as_str());
                let Some(&i) = open.iter().filter(|i| room[**i] == rest).min_by_key(near).or_else(|| open.iter().min_by_key(near)) else { break };
                let take = room[i].min(rest);
                room[i] -= take;
                rest -= take;
                if found[i].is_empty() {
                    found[i] = b.line.clone();
                }
            }
            if rest > 0 {
                out.push(Payment { id: b.id.clone(), date: b.date.clone(), amount: rest, means: if b.means.is_empty() { "transfer".into() } else { b.means.clone() }, line: b.line.clone(), fee: b.fee });
            }
        }
        for (h, line) in hand.iter().zip(found) {
            out.push(Payment { id: h.id.clone(), date: h.date.clone(), amount: h.amount, means: h.means.clone(), line, fee: 0 });
        }
        let mut said: Vec<Settled> = self.settled.get(&invoice.number).cloned().unwrap_or_default();
        if invoice.older.as_ref().is_some_and(|o| o.paid) && !self.removed.contains(&older_id(&invoice.number)) {
            let date = if invoice.due.is_empty() { invoice.date.clone() } else { invoice.due.clone() };
            said.push(Settled { id: older_id(&invoice.number), invoice: invoice.number.clone(), date, means: String::new() });
        }
        if let Some(first) = said.iter().min_by(|a, b| (a.date.as_str(), a.id.as_str()).cmp(&(b.date.as_str(), b.id.as_str()))) {
            let left = asked(&first.date) - out.iter().map(|p| p.amount).sum::<i64>();
            if left > 0 {
                out.push(Payment { id: first.id.clone(), date: first.date.clone(), amount: left, means: first.means.clone(), line: String::new(), fee: 0 });
            }
        }
        out.sort_by(|a, b| (a.date.as_str(), a.id.as_str()).cmp(&(b.date.as_str(), b.id.as_str())));
        out
    }

    /// What a new bank line cannot pay again on `number`: what the bank's
    /// lines paid it, and what was said paid in cash, which no bank line
    /// ever takes in. Payments said by hand otherwise, and "Paid", are no
    /// bound: a bank line is their evidence.
    pub fn bound(&self, number: &str) -> i64 {
        self.payments.get(number).map_or(0, |all| all.iter().filter(|p| !p.line.is_empty() || p.means == "cash").map(|p| p.amount).sum())
    }

    /// Whether `id` was ever written, taken back or not.
    pub fn knows(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    /// The next rank of entries whose ids are `prefix` and a number.
    pub fn next_rank(&self, prefix: &str) -> u32 {
        self.ids.iter().filter_map(|id| id.strip_prefix(prefix)?.parse::<u32>().ok()).max().unwrap_or(0) + 1
    }

    /// The ids of what was said by hand on `number`, still standing: its
    /// payments with no bank line, "Paid", a format-1 record's own word.
    pub fn said_by_hand(&self, number: &str, older_paid: bool) -> Vec<String> {
        let payments = self.payments.get(number).into_iter().flatten().filter(|p| p.line.is_empty()).map(|p| p.id.clone());
        let settled = self.settled.get(number).into_iter().flatten().map(|s| s.id.clone());
        let older = (older_paid && !self.removed.contains(&older_id(number))).then(|| older_id(number));
        payments.chain(settled).chain(older).collect()
    }

    /// The ids of the payments the bank line `line` made, still standing, on any invoice.
    pub fn of_line(&self, line: &str) -> Vec<String> {
        self.payments.values().flatten().filter(|p| p.line == line).map(|p| p.id.clone()).collect()
    }
}

/// How many days apart two days are (a day that does not read: far).
fn days_apart(a: &str, b: &str) -> i64 {
    match (a.parse::<jiff::civil::Date>(), b.parse::<jiff::civil::Date>()) {
        (Ok(a), Ok(b)) => i64::from((a - b).get_days().abs()),
        _ => i64::MAX,
    }
}

/// When the latest word on the declaration `key` was said, among every
/// device's life files as they are here (0 for none).
pub fn latest_declared(dir: &Path, key: &str) -> i64 {
    read_books(dir).0.iter().flat_map(|b| &b.declared).filter(|d| d.key == key).map(|d| d.at).max().unwrap_or(0)
}

/// This device's own file changed by `change`, under its lock: read,
/// changed, written whole (`write_whole`). No other device writes it; one
/// a newer Sioul wrote is never written over.
pub fn change_own<R>(dir: &Path, device: &str, change: impl FnOnce(&mut Book) -> R) -> Result<R, String> {
    let path = device_file(dir, device);
    crate::filelock::with_lock(&path, || {
        let mut book = match std::fs::read_to_string(&path) {
            Ok(text) if format_of(&text) > FORMAT => return Err(format!("{}: written by a newer Sioul; update Sioul on this device to change it.", path.display())),
            Ok(text) => toml::from_str::<Book>(&text).map_err(|e| format!("{}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Book::default(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let out = change(&mut book);
        book.format = FORMAT;
        let text = toml::to_string(&book).map_err(|e| e.to_string())?;
        super::write_whole(&path, &text)?;
        crate::changes::said(crate::changes::Kind::Issued);
        Ok(out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoice::record::Totals;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-lives-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn paid(invoice: &str, date: &str, amount: i64, line: &str) -> Paid {
        let id = if line.is_empty() { hand_id(invoice, date, amount, "transfer", 1) } else { bank_id(line, invoice) };
        Paid { id, invoice: invoice.into(), date: date.into(), amount, means: "transfer".into(), line: line.into(), fee: 0 }
    }

    fn settled(invoice: &str, date: &str) -> Settled {
        Settled { id: settled_id(invoice, 1), invoice: invoice.into(), date: date.into(), means: "transfer".into() }
    }

    fn read(books: &[Book], due: i64) -> Invoice {
        let mut records = vec![Invoice { number: "2026-005".into(), kind: Kind::Invoice, totals: Totals { due, ..Totals::default() }, ..Invoice::default() }];
        Lives::of(books).apply(&mut records);
        records.remove(0)
    }

    /// One payment said on two devices before they exchange counts once
    /// (the second review, G1): Paid on the phone and the bank's line on
    /// the desk; Paid on both; the same export on both; the same payment
    /// typed on both.
    #[test]
    fn a_payment_counts_once_whatever_the_devices_did() {
        let phone = Book { settled: vec![settled("2026-005", "2026-11-10")], ..Book::default() };
        let desk = Book { payments: vec![paid("2026-005", "2026-11-12", 120_000, "acct/2026-11-12/120000/ab12")], ..Book::default() };
        let both = read(&[desk.clone(), phone.clone()], 120_000);
        assert_eq!((both.paid(), both.payments.len(), both.payments[0].line.as_str()), (120_000, 1, "acct/2026-11-12/120000/ab12"));
        assert_eq!(read(&[phone.clone(), phone.clone()], 120_000).paid(), 120_000, "Paid on two devices");
        assert_eq!(read(&[desk.clone(), desk.clone()], 120_000).paid(), 120_000, "the same export taken in twice");
        let typed = Book { payments: vec![paid("2026-005", "2026-11-10", 50_000, "")], ..Book::default() };
        assert_eq!(read(&[typed.clone(), typed], 120_000).paid(), 50_000, "the same payment typed on two devices");
        // A bank line split on two devices at once: as the first file split it.
        let split = Book { payments: vec![Paid { amount: 70_000, ..paid("2026-005", "2026-11-12", 0, "acct/2026-11-12/120000/ab12") }, paid("2026-006", "2026-11-12", 50_000, "acct/2026-11-12/120000/ab12")], ..Book::default() };
        let lives = Lives::of(&[desk, split]);
        assert!(!lives.payments.contains_key("2026-006"), "the desk's split stands");
    }

    /// A bank line taken in on one device while "Paid" is taken back on
    /// another (G3): the bank's payment stays, nothing dangles. An older
    /// Sioul's Paid on a format-1 invoice the bank paid (G7): once.
    #[test]
    fn nothing_dangles_when_an_entry_is_taken_back() {
        let phone = Book { settled: vec![settled("2026-005", "2026-11-10")], removed: vec![Removed { id: settled_id("2026-005", 1) }], ..Book::default() };
        let desk = Book { payments: vec![paid("2026-005", "2026-11-12", 80_000, "acct/2026-11-12/80000/cd34")], ..Book::default() };
        let read_back = read(&[desk.clone(), phone], 80_000);
        assert_eq!((read_back.paid(), read_back.payments[0].line.as_str()), (80_000, "acct/2026-11-12/80000/cd34"));
        let mut older = vec![Invoice { number: "2026-005".into(), kind: Kind::Invoice, due: "2026-10-01".into(), totals: Totals { due: 80_000, ..Totals::default() }, older: Some(crate::invoice::record::Older { paid: true, ..Default::default() }), ..Invoice::default() }];
        Lives::of(std::slice::from_ref(&desk)).apply(&mut older);
        assert_eq!((older[0].paid(), older[0].payments.len()), (80_000, 1));
    }

    /// The bank's line and payments said by hand in instalments (G5): each
    /// line taken in by the one of its very amount, else the nearest in
    /// days; the hand payment's day kept; cash never taken in.
    #[test]
    fn a_bank_line_is_the_evidence_of_its_own_amount() {
        let hand = Book { payments: vec![paid("2026-005", "2026-10-01", 20_000, ""), paid("2026-005", "2026-11-03", 30_000, "")], ..Book::default() };
        let bank = Book { payments: vec![paid("2026-005", "2026-11-04", 30_000, "acct/2026-11-04/30000/ef56")], ..Book::default() };
        let read_back = read(&[bank, hand], 50_000);
        let days: Vec<(&str, i64, &str)> = read_back.payments.iter().map(|p| (p.date.as_str(), p.amount, p.line.as_str())).collect();
        assert_eq!(days, [("2026-10-01", 20_000, ""), ("2026-11-03", 30_000, "acct/2026-11-04/30000/ef56")]);
        // Paid by hand at once (€500), then two transfers: the hand payment's evidence both.
        let whole = Book { payments: vec![paid("2026-005", "2026-11-01", 50_000, "")], ..Book::default() };
        let two = Book { payments: vec![paid("2026-005", "2026-11-02", 30_000, "acct/a"), paid("2026-005", "2026-11-05", 20_000, "acct/b")], ..Book::default() };
        assert_eq!(read(&[two, whole], 50_000).paid(), 50_000);
        // In cash: a transfer is another payment.
        let cash = Book { payments: vec![Paid { means: "cash".into(), ..paid("2026-005", "2026-11-01", 20_000, "") }], ..Book::default() };
        let transfer = Book { payments: vec![paid("2026-005", "2026-11-02", 20_000, "acct/c")], ..Book::default() };
        assert_eq!(read(&[transfer, cash], 50_000).paid(), 40_000);
    }

    /// "Declared" and "Not declared" are dated words, the latest winning on
    /// every device (the review of phase 3, F16): an older Undo said on the
    /// phone never takes back a newer "Declared" said on the desk.
    #[test]
    fn the_latest_word_on_a_declaration_wins() {
        let word = |device: &str, on: bool, at: i64| Declared { id: declared_id("declaration:2026-Q3", device, at), key: "declaration:2026-Q3".into(), date: "2026-10-20".into(), on, at };
        let desk = Book { declared: vec![word("desk-1", true, 2_000)], ..Book::default() };
        let phone = Book { declared: vec![word("phone-2", false, 1_000)], ..Book::default() };
        assert_eq!(Lives::of(&[phone.clone(), desk.clone()]).declared.get("declaration:2026-Q3").map(String::as_str), Some("2026-10-20"), "the older Undo loses");
        let later = Book { declared: vec![word("phone-2", false, 1_000), word("phone-2", false, 3_000)], ..Book::default() };
        assert!(Lives::of(&[desk.clone(), later]).declared.is_empty(), "the newer Not declared wins");
        // A word of an older Sioul (no time, declared) still counts, and its removal still takes it back.
        let older = Book { declared: vec![Declared { id: "declared:declaration:2026-Q3".into(), key: "declaration:2026-Q3".into(), date: "2026-10-05".into(), on: true, at: 0 }], ..Book::default() };
        let read: Declared = toml::from_str("id = \"x\"\nkey = \"k\"\ndate = \"2026-10-05\"\n").unwrap();
        assert!(read.on, "an older Sioul's word says declared");
        assert_eq!(Lives::of(std::slice::from_ref(&older)).declared.len(), 1);
        let taken = Book { removed: vec![Removed { id: "declared:declaration:2026-Q3".into() }], ..older };
        assert!(Lives::of(&[taken]).declared.is_empty());
    }

    /// A word said after reading another is newer than it, whatever the
    /// clocks say (the second review of phase 3, R10): the desk's clock an
    /// hour ahead says Not declared at "11:00"; the phone says Declared at
    /// 10:30, after it: declared.
    #[test]
    fn a_clock_that_runs_ahead_never_wins_over_a_later_word() {
        let dir = dir("clock");
        let hour = 3_600_000;
        let ten: i64 = 1_800_000_000_000;
        let q3 = "declaration:2026-Q3";
        let day: jiff::civil::Date = "2026-10-20".parse().unwrap();
        crate::books::mark_declared(&dir, "desk-1", q3, day, false, ten + hour).unwrap();
        crate::books::mark_declared(&dir, "phone-2", q3, day, true, ten + hour / 2).unwrap();
        assert_eq!(Lives::load(&dir).declared.get(q3).map(String::as_str), Some("2026-10-20"));
        assert_eq!(latest_declared(&dir, q3), ten + hour + 1, "one more than the latest read");
        // Its Undo on the same phone, a moment later by its own clock, is newer again.
        crate::books::mark_declared(&dir, "phone-2", q3, day, false, ten + hour / 2 + 5).unwrap();
        assert!(Lives::load(&dir).declared.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two devices write their own files; a newer format is read, never written over (G13).
    #[test]
    fn each_device_writes_its_own_life() {
        let dir = dir("own");
        change_own(&dir, "desk-1", |b| b.payments.push(paid("2026-005", "2026-11-10", 50_000, ""))).unwrap();
        change_own(&dir, "phone-2", |b| b.settled.push(settled("2026-005", "2026-11-11"))).unwrap();
        let lives = Lives::load(&dir);
        assert_eq!((lives.payments["2026-005"].len(), lives.settled["2026-005"].len()), (1, 1));
        let files = std::fs::read_dir(dir.join(FOLDER)).unwrap().filter_map(Result::ok).filter(|e| e.path().extension().is_some_and(|x| x == "toml")).count();
        assert_eq!(files, 2, "one file per device");
        let laptop = dir.join(FOLDER).join("laptop-3.toml");
        std::fs::write(&laptop, "format = 2\n\n[[payment]]\nid = \"x\"\ninvoice = \"2026-006\"\ndate = \"2026-11-12\"\namount = 100\nwhat = \"something new\"\n").unwrap();
        let lives = Lives::load(&dir);
        assert_eq!((lives.newer.len(), lives.payments.contains_key("2026-006")), (1, true), "read where it can be, and said");
        assert!(change_own(&dir, "laptop-3", |b| b.sent.clear()).is_err(), "never written over");
        assert!(std::fs::read_to_string(&laptop).unwrap().contains("something new"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
