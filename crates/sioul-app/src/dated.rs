// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Dates asked of you, in the window (the unified agenda, part 4): the
//! agenda's marks (`items`), made by the core's adapters (`sioul_core::dated`)
//! from each store as it is now, a prescription's dates by their kind; and
//! each date's card (`card`, `datedCard(kind, key)`), in the words its own
//! page already says it, with what its own form needs; a letter's own form
//! for its dates while it has neither its task nor its event (`letter_dates`,
//! through `Letters::change`).

use crate::backend::{Shared, json, load_config, say, tr};
use jiff::Zoned;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use sioul_core::agenda::Occurrence;
use sioul_core::dated::{self, Dated, EventSeen, Kind};
use sioul_core::letters::{Letter, Letters};
use sioul_core::tasks::Task;
use sioul_core::timeline;

/// Each day's events as a letter's appointment is matched to them
/// (`dated::letter_event`): its start, whether it is yours (a calendar you
/// write, no meeting with others), and its file's text, read only for that day.
fn events_on<'a>(occurrences: &'a [Occurrence], zone: &TimeZone) -> impl Fn(Date) -> Vec<EventSeen> + 'a {
    let zone = zone.clone();
    move |date: Date| {
        occurrences
            .iter()
            .filter(|o| jiff::Timestamp::from_second(o.start).is_ok_and(|t| t.to_zoned(zone.clone()).date() == date))
            .map(|o| EventSeen { start: o.start, yours: !o.read_only && !o.with_others, text: std::fs::read_to_string(&o.key).unwrap_or_default() })
            .collect()
    }
}

/// The dates the agenda draws from `from` on, `count` days, today and after
/// only: every admin date (`dated::read_all`), and, health being read, a
/// prescription's end and the day its medicines run out, by their kind. An
/// errand's date asked is health too, named by its kind (`agenda::named`).
pub(crate) fn items(tasks: &[Task], occurrences: &[Occurrence], health: Option<&sioul_core::health::Health>, from: Date, count: i64, now: &Zoned, errand: &dyn Fn(&str) -> bool) -> Vec<timeline::Item> {
    let (today, zone) = (now.date(), now.time_zone().clone());
    let mut all = dated::read_all(&load_config(), tasks, &events_on(occurrences, &zone), today, &zone, tr());
    if let Some(health) = health {
        all.extend(dated::of_prescriptions(&health.prescriptions, today));
    }
    dated::within(all, from, count, today)
        .iter()
        .map(|d| {
            let mut item = dated::item(d, today, tr());
            if d.kind == Kind::Task && errand(&d.key) {
                item.privacy = timeline::Privacy::Health;
                item.address.clear();
            }
            item
        })
        .collect()
}

/// The kind a card is asked for ("letter", "notice"…), as `dated::Kind` writes it.
fn kind_of(kind: &str) -> Option<Kind> {
    serde_json::from_value(serde_json::Value::String(kind.to_string())).ok()
}

/// A date's day, with its time when it has one: "Friday 30 October, 17:00".
fn when(date: Date, at: Option<i64>, today: Date, zone: &TimeZone) -> String {
    let day = tr().day_in(date, today);
    match at.and_then(|at| jiff::Timestamp::from_second(at).ok()) {
        Some(at) => format!("{day}, {}", at.to_zoned(zone.clone()).strftime("%H:%M")),
        None => day,
    }
}

