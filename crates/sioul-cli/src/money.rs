// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Money movements from the command line (docs/accounting.md, "Movements";
//! design §3.4): the full ledger, newest first, with the window's filters
//! (`sioul movements`), and each movement shown, made, changed, removed
//! and put back (`sioul movement …`), by the same acts as the window and
//! the agents (`movements::acts`), each said in one line. Nothing here moves
//! money: these are records, never payments.

use crate::Session;
use clap::Subcommand;
use jiff::civil::Date;
use sioul_core::money::Money;
use sioul_core::moneylist::{self, Query, ToldBy};
use sioul_core::movements::{self, Fee, FeeFrom, FeeKind, Way, acts};
use std::path::PathBuf;

/// The ledger's filters (`sioul movements`).
#[derive(clap::Args)]
pub(crate) struct MovementsArgs {
    /// From this day (2026-10-01).
    #[arg(long)]
    from: Option<String>,
    /// To this day, included.
    #[arg(long)]
    to: Option<String>,
    /// A bank account, by its id.
    #[arg(long)]
    account: Option<String>,
    /// A budget, by its id.
    #[arg(long)]
    budget: Option<String>,
    /// Money in only.
    #[arg(long = "in", conflicts_with = "out")]
    money_in: bool,
    /// Money out only.
    #[arg(long)]
    out: bool,
    /// What it is for: work, admin, leisure, joined by commas.
    #[arg(long)]
    area: Option<String>,
    /// At least this much, whatever its sign.
    #[arg(long)]
    min: Option<String>,
    /// At most this much.
    #[arg(long)]
    max: Option<String>,
    /// Words of who, the note, the budget or the amount; they reach every area.
    #[arg(long)]
    search: Option<String>,
    /// What told of it: bank, mail, plan, hand (several by commas).
    #[arg(long)]
    told_by: Option<String>,
    /// The page, from 1 (60 movements each).
    #[arg(long, default_value_t = 1)]
    page: usize,
    /// As JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand)]
pub(crate) enum MovementCommand {
    /// One movement: every source, tie, fee and where it was read, when it was told and changed.
    Show {
        address: String,
        #[arg(long)]
        json: bool,
    },
    /// A movement of your own: a line of the budget file.
    Add {
        /// Who paid, or who was paid.
        #[arg(long)]
        who: String,
        /// What the payer paid, or the price: + in, − out.
        #[arg(long, allow_hyphen_values = true)]
        gross: Option<String>,
        /// What moved the account, or will: + in, − out.
        #[arg(long, allow_hyphen_values = true)]
        net: Option<String>,
        /// A fee, as KIND=N: processor, commission, conversion, contributions, bank.
        #[arg(long)]
        fee: Vec<String>,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        budget: String,
        /// Its day (2026-11-05).
        #[arg(long)]
        on: String,
        /// The day it was paid at its processor, when not its day.
        #[arg(long)]
        paid_on: Option<String>,
        /// To come.
        #[arg(long)]
        planned: bool,
        #[arg(long)]
        note: Option<String>,
        /// What it is tied to, by its address (sioul:project/…, sioul:contact/…).
        #[arg(long)]
        tie: Vec<String>,
    },
    /// Changes what is given, as its kind allows (never the bank's own amount or day).
    Change {
        address: String,
        #[arg(long)]
        who: Option<String>,
        #[arg(long)]
        budget: Option<String>,
        /// What was paid; "none" when it is not known.
        #[arg(long, allow_hyphen_values = true)]
        gross: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        net: Option<String>,
        /// Its fees, in place of those said, as KIND=N; "none" for none.
        #[arg(long)]
        fee: Vec<String>,
        #[arg(long)]
        on: Option<String>,
        /// The day it was paid; "none": its day.
        #[arg(long)]
        paid_on: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        planned: Option<bool>,
        #[arg(long)]
        account: Option<String>,
    },
    /// Removes it as its kind says: your line deleted, a recurring payment not this time, the bank's line left out.
    Remove { address: String },
    /// Puts back what was removed as "not this time" or "left out".
    Back { address: String },
}

fn root(s: &Session) -> Result<PathBuf, String> {
    s.config.notes_root_path().filter(|r| r.join(sioul_core::budget::LEDGER).is_file()).ok_or_else(|| s.tr.text("error-no-ledger", None))
}

fn day(text: &str) -> Result<Date, String> {
    text.trim().parse().map_err(|_| format!("{}: a day, as 2026-11-05.", crate::one_line(text)))
}

fn amount(text: &str) -> Result<Money, String> {
    sioul_core::bank::parse_amount(text).ok_or_else(|| format!("{}: an amount, as -52 or 520.00.", crate::one_line(text)))
}

/// "processor=15.43": a fee by your hand.
pub(crate) fn fee(text: &str) -> Result<Fee, String> {
    let (kind, value) = text.split_once('=').ok_or_else(|| format!("{}: a fee, as processor=1.86.", crate::one_line(text)))?;
    Ok(Fee { kind: FeeKind::parse(kind), amount: amount(value)?.abs(), from: FeeFrom::Hand, about: false })
}

fn print(lines: &[String]) {
    for line in lines {
        println!("{}", crate::one_line(line));
    }
}

/// `sioul movements`: the ledger, newest first, a page at a time.
pub(crate) fn list(s: &Session, args: MovementsArgs) -> Result<(), String> {
    let root = root(s)?;
    let today = sioul_core::zone::now().date();
    let words = sioul_core::words::Words::of(&s.config);
    let books = movements::books(&root, &words, today);
    let told_by: Vec<ToldBy> = args.told_by.as_deref().unwrap_or("").split(',').filter(|w| !w.trim().is_empty()).map(|w| ToldBy::parse(w).ok_or_else(|| format!("{}: bank, mail, plan or hand.", crate::one_line(w)))).collect::<Result<_, _>>()?;
    let query = Query {
        from: args.from.as_deref().map(day).transpose()?,
        to: args.to.as_deref().map(day).transpose()?,
        account: args.account,
        budget: args.budget,
        way: if args.money_in { Some(Way::In) } else if args.out { Some(Way::Out) } else { None },
        areas: args.area.as_deref().and_then(sioul_core::areas::Area::parse),
        at_least: args.min.as_deref().map(amount).transpose()?,
        at_most: args.max.as_deref().map(amount).transpose()?,
        text: args.search.unwrap_or_default(),
        told_by,
        page: args.page.saturating_sub(1),
    };
    let page = moneylist::query(&books, &query);
    if args.json {
        let rows: Vec<serde_json::Value> = page.movements.iter().map(|m| moneylist::data(&books.ledger, m)).collect();
        let total = page.total.map(|t| serde_json::json!({ "gross": t.gross.cents() as f64 / 100.0, "net": t.net.cents() as f64 / 100.0 }));
        println!("{}", serde_json::json!({ "movements": rows, "page": page.page + 1, "more": page.more, "total": total }));
        return Ok(());
    }
    if page.movements.is_empty() {
        println!("{}", s.tr.text("movements-none", None));
    }
    if let Some(total) = &page.total {
        println!("{}", moneylist::total_words(&s.tr, total));
    }
    for m in &page.movements {
        let lines = moneylist::row_words(&s.tr, &books.ledger, m, today);
        print(&lines[..1]);
        print(&lines[1..].iter().map(|l| format!("  {l}")).collect::<Vec<_>>());
    }
    if page.more {
        println!("{}", s.say("movements-more", &[("page", (page.page + 2).to_string())]));
    }
    Ok(())
}

/// What a change says for a fee list: "none" for none.
fn fees_of(given: &[String]) -> Result<Option<Vec<Fee>>, String> {
    if given.is_empty() {
        return Ok(None);
    }
    if given.iter().any(|f| f.trim() == "none") {
        return Ok(Some(Vec::new()));
    }
    given.iter().map(|f| fee(f)).collect::<Result<Vec<_>, _>>().map(Some)
}

pub(crate) fn run(s: &Session, command: MovementCommand) -> Result<(), String> {
    let root = root(s)?;
    let today = sioul_core::zone::now().date();
    let words = sioul_core::words::Words::of(&s.config);
    let origin = s.say("movement-origin", &[("date", today.to_string())]);
    let said = |act: acts::Act| {
        if !act.said.is_empty() {
            println!("{}", act.said);
        }
        println!("{}", act.address);
    };
    match command {
        MovementCommand::Show { address, json } => {
            let books = movements::books(&root, &words, today);
            let (m, gone) = moneylist::get(&books, &address).ok_or_else(|| s.tr.text("movement-gone", None))?;
            if json {
                let mut data = moneylist::data(&books.ledger, m);
                data["removed"] = serde_json::json!(gone);
                println!("{data}");
            } else {
                print(&moneylist::detail_words(&s.tr, &books.ledger, m, today));
            }
        }
        MovementCommand::Add { who, gross, net, fee: fees, account, budget, on, paid_on, planned, note, tie } => {
            let new = acts::New {
                label: who,
                gross: gross.as_deref().map(amount).transpose()?,
                net: net.as_deref().map(amount).transpose()?,
                fees: fees.iter().map(|f| fee(f)).collect::<Result<_, _>>()?,
                account,
                budget,
                day: day(&on)?,
                paid_on: paid_on.as_deref().map(day).transpose()?,
                planned,
                note: note.unwrap_or_default(),
                ties: tie,
            };
            said(acts::make(&root, &s.tr, &new, &origin, today)?);
        }
        MovementCommand::Change { address, who, budget, gross, net, fee: fees, on, paid_on, note, planned, account } => {
            let change = acts::Change {
                label: who,
                gross: gross.as_deref().map(|g| if g.trim() == "none" { Ok(None) } else { amount(g).map(Some) }).transpose()?,
                fees: fees_of(&fees)?,
                net: net.as_deref().map(amount).transpose()?,
                day: on.as_deref().map(day).transpose()?,
                paid_on: paid_on.as_deref().map(|d| if d.trim() == "none" { Ok(None) } else { day(d).map(Some) }).transpose()?,
                note,
                budget,
                planned,
                account,
            };
            said(acts::change(&root, &words, &s.tr, &address, &change, &origin, today)?);
        }
        MovementCommand::Remove { address } => said(acts::remove(&root, &words, &s.tr, &address, today)?),
        MovementCommand::Back { address } => {
            if !acts::put_back(&root, &address)? {
                return Err(s.tr.text("movement-gone", None));
            }
            println!("{address}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fees_are_read_as_kind_and_amount() {
        assert_eq!(fee("processor=1,86").unwrap(), Fee { kind: FeeKind::Processor, amount: Money(186), from: FeeFrom::Hand, about: false });
        assert_eq!(fee("commission=-120").unwrap().amount, Money(12000));
        assert!(fee("1.86").is_err());
        assert_eq!(fees_of(&["none".into()]).unwrap(), Some(Vec::new()));
        assert_eq!(fees_of(&[]).unwrap(), None);
    }
}

/// The history of money mail's filters (`sioul money-mail`).
#[derive(clap::Args)]
pub(crate) struct MoneyMailArgs {
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    /// The address it came to, by its account's id.
    #[arg(long)]
    address: Option<String>,
    /// The bank account of the movement it tells of.
    #[arg(long)]
    account: Option<String>,
    #[arg(long)]
    budget: Option<String>,
    /// Only mail tied to a movement.
    #[arg(long, conflicts_with = "untied")]
    tied: bool,
    /// Only mail tied to none.
    #[arg(long)]
    untied: bool,
    #[arg(long = "in", conflicts_with = "out")]
    money_in: bool,
    #[arg(long)]
    out: bool,
    #[arg(long)]
    min: Option<String>,
    #[arg(long)]
    max: Option<String>,
    /// What it is for: work, admin, leisure, joined by commas.
    #[arg(long)]
    area: Option<String>,
    /// received, paid, order, bill or refund.
    #[arg(long)]
    kind: Option<String>,
    #[arg(long)]
    search: Option<String>,
    #[arg(long, default_value_t = 1)]
    page: usize,
    #[arg(long)]
    json: bool,
}

/// A kind of money mail by its word.
pub(crate) fn kind_of(word: &str) -> Result<sioul_core::payments::PaymentKind, String> {
    use sioul_core::payments::PaymentKind;
    [PaymentKind::Received, PaymentKind::Paid, PaymentKind::Order, PaymentKind::Bill, PaymentKind::Refund]
        .into_iter()
        .find(|k| k.message_id().trim_start_matches("kind-") == word.trim())
        .ok_or_else(|| format!("{}: received, paid, order, bill or refund.", crate::one_line(word)))
}

/// `sioul money-mail`: every money mail kept on this computer, newest first, and what became of each.
pub(crate) fn money_mail(s: &Session, args: MoneyMailArgs) -> Result<(), String> {
    let root = root(s)?;
    let today = sioul_core::zone::now().date();
    let words = sioul_core::words::Words::of(&s.config);
    let books = movements::books(&root, &words, today);
    let store = movements::Store::load(&root).unwrap_or_default();
    let ignored = sioul_core::state::MoneyState::load(&sioul_core::state::MoneyState::default_path()).ignored;
    let query = sioul_core::moneyindex::MailQuery {
        from: args.from.as_deref().map(day).transpose()?,
        to: args.to.as_deref().map(day).transpose()?,
        address: args.address,
        account: args.account,
        budget: args.budget,
        tied: if args.tied { Some(true) } else if args.untied { Some(false) } else { None },
        way: if args.money_in { Some(Way::In) } else if args.out { Some(Way::Out) } else { None },
        at_least: args.min.as_deref().map(amount).transpose()?,
        at_most: args.max.as_deref().map(amount).transpose()?,
        areas: args.area.as_deref().and_then(sioul_core::areas::Area::parse),
        kind: args.kind.as_deref().map(kind_of).transpose()?,
        text: args.search.unwrap_or_default(),
        page: args.page.saturating_sub(1),
    };
    let page = sioul_core::moneyindex::history(&s.config, &books, &store, &ignored, &query, usize::MAX);
    if args.json {
        println!("{}", serde_json::json!({ "mail": page.rows, "page": page.page + 1, "more": page.more }));
        return Ok(());
    }
    if page.rows.is_empty() {
        println!("{}", s.tr.text("money-mail-none", None));
    }
    for row in &page.rows {
        let lines = sioul_core::moneyindex::row_words(&s.tr, &books.ledger, row);
        print(&lines[..1]);
        print(&lines[1..].iter().map(|l| format!("  {l}")).collect::<Vec<_>>());
        println!("  {}", row.key);
    }
    if page.more {
        println!("{}", s.say("movements-more", &[("page", (page.page + 2).to_string())]));
    }
    Ok(())
}

/// The receipts register (`sioul receipts`).
#[derive(clap::Args)]
pub(crate) struct ReceiptsArgs {
    /// From this day; with --to, the days asked; else the declaration period that holds today.
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    /// The work budget, by its id; else the only one that declares.
    #[arg(long)]
    budget: Option<String>,
    /// As CSV, for a spreadsheet or an accountant.
    #[arg(long, conflicts_with = "html")]
    csv: bool,
    /// As a page to print (the window prints it to PDF, as it prints invoices).
    #[arg(long)]
    html: bool,
}

/// `sioul receipts`: what a work budget received, as the declaration counts it.
pub(crate) fn receipts(s: &Session, args: ReceiptsArgs) -> Result<(), String> {
    let root = root(s)?;
    let today = sioul_core::zone::now().date();
    let words = sioul_core::words::Words::of(&s.config);
    let books = movements::books(&root, &words, today);
    let declaring: Vec<&sioul_core::budget::Budget> = books.ledger.budgets.iter().filter(|b| sioul_core::receipts::declares(b)).collect();
    let budget = match &args.budget {
        Some(id) => books.ledger.budgets.iter().find(|b| b.id == *id).ok_or_else(|| s.tr.text("movement-no-budget", None))?,
        None if declaring.len() == 1 => declaring[0],
        None => return Err(s.tr.text("receipts-which-budget", None)),
    };
    let register = match (args.from.as_deref().map(day).transpose()?, args.to.as_deref().map(day).transpose()?) {
        (Some(from), Some(to)) => sioul_core::receipts::register(&books, &budget.id, from, to, None),
        (Some(from), None) => sioul_core::receipts::register(&books, &budget.id, from, today, None),
        (None, _) => sioul_core::receipts::of_period(&books, budget, today),
    };
    if args.csv {
        print!("{}", sioul_core::receipts::csv(&s.tr, &register));
    } else if args.html {
        println!("{}", sioul_core::receipts::html(&s.tr, &register, &budget.title));
    } else {
        print(&sioul_core::receipts::sentences(&s.tr, &register));
        let unknown = s.tr.text("receipts-gross-unknown", None);
        for r in &register.receipts {
            let gross = r.gross.map_or_else(|| unknown.clone(), |g| s.tr.money(g));
            println!("  {} · {} · {} · {}{}", sioul_core::moneyview::short_day(&s.tr, r.day), crate::one_line(&r.client), r.means.words(&s.tr), gross, if r.invoice.is_empty() { String::new() } else { format!(" · {}", r.invoice) });
        }
    }
    Ok(())
}
