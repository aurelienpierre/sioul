// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An invoice's record, format 2: EN 16931's business terms (BT-xx) as its
//! fields, so that one record prints the PDF in any language and becomes an
//! e-invoice without translation (docs/projects.md, "Invoices").
//!
//! Money is integer cents, quantities thousandths, rates basis points: no
//! float in a sum. A record is a draft until it is issued: a draft has no
//! number and changes freely (`invoices/drafts/<id>.toml`); issued, it is
//! kept under its number (`invoices/<number>.toml`), its content frozen, only
//! its life written after: payments, the day it was sent, the kept file.
//!
//! Format-1 records (before November 2026: no `format` key) read as issued
//! invoices of kind 380 with time lines, and are never rewritten: their
//! life is written in place, every other line kept as it is.

use crate::money::Money;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};

/// The record's format: 2 since November 2026.
pub const FORMAT: u32 = 2;

/// The document's kind, UNTDID 1001 as EN 16931 uses it (BT-3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub enum Kind {
    /// 380, a commercial invoice.
    #[default]
    Invoice,
    /// 381, a credit note: it cites the invoice it credits.
    CreditNote,
    /// 386, a deposit (prepayment) invoice: a final invoice deducts it.
    Deposit,
}

impl Kind {
    pub fn code(self) -> u16 {
        match self {
            Kind::Invoice => 380,
            Kind::CreditNote => 381,
            Kind::Deposit => 386,
        }
    }

    /// Its name in messages' ids: "invoice", "credit-note", "deposit".
    pub fn id(self) -> &'static str {
        match self {
            Kind::Invoice => "invoice",
            Kind::CreditNote => "credit-note",
            Kind::Deposit => "deposit",
        }
    }

    pub fn named(name: &str) -> Option<Kind> {
        match name {
            "invoice" | "380" => Some(Kind::Invoice),
            "credit-note" | "credit" | "381" => Some(Kind::CreditNote),
            "deposit" | "386" => Some(Kind::Deposit),
            _ => None,
        }
    }
}

impl From<Kind> for u16 {
    fn from(kind: Kind) -> u16 {
        kind.code()
    }
}

impl TryFrom<u16> for Kind {
    type Error = String;
    fn try_from(code: u16) -> Result<Kind, String> {
        match code {
            380 => Ok(Kind::Invoice),
            381 => Ok(Kind::CreditNote),
            386 => Ok(Kind::Deposit),
            other => Err(format!("no invoice kind {other}")),
        }
    }
}

/// Where an invoice stands, read from its payments and its credit notes, never typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Draft,
    Issued,
    Sent,
    PartlyPaid,
    Paid,
    Credited,
    /// An issued credit note: never paid nor unpaid, it credits an invoice.
    CreditNote,
    /// A credit note for an invoice paid beyond what it now asks: that
    /// money is to be paid back, until a "paid back" says it was.
    ToPayBack,
}

impl Status {
    pub fn id(self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Issued => "issued",
            Status::Sent => "sent",
            Status::PartlyPaid => "partly-paid",
            Status::Paid => "paid",
            Status::Credited => "credited",
            Status::CreditNote => "credit-note",
            Status::ToPayBack => "to-pay-back",
        }
    }

    pub fn named(name: &str) -> Option<Status> {
        [Status::Draft, Status::Issued, Status::Sent, Status::PartlyPaid, Status::Paid, Status::Credited, Status::CreditNote, Status::ToPayBack].into_iter().find(|s| s.id() == name)
    }

    /// Settled: nothing left to pay (a credit note never has anything to pay).
    pub fn settled(self) -> bool {
        matches!(self, Status::Paid | Status::Credited | Status::CreditNote)
    }
}

/// A postal address, structured (BG-5, BG-8, BG-15), with its ISO 3166-1 country.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Address {
    /// Street lines.
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub postcode: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub country: String,
}

