// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail's topics in the window (docs/topics.md): giving messages a topic
//! ("Topic…" on a message, on several selected at once, "+ Topic" from the
//! reader, which makes the topic and gives it in one act), a sender's rule
//! ("Always <topic> for this sender", "Never…"), and the settings' page,
//! Mail ▸ ⚙ ▸ Topics: the list, each topic's seeds in words and its senders'
//! rules, Rename and Remove, and on a computer the groups Sioul noticed
//! (`sioul_learn::groups`, NMF), each with "Name it", which makes a topic and
//! gives it to the whole group.
//!
//! Each act is written at once in this device's log of your word
//! (`topics::labels`), which travels to your other devices, and said in one
//! sentence with Undo (`mail::offer_back`), which writes your word on each
//! message back as it was. Nothing asks you to tag anything.

use crate::backend::{QtThread, Shared, config_path, load_config, say, set_status, tr};
use serde::Serialize;
use sioul_core::topics::labels::{self, Act, Rule, Said};
use sioul_core::topics::{self, Seeds, Topic};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// This device's own log of your word on topics.
fn log() -> PathBuf {
    labels::own_log(&labels::state(), &sioul_sync::share::Here::load(&sioul_core::config::state_dir()).id)
}

fn now() -> i64 {
    jiff::Timestamp::now().as_second()
}

/// The bare Message-ID of the message a key names (a file of the mailbox or a folder).
fn message_id(key: &str) -> Option<String> {
    let (_, path) = crate::mail::locate(key)?;
    let card = sioul_core::maildir::read_one(&path)?;
    card.message_id.map(|id| sioul_core::mailindex::bare_id(&id)).filter(|id| !id.is_empty())
}

