// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul invoices`: the invoices you issued and your drafts, from the
//! terminal (docs/projects.md, "Invoices"): listed with where each stands,
//! shown, exported as a spreadsheet, a page to print or the PDF kept when
//! it was issued; a draft made from a project's unbilled time, issued by you
//! at the keyboard; a credit note; a payment received. Issuing follows the
//! window's rules: one device numbers at a time (`sioul_sync::lease`), and a
//! device whose sharing leaves invoices out never numbers. With sharing on,
//! the command line numbers only while this computer's window holds the
//! invoices' claim, which it renews with the new number at once.

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
    /// Discards a draft: nothing was numbered.
    Discard {
        /// The draft's id (`sioul invoices list` gives it).
        draft: String,
    },
    /// A payment received for an invoice, or paid back on a credit note.
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
    /// draft, issued, sent, partly-paid, paid, credited, credit-note, to-pay-back.
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
        InvoicesCommand::Discard { draft } => {
            // A newer Sioul's draft is never taken out here: the removal would travel, and nothing here could write it back (the second review of phase 3, R13).
            if invoice::draft_in(&invoice::folder(), &draft).is_some_and(|d| d.newer) {
                return Err(s.tr.text("invoice-draft-newer", None));
            }
            if invoice::remove_draft(&invoice::folder(), &draft)? {
                println!("{}", s.say("invoice-cli-discarded", &[("id", draft)]));
                Ok(())
            } else {
                Err(s.tr.text("invoice-draft-gone", None))
            }
        }
        InvoicesCommand::Pay { number, day, amount, means, received } => pay(s, &number, day.as_deref(), amount.as_deref(), &means, received.as_deref()),
        InvoicesCommand::Received { number, amount, day } => received(s, &number, &amount, day.as_deref()),
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

