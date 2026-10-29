// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Dates asked of you, as the agenda draws them (the unified agenda, part 4;
//! its proposal's §7): a task's date asked, at its hour when it has one; a
//! letter's date asked and its appointment while it is new; a contract's last
//! day to give notice, for one that renews each year; a paper's last valid
//! day; a planned payment; an invoice you issued and that is not paid; in the
//! window only, a prescription's end and the day a medicine runs out
//! (`of_prescriptions`, which the window alone calls: health never leaves it).
//!
//! Each adapter reads its own store as the caller gives it and keeps the
//! rules of §7.1, so that a date shows once: a task's date that its
//! appointment wrote (`pairs::own_date`) gives way to the appointment; a
//! letter's date to its task, and its appointment to an event that stands
//! for it (one that carries the letter's address, one that links its scan,
//! or an event of yours that day within an hour of it); a paper's end to its
//! renewal task. Only today and the days to come: a date gone by leaves the
//! agenda (its card and its page keep it), never carried onto today. Nothing
//! is written, and nothing here says "late".

use crate::contracts::Contract;
use crate::i18n::Translator;
use crate::letters::Letters;
use crate::papers::Paper;
use crate::tasks::Task;
use crate::timeline::{Can, Certainty, Form, Item, Move, Privacy, Source, Time, Tone};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::Serialize;

/// What a date is: each kind's own words (`date-…`) and its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A task's date asked (`DUE`).
    Task,
    /// A letter's date asked (`Reading::deadline`).
    Letter,
    /// A letter's appointment (`Reading::appointment`).
    LetterAppointment,
    /// A contract's last day to send notice (`Contract::cancel_by`).
    Notice,
    /// A paper's last valid day (`Paper::until`).
    Paper,
    /// A planned payment: a planned budget line of money out.
    Payment,
    /// An invoice you issued, not paid: its due day.
    Invoice,
    /// A prescription's last valid day: health, named by its kind outside its card.
    PrescriptionEnds,
    /// The day a prescription's medicines run out, from its last refill: health too.
    RunsOut,
}

impl Kind {
    /// The one family of words that names it, in marks, cards and reminders.
    pub fn word(self) -> &'static str {
        match self {
            Kind::Task | Kind::Letter => "date-asked",
            Kind::LetterAppointment => "date-letter-appointment",
            Kind::Notice => "date-notice-by",
            Kind::Paper => "date-valid-until",
            Kind::Payment => "date-payment",
            Kind::Invoice => "date-invoice-due",
            Kind::PrescriptionEnds => "date-prescription-ends",
            Kind::RunsOut => "date-runs-out",
        }
    }

    /// Health's: never named outside its card, never given to an agent.
    pub fn is_health(self) -> bool {
        matches!(self, Kind::PrescriptionEnds | Kind::RunsOut)
    }

    /// The form its card's Edit opens.
    fn form(self) -> Form {
        match self {
            Kind::Task => Form::Task,
            Kind::Letter | Kind::LetterAppointment => Form::Letter,
            Kind::Notice => Form::Contract,
            Kind::Paper => Form::Paper,
            Kind::Payment => Form::Line,
            Kind::Invoice => Form::None,
            Kind::PrescriptionEnds | Kind::RunsOut => Form::Prescription,
        }
    }
}

/// One date asked of you.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dated {
    pub kind: Kind,
    /// What its card reads (`datedCard(kind, key)`): the task's UID, the
    /// letter's, contract's or paper's id, the budget line's address, the
    /// invoice's number, the prescription's id.
    pub key: String,
    /// Its `sioul:` address, for ties; "" for health's.
    pub address: String,
    /// Its own title ("CAF form", "Rent · €650"); for health's, its kind's words.
    pub title: String,
    pub date: Date,
    /// Its time, Unix seconds, when it has one (a DUE at 17:00, a letter's
    /// appointment at 14:00); None for a day.
    pub at: Option<i64>,
}