/// A date's card (`datedCard(kind, key)`), as JSON: its title, its mark's
/// words, its day, its lines in its own page's words, what its Edit opens
/// (`edit`: "letter-dates", "task", "event", "paper", "contract", "line",
/// "health" or "none") with what that form takes (`item`, `kinds`,
/// `presets`), and its own actions' state. `{"there": false}` once it is gone.
pub(crate) fn card(shared: &Shared, kind: &str, key: &str) -> String {
    let now = Zoned::now();
    let (today, zone) = (now.date(), now.time_zone().clone());
    let gone = || json(&serde_json::json!({ "there": false, "kind": kind, "key": key }));
    let Some(kind) = kind_of(kind) else { return gone() };
    let config = load_config();
    let root = config.notes_root_path();
    let mark = |d: &Dated| dated::item(d, today, tr()).line;
    let head = |d: &Dated| serde_json::json!({ "there": true, "kind": kind, "key": key, "title": d.title, "mark": mark(d), "when": when(d.date, d.at, today, &zone), "filled": dated::filled(d, today), "read": matches!(kind, Kind::Letter | Kind::LetterAppointment) });
    // A paper or a contract with no date to come (none written, or gone by): its card all the same, as its row opens it.
    let undated = |title: &str| serde_json::json!({ "there": true, "kind": kind, "key": key, "title": title, "mark": "", "when": "", "filled": false, "read": false });
    let with = |mut value: serde_json::Value, more: serde_json::Value| {
        if let (Some(out), Some(more)) = (value.as_object_mut(), more.as_object()) {
            out.extend(more.clone());
        }
        json(&value)
    };
    match kind {
        Kind::Letter | Kind::LetterAppointment => {
            let Some(letters) = root.as_deref().and_then(|r| Letters::load(r).ok()) else { return gone() };
            let Some(letter) = letters.get(key).cloned() else { return gone() };
            let shown = letter_card(shared, &letters, &letter, kind, today, &zone);
            json(&shown)
        }
        Kind::Notice => {
            let view: serde_json::Value = serde_json::from_str(&crate::contracts::view()).unwrap_or_default();
            let Some(item) = view["contracts"].as_array().and_then(|list| list.iter().find(|c| c["id"] == key)).cloned() else { return gone() };
            let d = root.as_deref().and_then(|r| sioul_core::contracts::Contracts::load(r).ok()).and_then(|c| dated::of_contracts(&c.list, today).into_iter().find(|d| d.key == key));
            let lines: Vec<String> = [item["kind_label"].as_str(), item["party"].as_str(), item["line"].as_str(), item["cost"].as_str()].into_iter().flatten().filter(|l| !l.trim().is_empty()).map(str::to_string).collect();
            let head = d.as_ref().map_or_else(|| undated(item["title"].as_str().unwrap_or_default()), head);
            with(head, serde_json::json!({ "lines": lines, "edit": "contract", "item": item, "kinds": view["kinds"], "presets": view["presets"] }))
        }
        Kind::Paper => {
            let view: serde_json::Value = serde_json::from_str(&crate::papers::view()).unwrap_or_default();
            let Some(item) = view["families"].as_array().into_iter().flatten().flat_map(|f| f["papers"].as_array().cloned().unwrap_or_default()).find(|p| p["id"] == key) else { return gone() };
            let d = root.as_deref().and_then(|r| sioul_core::papers::Wallet::load(r).ok()).and_then(|w| dated::of_papers(&w.papers, &[], today).into_iter().find(|d| d.key == key));
            let lines: Vec<String> = [item["kind_label"].as_str(), item["holder"].as_str(), item["line"].as_str()].into_iter().flatten().filter(|l| !l.trim().is_empty()).map(str::to_string).collect();
            let head = d.as_ref().map_or_else(|| undated(item["title"].as_str().unwrap_or_default()), head);
            with(head, serde_json::json!({ "lines": lines, "edit": "paper", "item": item, "kinds": view["kinds"], "can_renew": item["can_renew"], "renewal": item["renewal"], "file": item["file"], "file_there": item["file_there"] }))
        }
        Kind::Payment => {
            let ledger = root.as_deref().and_then(|r| sioul_core::budget::Ledger::load_with_bank(r, &sioul_core::words::Words::of(&config)).ok());
            let Some(ledger) = ledger else { return gone() };
            let Some(d) = dated::of_payments(&ledger, today, tr()).into_iter().find(|d| d.key == key) else { return gone() };
            let place: Option<usize> = key.rsplit_once('/').and_then(|(_, p)| p.parse().ok());
            let Some(line) = place.and_then(|p| ledger.lines.get(p)) else { return gone() };
            let budget = ledger.budgets.iter().find(|b| b.id == line.budget).map_or_else(|| line.budget.clone(), |b| b.title.clone());
            let lines = vec![say("dated-payment-budget", &[("budget", budget.clone())])];
            let item = serde_json::json!({ "uri": key, "day": line.date.to_string(), "value": line.amount.cents() as f64 / 100.0, "label": line.label, "budget": line.budget });
            with(head(&d), serde_json::json!({ "lines": lines, "edit": "line", "item": item }))
        }
        Kind::Invoice => {
            let invoices = sioul_core::invoice::all_in(&sioul_core::invoice::folder());
            let Some(invoice) = invoices.iter().find(|i| i.number == key) else { return gone() };
            let Some(d) = dated::of_invoices(std::slice::from_ref(invoice), today, tr()).into_iter().next() else { return gone() };
            let lines: Vec<String> = [invoice.project_title.clone(), say("dated-invoice-total", &[("total", tr().money(invoice.total()))])].into_iter().filter(|l| !l.trim().is_empty()).collect();
            with(head(&d), serde_json::json!({ "lines": lines, "edit": "none", "why": tr().text("dated-invoice-record", None), "number": invoice.number, "paid": invoice.paid }))
        }
        Kind::PrescriptionEnds | Kind::RunsOut => {
            let Ok(health) = sioul_core::health::Health::read(&sioul_core::health::Health::default_path()) else { return gone() };
            let Some(p) = health.prescriptions.iter().find(|p| p.id == key) else { return gone() };
            let Some(mut d) = dated::of_prescriptions(std::slice::from_ref(p), today).into_iter().find(|d| d.kind == kind) else { return gone() };
            // The card names it, as Health does: its title, never on the timeline.
            d.title = p.title.clone();
            let next = crate::health::prescription_next(&health, p, today);
            let lines: Vec<String> = std::iter::once(p.prescriber.clone()).chain(next).filter(|l| !l.trim().is_empty()).collect();
            let mut head = head(&d);
            head["mark"] = serde_json::Value::String(format!("{} · {}", tr().text(kind.word(), None), dated::left(tr(), today, d.date)));
            with(head, serde_json::json!({ "lines": lines, "edit": "health", "fetched_today": p.last_refill == Some(today), "health": true }))
        }
        // A task's date opens its task's card (`CardHost.cardOf`).
        Kind::Task => gone(),
    }
}

