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

/// Computes the contacts on a thread and shows them; the agenda follows in
/// its own job (`agenda::show_agenda`), from the same change.
pub(crate) fn show_pim(qt: &QtThread, shared: &Arc<Shared>) {
    crate::agenda::show_agenda(qt, shared);
    let qt = qt.clone();
    crate::backend::coalesced(shared, |s| &s.pim_job, move |shared| {
        let generation = shared.pim_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let state = shared.pim.lock().map(|s| s.clone()).unwrap_or_default();
        let (removed, skipped) = mail::hidden_pim(shared);
        // The phone's home-screen card reads its month of events again (homecard.rs).
        crate::homecard::agenda_seen(&removed, &skipped);
        let everyone: Vec<contacts::Contact> = contacts::all().into_iter().filter(|c| !removed.contains(Path::new(&c.key))).collect();
        let contacts = json(&view::contacts(&everyone, &state.query, &state.category, tr()));
        let _ = qt.queue(move |mut sioul| {
            // Only the newest (see `backend::show`).
            if sioul.shared().pim_generation.load(Ordering::Relaxed) != generation || sioul.shared().pim_shown_generation.fetch_max(generation, Ordering::Relaxed) > generation {
                return;
            }
            sioul.as_mut().set_contacts(QString::from(&contacts));
        });
    });
}

/// The calendars' events from `from` to `to`, those whose deletion waits
/// aside, before their tasks are known (`agenda::occurrences_between`; the home card marks
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
        crate::work::show_work(qt, shared);
        show_pim(qt, shared);
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

/// Two events at once among `events` (the next two weeks', those whose
/// deletion waits left out), or closer than a time you set between them
/// (`overlaps::overlaps`), those set aside left out, as JSON:
/// [{key, day, today, first, second, gap, needed, at_once, source}], each
/// event {key, title, from, to} ("09:30"), `gap` and `needed` in minutes,
/// `source` where that time is set, in words, for the Porch and the agenda.
pub(crate) fn overlaps_of(events: &[agenda::Occurrence]) -> String {
    let now = Zoned::now();
    let zone = now.time_zone().clone();
    let aside = sioul_core::overlaps::SetAside::load(&sioul_core::overlaps::SetAside::default_path());
    let hm = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string()).unwrap_or_default();
    let day = |at: i64| Timestamp::from_second(at).map(|t| t.to_zoned(zone.clone()).date()).ok();
    // Each with its time: its card opens on that time of a series, not its next (review of 590eb18, later 6).
    let one = |e: &agenda::Occurrence| serde_json::json!({ "key": e.key, "start": e.start, "title": e.summary, "from": hm(e.start), "to": hm(e.end) });
    let between = load_config().agenda.between();
    let rows: Vec<serde_json::Value> = sioul_core::overlaps::overlaps(events, between)
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
    edit_with_own(&path)
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
    // The events as last read, with the legs made since, never a read of the stores here
    // (reviews of ab7daa4, later 3, and 4d7bab4, later 5).
    let events = ours(key).map(|path| events_known(shared, &path, &TimeZone::system())).unwrap_or_default();
    let saved = write_event_with(key, edit, calendar, &events)?;
    for account in &saved.accounts {
        nudge(shared, account);
    }
    if let Some(task) = saved.renamed {
        let _ = crate::work::retitle(qt, shared, &task, &saved.title);
    }
    // Its legs moved with it (part E2), said as a drag says them.
    let legs = if saved.legs.is_empty() { String::new() } else { format!(" {}", say("legs-moved-with", &[("legs", saved.legs.join(", "))])) };
    tell(qt, shared, format!("{}{legs}", tr().text("event-saved", None)));
    // The re-plan due before the agenda's job starts (`work::replan_asked`).
    crate::work::show_work(qt, shared);
    show_pim(qt, shared);
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
    /// The legs moved with it, as the line says them (part E2).
    legs: Vec<String>,
}

/// `write_event_with`, no leg known (the tests').
#[cfg(test)]
fn write_event(key: &str, edit: &str, calendar: &str) -> Result<SavedEvent, String> {
    write_event_with(key, edit, calendar, &[])
}

/// The event form written (`save_event`, off the window): into its file, or
/// a new one in `calendar`; an event already there moved to `calendar` when
/// another is chosen (`move_event_file`). Its times changed, the legs of an
/// appointment (among `events`) move with it in the same change, and a leg
/// moved by hand is set from its appointment (`anchors::gesture`).
fn write_event_with(key: &str, edit: &str, calendar: &str, events: &[sioul_core::agenda::EventRef]) -> Result<SavedEvent, String> {
    // What a new event is made from: a message, a task, a letter (`links`): each an
    // address, or an address with its label (`{"uri", "label"}`); and its projects
    // (`projects`, a letter's: re-check of 857c39e, §3.4).
    let given = serde_json::from_str::<serde_json::Value>(edit).ok();
    let projects: Vec<String> = given.as_ref().and_then(|v| v.get("projects")?.as_array().map(|a| a.iter().filter_map(|p| p.as_str().map(str::trim).filter(|p| !p.is_empty()).map(str::to_string)).collect())).unwrap_or_default();
    let links: Vec<(String, String)> = given
        .as_ref()
        .and_then(|v| {
            v.get("links").and_then(|l| l.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| match x {
                        serde_json::Value::String(uri) => Some((uri.clone(), String::new())),
                        serde_json::Value::Object(link) => Some((link.get("uri")?.as_str()?.to_string(), link.get("label").and_then(|l| l.as_str()).unwrap_or_default().to_string())),
                        _ => None,
                    })
                    .collect()
            })
        })
        .unwrap_or_default();
    let edit: EventEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
    let zone = TimeZone::system();
    // A task's time block renamed here: its task takes the title too, as both are kept in step (docs/tasks.md, "Pinned to a time").
    let mut renamed: Option<String> = None;
    let mut before: Option<String> = None;
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
            // Yours about it is kept in your own file, never in the meeting's (decision 18,
            // the unified agenda's part 4): nothing written into it, whose server tells every guest.
            if meeting {
                let uid = uid_of(&current).ok_or_else(|| tr().text("agenda-gone", None))?;
                let own = sioul_core::own_time::Own { before: edit.margins.before, after: edit.margins.after, demands: edit.demands, remind: edit.remind.clone() };
                sioul_core::own_time::set(&sioul_core::own_time::default_path(), &uid, Some(own))?;
                let title = agenda::edit_of_text(&current, &zone).map(|e| e.title).unwrap_or_default();
                return Ok(SavedEvent { path, accounts: Vec::new(), renamed: None, title, legs: Vec::new() });
            }
            if edit.title.trim().is_empty() {
                return Err(tr().text("event-no-title", None));
            }
            let task = sioul_core::lines::unfold(&current).iter().find(|l| sioul_core::lines::name(l) == sioul_core::blocks::TASK).map(|l| sioul_core::lines::value(l).trim().to_string());
            if let Some(task) = task.filter(|t| !t.is_empty())
                && agenda::edit_of_text(&current, &zone).is_some_and(|before| before.title.trim() != edit.title.trim())
            {
                renamed = Some(task);
            }
            let text = agenda::apply(&current, &edit, &zone)?;
            before = Some(current);
            (path, text)
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
                .map(|(uri, label)| {
                    let rel = if uri.starts_with("mid:") { "via" } else { "related" };
                    sioul_core::tasks::link_line(&sioul_core::tasks::Link { uri: uri.clone(), label: label.clone(), rel: rel.into() })
                })
                .chain(projects.iter().map(|project| sioul_core::tasks::project_line(project)))
                .collect();
            (agenda::new_path(&target), agenda::add_lines(&text, &lines).unwrap_or(text))
        }
    };
    let mut accounts: Vec<String> = account_of(&path).into_iter().collect();
    let mut legs = Vec::new();
    match &before {
        Some(before) => {
            let gesture = sioul_core::anchors::gesture(&path, before, &text, events, &zone);
            legs = gesture.legs.iter().map(|(_, t)| sioul_core::anchors::moved_name(t, tr())).collect();
            accounts.extend(gesture.legs.iter().filter_map(|(p, _)| account_of(p)));
            let mut written = vec![(path.clone(), gesture.own)];
            written.extend(gesture.legs);
            sioul_core::composed::Change { written, ..sioul_core::composed::Change::default() }.write()?;
        }
        None => vdir::write_item(&path, &text)?,
    }
    accounts.sort();
    accounts.dedup();
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
    Ok(SavedEvent { path: at, accounts, renamed, title: edit.title.trim().to_string(), legs })
}

