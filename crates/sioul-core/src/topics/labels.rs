// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Your word on topics (docs/topics.md, "Your word"): one JSON line per
//! act, added at the end of a file and never rewritten. Each device writes
//! its own log, `$XDG_STATE_HOME/sioul/topics/labels/<device>.jsonl`
//! (`own_log`), and reads every device's; the newest line about a message,
//! or about a sender and a topic, wins.
//!
//! - **A message's topic** (`Act::Message`): the topic you gave it ("Topic…"
//!   on a message, or on several at once: a line each), or none (you took
//!   it off); `forget`, your word taken back (Undo of a first word), so that
//!   what Sioul says counts again.
//! - **A sender's** (`Act::Sender`): always this topic for mail from this
//!   address, never it, or the rule taken off.
//!
//! A line names its message by its Message-ID, its sender by the address,
//! a topic by its id; never a subject or a word of the mail.
//!
//! ```text
//! {"act":"message","at":1791360000,"message_id":"abc@example.org","topic":"choir"}
//! {"act":"sender","at":1791360060,"address":"news@choir.example","topic":"choir","rule":"always"}
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The folder of the logs, one file per device, in the topics' state folder.
pub const FOLDER: &str = "labels";

/// A sender's rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rule {
    Always,
    Never,
    /// The rule taken off.
    Off,
}

/// One act, one line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "act", rename_all = "kebab-case")]
pub enum Act {
    /// A message's topic, by its bare Message-ID: `topic` its id, none for no topic;
    /// `forget`: your word taken back.
    Message {
        at: i64,
        message_id: String,
        #[serde(default)]
        topic: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        forget: bool,
    },
    /// A sender's rule for a topic, by the sender's address, lowercase.
    Sender { at: i64, address: String, topic: String, rule: Rule },
}

impl Act {
    pub fn at(&self) -> i64 {
        match self {
            Act::Message { at, .. } | Act::Sender { at, .. } => *at,
        }
    }
}

/// The topics' state folder on this device: `$XDG_STATE_HOME/sioul/topics`.
pub fn state() -> PathBuf {
    crate::config::state_dir().join("topics")
}

/// This device's own log under `root` (the topics' state folder),
/// `labels/<device>.jsonl`; `device` is its name in the sharing.
pub fn own_log(root: &Path, device: &str) -> PathBuf {
    let safe: String = device.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    root.join(FOLDER).join(format!("{}.jsonl", if safe.is_empty() { "this-device" } else { &safe }))
}

/// Adds acts at the end of the log at `path`, each a line, all at once under
/// the file's lock (`filelock`); the file yours alone, in a folder of yours alone (Unix).
pub fn append(path: &Path, acts: &[Act]) -> Result<(), String> {
    use std::io::Write;
    if acts.is_empty() {
        return Ok(());
    }
    let fail = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut text = String::new();
    for act in acts {
        text.push_str(&serde_json::to_string(act).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    if let Some(folder) = path.parent() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(folder).map_err(fail)?;
    }
    crate::filelock::with_lock(path, || {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(path).and_then(|mut file| file.write_all(text.as_bytes())).map_err(fail)
    })?;
    // Every view of mail's topics told (`changes`).
    crate::changes::said(crate::changes::Kind::Topics);
    Ok(())
}

/// Every act of every device's log under `root`, oldest first (by when you
/// said it, then in the order read); a line that does not read (cut by a
/// crash, written by a later Sioul) is left out.
pub fn read_all_in(root: &Path) -> Vec<Act> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join(FOLDER))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl") && p.is_file())
        .collect();
    files.sort();
    let mut acts: Vec<Act> = files.iter().flat_map(|p| crate::spam::labels::read_from::<Act>(p)).collect();
    acts.sort_by_key(Act::at);
    acts
}

/// What your acts say now, the newest of each: a message's topic (none: no
/// topic), each sender's rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Said {
    pub messages: HashMap<String, Option<String>>,
    /// By address: each topic's rule, Always or Never (Off taken out).
    pub senders: HashMap<String, Vec<(String, Rule)>>,
}

impl Said {
    /// From acts oldest first.
    pub fn of(acts: &[Act]) -> Said {
        let mut said = Said::default();
        for act in acts {
            match act {
                Act::Message { message_id, topic, forget, .. } => {
                    let id = crate::mailindex::bare_id(message_id);
                    if *forget {
                        said.messages.remove(&id);
                    } else {
                        said.messages.insert(id, topic.clone());
                    }
                }
                Act::Sender { address, topic, rule, .. } => {
                    let rules = said.senders.entry(address.trim().to_lowercase()).or_default();
                    rules.retain(|(t, _)| t != topic);
                    if *rule != Rule::Off {
                        rules.push((topic.clone(), *rule));
                    }
                }
            }
        }
        said.senders.retain(|_, rules| !rules.is_empty());
        said
    }

