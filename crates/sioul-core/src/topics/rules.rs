// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A message's topic, and why (docs/topics.md, "The order of words"), the
//! first that speaks:
//!
//! 1. **Your word on the message** (`labels::Act::Message`): its topic, or
//!    none, which nothing below overrules.
//! 2. **Your rule for its sender**: "Always <topic> for this sender".
//! 3. **Its conversation**: the topic you gave the first message of it you
//!    tagged (its References, oldest first, then In-Reply-To).
//! 4. **The topic's seeds** (`[[topic]]`'s `seeds`): one of its folders
//!    holds a copy, or its sender is on a card of one of its categories:
//!    your own sorting, before what Sioul ships.
//! 5. **A started preset's rules** (`presets::Matcher::find`): its own
//!    domains and List-Id patterns, then a site's category, then any list;
//!    for a sender Sioul's checks verify only.
//!
//! "Never <topic> for this sender" keeps that topic from 3, 4 and 5. A sender
//! counts only when Sioul's checks find the message authenticated (the
//! caller gives none otherwise): forged mail never borrows a sender's topic.
//! A topic since removed from the configuration names nothing.

use super::Topic;
use super::labels::{Rule, Said};

/// Why a message has its topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// You gave it this topic.
    Yours,
    /// You said "Always" for its sender.
    Sender,
    /// Its conversation's first message you tagged.
    Conversation,
    /// One of the topic's folders holds it (the folder's name).
    Folder(String),
    /// Its sender is on a card of one of the topic's categories (the category, as the topic writes it).
    Category(String),
    /// The topic's preset took it, and how.
    Preset(super::presets::By),
}

/// A message's topic: its place in the configuration's list, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tagged {
    pub topic: usize,
    pub why: Why,
}

/// What the rules read of a message.
#[derive(Debug, Clone, Copy, Default)]
pub struct Facts<'a> {
    /// Its bare Message-ID.
    pub id: Option<&'a str>,
    /// Its sender's address, lowercase, when Sioul's checks authenticate it.
    pub sender: Option<&'a str>,
    /// The messages it answers or cites, bare, its conversation's first first.
    pub conversation: &'a [String],
    /// Its copies' folders, as read ("Chorale", "Loisirs/Chorale").
    pub folders: &'a [String],
    /// Its sender's cards' categories, as compared (`porch::category_key`).
    pub categories: &'a [String],
    /// Its sender's domain when Sioul's checks verify it (DMARC, or a DKIM
    /// signature of the sender's domain): the presets' rules read nothing else.
    pub verified_domain: Option<&'a str>,
    /// Its List-Id when known, and whether it is a list or a newsletter.
    pub list_id: Option<&'a str>,
    pub is_list: bool,
}

fn same(a: &str, b: &str) -> bool {
    crate::porch::category_key(a) == crate::porch::category_key(b)
}

/// A message's topic, by the order of words (see the module); none for most
/// mail. `presets`: the presets' rules, for the topics started from one.
pub fn topic_of(topics: &[Topic], said: &Said, facts: &Facts, presets: Option<&super::presets::Matcher>) -> Option<Tagged> {
    let place = |id: &str| topics.iter().position(|t| t.id == id);
    if let Some(word) = facts.id.and_then(|id| said.message(id)) {
        return word.and_then(place).map(|topic| Tagged { topic, why: Why::Yours });
    }
    let rules = facts.sender.and_then(|s| said.senders.get(s));
    let never = |t: usize| rules.is_some_and(|rules| rules.iter().any(|(id, r)| *r == Rule::Never && *id == topics[t].id));
    // The newest "Always" for this sender.
    if let Some(topic) = rules.and_then(|rules| rules.iter().rev().filter(|(_, r)| *r == Rule::Always).find_map(|(id, _)| place(id))) {
        return Some(Tagged { topic, why: Why::Sender });
    }
    for id in facts.conversation {
        if let Some(Some(topic)) = said.message(id).map(|t| t.and_then(place))
            && !never(topic)
        {
            return Some(Tagged { topic, why: Why::Conversation });
        }
    }
    for (t, topic) in topics.iter().enumerate().filter(|(t, _)| !never(*t)) {
        let leaf = |folder: &str| folder.rsplit('/').next().unwrap_or(folder).trim().to_string();
        if let Some(folder) = topic.seeds.folders.iter().find(|f| facts.folders.iter().any(|g| same(f, g) || same(f, &leaf(g)))) {
            return Some(Tagged { topic: t, why: Why::Folder(folder.clone()) });
        }
        if facts.sender.is_some()
            && let Some(category) = topic.seeds.categories.iter().find(|c| facts.categories.iter().any(|k| *k == crate::porch::category_key(c)))
        {
            return Some(Tagged { topic: t, why: Why::Category(category.clone()) });
        }
    }
    let (p, by) = presets?.find(facts.verified_domain, facts.list_id, facts.is_list)?;
    let id = &presets?.presets[p].id;
    let topic = topics.iter().position(|t| t.preset.as_deref() == Some(id.as_str()))?;
    (!never(topic)).then_some(Tagged { topic, why: Why::Preset(by) })
}

