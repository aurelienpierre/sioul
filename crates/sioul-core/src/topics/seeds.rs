// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The first topics, proposed from what the person already sorted
//! (docs/topics.md, "How topics start"): their folders, their projects and
//! their contacts' categories, each a seed that names past messages. Nothing
//! is asked: the seeds are rules written before, which the model then learns
//! beyond (docs/research/mail-topics.md, §3).
//!
//! - **Seeds that share senders join** into one topic: a folder and a
//!   category, a folder and a project, projects of the same kind
//!   ("Taxes 2024", "Taxes 2025"). Two folders never join, nor two
//!   categories: the person kept them apart. A topic holds at most one
//!   folder and one category.
//! - **Its name** is the folder's, else the category's, as written; else its
//!   largest project's title, its years left out.
//! - **One topic per message**: a message two topics name goes to the more
//!   specific one, the one whose seeds name fewer messages; the topic says
//!   how many it gave (`Topic::given`).
//! - **A topic exists once it has mail**: at least 20 messages from at
//!   least 3 senders, or 50 from fewer (a single office). Below that, it
//!   waits (`Proposal::waiting`).
//! - **Twelve at most**, the largest; the others are `Proposal::beyond`.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// What a seed comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A folder of the person's (a Gmail label is one), not a system folder.
    Folder,
    /// A category of their contacts' cards (vCard CATEGORIES: Nextcloud's groups).
    Category,
    /// A project of the notes folder: its routes, its ties, its conversations.
    Project,
}

/// One seed: what it comes from, its name as the person wrote it, and the
/// messages it names (indices into the mail given to `propose`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seed {
    pub kind: Kind,
    pub name: String,
    pub messages: Vec<usize>,
}

/// One message as the seeds see it: its sender's address, lowercase; none when unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mail {
    pub sender: Option<String>,
}

/// The rules of `propose`, with their defaults (docs/topics.md).
#[derive(Debug, Clone, PartialEq)]
pub struct Rules {
    /// A topic needs this many messages…
    pub least: usize,
    /// …from this many senders at least…
    pub senders: usize,
    /// …or this many, from any number of senders.
    pub alone: usize,
    /// Topics proposed at most.
    pub most: usize,
    /// Two seeds join when this share of the smaller one's messages comes
    /// from senders the other names too.
    pub overlap: f64,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { least: 20, senders: 3, alone: 50, most: 12, overlap: 0.5 }
    }
}

/// A seed as a topic says it: its kind, its name, how many messages it named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Named {
    pub kind: Kind,
    pub name: String,
    pub messages: usize,
}

/// A topic proposed, or waiting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Topic {
    /// ASCII, unique among the proposal's (`topics::slug`).
    pub id: String,
    pub name: String,
    /// Its seeds, the largest first.
    pub seeds: Vec<Named>,
    /// Its messages, once each message went to one topic (indices, ascending);
    /// those its seeds name, for a topic that waits.
    #[serde(skip)]
    pub messages: Vec<usize>,
    /// How many: `messages`' length.
    pub count: usize,
    /// Their senders, each once.
    pub senders: usize,
    /// Messages another topic named too, which went to this one, the more specific.
    pub kept: usize,
    /// Messages its seeds named that went to a more specific topic, by that topic's id.
    pub given: BTreeMap<String, usize>,
}

/// The topics proposed, those waiting for more mail, those beyond the twelve.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Proposal {
    pub topics: Vec<Topic>,
    pub waiting: Vec<Topic>,
    pub beyond: Vec<Topic>,
}

impl Proposal {
    /// Each of `n` messages' topic (an index into `topics`), none for most.
    pub fn labels(&self, n: usize) -> Vec<Option<usize>> {
        let mut labels = vec![None; n];
        for (t, topic) in self.topics.iter().enumerate() {
            for &m in &topic.messages {
                if let Some(label) = labels.get_mut(m) {
                    *label = Some(t);
                }
            }
        }
        labels
    }
}

