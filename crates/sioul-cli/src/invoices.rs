// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul invoices`: the invoices you issued and your drafts, from the
//! terminal (docs/projects.md, "Invoices"): listed with where each stands,
//! shown, exported as a spreadsheet, a page to print or the PDF kept when
//! it was issued; a draft made from a project's unbilled time, issued by you
//! at the keyboard; a credit note; a payment received. Issuing follows the
//! window's rules: one device numbers at a time (`sioul_sync::lease`), and a
//! device whose sharing leaves invoices out never numbers.

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
    /// Every invoice as a spreadsheet (CSV), one as its page to print, or its PDF.
    Export {
        /// The page to print of this invoice (HTML), as a draft's preview shows it.
        #[arg(long, conflicts_with = "pdf")]
        html: Option<String>,
        /// This invoice's PDF as it was issued, copied where your invoices go; its path.
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
    /// Issues a draft: numbers it, keeps its PDF and its record, marks its time; you at the keyboard.
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
    /// draft, issued, sent, partly-paid, paid, credited.
    #[arg(long)]
    status: Option<String>,
}

pub(crate) fn run(s: &Session, command: Option<InvoicesCommand>) -> Result<(), String> {
    match command.unwrap_or(InvoicesCommand::List(ListArgs::default())) {
        InvoicesCommand::List(args) => list(s, &args),
        InvoicesCommand::Show { number } => show(s, &number),
        InvoicesCommand::Export { pdf: Some(number), .. } => {
            let found = invoice::find_in(&invoice::folder(), &number).ok_or_else(|| s.tr.text("invoice-gone", None))?;
            if found.file.is_empty() {
                // Before November 2026 the window printed it; it is where it went, or not there.
                let path = invoice::pdf_path(&found, &s.config.invoice.folder, &s.tr.text("invoice-folder-name", None));
                println!("{}", path.display());
                if !path.is_file() {
                    println!("{}", s.tr.text("invoice-cli-no-pdf", None));
                }
                return Ok(());
            }
            println!("{}", copied(s, &found).map_err(|why| not_given(s, why))?.display());
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
        InvoicesCommand::Pay { number, day, amount, means } => pay(s, &number, day.as_deref(), amount.as_deref(), &means),
    }
}

fn today() -> Date {
    sioul_core::zone::now().date()
}

/// The file issued of `found` (`invoice::kept`: made again from its record
/// when gone or changed and this Sioul makes the same bytes), copied where
/// your invoices go; its path, or why not.
fn copied(s: &Session, found: &Invoice) -> Result<std::path::PathBuf, invoice::kept::NotGiven> {
    let path = invoice::pdf_path(found, &s.config.invoice.folder, &s.tr.text("invoice-folder-name", None));
    invoice::kept::copy_to(&invoice::folder(), found, &path, &s.tr)?;
    Ok(path)
}

/// Why a file was not handed out, in words.
fn not_given(s: &Session, why: invoice::kept::NotGiven) -> String {
    match why {
        invoice::kept::NotGiven::Kept(why) => why,
        invoice::kept::NotGiven::Copy(path, e) => format!("{} ({e})", s.say("invoice-not-written", &[("path", path.display().to_string())])),
    }
}

/// An amount as typed ("120.50", "120,50") in cents.
fn cents(text: &str) -> Result<i64, String> {
    let units: f64 = text.trim().replace(',', ".").parse().map_err(|_| format!("{}: an amount, as 120.50.", one_line(text)))?;
    Ok(Money::from_units(units).cents())
}

/// A status's words.
fn status_words(s: &Session, status: Status) -> String {
    s.tr.text(&format!("invoice-status-{}", status.id()), None)
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
    format!("{name} · {}{kind} · {} · {} · {}{rest}{due}", if i.date.is_empty() { "—".into() } else { i.date.clone() }, one_line(i.client.first_line()), s.tr.money_in(i.total(), &i.currency), status_words(s, status))
}

fn list(s: &Session, args: &ListArgs) -> Result<(), String> {
    let folder = invoice::folder();
    let all = invoice::all_in(&folder);
    let wanted = args.status.as_deref().map(|w| Status::named(w).ok_or_else(|| format!("{}: draft, issued, sent, partly-paid, paid or credited.", one_line(w)))).transpose()?;
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
    for number in invoice::series::missing(&invoice::series::Journal::load(&folder), &all) {
        println!("{}", s.say("invoice-cli-missing", &[("number", number)]));
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
    println!("  {}", one_line(&found.client.name.replace('\n', ", ")));
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
        for check in life::checks(&found, &s.config.invoice, today()) {
            println!("  {} {}", if check.required { "✗" } else { "·" }, check.text);
        }
    }
    // Its issued file, when it is not as it was kept (export --pdf makes it again when it can).
    match invoice::kept::state(&folder, &found) {
        invoice::kept::State::Changed(_) => println!("  · {}", s.say("invoice-kept-changed", &[("number", found.number.clone())])),
        invoice::kept::State::Missing(_) => println!("  · {}", s.say("invoice-kept-missing", &[("number", found.number.clone())])),
        invoice::kept::State::Misnamed(file) => println!("  · {}", s.say("invoice-kept-misnamed", &[("number", found.number.clone()), ("file", file), ("record", folder.join(invoice::file_name(&found.number)).display().to_string())])),
        _ => {}
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
    let quote = |text: &str| if text.contains([sep, '"', '\n']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text.to_string() };
    let head = ["invoice-csv-number", "invoice-csv-kind", "invoice-csv-date", "invoice-csv-client", "invoice-csv-currency", "invoice-csv-without-tax", "invoice-csv-tax", "invoice-csv-due", "invoice-csv-paid", "invoice-csv-status"].map(|id| s.tr.text(id, None));
    let mut out = head.join(&sep.to_string()) + "\n";
    for i in all.iter().filter(|i| !i.unreadable && year.is_none_or(|y| i.date.get(..4).and_then(|d| d.parse::<i16>().ok()) == Some(y))) {
        let row = [i.number.clone(), s.tr.text(&format!("invoice-kind-{}", i.kind.id()), None), i.date.clone(), i.client.first_line().to_string(), i.currency.clone(), amount(i.totals.without_tax), amount(i.totals.tax), amount(i.totals.due), amount(i.paid()), status_words(s, i.status(&all))];
        out += &row.iter().map(|c| quote(c)).collect::<Vec<_>>().join(&sep.to_string());
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
    let made = life::from_time(&entries, &project, blank, &s.config.invoice, today()).ok_or_else(|| s.tr.text("invoice-nothing", None))?;
    let saved = invoice::save_draft(&invoice::folder(), &made)?;
    println!("{}", s.say("invoice-cli-drafted", &[("id", saved.draft.clone())]));
    show(s, &saved.draft)
}

/// What issuing needs of this computer's sharing, and whether it may number
/// now: sharing off, always; on, only when this computer keeps the invoices
/// (`sioul_sync::lease`), has kept them long enough for the others to know
/// it, and the others were heard from lately; with every device's claimed
/// numbers.
fn numbering(s: &Session) -> Result<life::Numbering, String> {
    use sioul_sync::lease;
    use sioul_sync::share;
    let state = sioul_core::config::state_dir();
    let here = share::Here::load(&state);
    let Some(folder) = here.folder_path() else { return Ok(life::Numbering::default()) };
    let drafts_shared = here.shares("drafts", &s.config);
    let key = sioul_sync::secret::named(share::KEY_NAME).and_then(|k| unhex(&k)).ok_or_else(|| s.tr.text("invoice-unchecked", None))?;
    let claims = lease::claims(&folder, &key, "invoices");
    let now = jiff::Timestamp::now().as_second();
    let keeper = lease::look(&folder, &key, "invoices", &here.id, now, lease::Rule::StaysPut);
    if !keeper.mine {
        return Err(s.say("invoice-elsewhere", &[("computer", keeper.name)]));
    }
    // Kept here long enough for the others to know it: by the claim's own age, this process being new.
    let settled = keeper.others == 0 || claims.iter().any(|c| c.computer == here.id && !c.closed && c.until > now && now - c.since >= lease::SETTLING);
    let memory = state.join("share").join("memory.json");
    let heard = share::heard(&memory, &here.id);
    if !settled || lease::closed_unread(&claims, &here.id, now, |computer, wrote| heard.complete(computer, wrote)).is_some() {
        return Err(s.tr.text("invoice-settling", None));
    }
    if keeper.others > 0 && now - keeper.others_heard > 3 * 60 {
        return Err(s.tr.text("invoice-others-silent", None));
    }
    Ok(life::Numbering { sharing: true, drafts_shared, claimed: lease::claimed_series(&claims) })
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    let bytes: Option<Vec<u8>> = (0..text.len()).step_by(2).map(|i| text.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
    <[u8; 32]>::try_from(bytes?).ok()
}

fn issue(s: &Session, id: &str) -> Result<(), String> {
    let folder = invoice::folder();
    let draft = invoice::draft_in(&folder, id).ok_or_else(|| s.tr.text("invoice-draft-gone", None))?;
    let numbering = numbering(s)?;
    // The client as its profile says now, frozen with the invoice; its project's budget expects it.
    let root = s.config.notes_root_path();
    let clients = root.as_deref().and_then(|r| sioul_core::clients::Clients::load(r).ok()).unwrap_or_default();
    let loaded = sioul_core::links::Loaded::read(&s.config);
    let client = (!draft.client_ref.is_empty()).then(|| sioul_core::clients::resolve(&draft.client_ref, &clients, &loaded.contacts).0);
    let project = loaded.projects.iter().find(|p| p.id == draft.project);
    let ledger = root.as_ref().map(|r| r.join(sioul_core::budget::LEDGER));
    let budget = project.and_then(|p| p.budget.clone()).unwrap_or_default();
    let steps = life::Steps { time: None, ledger: ledger.as_deref().filter(|_| !budget.is_empty()).map(|l| (l, budget.as_str())) };
    let issued = life::issue(&folder, id, &s.config.invoice, today(), &numbering, client, &steps, &s.tr)?;
    println!("{}", s.say("invoice-cli-issued", &[("number", issued.number.clone()), ("amount", s.tr.money_in(issued.total(), &issued.currency))]));
    // What the issue found to say (a file set aside), and the checks' notes (no Factur-X data, an IBAN whose key fails…).
    for note in issued.said.iter().cloned().chain(life::checks(&issued, &s.config.invoice, today()).into_iter().filter(|c| !c.required).map(|c| c.text)) {
        println!("  · {note}");
    }
    // Issued all the same when its copy cannot be written: said, and Print again later.
    match copied(s, &issued) {
        Ok(path) => println!("{}", s.say("invoice-cli-pdf", &[("path", path.display().to_string())])),
        Err(invoice::kept::NotGiven::Copy(path, e)) => eprintln!("{} ({e})", s.say("invoice-issued-not-copied", &[("number", issued.number.clone()), ("path", path.display().to_string())])),
        Err(why) => eprintln!("{}", not_given(s, why)),
    }
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

fn pay(s: &Session, number: &str, day: Option<&str>, amount: Option<&str>, means: &str) -> Result<(), String> {
    let folder = invoice::folder();
    let day = match day {
        Some(text) => text.trim().parse::<Date>().map_err(|_| format!("{}: a day, as 2026-11-16.", one_line(text)))?,
        None => today(),
    };
    if !matches!(means, "transfer" | "card" | "cash" | "cheque" | "direct-debit" | "other") {
        return Err(format!("{}: transfer, card, cash, cheque, direct-debit or other.", one_line(means)));
    }
    let written = match amount {
        Some(text) => Some(life::add_payment(&folder, number, Payment { date: day.to_string(), amount: cents(text)?, means: means.to_string(), movement: String::new() }).map(|_| ())?),
        None => life::pay_rest(&folder, number, day, means)?.map(|_| ()),
    };
    let all = invoice::all_in(&folder);
    let found = all.iter().find(|i| i.number == number).ok_or_else(|| s.tr.text("invoice-gone", None))?;
    if written.is_none() {
        println!("{}", s.tr.text("invoice-cli-nothing-left", None));
    }
    println!("{}", row(s, found, &all));
    Ok(())
}
