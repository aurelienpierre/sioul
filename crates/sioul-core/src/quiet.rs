// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Quiet time. Outside working hours, on days without any and during time
//! off, work rests: nothing administrative or professional comes forward,
//! only family, friends and what you enjoy. Outside every hours set (work,
//! admin, free time: the night, mostly), rest: only the people you marked
//! safe reach you, and the sites for leisure. Two overrides, each until a time,
//! kept in `$XDG_STATE_HOME/sioul/quiet.toml`: working late keeps work in
//! view; done for the day brings quiet early, until work comes back.
//!
//! Detachment from work in the evening is what recovery needs most
//! (Sonnentag & Fritz 2007, 2015); work cues in off-hours keep it from
//! happening. Codes from verified senders still come at once: they are asked for.

use crate::areas::{Area, TaskAreas, Time, Week, in_view};
use crate::config::TimeOff;
use crate::window::{self, AdminWindow};
use jiff::civil::Date;
use jiff::{Span, Zoned};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Why the time is what it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// Within working hours.
    Working,
    /// No working hours are set: never quiet.
    NoHours,
    /// Working past the usual hours, by choice.
    WorkingLate,
    /// Work shown now, by choice, until you say otherwise or the next working day is over.
    WorkNow,
    /// After or before the day's hours.
    Evening,
    /// A day without working hours.
    DayOff,
    /// Holidays, sick leave.
    TimeOff,
    /// Stopped early, by choice.
    DoneForTheDay,
    /// Hours set for your own admin.
    AdminTime,
    /// Hours set for rest and leisure.
    LeisureTime,
}

/// Work time or quiet time, and until when.
#[derive(Debug, Clone, PartialEq)]
pub struct Mode {
    /// Work rests: hours are set, and these are not work's.
    pub quiet: bool,
    /// What the hours now are for (docs/areas.md).
    pub time: Time,
    /// Which kinds of hours your week sets.
    pub week: Week,
    pub reason: Reason,
    /// When it changes: work starts again, or quiet comes.
    pub until: Option<Zoned>,
    /// The time off's word, when it is time off.
    pub label: String,
}

impl Mode {
    /// Rest: hours are set, and none of them is open now (docs/areas.md).
    pub fn rests(&self) -> bool {
        self.time == Time::Personal
    }
}

/// The overrides, each until a time (Unix seconds).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Overrides {
    /// Work stays in view until then.
    #[serde(default)]
    pub work_until: Option<i64>,
    /// Quiet until then: done for the day.
    #[serde(default)]
    pub rest_until: Option<i64>,
    /// "Work now", ticked: work shown until then, the end of the next working
    /// day. Sioul takes it back when it closes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_now: Option<i64>,
    /// When the day was closed: "back to today's plan" stays offered that day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_from: Option<i64>,
    /// The plan's first step when work comes back, in words, and its task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_step_task: Option<String>,
    /// The day it is for; it is not shown after.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_step_day: Option<Date>,
}

impl Overrides {
    /// The first step to show now: on the day it is for, in work time.
    pub fn first_step_on(&self, today: Date) -> Option<(&str, &str)> {
        let step = self.first_step.as_deref().filter(|s| !s.trim().is_empty())?;
        self.first_step_day.is_none_or(|d| d == today).then(|| (step, self.first_step_task.as_deref().unwrap_or("")))
    }

    /// Whether the day was closed today, so today's plan can be taken back.
    pub fn closed_today(&self, now: &Zoned) -> bool {
        self.rest_from.and_then(|t| jiff::Timestamp::from_second(t).ok()).is_some_and(|t| t.to_zoned(now.time_zone().clone()).date() == now.date())
    }
}

impl Overrides {
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("quiet.toml")
    }

    pub fn load(path: &Path) -> Overrides {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        std::fs::write(path, toml::to_string(self).map_err(|e| e.to_string())?).map_err(fail)
    }
}

/// The time off that covers `date`, if any.
pub fn time_off_on(time_off: &[TimeOff], date: Date) -> Option<&TimeOff> {
    time_off.iter().find(|t| t.from <= date && date <= t.until)
}

