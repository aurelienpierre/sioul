// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Health and well-being, for the window: the page (today's doses, the
//! medicines, the prescriptions), its forms, and the minute tick that reminds
//! a dose once, quietly, and turns refills and renewals into tasks in a list
//! your phone has. The rest goes to no server, except sealed to your other
//! computers when you share with them (docs/database.md).

use crate::backend::{QtThread, Shared, json, say, tell, tr};
use crate::work;
use cxx_qt_lib::QString;
use jiff::civil::Date;
use jiff::{Span, Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use sioul_core::health::{ChatLimit, Doubt, ErrandKind, Health, HealthState, Medicine, Movement, Peer, Prescription, Problem, Schedule};
use sioul_core::tasks::TaskEdit;
use std::sync::Arc;

fn load() -> Health {
    Health::load(&Health::default_path())
}

fn save(health: &Health) -> Result<(), String> {
    health.save(&Health::default_path())
}

/// "12:00 and 18:00", "12:00, 15:00 and 18:00".
fn listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), tr().text("word-and", None)),
    }
}

/// A schedule in words: "every day at 12:00 and 18:00".
fn words(schedule: &Schedule) -> String {
    match schedule {
        Schedule::Day { times } => say("health-every-day", &[("times", listed(times))]),
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
    /// a sentence; "" when it is known (docs/health.md, "Knowing").
    doubt: String,
}

#[derive(Serialize)]
struct MedicineRow {
    #[serde(flatten)]
    medicine: Medicine,
    when: String,
    prescription_title: String,
}

#[derive(Serialize)]
struct PrescriptionRow {
    #[serde(flatten)]
    prescription: Prescription,
    /// "Pharmacy from Tuesday 27 October", "Renew from Thursday 17 December": the next of each.
    next: Vec<String>,
}

#[derive(Serialize)]
struct HealthPage {
    /// The watch: what it says of today and the week, and where its files come from.
    watch: WatchView,
    today: Vec<DoseRow>,
    /// Doses due while Sioul ran nowhere, neither marked nor reminded: a question on the past.
    missed: Vec<DoseRow>,
    /// Why a dose marked elsewhere may not show here: this computer alone, or
    /// your other computers not heard from lately; "" when all is known.
    shared_note: String,
    /// Reminders come on another computer, the one you are at: said, by its name.
    reminded_there: String,
    medicines: Vec<MedicineRow>,
    prescriptions: Vec<PrescriptionRow>,
    movement: Movement,
    chats: ChatLimit,
    /// Where the errands go, and the lists they can go to: {id, name}.
    errands_list: String,
    lists: Vec<serde_json::Value>,
}

/// The watch, as the page shows it: words ready, curves for today.
#[derive(Serialize, Default)]
struct WatchView {
    /// Any data at all.
    any: bool,
    /// "Last data: today at 14:05", or nothing yet.
    synced: String,
    folder: String,
    offers: bool,
    steps: String,
    resting: String,
    slept: String,
    battery: String,
    week: String,
    heart_rate: Vec<(i64, u8)>,
    stress: Vec<(i64, u8)>,
    battery_curve: Vec<(i64, u8)>,
    /// The day's span on the curves: from midnight, 24 hours.
    from: i64,
}

fn watch_view(health: &Health) -> WatchView {
    let now = Zoned::now();
    let zone = now.time_zone().clone();
    let s = sioul_core::wearable::summary(&sioul_core::wearable::folder(), now.date(), &zone);
    let hm = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    let hours = |minutes: u32| say("watch-hours", &[("h", (minutes / 60).to_string()), ("m", format!("{:02}", minutes % 60))]);
    // "2 081" in French, "2,081" in English: the language's own separator.
    let separator = tr().text("thousands-separator", None);
    let grouped = |n: u32| {
        let digits = n.to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push_str(&separator);
            }
            out.push(c);
        }
        out
    };
    WatchView {
        any: s.newest > 0,
        synced: if s.newest > 0 { say("watch-synced", &[("when", Timestamp::from_second(s.newest).map(|t| tr().when(&t.to_zoned(zone.clone()))).unwrap_or_default())]) } else { String::new() },
        folder: health.watch_folder.clone(),
        offers: health.watch_offers,
        steps: if s.steps_today > 0 { say("watch-steps", &[("steps", grouped(s.steps_today))]) } else { String::new() },
        resting: match (s.resting_hr, s.resting_hr_usual) {
            (Some(rest), Some(usual)) => say("watch-resting-usual", &[("bpm", rest.to_string()), ("usual", usual.to_string())]),
            (Some(rest), None) => say("watch-resting", &[("bpm", rest.to_string())]),
            _ => String::new(),
        },
        slept: if s.slept_minutes > 0 { say("watch-slept", &[("time", hours(s.slept_minutes)), ("from", hm(s.slept_from)), ("to", hm(s.slept_to))]) } else { String::new() },
        battery: s.body_battery.map(|b| say("watch-battery", &[("level", b.to_string())])).unwrap_or_default(),
        week: if s.week_sleep_minutes > 0 || s.week_steps > 0 { say("watch-week", &[("sleep", hours(s.week_sleep_minutes)), ("steps", grouped(s.week_steps))]) } else { String::new() },
        heart_rate: s.heart_rate_curve,
        stress: s.stress_curve,
        battery_curve: s.battery_curve,
        from: now.date().to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second()),
    }
}

