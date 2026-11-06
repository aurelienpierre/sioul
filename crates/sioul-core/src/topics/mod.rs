// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail topics (docs/topics.md): a standing part of life (Health, Tax,
//! Choir) that Sioul writes on a message as a quiet word, one at most. A
//! topic is never a folder: mail stays where it is, and a topic is a way to
//! look at the one mailbox.
//!
//! What every device runs is here:
//!
//! - [`Topic`]: a topic as the configuration keeps it (`[[topic]]`), and
//!   the functions that add, rename and remove one (`add`, `rename`, `remove`);
//! - [`labels`]: your word on topics, one line per act, each device its own log;
//! - [`presets`]: topics Sioul offers to start with (Bank, Taxes, Health…), by the senders it proves;
//! - [`rules`]: a message's topic and why, your word first;
//! - [`sender_words`] and [`line()`]: what a message's sender adds to the line
//!   the model reads, when Sioul's own checks proved who sent it, and the line itself;
//! - [`seeds`]: the first topics, proposed from what the person already
//!   sorted (folders, contacts' categories);
//! - [`table`]: the model folded into a table that judges a message without
//!   it, on the phone too;
//! - [`judging`]: that table where this device keeps it, and what it judged
//!   of the mail kept here, after each fetch and when a table arrives.
//!
//! The training and its measures are sioul-learn's (`sioul_learn::topics`,
//! `sioul topics seeds` and `sioul topics eval`), on a computer only.

pub mod judging;
pub mod labels;
pub mod presets;
pub mod rules;
pub mod seeds;
pub mod table;

use crate::card::Card;
use serde::{Deserialize, Serialize};
use std::path::Path;
use toml_edit::{ArrayOfTables, Item, Table, value};

/// A topic, as the configuration keeps it (`[[topic]]`): a standing part of
/// life, never a folder. Its id is stable, ASCII (`slug`); its name as you
/// wrote it, in any language.
///
/// ```toml
/// [[topic]]
/// id = "choir"
/// name = "Chorale"
/// seeds = { folders = ["Chorale"], categories = ["Chorale"] }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub name: String,
    /// What it is for ("work", "admin", "leisure"): Mail's area filter only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    /// Open to AI agents; every topic is closed until you open it.
    #[serde(default)]
    pub agents: bool,
    /// The preset it started from (`presets`), whose rules it follows; none for your own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// The folders and contacts' categories whose mail it takes.
    #[serde(default)]
    pub seeds: Seeds,
}

/// A topic's seeds: folders by their name as read, categories as written on the cards.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seeds {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
}

/// Adds a topic to the configuration at `path`, named `name`, with these
/// seeds; its id made from its name, unique. Returns it.
pub fn add(path: &Path, name: &str, seeds: &Seeds) -> Result<Topic, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a topic needs a name".into());
    }
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let taken: Vec<String> = doc.get("topic").and_then(Item::as_array_of_tables).map(|t| t.iter().filter_map(|t| t.get("id").and_then(Item::as_str).map(str::to_string)).collect()).unwrap_or_default();
        let topic = Topic { id: slug(name, &taken), name: name.to_string(), seeds: seeds.clone(), ..Topic::default() };
        let mut table = Table::new();
        table["id"] = value(&topic.id);
        table["name"] = value(&topic.name);
        set_seeds(&mut table, seeds);
        let trailing = doc.trailing().as_str().unwrap_or("").trim_end().to_string();
        table.decor_mut().set_prefix(if trailing.trim().is_empty() { "\n".to_string() } else { format!("{trailing}\n\n") });
        doc.set_trailing("");
        let list = doc.entry("topic").or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
        list.as_array_of_tables_mut().ok_or_else(|| format!("{}: `topic` is not a list of tables", path.display()))?.push(table);
        crate::config::write_document(path, &doc)?;
        Ok(topic)
    })
}

fn set_seeds(table: &mut Table, seeds: &Seeds) {
    let list = |items: &[String]| {
        let mut array = toml_edit::Array::new();
        for item in items.iter().map(|i| i.trim()).filter(|i| !i.is_empty()) {
            array.push(item);
        }
        array
    };
    table.remove("seeds");
    if !seeds.folders.is_empty() || !seeds.categories.is_empty() {
        let mut inline = toml_edit::InlineTable::new();
        if !seeds.folders.is_empty() {
            inline.insert("folders", list(&seeds.folders).into());
        }
        if !seeds.categories.is_empty() {
            inline.insert("categories", list(&seeds.categories).into());
        }
        table["seeds"] = value(inline);
    }
}

