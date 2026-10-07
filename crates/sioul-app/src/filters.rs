// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail filters in the window (docs/client.md, "Filters"; the rules are
//! `sioul_core::rules`, what reaches the server `sioul_sync::filters`).
//!
//! In Mail ▸ ⚙, under "Filters" (`MailFilters.qml`): the one list, each
//! filter in words, narrowed to an address when you choose one; edited in
//! place and saved at once into the configuration, as every setting is;
//! "Try it", which counts what a filter takes in your inboxes now; and
//! running them all on the inboxes, read mail included: said first, done
//! after ten seconds to undo (`mail::schedule_filters`).
//!
//! After each fetch, on the watcher's own thread (never the window's), the
//! arrivals still unread are filtered on the server (`after_fetch`); what a
//! filter moves out of the inbox or marks read is never told as new mail
//! (`untold`, for `mailnote`), whichever device acts on it.

use crate::backend::{QtThread, Shared, config_path, load_config, say, tr};
use cxx_qt_lib::QString;
use serde::{Deserialize, Serialize};
use sioul_core::config::{Account, Config};
use sioul_core::folders::Role;
use sioul_core::porch::{Senders, Triaged};
use sioul_core::rules::{self, Act, Condition, Field, Filter, Join, Test};
use sioul_sync::filters::{Done, Job};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// "Try it" reads the newest messages of the inboxes, this many at most: enough to see what a filter takes.
const TRIED: usize = 2000;
/// Messages named under "Try it": a few, to see that they are the right ones.
const NAMED: usize = 3;

