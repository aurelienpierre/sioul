// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The books (docs/projects.md, "The books"): what the law asks a business
//! to keep of the money it receives, made from the payments of the invoices
//! it issued and, for what no invoice says, from the work budgets that
//! declare (`receipts`), as the law counts it: money received, on its day.
//!
//! - **The receipts book** (`book`, here): one row per payment received, on
//!   its day: the client (the invoice's, not the bank's words), the nature of
//!   what was sold, the invoice, the means of payment (cash said apart), the
//!   amount, and under VAT the amount without it; a refund paid back on a
//!   credit note is a negative row on its own day; totals per quarter and
//!   per year; as CSV and as a page to print (C. com. L123-28; CGI 50-0,
//!   102 ter, 286; BOI-BIC-DECLA-30-30 §20–30). Payments are read in one
//!   place, through each record's own (`received_of`).
//! - **Declarations** (`declarations`): per period and activity, the
//!   turnover received excluding VAT, the contributions estimated at the
//!   preset's dated rates, the day to declare by; zero periods too.
//! - **The VAT summary** (`vat`), for an issuer liable to VAT: per period and
//!   rate, what was collected; it prepares the return, never files it.
//! - **Thresholds** (`thresholds`): the VAT franchise's while it holds, the
//!   micro regime's under it; each step said once, on one device.
//! - **The DES** (`des`): services to businesses in other member states,
//!   month by month.
//!
//! Rates, thresholds and the rules of declarations are each preset's dated
//! data (`invoice::presets`), each overridable in `[invoice]`. Nothing here
//! says "late": a day to declare by is a day, before or after it.

pub mod declarations;
pub mod des;
pub mod thresholds;
pub mod vat;

use crate::budget::Ledger;
use crate::config::InvoiceSettings;
use crate::i18n::Translator;
use crate::invoice::presets::Preset;
use crate::invoice::{Invoice, Kind, Nature, lives};
use crate::movements::Books as Money;
use jiff::civil::{Date, Weekday};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

// ---------------------------------------------------------------- settings

/// How often something is declared or summed up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Every {
    Month,
    Quarter,
}

impl Every {
    pub fn named(word: &str) -> Option<Every> {
        match word.trim() {
            "month" | "monthly" => Some(Every::Month),
            "quarter" | "quarterly" => Some(Every::Quarter),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Every::Month => "month",
            Every::Quarter => "quarter",
        }
    }
}

/// A rate of your own over the preset's (`[[invoice.contribution]]`), in basis points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnRate {
    pub kind: String,
    pub activity: String,
    pub rate: i64,
    pub from: Option<Date>,
    pub until: Option<Date>,
}

/// A threshold of your own over the preset's (`[[invoice.threshold]]`), in cents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnThreshold {
    pub id: String,
    pub amount: i64,
    pub from: Option<Date>,
}

/// What the books read of the settings (`[invoice]`), with their defaults.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The invoices' settings as they are (the VAT scheme is read on each day: `scheme_on`).
    pub invoice: InvoiceSettings,
    pub preset: &'static Preset,
    /// "micro", "reel", "" (none said, none implied).
    pub regime: String,
    /// The declarations' period: as said; else as a budget that declares
    /// says; else the preset's first; else each quarter.
    pub every: Every,
    /// The VAT summary's period: as said, else each month.
    pub vat_every: Every,
    /// The versement libératoire, paid with each declaration.
    pub vl: bool,
    /// The activities said ("bnc", "goods-bic"…), in their order.
    pub activities: Vec<String>,
    /// The trade the training contribution is counted for; "" when not known.
    pub trade: String,
    pub started: Option<Date>,
    /// The option for VAT on debits (France): services' VAT on the invoice's day.
    pub debits: bool,
    /// The books' currency: the preset's, else the settings', else euros.
    pub currency: String,
    pub rates: Vec<OwnRate>,
    pub thresholds: Vec<OwnThreshold>,
}

/// A day as settings write it ("2026-03-01"); None for none or one that does not read.
fn day_of(text: &str) -> Option<Date> {
    text.trim().get(..10).and_then(|d| d.parse().ok())
}

