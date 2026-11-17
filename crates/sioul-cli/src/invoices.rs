// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul invoices`: the invoices you issued and your drafts, from the
//! terminal (docs/projects.md, "Invoices"): listed with where each stands,
//! shown, exported as a spreadsheet or a page to print; a draft made from a
//! project's unbilled time, issued by you at the keyboard; a credit note; a
//! payment received. Issuing follows the window's rules: one device numbers
//! at a time (`sioul_sync::lease`), and a device whose sharing leaves
//! invoices out never numbers. With sharing on, the command line numbers
//! only while this computer's window holds the invoices' claim, which it
//! renews with the new number at once.

use crate::{Session, one_line};
use jiff::civil::Date;
use sioul_core::invoice::{self, Invoice, Kind, Payment, Status, life};
use sioul_core::money::Money;

#[derive(clap::Subcommand)]
pub(crate) enum InvoicesCommand {
    /// Every invoice and draft, newest first, with where each stands.
    List(ListArgs),
    /// One invoice or draft, by its number or its draft's id: its lines, its totals, what it still lacks.
    Show { number: String },
    /// Every invoice as a spreadsheet (CSV), one as its page to print, or where its PDF is.
    Export {
        /// The page to print of this invoice (HTML; the window prints it to PDF).
        #[arg(long, conflicts_with = "pdf")]
        html: Option<String>,
        /// Where the window wrote this invoice's PDF, and whether it is there.
        #[arg(long)]
        pdf: Option<String>,
        /// Only this year's.
        #[arg(long)]
        year: Option<i16>,
    },
    /// A draft of a project's billable time not billed yet; nothing is numbered.
    Draft {
        /// The project, by its id.
        project: String,
    },
    /// Issues a draft: numbers it, keeps it, marks its time; you at the keyboard.
    Issue {
        /// The draft's id (`sioul invoices list` gives it).
        draft: String,
    },
    /// A credit note for an invoice, as a draft: all its lines, or those given.
    Credit {
        number: String,
        /// A line to credit, counted from 1; again for more. None: all of them.
        #[arg(long)]
        line: Vec<usize>,
    },
    /// A payment received for an invoice.
    Pay {
        number: String,
        /// The day it came, as 2026-11-16; today when unsaid.
        #[arg(long)]
        day: Option<String>,
        /// What came, as 120.50; what is left to pay when unsaid.
        #[arg(long)]
        amount: Option<String>,
        /// transfer, card, cash, cheque, direct-debit, other.
        #[arg(long, default_value = "transfer")]
        means: String,
        /// For an invoice in another currency: what reached you in the books' (euros), as 92.50.
        #[arg(long)]
        received: Option<String>,
    },
    /// What a payment in another currency came to in the books' (euros): the books count it so.
    Received {
        number: String,
        /// What reached you, as 92.50.
        amount: String,
        /// The payment of that day; else its last one with nothing said yet.
        #[arg(long)]
        day: Option<String>,
    },
}

#[derive(clap::Args, Default)]
pub(crate) struct ListArgs {
    /// Only this client's, by a part of its name.
    #[arg(long)]
    client: Option<String>,
    /// Only this year's.
    #[arg(long)]
    year: Option<i16>,
    /// draft, issued, sent, partly-paid, paid, credited, credit-note.
    #[arg(long)]
    status: Option<String>,
}