/// A task's date asked, at its hour when it has one, while the task is open;
/// not the day its appointment wrote (`pairs::own_date`): the appointment
/// stands for it. Only from `today` on.
pub fn of_tasks(tasks: &[Task], today: Date, zone: &TimeZone) -> Vec<Dated> {
    tasks
        .iter()
        .filter(|t| t.status.is_open())
        .filter_map(|task| {
            let date = crate::pairs::own_date(task)?;
            if date < today {
                return None;
            }
            let at = task.due.get(11..16).and_then(|_| task.due.parse::<jiff::civil::DateTime>().ok()).and_then(|d| d.to_zoned(zone.clone()).ok()).map(|z| z.timestamp().as_second());
            Some(Dated { kind: Kind::Task, key: task.uid.clone(), address: crate::links::task_uri(&task.uid), title: task.title.clone(), date, at })
        })
        .collect()
}

/// An event that may stand for a letter's appointment: its file and time,
/// whether it is yours (not of a calendar you only read), and its text,
/// which says the letter's address or its scan.
pub struct EventSeen {
    pub start: i64,
    pub yours: bool,
    pub text: String,
}

/// The event that stands for a letter's appointment on `day` at `at` (Unix
/// seconds, its time when written): one that carries the letter's address
/// ("Into the agenda" writes it); one made before that links its scan (the
/// scan's path read relative to the notes folder, so on every device); else
/// an event of yours that day within an hour of its time, which counts until
/// you correct it (its card says which). Its index in `events`.
pub fn letter_event(letters: &Letters, letter: &crate::letters::Letter, at: Option<i64>, events: &[EventSeen]) -> Option<usize> {
    let address = crate::links::letter_uri(&letter.id);
    let scan = letter.file.replace('\\', "/");
    let names_scan = |text: &str| !scan.is_empty() && crate::lines::unfold(text).iter().filter(|l| crate::lines::name(l) == "LINK").any(|l| links_scan(crate::lines::value(l).trim(), &scan, &letters.root));
    events
        .iter()
        .position(|e| crate::lines::unfold(&e.text).iter().any(|l| crate::lines::name(l) == "LINK" && crate::lines::value(l).trim() == address))
        .or_else(|| events.iter().position(|e| names_scan(&e.text)))
        .or_else(|| at.and_then(|at| events.iter().position(|e| e.yours && (e.start - at).abs() <= 3_600)))
}

/// Whether a LINK's address names the scan at `scan` (relative to the notes
/// folder `root`, or absolute): a `file:` address whose path ends with it,
/// so that the same scan reads on a device whose notes folder lies elsewhere.
pub fn links_scan(uri: &str, scan: &str, root: &std::path::Path) -> bool {
    let Some(path) = uri.strip_prefix("file://") else { return false };
    let path = crate::notes::decode(path).replace('\\', "/");
    if std::path::Path::new(scan).is_absolute() {
        return path == scan;
    }
    let root = root.display().to_string().replace('\\', "/");
    path == format!("{}/{scan}", root.trim_end_matches('/')) || path.ends_with(&format!("/{scan}"))
}

/// A letter's date asked and its appointment, while the letter is new: its
/// date asked gives way while its task is in a list on this device, open
/// (its own mark stands for it) or done (settled), found by its UID or, on a
/// list that keeps no UID (Google's), by the id that list gave it
/// (`Letter::task_in`, `Letters::note_elsewhere`); a task deleted, here or
/// elsewhere, gives the date back (review of 21436bf, later 3). Its appointment to an event that stands for it
/// (`letter_event`; `events_on` gives each day's events). Only from `today` on.
pub fn of_letters(letters: &Letters, tasks: &[Task], events_on: &dyn Fn(Date) -> Vec<EventSeen>, today: Date, zone: &TimeZone, tr: &Translator) -> Vec<Dated> {
    let mut out = Vec::new();
    for letter in letters.list.iter().filter(|l| l.status != "done") {
        let r = &letter.reading;
        let title = if r.sender.trim().is_empty() { tr.text("letter-unknown-sender", None) } else { r.sender.clone() };
        // Its task stands for its date asked, open or done: a date done is settled
        // (§7.2, "it goes when settled"; review of part 4, later 7); one deleted, not.
        let tasked = letter.task_in(tasks);
        if let Some(deadline) = r.deadline.filter(|d| *d >= today)
            && !tasked
        {
            out.push(Dated { kind: Kind::Letter, key: letter.id.clone(), address: crate::links::letter_uri(&letter.id), title: title.clone(), date: deadline, at: None });
        }
        if let Some((date, time)) = r.appointment.filter(|(d, _)| *d >= today) {
            let at = time.and_then(|(h, m)| date.at(h, m, 0, 0).to_zoned(zone.clone()).ok()).map(|z| z.timestamp().as_second());
            if letter_event(letters, letter, at, &events_on(date)).is_none() {
                out.push(Dated { kind: Kind::LetterAppointment, key: letter.id.clone(), address: crate::links::letter_uri(&letter.id), title, date, at });
            }
        }
    }
    out
}