/// What the watch says of this morning, for the Tasks page: "short-night", "strain", or "".
pub(crate) fn morning_word() -> String {
    let now = Zoned::now();
    match sioul_core::wearable::morning(&sioul_core::wearable::folder(), now.date(), now.time_zone()) {
        Some(sioul_core::wearable::Morning::ShortNight) => "short-night".into(),
        Some(sioul_core::wearable::Morning::Strain) => "strain".into(),
        None => String::new(),
    }
}

/// When the watch's files were last looked for, and the watches seen then.
static WATCH_LOOKED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static WATCHES_SEEN: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());

/// The watch's files brought in: from your folder and from a watch the
/// desktop shows, every quarter of an hour at most, at once when a watch appears.
fn watch_tick(qt: &QtThread, shared: &Arc<Shared>) {
    let health = load();
    let mounted = sioul_core::wearable::mounted_watches();
    let appeared = WATCHES_SEEN.lock().is_ok_and(|seen| mounted.iter().any(|m| !seen.contains(m)));
    let now = Timestamp::now().as_second();
    if !appeared && now - WATCH_LOOKED.load(std::sync::atomic::Ordering::Relaxed) < 900 {
        return;
    }
    WATCH_LOOKED.store(now, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut seen) = WATCHES_SEEN.lock() {
        *seen = mounted.clone();
    }
    let mut sources = mounted;
    if !health.watch_folder.trim().is_empty() {
        sources.push(sioul_core::config::expand_home(health.watch_folder.trim()));
    }
    let zone = jiff::tz::TimeZone::system();
    let new: usize = sources.iter().map(|s| sioul_core::wearable::import(s, &sioul_core::wearable::folder(), &zone).unwrap_or(0)).sum();
    if new > 0 {
        let mut args = tr().counted(new);
        // A number, as the sentence chooses "One file" or "3 files" by it.
        args.set("count", new);
        tell(qt, shared, tr().text("watch-imported", Some(&args)));
        crate::work::show_work(qt, shared);
    }
}

fn offers_path() -> std::path::PathBuf {
    sioul_core::config::state_dir().join("watch-offers.json")
}