impl Address {
    /// An address written on lines, as an envelope has it: kept as lines.
    pub fn from_text(text: &str) -> Address {
        Address { lines: text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect(), ..Address::default() }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.trim().is_empty()) && self.postcode.trim().is_empty() && self.city.trim().is_empty()
    }

    /// The address on lines: its street, then postcode and city; its
    /// country, when it is not `home`'s, by its name in `language` (G17).
    pub fn text(&self, home: &str, language: &str) -> String {
        let mut out: Vec<String> = self.lines.iter().filter(|l| !l.trim().is_empty()).cloned().collect();
        let town = [self.postcode.trim(), self.city.trim()].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
        if !town.is_empty() {
            out.push(town);
        }
        if !self.country.trim().is_empty() && !self.country.trim().eq_ignore_ascii_case(home.trim()) {
            out.push(crate::clients::country(&self.country).map_or_else(|| self.country.trim().to_uppercase(), |c| c.name(language).to_string()));
        }
        out.join("\n")
    }
}

/// An identifier: its kind (a preset's: "siren", "vat", "steuernummer"), its
/// value, and whether it is printed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Identifier {
    pub kind: String,
    pub value: String,
    #[serde(default = "printed")]
    pub printed: bool,
}

fn printed() -> bool {
    true
}

/// A bank account to pay into (BG-17).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Bank {
    #[serde(default)]
    pub holder: String,
    #[serde(default)]
    pub iban: String,
    #[serde(default)]
    pub bic: String,
}

/// Who sends the invoice, as it was at issue (BG-4).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Issuer {
    /// The country's preset: "fr", "de", "elsewhere"…
    #[serde(default)]
    pub preset: String,
    /// Its legal form, a preset's: "ei", "sasu", "sole"…
    #[serde(default)]
    pub legal_form: String,
    /// Its income regime for the books: "micro", "reel"; "" where none applies.
    #[serde(default)]
    pub regime: String,
    /// As printed: « EI » after it for a French entrepreneur individuel.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub trade_name: String,
    #[serde(default)]
    pub address: Address,
    #[serde(default)]
    pub ids: Vec<Identifier>,
    /// Its VAT scheme on the day of issue: "none", "franchise", "liable".
    #[serde(default)]
    pub scheme: String,
    /// The franchise's mention printed, when under it.
    #[serde(default)]
    pub mention: String,
    /// Options printed: "debits" (VAT on debits, France).
    #[serde(default)]
    pub options: Vec<String>,
    /// A company's share capital, as printed ("1 000 €").
    #[serde(default)]
    pub capital: String,
    #[serde(default)]
    pub banks: Vec<Bank>,
    /// Payment details in its own words, printed under the lines.
    #[serde(default)]
    pub payment: String,
    /// Professional insurance: insurer, contact, coverage, as printed.
    #[serde(default)]
    pub insurance: String,
    #[serde(default)]
    pub email: String,
}

impl Issuer {
    /// The identifier of `kind`, when it has one.
    pub fn id(&self, kind: &str) -> Option<&str> {
        self.ids.iter().find(|i| i.kind == kind && !i.value.trim().is_empty()).map(|i| i.value.as_str())
    }
}

/// Who the invoice is for, as it was at issue (BG-7).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Client {
    /// "business", "consumer", "public".
    #[serde(default = "business")]
    pub kind: String,
    /// As printed; an organisation and a person on two lines.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: Address,
    /// Where goods go, when not the address (BG-15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<Address>,
    #[serde(default)]
    pub ids: Vec<Identifier>,
    /// Where its invoices are mailed.
    #[serde(default)]
    pub email: String,
}

fn business() -> String {
    "business".into()
}

impl Client {
    pub fn id(&self, kind: &str) -> Option<&str> {
        self.ids.iter().find(|i| i.kind == kind && !i.value.trim().is_empty()).map(|i| i.value.as_str())
    }

    /// Billed as a business (or a public body): the late-payment lines apply.
    pub fn is_business(&self) -> bool {
        self.kind != "consumer"
    }

    /// Its first line: the organisation, else the person.
    pub fn first_line(&self) -> &str {
        self.name.lines().next().unwrap_or("")
    }
}

/// What a line sells: goods or a service (France's nature of operations).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Nature {
    Goods,
    #[default]
    Service,
}