impl Settings {
    /// The books' settings of `s`; `ledger` gives the period a budget that
    /// declares said before November 2026.
    pub fn of(s: &InvoiceSettings, ledger: Option<&Ledger>) -> Settings {
        let preset = crate::invoice::life::preset_of(s);
        let budgets = ledger.map(|l| l.budgets.as_slice()).unwrap_or_default();
        let budget_said = budgets.iter().filter(|b| crate::receipts::declares(b)).find_map(|b| b.declare.as_deref().and_then(Every::named));
        let every = Every::named(&s.declare).or(budget_said).or_else(|| preset.declaration.as_ref().and_then(|d| d.periods.first()).and_then(|p| Every::named(p))).unwrap_or(Every::Quarter);
        let activities: Vec<String> = s.activity.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
        // The CFP's trade, when the activity tells it (URSSAF): goods are sold
        // by a commerçant, liberal activities are liberal; BIC services may be
        // commercial or craft, which only you can say.
        let trade = match s.trade.trim() {
            "" => {
                let implied: BTreeSet<&str> = activities
                    .iter()
                    .map(|a| match a.as_str() {
                        "goods-bic" => "commerce",
                        "bnc" | "cipav" => "liberal",
                        _ => "",
                    })
                    .collect();
                if implied.len() == 1 { implied.into_iter().next().unwrap_or_default().to_string() } else { String::new() }
            }
            said => said.to_string(),
        };
        let currency = [preset.currency.as_str(), s.currency.as_str(), "EUR"].into_iter().map(str::trim).find(|c| !c.is_empty()).unwrap_or("EUR").to_uppercase();
        Settings {
            invoice: s.clone(),
            preset,
            regime: crate::invoice::life::regime_of(s),
            every,
            vat_every: Every::named(&s.vat_period).unwrap_or(Every::Month),
            vl: s.vl,
            activities,
            trade,
            started: day_of(&s.started),
            debits: s.options.iter().any(|o| o == "debits"),
            currency,
            rates: s
                .contributions
                .iter()
                .filter(|c| c.rate.is_finite() && c.rate >= 0.0)
                .map(|c| OwnRate { kind: c.kind.trim().to_string(), activity: c.activity.trim().to_string(), rate: (c.rate * 100.0).round() as i64, from: day_of(&c.from), until: day_of(&c.until) })
                .collect(),
            thresholds: s.thresholds.iter().filter(|t| t.amount.is_finite() && t.amount > 0.0).map(|t| OwnThreshold { id: t.id.trim().to_string(), amount: (t.amount * 100.0).round() as i64, from: day_of(&t.from) }).collect(),
        }
    }

    /// The VAT scheme on `day`: "none", "franchise", "liable".
    pub fn scheme_on(&self, day: Date) -> String {
        crate::invoice::life::scheme_on(&self.invoice, day).0
    }

    /// Whether the preset asks declarations of turnover under this regime (France's micro).
    pub fn declares(&self) -> bool {
        self.preset.declaration.as_ref().is_some_and(|d| d.regime == self.regime)
    }

    /// The nature of an activity of the preset's ("goods", "service").
    pub fn nature_of(&self, activity: &str) -> Option<&'static str> {
        self.preset.activity.iter().find(|a| a.id == activity).map(|a| if a.nature == "goods" { "goods" } else { "service" })
    }

    /// The activity that what is of `nature` is declared in: the first
    /// activity said of that nature; None when none is said.
    pub fn activity_for(&self, nature: &str) -> Option<&str> {
        self.activities.iter().find(|a| self.nature_of(a) == Some(nature)).map(String::as_str)
    }

    /// The activity a receipt no invoice describes is declared in: the first said.
    pub fn activity_unsaid(&self) -> Option<&str> {
        self.activities.first().map(String::as_str)
    }

    /// An activity's name in `language`, as its preset says it.
    pub fn activity_name(&self, activity: &str, language: &str) -> String {
        self.preset.activity.iter().find(|a| a.id == activity).map_or_else(|| activity.to_string(), |a| a.name.get(language).to_string())
    }

    /// The rate of `kind` for `activity` in force on `day`, in basis points:
    /// yours when one holds then, else the preset's for this regime.
    pub fn rate_on(&self, kind: &str, activity: &str, day: Date) -> Option<i64> {
        let own = self.rates.iter().filter(|r| r.kind == kind && r.activity == activity && r.from.is_none_or(|f| f <= day) && r.until.is_none_or(|u| day <= u)).max_by_key(|r| r.from);
        if let Some(own) = own {
            return Some(own.rate);
        }
        self.preset
            .contribution
            .iter()
            .filter(|c| c.kind == kind && c.activity == activity && c.from <= day && (c.regime.is_empty() || c.regime == self.regime))
            .max_by_key(|c| c.from)
            .map(|c| c.rate)
    }
}

// ---------------------------------------------------------------- days

/// The first and last days of the period of `every` that holds `day`.
pub fn period_of(every: Every, day: Date) -> (Date, Date) {
    let month = match every {
        Every::Month => day.month(),
        Every::Quarter => (day.month() - 1) / 3 * 3 + 1,
    };
    let from = Date::new(day.year(), month, 1).unwrap_or(day);
    let to = match every {
        Every::Month => from.last_of_month(),
        Every::Quarter => shift_months(from, 2).last_of_month(),
    };
    (from, to)
}

/// The same day `months` months later (the first of a month stays the first).
pub fn shift_months(day: Date, months: i64) -> Date {
    day.checked_add(jiff::Span::new().months(months)).unwrap_or(day)
}