    /// Every device's, here.
    pub fn load() -> Said {
        Said::of(&read_all_in(&state()))
    }

    /// Your word on a message: Some(Some(topic)), Some(None) for no topic, None when you said nothing.
    pub fn message(&self, id: &str) -> Option<Option<&str>> {
        self.messages.get(id).map(Option::as_deref)
    }

    /// A sender's rule for a topic.
    pub fn rule(&self, address: &str, topic: &str) -> Option<Rule> {
        self.senders.get(address).and_then(|rules| rules.iter().find(|(t, _)| t == topic)).map(|(_, r)| *r)
    }

    /// The acts that put these messages back as your word has them now: an
    /// Undo's lines, written `at`, after the act it takes back.
    pub fn restoring(&self, ids: &[String], at: i64) -> Vec<Act> {
        ids.iter()
            .map(|id| {
                let id = crate::mailindex::bare_id(id);
                match self.messages.get(&id) {
                    Some(topic) => Act::Message { at, message_id: id, topic: topic.clone(), forget: false },
                    None => Act::Message { at, message_id: id, topic: None, forget: true },
                }
            })
            .collect()
    }

    /// The act that puts a sender's rule for a topic back as it is now (Undo).
    pub fn restoring_rule(&self, address: &str, topic: &str, at: i64) -> Act {
        let address = address.trim().to_lowercase();
        Act::Sender { at, rule: self.rule(&address, topic).unwrap_or(Rule::Off), address, topic: topic.to_string() }
    }
}

/// The acts that give these messages one topic (none: no topic), `at`.
pub fn tagging(ids: &[String], topic: Option<&str>, at: i64) -> Vec<Act> {
    ids.iter().map(|id| Act::Message { at, message_id: crate::mailindex::bare_id(id), topic: topic.map(str::to_string), forget: false }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Written by two devices, read together: the newest word wins; Undo takes your word back.
    #[test]
    fn the_newest_word_wins() {
        let root = std::env::temp_dir().join(format!("sioul-topics-labels-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let desk = own_log(&root, "desk-1");
        let phone = own_log(&root, "phone/2");
        assert_eq!(phone, root.join(FOLDER).join("phone_2.jsonl"));
        let message = |at, topic: Option<&str>, forget| Act::Message { at, message_id: "<a@example.org>".into(), topic: topic.map(str::to_string), forget };
        let ((), said) = crate::changes::heard(|| append(&desk, &[message(1, Some("choir"), false), Act::Sender { at: 2, address: "News@Choir.example".into(), topic: "choir".into(), rule: Rule::Always }]).unwrap());
        assert_eq!(said, [crate::changes::Kind::Topics], "every view of topics told");
        append(&phone, &[message(3, None, false), Act::Sender { at: 4, address: "news@choir.example".into(), topic: "tax".into(), rule: Rule::Never }]).unwrap();
        let said = Said::of(&read_all_in(&root));
        assert_eq!(said.message("a@example.org"), Some(None), "the phone took it off later");
        assert_eq!(said.rule("news@choir.example", "choir"), Some(Rule::Always));
        assert_eq!(said.rule("news@choir.example", "tax"), Some(Rule::Never));
        append(&desk, &[message(5, None, true), Act::Sender { at: 6, address: "news@choir.example".into(), topic: "tax".into(), rule: Rule::Off }]).unwrap();
        let said = Said::of(&read_all_in(&root));
        assert_eq!(said.message("a@example.org"), None, "forgotten");
        assert_eq!(said.rule("news@choir.example", "tax"), None);
        // A line a later Sioul wrote, or cut short: left out.
        std::fs::write(root.join(FOLDER).join("later.jsonl"), "{\"act\":\"group\",\"at\":7}\n{\"act\":\"mess").unwrap();
        assert_eq!(read_all_in(&root).len(), 6);
        // Undo: what restoring writes puts each message back as it was.
        let before = Said::of(&read_all_in(&root));
        let ids = vec!["a@example.org".to_string(), "b@example.org".to_string()];
        let undo = before.restoring(&ids, 8);
        append(&desk, &tagging(&ids, Some("tax"), 7)).unwrap();
        append(&desk, &undo).unwrap();
        assert_eq!(Said::of(&read_all_in(&root)).messages, before.messages);
        let line = serde_json::to_string(&message(1, Some("choir"), false)).unwrap();
        assert_eq!(line, r#"{"act":"message","at":1,"message_id":"<a@example.org>","topic":"choir"}"#);
        let _ = std::fs::remove_dir_all(root);
    }
}
