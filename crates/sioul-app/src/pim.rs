// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contacts and calendars behind the window.
//!
//! What you change is written to the folders on disk at once, then the
//! account's sync sends it, right away when the server answers, later when it
//! does not: nothing waits on the network. Each contacts-and-calendars account
//! syncs in the background every fifteen minutes, and at once after a change
//! here. Deleting waits ten seconds with "Undo", as in the mail.

use crate::backend::{QtThread, Shared, config_path, json, load_config, say, set_status, show, tell, tr};
use crate::mail;
use cxx_qt_lib::QString;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use sioul_core::agenda::{self, EventEdit};
use sioul_core::config::{self, Account};
use sioul_core::contacts::{self, ContactEdit};
use sioul_core::vdir::{self, Collection, Kind};
use sioul_core::{maildir, reading, view};
use sioul_sync::dav::{self, Homes};
use sioul_sync::{Control, SyncError, google, secret};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// Between two syncs of an account's contacts and calendars, unless something changed here.
const DAV_PAUSE: Duration = Duration::from_secs(15 * 60);

/// What the contacts and agenda pages show: their search and their stretch of days.
#[derive(Clone)]
pub(crate) struct PimState {
    pub query: String,
    /// The contacts of one category only; empty: all of them.
    pub category: String,
    pub from: jiff::civil::Date,
    pub days: i64,
    /// What comes only: events over already are left out (the list opened on today).
    pub upcoming: bool,
}

impl Default for PimState {
    fn default() -> PimState {
        PimState { query: String::new(), category: String::new(), from: Zoned::now().date(), days: view::AGENDA_DAYS, upcoming: true }
    }
}

/// Computes both pages on a thread and shows them.
pub(crate) fn show_pim(qt: &QtThread, shared: &Arc<Shared>) {
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.pim_job, move |shared| {
        let generation = shared.pim_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let state = shared.pim.lock().map(|s| s.clone()).unwrap_or_default();
        let (removed, skipped) = mail::hidden_pim(shared);
        // The phone's home-screen card reads its month of events again (homecard.rs).
        crate::homecard::agenda_seen(&removed, &skipped);
        let everyone: Vec<contacts::Contact> = contacts::all().into_iter().filter(|c| !removed.contains(Path::new(&c.key))).collect();
        let contacts = json(&view::contacts(&everyone, &state.query, &state.category, tr()));
        let zone = TimeZone::system();
        let from = state.from.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second());
        let to = from + state.days * 86_400 + 3_600;
        let now = Zoned::now().timestamp().as_second();
        let occurrences: Vec<agenda::Occurrence> = agenda::occurrences(from, to)
            .into_iter()
            .filter(|o| !removed.contains(Path::new(&o.key)) && !skipped.contains(&(PathBuf::from(&o.key), o.start)))
            // What comes: an event over is the past, shown only when you go back to it.
            .filter(|o| !state.upcoming || o.end.max(o.start) > now)
            .collect();
        let mut shown = view::agenda(&occurrences, state.from, state.days, tr(), &zone);
        // The planning shows the hours given to something, not the night.
        view::set_hours(&mut shown, &load_config().week_hours(), &zone);
        let agenda = json(&shown);
        let clashes = overlaps(14, &removed, &skipped);
        let _ = qt.queue(move |mut sioul| {
            // Only the newest (see `backend::show`).
            if sioul.shared().pim_generation.load(Ordering::Relaxed) != generation || sioul.shared().pim_shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
                return;
            }
            sioul.as_mut().set_contacts(QString::from(&contacts));
            sioul.as_mut().set_agenda(QString::from(&agenda));
            sioul.as_mut().set_overlaps(QString::from(&clashes));
        });
    });
}

/// The account a contact or event file belongs to, from its folder.
pub(crate) fn account_of(path: &Path) -> Option<String> {
    [Kind::Contacts, Kind::Calendars]
        .into_iter()
        .find_map(|kind| path.strip_prefix(kind.root()).ok().and_then(|rest| rest.components().next()).map(|c| c.as_os_str().to_string_lossy().to_string()))
}

/// The collection a file is in.
pub(crate) fn collection_of(path: &Path) -> Option<Collection> {
    let dir = path.parent()?;
    [Kind::Contacts, Kind::Calendars].into_iter().flat_map(vdir::collections).find(|c| c.dir == dir)
}