/// `day` and `n` days after.
pub fn shift_days(day: Date, n: i64) -> Date {
    day.checked_add(jiff::Span::new().days(n)).unwrap_or(day)
}

/// The days from `a` to `b` (positive when `b` is later).
pub fn days_between(a: Date, b: Date) -> i64 {
    a.until(b).map_or(0, |s| i64::from(s.get_days()))
}

/// Whether a day is worked: Monday to Friday. Public holidays are not
/// counted (not in Sioul's sources): a day said may come a day or two
/// before the law's, never after.
pub fn working(day: Date) -> bool {
    !matches!(day.weekday(), Weekday::Saturday | Weekday::Sunday)
}

/// `day`, or the next working day when it is not one (a weekend).
pub fn next_working(day: Date) -> Date {
    let mut d = day;
    while !working(d) {
        d = shift_days(d, 1);
    }
    d
}

/// The `n`-th working day of the month of `month` (its first day, any day of it).
pub fn nth_working(month: Date, n: u32) -> Date {
    let mut d = month.first_of_month();
    let mut seen = 0;
    loop {
        if working(d) {
            seen += 1;
            if seen >= n.max(1) {
                return d;
            }
        }
        d = shift_days(d, 1);
    }
}

/// The calendar quarter of a day, 1 to 4.
pub fn quarter_of(day: Date) -> i8 {
    (day.month() - 1) / 3 + 1
}

// ---------------------------------------------------------------- the receipts book

/// One payment received, as the receipts book writes it; a refund paid back
/// on a credit note is one too, its amounts negative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// The day it was received.
    pub day: Date,
    /// Who paid: the invoice's client; for a receipt no invoice describes, its movement's words.
    pub client: String,
    /// What was sold: "goods", "service", "both"; "" when no invoice says it.
    pub nature: String,
    /// The invoice it pays, by its number; "" for none.
    pub invoice: String,
    /// How it was paid, in words.
    pub means: String,
    /// Paid in cash, said apart (BOI-BIC-DECLA-30-30 §20).
    pub cash: bool,
    /// What was received, VAT included, in the books' currency; none when
    /// not known: a payout whose gross is not known yet, a payment in
    /// another currency whose amount in the books' is not said.
    pub amount: Option<i64>,
    /// Excluding VAT: the turnover declared and counted against the
    /// thresholds (the same under the franchise).
    pub without_tax: Option<i64>,
    /// Of what is excluding VAT, the goods' part (a mixed activity shows goods and services apart).
    pub goods: i64,
    /// In another currency: the amount in it, and its code.
    pub foreign: Option<(i64, String)>,
    /// Its thing: the invoice's address, else the movement's.
    pub address: String,
    /// The invoice's project ("" none): the agents see it only when its project is open to them.
    pub project: String,
    /// The budget of a receipt no invoice describes ("" none).
    pub budget: String,
    /// The payment's own id in the invoices' life ("" for a movement's row).
    pub payment: String,
}

impl Row {
    /// The services' part of what is excluding VAT; none said for a receipt
    /// no invoice describes, whose nature is not known.
    pub fn services(&self) -> i64 {
        if self.nature.is_empty() { 0 } else { self.without_tax.map_or(0, |w| w - self.goods) }
    }

    /// Its goods' and services' parts, when its nature is known.
    fn split(&self) -> Option<(i64, i64)> {
        self.without_tax.filter(|_| !self.nature.is_empty()).map(|_| (self.goods, self.services()))
    }
}

/// The totals of some days of the book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Total {
    /// "quarter" (its number in `quarter`), "year".
    pub kind: &'static str,
    pub year: i16,
    pub quarter: i8,
    pub from: Date,
    pub to: Date,
    /// VAT included, then excluding it, then its goods' and services' parts
    /// (a receipt whose nature is not said in neither).
    pub amount: i64,
    pub without_tax: i64,
    pub goods: i64,
    pub services: i64,
    /// A receipt's amount is not known: these are the least they come to.
    pub at_least: bool,
    /// Payments in another currency not counted (their amount in the books' not said).
    pub foreign: usize,
}

/// The receipts book of some days, with its quarters' and years' totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Book {
    pub from: Date,
    pub to: Date,
    pub currency: String,
    /// In the order of the days.
    pub rows: Vec<Row>,
    /// Each quarter the days touch (the whole quarter until `to`), then each year's (from 1 January until `to`).
    pub totals: Vec<Total>,
}

/// What the books read: the settings, the invoices with their life, the
/// amounts said in the books' currency, the money's movements.
pub struct Input<'a> {
    pub settings: &'a Settings,
    /// Every issued invoice, with its payments (`invoice::all_in`).
    pub invoices: &'a [Invoice],
    /// What payments in another currency came to in the books', by payment id (`lives::Lives::booked`).
    pub booked: &'a BTreeMap<String, i64>,
    /// The money's movements: the receipts no invoice describes, from the budgets that declare.
    pub money: Option<&'a Money>,
    /// Only this budget's movements (`sioul receipts --budget`); none: every budget that declares.
    pub budget: Option<&'a str>,
}