/// A group of seeds that joined, while it is proposed.
struct Group {
    seeds: Vec<usize>,
    /// The messages its seeds name, each once.
    claimed: BTreeSet<usize>,
}

/// Proposes topics from the seeds (see the module): `mail` is every message
/// the seeds may name, by index.
pub fn propose(mail: &[Mail], seeds: Vec<Seed>, rules: &Rules) -> Proposal {
    let seeds: Vec<Seed> = seeds
        .into_iter()
        .map(|mut s| {
            s.messages.retain(|&m| m < mail.len());
            s.messages.sort_unstable();
            s.messages.dedup();
            s.name = s.name.trim().to_string();
            s
        })
        .filter(|s| !s.messages.is_empty() && !s.name.is_empty())
        .collect();
    let groups = joined(mail, &seeds, rules.overlap);
    let mut active: Vec<bool> = vec![true; groups.len()];
    let passes = |messages: &[usize]| {
        let count = messages.len();
        count >= rules.alone || (count >= rules.least && senders_of(mail, messages) >= rules.senders)
    };
    // A group that cannot pass even with every message its seeds name waits at once;
    // then the smallest that fails, one at a time, its messages back to the others.
    for (g, group) in groups.iter().enumerate() {
        active[g] = passes(&group.claimed.iter().copied().collect::<Vec<_>>());
    }
    loop {
        let owned = assign(&groups, &active);
        let failing = (0..groups.len()).filter(|&g| active[g] && !passes(&owned[g])).min_by_key(|&g| (owned[g].len(), g));
        match failing {
            Some(g) => active[g] = false,
            None => break,
        }
    }
    // The largest first; twelve proposed, the rest beyond.
    let owned = assign(&groups, &active);
    let mut passing: Vec<usize> = (0..groups.len()).filter(|&g| active[g]).collect();
    passing.sort_by(|&a, &b| owned[b].len().cmp(&owned[a].len()).then_with(|| name_of(&seeds, &groups[a]).cmp(&name_of(&seeds, &groups[b]))));
    let kept: Vec<usize> = passing.iter().copied().take(rules.most).collect();
    let beyond_groups: Vec<usize> = passing.iter().copied().skip(rules.most).collect();
    let mut chosen = vec![false; groups.len()];
    for &g in &kept {
        chosen[g] = true;
    }
    let owned = assign(&groups, &chosen);
    let mut waiting_groups: Vec<usize> = (0..groups.len()).filter(|&g| !active[g]).collect();
    waiting_groups.sort_by(|&a, &b| groups[b].claimed.len().cmp(&groups[a].claimed.len()).then_with(|| name_of(&seeds, &groups[a]).cmp(&name_of(&seeds, &groups[b]))));

    let mut taken: Vec<String> = Vec::new();
    let mut ids: HashMap<usize, String> = HashMap::new();
    for &g in kept.iter().chain(&beyond_groups).chain(&waiting_groups) {
        let id = super::slug(&name_of(&seeds, &groups[g]), &taken);
        taken.push(id.clone());
        ids.insert(g, id);
    }
    let topic = |g: usize, messages: Vec<usize>, kept_count: usize, given: BTreeMap<String, usize>| {
        let mut named: Vec<Named> = groups[g].seeds.iter().map(|&s| Named { kind: seeds[s].kind, name: seeds[s].name.clone(), messages: seeds[s].messages.len() }).collect();
        named.sort_by(|a, b| b.messages.cmp(&a.messages).then_with(|| a.kind.cmp(&b.kind)).then_with(|| a.name.cmp(&b.name)));
        Topic { id: ids[&g].clone(), name: name_of(&seeds, &groups[g]), seeds: named, count: messages.len(), senders: senders_of(mail, &messages), messages, kept: kept_count, given }
    };
    let claims = claims_of(&groups, &chosen);
    let topics = kept
        .iter()
        .map(|&g| {
            let kept_count = owned[g].iter().filter(|m| claims.get(*m).is_some_and(|c| c.len() > 1)).count();
            let mut given: BTreeMap<String, usize> = BTreeMap::new();
            for m in groups[g].claimed.iter().filter(|m| !owned[g].contains(m)) {
                if let Some(&to) = claims.get(m).and_then(|c| c.iter().find(|&&o| owned[o].binary_search(m).is_ok())) {
                    *given.entry(ids[&to].clone()).or_default() += 1;
                }
            }
            topic(g, owned[g].clone(), kept_count, given)
        })
        .collect();
    let all = |g: usize| groups[g].claimed.iter().copied().collect::<Vec<usize>>();
    Proposal {
        topics,
        waiting: waiting_groups.iter().map(|&g| topic(g, all(g), 0, BTreeMap::new())).collect(),
        beyond: beyond_groups.iter().map(|&g| topic(g, all(g), 0, BTreeMap::new())).collect(),
    }
}