/// One line (BG-25).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// Its own id in its draft, given once (`life::line_ids`) and kept at
    /// issue: the editor names a line by it, never by its place, so that a
    /// change made elsewhere meanwhile is never written over, and a line of
    /// time keeps its time whatever moves around it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// BT-153.
    pub description: String,
    /// BT-154.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub details: String,
    /// BT-129, in thousandths of `unit`.
    pub quantity: i64,
    /// BT-130, UN/ECE Recommendation 20: "HUR" hour, "MIN" minute, "DAY",
    /// "C62" one, "MON" month, "KMT" kilometre, "LS" lump sum.
    pub unit: String,
    /// BT-146, in cents, for `base` units.
    pub price: i64,
    /// BT-149: how many units the price is for (60 minutes for an hourly rate); 1 by default.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub base: i64,
    /// An amount off this line, in cents (BT-136).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub discount: i64,
    /// BT-151, UNTDID 5305: "S", "Z", "E", "AE", "K", "G", "O".
    pub category: String,
    /// BT-152, in basis points (2000 = 20 %).
    #[serde(default)]
    pub rate: i64,
    /// Why the line is exempt or not taxed here, when it is (BT-120 for its
    /// category): the preset's words for where its VAT is due, or your own.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub exemption: String,
    /// EN 16931's code for that reason (BT-121): "VATEX-EU-AE".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason_code: String,
    /// Its tax set by you (or kept from the invoice a credit note credits):
    /// Sioul never places it again (`life::refresh`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub own_tax: bool,
    #[serde(default)]
    pub nature: Nature,
    /// The activity it is declared in for the books (France: "goods-bic",
    /// "services-bic", "bnc", "cipav"), apart from its nature: a meal eaten
    /// on the spot or a night's lodging is a service for VAT and a sale for
    /// URSSAF and the thresholds. Empty: the issuer's activity for its
    /// nature (`books::line_activity`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub activity: String,
    /// BT-131, in cents: quantity × price / base − discount, to the cent.
    pub amount: i64,
    /// The time it bills, by session key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<String>,
    /// The record of a client's file it bills (`sioul:client/<id>/db/<kind>/<row>`,
    /// docs/clients.md); "" none. A row removed later leaves the line as it is.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub row: String,
}

fn one() -> i64 {
    1
}

fn is_one(n: &i64) -> bool {
    *n == 1
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

impl Line {
    /// A line of time: `minutes` at an hourly `price` in cents.
    pub fn of_time(description: &str, minutes: u32, price: i64, category: &str, rate: i64) -> Line {
        let mut line = Line {
            id: String::new(),
            description: description.to_string(),
            details: String::new(),
            quantity: i64::from(minutes) * 1000,
            unit: "MIN".into(),
            price,
            base: 60,
            discount: 0,
            category: category.to_string(),
            rate,
            exemption: String::new(),
            reason_code: String::new(),
            own_tax: false,
            nature: Nature::Service,
            activity: String::new(),
            amount: 0,
            sessions: Vec::new(),
            row: String::new(),
        };
        line.amount = line.net();
        line
    }

    /// Time, in minutes, when the line is time.
    pub fn minutes(&self) -> Option<u32> {
        (self.unit == "MIN").then(|| u32::try_from(self.quantity / 1000).ok()).flatten()
    }

    /// Its net amount as EN 16931 computes it (BR-LIN): quantity × price ÷
    /// base, rounded half away from zero to the cent, less its discount.
    pub fn net(&self) -> i64 {
        let gross = i128::from(self.quantity) * i128::from(self.price);
        let per = 1000 * i128::from(self.base.max(1));
        i64::try_from(rounded(gross, per)).unwrap_or(i64::MAX).saturating_sub(self.discount)
    }
}

/// `n / d` rounded half away from zero (EN 16931's rounding, Factur-X §7.1.8).
pub fn rounded(n: i128, d: i128) -> i128 {
    if d == 0 {
        return 0;
    }
    let (q, r) = (n / d, n % d);
    if 2 * r.abs() >= d.abs() { q + n.signum() * d.signum() } else { q }
}

/// The tax of one category and rate (BG-23).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaxBreak {
    pub category: String,
    pub rate: i64,
    /// BT-116.
    pub base: i64,
    /// BT-117.
    pub tax: i64,
    /// BT-120 and BT-121.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason_code: String,
}