/// What `load` read, owned: `input` lends it.
pub struct Loaded {
    pub settings: Settings,
    pub invoices: Vec<Invoice>,
    pub booked: BTreeMap<String, i64>,
    pub told: BTreeSet<String>,
    pub money: Option<std::sync::Arc<Money>>,
}

impl Loaded {
    pub fn input(&self) -> Input<'_> {
        Input { settings: &self.settings, invoices: &self.invoices, booked: &self.booked, money: self.money.as_deref(), budget: None }
    }
}

/// The books as the files say them now: the configuration's invoice
/// settings, the invoices of `dir` with their life, the notes folder's money.
pub fn load(config: &crate::config::Config, dir: &Path, today: Date) -> Loaded {
    let root = config.notes_root_path();
    let words = crate::words::Words::of(config);
    let money = root.as_deref().filter(|r| Ledger::load(r).is_ok()).map(|r| crate::movements::books(r, &words, today));
    let settings = Settings::of(&config.invoice, money.as_deref().map(|m| &m.ledger));
    let lives = lives::Lives::load(dir);
    Loaded { settings, invoices: crate::invoice::all_in(dir), booked: lives.booked, told: lives.told, money }
}

/// `part` of `whole`, as a share of `of`: `of × part / whole`, rounded half
/// away from zero; `of` itself when `whole` is nothing.
pub fn share(part: i64, of: i64, whole: i64) -> i64 {
    if whole == 0 {
        return of;
    }
    i64::try_from(crate::invoice::record::rounded(i128::from(of) * i128::from(part), i128::from(whole))).unwrap_or(of)
}

/// What a line sells, as an invoice's lines say it: "goods", "service", "both".
pub fn nature_of(invoice: &Invoice) -> &'static str {
    let goods = invoice.lines.iter().any(|l| l.nature == Nature::Goods);
    let services = invoice.lines.iter().any(|l| l.nature == Nature::Service);
    match (goods, services) {
        (true, true) => "both",
        (true, false) => "goods",
        _ => "service",
    }
}

/// Of `amount` excluding VAT on `invoice`, the goods' part: their share of its lines.
pub fn goods_part(invoice: &Invoice, amount: i64) -> i64 {
    let goods: i64 = invoice.lines.iter().filter(|l| l.nature == Nature::Goods).map(|l| l.amount).sum();
    let base = invoice.totals.lines;
    if base == 0 || goods == 0 {
        return 0;
    }
    i64::try_from(crate::invoice::record::rounded(i128::from(amount) * i128::from(goods), i128::from(base))).unwrap_or(0)
}

/// One entry of money received on an invoice, or given back on a credit
/// note (a refund, negative), as the invoice's own life gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The entry's id in the invoices' life.
    pub id: String,
    pub day: Date,
    /// Signed: what came in, or minus what was paid back.
    pub amount: i64,
    /// The entry's own amount, never signed: its share of the record's totals.
    pub entry: i64,
    pub means: String,
    /// The bank line's movement it came from; "" when said by hand.
    pub movement: String,
}

/// The money an issued record received (an invoice, a deposit invoice), or
/// gave back (a credit note's refunds, negative): the one place the books
/// read payments, through the record's own (`Invoice::payments`, as
/// `invoice::all_in` gives them its life). An entry whose day does not read
/// is left out.
pub fn received_of(invoice: &Invoice) -> Vec<Received> {
    let sign = if invoice.kind == Kind::CreditNote { -1 } else { 1 };
    invoice
        .payments
        .iter()
        .filter_map(|p| {
            let day = p.date.get(..10)?.parse::<Date>().ok()?;
            Some(Received { id: p.id.clone(), day, amount: sign * p.amount, entry: p.amount, means: p.means.clone(), movement: p.movement.clone() })
        })
        .collect()
}

/// How an invoice's payment was paid, in words.
pub fn means_words(tr: &Translator, means: &str) -> String {
    match means.trim() {
        "" => tr.text("books-means-unsaid", None),
        "transfer" | "card" | "cash" | "cheque" | "direct-debit" | "other" => tr.text(&format!("books-means-{}", means.trim()), None),
        other => other.to_string(),
    }
}

/// A movement of a budget that declares, as the old register read it, kept
/// aside until it is known whether an invoice's payment stands for it.
struct Moved {
    receipt: crate::receipts::Receipt,
    budget: String,
    taken: bool,
}