/// Distinct senders of some messages (unknown ones aside).
fn senders_of(mail: &[Mail], messages: &[usize]) -> usize {
    messages.iter().filter_map(|&m| mail[m].sender.as_deref()).collect::<HashSet<&str>>().len()
}

/// The seeds joined into groups: by shared senders, the closest pairs first,
/// never two folders nor two categories in one group.
fn joined(mail: &[Mail], seeds: &[Seed], overlap: f64) -> Vec<Group> {
    let senders: Vec<HashSet<&str>> = seeds.iter().map(|s| s.messages.iter().filter_map(|&m| mail[m].sender.as_deref()).collect()).collect();
    let shared = |small: usize, large: usize| -> f64 {
        let n = seeds[small].messages.len();
        let from_large = seeds[small].messages.iter().filter(|&&m| mail[m].sender.as_deref().is_some_and(|s| senders[large].contains(s))).count();
        from_large as f64 / n.max(1) as f64
    };
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for a in 0..seeds.len() {
        for b in a + 1..seeds.len() {
            let (ka, kb) = (seeds[a].kind, seeds[b].kind);
            if ka == kb && ka != Kind::Project {
                continue;
            }
            let (small, large) = if seeds[a].messages.len() <= seeds[b].messages.len() { (a, b) } else { (b, a) };
            let share = shared(small, large);
            if share >= overlap {
                pairs.push((share, a, b));
            }
        }
    }
    pairs.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| (x.1, x.2).cmp(&(y.1, y.2))));
    let mut group_of: Vec<usize> = (0..seeds.len()).collect();
    let mut members: Vec<Vec<usize>> = (0..seeds.len()).map(|s| vec![s]).collect();
    for (_, a, b) in pairs {
        let (ga, gb) = (group_of[a], group_of[b]);
        if ga == gb {
            continue;
        }
        let count = |kind: Kind| members[ga].iter().chain(&members[gb]).filter(|&&s| seeds[s].kind == kind).count();
        if count(Kind::Folder) > 1 || count(Kind::Category) > 1 {
            continue;
        }
        let moved = std::mem::take(&mut members[gb]);
        for &s in &moved {
            group_of[s] = ga;
        }
        members[ga].extend(moved);
    }
    members
        .into_iter()
        .filter(|m| !m.is_empty())
        .map(|mut seeds_of| {
            seeds_of.sort_unstable();
            let claimed = seeds_of.iter().flat_map(|&s| seeds[s].messages.iter().copied()).collect();
            Group { seeds: seeds_of, claimed }
        })
        .collect()
}

/// The groups among `active` that claim each message.
fn claims_of(groups: &[Group], active: &[bool]) -> HashMap<usize, Vec<usize>> {
    let mut claims: HashMap<usize, Vec<usize>> = HashMap::new();
    for (g, group) in groups.iter().enumerate().filter(|(g, _)| active[*g]) {
        for &m in &group.claimed {
            claims.entry(m).or_default().push(g);
        }
    }
    claims
}