/// A file inside Sioul's address books and calendars, never elsewhere: no
/// "..", which `starts_with` would let through ("contacts/../../.bashrc").
pub(crate) fn ours(key: &str) -> Option<PathBuf> {
    let path = PathBuf::from(key);
    let climbs = path.components().any(|c| c == std::path::Component::ParentDir);
    let inside = [Kind::Contacts, Kind::Calendars].iter().any(|k| path.starts_with(k.root()));
    (inside && !climbs && path.is_file()).then_some(path)
}

/// The account's background sync runs now.
pub(crate) fn nudge(shared: &Shared, account: &str) {
    if let Some(control) = shared.dav_watchers.lock().ok().and_then(|w| w.get(account).cloned()) {
        control.nudge();
    }
}

/// Stops an account's background sync: it was removed.
pub(crate) fn stop_dav_watcher(shared: &Arc<Shared>, account: &str) {
    if let Some(control) = shared.dav_watchers.lock().ok().and_then(|mut w| w.remove(account)) {
        control.stop();
    }
}

/// Starts a background sync for each contacts-and-calendars account that has none.
pub(crate) fn start_dav_watchers(qt: &QtThread, shared: &Arc<Shared>) -> usize {
    if crate::backend::offline() {
        return 0;
    }
    let accounts: Vec<Account> = load_config().accounts.into_iter().filter(Account::is_dav).collect();
    let count = accounts.len();
    for account in accounts {
        start_dav_watcher(qt, shared, account);
    }
    count
}

pub(crate) fn start_dav_watcher(qt: &QtThread, shared: &Arc<Shared>, account: Account) {
    let control = Arc::new(Control::default());
    {
        let Ok(mut watchers) = shared.dav_watchers.lock() else { return };
        if let Some(running) = watchers.get(&account.id) {
            // Asked again ("Sync now", an account changed): a refused password is tried again too.
            running.retry();
            return;
        }
        watchers.insert(account.id.clone(), Arc::clone(&control));
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        // After a lasting error (a refused password) it waits for a nudge, never ends.
        dav::watch(&account, &control, DAV_PAUSE, |result| reported(&qt, &shared, &account, result));
        if let Ok(mut watchers) = shared.dav_watchers.lock()
            && watchers.get(&account.id).is_some_and(|c| Arc::ptr_eq(c, &control))
        {
            watchers.remove(&account.id);
        }
    });
}

/// What a sync said: the account's status, conflicts in the status line, the pages again.
fn reported(qt: &QtThread, shared: &Arc<Shared>, account: &Account, result: Result<dav::Report, SyncError>) {
    let (line, problem) = match &result {
        Ok(_) => (say("dav-synced-at", &[("time", Zoned::now().strftime("%H:%M").to_string())]), false),
        Err(e) => (e.sentence(tr(), &account.id), true),
    };
    if let Ok(mut statuses) = shared.statuses.lock() {
        statuses.insert(account.id.clone(), (line.clone(), problem));
    }
    crate::backend::want_password(shared, &account.id, result.as_ref().err());
    // The pages again only when something came, went or clashed: a sync that
    // changed nothing every quarter of an hour costs nothing (a phone's battery).
    let changed = match result {
        Ok(report) => {
            let changed = report.sent + report.received + report.removed > 0 || !report.conflicts.is_empty();
            for conflict in report.conflicts {
                tell(qt, shared, say("dav-conflict", &[("path", conflict.display().to_string())]));
            }
            // An invitation you had answered otherwise on another device: its answer kept, said once.
            for (summary, kept) in report.answered {
                tell(qt, shared, say("dav-answered-elsewhere", &[("summary", summary), ("answer", kept.to_ascii_lowercase())]));
            }
            changed
        }
        Err(_) => {
            set_status(qt, line);
            true
        }
    };
    if changed {
        show(qt, shared);
        show_pim(qt, shared);
        crate::work::show_work(qt, shared);
    }
}

/// A contact, every field, for its card and its form.
pub(crate) fn contact(key: &str) -> String {
    let Some(path) = ours(key) else { return String::new() };
    let Some(book) = collection_of(&path) else { return String::new() };
    contacts::read(&path, &book).map(|c| json(&c)).unwrap_or_default()
}