/// The ids a message's conversation names, its first first: its References
/// (oldest first), then its In-Reply-To; bare, each once.
pub fn conversation(references: &[String], in_reply_to: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for value in references.iter().chain(in_reply_to) {
        for id in value.split(|c: char| c.is_whitespace() || c == ',').filter(|t| t.contains('@')).map(crate::mailindex::bare_id) {
            if !id.is_empty() && !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::labels::Act;
    use super::super::{Seeds, Topic};
    use super::*;

    fn topics() -> Vec<Topic> {
        vec![
            Topic { id: "choir".into(), name: "Chorale".into(), seeds: Seeds { folders: vec!["Chorale".into()], categories: vec!["Chœur".into()] }, ..Topic::default() },
            Topic { id: "tax".into(), name: "Impôts".into(), ..Topic::default() },
        ]
    }

    fn said(acts: &[Act]) -> Said {
        Said::of(acts)
    }

    fn tagged(at: i64, id: &str, topic: Option<&str>) -> Act {
        Act::Message { at, message_id: id.into(), topic: topic.map(str::to_string), forget: false }
    }

    fn rule(at: i64, address: &str, topic: &str, rule: Rule) -> Act {
        Act::Sender { at, address: address.into(), topic: topic.into(), rule }
    }

    /// Your word first, your sender's rule second, the conversation third, the seeds last.
    #[test]
    fn the_order_of_words() {
        let topics = topics();
        let folders = vec!["Loisirs/Chorale".to_string()];
        let categories = vec![crate::porch::category_key("chœur")];
        let conversation = vec!["root@example.org".to_string(), "parent@example.org".to_string()];
        let facts = Facts { id: Some("m@example.org"), sender: Some("anne@example.org"), conversation: &conversation, folders: &folders, categories: &categories, ..Facts::default() };
        // The folder seeds it.
        assert_eq!(topic_of(&topics, &said(&[]), &facts, None), Some(Tagged { topic: 0, why: Why::Folder("Chorale".into()) }));
        // Its category too, without the folder; never an unauthenticated sender's.
        let by_card = Facts { folders: &[], ..facts };
        assert_eq!(topic_of(&topics, &said(&[]), &by_card, None).map(|t| t.why), Some(Why::Category("Chœur".into())));
        assert_eq!(topic_of(&topics, &said(&[]), &Facts { sender: None, ..by_card }, None), None);
        // The conversation's first tagged message: the root's.
        let acts = [tagged(1, "parent@example.org", Some("choir")), tagged(2, "root@example.org", Some("tax"))];
        assert_eq!(topic_of(&topics, &said(&acts), &facts, None), Some(Tagged { topic: 1, why: Why::Conversation }));
        // "Always" for the sender before the conversation; "Never" keeps a topic from the rest.
        let acts = [tagged(1, "root@example.org", Some("tax")), rule(2, "anne@example.org", "choir", Rule::Always)];
        assert_eq!(topic_of(&topics, &said(&acts), &facts, None), Some(Tagged { topic: 0, why: Why::Sender }));
        let acts = [tagged(1, "root@example.org", Some("tax")), rule(2, "anne@example.org", "tax", Rule::Never)];
        assert_eq!(topic_of(&topics, &said(&acts), &facts, None).map(|t| t.why), Some(Why::Folder("Chorale".into())));
        // Your word on the message overrules everything, "no topic" included.
        let acts = [rule(1, "anne@example.org", "choir", Rule::Always), tagged(2, "m@example.org", Some("tax"))];
        assert_eq!(topic_of(&topics, &said(&acts), &facts, None), Some(Tagged { topic: 1, why: Why::Yours }));
        let acts = [tagged(2, "m@example.org", None)];
        assert_eq!(topic_of(&topics, &said(&acts), &facts, None), None);
        // A topic removed names nothing.
        let acts = [tagged(2, "m@example.org", Some("gone"))];
        assert_eq!(topic_of(&topics, &said(&acts), &facts, None), None);
    }

    /// A started preset takes a verified sender's mail after your own words
    /// and seeds; "Never" keeps it off; an unstarted preset names nothing.
    #[test]
    fn a_started_preset_takes_proven_mail() {
        let presets: super::super::presets::TopicPresets = serde_json::from_str(r#"{"checked": "2026-11-05", "topics": [
            {"id": "bank", "name": {"en": "Bank"}, "area": "admin", "rules": {"domains": [{"domain": "bank.example", "source": "https://bank.example/"}]}},
            {"id": "newsletters", "name": {"en": "Newsletters"}, "area": "leisure", "rules": {"lists": [{"pattern": "*", "source": "https://www.rfc-editor.org/rfc/rfc2919"}]}}]}"#).unwrap();
        let matcher = super::super::presets::Matcher::new(&presets, &crate::presets::Presets::default());
        let mut topics = topics();
        topics.push(Topic { id: "banque".into(), name: "Banque".into(), preset: Some("bank".into()), ..Topic::default() });
        let facts = Facts { id: Some("m@bank.example"), sender: Some("alerts@bank.example"), verified_domain: Some("bank.example"), ..Facts::default() };
        assert_eq!(topic_of(&topics, &said(&[]), &facts, Some(&matcher)), Some(Tagged { topic: 2, why: Why::Preset(super::super::presets::By::Domain) }));
        assert_eq!(topic_of(&topics, &said(&[]), &Facts { verified_domain: None, ..facts }, Some(&matcher)), None, "unproven: nothing");
        let acts = [rule(1, "alerts@bank.example", "banque", Rule::Never)];
        assert_eq!(topic_of(&topics, &said(&acts), &facts, Some(&matcher)), None);
        // Newsletters is not started: a list takes no topic.
        let list = Facts { id: Some("n@shop.example"), sender: Some("news@shop.example"), verified_domain: Some("shop.example"), is_list: true, ..Facts::default() };
        assert_eq!(topic_of(&topics, &said(&[]), &list, Some(&matcher)), None);
    }

    #[test]
    fn a_conversation_in_order() {
        let ids = conversation(&["<root@x.example> <b@x.example>".into()], &["<b@x.example>".into(), "<c@x.example>".into()]);
        assert_eq!(ids, vec!["root@x.example", "b@x.example", "c@x.example"]);
    }
}