/// The totals (BG-22), as EN 16931 checks them (BR-CO-10…17).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Totals {
    /// BT-106, the sum of the lines.
    pub lines: i64,
    /// BT-109.
    pub without_tax: i64,
    /// BT-110.
    pub tax: i64,
    /// BT-112.
    pub with_tax: i64,
    /// BT-113: deposits invoiced before, deducted.
    pub deposits: i64,
    /// BT-115.
    pub due: i64,
    #[serde(default)]
    pub breakdown: Vec<TaxBreak>,
}

/// Another invoice cited (BG-3): its number, its day, its amount with tax.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cited {
    pub number: String,
    pub date: String,
    #[serde(default)]
    pub amount: i64,
}

/// A note (BG-1), with its EN 16931 subject code (UNTDID 4451).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subject: String,
    /// The preset's line it comes from ("penalties"); empty for the person's own.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub legal: String,
}

/// A payment received (on a credit note: money paid back), as the
/// invoice's life reads it (`lives`): never written into its record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payment {
    /// Its entry's id in the life's files (`lives`: `hand:`, `bank:`,
    /// `settled:`, `older:<number>` for a format-1 record's own word).
    #[serde(default)]
    pub id: String,
    pub date: String,
    pub amount: i64,
    /// "transfer", "card", "cash", "cheque", "direct-debit", "other".
    #[serde(default)]
    pub means: String,
    /// The bank line it came from, or that was found for it when said by
    /// hand (`movements::line_key`); empty when none.
    #[serde(default)]
    pub line: String,
    /// What a processor kept of it, in cents: the bank received `amount − fee`.
    #[serde(default)]
    pub fee: i64,
}

/// Payment terms: days after the issue, at the end of that month or not.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Terms {
    pub days: u32,
    #[serde(default)]
    pub end_of_month: bool,
}

impl Terms {
    /// The due day of an invoice issued on `date`.
    pub fn due(&self, date: Date) -> Date {
        let after = date.checked_add(jiff::Span::new().days(i64::from(self.days))).unwrap_or(date);
        if self.end_of_month { after.last_of_month() } else { after }
    }
}

/// What a format-1 record printed, kept to print it again as before.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Older {
    pub siret: String,
    pub vat: String,
    pub payment: String,
    pub paid: bool,
    pub sessions: Vec<String>,
}