/// An event moved by a drag in the agenda: its occurrence starting at `start`
/// now from `new_start` to `new_end` (Unix seconds), that time alone or every
/// time (`agenda::moved`); then sent, the agenda and the plan made again, and
/// "Undo" offered for ten seconds, which writes back what the file held, unless
/// it changed since (a sync, another device). Returns what went wrong, else "".
pub(crate) fn move_event(qt: &QtThread, shared: &Arc<Shared>, key: &str, start: i64, new_start: i64, new_end: i64, only_this: bool) -> String {
    let Moved { text, accounts, files, legs } = match move_written(shared, key, start, new_start, new_end, only_this) {
        Ok(Some(moved)) => moved,
        Ok(None) => return String::new(),
        Err(problem) => return problem,
    };
    let zone = TimeZone::system();
    // Said as it is now: its title, its day, its times.
    let at = |seconds: i64| Timestamp::from_second(seconds).map(|t| t.to_zoned(zone.clone())).ok();
    let time = match (at(new_start), at(new_end)) {
        (Some(from), Some(to)) => format!("{}, {}–{}", tr().day(from.date()), from.strftime("%H:%M"), to.strftime("%H:%M")),
        _ => String::new(),
    };
    let title = agenda::edit_of_text(&text, &zone).map(|e| e.title).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| tr().text("agenda-untitled", None));
    let mut line = say("drag-done", &[("what", title), ("time", time)]);
    // Its legs moved with it, under the same Undo (part E2): "Moved with it: Bus 24 (timetable not checked)".
    if !legs.is_empty() {
        line = format!("{line} {}", say("legs-moved-with", &[("legs", legs.join(", "))]));
    }
    let said = sioul_core::agentwrites::Said { at: 0, line: String::new(), files };
    mail::offer_back(qt, shared, line, move |qt, shared| {
        // Only what this drag wrote is put back, every file or none: a change since stays.
        if !said.put_back() {
            return tr().text("undo-too-late", None);
        }
        for account in &accounts {
            nudge(shared, account);
        }
        crate::work::show_work(qt, shared);
        show_pim(qt, shared);
        String::new()
    });
    crate::work::show_work(qt, shared);
    show_pim(qt, shared);
    String::new()
}

/// An event's file as a drag moved it (`move_event`), written and sent: its
/// text, the accounts told, every file the gesture wrote (the event and its
/// legs, for one Undo), and the legs' names as the line says them.
pub(crate) struct Moved {
    text: String,
    accounts: Vec<String>,
    files: Vec<sioul_core::agentwrites::FileChange>,
    legs: Vec<String>,
}

/// What a gesture on an event's file writes beside it (`anchors::gesture`):
/// the legs of an appointment moved with it, a leg moved by hand set from its
/// appointment; through the writers' one rule (`Change::write`, taking back
/// what it wrote when a file fails), every file once. The files written, and
/// the legs' names as the line says them.
pub(crate) fn written_with_legs(shared: &Shared, path: &Path, before: &str, text: &str, zone: &TimeZone) -> Result<(Vec<sioul_core::agentwrites::FileChange>, Vec<String>), String> {
    // The events as last read (`loaded_known`) with the legs made since in the calendars legs
    // go to (`events_known`), never a read of the stores on the window's thread (reviews of
    // ab7daa4, later 3, and 4d7bab4, later 5). What a drag reads: the file moved, the files
    // changed since the last read in those calendars, and its legs' files.
    let events = events_known(shared, path, zone);
    let gesture = sioul_core::anchors::gesture(path, before, text, &events, zone);
    let legs: Vec<String> = gesture.legs.iter().map(|(_, t)| sioul_core::anchors::moved_name(t, tr())).collect();
    let mut written = vec![(path.to_path_buf(), gesture.own)];
    written.extend(gesture.legs);
    let files = sioul_core::composed::Change { written, ..sioul_core::composed::Change::default() }.write()?;
    Ok((files, legs))
}

