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
use sioul_core::dose_reminders::{DoseReminders, Elsewhere, Others, Tried};
use sioul_core::doses::{Answer, Answered, DoseRecords, Given, State};
use sioul_core::health::{ChatLimit, Doubt, ErrandKind, Health, HealthState, Medicine, Movement, Named, Peer, Prescription, Problem, Schedule, Take, TakeProblem};
use sioul_core::i18n::Translator;
use sioul_core::needs::{DayEdit, DayProblem, Days, Kept, Needs};
use sioul_core::tasks::TaskEdit;
use std::sync::Arc;

/// The health file, as every reader of the doses takes it (`load_said`).
fn load() -> Health {
    load_said().0
}

/// The meals, naps and night as the doses read them (`load`): the last read
/// when the file does not read here.
pub(crate) fn needs() -> sioul_core::needs::Needs {
    load().needs
}

/// The health file, and whether it is there but could not be read. A file
/// that reads is kept beside it as last read (`health.toml.last-read`,
/// written when it changed); one that does not read gives that copy
/// instead, so that no dose, reminder or alarm reads it as no medicines (the
/// rule: a record is never read as empty; review of 36002ea, can-wait 3).
/// Wherever the doses show, they are then said as last known, to check
/// before taking. The file itself is copied aside as it is (`Health::load`)
/// and never written over (`Health::change`).
fn load_said() -> (Health, bool) {
    load_known().unwrap_or_else(|| (Health::default(), true))
}

/// `load_said`, or nothing when nothing reads here: the file does not, nor
/// any last read kept beside it. A phone's alarms are then left as Android
/// has them (`alarms_coming`), and one that rings is said in doubt
/// (`alarm_decide`): never silenced by a file that does not read.
fn load_known() -> Option<(Health, bool)> {
    let path = Health::default_path();
    match Health::read_bytes(&path) {
        // Read as no medicines while the sharing holds that emptying (a file
        // emptied by hand or by a bug, no removal of Sioul's): never trusted
        // as none here either, its last read given instead, said as last
        // known, so that the alarms keep ringing until the word that ends
        // the hold (the A4 check, #12: they were handed an empty list); but
        // the medicines taken out in Sioul since (marked: "Take them out
        // everywhere" marks them first), at once. No last read: nothing
        // reads, and Android keeps the alarms it has.
        Ok((health, _)) if health.medicines.is_empty() && taken_out_in_sioul(&health, &path) != Some(true) && emptying_held(&path) => {
            let mut copy = last_read(&path)?;
            copy.medicines.retain(|m| !health.removed.contains_key(&m.id));
            if let Ok(state) = HealthState::read(&HealthState::default_path()) {
                copy.mend_shifts(&state, &backs());
            }
            Some((copy, true))
        }
        Ok((mut health, bytes)) => {
            // Never kept as the last read when it lost every medicine, one of
            // them with no removal of Sioul's (emptied by hand or by a bug):
            // the hold above gives that copy, which would then hold none
            // (the A4 check, #12).
            if !health.medicines.is_empty() || taken_out_in_sioul(&health, &path) != Some(false) {
                keep_last_read(&path, &bytes);
            }
            // Mended from the doses' record in memory, as the copy below is: a
            // move or a take back recorded elsewhere (its file not read there)
            // rings and marks here at once, in the window's minute or not (a
            // phone's background step, its alarm's decision), and the tick
            // writes it (the check of 1b6795e, H4).
            if let Ok(state) = HealthState::read(&HealthState::default_path()) {
                health.mend_shifts(&state, &backs());
            }
            Some((health, false))
        }
        Err(_) => {
            let _ = Health::load(&path);
            let mut copy = last_read(&path)?;
            // Mended from the doses' record in memory, nothing written: the
            // moves your devices made since it was read, and those taken back,
            // so that it rings and marks the times they hold (review of
            // 36002ea's check, must-fix 2). One structural rule: the record
            // holds every move; each device, its file read or not, follows it.
            if let Ok(state) = HealthState::read(&HealthState::default_path()) {
                copy.mend_shifts(&state, &backs());
            }
            Some((copy, true))
        }
    }
}

/// Whether this device's health file, read as no medicines though Sioul took
/// one of them out with no mark, is held from being trusted as none: the
/// sharing holds that emptying (`share::medicines_held_here`), or this
/// device is a phone, where nobody edits the file by hand, and its last read
/// held two or more medicines, the sharing's own threshold, at which it holds
/// an emptying and offers a word (the A5 check, L4: until the exchange that
/// began the hold, a phone was handed an empty list; the A6 check, M2: one
/// medicine removed elsewhere with no mark rang for good, nothing to end it).
/// "Take them out here too" takes them out in Sioul, marked
/// (`take_out_in_sioul`), and Remove marks one the last read alone holds:
/// trusted then.
fn emptying_held(path: &std::path::Path) -> bool {
    held_by_sharing() || (is_phone() && last_read(path).is_some_and(|copy| copy.medicines.len() >= 2))
}

/// Whether the sharing holds this device's health file's emptying.
fn held_by_sharing() -> bool {
    #[cfg(test)]
    if let Some(held) = HELD.with(std::cell::Cell::get) {
        return held;
    }
    crate::share::medicines_held_here()
}

/// Whether this device is a phone (Android).
fn is_phone() -> bool {
    #[cfg(test)]
    if let Some(phone) = PHONE.with(std::cell::Cell::get) {
        return phone;
    }
    cfg!(target_os = "android")
}

#[cfg(test)]
thread_local! {
    /// The sharing's hold of this device's medicines, in a test's thread that
    /// says one: never the sharing's memory, which other tests read beside it.
    static HELD: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    /// Whether this device is a phone, in a test's thread that says so.
    static PHONE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Of a read that holds no medicines, whether Sioul took out every medicine
/// of the last read (each marked removed in it, `Health::removed`); false:
/// one went with no removal of Sioul's (emptied by hand or by a bug); none:
/// no last read to tell.
fn taken_out_in_sioul(health: &Health, path: &std::path::Path) -> Option<bool> {
    last_read(path).map(|copy| copy.medicines.iter().all(|m| health.removed.contains_key(&m.id)))
}

/// The moves to put back (`DoseBacks`, a file of its own); one that does not
/// read puts nothing back: the next doses stay where the moves left them,
/// later, never sooner.
fn backs() -> sioul_core::dose_backs::DoseBacks {
    sioul_core::dose_backs::DoseBacks::read(&sioul_core::dose_backs::DoseBacks::default_path()).unwrap_or_default()
}

/// A move of the dose `key` to put back on every device (`DoseBacks`),
/// decided `now`: taken back, or replaced by a time corrected. Not written
/// (the file does not read, a full disk): put back here only.
fn restore_everywhere(key: &str, before: i64, after: i64, now: i64) {
    let _ = sioul_core::dose_backs::DoseBacks::update(&sioul_core::dose_backs::DoseBacks::default_path(), now, |b| b.restore(key, before, after, now));
}

/// The dose `key` taken anew: what was to be put back of it is passed.
fn restores_passed(key: &str, now: i64) {
    let _ = sioul_core::dose_backs::DoseBacks::update(&sioul_core::dose_backs::DoseBacks::default_path(), now, |b| b.passed(key));
}

/// Where the health file's last good read is kept, beside it.
fn last_read_path(path: &std::path::Path) -> std::path::PathBuf {
    Health::last_read_path(path)
}

/// The health file's bytes kept beside it as last read, when they changed
/// since: the very bytes that read (`Health::read_bytes`), never a second
/// look at the file; written as the file is, synced; never of no bytes (no
/// file yet: nothing to keep) (review of 36002ea's check, A2).
fn keep_last_read(path: &std::path::Path, bytes: &[u8]) {
    static KEPT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return;
    }
    let hash = sioul_core::vdir::content_hash(bytes);
    let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let copy = last_read_path(path);
    if kept.as_deref() == Some(hash.as_str()) && copy.exists() {
        return;
    }
    if std::fs::read(&copy).ok().as_deref() != Some(bytes) && sioul_core::health::write_synced(&copy, bytes).is_err() {
        return;
    }
    *kept = Some(hash);
}

/// The health file as last read here (`keep_last_read`); none kept yet, and
/// never a copy of no bytes read as no medicines.
fn last_read(path: &std::path::Path) -> Option<Health> {
    let text = std::fs::read_to_string(last_read_path(path)).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    toml::from_str(&text).ok()
}

/// A reminder's words when the health file does not read here: its doses
/// are as last known, the check title and the doubt said, whatever else
/// the reminder says (review of 36002ea, can-wait 3). Unchanged otherwise.
fn as_last_known(dose: &sioul_core::health::Dose, title: String, body: String, last_known: bool) -> (String, String) {
    if !last_known {
        return (title, body);
    }
    (say("dose-check-title", &[("dose", named(dose))]), format!("{body} {}", tr().text("dose-last-known", None)))
}

/// The health file read, changed by `change` and written back, all under its
/// lock, which the sharing takes too: a change is set over the medicines as
/// the file holds them then (`Health::change`), never over a copy read earlier.
fn change_health<R>(change: impl FnOnce(&mut Health) -> Result<R, String>) -> Result<R, String> {
    let path = Health::default_path();
    let mut before: Option<Health> = None;
    // A file that does not read is never written over (`Health::change`): said in your words.
    let out = Health::change(&path, |health| {
        before = Some(health.clone());
        change(health)
    })
    .map_err(|e| if e == Health::UNREADABLE { tr().text("health-not-written", None) } else { e })?;
    // A set time moved: today's marks carried to the new times (the A4 check, #6).
    if let (Some(before), Ok(after)) = (before, Health::read(&path)) {
        carry_marks(&before, &after, &Zoned::now());
    }
    // A medicine changed: a phone's alarms follow (`alarms`).
    crate::alarms::schedule();
    Ok(out)
}

/// A medicine at set times whose times changed today, after a dose of
/// today's was taken: that mark and its reminder carried to the new time
/// that takes its place, the old times and the new ones paired in their
/// order, so that a dose taken never rings again at its new time (20:00
/// taken, its time moved to 21:00: 21:00 is taken; the A4 check, #6). A dose
/// said not taken, or not marked, carries nothing: its new time rings, and
/// nothing says "not taken" of a time not come yet (the A5 check, M2).
fn carry_marks(before: &Health, after: &Health, now: &Zoned) {
    let midnight = now.date().to_zoned(now.time_zone().clone()).unwrap_or_else(|_| now.clone());
    let end = midnight.checked_add(Span::new().days(1)).unwrap_or_else(|_| now.clone());
    let today = |health: &Health, id: &str| -> Vec<String> { health.doses(&midnight, &end).into_iter().filter(|d| d.key.rsplit_once('@').is_some_and(|(m, _)| m == id)).map(|d| d.key).collect() };
    let mut pairs: Vec<(String, String)> = Vec::new();
    for medicine in after.medicines.iter().filter(|m| matches!(m.schedule, Schedule::Day { .. })) {
        let Some(was) = before.medicines.iter().find(|m| m.id == medicine.id) else { continue };
        if was.schedule == medicine.schedule {
            continue;
        }
        let (old, new) = (today(before, &medicine.id), today(after, &medicine.id));
        let gone: Vec<&String> = old.iter().filter(|k| !new.contains(k)).collect();
        let came: Vec<&String> = new.iter().filter(|k| !old.contains(k)).collect();
        pairs.extend(gone.into_iter().zip(came).map(|(from, to)| (from.clone(), to.clone())));
    }
    if pairs.is_empty() {
        return;
    }
    let _ = change(|record| {
        for (from, to) in &pairs {
            let Some(at) = record.taken.get(from).copied() else { continue };
            if record.taken.contains_key(to) || record.not_taken.contains_key(to) {
                continue;
            }
            record.taken.insert(to.clone(), at);
            if let Some(at) = record.reminded.get(from).copied() {
                record.reminded.entry(to.clone()).or_insert(at);
            }
        }
    });
}

/// Medicines marked as removed in Sioul (`Health::mark_removed`, under the
/// health file's lock), so that their removals travel with that mark and no
/// device holds them as gone by hand: "Take them out everywhere" for those a
/// hold kept from going (`share::take_out_medicines`; the A4 check, #10). A
/// file that does not read is left as it is.
pub(crate) fn mark_removed(ids: &[String]) {
    let now = Timestamp::now().as_second();
    let _ = change_health(|health| {
        health.mark_removed(ids, now);
        Ok(())
    });
}

