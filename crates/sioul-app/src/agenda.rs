// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The agenda's job: the days the agenda page asks for, made from the one
//! model of time (`sioul_core::timeline`), and published as its `agenda`
//! property (version 3), with the overlaps of the next two weeks.
//!
//! Within the plan's horizon the events are the plan's one read of the
//! calendars (`work::PlanShown`), when nothing was written since it was read;
//! else, and for the days before today or beyond the horizon, the job reads
//! the calendars itself, as the page's own read did. Today is never laid
//! here: the Tasks job lays it, and writes the slots of time for you, alone
//! (`PlanShown::today`). The job writes nothing, and reads health only as a
//! pure read (`Needs::read`, `Health::read`): never a record, never what the
//! other devices said. It computes nothing until the page asked once, nor
//! while Sioul is away on a phone (`backend::coalesced`).
//!
//! The Day and the Week (version 3) draw every kind: the events and the time
//! blocks, the plan's steps (today's as the Tasks job laid them, a day to
//! come's laid from the plan, within its reach), the tasks done, the time
//! kept for you, meals, naps and the night, the doses by key and time (today's
//! states are the Health job's rows, `doseMarks`), the days off; each with
//! what may change it (`timeline::Can`), placed and dressed in Rust
//! (`timeline::dress`), with the hours shown and today's head line given
//! ahead. The Month and What comes draw the events alone, as before.

use crate::backend::{QtThread, Shared, json, load_config, tr};
use crate::{mail, pim, work};
use cxx_qt_lib::QString;
use jiff::Zoned;
use jiff::tz::TimeZone;
use serde::Serialize;
use sioul_core::agenda::{self, Occurrence};
use sioul_core::timeline::{self, Placed};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// The days the agenda shows, version 3 of its JSON.
#[derive(Serialize)]
struct Shown {
    version: u32,
    /// The first day shown, "2026-10-03", to move week by week or month by month.
    from: String,
    /// "Nothing planned in the next two weeks.", when so.
    sentence: String,
    /// Whether an event can be added: a calendar that takes it exists.
    can_add: bool,
    /// The hours the Day and the Week show, the same every day (`timeline::dress`).
    #[serde(skip_serializing_if = "Option::is_none")]
    frame: Option<timeline::Frame>,
    /// Today's head line, given ahead (`timeline::head_line`), when today is shown.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    head: Vec<timeline::HeadFrame>,
    /// The medicines could not be read here: said where the doses would be.
    #[serde(skip_serializing_if = "String::is_empty")]
    doses_note: String,
    days: Vec<DayShown>,
}

/// One day, as `timeline::days` places it and `timeline::dress` dresses it,
/// each thing with its details.
#[derive(Serialize)]
struct DayShown {
    date: String,
    title: String,
    today: bool,
    past: bool,
    start: i64,
    hours_from: i32,
    hours_to: i32,
    head: Vec<Row>,
    timed: Vec<Row>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    hours: Vec<sioul_core::dayview::Stretch>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bands: Vec<Placed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    hidden: Vec<Placed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    doses: Vec<Placed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    edges: Vec<Placed>,
    planned: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    plan: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    more: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    said: Vec<String>,
}

/// A thing of the day: the item as placed, and an event's details for its
/// panel (until its card reads them itself).
#[derive(Serialize)]
struct Row {
    #[serde(flatten)]
    placed: Placed,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<Detail>,
}

/// What the event's panel says of it, beyond its item (its time, its file,
/// its calendar and colour are the item's: `timeline::Source`, `Time`); its
/// notes, organizer and guests come when it opens (`event_details`), so that
/// a week of long invitations stays light on a phone. What is false or empty
/// is left out.
#[derive(Serialize)]
struct Detail {
    /// This occurrence's end, Unix seconds (an all-day one's included).
    end: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    location: String,
    #[serde(skip_serializing_if = "is_false")]
    cancelled: bool,
    #[serde(skip_serializing_if = "is_false")]
    tentative: bool,
    #[serde(skip_serializing_if = "is_false")]
    read_only: bool,
    #[serde(skip_serializing_if = "is_false")]
    with_others: bool,
    /// The task it is the time block of, by UID, known here; "" for an event of its own.
    #[serde(skip_serializing_if = "String::is_empty")]
    task: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    list: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Detail {
    fn of(o: &Occurrence) -> Detail {
        Detail { end: o.end, location: o.location.clone(), cancelled: o.cancelled, tentative: o.tentative, read_only: o.read_only, with_others: o.with_others, task: o.task.clone(), list: o.list.clone() }
    }
}

/// One time of an event, read from its file when its panel opens: its notes,
/// organizer and guests, as JSON (`{notes, organizer, attendees}`); "{}" when
/// it is gone.
pub(crate) fn event_details(key: &str, start: i64) -> String {
    let Some(path) = pim::ours(key) else { return "{}".into() };
    let Some(calendar) = pim::collection_of(&path) else { return "{}".into() };
    let found = agenda::file_occurrences(&path, &calendar, start, start + 1, &TimeZone::system());
    found.iter().find(|o| o.start == start).map_or_else(|| "{}".to_string(), |o| serde_json::json!({ "notes": o.notes, "organizer": o.organizer, "attendees": o.attendees }).to_string())
}

/// What the Day and the Week show on this device ("Show"): the plan's steps,
/// meals, naps and the night, doses, and each calendar (by its name), all
/// shown unless you say. Kept in the state folder, never shared: each device
/// shows what you ask of it there. Written only when it changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct View {
    steps: bool,
    needs: bool,
    doses: bool,
    calendars_hidden: Vec<String>,
}

impl Default for View {
    fn default() -> View {
        View { steps: true, needs: true, doses: true, calendars_hidden: Vec::new() }
    }
}

fn view_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("agenda-view.toml")
}

/// What Show says, as JSON (`{steps, needs, doses, calendars_hidden}`); the
/// default when nothing was said or the file does not read.
pub(crate) fn view() -> String {
    json(&read_view())
}