/// At a breakpoint (a task done, a focus session ended): one gentle offer,
/// when the watch has something to say and the limits allow it.
pub(crate) fn watch_offer(qt: &QtThread, focus_minutes: u32) {
    let health = load();
    let folder = sioul_core::wearable::folder();
    if !health.watch_offers || !folder.join("imported.json").exists() {
        return;
    }
    let now = Zoned::now();
    let mut memory: sioul_core::wearable::Memory = std::fs::read_to_string(offers_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let moment = sioul_core::wearable::Moment { now: now.timestamp().as_second(), hour: now.hour() as u8, quiet: crate::backend::quiet_now(), breakpoint: true, focus_minutes };
    let Some(offer) = sioul_core::wearable::offer(&folder, &moment, &memory, now.time_zone(), now.date()) else { return };
    sioul_core::wearable::offered(&mut memory, offer, moment.now, now.date());
    let _ = std::fs::create_dir_all(sioul_core::config::state_dir());
    let _ = std::fs::write(offers_path(), serde_json::to_string(&memory).unwrap_or_default());
    let key = match offer {
        sioul_core::wearable::Offer::Move => "move",
        sioul_core::wearable::Offer::Pause => "pause",
        sioul_core::wearable::Offer::LowReserve => "low-reserve",
        sioul_core::wearable::Offer::Walk => "walk",
    };
    let action: Option<(String, Box<dyn FnOnce() + Send>)> = (offer == sioul_core::wearable::Offer::LowReserve).then(|| {
        let qt = qt.clone();
        let close: Box<dyn FnOnce() + Send> = Box::new(move || {
            let _ = qt.queue(|mut sioul| {
                let _ = sioul.as_mut().done_for_the_day();
            });
        });
        (tr().text("watch-offer-low-reserve-action", None), close)
    });
    let _ = sioul_sync::notify::remind(&tr().text(&format!("watch-offer-{key}"), None), &tr().text(&format!("watch-offer-{key}-text"), None), action);
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

/// The health page, as JSON.
pub(crate) fn page() -> String {
    let health = load();
    let state = record();
    let knowledge = know();
    let now = Zoned::now();
    let morning = now.date().to_zoned(now.time_zone().clone()).unwrap_or_else(|_| now.clone());
    let night = morning.checked_add(Span::new().days(1)).unwrap_or_else(|_| now.clone());
    let hm = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    let mut today: Vec<DoseRow> = health
        .doses(&morning, &night)
        .into_iter()
        .map(|d| {
            let (due, stamp) = (d.at.timestamp().as_second(), now.timestamp().as_second());
            let marked = state.taken.contains_key(&d.key);
            DoseRow {
                taken: state.taken.get(&d.key).map(|t| hm(*t)).unwrap_or_default(),
                past: d.at <= now,
                late: !marked && stamp - due > GRACE_MINUTES * 60,
                // Due, not marked here: whether it was taken elsewhere, said when not known.
                doubt: if marked || due > stamp { String::new() } else { doubt_of(&knowledge, due, stamp) },
                time: d.at.strftime("%H:%M").to_string(),
                key: d.key,
                name: d.name,
                dose: d.dose,
            }
        })
        .collect();
    // Taken today, at a time the schedule no longer has: a dose taken late or
    // early moved the next ones (a medicine counted from its last dose).
    let (from, to) = (morning.timestamp().as_second(), night.timestamp().as_second());
    for (key, at) in &state.taken {
        let Some((id, due)) = key.rsplit_once('@') else { continue };
        let (Some(medicine), Ok(due)) = (health.medicines.iter().find(|m| m.id == id), due.parse::<i64>()) else { continue };
        if due < from || due >= to || today.iter().any(|d| &d.key == key) {
            continue;
        }
        today.push(DoseRow { key: key.clone(), time: hm(due), name: medicine.name.clone(), dose: medicine.dose.clone(), taken: hm(*at), past: true, late: false, doubt: String::new() });
    }
    today.sort_by(|a, b| a.time.cmp(&b.time).then(a.name.cmp(&b.name)));
    let title_of = |id: &Option<String>| id.as_ref().and_then(|id| health.prescriptions.iter().find(|p| &p.id == id)).map(|p| p.title.clone()).unwrap_or_default();
    let errands = health.errands();
    let missed = missed_rows(&health, &state, &knowledge, &now);
    json(&HealthPage {
        today,
        missed,
        shared_note: if health.medicines.is_empty() { String::new() } else { shared_note(&knowledge) },
        reminded_there: REMINDED_THERE.lock().map(|r| r.clone()).unwrap_or_default(),
        medicines: health.medicines.iter().map(|m| MedicineRow { when: words(&m.schedule), prescription_title: title_of(&m.prescription), medicine: m.clone() }).collect(),
        prescriptions: health
            .prescriptions
            .iter()
            .map(|p| PrescriptionRow {
                next: errands
                    .iter()
                    .filter(|e| e.prescription == p.id && e.day >= now.date().checked_sub(Span::new().days(30)).unwrap_or(now.date()))
                    .map(|e| say(if e.kind == ErrandKind::Refill { "health-next-refill" } else { "health-next-renew" }, &[("day", tr().day(e.day))]))
                    .collect(),
                prescription: p.clone(),
            })
            .collect(),
        movement: health.movement.clone(),
        chats: health.chats.clone(),
        errands_list: errands_list(&health).unwrap_or_default(),
        watch: watch_view(&health),
        lists: { let config = crate::backend::load_config(); sioul_core::tasks::lists().into_iter().filter(|c| !c.read_only).map(|c| serde_json::json!({ "id": format!("{}/{}", c.account, c.id), "name": c.label(&config, tr()), "local": c.account == sioul_core::vdir::LOCAL })).collect() },
    })
}

/// Doses due while Sioul ran nowhere, neither marked nor reminded: a question
/// on the past, each saying when whether it was taken is not known here.
fn missed_rows(health: &Health, state: &HealthState, knowledge: &Knowledge, now: &Zoned) -> Vec<DoseRow> {
    state
        .unanswered(health, now, MISSED_HOURS, GRACE_MINUTES)
        .into_iter()
        .map(|d| DoseRow {
            taken: String::new(),
            past: true,
            late: true,
            doubt: doubt_of(knowledge, d.at.timestamp().as_second(), now.timestamp().as_second()),
            time: if d.at.date() == now.date() { d.at.strftime("%H:%M").to_string() } else { format!("{} {}", tr().weekday_short(d.at.date()), d.at.strftime("%H:%M")) },
            key: d.key,
            name: d.name,
            dose: d.dose,
        })
        .collect()
}

/// The same question on the Porch, as JSON: once your other computers were
/// heard from, so that a dose marked there is not asked about here.
pub(crate) fn missed() -> String {
    let heard = !crate::share::on() || crate::share::last_exchange().is_some();
    let health = load();
    if !heard || health.medicines.is_empty() {
        return "[]".into();
    }
    json(&missed_rows(&health, &record(), &know(), &Zoned::now()))
}

/// A medicine as its form gives it.
#[derive(Deserialize)]
struct MedicineEdit {
    name: String,
    #[serde(default)]
    dose: String,
    /// "day", "days", "hours".
    every: String,
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

/// A medicine made (`id` empty) or changed; returns {"id"} or {"error"}.
pub(crate) fn save_medicine(id: &str, edit: &str) -> String {
    let result = serde_json::from_str::<MedicineEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        if edit.name.trim().is_empty() {
            return Err(tr().text("health-no-name", None));
        }
        let now = Zoned::now();
        let day = |text: &str| text.trim().parse::<Date>().ok();
        let schedule = match edit.every.as_str() {
            "days" => Schedule::Days { days: edit.days.max(1), time: edit.time.trim().to_string(), from: day(&edit.from).unwrap_or(now.date()) },
            "hours" => {
                let from = edit.from.trim().parse::<jiff::civil::DateTime>().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()).map_or(now.timestamp().as_second(), |z| z.timestamp().as_second());
                Schedule::Hours { hours: edit.hours.max(1), from }
            }
            _ => Schedule::Day { times: edit.times.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect() },
        };
        let mut health = load();
        let id = if id.is_empty() { health.new_id(&edit.name) } else { id.to_string() };
        let medicine = Medicine {
            id: id.clone(),
            name: edit.name.trim().to_string(),
            dose: edit.dose.trim().to_string(),
            schedule,
            prescription: Some(edit.prescription).filter(|p| !p.is_empty()),
            until: day(&edit.until),
            paused: edit.paused,
        };
        match health.medicines.iter_mut().find(|m| m.id == id) {
            Some(slot) => *slot = medicine,
            None => health.medicines.push(medicine),
        }
        save(&health).map(|()| id)
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
}

/// A prescription made (`id` empty) or changed; returns {"id"} or {"error"}.
pub(crate) fn save_prescription(id: &str, edit: &str) -> String {
    let result = serde_json::from_str::<PrescriptionEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        if edit.title.trim().is_empty() {
            return Err(tr().text("health-no-name", None));
        }
        let mut health = load();
        let id = if id.is_empty() { health.new_id(&edit.title) } else { id.to_string() };
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
        save(&health).map(|()| id)
    });
    answer(result)
}

