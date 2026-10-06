// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Reminders before dates (docs/reminders.md): remembering "on the 30th" is
//! what fails most, in ADHD and in autism alike, and reminders work where
//! memory is the bottleneck (Landsiedel et al. 2017; Altgassen et al. 2014;
//! Jamieson et al. 2014). One quiet notification per date, never repeated:
//! - an event, at the end of the working day before, and at the alarms it carries;
//! - a date asked, a few working days before, when work starts;
//! - a wait after a step done (an answer due), once it is over;
//! - a payment planned, a few working days before;
//! - a paper to renew (a passport, an identity card, the health cover), when renewing starts;
//! - a contract that renews each year, two weeks before the last day to stop it;
//! - from the bank's movements: a payment that did not pass, once; the day the
//!   account would not hold a payment, five working days before.
//!
//! What is created after its reminder's time is not reminded: you just saw it.
//! A reminder still makes sense until what it is about begins; a computer
//! asleep at the time reminds on waking, if it is not too late. Work waits
//! while work rests; your own things, events and payments do not. While you
//! sleep nothing is told: every reminder waits for waking (docs/health.md,
//! "Do not disturb"), and comes then if it still makes sense. Payments
//! that leave by themselves (presets) are not reminded: the money watch tells
//! when the account will not hold them, which a reminder alone cannot
//! (Medina 2021).

use crate::agenda::Occurrence;
use crate::budget::Ledger;
use crate::config::{Config, TimeOff};
use crate::window::AdminWindow;
use crate::i18n::Translator;
use crate::tasks::{Status, Task};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Zoned};
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

/// What a reminder is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// An event, the working day before.
    Event,
    /// An alarm the event carries.
    Alarm,
    /// A date asked.
    Asked,
    /// A wait over.
    Wait,
    /// A payment planned.
    Payment,
    /// A paper to renew.
    Paper,
    /// A contract renewing.
    Contract,
    /// The money watch: a payment missed, a balance short.
    Money,
}

/// One reminder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reminder {
    /// Unique to the thing and its date: told once.
    pub key: String,
    pub kind: Kind,
    /// When it is due, Unix seconds.
    pub at: i64,
    /// Until when it still makes sense: the event begins, the day asked ends.
    pub until: i64,
    pub title: String,
    pub body: String,
    /// What it is about: "sioul:task/<UID>", an event's file.
    pub target: String,
    /// Work: it waits while work rests.
    pub work: bool,
}

/// What waits now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    Nothing,
    /// Work rests: work's reminders wait for it.
    Work,
    /// You sleep: nothing disturbs, every reminder waits for waking.
    Everything,
}

impl Reminder {
    /// Whether to tell it now: its time has come, it is not too late, and nothing holds it.
    pub fn ready(&self, now: i64, wait: Wait) -> bool {
        let held = match wait {
            Wait::Nothing => false,
            Wait::Work => self.work,
            Wait::Everything => true,
        };
        self.at <= now && now < self.until && !held
    }
}

/// The days and hours work happens: the working hours, else Monday to Friday 9:00–17:00.
fn hours(config: &Config) -> Vec<AdminWindow> {
    let hours = config.working_hours();
    if hours.is_empty() { crate::window::default_office_hours() } else { hours }
}

fn working(hours: &[AdminWindow], time_off: &[TimeOff], date: Date) -> bool {
    crate::window::open_day(hours, date) && crate::quiet::time_off_on(time_off, date).is_none()
}

/// When work starts, `days` working days before `date`.
fn working_days_before(hours: &[AdminWindow], time_off: &[TimeOff], date: Date, days: u32, zone: &TimeZone) -> Option<Zoned> {
    let mut day = date;
    let mut left = days;
    for _ in 0..90 {
        day = day.yesterday().ok()?;
        if working(hours, time_off, day) {
            left -= 1;
            if left == 0 {
                return crate::window::hours_on(hours, day, zone).map(|(opening, _)| opening);
            }
        }
    }
    None
}

