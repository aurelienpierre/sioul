// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The chain of ties from one thing (docs/research/link-graph.md, §2.7):
//! what it is tied to, what those are tied to, up to four steps, both ways,
//! whoever wrote the tie, the strongest chains first.
//!
//! **The strongest chain.** A thing's score is the product of its ties'
//! strengths along its best chain from the entry point (`Weights::strength`),
//! times 0.75 for each step after the first, found best-first over the tie
//! index (`ties::TieIndex`). Each thing shown is explained by that one chain,
//! said in words; the same files give the same picture.
//!
//! **Collections stop the walk**: a project, a contact, a person, a budget
//! and a site are shown with how many things they hold, and not walked
//! through, unless one is the entry point. Leaving any other thing with `d`
//! ties to follow, each is weighed `min(1, √(4/d))`, which damps the hubs
//! that are not collections. The web, files and programs are ends; an event
//! another shares for its availability alone shows as "Busy" and mail the
//! Porch set aside is shown, never walked through (`ties::Rule`).
//!
//! **Bounds**: four steps, a score of 0.1 at least, five things under each
//! thing, 24 in all on a computer and 12 on a phone. Equal scores come
//! nearest to the entry point's date first, then by kind, title and address.
//! Inside one collection (the entry point, or one whose "+" is pressed), one
//! of each kind comes first (a letter, a task, a note, a message, a payment,
//! then the others), then the rest by date, and no more than two messages
//! from one sender before the others.
//!
//! **"+N"** on each thing counts its ties not shown, apart from the one it
//! hangs from and things shown elsewhere. "+" reveals the next eight of them
//! by the same score; it computes nothing again, so nothing already shown
//! moves or leaves, and "−" gives back the picture before.
//!
//! **The tree**: each thing in the column of its step and on its row, the
//! thing it hangs from beside its first; ties between things already shown
//! are cross-ties. One `ChainView` gives the drawing and the outline (the
//! same rows in the same order).

use crate::i18n::Translator;
use crate::links::{Kind, Related, World};
use crate::ties::{How, Rule, Seen, TieIndex};
use jiff::Timestamp;
use serde::Serialize;
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::rc::Rc;

/// The walk's parameters, each with its reason (docs/research/link-graph.md,
/// §2.7). Starting values, to be tuned on real data.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Weights {
    /// Tied by hand; made from; an appointment; a reply; a step; a wait; the
    /// note describing it; a movement's sure source; a person's own card.
    pub sure: f64,
    /// A movement's probable source; a contract's preset or paper; whom a
    /// paper or a letter is for; a stand-in; who takes a child's event; instalments.
    pub close: f64,
    /// In the project; a contact or a guest; a sender; mentioned by a note;
    /// a budget; a site's person; a person's agenda; a child's other parent;
    /// the same conversation.
    pub member: f64,
    /// In a project by its routes alone; a client or a contract's party by name.
    pub loose: f64,
    /// Each step after the first: interest falls with distance.
    pub decay: f64,
    /// Under this score, a thing is not shown.
    pub floor: f64,
    /// Steps from the entry point, at most.
    pub depth: usize,
    /// Things shown under each thing, at most (van Ham and Perer's "n < 5").
    pub under: usize,
    /// Things shown in all, at most: under the 50 nodes where finding paths became hard.
    pub total: usize,
    /// Things each "+" adds.
    pub more: usize,
    /// The fan-out's reference: leaving a thing with `d` ties, each is weighed `min(1, √(fan / d))`.
    pub fan: f64,
    /// Inside one collection, messages from one sender at most, before the
    /// others come (a weekly notification crowded out the office's letters
    /// in the first trial on real data).
    pub per_sender: usize,
}

impl Weights {
    /// On a computer: 24 things at most.
    pub fn computer() -> Weights {
        Weights { sure: 1.0, close: 0.8, member: 0.6, loose: 0.4, decay: 0.75, floor: 0.1, depth: 4, under: 5, total: 24, more: 8, fan: 4.0, per_sender: 2 }
    }

