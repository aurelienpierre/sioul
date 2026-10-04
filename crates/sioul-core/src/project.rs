// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A project, or a case, on one page: everything dated in it on one line of
//! time (tasks asked and done, events, mail, notes changed, time noted,
//! invoices), what is coming first, then what happened, newest first. Mail
//! belongs to it by its routes or by a link; the rest by its case (REFID) or
//! a link.

use crate::cases::Case;
use crate::i18n::Translator;
use crate::invoice::Invoice;
use crate::links::{self, Kind, Loaded};
use crate::money::Money;
use crate::tasks::Status;
use crate::timereport::{Entry, duration};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::Serialize;
use std::collections::BTreeMap;

/// One thing on the project's line of time.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Moment {
    pub when: i64,
    /// "Monday 5 October", in words.
    pub date: String,
    /// "task", "event", "mail", "note", "time", "invoice".
    pub kind: String,
    pub title: String,
    /// "Done", "Date asked", "From Jane", "2 h 30 noted"…
    pub detail: String,
    /// What opens it: its address, for the window's `openThing`.
    pub uri: String,
    pub key: String,
    /// Still to come.
    pub coming: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceRow {
    pub number: String,
    pub date: String,
    pub total: String,
    pub paid: bool,
}

/// A project's page.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectView {
    pub id: String,
    pub title: String,
    pub is_project: bool,
    pub status: String,
    pub client: String,
    pub rate: f64,
    pub budget: String,
    /// "personal": it stays in view in quiet time.
    pub area: String,
    pub open_tasks: usize,
    pub done_tasks: usize,
    /// All the time noted, and what is left to bill.
    pub time: String,
    pub unbilled: String,
    pub unbilled_amount: String,
    pub moments: Vec<Moment>,
    pub invoices: Vec<InvoiceRow>,
}

/// Every case and project, for the list: open ones first, projects first.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectRow {
    pub id: String,
    pub title: String,
    pub is_project: bool,
    pub status: String,
    pub client: String,
    pub open_tasks: usize,
    pub unbilled: String,
    /// Yours outside work: listed in quiet time too.
    pub personal: bool,
}

pub fn rows(loaded: &Loaded, entries: &[Entry], cases: &[Case]) -> Vec<ProjectRow> {
    let mut out: Vec<ProjectRow> = cases
        .iter()
        .map(|c| {
            let unbilled: u32 = entries.iter().filter(|e| e.project == c.id && e.billable && e.invoice.is_empty()).map(|e| e.minutes).sum();
            ProjectRow {
                id: c.id.clone(),
                title: c.title.clone(),
                is_project: c.is_project(),
                status: c.status.clone().unwrap_or_default(),
                client: c.client.clone().unwrap_or_default(),
                open_tasks: loaded.tasks.iter().filter(|t| t.status.is_open() && t.cases.iter().any(|x| x == &c.id)).count(),
                unbilled: if unbilled > 0 { duration(unbilled) } else { String::new() },
                personal: c.area.as_deref() == Some("personal"),
            }
        })
        .collect();
    out.sort_by_key(|r| (r.status == "closed", !r.is_project, r.title.to_lowercase()));
    out
}