/// Saves the contact form: into its file, or a new one in the first address book; then sent.
/// Returns the contact's file.
pub(crate) fn save_contact(qt: &QtThread, shared: &Arc<Shared>, key: &str, edit: &str) -> Result<String, String> {
    let edit: ContactEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
    if edit.name.trim().is_empty() && edit.emails.iter().all(|e| e.value.trim().is_empty()) {
        return Err(tr().text("contact-no-name", None));
    }
    let (path, text) = match ours(key) {
        Some(path) => {
            let current = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            (path, contacts::apply(&current, &edit))
        }
        None => {
            let book = contacts::default_book().ok_or_else(|| tr().text("dav-no-book", None))?;
            (contacts::new_path(&book), contacts::new_card(&edit))
        }
    };
    vdir::write_item(&path, &text)?;
    if let Some(account) = account_of(&path) {
        nudge(shared, &account);
    }
    tell(qt, shared, tr().text("contact-saved", None));
    show_pim(qt, shared);
    Ok(path.display().to_string())
}

/// The address books a contact can go into: [{id, name}].
pub(crate) fn books() -> String {
    let config = load_config();
    let rows: Vec<serde_json::Value> = vdir::collections(Kind::Contacts).into_iter().filter(|c| !c.read_only).map(|c| serde_json::json!({ "id": format!("{}/{}", c.account, c.id), "name": c.label(&config, tr()) })).collect();
    json(&rows)
}

/// A contact moved to another address book: written there, then taken out
/// here. When the book would not keep something the card holds, nothing moves
/// until `confirmed`: returns `{"losses": [words]}`, else `{"key"}` or `{"error"}`.
pub(crate) fn move_contact(qt: &QtThread, shared: &Arc<Shared>, key: &str, book: &str, confirmed: bool) -> String {
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let Some(from) = ours(key) else { return fail(tr().text("contact-gone", None)) };
    let Some(target) = vdir::collections(Kind::Contacts).into_iter().find(|c| !c.read_only && format!("{}/{}", c.account, c.id) == book) else { return fail(tr().text("dav-no-book", None)) };
    if collection_of(&from).is_some_and(|c| c.read_only) {
        return fail(tr().text("contact-read-only", None));
    }
    if from.parent() == Some(target.dir.as_path()) {
        return serde_json::json!({ "key": from.display().to_string() }).to_string();
    }
    let Ok(text) = std::fs::read_to_string(&from) else { return fail(tr().text("contact-gone", None)) };
    let provider = sioul_core::capabilities::provider_of(load_config().account(&target.account));
    let losses = sioul_core::capabilities::contact_losses(&text, provider);
    if !losses.is_empty() && !confirmed {
        return serde_json::json!({ "losses": losses.iter().map(|l| tr().text(&format!("loss-{l}"), None)).collect::<Vec<_>>() }).to_string();
    }
    let to = contacts::new_path(&target);
    if let Err(e) = vdir::write_item(&to, &text).and_then(|()| std::fs::remove_file(&from).map_err(|e| e.to_string())) {
        return fail(e);
    }
    for account in [account_of(&from), Some(target.account.clone())].into_iter().flatten() {
        nudge(shared, &account);
    }
    tell(qt, shared, tr().text("contact-moved", None));
    show_pim(qt, shared);
    serde_json::json!({ "key": to.display().to_string() }).to_string()
}

/// Deletes a contact, after ten seconds to undo.
pub(crate) fn delete_contact(qt: &QtThread, shared: &Arc<Shared>, key: &str) {
    let Some((path, account)) = ours(key).and_then(|p| Some((p.clone(), account_of(&p)?))) else { return };
    mail::schedule_removal(qt, shared, &account, path, tr().text("undo-contact-deleted", None));
    show_pim(qt, shared);
}

#[derive(serde::Serialize)]
struct CalendarChoice {
    /// "account/id", to say where a new event goes.
    id: String,
    name: String,
    color: Option<String>,
}

/// The calendars an event can go into.
pub(crate) fn calendars() -> String {
    let config = load_config();
    let mut writable: Vec<Collection> = vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only).collect();
    // The one made for time blocks last: a new event goes elsewhere first (docs/tasks.md, "Pinned to a time").
    writable.sort_by_key(|c| c.id == sioul_core::blocks::CALENDAR);
    let choices: Vec<CalendarChoice> = writable.into_iter().map(|c| CalendarChoice { id: format!("{}/{}", c.account, c.id), name: c.label(&config, tr()), color: c.color }).collect();
    json(&choices)
}

