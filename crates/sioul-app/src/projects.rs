// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Projects, time and invoices, for the window: a project's page, the Time
//! page, time noted by hand, an invoice made from a project's unbilled hours
//! (its record kept, its hours marked, its payment expected in the project's
//! budget), and paid.

use crate::backend::{QtThread, Shared, json, load_config, say, tr};
use crate::work::{self, loaded};
use jiff::Zoned;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::Deserialize;
use sioul_core::budget::{self, Line};
use sioul_core::projects::{self, ProjectEdit, ProjectStore};
use sioul_core::invoice::{self, Invoice};
use sioul_core::money::Money;
use sioul_core::timelog::{self, Session};
use sioul_core::timereport::{self, Entry};
use sioul_core::{links, projectview};
use std::path::PathBuf;
use std::sync::Arc;

fn store() -> Result<ProjectStore, String> {
    let root = load_config().notes_root_path().ok_or_else(|| tr().text("error-no-store", None))?;
    ProjectStore::load(&root)
}

/// Every session with its project and title.
fn entries(shared: &Shared) -> Vec<Entry> {
    let loaded = loaded(shared);
    timereport::entries(&timelog::sessions(), &loaded.tasks, &loaded.projects, &TimeZone::system())
}

/// The projects, for the list.
pub(crate) fn rows(shared: &Shared) -> String {
    let loaded = loaded(shared);
    json(&projectview::rows(&loaded, &entries(shared), &loaded.projects))
}

/// One project's page, as JSON; "" when it is gone.
pub(crate) fn page(shared: &Shared, id: &str) -> String {
    let loaded = loaded(shared);
    let Some(project) = loaded.projects.iter().find(|c| c.id == id) else { return String::new() };
    let config = load_config();
    let now = Zoned::now().timestamp().as_second();
    // Its mail as the Porch takes it: never what it sets aside.
    let gate = sioul_core::porch::Gate::load(&config, now);
    let view = projectview::view(&loaded, project, &entries(shared), &invoice::all_in(&invoice::folder()), config.invoice.rate, now, tr(), &gate);
    json(&view)
}

/// Saves a project from its form (a new one when `id` is empty); returns {"id"} or {"error"}.
pub(crate) fn save(qt: &QtThread, shared: &Arc<Shared>, id: &str, edit: &str) -> String {
    let result = serde_json::from_str::<ProjectEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        if edit.title.trim().is_empty() {
            return Err(tr().text("project-no-title", None));
        }
        let store = store()?;
        projects::save_project(&store.file(), id, &edit)
    });
    work::refresh(qt, shared);
    match result {
        Ok(id) => serde_json::json!({ "id": id }).to_string(),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

/// A project taken out of the list; its tasks, notes, mail and time
/// stay. Returns what went wrong, else "".
pub(crate) fn remove(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let result = store().and_then(|store| projects::remove_project(&store.file(), id));
    work::refresh(qt, shared);
    result.err().unwrap_or_default()
}

/// A budget made (`id` empty) or changed; returns {"id"} or {"error"}.
pub(crate) fn save_budget(qt: &QtThread, shared: &Arc<Shared>, id: &str, edit: &str) -> String {
    let result = serde_json::from_str::<budget::BudgetEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        let root = load_config().notes_root_path().ok_or_else(|| tr().text("error-no-store", None))?;
        budget::save_budget(&root.join(budget::LEDGER), id, &edit)
    });
    crate::backend::show(qt, shared);
    match result {
        Ok(id) => serde_json::json!({ "id": id }).to_string(),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

/// A budget taken out (its lines stay in the file), or one of its lines
/// (`sioul:budget/<budget>/<place>`); returns what went wrong, else "".
pub(crate) fn remove_budget(qt: &QtThread, shared: &Arc<Shared>, what: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return tr().text("error-no-store", None) };
    let path = root.join(budget::LEDGER);
    let result = match what.strip_prefix("sioul:budget/").and_then(|rest| rest.rsplit_once('/')) {
        Some((_, place)) => place.parse::<usize>().map_err(|e| e.to_string()).and_then(|place| budget::remove_line(&path, place)),
        None => budget::remove_budget(&path, what),
    };
    crate::backend::show(qt, shared);
    result.err().unwrap_or_default()
}