/// An invoice, a credit note or a deposit invoice; a draft until issued.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Invoice {
    /// 2. A format-1 record reads as 1 and is never written whole.
    pub format: u32,
    pub kind: Kind,
    /// The draft's own id, kept after issue: it is no number.
    #[serde(default)]
    pub draft: String,
    /// The sequence its number was given in ("2026-{N:3}"); empty on a draft.
    #[serde(default)]
    pub series: String,
    /// BT-1; empty on a draft.
    #[serde(default)]
    pub number: String,
    /// BT-2, "2026-11-16"; empty on a draft.
    #[serde(default)]
    pub date: String,
    /// The service's day or period (BG-14): the first and last days billed, or set.
    #[serde(default)]
    pub period_from: String,
    #[serde(default)]
    pub period_to: String,
    /// A draft's service day said by Sioul (the day of issue), not by you:
    /// made again until issued, when it is the issue's day.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub period_default: bool,
    /// BT-9, from the terms.
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub terms: Terms,
    /// BT-5, ISO 4217.
    pub currency: String,
    /// The invoice's own language ("en", "fr"), whatever the window's.
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub project_title: String,
    /// Who it bills, as chosen: a client's address (`sioul:client/<id>`,
    /// docs/clients.md), a client profile's id, a contact's address
    /// (`sioul:contact/<uid>`), or a name as written. An issued invoice's is
    /// never rewritten.
    #[serde(default)]
    pub client_ref: String,
    /// The card it is addressed to, by UID, beside a client's address: the
    /// one its billing identity mails, else the client's first; "" none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub contact: String,
    /// It names a client whose file holds a health mark (docs/clients.md,
    /// "Billing"): kept from agents unless they may read clients' files, its
    /// printed copy named by its number alone.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub health: bool,
    #[serde(default)]
    pub client: Client,
    #[serde(default)]
    pub issuer: Issuer,
    #[serde(default)]
    pub lines: Vec<Line>,
    #[serde(default)]
    pub totals: Totals,
    /// A credit note: the invoice it credits (BG-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrects: Option<Cited>,
    /// A final invoice: the deposit invoices it deducts.
    #[serde(default)]
    pub deposits: Vec<Cited>,
    /// A draft: the time taken out of it (session keys), left for a later
    /// invoice: "Make the invoice" never puts it back (`life::held`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set_aside: Vec<String>,
    /// A deposit invoice: the draft it was made from, whose final invoice deducts it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub deposit_of: String,
    /// The client's purchase order (BT-13).
    #[serde(default)]
    pub order: String,
    #[serde(default)]
    pub notes: Vec<Note>,
    /// The late-payment rate printed, in basis points; 0 when none.
    #[serde(default)]
    pub penalty_rate: i64,
    /// Its payments, from its life (`lives`), never from its record's file.
    #[serde(skip)]
    pub payments: Vec<Payment>,
    /// The day the mail with it left Sioul, from its life too.
    #[serde(skip)]
    pub sent: String,
    /// The issued file kept beside the record, and the SHA-256 of it and of
    /// the record (written from phase 2 on).
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub file_sha256: String,
    #[serde(default)]
    pub record_sha256: String,
    /// What a format-1 record printed.
    #[serde(skip)]
    pub older: Option<Older>,
    /// A record that no longer reads: its number only, never written over.
    #[serde(skip)]
    pub unreadable: bool,
    /// What issuing it found to say besides (a file of an issue stopped
    /// halfway, set aside): said once, never kept.
    #[serde(skip)]
    pub said: Vec<String>,
    /// Written by a newer Sioul (a later `format`): read where it could
    /// be, and said, so that you update Sioul here to see it as it is.
    #[serde(skip)]
    pub newer: bool,
    /// A draft's file that is a sync's conflicted copy of another draft:
    /// its name, said beside the draft, never hidden (`drafts_in`).
    #[serde(skip)]
    pub copy_of: String,
}

impl Invoice {
    /// The amount due (BT-115).
    pub fn total(&self) -> Money {
        Money(self.totals.due)
    }

    pub fn is_draft(&self) -> bool {
        self.number.is_empty()
    }

    /// Who it bills, as lists name it: its client, else its project.
    pub fn who(&self) -> String {
        if self.client.name.trim().is_empty() { self.project_title.clone() } else { self.client.name.clone() }
    }

    /// The time it bills, by session key: its lines', a format-1 record's own.
    pub fn sessions(&self) -> Vec<String> {
        let mut all: Vec<String> = self.lines.iter().flat_map(|l| l.sessions.iter().cloned()).collect();
        if let Some(older) = &self.older {
            all.extend(older.sessions.iter().cloned());
        }
        all.sort();
        all.dedup();
        all
    }

    /// What its payments came to, in cents.
    pub fn paid(&self) -> i64 {
        self.payments.iter().map(|p| p.amount).sum()
    }

    /// What issued credit notes among `all` credit it, in cents with tax.
    pub fn credited(&self, all: &[Invoice]) -> i64 {
        if self.number.is_empty() || self.kind == Kind::CreditNote {
            return 0;
        }
        all.iter().filter(|c| c.kind == Kind::CreditNote && !c.number.is_empty() && c.corrects.as_ref().is_some_and(|x| x.number == self.number)).map(|c| c.totals.due).sum()
    }

    /// Where it stands among `all` (its credit notes found there).
    pub fn status(&self, all: &[Invoice]) -> Status {
        if self.number.is_empty() {
            return Status::Draft;
        }
        if self.kind == Kind::CreditNote {
            return if self.to_pay_back(all) > 0 { Status::ToPayBack } else { Status::CreditNote };
        }
        let credited = self.credited(all);
        let paid = self.paid();
        let due = self.totals.due;
        if credited > 0 && credited >= due {
            Status::Credited
        } else if due <= 0 || (paid > 0 && paid + credited >= due) {
            Status::Paid
        } else if paid > 0 {
            // Partly paid only with a payment: a part credited is not paid.
            Status::PartlyPaid
        } else if !self.sent.is_empty() {
            Status::Sent
        } else {
            Status::Issued
        }
    }