/// A contract's last day to send notice, for an open one that renews each
/// year with a notice to give (decision 23: one that renews each month draws
/// no mark). Only from `today` on.
pub fn of_contracts(contracts: &[Contract], today: Date) -> Vec<Dated> {
    contracts
        .iter()
        .filter(|c| c.is_open() && c.every == "year" && c.notice_days > 0)
        .filter_map(|c| {
            let by = c.cancel_by(today).filter(|d| *d >= today)?;
            Some(Dated { kind: Kind::Notice, key: c.id.clone(), address: crate::links::contract_uri(&c.id), title: c.title.clone(), date: by, at: None })
        })
        .collect()
}

/// A paper's last valid day, while it is kept and its renewal task is not
/// open (that task's date asked is the same day). Only from `today` on.
pub fn of_papers(papers: &[Paper], tasks: &[Task], today: Date) -> Vec<Dated> {
    papers
        .iter()
        .filter(|p| p.renewal.is_empty() || !tasks.iter().any(|t| t.uid == p.renewal && t.status.is_open()))
        .filter_map(|p| {
            let until = p.until.filter(|d| *d >= today)?;
            Some(Dated { kind: Kind::Paper, key: p.id.clone(), address: crate::links::paper_uri(&p.id), title: p.title.clone(), date: until, at: None })
        })
        .collect()
}

/// A planned payment: a planned line of money out, on its day, its amount in
/// its title. Each by its own line (`links::budget_uri`, its place in the
/// ledger as `Ledger::load_with_bank` reads it). An invoice's own line is
/// money in, never drawn. Only from `today` on.
pub fn of_payments(ledger: &crate::budget::Ledger, today: Date, tr: &Translator) -> Vec<Dated> {
    ledger
        .lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.planned && l.amount.cents() < 0 && l.date >= today)
        .map(|(place, line)| {
            let address = crate::links::budget_uri(&line.budget, place);
            Dated { kind: Kind::Payment, key: address.clone(), address, title: format!("{} · {}", line.label, tr.money(crate::money::Money(-line.amount.cents()))), date: line.date, at: None }
        })
        .collect()
}

/// An invoice you issued and that is not paid, on its due day (its own
/// device's: invoices live where they were made). Only from `today` on.
pub fn of_invoices(invoices: &[crate::invoice::Invoice], today: Date, tr: &Translator) -> Vec<Dated> {
    invoices
        .iter()
        .filter(|i| !i.paid && !i.number.is_empty())
        .filter_map(|i| {
            let due: Date = i.due.get(..10)?.parse().ok().filter(|d| *d >= today)?;
            let who = if i.client.trim().is_empty() { i.project_title.clone() } else { i.client.clone() };
            let title = [i.number.clone(), who, tr.money(i.total())].into_iter().filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join(" · ");
            Some(Dated { kind: Kind::Invoice, key: i.number.clone(), address: crate::links::invoice_uri(&i.number), title, date: due, at: None })
        })
        .collect()
}

/// A prescription's last valid day and the day its medicines run out (its
/// last refill and the days the pharmacy gives), for the window alone: health,
/// named by their kind outside their card (`item`). Only from `today` on; the
/// day they run out only while the prescription still holds then.
pub fn of_prescriptions(prescriptions: &[crate::health::Prescription], today: Date) -> Vec<Dated> {
    let mut out = Vec::new();
    for p in prescriptions {
        if let (Some(days), Some(last)) = (p.refill_days, p.last_refill)
            && let Ok(out_of) = last.checked_add(jiff::Span::new().days(i64::from(days)))
            && out_of >= today
            && p.until.is_none_or(|until| out_of <= until)
        {
            out.push(Dated { kind: Kind::RunsOut, key: p.id.clone(), address: String::new(), title: p.title.clone(), date: out_of, at: None });
        }
        if let Some(until) = p.until.filter(|d| *d >= today) {
            out.push(Dated { kind: Kind::PrescriptionEnds, key: p.id.clone(), address: String::new(), title: p.title.clone(), date: until, at: None });
        }
    }
    out
}