/// The write of `move_event`, without the window: its checks, the file
/// written, its account told, and its re-plan due at once, before any job
/// starts (`work::replan_asked`). None when nothing changes.
pub(crate) fn move_written(shared: &Shared, key: &str, start: i64, new_start: i64, new_end: i64, only_this: bool) -> Result<Option<Moved>, String> {
    let Some(path) = ours(key) else { return Err(tr().text("agenda-gone", None)) };
    if collection_of(&path).is_none_or(|c| c.read_only) {
        return Err(tr().text("agenda-read-only", None));
    }
    let Ok(before) = std::fs::read_to_string(&path) else { return Err(tr().text("agenda-gone", None)) };
    // Its organizer's time: never moved here (`agenda::may_change`; the window offers no drag of it).
    if with_others(&before) {
        return Err(tr().text("event-change-with-others", None));
    }
    let zone = TimeZone::system();
    let text = match agenda::moved(&before, start, new_start, new_end, only_this, &zone) {
        Ok(text) => text,
        Err(agenda::MoveProblem::Gone) => return Err(tr().text("agenda-gone", None)),
        Err(agenda::MoveProblem::SetDays) => return Err(tr().text("agenda-move-set-days", None)),
        Err(agenda::MoveProblem::Unreadable(e)) => return Err(e),
    };
    if text == before {
        return Ok(None);
    }
    let (files, legs) = written_with_legs(shared, &path, &before, &text, &zone)?;
    crate::work::replan_asked(shared);
    let accounts: Vec<String> = files.iter().filter_map(|f| account_of(&f.path)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    for account in &accounts {
        nudge(shared, account);
    }
    Ok(Some(Moved { text, accounts, files, legs }))
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
        // Its legs to come go with it, under the same Undo (part E2); those over stay.
        let legs = legs_to_come(shared, &path, &TimeZone::system());
        if legs.is_empty() {
            mail::schedule_removal(qt, shared, &account, path, tr().text("undo-event-deleted", None));
        } else {
            let line = tr().text("legs-deleted-with", Some(&tr().counted(legs.len())));
            let files = std::iter::once((account.clone(), path)).chain(legs.into_iter().map(|leg| (account_of(&leg).unwrap_or_default(), leg))).collect();
            mail::schedule_removals(qt, shared, files, None, line);
        }
    }
    show_pim(qt, shared);
    String::new()
}

/// The events as last read (`work::loaded_known`), with the legs made or
/// changed since in the calendars a leg of the appointment in `path` goes to
/// (its own, and the one `leg_calendar` gives): only the files changed since
/// that read are read, every file of those two before any read. Never a read
/// of the stores on the window's thread; a leg made seconds before a delete
/// or a drag goes with its appointment (review of 4d7bab4, later 5).
fn events_known(shared: &Shared, path: &Path, zone: &TimeZone) -> Vec<sioul_core::agenda::EventRef> {
    let known = crate::work::loaded_known(shared);
    let mut events: Vec<sioul_core::agenda::EventRef> = known.as_ref().map(|l| l.events.clone()).unwrap_or_default();
    // A margin for file systems that keep coarse times.
    let since = known.as_ref().and_then(|l| l.read_at).and_then(|t| t.checked_sub(Duration::from_secs(2))).unwrap_or(std::time::UNIX_EPOCH);
    let own = collection_of(path);
    let mut dirs: Vec<PathBuf> = own.iter().map(|c| c.dir.clone()).chain(own.as_ref().and_then(leg_calendar).map(|c| c.dir)).collect();
    dirs.dedup();
    for dir in dirs {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let file = entry.path();
            let newer = entry.metadata().and_then(|m| m.modified()).is_ok_and(|m| m >= since);
            if !newer || file.extension().is_none_or(|e| e != "ics") {
                continue;
            }
            let key = file.display().to_string();
            let Some(found) = std::fs::read_to_string(&file).ok().and_then(|t| agenda::event_ref(&t, &key, zone)).filter(|r| r.anchor.is_some()) else { continue };
            events.retain(|e| e.key != key);
            events.push(found);
        }
    }
    events
}

/// The legs to come of the appointment in `path` (`anchors::to_come`), from
/// what `events_known` holds; none for a leg or a file that does not read.
fn legs_to_come(shared: &Shared, path: &Path, zone: &TimeZone) -> Vec<PathBuf> {
    let uid = std::fs::read_to_string(path).ok().and_then(|t| agenda::event_ref(&t, &path.display().to_string(), zone)).filter(|r| r.anchor.is_none()).map(|r| r.uid).unwrap_or_default();
    if uid.is_empty() {
        return Vec::new();
    }
    sioul_core::anchors::to_come(&uid, &events_known(shared, path, zone), Timestamp::now().as_second(), zone)
}

/// Whether a calendar is Google's: it may not keep a leg's lines (part E, D17's else).
fn google(calendar: &Collection) -> bool {
    sioul_core::capabilities::provider_of_collection(load_config().account(&calendar.account), calendar) == sioul_core::capabilities::Provider::Google
}

/// The calendar a leg of the appointment in `appointment` (its file's
/// calendar) goes into: that one, when it can be written and is no Google
/// calendar; else the calendar for new events, never a Google one.
fn leg_calendar(appointment: &Collection) -> Option<Collection> {
    if !appointment.read_only && appointment.holds("VEVENT") && !google(appointment) {
        return Some(appointment.clone());
    }
    agenda::default_calendar().filter(|c| !google(c))
}

/// What a leg made says: before or after which appointment, and in which
/// calendar when the appointment's own could not take it (the calendar for
/// new events; review of ab7daa4, later 7).
fn leg_made_line(side: sioul_core::anchors::Side, leg: &str, appointment: &str, own: Option<&Collection>, calendar: &Collection) -> String {
    let word = if side == sioul_core::anchors::Side::Before { "leg-made" } else { "leg-made-after" };
    let line = say(word, &[("leg", leg.to_string()), ("appointment", appointment.to_string())]);
    if own.is_none_or(|c| c.dir != calendar.dir) {
        return format!("{line} {}", say("leg-elsewhere", &[("calendar", calendar.label(&load_config(), tr()))]));
    }
    line
}

/// A leg added beside the appointment in `key` (part E5: "Add a ride or a
/// walk…"): the event form `edit`, on `side` ("before", "after"), from a
/// timetable or not; in the appointment's calendar and projects (the
/// calendar for new events when that one cannot hold it), said with Undo.
/// Returns what went wrong, else "".
pub(crate) fn save_leg(qt: &QtThread, shared: &Arc<Shared>, key: &str, edit: &str, side: &str, timetable: bool, step: &str) -> String {
    let made = || -> Result<(), String> {
        // As the sharing's job last found it, never the folder here (review of eb6f660, later 3).
        let offered = step.is_empty() || crate::share::legs_offered();
        let (change, line) = leg_change(shared, key, edit, side, timetable, step, offered)?;
        let files = change.write()?;
        crate::agents::say(qt, shared, sioul_core::agentwrites::Said { at: 0, line, files });
        Ok(())
    };
    match made() {
        Ok(()) => {
            crate::work::show_work(qt, shared);
            show_pim(qt, shared);
            String::new()
        }
        Err(e) => e,
    }
}

