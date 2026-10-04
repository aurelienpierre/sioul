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
use sioul_core::health::{ChatLimit, ErrandKind, Health, HealthState, Medicine, Movement, Prescription, Schedule};
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
            let from = Timestamp::from_second(*from).map(|t| tr().date(&t.to_zoned(jiff::tz::TimeZone::system()), false)).unwrap_or_default();
            let mut args = sioul_core::i18n::args();
            args.set("hours", *hours);
            args.set("from", from);
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
    let state = HealthState::load(&HealthState::default_path());
    let now = Zoned::now();
    let morning = now.date().to_zoned(now.time_zone().clone()).unwrap_or_else(|_| now.clone());
    let night = morning.checked_add(Span::new().days(1)).unwrap_or_else(|_| now.clone());
    let today = health
        .doses(&morning, &night)
        .into_iter()
        .map(|d| DoseRow {
            taken: state.taken.get(&d.key).and_then(|t| Timestamp::from_second(*t).ok()).map(|t| t.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default(),
            past: d.at <= now,
            time: d.at.strftime("%H:%M").to_string(),
            key: d.key,
            name: d.name,
            dose: d.dose,
        })
        .collect();
    let title_of = |id: &Option<String>| id.as_ref().and_then(|id| health.prescriptions.iter().find(|p| &p.id == id)).map(|p| p.title.clone()).unwrap_or_default();
    let errands = health.errands();
    let missed = state
        .unanswered(&health, &now, MISSED_HOURS, GRACE_MINUTES)
        .into_iter()
        .map(|d| DoseRow {
            taken: String::new(),
            past: true,
            time: if d.at.date() == now.date() { d.at.strftime("%H:%M").to_string() } else { format!("{} {}", tr().weekday_short(d.at.date()), d.at.strftime("%H:%M")) },
            key: d.key,
            name: d.name,
            dose: d.dose,
        })
        .collect();
    json(&HealthPage {
        today,
        missed,
        shared_note: if health.medicines.is_empty() { String::new() } else { shared_note() },
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
/// "Reminders come on <computer>", when another computer keeps them.
static REMINDED_THERE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Why the doses here may not be all: this computer alone, or the others not
/// heard from lately (a dose marked there not come yet); "" when all is known.
fn shared_note() -> String {
    if !crate::share::on() {
        return tr().text("health-alone", None);
    }
    match crate::share::last_exchange() {
        Some((at, true)) if jiff::Timestamp::now().as_second() - at < 5 * 60 => String::new(),
        Some((at, _)) => {
            let at = Timestamp::from_second(at).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M").to_string()).unwrap_or_default();
            say("health-unchecked", &[("time", at)])
        }
        None => tr().text("health-unchecked-yet", None),
    }
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

/// A dose answered "not taken" when asked afterwards: not asked again.
pub(crate) fn set_not_taken(key: &str) {
    let _held = state_held();
    let path = HealthState::default_path();
    let mut state = HealthState::load(&path);
    let now = Zoned::now().timestamp().as_second();
    state.not_taken.insert(key.to_string(), now);
    let _ = state.save(&path, now);
}

/// A dose marked taken, or not.
pub(crate) fn set_taken(key: &str, taken: bool) {
    let _held = state_held();
    let path = HealthState::default_path();
    let mut state = HealthState::load(&path);
    let now = Zoned::now().timestamp().as_second();
    if taken {
        state.taken.insert(key.to_string(), now);
    } else {
        state.taken.remove(key);
    }
    let _ = state.save(&path, now);
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
    let path = HealthState::default_path();
    let mut state = HealthState::load(&path);
    let now = Zoned::now();
    let covered = state.chat_minute(&limit, &now);
    let _ = state.save(&path, now.timestamp().as_second());
    covered
}

/// Whether chats are covered now.
pub(crate) fn chats_covered() -> bool {
    HealthState::load(&HealthState::default_path()).chats_covered(&Zoned::now())
}

/// Each minute: a dose due is reminded once, quietly, with "Taken"; a refill
/// or a renewal coming becomes a task in a list kept on this computer.
pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    let Some(_ticking) = crate::backend::one_at_a_time(&TICKING) else { return };
    watch_tick(qt, shared);
    let health = load();
    if health.medicines.is_empty() && health.prescriptions.is_empty() {
        // No medicines: only the pause to move, from the computer you are at.
        let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
        let (keeper, _) = crate::share::keeper("health", sioul_sync::lease::Rule::FollowsYou, active, false);
        movement_tick(qt, shared, &health, &Zoned::now(), keeper.mine);
        return;
    }
    // One computer reminds: the one you are at, once it has been so long
    // enough for the others to know (`sioul_sync::lease`). A dose reminded
    // twice could be taken twice.
    let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
    let (keeper, _) = crate::share::keeper("health", sioul_sync::lease::Rule::FollowsYou, active, false);
    let _held = state_held();
    let path = HealthState::default_path();
    let mut state = HealthState::load(&path);
    let now = Zoned::now();
    if let Ok(mut there) = REMINDED_THERE.lock() {
        *there = if keeper.mine { String::new() } else { say("health-reminded-there", &[("computer", keeper.name.clone())]) };
    }
    movement_tick(qt, shared, &health, &now, keeper.mine);
    if keeper.mine && keeper.settled {
        for dose in state.to_remind(&health, &now, GRACE_MINUTES) {
            state.reminded.insert(dose.key.clone(), now.timestamp().as_second());
            let title = if dose.dose.is_empty() { dose.name.clone() } else { format!("{} · {}", dose.name, dose.dose) };
            let key = dose.key.clone();
            let (qt_taken, shared_taken) = (qt.clone(), Arc::clone(shared));
            let taken: Box<dyn FnOnce() + Send> = Box::new(move || {
                set_taken(&key, true);
                // Your other computers know at once.
                crate::share::exchange(&qt_taken, &shared_taken);
            });
            if let Err(e) = sioul_sync::notify::remind(&title, &dose.at.strftime("%H:%M").to_string(), Some((tr().text("health-taken", None), taken))) {
                tell(qt, shared, e);
            }
        }
    }
    // Doses due while Sioul ran nowhere: asked about once, as a question on
    // the past, after your other computers were heard from (a dose marked
    // there comes first), and only where you are.
    let heard = !crate::share::on() || crate::share::last_exchange().is_some();
    if heard && keeper.mine && !ASKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let missed = state.unanswered(&health, &now, MISSED_HOURS, GRACE_MINUTES);
        if !missed.is_empty() {
            let names: Vec<String> = missed.iter().map(|d| format!("{} {}", d.name, d.at.strftime("%H:%M"))).collect();
            let qt_open = qt.clone();
            let open: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = qt_open.queue(|mut sioul| sioul.as_mut().reminder_opened(QString::from("health"), QString::default(), QString::default()));
            });
            let body = say("health-missed-body", &[("doses", names.join(", "))]);
            if let Err(e) = sioul_sync::notify::remind(&tr().text("health-missed", None), &body, Some((tr().text("health-missed-open", None), open))) {
                tell(qt, shared, e);
            }
        }
    }
    // Errands: a task each, the day it comes into view, made once, in the list
    // the phone has; those made on this computer before move there, once.
    let target = errands_list(&health);
    for errand in health.errands().into_iter().filter(|e| e.day <= now.date().checked_add(Span::new().days(7)).unwrap_or(now.date())) {
        if state.errands.contains_key(&errand.key) {
            continue;
        }
        let title = say(if errand.kind == ErrandKind::Refill { "health-errand-refill" } else { "health-errand-renew" }, &[("title", errand.title.clone())]);
        let edit = TaskEdit { title, start: errand.day.to_string(), categories: vec![tr().text("health-category", None)], estimate: 30, ..TaskEdit::default() };
        let made = match &target {
            Some(list) => work::create_task(qt, shared, &edit, list),
            None => work::local_task(qt, shared, &tr().text("health-list", None), &edit),
        };
        if let Ok(uid) = made {
            state.errands.insert(errand.key, uid);
        }
    }
    if let Some(list) = target.as_ref().filter(|l| !l.starts_with(&format!("{}/", sioul_core::vdir::LOCAL))) {
        let loaded = work::loaded(shared);
        for uid in state.errands.values() {
            let here = loaded.tasks.iter().find(|t| &t.uid == uid).is_some_and(|t| t.status.is_open() && t.list_id.starts_with(&format!("{}/", sioul_core::vdir::LOCAL)));
            if here {
                let _ = work::move_task(qt, shared, uid, list, true);
            }
        }
    }
    let _ = state.save(&path, now.timestamp().as_second());
}