/// When working hours next open after `now`, past any time off; None without hours.
pub fn next_work(windows: &[AdminWindow], time_off: &[TimeOff], now: &Zoned) -> Option<Zoned> {
    let mut from = now.clone();
    for _ in 0..60 {
        let opening = window::next_opening(windows, &from)?;
        match time_off_on(time_off, opening.date()) {
            None => return Some(opening),
            // Within time off: from the day after it ends.
            Some(off) => {
                let after = off.until.checked_add(Span::new().days(1)).ok()?;
                from = after.to_zoned(now.time_zone().clone()).ok()?.checked_sub(Span::new().seconds(1)).ok()?;
            }
        }
    }
    None
}

/// When work comes back once you are done for today: the next working hours
/// after today, past any time off; without hours, tomorrow at `morning`.
pub fn after_today(windows: &[AdminWindow], time_off: &[TimeOff], now: &Zoned, morning: i8) -> Zoned {
    let Some(tomorrow) = now.date().tomorrow().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()) else {
        return now.clone();
    };
    // `morning` comes from the settings: an hour that is none is midnight.
    let at_morning = jiff::civil::Time::new(morning, 0, 0, 0).ok().and_then(|t| tomorrow.date().to_datetime(t).to_zoned(now.time_zone().clone()).ok());
    next_work(windows, time_off, &tomorrow).unwrap_or_else(|| at_morning.unwrap_or(tomorrow))
}

/// When "Work now" ends: once the next working day is over, today's while
/// its working hours are not; past any time off. Without working hours, at
/// midnight. `work` are the working hours only.
pub fn end_of_next_workday(work: &[AdminWindow], time_off: &[TimeOff], now: &Zoned) -> Zoned {
    let zone = now.time_zone();
    if time_off_on(time_off, now.date()).is_none()
        && let Some((_, closing)) = window::hours_on(work, now.date(), zone).filter(|(_, closing)| closing.timestamp() > now.timestamp())
    {
        return closing;
    }
    if let Some(opening) = next_work(work, time_off, now)
        && let Some((_, closing)) = window::hours_on(work, opening.date(), zone)
    {
        return closing;
    }
    now.date().tomorrow().ok().and_then(|d| d.to_zoned(zone.clone()).ok()).unwrap_or_else(|| now.clone())
}

/// "17:00", "tomorrow at 9:00", "Monday 5 October at 09:00".
pub fn until_text(tr: &crate::i18n::Translator, until: &Zoned, now: &Zoned) -> String {
    let mut args = crate::i18n::args();
    args.set("time", until.strftime("%H:%M").to_string());
    if until.date() == now.date() {
        until.strftime("%H:%M").to_string()
    } else if now.date().tomorrow().is_ok_and(|d| d == until.date()) {
        tr.text("until-tomorrow", Some(&args))
    } else {
        tr.date(until, false)
    }
}

/// Work time or quiet time, now; and what the hours are for: work, your
/// own admin, leisure, or none of them (personal time). `windows` are the
/// week's hours of every kind.
pub fn mode(windows: &[AdminWindow], time_off: &[TimeOff], overrides: &Overrides, now: &Zoned) -> Mode {
    let of = |kind: &str| -> Vec<AdminWindow> { windows.iter().filter(|w| w.kind() == kind).cloned().collect() };
    let (work, admin, leisure) = (of("work"), of("admin"), of("leisure"));
    let week = Week { work_hours: !work.is_empty(), admin_hours: !admin.is_empty(), leisure_hours: !leisure.is_empty() };
    let made = |time: Time, reason: Reason, until: Option<Zoned>, label: String| Mode { quiet: !(time.works() || time == Time::Any), time, week, reason, until, label };
    let at = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(now.time_zone().clone()));
    let stamp = now.timestamp().as_second();
    if let Some(rest) = overrides.rest_until.filter(|r| *r > stamp) {
        return made(Time::Leisure, Reason::DoneForTheDay, at(rest), String::new());
    }
    if let Some(late) = overrides.work_until.filter(|w| *w > stamp) {
        return made(Time::Work, Reason::WorkingLate, at(late), String::new());
    }
    if let Some(until) = overrides.work_now.filter(|w| *w > stamp) {
        return made(Time::Work, Reason::WorkNow, at(until), String::new());
    }
    if windows.is_empty() && time_off.is_empty() {
        return made(Time::Any, Reason::NoHours, None, String::new());
    }
    if let Some(off) = time_off_on(time_off, now.date()) {
        return made(Time::Leisure, Reason::TimeOff, next_work(windows, time_off, now), off.label.clone());
    }
    if windows.is_empty() {
        return made(Time::Any, Reason::NoHours, None, String::new());
    }
    // Every kind of hours open now: several at once bring all they bring, until the first of them closes.
    let (at_work, at_admin, at_leisure) = (window::current(&work, now), window::current(&admin, now), window::current(&leisure, now));
    let open = Area { work: at_work.is_some(), admin: at_admin.is_some(), leisure: at_leisure.is_some() };
    let closing = [&at_work, &at_admin, &at_leisure].into_iter().flatten().map(|(_, c)| c.clone()).min_by_key(Zoned::timestamp);
    match Time::of(open) {
        Time::Work => return made(Time::Work, Reason::Working, closing, String::new()),
        Time::Admin => return made(Time::Admin, Reason::AdminTime, closing, String::new()),
        Time::Leisure => return made(Time::Leisure, Reason::LeisureTime, closing, String::new()),
        several @ Time::Several(_) => {
            let reason = if open.work { Reason::Working } else { Reason::AdminTime };
            return made(several, reason, closing, String::new());
        }
        _ => {}
    }
    let reason = if window::open_day(&work, now.date()) { Reason::Evening } else { Reason::DayOff };
    // Until the next hours of any kind.
    made(Time::Personal, reason, next_work(windows, time_off, now), String::new())
}