fn read_view() -> View {
    std::fs::read_to_string(view_path()).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// Show changed (`view`'s JSON): written whole, beside then in place. Returns
/// what went wrong, else "".
pub(crate) fn set_view(asked: &str) -> String {
    let Ok(view) = serde_json::from_str::<View>(asked) else { return tr().text("agenda-show-not-saved", None) };
    if view == read_view() {
        return String::new();
    }
    let path = view_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text = toml::to_string(&view).unwrap_or_default();
    match sioul_core::filelock::replace(&path, text.as_bytes()) {
        Ok(()) => String::new(),
        Err(_) => tr().text("agenda-show-not-saved", None),
    }
}

/// Midnight of `date` where you are, Unix seconds.
fn midnight(date: jiff::civil::Date, zone: &TimeZone) -> i64 {
    timeline::midnight(date, zone)
}

/// The calendars' occurrences from `from` to `to` (Unix seconds): the plan's
/// one read where it reaches and nothing was written since it was read
/// (`work::PlanShown::current`), read here elsewhere; each occurrence once, in
/// the order `agenda::occurrences` gives them.
fn read_between(from: i64, to: i64, zone: &TimeZone) -> Vec<Occurrence> {
    let today = Zoned::now().date();
    let Some(plan) = work::plan_shown().filter(|s| s.made_on == today && s.current()) else {
        return agenda::occurrences(from, to);
    };
    let (reach_from, reach_to) = (midnight(today, zone), midnight(today, zone) + sioul_core::plan::HORIZON_DAYS * 86_400);
    let mut all: Vec<Occurrence> = plan.read().iter().filter(|o| o.end > from && o.start < to).cloned().collect();
    if from < reach_from {
        all.extend(agenda::occurrences(from, reach_from.min(to)));
    }
    if to > reach_to {
        all.extend(agenda::occurrences(reach_to.max(from), to));
    }
    // One read's edge and the other's: an event over both, once.
    let mut seen = std::collections::BTreeSet::new();
    all.retain(|o| seen.insert((o.key.clone(), o.start)));
    all.sort_by(|a, b| (a.start, !a.all_day, &a.summary).cmp(&(b.start, !b.all_day, &b.summary)));
    all
}

/// What the window's agenda shows from `from` to `to` (Unix seconds), the
/// phone's alike: the calendars' events (`read_between`), those whose
/// deletion waits ("Undo") aside, a task's time block marked as the task it
/// is, with its list (`agenda::mark_tasks`).
pub(crate) fn occurrences_between(from: i64, to: i64, removed: &std::collections::BTreeSet<std::path::PathBuf>, skipped: &std::collections::BTreeSet<(std::path::PathBuf, i64)>, tasks: &[sioul_core::tasks::Task]) -> Vec<Occurrence> {
    let mut items = pim::without_waiting(read_between(from, to, &TimeZone::system()), removed, skipped);
    agenda::mark_tasks(&mut items, tasks);
    items
}

/// The tasks a time block is marked with: the plan's while nothing was written
/// since it was read (a task renamed or moved since shows as it is now:
/// review of 1880bce, later 4), else those last read, else the tasks alone,
/// read now: never everything (`Loaded::read`), which may write the mail's
/// index (review of 1880bce, L9).
fn tasks_known(shared: &Shared) -> Vec<sioul_core::tasks::Task> {
    if let Some(plan) = work::plan_shown().filter(|p| p.current()) {
        return plan.loaded().tasks.clone();
    }
    if work::plan_shown().is_none()
        && let Some(loaded) = shared.loaded.lock().ok().and_then(|l| l.clone())
    {
        return loaded.tasks.clone();
    }
    sioul_core::tasks::all(&TimeZone::system())
}

/// The overlaps of the next two weeks (`pim::overlaps_of`), what waits for
/// Undo left out: the Porch's, made at every run whether the agenda page has
/// asked or not (review of 1880bce, M2).
fn overlaps_now(shared: &Shared) -> String {
    let zone = TimeZone::system();
    let reach = midnight(Zoned::now().date(), &zone);
    let (removed, skipped) = mail::hidden_pim(shared);
    pim::overlaps_of(&pim::without_waiting(read_between(reach, reach + 14 * 86_400, &zone), &removed, &skipped))
}

/// The days the page asks for (`pim::PimState`: from, how many, what comes
/// only), as JSON, and how long their read of the calendars took.
fn made(shared: &Shared) -> (String, std::time::Duration) {
    let state = shared.pim.lock().map(|s| s.clone()).unwrap_or_default();
    let zone = TimeZone::system();
    let now = Zoned::now();
    let today = now.date();
    let from = midnight(state.from, &zone);
    let mut to = from + state.days * 86_400 + 3_600;
    let (removed, skipped) = mail::hidden_pim(shared);
    let stamp = now.timestamp().as_second();
    // A task's time block shows as the task it is, with its list (`agenda::mark_tasks`).
    let tasks = tasks_known(shared);
    let started = std::time::Instant::now();
    // The Day and the Week draw every kind, the day before's night too (its morning); the
    // Month and What comes the events alone (the unified agenda, §4.7).
    let drawn = state.days == 1 || state.days == 7;
    let read_from = if drawn { from - 86_400 } else { from };
    // Today drawn: tomorrow read with it, for the head line (one read of the calendars).
    let after_tomorrow = today.tomorrow().and_then(|d| d.tomorrow()).map_or(to, |d| midnight(d, &zone));
    let today_start = midnight(today, &zone);
    if drawn && today_start >= from && today_start < to {
        to = to.max(after_tomorrow);
    }
    let occurrences: Vec<Occurrence> = occurrences_between(read_from, to, &removed, &skipped, &tasks)
        .into_iter()
        // What comes: an event over is the past, shown only when you go back to it.
        .filter(|o| !state.upcoming || o.end.max(o.start) > stamp)
        .collect();
    let read = started.elapsed();
    // An errand's block and steps are health: never named by its prescription
    // on the timeline (decision 7), never given to an agent (the MCP makes its
    // own items, through `ForAgents`). The record is never read here: an errand
    // is known by its category or its title.
    let errand = |uid: &str| tasks.iter().find(|t| t.uid == uid).is_some_and(|t| sioul_core::health::is_errand(t, &Default::default()));
    let mut items = agenda::items(&occurrences, &zone, &errand);
    let mut doses_note = String::new();
    let plan = work::plan_shown().filter(|p| p.made_on == today);
    if drawn {
        if let Some(plan) = &plan {
            // An errand's steps and its task done are health too, as its block is.
            items.extend(laid_items(plan, &occurrences, state.from, state.days, &now).into_iter().map(|mut item| {
                if errand(&item.owner) {
                    item.privacy = timeline::Privacy::Health;
                    item.address.clear();
                }
                item
            }));
        }
        let needs = sioul_core::needs::Needs::read(&sioul_core::health::Health::default_path());
        let days = plan.as_ref().filter(|p| p.current()).map_or_else(|| sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path()), |p| p.settings().needs_days.clone());
        let shifts_today = plan.as_ref().filter(|p| p.current()).map(|p| p.settings().shifts.clone());
        items.extend(needs_items(&needs, &days, shifts_today.as_ref(), &occurrences, state.from, state.days, &now));
        match sioul_core::health::Health::read(&sioul_core::health::Health::default_path()) {
            Ok(health) => items.extend(dose_items(&health, state.from, state.days, &now)),
            Err(_) => doses_note = tr().text("agenda-doses-unread", None),
        }
        items.extend(timeline::days_off(&load_config().time_off));
        // A task once a day: a block that holds its step gives way to it (`timeline::merge`).
        let open = |uid: &str| tasks.iter().any(|t| t.uid == uid && t.status.is_open());
        items = timeline::merge(items, stamp, &open);
    }
    for item in items.iter_mut() {
        // Time for you and the time kept free, by their words, on every day (today's the Tasks job dressed).
        if item.title.trim().is_empty() && matches!(item.kind, timeline::Kind::Gain | timeline::Kind::Slack) {
            item.title = tr().text(if item.kind == timeline::Kind::Gain { "capacity-gain-slot" } else { "capacity-slack" }, None);
        }
        // An event of no title, said so; an errand by its kind, never its prescription.
        if item.title.trim().is_empty() && matches!(item.kind, timeline::Kind::Event | timeline::Kind::Block) {
            item.title = tr().text("agenda-untitled", None);
        }
        if item.privacy == timeline::Privacy::Health && matches!(item.kind, timeline::Kind::Block | timeline::Kind::Step | timeline::Kind::Done | timeline::Kind::Event) {
            item.title = match sioul_core::health::errand_kind_of_title(&item.title) {
                Some(sioul_core::health::ErrandKind::Renew) => tr().text("agenda-errand-renew", None),
                _ => tr().text("agenda-errand-refill", None),
            };
        }
    }
    let mut days = timeline::days(&items, state.from, state.days, today, &zone, tr());
    timeline::set_hours(&mut days, &load_config().week_hours(), &zone);
    let (frame, head) = if drawn { dressed(&mut days, &items, plan.as_deref(), &occurrences, (read_from, to), &now) } else { (None, Vec::new()) };
    let detail = |placed: &Placed| match &placed.item.source {
        timeline::Source::Calendar { key, start, .. } => occurrences.iter().find(|o| &o.key == key && o.start == *start).map(Detail::of),
        _ => None,
    };
    let row = |placed: Placed| Row { detail: detail(&placed), placed };
    let empty = days.iter().all(|d| d.head.is_empty() && d.timed.is_empty());
    let can_add = agenda::default_calendar().is_some();
    let sentence = match (empty, can_add || !sioul_core::vdir::collections(sioul_core::vdir::Kind::Calendars).is_empty()) {
        (true, false) => tr().text("agenda-no-calendar", None),
        (true, true) => tr().text("agenda-nothing", None),
        _ => String::new(),
    };
    let shown = Shown {
        version: 3,
        from: state.from.to_string(),
        sentence,
        can_add,
        frame,
        head,
        doses_note,
        days: days
            .into_iter()
            .map(|d| DayShown {
                date: d.date,
                title: d.title,
                today: d.today,
                past: d.past,
                start: d.start,
                hours_from: d.hours_from,
                hours_to: d.hours_to,
                head: d.head.into_iter().map(row).collect(),
                timed: d.timed.into_iter().map(row).collect(),
                hours: d.hours,
                bands: d.bands,
                hidden: d.hidden,
                doses: d.doses,
                edges: d.edges,
                planned: d.planned,
                plan: d.plan,
                more: d.more,
                said: d.said,
            })
            .collect(),
    };
    (json(&shown), read)
}