/// `save_leg`'s change, and what it says: the leg in the appointment's
/// calendar (else the one for new events), and, with `step` (its UID), that
/// step pinned to it in the same change (part 7: the leg is the step's time),
/// once every device keeps such a leg (`offered`, `share::legs_as_steps`).
fn leg_change(shared: &Shared, key: &str, edit: &str, side: &str, timetable: bool, step: &str, offered: bool) -> Result<(sioul_core::composed::Change, String), String> {
    let path = ours(key).ok_or_else(|| tr().text("leg-gone", None))?;
    let text = std::fs::read_to_string(&path).map_err(|_| tr().text("leg-gone", None))?;
    let zone = TimeZone::system();
    let appointment = sioul_core::anchors::Followed::of(&text, &path, &zone).ok_or_else(|| tr().text("leg-gone", None))?;
    let edit: EventEdit = serde_json::from_str(edit).map_err(|e| e.to_string())?;
    if edit.title.trim().is_empty() {
        return Err(tr().text("event-no-title", None));
    }
    let side = if side == "after" { sioul_core::anchors::Side::After } else { sioul_core::anchors::Side::Before };
    let projects = agenda::event_ref(&text, key, &zone).map(|r| r.projects).unwrap_or_default();
    let mut leg = sioul_core::anchors::new_leg(&edit, &appointment, &projects, side, timetable, &zone).map_err(|e| tr().text(&e, None))?;
    let own = collection_of(&path);
    let calendar = own.as_ref().and_then(leg_calendar).ok_or_else(|| tr().text("dav-no-calendar", None))?;
    let mut written = Vec::new();
    if !step.is_empty() {
        if !offered {
            return Err(tr().text("leg-step-not-yet", None));
        }
        let task = leg_steps_of(shared, &appointment.uid).into_iter().find(|t| t.uid == step).ok_or_else(|| tr().text("leg-step-gone", None))?;
        let step_text = std::fs::read_to_string(&task.key).map_err(|_| tr().text("leg-step-gone", None))?;
        let (as_time, pinned) = sioul_core::anchors::leg_for_step(&leg, &task, &step_text, &Zoned::now()).map_err(|e| tr().text(&e, None))?;
        leg = as_time;
        written.push((PathBuf::from(&task.key), pinned));
    }
    let change = sioul_core::composed::Change { made: vec![(agenda::new_path(&calendar), leg)], written, ..sioul_core::composed::Change::default() };
    let named = agenda::edit_of_text(&text, &zone).map(|e| e.title).unwrap_or_default();
    Ok((change, leg_made_line(side, edit.title.trim(), &named, own.as_ref(), &calendar)))
}

/// The steps a new leg of the appointment `uid` may be the time of (part
/// 7): its preparation's open steps that can be written and have no time
/// of their own (no time block, no start), from the tasks as last read.
fn leg_steps_of(shared: &Shared, uid: &str) -> Vec<sioul_core::tasks::Task> {
    let Some(known) = crate::work::loaded_known(shared) else { return Vec::new() };
    let reference = sioul_core::agenda::EventRef { uid: uid.to_string(), ..sioul_core::agenda::EventRef::default() };
    let Some(preparation) = sioul_core::pairs::preparation_of(&known.tasks, &reference).filter(|t| t.status.is_open()) else { return Vec::new() };
    sioul_core::tasks::steps_of(&known.tasks, &preparation.uid).into_iter().filter(|s| s.status.is_open() && !s.read_only && s.start.is_empty() && !s.links.iter().any(sioul_core::blocks::is_block_link)).cloned().collect()
}

/// What the leg form offers as its step (part 7): {"offered", "steps":
/// [{uid, title}], "why"}; offered only once every device keeps such a leg.
pub(crate) fn leg_steps(shared: &Shared, key: &str) -> String {
    let zone = TimeZone::system();
    let uid = ours(key).and_then(|p| std::fs::read_to_string(&p).ok().and_then(|t| agenda::event_ref(&t, key, &zone))).map(|r| r.uid).unwrap_or_default();
    let steps: Vec<serde_json::Value> = leg_steps_of(shared, &uid).iter().map(|t| serde_json::json!({ "uid": t.uid, "title": t.title })).collect();
    let offered = !steps.is_empty() && crate::share::legs_offered();
    let why = if steps.is_empty() || offered { String::new() } else { tr().text("leg-step-not-yet", None) };
    json(&serde_json::json!({ "offered": offered, "steps": if offered { steps } else { Vec::new() }, "why": why }))
}

/// The leg in `key` untied from its appointment (its card's ×): an event of
/// its own, its anchor and belonging gone, said with Undo. Returns what went
/// wrong, else "".
pub(crate) fn untie_leg(qt: &QtThread, shared: &Arc<Shared>, key: &str) -> String {
    let untied = || -> Result<(), String> {
        let path = ours(key).ok_or_else(|| tr().text("agenda-gone", None))?;
        let text = std::fs::read_to_string(&path).map_err(|_| tr().text("agenda-gone", None))?;
        let zone = TimeZone::system();
        let known = crate::work::loaded_known(shared);
        let appointment = sioul_core::anchors::parse(&text).and_then(|a| known.as_ref()?.events.iter().find(|e| e.uid == a.appointment).map(|e| e.summary.clone())).unwrap_or_default();
        let Some(alone) = sioul_core::anchors::untied(&text) else { return Ok(()) };
        let files = sioul_core::composed::Change { written: vec![(path.clone(), alone)], ..sioul_core::composed::Change::default() }.write()?;
        let _ = zone;
        crate::agents::say(qt, shared, sioul_core::agentwrites::Said { at: 0, line: say("leg-untied", &[("appointment", appointment)]), files });
        Ok(())
    };
    match untied() {
        Ok(()) => {
            crate::work::show_work(qt, shared);
            show_pim(qt, shared);
            String::new()
        }
        Err(e) => e,
    }
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

/// An event file's UID, its series' (the first one written).
fn uid_of(text: &str) -> Option<String> {
    sioul_core::lines::unfold(text).iter().find(|l| sioul_core::lines::name(l) == "UID").map(|l| sioul_core::lines::value(l).trim().to_string()).filter(|u| !u.is_empty())
}

/// The form's fields of an event, yours about a meeting with others read
/// from your own file when it has it (`own_time`), else from the meeting's
/// lines an older Sioul wrote: what the form and the reminder chooser start
/// from. An event that is no meeting reads its own lines, whatever your own
/// file holds of it (its guests taken out elsewhere; review of part 4, later 4).
pub(crate) fn edit_with_own(path: &Path) -> Option<EventEdit> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut edit = agenda::edit_of_text(&text, &TimeZone::system())?;
    if let Some(own) = Some(&text).filter(|text| with_others(text)).and_then(|text| uid_of(text)).and_then(|uid| sioul_core::own_time::current().events.get(&uid).cloned()) {
        (edit.margins, edit.demands, edit.remind) = (own.margins(), own.demands, own.remind);
    }
    Some(edit)
}

