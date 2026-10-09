// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A project on one page: everything dated in it on one line of
//! time (tasks asked and done, events, mail, notes changed, time noted,
//! invoices), what is coming first, then what happened, newest first. Mail
//! belongs to it by its routes or by a link; the rest by its project (REFID) or
//! a link.

use crate::projects::Project;
use crate::i18n::Translator;
use crate::invoice::Invoice;
use crate::links::{self, Kind, Loaded};
use crate::mailindex::{MailIndex, Threads};
use crate::money::Money;
use crate::porch::{Admission, Gate};
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
    /// Work for a client, its time billable (`Project::is_for_client`).
    pub for_client: bool,
    pub status: String,
    pub client: String,
    pub rate: f64,
    pub budget: String,
    /// "personal": it stays in view in quiet time.
    pub area: String,
    /// Open to AI agents (`Project::ai`).
    pub ai: bool,
    pub open_tasks: usize,
    pub done_tasks: usize,
    /// All the time noted, and what is left to bill.
    pub time: String,
    pub unbilled: String,
    pub unbilled_amount: String,
    pub moments: Vec<Moment>,
    pub invoices: Vec<InvoiceRow>,
}

/// Every project, for the list: open ones first, those for a client first.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectRow {
    pub id: String,
    pub title: String,
    /// Work for a client, its time billable.
    pub for_client: bool,
    pub status: String,
    pub client: String,
    pub open_tasks: usize,
    pub unbilled: String,
    /// Yours outside work: listed in quiet time too.
    pub personal: bool,
    /// Open to AI agents (`Project::ai`).
    pub ai: bool,
}

pub fn rows(loaded: &Loaded, entries: &[Entry], projects: &[Project]) -> Vec<ProjectRow> {
    let mut out: Vec<ProjectRow> = projects
        .iter()
        .map(|c| {
            let unbilled: u32 = entries.iter().filter(|e| e.project == c.id && e.billable && e.invoice.is_empty()).map(|e| e.minutes).sum();
            ProjectRow {
                id: c.id.clone(),
                title: c.title.clone(),
                for_client: c.is_for_client(),
                status: c.status.clone().unwrap_or_default(),
                client: c.client.clone().unwrap_or_default(),
                open_tasks: loaded.tasks.iter().filter(|t| t.status.is_open() && t.projects.iter().any(|x| x == &c.id)).count(),
                unbilled: if unbilled > 0 { duration(unbilled) } else { String::new() },
                personal: c.area.as_deref() == Some("personal"),
                ai: c.ai,
            }
        })
        .collect();
    out.sort_by_key(|r| (r.status == "closed", !r.for_client, r.title.to_lowercase()));
    out
}

/// What a message is to projects (`porch::Admission`), read once per
/// message (by `Gate`), for every project that asks.
pub type Admissions = BTreeMap<String, Option<Admission>>;

/// The mail a project takes, as its page shows it and agents' consent reads
/// it (`consent::Consent::of`): by its routes on the sender and the subject,
/// by a tie (`tied`: the messages' addresses, `by_hand` those you tied
/// yourself), then the rest of their conversations (`threads`). With `gate`,
/// each as the Porch takes it: never what it sets aside, nor by its sender's
/// address or domain when nothing authenticates it; what you tied by hand,
/// as you tied it; conversations followed through the messages let in only.
/// Without, every message as it stands. Each message's id, and whether its
/// sender is proven (always, without a gate).
pub fn mail_of(mail: &MailIndex, threads: &Threads<'_>, project: &Project, tied: &std::collections::BTreeSet<String>, by_hand: &std::collections::BTreeSet<String>, gate: Option<&Gate>, admissions: &mut Admissions) -> Vec<(String, bool)> {
    let mut admit = |id: &str, path: &std::path::Path| match gate {
        Some(gate) => admissions.entry(id.to_string()).or_insert_with(|| gate.admission_of_file(path)).clone(),
        None => Some(Admission::Admitted),
    };
    let mut seeds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (id, message) in mail.iter() {
        let uri = links::mail_uri(id);
        let is_tied = tied.contains(&uri);
        let routed = |address: &str| project.routes.iter().any(|r| r.takes_header(address, &message.subject));
        let taken = by_hand.contains(&uri)
            || ((is_tied || routed(&message.address))
                && match admit(id, &message.path) {
                    Some(Admission::Admitted) => true,
                    // Nothing proves its sender: a tie, or a route that names no sender.
                    Some(Admission::Unproven) => is_tied || routed(""),
                    _ => false,
                });
        if taken {
            seeds.insert(id.clone());
        }
    }
    let shown = threads.through(&seeds, |id, message| by_hand.contains(&links::mail_uri(id)) || admit(id, &message.path).is_some_and(|a| !a.refused()));
    shown
        .into_iter()
        .map(|id| {
            let proven = mail.get(&id).is_some_and(|message| admit(&id, &message.path) == Some(Admission::Admitted));
            (id, proven)
        })
        .collect()
}