    /// On a phone: 12 things at most, about a quarter of a laptop window's area.
    pub fn phone() -> Weights {
        Weights { total: 12, ..Weights::computer() }
    }

    /// How strongly a kind of tie ties two things.
    pub fn strength(&self, how: How) -> f64 {
        match how {
            How::Appointment | How::Waits | How::Step | How::Source | How::Answers | How::Note | How::Told | How::Link | How::Card => self.sure,
            How::ToldProbably | How::Contract | How::ContractPaper | How::For | How::StandIn | How::TakenBy | How::Together => self.close,
            How::Project | How::Contact | How::Sender | How::Mention | How::Budget | How::ProjectBudget | How::SiteFor | How::Agenda | How::Parent | How::Conversation => self.member,
            How::Routes | How::Client | How::Party => self.loose,
        }
    }
}

/// A kind's family, which gives its shape and its colour: "time" (tasks,
/// events), "messages" (mail, drafts, letters), "people" (contacts, persons),
/// "money" (movements, budgets, invoices, contracts), "projects", "records" (the rest).
pub fn family(kind: Kind) -> &'static str {
    match kind {
        Kind::Task | Kind::Event => "time",
        Kind::Mail | Kind::Draft | Kind::Letter => "messages",
        Kind::Contact | Kind::Person => "people",
        Kind::Movement | Kind::Budget | Kind::Invoice | Kind::Contract => "money",
        Kind::Project => "projects",
        Kind::Note | Kind::Paper | Kind::Site | Kind::Web | Kind::File | Kind::Other => "records",
    }
}

/// A kind's id, as its words are named (`chain-kind-<id>`, `chain-count-<id>`).
pub fn kind_id(kind: Kind) -> &'static str {
    match kind {
        Kind::Task => "task",
        Kind::Event => "event",
        Kind::Mail => "mail",
        Kind::Draft => "draft",
        Kind::Note => "note",
        Kind::Contact => "contact",
        Kind::Person => "person",
        Kind::Budget => "budget",
        Kind::Project => "project",
        Kind::Paper => "paper",
        Kind::Contract => "contract",
        Kind::Letter => "letter",
        Kind::Invoice => "invoice",
        Kind::Movement => "movement",
        Kind::Site => "site",
        Kind::Web => "web",
        Kind::File => "file",
        Kind::Other => "other",
    }
}

/// The kinds a collection shows one of first, in this order; then one of
/// each other kind, in `Kind`'s order (the first trial on real data: from a
/// project, its five newest messages hid its letters, tasks and notes).
pub const FIRST_KINDS: [Kind; 5] = [Kind::Letter, Kind::Task, Kind::Note, Kind::Mail, Kind::Movement];

/// Every kind, for the words' check.
pub const KINDS: [Kind; 18] = [Kind::Task, Kind::Event, Kind::Mail, Kind::Draft, Kind::Note, Kind::Contact, Kind::Person, Kind::Budget, Kind::Project, Kind::Paper, Kind::Contract, Kind::Letter, Kind::Invoice, Kind::Movement, Kind::Site, Kind::Web, Kind::File, Kind::Other];

/// A collection: shown with what it holds, not walked through unless it is
/// the entry point. A budget whole (`sioul:budget/<budget>`) is one; its line is not.
pub fn is_collection(uri: &str) -> bool {
    match crate::links::kind_of(uri) {
        Kind::Project | Kind::Contact | Kind::Person | Kind::Site => true,
        Kind::Budget => uri.strip_prefix("sioul:budget/").is_some_and(|rest| !rest.contains('/')),
        _ => false,
    }
}

