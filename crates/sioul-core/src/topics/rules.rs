// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A message's topic, and why (docs/topics.md, "The order of words"), the
//! first that speaks:
//!
//! 1. **Your word on the message** (`labels::Act::Message`): its topic, or
//!    none, which nothing below overrules.
//! 2. **Your rule for its sender**: "Always `<topic>` for this sender".
//! 3. **Its conversation**: the topic you gave the first message of it you
//!    tagged (its References, oldest first, then In-Reply-To).
//! 4. **The topic's seeds** (`[[topic]]`'s `seeds`): one of its folders
//!    holds a copy, or its sender is on a card of one of its categories:
//!    your own sorting, before what Sioul ships.
//! 5. **A started preset's rules** (`presets::Matcher::find`): its own
//!    domains and List-Id patterns, then a site's category, then any list;
//!    for a sender Sioul's checks verify only.
//!
//! "Never `<topic>` for this sender" keeps that topic from 3, 4 and 5. A sender
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
    /// The model judged it (`judging`): sure, or maybe (shown only under the topic's filter, "Perhaps").
    Model(super::table::Band),
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

/// A message's topic with the model's word last (docs/topics.md, "The
/// model"): the order of words first (`topic_of`); then, unless you said
/// "no topic" for the message or "Never" for its sender and that topic,
/// the topic the table judged (`judging::Judgments`), sure or maybe, while
/// the topic is still yours.
pub fn with_model(topics: &[Topic], said: &Said, facts: &Facts, presets: Option<&super::presets::Matcher>, judged: Option<&(String, super::table::Band)>) -> Option<Tagged> {
    if let Some(tagged) = topic_of(topics, said, facts, presets) {
        return Some(tagged);
    }
    if facts.id.and_then(|id| said.message(id)).is_some() {
        return None;
    }
    let (id, band) = judged?;
    let topic = topics.iter().position(|t| t.id == *id)?;
    let never = facts.sender.and_then(|s| said.senders.get(s)).is_some_and(|rules| rules.iter().any(|(t, r)| *r == Rule::Never && t == id));
    (!never && *band != super::table::Band::No).then_some(Tagged { topic, why: Why::Model(*band) })
}

/// What a search's "Topic is" reads (`Field::Topic`), loaded once per
/// search, and a folder's rows (`view::folder`), once per look: your topics
/// and your word on them, the presets' rules, what the table judged, the
/// stamps each address trusts, your addresses and your own domains (their
/// ARC seals vouch, as the mailbox reads them), your contacts' categories.
pub struct Lookup {
    topics: Vec<Topic>,
    said: Said,
    presets: std::sync::Arc<super::presets::Matcher>,
    judged: super::judging::Judgments,
    trusted: std::collections::HashMap<String, Vec<String>>,
    own: std::collections::HashSet<String>,
    own_domains: Vec<String>,
    contacts: crate::porch::Senders,
}

impl Lookup {
    pub fn load(config: &crate::config::Config) -> Lookup {
        let sources = config.mail_sources();
        Lookup {
            topics: config.topics.clone(),
            said: Said::load(),
            presets: super::presets::Matcher::current(),
            judged: super::judging::Judgments::load(&super::judging::table_path()),
            trusted: sources.iter().filter_map(|s| Some((s.account.clone()?, s.trusted_ids.clone()))).collect(),
            own: crate::porch::own_addresses(&sources).into_iter().map(|a| a.trim().to_lowercase()).collect(),
            own_domains: config.own_domains(),
            contacts: crate::porch::Senders::load(config),
        }
    }

    /// A stored message's topic by name, as Mail's rows show it (sure; a
    /// doubt is none), from its card, its account and the folder it is in.
    pub fn name_of(&self, card: &crate::card::Card, account: &str, folder: &str) -> Option<String> {
        let (topic, why) = self.tagged(card, account, folder)?;
        (why != Why::Model(super::table::Band::Maybe)).then(|| topic.name.clone())
    }

