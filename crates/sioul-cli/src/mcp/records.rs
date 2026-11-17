// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The tools that read the person's records, all read-only: papers,
//! contracts and paper letters (what each is, what ends and when, the notice
//! a contract needs, a letter's sender, kind and date asked); time and
//! invoices (time noted per project, what is billable and not billed yet,
//! invoices issued and whether they are paid); reminders (dates asked,
//! waits, payments, papers' and contracts' dates); and the moment now (what
//! now is for, do-not-disturb, what reaches the person now), so that an
//! agent respects their hours.
//!
//! Each keeps to what the person opened to agents (`access`): a thing of a
//! closed project, or in no project while those are closed, is left out and
//! counted. Health stays out; identifiers that are no help to an agent (a
//! contract's or a letter's reference, a paper's file) are not given; account
//! and social security numbers are masked on the way out, as everywhere.

use super::access;
use super::tools::{Answer, Args};
use crate::{Session, one_line};
use jiff::civil::Date;
use jiff::{Timestamp, ToSpan, Zoned};
use serde_json::{Value, json};
use sioul_core::consent::Consent;
use sioul_core::links::{self, Loaded};
use sioul_core::{contracts, invoice, letters, papers, reminders, timelog, timereport};

/// What the person opened to agents, from everything read.
fn consent(s: &Session) -> (Consent, Loaded) {
    let loaded = Loaded::read(&s.config);
    (access::of(s, &loaded), loaded)
}

/// Unix seconds as a moment in the person's time zone, for data.
fn instant(seconds: i64) -> String {
    Timestamp::from_second(seconds).map(|t| t.to_zoned(sioul_core::zone::current()).strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()).unwrap_or_default()
}

/// The notes folder, where papers, contracts and letters are kept.
fn store(s: &Session) -> Result<std::path::PathBuf, String> {
    s.config.notes_root_path().ok_or_else(|| s.tr.text("error-no-store", None))
}

