// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Groups Sioul noticed (docs/topics.md, "Groups"): the mail kept here
//! (spam and forged mail out, projects' mail in, as `topics::read` reads it)
//! gathered four ways, side by side, for the person to name later, a group
//! named being tagged in one act (`sioul topics groups`). Nothing is
//! written: this measures which way gives groups a person recognises.
//!
//! 1. **Senders** (`senders`): the graph of the correspondents Sioul's checks
//!    proved (and yours: the people you write to), two of them tied each
//!    time they meet in a conversation or among one message's recipients;
//!    its communities by Louvain's modularity (Blondel et al. 2008). A
//!    group is the mail of a community's senders.
//! 2. **Model2Vec and k-means** (`model2vec`, with a Model2Vec folder): each
//!    message's static embedding (`model2vec`), unit length, gathered by
//!    k-means (k-means++ seeds), k chosen by the silhouette from `k_min` to
//!    `k_max` on a sample.
//! 3. **LDA** (`lda`): latent Dirichlet allocation by collapsed Gibbs
//!    sampling (Griffiths & Steyvers 2004), a message's group its most
//!    likely topic. A baseline.
//! 4. **NMF** (`nmf`): non-negative factorisation of the messages' TF-IDF by
//!    multiplicative updates (Lee & Seung 2001). A baseline.
//!
//! The words groups are made of are each message's subject and own text,
//! what it quotes, its attributions and its signature left out, without the
//! tokenizer's placeholders nor HTML's and CSS's words
//! (`topics::Mail::grouping`). NMF is the way "Groups Sioul noticed" shows
//! (docs/topics.md): its groups read as categories a person recognises.
//!
//! The words that say each group: c-TF-IDF for the first two (each group's
//! words weighed against the others', as BERTopic does), each model's own
//! for the last two. For each group: its size, its senders, its words, two
//! subjects, its coherence (NPMI of its words over every message: Bouma
//! 2009; Röder et al. 2015) and the seed topic most of its mail is in.

use crate::model2vec::StaticModel;
use crate::topics::{Mail, Read};
use crate::{Cancel, LearnError, Progress, Rng, Stage};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

/// How groups are made; `Default` holds docs/topics.md's values.
#[derive(Debug, Clone)]
pub struct Options {
    /// Groups smaller than this are not shown.
    pub least: usize,
    /// Groups shown per way at most, the largest.
    pub most: usize,
    /// k-means' k is chosen in `k_min..=k_max`.
    pub k_min: usize,
    pub k_max: usize,
    /// LDA's and NMF's topics: this, else k-means' k, else 10.
    pub k: Option<usize>,
    /// LDA's sweeps and NMF's updates.
    pub lda_iterations: usize,
    pub nmf_iterations: usize,
    /// Words kept: the most common, seen in at least `min_df` messages and in at most half.
    pub vocabulary: usize,
    pub min_df: usize,
    /// Words a message brings to LDA at most.
    pub doc_words: usize,
    /// Words that say a group.
    pub top_words: usize,
    /// Messages the silhouette is measured on at most.
    pub sample: usize,
    pub seed: u64,
    /// NMF alone: "Groups Sioul noticed" in the window.
    pub nmf_only: bool,
    /// Messages left out of every group, by bare Message-ID: those you gave a topic.
    pub leave_out: HashSet<String>,
    /// The sites offered (`presets`), to name a group by its senders' sites;
    /// none: named after its most common sender alone.
    pub presets: Option<sioul_core::presets::Presets>,
    /// The language a category's word is said in ("fr": "Banque").
    pub language: String,
}

impl Default for Options {
    fn default() -> Self {
        Options { least: 10, most: 30, k_min: 4, k_max: 16, k: None, lda_iterations: 200, nmf_iterations: 100, vocabulary: 5000, min_df: 5, doc_words: 200, top_words: 10, sample: 2000, seed: 1, nmf_only: false, leave_out: HashSet::new(), presets: None, language: "en".into() }
    }
}

/// One group, as shown.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Group {
    pub size: u64,
    /// Its distinct correspondents.
    pub senders: u64,
    /// Of its messages, yours.
    pub yours: u64,
    pub words: Vec<String>,
    /// Two of its subjects, its most typical messages' (as written: the
    /// command line masks codes and links before it shows them).
    pub subjects: Vec<String>,
    /// NPMI of its words, from -1 to 1.
    pub coherence: Option<f64>,
    /// The seed topic most of its messages are in (by id), and that share of the group.
    pub seed: Option<(String, f64)>,
    /// Its messages' bare Message-IDs ("Name it" tags them): never shown nor written.
    #[serde(skip)]
    pub ids: Vec<String>,
    /// Its most frequent senders' names, the most frequent first (three at most).
    pub names: Vec<String>,
    /// A name proposed for it, and how it was found: "sites" (most of its
    /// senders' proven domains are sites of one category: that category's
    /// word), "sender" (its most common sender's name); "" without one.
    pub proposed: String,
    pub proposed_by: &'static str,
}

/// One way's groups.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Way {
    /// "senders", "model2vec", "lda", "nmf".
    pub name: String,
    /// Its groups in all (communities for the senders), before the small ones are left aside.
    pub found: u64,
    pub k: u64,
    /// k-means' silhouette at its k.
    pub silhouette: Option<f64>,
    pub groups: Vec<Group>,
    /// Messages in the groups shown.
    pub grouped: u64,
    /// The groups' mean coherence.
    pub coherence: Option<f64>,
    /// Of the seeded messages in the groups shown, the share whose group's main seed is theirs.
    pub agreement: Option<f64>,
    /// Why it made no groups ("no-model", "no-senders", "too-few").
    pub skipped: Option<String>,
    pub seconds: f64,
}

