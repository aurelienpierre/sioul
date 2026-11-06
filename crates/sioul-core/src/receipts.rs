// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The declaration view and the receipts register (the money ledger's
//! design §6, docs/accounting.md, "The declaration"): what a work budget
//! that declares received in a declaration's period, counted as the
//! declaration counts it.
//!
//! - **The gross, on the day it was paid.** A micro-entrepreneur declares
//!   what clients paid, before any fee (CGI art. 102 ter; URSSAF), received
//!   when the processor or the platform received it, even when it reaches
//!   the bank later (BOFiP, BOI-BNC-BASE-20-10-10 §80): each movement by its
//!   day paid (`Movement::paid_day`), each payment a payout carried by its own.
//! - **The register** (Service-public, F23266): in the order of the days,
//!   each receipt's day, its client, its invoice, its means of payment and
//!   what was paid; saved as CSV here, and as a PDF by the window, which
//!   prints its HTML as it prints invoices (`html`).
//! - **Nothing guessed:** a receipt whose gross is not known (a payout whose
//!   fees are not known yet) says so, and the total then reads "at least".

use crate::budget::Budget;
use crate::i18n::Translator;
use crate::money::Money;
use crate::movements::{Books, FeeKind, Kind, Movement, Place, State, Way};
use jiff::civil::Date;
use jiff::ToSpan;

/// A declaration's period and the day it is declared by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub from: Date,
    pub to: Date,
    /// The last day of the next month, or of the month after the quarter (`movements::declaration_day`).
    pub due: Date,
}

/// Whether a budget declares: a work budget that sets money aside, or says how often it declares.
pub fn declares(budget: &Budget) -> bool {
    budget.area_of() == crate::areas::Area::WORK && (budget.set_aside.is_some_and(|s| s > 0.0) || budget.declare.is_some())
}

/// The declaration period that holds `day`: its month, or its quarter (the default).
pub fn period_of(budget: &Budget, day: Date) -> Period {
    let quarterly = budget.declare.as_deref() != Some("month");
    let month = if quarterly { (day.month() - 1) / 3 * 3 + 1 } else { day.month() };
    let from = Date::new(day.year(), month, 1).unwrap_or(day);
    let to = if quarterly { from.checked_add(2.months()).map_or(day, |d| d.last_of_month()) } else { from.last_of_month() };
    Period { from, to, due: crate::movements::declaration_day(day, quarterly) }
}

/// How a receipt was paid.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Means {
    /// Through a processor or a platform, by its name: "PayPal".
    Processor(String),
    /// A transfer the bank's line says.
    Transfer,
    /// A line of yours: as you wrote it.
    Hand,
}

impl Means {
    pub fn words(&self, tr: &Translator) -> String {
        match self {
            Means::Processor(name) => name.clone(),
            Means::Transfer => tr.text("receipts-means-transfer", None),
            Means::Hand => tr.text("receipts-means-hand", None),
        }
    }
}

/// One receipt of the register.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Receipt {
    /// The day it was paid.
    pub day: Date,
    pub client: String,
    /// The invoice it pays, by its number; "" for none.
    pub invoice: String,
    /// How it was paid.
    pub means: Means,
    /// What the client paid; none when not known yet.
    pub gross: Option<Money>,
    /// What reached your accounts, and the fees kept.
    pub net: Money,
    pub fees: Money,
    /// Its movement.
    pub address: String,
}

/// What a budget received in a period: its receipts, and what they add up to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Register {
    pub budget: String,
    pub from: Date,
    pub to: Date,
    pub due: Option<Date>,
    pub receipts: Vec<Receipt>,
    /// What the clients paid: at least this, when one receipt's gross is not known (`at_least`).
    pub gross: Money,
    pub net: Money,
    pub fees: Money,
    pub at_least: bool,
}

/// The processor a movement went through, by its account or its bank line's words.
fn processor_of<'a>(books: &Books, m: &Movement) -> Option<&'a crate::processors::Processor> {
    let processors = crate::processors::known();
    let by_account = m.account.as_deref().and_then(|a| books.ledger.bank_accounts.iter().find(|b| b.id == a)).and_then(|a| processors.by_id(a.kind()));
    by_account.or_else(|| processors.by_label(&m.label))
}

/// This budget's share of a movement: the whole, or its part there.
fn share(m: &Movement, budget: &str) -> Option<Money> {
    match &m.place {
        Place::Budget(b) if b == budget => Some(m.sum.amount()),
        Place::Reserve { budget: b, .. } if b == budget => Some(m.sum.amount()),
        Place::Parts(parts) => parts.iter().find(|(b, _)| b == budget).map(|(_, a)| *a),
        _ => None,
    }
}