/// The days from `from`, `count` of them: each date.
fn dates(from: jiff::civil::Date, count: i64) -> impl Iterator<Item = jiff::civil::Date> {
    (0..count).map_while(move |offset| from.checked_add(jiff::Span::new().days(offset)).ok())
}

/// Each day's steps, tasks done and time kept for you, as the plan lays them
/// (`timeline::from_day`): today as the Tasks job laid and dressed it
/// (`PlanShown::today`), the one producer of today's layout; a day to come
/// laid from the plan (`dayview::lay_on`, every task: in quiet time those
/// hidden now are folded into bands later, `timeline::dress`), within its
/// reach; a day gone by, its tasks done where their work was.
fn laid_items(plan: &work::PlanShown, occurrences: &[Occurrence], from: jiff::civil::Date, count: i64, now: &Zoned) -> Vec<timeline::Item> {
    let zone = now.time_zone();
    let tasks = &plan.loaded().tasks;
    let mut out = Vec::new();
    for date in dates(from, count) {
        if date == now.date() {
            out.extend(timeline::from_day(&plan.today, date, tasks));
            continue;
        }
        let start = midnight(date, zone);
        let end = date.tomorrow().map_or(start + 86_400, |d| midnight(d, zone));
        let events: Vec<Occurrence> = occurrences.iter().filter(|o| o.end > start && o.start < end).cloned().collect();
        let laid = sioul_core::dayview::lay_on(date, now, &events, plan.plan(), tasks, plan.settings(), sioul_core::dayview::Drawn::ADMIN);
        out.extend(timeline::from_day(&laid, date, tasks));
    }
    out
}