/// What the task pages need of the moment: whether work rests, whether
/// offices are open, the days without room for tasks.
pub struct Situation {
    pub mode: Mode,
    pub offices: crate::taskview::Offices,
    /// Time off, and today when you are done for the day.
    pub closed: std::collections::BTreeSet<Date>,
    /// The weekdays offices open, Monday first.
    pub office_days: [bool; 7],
    /// What quiet time keeps of the tasks.
    pub quiet_tasks: QuietTasks,
}

/// What the task pages keep now (docs/areas.md): what the hours are for, by
/// each task's area (its own, else its categories and projects). A call to an
/// office fits work and admin hours, never free time: offices keep business
/// hours. On holidays and once the day is closed, free time: only what is
/// yours to enjoy, and what is yours either way (health). Outside every hours
/// set (evenings, nights, days off), rest: no task at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuietTasks {
    pub time: Time,
    pub week: Week,
    pub areas: TaskAreas,
}

impl QuietTasks {
    /// Whether a task stays in view now.
    pub fn keeps(&self, task: &crate::tasks::Task) -> bool {
        if self.time == Time::Personal {
            return false;
        }
        let area = self.areas.of(task);
        if task.office_hours && area.admin && !area.work {
            return self.time.offices();
        }
        in_view(area, self.time, self.week)
    }
}

impl Situation {
    pub fn now(config: &crate::config::Config, overrides: &Overrides, now: &Zoned, tr: &crate::i18n::Translator, cases: &[crate::cases::Case]) -> Situation {
        let mode = mode(&config.week_hours(), &config.time_off, overrides, now);
        let office = config.office_hours();
        let open = window::current(&office, now).is_some();
        let next = if open { String::new() } else { window::next_opening(&office, now).map(|z| tr.when(&z)).unwrap_or_default() };
        let mut office_days = [false; 7];
        for w in &office {
            if let Some(day) = w.weekday() {
                office_days[day.to_monday_zero_offset() as usize] = true;
            }
        }
        // Time off within the coming year, and today once done for it.
        let today = now.date();
        let horizon = today.checked_add(Span::new().days(400)).unwrap_or(today);
        let mut closed = std::collections::BTreeSet::new();
        for off in &config.time_off {
            let mut day = off.from.max(today);
            while day <= off.until && day <= horizon {
                closed.insert(day);
                let Ok(next) = day.tomorrow() else { break };
                day = next;
            }
        }
        if mode.reason == Reason::DoneForTheDay {
            closed.insert(today);
        }
        let quiet_tasks = QuietTasks { time: mode.time, week: mode.week, areas: TaskAreas::of_config(config, cases) };
        Situation { mode, offices: crate::taskview::Offices { open, next, now: Some(now.clone()) }, closed, office_days, quiet_tasks }
    }

    /// The task filter, by what the hours are for; none when no hours are set.
    pub fn quiet_tasks(&self) -> Option<QuietTasks> {
        (self.mode.time != Time::Any).then(|| self.quiet_tasks.clone())
    }
}