/// A filter as the editor has it: each part plain, its sentence, what keeps
/// it from running, and to which addresses alone it applies (said, never read back).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Edited {
    #[serde(default)]
    name: String,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    accounts: Vec<String>,
    /// One condition is enough.
    #[serde(default)]
    any: bool,
    #[serde(default)]
    conditions: Vec<EditedCondition>,
    #[serde(default)]
    actions: Vec<EditedAct>,
    #[serde(default)]
    stop: bool,
    #[serde(default, skip_deserializing)]
    said: String,
    #[serde(default, skip_deserializing)]
    problem: String,
    #[serde(default, skip_deserializing)]
    only: String,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EditedCondition {
    #[serde(default)]
    field: String,
    #[serde(default)]
    test: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    until: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EditedAct {
    #[serde(default, rename = "do")]
    id: String,
    /// Its folder, its keyword.
    #[serde(default)]
    name: String,
}

impl Edited {
    fn of(filter: &Filter, config: &Config) -> Edited {
        let only = if filter.accounts.is_empty() {
            String::new()
        } else {
            let names: Vec<String> = filter.accounts.iter().map(|id| config.account(id).and_then(|a| a.address.clone()).unwrap_or_else(|| id.clone())).collect();
            say("filter-only", &[("accounts", names.join(", "))])
        };
        Edited {
            name: filter.name.clone(),
            enabled: filter.enabled,
            accounts: filter.accounts.clone(),
            any: filter.join == Join::Any,
            conditions: filter.conditions.iter().map(|c| EditedCondition { field: c.field.id().into(), test: c.test.id().into(), value: c.value.clone(), until: c.until.clone() }).collect(),
            actions: filter.actions.iter().map(|a| EditedAct { id: a.id().into(), name: a.name().to_string() }).collect(),
            stop: filter.stop,
            said: filter.said(tr()),
            problem: filter.problem().map(|id| tr().text(id, None)).unwrap_or_default(),
            only,
        }
    }

    fn filter(&self) -> Filter {
        Filter {
            name: self.name.trim().to_string(),
            enabled: self.enabled,
            accounts: self.accounts.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect(),
            join: if self.any { Join::Any } else { Join::All },
            conditions: self
                .conditions
                .iter()
                .map(|c| Condition {
                    field: Field::parse(&c.field).unwrap_or(Field::Unknown),
                    test: Test::parse(&c.test).unwrap_or(Test::Unknown),
                    value: c.value.trim().to_string(),
                    until: c.until.trim().to_string(),
                })
                .collect(),
            actions: self.actions.iter().map(|a| Act::of(&a.id, &a.name)).collect(),
            stop: self.stop,
        }
    }
}

/// An address, for the narrowing and the editor's "On which addresses".
#[derive(Debug, Serialize)]
struct Address {
    id: String,
    label: String,
}

/// A folder a filter may move mail into, by the name you read, with the addresses that have it.
#[derive(Debug, Serialize)]
struct FolderChoice {
    name: String,
    accounts: Vec<String>,
}

#[derive(Debug, Serialize)]
struct View {
    filters: Vec<Edited>,
    accounts: Vec<Address>,
    folders: Vec<FolderChoice>,
    form: rules::Form,
    /// The filter to open in the editor, by its place: one just made from a search; -1 for none.
    open: i64,
    /// What that search held that a filter does not read, said; "" else.
    note: String,
}

/// A filter just made from a search (`new_from`): its place in the list, and
/// what of the search was left out, said; taken by the next `view`, which opens it.
static OPENED: Mutex<Option<(usize, String)>> = Mutex::new(None);

/// The filters as the editor shows them, with the addresses, their folders and the editor's words, as JSON.
pub(crate) fn view() -> String {
    let config = load_config();
    let (open, note) = OPENED.lock().ok().and_then(|mut opened| opened.take()).filter(|(at, _)| *at < config.mail.filters.len()).map_or((-1, String::new()), |(at, note)| (at as i64, note));
    let accounts: Vec<&Account> = config.accounts.iter().filter(|a| a.syncs()).collect();
    let mut folders: Vec<FolderChoice> = Vec::new();
    for account in &accounts {
        for folder in sioul_sync::mailbox::folders(&account.id).into_iter().filter(|f| matches!(f.role, Role::Other | Role::Archive)) {
            match folders.iter_mut().find(|c| c.name.to_lowercase() == folder.display.to_lowercase()) {
                Some(choice) => choice.accounts.push(account.id.clone()),
                None => folders.push(FolderChoice { name: folder.display.clone(), accounts: vec![account.id.clone()] }),
            }
        }
    }
    folders.sort_by_key(|f| f.name.to_lowercase());
    let view = View {
        filters: config.mail.filters.iter().map(|f| Edited::of(f, &config)).collect(),
        accounts: accounts.iter().map(|a| Address { id: a.id.clone(), label: a.address.clone().unwrap_or_else(|| a.id.clone()) }).collect(),
        folders,
        form: rules::form(tr()),
        open,
        note,
    };
    serde_json::to_string(&view).unwrap_or_default()
}

/// What "Make it a filter…" gives (MailSearch.qml): the search's conditions,
/// as `rules::Condition` writes them, all of them or any; a name and actions
/// when a whole filter is given (the window's pictures), as the editor has them.
#[derive(Debug, Default, Deserialize)]
struct Given {
    #[serde(default)]
    conditions: Vec<Condition>,
    #[serde(default, rename = "match")]
    join: Join,
    #[serde(default)]
    name: String,
    #[serde(default)]
    actions: Vec<EditedAct>,
}

/// A new filter, last in the list, from a search (`Given`, as JSON), saved
/// and opened in the editor the next time the list is shown; what went wrong, else "".
pub(crate) fn new_from(given: &str) -> String {
    let given: Given = match serde_json::from_str(given) {
        Ok(given) => given,
        Err(e) => return e.to_string(),
    };
    let config = load_config();
    let ids: Vec<String> = config.accounts.iter().filter(|a| a.syncs()).map(|a| a.id.clone()).collect();
    let (filter, left_out) = from_search(given, &ids);
    let mut filters = config.mail.filters.clone();
    filters.push(filter);
    if let Err(e) = sioul_core::config::set_filters(&config_path(), &filters) {
        return e;
    }
    let note = if left_out { tr().text("filter-from-search-left", None) } else { String::new() };
    if let Ok(mut opened) = OPENED.lock() {
        *opened = Some((filters.len() - 1, note));
    }
    String::new()
}

/// A filter from a search's conditions, and whether some were left out. A
/// filter reads what arrives in an inbox: the search's address conditions
/// become the addresses it runs on; a folder condition other than the inbox,
/// and a mark other than "not read", are left out. Without a condition left,
/// one to fill; without an action given, one folder to choose.
fn from_search(given: Given, ids: &[String]) -> (Filter, bool) {
    let mut left_out = false;
    let mut only: Option<Vec<String>> = None;
    let mut conditions = Vec::new();
    for condition in given.conditions {
        let value = condition.value.trim().to_string();
        match condition.field {
            Field::Account => {
                let kept = only.take().unwrap_or_else(|| ids.to_vec());
                only = Some(kept.into_iter().filter(|id| (*id == value) == (condition.test == Test::Is)).collect());
            }
            Field::Folder if condition.test == Test::Is && rules::folder_named(&value, Role::Inbox, "") => {}
            Field::Mark if condition.test == Test::IsNot && value == "read" => {}
            Field::Folder | Field::Mark => left_out = true,
            _ => conditions.push(condition),
        }
    }
    let accounts = match only {
        // Every address, or none (conditions at odds): the filter runs on every one.
        Some(list) if list.is_empty() || list.len() == ids.len() => {
            left_out |= list.is_empty();
            Vec::new()
        }
        Some(list) => list,
        None => Vec::new(),
    };
    if conditions.is_empty() {
        conditions.push(Condition::new(Field::From, Test::Contains, ""));
    }
    let mut actions: Vec<Act> = given.actions.iter().map(|a| Act::of(&a.id, &a.name)).collect();
    if actions.is_empty() {
        actions.push(Act::Move { folder: String::new() });
    }
    let filter = Filter { name: given.name.trim().to_string(), accounts, join: given.join, conditions, actions, ..Filter::default() };
    (filter, left_out)
}

/// The whole list written, in its order, as the editor has it (JSON); what went wrong, else "".
pub(crate) fn save(list: &str) -> String {
    let edited: Vec<Edited> = match serde_json::from_str(list) {
        Ok(edited) => edited,
        Err(e) => return e.to_string(),
    };
    let filters: Vec<Filter> = edited.iter().map(Edited::filter).collect();
    sioul_core::config::set_filters(&config_path(), &filters).err().unwrap_or_default()
}

/// One filter being edited, as the list would show it: {said, problem, only}, as JSON.
pub(crate) fn said(one: &str) -> String {
    let config = load_config();
    let edited: Edited = serde_json::from_str(one).unwrap_or_default();
    let shown = Edited::of(&edited.filter(), &config);
    serde_json::json!({ "said": shown.said, "problem": shown.problem, "only": shown.only }).to_string()
}

/// What "Try it" and "Run them on the inboxes" found, for the window.
#[derive(Debug, Default, Serialize)]
struct Found {
    /// "try" for one filter, "preview" for all of them.
    kind: String,
    /// The count, in a sentence.
    text: String,
    /// "Paul, Déjeuner": a few of the messages one filter takes; each filter's count and sentence for all of them.
    lines: Vec<String>,
    /// Messages that would change.
    count: usize,
}

/// What all the filters would do, kept from the preview for the run, so that it does what was said.
static PREVIEWED: Mutex<Vec<(Account, Vec<Job>)>> = Mutex::new(Vec::new());

/// The inboxes' messages kept here, the newest first, at most `limit`, by account.
fn inboxes(config: &Config, limit: Option<usize>) -> (Vec<(Account, Vec<PathBuf>)>, usize, bool) {
    let mut all: Vec<(i64, usize, PathBuf)> = Vec::new();
    let accounts: Vec<Account> = config.accounts.iter().filter(|a| a.syncs()).cloned().collect();
    for (at, account) in accounts.iter().enumerate() {
        for file in sioul_sync::filters::inbox_files(account) {
            // Stored when: the start of its name (`maildir::store`).
            let stored = file.file_name().and_then(|n| n.to_str()).and_then(|n| n.split('.').next()).and_then(|s| s.parse().ok()).unwrap_or(0);
            all.push((stored, at, file));
        }
    }
    let total = all.len();
    all.sort_by_key(|(stored, _, _)| std::cmp::Reverse(*stored));
    let cut = limit.is_some_and(|limit| total > limit);
    if let Some(limit) = limit {
        all.truncate(limit);
    }
    let mut by: Vec<(Account, Vec<PathBuf>)> = accounts.into_iter().map(|a| (a, Vec::new())).collect();
    for (_, at, file) in all {
        by[at].1.push(file);
    }
    (by, total.min(limit.unwrap_or(total)), cut)
}

/// "Try it": what one filter, as it is being edited (JSON), takes in the
/// inboxes now, the newest messages first; `mail_filters_found` brings it.
pub(crate) fn try_one(qt: QtThread, one: String) {
    std::thread::spawn(move || {
        let edited: Edited = serde_json::from_str(&one).unwrap_or_default();
        let mut filter = edited.filter();
        filter.enabled = true;
        let found = match filter.problem() {
            Some(id) => Found { kind: "try".into(), text: tr().text(id, None), ..Found::default() },
            None => {
                let mut config = load_config();
                config.mail.filters = vec![filter];
                let (inboxes, total, cut) = inboxes(&config, Some(TRIED));
                let jobs: Vec<Job> = inboxes.iter().flat_map(|(account, files)| sioul_sync::filters::jobs(&config, account, files)).collect();
                let mut args = tr().counted(jobs.len());
                args.set("total", total);
                let text = tr().text(if cut { "filter-tried-newest" } else { "filter-tried" }, Some(&args));
                let lines = jobs.iter().filter_map(|job| sioul_core::maildir::read_one(&job.file)).take(NAMED).map(|card| say("filter-tried-one", &[("who", card.sender().to_string()), ("subject", card.subject.clone())])).collect();
                Found { kind: "try".into(), text, lines, count: jobs.len() }
            }
        };
        let json = serde_json::to_string(&found).unwrap_or_default();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().mail_filters_found(QString::from(&json)));
    });
}