/// A line of the budgets' file changed: label, amount, date; returns what went wrong, else "".
pub(crate) fn change_line(qt: &QtThread, shared: &Arc<Shared>, uri: &str, label: &str, amount: f64, date: &str) -> String {
    let Some(root) = load_config().notes_root_path() else { return tr().text("error-no-store", None) };
    let path = root.join(budget::LEDGER);
    let today = Zoned::now().date();
    let result = uri
        .strip_prefix("sioul:budget/")
        .and_then(|rest| rest.rsplit_once('/'))
        .and_then(|(_, place)| place.parse::<usize>().ok())
        .ok_or_else(|| tr().text("budget-line-gone", None))
        .and_then(|place| budget::change_line(&path, place, label, amount, date.parse().unwrap_or(today), today));
    crate::backend::show(qt, shared);
    result.err().unwrap_or_default()
}

/// The Time page for `period` ("week", "month", "year") around `anchor`, one project or all.
pub(crate) fn time(shared: &Shared, period: &str, anchor: &str, project: &str) -> String {
    let loaded = loaded(shared);
    let today = Zoned::now().date();
    let anchor = anchor.parse::<Date>().unwrap_or(today);
    let sessions = timelog::sessions();
    // Time noted by hand has no task, or was not timed: it can be taken out.
    let by_hand: std::collections::BTreeSet<String> = sessions.iter().filter(|s| s.task.is_empty() || !s.project.is_empty()).map(Session::key).collect();
    let all = timereport::entries(&sessions, &loaded.tasks, &loaded.projects, &TimeZone::system());
    let config = load_config();
    let mut view = timereport::view(&all, &loaded.projects, &|key| by_hand.contains(key), period, anchor, Some(project).filter(|p| !p.is_empty()), config.invoice.rate, today, tr());
    // How each stretch's minutes were known, said quietly: timed, typed, corrected, or not known.
    let kinds: std::collections::BTreeMap<String, Option<timelog::Kind>> = sessions.iter().map(|s| (s.key(), s.kind)).collect();
    for entry in &mut view.entries {
        let kind = kinds.get(&entry.key).copied().flatten();
        entry.kind = timelog::Kind::id(kind).to_string();
        entry.kind_said = tr().text(&format!("time-kind-{}", entry.kind), None);
    }
    json(&view)
}

/// A project's billable time from `from` to `to`, written as a spreadsheet at
/// `target` (a file address); returns the file written.
pub(crate) fn export_time_csv(shared: &Shared, project: &str, from: &str, to: &str, target: &str) -> Result<String, String> {
    let loaded = loaded(shared);
    let Some(found) = loaded.projects.iter().find(|c| c.id == project) else { return Err(tr().text("project-gone", None)) };
    let (Ok(from), Ok(to)) = (from.parse::<Date>(), to.parse::<Date>()) else { return Err(tr().text("time-bad-day", None)) };
    let all = timereport::entries(&timelog::sessions(), &loaded.tasks, &loaded.projects, &TimeZone::system());
    let config = load_config();
    let french = tr().text("qt-locale", None).starts_with("fr");
    let words = ["time-csv-what", "time-csv-hours", "time-csv-rate", "time-csv-amount", "time-csv-total"].map(|w| tr().text(w, None));
    let csv = timereport::billable_csv(&all, found, from, to, config.invoice.rate, french, [&words[0], &words[1], &words[2], &words[3], &words[4]]);
    let path = crate::backend::local_path(target);
    std::fs::write(&path, csv).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path.display().to_string())
}

/// A file address's path with its %-escapes undone ("Mes%20factures" → "Mes factures").
pub(crate) fn percent_decoded(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) = path.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Time noted by hand, as its form gives it.
#[derive(Deserialize)]
struct TimeEdit {
    /// "2026-10-05".
    day: String,
    /// "14:00", or "" for the middle of the day.
    #[serde(default)]
    at: String,
    minutes: u32,
    #[serde(default)]
    project: String,
    #[serde(default)]
    task: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    unbilled: bool,
}