/// The movements of the budgets that declare (or the one asked), each by
/// its day paid, over every day the money knows.
fn moved(input: &Input) -> Vec<Moved> {
    let Some(money) = input.money else { return Vec::new() };
    let mut out = Vec::new();
    for budget in money.ledger.budgets.iter().filter(|b| crate::receipts::declares(b) && input.budget.is_none_or(|only| only == b.id)) {
        let register = crate::receipts::register_rows(money, &budget.id, money.from, money.to, None);
        out.extend(register.receipts.into_iter().map(|receipt| Moved { receipt, budget: budget.id.clone(), taken: false }));
    }
    out
}

/// Every payment received from `from` to `to`, in the order of the days
/// (see `book`): each invoice's payments, each credit note's refunds
/// (negative: money given back lowers the turnover of its day's period),
/// then the receipts of the budgets that declare that no payment stands for.
pub fn rows(input: &Input, from: Date, to: Date, tr: &Translator) -> Vec<Row> {
    let within = |d: Date| d >= from && d <= to;
    let mut moved = moved(input);
    let records: Vec<(&Invoice, Vec<Received>)> = input.invoices.iter().filter(|i| !i.number.is_empty() && !i.unreadable).map(|i| (i, received_of(i))).collect();
    // A movement an invoice's payment came from is that payment: never twice.
    let paid_by: BTreeSet<&str> = records.iter().flat_map(|(_, r)| r.iter()).map(|p| p.movement.as_str()).filter(|m| !m.is_empty()).collect();
    for m in &mut moved {
        if paid_by.contains(m.receipt.address.as_str()) {
            m.taken = true;
        }
    }
    let currency = input.settings.currency.as_str();
    let mut out = Vec::new();
    for (invoice, received) in &records {
        let nature = nature_of(invoice);
        let client = if invoice.client.first_line().trim().is_empty() { invoice.who() } else { invoice.client.first_line().to_string() };
        for p in received {
            let mut day = p.day;
            // Said by hand, and the bank's line for this invoice found without
            // being matched to it: that line is this payment. A format-1
            // record's own "paid" had no day: the line's is its day.
            if p.movement.is_empty()
                && p.amount > 0
                && let Some(m) = moved.iter_mut().find(|m| !m.taken && m.receipt.invoice == invoice.number && (p.id.starts_with("older:") || m.receipt.gross.map(|g| g.cents()) == Some(p.amount) || m.receipt.net.cents() == p.amount))
            {
                m.taken = true;
                if p.id.starts_with("older:") {
                    day = m.receipt.day;
                }
            }
            if !within(day) {
                continue;
            }
            let foreign = !invoice.currency.trim().is_empty() && !invoice.currency.eq_ignore_ascii_case(currency);
            let sign = p.amount.signum();
            let amount = if foreign { input.booked.get(&p.id).map(|b| sign * b.abs()) } else { Some(p.amount) };
            // Excluding VAT, as the record's share; in the books' currency, the same share of what it came to.
            let without_tax = amount.map(|a| if foreign { share(crate::invoice::life::received_without_tax(invoice, p.amount), a, p.amount) } else { crate::invoice::life::received_without_tax(invoice, a) });
            out.push(Row {
                day,
                client: client.clone(),
                nature: nature.to_string(),
                invoice: invoice.number.clone(),
                means: means_words(tr, &p.means),
                cash: p.means == "cash",
                amount,
                without_tax,
                goods: without_tax.map_or(0, |w| goods_part(invoice, w)),
                foreign: foreign.then(|| (p.amount, invoice.currency.to_uppercase())),
                address: crate::links::invoice_uri(&invoice.number),
                project: invoice.project.clone(),
                budget: String::new(),
                payment: p.id.clone(),
            });
        }
    }
    let invoices: BTreeMap<&str, &Invoice> = input.invoices.iter().filter(|i| !i.number.is_empty() && !i.unreadable).map(|i| (i.number.as_str(), i)).collect();
    for m in moved.into_iter().filter(|m| !m.taken && within(m.receipt.day)) {
        let r = m.receipt;
        // A movement the matching tied to an invoice without a payment written:
        // the invoice says who and what.
        let invoice = invoices.get(r.invoice.as_str()).copied();
        let amount = r.gross.map(crate::money::Money::cents);
        let liable = input.settings.scheme_on(r.day) == "liable";
        let without_tax = match invoice {
            Some(i) => amount.map(|a| crate::invoice::life::received_without_tax(i, a)),
            // Under VAT, what is without it is not known when no invoice says it.
            None if liable => None,
            None => amount,
        };
        out.push(Row {
            day: r.day,
            client: invoice.map_or_else(|| r.client.clone(), |i| i.client.first_line().to_string()),
            nature: invoice.map(|i| nature_of(i).to_string()).unwrap_or_default(),
            invoice: r.invoice.clone(),
            means: r.means.words(tr),
            cash: false,
            amount,
            without_tax,
            goods: match (invoice, without_tax) {
                (Some(i), Some(w)) => goods_part(i, w),
                _ => 0,
            },
            foreign: None,
            address: r.address.clone(),
            project: invoice.map(|i| i.project.clone()).unwrap_or_default(),
            budget: m.budget,
            payment: String::new(),
        });
    }
    out.sort_by(|a, b| (a.day, &a.invoice, &a.client).cmp(&(b.day, &b.invoice, &b.client)));
    out
}

