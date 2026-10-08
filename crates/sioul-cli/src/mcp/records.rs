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
use jiff::tz::TimeZone;
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
    Timestamp::from_second(seconds).map(|t| t.to_zoned(TimeZone::system()).strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()).unwrap_or_default()
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
    let (consent, _) = consent(s);
    let today = Zoned::now().date();
    let before = wallet.papers.len();
    // A paper is in no project unless tied to one.
    let shown: Vec<&papers::Paper> = wallet.papers.iter().filter(|p| consent.allows(&links::paper_uri(&p.id))).collect();
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
        }));
    }
    if out.is_empty() && closed == 0 {
        lines.push("No paper is kept in the wallet yet.".into());
    }
    lines.extend(access::left_out(closed));
    Ok(Answer { text: rows(&lines), data: json!({ "papers": out, "kept_from_agents": closed }) })
}

pub fn contracts(s: &Session, _args: &Args) -> Result<Answer, String> {
    let list = contracts::Contracts::load(&store(s)?)?;
    let (consent, _) = consent(s);
    let today = Zoned::now().date();
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
    let mut shown: Vec<&letters::Letter> = all.list.iter().filter(|l| consent.allows_projects([l.project.as_str()])).collect();
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
    let today = Zoned::now().date();
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
    let entries = timereport::entries(&timelog::sessions(), &loaded.tasks, &loaded.projects, &TimeZone::system());
    let in_range: Vec<&timereport::Entry> = entries.iter().filter(|e| e.day >= from && e.day <= to && project.as_ref().is_none_or(|p| &e.project == p)).collect();
    // Time of a project kept from agents, or of a task kept from them; in none, as things outside projects are.
    let allowed = |e: &timereport::Entry| consent.allows_projects([e.project.as_str()]) && (e.task.is_empty() || consent.allows(&links::task_uri(&e.task)));
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
    Ok(Answer { text: rows(&lines), data: json!({ "from": from.to_string(), "to": to.to_string(), "projects": projects, "sessions": sessions, "kept_from_agents": closed }) })
}

pub fn invoices(s: &Session, _args: &Args) -> Result<Answer, String> {
    let (consent, _) = consent(s);
    let all = invoice::all_in(&invoice::folder());
    let shown: Vec<&invoice::Invoice> = all.iter().filter(|i| consent.allows_projects([i.project.as_str()])).collect();
    let closed = all.len() - shown.len();
    let mut lines = Vec::new();
    let mut out = Vec::new();
    for i in shown.iter().rev() {
        let paid = if i.paid { "paid" } else { "not paid yet" };
        let client = i.client.lines().next().unwrap_or_default();
        lines.push(format!("{} · {} · {client} · {} · {paid}{}", i.number, i.date, s.tr.money(i.total()), if i.due.is_empty() || i.paid { String::new() } else { format!(" (due {})", i.due) }));
        out.push(json!({ "number": i.number, "date": i.date, "due": i.due, "project": i.project, "project_title": i.project_title, "client": client, "amount": s.tr.money(i.total()), "currency": i.currency, "paid": i.paid }));
    }
    if out.is_empty() && closed == 0 {
        lines.push("No invoice issued yet.".into());
    }
    lines.extend(access::left_out(closed));
    Ok(Answer { text: rows(&lines), data: json!({ "invoices": out, "kept_from_agents": closed }) })
}

// Reminders.

pub fn reminders(s: &Session, args: &Args) -> Result<Answer, String> {
    let days = args.number("days", 14, 1, 92)?;
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let (all, _) = reminders::gather(&s.config, &s.tr, &now);
    let (consent, loaded) = consent(s);
    // What each is about: a task, an event (by its file), a budget, a paper, a contract.
    let allowed = |r: &reminders::Reminder| {
        let event = loaded.events.iter().find(|e| e.key == r.target);
        match event {
            Some(event) => consent.allows(&links::event_uri(&event.uid)),
            None if r.target.starts_with("sioul:") => consent.allows(&r.target),
            None => consent.outside,
        }
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
    Ok(Answer { text: rows(&lines), data: json!({ "days": days, "reminders": out, "kept_from_agents": closed }) })
}

// The moment now.

pub fn now(s: &Session, _args: &Args) -> Result<Answer, String> {
    use sioul_core::attention::{self, Attention, Row};
    use sioul_core::reach::Channel;
    use sioul_core::everywhere::{self, Sources, Switch, Why};
    use sioul_core::quiet::{self, Overrides, Reason};
    let config = &s.config;
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
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
    // Do-not-disturb, on every device: why it holds, until when.
    let free = overrides.free_from().map(|since| (since, sioul_core::pause::free_until(since, &blocks, now.time_zone())));
    let sleep = blocks.at(stamp, &["sleep", "nap"]).map(|k| (k.start, k.end));
    let focus = timelog::running().and_then(|r| everywhere::focus(&r, stamp));
    let sources = Sources { paused: overrides.paused_from(), free, sleep, focus };
    let dnd = everywhere::now(&config.dnd, &sources, Switch::load(&Switch::default_path()).latest().as_ref(), stamp);
    let dnd_until = dnd.until().map(instant).unwrap_or_default();
    // What reaches the person now, row by row of the matrix of what reaches them.
    let matrix = Attention::of(config);
    let mut moment = attention::Now::of(&mode).layers(attention::Slots::load(&attention::Slots::default_path()).at(&now), dnd.gates());
    moment.nothing = mode.free() && nothing;
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