/// Each group's messages, once each message went to the most specific of
/// the active groups that claim it (the fewest claimed messages; then the first).
fn assign(groups: &[Group], active: &[bool]) -> Vec<Vec<usize>> {
    let mut owned: Vec<Vec<usize>> = vec![Vec::new(); groups.len()];
    let mut claims: Vec<(usize, Vec<usize>)> = claims_of(groups, active).into_iter().collect();
    claims.sort_unstable_by_key(|(m, _)| *m);
    for (m, by) in claims {
        if let Some(&g) = by.iter().min_by_key(|&&g| (groups[g].claimed.len(), g)) {
            owned[g].push(m);
        }
    }
    owned
}

/// A group's name: its folder's, else its category's, as written; else its
/// largest project's title without its years ("Taxes 2025" → "Taxes").
fn name_of(seeds: &[Seed], group: &Group) -> String {
    let first = |kind: Kind| group.seeds.iter().map(|&s| &seeds[s]).filter(|s| s.kind == kind).max_by(|a, b| a.messages.len().cmp(&b.messages.len()).then_with(|| b.name.cmp(&a.name)));
    if let Some(seed) = first(Kind::Folder).or_else(|| first(Kind::Category)) {
        return seed.name.clone();
    }
    let title = first(Kind::Project).map_or("", |s| s.name.as_str());
    let years = |word: &str| !word.is_empty() && word.chars().all(|c| c.is_ascii_digit() || matches!(c, '-' | '/' | '–' | '.'));
    let kept: Vec<&str> = title.split_whitespace().filter(|w| !years(w)).collect();
    if kept.is_empty() { title.to_string() } else { kept.join(" ").trim_end_matches(['-', '–', ',', ':']).trim().to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` messages from `senders` senders on one domain, after `start`.
    fn mail_from(mail: &mut Vec<Mail>, n: usize, senders: usize, domain: &str) -> Vec<usize> {
        let start = mail.len();
        for i in 0..n {
            mail.push(Mail { sender: Some(format!("s{}@{domain}", i % senders.max(1))) });
        }
        (start..start + n).collect()
    }

    fn seed(kind: Kind, name: &str, messages: &[usize]) -> Seed {
        Seed { kind, name: name.into(), messages: messages.to_vec() }
    }

    /// A folder and a category of the same senders join, named after the
    /// folder; two folders never join; a project's years leave its name.
    #[test]
    fn seeds_that_share_senders_join() {
        let mut mail = Vec::new();
        let choir = mail_from(&mut mail, 40, 6, "chorale.example");
        let tax = mail_from(&mut mail, 30, 2, "impots.example");
        let tax_more = mail_from(&mut mail, 30, 2, "impots.example");
        let seeds = vec![
            seed(Kind::Folder, "Chorale", &choir[..30]),
            seed(Kind::Category, "Choir friends", &choir[10..]),
            seed(Kind::Project, "Taxes 2024", &tax),
            seed(Kind::Project, "Taxes 2025", &tax_more),
            // Another folder of the same senders as the first: kept apart.
            seed(Kind::Folder, "Concerts", &choir[20..]),
        ];
        let proposal = propose(&mail, seeds, &Rules::default());
        let names: Vec<&str> = proposal.topics.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Taxes", "Chorale", "Concerts"], "{proposal:#?}");
        let taxes = &proposal.topics[0];
        assert_eq!((taxes.count, taxes.senders, taxes.id.as_str()), (60, 2, "taxes"));
        // The folder and the category joined; "Concerts", a folder, stood apart and,
        // more specific, kept the 20 messages both name.
        let chorale = &proposal.topics[1];
        assert_eq!(chorale.seeds.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![Kind::Folder, Kind::Category]);
        assert_eq!((chorale.count, chorale.given.get("concerts")), (20, Some(&20)));
        assert_eq!(proposal.topics[2].kept, 20);
        let labels = proposal.labels(mail.len());
        assert_eq!(labels.iter().filter(|l| l.is_some()).count(), 100);
        assert!(choir[20..].iter().all(|&m| labels[m] == Some(2)));
    }

    /// One message, one topic: the more specific one keeps it, and the
    /// larger says how many it gave.
    #[test]
    fn a_message_two_topics_name_goes_to_the_more_specific() {
        let mut mail = Vec::new();
        let health = mail_from(&mut mail, 100, 10, "sante.example");
        let dentist = mail_from(&mut mail, 30, 3, "dentiste.example");
        let mut wide = health.clone();
        wide.extend(&dentist);
        // Two labels on the same messages (Gmail): two folders, which never join.
        let seeds = vec![seed(Kind::Folder, "Santé", &wide), seed(Kind::Folder, "Dentiste", &dentist)];
        let proposal = propose(&mail, seeds, &Rules::default());
        assert_eq!(proposal.topics.len(), 2, "{proposal:#?}");
        let sante = proposal.topics.iter().find(|t| t.name == "Santé").unwrap();
        let dentiste = proposal.topics.iter().find(|t| t.name == "Dentiste").unwrap();
        assert_eq!((sante.count, dentiste.count, sante.id.as_str()), (100, 30, "sante"));
        assert_eq!(sante.given.get("dentiste"), Some(&30));
        assert_eq!(dentiste.kept, 30);
        let labels = proposal.labels(mail.len());
        let at = proposal.topics.iter().position(|t| t.id == "dentiste");
        assert!(dentist.iter().all(|&m| labels[m] == at));
        // A category inside the folder, of the same senders: one topic.
        let seeds = vec![seed(Kind::Folder, "Santé", &wide), seed(Kind::Category, "Dentiste", &dentist)];
        let proposal = propose(&mail, seeds, &Rules::default());
        assert_eq!(proposal.topics.len(), 1);
        assert_eq!((proposal.topics[0].name.as_str(), proposal.topics[0].count), ("Santé", 130));
    }

    /// Below 20 messages, or 3 senders under 50, a topic waits; 50 from one office pass.
    #[test]
    fn a_topic_needs_mail() {
        let mut mail = Vec::new();
        let small = mail_from(&mut mail, 12, 4, "club.example");
        let two = mail_from(&mut mail, 30, 2, "two.example");
        let office = mail_from(&mut mail, 50, 1, "office.example");
        let seeds = vec![seed(Kind::Folder, "Book club", &small), seed(Kind::Folder, "Pair", &two), seed(Kind::Category, "Office", &office)];
        let proposal = propose(&mail, seeds, &Rules::default());
        assert_eq!(proposal.topics.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["Office"]);
        assert_eq!(proposal.waiting.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["Pair", "Book club"]);
        assert_eq!(proposal.waiting[1].count, 12);
    }

    /// Twelve at most, the largest; a waiting topic's messages return to the others.
    #[test]
    fn twelve_at_most_and_messages_return() {
        let mut mail = Vec::new();
        let mut seeds = Vec::new();
        for t in 0..14 {
            let messages = mail_from(&mut mail, 30 + t, 5, &format!("t{t}.example"));
            seeds.push(seed(Kind::Folder, &format!("Folder {t:02}"), &messages));
        }
        let proposal = propose(&mail, seeds, &Rules::default());
        assert_eq!(proposal.topics.len(), 12);
        assert_eq!(proposal.beyond.len(), 2);
        assert_eq!(proposal.topics[0].name, "Folder 13");
        assert_eq!(proposal.beyond.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["Folder 01", "Folder 00"]);

        // A small category inside a large folder, too small to stand: the folder keeps all.
        let mut mail = Vec::new();
        let all = mail_from(&mut mail, 60, 6, "big.example");
        let seeds = vec![seed(Kind::Folder, "Big", &all), seed(Kind::Folder, "Tiny", &all[..5])];
        let proposal = propose(&mail, seeds, &Rules::default());
        assert_eq!(proposal.topics.len(), 1);
        assert_eq!(proposal.topics[0].count, 60);
        assert_eq!(proposal.waiting[0].name, "Tiny");
    }
}