/// Where an invoice stands, in words; a credit note says what it credits,
/// and what it has to pay back.
fn status_words(s: &Session, i: &Invoice, status: Status, all: &[Invoice]) -> String {
    match (&i.corrects, status) {
        (Some(cited), Status::CreditNote) => s.say("invoice-credits", &[("number", cited.number.clone())]),
        (Some(cited), Status::ToPayBack) => format!("{} · {}", s.say("invoice-credits", &[("number", cited.number.clone())]), s.say("invoice-to-pay-back", &[("amount", s.tr.money_in(Money(i.to_pay_back(all)), &i.currency))])),
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
    let name = if i.copy_of.is_empty() { name } else { format!("{name} ({})", s.say("invoice-cli-draft-copy", &[("id", i.copy_of.clone())])) };
    let kind = if i.kind == Kind::Invoice { String::new() } else { format!(" · {}", s.tr.text(&format!("invoice-kind-{}", i.kind.id()), None)) };
    let left = i.left(all);
    let due = if matches!(status, Status::Issued | Status::Sent | Status::PartlyPaid) && !i.due.is_empty() { format!(" · {}", s.say("invoice-cli-due", &[("date", i.due.clone())])) } else { String::new() };
    let rest = if status == Status::PartlyPaid { format!(" · {}", s.say("invoice-cli-left", &[("amount", s.tr.money_in(Money(left), &i.currency))])) } else { String::new() };
    format!("{name} · {}{kind} · {} · {} · {}{rest}{due}", if i.date.is_empty() { "—".into() } else { i.date.clone() }, one_line(i.client.first_line()), s.tr.money_in(i.total(), &i.currency), status_words(s, i, status, all))
}

fn list(s: &Session, args: &ListArgs) -> Result<(), String> {
    let folder = invoice::folder();
    let all = invoice::all_in(&folder);
    let wanted = args.status.as_deref().map(|w| Status::named(w).ok_or_else(|| format!("{}: draft, issued, sent, partly-paid, paid, credited, credit-note or to-pay-back.", one_line(w)))).transpose()?;
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
    // A number given twice (G9), and files a newer Sioul wrote (G13): said, never hidden.
    for twice in invoice::twice_in(&folder) {
        let file = twice.copy.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        println!("{}", s.say("invoice-cli-twice", &[("number", twice.number), ("first", one_line(&twice.clients.0)), ("second", one_line(&twice.clients.1)), ("file", file)]));
    }
    let (lives, journals) = (invoice::lives::Lives::load(&folder).newer, invoice::series::Journal::newer(&folder));
    let newer = all.iter().filter(|i| i.newer).map(|i| invoice::file_name(&i.number)).chain(lives.iter().chain(journals.iter()).map(|p| p.display().to_string()));
    for file in newer {
        println!("{}", s.say("invoice-cli-newer", &[("file", file)]));
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
    // The client, and whether it is billed as a business or a person; a
    // draft's client as its profile says it now (G12), a business when none says (G8).
    let mut found = found;
    let root = s.config.records_root();
    let clients = root.as_ref().and_then(|r| sioul_core::clients::Clients::load(r).ok()).unwrap_or_default();
    let loaded = sioul_core::links::Loaded::read(&s.config);
    let profiled = sioul_core::clients::resolve(&found.client_ref, &clients, &loaded.contacts).1.is_some();
    if found.number.is_empty() {
        life::refresh_client(&mut found, &clients, &loaded.contacts);
        life::refresh(&mut found, &s.config.invoice, today());
    }
    let kind = if found.client.kind == "business" && !profiled && found.kind != Kind::CreditNote { s.tr.text("invoice-client-kind-unsaid", None) } else { s.tr.text(&format!("invoice-client-kind-{}", found.client.kind), None) };
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
    // Text cells never run as formulas (a client named "=HYPERLINK(…)"), as the time's spreadsheet.
    let quote = |text: &str| sioul_core::timereport::csv_cell(text, sep);
    let head = ["invoice-csv-number", "invoice-csv-kind", "invoice-csv-date", "invoice-csv-client", "invoice-csv-currency", "invoice-csv-without-tax", "invoice-csv-tax", "invoice-csv-due", "invoice-csv-paid", "invoice-csv-status"].map(|id| s.tr.text(id, None));
    let mut out = head.join(&sep.to_string()) + "\n";
    for i in all.iter().filter(|i| !i.unreadable && year.is_none_or(|y| i.date.get(..4).and_then(|d| d.parse::<i16>().ok()) == Some(y))) {
        let text = [i.number.clone(), s.tr.text(&format!("invoice-kind-{}", i.kind.id()), None), i.date.clone(), i.client.first_line().to_string(), i.currency.clone()].map(|c| quote(&c));
        let numbers = [amount(i.totals.without_tax), amount(i.totals.tax), amount(i.totals.due), amount(i.paid())];
        let row: Vec<String> = text.into_iter().chain(numbers).chain(std::iter::once(quote(&status_words(s, i, i.status(&all), &all)))).collect();
        out += &row.join(&sep.to_string());
        out.push('\n');
    }
    out
}

fn draft(s: &Session, id: &str) -> Result<(), String> {
    let root = s.config.records_root().ok_or_else(|| s.tr.text("error-no-store", None))?;
    let store = sioul_core::projects::ProjectStore::load(&root)?;
    let project = store.get(id).ok_or_else(|| s.tr.text("project-gone", None))?.clone();
    let loaded = sioul_core::links::Loaded::read(&s.config);
    let entries = sioul_core::timereport::entries(&sioul_core::timelog::sessions(), &loaded.tasks, &loaded.projects, &sioul_core::zone::current());
    let clients = sioul_core::clients::Clients::load(&root).unwrap_or_default();
    let blank = life::for_client(&s.config.invoice, project.client.as_deref().unwrap_or(""), &clients, &loaded.contacts, s.tr.language(), today());
    let records = invoice::all_in(&invoice::folder());
    // Never time a draft holds already, nor time taken out of one of the project's drafts (the second review of phase 3, R15).
    let entries = life::untaken(entries, &invoice::drafts_in(&invoice::folder()), &project.id);
    let made = life::from_time(&entries, &project, blank, &records, &s.config.invoice, today()).ok_or_else(|| s.tr.text("invoice-nothing", None))?;
    let saved = invoice::save_draft(&invoice::folder(), &made)?;
    println!("{}", s.say("invoice-cli-drafted", &[("id", saved.draft.clone())]));
    show(s, &saved.draft)
}

/// What issuing needs of this computer's sharing, and whether it may number
/// now: sharing off, always; on, only as the window itself would, its claim
/// on invoices read as the window reads it (`lease::may_number`): alive,
/// renewed within two minutes (a window runs), never closed, kept long
/// enough for the others to know it, no other computer having closed with
/// records not read here, the others heard from lately. The command line
/// never claims by itself: a claim it took would lapse unseen as soon as it
/// ends, and another computer could number meanwhile.
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
    let heard = share::heard(&state.join("share").join("memory.json"), &here.id);
    match lease::may_number(&claims, &here.id, now, |computer, wrote| heard.complete(computer, wrote)) {
        lease::Numbering::Yes => Ok((life::Numbering { device: here.id, sharing: true, drafts_shared, claimed: lease::claimed_series(&claims) }, Some((folder, key)))),
        lease::Numbering::NotRunning => Err(s.tr.text("invoice-cli-open-sioul", None)),
        lease::Numbering::Elsewhere(name) => Err(s.say("invoice-elsewhere", &[("computer", name)])),
        lease::Numbering::Settling => Err(s.tr.text("invoice-settling", None)),
        lease::Numbering::OthersSilent => Err(s.tr.text("invoice-others-silent", None)),
    }
}

/// This computer's claim says the new number at once (the window's next
/// renewal would say it within a minute): renewed with the journal's last
/// numbers, everything else as the window wrote it; a claim the window
/// closed meanwhile stays closed, its numbers raised (`lease::close_with`).
fn claim_numbers(sharing: Option<(std::path::PathBuf, [u8; 32])>, device: &str) {
    use sioul_sync::lease;
    let Some((folder, key)) = sharing else { return };
    let now = jiff::Timestamp::now().as_second();
    let Some(own) = lease::claims(&folder, &key, "invoices").into_iter().find(|c| c.computer == device) else { return };
    let series = invoice::series::Journal::load(&invoice::folder()).lasts();
    if own.closed || own.until <= now {
        let _ = lease::close_with(&folder, &key, "invoices", device, now, own.wrote, Some(series));
        return;
    }
    let renewal = lease::Renewal { now, active: own.active, take: false, rule: lease::Rule::StaysPut, wrote: own.wrote, series: Some(series) };
    let _ = lease::renew_with(&folder, &key, "invoices", device, renewal);
}

pub(crate) fn unhex(text: &str) -> Option<[u8; 32]> {
    let bytes: Option<Vec<u8>> = (0..text.len()).step_by(2).map(|i| text.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
    <[u8; 32]>::try_from(bytes?).ok()
}

fn issue(s: &Session, id: &str) -> Result<(), String> {
    let folder = invoice::folder();
    let draft = invoice::draft_in(&folder, id).ok_or_else(|| s.tr.text("invoice-draft-gone", None))?;
    let (numbering, sharing) = numbering(s)?;
    // Its client as its profile says it now (a credit note's, the credited invoice's as it was); its project's budget expects it.
    let root = s.config.records_root();
    let loaded = sioul_core::links::Loaded::read(&s.config);
    let project = loaded.projects.iter().find(|p| p.id == draft.project);
    let ledger = root.as_ref().map(|r| r.join(sioul_core::budget::LEDGER)).filter(|l| l.is_file());
    let budget = project.and_then(|p| p.budget.clone()).unwrap_or_default();
    let clients = root.as_ref().and_then(|r| sioul_core::clients::Clients::load(r).ok()).unwrap_or_default();
    let steps = life::Steps { time: None, ledger: ledger.as_deref().map(|l| (l, budget.as_str())), clients: Some((&clients, &loaded.contacts)) };
    // Read and issued in one step: the revision is the one just read (R6).
    let revision = invoice::draft_revision(&folder, id);
    let issued = life::issue_as_read(&folder, id, &revision, &s.config.invoice, today(), &numbering, &steps, &s.tr).map_err(|e| match e {
        life::NotIssued::Moved(why) | life::NotIssued::Refused(why) => why,
    })?;
    claim_numbers(sharing, &numbering.device);
    println!("{}", s.say("invoice-cli-issued", &[("number", issued.number.clone()), ("amount", s.tr.money_in(issued.total(), &issued.currency))]));
    // What the issue found to say (a file set aside), and the checks' notes (no Factur-X data, an IBAN whose key fails…).
    let records = invoice::all_in(&folder);
    for note in issued.said.iter().cloned().chain(life::checks(&issued, &s.config.invoice, today(), &records).into_iter().filter(|c| !c.required).map(|c| c.text)) {
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
    let loaded = sioul_core::links::Loaded::read(&s.config);
    let entries = sioul_core::timereport::entries(&sioul_core::timelog::sessions(), &loaded.tasks, &loaded.projects, &sioul_core::zone::current());
    let made = life::credit(&found, if lines.is_empty() { None } else { Some(&chosen) }, &entries, &s.config.invoice, today());
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
        Some(text) => Some(life::add_payment(&folder, &device(), number, Payment { id: String::new(), date: day.to_string(), amount: cents(text)?, means: means.to_string(), ..Payment::default() }, &s.tr)?),
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
