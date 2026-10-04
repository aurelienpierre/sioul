// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Google Tasks, over its REST API (v1): Google's CalDAV keeps no task, so a
//! Google account's task lists come this way. Each list is a folder of VTODO
//! files like any other (`calendars/<account>/tasks-<id>`), read by the rest
//! of Sioul as it reads a CalDAV list. Google keeps a title, notes, done or
//! not, a day (no hour), and one level of steps; what it does not keep shows
//! greyed in the task form (sioul_core::capabilities). Lines Google does not
//! know stay in the file here.
//!
//! What changed here goes first: lists made, renamed or deleted; tasks
//! inserted, patched and moved under their parent, deleted. Then what changed
//! there since the last sync (`updatedMin`, deletions included). A task
//! changed on both sides between two syncs: yours is sent, and Google's
//! answer is what stays.

use crate::SyncError;
use crate::dav::Report;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde_json::{Value, json};
use sioul_core::config::Account;
use sioul_core::lines;
use sioul_core::tasks::{self, Status};
use sioul_core::vdir::{self, ItemState, Kind, State};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

const API: &str = "https://tasks.googleapis.com/tasks/v1";

fn api() -> String {
    // A local stand-in for Google, in tests only (never in Sioul's builds).
    #[cfg(feature = "insecure-test-tls")]
    if let Some(base) = std::env::var_os("SIOUL_TEST_GOOGLE_TASKS") {
        return base.to_string_lossy().trim_end_matches('/').to_string();
    }
    API.to_string()
}

/// Where a list is, as its state keeps it.
fn list_url(id: &str) -> String {
    format!("{}/lists/{id}", api())
}

/// Whether a collection's address is a Google Tasks list.
pub fn is_tasks_list(url: &str) -> bool {
    url.starts_with(&format!("{}/lists/", api()))
}

fn list_id(url: &str) -> &str {
    url.rsplit('/').next().unwrap_or("")
}

/// The API, with the account's access token: asked again once when refused.
struct Api<'a> {
    address: &'a str,
    agent: ureq::Agent,
    token: Mutex<String>,
}

impl<'a> Api<'a> {
    fn new(address: &'a str) -> Result<Api<'a>, SyncError> {
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).http_status_as_error(false).build().into();
        Ok(Api { address, agent, token: Mutex::new(crate::google::access_token(address, false)?) })
    }

    /// One call: its status and its JSON (null when none).
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<(u16, Value), SyncError> {
        let url = format!("{}{path}", api());
        for attempt in 0..2 {
            let token = self.token.lock().map(|t| t.clone()).unwrap_or_default();
            let request = ureq::http::Request::builder()
                .method(method)
                .uri(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("User-Agent", "Sioul")
                .header("Content-Type", "application/json")
                .body(body.map(Value::to_string).unwrap_or_default())
                .map_err(|e| SyncError::Server(e.to_string()))?;
            let mut response = self.agent.run(request).map_err(|e| SyncError::Network(e.to_string()))?;
            let status = response.status().as_u16();
            if status == 401 && attempt == 0 {
                let fresh = crate::google::access_token(self.address, true)?;
                if let Ok(mut token) = self.token.lock() {
                    *token = fresh;
                }
                continue;
            }
            if status == 401 {
                return Err(SyncError::Login(format!("{method} {url}: 401")));
            }
            let text = response.body_mut().with_config().limit(16 * 1024 * 1024).read_to_string().unwrap_or_default();
            // The API turned off in the project, or the access not given: said as Google says it.
            if status == 403 {
                let reason: Value = serde_json::from_str(&text).unwrap_or_default();
                return Err(SyncError::Server(format!("Google Tasks: {}", reason["error"]["message"].as_str().unwrap_or("403"))));
            }
            return Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)));
        }
        Err(SyncError::Login(format!("{method} {url}: 401")))
    }

    /// A call that must succeed.
    fn ok(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, SyncError> {
        match self.call(method, path, body)? {
            (200..=299, value) => Ok(value),
            (status, _) => Err(SyncError::Server(format!("{method} {path}: {status}"))),
        }
    }

    /// Every page of a listing.
    fn all(&self, path: &str) -> Result<Vec<Value>, SyncError> {
        let mut items = Vec::new();
        let mut page: Option<String> = None;
        loop {
            let at = match &page {
                Some(token) => format!("{path}&pageToken={}", crate::google::encode(token)),
                None => path.to_string(),
            };
            let answer = self.ok("GET", &at, None)?;
            items.extend(answer["items"].as_array().cloned().unwrap_or_default());
            match answer["nextPageToken"].as_str() {
                Some(token) if !token.is_empty() => page = Some(token.to_string()),
                _ => return Ok(items),
            }
        }
    }
}