/// Lines of an answer: each one line, whatever was written in it.
fn rows(lines: &[String]) -> String {
    lines
        .iter()
        .map(|line| {
            let words = line.trim_start_matches(' ');
            format!("{}{}", &line[..line.len() - words.len()], one_line(words))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn day(date: Option<Date>) -> String {
    date.map(|d| d.to_string()).unwrap_or_default()
}

// Papers, contracts, letters.

pub fn papers(s: &Session, _args: &Args) -> Result<Answer, String> {
    let wallet = papers::Wallet::load(&store(s)?)?;
    let (consent, loaded) = consent(s);
    let today = sioul_core::zone::now().date();
    let before = wallet.papers.len();
    // A paper is in no project unless tied to one; a closed person's, and others' health papers, are kept (`access::of`).
    let shown: Vec<&papers::Paper> = wallet.papers.iter().filter(|p| consent.allows(&links::paper_uri(&p.id))).collect();
    // Whom each is for, by address: the persons opened to agents, and you (`sioul:person/me`).
    let own: Vec<String> = s.config.every_account().filter_map(|a| a.name.clone()).collect();
    let persons = sioul_core::persons::Persons::as_read(&sioul_core::persons::Persons::default_path(), &s.config.agenda.calendars, &wallet.papers, &own);
    let closed = before - shown.len();
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for paper in shown {
        let kind = s.tr.text(&format!("paper-kind-{}", paper.kind.id()), None);
        let (standing, said) = match paper.standing(today) {
            papers::Standing::Valid(until) => ("valid", format!("holds until {until}")),
            papers::Standing::Renew(until) => ("renew", format!("holds until {until}; renewing starts now")),
            papers::Standing::Ended(until) => ("ended", format!("ended on {until}")),
            papers::Standing::Fresh(issued) => ("fresh", format!("issued on {issued}, recent enough to send")),
            papers::Standing::Old(issued) => ("old", format!("issued on {issued}, older than what is usually asked")),
            papers::Standing::Undated => ("undated", String::new()),
        };
        let holder = if paper.holder.is_empty() { String::new() } else { format!(" · {}", paper.holder) };
        let tail = if said.is_empty() { String::new() } else { format!(" · {said}") };
        lines.push(format!("{} ({kind}){holder}{tail}  <{}>", paper.title, links::paper_uri(&paper.id)));
        out.push(json!({
            "uri": links::paper_uri(&paper.id), "kind": paper.kind.id(), "kind_said": kind, "title": paper.title, "holder": paper.holder,
            "issued": day(paper.issued), "until": day(paper.until), "renew_from": day(paper.renew_from()), "standing": standing,
            "renewal": paper.renewal,
            "for": persons.holders_of(paper, &loaded.local, &own).persons.iter().map(|id| links::person_uri(id)).collect::<Vec<_>>(),
        }));
    }
    if out.is_empty() && closed == 0 {
        lines.push("No paper is kept in the wallet yet.".into());
    }
    lines.extend(access::left_out(closed));
    Ok(Answer { text: rows(&lines), data: json!({ "papers": out, "kept_from_agents": closed }) })
}

/// The persons opened to agents (docs/persons.md), each with what links to
/// them by address, as the consent lets each thing through; never a date of
/// birth, whether they live with the person, nor their cards. Those not
/// opened are counted, never named.
pub fn persons(s: &Session, _args: &Args) -> Result<Answer, String> {
    use sioul_core::persons::{self, Persons, Relation};
    let (consent, loaded) = consent(s);
    let own: Vec<String> = s.config.every_account().filter_map(|a| a.name.clone()).collect();
    let root = s.config.notes_root_path();
    let wallet = root.as_deref().and_then(|r| papers::Wallet::load(r).ok()).map(|w| w.papers).unwrap_or_default();
    let store = Persons::as_read(&Persons::default_path(), &s.config.agenda.calendars, &wallet, &own);
    let letters = root.as_deref().and_then(|r| letters::Letters::load(r).ok()).map(|l| l.list).unwrap_or_default();
    let ledger = root.as_deref().and_then(|r| sioul_core::budget::Ledger::load_file(&r.join(sioul_core::budget::LEDGER)).ok());
    let agendas: Vec<sioul_core::agendas::Agenda> = sioul_core::agendas::all(&s.config).into_iter().map(|a| a.with_persons(&store)).collect();
    let opened: Vec<&persons::Person> = store.persons.iter().filter(|p| p.agents).collect();
    let closed = store.persons.len() - opened.len();
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for person in opened {
        let relation = match person.relation() {
            Relation::Unsaid => "in-household",
            other => other.id(),
        };
        let papers: Vec<String> = wallet.iter().filter(|p| store.holders_of(p, &loaded.local, &own).has(&person.id)).map(|p| links::paper_uri(&p.id)).filter(|u| consent.allows(u)).collect();
        let letters_for: Vec<String> = letters.iter().filter(|l| store.letter_for(&l.id, &l.reading.addressee, &loaded.local, &own).iter().any(|p| *p == person.id)).map(|l| links::letter_uri(&l.id)).filter(|u| consent.allows(u)).collect();
        let places: Vec<String> = agendas.iter().filter(|a| a.person_id == person.id || a.parent_person == person.id).map(|a| a.id.clone()).collect();
        let budgets: Vec<String> = ledger.iter().flat_map(|l| l.budgets.iter()).filter(|b| sioul_core::budget::person_of(b).as_deref() == Some(person.id.as_str()) && consent.allows_projects(consent.budget_projects(&b.id).iter().map(String::as_str))).map(|b| b.id.clone()).collect();
        let sites: Vec<String> = persons::sites_for(&loaded.local, &person.id).iter().map(|id| links::site_uri(id)).collect();
        lines.push(format!("{} ({relation})  <{}>", person.name, persons::uri(&person.id)));
        out.push(json!({ "uri": persons::uri(&person.id), "name": person.name, "relation": relation, "papers": papers, "letters": letters_for, "agendas": places, "budgets": budgets, "sites": sites }));
    }
    if out.is_empty() && closed == 0 {
        lines.push("No one of the person's household is kept in Sioul yet.".into());
    }
    if closed > 0 {
        lines.push(format!("({closed} {} of the household the person has not opened to agents: left out on purpose. Do not guess who; ask the person if it matters.)", if closed == 1 { "person" } else { "persons" }));
    }
    Ok(Answer { text: rows(&lines), data: json!({ "persons": out, "kept_from_agents": closed }) })
}

pub fn contracts(s: &Session, _args: &Args) -> Result<Answer, String> {
    let list = contracts::Contracts::load(&store(s)?)?;
    let (consent, _) = consent(s);
    let today = sioul_core::zone::now().date();
    // A contract is in no project: as things outside projects are.
    let shown: Vec<&contracts::Contract> = if consent.outside { list.list.iter().collect() } else { Vec::new() };
    let closed = list.list.len() - shown.len();
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for contract in shown {
        let kind = s.tr.text(&format!("contract-kind-{}", contract.kind.id()), None);
        let renews = contract.next_renewal(today);
        let cancel_by = contract.cancel_by(today);
        let mut said = Vec::new();
        if !contract.party.is_empty() {
            said.push(contract.party.clone());
        }
        if let Some(ended) = contract.ended {
            said.push(format!("ended on {ended}"));
        } else {
            if let Some(renews) = renews {
                said.push(format!("renews on {renews}"));
            }
            if contract.notice_days > 0 {
                said.push(format!("{} days' notice", contract.notice_days));
            }
            if let Some(by) = cancel_by {
                said.push(format!("to stop it, notice sent by {by}"));
            }
        }
        lines.push(format!("{} ({kind}) · {}", contract.title, said.join(" · ")));
        out.push(json!({
            "kind": contract.kind.id(), "kind_said": kind, "title": contract.title, "party": contract.party, "started": day(contract.started),
            "renews": day(renews), "every": contract.every, "notice_days": contract.notice_days, "cancel_by": day(cancel_by),
            "ended": day(contract.ended), "covers": contract.covers, "open": contract.is_open(),
        }));
    }
    if out.is_empty() && closed == 0 {
        lines.push("No contract is kept yet.".into());
    }
    lines.extend(access::left_out(closed));
    Ok(Answer { text: rows(&lines), data: json!({ "contracts": out, "kept_from_agents": closed }) })
}

pub fn letters(s: &Session, args: &Args) -> Result<Answer, String> {
    let limit = args.number("limit", 30, 1, 500)? as usize;
    let all = letters::Letters::load(&store(s)?)?;
    let (consent, _) = consent(s);
    // A letter is in its project; in none, as things outside projects are.
    // A letter for someone of the household kept from agents is kept too (`access::of`, docs/persons.md).
    let mut shown: Vec<&letters::Letter> = all.list.iter().filter(|l| consent.allows_projects([l.project.as_str()]) && !consent.keeps_for_persons(&links::letter_uri(&l.id))).collect();
    let closed = all.list.len() - shown.len();
    shown.sort_by_key(|l| std::cmp::Reverse(l.received));
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for letter in shown.iter().take(limit) {
        let r = &letter.reading;
        let kind = s.tr.text(&format!("letter-kind-{}", r.kind.id()), None);
        let mut said = vec![format!("{} · {kind}", if r.sender.is_empty() { "(sender not read)" } else { r.sender.as_str() })];
        if let Some(received) = letter.received {
            said.push(format!("received {received}"));
        }
        if let Some(amount) = r.amount {
            said.push(s.tr.money(amount));
        }
        if let Some(deadline) = r.deadline {
            said.push(format!("asked by {deadline}"));
        }
        if let Some((date, time)) = r.appointment {
            said.push(match time {
                Some((h, m)) => format!("appointment {date} {h:02}:{m:02}"),
                None => format!("appointment {date}"),
            });
        }
        lines.push(said.join(" · "));
        if !r.why.is_empty() {
            lines.push(format!("    {}", r.why));
        }
        out.push(json!({
            "id": letter.id, "sender": r.sender, "kind": r.kind.id(), "kind_said": kind, "received": day(letter.received), "dated": day(r.dated),
            "deadline": day(r.deadline), "why": r.why, "amount": r.amount.map(|a| s.tr.money(a)), "registered": r.registered,
            "project": letter.project, "status": letter.status, "task": if letter.task.is_empty() { String::new() } else { links::task_uri(&letter.task) },
        }));
    }
    if out.is_empty() && closed == 0 {
        lines.push("No paper letter is kept yet.".into());
    } else if shown.len() > limit {
        lines.push(format!("… {} more", shown.len() - limit));
    }
    lines.extend(access::left_out(closed));
    Ok(Answer { text: rows(&lines), data: json!({ "letters": out, "kept_from_agents": closed }) })
}

// Time and invoices.

pub fn time(s: &Session, args: &Args) -> Result<Answer, String> {
    let today = sioul_core::zone::now().date();
    let to = args.date("to")?.unwrap_or(today);
    let from = args.date("from")?.unwrap_or_else(|| to.checked_sub(30.days()).unwrap_or(to));
    if to < from {
        return Err("“to” comes before “from”.".into());
    }
    let project = args.text("project")?;
    let (consent, loaded) = consent(s);
    if let Some(id) = &project {
        if !loaded.projects.iter().any(|c| &c.id == id) {
            return Err(format!("No project “{}”: list_projects gives their ids.", one_line(id)));
        }
        if !consent.is_open(id) {
            return Err(access::closed_project(id));
        }
    }
    let entries = timereport::entries(&timelog::sessions(), &loaded.tasks, &loaded.projects, &sioul_core::zone::current());
    let in_range: Vec<&timereport::Entry> = entries.iter().filter(|e| e.day >= from && e.day <= to && project.as_ref().is_none_or(|p| &e.project == p)).collect();
    // Time of a project kept from agents, or of a task kept from them; in none, as things outside projects are.
    // The errands' record not read here: a session of a task not known here may be a gone
    // errand's, its note naming it: left out, the doubt said (the check of ef8c0ca, item 2).
    let unknown = |uid: &str| consent.health_unread && !loaded.tasks.iter().any(|t| t.uid == uid);
    let allowed = |e: &timereport::Entry| consent.allows_projects([e.project.as_str()]) && (e.task.is_empty() || (consent.allows(&links::task_uri(&e.task)) && !unknown(&e.task)));
    let shown: Vec<&timereport::Entry> = in_range.iter().copied().filter(|e| allowed(e)).collect();
    let closed = in_range.len() - shown.len();
    let title_of = |id: &str| loaded.projects.iter().find(|c| c.id == id).map_or_else(|| "(no project)".to_string(), |c| c.title.clone());
    // By project: all the time, the billable part, what is left to bill.
    let mut by_project: std::collections::BTreeMap<&str, (u32, u32, u32)> = Default::default();
    for e in &shown {
        let sums = by_project.entry(e.project.as_str()).or_default();
        sums.0 += e.minutes;
        if e.billable {
            sums.1 += e.minutes;
            if e.invoice.is_empty() {
                sums.2 += e.minutes;
            }
        }
    }
    let mut lines = vec![format!("Time noted from {from} to {to}:")];
    let mut projects = Vec::new();
    for (id, (all, billable, unbilled)) in &by_project {
        let mut said = vec![timereport::duration(*all)];
        if *billable > 0 {
            said.push(format!("{} billable", timereport::duration(*billable)));
        }
        if *unbilled > 0 {
            said.push(format!("{} not billed yet", timereport::duration(*unbilled)));
        }
        let address = if id.is_empty() { String::new() } else { format!("  <{}>", links::project_uri(id)) };
        lines.push(format!("  {}: {}{address}", title_of(id), said.join(" · ")));
        projects.push(json!({ "project": id, "title": title_of(id), "minutes": all, "billable_minutes": billable, "unbilled_minutes": unbilled }));
    }
    let sessions: Vec<Value> = shown
        .iter()
        .map(|e| {
            json!({
                "start": instant(e.start), "day": e.day.to_string(), "minutes": e.minutes, "project": e.project, "title": e.title,
                "task": if e.task.is_empty() { String::new() } else { links::task_uri(&e.task) }, "note": e.note, "billable": e.billable, "invoice": e.invoice,
            })
        })
        .collect();
    if shown.is_empty() {
        lines.push("  Nothing noted then.".into());
    } else {
        lines.push(String::new());
        for e in shown.iter().rev().take(60) {
            let billed = if !e.invoice.is_empty() { format!(" · billed ({})", e.invoice) } else if e.billable { " · billable".into() } else { String::new() };
            lines.push(format!("  {} · {} · {}{billed}", e.day, timereport::duration(e.minutes), e.title));
        }
        if shown.len() > 60 {
            lines.push(format!("  … {} more, in the data", shown.len() - 60));
        }
    }
    lines.extend(access::left_out(closed));
    lines.extend(access::health_doubt(&consent));
    Ok(Answer { text: rows(&lines), data: json!({ "from": from.to_string(), "to": to.to_string(), "projects": projects, "sessions": sessions, "kept_from_agents": closed }) })
}

pub fn invoices(s: &Session, _args: &Args) -> Result<Answer, String> {
    let (consent, _) = consent(s);
    let folder = invoice::folder();
    let all = invoice::all_in(&folder);
    let drafts = invoice::drafts_in(&folder);
    // Drafts first, then invoices, newest first; each project's as the person opened it.
    let every: Vec<&invoice::Invoice> = drafts.iter().chain(all.iter().rev()).filter(|i| !i.unreadable).collect();
    let shown: Vec<&&invoice::Invoice> = every.iter().filter(|i| consent.allows_projects([i.project.as_str()])).collect();
    let closed = every.len() - shown.len();
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for i in shown {
        let status = i.status(&all);
        let client = i.client.first_line();
        let amount = s.tr.money_in(i.total(), &i.currency);
        let name = if i.number.is_empty() { format!("draft {}", i.draft) } else { i.number.clone() };
        let kind = match i.kind {
            invoice::Kind::Invoice => "",
            invoice::Kind::CreditNote => " · credit note",
            invoice::Kind::Deposit => " · deposit invoice",
        };
        let left = i.left(&all);
        let due = if matches!(status, invoice::Status::Issued | invoice::Status::Sent | invoice::Status::PartlyPaid) && !i.due.is_empty() { format!(" (due {})", i.due) } else { String::new() };
        let words = match status {
            invoice::Status::Draft => "draft, not numbered".to_string(),
            invoice::Status::Issued | invoice::Status::Sent => "not paid yet".to_string(),
            invoice::Status::PartlyPaid => format!("partly paid, {} left", s.tr.money_in(sioul_core::money::Money(left), &i.currency)),
            invoice::Status::Paid => "paid".to_string(),
            invoice::Status::Credited => "credited".to_string(),
            invoice::Status::CreditNote => format!("credits {}", i.corrects.as_ref().map(|c| c.number.as_str()).unwrap_or("an invoice")),
        };
        lines.push(format!("{name}{kind} · {} · {client} · {amount} · {words}{due}", if i.date.is_empty() { "—" } else { i.date.as_str() }));
        out.push(json!({
            "number": i.number,
            "draft": i.draft,
            "kind": i.kind.id(),
            "status": status.id(),
            "date": i.date,
            "due": i.due,
            "project": i.project,
            "project_title": i.project_title,
            "client": client,
            "client_kind": i.client.kind,
            "amount": amount,
            "left": s.tr.money_in(sioul_core::money::Money(left), &i.currency),
            "currency": i.currency,
            "paid": status.settled(),
            "corrects": i.corrects.as_ref().map(|c| c.number.clone()).unwrap_or_default(),
        }));
    }
    if out.is_empty() && closed == 0 {
        lines.push("No invoice issued yet.".into());
    }
    lines.extend(access::left_out(closed));
    Ok(Answer { text: rows(&lines), data: json!({ "invoices": out, "kept_from_agents": closed }) })
}

// The books.

/// The books (`sioul_core::books`), read-only, of a year (this one by
/// default): the receipts book's rows and totals, the declarations of
/// turnover, the VAT summary, the DES, the thresholds watched. Each kept to
/// what the person opened to agents: an invoice's payments only when its
/// project is open; a receipt no invoice describes only when its budget is
/// open (as the `budgets` tool's gate says); the rest left out and counted,
/// and every total then counts only what is shown. The thresholds, which
/// add up everything, are given only when nothing is kept from agents.
pub fn books(s: &Session, args: &Args) -> Result<Answer, String> {
    use sioul_core::books::{self, declarations, des, thresholds, vat};
    let today = sioul_core::zone::now().date();
    let year = i16::try_from(args.number("year", i64::from(today.year()), 2000, 2100)?).unwrap_or(today.year());
    let (consent, _) = consent(s);
    let folder = invoice::folder();
    let loaded = books::load(&s.config, &folder, today);
    let currency = loaded.settings.currency.clone();
    let money = |m: i64| s.tr.money_in(sioul_core::money::Money(m), &currency);
    // What is open: invoices by their project; budgets by the budgets' gate.
    let (open_budgets, kept_labels): (std::collections::BTreeSet<String>, std::collections::BTreeSet<String>) = loaded
        .money
        .as_ref()
        .map(|m| {
            let (ledger, _, kept) = super::read::open_ledger(m.counted.clone(), &consent);
            (ledger.budgets.iter().map(|b| b.id.clone()).collect(), kept)
        })
        .unwrap_or_default();
    let invoice_open = |i: &invoice::Invoice| consent.allows_projects([i.project.as_str()]);
    let shown_invoices: Vec<invoice::Invoice> = loaded.invoices.iter().filter(|i| invoice_open(i)).cloned().collect();
    let input = books::Input { invoices: &shown_invoices, ..loaded.input() };
    let (first, last) = (Date::new(year, 1, 1).unwrap_or(today), Date::new(year, 12, 31).unwrap_or(today).min(today));
    // Rows of every invoice's (to keep a payment's bank line from coming back as a receipt of its own), then those open.
    let every = books::rows(&loaded.input(), first, last, &s.tr);
    // A receipt no invoice describes: its budget open; one an invoice says (matched or paid): its project too.
    let movement = |r: &books::Row| r.payment.is_empty() && !r.address.starts_with("sioul:invoice/");
    let open_row = |r: &books::Row| (!movement(r) || (open_budgets.contains(&r.budget) && !kept_labels.contains(&r.client))) && (r.invoice.is_empty() || consent.allows_projects([r.project.as_str()]));
    let shown: Vec<books::Row> = every.iter().filter(|r| open_row(r)).cloned().collect();
    let closed = every.len() - shown.len();
    let mut lines = vec![format!("The books of {year}, in {currency}.")];
    let total: i64 = shown.iter().filter_map(|r| r.without_tax).sum();
    lines.push(format!("Received excluding VAT: {}{}.", money(total), if closed > 0 { " (only what is shown)" } else { "" }));
    let declared: Vec<declarations::Declaration> = declarations::periods_of_year(&loaded.settings, year, today).into_iter().map(|(a, b, f)| declarations::of_rows(&loaded.settings, &shown, a, b, f, &s.tr)).collect();
    for d in &declared {
        lines.extend(declarations::sentences(&s.tr, d, &currency));
    }
    let summaries = vat::between(&input, first, last);
    for summary in &summaries {
        lines.extend(vat::sentences(&s.tr, summary, &currency));
    }
    let des_months = des::between(&input, first, last);
    for d in &des_months {
        lines.extend(des::sentences(&s.tr, d, &currency));
    }
    // Each thing kept counted once: a payment kept, and an invoice kept that has no payment that year.
    let kept_rows: std::collections::BTreeSet<&str> = every.iter().filter(|r| !open_row(r)).map(|r| r.invoice.as_str()).collect();
    let kept = closed + loaded.invoices.iter().filter(|i| !invoice_open(i) && !kept_rows.contains(i.number.as_str())).count();
    let watched = if kept == 0 && year == today.year() { thresholds::watch(&loaded.input(), today, &s.tr) } else { Vec::new() };
    lines.extend(watched.iter().map(|w| thresholds::sentence(&s.tr, w, &currency)));
    if kept > 0 && year == today.year() {
        lines.push("The thresholds are not given while things are kept from agents: they add up everything.".into());
    }
    if shown.is_empty() && kept == 0 {
        lines.push("No payment received that year.".into());
    }
    lines.extend(access::left_out(kept));
    let data = json!({
        "year": year,
        "currency": currency,
        "rows": shown.iter().map(|r| json!({
            "day": r.day.to_string(), "client": r.client, "nature": r.nature, "invoice": r.invoice, "means": r.means, "cash": r.cash,
            "amount": r.amount.map(&money), "without_tax": r.without_tax.map(&money),
            "foreign": r.foreign.as_ref().map(|(m, code)| s.tr.money_in(sioul_core::money::Money(*m), code)),
            "address": r.address,
        })).collect::<Vec<_>>(),
        "declarations": declared.iter().map(|d| json!({
            "from": d.from.to_string(), "to": d.to.to_string(), "due": d.due.to_string(), "turnover": money(d.turnover), "contributions": money(d.contributions),
            "activities": d.lines.iter().map(|l| json!({ "activity": l.activity, "name": l.name, "turnover": money(l.turnover) })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "vat": summaries.iter().map(|v| json!({ "from": v.from.to_string(), "to": v.to.to_string(), "tax": money(v.tax), "base": money(v.base),
            "rates": v.entries.iter().map(|e| json!({ "category": e.category, "rate": s.tr.percent_bp(e.rate), "base": money(e.base), "tax": money(e.tax) })).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        "des": des_months.iter().map(|d| json!({ "month": d.key(), "due": d.due.to_string(), "total": money(d.total),
            "invoices": d.invoices.iter().map(|c| json!({ "number": c.number, "client": c.client, "vat": c.vat, "amount": s.tr.money_in(sioul_core::money::Money(c.amount), &c.currency) })).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        "thresholds": watched.iter().map(|w| json!({ "id": w.id, "kind": w.kind, "amount": money(w.amount), "turnover": money(w.turnover), "near": w.near.map(|d| d.to_string()), "crossed": w.crossed.map(|d| d.to_string()) })).collect::<Vec<_>>(),
        "kept_from_agents": kept,
    });
    Ok(Answer { text: rows(&lines), data })
}

// Reminders.

pub fn reminders(s: &Session, args: &Args) -> Result<Answer, String> {
    let days = args.number("days", 14, 1, 92)?;
    let now = sioul_core::zone::now();
    let stamp = now.timestamp().as_second();
    let (all, _) = reminders::gather(&s.config, &s.tr, &now);
    let (consent, loaded) = consent(s);
    // What each is about: a task, an event (by its file), a budget, a paper, a
    // contract; and what its words name besides (the task a wait came after),
    // never one kept from agents (review of 36002ea, low 27).
    let allowed = |r: &reminders::Reminder| {
        let event = loaded.events.iter().find(|e| e.key == r.target);
        let about = match event {
            Some(event) => consent.allows(&links::event_uri(&event.uid)),
            None if r.target.starts_with("sioul:") => consent.allows(&r.target),
            None => consent.outside,
        };
        about && r.names.as_deref().is_none_or(|other| consent.allows(other))
    };
    let coming: Vec<&reminders::Reminder> = all.iter().filter(|r| r.until > stamp && r.at < stamp + days * 86_400).collect();
    let shown: Vec<&&reminders::Reminder> = coming.iter().filter(|r| allowed(r)).collect();
    let closed = coming.len() - shown.len();
    let told = reminders::told_dir();
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for r in shown {
        let when = crate::date(s, Some(r.at));
        let body = if r.body.is_empty() { String::new() } else { format!(" — {}", r.body) };
        lines.push(format!("{when} · {}{body}", r.title));
        let about = loaded.events.iter().find(|e| e.key == r.target).map_or_else(|| r.target.clone(), |e| links::event_uri(&e.uid));
        out.push(json!({ "at": instant(r.at), "until": instant(r.until), "kind": r.kind, "title": r.title, "body": r.body, "about": about, "work": r.work, "told": reminders::told(&told, &r.key) }));
    }
    if out.is_empty() {
        lines.push(s.tr.text("remind-nothing", None));
    }
    lines.extend(access::left_out(closed));
    lines.extend(access::health_doubt(&consent));
    Ok(Answer { text: rows(&lines), data: json!({ "days": days, "reminders": out, "kept_from_agents": closed }) })
}

// The moment now.

/// What now is for, as the status line says it: its kind ("work",
/// "leisure", "pause"…) and its line ("Leisure until 07:00."; "" when it says
/// nothing).
pub(super) fn moment(s: &Session) -> (String, String) {
    let (kind, line, ..) = moment_read(s);
    (kind, line)
}

/// The moment and what it was read from: its kind and its line, the mode,
/// the overrides, what the hours hold, and now.
pub(super) type Moment = (String, String, sioul_core::quiet::Mode, sioul_core::quiet::Overrides, sioul_core::quiet::Blocks, Zoned);

pub(super) fn moment_read(s: &Session) -> Moment {
    use sioul_core::quiet::{self, Overrides, Reason};
    let config = &s.config;
    let now = sioul_core::zone::now();
    let overrides = Overrides::load(&Overrides::default_path());
    let blocks = quiet::Blocks::read_now(&now);
    let mode = quiet::mode(&config.week_hours(), &config.time_off, &overrides, &blocks, &now);
    let until = mode.until.as_ref().map(|z| quiet::until_text(&s.tr, z, &now)).unwrap_or_default();
    let mut args = sioul_core::i18n::args();
    args.set("until", until.clone());
    args.set("label", if mode.label.trim().is_empty() { "none".to_string() } else { mode.label.clone() });
    let nothing = sioul_core::pause::nothing_now(&overrides, &config.free_time);
    // What now is for, in the words of the status line.
    let line = match mode.reason {
        Reason::FreeTime => s.tr.text(if nothing { "mode-free-time-nothing" } else { "mode-free-time" }, None),
        Reason::Paused => s.tr.text("mode-paused", None),
        Reason::NoHours => String::new(),
        _ if until.is_empty() => String::new(),
        Reason::Working | Reason::Extended => s.tr.text("home-card-work", Some(&args)),
        Reason::TimeOff => s.tr.text("mode-time-off", Some(&args)),
        Reason::WorkingLate => s.tr.text("mode-working-late", Some(&args)),
        Reason::WorkNow => s.tr.text("mode-work-now-line", Some(&args)),
        Reason::AdminTime => s.tr.text("mode-admin", Some(&args)),
        Reason::DoneForTheDay => s.tr.text("mode-quiet", Some(&args)),
        Reason::Evening | Reason::DayOff => s.tr.text("mode-leisure", Some(&args)),
        Reason::Meal => s.tr.text("mode-meal", Some(&args)),
        Reason::WindingDown => s.tr.text("mode-wind-down", Some(&args)),
        Reason::Nap => s.tr.text("mode-nap", Some(&args)),
        Reason::Sleep => s.tr.text("mode-sleep", Some(&args)),
    };
    let kind = match mode.reason {
        Reason::Paused => "pause",
        Reason::FreeTime => "free-time",
        _ => mode.time.id(),
    };
    (kind.to_string(), line, mode, overrides, blocks, now)
}

pub fn now(s: &Session, _args: &Args) -> Result<Answer, String> {
    use sioul_core::attention::{self, Attention, Row};
    use sioul_core::reach::Channel;
    use sioul_core::everywhere::{self, Why};
    let config = &s.config;
    let (kind, line, mode, overrides, blocks, now) = moment_read(s);
    let kind = kind.as_str();
    let stamp = now.timestamp().as_second();
    let nothing = sioul_core::pause::nothing_now(&overrides, &config.free_time);
    let until = mode.until.as_ref().map(|z| sioul_core::quiet::until_text(&s.tr, z, &now)).unwrap_or_default();
    // Do-not-disturb, on every device: why it holds, until when.
    // As the window reads them: the devices gone from the sharing as As usual, the pause ending Reachable (F1, F7).
    let sources = everywhere::sources_at(&overrides, &blocks, &now);
    let dnd = everywhere::now(&config.dnd, &sources, everywhere::read_switch("").latest().as_ref(), stamp);
    let dnd_until = dnd.until().map(instant).unwrap_or_default();
    // What reaches the person now, row by row of the matrix of what reaches them.
    let matrix = Attention::of(config);
    let mut moment = attention::Now::of(&mode).layers(attention::Slots::load(&attention::Slots::default_path()).at(&now), dnd.gates());
    moment.nothing = mode.free() && nothing;
    moment.reachable = dnd.reachable.is_some();
    let mut reach = Vec::new();
    let mut by_group: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
    for row in Row::ALL {
        let level = matrix.level(row, &moment);
        let label = attention::row_label(&s.tr, row);
        let said = attention::level_label(&s.tr, row, level);
        let group = match row {
            Row::People(Channel::Mail, _) => "Mail",
            Row::People(Channel::Calls, _) => "Calls",
            Row::People(Channel::Messages, _) => "Messages from other apps",
            Row::Own(_) => "Sioul's own",
        };
        by_group.entry(group).or_default().push(format!("{label}: {said}"));
        reach.push(json!({ "row": row.id(), "label": label, "level": level.id(), "said": said }));
    }
    let mut lines = vec![format!("Now: {kind}{}", if line.is_empty() { String::new() } else { format!(". {line}") })];
    lines.push(if dnd.on() {
        let why = dnd.why().map(Why::id).unwrap_or_default();
        format!("Do-not-disturb: on ({why}){}", if dnd_until.is_empty() { String::new() } else { format!(", until {dnd_until}") })
    } else {
        "Do-not-disturb: off".into()
    });
    lines.push("What reaches the person now:".into());
    for group in ["Mail", "Calls", "Messages from other apps", "Sioul's own"] {
        if let Some(rows) = by_group.get(group) {
            lines.push(format!("  {group}: {}", rows.join("; ")));
        }
    }
    lines.push("Respect this: outside work and admin hours, in a meal, asleep, in a pause or in Free time, do not press the person with admin; say what can wait, and when it comes back.".into());
    let data = json!({
        "time": mode.time.id(), "kind": kind, "reason": serde_json::to_value(&mode.reason).unwrap_or(Value::Null), "quiet": mode.quiet,
        "until": mode.until.as_ref().map(|z| instant(z.timestamp().as_second())), "until_said": until, "line": line,
        "back": mode.back.as_ref().map(|z| instant(z.timestamp().as_second())),
        "dnd": { "on": dnd.on(), "why": dnd.why().map(Why::id), "until": dnd.until().map(instant) },
        "reaches": reach,
    });
    Ok(Answer { text: rows(&lines), data })
}