pub(crate) fn run(s: &Session, command: Option<InvoicesCommand>) -> Result<(), String> {
    match command.unwrap_or(InvoicesCommand::List(ListArgs::default())) {
        InvoicesCommand::List(args) => list(s, &args),
        InvoicesCommand::Show { number } => show(s, &number),
        InvoicesCommand::Export { pdf: Some(number), .. } => {
            let found = invoice::find_in(&invoice::folder(), &number).ok_or_else(|| s.tr.text("invoice-gone", None))?;
            let path = invoice::pdf_path(&found, &s.config.invoice.folder, &s.tr.text("invoice-folder-name", None));
            println!("{}", path.display());
            if !path.is_file() {
                println!("{}", s.tr.text("invoice-cli-no-pdf", None));
            }
            Ok(())
        }
        InvoicesCommand::Export { html: Some(number), .. } => {
            let found = invoice::find_in(&invoice::folder(), &number).or_else(|| invoice::draft_in(&invoice::folder(), &number)).ok_or_else(|| s.tr.text("invoice-gone", None))?;
            println!("{}", invoice::page(&found, &s.tr));
            Ok(())
        }
        InvoicesCommand::Export { html: None, pdf: None, year } => {
            print!("{}", csv(s, year));
            Ok(())
        }
        InvoicesCommand::Draft { project } => draft(s, &project),
        InvoicesCommand::Issue { draft } => issue(s, &draft),
        InvoicesCommand::Credit { number, line } => credit(s, &number, &line),
        InvoicesCommand::Pay { number, day, amount, means, received } => pay(s, &number, day.as_deref(), amount.as_deref(), &means, received.as_deref()),
        InvoicesCommand::Received { number, amount, day } => received(s, &number, &amount, day.as_deref()),
    }
}

fn today() -> Date {
    sioul_core::zone::now().date()
}

/// An amount as typed ("120.50", "120,50") in cents.
fn cents(text: &str) -> Result<i64, String> {
    let units: f64 = text.trim().replace(',', ".").parse().map_err(|_| format!("{}: an amount, as 120.50.", one_line(text)))?;
    Ok(Money::from_units(units).cents())
}

/// Where an invoice stands, in words; a credit note says what it credits.
fn status_words(s: &Session, i: &Invoice, status: Status) -> String {
    match (&i.corrects, status) {
        (Some(cited), Status::CreditNote) => s.say("invoice-credits", &[("number", cited.number.clone())]),
        _ => s.tr.text(&format!("invoice-status-{}", status.id()), None),
    }
}

/// This device's name, for its own life and journal of the invoices.
fn device() -> String {
    sioul_sync::share::Here::load(&sioul_core::config::state_dir()).id
}

/// An invoice's line in a list: its number (or its draft's id), its day,
/// its kind when not an invoice, its client, its amount, where it stands.
fn row(s: &Session, i: &Invoice, all: &[Invoice]) -> String {
    let status = i.status(all);
    let name = if i.number.is_empty() { s.say("invoice-cli-draft", &[("id", i.draft.clone())]) } else { i.number.clone() };
    let kind = if i.kind == Kind::Invoice { String::new() } else { format!(" · {}", s.tr.text(&format!("invoice-kind-{}", i.kind.id()), None)) };
    let left = i.left(all);
    let due = if matches!(status, Status::Issued | Status::Sent | Status::PartlyPaid) && !i.due.is_empty() { format!(" · {}", s.say("invoice-cli-due", &[("date", i.due.clone())])) } else { String::new() };
    let rest = if status == Status::PartlyPaid { format!(" · {}", s.say("invoice-cli-left", &[("amount", s.tr.money_in(Money(left), &i.currency))])) } else { String::new() };
    format!("{name} · {}{kind} · {} · {} · {}{rest}{due}", if i.date.is_empty() { "—".into() } else { i.date.clone() }, one_line(i.client.first_line()), s.tr.money_in(i.total(), &i.currency), status_words(s, i, status))
}