/// One thing of the chain.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChainNode {
    pub uri: String,
    pub kind: Kind,
    /// Its family (`family`): its shape and its colour.
    pub family: &'static str,
    /// Its kind in words: "Task", "Message", "Payment".
    pub kind_words: String,
    pub title: String,
    /// Names only: who, which list, an amount, the Porch's words for mail it set aside.
    pub detail: String,
    /// Its date in words, as Mail's rows show them; "" for none.
    pub date: String,
    pub when: Option<i64>,
    /// What the window opens (`Related::key`).
    pub key: String,
    pub found: bool,
    pub program: bool,
    /// Its step from the entry point: its column.
    pub column: usize,
    /// Its row in the drawing; the outline lists the things in their order here.
    pub row: usize,
    /// The thing it hangs from, by its place in `ChainView::things`.
    pub parent: Option<usize>,
    /// How it is tied to the thing it hangs from, seen from that thing (`how-<id>`).
    pub how: &'static str,
    pub how_words: String,
    /// Its strongest chain's score.
    pub score: f64,
    /// "+N": its ties not shown.
    pub more: usize,
    /// What they are: "12 more: 9 messages, 3 tasks"; "" for none.
    pub more_words: String,
    /// How many times its "+" was pressed.
    pub opened: usize,
    /// A collection (`is_collection`), and how many things it holds.
    pub collection: bool,
    pub count: usize,
    /// An event shared for its availability alone, shown "Busy".
    pub busy: bool,
    /// Mail the Porch set aside, reached by a tie made by hand: never walked through.
    pub set_aside: bool,
    pub health: bool,
    /// The time sessions noted on a task, folded into it.
    pub sessions: u32,
    /// Its chain from the entry point, in words: "Told by: Message, Rent · Made into: Task, Pay the rent".
    pub chain: String,
    /// Its accessible name: "Task: Pay the rent; Made into: Rent; 3 more: 2 messages, 1 task".
    pub name: String,
}

/// A tie between two things shown that is not the tree's: drawn as a thin curve.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrossTie {
    pub from: usize,
    pub to: usize,
    /// Seen from `from`.
    pub how: &'static str,
    pub words: String,
}

/// The chain from one thing: its drawing and its outline.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChainView {
    pub entry: String,
    /// In the outline's order (each thing, then what hangs from it); the entry point first.
    pub things: Vec<ChainNode>,
    pub cross: Vec<CrossTie>,
    /// A health thing is shown: the window keeps itself from the overview of apps.
    pub health: bool,
    pub columns: usize,
    pub rows: usize,
}

/// How a thing was reached: its score, the thing it hangs from, its step, the tie's words.
#[derive(Debug, Clone, Copy)]
struct Reached {
    score: f64,
    parent: Option<u32>,
    depth: usize,
    /// Seen from the thing it hangs from ("made into"), and from itself ("made from").
    how: &'static str,
    back: &'static str,
}

/// A seen tie's words from its other end.
fn back_words(seen: &Seen) -> &'static str {
    if seen.forward { seen.how.back() } else { seen.how.forward() }
}

/// A sentence with its arguments, a whole number as a number (for "one" and "other").
fn say(tr: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = crate::i18n::args();
    for (key, value) in pairs {
        match value.parse::<i64>() {
            Ok(n) if n.to_string() == *value => args.set(*key, n),
            _ => args.set(*key, value.clone()),
        }
    }
    tr.text(id, Some(&args))
}

/// A score as compared: equal to the ninth decimal, so that the same
/// product reached in two orders is one score.
fn quantized(score: f64) -> u64 {
    (score * 1e9).round().max(0.0) as u64
}

/// The walk's memory of what it read.
struct Walk<'a> {
    world: &'a World<'a>,
    index: &'a TieIndex,
    rule: &'a dyn Rule,
    weights: &'a Weights,
    start: u32,
    follow: RefCell<HashMap<u32, Rc<Vec<Seen>>>>,
    described: RefCell<HashMap<u32, Rc<Related>>>,
    refused: RefCell<HashMap<u32, Option<crate::porch::Reason>>>,
}