/// The totals of `rows` from `from` to `to`.
fn total_of(rows: &[Row], kind: &'static str, from: Date, to: Date) -> Total {
    let of: Vec<&Row> = rows.iter().filter(|r| r.day >= from && r.day <= to).collect();
    Total {
        kind,
        year: from.year(),
        quarter: quarter_of(from),
        from,
        to,
        amount: of.iter().filter_map(|r| r.amount).sum(),
        without_tax: of.iter().filter_map(|r| r.without_tax).sum(),
        goods: of.iter().map(|r| r.goods).sum(),
        services: of.iter().map(|r| r.services()).sum(),
        at_least: of.iter().any(|r| r.foreign.is_none() && (r.amount.is_none() || r.without_tax.is_none())),
        foreign: of.iter().filter(|r| r.foreign.is_some() && r.amount.is_none()).count(),
    }
}

/// The receipts book from `from` to `to`: its rows, each quarter's totals
/// (the whole quarter, until `to`) and each year's (from its 1 January,
/// until `to`), as the law asks them (BOI-BIC-DECLA-30-30 §20).
pub fn book(input: &Input, from: Date, to: Date, tr: &Translator) -> Book {
    let start = Date::new(from.year(), 1, 1).unwrap_or(from);
    let all = rows(input, start, to, tr);
    let mut totals = Vec::new();
    let mut q = period_of(Every::Quarter, from).0;
    while q <= to {
        let end = period_of(Every::Quarter, q).1.min(to);
        totals.push(total_of(&all, "quarter", q, end));
        q = shift_months(q, 3);
    }
    for year in from.year()..=to.year() {
        let first = Date::new(year, 1, 1).unwrap_or(from);
        let last = Date::new(year, 12, 31).unwrap_or(to).min(to);
        totals.push(total_of(&all, "year", first, last));
    }
    Book { from, to, currency: input.settings.currency.clone(), rows: all.into_iter().filter(|r| r.day >= from && r.day <= to).collect(), totals }
}

/// "1 October 2026".
pub fn long_day(tr: &Translator, d: Date) -> String {
    format!("{} {}", tr.day_month(d), d.year())
}

/// A total's name: "First quarter 2026", "2026".
pub fn total_name(tr: &Translator, t: &Total) -> String {
    let mut args = crate::i18n::args();
    args.set("quarter", i64::from(t.quarter));
    args.set("year", t.year.to_string());
    tr.text(if t.kind == "quarter" { "books-total-quarter" } else { "books-total-year" }, Some(&args))
}

/// Whether some row of the book bears VAT: its columns show both amounts then.
pub fn taxed(book: &Book) -> bool {
    book.rows.iter().any(|r| r.amount.zip(r.without_tax).is_some_and(|(a, w)| a != w))
}

/// Whether the book shows goods and services apart: some row sells goods,
/// another (or the same) services.
pub fn mixed(book: &Book) -> bool {
    book.rows.iter().any(|r| r.goods != 0) && book.rows.iter().any(|r| r.services() != 0)
}

/// What the book says above its rows: what was received, the totals; a
/// payment in another currency not counted.
pub fn sentences(tr: &Translator, book: &Book) -> Vec<String> {
    let mut out: Vec<String> = book.totals.iter().map(|t| total_sentence(tr, t, &book.currency)).collect();
    let foreign: Vec<&Row> = book.rows.iter().filter(|r| r.foreign.is_some() && r.amount.is_none()).collect();
    if !foreign.is_empty() {
        let mut args = crate::i18n::args();
        args.set("n", foreign.len() as i64);
        args.set("currency", foreign[0].foreign.as_ref().map(|(_, c)| c.clone()).unwrap_or_default());
        out.push(tr.text("books-foreign-not-counted", Some(&args)));
    }
    out
}

/// A total in a sentence: "Fourth quarter 2026: €2,080 received.", and under VAT both amounts.
pub fn total_sentence(tr: &Translator, t: &Total, currency: &str) -> String {
    let money = |m: i64| tr.money_in(crate::money::Money(m), currency);
    let mut args = crate::i18n::args();
    args.set("name", total_name(tr, t));
    args.set("amount", money(t.amount));
    args.set("without", money(t.without_tax));
    args.set("least", if t.at_least { "yes" } else { "no" });
    tr.text(if t.amount != t.without_tax { "books-total-taxed" } else { "books-total" }, Some(&args))
}