/// Half an hour before work ends, on the last working day before `date`.
fn evening_before(hours: &[AdminWindow], time_off: &[TimeOff], date: Date, zone: &TimeZone) -> Option<Zoned> {
    let mut day = date;
    for _ in 0..90 {
        day = day.yesterday().ok()?;
        if working(hours, time_off, day) {
            let (opening, closing) = crate::window::hours_on(hours, day, zone)?;
            let before = closing.checked_sub(Span::new().minutes(30)).ok()?;
            return Some(if before > opening { before } else { opening });
        }
    }
    None
}

/// When work next starts, at or after `at`; `at` itself within working hours.
fn work_from(hours: &[AdminWindow], time_off: &[TimeOff], at: &Zoned) -> Option<Zoned> {
    if working(hours, time_off, at.date()) && crate::window::current(hours, at).is_some() {
        return Some(at.clone());
    }
    crate::quiet::next_work(hours, time_off, at)
}

/// "2026-10-09" or "2026-10-09T14:00": its day, and the moment it ends (the end of the day without a time).
fn asked_date(text: &str, zone: &TimeZone) -> Option<(Date, i64)> {
    let day: Date = text.get(..10)?.parse().ok()?;
    let time = text.get(11..16).and_then(|t| t.split_once(':')).and_then(|(h, m)| jiff::civil::Time::new(h.parse().ok()?, m.parse().ok()?, 0, 0).ok());
    let end = match time {
        Some(time) => day.to_datetime(time).to_zoned(zone.clone()).ok()?,
        None => day.tomorrow().ok()?.to_zoned(zone.clone()).ok()?,
    };
    Some((day, end.timestamp().as_second()))
}

fn modified(path: &str) -> i64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64)
}