/// Changes one topic in the configuration at `path`, found by its id.
fn change(path: &Path, id: &str, edit: impl FnOnce(&mut Table)) -> Result<(), String> {
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let table = doc
            .get_mut("topic")
            .and_then(Item::as_array_of_tables_mut)
            .and_then(|list| list.iter_mut().find(|t| t.get("id").and_then(Item::as_str) == Some(id)))
            .ok_or_else(|| format!("{}: no topic {id}", path.display()))?;
        edit(table);
        crate::config::write_document(path, &doc)
    })
}

/// A topic's name changed; its id stays, and with it every word you gave.
pub fn rename(path: &Path, id: &str, name: &str) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("a topic needs a name".into());
    }
    change(path, id, |t| t["name"] = value(name))
}

/// A topic's seeds replaced.
pub fn reseed(path: &Path, id: &str, seeds: &Seeds) -> Result<(), String> {
    change(path, id, |t| set_seeds(t, seeds))
}

/// A topic put back as it was (a removal's Undo): its id, name, area,
/// agents' switch and seeds, at the end of the list.
pub fn put(path: &Path, topic: &Topic) -> Result<(), String> {
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let mut table = Table::new();
        table["id"] = value(&topic.id);
        table["name"] = value(&topic.name);
        if let Some(area) = &topic.area {
            table["area"] = value(area);
        }
        if topic.agents {
            table["agents"] = value(true);
        }
        if let Some(preset) = &topic.preset {
            table["preset"] = value(preset);
        }
        set_seeds(&mut table, &topic.seeds);
        let list = doc.entry("topic").or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
        let list = list.as_array_of_tables_mut().ok_or_else(|| format!("{}: `topic` is not a list of tables", path.display()))?;
        list.retain(|t| t.get("id").and_then(Item::as_str) != Some(topic.id.as_str()));
        list.push(table);
        crate::config::write_document(path, &doc)
    })
}

/// A topic taken out of the configuration. Your words stay in the logs, so
/// that its Undo, or a topic of the same id added again, finds them.
pub fn remove(path: &Path, id: &str) -> Result<(), String> {
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let Some(list) = doc.get_mut("topic").and_then(Item::as_array_of_tables_mut) else { return Ok(()) };
        list.retain(|t| t.get("id").and_then(Item::as_str) != Some(id));
        crate::config::write_document(path, &doc)
    })
}

/// The sender words' version ([`sender_words`]): a table made with other
/// words reads its senders otherwise, and is refused.
pub const SENDERS: u32 = 1;

/// Words a line holds at most: fastText learns from lines of 1024 words and
/// drops the rest, so the sender's words come first ([`line()`]).
pub const LINE_WORDS: usize = 1000;

/// The words a message's sender adds to its line, when Sioul's own checks
/// proved who sent it (`verified`: `trust::judge_sender` says Verified):
/// its address under a hash, `from:` and 12 hexadecimal digits; its domain,
/// `from-dom:example.org`; its mailing list, `list:` and its List-Id.
/// Unproven mail adds none, so that forged mail never borrows a sender's topic.
pub fn sender_words(card: &Card, verified: bool) -> Vec<String> {
    if !verified {
        return Vec::new();
    }
    let mut words = Vec::new();
    if let Some(address) = card.from_address.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        let hash = crate::spam::table::word_hash(&address.to_lowercase()) & 0xffff_ffff_ffff;
        words.push(format!("from:{hash:012x}"));
    }
    if let Some(domain) = card.sender_domain().map(|d| d.trim().trim_end_matches('.').to_lowercase()).filter(|d| !d.is_empty() && !d.contains(char::is_whitespace)) {
        words.push(format!("from-dom:{domain}"));
    }
    if let Some(list) = card.headers.first("List-Id").and_then(list_id) {
        words.push(format!("list:{list}"));
    }
    words
}

/// A List-Id's identifier (RFC 2919): between `<` and `>`, else the whole
/// value; lowercase, without spaces; none when empty.
fn list_id(value: &str) -> Option<String> {
    let inside = match (value.rfind('<'), value.rfind('>')) {
        (Some(open), Some(close)) if open < close => &value[open + 1..close],
        _ => value,
    };
    let id: String = inside.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect::<String>().to_lowercase();
    (!id.is_empty()).then_some(id)
}

/// The line a model reads, and the table: the sender's words first, then the
/// message's words (`spam::tokenize`), [`LINE_WORDS`] at most.
pub fn line(words: &[String], senders: &[String]) -> Vec<String> {
    senders.iter().chain(words).filter(|w| !w.is_empty() && !w.contains(char::is_whitespace)).take(LINE_WORDS).cloned().collect()
}