/// Whether the access given covers tasks (you may untick it on Google's page).
fn tasks_granted(address: &str) -> bool {
    #[cfg(feature = "insecure-test-tls")]
    if std::env::var_os("SIOUL_TEST_GOOGLE_TASKS").is_some() {
        return true;
    }
    crate::google::grant(address).is_some_and(|g| g.scopes.iter().any(|s| s.ends_with("/auth/tasks")))
}

/// Syncs a Google account's task lists; nothing when tasks were not given.
pub fn sync(account: &Account) -> Result<Report, SyncError> {
    let mut report = Report::default();
    let address = account.address.as_deref().or(account.login()).ok_or(SyncError::NoServer)?;
    if !tasks_granted(address) {
        return Ok(report);
    }
    let api = Api::new(address)?;
    let zone = TimeZone::system();
    send_list_changes(&api, &account.id)?;
    // The lists known here, by their address.
    let known: BTreeMap<String, String> = vdir::every_collection(Kind::Calendars)
        .into_iter()
        .filter(|c| c.account == account.id)
        .map(|c| (State::load(&c.state_path()).url, c.id))
        .filter(|(url, _)| is_tasks_list(url))
        .collect();
    let mut kept = BTreeSet::new();
    for list in api.all("/users/@me/lists?maxResults=100")? {
        let Some(id) = list["id"].as_str() else { continue };
        let url = list_url(id);
        let local = known.get(&url).cloned().unwrap_or_else(|| format!("tasks-{}", vdir::folder_id(id)));
        sync_list(&api, &account.id, id, &local, list["title"].as_str().unwrap_or("Tasks"), &zone, &mut report)?;
        kept.insert(url);
        report.collections += 1;
    }
    forget_gone(&account.id, &kept);
    Ok(report)
}

/// Lists made, renamed or deleted here, told to Google. A list deleted here
/// that holds tasks there (added elsewhere meanwhile) comes back.
fn send_list_changes(api: &Api, account: &str) -> Result<(), SyncError> {
    for collection in vdir::every_collection(Kind::Calendars).into_iter().filter(|c| c.account == account) {
        let path = collection.state_path();
        let mut state = State::load(&path);
        if state.pending && collection.components.iter().any(|c| c.eq_ignore_ascii_case("VTODO")) {
            let made = api.ok("POST", "/users/@me/lists", Some(&json!({ "title": collection.name })))?;
            let id = made["id"].as_str().ok_or_else(|| SyncError::Server("Google Tasks: a list without id".into()))?;
            state.pending = false;
            state.url = list_url(id);
            state.save(&path).map_err(SyncError::Disk)?;
            continue;
        }
        if !is_tasks_list(&state.url) {
            continue;
        }
        let id = list_id(&state.url).to_string();
        if state.renamed {
            api.ok("PATCH", &format!("/users/@me/lists/{id}"), Some(&json!({ "title": collection.name })))?;
            state.renamed = false;
            state.save(&path).map_err(SyncError::Disk)?;
        }
        if state.deleted {
            let held = api.ok("GET", &format!("/lists/{id}/tasks?maxResults=1&showCompleted=true&showHidden=true"), None)?;
            if held["items"].as_array().is_some_and(|items| !items.is_empty()) {
                state.deleted = false;
                state.save(&path).map_err(SyncError::Disk)?;
                continue;
            }
            match api.call("DELETE", &format!("/users/@me/lists/{id}"), None)? {
                (200..=299 | 404, _) => {
                    let _ = std::fs::remove_dir_all(&collection.dir);
                    let _ = std::fs::remove_file(&path);
                }
                (status, _) => return Err(SyncError::Server(format!("DELETE list {id}: {status}"))),
            }
        }
    }
    Ok(())
}