/// A medicine or a prescription taken out; returns what went wrong, else "".
pub(crate) fn remove(id: &str) -> String {
    let mut health = load();
    health.medicines.retain(|m| m.id != id);
    health.prescriptions.retain(|p| p.id != id);
    for medicine in health.medicines.iter_mut().filter(|m| m.prescription.as_deref() == Some(id)) {
        medicine.prescription = None;
    }
    save(&health).err().unwrap_or_default()
}

/// Fetched at the pharmacy today: the next refill is counted from now.
pub(crate) fn refilled(id: &str) -> String {
    let mut health = load();
    if let Some(p) = health.prescriptions.iter_mut().find(|p| p.id == id) {
        p.last_refill = Some(Zoned::now().date());
    }
    save(&health).err().unwrap_or_default()
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
    if last == 0 || !health.movement.enabled || sioul_core::timelog::running().is_some() {
        MOVED.store(stamp, Ordering::Relaxed);
        return;
    }
    if !here || stamp - last < i64::from(health.movement.minutes.max(10)) * 60 {
        return;
    }
    MOVED.store(stamp, Ordering::Relaxed);
    if let Err(e) = sioul_sync::notify::remind(&tr().text("health-move", None), &tr().text("health-move-body", None), None) {
        tell(qt, shared, e);
    }
}