/// "Run them on the inboxes": what every filter would do to every message
/// of the inboxes, read ones included, said first; kept for the run.
pub(crate) fn preview(qt: QtThread) {
    std::thread::spawn(move || {
        let config = load_config();
        let (inboxes, _, _) = inboxes(&config, None);
        let planned: Vec<(Account, Vec<Job>)> = inboxes.iter().map(|(account, files)| (account.clone(), sioul_sync::filters::jobs(&config, account, files))).filter(|(_, jobs)| !jobs.is_empty()).collect();
        let count: usize = planned.iter().map(|(_, jobs)| jobs.len()).sum();
        let mut lines = Vec::new();
        for (at, filter) in config.mail.filters.iter().enumerate() {
            let took = planned.iter().flat_map(|(_, jobs)| jobs).filter(|job| job.plan.by.contains(&at)).count();
            if took > 0 {
                let mut args = tr().counted(took);
                args.set("filter", if filter.name.is_empty() { filter.said(tr()) } else { format!("{} ({})", filter.name, filter.said(tr())) });
                lines.push(tr().text("filter-preview-line", Some(&args)));
            }
        }
        let text = tr().text(if count == 0 { "filter-preview-none" } else { "filter-preview" }, Some(&tr().counted(count)));
        if let Ok(mut kept) = PREVIEWED.lock() {
            *kept = planned;
        }
        let json = serde_json::to_string(&Found { kind: "preview".into(), text, lines, count }).unwrap_or_default();
        let _ = qt.queue(move |mut sioul| sioul.as_mut().mail_filters_found(QString::from(&json)));
    });
}