#[derive(serde::Serialize)]
struct EventView {
    edit: EventEdit,
    calendar: String,
    read_only: bool,
}

/// Two events at once in the next `days` days, their margins counted, those
/// set aside left out, as JSON: [{key, day, today, first, second}], each
/// event {key, title, from, to} ("09:30"), for the Porch and the agenda.
pub(crate) fn overlaps(days: i64, removed: &std::collections::BTreeSet<PathBuf>, skipped: &std::collections::BTreeSet<(PathBuf, i64)>) -> String {
    let now = Zoned::now();
    let zone = now.time_zone().clone();
    let midnight = now.date().to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second());
    let aside = sioul_core::overlaps::SetAside::load(&sioul_core::overlaps::SetAside::default_path());
    let hm = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    let day = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).date()).ok();
    let one = |e: &agenda::Occurrence| serde_json::json!({ "key": e.key, "title": e.summary, "from": hm(e.start), "to": hm(e.end) });
    let events: Vec<agenda::Occurrence> = agenda::occurrences(midnight, midnight + days.max(1) * 86_400)
        .into_iter()
        .filter(|o| !removed.contains(Path::new(&o.key)) && !skipped.contains(&(PathBuf::from(&o.key), o.start)))
        .collect();
    let rows: Vec<serde_json::Value> = sioul_core::overlaps::overlaps(&events)
        .iter()
        // Over already: nothing to do about it.
        .filter(|o| !aside.keys.contains(&o.key) && o.first.end.max(o.second.end) > now.timestamp().as_second())
        .map(|o| serde_json::json!({ "key": o.key, "day": day(o.second.start).map(|d| d.to_string()).unwrap_or_default(), "today": day(o.second.start) == Some(now.date()), "first": one(&o.first), "second": one(&o.second) }))
        .collect();
    json(&rows)
}

/// An overlap set aside for good; returns what went wrong, else "".
pub(crate) fn set_overlap_aside(key: &str) -> String {
    let path = sioul_core::overlaps::SetAside::default_path();
    let mut aside = sioul_core::overlaps::SetAside::load(&path);
    aside.keys.insert(key.to_string());
    aside.save(&path).err().unwrap_or_default()
}

/// An event as its form shows it.
pub(crate) fn event(key: &str) -> String {
    let Some(path) = ours(key) else { return String::new() };
    let Some(calendar) = collection_of(&path) else { return String::new() };
    agenda::edit_of(&path)
        .map(|edit| json(&EventView { edit, calendar: format!("{}/{}", calendar.account, calendar.id), read_only: calendar.read_only }))
        .unwrap_or_default()
}

/// Saves the event form: into its file, or a new one in `calendar` ("account/id"); then sent.
pub(crate) fn save_event(qt: &QtThread, shared: &Arc<Shared>, key: &str, edit: &str, calendar: &str) -> Result<(), String> {
    // What a new event is made from: a message, a task (`links`).
    let links: Vec<String> = serde_json::from_str::<serde_json::Value>(edit)
        .ok()
        .and_then(|v| v.get("links").and_then(|l| l.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()))
        .unwrap_or_default();
    let edit: EventEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
    if edit.title.trim().is_empty() {
        return Err(tr().text("event-no-title", None));
    }
    let zone = TimeZone::system();
    // A task's time block renamed here: its task takes the title too, as both are kept in step (docs/tasks.md, "Pinned to a time").
    let mut renamed: Option<String> = None;
    let (path, text) = match ours(key) {
        Some(path) => {
            let current = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let task = sioul_core::lines::unfold(&current).iter().find(|l| sioul_core::lines::name(l) == sioul_core::blocks::TASK).map(|l| sioul_core::lines::value(l).trim().to_string());
            if let Some(task) = task.filter(|t| !t.is_empty())
                && agenda::edit_of_text(&current, &zone).is_some_and(|before| before.title.trim() != edit.title.trim())
            {
                renamed = Some(task);
            }
            (path, agenda::apply(&current, &edit, &zone)?)
        }
        None => {
            let target = vdir::collections(Kind::Calendars)
                .into_iter()
                .find(|c| !c.read_only && format!("{}/{}", c.account, c.id) == calendar)
                .or_else(agenda::default_calendar)
                .ok_or_else(|| tr().text("dav-no-calendar", None))?;
            let text = agenda::new_event(&edit, &zone)?;
            let lines: Vec<String> = links
                .iter()
                .map(|uri| {
                    let rel = if uri.starts_with("mid:") { "via" } else { "related" };
                    sioul_core::tasks::link_line(&sioul_core::tasks::Link { uri: uri.clone(), label: String::new(), rel: rel.into() })
                })
                .collect();
            (agenda::new_path(&target), agenda::add_lines(&text, &lines).unwrap_or(text))
        }
    };
    vdir::write_item(&path, &text)?;
    if let Some(account) = account_of(&path) {
        nudge(shared, &account);
    }
    if let Some(task) = renamed {
        let _ = crate::work::retitle(qt, shared, &task, edit.title.trim());
    }
    tell(qt, shared, tr().text("event-saved", None));
    show_pim(qt, shared);
    crate::work::show_work(qt, shared);
    Ok(())
}

