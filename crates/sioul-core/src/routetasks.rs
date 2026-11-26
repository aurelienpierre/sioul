// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Routes that make a task of each message (docs/notes-folder.md, "Routes
//! that make tasks"; the design of 23 November 2026, part 2).
//!
//! A project's route may point at one of its models of task (`makes`, a
//! `[[project.model]]`): each message that route takes, as the Porch admits
//! it, becomes one task of that model, once, on every device:
//!
//! - **which mail**: the route as it is, read as the Porch reads it
//!   (`porch::Admission::for_routes`): nothing it sets aside, and by a route
//!   on the sender only mail whose sender is authenticated;
//! - **the task**: its title the subject, the route's words and the brackets
//!   around them taken off, the model's prefix in front (`title`); in the
//!   project (REFID), so what it is for is the project's; in the model's
//!   list, else the usual one (`list_for`); its date asked always set, so
//!   many business days after the mail came (`Workdays::date_asked`); its
//!   first estimate and the supplies it will use, the model's; where it came
//!   from kept on it (`ORIGIN`, `mid:<Message-ID>`); nothing of the mail's
//!   text copied;
//! - **once**: its UID made from the project and the message (`uid`), its
//!   stamps from the message's arrival, so that two devices write the same
//!   bytes in the same file, never two tasks; tied to its
//!   message in `links.toml` (`how = "made"`, `MADE`), which travels between
//!   devices: a task tied so is never made again, even once deleted
//!   (`made_before`);
//! - **mail already here** when the route is set makes nothing by itself:
//!   `already_here` finds it, for "Also for the 12 messages already here".

use crate::card::Card;
use crate::config::Config;
use crate::links::{self, Edge, LocalLinks};
use crate::mailindex::{MailIndex, MailRef, bare_id};
use crate::porch::{Admission, Gate};
use crate::projects::{Model, Project, ProjectStore, Route};
use crate::tasks::TaskEdit;
use crate::text::{find_word, fold};
use crate::vacations::DaysOff;
use crate::vdir::Collection;
use crate::window::AdminWindow;
use jiff::civil::Date;
use std::path::{Path, PathBuf};

/// Where a task made of a message came from, kept on it: `mid:<Message-ID>`
/// (RFC 2392), or `sioul:mail/<account>/<digest>` for a message without one
/// (docs/compatibility.md: an X- property, which a phone's account manager
/// keeps as any).
pub const ORIGIN: &str = "X-SIOUL-ORIGIN";

/// How a task made of a message is tied to it in `links.toml`: read as
/// "made from" from the task, "made into" from the message (`ties::How::Source`).
pub const MADE: &str = "made";

/// The business days a new model's form is filled with.
pub use crate::projects::BUSINESS_DAYS;

/// A model that does not say its business days (a file written by hand):
/// its tasks are asked for on the first business day after the mail.
pub const UNSAID_DAYS: u32 = 1;

/// Days looked through for a business day before giving up (ten years: a
/// week with no working day at all, every day off).
const SEARCHED: u32 = 3660;

/// The days the person works: the weekdays with working hours in their week
/// (the hours of kind work; none at all, Monday to Friday), less every day
/// off (`vacations::DaysOff::on`: the periods typed, and the whole days of
/// the subscribed calendars of holidays that count, public holidays among
/// them once their calendar is subscribed).
#[derive(Debug, Clone)]
pub struct Workdays {
    /// Monday first.
    weekdays: [bool; 7],
    off: DaysOff,
}

impl Workdays {
    /// As the configuration and this device's calendars of holidays say them now.
    pub fn of(config: &Config) -> Workdays {
        Workdays::new(&config.week_hours(), DaysOff::of(config))
    }

    /// From a week's hours and the days off.
    pub fn new(week: &[AdminWindow], off: DaysOff) -> Workdays {
        let mut weekdays = [false; 7];
        for window in week.iter().filter(|w| w.kind() == "work") {
            if let Some(day) = window.weekday() {
                weekdays[usize::try_from(day.to_monday_zero_offset()).unwrap_or(0)] = true;
            }
        }
        if !weekdays.contains(&true) {
            weekdays = [true, true, true, true, true, false, false];
        }
        Workdays { weekdays, off }
    }

