// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Topic presets (docs/topics.md, "Topic presets"): topics Sioul offers to
//! start sorting mail by, Bank, Taxes, Health, Phone and internet…, each by
//! its rules: the site presets' categories and types it takes
//! (`presets/sites.json`), sender domains beyond them, mailing lists. A plain
//! JSON file anyone can update without building Sioul
//! (`presets/topics.json`, checked by `tools/check-topic-presets.py`): the
//! newest, by its `checked` date, of the copy built in and those on disk, as
//! the site presets are.
//!
//! - **Only proven senders**: a rule holds for a sender Sioul's checks
//!   verify (`trust::judge_sender`); forged mail never takes a preset's topic.
//! - **The order** (`Matcher::find`): a preset's own domain or List-Id
//!   pattern; then a site's category or type; then "any list" (Newsletters),
//!   so that a bank's newsletter is the bank's. A site with several
//!   categories goes to the earliest preset in the file that takes one of them.
//! - **Day one** (`day_one`): a preset becomes a topic only when it matches
//!   at least `DAY_ONE` messages kept here; the others stay offered under
//!   "Add a topic". Started, a topic is the person's own (`[[topic]]` with
//!   its `preset`): its name, area and switch are theirs, and an update of
//!   the presets never changes them back; only its rules follow the file.

use crate::presets::Words;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// Messages a preset must match, on day one, to become a topic by itself.
pub const DAY_ONE: u64 = 5;

/// Messages kept here before day one comes: fewer, and it waits for the mail to come.
pub const DAY_ONE_MAIL: u64 = 100;

/// The presets file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TopicPresets {
    #[serde(default)]
    pub version: u32,
    /// When it was last checked: "2026-11-05". The newest file wins.
    #[serde(default)]
    pub checked: String,
    #[serde(default)]
    pub topics: Vec<Preset>,
}

/// One topic offered.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Preset {
    pub id: String,
    /// Its name in each language: "en" → "Bank", "fr" → "Banque".
    pub name: Words,
    /// What it is for: "work", "admin", "leisure".
    #[serde(default)]
    pub area: String,
    /// Open to AI agents when it starts; health is not.
    #[serde(default = "yes")]
    pub agents: bool,
    #[serde(default)]
    pub rules: Rules,
}

fn yes() -> bool {
    true
}

/// A preset's rules.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Rules {
    /// The site presets' categories it takes (`presets::Presets::tags`): "bank".
    #[serde(default)]
    pub site_tags: Vec<String>,
    /// The site presets' types it takes: "social".
    #[serde(default)]
    pub site_types: Vec<String>,
    /// Sender domains beyond the site presets, each with its source.
    #[serde(default)]
    pub domains: Vec<Sourced>,
    /// List-Id patterns ("*": any mailing list or newsletter), each with its source.
    #[serde(default)]
    pub lists: Vec<Pattern>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Sourced {
    pub domain: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Pattern {
    pub pattern: String,
    #[serde(default)]
    pub source: String,
}

const BUILT_IN: &str = include_str!("../../../../presets/topics.json");

/// The presets built into this Sioul.
pub fn built_in() -> TopicPresets {
    serde_json::from_str(BUILT_IN).unwrap_or_default()
}

/// Where newer presets may be: yours first, then the system's (as the site presets').
fn places() -> Vec<PathBuf> {
    let mut places = vec![crate::config::data_dir().join("presets").join("topics.json")];
    let system = std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| if cfg!(windows) { Default::default() } else { "/usr/local/share:/usr/share".into() });
    places.extend(std::env::split_paths(&system).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join("sioul").join("presets").join("topics.json")));
    places
}

/// The newest presets: of the copy built in and those on disk, the one checked last.
pub fn load() -> TopicPresets {
    let mut best = built_in();
    for path in places() {
        if let Some(found) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<TopicPresets>(&t).ok())
            && found.checked > best.checked
        {
            best = found;
        }
    }
    best
}