/// What `budget` received from `from` to `to`, each receipt by the day it
/// was paid: the money in that has come (done), each payment a payout
/// carried by its own day and client; the oldest first.
pub fn register_rows(books: &Books, budget: &str, from: Date, to: Date, due: Option<Date>) -> Register {
    let mut receipts: Vec<Receipt> = Vec::new();
    let within = |d: Date| d >= from && d <= to;
    for m in books.movements.iter().filter(|m| m.way == Way::In && matches!(m.state, State::Done) && !m.sources.iter().any(|s| s.kind == Kind::SetAside)) {
        let Some(part) = share(m, budget).filter(|p| p.cents() > 0) else { continue };
        let processor = processor_of(books, m);
        let means = || processor.map_or_else(|| if m.sources.iter().any(|s| s.kind == Kind::Bank) { Means::Transfer } else { Means::Hand }, |p| Means::Processor(p.name.clone()));
        let invoice = m.sources.iter().find(|s| s.kind == Kind::Invoice).map(|s| crate::links::id_of(&s.key)).unwrap_or_default();
        // A payout of a processor whose export you do not take in: each payment it carried, on its own day.
        if m.carried.iter().any(|c| c.how != "export") {
            for c in m.carried.iter().filter(|c| within(c.day)) {
                let fees = c.fee.map_or(Money::ZERO, |f| f.amount);
                receipts.push(Receipt { day: c.day, client: c.label.clone(), invoice: String::new(), means: means(), gross: Some(c.gross), net: c.gross - fees, fees, address: m.address() });
            }
            continue;
        }
        if !within(m.paid_day()) {
            continue;
        }
        let whole = m.sum.amount();
        let fees: Money = m.fees.iter().filter(|f| f.kind != FeeKind::Contributions && !f.about).map(|f| f.amount).sum();
        // Gross known; else the net when nothing was taken on the way; else
        // not known yet: a processor's payout that says no fee, figures that do not add up.
        let gross = match m.gross.filter(|_| m.gross_currency.is_none()) {
            _ if m.fees_unknown => None,
            Some(g) => Some(g),
            None if processor.is_some() && fees == Money::ZERO => None,
            None => Some(whole + fees),
        };
        // In parts: this budget's share of each figure.
        let scale = |amount: Money| if whole == part || whole == Money::ZERO { amount } else { Money((amount.cents() as f64 * part.cents() as f64 / whole.cents() as f64).round() as i64) };
        receipts.push(Receipt { day: m.paid_day(), client: m.label.clone(), invoice, means: means(), gross: gross.map(scale), net: part, fees: scale(fees), address: m.address() });
    }
    receipts.sort_by(|a, b| (a.day, &a.client).cmp(&(b.day, &b.client)));
    let at_least = receipts.iter().any(|r| r.gross.is_none());
    let gross = receipts.iter().map(|r| r.gross.unwrap_or(r.net)).sum();
    let net = receipts.iter().map(|r| r.net).sum();
    let fees = receipts.iter().map(|r| r.fees).sum();
    Register { budget: budget.to_string(), from, to, due, receipts, gross, net, fees, at_least }
}

/// The register of the declaration period that holds `day`.
pub fn of_period(books: &Books, budget: &Budget, day: Date) -> Register {
    let p = period_of(budget, day);
    register_rows(books, &budget.id, p.from, p.to, Some(p.due))
}

fn long_day(tr: &Translator, d: Date) -> String {
    format!("{} {}", tr.day_month(d), d.year())
}

/// What the declaration view says: "Turnover received from 1 October to 31
/// December: €6,940, paid by clients. €6,735 reached your accounts after
/// €205 of fees. To declare by 31 January."
pub fn sentences(tr: &Translator, r: &Register) -> Vec<String> {
    let mut args = crate::i18n::args();
    args.set("from", long_day(tr, r.from));
    args.set("to", long_day(tr, r.to));
    args.set("gross", tr.money(r.gross));
    args.set("net", tr.money(r.net));
    args.set("fees", tr.money(r.fees));
    let mut out = vec![tr.text(if r.at_least { "receipts-turnover-at-least" } else { "receipts-turnover" }, Some(&args))];
    if r.fees.cents() > 0 {
        out.push(tr.text("receipts-reached", Some(&args)));
    }
    if let Some(due) = r.due {
        args.set("due", long_day(tr, due));
        out.push(tr.text("receipts-declare-by", Some(&args)));
    }
    out
}