/// An event moved by a drag in the agenda: its occurrence starting at `start`
/// now from `new_start` to `new_end` (Unix seconds), that time alone or every
/// time (`agenda::moved`); then sent, the agenda and the plan made again, and
/// "Undo" offered for ten seconds, which writes back what the file held, unless
/// it changed since (a sync, another device). Returns what went wrong, else "".
pub(crate) fn move_event(qt: &QtThread, shared: &Arc<Shared>, key: &str, start: i64, new_start: i64, new_end: i64, only_this: bool) -> String {
    let Some(path) = ours(key) else { return tr().text("agenda-gone", None) };
    if collection_of(&path).is_none_or(|c| c.read_only) {
        return tr().text("agenda-read-only", None);
    }
    let Ok(before) = std::fs::read_to_string(&path) else { return tr().text("agenda-gone", None) };
    let zone = TimeZone::system();
    let text = match agenda::moved(&before, start, new_start, new_end, only_this, &zone) {
        Ok(text) => text,
        Err(agenda::MoveProblem::Gone) => return tr().text("agenda-gone", None),
        Err(agenda::MoveProblem::SetDays) => return tr().text("agenda-move-set-days", None),
        Err(agenda::MoveProblem::Unreadable(e)) => return e,
    };
    if text == before {
        return String::new();
    }
    if let Err(e) = vdir::write_item(&path, &text) {
        return e;
    }
    let account = account_of(&path);
    if let Some(account) = &account {
        nudge(shared, account);
    }
    // Said as it is now: its title, its day, its times.
    let at = |seconds: i64| Timestamp::from_second(seconds).map(|t| t.to_zoned(zone.clone())).ok();
    let time = match (at(new_start), at(new_end)) {
        (Some(from), Some(to)) => format!("{}, {}–{}", tr().day(from.date()), from.strftime("%H:%M"), to.strftime("%H:%M")),
        _ => String::new(),
    };
    let title = agenda::edit_of_text(&text, &zone).map(|e| e.title).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| tr().text("agenda-untitled", None));
    let line = say("drag-done", &[("what", title), ("time", time)]);
    mail::offer_back(qt, shared, line, move |qt, shared| {
        // Only what this drag wrote is put back: a change since stays.
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            return tr().text("undo-too-late", None);
        }
        if let Err(e) = vdir::write_item(&path, &before) {
            return e;
        }
        if let Some(account) = &account {
            nudge(shared, account);
        }
        show_pim(qt, shared);
        crate::work::show_work(qt, shared);
        String::new()
    });
    show_pim(qt, shared);
    crate::work::show_work(qt, shared);
    String::new()
}

/// Deletes an event, or only this occurrence of a repeating one, after ten seconds to undo.
pub(crate) fn delete_event(qt: &QtThread, shared: &Arc<Shared>, key: &str, start: i64, only_this: bool) {
    let Some((path, account)) = ours(key).and_then(|p| Some((p.clone(), account_of(&p)?))) else { return };
    if only_this {
        mail::schedule_skip(qt, shared, &account, path, start, tr().text("undo-occurrence-skipped", None));
    } else {
        mail::schedule_removal(qt, shared, &account, path, tr().text("undo-event-deleted", None));
    }
    show_pim(qt, shared);
}

/// Addresses to complete what is typed in an address field, as JSON.
pub(crate) fn completions(typed: &str) -> String {
    // Only the part after the last comma is being typed.
    let current = typed.rsplit([',', ';']).next().unwrap_or(typed);
    json(&contacts::completions(&contacts::all(), current, 6))
}