/// What `sioul topics groups` says.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Groups {
    /// Messages read, and those with words enough to group.
    pub messages: u64,
    pub with_words: u64,
    /// Words kept to group by.
    pub vocabulary: u64,
    pub ways: Vec<Way>,
    pub seconds: f64,
}

/// The messages as bags of kept words.
struct Docs {
    /// Each message's kept words, as ids, in order (repeats kept); empty for a message without any.
    words: Vec<Vec<u32>>,
    vocabulary: Vec<String>,
    /// Each word's messages, ascending (each once).
    postings: Vec<Vec<u32>>,
}

/// A word worth grouping by: three characters or more, a letter among them,
/// no sign of a feature or a placeholder (`topics::Mail::grouping` has none already).
fn worth(word: &str) -> bool {
    word.chars().count() >= 3 && word.chars().any(char::is_alphabetic) && !word.contains(':') && !word.starts_with('_')
}

fn docs(mail: &[Mail], options: &Options) -> Docs {
    let left_out = |m: &Mail| m.id.as_deref().is_some_and(|id| options.leave_out.contains(id));
    let mut df: HashMap<&str, usize> = HashMap::new();
    for m in mail.iter().filter(|m| !left_out(m)) {
        let unique: HashSet<&str> = m.grouping.iter().map(String::as_str).filter(|w| worth(w)).collect();
        for w in unique {
            *df.entry(w).or_default() += 1;
        }
    }
    let half = (mail.len() / 2).max(1);
    let mut kept: Vec<(&str, usize)> = df.into_iter().filter(|&(_, n)| n >= options.min_df && n <= half).collect();
    kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    kept.truncate(options.vocabulary);
    let vocabulary: Vec<String> = kept.iter().map(|(w, _)| w.to_string()).collect();
    let id: HashMap<&str, u32> = vocabulary.iter().enumerate().map(|(i, w)| (w.as_str(), i as u32)).collect();
    let words: Vec<Vec<u32>> = mail.iter().map(|m| if left_out(m) { Vec::new() } else { m.grouping.iter().filter_map(|w| id.get(w.as_str()).copied()).collect() }).collect();
    let mut postings = vec![Vec::new(); vocabulary.len()];
    for (d, ws) in words.iter().enumerate() {
        let unique: std::collections::BTreeSet<u32> = ws.iter().copied().collect();
        for w in unique {
            postings[w as usize].push(d as u32);
        }
    }
    Docs { words, vocabulary, postings }
}

/// NPMI of some words over the messages (Bouma 2009): each pair's
/// `ln(p(a,b) / (p(a) p(b))) / -ln p(a,b)`, -1 for words never together; their mean.
fn npmi(docs: &Docs, words: &[u32]) -> Option<f64> {
    let n = docs.words.len() as f64;
    let mut sum = 0.0;
    let mut pairs = 0usize;
    for (i, &a) in words.iter().enumerate() {
        for &b in &words[i + 1..] {
            let (pa, pb) = (&docs.postings[a as usize], &docs.postings[b as usize]);
            let both = intersection(pa, pb) as f64;
            sum += if both == 0.0 {
                -1.0
            } else {
                let p = both / n;
                let value = (p / ((pa.len() as f64 / n) * (pb.len() as f64 / n))).ln() / -p.ln();
                if value.is_finite() { value } else { 1.0 }
            };
            pairs += 1;
        }
    }
    (pairs > 0).then(|| sum / pairs as f64)
}

fn intersection(a: &[u32], b: &[u32]) -> usize {
    let (mut i, mut j, mut n) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                n += 1;
                i += 1;
                j += 1;
            }
        }
    }
    n
}

/// Each group's words by c-TF-IDF: its words' frequency in it, weighed by
/// `ln(1 + A / f(w))`, A the mean words per group, f(w) the word's count in all.
fn ctfidf(docs: &Docs, assigned: &[Option<usize>], groups: usize, top: usize) -> Vec<Vec<u32>> {
    let v = docs.vocabulary.len();
    let mut tf = vec![vec![0f64; v]; groups];
    let mut total = vec![0f64; v];
    for (d, g) in assigned.iter().enumerate() {
        if let Some(g) = *g {
            for &w in &docs.words[d] {
                tf[g][w as usize] += 1.0;
                total[w as usize] += 1.0;
            }
        }
    }
    let a = tf.iter().map(|t| t.iter().sum::<f64>()).sum::<f64>() / groups.max(1) as f64;
    tf.iter()
        .map(|t| {
            let words: f64 = t.iter().sum::<f64>().max(1.0);
            let mut scored: Vec<(u32, f64)> = (0..v).filter(|&w| t[w] > 0.0).map(|w| (w as u32, t[w] / words * (1.0 + a / total[w]).ln())).collect();
            scored.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal).then(x.0.cmp(&y.0)));
            scored.into_iter().take(top).map(|(w, _)| w).collect()
        })
        .collect()
}