/// A topic's id from its name: ASCII, lowercase, accents taken off, every
/// other character a dash ("Impôts 2025" → "impots-2025"); "topic" when
/// nothing is left; "-2", "-3"… when it is taken.
pub fn slug(name: &str, taken: &[String]) -> String {
    let folded: String = crate::text::fold(name).into_iter().flat_map(char::to_lowercase).collect();
    let mut out = String::new();
    for c in folded.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let base = match out.trim_end_matches('-') {
        "" => "topic".to_string(),
        trimmed => trimmed.chars().take(40).collect::<String>().trim_end_matches('-').to_string(),
    };
    let mut id = base.clone();
    let mut n = 2;
    while taken.iter().any(|t| t == &id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Added, renamed, reseeded, removed: the other tables stay where they were.
    #[test]
    fn topics_in_the_configuration() {
        let root = std::env::temp_dir().join(format!("sioul-topics-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("config.toml");
        std::fs::write(&path, "# Mine.\nlanguage = \"fr\"\n\n[[account]]\nid = \"home\"\nkind = \"imap\"\n").unwrap();
        let (choir, said) = crate::changes::heard(|| add(&path, " Chorale ", &Seeds { folders: vec!["Chorale".into()], categories: Vec::new() }).unwrap());
        assert_eq!(said, [crate::changes::Kind::Topics], "the topic list's views told");
        let again = add(&path, "chorale", &Seeds::default()).unwrap();
        assert_eq!((choir.id.as_str(), again.id.as_str()), ("chorale", "chorale-2"));
        rename(&path, "chorale", "Chœur").unwrap();
        reseed(&path, "chorale-2", &Seeds { folders: Vec::new(), categories: vec!["Amis".into()] }).unwrap();
        let config: crate::config::Config = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config.topics.len(), 2);
        assert_eq!((config.topics[0].name.as_str(), config.topics[0].seeds.folders.as_slice()), ("Chœur", &["Chorale".to_string()][..]));
        assert_eq!(config.topics[1].seeds.categories, vec!["Amis"]);
        assert_eq!(config.accounts.len(), 1);
        remove(&path, "chorale").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# Mine.") && !text.contains("Chœur") && text.contains("chorale-2"), "{text}");
        // Put back as it was (Undo).
        put(&path, &config.topics[0]).unwrap();
        let back: crate::config::Config = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.topics.iter().find(|t| t.id == "chorale"), Some(&config.topics[0]));
        assert!(rename(&path, "gone", "x").is_err() && add(&path, "  ", &Seeds::default()).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    fn card(raw: &str) -> Card {
        Card::from_bytes(raw.as_bytes()).unwrap()
    }

    /// Proven mail names its sender, domain and list; unproven mail nothing.
    #[test]
    fn sender_words_only_for_proven_mail() {
        let c = card("From: Choir <News@Chorale.Example>\r\nList-Id: Choir news <news.chorale.example>\r\nSubject: Rehearsal\r\n\r\nSee you.\r\n");
        let words = sender_words(&c, true);
        assert_eq!(words.len(), 3, "{words:?}");
        assert!(words[0].starts_with("from:") && words[0].len() == 5 + 12 && words[0][5..].chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(words[1], "from-dom:chorale.example");
        assert_eq!(words[2], "list:news.chorale.example");
        // The same address, another case: the same word.
        let again = card("From: news@chorale.example\r\nSubject: x\r\n\r\ny\r\n");
        assert_eq!(sender_words(&again, true)[0], words[0]);
        assert!(sender_words(&c, false).is_empty(), "unproven: nothing");
        assert_eq!(list_id("plain.list.example"), Some("plain.list.example".into()));
        assert_eq!(list_id(" <> "), None);
    }

    /// The sender first, the words after, a thousand at most, never a space inside.
    #[test]
    fn the_line_puts_the_sender_first() {
        let words: Vec<String> = (0..1500).map(|i| format!("w{i}")).collect();
        let senders = vec!["from-dom:example.org".to_string(), String::new()];
        let line = line(&words, &senders);
        assert_eq!(line.len(), LINE_WORDS);
        assert_eq!(line[0], "from-dom:example.org");
        assert_eq!(line[1], "w0");
    }

    #[test]
    fn slugs_are_ascii_and_unique() {
        assert_eq!(slug("Impôts 2025", &[]), "impots-2025");
        assert_eq!(slug("  Book club! ", &[]), "book-club");
        assert_eq!(slug("Santé", &["sante".into()]), "sante-2");
        assert_eq!(slug("Santé", &["sante".into(), "sante-2".into()]), "sante-3");
        assert_eq!(slug("…", &[]), "topic");
    }
}