/// Lists deleted on Google go here too, unless something here was never sent.
fn forget_gone(account: &str, kept: &BTreeSet<String>) {
    for collection in vdir::every_collection(Kind::Calendars).into_iter().filter(|c| c.account == account) {
        let state = State::load(&collection.state_path());
        if !is_tasks_list(&state.url) || kept.contains(&state.url) || state.pending {
            continue;
        }
        let known: BTreeSet<&str> = state.items.iter().map(|i| i.file.as_str()).collect();
        let unsent = files(&collection.dir).iter().any(|f| !known.contains(f.as_str()))
            || state.items.iter().any(|i| hash_of(&collection.dir.join(&i.file)).is_some_and(|h| h != i.hash));
        if unsent {
            continue;
        }
        let _ = std::fs::remove_dir_all(&collection.dir);
        let _ = std::fs::remove_file(collection.state_path());
    }
}

fn files(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.ends_with(".ics") && !n.starts_with('.')).collect()
}

fn hash_of(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|bytes| vdir::content_hash(&bytes))
}

#[allow(clippy::too_many_arguments)]
fn sync_list(api: &Api, account: &str, id: &str, local: &str, title: &str, zone: &TimeZone, report: &mut Report) -> Result<(), SyncError> {
    let dir = vdir::prepare(Kind::Calendars, account, local, title, None).map_err(|e| SyncError::Disk(e.to_string()))?;
    let state_path = vdir::state_path(account, Kind::Calendars, local);
    let mut state = State::load(&state_path);
    state.url = list_url(id);
    state.components = vec!["VTODO".into()];
    state.read_only = false;
    // A little before now: what changes while this runs comes again next time.
    let started = Timestamp::now() - jiff::SignedDuration::from_secs(120);
    push(api, id, &dir, &mut state, zone, report)?;
    pull(api, id, &dir, &mut state, report)?;
    state.sync_token = Some(started.strftime("%Y-%m-%dT%H:%M:%S.000Z").to_string());
    state.save(&state_path).map_err(SyncError::Disk)
}

/// The UID in a task's file.
fn uid_in(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    lines::unfold(&text).iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string())
}