/// Meals, naps and the night, each day's own as it has them (`Needs::blocks_of`:
/// moved, its times changed, quiet that day; taken out that day, not there),
/// and the night before the first day (its morning): Health's, never
/// beyond the window, never named but by their own names. Meals sit where
/// the plan keeps them: today's moved past today's events as the plan moved
/// them (`Settings::shifts`), another day's past its events by the plan's rule
/// (`work::held_from`). What each lets change it: a day past, or a block over,
/// stays as it was (its writer refuses it: `Needs::change_day`).
fn needs_items(needs: &sioul_core::needs::Needs, days: &sioul_core::needs::Days, shifts_today: Option<&std::collections::BTreeMap<String, i64>>, occurrences: &[Occurrence], from: jiff::civil::Date, count: i64, now: &Zoned) -> Vec<timeline::Item> {
    use sioul_core::timeline::{Can, Certainty, Form, Item, Kind, Move, Privacy, Source, Time, Tone};
    let zone = now.time_zone();
    let today = now.date();
    let stamp = now.timestamp().as_second();
    let mut out = Vec::new();
    let before = from.yesterday().ok();
    for date in before.into_iter().chain(dates(from, count)) {
        let held = || {
            let start = midnight(date, zone);
            let end = date.tomorrow().map_or(start + 86_400, |d| midnight(d, zone));
            let near: Vec<Occurrence> = occurrences.iter().filter(|o| o.end > start - 86_400 && o.start < end + 86_400).cloned().collect();
            work::held_by_plan(date, now).unwrap_or_else(|| work::held_from(date, now, near))
        };
        let pushed = match (date == today, shifts_today) {
            (true, Some(shifts)) => shifts.clone(),
            _ => needs.past_events_on(date, zone, days, &held()),
        };
        for kept in needs.blocks_of(date, zone, days, &|key: &str| pushed.get(key).copied().unwrap_or(0)) {
            // The day before the first: its night alone, for its morning.
            if Some(date) == before && kept.kind != "sleep" {
                continue;
            }
            let kind = match kept.kind {
                "meal" => Kind::Meal,
                "nap" => Kind::Nap,
                _ => Kind::Night,
            };
            let until = match kind {
                Kind::Night => kept.end,
                _ => needs.lengths(date, &kept.key, days).map_or(kept.end, |(minutes, _, _)| (kept.at + i64::from(minutes) * 60).min(kept.end)),
            };
            let change = days.get(date, &kept.key);
            let (past, over) = (date < today, kept.end <= stamp);
            let why = if past {
                tr().text("need-past", None)
            } else if over {
                tr().text("need-over", None)
            } else {
                String::new()
            };
            out.push(Item {
                id: timeline::need_id(date, &kept.key),
                kind,
                time: Time::Span { start: kept.start, at: kept.at, until, end: kept.end },
                certainty: if change.is_some() { Certainty::ThatDay } else { Certainty::Usual },
                source: Source::Needs { date, key: kept.key.clone() },
                can: Can { edit: if past { Form::None } else { Form::Need }, drag: if past || over { Move::None } else { Move::Need }, ends: !past && !over, delete: false, tick: false, why },
                privacy: Privacy::Health,
                title: crate::health::block_name(kept.kind, &kept.key, &kept.name),
                line: String::new(),
                said: String::new(),
                tone: if change.is_some_and(|c| c.quiet) { Tone::Quiet } else { Tone::Plain },
                colour: None,
                address: String::new(),
                owner: String::new(),
                holds_time: true,
            });
        }
    }
    out
}

/// The doses of today and of the days to come shown, by key and time, read
/// from the health file alone (`Health::read`: no record, nothing written):
/// today's are drawn in the state the Health job's rows give them (`doseMarks`;
/// one the rows do not hold is being read), a day to come's plainly; a day
/// gone by draws none, which could read as missed. Named "Dose", never by
/// the medicine (decision 7); nothing to press on the timeline.
fn dose_items(health: &sioul_core::health::Health, from: jiff::civil::Date, count: i64, now: &Zoned) -> Vec<timeline::Item> {
    use sioul_core::timeline::{Can, Certainty, DoseState, Item, Kind, Privacy, Source, Time, Tone};
    let zone = now.time_zone();
    let today = now.date();
    let first = from.max(today);
    let Some(last) = from.checked_add(jiff::Span::new().days(count)).ok().filter(|last| *last > first) else { return Vec::new() };
    let (Ok(start), Ok(end)) = (first.to_zoned(zone.clone()), last.to_zoned(zone.clone())) else { return Vec::new() };
    health
        .doses(&start, &end)
        .into_iter()
        .map(|dose| {
            let ahead = dose.at.date() > today;
            Item {
                id: timeline::dose_id(&dose.key),
                kind: Kind::Dose,
                time: Time::At { at: dose.at.timestamp().as_second() },
                certainty: Certainty::Dose(if ahead { DoseState::Ahead } else { DoseState::Reading }),
                source: Source::Dose { key: dose.key },
                can: Can::nothing(),
                privacy: Privacy::Health,
                title: tr().text("agenda-dose", None),
                line: String::new(),
                said: String::new(),
                tone: if ahead { Tone::Muted } else { Tone::Plain },
                colour: None,
                address: String::new(),
                owner: String::new(),
                holds_time: false,
            }
        })
        .collect()
}

/// The days laid for the Day and the Week, dressed (`timeline::dress`): their
/// hours, the night's edges, the doses' lane, quiet time's bands, the lists'
/// line, today's lines; the hours shown; and today's head line, given ahead.
fn dressed(days: &mut [timeline::Day], items: &[timeline::Item], plan: Option<&work::PlanShown>, occurrences: &[Occurrence], read: (i64, i64), now: &Zoned) -> (Option<timeline::Frame>, Vec<timeline::HeadFrame>) {
    let zone = now.time_zone().clone();
    let today = now.date();
    let config = load_config();
    let settings = plan.map(work::PlanShown::settings);
    let hours = |date: jiff::civil::Date| -> Vec<sioul_core::dayview::Stretch> {
        let stretches = match settings {
            Some(settings) => settings.hours_on(date, &zone),
            None => sioul_core::plan::Settings::of_hours(&config.week_hours(), sioul_core::areas::TaskAreas::usual()).hours_on(date, &zone),
        };
        stretches.into_iter().map(|(start, end, area)| sioul_core::dayview::Stretch { start, end, kind: area.id() }).collect()
    };
    // In quiet time, what the task pages hide now (decision g): a coming day's steps of it make bands.
    let tasks = plan.map(|p| p.loaded().tasks.as_slice()).unwrap_or_default();
    let quiet = plan.and_then(work::PlanShown::quiet);
    let hidden = |uid: &str| quiet.is_some_and(|q| tasks.iter().find(|t| t.uid == uid).is_some_and(|t| !q.keeps(t)));
    let horizon = today.checked_add(jiff::Span::new().days(sioul_core::plan::HORIZON_DAYS - 1)).unwrap_or(today);
    let dressing = timeline::Dressing {
        today,
        now: now.timestamp().as_second(),
        zone: &zone,
        tr: tr(),
        hours: &hours,
        horizon,
        hidden: &hidden,
        said: plan.map(|p| p.today.said.clone()).unwrap_or_default(),
        day_start: i32::try_from(config.agenda.day_start.unwrap_or(7).min(23)).unwrap_or(7),
    };
    let frame = timeline::dress(days, &dressing);
    if !days.iter().any(|d| d.today) {
        return (Some(frame), Vec::new());
    }
    (Some(frame), head_frames(items, plan, occurrences, read, now))
}