/// Everything to remind, told or not, from the events of the coming days, the
/// tasks, and the payments planned; `personal` says which tasks are yours (they do not wait for work).
pub fn all(config: &Config, tr: &Translator, now: &Zoned, events: &[Occurrence], tasks: &[Task], ledger: Option<&Ledger>, papers: &[crate::papers::Paper], contracts: &[crate::contracts::Contract], money: Option<&crate::bank::Watch>, personal: impl Fn(&Task) -> bool) -> Vec<Reminder> {
    let zone = now.time_zone().clone();
    let settings = &config.reminders;
    let hours = hours(config);
    let at = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()));
    let mut out = Vec::new();

    for event in events.iter().filter(|e| !e.cancelled) {
        let Some(start) = at(event.start) else { continue };
        let when = if event.all_day { tr.day(start.date()) } else { tr.date(&start, true) };
        let place = |text: String| if event.location.is_empty() { text } else if text.is_empty() { event.location.clone() } else { format!("{text} · {}", event.location) };
        // Its own alarms: set by you or the one who invited you, told at their time.
        for offset in &event.alarms {
            // The alarm's offset is the event file's: too far to be a time, none.
            let Some(due) = event.start.checked_add(*offset).filter(|due| jiff::Timestamp::from_second(*due).is_ok()) else { continue };
            out.push(Reminder {
                key: format!("alarm:{}:{}:{offset}", event.uid, event.start),
                kind: Kind::Alarm,
                at: due,
                until: event.start.max(due) + 15 * 60,
                title: event.summary.clone(),
                body: place(when.clone()),
                target: event.key.clone(),
                work: false,
            });
        }
        // The working day before; not for calendars you only read (holidays, subscriptions).
        if settings.events && !event.read_only
            && let Some(due) = evening_before(&hours, &config.time_off, start.date(), &zone)
            && modified(&event.key) <= due.timestamp().as_second()
        {
            let mut args = crate::i18n::args();
            args.set("when", when.clone());
            args.set("what", event.summary.clone());
            out.push(Reminder {
                key: format!("event:{}:{}", event.uid, event.start),
                kind: Kind::Event,
                at: due.timestamp().as_second(),
                until: event.start,
                title: tr.text("reminder-event", Some(&args)),
                body: event.location.clone(),
                target: event.key.clone(),
                work: false,
            });
        }
    }

    let open = |t: &&Task| matches!(t.status, Status::NeedsAction | Status::InProcess);
    for task in tasks.iter().filter(open) {
        // A date asked, a few working days before.
        if settings.asked_days > 0
            && let Some((day, end)) = asked_date(&task.due, &zone)
            && let Some(due) = working_days_before(&hours, &config.time_off, day, settings.asked_days, &zone)
            && task.created <= due.timestamp().as_second()
        {
            let mut args = crate::i18n::args();
            args.set("date", tr.day(day));
            out.push(Reminder {
                key: format!("asked:{}:{}", task.uid, task.due),
                kind: Kind::Asked,
                at: due.timestamp().as_second(),
                until: end,
                title: tr.text("reminder-asked", Some(&args)),
                body: task.title.clone(),
                target: format!("sioul:task/{}", task.uid),
                work: !personal(task),
            });
        }
        // A wait after a step done: once it is over, when work is there.
        if settings.waits {
            for relation in task.relations.iter().filter(|r| r.kind.eq_ignore_ascii_case("FINISHTOSTART") && r.gap > 0) {
                let Some(before) = tasks.iter().find(|t| t.uid == relation.uid) else { continue };
                // The gap is the task file's: one too long to be a time waits for nothing.
                let Some(over) = before.completed.and_then(|done| done.checked_add(relation.gap.checked_mul(60)?)).and_then(at) else { continue };
                let Some(due) = work_from(&hours, &config.time_off, &over) else { continue };
                let mut args = crate::i18n::args();
                args.set("before", before.title.clone());
                out.push(Reminder {
                    key: format!("wait:{}:{}", task.uid, over.timestamp().as_second()),
                    kind: Kind::Wait,
                    at: due.timestamp().as_second(),
                    until: over.timestamp().as_second() + 7 * 86_400,
                    title: task.title.clone(),
                    body: tr.text("reminder-wait", Some(&args)),
                    target: format!("sioul:task/{}", task.uid),
                    work: !personal(task),
                });
            }
        }
    }

    // Payments planned (a bill to pay, a tax), a few working days before.
    if settings.payment_days > 0
        && let Some(ledger) = ledger
    {
        let today = now.date();
        let horizon = today.checked_add(Span::new().days(30)).unwrap_or(today);
        for line in ledger.lines.iter().filter(|l| l.planned && l.amount.cents() < 0 && l.date >= today && l.date <= horizon) {
            let Some(due) = working_days_before(&hours, &config.time_off, line.date, settings.payment_days, &zone) else { continue };
            let Some(end) = line.date.tomorrow().ok().and_then(|d| d.to_zoned(zone.clone()).ok()) else { continue };
            let mut args = crate::i18n::args();
            args.set("date", tr.day(line.date));
            // The balance read first: a reminder that ignores it can push an account into overdraft (Medina 2021).
            let body = match money.and_then(|w| w.balance_on(line.date)) {
                Some(after) if after.is_negative() => {
                    args.set("short", tr.money(crate::money::Money(-after.cents())));
                    tr.text("reminder-payment-short", Some(&args))
                }
                Some(_) => tr.text("reminder-payment-held", Some(&args)),
                None => tr.text("reminder-payment", Some(&args)),
            };
            out.push(Reminder {
                key: format!("payment:{}:{}:{}", line.budget, line.label, line.date),
                kind: Kind::Payment,
                at: due.timestamp().as_second(),
                until: end.timestamp().as_second(),
                title: format!("{} · {}", line.label, tr.money(crate::money::Money(-line.amount.cents()))),
                body,
                target: format!("sioul:budget/{}", line.budget),
                work: false,
            });
        }
    }
    // Papers to renew, when renewing starts; not once a renewal task is made (it has its own date).
    for paper in papers.iter().filter(|p| p.renewal.is_empty()) {
        let (Some(from), Some(until)) = (paper.renew_from(), paper.until) else { continue };
        if paper.added.is_some_and(|added| added > from) {
            continue;
        }
        let Some(due) = from.to_zoned(zone.clone()).ok().and_then(|z| work_from(&hours, &config.time_off, &z)) else { continue };
        let Some(end) = until.tomorrow().ok().and_then(|d| d.to_zoned(zone.clone()).ok()) else { continue };
        let mut args = crate::i18n::args();
        args.set("title", paper.title.clone());
        args.set("date", tr.day_in(until, now.date()));
        out.push(Reminder {
            key: format!("paper:{}:{until}", paper.id),
            kind: Kind::Paper,
            at: due.timestamp().as_second(),
            until: end.timestamp().as_second(),
            title: tr.text("paper-reminder", Some(&args)),
            body: tr.text(&format!("paper-renew-{}", paper.kind.family()), None),
            target: format!("sioul:paper/{}", paper.id),
            work: false,
        });
    }
    // Contracts renewing each year: two weeks before the last day a notice can leave.
    for contract in contracts.iter().filter(|c| c.is_open() && c.every == "year") {
        let (Some(renewal), Some(by)) = (contract.next_renewal(now.date()), contract.cancel_by(now.date())) else { continue };
        let Some(from) = by.checked_sub(Span::new().days(14)).ok().and_then(|d| d.to_zoned(zone.clone()).ok()) else { continue };
        let Some(due) = work_from(&hours, &config.time_off, &from) else { continue };
        let Some(end) = by.tomorrow().ok().and_then(|d| d.to_zoned(zone.clone()).ok()) else { continue };
        let mut args = crate::i18n::args();
        args.set("title", contract.title.clone());
        args.set("date", tr.day_in(renewal, now.date()));
        args.set("by", tr.day_in(by, now.date()));
        out.push(Reminder {
            key: format!("contract:{}:{renewal}", contract.id),
            kind: Kind::Contract,
            at: due.timestamp().as_second(),
            until: end.timestamp().as_second(),
            title: tr.text("contract-reminder", Some(&args)),
            body: tr.text(if contract.notice_days > 0 { "contract-reminder-notice" } else { "contract-reminder-free" }, Some(&args)),
            target: format!("sioul:contract/{}", contract.id),
            work: false,
        });
    }
    // The money watch: once each, calmly.
    for finding in money.map(|w| w.findings.as_slice()).unwrap_or_default() {
        let mut args = crate::i18n::args();
        let (key, at, until, title, body) = match finding {
            crate::bank::Finding::Missed { label, amount, date } => {
                args.set("label", label.clone());
                args.set("amount", tr.money(amount.abs()));
                args.set("date", tr.day_in(*date, now.date()));
                let Some(from) = date.checked_add(Span::new().days(7)).ok().and_then(|d| d.to_zoned(zone.clone()).ok()) else { continue };
                let Some(due) = work_from(&hours, &config.time_off, &from) else { continue };
                (format!("missed:{label}:{date}"), due, from.timestamp().as_second() + 30 * 86_400, tr.text(if amount.is_negative() { "money-missed-out" } else { "money-missed-in" }, Some(&args)), tr.text("money-missed-why", Some(&args)))
            }
            crate::bank::Finding::Short { label, amount, date, short, reserve } => {
                args.set("label", label.clone());
                args.set("amount", tr.money(amount.abs()));
                args.set("date", tr.day_in(*date, now.date()));
                args.set("short", tr.money(*short));
                let body = match reserve {
                    Some(reserve) => {
                        args.set("reserve", reserve.clone());
                        tr.text("money-short-reserve", Some(&args))
                    }
                    None => tr.text("money-short-ask", Some(&args)),
                };
                let Some(due) = working_days_before(&hours, &config.time_off, *date, 5, &zone) else { continue };
                let Some(end) = date.tomorrow().ok().and_then(|d| d.to_zoned(zone.clone()).ok()) else { continue };
                (format!("short:{label}:{date}"), due, end.timestamp().as_second(), tr.text("money-short", Some(&args)), body)
            }
            crate::bank::Finding::Changed { .. } => continue,
        };
        out.push(Reminder { key, kind: Kind::Money, at: at.timestamp().as_second(), until, title, body, target: "sioul:budget/".into(), work: false });
    }
    out.sort_by(|a, b| (a.at, &a.key).cmp(&(b.at, &b.key)));
    out
}

