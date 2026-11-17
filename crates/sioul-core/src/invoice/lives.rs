// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An issued invoice's life, kept apart from its record: the record is never
//! written again once issued (its file, and phase 2's kept PDF and their
//! hashes, stay as they were), and what happens to it later is written in
//! `invoices/life/<device>.toml`, each device's own file, which no other
//! device writes, so that the sync never makes a conflicted copy of it. An
//! older Sioul lists only the folder's top files: it never sees these.
//!
//! Each file holds entries, each with its own id: payments, a bank line
//! taken in as a payment's evidence, the day an invoice was sent, and
//! removals (an Undo, a "Not paid"), which name the entry they take back,
//! whichever device wrote it. Reading merges every device's file by union;
//! a removal wins over what it removes.
//!
//! A format-1 record (before November 2026) is never written at all: its
//! own `paid` reads as a payment from before (`older:<number>`), which an
//! older Sioul ticking Paid changes, and which a removal here takes back.
//!
//! The books write here too (`books`): what a payment in another currency
//! came to in the books' own (`booked`), and the books' notices told (a
//! threshold's step), each once whichever device told it.

use super::record::{Invoice, Payment};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The life's folder, in the invoices' folder.
pub const FOLDER: &str = "life";

/// The id of a format-1 record's own word that it was paid.
pub fn older_id(number: &str) -> String {
    format!("older:{number}")
}

/// One device's file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Book {
    #[serde(default = "one")]
    pub format: u32,
    #[serde(default, rename = "payment")]
    pub payments: Vec<Paid>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub removed: Vec<Removed>,
    #[serde(default)]
    pub sent: Vec<Sent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub booked: Vec<Booked>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub told: Vec<Told>,
}

fn one() -> u32 {
    1
}

/// A payment on an invoice.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paid {
    pub id: String,
    pub invoice: String,
    pub date: String,
    pub amount: i64,
    #[serde(default)]
    pub means: String,
    /// The bank line's movement it came from; empty when said by hand.
    #[serde(default)]
    pub movement: String,
}

/// A bank line taken in as the evidence of a payment said by hand: the
/// payment is that line's money, never a second payment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub payment: String,
    pub movement: String,
    /// The bank's day.
    pub date: String,
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

/// A device's name as a file name: letters, digits and dashes.
fn device_file(dir: &Path, device: &str) -> PathBuf {
    let name: String = device.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
    let name = if name.trim_matches('-').is_empty() { "here".to_string() } else { name };
    dir.join(FOLDER).join(format!("{name}.toml"))
}

/// Every device's file of `dir`.
pub fn books(dir: &Path) -> Vec<Book> {
    let Ok(entries) = std::fs::read_dir(dir.join(FOLDER)) else { return Vec::new() };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            match toml::from_str::<Book>(&text) {
                Ok(book) => Some(book),
                Err(e) => {
                    eprintln!("Sioul: {}: an invoices' life file that does not read, left as it is: {e}", p.display());
                    None
                }
            }
        })
        .collect()
}

/// What every device's files say, merged.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lives {
    /// Payments by invoice, each once, in the order written.
    pub payments: BTreeMap<String, Vec<Payment>>,
    /// The day each invoice was sent (the first said).
    pub sent: BTreeMap<String, String>,
    /// Ids taken back.
    pub removed: BTreeSet<String>,
    /// Each payment's evidence, by the payment's id: (evidence id, movement, bank's day).
    pub evidence: BTreeMap<String, (String, String, String)>,
    /// What a payment in another currency came to in the books', by the payment's id (the last said).
    pub booked: BTreeMap<String, i64>,
    /// The books' notices told, by key.
    pub told: BTreeSet<String>,
}

impl Lives {
    /// Every device's file of `dir`, merged by union.
    pub fn load(dir: &Path) -> Lives {
        Lives::of(&books(dir))
    }