/// An amount as a spreadsheet reads it in the language's own way: "520.00", "520,00".
pub fn plain_amount(tr: &Translator, cents: i64) -> String {
    let m = crate::money::Money(cents);
    let (units, rest) = m.split();
    let sign = if m.is_negative() { "-" } else { "" };
    format!("{sign}{units}{}{rest:02}", tr.text("decimal-separator", None))
}

/// The book as CSV: its header in your language, one receipt a line, in
/// the order of the days, each quarter's total after its days, the year's
/// last. Days as 2026-10-05; amounts with the language's decimal mark and
/// separator; cash said in its own column; goods and services apart for a
/// mixed activity; both amounts under VAT. Every text cell passes the
/// spreadsheet's guard (`timereport::csv_cell`).
pub fn csv(tr: &Translator, book: &Book) -> String {
    let separator = if tr.text("decimal-separator", None) == "," { ';' } else { ',' };
    let cell = |text: &str| crate::timereport::csv_cell(text, separator);
    let (taxed, mixed) = (taxed(book), mixed(book));
    let mut header = vec!["books-column-day", "books-column-client", "books-column-nature", "books-column-invoice", "books-column-means", "books-column-cash", "books-column-amount"];
    if taxed {
        header.push("books-column-without-tax");
    }
    if mixed {
        header.extend(["books-column-goods", "books-column-services"]);
    }
    header.push("books-column-note");
    let line = |cells: Vec<String>| format!("{}\r\n", cells.join(&separator.to_string()));
    let mut out = line(header.iter().map(|id| cell(&tr.text(id, None))).collect());
    let amount = |m: Option<i64>| m.map(|m| plain_amount(tr, m)).unwrap_or_default();
    let unknown = tr.text("receipts-gross-unknown", None);
    let row_cells = |r: &Row| {
        let mut cells = vec![r.day.to_string(), cell(&r.client), cell(&nature_words(tr, &r.nature)), cell(&r.invoice), cell(&r.means), if r.cash { cell(&tr.text("books-cash", None)) } else { String::new() }, amount(r.amount)];
        if taxed {
            cells.push(amount(r.without_tax));
        }
        if mixed {
            cells.push(r.split().map(|(g, _)| plain_amount(tr, g)).unwrap_or_default());
            cells.push(r.split().map(|(_, s)| plain_amount(tr, s)).unwrap_or_default());
        }
        let note = match &r.foreign {
            Some((m, code)) => tr.money_in(crate::money::Money(*m), code),
            None if r.amount.is_none() => unknown.clone(),
            None => String::new(),
        };
        cells.push(cell(&note));
        cells
    };
    let total_cells = |t: &Total| {
        let mut cells = vec![String::new(), cell(&total_name(tr, t)), String::new(), String::new(), String::new(), String::new(), plain_amount(tr, t.amount)];
        if taxed {
            cells.push(plain_amount(tr, t.without_tax));
        }
        if mixed {
            cells.push(plain_amount(tr, t.goods));
            cells.push(plain_amount(tr, t.services));
        }
        cells.push(if t.at_least { cell(&tr.text("receipts-at-least", None)) } else { String::new() });
        cells
    };
    let quarters: Vec<&Total> = book.totals.iter().filter(|t| t.kind == "quarter").collect();
    for (i, r) in book.rows.iter().enumerate() {
        out.push_str(&line(row_cells(r)));
        // A quarter's total after its last row.
        let next = book.rows.get(i + 1).map(|n| n.day);
        for q in quarters.iter().filter(|q| r.day >= q.from && r.day <= q.to && next.is_none_or(|n| n > q.to)) {
            out.push_str(&line(total_cells(q)));
        }
    }
    // Quarters without a row in the book's days, then the years.
    for q in quarters.iter().filter(|q| !book.rows.iter().any(|r| r.day >= q.from && r.day <= q.to)) {
        out.push_str(&line(total_cells(q)));
    }
    for y in book.totals.iter().filter(|t| t.kind == "year") {
        out.push_str(&line(total_cells(y)));
    }
    out
}