/// The messages' ids, each row once (a row's copies are one message).
fn ids_of(rows: &[Vec<String>]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for row in rows {
        if let Some(id) = row.iter().find_map(|key| message_id(key))
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    ids
}

fn topic_named(config: &sioul_core::config::Config, id: &str) -> Option<Topic> {
    config.topics.iter().find(|t| t.id == id).cloned()
}

/// Writes acts in this device's log; what went wrong, said.
fn write(qt: &QtThread, acts: &[Act]) -> bool {
    match labels::append(&log(), acts) {
        Ok(()) => true,
        Err(e) => {
            set_status(qt, say("topics-not-written", &[("detail", e)]));
            false
        }
    }
}

/// Offers Undo for acts written already: `undo` writes your word back as it
/// was, and `then` puts back what else the act changed (a topic made).
fn offer(qt: &QtThread, shared: &Arc<Shared>, line: String, undo: Vec<Act>, then: impl Fn() -> String + Send + Sync + 'static) {
    let undo = Mutex::new(undo);
    crate::mail::offer_back(qt, shared, line, move |qt, shared| {
        // The Undo's lines come after the act's, whatever the clock did meanwhile.
        let at = now() + 1;
        let acts: Vec<Act> = undo.lock().map(|u| u.clone()).unwrap_or_default().into_iter().map(|act| at_time(act, at)).collect();
        let problem = labels::append(&log(), &acts).err().unwrap_or_default();
        let also = then();
        crate::mail::show_mail(qt, shared);
        if problem.is_empty() { also } else { problem }
    });
    crate::mail::show_mail(qt, shared);
}

fn at_time(act: Act, at: i64) -> Act {
    match act {
        Act::Message { message_id, topic, forget, .. } => Act::Message { at, message_id, topic, forget },
        Act::Sender { address, topic, rule, .. } => Act::Sender { at, address, topic, rule },
    }
}

/// Gives the messages of `rows` (a mailbox's or a folder's keys) the topic
/// `id`, or none (`""`: taken off), said in one sentence with Undo.
pub(crate) fn tag(qt: &QtThread, shared: &Arc<Shared>, rows: &[Vec<String>], id: &str) {
    let config = load_config();
    let topic = (!id.is_empty()).then(|| topic_named(&config, id)).flatten();
    if !id.is_empty() && topic.is_none() {
        return set_status(qt, tr().text("topics-gone", None));
    }
    let ids = ids_of(rows);
    if ids.is_empty() {
        return set_status(qt, tr().text("topics-no-id", None));
    }
    let undo = Said::load().restoring(&ids, now());
    if !write(qt, &labels::tagging(&ids, topic.as_ref().map(|t| t.id.as_str()), now())) {
        return;
    }
    let line = match &topic {
        Some(topic) => say("topics-given", &[("topic", topic.name.clone()), ("n", ids.len().to_string())]),
        None => say("topics-taken-off", &[("n", ids.len().to_string())]),
    };
    offer(qt, shared, line, undo, String::new);
}

/// "+ Topic" (or "New topic…"): a topic made from `name`, and the messages of
/// `rows` given it, in one act; Undo takes both back. Returns what went wrong, else "".
pub(crate) fn new_topic(qt: &QtThread, shared: &Arc<Shared>, rows: &[Vec<String>], name: &str) -> String {
    let ids = ids_of(rows);
    give_new(qt, shared, &ids, name, Seeds::default())
}

/// A topic made and given to these messages, in one act, with Undo.
fn give_new(qt: &QtThread, shared: &Arc<Shared>, ids: &[String], name: &str, seeds: Seeds) -> String {
    let path = config_path();
    let topic = match topics::add(&path, name, &seeds) {
        Ok(topic) => topic,
        Err(e) => return e,
    };
    let undo = Said::load().restoring(ids, now());
    if !ids.is_empty() && !write(qt, &labels::tagging(ids, Some(&topic.id), now())) {
        let _ = topics::remove(&path, &topic.id);
        return String::new();
    }
    let line = if ids.is_empty() { say("topics-made", &[("topic", topic.name.clone())]) } else { say("topics-made-given", &[("topic", topic.name.clone()), ("n", ids.len().to_string())]) };
    let id = topic.id.clone();
    offer(qt, shared, line, undo, move || topics::remove(&config_path(), &id).err().unwrap_or_default());
    String::new()
}

/// "Always <topic> for this sender" (`rule` "always"), "Never…" ("never"),
/// or the rule taken off ("off"), for the sender of the message `key`.
pub(crate) fn sender_rule(qt: &QtThread, shared: &Arc<Shared>, key: &str, id: &str, rule: &str) {
    let rule = match rule {
        "always" => Rule::Always,
        "never" => Rule::Never,
        _ => Rule::Off,
    };
    let config = load_config();
    let Some(topic) = topic_named(&config, id) else { return set_status(qt, tr().text("topics-gone", None)) };
    let Some(address) = crate::mail::locate(key).and_then(|(_, path)| sioul_core::maildir::read_one(&path)).and_then(|c| c.from_address).map(|a| a.trim().to_lowercase()) else {
        return set_status(qt, tr().text("mail-message-gone", None));
    };
    set_rule(qt, shared, &address, &topic, rule);
}

/// A sender's rule written, said with Undo (the settings' × too).
pub(crate) fn set_rule(qt: &QtThread, shared: &Arc<Shared>, address: &str, topic: &Topic, rule: Rule) {
    let undo = vec![Said::load().restoring_rule(address, &topic.id, now())];
    if !write(qt, &[Act::Sender { at: now(), address: address.to_string(), topic: topic.id.clone(), rule }]) {
        return;
    }
    let pairs = [("topic", topic.name.clone()), ("address", address.to_string())];
    let line = match rule {
        Rule::Always => say("topics-rule-always", &pairs),
        Rule::Never => say("topics-rule-never", &pairs),
        Rule::Off => say("topics-rule-off", &pairs),
    };
    offer(qt, shared, line, undo, String::new);
}

/// A topic renamed, said with Undo. Returns what went wrong, else "".
pub(crate) fn rename(qt: &QtThread, shared: &Arc<Shared>, id: &str, name: &str) -> String {
    let Some(before) = topic_named(&load_config(), id) else { return tr().text("topics-gone", None) };
    if let Err(e) = topics::rename(&config_path(), id, name) {
        return e;
    }
    let (id, old) = (id.to_string(), before.name.clone());
    offer(qt, shared, say("topics-renamed", &[("old", before.name), ("topic", name.trim().to_string())]), Vec::new(), move || topics::rename(&config_path(), &id, &old).err().unwrap_or_default());
    String::new()
}

/// A topic removed, its word taken off every message; your words stay in
/// the logs, so that Undo puts it back as it was.
pub(crate) fn remove(qt: &QtThread, shared: &Arc<Shared>, id: &str) {
    let Some(before) = topic_named(&load_config(), id) else { return set_status(qt, tr().text("topics-gone", None)) };
    if let Err(e) = topics::remove(&config_path(), id) {
        return set_status(qt, e);
    }
    let line = say("topics-removed", &[("topic", before.name.clone())]);
    offer(qt, shared, line, Vec::new(), move || topics::put(&config_path(), &before).err().unwrap_or_default());
}

/// One topic as the settings' page lists it.
#[derive(Serialize)]
struct TopicRow {
    id: String,
    name: String,
    /// Its seeds in words: "From the folder Chorale and your contacts' category Chorale."; "" for none.
    seeds: String,
    /// Its senders' rules: (address, "always" | "never").
    senders: Vec<(String, &'static str)>,
    /// In the topics' table: Sioul also sorts mail like its own; else your words and its rules alone, until it has more mail.
    learned: bool,
}

/// The topics and their senders' rules, as the settings' page and the menus show them (JSON).
pub(crate) fn view() -> String {
    let config = load_config();
    let said = Said::load();
    let table = topics::judging::cached(&topics::judging::table_path()).map(|(t, _)| t);
    let rows: Vec<TopicRow> = config
        .topics
        .iter()
        .map(|topic| {
            let mut senders: Vec<(String, &'static str)> = said
                .senders
                .iter()
                .flat_map(|(address, rules)| rules.iter().filter(|(t, _)| *t == topic.id).map(move |(_, r)| (address.clone(), if *r == Rule::Always { "always" } else { "never" })))
                .collect();
            senders.sort();
            let learned = table.as_ref().is_some_and(|t| t.meta.topics.contains(&topic.id));
            TopicRow { id: topic.id.clone(), name: topic.name.clone(), seeds: seeds_words(&topic.seeds), senders, learned }
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

/// A topic's seeds in one sentence.
fn seeds_words(seeds: &Seeds) -> String {
    let mut parts: Vec<String> = seeds.folders.iter().map(|f| say("topics-seed-folder-of", &[("name", f.clone())])).collect();
    parts.extend(seeds.categories.iter().map(|c| say("topics-seed-category-of", &[("name", c.clone())])));
    if parts.is_empty() {
        return String::new();
    }
    let and = tr().text("topics-and", None);
    let joined = match parts.len() {
        1 => parts.remove(0),
        _ => {
            let last = parts.pop().unwrap_or_default();
            format!("{} {and} {last}", parts.join(", "))
        }
    };
    say("topics-seeds-line", &[("seeds", joined)])
}

// The groups Sioul noticed: on a computer only (sioul-learn).

/// The groups found last, each with its messages' ids, for "Name it".
static GROUPS: Mutex<Vec<sioul_learn_groups::Found>> = Mutex::new(Vec::new());
/// The senders offered last ("Always a topic…"), by address, in their order.
static SENDERS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[cfg(not(target_os = "android"))]
mod sioul_learn_groups {
    /// One group the page shows: its senders' names, its size, two subjects
    /// (masked), the name proposed and how it was found; its ids kept here.
    #[derive(Clone, serde::Serialize)]
    pub(crate) struct Found {
        pub size: u64,
        pub senders: u64,
        pub names: Vec<String>,
        pub subjects: Vec<String>,
        pub proposed: String,
        pub proposed_by: &'static str,
        #[serde(skip)]
        pub ids: Vec<String>,
    }
}

#[cfg(target_os = "android")]
mod sioul_learn_groups {
    #[derive(Clone, serde::Serialize)]
    pub(crate) struct Found {
        #[serde(skip)]
        pub ids: Vec<String>,
    }
}

/// Looks for groups of the mail kept here that no word of yours names (NMF on
/// their words: `sioul_learn::groups`), off the window's thread; `topicGroups`
/// says where it stands ({"state": "looking"}), then what it found
/// ({"state": "done", "groups": […]}). On a phone, none.
#[cfg(not(target_os = "android"))]
pub(crate) fn find_groups(qt: &QtThread) {
    let _ = qt.queue(|mut sioul| sioul.as_mut().set_topic_groups(cxx_qt_lib::QString::from("{\"state\":\"looking\"}")));
    let qt = qt.clone();
    std::thread::spawn(move || {
        let config = load_config();
        let dirs = sioul_learn::Dirs::standard();
        let options = sioul_learn::topics::Options::of(&config);
        let cancel = sioul_learn::Cancel::new();
        let found = sioul_learn::topics::read(&dirs, &sioul_learn::train::trusted_ids(&config), &sioul_learn::topics::Sources::of(&config), &options, true, None, &mut |_| {}, &cancel).and_then(|read| {
            let (seeded, labels) = sioul_learn::topics::seeded(&read, &options);
            let ids: Vec<String> = seeded.proposal.topics.iter().map(|t| t.id.clone()).collect();
            let said = Said::load();
            let language = config.language.clone().unwrap_or_else(sioul_core::i18n::system_language);
            let grouping = sioul_learn::groups::Options {
                window: true,
                leave_out: said.messages.keys().cloned().collect(),
                presets: Some(sioul_core::presets::load()),
                language: language.split(['-', '_']).next().unwrap_or("en").to_string(),
                topic_presets: Some(topics::presets::Matcher::current()),
                started: sioul_learn::groups::started(&config.topics, config.topics_day_one.is_some(), &read),
                ..sioul_learn::groups::Options::default()
            };
            let found = sioul_learn::groups::groups(&read, &labels, &ids, None, &grouping, &mut |_| {}, &cancel)?;
            Ok((sioul_learn::groups::noticed(&found), said))
        });
        let text = match found {
            Ok((noticed, said)) => {
                let found = |g: sioul_learn::groups::Group| sioul_learn_groups::Found { size: g.size, senders: g.senders, names: g.names, subjects: g.subjects, proposed: g.proposed, proposed_by: g.proposed_by, ids: g.ids };
                let shown: Vec<sioul_learn_groups::Found> = noticed.groups.into_iter().map(found).collect();
                // A sender with a rule of yours already is offered no more.
                let offered: Vec<(String, sioul_learn_groups::Found)> = noticed.senders.into_iter().filter_map(|g| Some((g.sole.clone()?, g))).filter(|(address, _)| said.senders.get(address).is_none_or(|rules| rules.iter().all(|(_, rule)| matches!(rule, Rule::Off)))).map(|(address, g)| (address, found(g))).collect();
                // Fewer than a thousand messages kept: no group yet ("Groups come once more mail is kept here"), the offers all the same.
                let text = serde_json::json!({ "state": "done", "waiting": noticed.waiting, "groups": shown, "senders": offered.iter().map(|(_, g)| g).collect::<Vec<_>>() }).to_string();
                if let Ok(mut kept) = GROUPS.lock() {
                    *kept = shown;
                }
                if let Ok(mut kept) = SENDERS.lock() {
                    *kept = offered.into_iter().map(|(address, _)| address).collect();
                }
                text
            }
            Err(e) => serde_json::json!({ "state": "failed", "detail": e.to_string() }).to_string(),
        };
        let _ = qt.queue(move |mut sioul| sioul.as_mut().set_topic_groups(cxx_qt_lib::QString::from(&text)));
    });
}

#[cfg(target_os = "android")]
pub(crate) fn find_groups(qt: &QtThread) {
    let _ = qt.queue(|mut sioul| sioul.as_mut().set_topic_groups(cxx_qt_lib::QString::from("{\"state\":\"done\",\"groups\":[],\"senders\":[]}")));
}

/// "Name it": the group at `index` of those found last becomes a topic named
/// `name`, every message of it given it, in one act, with Undo. Returns what went wrong, else "".
pub(crate) fn name_group(qt: &QtThread, shared: &Arc<Shared>, index: usize, name: &str) -> String {
    let Some(ids) = GROUPS.lock().ok().and_then(|g| g.get(index).map(|g| g.ids.clone())) else { return tr().text("topics-group-gone", None) };
    give_new(qt, shared, &ids, name, Seeds::default())
}

/// "Always a topic…": the sender offered at `index` of those found last
/// gets "Always <topic>" for the topic named `name` (made when no topic of
/// yours has that name), in one act, with Undo. Returns what went wrong, else "".
pub(crate) fn name_sender(qt: &QtThread, shared: &Arc<Shared>, index: usize, name: &str) -> String {
    let Some(address) = SENDERS.lock().ok().and_then(|s| s.get(index).cloned()) else { return tr().text("topics-group-gone", None) };
    let path = config_path();
    let (topic, made) = match load_config().topics.iter().find(|t| t.name.trim().to_lowercase() == name.trim().to_lowercase()) {
        Some(topic) => (topic.clone(), false),
        None => match topics::add(&path, name, &Seeds::default()) {
            Ok(topic) => (topic, true),
            Err(e) => return e,
        },
    };
    let undo = vec![Said::load().restoring_rule(&address, &topic.id, now())];
    if !write(qt, &[Act::Sender { at: now(), address: address.clone(), topic: topic.id.clone(), rule: Rule::Always }]) {
        if made {
            let _ = topics::remove(&path, &topic.id);
        }
        return String::new();
    }
    let line = say("topics-rule-always", &[("topic", topic.name.clone()), ("address", address)]);
    let id = topic.id.clone();
    offer(qt, shared, line, undo, move || if made { topics::remove(&config_path(), &id).err().unwrap_or_default() } else { String::new() });
    String::new()
}

// The topics' model on this device (`topics::judging`).

/// Mail of `account` that just arrived, judged by the topics' table; how many.
pub(crate) fn after_fetch(account: &sioul_core::config::Account, new: &[PathBuf]) -> usize {
    if new.is_empty() {
        return 0;
    }
    topics::judging::judge_files(&load_config(), &account.id, new)
}

/// Messages judged in one look at the mail kept here (`judge_by_itself`): a
/// table that just arrived judges the rest at the next looks.
const KEPT_AT_ONCE: usize = 2000;

/// The mail kept here that the table in use has not judged yet, off the
/// window's thread, a bounded number at a time; the mailbox made again when some was.
pub(crate) fn judge_by_itself(qt: &QtThread, shared: &Arc<Shared>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if !topics::judging::table_path().exists() || RUNNING.swap(true, Ordering::Relaxed) {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let (judged, _) = topics::judging::judge_kept(&load_config(), KEPT_AT_ONCE);
        if judged > 0 {
            crate::mail::show_mail(&qt, &shared);
        }
        RUNNING.store(false, Ordering::Relaxed);
    });
}

// Topic presets: day one, and those offered under "Add a topic".

/// Where day one's card keeps the topics it started, until the Porch said it once.
fn card_path() -> PathBuf {
    labels::state().join("day-one.toml")
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct DayOneCard {
    #[serde(default)]
    started: Vec<String>,
}

/// Day one (`topics::presets::day_one`), by itself, once: when no day one
/// was recorded and the mail kept here holds enough to judge, off the
/// window's thread, the presets that match at least five messages start as
/// topics; the Porch's card says which, once (`card`).
pub(crate) fn day_one_by_itself(qt: &QtThread, shared: &Arc<Shared>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    let config = load_config();
    if config.topics_day_one.is_some() || config.accounts.is_empty() || RUNNING.swap(true, Ordering::Relaxed) {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let matcher = topics::presets::Matcher::current();
        let (counts, read) = topics::presets::count(&config, &matcher);
        if read >= topics::presets::DAY_ONE_MAIL {
            let language = config.language.clone().unwrap_or_else(sioul_core::i18n::system_language);
            let today = jiff::Zoned::now().date().to_string();
            if let Ok(started) = topics::presets::day_one(&config_path(), &counts, &matcher, &language, &today)
                && !started.is_empty()
            {
                let card = DayOneCard { started: started.into_iter().map(|t| t.name).collect() };
                if let Ok(text) = toml::to_string(&card) {
                    let _ = std::fs::create_dir_all(labels::state());
                    let _ = sioul_core::filelock::replace(&card_path(), text.as_bytes());
                }
                crate::mail::show_mail(&qt, &shared);
            }
        }
        RUNNING.store(false, Ordering::Relaxed);
    });
}

/// Day one's card, for the Porch: "Sioul sorted your mail into 3 topics…",
/// "" once nothing was started or the card was put away (its view flag).
pub(crate) fn card() -> String {
    let card: DayOneCard = std::fs::read_to_string(card_path()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    if card.started.is_empty() {
        return String::new();
    }
    say("topics-day-one", &[("n", card.started.len().to_string()), ("topics", card.started.join(", "))])
}

/// The presets not started yet, offered under "Add a topic": [{id, name}], as JSON.
pub(crate) fn presets_offered() -> String {
    let config = load_config();
    let language = config.language.clone().unwrap_or_else(sioul_core::i18n::system_language);
    let matcher = topics::presets::Matcher::current();
    let offered: Vec<serde_json::Value> = matcher
        .presets
        .iter()
        .filter(|p| !config.topics.iter().any(|t| t.id == p.id || t.preset.as_deref() == Some(p.id.as_str())))
        .map(|p| serde_json::json!({ "id": p.id, "name": topics::presets::name_in(p, &language) }))
        .collect();
    serde_json::to_string(&offered).unwrap_or_else(|_| "[]".into())
}

/// A preset started as a topic of yours, said with Undo. Returns what went wrong, else "".
pub(crate) fn start_preset(qt: &QtThread, shared: &Arc<Shared>, id: &str) -> String {
    let config = load_config();
    let language = config.language.clone().unwrap_or_else(sioul_core::i18n::system_language);
    let matcher = topics::presets::Matcher::current();
    let Some(preset) = matcher.get(id) else { return tr().text("topics-gone", None) };
    let topic = topics::presets::topic_of(preset, &language);
    if let Err(e) = topics::put(&config_path(), &topic) {
        return e;
    }
    let id = topic.id.clone();
    offer(qt, shared, say("topics-made", &[("topic", topic.name.clone())]), Vec::new(), move || topics::remove(&config_path(), &id).err().unwrap_or_default());
    String::new()
}