fn list(s: &Session, args: &ListArgs) -> Result<(), String> {
    let folder = invoice::folder();
    let all = invoice::all_in(&folder);
    let wanted = args.status.as_deref().map(|w| Status::named(w).ok_or_else(|| format!("{}: draft, issued, sent, partly-paid, paid, credited or credit-note.", one_line(w)))).transpose()?;
    let keep = |i: &Invoice| {
        args.client.as_deref().is_none_or(|c| i.client.name.to_lowercase().contains(&c.to_lowercase()))
            && args.year.is_none_or(|y| i.date.get(..4).and_then(|d| d.parse::<i16>().ok()) == Some(y) || (i.date.is_empty() && i.number.is_empty()))
            && wanted.is_none_or(|w| i.status(&all) == w)
    };
    let drafts = invoice::drafts_in(&folder);
    let mut shown = 0;
    for d in drafts.iter().filter(|d| keep(d)) {
        println!("{}", row(s, d, &all));
        shown += 1;
    }
    for i in all.iter().rev().filter(|i| !i.unreadable && keep(i)) {
        println!("{}", row(s, i, &all));
        shown += 1;
    }
    for number in all.iter().filter(|i| i.unreadable).map(|i| &i.number) {
        println!("{}", s.say("invoice-cli-unreadable", &[("number", number.clone())]));
    }
    for gone in invoice::series::missing(&invoice::series::Journal::load(&folder), &all) {
        match gone.number {
            Some(number) => println!("{}", s.say("invoice-cli-missing", &[("number", number)])),
            None => println!("{}", s.say("invoice-cli-missing-in", &[("sequence", gone.sequence), ("n", gone.counter.to_string())])),
        }
    }
    if shown == 0 {
        println!("{}", s.tr.text("invoice-cli-none", None));
    }
    Ok(())
}

fn show(s: &Session, number: &str) -> Result<(), String> {
    let folder = invoice::folder();
    let all = invoice::all_in(&folder);
    let found = all.iter().find(|i| i.number == number && !i.unreadable).cloned().or_else(|| invoice::draft_in(&folder, number)).ok_or_else(|| s.tr.text("invoice-gone", None))?;
    println!("{}", row(s, &found, &all));
    // The client, and whether it is billed as a business or a person (a guess when no profile says it).
    let kind = s.tr.text(&format!("invoice-client-kind-{}", found.client.kind), None);
    println!("  {} ({kind})", one_line(&found.client.name.replace('\n', ", ")));
    match (found.period_from.as_str(), found.period_to.as_str()) {
        ("", _) => {}
        (from, to) if to.is_empty() || from == to => println!("  {}", s.say("invoice-period-day", &[("date", from.to_string())])),
        (from, to) => println!("  {}", s.say("invoice-period", &[("from", from.to_string()), ("to", to.to_string())])),
    }
    for (n, line) in found.lines.iter().enumerate() {
        let quantity = line.minutes().map_or_else(|| s.tr.thousandths(line.quantity), sioul_core::timereport::duration);
        println!("  {}. {} · {quantity} · {}", n + 1, one_line(&line.description), s.tr.money_in(Money(line.amount), &found.currency));
    }
    let t = &found.totals;
    if t.tax != 0 {
        println!("  {} {} · {} {}", s.tr.text("invoice-total-without-tax", None), s.tr.money_in(Money(t.without_tax), &found.currency), s.tr.text("invoice-vat-rate", None), s.tr.money_in(Money(t.tax), &found.currency));
    }
    println!("  {} {}", s.tr.text("invoice-amount-due", None), s.tr.money_in(Money(t.due), &found.currency));
    for p in &found.payments {
        println!("  {}", s.say("invoice-cli-payment", &[("date", p.date.clone()), ("amount", s.tr.money_in(Money(p.amount), &found.currency)), ("means", p.means.clone())]));
    }
    if found.number.is_empty() {
        for check in life::checks(&found, &s.config.invoice, today(), &all) {
            println!("  {} {}", if check.required { "✗" } else { "·" }, check.text);
        }
    }
    Ok(())
}

