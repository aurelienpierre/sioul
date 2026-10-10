// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Health and well-being, for the window: the page (a day, or its week, at a
//! glance: meals, naps, the night and the doses, each day's own changes; the
//! medicines and the prescriptions), its settings (the usual meals and nights,
//! the pauses), and the minute tick that reminds a dose once, quietly,
//! and turns refills and renewals into tasks in a list your phone has. The
//! rest goes to no server, except sealed to your other computers when you
//! share with them (docs/database.md).

use crate::backend::{QtThread, Shared, json, say, tell, tr};
use crate::work;
use cxx_qt_lib::QString;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use sioul_core::doses::{Answer, Answered, DoseRecords, Given, State};
use sioul_core::health::{ChatLimit, Doubt, ErrandKind, Health, HealthState, Medicine, Movement, Named, Peer, Prescription, Problem, Schedule, Take, TakeProblem};
use sioul_core::i18n::Translator;
use sioul_core::needs::{DayEdit, DayProblem, Days, Kept, Needs};
use sioul_core::tasks::TaskEdit;
use std::sync::Arc;

fn load() -> Health {
    Health::load(&Health::default_path())
}

/// The health file read, changed by `change` and written back, all under its
/// lock, which the sharing takes too: a change is set over the medicines as
/// the file holds them then (`Health::change`), never over a copy read earlier.
fn change_health<R>(change: impl FnOnce(&mut Health) -> Result<R, String>) -> Result<R, String> {
    let out = Health::change(&Health::default_path(), change)?;
    // A medicine changed: a phone's alarms follow (`alarms`).
    crate::alarms::schedule();
    Ok(out)
}

/// "Levothyroxine and Magnesium", "A, B and C".
fn listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), tr().text("word-and", None)),
    }
}

/// "8:00" → "08:00", as the day's list writes its times; what does not read stays as written.
fn hhmm(text: &str) -> String {
    let parsed = text.trim().split_once(':').and_then(|(h, m)| Some((h.trim().parse::<u8>().ok()?, m.trim().parse::<u8>().ok()?)));
    match parsed {
        Some((h, m)) if h < 24 && m < 60 => format!("{h:02}:{m:02}"),
        _ => text.trim().to_string(),
    }
}

/// When a medicine is taken: its times each day, "08:00 · 20:00"; every few
/// days or hours, in words ("every other day at 08:00, from Sunday 4 October").
fn words(schedule: &Schedule) -> String {
    match schedule {
        Schedule::Day { times, .. } => {
            let mut times: Vec<String> = times.iter().map(|t| hhmm(t)).collect();
            times.sort();
            times.dedup();
            times.join(" · ")
        }
        Schedule::Days { days, time, from } => {
            let mut args = sioul_core::i18n::args();
            args.set("days", *days);
            args.set("time", time.clone());
            args.set("from", tr().day(*from));
            tr().text("health-every-days", Some(&args))
        }
        Schedule::Hours { hours, from } => {
            // Said by its next dose: each dose taken sets the next.
            let now = Zoned::now();
            let next = schedule.doses(&now, &now.checked_add(Span::new().hours(i64::from(*hours))).unwrap_or_else(|_| now.clone())).into_iter().next();
            let next = next.map_or(*from, |z| z.timestamp().as_second());
            let next = Timestamp::from_second(next).map(|t| tr().date(&t.to_zoned(jiff::tz::TimeZone::system()), false)).unwrap_or_default();
            let mut args = sioul_core::i18n::args();
            args.set("hours", *hours);
            args.set("next", next);
            tr().text("health-every-hours", Some(&args))
        }
    }
}

#[derive(Serialize)]
struct DoseRow {
    key: String,
    time: String,
    name: String,
    dose: String,
    /// "12:04" when marked taken; "" otherwise.
    taken: String,
    past: bool,
    /// Past its time by more than half an hour, not marked: marked now, it
    /// asks when it was taken (`DoseTaken.qml`).
    late: bool,
    /// Not marked here, but whether it was taken is not known here: why, in
    /// a sentence; "" when it is known (docs/health.md, "Knowing"). Answers
    /// that differ on your devices: both, said (`choose`).
    doubt: String,
    /// A "This device is off" for each device the doubt names (`device_off`).
    doubt_off: Vec<OffButton>,
    /// Answers that differ, said in `doubt`: "Taken" and "Not taken" settle it (`choose`).
    choose: bool,
    /// For a dose due earlier and answered nowhere (`missed_rows`), the
    /// question it is asked under, shown above the first of its kind: due
    /// while Sioul was closed, or due while it ran but its reminder could not
    /// be shown. "" for today's doses on the Porch.
    question: String,
}

/// "This device is off", under a doubt naming it: the device's id, the button's words.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
struct OffButton {
    id: String,
    label: String,
}

/// A medicine as the page lists it, and as its form reads it.
#[derive(Serialize)]
struct MedicineRow {
    #[serde(flatten)]
    medicine: Medicine,
    /// Its times, "08:00 · 20:00"; each take with its amount when they
    /// differ, "08:00 · 1 tablet, 20:00 · 2 tablets"; every few days or
    /// hours, in words.
    when: String,
    /// What the page says beside its name: its dose; "" when each take says its own.
    amount: String,
    /// Its name, generic name and strength: "Thyrolan — levothyroxine 75 µg" (`Medicine::precise`).
    precise: String,
    /// All of it in a line, for a prescription's entry: "Thyrolan — levothyroxine 75 µg · 07:30 · 1 tablet".
    line: String,
    /// Its form's dose: the amount most takes have (`Medicine::usual`).
    usual: String,
    /// Its takes at set times each day, in time order, for its form: each
    /// take's own amount only when it differs from `usual` ("" otherwise).
    takes: Vec<Take>,
    /// "until Monday 26 October"; "" for as long as it goes.
    ends: String,
    /// Paused, or past its last day: listed all the same, quieter.
    quiet: bool,
}

/// What a medicine says of its takes: each with its amount when they differ
/// ("08:00 · 1 tablet, 20:00 · 2 tablets"), else its times, or its schedule in words.
fn when_of(medicine: &Medicine) -> String {
    match &medicine.schedule {
        Schedule::Day { amounts, .. } if !amounts.is_empty() => medicine.takes_line(),
        schedule => words(schedule),
    }
}

/// A medicine as the page and its forms read it.
fn medicine_row(m: &Medicine, today: Date) -> MedicineRow {
    let usual = m.usual();
    let each = matches!(&m.schedule, Schedule::Day { amounts, .. } if !amounts.is_empty());
    let when = when_of(m);
    let amount = if each { String::new() } else { m.dose.clone() };
    MedicineRow {
        precise: m.precise(),
        line: [m.precise(), when.clone(), amount.clone()].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" · "),
        when,
        amount,
        takes: m.takes().into_iter().map(|t| Take { amount: if t.amount == usual { String::new() } else { t.amount }, time: t.time }).collect(),
        usual,
        ends: m.until.map(|day| say("health-until", &[("day", tr().day_in(day, today))])).unwrap_or_default(),
        quiet: m.paused || m.until.is_some_and(|day| day < today),
        medicine: m.clone(),
    }
}

/// A prescription as the page lists it, and as its form reads it.
#[derive(Serialize)]
struct PrescriptionRow {
    #[serde(flatten)]
    prescription: Prescription,
    /// What comes, in words, the soonest first: "pharmacy from Tuesday 27
    /// October", "renew by Thursday 4 February 2027" (its last valid day).
    next: Vec<String>,
    /// The medicines that come with it, unless its title names them all:
    /// "for Levothyroxine and Magnesium"; "" when none or named, or when
    /// `lines` says them.
    covers: String,
    /// Its medicines precisely, a line each, when one has a generic name or
    /// a strength: "Thyrolan — levothyroxine 75 µg · 07:30 · 1 tablet".
    lines: Vec<String>,
    /// Fetched at the pharmacy today: its button says so.
    fetched_today: bool,
    /// The medicines tied to it, in the order they were added: its form's rows.
    medicines: Vec<MedicineRow>,
}

/// The medicines, in the order they were added (a row keeps its place).
fn medicine_rows(health: &Health, today: Date) -> Vec<MedicineRow> {
    health.medicines.iter().map(|m| medicine_row(m, today)).collect()
}

/// What comes for a prescription, on a day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Coming {
    /// Its medicines fetched at the pharmacy, from that day.
    Pharmacy,
    /// Renewed by that day, its last valid day.
    RenewBy,
    /// Valid until that day, now passed: said plainly, never as late.
    ValidUntil,
}

/// What comes for a prescription, the soonest first: the next visit to the
/// pharmacy (a month back at most), and the day to renew it by.
fn coming(errands: &[sioul_core::health::Errand], p: &Prescription, today: Date) -> Vec<(Coming, Date)> {
    let month_ago = today.checked_sub(Span::new().days(30)).unwrap_or(today);
    let mut next: Vec<(Coming, Date)> = errands.iter().filter(|e| e.prescription == p.id && e.kind == ErrandKind::Refill && e.day >= month_ago).map(|e| (Coming::Pharmacy, e.day)).collect();
    if let Some(until) = p.until {
        next.push((if until < today { Coming::ValidUntil } else { Coming::RenewBy }, until));
    }
    next.sort_by_key(|(_, day)| *day);
    next
}

/// The medicines tied to a prescription (`Medicine::prescription`), in the order they were added.
fn tied<'a>(health: &'a Health, p: &'a Prescription) -> impl Iterator<Item = &'a Medicine> {
    health.medicines.iter().filter(|m| m.prescription.as_deref() == Some(p.id.as_str()))
}

/// The medicines tied to a prescription, unless its title already names
/// them all ("Levothyroxine 75 µg" for Levothyroxine).
fn covered(health: &Health, p: &Prescription) -> Vec<String> {
    let named: Vec<String> = tied(health, p).map(|m| m.name.clone()).collect();
    let title = p.title.to_lowercase();
    if named.iter().all(|name| title.contains(&name.to_lowercase())) { Vec::new() } else { named }
}

/// The prescriptions, in the order they were added, as the page lists them.
fn prescription_rows(health: &Health, today: Date) -> Vec<PrescriptionRow> {
    let errands = health.errands();
    health
        .prescriptions
        .iter()
        .map(|p| {
            let precise = tied(health, p).any(|m| !m.generic.trim().is_empty() || !m.strength.trim().is_empty());
            let lines = if precise { tied(health, p).map(|m| medicine_row(m, today).line).collect() } else { Vec::new() };
            let covers = if precise { Vec::new() } else { covered(health, p) };
            PrescriptionRow {
                lines,
                next: coming(&errands, p, today)
                    .into_iter()
                    .map(|(what, day)| {
                        let day = [("day", tr().day_in(day, today))];
                        match what {
                            Coming::Pharmacy => say("health-next-refill", &day),
                            Coming::RenewBy => say("health-renew-by", &day),
                            Coming::ValidUntil => say("health-valid-until", &day),
                        }
                    })
                    .collect(),
                covers: if covers.is_empty() { String::new() } else { say("health-covers", &[("names", listed(&covers))]) },
                fetched_today: p.last_refill == Some(today),
                medicines: tied(health, p).map(|m| medicine_row(m, today)).collect(),
                prescription: p.clone(),
            }
        })
        .collect()
}

/// The page: a week of days, the day shown one of them (`week`), and what
/// only today says.
#[derive(Serialize)]
struct PageView {
    week: WeekView,
    /// Doses neither marked nor reminded: due while Sioul ran nowhere, or
    /// while it ran but could not show their reminder; a question on the past.
    missed: Vec<DoseRow>,
    /// Why a dose marked elsewhere may not show here: this computer alone, or
    /// your other computers not heard from lately; "" when all is known.
    shared_note: String,
    /// Reminders come on another computer, the one you are at: said, by its name.
    reminded_there: String,
    /// The minutes "Later" moves a block by.
    later: u32,
    /// Anything set at all (meals, naps, the night, a medicine): else the page says where to set them.
    any: bool,
    /// The medicines and the prescriptions: content of the page, changed there (their forms read them).
    medicines: Vec<MedicineRow>,
    prescriptions: Vec<PrescriptionRow>,
}

/// The page's settings (⚙), what is set once: the pauses, where the errands
/// go (the usual meals and night: `needs_page`).
#[derive(Serialize)]
struct SettingsView {
    movement: Movement,
    chats: ChatLimit,
    /// Where the errands go, and the lists they can go to: {id, name}.
    errands_list: String,
    lists: Vec<serde_json::Value>,
}

/// Monday to Sunday, the hours the timeline shows (minutes from midnight,
/// whole hours, the same for every day: the lines stay put from day to day).
#[derive(Serialize)]
struct WeekView {
    monday: String,
    today: String,
    from_minute: i64,
    to_minute: i64,
    days: Vec<DayView>,
}

/// One day: its list and what its column of the timeline draws.
#[derive(Serialize)]
struct DayView {
    /// "2026-10-07".
    date: String,
    /// "Today, Wednesday 7 October", "Tomorrow, …", "Thursday 8 October".
    title: String,
    /// "Wed", and its number: a week's column.
    weekday: String,
    number: i8,
    today: bool,
    /// Before today: shown, never changed.
    past: bool,
    /// The list, in time order: the day's meals, naps and night (those taken
    /// out that day too, to put back), those added that day, its doses.
    items: Vec<DayItem>,
    /// What the timeline draws in this day's column, in minutes from its midnight.
    segments: Vec<Segment>,
    /// Its events and today's planned steps, faded, for context.
    context: Vec<ContextItem>,
    /// Events of the whole day; refills and renewals falling on it.
    lines: Vec<String>,
}

/// A row of a day's list: a meal, a nap, the night, or a dose.
#[derive(Serialize)]
struct DayItem {
    /// "meal:1", "nap:0", "sleep", "added-…", or a dose's key.
    key: String,
    /// "meal", "nap", "sleep", "dose".
    kind: &'static str,
    name: String,
    /// A dose's amount ("75 µg").
    dose: String,
    /// Kept from, to ("12:10", "13:00"): getting it ready, winding down,
    /// coming back included, as the notices say; a dose: its time.
    from: String,
    to: String,
    /// When it starts proper: eating, the nap, bed.
    at: String,
    /// Said under it: "eating from 12:30", "winding down, bed at 23:00".
    detail: String,
    /// How that day differs: "changed for this day", "this day only", "moved
    /// after an event", "no notice that day…", "removed from this day"; "" as usual.
    note: String,
    /// Changed that day (its times), added that day, quiet, taken out that day.
    changed: bool,
    added: bool,
    quiet: bool,
    off: bool,
    /// Over (or on a past day): shown, never changed.
    past: bool,
    /// Its lengths that day, for its form: minutes, getting ready (winding down), coming back.
    minutes: u32,
    before: u32,
    after: u32,
    /// A dose of today: "12:04" when marked taken; late past half an hour;
    /// the doubt when another device may know (never "not taken").
    taken: String,
    late: bool,
    doubt: String,
    /// "This device is off" for each device the doubt names; answers that differ (`DoseRow`).
    doubt_off: Vec<OffButton>,
    choose: bool,
    /// The night: when its alarm rings, the morning it ends ("07:00"; "" when
    /// none would), and whether "No alarm" was asked for that night (`wake`).
    alarm: String,
    alarm_skipped: bool,
    /// Its alarm not rung yet: "No alarm" can still be asked, even after
    /// midnight, on the night going on (a past day's row, its menu that item only).
    alarm_open: bool,
}

/// A part of the timeline's column: from, to, in minutes from the column's
/// midnight (a night split at midnight, its morning part the day before's).
#[derive(Serialize)]
struct Segment {
    /// The day it belongs to, and its key: what the list's row is.
    date: String,
    key: String,
    /// "meal", "nap", "sleep", "dose".
    kind: &'static str,
    name: String,
    from_minute: i64,
    to_minute: i64,
    /// Its whole span as kept, not cut at the column's midnights (before 0, past
    /// 1440): a drag knows which of its ends this column holds, and how long it is.
    start_minute: i64,
    end_minute: i64,
    /// Where it starts and ends proper: before and after, lighter (getting ready, winding down, coming back).
    at_minute: i64,
    until_minute: i64,
    quiet: bool,
    past: bool,
    /// A dose of today: "taken", "due", "check" (another device may know); "" another day.
    state: &'static str,
}

/// An event or a planned step, for context only.
#[derive(Serialize)]
struct ContextItem {
    title: String,
    /// "event", "task".
    kind: &'static str,
    from_minute: i64,
    to_minute: i64,
}

/// The list the errands go to: the one chosen, else your usual list, else the
/// first list on a server (so the phone has them), else one on this computer.
fn errands_list(health: &Health) -> Option<String> {
    let lists: Vec<sioul_core::vdir::Collection> = sioul_core::tasks::lists().into_iter().filter(|c| !c.read_only).collect();
    let id = |c: &sioul_core::vdir::Collection| format!("{}/{}", c.account, c.id);
    let usual = crate::backend::load_config().tasks.list.clone().unwrap_or_default();
    [health.errands_list.clone(), usual]
        .into_iter()
        .find(|wanted| !wanted.is_empty() && lists.iter().any(|c| id(c) == *wanted))
        .or_else(|| lists.iter().find(|c| c.account != sioul_core::vdir::LOCAL).map(id))
        .or_else(|| lists.first().map(id))
}

/// The day the page shows (`show_week`): its week is made. None: today.
static SHOWN: std::sync::Mutex<Option<Date>> = std::sync::Mutex::new(None);

/// The page shows the week of `day` ("2026-10-07"); one that does not read: today's.
pub(crate) fn show_week(day: &str) {
    if let Ok(mut shown) = SHOWN.lock() {
        *shown = day.trim().parse().ok();
    }
}

/// `at` (Unix seconds) on the clock in `zone`: "12:30".
fn clock(at: i64, zone: &TimeZone) -> String {
    Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default()
}

/// Midnight of `date` in `zone`, Unix seconds.
fn midnight(date: Date, zone: &TimeZone) -> i64 {
    date.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second())
}

/// "Wednesday": a word's first letter in capitals, as a title starts.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The health page, as JSON: the week of the day it shows, and what only today says.
pub(crate) fn page() -> String {
    let health = load();
    let state = record();
    let knowledge = know();
    let now = Zoned::now();
    let shown = SHOWN.lock().ok().and_then(|s| *s).unwrap_or(now.date());
    let records = records();
    let week = week_view(&health, &state, &records, &knowledge, shown, &now);
    let needs = &health.needs;
    json(&PageView {
        any: needs.meals_on || needs.naps_on || needs.sleep_on || !health.medicines.is_empty() || week.days.iter().any(|d| !d.items.is_empty()),
        week,
        missed: missed_rows(&health, &state, &knowledge, &now, tr()),
        shared_note: if health.medicines.is_empty() { String::new() } else { shared_note(&knowledge) },
        reminded_there: [
            REMINDED_THERE.lock().map(|r| r.clone()).unwrap_or_default(),
            if health.medicines.is_empty() || crate::alarms::notifications_allowed() { String::new() } else { tr().text("dose-notifications-off", None) },
            if health.medicines.is_empty() || crate::alarms::exact() { String::new() } else { tr().text("dose-alarms-inexact", None) },
        ]
        .into_iter()
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" "),
        later: needs.later,
        medicines: medicine_rows(&health, now.date()),
        prescriptions: prescription_rows(&health, now.date()),
    })
}

/// The page's settings (⚙), as JSON.
pub(crate) fn settings_view() -> String {
    let health = load();
    let config = crate::backend::load_config();
    json(&SettingsView {
        movement: health.movement.clone(),
        chats: health.chats.clone(),
        errands_list: errands_list(&health).unwrap_or_default(),
        lists: sioul_core::tasks::lists().into_iter().filter(|c| !c.read_only).map(|c| serde_json::json!({ "id": format!("{}/{}", c.account, c.id), "name": c.label(&config, tr()), "local": c.account == sioul_core::vdir::LOCAL })).collect(),
    })
}

/// The events of the week from `monday`, read again five minutes after at most:
/// the page is made again each minute while it is open.
fn week_events(monday: Date, zone: &TimeZone) -> Arc<Vec<sioul_core::agenda::Occurrence>> {
    type Cached = Option<(i64, Date, Arc<Vec<sioul_core::agenda::Occurrence>>)>;
    static CACHE: std::sync::Mutex<Cached> = std::sync::Mutex::new(None);
    let stamp = Timestamp::now().as_second();
    let mut cache = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((at, day, events)) = cache.as_ref()
        && *day == monday
        && (0..300).contains(&(stamp - at))
    {
        return Arc::clone(events);
    }
    let from = midnight(monday, zone);
    let to = monday.checked_add(Span::new().days(7)).map_or(from + 7 * 86_400, |d| midnight(d, zone));
    let events = Arc::new(sioul_core::agenda::occurrences(from, to));
    *cache = Some((stamp, monday, Arc::clone(&events)));
    events
}

/// Which day a night is: the evening it starts, or, its bedtime after
/// midnight, the evening before (`Needs::kept_with`'s rule).
fn night_of(kept: &Kept, zone: &TimeZone) -> Option<Date> {
    let bed = Timestamp::from_second(kept.at).ok()?.to_zoned(zone.clone());
    if bed.hour() < 12 { bed.date().yesterday().ok() } else { Some(bed.date()) }
}

/// The week of `day`, Monday to Sunday: each day's list and timeline, and
/// the hours the timeline shows (the same each day).
fn week_view(health: &Health, state: &HealthState, records: &DoseRecords, knowledge: &Knowledge, day: Date, now: &Zoned) -> WeekView {
    let zone = now.time_zone().clone();
    let monday = day.checked_sub(Span::new().days(i64::from(day.weekday().to_monday_zero_offset()))).unwrap_or(day);
    let days = days();
    let events = week_events(monday, &zone);
    // Meals move past the events with their margins, as the plan has them.
    let held = sioul_core::plan::event_spans(&events, 0);
    let steps = crate::work::day_steps();
    let days: Vec<DayView> = (0..7).filter_map(|n| monday.checked_add(Span::new().days(n)).ok()).map(|date| day_view(health, state, records, knowledge, &days, &Around { events: &events, held: &held, steps: &steps }, date, now)).collect();
    // From half an hour before the first thing (waking, the first block or
    // event), to half an hour after the last (bedtime, the last one), whole hours.
    let (mut first, mut last) = (i64::MAX, i64::MIN);
    for day in &days {
        for s in &day.segments {
            match s.kind {
                "sleep" if s.from_minute <= 0 => first = first.min(s.to_minute),
                "sleep" => last = last.max(s.at_minute.min(24 * 60)),
                _ => {
                    first = first.min(s.from_minute);
                    last = last.max(s.to_minute);
                }
            }
        }
        for c in &day.context {
            first = first.min(c.from_minute);
            last = last.max(c.to_minute);
        }
    }
    let first = if first == i64::MAX { 7 * 60 } else { first };
    let last = if last == i64::MIN { 22 * 60 } else { last };
    let from = (first - 30).clamp(0, 23 * 60) / 60 * 60;
    let to = ((last + 30 + 59) / 60 * 60).clamp(from + 6 * 60, 24 * 60);
    WeekView { monday: monday.to_string(), today: now.date().to_string(), from_minute: from.min(to - 60), to_minute: to, days }
}

/// What lies around the days: the week's events, their times with margins, today's planned steps.
struct Around<'a> {
    events: &'a [sioul_core::agenda::Occurrence],
    held: &'a [(i64, i64)],
    steps: &'a [(i64, i64, String)],
}

