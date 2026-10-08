// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Reminders before dates (docs/reminders.md): remembering "on the 30th" is
//! what fails most, in ADHD and in autism alike, and reminders work where
//! memory is the bottleneck (Landsiedel et al. 2017; Altgassen et al. 2014;
//! Jamieson et al. 2014). One quiet notification per date, never repeated:
//! - an event, a quarter of an hour before it (yours to set, and each event's
//!   own: `X-SIOUL-REMIND`), counted before its margin (getting ready,
//!   getting there); at the end of the working day before; and at the alarms
//!   it carries, an alarm and Sioul's reminder within minutes told once;
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
//! "Do not disturb"), and comes then if it still makes sense; but an event's
//! own reminders come when the event falls in that sleep (a choice you
//! made), and in a pause when it falls in the pause (docs/pauses.md). These
//! are the usual values of the matrix of what reaches you (`attention`), which
//! you may change kind by kind and time by time (`Holds`). Payments that leave by
//! themselves (presets) are not reminded: the money watch tells when the
//! account will not hold them, which a reminder alone cannot (Medina 2021).

use crate::agenda::Occurrence;
use crate::attention::{self, Attention, Level};
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

/// Sioul's own property for an event's reminder: absent, as usual; `NONE`,
/// not this one; `15`, so many minutes before (before its margin). Not a
/// VALARM: your other calendar apps would ring it too, and a VALARM cannot
/// say "not this one" (docs/reminders.md).
pub const REMIND: &str = "X-SIOUL-REMIND";

/// The usual reminder before an event, as Settings offers it, in minutes; 0 for none.
pub const LEADS: [u32; 7] = [0, 5, 10, 15, 30, 60, 120];

/// An alarm the event carries and Sioul's reminder this close are told once, at the earlier.
const SAME_MOMENT: i64 = 5 * 60;

/// What an event says of its reminder before it (`REMIND`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remind {
    /// The usual time (Settings ▸ Reminders and notifications).
    Usual,
    /// Not this one.
    Never,
    /// So many minutes before.
    Minutes(u32),
}

impl Remind {
    /// As the property says it ("", "NONE", "15"); "0" is none, a word not
    /// understood is as usual, a time at most a day.
    pub fn read(value: &str) -> Remind {
        let value = value.trim();
        if value.is_empty() {
            return Remind::Usual;
        }
        if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("off") {
            return Remind::Never;
        }
        match value.parse::<u32>() {
            Ok(0) => Remind::Never,
            Ok(minutes) => Remind::Minutes(minutes.min(24 * 60)),
            Err(_) => Remind::Usual,
        }
    }

    /// As the property writes it; none as usual.
    pub fn value(self) -> Option<String> {
        match self {
            Remind::Usual => None,
            Remind::Never => Some("NONE".to_string()),
            Remind::Minutes(minutes) => Some(minutes.to_string()),
        }
    }

    /// Minutes before the event and its margin, `usual` when it says nothing; 0 for none.
    pub fn minutes(self, usual: u32) -> u32 {
        match self {
            Remind::Usual => usual,
            Remind::Never => 0,
            Remind::Minutes(minutes) => minutes,
        }
    }
}

/// A reminder's time before, in words: "15 minutes", "1 hour", "none".
pub fn lead_text(tr: &Translator, minutes: u32) -> String {
    let mut args = crate::i18n::args();
    args.set("minutes", minutes);
    tr.text("remind-lead", Some(&args))
}

/// The time left, in words: "in 15 minutes", "in 1 hour", "in 1 h 30", "now".
fn in_text(tr: &Translator, seconds: i64) -> String {
    let minutes = (seconds.max(0) + 30) / 60;
    let mut args = crate::i18n::args();
    let id = if minutes < 1 {
        "reminder-in-now"
    } else if minutes < 60 {
        args.set("minutes", minutes);
        "reminder-in-minutes"
    } else if minutes % 60 == 0 {
        args.set("hours", minutes / 60);
        "reminder-in-hours"
    } else {
        args.set("hours", minutes / 60);
        args.set("minutes", format!("{:02}", minutes % 60));
        "reminder-in-hours-minutes"
    };
    tr.text(id, Some(&args))
}

/// Whether you declined the event: one of your addresses among its guests, "declined".
fn declined(event: &Occurrence, own: &[String]) -> bool {
    event.attendees.iter().any(|a| a.answer == "declined" && own.iter().any(|o| o.eq_ignore_ascii_case(a.address.trim())))
}

/// What a reminder is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// An event, the working day before.
    Event,
    /// An event, so many minutes before it and its margin.
    Before,
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
    /// What it is about: `sioul:task/<UID>`, an event's file.
    pub target: String,
    /// Work: it waits while work rests.
    pub work: bool,
    /// An event's own reminder (its alarm, Sioul's before it): when the event starts, Unix seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starts: Option<i64>,
}