    pub fn of(books: &[Book]) -> Lives {
        let removed: BTreeSet<String> = books.iter().flat_map(|b| b.removed.iter().map(|r| r.id.clone())).collect();
        let mut evidence: BTreeMap<String, (String, String, String)> = BTreeMap::new();
        for e in books.iter().flat_map(|b| &b.evidence).filter(|e| !removed.contains(&e.id)) {
            evidence.entry(e.payment.clone()).or_insert_with(|| (e.id.clone(), e.movement.clone(), e.date.clone()));
        }
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut payments: BTreeMap<String, Vec<Payment>> = BTreeMap::new();
        for p in books.iter().flat_map(|b| &b.payments) {
            if removed.contains(&p.id) || !seen.insert(p.id.clone()) {
                continue;
            }
            let mut payment = Payment { id: p.id.clone(), date: p.date.clone(), amount: p.amount, means: p.means.clone(), movement: p.movement.clone() };
            // A payment said by hand, its bank line found since: that line's day and movement.
            if payment.movement.is_empty()
                && let Some((_, movement, date)) = evidence.get(&p.id)
            {
                payment.movement = movement.clone();
                if !date.is_empty() {
                    payment.date = date.clone();
                }
            }
            payments.entry(p.invoice.clone()).or_default().push(payment);
        }
        let mut sent: BTreeMap<String, String> = BTreeMap::new();
        for s in books.iter().flat_map(|b| &b.sent) {
            let at = sent.entry(s.invoice.clone()).or_insert_with(|| s.date.clone());
            if s.date < *at {
                *at = s.date.clone();
            }
        }
        // The last said of each payment.
        let mut said: Vec<&Booked> = books.iter().flat_map(|b| &b.booked).filter(|b| !removed.contains(&b.id)).collect();
        said.sort_by(|a, b| (a.at, &a.id).cmp(&(b.at, &b.id)));
        let booked: BTreeMap<String, i64> = said.into_iter().map(|b| (b.payment.clone(), b.amount)).collect();
        let told: BTreeSet<String> = books.iter().flat_map(|b| &b.told).map(|t| t.key.clone()).collect();
        Lives { payments, sent, removed, evidence, booked, told }
    }

    /// The records given their life: their payments, the day they were
    /// sent; a format-1 record's own "paid", unless taken back, as a
    /// payment from before of its whole amount.
    pub fn apply(&self, records: &mut [Invoice]) {
        for invoice in records.iter_mut().filter(|i| !i.unreadable) {
            let mut payments = Vec::new();
            if let Some(older) = invoice.older.as_ref().filter(|o| o.paid) {
                let id = older_id(&invoice.number);
                if !self.removed.contains(&id) {
                    let _ = older;
                    let date = if invoice.due.is_empty() { invoice.date.clone() } else { invoice.due.clone() };
                    let mut payment = Payment { id: id.clone(), date, amount: invoice.totals.due, means: String::new(), movement: String::new() };
                    if let Some((_, movement, day)) = self.evidence.get(&id) {
                        payment.movement = movement.clone();
                        if !day.is_empty() {
                            payment.date = day.clone();
                        }
                    }
                    payments.push(payment);
                }
            }
            payments.extend(self.payments.get(&invoice.number).cloned().unwrap_or_default());
            invoice.payments = payments;
            invoice.sent = self.sent.get(&invoice.number).cloned().unwrap_or_default();
        }
    }
}

/// This device's own file changed by `change`, under its lock: read,
/// changed, written whole. No other device writes it.
pub fn change_own<R>(dir: &Path, device: &str, change: impl FnOnce(&mut Book) -> R) -> Result<R, String> {
    let path = device_file(dir, device);
    crate::filelock::with_lock(&path, || {
        let mut book = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<Book>(&text).map_err(|e| format!("{}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Book::default(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let out = change(&mut book);
        book.format = 1;
        let text = toml::to_string(&book).map_err(|e| e.to_string())?;
        super::write_whole(&path, &text)?;
        Ok(out)
    })
}

/// A new entry's id.
pub fn new_id() -> String {
    crate::ids::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two devices write their own files; reading merges them; a removal
    /// written on one device takes back the other's payment; a bank line
    /// taken in as evidence gives a hand payment its movement and day.
    #[test]
    fn each_device_writes_its_own_life() {
        let dir = std::env::temp_dir().join(format!("sioul-lives-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paid = |id: &str, amount: i64, movement: &str| Paid { id: id.into(), invoice: "2026-005".into(), date: "2026-11-10".into(), amount, means: "transfer".into(), movement: movement.into() };
        change_own(&dir, "desk-1", |b| b.payments.push(paid("a", 50_000, ""))).unwrap();
        change_own(&dir, "phone-2", |b| {
            b.payments.push(paid("b", 20_000, "sioul:movement/x"));
            b.payments.push(paid("a", 50_000, ""));
        })
        .unwrap();
        let lives = Lives::load(&dir);
        assert_eq!(lives.payments["2026-005"].iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["a", "b"], "each once");
        change_own(&dir, "phone-2", |b| {
            b.removed.push(Removed { id: "b".into() });
            b.evidence.push(Evidence { id: "e".into(), payment: "a".into(), movement: "sioul:movement/y".into(), date: "2026-11-12".into() });
        })
        .unwrap();
        let lives = Lives::load(&dir);
        let a = &lives.payments["2026-005"];
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].movement.as_str(), a[0].date.as_str()), ("sioul:movement/y", "2026-11-12"));
        let files = std::fs::read_dir(dir.join(FOLDER)).unwrap().filter_map(Result::ok).filter(|e| e.path().extension().is_some_and(|x| x == "toml")).count();
        assert_eq!(files, 2, "one file per device");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