    /// Whether `date` is a business day.
    pub fn is_business(&self, date: Date) -> bool {
        self.weekdays[usize::try_from(date.weekday().to_monday_zero_offset()).unwrap_or(0)] && self.off.on(date).is_none()
    }

    /// The date asked of a message that came on `arrived`, `n` business days
    /// after it, counted from the day after: 0 is the first business day on
    /// or after the day it came, 1 the first after it. With no business day
    /// for years (every day off), the day `n` days after it.
    pub fn date_asked(&self, arrived: Date, n: u32) -> Date {
        let mut day = arrived;
        let mut counted = 0;
        if n == 0 && self.is_business(day) {
            return day;
        }
        for _ in 0..SEARCHED {
            let Ok(next) = day.tomorrow() else { break };
            day = next;
            if self.is_business(day) {
                counted += 1;
                if counted >= n.max(1) {
                    return day;
                }
            }
        }
        arrived.checked_add(jiff::Span::new().days(i64::from(n))).unwrap_or(arrived)
    }
}

/// A task's title from its message's subject: the route's words of the
/// subject taken off, with the brackets around them when they hold nothing
/// else (`[New sale] Gift card 50 €` gives "Gift card 50 €"), what is left
/// at either end of a separator (":", "-", "|"…) too; the model's prefix in
/// front ("Send:" gives "Send: Gift card 50 €"). Nothing left: the subject
/// as it was; no subject: the model's name.
pub fn title(subject: &str, words: &[String], prefix: &str, name: &str) -> String {
    let chars: Vec<char> = subject.chars().collect();
    let folded = fold(subject);
    let mut gone = vec![false; chars.len()];
    for word in words.iter().map(|w| w.trim()).filter(|w| !w.is_empty()) {
        let length = fold(word).len();
        let mut from = 0;
        while let Some(at) = find_word(&folded, word, from) {
            let end = at + length;
            gone[at..end].iter_mut().for_each(|g| *g = true);
            // The brackets around it, when they hold it alone.
            let before = (0..at).rev().find(|&i| !chars[i].is_whitespace() && !gone[i]);
            let after = (end..chars.len()).find(|&i| !chars[i].is_whitespace() && !gone[i]);
            if let (Some(open), Some(close)) = (before, after)
                && closes(chars[open], chars[close])
            {
                gone[open..=close].iter_mut().for_each(|g| *g = true);
            }
            from = end;
        }
    }
    let kept: String = chars.iter().zip(&gone).map(|(c, g)| if *g { ' ' } else { *c }).collect();
    let separators: &[char] = &[':', '-', '–', '—', '|', '·', '/', ',', ';', '•'];
    let mut joined = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    // Brackets that held more than the words: no space left inside them, none left empty.
    for (from, to) in [("[ ", "["), (" ]", "]"), ("( ", "("), (" )", ")"), ("[]", ""), ("()", "")] {
        joined = joined.replace(from, to);
    }
    let joined = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut left = joined.trim_matches(|c: char| c.is_whitespace() || separators.contains(&c)).to_string();
    if left.is_empty() {
        left = subject.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if left.is_empty() {
        left = name.trim().to_string();
    }
    let prefix = prefix.trim();
    if prefix.is_empty() { left } else { format!("{prefix} {left}").trim().to_string() }
}

/// Whether `open` and `close` are a pair of brackets.
fn closes(open: char, close: char) -> bool {
    matches!((open, close), ('[', ']') | ('(', ')') | ('{', '}') | ('<', '>') | ('«', '»') | ('【', '】') | ('「', '」') | ('〔', '〕'))
}

/// The words between brackets in a subject (`[New sale] Gift card` gives
/// "New sale"): what "Make a task of mail like this…" fills the route's words
/// of the subject with.
pub fn bracketed(subject: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = subject;
    while let Some(start) = rest.find(['[', '【']) {
        let inner = &rest[start + rest[start..].chars().next().map_or(1, char::len_utf8)..];
        let Some(end) = inner.find([']', '】']) else { break };
        let word = inner[..end].trim();
        if !word.is_empty() && !out.iter().any(|w| w.eq_ignore_ascii_case(word)) {
            out.push(word.to_string());
        }
        rest = &inner[end..];
    }
    out
}

/// The UID of the task a project's route makes of a message (its bare
/// Message-ID, else its address without one): the same on every device.
pub fn uid(project: &str, message: &str) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("made\n{project}\n{message}"))
}