/// Notes time by hand; returns what went wrong, else "".
pub(crate) fn note_time(qt: &QtThread, shared: &Arc<Shared>, edit: &str) -> String {
    let result = serde_json::from_str::<TimeEdit>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        if edit.minutes == 0 {
            return Err(tr().text("time-no-minutes", None));
        }
        if edit.project.is_empty() && edit.task.is_empty() {
            return Err(tr().text("time-no-project-chosen", None));
        }
        let day = edit.day.parse::<Date>().map_err(|_| tr().text("time-bad-day", None))?;
        let (hour, minute) = edit.at.split_once(':').and_then(|(h, m)| Some((h.trim().parse::<i8>().ok()?, m.trim().parse::<i8>().ok()?))).unwrap_or((12, 0));
        let start = day.at(hour.clamp(0, 23), minute.clamp(0, 59), 0, 0).to_zoned(TimeZone::system()).map_err(|e| e.to_string())?.timestamp().as_second();
        timelog::record(&Session { task: edit.task, project: edit.project, start, minutes: edit.minutes, note: edit.note.trim().to_string(), unbilled: edit.unbilled, kind: Some(timelog::Kind::Typed), ..Session::default() })
    });
    work::refresh(qt, shared);
    result.err().unwrap_or_default()
}

/// A stretch of time changed, as its form gives it: its day, from when to
/// when, what it was, its task and project.
#[derive(Deserialize)]
struct TimeChange {
    day: String,
    from: String,
    to: String,
    #[serde(default)]
    project: String,
    #[serde(default)]
    task: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    unbilled: bool,
}

/// Any stretch of time changed once noted, by the timer or by hand: its
/// start, its end, what it was, its task, its project. Billed time stays as
/// billed. Returns what went wrong, else "".
pub(crate) fn change_time(qt: &QtThread, shared: &Arc<Shared>, key: &str, edit: &str) -> String {
    let result = serde_json::from_str::<TimeChange>(edit).map_err(|e| e.to_string()).and_then(|edit| {
        if timelog::sessions().iter().any(|s| s.key() == key && !s.invoice.is_empty()) {
            return Err(tr().text("time-billed-stays", None));
        }
        let day = edit.day.parse::<Date>().map_err(|_| tr().text("time-bad-day", None))?;
        let clock = |text: &str| text.split_once(':').and_then(|(h, m)| jiff::civil::Time::new(h.trim().parse().ok()?, m.trim().parse().ok()?, 0, 0).ok());
        let (Some(from), Some(to)) = (clock(&edit.from), clock(&edit.to)) else { return Err(tr().text("time-bad-hours", None)) };
        let start = day.to_datetime(from).to_zoned(TimeZone::system()).map_err(|e| e.to_string())?.timestamp().as_second();
        // Ending at or before it began: past midnight.
        let mut end = day.to_datetime(to).to_zoned(TimeZone::system()).map_err(|e| e.to_string())?.timestamp().as_second();
        if end <= start {
            end += 86_400;
        }
        let minutes = u32::try_from((end - start + 30) / 60).unwrap_or(0);
        if minutes == 0 {
            return Err(tr().text("time-no-minutes", None));
        }
        if edit.project.is_empty() && edit.task.is_empty() {
            return Err(tr().text("time-no-project-chosen", None));
        }
        let changed = timelog::change_in(&timelog::folder(), key, |s| {
            s.start = start;
            // A timed stretch whose minutes change becomes "corrected" (docs/capacity.md).
            s.set_minutes(minutes);
            s.task = edit.task.clone();
            s.project = edit.project.clone();
            s.note = edit.note.trim().to_string();
            s.unbilled = edit.unbilled;
        })?;
        if changed { Ok(()) } else { Err(tr().text("time-gone", None)) }
    });
    work::refresh(qt, shared);
    result.err().unwrap_or_default()
}