/// Whether mail reaches you in quiet time: codes always, what you sent
/// yourself, and mail from the senders you marked safe; everyone else waits
/// for working hours. Forged mail never gets here: it was set aside before (porch.rs).
pub fn personal_mail(triaged: &crate::porch::Triaged, senders: &crate::porch::Senders) -> bool {
    triaged.lane == crate::porch::Lane::RightNow
        || triaged.reasons.contains(&crate::porch::Reason::FromYourself)
        || (triaged.lane != crate::porch::Lane::SetAside && senders.standing_of(&triaged.card) == crate::porch::Standing::Safe)
}

/// Whether mail comes forward now (docs/areas.md): verified codes and the
/// senders you marked safe always (who may reach you is the other axis);
/// else as its address is for. An address for leisure and something else
/// shows in free time only what your safe senders write: the rest may be admin
/// or work. Outside every hours set, rest: your safe senders and codes alone.
pub fn mail_in_view(triaged: &crate::porch::Triaged, senders: &crate::porch::Senders, account: Area, time: Time, week: Week) -> bool {
    if personal_mail(triaged, senders) {
        return true;
    }
    if time == Time::Personal {
        return false;
    }
    // In free time alone, an address also for admin or work keeps to its safe senders.
    if time == Time::Leisure && (account.admin || account.work) {
        return false;
    }
    in_view(account, time, week)
}

