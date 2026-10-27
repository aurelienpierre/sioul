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
        let occurrences: Vec<agenda::Occurrence> = agenda_items(from, to, &removed, &skipped, &crate::work::loaded(shared).tasks)
            .into_iter()
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

/// What every agenda shows from `from` to `to` (Unix seconds), the
/// window's, the phone's and the home card alike (21 October 2026): the
/// calendars' events, those whose deletion waits ("Undo") aside, a task's
/// time block marked as the task it is, with its list (`agenda::mark_tasks`).
pub(crate) fn agenda_items(from: i64, to: i64, removed: &std::collections::BTreeSet<PathBuf>, skipped: &std::collections::BTreeSet<(PathBuf, i64)>, tasks: &[sioul_core::tasks::Task]) -> Vec<agenda::Occurrence> {
    let mut items = agenda_read(from, to, removed, skipped);
    agenda::mark_tasks(&mut items, tasks);
    items
}

/// The calendars' events from `from` to `to`, those whose deletion waits
/// aside, before their tasks are known (`agenda_items`; the home card marks
/// them with the tasks of its plan).
pub(crate) fn agenda_read(from: i64, to: i64, removed: &std::collections::BTreeSet<PathBuf>, skipped: &std::collections::BTreeSet<(PathBuf, i64)>) -> Vec<agenda::Occurrence> {
    without_waiting(agenda::occurrences(from, to), removed, skipped)
}

/// Occurrences read, those whose deletion waits ("Undo": `mail::hidden_pim`) left out:
/// what every agenda reads (`agenda_read`), Health's week too.
pub(crate) fn without_waiting(occurrences: Vec<agenda::Occurrence>, removed: &std::collections::BTreeSet<PathBuf>, skipped: &std::collections::BTreeSet<(PathBuf, i64)>) -> Vec<agenda::Occurrence> {
    occurrences.into_iter().filter(|o| !removed.contains(Path::new(&o.key)) && !skipped.contains(&(PathBuf::from(&o.key), o.start))).collect()
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

/// Whether a calendar stays on this device alone (21 October 2026): this
/// device's own (`vdir::LOCAL`) while the sharing does not carry them (no
/// sharing set up, or its part "lists" off here). A server's calendar is on
/// every device that has its account.
pub(crate) fn here_only(calendar: &Collection) -> bool {
    calendar.account == vdir::LOCAL && !(crate::share::configured() && crate::share::shares("lists"))
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
    let config = load_config();
    let accounts: Vec<Account> = config.accounts.iter().filter(|a| a.is_dav()).cloned().collect();
    let count = accounts.len();
    // The switched-off marks, this device's own, made to follow the configuration, which travels.
    let ids = |list: &[Account]| list.iter().filter(|a| a.kind == sioul_core::config::AccountKind::Dav).map(|a| a.id.clone()).collect::<Vec<_>>();
    if vdir::follow_switches(&ids(&config.accounts), &ids(&config.accounts_off)) {
        eprintln!("calendars: an account switched on or off on another device: its calendars follow here");
        show_pim(qt, shared);
    }
    // The phone's log: whether this device has any to sync, never which (21 October 2026).
    let off = config.accounts_off.iter().filter(|a| a.is_dav()).count();
    let unread = config.every_account().filter(|a| a.kind == sioul_core::config::AccountKind::Dav && a.host.is_none()).count();
    eprintln!("{}", watchers_line(count, off, unread));
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
        // What this device has for it, in yes or no: its homes found here, its password kept here.
        let homes = !dav::Homes::load(&account.id).is_empty();
        let password = account.auth.as_deref() == Some("google") || sioul_sync::secret::password(&account).is_ok();
        eprintln!("calendars: an account's sync starts (its homes {} here, its password {} here)", if homes { "known" } else { "not known yet" }, if password { "kept" } else { "not kept" });
        // After a lasting error (a refused password) it waits for a nudge, never ends.
        dav::watch(&account, &control, DAV_PAUSE, |result| reported(&qt, &shared, &account, result));
        if let Ok(mut watchers) = shared.dav_watchers.lock()
            && watchers.get(&account.id).is_some_and(|c| Arc::ptr_eq(c, &control))
        {
            watchers.remove(&account.id);
        }
    });
}

/// The phone's log at the start of the syncs: how many contacts-and-calendars
/// accounts this device has, switched on and off, and those it cannot sync
/// (no server named); never which.
fn watchers_line(on: usize, off: usize, unread: usize) -> String {
    let mut line = format!("calendars: {on} contacts-and-calendars account(s) here, synced while the window is open");
    if off > 0 {
        line.push_str(&format!("; {off} switched off"));
    }
    if unread > 0 {
        line.push_str(&format!("; {unread} without a server, not synced"));
    }
    if on == 0 {
        line.push_str("; the calendars shown are this device's own and those your other devices share (none of a server's)");
    }
    line
}