/// "Run them": what the preview said, after ten seconds to undo. What went wrong, else "".
pub(crate) fn run(qt: &QtThread, shared: &Arc<Shared>) -> String {
    let planned = PREVIEWED.lock().map(|mut kept| std::mem::take(&mut *kept)).unwrap_or_default();
    let count: usize = planned.iter().map(|(_, jobs)| jobs.len()).sum();
    if count == 0 {
        return tr().text("filter-preview-none", None);
    }
    let line = tr().text("filter-running", Some(&tr().counted(count)));
    crate::mail::schedule_filters(qt, shared, planned, line);
    String::new()
}

/// The run, once its ten seconds are over: each account in one connection;
/// what the status line says.
pub(crate) fn perform(planned: &[(Account, Vec<Job>)]) -> Option<String> {
    let mut done = Done::default();
    // An account that could not be reached: the others are still done, the first such said.
    let mut unreached: Option<String> = None;
    for (account, jobs) in planned {
        let result = sioul_sync::secret::password(account).and_then(|password| sioul_sync::filters::run(account, &password, jobs, false));
        match result {
            Ok(one) => {
                done.acted.extend(one.acted);
                done.failed.extend(one.failed);
            }
            Err(e) => {
                unreached.get_or_insert_with(|| e.sentence(tr(), &account.id));
            }
        }
    }
    let mut line = tr().text("filter-ran", Some(&tr().counted(done.acted.len())));
    for why in [failures(&done), unreached].into_iter().flatten() {
        line = format!("{line} {why}");
    }
    Some(line)
}