/// Today's head line from now to midnight, given ahead (`timeline::head_line`):
/// what each moment is for (`quiet::frames`, the home card's), today's things
/// and tomorrow's first appointment, and tomorrow morning's alarm. Tomorrow's
/// appointments come from the days' read when it holds tomorrow (`read`, its
/// span), else are read.
fn head_frames(items: &[timeline::Item], plan: Option<&work::PlanShown>, occurrences: &[Occurrence], read: (i64, i64), now: &Zoned) -> Vec<timeline::HeadFrame> {
    let zone = now.time_zone().clone();
    let today = now.date();
    let config = load_config();
    let Some(tomorrow) = today.tomorrow().ok() else { return Vec::new() };
    let (Ok(next_midnight), Ok(after)) = (tomorrow.to_zoned(zone.clone()), tomorrow.tomorrow().and_then(|d| d.to_zoned(zone.clone()))) else { return Vec::new() };
    let needs = sioul_core::needs::Needs::read(&sioul_core::health::Health::default_path());
    let days = plan.filter(|p| p.current()).map_or_else(|| sioul_core::needs::Days::load(&sioul_core::needs::Days::default_path()), |p| p.settings().needs_days.clone());
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let held = work::held_by_plan(today, now).unwrap_or_else(|| sioul_core::plan::event_spans(occurrences, 0));
    let blocks = sioul_core::quiet::Blocks::of(&needs, &days, &held, now);
    let moments: Vec<(i64, i64, sioul_core::quiet::Mode)> = sioul_core::quiet::frames(&config.week_hours(), &config.time_off, &overrides, &blocks, &[], now, &next_midnight, &[], 48)
        .into_iter()
        .map(|(from, until, mode)| (from.timestamp().as_second(), until.timestamp().as_second(), mode))
        .collect();
    let (start, end) = (midnight(today, &zone), next_midnight.timestamp().as_second());
    let today_items: Vec<timeline::Item> = items
        .iter()
        .filter(|item| {
            let (at, until) = item.time.whole(&zone);
            at < end && until > start
        })
        .cloned()
        .collect();
    // Tomorrow's appointments, whether tomorrow is shown or not.
    let next_start = next_midnight.timestamp().as_second();
    let after_start = after.timestamp().as_second();
    let tomorrow_events: Vec<Occurrence> = if read.0 <= next_start && read.1 >= after_start {
        occurrences.iter().filter(|o| o.end > next_start && o.start < after_start).cloned().collect()
    } else {
        read_between(next_start, after_start, &zone)
    };
    let tomorrow_items = agenda::items(&tomorrow_events, &zone, &|_| false);
    let alarm = needs.alarm_of(today, &zone, &days).filter(|a| !a.skipped).map(|a| a.at);
    let inputs = timeline::HeadInputs { now: now.timestamp().as_second(), next_midnight: next_start, moments: &moments, today: &today_items, tomorrow: &tomorrow_items, alarm, zone: &zone, tr: tr() };
    timeline::head_line(&inputs)
}

/// Whether a run makes the days: once the page asked for them (`asked`), or
/// from the first plan (the Tasks job's first run) until the window has shown
/// them once, so that the page's first opening never shows an empty week,
/// which would read as "nothing planned" (review of 1880bce, L18; of
/// 85da7f0, later 10).
fn days_wanted(shared: &Shared) -> bool {
    shared.agenda_asked.load(Ordering::Relaxed) || (work::plan_shown().is_some() && !shared.agenda_prefilled.load(Ordering::Relaxed))
}

/// One run of the agenda's job: the overlaps of the next two weeks, always,
/// for the Porch; the days, with their own generation, once they are wanted
/// (`days_wanted`), with how long their read of the calendars took.
struct Run {
    generation: u64,
    overlaps: String,
    days: Option<(u64, String, std::time::Duration)>,
}

fn run(shared: &Shared) -> Run {
    let generation = shared.agenda_generation.fetch_add(1, Ordering::Relaxed) + 1;
    let overlaps = overlaps_now(shared);
    let days = days_wanted(shared).then(|| {
        let (agenda, read) = made(shared);
        (shared.agenda_days_generation.fetch_add(1, Ordering::Relaxed) + 1, agenda, read)
    });
    Run { generation, overlaps, days }
}

/// Makes the overlaps of the next two weeks at every run, for the Porch, and
/// the agenda's days once they are wanted (`days_wanted`), on a thread, then
/// shows them: whenever the plan or a calendar changes.
pub(crate) fn show_agenda(qt: &QtThread, shared: &Arc<Shared>) {
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.agenda_job, move |shared| {
        let started = std::time::Instant::now();
        let Run { generation, overlaps, days } = run(shared);
        if crate::timed() {
            let (bytes, read) = days.as_ref().map_or((0, 0), |(_, agenda, read)| (agenda.len(), read.as_millis()));
            eprintln!("sioul-perf: agenda job {} ms, its calendars read {read} ms, {bytes} bytes", started.elapsed().as_millis());
        }
        let _ = qt.queue(move |mut sioul| {
            // Only the newest (see `backend::show`): the newest days of the runs
            // that made days, the newest overlaps of all.
            let shared = sioul.shared();
            if let Some((days_generation, agenda, _)) = &days
                && shared.agenda_days_generation.load(Ordering::Relaxed) == *days_generation
                && shared.agenda_days_shown_generation.fetch_max(*days_generation, Ordering::Relaxed) <= *days_generation
            {
                shared.agenda_prefilled.store(true, Ordering::Relaxed);
                sioul.as_mut().set_agenda(QString::from(agenda));
            }
            let shared = sioul.shared();
            if shared.agenda_generation.load(Ordering::Relaxed) == generation && shared.agenda_shown_generation.fetch_max(generation, Ordering::Relaxed) <= generation {
                sioul.as_mut().set_overlaps(QString::from(&overlaps));
            }
        });
    });
}