/// Everything to remind, read from the files now: the events of the coming
/// days, the tasks, the payments planned in the case store; and what waits now.
pub fn gather(config: &Config, tr: &Translator, now: &Zoned) -> (Vec<Reminder>, Wait) {
    let stamp = now.timestamp().as_second();
    let events = crate::agenda::occurrences(stamp - 86_400, stamp + 9 * 86_400);
    let tasks = crate::tasks::all(now.time_zone());
    let store = config.case_store_path();
    let ledger = store.as_deref().and_then(|root| Ledger::load_with_bank(root).ok());
    let cases = store.as_deref().and_then(|root| crate::cases::CaseStore::load(root).ok()).map(|s| s.cases).unwrap_or_default();
    let overrides = crate::quiet::Overrides::load(&crate::quiet::Overrides::default_path());
    // Meals and sleep as Health keeps them: while you sleep, nothing is told.
    let blocks = crate::quiet::Blocks::read(now, &events);
    let situation = crate::quiet::Situation::now(config, &overrides, &blocks, now, tr, &cases);
    let papers = store.as_deref().and_then(|root| crate::papers::Wallet::load(root).ok()).map(|w| w.papers).unwrap_or_default();
    let contracts = store.as_deref().and_then(|root| crate::contracts::Contracts::load(root).ok()).map(|c| c.list).unwrap_or_default();
    let money = store.as_deref().and_then(|root| crate::bank::Bank::load(root).ok()).filter(|b| !b.movements.is_empty() || !b.accounts.is_empty()).zip(ledger.as_ref()).map(|(bank, ledger)| crate::bank::watch(&bank, ledger, now.date()));
    let reminders = all(config, tr, now, &events, &tasks, ledger.as_ref(), &papers, &contracts, money.as_ref(), |t| situation.quiet_tasks.keeps(t));
    let wait = if situation.mode.sleeps() {
        Wait::Everything
    } else if situation.mode.quiet {
        Wait::Work
    } else {
        Wait::Nothing
    };
    (reminders, wait)
}