/// Every admin date asked of you from `today` on, each store read as it is
/// now (the notes folder's letters, contracts, papers and budgets; this
/// device's invoices), the tasks and each day's events given by the caller.
/// Health's dates are not here (`of_prescriptions`).
pub fn read_all(config: &crate::config::Config, tasks: &[Task], events_on: &dyn Fn(Date) -> Vec<EventSeen>, today: Date, zone: &TimeZone, tr: &Translator) -> Vec<Dated> {
    let mut out = of_tasks(tasks, today, zone);
    if let Some(root) = config.notes_root_path() {
        if let Ok(letters) = Letters::load(&root) {
            out.extend(of_letters(&letters, tasks, events_on, today, zone, tr));
        }
        if let Ok(contracts) = crate::contracts::Contracts::load(&root) {
            out.extend(of_contracts(&contracts.list, today));
        }
        if let Ok(wallet) = crate::papers::Wallet::load(&root) {
            out.extend(of_papers(&wallet.papers, tasks, today));
        }
        if let Ok(ledger) = crate::budget::Ledger::load_with_bank(&root, &crate::words::Words::of(config)) {
            out.extend(of_payments(&ledger, today, tr));
        }
    }
    out.extend(of_invoices(&crate::invoice::all_in(&crate::invoice::folder()), today, tr));
    out.sort_by(|a, b| (a.date, a.at, &a.title).cmp(&(b.date, b.at, &b.title)));
    out
}

/// The time left until `date`, in words: "today", "tomorrow", "four days
/// left", "three weeks left"; the same whether the plan finishes in time or
/// not, so that no missing phrase ever comes to mean "not in time".
pub fn left(tr: &Translator, today: Date, date: Date) -> String {
    let days = today.until(date).map_or(0, |span| span.get_days() as i64);
    let counted = |id: &str, n: i64| tr.text(id, Some(&tr.counted(n.max(0) as usize)));
    match days {
        d if d <= 0 => tr.text("date-left-today", None),
        1 => tr.text("date-left-tomorrow", None),
        d if d < 14 => counted("task-left-days", d),
        d if d < 61 => counted("task-left-weeks", d / 7),
        d => counted("task-left-months", d / 30),
    }
}

/// Within seven days, its mark is filled; beyond, outlined; an invoice is
/// money owed to you, never filled.
pub fn filled(dated: &Dated, today: Date) -> bool {
    dated.kind != Kind::Invoice && today.until(dated.date).is_ok_and(|span| span.get_days() < 7)
}

/// The item the agenda draws: a mark at the head of its day, or at its time;
/// its line its kind's words and the time left ("Date asked · four days
/// left"); filled within seven days (`Tone::Plain`), outlined beyond
/// (`Tone::Quiet`). A date takes no time: nothing moves it here, its card's
/// Edit opens its own form.
pub fn item(dated: &Dated, today: Date, tr: &Translator) -> Item {
    let health = dated.kind.is_health();
    let time = match dated.at {
        Some(at) => Time::At { at },
        None => Time::Day { date: dated.date, until: dated.date },
    };
    let word = tr.text(dated.kind.word(), None);
    Item {
        id: format!("dated:{}:{}:{}", serde_json::to_value(dated.kind).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(), dated.key, dated.date),
        kind: crate::timeline::Kind::Dated,
        time,
        // Read from a scan: it counts until you correct it.
        certainty: if matches!(dated.kind, Kind::Letter | Kind::LetterAppointment) { Certainty::Read } else { Certainty::Fixed },
        source: Source::Dated { kind: dated.kind, key: dated.key.clone() },
        can: Can { edit: dated.kind.form(), drag: Move::None, ends: false, delete: false, tick: false, why: String::new() },
        privacy: if health { Privacy::Health } else { Privacy::Admin },
        // Health's by its kind alone: its card names it.
        title: if health { word.clone() } else { dated.title.clone() },
        line: if health { left(tr, today, dated.date) } else { format!("{word} · {}", left(tr, today, dated.date)) },
        said: String::new(),
        tone: if filled(dated, today) { Tone::Plain } else { Tone::Quiet },
        colour: None,
        address: if health { String::new() } else { dated.address.clone() },
        owner: if dated.kind == Kind::Task { dated.key.clone() } else { String::new() },
        holds_time: false,
    }
}