/// Every invoice as rows of a spreadsheet: in French with ";" and decimal commas, as `timereport::billable_csv` writes them.
fn csv(s: &Session, year: Option<i16>) -> String {
    let all = invoice::all_in(&invoice::folder());
    let french = s.tr.language() == "fr";
    let sep = if french { ';' } else { ',' };
    let amount = |cents: i64| {
        let text = format!("{}.{:02}", cents / 100, (cents % 100).abs());
        let text = if cents < 0 && cents > -100 { format!("-{text}") } else { text };
        if french { text.replace('.', ",") } else { text }
    };
    // Text cells never run as formulas (a client named "=HYPERLINK(…)"), as the time's spreadsheet.
    let quote = |text: &str| sioul_core::timereport::csv_cell(text, sep);
    let head = ["invoice-csv-number", "invoice-csv-kind", "invoice-csv-date", "invoice-csv-client", "invoice-csv-currency", "invoice-csv-without-tax", "invoice-csv-tax", "invoice-csv-due", "invoice-csv-paid", "invoice-csv-status"].map(|id| s.tr.text(id, None));
    let mut out = head.join(&sep.to_string()) + "\n";
    for i in all.iter().filter(|i| !i.unreadable && year.is_none_or(|y| i.date.get(..4).and_then(|d| d.parse::<i16>().ok()) == Some(y))) {
        let text = [i.number.clone(), s.tr.text(&format!("invoice-kind-{}", i.kind.id()), None), i.date.clone(), i.client.first_line().to_string(), i.currency.clone()].map(|c| quote(&c));
        let numbers = [amount(i.totals.without_tax), amount(i.totals.tax), amount(i.totals.due), amount(i.paid())];
        let row: Vec<String> = text.into_iter().chain(numbers).chain(std::iter::once(quote(&status_words(s, i, i.status(&all))))).collect();
        out += &row.join(&sep.to_string());
        out.push('\n');
    }
    out
}

fn draft(s: &Session, id: &str) -> Result<(), String> {
    let root = s.config.notes_root_path().ok_or_else(|| s.tr.text("error-no-store", None))?;
    let store = sioul_core::projects::ProjectStore::load(&root)?;
    let project = store.get(id).ok_or_else(|| s.tr.text("project-gone", None))?.clone();
    let loaded = sioul_core::links::Loaded::read(&s.config);
    let entries = sioul_core::timereport::entries(&sioul_core::timelog::sessions(), &loaded.tasks, &loaded.projects, &sioul_core::zone::current());
    let clients = sioul_core::clients::Clients::load(&root).unwrap_or_default();
    let blank = life::for_client(&s.config.invoice, project.client.as_deref().unwrap_or(""), &clients, &loaded.contacts, s.tr.language(), today());
    let records = invoice::all_in(&invoice::folder());
    let made = life::from_time(&entries, &project, blank, &records, &s.config.invoice, today()).ok_or_else(|| s.tr.text("invoice-nothing", None))?;
    let saved = invoice::save_draft(&invoice::folder(), &made)?;
    println!("{}", s.say("invoice-cli-drafted", &[("id", saved.draft.clone())]));
    show(s, &saved.draft)
}