/// A way's groups as shown, from each message's group and how typical it is of it.
#[allow(clippy::too_many_arguments)]
fn summed(name: &str, read: &Read, docs: &Docs, assigned: &[Option<usize>], typical: &[f64], words: &[Vec<u32>], labels: &[Option<usize>], ids: &[String], options: &Options) -> Way {
    let groups = words.len();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); groups];
    for (d, g) in assigned.iter().enumerate() {
        if let Some(g) = *g {
            members[g].push(d);
        }
    }
    let found = members.iter().filter(|m| !m.is_empty()).count() as u64;
    let mut order: Vec<usize> = (0..groups).filter(|&g| members[g].len() >= options.least).collect();
    order.sort_by(|&a, &b| members[b].len().cmp(&members[a].len()).then(a.cmp(&b)));
    order.truncate(options.most);
    let (mut right, mut seeded) = (0u64, 0u64);
    let mut shown = Vec::new();
    let sites = options.presets.as_ref().map(Sites::of);
    for g in order {
        let m = &members[g];
        let mut by_seed: BTreeMap<usize, u64> = BTreeMap::new();
        for &d in m {
            if let Some(t) = labels[d] {
                *by_seed.entry(t).or_default() += 1;
            }
        }
        let main = by_seed.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(&t, &n)| (t, n));
        for &d in m {
            if let Some(t) = labels[d] {
                seeded += 1;
                right += u64::from(main.is_some_and(|(mt, _)| mt == t));
            }
        }
        let mut best: Vec<usize> = m.clone();
        best.sort_by(|&a, &b| typical[b].partial_cmp(&typical[a]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
        let mut subjects: Vec<String> = Vec::new();
        for &d in &best {
            let subject = masked(read.mail[d].subject.trim());
            if !subject.is_empty() && !subjects.contains(&subject) {
                subjects.push(subject);
            }
            if subjects.len() == 2 {
                break;
            }
        }
        let names = names_of(read, m);
        let (proposed, proposed_by) = match sites.as_ref().and_then(|sites| site_word(read, m, sites, options)) {
            Some(word) => (word, "sites"),
            None => names.first().map_or((String::new(), ""), |n| (n.clone(), "sender")),
        };
        shown.push(Group {
            size: m.len() as u64,
            senders: m.iter().filter_map(|&d| read.mail[d].sender.as_deref()).collect::<HashSet<_>>().len() as u64,
            yours: m.iter().filter(|&&d| read.mail[d].yours).count() as u64,
            words: words[g].iter().map(|&w| docs.vocabulary[w as usize].clone()).collect(),
            subjects,
            coherence: npmi(docs, &words[g]),
            seed: main.map(|(t, n)| (ids.get(t).cloned().unwrap_or_default(), n as f64 / m.len() as f64)),
            ids: m.iter().filter_map(|&d| read.mail[d].id.clone()).collect(),
            names,
            proposed,
            proposed_by,
        });
    }
    let coherences: Vec<f64> = shown.iter().filter_map(|g| g.coherence).collect();
    Way {
        name: name.to_string(),
        found,
        k: groups as u64,
        grouped: shown.iter().map(|g| g.size).sum(),
        coherence: (!coherences.is_empty()).then(|| coherences.iter().sum::<f64>() / coherences.len() as f64),
        agreement: (seeded > 0).then(|| right as f64 / seeded as f64),
        groups: shown,
        ..Way::default()
    }
}

/// Groups the mail four ways (see the module); `labels`, each message's
/// seed topic (`topics::seeded`), by `ids`.
pub fn groups(read: &Read, labels: &[Option<usize>], ids: &[String], model: Option<&StaticModel>, options: &Options, progress: &mut dyn FnMut(&Progress), cancel: &Cancel) -> Result<Groups, LearnError> {
    let started = std::time::Instant::now();
    let check = || if cancel.cancelled() { Err(LearnError::Cancelled) } else { Ok(()) };
    let docs = docs(&read.mail, options);
    let with_words = docs.words.iter().filter(|w| !w.is_empty()).count() as u64;
    let mut out = Groups { messages: read.mail.len() as u64, with_words, vocabulary: docs.vocabulary.len() as u64, ..Groups::default() };
    let step = |name: &str, progress: &mut dyn FnMut(&Progress)| progress(&Progress { stage: Stage::Evaluation, done: 0, total: 0, detail: name.to_string() });

    if options.nmf_only {
        let k = options.k.unwrap_or(10).max(2);
        step("nmf", progress);
        let at = std::time::Instant::now();
        let mut way = if with_words as usize >= 2 * k && docs.vocabulary.len() >= k { nmf(read, &docs, k, labels, ids, options, cancel)? } else { Way { name: "nmf".into(), skipped: Some("too-few".into()), ..Way::default() } };
        way.seconds = at.elapsed().as_secs_f64();
        out.ways.push(way);
        out.seconds = started.elapsed().as_secs_f64();
        return Ok(out);
    }
    // 1. Senders.
    step("senders", progress);
    let at = std::time::Instant::now();
    let mut way = senders(read, &docs, labels, ids, options);
    way.seconds = at.elapsed().as_secs_f64();
    out.ways.push(way);
    check()?;

    // 2. Model2Vec and k-means.
    step("model2vec", progress);
    let at = std::time::Instant::now();
    let mut chosen_k = None;
    let mut way = match model {
        None => Way { name: "model2vec".into(), skipped: Some("no-model".into()), ..Way::default() },
        Some(model) => {
            let (way, k) = kmeans_way(read, &docs, model, labels, ids, options, cancel)?;
            chosen_k = k;
            way
        }
    };
    way.seconds = at.elapsed().as_secs_f64();
    out.ways.push(way);
    check()?;

    let k = options.k.or(chosen_k).unwrap_or(10).max(2);
    let enough = with_words as usize >= 2 * k && docs.vocabulary.len() >= k;
    // 3. LDA.
    step("lda", progress);
    let at = std::time::Instant::now();
    let mut way = if enough { lda(read, &docs, k, labels, ids, options, cancel)? } else { Way { name: "lda".into(), skipped: Some("too-few".into()), ..Way::default() } };
    way.seconds = at.elapsed().as_secs_f64();
    out.ways.push(way);
    check()?;
    // 4. NMF.
    step("nmf", progress);
    let at = std::time::Instant::now();
    let mut way = if enough { nmf(read, &docs, k, labels, ids, options, cancel)? } else { Way { name: "nmf".into(), skipped: Some("too-few".into()), ..Way::default() } };
    way.seconds = at.elapsed().as_secs_f64();
    out.ways.push(way);
    out.seconds = started.elapsed().as_secs_f64();
    Ok(out)
}