/// What could not be done, in a sentence, for the account of its first
/// failure (`Done::said`); none when everything was.
fn failures(done: &Done) -> Option<String> {
    let (job, _) = done.failed.first()?;
    let account = load_config().account_of(&job.file).map(|a| a.address.clone().unwrap_or_else(|| a.id.clone())).unwrap_or_default();
    done.said(tr(), &account)
}

/// What a filters' run after a fetch did: how many messages it took, what
/// could not be done, said ("" when all was), and the messages to tell as
/// new mail all the same (`mailnote::unfiltered`).
#[derive(Debug, Default)]
pub(crate) struct Filtered {
    pub(crate) acted: usize,
    pub(crate) problem: String,
    pub(crate) tell: Vec<PathBuf>,
}

/// After a fetch of `account` (its inbox's arrivals, `new`; none when only
/// something waits for another look): the filters on the server, on a thread
/// that may wait for it. What a filter could not act on is to be told as any
/// new mail, the notifications having held it back for them (`untold`).
pub(crate) fn after_fetch(account: &Account, new: &[PathBuf], first: bool) -> Filtered {
    let config = load_config();
    if first || !config.mail.filters.iter().any(|f| f.runs_on(&account.id)) {
        return Filtered::default();
    }
    let Ok(password) = sioul_sync::secret::password(account) else {
        // Nothing could be done: what a filter would have taken quietly is told.
        let unread: Vec<PathBuf> = new.iter().filter(|f| !sioul_core::maildir::flags_of(f).contains('S')).cloned().collect();
        let tell = sioul_sync::filters::jobs(&config, account, &unread).into_iter().filter(|job| job.plan.quiet()).map(|job| job.file).collect();
        return Filtered { tell, ..Filtered::default() };
    };
    let done = sioul_sync::filters::after_fetch(&config, account, &password, new, first);
    if !done.failed.is_empty() {
        eprintln!("sioul: filters: {}: {:?}", account.id, done.failed.iter().map(|(_, f)| f).collect::<Vec<_>>());
    }
    Filtered { acted: done.acted.len(), problem: failures(&done).unwrap_or_default(), tell: done.to_tell() }
}