/// The tasks time can be given to, as JSON [{uid, title, project}]: the open
/// ones, and `keep` (the stretch's own task, done or not).
pub(crate) fn task_choices(shared: &Shared, keep: &str) -> String {
    let loaded = loaded(shared);
    let mut tasks: Vec<&sioul_core::tasks::Task> = loaded.tasks.iter().filter(|t| t.status.is_open() || t.uid == keep).collect();
    tasks.sort_by_key(|t| t.title.to_lowercase());
    json(&tasks.iter().map(|t| serde_json::json!({ "uid": t.uid, "title": t.title, "project": t.projects.first().cloned().unwrap_or_default() })).collect::<Vec<_>>())
}

/// Takes out time noted by hand; billed time stays.
pub(crate) fn remove_time(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    let billed = timelog::sessions().iter().any(|s| s.key() == key && !s.invoice.is_empty());
    if billed {
        return tr().text("time-billed-stays", None);
    }
    let result = timelog::remove_in(&timelog::folder(), &[key.to_string()]);
    work::refresh(qt, shared);
    match result {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}

/// Where an invoice's PDF goes: the folder set, else ~/Documents/Invoices (Factures in French).
fn pdf_path(invoice: &Invoice) -> PathBuf {
    let config = load_config();
    let folder = if config.invoice.folder.trim().is_empty() {
        sioul_core::config::expand_home(&format!("~/Documents/{}", tr().text("invoice-folder-name", None)))
    } else {
        sioul_core::config::expand_home(config.invoice.folder.trim())
    };
    let client: String = invoice.client.chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-').collect();
    // A prefix such as "F/2026-" makes no folder, and nothing a system refuses in a name.
    let number: String = invoice.number.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '-' }).collect();
    folder.join(format!("{} {}.pdf", number.trim_matches('.'), client.trim()).trim().to_string())
}

/// Why invoices cannot be numbered here now, as the window's answer
/// ({"error", "elsewhere"}: "elsewhere" offers to make them here); None when they can.
fn invoices_held_elsewhere(shared: &Shared) -> Option<String> {
    let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
    let (keeper, unwritten) = crate::share::keeper(INVOICES, sioul_sync::lease::Rule::StaysPut, active, false);
    let now = jiff::Timestamp::now().as_second();
    let refused = |text: String, elsewhere: bool| Some(serde_json::json!({ "error": text, "elsewhere": elsewhere }).to_string());
    if unwritten {
        return refused(tr().text("invoice-unchecked", None), false);
    }
    if !keeper.mine {
        return refused(say("invoice-elsewhere", &[("computer", keeper.name)]), true);
    }
    if !keeper.settled {
        return refused(tr().text("invoice-settling", None), false);
    }
    // Another computer alive but not heard from lately: the sync may hold its claim back.
    if keeper.others > 0 && now - keeper.others_heard > 3 * 60 {
        return refused(tr().text("invoice-others-silent", None), false);
    }
    None
}

/// The lease's name for invoices' numbers.
pub(crate) const INVOICES: &str = "invoices";

/// Invoices made on this computer from now on, on purpose; returns what to say.
pub(crate) fn take_invoices(shared: &Shared) -> String {
    let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
    let (keeper, unwritten) = crate::share::keeper(INVOICES, sioul_sync::lease::Rule::StaysPut, active, true);
    if unwritten {
        tr().text("invoice-unchecked", None)
    } else if keeper.settled {
        tr().text("invoice-here", None)
    } else {
        tr().text("invoice-settling", None)
    }
}

/// This computer's claim on invoices, renewed each minute while you make some (a sender set).
pub(crate) fn keep_invoices(shared: &Shared) {
    if !load_config().invoice.name.trim().is_empty() && crate::share::on() {
        let active = shared.active.load(std::sync::atomic::Ordering::Relaxed);
        let _ = crate::share::keeper(INVOICES, sioul_sync::lease::Rule::StaysPut, active, false);
    }
}