/// One day: its list (blocks, those taken out that day, its doses) and what its column draws.
fn day_view(health: &Health, state: &HealthState, records: &DoseRecords, knowledge: &Knowledge, days: &Days, around: &Around, date: Date, now: &Zoned) -> DayView {
    let zone = now.time_zone().clone();
    let today = now.date();
    let stamp = now.timestamp().as_second();
    let start = midnight(date, &zone);
    let next = date.tomorrow().map_or(start + 86_400, |d| midnight(d, &zone));
    // Minutes from this day's midnight on the clock (a time the next day: past 1440).
    let minute = |at: i64| -> i64 {
        let Ok(t) = Timestamp::from_second(at) else { return 0 };
        let z = t.to_zoned(zone.clone());
        let after = date.until(z.date()).map_or(0, |s| i64::from(s.get_days()));
        after * 24 * 60 + i64::from(z.hour()) * 60 + i64::from(z.minute())
    };
    let needs = &health.needs;
    let pushed = needs.past_events_on(date, &zone, days, around.held);
    let shift = |key: &str| pushed.get(key).copied().unwrap_or(0);
    let past_day = date < today;
    let mut items: Vec<(i64, DayItem)> = Vec::new();
    let mut segments: Vec<Segment> = Vec::new();
    let block_item = |k: &Kept, off: bool| -> DayItem {
        let change = days.get(date, &k.key);
        let added = sioul_core::needs::is_added(&k.key);
        let (minutes, before, after) = needs.lengths(date, &k.key, days).unwrap_or((0, 0, 0));
        let detail = match k.kind {
            "meal" if before > 0 => say("need-meal-detail", &[("at", clock(k.at, &zone))]),
            "nap" if after > 0 => say("need-nap-detail", &[("minutes", after.to_string())]),
            "sleep" => say("need-night-detail", &[("bed", clock(k.at, &zone))]),
            _ => String::new(),
        };
        // The night's alarm at waking, said quietly; "No alarm" asked, said as how that day differs.
        let (alarm, alarm_skipped) = if k.kind == "sleep" && !off { crate::wake::of_night(needs, date, &zone, days) } else { (String::new(), false) };
        let alarm_open = !alarm.is_empty() && needs.alarm_of(date, &zone, days).is_some_and(|a| a.at > stamp);
        let detail = if alarm.is_empty() || alarm_skipped { detail } else { format!("{detail}  ·  {}", say("wake-at", &[("time", alarm.clone())])) };
        let changed = !added && change.is_some_and(|c| !c.at.is_empty() || !c.wake.is_empty() || c.minutes.is_some() || c.before.is_some() || c.after.is_some());
        let quiet = change.is_some_and(|c| c.quiet);
        let note = if off {
            tr().text("need-off", None)
        } else if added {
            tr().text("need-added", None)
        } else if changed {
            tr().text("need-changed", None)
        } else if shift(&k.key) > 0 {
            tr().text("need-pushed", None)
        } else {
            String::new()
        };
        let note = if quiet && !off { [note, tr().text("need-quiet", None)].into_iter().filter(|n| !n.is_empty()).collect::<Vec<_>>().join(" · ") } else { note };
        let note = if alarm_skipped { [note, tr().text("wake-none", None)].into_iter().filter(|n| !n.is_empty()).collect::<Vec<_>>().join(" · ") } else { note };
        DayItem {
            key: k.key.clone(),
            kind: k.kind,
            name: if k.kind == "sleep" { tr().text("needs-sleep", None) } else { block_name(k.kind, &k.key, &k.name) },
            dose: String::new(),
            from: clock(k.start, &zone),
            to: clock(k.end, &zone),
            at: clock(k.at, &zone),
            detail,
            note,
            changed,
            added,
            quiet,
            off,
            past: past_day || k.end <= stamp,
            minutes,
            before,
            after,
            taken: String::new(),
            late: false,
            doubt: String::new(),
            doubt_off: Vec::new(),
            choose: false,
            alarm,
            alarm_skipped,
            alarm_open,
        }
    };
    for k in needs.blocks_of(date, &zone, days, &shift) {
        let item = block_item(&k, false);
        // Before and after it proper, lighter: getting it ready, winding down, coming back.
        let until = if k.kind == "sleep" { k.end } else { k.at + i64::from(item.minutes) * 60 };
        if minute(k.start) < 24 * 60 {
            segments.push(Segment { date: date.to_string(), key: k.key.clone(), kind: k.kind, name: item.name.clone(), from_minute: minute(k.start).max(0), to_minute: minute(k.end).min(24 * 60), start_minute: minute(k.start), end_minute: minute(k.end), at_minute: minute(k.at), until_minute: minute(until), quiet: item.quiet, past: item.past, state: "" });
        }
        items.push((minute(k.start), item));
    }
    // Taken out that day: in the list still, to put back; its time is free.
    let usual = needs.blocks_of(date, &zone, &Days::default(), &|_| 0);
    for k in usual.iter().filter(|k| days.get(date, &k.key).is_some_and(|c| c.off)) {
        items.push((minute(k.start), block_item(k, true)));
    }
    // The night ending this morning is the day before's: its morning part.
    for k in needs.kept_with(date, &zone, days, &shift).iter().filter(|k| k.kind == "sleep") {
        let Some(night) = night_of(k, &zone).filter(|night| *night != date) else { continue };
        segments.push(Segment {
            date: night.to_string(),
            key: k.key.clone(),
            kind: "sleep",
            name: tr().text("needs-sleep", None),
            from_minute: minute(k.start).max(0),
            to_minute: minute(k.end).min(24 * 60),
            start_minute: minute(k.start),
            end_minute: minute(k.end),
            at_minute: minute(k.at),
            until_minute: minute(k.end),
            quiet: days.get(night, "sleep").is_some_and(|c| c.quiet),
            past: night < today || k.end <= stamp,
            state: "",
        });
    }
    // The doses: on today with whether they are marked, and the doubt when
    // another device may know; another day, plainly (no record of the past here).
    let (from_z, to_z) = (Timestamp::from_second(start).map(|t| t.to_zoned(zone.clone())), Timestamp::from_second(next).map(|t| t.to_zoned(zone.clone())));
    if let (Ok(from_z), Ok(to_z)) = (from_z, to_z) {
        let mut doses: Vec<(String, i64, String, String)> = health.doses(&from_z, &to_z).into_iter().map(|d| (d.key, d.at.timestamp().as_second(), d.name, d.dose)).collect();
        // Taken today, at a time the schedule no longer has: a dose taken late
        // or early moved the next ones (a medicine counted from its last dose).
        if date == today {
            for key in state.taken.keys() {
                let Some((id, due)) = key.rsplit_once('@') else { continue };
                let (Some(medicine), Ok(due)) = (health.medicines.iter().find(|m| m.id == id), due.parse::<i64>()) else { continue };
                if due >= start && due < next && !doses.iter().any(|d| &d.0 == key) {
                    doses.push((key.clone(), due, medicine.short_name(), health.amount_of(key, &zone).unwrap_or_default()));
                }
            }
        }
        for (key, due, name, dose) in doses {
            // Today, as your devices answered it: taken (the earliest answer),
            // or answers that differ, both said; else the doubt when another
            // device may hold an answer (never "not taken").
            let answered = if date == today { sioul_core::doses::answered(&key, state, records) } else { Answered::Open };
            let (taken, marked) = match &answered {
                Answered::Taken(given) => (clock(given.at, &zone), true),
                _ => (String::new(), false),
            };
            let (doubt, doubt_off) = match &answered {
                Answered::Differ(given) => (differ_sentence(given, &this_device(), tr()), Vec::new()),
                _ if date != today || marked || due > stamp => (String::new(), Vec::new()),
                _ => doubt_now_with(knowledge, due, stamp, tr()),
            };
            let item = DayItem {
                taken,
                choose: matches!(answered, Answered::Differ(_)),
                doubt_off,
                late: date == today && !marked && stamp - due > GRACE_MINUTES * 60,
                key: key.clone(),
                kind: "dose",
                name: name.clone(),
                dose,
                from: clock(due, &zone),
                to: String::new(),
                at: clock(due, &zone),
                detail: String::new(),
                note: String::new(),
                changed: false,
                added: false,
                quiet: false,
                off: false,
                past: past_day || due <= stamp,
                minutes: 0,
                before: 0,
                after: 0,
                doubt,
                alarm: String::new(),
                alarm_skipped: false,
                alarm_open: false,
            };
            let state = if date != today { "" } else if marked { "taken" } else if item.doubt.is_empty() { "due" } else { "check" };
            segments.push(Segment { date: date.to_string(), key, kind: "dose", name, from_minute: minute(due), to_minute: minute(due), start_minute: minute(due), end_minute: minute(due), at_minute: minute(due), until_minute: minute(due), quiet: false, past: item.past, state });
            items.push((minute(due), item));
        }
    }
    items.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| (a.1.kind == "dose").cmp(&(b.1.kind == "dose"))));
    // The day's events, and today's planned steps, for context; whole days and errands in words.
    let mut context = Vec::new();
    let mut lines = Vec::new();
    for event in around.events.iter().filter(|e| !e.cancelled && e.end > start && e.start < next) {
        if event.all_day {
            lines.push(say("day-all-day", &[("what", event.summary.clone())]));
        } else {
            context.push(ContextItem { title: event.summary.clone(), kind: "event", from_minute: minute(event.start).max(0), to_minute: minute(event.end.max(event.start + 15 * 60)).min(24 * 60) });
        }
    }
    if date == today {
        for (from, to, title) in around.steps.iter().filter(|(from, to, _)| *to > start && *from < next) {
            context.push(ContextItem { title: title.clone(), kind: "task", from_minute: minute(*from).max(0), to_minute: minute(*to).min(24 * 60) });
        }
    }
    context.sort_by_key(|c| c.from_minute);
    for errand in health.errands().into_iter().filter(|e| e.day == date) {
        let what = say(if errand.kind == ErrandKind::Refill { "health-errand-refill" } else { "health-errand-renew" }, &[("title", errand.title.clone())]);
        lines.push(say("health-errand-day", &[("what", what)]));
    }
    let named = tr().day(date);
    let title = if date == today {
        format!("{}, {named}", tr().text("agenda-today", None))
    } else if today.tomorrow().is_ok_and(|t| t == date) {
        format!("{}, {named}", tr().text("agenda-tomorrow", None))
    } else {
        capitalized(&named)
    };
    DayView {
        date: date.to_string(),
        title,
        weekday: tr().weekday_short(date),
        number: date.day(),
        today: date == today,
        past: past_day,
        items: items.into_iter().map(|(_, item)| item).collect(),
        segments,
        context,
        lines,
    }
}

/// Doses neither marked nor reminded: a question on the past, each saying
/// when whether it was taken is not known here. Two questions, each its own
/// heading: those due while Sioul ran nowhere, then those due while it ran
/// but whose reminder could not be shown (no notification server: never
/// said "while Sioul was closed"); by their time within each.
fn missed_rows(health: &Health, state: &HealthState, knowledge: &Knowledge, now: &Zoned, words: &Translator) -> Vec<DoseRow> {
    let mut rows: Vec<DoseRow> = state
        .unanswered(health, now, MISSED_HOURS, GRACE_MINUTES)
        .into_iter()
        .map(|d| {
            let (doubt, doubt_off) = doubt_now_with(knowledge, d.at.timestamp().as_second(), now.timestamp().as_second(), words);
            DoseRow {
                taken: String::new(),
                past: true,
                late: true,
                doubt,
                doubt_off,
                choose: false,
                question: if state.reminder_unshown(&d.key) { words.text("health-unshown-question", None) } else { words.text("health-missed-question", None) },
                time: if d.at.date() == now.date() { d.at.strftime("%H:%M").to_string() } else { format!("{} {}", words.weekday_short(d.at.date()), d.at.strftime("%H:%M")) },
                key: d.key,
                name: d.name,
                dose: d.dose,
            }
        })
        .collect();
    rows.sort_by_key(|row| state.reminder_unshown(&row.key));
    rows
}

/// The notification's title for the question on doses neither marked nor
/// reminded (`missed`): "While Sioul was closed" only when it was; "A
/// reminder could not be shown" when Sioul ran; both kinds, a title true of
/// both.
fn missed_title(missed: &[sioul_core::health::Dose], state: &HealthState, words: &Translator) -> String {
    let unshown = missed.iter().filter(|d| state.reminder_unshown(&d.key)).count();
    match unshown {
        0 => words.text("health-missed", None),
        n if n == missed.len() => words.text("health-unshown", None),
        _ => words.text("health-missed-some", None),
    }
}

/// What a minute of `tick` did, written in the doses' record: the reminders
/// shown, those that could not be (`unshown`, so that the question on the
/// past says Sioul ran), and the errands' tasks made.
fn note_minute(record: &mut HealthState, reminded: &[String], unshown: &[String], made: &[(String, String)], stamp: i64) {
    for key in reminded {
        record.reminded.insert(key.clone(), stamp);
    }
    for key in unshown {
        record.unshown.insert(key.clone(), stamp);
    }
    for (key, uid) in made {
        record.errands.insert(key.clone(), uid.clone());
    }
}

/// The Porch's doses (docs/health.md, "On the Porch"). `due`: today's, from
/// their time on, not marked yet, reminded or not (a notification can go
/// unseen), each with "Taken" (past its half hour, when it was taken is asked)
/// and the doubt when another device may know; until marked, the day's end,
/// or twelve hours after its time. `closed`: those due while Sioul ran
/// nowhere, the question on the past, once your other devices were heard
/// from (`heard`), so that a dose marked there is not asked about here.
/// Whatever the hours: doses are not mail. None while you sleep with doses
/// staying silent (`silent`): they come at waking.
#[derive(Default, Serialize)]
struct PorchDoses {
    due: Vec<DoseRow>,
    closed: Vec<DoseRow>,
}

/// `records` and `me` (this device's id): today's doses whose answers differ
/// on your devices come too, both answers said, until you choose.
#[allow(clippy::too_many_arguments)]
fn porch_doses(health: &Health, state: &HealthState, records: &DoseRecords, me: &str, knowledge: &Knowledge, now: &Zoned, silent: bool, heard: bool, words: &Translator) -> PorchDoses {
    if silent || health.medicines.is_empty() {
        return PorchDoses::default();
    }
    let stamp = now.timestamp().as_second();
    let row = |d: sioul_core::health::Dose, doubt: String, doubt_off: Vec<OffButton>, choose: bool| {
        let at = d.at.timestamp().as_second();
        DoseRow { taken: String::new(), past: true, late: stamp - at > GRACE_MINUTES * 60, doubt, doubt_off, choose, question: String::new(), time: d.at.strftime("%H:%M").to_string(), key: d.key, name: d.name, dose: d.dose }
    };
    // Answered in the records, their marks not here (yet): answered all the same; never due twice.
    let mut due: Vec<DoseRow> = state
        .due_today(health, now, MISSED_HOURS, GRACE_MINUTES)
        .into_iter()
        .filter(|d| sioul_core::doses::answered(&d.key, state, records) == Answered::Open)
        .map(|d| {
            let (doubt, doubt_off) = doubt_now_with(knowledge, d.at.timestamp().as_second(), stamp, words);
            row(d, doubt, doubt_off, false)
        })
        .collect();
    // Answered on two devices, differently: both said, as far back as today's doses show.
    let midnight = now.date().to_zoned(now.time_zone().clone()).unwrap_or_else(|_| now.clone());
    let back = now.checked_sub(Span::new().hours(MISSED_HOURS)).unwrap_or_else(|_| now.clone());
    let end = now.checked_add(Span::new().seconds(1)).unwrap_or_else(|_| now.clone());
    for d in health.doses(if back > midnight { &back } else { &midnight }, &end) {
        if let Answered::Differ(given) = sioul_core::doses::answered(&d.key, state, records) {
            let sentence = differ_sentence(&given, me, words);
            due.push(row(d, sentence, Vec::new(), true));
        }
    }
    due.sort_by_key(|row| due_of(&row.key).unwrap_or(0));
    PorchDoses { due, closed: if heard { missed_rows(health, state, knowledge, now, words) } else { Vec::new() } }
}

/// The Porch's doses, as JSON (`PorchDoses`), made off the window's thread.
pub(crate) fn missed() -> String {
    let health = load();
    if health.medicines.is_empty() {
        return json(&PorchDoses::default());
    }
    let heard = !crate::share::on() || crate::share::last_exchange().is_some();
    json(&porch_doses(&health, &record(), &records(), &this_device(), &know(), &Zoned::now(), crate::hours::doses_silent(), heard, tr()))
}

/// A medicine as its form gives it.
#[derive(Deserialize)]
struct MedicineEdit {
    /// Its brand name, or your own word for it; "" when its generic name says it.
    #[serde(default)]
    name: String,
    /// The generic name of its molecule (INN), and its strength: optional.
    #[serde(default)]
    generic: String,
    #[serde(default)]
    strength: String,
    /// "2026-03-02", the day it was first taken; "" when not known.
    #[serde(default)]
    since: String,
    /// Its usual amount: each take's when it says none.
    #[serde(default)]
    dose: String,
    /// "day", "days", "hours".
    every: String,
    /// At set times each day: each take, its time and its own amount ("" for the usual).
    #[serde(default)]
    takes: Vec<Take>,
    /// The times alone, as the form gave them before takes had rows.
    #[serde(default)]
    times: Vec<String>,
    #[serde(default)]
    days: u32,
    #[serde(default)]
    hours: u32,
    /// "2026-10-04"; for hours, "2026-10-03T18:30".
    #[serde(default)]
    from: String,
    #[serde(default)]
    time: String,
    #[serde(default)]
    until: String,
    #[serde(default)]
    prescription: String,
    #[serde(default)]
    paused: bool,
}

fn answer(result: Result<String, String>) -> String {
    match result {
        Ok(id) => serde_json::json!({ "id": id }).to_string(),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

/// Why takes could not be set, in words; with the medicine's name in a
/// prescription's form, where several are.
fn take_problem(problem: &TakeProblem, name: Option<&str>, words: &Translator) -> String {
    let id = match problem {
        TakeProblem::None => "health-no-take",
        TakeProblem::Unreadable(_) => "health-take-unreadable",
        TakeProblem::Twice(_) => "health-take-twice",
    };
    let mut pairs = vec![];
    if let TakeProblem::Unreadable(time) | TakeProblem::Twice(time) = problem {
        pairs.push(("time", time.clone()));
    }
    match name {
        Some(name) => {
            pairs.push(("name", name.to_string()));
            said(words, &format!("{id}-of"), &pairs)
        }
        None => said(words, id, &pairs),
    }
}

/// A medicine made (`id` empty) or changed in `health`, as its form gives
/// it: its id, or what is wrong, nothing changed then.
fn apply_medicine(health: &mut Health, id: &str, edit: MedicineEdit, now: &Zoned, words: &Translator) -> Result<String, String> {
    // Its name, else its generic name: reminders always have one, and so does an older Sioul.
    let name = if edit.name.trim().is_empty() { edit.generic.trim() } else { edit.name.trim() }.to_string();
    if name.is_empty() {
        return Err(words.text("health-no-name", None));
    }
    let day = |text: &str| text.trim().parse::<Date>().ok();
    let id = if id.is_empty() { health.new_id(&name) } else { id.to_string() };
    let mut medicine = Medicine {
        id: id.clone(),
        name,
        generic: edit.generic.trim().to_string(),
        strength: edit.strength.trim().to_string(),
        since: day(&edit.since),
        dose: edit.dose.trim().to_string(),
        schedule: Schedule::Day { times: Vec::new(), amounts: Default::default() },
        prescription: Some(edit.prescription).filter(|p| !p.is_empty()),
        until: day(&edit.until),
        paused: edit.paused,
    };
    match edit.every.as_str() {
        "days" => medicine.schedule = Schedule::Days { days: edit.days.max(1), time: edit.time.trim().to_string(), from: day(&edit.from).unwrap_or(now.date()) },
        "hours" => {
            let from = edit.from.trim().parse::<jiff::civil::DateTime>().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()).map_or(now.timestamp().as_second(), |z| z.timestamp().as_second());
            medicine.schedule = Schedule::Hours { hours: edit.hours.max(1), from };
        }
        _ => {
            let takes: Vec<Take> = if edit.takes.is_empty() { edit.times.iter().filter(|t| !t.trim().is_empty()).map(|t| Take { time: t.clone(), amount: String::new() }).collect() } else { edit.takes };
            medicine.set_takes(&edit.dose, &takes).map_err(|problem| take_problem(&problem, None, words))?;
        }
    }
    match health.medicines.iter_mut().find(|m| m.id == id) {
        Some(slot) => *slot = medicine,
        None => health.medicines.push(medicine),
    }
    Ok(id)
}

/// A medicine made (`id` empty) or changed; returns {"id"} or {"error"}.
pub(crate) fn save_medicine(id: &str, edit: &str) -> String {
    let result = serde_json::from_str::<MedicineEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        change_health(|health| apply_medicine(health, id, edit, &Zoned::now(), tr()))
    });
    answer(result)
}

/// A prescription as its form gives it.
#[derive(Deserialize)]
struct PrescriptionEdit {
    title: String,
    #[serde(default)]
    prescriber: String,
    #[serde(default)]
    until: String,
    #[serde(default)]
    refill_days: u32,
    #[serde(default)]
    last_refill: String,
    #[serde(default)]
    note: String,
    /// Its medicines, row by row; none given: they stay as they are.
    #[serde(default)]
    medicines: Option<Vec<PrescribedEdit>>,
}

/// A medicine as a row of its prescription's form gives it.
#[derive(Deserialize)]
struct PrescribedEdit {
    /// "" for one added in the form.
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    /// The generic name of its molecule (INN), and its strength: optional.
    #[serde(default)]
    generic: String,
    #[serde(default)]
    strength: String,
    /// Its usual amount: each take's when it says none.
    #[serde(default)]
    dose: String,
    /// Its takes, at set times each day (a medicine taken every few days or
    /// hours keeps its own schedule: its form changes it).
    #[serde(default)]
    takes: Vec<Take>,
    /// Taken out in the form: the medicine goes when the form is saved.
    #[serde(default)]
    removed: bool,
}

/// A prescription made (`id` empty) or changed in `health`, with its
/// medicines as its rows give them: made, changed, taken out. Every row is
/// read before anything changes: what does not save changes nothing. Its id,
/// or what is wrong.
fn apply_prescription(health: &mut Health, id: &str, edit: PrescriptionEdit, words: &Translator) -> Result<String, String> {
    if edit.title.trim().is_empty() {
        return Err(words.text("health-no-name", None));
    }
    let id = if id.is_empty() { health.new_id(&edit.title) } else { id.to_string() };
    let mut made: Vec<Medicine> = Vec::new();
    let mut gone: Vec<String> = Vec::new();
    for row in edit.medicines.iter().flatten() {
        if row.removed {
            gone.extend(Some(row.id.clone()).filter(|id| !id.is_empty()));
            continue;
        }
        // Its name, else its generic name.
        let name = if row.name.trim().is_empty() { row.generic.trim() } else { row.name.trim() };
        // A row added and left as it came: nothing to take.
        if row.id.is_empty() && name.is_empty() && row.dose.trim().is_empty() && row.strength.trim().is_empty() && row.takes.iter().all(|t| t.amount.trim().is_empty()) {
            continue;
        }
        if name.is_empty() {
            return Err(words.text("health-medicine-no-name", None));
        }
        let mut medicine = health.medicines.iter().find(|m| !row.id.is_empty() && m.id == row.id).cloned().unwrap_or_else(|| Medicine {
            id: row.id.clone(),
            name: String::new(),
            dose: String::new(),
            schedule: Schedule::Day { times: Vec::new(), amounts: Default::default() },
            prescription: None,
            until: None,
            paused: false,
            generic: String::new(),
            strength: String::new(),
            since: None,
        });
        medicine.name = name.to_string();
        medicine.generic = row.generic.trim().to_string();
        medicine.strength = row.strength.trim().to_string();
        medicine.prescription = Some(id.clone());
        if matches!(medicine.schedule, Schedule::Day { .. }) {
            medicine.set_takes(&row.dose, &row.takes).map_err(|problem| take_problem(&problem, Some(name), words))?;
        } else {
            medicine.dose = row.dose.trim().to_string();
        }
        made.push(medicine);
    }
    let prescription = Prescription {
        id: id.clone(),
        title: edit.title.trim().to_string(),
        prescriber: edit.prescriber.trim().to_string(),
        until: edit.until.trim().parse().ok(),
        refill_days: Some(edit.refill_days).filter(|d| *d > 0),
        last_refill: edit.last_refill.trim().parse().ok(),
        note: edit.note.trim().to_string(),
    };
    match health.prescriptions.iter_mut().find(|p| p.id == id) {
        Some(slot) => *slot = prescription,
        None => health.prescriptions.push(prescription),
    }
    health.medicines.retain(|m| !gone.contains(&m.id));
    for mut medicine in made {
        if medicine.id.is_empty() {
            medicine.id = health.new_id(&medicine.name);
        }
        match health.medicines.iter_mut().find(|m| m.id == medicine.id) {
            Some(slot) => *slot = medicine,
            None => health.medicines.push(medicine),
        }
    }
    Ok(id)
}