/// A message's name for its task's UID and its address: its bare
/// Message-ID and `mid:…`; one without, `sioul:mail/<account>/<digest>` for
/// both (`mailfind::address_of_file`, the same on every device).
pub fn message_of(card: &Card) -> Option<(String, String)> {
    match card.message_id.as_deref().map(bare_id).filter(|id| !id.is_empty()) {
        Some(id) => Some((id.clone(), links::mail_uri(&id))),
        None => {
            let address = crate::mailfind::address_of_file(card.account.as_deref()?, card.path.as_deref()?)?;
            Some((address.clone(), address))
        }
    }
}

/// Whether a task was made of this message for this project already: a tie
/// says so (made here or on another device, done or deleted since), or a
/// task of its UID is known.
pub fn made_before(uid: &str, known: &dyn Fn(&str) -> bool, ties: &LocalLinks) -> bool {
    let me = links::task_uri(uid);
    known(uid) || ties.links.iter().any(|e| e.how == MADE && (e.from == me || e.to == me))
}

/// What makes a task of a message: a project, the route of it that takes
/// the message, and the model that route makes.
#[derive(Debug, Clone, Copy)]
pub struct Taker<'a> {
    pub project: &'a Project,
    pub route: &'a Route,
    pub model: &'a Model,
}

/// The routes that make tasks and take `card` as the Porch admits it
/// (`admission`): one per project, the first of its routes that does.
pub fn takers<'a>(store: &'a ProjectStore, card: &Card, admission: &Admission) -> Vec<Taker<'a>> {
    let Some(seen) = admission.for_routes(card) else { return Vec::new() };
    store
        .projects
        .iter()
        .filter_map(|project| {
            project.routes.iter().filter(|r| !r.makes.is_empty()).find_map(|route| {
                let model = project.model(&route.makes)?;
                route.explain(&seen).map(|_| Taker { project, route, model })
            })
        })
        .collect()
}

/// Whether any project has a route that makes tasks.
pub fn any(store: &ProjectStore) -> bool {
    store.projects.iter().any(|p| p.routes.iter().any(|r| !r.makes.is_empty() && p.model(&r.makes).is_some()))
}

/// The list a model's tasks go into: its own ("account/id") when it is
/// there and can be written, else the usual one (Tasks ⚙, "New tasks go
/// into"), else the first made for tasks (`tasks::default_list`).
pub fn list_for(model: &Model, config: &Config) -> Option<Collection> {
    let lists: Vec<Collection> = crate::tasks::lists().into_iter().filter(|c| !c.read_only).collect();
    let named = |wanted: &str| lists.iter().find(|c| !wanted.is_empty() && format!("{}/{}", c.account, c.id) == wanted).cloned();
    named(model.list.trim()).or_else(|| named(config.tasks.list.as_deref().unwrap_or_default())).or_else(crate::tasks::default_list)
}

/// A task made of a message: its file's text.
pub fn task_text(taker: &Taker, subject: &str, due: Date, uid: &str, origin: &str, zone: &jiff::tz::TimeZone, now: &jiff::Zoned) -> Result<String, String> {
    let edit = TaskEdit {
        title: title(subject, &taker.route.subject_contains, &taker.model.prefix, &taker.model.name),
        due: due.to_string(),
        estimate: taker.model.estimate.unwrap_or(0),
        projects: vec![taker.project.id.clone()],
        supplies: taker.model.supplies.clone(),
        ..TaskEdit::default()
    };
    let text = crate::tasks::new_task(&edit, uid, zone, now)?;
    crate::tasks::add_lines(&text, &[format!("{ORIGIN}:{}", crate::lines::escape(origin))], now)
}