/// The contact with this address, by file; empty when there is none.
pub(crate) fn contact_for(address: &str) -> String {
    contacts::by_address(&contacts::all(), address).map(|c| c.key.clone()).unwrap_or_default()
}

/// A sender becomes a contact, in the first address book.
pub(crate) fn add_sender(qt: &QtThread, shared: &Arc<Shared>, name: &str, address: &str) -> Result<String, String> {
    let edit = ContactEdit {
        name: if name.trim().is_empty() { address.to_string() } else { name.trim().to_string() },
        emails: vec![contacts::Labeled { label: String::new(), value: address.trim().to_string() }],
        ..ContactEdit::default()
    };
    save_contact(qt, shared, "", &serde_json::to_string(&serde_json::json!({ "name": edit.name, "emails": edit.emails })).unwrap_or_default())
}

#[derive(serde::Serialize)]
struct InvitationView {
    /// "REQUEST": answer; "CANCEL": take it out; "PUBLISH": add.
    method: String,
    summary: String,
    /// "Thursday 8 October, 10:00 – 11:00".
    when: String,
    location: String,
    organizer: String,
    /// Already in one of your calendars.
    known: bool,
}

/// The invitation a message carries, as the reader shows it; empty when it carries none.
pub(crate) fn invitation(key: &str) -> String {
    let Some(path) = maildir::locate(Path::new(key)) else { return String::new() };
    let Some(text) = reading::calendar(&path) else { return String::new() };
    let zone = TimeZone::system();
    let Some(asked) = agenda::invitation(&text, &zone) else { return String::new() };
    let at = |t: i64| Timestamp::from_second(t).map(|t| t.to_zoned(zone.clone())).ok();
    let when = match (at(asked.start), at(asked.end)) {
        (Some(start), _) if asked.all_day => tr().day(start.date()),
        (Some(start), Some(end)) => format!("{}, {} – {}", tr().day(start.date()), start.strftime("%H:%M"), end.strftime("%H:%M")),
        _ => String::new(),
    };
    json(&InvitationView { method: asked.method, summary: asked.summary, when, location: asked.location, organizer: asked.organizer, known: find_uid(&asked.uid).is_some() })
}

/// The event file with this UID, in any calendar.
fn find_uid(uid: &str) -> Option<PathBuf> {
    if uid.is_empty() {
        return None;
    }
    vdir::collections(Kind::Calendars).into_iter().flat_map(|c| c.items()).find(|path| {
        std::fs::read_to_string(path).ok().is_some_and(|text| sioul_core::lines::unfold(&text).iter().any(|l| sioul_core::lines::name(l) == "UID" && sioul_core::lines::value(l).trim() == uid))
    })
}

/// Answers an invitation: "accepted", "tentative" or "declined". Accepted or
/// maybe, the event goes into your first calendar; then your answer goes to
/// the organizer, from the account the invitation came to. "add" only adds it
/// (an event published, not asked); "remove" takes a cancelled one out.
pub(crate) fn answer(qt: &QtThread, shared: &Arc<Shared>, key: &str, answer: &str) -> Result<(), String> {
    let path = maildir::locate(Path::new(key)).ok_or_else(|| tr().text("mail-message-gone", None))?;
    let text = reading::calendar(&path).ok_or_else(|| tr().text("agenda-gone", None))?;
    let zone = TimeZone::system();
    let asked = agenda::invitation(&text, &zone).ok_or_else(|| tr().text("agenda-gone", None))?;
    let known = find_uid(&asked.uid);
    match answer {
        "remove" => {
            if let Some(file) = known {
                let account = account_of(&file).unwrap_or_default();
                mail::schedule_removal(qt, shared, &account, file, tr().text("undo-event-deleted", None));
            }
            return Ok(());
        }
        "accepted" | "tentative" | "add" => {
            let calendar = agenda::default_calendar().ok_or_else(|| tr().text("dav-no-calendar", None))?;
            let target = known.clone().unwrap_or_else(|| agenda::new_path(&calendar));
            // Answered, your calendar keeps your answer on your own line: a device
            // answering it too can tell whether it said the same (`dav`, an item
            // the server holds already under its UID).
            let me = load_config().account_of(&path).and_then(|a| a.address.clone()).unwrap_or_default();
            let kept = if answer == "add" { agenda::stored(&text) } else { agenda::stored_answered(&text, &me, &answer.to_ascii_uppercase()) };
            vdir::write_item(&target, &kept)?;
            if let Some(account) = account_of(&target) {
                nudge(shared, &account);
            }
        }
        _ => {}
    }
    if answer != "add" {
        send_answer(&path, &text, &asked, answer)?;
    }
    tell(qt, shared, tr().text(&format!("invitation-{answer}"), None));
    show_pim(qt, shared);
    // Accepted or added, it takes its time in the plan at once, not after the sync.
    crate::work::show_work(qt, shared);
    Ok(())
}