/// What changed here, sent: deleted, changed (patched, moved under its
/// parent), new (inserted, parents first: Google has one level of steps).
fn push(api: &Api, list: &str, dir: &Path, state: &mut State, zone: &TimeZone, report: &mut Report) -> Result<(), SyncError> {
    // UID → Google's id, for parents.
    let mut google_of: BTreeMap<String, String> = state.items.iter().filter_map(|i| Some((uid_in(&dir.join(&i.file))?, i.href.clone()))).collect();
    let mut kept: Vec<ItemState> = Vec::new();
    let mut again: Vec<String> = Vec::new();
    for item in std::mem::take(&mut state.items) {
        let path = dir.join(&item.file);
        let Some(hash) = hash_of(&path) else {
            match api.call("DELETE", &format!("/lists/{list}/tasks/{}", item.href), None)? {
                (200..=299 | 404 | 410, _) => report.sent += 1,
                (status, _) => return Err(SyncError::Server(format!("DELETE task {}: {status}", item.href))),
            }
            continue;
        };
        if hash == item.hash {
            kept.push(item);
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| SyncError::Disk(e.to_string()))?;
        let Some(task) = tasks::task_of_text(&text, zone) else {
            kept.push(item);
            continue;
        };
        let answer = match api.call("PATCH", &format!("/lists/{list}/tasks/{}", item.href), Some(&fields_of(&task)))? {
            (200..=299, answer) => answer,
            // Gone there meanwhile: yours goes again, as new.
            (404 | 410, _) => {
                again.push(item.file);
                continue;
            }
            (status, _) => return Err(SyncError::Server(format!("PATCH task {}: {status}", item.href))),
        };
        let wanted = task.parent().and_then(|uid| google_of.get(uid)).cloned();
        let answer = if answer["parent"].as_str().map(str::to_string) != wanted { moved(api, list, &item.href, wanted.as_deref())? } else { answer };
        let text = merged(&text, &answer, task.parent());
        vdir::write_item(&path, &text).map_err(SyncError::Disk)?;
        report.sent += 1;
        kept.push(ItemState { etag: answer["etag"].as_str().unwrap_or_default().to_string(), hash: vdir::content_hash(text.as_bytes()), ..item });
    }
    // New here: those whose parent is new too come after it.
    let known: BTreeSet<String> = kept.iter().map(|i| i.file.clone()).collect();
    let mut new: Vec<(String, String, tasks::Task)> = files(dir)
        .into_iter()
        .filter(|f| !known.contains(f))
        .chain(again)
        .filter_map(|file| {
            let text = std::fs::read_to_string(dir.join(&file)).ok()?;
            let task = tasks::task_of_text(&text, zone)?;
            Some((file, text, task))
        })
        .collect();
    let new_uids: BTreeSet<String> = new.iter().map(|(_, _, t)| t.uid.clone()).collect();
    new.sort_by_key(|(_, _, t)| t.parent().is_some_and(|p| new_uids.contains(p)));
    for (file, text, task) in new {
        let parent = task.parent().and_then(|uid| google_of.get(uid)).cloned();
        let at = match &parent {
            Some(parent) => format!("/lists/{list}/tasks?parent={}", crate::google::encode(parent)),
            None => format!("/lists/{list}/tasks"),
        };
        let answer = api.ok("POST", &at, Some(&fields_of(&task)))?;
        let Some(id) = answer["id"].as_str() else { continue };
        google_of.insert(task.uid.clone(), id.to_string());
        let text = merged(&text, &answer, task.parent());
        vdir::write_item(&dir.join(&file), &text).map_err(SyncError::Disk)?;
        report.sent += 1;
        kept.push(ItemState { href: id.to_string(), file, etag: answer["etag"].as_str().unwrap_or_default().to_string(), hash: vdir::content_hash(text.as_bytes()) });
    }
    state.items = kept;
    Ok(())
}

/// A task put under another (or at the top, `None`); Google's answer.
fn moved(api: &Api, list: &str, id: &str, parent: Option<&str>) -> Result<Value, SyncError> {
    let at = match parent {
        Some(parent) => format!("/lists/{list}/tasks/{id}/move?parent={}", crate::google::encode(parent)),
        None => format!("/lists/{list}/tasks/{id}/move"),
    };
    api.ok("POST", &at, None)
}

/// What changed there since the last sync; everything the first time, and
/// then what is no longer there is gone here too.
fn pull(api: &Api, list: &str, dir: &Path, state: &mut State, report: &mut Report) -> Result<(), SyncError> {
    let since = state.sync_token.clone();
    let mut path = format!("/lists/{list}/tasks?maxResults=100&showCompleted=true&showHidden=true&showDeleted=true");
    if let Some(since) = &since {
        path.push_str(&format!("&updatedMin={}", crate::google::encode(since)));
    }
    let found = api.all(&path)?;
    let mut seen = BTreeSet::new();
    for task in &found {
        let Some(id) = task["id"].as_str() else { continue };
        seen.insert(id.to_string());
        let position = state.items.iter().position(|i| i.href == id);
        if task["deleted"].as_bool() == Some(true) {
            if let Some(i) = position {
                let _ = std::fs::remove_file(dir.join(&state.items[i].file));
                state.items.remove(i);
                report.removed += 1;
            }
            continue;
        }
        let etag = task["etag"].as_str().unwrap_or_default();
        if position.is_some_and(|i| state.items[i].etag == etag && !etag.is_empty()) {
            continue;
        }
        // The parent by its UID here: one made here has its own.
        let parent = task["parent"].as_str().map(|p| state.items.iter().find(|i| i.href == p).and_then(|i| uid_in(&dir.join(&i.file))).unwrap_or_else(|| p.to_string()));
        let (file, before) = match position {
            Some(i) => (state.items[i].file.clone(), std::fs::read_to_string(dir.join(&state.items[i].file)).unwrap_or_else(|_| skeleton(id))),
            None => {
                let taken: BTreeSet<String> = state.items.iter().map(|i| i.file.clone()).chain(files(dir)).collect();
                let stem = vdir::folder_id(id);
                let file = std::iter::once(format!("{stem}.ics")).chain((2..).map(|n| format!("{stem}-{n}.ics"))).find(|f| !taken.contains(f)).unwrap_or_default();
                (file, skeleton(id))
            }
        };
        let text = merged(&before, task, parent.as_deref());
        vdir::write_item(&dir.join(&file), &text).map_err(SyncError::Disk)?;
        let item = ItemState { href: id.to_string(), file, etag: etag.to_string(), hash: vdir::content_hash(text.as_bytes()) };
        match position {
            Some(i) => state.items[i] = item,
            None => state.items.push(item),
        }
        report.received += 1;
    }
    if since.is_none() {
        let mut gone = Vec::new();
        state.items.retain(|i| {
            let here = seen.contains(&i.href);
            if !here {
                gone.push(i.file.clone());
            }
            here
        });
        for file in gone {
            let _ = std::fs::remove_file(dir.join(file));
            report.removed += 1;
        }
    }
    Ok(())
}