/// Whether a task is yours, outside work: one of its categories is among
/// `personal` (case and accents aside), or one of its cases is marked as yours.
pub fn personal_task(task: &crate::tasks::Task, personal: &[String], personal_cases: &[String]) -> bool {
    let fold = |s: &str| crate::text::fold(s.trim()).into_iter().collect::<String>();
    let wanted: Vec<String> = personal.iter().map(|p| fold(p)).collect();
    task.categories.iter().any(|c| wanted.contains(&fold(c))) || task.cases.iter().any(|c| personal_cases.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    fn week() -> Vec<AdminWindow> {
        ["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: None }).collect()
    }

    #[test]
    fn work_rests_outside_its_hours() {
        let none = Overrides::default();
        // 2 October 2026 is a Friday.
        let friday_noon = at("2026-10-02T12:00[Europe/Paris]");
        let m = mode(&week(), &[], &none, &friday_noon);
        assert_eq!((m.quiet, m.reason.clone()), (false, Reason::Working));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-02T17:00:00");
        let friday_evening = at("2026-10-02T19:00[Europe/Paris]");
        let m = mode(&week(), &[], &none, &friday_evening);
        assert_eq!((m.quiet, m.reason.clone()), (true, Reason::Evening));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-05T09:00:00", "until Monday morning");
        let saturday = at("2026-10-03T11:00[Europe/Paris]");
        assert_eq!(mode(&week(), &[], &none, &saturday).reason, Reason::DayOff);
        // Holidays the next week: quiet until the Monday after.
        let off = vec![TimeOff { from: "2026-10-05".parse().unwrap(), until: "2026-10-09".parse().unwrap(), label: "Holidays".into() }];
        let m = mode(&week(), &off, &none, &saturday);
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-12T09:00:00");
        let m = mode(&week(), &off, &none, &at("2026-10-06T10:00[Europe/Paris]"));
        assert_eq!((m.quiet, m.reason, m.label.as_str()), (true, Reason::TimeOff, "Holidays"));
        // Done for the day at noon; working late in the evening.
        let rest = Overrides { rest_until: Some(at("2026-10-05T09:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        assert_eq!(mode(&week(), &[], &rest, &friday_noon).reason, Reason::DoneForTheDay);
        let late = Overrides { work_until: Some(at("2026-10-02T20:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let m = mode(&week(), &[], &late, &friday_evening);
        assert_eq!((m.quiet, m.reason), (false, Reason::WorkingLate));
        assert_eq!(mode(&[], &[], &none, &saturday).reason, Reason::NoHours);
        // Stopping on Friday at noon: work comes back on Monday; without hours, tomorrow morning.
        assert_eq!(after_today(&week(), &[], &friday_noon, 7).datetime().to_string(), "2026-10-05T09:00:00");
        assert_eq!(after_today(&[], &[], &friday_noon, 7).datetime().to_string(), "2026-10-03T07:00:00");
        assert_eq!(after_today(&[], &[], &friday_noon, 30).datetime().to_string(), "2026-10-03T00:00:00", "a morning hour that is none");
        let tr = crate::i18n::Translator::new("en");
        assert_eq!(until_text(&tr, &at("2026-10-02T17:00[Europe/Paris]"), &friday_noon), "17:00");
        assert_eq!(until_text(&tr, &at("2026-10-03T09:00[Europe/Paris]"), &friday_noon), "tomorrow at 09:00");
        assert_eq!(until_text(&tr, &at("2026-10-05T09:00[Europe/Paris]"), &friday_noon), "Monday 5 October at 09:00");
    }

    #[test]
    fn work_now_until_the_next_working_day_is_over() {
        // Friday 2 October 2026 at 19:00, Saturday at 11:00, Friday at noon.
        let (friday_evening, saturday, friday_noon) = (at("2026-10-02T19:00[Europe/Paris]"), at("2026-10-03T11:00[Europe/Paris]"), at("2026-10-02T12:00[Europe/Paris]"));
        assert_eq!(end_of_next_workday(&week(), &[], &saturday).datetime().to_string(), "2026-10-05T17:00:00", "Monday's end");
        assert_eq!(end_of_next_workday(&week(), &[], &friday_evening).datetime().to_string(), "2026-10-05T17:00:00");
        assert_eq!(end_of_next_workday(&week(), &[], &friday_noon).datetime().to_string(), "2026-10-02T17:00:00", "today's, not over yet");
        // Two spans a day: the day is over at the last one's end.
        let mut split = week();
        split.push(AdminWindow { day: "monday".into(), start: "19:00".into(), end: Some("20:00".into()), minutes: 0, kind: None });
        assert_eq!(end_of_next_workday(&split, &[], &saturday).datetime().to_string(), "2026-10-05T20:00:00");
        // Holidays the next week: the Monday after; no working hours: midnight.
        let off = vec![TimeOff { from: "2026-10-05".parse().unwrap(), until: "2026-10-09".parse().unwrap(), label: "Holidays".into() }];
        assert_eq!(end_of_next_workday(&week(), &off, &saturday).datetime().to_string(), "2026-10-12T17:00:00");
        assert_eq!(end_of_next_workday(&[], &[], &saturday).datetime().to_string(), "2026-10-04T00:00:00");
        // Ticked on Saturday: work shown, as work, until then.
        let now = Overrides { work_now: Some(end_of_next_workday(&week(), &[], &saturday).timestamp().as_second()), ..Overrides::default() };
        let m = mode(&week(), &[], &now, &saturday);
        assert_eq!((m.time, m.reason, m.quiet), (Time::Work, Reason::WorkNow, false));
        assert_eq!(mode(&week(), &[], &now, &at("2026-10-05T17:30[Europe/Paris]")).reason, Reason::Evening, "over with Monday");
    }

    #[test]
    fn admin_within_free_time() {
        let none = Overrides::default();
        let mut hours = week();
        hours.push(AdminWindow { day: "saturday".into(), start: "10:00".into(), end: Some("18:00".into()), minutes: 0, kind: Some("leisure".into()) });
        hours.push(AdminWindow { day: "saturday".into(), start: "14:00".into(), end: Some("16:00".into()), minutes: 0, kind: Some("admin".into()) });
        // Saturday 3 October at 15:00: both are open; until 16:00, when admin's close.
        let m = mode(&hours, &[], &none, &at("2026-10-03T15:00[Europe/Paris]"));
        assert_eq!(m.time, Time::Several(Area::PERSONAL));
        assert!(m.quiet, "work still rests");
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-03T16:00:00");
        assert!(in_view(Area::ADMIN, m.time, m.week) && in_view(Area::LEISURE, m.time, m.week) && !in_view(Area::WORK, m.time, m.week));
        // At 17:00, free time alone.
        assert_eq!(mode(&hours, &[], &none, &at("2026-10-03T17:00[Europe/Paris]")).time, Time::Leisure);
    }

    #[test]
    fn the_first_step_on_its_day() {
        let monday: Date = "2026-10-05".parse().unwrap();
        let overrides = Overrides { first_step: Some("Open the renewal form (15 min)".into()), first_step_task: Some("renewal".into()), first_step_day: Some(monday), ..Overrides::default() };
        assert_eq!(overrides.first_step_on(monday), Some(("Open the renewal form (15 min)", "renewal")));
        assert_eq!(overrides.first_step_on("2026-10-06".parse().unwrap()), None, "never the day after");
        let closed = Overrides { rest_from: Some(at("2026-10-02T12:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        assert!(closed.closed_today(&at("2026-10-02T19:00[Europe/Paris]")));
        assert!(!closed.closed_today(&at("2026-10-03T09:00[Europe/Paris]")), "today's plan is taken back the same day only");
    }

    #[test]
    fn what_is_yours() {
        let task = |cats: &[&str], cases: &[&str]| crate::tasks::Task { categories: cats.iter().map(|s| s.to_string()).collect(), cases: cases.iter().map(|s| s.to_string()).collect(), ..crate::tasks::Task::default() };
        let personal = vec!["Famille".to_string(), "joy".into()];
        assert!(personal_task(&task(&["famille"], &[]), &personal, &[]));
        assert!(personal_task(&task(&[], &["garden"]), &personal, &["garden".into()]));
        assert!(!personal_task(&task(&["you"], &["taxes"]), &personal, &["garden".into()]));
        let areas = TaskAreas { work_categories: vec!["travail".into()], work_cases: vec!["client-x".into()], ..TaskAreas::usual() };
        let week = Week { work_hours: true, admin_hours: false, leisure_hours: false };
        // Outside every hours set (the night), rest: no task at all, yours neither.
        let night = QuietTasks { time: Time::Personal, week, areas: areas.clone() };
        assert!(!night.keeps(&task(&[], &["taxes"])) && !night.keeps(&task(&["joy"], &[])) && !night.keeps(&task(&["Travail"], &[])));
        // On holidays and once the day is closed: free time, only what is yours.
        let holidays = QuietTasks { time: Time::Leisure, ..night.clone() };
        assert!(!holidays.keeps(&task(&[], &["taxes"])) && holidays.keeps(&task(&["joy"], &[])) && holidays.keeps(&task(&["santé"], &[])));
        // Working hours without admin hours of its own: admin comes then, as it always did.
        let working = QuietTasks { time: Time::Work, ..night.clone() };
        assert!(working.keeps(&task(&[], &["taxes"])) && working.keeps(&task(&["Travail"], &[])) && !working.keeps(&task(&["joy"], &[])));
        // Admin with hours of its own: it waits for them; a call to an office still fits the working day.
        let set = QuietTasks { time: Time::Work, week: Week { admin_hours: true, ..week }, areas };
        assert!(!set.keeps(&task(&[], &["taxes"])));
        assert!(set.keeps(&crate::tasks::Task { office_hours: true, ..task(&[], &["taxes"]) }));
    }

    #[test]
    fn hours_for_admin_and_for_rest() {
        let none = Overrides::default();
        let mut hours = week();
        hours.push(AdminWindow { day: "friday".into(), start: "18:00".into(), end: Some("19:00".into()), minutes: 0, kind: Some("admin".into()) });
        hours.push(AdminWindow { day: "saturday".into(), start: "10:00".into(), end: Some("18:00".into()), minutes: 0, kind: Some("leisure".into()) });
        let m = mode(&hours, &[], &none, &at("2026-10-02T18:30[Europe/Paris]"));
        assert_eq!((m.time, m.reason.clone(), m.quiet), (Time::Admin, Reason::AdminTime, true));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-02T19:00:00");
        let m = mode(&hours, &[], &none, &at("2026-10-02T20:00[Europe/Paris]"));
        assert_eq!((m.time, m.reason.clone()), (Time::Personal, Reason::Evening));
        assert_eq!(m.until.unwrap().datetime().to_string(), "2026-10-03T10:00:00", "until Saturday's free time");
        assert_eq!(mode(&hours, &[], &none, &at("2026-10-03T11:00[Europe/Paris]")).time, Time::Leisure);
        assert_eq!(mode(&hours, &[], &none, &at("2026-10-02T10:00[Europe/Paris]")).time, Time::Work);
        assert!(m.week.work_hours && m.week.admin_hours && m.week.leisure_hours);
        assert_eq!(mode(&[], &[], &none, &at("2026-10-02T10:00[Europe/Paris]")).time, Time::Any);
    }
}