/// The REPLY to the organizer, through the account the invitation came to.
fn send_answer(message: &Path, text: &str, asked: &agenda::Invitation, answer: &str) -> Result<(), String> {
    let config = load_config();
    let account = config.account_of(message).cloned().ok_or_else(|| tr().text("mail-no-account", None))?;
    let me = account.address.clone().unwrap_or_default();
    let partstat = answer.to_ascii_uppercase();
    let ics = agenda::reply(text, &me, &partstat).ok_or_else(|| tr().text("agenda-gone", None))?;
    let name = account.name.clone().unwrap_or_default();
    let sentence = say(&format!("invitation-sentence-{answer}"), &[("name", if name.is_empty() { me.clone() } else { name.clone() }), ("summary", asked.summary.clone())]);
    let subject = say(&format!("invitation-subject-{answer}"), &[("summary", asked.summary.clone())]);
    let outgoing = sioul_core::compose::invitation_answer((&name, &me), &asked.organizer_address, &subject, &sentence, &ics, Zoned::now().timestamp().as_second())?;
    let password = secret::password(&account).map_err(|e| e.sentence(tr(), &account.id))?;
    let account = with_smtp(&account)?;
    match sioul_sync::send::send(&account, &password, &outgoing) {
        Ok(()) | Err(SyncError::NotFiled(_)) => Ok(()),
        Err(e) => Err(e.sentence(tr(), &account.id)),
    }
}

/// The account with its sending server, found and kept when it had none.
pub(crate) fn with_smtp(account: &Account) -> Result<Account, String> {
    if account.smtp_host.is_some() {
        return Ok(account.clone());
    }
    let smtp = sioul_sync::discover_smtp(account.address.as_deref().unwrap_or(""), account.host.as_deref()).map_err(|e| e.sentence(tr(), &account.id))?;
    config::set_smtp(&config_path(), &account.id, &smtp.host, smtp.port, smtp.security)?;
    let mut account = account.clone();
    account.smtp_host = Some(smtp.host);
    account.smtp_port = Some(smtp.port);
    account.smtp_security = smtp.security;
    Ok(account)
}

/// Adds a Google account, or signs one in again (`again`: its id): Google's
/// page opens in your browser, Sioul waits for its answer here (Cancel stops
/// the wait), finds the calendars and contacts, and syncs them. On a thread.
pub(crate) fn add_google_account(qt: &QtThread, shared: &Arc<Shared>, address: String, client_id: String, client_secret: String, again: Option<String>) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    shared.google_stop.store(false, Ordering::Relaxed);
    std::thread::spawn(move || {
        let config = load_config();
        let address = address.trim().to_ascii_lowercase();
        // Signed in again: the client kept with its grant.
        let kept = again.as_ref().and_then(|_| google::grant(&address));
        // The key kept (signing in again), else the one given, else Sioul's own when the build has one.
        let (client_id, client_secret) = match (&kept, google::built_in()) {
            (Some(grant), _) if client_id.trim().is_empty() => (grant.client_id.clone(), grant.client_secret.clone()),
            (_, Some((id, secret))) if client_id.trim().is_empty() => (id.to_string(), secret.to_string()),
            _ => (client_id, client_secret),
        };
        let id = again.clone().unwrap_or_else(|| {
            let base = config::slug("google");
            // Those switched off keep their id too.
            std::iter::once(base.clone()).chain((2..).map(|n| format!("{base}-{n}"))).find(|id| config.every_account().all(|a| &a.id != id)).unwrap_or(base)
        });
        let outcome = google::begin(&client_id, &address)
            .and_then(|sign_in| {
                let url = sign_in.url.clone();
                let _ = qt.queue(move |mut sioul| sioul.as_mut().open_url(QString::from(&url)));
                google::finish(sign_in, &client_id, &client_secret, &address, &shared.google_stop)
            })
            .and_then(|_| dav::test_google(&address))
            .map_err(|e| google_sentence(&e, &id))
            .and_then(|homes: Homes| {
                if again.is_none() {
                    let mut account = Account::dav(&id, &address, "apidata.googleusercontent.com", None, Some(&address));
                    account.auth = Some("google".into());
                    config::add_dav_account(&config_path(), &account)?;
                }
                homes.save(&id).map_err(|e| e.sentence(tr(), &id))?;
                load_config().account(&id).cloned().ok_or_else(|| say("account-unknown", &[("id", id.clone())]))
            });
        let added = outcome.as_ref().ok().map(|a| say("dav-added", &[("id", a.id.clone())]));
        if let Ok(account) = outcome.as_ref() {
            stop_dav_watcher(&shared, &account.id);
            start_dav_watcher(&qt, &shared, account.clone());
        }
        let error = outcome.err();
        let _ = qt.queue(move |mut sioul| {
            sioul.as_mut().set_form_busy(false);
            match (added, error) {
                (Some(line), _) => {
                    sioul.as_mut().set_status(QString::from(&line));
                    sioul.as_mut().account_added();
                }
                (None, Some(e)) => sioul.as_mut().set_form_error(QString::from(&e)),
                (None, None) => {}
            }
        });
        show(&qt, &shared);
    });
}