/// The phone's log after each sync of an account (21 October 2026): counts
/// only, and what went wrong as a code; never a name, an address or a calendar's.
fn sync_line(result: &Result<dav::Report, SyncError>) -> String {
    match result {
        Ok(r) => format!(
            "calendars: {} found ({} read only, {} for tasks only), {} synced, {} new or renamed; address books: {} found, {} synced; first problem: {}",
            r.calendars_listed,
            r.calendars_read_only,
            r.calendars_without_events,
            r.calendars_synced,
            r.registered,
            r.listed - r.calendars_listed,
            r.collections.saturating_sub(r.calendars_synced),
            r.problems.first().map_or("none", SyncError::code)
        ),
        Err(e) => format!("calendars: the account's sync stopped: {}{}", e.code(), if e.is_lasting() { "; it waits for a change (a password given, Sync now) before trying again" } else { "; tried again at the next sync" }),
    }
}

/// An account's status after a sync that listed its collections: how many
/// were found and how many synced whole, at what time; with what went wrong
/// with some of them, the first (the phone's calendars of 21 October 2026:
/// a partial state is never silent).
fn status_of(report: &dav::Report, account: &str, time: &str) -> (String, bool) {
    let (found, synced) = (report.listed.to_string(), report.collections.to_string());
    match report.problems.first() {
        None => (say("dav-synced-counts", &[("time", time.to_string()), ("found", found), ("synced", synced)]), false),
        Some(e) => (say("dav-synced-partial", &[("found", found), ("synced", synced), ("problem", e.sentence(tr(), account))]), true),
    }
}