/// A prescription made (`id` empty) or changed, with its medicines; returns {"id"} or {"error"}.
pub(crate) fn save_prescription(id: &str, edit: &str) -> String {
    let result = serde_json::from_str::<PrescriptionEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        change_health(|health| apply_prescription(health, id, edit, tr()))
    });
    answer(result)
}

/// What a doctor or a pharmacist is shown, read-only, at arm's length
/// (`qml/ShowMedicines.qml`): a prescription's medicines, or all those taken now.
#[derive(Serialize, Debug, PartialEq)]
struct ForProfessional {
    /// The prescription's title, or "Current medicines".
    title: String,
    /// Who prescribed it and until when it is valid; for all, the day shown.
    about: Vec<String>,
    medicines: Vec<Shown>,
    /// No medicine to show, said; "" otherwise.
    empty: String,
}

/// One medicine, for a professional.
#[derive(Serialize, Debug, PartialEq)]
struct Shown {
    /// Its generic name and strength, first: "levothyroxine 75 µg"; its name
    /// and strength when no generic name is given.
    molecule: String,
    /// Its brand name, or your word for it, when it is not the generic name.
    brand: String,
    /// Each take, its time and amount ("07:30 — 1 tablet"); every few days
    /// or hours in words, with the amount.
    takes: Vec<String>,
    /// "Taken since Monday 2 March, 7 months"; "" when not known.
    since: String,
    /// "Prescribed by Dr Elena Varga", when the medicine's prescription says
    /// who, and the view is not that prescription's (it says so above).
    prescriber: String,
    /// "Until Wednesday 28 October", "Paused for now".
    notes: Vec<String>,
}

/// How long since `since`, in words: days, then weeks, months, years.
fn how_long(since: Date, today: Date, words: &Translator) -> String {
    let days = since.until(today).map_or(0, |s| i64::from(s.get_days())).max(0);
    let (id, n) = if days == 0 {
        return String::new();
    } else if days < 14 {
        ("health-pro-days", days)
    } else if days < 61 {
        ("health-pro-weeks", days / 7)
    } else if days < 730 {
        ("health-pro-months", days * 12 / 365)
    } else {
        ("health-pro-years", days / 365)
    };
    words.text(id, Some(&words.counted(usize::try_from(n).unwrap_or(0))))
}

/// A prescription's medicines (`prescription`, its id), or those taken now
/// ("": not paused, not past their last day), for a professional, in your
/// language; the generic names as typed (international).
fn for_professional_of(health: &Health, prescription: &str, today: Date, words: &Translator) -> ForProfessional {
    let p = health.prescriptions.iter().find(|p| p.id == prescription);
    let medicines: Vec<&Medicine> = match p {
        Some(p) => tied(health, p).collect(),
        None => health.medicines.iter().filter(|m| !m.paused && m.until.is_none_or(|day| day >= today)).collect(),
    };
    let shown = medicines
        .into_iter()
        .map(|m| {
            let (generic, strength) = (m.generic.trim(), m.strength.trim());
            let first = if generic.is_empty() { m.name.trim() } else { generic };
            let molecule = [first, strength].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
            let brand = if generic.is_empty() || m.name.trim().to_lowercase() == generic.to_lowercase() { String::new() } else { m.name.trim().to_string() };
            let takes = match &m.schedule {
                Schedule::Day { .. } => m.takes().into_iter().map(|t| if t.amount.is_empty() { t.time } else { format!("{} — {}", t.time, t.amount) }).collect(),
                schedule => vec![[words_of(schedule, words), m.dose.trim().to_string()].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" — ")],
            };
            let since = m
                .since
                .map(|day| match how_long(day, today, words) {
                    long if long.is_empty() => said(words, "health-pro-since-day", &[("day", words.day_in(day, today))]),
                    long => said(words, "health-pro-since", &[("day", words.day_in(day, today)), ("long", long)]),
                })
                .unwrap_or_default();
            let prescriber = match (p, m.prescription.as_deref().and_then(|id| health.prescriptions.iter().find(|p| p.id == id))) {
                (None, Some(of)) if !of.prescriber.trim().is_empty() => said(words, "health-pro-prescribed-by", &[("name", of.prescriber.trim().to_string())]),
                _ => String::new(),
            };
            let mut notes: Vec<String> = m.until.map(|day| said(words, "health-pro-until", &[("day", words.day_in(day, today))])).into_iter().collect();
            if m.paused {
                notes.push(words.text("health-pause", None));
            }
            Shown { molecule, brand, takes, since, prescriber, notes }
        })
        .collect::<Vec<_>>();
    let about = match p {
        Some(p) => [
            Some(p.prescriber.trim()).filter(|w| !w.is_empty()).map(|name| said(words, "health-pro-prescribed-by", &[("name", name.to_string())])),
            p.until.map(|day| said(words, "health-pro-valid-until", &[("day", words.day_in(day, today))])),
        ]
        .into_iter()
        .flatten()
        .collect(),
        None => vec![said(words, "health-pro-as-of", &[("day", format!("{} {}", words.day(today), today.year()))])],
    };
    ForProfessional {
        title: p.map_or_else(|| words.text("health-pro-current", None), |p| p.title.clone()),
        about,
        empty: if shown.is_empty() { words.text("health-pro-none", None) } else { String::new() },
        medicines: shown,
    }
}

/// When a medicine taken every few days or hours is taken, in `words`; at set times, its times.
fn words_of(schedule: &Schedule, words: &Translator) -> String {
    // Numbers as numbers: "every other day" is chosen by the number 2.
    let mut args = sioul_core::i18n::args();
    match schedule {
        Schedule::Days { days, time, from } => {
            args.set("days", *days);
            args.set("time", time.clone());
            args.set("from", words.day(*from));
            words.text("health-every-days", Some(&args))
        }
        Schedule::Hours { hours, .. } => {
            args.set("hours", *hours);
            words.text("health-pro-every-hours", Some(&args))
        }
        Schedule::Day { .. } => self::words(schedule),
    }
}

/// "Show to a doctor or pharmacist": a prescription's medicines (its id), or
/// all those taken now (""), as JSON (`ForProfessional`).
pub(crate) fn for_professional(prescription: &str) -> String {
    json(&for_professional_of(&load(), prescription, Zoned::now().date(), tr()))
}

/// A medicine or a prescription taken out; returns what went wrong, else "".
pub(crate) fn remove(id: &str) -> String {
    change_health(|health| {
        health.medicines.retain(|m| m.id != id);
        health.prescriptions.retain(|p| p.id != id);
        for medicine in health.medicines.iter_mut().filter(|m| m.prescription.as_deref() == Some(id)) {
            medicine.prescription = None;
        }
        Ok(())
    })
    .err()
    .unwrap_or_default()
}

/// Fetched at the pharmacy today: the next refill is counted from now.
pub(crate) fn refilled(id: &str) -> String {
    change_health(|health| {
        if let Some(p) = health.prescriptions.iter_mut().find(|p| p.id == id) {
            p.last_refill = Some(Zoned::now().date());
        }
        Ok(())
    })
    .err()
    .unwrap_or_default()
}

/// How far back a dose due while Sioul was closed is asked about, and the
/// minutes a dose is reminded live before that.
const MISSED_HOURS: i64 = 12;
const GRACE_MINUTES: i64 = 30;
/// When the pause to move was last offered, or Sioul started (Unix seconds).
static MOVED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// The pause to move and stretch, every so many minutes since Sioul started
/// or since the last one, without waiting for a focus session; a session
/// running offers its own (`FocusWindow.qml`). A desktop notification: it
/// reaches you with the window hidden, from the computer you are at.
fn movement_tick(qt: &QtThread, shared: &Arc<Shared>, health: &Health, now: &Zoned, here: bool) {
    use std::sync::atomic::Ordering;
    let stamp = now.timestamp().as_second();
    let last = MOVED.load(Ordering::Relaxed);
    // At a time the matrix of what reaches you drops it (as usual: sleep, the pauses, a slot
    // of time for you, do-not-disturb), the pause is counted again from its end.
    if last == 0 || !health.movement.enabled || sioul_core::timelog::running().is_some() || !crate::hours::comes(sioul_core::attention::Kind::Move) {
        MOVED.store(stamp, Ordering::Relaxed);
        return;
    }
    if !here || stamp - last < i64::from(health.movement.minutes.max(10)) * 60 {
        return;
    }
    MOVED.store(stamp, Ordering::Relaxed);
    let qt_open = qt.clone();
    let open: Box<dyn FnOnce() + Send> = Box::new(move || {
        let _ = qt_open.queue(|mut sioul| sioul.as_mut().reminder_opened(QString::from("stopped"), QString::default(), QString::default()));
    });
    if let Err(e) = sioul_sync::notify::remind(&tr().text("health-move", None), &tr().text("health-move-body", None), Some((tr().text("stopped-note", None), open))) {
        tell(qt, shared, e);
    }
}

/// The question on doses due while Sioul was closed: asked once a session.
static ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Doses reminded in this session: never twice, even when the record cannot be written.
static SENT: std::sync::Mutex<std::collections::BTreeSet<String>> = std::sync::Mutex::new(std::collections::BTreeSet::new());
/// "Reminders come on `<computer>`", when another computer keeps them.
static REMINDED_THERE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Why the doses here may not be all: this computer alone, or what is not
/// known of your other devices now; "" when all is known.
fn shared_note(knowledge: &Knowledge) -> String {
    if !crate::share::on() {
        return tr().text("health-alone", None);
    }
    let now = Timestamp::now().as_second();
    // As for a dose due a while ago (the wait for news): a device in use whose
    // news is fresh is known by then; what is said here lasts (a device silent
    // while in use, an older Sioul, a record that does not read).
    let doubts = sioul_core::health::doubts_now(now - WAIT_FOR_NEWS, now, knowledge.record_lost, &knowledge.peers);
    let mut lines: Vec<String> = Vec::new();
    if !doubts.is_empty() {
        lines.push(say("dose-doubt-now", &[("why", why(&doubts))]));
    }
    // Devices you said are off: said plainly, for as long as it lasts.
    for peer in knowledge.peers.iter().filter(|p| p.off) {
        lines.push(say("dose-off-note", &[("name", device_start(&named_of(peer), tr()))]));
    }
    lines.join(" ")
}

/// The Health page, the doses due while closed and today's meals and naps,
/// made on a thread (they read the doses' record and the other devices'
/// claims, a shared folder that may be slow) and shown.
pub(crate) fn show_health(qt: &QtThread, shared: &Arc<Shared>) {
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.health_job, move |_| {
        let (view, missed, needs) = (page(), missed(), needs_today());
        if cfg!(target_os = "android") {
            eprintln!("sioul: health: {} medicines, {} prescriptions", load().medicines.len(), load().prescriptions.len());
        }
        let _ = qt.queue(move |mut sioul| {
            sioul.as_mut().set_health_view(QString::from(&view));
            sioul.as_mut().set_missed_view(QString::from(&missed));
            sioul.as_mut().set_needs_view(QString::from(&needs));
        });
    });
}

// ---------------------------------------------------------------- phone alarms

/// How far ahead the coming doses are given to Android's alarm clock.
const ALARMS_AHEAD_HOURS: i64 = 48;

/// A dose as a reminder names it: "Levothyroxine · 75 µg".
fn named(dose: &sioul_core::health::Dose) -> String {
    if dose.dose.is_empty() { dose.name.clone() } else { format!("{} · {}", dose.name, dose.dose) }
}

/// The coming doses, as Android's alarm clock is given them (`alarms::schedule`):
/// [{key, at (Unix milliseconds), title}], from half an hour ago to two days
/// ahead, those marked or reminded left out. A record that does not read
/// leaves none out: the decision at their time reads it again.
pub(crate) fn alarms_coming() -> String {
    let health = load();
    let state = HealthState::read(&HealthState::default_path()).unwrap_or_default();
    let now = Zoned::now();
    let start = now.checked_sub(Span::new().minutes(GRACE_MINUTES)).unwrap_or_else(|_| now.clone());
    let end = now.checked_add(Span::new().hours(ALARMS_AHEAD_HOURS)).unwrap_or_else(|_| now.clone());
    let rows: Vec<serde_json::Value> = health
        .doses(&start, &end)
        .into_iter()
        .filter(|d| !state.taken.contains_key(&d.key) && !state.not_taken.contains_key(&d.key) && !state.reminded.contains_key(&d.key))
        .map(|d| serde_json::json!({ "key": d.key, "at": d.at.timestamp().as_millisecond(), "title": named(&d) }))
        .collect();
    serde_json::Value::Array(rows).to_string()
}

/// The doses of the last day marked taken or not taken: their reminders, if
/// shown on a phone, are taken away (`alarms::schedule`).
pub(crate) fn marked_lately() -> Vec<String> {
    let Ok(state) = HealthState::read(&HealthState::default_path()) else { return Vec::new() };
    let since = Timestamp::now().as_second() - 26 * 3600;
    state.taken.keys().chain(state.not_taken.keys()).filter(|key| due_of(key).is_some_and(|due| due >= since)).cloned().collect()
}

/// How long a reminder shown on a phone is checked again (every quarter of an
/// hour, the others' news read first), so that a dose marked on another device
/// takes it away: three hours after the dose's time.
const SHOWN_WATCH: i64 = 3 * 3600;
const SHOWN_EVERY: i64 = 15 * 60;

/// At a dose's time, Android's alarm asks what to say (`alarms`), maybe with
/// nothing on the screen. What your other devices marked is read first
/// (`share::exchange_here`: Sioul's own pull from the server while it works,
/// else the sync app asked to bring it; an exchange without notes and papers,
/// which never holds a dose back); then, as the window would: taken, not taken
/// said, or reminded already: nothing; kept by another device you use: its
/// turn, asked again after the wait for news; not known: after that wait,
/// reminded with the doubt said first; else the dose. Recorded `reminded`
/// only when shown, so that another device still reminds what was not.
/// JSON {show, title, body, again_at (Unix ms, 0: never)}.
pub(crate) fn alarm_decide(key: &str) -> String {
    let answer = |show: bool, title: &str, body: &str, again: i64| serde_json::json!({ "show": show, "title": title, "body": body, "again_at": again * 1000, "taken": tr().text("health-taken", None) }).to_string();
    let _ = crate::share::exchange_here(true);
    // None of the others' news read since this alarm began (its pull and the
    // sync app bringing nothing): what was read before is said in doubt.
    let missed = crate::share::news_missed();
    let health = load();
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    let _held = state_held();
    let state = record();
    let start = now.checked_sub(Span::new().hours(MISSED_HOURS)).unwrap_or_else(|_| now.clone());
    let end = now.checked_add(Span::new().hours(ALARMS_AHEAD_HOURS)).unwrap_or_else(|_| now.clone());
    // The medicine taken out, or its doses moved: this one is no more, nor its reminder.
    let Some(dose) = health.doses(&start, &end).into_iter().find(|d| d.key == key) else {
        crate::alarms::remove_reminder(key);
        return answer(false, "", "", 0);
    };
    let due = dose.at.timestamp().as_second();
    // Marked, here or on another device: its reminder, if shown, goes.
    if state.taken.contains_key(key) || state.not_taken.contains_key(key) {
        crate::alarms::remove_reminder(key);
        return answer(false, "", "", 0);
    }
    // Reminded already (here, still shown, or on another device): looked at
    // again a while, so that a mark made elsewhere takes the reminder away.
    if state.reminded.contains_key(key) || SENT.lock().is_ok_and(|sent| sent.contains(key)) {
        let again = if stamp < due + SHOWN_WATCH { stamp + SHOWN_EVERY } else { 0 };
        return answer(false, "", "", again);
    }
    // Too early (moved later): at its time.
    if due > stamp + 60 {
        return answer(false, "", "", due);
    }
    // Asleep, "Doses during sleep: stay silent" chosen: asked again at waking.
    if crate::hours::doses_silent()
        && let Some(waking) = crate::hours::waking()
    {
        return answer(false, "", "", waking);
    }
    let waiting = stamp < due + WAIT_FOR_NEWS;
    let keeper = crate::share::looked("health", sioul_sync::lease::Rule::FollowsYou);
    if !keeper.mine && waiting {
        return answer(false, "", "", due + WAIT_FOR_NEWS);
    }
    let knowledge = know();
    let doubt = doubt_unread(&knowledge, due, stamp, missed.as_ref());
    if !doubt.is_empty() && waiting {
        // Asked again as soon as it may be known (a device closed before the
        // dose, once the news of an opening had the time to come), a minute
        // apart at least, within the wait for news.
        let known = sioul_core::health::known_from(due, stamp + 1, due + WAIT_FOR_NEWS, knowledge.record_lost, &knowledge.peers);
        return answer(false, "", "", known.map_or(due + WAIT_FOR_NEWS, |at| at.max(stamp + 60).min(due + WAIT_FOR_NEWS)));
    }
    let named = named(&dose);
    let (title, body) = if doubt.is_empty() { (named, dose.at.strftime("%H:%M").to_string()) } else { (say("dose-check-title", &[("dose", named)]), format!("{}. {doubt}", dose.at.strftime("%H:%M"))) };
    if let Ok(mut sent) = SENT.lock() {
        sent.insert(key.to_string());
    }
    // A session of its own, Sioul not on the screen (`devices::receiver`): up,
    // the reminder and the dose's record (the canary) noted, sent, down. Your
    // other devices know it was reminded: they do not remind it again.
    crate::devices::receiver(move || {
        let _ = change(|record| {
            record.reminded.insert(key.to_string(), stamp);
        });
        open_canaries(&[key.to_string()], stamp);
        drop(_held);
        let _ = crate::share::exchange_here(false);
    });
    // Shown: looked at again while it may still be shown (a mark elsewhere takes it away).
    answer(true, &title, &body, stamp + SHOWN_EVERY)
}

/// "Taken", pressed on the phone's reminder. More than half an hour late, when
/// it was taken is asked in the window instead (`open`). JSON {done, open, line}.
pub(crate) fn alarm_taken(key: &str) -> String {
    if is_late(key) {
        return serde_json::json!({ "done": false, "open": true, "line": tr().text("dose-alarm-late", None) }).to_string();
    }
    // A session of its own, Sioul not on the screen (`devices::receiver`): up,
    // the answer noted, sent, down; your other devices show it taken.
    crate::devices::receiver(|| {
        let problem = set_taken(key, true);
        if !problem.is_empty() {
            return serde_json::json!({ "done": false, "open": true, "line": problem }).to_string();
        }
        let _ = crate::share::exchange_here(false);
        let line = say("health-taken-at", &[("time", Zoned::now().strftime("%H:%M").to_string())]);
        serde_json::json!({ "done": true, "open": false, "line": line }).to_string()
    })
}

// ---------------------------------------------------------------- the home screen's card

/// Today's doses for the card on a phone's home screen (`homecard`): those
/// due and not marked, from their time on, as the Porch has them
/// (`due_today`: until marked, the day's end, or twelve hours after their
/// time), and those still to come today; each with until when it is known
/// not marked on another device, the Porch's rule given ahead
/// (`doubts_now`: every other device heard within five minutes), 0 when it
/// is not known from its time on. A record that does not read is never read
/// as empty: every dose is then not known. Whether doses show while you
/// sleep or pause is the card's frames' to say.
pub(crate) fn card_doses(now: &Zoned) -> Vec<crate::homecard::Dose> {
    let health = load();
    if health.medicines.is_empty() {
        return Vec::new();
    }
    let read = HealthState::read(&HealthState::default_path());
    let readable = read.is_ok();
    doses_for_card(&health, &read.unwrap_or_default(), &records(), readable, &know(), now)
}

/// `card_doses`, from what was read. A dose answered differently on two
/// devices (`doses::answered`: taken on one, said not taken on another) is
/// on the card too, never known: "check before taking it", as the Porch asks you to choose.
fn doses_for_card(health: &Health, state: &HealthState, records: &DoseRecords, readable: bool, knowledge: &Knowledge, now: &Zoned) -> Vec<crate::homecard::Dose> {
    let stamp = now.timestamp().as_second();
    let midnight_after = |at: &Zoned| at.date().tomorrow().ok().and_then(|d| d.to_zoned(at.time_zone().clone()).ok());
    let Some(tonight) = midnight_after(now) else { return Vec::new() };
    let later = now.checked_add(Span::new().seconds(1)).unwrap_or_else(|_| now.clone());
    let coming = health.doses(&later, &tonight).into_iter().filter(|d| !state.taken.contains_key(&d.key) && !state.not_taken.contains_key(&d.key));
    // Answered in the records, their marks not here (yet): answered all the same; never on the card twice.
    let mut out: Vec<crate::homecard::Dose> = state
        .due_today(health, now, MISSED_HOURS, GRACE_MINUTES)
        .into_iter()
        .chain(coming)
        .filter(|d| sioul_core::doses::answered(&d.key, state, records) == Answered::Open)
        .flat_map(|d| {
            let at = d.at.timestamp().as_second();
            let end = midnight_after(&d.at).map_or(at + 86_400, |m| m.timestamp().as_second());
            let until = (at + MISSED_HOURS * 3600).min(end);
            let start = stamp.max(at);
            // Not known at once (a device closed before it may open around it,
            // `known_from`): "check" from its time until it can be, then the line.
            let pieces = match readable.then(|| sioul_core::health::known_from(at, start, until, knowledge.record_lost, &knowledge.peers)).flatten() {
                None => vec![(at, until, 0)],
                Some(known) if known <= start => vec![(at, until, card_known_until(knowledge, at, start, until))],
                Some(known) => vec![(at, known, 0), (known, until, card_known_until(knowledge, at, known, until))],
            };
            let (name, time) = (named(&d), d.at.strftime("%H:%M").to_string());
            pieces.into_iter().map(move |(from, until, known_until)| crate::homecard::Dose { name: name.clone(), time: time.clone(), at: from, until, known_until })
        })
        .collect();
    // Answered differently, as far back as today's doses show: never known, until one answer is chosen.
    let midnight = now.date().to_zoned(now.time_zone().clone()).unwrap_or_else(|_| now.clone());
    let back = now.checked_sub(Span::new().hours(MISSED_HOURS)).unwrap_or_else(|_| now.clone());
    for d in health.doses(if back > midnight { &back } else { &midnight }, &later) {
        if matches!(sioul_core::doses::answered(&d.key, state, records), Answered::Differ(_)) {
            let at = d.at.timestamp().as_second();
            let end = midnight_after(&d.at).map_or(at + 86_400, |m| m.timestamp().as_second());
            out.push(crate::homecard::Dose { name: named(&d), time: d.at.strftime("%H:%M").to_string(), at, until: (at + MISSED_HOURS * 3600).min(end), known_until: 0 });
        }
    }
    out.sort_by_key(|d| d.at);
    out
}

/// Until when a dose due at `due` stays known not marked elsewhere, seen
/// from `from` (`doubts_now`: each other device fresh five minutes after it
/// was last heard in full): `until` when nothing ends it before; 0 when it is
/// not known at `from`.
fn card_known_until(knowledge: &Knowledge, due: i64, from: i64, until: i64) -> i64 {
    sioul_core::health::known_until(due, from, until, knowledge.record_lost, &knowledge.peers)
}

// ---------------------------------------------------------------- knowing

/// How long a dose not known here waits for news from your other devices
/// before it is reminded all the same, the doubt said.
const WAIT_FOR_NEWS: i64 = 10 * 60;