/// One project's page, `now` in Unix seconds.
pub fn view(loaded: &Loaded, case: &Case, entries: &[Entry], invoices: &[Invoice], default_rate: f64, now: i64, tr: &Translator) -> ProjectView {
    let zone = TimeZone::system();
    let words = |when: i64| Timestamp::from_second(when).map(|t| tr.day(t.to_zoned(zone.clone()).date())).unwrap_or_default();
    let mut moments: Vec<Moment> = Vec::new();
    let mut push = |when: i64, kind: &str, title: String, detail: String, uri: String, key: String| {
        if when > 0 {
            moments.push(Moment { when, date: words(when), kind: kind.into(), title, detail, uri, key, coming: when > now });
        }
    };
    let me = links::case_uri(&case.id);
    let world = loaded.world();
    let tied: Vec<links::Related> = world.related(&me);
    let in_case = |uri: &str| tied.iter().any(|r| r.uri == uri);
    // Tasks: when asked, when done.
    let (mut open, mut done) = (0, 0);
    for task in &loaded.tasks {
        if !task.cases.iter().any(|c| c == &case.id) && !in_case(&links::task_uri(&task.uid)) {
            continue;
        }
        let uri = links::task_uri(&task.uid);
        if task.status.is_open() {
            open += 1;
        } else if task.status == Status::Completed {
            done += 1;
        }
        if let Some(at) = task.completed {
            push(at, "task", task.title.clone(), tr.text("project-done", None), uri.clone(), task.key.clone());
        }
        if task.status.is_open()
            && let Some(due) = task.due_date().and_then(|d| d.to_zoned(zone.clone()).ok())
        {
            push(due.timestamp().as_second(), "task", task.title.clone(), tr.text("project-asked", None), uri, task.key.clone());
        }
    }
    // Events.
    for event in loaded.events.iter().filter(|e| e.cases.iter().any(|c| c == &case.id) || in_case(&links::event_uri(&e.uid))) {
        push(event.start, "event", event.summary.clone(), String::new(), links::event_uri(&event.uid), event.key.clone());
    }
    // Mail: by its routes, or tied by hand; then the rest of their conversations.
    let seeds: std::collections::BTreeSet<String> = loaded
        .mail
        .iter()
        .filter(|(id, mail)| case.routes.iter().any(|r| r.takes_header(&mail.address, &mail.subject)) || in_case(&links::mail_uri(id)))
        .map(|(id, _)| id.clone())
        .collect();
    for id in loaded.mail.conversations(&seeds) {
        let Some(mail) = loaded.mail.get(&id) else { continue };
        let mut args = crate::i18n::args();
        args.set("sender", mail.from.clone());
        push(mail.date, "mail", if mail.subject.trim().is_empty() { tr.text("mail-no-subject", None) } else { mail.subject.clone() }, tr.text("project-from", Some(&args)), links::mail_uri(&id), mail.path.display().to_string());
    }
    // Notes: those of the case's record, or tied to it.
    if let Some(vault) = loaded.vault.as_ref() {
        for note in vault.notes.iter().filter(|n| case.files.iter().any(|f| f == &n.path) || in_case(&n.uri())) {
            push(note.modified, "note", note.title.clone(), tr.text("project-note-changed", None), note.uri(), vault.root.join(&note.path).display().to_string());
        }
    }
    // Time: a line a day.
    let mut by_day: BTreeMap<jiff::civil::Date, (i64, u32)> = BTreeMap::new();
    for entry in entries.iter().filter(|e| e.project == case.id) {
        let day = by_day.entry(entry.day).or_insert((entry.start, 0));
        day.0 = day.0.max(entry.start);
        day.1 += entry.minutes;
    }
    for (_, (when, minutes)) in by_day {
        let mut args = crate::i18n::args();
        args.set("time", duration(minutes));
        push(when, "time", tr.text("project-time-noted", Some(&args)), String::new(), String::new(), String::new());
    }
    // Invoices.
    for invoice in invoices.iter().filter(|i| i.project == case.id) {
        let at = invoice.date.parse::<jiff::civil::Date>().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map_or(0, |z| z.timestamp().as_second());
        let mut args = crate::i18n::args();
        args.set("number", invoice.number.clone());
        push(at, "invoice", tr.text("project-invoice", Some(&args)), tr.money(invoice.total()), String::new(), invoice.number.clone());
    }
    // What is coming, the nearest first; then what happened, the newest first.
    moments.sort_by_key(|m| if m.coming { (0, m.when) } else { (1, -m.when) });
    moments.truncate(300);
    let all: u32 = entries.iter().filter(|e| e.project == case.id).map(|e| e.minutes).sum();
    let unbilled: u32 = entries.iter().filter(|e| e.project == case.id && e.billable && e.invoice.is_empty()).map(|e| e.minutes).sum();
    let rate = case.rate.unwrap_or(default_rate);
    ProjectView {
        id: case.id.clone(),
        title: case.title.clone(),
        is_project: case.is_project(),
        status: case.status.clone().unwrap_or_default(),
        client: case.client.clone().unwrap_or_default(),
        rate,
        budget: case.budget.clone().unwrap_or_default(),
        area: case.area.clone().unwrap_or_default(),
        open_tasks: open,
        done_tasks: done,
        time: if all > 0 { duration(all) } else { String::new() },
        unbilled: if unbilled > 0 { duration(unbilled) } else { String::new() },
        unbilled_amount: if unbilled > 0 && rate > 0.0 { tr.money(crate::timereport::amount(unbilled, rate)) } else { String::new() },
        moments,
        invoices: invoices
            .iter()
            .filter(|i| i.project == case.id)
            .rev()
            .map(|i| InvoiceRow { number: i.number.clone(), date: i.date.clone(), total: tr.money(Money(i.total_cents)), paid: i.paid })
            .collect(),
    }
}