/// A task Google made: its own id as UID.
fn skeleton(uid: &str) -> String {
    lines::fold(&["BEGIN:VCALENDAR".into(), "VERSION:2.0".into(), "PRODID:-//Sioul//Sioul//EN".into(), "BEGIN:VTODO".into(), format!("UID:{uid}"), "END:VTODO".into(), "END:VCALENDAR".into()])
}

/// What Google keeps of a task, as its API takes it.
pub fn fields_of(task: &tasks::Task) -> Value {
    let done = matches!(task.status, Status::Completed | Status::Cancelled);
    let completed = done.then(|| task.completed.and_then(|s| Timestamp::from_second(s).ok()).unwrap_or_else(Timestamp::now).strftime("%Y-%m-%dT%H:%M:%S.000Z").to_string());
    let due = task.due_date().map(|d| format!("{d}T00:00:00.000Z"));
    json!({
        "title": task.title.chars().take(1024).collect::<String>(),
        "notes": task.notes.chars().take(8192).collect::<String>(),
        "status": if done { "completed" } else { "needsAction" },
        "completed": completed,
        "due": due,
    })
}

/// "2026-10-03T12:00:00.000Z" → "20261003T120000Z".
fn stamp(rfc3339: &str) -> Option<String> {
    rfc3339.parse::<Timestamp>().ok().map(|t| t.strftime("%Y%m%dT%H%M%SZ").to_string())
}

/// What Google says of a task, as VTODO lines.
fn google_lines(task: &Value, parent: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(updated) = task["updated"].as_str().and_then(stamp) {
        out.push(format!("DTSTAMP:{updated}"));
        out.push(format!("LAST-MODIFIED:{updated}"));
    }
    out.push(format!("SUMMARY:{}", lines::escape(task["title"].as_str().unwrap_or(""))));
    if let Some(notes) = task["notes"].as_str().filter(|n| !n.is_empty()) {
        out.push(format!("DESCRIPTION:{}", lines::escape(notes)));
    }
    let done = task["status"].as_str() == Some("completed");
    out.push(format!("STATUS:{}", if done { "COMPLETED" } else { "NEEDS-ACTION" }));
    if done && let Some(at) = task["completed"].as_str().and_then(stamp) {
        out.push(format!("COMPLETED:{at}"));
    }
    // Google's "due" is a day: "the day the task should be done".
    if let Some(day) = task["due"].as_str().and_then(|d| d.get(..10)) {
        out.push(format!("DUE;VALUE=DATE:{}", day.replace('-', "")));
    }
    if let Some(parent) = parent {
        out.push(tasks::relation_line("PARENT", parent, 0));
    }
    out
}