/// What waits now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    Nothing,
    /// Work rests: work's reminders wait for it.
    Work,
    /// You sleep, from `from` (winding down, a nap) to `until` (waking):
    /// every reminder waits for waking, but an event's own when the event
    /// falls in this sleep: you chose it (an event at 03:00).
    Sleep { from: i64, until: i64 },
    /// The pause, its end unknown (docs/pauses.md): an event's own reminders
    /// come, the event falling in it as far as anyone knows (it has not
    /// begun); everything else waits for your return.
    Paused,
    /// Free time until `until`: an event's own reminders come when the event
    /// falls in it; everything else waits for its end (docs/pauses.md).
    Free { until: i64 },
}

/// What holds reminders now: the time (`Wait`), and what the matrix of what
/// reaches you says then of each kind of reminder (`attention`): an event's
/// alarms, Sioul's reminder before an event, the working day before, the
/// other dates.
#[derive(Debug, Clone, PartialEq)]
pub struct Holds {
    pub wait: Wait,
    pub attention: Attention,
    /// The moment as the matrix reads it, its layers read from the files
    /// (today's slots of time for you, do-not-disturb's switch and focus);
    /// its span the time now, from when to when (Unix seconds), for "if its
    /// event falls then": a sleep's block, Free time, the hours until they change.
    pub now: attention::Now,
    /// When the time now ends, when known: what it holds is asked about again then.
    pub until: Option<i64>,
}

impl Holds {
    /// The usual matrix, at a time: what Sioul does unless you change it.
    pub fn usual(wait: Wait) -> Holds {
        let all = (i64::MIN, i64::MAX);
        let (column, span, until) = match wait {
            Wait::Nothing => (attention::Column::Work, all, None),
            Wait::Work => (attention::Column::Leisure, all, None),
            Wait::Sleep { from, until } => (attention::Column::Sleep, (from, until), Some(until)),
            Wait::Paused => (attention::Column::Pause, all, None),
            Wait::Free { until } => (attention::Column::Free, (i64::MIN, until), Some(until)),
        };
        let mut now = attention::Now::time(column);
        now.span = span;
        Holds { wait, attention: Attention::usual(), now, until }
    }

    /// As the configuration says, at the time `mode` (Health's `blocks` for
    /// sleep's span), today's slots of time for you and do-not-disturb's
    /// switch and focus read from the files.
    pub fn of(config: &Config, mode: &crate::quiet::Mode, blocks: &crate::quiet::Blocks, stamp: i64) -> Holds {
        let wait = wait_of(mode, blocks, stamp);
        let ends = mode.until.as_ref().map(|u| u.timestamp().as_second());
        let (span, until) = match wait {
            Wait::Sleep { from, until } => ((from, until), Some(until)),
            Wait::Paused => ((i64::MIN, i64::MAX), None),
            _ => ((i64::MIN, ends.unwrap_or(i64::MAX)), ends),
        };
        let mut now = attention::Now::of(mode).layers(attention::Slots::load(&attention::Slots::default_path()).holds(stamp), attention::dnd_from_files(config, stamp));
        now.span = span;
        Holds { wait, attention: Attention::of(config), now, until }
    }

    /// What the matrix says now of a reminder of this kind.
    pub fn level(&self, kind: Kind) -> Level {
        self.attention.level(attention::Row::Own(row_of(kind)), &self.now)
    }
}

/// A reminder's row in the matrix.
fn row_of(kind: Kind) -> attention::Kind {
    match kind {
        Kind::Alarm => attention::Kind::Alarms,
        Kind::Before => attention::Kind::Before,
        Kind::Event => attention::Kind::DayBefore,
        _ => attention::Kind::Dates,
    }
}

impl Reminder {
    /// Whether to tell it now, the matrix as usual (`Holds::usual`).
    pub fn ready(&self, now: i64, wait: Wait) -> bool {
        self.ready_in(now, &Holds::usual(wait))
    }