/// Overview's line beside what a work budget sets aside: "Turnover received
/// this quarter: €6,940, to declare by 31 January." None for a budget that does not declare.
pub fn period_line(books: &Books, tr: &Translator, budget: &Budget, today: Date) -> Option<String> {
    if !declares(budget) {
        return None;
    }
    let r = of_period(books, budget, today);
    let mut args = crate::i18n::args();
    args.set("gross", tr.money(r.gross));
    args.set("due", long_day(tr, r.due.unwrap_or(r.to)));
    let quarterly = budget.declare.as_deref() != Some("month");
    let id = match (quarterly, r.at_least) {
        (true, false) => "receipts-this-quarter",
        (true, true) => "receipts-this-quarter-at-least",
        (false, false) => "receipts-this-month",
        (false, true) => "receipts-this-month-at-least",
    };
    Some(tr.text(id, Some(&args)))
}

/// An amount as a spreadsheet reads it in the language's own way: "520.00", "520,00".
fn plain_amount(tr: &Translator, m: Money) -> String {
    let (units, cents) = m.split();
    let sign = if m.is_negative() { "-" } else { "" };
    format!("{sign}{units}{}{cents:02}", tr.text("decimal-separator", None))
}

/// The register as CSV: its header in your language, one receipt a line,
/// in the order of the days, its total last. Days as 2026-10-05; amounts
/// with the language's decimal mark, and the language's separator (";"
/// where the decimal mark is a comma).
pub fn csv(tr: &Translator, r: &Register) -> String {
    let separator = if tr.text("decimal-separator", None) == "," { ';' } else { ',' };
    let cell = |text: &str| {
        let text = text.replace(['\n', '\r'], " ");
        if text.contains([separator, '"']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text }
    };
    let mut out = String::new();
    let header = ["receipts-column-day", "receipts-column-client", "receipts-column-invoice", "receipts-column-means", "receipts-column-gross", "receipts-column-note"].map(|id| cell(&tr.text(id, None)));
    out.push_str(&header.join(&separator.to_string()));
    out.push_str("\r\n");
    let unknown = tr.text("receipts-gross-unknown", None);
    for receipt in &r.receipts {
        let row = [receipt.day.to_string(), cell(&receipt.client), cell(&receipt.invoice), cell(&receipt.means.words(tr)), receipt.gross.map(|g| plain_amount(tr, g)).unwrap_or_default(), if receipt.gross.is_none() { cell(&unknown) } else { String::new() }];
        out.push_str(&row.join(&separator.to_string()));
        out.push_str("\r\n");
    }
    let total = [String::new(), cell(&tr.text("receipts-column-total", None)), String::new(), String::new(), plain_amount(tr, r.gross), if r.at_least { cell(&tr.text("receipts-at-least", None)) } else { String::new() }];
    out.push_str(&total.join(&separator.to_string()));
    out.push_str("\r\n");
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The register as a page to print (the window prints it to PDF, as it prints invoices).
pub fn html(tr: &Translator, r: &Register, title_of_budget: &str) -> String {
    let mut args = crate::i18n::args();
    args.set("from", long_day(tr, r.from));
    args.set("to", long_day(tr, r.to));
    args.set("budget", title_of_budget.to_string());
    let title = tr.text("receipts-title", Some(&args));
    let head: String = ["receipts-column-day", "receipts-column-client", "receipts-column-invoice", "receipts-column-means", "receipts-column-gross"].iter().map(|id| format!("<th align=\"left\">{}</th>", escape(&tr.text(id, None)))).collect();
    let unknown = tr.text("receipts-gross-unknown", None);
    let rows: String = r
        .receipts
        .iter()
        .map(|x| {
            let gross = x.gross.map_or_else(|| format!("<span class=\"muted\">{}</span>", escape(&unknown)), |g| escape(&tr.money(g)));
            format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td align=\"right\">{gross}</td></tr>", escape(&tr.day_month(x.day)), escape(&x.client), escape(&x.invoice), escape(&x.means.words(tr)))
        })
        .collect();
    let said: String = sentences(tr, r).iter().map(|s| format!("<p>{}</p>", escape(s))).collect();
    format!(
        "<html><head><meta charset=\"utf-8\"><style>\
         body {{ font-family: sans-serif; font-size: 11pt; color: #222; }}\
         h1 {{ font-size: 18pt; font-weight: normal; margin: 0 0 8pt 0; }}\
         table {{ border-collapse: collapse; margin-top: 12pt; width: 100%; }}\
         th {{ border-bottom: 1px solid #888; font-weight: normal; color: #555; }}\
         td {{ border-bottom: 1px solid #ddd; font-variant-numeric: tabular-nums; }}\
         .muted {{ color: #555; }}\
         </style></head><body><h1>{}</h1>{said}<table><tr>{head}</tr>{rows}</table></body></html>",
        escape(&title)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movements::{Input, Store, derive_with_gone};

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    /// The declaration's figures (design §6): the gross by the day paid at
    /// the processor, a payment paid in December and reaching the bank in
    /// January counted in December's quarter; a payout's fees not known make
    /// its row "not known yet" and the total "at least"; the register as CSV
    /// in French; the sentence; the quarter and its day.
    #[test]
    fn the_declaration_counts_the_gross_on_the_day_paid() {
        let ledger: crate::budget::Ledger = toml::from_str(
            "[[budget]]\nid = 'work'\ntitle = 'Work'\nperiod = 'month'\narea = 'work'\nset_aside = 22\n\
             [[bank_account]]\nid = 'main'\ntitle = 'My bank'\nfills = ['work']\nexports = ['0123456A']\n\
             [[line]]\nid = 'a'\nbudget = 'work'\ndate = 2027-01-04\npaid_on = 2026-12-28\namount = 504.57\ngross = 520\nfees = [{ kind = 'processor', amount = 15.43 }]\nlabel = 'Client A'\n\
             [[line]]\nid = 'b'\nbudget = 'work'\ndate = 2026-11-30\namount = 300\nlabel = 'Client B'\nplanned = true\nlinks = ['sioul:invoice/2026-007']\n\
             [[line]]\nid = 'c'\nbudget = 'work'\ndate = 2026-09-30\namount = 100\nlabel = 'Client C'\n",
        )
        .unwrap();
        let mut bank = crate::bank::Bank::default();
        bank.movements.push(crate::bank::Movement { account: "0123456A".into(), date: day("2026-11-10"), amount: Money(30000), label: "VIR CLIENT B FACTURE 2026-007".into(), id: "B1".into() });
        bank.movements.push(crate::bank::Movement { account: "0123456A".into(), date: day("2026-12-15"), amount: Money(19385), label: "VIR PAYPAL EUROPE SARL".into(), id: "P1".into() });
        let store = Store::default();
        let words = crate::words::Words::builtin();
        let input = Input { ledger: &ledger, bank: &bank, contracts: &[], letters: &[], store: &store, words: &words };
        let (movements, gone) = derive_with_gone(&input, day("2026-09-01"), day("2027-03-31"), day("2027-01-10"));
        let books = Books { counted: ledger.clone(), ledger: ledger.clone(), bank, movements, gone, from: day("2026-09-01"), to: day("2027-03-31") };
        let work = &ledger.budgets[0];
        assert!(declares(work));
        let p = period_of(work, day("2026-11-15"));
        assert_eq!((p.from, p.to, p.due), (day("2026-10-01"), day("2026-12-31"), day("2027-01-31")));
        let tr = Translator::new("en");
        let r = of_period(&books, work, day("2026-11-15"));
        let rows: Vec<(String, &str, Option<i64>)> = r.receipts.iter().map(|x| (x.day.to_string(), x.client.as_str(), x.gross.map(Money::cents))).collect();
        assert_eq!(rows, vec![("2026-11-10".into(), "Client B", Some(30000)), ("2026-12-15".into(), "VIR PAYPAL EUROPE SARL", None), ("2026-12-28".into(), "Client A", Some(52000))], "{r:#?}");
        assert_eq!((r.receipts[0].invoice.as_str(), r.receipts[0].means.words(&tr)), ("2026-007", "Bank transfer".to_string()));
        assert_eq!(r.receipts[1].means, Means::Processor("PayPal".into()));
        assert!(r.at_least, "PayPal's payout says nothing of what was paid: not known yet");
        assert_eq!(sentences(&tr, &r), vec!["Turnover received from 1 October 2026 to 31 December 2026: at least €1,013.85, paid by clients.".to_string(), "€998.42 reached your accounts after €15.43 of fees.".into(), "To declare by 31 January 2027.".into()]);
        let fr = Translator::new("fr");
        let csv = csv(&fr, &r);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "Jour;Client;Facture;Moyen de paiement;Payé;Note");
        assert_eq!(lines[3], "2026-12-28;Client A;;écrit à la main;520,00;", "{csv}");
        assert!(lines[2].ends_with(";ce que le client a payé n’est pas encore connu"), "{csv}");
        assert!(lines[4].contains("Total") && lines[4].contains("1013,85"), "{csv}");
        assert_eq!(period_line(&books, &tr, work, day("2026-11-15")).as_deref(), Some("Turnover received this quarter: at least €1,013.85, to declare by 31 January 2027."));
        assert!(html(&tr, &r, "Work").contains("Receipts register of Work, 1 October 2026 to 31 December 2026"));
    }
}