/// The message that brought a meeting's invitation, while it is in your
/// mail: its file, for the card's Answer… (the unified agenda, part 4); ""
/// when none is known. The newest of its organizer's messages whose calendar
/// names this event's UID, among those the last read knows: only those few
/// messages are opened here, never the stores.
pub(crate) fn invitation_of(shared: &Shared, key: &str) -> String {
    let Some(path) = ours(key) else { return String::new() };
    let Ok(text) = std::fs::read_to_string(&path) else { return String::new() };
    if !with_others(&text) {
        return String::new();
    }
    let Some(uid) = uid_of(&text) else { return String::new() };
    let organizer = sioul_core::lines::unfold(&text).iter().find(|l| sioul_core::lines::name(l) == "ORGANIZER").map(|l| sioul_core::lines::value(l).trim().trim_start_matches("mailto:").trim_start_matches("MAILTO:").to_ascii_lowercase()).unwrap_or_default();
    let Some(loaded) = crate::work::loaded_known(shared) else { return String::new() };
    let mut theirs: Vec<&sioul_core::mailindex::MailRef> = loaded.mail.iter().map(|(_, m)| m).filter(|m| !organizer.is_empty() && m.address == organizer).collect();
    theirs.sort_by_key(|m| std::cmp::Reverse(m.date));
    let zone = TimeZone::system();
    theirs
        .into_iter()
        .take(8)
        .find(|m| reading::calendar(&m.path).and_then(|ics| agenda::invitation(&ics, &zone)).is_some_and(|asked| asked.uid == uid))
        .map(|m| m.path.display().to_string())
        .unwrap_or_default()
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
                    crate::work::show_work(qt, shared);
                    show_pim(qt, shared);
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
    // Accepted or added, it takes its time in the plan at once, not after the
    // sync; its re-plan due before the agenda's job starts.
    crate::work::show_work(qt, shared);
    show_pim(qt, shared);
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

    /// An appointment and a bus 25 minutes before it, on 29 October 2099 (UTC).
    fn doctor_and_bus(name: &str) -> (Collection, PathBuf, PathBuf) {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, name, None, &["VEVENT"]).unwrap();
        let doctor = calendar.dir.join(format!("{name}-doctor.ics"));
        vdir::write_item(&doctor, &format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{name}-doctor\r\nDTSTAMP:20991020T100000Z\r\nDTSTART:20991029T130000Z\r\nDTEND:20991029T133000Z\r\nSUMMARY:Doctor\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")).unwrap();
        let bus = calendar.dir.join(format!("{name}-bus.ics"));
        vdir::write_item(&bus, &format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{name}-bus\r\nDTSTAMP:20991020T100000Z\r\nDTSTART:20991029T121000Z\r\nDTEND:20991029T123500Z\r\nSUMMARY:Bus 24\r\nRELATED-TO;RELTYPE=PARENT:{name}-doctor\r\nX-SIOUL-ANCHOR;GAP=PT25M;TIMETABLE=TRUE;WAS=20991029T130000Z:BEFORE\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")).unwrap();
        (calendar, doctor, bus)
    }

    /// A leg made in the calendar for new events, the appointment's own not
    /// taking it, says where it went; one made beside its appointment says
    /// nothing more (review of ab7daa4, later 7).
    #[test]
    fn a_leg_made_elsewhere_says_where() {
        let (own, _, _) = doctor_and_bus("legs-elsewhere");
        let (other, _, _) = doctor_and_bus("legs-elsewhere-new");
        let beside = leg_made_line(sioul_core::anchors::Side::Before, "Bus 24", "Doctor", Some(&own), &own);
        assert_eq!(beside, say("leg-made", &[("leg", "Bus 24".into()), ("appointment", "Doctor".into())]));
        let label = other.label(&load_config(), tr());
        let elsewhere = leg_made_line(sioul_core::anchors::Side::After, "Walk home", "Doctor", Some(&own), &other);
        assert!(elsewhere.starts_with(&say("leg-made-after", &[("leg", "Walk home".into()), ("appointment", "Doctor".into())])));
        assert!(elsewhere.ends_with(&say("leg-elsewhere", &[("calendar", label.clone())])), "said where: {elsewhere}");
        assert!(leg_made_line(sioul_core::anchors::Side::Before, "Bus 24", "Doctor", None, &other).contains(&label));
    }

    /// What the leg form offers is read from what the sharing's job last
    /// found, never from the sharing's folder on the window's thread (review
    /// of eb6f660, later 3).
    #[test]
    fn the_leg_form_reads_the_gate_s_answer_kept() {
        let (_calendar, doctor, _bus) = doctor_and_bus("legs-kept-answer");
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "legs-kept-answer tasks", None, &["VTODO"]).unwrap();
        let (zone, now) = (TimeZone::system(), Zoned::now());
        let tie = sioul_core::tasks::Link { uri: "sioul:event/legs-kept-answer-doctor".into(), label: String::new(), rel: sioul_core::pairs::APPOINTMENT.into() };
        vdir::write_item(&list.dir.join("legs-kept-answer-prep.ics"), &sioul_core::tasks::new_task(&sioul_core::tasks::TaskEdit { title: "Going".into(), links: vec![tie], ..Default::default() }, "legs-kept-answer-prep", &zone, &now).unwrap()).unwrap();
        vdir::write_item(&list.dir.join("legs-kept-answer-walk.ics"), &sioul_core::tasks::new_task(&sioul_core::tasks::TaskEdit { title: "Walk".into(), parent: "legs-kept-answer-prep".into(), ..Default::default() }, "legs-kept-answer-walk", &zone, &now).unwrap()).unwrap();
        let shared = Shared::default();
        let _ = crate::work::loaded(&shared);
        let reads = || crate::share::LEGS_READS.with(std::cell::Cell::get);
        // The answer the sharing's job kept: yes (put back as it was after).
        let kept = sioul_core::anchors::steps_offered();
        sioul_core::anchors::say_steps_offered(true);
        let before = reads();
        let offer: serde_json::Value = serde_json::from_str(&leg_steps(&shared, &doctor.display().to_string())).unwrap();
        let after = reads();
        sioul_core::anchors::say_steps_offered(kept);
        assert_eq!(after, before, "no read of the folder");
        assert_eq!(offer["offered"], true, "the kept answer: offered ({offer})");
        let _ = std::fs::remove_dir_all(&list.dir);
    }

    /// A new leg made a step's time (part 7): the step chosen pinned to it in
    /// the same change, the leg its block; refused until every device keeps
    /// such a leg; only a step of the appointment's preparation.
    #[test]
    fn a_new_leg_can_be_a_step_s_time() {
        let (_calendar, doctor, _bus) = doctor_and_bus("legs-step");
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "legs-step tasks", None, &["VTODO"]).unwrap();
        let zone = TimeZone::system();
        let now = Zoned::now();
        let tie = sioul_core::tasks::Link { uri: "sioul:event/legs-step-doctor".into(), label: String::new(), rel: sioul_core::pairs::APPOINTMENT.into() };
        let prep = sioul_core::tasks::new_task(&sioul_core::tasks::TaskEdit { title: "Going to the doctor".into(), links: vec![tie], ..Default::default() }, "legs-step-prep", &zone, &now).unwrap();
        vdir::write_item(&list.dir.join("legs-step-prep.ics"), &prep).unwrap();
        let walk = sioul_core::tasks::new_task(&sioul_core::tasks::TaskEdit { title: "Walk to the stop".into(), parent: "legs-step-prep".into(), ..Default::default() }, "legs-step-walk", &zone, &now).unwrap();
        vdir::write_item(&list.dir.join("legs-step-walk.ics"), &walk).unwrap();
        let shared = Shared::default();
        let _ = crate::work::loaded(&shared);
        let edit = serde_json::to_string(&EventEdit { title: "Walk".into(), start: "2099-10-29T12:00".into(), end: "2099-10-29T12:08".into(), ..EventEdit::default() }).unwrap();
        let key = doctor.display().to_string();
        assert_eq!(leg_change(&shared, &key, &edit, "before", false, "legs-step-walk", false).err(), Some(tr().text("leg-step-not-yet", None)), "not until every device keeps it");
        let (change, _) = leg_change(&shared, &key, &edit, "before", false, "legs-step-walk", true).unwrap();
        assert!(change.made[0].1.contains("X-SIOUL-TASK:legs-step-walk"), "the leg is the step's time");
        assert_eq!(change.written.len(), 1, "and the step, in the same change");
        let pinned = sioul_core::tasks::task_of_text(&change.written[0].1, &zone).unwrap();
        assert!(pinned.links.iter().any(sioul_core::blocks::is_block_link), "pinned to it");
        assert_eq!(leg_change(&shared, &key, &edit, "before", false, "nowhere", true).err(), Some(tr().text("leg-step-gone", None)), "a step of its preparation only");
        let _ = std::fs::remove_dir_all(&list.dir);
    }

    /// A leg made since the last read (seconds before a delete, or before any
    /// plan) goes with its appointment: the calendars legs go to are read for
    /// the files changed since, never the stores (review of 4d7bab4, later 5).
    #[test]
    fn a_leg_made_just_before_a_delete_goes_with_its_appointment() {
        let (calendar, doctor, bus) = doctor_and_bus("legs-late");
        let shared = Shared::default();
        let _ = crate::work::loaded(&shared);
        let walk = calendar.dir.join("legs-late-walk.ics");
        vdir::write_item(&walk, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:legs-late-walk\r\nDTSTAMP:20991020T100000Z\r\nDTSTART:20991029T133000Z\r\nDTEND:20991029T135000Z\r\nSUMMARY:Walk home\r\nRELATED-TO;RELTYPE=PARENT:legs-late-doctor\r\nX-SIOUL-ANCHOR;GAP=PT0M;WAS=20991029T133000Z:AFTER\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let legs = legs_to_come(&shared, &doctor, &TimeZone::system());
        assert!(legs.contains(&bus) && legs.contains(&walk), "the leg read before, and the one made since: {legs:?}");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// Part E2: a drag of the appointment moves its legs in the same change, the
    /// line names them with their doubt, and one Undo puts every file back; a leg
    /// dragged by hand keeps its time, its distance taken again; the form moves
    /// them too; a deletion takes the legs to come with it.
    #[test]
    fn a_drag_moves_the_legs_with_the_appointment_under_one_undo() {
        let (calendar, doctor, bus) = doctor_and_bus("legs-drag");
        let (doctor_before, bus_before) = (std::fs::read_to_string(&doctor).unwrap(), std::fs::read_to_string(&bus).unwrap());
        let at = |h: i8, m: i8| jiff::civil::date(2099, 10, 29).at(h, m, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp().as_second();
        // Nothing read yet: a drag reads no store on the window's thread (review of ab7daa4, later 3);
        // the calendar its legs go to is read alone, and its leg moves with it (review of 4d7bab4, later 5).
        let cold = Shared::default();
        assert!(move_written(&cold, &doctor.display().to_string(), at(13, 0), at(13, 5), at(13, 35), false).unwrap().is_some());
        assert!(cold.loaded.lock().unwrap().is_none(), "no read of the stores for a drag");
        assert_eq!(agenda::first_times(&std::fs::read_to_string(&bus).unwrap(), &TimeZone::system()), Some((at(12, 15), at(12, 40))), "its leg moved with it");
        std::fs::write(&doctor, &doctor_before).unwrap();
        std::fs::write(&bus, &bus_before).unwrap();
        let shared = Shared::default();
        let _ = crate::work::loaded(&shared);
        let moved = move_written(&shared, &doctor.display().to_string(), at(13, 0), at(14, 0), at(14, 30), false).unwrap().unwrap();
        let zone = TimeZone::system();
        assert_eq!(agenda::first_times(&std::fs::read_to_string(&bus).unwrap(), &zone), Some((at(13, 10), at(13, 35))), "the bus moved with it");
        let doubt = say("legs-unconfirmed", &[("leg", "Bus 24".to_string())]);
        assert_eq!(moved.legs, [doubt.clone()]);
        assert_eq!(moved.files.len(), 2, "one change, one Undo");
        assert!(sioul_core::agentwrites::Said { at: 0, line: String::new(), files: moved.files }.put_back());
        assert_eq!([std::fs::read_to_string(&doctor).unwrap(), std::fs::read_to_string(&bus).unwrap()], [doctor_before.clone(), bus_before.clone()], "every file back");
        // The bus dragged by hand ten minutes earlier: its time kept, its distance 35 minutes now.
        move_written(&shared, &bus.display().to_string(), at(12, 10), at(12, 0), at(12, 25), false).unwrap().unwrap();
        let anchor = sioul_core::anchors::parse(&std::fs::read_to_string(&bus).unwrap()).unwrap();
        assert_eq!((anchor.gap, anchor.unconfirmed), (35 * 60, None));
        // The form moves the appointment, and its leg with it.
        std::fs::write(&bus, &bus_before).unwrap();
        let events = crate::work::loaded(&shared).events.clone();
        let mut edit = agenda::edit_of(&doctor).unwrap();
        (edit.start, edit.end) = ("2099-10-30T09:00".into(), "2099-10-30T09:30".into());
        let form = |edit: &EventEdit| serde_json::to_string(edit).unwrap();
        let local = |h: i8, m: i8| jiff::civil::date(2099, 10, 30).at(h, m, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let saved = write_event_with(&doctor.display().to_string(), &form(&edit), &format!("{}/{}", calendar.account, calendar.id), &events).unwrap();
        assert_eq!(saved.legs, [doubt]);
        assert_eq!(agenda::first_times(&std::fs::read_to_string(&bus).unwrap(), &zone), Some((local(8, 10), local(8, 35))));
        // Deleted: its legs to come go with it.
        assert_eq!(sioul_core::anchors::to_come("legs-drag-doctor", &events, Timestamp::now().as_second(), &zone), vec![bus.clone()]);
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// Your own time around a meeting with others (decision 18, the unified
    /// agenda's part 4): the form's time kept before and after it, its costs
    /// and your reminder go into your own file, by its UID; the meeting's file
    /// stays byte for byte, and every reader takes your time from your file:
    /// the expander (the plan, the reminders, the agenda), its form again, and
    /// when to leave.
    #[test]
    fn your_own_time_around_a_meeting_never_writes_into_it() {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Own time", None, &["VEVENT"]).unwrap();
        let file = calendar.dir.join("board.ics");
        let meeting = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:own-time-board\r\nDTSTART:20991104T130000Z\r\nDTEND:20991104T140000Z\r\nSUMMARY:Board\r\nORGANIZER:mailto:boss@example.org\r\nATTENDEE:mailto:someone@example.org\r\nX-SIOUL-BEFORE:5\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        vdir::store_item(&file, meeting).unwrap();
        let key = file.display().to_string();
        let own = sioul_core::own_time::default_path();
        let _ = sioul_core::own_time::set(&own, "own-time-board", None);
        // An older Sioul's line read until your file has the meeting.
        let start = 4_097_480_400; // 2099-11-04 13:00Z
        let read = || sioul_core::agenda::file_occurrences(&file, &calendar, start - 60, start + 60, &TimeZone::system()).into_iter().next().unwrap();
        assert_eq!(read().margins.before, 5);
        let mut edit = edit_with_own(&file).unwrap();
        edit.margins.before = 25;
        edit.remind = "15".into();
        let saved = write_event(&key, &serde_json::to_string(&edit).unwrap(), "").unwrap();
        assert!(saved.accounts.is_empty(), "nothing to send");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), meeting, "the meeting byte for byte");
        let kept = sioul_core::own_time::load(&own);
        assert_eq!(kept.events.get("own-time-board").map(|o| (o.before, o.remind.as_str())), Some((25, "15")));
        // Read everywhere from your file: the expander, the form, when to leave.
        let o = read();
        assert_eq!((o.margins.before, o.remind.as_str()), (25, "15"), "this file wins");
        assert_eq!(edit_with_own(&file).map(|e| e.margins.before), Some(25));
        assert_eq!(sioul_core::pairs::appointment_view(&o, &[], &[], &[], None, tr(), &TimeZone::system()).leave_at, Some(start - 25 * 60));
        // And no writer writes into it: a raw write through the doors is refused.
        assert!(vdir::write_item(&file, &meeting.replace("X-SIOUL-BEFORE:5", "X-SIOUL-BEFORE:25")).is_err());
        let _ = sioul_core::own_time::set(&own, "own-time-board", None);
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// An appointment saved from the form (the computer's and the phone's,
    /// one writer) is on the local clock with its zone, and every reader finds
    /// it at the instant typed: the agenda's, the plan's, Health's week's and
    /// the Porch's overlaps' read (`agenda::occurrences`), the reminders',
    /// the phone's alarms' and the MCP's (the same core), its card (item 11 of
    /// the fix round of 590eb18).
    #[test]
    fn a_form_s_appointment_reads_back_at_its_instant() {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Local clock", None, &["VEVENT"]).unwrap();
        let zone = TimeZone::system();
        let day = Zoned::now().date().checked_add(jiff::Span::new().days(120)).unwrap();
        let typed = day.at(9, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let edit = serde_json::json!({ "title": "Local clock", "start": format!("{day}T09:00"), "end": format!("{day}T09:30") }).to_string();
        let saved = write_event("", &edit, &format!("{}/{}", calendar.account, calendar.id)).unwrap();
        let text = std::fs::read_to_string(&saved.path).unwrap();
        if let Some(name) = zone.iana_name() {
            assert!(text.contains(&format!("DTSTART;TZID={name}:")) && text.contains("BEGIN:VTIMEZONE"), "on the local clock: {text}");
        }
        let found: Vec<i64> = sioul_core::agenda::occurrences(typed - 86_400, typed + 86_400).into_iter().filter(|o| o.summary == "Local clock").map(|o| o.start).collect();
        assert_eq!(found, vec![typed], "read back at the instant typed");
        let card: serde_json::Value = serde_json::from_str(&crate::agenda::event_card(&Shared::default(), &saved.path.display().to_string(), typed)).unwrap();
        assert_eq!(card["start"], typed);
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// Each side of an overlap says its time, which its card opens (review
    /// of 590eb18, later 6): a daily series' Thursday, not its next time.
    #[test]
    fn an_overlap_names_each_time() {
        sioul_core::config::stand_ins_only();
        let start = Zoned::now().timestamp().as_second() + 3 * 86_400;
        let one = |key: &str, from: i64| agenda::Occurrence { key: key.into(), uid: key.into(), summary: key.into(), start: from, end: from + 3_600, recurring: true, ..Default::default() };
        let rows: serde_json::Value = serde_json::from_str(&overlaps_of(&[one("/c/daily.ics", start), one("/c/other.ics", start + 1_800)])).unwrap();
        assert_eq!(rows[0]["first"]["start"], start, "{rows}");
        assert_eq!(rows[0]["second"]["start"], start + 1_800, "{rows}");
    }

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
        // In its own calendar: the time kept before it changes from its form, kept in your
        // own file (decision 18, the unified agenda's part 4); the meeting's file, never.
        let here = format!("{}/{}", from.account, from.id);
        let mut edit = agenda::edit_of(&path).unwrap();
        edit.margins.before = 15;
        assert!(write_event(path.to_str().unwrap(), &serde_json::to_string(&edit).unwrap(), &here).is_ok());
        let kept = std::fs::read_to_string(&path).unwrap();
        assert_eq!(kept, text, "the meeting as it was");
        assert_eq!(sioul_core::own_time::load(&sioul_core::own_time::default_path()).events.get("meeting-1").map(|o| o.before), Some(15));
        // Its organizer's fields typed in its form anyway (they are not offered): never written (must-fix 2).
        let mut theirs = edit.clone();
        theirs.title = "Meeting, moved".into();
        theirs.location = "Room 2".into();
        (theirs.start, theirs.end) = ("2099-10-23T15:00".into(), "2099-10-23T16:00".into());
        assert!(write_event(path.to_str().unwrap(), &serde_json::to_string(&theirs).unwrap(), &here).is_ok());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), kept, "nothing of its organizer's written");
        // And the door refuses it whatever writes it (`agenda::may_change`).
        assert!(vdir::write_item(&path, &kept.replace("SUMMARY:Meeting", "SUMMARY:Meeting, moved")).is_err());
        let _ = sioul_core::own_time::set(&sioul_core::own_time::default_path(), "meeting-1", None);
        let _ = std::fs::remove_dir_all(Kind::Calendars.root().join(account));
        let _ = std::fs::remove_dir_all(&to.dir);
    }

    /// Must-fix 2 of the review of 36002ea: a meeting's own fields (the time
    /// kept before it) go to your own file (`own_time`), never into the
    /// meeting, whatever its notes hold and however the form gives them back
    /// (a notes field turns a no-break space and a line separator into plain
    /// ones), and a meeting with no title too: the meeting's file stays as it
    /// was, byte for byte (the unified agenda's part 4).
    #[test]
    fn a_meeting_s_own_time_goes_to_your_own_file_whatever_its_notes_hold() {
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
            assert_eq!(written, text, "{name}: the meeting as it was");
            assert_eq!(theirs(&written), theirs(&text), "{name}: the organizer's lines as they were");
            let own = sioul_core::own_time::load(&sioul_core::own_time::default_path());
            assert_eq!(own.events.get(&format!("{name}-1")).map(|o| o.before), Some(15), "{name}: in your own file");
            let _ = sioul_core::own_time::set(&sioul_core::own_time::default_path(), &format!("{name}-1"), None);
        }
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// "Into the agenda" (`letters::event_of`): the new event carries the
    /// letter's address labelled with its sender, a comma and an accent
    /// quoted and read back as written, and stands for the letter's
    /// appointment there (review of part 4, later 11); it is in the letter's
    /// project (re-check of 857c39e, §3.4 and §3.6).
    #[test]
    fn a_letter_s_appointment_carries_its_address_and_its_sender() {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Letters' appointments", None, &["VEVENT"]).unwrap();
        let here = format!("{}/{}", calendar.account, calendar.id);
        let reading = sioul_core::letters::Reading { sender: "Caisse, Défense".into(), appointment: Some(("2099-10-29".parse().unwrap(), Some((14, 0)))), ..Default::default() };
        let letter = sioul_core::letters::Letter { id: "2026-10-28-1a2b".into(), file: "letters/inbox/caf.pdf".into(), status: "new".into(), project: "caf-file".into(), reading, ..Default::default() };
        let letters = sioul_core::letters::Letters { root: "/nowhere/Notes".into(), list: vec![letter.clone()] };
        let edit = crate::letters::event_of(&letters, &letter, "2099-10-29".parse().unwrap(), Some((14, 0)));
        let saved = write_event("", &edit.to_string(), &here).unwrap();
        let text = std::fs::read_to_string(&saved.path).unwrap();
        let tie = sioul_core::lines::unfold(&text).into_iter().find(|l| l.ends_with(":sioul:letter/2026-10-28-1a2b"));
        // In the letter's project, as its task would be (re-check of 857c39e, §3.4).
        assert!(sioul_core::lines::unfold(&text).contains(&sioul_core::tasks::project_line("caf-file")), "{text}");
        assert!(tie.as_deref().is_some_and(|l| sioul_core::lines::name(l).ends_with("LINK") && l.contains(";LABEL=\"Caisse, Défense\";") && sioul_core::lines::params(l).iter().any(|(name, value)| name == "LABEL" && value == "Caisse, Défense")), "{text}");
        let seen = sioul_core::dated::EventSeen { start: 0, yours: false, text };
        assert_eq!(sioul_core::dated::letter_event(&letters, &letter, None, &[seen]), Some(0), "it stands for the letter's appointment");
        let _ = std::fs::remove_dir_all(&calendar.dir);
    }

    /// An event of yours that your own file still has (a meeting whose guests
    /// were taken out elsewhere) reads its own lines, and its form saves into
    /// it: the change reads back (review of part 4, later 4).
    #[test]
    fn an_event_of_yours_reads_its_own_lines_whatever_your_own_file_holds() {
        sioul_core::config::stand_ins_only();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Own lines", None, &["VEVENT"]).unwrap();
        let here = format!("{}/{}", calendar.account, calendar.id);
        let path = calendar.dir.join("alone.ics");
        let uid = format!("alone-{}", std::process::id());
        std::fs::write(&path, format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTART:20991023T120000Z\r\nDTEND:20991023T130000Z\r\nSUMMARY:Alone now\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")).unwrap();
        sioul_core::own_time::set(&sioul_core::own_time::default_path(), &uid, Some(sioul_core::own_time::Own { before: 25, remind: "30".into(), ..Default::default() })).unwrap();
        let mut edit = edit_with_own(&path).unwrap();
        assert_eq!((edit.margins.before, edit.remind.as_str()), (0, ""), "its own lines, not your own file's");
        edit.margins.before = 10;
        write_event(path.to_str().unwrap(), &serde_json::to_string(&edit).unwrap(), &here).unwrap();
        assert_eq!(edit_with_own(&path).unwrap().margins.before, 10, "the change reads back");
        let _ = sioul_core::own_time::set(&sioul_core::own_time::default_path(), &uid, None);
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