impl Walk<'_> {
    fn address(&self, node: u32) -> &str {
        self.index.address(node)
    }

    /// The ties a thing may be followed by, one per thing at the other end (the strongest).
    fn follow(&self, node: u32) -> Rc<Vec<Seen>> {
        if let Some(found) = self.follow.borrow().get(&node) {
            return found.clone();
        }
        let mut out: Vec<Seen> = Vec::new();
        let mut at: HashMap<u32, usize> = HashMap::new();
        for seen in self.index.ties_of(node).filter(|s| self.index.follows(self.world, self.rule, node, s)) {
            match at.get(&seen.other) {
                Some(&i) => {
                    let (was, now) = (self.weights.strength(out[i].how), self.weights.strength(seen.how));
                    if now > was || (now == was && seen.how < out[i].how) {
                        out[i] = seen;
                    }
                }
                None => {
                    at.insert(seen.other, out.len());
                    out.push(seen);
                }
            }
        }
        let out = Rc::new(out);
        self.follow.borrow_mut().insert(node, out.clone());
        out
    }

    fn describe(&self, node: u32) -> Rc<Related> {
        if let Some(found) = self.described.borrow().get(&node) {
            return found.clone();
        }
        let found = Rc::new(self.world.describe(self.address(node)));
        self.described.borrow_mut().insert(node, found.clone());
        found
    }

    /// The Porch's reason when it set this message aside.
    fn refused(&self, node: u32) -> Option<crate::porch::Reason> {
        if let Some(found) = self.refused.borrow().get(&node) {
            return found.clone();
        }
        let found = crate::ties::set_aside(self.world, self.rule, self.address(node));
        self.refused.borrow_mut().insert(node, found.clone());
        found
    }

    /// Whether the walk goes on through a thing: never a collection (but the
    /// entry point), an end (the web, a file, a program), a busy event, mail set aside.
    fn walkable(&self, node: u32) -> bool {
        let address = self.address(node);
        let kind = crate::links::kind_of(address);
        if matches!(kind, Kind::Web | Kind::File | Kind::Other) || self.rule.busy(address) || self.refused(node).is_some() {
            return false;
        }
        node == self.start || !is_collection(address)
    }

    /// Whether its "+" may reveal what it is tied to: a thing walked through, or a collection.
    fn opens(&self, node: u32) -> bool {
        self.walkable(node) || (is_collection(self.address(node)) && !self.rule.busy(self.address(node)))
    }

    /// The weight of each tie leaving a thing: `min(1, √(fan / d))`, `d` its
    /// ties but the one it hangs from; never on the entry point, whose ties are its own.
    fn fan(&self, node: u32, hangs: bool) -> f64 {
        if node == self.start {
            return 1.0;
        }
        let d = self.follow(node).len().saturating_sub(usize::from(hangs)).max(1);
        (self.weights.fan / d as f64).sqrt().min(1.0)
    }

    /// The score a tie from `from` (reached as `r`) gives the thing at its other end.
    fn through(&self, from: u32, r: &Reached, seen: &Seen) -> f64 {
        let decay = if r.depth >= 1 { self.weights.decay } else { 1.0 };
        r.score * self.weights.strength(seen.how) * self.fan(from, r.parent.is_some()) * decay
    }

    /// How things of equal standing are ordered: the score, then the nearest
    /// to the entry point's date (things without one after), the kind, the
    /// title, the address. The same data, the same order.
    fn key(&self, node: u32, score: f64, reference: i64) -> (Reverse<u64>, u64, Kind, String, String) {
        let found = self.describe(node);
        let distance = found.when.map_or(u64::MAX, |w| w.abs_diff(reference));
        (Reverse(quantized(score)), distance, found.kind, crate::text::fold(&found.title).into_iter().collect(), self.address(node).to_string())
    }

    /// A message's sender, as compared; none for anything else.
    fn sender(&self, node: u32) -> Option<String> {
        let address = self.address(node);
        (crate::links::kind_of(address) == Kind::Mail).then(|| self.world.mail.get(address).map(|m| m.address.trim().to_ascii_lowercase())).flatten().filter(|s| !s.is_empty())
    }

    /// A collection's members in the order it shows them: one of each kind
    /// first (`FIRST_KINDS`, then the others in `Kind`'s order), then the rest
    /// by date, nearest the entry point's first; past `per_sender` messages
    /// from one sender (those already shown under it counted), the others
    /// before them. The same members, the same order.
    fn members(&self, mut members: Vec<u32>, reference: i64, shown: &[u32]) -> Vec<u32> {
        members.sort_by_cached_key(|n| {
            let (_, distance, kind, title, address) = self.key(*n, 0.0, reference);
            (distance, kind, title, address)
        });
        let kind = |n: &u32| crate::links::kind_of(self.address(*n));
        let mut first: Vec<u32> = Vec::new();
        for wanted in FIRST_KINDS.iter().chain(KINDS.iter().filter(|k| !FIRST_KINDS.contains(k))) {
            if let Some(&n) = members.iter().find(|n| kind(n) == *wanted) {
                first.push(n);
            }
        }
        let rest: Vec<u32> = members.iter().copied().filter(|n| !first.contains(n)).collect();
        let mut from: HashMap<String, usize> = HashMap::new();
        for sender in shown.iter().filter_map(|n| self.sender(*n)) {
            *from.entry(sender).or_default() += 1;
        }
        let (mut kept, mut later) = (Vec::new(), Vec::new());
        for n in first.into_iter().chain(rest) {
            match self.sender(n) {
                Some(sender) => {
                    let count = from.entry(sender).or_default();
                    if *count >= self.weights.per_sender {
                        later.push(n);
                    } else {
                        *count += 1;
                        kept.push(n);
                    }
                }
                None => kept.push(n),
            }
        }
        kept.extend(later);
        kept
    }
}