/// New mail as the notifications first see it: without what a filter would
/// move out of the inbox, put in the junk or the trash, or mark read, held
/// back until this device's filters have run (docs/porch.md, "Notifications").
/// Then what they did is never told, nor what another device marked first and
/// does; what they could not act on is told as any new mail (`after_fetch`'s
/// `tell`, `mailnote::unfiltered`).
pub(crate) fn untold(config: &Config, arrivals: Vec<Triaged>, senders: &Senders) -> Vec<Triaged> {
    if config.mail.filters.is_empty() {
        return arrivals;
    }
    let quiet: Vec<PathBuf> = rules::plans(&config.mail.filters, &arrivals, senders).into_iter().filter(|(_, plan)| plan.quiet()).map(|(path, _)| path).collect();
    arrivals.into_iter().filter(|t| t.card.path.as_ref().is_none_or(|p| !quiet.contains(p))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editor_and_the_configuration_say_the_same() {
        let filter = Filter {
            name: "Bank".into(),
            accounts: vec!["home".into()],
            join: Join::Any,
            conditions: vec![Condition::new(Field::From, Test::Contains, "@bank.example"), Condition { field: Field::Hour, test: Test::Between, value: "22:00".into(), until: "06:00".into() }],
            actions: vec![Act::Move { folder: "Banque".into() }, Act::Read, Act::Keyword { name: "$label1".into() }],
            stop: true,
            ..Filter::default()
        };
        let edited = Edited::of(&filter, &Config::default());
        let json = serde_json::to_string(&edited).unwrap();
        let back: Edited = serde_json::from_str(&json).unwrap();
        assert_eq!(back.filter(), filter);
        assert!(back.said.is_empty() && !edited.said.is_empty(), "said, never read back");
        // What the editor does not fill reads as a new filter's.
        let fresh: Edited = serde_json::from_str("{}").unwrap();
        assert_eq!(fresh.filter(), Filter::default());
        let odd: Edited = serde_json::from_str(r#"{"conditions": [{"field": "frobnicate", "test": "contains", "value": "x"}], "actions": [{"do": "explode"}]}"#).unwrap();
        assert_eq!(odd.filter().problem(), Some("filter-problem-unknown"));
    }

    #[test]
    fn a_search_becomes_a_filter() {
        let ids = vec!["home".to_string(), "work".to_string()];
        // As MailSearch.qml gives it: the inbox and unread mail are what a filter reads anyway.
        let given: Given = serde_json::from_str(
            r#"{"conditions": [
                {"field": "anywhere", "test": "contains", "value": "facture", "until": ""},
                {"field": "account", "test": "is", "value": "home", "until": ""},
                {"field": "folder", "test": "is", "value": "inbox", "until": ""},
                {"field": "mark", "test": "is-not", "value": "read", "until": ""}
            ], "match": "all"}"#,
        )
        .unwrap();
        let (filter, left_out) = from_search(given, &ids);
        assert!(!left_out);
        assert_eq!(filter.accounts, vec!["home".to_string()]);
        assert_eq!(filter.conditions, vec![Condition::new(Field::Anywhere, Test::Contains, "facture")]);
        assert_eq!(filter.actions, vec![Act::Move { folder: String::new() }]);
        assert_eq!(filter.problem(), Some("filter-problem-folder"), "it waits for its folder");
        // Another folder and flagged mail are left out, and said; "is not" an address: the others.
        let given: Given = serde_json::from_str(
            r#"{"conditions": [
                {"field": "folder", "test": "is", "value": "archive"},
                {"field": "mark", "test": "is", "value": "flagged"},
                {"field": "account", "test": "is-not", "value": "home"}
            ], "match": "any"}"#,
        )
        .unwrap();
        let (filter, left_out) = from_search(given, &ids);
        assert!(left_out);
        assert_eq!((filter.accounts.clone(), filter.join), (vec!["work".to_string()], Join::Any));
        assert_eq!(filter.conditions, vec![Condition::new(Field::From, Test::Contains, "")], "one condition to fill");
        // Two addresses at odds: every address, said.
        let given: Given = serde_json::from_str(r#"{"conditions": [{"field": "account", "test": "is", "value": "home"}, {"field": "account", "test": "is", "value": "work"}, {"field": "from", "test": "contains", "value": "@bank.example"}]}"#).unwrap();
        let (filter, left_out) = from_search(given, &ids);
        assert!(left_out && filter.accounts.is_empty());
        // A whole filter given keeps its name and actions.
        let given: Given = serde_json::from_str(r#"{"name": "Bank", "conditions": [{"field": "from", "test": "contains", "value": "@bank.example"}], "actions": [{"do": "move", "name": "Banque"}, {"do": "read"}]}"#).unwrap();
        let (filter, _) = from_search(given, &ids);
        assert_eq!((filter.name.as_str(), filter.actions.clone()), ("Bank", vec![Act::Move { folder: "Banque".into() }, Act::Read]));
        assert_eq!(filter.problem(), None);
    }
}