    /// A stored message's topic and why, a doubt included, as the mailbox
    /// reads it (`mailbox::topic_of`): from its card, its account and the
    /// folder it is in (`folder`, as the folder's name reads).
    pub fn tagged(&self, card: &crate::card::Card, account: &str, folder: &str) -> Option<(&Topic, Why)> {
        use crate::trust::{self, Trust};
        if self.topics.is_empty() {
            return None;
        }
        let trusted = self.trusted.get(account).map(Vec::as_slice).unwrap_or(&[]);
        let results = trust::read_auth_results(&card.headers, trusted);
        let sealers = trust::sealers(trusted, &self.own_domains);
        let excused = card.is_list.then(|| trust::list_excused(results.as_ref(), &card.headers, &sealers)).flatten();
        let (judged, _) = trust::judge_sender(results.as_ref(), excused.as_ref(), card.sender_domain());
        let from = card.from_address.as_deref().map(|a| a.trim().to_lowercase());
        let own = from.as_ref().is_some_and(|f| self.own.contains(f));
        let sender = (!own && judged != Trust::Forged && trust::authenticated(results.as_ref(), &card.headers, &sealers)).then_some(from).flatten();
        let id = card.message_id.as_deref().map(crate::mailindex::bare_id);
        let all = |name: &str| card.headers.all(name).map(str::to_string).collect::<Vec<String>>();
        let conversation = conversation(&all("References"), &all("In-Reply-To"));
        let folders = [folder.to_string()];
        let categories = sender.as_deref().map(|a| self.contacts.contacts.of_address(a)).unwrap_or_default();
        let verified = (!own && judged == Trust::Verified).then(|| card.sender_domain()).flatten();
        let facts = Facts { id: id.as_deref(), sender: sender.as_deref(), conversation: &conversation, folders: &folders, categories: &categories, verified_domain: verified, list_id: None, is_list: card.is_list };
        let judgment = id.as_deref().and_then(|id| self.judged.of(id));
        let tagged = with_model(&self.topics, &self.said, &facts, Some(&self.presets), judgment)?;
        Some((&self.topics[tagged.topic], tagged.why))
    }
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

    /// The model's word comes last: under every rule, never over your "no
    /// topic" nor a sender's "Never"; a topic since removed names nothing.
    #[test]
    fn the_model_speaks_last() {
        use super::super::table::Band;
        let topics = topics();
        let facts = Facts { id: Some("m@example.org"), sender: Some("anne@example.org"), ..Facts::default() };
        let sure = ("tax".to_string(), Band::Sure);
        assert_eq!(with_model(&topics, &said(&[]), &facts, None, Some(&sure)), Some(Tagged { topic: 1, why: Why::Model(Band::Sure) }));
        let maybe = ("tax".to_string(), Band::Maybe);
        assert_eq!(with_model(&topics, &said(&[]), &facts, None, Some(&maybe)).map(|t| t.why), Some(Why::Model(Band::Maybe)));
        let folders = vec!["Chorale".to_string()];
        assert_eq!(with_model(&topics, &said(&[]), &Facts { folders: &folders, ..facts }, None, Some(&sure)).map(|t| t.topic), Some(0), "a seed before the model");
        assert_eq!(with_model(&topics, &said(&[tagged(1, "m@example.org", None)]), &facts, None, Some(&sure)), None, "your no topic");
        assert_eq!(with_model(&topics, &said(&[rule(1, "anne@example.org", "tax", Rule::Never)]), &facts, None, Some(&sure)), None, "Never for the sender");
        assert_eq!(with_model(&topics, &said(&[]), &facts, None, Some(&("gone".to_string(), Band::Sure))), None);
    }