/// A subject as a list of groups shows it: a link, and a word of four digits
/// or more (a code, an account number), each left out ("…").
fn masked(subject: &str) -> String {
    subject
        .split_whitespace()
        .map(|w| if w.contains("://") || w.starts_with("www.") || w.chars().filter(char::is_ascii_digit).count() >= 4 { "…" } else { w })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A group's most frequent senders' names (their display names, else their
/// addresses), the most frequent first, three at most.
fn names_of(read: &Read, members: &[usize]) -> Vec<String> {
    let mut count: HashMap<String, usize> = HashMap::new();
    for &d in members {
        let m = &read.mail[d];
        if let Some(name) = m.from_name.clone().or_else(|| (!m.yours).then(|| m.sender.clone()).flatten()) {
            *count.entry(name).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = count.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.into_iter().take(3).map(|(n, _)| n).collect()
}

/// The sites offered, by the domains their mail comes from (`announced_by`)
/// and their own, each with its categories.
struct Sites<'a> {
    by_domain: HashMap<String, Vec<&'a str>>,
    presets: &'a sioul_core::presets::Presets,
}

impl<'a> Sites<'a> {
    fn of(presets: &'a sioul_core::presets::Presets) -> Sites<'a> {
        let mut by_domain: HashMap<String, Vec<&'a str>> = HashMap::new();
        let every = presets.international.iter().chain(presets.countries.values().flat_map(|c| c.sites.iter().chain(c.regions.values().flat_map(|r| r.sites.iter()))));
        for preset in every.filter(|p| !p.tags.is_empty()) {
            let host = preset.url.split("://").nth(1).unwrap_or("").split(['/', ':', '?']).next().unwrap_or("").trim_start_matches("www.").to_lowercase();
            for domain in preset.announced_by.iter().map(|d| d.trim().trim_start_matches('@').to_lowercase()).chain((!host.is_empty()).then_some(host)) {
                let tags = by_domain.entry(domain).or_default();
                for tag in &preset.tags {
                    if !tags.contains(&tag.as_str()) {
                        tags.push(tag.as_str());
                    }
                }
            }
        }
        Sites { by_domain, presets }
    }

    /// The categories of the sites a domain's mail comes from: the domain's own, or a parent's.
    fn tags(&self, domain: &str) -> &[&'a str] {
        let mut at = domain;
        loop {
            if let Some(tags) = self.by_domain.get(at) {
                return tags;
            }
            match at.split_once('.') {
                Some((_, parent)) if parent.contains('.') => at = parent,
                _ => return &[],
            }
        }
    }
}

/// A group's name from its senders' sites: the category that most of its
/// messages' proven domains share (the plainest, "admin", only when no
/// other one does), in the person's language; none when no category holds most.
fn site_word(read: &Read, members: &[usize], sites: &Sites, options: &Options) -> Option<String> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    for &d in members {
        let Some(domain) = read.mail[d].senders.iter().find_map(|w| w.strip_prefix("from-dom:")) else { continue };
        for tag in sites.tags(domain) {
            *count.entry(tag).or_default() += 1;
        }
    }
    let most = members.len() / 2 + 1;
    let mut held: Vec<(&str, usize)> = count.into_iter().filter(|&(_, n)| n >= most).collect();
    held.sort_by(|a, b| (a.0 == "admin").cmp(&(b.0 == "admin")).then(b.1.cmp(&a.1)).then(a.0.cmp(b.0)));
    let (tag, _) = held.first()?;
    let name = sites.presets.tags.get(*tag).map(|t| sioul_core::presets::words(&t.name, &options.language))?;
    (!name.is_empty()).then_some(name)
}

// 1. The senders' graph.

/// The graph's communities by Louvain's method: local moves by modularity
/// gain, then each community one node, again, until nothing moves.
pub(crate) fn louvain(n: usize, edges: &[(usize, usize, f64)]) -> Vec<usize> {
    let mut community: Vec<usize> = (0..n).collect();
    let mut adjacency: Vec<HashMap<usize, f64>> = vec![HashMap::new(); n];
    let mut inner = vec![0f64; n];
    for &(a, b, w) in edges {
        if a == b {
            inner[a] += w;
        } else {
            *adjacency[a].entry(b).or_default() += w;
            *adjacency[b].entry(a).or_default() += w;
        }
    }
    loop {
        let size = adjacency.len();
        let degree: Vec<f64> = (0..size).map(|i| adjacency[i].values().sum::<f64>() + 2.0 * inner[i]).collect();
        let m2: f64 = degree.iter().sum();
        if m2 == 0.0 {
            break;
        }
        let mut comm: Vec<usize> = (0..size).collect();
        let mut total = degree.clone();
        let mut moved_any = false;
        for _ in 0..50 {
            let mut moved = false;
            for i in 0..size {
                let old = comm[i];
                let mut towards: BTreeMap<usize, f64> = BTreeMap::new();
                for (&j, &w) in &adjacency[i] {
                    *towards.entry(comm[j]).or_default() += w;
                }
                total[old] -= degree[i];
                let gain = |c: usize, total: &[f64]| towards.get(&c).copied().unwrap_or(0.0) - total[c] * degree[i] / m2;
                let mut best = (old, gain(old, &total));
                for &c in towards.keys() {
                    let g = gain(c, &total);
                    if g > best.1 + 1e-12 {
                        best = (c, g);
                    }
                }
                total[best.0] += degree[i];
                if best.0 != old {
                    comm[i] = best.0;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
            moved_any = true;
        }
        if !moved_any {
            break;
        }
        // Each community one node.
        let mut renumber: HashMap<usize, usize> = HashMap::new();
        for c in &comm {
            let next = renumber.len();
            renumber.entry(*c).or_insert(next);
        }
        let k = renumber.len();
        for c in community.iter_mut() {
            *c = renumber[&comm[*c]];
        }
        let mut next_adjacency: Vec<HashMap<usize, f64>> = vec![HashMap::new(); k];
        let mut next_inner = vec![0f64; k];
        for i in 0..size {
            let ci = renumber[&comm[i]];
            next_inner[ci] += inner[i];
            for (&j, &w) in &adjacency[i] {
                let cj = renumber[&comm[j]];
                if ci == cj {
                    // Each inner edge is seen from both ends.
                    next_inner[ci] += w / 2.0;
                } else {
                    *next_adjacency[ci].entry(cj).or_default() += w;
                }
            }
        }
        adjacency = next_adjacency;
        inner = next_inner;
        if k == size {
            break;
        }
    }
    community
}

/// Whether a message's correspondent is proven: a sender Sioul's checks
/// verified (its sender words), or a person you wrote to.
fn proven(m: &Mail) -> bool {
    m.sender.is_some() && (m.yours || !m.senders.is_empty())
}

fn senders(read: &Read, docs: &Docs, labels: &[Option<usize>], ids: &[String], options: &Options) -> Way {
    let mail = &read.mail;
    let mut node: HashMap<&str, usize> = HashMap::new();
    for m in mail.iter().filter(|m| proven(m)) {
        let next = node.len();
        node.entry(m.sender.as_deref().unwrap_or_default()).or_insert(next);
    }
    if node.is_empty() {
        return Way { name: "senders".into(), skipped: Some("no-senders".into()), ..Way::default() };
    }
    let by_id: HashMap<&str, usize> = mail.iter().enumerate().filter_map(|(d, m)| Some((m.id.as_deref()?, d))).collect();
    let mut weights: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    let mut tie = |a: usize, b: usize| {
        if a != b {
            *weights.entry((a.min(b), a.max(b))).or_default() += 1.0;
        }
    };
    for m in mail.iter().filter(|m| proven(m)) {
        let s = node[m.sender.as_deref().unwrap_or_default()];
        // A conversation: the messages it answers or cites, theirs proven too.
        for r in &m.refs {
            if let Some(&d) = by_id.get(r.as_str())
                && proven(&mail[d])
            {
                tie(s, node[mail[d].sender.as_deref().unwrap_or_default()]);
            }
        }
        // One message's recipients: the sender with each, or, for yours, each with the others.
        let met: Vec<usize> = m.to.iter().filter_map(|a| node.get(a.as_str()).copied()).collect();
        if m.yours {
            for (i, &a) in met.iter().enumerate() {
                for &b in &met[i + 1..] {
                    tie(a, b);
                }
            }
        } else {
            for &b in &met {
                tie(s, b);
            }
        }
    }
    let edges: Vec<(usize, usize, f64)> = weights.into_iter().map(|((a, b), w)| (a, b, w)).collect();
    let community = louvain(node.len(), &edges);
    let groups = community.iter().copied().max().map_or(0, |c| c + 1);
    let assigned: Vec<Option<usize>> = mail.iter().map(|m| proven(m).then(|| community[node[m.sender.as_deref().unwrap_or_default()]])).collect();
    let typical: Vec<f64> = mail.iter().map(|m| m.date as f64).collect();
    let words = ctfidf(docs, &assigned, groups, options.top_words);
    summed("senders", read, docs, &assigned, &typical, &words, labels, ids, options)
}

// 2. Model2Vec and k-means.

fn dot(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| f64::from(*x) * f64::from(*y)).sum()
}

/// k-means on unit vectors (k-means++ seeds, Lloyd's steps): each point's
/// cluster, the centroids (unit length).
pub(crate) fn kmeans(points: &[Vec<f32>], k: usize, seed: u64) -> (Vec<usize>, Vec<Vec<f32>>) {
    let n = points.len();
    let dim = points.first().map_or(0, Vec::len);
    let mut rng = Rng::new(seed);
    let mut centroids: Vec<Vec<f32>> = vec![points[rng.below(n)].clone()];
    let mut nearest = vec![f64::INFINITY; n];
    while centroids.len() < k.min(n) {
        let last = centroids.last().cloned().unwrap_or_default();
        for (i, p) in points.iter().enumerate() {
            nearest[i] = nearest[i].min((1.0 - dot(p, &last)).max(0.0));
        }
        let total: f64 = nearest.iter().sum();
        let pick = if total <= 0.0 {
            rng.below(n)
        } else {
            let mut target = rng.unit() * total;
            nearest.iter().position(|&d| {
                target -= d;
                target <= 0.0
            }).unwrap_or(n - 1)
        };
        centroids.push(points[pick].clone());
    }
    let mut assigned = vec![usize::MAX; n];
    for _ in 0..50 {
        let mut changed = false;
        for (i, p) in points.iter().enumerate() {
            let best = (0..centroids.len()).max_by(|&a, &b| dot(p, &centroids[a]).partial_cmp(&dot(p, &centroids[b])).unwrap_or(std::cmp::Ordering::Equal).then(b.cmp(&a))).unwrap_or(0);
            if assigned[i] != best {
                assigned[i] = best;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        let mut sums = vec![vec![0f64; dim]; centroids.len()];
        for (i, p) in points.iter().enumerate() {
            for (s, x) in sums[assigned[i]].iter_mut().zip(p) {
                *s += f64::from(*x);
            }
        }
        for (c, s) in sums.into_iter().enumerate() {
            let norm = s.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 0.0 {
                centroids[c] = s.into_iter().map(|x| (x / norm) as f32).collect();
            }
        }
    }
    (assigned, centroids)
}

/// The mean silhouette of a clustering, by cosine distance, on the first `sample` points.
pub(crate) fn silhouette(points: &[Vec<f32>], assigned: &[usize], k: usize, sample: usize) -> f64 {
    let n = points.len().min(sample);
    let mut total = 0.0;
    let mut counted = 0usize;
    for i in 0..n {
        let mut sums = vec![0f64; k];
        let mut counts = vec![0usize; k];
        for j in 0..n {
            if i != j {
                sums[assigned[j]] += (1.0 - dot(&points[i], &points[j])).max(0.0);
                counts[assigned[j]] += 1;
            }
        }
        let own = assigned[i];
        if counts[own] == 0 {
            continue;
        }
        let a = sums[own] / counts[own] as f64;
        let b = (0..k).filter(|&c| c != own && counts[c] > 0).map(|c| sums[c] / counts[c] as f64).fold(f64::INFINITY, f64::min);
        if b.is_finite() && a.max(b) > 0.0 {
            total += (b - a) / a.max(b);
            counted += 1;
        }
    }
    if counted == 0 { 0.0 } else { total / counted as f64 }
}

#[allow(clippy::too_many_arguments)]
fn kmeans_way(read: &Read, docs: &Docs, model: &StaticModel, labels: &[Option<usize>], ids: &[String], options: &Options, cancel: &Cancel) -> Result<(Way, Option<usize>), LearnError> {
    let mut points = Vec::new();
    let mut of_point = Vec::new();
    for (d, m) in read.mail.iter().enumerate() {
        if let Some(v) = model.mean(&m.tokens) {
            let norm = v.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>().sqrt();
            if norm > 0.0 {
                points.push(v.into_iter().map(|x| (f64::from(x) / norm) as f32).collect::<Vec<f32>>());
                of_point.push(d);
            }
        }
    }
    if points.len() < 2 * options.k_min.max(2) {
        return Ok((Way { name: "model2vec".into(), skipped: Some("too-few".into()), ..Way::default() }, None));
    }
    // The silhouette's sample: points spread over the whole mail, the same for every k.
    let mut order: Vec<usize> = (0..points.len()).collect();
    let mut rng = Rng::new(options.seed.wrapping_add(11));
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i + 1));
    }
    let sampled: Vec<Vec<f32>> = order.iter().take(options.sample).map(|&i| points[i].clone()).collect();
    let mut best: Option<(f64, usize, Vec<usize>, Vec<Vec<f32>>)> = None;
    for k in options.k_min.max(2)..=options.k_max.max(options.k_min).min(points.len() / 2) {
        if cancel.cancelled() {
            return Err(LearnError::Cancelled);
        }
        let (assigned, centroids) = kmeans(&points, k, options.seed);
        let sample_assigned: Vec<usize> = order.iter().take(options.sample).map(|&i| assigned[i]).collect();
        let score = silhouette(&sampled, &sample_assigned, k, options.sample);
        if best.as_ref().is_none_or(|b| score > b.0) {
            best = Some((score, k, assigned, centroids));
        }
    }
    let Some((score, k, assigned, centroids)) = best else { return Ok((Way { name: "model2vec".into(), skipped: Some("too-few".into()), ..Way::default() }, None)) };
    let mut by_message: Vec<Option<usize>> = vec![None; read.mail.len()];
    let mut typical = vec![f64::NEG_INFINITY; read.mail.len()];
    for (p, &d) in of_point.iter().enumerate() {
        by_message[d] = Some(assigned[p]);
        typical[d] = dot(&points[p], &centroids[assigned[p]]);
    }
    let words = ctfidf(docs, &by_message, k, options.top_words);
    let mut way = summed("model2vec", read, docs, &by_message, &typical, &words, labels, ids, options);
    way.silhouette = Some(score);
    Ok((way, Some(k)))
}

// 3. LDA by collapsed Gibbs sampling.

fn lda(read: &Read, docs: &Docs, k: usize, labels: &[Option<usize>], ids: &[String], options: &Options, cancel: &Cancel) -> Result<Way, LearnError> {
    let (alpha, beta) = (0.1f64, 0.01f64);
    let v = docs.vocabulary.len();
    let mut rng = Rng::new(options.seed.wrapping_add(23));
    let words: Vec<&[u32]> = docs.words.iter().map(|w| &w[..w.len().min(options.doc_words)]).collect();
    let mut z: Vec<Vec<u16>> = words.iter().map(|ws| ws.iter().map(|_| rng.below(k) as u16).collect()).collect();
    let mut n_dk = vec![vec![0u32; k]; words.len()];
    let mut n_kw = vec![vec![0u32; v]; k];
    let mut n_k = vec![0u32; k];
    for (d, ws) in words.iter().enumerate() {
        for (i, &w) in ws.iter().enumerate() {
            let t = z[d][i] as usize;
            n_dk[d][t] += 1;
            n_kw[t][w as usize] += 1;
            n_k[t] += 1;
        }
    }
    let vb = v as f64 * beta;
    let mut p = vec![0f64; k];
    for _ in 0..options.lda_iterations {
        if cancel.cancelled() {
            return Err(LearnError::Cancelled);
        }
        for (d, ws) in words.iter().enumerate() {
            for (i, &w) in ws.iter().enumerate() {
                let (w, old) = (w as usize, z[d][i] as usize);
                n_dk[d][old] -= 1;
                n_kw[old][w] -= 1;
                n_k[old] -= 1;
                let mut total = 0.0;
                for t in 0..k {
                    total += (f64::from(n_dk[d][t]) + alpha) * (f64::from(n_kw[t][w]) + beta) / (f64::from(n_k[t]) + vb);
                    p[t] = total;
                }
                let target = rng.unit() * total;
                let new = p.iter().position(|&c| c >= target).unwrap_or(k - 1);
                z[d][i] = new as u16;
                n_dk[d][new] += 1;
                n_kw[new][w] += 1;
                n_k[new] += 1;
            }
        }
    }
    let mut assigned = vec![None; words.len()];
    let mut typical = vec![0f64; words.len()];
    for (d, ws) in words.iter().enumerate() {
        if ws.is_empty() {
            continue;
        }
        let t = (0..k).max_by_key(|&t| (n_dk[d][t], std::cmp::Reverse(t))).unwrap_or(0);
        assigned[d] = Some(t);
        typical[d] = (f64::from(n_dk[d][t]) + alpha) / (ws.len() as f64 + k as f64 * alpha);
    }
    let top: Vec<Vec<u32>> = (0..k)
        .map(|t| {
            let mut ws: Vec<u32> = (0..v as u32).collect();
            ws.sort_by(|&a, &b| n_kw[t][b as usize].cmp(&n_kw[t][a as usize]).then(a.cmp(&b)));
            ws.into_iter().take(options.top_words).filter(|&w| n_kw[t][w as usize] > 0).collect()
        })
        .collect();
    Ok(summed("lda", read, docs, &assigned, &typical, &top, labels, ids, options))
}

// 4. NMF on TF-IDF.

fn nmf(read: &Read, docs: &Docs, k: usize, labels: &[Option<usize>], ids: &[String], options: &Options, cancel: &Cancel) -> Result<Way, LearnError> {
    let (n, v) = (docs.words.len(), docs.vocabulary.len());
    let total = n as f64;
    // Each message's TF-IDF, unit length, sparse.
    let x: Vec<Vec<(usize, f64)>> = docs
        .words
        .iter()
        .map(|ws| {
            let mut tf: BTreeMap<usize, f64> = BTreeMap::new();
            for &w in ws {
                *tf.entry(w as usize).or_default() += 1.0;
            }
            let row: Vec<(usize, f64)> = tf.into_iter().map(|(w, c)| (w, c * (total / docs.postings[w].len().max(1) as f64).ln())).collect();
            let norm = row.iter().map(|(_, x)| x * x).sum::<f64>().sqrt();
            if norm > 0.0 { row.into_iter().map(|(w, x)| (w, x / norm)).collect() } else { Vec::new() }
        })
        .collect();
    let mut rng = Rng::new(options.seed.wrapping_add(31));
    let mut w = vec![vec![0f64; k]; n];
    for row in w.iter_mut() {
        for x in row.iter_mut() {
            *x = 0.1 + rng.unit();
        }
    }
    let mut h = vec![vec![0f64; v]; k];
    for row in h.iter_mut() {
        for x in row.iter_mut() {
            *x = 0.1 + rng.unit();
        }
    }
    const EPS: f64 = 1e-9;
    for _ in 0..options.nmf_iterations {
        if cancel.cancelled() {
            return Err(LearnError::Cancelled);
        }
        // H ← H ∘ (WᵀX) / (WᵀW H).
        let mut wtx = vec![vec![0f64; v]; k];
        for (d, row) in x.iter().enumerate() {
            for &(j, value) in row {
                for t in 0..k {
                    wtx[t][j] += w[d][t] * value;
                }
            }
        }
        let mut wtw = vec![vec![0f64; k]; k];
        for row in &w {
            for a in 0..k {
                for b in 0..k {
                    wtw[a][b] += row[a] * row[b];
                }
            }
        }
        for j in 0..v {
            for t in 0..k {
                let denominator: f64 = (0..k).map(|s| wtw[t][s] * h[s][j]).sum::<f64>() + EPS;
                h[t][j] *= wtx[t][j] / denominator;
            }
        }
        // W ← W ∘ (X Hᵀ) / (W H Hᵀ).
        let mut hht = vec![vec![0f64; k]; k];
        for j in 0..v {
            for a in 0..k {
                for b in 0..k {
                    hht[a][b] += h[a][j] * h[b][j];
                }
            }
        }
        for (d, row) in x.iter().enumerate() {
            let mut xht = vec![0f64; k];
            for &(j, value) in row {
                for t in 0..k {
                    xht[t] += value * h[t][j];
                }
            }
            let current = w[d].clone();
            for t in 0..k {
                let denominator: f64 = (0..k).map(|s| current[s] * hht[s][t]).sum::<f64>() + EPS;
                w[d][t] *= xht[t] / denominator;
            }
        }
    }
    let mut assigned = vec![None; n];
    let mut typical = vec![0f64; n];
    for d in 0..n {
        if x[d].is_empty() {
            continue;
        }
        let (t, value) = (0..k).map(|t| (t, w[d][t])).fold((0, f64::NEG_INFINITY), |a, b| if b.1 > a.1 { b } else { a });
        if value > 0.0 {
            assigned[d] = Some(t);
            typical[d] = value;
        }
    }
    let top: Vec<Vec<u32>> = h
        .iter()
        .map(|row| {
            let mut ws: Vec<u32> = (0..v as u32).collect();
            ws.sort_by(|&a, &b| row[b as usize].partial_cmp(&row[a as usize]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
            ws.into_iter().take(options.top_words).collect()
        })
        .collect();
    Ok(summed("nmf", read, docs, &assigned, &typical, &top, labels, ids, options))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two cliques joined by one edge: two communities.
    #[test]
    fn louvain_finds_two_cliques() {
        let mut edges = Vec::new();
        for clique in [[0, 1, 2, 3], [4, 5, 6, 7]] {
            for (i, &a) in clique.iter().enumerate() {
                for &b in &clique[i + 1..] {
                    edges.push((a, b, 1.0));
                }
            }
        }
        edges.push((3, 4, 1.0));
        let c = louvain(9, &edges);
        assert!(c[0] == c[1] && c[1] == c[2] && c[2] == c[3], "{c:?}");
        assert!(c[4] == c[5] && c[5] == c[6] && c[6] == c[7], "{c:?}");
        assert_ne!(c[0], c[4]);
        assert!(c[8] != c[0] && c[8] != c[4], "alone");
    }

    /// Three directions: k-means finds them, and the silhouette prefers three to two or five.
    #[test]
    fn kmeans_and_the_silhouette() {
        let mut rng = Rng::new(5);
        let mut points = Vec::new();
        for i in 0..90 {
            let mut p = vec![0.05f32; 3];
            p[i % 3] = 1.0;
            for x in p.iter_mut() {
                *x += (rng.unit() as f32 - 0.5) * 0.1;
            }
            let norm = p.iter().map(|x| x * x).sum::<f32>().sqrt();
            points.push(p.into_iter().map(|x| x / norm).collect::<Vec<f32>>());
        }
        let (assigned, _) = kmeans(&points, 3, 1);
        for i in 0..90 {
            assert_eq!(assigned[i], assigned[i % 3], "point {i}");
        }
        let score = |k: usize| {
            let (a, _) = kmeans(&points, k, 1);
            silhouette(&points, &a, k, 1000)
        };
        assert!(score(3) > score(2) && score(3) > score(5));
    }

    /// The stand-in mail grouped four ways: the offices' senders apart, each
    /// group in its seed; LDA and NMF find the topics' words; no k-means without a model.
    #[test]
    fn stand_in_mail_in_groups() {
        let root = std::env::temp_dir().join(format!("sioul-topics-groups-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (dirs, sources) = crate::topics::tests::home(&root, "groups");
        let topic_options = crate::topics::tests::options();
        let read = crate::topics::read(&dirs, &crate::topics::tests::trusted(), &sources, &topic_options, true, None, &mut |_| {}, &Cancel::new()).unwrap();
        let (seeded, labels) = crate::topics::seeded(&read, &topic_options);
        let ids: Vec<String> = seeded.proposal.topics.iter().map(|t| t.id.clone()).collect();
        let options = Options { k: Some(4), lda_iterations: 60, nmf_iterations: 60, ..Options::default() };
        let found = groups(&read, &labels, &ids, None, &options, &mut |_| {}, &Cancel::new()).unwrap();
        assert_eq!(found.ways.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), vec!["senders", "model2vec", "lda", "nmf"]);
        assert_eq!(found.ways[1].skipped.as_deref(), Some("no-model"));
        let senders = &found.ways[0];
        assert!(senders.groups.len() >= 3 && senders.agreement.unwrap() > 0.9, "{senders:#?}");
        for way in &found.ways[2..] {
            assert!(way.groups.len() >= 3 && way.agreement.unwrap() > 0.6 && way.coherence.is_some(), "{way:#?}");
            assert!(way.groups.iter().all(|g| g.words.len() == options.top_words && !g.subjects.is_empty()));
        }
        // The window's: NMF alone, the messages you tagged left out; each group's ids for "Name it".
        let choir: HashSet<String> = labels.iter().zip(&read.mail).filter(|(l, _)| l.is_some_and(|t| ids[t] == "chorale")).filter_map(|(_, m)| m.id.clone()).collect();
        let window = Options { nmf_only: true, leave_out: choir.clone(), ..options.clone() };
        let found = groups(&read, &labels, &ids, None, &window, &mut |_| {}, &Cancel::new()).unwrap();
        assert_eq!(found.ways.len(), 1);
        let nmf = &found.ways[0];
        assert!(nmf.groups.iter().all(|g| g.ids.len() as u64 == g.size && g.ids.iter().all(|id| !choir.contains(id))), "{nmf:#?}");
        assert!(nmf.groups.iter().all(|g| g.seed.as_ref().is_none_or(|(id, _)| id != "chorale")));
        // Named from stand-in sites: the tax office's and the doctor's domains are sites of a category.
        let presets: sioul_core::presets::Presets = serde_json::from_str(r#"{"checked": "2026-11-05", "tags": {"tax": {"name": {"en": "Taxes", "fr": "Impôts"}}, "health": {"name": {"en": "Health", "fr": "Santé"}}, "admin": {"name": {"en": "Administration", "fr": "Démarches"}}},
            "international": [{"name": "Tax office", "url": "https://www.impots.example/login", "type": "mailbox", "tags": ["admin", "tax"], "announced_by": []},
                              {"name": "Doctor", "url": "https://portal.example/", "type": "mailbox", "tags": ["health"], "announced_by": ["sante.example"]}]}"#).unwrap();
        let named = Options { nmf_only: true, presets: Some(presets), language: "fr".into(), ..options.clone() };
        let found = groups(&read, &labels, &ids, None, &named, &mut |_| {}, &Cancel::new()).unwrap();
        let proposed: Vec<(&str, &str)> = found.ways[0].groups.iter().map(|g| (g.proposed.as_str(), g.proposed_by)).collect();
        assert!(proposed.contains(&("Impôts", "sites")) && proposed.contains(&("Santé", "sites")), "{proposed:?}");
        assert!(found.ways[0].groups.iter().all(|g| !g.names.is_empty() && !g.proposed.is_empty()), "{:#?}", found.ways[0]);
        assert_eq!(masked("Votre code 482913 sur https://x.example/a"), "Votre code … sur …");
        let _ = std::fs::remove_dir_all(root);
    }

    /// Words always together score near 1, never together -1.
    #[test]
    fn npmi_of_words() {
        let docs = Docs { words: vec![vec![0, 1], vec![0, 1], vec![2], vec![2]], vocabulary: vec!["a".into(), "b".into(), "c".into()], postings: vec![vec![0, 1], vec![0, 1], vec![2, 3]] };
        assert!((npmi(&docs, &[0, 1]).unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(npmi(&docs, &[0, 2]), Some(-1.0));
        assert_eq!(npmi(&docs, &[0]), None);
    }
}