/// A letter's card: its sender and kind, its amount, its date asked with the
/// letter's own words, its appointment, the event that stands for it ("In
/// the agenda: Doctor, 14:00"), whether Into the agenda is still offered (no
/// event carries its address), its task; Edit opens its own dates' form while
/// it has neither its task nor its event, else that one's form.
fn letter_card(shared: &Shared, letters: &Letters, letter: &Letter, kind: Kind, today: Date, zone: &TimeZone) -> serde_json::Value {
    let r = &letter.reading;
    let day = |d: Date| tr().day_in(d, today);
    let tasks = sioul_core::tasks::all(zone);
    let task_open = !letter.task.is_empty() && tasks.iter().any(|t| t.uid == letter.task && t.status.is_open());
    let at = r.appointment.and_then(|(d, t)| t.and_then(|(h, m)| d.at(h, m, 0, 0).to_zoned(zone.clone()).ok())).map(|z| z.timestamp().as_second());
    // That day's events, as the agenda read them (the plan's read when current).
    let event = r.appointment.and_then(|(date, _)| {
        let from = sioul_core::timeline::midnight(date, zone);
        let (removed, skipped) = crate::mail::hidden_pim(shared);
        let occurrences = crate::agenda::occurrences_between(from, from + 86_400, &removed, &skipped, &tasks);
        let seen = events_on(&occurrences, zone)(date);
        let found = dated::letter_event(letters, letter, at, &seen)?;
        let o = occurrences.iter().find(|o| o.start == seen[found].start && std::fs::read_to_string(&o.key).ok().as_deref() == Some(seen[found].text.as_str()))?;
        let address = sioul_core::lines::unfold(&seen[found].text).iter().any(|l| sioul_core::lines::name(l) == "LINK" && sioul_core::lines::value(l).trim() == sioul_core::links::letter_uri(&letter.id));
        let time = jiff::Timestamp::from_second(o.start).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default();
        Some(serde_json::json!({ "key": o.key, "start": o.start, "title": o.summary, "line": say("dated-in-the-agenda", &[("title", o.summary.clone()), ("time", time)]), "address": address }))
    });
    let mut lines = Vec::new();
    lines.push(tr().text(&format!("letter-kind-{}", r.kind.id()), None));
    if let Some(amount) = r.amount {
        lines.push(say("letter-amount", &[("amount", tr().money(amount))]));
    }
    if let Some(deadline) = r.deadline {
        lines.push(say("letter-deadline", &[("date", day(deadline)), ("why", r.why.clone())]));
    }
    if let Some((date, time)) = r.appointment {
        let when = match time {
            Some((h, m)) => format!("{} {h:02}:{m:02}", day(date)),
            None => day(date),
        };
        lines.push(say("letter-appointment", &[("when", when)]));
    }
    if let Some(line) = event.as_ref().and_then(|e| e["line"].as_str()) {
        lines.push(line.to_string());
    }
    let date = match kind {
        Kind::LetterAppointment => r.appointment.map(|(d, _)| d),
        _ => r.deadline,
    };
    let title = if r.sender.trim().is_empty() { tr().text("letter-unknown-sender", None) } else { r.sender.clone() };
    let (mark, when_line, filled) = match date {
        Some(d) => {
            let shown = Dated { kind, key: letter.id.clone(), address: String::new(), title: title.clone(), date: d, at: if kind == Kind::LetterAppointment { at } else { None } };
            (dated::item(&shown, today, tr()).line, self::when(d, shown.at, today, zone), dated::filled(&shown, today))
        }
        None => (String::new(), String::new(), false),
    };
    let edit = if task_open { "task" } else if event.is_some() { "event" } else { "letter-dates" };
    serde_json::json!({
        "there": true,
        "kind": kind,
        "key": letter.id,
        "title": title,
        "mark": mark,
        "when": when_line,
        "filled": filled,
        "read": true,
        "lines": lines,
        "edit": edit,
        "file": letters.file_path(letter).display().to_string(),
        "task": letter.task,
        "task_open": task_open,
        "event": event,
        "into_agenda": r.appointment.is_some() && !event.as_ref().is_some_and(|e| e["address"] == true),
        "can_task": r.deadline.is_some() && letter.task.is_empty(),
        "project": letter.project,
        "item": {
            "deadline": r.deadline.map(|d| d.to_string()).unwrap_or_default(),
            "appointment": r.appointment.map(|(d, _)| d.to_string()).unwrap_or_default(),
            "time": r.appointment.and_then(|(_, t)| t).map(|(h, m)| format!("{h:02}:{m:02}")).unwrap_or_default(),
        },
    })
}