/// Who a project bills: the contact it names, with its postal address, else the name as written.
fn client_of(shared: &Shared, client: &str) -> (String, String) {
    let loaded = loaded(shared);
    // A contact's address, or a name that is one of your contacts' (its name or organisation).
    let named = || loaded.contacts.iter().find(|c| !client.trim().is_empty() && (c.name.eq_ignore_ascii_case(client.trim()) || c.org.eq_ignore_ascii_case(client.trim()))).cloned();
    match client.strip_prefix("sioul:contact/").map(sioul_core::notes::decode).and_then(|uid| loaded.contacts.iter().find(|c| c.uid == uid).cloned()).or_else(named) {
        Some(contact) => {
            let name = if contact.org.is_empty() || contact.org == contact.name { contact.name.clone() } else { format!("{}\n{}", contact.org, contact.name) };
            (name, contact.addresses.first().map(|a| a.value.clone()).unwrap_or_default())
        }
        None => (client.to_string(), String::new()),
    }
}

/// Makes the invoice of a project's unbilled hours: its record, its hours
/// marked, its payment expected in the project's budget. Returns {"number",
/// "html", "pdf"} for the window to print, or {"error"}.
pub(crate) fn make_invoice(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    // One computer numbers invoices (`sioul_sync::lease`): two at once could give one number twice.
    if let Some(elsewhere) = invoices_held_elsewhere(shared) {
        return elsewhere;
    }
    let result = (|| -> Result<serde_json::Value, String> {
        let config = load_config();
        let store = store()?;
        let project = store.get(id).ok_or_else(|| tr().text("project-gone", None))?.clone();
        let existing = invoice::all_in(&invoice::folder());
        let today = Zoned::now().date();
        let number = invoice::next_number(&existing, &config.invoice.prefix, today.year());
        let (client, address) = client_of(shared, project.client.as_deref().unwrap_or(""));
        let made = invoice::make(&entries(shared), &project, &client, &address, &config.invoice, &number, today).ok_or_else(|| tr().text("invoice-nothing", None))?;
        invoice::save_in(&invoice::folder(), &made)?;
        timelog::update_in(&timelog::folder(), &made.sessions, |s| s.invoice = number.clone())?;
        // Expected in the project's budget until paid.
        if let Some(budget_id) = project.budget.as_deref().filter(|b| !b.is_empty()) {
            let ledger = store.root.join(budget::LEDGER);
            let line = Line {
                budget: budget_id.to_string(),
                date: made.due.parse().unwrap_or(today),
                amount: Money(made.total_cents),
                label: say("invoice-budget-line", &[("number", number.clone()), ("client", client.lines().next().unwrap_or("").to_string())]),
                planned: true,
                reserve: None,
                links: vec![format!("sioul:invoice/{number}"), links::project_uri(&project.id)],
                preset: None,
            };
            budget::record_line(&ledger, &line, &say("invoice-budget-origin", &[("number", number.clone())]))?;
        }
        Ok(serde_json::json!({ "number": number, "html": invoice::html(&made, tr()), "pdf": pdf_path(&made).display().to_string() }))
    })();
    work::refresh(qt, shared);
    match result {
        Ok(answer) => answer.to_string(),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

/// An invoice made before, to print again: {"html", "pdf"}.
pub(crate) fn invoice_again(number: &str) -> String {
    match invoice::all_in(&invoice::folder()).into_iter().find(|i| i.number == number) {
        Some(found) => serde_json::json!({ "html": invoice::html(&found, tr()), "pdf": pdf_path(&found).display().to_string() }).to_string(),
        None => serde_json::json!({ "error": tr().text("invoice-gone", None) }).to_string(),
    }
}

/// An invoice paid (or not): its record says so, and its budget line is no
/// longer planned but dated today.
pub(crate) fn set_paid(qt: &QtThread, shared: &Arc<Shared>, number: &str, paid: bool) -> String {
    let result = (|| -> Result<(), String> {
        let mut found = invoice::all_in(&invoice::folder()).into_iter().find(|i| i.number == number).ok_or_else(|| tr().text("invoice-gone", None))?;
        found.paid = paid;
        invoice::save_in(&invoice::folder(), &found)?;
        if paid && let Ok(store) = store() {
            budget::settle_line(&store.root.join(budget::LEDGER), &format!("sioul:invoice/{number}"), Zoned::now().date())?;
        }
        Ok(())
    })();
    work::refresh(qt, shared);
    result.err().unwrap_or_default()
}
