// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul books`: the books from the terminal (docs/projects.md, "The
//! books"; `sioul_core::books`): the receipts book from the invoices'
//! payments and the budgets that declare, with its quarters' and years'
//! totals, as CSV or a page to print; the declarations of turnover, each
//! with its day and the contributions estimated; the VAT summary, for an
//! issuer liable to VAT; the thresholds watched; the DES. Read only:
//! nothing is declared, filed or written.

use crate::Session;
use jiff::civil::Date;
use sioul_core::books::{self, declarations, des, thresholds, vat};

#[derive(clap::Subcommand)]
pub(crate) enum BooksCommand {
    /// The receipts book: one row per payment received, by its day, with its quarters' and years' totals.
    Receipts {
        /// The book of this year; the current year's when unsaid.
        #[arg(long, conflicts_with_all = ["from", "to"])]
        year: Option<i16>,
        /// From this day; with --to, the days asked.
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        /// As CSV, for a spreadsheet or an accountant.
        #[arg(long, conflicts_with = "html")]
        csv: bool,
        /// As a page to print (the window prints it to PDF).
        #[arg(long)]
        html: bool,
    },
    /// The declarations of turnover of a year, until today's: what to declare, by when, the contributions estimated.
    Declarations {
        /// The current year's when unsaid.
        #[arg(long)]
        year: Option<i16>,
    },
    /// The VAT summary of a year, period by period and rate by rate, for an issuer liable to VAT; nothing is filed.
    Vat {
        /// The current year's when unsaid.
        #[arg(long)]
        year: Option<i16>,
    },
    /// The thresholds watched this year, and where the turnover stands.
    Thresholds,
    /// The DES of a year: services to businesses in other EU countries, month by month, with their invoices.
    Des {
        /// The current year's when unsaid.
        #[arg(long)]
        year: Option<i16>,
    },
}

pub(crate) fn run(s: &Session, command: Option<BooksCommand>) -> Result<(), String> {
    let today = sioul_core::zone::now().date();
    let loaded = books::load(&s.config, &sioul_core::invoice::folder(), today);
    let input = loaded.input();
    let currency = loaded.settings.currency.clone();
    let year_of = |year: Option<i16>| year.unwrap_or(today.year());
    let bounds = |year: i16| (Date::new(year, 1, 1).unwrap_or(today), Date::new(year, 12, 31).unwrap_or(today).min(today));
    match command {
        None => {
            // Now: the declarations to make, this year's thresholds, the DES to make, the period's VAT.
            let mut said = Vec::new();
            let open = declarations::open_now(&input, today, 60, &s.tr);
            for d in open.iter().chain(declarations::holding(&input, today, &s.tr).iter()) {
                said.extend(declarations::sentences(&s.tr, d, &currency));
            }
            for d in des::open_now(&input, today, 60) {
                said.extend(des::sentences(&s.tr, &d, &currency));
            }
            for summary in vat::between(&input, today, today) {
                said.extend(vat::sentences(&s.tr, &summary, &currency));
            }
            said.extend(thresholds::watch(&input, today, &s.tr).iter().map(|w| thresholds::sentence(&s.tr, w, &currency)));
            let declaring = loaded.money.as_ref().is_some_and(|m| m.ledger.budgets.iter().any(sioul_core::receipts::declares));
            if loaded.invoices.is_empty() && !declaring {
                said.push(s.tr.text("books-none", None));
            } else {
                // The year so far.
                let (from, to) = bounds(today.year());
                let book = books::book(&input, from, to, &s.tr);
                said.extend(book.totals.iter().filter(|t| t.kind == "year").map(|t| books::total_sentence(&s.tr, t, &currency)));
            }
            print(&said);
        }
        Some(BooksCommand::Receipts { year, from, to, csv, html }) => {
            let (from, to) = match (from.as_deref().map(day).transpose()?, to.as_deref().map(day).transpose()?) {
                (Some(from), Some(to)) => (from, to),
                (Some(from), None) => (from, today),
                (None, _) => bounds(year_of(year)),
            };
            let book = books::book(&input, from, to, &s.tr);
            if csv {
                print!("{}", books::csv(&s.tr, &book));
            } else if html {
                let who = if s.config.invoice.name.trim().is_empty() { s.tr.text("books-name", None) } else { s.config.invoice.name.trim().to_string() };
                println!("{}", books::html(&s.tr, &book, &who));
            } else {
                print(&books::sentences(&s.tr, &book));
                for r in &book.rows {
                    println!("  {}", crate::one_line(&row_words(s, r, &currency)));
                }
            }
        }
        Some(BooksCommand::Declarations { year }) => {
            let all = declarations::of_year(&input, year_of(year), today, &s.tr);
            if all.is_empty() {
                println!("{}", s.tr.text("books-cli-no-declaration", None));
            }
            for d in all {
                print(&declarations::sentences(&s.tr, &d, &currency));
                println!();
            }
        }
        Some(BooksCommand::Vat { year }) => {
            let (from, to) = bounds(year_of(year));
            let all = vat::between(&input, from, to);
            if all.is_empty() {
                println!("{}", s.tr.text("books-cli-no-vat", None));
            } else {
                for summary in &all {
                    print(&vat::sentences(&s.tr, summary, &currency));
                }
                println!("{}", s.tr.text("books-vat-files-nothing", None));
            }
        }
        Some(BooksCommand::Thresholds) => {
            let watched = thresholds::watch(&input, today, &s.tr);
            if watched.is_empty() {
                println!("{}", s.tr.text("books-cli-no-threshold", None));
            }
            for w in &watched {
                println!("{}", crate::one_line(&thresholds::sentence(&s.tr, w, &currency)));
            }
        }
        Some(BooksCommand::Des { year }) => {
            let (from, to) = bounds(year_of(year));
            let all = des::between(&input, from, to);
            if all.is_empty() {
                println!("{}", s.tr.text("books-cli-no-des", None));
            }
            for d in &all {
                print(&des::sentences(&s.tr, d, &currency));
            }
        }
    }
    Ok(())
}

/// A row of the book, on a line: day · client · what was received · its invoice, nature and means.
pub(crate) fn row_words(s: &Session, r: &books::Row, currency: &str) -> String {
    let amount = match (r.amount, &r.foreign) {
        (Some(a), _) => s.tr.money_in(sioul_core::money::Money(a), currency),
        (None, Some((m, code))) => format!("{} ({})", s.tr.money_in(sioul_core::money::Money(*m), code), s.tr.text("books-cli-not-counted", None)),
        (None, None) => s.tr.text("receipts-gross-unknown", None),
    };
    let without = r.without_tax.filter(|w| Some(*w) != r.amount).map(|w| s.say("books-row-without", &[("amount", s.tr.money_in(sioul_core::money::Money(w), currency))]));
    let means = if r.cash { format!("{} ({})", r.means, s.tr.text("books-cash", None)) } else { r.means.clone() };
    [sioul_core::moneyview::short_day(&s.tr, r.day), r.client.clone(), amount, r.invoice.clone(), books::nature_words(&s.tr, &r.nature), means, without.unwrap_or_default()].into_iter().filter(|w| !w.trim().is_empty()).collect::<Vec<_>>().join(" · ")
}

fn day(text: &str) -> Result<Date, String> {
    text.trim().parse().map_err(|_| format!("{}: a day, as 2026-11-05.", crate::one_line(text)))
}

fn print(lines: &[String]) {
    for line in lines {
        println!("{}", crate::one_line(line));
    }
}