/// The doses' record found broken or gone here, kept on this computer only
/// (`$XDG_STATE_HOME/sioul/health-doubt.toml`): the doses due before stay
/// not known, for a day.
#[derive(Debug, Default, Serialize, Deserialize)]
struct RecordDoubt {
    since: i64,
    /// Where the broken record was kept aside; "" when it was gone.
    #[serde(default)]
    aside: String,
}

fn record_doubt_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("health-doubt.toml")
}

fn record_doubt() -> Option<i64> {
    let doubt: RecordDoubt = toml::from_str(&std::fs::read_to_string(record_doubt_path()).ok()?).ok()?;
    Some(doubt.since)
}

fn write_record_doubt(since: i64, aside: &str) {
    let doubt = RecordDoubt { since, aside: aside.to_string() };
    if let Ok(text) = toml::to_string(&doubt) {
        let _ = std::fs::write(record_doubt_path(), text);
    }
}

/// The doses' record. One that cannot be trusted is never read as empty: it
/// is repaired (`repair`), and the doses due before are said not known.
fn record() -> HealthState {
    let path = HealthState::default_path();
    match HealthState::read(&path) {
        Ok(state) => state,
        Err(_) => {
            repair();
            HealthState::read(&path).unwrap_or_default()
        }
    }
}

/// A record that cannot be trusted: nothing it held may be taken out on your
/// other devices, so the sharing forgets it first and reads it again from
/// every device's records at the next exchange; then it is kept aside and a
/// new one starts. Sharing failing that, it is left as it is: never started
/// again empty where the sharing could take marks out elsewhere.
fn repair() {
    let now = Timestamp::now().as_second();
    if crate::share::on() && !crate::share::rebuild_health_record().is_empty() {
        write_record_doubt(now, "");
        return;
    }
    let aside = HealthState::set_aside(&HealthState::default_path(), now).ok().flatten().map(|p| p.display().to_string()).unwrap_or_default();
    write_record_doubt(now, &aside);
}