/// Medicines taken out of this device's health file in Sioul, marked as
/// removed (`Health::mark_removed`): "Take them out here too" for those
/// another device's records took out with no mark (`share::take_out_medicines`),
/// so that the file, read as none, is trusted as none and its last read
/// follows, never given in its place (`emptying_held`; the A5 check, L4).
pub(crate) fn take_out_in_sioul(ids: &[String]) {
    let now = Timestamp::now().as_second();
    let _ = change_health(|health| {
        health.medicines.retain(|m| !ids.contains(&m.id));
        health.mark_removed(ids, now);
        Ok(())
    });
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
    /// The health file could not be read here: said, its doses not shown; "" when it reads.
    unreadable: String,
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

/// The health page, as JSON: the week of the day it shows, and what only
/// today says; the events whose deletion waits (`waiting`) left out.
pub(crate) fn page(waiting: &Waiting) -> String {
    page_of(&Reads::now(), waiting)
}

/// What the Health job reads once a run (the unified agenda, §8.1): the
/// health file (or its last read), this device's record of doses, the
/// records, what your other devices know, and what each device did of a
/// reminder. The page, the Porch's doses and today's rows are all made from
/// this one read: what your devices know is taken once (`know`, `KNOWING`).
pub(crate) struct Reads {
    known: Option<(Health, bool)>,
    state: HealthState,
    records: DoseRecords,
    knowledge: Knowledge,
    told: DoseReminders,
}

impl Reads {
    pub(crate) fn now() -> Reads {
        Reads { known: load_known(), state: record(), records: records(), knowledge: know(), told: told().unwrap_or_default() }
    }
}

/// The page, from what was read (`Reads`).
fn page_of(reads: &Reads, waiting: &Waiting) -> String {
    // As last known only when its last read is what is shown (review of 36002ea's check, low 17).
    let copy = reads.known.as_ref().is_some_and(|(_, last_known)| *last_known);
    let (health, unreadable) = reads.known.clone().unwrap_or_else(|| (Health::default(), true));
    let (state, knowledge, records) = (&reads.state, &reads.knowledge, &reads.records);
    let now = Zoned::now();
    let shown = SHOWN.lock().ok().and_then(|s| *s).unwrap_or(now.date());
    let week = week_view(&health, state, records, knowledge, shown, &now, waiting);
    let needs = &health.needs;
    json(&PageView {
        any: unreadable || needs.meals_on || needs.naps_on || needs.sleep_on || !health.medicines.is_empty() || week.days.iter().any(|d| !d.items.is_empty()),
        // Its medicines as last read, when a copy was kept; else none shown.
        unreadable: if unreadable { tr().text(if copy { "health-last-known" } else { "health-unreadable" }, None) } else { String::new() },
        week,
        missed: with_lately(missed_rows(&health, state, &reads.told, knowledge, &now, tr()), &health, state, &now),
        shared_note: if health.medicines.is_empty() { String::new() } else { shared_note(knowledge) },
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

/// Events whose deletion waits ("Undo": `mail::hidden_pim`), and occurrences left out.
pub(crate) type Waiting = (std::collections::BTreeSet<std::path::PathBuf>, std::collections::BTreeSet<(std::path::PathBuf, i64)>);

/// The events of the week from `monday`, read again five minutes after at most
/// (the page is made again each minute while it is open), those whose deletion
/// waits left out as every agenda leaves them out (`pim::without_waiting`), at once.
fn week_events(monday: Date, zone: &TimeZone, waiting: &Waiting) -> Vec<sioul_core::agenda::Occurrence> {
    crate::pim::without_waiting(week_read(monday, zone).as_ref().clone(), &waiting.0, &waiting.1)
}

/// The calendars' events of the week from `monday`, as read, kept five
/// minutes, never past a delayed work on the calendars' files ending (a
/// deletion written, or undone: `mail::pim_ended`), so that an event deleted
/// never comes back for those minutes (the check of 36002ea, low 23).
fn week_read(monday: Date, zone: &TimeZone) -> Arc<Vec<sioul_core::agenda::Occurrence>> {
    type Cached = Option<(i64, Date, u64, sioul_core::timeline::Stamp, Arc<Vec<sioul_core::agenda::Occurrence>>)>;
    static CACHE: std::sync::Mutex<Cached> = std::sync::Mutex::new(None);
    let stamp = Timestamp::now().as_second();
    let ended = crate::mail::pim_ended();
    // Never past a write either, by any program (`timeline::Stamp`): an event
    // moved or deleted moves the week's meals at once (review of 1880bce, later 4).
    let written = sioul_core::timeline::Stamp::now();
    let mut cache = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((at, day, seen, read, events)) = cache.as_ref()
        && *day == monday
        && *seen == ended
        && *read == written
        && (0..300).contains(&(stamp - at))
    {
        return Arc::clone(events);
    }
    let from = midnight(monday, zone);
    let to = monday.checked_add(Span::new().days(7)).map_or(from + 7 * 86_400, |d| midnight(d, zone));
    #[cfg(test)]
    WEEK_READS.with(|n| n.set(n.get() + 1));
    let events = Arc::new(sioul_core::agenda::occurrences(from, to));
    *cache = Some((stamp, monday, ended, written, Arc::clone(&events)));
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
fn week_view(health: &Health, state: &HealthState, records: &DoseRecords, knowledge: &Knowledge, day: Date, now: &Zoned, waiting: &Waiting) -> WeekView {
    let zone = now.time_zone().clone();
    let monday = day.checked_sub(Span::new().days(i64::from(day.weekday().to_monday_zero_offset()))).unwrap_or(day);
    let days = days();
    let events = week_events(monday, &zone, waiting);
    // Meals move past the events with their margins, as the plan has them
    // (`work::held_from`'s rule): within the plan's reach from its one read
    // while nothing was written since (`work::held_by_plan`); else from the
    // week's one read (`week_read`), never a read a day (review of 1880bce, M5).
    let held = |date: Date| crate::work::held_by_plan(date, now).unwrap_or_else(|| crate::work::held_from(date, now, events.clone()));
    let steps = crate::work::day_steps();
    let days: Vec<DayView> = (0..7).filter_map(|n| monday.checked_add(Span::new().days(n)).ok()).map(|date| day_view(health, state, records, knowledge, &days, &Around { events: &events, held: &held(date), steps: &steps }, date, now)).collect();
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
/// when whether it was taken is not known here. Three questions, each its
/// own heading: those due while Sioul ran nowhere, then those due while it
/// ran but whose reminder could not be shown (no notification server: never
/// said "while Sioul was closed"), then those whose reminder went and was not
/// known shown (`told`: what each device did); by their time within each.
fn missed_rows(health: &Health, state: &HealthState, told: &DoseReminders, knowledge: &Knowledge, now: &Zoned, words: &Translator) -> Vec<DoseRow> {
    let maybe = |key: &str| told.maybe_anywhere(key).is_some();
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
                question: if state.reminder_unshown(&d.key) && maybe(&d.key) {
                    words.text("health-maybe-question", None)
                } else if state.reminder_unshown(&d.key) {
                    words.text("health-unshown-question", None)
                } else {
                    words.text("health-missed-question", None)
                },
                time: if d.at.date() == now.date() { d.at.strftime("%H:%M").to_string() } else { format!("{} {}", words.weekday_short(d.at.date()), d.at.strftime("%H:%M")) },
                key: d.key,
                name: d.name,
                dose: d.dose,
            }
        })
        .collect();
    rows.sort_by_key(|row| (state.reminder_unshown(&row.key), maybe(&row.key)));
    rows
}

/// The notification's title for the question on doses neither marked nor
/// reminded (`missed`): "While Sioul was closed" only when it was; "A
/// reminder could not be shown" when Sioul ran; both kinds, a title true of
/// both.
fn missed_title(missed: &[sioul_core::health::Dose], state: &HealthState, told: &DoseReminders, words: &Translator) -> String {
    let maybe = missed.iter().filter(|d| state.reminder_unshown(&d.key) && told.maybe_anywhere(&d.key).is_some()).count();
    let unshown = missed.iter().filter(|d| state.reminder_unshown(&d.key)).count() - maybe;
    match (unshown, maybe) {
        (0, 0) => words.text("health-missed", None),
        (n, 0) if n == missed.len() => words.text("health-unshown", None),
        (0, n) if n == missed.len() => words.text("health-maybe", None),
        _ => words.text("health-missed-some", None),
    }
}

/// What a minute of `tick` did, written in the doses' record: the reminders
/// shown, those that surely could not be, and those sent whose fate is not
/// known, both `unshown` (the question on the past says Sioul ran; an older
/// Sioul reminds them; which of the two, and what each device did, is in
/// `dose_reminders`), and the errands' tasks made.
fn note_minute(record: &mut HealthState, reminded: &[String], unshown: &[String], maybe: &[String], made: &[(String, String)], stamp: i64) {
    for key in reminded {
        record.reminded.insert(key.clone(), stamp);
    }
    for key in unshown.iter().chain(maybe) {
        record.unshown.entry(key.clone()).or_insert(stamp);
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
    /// Nothing read here, the health file nor its last read: said, never an
    /// empty list read as no doses (review of 36002ea's check, low 19).
    note: String,
}

/// Rows of doses with the safety net of a medicine taken every few hours
/// said first in their doubt (`taken_lately`).
fn with_lately(mut rows: Vec<DoseRow>, health: &Health, state: &HealthState, now: &Zoned) -> Vec<DoseRow> {
    for row in &mut rows {
        if let Some(said) = taken_lately(&row.key, health, state, now) {
            row.doubt = if row.doubt.is_empty() { said } else { format!("{said} {}", row.doubt) };
        }
    }
    rows
}

/// `records` and `me` (this device's id): today's doses whose answers differ
/// on your devices come too, both answers said, until you choose.
#[allow(clippy::too_many_arguments)]
fn porch_doses(health: &Health, state: &HealthState, records: &DoseRecords, told: &DoseReminders, me: &str, knowledge: &Knowledge, now: &Zoned, silent: bool, heard: bool, words: &Translator) -> PorchDoses {
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
    PorchDoses { due, closed: if heard { missed_rows(health, state, told, knowledge, now, words) } else { Vec::new() }, note: String::new() }
}

/// The Porch's doses, as JSON (`PorchDoses`), made off the window's thread.
pub(crate) fn missed() -> String {
    missed_of(&Reads::now())
}

/// The Porch's doses, from what was read (`Reads`).
fn missed_of(reads: &Reads) -> String {
    let Some((health, last_known)) = reads.known.clone() else {
        return json(&PorchDoses { note: tr().text("health-unreadable", None), ..PorchDoses::default() });
    };
    if health.medicines.is_empty() {
        return json(&PorchDoses::default());
    }
    let heard = !crate::share::on() || crate::share::last_exchange().is_some();
    let (state, now) = (&reads.state, Zoned::now());
    let mut doses = porch_doses(&health, state, &reads.records, &reads.told, &this_device(), &reads.knowledge, &now, crate::hours::doses_silent(), heard, tr());
    doses.due = with_lately(doses.due, &health, state, &now);
    doses.closed = with_lately(doses.closed, &health, state, &now);
    // The health file not read here: each dose as last known, to check first (review of 36002ea, can-wait 3).
    if last_known {
        let said = tr().text("dose-last-known", None);
        for row in doses.due.iter_mut().chain(doses.closed.iter_mut()) {
            row.doubt = if row.doubt.is_empty() { said.clone() } else { format!("{said} {}", row.doubt) };
        }
    }
    json(&doses)
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
    // Taken out on purpose: marked, so that your other devices take them out too (the check of 1b6795e, H5).
    let taken_out: Vec<String> = health.medicines.iter().filter(|m| gone.contains(&m.id)).map(|m| m.id.clone()).collect();
    health.medicines.retain(|m| !gone.contains(&m.id));
    health.mark_removed(&taken_out, Timestamp::now().as_second());
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
/// One the last read alone holds (given in place of a file read as no
/// medicines, `emptying_held`) is marked all the same, so that it stops
/// ringing (the A6 check, M2).
pub(crate) fn remove(id: &str) -> String {
    let in_last_read = last_read(&Health::default_path()).is_some_and(|copy| copy.medicines.iter().any(|m| m.id == id));
    change_health(|health| {
        // Taken out on purpose: marked, so that your other devices take it out too (the check of 1b6795e, H5).
        let mut taken_out: Vec<String> = health.medicines.iter().filter(|m| m.id == id).map(|m| m.id.clone()).collect();
        if taken_out.is_empty() && in_last_read {
            taken_out.push(id.to_string());
        }
        health.medicines.retain(|m| m.id != id);
        health.mark_removed(&taken_out, Timestamp::now().as_second());
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
/// reaches you with the window hidden, from the computer you are at. A phone
/// makes none (docs/android.md, "Notifications"): nothing is tried there, so
/// that its status line never says notifications are not made on Android
/// (seen on a phone, 16 October 2026).
fn movement_tick(qt: &QtThread, shared: &Arc<Shared>, health: &Health, now: &Zoned, here: bool) {
    use std::sync::atomic::Ordering;
    if cfg!(target_os = "android") {
        return;
    }
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

// ---------------------------------------------------------------- today's doses, one row

/// One dose of today, as every place that shows today's doses reads it (the
/// unified agenda, §8.1): Health's list, the Porch, the dose card and the
/// agenda each lay it out their own way, from this one row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DoseToday {
    /// `<medicine>@<Unix seconds>`: the key every dose path uses; routing only.
    pub(crate) key: String,
    /// Its day: a row of another day is no row of today.
    pub(crate) day: Date,
    /// When it is due, Unix seconds.
    pub(crate) at: i64,
    /// The medicine's name and the take's amount: for the card, Health's list
    /// and the Porch, which name the medicine beside its buttons; never the timeline.
    pub(crate) name: String,
    pub(crate) amount: String,
    /// What is known of it, given ahead from the reading to the day's end:
    /// each state from the moment it holds, in order.
    pub(crate) states: Vec<GivenState>,
}

/// A state of a dose from a moment on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GivenState {
    /// From when it holds, Unix seconds.
    pub(crate) from: i64,
    pub(crate) state: sioul_core::timeline::DoseState,
    /// The doubt, the question, or the answers that differ, in words; "" for a plain state.
    pub(crate) words: String,
}

/// Today's doses as one reading has them (`doses_today`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DosesToday {
    /// The day the reading is of.
    pub(crate) date: Date,
    /// When it was read, Unix seconds.
    pub(crate) read_at: i64,
    /// The health file read here, not as last known: else every open dose
    /// is to check (`Check`), never plainly due.
    pub(crate) readable: bool,
    pub(crate) rows: Vec<DoseToday>,
}

/// Today's doses from one read (`Reads`), as of `now`: those of the schedule
/// and those taken today at a time it no longer has, as Health's day lists
/// them, whatever week the page shows; each with its states given ahead.
/// Taken, or answered differently on two devices, or due while Sioul was
/// closed and answered nowhere (the question on the past): so for the day.
/// Else, before its time, open and not come; from its time, known not
/// marked on any device from when your devices' news can say so
/// (`health::known_from`, until `known_until`), open, late past its half
/// hour; to check while not known. Nothing read here: no row, and the
/// reading says it (`readable`). No state says "not taken".
pub(crate) fn doses_today(reads: &Reads, now: &Zoned) -> DosesToday {
    use sioul_core::timeline::DoseState;
    let zone = now.time_zone().clone();
    let today = now.date();
    let stamp = now.timestamp().as_second();
    let start = midnight(today, &zone);
    let next = today.tomorrow().map_or(start + 86_400, |d| midnight(d, &zone));
    let Some((health, last_known)) = &reads.known else {
        return DosesToday { date: today, read_at: stamp, readable: false, rows: Vec::new() };
    };
    let knowledge = &reads.knowledge;
    // As last known (the file does not read here): every open dose to check. A record
    // lost here doubts the doses due before it was found so (`doubts_now`), each said.
    let readable = !last_known;
    let (Ok(from_z), Ok(to_z)) = (Timestamp::from_second(start).map(|t| t.to_zoned(zone.clone())), Timestamp::from_second(next).map(|t| t.to_zoned(zone.clone()))) else {
        return DosesToday { date: today, read_at: stamp, readable, rows: Vec::new() };
    };
    // As Health's day lists them (`day_view`): the schedule's, and those taken today at a time it no longer has.
    let mut doses: Vec<(String, i64, String, String)> = health.doses(&from_z, &to_z).into_iter().map(|d| (d.key, d.at.timestamp().as_second(), d.name, d.dose)).collect();
    for key in reads.state.taken.keys() {
        let Some((id, due)) = key.rsplit_once('@') else { continue };
        let (Some(medicine), Ok(due)) = (health.medicines.iter().find(|m| m.id == id), due.parse::<i64>()) else { continue };
        if due >= start && due < next && !doses.iter().any(|d| &d.0 == key) {
            doses.push((key.clone(), due, medicine.short_name(), health.amount_of(key, &zone).unwrap_or_default()));
        }
    }
    doses.sort_by_key(|d| d.1);
    // Due earlier and answered nowhere: the question on the past.
    let asked: std::collections::BTreeMap<String, String> = missed_rows(health, &reads.state, &reads.told, knowledge, now, tr()).into_iter().map(|row| (row.key, row.question)).collect();
    let doubt_at = |due: i64, at: i64| doubt_now_with(knowledge, due, at, tr()).0;
    let given = |from: i64, state: DoseState, words: String| GivenState { from, state, words };
    let rows = doses
        .into_iter()
        .map(|(key, due, name, amount)| {
            let states = match sioul_core::doses::answered(&key, &reads.state, &reads.records) {
                Answered::Taken(answer) => vec![given(stamp, DoseState::Taken { at: answer.at }, String::new())],
                Answered::Differ(answers) => vec![given(stamp, DoseState::Differ, differ_sentence(&answers, &this_device(), tr()))],
                // Said not taken: shown as Health's list shows it, open, with the doubt whenever another device may know.
                _ if asked.contains_key(&key) => vec![given(stamp, DoseState::Asked, asked.get(&key).cloned().unwrap_or_default())],
                _ => {
                    let mut out = Vec::new();
                    if due > stamp {
                        out.push(given(stamp, DoseState::Open { come: false, late: false }, String::new()));
                    }
                    let from = stamp.max(due);
                    let until = (due + MISSED_HOURS * 3600).min(next).max(from);
                    let late = due + GRACE_MINUTES * 60;
                    let known = if readable { sioul_core::health::known_from(due, from, until, knowledge.record_lost, &knowledge.peers) } else { None };
                    match known {
                        None => {
                            // As last known, said; else the doubt that keeps it from being known.
                            let why = if *last_known { tr().text("dose-last-known", None) } else { doubt_at(due, from) };
                            out.push(given(from, DoseState::Check { why: why.clone() }, why));
                        }
                        Some(known) => {
                            let known = known.max(from);
                            if known > from {
                                let why = doubt_at(due, from);
                                out.push(given(from, DoseState::Check { why: why.clone() }, why));
                            }
                            let ends = sioul_core::health::known_until(due, known, until, knowledge.record_lost, &knowledge.peers).max(known);
                            out.push(given(known, DoseState::Open { come: true, late: known >= late }, String::new()));
                            if late > known && late < ends {
                                out.push(given(late, DoseState::Open { come: true, late: true }, String::new()));
                            }
                            if ends < until {
                                let why = doubt_at(due, ends);
                                out.push(given(ends, DoseState::Check { why: why.clone() }, why));
                            }
                        }
                    }
                    out
                }
            };
            DoseToday { key, day: today, at: due, name, amount, states }
        })
        .collect();
    DosesToday { date: today, read_at: stamp, readable, rows }
}

/// How long a reading of the doses holds what it knows (§8.2): past it,
/// every open dose is to check, whatever stalls; a dose taken stays taken.
pub(crate) const READING_HOLDS: i64 = 2 * 60;

/// What a row of today's doses shows at `now` (Unix seconds), on `today`,
/// read at `read_at`, Sioul back on the screen since `back` (0: never away):
/// its state given ahead for that moment; a reading of another day is being
/// read (nothing of it carries over, its "taken" included); a dose taken
/// stays taken; any other, with no reading since Sioul came back, is being
/// read; an open dose read longer ago than `READING_HOLDS` is to check. With
/// its words.
// The dose card reads it in Rust (the unified agenda, part 3); the timeline
// turns its marks by the same rule from `doseMarks` (`holds`, `old`, `reading`).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn dose_state(row: &DoseToday, today: Date, now: i64, read_at: i64, back: i64) -> (sioul_core::timeline::DoseState, String) {
    use sioul_core::timeline::DoseState;
    // A reading of another day is no reading: nothing of it carries over, its "taken" included.
    if row.day != today {
        return (DoseState::Reading, tr().text("dose-reading", None));
    }
    let Some(given) = row.states.iter().rev().find(|g| g.from <= now).or_else(|| row.states.first()) else {
        return (DoseState::Reading, tr().text("dose-reading", None));
    };
    match &given.state {
        // Taken stays taken, being read again or not (§8.2).
        DoseState::Taken { .. } => (given.state.clone(), given.words.clone()),
        // None since Sioul came back on the screen: being read.
        _ if back > read_at => (DoseState::Reading, tr().text("dose-reading", None)),
        DoseState::Open { .. } if now - read_at > READING_HOLDS => {
            let why = tr().text("dose-reading-old", None);
            (DoseState::Check { why: why.clone() }, why)
        }
        state => (state.clone(), given.words.clone()),
    }
}

/// The Health page, the Porch's doses and today's meals and naps, from one
/// read of what is known of the doses (`Reads`).
fn views(waiting: &Waiting) -> (String, String, String, DosesToday) {
    // The reading's time is the read's, before anything of it is made (review of 1880bce, L11).
    let read_at = Zoned::now();
    let reads = Reads::now();
    (page_of(&reads, waiting), missed_of(&reads), needs_today(), doses_today(&reads, &read_at))
}

/// Today's doses as the timeline marks them (`doseMarks`): each dose's key
/// (routing only), its day, its minute of the day, its states given ahead
/// with their words, and the words of "being read" and of a reading grown
/// old (`dose_state`'s); never a medicine's name.
fn dose_marks_json(doses: &DosesToday, zone: &TimeZone) -> String {
    let start = midnight(doses.date, zone);
    let rows: Vec<serde_json::Value> = doses
        .rows
        .iter()
        .map(|row| serde_json::json!({ "key": row.key, "day": row.day.to_string(), "minute": (row.at - start) / 60, "at": row.at, "states": row.states.iter().map(|g| serde_json::json!({ "from": g.from, "state": g.state, "words": g.words })).collect::<Vec<_>>() }))
        .collect();
    json(&serde_json::json!({ "date": doses.date.to_string(), "readable": doses.readable, "holds": READING_HOLDS, "reading": tr().text("dose-reading", None), "old": tr().text("dose-reading-old", None), "rows": rows }))
}

/// The Health page, the doses due while closed and today's meals and naps,
/// made on a thread (they read the doses' record and the other devices'
/// claims, a shared folder that may be slow) and shown.
pub(crate) fn show_health(qt: &QtThread, shared: &Arc<Shared>) {
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.health_job, move |shared| {
        let (view, missed, needs, today) = views(&crate::mail::hidden_pim(shared));
        let (marks, read_at) = (dose_marks_json(&today, &TimeZone::system()), today.read_at);
        if cfg!(target_os = "android") {
            eprintln!("sioul: health: {} medicines, {} prescriptions", load().medicines.len(), load().prescriptions.len());
        }
        let _ = qt.queue(move |mut sioul| {
            sioul.as_mut().set_health_view(QString::from(&view));
            sioul.as_mut().set_missed_view(QString::from(&missed));
            sioul.as_mut().set_needs_view(QString::from(&needs));
            sioul.as_mut().set_dose_marks(QString::from(&marks));
            sioul.as_mut().set_dose_read_at(read_at as f64);
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
/// [{key, at (Unix milliseconds), title}], from half a day ago (`MISSED_HOURS`)
/// to two days ahead, those marked or reminded left out: a dose held by sleep
/// or a pause, asked again later under its own key, stays listed while it is
/// neither, so that no hand cancels its asking (`DoseAlarms.Plan` keeps it at
/// its later time; one past half an hour and never armed is left alone
/// there: the check of 1b6795e, M6). A record that does not read leaves none
/// out: the decision at their time reads it again.
/// Nothing when nothing reads here, the health file nor its last read:
/// Android keeps the alarms it has, as `wake::schedule` does, never an empty
/// list (review of 36002ea, can-wait 3).
/// A dose whose reminder a hand took away as gone (`HealthState::taken_away`)
/// counts as not reminded once its schedule holds it again (a late mark
/// taken back, a time changed back, a pause resumed: listed and rung again);
/// and the dose of the same medicine that replaced a gone one, its time
/// come within the Porch's hours, nearest the gone one's, is tagged
/// (`"replaces"`: the gone key), so that Android rings it once at once even
/// past the half hour (`DoseAlarms.Plan`; the A4 check, #1 and #8: a
/// correction typed at 14:50 moving 14:00 to 14:10 rang nowhere).
pub(crate) fn alarms_coming() -> Option<String> {
    let (health, _) = load_known()?;
    let state = HealthState::read(&HealthState::default_path()).unwrap_or_default();
    let now = crate::alarms::now_here();
    let stamp = now.timestamp().as_second();
    let start = now.checked_sub(Span::new().hours(MISSED_HOURS)).unwrap_or_else(|_| now.clone());
    let end = now.checked_add(Span::new().hours(ALARMS_AHEAD_HOURS)).unwrap_or_else(|_| now.clone());
    let all = health.doses(&start, &end);
    // Gone: no longer in the schedule at all (one back in it, marked since, replaces nothing).
    let scheduled: std::collections::BTreeSet<&str> = all.iter().map(|d| d.key.as_str()).collect();
    let open = |d: &&sioul_core::health::Dose| !state.taken.contains_key(&d.key) && !state.not_taken.contains_key(&d.key) && !state.reminded_now(&d.key);
    let doses: Vec<&sioul_core::health::Dose> = all.iter().filter(|d| open(d)).collect();
    let mut replaces: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
    // Back in the schedule, unmarked, after its reminder went: rung again once, at once, even past
    // the half hour (a late mark taken back long after its time: the A5 dose model, I12).
    for back in doses.iter().filter(|d| state.taken_away.get(&d.key).is_some_and(|at| state.reminded.get(&d.key) == Some(at))) {
        replaces.insert(back.key.as_str(), back.key.as_str());
    }
    // Gone: out of a schedule that would still hold it (due within the half day listed), never one aged out of it,
    // nor one gone with the zone (`gone_with_zone`), nor one answered since (taken away while its gesture was half
    // written, its file moved before its mark: the A6 dose model). What replaced it: the dose of its medicine
    // nearest its time, due by now, rung or marked since or not; rung once, at once, while it is neither, and never
    // another after it (a noon dose left unanswered never rings at 20:31 for a 20:00 moved to 19:00, once 19:00
    // rang or was marked).
    let from = start.timestamp().as_second();
    let zoned = keyed_before_zone(&health, &start, &end);
    let answered = |key: &str| state.taken.contains_key(key) || state.not_taken.contains_key(key);
    for gone in state.taken_away.iter().filter(|(key, at)| state.reminded.get(*key) == Some(at) && !scheduled.contains(key.as_str()) && due_of(key).is_some_and(|due| due >= from) && !gone_with_zone(&zoned, key, **at) && !answered(key)).map(|(key, _)| key.as_str()) {
        let (Some((id, _)), Some(due)) = (gone.rsplit_once('@'), due_of(gone)) else { continue };
        let nearest = all.iter().filter(|d| d.key.rsplit_once('@').is_some_and(|(other, _)| other == id) && d.at.timestamp().as_second() <= stamp).min_by_key(|d| ((d.at.timestamp().as_second() - due).abs(), d.at.timestamp().as_second()));
        if let Some(dose) = nearest.filter(|d| open(d)) {
            replaces.entry(dose.key.as_str()).or_insert(gone);
        }
    }
    let rows: Vec<serde_json::Value> = doses
        .iter()
        .map(|d| match replaces.get(d.key.as_str()) {
            Some(gone) => serde_json::json!({ "key": d.key, "at": d.at.timestamp().as_millisecond(), "title": named(d), "replaces": gone }),
            None => serde_json::json!({ "key": d.key, "at": d.at.timestamp().as_millisecond(), "title": named(d) }),
        })
        .collect();
    Some(serde_json::Value::Array(rows).to_string())
}

/// The doses of the last day marked taken or not taken, each with when it
/// was last reminded (0 never): their reminders, if shown on a phone, are
/// taken away (`alarms::schedule`), again when reminded anew since.
pub(crate) fn marked_lately() -> Vec<(String, i64)> {
    let Ok(state) = HealthState::read(&HealthState::default_path()) else { return Vec::new() };
    let since = Timestamp::now().as_second() - 26 * 3600;
    state.taken.keys().chain(state.not_taken.keys()).filter(|key| due_of(key).is_some_and(|due| due >= since)).map(|key| (key.clone(), state.reminded.get(key).copied().unwrap_or(0))).collect()
}

/// The doses reminded in the last day (here or elsewhere, `HealthState::reminded`)
/// that the schedule no longer holds, as it reads now, mended from the
/// record (`load_known`): moved away by a correction, a late Taken or a
/// change of their medicine on another device, or their medicine taken out
/// or paused; each with when it was reminded. Their reminders, if shown on a
/// phone, are taken away (`alarms::schedule`), as an alarm of theirs finds
/// them (`alarm_decide`), and noted so (`note_taken_away`), so that they come
/// back with their dose. Only doses due within the day the schedule is read
/// over: one reminded late, its time before that, is never taken for gone
/// (the A4 check, #4); one taken away already is not listed again. None
/// when nothing reads here: what Android shows stays, as its alarms do.
pub(crate) fn reminded_gone() -> Option<Vec<(String, i64)>> {
    let (health, _) = load_known()?;
    let state = HealthState::read(&HealthState::default_path()).ok()?;
    let now = crate::alarms::now_here();
    let stamp = now.timestamp().as_second();
    let start = now.checked_sub(Span::new().hours(26)).unwrap_or_else(|_| now.clone());
    let end = now.checked_add(Span::new().hours(ALARMS_AHEAD_HOURS)).unwrap_or_else(|_| now.clone());
    let held: std::collections::BTreeSet<String> = health.doses(&start, &end).into_iter().map(|d| d.key).collect();
    let from = start.timestamp().as_second();
    // Keyed in the zone before its change, and reminded then: gone with the zone, its reminder left as it is (M3).
    let zoned = keyed_before_zone(&health, &start, &end);
    Some(
        state
            .reminded
            .iter()
            .filter(|(key, at)| stamp - **at < 26 * 3600 && due_of(key).is_some_and(|due| due >= from) && !held.contains(*key) && !state.taken.contains_key(*key) && !state.not_taken.contains_key(*key) && state.taken_away.get(*key) != Some(at) && !gone_with_zone(&zoned, key, **at))
            .map(|(key, at)| (key.clone(), *at))
            .collect(),
    )
}

/// The doses as the schedule keyed them in each zone in force before the
/// one in force now, within the last day (`alarms::ZoneSaid::earlier`), over
/// `start..end`, each with when that zone ended; none when no change of zone
/// is known. A set time is a time of the day in the zone in force: its key
/// changes with the zone.
fn keyed_before_zone(health: &Health, start: &Zoned, end: &Zoned) -> Vec<(std::collections::BTreeSet<String>, i64)> {
    let said = crate::alarms::zone_said();
    said.earlier
        .iter()
        .filter(|earlier| earlier.zone != said.zone)
        .filter_map(|earlier| {
            let zone = jiff::tz::TimeZone::get(&earlier.zone).ok()?;
            Some((health.doses(&start.with_time_zone(zone.clone()), &end.with_time_zone(zone)).into_iter().map(|d| d.key).collect(), earlier.until))
        })
        .collect()
}

/// Whether a dose no longer in the schedule is gone with the zone, not moved:
/// keyed so in a zone in force before, and reminded before that zone ended.
/// Its reminder stays as it is, and nothing replaces it (the A5 check, M3: at
/// 18:00 in Tokyo, the 08:00 dose of Tokyo's day rang at once for 08:00 in
/// Paris, answered there, and 20:00 rang two hours later; the A6 check, L4:
/// with a stop at Dubai between).
fn gone_with_zone(zoned: &[(std::collections::BTreeSet<String>, i64)], key: &str, reminded: i64) -> bool {
    zoned.iter().any(|(keys, until)| reminded < *until && keys.contains(key))
}

/// The reminders a hand took away as gone (`reminded_gone`), noted beside
/// their reminded stamp in the record both processes read
/// (`HealthState::taken_away`): back in the schedule and unmarked, such a
/// dose counts as not reminded, and is armed and rung again
/// (`alarms_coming`, `alarm_decide`, `to_remind`); its dose's replacement,
/// already past, rings once (the A4 check, #1 and #8).
pub(crate) fn note_taken_away(gone: &[(String, i64)]) {
    if gone.is_empty() {
        return;
    }
    let _ = change(|record| {
        for (key, at) in gone {
            if record.reminded.get(key) == Some(at) {
                record.taken_away.insert(key.clone(), *at);
            }
        }
    });
}

/// When a dose was last reminded here or elsewhere (`HealthState::reminded`), 0 never or not read.
pub(crate) fn reminded_at(key: &str) -> i64 {
    HealthState::read(&HealthState::default_path()).ok().and_then(|state| state.reminded.get(key).copied()).unwrap_or(0)
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
    let known = load_known();
    let nothing_read = known.is_none();
    let (health, last_known) = known.unwrap_or_else(|| (Health::default(), true));
    let now = crate::alarms::now_here();
    let stamp = now.timestamp().as_second();
    let _held = state_held();
    let state = record();
    let start = now.checked_sub(Span::new().hours(MISSED_HOURS)).unwrap_or_else(|_| now.clone());
    let end = now.checked_add(Span::new().hours(ALARMS_AHEAD_HOURS)).unwrap_or_else(|_| now.clone());
    // The medicine taken out, or its doses moved: this one is no more, nor its reminder.
    let Some(dose) = health.doses(&start, &end).into_iter().find(|d| d.key == key) else {
        // Nothing read here, the health file nor its last read: the alarm
        // Android kept rings, said in doubt, unless the dose was marked, its
        // time is past the Porch's hours or a recorded move of its medicine
        // replaced it, or your sleep keeps doses silent; shown as the alarm
        // that reads shows it: recorded reminded and said to your other
        // devices, once across restarts (review of 36002ea, can-wait 3; its
        // check, must-fix 5).
        let due = due_of(key).unwrap_or(stamp);
        if nothing_read && !state.taken.contains_key(key) && !state.not_taken.contains_key(key) && stamp - due <= MISSED_HOURS * 3600 && !replaced_by_move(key, &state) {
            if crate::hours::doses_silent()
                && let Some(waking) = crate::hours::waking()
            {
                return answer(false, "", "", waking);
            }
            if reminded_already(&state, key) {
                let again = if stamp < due + SHOWN_WATCH { stamp + SHOWN_EVERY } else { 0 };
                return answer(false, "", "", again);
            }
            let at = Timestamp::from_second(due).map(|t| t.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default();
            let lately = marked_lately_said(key, &state, &now).map(|said| format!(" {said}")).unwrap_or_default();
            remind_shown(key, stamp, _held);
            return answer(true, "", &format!("{at}. {}{lately}", tr().text("dose-last-known", None)), stamp + SHOWN_EVERY);
        }
        crate::alarms::remove_reminder(key);
        // Gone (moved, its medicine changed): Android handed the doses as
        // they are now, the moved one armed. A change of time zone hands them
        // at once in the zone in force (`alarms::sioul_dose_zone`).
        drop(_held);
        crate::alarms::schedule();
        return answer(false, "", "", 0);
    };
    let due = dose.at.timestamp().as_second();
    // Marked, here or on another device: its reminder, if shown, goes, and
    // Android is handed the doses as the mark left them.
    if state.taken.contains_key(key) || state.not_taken.contains_key(key) {
        crate::alarms::remove_reminder(key);
        drop(_held);
        crate::alarms::schedule();
        return answer(false, "", "", 0);
    }
    // Reminded already (here, still shown, or on another device): looked at
    // again a while, so that a mark made elsewhere takes the reminder away.
    // One a hand took away as gone, its dose back: reminded again.
    if reminded_already(&state, key) {
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
    // News waited for from the dose's time; held by sleep or a pause, from
    // waking: a computer keeping the doses reminds it then too.
    let since = news_from(due, crate::hours::woken());
    let waiting = stamp < since + WAIT_FOR_NEWS;
    let keeper = crate::share::looked("health", sioul_sync::lease::Rule::FollowsYou);
    if !keeper.mine && waiting {
        return answer(false, "", "", since + WAIT_FOR_NEWS);
    }
    let knowledge = know();
    // What the other devices did to remind it (docs/health.md, "What became of
    // a reminder"): shown somewhere, a reminder (this alarm silent); this
    // alarm plain only when each other device that could have reminded it is
    // known, by its own word, not to have; else why not, said first.
    // A dose back in the schedule after its reminder was taken away: every
    // `shown` of it is that reminder, passed over (`back`; the A5 check, M1).
    let told_now = told_here(&state);
    let me = this_device();
    // A computer's "closed" taken as its word only after this alarm's own pull went through (C1).
    let pulled = missed.is_none() && PULLED.load(std::sync::atomic::Ordering::Relaxed);
    let first = match sioul_core::dose_reminders::others(key, since, stamp, &me, true, told_now.as_ref(), &knowledge.peers, pulled) {
        Others::Reminded => {
            let again = if stamp < due + SHOWN_WATCH { stamp + SHOWN_EVERY } else { 0 };
            return answer(false, "", "", again);
        }
        Others::First => None,
        Others::Doubt(elsewhere) => {
            // Kept here, the phone in use at the dose's time: the computers'
            // words that they leave it to the phone come within a minute or
            // two; waited for that long.
            if keeper.mine && elsewhere.iter().all(|e| matches!(e, Elsewhere::Unknown { .. })) && stamp < since + FIRST_WORD {
                return answer(false, "", "", since + FIRST_WORD);
            }
            Some(elsewhere_said(&elsewhere, since, tr()))
        }
    };
    let doubt = doubt_unread_with(&knowledge, due, since, stamp, missed.as_ref(), first.as_deref());
    if first.is_none() && !doubt.is_empty() && waiting {
        let known = sioul_core::health::known_from_at(due, since, stamp + 1, since + WAIT_FOR_NEWS, knowledge.record_lost, &knowledge.peers);
        return answer(false, "", "", alarm_again(since, stamp, known));
    }
    let named = named(&dose);
    let (title, body) = if doubt.is_empty() { (named, dose.at.strftime("%H:%M").to_string()) } else { (say("dose-check-title", &[("dose", named)]), format!("{}. {doubt}", dose.at.strftime("%H:%M"))) };
    let (title, body) = as_last_known(&dose, title, body, last_known);
    let body = match taken_lately(key, &health, &state, &now) {
        Some(said) => format!("{body} {said}"),
        None => body,
    };
    let (title, body) = said_back(&dose, &state, &now, title, body);
    remind_shown(key, stamp, _held);
    // Shown: looked at again while it may still be shown (a mark elsewhere takes it away).
    answer(true, &title, &body, stamp + SHOWN_EVERY)
}

/// A reminder the phone's alarm shows, noted before it shows: in a session
/// of its own, Sioul not on the screen (`devices::receiver`): up, the
/// reminder and the dose's record (the canary) noted, sent, down. Your other
/// devices know it was reminded: they do not remind it again. `held`: the
/// record's hold, let go once noted.
fn remind_shown(key: &str, stamp: i64, held: std::sync::MutexGuard<'static, ()>) {
    if let Ok(mut sent) = SENT.lock() {
        sent.insert(key.to_string());
    }
    crate::devices::receiver(move || {
        let _ = change(|record| {
            record.reminded.insert(key.to_string(), stamp);
            record.taken_away.remove(key);
        });
        // About to show: a reminder it never took away (the A4 check, #3).
        crate::alarms::forget_removed(key);
        // Its own word, before it shows: the others read it reminded.
        tell_mine(key, |mine| {
            mine.maybe = stamp;
            mine.shown = stamp;
        });
        open_canaries(&[key.to_string()], stamp);
        drop(held);
        let _ = crate::share::exchange_here(false);
    });
}

/// Whether a dose is back in the schedule after a hand took its reminder
/// away as gone (`HealthState::taken_away`, still its reminded stamp): a
/// mark taken back, a time changed back, a pause resumed. Its reminder is
/// due again, and every `shown` of it is the one taken away (the A5 check,
/// M1: such a dose rang, then was decided silent).
fn back(state: &HealthState, key: &str) -> bool {
    state.taken_away.get(key).is_some_and(|at| state.reminded.get(key) == Some(at))
}

/// What each device did to remind each dose, as read here, the `shown` of
/// every dose back in the schedule passed over (`back`): in memory only,
/// never written. None when the file does not read.
fn told_here(state: &HealthState) -> Option<DoseReminders> {
    let mut told = told()?;
    for (key, devices) in &mut told.dose {
        if back(state, key) {
            for tried in devices.values_mut() {
                tried.shown = 0;
            }
        }
    }
    Some(told)
}

/// A dose back in the schedule, reminded again: the doubt said, as it may
/// have been taken without a press of Taken ("Reminded at 14:00 already:
/// check before you take it."), under the check title. Other words as they are.
fn said_back(dose: &sioul_core::health::Dose, state: &HealthState, now: &Zoned, title: String, body: String) -> (String, String) {
    if !back(state, &dose.key) {
        return (title, body);
    }
    let at = state.reminded.get(&dose.key).and_then(|at| Timestamp::from_second(*at).ok()).map(|t| t.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    (say("dose-check-title", &[("dose", named(dose))]), format!("{body} {}", say("dose-reminded-before", &[("time", at)])))
}

/// Whether a dose was reminded already, here (this process showed it:
/// `SENT`) or on any device (`HealthState::reminded`), and its reminder still
/// stands: one a hand took away as gone since (`taken_away`), its dose back,
/// is reminded again, whatever this process showed (the A4 check, #8).
fn reminded_already(state: &HealthState, key: &str) -> bool {
    state.reminded_now(key) || (!state.taken_away.contains_key(key) && SENT.lock().is_ok_and(|sent| sent.contains(key)))
}

/// Whether a dose's time is one a recorded move of its medicine replaced:
/// its medicine started again after it, from a dose taken before it
/// (`HealthState::moved`), so that its schedule no longer holds it; known
/// without the health file (review of 36002ea's check, must-fix 5).
fn replaced_by_move(key: &str, state: &HealthState) -> bool {
    let (Some((id, _)), Some(due)) = (key.rsplit_once('@'), due_of(key)) else { return false };
    state.moved.iter().any(|(taken, [_, after])| taken.rsplit_once('@').is_some_and(|(other, _)| other == id) && due_of(taken).is_some_and(|first| first < due) && due < *after)
}

/// The latest mark of a dose's medicine in the record, other than its own:
/// the moment it was marked taken (Unix seconds), within the Porch's hours.
fn latest_mark(key: &str, state: &HealthState, now: i64) -> Option<i64> {
    let (id, _) = key.rsplit_once('@')?;
    state.taken.iter().filter(|(other, at)| other.as_str() != key && other.rsplit_once('@').is_some_and(|(medicine, _)| medicine == id) && **at <= now && now - **at <= MISSED_HOURS * 3600).map(|(_, at)| *at).max()
}

/// The safety net of a medicine taken every few hours (review of 36002ea's
/// check, A2), wherever one of its doses is reminded or shown: its latest
/// mark in the record, when it is fewer hours before than the hours between
/// two doses: when it was taken, and from when the next may be. None else.
fn taken_lately(key: &str, health: &Health, state: &HealthState, now: &Zoned) -> Option<String> {
    let (id, _) = key.rsplit_once('@')?;
    let Schedule::Hours { hours, .. } = health.medicines.iter().find(|m| m.id == id)?.schedule else { return None };
    let step = i64::from(hours) * 3600;
    let stamp = now.timestamp().as_second();
    let last = latest_mark(key, state, stamp)?;
    let at = |seconds: i64| Timestamp::from_second(seconds).map(|t| t.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    (stamp - last < step).then(|| say("dose-taken-lately", &[("time", at(last)), ("next", at(last + step))]))
}

/// The same when nothing reads here, its hours not known: the latest mark of its medicine, said.
fn marked_lately_said(key: &str, state: &HealthState, now: &Zoned) -> Option<String> {
    let last = latest_mark(key, state, now.timestamp().as_second())?;
    let at = Timestamp::from_second(last).map(|t| t.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    Some(say("dose-last-marked", &[("time", at)]))
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
        // Marked, nothing read here to move its next doses: said (review of 36002ea, can-wait 2; its check, low 22).
        let line = unmoved_note().unwrap_or_else(|| say("health-taken-at", &[("time", Zoned::now().strftime("%H:%M").to_string())]));
        serde_json::json!({ "done": true, "open": false, "line": line }).to_string()
    })
}

// ---------------------------------------------------------------- a phone's steps while a dose is due

/// Whether a dose's time has come and it is not marked yet, as the Porch
/// still asks about it (until the day's end, twelve hours at most): while one
/// is and another device runs Sioul, a phone in the background keeps its
/// two-minute steps, while you sleep too, so that a mark made there comes
/// within them, not at its next slow look (docs/health.md, "Knowing";
/// `steps::keeps_close`). Read only, nothing repaired here; a record that does
/// not read counts every dose whose time has come as open, never as none, and
/// only within the same hours: never before a dose's time nor past its day's
/// end (review of battery part C, G4).
pub(crate) fn dose_open_now() -> bool {
    let health = load();
    if health.medicines.is_empty() {
        return false;
    }
    let read = HealthState::read(&HealthState::default_path()).ok().zip(DoseRecords::read(&DoseRecords::default_path()).ok());
    dose_open(&health, read.as_ref().map(|(state, records)| (state, records)), &Zoned::now())
}

/// `dose_open_now`, from what was read (`records`: the dose record and the
/// records come from the other devices; none when one does not read): a dose
/// of today's, its time come (twelve hours back at most), answered nowhere
/// this device knows; the records unread, any such dose.
fn dose_open(health: &Health, records: Option<(&HealthState, &DoseRecords)>, now: &Zoned) -> bool {
    let midnight = now.date().to_zoned(now.time_zone().clone()).unwrap_or_else(|_| now.clone());
    let back = now.checked_sub(Span::new().hours(MISSED_HOURS)).unwrap_or_else(|_| now.clone());
    let start = if back > midnight { back } else { midnight };
    let end = now.checked_add(Span::new().seconds(1)).unwrap_or_else(|_| now.clone());
    let mut come = health.doses(&start, &end).into_iter();
    match records {
        Some((state, records)) => come.any(|d| sioul_core::doses::answered(&d.key, state, records) == Answered::Open),
        None => come.next().is_some(),
    }
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
    let known = load_known();
    // The health file not read here, its last read or nothing: said first,
    // until midnight, its doses as last known, every one "check" (review of
    // 36002ea, can-wait 3; its check, low 19).
    let unread = known.as_ref().is_none_or(|(_, last_known)| *last_known);
    let (health, last_known) = known.unwrap_or_else(|| (Health::default(), true));
    let said = unread.then(|| {
        let stamp = now.timestamp().as_second();
        let midnight = now.date().tomorrow().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()).map_or(stamp + 86_400, |m| m.timestamp().as_second());
        crate::homecard::Dose { name: tr().text("health-card-unread", None), time: String::new(), at: stamp, until: midnight, known_until: 0 }
    });
    if health.medicines.is_empty() {
        return said.into_iter().collect();
    }
    let read = HealthState::read(&HealthState::default_path());
    let readable = read.is_ok() && !last_known;
    said.into_iter().chain(doses_for_card(&health, &read.unwrap_or_default(), &records(), readable, &know(), now)).collect()
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
/// before it is reminded all the same, the doubt said; a phone's alarm, when
/// a computer keeps the doses, waits as long for the computer's reminder
/// (`sioul_core::health::ALARM_AFTER`, one moment for both).
const WAIT_FOR_NEWS: i64 = sioul_core::health::ALARM_AFTER;

/// Until when after a dose's time this computer may show its reminder late
/// while a phone shares the doses (a try after the first that did not go
/// out, a dose an earlier Sioul here tried, a reminder waiting for news):
/// three minutes before the phone's alarm, so that what it shows reaches the
/// phone first (its exchange each minute, the server, the phone's own pull at
/// its alarm); later, the phone's alarm and the question on the past take it
/// up (review of battery part C, the fourth review's H2).
const LATE_UNTIL: i64 = WAIT_FOR_NEWS - 3 * 60;

/// How long a phone's alarm keeping the doses itself (the phone in use at the
/// dose's time) waits for the computers' words that they leave the dose to
/// it (`dose_reminders::Tried::quiet`), before it reminds with the doubt.
const FIRST_WORD: i64 = 2 * 60;

/// When a dose's alarm in doubt within the wait for news asks again: as soon
/// as it may be known (`known`: a device closed before the dose, once the
/// news of an opening had the time to come), a minute apart at least; known
/// already by what was read (`known` now), in doubt only because no news came
/// since the alarm began, nothing tells when news comes: at the end of the
/// wait, as for a doubt that has no end in view, then reminded with the doubt
/// said if none came (until 10 October 2026 it asked each minute, a pull each
/// time: R9 of that day's review).
fn alarm_again(since: i64, stamp: i64, known: Option<i64>) -> i64 {
    match known {
        Some(at) if at > stamp + 1 => at.max(stamp + 60).min(since + WAIT_FOR_NEWS),
        _ => since + WAIT_FOR_NEWS,
    }
}

/// From when a phone's alarm waits for news of a dose due at `due`: its
/// time; the dose due while sleep or a pause held it (`hours::woken`: when it
/// began and ended), the waking, when a computer keeping the doses reminds
/// it too, so that the two never remind it together, both plainly (review
/// of battery part C, the fourth round).
fn news_from(due: i64, woken: Option<(i64, i64)>) -> i64 {
    woken.filter(|(from, _)| *from <= due).map_or(due, |(_, woke)| woke.max(due))
}

/// The doses' record found broken or gone here, kept on this computer only
/// (`$XDG_STATE_HOME/sioul/health-doubt.toml`): the doses due in the day
/// before stay not known, and those of the rest of that day, which may have
/// been marked ahead in what was lost (`doubts_at`; review of 1880bce, L13).
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
/// every device's records at the next exchange; it is kept aside in the same
/// step, before any exchange can run, and a new one starts. Sharing failing
/// that, it is left as it is: never started again empty where the sharing
/// could take marks out elsewhere.
fn repair() {
    let now = Timestamp::now().as_second();
    let path = HealthState::default_path();
    let set_aside = || HealthState::set_aside(&path, now);
    let aside = if crate::share::on() {
        match crate::share::rebuild_health_record(set_aside) {
            Ok(aside) => aside,
            Err(_) => {
                write_record_doubt(now, "");
                return;
            }
        }
    } else {
        set_aside()
    };
    write_record_doubt(now, &aside.ok().flatten().map(|p| p.display().to_string()).unwrap_or_default());
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
/// doses due in the day before and in the rest of that day are said not known.
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
/// records; they are kept aside in the same step, and new ones start.
fn repair_records() {
    let now = Timestamp::now().as_second();
    let path = DoseRecords::default_path();
    let set_aside = || DoseRecords::set_aside(&path, now);
    let aside = if crate::share::on() {
        match crate::share::rebuild_dose_records(set_aside) {
            Ok(aside) => aside,
            Err(_) => {
                write_record_doubt(now, "");
                return;
            }
        }
    } else {
        set_aside()
    };
    write_record_doubt(now, &aside.ok().flatten().map(|p| p.display().to_string()).unwrap_or_default());
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
    #[cfg(test)]
    if let Some(id) = DEVICE.with(|device| device.borrow().clone()) {
        return id;
    }
    crate::share::vault().map(|(id, _)| id).unwrap_or_default()
}

#[cfg(test)]
thread_local! {
    /// This device's id in a test's thread, when it says one (two devices
    /// played in one test): never the sharing's file, which other tests read
    /// beside it (review of 36002ea's check, A2: here.toml rewritten under them).
    static DEVICE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
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
    /// Its newest entry in the devices' registry read here, by its own times
    /// (`devices::written`; `peer.seen`: when it came): never replaced by an
    /// older copy put back, nor taken from a clock more than `FUTURE` ahead.
    #[serde(default)]
    entry: Option<sioul_sync::devices::Entry>,
    /// The words (`Entry::acked`) of the copy of its entry read now, when one
    /// reads and is not set aside (`Peer::set_aside`): a phone's word stands
    /// here only where this copy says it as well as the newest one, so that a
    /// copy that is no news may take a word away, never give one (the final
    /// review before 0.0.6, F1H). Not kept from one reading to the next.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    acked_now: Option<std::collections::BTreeMap<String, i64>>,
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

/// How far ahead of this device's clock another device's times may run and
/// still be taken (ten minutes: a clock left to drift; one set wrong by hours
/// or a year is not): beyond it, its copy is no news and is set aside
/// (`Peer::set_aside`): nothing it says counts, nor a word it gives, and its
/// doubt says its clock runs ahead (the ninth review's F1; the final review's
/// F1H).
const FUTURE: i64 = 10 * 60;

/// The latest of an entry's times (its device's clock).
fn latest_time(entry: &sioul_sync::devices::Entry) -> i64 {
    entry.started.max(entry.closed).max(entry.imported).max(entry.exported)
}

/// Whether a copy of an entry is dated more than `FUTURE` ahead of `now`.
fn from_the_future(entry: &sioul_sync::devices::Entry, now: i64) -> bool {
    latest_time(entry) > now + FUTURE
}

/// How many of a device's openings are kept to time the next: an opening
/// whose news came late once (a phone offline when picked up) holds the wait
/// for this many more, then goes.
const WAKES: usize = 10;

/// The longest an opening's news counts as late when this device cannot
/// tell whether its own news came in time (a sync app carries the folder:
/// paused, or stopped, unseen from here): a longer wait is as likely this
/// device's own outage, and would hold the doses of a closed device in doubt
/// that long, for its next `WAKES` openings (review of 10 October 2026, R8).
const UNSURE_WAKE: i64 = 60 * 60;

/// How late the news of a device's opening came here, kept (`PeerSeen::wakes`),
/// when this device's own news was flowing then (`news`: its own pull went
/// through lately; none when a sync app alone carries the folder, the delay
/// then counted up to `UNSURE_WAKE`). Its own pull stale or failing: the
/// delay is its own outage, not timed.
fn note_wake(seen: &mut PeerSeen, delay: i64, news: Option<bool>) {
    let delay = match news {
        Some(false) => return,
        Some(true) => delay,
        None => delay.min(UNSURE_WAKE),
    };
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

#[cfg(test)]
thread_local! {
    /// How many times this thread asked what the devices know (`know`): once a Health run.
    static KNOWN_IN_THREAD: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// How many times this thread read a week's calendars (`week_read`, past its cache).
    static WEEK_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Whether this device's own pull from the server went through lately, as
/// `know` last saw it (`share::Others::news`): a phone's alarm takes a
/// computer's "closed" as its word that it did not remind a dose only then
/// (`dose_reminders::others`; the sixth review's C1).
static PULLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// What this computer knows now: the others' claims on the doses read (each
/// says how far it wrote its records, and whether it closed), set against
/// what was read of their records here.
fn know() -> Knowledge {
    #[cfg(test)]
    KNOWN_IN_THREAD.with(|n| n.set(n.get() + 1));
    let now = Timestamp::now().as_second();
    let record_lost = record_doubt();
    let Some(others) = crate::share::others_on_health() else {
        PULLED.store(false, std::sync::atomic::Ordering::Relaxed);
        return Knowledge { record_lost, peers: Vec::new() };
    };
    PULLED.store(others.news == Some(true), std::sync::atomic::Ordering::Relaxed);
    let _held = KNOWING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // This device's own entry: its word on each one's opening, the devices it replaces.
    let own = sioul_sync::devices::own(&sioul_sync::devices::own_path(&sioul_core::config::state_dir()));
    // Read, learned, written under its lock: a phone's background service learns too, in its own process.
    let kept = sioul_core::filelock::with_lock(&peers_path(), || {
        let mut kept = load_peers();
        learn_with(&mut kept, &others.claims, &others.entries, &others.heard, others.news, now);
        let replaced = replacements(&kept, &others.entries, &own);
        apply_replaced(&mut kept, &replaced);
        save_peers(&kept);
        kept
    });
    // Each a phone or not as it says: what a phone does (its alarm) is read
    // from it; two phones or more are each named by their names, never both
    // "the phone", in the words only (`sioul_core::health::Named`; review of
    // battery part C, the fifth review's C1).
    let mut peers: Vec<Peer> = kept.peers.values().map(|s| s.peer.clone()).collect();
    // This device's own word on each one's opening, as its sealed entry says it (`acknowledged_openings`).
    for peer in &mut peers {
        peer.acknowledged = own.acked.get(&peer.id).copied().unwrap_or(0);
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

/// Every replacement said (`devices::Entry::replaces`): by the entries read
/// now, by the newest copy of each kept here (an older copy put back never
/// takes one away), and by this device's own entry (a phone that joined in
/// another's place).
fn replacements(kept: &Peers, entries: &[sioul_sync::devices::Entry], own: &sioul_sync::devices::Entry) -> Vec<sioul_sync::devices::Replaced> {
    let newest = kept.peers.values().filter_map(|seen| seen.entry.as_ref());
    entries.iter().chain(newest).chain(std::iter::once(own)).flat_map(|e| e.replaces.iter().cloned()).collect()
}

/// The devices another device says it replaces (`devices::Entry::replaces`;
/// this device's own entry's too), counted as forgotten here (`Peer::off`,
/// `Peer::forgotten`: no word owed on this computer's openings, no doses
/// waited for from them) while they said nothing newer than the replacement
/// says: a new start, a later time of their entry, a claim renewed since. A
/// phone said replaced wrongly counts again by itself the moment it speaks.
/// Read anew at each look, never kept as a mark.
fn apply_replaced(kept: &mut Peers, replaced: &[sioul_sync::devices::Replaced]) {
    for r in replaced {
        let Some(seen) = kept.peers.get_mut(&r.id) else { continue };
        let latest = seen.entry.as_ref().map_or(0, latest_time).max(seen.renewed).max(seen.peer.heard).saturating_mul(1000);
        if r.holds(seen.entry.as_ref().map(|e| e.started), latest) {
            seen.peer.off = true;
            seen.peer.forgotten = true;
        }
    }
}

/// What a replacement says of `id` now (`devices::Replaced`): its last start
/// as kept here, and the latest it was heard of, from its entry, its claims
/// and its records (`share::heard_of`), in milliseconds.
pub(crate) fn replaced_now(id: &str) -> sioul_sync::devices::Replaced {
    let _ = know();
    let kept = load_peers();
    let seen = kept.peers.get(id);
    let entry = seen.and_then(|s| s.entry.as_ref());
    let latest = entry.map_or(0, latest_time).max(seen.map_or(0, |s| s.renewed.max(s.peer.heard))).saturating_mul(1000);
    sioul_sync::devices::Replaced { id: id.to_string(), started: entry.map_or(0, |e| e.started), heard: latest.max(crate::share::heard_of(id)) }
}

/// Each other device's newest entry read here (`learn_with`: an older copy
/// put back never replaces it), as `know` last kept them, of the devices
/// whose entry reads now only (`Peer::said`), and not set aside: one no
/// longer read, or dated far ahead, says nothing. Its words (`Entry::acked`)
/// only those the copy read now says too: a copy that is no news (an older
/// one, one written in the same second, one of a phone whose clock was set
/// back) may take a word away, never give one (the final review's F1H).
fn newest_entries() -> Vec<sioul_sync::devices::Entry> {
    newest_of(&load_peers())
}

fn newest_of(kept: &Peers) -> Vec<sioul_sync::devices::Entry> {
    kept.peers
        .values()
        .filter(|seen| seen.peer.said.is_some() && !seen.peer.set_aside)
        .filter_map(|seen| {
            let now = seen.acked_now.as_ref()?;
            let mut entry = seen.entry.clone()?;
            entry.acked.retain(|computer, opening| now.get(computer) == Some(opening));
            Some(entry)
        })
        .collect()
}

fn load_peers() -> Peers {
    std::fs::read_to_string(peers_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Written whole beside its place, then renamed over it (`share::write_atomically`):
/// a crash in mid-write never leaves half of it, which read as nothing would
/// lose the newest copies kept, the openings read and the off and forgotten
/// marks (final review of 15 October 2026).
fn save_peers(kept: &Peers) -> String {
    let written = serde_json::to_string(kept).map_err(|e| e.to_string()).and_then(|text| sioul_sync::share::write_atomically(&peers_path(), text.as_bytes()));
    written.err().unwrap_or_default()
}

/// What the others' claims say, learned: each one's name, how late its news
/// comes, and until when everything it wrote is read here.
#[cfg(test)]
fn learn(kept: &mut Peers, claims: &[sioul_sync::lease::Claim], entries: &[sioul_sync::devices::Entry], heard: &sioul_sync::share::Heard, now: i64) {
    learn_with(kept, claims, entries, heard, None, now);
}

/// The same, `news` saying whether this device's own news was flowing now
/// (`share::Others::news`), for timing the others' openings.
fn learn_with(kept: &mut Peers, claims: &[sioul_sync::lease::Claim], entries: &[sioul_sync::devices::Entry], heard: &sioul_sync::share::Heard, news: Option<bool>, now: i64) {
    // How late a device's opening comes is timed only while this device
    // looked a moment before (review of 5 October 2026, F21), and its own
    // news came in time (`note_wake`).
    let watching = kept.looked > 0 && now - kept.looked <= 2 * 60;
    kept.looked = now;
    for claim in claims {
        let seen = kept.peers.entry(claim.computer.clone()).or_default();
        seen.peer.id = claim.computer.clone();
        seen.peer.name = claim.name.clone();
        // Its claim begun again, open (an older Sioul's opening): how late its news came.
        if claim.since != seen.since {
            if seen.since > 0 && watching && !claim.closed {
                note_wake(seen, now - claim.since, news);
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
        // Renewed while open: it is open, whatever its entry last said (R1d).
        if !claim.closed {
            seen.peer.claimed = seen.peer.claimed.max(claim.renewed);
        }
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
        seen.peer.set_aside = false;
        seen.acked_now = None;
    }
    for entry in entries {
        let seen = kept.peers.entry(entry.id.clone()).or_default();
        seen.peer.id = entry.id.clone();
        if !entry.name.is_empty() {
            seen.peer.name = entry.name.clone();
        }
        // A copy kept from a clock far ahead already when it came (by a
        // Sioul before this rule), that clock since set right: dropped, so
        // that what it says now counts again (the ninth review's F1). One
        // that only looks ahead because this clock went back since is kept,
        // and so are the marks set against it: an older copy put back is no
        // news when this clock is set right (W1), and a device said off or
        // forgotten stays so (the final review's F1L).
        let came = seen.peer.seen;
        let ahead_when_kept = |at: i64| at > now + FUTURE && at > came + FUTURE;
        if seen.entry.as_ref().is_some_and(|last| ahead_when_kept(latest_time(last))) {
            seen.entry = None;
        }
        if ahead_when_kept(seen.peer.latest_opening) {
            seen.peer.latest_opening = 0;
        }
        // A copy dated far ahead of this clock is set aside: no news, no word,
        // said so. The words of the one read now, kept for `newest_of`.
        let aside = from_the_future(entry, now);
        seen.peer.set_aside = aside;
        seen.acked_now = (!aside).then(|| entry.acked.clone());
        // News of it only from a copy newer by its own times than the newest
        // read here: an older genuine copy put back (a server replaying them)
        // is no news, and never keeps a phone's word "fresh" (the ninth
        // review's W1); nor a copy dated far ahead of this clock (F1).
        let newer = !from_the_future(entry, now) && seen.entry.as_ref().is_none_or(|last| sioul_sync::devices::written(entry) > sioul_sync::devices::written(last));
        if newer {
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
                note_wake(seen, now - entry.started + ahead, news);
            }
            seen.entry = Some(entry.clone());
            // Said off, or forgotten, until it says anything newer: a new
            // start or a later export, in a copy taken as news (a claim renewed
            // since: below). A copy set aside or dropped lifts nothing (F1L).
            let spoke = |mark: &Mark| entry.started != mark.started || entry.exported > mark.exported;
            if seen.off.as_ref().is_some_and(spoke) {
                seen.off = None;
            }
            if seen.forgotten.as_ref().is_some_and(spoke) {
                seen.forgotten = None;
            }
        }
        seen.peer.ahead = seen.aheads.iter().copied().max().unwrap_or(0).max(0);
        // What it says: its newest copy read here, never an older one put back.
        let Some(entry) = seen.entry.clone() else { continue };
        let entry = &entry;
        // Its entry says it closed, but its claim, renewed open more than a
        // minute after that close (both on its own clock), says it runs: an
        // older Sioul there now (a version put back), which updates no entry.
        // The entry is not trusted: known by its claims, as an older Sioul is.
        // Not so of one set aside: its claims, dated ahead as its entry is,
        // come after any close it said (its doubt says its clock runs ahead).
        let alive_since = claims.iter().any(|c| c.computer == entry.id && !c.closed && c.renewed > entry.closed + 60);
        if !entry.working && alive_since && !aside {
            continue;
        }
        seen.peer.said = Some(entry.said());
        // The latest opening ever read of it: an older copy put back never lowers it (N3).
        seen.peer.latest_opening = seen.peer.latest_opening.max(entry.started);
        // Everything it wrote up to its last export, read here.
        seen.peer.complete = entry.wrote.is_none_or(|wrote| heard.complete(&entry.id, wrote));
        seen.peer.heard = seen.peer.heard.max(entry.exported);
        seen.peer.broken = heard.broken.get(&entry.id).copied();
    }
    // Said off, or forgotten, until it says anything newer: a new start or a
    // new export (above, in a copy taken as news), a claim renewed since.
    for seen in kept.peers.values_mut() {
        seen.peer.wake = seen.wakes.iter().copied().max();
        let renewed = seen.renewed;
        let newer = |mark: &Mark| renewed > mark.renewed;
        if seen.off.as_ref().is_some_and(newer) {
            seen.off = None;
        }
        if seen.forgotten.as_ref().is_some_and(newer) {
            seen.forgotten = None;
        }
        seen.peer.off = seen.off.is_some() || seen.forgotten.is_some();
        seen.peer.forgotten = seen.forgotten.is_some();
    }
}

/// A device you say is off (`off`), or not any more: not counted for the
/// doses until it says anything newer (a new start, a new export, a claim),
/// said plainly meanwhile (`shared_note`, Settings). It never answers a dose
/// for you. Returns what went wrong, else "".
pub(crate) fn device_off(id: &str, off: bool) -> String {
    mark_device(id, |seen, mark| seen.off = off.then_some(mark))
}

/// Since when another device has said nothing, as this device sees it: the
/// later of when this device saw its entry change and its own last word
/// (`Peer::seen`, `Peer::heard`), as "Forget it now" judges it
/// (`FORGET_NOW_AFTER`); none while this device knows nothing of it.
pub(crate) fn quiet_since(id: &str) -> Option<i64> {
    load_peers().peers.get(id).map(|seen| seen.peer.seen.max(seen.peer.heard)).filter(|since| *since > 0)
}

/// Whether `id` is forgotten here (Forget this device), or replaced by
/// another device (`apply_replaced`), as this device last knew it.
pub(crate) fn forgotten_here(id: &str) -> bool {
    load_peers().peers.get(id).is_some_and(|seen| seen.forgotten.is_some() || seen.peer.forgotten)
}

/// A device silent for a week, forgotten: not listed until it says anything newer.
pub(crate) fn device_forget(id: &str) -> String {
    // Forgotten by the sharing too: it no longer holds its newer form back (docs/database.md).
    crate::share::forget_device(id);
    mark_device(id, |seen, mark| seen.forgotten = Some(mark))
}

/// A device whose files you removed from the folder while it no longer
/// counted for the sharing (it left, was forgotten or replaced): forgotten
/// by the doses too, exactly as "Forget it now" does, before its claims go
/// (its entry too, once nothing needs it): a device whose entry no longer
/// reads is known by its claims, as an older Sioul, and with none left the
/// doses would doubt for it until it had been silent `SILENT_DAYS`. It
/// counts again if it says anything newer. Returns what went wrong, else "".
pub(crate) fn device_forget_doses(id: &str) -> String {
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

/// How long a phone says nothing before a computer offers to forget it now
/// (`DeviceRow::forget_now`): a day, where a living phone restates its entry
/// every hour (`devices::RESTATED`).
pub(crate) const FORGET_NOW_AFTER: i64 = 86_400;

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
    /// On a computer, a phone silent for a day or more, still counted:
    /// "Forget it now", said with what it changes (a phone that will never
    /// share again, dead or lost, holds the doses' reminders back for a week otherwise).
    forget_now: bool,
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
        rows.push(DeviceRow { id: id.clone(), name: capitalized(&words.text("share-other-device", None)), state, notes: Vec::new(), off: false, forget: true, forget_now: false, build: String::new() });
    }
    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
}

fn device_rows_of(peers: &[Peer], kept: &Peers, now: i64, words: &Translator) -> Vec<DeviceRow> {
    let zone = jiff::tz::TimeZone::system();
    let when = |at: i64| Timestamp::from_second(at).map(|t| when_said(&t.to_zoned(zone.clone()), words)).unwrap_or_default();
    let mut rows: Vec<DeviceRow> = peers
        .iter()
        // Forgotten here, or replaced by another device (`apply_replaced`): not listed until it says anything newer.
        .filter(|peer| !peer.id.is_empty() && !peer.forgotten && kept.peers.get(&peer.id).is_none_or(|seen| seen.forgotten.is_none()))
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
            // Its entry from a clock far ahead of this one's, set aside (`FUTURE`): said, so that its clock is set right.
            let future = peer.set_aside;
            let state = if off {
                words.text("share-device-off", None)
            } else if future && entry.is_none() {
                words.text("share-device-future", None)
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
            if future && (off || entry.is_some()) {
                notes.push(words.text("share-device-future", None));
            } else if peer.ahead >= 60 {
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
            // Forgotten once silent a week, or once said off (a phone that never gives its word holds the computers until then: L2).
            let forget = !counted || off;
            // On a computer, a phone silent a day (its entry not changed here since: a living
            // phone restates it each hour): "Forget it now", rather than a week's wait.
            let phone = peer.said.as_ref().is_some_and(|s| s.phone) || entry.is_some_and(|e| e.kind == sioul_sync::devices::PHONE);
            let forget_now = !forget && phone && crate::devices::kind() == sioul_sync::devices::COMPUTER && now - peer.seen.max(peer.heard) >= FORGET_NOW_AFTER;
            DeviceRow { id: peer.id.clone(), name, state, notes, off, forget, forget_now, build }
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
    joined(why_parts(doubts, words), words)
}

/// Each reason in words, one per doubt.
fn why_parts(doubts: &[Doubt], words: &Translator) -> Vec<String> {
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
            Doubt::Ahead { device } => said(words, "dose-doubt-ahead", &[("name", named(device))]),
        })
        .collect();
    parts
}

/// Reasons said one after the other, as `words`' language joins them
/// ("…; …", in French "… ; …" with its thin space: the fifth review's L6).
fn joined(parts: Vec<String>, words: &Translator) -> String {
    // A reason said once, however many ways it came (the eighth review's L1).
    let mut once: Vec<String> = Vec::new();
    for part in parts {
        if !once.contains(&part) {
            once.push(part);
        }
    }
    once.into_iter().reduce(|first, then| said(words, "dose-doubt-and", &[("first", first), ("then", then)])).unwrap_or_default()
}

/// Why a phone's alarm cannot know it is a dose's first reminder, in words,
/// each device said (`dose_reminders::others`): "the desk may have shown its
/// reminder at 08:00", "Pixel may have reminded you too, by its own alarm".
fn elsewhere_said(elsewhere: &[Elsewhere], since: i64, words: &Translator) -> String {
    let zone = jiff::tz::TimeZone::system();
    let when = |at: i64| Timestamp::from_second(at).map(|t| when_said(&t.to_zoned(zone.clone()), words)).unwrap_or_default();
    let parts = elsewhere
        .iter()
        .map(|e| match e {
            Elsewhere::Sent { device, at } => said(words, "dose-doubt-maybe", &[("name", device_word(device, words)), ("when", when(*at))]),
            Elsewhere::Unshown { device, at } => said(words, "dose-doubt-maybe-late", &[("name", device_word(device, words)), ("when", when(*at))]),
            Elsewhere::Unknown { device } => said(words, "dose-doubt-unknown", &[("name", device_word(device, words)), ("when", when(since))]),
            Elsewhere::Closed { device, at } => said(words, "dose-doubt-closed", &[("name", device_word(device, words)), ("when", when(*at))]),
            Elsewhere::Alarm { device } => said(words, "dose-doubt-alarm", &[("name", device_word(device, words))]),
            // The same words as its doubt (`why_parts`): said once (`joined`, `doubt_unread_with`).
            Elsewhere::Ahead { device } => said(words, "dose-doubt-ahead", &[("name", device_word(device, words))]),
            Elsewhere::Unread => words.text("dose-doubt-reminders", None),
        })
        .collect();
    joined(parts, words)
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

/// A dose's doubt as its reminder says it, at its time (`waking`: the
/// waking it waited for, else its time); "" when it is known.
fn doubt_of(knowledge: &Knowledge, due: i64, waking: i64, now: i64) -> String {
    doubt_unread(knowledge, due, waking, now, None)
}

/// The doubt, `first` said before what the devices leave unknown (a reminder
/// that comes late, one another device may have shown): "Sioul can't tell
/// whether it was taken: …. Check before taking it."; "" when nothing is.
/// What looks back counts from the dose's time, the opening for it from
/// `waking` (`sioul_core::health::doubts_at`; the sixth review's M1).
fn doubt_unread_with(knowledge: &Knowledge, due: i64, waking: i64, now: i64, missed: Option<&(i64, std::collections::BTreeSet<String>)>, first: Option<&str>) -> String {
    let doubts = match missed {
        Some((since, heard)) => sioul_core::health::doubts_unread_at(due, waking, now, knowledge.record_lost, &knowledge.peers, *since, heard),
        None => sioul_core::health::doubts_at(due, waking, now, knowledge.record_lost, &knowledge.peers),
    };
    // The devices' reasons, each said once: one `first` says already is not said again (L1).
    let parts: Vec<String> = why_parts(&doubts, tr()).into_iter().filter(|part| first.is_none_or(|first| !first.contains(part.as_str()))).collect();
    let reasons = (!parts.is_empty()).then(|| joined(parts, tr()));
    let why = match (first, reasons) {
        (None, None) => return String::new(),
        (Some(first), None) => first.to_string(),
        (None, Some(reasons)) => reasons,
        (Some(first), Some(reasons)) => joined(vec![first.to_string(), reasons], tr()),
    };
    say("dose-doubt", &[("why", why)])
}

/// A moment (Unix seconds) as a doubt says it (`when_said`).
fn moment_said(at: i64) -> String {
    Timestamp::from_second(at).map(|t| when_said(&t.to_zoned(TimeZone::system()), tr())).unwrap_or_default()
}

/// Whether a phone may remind the doses by its own alarm: one in the folder,
/// counted or not (silent a while, its alarms may still be set), or a device
/// whose entry does not say (an older Sioul, one not read): this computer
/// then keeps quiet around that alarm, promised (the sixth review's R3).
fn phone_alarms(knowledge: &Knowledge) -> bool {
    knowledge.peers.iter().any(|p| p.said.as_ref().is_none_or(|said| said.phone && !said.left))
}

/// The devices that must say they read this computer's opening before it
/// may be a dose's first (`first_here`): every phone in the folder that has
/// not stopped sharing and that you have not forgotten here, counted or not,
/// said off or not, silent or not (the seventh review's C1: what counts is
/// each device's own view, maybe stale); and every device whose entry does
/// not say what it is (an older Sioul, one not read), which never says it:
/// this computer is then never the first.
fn must_acknowledge(knowledge: &Knowledge) -> Vec<&Peer> {
    knowledge.peers.iter().filter(|p| !p.forgotten && p.said.as_ref().is_none_or(|said| said.phone && !said.left)).collect()
}

/// What each phone's sealed entry says of `computer`'s opening
/// (`devices::Entry::acked`), by phone.
fn acknowledgements(computer: &str, entries: &[sioul_sync::devices::Entry]) -> std::collections::BTreeMap<String, i64> {
    entries.iter().filter(|e| e.kind == sioul_sync::devices::PHONE).filter_map(|e| e.acked.get(computer).map(|opening| (e.id.clone(), *opening))).collect()
}

/// A phone's word on each computer it counts now (`Entry::acked`): the
/// opening of that computer's entry it read. Written by phones only, of
/// computers only: none elsewhere (the seventh review's L4). A computer said
/// off, forgotten or silent a week here goes from it (C1).
pub(crate) fn acknowledged_openings() -> std::collections::BTreeMap<String, i64> {
    if crate::devices::kind() != sioul_sync::devices::PHONE {
        return std::collections::BTreeMap::new();
    }
    let now = Timestamp::now().as_second();
    know()
        .peers
        .iter()
        .filter(|p| sioul_core::health::counts(p, now))
        // Its latest opening ever read: a word that never goes back (the eighth review's N3).
        .filter_map(|p| p.said.as_ref().filter(|s| !s.phone && s.started > 0).map(|s| (p.id.clone(), s.started.max(p.latest_opening))))
        .collect()
}

/// Since when every device that must (`must_acknowledge`) said in its sealed
/// entry that it read this computer's opening, `start`, as this process saw
/// it first: kept across ticks for the same opening; none while one has not.
static ACKED: std::sync::Mutex<Option<(i64, i64)>> = std::sync::Mutex::new(None);

/// How long a phone's word counts on this computer after this computer last
/// saw that phone's entry change (`Peer::seen`): twice the hour within which a
/// living phone restates its entry (`devices::RESTATED`), one restatement
/// missed allowed. Past it the phone may have taken its word back (said this
/// computer off, or stopped counting it after a week's silence) without this
/// computer hearing it: no word, this computer is not the first (the eighth
/// review's N1, N2).
const WORD_FRESH: i64 = 2 * sioul_sync::devices::RESTATED;

/// `ACKED` brought up to date at `now`; since when all acknowledged, if they do.
fn acknowledged_since(knowledge: &Knowledge, start: i64, acks: &std::collections::BTreeMap<String, i64>, now: i64) -> Option<i64> {
    let all = all_acknowledged(knowledge, start, acks, now);
    let mut acked = ACKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    *acked = acked_after(*acked, all, start, now);
    acked.map(|(_, at)| at)
}

/// Whether every device that must (`must_acknowledge`) said it read this
/// computer's opening `start`, that very one (`acks`: `acknowledgements`): a
/// word naming another opening, later or impossible, says nothing (M1); a
/// word of a phone not heard here within `WORD_FRESH` says nothing either
/// (N1, N2).
fn all_acknowledged(knowledge: &Knowledge, start: i64, acks: &std::collections::BTreeMap<String, i64>, now: i64) -> bool {
    start > 0 && must_acknowledge(knowledge).iter().all(|p| acks.get(&p.id) == Some(&start) && (0..=WORD_FRESH).contains(&(now - p.seen)))
}

/// `ACKED`'s next value: kept for the same opening while all still say it,
/// from `now` for a new one, none while one does not.
fn acked_after(before: Option<(i64, i64)>, all: bool, start: i64, now: i64) -> Option<(i64, i64)> {
    match before {
        Some((opening, at)) if all && opening == start => Some((opening, at)),
        _ if all => Some((start, now)),
        _ => None,
    }
}

/// How long before the dose's time the phones' word must be seen here: two
/// minutes (`SKEW`) and the most any of them was measured ahead of this
/// computer's clock (`Peer::ahead`): a phone ahead decides its alarm that
/// much earlier (the seventh review's L1).
fn word_lead(knowledge: &Knowledge) -> i64 {
    sioul_core::health::SKEW + must_acknowledge(knowledge).iter().map(|p| p.ahead.max(0)).max().unwrap_or(0)
}

/// Whether this computer may remind a dose to remind from `since` plainly, as
/// its first (docs/health.md, "What became of a reminder"; the sixth and
/// seventh reviews' C1): alone (sharing off, or no other device in the
/// folder); or its opening, its claim and its records written (`told`), the
/// doses kept here since before `since` (`kept_since`), and every phone's
/// word that it read this opening seen here `lead` before `since` (`acked`).
/// Else it is as a computer opened after the dose: quiet around a phone's
/// alarm, then the doubt first.
fn first_here(alone: bool, told: bool, kept_since: i64, acked: Option<i64>, since: i64, lead: i64) -> bool {
    alone || (told && kept_since <= since && acked.is_some_and(|at| at <= since - lead))
}

/// The same, at an alarm whose pull did not go through since it began
/// (`share::news_missed`: when, and the devices whose entry or health claim
/// says more since): any other known closed by what was read before is said in doubt,
/// as last heard (`sioul_core::health::doubts_unread`).
fn doubt_unread(knowledge: &Knowledge, due: i64, waking: i64, now: i64, missed: Option<&(i64, std::collections::BTreeSet<String>)>) -> String {
    let doubts = match missed {
        Some((since, heard)) => sioul_core::health::doubts_unread_at(due, waking, now, knowledge.record_lost, &knowledge.peers, *since, heard),
        None => sioul_core::health::doubts_at(due, waking, now, knowledge.record_lost, &knowledge.peers),
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

/// A device named by its id, from what is known here of the others: "the
/// phone" when it is the one phone, else its name; one not known, "another
/// device of yours".
fn named_by(knowledge: &Knowledge, id: &str) -> Named {
    let phones = knowledge.peers.iter().filter(|p| p.said.as_ref().is_some_and(|s| s.phone)).count();
    knowledge.peers.iter().find(|p| p.id == id).map_or_else(|| Named { id: id.to_string(), name: String::new(), phone: false }, |p| Named { id: p.id.clone(), name: p.name.clone(), phone: p.said.as_ref().is_some_and(|s| s.phone) && phones == 1 })
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

/// The health locks held at once, as a run of the doses would (`STATE`,
/// `TICKING`, `KNOWING`): what takes none of them finishes meanwhile (the
/// agenda's job, a test of it).
#[cfg(test)]
pub(crate) fn every_lock_held() -> [std::sync::MutexGuard<'static, ()>; 3] {
    [state_held(), TICKING.lock().unwrap_or_else(std::sync::PoisonError::into_inner), KNOWING.lock().unwrap_or_else(std::sync::PoisonError::into_inner)]
}

/// The health tests' files, one test at a time (`tests::health_files`); a
/// test elsewhere that watches them for writes takes it too (the agenda's
/// job's), so that a health test's own read is never counted as its write.
#[cfg(test)]
pub(crate) fn health_files_held() -> std::sync::MutexGuard<'static, ()> {
    static FILES: std::sync::Mutex<()> = std::sync::Mutex::new(());
    FILES.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One tick at a time: a slow one (a shared folder) is not overtaken by the next.
static TICKING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Whether the window may answer a dose now (decision 26 of the unified
/// agenda): today's, at any hour; one of a day to come is refused, since
/// marked it would leave the alarms a phone is given (`alarms_coming`) and its
/// reminder would not come. First in the window's dose writers (`not_taken`,
/// `taken_late`, `choose`, and the window's Taken before `set_taken`); the
/// alarms' own Taken keeps no day check (`set_taken`): a reminder can show a
/// minute before its dose, a minute before midnight. `now` given: the tests' clock.
pub(crate) fn may_answer(key: &str, now: &Zoned) -> Result<(), String> {
    let Some(due) = due_of(key) else { return Err(tr().text("dose-not-known", None)) };
    let today_ends = now.date().tomorrow().ok().and_then(|d| d.to_zoned(now.time_zone().clone()).ok()).map(|z| z.timestamp().as_second());
    // A dose due within a minute of now is answered as today's: its reminder,
    // and the question it opens, can come a minute before it, a minute before
    // midnight too (review of 36002ea, low 17); never a dose of tomorrow
    // answered all day long (the check's low 24).
    match today_ends {
        Some(end) if due >= end && due > now.timestamp().as_second() + 60 => Err(tr().text("dose-day-to-come", None)),
        _ => Ok(()),
    }
}

/// The window's Taken (or its taking back): a dose of a day to come refused
/// first when taking (`may_answer`; a taking back is never refused), then
/// `set_taken`, through which the alarms answer alone. Returns what went
/// wrong, else "" (what to know besides: `unmoved_note`).
pub(crate) fn window_taken(key: &str, taken: bool, now: &Zoned) -> String {
    if taken && let Err(problem) = may_answer(key, now) {
        return problem;
    }
    set_taken(key, taken)
}

/// What a Taken written says besides, when nothing reads here (the health
/// file nor its last read): its next doses could not be moved, if its
/// medicine moves them. With the last read, the move is recorded beside the
/// mark and made everywhere (`set_taken`): nothing to say.
pub(crate) fn unmoved_note() -> Option<String> {
    load_known().is_none().then(|| tr().text("dose-next-unmoved", None))
}

/// A dose marked taken already, Taken again at `at`: this device's TAKEN
/// answer anew, and its mark written again, a newer word than a take back
/// written elsewhere before it, which then never leaves the dose open
/// (review of 36002ea's check, must-fix 1). The same take said again (its
/// minute): its time kept, a second more, nothing moved. A later time said
/// (the late question, a time corrected): the take then, and for a medicine
/// taken every few hours, its one move made again from where it moved from,
/// to that time and its hours, the start it had moved to put back first on
/// every device (`DoseBacks`), so that every device follows, its file read
/// or not; a take back then puts the start back where it was (the check of
/// 1b6795e, H2). An earlier time: the take then, nothing moved: the next
/// doses come later, never sooner. Returns what went wrong, else "".
fn taken_again(key: &str, at: i64) -> String {
    let state = record();
    let marked = state.taken.get(key).copied();
    let now = Timestamp::now().as_second();
    // Times said to the minute: the mark's own minute is the same take.
    let same = marked.is_some_and(|then| at.div_euclid(60) == then.div_euclid(60));
    // A later time: the dose's move made again, where the file reads, else recorded alone.
    let later = marked.is_some_and(|then| !same && at > then);
    let at = match marked {
        Some(then) if same => then + 1,
        _ => at,
    };
    let step = load_known().and_then(|(health, _)| health.step_of(key));
    let again: Option<([i64; 2], Option<i64>)> = match (later, step, state.moved.get(key).copied()) {
        (true, Some(step), Some([before, after])) if at + step != after => {
            let (read, backs) = (HealthState::read(&HealthState::default_path()).ok(), backs());
            let _ = change_health(|health| {
                // Mended only from a record that reads, as `load_known` mends (the A4 check, #11).
                if let Some(read) = &read {
                    health.mend_shifts(read, &backs);
                }
                Ok(health.moved_on(key, after, at + step))
            });
            Some(([before, at + step], Some(after)))
        }
        // Taken at its very time first (nothing moved then): moved now, as a first take moves.
        (true, Some(_), None) => match moved_by(key, at) {
            Ok(Some((before, after))) => Some(([before, after], None)),
            Ok(None) => None,
            Err(e) => return e,
        },
        _ => None,
    };
    if let Some(([before, _], Some(replaced))) = again {
        restore_everywhere(key, before, replaced, now);
    }
    let problem = change(|state| {
        state.taken.insert(key.to_string(), at);
        state.not_taken.remove(key);
        if let Some((moved, _)) = again {
            state.moved.insert(key.to_string(), moved);
        }
    });
    if !problem.is_empty() {
        return problem;
    }
    record_answer(key, sioul_core::doses::TAKEN, at, false)
}

/// A dose not taken, said afterwards: asked no more. Returns what went wrong, else "".
pub(crate) fn not_taken(key: &str) -> String {
    if let Err(problem) = may_answer(key, &Zoned::now()) {
        return problem;
    }
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
    if let Err(problem) = may_answer(key, &Zoned::now()) {
        return problem;
    }
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
        restore_everywhere(key, before, after, now);
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
    if let Err(problem) = may_answer(key, &now) {
        return problem;
    }
    let Some(clock) = time.trim().split_once(':').and_then(|(h, m)| jiff::civil::Time::new(h.trim().parse().ok()?, m.trim().parse().ok()?, 0, 0).ok()) else {
        return tr().text("dose-time-wrong", None);
    };
    let mut at = now.date().to_datetime(clock).to_zoned(now.time_zone().clone()).map(|z| z.timestamp().as_second()).unwrap_or(now.timestamp().as_second());
    if at > now.timestamp().as_second() {
        at -= 86_400;
    }
    let _held = state_held();
    // Marked already (the phone's "Taken at 08:03" line tapped later, or a
    // time corrected): the mark written again at the time said, and this
    // device's answer anew, no second move (review of 36002ea, can-wait 1;
    // its check, must-fix 1).
    if HealthState::read(&HealthState::default_path()).is_ok_and(|state| state.taken.contains_key(key)) {
        return taken_again(key, at);
    }
    let moved = match moved_by(key, at) {
        Ok(moved) => moved,
        Err(e) => return e,
    };
    // Taken anew: a move taken back before is passed.
    restores_passed(key, Timestamp::now().as_second());
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

/// The next doses a Taken at `at` moves (a medicine taken every few hours:
/// `Health::taken_at`), where they started before and after: moved in the
/// health file as it holds them now, under its lock. A file that does not
/// read is never written over: the move is made on its last read, mended
/// from the record (`load_known`), and recorded beside the mark alone
/// (`HealthState::moved`), so that each device whose file reads makes it
/// at its next minute (`Health::mend_shifts`) and this one shows it
/// (review of 36002ea's check, must-fix 3). Nothing read at all: none.
fn moved_by(key: &str, at: i64) -> Result<Option<(i64, i64)>, String> {
    // Judged on the schedule this device rings by: the file mended first from
    // the record (a move made on another device whose file did not read lives
    // there alone), and written so (the chain check of e3505f7, must-fix 2:
    // a Taken pressed with Sioul closed was checked on the file as written,
    // and moved nothing). Only from a record that reads, as `load_known`
    // mends: a record repaired to nothing would put back every take back's
    // start over moves still marked (the A4 check, #11).
    let (read, backs) = (HealthState::read(&HealthState::default_path()).ok(), backs());
    match change_health(|health| {
        if let Some(read) = &read {
            health.mend_shifts(read, &backs);
        }
        Ok(health.taken_at(key, at))
    }) {
        Ok(moved) => Ok(moved),
        Err(_) if Health::read(&Health::default_path()).is_err() => Ok(load_known().and_then(|(mut copy, _)| copy.taken_at(key, at))),
        Err(e) => Err(e),
    }
}

/// What the late dose's question shows, as JSON: {name, dose, due, now, hours}.
pub(crate) fn dose_info(key: &str) -> String {
    let known = load_known();
    let now = Zoned::now();
    let (Some((id, _)), Some(due)) = (key.rsplit_once('@'), due_of(key)) else { return "null".into() };
    let medicine = known.as_ref().and_then(|(health, _)| health.medicines.iter().find(|m| m.id == id));
    // Its medicine taken out: no question. Nothing read here at all (the file
    // nor its last read): asked all the same, from the key alone, so that a
    // late Taken is never lost (review of 36002ea's check, must-fix 5).
    if known.is_some() && medicine.is_none() {
        return "null".into();
    }
    let due = Timestamp::from_second(due).map(|t| t.to_zoned(now.time_zone().clone())).unwrap_or_else(|_| now.clone());
    let hours = match medicine.map(|m| &m.schedule) {
        Some(Schedule::Hours { hours, .. }) => *hours,
        _ => 0,
    };
    let dose = match (&known, medicine) {
        (Some((health, _)), Some(medicine)) => health.amount_of(key, now.time_zone()).unwrap_or_else(|| medicine.dose.clone()),
        _ => String::new(),
    };
    serde_json::json!({
        "name": medicine.map_or_else(|| id.to_string(), |m| m.short_name()),
        "dose": dose,
        "due": if due.date() == now.date() { due.strftime("%H:%M").to_string() } else { format!("{} {}", tr().weekday_short(due.date()), due.strftime("%H:%M")) },
        "now": now.strftime("%H:%M").to_string(),
        // Every few hours: the next dose comes that many hours after the one taken.
        "hours": hours,
        // Marked taken already (the phone's "Taken at 08:03" line tapped later): when, said
        // beside the Taken that writes it again, a time corrected too (review of 36002ea,
        // can-wait 1; its check, must-fix 1); "" when not.
        "taken": HealthState::read(&HealthState::default_path()).ok().and_then(|state| state.taken.get(key).copied()).and_then(|at| Timestamp::from_second(at).ok()).map(|at| {
            let at = at.to_zoned(now.time_zone().clone());
            if at.date() == now.date() { at.strftime("%H:%M").to_string() } else { format!("{} {}", tr().weekday_short(at.date()), at.strftime("%H:%M")) }
        }).unwrap_or_default(),
        // Its time alone ("08:03"), the time field's start for a dose marked
        // already: pressed as it is, the same take, nothing rewritten (the
        // check of 1b6795e, M4); "" when not marked.
        "taken_at": HealthState::read(&HealthState::default_path()).ok().and_then(|state| state.taken.get(key).copied()).and_then(|at| Timestamp::from_second(at).ok()).map(|at| at.to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string()).unwrap_or_default(),
    })
    .to_string()
}

/// A dose marked taken, or not, as it happens: taken every few hours, the
/// next doses come that many hours after now (`moved_by`: recorded beside
/// the mark, made on every device); a mark taken back puts them back, the
/// move kept as taken back (`DoseBacks`, health-back.toml) for every
/// device to put back, its file read here or not (review of 36002ea's
/// check, must-fix 4). Taken again, a dose already marked taken: its mark
/// and this device's answer written anew, no second move (`taken_again`).
/// Returns what went wrong, else "".
pub(crate) fn set_taken(key: &str, taken: bool) -> String {
    let _held = state_held();
    let now = Timestamp::now().as_second();
    // Marked already: the same take said again, its time kept (`taken_again`).
    if taken && let Some(then) = HealthState::read(&HealthState::default_path()).ok().and_then(|state| state.taken.get(key).copied()) {
        return taken_again(key, then);
    }
    let shift = if taken {
        match moved_by(key, now) {
            Ok(shift) => shift,
            Err(e) => return e,
        }
    } else {
        None
    };
    let moved = std::sync::Mutex::new(None);
    if taken {
        // Taken anew: a move taken back before is passed.
        restores_passed(key, now);
    }
    let problem = change(|state| {
        if taken {
            state.taken.insert(key.to_string(), now);
            if let Some((before, after)) = shift {
                state.moved.insert(key.to_string(), [before, after]);
            }
        } else {
            state.taken.remove(key);
            if let Some(moved_then) = state.moved.remove(key)
                && let Ok(mut moved) = moved.lock()
            {
                *moved = Some(moved_then);
            }
        }
    });
    // Put back here at once where the file reads, and on every device by its
    // word (`DoseBacks`), its file read or not where it was taken back.
    if let Some([before, after]) = moved.into_inner().ok().flatten() {
        restore_everywhere(key, before, after, now);
        let _ = change_health(|health| Ok(health.taken_back(key, before, after)));
    }
    // Not marked: the doses its take moved go back, as `taken_late` does (review of 36002ea, low 19).
    if !problem.is_empty()
        && let Some((before, after)) = shift
    {
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

/// `date`'s held times, what its meals move past: today's from the minute's
/// cache; another day's as the plan placed that day's meals (`work::held_on`:
/// its own events and the blocks tasks are pinned to), so that "15 min later"
/// moves a meal from where every view shows it.
fn held_on(date: Date, now: &Zoned) -> Vec<(i64, i64)> {
    if date == now.date() {
        return held_today(now);
    }
    crate::work::held_on(date, now)
}

/// A day's own blocks as they are: its changes, its meals pushed past its events.
fn blocks_now(needs: &Needs, days: &Days, date: Date, now: &Zoned) -> Vec<Kept> {
    let pushed = needs.past_events_on(date, now.time_zone(), days, &held_on(date, now));
    needs.blocks_of(date, now.time_zone(), days, &|key: &str| pushed.get(key).copied().unwrap_or(0))
}

/// The needs' notices that may come at `now`, each its block and whether it
/// is the heads-up, marked given on this device (`Today::due`): two at most a
/// block, each once, only near its time; none while an event goes on, none
/// for a block quiet or taken out that day; as the matrix of what reaches you
/// lets them come (as usual, nothing while you sleep, in a pause or in Free
/// time). The window's minute on a computer and a phone's alarm at the
/// night's start both ask here (`needs_tick`, `decide_night`).
fn needs_now(health: &Health, now: &Zoned) -> Vec<(Kept, bool)> {
    let needs = &health.needs;
    let days = days();
    if !(needs.meals_on || needs.naps_on || needs.sleep_on) && !days.0.contains_key(&now.date()) {
        return Vec::new();
    }
    let path = sioul_core::needs::Today::default_path();
    let mut today = sioul_core::needs::Today::load(&path, now.date());
    let stamp = now.timestamp().as_second();
    let pushed = needs.past_events_on(now.date(), now.time_zone(), &days, &held_today(now));
    let kept = needs.kept_with(now.date(), now.time_zone(), &days, &|key: &str| pushed.get(key).copied().unwrap_or(0));
    let due: Vec<(Kept, bool)> = today.due(&kept, needs.heads_up, stamp).into_iter().map(|(block, heads_up)| (block.clone(), heads_up)).collect();
    if due.is_empty() {
        return Vec::new();
    }
    let _ = today.save(&path);
    // In a meeting, nothing is said: the time stays kept.
    let meeting = sioul_core::agenda::occurrences(stamp - 86_400, stamp + 60).iter().any(|e| !e.cancelled && !e.all_day && e.start <= stamp && e.end > stamp);
    if meeting {
        return Vec::new();
    }
    let attention = crate::hours::attention();
    let moment = crate::hours::attention_now();
    let row = sioul_core::attention::Row::Own(sioul_core::attention::Kind::Needs);
    let may = attention.level(row, &moment) == sioul_core::attention::Level::Now;
    // The night's or a nap's own notice as it begins says that sleep begins:
    // sleep's column does not hold it (docs/health.md); a pause does.
    let mut awake = moment.clone();
    awake.times.retain(|c| *c != sioul_core::attention::Column::Sleep);
    let starting = attention.level(row, &awake) == sioul_core::attention::Level::Now;
    due.into_iter()
        .filter(|(block, heads_up)| {
            let own_start = !heads_up && (block.kind == "sleep" || block.kind == "nap") && block.start <= stamp;
            may || (own_start && starting)
        })
        .collect()
}

/// The night's own notice's buttons (docs/reviews.md): "Close the whole
/// day" while that night's review is to give at `now`, its press carrying
/// the night's day ("night:2026-10-14"), then "Options…", the block's
/// question.
fn night_offered(block: &Kept, now: &Zoned) -> Vec<crate::reviews::Offered> {
    let options = crate::reviews::Offered { label: tr().text("need-open", None), kind: "need", key: block.key.clone() };
    match sioul_core::reviews::night_day(block.at, now.time_zone()).filter(|_| crate::reviews::night_open(block.at, now)) {
        Some(day) => vec![crate::reviews::Offered { label: tr().text("review-close-night", None), kind: "review", key: format!("night:{day}") }, options],
        None => vec![options],
    }
}

/// Each minute, from the computer you are at (`needs_now`): a block's
/// heads-up about the work, `heads_up` minutes before it ("No new big
/// task"), with "Later"; then one at its time. Its name and time only: safe
/// to be read by someone else (docs/health.md). A phone shows only the
/// night's own, at its alarm (`decide_night`).
fn needs_tick(qt: &QtThread, shared: &Arc<Shared>, health: &Health, now: &Zoned) {
    if cfg!(target_os = "android") {
        return;
    }
    let hm = |at: i64| clock(at, now.time_zone());
    for (block, heads_up) in needs_now(health, now) {
        let name = name_of(&block);
        // One button: the window's question (later, at another time, not
        // today; a line on where you stopped).
        let (key, qt_open) = (block.key.clone(), qt.clone());
        let open: Box<dyn FnOnce() + Send> = Box::new(move || {
            let _ = qt_open.queue(move |mut sioul| sioul.as_mut().reminder_opened(QString::from("need"), QString::default(), QString::from(&key)));
        });
        let action = Some((tr().text("need-open", None), open));
        let shown = if heads_up {
            sioul_sync::notify::remind(&tr().text("need-heads-up", None), &say("need-at", &[("name", name), ("time", hm(block.start))]), action)
        } else if block.kind == "sleep" {
            // The night's own notice: closing the day is offered beside its
            // options while that night's review is to give, as each system
            // shows buttons (docs/reviews.md).
            crate::reviews::show_notice(qt, &name, &hm(block.start), night_offered(&block, now))
        } else {
            sioul_sync::notify::remind(&name, &hm(block.start), action)
        };
        if let Err(e) = shown {
            tell(qt, shared, e);
        }
    }
}

/// When the night's own notice may come in the next nights, for a phone's
/// alarms (`eventalarms::coming`): as each night's wind-down begins, while
/// sleep is kept with its notices, and in the quarter of an hour after, as
/// `Today::due` gives it, unless given here already; each
/// (`day:night:<date>`, Unix seconds). A start just past stays as it was, so
/// that the list stays the same from one minute to the next: Android rings
/// it at once.
pub(crate) fn coming_night(now: &Zoned) -> Vec<(String, i64)> {
    let stamp = now.timestamp().as_second();
    let today = sioul_core::needs::Today::load(&sioul_core::needs::Today::default_path(), now.date());
    crate::hours::blocks(now)
        .kept
        .iter()
        .filter(|block| block.kind == "sleep" && block.notices && stamp - block.start < 15 * 60)
        .filter_map(|block| {
            let date = Timestamp::from_second(block.start).ok()?.to_zoned(now.time_zone().clone()).date();
            let given = date == now.date() && today.sent.contains(&format!("{}:start", block.key));
            (!given).then(|| (format!("day:night:{date}"), block.start))
        })
        .collect()
}

/// A phone's alarm as the night's wind-down begins (`eventalarms`,
/// "day:night:…"): the night's own notice as a computer's minute says it
/// (`needs_now`), from the device you are at as who keeps the reminders
/// reads, never claimed from there (`share::looked`). Shown in Android's
/// notifications with "Close the whole day" while that night's review is to
/// give, and "Options…", each opening Sioul on its sheet or its question.
/// {"shown": true}; {"again_at": ms} rung early, or kept here only lately;
/// {} when it does not come.
pub(crate) fn decide_night(now: &Zoned) -> serde_json::Value {
    let stamp = now.timestamp().as_second();
    let starts: Vec<i64> = crate::hours::blocks(now).kept.iter().filter(|block| block.kind == "sleep").map(|block| block.start).collect();
    if let Some(start) = starts.iter().copied().filter(|start| *start > stamp).min()
        && start - stamp < 3600
    {
        return serde_json::json!({ "again_at": start * 1000 });
    }
    // Past the quarter of an hour its notice may come in (`Today::due`): nothing, nor asked again.
    if !starts.iter().any(|start| (0..15 * 60).contains(&(stamp - start))) {
        return serde_json::json!({});
    }
    let keeper = crate::share::looked("health", sioul_sync::lease::Rule::FollowsYou);
    if !keeper.mine {
        return serde_json::json!({});
    }
    if !keeper.settled {
        return serde_json::json!({ "again_at": (stamp + 60) * 1000 });
    }
    let Some(block) = needs_now(&load(), now).into_iter().find(|(block, heads_up)| !heads_up && block.kind == "sleep").map(|(block, _)| block) else {
        return serde_json::json!({});
    };
    let date = Timestamp::from_second(block.start).map(|t| t.to_zoned(now.time_zone().clone()).date()).unwrap_or(now.date());
    let offered = night_offered(&block, now);
    crate::eventalarms::day_note(&format!("day:night:{date}"), &name_of(&block), &clock(block.start, now.time_zone()), &offered, block.end.min(stamp + crate::reviews::PHONE_NOTICE_KEPT));
    serde_json::json!({ "shown": true })
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
    change_day_seen(date, edit).err().unwrap_or_default()
}

/// `change_day`, and its block as it was and as the change left it, both
/// read under the file's lock with the change itself: what a drag's Undo
/// puts back is the drag's alone, never a change an exchange wrote between
/// (the check of 36002ea, low 21). Returns the day, or what went wrong.
fn change_day_seen(date: &str, edit: &DayEdit) -> Result<(Date, Option<sioul_core::needs::DayBlock>, Option<sioul_core::needs::DayBlock>), String> {
    let now = Zoned::now();
    let date = if date.trim().is_empty() { Some(now.date()) } else { date.trim().parse::<Date>().ok() };
    let Some(date) = date else { return Err(tr().text("need-times-wrong", None)) };
    // Changed over the days as the file holds them now, under its lock, which the sharing takes too.
    let _ = days();
    let needs = load().needs;
    let held = held_on(date, &now);
    let (changed, was, wrote) = Days::change_file(&Days::default_path(), now.date(), |days| {
        let was = days.get(date, &edit.key).cloned();
        let changed = needs.change_day(days, date, edit, &now, &held);
        (changed, was, days.get(date, &edit.key).cloned())
    })?;
    match changed {
        Ok(()) => {
            // A night changed, or its alarm: a phone's alarm at waking follows (`wake`).
            crate::wake::schedule();
            Ok((date, was, wrote))
        }
        Err(DayProblem::Past) => Err(tr().text("need-past", None)),
        Err(DayProblem::Times) => Err(tr().text("need-times-wrong", None)),
        Err(DayProblem::TooShort) => Err(tr().text("need-too-short", None)),
        Err(DayProblem::OtherDay) => Err(tr().text("need-other-day", None)),
        Err(DayProblem::Over) => Err(tr().text("need-over", None)),
        Err(DayProblem::Unknown) => Err(say("setting-unknown-key", &[("key", edit.action.clone())])),
    }
}

/// A day's block moved by a drag on the page's timeline (`change_need`'s
/// JSON: "move", or "times" for one of its ends), changed as "Move to…" and
/// "Change its times…" change it, then "Undo" offered for ten seconds, which
/// puts back what the drag wrote and only that (`undo_drag`): a choice made
/// since on the same block, the night's alarm included, stays. Dropped where
/// it was: nothing changes, nothing to undo. Returns what went wrong, else "".
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
    let key = asked.edit.key.clone();
    let (date, was, wrote) = match change_day_seen(&asked.date, &asked.edit) {
        Ok(seen) => seen,
        Err(problem) => return problem,
    };
    if wrote == was {
        return String::new();
    }
    let after = days();
    // Said as the day has it now: its name, its time kept.
    let zone = now.time_zone().clone();
    let line = blocks_now(&load().needs, &after, date, &now)
        .iter()
        .find(|k| k.key == key)
        .map(|k| say("drag-done", &[("what", name_of(k)), ("time", format!("{}–{}", clock(k.start, &zone), clock(k.end, &zone)))]))
        .unwrap_or_else(|| tr().text("drag-done-plain", None));
    crate::mail::offer_back(qt, shared, line, move |qt, shared| {
        let problem = undo_drag(date, &key, was.as_ref(), wrote.as_ref());
        crate::work::show_work(qt, shared);
        show_health(qt, shared);
        problem
    });
    String::new()
}

/// A drag's Undo (`drag_need`): what it wrote on that day's block (`was`
/// before it, `wrote` after it) put back, field by field, over the days as
/// the file holds them now, under its lock, which the sharing takes too; a
/// field changed since stays (`Days::put_back`). Nothing left to put back:
/// "too late". Returns what went wrong, else "".
fn undo_drag(date: Date, key: &str, was: Option<&sioul_core::needs::DayBlock>, wrote: Option<&sioul_core::needs::DayBlock>) -> String {
    let _ = days();
    let problem = match Days::change_file(&Days::default_path(), Zoned::now().date(), |days| days.put_back(date, key, was, wrote)) {
        Ok(true) => String::new(),
        Ok(false) => tr().text("undo-too-late", None),
        Err(e) => e,
    };
    // A night put back: a phone's alarm at waking follows (`wake`).
    crate::wake::schedule();
    problem
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

// ---------------------------------------------------------------- what each device did to remind a dose

/// What each device did to remind each dose (`sioul_core::dose_reminders`),
/// as read here; none when the file cannot be trusted (repaired first, the
/// doses due in the day before and in the rest of that day then said not
/// known: `repair_told`). None is
/// never read as "nothing done": a phone's alarm then says the doubt.
fn told() -> Option<DoseReminders> {
    let path = DoseReminders::default_path();
    match DoseReminders::read(&path) {
        Ok(told) => Some(told),
        Err(_) => {
            repair_told();
            DoseReminders::read(&path).ok()
        }
    }
}

/// A reminders' file that cannot be trusted, as `repair_records` does the
/// records: read again from every device's at the next exchange, kept aside
/// in the same step; the doses due in the day before and in the rest of that
/// day said not known (`write_record_doubt`).
fn repair_told() {
    let now = Timestamp::now().as_second();
    let path = DoseReminders::default_path();
    let set_aside = || sioul_core::health::HealthState::set_aside(&path, now);
    let aside = if crate::share::on() {
        match crate::share::rebuild_dose_reminders(set_aside) {
            Ok(aside) => aside,
            Err(_) => {
                write_record_doubt(now, "");
                return;
            }
        }
    } else {
        set_aside()
    };
    write_record_doubt(now, &aside.ok().flatten().map(|p| p.display().to_string()).unwrap_or_default());
}

/// This computer's entry for the dose `key` changed by `change`; false when
/// it could not be written.
fn tell_mine(key: &str, change: impl FnOnce(&mut Tried)) -> bool {
    let me = this_device();
    let now = Timestamp::now().as_second();
    let written = DoseReminders::update(&DoseReminders::default_path(), now, &me, |all| change(all.entry(key, &me))).is_ok();
    if written {
        crate::devices::recorded();
    }
    written
}

/// This computer promises no notice of the dose `key` until `until`
/// (`dose_reminders::NEVER`: none again); a longer promise kept.
fn promise(key: &str, until: i64) -> bool {
    tell_mine(key, |mine| mine.quiet = mine.quiet.max(until))
}

/// What this computer does for a dose at its tick (docs/health.md, "What
/// became of a reminder"; review of battery part C, the fifth review).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// Nothing now: shown, promised quiet, or not this computer's to remind.
    Not,
    /// Its first reminder, plainly unless the devices leave a doubt: this
    /// computer kept the doses at the dose's time, and nothing of it was
    /// recorded here before.
    First,
    /// A phone's alarm due: promised quiet until then, nothing now.
    Quiet(i64),
    /// A reminder shown late, the doubt first: its first try did not go out
    /// (`Late::Retry`), went and was not known shown (`Late::Unconfirmed`,
    /// by an earlier Sioul here), or another device may have reminded it
    /// (`None`: this computer did not keep the doses then).
    Late(Option<sioul_sync::notify::Late>),
}

/// How long this computer's first reminder of a dose may wait for news, its
/// window as its first: until three minutes before a phone's alarm while a
/// phone shares the doses (`LATE_UNTIL`), else ten minutes (`WAIT_FOR_NEWS`).
fn first_wait(alarms: bool) -> i64 {
    if alarms { LATE_UNTIL } else { WAIT_FOR_NEWS }
}

/// The plan for a dose to remind from `since` (its time, or the waking it
/// waited for), at `stamp`: `own`, this computer's entry for it (`read`: the
/// file read here); `first_ok`, whether this computer may be its first
/// (`first_here`: the phones' word on its opening, before `since`); `alarms`,
/// a phone may remind it by its own alarm ten minutes on: none from here
/// around it, promised. The rule: a first reminder is plain only where it is
/// the dose's first; every other says the doubt first; none twice in the
/// same words.
fn plan(own: Option<Tried>, read: bool, first_ok: bool, since: i64, stamp: i64, alarms: bool) -> Plan {
    use sioul_sync::notify::Late;
    let first = first_ok && stamp < since + first_wait(alarms);
    let quiet = alarms && stamp < since + sioul_core::dose_reminders::QUIET_AFTER;
    // The file not read: a first try, else quiet around a phone's alarm (nothing
    // promised can be read or written here), then the doubt first (the sixth review's L1).
    if !read {
        return match own {
            None if first => Plan::First,
            _ if quiet => Plan::Quiet(since + sioul_core::dose_reminders::QUIET_AFTER),
            _ => Plan::Late(None),
        };
    }
    match own {
        Some(own) if own.shown > 0 || own.quiet >= stamp => Plan::Not,
        Some(own) if own.maybe > 0 || own.unshown > 0 => {
            if quiet && stamp >= since + LATE_UNTIL {
                Plan::Quiet(since + sioul_core::dose_reminders::QUIET_AFTER)
            } else if own.maybe > 0 {
                Plan::Late(Some(Late::Unconfirmed))
            } else {
                Plan::Late(Some(Late::Retry))
            }
        }
        // A promise of its own, over: not its first.
        Some(_) if quiet => Plan::Quiet(since + sioul_core::dose_reminders::QUIET_AFTER),
        Some(_) => Plan::Late(None),
        None if first => Plan::First,
        None if quiet => Plan::Quiet(since + sioul_core::dose_reminders::QUIET_AFTER),
        None => Plan::Late(None),
    }
}

/// Whether this computer keeps the doses now, as the claims read.
fn kept_here() -> bool {
    crate::share::looked("health", sioul_sync::lease::Rule::FollowsYou).mine
}

/// The doses kept elsewhere a moment: this dose's tries end here for now,
/// nothing promised (never "none again": the sixth review's M3), and it is
/// taken out of this session's sent ones, so that the tick works its plan
/// out again should the doses come back here within its grace.
fn let_go(key: &str) {
    if let Ok(mut sent) = SENT.lock() {
        sent.remove(key);
    }
}

/// Whether a dose is still to remind, as read here now: neither marked nor
/// answered anywhere, nor reminded anywhere (the marks' record, or a
/// device's notice shown).
fn still_to_remind(key: &str) -> bool {
    let _held = state_held();
    let state = record();
    late_reminder_wanted(&state, &records(), key) && !told_here(&state).is_some_and(|told| told.shown(key))
}

/// The words of a dose's reminder shown late on this computer, the doubt
/// first, always (`late_said`); none when it is no longer to remind
/// (`still_to_remind`), or its grace is over (`GRACE_MINUTES` from `since`:
/// its question on the past takes it up).
fn late_words(key: &str, since: i64, why: Option<sioul_sync::notify::Late>) -> Option<(String, String)> {
    if !still_to_remind(key) {
        return None;
    }
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    if stamp >= since + GRACE_MINUTES * 60 {
        return None;
    }
    let start = now.checked_sub(Span::new().hours(MISSED_HOURS)).unwrap_or_else(|_| now.clone());
    let end = now.checked_add(Span::new().seconds(1)).unwrap_or_else(|_| now.clone());
    let dose = load().doses(&start, &end).into_iter().find(|d| d.key == key)?;
    let (title, body) = late_said(&dose, &know(), stamp, since, why);
    Some(said_back(&dose, &record(), &now, title, body))
}

/// A late reminder's words: "Check first: …", then its time and why Sioul
/// cannot tell, why it is late first: its first try could not be shown in
/// time (`Late::Retry`), it was shown and not confirmed in time
/// (`Late::Unconfirmed`), or another device may have reminded it (`None`);
/// then what the devices leave unknown, from `since`.
fn late_said(dose: &sioul_core::health::Dose, knowledge: &Knowledge, now: i64, since: i64, why: Option<sioul_sync::notify::Late>) -> (String, String) {
    use sioul_sync::notify::Late;
    let due = dose.at.timestamp().as_second();
    let late = match why {
        Some(Late::Retry) => say("dose-doubt-late", &[("when", moment_said(due))]),
        Some(Late::Unconfirmed) => say("dose-doubt-unconfirmed", &[("when", moment_said(due))]),
        None => say("dose-doubt-other", &[("when", moment_said(since))]),
    };
    let doubt = doubt_unread_with(knowledge, due, since, now, None, Some(&late));
    (say("dose-check-title", &[("dose", named(dose))]), format!("{}. {doubt}", dose.at.strftime("%H:%M")))
}

/// A dose's reminder as this computer records it, for the notice's thread
/// (`sioul_sync::notify::DoseRecord`): this device's entry in the dose's
/// reminders, written before each notice goes, and the marks' record for an
/// older Sioul (`reminded`, `unshown`).
struct DoseNotes {
    key: String,
    /// When it was to be reminded: its time, or the waking it waited for.
    since: i64,
    /// The entry's `maybe` before the notice last recorded going: put back if refused.
    before: Option<i64>,
}

impl sioul_sync::notify::DoseRecord for DoseNotes {
    fn going(&mut self, going: sioul_sync::notify::Going) -> sioul_sync::notify::Say {
        use sioul_sync::notify::{Going, Say};
        // The first: the tick decided it a moment ago (its record's lock is
        // its own until it waits for this outcome: not taken again here).
        let words = match going {
            Going::First => Some(None),
            // The doses kept elsewhere now: let go, nothing promised (M3).
            Going::Late(_) if !kept_here() => {
                let_go(&self.key);
                return Say::Nothing;
            }
            Going::Late(why) => late_words(&self.key, self.since, Some(why)).map(Some),
        };
        let Some(words) = words else {
            promise(&self.key, sioul_core::dose_reminders::NEVER);
            return Say::Nothing;
        };
        let now = Timestamp::now().as_second();
        let mut before = 0;
        let written = tell_mine(&self.key, |mine| {
            before = mine.maybe;
            mine.maybe = now;
        });
        // Not recorded: it goes all the same (late, the doubt first): a phone
        // reads no word of this computer's on the dose, which is a doubt
        // there (the sixth review's L1).
        self.before = written.then_some(before);
        match words {
            None => Say::Own,
            Some((title, body)) => Say::Words(title, body),
        }
    }

    fn refused(&mut self) {
        if let Some(before) = self.before.take() {
            tell_mine(&self.key, |mine| mine.maybe = before);
        }
    }

    fn shown(&mut self) {
        let now = Timestamp::now().as_second();
        tell_mine(&self.key, |mine| {
            if mine.shown == 0 {
                mine.shown = now;
            }
        });
        let _held = state_held();
        let _ = change(|record| late_reminded(record, &self.key, now));
    }

    fn unshown(&mut self) {
        let now = Timestamp::now().as_second();
        tell_mine(&self.key, |mine| {
            if mine.unshown == 0 {
                mine.unshown = now;
            }
        });
        let _held = state_held();
        let _ = change(|record| {
            if !record.reminded_now(&self.key) {
                record.unshown.entry(self.key.clone()).or_insert(now);
            }
        });
    }

    fn pause(&mut self, until: i64) -> bool {
        if !kept_here() {
            let_go(&self.key);
            false
        } else if still_to_remind(&self.key) {
            // Promised, or not written: quiet all the same (the thread sleeps through it).
            promise(&self.key, until);
            true
        } else {
            promise(&self.key, sioul_core::dose_reminders::NEVER);
            false
        }
    }

    fn over(&mut self) {
        promise(&self.key, sioul_core::dose_reminders::NEVER);
    }

    fn settled(&mut self) -> bool {
        let _held = state_held();
        let state = record();
        state.taken.contains_key(&self.key) || state.not_taken.contains_key(&self.key) || sioul_core::doses::answered(&self.key, &state, &records()) != Answered::Open
    }
}

/// Whether a dose's reminder may still be shown: neither marked here (taken,
/// or said not taken) nor answered on another device, nor reminded since
/// (another device took it up: a phone's alarm rang): what the phone did is
/// never contradicted, and nothing is ever said of it but a reminder.
fn late_reminder_wanted(state: &HealthState, records: &DoseRecords, key: &str) -> bool {
    !state.taken.contains_key(key) && !state.not_taken.contains_key(key) && !state.reminded_now(key) && sioul_core::doses::answered(key, state, records) == Answered::Open
}

/// `shown_late`'s record: a reminder at `now`, unless reminded or marked
/// meanwhile (the earlier word kept); the `unshown` of its first try stays,
/// a reminder outweighing it (`HealthState::reminder_unshown`).
fn late_reminded(record: &mut HealthState, key: &str, now: i64) {
    if !record.reminded_now(key) && !record.taken.contains_key(key) && !record.not_taken.contains_key(key) {
        record.reminded.insert(key.to_string(), now);
        record.taken_away.remove(key);
    }
}

/// Each minute: a dose due is reminded once, quietly, with "Taken"; a refill
/// or a renewal coming becomes a task in a list kept on this computer.
/// How long a gap between two of this computer's ticks (each minute) makes
/// its coming back a new opening (`devices::resumed`): three minutes, two
/// ticks and their exchanges missed (asleep, the machine frozen), so that a
/// laptop woken has no phone's word on it until a phone read it awake (the
/// seventh review's C1).
const WAKE_GAP: i64 = 3 * 60;

/// This computer's last tick (Unix seconds; 0 none yet).
static LAST_TICK: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Whether a tick at `now`, after one at `last`, comes back from a gap
/// (`WAKE_GAP`); never the first tick, nor a clock set back.
fn woke_from_gap(last: i64, now: i64) -> bool {
    last > 0 && now - last >= WAKE_GAP
}

pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {
    // Back from a gap (asleep): a new opening, its word asked anew.
    let now_tick = Timestamp::now().as_second();
    if !cfg!(target_os = "android") && woke_from_gap(LAST_TICK.swap(now_tick, std::sync::atomic::Ordering::SeqCst), now_tick) {
        crate::devices::resumed();
    }
    let Some(_ticking) = crate::backend::one_at_a_time(&TICKING) else { return };
    let (health, last_known) = load_said();
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
    let (keeper, claim_failed) = crate::share::keeper("health", sioul_sync::lease::Rule::FollowsYou, active, false);
    let _held = state_held();
    let state = record();
    // A dose taken late moved the next ones; the medicine, changed meanwhile
    // on another device, came back with its start as it was: moved again,
    // and sent (review of 5 October 2026, F23).
    // Its write refused (the file does not read here): the mended copy kept all the same (review of 36002ea's check, must-fix 2).
    // The file's own words mended and written, once (`load_known` mends in memory everywhere).
    let backs = backs();
    if let Ok(mut own) = Health::read(&Health::default_path())
        && own.mend_shifts(&state, &backs)
    {
        let _ = change_health(|h| Ok(h.mend_shifts(&state, &backs)));
    }
    let knowledge = know();
    let now = Zoned::now();
    let stamp = now.timestamp().as_second();
    // Asleep inside this very tick (its prelude): a new opening all the same (the eighth review's L4).
    if !cfg!(target_os = "android") && woke_from_gap(now_tick, stamp) {
        crate::devices::resumed();
    }
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
    let mut maybe: Vec<String> = Vec::new();
    // A phone may remind the doses by its own alarm, ten minutes on: no
    // notice from here meanwhile, promised, so that it may know it is the
    // first; one in the folder, counted or not (the sixth review's R3).
    let alarms = phone_alarms(&knowledge);
    // The sleep or pause the doses waited for, ended now: their reminders count from waking (`news_from`).
    let woken = crate::hours::woken();
    // What each device did to remind each dose (none: the file does not read
    // here), the `shown` of a dose back in the schedule passed over (`back`).
    let told_now = told_here(&state);
    let answers = records();
    let me = this_device();
    // The handshake (`first_here`): alone (sharing off, or no other device in
    // the folder); else this opening written into the folder, the claim and
    // the records written, and every phone's word that it read this opening.
    let configured = crate::share::configured();
    let alone = !configured || (crate::share::on() && knowledge.peers.is_empty());
    // Sharing set up, its key not read: the others not known here, a phone among them maybe (R2).
    let alarms = alarms || (configured && !crate::share::on());
    let start = crate::devices::published_start();
    let told_all = crate::share::on() && !claim_failed && start > 0;
    // Each phone's word as its newest copy read here says it, never an older one put back (the ninth review's W1).
    let acked = told_all.then(|| acknowledged_since(&knowledge, start, &acknowledgements(&me, &newest_entries()), stamp)).flatten();
    let lead = word_lead(&knowledge);
    // A device that stopped sharing: its entries in the reminders taken out (L5); one only
    // forgotten here keeps them until they are a month old (`STALE_DAYS`: the eighth review's L6).
    let gone: Vec<String> = knowledge.peers.iter().filter(|p| p.said.as_ref().is_some_and(|s| s.left)).map(|p| p.id.clone()).collect();
    if told_now.as_ref().is_some_and(|told| told.holds_any(&gone)) {
        let _ = DoseReminders::update(&DoseReminders::default_path(), stamp, &me, |all| all.forget(&gone));
    }
    // Each dose's reminder under way, what became of it waited for below.
    let mut sending: Vec<(String, sioul_sync::notify::Sending)> = Vec::new();
    // On a phone, Android's alarm clock reminds, Sioul shown or not (`alarms`).
    crate::alarms::schedule();
    // Asleep with "Doses during sleep: stay silent" (Settings ▸ Reminders and
    // notifications), they wait for waking; by default they come, asleep or not.
    if !cfg!(target_os = "android") && !crate::hours::doses_silent() {
        // Those that waited come at waking, once (more than half an hour late, Taken asks when).
        let window = crate::hours::woke_from().map_or(GRACE_MINUTES, |since| GRACE_MINUTES.max((stamp - since) / 60 + 1));
        for dose in state.to_remind(&health, &now, window) {
            // Shown by this process already, but a dose back in the schedule after its reminder went.
            if !back(&state, &dose.key) && SENT.lock().is_ok_and(|sent| sent.contains(&dose.key)) {
                continue;
            }
            let due = dose.at.timestamp().as_second();
            let since = news_from(due, woken);
            // Shown on a device, or answered anywhere read here: nothing to remind.
            if told_now.as_ref().is_some_and(|told| told.shown(&dose.key)) || sioul_core::doses::answered(&dose.key, &state, &answers) != Answered::Open {
                continue;
            }
            let own = told_now.as_ref().and_then(|told| told.of(&dose.key, &me));
            let quiet_until = since + sioul_core::dose_reminders::QUIET_AFTER;
            // Not this computer's to remind (the doses kept elsewhere, or kept
            // here too lately): a phone sharing them is promised none from
            // here until after its alarm (docs/health.md, "What became of a reminder").
            if !(keeper.mine && keeper.settled) {
                if alarms && told_now.is_some() && stamp < quiet_until && own.is_none_or(|own| own.maybe == 0 && own.quiet < quiet_until) {
                    promise(&dose.key, quiet_until);
                }
                continue;
            }
            let first_ok = first_here(alone, told_all, keeper.since, acked, since, lead);
            let (title, body) = match plan(own, told_now.is_some(), first_ok, since, stamp, alarms) {
                Plan::Not => continue,
                Plan::Quiet(until) => {
                    promise(&dose.key, until);
                    continue;
                }
                Plan::First => {
                    // Not known whether it was taken on another device: news is
                    // waited for, within its window as the first (`first_wait`);
                    // then it is reminded all the same, the doubt said first.
                    // Another device's notice sent and not known shown (two
                    // computers each keeping the doses, the sync slow): said first too.
                    let other = told_now.as_ref().and_then(|told| told.maybe_by(&dose.key, &me)).map(|(device, at)| say("dose-doubt-maybe", &[("name", device_word(&named_by(&knowledge, &device), tr())), ("when", moment_said(at))]));
                    let doubt = doubt_unread_with(&knowledge, due, since, stamp, None, other.as_deref());
                    if !doubt.is_empty() && other.is_none() && stamp + 60 < since + first_wait(alarms) {
                        continue;
                    }
                    let named = named(&dose);
                    if doubt.is_empty() { (named, dose.at.strftime("%H:%M").to_string()) } else { (say("dose-check-title", &[("dose", named)]), format!("{}. {doubt}", dose.at.strftime("%H:%M"))) }
                }
                Plan::Late(None) => {
                    // Not its first: why another device may have reminded it, each named.
                    let elsewhere = match sioul_core::dose_reminders::others(&dose.key, since, stamp, &me, false, told_now.as_ref(), &knowledge.peers, PULLED.load(std::sync::atomic::Ordering::Relaxed)) {
                        Others::Doubt(elsewhere) => elsewhere,
                        Others::First | Others::Reminded => Vec::new(),
                    };
                    let untold = (configured && !told_all).then(|| say("dose-doubt-untold", &[]));
                    let reasons: Vec<String> = untold.into_iter().chain((!elsewhere.is_empty()).then(|| elsewhere_said(&elsewhere, since, tr()))).collect();
                    // Never plain: not its first (the seventh review's C2); none to name, said all the same.
                    let first = if reasons.is_empty() { say("dose-doubt-other", &[("when", moment_said(since))]) } else { joined(reasons, tr()) };
                    let doubt = doubt_unread_with(&knowledge, due, since, stamp, None, Some(&first));
                    (say("dose-check-title", &[("dose", named(&dose))]), format!("{}. {doubt}", dose.at.strftime("%H:%M")))
                }
                Plan::Late(why) => late_said(&dose, &knowledge, stamp, since, why),
            };
            let (title, body) = as_last_known(&dose, title, body, last_known);
            let body = match taken_lately(&dose.key, &health, &state, &now) {
                Some(said) => format!("{body} {said}"),
                None => body,
            };
            let (title, body) = said_back(&dose, &state, &now, title, body);
            if let Ok(mut sent) = SENT.lock() {
                sent.insert(dose.key.clone());
            }
            let key = dose.key.clone();
            let (qt_taken, shared_taken) = (qt.clone(), Arc::clone(shared));
            let taken: Box<dyn FnOnce() + Send> = Box::new(move || {
                // Pressed more than half an hour late: when it was taken is asked, in the window.
                if is_late(&key) {
                    let _ = qt_taken.queue(move |mut sioul| sioul.as_mut().reminder_opened(QString::from("dose"), QString::default(), QString::from(&key)));
                    return;
                }
                let problem = set_taken(&key, true);
                // Marked, nothing read here to move its next doses: said (review of 36002ea, can-wait 2; its check, low 22).
                let problem = if problem.is_empty() { unmoved_note().unwrap_or_default() } else { problem };
                if !problem.is_empty() {
                    tell(&qt_taken, &shared_taken, problem);
                }
                // Your other computers know at once.
                crate::share::exchange(&qt_taken, &shared_taken);
            });
            // Each notice recorded before it goes (`DoseNotes`), what became of
            // it after (below, and on its thread): not gone out, tried again,
            // the doubt first, until its grace's end, none while a phone's
            // alarm is due (promised); sent and not known shown, listened for.
            let instant = |at: i64| std::time::Instant::now() + std::time::Duration::from_secs(u64::try_from(at - stamp).unwrap_or(0));
            let quiet = (alarms && stamp < quiet_until).then(|| (instant(since + LATE_UNTIL), instant(quiet_until), quiet_until));
            let again = sioul_sync::notify::Again { end: instant(since + GRACE_MINUTES * 60), quiet, record: Box::new(DoseNotes { key: dose.key.clone(), since, before: None }) };
            sending.push((dose.key.clone(), sioul_sync::notify::remind_dose(&title, &body, (tr().text("health-taken", None), taken), again)));
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
            let doubts: Vec<String> = missed.iter().map(|d| doubt_of(&knowledge, d.at.timestamp().as_second(), d.at.timestamp().as_second(), stamp)).filter(|d| !d.is_empty()).collect();
            let mut body = say("health-missed-body", &[("doses", names.join(", "))]);
            if let Some(doubt) = doubts.first() {
                body = format!("{body} {doubt}");
            }
            // The health file not read here: these doses as last known (review of 36002ea's check, low 20).
            if last_known {
                body = format!("{body} {}", tr().text("health-last-known", None));
            }
            if let Err(e) = sioul_sync::notify::remind(&missed_title(&missed, &state, &told_now.clone().unwrap_or_default(), tr()), &body, Some((tr().text("health-missed-open", None), open))) {
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
    // What became of each dose's reminder: waited for here, ten seconds at most
    // in all (`notify::FIRST_TRY`: its whole first try fits in it), the record
    // let go meanwhile (the page and the notifications' buttons mark doses
    // meanwhile). Shown: a reminder. Surely not shown (no connection, no
    // notification server, nothing sent in time): `unshown`, the phone's alarm
    // rings and the question on the past asks about it. Sent and not known
    // shown (no answer in time, the bus stalled under it, this wait over):
    // `maybe`, the phone's alarm rings with the doubt first (review of battery
    // part C, the third and fourth reviews' H1).
    drop(_held);
    let deadline = std::time::Instant::now() + sioul_sync::notify::FIRST_TRY;
    for (key, sent) in sending {
        match sent.outcome(deadline.saturating_duration_since(std::time::Instant::now())) {
            sioul_sync::notify::Outcome::Shown => reminded.push(key),
            sioul_sync::notify::Outcome::Unshown(e) => {
                unshown.push(key);
                tell(qt, shared, e);
            }
            sioul_sync::notify::Outcome::Maybe(e) => {
                maybe.push(key);
                tell(qt, shared, e);
            }
            sioul_sync::notify::Outcome::Gone => {}
        }
    }
    let _held = state_held();
    // What this minute did, written over nothing anyone else wrote meanwhile.
    let problem = change(|record| note_minute(record, &reminded, &unshown, &maybe, &made, stamp));
    if !problem.is_empty() && (!reminded.is_empty() || !unshown.is_empty() || !maybe.is_empty() || !made.is_empty()) {
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
    /// The window's dose writers (decision 26 of the unified agenda), on
    /// stand-in files: each refuses a dose of tomorrow and writes nothing, and
    /// the dose stays among the alarms a phone is given; the window's Taken
    /// says its problem; under a fake clock, a dose due at midnight is
    /// tomorrow's at 23:59:30 and today's at 00:00:10; the alarms' Taken
    /// keeps no day check (a reminder can show a minute before midnight); a
    /// second Taken changes nothing, the next doses it moved included, and
    /// taking it back puts them back. One test: the files are this process's.
    /// The health files are this process's (its stand-in folders, never the
    /// person's: `stand_ins_only`): one test writing them at a time, each
    /// from none, its records' witnesses included.
    fn health_files() -> std::sync::MutexGuard<'static, ()> {
        sioul_core::config::stand_ins_only();
        let held = super::health_files_held();
        let health = Health::default_path();
        // This device's sharing file stays (other tests read it beside these): a
        // test playing another device says its id in its thread (`DEVICE`).
        let mut gone = vec![last_read_path(&health), health.with_extension("toml.unreadable")];
        // Each file with its witnesses, beside it and in the other folder (`health::witnesses`).
        for record in [health, HealthState::default_path(), DoseRecords::default_path(), sioul_core::dose_backs::DoseBacks::default_path()] {
            let witness = format!(".{}.written", record.file_name().unwrap().to_string_lossy());
            gone.extend([record.with_file_name(&witness), sioul_core::config::data_dir().join(&witness), sioul_core::config::state_dir().join(&witness), record]);
        }
        for path in gone {
            let _ = std::fs::remove_file(path);
        }
        DEVICE.with(|device| *device.borrow_mut() = None);
        HELD.with(|held| held.set(None));
        crate::alarms::ZONE_SAID.with(|said| *said.borrow_mut() = None);
        PHONE.with(|phone| phone.set(None));
        held
    }

    #[test]
    fn the_window_never_answers_a_day_to_come() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let _files = health_files();
        let (health_path, state_path, records_path) = (Health::default_path(), HealthState::default_path(), DoseRecords::default_path());
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let next_midnight = now.date().tomorrow().unwrap().to_zoned(zone.clone()).unwrap();
        let tomorrow_noon = next_midnight.checked_add(Span::new().hours(12)).unwrap().timestamp().as_second();
        let from = now.timestamp().as_second() - 3600;
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"00:00\", \"12:00\"]\n\n[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n")).unwrap();
        let bytes = |p: &std::path::Path| std::fs::read(p).ok();
        let files = || (bytes(&health_path), bytes(&state_path), bytes(&records_path));
        let tomorrow = format!("lumiel@{tomorrow_noon}");
        let refused = tr().text("dose-day-to-come", None);
        let before = files();
        assert_eq!(not_taken(&tomorrow), refused);
        assert_eq!(taken_late(&tomorrow, "09:00"), refused);
        assert_eq!(choose(&tomorrow, true), refused);
        assert_eq!(window_taken(&tomorrow, true, &now), refused, "the window's Taken says its problem");
        assert_eq!(files(), before, "nothing written");
        let coming: Vec<serde_json::Value> = serde_json::from_str(&alarms_coming().unwrap()).unwrap();
        assert!(coming.iter().any(|a| a["key"] == tomorrow.as_str()), "still rung on its day: {coming:?}");
        assert_eq!(may_answer("lumiel", &now), Err(tr().text("dose-not-known", None)));
        let at_midnight = format!("lumiel@{}", next_midnight.timestamp().as_second());
        let half_a_minute_before = now.date().to_datetime(jiff::civil::time(23, 59, 30, 0)).to_zoned(zone.clone()).unwrap();
        // A dose due at midnight is still answered from the reminder shown a minute before it (low 17); one a minute and a half after, not.
        assert_eq!(may_answer(&at_midnight, &half_a_minute_before), Ok(()));
        assert_eq!(may_answer(&format!("lumiel@{}", next_midnight.timestamp().as_second() + 90), &half_a_minute_before), Err(refused.clone()));
        assert_eq!(may_answer(&at_midnight, &next_midnight.checked_add(Span::new().seconds(10)).unwrap()), Ok(()));
        assert_eq!(window_taken(&tomorrow, false, &now), "", "a taking back is never refused");
        let answered: serde_json::Value = serde_json::from_str(&alarm_taken(&at_midnight)).unwrap();
        assert_eq!(answered["done"], true, "{answered}");
        assert!(HealthState::read(&state_path).unwrap().taken.contains_key(&at_midnight), "the alarm's Taken marks it");
        // A dose of a medicine counted from its last take, taken, then Taken again.
        let due = format!("corvan@{from}");
        assert_eq!(set_taken(&due, true), "");
        assert!(HealthState::read(&state_path).unwrap().moved.contains_key(&due), "the next doses moved, once");
        let (once, moved) = (files(), HealthState::read(&state_path).unwrap().moved.clone());
        assert_eq!(set_taken(&due, true), "");
        // A second Taken: its mark and answer anew (the check of 36002ea, must-fix 1), never a second move.
        assert_eq!((files().0, HealthState::read(&state_path).unwrap().moved.clone()), (once.0, moved), "the medicines and the move as they were");
        assert_eq!(set_taken(&due, false), "");
        assert!(load().medicines.iter().any(|m| m.id == "corvan" && m.schedule == Schedule::Hours { hours: 6, from }), "taken back, the next doses as they were");
        // An unreadable health file is said on the page, its medicines as last read, never as none.
        std::fs::write(&health_path, "[[medicine]\nid = ").unwrap();
        let shown: serde_json::Value = serde_json::from_str(&page(&Waiting::default())).unwrap();
        assert_eq!(shown["unreadable"], tr().text("health-last-known", None).as_str(), "{shown}");
        assert!(shown["any"] == true && shown["medicines"].as_array().is_some_and(|m| m.len() == 2), "as last read: {shown}");
        let _ = std::fs::remove_file(last_read_path(&health_path));
        let shown: serde_json::Value = serde_json::from_str(&page(&Waiting::default())).unwrap();
        assert_eq!(shown["unreadable"], tr().text("health-unreadable", None).as_str(), "no copy kept: none shown, said");
    }

    /// Must-fix 1 of the review of 36002ea, on two devices: the desk marks a
    /// dose taken by mistake and takes it back; the phone, holding the desk's
    /// mark but not yet its take back, is told Taken too: it writes no
    /// second mark nor move, but its own answer, so that the desk's take
    /// back, coming after, leaves the dose taken.
    #[test]
    fn a_second_taken_keeps_this_device_s_own_answer() {
        let _files = health_files();
        let (health_path, state_path, records_path) = (Health::default_path(), HealthState::default_path(), DoseRecords::default_path());
        let now = Timestamp::now().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, "[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\"]\n").unwrap();
        // This device is the phone, said in this test's thread: the sharing's file stays as it is.
        DEVICE.with(|device| *device.borrow_mut() = Some("phone-1".into()));
        assert_eq!(this_device(), "phone-1");
        // The desk's Taken at 08:01, come here: its mark and its answer.
        let key = format!("lumiel@{}", now - 240);
        let desk = Answer { state: sioul_core::doses::TAKEN.into(), at: now - 180, noted: now - 180, name: "desk".into(), kind: "computer".into() };
        HealthState::update(&state_path, now, |state| {
            state.taken.insert(key.clone(), now - 180);
        })
        .unwrap();
        DoseRecords::update(&records_path, now, |records| records.answer(&key, "desk-1", desk.clone())).unwrap();
        let moved_before = HealthState::read(&state_path).unwrap().moved.clone();
        // Taken on the phone: the same take said again, its time kept (a second
        // more: a newer word than a take back written before it, the check of
        // 36002ea, must-fix 1), its own answer, no move (the check of 1b6795e, M4).
        assert_eq!(set_taken(&key, true), "");
        let state = HealthState::read(&state_path).unwrap();
        assert_eq!(state.taken.get(&key), Some(&(now - 180 + 1)), "the same take, its time kept: {:?}", state.taken);
        assert_eq!(state.moved, moved_before, "no move");
        assert!(DoseRecords::read(&records_path).unwrap().dose[&key].answer.get("phone-1").is_some_and(|a| a.state == sioul_core::doses::TAKEN), "this device's own answer");
        // The desk's take back, written before the phone's Taken, comes: its own answer goes (the mark, older, loses to the phone's).
        DoseRecords::update(&records_path, now, |records| {
            records.dose.get_mut(&key).map(|r| r.answer.remove("desk-1"));
        })
        .unwrap();
        assert!(matches!(sioul_core::doses::answered(&key, &record(), &records()), Answered::Taken(_)), "taken, as answered on the phone");
        // Taken once more, after a take back elsewhere took this device's answer too: written anew.
        DoseRecords::update(&records_path, now, |records| {
            records.dose.get_mut(&key).map(|r| r.answer.remove("phone-1"));
        })
        .unwrap();
        assert_eq!(set_taken(&key, true), "");
        assert!(DoseRecords::read(&records_path).unwrap().dose[&key].answer.get("phone-1").is_some_and(|a| a.state == sioul_core::doses::TAKEN), "its answer anew");
        // Late, at a time typed: the mark at that time, corrected (the check's low 25); still no move.
        let typed = Zoned::now().checked_sub(Span::new().minutes(2)).unwrap();
        assert_eq!(taken_late(&key, &typed.strftime("%H:%M").to_string()), "");
        let marked = HealthState::read(&state_path).unwrap().taken[&key];
        assert_eq!(marked / 60, typed.timestamp().as_second() / 60, "the time said");
        assert_eq!(HealthState::read(&state_path).unwrap().moved, moved_before);
    }

    /// Can-wait 1 of the review of 36002ea, and H2 of the check of 1b6795e: a
    /// time given to a dose its Taken marked already (the phone's "Taken at
    /// 08:03" line, tapped later) never moves the next doses twice: its own
    /// minute, nothing; an earlier time, the take then, nothing moved (they
    /// come later, never sooner); a later time, its one move made again from
    /// where it moved from, to that time and its hours, the start it had
    /// moved to put back first everywhere; taking it back then puts the next
    /// doses back where they were. The question says when it was marked.
    #[test]
    fn a_late_time_never_moves_the_doses_twice() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let from = now.timestamp().as_second() - 3 * 3600;
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, format!("[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n")).unwrap();
        let key = format!("corvan@{from}");
        assert_eq!(set_taken(&key, true), "");
        let after_take = (std::fs::read(&health_path).unwrap(), HealthState::read(&state_path).unwrap().moved.clone());
        assert!(!after_take.1.is_empty(), "the take moved the next doses once");
        let first = Timestamp::from_second(HealthState::read(&state_path).unwrap().taken[&key]).unwrap().to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string();
        let info: serde_json::Value = serde_json::from_str(&dose_info(&key)).unwrap();
        assert_eq!((info["taken"].as_str(), info["taken_at"].as_str()), (Some(first.as_str()), Some(first.as_str())), "the question says when it was marked, and starts from it");
        let late = now.checked_sub(Span::new().minutes(33)).unwrap();
        for typed in [first.clone(), late.strftime("%H:%M").to_string()] {
            assert_eq!(taken_late(&key, &typed), "", "{typed}");
            assert_eq!((std::fs::read(&health_path).unwrap(), HealthState::read(&state_path).unwrap().moved.clone()), after_take, "its minute, or earlier: nothing moved: {typed}");
        }
        // A later time than the mark now says: the one move made again from its own start.
        let [before, after] = after_take.1[&key];
        let later = now.checked_sub(Span::new().minutes(10)).unwrap();
        assert_eq!(taken_late(&key, &later.strftime("%H:%M").to_string()), "");
        let taken = HealthState::read(&state_path).unwrap().taken[&key];
        assert_eq!(HealthState::read(&state_path).unwrap().moved.get(&key), Some(&[before, taken + 6 * 3600]), "one move, replaced");
        assert!(load().medicines.iter().any(|m| m.schedule == Schedule::Hours { hours: 6, from: taken + 6 * 3600 }), "from the time said");
        assert!(backs().back.values().any(|b| [b[0], b[1]] == [before, after]), "the start it had moved to, put back first everywhere: {:?}", backs());
        assert_eq!(set_taken(&key, false), "");
        assert!(load().medicines.iter().any(|m| m.schedule == Schedule::Hours { hours: 6, from }), "taken back: the next doses where they were");
        // At the root: a dose the schedule no longer has never moves the next ones.
        let mut moved = load();
        assert!(moved.taken_at(&key, from + 600).is_some());
        assert_eq!(moved.taken_at(&key, from + 3 * 3600), None, "its schedule moved since: no second move");
    }

    /// H4 of the check of 1b6795e, on two devices: the desk, its file not
    /// read, marks the 08:00 dose of a medicine taken every 6 hours at 10:00;
    /// the move lives in the record alone. The phone, its file read and not
    /// yet mended, the window closed (no minute tick): what Android is handed
    /// and what its alarm decides follow the record at once (16:00, never
    /// 14:00); the take back made on the desk too (from its own file, the
    /// start put back), and the tick then writes the file once.
    #[test]
    fn a_move_recorded_elsewhere_rings_here_before_any_tick() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let stamp = now.timestamp().as_second();
        // The 08:00 dose two hours ago, taken now on the desk: the next from now + 6 h.
        let from = stamp - 2 * 3600;
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let file = format!("[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n");
        std::fs::write(&health_path, &file).unwrap();
        let key = format!("corvan@{from}");
        let (replaced, moved) = (from + 6 * 3600, stamp + 6 * 3600);
        HealthState::update(&state_path, stamp, |state| {
            state.taken.insert(key.clone(), stamp);
            state.moved.insert(key.clone(), [from, moved]);
        })
        .unwrap();
        let coming: Vec<serde_json::Value> = serde_json::from_str(&alarms_coming().unwrap()).unwrap();
        let at: Vec<i64> = coming.iter().filter_map(|d| d["at"].as_i64()).collect();
        assert!(at.contains(&(moved * 1000)) && !at.contains(&(replaced * 1000)), "Android handed the moved dose, never the replaced one: {coming:?}");
        let decided: serde_json::Value = serde_json::from_str(&alarm_decide(&format!("corvan@{replaced}"))).unwrap();
        assert_eq!(decided["show"], false, "the replaced dose rings nowhere: {decided}");
        assert_eq!(std::fs::read_to_string(&health_path).unwrap(), file, "nothing written before the tick");
        // Taken back on the desk: the record's word puts the start back here at once.
        HealthState::update(&state_path, stamp, |state| {
            state.taken.remove(&key);
            state.moved.remove(&key);
        })
        .unwrap();
        sioul_core::dose_backs::DoseBacks::update(&sioul_core::dose_backs::DoseBacks::default_path(), stamp, |b| b.restore(&key, from, moved, stamp)).unwrap();
        let coming: Vec<serde_json::Value> = serde_json::from_str(&alarms_coming().unwrap()).unwrap();
        let at: Vec<i64> = coming.iter().filter_map(|d| d["at"].as_i64()).collect();
        assert!(at.contains(&(replaced * 1000)) && !at.contains(&(moved * 1000)), "the dose of before comes back: {coming:?}");
    }

    /// The chain check of e3505f7, must-fix 2: a move recorded on another
    /// device alone (its file did not read there), the phone's file as it
    /// was: the dose that rings is the moved one, and its Taken, pressed with
    /// Sioul closed before any minute of the window, moves the next one from
    /// the press, the file mended and written once.
    #[test]
    fn a_taken_with_sioul_closed_is_judged_on_the_moved_schedule() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let stamp = Zoned::now().timestamp().as_second();
        // Every 6 hours from 7 hours ago; that dose taken an hour later on the
        // computer: the next one a minute ago, recorded there alone.
        let (from, due) = (stamp - 7 * 3600, stamp - 60);
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, format!("[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n")).unwrap();
        let (first, next) = (format!("corvan@{from}"), format!("corvan@{due}"));
        HealthState::update(&state_path, stamp, |state| {
            state.taken.insert(first.clone(), due - 6 * 3600);
            state.moved.insert(first.clone(), [from, due]);
        })
        .unwrap();
        // The reminder's Taken, Sioul closed: a session of its own, no window (`alarm_taken`).
        let said: serde_json::Value = serde_json::from_str(&alarm_taken(&next)).unwrap();
        assert_eq!(said["done"], true, "{said}");
        let after = Zoned::now().timestamp().as_second();
        let state = HealthState::read(&state_path).unwrap();
        let [before, moved] = state.moved.get(&next).copied().expect("the Taken moved the next dose");
        assert!(before == due && (stamp + 6 * 3600..=after + 6 * 3600).contains(&moved), "{:?}", state.moved);
        let file = Health::read(&health_path).unwrap();
        assert!(matches!(file.medicines[0].schedule, Schedule::Hours { from, .. } if from == moved), "the file mended and moved: {:?}", file.medicines[0].schedule);
    }

    /// The keys a hand gives Android now (`alarms_coming`), with their tags.
    fn coming_now() -> Vec<(String, String)> {
        let coming: Vec<serde_json::Value> = serde_json::from_str(&alarms_coming().unwrap()).unwrap();
        coming.iter().map(|a| (a["key"].as_str().unwrap_or_default().to_string(), a["replaces"].as_str().unwrap_or_default().to_string())).collect()
    }

    /// The A4 check, #6: a medicine at a set time whose time moves after
    /// today's dose was taken (taken at 20:00, moved to 21:00): its mark and
    /// reminder carried to the new time, which never rings; a dose not taken
    /// carries nothing, and its new time rings.
    #[test]
    fn a_set_time_moved_after_its_dose_was_taken_never_rings_again() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let stamp = now.timestamp().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        // Both within today, whatever the time: an hour after midnight, and two hours before the next.
        let midnight = now.date().to_zoned(now.time_zone().clone()).unwrap();
        let (taken_at, moved_to) = (midnight.checked_add(Span::new().hours(1)).unwrap(), midnight.checked_add(Span::new().hours(22)).unwrap());
        let daily = |id: &str, at: &Zoned| format!("[[medicine]]\nid = \"{id}\"\nname = \"{id}\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n", at.strftime("%H:%M"));
        std::fs::write(&health_path, daily("lumiel", &taken_at) + &daily("corvan", &taken_at)).unwrap();
        let key_at = |id: &str, at: &Zoned| load().doses(&at.checked_sub(Span::new().minutes(1)).unwrap(), &at.checked_add(Span::new().minutes(1)).unwrap()).into_iter().find(|d| d.key.starts_with(&format!("{id}@"))).unwrap().key;
        let (lumiel, corvan) = (key_at("lumiel", &taken_at), key_at("corvan", &taken_at));
        HealthState::update(&state_path, stamp, |state| {
            state.taken.insert(lumiel.clone(), stamp - 60);
            state.reminded.insert(lumiel.clone(), stamp - 120);
            state.reminded.insert(corvan.clone(), stamp - 120);
        })
        .unwrap();
        change_health(|health| {
            for medicine in &mut health.medicines {
                medicine.schedule = Schedule::Day { times: vec![moved_to.strftime("%H:%M").to_string()], amounts: Default::default() };
            }
            Ok(())
        })
        .unwrap();
        let (lumiel_now, corvan_now) = (key_at("lumiel", &moved_to), key_at("corvan", &moved_to));
        let state = HealthState::read(&state_path).unwrap();
        assert!(state.taken.contains_key(&lumiel_now) && state.reminded.contains_key(&lumiel_now), "taken: carried to its new time: {state:?}");
        assert!(!state.taken.contains_key(&corvan_now) && !state.reminded.contains_key(&corvan_now), "not taken: nothing carried");
        let coming = coming_now();
        assert!(!coming.iter().any(|(key, _)| *key == lumiel_now) && coming.iter().any(|(key, _)| *key == corvan_now), "{coming:?}");
    }

    /// The A5 check, M3 and L2: a set-time dose reminded in Paris and left
    /// unmarked, then the phone's zone changed to Tokyo (Android says it, both
    /// processes read it at once): its key is gone with the zone, not moved,
    /// so its reminder stays and no dose of Tokyo's day replaces it, even one
    /// noted as taken away; the doses are keyed in Tokyo, never in Paris.
    #[test]
    fn a_change_of_zone_takes_no_reminder_away_and_rings_no_replacement() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let stamp = Timestamp::now().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let (paris, tokyo) = (TimeZone::get("Europe/Paris").unwrap(), TimeZone::get("Asia/Tokyo").unwrap());
        // Reminded two hours ago in Paris; its second time of the day, twelve hours on.
        let rang = Timestamp::from_second(stamp - 2 * 3600 - stamp % 60).unwrap().to_zoned(paris);
        let later = rang.checked_add(Span::new().hours(12)).unwrap();
        std::fs::write(&health_path, format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\", \"{}\"]\n", rang.strftime("%H:%M"), later.strftime("%H:%M"))).unwrap();
        let in_paris = format!("lumiel@{}", rang.timestamp().as_second());
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(in_paris.clone(), rang.timestamp().as_second());
        })
        .unwrap();
        // Landed: Android said Tokyo a minute ago, in the file both processes read.
        crate::alarms::ZONE_SAID.with(|said| *said.borrow_mut() = Some(crate::alarms::ZoneSaid { zone: "Asia/Tokyo".into(), said: stamp - 60, earlier: vec![crate::alarms::EarlierZone { zone: "Europe/Paris".into(), until: stamp - 60 }] }));
        assert_eq!(crate::alarms::now_here().time_zone().iana_name(), Some("Asia/Tokyo"));
        assert!(!reminded_gone().unwrap().iter().any(|(key, _)| *key == in_paris), "its reminder stays");
        // Noted as taken away all the same (an earlier build's hand): nothing replaces it.
        HealthState::update(&state_path, stamp, |state| {
            state.taken_away.insert(in_paris.clone(), rang.timestamp().as_second());
        })
        .unwrap();
        let coming = coming_now();
        assert!(coming.iter().all(|(_, tag)| tag.is_empty()), "no replacement: {coming:?}");
        // Keyed in Tokyo: its second time there, never Paris's.
        let now = Timestamp::from_second(stamp).unwrap().to_zoned(tokyo);
        let in_tokyo = load().doses(&now, &now.checked_add(Span::new().hours(24)).unwrap()).into_iter().find(|d| d.key.starts_with("lumiel@") && d.at.strftime("%H:%M").to_string() == later.strftime("%H:%M").to_string()).unwrap().key;
        assert!(coming.iter().any(|(key, _)| *key == in_tokyo) && !coming.iter().any(|(key, _)| *key == format!("lumiel@{}", later.timestamp().as_second())), "{coming:?}");
    }

    /// The A6 check, L4: a set-time dose reminded in Paris, then a stop at
    /// Dubai, then Tokyo, all within twelve hours: keyed two zones back, it
    /// is still gone with the zone, its reminder kept and replaced by none.
    #[test]
    fn a_trip_with_a_stop_between_replaces_nothing() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let stamp = Timestamp::now().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let paris = TimeZone::get("Europe/Paris").unwrap();
        // Reminded three hours ago in Paris; its second time of the day, twelve hours on.
        let rang = Timestamp::from_second(stamp - 3 * 3600 - stamp % 60).unwrap().to_zoned(paris);
        let later = rang.checked_add(Span::new().hours(12)).unwrap();
        std::fs::write(&health_path, format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\", \"{}\"]\n", rang.strftime("%H:%M"), later.strftime("%H:%M"))).unwrap();
        let in_paris = format!("lumiel@{}", rang.timestamp().as_second());
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(in_paris.clone(), rang.timestamp().as_second());
        })
        .unwrap();
        // Paris until two hours ago, then Dubai, then Tokyo since a minute.
        let earlier = |zone: &str, until: i64| crate::alarms::EarlierZone { zone: zone.into(), until };
        crate::alarms::ZONE_SAID.with(|said| *said.borrow_mut() = Some(crate::alarms::ZoneSaid { zone: "Asia/Tokyo".into(), said: stamp - 60, earlier: vec![earlier("Europe/Paris", stamp - 2 * 3600), earlier("Asia/Dubai", stamp - 60)] }));
        assert!(!reminded_gone().unwrap().iter().any(|(key, _)| *key == in_paris), "its reminder stays");
        HealthState::update(&state_path, stamp, |state| {
            state.taken_away.insert(in_paris.clone(), rang.timestamp().as_second());
        })
        .unwrap();
        let coming = coming_now();
        assert!(coming.iter().all(|(_, tag)| tag.is_empty()), "no replacement: {coming:?}");
    }

    /// The A6 check, M2, on a phone (`is_phone`): with no medicines and no
    /// last read, its file reads as none (L5); its only medicine taken out
    /// elsewhere with no mark is trusted as none, nothing handed; two gone
    /// so give its last read, the sharing's own threshold; Remove of one the
    /// last read alone holds ends its ringing, and of the other too, the file
    /// then trusted as none.
    #[test]
    fn a_phone_trusts_one_medicine_gone_unmarked_and_holds_two() {
        let _files = health_files();
        let health_path = Health::default_path();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        PHONE.with(|phone| phone.set(Some(true)));
        std::fs::write(&health_path, "# none yet\n").unwrap();
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.is_empty()), "no last read: read as none");
        let soon = Zoned::now().checked_add(Span::new().hours(1)).unwrap();
        let daily = |id: &str| format!("[[medicine]]\nid = \"{id}\"\nname = \"{id}\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n", soon.strftime("%H:%M"));
        // One medicine, taken out elsewhere with no mark: trusted as none, nothing handed.
        std::fs::write(&health_path, daily("lumiel")).unwrap();
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.len() == 1));
        std::fs::write(&health_path, "# emptied elsewhere\n").unwrap();
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.is_empty()), "one gone: trusted as none");
        assert!(coming_now().is_empty(), "{:?}", coming_now());
        // Two, gone with no mark: the last read given, said as last known.
        std::fs::write(&health_path, daily("lumiel") + &daily("corvan")).unwrap();
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.len() == 2));
        std::fs::write(&health_path, "# emptied elsewhere\n").unwrap();
        assert!(matches!(load_known(), Some((read, true)) if read.medicines.len() == 2), "two gone: the last read");
        // Removed on the Health page, one the last read alone holds: it rings no more; then the other.
        assert_eq!(remove("lumiel"), "");
        assert!(matches!(load_known(), Some((read, true)) if read.medicines.iter().map(|m| m.id.as_str()).collect::<Vec<_>>() == ["corvan"]));
        assert!(!coming_now().iter().any(|(key, _)| key.starts_with("lumiel@")), "{:?}", coming_now());
        assert_eq!(remove("corvan"), "");
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.is_empty()), "both removed: trusted as none");
        assert!(coming_now().is_empty(), "{:?}", coming_now());
    }

    /// The A5 check, M2: a medicine at a set time whose dose of today was said
    /// not taken, then its time moved later the same day (08:00 said not
    /// taken at 10:00, moved to 21:00 at 10:05): nothing carried, the new time
    /// rings, and nothing says "not taken" of it before it comes.
    #[test]
    fn a_set_time_moved_after_its_dose_was_said_not_taken_still_rings() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let stamp = now.timestamp().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        // Both within today, whatever the time: an hour after midnight, and two hours before the next.
        let midnight = now.date().to_zoned(now.time_zone().clone()).unwrap();
        let (said_at, moved_to) = (midnight.checked_add(Span::new().hours(1)).unwrap(), midnight.checked_add(Span::new().hours(22)).unwrap());
        let daily = |at: &Zoned| format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n", at.strftime("%H:%M"));
        std::fs::write(&health_path, daily(&said_at)).unwrap();
        let key_at = |at: &Zoned| load().doses(&at.checked_sub(Span::new().minutes(1)).unwrap(), &at.checked_add(Span::new().minutes(1)).unwrap()).into_iter().find(|d| d.key.starts_with("lumiel@")).unwrap().key;
        let before = key_at(&said_at);
        HealthState::update(&state_path, stamp, |state| {
            state.not_taken.insert(before.clone(), stamp - 60);
            state.reminded.insert(before.clone(), stamp - 120);
        })
        .unwrap();
        change_health(|health| {
            health.medicines[0].schedule = Schedule::Day { times: vec![moved_to.strftime("%H:%M").to_string()], amounts: Default::default() };
            Ok(())
        })
        .unwrap();
        let after = key_at(&moved_to);
        let state = HealthState::read(&state_path).unwrap();
        assert!(!state.not_taken.contains_key(&after) && !state.reminded.contains_key(&after), "nothing carried: {state:?}");
        assert!(state.not_taken.contains_key(&before), "the answer stays with its own time");
        assert!(coming_now().iter().any(|(key, _)| *key == after), "the new time rings: {:?}", coming_now());
    }

    /// The A4 check, #8: a reminder a hand took away because its dose left the
    /// schedule comes back with its dose. Corvan every 6 hours, the 14:00 dose
    /// shown on the phone; the 08:00 one marked taken late at 08:30 on the
    /// computer moves it to 14:30, and the phone's next hand takes its
    /// reminder away, noted; the mark taken back, 14:00 is due again: listed
    /// for Android again, reminded on the computer, decided as not reminded,
    /// its shown on record (the phone's own) passed over: the phone's alarm
    /// shows it again, the doubt said ("Reminded at 14:00 already"), and the
    /// computer reminds it (the A5 check, M1); reminded anew, it is reminded.
    /// A daily medicine's time changed and changed back, and a pause resumed,
    /// the same.
    #[test]
    fn a_reminder_taken_away_comes_back_with_its_dose() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let stamp = now.timestamp().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let every = |from: i64| format!("[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n");
        let (eight, two) = (stamp - 6 * 3600 - 600, stamp - 600);
        let (first, shown) = (format!("corvan@{eight}"), format!("corvan@{two}"));
        std::fs::write(&health_path, every(eight)).unwrap();
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(shown.clone(), two + 1);
        })
        .unwrap();
        // Shown on the phone, its own word written as `remind_shown` writes it.
        DEVICE.with(|device| *device.borrow_mut() = Some("phone-1".into()));
        let showed = |key: &str, at: i64| {
            tell_mine(key, |mine| {
                mine.maybe = at;
                mine.shown = at;
            })
        };
        assert!(showed(&shown, two + 1));
        // Its reminder shown again: the doubt said, at its time as reminded first.
        let again = |key: &str, at: i64, name: &str| {
            let said: serde_json::Value = serde_json::from_str(&alarm_decide(key)).unwrap();
            let before = say("dose-reminded-before", &[("time", Timestamp::from_second(at).unwrap().to_zoned(now.time_zone().clone()).strftime("%H:%M").to_string())]);
            assert!(said["show"] == true && said["body"].as_str().is_some_and(|b| b.ends_with(&before)), "{key}: {said}");
            assert_eq!(said["title"].as_str(), Some(say("dose-check-title", &[("dose", name.to_string())]).as_str()), "{said}");
        };
        // Marked late elsewhere: the next dose at 14:30; the hand takes 14:00's reminder away.
        std::fs::write(&health_path, every(eight + 1800 + 6 * 3600)).unwrap();
        HealthState::update(&state_path, stamp, |state| {
            state.taken.insert(first.clone(), eight + 1800);
            state.moved.insert(first.clone(), [eight, eight + 1800 + 6 * 3600]);
        })
        .unwrap();
        let gone = reminded_gone().unwrap();
        assert_eq!(gone, [(shown.clone(), two + 1)]);
        note_taken_away(&gone);
        assert_eq!(reminded_gone(), Some(Vec::new()), "noted: not taken away twice");
        assert!(!coming_now().iter().any(|(key, _)| *key == shown));
        // The mark taken back: 14:00 due again, unmarked.
        std::fs::write(&health_path, every(eight)).unwrap();
        HealthState::update(&state_path, stamp, |state| {
            state.taken.remove(&first);
            state.moved.remove(&first);
        })
        .unwrap();
        assert!(coming_now().iter().any(|(key, tag)| *key == shown && *tag == shown), "armed again, at once even past its half hour: {:?}", coming_now());
        let state = HealthState::read(&state_path).unwrap();
        assert!(!state.reminded_now(&shown) && state.to_remind(&load(), &now, 30).iter().any(|d| d.key == shown), "not reminded: the computer reminds it too");
        assert!(!reminded_already(&state, &shown), "the alarm decides it as not reminded");
        // Its shown on record is the reminder taken away: passed over by the
        // computer's tick and its late reminder (the A5 check, M1).
        assert!(told().is_some_and(|told| told.shown(&shown)) && !told_here(&state).is_some_and(|told| told.shown(&shown)), "passed over");
        assert!(still_to_remind(&shown), "the computer reminds it");
        // The phone's alarm shows it, the doubt said; reminded anew, the note ended.
        again(&shown, two + 1, "Corvan");
        let state = HealthState::read(&state_path).unwrap();
        assert!(state.reminded_now(&shown) && !state.taken_away.contains_key(&shown), "reminded anew");
        assert!(!coming_now().iter().any(|(key, _)| *key == shown));
        // A daily medicine's time changed after it rang, then changed back; then paused and resumed.
        let rang = now.checked_sub(Span::new().minutes(10)).unwrap();
        let daily = |at: &Zoned, paused: bool| format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\npaused = {paused}\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n", at.strftime("%H:%M"));
        std::fs::write(&health_path, daily(&rang, false)).unwrap();
        let key = load().doses(&rang.checked_sub(Span::new().minutes(1)).unwrap(), &rang.checked_add(Span::new().minutes(1)).unwrap()).into_iter().find(|d| d.key.starts_with("lumiel@")).unwrap().key;
        for away in [daily(&rang.checked_add(Span::new().hours(1)).unwrap(), false), daily(&rang, true)] {
            HealthState::update(&state_path, stamp, |state| {
                state.reminded.insert(key.clone(), stamp - 540);
                state.taken_away.remove(&key);
            })
            .unwrap();
            assert!(showed(&key, stamp - 540));
            std::fs::write(&health_path, &away).unwrap();
            let gone = reminded_gone().unwrap();
            assert!(gone.iter().any(|(k, _)| *k == key), "{away}: {gone:?}");
            note_taken_away(&gone);
            std::fs::write(&health_path, daily(&rang, false)).unwrap();
            assert!(coming_now().iter().any(|(k, _)| *k == key), "{away}, then back: armed again");
            assert!(still_to_remind(&key), "{away}, then back: the computer reminds it");
            again(&key, stamp - 540, "Lumiel");
        }
    }

    /// The A4 check, #1: a move that puts the dose replacing a reminded one
    /// more than half an hour in the past. Corvan every 6 hours, the 08:00
    /// dose not marked, the phone showing 14:00; at 14:50 the computer says
    /// the 08:00 one was taken at 08:10: 14:00 is gone, its reminder taken
    /// away at the phone's next hand, and corvan@14:10, 40 minutes past, is
    /// tagged as its replacement, for Android to ring it once, at once. A
    /// daily medicine's time moved from 20:00 to 19:00 at 20:30, after 20:00
    /// rang, the same.
    #[test]
    fn the_dose_replacing_a_gone_reminder_past_the_half_hour_rings() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let stamp = now.timestamp().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let every = |from: i64| format!("[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n");
        let (eight, two) = (stamp - 6 * 3600 - 50 * 60, stamp - 50 * 60);
        let (first, shown, moved) = (format!("corvan@{eight}"), format!("corvan@{two}"), format!("corvan@{}", two + 600));
        std::fs::write(&health_path, every(eight)).unwrap();
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(shown.clone(), two + 1);
        })
        .unwrap();
        std::fs::write(&health_path, every(eight + 600 + 6 * 3600)).unwrap();
        HealthState::update(&state_path, stamp, |state| {
            state.taken.insert(first.clone(), eight + 600);
            state.moved.insert(first.clone(), [eight, eight + 600 + 6 * 3600]);
        })
        .unwrap();
        assert!(coming_now().iter().any(|(key, tag)| *key == moved && tag.is_empty()), "listed, untagged before its reminder is taken away");
        note_taken_away(&reminded_gone().unwrap());
        assert!(coming_now().iter().any(|(key, tag)| *key == moved && *tag == shown), "tagged: {:?}", coming_now());
        // Rung and reminded: no longer listed, nor tagged.
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(moved.clone(), stamp);
        })
        .unwrap();
        assert!(!coming_now().iter().any(|(key, _)| *key == moved));
        // A daily medicine moved from 20:00 to 19:00 at 20:30, after 20:00 rang.
        let rang = now.checked_sub(Span::new().minutes(30)).unwrap();
        let earlier = rang.checked_sub(Span::new().hours(1)).unwrap();
        let daily = |at: &Zoned| format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n", at.strftime("%H:%M"));
        let near = |at: &Zoned| load().doses(&at.checked_sub(Span::new().minutes(1)).unwrap(), &at.checked_add(Span::new().minutes(1)).unwrap()).into_iter().find(|d| d.key.starts_with("lumiel@")).unwrap().key;
        std::fs::write(&health_path, daily(&rang)).unwrap();
        let eight_pm = near(&rang);
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(eight_pm.clone(), stamp - 1800);
        })
        .unwrap();
        std::fs::write(&health_path, daily(&earlier)).unwrap();
        let seven_pm = near(&earlier);
        note_taken_away(&reminded_gone().unwrap());
        assert!(coming_now().iter().any(|(key, tag)| *key == seven_pm && *tag == eight_pm), "tagged: {:?}", coming_now());
    }

    /// What replaced a gone dose is the dose of its medicine nearest its
    /// time, due by now, and it rings once; once it rang or was marked, no
    /// other dose takes its place: one of that medicine left unanswered hours
    /// before never rings late for it (a noon dose at 20:31, beside a 19:00
    /// just taken, would invite two at once).
    #[test]
    fn only_the_nearest_dose_replaces_a_gone_one_and_only_once() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let stamp = now.timestamp().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let ago = |minutes: i64| now.checked_sub(Span::new().minutes(minutes)).unwrap();
        let (noon, rang, earlier) = (ago(510), ago(30), ago(90));
        let twice = |a: &Zoned, b: &Zoned| format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\", \"{}\"]\n", a.strftime("%H:%M"), b.strftime("%H:%M"));
        let near = |at: &Zoned| load().doses(&at.checked_sub(Span::new().minutes(1)).unwrap(), &at.checked_add(Span::new().minutes(1)).unwrap()).into_iter().find(|d| d.key.starts_with("lumiel@")).unwrap().key;
        std::fs::write(&health_path, twice(&noon, &rang)).unwrap();
        let (noon_key, gone) = (near(&noon), near(&rang));
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(gone.clone(), stamp - 1800);
        })
        .unwrap();
        // Its time moved an hour earlier after it rang: its reminder taken away, the moved dose tagged, not noon's.
        std::fs::write(&health_path, twice(&noon, &earlier)).unwrap();
        let moved = near(&earlier);
        note_taken_away(&reminded_gone().unwrap());
        let coming = coming_now();
        assert!(coming.iter().any(|(key, tag)| *key == moved && *tag == gone) && coming.iter().any(|(key, tag)| *key == noon_key && tag.is_empty()), "{coming:?}");
        // Taken: nothing replaces it any more, noon's dose least of all.
        HealthState::update(&state_path, stamp, |state| {
            state.taken.insert(moved.clone(), stamp);
        })
        .unwrap();
        assert!(coming_now().iter().all(|(_, tag)| tag.is_empty()), "{:?}", coming_now());
    }

    /// The A6 dose model: a dose whose reminder a hand took away while its
    /// late Taken was half written (its file moved before its mark), then
    /// marked: answered, so nothing replaces it, and its next dose, past its
    /// half hour, is the late question's, never rung late.
    #[test]
    fn a_dose_marked_after_its_reminder_went_is_replaced_by_none() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let stamp = Timestamp::now().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let every = |from: i64| format!("[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n");
        let eight = stamp - 8 * 3600;
        let first = format!("corvan@{eight}");
        std::fs::write(&health_path, every(eight)).unwrap();
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(first.clone(), eight + 1);
        })
        .unwrap();
        // Taken at 08:30, typed late: the file moved first, a hand in between takes its reminder away.
        let taken = eight + 1800;
        std::fs::write(&health_path, every(taken + 6 * 3600)).unwrap();
        note_taken_away(&reminded_gone().unwrap());
        assert!(HealthState::read(&state_path).unwrap().taken_away.contains_key(&first));
        HealthState::update(&state_path, stamp, |state| {
            state.taken.insert(first.clone(), taken);
            state.moved.insert(first.clone(), [eight, taken + 6 * 3600]);
        })
        .unwrap();
        let coming = coming_now();
        assert!(coming.iter().any(|(key, _)| *key == format!("corvan@{}", taken + 6 * 3600)) && coming.iter().all(|(_, tag)| tag.is_empty()), "{coming:?}");
    }

    /// The A4 check, #12: this device's health file emptied by hand or by a
    /// bug (no removal of Sioul's) is never kept as the last read; while the
    /// sharing holds that emptying, that last read is given, said as last
    /// known, so that the phone is handed its doses until the word that ends
    /// the hold. Medicines taken out in Sioul (their removals marked) leave
    /// the last read too.
    #[test]
    fn a_file_emptied_while_the_sharing_holds_it_still_hands_its_doses() {
        let _files = health_files();
        let health_path = Health::default_path();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let soon = Zoned::now().checked_add(Span::new().hours(1)).unwrap();
        let daily = |id: &str| format!("[[medicine]]\nid = \"{id}\"\nname = \"{id}\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n", soon.strftime("%H:%M"));
        std::fs::write(&health_path, daily("lumiel") + &daily("corvan")).unwrap();
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.len() == 2));
        // Emptied by hand: read as it is, and never kept as the last read.
        std::fs::write(&health_path, "# emptied by hand\n").unwrap();
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.is_empty()));
        assert!(last_read(&health_path).is_some_and(|copy| copy.medicines.len() == 2), "the last read keeps them");
        // The sharing holds that emptying: the last read, said as last known; its doses handed.
        HELD.with(|held| held.set(Some(true)));
        assert!(matches!(load_known(), Some((read, true)) if read.medicines.len() == 2));
        let coming = coming_now();
        assert!(["lumiel@", "corvan@"].iter().all(|id| coming.iter().any(|(key, _)| key.starts_with(id))), "{coming:?}");
        // One taken out in Sioul since (marked): never given again.
        std::fs::write(&health_path, "[removed]\nlumiel = 1\n").unwrap();
        assert!(matches!(load_known(), Some((read, true)) if read.medicines.iter().map(|m| m.id.as_str()).collect::<Vec<_>>() == ["corvan"]));
        // "Take them out everywhere" marks both first: read as none at once, the hold still standing; the last read follows.
        std::fs::write(&health_path, "[removed]\nlumiel = 1\ncorvan = 1\n").unwrap();
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.is_empty()));
        assert!(last_read(&health_path).is_some_and(|copy| copy.medicines.is_empty()), "taken out in Sioul: the last read follows");
        // Held with no last read at all: nothing reads, and Android keeps the alarms it has.
        let _ = std::fs::remove_file(last_read_path(&health_path));
        std::fs::write(&health_path, "# emptied by hand\n").unwrap();
        assert!(load_known().is_none() && alarms_coming().is_none());
    }

    /// The A5 check, L4: on a phone (or while the sharing holds it) a health
    /// file read as no medicines, one gone with no mark, gives its last read;
    /// "Take them out here too" takes them out here in Sioul, marked
    /// (`take_out_in_sioul`): the file is trusted as none at once, its last
    /// read following, and no alarm is handed for them.
    #[test]
    fn the_word_here_takes_the_medicines_out_marked_and_the_last_read_follows() {
        let _files = health_files();
        let health_path = Health::default_path();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let soon = Zoned::now().checked_add(Span::new().hours(1)).unwrap();
        let daily = |id: &str| format!("[[medicine]]\nid = \"{id}\"\nname = \"{id}\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n", soon.strftime("%H:%M"));
        std::fs::write(&health_path, daily("lumiel") + &daily("corvan")).unwrap();
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.len() == 2));
        HELD.with(|held| held.set(Some(true)));
        take_out_in_sioul(&["lumiel".to_string(), "corvan".to_string()]);
        let file = Health::read(&health_path).unwrap();
        assert!(file.medicines.is_empty() && ["lumiel", "corvan"].iter().all(|id| file.removed.contains_key(*id)), "{file:?}");
        assert!(matches!(load_known(), Some((read, false)) if read.medicines.is_empty()), "trusted as none");
        assert!(last_read(&health_path).is_some_and(|copy| copy.medicines.is_empty()), "the last read follows");
        assert!(coming_now().is_empty(), "{:?}", coming_now());
    }

    /// The chain check of e3505f7, must-fix 1: a dose reminded, then moved
    /// away by a change made on another device (a late Taken there, read by
    /// a background exchange): its reminder is one to take away, at the hand
    /// that follows; one still due is not; a daily medicine moved from one
    /// time to another after the first rang leaves the first's to take away;
    /// nothing reads, nothing taken away.
    #[test]
    fn a_reminded_dose_moved_away_is_one_whose_reminder_goes() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let stamp = Zoned::now().timestamp().as_second();
        let from = stamp - 10 * 60;
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let file = |from: i64| format!("[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n");
        std::fs::write(&health_path, file(from)).unwrap();
        let shown = format!("corvan@{from}");
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(shown.clone(), stamp - 5 * 60);
        })
        .unwrap();
        assert_eq!(reminded_gone(), Some(Vec::new()), "still due: its reminder stays");
        // The dose before marked late elsewhere: this one moved half an hour on.
        std::fs::write(&health_path, file(from + 30 * 60)).unwrap();
        assert_eq!(reminded_gone(), Some(vec![(shown.clone(), stamp - 5 * 60)]));
        // Marked: taken away as marked, not as gone.
        HealthState::update(&state_path, stamp, |state| {
            state.taken.insert(shown.clone(), stamp);
        })
        .unwrap();
        assert_eq!(reminded_gone(), Some(Vec::new()));
        // Nothing reads: nothing taken away.
        std::fs::write(&health_path, "not toml [").unwrap();
        let _ = std::fs::remove_file(last_read_path(&health_path));
        assert_eq!(reminded_gone(), None);
        // A medicine at a set time moved an hour on after it rang (20:00 to
        // 21:00): the reminder rung goes, and the new time's alarm is the one
        // to come, one reminder, not two.
        let now = Zoned::now();
        let rang = now.checked_sub(Span::new().minutes(10)).unwrap();
        let later = rang.checked_add(Span::new().hours(1)).unwrap();
        let day = |at: &Zoned| format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n", at.strftime("%H:%M"));
        std::fs::write(&health_path, day(&rang)).unwrap();
        let near = |health: &Health, at: &Zoned| health.doses(&at.checked_sub(Span::new().minutes(1)).unwrap(), &at.checked_add(Span::new().minutes(1)).unwrap()).into_iter().find(|d| d.key.starts_with("lumiel@")).map(|d| d.key);
        let rung = near(&load(), &rang).expect("its dose at the time it rang");
        HealthState::update(&state_path, stamp, |state| {
            state.reminded.insert(rung.clone(), stamp - 9 * 60);
        })
        .unwrap();
        assert_eq!(reminded_gone(), Some(Vec::new()), "rung, still due: its reminder stays");
        std::fs::write(&health_path, day(&later)).unwrap();
        assert_eq!(reminded_gone(), Some(vec![(rung.clone(), stamp - 9 * 60)]), "moved on: the reminder rung goes");
        let moved = near(&load(), &later).expect("its dose at the new time");
        let coming: Vec<serde_json::Value> = serde_json::from_str(&alarms_coming().unwrap()).unwrap();
        let keys: Vec<&str> = coming.iter().filter_map(|a| a["key"].as_str()).collect();
        assert!(keys.contains(&moved.as_str()) && !keys.contains(&rung.as_str()), "the new time armed, the old one not: {keys:?}");
    }

    /// Can-wait 2 and 3 of the review of 36002ea: a health file that does not
    /// read is never written over, a Taken marks the dose there all the same
    /// without moving the next ones, and says so; its doses stay as last read
    /// for the alarms a phone is given, the Porch, the home card and the
    /// reminders' words, each saying it, never read as none.
    #[test]
    fn an_unreadable_health_file_is_never_written_over_nor_silences_a_dose() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let soon = now.checked_add(Span::new().hours(2)).unwrap();
        let from = now.timestamp().as_second() - 3600;
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, format!("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\"]\n\n[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n", soon.strftime("%H:%M"))).unwrap();
        // A good read keeps its copy; then the file breaks (a newer Sioul, a hand).
        assert_eq!(load_said().0.medicines.len(), 2);
        assert!(last_read_path(&health_path).exists());
        let broken = "[[medicine]\nid = \"lumiel\"\n";
        std::fs::write(&health_path, broken).unwrap();
        let (health, unread) = load_said();
        assert!(unread && health.medicines.len() == 2, "as last read, never as none");
        let coming: Vec<serde_json::Value> = serde_json::from_str(&alarms_coming().unwrap()).unwrap();
        assert!(coming.iter().any(|a| a["key"].as_str().is_some_and(|k| k.starts_with("lumiel@"))), "a phone keeps its alarms: {coming:?}");
        assert!(card_doses(&now).iter().all(|d| d.known_until == 0), "the home card says check");
        let dose = health.doses(&now, &now.checked_add(Span::new().hours(3)).unwrap()).into_iter().find(|d| d.key.starts_with("lumiel@")).unwrap();
        let (title, body) = as_last_known(&dose, "Lumiel".into(), "10:00".into(), true);
        assert!(title.contains("Lumiel") && body.ends_with(&tr().text("dose-last-known", None)), "{title} / {body}");
        assert_eq!(as_last_known(&dose, "Lumiel".into(), "10:00".into(), false), ("Lumiel".to_string(), "10:00".to_string()));
        // Taken: marked, never written over the file; its move made on the last
        // read and recorded beside the mark, for every device whose file reads
        // to make, and shown here (the check of 36002ea, must-fix 3).
        let key = format!("corvan@{from}");
        assert_eq!(window_taken(&key, true, &now), "");
        assert_eq!(unmoved_note(), None, "the move recorded: nothing to say");
        let state = HealthState::read(&state_path).unwrap();
        assert!(state.taken.contains_key(&key), "the mark kept");
        let [before, after] = state.moved[&key];
        assert!(before == from && after >= now.timestamp().as_second() + 6 * 3600, "its move recorded: {:?}", state.moved);
        assert!(load().medicines.iter().any(|m| m.schedule == Schedule::Hours { hours: 6, from: after }), "this device's last read follows the record");
        // Another device's file, as this one last read it: the move not made there yet.
        let mut reading = last_read(&health_path).unwrap();
        assert!(reading.mend_shifts(&state, &backs()) && reading.medicines.iter().any(|m| m.schedule == Schedule::Hours { hours: 6, from: after }), "a device whose file reads makes it");
        assert_eq!(std::fs::read_to_string(&health_path).unwrap(), broken, "never written over");
        assert_eq!(std::fs::read_to_string(health_path.with_extension("toml.unreadable")).unwrap(), broken, "copied aside as it is");
        // Taken back here: its move kept in the record as taken back, put back
        // here and on every device whose file reads (must-fix 4), never dropped.
        assert_eq!(window_taken(&key, false, &now), "");
        let state = HealthState::read(&state_path).unwrap();
        assert!(!state.taken.contains_key(&key) && !state.moved.contains_key(&key), "{state:?}");
        assert!(backs().back.values().any(|b| [b[0], b[1]] == [before, after]), "kept as taken back, in its own file: {:?}", backs());
        assert!(load().medicines.iter().any(|m| m.schedule == Schedule::Hours { hours: 6, from: before }), "put back here too");
        assert!(reading.mend_shifts(&state, &backs()) && reading.medicines.iter().any(|m| m.schedule == Schedule::Hours { hours: 6, from: before }), "and where the file reads");
        assert_eq!(set_setting("movement.minutes", "5"), tr().text("health-not-written", None), "every writer refuses, saying why");
        assert_eq!(std::fs::read_to_string(&health_path).unwrap(), broken);
    }

    /// Can-wait 3 of the review of 36002ea, on the alarm path. The health
    /// file broken, a phone's alarm at a dose's time rings from its last read,
    /// the doubt said; the computer's minute finds the same doses (under a
    /// clock set by the test). With no last read either, Android keeps the
    /// alarms it has (nothing handed, never an empty list), and the one that
    /// rings is still shown, in doubt, once; a dose marked meanwhile is not.
    #[test]
    fn an_alarm_rings_in_doubt_whatever_the_health_file_reads() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Timestamp::now().as_second();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, format!("[[medicine]]\nid = \"mirel\"\nname = \"Mirel\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {}\n\n[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\"]\n", now - 30)).unwrap();
        assert_eq!(load_said().0.medicines.len(), 2, "read: its copy kept");
        std::fs::write(&health_path, "[[medicine]\nid = ").unwrap();
        // The computer's minute, at 08:01 by the test's clock: the 08:00 dose, as last read.
        let zone = TimeZone::system();
        let at_eight = Zoned::now().date().to_datetime(jiff::civil::time(8, 1, 0, 0)).to_zoned(zone.clone()).unwrap();
        let (health, last_known) = load_said();
        assert!(last_known && record().to_remind(&health, &at_eight, GRACE_MINUTES).iter().any(|d| d.key.starts_with("lumiel@")), "reminded from the last read");
        // The phone's alarm, as last read: shown, the doubt said.
        let mirel = format!("mirel@{}", now - 30);
        let said: serde_json::Value = serde_json::from_str(&alarm_decide(&mirel)).unwrap();
        assert_eq!(said["show"], true, "{said}");
        assert!(said["title"].as_str().is_some_and(|t| t == say("dose-check-title", &[("dose", "Mirel".into())])), "{said}");
        assert!(said["body"].as_str().is_some_and(|b| b.ends_with(&tr().text("dose-last-known", None))), "{said}");
        // Its Taken: marked, its move made on the last read and recorded (the check of 36002ea, must-fix 3), nothing to say besides.
        let taken: serde_json::Value = serde_json::from_str(&alarm_taken(&mirel)).unwrap();
        assert!(taken["done"] == true && taken["line"] != tr().text("dose-next-unmoved", None).as_str(), "{taken}");
        let state = HealthState::read(&state_path).unwrap();
        let [_, after] = state.moved[&mirel];
        assert!(state.taken.contains_key(&mirel) && after >= now + 6 * 3600, "marked, its move recorded: {:?}", state.moved);
        // Nothing reads, nor a last read: no list handed to Android, which keeps its own.
        std::fs::remove_file(last_read_path(&health_path)).unwrap();
        assert_eq!(alarms_coming(), None, "Android keeps the alarms it has");
        // The alarm Android kept rings all the same, in doubt, once: recorded
        // reminded, as the alarm that reads records it (must-fix 5).
        let kept = format!("lumiel@{}", now - 20);
        let said: serde_json::Value = serde_json::from_str(&alarm_decide(&kept)).unwrap();
        assert_eq!(said["show"], true, "never silenced: {said}");
        assert!(said["body"].as_str().is_some_and(|b| b.ends_with(&tr().text("dose-last-known", None))), "{said}");
        assert!(said["again_at"].as_i64().is_some_and(|again| again > now * 1000), "looked at again: {said}");
        assert!(HealthState::read(&state_path).unwrap().reminded.contains_key(&kept), "recorded reminded: other devices know");
        let again: serde_json::Value = serde_json::from_str(&alarm_decide(&kept)).unwrap();
        assert_eq!(again["show"], false, "shown once: {again}");
        // Past the Porch's hours, or a time the recorded move replaced: no more.
        for gone in [format!("lumiel@{}", now - (MISSED_HOURS + 1) * 3600), format!("mirel@{}", now + 3600)] {
            let said: serde_json::Value = serde_json::from_str(&alarm_decide(&gone)).unwrap();
            assert_eq!((said["show"].as_bool(), said["again_at"].as_i64()), (Some(false), Some(0)), "{gone}: {said}");
        }
        // Its late question opens from the key alone, and its Taken is written, said unmoved.
        let info: serde_json::Value = serde_json::from_str(&dose_info(&kept)).unwrap();
        assert_eq!(info["name"], "lumiel", "{info}");
        let late = Zoned::now().checked_sub(Span::new().minutes(1)).unwrap().strftime("%H:%M").to_string();
        assert_eq!(taken_late(&kept, &late), "");
        assert!(HealthState::read(&state_path).unwrap().taken.contains_key(&kept), "never lost");
        assert_eq!(unmoved_note(), Some(tr().text("dose-next-unmoved", None)));
        // A dose marked on another device: not shown, never asked again.
        let marked = format!("lumiel@{}", now - 10);
        HealthState::update(&state_path, now, |state| {
            state.taken.insert(marked.clone(), now - 5);
        })
        .unwrap();
        let said: serde_json::Value = serde_json::from_str(&alarm_decide(&marked)).unwrap();
        assert_eq!((said["show"].as_bool(), said["again_at"].as_i64()), (Some(false), Some(0)), "{said}");
    }

    /// The check of 36002ea, must-fix 2, on two devices: the desk marks an
    /// every-six-hours dose late and moves the next ones; the phone, whose
    /// health file does not read, rings and marks the times the desk holds,
    /// its last read mended from the record (nothing written), and says when
    /// the dose was last taken, from when the next may be (the safety net).
    #[test]
    fn the_last_read_follows_the_record_on_two_devices() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Timestamp::now().as_second();
        let from = now - 2 * 3600;
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, format!("[[medicine]]\nid = \"corvan\"\nname = \"Corvan\"\n[medicine.schedule]\nevery = \"hours\"\nhours = 6\nfrom = {from}\n")).unwrap();
        assert_eq!(load_said().0.medicines.len(), 1, "read: its copy kept");
        std::fs::write(&health_path, "[[medicine]\nid = ").unwrap();
        // The desk's Taken an hour late, come here: its mark and its move.
        let (first, moved_to) = (format!("corvan@{from}"), from + 3600 + 6 * 3600);
        HealthState::update(&state_path, now, |state| {
            state.taken.insert(first.clone(), from + 3600);
            state.moved.insert(first.clone(), [from, moved_to]);
        })
        .unwrap();
        // Here: the times the desk holds, the file and its copy as they were.
        let copy = std::fs::read(last_read_path(&health_path)).unwrap();
        assert!(load().medicines.iter().any(|m| m.schedule == Schedule::Hours { hours: 6, from: moved_to }), "mended in memory");
        assert_eq!(std::fs::read(last_read_path(&health_path)).unwrap(), copy, "nothing written");
        let coming: Vec<serde_json::Value> = serde_json::from_str(&alarms_coming().unwrap()).unwrap();
        let keys: Vec<&str> = coming.iter().filter_map(|a| a["key"].as_str()).collect();
        assert!(keys.contains(&format!("corvan@{moved_to}").as_str()) && !keys.contains(&format!("corvan@{}", from + 6 * 3600).as_str()), "{keys:?}");
        // The safety net: the next dose said with the time the last was taken, and from when the next may be.
        let next = format!("corvan@{moved_to}");
        let said = taken_lately(&next, &load(), &record(), &Zoned::now()).expect("taken an hour ago, every six hours");
        let hour = |at: i64| Timestamp::from_second(at).unwrap().to_zoned(TimeZone::system()).strftime("%H:%M").to_string();
        assert_eq!(said, say("dose-taken-lately", &[("time", hour(from + 3600)), ("next", hour(from + 3600 + 6 * 3600))]));
        assert_eq!(taken_lately(&format!("lumiel@{now}"), &load(), &record(), &Zoned::now()), None, "a medicine at set times: none");
    }

    /// The check of 36002ea, A2: a health file of no bytes after a good read
    /// (a crash before its bytes reached the disk) was lost: the doses come
    /// from its last read, said so, nothing is written over it, and a copy of
    /// no bytes is never kept nor read as no medicines.
    #[test]
    fn a_health_file_of_no_bytes_gives_its_last_read() {
        let _files = health_files();
        let health_path = Health::default_path();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, "[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\"]\n").unwrap();
        assert!(!load_said().1);
        std::fs::write(&health_path, "").unwrap();
        let (health, last_known) = load_said();
        assert!(last_known && health.medicines.len() == 1, "lost: its last read");
        assert_eq!(set_setting("movement.minutes", "5"), tr().text("health-not-written", None));
        assert_eq!(std::fs::read(&health_path).unwrap(), b"", "never written over");
        let shown: serde_json::Value = serde_json::from_str(&page(&Waiting::default())).unwrap();
        assert_eq!(shown["unreadable"], tr().text("health-last-known", None).as_str());
        // A copy of no bytes: never read as no medicines; nothing reads, said.
        std::fs::write(last_read_path(&health_path), "").unwrap();
        assert!(last_read(&health_path).is_none() && load_known().is_none());
        let shown: serde_json::Value = serde_json::from_str(&page(&Waiting::default())).unwrap();
        assert_eq!(shown["unreadable"], tr().text("health-unreadable", None).as_str(), "as last known only when its copy is used");
        let porch: serde_json::Value = serde_json::from_str(&missed()).unwrap();
        assert_eq!(porch["note"], tr().text("health-unreadable", None).as_str(), "the Porch says it");
        let card = card_doses(&Zoned::now());
        assert!(card.first().is_some_and(|d| d.name == tr().text("health-card-unread", None) && d.time.is_empty()), "the home card says it");
    }

    /// Health's week reads the calendars as every agenda does: an event
    /// whose deletion waits for its "Undo" is not in it, and comes back with
    /// "Undo" (`pim::without_waiting`).
    #[test]
    fn health_s_week_leaves_out_an_event_waiting_for_undo() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        use sioul_core::vdir::{self, Kind};
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Health week waiting", None, &["VEVENT"]).unwrap();
        let monday: Date = "2031-03-03".parse().unwrap();
        let file = calendar.dir.join("visit.ics");
        std::fs::write(&file, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:visit-1\r\nDTSTART:20310304T090000Z\r\nDTEND:20310304T100000Z\r\nSUMMARY:Visit\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let zone = TimeZone::system();
        let shown = |waiting: &Waiting| week_events(monday, &zone, waiting).iter().any(|o| o.uid == "visit-1");
        assert!(shown(&Waiting::default()));
        let waiting: Waiting = ([file.clone()].into(), Default::default());
        assert!(!shown(&waiting), "its deletion waits: left out at once");
        assert!(shown(&Waiting::default()), "undone: back");
        // Its deletion written: the week read again at once, never the event back for five minutes (the check of 36002ea, low 23).
        std::fs::remove_file(&file).unwrap();
        crate::mail::pim_work_ended();
        assert!(!shown(&Waiting::default()), "deleted: gone at once");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

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
        let porch = serde_json::to_value(porch_doses(&health, &state, &DoseRecords::default(), &DoseReminders::default(), "desk-id", &alone, &evening, false, true, &english)).unwrap();
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
        let shown = |state: &HealthState, knowledge: &Knowledge, now: &Zoned, silent: bool, heard: bool| serde_json::to_value(porch_doses(&health, state, &DoseRecords::default(), &DoseReminders::default(), "desk-id", knowledge, now, silent, heard, &english)).unwrap();
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
        note_minute(&mut state, &[], std::slice::from_ref(&a), &[], &[], at("2026-10-06T08:00:30[Europe/Paris]").timestamp().as_second());
        assert!(state.reminded.is_empty() && state.reminder_unshown(&a));
        // Demo B, at 07:00, due while Sioul ran nowhere: the usual question, first.
        let view = serde_json::to_value(porch_doses(&health, &state, &DoseRecords::default(), &DoseReminders::default(), "desk-id", &alone, &noon, false, true, &english)).unwrap();
        let closed = view["closed"].as_array().unwrap();
        assert_eq!(closed.len(), 2, "{view}");
        assert_eq!((closed[0]["key"].as_str(), closed[0]["question"].as_str()), (Some(b.as_str()), Some("Due while Sioul was closed, and marked nowhere Sioul can see: did you take them?")));
        // Demo A, Sioul running: still asked about, under what happened.
        assert_eq!(closed[1]["key"].as_str(), Some(a.as_str()));
        let question = closed[1]["question"].as_str().unwrap();
        assert!(question.starts_with("Due while Sioul ran, but their reminder could not be shown") && !question.contains("closed"), "{question}");
        assert!(view["due"].as_array().unwrap().iter().all(|row| row["question"] == ""), "today's doses carry no question");
        // The Health page's list says the same.
        let rows = missed_rows(&health, &state, &DoseReminders::default(), &alone, &noon, &french);
        assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), [b.as_str(), a.as_str()]);
        assert!(rows[1].question.starts_with("Prévus pendant que Sioul tournait, mais leur rappel n’a pas pu s’afficher"), "{}", rows[1].question);
        // Another device not heard since: the doubt said under both, never "not taken".
        let heard = at("2026-10-06T06:30[Europe/Paris]").timestamp().as_second();
        let unsure = Knowledge { record_lost: None, peers: vec![Peer { name: "laptop".into(), known_until: heard, closed: false, delay: Some(30), broken: None, heard, ..Peer::default() }] };
        for row in missed_rows(&health, &state, &DoseReminders::default(), &unsure, &noon, &english) {
            assert!(row.doubt.starts_with("Sioul can't tell whether it was taken") && !row.doubt.to_lowercase().contains("not taken"), "{}", row.doubt);
        }
        // The notification's title: one kind, the other, both.
        let missed = state.unanswered(&health, &noon, MISSED_HOURS, GRACE_MINUTES);
        assert_eq!(missed_title(&missed, &state, &DoseReminders::default(), &english), "Doses due earlier");
        assert_eq!(missed_title(&missed[1..], &state, &DoseReminders::default(), &english), "A reminder could not be shown");
        assert_eq!(missed_title(&missed[..1], &state, &DoseReminders::default(), &english), "While Sioul was closed");
        assert_eq!(missed_title(&missed[1..], &state, &DoseReminders::default(), &french), "Un rappel n’a pas pu s’afficher");
        // Reminded later (another device, or notifications back): no question at all.
        note_minute(&mut state, std::slice::from_ref(&a), &[], &[], &[], at("2026-10-06T08:05[Europe/Paris]").timestamp().as_second());
        let rows = missed_rows(&health, &state, &DoseReminders::default(), &alone, &noon, &english);
        assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), [b.as_str()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A dose's reminder is recorded as what became of it (review of battery
    /// part C, the third and fourth reviews' H1): the tick waits for each
    /// notice's first try (`remind_dose`, `Sending::outcome`, `FIRST_TRY` at
    /// most in all, the record let go meanwhile); shown, a reminder; surely
    /// not shown, `unshown`; sent and not known shown, `maybe` (with `unshown`
    /// beside it, for an older Sioul). Shown late, it becomes a reminder then,
    /// only while nothing was said of it meanwhile: marked here, answered on
    /// another device, or reminded by one (the phone's alarm) leaves it as it
    /// is, never contradicted, never downgraded; a try after the first sent
    /// and not answered in time is recorded maybe.
    #[test]
    fn a_dose_reminder_counts_once_shown_and_late_only_if_still_to_remind() {
        let key = "mag@1791360000".to_string();
        let (state, none) = (HealthState::default(), DoseRecords::default());
        assert!(late_reminder_wanted(&state, &none, &key), "nothing said of it: shown late");
        let mut taken = HealthState::default();
        taken.taken.insert(key.clone(), 1_791_360_100);
        assert!(!late_reminder_wanted(&taken, &none, &key), "taken meanwhile");
        let mut skipped = HealthState::default();
        skipped.not_taken.insert(key.clone(), 1_791_360_100);
        assert!(!late_reminder_wanted(&skipped, &none, &key), "said not taken");
        let mut phone = HealthState::default();
        phone.unshown.insert(key.clone(), 1_791_360_005);
        phone.reminded.insert(key.clone(), 1_791_360_600);
        assert!(!late_reminder_wanted(&phone, &none, &key), "the phone's alarm reminded it meanwhile: its turn kept");
        let mut records = DoseRecords::default();
        records.answer(&key, "phone", Answer { state: sioul_core::doses::TAKEN.into(), at: 1_791_360_300, noted: 1_791_360_300, name: "phone".into(), kind: "phone".into() });
        assert!(!late_reminder_wanted(&state, &records, &key), "answered on the phone, its mark not here yet");
        // Shown late: a reminder then, its first try's record kept; nothing over an earlier word.
        let mut first = HealthState::default();
        first.unshown.insert(key.clone(), 1_791_360_005);
        assert!(first.reminder_unshown(&key));
        late_reminded(&mut first, &key, 1_791_360_040);
        assert_eq!(first.reminded.get(&key), Some(&1_791_360_040));
        assert!(!first.reminder_unshown(&key), "a reminder now: not asked as one that could not be shown");
        // Recorded, it is no longer to remind: a late show reads its words
        // before it records (`notify::shown_late`), or it would take itself away.
        assert!(!late_reminder_wanted(&first, &none, &key), "recorded: no longer to remind");
        let mut earlier = phone.clone();
        late_reminded(&mut earlier, &key, 1_791_360_700);
        assert_eq!(earlier.reminded.get(&key), Some(&1_791_360_600), "the phone's reminder kept");
        let mut marked = taken.clone();
        late_reminded(&mut marked, &key, 1_791_360_700);
        assert!(marked.reminded.is_empty(), "nothing recorded over a mark");
        // The minute's record: a reminder sent and not known shown is written
        // `unshown` for an older Sioul (what each device did is in its own file);
        // answered late, a reminder.
        let mut minute = HealthState::default();
        note_minute(&mut minute, &[], &[], std::slice::from_ref(&key), &[], 1_791_360_002);
        assert!(minute.unshown.contains_key(&key) && minute.reminder_unshown(&key) && !minute.reminded.contains_key(&key));
        late_reminded(&mut minute, &key, 1_791_360_004);
        assert!(!minute.reminder_unshown(&key), "answered late: shown, upgraded");
        // The tick, read from the source: each first try's outcome waited for, the record let go, recorded as it went.
        let source = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/health.rs")).unwrap();
        let tick = source.split("pub(crate) fn tick(qt: &QtThread, shared: &Arc<Shared>) {").nth(1).and_then(|rest| rest.split("\n}\n").next()).unwrap();
        assert!(tick.contains("sioul_sync::notify::remind_dose(") && !tick.contains("sioul_sync::notify::remind(&title, &body"), "a dose's reminder says what became of it");
        let waited = tick.split("drop(_held);").nth(1).unwrap();
        assert!(waited.contains("sent.outcome(deadline.saturating_duration_since("));
        for (outcome, list) in [("Outcome::Shown => reminded.push(key)", ""), ("Outcome::Unshown(e) => {\n                unshown.push(key);", ""), ("Outcome::Maybe(e) => {\n                maybe.push(key);", "")] {
            assert!(waited.contains(outcome), "{outcome}{list}");
        }
        assert!(waited.find("let _held = state_held();").unwrap() < waited.find("note_minute(record, &reminded, &unshown, &maybe").unwrap(), "recorded under the record's lock again");
    }

    /// The phone's alarm, at a dose's time plus `WAIT_FOR_NEWS` (the desk kept
    /// the doses), as it holds what the others did (review of battery part C,
    /// the fifth review: THE RULE): plain only on the desk's own word that it
    /// did not remind the dose (its first try not gone out, then its promise
    /// of quiet past the alarm; or closed, everything read); every other way,
    /// the doubt first, each device said; another phone always a doubt (its
    /// own alarm). In English and French, the French reasons joined with a
    /// thin space before ";".
    #[test]
    fn a_phone_reminds_plainly_only_on_the_others_word() {
        use sioul_core::dose_reminders::{QUIET_AFTER, others};
        use sioul_core::health::Said;
        let due: i64 = 1_791_360_000;
        let key = format!("levo@{due}");
        let at_alarm = due + WAIT_FOR_NEWS;
        let (english, french) = (Translator::new("en"), Translator::new("fr"));
        let desk = |exported: i64, working: bool| Peer {
            name: "desk".into(),
            id: "desk-id".into(),
            said: Some(Said { started: due - 10_800, closed: if working { 0 } else { exported }, working, exported, imported: exported, doses: true, left: false, phone: false }),
            complete: true,
            seen: exported + 5,
            heard: exported,
            ..Peer::default()
        };
        let in_use = [desk(at_alarm - 30, true)];
        let with = |tried: Tried| {
            let mut told = DoseReminders::default();
            *told.entry(&key, "desk-id") = tried;
            told
        };
        let said_as = |told: &DoseReminders, peers: &[Peer], words: &Translator| match others(&key, due, at_alarm, "phone-id", true, Some(told), peers, true) {
            Others::Doubt(e) => Some(elsewhere_said(&e, due, words)),
            Others::First => None,
            Others::Reminded => Some("reminded".into()),
        };
        // H1: the desk's notice went and was not known shown: "may have shown".
        let sent = with(Tried { maybe: due + 2, ..Tried::default() });
        let when = |at: i64, words: &Translator| when_said(&Timestamp::from_second(at).unwrap().to_zoned(TimeZone::system()), words);
        assert_eq!(said_as(&sent, &in_use, &english), Some(format!("desk may have shown its reminder {}", when(due + 2, &english))));
        assert_eq!(said_as(&sent, &in_use, &french), Some(format!("desk a peut-être affiché son rappel {}", when(due + 2, &french))));
        // Shown (answered late): reminded, the alarm silent.
        assert_eq!(said_as(&with(Tried { maybe: due + 2, shown: due + 3, ..Tried::default() }), &in_use, &english).as_deref(), Some("reminded"));
        // H2: not gone out, its tries going on (no promise yet): "could not show its reminder …, and may have shown it since".
        let tried = with(Tried { unshown: due + 5, ..Tried::default() });
        assert!(said_as(&tried, &in_use, &english).is_some_and(|w| w.contains("could not show its reminder") && w.contains("may have shown it since")));
        // Not gone out, then quiet past the alarm, promised: the alarm is the first, plainly.
        let promised = with(Tried { unshown: due + 5, quiet: due + QUIET_AFTER, ..Tried::default() });
        assert_eq!(said_as(&promised, &in_use, &english), None);
        // In use, no word of its own on the dose read here: "was in use and may have reminded you".
        assert!(said_as(&DoseReminders::default(), &in_use, &english).is_some_and(|w| w.starts_with("desk was in use and may have reminded you")));
        // Closed before the dose, everything read: did not; plainly.
        assert_eq!(said_as(&DoseReminders::default(), &[Peer { acknowledged: due - 10_800, ..desk(due - 600, false) }], &english), None, "its opening, the one this phone acknowledged");
        // C1: two phones; the other one's own alarm may ring: a doubt, named, whatever the desk promised.
        let tablet = Peer { name: "tablet".into(), id: "tablet-id".into(), said: Some(Said { phone: true, ..desk(due - 600, false).said.unwrap() }), ..desk(due - 600, false) };
        assert_eq!(said_as(&promised, &[in_use[0].clone(), tablet.clone()], &english).as_deref(), Some("tablet may have reminded you too, by its own alarm"));
        assert_eq!(said_as(&promised, &[in_use[0].clone(), tablet], &french).as_deref(), Some("tablet vous l’a peut-être rappelée aussi, par sa propre alarme"));
        // Two reasons, joined: in French with a thin space before ";".
        let two = elsewhere_said(&[Elsewhere::Unknown { device: Named { id: "a".into(), name: "desk".into(), phone: false } }, Elsewhere::Alarm { device: Named { id: "b".into(), name: "tablet".into(), phone: false } }], due, &french);
        assert_eq!(two, format!("desk était utilisé et vous l’a peut-être rappelée {}\u{202f}; tablet vous l’a peut-être rappelée aussi, par sa propre alarme", when(due, &french)));
        assert_eq!(joined(vec!["a".into(), "b".into()], &english), "a; b");
        assert_eq!(joined(vec!["a".into(), "b".into()], &french), "a\u{202f}; b");
        // The file not read here: nothing known.
        assert!(matches!(others(&key, due, at_alarm, "phone-id", true, None, &in_use, true), Others::Doubt(ref e) if e == &[Elsewhere::Unread]));
    }

    /// What the desk does for a dose at its tick (`plan`; the fifth review's
    /// C1, M1, M2): its first reminder only where it kept the doses at the
    /// dose's time, within its window; with a phone sharing the doses
    /// (one or two: C1), none in the quiet around the phone's alarm, promised;
    /// after it, its tries go on, the doubt first, whether the phone rang or
    /// not (M1), until the grace's end; at waking, counted from waking (M2).
    #[test]
    fn a_computer_reminds_first_only_when_it_is_and_keeps_its_word() {
        use sioul_core::dose_reminders::{NEVER, QUIET_AFTER};
        use sioul_sync::notify::Late;
        let since: i64 = 1_791_360_000;
        let unshown = Some(Tried { unshown: since + 5, ..Tried::default() });
        // Kept the doses then, nothing recorded: its first, within its window.
        assert_eq!(plan(None, true, true, since, since + 30, true), Plan::First);
        assert_eq!(plan(None, true, true, since, since + LATE_UNTIL - 1, true), Plan::First);
        assert_eq!(plan(None, true, true, since, since + WAIT_FOR_NEWS - 1, false), Plan::First, "no phone: its window is the wait for news");
        // Past its window with a phone: quiet until after its alarm, promised.
        assert_eq!(plan(None, true, true, since, since + LATE_UNTIL, true), Plan::Quiet(since + QUIET_AFTER));
        // Kept only since after the dose's time (a restart, the lease moved): never its first.
        assert_eq!(plan(None, true, false, since, since + 60, true), Plan::Quiet(since + QUIET_AFTER));
        assert_eq!(plan(None, true, false, since, since + 60, false), Plan::Late(None));
        // Its first try not gone out: tries, the doubt first; quiet in the phone's window, promised;
        // after it, the tries again, the doubt first, whether the phone rang or not (M1).
        assert_eq!(plan(unshown, true, true, since, since + 120, true), Plan::Late(Some(Late::Retry)));
        assert_eq!(plan(unshown, true, true, since, since + LATE_UNTIL + 10, true), Plan::Quiet(since + QUIET_AFTER));
        let paused = Some(Tried { unshown: since + 5, quiet: since + QUIET_AFTER, ..Tried::default() });
        assert_eq!(plan(paused, true, true, since, since + QUIET_AFTER - 1, true), Plan::Not, "its promise kept");
        assert_eq!(plan(paused, true, true, since, since + QUIET_AFTER + 60, true), Plan::Late(Some(Late::Retry)));
        // Two phones count as phones (C1): the quiet holds as for one.
        assert_eq!(plan(unshown, true, true, since, since + 8 * 60, true), Plan::Quiet(since + QUIET_AFTER));
        // An earlier Sioul here sent it, not known shown: unconfirmed, the doubt first; shown, or never again: nothing.
        assert_eq!(plan(Some(Tried { maybe: since + 2, ..Tried::default() }), true, true, since, since + 120, true), Plan::Late(Some(Late::Unconfirmed)));
        assert_eq!(plan(Some(Tried { maybe: since + 2, shown: since + 3, ..Tried::default() }), true, true, since, since + 120, true), Plan::Not);
        assert_eq!(plan(Some(Tried { unshown: since + 5, quiet: NEVER, ..Tried::default() }), true, true, since, since + 1_200, true), Plan::Not);
        // The file not read: a first try only.
        assert_eq!(plan(None, false, true, since, since + 30, true), Plan::First);
        assert_eq!(plan(None, false, false, since, since + 120, true), Plan::Quiet(since + QUIET_AFTER), "the file not read, not its first: quiet around the phone's alarm");
        assert_eq!(plan(None, false, true, since, since + QUIET_AFTER + 60, true), Plan::Late(None), "then the doubt first, its late notice going all the same (L1)");
        // The tries' words: the doubt first, why it is late first (L6: a notice on the screen since its time, answered late, "was not confirmed in time").
        let dose = sioul_core::health::Dose { key: format!("levo@{since}"), medicine: "levo".into(), name: "Levothyroxine".into(), dose: "75 µg".into(), at: Timestamp::from_second(since).unwrap().to_zoned(TimeZone::system()) };
        let alone = Knowledge { record_lost: None, peers: Vec::new() };
        let (title, retry) = late_said(&dose, &alone, since + 120, since, Some(Late::Retry));
        assert_eq!(title, say("dose-check-title", &[("dose", "Levothyroxine · 75 µg".to_string())]));
        assert!(retry.contains(&say("dose-doubt-late", &[("when", moment_said(since))])), "{retry}");
        let (_, unconfirmed) = late_said(&dose, &alone, since + 120, since, Some(Late::Unconfirmed));
        assert!(unconfirmed.contains(&say("dose-doubt-unconfirmed", &[("when", moment_said(since))])) && !unconfirmed.contains(&say("dose-doubt-late", &[("when", moment_said(since))])), "{unconfirmed}");
        let (_, other) = late_said(&dose, &alone, since + 120, since, None);
        assert!(other.contains(&say("dose-doubt-other", &[("when", moment_said(since))])), "{other}");
        // In English, as L6 asks.
        assert_eq!(said(&Translator::new("en"), "dose-doubt-unconfirmed", &[("when", "at 08:00".to_string())]), "its reminder, due at 08:00, was not confirmed in time, and it may have been taken since without being marked");
    }

    /// The sixth review's timelines R1 to R9, each device's decision as the
    /// tick (`first_here`, `plan`) and the phone's alarm (`dose_reminders::others`)
    /// compose it from what that device has read then: never two plain
    /// reminders of one dose. The handshake: the desk is the first only once
    /// every phone said, before the dose's time, that it read its current
    /// opening; else quiet around the phone's alarm, then the doubt first,
    /// the phone named. The phone takes the desk's "closed" as its word only
    /// after its own pull went through since the dose, and nothing later read
    /// of the desk.
    #[test]
    fn a_desk_opened_just_before_a_dose_is_never_a_second_first() {
        use sioul_core::dose_reminders::{QUIET_AFTER, others};
        use sioul_core::health::Said;
        let due: i64 = 1_791_360_000; // 08:00
        let key = format!("levo@{due}");
        let english = Translator::new("en");
        // The desk as the phone last read it: closed at 23:00, everything read.
        let closed_desk = |claimed: i64| Peer {
            name: "desk".into(),
            id: "desk-id".into(),
            said: Some(Said { started: due - 36_000, closed: due - 32_400, working: false, exported: due - 32_400, imported: due - 32_400, doses: true, left: false, phone: false }),
            complete: true,
            seen: due - 32_400,
            heard: due - 32_400,
            claimed,
            // This phone's own word on that opening (its sealed entry): its "closed" is its word only for it, or later.
            acknowledged: due - 36_000,
            ..Peer::default()
        };
        let phone = Peer {
            name: "phone".into(),
            id: "phone-id".into(),
            said: Some(Said { started: due - 7_200, closed: due - 3_600, working: false, exported: due - 3_600, imported: due - 3_600, doses: true, left: false, phone: true }),
            complete: true,
            seen: due - 3_600,
            heard: due - 3_600,
            ..Peer::default()
        };
        let desk_knows = Knowledge { record_lost: None, peers: vec![phone.clone()] };
        let nothing = DoseReminders::default();
        let opened = due - 30; // 07:59:30
        let mut acks = std::collections::BTreeMap::new();
        // R1: the desk opened at 07:59:30; the phone read its opening only at 08:02 (its notes said last night's).
        acks.insert("phone-id".to_string(), due - 36_000);
        let lead = word_lead(&desk_knows);
        assert!(!all_acknowledged(&desk_knows, opened, &acks, due));
        let acked = acked_after(None, all_acknowledged(&desk_knows, opened, &acks, due), opened, due + 5);
        let first = first_here(false, true, opened, acked.map(|(_, at)| at), due, lead);
        assert!(!first, "the phone has not said it read this opening: not the first");
        assert_eq!(plan(None, true, first, due, due + 5, true), Plan::Quiet(due + QUIET_AFTER), "R1: quiet around the phone's alarm, promised");
        // The phone at 08:00:05, keeping the doses itself: the desk closed, its own pull through: the first, plainly.
        assert_eq!(others(&key, due, due + 5, "phone-id", true, Some(&nothing), &[closed_desk(0)], true), Others::First);
        // The desk after the quiet: the doubt first, the phone named.
        assert_eq!(plan(Some(Tried { quiet: due + QUIET_AFTER, ..Tried::default() }), true, first, due, due + QUIET_AFTER + 60, true), Plan::Late(None));
        let from_desk = match others(&key, due, due + QUIET_AFTER + 60, "desk-id", false, Some(&nothing), std::slice::from_ref(&phone), false) {
            Others::Doubt(e) => elsewhere_said(&e, due, &english),
            other => panic!("{other:?}"),
        };
        assert_eq!(from_desk, "the phone may have reminded you too, by its own alarm");
        // R1b, R1c: a network slow this time, a restart: the same: its new opening said by no phone before 08:00.
        acks.insert("phone-id".to_string(), opened);
        let late_ack = acked_after(None, all_acknowledged(&desk_knows, opened, &acks, due), opened, due + 245);
        assert!(!first_here(false, true, opened, late_ack.map(|(_, at)| at), due, lead), "said at 08:04: too late for this dose");
        let early = acked_after(None, all_acknowledged(&desk_knows, opened, &acks, due), opened, due - lead - 10);
        assert!(first_here(false, true, opened, early.map(|(_, at)| at), due, lead), "said before 08:00 (and the clocks' lead): the desk is the first; the phone then knows it open");
        let just = acked_after(None, true, opened, due - 10);
        assert!(!first_here(false, true, opened, just.map(|(_, at)| at), due, lead), "said ten seconds before the dose: a phone ahead may have decided already (L1)");
        assert_eq!(acked_after(early, true, opened, due + 600), early, "kept for the same opening");
        assert_eq!(acked_after(early, true, opened + 60, due + 600), Some((opened + 60, due + 600)), "a new opening, said anew");
        // R1d: the phone read the desk's claim (renewed at 07:59:30, open) before its entry: not its word.
        match others(&key, due, due + 600, "phone-id", true, Some(&nothing), &[closed_desk(opened)], true) {
            Others::Doubt(e) => assert_eq!(elsewhere_said(&e, due, &english), format!("desk closed {}, and its news can be slow to come", when_said(&Timestamp::from_second(due - 32_400).unwrap().to_zoned(TimeZone::system()), &english))),
            other => panic!("{other:?}"),
        }
        // The phone's own pull not through since the dose: not its word either.
        assert!(matches!(others(&key, due, due + 600, "phone-id", true, Some(&nothing), &[closed_desk(0)], false), Others::Doubt(_)));
        // R2: the key not read, the claim or the opening not written: never the first, whatever it kept.
        assert!(!first_here(false, false, due - 3_600, Some(due - 3_600), due, lead));
        // R3: a phone in the folder, counted or not (silent a while): quiet all the same, and its word asked (C1b).
        let silent_phone = Peer { seen: due - 30 * 86_400, said: Some(Said { exported: due - 30 * 86_400, ..phone.said.clone().unwrap() }), ..phone.clone() };
        assert!(phone_alarms(&Knowledge { record_lost: None, peers: vec![silent_phone.clone()] }) && must_acknowledge(&Knowledge { record_lost: None, peers: vec![silent_phone] }).len() == 1);
        // R5: the doses kept elsewhere a moment: nothing promised; back within the grace, tried again, the doubt first.
        assert_eq!(plan(Some(Tried { unshown: due + 5, ..Tried::default() }), true, true, due, due + 180, true), Plan::Late(Some(sioul_sync::notify::Late::Retry)));
        // R9: alone in the folder (no other device at all), started at 08:00:02: the first, plainly.
        assert!(first_here(true, true, due + 2, None, due, lead));
        assert_eq!(plan(None, true, true, due, due + 2, false), Plan::First);
    }

    /// The seventh review's timelines: the word asked of every phone in the
    /// folder not gone (counted or not, said off or not, silent or not), and
    /// dropped by a phone that stops counting the computer; a computer back
    /// from a gap opens anew; a computer never takes another's "closed"; a
    /// word names this very opening or says nothing; a device without an
    /// entry is never "in use".
    #[test]
    fn every_phone_not_gone_must_say_it_read_this_opening() {
        use sioul_core::dose_reminders::{QUIET_AFTER, others};
        use sioul_core::health::Said;
        let due: i64 = 1_791_360_000; // 08:00
        let key = format!("levo@{due}");
        let english = Translator::new("en");
        let phone = |seen: i64, off: bool, forgotten: bool| Peer {
            name: "phone".into(),
            id: "phone-id".into(),
            said: Some(Said { started: seen - 600, closed: seen, working: false, exported: seen, imported: seen, doses: true, left: false, phone: true }),
            complete: true,
            seen,
            heard: seen,
            off: off || forgotten,
            forgotten,
            ..Peer::default()
        };
        let opened = due - 3 * 86_400; // the laptop's opening, three days old
        let acks = |opening: i64| [("phone-id".to_string(), opening)].into_iter().collect::<std::collections::BTreeMap<String, i64>>();
        // C1a: the laptop said off on the phone while asleep: the phone drops its word (it no longer counts it);
        // the laptop, woken, asks the word of the phone all the same: not the first.
        let off_there = phone(due - 3_600, false, false);
        let knows = Knowledge { record_lost: None, peers: vec![off_there.clone()] };
        assert!(!all_acknowledged(&knows, opened, &std::collections::BTreeMap::new(), due), "no word: not the first");
        // C1b, C1c: the phone silent eight days here, or said off here: its word asked all the same.
        for phone in [phone(due - 8 * 86_400, false, false), phone(due - 3_600, true, false)] {
            let fresh = due - phone.seen <= WORD_FRESH;
            let knows = Knowledge { record_lost: None, peers: vec![phone] };
            assert_eq!(must_acknowledge(&knows).len(), 1);
            assert!(!all_acknowledged(&knows, opened, &std::collections::BTreeMap::new(), due));
            assert_eq!(all_acknowledged(&knows, opened, &acks(opened), due), fresh, "its word on this very opening, while it is heard here (N2)");
        }
        // Forgotten here, or stopped sharing: no word owed.
        assert!(must_acknowledge(&Knowledge { record_lost: None, peers: vec![phone(due - 3_600, false, true)] }).is_empty());
        let left = Peer { said: Some(Said { left: true, ..phone(due - 3_600, false, false).said.unwrap() }), ..phone(due - 3_600, false, false) };
        assert!(must_acknowledge(&Knowledge { record_lost: None, peers: vec![left] }).is_empty());
        // An older or unknown device (no entry): its word never comes: never the first.
        let older = Peer { id: "old-id".into(), name: "FP5".into(), heard: due - 600, ..Peer::default() };
        assert!(!all_acknowledged(&Knowledge { record_lost: None, peers: vec![older.clone()] }, opened, &acks(opened), due));
        // M1: a word naming another opening, later or impossible, says nothing.
        let knows = Knowledge { record_lost: None, peers: vec![phone(due - 3_600, false, false)] };
        assert!(!all_acknowledged(&knows, opened, &acks(opened + 3_000), due));
        assert!(!all_acknowledged(&knows, opened, &acks(i64::MAX), due));
        // C1c: a laptop asleep (no tick for a while) opens anew at waking: the old word is for another opening.
        assert!(woke_from_gap(due - 600, due - 30) && !woke_from_gap(due - 90, due - 30) && !woke_from_gap(0, due), "three minutes without a tick: a new opening");
        // C1d: a desk not first (the phone not counted there): quiet, then never plain (C2).
        assert_eq!(plan(None, true, false, due, due + 180, true), Plan::Quiet(due + QUIET_AFTER));
        assert_eq!(plan(Some(Tried { quiet: due + QUIET_AFTER, ..Tried::default() }), true, false, due, due + QUIET_AFTER + 60, true), Plan::Late(None));
        // H1 (C2): two computers, no phone: another computer's "closed" is never the desk's proof.
        let laptop = Peer {
            name: "laptop".into(),
            id: "laptop-id".into(),
            said: Some(Said { started: due - 36_000, closed: due - 32_400, working: false, exported: due - 32_400, imported: due - 32_400, doses: true, left: false, phone: false }),
            complete: true,
            seen: due - 32_400,
            heard: due - 32_400,
            acknowledged: due - 36_000,
            ..Peer::default()
        };
        match others(&key, due, due + 180, "desk-id", false, Some(&DoseReminders::default()), std::slice::from_ref(&laptop), true) {
            Others::Doubt(e) => assert!(elsewhere_said(&e, due, &english).starts_with("laptop closed "), "{e:?}"),
            other => panic!("{other:?}"),
        }
        // L3: a device without an entry, named: it may have reminded by its own alarm, never "was in use".
        match others(&key, due, due + 600, "phone-id", true, Some(&DoseReminders::default()), std::slice::from_ref(&older), true) {
            Others::Doubt(e) => assert_eq!(elsewhere_said(&e, due, &english), "FP5 may have reminded you too, by its own alarm"),
            other => panic!("{other:?}"),
        }
    }

    /// The eighth review's timelines. N1: a desk that runs, offline from
    /// 06:00, said off on the phone at 07:05: the phone's word, taken back
    /// there, still names the desk's opening here, but the phone was last
    /// heard here at 05:50: past `WORD_FRESH` its word says nothing, the desk
    /// is not the dose's first (quiet, then the doubt). N2: the sharing down
    /// both ways eight days: the same. N3: the phone's word never goes back:
    /// an older copy of the desk's entry put back is not its word ("closed"):
    /// what the desk says comes from its newest copy (in use since 07:30).
    #[test]
    fn a_phones_word_counts_while_it_is_heard_and_never_goes_back() {
        use sioul_core::dose_reminders::{QUIET_AFTER, others};
        use sioul_core::health::Said;
        let due: i64 = 1_791_360_000; // 08:00
        let key = format!("levo@{due}");
        let english = Translator::new("en");
        let opened = due - 82_800; // the desk opened yesterday at 09:00, acknowledged then
        let phone = |seen: i64| Peer {
            name: "phone".into(),
            id: "phone-id".into(),
            said: Some(Said { started: seen - 600, closed: seen, working: false, exported: seen, imported: seen, doses: true, left: false, phone: true }),
            complete: true,
            seen,
            heard: seen,
            ..Peer::default()
        };
        let word = [("phone-id".to_string(), opened)].into_iter().collect::<std::collections::BTreeMap<String, i64>>();
        // Heard an hour ago: its word counts.
        assert!(all_acknowledged(&Knowledge { record_lost: None, peers: vec![phone(due - 3_600)] }, opened, &word, due));
        // N1: last heard at 05:50, the desk offline since 06:00: not the first at 08:00, quiet then the doubt.
        let n1 = Knowledge { record_lost: None, peers: vec![phone(due - 7_800)] };
        assert!(!all_acknowledged(&n1, opened, &word, due));
        let acked = acked_after(Some((opened, opened + 180)), all_acknowledged(&n1, opened, &word, due), opened, due);
        assert_eq!(acked, None, "the old time forgotten with the word");
        let first = first_here(false, true, opened, acked.map(|(_, at)| at), due, word_lead(&n1));
        assert!(!first);
        assert_eq!(plan(None, true, first, due, due + 5, true), Plan::Quiet(due + QUIET_AFTER));
        // N2: eight days of silence both ways: the same.
        assert!(!all_acknowledged(&Knowledge { record_lost: None, peers: vec![phone(due - 8 * 86_400)] }, opened, &word, due));
        // N3: the desk opened at 07:30, read on the phone at 07:32; at 08:00:05 the phone is served last night's entry.
        let mut kept = Peers::default();
        let desk_entry = |started: i64, closed: i64, working: bool| sioul_sync::devices::Entry { id: "desk-id".into(), name: "desk".into(), kind: sioul_sync::devices::COMPUTER.into(), started, closed, working, exported: started + 60, doses: true, ..sioul_sync::devices::Entry::default() };
        learn(&mut kept, &[], &[desk_entry(due - 1_800, 0, true)], &sioul_sync::share::Heard::default(), due - 1_680);
        learn(&mut kept, &[], &[desk_entry(due - 32_400 - 3_600, due - 32_400, false)], &sioul_sync::share::Heard::default(), due + 5);
        let desk = kept.peers.get("desk-id").unwrap().peer.clone();
        assert_eq!(desk.latest_opening, due - 1_800, "never lowered by the older copy");
        let said = desk.said.clone().unwrap();
        assert_eq!(said.started.max(desk.latest_opening), due - 1_800, "the phone's word stays on 07:30's opening");
        assert!(said.working && said.started == due - 1_800, "what it says comes from its newest copy (the ninth review's W1)");
        let desk = Peer { acknowledged: due - 1_800, complete: true, ..desk };
        match others(&key, due, due + 5, "phone-id", true, Some(&DoseReminders::default()), std::slice::from_ref(&desk), true) {
            Others::Doubt(e) => assert!(elsewhere_said(&e, due, &english).starts_with("desk was in use and may have reminded you"), "{e:?}"),
            other => panic!("the older copy taken as its word: {other:?}"),
        }
        // L1: one reason said once, however many ways it came.
        assert_eq!(joined(vec!["desk closed".into(), "desk closed".into(), "the phone may have reminded you".into()], &english), "desk closed; the phone may have reminded you");
    }

    /// The ninth review of battery part C (15 October 2026). W1: two
    /// genuine older copies of the phone's entry, both naming the desk's
    /// opening, served in turn each hour for six hours: no news of the phone,
    /// its word expires, the desk is not the first; and the word comes from the
    /// newest copy read, never an older one put back. F1: a computer's copy
    /// dated a year ahead of this clock is no news and leaves nothing kept;
    /// an opening kept from such a clock is dropped. A `seen` in the future
    /// (this clock set back) is no news of the phone either.
    #[test]
    fn only_a_newer_copy_of_an_entry_is_news() {
        let t0: i64 = 1_791_349_200; // 05:00
        let opening = t0 - 72_000;
        let copy = |exported: i64, acked: bool| sioul_sync::devices::Entry {
            id: "phone-id".into(),
            kind: sioul_sync::devices::PHONE.into(),
            started: t0 - 3_600,
            closed: t0 - 3_000,
            exported,
            imported: exported,
            doses: true,
            acked: if acked { [("desk-id".to_string(), opening)].into_iter().collect() } else { std::collections::BTreeMap::new() },
            ..sioul_sync::devices::Entry::default()
        };
        let none = sioul_sync::share::Heard::default();
        let mut kept = Peers::default();
        // Its 05:50 restatement, read at 05:51.
        learn(&mut kept, &[], &[copy(t0 + 3_000, true)], &none, t0 + 3_060);
        // Six hours with no newer copy, the two older ones served in turn, one an hour.
        for h in 1..=6 {
            learn(&mut kept, &[], &[copy(if h % 2 == 1 { t0 - 600 } else { t0 + 3_000 }, true)], &none, t0 + 3_060 + h * 3_600);
        }
        let knows = Knowledge { record_lost: None, peers: kept.peers.values().map(|s| s.peer.clone()).collect() };
        let acks = [("phone-id".to_string(), opening)].into_iter().collect();
        let at = t0 + 3_060 + 6 * 3_600 + 60;
        assert!(!all_acknowledged(&knows, opening, &acks, at), "an older copy put back is no news of the phone");
        assert_eq!(kept.peers["phone-id"].peer.seen, t0 + 3_060, "seen when its newest copy came, not since");
        // Its word taken back in a newer copy, then the older one with the word served again: the newest copy's word.
        learn(&mut kept, &[], &[copy(t0 + 3_600 * 7, false)], &none, t0 + 3_600 * 7 + 60);
        learn(&mut kept, &[], &[copy(t0 + 3_000, true)], &none, t0 + 3_600 * 7 + 120);
        assert!(acknowledgements("desk-id", &newest_of(&kept)).is_empty(), "the word from the newest copy read, never an older one");
        // Its entry not read any more: its word, kept, says nothing.
        learn(&mut kept, &[], &[copy(t0 + 3_600 * 8, true)], &none, t0 + 3_600 * 8 + 60);
        assert_eq!(acknowledgements("desk-id", &newest_of(&kept)).get("phone-id"), Some(&opening));
        learn(&mut kept, &[], &[], &none, t0 + 3_600 * 8 + 120);
        assert!(acknowledgements("desk-id", &newest_of(&kept)).is_empty(), "a word no longer read says nothing");
        // A clock set back here: a `seen` in the future is no news of the phone.
        let fresh = Knowledge { record_lost: None, peers: vec![Peer { seen: at + 3_600, ..knows.peers[0].clone() }] };
        assert!(!all_acknowledged(&fresh, opening, &acks, at));
        // F1: the desk's entry from a clock a year ahead: no news, nothing kept; set right, counted again.
        let desk = |started: i64| sioul_sync::devices::Entry { id: "desk-id".into(), name: "desk".into(), kind: sioul_sync::devices::COMPUTER.into(), started, closed: started + 600, exported: started + 590, imported: started + 590, doses: true, ..sioul_sync::devices::Entry::default() };
        let year = 400 * 86_400;
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[desk(t0 + year)], &none, t0 + 3_660);
        assert!(kept.peers["desk-id"].entry.is_none() && kept.peers["desk-id"].peer.latest_opening == 0, "nothing kept from a clock a year ahead");
        // Settings says why it is not heard, so that its clock is set right.
        let english = Translator::new("en");
        let rows = |kept: &Peers, now: i64| device_rows_of(&kept.peers.values().map(|s| s.peer.clone()).collect::<Vec<_>>(), kept, now, &english);
        assert_eq!(rows(&kept, t0 + 3_660)[0].state, "Its clock runs more than ten minutes ahead of this one's: what it says is set aside here until its clock is set right.");
        learn(&mut kept, &[], &[desk(t0 + 4_800)], &none, t0 + 5_460);
        assert_eq!(kept.peers["desk-id"].peer.latest_opening, t0 + 4_800, "set right: its opening counts");
        let row = &rows(&kept, t0 + 5_460)[0];
        assert!(row.state.starts_with("Closed ") && row.notes.is_empty(), "{row:?}");
        // An opening kept before this rule from a clock far ahead: dropped once seen in the future.
        kept.peers.get_mut("desk-id").unwrap().peer.latest_opening = t0 + year;
        learn(&mut kept, &[], &[desk(t0 + 4_800)], &none, t0 + 5_520);
        assert_eq!(kept.peers["desk-id"].peer.latest_opening, t0 + 4_800);
    }

    /// The final review before 0.0.6 (15 October 2026), F1H: the desk hears
    /// the phone but cannot send; at 06:40 the phone's clock jumps a quarter
    /// of an hour ahead, and at 07:05 "This device is off" is said there of
    /// the desk: the word goes, in copies the desk sets aside. The desk keeps
    /// the 06:30 copy, word included, but a phone whose copy read now is set
    /// aside gives no word: at 08:00 the desk is not the first (quiet, then
    /// the doubt, which says why), and the phone alone reminds plainly. The
    /// smaller windows of the same class: a copy without the word written in
    /// the same second as the one before (W1S), and one of a phone whose clock
    /// was set back half an hour (W1B), take the word away at once, no news
    /// though they are; and a copy that is no news never gives a word.
    #[test]
    fn a_copy_that_is_no_news_takes_a_word_away_and_never_gives_one() {
        use sioul_core::dose_reminders::QUIET_AFTER;
        let due: i64 = 1_791_360_000; // 08:00
        let opening = due - 82_800; // the desk opened yesterday at 09:00, the phone's word on it since
        let none = sioul_sync::share::Heard::default();
        let copy = |exported: i64, word: bool| sioul_sync::devices::Entry {
            id: "phone-id".into(),
            kind: sioul_sync::devices::PHONE.into(),
            started: due - 10_800,
            closed: due - 10_700,
            exported,
            imported: exported,
            doses: true,
            acked: if word { [("desk-id".to_string(), opening)].into_iter().collect() } else { std::collections::BTreeMap::new() },
            ..sioul_sync::devices::Entry::default()
        };
        // The phone's word as the tick reads it.
        let word_here = |kept: &Peers| acknowledgements("desk-id", &newest_of(kept));
        // F1H. Its 06:30 restatement, read at 06:31: the word stands.
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[copy(due - 5_400, true)], &none, due - 5_340);
        assert_eq!(word_here(&kept).get("phone-id"), Some(&opening));
        // From 06:40 its clock runs a quarter of an hour ahead: its copies set aside, the word taken back in them at 07:05.
        for (real, word) in [(due - 4_740, true), (due - 3_300, false), (due - 1_800, false), (due - 60, false)] {
            learn(&mut kept, &[], &[copy(real + 900, word)], &none, real + 30);
        }
        let seen = &kept.peers["phone-id"];
        assert!(seen.peer.set_aside && seen.entry.as_ref().is_some_and(|e| e.acked.get("desk-id") == Some(&opening)), "the 06:30 copy kept, its word in it");
        assert!(word_here(&kept).is_empty(), "a phone whose copy read now is set aside gives no word");
        let knows = Knowledge { record_lost: None, peers: kept.peers.values().map(|s| s.peer.clone()).collect() };
        let at = due - 30;
        assert!(!all_acknowledged(&knows, opening, &word_here(&kept), at));
        let acked = acked_after(Some((opening, opening + 180)), all_acknowledged(&knows, opening, &word_here(&kept), at), opening, at);
        let first = first_here(false, true, opening, acked.map(|(_, at)| at), due, word_lead(&knows));
        assert!(!first);
        assert_eq!(plan(None, true, first, due, due + 10, true), Plan::Quiet(due + QUIET_AFTER), "quiet, then the doubt: the phone's reminder the only plain one");
        let english = Translator::new("en");
        assert_eq!(why_in(&sioul_core::health::doubts_now(due, due + QUIET_AFTER, None, &knows.peers), &english), "the phone has its clock more than ten minutes ahead of this one's, so what it says is set aside here");
        // W1S: a copy without the word written in the same second as the one before: no news, and no word.
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[copy(due - 3_600, true)], &none, due - 3_590);
        learn(&mut kept, &[], &[copy(due - 3_600, false)], &none, due - 3_580);
        assert_eq!(kept.peers["phone-id"].peer.seen, due - 3_590, "no news");
        assert!(word_here(&kept).is_empty(), "the word taken back at once");
        // W1B: its clock set back half an hour at 07:01. A copy saying the word as the newest does keeps it;
        // the one taking it back at 07:05 takes it away at once, no news though both are.
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[copy(due - 3_600, true)], &none, due - 3_590);
        learn(&mut kept, &[], &[copy(due - 3_480 - 1_800, true)], &none, due - 3_470);
        assert_eq!(word_here(&kept).get("phone-id"), Some(&opening), "said by both: it stands");
        learn(&mut kept, &[], &[copy(due - 3_300 - 1_800, false)], &none, due - 3_290);
        assert_eq!(kept.peers["phone-id"].peer.seen, due - 3_590, "no news");
        assert!(word_here(&kept).is_empty(), "taken away at 07:05, not at 07:31");
        // A copy that is no news never gives a word: the newest says none, an older one put back says it.
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[copy(due - 3_600, false)], &none, due - 3_590);
        learn(&mut kept, &[], &[copy(due - 7_200, true)], &none, due - 3_500);
        assert!(word_here(&kept).is_empty());
    }

    /// The final review's F1L: a device said off and forgotten here stays so
    /// while this clock goes back an hour and is set right again (its copy
    /// then looks ahead: set aside, never dropped), until it says anything
    /// newer. A copy kept from a clock already far ahead when it came (by a
    /// Sioul before the rule) is dropped, and the copy read now taken.
    #[test]
    fn a_device_said_off_stays_so_until_it_says_anything_newer() {
        let t: i64 = 1_791_349_200;
        let desk = |started: i64, exported: i64| sioul_sync::devices::Entry { id: "desk-id".into(), name: "desk".into(), kind: sioul_sync::devices::COMPUTER.into(), started, closed: exported + 10, exported, imported: exported, doses: true, ..sioul_sync::devices::Entry::default() };
        let none = sioul_sync::share::Heard::default();
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[desk(t, t + 600)], &none, t + 660);
        let mark = Mark { started: t, exported: t + 600, renewed: 0, at: t + 700 };
        let seen = kept.peers.get_mut("desk-id").unwrap();
        (seen.off, seen.forgotten) = (Some(mark.clone()), Some(mark));
        // This clock set back an hour: the same copy looks ahead, set aside; kept, and the marks with it.
        learn(&mut kept, &[], &[desk(t, t + 600)], &none, t - 2_900);
        let seen = &kept.peers["desk-id"];
        assert!(seen.peer.set_aside && seen.entry.is_some() && seen.peer.off && seen.peer.forgotten);
        // Set right again: the same copy, nothing newer.
        learn(&mut kept, &[], &[desk(t, t + 600)], &none, t + 800);
        let seen = &kept.peers["desk-id"];
        assert!(!seen.peer.set_aside && seen.peer.off && seen.peer.forgotten);
        // It speaks again: a new start.
        learn(&mut kept, &[], &[desk(t + 900, t + 960)], &none, t + 1_000);
        let seen = &kept.peers["desk-id"];
        assert!(!seen.peer.off && !seen.peer.forgotten, "it spoke again");
        // A copy kept from a clock already a day ahead when it came: dropped, the copy read now taken.
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[desk(t, t + 600)], &none, t + 660);
        kept.peers.get_mut("desk-id").unwrap().entry = Some(desk(t + 86_400, t + 86_400));
        learn(&mut kept, &[], &[desk(t + 700, t + 760)], &none, t + 800);
        assert_eq!(kept.peers["desk-id"].entry.as_ref().map(|e| e.started), Some(t + 700));
    }

    /// A phone that will never share again (for example, its
    /// battery dead, its USB broken), and the new phone that replaces it
    /// (`devices::Entry::replaces`), or the computer's "Forget it now": never
    /// two plain reminders of a dose, nor none. Silent, the old phone reminds
    /// nothing; replaced or forgotten, it owes the desk no word on its opening,
    /// and the desk is the dose's first again once the new phone said it read
    /// that opening; until then it keeps quiet around the phone's alarm, then
    /// reminds with the doubt. The new phone's alarm owes the old one nothing
    /// either: the desk's own word decides. Should the old phone speak again
    /// (said replaced by mistake), it counts again at once: the desk owes its
    /// word again, and is not the first until it gives it. An older copy of
    /// its entry put back is no news; nothing of it is deleted.
    #[test]
    fn a_replaced_or_forgotten_phone_never_makes_two_plain_reminders_nor_none() {
        use sioul_core::dose_reminders::{DoseReminders, QUIET_AFTER, Tried, others};
        use sioul_sync::devices::{Entry, PHONE, Replaced};
        let due: i64 = 1_791_360_000; // 08:00
        let key = format!("levo@{due}");
        let opened = due - 3_600; // the desk opened at 07:00, the doses kept there since
        let none = sioul_sync::share::Heard::default();
        let phone = |id: &str, started: i64, exported: i64, acked: Option<i64>| Entry {
            id: id.into(),
            name: format!("{id} model"),
            kind: PHONE.into(),
            started,
            closed: exported + 5,
            working: false,
            imported: exported,
            exported,
            doses: true,
            acked: acked.map(|opening| [("desk-id".to_string(), opening)].into_iter().collect()).unwrap_or_default(),
            ..Entry::default()
        };
        let knows = |kept: &Peers| Knowledge { record_lost: None, peers: kept.peers.values().map(|s| s.peer.clone()).collect() };
        let learned = |kept: &mut Peers, entries: &[Entry], now: i64| {
            learn(kept, &[], entries, &none, now);
            let replaced = replacements(kept, entries, &Entry::default());
            apply_replaced(kept, &replaced);
        };
        let must = |kept: &Peers| must_acknowledge(&knows(kept)).iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        // The old phone, silent since three days, its word on an opening of the desk long gone.
        let old = phone("old-id", due - 3 * 86_400 - 600, due - 3 * 86_400, Some(due - 4 * 86_400));
        let mut kept = Peers::default();
        learned(&mut kept, std::slice::from_ref(&old), due - 3 * 86_400 + 60);
        assert_eq!(must(&kept), ["old-id"], "silent, its word is asked all the same");
        assert!(!all_acknowledged(&knows(&kept), opened, &acknowledgements("desk-id", std::slice::from_ref(&old)), due - 600), "never given: the desk is never the first");
        // The new phone joins in its place, its entry saying so, with what the old one last said.
        let replaces = Replaced { id: "old-id".into(), started: old.started, heard: (old.closed.max(old.exported)) * 1000 };
        let new = Entry { replaces: vec![replaces.clone()], ..phone("new-id", due - 1_800, due - 1_700, None) };
        learned(&mut kept, &[old.clone(), new.clone()], due - 1_690);
        let seen_old = kept.peers["old-id"].peer.clone();
        assert!(seen_old.off && seen_old.forgotten, "replaced: as forgotten here");
        assert_eq!(must(&kept), ["new-id"], "the old phone owes no word any more");
        // An older copy of the old phone's entry put back by whoever serves the files: no news, still replaced.
        let older = phone("old-id", due - 5 * 86_400, due - 5 * 86_400 + 60, None);
        learned(&mut kept, &[older, new.clone()], due - 1_680);
        assert!(kept.peers["old-id"].peer.forgotten, "an older copy is no news");
        // Before the new phone said it read the desk's opening: not the first; quiet around
        // the phone's alarm, then the doubt first: never plain, never none.
        let acks = acknowledgements("desk-id", &[old.clone(), new.clone()]);
        assert!(!all_acknowledged(&knows(&kept), opened, &acks, due - 1_680));
        assert_eq!(plan(None, true, false, due, due + 180, true), Plan::Quiet(due + QUIET_AFTER));
        assert_eq!(plan(Some(Tried { quiet: due + QUIET_AFTER, ..Tried::default() }), true, false, due, due + QUIET_AFTER + 60, true), Plan::Late(None));
        // The new phone said it read this very opening: the desk is the first again, plainly, at the dose's time.
        let new = Entry { replaces: vec![replaces.clone()], ..phone("new-id", due - 1_800, due - 1_200, Some(opened)) };
        learned(&mut kept, &[old.clone(), new.clone()], due - 1_190);
        let acks = acknowledgements("desk-id", &[old.clone(), new.clone()]);
        let knowledge = knows(&kept);
        assert!(all_acknowledged(&knowledge, opened, &acks, due - 1_190));
        assert!(first_here(false, true, opened, Some(due - 1_190), due, word_lead(&knowledge)));
        assert_eq!(plan(None, true, true, due, due + 30, true), Plan::First);
        // The new phone's alarm: the old phone, replaced there too, owes it no word; the desk's own decides.
        let desk = Peer {
            name: "desk".into(),
            id: "desk-id".into(),
            said: Some(sioul_core::health::Said { started: opened, closed: 0, working: true, exported: due + 570, imported: due + 570, doses: true, left: false, phone: false }),
            complete: true,
            seen: due + 575,
            heard: due + 570,
            ..Peer::default()
        };
        let gone = Peer { off: true, forgotten: true, ..seen_old.clone() };
        let mut shown = DoseReminders::default();
        *shown.entry(&key, "desk-id") = Tried { maybe: due + 2, shown: due + 3, ..Tried::default() };
        assert!(matches!(others(&key, due, due + WAIT_FOR_NEWS, "new-id", true, Some(&shown), &[desk.clone(), gone.clone()], true), Others::Reminded), "the desk reminded it: the phone stays silent");
        let mut quiet = DoseReminders::default();
        *quiet.entry(&key, "desk-id") = Tried { quiet: due + QUIET_AFTER, ..Tried::default() };
        assert!(matches!(others(&key, due, due + WAIT_FOR_NEWS, "new-id", true, Some(&quiet), &[desk.clone(), gone.clone()], true), Others::First), "the desk promised none: the phone's alarm is the first");
        let counted = Peer { off: false, forgotten: false, ..gone.clone() };
        assert!(matches!(others(&key, due, due + WAIT_FOR_NEWS, "new-id", true, Some(&quiet), &[desk, counted], true), Others::Doubt(_)), "counted, the old phone's alarm could ring: the doubt");
        // The old phone speaks again (said replaced by mistake): a new start. It counts again at
        // once, and owes its word on the desk's opening: the desk is not the first until it gives it.
        let back = phone("old-id", due - 900, due - 890, Some(due - 4 * 86_400));
        learned(&mut kept, &[back.clone(), new.clone()], due - 880);
        let seen_back = kept.peers["old-id"].peer.clone();
        assert!(!seen_back.off && !seen_back.forgotten, "it spoke again: counted");
        assert_eq!(must(&kept).len(), 2);
        assert!(!all_acknowledged(&knows(&kept), opened, &acknowledgements("desk-id", &[back.clone(), new.clone()]), due - 880), "never plain beside it");
        // Its word on this opening, given: the desk the first again.
        let back = phone("old-id", due - 900, due - 860, Some(opened));
        learned(&mut kept, &[back.clone(), new.clone()], due - 850);
        assert!(all_acknowledged(&knows(&kept), opened, &acknowledgements("desk-id", &[back, new]), due - 850));
        // "Forget it now" on the desk, the same for a phone silent a day: forgotten until it says anything newer.
        let mut kept = Peers::default();
        let silent = phone("old-id", due - 86_400 - 600, due - 86_400, Some(due - 2 * 86_400));
        learned(&mut kept, std::slice::from_ref(&silent), due - 86_400 + 60);
        let seen = kept.peers.get_mut("old-id").unwrap();
        seen.forgotten = Some(Mark { started: silent.started, exported: silent.exported, renewed: 0, at: due - 600 });
        learned(&mut kept, std::slice::from_ref(&silent), due - 590);
        assert!(must(&kept).is_empty() && kept.peers["old-id"].peer.forgotten);
        let rows = device_rows_of(&knows(&kept).peers, &kept, due - 590, &Translator::new("en"));
        assert!(rows.iter().all(|r| r.id != "old-id"), "forgotten: not listed");
        learned(&mut kept, &[phone("old-id", due - 300, due - 290, None)], due - 280);
        assert_eq!(must(&kept), ["old-id"], "it spoke again: its word owed again");
        // What the replacement says: still that start, nothing later than its time.
        assert!(replaces.holds(Some(old.started), old.exported * 1000) && !replaces.holds(Some(due - 900), old.exported * 1000) && !replaces.holds(Some(old.started), (old.exported + 60) * 1000));
    }

    /// On a computer, a phone silent for a day or more is offered "Forget it
    /// now" (a dead phone could hold every dose back a
    /// week); one heard within the day, or already offered "Forget this
    /// device", is not; nor is a computer.
    #[test]
    fn a_phone_silent_a_day_is_offered_to_be_forgotten_now() {
        use sioul_core::health::Said;
        let now: i64 = 1_791_360_000;
        let peer = |id: &str, phone: bool, seen: i64| Peer {
            id: id.into(),
            name: id.into(),
            said: Some(Said { started: seen - 60, closed: seen, working: false, exported: seen, imported: seen, doses: true, left: false, phone }),
            complete: true,
            seen,
            heard: seen,
            ..Peer::default()
        };
        let peers = [peer("silent-phone", true, now - FORGET_NOW_AFTER - 60), peer("fresh-phone", true, now - 3_600), peer("silent-desk", false, now - FORGET_NOW_AFTER - 60), peer("week-phone", true, now - 8 * 86_400)];
        let rows = device_rows_of(&peers, &Peers::default(), now, &Translator::new("en"));
        let row = |id: &str| rows.iter().find(|r| r.id == id).unwrap();
        assert!(row("silent-phone").forget_now && !row("silent-phone").forget);
        assert!(!row("fresh-phone").forget_now);
        assert!(!row("silent-desk").forget_now, "a computer is not offered it");
        assert!(row("week-phone").forget && !row("week-phone").forget_now, "silent a week: Forget this device, as before");
    }

    /// The final review's words: on the phone, a desk whose clock runs an hour
    /// ahead (its copies set aside, last night's kept, its claims dated ahead)
    /// is said as it is, never "an older Sioul there?", in the alarm's reason
    /// and the doubt alike, said once; its word stays on last night's opening,
    /// so the desk, opened anew, is never the first.
    #[test]
    fn a_computer_whose_clock_runs_ahead_is_said_so() {
        use sioul_core::dose_reminders::{DoseReminders, others};
        let due: i64 = 1_791_360_000; // 08:00
        let key = format!("levo@{due}");
        let desk = |started: i64, closed: i64, working: bool, exported: i64| sioul_sync::devices::Entry { id: "desk-id".into(), name: "desk".into(), kind: sioul_sync::devices::COMPUTER.into(), started, closed, working, exported, imported: exported, doses: true, ..sioul_sync::devices::Entry::default() };
        let none = sioul_sync::share::Heard::default();
        let mut kept = Peers::default();
        // Last night's: open at 22:00, closed at 23:00, read at 23:06.
        learn(&mut kept, &[], &[desk(due - 36_000, due - 32_400, false, due - 32_410)], &none, due - 32_000);
        // This morning its clock runs an hour ahead: opened at 07:00 (08:00 by its clock), its claim renewed at 07:59 (08:59).
        let claim = sioul_sync::lease::Claim { computer: "desk-id".into(), name: "desk".into(), since: due, renewed: due + 3_540, until: due + 3_840, active: due + 3_540, taken: 0, wrote: None, closed: false, pad: String::new() };
        learn(&mut kept, std::slice::from_ref(&claim), &[desk(due, 0, true, due + 3_540)], &none, due - 40);
        let peer = kept.peers["desk-id"].peer.clone();
        assert!(peer.set_aside && peer.said.as_ref().is_some_and(|s| s.started == due - 36_000), "set aside, last night's kept: never an older Sioul");
        let english = Translator::new("en");
        let french = Translator::new("fr");
        let words = "desk has its clock more than ten minutes ahead of this one's, so what it says is set aside here";
        let Others::Doubt(e) = others(&key, due, due + 600, "phone-id", true, Some(&DoseReminders::default()), std::slice::from_ref(&peer), true) else { panic!("never plain") };
        assert_eq!(elsewhere_said(&e, due, &english), words);
        assert_eq!(elsewhere_said(&e, due, &french), "desk a son horloge en avance de plus de dix minutes sur celle de cet appareil, et ce qu’il dit est mis de côté ici");
        let doubts = sioul_core::health::doubts_now(due, due + 600, None, std::slice::from_ref(&peer));
        assert_eq!(why_in(&doubts, &english), words);
        assert!(!why_in(&doubts, &french).contains("plus ancien"));
        // The alarm's notice: the reason said once.
        let knows = Knowledge { record_lost: None, peers: vec![peer.clone()] };
        let first = elsewhere_said(&e, due, tr());
        assert_eq!(doubt_unread_with(&knows, due, due, due + 600, None, Some(&first)), say("dose-doubt", &[("why", first.clone())]));
        // The phone's word stays on last night's opening: the desk, opened anew, is never the first.
        assert_eq!(peer.said.as_ref().map(|s| s.started.max(peer.latest_opening)), Some(due - 36_000));
    }

    /// The re-check before 0.0.6 (LOW-2): the residual the final round left,
    /// composed as the tick reads it (`devices::chosen` of the folder's copy
    /// and the one fetched, `learn`, `newest_of`). The folder stops at the
    /// phone's 06:30 copy, its word on the desk in it; Sioul's own pull stays
    /// current. At 06:35 the phone's clock is set back three hours, and at
    /// 07:05 "This device is off" is said there of the desk. From the phone's
    /// next copy (07:10, dated 04:10) the word is withdrawn, though the
    /// folder's copy, dated later, still names the desk: at 08:00 the desk is
    /// not the first, where it reminded plainly (with the phone: two plain).
    #[test]
    fn a_word_the_lagging_copy_still_names_is_withdrawn() {
        use sioul_core::dose_reminders::QUIET_AFTER;
        let due: i64 = 1_791_360_000; // 08:00
        let opening = due - 82_800; // the desk opened yesterday at 09:00
        let none = sioul_sync::share::Heard::default();
        let copy = |dated: i64, word: bool| sioul_sync::devices::Entry {
            id: "phone-id".into(),
            kind: sioul_sync::devices::PHONE.into(),
            started: due - 10_800,
            closed: due - 10_700,
            exported: dated,
            imported: dated,
            doses: true,
            acked: if word { [("desk-id".to_string(), opening)].into_iter().collect() } else { std::collections::BTreeMap::new() },
            ..sioul_sync::devices::Entry::default()
        };
        let folder = copy(due - 5_400, true);
        let mut kept = Peers::default();
        learn(&mut kept, &[], &[sioul_sync::devices::chosen(folder.clone(), folder.clone())], &none, due - 5_340);
        let mut word_until = 0;
        // Every ten minutes from 06:40 a copy, dated three hours behind; the word taken back at 07:05.
        for real in (due - 4_800..due).step_by(600) {
            let fetched = copy(real - 10_800, real < due - 3_300);
            let read = sioul_sync::devices::chosen(folder.clone(), fetched);
            assert_eq!(read.exported, due - 5_400, "the folder's copy, dated later, is the one read");
            learn(&mut kept, &[], &[read], &none, real + 60);
            if acknowledgements("desk-id", &newest_of(&kept)).get("phone-id") == Some(&opening) {
                word_until = real + 60;
            }
        }
        assert_eq!(word_until, due - 3_540, "the word stood until 07:01, gone from 07:11");
        let knows = Knowledge { record_lost: None, peers: kept.peers.values().map(|s| s.peer.clone()).collect() };
        let at = due - 30;
        let acks = acknowledgements("desk-id", &newest_of(&kept));
        let acked = acked_after(Some((opening, opening + 180)), all_acknowledged(&knows, opening, &acks, at), opening, at);
        let first = first_here(false, true, opening, acked.map(|(_, at)| at), due, word_lead(&knows));
        assert!(!first);
        assert_eq!(plan(None, true, first, due, due + 10, true), Plan::Quiet(due + QUIET_AFTER), "no plain reminder at 08:00");
    }

    /// The re-check before 0.0.6 (LOW-2): a new word stands while one channel
    /// lags. The desk opened anew at 07:00; the phone names that opening from
    /// 07:02, in every copy Sioul's own pull brings; the folder stays on the
    /// phone's 06:30 copy, which names the desk's opening before: only behind,
    /// it takes nothing away, and the desk reminds plainly at 08:00. The cost:
    /// had the lagging copy named the desk not at all (counted again, paired
    /// anew), the word would wait for it.
    #[test]
    fn a_new_word_stands_while_one_channel_lags() {
        let due: i64 = 1_791_360_000; // 08:00
        let (before, opening) = (due - 82_800, due - 3_600); // yesterday 09:00; today 07:00
        let none = sioul_sync::share::Heard::default();
        let copy = |dated: i64, word: Option<i64>| sioul_sync::devices::Entry {
            id: "phone-id".into(),
            kind: sioul_sync::devices::PHONE.into(),
            started: due - 10_800,
            closed: due - 10_700,
            exported: dated,
            imported: dated,
            doses: true,
            acked: word.map(|at| ("desk-id".to_string(), at)).into_iter().collect(),
            ..sioul_sync::devices::Entry::default()
        };
        let run = |folder: &sioul_sync::devices::Entry| {
            let mut kept = Peers::default();
            let mut acked = None;
            for real in (due - 3_480..due).step_by(600) {
                learn(&mut kept, &[], &[sioul_sync::devices::chosen(folder.clone(), copy(real, Some(opening)))], &none, real + 60);
                let knows = Knowledge { record_lost: None, peers: kept.peers.values().map(|s| s.peer.clone()).collect() };
                acked = acked_after(acked, all_acknowledged(&knows, opening, &acknowledgements("desk-id", &newest_of(&kept)), real + 60), opening, real + 60);
            }
            let knows = Knowledge { record_lost: None, peers: kept.peers.values().map(|s| s.peer.clone()).collect() };
            (acked, first_here(false, true, opening, acked.map(|(_, at)| at), due, word_lead(&knows)))
        };
        let (acked, first) = run(&copy(due - 5_400, Some(before)));
        assert_eq!(acked, Some((opening, due - 3_420)), "the word from 07:03");
        assert!(first);
        assert_eq!(plan(None, true, first, due, due + 10, true), Plan::First, "plain at 08:00, as before");
        // The cost: a lagging copy that names the desk not at all holds the word back until it catches up.
        let (acked, first) = run(&copy(due - 5_400, None));
        assert!(acked.is_none() && !first);
    }

    /// At waking (doses kept for it), the desk's first reminder counts its
    /// doubt from waking, not from the dose's time (the fifth review's M2):
    /// a phone put away before the sleep may be picked up at waking to mark
    /// it, its alarm having waited for waking too; known only once the news
    /// of that opening had the time to come, as at a dose's time.
    #[test]
    fn at_waking_the_first_reminder_counts_from_waking() {
        use sioul_core::health::Said;
        let (bed, woke) = (1_800_000_000, 1_800_028_800);
        let due = bed + 6 * 3600;
        let since = news_from(due, Some((bed, woke)));
        assert_eq!(since, woke);
        let phone = Peer {
            name: "phone".into(),
            id: "phone-id".into(),
            said: Some(Said { started: bed - 7_200, closed: bed - 1_800, working: false, exported: bed - 1_800, imported: bed - 1_800, doses: true, left: false, phone: true }),
            complete: true,
            seen: bed - 1_800,
            heard: bed - 1_800,
            wake: Some(30),
            ..Peer::default()
        };
        let knowledge = Knowledge { record_lost: None, peers: vec![phone] };
        assert!(doubt_of(&knowledge, due, due, woke + 30).is_empty(), "counted from the dose's time, the phone would be known by its alarm long rung");
        assert!(!doubt_of(&knowledge, due, since, woke + 30).is_empty(), "counted from waking: the phone may have been picked up to mark it");
        assert!(doubt_of(&knowledge, due, since, woke + 30 + sioul_core::health::SKEW).is_empty(), "known once the news of an opening had the time to come");
        assert_eq!(plan(None, true, true, since, woke + 30, true), Plan::First, "its first, at waking");
    }

    /// The question on the past for a dose whose reminder may have shown
    /// (`maybe`): its own question, "may not have been shown", its own title,
    /// in English and French; mixed with others, a title true of all.
    #[test]
    fn a_reminder_that_may_have_shown_is_asked_about_under_its_own_question() {
        let health = Health {
            medicines: vec![
                Medicine { id: "demo-a".into(), name: "Demo A".into(), dose: String::new(), schedule: Schedule::Day { times: vec!["08:00".into()], amounts: std::collections::BTreeMap::new() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None },
                Medicine { id: "demo-b".into(), name: "Demo B".into(), dose: String::new(), schedule: Schedule::Day { times: vec!["07:00".into()], amounts: std::collections::BTreeMap::new() }, prescription: None, until: None, paused: false, generic: String::new(), strength: String::new(), since: None },
            ],
            ..Health::default()
        };
        let (english, french) = (Translator::new("en"), Translator::new("fr"));
        let at = |text: &str| text.parse::<Zoned>().unwrap();
        let key = |id: &str, when: &str| format!("{id}@{}", at(when).timestamp().as_second());
        let (a, b) = (key("demo-a", "2026-10-06T08:00[Europe/Paris]"), key("demo-b", "2026-10-06T07:00[Europe/Paris]"));
        let alone = Knowledge { record_lost: None, peers: Vec::new() };
        let noon = at("2026-10-06T12:00[Europe/Paris]");
        let mut state = HealthState::default();
        let sent = at("2026-10-06T08:00:02[Europe/Paris]").timestamp().as_second();
        note_minute(&mut state, &[], &[], std::slice::from_ref(&a), &[], sent);
        // The desk's notice went (recorded before it went), not known shown.
        let mut told = DoseReminders::default();
        told.entry(&a, "desk-id").maybe = sent;
        let rows = missed_rows(&health, &state, &told, &alone, &noon, &english);
        assert_eq!(rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), [b.as_str(), a.as_str()]);
        assert!(rows[1].question.starts_with("Due while Sioul ran, but their reminder may not have been shown"), "{}", rows[1].question);
        assert!(missed_rows(&health, &state, &told, &alone, &noon, &french)[1].question.starts_with("Prévus pendant que Sioul tournait, mais leur rappel ne s’est peut-être pas affiché"));
        let missed = state.unanswered(&health, &noon, MISSED_HOURS, GRACE_MINUTES);
        assert_eq!(missed_title(&missed[1..], &state, &told, &english), "A reminder may not have shown");
        assert_eq!(missed_title(&missed[1..], &state, &told, &french), "Un rappel ne s’est peut-être pas affiché");
        assert_eq!(missed_title(&missed, &state, &told, &english), "Doses due earlier");
        // Without the desk's word: not shown, as it reads here.
        assert_eq!(missed_title(&missed[1..], &state, &DoseReminders::default(), &english), "A reminder could not be shown");
    }

    /// A dose is open, for a phone's steps (`steps::keeps_close`), from its
    /// time until it is marked (here, or in the records come from another
    /// device) or its day ends: while one is and another device runs, the
    /// phone keeps its two-minute steps, at night too (review of battery part
    /// C, F2). A record that does not read counts the dose open in the same
    /// hours alone, marked or not: never before its time, never past its
    /// day's end (G4).
    #[test]
    fn a_dose_is_open_from_its_time_until_marked_or_its_day_ends() {
        let dir = std::env::temp_dir().join(format!("sioul-dose-open-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("health.toml");
        std::fs::write(&path, "[[medicine]]\nid = \"mag\"\nname = \"Magnesium\"\ndose = \"300 mg\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"22:00\"]\n").unwrap();
        let health = Health::load(&path);
        let at = |text: &str| text.parse::<Zoned>().unwrap();
        let s = |text: &str| at(text).timestamp().as_second();
        let (state, none) = (HealthState::default(), DoseRecords::default());
        let read = Some((&state, &none));
        assert!(!dose_open(&health, read, &at("2026-10-06T21:59[Europe/Paris]")), "not its time yet");
        assert!(dose_open(&health, read, &at("2026-10-06T22:00[Europe/Paris]")), "its time: open");
        assert!(dose_open(&health, read, &at("2026-10-06T23:30[Europe/Paris]")), "not marked, the night begun");
        assert!(!dose_open(&health, read, &at("2026-10-07T00:30[Europe/Paris]")), "its day over: no longer asked about");
        // Marked here; or at the desk, its record come before its mark: closed.
        let key = format!("mag@{}", s("2026-10-06T22:00[Europe/Paris]"));
        let mut taken = HealthState::default();
        taken.taken.insert(key.clone(), s("2026-10-06T22:05[Europe/Paris]"));
        assert!(!dose_open(&health, Some((&taken, &none)), &at("2026-10-06T22:10[Europe/Paris]")));
        let mut records = DoseRecords::default();
        let marked = s("2026-10-06T22:07[Europe/Paris]");
        records.answer(&key, "desk", Answer { state: sioul_core::doses::TAKEN.into(), at: marked, noted: marked, name: "desk".into(), kind: "computer".into() });
        assert!(!dose_open(&health, Some((&state, &records)), &at("2026-10-06T22:10[Europe/Paris]")), "marked at the desk");
        assert!(!dose_open(&Health::default(), read, &at("2026-10-06T22:10[Europe/Paris]")), "no medicine");
        // A record that does not read: open from the dose's time to its day's end, marked or not; never outside.
        assert!(!dose_open(&health, None, &at("2026-10-06T20:00[Europe/Paris]")), "unread, its time two hours on: not open");
        assert!(!dose_open(&health, None, &at("2026-10-06T21:59[Europe/Paris]")), "unread, not its time yet");
        assert!(dose_open(&health, None, &at("2026-10-06T22:00[Europe/Paris]")), "unread, its time: open (the safe side)");
        assert!(dose_open(&health, None, &at("2026-10-06T23:59[Europe/Paris]")), "unread, the night begun");
        assert!(!dose_open(&health, None, &at("2026-10-07T00:30[Europe/Paris]")), "unread, its day over: closed");
        assert!(!dose_open(&health, None, &at("2026-10-07T12:00[Europe/Paris]")), "unread, the next day before its time");
        assert!(!dose_open(&Health::default(), None, &at("2026-10-06T22:10[Europe/Paris]")), "unread, no medicine");
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
    /// its time for those two minutes and `SKEW` more, then said as known; a dose due an hour
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
        let woke = eight + 120 + sioul_core::health::SKEW;
        assert_eq!(pieces, [("12:30", s("2026-10-06T12:30[Europe/Paris]"), midnight, midnight), ("20:00", eight, woke, 0), ("20:00", woke, midnight, midnight)]);
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
        // A dose due at 3 000, the phone closed at 2 900: in doubt for those 240 s and `SKEW`, then known.
        learn(&mut kept, &[], &[phone(2_300, false, 2_900)], &heard_of("phone-id", 5), 2_910);
        let peers = vec![kept.peers["phone-id"].peer.clone()];
        let skew = sioul_core::health::SKEW;
        assert_eq!(sioul_core::health::doubts_now(3_000, 3_239 + skew, None, &peers).len(), 1);
        assert!(sioul_core::health::doubts_now(3_000, 3_240 + skew, None, &peers).is_empty());
    }

    /// R9 of the review of 10 October 2026: a dose's alarm in doubt asks again
    /// when the doubt may end, a minute apart at least; in doubt only for the
    /// news missed since it began, at the end of the wait for news, not each
    /// minute.
    #[test]
    fn an_alarm_in_doubt_asks_again_when_the_doubt_may_end() {
        let due = 1_800_000_000;
        assert_eq!(alarm_again(due, due + 5, Some(due + 300)), due + 300, "a closed device's opening could have come by then");
        assert_eq!(alarm_again(due, due + 5, Some(due + 30)), due + 65, "a minute apart at least");
        assert_eq!(alarm_again(due, due + 5, Some(due + 6)), due + WAIT_FOR_NEWS, "known by what was read: only the news missed");
        assert_eq!(alarm_again(due, due + 5, None), due + WAIT_FOR_NEWS);
        assert_eq!(alarm_again(due, due + 5, Some(due + 2 * WAIT_FOR_NEWS)), due + WAIT_FOR_NEWS);
    }

    /// A dose due while sleep held it (doses kept for waking): the phone's
    /// alarm, asked again at waking, waits for a computer's news from waking,
    /// as from a dose's time, since that computer reminds it at waking too;
    /// the two never remind it together, both plainly (review of battery part
    /// C, the fourth round). The computer's late tries count from waking too
    /// (until `LATE_UNTIL` after it), and the phone knows them read only once
    /// the computer's records from past then are. A dose due after waking,
    /// or before the sleep began, counts from its own time; asking again in
    /// doubt ends there too.
    #[test]
    fn a_phone_held_by_sleep_waits_for_news_from_waking() {
        use sioul_core::health::Said;
        let (bed, woke) = (1_800_000_000, 1_800_028_800);
        let due = bed + 5 * 3600;
        assert_eq!(news_from(due, Some((bed, woke))), woke, "due asleep: from waking");
        assert_eq!(news_from(woke + 600, Some((bed, woke))), woke + 600, "due after waking: from its time");
        assert_eq!(news_from(bed - 600, Some((bed, woke))), bed - 600, "due before the sleep: from its time");
        assert_eq!(news_from(due, None), due, "no sleep held it");
        let since = news_from(due, Some((bed, woke)));
        assert_eq!(alarm_again(since, woke + 5, None), woke + WAIT_FOR_NEWS, "asked again at the end of the wait from waking");
        // The desk's tries count from waking too: quiet around the phone's alarm, promised; the
        // phone, reading that promise, is the first, plainly; without it, the doubt.
        let unshown = Some(Tried { unshown: woke + 2, ..Tried::default() });
        assert_eq!(plan(unshown, true, true, since, woke + LATE_UNTIL + 30, true), Plan::Quiet(woke + sioul_core::dose_reminders::QUIET_AFTER));
        let key = format!("levo@{due}");
        let mut told = DoseReminders::default();
        *told.entry(&key, "desk-id") = Tried { unshown: woke + 2, quiet: woke + sioul_core::dose_reminders::QUIET_AFTER, ..Tried::default() };
        let desk = Peer {
            name: "desk".into(),
            id: "desk-id".into(),
            said: Some(Said { started: bed - 3_600, closed: 0, working: true, exported: woke + 540, imported: woke + 540, doses: true, left: false, phone: false }),
            complete: true,
            seen: woke + 540,
            heard: woke + 540,
            ..Peer::default()
        };
        let at_alarm = woke + WAIT_FOR_NEWS;
        assert_eq!(sioul_core::dose_reminders::others(&key, since, at_alarm, "phone-id", true, Some(&told), std::slice::from_ref(&desk), true), Others::First);
        assert!(matches!(sioul_core::dose_reminders::others(&key, due, at_alarm, "phone-id", true, Some(&told), std::slice::from_ref(&desk), true), Others::First), "the promise, not the dose's time, says it");
        let unpromised = { let mut told = told.clone(); told.entry(&key, "desk-id").quiet = 0; told };
        assert!(matches!(sioul_core::dose_reminders::others(&key, since, at_alarm, "phone-id", true, Some(&unpromised), std::slice::from_ref(&desk), true), Others::Doubt(_)));
    }

    /// R8 of the review of 10 October 2026: an opening is timed only while
    /// this device's own news flows. Its own pull stale (a desktop whose
    /// pulls failed for hours): the delay is its outage, not timed. A sync
    /// app alone carrying the folder (nothing tells here whether it paused):
    /// timed up to an hour, never more.
    #[test]
    fn an_opening_is_not_timed_by_this_devices_own_outage() {
        use sioul_sync::devices::Entry;
        let phone = |started: i64, working: bool, closed: i64| Entry { id: "phone-id".into(), name: "FP3".into(), kind: "phone".into(), started, closed, working, exported: started.max(closed), wrote: Some((1, 5)), doses: true, ..Entry::default() };
        let opened_late = |news: Option<bool>| {
            let mut kept = Peers::default();
            learn_with(&mut kept, &[], &[phone(900, false, 1_000)], &heard_of("phone-id", 5), news, 1_020);
            learn_with(&mut kept, &[], &[phone(900, false, 1_000)], &heard_of("phone-id", 5), news, 1_080);
            // Opened at 1 100, its news seen here five hours later, this device looking each minute.
            for now in (1_140..19_100).step_by(60) {
                learn_with(&mut kept, &[], &[phone(900, false, 1_000)], &heard_of("phone-id", 5), news, now);
            }
            learn_with(&mut kept, &[], &[phone(1_100, true, 1_000)], &heard_of("phone-id", 5), news, 19_100);
            kept.peers["phone-id"].peer.wake
        };
        assert_eq!(opened_late(Some(false)), None, "its own pull stale: its outage, not the phone's");
        assert_eq!(opened_late(Some(true)), Some(18_000), "its own pull went through: the phone's news was late");
        assert_eq!(opened_late(None), Some(UNSURE_WAKE), "a sync app alone: an hour at most");
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
        let shown = |knowledge: &Knowledge| serde_json::to_value(porch_doses(&health, &state, &records, &DoseReminders::default(), "desk-id", knowledge, &noon, false, true, &english)).unwrap();
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
        let view = serde_json::to_value(porch_doses(&health, &HealthState::default(), &records, &DoseReminders::default(), "desk-id", &alone, &noon, false, true, &english)).unwrap();
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
    /// A Health run reads once what your devices know (`know`, under
    /// `KNOWING`): its page and its Porch's doses come from one read.
    #[test]
    fn a_health_run_reads_what_the_devices_know_once() {
        let _files = health_files();
        KNOWN_IN_THREAD.with(|n| n.set(0));
        let _ = views(&(std::collections::BTreeSet::new(), std::collections::BTreeSet::new()));
        assert_eq!(KNOWN_IN_THREAD.with(std::cell::Cell::get), 1);
    }

    /// Today's rows are Health's own day: the doses of the schedule and one
    /// taken at a time the schedule no longer has, whatever week the page
    /// shows; each named, with its amount, for the places that name it.
    #[test]
    fn today_s_rows_are_health_s_own_day() {
        let _files = health_files();
        let (health_path, state_path) = (Health::default_path(), HealthState::default_path());
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        std::fs::write(&health_path, "[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\ndose = \"1 tablet\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\", \"20:00\"]\n").unwrap();
        let off = now.date().at(10, 30, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let mut state = HealthState::default();
        state.taken.insert(format!("lumiel@{off}"), off);
        std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        state.save(&state_path, now.timestamp().as_second()).unwrap();
        let reads = Reads::now();
        let rows = doses_today(&reads, &now);
        let (health, _) = reads.known.clone().unwrap();
        let day = |shown: Date| {
            let week = week_view(&health, &reads.state, &reads.records, &reads.knowledge, shown, &now, &(std::collections::BTreeSet::new(), std::collections::BTreeSet::new()));
            week.days.into_iter().find(|d| d.today).map(|d| d.items.into_iter().filter(|i| i.kind == "dose").map(|i| i.key).collect::<Vec<_>>())
        };
        let keys: Vec<String> = rows.rows.iter().map(|r| r.key.clone()).collect();
        assert_eq!(Some(keys.clone()), day(now.date()), "as Health's day lists them");
        assert!(keys.contains(&format!("lumiel@{off}")), "the dose taken at a time the schedule no longer has: {keys:?}");
        let next_week = now.date().checked_add(Span::new().days(7)).unwrap();
        assert_eq!(day(next_week), None, "the page shows another week: today is not in it");
        assert_eq!(doses_today(&reads, &now).rows.len(), keys.len(), "the rows are today's all the same");
        assert!(rows.rows.iter().all(|r| r.name == "Lumiel" && r.amount == "1 tablet" && r.day == now.date()));
        let taken = rows.rows.iter().find(|r| r.at == off).unwrap();
        assert_eq!(taken.states.last().map(|g| g.state.clone()), Some(sioul_core::timeline::DoseState::Taken { at: off }));
        // The timeline's marks name no medicine, nor its amount: the key routes, nothing more.
        let marks = dose_marks_json(&rows, &zone);
        assert!(!marks.contains("Lumiel") && !marks.contains("1 tablet") && marks.contains(&format!("lumiel@{off}")), "{marks}");
    }

    /// A row's states follow the window's clock, given ahead: before its
    /// time, open; from it, known not marked, then late past its half hour.
    /// A reading of another day, or none since Sioul came back, is being
    /// read; one older than two minutes turns an open dose into "check", and
    /// a dose taken stays taken. Nothing ever says "not taken".
    #[test]
    fn a_dose_row_follows_the_clock_and_ages() {
        use sioul_core::timeline::DoseState;
        let zone = TimeZone::get("Europe/Paris").unwrap();
        let today = jiff::civil::date(2026, 10, 5);
        let at = |h: i8, m: i8, s: i8| today.at(h, m, s, 0).to_zoned(zone.clone()).unwrap();
        let health: Health = toml::from_str("[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"08:00\"]\n").unwrap();
        let reads = |state: HealthState, lost: Option<i64>| Reads { known: Some((health.clone(), false)), state, records: DoseRecords::default(), knowledge: Knowledge { record_lost: lost, peers: Vec::new() }, told: DoseReminders::default() };
        // Read at 07:59:50, for the 08:00 dose.
        let read = at(7, 59, 50);
        let rows = doses_today(&reads(HealthState::default(), None), &read);
        let row = &rows.rows[0];
        let stamp = |z: Zoned| z.timestamp().as_second();
        let (r, today_) = (stamp(read.clone()), today);
        assert_eq!(dose_state(row, today_, stamp(at(7, 59, 55)), r, 0).0, DoseState::Open { come: false, late: false });
        assert_eq!(dose_state(row, today_, stamp(at(8, 0, 30)), r, 0).0, DoseState::Open { come: true, late: false }, "its time come, read 40 s ago");
        assert!(matches!(dose_state(row, today_, stamp(at(8, 2, 0)), r, 0).0, DoseState::Check { .. }), "the reading is older than two minutes");
        // Late, as given ahead: past its half hour (the reading as fresh as a minute's run keeps it).
        assert_eq!(dose_state(row, today_, stamp(at(8, 31, 0)), stamp(at(8, 30, 30)), 0).0, DoseState::Open { come: true, late: true });
        // Read at 08:30, its reminder shown nowhere: the question on the past, never "due" again.
        let later = doses_today(&reads(HealthState::default(), None), &at(8, 30, 30));
        assert_eq!(dose_state(&later.rows[0], today_, stamp(at(8, 31, 0)), stamp(at(8, 30, 30)), 0).0, DoseState::Asked);
        // Read at 23:59 the day before, the window at 00:01: no reading of today.
        let yesterday = doses_today(&reads(HealthState::default(), None), &today.yesterday().unwrap().at(23, 59, 0, 0).to_zoned(zone.clone()).unwrap());
        assert!(yesterday.rows.iter().all(|row| dose_state(row, today_, stamp(at(0, 1, 0)), stamp(at(0, 0, 0)) - 60, 0).0 == DoseState::Reading));
        // Sioul back on the screen after the reading (a phone opened by its alarm at 08:00:10): being read.
        assert_eq!(dose_state(row, today_, stamp(at(8, 0, 10)), r, stamp(at(8, 0, 10))).0, DoseState::Reading);
        // Taken stays taken, however old the reading, and being read again after Sioul comes back.
        let mut state = HealthState::default();
        let key = row.key.clone();
        state.taken.insert(key, stamp(at(8, 2, 0)));
        let taken = doses_today(&reads(state, None), &at(9, 0, 0));
        assert_eq!(dose_state(&taken.rows[0], today_, stamp(at(11, 0, 0)), stamp(at(9, 0, 0)), 0).0, DoseState::Taken { at: stamp(at(8, 2, 0)) });
        assert_eq!(dose_state(&taken.rows[0], today_, stamp(at(11, 0, 0)), stamp(at(9, 0, 0)), stamp(at(10, 0, 0))).0, DoseState::Taken { at: stamp(at(8, 2, 0)) }, "back on the screen since");
        // The health file not read here, its doses as last known: never plainly due, to check, said.
        let last_known = Reads { known: Some((health.clone(), true)), ..reads(HealthState::default(), None) };
        let lost = doses_today(&last_known, &read);
        assert!(!lost.readable);
        let (state, words) = dose_state(&lost.rows[0], today_, stamp(at(8, 0, 30)), r, 0);
        assert!(matches!(state, DoseState::Check { .. }) && words == tr().text("dose-last-known", None), "{state:?}");
        // A record lost here at 06:00 doubts the rest of its day: the 08:00 dose may have been
        // marked ahead in what was lost (review of 1880bce, L13). The next day's is known.
        let lost_at = Some(stamp(at(6, 0, 0)));
        let after_loss = doses_today(&reads(HealthState::default(), lost_at), &read);
        assert!(matches!(dose_state(&after_loss.rows[0], today_, stamp(at(8, 0, 30)), r, 0).0, DoseState::Check { .. }));
        let next_day = today.tomorrow().unwrap();
        let next_read = next_day.at(7, 59, 50, 0).to_zoned(zone.clone()).unwrap();
        let next = doses_today(&reads(HealthState::default(), lost_at), &next_read);
        assert_eq!(dose_state(&next.rows[0], next_day, stamp(next_read.clone()) + 40, stamp(next_read), 0).0, DoseState::Open { come: true, late: false });
    }
    /// The week's read is kept a few minutes, never past a write, by this
    /// program or another (`timeline::Stamp`): an event moved moves the
    /// week's meals at once (review of 1880bce, later 4).
    #[test]
    fn health_s_week_reads_again_after_a_write() {
        let _files = health_files();
        let zone = TimeZone::system();
        let now = Zoned::now();
        let monday = now.date().checked_sub(Span::new().days(i64::from(now.date().weekday().to_monday_zero_offset()))).unwrap();
        let _ = week_read(monday, &zone);
        WEEK_READS.with(|n| n.set(0));
        let calendar = sioul_core::vdir::create(sioul_core::vdir::Kind::Calendars, sioul_core::vdir::LOCAL, "Week read test", None, &["VEVENT"]).unwrap();
        let utc = |hour: i8| now.date().at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        sioul_core::vdir::write_item(&calendar.dir.join("w.ics"), &format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:week-read-test\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:week-read-test\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(10), utc(11))).unwrap();
        let read = week_read(monday, &zone);
        assert_eq!(WEEK_READS.with(std::cell::Cell::get), 1, "read again after the write");
        assert!(read.iter().any(|o| o.uid == "week-read-test"));
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// Health's week reads the calendars once, its week's read (`week_read`),
    /// never once a day: with no plan yet, a day gone by or beyond the plan's
    /// reach takes the week's held times (review of 1880bce, M5: six full reads
    /// a minute on a phone's Porch on a Sunday).
    #[test]
    fn health_s_week_reads_the_calendars_once() {
        let _files = health_files();
        crate::work::HELD_READS.with(|n| n.set(0));
        WEEK_READS.with(|n| n.set(0));
        let reads = Reads::now();
        let (health, _) = reads.known.clone().unwrap_or_default();
        let now = Zoned::now();
        let last_week = now.date().checked_sub(Span::new().days(7)).unwrap();
        let _ = week_view(&health, &reads.state, &reads.records, &reads.knowledge, last_week, &now, &(std::collections::BTreeSet::new(), std::collections::BTreeSet::new()));
        assert_eq!(crate::work::HELD_READS.with(std::cell::Cell::get), 0, "no read a day");
        assert!(WEEK_READS.with(std::cell::Cell::get) <= 1, "the week's one read, or its cache");
    }

    /// "15 min later" on a meal beside a pinned block moves it from where
    /// the page shows it, past the block (the plan's held times,
    /// `work::held_on`), never from its usual time, where the block would
    /// push it back (unified agenda, plan step 5; review of 1880bce, L22).
    #[test]
    fn later_moves_a_meal_from_where_the_page_shows_it() {
        let _files = health_files();
        let health_path = Health::default_path();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        // Meals as usual: lunch eaten from 12:30, got ready from 12:10, until 13:00.
        std::fs::write(&health_path, "[needs]\nmeals_on = true\n").unwrap();
        let now = Zoned::now();
        let zone = now.time_zone().clone();
        let tomorrow = now.date().tomorrow().unwrap();
        let _ = Days::change_file(&Days::default_path(), now.date(), |days| days.forget(tomorrow, "meal:1"));
        // A task pinned to a block tomorrow from 12:00 to 13:00.
        let calendar = sioul_core::vdir::create(sioul_core::vdir::Kind::Calendars, sioul_core::vdir::LOCAL, "Later test", None, &["VEVENT", "VTODO"]).unwrap();
        let edit = TaskEdit { title: "Later test task".into(), estimate: 30, ..TaskEdit::default() };
        sioul_core::vdir::write_item(&calendar.dir.join("later-task.ics"), &sioul_core::tasks::new_task(&edit, "later-task", &zone, &now).unwrap()).unwrap();
        let at = |hour: i8, minute: i8| tomorrow.at(hour, minute, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let utc = |hour: i8| tomorrow.at(hour, 0, 0, 0).to_zoned(zone.clone()).unwrap().with_time_zone(TimeZone::UTC).strftime("%Y%m%dT%H%M%SZ").to_string();
        let block = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:later-block\r\nDTSTAMP:20261001T000000Z\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Later test task\r\n{}:later-task\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n", utc(12), utc(13), sioul_core::blocks::TASK);
        sioul_core::vdir::write_item(&calendar.dir.join("later-block.ics"), &block).unwrap();
        let shared = Arc::new(Shared::default());
        crate::work::publish_now(&shared);
        let lunch = || blocks_now(&load().needs, &days(), tomorrow, &now).into_iter().find(|k| k.key == "meal:1").unwrap();
        let shown = lunch();
        assert!(shown.start >= at(13, 0), "pushed past the block, as the page shows it: {shown:?}");
        let later = serde_json::json!({ "date": tomorrow.to_string(), "key": "meal:1", "action": "later", "minutes": 15 }).to_string();
        assert_eq!(change_need(&later), "");
        let moved = lunch();
        assert_eq!((moved.start, moved.at), (shown.start + 15 * 60, shown.at + 15 * 60), "from where it was shown: {moved:?}");
        crate::work::forget_plan();
        let _ = Days::change_file(&Days::default_path(), now.date(), |days| days.forget(tomorrow, "meal:1"));
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_file(&health_path);
    }

    /// The page and the Porch's doses made from the run's one read (`views`)
    /// are byte for byte those each made from a read of its own (`page`,
    /// `missed`, as before the one read), on a day with a dose due and one to
    /// come (unified agenda, plan step 6; review of 1880bce, L22).
    #[test]
    fn one_read_makes_the_page_and_the_porch_byte_for_byte() {
        let _files = health_files();
        let health_path = Health::default_path();
        std::fs::create_dir_all(health_path.parent().unwrap()).unwrap();
        let hour = |offset: i64| Zoned::now().checked_add(Span::new().hours(offset)).unwrap().strftime("%H:00").to_string();
        std::fs::write(&health_path, format!("[needs]\nmeals_on = true\n\n[[medicine]]\nid = \"lumiel\"\nname = \"Lumiel\"\n[medicine.schedule]\nevery = \"day\"\ntimes = [\"{}\", \"{}\"]\n", hour(-2), hour(2))).unwrap();
        let waiting = Waiting::default();
        // Made within one minute, both ways: a minute's turn between them changes what they say.
        for _ in 0..3 {
            let minute = Zoned::now().timestamp().as_second() / 60;
            let (page_once, missed_once, _, _) = views(&waiting);
            let (page_alone, missed_alone) = (page(&waiting), missed());
            if Zoned::now().timestamp().as_second() / 60 != minute {
                continue;
            }
            assert!(page_once.contains("Lumiel"), "{page_once}");
            assert_eq!(page_once, page_alone);
            assert_eq!(missed_once, missed_alone);
            let _ = std::fs::remove_file(&health_path);
            return;
        }
        panic!("a minute turned at each of three tries");
    }
}