    /// What is left to pay, in cents (never below zero); on a credit note,
    /// what is left to pay back.
    pub fn left(&self, all: &[Invoice]) -> i64 {
        if self.kind == Kind::CreditNote {
            return self.to_pay_back(all);
        }
        if self.status(all).settled() {
            return 0;
        }
        (self.totals.due - self.paid() - self.credited(all)).max(0)
    }

    /// A credit note's share of what was paid on the invoice it credits
    /// beyond what that invoice still asks (its amount less every credit
    /// note): the money it gives back, its paid-back entries aside. Shared
    /// among the invoice's credit notes in their numbers' order, each up to
    /// its own amount. 0 for anything but an issued credit note.
    pub fn pay_back_share(&self, all: &[Invoice]) -> i64 {
        let Some(cited) = self.corrects.as_ref().filter(|_| self.kind == Kind::CreditNote && !self.number.is_empty()) else { return 0 };
        let Some(invoice) = all.iter().find(|i| i.number == cited.number && !i.unreadable && i.kind != Kind::CreditNote) else { return 0 };
        let mut over = (invoice.paid() - (invoice.totals.due - invoice.credited(all)).max(0)).max(0);
        let mut notes: Vec<&Invoice> = all.iter().filter(|c| c.kind == Kind::CreditNote && !c.number.is_empty() && c.corrects.as_ref().is_some_and(|x| x.number == invoice.number)).collect();
        notes.sort_by(|a, b| super::natural(&a.number, &b.number));
        for note in notes {
            let share = over.min(note.totals.due.max(0));
            if note.number == self.number {
                return share;
            }
            over -= share;
        }
        0
    }

    /// What a credit note still has to pay back (`pay_back_share` less what it paid back).
    pub fn to_pay_back(&self, all: &[Invoice]) -> i64 {
        (self.pay_back_share(all) - self.paid()).max(0)
    }

    /// What is left to credit on it: its amount less the credit notes issued for it.
    pub fn left_to_credit(&self, all: &[Invoice]) -> i64 {
        if self.kind == Kind::CreditNote || self.number.is_empty() {
            return 0;
        }
        (self.totals.due - self.credited(all)).max(0)
    }

    /// The day it is due, when it says one.
    pub fn due_day(&self) -> Option<Date> {
        self.due.get(..10)?.parse().ok()
    }
}

/// A record's text read: format 2 as it is; format 1 (no `format` key) as an
/// issued invoice of kind 380 with time lines; a newer format as format 2
/// where it reads so, said (`Invoice::newer`); None when it reads no way.
pub fn read(text: &str) -> Option<Invoice> {
    let table: toml::Table = text.parse().ok()?;
    match table.get("format").and_then(toml::Value::as_integer) {
        Some(2) => toml::from_str::<Invoice>(text).ok(),
        Some(f) if f > 2 => toml::from_str::<Invoice>(text).ok().map(|i| Invoice { newer: true, ..i }),
        Some(_) => None,
        None => toml::from_str::<V1>(text).ok().map(V1::into_invoice),
    }
}

/// A format-1 record, as Sioul wrote it until November 2026.
#[derive(Debug, Deserialize)]
struct V1 {
    number: String,
    date: String,
    due: String,
    project: String,
    project_title: String,
    client: String,
    #[serde(default)]
    client_address: String,
    #[serde(default)]
    period_from: String,
    #[serde(default)]
    period_to: String,
    lines: Vec<V1Line>,
    total_cents: i64,
    currency: String,
    issuer: V1Issuer,
    sessions: Vec<String>,
    /// Its own word that it was paid, an older Sioul's: read as a payment
    /// from before (`lives`), which only an older Sioul changes.
    #[serde(default)]
    paid: bool,
}

#[derive(Debug, Deserialize)]
struct V1Line {
    label: String,
    minutes: u32,
    rate: f64,
    cents: i64,
}

#[derive(Debug, Deserialize)]
struct V1Issuer {
    name: String,
    address: String,
    siret: String,
    vat: String,
    payment: String,
}