/// The task's file with what Google keeps written again from its answer; the
/// rest (lines Google does not know, alarms) stays as it was.
pub fn merged(text: &str, task: &Value, parent: Option<&str>) -> String {
    const GOOGLES: &[&str] = &["SUMMARY", "DESCRIPTION", "STATUS", "COMPLETED", "DUE", "DTSTAMP", "LAST-MODIFIED", "PERCENT-COMPLETE"];
    let source = lines::unfold(text);
    let mut out = Vec::with_capacity(source.len() + 8);
    let (mut in_todo, mut nested, mut placed) = (false, 0usize, false);
    for line in source {
        let name = lines::name(&line);
        let value = lines::value(&line).trim().to_ascii_uppercase();
        match (name.as_str(), value.as_str()) {
            ("BEGIN", "VTODO") if !in_todo => in_todo = true,
            ("BEGIN", _) if in_todo => {
                if nested == 0 && !placed {
                    out.extend(google_lines(task, parent));
                    placed = true;
                }
                nested += 1;
            }
            ("END", "VTODO") if in_todo && nested == 0 => {
                if !placed {
                    out.extend(google_lines(task, parent));
                    placed = true;
                }
                in_todo = false;
            }
            ("END", _) if in_todo && nested > 0 => nested -= 1,
            _ if in_todo && nested == 0 => {
                let parent_line = name == "RELATED-TO" && lines::param(&line, "RELTYPE").is_none_or(|t| t.eq_ignore_ascii_case("PARENT"));
                if GOOGLES.contains(&name.as_str()) || parent_line {
                    continue;
                }
            }
            _ => {}
        }
        out.push(line);
    }
    lines::fold(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_both_ways() {
        let google = json!({ "id": "abc", "etag": "\"1\"", "title": "Call the notary, today", "notes": "Ask for the deed", "status": "completed",
            "completed": "2026-10-03T09:30:00.000Z", "due": "2026-10-05T00:00:00.000Z", "updated": "2026-10-03T09:31:00.000Z", "parent": "top" });
        let text = merged(&skeleton("abc"), &google, Some("top-uid"));
        let task = tasks::task_of_text(&text, &TimeZone::UTC).unwrap();
        assert_eq!((task.uid.as_str(), task.title.as_str(), task.notes.as_str()), ("abc", "Call the notary, today", "Ask for the deed"));
        assert_eq!((task.status, task.due.as_str(), task.parent()), (Status::Completed, "2026-10-05", Some("top-uid")));
        let back = fields_of(&task);
        assert_eq!(back["status"], "completed");
        assert_eq!(back["completed"], "2026-10-03T09:30:00.000Z");
        assert_eq!(back["due"], "2026-10-05T00:00:00.000Z");
        // What Google does not know stays; what it says replaces the old.
        let mine = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:mine\r\nSUMMARY:Old\r\nCATEGORIES:joy\r\nRELATED-TO;RELTYPE=DEPENDS-ON:other\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nEND:VALARM\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let open = json!({ "title": "New", "status": "needsAction" });
        let text = merged(mine, &open, None);
        assert!(text.contains("UID:mine") && text.contains("CATEGORIES:joy") && text.contains("RELATED-TO;RELTYPE=DEPENDS-ON:other"), "{text}");
        assert!(text.contains("SUMMARY:New\r\nSTATUS:NEEDS-ACTION\r\nBEGIN:VALARM") && !text.contains("Old"), "{text}");
        assert_eq!(fields_of(&tasks::task_of_text(&text, &TimeZone::UTC).unwrap())["due"], Value::Null);
    }

    /// Against a stand-in for Google (tools/google-tasks-stand-in.py), with
    /// SIOUL_TEST_GOOGLE_TASKS, SIOUL_TEST_GOOGLE_TOKEN and XDG_* set to a
    /// scratch folder: `cargo test -p sioul-sync --features insecure-test-tls -- --ignored stand_in`.
    #[cfg(feature = "insecure-test-tls")]
    #[test]
    #[ignore]
    fn against_a_stand_in() {
        let base = std::env::var("SIOUL_TEST_GOOGLE_TASKS").expect("SIOUL_TEST_GOOGLE_TASKS");
        let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        let google = |path: &str| -> Value { serde_json::from_str(&agent.get(&format!("{base}/_state{path}")).call().unwrap().body_mut().read_to_string().unwrap()).unwrap() };
        let poke = |path: &str, body: Value| {
            agent.post(&format!("{base}/_poke{path}")).send(body.to_string()).unwrap();
        };
        let account: Account = toml::from_str("id = \"g\"\nkind = \"dav\"\naddress = \"you@example.org\"\nhost = \"apidata.googleusercontent.com\"\nauth = \"google\"\n").unwrap();
        let zone = TimeZone::UTC;
        let list = || vdir::collections(Kind::Calendars).into_iter().find(|c| c.account == "g" && c.id == "tasks-L1").expect("tasks-L1 here");
        let read = |uid: &str| tasks::all(&zone).into_iter().find(|t| t.uid == uid);

        // First sync: everything comes, the step under its task.
        let report = sync(&account).unwrap();
        assert_eq!((report.collections, report.received), (1, 3), "{report:?}");
        assert_eq!(read("t2").unwrap().parent(), Some("t1"));
        assert_eq!(read("t1").unwrap().due, "2026-10-05");

        // Here: one renamed, one new step under it, one deleted. There: one changed.
        let dir = list().dir;
        let t1 = dir.join("t1.ics");
        std::fs::write(&t1, std::fs::read_to_string(&t1).unwrap().replace("SUMMARY:Buy stamps", "SUMMARY:Buy stamps today")).unwrap();
        let edit = tasks::TaskEdit { title: "Find a pen".into(), parent: "t1".into(), ..Default::default() };
        std::fs::write(dir.join("pen.ics"), tasks::new_task(&edit, "pen-uid", &zone, &jiff::Zoned::now()).unwrap()).unwrap();
        std::fs::remove_file(dir.join("t3.ics")).unwrap();
        poke("/lists/L1/tasks/t2", json!({ "title": "Find the envelope, the big one" }));
        let report = sync(&account).unwrap();
        assert_eq!((report.sent, report.received), (3, 1), "{report:?}");
        let there = google("/lists/L1");
        let by_title = |title: &str| there["tasks"].as_array().unwrap().iter().find(|t| t["title"] == title).cloned();
        assert!(by_title("Buy stamps today").is_some(), "{there}");
        assert_eq!(by_title("Find a pen").expect("the pen")["parent"], "t1");
        assert_eq!(there["tasks"].as_array().unwrap().iter().find(|t| t["id"] == "t3").unwrap()["deleted"], true);
        assert_eq!(read("t2").unwrap().title, "Find the envelope, the big one");
        assert_eq!(read("pen-uid").unwrap().parent(), Some("t1"), "the new step keeps its UID here");

        // A list made here, renamed, deleted.
        let made = vdir::create(Kind::Calendars, "g", "Errands", None, &["VTODO"]).unwrap();
        sync(&account).unwrap();
        assert!(google("").as_array().unwrap().iter().any(|l| l["title"] == "Errands"));
        let made = vdir::collections(Kind::Calendars).into_iter().find(|c| c.id == made.id).unwrap();
        vdir::rename_collection(&made, "Errands, Saturday").unwrap();
        sync(&account).unwrap();
        assert!(google("").as_array().unwrap().iter().any(|l| l["title"] == "Errands, Saturday"));
        vdir::delete_collection(&vdir::collections(Kind::Calendars).into_iter().find(|c| c.id == made.id).unwrap()).unwrap();
        sync(&account).unwrap();
        assert!(!google("").as_array().unwrap().iter().any(|l| l["title"].as_str().unwrap_or("").starts_with("Errands")));
        assert!(!vdir::every_collection(Kind::Calendars).iter().any(|c| c.id == made.id));

        // A list deleted on Google goes here too.
        poke("/delete-list/L1", json!({}));
        sync(&account).unwrap();
        assert!(!vdir::every_collection(Kind::Calendars).iter().any(|c| c.id == "tasks-L1"));
    }
}