/// What a row sold, in words.
pub fn nature_words(tr: &Translator, nature: &str) -> String {
    match nature {
        "goods" => tr.text("invoice-nature-goods", None),
        "service" => tr.text("invoice-nature-services", None),
        "both" => tr.text("invoice-nature-both", None),
        _ => String::new(),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The book as a page to print (the window prints it to PDF, as it prints
/// the register before): its title, what was received, the rows, the totals.
pub fn html(tr: &Translator, book: &Book, name: &str) -> String {
    let mut args = crate::i18n::args();
    args.set("from", long_day(tr, book.from));
    args.set("to", long_day(tr, book.to));
    args.set("name", name.to_string());
    let title = tr.text("books-title", Some(&args));
    let (taxed, mixed) = (taxed(book), mixed(book));
    let money = |m: i64| escape(&tr.money_in(crate::money::Money(m), &book.currency));
    let mut head = vec!["books-column-day", "books-column-client", "books-column-nature", "books-column-invoice", "books-column-means", "books-column-amount"];
    if taxed {
        head.push("books-column-without-tax");
    }
    if mixed {
        head.extend(["books-column-goods", "books-column-services"]);
    }
    let head: String = head.iter().enumerate().map(|(i, id)| format!("<th align=\"{}\">{}</th>", if i >= 5 { "right" } else { "left" }, escape(&tr.text(id, None)))).collect();
    let unknown = tr.text("receipts-gross-unknown", None);
    let rows: String = book
        .rows
        .iter()
        .map(|r| {
            let means = if r.cash { format!("{} ({})", escape(&r.means), escape(&tr.text("books-cash", None))) } else { escape(&r.means) };
            let amount = match (r.amount, &r.foreign) {
                (Some(a), _) => money(a),
                (None, Some((m, code))) => format!("<span class=\"muted\">{}</span>", escape(&tr.money_in(crate::money::Money(*m), code))),
                (None, None) => format!("<span class=\"muted\">{}</span>", escape(&unknown)),
            };
            let mut cells = format!("<td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{means}</td><td align=\"right\">{amount}</td>", escape(&tr.day_month(r.day)), escape(&r.client), escape(&nature_words(tr, &r.nature)), escape(&r.invoice));
            if taxed {
                cells.push_str(&format!("<td align=\"right\">{}</td>", r.without_tax.map(money).unwrap_or_default()));
            }
            if mixed {
                cells.push_str(&format!("<td align=\"right\">{}</td><td align=\"right\">{}</td>", r.split().map(|(g, _)| money(g)).unwrap_or_default(), r.split().map(|(_, s)| money(s)).unwrap_or_default()));
            }
            format!("<tr>{cells}</tr>")
        })
        .collect();
    let totals: String = book
        .totals
        .iter()
        .map(|t| {
            let mut cells = format!("<td colspan=\"5\">{}</td><td align=\"right\">{}</td>", escape(&total_name(tr, t)), money(t.amount));
            if taxed {
                cells.push_str(&format!("<td align=\"right\">{}</td>", money(t.without_tax)));
            }
            if mixed {
                cells.push_str(&format!("<td align=\"right\">{}</td><td align=\"right\">{}</td>", money(t.goods), money(t.services)));
            }
            format!("<tr class=\"total\">{cells}</tr>")
        })
        .collect();
    let said: String = sentences(tr, book).iter().map(|s| format!("<p>{}</p>", escape(s))).collect();
    format!(
        "<html><head><meta charset=\"utf-8\"><style>\
         body {{ font-family: sans-serif; font-size: 10pt; color: #222; }}\
         h1 {{ font-size: 18pt; font-weight: normal; margin: 0 0 8pt 0; }}\
         table {{ border-collapse: collapse; margin-top: 12pt; width: 100%; }}\
         th {{ border-bottom: 1px solid #888; font-weight: normal; color: #555; }}\
         td {{ border-bottom: 1px solid #ddd; font-variant-numeric: tabular-nums; }}\
         tr.total td {{ border-top: 1px solid #888; font-weight: bold; }}\
         .muted {{ color: #555; }}\
         </style></head><body><h1>{}</h1>{said}<table><tr>{head}</tr>{rows}{totals}</table></body></html>",
        escape(&title)
    )
}

// ---------------------------------------------------------------- written in the invoices' life

/// What a payment in another currency came to in the books' (the euros
/// that reached you), said in this `device`'s life: the books count it so.
pub fn book_payment(dir: &Path, device: &str, payment: &str, amount: i64, at_ms: i64) -> Result<(), String> {
    lives::change_own(dir, device, |book| book.booked.push(lives::Booked { id: lives::new_id(), payment: payment.to_string(), amount, at: at_ms }))
}

/// The books' notices of `keys` marked told, on `day`, in this `device`'s
/// life: told once, whichever device told them (`thresholds::to_tell`).
pub fn mark_told(dir: &Path, device: &str, keys: &[String], day: Date) -> Result<(), String> {
    if keys.is_empty() {
        return Ok(());
    }
    lives::change_own(dir, device, |book| {
        for key in keys {
            if !book.told.iter().any(|t| t.key == *key) {
                book.told.push(lives::Told { key: key.clone(), date: day.to_string() });
            }
        }
    })
}

/// The preset's word for a VAT category, as the summary names its lines.
pub fn category_words(tr: &Translator, category: &str, rate: i64) -> String {
    match category {
        "S" => {
            let mut args = crate::i18n::args();
            args.set("rate", tr.percent_bp(rate));
            tr.text("books-vat-rate", Some(&args))
        }
        "AE" | "K" | "G" | "O" | "E" | "Z" => tr.text(&format!("books-vat-category-{}", category.to_lowercase()), None),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests;