impl V1 {
    fn into_invoice(self) -> Invoice {
        // Format 1 printed no VAT: a franchise's mention, or nothing.
        let (category, scheme) = if self.issuer.vat.trim().is_empty() { ("O", "none") } else { ("E", "franchise") };
        let lines: Vec<Line> = self
            .lines
            .iter()
            .map(|l| Line {
                // The project's rate, a float then, made cents at the boundary.
                amount: l.cents,
                ..Line::of_time(&l.label, l.minutes, (l.rate * 100.0).round() as i64, category, 0)
            })
            .collect();
        let breakdown = vec![TaxBreak { category: category.into(), rate: 0, base: self.total_cents, tax: 0, reason: self.issuer.vat.clone(), reason_code: String::new() }];
        Invoice {
            format: 1,
            kind: Kind::Invoice,
            number: self.number,
            date: self.date,
            due: self.due,
            period_from: self.period_from,
            period_to: self.period_to,
            currency: self.currency,
            project: self.project,
            project_title: self.project_title.clone(),
            client: Client { kind: "business".into(), name: self.client.clone(), address: Address::from_text(&self.client_address), ..Client::default() },
            client_ref: self.client,
            issuer: Issuer {
                name: self.issuer.name,
                address: Address::from_text(&self.issuer.address),
                ids: if self.issuer.siret.trim().is_empty() { Vec::new() } else { vec![Identifier { kind: "siret".into(), value: self.issuer.siret.clone(), printed: true }] },
                scheme: scheme.into(),
                mention: self.issuer.vat.clone(),
                payment: self.issuer.payment.clone(),
                ..Issuer::default()
            },
            lines,
            totals: Totals { lines: self.total_cents, without_tax: self.total_cents, tax: 0, with_tax: self.total_cents, deposits: 0, due: self.total_cents, breakdown },
            older: Some(Older { siret: self.issuer.siret, vat: self.issuer.vat, payment: self.issuer.payment, paid: self.paid, sessions: self.sessions }),
            ..Invoice::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An older Sioul (before November 2026) reads a record as `V1`, the
    /// struct of main a9407a1d, and writes back whole what it read when its
    /// Paid is ticked: a format-2 record must never read so, or Paid on an
    /// older device would write it over without its lines, its taxes and
    /// its client (the second review, G16). It stays a number to it, which
    /// it never gives again and lists in no project.
    #[test]
    fn an_older_sioul_never_reads_a_format_2_record_as_its_own() {
        let issued = Invoice { format: FORMAT, number: "2026-012".into(), date: "2026-11-16".into(), due: "2026-12-16".into(), currency: "EUR".into(), project: "lumen".into(), project_title: "Studio Lumen".into(), client: Client { name: "Studio Lumen".into(), ..Client::default() }, lines: vec![Line::of_time("Advice", 60, 9000, "E", 0)], ..Invoice::default() };
        let text = toml::to_string(&issued).unwrap();
        assert!(toml::from_str::<V1>(&text).is_err(), "{text}");
        assert_eq!(read(&text).map(|i| i.number), Some("2026-012".to_string()));
    }

    #[test]
    fn amounts_round_as_en_16931_does() {
        assert_eq!(rounded(13455, 1000), 13);
        assert_eq!(rounded(1345, 100), 13);
        assert_eq!(rounded(1355, 100), 14);
        assert_eq!(rounded(-1355, 100), -14);
        // 25 minutes at €60 an hour: €25.00, not 0.417 h × 60.
        let line = Line::of_time("Call", 25, 6000, "E", 0);
        assert_eq!((line.net(), line.minutes()), (2500, Some(25)));
        // 90 minutes at €33.33: 4999.5 cents, half away from zero.
        assert_eq!(Line::of_time("Build", 90, 3333, "E", 0).amount, 5000);
        // Three days at €450, €100 off.
        let days = Line { quantity: 3000, unit: "DAY".into(), price: 45000, base: 1, discount: 10000, ..Line::of_time("x", 0, 0, "S", 2000) };
        assert_eq!(days.net(), 125_000);
        assert_eq!(Terms { days: 30, end_of_month: false }.due("2026-11-16".parse().unwrap()).to_string(), "2026-12-16");
        assert_eq!(Terms { days: 30, end_of_month: true }.due("2026-11-16".parse().unwrap()).to_string(), "2026-12-31");
    }
}