/// The messages you tied to a project yourself (`links.toml`, any tie but
/// the "project" ones Sioul makes as mail arrives), by their address.
fn tied_by_hand(loaded: &Loaded, world: &links::World<'_>, project: &str) -> std::collections::BTreeSet<String> {
    loaded
        .local
        .links
        .iter()
        .filter(|edge| !matches!(edge.how.as_str(), "project" | "case"))
        .filter_map(|edge| {
            let (from, to) = (world.canonical(&edge.from), world.canonical(&edge.to));
            if from == project { Some(to) } else if to == project { Some(from) } else { None }
        })
        .filter(|uri| links::kind_of(uri) == Kind::Mail)
        .collect()
}

/// One project's page, `now` in Unix seconds. Its mail is what `gate`, the
/// Porch's checks, lets a project take.
#[allow(clippy::too_many_arguments)]
pub fn view(loaded: &Loaded, project: &Project, entries: &[Entry], invoices: &[Invoice], default_rate: f64, now: i64, tr: &Translator, gate: &Gate) -> ProjectView {
    let zone = TimeZone::system();
    let words = |when: i64| Timestamp::from_second(when).map(|t| tr.day(t.to_zoned(zone.clone()).date())).unwrap_or_default();
    let mut moments: Vec<Moment> = Vec::new();
    let mut push = |when: i64, kind: &str, title: String, detail: String, uri: String, key: String| {
        if when > 0 {
            moments.push(Moment { when, date: words(when), kind: kind.into(), title, detail, uri, key, coming: when > now });
        }
    };
    let me = links::project_uri(&project.id);
    let world = loaded.world();
    let tied: Vec<links::Related> = world.related(&me);
    let in_project = |uri: &str| tied.iter().any(|r| r.uri == uri);
    // Tasks: when asked, when done.
    let (mut open, mut done) = (0, 0);
    for task in &loaded.tasks {
        if !task.projects.iter().any(|c| c == &project.id) && !in_project(&links::task_uri(&task.uid)) {
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
    for event in loaded.events.iter().filter(|e| e.projects.iter().any(|c| c == &project.id) || in_project(&links::event_uri(&e.uid))) {
        push(event.start, "event", event.summary.clone(), String::new(), links::event_uri(&event.uid), event.key.clone());
    }
    // Mail: by its routes, or tied to the project; then the rest of their
    // conversations. Each as the Porch takes it (`porch::Admission`, read by
    // `gate`): never what it sets aside, nor by its sender's address or domain
    // when nothing authenticates it; what you tied by hand, as you tied it.
    let by_hand = tied_by_hand(loaded, &world, &me);
    let tied_mail: std::collections::BTreeSet<String> = tied.iter().filter(|r| r.kind == Kind::Mail).map(|r| r.uri.clone()).collect();
    for (id, proven) in mail_of(&loaded.mail, &loaded.mail.threads(), project, &tied_mail, &by_hand, Some(gate), &mut Admissions::new()) {
        let Some(mail) = loaded.mail.get(&id) else { continue };
        let mut args = crate::i18n::args();
        args.set("sender", mail.from.clone());
        // A sender nothing proves is said so, as the Porch says it.
        let detail = tr.text(if proven { "project-from" } else { "project-from-unproven" }, Some(&args));
        push(mail.date, "mail", if mail.subject.trim().is_empty() { tr.text("mail-no-subject", None) } else { mail.subject.clone() }, detail, links::mail_uri(&id), mail.path.display().to_string());
    }
    // Notes: those of the project's record, or tied to it.
    if let Some(vault) = loaded.vault.as_ref() {
        for note in vault.notes.iter().filter(|n| project.files.iter().any(|f| f == &n.path) || in_project(&n.uri())) {
            push(note.modified, "note", note.title.clone(), tr.text("project-note-changed", None), note.uri(), vault.root.join(&note.path).display().to_string());
        }
    }
    // Time: a line a day.
    let mut by_day: BTreeMap<jiff::civil::Date, (i64, u32)> = BTreeMap::new();
    for entry in entries.iter().filter(|e| e.project == project.id) {
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
    for invoice in invoices.iter().filter(|i| i.project == project.id) {
        let at = invoice.date.parse::<jiff::civil::Date>().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map_or(0, |z| z.timestamp().as_second());
        let mut args = crate::i18n::args();
        args.set("number", invoice.number.clone());
        push(at, "invoice", tr.text("project-invoice", Some(&args)), tr.money(invoice.total()), String::new(), invoice.number.clone());
    }
    // What is coming, the nearest first; then what happened, the newest first.
    moments.sort_by_key(|m| if m.coming { (0, m.when) } else { (1, -m.when) });
    moments.truncate(300);
    let all: u32 = entries.iter().filter(|e| e.project == project.id).map(|e| e.minutes).sum();
    let unbilled: u32 = entries.iter().filter(|e| e.project == project.id && e.billable && e.invoice.is_empty()).map(|e| e.minutes).sum();
    let rate = project.rate.unwrap_or(default_rate);
    ProjectView {
        id: project.id.clone(),
        title: project.title.clone(),
        for_client: project.is_for_client(),
        status: project.status.clone().unwrap_or_default(),
        client: project.client.clone().unwrap_or_default(),
        rate,
        budget: project.budget.clone().unwrap_or_default(),
        area: project.area.clone().unwrap_or_default(),
        ai: project.ai,
        open_tasks: open,
        done_tasks: done,
        time: if all > 0 { duration(all) } else { String::new() },
        unbilled: if unbilled > 0 { duration(unbilled) } else { String::new() },
        unbilled_amount: if unbilled > 0 && rate > 0.0 { tr.money(crate::timereport::amount(unbilled, rate)) } else { String::new() },
        moments,
        invoices: invoices
            .iter()
            .filter(|i| i.project == project.id)
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
    use crate::config::{Priority, Source};
    use crate::links::{Edge, LocalLinks};
    use crate::mailindex::MailIndex;
    use crate::porch::{SenderList, Senders};
    use crate::projects::{ProjectStore, Route};
    use crate::tasks::Task;
    use std::path::PathBuf;

    /// Your provider, whose results the checks trust.
    const PROVIDER: &str = "mx.provider.example";

    /// A message as stored: its provider's results (none when ""), the ids it cites.
    fn message(id: &str, from: &str, results: &str, subject: &str, date: i64, cites: &[&str]) -> (String, String) {
        let results = if results.is_empty() { String::new() } else { format!("Authentication-Results: {PROVIDER}; {results}\r\n") };
        let refs = if cites.is_empty() { String::new() } else { format!("References: {}\r\n", cites.iter().map(|c| format!("<{c}>")).collect::<Vec<_>>().join(" ")) };
        let date = Timestamp::from_second(date).unwrap().strftime("%a, %d %b %Y %H:%M:%S +0000");
        (id.to_string(), format!("{results}Message-ID: <{id}>\r\nFrom: {from}\r\nSubject: {subject}\r\nDate: {date}\r\n{refs}\r\nHello.\r\n"))
    }

    /// The messages in a Maildir of their own, indexed, and the Porch's checks
    /// over it: one account, `you@you.example.org`, its provider's results trusted.
    fn mailbox(name: &str, messages: &[(String, String)], projects: &[Project], senders: Senders, now: i64) -> (PathBuf, MailIndex, Gate) {
        let root = std::env::temp_dir().join(format!("sioul-projectview-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("cur")).unwrap();
        for (n, (_, raw)) in messages.iter().enumerate() {
            std::fs::write(root.join("cur").join(format!("{n}.U1-{n}.sioul{}2,S", crate::maildir::INFO)), raw).unwrap();
        }
        let index = MailIndex::build(std::slice::from_ref(&root), &root.join("index.tsv"));
        let source = Source { account: Some("home".into()), address: Some("you@you.example.org".into()), folder: root.clone(), trusted_ids: vec![PROVIDER.into()], priority: Priority::Average, shielded: false, words: crate::words::Words::builtin(), spam: None };
        let store = ProjectStore { root: root.clone(), projects: projects.to_vec(), ties: Default::default() };
        let gate = Gate::new(vec![source], Some(store), SenderList::default(), senders, now);
        (root, index, gate)
    }

    #[test]
    fn a_project_on_one_line_of_time() {
        let now = 1_791_000_000;
        let lumen = Project {
            id: "lumen".into(),
            title: "Studio Lumen".into(),
            kind: Some("project".into()),
            rate: Some(60.0),
            routes: vec![Route { from_domains: vec!["lumen.example.net".into()], ..Route::default() }],
            ..Project::default()
        };
        let done = Task { uid: "d".into(), title: "Mock-ups".into(), status: Status::Completed, completed: Some(now - 86_400), projects: vec!["lumen".into()], ..Task::default() };
        let coming = Task { uid: "c".into(), title: "Deliver the site".into(), due: "2030-01-15".into(), projects: vec!["lumen".into()], ..Task::default() };
        let other = Task { uid: "o".into(), title: "Housing".into(), projects: vec!["housing".into()], ..Task::default() };
        let messages = [
            message("q@lumen.example.net", "Jane <jane@lumen.example.net>", "dkim=pass header.d=lumen.example.net; dmarc=pass header.from=lumen.example.net", "Quote", now - 3 * 86_400, &[]),
            message("x@else.example.org", "x@else.example.org", "", "Other", now - 86_400, &[]),
            // Your answer to the quote, from your own address: in its conversation, so in the project.
            message("a@you.example.org", "you@you.example.org", "", "Re: Quote", now - 2 * 86_400 - 3600, &["q@lumen.example.net"]),
        ];
        let (root, mail, gate) = mailbox("line", &messages, std::slice::from_ref(&lumen), Senders::default(), now);
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
        let page = view(&loaded, &lumen, &entries, &[], 40.0, now, &tr, &gate);
        let line: Vec<(&str, &str, bool)> = page.moments.iter().map(|m| (m.kind.as_str(), m.title.as_str(), m.coming)).collect();
        assert_eq!(line, vec![("task", "Deliver the site", true), ("task", "Mock-ups", false), ("time", "2 h 30 noted", false), ("mail", "Re: Quote", false), ("mail", "Quote", false)]);
        assert_eq!(page.moments.last().map(|m| m.detail.as_str()), Some("From Jane"));
        assert_eq!((page.open_tasks, page.done_tasks, page.unbilled.as_str()), (1, 1, "2 h 30"));
        assert_eq!(page.unbilled_amount, tr.money(Money(15000)));
        let listed = rows(&loaded, &entries, &[Project { id: "housing".into(), title: "Housing".into(), ..Project::default() }, lumen]);
        assert_eq!(listed[0].id, "lumen", "projects first");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A project's page takes mail as the Porch does: what it sets aside
    /// (forged, a blocked sender, a borrowed name) never shows, by a route, a
    /// conversation or a tie Sioul made; what nothing authenticates comes by
    /// no route on its sender; genuine mail, your own and what you tied by
    /// hand still come.
    #[test]
    fn mail_on_a_project_page_passes_the_porchs_checks() {
        let now = 1_791_000_000;
        let day = |n: i64| now - n * 86_400;
        let lumen = Project {
            id: "lumen".into(),
            title: "Studio Lumen".into(),
            routes: vec![Route { from_domains: vec!["lumen.example.net".into()], ..Route::default() }, Route { subject_contains: vec!["Lumen site".into()], ..Route::default() }],
            ..Project::default()
        };
        let genuine = "spf=pass smtp.mailfrom=lumen.example.net; dkim=pass header.d=lumen.example.net; dmarc=pass header.from=lumen.example.net";
        // The client's domain in From, DMARC failing, no DKIM signature at all.
        let forged = "spf=fail smtp.mailfrom=attacker.example; dkim=none; dmarc=fail header.from=lumen.example.net";
        let failed = "spf=fail smtp.mailfrom=lumen.example.net; dkim=fail header.d=lumen.example.net";
        let messages = [
            message("quote@lumen.example.net", "Jane <jane@lumen.example.net>", genuine, "Quote", day(9), &[]),
            message("forged@lumen.example.net", "Jane <jane@lumen.example.net>", forged, "New bank details", day(8), &[]),
            message("unproven@lumen.example.net", "Jane <jane@lumen.example.net>", failed, "Invoice", day(7), &[]),
            message("subject@lumen.example.net", "Jane <jane@lumen.example.net>", failed, "About the Lumen site", day(6), &[]),
            message("borrowed@lumen.example.net", "PayPal <billing@lumen.example.net>", genuine, "Your account", day(5), &[]),
            message("blocked@lumen.example.net", "Pest <pest@lumen.example.net>", genuine, "Hi", day(5), &[]),
            // A forged answer, citing the quote and an unrelated conversation: neither shown nor joining them.
            message("reply@lumen.example.net", "Jane <jane@lumen.example.net>", forged, "Re: Quote", day(4), &["quote@lumen.example.net", "trip@else.example.org"]),
            message("trip@else.example.org", "Paul <paul@else.example.org>", "dkim=pass header.d=else.example.org", "The trip", day(4), &[]),
            message("mine@you.example.org", "you@you.example.org", "", "Re: Quote", day(3), &["quote@lumen.example.net"]),
            message("tied@lumen.example.net", "Jane <jane@lumen.example.net>", forged, "Tied when it came", day(2), &[]),
            message("hand@lumen.example.net", "Jane <jane@lumen.example.net>", forged, "Tied by hand", day(1), &[]),
        ];
        let senders = Senders { blocked: SenderList::parse("pest@lumen.example.net"), ..Senders::default() };
        let (root, mail, gate) = mailbox("checks", &messages, std::slice::from_ref(&lumen), senders, now);
        let tie = |id: &str, how: &str| Edge { from: links::mail_uri(id), to: links::project_uri("lumen"), how: how.into() };
        let local = LocalLinks { links: vec![tie("tied@lumen.example.net", "project"), tie("hand@lumen.example.net", "link")] };
        let loaded = Loaded { mail, local, ..Loaded::default() };
        let page = view(&loaded, &lumen, &[], &[], 40.0, now, &Translator::new("en"), &gate);
        let shown: Vec<(&str, &str)> = page.moments.iter().map(|m| (m.title.as_str(), m.detail.as_str())).collect();
        assert_eq!(
            shown,
            vec![
                ("Tied by hand", "From Jane, whose address could not be verified"),
                ("Re: Quote", "From you@you.example.org"),
                ("About the Lumen site", "From Jane, whose address could not be verified"),
                ("Quote", "From Jane"),
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