    /// A search's "Topic is" reads a stored message as Mail's rows do: its
    /// sender's rule when Sioul's checks prove it, never a forged one's; the
    /// folder it is in; the table's sure word, never its doubt.
    #[test]
    fn a_search_reads_a_message_s_topic() {
        use super::super::table::Band;
        let lookup = |acts: &[Act], judged: &[(&str, &str, Band)]| Lookup {
            topics: topics(),
            said: said(acts),
            presets: std::sync::Arc::default(),
            judged: super::super::judging::Judgments { table: 1, topics: judged.iter().map(|(id, t, b)| (id.to_string(), (t.to_string(), *b))).collect(), judged: Default::default() },
            trusted: [("work".to_string(), vec!["mx.example.org".to_string()])].into_iter().collect(),
            own: Default::default(),
            own_domains: Vec::new(),
            contacts: crate::porch::Senders::default(),
        };
        let card = |auth: &str| crate::card::Card::from_bytes(format!("Authentication-Results: mx.example.org; {auth}\r\nFrom: Anne <anne@choir.example>\r\nMessage-ID: <m@choir.example>\r\nSubject: Rehearsal\r\n\r\nThursday.\r\n").as_bytes()).unwrap();
        let proven = card("dkim=pass header.d=choir.example; spf=pass smtp.mailfrom=choir.example; dmarc=pass header.from=choir.example");
        let forged = card("dkim=fail header.d=choir.example; spf=fail smtp.mailfrom=choir.example; dmarc=fail header.from=choir.example");
        let always = [rule(1, "anne@choir.example", "choir", Rule::Always)];
        assert_eq!(lookup(&always, &[]).name_of(&proven, "work", "INBOX").as_deref(), Some("Chorale"));
        assert_eq!(lookup(&always, &[]).name_of(&forged, "work", "INBOX"), None, "never a forged sender's rule");
        assert_eq!(lookup(&[], &[]).name_of(&forged, "work", "Loisirs/Chorale").as_deref(), Some("Chorale"), "its folder");
        assert_eq!(lookup(&[], &[("m@choir.example", "tax", Band::Sure)]).name_of(&proven, "work", "INBOX").as_deref(), Some("Impôts"));
        assert_eq!(lookup(&[], &[("m@choir.example", "tax", Band::Maybe)]).name_of(&proven, "work", "INBOX"), None, "a doubt is no topic for a search");
        // A folder's rows (8 November 2026): each received message's topic as
        // the mailbox says it, its why; a doubt none; none in Sent nor Junk.
        let account = crate::config::Account::imap("work", "me@example.net", "imap.example.net", 993, crate::config::Security::Tls, None);
        let tr = crate::i18n::Translator::new("en");
        let mut row = proven.clone();
        row.path = Some(std::path::PathBuf::from("new/1.U1-1.sioul"));
        let words = crate::words::Words::builtin_ref();
        let shown = |acts: &[Act], judged: &[(&str, &str, Band)], role: Option<crate::folders::Role>| {
            let name = match role { Some(crate::folders::Role::Sent) => "Sent", Some(crate::folders::Role::Junk) => "Junk", _ => "INBOX" };
            let folder = crate::folders::folder(name, Some("."), role);
            crate::view::folder(&account, &folder, vec![row.clone()], &[], &[], words, crate::view::FOLDER_PAGE, "", &tr, None, &Default::default(), Some(&lookup(acts, judged))).items.remove(0).topic
        };
        let given = shown(&[tagged(1, "m@choir.example", Some("tax"))], &[], None);
        assert_eq!((given.topic.as_str(), given.topic_id.as_str(), given.topic_why), ("Impôts", "tax", "yours"));
        assert_eq!(shown(&always, &[], None).topic_why, "sender");
        assert!(shown(&[], &[("m@choir.example", "tax", Band::Maybe)], None).topic.is_empty(), "a doubt is no topic in a folder");
        assert!(shown(&always, &[], Some(crate::folders::Role::Sent)).topic.is_empty() && shown(&always, &[], Some(crate::folders::Role::Junk)).topic.is_empty());
        let json = serde_json::to_value(crate::view::folder(&account, &crate::folders::folder("INBOX", Some("."), None), vec![row.clone()], &[], &[], words, 60, "", &tr, None, &Default::default(), Some(&lookup(&always, &[])))).unwrap();
        assert_eq!(json["items"][0]["topic_id"], "choir", "flattened into the row, as the mailbox's: {json}");
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