/// A letter's dates as its own small form gives them back: its date asked
/// and its appointment's day and time, each "" for none. Read from a scan,
/// a date can be put right here while the letter has neither its task nor
/// its event (decision 19); written through `Letters::change`, its other
/// fields as the file holds them then. Returns what went wrong, else "".
pub(crate) fn letter_dates(id: &str, edit: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return tr().text("papers-no-store", None) };
    letter_dates_in(&root, id, edit)
}

fn letter_dates_in(root: &std::path::Path, id: &str, edit: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Edit {
        #[serde(default)]
        deadline: String,
        #[serde(default)]
        appointment: String,
        #[serde(default)]
        time: String,
    }
    let Ok(edit) = serde_json::from_str::<Edit>(edit) else { return tr().text("papers-gone", None) };
    let parse = |text: &str| -> Result<Option<Date>, String> {
        let text = text.trim();
        if text.is_empty() { Ok(None) } else { text.parse().map(Some).map_err(|_| tr().text("dated-letter-bad-date", None)) }
    };
    let time = edit.time.trim().split_once(':').and_then(|(h, m)| Some((h.trim().parse::<i8>().ok()?, m.trim().parse::<i8>().ok()?))).filter(|(h, m)| (0..24).contains(h) && (0..60).contains(m));
    if !edit.time.trim().is_empty() && time.is_none() {
        return tr().text("dated-letter-bad-time", None);
    }
    let (deadline, appointment) = match (parse(&edit.deadline), parse(&edit.appointment)) {
        (Ok(d), Ok(a)) => (d, a),
        (Err(e), _) | (_, Err(e)) => return e,
    };
    let result = Letters::try_change(root, |letters| {
        let mut letter = letters.get(id).cloned().ok_or_else(|| tr().text("papers-gone", None))?;
        letter.reading.deadline = deadline;
        letter.reading.appointment = appointment.map(|d| (d, time));
        letters.put(letter);
        Ok(())
    });
    result.err().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A letter's dates put right in its own form: written through the
    /// letters' file, its other fields kept; a date that does not read is
    /// said, and nothing is written.
    #[test]
    fn a_letter_s_dates_are_put_right_in_its_form() {
        sioul_core::config::stand_ins_only();
        let root = sioul_core::config::data_dir().join("dated-letter-notes");
        let _ = std::fs::remove_dir_all(&root);
        let letter = Letter { id: "caf-1".into(), file: "letters/inbox/caf.pdf".into(), status: "new".into(), reading: sioul_core::letters::Reading { sender: "CAF".into(), deadline: Some("2026-11-30".parse().unwrap()), why: "within two months".into(), ..Default::default() }, ..Default::default() };
        Letters::change(&root, |letters| letters.put(letter.clone())).unwrap();
        assert_eq!(letter_dates_in(&root, "caf-1", r#"{"deadline": "2026-11-03", "appointment": "2026-11-05", "time": "14:30"}"#), "");
        let read = Letters::load(&root).unwrap();
        let kept = read.get("caf-1").unwrap();
        assert_eq!((kept.reading.deadline, kept.reading.appointment), (Some("2026-11-03".parse().unwrap()), Some(("2026-11-05".parse().unwrap(), Some((14, 30))))));
        assert_eq!((kept.reading.sender.as_str(), kept.reading.why.as_str(), kept.file.as_str()), ("CAF", "within two months", "letters/inbox/caf.pdf"), "the rest as it was");
        assert_ne!(letter_dates_in(&root, "caf-1", r#"{"deadline": "the 3rd"}"#), "", "said");
        assert_eq!(Letters::load(&root).unwrap().get("caf-1").unwrap().reading.deadline, Some("2026-11-03".parse().unwrap()), "nothing written");
        let _ = std::fs::remove_dir_all(&root);
    }
}