/// The question on doses due while Sioul was closed: asked once a session.
static ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Doses reminded in this session: never twice, even when the record cannot be written.
static SENT: std::sync::Mutex<std::collections::BTreeSet<String>> = std::sync::Mutex::new(std::collections::BTreeSet::new());
/// "Reminders come on <computer>", when another computer keeps them.
static REMINDED_THERE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Why the doses here may not be all: this computer alone, or what is not
/// known of your other devices now; "" when all is known.
fn shared_note(knowledge: &Knowledge) -> String {
    if !crate::share::on() {
        return tr().text("health-alone", None);
    }
    let now = Timestamp::now().as_second();
    let doubts = sioul_core::health::doubts(now, now, knowledge.record_lost, &knowledge.peers);
    if doubts.is_empty() { String::new() } else { say("dose-doubt-now", &[("why", why(&doubts))]) }
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
    match HealthState::update(&path, now, &change) {
        Ok(()) => String::new(),
        Err(Problem::Unsound(_)) => {
            repair();
            match HealthState::update(&path, now, &change) {
                Ok(()) => String::new(),
                Err(_) => tr().text("dose-record-broken", None),
            }
        }
        Err(Problem::Write(e)) => e,
    }
}

/// What this computer saw of your other devices, for the doses: kept here
/// only (`$XDG_STATE_HOME/sioul/share/peers.json`), never shared.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Peers {
    #[serde(default)]
    peers: std::collections::BTreeMap<String, PeerSeen>,
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
    let Some((_, claims, heard)) = crate::share::others_on_health() else { return Knowledge { record_lost, peers: Vec::new() } };
    let _held = KNOWING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut kept: Peers = std::fs::read_to_string(peers_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    learn(&mut kept, &claims, &heard, now);
    if let Ok(text) = serde_json::to_string(&kept) {
        let _ = std::fs::create_dir_all(sioul_core::config::state_dir().join("share"));
        let _ = std::fs::write(peers_path(), text);
    }
    Knowledge { record_lost, peers: kept.peers.into_values().map(|s| s.peer).collect() }
}

/// What the others' claims say, learned: each one's name, how late its news
/// comes, and until when everything it wrote is read here.
fn learn(kept: &mut Peers, claims: &[sioul_sync::lease::Claim], heard: &sioul_sync::share::Heard, now: i64) {
    for claim in claims {
        let seen = kept.peers.entry(claim.computer.clone()).or_default();
        seen.peer.name = claim.name.clone();
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
        // Everything it wrote until its claim, read here: known until then.
        if let Some(wrote) = claim.wrote
            && heard.complete(&claim.computer, wrote)
            && claim.renewed >= seen.peer.known_until
        {
            seen.peer.known_until = claim.renewed;
            seen.peer.closed = claim.closed;
        }
        seen.peer.broken = heard.broken.get(&claim.computer).copied();
    }
}

/// Why a dose is not known, in words: "your laptop: last heard at 07:52".
fn why(doubts: &[Doubt]) -> String {
    let zone = jiff::tz::TimeZone::system();
    let when = |at: i64| Timestamp::from_second(at).map(|t| tr().when(&t.to_zoned(zone.clone()))).unwrap_or_default();
    let parts: Vec<String> = doubts
        .iter()
        .map(|doubt| match doubt {
            Doubt::Record { since } => say("dose-doubt-record", &[("when", when(*since))]),
            Doubt::Unheard { name, until: 0, .. } => say("dose-doubt-never", &[("name", name.clone())]),
            Doubt::Unheard { name, until, closed: true } => say("dose-doubt-closed", &[("name", name.clone()), ("when", when(*until))]),
            Doubt::Unheard { name, until, closed: false } => say("dose-doubt-open", &[("name", name.clone()), ("when", when(*until))]),
            Doubt::Broken { name } => say("dose-doubt-broken", &[("name", name.clone())]),
        })
        .collect();
    parts.join("; ")
}

