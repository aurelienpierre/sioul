// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Searching the mail by conditions, for the Mail page (docs/client.md,
//! "Searching").
//!
//! The page sends its conditions (`sioul_core::mailsearch::Search`, as JSON);
//! the mail kept here is looked through at once, on every core, each message
//! read as little as its conditions need; then, after a pause in typing, the
//! servers are asked for what they hold and this device does not (folders
//! left on the server, older mail still waiting for room: `fetch::kept`),
//! one connection per address (`sioul_sync::search`). Both parts merge into
//! one list, newest first, without twins: a message kept here is never shown
//! again from its server (same address, folder, UIDVALIDITY and UID), nor a
//! copy of it in another folder of the same address (same Message-ID: Gmail's
//! labels). The page reads `mailSearch`: the sentence, what the servers said,
//! the rows. A row seen on its server only is brought here when opened.

use crate::backend::{QtThread, Shared, json, load_config, offline, say, tell, tr};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use jiff::Zoned;
use serde::Serialize;
use sioul_core::card::ImapOrigin;
use sioul_core::config::{Account, Config};
use sioul_core::folders::{Folder, Role};
use sioul_core::mailsearch::{self, Found, Names, Search, ServerRef, Stored};
use sioul_core::porch::Senders;
use sioul_core::{maildir, rules, view};
use sioul_sync::fetch::Kept;
use sioul_sync::{mailbox, secret};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Rows shown at most: past them, the sentence says how many more.
const SHOWN: usize = 500;
/// Messages asked of each server at most, the newest first.
const PER_SERVER: usize = 200;
/// The servers are asked after this pause in typing (Enter asks at once).
const PAUSE: Duration = Duration::from_millis(900);

/// One message found, here or on its server, as the list shows it and as the merge compares it.
#[derive(Clone)]
struct Hit {
    date: i64,
    account: String,
    folder: String,
    role: Role,
    origin: Option<ImapOrigin>,
    message_id: Option<String>,
    row: Row,
}

/// A row of the list: the folder's own (`view::MailItem`), and where it is.
#[derive(Clone, Serialize)]
struct Row {
    #[serde(flatten)]
    item: view::MailItem,
    /// "Inbox · noa@example.com".
    place: String,
    account: String,
    folder: String,
    role: Role,
    /// Seen on its server only: opened, it is brought here.
    server: bool,
}

/// What an address's server said.
#[derive(Clone)]
enum ServerPart {
    Looking,
    Found { hits: Vec<Hit>, more: usize },
    Failed(String),
}

#[derive(Default)]
struct State {
    /// A search is open on the page.
    active: bool,
    search: Search,
    generation: u64,
    /// What the mail kept here gave, the newest first; how many in all.
    local: Vec<Hit>,
    local_total: usize,
    /// Still looking here.
    scanning: bool,
    server: BTreeMap<String, ServerPart>,
    /// A message brought from its server: its key then, its file now.
    brought: Option<(String, String)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// The search being made: an older one gives up as soon as it sees a newer one.
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// A rescan of the mail kept here is running (after files moved).
static RESCANNING: AtomicBool = AtomicBool::new(false);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut guard = STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(guard.get_or_insert_with(State::default))
}

fn current(generation: u64) -> bool {
    GENERATION.load(Ordering::SeqCst) == generation
}

/// A folder's name as the list says it: its purpose, else its own name
/// (Gmail's prefix aside); Gmail's "All Mail" is its archive when it has none.
fn folder_title(folder: &Folder, all: &[Folder]) -> String {
    let has_archive = all.iter().any(|f| f.role == Role::Archive);
    match folder.role {
        Role::All if !has_archive => tr().text(Role::Archive.message_id(), None),
        Role::Other | Role::All => ["[Gmail]/", "[Google Mail]/"].iter().fold(folder.display.clone(), |name, prefix| name.strip_prefix(prefix).map_or(name.clone(), str::to_string)),
        role => tr().text(role.message_id(), None),
    }
}