/// What issuing needs of this computer's sharing, and whether it may number
/// now: sharing off, always; on, only while this computer's window holds the
/// invoices' claim, alive and kept long enough for the others to know it
/// (`sioul_sync::lease`), no other computer having closed with records not
/// read here, the others heard from lately. The command line never claims
/// by itself: a claim it took would lapse unseen as soon as it ends, and
/// another computer could number meanwhile.
fn numbering(s: &Session) -> Result<(life::Numbering, Option<(std::path::PathBuf, [u8; 32])>), String> {
    use sioul_sync::lease;
    use sioul_sync::share;
    let state = sioul_core::config::state_dir();
    let here = share::Here::load(&state);
    let Some(folder) = here.folder_path() else { return Ok((life::Numbering { device: here.id, ..life::Numbering::default() }, None)) };
    let drafts_shared = here.shares("drafts", &s.config);
    let key = sioul_sync::secret::named(share::KEY_NAME).and_then(|k| unhex(&k)).ok_or_else(|| s.tr.text("invoice-unchecked", None))?;
    let claims = lease::claims(&folder, &key, "invoices");
    let now = jiff::Timestamp::now().as_second();
    let keeper = lease::look(&folder, &key, "invoices", &here.id, now, lease::Rule::StaysPut);
    let own = claims.iter().find(|c| c.computer == here.id && !c.closed && c.until > now);
    if own.is_none() {
        return Err(s.tr.text("invoice-cli-open-sioul", None));
    }
    if !keeper.mine {
        return Err(s.say("invoice-elsewhere", &[("computer", keeper.name)]));
    }
    // Kept here long enough for the others to know it: by the claim's own age, this process being new.
    let settled = keeper.others == 0 || own.is_some_and(|c| now - c.since >= lease::SETTLING);
    let memory = state.join("share").join("memory.json");
    let heard = share::heard(&memory, &here.id);
    if !settled || lease::closed_unread(&claims, &here.id, now, |computer, wrote| heard.complete(computer, wrote)).is_some() {
        return Err(s.tr.text("invoice-settling", None));
    }
    if keeper.others > 0 && now - keeper.others_heard > 3 * 60 {
        return Err(s.tr.text("invoice-others-silent", None));
    }
    Ok((life::Numbering { device: here.id, sharing: true, drafts_shared, claimed: lease::claimed_series(&claims) }, Some((folder, key))))
}