/// The chain from `entry`, with the things whose "+" was pressed (`opened`,
/// once per press), as `rule` lets ties be followed. `world` carries the
/// index (`Loaded::world_with`), which names movements and records; `now`,
/// Unix seconds, stands for the entry point's date when it has none.
#[allow(clippy::too_many_arguments)]
pub fn walk(world: &World, index: &TieIndex, rule: &dyn Rule, entry: &str, opened: &[String], weights: &Weights, tr: &Translator, now: i64) -> ChainView {
    let me = world.canonical(entry);
    let Some(start) = index.node(&me) else {
        // Tied to nothing: the thing alone.
        let found = world.describe(&me);
        let mut view = ChainView { entry: me.clone(), things: Vec::new(), cross: Vec::new(), health: rule.health(&me), columns: 1, rows: 1 };
        let at = Placed { parent: None, column: 0, row: 0, how: "", back: "", score: 1.0, parent_title: None };
        view.things.push(node_of(tr, &found, rule, &at, None, None, (0, String::new()), 0, 0, 0, String::new()));
        return view;
    };
    let walk = Walk { world, index, rule, weights, start, follow: Default::default(), described: Default::default(), refused: Default::default() };
    let reference = walk.describe(start).when.unwrap_or(now);

    // The strongest chains first (van Ham and Perer's greedy subgraph, best-first like
    // Dijkstra on the product of strengths): each candidate hangs from a thing already
    // shown; the best is taken while its thing has room (five) and the total allows;
    // a thing whose strongest chain's thing is full comes by its next chain shown.
    let mut shown: HashMap<u32, Reached> = HashMap::new();
    shown.insert(start, Reached { score: 1.0, parent: None, depth: 0, how: "", back: "" });
    let mut under: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut found: Vec<Reached> = Vec::new();
    type Key = (Reverse<u64>, u64, Kind, String, String);
    let mut heap: BinaryHeap<Reverse<(Key, String, usize, u32)>> = BinaryHeap::new();
    let expand = |node: u32, r: &Reached, shown: &HashMap<u32, Reached>, found: &mut Vec<Reached>, heap: &mut BinaryHeap<Reverse<(Key, String, usize, u32)>>| {
        if r.depth >= weights.depth || !walk.walkable(node) {
            return;
        }
        let ties = walk.follow(node);
        let candidates: Vec<&Seen> = ties.iter().filter(|s| !shown.contains_key(&s.other) && walk.through(node, r, s) >= weights.floor).collect();
        // The entry point a collection: its members in a collection's order, before anything further.
        let ranked: Option<HashMap<u32, usize>> = (node == start && is_collection(index.address(node))).then(|| walk.members(candidates.iter().map(|s| s.other).collect(), reference, &[]).into_iter().enumerate().map(|(rank, n)| (n, rank)).collect());
        let top = candidates.iter().map(|s| walk.through(node, r, s)).fold(0.0, f64::max);
        for seen in candidates {
            let score = walk.through(node, r, seen);
            found.push(Reached { score, parent: Some(node), depth: r.depth + 1, how: seen.words(), back: back_words(seen) });
            let key = match ranked.as_ref().and_then(|ranks| ranks.get(&seen.other)) {
                Some(&rank) => (Reverse(quantized(top)), rank as u64, Kind::Task, String::new(), index.address(seen.other).to_string()),
                None => walk.key(seen.other, score, reference),
            };
            heap.push(Reverse((key, index.address(node).to_string(), found.len() - 1, seen.other)));
        }
    };
    expand(start, &shown[&start], &shown, &mut found, &mut heap);
    while shown.len() < weights.total {
        let Some(Reverse((_, _, at, node))) = heap.pop() else { break };
        let r = found[at];
        let parent = r.parent.unwrap_or(start);
        if shown.contains_key(&node) || under.get(&parent).is_some_and(|c| c.len() >= weights.under) {
            continue;
        }
        under.entry(parent).or_default().push(node);
        shown.insert(node, r);
        expand(node, &r, &shown, &mut found, &mut heap);
    }

    // "+": the next things of each one pressed, by the same score, under it; nothing else moves.
    let mut presses: Vec<(u32, usize)> = Vec::new();
    for address in opened {
        let Some(node) = index.node(&world.canonical(address)) else { continue };
        match presses.iter_mut().find(|(n, _)| *n == node) {
            Some((_, count)) => *count += 1,
            None => presses.push((node, 1)),
        }
    }
    let mut done: HashSet<u32> = HashSet::new();
    for _ in 0..=presses.len() {
        let mut grew = false;
        for &(node, count) in &presses {
            if done.contains(&node) || !shown.contains_key(&node) {
                continue;
            }
            done.insert(node);
            if !walk.opens(node) {
                continue;
            }
            let r = shown[&node];
            let mut hidden: Vec<(u32, f64, &'static str, &'static str)> = walk.follow(node).iter().filter(|s| !shown.contains_key(&s.other)).map(|s| (s.other, walk.through(node, &r, s), s.words(), back_words(s))).collect();
            if is_collection(index.address(node)) {
                // A collection opened: in a collection's order, those already under it counted.
                let order = walk.members(hidden.iter().map(|h| h.0).collect(), reference, under.get(&node).map_or(&[][..], |c| &c[..]));
                hidden.sort_by_key(|h| order.iter().position(|n| *n == h.0));
            } else {
                hidden.sort_by_cached_key(|(n, score, ..)| walk.key(*n, *score, reference));
            }
            for (other, score, how, back) in hidden.into_iter().take(count * weights.more) {
                under.entry(node).or_default().push(other);
                shown.insert(other, Reached { score, parent: Some(node), depth: r.depth + 1, how, back });
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }

    // The tree: rows by a post-order count, a thing beside the first that hangs from it; the outline in pre-order.
    let mut rows: HashMap<u32, usize> = HashMap::new();
    let mut next_row = 0usize;
    fn place(node: u32, under: &HashMap<u32, Vec<u32>>, rows: &mut HashMap<u32, usize>, next_row: &mut usize) -> usize {
        let row = match under.get(&node).filter(|c| !c.is_empty()) {
            Some(children) => {
                let first = place(children[0], under, rows, next_row);
                for &child in &children[1..] {
                    place(child, under, rows, next_row);
                }
                first
            }
            None => {
                *next_row += 1;
                *next_row - 1
            }
        };
        rows.insert(node, row);
        row
    }
    place(start, &under, &mut rows, &mut next_row);
    let mut outline: Vec<u32> = Vec::new();
    let mut stack = vec![start];
    while let Some(node) = stack.pop() {
        outline.push(node);
        if let Some(children) = under.get(&node) {
            stack.extend(children.iter().rev());
        }
    }
    let position: HashMap<u32, usize> = outline.iter().enumerate().map(|(i, n)| (*n, i)).collect();

    // Each thing, its words, its "+N".
    let mut things: Vec<ChainNode> = Vec::with_capacity(outline.len());
    for &node in &outline {
        let r = shown[&node];
        let found = walk.describe(node);
        let address = index.address(node);
        let collection = is_collection(address);
        let ties = walk.follow(node);
        let (more, more_words) = if walk.opens(node) {
            let mut kinds: Vec<(Kind, usize)> = Vec::new();
            for seen in ties.iter().filter(|s| !shown.contains_key(&s.other)) {
                let kind = crate::links::kind_of(index.address(seen.other));
                match kinds.iter_mut().find(|(k, _)| *k == kind) {
                    Some((_, n)) => *n += 1,
                    None => kinds.push((kind, 1)),
                }
            }
            kinds.sort_by_key(|(k, n)| (Reverse(*n), *k));
            let total: usize = kinds.iter().map(|(_, n)| n).sum();
            (total, more_sentence(tr, total, &kinds))
        } else {
            (0, String::new())
        };
        let parent = r.parent.and_then(|p| position.get(&p).copied());
        let chain = chain_words(tr, &walk, &shown, node);
        let opened = presses.iter().find(|(n, _)| *n == node).map_or(0, |(_, c)| *c);
        let parent_title = r.parent.map(|p| shown_title(tr, rule, &walk.describe(p)));
        let at = Placed { parent, column: r.depth, row: rows[&node], how: r.how, back: r.back, score: r.score, parent_title };
        let amount = index.named(address).and_then(|n| n.amount).filter(|_| found.kind == Kind::Movement);
        let counted = if collection { ties.len() } else { 0 };
        things.push(node_of(tr, &found, rule, &at, walk.refused(node), amount, (more, more_words), opened, counted, index.sessions(node), chain));
    }

    // Ties between things shown that are not the tree's; none to a collection, whose
    // count already says what it holds (from a project, a curve to each of its things).
    let mut cross: Vec<CrossTie> = Vec::new();
    for (i, &node) in outline.iter().enumerate() {
        if is_collection(index.address(node)) {
            continue;
        }
        for seen in walk.follow(node).iter().filter(|s| !is_collection(index.address(s.other))) {
            let Some(&j) = position.get(&seen.other) else { continue };
            let tree = shown[&node].parent == Some(seen.other) || shown[&seen.other].parent == Some(node);
            if j > i && !tree {
                cross.push(CrossTie { from: i, to: j, how: seen.words(), words: tr.text(&format!("how-{}", seen.words()), None) });
            }
        }
    }
    let health = things.iter().any(|t| t.health);
    let columns = things.iter().map(|t| t.column + 1).max().unwrap_or(1);
    ChainView { entry: index.address(start).to_string(), things, cross, health, columns, rows: next_row.max(1) }
}

/// "12 more: 9 messages, 3 tasks"; "" for none.
fn more_sentence(tr: &Translator, total: usize, kinds: &[(Kind, usize)]) -> String {
    if total == 0 {
        return String::new();
    }
    let parts: Vec<String> = kinds.iter().map(|(kind, n)| say(tr, &format!("chain-count-{}", kind_id(*kind)), &[("n", n.to_string())])).collect();
    say(tr, "chain-more", &[("n", total.to_string()), ("what", parts.join(", "))])
}

/// A thing's chain from the entry point, in words: each step's tie, seen from the thing before, and the thing.
fn chain_words(tr: &Translator, walk: &Walk, shown: &HashMap<u32, Reached>, node: u32) -> String {
    let mut steps: Vec<String> = Vec::new();
    let mut at = node;
    let mut seen = HashSet::new();
    while let Some(r) = shown.get(&at).filter(|r| r.parent.is_some()) {
        if !seen.insert(at) {
            break;
        }
        let found = walk.describe(at);
        let kind = tr.text(&format!("chain-kind-{}", kind_id(found.kind)), None);
        let how = tr.text(&format!("how-{}", r.how), None);
        steps.push(say(tr, "chain-step", &[("how", how), ("kind", kind), ("title", shown_title(tr, walk.rule, &found))]));
        at = r.parent.unwrap_or(at);
    }
    steps.reverse();
    steps.join(" · ")
}

/// A thing's title as the chain shows it: an availability-only event says "Busy".
fn shown_title(tr: &Translator, rule: &dyn Rule, found: &Related) -> String {
    if rule.busy(&found.uri) {
        tr.text("availability-busy", None)
    } else if found.title.trim().is_empty() {
        tr.text("task-untitled", None)
    } else {
        found.title.clone()
    }
}

/// Where a thing is in the tree, and how it hangs there.
struct Placed {
    parent: Option<usize>,
    column: usize,
    row: usize,
    how: &'static str,
    back: &'static str,
    score: f64,
    /// The title of the thing it hangs from, for its accessible name.
    parent_title: Option<String>,
}

/// One thing as the chain shows it: its words, its date, its accessible name.
#[allow(clippy::too_many_arguments)]
fn node_of(tr: &Translator, found: &Related, rule: &dyn Rule, at: &Placed, refused: Option<crate::porch::Reason>, amount: Option<crate::money::Money>, (more, more_words): (usize, String), opened: usize, count: usize, sessions: u32, chain: String) -> ChainNode {
    let busy = rule.busy(&found.uri);
    let title = shown_title(tr, rule, found);
    let zone = crate::zone::current();
    // A day, not an hour, for what happens on a day: a payment, a letter, an invoice, a paper, a contract.
    let day_kind = matches!(found.kind, Kind::Movement | Kind::Letter | Kind::Invoice | Kind::Paper | Kind::Contract);
    let date = found
        .when
        .and_then(|t| Timestamp::from_second(t).ok())
        .map(|t| {
            let at = t.to_zoned(zone.clone());
            if day_kind { tr.day(at.date()) } else { tr.date(&at, true) }
        })
        .unwrap_or_default();
    let detail = match (&refused, amount) {
        // Set aside: the Porch's own words, never its sender's.
        (Some(reason), _) => tr.reason(reason, None),
        (None, Some(amount)) => tr.money(amount),
        _ if busy => String::new(),
        _ => found.detail.clone(),
    };
    let kind_words = tr.text(&format!("chain-kind-{}", kind_id(found.kind)), None);
    let how_words = if at.how.is_empty() { String::new() } else { tr.text(&format!("how-{}", at.how), None) };
    // "Task: Pay the rent; Made from: Rent for November; 3 more: 2 messages, 1 task".
    let mut name = say(tr, "chain-name", &[("kind", kind_words.clone()), ("title", title.clone())]);
    if let Some(parent_title) = at.parent_title.as_ref().filter(|_| !at.back.is_empty()) {
        let tied = say(tr, "chain-tied", &[("how", tr.text(&format!("how-{}", at.back), None)), ("title", parent_title.clone())]);
        name = say(tr, "chain-and", &[("a", name), ("b", tied)]);
    }
    if !more_words.is_empty() {
        name = say(tr, "chain-and", &[("a", name), ("b", more_words.clone())]);
    }
    ChainNode {
        uri: found.uri.clone(),
        kind: found.kind,
        family: family(found.kind),
        kind_words,
        title,
        detail,
        date,
        when: found.when,
        key: found.key.clone(),
        found: found.found,
        program: found.program,
        column: at.column,
        row: at.row,
        parent: at.parent,
        how: at.how,
        how_words,
        score: at.score,
        more,
        more_words,
        opened,
        collection: is_collection(&found.uri),
        count,
        busy,
        set_aside: refused.is_some(),
        health: rule.health(&found.uri),
        sessions,
        chain,
        name,
    }
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