/// A dose's doubt, as a row says it; "" when it is known.
fn doubt_of(knowledge: &Knowledge, due: i64, now: i64) -> String {
    let doubts = sioul_core::health::doubts(due, now, knowledge.record_lost, &knowledge.peers);
    if doubts.is_empty() { String::new() } else { say("dose-doubt", &[("why", why(&doubts))]) }
}

/// The doses' state is read, changed and written by the minute's tick, the
/// page and the notifications' buttons: one at a time, so that none writes
/// back an older state over another's mark (a dose shown untaken could be taken twice).
static STATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn state_held() -> std::sync::MutexGuard<'static, ()> {
    STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One tick at a time: a slow one (a watch's files, a shared folder) is not
/// overtaken by the next.
static TICKING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A dose not taken, said afterwards: asked no more. Returns what went wrong, else "".
pub(crate) fn not_taken(key: &str) -> String {
    let now = Timestamp::now().as_second();
    change(|state| {
        state.not_taken.insert(key.to_string(), now);
    })
}

/// When a dose is due, from its key ("<medicine>@<Unix seconds>").
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
    let mut health = load();
    let moved = health.taken_at(key, at);
    if moved.is_some()
        && let Err(e) = save(&health)
    {
        return e;
    }
    let problem = change(|state| {
        state.taken.insert(key.to_string(), at);
        state.not_taken.remove(key);
        if let Some((before, after)) = moved {
            state.moved.insert(key.to_string(), [before, after]);
        }
    });
    // Not marked: the doses it moved go back.
    if !problem.is_empty()
        && let Some((before, after)) = moved
        && health.taken_back(key, before, after)
    {
        let _ = save(&health);
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
        "name": medicine.name,
        "dose": medicine.dose,
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
    let mut health = load();
    let shift = if taken { health.taken_at(key, now) } else { None };
    if shift.is_some()
        && let Err(e) = save(&health)
    {
        return e;
    }
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
        let mut health = load();
        if health.taken_back(key, before, after) {
            let _ = save(&health);
        }
    }
    problem
}