    /// Whether to tell it now: its time has come, it is not too late, the
    /// matrix lets it come (an event's own, when its event begins within the
    /// time now, where the cell says so), and work does not rest for it
    /// (what a thing is for: work's reminders wait while work rests).
    pub fn ready_in(&self, now: i64, holds: &Holds) -> bool {
        let own = matches!(self.kind, Kind::Alarm | Kind::Before);
        let event = attention::Event::own(row_of(self.kind)).starting(self.starts.filter(|_| own)).for_area(self.work.then_some(crate::areas::Area::WORK));
        self.at <= now && now < self.until && holds.attention.decide(&event, &holds.now).told
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

/// When Sioul reminds of an event before it (Unix seconds): its own time
/// (`REMIND`) or the usual `minutes`, counted before its margin (getting
/// ready, getting there): 14:00 with 30 minutes to get there and a quarter
/// of an hour reminds at 13:15. A task's time block is an event like any
/// other (docs/tasks.md, "Pinned to a time"). None for a whole day (the
/// working day before tells it), for "not this one", and, unless they ask,
/// for calendars you only read (holidays, subscriptions: their own alarms
/// still come).
pub fn before_at(event: &Occurrence, minutes: u32) -> Option<i64> {
    if event.all_day {
        return None;
    }
    let usual = if event.read_only { 0 } else { minutes };
    let lead = Remind::read(&event.remind).minutes(usual);
    (lead > 0).then(|| event.start - i64::from(event.margins.before + lead) * 60)
}

/// Everything to remind, told or not, from the events of the coming days, the
/// tasks, and the payments planned; `personal` says which tasks are yours (they do not wait for work).
pub fn all(config: &Config, tr: &Translator, now: &Zoned, events: &[Occurrence], tasks: &[Task], ledger: Option<&Ledger>, papers: &[crate::papers::Paper], contracts: &[crate::contracts::Contract], money: Option<&crate::bank::Watch>, personal: impl Fn(&Task) -> bool) -> Vec<Reminder> {
    let zone = now.time_zone().clone();
    let settings = &config.reminders;
    let hours = hours(config);
    let at = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()));
    let stamp = now.timestamp().as_second();
    let mut out = Vec::new();
    // Your addresses: an event you declined is not reminded.
    let own: Vec<String> = config.every_account().filter_map(|a| a.address.clone()).collect();