fn account_title(account: &Account) -> String {
    account.address.clone().unwrap_or_else(|| account.id.clone())
}

/// The folders a search looks in, of an account: those a list shows (Gmail's
/// "All Mail" only where it stands for the archive), those it can reach.
fn searched(search: &Search, account: &Account, folders: &[Folder]) -> Vec<Folder> {
    let has_archive = folders.iter().any(|f| f.role == Role::Archive);
    folders.iter().filter(|f| (f.role != Role::All || !has_archive) && search.reaches(&account.id, f)).cloned().collect()
}

/// A folder kept here (the inbox always is), as a folder of its own: one name under the account's Maildir.
fn kept_dir(account: &Account, folder: &Folder) -> Option<PathBuf> {
    let kept = folder.role == Role::Inbox || !account.skip_folders.contains(&folder.name);
    let mut parts = Path::new(&folder.local).components();
    let own = matches!((parts.next(), parts.next()), (None, None) | (Some(std::path::Component::Normal(_)), None));
    (kept && own).then(|| account.maildir_path().join(&folder.local))
}

/// Which messages the trust marks believe, per account (as the folder's list).
fn trusted(config: &Config) -> BTreeMap<String, Vec<String>> {
    config.mail_sources().into_iter().filter_map(|s| Some((s.account?, s.trusted_ids))).collect()
}

/// The rank of a folder when one message is in two (Gmail's labels): the one you sort from first.
fn rank(role: Role) -> u8 {
    match role {
        Role::Inbox => 0,
        Role::Other => 1,
        Role::Archive => 2,
        Role::Sent | Role::Drafts => 3,
        Role::Junk | Role::Trash => 4,
        Role::All => 5,
    }
}

/// The newest first; a message twice in one address (its Message-ID) once, from the folder that ranks first.
fn merged(mut hits: Vec<Hit>) -> Vec<Hit> {
    hits.sort_by_key(|h| (std::cmp::Reverse(h.date), rank(h.role)));
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut by_rank: BTreeMap<(String, String), u8> = BTreeMap::new();
    for hit in &hits {
        if let Some(id) = &hit.message_id {
            let best = by_rank.entry((hit.account.clone(), id.clone())).or_insert(u8::MAX);
            *best = (*best).min(rank(hit.role));
        }
    }
    hits.retain(|hit| match &hit.message_id {
        Some(id) => {
            let key = (hit.account.clone(), id.clone());
            by_rank.get(&key) == Some(&rank(hit.role)) && seen.insert(key)
        }
        None => true,
    });
    hits
}

/// Who a sender is to you, when a condition asks it (`porch::Senders`).
fn senders(search: &Search, config: &Config) -> Option<Senders> {
    search.needs_senders().then(|| Senders::load(config))
}