/// A task made of a message, or found made already.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub uid: String,
    pub project: String,
    pub title: String,
    /// The date asked.
    pub due: Date,
    /// Its file, and its list's account and name.
    pub path: PathBuf,
    pub account: String,
    pub list: String,
}

/// What `make` did with one message: the tasks written, and the ties to add
/// (one per task made or found made, so that none is made again).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub made: Vec<Made>,
    pub ties: Vec<Edge>,
    /// What could not be written, said once.
    pub problems: Vec<String>,
}

/// What a device needs to make tasks: the configuration, the person's own
/// addresses, where each model's tasks go, the days worked, the tasks it
/// knows (by UID), the ties, the time zone and now.
pub struct Making<'a> {
    pub config: &'a Config,
    /// The person's own addresses (`porch::own_addresses`): what they send makes no task.
    pub own: &'a [String],
    /// The list a model's tasks go into (`list_for`).
    pub lists: &'a dyn Fn(&Model) -> Option<Collection>,
    pub workdays: &'a Workdays,
    pub known: &'a dyn Fn(&str) -> bool,
    pub ties: &'a LocalLinks,
    pub zone: &'a jiff::tz::TimeZone,
    pub now: &'a jiff::Zoned,
}

/// The tasks the routes of `takers` make of `card`, each once: none made
/// before (`made_before`); each written in its list as `<UID>.ics` unless a
/// file of that name is there already (another device made it), and tied to
/// its message either way.
pub fn make(card: &Card, takers: &[Taker], making: &Making, out: &mut Outcome) {
    let Some((name, address)) = message_of(card) else { return };
    if card.from_address.as_deref().is_some_and(|from| making.own.iter().any(|own| own.eq_ignore_ascii_case(from.trim()))) {
        return;
    }
    // Stamped from when the message came, never this device's clock: two devices write the same bytes.
    let stamp = crate::porch::sent(card).and_then(|at| jiff::Timestamp::from_second(at).ok()).map_or_else(|| making.now.clone(), |t| t.to_zoned(making.zone.clone()));
    let arrived = stamp.date();
    for taker in takers {
        let uid = uid(&taker.project.id, &name);
        if made_before(&uid, making.known, making.ties) || out.ties.iter().any(|e| e.from == links::task_uri(&uid)) {
            continue;
        }
        let Some(list) = (making.lists)(taker.model) else {
            out.problems.push(format!("{}: no task list to write in", taker.project.id));
            continue;
        };
        let due = making.workdays.date_asked(arrived, taker.model.business_days.unwrap_or(UNSAID_DAYS));
        let path = list.dir.join(format!("{uid}.ics"));
        let tie = Edge { from: links::task_uri(&uid), to: address.clone(), how: MADE.to_string() };
        if path.exists() {
            out.ties.push(tie);
            continue;
        }
        let written = task_text(taker, &card.subject, due, &uid, &address, making.zone, &stamp).and_then(|text| crate::vdir::write_item(&path, &text));
        match written {
            Ok(()) => {
                out.ties.push(tie);
                out.made.push(Made {
                    uid: uid.clone(),
                    project: taker.project.id.clone(),
                    title: title(&card.subject, &taker.route.subject_contains, &taker.model.prefix, &taker.model.name),
                    due,
                    path,
                    account: list.account.clone(),
                    list: list.name.clone(),
                });
            }
            Err(e) => out.problems.push(e),
        }
    }
}

/// The tasks made of mail just fetched (its files): each message read once,
/// as the Porch admits it (`gate`); its ties added to `links.toml` under its
/// lock (`ties_path`). Nothing read when no route makes tasks.
pub fn on_arrival(gate: &Gate, store: &ProjectStore, files: &[PathBuf], making: &Making, ties_path: &Path) -> Outcome {
    let mut out = Outcome::default();
    if !any(store) {
        return out;
    }
    for file in files {
        let Some(card) = read(file, making.config) else { continue };
        let Some(admission) = gate.admission(&card) else { continue };
        let takers = takers(store, &card, &admission);
        if !takers.is_empty() {
            make(&card, &takers, making, &mut out);
        }
    }
    if let Err(e) = LocalLinks::add_all(ties_path, &out.ties) {
        out.problems.push(e);
    }
    out
}