/// Where reminders told are marked, one small file each.
pub fn told_dir() -> PathBuf {
    crate::config::state_dir().join("reminded")
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// Marks a reminder told: true for whoever marks it first, the window or
/// `sioul remind`, so it is told once whichever runs.
pub fn claim(dir: &Path, key: &str) -> bool {
    let _ = std::fs::create_dir_all(dir);
    match std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(format!("{:016x}", fnv(key)))) {
        Ok(mut file) => {
            let _ = file.write_all(key.as_bytes());
            true
        }
        Err(_) => false,
    }
}

/// Whether a reminder was told already.
pub fn told(dir: &Path, key: &str) -> bool {
    dir.join(format!("{:016x}", fnv(key))).exists()
}

/// Marks older than two months forgotten: their dates are past.
pub fn forget_old(dir: &Path) {
    let limit = std::time::SystemTime::now() - std::time::Duration::from_secs(60 * 86_400);
    for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok) {
        if entry.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < limit) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// What to tell now, each marked told: those ready, not told yet.
pub fn to_tell(reminders: Vec<Reminder>, dir: &Path, now: i64, wait: Wait) -> Vec<Reminder> {
    reminders.into_iter().filter(|r| r.ready(now, wait) && claim(dir, &r.key)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn at(text: &str) -> Zoned {
        text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone()).unwrap()
    }

    fn config() -> Config {
        toml::from_str(
            "[[window]]\nday = \"monday\"\nstart = \"09:00\"\nend = \"17:00\"\n[[window]]\nday = \"tuesday\"\nstart = \"09:00\"\nend = \"17:00\"\n\
             [[window]]\nday = \"wednesday\"\nstart = \"09:00\"\nend = \"17:00\"\n[[window]]\nday = \"thursday\"\nstart = \"09:00\"\nend = \"17:00\"\n\
             [[window]]\nday = \"friday\"\nstart = \"10:00\"\nend = \"16:00\"\n",
        )
        .unwrap()
    }

    fn event(uid: &str, start: &str, alarms: Vec<i64>) -> Occurrence {
        let start = at(start).timestamp().as_second();
        Occurrence {
            key: "/nonexistent/event.ics".into(),
            uid: uid.into(),
            summary: "Dentist".into(),
            location: "12 rue Example".into(),
            notes: String::new(),
            start,
            end: start + 3600,
            all_day: false,
            recurring: false,
            calendar: "Personal".into(),
            color: None,
            cancelled: false,
            tentative: false,
            organizer: String::new(),
            attendees: vec![],
            read_only: false,
            alarms,
            ..Occurrence::default()
        }
    }

    fn task(uid: &str, due: &str) -> Task {
        Task { uid: uid.into(), title: format!("Task {uid}"), due: due.into(), created: at("2026-09-01T10:00").timestamp().as_second(), ..Task::default() }
    }

    #[test]
    fn once_each_at_its_time() {
        let tr = Translator::new("en");
        let config = config();
        let now = at("2026-10-05T08:00"); // a Monday
        // Monday 12 October at 9:00: told on Friday 9 October at 15:30 (Friday ends at 16:00), and at its alarm.
        let events = vec![event("dentist", "2026-10-12T09:00", vec![-3600])];
        // Asked for Thursday 8 October: two working days before, Tuesday at 9:00.
        let mut tasks = vec![task("form", "2026-10-08")];
        // A letter sent on Friday 2 October at 11:00, its answer waited 2 days: Sunday 11:00 → Monday 9:00.
        let mut letter = task("letter", "");
        letter.status = Status::Completed;
        letter.completed = Some(at("2026-10-02T11:00").timestamp().as_second());
        let mut follow = task("follow-up", "");
        follow.relations = vec![crate::tasks::Relation { kind: "FINISHTOSTART".into(), uid: "letter".into(), gap: 2 * 1440 }];
        tasks.extend([letter, follow]);
        let ledger: Ledger = toml::from_str("[[line]]\nbudget = \"home\"\ndate = 2026-10-15\namount = -120.50\nlabel = \"Water bill\"\nplanned = true\n").unwrap();
        // A passport ending on 30 December: renewing starts on 1 October, a Thursday, told when work starts.
        let passport = crate::papers::Paper { id: "passport".into(), kind: crate::papers::Kind::Passport, title: "Passport".into(), until: Some("2026-12-30".parse().unwrap()), ..Default::default() };
        // A phone plan renewing each year on 20 November, 10 days' notice: by 10 November, told from 27 October.
        let phone = crate::contracts::Contract { id: "phone".into(), title: "Phone".into(), renews: Some("2025-11-20".parse().unwrap()), every: "year".into(), notice_days: 10, ..Default::default() };
        // The bank says the account will be short on 15 October, when the water bill leaves; the savings cover it.
        let money = crate::bank::Watch {
            findings: vec![crate::bank::Finding::Short { label: "Water bill".into(), amount: crate::money::Money(-12050), date: "2026-10-15".parse().unwrap(), short: crate::money::Money(4000), reserve: Some("Savings".into()) }],
            forecast: vec![("2026-10-15".parse().unwrap(), crate::money::Money(-4000))],
            ..Default::default()
        };
        let all = all(&config, &tr, &now, &events, &tasks, Some(&ledger), &[passport], &[phone], Some(&money), |_| false);
        let find = |prefix: &str| all.iter().find(|r| r.key.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix}: {all:#?}"));
        assert_eq!(find("event:").at, at("2026-10-09T15:30").timestamp().as_second());
        assert_eq!(find("event:").title, "Mon 12 Oct 09:00 · Dentist");
        assert_eq!(find("alarm:").at, at("2026-10-12T08:00").timestamp().as_second());
        assert_eq!(find("asked:").at, at("2026-10-06T09:00").timestamp().as_second());
        assert_eq!(find("asked:").title, "Asked for Thursday 8 October");
        assert_eq!(find("wait:").at, at("2026-10-05T09:00").timestamp().as_second());
        assert_eq!(find("payment:").at, at("2026-10-13T09:00").timestamp().as_second());
        assert!(find("payment:").title.contains("Water bill") && find("payment:").title.contains("120.50"), "{}", find("payment:").title);
        assert_eq!(find("paper:").at, at("2026-10-01T09:00").timestamp().as_second());
        assert_eq!(find("paper:").title, "Passport: valid until Wednesday 30 December");
        assert_eq!(find("contract:").at, at("2026-10-27T09:00").timestamp().as_second());
        assert_eq!(find("contract:").body, "To stop it, the notice must leave by Tuesday 10 November.");
        assert_eq!(find("short:").at, at("2026-10-08T09:00").timestamp().as_second(), "five working days before");
        assert_eq!(find("short:").title, "The account may not hold Water bill on Thursday 15 October");
        assert!(find("short:").body.contains("Savings covers it"), "{}", find("short:").body);
        assert!(find("payment:").body.contains("short then"), "the payment reminder reads the balance: {}", find("payment:").body);

        // Told once, by whoever comes first; work waits while work rests.
        let dir = std::env::temp_dir().join(format!("sioul-reminded-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let tuesday = at("2026-10-06T09:05").timestamp().as_second();
        // Asleep: nothing told, nothing marked; told at waking.
        assert!(to_tell(all.clone(), &dir, tuesday, Wait::Everything).is_empty());
        let mut told = to_tell(all.clone(), &dir, tuesday, Wait::Nothing);
        told.retain(|r| r.kind == Kind::Asked);
        assert_eq!(told.len(), 1);
        assert!(to_tell(all.clone(), &dir, tuesday + 60, Wait::Nothing).iter().all(|r| r.kind != Kind::Asked), "never twice");
        let wait = find("wait:").clone();
        assert!(!wait.ready(tuesday, Wait::Work) && wait.ready(tuesday, Wait::Nothing));
        assert!(!find("payment:").clone().ready(find("payment:").at, Wait::Everything) && find("payment:").ready(find("payment:").at, Wait::Work), "yours wait only for waking");
        // Too late: the day asked is over.
        assert!(!find("asked:").ready(at("2026-10-09T08:00").timestamp().as_second(), Wait::Nothing));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn not_for_what_was_just_made() {
        let tr = Translator::new("fr");
        let config = config();
        let now = at("2026-10-06T10:00");
        // Asked for Wednesday, made on Tuesday at 10:00, after its reminder (Monday 9:00): not told.
        let mut late = task("late", "2026-10-07T12:00");
        late.created = now.timestamp().as_second();
        // Done already: nothing.
        let mut done = task("done", "2026-10-09");
        done.status = Status::Completed;
        let all = all(&config, &tr, &now, &[], &[late, done], None, &[], &[], None, |_| true);
        assert!(all.is_empty(), "{all:#?}");
        // Time off moves the working days.
        let mut off = self::config();
        off.time_off = vec![TimeOff { from: "2026-10-12".parse().unwrap(), until: "2026-10-13".parse().unwrap(), label: String::new() }];
        let all = super::all(&off, &tr, &now, &[], &[task("after", "2026-10-14")], None, &[], &[], None, |_| true);
        assert_eq!(all[0].at, at("2026-10-08T09:00").timestamp().as_second(), "Thursday and Friday before the days off");
        assert_eq!(all[0].title, "Demandé pour mercredi 14 octobre");
        // A wait or an alarm too long to be a time, from someone else's file: nothing, and no crash.
        let mut sent = task("sent", "");
        sent.status = Status::Completed;
        sent.completed = Some(now.timestamp().as_second());
        let mut answer = task("answer", "");
        answer.relations = vec![crate::tasks::Relation { kind: "FINISHTOSTART".into(), uid: "sent".into(), gap: i64::MAX / 7 }];
        let odd = event("odd", "2026-10-12T09:00", vec![i64::MAX - 10]);
        assert!(super::all(&config, &tr, &now, &[odd], &[sent, answer], None, &[], &[], None, |_| true).iter().all(|r| r.kind != Kind::Wait && r.kind != Kind::Alarm));
    }
}