/// Looks through the mail kept here, on every core. None when a newer search came first.
fn scan(search: &Search, generation: u64, hidden: &BTreeSet<PathBuf>) -> Option<(Vec<Hit>, usize)> {
    let config = load_config();
    let judge = senders(search, &config);
    let mut candidates: Vec<(PathBuf, Arc<(String, Folder)>)> = Vec::new();
    for account in config.accounts.iter().filter(|a| a.syncs()) {
        let folders = mailbox::folders(&account.id);
        for folder in searched(search, account, &folders) {
            let Some(dir) = kept_dir(account, &folder) else { continue };
            let place = Arc::new((account.id.clone(), folder));
            for sub in ["cur", "new"] {
                for entry in std::fs::read_dir(dir.join(sub)).into_iter().flatten().filter_map(Result::ok) {
                    let path = entry.path();
                    let hidden_file = path.file_name().and_then(|n| n.to_str()).is_none_or(|n| n.starts_with('.'));
                    if !hidden_file && !hidden.contains(&path) {
                        candidates.push((path, Arc::clone(&place)));
                    }
                }
            }
        }
    }
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 8);
    let chunk = candidates.len().div_ceil(workers).max(1);
    let stopped = AtomicBool::new(false);
    let found: Vec<(i64, PathBuf, Arc<(String, Folder)>, Option<String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = candidates
            .chunks(chunk)
            .map(|part| {
                let (stopped, judge) = (&stopped, judge.as_ref());
                scope.spawn(move || {
                    let mut out = Vec::new();
                    for (i, (path, place)) in part.iter().enumerate() {
                        if i % 64 == 0 && (!current(generation) || stopped.load(Ordering::Relaxed)) {
                            stopped.store(true, Ordering::Relaxed);
                            return out;
                        }
                        let stored = Stored::new(path.clone(), &place.0, &place.1);
                        if stored.matches(search, judge) {
                            out.push((stored.date().unwrap_or(0), path.clone(), Arc::clone(place), stored.message_id()));
                        }
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    if stopped.load(Ordering::Relaxed) || !current(generation) {
        return None;
    }
    let trusted = trusted(&config);
    let titles: BTreeMap<String, (String, Vec<Folder>)> = config.accounts.iter().filter(|a| a.syncs()).map(|a| (a.id.clone(), (account_title(a), mailbox::folders(&a.id)))).collect();
    // Twins aside first, then the newest made into rows.
    let light: Vec<Hit> = found
        .into_iter()
        .map(|(date, path, place, message_id)| Hit {
            date,
            account: place.0.clone(),
            folder: place.1.name.clone(),
            role: place.1.role,
            origin: maildir::origin_of(&path),
            message_id,
            row: Row { item: empty_item(&path), place: String::new(), account: place.0.clone(), folder: place.1.name.clone(), role: place.1.role, server: false },
        })
        .collect();
    let all = merged(light);
    let total = all.len();
    let hits = all
        .into_iter()
        .take(SHOWN)
        .filter_map(|mut hit| {
            let path = PathBuf::from(&hit.row.item.key);
            let card = maildir::read_one(&path)?;
            let (account, folders) = titles.get(&hit.account)?;
            let folder = folders.iter().find(|f| f.name == hit.folder)?;
            let ids = trusted.get(&hit.account).map(Vec::as_slice).unwrap_or_default();
            hit.row.item = view::mail_item(&card, folder.role, false, &maildir::flags_of(&path), ids, tr());
            hit.row.place = say("search-place", &[("folder", folder_title(folder, folders)), ("account", account.clone())]);
            Some(hit)
        })
        .collect();
    Some((hits, total))
}

/// A row's place holder while the merge runs: its key alone.
fn empty_item(path: &Path) -> view::MailItem {
    view::MailItem {
        key: path.display().to_string(),
        who: String::new(),
        address: String::new(),
        subject: String::new(),
        date: String::new(),
        unread: false,
        flagged: false,
        answered: false,
        attachments: false,
        trust_level: "own",
        trust: String::new(),
        checks: String::new(),
        thread: String::new(),
        size: 1,
        member: false,
    }
}

/// A message its server found, as a row.
fn server_hit(found: &Found, account: &Account, folders: &[Folder], trusted: &[String]) -> Option<Hit> {
    let mut item = view::mail_item(&found.card, found.folder.role, false, &found.flags, trusted, tr());
    item.key = found.place().key();
    Some(Hit {
        date: found.date().unwrap_or(0),
        account: account.id.clone(),
        folder: found.folder.name.clone(),
        role: found.folder.role,
        origin: Some(found.origin),
        message_id: found.message_id(),
        row: Row {
            item,
            place: format!("{} · {}", say("search-place", &[("folder", folder_title(&found.folder, folders)), ("account", account_title(account))]), tr().text("search-on-server", None)),
            account: account.id.clone(),
            folder: found.folder.name.clone(),
            role: found.folder.role,
            server: true,
        },
    })
}

/// Whether a folder's server must be asked: left on the server, or not all
/// of it kept here for the days searched. The server counts the days of its
/// fetch by its own clock: a search reaching within a day of them asks it too.
fn needs_server(account: &Account, folder: &Folder, earliest: Option<i64>) -> bool {
    if kept_dir(account, folder).is_none() {
        return true;
    }
    match sioul_sync::fetch::kept(account, &folder.name) {
        Kept::Everything => false,
        Kept::Since(since) => earliest.is_none_or(|first| first < since + 86_400),
        Kept::Unknown => true,
    }
}

/// Asks each address's server, each on a thread of its own; their answers
/// show as they come.
fn ask_servers(qt: &QtThread, shared: &Arc<Shared>, search: &Search, generation: u64) {
    if offline() {
        return;
    }
    let config = load_config();
    let earliest = mailsearch::earliest(search).and_then(|d| d.to_zoned(jiff::tz::TimeZone::system()).ok()).map(|z| z.timestamp().as_second());
    let trusted = trusted(&config);
    let judge = senders(search, &config).map(Arc::new);
    for account in config.accounts.into_iter().filter(Account::syncs) {
        let all = mailbox::folders(&account.id);
        let folders: Vec<Folder> = searched(search, &account, &all).into_iter().filter(|f| needs_server(&account, f, earliest)).collect();
        if folders.is_empty() {
            continue;
        }
        if !with_state(|s| {
            let fresh = s.generation == generation;
            if fresh {
                s.server.insert(account.id.clone(), ServerPart::Looking);
            }
            fresh
        }) {
            return;
        }
        let (qt, shared, search) = (qt.clone(), Arc::clone(shared), search.clone());
        let ids = trusted.get(&account.id).cloned().unwrap_or_default();
        let judge = judge.clone();
        std::thread::spawn(move || {
            let here: BTreeMap<String, BTreeSet<(u32, u32)>> = folders.iter().map(|f| (f.name.clone(), sioul_sync::search::kept_uids(&account, f))).collect();
            let answer = secret::password(&account).and_then(|p| sioul_sync::search::search(&account, &p, &search, &folders, &here, PER_SERVER, judge.as_deref()));
            let part = match answer {
                Ok(outcome) => ServerPart::Found { hits: outcome.found.iter().filter_map(|f| server_hit(f, &account, &all, &ids)).collect(), more: outcome.more },
                Err(e) => ServerPart::Failed(e.sentence(tr(), &account_title(&account))),
            };
            with_state(|s| {
                if s.generation == generation {
                    s.server.insert(account.id.clone(), part);
                }
            });
            publish(&qt, &shared);
        });
    }
    publish(qt, shared);
}

/// What the page shows of the search.
#[derive(Serialize)]
struct SearchView {
    active: bool,
    sentence: String,
    /// What was found, here and on the servers, in words.
    status: String,
    /// Still looking, here or on a server.
    busy: bool,
    items: Vec<Row>,
    /// A message brought from its server: the page opens its file.
    brought: Option<Brought>,
}

#[derive(Serialize)]
struct Brought {
    from: String,
    to: String,
}

/// The sentence's names for addresses and folders.
fn names_sentence(search: &Search) -> String {
    let config = load_config();
    let account = |id: &str| config.account(id).map_or_else(|| id.to_string(), account_title);
    let folder = |value: &str| match rules::ROLES.iter().find(|(id, _)| *id == value) {
        Some((_, role)) => tr().text(role.message_id(), None),
        None => value.to_string(),
    };
    mailsearch::sentence(search, tr(), &Names { account: &account, folder: &folder }, Zoned::now().date())
}

/// The view, from what is known now: what waits ten seconds for "Undo" left out.
fn view_now(shared: &Shared) -> SearchView {
    let hidden = crate::mail::hidden_files(shared);
    let hidden_keys = crate::mail::hidden_keys(shared);
    with_state(|s| {
        if !s.active {
            return SearchView { active: false, sentence: String::new(), status: String::new(), busy: false, items: Vec::new(), brought: None };
        }
        let mut hits: Vec<Hit> = s.local.iter().filter(|h| !hidden.contains(Path::new(&h.row.item.key))).cloned().collect();
        // Those waiting ten seconds to leave are not counted either.
        let here_count = s.local_total.saturating_sub(s.local.len() - hits.len());
        // Kept here: never again from the server.
        let here: BTreeSet<(String, String, ImapOrigin)> = hits.iter().filter_map(|h| Some((h.account.clone(), h.folder.clone(), h.origin?))).collect();
        let (mut looking, mut found_there, mut more_there, mut failures) = (false, 0usize, 0usize, Vec::new());
        for part in s.server.values() {
            match part {
                ServerPart::Looking => looking = true,
                ServerPart::Found { hits: there, more } => {
                    let fresh: Vec<Hit> = there.iter().filter(|h| !hidden_keys.contains(&h.row.item.key) && h.origin.is_none_or(|o| !here.contains(&(h.account.clone(), h.folder.clone(), o)))).cloned().collect();
                    found_there += fresh.len();
                    more_there += more;
                    hits.extend(fresh);
                }
                ServerPart::Failed(why) => failures.push(why.clone()),
            }
        }
        let before = hits.len();
        let hits = merged(hits);
        // Twins of what is here, found again on a server, are not news.
        found_there = found_there.saturating_sub(before - hits.len());
        let empty = s.search.is_empty();
        let mut status: Vec<String> = Vec::new();
        if !empty && !s.scanning {
            let asked = !s.server.is_empty();
            status.push(say(if asked { "search-found" } else { "search-found-all" }, &[("n", here_count.to_string())]));
            if here_count > SHOWN {
                status.push(say("search-found-more", &[("shown", SHOWN.to_string()), ("n", here_count.to_string())]));
            }
            if looking {
                status.push(tr().text("search-server-looking", None));
            } else if asked && failures.len() < s.server.len() {
                status.push(if found_there > 0 { say("search-server-found", &[("n", found_there.to_string())]) } else { tr().text("search-server-none", None) });
            }
            if more_there > 0 {
                status.push(say("search-server-more", &[("n", more_there.to_string())]));
            }
            status.extend(failures);
        }
        SearchView {
            active: true,
            sentence: names_sentence(&s.search),
            status: status.join(" "),
            busy: s.scanning || looking,
            items: hits.into_iter().map(|h| h.row).collect(),
            brought: s.brought.clone().map(|(from, to)| Brought { from, to }),
        }
    })
}

/// What the page was last given: the same again is not sent (the list would
/// be laid out anew, its scrolling lost, at every fetch).
static PUBLISHED: Mutex<String> = Mutex::new(String::new());

/// Shows the search on the page, when it changed.
fn publish(qt: &QtThread, shared: &Arc<Shared>) {
    let text = json(&view_now(shared));
    {
        let mut last = PUBLISHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if *last == text {
            return;
        }
        last.clone_from(&text);
    }
    let _ = qt.queue(move |mut sioul| sioul.as_mut().set_mail_search(QString::from(&text)));
}

/// A new search from the page: the mail here at once, the servers after a
/// pause in typing (`now`: at once, Enter was pressed).
pub(crate) fn search(qt: &QtThread, shared: &Arc<Shared>, text: &str, now: bool) {
    let search: Search = serde_json::from_str(text).unwrap_or_default();
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let empty = search.is_empty();
    with_state(|s| {
        *s = State { active: true, search: search.clone(), generation, scanning: !empty, ..State::default() };
    });
    publish(qt, shared);
    if empty {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let hidden = crate::mail::hidden_files(&shared);
        if let Some((hits, total)) = scan(&search, generation, &hidden) {
            with_state(|s| {
                if s.generation == generation {
                    s.local = hits;
                    s.local_total = total;
                    s.scanning = false;
                }
            });
            publish(&qt, &shared);
        }
        if !now {
            std::thread::sleep(PAUSE);
        }
        if current(generation) {
            ask_servers(&qt, &shared, &search, generation);
        }
    });
}

/// The search closed: the page shows its folder again.
pub(crate) fn clear(qt: &QtThread, shared: &Arc<Shared>) {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    with_state(|s| *s = State::default());
    publish(qt, shared);
}

/// The mail changed (a move done or undone, a fetch): the search shows it.
/// A message whose file is gone (moved, renamed by a sync) has the mail kept
/// here looked through again, without asking the servers again.
pub(crate) fn changed(qt: &QtThread, shared: &Arc<Shared>) {
    let (active, gone, search, generation) = with_state(|s| (s.active, s.local.iter().any(|h| !Path::new(&h.row.item.key).exists()), s.search.clone(), s.generation));
    if !active {
        return;
    }
    publish(qt, shared);
    if !gone || RESCANNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        // Files move in batches: the rescan waits for them to settle.
        std::thread::sleep(Duration::from_millis(400));
        let hidden = crate::mail::hidden_files(&shared);
        if let Some((hits, total)) = scan(&search, generation, &hidden) {
            with_state(|s| {
                if s.generation == generation {
                    s.local = hits;
                    s.local_total = total;
                }
            });
        }
        RESCANNING.store(false, Ordering::SeqCst);
        publish(&qt, &shared);
    });
}

/// Messages acted on from their server (moved, archived, deleted): out of the list.
pub(crate) fn forget(keys: &[String]) {
    with_state(|s| {
        for part in s.server.values_mut() {
            if let ServerPart::Found { hits, .. } = part {
                hits.retain(|h| !keys.contains(&h.row.item.key));
            }
        }
    });
}

/// A server row's marks changed (read, flagged): the row says it.
pub(crate) fn marked(keys: &[String], letters: impl Fn(&str) -> String) {
    with_state(|s| {
        for part in s.server.values_mut() {
            if let ServerPart::Found { hits, .. } = part {
                for hit in hits.iter_mut().filter(|h| keys.contains(&h.row.item.key)) {
                    let mut flags = String::new();
                    flags.push_str(if hit.row.item.unread { "" } else { "S" });
                    flags.push_str(if hit.row.item.flagged { "F" } else { "" });
                    let after = letters(&flags);
                    hit.row.item.unread = matches!(hit.role, Role::Inbox | Role::Other | Role::Archive) && !after.contains('S');
                    hit.row.item.flagged = after.contains('F');
                }
            }
        }
    });
}

/// Brings a message seen on its server only here, then has the page open it.
pub(crate) fn bring(qt: &QtThread, shared: &Arc<Shared>, key: &str) {
    let Some(place) = ServerRef::parse(key) else { return };
    let config = load_config();
    let Some(account) = config.account(&place.account).filter(|a| a.syncs()).cloned() else {
        tell(qt, shared, say("account-unknown", &[("id", place.account.clone())]));
        return;
    };
    crate::backend::set_status(qt, tr().text("search-bringing", None));
    let trusted = trusted(&config).remove(&account.id).unwrap_or_default();
    let (qt, shared, key) = (qt.clone(), Arc::clone(shared), key.to_string());
    std::thread::spawn(move || {
        let brought = secret::password(&account).and_then(|p| sioul_sync::search::bring(&account, &p, &place));
        match brought {
            Ok(path) => {
                let folders = mailbox::folders(&account.id);
                let row = maildir::read_one(&path).and_then(|card| {
                    let folder = folders.iter().find(|f| f.name == place.folder)?;
                    let mut item = view::mail_item(&card, folder.role, false, &maildir::flags_of(&path), &trusted, tr());
                    item.key = path.display().to_string();
                    Some(Hit {
                        date: card.date.unwrap_or(0),
                        account: account.id.clone(),
                        folder: folder.name.clone(),
                        role: folder.role,
                        origin: maildir::origin_of(&path),
                        message_id: card.message_id.as_deref().map(|id| id.trim_matches(['<', '>']).to_string()),
                        row: Row { item, place: say("search-place", &[("folder", folder_title(folder, &folders)), ("account", account_title(&account))]), account: account.id.clone(), folder: folder.name.clone(), role: folder.role, server: false },
                    })
                });
                with_state(|s| {
                    // Kept here now: a row of the mail here, in its place.
                    s.local.extend(row);
                    s.local.sort_by_key(|h| std::cmp::Reverse(h.date));
                    s.local_total += 1;
                    s.brought = Some((key.clone(), path.display().to_string()));
                });
                crate::backend::set_status(&qt, String::new());
                publish(&qt, &shared);
                with_state(|s| s.brought = None);
            }
            Err(e) => tell(&qt, &shared, e.sentence(tr(), &account_title(&account))),
        }
    });
}

#[derive(Serialize)]
struct Fields {
    #[serde(flatten)]
    words: mailsearch::Form,
    accounts: Vec<rules::Choice>,
    /// The purposes first (every address's inbox…), then each folder of yours, by name.
    folders: Vec<rules::Choice>,
    help: String,
}

/// What the search's column offers, in your language, with your addresses and folders.
fn fields() -> String {
    let config = load_config();
    let words = mailsearch::form(tr());
    let accounts: Vec<rules::Choice> = config.accounts.iter().filter(|a| a.syncs()).map(|a| rules::Choice { id: a.id.clone(), label: account_title(a) }).collect();
    let mut folders = words.roles.clone();
    let mut named: BTreeMap<String, String> = BTreeMap::new();
    for account in config.accounts.iter().filter(|a| a.syncs()) {
        let all = mailbox::folders(&account.id);
        for folder in all.iter().filter(|f| f.role == Role::Other) {
            named.entry(folder.name.clone()).or_insert_with(|| folder_title(folder, &all));
        }
    }
    let mut others: Vec<rules::Choice> = named.into_iter().map(|(id, label)| rules::Choice { id, label }).collect();
    others.sort_by_key(|c| c.label.to_lowercase());
    folders.extend(others);
    json(&Fields { words, accounts, folders, help: tr().text("search-help", None) })
}

impl crate::backend::qobject::Sioul {
    pub(crate) fn search_mail(self: Pin<&mut Self>, conditions: &QString, now: bool) {
        search(&self.qt_thread(), &self.shared(), &conditions.to_string(), now);
    }

    pub(crate) fn clear_mail_search(self: Pin<&mut Self>) {
        clear(&self.qt_thread(), &self.shared());
    }

    pub(crate) fn bring_message(self: Pin<&mut Self>, key: &QString) {
        bring(&self.qt_thread(), &self.shared(), &key.to_string());
    }

    pub(crate) fn mail_search_fields(&self) -> QString {
        QString::from(&fields())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(account: &str, folder: &str, role: Role, date: i64, id: Option<&str>) -> Hit {
        let path = PathBuf::from(format!("/m/{account}/{folder}/{date}"));
        Hit {
            date,
            account: account.into(),
            folder: folder.into(),
            role,
            origin: None,
            message_id: id.map(str::to_string),
            row: Row { item: empty_item(&path), place: String::new(), account: account.into(), folder: folder.into(), role, server: false },
        }
    }

    #[test]
    fn twins_once_from_the_folder_you_sort_from() {
        // Gmail: one message in the inbox and in All Mail; another address has its own copy.
        let hits = vec![
            hit("gmail", "[Gmail]/All Mail", Role::All, 30, Some("a@x")),
            hit("gmail", "INBOX", Role::Inbox, 30, Some("a@x")),
            hit("home", "INBOX", Role::Inbox, 30, Some("a@x")),
            hit("gmail", "INBOX", Role::Inbox, 50, None),
            hit("gmail", "Sent", Role::Sent, 10, Some("b@x")),
        ];
        let kept: Vec<(String, String, i64)> = merged(hits).into_iter().map(|h| (h.account, h.folder, h.date)).collect();
        assert_eq!(
            kept,
            vec![("gmail".into(), "INBOX".into(), 50), ("gmail".into(), "INBOX".into(), 30), ("home".into(), "INBOX".into(), 30), ("gmail".into(), "Sent".into(), 10)]
        );
    }
}