/// How a preset took a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum By {
    /// One of its own sender domains, or a List-Id pattern of its own.
    Domain,
    /// A site of a category or type it takes.
    Site,
    /// A mailing list or newsletter, any.
    List,
}

/// The presets' rules made into lookups (see the module).
#[derive(Debug, Clone, Default)]
pub struct Matcher {
    pub presets: Vec<Preset>,
    domains: HashMap<String, usize>,
    sites: HashMap<String, usize>,
    lists: Vec<(String, usize)>,
    any_list: Option<usize>,
}

/// A domain and its parents, the domain first, never a bare top-level one.
fn and_parents(domain: &str) -> impl Iterator<Item = &str> {
    let parts: Vec<usize> = std::iter::once(0).chain(domain.match_indices('.').map(|(i, _)| i + 1)).collect();
    parts.into_iter().map(move |at| &domain[at..]).filter(|d| d.contains('.'))
}

impl Matcher {
    /// From the topic presets and the site presets.
    pub fn new(topics: &TopicPresets, sites: &crate::presets::Presets) -> Matcher {
        let presets = topics.topics.clone();
        let mut matcher = Matcher { presets, ..Matcher::default() };
        for (p, preset) in matcher.presets.iter().enumerate() {
            for d in &preset.rules.domains {
                matcher.domains.entry(d.domain.trim().to_lowercase()).or_insert(p);
            }
            for l in &preset.rules.lists {
                match l.pattern.trim() {
                    "*" => matcher.any_list = matcher.any_list.or(Some(p)),
                    pattern => matcher.lists.push((pattern.to_lowercase(), p)),
                }
            }
        }
        let every = sites.international.iter().chain(sites.countries.values().flat_map(|c| c.sites.iter().chain(c.regions.values().flat_map(|r| r.sites.iter()))));
        for site in every {
            // The earliest preset that takes one of its categories or its type.
            let Some(p) = matcher.presets.iter().position(|preset| preset.rules.site_tags.iter().any(|t| site.tags.contains(t)) || preset.rules.site_types.contains(&site.kind)) else { continue };
            let host = site.url.split("://").nth(1).unwrap_or("").split(['/', ':', '?']).next().unwrap_or("").trim_start_matches("www.").to_lowercase();
            for domain in site.announced_by.iter().map(|d| d.trim().trim_start_matches('@').to_lowercase()).chain((!host.is_empty()).then_some(host)) {
                matcher.sites.entry(domain).or_insert(p);
            }
        }
        matcher
    }

    /// The presets as Sioul has them now (`load`, the site presets'
    /// `presets::load`), read once per process.
    pub fn current() -> Arc<Matcher> {
        static CURRENT: OnceLock<Arc<Matcher>> = OnceLock::new();
        Arc::clone(CURRENT.get_or_init(|| Arc::new(Matcher::new(&load(), &crate::presets::load()))))
    }

    /// The preset that takes a message, and how: `domain`, its sender's
    /// domain when Sioul's checks verify it (none otherwise: nothing takes
    /// it); `list_id`, its List-Id when known; `is_list`, a list or a newsletter.
    pub fn find(&self, domain: Option<&str>, list_id: Option<&str>, is_list: bool) -> Option<(usize, By)> {
        let domain = domain.map(|d| d.trim().trim_end_matches('.').to_lowercase())?;
        if let Some(&p) = and_parents(&domain).find_map(|d| self.domains.get(d)) {
            return Some((p, By::Domain));
        }
        if let Some(id) = list_id.map(|l| l.trim().to_lowercase())
            && let Some(&(_, p)) = self.lists.iter().find(|(pattern, _)| glob(pattern, &id))
        {
            return Some((p, By::Domain));
        }
        if let Some(&p) = and_parents(&domain).find_map(|d| self.sites.get(d)) {
            return Some((p, By::Site));
        }
        (is_list || list_id.is_some()).then_some(self.any_list).flatten().map(|p| (p, By::List))
    }