/// The record changed by `change`, under its lock; one that cannot be trusted
/// is repaired first, then changed. Returns what went wrong, else "".
fn change(change: impl Fn(&mut HealthState)) -> String {
    let path = HealthState::default_path();
    let now = Timestamp::now().as_second();
    // What changed is owed to your other devices until an export reads it,
    // said once written (`devices::recorded`); a minute that changed nothing owes nothing.
    let changed = std::sync::atomic::AtomicBool::new(false);
    let changing = |state: &mut HealthState| {
        let before = state.clone();
        change(state);
        if *state != before {
            changed.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    };
    let problem = match HealthState::update(&path, now, &changing) {
        Ok(()) => {
            // A dose marked: a phone's alarm for it goes (`alarms`).
            crate::alarms::schedule();
            String::new()
        }
        Err(Problem::Unsound(_)) => {
            repair();
            match HealthState::update(&path, now, &changing) {
                Ok(()) => String::new(),
                Err(_) => tr().text("dose-record-broken", None),
            }
        }
        Err(Problem::Write(e)) => e,
    };
    if changed.load(std::sync::atomic::Ordering::Relaxed) {
        crate::devices::recorded();
    }
    problem
}

/// The doses' records (`sioul_core::doses`: each dose that fell due, and the
/// answers your devices captured), as `record` reads the marks: records that
/// cannot be trusted are never read as empty; they are repaired, and the
/// doses due before are said not known for a day.
fn records() -> DoseRecords {
    let path = DoseRecords::default_path();
    match DoseRecords::read(&path) {
        Ok(records) => records,
        Err(_) => {
            repair_records();
            DoseRecords::read(&path).unwrap_or_default()
        }
    }
}

/// Records that cannot be trusted, as `repair` does the marks: the sharing
/// first forgets holding them and reads them again from every device's
/// records; then they are kept aside and new ones start.
fn repair_records() {
    let now = Timestamp::now().as_second();
    if crate::share::on() && !crate::share::rebuild_dose_records().is_empty() {
        write_record_doubt(now, "");
        return;
    }
    let aside = DoseRecords::set_aside(&DoseRecords::default_path(), now).ok().flatten().map(|p| p.display().to_string()).unwrap_or_default();
    write_record_doubt(now, &aside);
}

/// The records changed by `change`, under their lock; ones that cannot be
/// trusted repaired first, then changed. Returns what went wrong, else "".
fn change_records(change: impl Fn(&mut DoseRecords)) -> String {
    let path = DoseRecords::default_path();
    let now = Timestamp::now().as_second();
    // Owed to your other devices until an export reads it, as the marks (`change`).
    let changed = std::sync::atomic::AtomicBool::new(false);
    let changing = |records: &mut DoseRecords| {
        let before = records.clone();
        change(records);
        if *records != before {
            changed.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    };
    let problem = match DoseRecords::update(&path, now, &changing) {
        Ok(()) => String::new(),
        Err(Problem::Unsound(_)) => {
            repair_records();
            match DoseRecords::update(&path, now, &changing) {
                Ok(()) => String::new(),
                Err(_) => tr().text("dose-record-broken", None),
            }
        }
        Err(Problem::Write(e)) => e,
    };
    if changed.load(std::sync::atomic::Ordering::Relaxed) {
        crate::devices::recorded();
    }
    problem
}

/// Your answer for a dose, captured here (its time `at`), in the doses'
/// records, beside the marks: `settle` takes every other answer saying
/// otherwise out (you chose, seeing them). Returns what went wrong, else "".
fn record_answer(key: &str, state: &str, at: i64, settle: bool) -> String {
    let me = this_device();
    if me.is_empty() {
        return String::new();
    }
    let answer = Answer { state: state.to_string(), at, noted: Timestamp::now().as_second(), name: crate::devices::name(), kind: crate::devices::kind().to_string() };
    change_records(|records| if settle { records.choose(key, &me, answer.clone()) } else { records.answer(key, &me, answer.clone()) })
}

/// The doses due in the last hours (as far back as a dose is asked about),
/// answered nowhere this device knows: their records opened, not taken yet
/// (the canary), where no device opened them yet.
fn open_due(health: &Health, state: &HealthState, now: &Zoned) {
    let start = now.checked_sub(Span::new().hours(MISSED_HOURS)).unwrap_or_else(|_| now.clone());
    let end = now.checked_add(Span::new().seconds(1)).unwrap_or_else(|_| now.clone());
    let keys: Vec<String> = health.doses(&start, &end).into_iter().map(|d| d.key).filter(|key| !state.taken.contains_key(key) && !state.not_taken.contains_key(key)).collect();
    open_canaries(&keys, now.timestamp().as_second());
}

/// Each of `keys` opened here, not taken yet, unless a device opened or answered it already.
fn open_canaries(keys: &[String], now: i64) {
    let me = this_device();
    if keys.is_empty() || me.is_empty() {
        return;
    }
    // Records that do not read are repaired where they are read (`records`), not here.
    let Ok(records) = DoseRecords::read(&DoseRecords::default_path()) else { return };
    if keys.iter().all(|key| records.dose.get(key).is_some_and(|r| !r.opened.is_empty() || !r.answer.is_empty())) {
        return;
    }
    let _ = change_records(|records| {
        for key in keys {
            records.open(key, &me, now);
        }
    });
}

/// This device's id in the sharing ("" before it has one).
fn this_device() -> String {
    crate::share::vault().map(|(id, _)| id).unwrap_or_default()
}

/// What this computer saw of your other devices, for the doses: kept here
/// only (`$XDG_STATE_HOME/sioul/share/peers.json`), never shared.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Peers {
    #[serde(default)]
    peers: std::collections::BTreeMap<String, PeerSeen>,
    /// When this device last looked (its clock): an opening seen now is timed
    /// only when it looked a moment before, else its own absence would count.
    #[serde(default)]
    looked: i64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct PeerSeen {
    peer: Peer,
    /// Its last claim seen here: when it renewed it (its clock).
    #[serde(default)]
    renewed: i64,
    /// When this computer last looked (its clock): news is timed only while it watches.
    #[serde(default)]
    looked: i64,
    /// How late its last claims came here, in seconds.
    #[serde(default)]
    delays: Vec<i64>,
    /// Its entry in the devices' registry as last seen changing here (`peer.seen`: when).
    #[serde(default)]
    entry: Option<sioul_sync::devices::Entry>,
    /// Its export's date (its clock) less when it was seen here (this one's),
    /// the last sixty: how far its clock is ahead, at least, is the largest.
    #[serde(default)]
    aheads: Vec<i64>,
    /// You said it is off: what it had said then. Counted again once it says anything newer.
    #[serde(default)]
    off: Option<Mark>,
    /// You forgot it, silent: not listed until it says anything newer.
    #[serde(default)]
    forgotten: Option<Mark>,
    /// How late the news of its last openings came here, in seconds
    /// (`Peer::wake`: the longest), the last `WAKES`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    wakes: Vec<i64>,
    /// Since when its claim on the doses ran, as last seen: an older Sioul's
    /// openings, timed as an entry's start is.
    #[serde(default, skip_serializing_if = "is_zero")]
    since: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// How many of a device's openings are kept to time the next: an opening
/// whose news came late once (a phone offline when picked up) holds the wait
/// for this many more, then goes.
const WAKES: usize = 10;

/// How late the news of a device's opening came here, kept (`PeerSeen::wakes`).
fn note_wake(seen: &mut PeerSeen, delay: i64) {
    seen.wakes.push(delay.max(0));
    let keep = seen.wakes.len().saturating_sub(WAKES);
    seen.wakes.drain(..keep);
}

/// What a device had said when you said it was off, or forgot it.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Mark {
    #[serde(default)]
    started: i64,
    #[serde(default)]
    exported: i64,
    #[serde(default)]
    renewed: i64,
    /// When you said so (this device's clock).
    #[serde(default)]
    at: i64,
}

fn peers_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("share").join("peers.json")
}

/// What is known here of the doses marked elsewhere: this computer's record,
/// and each other device sharing with it (`sioul_core::health::doubts`).
pub(crate) struct Knowledge {
    record_lost: Option<i64>,
    peers: Vec<Peer>,
}

/// One look at a time at the others' claims.
static KNOWING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// What this computer knows now: the others' claims on the doses read (each
/// says how far it wrote its records, and whether it closed), set against
/// what was read of their records here.
fn know() -> Knowledge {
    let now = Timestamp::now().as_second();
    let record_lost = record_doubt();
    let Some(others) = crate::share::others_on_health() else { return Knowledge { record_lost, peers: Vec::new() } };
    let _held = KNOWING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // Read, learned, written under its lock: a phone's background service learns too, in its own process.
    let kept = sioul_core::filelock::with_lock(&peers_path(), || {
        let mut kept = load_peers();
        learn(&mut kept, &others.claims, &others.entries, &others.heard, now);
        save_peers(&kept);
        kept
    });
    let mut peers: Vec<Peer> = kept.peers.values().map(|s| s.peer.clone()).collect();
    // Two phones or more: each said by its name, never both "the phone".
    if peers.iter().filter(|p| p.said.as_ref().is_some_and(|s| s.phone)).count() > 1 {
        for said in peers.iter_mut().filter_map(|p| p.said.as_mut()) {
            said.phone = false;
        }
    }
    // A device sharing through the folder this last week whose claim and entry
    // do not read here (its seal not arrived yet, a damaged file): a peer all
    // the same, never heard from, so that its doses are said not known.
    let silent = sioul_core::health::SILENT_DAYS * 86_400;
    for other in others.others.iter().filter(|o| !kept.peers.contains_key(&o.id) && now - o.heard < silent) {
        peers.push(Peer { id: other.id.clone(), name: tr().text("share-other-device", None), heard: other.heard, ..Peer::default() });
    }
    let unread: Vec<String> = others.unread.iter().filter(|id| !kept.peers.contains_key(*id) && !peers.iter().any(|p| p.id == **id)).cloned().collect();
    for id in unread {
        peers.push(Peer { id, name: tr().text("share-other-device", None), heard: now, ..Peer::default() });
    }
    Knowledge { record_lost, peers }
}

fn load_peers() -> Peers {
    std::fs::read_to_string(peers_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_peers(kept: &Peers) -> String {
    let written = serde_json::to_string(kept).map_err(|e| e.to_string()).and_then(|text| {
        std::fs::create_dir_all(sioul_core::config::state_dir().join("share")).map_err(|e| e.to_string())?;
        std::fs::write(peers_path(), text).map_err(|e| e.to_string())
    });
    written.err().unwrap_or_default()
}

/// What the others' claims say, learned: each one's name, how late its news
/// comes, and until when everything it wrote is read here.
fn learn(kept: &mut Peers, claims: &[sioul_sync::lease::Claim], entries: &[sioul_sync::devices::Entry], heard: &sioul_sync::share::Heard, now: i64) {
    // How late a device's opening comes is timed only while this device
    // looked a moment before (review of 5 October 2026, F21).
    let watching = kept.looked > 0 && now - kept.looked <= 2 * 60;
    kept.looked = now;
    for claim in claims {
        let seen = kept.peers.entry(claim.computer.clone()).or_default();
        seen.peer.id = claim.computer.clone();
        seen.peer.name = claim.name.clone();
        // Its claim begun again, open (an older Sioul's opening): how late its news came.
        if claim.since != seen.since {
            if seen.since > 0 && watching && !claim.closed {
                note_wake(seen, now - claim.since);
            }
            seen.since = claim.since;
        }
        if claim.renewed > seen.renewed {
            // How late its news comes, timed only while watching: a claim
            // found after this computer was closed is old, not late.
            if seen.renewed > 0 && now - seen.looked <= 2 * 60 {
                seen.delays.push((now - claim.renewed).max(0));
                let keep = seen.delays.len().saturating_sub(60);
                seen.delays.drain(..keep);
            }
            seen.renewed = claim.renewed;
        }
        seen.looked = now;
        seen.peer.delay = seen.delays.iter().copied().max();
        seen.peer.heard = seen.peer.heard.max(claim.renewed);
        // Everything it wrote until its claim, read here: known until then,
        // never past this device's clock: a claim dated ahead (its clock
        // fast) by more than a minute is known not at all (review of 5
        // October 2026, F13), one a little ahead until now.
        if let Some(wrote) = claim.wrote
            && heard.complete(&claim.computer, wrote)
            && claim.renewed <= now + 60
            && claim.renewed.min(now) >= seen.peer.known_until
        {
            seen.peer.known_until = claim.renewed.min(now);
            seen.peer.closed = claim.closed;
        }
        seen.peer.broken = heard.broken.get(&claim.computer).copied();
    }
    // What each says of itself in the registry (`sioul_sync::devices`), read
    // now: only what reads now counts; a device whose entry no longer reads is
    // known by its claims, as an older Sioul is, never as it last said.
    for seen in kept.peers.values_mut() {
        seen.peer.said = None;
        seen.peer.complete = false;
    }
    for entry in entries {
        let seen = kept.peers.entry(entry.id.clone()).or_default();
        seen.peer.id = entry.id.clone();
        if !entry.name.is_empty() {
            seen.peer.name = entry.name.clone();
        }
        let state = |e: &sioul_sync::devices::Entry| (e.started, e.closed, e.working, e.exported, e.left);
        if seen.entry.as_ref().is_none_or(|last| state(last) != state(entry)) {
            // Seen changing now (this device's clock); its export dated later
            // than now, its clock is ahead of this one's by that much at least.
            seen.peer.seen = now;
            if seen.entry.as_ref().is_none_or(|last| last.exported != entry.exported) {
                seen.aheads.push(entry.exported - now);
                let keep = seen.aheads.len().saturating_sub(60);
                seen.aheads.drain(..keep);
            }
            // Started again since last seen: how late the news of its opening
            // came, its clock's lead taken off (a clock behind only adds to it).
            if watching && seen.entry.as_ref().is_some_and(|last| entry.started > last.started) {
                let ahead = seen.aheads.iter().copied().max().unwrap_or(0).max(0);
                note_wake(seen, now - entry.started + ahead);
            }
            seen.entry = Some(entry.clone());
        }
        seen.peer.ahead = seen.aheads.iter().copied().max().unwrap_or(0).max(0);
        // Its entry says it closed, but its claim, renewed open more than a
        // minute after that close (both on its own clock), says it runs: an
        // older Sioul there now (a version put back), which updates no entry.
        // The entry is not trusted: known by its claims, as an older Sioul is.
        let alive_since = claims.iter().any(|c| c.computer == entry.id && !c.closed && c.renewed > entry.closed + 60);
        if !entry.working && alive_since {
            continue;
        }
        seen.peer.said = Some(entry.said());
        // Everything it wrote up to its last export, read here.
        seen.peer.complete = entry.wrote.is_none_or(|wrote| heard.complete(&entry.id, wrote));
        seen.peer.heard = seen.peer.heard.max(entry.exported);
        seen.peer.broken = heard.broken.get(&entry.id).copied();
    }
    // Said off, or forgotten, until it says anything newer: a new start, a new
    // export, a claim renewed since.
    for seen in kept.peers.values_mut() {
        seen.peer.wake = seen.wakes.iter().copied().max();
        let (started, exported, renewed) = (seen.entry.as_ref().map_or(0, |e| e.started), seen.entry.as_ref().map_or(0, |e| e.exported), seen.renewed);
        let newer = |mark: &Mark| started != mark.started || exported > mark.exported || renewed > mark.renewed;
        if seen.off.as_ref().is_some_and(newer) {
            seen.off = None;
        }
        if seen.forgotten.as_ref().is_some_and(newer) {
            seen.forgotten = None;
        }
        seen.peer.off = seen.off.is_some() || seen.forgotten.is_some();
    }
}

/// A device you say is off (`off`), or not any more: not counted for the
/// doses until it says anything newer (a new start, a new export, a claim),
/// said plainly meanwhile (`shared_note`, Settings). It never answers a dose
/// for you. Returns what went wrong, else "".
pub(crate) fn device_off(id: &str, off: bool) -> String {
    mark_device(id, |seen, mark| seen.off = off.then_some(mark))
}

/// A device silent for a week, forgotten: not listed until it says anything newer.
pub(crate) fn device_forget(id: &str) -> String {
    // Forgotten by the sharing too: it no longer holds its newer form back (docs/database.md).
    crate::share::forget_device(id);
    mark_device(id, |seen, mark| seen.forgotten = Some(mark))
}

fn mark_device(id: &str, set: impl FnOnce(&mut PeerSeen, Mark)) -> String {
    let id = id.trim();
    if id.is_empty() {
        return String::new();
    }
    // What it says now, learned first: the mark is lifted by anything newer than that only.
    let _ = know();
    let _held = KNOWING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    sioul_core::filelock::with_lock(&peers_path(), || {
        let mut kept = load_peers();
        let seen = kept.peers.entry(id.to_string()).or_default();
        seen.peer.id = id.to_string();
        let mark = Mark { started: seen.entry.as_ref().map_or(0, |e| e.started), exported: seen.entry.as_ref().map_or(0, |e| e.exported), renewed: seen.renewed, at: Timestamp::now().as_second() };
        set(seen, mark);
        save_peers(&kept)
    })
}

/// A device as Settings ▸ Your folder and sharing lists it (`share::status`).
#[derive(Serialize, Debug)]
pub(crate) struct DeviceRow {
    id: String,
    name: String,
    /// How it is, in words: in use now, closed at 22:14, silent since…, off as you said, an older Sioul.
    state: String,
    /// Said under it, quietly: it does not share its doses; its clock ahead.
    notes: Vec<String>,
    /// Counted as off, as you said: "Count it again".
    off: bool,
    /// Silent for a week: "Forget this device".
    forget: bool,
    /// The Sioul it runs, as its entry says it: "Sioul 0.0.3 (eff8661abcde)", said
    /// calmly when older than this device's; "" when its entry does not say.
    build: String,
}

/// Your other devices, as this one knows them, for Settings: nothing red, no
/// counts. `holders`: the devices that hold the newer form of what travels
/// back, or run an older Sioul once it travels (`share::formats`), with when
/// each was last heard (seconds, 0 when nothing says it) and whether it holds
/// it back; each is listed (`holder_rows`).
pub(crate) fn device_rows(holders: &[(String, i64, bool)]) -> Vec<DeviceRow> {
    let knowledge = know();
    let kept = load_peers();
    let mut rows = device_rows_of(&knowledge.peers, &kept, Timestamp::now().as_second(), tr());
    holder_rows(&mut rows, holders, tr());
    rows
}

/// Every device holding the newer form back has its line, so that the window
/// says which (docs/database.md, "The format of what travels"): one the rows
/// do not list (known to the sharing alone, by what it shared before, or
/// forgotten here while the sharing heard it since) is added, with "Forget
/// this device": it is not counted for your doses, so forgetting it makes no
/// dose look known. One listed keeps its own line: forgotten only once silent
/// for a week, as any device, since a device counted for your doses and
/// forgotten could make a dose it took look not taken.
fn holder_rows(rows: &mut Vec<DeviceRow>, holders: &[(String, i64, bool)], words: &Translator) {
    let zone = jiff::tz::TimeZone::system();
    let when = |at: i64| Timestamp::from_second(at).map(|t| when_said(&t.to_zoned(zone.clone()), words)).unwrap_or_default();
    for (id, heard, holds) in holders {
        if id.is_empty() || rows.iter().any(|row| row.id == *id) {
            continue;
        }
        let state = match (holds, *heard > 0) {
            (true, true) => said(words, "share-device-holds", &[("when", when(*heard))]),
            (true, false) => words.text("share-device-holds-unknown", None),
            (false, true) => said(words, "share-device-older", &[("when", when(*heard))]),
            (false, false) => words.text("share-device-unread", None),
        };
        rows.push(DeviceRow { id: id.clone(), name: capitalized(&words.text("share-other-device", None)), state, notes: Vec::new(), off: false, forget: true, build: String::new() });
    }
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
}

fn device_rows_of(peers: &[Peer], kept: &Peers, now: i64, words: &Translator) -> Vec<DeviceRow> {
    let zone = jiff::tz::TimeZone::system();
    let when = |at: i64| Timestamp::from_second(at).map(|t| when_said(&t.to_zoned(zone.clone()), words)).unwrap_or_default();
    let mut rows: Vec<DeviceRow> = peers
        .iter()
        .filter(|peer| !peer.id.is_empty() && kept.peers.get(&peer.id).is_none_or(|seen| seen.forgotten.is_none()))
        .map(|peer| {
            let seen = kept.peers.get(&peer.id);
            let entry = seen.and_then(|s| s.entry.as_ref()).filter(|_| peer.said.is_some());
            let named = named_of(peer);
            let name = match entry {
                Some(e) if e.kind == sioul_sync::devices::PHONE && !e.name.is_empty() && e.name != sioul_sync::devices::PHONE => format!("{} ({})", capitalized(&words.text("device-the-phone", None)), e.name),
                _ => device_start(&named, words),
            };
            let off = peer.off;
            let counted = sioul_core::health::counts(&Peer { off: false, ..peer.clone() }, now);
            let state = if off {
                words.text("share-device-off", None)
            } else if !counted {
                said(words, "share-device-silent", &[("when", when(peer.heard))])
            } else {
                match entry {
                    Some(e) if e.working && now - peer.seen > sioul_core::health::QUIET => said(words, "share-device-quiet", &[("when", when(e.exported))]),
                    Some(e) if e.working => said(words, "share-device-in-use", &[("when", when(e.exported))]),
                    Some(e) => said(words, "share-device-closed", &[("when", when(e.closed)), ("shared", when(e.exported))]),
                    None if seen.is_some_and(|s| s.renewed > 0) => said(words, "share-device-older", &[("when", when(peer.heard))]),
                    None => words.text("share-device-unread", None),
                }
            };
            let mut notes = Vec::new();
            if entry.is_some_and(|e| !e.doses) {
                notes.push(words.text("share-device-apart", None));
            }
            if peer.ahead >= 60 {
                let mut args = sioul_core::i18n::args();
                args.set("minutes", peer.ahead / 60);
                notes.push(words.text("share-device-ahead", Some(&args)));
            }
            // The build it runs (docs/database.md, "Devices"): an older one said, calmly.
            let build = match entry.map(|e| (e.build(), sioul_core::build::older(&e.version, sioul_core::build::VERSION))) {
                Some((build, _)) if build.is_empty() => String::new(),
                Some((build, true)) => said(words, "share-device-build-older", &[("build", build), ("here", sioul_core::build::DESCRIBED.to_string())]),
                Some((build, false)) => said(words, "share-device-build", &[("build", build)]),
                None => String::new(),
            };
            DeviceRow { id: peer.id.clone(), name, state, notes, off, forget: !counted && !off, build }
        })
        .collect();
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    rows
}

/// Why a dose is not known, in words: "your laptop: last heard at 07:52".
fn why(doubts: &[Doubt]) -> String {
    why_in(doubts, tr())
}

/// The same, in `words`' language (the tests give theirs).
fn why_in(doubts: &[Doubt], words: &Translator) -> String {
    let zone = jiff::tz::TimeZone::system();
    let when = |at: i64| Timestamp::from_second(at).map(|t| when_said(&t.to_zoned(zone.clone()), words)).unwrap_or_default();
    let named = |device: &sioul_core::health::Named| device_word(device, words);
    let parts: Vec<String> = doubts
        .iter()
        .map(|doubt| match doubt {
            Doubt::Record { since } => said(words, "dose-doubt-record", &[("when", when(*since))]),
            Doubt::Unheard { device, until: 0, .. } => said(words, "dose-doubt-never", &[("name", named(device))]),
            Doubt::Unheard { device, until, closed: true } => said(words, "dose-doubt-closed", &[("name", named(device)), ("when", when(*until))]),
            Doubt::Unheard { device, until, closed: false } => said(words, "dose-doubt-open", &[("name", named(device)), ("when", when(*until))]),
            Doubt::Broken { device } => said(words, "dose-doubt-broken", &[("name", named(device))]),
            Doubt::Working { device, shared, quiet: false } => said(words, "dose-doubt-working", &[("name", named(device)), ("when", when(*shared))]),
            Doubt::Working { device, shared, quiet: true } => said(words, "dose-doubt-quiet", &[("name", named(device)), ("when", when(*shared))]),
            Doubt::Coming { device, shared } => said(words, "dose-doubt-coming", &[("name", named(device)), ("when", when(*shared))]),
            Doubt::Apart { device } => said(words, "dose-doubt-apart", &[("name", named(device))]),
        })
        .collect();
    parts.join("; ")
}

/// A moment as a doubt says it: "at 07:45" today, "on Monday 5 October at 07:45" before.
fn when_said(at: &Zoned, words: &Translator) -> String {
    if at.date() == Zoned::now().with_time_zone(at.time_zone().clone()).date() { said(words, "when-at", &[("time", at.strftime("%H:%M").to_string())]) } else { words.when(at) }
}

/// A device as a doubt names it: "the phone", else its name; one whose name
/// is not known (an older Sioul on a phone says "localhost"), "another device of yours".
fn device_word(device: &sioul_core::health::Named, words: &Translator) -> String {
    if device.phone {
        words.text("device-the-phone", None)
    } else if device.name.trim().is_empty() || device.name == "localhost" || device.name == "?" {
        words.text("share-other-device", None)
    } else {
        device.name.clone()
    }
}

/// A message with its words given, in `words`' language (`say`, for any translator).
fn said(words: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = sioul_core::i18n::args();
    for (key, value) in pairs {
        args.set(key.to_string(), value.clone());
    }
    words.text(id, Some(&args))
}

/// A dose's doubt as its reminder says it, at its time; "" when it is known.
fn doubt_of(knowledge: &Knowledge, due: i64, now: i64) -> String {
    doubt_unread(knowledge, due, now, None)
}

/// The same, at an alarm whose pull did not go through since it began
/// (`share::news_missed`: when, and the devices whose entry or health claim
/// says more since): any other known closed by what was read before is said in doubt,
/// as last heard (`sioul_core::health::doubts_unread`).
fn doubt_unread(knowledge: &Knowledge, due: i64, now: i64, missed: Option<&(i64, std::collections::BTreeSet<String>)>) -> String {
    let doubts = match missed {
        Some((since, heard)) => sioul_core::health::doubts_unread(due, now, knowledge.record_lost, &knowledge.peers, *since, heard),
        None => sioul_core::health::doubts(due, now, knowledge.record_lost, &knowledge.peers),
    };
    if doubts.is_empty() { String::new() } else { say("dose-doubt", &[("why", why(&doubts))]) }
}

/// A dose's doubt as what stays in view says it (the Porch, the Health page,
/// where a dose shows due hours after its time: as of now,
/// `health::doubts_now`), and a "This device is off" for each device it names
/// (one: "This device is off"; several: each by its name). None for a line
/// that could not be read, nor a device that does not share its doses: its
/// being off would lift neither.
fn doubt_now_with(knowledge: &Knowledge, due: i64, now: i64, words: &Translator) -> (String, Vec<OffButton>) {
    let doubts = sioul_core::health::doubts_now(due, now, knowledge.record_lost, &knowledge.peers);
    if doubts.is_empty() {
        return (String::new(), Vec::new());
    }
    let named: Vec<&Named> = doubts
        .iter()
        .filter_map(|doubt| match doubt {
            Doubt::Working { device, .. } | Doubt::Coming { device, .. } | Doubt::Unheard { device, .. } => Some(device),
            _ => None,
        })
        .filter(|device| !device.id.is_empty())
        .collect();
    let off = match named.as_slice() {
        [one] => vec![OffButton { id: one.id.clone(), label: words.text("device-off", None) }],
        many => many.iter().map(|device| OffButton { id: device.id.clone(), label: said(words, "device-off-named", &[("name", device_start(device, words))]) }).collect(),
    };
    (said(words, "dose-doubt", &[("why", why_in(&doubts, words))]), off)
}

/// A device named at a sentence's start: "The phone", "Another device of yours", else its name as it is.
fn device_start(device: &Named, words: &Translator) -> String {
    let word = device_word(device, words);
    if word == device.name { word } else { capitalized(&word) }
}

/// A device as a doubt names it, from what is known of it.
fn named_of(peer: &Peer) -> Named {
    Named { id: peer.id.clone(), name: peer.name.clone(), phone: peer.said.as_ref().is_some_and(|s| s.phone) }
}

/// Answers that differ, each said, the earliest first: "Marked taken on the
/// phone at 08:02, skipped here at 08:10. Check which is right before taking
/// it." `me`: this device's id, said "here".
fn differ_sentence(given: &[Given], me: &str, words: &Translator) -> String {
    let zone = jiff::tz::TimeZone::system();
    let parts: Vec<String> = given
        .iter()
        .map(|g| {
            let when = Timestamp::from_second(g.at).map(|t| when_said(&t.to_zoned(zone.clone()), words)).unwrap_or_default();
            let name = device_word(&Named { id: g.device.clone(), name: g.name.clone(), phone: g.kind == sioul_sync::devices::PHONE }, words);
            let here = !me.is_empty() && g.device == me;
            let id = match (&g.state, g.device.is_empty(), here) {
                (State::Taken, true, _) => "dose-answer-taken",
                (State::Taken, _, true) => "dose-answer-taken-here",
                (State::Taken, _, false) => "dose-answer-taken-on",
                (State::Skipped, true, _) => "dose-answer-skipped",
                (State::Skipped, _, true) => "dose-answer-skipped-here",
                (State::Skipped, _, false) => "dose-answer-skipped-on",
                (State::Other(_), _, _) => "dose-answer-other-on",
            };
            said(words, id, &[("name", name), ("when", when)])
        })
        .collect();
    said(words, "dose-answers", &[("answers", parts.join(", "))])
}

/// The doses' state is read, changed and written by the minute's tick, the
/// page and the notifications' buttons: one at a time, so that none writes
/// back an older state over another's mark (a dose shown untaken could be taken twice).
static STATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn state_held() -> std::sync::MutexGuard<'static, ()> {
    STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One tick at a time: a slow one (a shared folder) is not overtaken by the next.
static TICKING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A dose not taken, said afterwards: asked no more. Returns what went wrong, else "".
pub(crate) fn not_taken(key: &str) -> String {
    let now = Timestamp::now().as_second();
    let problem = change(|state| {
        state.not_taken.insert(key.to_string(), now);
    });
    if !problem.is_empty() {
        return problem;
    }
    // Your answer, in the doses' records too (`sioul_core::doses`).
    record_answer(key, sioul_core::doses::SKIPPED, now, false)
}

/// You chose, seeing answers that differ (`DoseRow::choose`): taken (at the
/// time the earliest "taken" said, else now), or not taken. Your answer here,
/// every answer saying otherwise taken out, on purpose; the marks beside, for
/// an older Sioul, as you chose; not taken after all, the doses its take moved
/// go back. Returns what went wrong, else "".
pub(crate) fn choose(key: &str, taken: bool) -> String {
    let _held = state_held();
    let now = Timestamp::now().as_second();
    let first_taken = match sioul_core::doses::answered(key, &record(), &records()) {
        Answered::Taken(given) => Some(given.at),
        Answered::Differ(given) => given.iter().find(|g| g.state == State::Taken).map(|g| g.at),
        _ => None,
    };
    let at = if taken { first_taken.unwrap_or(now) } else { now };
    let moved = std::sync::Mutex::new(None);
    let problem = change(|state| {
        if taken {
            state.taken.insert(key.to_string(), at);
            state.not_taken.remove(key);
        } else {
            state.taken.remove(key);
            state.not_taken.insert(key.to_string(), now);
            if let Ok(mut moved) = moved.lock() {
                *moved = state.moved.remove(key);
            }
        }
    });
    if let Some([before, after]) = moved.into_inner().ok().flatten() {
        let _ = change_health(|health| Ok(health.taken_back(key, before, after)));
    }
    if !problem.is_empty() {
        return problem;
    }
    record_answer(key, if taken { sioul_core::doses::TAKEN } else { sioul_core::doses::SKIPPED }, at, true)
}

/// When a dose is due, from its key (`<medicine>@<Unix seconds>`).
fn due_of(key: &str) -> Option<i64> {
    key.rsplit_once('@').and_then(|(_, at)| at.parse().ok())
}

/// Whether a dose marked now is late: more than half an hour past its time.
pub(crate) fn is_late(key: &str) -> bool {
    due_of(key).is_some_and(|due| Timestamp::now().as_second() - due > GRACE_MINUTES * 60)
}

/// A dose taken late, at `time` ("09:30", the last such time before now),
/// as you say: marked then; for a medicine taken every few hours, the next
/// doses that many hours after it. Returns what went wrong, else "".
pub(crate) fn taken_late(key: &str, time: &str) -> String {
    let now = Zoned::now();
    let Some(clock) = time.trim().split_once(':').and_then(|(h, m)| jiff::civil::Time::new(h.trim().parse().ok()?, m.trim().parse().ok()?, 0, 0).ok()) else {
        return tr().text("dose-time-wrong", None);
    };
    let mut at = now.date().to_datetime(clock).to_zoned(now.time_zone().clone()).map(|z| z.timestamp().as_second()).unwrap_or(now.timestamp().as_second());
    if at > now.timestamp().as_second() {
        at -= 86_400;
    }
    let _held = state_held();
    // The doses it moves, moved in the medicines as the file holds them now, under its lock.
    let moved = match change_health(|health| Ok(health.taken_at(key, at))) {
        Ok(moved) => moved,
        Err(e) => return e,
    };
    let problem = change(|state| {
        state.taken.insert(key.to_string(), at);
        state.not_taken.remove(key);
        if let Some((before, after)) = moved {
            state.moved.insert(key.to_string(), [before, after]);
        }
    });
    // Your answer, in the doses' records too, at the time you said.
    if problem.is_empty() {
        let _ = record_answer(key, sioul_core::doses::TAKEN, at, false);
    }
    // Not marked: the doses it moved go back, in the medicines as the file holds them then.
    if !problem.is_empty()
        && let Some((before, after)) = moved
    {
        let _ = change_health(|health| Ok(health.taken_back(key, before, after)));
    }
    problem
}

/// What the late dose's question shows, as JSON: {name, dose, due, now, hours}.
pub(crate) fn dose_info(key: &str) -> String {
    let health = load();
    let now = Zoned::now();
    let (Some((id, _)), Some(due)) = (key.rsplit_once('@'), due_of(key)) else { return "null".into() };
    let Some(medicine) = health.medicines.iter().find(|m| m.id == id) else { return "null".into() };
    let due = Timestamp::from_second(due).map(|t| t.to_zoned(now.time_zone().clone())).unwrap_or_else(|_| now.clone());
    let hours = match medicine.schedule {
        Schedule::Hours { hours, .. } => hours,
        _ => 0,
    };
    serde_json::json!({
        "name": medicine.short_name(),
        "dose": health.amount_of(key, now.time_zone()).unwrap_or_else(|| medicine.dose.clone()),
        "due": if due.date() == now.date() { due.strftime("%H:%M").to_string() } else { format!("{} {}", tr().weekday_short(due.date()), due.strftime("%H:%M")) },
        "now": now.strftime("%H:%M").to_string(),
        // Every few hours: the next dose comes that many hours after the one taken.
        "hours": hours,
    })
    .to_string()
}

/// A dose marked taken, or not, as it happens: taken every few hours, the
/// next doses come that many hours after now; a mark taken back puts them
/// back. Returns what went wrong, else "".
pub(crate) fn set_taken(key: &str, taken: bool) -> String {
    let _held = state_held();
    let now = Timestamp::now().as_second();
    let shift = if taken {
        match change_health(|health| Ok(health.taken_at(key, now))) {
            Ok(shift) => shift,
            Err(e) => return e,
        }
    } else {
        None
    };
    let moved = std::sync::Mutex::new(None);
    let problem = change(|state| {
        if taken {
            state.taken.insert(key.to_string(), now);
            if let Some((before, after)) = shift {
                state.moved.insert(key.to_string(), [before, after]);
            }
        } else {
            state.taken.remove(key);
            if let Ok(mut moved) = moved.lock() {
                *moved = state.moved.remove(key);
            }
        }
    });
    if let Some([before, after]) = moved.into_inner().ok().flatten() {
        let _ = change_health(|health| Ok(health.taken_back(key, before, after)));
    }
    // Your answer, in the doses' records too: taken now; taken back, every
    // "taken" goes, on purpose (one click takes it back, wherever it was marked).
    if problem.is_empty() {
        let _ = if taken { record_answer(key, sioul_core::doses::TAKEN, now, false) } else { change_records(|records| records.take_back(key)) };
    }
    problem
}

/// The pause to move and the chats' limit: "movement.enabled", "movement.minutes",
/// "chats.enabled", "chats.minutes", "chats.locked_minutes".
pub(crate) fn set_setting(key: &str, value: &str) -> String {
    let number = || value.trim().parse::<u32>().unwrap_or(0);
    if !matches!(key, "movement.enabled" | "movement.minutes" | "chats.enabled" | "chats.minutes" | "chats.locked_minutes" | "errands_list") {
        return say("setting-unknown-key", &[("key", key.to_string())]);
    }
    change_health(|health| {
        match key {
            "movement.enabled" => health.movement.enabled = value == "true",
            "movement.minutes" => health.movement.minutes = number().clamp(10, 240),
            "chats.enabled" => health.chats.enabled = value == "true",
            "chats.minutes" => health.chats.minutes = number(),
            "chats.locked_minutes" => health.chats.locked_minutes = number(),
            _ => health.errands_list = value.trim().to_string(),
        }
        Ok(())
    })
    .err()
    .unwrap_or_default()
}

/// Minutes of focus before the pause to move; 0 when off.
pub(crate) fn movement_minutes() -> i32 {
    let movement = load().movement;
    if movement.enabled { i32::try_from(movement.minutes).unwrap_or(45) } else { 0 }
}

/// One more minute in a chat; returns whether chats are covered now.
pub(crate) fn chat_minute() -> bool {
    let limit = load().chats;
    let _held = state_held();
    let now = Zoned::now();
    let covered = std::sync::atomic::AtomicBool::new(false);
    let _ = change(|state| covered.store(state.chat_minute(&limit, &now), std::sync::atomic::Ordering::Relaxed));
    covered.into_inner()
}

/// Whether chats are covered now.
pub(crate) fn chats_covered() -> bool {
    record().chats_covered(&Zoned::now())
}

// ---------------------------------------------------------------- meals, rest and sleep

/// A block's usual name, in your language: breakfast, lunch, dinner, then
/// "Meal 4"; a nap; winding down for the night.
pub(crate) fn usual_name(kind: &str, index: usize) -> String {
    match (kind, index) {
        ("meal", 0..=2) => tr().text(&format!("need-meal-{index}"), None),
        ("meal", n) => say("need-meal-n", &[("n", (n + 1).to_string())]),
        ("nap", _) => tr().text("need-nap", None),
        _ => tr().text("need-sleep", None),
    }
}

/// A block's name: its own, else its usual one by its key ("meal:1": lunch);
/// one added for a day without a name of its own: "Meal", "Rest".
pub(crate) fn block_name(kind: &str, key: &str, name: &str) -> String {
    if !name.trim().is_empty() {
        return name.to_string();
    }
    if sioul_core::needs::is_added(key) {
        return tr().text(if kind == "nap" { "need-added-nap" } else { "need-added-meal" }, None);
    }
    usual_name(kind, key.rsplit_once(':').and_then(|(_, i)| i.parse().ok()).unwrap_or(0))
}

fn name_of(kept: &Kept) -> String {
    block_name(kept.kind, &kept.key, &kept.name)
}

/// Each day's own meals, naps and nights (`sioul_core::needs::Days`). The
/// moves and skips of today an older Sioul kept on this device
/// (`needs-today.toml`) are moved into them once, as times.
pub(crate) fn days() -> Days {
    let path = Days::default_path();
    let now = Zoned::now();
    let today_path = sioul_core::needs::Today::default_path();
    let mut today = sioul_core::needs::Today::load(&today_path, now.date());
    if today.shifts.is_empty() && today.skipped.is_empty() {
        return Days::load(&path);
    }
    let zone = now.time_zone().clone();
    let blocks = load().needs.blocks_of(now.date(), &zone, &Days::default(), &|_| 0);
    // Moved in once, over the days as the file holds them then, under its lock.
    let moved = Days::change_file(&path, now.date(), |days| {
        for k in &blocks {
            let shift = today.shifts.get(&k.key).copied().unwrap_or(0) * 60;
            let skipped = today.skipped.contains(&k.key);
            days.change(now.date(), &k.key, |b| {
                if shift != 0 && b.at.is_empty() {
                    b.at = clock(k.at + shift, &zone);
                    if k.kind == "sleep" {
                        b.wake = clock(k.end + shift, &zone);
                    }
                }
                b.quiet |= skipped;
            });
        }
    });
    if moved.is_ok() {
        today.shifts.clear();
        today.skipped.clear();
        let _ = today.save(&today_path);
    }
    Days::load(&path)
}

/// Today's events' held times (their margins counted), read again five
/// minutes after at most: the needs' tick runs every minute.
fn held_today(now: &Zoned) -> Vec<(i64, i64)> {
    static CACHE: std::sync::Mutex<Option<(i64, Date, Vec<(i64, i64)>)>> = std::sync::Mutex::new(None);
    let stamp = now.timestamp().as_second();
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, day, held)) = cache.as_ref()
        && *day == now.date()
        && (0..300).contains(&(stamp - at))
    {
        return held.clone();
    }
    let midnight = now.date().to_zoned(now.time_zone().clone()).map_or(stamp, |z| z.timestamp().as_second());
    let held = sioul_core::plan::event_spans(&sioul_core::agenda::occurrences(midnight, midnight + 26 * 3600), 0);
    *cache = Some((stamp, now.date(), held.clone()));
    held
}

/// `date`'s events' held times: today's from the minute's cache, another day's read now.
fn held_on(date: Date, now: &Zoned) -> Vec<(i64, i64)> {
    if date == now.date() {
        return held_today(now);
    }
    let start = midnight(date, now.time_zone());
    sioul_core::plan::event_spans(&sioul_core::agenda::occurrences(start, start + 26 * 3600), 0)
}

/// A day's own blocks as they are: its changes, its meals pushed past its events.
fn blocks_now(needs: &Needs, days: &Days, date: Date, now: &Zoned) -> Vec<Kept> {
    let pushed = needs.past_events_on(date, now.time_zone(), days, &held_on(date, now));
    needs.blocks_of(date, now.time_zone(), days, &|key: &str| pushed.get(key).copied().unwrap_or(0))
}

/// Each minute, from the computer you are at: a block's heads-up about the
/// work, `heads_up` minutes before it ("No new big task"), with "Later";
/// then one at its time. Two at most, each once, only near its time; none
/// while an event goes on, none for a block quiet or taken out that day. Its
/// name and time only: safe to be read by someone else (docs/health.md).
fn needs_tick(qt: &QtThread, shared: &Arc<Shared>, health: &Health, now: &Zoned) {
    let needs = &health.needs;
    let days = days();
    if !(needs.meals_on || needs.naps_on || needs.sleep_on) && !days.0.contains_key(&now.date()) {
        return;
    }
    let path = sioul_core::needs::Today::default_path();
    let mut today = sioul_core::needs::Today::load(&path, now.date());
    let stamp = now.timestamp().as_second();
    let pushed = needs.past_events_on(now.date(), now.time_zone(), &days, &held_today(now));
    let kept = needs.kept_with(now.date(), now.time_zone(), &days, &|key: &str| pushed.get(key).copied().unwrap_or(0));
    let due = today.due(&kept, needs.heads_up, stamp);
    if due.is_empty() {
        return;
    }
    let _ = today.save(&path);
    // In a meeting, nothing is said: the time stays kept.
    let meeting = sioul_core::agenda::occurrences(stamp - 86_400, stamp + 60).iter().any(|e| !e.cancelled && !e.all_day && e.start <= stamp && e.end > stamp);
    if meeting {
        return;
    }
    let hm = |at: i64| clock(at, now.time_zone());
    // As the matrix of what reaches you says (as usual, nothing while you sleep, in a pause or in Free time).
    let attention = crate::hours::attention();
    let moment = crate::hours::attention_now();
    let needs = sioul_core::attention::Row::Own(sioul_core::attention::Kind::Needs);
    let may = attention.level(needs, &moment) == sioul_core::attention::Level::Now;
    // The night's or a nap's own notice as it begins says that sleep begins:
    // sleep's column does not hold it (docs/health.md); a pause does.
    let mut awake = moment.clone();
    awake.times.retain(|c| *c != sioul_core::attention::Column::Sleep);
    let starting = attention.level(needs, &awake) == sioul_core::attention::Level::Now;
    for (block, heads_up) in due {
        let own_start = !heads_up && (block.kind == "sleep" || block.kind == "nap") && block.start <= stamp;
        if !(may || (own_start && starting)) {
            continue;
        }
        let name = name_of(block);
        // One button: the window's question (later, at another time, not
        // today; a line on where you stopped).
        let (key, qt_open) = (block.key.clone(), qt.clone());
        let open: Box<dyn FnOnce() + Send> = Box::new(move || {
            let _ = qt_open.queue(move |mut sioul| sioul.as_mut().reminder_opened(QString::from("need"), QString::default(), QString::from(&key)));
        });
        let action = Some((tr().text("need-open", None), open));
        let shown = if heads_up {
            sioul_sync::notify::remind(&tr().text("need-heads-up", None), &say("need-at", &[("name", name), ("time", hm(block.start))]), action)
        } else if block.kind == "sleep" && crate::reviews::night_open(block.at, now) {
            // The night's own notice: closing the day is offered beside its options (docs/reviews.md).
            let qt_review = qt.clone();
            let review: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = qt_review.queue(|mut sioul| sioul.as_mut().reminder_opened(QString::from("review"), QString::default(), QString::from("night")));
            });
            let choices = std::iter::once((tr().text("review-close-night", None), review)).chain(action).collect();
            sioul_sync::notify::remind_choices(&name, &hm(block.start), choices)
        } else {
            sioul_sync::notify::remind(&name, &hm(block.start), action)
        };
        if let Err(e) = shown {
            tell(qt, shared, e);
        }
    }
}