/// The days the agenda page shows from now on (`show_days`): it has asked.
pub(crate) fn asked(qt: &QtThread, shared: &Arc<Shared>) {
    shared.agenda_asked.store(true, Ordering::Relaxed);
    show_agenda(qt, shared);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::vdir::{self, Kind};

    /// Every file under `dir`, with its length and time: what a run wrote, compared.
    fn files(dir: &std::path::Path) -> Vec<(std::path::PathBuf, u64, Option<std::time::SystemTime>)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(at) = stack.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                let path = entry.path();
                match entry.metadata() {
                    Ok(meta) if meta.is_dir() => stack.push(path),
                    Ok(meta) => out.push((path, meta.len(), meta.modified().ok())),
                    Err(_) => {}
                }
            }
        }
        out.sort();
        out
    }

    /// Show is this device's: everything shown when nothing was said (or the
    /// file does not read), what is said read back, the same said again
    /// written nothing, and nothing but its own file written.
    #[test]
    fn show_is_kept_on_this_device_and_written_only_when_it_changes() {
        sioul_core::config::stand_ins_only();
        let path = view_path();
        let _ = std::fs::remove_file(&path);
        assert_eq!(read_view(), View::default());
        let all: serde_json::Value = serde_json::from_str(&view()).unwrap();
        assert_eq!(all, serde_json::json!({ "steps": true, "needs": true, "doses": true, "calendars_hidden": [] }));
        assert_eq!(set_view(r#"{"steps": true, "needs": false, "doses": true, "calendars_hidden": ["Club"]}"#), "");
        assert_eq!(read_view(), View { needs: false, calendars_hidden: vec!["Club".into()], ..View::default() });
        let written = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(set_view(r#"{"needs": false, "calendars_hidden": ["Club"]}"#), "", "what is not said stays shown");
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), written, "the same said again writes nothing");
        assert_eq!(set_view("not json"), tr().text("agenda-show-not-saved", None));
        std::fs::write(&path, "steps = [").unwrap();
        assert_eq!(read_view(), View::default(), "a file that does not read shows everything");
        let _ = std::fs::remove_file(&path);
    }

    /// The agenda's job makes its days while another thread holds every
    /// health lock (it takes none), writes no file, and reads the calendars
    /// itself once something was written after the plan it would read.
    #[test]
    fn the_agenda_job_writes_nothing_and_waits_for_no_health_lock() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Agenda job", None, &["VEVENT"]).unwrap();
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let tomorrow = now.date().tomorrow().unwrap();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let event = |uid: &str, hour: i8| format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{uid}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(hour), utc(hour + 1));
        std::fs::write(calendar.dir.join("dentist.ics"), event("agenda-job-dentist", 9)).unwrap();
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            // The week, which reads health's needs and doses, by pure reads, while every health lock is held.
            state.from = now.date();
            state.days = 7;
            state.upcoming = false;
        }
        // What a reader of health could write: the health file's last-read and aside copies; and
        // this test's calendar. Other tests write elsewhere in the same folders meanwhile; the
        // health tests, which write and read these, wait (their own read once came in as this
        // job's write). Today's slots of time for you, which the Tasks job writes as it lays
        // today (other tests do it meanwhile): this thread, where the job runs, never lays it.
        let _health_tests = crate::health::health_files_held();
        let health = sioul_core::health::Health::default_path();
        let watched = [sioul_core::health::Health::last_read_path(&health), health.with_extension("toml.unreadable")];
        crate::capacity::DRESSED.with(|n| n.set(0));
        let state = |path: &std::path::Path| std::fs::metadata(path).ok().map(|m| (m.len(), m.modified().ok()));
        let seen = || (watched.iter().map(|p| state(p)).collect::<Vec<_>>(), files(&calendar.dir));
        let before = seen();
        // With no plan yet, the job reads the tasks alone, never everything, which writes the
        // mail's index (review of 1880bce, L9): counted on this thread, where the job runs.
        crate::work::LOADS.with(|n| n.set(0));
        // Every health lock held on another thread, for longer than a run takes.
        let (held, release) = std::sync::mpsc::channel::<()>();
        let (go, ready) = std::sync::mpsc::channel::<()>();
        let holder = std::thread::spawn(move || {
            let _locks = crate::health::every_lock_held();
            let _ = go.send(());
            let _ = release.recv_timeout(std::time::Duration::from_secs(20));
        });
        ready.recv().unwrap();
        let started = std::time::Instant::now();
        let (shown, _) = made(&shared);
        let _ = overlaps_now(&shared);
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "it waited for a health lock");
        let _ = held.send(());
        holder.join().unwrap();
        assert!(shown.contains("agenda-job-dentist"), "{shown}");
        let parsed: serde_json::Value = serde_json::from_str(&shown).unwrap();
        assert_eq!(parsed["version"], 3);
        assert_eq!(before, seen(), "nothing written");
        assert_eq!(crate::capacity::DRESSED.with(std::cell::Cell::get), 0, "today never laid nor its slots written");
        assert_eq!(crate::work::LOADS.with(std::cell::Cell::get), 0, "the tasks alone read");
        // A write after the plan it reads: the calendars read again, the new event there at once.
        let written = sioul_core::timeline::written();
        vdir::write_item(&calendar.dir.join("visit.ics"), &event("agenda-job-visit", 14)).unwrap();
        assert!(sioul_core::timeline::written() > written, "a calendar write is counted");
        let (shown, _) = made(&shared);
        assert!(shown.contains("agenda-job-visit"), "{shown}");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }
    /// A time block is marked with its task as it is now: renamed after the
    /// plan was made, its new title at once (review of 1880bce, later 4).
    #[test]
    fn a_block_is_marked_with_its_task_as_it_is_now() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "Marks test", None, &["VTODO"]).unwrap();
        let (zone, now) = (TimeZone::system(), Zoned::now());
        let write = |title: &str| {
            let edit = sioul_core::tasks::TaskEdit { title: title.into(), ..sioul_core::tasks::TaskEdit::default() };
            vdir::write_item(&list.dir.join("marked.ics"), &sioul_core::tasks::new_task(&edit, "marked-task", &zone, &now).unwrap()).unwrap();
        };
        write("Marked as made");
        let shared = Arc::new(Shared::default());
        work::publish_now(&shared);
        let title = |shared: &Shared| tasks_known(shared).into_iter().find(|t| t.uid == "marked-task").map(|t| t.title);
        assert_eq!(title(&shared).as_deref(), Some("Marked as made"));
        write("Marked as renamed");
        assert_eq!(title(&shared).as_deref(), Some("Marked as renamed"), "the plan older: the tasks read again");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&list.dir);
    }

    /// The Porch's overlaps are made at every run, the agenda page never
    /// opened: two of today's events at once are said from the start (review of
    /// 1880bce, M2); the days wait for the page's first ask, or the first plan.
    #[test]
    fn the_porch_s_overlaps_never_wait_for_the_agenda_page() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Overlaps", None, &["VEVENT"]).unwrap();
        let now = Zoned::now();
        let utc = |minutes: i64| now.checked_add(jiff::Span::new().minutes(minutes)).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M00Z").to_string();
        for (uid, from, to) in [("overlap-dentist", 120, 180), ("overlap-bank", 150, 210)] {
            std::fs::write(calendar.dir.join(format!("{uid}.ics")), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:{uid}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(from), utc(to))).unwrap();
        }
        let shared = Arc::new(Shared::default());
        assert!(!shared.agenda_asked.load(Ordering::Relaxed));
        // The job's own run (review of 85da7f0, later 13): the overlaps, and no days.
        let first = run(&shared);
        assert!(first.overlaps.contains("overlap-dentist") && first.overlaps.contains("overlap-bank"), "{}", first.overlaps);
        assert!(first.days.is_none(), "no days made: the page never asked, no plan yet");
        // From the first plan, the days, again at each run until the window has
        // shown them: a newer run never drops them (review of 85da7f0, later 10).
        work::publish_now(&shared);
        let (second, third) = (run(&shared), run(&shared));
        assert!(second.days.is_some() && third.days.is_some(), "made until shown");
        assert!(third.days.as_ref().unwrap().0 > second.days.as_ref().unwrap().0, "each with its own generation");
        shared.agenda_prefilled.store(true, Ordering::Relaxed);
        assert!(run(&shared).days.is_none(), "shown once: then on the page's ask");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// A heavy week (fifteen events a day, each with a few lines of notes)
    /// stays within the phone's budget for the agenda's JSON: under 150 KB
    /// (the unified agenda, §4.9).
    #[test]
    fn a_heavy_week_stays_within_its_budget() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        // Meals, the night and four dose times a day (the unified agenda, §4.9), the health
        // tests' files held meanwhile: a measure of its own, never another test's.
        let _health_tests = crate::health::health_files_held();
        let health = sioul_core::health::Health::default_path();
        std::fs::create_dir_all(health.parent().unwrap()).unwrap();
        std::fs::write(&health, "[needs]\nmeals_on = true\nsleep_on = true\n\n[[medicine]]\nid = \"heavy\"\nname = \"Heavy\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"12:00\", \"16:00\", \"20:00\"]\n").unwrap();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Heavy week", None, &["VEVENT"]).unwrap();
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let monday = now.date();
        for day in 0..7 {
            let date = monday.checked_add(jiff::Span::new().days(day)).unwrap();
            for n in 0..15 {
                let utc = |minute: i64| date.at(7, 0, 0, 0).to_zoned(zone.clone()).unwrap().checked_add(jiff::Span::new().minutes(minute)).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
                let text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:heavy-{day}-{n}\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Appointment number {n} of a long day\r\nLOCATION:12 rue de l'Exemple, 75000 Paris\r\nDESCRIPTION:Bring the papers.\\nAsk about the next visit.\\nThe door code is on the letter.\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(n * 50), utc(n * 50 + 45));
                std::fs::write(calendar.dir.join(format!("heavy-{day}-{n}.ics")), text).unwrap();
            }
        }
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            state.from = monday;
            state.days = 7;
            state.upcoming = false;
        }
        let began = std::time::Instant::now();
        let (shown, read) = made(&shared);
        let first = began.elapsed();
        // Said for the record (the plan's aim: composed in under 20 ms on a computer, beside the
        // calendars' read): the first made, then again as each minute's turn makes it.
        let again = std::time::Instant::now();
        let _ = made(&shared);
        let took = again.elapsed();
        let ours = shown.matches("\"heavy-").count();
        assert!(ours >= 105, "{ours}");
        eprintln!("sioul-perf: a heavy week's agenda JSON: {} bytes, made in {} ms the first time, {} ms again, of which its calendars' read {} ms", shown.len(), first.as_millis(), took.as_millis(), read.as_millis());
        assert!(shown.len() < 150_000, "{} bytes", shown.len());
        // The Month (six weeks from this week's Monday) and the 15-day list, said for the record.
        for (name, days) in [("month", 42), ("list", 15)] {
            if let Ok(mut state) = shared.pim.lock() {
                state.from = monday;
                state.days = days;
            }
            eprintln!("sioul-perf: a heavy {name}'s agenda JSON ({days} days, the heavy week in it): {} bytes", made(&shared).0.len());
        }
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_file(&health);
    }

    /// A time block dragged is one write, and the agenda shows it at its new
    /// time at once and after the plan is made again at that write, never
    /// back at the old one (unified agenda, plan step 7: a drag published
    /// once, at its generation; review of 1880bce, L22).
    #[test]
    fn a_drag_is_shown_once_at_its_generation() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Drag test", None, &["VEVENT", "VTODO"]).unwrap();
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let tomorrow = now.date().tomorrow().unwrap();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let edit = sioul_core::tasks::TaskEdit { title: "Drag test task".into(), estimate: 30, ..sioul_core::tasks::TaskEdit::default() };
        vdir::write_item(&calendar.dir.join("drag-task.ics"), &sioul_core::tasks::new_task(&edit, "drag-task", &zone, &now).unwrap()).unwrap();
        let block = calendar.dir.join("drag-block.ics");
        let text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:drag-block\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Drag test task\r\n{}:drag-task\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(9), utc(10), sioul_core::blocks::TASK);
        vdir::write_item(&block, &text).unwrap();
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            state.from = now.date();
            state.days = 3;
            state.upcoming = false;
        }
        work::publish_now(&shared);
        // The block's minute of the day as the agenda shows it.
        let minute = |shared: &Shared| -> Option<i64> {
            let shown: serde_json::Value = serde_json::from_str(&made(shared).0).unwrap();
            let rows: Vec<serde_json::Value> = shown["days"].as_array()?.iter().filter(|d| d["date"] == tomorrow.to_string()).flat_map(|d| d["timed"].as_array().cloned().unwrap_or_default()).collect();
            rows.iter().find(|r| r["item"]["source"].to_string().contains("\"drag-block\"")).and_then(|r| r["from_minute"].as_i64())
        };
        assert_eq!(minute(&shared), Some(9 * 60));
        // The drag: the block written at 11:00 through the door a drag takes (`blocks::moved`, then `write`).
        let generation = sioul_core::timeline::written();
        let at = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let moved = sioul_core::blocks::moved(&text, at(11), at(12), &zone).unwrap();
        vdir::write_item(&block, &moved).unwrap();
        // Other tests write beside this one (the count is the process's): counted at least once.
        assert!(sioul_core::timeline::written() > generation, "the drag is a write, counted");
        assert!(work::plan_shown().is_some_and(|p| !p.current()), "the plan made before it is older");
        assert_eq!(minute(&shared), Some(11 * 60), "shown at its new time at once");
        // The plan made again after it, current (made again while another test wrote meanwhile):
        // still at its new time, and the block as the drag wrote it.
        for _ in 0..5 {
            work::publish_now(&shared);
            if work::plan_shown().is_some_and(|p| p.current()) {
                break;
            }
        }
        assert!(work::plan_shown().is_some_and(|p| p.stamp.written > generation), "made at the drag's generation or after");
        assert_eq!(minute(&shared), Some(11 * 60), "never back at the old one");
        assert_eq!(std::fs::read_to_string(&block).unwrap(), moved, "shown, never written again");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// The Day and the Week draw every kind (the unified agenda, part 2):
    /// the events, the plan's steps, meals and the night (its parts at the
    /// edges), the doses by key and time, today's plainly being read and a
    /// day to come's plainly ahead, the hours shown and today's head line;
    /// an errand's block named by its kind, never by its prescription, and no
    /// dose by its medicine.
    #[test]
    fn the_day_and_the_week_draw_every_kind() {
        let _health_tests = crate::health::health_files_held();
        sioul_core::config::stand_ins_only();
        let health = sioul_core::health::Health::default_path();
        std::fs::create_dir_all(health.parent().unwrap()).unwrap();
        std::fs::write(&health, "[needs]\nmeals_on = true\nsleep_on = true\n\n[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"20:00\"]\n").unwrap();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Every kind", None, &["VEVENT", "VTODO"]).unwrap();
        let (now, zone) = (Zoned::now(), TimeZone::system());
        let tomorrow = now.date().tomorrow().unwrap();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        std::fs::write(calendar.dir.join("kinds-event.ics"), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:kinds-event\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Kinds appointment\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(10), utc(11))).unwrap();
        let task = |uid: &str, title: &str| {
            let edit = sioul_core::tasks::TaskEdit { title: title.into(), estimate: 30, ..sioul_core::tasks::TaskEdit::default() };
            vdir::write_item(&calendar.dir.join(format!("{uid}.ics")), &sioul_core::tasks::new_task(&edit, uid, &zone, &now).unwrap()).unwrap();
        };
        task("kinds-task", "Kinds step");
        task("kinds-errand", "Pharmacy: Kindsoprazole 20 mg");
        std::fs::write(calendar.dir.join("kinds-errand-block.ics"), format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:kinds-errand-block\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Pharmacy: Kindsoprazole 20 mg\r\n{}:kinds-errand\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(15), utc(16), sioul_core::blocks::TASK)).unwrap();
        let shared = Arc::new(Shared::default());
        if let Ok(mut state) = shared.pim.lock() {
            state.from = now.date();
            state.days = 7;
            state.upcoming = false;
        }
        work::publish_now(&shared);
        let shown = made(&shared).0;
        let parsed: serde_json::Value = serde_json::from_str(&shown).unwrap();
        let days = parsed["days"].as_array().unwrap();
        // A coming day's steps hidden now (quiet time, the night) are drawn as bands, kept apart.
        let kinds: std::collections::BTreeSet<String> = days.iter().flat_map(|d| d["timed"].as_array().cloned().unwrap_or_default().into_iter().chain(d["hidden"].as_array().cloned().unwrap_or_default())).map(|r| r["item"]["kind"].as_str().unwrap_or("").to_string()).collect();
        for kind in ["event", "step", "meal"] {
            assert!(kinds.contains(kind), "{kind} in {kinds:?}");
        }
        assert!(days.iter().all(|d| d["edges"].as_array().is_some_and(|e| !e.is_empty())), "the night's parts on each day");
        let doses = |d: &serde_json::Value| d["doses"].as_array().cloned().unwrap_or_default();
        let dose = tr().text("agenda-dose", None);
        assert!(doses(&days[1]).iter().all(|r| r["item"]["certainty"]["dose"]["state"] == "ahead" && r["item"]["title"] == dose.as_str()), "{}", days[1]["doses"]);
        assert_eq!(doses(&days[1]).len(), 2);
        assert!(doses(&days[0]).iter().all(|r| r["item"]["certainty"]["dose"]["state"] == "reading"));
        assert!(parsed["frame"]["from"].as_i64().is_some_and(|from| from <= 7 * 60), "{}", parsed["frame"]);
        assert!(parsed["head"].as_array().is_none_or(|head| head.iter().all(|f| f["text"].as_str().is_some_and(|t| !t.is_empty()))), "today's head line: {}", parsed["head"]);
        assert!(!shown.contains("Kindsoprazole") && !shown.contains("Lumiel"), "never a prescription's or a medicine's name");
        assert!(shown.contains(&format!("\"{}\"", tr().text("agenda-errand-refill", None))), "the errand by its kind");
        work::forget_plan();
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_file(&health);
    }
}