    /// A preset by its id.
    pub fn get(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }
}

/// Whether a pattern with `*` for any characters covers a List-Id.
fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || !text.ends_with(last) || text.len() < first.len() + last.len() {
        return false;
    }
    let mut at = first.len();
    for middle in &parts[1..parts.len() - 1] {
        match text[at..text.len() - last.len()].find(middle) {
            Some(found) => at += found + middle.len(),
            None => return false,
        }
    }
    true
}

/// A preset's name in a language, else English.
pub fn name_in(preset: &Preset, language: &str) -> String {
    crate::presets::words(&preset.name, language.split(['-', '_']).next().unwrap_or("en"))
}

/// The topic a preset starts: its id, its name in your language, its area, its agents' switch.
pub fn topic_of(preset: &Preset, language: &str) -> super::Topic {
    super::Topic {
        id: preset.id.clone(),
        name: name_in(preset, language),
        area: (!preset.area.is_empty()).then(|| preset.area.clone()),
        agents: preset.agents,
        preset: Some(preset.id.clone()),
        seeds: super::Seeds::default(),
    }
}

/// Each preset's messages among those kept here, by how it took them: the
/// counts `day_one` weighs, and `sioul topics seeds` says.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Counted {
    pub by_domain: u64,
    pub by_site: u64,
    pub by_list: u64,
}

impl Counted {
    pub fn total(&self) -> u64 {
        self.by_domain + self.by_site + self.by_list
    }

    pub fn add(&mut self, by: By) {
        match by {
            By::Domain => self.by_domain += 1,
            By::Site => self.by_site += 1,
            By::List => self.by_list += 1,
        }
    }
}

/// Counts, by preset id, the messages each preset takes among the mail kept
/// on this device (every address's folders but Junk, Trash and Drafts, each
/// message once, through the folders' indexes: no message read again), as
/// the mailbox judges a sender: verified, or nothing. With the messages read.
pub fn count(config: &crate::config::Config, matcher: &Matcher) -> (BTreeMap<String, Counted>, u64) {
    use crate::folders::Role;
    let sources = config.mail_sources();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut counts: BTreeMap<String, Counted> = BTreeMap::new();
    let addresses: Vec<(String, std::path::PathBuf)> = config.accounts.iter().filter(|a| a.kind == crate::config::AccountKind::Imap).map(|a| (a.id.clone(), a.maildir_path())).collect();
    let saved: Vec<_> = addresses.iter().map(|(id, _)| crate::folders::saved(id)).collect();
    for (a, local) in crate::mailpass::folders(&addresses) {
        let (account, root) = &addresses[a];
        let trusted = sources.iter().find(|s| s.account.as_deref() == Some(account.as_str())).map(|s| s.trusted_ids.clone()).unwrap_or_default();
        let role = if local.is_empty() { Role::Inbox } else { saved[a].iter().find(|f| f.local == local).map_or_else(|| crate::folders::role_of(local.trim_start_matches('.'), None), |f| f.role) };
        if matches!(role, Role::Junk | Role::Trash | Role::Drafts) {
            continue;
        }
        for found in crate::mailpass::folder(&crate::mailpass::Headers, account, root, &local, 0).found {
            let key = found.line.message_id.as_deref().map(crate::mailindex::bare_id).unwrap_or_else(|| format!("{account}/{}", found.line.unique));
            if !seen.insert(key) {
                continue;
            }
            let card = found.line.card(&found.path);
            if let Some((p, by)) = judge(matcher, &card, &trusted) {
                counts.entry(matcher.presets[p].id.clone()).or_default().add(by);
            }
        }
    }
    let read = seen.len() as u64;
    (counts, read)
}