/// A day's block changed, that day only, as the page asks (JSON: its `date`,
/// "2026-10-07", today when none, and `sioul_core::needs::DayEdit`: key,
/// action, from, to…; `Needs::change_day`). The plan and the page follow;
/// a block moved is not said again: moving it never brings a nag. Returns
/// what went wrong, else "".
pub(crate) fn change_need(edit: &str) -> String {
    #[derive(Deserialize)]
    struct Asked {
        #[serde(default)]
        date: String,
        #[serde(flatten)]
        edit: DayEdit,
    }
    match serde_json::from_str::<Asked>(edit) {
        Ok(asked) => change_day(&asked.date, &asked.edit),
        Err(e) => e.to_string(),
    }
}

fn change_day(date: &str, edit: &DayEdit) -> String {
    let now = Zoned::now();
    let date = if date.trim().is_empty() { Some(now.date()) } else { date.trim().parse::<Date>().ok() };
    let Some(date) = date else { return tr().text("need-times-wrong", None) };
    // Changed over the days as the file holds them now, under its lock, which the sharing takes too.
    let _ = days();
    let needs = load().needs;
    let held = held_on(date, &now);
    let changed = match Days::change_file(&Days::default_path(), now.date(), |days| needs.change_day(days, date, edit, &now, &held)) {
        Ok(changed) => changed,
        Err(e) => return e,
    };
    match changed {
        Ok(()) => {
            // A night changed, or its alarm: a phone's alarm at waking follows (`wake`).
            crate::wake::schedule();
            String::new()
        }
        Err(DayProblem::Past) => tr().text("need-past", None),
        Err(DayProblem::Times) => tr().text("need-times-wrong", None),
        Err(DayProblem::TooShort) => tr().text("need-too-short", None),
        Err(DayProblem::OtherDay) => tr().text("need-other-day", None),
        Err(DayProblem::Unknown) => say("setting-unknown-key", &[("key", edit.action.clone())]),
    }
}

/// A day's block moved by a drag on the page's timeline (`change_need`'s
/// JSON: "move", or "times" for one of its ends), changed as "Move to…" and
/// "Change its times…" change it, then "Undo" offered for ten seconds, which
/// puts that day's block back as it was. Dropped where it was: nothing
/// changes, nothing to undo. Returns what went wrong, else "".
pub(crate) fn drag_need(qt: &QtThread, shared: &Arc<Shared>, edit: &str) -> String {
    #[derive(Deserialize)]
    struct Asked {
        #[serde(default)]
        date: String,
        #[serde(flatten)]
        edit: DayEdit,
    }
    let asked = match serde_json::from_str::<Asked>(edit) {
        Ok(asked) => asked,
        Err(e) => return e.to_string(),
    };
    let now = Zoned::now();
    let date = if asked.date.trim().is_empty() { Some(now.date()) } else { asked.date.trim().parse::<Date>().ok() };
    let Some(date) = date else { return tr().text("need-times-wrong", None) };
    let key = asked.edit.key.clone();
    let was = days().get(date, &key).cloned();
    let problem = change_day(&asked.date, &asked.edit);
    if !problem.is_empty() {
        return problem;
    }
    let after = days();
    if after.get(date, &key) == was.as_ref() {
        return String::new();
    }
    // Said as the day has it now: its name, its time kept.
    let zone = now.time_zone().clone();
    let line = blocks_now(&load().needs, &after, date, &now)
        .iter()
        .find(|k| k.key == key)
        .map(|k| say("drag-done", &[("what", name_of(k)), ("time", format!("{}–{}", clock(k.start, &zone), clock(k.end, &zone)))]))
        .unwrap_or_else(|| tr().text("drag-done-plain", None));
    crate::mail::offer_back(qt, shared, line, move |qt, shared| {
        // Put back over the days as the file holds them now, under its lock, which the sharing takes too.
        let _ = days();
        let problem = Days::change_file(&Days::default_path(), Zoned::now().date(), |days| match &was {
            Some(block) => days.change(date, &key, |b| *b = block.clone()),
            None => days.forget(date, &key),
        })
        .err()
        .unwrap_or_default();
        // A night put back: a phone's alarm at waking follows (`wake`).
        crate::wake::schedule();
        crate::work::show_work(qt, shared);
        show_health(qt, shared);
        problem
    });
    String::new()
}

/// A block moved today only (the question, a notice's "Later"): by `minutes`
/// more ("Later": by the minutes set, again and again), or to start at `time`
/// ("13:30"). Returns what went wrong, else "".
pub(crate) fn move_today(key: &str, minutes: i64, time: &str) -> String {
    let action = if time.is_empty() { "later" } else { "move" };
    change_day("", &DayEdit { key: key.to_string(), action: action.to_string(), minutes, from: time.to_string(), ..DayEdit::default() })
}

/// Today's meals, naps and night as they are now, as JSON: [{key, kind,
/// name, from, to, skipped, past}], for the question (`Interruption.qml`).
pub(crate) fn needs_today() -> String {
    let needs = load().needs;
    let now = Zoned::now();
    let days = days();
    let zone = now.time_zone().clone();
    let stamp = now.timestamp().as_second();
    let rows: Vec<serde_json::Value> = blocks_now(&needs, &days, now.date(), &now)
        .iter()
        .map(|k| serde_json::json!({ "key": k.key, "kind": k.kind, "name": name_of(k), "from": clock(k.start, &zone), "to": clock(k.end, &zone), "skipped": days.get(now.date(), &k.key).is_some_and(|c| c.quiet), "past": k.end <= stamp }))
        .collect();
    json(&rows)
}

/// The minutes "Later" moves a block by.
pub(crate) fn later_minutes() -> u32 {
    load().needs.later
}

/// A block without notices today, or with them again: kept free all the same.
pub(crate) fn skip_today(key: &str, skip: bool) -> String {
    change_day("", &DayEdit { key: key.to_string(), action: if skip { "quiet" } else { "loud" }.to_string(), ..DayEdit::default() })
}

/// The usual meals, naps and night as the page's settings set them, as
/// JSON: the settings, the usual names, the long gaps between meals.
pub(crate) fn needs_page() -> String {
    let needs = load().needs;
    serde_json::json!({
        "needs": needs,
        "usual": {
            "meals": (0..needs.meals.len().max(3) + 1).map(|i| usual_name("meal", i)).collect::<Vec<_>>(),
            "nap": usual_name("nap", 0),
            "sleep": usual_name("sleep", 0),
        },
        "gaps": needs.long_gaps().into_iter().map(|(from, to)| say("need-gap", &[("from", from), ("to", to)])).collect::<Vec<_>>(),
        // The alarm at waking: its weekdays' names, the next ring, what Android refuses it.
        "wake": crate::wake::status(&needs),
    })
    .to_string()
}

/// Where a list lost one block: the place taken out, when the rest is the same.
fn removed_at(old: &[sioul_core::needs::Block], new: &[sioul_core::needs::Block]) -> Option<usize> {
    if new.len() + 1 != old.len() {
        return None;
    }
    let at = (0..old.len()).find(|&i| i == new.len() || old[i] != new[i])?;
    (old[at + 1..] == new[at..]).then_some(at)
}

/// The usual meals, naps and night saved as the settings give them; a meal
/// or a nap taken out, the days' changes of those after it follow them to
/// their new place. Returns what went wrong, else "".
pub(crate) fn save_needs(edit: &str, shown: &str) -> String {
    let needs: Needs = match serde_json::from_str(edit) {
        Ok(needs) => needs,
        Err(e) => return e.to_string(),
    };
    // What the page showed: its change set over the settings as the file holds them now.
    let shown: Option<Needs> = serde_json::from_str(shown).ok();
    let removed = change_health(|health| {
        let needs = shown.as_ref().map_or_else(|| needs.clone(), |shown| needs.rebased(shown, &health.needs));
        let removed = (removed_at(&health.needs.meals, &needs.meals), removed_at(&health.needs.naps, &needs.naps));
        health.needs = needs;
        Ok(removed)
    });
    let (meal, nap) = match removed {
        Ok(removed) => removed,
        Err(e) => return e,
    };
    if meal.is_none() && nap.is_none() {
        return String::new();
    }
    // A meal or a nap taken out: the days' changes of those after it move up a place.
    Days::change_file(&Days::default_path(), Zoned::now().date(), |days| {
        if let Some(at) = meal {
            days.removed_usual("meal", at);
        }
        if let Some(at) = nap {
            days.removed_usual("nap", at);
        }
    })
    .err()
    .unwrap_or_default()
}