/// What went wrong while signing in with Google, in your language (its mail's sign-in too, gmail.rs).
pub(crate) fn google_sentence(error: &SyncError, id: &str) -> String {
    match error {
        SyncError::Message(m) if m.starts_with("google-") => {
            let (key, other) = m.split_once(':').unwrap_or((m.as_str(), ""));
            say(key, &[("address", other.to_string())])
        }
        e => e.sentence(tr(), id),
    }
}

/// Adds a contacts-and-calendars account: finds where they are, tests the
/// password, keeps it in the keyring, and starts syncing. On a thread.
pub(crate) fn add_account(qt: &QtThread, shared: &Arc<Shared>, address: String, url: String, login: String, password: String) {
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let config = load_config();
        let address = address.trim().to_ascii_lowercase();
        let login = if login.trim().is_empty() { address.clone() } else { login.trim().to_string() };
        let mail_host = config.accounts.iter().find(|a| a.address.as_deref() == Some(address.as_str())).and_then(|a| a.host.clone());
        let given = Some(url.trim()).filter(|u| !u.is_empty());
        let label = address.rsplit('@').next().unwrap_or("dav").split('.').next().unwrap_or("dav");
        let base = config::slug(&format!("{label}-agenda"));
        let id = std::iter::once(base.clone()).chain((2..).map(|n| format!("{base}-{n}"))).find(|id| config.every_account().all(|a| &a.id != id)).unwrap_or(base);
        let outcome = dav::test(&address, &login, &password, given, mail_host.as_deref()).map_err(|e| e.sentence(tr(), &id)).and_then(|homes: Homes| {
            let found = homes.calendars.clone().or(homes.contacts.clone()).unwrap_or_default();
            let host = found.split_once("://").map_or(found.as_str(), |(_, r)| r).split(['/', ':']).next().unwrap_or("").to_string();
            let account = Account::dav(&id, &address, &host, Some(&found), Some(&login));
            secret::save(&account, &password).map_err(|e| e.sentence(tr(), &id))?;
            config::add_dav_account(&config_path(), &account)?;
            homes.save(&id).map_err(|e| e.sentence(tr(), &id))?;
            Ok(account)
        });
        let added = outcome.as_ref().ok().map(|a| say("dav-added", &[("id", a.id.clone())]));
        if let Ok(account) = outcome.as_ref() {
            start_dav_watcher(&qt, &shared, account.clone());
        }
        let error = outcome.err();
        let _ = qt.queue(move |mut sioul| {
            sioul.as_mut().set_form_busy(false);
            match (added, error) {
                (Some(line), _) => {
                    sioul.as_mut().set_status(QString::from(&line));
                    sioul.as_mut().account_added();
                }
                (None, Some(e)) => sioul.as_mut().set_form_error(QString::from(&e)),
                (None, None) => {}
            }
        });
        show(&qt, &shared);
    });
}