    for event in events.iter().filter(|e| !e.cancelled && !declined(e, &own)) {
        let Some(start) = at(event.start) else { continue };
        let when = if event.all_day { tr.day(start.date()) } else { tr.date(&start, true) };
        let place = |text: String| if event.location.is_empty() { text } else if text.is_empty() { event.location.clone() } else { format!("{text} · {}", event.location) };
        // Its own alarms: set by you or the one who invited you. The alarm's
        // offset is the event file's: too far to be a time, none.
        let mut alarms: Vec<(i64, i64)> = event.alarms.iter().filter_map(|offset| event.start.checked_add(*offset).filter(|due| jiff::Timestamp::from_second(*due).is_ok()).map(|due| (*offset, due))).collect();
        // Sioul's before it, counted before its margin; one of its alarms
        // within minutes of it makes one reminder, at the earlier.
        if let Some(due) = before_at(event, settings.before_event)
            && modified(&event.key) <= due
            && !alarms.iter().any(|(_, alarm)| (alarm - due).abs() <= SAME_MOMENT && *alarm < due)
        {
            alarms.retain(|(_, alarm)| (alarm - due).abs() > SAME_MOMENT);
            // Said as it is when told: at waking, the time left is less.
            let told = due.max(stamp);
            let hm = |seconds: i64| at(seconds).map(|z| z.strftime("%H:%M").to_string()).unwrap_or_default();
            let mut args = crate::i18n::args();
            args.set("time", if at(told).is_some_and(|t| t.date() == start.date()) { hm(event.start) } else { tr.date(&start, true) });
            args.set("what", if event.summary.is_empty() { tr.text("agenda-untitled", None) } else { event.summary.clone() });
            args.set("in", in_text(tr, event.start - told));
            let ready = event.start - i64::from(event.margins.before) * 60;
            let margin = match event.margins.before {
                0 => String::new(),
                _ if ready <= told => tr.text("reminder-margin-now", None),
                _ => {
                    let mut from = crate::i18n::args();
                    from.set("time", hm(ready));
                    tr.text("reminder-margin", Some(&from))
                }
            };
            out.push(Reminder {
                key: format!("before:{}:{}", event.uid, event.start),
                kind: Kind::Before,
                at: due,
                until: event.start,
                title: tr.text("reminder-before", Some(&args)),
                body: [margin, event.location.clone()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n"),
                target: event.key.clone(),
                work: false,
                starts: Some(event.start),
            });
        }
        for (offset, due) in alarms {
            out.push(Reminder {
                key: format!("alarm:{}:{}:{offset}", event.uid, event.start),
                kind: Kind::Alarm,
                at: due,
                until: event.start.max(due) + 15 * 60,
                title: event.summary.clone(),
                body: place(when.clone()),
                target: event.key.clone(),
                work: false,
                starts: Some(event.start),
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
                starts: None,
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
                starts: None,
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
                    starts: None,
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
                starts: None,
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
            starts: None,
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
            starts: None,
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
        out.push(Reminder { key, kind: Kind::Money, at: at.timestamp().as_second(), until, title, body, target: "sioul:budget/".into(), work: false, starts: None });
    }
    out.sort_by(|a, b| (a.at, &a.key).cmp(&(b.at, &b.key)));
    out
}

/// Everything to remind, read from the files now: the events of the coming
/// days, the tasks, the payments planned in the case store; and what holds now.
pub fn gather(config: &Config, tr: &Translator, now: &Zoned) -> (Vec<Reminder>, Holds) {
    let stamp = now.timestamp().as_second();
    let events = crate::agenda::occurrences(stamp - 86_400, stamp + 9 * 86_400);
    let tasks = crate::tasks::all(now.time_zone());
    let store = config.case_store_path();
    let looked = crate::words::Words::of(config);
    let ledger = store.as_deref().and_then(|root| Ledger::load_with_bank(root, &looked).ok());
    let cases = store.as_deref().and_then(|root| crate::cases::CaseStore::load(root).ok()).map(|s| s.cases).unwrap_or_default();
    let overrides = crate::quiet::Overrides::load(&crate::quiet::Overrides::default_path());
    // Meals and sleep as Health keeps them: while you sleep, nothing is told.
    let blocks = crate::quiet::Blocks::read(now, &events);
    let situation = crate::quiet::Situation::now(config, &overrides, &blocks, now, tr, &cases);
    let papers = store.as_deref().and_then(|root| crate::papers::Wallet::load(root).ok()).map(|w| w.papers).unwrap_or_default();
    let contracts = store.as_deref().and_then(|root| crate::contracts::Contracts::load(root).ok()).map(|c| c.list).unwrap_or_default();
    let money = store.as_deref().and_then(|root| crate::bank::Bank::load(root).ok()).filter(|b| !b.movements.is_empty() || !b.accounts.is_empty()).zip(ledger.as_ref()).map(|(bank, ledger)| crate::bank::watch(&bank, ledger, now.date(), &looked.bank.filler));
    let reminders = all(config, tr, now, &events, &tasks, ledger.as_ref(), &papers, &contracts, money.as_ref(), |t| situation.quiet_tasks.keeps(t));
    (reminders, Holds::of(config, &situation.mode, &blocks, stamp))
}

/// The events' reminders alone (their alarms, Sioul's before them, the
/// working day before), read from the files now, and what holds now: what a
/// phone's alarm asks at its time (docs/android.md, "Events"), lighter than
/// `gather`.
pub fn gather_events(config: &Config, tr: &Translator, now: &Zoned) -> (Vec<Reminder>, Holds) {
    let stamp = now.timestamp().as_second();
    let events = crate::agenda::occurrences(stamp - 86_400, stamp + 9 * 86_400);
    let overrides = crate::quiet::Overrides::load(&crate::quiet::Overrides::default_path());
    let blocks = crate::quiet::Blocks::read(now, &events);
    let mode = crate::quiet::mode(&config.week_hours(), &config.time_off, &overrides, &blocks, now);
    let reminders = all(config, tr, now, &events, &[], None, &[], &[], None, |_| true);
    (reminders, Holds::of(config, &mode, &blocks, stamp))
}

/// What waits now (`Wait`), from what now is for: the pause first (its time
/// is sleep's too), then sleep, from its block in Health (the night from
/// winding down to waking, a nap), then Free time until it ends, then work
/// resting.
pub fn wait_of(mode: &crate::quiet::Mode, blocks: &crate::quiet::Blocks, stamp: i64) -> Wait {
    let until = mode.until.as_ref().map_or(stamp, |u| u.timestamp().as_second());
    if mode.paused() {
        Wait::Paused
    } else if mode.sleeps() {
        blocks.at(stamp, &["sleep", "nap"]).map_or(Wait::Sleep { from: stamp, until }, |block| Wait::Sleep { from: block.start, until: block.end })
    } else if mode.free() {
        Wait::Free { until }
    } else if mode.quiet {
        Wait::Work
    } else {
        Wait::Nothing
    }
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
pub fn to_tell(reminders: Vec<Reminder>, dir: &Path, now: i64, holds: &Holds) -> Vec<Reminder> {
    reminders.into_iter().filter(|r| r.ready_in(now, holds) && claim(dir, &r.key)).collect()
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
        let asleep = Wait::Sleep { from: tuesday - 8 * 3600, until: tuesday + 3600 };
        assert!(to_tell(all.clone(), &dir, tuesday, &Holds::usual(asleep)).is_empty());
        let mut told = to_tell(all.clone(), &dir, tuesday, &Holds::usual(Wait::Nothing));
        told.retain(|r| r.kind == Kind::Asked);
        assert_eq!(told.len(), 1);
        assert!(to_tell(all.clone(), &dir, tuesday + 60, &Holds::usual(Wait::Nothing)).iter().all(|r| r.kind != Kind::Asked), "never twice");
        let wait = find("wait:").clone();
        assert!(!wait.ready(tuesday, Wait::Work) && wait.ready(tuesday, Wait::Nothing));
        let night = Wait::Sleep { from: find("payment:").at - 3600, until: find("payment:").at + 3600 };
        assert!(!find("payment:").clone().ready(find("payment:").at, night) && find("payment:").ready(find("payment:").at, Wait::Work), "yours wait only for waking");
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

    /// Sioul's own reminders before events, as told at `now`.
    fn befores(config: &Config, tr: &Translator, now: &Zoned, events: &[Occurrence]) -> Vec<Reminder> {
        all(config, tr, now, events, &[], None, &[], &[], None, |_| true).into_iter().filter(|r| matches!(r.kind, Kind::Before | Kind::Alarm)).collect()
    }

    #[test]
    fn before_an_event() {
        let en = Translator::new("en");
        let config = config();
        let now = at("2026-10-07T08:00"); // a Wednesday
        let stamp = |text: &str| at(text).timestamp().as_second();
        // The usual quarter of an hour; with 30 minutes to get there, before them.
        let plain = event("dentist", "2026-10-07T14:00", vec![]);
        let told = befores(&config, &en, &now, std::slice::from_ref(&plain));
        assert_eq!((told.len(), told[0].kind, told[0].at), (1, Kind::Before, stamp("2026-10-07T13:45")));
        assert_eq!((told[0].title.as_str(), told[0].body.as_str()), ("14:00 · Dentist, in 15 minutes", "12 rue Example"));
        assert_eq!((told[0].until, told[0].starts), (stamp("2026-10-07T14:00"), Some(stamp("2026-10-07T14:00"))), "it makes sense until the event begins");
        let far = Occurrence { margins: crate::demands::Margins { before: 30, after: 0 }, ..plain.clone() };
        let told = befores(&config, &en, &now, std::slice::from_ref(&far));
        assert_eq!(told[0].at, stamp("2026-10-07T13:15"));
        assert_eq!((told[0].title.as_str(), told[0].body.as_str()), ("14:00 · Dentist, in 45 minutes", "Getting ready, getting there: from 13:30.\n12 rue Example"));
        // Told late (at waking, say): the time left as it is then.
        let late = befores(&config, &en, &at("2026-10-07T13:40"), std::slice::from_ref(&far));
        assert_eq!((late[0].title.as_str(), late[0].body.as_str()), ("14:00 · Dentist, in 20 minutes", "Getting ready, getting there: now.\n12 rue Example"));
        // In French, and an hour and a half ahead.
        let fr = Translator::new("fr");
        let long = Occurrence { remind: "60".into(), ..far.clone() };
        let dit = befores(&config, &fr, &now, std::slice::from_ref(&long));
        assert_eq!(dit[0].at, stamp("2026-10-07T12:30"));
        assert_eq!((dit[0].title.as_str(), dit[0].body.as_str()), ("14:00 · Dentist, dans 1 h 30", "Se préparer, y aller\u{202f}: dès 13:30.\n12 rue Example"));
        assert_eq!([60, 90 * 60, 2 * 3600, 20].map(|s| in_text(&en, s)), ["in one minute", "in 1 h 30 min", "in 2 hours", "now"]);

        // The event's own choice: not this one; ten minutes; the usual time none, its own still.
        assert!(befores(&config, &en, &now, &[Occurrence { remind: "NONE".into(), ..plain.clone() }]).is_empty());
        assert_eq!(befores(&config, &en, &now, &[Occurrence { remind: "10".into(), ..plain.clone() }])[0].at, stamp("2026-10-07T13:50"));
        let mut none = config.clone();
        none.reminders.before_event = 0;
        assert!(befores(&none, &en, &now, std::slice::from_ref(&plain)).is_empty());
        assert_eq!(befores(&none, &en, &now, &[Occurrence { remind: "5".into(), ..plain.clone() }])[0].at, stamp("2026-10-07T13:55"));
        assert_eq!((Remind::read(""), Remind::read("none"), Remind::read("0"), Remind::read("15"), Remind::read("soon")), (Remind::Usual, Remind::Never, Remind::Never, Remind::Minutes(15), Remind::Usual));
        assert_eq!((Remind::Never.value(), Remind::Minutes(30).value(), Remind::Usual.value()), (Some("NONE".to_string()), Some("30".to_string()), None));
        // Not for a whole day, a calendar you only read, an event cancelled or declined; a task's time block as any event.
        assert!(befores(&config, &en, &now, &[Occurrence { all_day: true, ..plain.clone() }]).is_empty());
        assert!(befores(&config, &en, &now, &[Occurrence { read_only: true, ..plain.clone() }]).is_empty());
        assert_eq!(befores(&config, &en, &now, &[Occurrence { read_only: true, remind: "30".into(), ..plain.clone() }]).len(), 1, "unless it asks");
        assert!(befores(&config, &en, &now, &[Occurrence { cancelled: true, ..plain.clone() }]).is_empty());
        let mut mine = config.clone();
        mine.accounts = toml::from_str::<Config>("[[account]]\nid = \"home\"\nkind = \"imap\"\naddress = \"me@example.net\"\n").unwrap().accounts;
        let invited = |answer: &str| Occurrence { attendees: vec![crate::agenda::Attendee { name: "Me".into(), address: "ME@example.net".into(), answer: answer.into() }], ..plain.clone() };
        assert!(befores(&mine, &en, &now, &[invited("declined")]).is_empty());
        assert_eq!(befores(&mine, &en, &now, &[invited("accepted")]).len(), 1);
        assert_eq!(befores(&config, &en, &now, &[Occurrence { task: "bank".into(), ..plain.clone() }]).len(), 1);

        // One reminder per moment: an alarm within five minutes and Sioul's make one, the earlier.
        let same = befores(&config, &en, &now, &[event("dentist", "2026-10-07T14:00", vec![-15 * 60])]);
        assert_eq!(same.iter().map(|r| r.kind).collect::<Vec<_>>(), [Kind::Before]);
        let earlier = befores(&config, &en, &now, &[event("dentist", "2026-10-07T14:00", vec![-20 * 60])]);
        assert_eq!(earlier.iter().map(|r| (r.kind, r.at)).collect::<Vec<_>>(), [(Kind::Alarm, stamp("2026-10-07T13:40"))]);
        let apart = befores(&config, &en, &now, &[event("dentist", "2026-10-07T14:00", vec![-60 * 60])]);
        assert_eq!(apart.iter().map(|r| r.kind).collect::<Vec<_>>(), [Kind::Alarm, Kind::Before]);

        // Sleep: an event in the night is reminded in it; one after waking waits for waking.
        let night = Wait::Sleep { from: stamp("2026-10-07T23:00"), until: stamp("2026-10-08T07:00") };
        let flight = &befores(&config, &en, &now, &[event("flight", "2026-10-08T03:00", vec![])])[0];
        assert!(flight.ready(stamp("2026-10-08T02:45"), night), "a choice you made");
        let train = Occurrence { margins: crate::demands::Margins { before: 60, after: 0 }, ..event("train", "2026-10-08T08:00", vec![]) };
        let early = &befores(&config, &en, &now, std::slice::from_ref(&train))[0];
        assert_eq!(early.at, stamp("2026-10-08T06:45"));
        assert!(!early.ready(stamp("2026-10-08T06:45"), night) && early.ready(stamp("2026-10-08T07:00"), Wait::Nothing));
        let woken = befores(&config, &en, &at("2026-10-08T07:00"), std::slice::from_ref(&train));
        assert_eq!(woken[0].title, "08:00 · Dentist, in one hour", "said at waking");
        // The pause: an event's own reminders come (the event falls in it); the working day before waits.
        assert!(flight.ready(stamp("2026-10-08T02:45"), Wait::Paused));
        let eve = all(&config, &en, &now, &[event("dentist", "2026-10-12T09:00", vec![])], &[], None, &[], &[], None, |_| true);
        let evening = eve.iter().find(|r| r.kind == Kind::Event).unwrap();
        assert!(!evening.ready(evening.at, Wait::Paused) && !evening.ready(evening.at, night_of(evening.at)));
        // Free time: an event within it comes, one after it waits for its end.
        let dinner = &befores(&config, &en, &now, &[event("dinner", "2026-10-07T20:00", vec![])])[0];
        assert!(dinner.ready(dinner.at, Wait::Free { until: stamp("2026-10-07T22:30") }));
        assert!(!dinner.ready(dinner.at, Wait::Free { until: stamp("2026-10-07T19:00") }));

        // Kept in the event's own file, as Sioul's property: written, read back, changed, taken away.
        let edit = crate::agenda::EventEdit { title: "Choir".into(), start: "2026-10-07T20:00".into(), end: "2026-10-07T22:00".into(), repeat: "weekly".into(), remind: "30".into(), ..Default::default() };
        let master = crate::agenda::new_event(&edit, &zone()).unwrap();
        assert!(master.contains("X-SIOUL-REMIND:30") && !master.contains("VALARM"), "{master}");
        assert_eq!(crate::agenda::edit_of_text(&master, &zone()).unwrap().remind, "30");
        let never = crate::agenda::apply(&master, &crate::agenda::EventEdit { remind: "none".into(), ..edit.clone() }, &zone()).unwrap();
        assert!(never.contains("X-SIOUL-REMIND:NONE") && !never.contains("X-SIOUL-REMIND:30"), "{never}");
        assert_eq!(crate::agenda::edit_of_text(&never, &zone()).unwrap().remind, "none");
        let usual = crate::agenda::apply(&never, &crate::agenda::EventEdit { remind: String::new(), ..edit.clone() }, &zone()).unwrap();
        assert!(!usual.contains("X-SIOUL-REMIND"), "{usual}");
        let retitled = crate::agenda::apply(&master, &crate::agenda::EventEdit { title: "Choir practice".into(), ..edit.clone() }, &zone()).unwrap();
        assert!(retitled.contains("X-SIOUL-REMIND:30"), "kept when something else changes");

        // Recurring: each occurrence its own, from the file; a changed one keeps the series' choice.
        let uid = crate::lines::unfold(&master).iter().find(|l| crate::lines::name(l) == "UID").map(|l| crate::lines::value(l).trim().to_string()).unwrap();
        let changed = format!(
            "BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261001T000000Z\r\nRECURRENCE-ID;TZID=Europe/Paris:20261014T200000\r\nDTSTART;TZID=Europe/Paris:20261014T193000\r\n\
             DTEND;TZID=Europe/Paris:20261014T213000\r\nSUMMARY:Choir, earlier\r\nEND:VEVENT\r\n"
        );
        let dir = std::env::temp_dir().join(format!("sioul-remind-series-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("choir.ics");
        std::fs::write(&path, master.replace("END:VCALENDAR", &format!("{changed}END:VCALENDAR"))).unwrap();
        // Written long ago: "changed after its reminder" never holds it back here.
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_750_000_000)).unwrap();
        let calendar = crate::vdir::Collection { kind: crate::vdir::Kind::Calendars, account: "a".into(), id: "c".into(), dir: dir.clone(), name: "Personal".into(), color: None, read_only: false, components: Vec::new() };
        let series = crate::agenda::file_occurrences(&path, &calendar, stamp("2026-10-07T00:00"), stamp("2026-10-22T00:00"), &zone());
        let told = befores(&config, &en, &now, &series);
        assert_eq!(told.iter().map(|r| r.at).collect::<Vec<_>>(), [stamp("2026-10-07T19:30"), stamp("2026-10-14T19:00"), stamp("2026-10-21T19:30")], "{told:#?}");
        assert_eq!(told.iter().map(|r| r.key.as_str()).collect::<std::collections::BTreeSet<_>>().len(), 3, "each its own, told once each");
        assert_eq!(told[1].title, "19:30 · Choir, earlier, in 30 minutes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A night around `stamp` that holds nothing of the events tested.
    fn night_of(stamp: i64) -> Wait {
        Wait::Sleep { from: stamp - 3600, until: stamp + 3600 }
    }

    #[test]
    fn what_waits_now() {
        use crate::areas::{Time, Week};
        use crate::quiet::{Blocks, Mode, Reason};
        let now = at("2026-10-07T23:30").timestamp().as_second();
        let mode = |time: Time, reason: Reason, until: Option<&str>| Mode { quiet: true, time, week: Week::default(), reason, until: until.map(at), back: None, label: String::new() };
        let night = crate::needs::Kept { key: "sleep".into(), kind: "sleep", index: 0, name: String::new(), start: now - 3600, at: now, end: now + 7 * 3600, notices: false };
        let blocks = Blocks { kept: vec![night], sleep: true, night: true, ..Blocks::default() };
        assert_eq!(wait_of(&mode(Time::Sleep, Reason::Sleep, Some("2026-10-08T06:30")), &blocks, now), Wait::Sleep { from: now - 3600, until: now + 7 * 3600 });
        assert_eq!(wait_of(&mode(Time::Sleep, Reason::Paused, None), &blocks, now), Wait::Paused, "the pause first: its time is sleep's too");
        assert_eq!(wait_of(&mode(Time::Leisure, Reason::FreeTime, Some("2026-10-07T23:45")), &Blocks::default(), now), Wait::Free { until: now + 15 * 60 });
        assert_eq!(wait_of(&mode(Time::Leisure, Reason::Evening, None), &Blocks::default(), now), Wait::Work);
        assert_eq!(wait_of(&Mode { quiet: false, ..mode(Time::Work, Reason::Working, None) }, &Blocks::default(), now), Wait::Nothing);
        // As the matrix reads it, from the configuration: the sleep's block, Free time's end, the hours' end.
        let config = Config::default();
        let asleep = Holds::of(&config, &mode(Time::Sleep, Reason::Sleep, Some("2026-10-08T06:30")), &blocks, now);
        assert_eq!((asleep.now.times.clone(), asleep.now.span, asleep.until), (vec![attention::Column::Sleep], (now - 3600, now + 7 * 3600), Some(now + 7 * 3600)));
        let free = Holds::of(&config, &mode(Time::Leisure, Reason::FreeTime, Some("2026-10-07T23:45")), &Blocks::default(), now);
        assert_eq!((free.now.times.clone(), free.now.span.1, free.until), (vec![attention::Column::Free], now + 15 * 60, Some(now + 15 * 60)));
        let paused = Holds::of(&config, &mode(Time::Sleep, Reason::Paused, None), &blocks, now);
        assert_eq!((paused.now.times.clone(), paused.until), (vec![attention::Column::Pause], None));
        let evening = Holds::of(&config, &mode(Time::Leisure, Reason::Evening, Some("2026-10-08T09:00")), &Blocks::default(), now);
        assert_eq!((evening.wait, evening.until), (Wait::Work, Some(at("2026-10-08T09:00").timestamp().as_second())));
    }

    /// Reminders of each kind, due at `at` for an event (or a date) at `starts`.
    fn each_kind(at: i64, starts: i64) -> Vec<Reminder> {
        [Kind::Before, Kind::Alarm, Kind::Event, Kind::Asked, Kind::Wait, Kind::Payment, Kind::Paper, Kind::Contract, Kind::Money]
            .into_iter()
            .flat_map(|kind| [false, true].map(move |work| Reminder { key: format!("{kind:?}:{work}"), kind, at, until: starts + 60, title: String::new(), body: String::new(), target: String::new(), work, starts: Some(starts) }))
            .collect()
    }

    #[test]
    fn the_usual_matrix_holds_as_before() {
        // The rule as it was before the matrix, word for word.
        let before = |r: &Reminder, now: i64, wait: Wait| {
            let own = matches!(r.kind, Kind::Alarm | Kind::Before);
            let falls_in = |from: i64, until: i64| own && r.starts.is_some_and(|s| from <= s && s < until);
            let held = match wait {
                Wait::Nothing => false,
                Wait::Work => r.work,
                Wait::Sleep { from, until } => !falls_in(from, until),
                Wait::Paused => !own,
                Wait::Free { until } => !falls_in(i64::MIN, until),
            };
            r.at <= now && now < r.until && !held
        };
        let now = at("2026-10-07T23:30").timestamp().as_second();
        let waits = [Wait::Nothing, Wait::Work, Wait::Sleep { from: now - 3600, until: now + 3600 }, Wait::Paused, Wait::Free { until: now + 1800 }];
        for starts in [now + 600, now + 2 * 3600] {
            for r in each_kind(now - 60, starts) {
                for wait in waits {
                    assert_eq!(r.ready(now, wait), before(&r, now, wait), "{:?} {wait:?} {starts}", r.key);
                }
            }
        }
    }

    #[test]
    fn the_matrix_changes_what_waits() {
        let now = at("2026-10-07T23:30").timestamp().as_second();
        let night = Wait::Sleep { from: now - 3600, until: now + 7 * 3600 };
        let after_waking = now + 8 * 3600;
        let train = Reminder { key: "before:train".into(), kind: Kind::Before, at: now - 60, until: after_waking, title: String::new(), body: String::new(), target: String::new(), work: false, starts: Some(after_waking) };
        let dinner = Reminder { key: "alarm:dinner".into(), kind: Kind::Alarm, starts: Some(now + 600), until: now + 600, ..train.clone() };
        let bill = Reminder { key: "payment:water".into(), kind: Kind::Payment, until: now + 86_400, starts: None, ..train.clone() };
        let mut holds = Holds::usual(night);
        assert!(!train.ready_in(now, &holds) && !bill.ready_in(now, &holds), "as usual: both wait for waking");
        // Reminders before an event come during sleep, whenever the event; the working day's dates too.
        holds.attention.set(attention::Row::Own(attention::Kind::Before), attention::Column::Sleep, Level::Now).unwrap();
        holds.attention.set(attention::Row::Own(attention::Kind::Dates), attention::Column::Sleep, Level::Now).unwrap();
        assert!(train.ready_in(now, &holds) && bill.ready_in(now, &holds));
        // Reminders before an event wait during a pause; the alarms you set still come there.
        let mut paused = Holds::usual(Wait::Paused);
        paused.attention.set(attention::Row::Own(attention::Kind::Before), attention::Column::Pause, Level::Later).unwrap();
        assert!(!train.ready_in(now, &paused) && dinner.ready_in(now, &paused));
        // An event's alarms during sleep, waiting for waking whatever the event.
        let mut asleep = Holds::usual(night);
        assert!(dinner.ready_in(now, &asleep), "as usual: its event falls in the night");
        asleep.attention.set(attention::Row::Own(attention::Kind::Alarms), attention::Column::Sleep, Level::Later).unwrap();
        assert!(!dinner.ready_in(now, &asleep));
        // During do-not-disturb: the working day before waits when you say so; work resting still holds work's.
        let mut working = Holds::usual(Wait::Nothing);
        let eve = Reminder { key: "event:board".into(), kind: Kind::Event, starts: None, ..bill.clone() };
        working.now.dnd = true;
        assert!(eve.ready_in(now, &working));
        working.attention.set(attention::Row::Own(attention::Kind::DayBefore), attention::Column::Dnd, Level::Later).unwrap();
        assert!(!eve.ready_in(now, &working) && bill.ready_in(now, &working));
        let mut evening = Holds::usual(Wait::Work);
        evening.attention.set(attention::Row::Own(attention::Kind::Dates), attention::Column::Leisure, Level::Later).unwrap();
        let personal = Reminder { key: "asked:garden".into(), kind: Kind::Asked, ..bill.clone() };
        let work = Reminder { key: "asked:report".into(), work: true, ..personal.clone() };
        assert!(!personal.ready_in(now, &evening) && !work.ready_in(now, &evening));
        assert!(personal.ready(now, Wait::Work) && !work.ready(now, Wait::Work), "as usual, work's waits for work");
    }
}