/// Each minute: a dose due is reminded once, quietly, with "Taken"; a refill
/// or a renewal coming becomes a task in a list kept on this computer.
pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    let Some(_ticking) = crate::backend::one_at_a_time(&TICKING) else { return };
    let health = load();
    if health.medicines.is_empty() && health.prescriptions.is_empty() {
        // No medicines: the pause to move, meals and rest, from the computer you are at.
        let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
        let (keeper, _) = crate::share::keeper("health", sioul_sync::lease::Rule::FollowsYou, active, false);
        movement_tick(qt, shared, &health, &Zoned::now(), keeper.mine);
        if keeper.mine && keeper.settled {
            needs_tick(qt, shared, &health, &Zoned::now());
        }
        // On a phone, the alarm at waking (`wake`; with medicines, `alarms::schedule` below hands it).
        crate::wake::schedule();
        return;
    }
    // One computer reminds: the one you are at, once it has been so long
    // enough for the others to know (`sioul_sync::lease`). A dose reminded
    // twice could be taken twice.
    let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
    let (keeper, _) = crate::share::keeper("health", sioul_sync::lease::Rule::FollowsYou, active, false);
    let _held = state_held();
    let state = record();
    // A dose taken late moved the next ones; the medicine, changed meanwhile
    // on another device, came back with its start as it was: moved again,
    // and sent (review of 5 October 2026, F23).
    let health = if health.clone().mend_shifts(&state) && change_health(|h| Ok(h.mend_shifts(&state))).is_ok() { load() } else { health };
    let knowledge = know();
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    // Each dose due lately, answered nowhere this device knows: its record
    // opened, not taken yet (the canary), for your devices to answer.
    open_due(&health, &state, &now);
    if let Ok(mut there) = REMINDED_THERE.lock() {
        *there = if keeper.mine { String::new() } else { say("health-reminded-there", &[("computer", keeper.name.clone())]) };
    }
    movement_tick(qt, shared, &health, &now, keeper.mine);
    if keeper.mine && keeper.settled {
        needs_tick(qt, shared, &health, &now);
    }
    let mut reminded: Vec<String> = Vec::new();
    let mut unshown: Vec<String> = Vec::new();
    // On a phone, Android's alarm clock reminds, Sioul shown or not (`alarms`).
    crate::alarms::schedule();
    // Asleep with "Doses during sleep: stay silent" (Settings ▸ Reminders and
    // notifications), they wait for waking; by default they come, asleep or not.
    if keeper.mine && keeper.settled && !cfg!(target_os = "android") && !crate::hours::doses_silent() {
        // Those that waited come at waking, once (more than half an hour late, Taken asks when).
        let window = crate::hours::woke_from().map_or(GRACE_MINUTES, |since| GRACE_MINUTES.max((stamp - since) / 60 + 1));
        for dose in state.to_remind(&health, &now, window) {
            if SENT.lock().is_ok_and(|sent| sent.contains(&dose.key)) {
                continue;
            }
            // Not known whether it was taken on another device: news is waited
            // for a while; then it is reminded all the same, the doubt said first.
            let due = dose.at.timestamp().as_second();
            let doubt = doubt_of(&knowledge, due, stamp);
            if !doubt.is_empty() && stamp < due + WAIT_FOR_NEWS {
                continue;
            }
            if let Ok(mut sent) = SENT.lock() {
                sent.insert(dose.key.clone());
            }
            let named = named(&dose);
            let (title, body) = if doubt.is_empty() {
                (named, dose.at.strftime("%H:%M").to_string())
            } else {
                (say("dose-check-title", &[("dose", named)]), format!("{}. {doubt}", dose.at.strftime("%H:%M")))
            };
            let key = dose.key.clone();
            let (qt_taken, shared_taken) = (qt.clone(), Arc::clone(shared));
            let taken: Box<dyn FnOnce() + Send> = Box::new(move || {
                // Pressed more than half an hour late: when it was taken is asked, in the window.
                if is_late(&key) {
                    let _ = qt_taken.queue(move |mut sioul| sioul.as_mut().reminder_opened(QString::from("dose"), QString::default(), QString::from(&key)));
                    return;
                }
                let problem = set_taken(&key, true);
                if !problem.is_empty() {
                    tell(&qt_taken, &shared_taken, problem);
                }
                // Your other computers know at once.
                crate::share::exchange(&qt_taken, &shared_taken);
            });
            // Recorded reminded only when shown: else another device reminds it,
            // and the question on the past still asks about it, saying that its
            // reminder could not be shown (`unshown`), never that Sioul was closed.
            match sioul_sync::notify::remind(&title, &body, Some((tr().text("health-taken", None), taken))) {
                Ok(()) => reminded.push(dose.key.clone()),
                Err(e) => {
                    unshown.push(dose.key.clone());
                    tell(qt, shared, e);
                }
            }
        }
    }
    // Doses due while Sioul ran nowhere, or whose reminder could not be shown:
    // asked about once, as a question on the past, after your other computers
    // were heard from (a dose marked there comes first), and only where you
    // are; its title says which (`missed_title`); what is not known, said.
    let heard = !crate::share::on() || crate::share::last_exchange().is_some();
    // On a phone, without notifications, the Porch asks it (`missed`).
    // Asleep with doses staying silent, it waits for waking too.
    if heard && keeper.mine && !cfg!(target_os = "android") && !crate::hours::doses_silent() && !ASKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let missed = state.unanswered(&health, &now, MISSED_HOURS, GRACE_MINUTES);
        if !missed.is_empty() {
            let names: Vec<String> = missed.iter().map(|d| format!("{} {}", d.name, d.at.strftime("%H:%M"))).collect();
            let qt_open = qt.clone();
            let open: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = qt_open.queue(|mut sioul| sioul.as_mut().reminder_opened(QString::from("porch"), QString::default(), QString::default()));
            });
            let doubts: Vec<String> = missed.iter().map(|d| doubt_of(&knowledge, d.at.timestamp().as_second(), stamp)).filter(|d| !d.is_empty()).collect();
            let mut body = say("health-missed-body", &[("doses", names.join(", "))]);
            if let Some(doubt) = doubts.first() {
                body = format!("{body} {doubt}");
            }
            if let Err(e) = sioul_sync::notify::remind(&missed_title(&missed, &state, tr()), &body, Some((tr().text("health-missed-open", None), open))) {
                tell(qt, shared, e);
            }
        }
    }
    // Errands: a task each, the day it comes into view, made once, in the list
    // the phone has; those made on this computer before move there, once.
    let target = errands_list(&health);
    let mut made: Vec<(String, String)> = Vec::new();
    for errand in health.errands().into_iter().filter(|e| e.day <= now.date().checked_add(Span::new().days(7)).unwrap_or(now.date())) {
        if state.errands.contains_key(&errand.key) {
            continue;
        }
        let title = say(if errand.kind == ErrandKind::Refill { "health-errand-refill" } else { "health-errand-renew" }, &[("title", errand.title.clone())]);
        let edit = TaskEdit { title, start: errand.day.to_string(), categories: vec![tr().text("health-category", None)], estimate: 30, ..TaskEdit::default() };
        let task = match &target {
            Some(list) => work::create_task(qt, shared, &edit, list),
            None => work::local_task(qt, shared, &tr().text("health-list", None), &edit),
        };
        if let Ok(uid) = task {
            made.push((errand.key, uid));
        }
    }
    if let Some(list) = target.as_ref().filter(|l| !l.starts_with(&format!("{}/", sioul_core::vdir::LOCAL))) {
        let loaded = work::loaded(shared);
        for uid in state.errands.values().chain(made.iter().map(|(_, uid)| uid)) {
            let here = loaded.tasks.iter().find(|t| &t.uid == uid).is_some_and(|t| t.status.is_open() && t.list_id.starts_with(&format!("{}/", sioul_core::vdir::LOCAL)));
            if here {
                let _ = work::move_task(qt, shared, uid, list, true);
            }
        }
    }
    // What this minute did, written over nothing anyone else wrote meanwhile.
    let problem = change(|record| note_minute(record, &reminded, &unshown, &made, stamp));
    if !problem.is_empty() && (!reminded.is_empty() || !unshown.is_empty() || !made.is_empty()) {
        tell(qt, shared, problem);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_sync::lease::Claim;

    fn claim(renewed: i64, wrote: Option<(u32, u64)>, closed: bool) -> Claim {
        Claim { computer: "laptop-id".into(), name: "laptop".into(), since: 0, renewed, until: renewed + 300, active: renewed, taken: 0, wrote, closed, pad: String::new() }
    }

    fn heard_up_to(n: u64) -> sioul_sync::share::Heard {
        sioul_sync::share::Heard { read: [("laptop-id".to_string(), (1, n))].into(), broken: Default::default() }
    }

    #[test]
    fn what_the_others_claims_teach() {
        let mut kept = Peers::default();
        // Seen for the first time, an old claim: known until then, but no delay measured from it.
        learn(&mut kept, &[claim(1_000, Some((1, 5)), false)], &[], &heard_up_to(5), 9_000);
        let peer = &kept.peers["laptop-id"].peer;
        assert_eq!((peer.known_until, peer.delay, peer.name.as_str()), (1_000, None, "laptop"));
        // Watching, each new claim times how late it came.
        learn(&mut kept, &[claim(9_030, Some((1, 6)), false)], &[], &heard_up_to(6), 9_060);
        learn(&mut kept, &[claim(9_090, Some((1, 6)), false)], &[], &heard_up_to(6), 9_120);
        assert_eq!(kept.peers["laptop-id"].peer.delay, Some(30));
        // A record it wrote not read here yet: known only until the claim before.
        learn(&mut kept, &[claim(9_150, Some((1, 9)), false)], &[], &heard_up_to(6), 9_180);
        assert_eq!(kept.peers["laptop-id"].peer.known_until, 9_090);
        // Read, and it says it closed: known until then, closed.
        learn(&mut kept, &[claim(9_200, Some((1, 9)), true)], &[], &heard_up_to(9), 9_240);
        let peer = &kept.peers["laptop-id"].peer;
        assert!(peer.closed && peer.known_until == 9_200);
        // Its news comes within a minute: a dose due after it closed is known.
        assert!(sioul_core::health::doubts(9_600, 9_700, None, std::slice::from_ref(peer)).is_empty());
        // An older Sioul that never says how far it wrote: heard, never known.
        let mut old = Peers::default();
        learn(&mut old, &[claim(9_000, None, false)], &[], &heard_up_to(99), 9_030);
        assert_eq!(old.peers["laptop-id"].peer.known_until, 0);
        assert_eq!(sioul_core::health::doubts(9_010, 9_030, None, &[old.peers["laptop-id"].peer.clone()]).len(), 1);
    }

    /// F13: an older Sioul's claim dated ahead of this device's clock (its
    /// clock ten minutes fast) is never known past now: what it may mark in
    /// those ten minutes is not known here yet. A little ahead: until now.
    #[test]
    fn a_claim_from_a_clock_ahead_is_never_known_past_now() {
        let mut kept = Peers::default();
        learn(&mut kept, &[claim(9_600, Some((1, 5)), false)], &[], &heard_up_to(5), 9_000);
        assert_eq!(kept.peers["laptop-id"].peer.known_until, 0, "ten minutes ahead: not known");
        assert_eq!(sioul_core::health::doubts(9_000, 9_000, None, &[kept.peers["laptop-id"].peer.clone()]).len(), 1);
        learn(&mut kept, &[claim(9_630, Some((1, 5)), false)], &[], &heard_up_to(5), 9_600);
        assert_eq!(kept.peers["laptop-id"].peer.known_until, 9_600, "half a minute ahead: until now");
    }

    /// The page's medicines and prescriptions, without their words (those
    /// need the translator, which reads the configuration): times as the
    /// day's list writes them, what comes for each prescription and when,
    /// the medicines it covers when its title does not name them.
    #[test]
    fn medicines_and_prescriptions_on_the_page() {
        let day = |text: &str| text.parse::<Date>().unwrap();
        let today = day("2026-10-06");
        assert_eq!((hhmm("8:00"), hhmm(" 18:30 "), hhmm("noon"), hhmm("25:00")), ("08:00".into(), "18:30".into(), "noon".into(), "25:00".into()));
        assert_eq!(words(&Schedule::Day { times: vec!["20:00".into(), "8:00".into(), "08:00".into()], amounts: Default::default() }), "08:00 · 20:00");
        let medicine = |id: &str, prescription: Option<&str>| Medicine {
            id: id.into(),
            name: id.into(),
            dose: String::new(),
            schedule: Schedule::Day { times: vec!["07:30".into()], amounts: Default::default() },
            prescription: prescription.map(Into::into),
            until: None,
            paused: false,
            generic: String::new(),
            strength: String::new(),
            since: None,
        };
        let health = Health {
            prescriptions: vec![
                // Fetched today, 28 days at a time, valid until February.
                Prescription { id: "thyroid".into(), title: "Thyroid".into(), until: Some(day("2027-02-04")), refill_days: Some(28), last_refill: Some(today), ..Default::default() },
                // Its title names its medicine; its last valid day passed.
                Prescription { id: "levo".into(), title: "Levothyroxine 75 µg".into(), until: Some(day("2026-10-01")), ..Default::default() },
                // Last fetched long ago: no pharmacy visit said from three months back.
                Prescription { id: "old".into(), title: "Old".into(), refill_days: Some(30), last_refill: Some(day("2026-06-01")), ..Default::default() },
            ],
            medicines: vec![medicine("Levothyroxine", Some("levo")), medicine("Magnesium", Some("thyroid")), medicine("Iron", Some("thyroid")), medicine("Zinc", None)],
            ..Default::default()
        };
        let errands = health.errands();
        // Two days before they run out (6 October + 28 days), then the renewal.
        assert_eq!(coming(&errands, &health.prescriptions[0], today), [(Coming::Pharmacy, day("2026-11-01")), (Coming::RenewBy, day("2027-02-04"))]);
        assert_eq!(coming(&errands, &health.prescriptions[1], today), [(Coming::ValidUntil, day("2026-10-01"))]);
        assert!(coming(&errands, &health.prescriptions[2], today).is_empty());
        assert_eq!(covered(&health, &health.prescriptions[0]), ["Magnesium", "Iron"]);
        assert!(covered(&health, &health.prescriptions[1]).is_empty() && covered(&health, &health.prescriptions[2]).is_empty());
    }

    fn take(time: &str, amount: &str) -> Take {
        Take { time: time.into(), amount: amount.into() }
    }

    fn prescription_edit(medicines: serde_json::Value) -> PrescriptionEdit {
        serde_json::from_value(serde_json::json!({ "title": "Thyroid and iron", "prescriber": "Dr Martin", "refill_days": 30, "medicines": medicines })).unwrap()
    }

    /// The prescription's form: its medicines as rows, each with its takes;
    /// a row added, changed, taken out; what cannot be saved changes nothing.
    #[test]
    fn a_prescriptions_medicines_and_their_takes_saved_from_its_form() {
        let english = Translator::new("en");
        let now: Zoned = "2026-10-08T09:00[Europe/Paris]".parse().unwrap();
        let mut health = Health::default();
        // Levothyroxine made on its own form first, every morning.
        let levo: MedicineEdit = serde_json::from_value(serde_json::json!({ "name": "Levothyroxine", "dose": "75 µg", "every": "day", "takes": [{ "time": "07:30", "amount": "" }] })).unwrap();
        let levo = apply_medicine(&mut health, "", levo, &now, &english).unwrap();
        // A new prescription with three medicines: Levothyroxine tied to it, two added there; and a row left empty.
        let rows = serde_json::json!([
            { "id": levo, "name": "Levothyroxine", "dose": "75 µg", "takes": [{ "time": "07:30", "amount": "" }] },
            { "id": "", "name": "Iron", "dose": "1 tablet", "takes": [{ "time": "08:00", "amount": "" }, { "time": "13:00", "amount": "" }, { "time": "20:00", "amount": "2 tablets" }] },
            { "id": "", "name": "Magnesium", "dose": "300 mg", "takes": [{ "time": "21:30", "amount": "" }, { "time": "12:30", "amount": "150 mg" }] },
            { "id": "", "name": "", "dose": "", "takes": [{ "time": "08:00", "amount": "" }] },
        ]);
        let id = apply_prescription(&mut health, "", prescription_edit(rows), &english).unwrap();
        assert_eq!(health.prescriptions.len(), 1);
        assert_eq!(health.medicines.iter().map(|m| (m.name.as_str(), m.prescription.as_deref())).collect::<Vec<_>>(), [("Levothyroxine", Some(id.as_str())), ("Iron", Some(id.as_str())), ("Magnesium", Some(id.as_str()))]);
        let iron = health.medicines.iter().find(|m| m.name == "Iron").unwrap().clone();
        assert_eq!(iron.takes(), [take("08:00", "1 tablet"), take("13:00", "1 tablet"), take("20:00", "2 tablets")]);
        // A medicine like any other: on the page, its prescription form's rows, the doses.
        let today = now.date();
        assert_eq!(tied(&health, &health.prescriptions[0]).map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Levothyroxine", "Iron", "Magnesium"]);
        let row = medicine_row(&iron, today);
        assert_eq!((row.when.as_str(), row.amount.as_str(), row.usual.as_str()), ("08:00 · 1 tablet, 13:00 · 1 tablet, 20:00 · 2 tablets", "", "1 tablet"));
        assert_eq!(row.takes, [take("08:00", ""), take("13:00", ""), take("20:00", "2 tablets")], "the form says a take's own amount only where it differs");
        let levo_row = medicine_row(&health.medicines[0], today);
        assert_eq!((levo_row.when.as_str(), levo_row.amount.as_str()), ("07:30", "75 µg"));
        let start = now.date().to_zoned(now.time_zone().clone()).unwrap();
        let end = start.checked_add(Span::new().days(1)).unwrap();
        let doses: Vec<(String, String, String)> = health.doses(&start, &end).into_iter().map(|d| (d.at.strftime("%H:%M").to_string(), d.name, d.dose)).collect();
        let expected = [("07:30", "Levothyroxine", "75 µg"), ("08:00", "Iron", "1 tablet"), ("12:30", "Magnesium", "150 mg"), ("13:00", "Iron", "1 tablet"), ("20:00", "Iron", "2 tablets"), ("21:30", "Magnesium", "300 mg")];
        assert_eq!(doses, expected.map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())));
        // Two takes at one time: said with the medicine's name, and nothing changes.
        let before = health.clone();
        let twice = serde_json::json!([{ "id": iron.id, "name": "Iron", "dose": "1 tablet", "takes": [{ "time": "08:00", "amount": "" }, { "time": "8:00", "amount": "2 tablets" }] }]);
        assert_eq!(apply_prescription(&mut health, &id, prescription_edit(twice), &english), Err("Iron: two takes at 08:00. Keep one, with its amount.".into()));
        let unnamed = serde_json::json!([{ "id": "", "name": " ", "dose": "5 mg", "takes": [{ "time": "08:00", "amount": "" }] }]);
        assert!(apply_prescription(&mut health, &id, prescription_edit(unnamed), &english).is_err());
        let none = serde_json::json!([{ "id": iron.id, "name": "Iron", "dose": "1 tablet", "takes": [] }]);
        assert_eq!(apply_prescription(&mut health, &id, prescription_edit(none), &english), Err("Iron: add a take, the time it is taken each day.".into()));
        assert_eq!(health, before);
        // Iron taken out of the form: gone, with its doses; the others as they were. A form
        // without its rows (none given) leaves the medicines alone.
        let out = serde_json::json!([{ "id": levo, "name": "Levothyroxine", "dose": "75 µg", "takes": [{ "time": "07:30", "amount": "" }] }, { "id": iron.id, "removed": true }]);
        apply_prescription(&mut health, &id, prescription_edit(out), &english).unwrap();
        assert_eq!(health.medicines.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Levothyroxine", "Magnesium"]);
        assert!(health.doses(&start, &end).iter().all(|d| d.name != "Iron"));
        let kept = health.clone();
        let plain: PrescriptionEdit = serde_json::from_value(serde_json::json!({ "title": "Thyroid and iron", "refill_days": 30 })).unwrap();
        apply_prescription(&mut health, &id, plain, &english).unwrap();
        assert_eq!(health.medicines, kept.medicines);
        // A medicine every few hours, in a row: its name and dose change, its schedule stays.
        let hours: MedicineEdit = serde_json::from_value(serde_json::json!({ "name": "Antibiotic", "dose": "500 mg", "every": "hours", "hours": 8, "from": "2026-10-08T08:00", "prescription": id })).unwrap();
        let antibiotic = apply_medicine(&mut health, "", hours, &now, &english).unwrap();
        let schedule = health.medicines.iter().find(|m| m.id == antibiotic).unwrap().schedule.clone();
        let rename = serde_json::json!([{ "id": antibiotic, "name": "Amoxicillin", "dose": "1 g", "takes": [] }]);
        apply_prescription(&mut health, &id, prescription_edit(rename), &english).unwrap();
        let changed = health.medicines.iter().find(|m| m.id == antibiotic).unwrap();
        assert_eq!((changed.name.as_str(), changed.dose.as_str(), &changed.schedule), ("Amoxicillin", "1 g", &schedule));
        // Its own form: takes as rows, a time that does not read said; the times as typed before still read.
        let typed: MedicineEdit = serde_json::from_value(serde_json::json!({ "name": "Zinc", "every": "day", "times": ["12:00", " 18:00"] })).unwrap();
        let zinc = apply_medicine(&mut health, "", typed, &now, &english).unwrap();
        assert_eq!(health.medicines.iter().find(|m| m.id == zinc).unwrap().takes(), [take("12:00", ""), take("18:00", "")]);
        let wrong: MedicineEdit = serde_json::from_value(serde_json::json!({ "name": "Zinc", "every": "day", "takes": [{ "time": "noon", "amount": "" }] })).unwrap();
        assert_eq!(apply_medicine(&mut health, &zinc, wrong, &now, &english), Err("“noon” is not a time of day.".into()));
    }

    /// A take's own amount wherever a dose is said: the Porch, the home
    /// screen's card, the reminder (desktop and phone: `named`), the day's list.
    #[test]
    fn a_takes_own_amount_is_said_wherever_its_dose_is() {
        let dir = std::env::temp_dir().join(format!("sioul-takes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        let mut iron = Medicine { id: "iron".into(), name: "Iron".into(), dose: String::new(), schedule: Schedule::Day { times: Vec::new(), amounts: Default::default() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None };
        iron.set_takes("1 tablet", &[take("08:00", ""), take("20:00", "2 tablets")]).unwrap();
        Health { medicines: vec![iron], ..Health::default() }.save(&path).unwrap();
        let health = Health::load(&path);
        let english = Translator::new("en");
        let at = |text: &str| text.parse::<Zoned>().unwrap();
        let alone = Knowledge { record_lost: None, peers: Vec::new() };
        let state = HealthState::default();
        // The Porch at 20:01: the evening's take, 2 tablets.
        let evening = at("2026-10-08T20:01[Europe/Paris]");
        let porch = serde_json::to_value(porch_doses(&health, &state, &DoseRecords::default(), "desk-id", &alone, &evening, false, true, &english)).unwrap();
        let due: Vec<(&str, &str)> = porch["due"].as_array().unwrap().iter().map(|d| (d["time"].as_str().unwrap(), d["dose"].as_str().unwrap())).collect();
        assert_eq!(due, [("20:00", "2 tablets")]);
        // The home screen's card at 07:00: both, each with its amount.
        let card = doses_for_card(&health, &state, &DoseRecords::default(), true, &alone, &at("2026-10-08T07:00[Europe/Paris]"));
        assert_eq!(card.iter().map(|d| (d.time.as_str(), d.name.as_str())).collect::<Vec<_>>(), [("08:00", "Iron · 1 tablet"), ("20:00", "Iron · 2 tablets")]);
        // The reminder's words, the phone's alarms' and the day's list: each dose its take's amount.
        let doses = health.doses(&at("2026-10-08T00:00[Europe/Paris]"), &at("2026-10-09T00:00[Europe/Paris]"));
        assert_eq!(doses.iter().map(named).collect::<Vec<_>>(), ["Iron · 1 tablet", "Iron · 2 tablets"]);
        // The late dose's question: its take's amount.
        assert_eq!(health.amount_of(&doses[1].key, evening.time_zone()).as_deref(), Some("2 tablets"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A medicine's generic name and strength, saved from both forms and read
    /// back; said precisely on the page; and the view a doctor or a pharmacist
    /// reads: the generic name and strength first, then the brand, the takes,
    /// how long, who prescribed it; a prescription's, or all those taken now.
    #[test]
    fn a_doctor_or_pharmacist_reads_the_generic_name_first() {
        let english = Translator::new("en");
        let now: Zoned = "2026-10-08T09:00[Europe/Paris]".parse().unwrap();
        let today = now.date();
        let mut health = Health::default();
        let rows = serde_json::json!([
            { "id": "", "name": "Thyrolan", "generic": "levothyroxine", "strength": "75 µg", "dose": "1 tablet", "takes": [{ "time": "07:30", "amount": "" }] },
            { "id": "", "name": " ", "generic": "metformin", "strength": "500 mg per tablet", "dose": "1 tablet", "takes": [{ "time": "08:00", "amount": "" }, { "time": "20:00", "amount": "2 tablets" }] },
        ]);
        let edit: PrescriptionEdit = serde_json::from_value(serde_json::json!({ "title": "Thyroid and sugar", "prescriber": "Dr Martin", "until": "2027-02-04", "medicines": rows })).unwrap();
        let id = apply_prescription(&mut health, "", edit, &english).unwrap();
        // No name given: the generic name is its name, for reminders and an older Sioul.
        let fields: Vec<(&str, &str, &str)> = health.medicines.iter().map(|m| (m.name.as_str(), m.generic.as_str(), m.strength.as_str())).collect();
        assert_eq!(fields, [("Thyrolan", "levothyroxine", "75 µg"), ("metformin", "metformin", "500 mg per tablet")]);
        // Its own form: since a week, every eight hours.
        let antibiotic: MedicineEdit = serde_json::from_value(serde_json::json!({ "name": "Amoxi", "generic": "amoxicillin", "strength": "500 mg", "dose": "1 capsule", "every": "hours", "hours": 8, "from": "2026-10-08T08:00", "since": "2026-10-01" })).unwrap();
        apply_medicine(&mut health, "", antibiotic, &now, &english).unwrap();
        // Paused, and past its last day: not taken now.
        let mut paused = health.medicines[0].clone();
        (paused.id, paused.name, paused.paused, paused.prescription) = ("zinc".into(), "Zinc".into(), true, None);
        let mut over = health.medicines[0].clone();
        (over.id, over.name, over.prescription, over.until) = ("iron".into(), "Iron".into(), None, Some("2026-10-01".parse().unwrap()));
        health.medicines.extend([paused, over]);
        // Written and read back, as they were.
        let again: Health = toml::from_str(&toml::to_string(&health).unwrap()).unwrap();
        assert_eq!(again, health);
        // Nothing given, nothing to call it: said.
        let nameless: MedicineEdit = serde_json::from_value(serde_json::json!({ "name": "", "generic": " ", "every": "day", "takes": [{ "time": "08:00" }] })).unwrap();
        assert_eq!(apply_medicine(&mut health.clone(), "", nameless, &now, &english), Err("A name is needed.".into()));
        // On the page, precisely; the reminders keep the short name.
        let levo = medicine_row(&health.medicines[0], today);
        assert_eq!((levo.precise.as_str(), levo.line.as_str()), ("Thyrolan — levothyroxine 75 µg", "Thyrolan — levothyroxine 75 µg · 07:30 · 1 tablet"));
        assert_eq!(medicine_row(&health.medicines[1], today).precise, "metformin 500 mg per tablet");
        let start = now.date().to_zoned(now.time_zone().clone()).unwrap();
        let end = start.checked_add(Span::new().days(1)).unwrap();
        assert_eq!(health.doses(&start, &end).iter().find(|d| d.at.hour() == 7).map(|d| d.name.as_str()), Some("Thyrolan"));
        // The prescription's view: who and until when above, the generic names first.
        let view = for_professional_of(&health, &id, today, &english);
        assert_eq!(view.title, "Thyroid and sugar");
        assert_eq!(view.about, ["Prescribed by Dr Martin", "Valid until Thursday 4 February 2027"]);
        let levo = &view.medicines[0];
        assert_eq!((levo.molecule.as_str(), levo.brand.as_str(), levo.takes.clone(), levo.prescriber.as_str()), ("levothyroxine 75 µg", "Thyrolan", vec!["07:30 — 1 tablet".to_string()], ""));
        let metformin = &view.medicines[1];
        assert_eq!((metformin.molecule.as_str(), metformin.brand.as_str()), ("metformin 500 mg per tablet", ""), "its name is its generic name: said once");
        assert_eq!(metformin.takes, ["08:00 — 1 tablet", "20:00 — 2 tablets"]);
        assert_eq!(view.medicines.len(), 2, "the antibiotic is not on it");
        // All those taken now: neither the paused one nor the one past its last day.
        let all = for_professional_of(&health, "", today, &english);
        assert_eq!((all.title.as_str(), all.about.clone()), ("Current medicines", vec!["As of Thursday 8 October 2026".to_string()]));
        assert_eq!(all.medicines.iter().map(|m| m.molecule.as_str()).collect::<Vec<_>>(), ["levothyroxine 75 µg", "metformin 500 mg per tablet", "amoxicillin 500 mg"]);
        assert_eq!(all.medicines[0].prescriber, "Prescribed by Dr Martin");
        let amoxicillin = &all.medicines[2];
        assert_eq!((amoxicillin.takes.clone(), amoxicillin.since.as_str(), amoxicillin.brand.as_str()), (vec!["every 8 hours — 1 capsule".to_string()], "Taken since Thursday 1 October, 7 days", "Amoxi"));
        // A paused medicine in its prescription's view: said.
        health.medicines[1].paused = true;
        assert_eq!(for_professional_of(&health, &id, today, &english).medicines[1].notes, ["Paused for now"]);
        // In French, the generic names as typed.
        let french = for_professional_of(&health, "", today, &Translator::new("fr"));
        assert_eq!(french.title, "Médicaments en cours");
        assert_eq!(french.medicines[1].since, "Pris depuis le jeudi 1er octobre, 7 jours");
        assert_eq!(french.medicines[0].molecule, "levothyroxine 75 µg");
        // Nothing to show: said.
        assert_eq!(for_professional_of(&Health::default(), "", today, &english).empty, "No medicine to show.");
        assert_eq!((how_long("2024-01-01".parse().unwrap(), today, &english), how_long("2026-06-01".parse().unwrap(), today, &english), how_long(today, today, &english)), ("2 years".into(), "4 months".into(), String::new()));
    }

    /// The Porch's doses at a desktop, on a health file written for the test
    /// (never yours) and fake times: due now while Sioul runs; due earlier
    /// today, reminded by its notification but not answered; due while Sioul
    /// ran nowhere; another device not heard lately; asleep with doses silent;
    /// marked; the day over.
    #[test]
    fn the_porch_shows_todays_doses_until_marked() {
        let dir = std::env::temp_dir().join(format!("sioul-porch-doses-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        let file = "[[medicine]]\nid = \"levo\"\nname = \"Levothyroxine\"\ndose = \"75 µg\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"07:30\"]\n\n\
                    [[medicine]]\nid = \"iron\"\nname = \"Iron\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"12:00\", \"21:00\"]\n\n\
                    [[medicine]]\nid = \"zinc\"\nname = \"Zinc\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"09:00\"]\n";
        std::fs::write(&path, file).unwrap();
        let health = Health::load(&path);
        assert_eq!(health.medicines.len(), 3);
        let english = Translator::new("en");
        let at = |text: &str| text.parse::<Zoned>().unwrap();
        let key = |id: &str, when: &str| format!("{id}@{}", at(when).timestamp().as_second());
        let alone = Knowledge { record_lost: None, peers: Vec::new() };
        let shown = |state: &HealthState, knowledge: &Knowledge, now: &Zoned, silent: bool, heard: bool| serde_json::to_value(porch_doses(&health, state, &DoseRecords::default(), "desk-id", knowledge, now, silent, heard, &english)).unwrap();
        let mut state = HealthState::default();
        // 07:30, reminded by its notification while Sioul ran, never answered.
        state.reminded.insert(key("levo", "2026-10-06T07:30[Europe/Paris]"), at("2026-10-06T07:31[Europe/Paris]").timestamp().as_second());
        let noon = at("2026-10-06T12:00:40[Europe/Paris]");
        let view = shown(&state, &alone, &noon, false, true);
        let due = view["due"].as_array().unwrap();
        assert_eq!(due.len(), 2, "{view}");
        // Due earlier, reminded, not answered: on the Porch, "Taken…" asking when (late).
        assert_eq!((due[0]["time"].as_str(), due[0]["name"].as_str(), due[0]["dose"].as_str(), due[0]["late"].as_bool()), (Some("07:30"), Some("Levothyroxine"), Some("75 µg"), Some(true)));
        // Due now, Sioul running, its notification given or not yet: "Taken", marked now; nothing in doubt.
        assert_eq!((due[1]["time"].as_str(), due[1]["late"].as_bool(), due[1]["doubt"].as_str()), (Some("12:00"), Some(false), Some("")));
        // Due while Sioul ran nowhere (09:00, reminded nowhere): the question, apart.
        let closed = view["closed"].as_array().unwrap();
        assert_eq!(closed.len(), 1);
        assert_eq!((closed[0]["key"].as_str(), closed[0]["time"].as_str()), (Some(key("zinc", "2026-10-06T09:00[Europe/Paris]").as_str()), Some("09:00")));
        // Before your other devices were heard from, the question waits; today's doses do not.
        let early = shown(&state, &alone, &noon, false, false);
        assert_eq!((early["due"].as_array().unwrap().len(), early["closed"].as_array().unwrap().len()), (2, 0));
        // Asleep, doses staying silent: nothing until waking.
        let asleep = shown(&state, &alone, &noon, true, true);
        assert!(asleep["due"].as_array().unwrap().is_empty() && asleep["closed"].as_array().unwrap().is_empty());
        // The laptop heard in full at 07:35, not since: it may have marked them. Check first, never "not taken".
        let heard = at("2026-10-06T07:35[Europe/Paris]").timestamp().as_second();
        let unsure = Knowledge { record_lost: None, peers: vec![Peer { name: "laptop".into(), known_until: heard, closed: false, delay: Some(30), broken: None, heard, ..Peer::default() }] };
        let doubted = shown(&state, &unsure, &noon, false, true);
        for row in doubted["due"].as_array().unwrap().iter().chain(doubted["closed"].as_array().unwrap()) {
            let doubt = row["doubt"].as_str().unwrap();
            assert!(doubt.starts_with("Sioul can't tell whether it was taken") && doubt.contains("laptop") && doubt.ends_with("Check before taking it."), "{doubt}");
            assert!(!doubt.to_lowercase().contains("not taken"), "{doubt}");
        }
        // Marked, taken or said not taken: gone.
        state.taken.insert(key("iron", "2026-10-06T12:00[Europe/Paris]"), noon.timestamp().as_second());
        state.not_taken.insert(key("levo", "2026-10-06T07:30[Europe/Paris]"), noon.timestamp().as_second());
        assert!(shown(&state, &alone, &noon, false, true)["due"].as_array().unwrap().is_empty());
        // The day over: today's are gone at midnight; the evening's iron, reminded nowhere, is the question's.
        let night = shown(&state, &alone, &at("2026-10-07T00:30[Europe/Paris]"), false, true);
        assert!(night["due"].as_array().unwrap().is_empty());
        assert_eq!(night["closed"][0]["time"].as_str(), Some("Tue 21:00"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A dose whose reminder could not be shown (no notification server) while
    /// Sioul ran, on a health file written for the test: recorded apart from a
    /// reminder (`note_minute`), still asked about, but under its own question,
    /// never "Due while Sioul was closed"; a dose due while Sioul ran nowhere
    /// keeps that one. Both say the doubt, never "not taken"; the
    /// notification's title says which, in English and French.
    #[test]
    fn a_reminder_not_shown_is_asked_about_under_its_own_question() {
        let dir = std::env::temp_dir().join(format!("sioul-unshown-doses-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        let file = "[[medicine]]\nid = \"demo-a\"\nname = \"Demo A\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\"]\n\n\
                    [[medicine]]\nid = \"demo-b\"\nname = \"Demo B\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"07:00\"]\n";
        std::fs::write(&path, file).unwrap();
        let health = Health::load(&path);
        let (english, french) = (Translator::new("en"), Translator::new("fr"));
        let at = |text: &str| text.parse::<Zoned>().unwrap();
        let key = |id: &str, when: &str| format!("{id}@{}", at(when).timestamp().as_second());
        let (a, b) = (key("demo-a", "2026-10-06T08:00[Europe/Paris]"), key("demo-b", "2026-10-06T07:00[Europe/Paris]"));
        let alone = Knowledge { record_lost: None, peers: Vec::new() };
        let noon = at("2026-10-06T12:00[Europe/Paris]");
        // The minute at 08:00: Demo A's notification failed. Not reminded: noted apart.
        let mut state = HealthState::default();
        note_minute(&mut state, &[], std::slice::from_ref(&a), &[], at("2026-10-06T08:00:30[Europe/Paris]").timestamp().as_second());
        assert!(state.reminded.is_empty() && state.reminder_unshown(&a));
        // Demo B, at 07:00, due while Sioul ran nowhere: the usual question, first.
        let view = serde_json::to_value(porch_doses(&health, &state, &DoseRecords::default(), "desk-id", &alone, &noon, false, true, &english)).unwrap();
        let closed = view["closed"].as_array().unwrap();
        assert_eq!(closed.len(), 2, "{view}");
        assert_eq!((closed[0]["key"].as_str(), closed[0]["question"].as_str()), (Some(b.as_str()), Some("Due while Sioul was closed, and marked nowhere Sioul can see: did you take them?")));
        // Demo A, Sioul running: still asked about, under what happened.
        assert_eq!(closed[1]["key"].as_str(), Some(a.as_str()));
        let question = closed[1]["question"].as_str().unwrap();
        assert!(question.starts_with("Due while Sioul ran, but their reminder could not be shown") && !question.contains("closed"), "{question}");
        assert!(view["due"].as_array().unwrap().iter().all(|row| row["question"] == ""), "today's doses carry no question");
        // The Health page's list says the same.
        let rows = missed_rows(&health, &state, &alone, &noon, &french);
        assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), [b.as_str(), a.as_str()]);
        assert!(rows[1].question.starts_with("Prévus pendant que Sioul tournait, mais leur rappel n’a pas pu s’afficher"), "{}", rows[1].question);
        // Another device not heard since: the doubt said under both, never "not taken".
        let heard = at("2026-10-06T06:30[Europe/Paris]").timestamp().as_second();
        let unsure = Knowledge { record_lost: None, peers: vec![Peer { name: "laptop".into(), known_until: heard, closed: false, delay: Some(30), broken: None, heard, ..Peer::default() }] };
        for row in missed_rows(&health, &state, &unsure, &noon, &english) {
            assert!(row.doubt.starts_with("Sioul can't tell whether it was taken") && !row.doubt.to_lowercase().contains("not taken"), "{}", row.doubt);
        }
        // The notification's title: one kind, the other, both.
        let missed = state.unanswered(&health, &noon, MISSED_HOURS, GRACE_MINUTES);
        assert_eq!(missed_title(&missed, &state, &english), "Doses due earlier");
        assert_eq!(missed_title(&missed[1..], &state, &english), "A reminder could not be shown");
        assert_eq!(missed_title(&missed[..1], &state, &english), "While Sioul was closed");
        assert_eq!(missed_title(&missed[1..], &state, &french), "Un rappel n’a pas pu s’afficher");
        // Reminded later (another device, or notifications back): no question at all.
        note_minute(&mut state, std::slice::from_ref(&a), &[], &[], at("2026-10-06T08:05[Europe/Paris]").timestamp().as_second());
        let rows = missed_rows(&health, &state, &alone, &noon, &english);
        assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), [b.as_str()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The phone's home screen card (`homecard`), on a health file written
    /// for the test: today's doses due and not marked, and those to come
    /// today; each known not marked only while the other devices were heard
    /// within five minutes; a record that does not read, never as empty.
    #[test]
    fn the_home_screen_card_says_doses_as_known() {
        let dir = std::env::temp_dir().join(format!("sioul-card-doses-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        std::fs::write(&path, "[[medicine]]\nid = \"mag\"\nname = \"Magnesium\"\ndose = \"300 mg\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"12:30\", \"20:00\"]\n").unwrap();
        let health = Health::load(&path);
        let at = |text: &str| text.parse::<Zoned>().unwrap();
        let s = |text: &str| at(text).timestamp().as_second();
        let now = at("2026-10-06T13:00[Europe/Paris]");
        let alone = Knowledge { record_lost: None, peers: Vec::new() };
        let mut state = HealthState::default();
        let doses = doses_for_card(&health, &state, &DoseRecords::default(), true, &alone, &now);
        // 12:30 due and not marked; 20:00 to come; tomorrow's not given.
        assert_eq!(doses.iter().map(|d| (d.name.as_str(), d.time.as_str())).collect::<Vec<_>>(), [("Magnesium · 300 mg", "12:30"), ("Magnesium · 300 mg", "20:00")]);
        // This device alone: known until the card lets it go, at midnight.
        let midnight = s("2026-10-07T00:00[Europe/Paris]");
        assert_eq!((doses[0].at, doses[0].until, doses[0].known_until), (s("2026-10-06T12:30[Europe/Paris]"), midnight, midnight));
        // Marked: gone.
        state.taken.insert(format!("mag@{}", s("2026-10-06T12:30[Europe/Paris]")), s("2026-10-06T12:35[Europe/Paris]"));
        assert_eq!(doses_for_card(&health, &state, &DoseRecords::default(), true, &alone, &now).len(), 1);
        // The laptop heard in full at 12:58: known until five minutes after, then not.
        let heard = s("2026-10-06T12:58[Europe/Paris]");
        let laptop = Knowledge { record_lost: None, peers: vec![Peer { name: "laptop".into(), known_until: heard, closed: false, delay: Some(30), broken: None, heard, ..Peer::default() }] };
        let fresh = doses_for_card(&health, &HealthState::default(), &DoseRecords::default(), true, &laptop, &now);
        assert_eq!(fresh[0].known_until, heard + 5 * 60 + 1);
        // 20:00, still to come: what the laptop marks by then is not here now; not known from its time on.
        assert_eq!(fresh[1].known_until, 0);
        // Heard before the dose was due: not known at all.
        let noon = s("2026-10-06T12:00[Europe/Paris]");
        let old = Knowledge { record_lost: None, peers: vec![Peer { name: "laptop".into(), known_until: noon, closed: false, delay: Some(30), broken: None, heard: noon, ..Peer::default() }] };
        assert_eq!(doses_for_card(&health, &HealthState::default(), &DoseRecords::default(), true, &old, &now)[0].known_until, 0);
        // A record that does not read: every dose not known, none left out.
        let unread = doses_for_card(&health, &HealthState::default(), &DoseRecords::default(), false, &alone, &now);
        assert_eq!(unread.len(), 2);
        assert!(unread.iter().all(|d| d.known_until == 0));
        // Taken here, said not taken on another device: answers that differ, on the card all the
        // same, never known (the Porch asks to choose); never left out as if marked.
        let mut differ = HealthState::default();
        let key = format!("mag@{}", s("2026-10-06T12:30[Europe/Paris]"));
        differ.taken.insert(key.clone(), s("2026-10-06T12:35[Europe/Paris]"));
        differ.not_taken.insert(key, s("2026-10-06T12:40[Europe/Paris]"));
        let doses = doses_for_card(&health, &differ, &DoseRecords::default(), true, &alone, &now);
        assert_eq!(doses.iter().map(|d| (d.time.as_str(), d.known_until)).collect::<Vec<_>>(), [("12:30", 0), ("20:00", midnight)]);
        // Answered in the records only, their marks not here yet: the answer counts, and a dose
        // answered differently there is on the card once, never known.
        let key = format!("mag@{}", s("2026-10-06T12:30[Europe/Paris]"));
        let given = |state: &str, at: &str, name: &str| Answer { state: state.into(), at: s(at), noted: s(at), name: name.into(), kind: name.into() };
        let mut records = DoseRecords::default();
        records.answer(&key, "phone-id", given(sioul_core::doses::TAKEN, "2026-10-06T12:35[Europe/Paris]", "phone"));
        let doses = doses_for_card(&health, &HealthState::default(), &records, true, &alone, &now);
        assert_eq!(doses.iter().map(|d| d.time.as_str()).collect::<Vec<_>>(), ["20:00"]);
        records.answer(&key, "desk-id", given(sioul_core::doses::SKIPPED, "2026-10-06T12:40[Europe/Paris]", "computer"));
        let doses = doses_for_card(&health, &HealthState::default(), &records, true, &alone, &now);
        assert_eq!(doses.iter().map(|d| (d.time.as_str(), d.known_until)).collect::<Vec<_>>(), [("12:30", 0), ("20:00", midnight)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F21 on the phone's home screen card: a dose to come, a computer closed
    /// before it whose opening's news comes within two minutes: "check" from
    /// its time for those two minutes, then said as known; a dose due an hour
    /// ago, known at once. The card's Java shows each piece in its own time.
    #[test]
    fn the_card_says_check_while_a_closed_device_may_have_opened() {
        let dir = std::env::temp_dir().join(format!("sioul-card-wake-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        std::fs::write(&path, "[[medicine]]\nid = \"mag\"\nname = \"Magnesium\"\ndose = \"300 mg\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"12:30\", \"20:00\"]\n").unwrap();
        let health = Health::load(&path);
        let s = |text: &str| text.parse::<Zoned>().unwrap().timestamp().as_second();
        let now = "2026-10-06T13:00[Europe/Paris]".parse::<Zoned>().unwrap();
        let said = sioul_core::health::Said { started: s("2026-10-06T08:00[Europe/Paris]"), closed: s("2026-10-06T12:00[Europe/Paris]"), working: false, exported: s("2026-10-06T12:00[Europe/Paris]"), doses: true, ..Default::default() };
        let desk = Peer { id: "desk-id".into(), name: "desk".into(), said: Some(said), complete: true, seen: s("2026-10-06T12:01[Europe/Paris]"), heard: s("2026-10-06T12:00[Europe/Paris]"), wake: Some(120), ..Peer::default() };
        let knowledge = Knowledge { record_lost: None, peers: vec![desk] };
        let doses = doses_for_card(&health, &HealthState::default(), &DoseRecords::default(), true, &knowledge, &now);
        let (eight, midnight) = (s("2026-10-06T20:00[Europe/Paris]"), s("2026-10-07T00:00[Europe/Paris]"));
        let pieces: Vec<(&str, i64, i64, i64)> = doses.iter().map(|d| (d.time.as_str(), d.at, d.until, d.known_until)).collect();
        assert_eq!(pieces, [("12:30", s("2026-10-06T12:30[Europe/Paris]"), midnight, midnight), ("20:00", eight, eight + 120, 0), ("20:00", eight + 120, midnight, midnight)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn heard_of(id: &str, n: u64) -> sioul_sync::share::Heard {
        sioul_sync::share::Heard { read: [(id.to_string(), (1, n))].into(), broken: Default::default() }
    }

    /// The devices' registry, learned (`learn`): each entry seen changing here
    /// is timed by this device's clock; an export dated later than it was seen
    /// tells how far the other clock is ahead, at least; what no longer reads
    /// is not said; a device said off counts again once it says anything newer.
    /// An alarm that read none of the others' news since it went off
    /// (`share::news_missed`): the laptop, known closed before the dose by
    /// what was read before, is said in doubt with when it closed; with news
    /// read, it is known, as before (docs/health.md, "No news at the alarm").
    #[test]
    fn an_alarm_that_read_no_news_says_a_closed_device_in_doubt() {
        let english = Translator::new("en");
        let due = 1_800_000_000;
        let said = sioul_core::health::Said { started: due - 7_200, closed: due - 3_600, working: false, exported: due - 3_605, imported: due - 3_605, doses: true, ..Default::default() };
        let laptop = Peer { id: "laptop-id".into(), name: "laptop".into(), said: Some(said), complete: true, seen: due - 3_500, heard: due - 3_605, ..Peer::default() };
        let peers = vec![laptop];
        assert!(sioul_core::health::doubts(due, due + 60, None, &peers).is_empty(), "news read: known");
        let doubts = sioul_core::health::doubts_unread(due, due + 60, None, &peers, due + 30, &Default::default());
        let words = why_in(&doubts, &english);
        assert!(words.starts_with("laptop closed") && words.contains("its news can be slow to come") && !words.to_lowercase().contains("not taken"), "{words}");
    }

    /// F21 of the review of 5 October 2026: how late the news of a device's
    /// opening comes here, timed on its starts seen while this device looked
    /// a moment before (its own absence never counts), its clock's lead taken
    /// off; the longest of the last ones is the wait for a device closed
    /// before a dose (`sioul_core::health::doubts_now`).
    #[test]
    fn how_late_a_devices_opening_comes_is_timed_while_looking() {
        use sioul_sync::devices::Entry;
        let phone = |started: i64, working: bool, closed: i64| Entry { id: "phone-id".into(), name: "FP3".into(), kind: "phone".into(), started, closed, working, exported: started.max(closed), wrote: Some((1, 5)), doses: true, ..Entry::default() };
        let mut kept = Peers::default();
        // Closed at 1 000; seen first at 1 020: nothing timed.
        learn(&mut kept, &[], &[phone(900, false, 1_000)], &heard_of("phone-id", 5), 1_020);
        for now in [1_100, 1_200, 1_300] {
            learn(&mut kept, &[], &[phone(900, false, 1_000)], &heard_of("phone-id", 5), now);
        }
        assert_eq!(kept.peers["phone-id"].peer.wake, None);
        // Opened at 1 150 (its clock as this one), its news seen here at 1 390, this device looking each minute or two: 240 s.
        learn(&mut kept, &[], &[phone(1_150, true, 1_000)], &heard_of("phone-id", 5), 1_390);
        assert_eq!(kept.peers["phone-id"].peer.wake, Some(240));
        // Closed again, then opened while this device was away for ten minutes: its absence not counted.
        learn(&mut kept, &[], &[phone(1_150, false, 1_500)], &heard_of("phone-id", 5), 1_510);
        learn(&mut kept, &[], &[phone(1_600, true, 1_500)], &heard_of("phone-id", 5), 2_200);
        assert_eq!(kept.peers["phone-id"].peer.wake, Some(240));
        // A quicker one: the longest of the last ones stays the wait.
        learn(&mut kept, &[], &[phone(1_600, false, 2_250)], &heard_of("phone-id", 5), 2_260);
        learn(&mut kept, &[], &[phone(2_300, true, 2_250)], &heard_of("phone-id", 5), 2_330);
        assert_eq!(kept.peers["phone-id"].peer.wake, Some(240));
        // A dose due at 3 000, the phone closed at 2 900: in doubt for those 240 s, then known.
        learn(&mut kept, &[], &[phone(2_300, false, 2_900)], &heard_of("phone-id", 5), 2_910);
        let peers = vec![kept.peers["phone-id"].peer.clone()];
        assert_eq!(sioul_core::health::doubts_now(3_000, 3_200, None, &peers).len(), 1);
        assert!(sioul_core::health::doubts_now(3_000, 3_240, None, &peers).is_empty());
    }

    #[test]
    fn what_the_devices_say_of_themselves_teaches() {
        use sioul_sync::devices::Entry;
        let phone = |exported: i64, working: bool, closed: i64| Entry { id: "phone-id".into(), name: "FP3".into(), kind: "phone".into(), started: 1_000, closed, working, exported, wrote: Some((1, 5)), doses: true, ..Entry::default() };
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[phone(1_100, true, 0)], &heard_of("phone-id", 5), 1_120);
        let peer = &kept.peers["phone-id"].peer;
        assert_eq!((peer.seen, peer.complete, peer.said.as_ref().map(|s| (s.working, s.phone))), (1_120, true, Some((true, true))));
        // The same entry a minute later: not seen changing.
        learn(&mut kept, &[], &[phone(1_100, true, 0)], &heard_of("phone-id", 5), 1_180);
        assert_eq!(kept.peers["phone-id"].peer.seen, 1_120);
        // An export dated three minutes after it was seen here: its clock three minutes ahead, at least.
        learn(&mut kept, &[], &[phone(1_400, true, 0)], &heard_of("phone-id", 5), 1_220);
        assert_eq!((kept.peers["phone-id"].peer.seen, kept.peers["phone-id"].peer.ahead), (1_220, 180));
        // Its records not all read here: not complete.
        learn(&mut kept, &[], &[Entry { wrote: Some((1, 9)), ..phone(1_460, true, 0) }], &heard_of("phone-id", 5), 1_280);
        assert!(!kept.peers["phone-id"].peer.complete);
        // Its entry no longer reads: nothing said of it, never as it last said.
        learn(&mut kept, &[], &[], &heard_of("phone-id", 9), 1_340);
        assert_eq!(kept.peers["phone-id"].peer.said, None);
        // Said off: not counted; counted again once it says anything newer.
        learn(&mut kept, &[], &[phone(1_460, true, 0)], &heard_of("phone-id", 9), 1_400);
        kept.peers.get_mut("phone-id").unwrap().off = Some(Mark { started: 1_000, exported: 1_460, renewed: 0, at: 1_400 });
        learn(&mut kept, &[], &[phone(1_460, true, 0)], &heard_of("phone-id", 9), 1_460);
        assert!(kept.peers["phone-id"].peer.off, "nothing newer: still off");
        learn(&mut kept, &[], &[phone(1_520, false, 1_525)], &heard_of("phone-id", 9), 1_530);
        assert!(!kept.peers["phone-id"].peer.off, "it spoke again");
        // Closed at 1 525 by its entry, but its claim renewed open at 1 700 (an older Sioul put back
        // there, which updates no entry): the entry is not trusted, its claims are, as an older Sioul's.
        let open = Claim { computer: "phone-id".into(), name: "localhost".into(), since: 1_600, renewed: 1_700, until: 2_000, active: 1_700, taken: 0, wrote: Some((1, 9)), closed: false, pad: String::new() };
        learn(&mut kept, &[open], &[phone(1_520, false, 1_525)], &heard_of("phone-id", 9), 1_710);
        assert_eq!(kept.peers["phone-id"].peer.said, None);
    }

    /// The Porch's doses with the devices' registry, on a health file written
    /// for the test: a dose your devices answered differently shows both
    /// answers, the earliest first, and lets you choose; one the phone may
    /// hold (in use, silent since the dose) names it and offers "This device
    /// is off"; the phone closed cleanly, nothing in doubt. Never "not taken".
    #[test]
    fn the_porch_says_answers_that_differ_and_which_device_may_know() {
        use sioul_core::health::Said;
        let dir = std::env::temp_dir().join(format!("sioul-porch-registry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        std::fs::write(&path, "[[medicine]]\nid = \"levo\"\nname = \"Levothyroxine\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"12:00\"]\n").unwrap();
        let health = Health::load(&path);
        let english = Translator::new("en");
        let s = |text: &str| text.parse::<Zoned>().unwrap().timestamp().as_second();
        let eight = format!("levo@{}", s("2026-10-06T08:00[Europe/Paris]"));
        let noon = "2026-10-06T12:00:40[Europe/Paris]".parse::<Zoned>().unwrap();
        // Taken on the phone at 08:02 (its sync slow); said not taken here at 08:10.
        let mut records = DoseRecords::default();
        records.answer(&eight, "phone-id", Answer { state: "taken".into(), at: s("2026-10-06T08:02[Europe/Paris]"), noted: s("2026-10-06T08:02[Europe/Paris]"), name: "FP3".into(), kind: "phone".into() });
        records.answer(&eight, "desk-id", Answer { state: "skipped".into(), at: s("2026-10-06T08:10[Europe/Paris]"), noted: s("2026-10-06T08:10[Europe/Paris]"), name: "desk".into(), kind: "computer".into() });
        let mut state = HealthState::default();
        state.taken.insert(eight.clone(), s("2026-10-06T08:02[Europe/Paris]"));
        state.not_taken.insert(eight.clone(), s("2026-10-06T08:10[Europe/Paris]"));
        let shown = |knowledge: &Knowledge| serde_json::to_value(porch_doses(&health, &state, &records, "desk-id", knowledge, &noon, false, true, &english)).unwrap();
        let alone = Knowledge { record_lost: None, peers: Vec::new() };
        let view = shown(&alone);
        let due = view["due"].as_array().unwrap();
        assert_eq!(due.len(), 2, "{view}");
        let differ = due[0]["doubt"].as_str().unwrap();
        assert!(differ.starts_with("Marked taken on the phone ") && differ.contains(", skipped here ") && differ.ends_with("Check which is right before taking it."), "{differ}");
        assert_eq!((due[0]["key"].as_str(), due[0]["choose"].as_bool()), (Some(eight.as_str()), Some(true)));
        assert_eq!((due[1]["doubt"].as_str(), due[1]["choose"].as_bool()), (Some(""), Some(false)));
        // The phone in use, its last export at 11:45, before the 12:00 dose: named, and "This device is off" offered.
        let phone = Peer { id: "phone-id".into(), name: "FP3".into(), said: Some(Said { started: s("2026-10-06T07:00[Europe/Paris]"), working: true, exported: s("2026-10-06T11:45[Europe/Paris]"), doses: true, phone: true, ..Said::default() }), complete: true, seen: s("2026-10-06T11:45:30[Europe/Paris]"), heard: s("2026-10-06T11:45[Europe/Paris]"), ..Peer::default() };
        let view = shown(&Knowledge { record_lost: None, peers: vec![phone.clone()] });
        let row = &view["due"][1];
        let doubt = row["doubt"].as_str().unwrap();
        assert!(doubt.starts_with("Sioul can't tell whether it was taken: the phone was in use and last shared ") && doubt.ends_with(". Check before taking it."), "{doubt}");
        assert!(!doubt.to_lowercase().contains("not taken"), "{doubt}");
        assert_eq!(row["doubt_off"], serde_json::json!([{ "id": "phone-id", "label": "This device is off" }]));
        // Closed cleanly at 11:50, after its last export, read in full: known; nothing to say off.
        let closed = Peer { said: Some(Said { started: s("2026-10-06T07:00[Europe/Paris]"), closed: s("2026-10-06T11:50[Europe/Paris]"), exported: s("2026-10-06T11:49[Europe/Paris]"), doses: true, phone: true, ..Said::default() }), ..phone.clone() };
        let view = shown(&Knowledge { record_lost: None, peers: vec![closed] });
        assert_eq!((view["due"][1]["doubt"].as_str(), view["due"][1]["doubt_off"].as_array().map(Vec::len)), (Some(""), Some(0)));
        // You said it is off: not counted, the dose shown due, never answered for you.
        let off = Peer { off: true, ..phone };
        let view = shown(&Knowledge { record_lost: None, peers: vec![off] });
        assert_eq!((view["due"][1]["doubt"].as_str(), view["due"].as_array().map(Vec::len)), (Some(""), Some(2)));
        // The same answers in the records only, their marks not here yet: the 08:00 dose once, to choose.
        let view = serde_json::to_value(porch_doses(&health, &HealthState::default(), &records, "desk-id", &alone, &noon, false, true, &english)).unwrap();
        let keys: Vec<&str> = view["due"].as_array().unwrap().iter().map(|row| row["key"].as_str().unwrap_or("")).collect();
        assert_eq!(keys.iter().filter(|key| **key == eight).count(), 1, "{view}");
        assert_eq!(view["due"][0]["choose"].as_bool(), Some(true), "{view}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Settings ▸ Your folder and sharing lists your other devices: how each
    /// is, in words, with what can be done; nothing red, no counts.
    #[test]
    fn your_devices_listed_in_words() {
        use sioul_core::health::Said;
        use sioul_sync::devices::Entry;
        let english = Translator::new("en");
        let now = 1_800_000_000;
        let entry = |id: &str, kind: &str, name: &str, working: bool, exported: i64, doses: bool| Entry { id: id.into(), name: name.into(), kind: kind.into(), started: now - 7_200, closed: if working { 0 } else { exported + 5 }, working, exported, doses, ..Entry::default() };
        let mut kept = Peers::default();
        let mut add = |entry: Entry, seen: i64, ahead: i64| {
            let peer = Peer { id: entry.id.clone(), name: entry.name.clone(), said: Some(entry.said()), complete: true, seen, heard: entry.exported, ahead, ..Peer::default() };
            kept.peers.insert(entry.id.clone(), PeerSeen { peer, entry: Some(entry), ..PeerSeen::default() });
        };
        add(entry("a-phone", "phone", "FP3", false, now - 600, true), now - 590, 240);
        add(entry("b-desk", "computer", "desk", true, now - 30, true), now - 25, 0);
        add(entry("c-laptop", "computer", "laptop", true, now - 4 * 3_600, false), now - 4 * 3_600, 0);
        add(entry("d-old", "computer", "old", true, now - 9 * 86_400, true), now - 9 * 86_400, 0);
        kept.peers.insert("e-older".into(), PeerSeen { peer: Peer { id: "e-older".into(), name: "tower".into(), heard: now - 120, ..Peer::default() }, renewed: now - 120, ..PeerSeen::default() });
        kept.peers.get_mut("b-desk").unwrap().peer.off = true;
        let peers: Vec<Peer> = kept.peers.values().map(|s| s.peer.clone()).collect();
        let rows = device_rows_of(&peers, &kept, now, &english);
        let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert_eq!(row("a-phone").name, "The phone (FP3)");
        assert!(row("a-phone").state.starts_with("Closed ") && row("a-phone").notes == ["Its clock is 4 minutes ahead of this one's, at least."], "{:?}", row("a-phone"));
        assert!(row("b-desk").off && row("b-desk").state == "Counted as off, as you said, until it shares again.");
        assert!(row("c-laptop").state.starts_with("In use when it last shared") && row("c-laptop").notes == ["It does not share its doses."], "{:?}", row("c-laptop"));
        assert!(row("d-old").forget && row("d-old").state.starts_with("Silent since "), "{:?}", row("d-old"));
        assert!(row("e-older").state.starts_with("An older Sioul: last heard "), "{:?}", row("e-older"));
        assert!(rows.iter().all(|r| !r.state.contains("not taken")));
        let _ = Said::default();
        // The devices holding the newer form of what travels back: each has its
        // line; one not listed is added, with Forget; one listed keeps its own.
        let mut rows = rows;
        holder_rows(&mut rows, &[("e-older".into(), now - 120, true), ("f-gone".into(), now - 90 * 86_400, true), ("g-unknown".into(), 0, true), ("h-after".into(), now - 3_600, false)], &english);
        assert_eq!(rows.iter().filter(|r| r.id == "e-older").count(), 1);
        let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert!(!row("e-older").forget, "counted for the doses: forgotten once silent a week only");
        assert!(row("f-gone").forget && row("f-gone").name == "Another device of yours" && row("f-gone").state.starts_with("Known here by what it shared, last "), "{:?}", row("f-gone"));
        assert!(row("h-after").forget && row("h-after").state.starts_with("An older Sioul: last heard "), "{:?}", row("h-after"));
        assert!(row("g-unknown").forget && row("g-unknown").state == "Known here by what it shared: until it is updated there, or forgotten here, your devices keep sharing in the form an older Sioul reads.", "{:?}", row("g-unknown"));
    }
}