/// What a sync said: the account's status, conflicts in the status line, the pages again.
fn reported(qt: &QtThread, shared: &Arc<Shared>, account: &Account, result: Result<dav::Report, SyncError>) {
    eprintln!("{}", sync_line(&result));
    let (line, problem) = match &result {
        Ok(report) => status_of(report, &account.id, &Zoned::now().strftime("%H:%M").to_string()),
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
            // A sync of an account switched on: its calendars show here, whatever an old mark says.
            if vdir::follow_switches(std::slice::from_ref(&account.id), &[]) {
                eprintln!("calendars: an account's calendars were marked off here, though it is on: shown again");
            }
            // Collections new here, renamed or recoloured count too: the lists are read again.
            let changed = report.sent + report.received + report.removed > 0 || !report.conflicts.is_empty() || report.registered > 0;
            // Yours kept among the earlier versions, which the window lists and puts back, on a phone too.
            if !report.conflicts.is_empty() {
                tell(qt, shared, tr().text("dav-conflict", None));
            }
            // An invitation you had answered otherwise on another device: its answer kept, said once.
            for (summary, kept) in report.answered {
                tell(qt, shared, say("dav-answered-elsewhere", &[("summary", summary), ("answer", kept.to_ascii_lowercase())]));
            }
            // Its tasks and events rewritten once in the form a phone can send back, said once, calmly, with the count.
            if report.rewritten > 0 {
                tell(qt, shared, say("dav-rewritten", &[("count", report.rewritten.to_string())]));
            }
            for summary in &report.moved_out {
                tell(qt, shared, say("dav-moved-out", &[("summary", summary.clone())]));
            }
            changed || report.rewritten > 0 || !report.moved_out.is_empty()
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
    // Through the moves' journal: a step cut short is finished, never both, never neither (the check of ef8c0ca, low 27).
    if let Err(e) = vdir::move_item(&from, &to) {
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
    // Those that take events: a task list is the task views' (21 October 2026).
    let mut writable: Vec<Collection> = vdir::collections(Kind::Calendars).into_iter().filter(|c| !c.read_only && c.holds("VEVENT")).collect();
    // The one made for time blocks last: a new event goes elsewhere first (docs/tasks.md, "Pinned to a time");
    // the one chosen in Settings ▸ Agenda first.
    let default = agenda::default_calendar().map(|c| format!("{}/{}", c.account, c.id));
    writable.sort_by_key(|c| (default.as_deref() != Some(format!("{}/{}", c.account, c.id).as_str()), c.id == sioul_core::blocks::CALENDAR));
    let choices: Vec<CalendarChoice> = writable.into_iter().map(|c| CalendarChoice { id: format!("{}/{}", c.account, c.id), name: c.label(&config, tr()), color: c.color }).collect();
    json(&choices)
}

#[derive(serde::Serialize)]
struct EventView {
    edit: EventEdit,
    calendar: String,
    /// Its calendar's name and place, for the editor to say it.
    calendar_name: String,
    /// "Only on this computer", when its calendar stays here alone; else "".
    here_only: String,
    read_only: bool,
    /// A meeting with others (`with_others`): the editor offers none of its
    /// organizer's fields (`agenda::may_change`), only your own around it.
    with_others: bool,
    /// What stays as it is, said in the editor: a meeting with others' time,
    /// title, place, notes and calendar (`with_others`); else "".
    stays: String,
}

/// Two events at once in the next `days` days, or closer than a time you set
/// between them (`overlaps::overlaps`), those set aside left out, as JSON:
/// [{key, day, today, first, second, gap, needed, at_once, source}], each
/// event {key, title, from, to} ("09:30"), `gap` and `needed` in minutes,
/// `source` where that time is set, in words, for the Porch and the agenda.
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
    let between = load_config().agenda.between();
    let rows: Vec<serde_json::Value> = sioul_core::overlaps::overlaps(&events, between)
        .iter()
        // Over already: nothing to do about it.
        .filter(|o| !aside.keys.contains(&o.key) && o.first.end.max(o.second.end) > now.timestamp().as_second())
        .map(|o| serde_json::json!({ "key": o.key, "day": day(o.second.start).map(|d| d.to_string()).unwrap_or_default(), "today": day(o.second.start) == Some(now.date()), "first": one(&o.first), "second": one(&o.second), "gap": o.gap, "needed": o.needed, "at_once": o.at_once(), "source": if o.source.is_empty() { String::new() } else { tr().text(&format!("overlap-source-{}", o.source), None) } }))
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

/// An event as its form shows it: the calendar it belongs to, by name too,
/// and whether that calendar stays on this device alone.
pub(crate) fn event(key: &str) -> String {
    let Some(path) = ours(key) else { return String::new() };
    let Some(calendar) = collection_of(&path) else { return String::new() };
    let config = load_config();
    let place = if here_only(&calendar) { tr().text(if cfg!(target_os = "android") { "event-calendar-here-phone" } else { "event-calendar-here-computer" }, None) } else { String::new() };
    let others = std::fs::read_to_string(&path).is_ok_and(|t| with_others(&t));
    let stays = if others { tr().text("event-change-with-others", None) } else { String::new() };
    agenda::edit_of(&path)
        .map(|edit| json(&EventView { edit, calendar: format!("{}/{}", calendar.account, calendar.id), calendar_name: calendar.label(&config, tr()), here_only: place, read_only: calendar.read_only, with_others: others, stays }))
        .unwrap_or_default()
}

/// An event's file moved to another calendar (its editor, 21 October 2026),
/// of any account, this device's own to a server's above all, so that an
/// event filed there by mistake reaches your other devices: noted, written
/// there, taken out here (`vdir::move_item`, a step cut short finished at the
/// next look), never twice, never lost. The sync sends both: the new item
/// there, the deletion here. Its new path.
pub(crate) fn move_event_file(path: &Path, to: &Collection) -> Result<PathBuf, String> {
    if path.parent() == Some(to.dir.as_path()) {
        return Ok(path.to_path_buf());
    }
    // A meeting with others is never moved: its server would cancel it (or
    // decline it) for them from the old calendar, then invite them again
    // (the review of 22 October 2026, F2).
    if std::fs::read_to_string(path).is_ok_and(|t| with_others(&t)) {
        return Err(tr().text("event-move-with-others", None));
    }
    if to.read_only || !to.holds("VEVENT") {
        return Err(tr().text("dav-no-calendar", None));
    }
    let moved = agenda::new_path(to);
    vdir::move_item(path, &moved)?;
    Ok(moved)
}

/// A meeting with others (`agenda::with_others`): never moved, deleted, nor
/// changed here but for Sioul's own lines (`agenda::may_change`, which every
/// event's writer applies through `vdir`).
pub(crate) fn with_others(text: &str) -> bool {
    agenda::with_others(text)
}

/// The notes of an event with an item of their checklist ticked (`line`,
/// "toggle", "done", "open" or "skipped"), its DESCRIPTION alone written:
/// one character of it (`checklist::ticked`). Refused for a calendar that can
/// only be read, and for a meeting with others: its list is its
/// organizer's, and its server would send the change to every guest.
/// Returns what went wrong, else "".
pub(crate) fn check_event_item(qt: &QtThread, shared: &Arc<Shared>, key: &str, line: usize, state: &str) -> String {
    let checked = || -> Result<(), String> {
        let path = ours(key).ok_or_else(|| tr().text("agenda-gone", None))?;
        let read_only = vdir::collections(Kind::Calendars).into_iter().find(|c| path.parent() == Some(c.dir.as_path())).is_none_or(|c| c.read_only);
        if read_only {
            return Err(tr().text("agenda-read-only", None));
        }
        let current = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if with_others(&current) {
            return Err(tr().text("checklist-with-others", None));
        }
        let zone = TimeZone::system();
        let mut edit = agenda::edit_of_text(&current, &zone).ok_or_else(|| tr().text("agenda-gone", None))?;
        edit.notes = crate::work::ticked_notes(&edit.notes, line, state)?;
        let text = agenda::apply(&current, &edit, &zone)?;
        if text != current {
            vdir::write_item(&path, &text)?;
            if let Some(account) = account_of(&path) {
                nudge(shared, &account);
            }
            show_pim(qt, shared);
        }
        Ok(())
    };
    checked().err().unwrap_or_default()
}

/// Saves the event form: into its file, or a new one in `calendar` ("account/id"); then sent.
pub(crate) fn save_event(qt: &QtThread, shared: &Arc<Shared>, key: &str, edit: &str, calendar: &str) -> Result<(), String> {
    let saved = write_event(key, edit, calendar)?;
    for account in &saved.accounts {
        nudge(shared, account);
    }
    if let Some(task) = saved.renamed {
        let _ = crate::work::retitle(qt, shared, &task, &saved.title);
    }
    tell(qt, shared, tr().text("event-saved", None));
    show_pim(qt, shared);
    crate::work::show_work(qt, shared);
    Ok(())
}

/// What saving the event form wrote: its file at last, the accounts to tell,
/// the task of a time block renamed, and its title.
struct SavedEvent {
    // Where it is at last: what the tests look at.
    #[cfg_attr(not(test), allow(dead_code))]
    path: PathBuf,
    accounts: Vec<String>,
    renamed: Option<String>,
    title: String,
}

/// The event form written (`save_event`, off the window): into its file, or
/// a new one in `calendar`; an event already there moved to `calendar` when
/// another is chosen (`move_event_file`).
fn write_event(key: &str, edit: &str, calendar: &str) -> Result<SavedEvent, String> {
    // What a new event is made from: a message, a task (`links`).
    let links: Vec<String> = serde_json::from_str::<serde_json::Value>(edit)
        .ok()
        .and_then(|v| v.get("links").and_then(|l| l.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()))
        .unwrap_or_default();
    let mut edit: EventEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
    let zone = TimeZone::system();
    // A task's time block renamed here: its task takes the title too, as both are kept in step (docs/tasks.md, "Pinned to a time").
    let mut renamed: Option<String> = None;
    let (path, text) = match ours(key) {
        Some(path) => {
            let current = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let meeting = with_others(&current);
            // Another calendar chosen for a meeting with others: nothing written (F2).
            let elsewhere = vdir::collections(Kind::Calendars).into_iter().find(|c| format!("{}/{}", c.account, c.id) == calendar).is_some_and(|c| path.parent() != Some(c.dir.as_path()));
            if elsewhere && meeting {
                return Err(tr().text("event-move-with-others", None));
            }
            // A meeting with others: its organizer's fields as its file has them, the form's
            // taken for yours alone (the time kept around it, its costs and gain, your
            // reminder). Its read-only notes come back from the form changed (a no-break
            // space, a line separator), never as the organizer wrote them (review of 36002ea, must-fix 2).
            if meeting {
                let mut kept = agenda::edit_of_text(&current, &zone).ok_or_else(|| tr().text("agenda-gone", None))?;
                (kept.margins, kept.demands, kept.remind) = (edit.margins, edit.demands, edit.remind);
                edit = kept;
            } else if edit.title.trim().is_empty() {
                return Err(tr().text("event-no-title", None));
            }
            let task = sioul_core::lines::unfold(&current).iter().find(|l| sioul_core::lines::name(l) == sioul_core::blocks::TASK).map(|l| sioul_core::lines::value(l).trim().to_string());
            if let Some(task) = task.filter(|t| !t.is_empty())
                && agenda::edit_of_text(&current, &zone).is_some_and(|before| before.title.trim() != edit.title.trim())
            {
                renamed = Some(task);
            }
            (path, agenda::apply(&current, &edit, &zone)?)
        }
        None => {
            if edit.title.trim().is_empty() {
                return Err(tr().text("event-no-title", None));
            }
            let target = vdir::collections(Kind::Calendars)
                .into_iter()
                .find(|c| !c.read_only && c.holds("VEVENT") && format!("{}/{}", c.account, c.id) == calendar)
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
    let mut accounts: Vec<String> = account_of(&path).into_iter().collect();
    let mut at = path.clone();
    // Another calendar chosen for an event already there: moved, its account told too.
    if ours(key).is_some()
        && let Some(target) = vdir::collections(Kind::Calendars).into_iter().find(|c| format!("{}/{}", c.account, c.id) == calendar)
        && path.parent() != Some(target.dir.as_path())
        && collection_of(&path).is_some_and(|from| !from.read_only)
    {
        at = move_event_file(&path, &target)?;
        accounts.push(target.account.clone());
    }
    Ok(SavedEvent { path: at, accounts, renamed, title: edit.title.trim().to_string() })
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
    // Its organizer's time: never moved here (`agenda::may_change`; the window offers no drag of it).
    if with_others(&before) {
        return tr().text("event-change-with-others", None);
    }
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

/// Deletes an event, or only this occurrence of a repeating one, after ten
/// seconds to undo; never one of a calendar you only read, nor a meeting with
/// others (`agenda::may_change`, asked again when the ten seconds end).
/// Returns what went wrong, else "".
pub(crate) fn delete_event(qt: &QtThread, shared: &Arc<Shared>, key: &str, start: i64, only_this: bool) -> String {
    let Some((path, account)) = ours(key).and_then(|p| Some((p.clone(), account_of(&p)?))) else { return tr().text("agenda-gone", None) };
    let change = agenda::Change::Delete;
    if let Err(refusal) = agenda::guard(&path, change) {
        return tr().text(refusal.word(&change), None);
    }
    if only_this {
        mail::schedule_skip(qt, shared, &account, path, start, tr().text("undo-occurrence-skipped", None));
    } else {
        mail::schedule_removal(qt, shared, &account, path, tr().text("undo-event-deleted", None));
    }
    show_pim(qt, shared);
    String::new()
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
            // Your answer to its cancellation: the one path that takes a meeting with
            // others out (`mail`), as its organizer asks it alone, and only what it
            // cancels (review of 36002ea, can-wait 5).
            let Some(file) = known else { return Ok(()) };
            let there = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
            match agenda::cancellation(&there, &text, &agenda::own_addresses()) {
                agenda::Cancellation::Refused => return Err(tr().text("invitation-cancel-refused", None)),
                agenda::Cancellation::Whole => {
                    let account = account_of(&file).unwrap_or_default();
                    mail::schedule_cancelled_removal(qt, shared, &account, file, text.clone(), tr().text("undo-event-deleted", None));
                }
                agenda::Cancellation::Changed(changed) => {
                    if vdir::calendar_read_only(&file) == Some(true) {
                        return Err(tr().text("agenda-read-only", None));
                    }
                    vdir::store_item(&file, &changed)?;
                    if let Some(account) = account_of(&file) {
                        nudge(shared, &account);
                    }
                    show_pim(qt, shared);
                    crate::work::show_work(qt, shared);
                }
            }
            return Ok(());
        }
        "accepted" | "tentative" | "add" => {
            let calendar = agenda::default_calendar().ok_or_else(|| tr().text("dav-no-calendar", None))?;
            let mut older = false;
            let me = load_config().account_of(&path).and_then(|a| a.address.clone()).unwrap_or_default();
            // Your answer made before anything is written: one that cannot be made writes nothing (can-wait 4).
            let reply = if answer == "add" { None } else { Some(agenda::reply(&text, &me, &answer.to_ascii_uppercase()).ok_or_else(|| tr().text("agenda-gone", None))?) };
            // A copy in a calendar you only read is never written: its server keeps it, and your answer goes all the same.
            if known.as_deref().is_none_or(|file| vdir::calendar_read_only(file) != Some(true)) {
                // Answered, your calendar keeps your answer on your own line: a device
                // answering it too can tell whether it said the same (`dav`, an item
                // the server holds already under its UID). The one path that writes a
                // meeting with others (`vdir::store_item`): how a guest speaks to its
                // organizer. Into a copy there already, only what the message carries
                // (one time of a series, say), your own lines kept, nothing for an
                // older version (`agenda::answered_into`).
                let partstat = if answer == "add" { String::new() } else { answer.to_ascii_uppercase() };
                let written = match &known {
                    Some(file) => {
                        let there = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
                        match agenda::answered_into(&there, &text, &me, &partstat, &agenda::own_addresses()) {
                            agenda::Answered::Written(changed) => Some((file.clone(), changed)),
                            agenda::Answered::Same => None,
                            // Older than your copy (the organizer's SEQUENCE, then when its
                            // message was made): its times never written, your answer alone,
                            // and said so, never "it is in your calendar" (review of 36002ea's
                            // check, must-fix 6; the check of 1b6795e, M7, M8).
                            agenda::Answered::Older(answer_alone) => {
                                older = true;
                                answer_alone.map(|changed| (file.clone(), changed))
                            }
                            // Not from its organizer, or a meeting you organize: nothing written, no answer sent (its check, low 8).
                            agenda::Answered::Refused => return Err(tr().text("invitation-update-refused", None)),
                        }
                    }
                    None => Some((agenda::new_path(&calendar), if answer == "add" { agenda::stored(&text) } else { agenda::stored_answered(&text, &me, &partstat) })),
                };
                if let Some((target, kept)) = written {
                    vdir::store_item(&target, &kept)?;
                    if let Some(account) = account_of(&target) {
                        nudge(shared, &account);
                    }
                }
            }
            if let Some(ics) = reply {
                send_answer(&path, &ics, &asked, answer)?;
            }
            if older {
                tell(qt, shared, tr().text(if answer == "add" { "invitation-older-added" } else { "invitation-older" }, None));
                show_pim(qt, shared);
                return Ok(());
            }
        }
        _ => send_answer(&path, &agenda::reply(&text, &load_config().account_of(&path).and_then(|a| a.address.clone()).unwrap_or_default(), &answer.to_ascii_uppercase()).ok_or_else(|| tr().text("agenda-gone", None))?, &asked, answer)?,
    }
    tell(qt, shared, tr().text(&format!("invitation-{answer}"), None));
    show_pim(qt, shared);
    // Accepted or added, it takes its time in the plan at once, not after the sync.
    crate::work::show_work(qt, shared);
    Ok(())
}

/// The REPLY (`ics`, made by `agenda::reply`) to the organizer, through the account the invitation came to.
fn send_answer(message: &Path, ics: &str, asked: &agenda::Invitation, answer: &str) -> Result<(), String> {
    let config = load_config();
    let account = config.account_of(message).cloned().ok_or_else(|| tr().text("mail-no-account", None))?;
    let me = account.address.clone().unwrap_or_default();
    let name = account.name.clone().unwrap_or_default();
    let sentence = say(&format!("invitation-sentence-{answer}"), &[("name", if name.is_empty() { me.clone() } else { name.clone() }), ("summary", asked.summary.clone())]);
    let subject = say(&format!("invitation-subject-{answer}"), &[("summary", asked.summary.clone())]);
    let outgoing = sioul_core::compose::invitation_answer((&name, &me), &asked.organizer_address, &subject, &sentence, ics, Zoned::now().timestamp().as_second())?;
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
    shared.oauth_stop.store(false, Ordering::Relaxed);
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
                google::finish(sign_in, &client_id, &client_secret, &address, &shared.oauth_stop)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// An event moved to another calendar (21 October 2026): written there,
    /// taken out here, never twice, never lost; not into a read-only one. A
    /// calendar of this device's own, the sharing not carrying it, is said
    /// to stay here.
    #[test]
    fn an_event_moves_to_another_calendar_once() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let account = "moves-here";
        for (id, name) in [("from", "From"), ("to", "To"), ("shared", "Shared")] {
            vdir::prepare(Kind::Calendars, account, id, name, None).unwrap();
        }
        let path = vdir::state_path(account, Kind::Calendars, "shared");
        let mut state = vdir::State::load(&path);
        state.read_only = true;
        state.save(&path).unwrap();
        let find = |id: &str| vdir::collections(Kind::Calendars).into_iter().find(|c| c.account == account && c.id == id).unwrap();
        let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:move-1\r\nDTSTART:20261022T090000Z\r\nSUMMARY:Moved\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let event = find("from").dir.join("e.ics");
        std::fs::write(&event, text).unwrap();
        assert!(move_event_file(&event, &find("shared")).is_err(), "never into a read-only calendar");
        assert!(event.exists());
        let moved = move_event_file(&event, &find("to")).unwrap();
        assert!(!event.exists() && std::fs::read_to_string(&moved).unwrap() == text, "there, the same, and no longer here");
        assert_eq!(find("to").items().len() + find("from").items().len(), 1, "never twice");
        assert_eq!(move_event_file(&moved, &find("to")).unwrap(), moved, "already there: nothing done");
        vdir::prepare(Kind::Calendars, vdir::LOCAL, "moves-mine", "Mine", None).unwrap();
        let mine = vdir::collections(Kind::Calendars).into_iter().find(|c| c.id == "moves-mine").unwrap();
        assert!(here_only(&mine), "this device's own, no sharing set up in a test: here only");
        assert!(!here_only(&find("to")), "a server's: on every device with its account");
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(account));
        let _ = std::fs::remove_dir_all(&mine.dir);
    }

    /// A meeting with others is never moved to another calendar: its server
    /// would cancel it for them, then invite them again (the review of 22
    /// October 2026, F2). Nothing is written; the editor says why.
    #[test]
    fn a_meeting_with_others_stays_in_its_calendar() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let account = "with-others";
        let from = vdir::create(Kind::Calendars, account, "Personal", None, &["VEVENT"]).unwrap();
        let to = vdir::create(Kind::Calendars, vdir::LOCAL, "With others mine", None, &["VEVENT"]).unwrap();
        let path = from.dir.join("meeting.ics");
        let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:meeting-1\r\nDTSTART:20991023T120000Z\r\nDTEND:20991023T130000Z\r\nSUMMARY:Meeting\r\nORGANIZER:mailto:boss@example.org\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:me@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        std::fs::write(&path, text).unwrap();
        assert!(move_event_file(&path, &to).is_err());
        let form = serde_json::to_string(&agenda::edit_of(&path).unwrap()).unwrap();
        assert!(write_event(path.to_str().unwrap(), &form, &format!("{}/{}", to.account, to.id)).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "nothing written, nothing moved");
        assert!(to.items().is_empty());
        let shown: serde_json::Value = serde_json::from_str(&event(path.to_str().unwrap())).unwrap();
        assert!(!shown["stays"].as_str().unwrap_or_default().is_empty(), "the editor says why");
        assert_eq!(shown["with_others"], true, "and offers none of its organizer's fields");
        // In its own calendar: its own lines (the time kept before it) change from its
        // form, no SEQUENCE its server would send on; its organizer's never (`agenda::may_change`).
        let here = format!("{}/{}", from.account, from.id);
        let mut edit = agenda::edit_of(&path).unwrap();
        edit.margins.before = 15;
        assert!(write_event(path.to_str().unwrap(), &serde_json::to_string(&edit).unwrap(), &here).is_ok());
        let kept = std::fs::read_to_string(&path).unwrap();
        assert!(kept.contains(sioul_core::demands::BEFORE) && !kept.contains("SEQUENCE") && kept.contains("SUMMARY:Meeting"), "{kept}");
        // Its organizer's fields typed in its form anyway (they are not offered): never written (must-fix 2).
        let mut theirs = edit.clone();
        theirs.title = "Meeting, moved".into();
        theirs.location = "Room 2".into();
        (theirs.start, theirs.end) = ("2099-10-23T15:00".into(), "2099-10-23T16:00".into());
        assert!(write_event(path.to_str().unwrap(), &serde_json::to_string(&theirs).unwrap(), &here).is_ok());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), kept, "nothing of its organizer's written");
        // And the door refuses it whatever writes it (`agenda::may_change`).
        assert!(vdir::write_item(&path, &kept.replace("SUMMARY:Meeting", "SUMMARY:Meeting, moved")).is_err());
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(account));
        let _ = std::fs::remove_dir_all(&to.dir);
    }

    /// Must-fix 2 of the review of 36002ea: a meeting's own fields (the time
    /// kept before it) are saved from its form however the form gives the
    /// organizer's back (a notes field turns a no-break space and a line
    /// separator into plain ones), and a meeting with no title takes them
    /// too; its organizer's lines stay as they were, its SEQUENCE too.
    #[test]
    fn a_meeting_s_own_fields_save_whatever_its_notes_hold() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Meeting notes", None, &["VEVENT"]).unwrap();
        let here = format!("{}/{}", calendar.account, calendar.id);
        let theirs = |text: &str| text.lines().filter(|l| !l.starts_with("X-SIOUL-") && !l.starts_with("DTSTAMP") && !l.starts_with("LAST-MODIFIED")).map(str::to_string).collect::<Vec<_>>();
        for (name, extra) in [("spaces", "SUMMARY:Point d\u{2019}équipe\r\nDESCRIPTION:Ordre du jour\u{a0}: budget\u{2028}puis questions\r\n"), ("untitled", "")] {
            let path = calendar.dir.join(format!("{name}.ics"));
            let text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{name}-1\r\nDTSTART:20991023T120000Z\r\nDTEND:20991023T130000Z\r\n{extra}ORGANIZER:mailto:boss@example.org\r\nATTENDEE:mailto:me@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
            std::fs::write(&path, &text).unwrap();
            let mut edit = agenda::edit_of(&path).unwrap();
            // As Qt's text field gives the notes back.
            edit.notes = edit.notes.replace('\u{a0}', " ").replace('\u{2028}', "\n");
            edit.margins.before = 15;
            let saved = write_event(path.to_str().unwrap(), &serde_json::to_string(&edit).unwrap(), &here);
            assert!(saved.is_ok(), "{name}: {:?}", saved.err());
            let written = std::fs::read_to_string(&path).unwrap();
            assert!(written.contains(sioul_core::demands::BEFORE), "{name}: {written}");
            assert_eq!(theirs(&written), theirs(&text), "{name}: the organizer's lines as they were");
        }
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// No writer of the window writes an event of a calendar you only read:
    /// its form, its reminder (through the form), its checklist, its drag
    /// and its Delete (`agenda::may_change`, in `vdir`'s doors and asked first).
    #[test]
    fn a_calendar_you_only_read_is_never_written() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let account = "read-only-writers";
        vdir::prepare(Kind::Calendars, account, "theirs", "Theirs", None).unwrap();
        let state = vdir::state_path(account, Kind::Calendars, "theirs");
        let mut read_only = vdir::State::load(&state);
        read_only.read_only = true;
        read_only.save(&state).unwrap();
        let path = Kind::Calendars.root().join(account).join("theirs").join("e.ics");
        let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:theirs-1\r\nDTSTART:20991023T120000Z\r\nDTEND:20991023T130000Z\r\nSUMMARY:Theirs\r\nDESCRIPTION:- [ ] bring the form\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        std::fs::write(&path, text).unwrap();
        let mut edit = agenda::edit_of(&path).unwrap();
        edit.remind = "30".into();
        assert!(write_event(path.to_str().unwrap(), &serde_json::to_string(&edit).unwrap(), "").is_err(), "its form, its reminder");
        assert!(agenda::guard(&path, agenda::Change::Delete).is_err() && agenda::guard(&path, agenda::Change::Move).is_err(), "its Delete, its drag");
        let ticked = text.replace("- [ ] bring", "- [x] bring");
        assert!(vdir::write_item(&path, &ticked).is_err(), "its checklist");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(account));
    }

    /// The choosers by what a collection takes (21 October 2026): a new
    /// event's calendars take events, a task's lists take tasks (a calendar
    /// of events alone greyed, never offered); one taking both is in both and
    /// says so.
    #[test]
    fn each_chooser_offers_what_takes_its_kind() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let events = vdir::create(Kind::Calendars, vdir::LOCAL, "Kinds chooser events", None, &["VEVENT"]).unwrap();
        let both = vdir::create(Kind::Calendars, vdir::LOCAL, "Kinds chooser both", None, &["VEVENT", "VTODO"]).unwrap();
        let tasks = vdir::create(Kind::Calendars, vdir::LOCAL, "Kinds chooser tasks", None, &["VTODO"]).unwrap();
        let id = |c: &Collection| format!("{}/{}", c.account, c.id);
        let offered: Vec<serde_json::Value> = serde_json::from_str(&calendars()).unwrap();
        let offered_id = |c: &Collection| offered.iter().find(|o| o["id"] == id(c)).map(|o| o["name"].as_str().unwrap_or_default().to_string());
        assert!(offered_id(&events).is_some() && offered_id(&tasks).is_none());
        assert!(offered_id(&both).is_some_and(|name| name.contains("events and tasks") || name.contains("événements et tâches")), "{offered:?}");
        let lists = crate::work::writable_lists();
        let takes = |c: &Collection| lists.iter().find(|l| l.id == id(c)).map(|l| l.tasks);
        assert_eq!((takes(&tasks), takes(&both), takes(&events)), (Some(true), Some(true), Some(false)));
        for c in [events, both, tasks] {
            let _ = std::fs::remove_dir_all(&c.dir);
        }
    }

    /// An event in a server's "Personal":
    /// its form saved with a calendar of this device's own moves it there,
    /// and back, one file each time; saved with its own calendar, it stays.
    #[test]
    fn a_saved_event_goes_to_the_calendar_chosen() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let account = "saved-moves";
        let personal = vdir::create(Kind::Calendars, account, "Personal", None, &["VEVENT", "VTODO"]).unwrap();
        let mine = vdir::create(Kind::Calendars, vdir::LOCAL, "Saved moves mine", None, &["VEVENT"]).unwrap();
        let id = |c: &Collection| format!("{}/{}", c.account, c.id);
        let event = personal.dir.join("dentist.ics");
        std::fs::write(&event, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:saved-1\r\nDTSTART:20991023T120000Z\r\nDTEND:20991023T124000Z\r\nSUMMARY:Dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let form = serde_json::to_string(&agenda::edit_of(&event).unwrap()).unwrap();
        let stays = write_event(event.to_str().unwrap(), &form, &id(&personal)).unwrap();
        assert_eq!(stays.path, event, "its own calendar: it stays");
        let moved = write_event(event.to_str().unwrap(), &form, &id(&mine)).unwrap();
        assert!(moved.path.starts_with(&mine.dir) && !event.exists(), "moved there, gone from here");
        assert_eq!(moved.accounts, [account.to_string(), vdir::LOCAL.to_string()], "both accounts told");
        assert_eq!(personal.items().len() + mine.items().len(), 1, "never twice");
        let back = write_event(moved.path.to_str().unwrap(), &form, &id(&personal)).unwrap();
        assert!(back.path.starts_with(&personal.dir) && !moved.path.exists(), "and back");
        assert!(std::fs::read_to_string(&back.path).unwrap().contains("UID:saved-1"));
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(account));
        let _ = std::fs::remove_dir_all(&mine.dir);
    }

    /// The phone's calendars of 21 October 2026: an account's status says how
    /// many collections were found and how many synced, and what went wrong
    /// with some, never a partial state in silence.
    #[test]
    fn the_status_says_found_and_synced() {
        let whole = dav::Report { listed: 3, collections: 3, ..dav::Report::default() };
        let (line, problem) = status_of(&whole, "Murena", "09:30");
        assert!(!problem && line.contains("09:30") && line.contains('3'), "{line}");
        let partial = dav::Report { listed: 3, collections: 1, problems: vec![SyncError::Login("401".into())], ..dav::Report::default() };
        let (line, problem) = status_of(&partial, "Murena", "09:30");
        assert!(problem && line.contains('1') && line.contains('3') && line.len() > 30, "{line}");
        assert!(!line.contains("dav-") && !line.contains('{'), "{line}");
    }

    /// The phone's log: one line per sync, counts and a code, never a name
    /// nor an address; and one at the start, saying whether there is any
    /// account to sync at all.
    #[test]
    fn the_log_says_counts_and_codes_only() {
        let report = dav::Report { listed: 5, collections: 3, calendars_listed: 3, calendars_synced: 2, problems: vec![SyncError::Server("PROPFIND https://dav.example.org/cal/secret/: 500".into())], ..dav::Report::default() };
        let line = sync_line(&Ok(report));
        assert_eq!(line, "calendars: 3 found (0 read only, 0 for tasks only), 2 synced, 0 new or renamed; address books: 2 found, 1 synced; first problem: sync-error-server");
    }

    /// 21 October 2026: a calendar that holds no event yet is offered for a
    /// new event like the others, and listed in Settings; only a read-only
    /// one stays out of the chooser, listed there fixed, its row saying why.
    #[test]
    fn an_empty_calendar_is_offered_and_a_read_only_one_says_why() {
        // Writes Sioul's files where it reads them: never the person's own (review of 36002ea).
        sioul_core::config::stand_ins_only();
        let account = "offered-here";
        for (id, name) in [("full", "Full"), ("empty", "Empty"), ("shared", "Shared")] {
            vdir::prepare(Kind::Calendars, account, id, name, None).unwrap();
        }
        std::fs::write(Kind::Calendars.root().join(account).join("full").join("x.ics"), "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n").unwrap();
        let path = vdir::state_path(account, Kind::Calendars, "shared");
        let mut state = vdir::State::load(&path);
        state.read_only = true;
        state.save(&path).unwrap();
        let offered: Vec<String> = serde_json::from_str::<Vec<serde_json::Value>>(&calendars()).unwrap().iter().map(|c| c["id"].as_str().unwrap().to_string()).filter(|id| id.starts_with(account)).collect();
        assert!(offered.contains(&format!("{account}/empty")) && offered.contains(&format!("{account}/full")), "{offered:?}");
        assert!(!offered.contains(&format!("{account}/shared")), "read only: not offered");
        vdir::create(Kind::Calendars, account, "Errands", None, &["VTODO"]).unwrap();
        let offered: Vec<String> = serde_json::from_str::<Vec<serde_json::Value>>(&calendars()).unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect();
        assert!(!offered.iter().any(|n| n.starts_with("Errands")), "a task list is never offered for an event: {offered:?}");
        let rows = sioul_core::settings::for_view("agenda", &load_config(), tr(), &[], None);
        let named = rows.iter().find(|s| s.key == "collections" && matches!(&s.value, sioul_core::config::SettingValue::Named(n) if n.iter().any(|v| v.id.contains(account)))).map(|s| match &s.value {
            sioul_core::config::SettingValue::Named(n) => n.clone(),
            _ => Vec::new(),
        });
        let named = named.expect("the calendars' setting");
        let shared = named.iter().find(|v| v.id.ends_with(&format!("{account}/shared"))).expect("the read-only one listed");
        assert_eq!(shared.locked, tr().text("collection-read-only", None));
        assert!(named.iter().any(|v| v.id.ends_with(&format!("{account}/empty")) && v.locked.is_empty()));
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(account));
        let stopped = sync_line(&Err(SyncError::NoPassword));
        assert!(stopped.starts_with("calendars: the account's sync stopped: ") && stopped.contains("waits"), "{stopped}");
        assert!(!stopped.contains("example"));
        assert!(watchers_line(0, 0, 0).contains("0 contacts-and-calendars account(s) here") && watchers_line(0, 0, 0).contains("none of a server's"));
        assert!(watchers_line(1, 1, 0).contains("1 switched off"));
    }
}