/// This computer's claim says the new number at once (the window's next
/// renewal would say it within a minute): renewed with the journal's last
/// numbers, everything else as the window wrote it.
fn claim_numbers(sharing: Option<(std::path::PathBuf, [u8; 32])>, device: &str) {
    use sioul_sync::lease;
    let Some((folder, key)) = sharing else { return };
    let now = jiff::Timestamp::now().as_second();
    let Some(own) = lease::claims(&folder, &key, "invoices").into_iter().find(|c| c.computer == device) else { return };
    let series = invoice::series::Journal::load(&invoice::folder()).lasts();
    let renewal = lease::Renewal { now, active: own.active, take: false, rule: lease::Rule::StaysPut, wrote: own.wrote, series: Some(series) };
    let _ = lease::renew_with(&folder, &key, "invoices", device, renewal);
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    let bytes: Option<Vec<u8>> = (0..text.len()).step_by(2).map(|i| text.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
    <[u8; 32]>::try_from(bytes?).ok()
}

fn issue(s: &Session, id: &str) -> Result<(), String> {
    let folder = invoice::folder();
    let draft = invoice::draft_in(&folder, id).ok_or_else(|| s.tr.text("invoice-draft-gone", None))?;
    let (numbering, sharing) = numbering(s)?;
    // Its client as the draft froze it (a credit note's, the credited invoice's); its project's budget expects it.
    let root = s.config.notes_root_path();
    let loaded = sioul_core::links::Loaded::read(&s.config);
    let project = loaded.projects.iter().find(|p| p.id == draft.project);
    let ledger = root.as_ref().map(|r| r.join(sioul_core::budget::LEDGER)).filter(|l| l.is_file());
    let budget = project.and_then(|p| p.budget.clone()).unwrap_or_default();
    let steps = life::Steps { time: None, ledger: ledger.as_deref().map(|l| (l, budget.as_str())) };
    let issued = life::issue(&folder, id, &s.config.invoice, today(), &numbering, &steps, &s.tr)?;
    claim_numbers(sharing, &numbering.device);
    println!("{}", s.say("invoice-cli-issued", &[("number", issued.number.clone()), ("amount", s.tr.money_in(issued.total(), &issued.currency))]));
    Ok(())
}

fn credit(s: &Session, number: &str, lines: &[usize]) -> Result<(), String> {
    let folder = invoice::folder();
    let found = invoice::find_in(&folder, number).ok_or_else(|| s.tr.text("invoice-gone", None))?;
    if found.kind == Kind::CreditNote {
        return Err(s.tr.text("invoice-cli-credit-of-credit", None));
    }
    let chosen: Vec<usize> = lines.iter().filter(|n| **n >= 1 && **n <= found.lines.len()).map(|n| n - 1).collect();
    let made = life::credit(&found, if lines.is_empty() { None } else { Some(&chosen) }, &s.config.invoice, today());
    let saved = invoice::save_draft(&folder, &made)?;
    println!("{}", s.say("invoice-cli-drafted", &[("id", saved.draft.clone())]));
    show(s, &saved.draft)
}

fn pay(s: &Session, number: &str, day: Option<&str>, amount: Option<&str>, means: &str, received: Option<&str>) -> Result<(), String> {
    let folder = invoice::folder();
    let booked = received.map(cents).transpose()?.map(i64::abs);
    if booked.is_some() {
        same_currency(s, number)?;
    }
    let day = match day {
        Some(text) => text.trim().parse::<Date>().map_err(|_| format!("{}: a day, as 2026-11-16.", one_line(text)))?,
        None => today(),
    };
    if !matches!(means, "transfer" | "card" | "cash" | "cheque" | "direct-debit" | "other") {
        return Err(format!("{}: transfer, card, cash, cheque, direct-debit or other.", one_line(means)));
    }
    let written = match amount {
        Some(text) => Some(life::add_payment(&folder, &device(), number, Payment { id: String::new(), date: day.to_string(), amount: cents(text)?, means: means.to_string(), movement: String::new() }, &s.tr)?),
        None => life::pay_rest(&folder, &device(), number, day, means, &s.tr)?,
    };
    if let (Some(payment), Some(booked)) = (&written, booked) {
        sioul_core::books::book_payment(&folder, &device(), &payment.id, booked, jiff::Timestamp::now().as_millisecond())?;
    }
    let all = invoice::all_in(&folder);
    let found = all.iter().find(|i| i.number == number).ok_or_else(|| s.tr.text("invoice-gone", None))?;
    if written.is_none() {
        println!("{}", s.tr.text("invoice-cli-nothing-left", None));
    }
    println!("{}", row(s, found, &all));
    Ok(())
}

/// Refused for an invoice in the books' own currency: what it received counts as it is.
fn same_currency(s: &Session, number: &str) -> Result<(), String> {
    let found = invoice::find_in(&invoice::folder(), number).ok_or_else(|| s.tr.text("invoice-gone", None))?;
    let books = sioul_core::books::Settings::of(&s.config.invoice, None).currency;
    if found.currency.trim().is_empty() || found.currency.eq_ignore_ascii_case(&books) {
        return Err(s.tr.text("books-cli-same-currency", None));
    }
    Ok(())
}

/// What a payment of `number`, in another currency, came to in the books'
/// (`books::book_payment`): the payment of `day`, else its last one with
/// nothing said yet, else its last one.
fn received(s: &Session, number: &str, amount: &str, day: Option<&str>) -> Result<(), String> {
    let folder = invoice::folder();
    same_currency(s, number)?;
    let found = invoice::find_in(&folder, number).ok_or_else(|| s.tr.text("invoice-gone", None))?;
    let day = day.map(|text| text.trim().parse::<Date>().map_err(|_| format!("{}: a day, as 2026-11-16.", one_line(text)))).transpose()?;
    let booked = sioul_core::invoice::lives::Lives::load(&folder).booked;
    let payments = sioul_core::books::received_of(&found);
    let chosen = match day {
        Some(day) => payments.iter().rev().find(|p| p.day == day),
        None => payments.iter().rev().find(|p| !booked.contains_key(&p.id)).or(payments.last()),
    };
    let payment = chosen.ok_or_else(|| s.tr.text("books-cli-no-payment", None))?;
    sioul_core::books::book_payment(&folder, &device(), &payment.id, cents(amount)?.abs(), jiff::Timestamp::now().as_millisecond())?;
    println!("{}", s.say("books-cli-received", &[("number", number.to_string()), ("day", payment.day.to_string()), ("amount", s.tr.money(Money(cents(amount)?.abs())))]));
    Ok(())
}