/// The preset a message's card takes (`Matcher::find`), its sender verified
/// by the stamps its account trusts; none for forged or unproven mail.
pub fn judge(matcher: &Matcher, card: &crate::card::Card, trusted: &[String]) -> Option<(usize, By)> {
    use crate::trust::{self, Trust};
    let results = trust::read_auth_results(&card.headers, trusted);
    let excused = card.is_list.then(|| trust::list_excused(results.as_ref(), &card.headers, &trust::sealers(trusted, &[]))).flatten();
    let (judged, _) = trust::judge_sender(results.as_ref(), excused.as_ref(), card.sender_domain());
    let domain = (judged == Trust::Verified).then(|| card.sender_domain()).flatten();
    let list = card.headers.first("List-Id").and_then(|v| {
        let inside = v.rsplit_once('<').map_or(v, |(_, r)| r.split('>').next().unwrap_or(r));
        let id = inside.trim().to_lowercase();
        (!id.is_empty()).then_some(id)
    });
    matcher.find(domain, list.as_deref(), card.is_list)
}

/// Day one (see the module): the presets that take at least `DAY_ONE` of
/// the messages counted start as topics (`[[topic]]` with their `preset`),
/// named in `language`, those already there left as they are; then
/// `topics_day_one` is written, so that it never runs again. Returns the
/// topics started.
pub fn day_one(path: &Path, counts: &BTreeMap<String, Counted>, matcher: &Matcher, language: &str, today: &str) -> Result<Vec<super::Topic>, String> {
    let config = crate::config::Config::load(path).unwrap_or_default();
    let mut started = Vec::new();
    for preset in &matcher.presets {
        let enough = counts.get(&preset.id).is_some_and(|c| c.total() >= DAY_ONE);
        let there = config.topics.iter().any(|t| t.id == preset.id || t.preset.as_deref() == Some(preset.id.as_str()));
        if enough && !there {
            let topic = topic_of(preset, language);
            super::put(path, &topic)?;
            started.push(topic);
        }
    }
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        doc["topics_day_one"] = toml_edit::value(today);
        crate::config::write_document(path, &doc)
    })?;
    Ok(started)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sites() -> crate::presets::Presets {
        serde_json::from_str(
            r#"{"checked": "2026-11-05", "tags": {"bank": {"name": {"en": "Bank"}}, "admin": {"name": {"en": "Administration"}}, "tax": {"name": {"en": "Taxes"}}},
            "international": [{"name": "Chat", "url": "https://chat.example/", "type": "chat", "announced_by": ["chat.example"]}],
            "countries": {"FR": {"name": {"fr": "France"}, "sites": [
                {"name": "Banque", "url": "https://www.banque.example/login", "type": "mailbox", "tags": ["bank"], "announced_by": ["mail.banque.example"]},
                {"name": "Impôts", "url": "https://impots.example/", "type": "mailbox", "tags": ["admin", "tax"], "announced_by": []},
                {"name": "Mairie", "url": "https://mairie.example/", "type": "mailbox", "tags": ["admin"], "announced_by": []}]}}}"#,
        )
        .unwrap()
    }

    fn topics() -> TopicPresets {
        serde_json::from_str(
            r#"{"checked": "2026-11-05", "topics": [
            {"id": "bank", "name": {"en": "Bank", "fr": "Banque"}, "area": "admin", "rules": {"site_tags": ["bank"], "domains": [{"domain": "pay.example", "source": "https://pay.example/"}]}},
            {"id": "taxes", "name": {"en": "Taxes", "fr": "Impôts"}, "area": "admin", "rules": {"site_tags": ["tax"]}},
            {"id": "health", "name": {"en": "Health", "fr": "Santé"}, "area": "admin", "agents": false, "rules": {"domains": [{"domain": "clinic.example", "source": "https://clinic.example/"}]}},
            {"id": "administration", "name": {"en": "Administration", "fr": "Démarches"}, "area": "admin", "rules": {"site_tags": ["admin"]}},
            {"id": "online", "name": {"en": "Online services"}, "area": "admin", "rules": {"site_types": ["chat"], "lists": [{"pattern": "*.forum.example", "source": "https://forum.example/"}]}},
            {"id": "newsletters", "name": {"en": "Newsletters", "fr": "Lettres d’information"}, "area": "leisure", "rules": {"lists": [{"pattern": "*", "source": "https://www.rfc-editor.org/rfc/rfc2919"}]}}]}"#,
        )
        .unwrap()
    }

    /// Its own domains first, a List-Id pattern, then a site's category (the
    /// earliest preset of a site with several), then any list; unproven: nothing.
    #[test]
    fn the_order_of_a_presets_rules() {
        let m = Matcher::new(&topics(), &sites());
        let id = |found: Option<(usize, By)>| found.map(|(p, by)| (m.presets[p].id.as_str(), by));
        assert_eq!(id(m.find(Some("news.pay.example"), None, true)), Some(("bank", By::Domain)), "its own domain, a list or not");
        assert_eq!(id(m.find(Some("mail.banque.example"), None, false)), Some(("bank", By::Site)));
        assert_eq!(id(m.find(Some("Banque.Example."), None, false)), Some(("bank", By::Site)), "the site's own address");
        assert_eq!(id(m.find(Some("impots.example"), None, false)), Some(("taxes", By::Site)), "admin and tax: the earlier preset");
        assert_eq!(id(m.find(Some("mairie.example"), None, false)), Some(("administration", By::Site)));
        assert_eq!(id(m.find(Some("chat.example"), None, false)), Some(("online", By::Site)), "by the site's type");
        assert_eq!(id(m.find(Some("lists.example"), Some("dev.forum.example"), true)), Some(("online", By::Domain)), "a List-Id pattern");
        assert_eq!(id(m.find(Some("shop.example"), None, true)), Some(("newsletters", By::List)));
        assert_eq!(m.find(Some("shop.example"), None, false), None);
        assert_eq!(m.find(None, None, true), None, "an unproven sender: nothing");
        assert!(glob("*", "x") && glob("a*c", "abc") && !glob("a*c", "ab") && glob("*.forum.example", "dev.forum.example"));
        let health = topic_of(m.get("health").unwrap(), "fr-FR");
        assert_eq!((health.name.as_str(), health.agents, health.preset.as_deref(), health.area.as_deref()), ("Santé", false, Some("health"), Some("admin")));
    }

    /// Day one starts the presets with five messages or more, once; a topic
    /// already there, renamed by you, stays as it is.
    #[test]
    fn day_one_starts_what_matches() {
        let root = std::env::temp_dir().join(format!("sioul-topic-presets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("config.toml");
        std::fs::write(&path, "language = \"fr\"\n\n[[topic]]\nid = \"taxes\"\nname = \"Mes impôts\"\npreset = \"taxes\"\n").unwrap();
        let m = Matcher::new(&topics(), &sites());
        let counts = BTreeMap::from([
            ("bank".to_string(), Counted { by_domain: 2, by_site: 3, by_list: 0 }),
            ("taxes".to_string(), Counted { by_domain: 0, by_site: 9, by_list: 0 }),
            ("newsletters".to_string(), Counted { by_domain: 0, by_site: 0, by_list: 4 }),
        ]);
        let started = day_one(&path, &counts, &m, "fr", "2026-11-05").unwrap();
        assert_eq!(started.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["Banque"]);
        let config = crate::config::Config::load(&path).unwrap();
        assert_eq!(config.topics.iter().map(|t| (t.id.as_str(), t.name.as_str())).collect::<Vec<_>>(), vec![("taxes", "Mes impôts"), ("bank", "Banque")]);
        assert_eq!(config.topics_day_one.as_deref(), Some("2026-11-05"));
        let _ = std::fs::remove_dir_all(root);
    }
}