/// The messages already here that a project's route takes as the Porch
/// admits them, and that made no task yet (`made_before`): read from the
/// mail index, the route's sender and subject first; a route that needs the
/// text or an attachment, only among the messages tied to the project (as
/// they arrived, or when its routes changed: `porch::Gate::routed`); each
/// then read and judged as it would be on arrival. Every copy of a message
/// counts once, the one that ranks first.
pub fn already_here(gate: &Gate, store: &ProjectStore, project: &str, route: &str, index: &MailIndex, making: &Making) -> Vec<Card> {
    let Some(project) = store.get(project) else { return Vec::new() };
    let Some(route) = project.routes.iter().find(|r| r.id == route && !r.makes.is_empty()) else { return Vec::new() };
    let Some(model) = project.model(&route.makes) else { return Vec::new() };
    let tied = links::mail_projects(making.ties);
    let taker = Taker { project, route, model };
    let header = |mail: &MailRef| {
        let domain = mail.address.rsplit_once('@').map_or("", |(_, d)| d);
        (route.from_domains.is_empty() || route.from_domains.iter().any(|d| domain_matches(domain, d)))
            && (route.from_addresses.is_empty() || route.from_addresses.iter().any(|a| mail.address.eq_ignore_ascii_case(a.trim())))
            && (route.subject_contains.is_empty() || route.subject_contains.iter().any(|w| crate::text::contains_word(&mail.subject, w)))
    };
    let mut out = Vec::new();
    let candidates = index.iter().map(|(id, mail)| (Some(id.as_str()), mail)).chain(index.without_id().map(|(_, mail)| (None, mail)));
    for (id, mail) in candidates {
        if !header(mail) || (route.needs_body() && !id.is_some_and(|id| tied.get(id).is_some_and(|p| p == &project.id))) {
            continue;
        }
        let Some(card) = read(&mail.path, making.config) else { continue };
        let Some((name, _)) = message_of(&card) else { continue };
        if made_before(&uid(&project.id, &name), making.known, making.ties) {
            continue;
        }
        let Some(admission) = gate.admission(&card) else { continue };
        if admission.for_routes(&card).is_some_and(|seen| taker.route.explain(&seen).is_some()) {
            out.push(card);
        }
    }
    out.sort_by_key(|c| c.date.unwrap_or(0));
    out
}

/// A message's file, read, with the account it came through.
fn read(file: &Path, config: &Config) -> Option<Card> {
    let mut card = crate::maildir::read_one(file)?;
    if card.account.is_none() {
        card.account = config.account_of(file).map(|a| a.id.clone());
    }
    Some(card)
}

/// "dgfip.finances.example" is "finances.example"'s; "notfinances.example" is not.
fn domain_matches(domain: &str, pattern: &str) -> bool {
    let (domain, pattern) = (domain.to_ascii_lowercase(), pattern.trim().trim_start_matches('@').to_ascii_lowercase());
    !pattern.is_empty() && (domain == pattern || domain.ends_with(&format!(".{pattern}")))
}

/// The tasks made of the messages already here (`already_here`), once: what
/// "Also for the 12 messages already here" does. Their ties added to
/// `links.toml` under its lock.
pub fn for_already_here(cards: &[Card], store: &ProjectStore, project: &str, route: &str, making: &Making, ties_path: &Path) -> Outcome {
    let mut out = Outcome::default();
    let Some(project) = store.get(project) else { return out };
    let Some(route) = project.routes.iter().find(|r| r.id == route) else { return out };
    let Some(model) = project.model(&route.makes) else { return out };
    let takers = [Taker { project, route, model }];
    for card in cards {
        make(card, &takers, making, &mut out);
    }
    if let Err(e) = LocalLinks::add_all(ties_path, &out.ties) {
        out.problems.push(e);
    }
    out
}

#[cfg(test)]
#[path = "routetasks_tests.rs"]
mod tests;