/// The dates of `all` from `from` on, `count` days of them, and none before
/// `today`: once its day has passed, a date leaves the agenda.
pub fn within(all: Vec<Dated>, from: Date, count: i64, today: Date) -> Vec<Dated> {
    let end = from.checked_add(jiff::Span::new().days(count)).unwrap_or(from);
    all.into_iter().filter(|d| d.date >= today && d.date >= from && d.date < end).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::Status;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn task(uid: &str, due: &str) -> Task {
        Task { uid: uid.into(), title: uid.into(), due: due.into(), status: Status::NeedsAction, ..Task::default() }
    }

    /// A task's date asked, at its hour when it has one; not once done, nor
    /// the day its appointment wrote, nor a day gone by.
    #[test]
    fn a_task_s_date_asked() {
        let today = day("2026-10-26");
        let mut paired = task("paired", "2026-10-29");
        paired.due_written = "2026-10-29".into();
        let mut done = task("done", "2026-10-30");
        done.status = Status::Completed;
        let tasks = [task("caf", "2026-10-30"), task("form", "2026-10-30T17:00"), task("gone", "2026-10-20"), paired, done];
        let dates = of_tasks(&tasks, today, &paris());
        assert_eq!(dates.iter().map(|d| d.key.as_str()).collect::<Vec<_>>(), ["caf", "form"]);
        assert_eq!(dates[0].at, None);
        assert_eq!(dates[1].at, Some(day("2026-10-30").at(17, 0, 0, 0).to_zoned(paris()).unwrap().timestamp().as_second()), "at its hour");
        assert_eq!(dates[0].address, "sioul:task/caf");
    }

    fn letter(id: &str, deadline: Option<&str>, appointment: Option<(&str, Option<(i8, i8)>)>) -> crate::letters::Letter {
        crate::letters::Letter {
            id: id.into(),
            file: format!("letters/2026/{id}.pdf"),
            status: "new".into(),
            reading: crate::letters::Reading { sender: "CAF".into(), deadline: deadline.map(day), appointment: appointment.map(|(d, t)| (day(d), t)), ..Default::default() },
            ..Default::default()
        }
    }

    /// A letter's date asked gives way to its open task; its appointment to an
    /// event that carries its address, links its scan (read from two notes
    /// folders), or is yours that day within an hour of it; a letter done shows nothing.
    #[test]
    fn a_letter_s_dates_and_what_stands_for_them() {
        let (today, zone, tr) = (day("2026-10-26"), paris(), Translator::new("en"));
        let mut tasked = letter("tasked", Some("2026-10-30"), None);
        tasked.task = "t1".into();
        let mut done = letter("done", Some("2026-10-30"), None);
        done.status = "done".into();
        let letters = Letters { root: "/home/a/Notes".into(), list: vec![letter("plain", Some("2026-10-30"), Some(("2026-11-02", Some((14, 0))))), tasked, done] };
        let none = |_: Date| Vec::new();
        let mut done_task = task("t1", "");
        done_task.status = Status::Completed;
        let dates = of_letters(&letters, &[task("t1", "")], &none, today, &zone, &tr);
        assert_eq!(dates.iter().map(|d| (d.key.as_str(), d.kind)).collect::<Vec<_>>(), [("plain", Kind::Letter), ("plain", Kind::LetterAppointment)]);
        assert_eq!(dates[1].address, "sioul:letter/plain");
        let two = day("2026-11-02").at(14, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let stands = |text: &'static str, start: i64, yours: bool| move |_: Date| vec![EventSeen { start, yours, text: text.to_string() }];
        let appointment = |events: &dyn Fn(Date) -> Vec<EventSeen>| of_letters(&letters, &[], events, today, &zone, &tr).into_iter().any(|d| d.kind == Kind::LetterAppointment);
        assert!(!appointment(&stands("BEGIN:VEVENT\r\nLINK;VALUE=URI:sioul:letter/plain\r\nEND:VEVENT\r\n", two + 7_200, false)), "its address");
        assert!(!appointment(&stands("BEGIN:VEVENT\r\nLINK;VALUE=URI:file:///home/a/Notes/letters/2026/plain.pdf\r\nEND:VEVENT\r\n", two + 7_200, false)), "its scan, here");
        assert!(!appointment(&stands("BEGIN:VEVENT\r\nLINK;VALUE=URI:file:///storage/emulated/0/Notes/letters/2026/plain.pdf\r\nEND:VEVENT\r\n", two + 7_200, false)), "its scan, on a phone's notes folder");
        assert!(!appointment(&stands("BEGIN:VEVENT\r\nSUMMARY:Doctor\r\nEND:VEVENT\r\n", two + 1_800, true)), "an event of yours within an hour");
        assert!(appointment(&stands("BEGIN:VEVENT\r\nSUMMARY:Doctor\r\nEND:VEVENT\r\n", two + 5_400, true)), "beyond the hour: still its own mark");
        assert!(appointment(&stands("BEGIN:VEVENT\r\nSUMMARY:Board\r\nEND:VEVENT\r\n", two, false)), "not yours");
        // Its task, open or done (settled), stands for its date asked (review of part 4, later 7);
        // deleted, here or elsewhere, its date back (review of 21436bf, later 3).
        assert!(!dates.iter().any(|d| d.key == "tasked"));
        assert!(!of_letters(&letters, &[done_task], &none, today, &zone, &tr).iter().any(|d| d.key == "tasked"), "done: settled");
        assert!(of_letters(&letters, &[], &none, today, &zone, &tr).iter().any(|d| d.key == "tasked" && d.kind == Kind::Letter), "deleted: its date back");
        // A Google list's task, known on this device by the id that list gave it: found, the date kept hidden.
        let mut elsewhere = letters.clone();
        elsewhere.list.iter_mut().filter(|l| l.id == "tasked").for_each(|l| l.task_elsewhere = "g-17".into());
        assert!(!of_letters(&elsewhere, &[task("g-17", "")], &none, today, &zone, &tr).iter().any(|d| d.key == "tasked"), "found by its list's id");
        assert!(of_letters(&elsewhere, &[task("g-18", "")], &none, today, &zone, &tr).iter().any(|d| d.key == "tasked"), "another task: the date back");
    }

    /// A contract renewing each year with a notice: its last day for notice;
    /// none for one renewing each month, one ended, or one with no notice.
    #[test]
    fn a_contract_s_last_day_for_notice() {
        let today = day("2026-10-26");
        let contract = |id: &str, every: &str, notice: u32| Contract { id: id.into(), title: id.into(), renews: Some(day("2027-01-01")), every: every.into(), notice_days: notice, ..Default::default() };
        let dates = of_contracts(&[contract("insurance", "year", 30), contract("phone", "month", 30), contract("free", "year", 0)], today);
        assert_eq!(dates.iter().map(|d| (d.key.as_str(), d.date)).collect::<Vec<_>>(), [("insurance", day("2026-12-02"))]);
        assert_eq!(dates[0].address, "sioul:contract/insurance");
    }

    /// A paper's last valid day, unless its renewal task is open.
    #[test]
    fn a_paper_s_last_day_gives_way_to_its_renewal() {
        let today = day("2026-10-26");
        let paper = |id: &str, renewal: &str| Paper { id: id.into(), title: id.into(), until: Some(day("2026-12-20")), renewal: renewal.into(), ..Default::default() };
        let tasks = [task("renew", "2026-12-20")];
        let dates = of_papers(&[paper("passport", ""), paper("card", "renew"), paper("old", "gone")], &tasks, today);
        assert_eq!(dates.iter().map(|d| d.key.as_str()).collect::<Vec<_>>(), ["passport", "old"]);
    }

    /// Planned payments by their own line; money in never; an invoice not
    /// paid on its due day, never filled; a paid one, never.
    #[test]
    fn payments_and_invoices() {
        let (today, tr) = (day("2026-10-26"), Translator::new("en"));
        let ledger: crate::budget::Ledger = toml::from_str("[[line]]\nbudget = \"home\"\ndate = 2026-10-26\namount = -650.0\nlabel = \"Rent\"\nplanned = true\n\n[[line]]\nbudget = \"studio\"\ndate = 2026-11-15\namount = 480.0\nlabel = \"Invoice 2026-002\"\nplanned = true\n\n[[line]]\nbudget = \"home\"\ndate = 2026-10-20\namount = -40.0\nlabel = \"Water\"\nplanned = true\n").unwrap();
        let paid = |paid: bool| crate::invoice::Invoice { number: "2026-002".into(), due: "2026-11-15".into(), client: "Fernhill".into(), total_cents: 48_000, paid, ..Default::default() };
        let payments = of_payments(&ledger, today, &tr);
        assert_eq!(payments.iter().map(|d| (d.title.as_str(), d.address.as_str())).collect::<Vec<_>>(), [("Rent · €650", "sioul:budget/home/0")]);
        let invoices = of_invoices(&[paid(false)], today, &tr);
        assert_eq!(invoices.iter().map(|d| d.address.as_str()).collect::<Vec<_>>(), ["sioul:invoice/2026-002"]);
        assert!(!filled(&invoices[0], day("2026-11-14")), "never filled");
        assert!(of_invoices(&[paid(true)], today, &tr).is_empty());
    }

    /// A prescription's end and the day it runs out, from its last refill; not
    /// past its end, not before today.
    #[test]
    fn a_prescription_s_dates() {
        let today = day("2026-10-26");
        let p = crate::health::Prescription { id: "dr".into(), title: "Thyroid".into(), until: Some(day("2027-01-31")), refill_days: Some(30), last_refill: Some(day("2026-10-01")), ..Default::default() };
        let dates = of_prescriptions(&[p.clone()], today);
        assert_eq!(dates.iter().map(|d| (d.kind, d.date)).collect::<Vec<_>>(), [(Kind::RunsOut, day("2026-10-31")), (Kind::PrescriptionEnds, day("2027-01-31"))]);
        assert!(dates.iter().all(|d| d.address.is_empty()), "health: no address");
        let ended = crate::health::Prescription { until: Some(day("2026-10-20")), ..p };
        assert!(of_prescriptions(&[ended], today).is_empty(), "its end gone by, and its medicines past it");
    }

    /// One family of words; the time left, never "late"; filled within seven
    /// days; health named by its kind alone; today and after only.
    #[test]
    fn a_date_s_words_and_mark() {
        let (today, tr) = (day("2026-10-26"), Translator::new("en"));
        let caf = Dated { kind: Kind::Task, key: "caf".into(), address: "sioul:task/caf".into(), title: "CAF form".into(), date: day("2026-10-30"), at: None };
        let shown = item(&caf, today, &tr);
        assert_eq!((shown.title.as_str(), shown.line.as_str(), shown.tone), ("CAF form", "Date asked · four days left", Tone::Plain));
        assert_eq!(item(&Dated { date: day("2026-11-20"), ..caf.clone() }, today, &tr).tone, Tone::Quiet, "outlined beyond seven days");
        assert_eq!(item(&Dated { date: today, ..caf.clone() }, today, &tr).line, "Date asked · today");
        let pills = Dated { kind: Kind::RunsOut, key: "dr".into(), address: String::new(), title: "Levothyrox 75".into(), date: day("2026-10-28"), at: None };
        let shown = item(&pills, today, &tr);
        assert!(!format!("{} {} {}", shown.title, shown.line, shown.address).contains("Levothyrox"), "{shown:?}");
        assert_eq!((shown.title.as_str(), shown.privacy), ("A medicine runs out", Privacy::Health));
        for kind in [Kind::Task, Kind::Letter, Kind::LetterAppointment, Kind::Notice, Kind::Paper, Kind::Payment, Kind::Invoice, Kind::PrescriptionEnds, Kind::RunsOut] {
            let word = tr.text(kind.word(), None);
            assert!(!word.starts_with("date-") && !word.to_lowercase().contains("late") && !word.to_lowercase().contains("overdue"), "{kind:?}: {word}");
        }
        let kept = within(vec![Dated { date: day("2026-10-25"), ..caf.clone() }, caf.clone(), Dated { date: day("2026-11-02"), ..caf.clone() }], day("2026-10-20"), 11, today);
        assert_eq!(kept.iter().map(|d| d.date).collect::<Vec<_>>(), [day("2026-10-30")], "never a day gone by, nor one past the days asked");
        assert_eq!(within(vec![caf.clone()], day("2026-10-26"), 7, today).len(), 1);
    }
}