/// The kind of a moment, as `links::Kind` names it, for the window's icons.
pub fn kind_icon(kind: &str) -> Kind {
    match kind {
        "task" => Kind::Task,
        "event" => Kind::Event,
        "mail" => Kind::Mail,
        "note" => Kind::Note,
        _ => Kind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cases::Route;
    use crate::tasks::Task;

    #[test]
    fn a_project_on_one_line_of_time() {
        let now = 1_791_000_000;
        let lumen = Case {
            id: "lumen".into(),
            title: "Studio Lumen".into(),
            kind: Some("project".into()),
            rate: Some(60.0),
            routes: vec![Route { from_domains: vec!["lumen.example.net".into()], ..Route::default() }],
            ..Case::default()
        };
        let done = Task { uid: "d".into(), title: "Mock-ups".into(), status: Status::Completed, completed: Some(now - 86_400), cases: vec!["lumen".into()], ..Task::default() };
        let coming = Task { uid: "c".into(), title: "Deliver the site".into(), due: "2030-01-15".into(), cases: vec!["lumen".into()], ..Task::default() };
        let other = Task { uid: "o".into(), title: "Housing".into(), cases: vec!["housing".into()], ..Task::default() };
        let mut mail = crate::mailindex::MailIndex::default();
        mail.insert("q@lumen.example.net", crate::mailindex::MailRef { subject: "Quote".into(), from: "Jane".into(), address: "jane@lumen.example.net".into(), date: now - 3 * 86_400, ..Default::default() });
        mail.insert("x@else.example.org", crate::mailindex::MailRef { subject: "Other".into(), address: "x@else.example.org".into(), date: now - 86_400, ..Default::default() });
        // Your answer to the quote, from your own address: in its conversation, so in the project.
        mail.insert("a@you.example.org", crate::mailindex::MailRef { subject: "Re: Quote".into(), address: "you@you.example.org".into(), date: now - 2 * 86_400 - 3600, refs: vec!["q@lumen.example.net".into()], ..Default::default() });
        let loaded = Loaded { tasks: vec![done, coming, other], mail, ..Loaded::default() };
        let entries = vec![Entry {
            key: "k".into(),
            start: now - 2 * 86_400,
            day: Timestamp::from_second(now - 2 * 86_400).unwrap().to_zoned(TimeZone::system()).date(),
            minutes: 150,
            project: "lumen".into(),
            task: String::new(),
            title: "Meeting".into(),
            note: String::new(),
            billable: true,
            invoice: String::new(),
        }];
        let tr = Translator::new("en");
        let page = view(&loaded, &lumen, &entries, &[], 40.0, now, &tr);
        let line: Vec<(&str, &str, bool)> = page.moments.iter().map(|m| (m.kind.as_str(), m.title.as_str(), m.coming)).collect();
        assert_eq!(line, vec![("task", "Deliver the site", true), ("task", "Mock-ups", false), ("time", "2 h 30 noted", false), ("mail", "Re: Quote", false), ("mail", "Quote", false)]);
        assert_eq!((page.open_tasks, page.done_tasks, page.unbilled.as_str()), (1, 1, "2 h 30"));
        assert_eq!(page.unbilled_amount, tr.money(Money(15000)));
        let listed = rows(&loaded, &entries, &[Case { id: "housing".into(), title: "Housing".into(), ..Case::default() }, lumen]);
        assert_eq!(listed[0].id, "lumen", "projects first");
    }
}