/// The pause to move and the chats' limit: "movement.enabled", "movement.minutes",
/// "chats.enabled", "chats.minutes", "chats.locked_minutes".
pub(crate) fn set_setting(key: &str, value: &str) -> String {
    let mut health = load();
    let number = || value.trim().parse::<u32>().unwrap_or(0);
    match key {
        "movement.enabled" => health.movement.enabled = value == "true",
        "movement.minutes" => health.movement.minutes = number().clamp(10, 240),
        "chats.enabled" => health.chats.enabled = value == "true",
        "chats.minutes" => health.chats.minutes = number(),
        "chats.locked_minutes" => health.chats.locked_minutes = number(),
        "errands_list" => health.errands_list = value.trim().to_string(),
        "watch_folder" => {
            health.watch_folder = crate::backend::local_path(value.trim()).display().to_string();
            WATCH_LOOKED.store(0, std::sync::atomic::Ordering::Relaxed);
        }
        "watch_offers" => health.watch_offers = value == "true",
        _ => return say("setting-unknown-key", &[("key", key.to_string())]),
    }
    save(&health).err().unwrap_or_default()
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

fn name_of(kept: &sioul_core::needs::Kept) -> String {
    if kept.name.trim().is_empty() { usual_name(kept.kind, kept.index) } else { kept.name.clone() }
}

/// Each minute, from the computer you are at: a block's heads-up about the
/// work, `heads_up` minutes before it ("No new big task"), with "Later";
/// then one at its time. Two at most, each once, only near its time; none
/// while an event goes on, none for a block skipped today. Its name and time
/// only: safe to be read by someone else (docs/health.md).
fn needs_tick(qt: &QtThread, shared: &Arc<Shared>, health: &Health, now: &Zoned) {
    let needs = &health.needs;
    if !(needs.meals_on || needs.naps_on || needs.sleep_on) {
        return;
    }
    let path = sioul_core::needs::Today::default_path();
    let mut today = sioul_core::needs::Today::load(&path, now.date());
    let stamp = now.timestamp().as_second();
    let moved = today.shifts.clone();
    let kept = needs.kept_on(now.date(), now.time_zone(), &|key: &str| moved.get(key).copied().unwrap_or(0));
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
    let hm = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    for (block, heads_up) in due {
        let name = name_of(block);
        if heads_up {
            // "Later", once a day: the block a few minutes on, today only.
            let action: Option<(String, Box<dyn FnOnce() + Send>)> = (!today.shifts.contains_key(&block.key)).then(|| {
                let (key, qt_later, shared_later) = (block.key.clone(), qt.clone(), Arc::clone(shared));
                let later: Box<dyn FnOnce() + Send> = Box::new(move || {
                    later_today(&key);
                    crate::work::show_work(&qt_later, &shared_later);
                });
                (tr().text("need-later", None), later)
            });
            if let Err(e) = sioul_sync::notify::remind(&tr().text("need-heads-up", None), &say("need-at", &[("name", name), ("time", hm(block.start))]), action) {
                tell(qt, shared, e);
            }
        } else if let Err(e) = sioul_sync::notify::remind(&name, &hm(block.start), None) {
            tell(qt, shared, e);
        }
    }
}

/// A block moved a few minutes on, today only ("Later"); once.
pub(crate) fn later_today(key: &str) {
    let path = sioul_core::needs::Today::default_path();
    let now = Zoned::now();
    let mut today = sioul_core::needs::Today::load(&path, now.date());
    if today.shifts.contains_key(key) {
        return;
    }
    today.shifts.insert(key.to_string(), i64::from(load().needs.later));
    let _ = today.save(&path);
}

/// A block skipped today, or not: no notice, kept free all the same.
pub(crate) fn skip_today(key: &str, skip: bool) -> String {
    let path = sioul_core::needs::Today::default_path();
    let mut today = sioul_core::needs::Today::load(&path, Zoned::now().date());
    if skip {
        today.skipped.insert(key.to_string());
    } else {
        today.skipped.remove(key);
    }
    today.save(&path).err().unwrap_or_default()
}

/// Meals, naps and the night as the page sets them, as JSON: the settings,
/// the usual names, the long gaps between meals, today's moves and skips.
pub(crate) fn needs_page() -> String {
    let needs = load().needs;
    let today = sioul_core::needs::Today::load(&sioul_core::needs::Today::default_path(), Zoned::now().date());
    serde_json::json!({
        "needs": needs,
        "usual": {
            "meals": (0..needs.meals.len().max(3) + 1).map(|i| usual_name("meal", i)).collect::<Vec<_>>(),
            "nap": usual_name("nap", 0),
            "sleep": usual_name("sleep", 0),
        },
        "gaps": needs.long_gaps().into_iter().map(|(from, to)| say("need-gap", &[("from", from), ("to", to)])).collect::<Vec<_>>(),
        "skipped": today.skipped,
        "moved": today.shifts,
    })
    .to_string()
}

/// Meals, naps and the night saved as the page gives them; returns what went wrong, else "".
pub(crate) fn save_needs(edit: &str) -> String {
    let needs: sioul_core::needs::Needs = match serde_json::from_str(edit) {
        Ok(needs) => needs,
        Err(e) => return e.to_string(),
    };
    let mut health = load();
    health.needs = needs;
    save(&health).err().unwrap_or_default()
}

/// Each minute: a dose due is reminded once, quietly, with "Taken"; a refill
/// or a renewal coming becomes a task in a list kept on this computer.
pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    let Some(_ticking) = crate::backend::one_at_a_time(&TICKING) else { return };
    watch_tick(qt, shared);
    let health = load();
    if health.medicines.is_empty() && health.prescriptions.is_empty() {
        // No medicines: the pause to move, meals and rest, from the computer you are at.
        let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
        let (keeper, _) = crate::share::keeper("health", sioul_sync::lease::Rule::FollowsYou, active, false);
        movement_tick(qt, shared, &health, &Zoned::now(), keeper.mine);
        if keeper.mine && keeper.settled {
            needs_tick(qt, shared, &health, &Zoned::now());
        }
        return;
    }
    // One computer reminds: the one you are at, once it has been so long
    // enough for the others to know (`sioul_sync::lease`). A dose reminded
    // twice could be taken twice.
    let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
    let (keeper, _) = crate::share::keeper("health", sioul_sync::lease::Rule::FollowsYou, active, false);
    let _held = state_held();
    let state = record();
    let knowledge = know();
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    if let Ok(mut there) = REMINDED_THERE.lock() {
        *there = if keeper.mine { String::new() } else { say("health-reminded-there", &[("computer", keeper.name.clone())]) };
    }
    movement_tick(qt, shared, &health, &now, keeper.mine);
    if keeper.mine && keeper.settled {
        needs_tick(qt, shared, &health, &now);
    }
    let mut reminded: Vec<String> = Vec::new();
    if keeper.mine && keeper.settled {
        for dose in state.to_remind(&health, &now, GRACE_MINUTES) {
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
            reminded.push(dose.key.clone());
            if let Ok(mut sent) = SENT.lock() {
                sent.insert(dose.key.clone());
            }
            let named = if dose.dose.is_empty() { dose.name.clone() } else { format!("{} · {}", dose.name, dose.dose) };
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
            if let Err(e) = sioul_sync::notify::remind(&title, &body, Some((tr().text("health-taken", None), taken))) {
                tell(qt, shared, e);
            }
        }
    }
    // Doses due while Sioul ran nowhere: asked about once, as a question on
    // the past, after your other computers were heard from (a dose marked
    // there comes first), and only where you are; what is not known, said.
    let heard = !crate::share::on() || crate::share::last_exchange().is_some();
    if heard && keeper.mine && !ASKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
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
            if let Err(e) = sioul_sync::notify::remind(&tr().text("health-missed", None), &body, Some((tr().text("health-missed-open", None), open))) {
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
    let problem = change(|record| {
        for key in &reminded {
            record.reminded.insert(key.clone(), stamp);
        }
        for (key, uid) in &made {
            record.errands.insert(key.clone(), uid.clone());
        }
    });
    if !problem.is_empty() && (!reminded.is_empty() || !made.is_empty()) {
        tell(qt, shared, problem);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_sync::lease::Claim;

    fn claim(renewed: i64, wrote: Option<(u32, u64)>, closed: bool) -> Claim {
        Claim { computer: "laptop-id".into(), name: "laptop".into(), since: 0, renewed, until: renewed + 300, active: renewed, taken: 0, wrote, closed }
    }

    fn heard_up_to(n: u64) -> sioul_sync::share::Heard {
        sioul_sync::share::Heard { read: [("laptop-id".to_string(), (1, n))].into(), broken: Default::default() }
    }

    #[test]
    fn what_the_others_claims_teach() {
        let mut kept = Peers::default();
        // Seen for the first time, an old claim: known until then, but no delay measured from it.
        learn(&mut kept, &[claim(1_000, Some((1, 5)), false)], &heard_up_to(5), 9_000);
        let peer = &kept.peers["laptop-id"].peer;
        assert_eq!((peer.known_until, peer.delay, peer.name.as_str()), (1_000, None, "laptop"));
        // Watching, each new claim times how late it came.
        learn(&mut kept, &[claim(9_030, Some((1, 6)), false)], &heard_up_to(6), 9_060);
        learn(&mut kept, &[claim(9_090, Some((1, 6)), false)], &heard_up_to(6), 9_120);
        assert_eq!(kept.peers["laptop-id"].peer.delay, Some(30));
        // A record it wrote not read here yet: known only until the claim before.
        learn(&mut kept, &[claim(9_150, Some((1, 9)), false)], &heard_up_to(6), 9_180);
        assert_eq!(kept.peers["laptop-id"].peer.known_until, 9_090);
        // Read, and it says it closed: known until then, closed.
        learn(&mut kept, &[claim(9_200, Some((1, 9)), true)], &heard_up_to(9), 9_240);
        let peer = &kept.peers["laptop-id"].peer;
        assert!(peer.closed && peer.known_until == 9_200);
        // Its news comes within a minute: a dose due after it closed is known.
        assert!(sioul_core::health::doubts(9_600, 9_700, None, std::slice::from_ref(peer)).is_empty());
        // An older Sioul that never says how far it wrote: heard, never known.
        let mut old = Peers::default();
        learn(&mut old, &[claim(9_000, None, false)], &heard_up_to(99), 9_030);
        assert_eq!(old.peers["laptop-id"].peer.known_until, 0);
        assert_eq!(sioul_core::health::doubts(9_010, 9_030, None, &[old.peers["laptop-id"].peer.clone()]).len(), 1);
    }
}
