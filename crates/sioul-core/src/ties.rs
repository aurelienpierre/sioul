// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The tie index: every tie Sioul knows, read from both ends, in memory
//! (docs/research/link-graph.md, §1).
//!
//! Each tie stays written where it is (`links`: "each link lives in the thing
//! that makes it"): in a task, an event, a note, a draft, a budget line, the
//! local `links.toml`, the money store, the persons, papers, letters and
//! contracts, a project's routes and files, a message's headers. The index
//! gathers them in one pass, the reverse entry made with the forward one, so
//! that a thing's ties are read from either end without going over
//! everything again. Nothing is copied into a store of its own and nothing
//! migrates.
//!
//! **It is never written anywhere**: not to a file, a cache, the sharing or a
//! log line. It holds the ties of sealed stores (persons, papers, health)
//! beside the plain ones, and written out it would undo their sealing.
//!
//! Addresses are kept once, in one table; a tie is two small numbers into it,
//! its kind (`How`), and what the Porch must say of a message at its ends for
//! the tie to be followed (`Need`). A tie Sioul computed from a message's
//! headers (a reply, a conversation, its sender, a project's routes, the ties
//! Sioul makes to a project as mail arrives) passes only through mail the
//! Porch admits, as a project's page and the agents' consent take it
//! (`projectview::mail_of`, `mailindex::Threads::through`): a forged message
//! citing two conversations never joins them. A tie made by hand is followed
//! as written.
//!
//! A movement is derived (`movements`), not stored: the index takes the
//! movements the budgets page derives, gives each its one address
//! (`Movement::address`), and maps every other key a movement is named by (its
//! line's, its occurrence's, its bank line's, `sioul:movement/<any of them>`)
//! to it.

use crate::agenda::EventRef;
use crate::config::Config;
use crate::consent::Consent;
use crate::contracts::Contract;
use crate::invoice::Invoice;
use crate::letters::Letters;
use crate::links::{self, Kind, Loaded, Related, World};
use crate::money::Money;
use crate::movements::Movement;
use crate::papers::Paper;
use crate::persons::Persons;
use crate::porch::{Admission, Gate};
use crate::timelog::Session;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

/// How two things are tied: what the tie says, in words from each end, and
/// how strongly it ties them (`chain::Weights::strength`). Declared from the
/// closest to the loosest: when two things are tied twice, the first says how.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum How {
    /// A task and the appointment it prepares (`pairs`).
    Appointment,
    /// A task that waits for another.
    Waits,
    /// A step and its bigger task.
    Step,
    /// Made from: a task from a message, a letter or a paper's renewal; a budget line from what tells of it.
    Source,
    /// A reply and the message it answers; a draft and the message it answers.
    Answers,
    /// A thing and the note that describes it.
    Note,
    /// A movement and what tells of it for sure: its line, a message, a letter, an invoice.
    Told,
    /// Tied by hand, or by a link that says nothing more.
    Link,
    /// A movement and what probably tells of it ("probably the same as …").
    ToldProbably,
    /// A movement and the contract it pays.
    Contract,
    /// A contract and the paper it is held in.
    ContractPaper,
    /// A paper or a letter and the person it is for.
    For,
    /// A stand-in event in your agenda and the child's event it stands for (`holders`).
    StandIn,
    /// A child's event and who takes it (`holders`).
    TakenBy,
    /// Movements together: instalments, an order charged in parts.
    Together,
    /// A person and their contact card.
    Card,
    /// A thing and its project: REFID, a tie, a note in its files, a letter's or an invoice's.
    Project,
    /// A task's contact, an event's guest.
    Contact,
    /// A message and its sender's card.
    Sender,
    /// A note and what it mentions.
    Mention,
    /// A movement and the budget it counts in.
    Budget,
    /// A project and the budget its invoices go to.
    ProjectBudget,
    /// A site and the person it is opened for.
    SiteFor,
    /// An event and the person whose agenda holds it.
    Agenda,
    /// A child's other parent and the child.
    Parent,
    /// Two messages of one conversation, the one citing the other without answering it.
    Conversation,
    /// A message and a project, by the project's routes alone.
    Routes,
    /// A project or an invoice and its client's card, by name.
    Client,
    /// A contract and the card of whom it is with, by name.
    Party,
}

impl How {
    /// Its words seen from its first end (`how-<id>` in the Fluent files): "source", Made from.
    pub fn forward(self) -> &'static str {
        match self {
            How::Appointment => "appointment",
            How::Waits => "waits-for",
            How::Step => "part-of",
            How::Source => "source",
            How::Answers => "answers",
            How::Note => "note",
            How::Told => "told-by",
            How::Link => "link",
            How::ToldProbably => "probably-told-by",
            How::Contract => "contract",
            How::ContractPaper => "paper",
            How::For => "for",
            How::StandIn => "stands-for",
            How::TakenBy => "taken-by",
            How::Together => "together",
            How::Card => "card",
            How::Project | How::Routes => "project",
            How::Contact => "contact",
            How::Sender => "from",
            How::Mention => "mentions",
            How::Budget | How::ProjectBudget => "budget",
            How::SiteFor => "opened-for",
            How::Agenda => "agenda-of",
            How::Parent => "parent-of",
            How::Conversation => "conversation",
            How::Client => "client",
            How::Party => "party",
        }
    }

    /// Its words seen from its second end: "made", Made into.
    pub fn back(self) -> &'static str {
        match self {
            How::Appointment => "prepared-by",
            How::Waits => "unblocks",
            How::Step => "step",
            How::Source => "made",
            How::Answers => "answered-by",
            How::Note => "notes-of",
            How::Told => "payment",
            How::Link => "link",
            How::ToldProbably => "probable-payment",
            How::Contract | How::Budget => "payments",
            How::ContractPaper | How::Party => "contract",
            How::For => "theirs",
            How::StandIn => "stand-in",
            How::TakenBy => "takes",
            How::Together => "together",
            How::Card => "person",
            How::Project | How::Routes => "in-project",
            How::Contact => "involves",
            How::Sender => "sent",
            How::Mention => "mentioned-by",
            How::ProjectBudget => "project",
            How::SiteFor => "site",
            How::Agenda => "agenda",
            How::Parent => "other-parent",
            How::Conversation => "conversation",
            How::Client => "client-of",
        }
    }

    /// Every kind, for the words' check.
    pub const ALL: [How; 29] = [
        How::Appointment,
        How::Waits,
        How::Step,
        How::Source,
        How::Answers,
        How::Note,
        How::Told,
        How::Link,
        How::ToldProbably,
        How::Contract,
        How::ContractPaper,
        How::For,
        How::StandIn,
        How::TakenBy,
        How::Together,
        How::Card,
        How::Project,
        How::Contact,
        How::Sender,
        How::Mention,
        How::Budget,
        How::ProjectBudget,
        How::SiteFor,
        How::Agenda,
        How::Parent,
        How::Conversation,
        How::Routes,
        How::Client,
        How::Party,
    ];
}

/// What the Porch must say of a message at a tie's end for the tie to be
/// followed (`porch::Admission`), as `projectview::mail_of` asks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Need {
    /// Written by hand or by a thing of yours: followed as written.
    Nothing,
    /// Computed from what the message says (a conversation, a route on its
    /// subject, a tie Sioul made as it arrived): never through mail the Porch set aside.
    NotSetAside,
    /// Computed from its sender's address (its card, a route on its sender):
    /// only when something proves it comes from that address.
    Proven,
}

/// What the window, or an agent, may follow (decision 20 of the design;
/// docs/research/link-graph.md, §7).
pub trait Rule {
    /// The Porch's word on a message stored at `path` (`porch::Gate`): none when it cannot tell.
    fn admission(&self, uri: &str, path: &Path) -> Option<Admission>;
    /// An event of an agenda shared for its availability alone: shown "Busy", never walked through.
    fn busy(&self, _uri: &str) -> bool {
        false
    }
    /// Health (`consent::Consent::keep_errands`): shown in the window, which then keeps itself from the overview of apps.
    fn health(&self, _uri: &str) -> bool {
        false
    }
    /// Whether a thing may be shown and walked through at all: always in the
    /// window; for an agent, as its consent says (phase 5).
    fn shows(&self, _uri: &str) -> bool {
        true
    }
}

/// Every message as it stands, nothing busy, nothing health: a test's rule,
/// or a reading with no Porch to ask (as `projectview::mail_of` without a gate).
pub struct Open;

impl Rule for Open {
    fn admission(&self, _uri: &str, _path: &Path) -> Option<Admission> {
        Some(Admission::Admitted)
    }
}

/// The window's rule: the Porch's checks for mail, the agendas shared for
/// their availability alone, and health, as agents' consent computes it.
pub struct Gates {
    /// The Porch's checks; none, every message as it stands.
    pub gate: Option<Gate>,
    /// The events of agendas shared for their availability alone, by address.
    pub busy: BTreeSet<String>,
    /// Health (`Consent::keep_errands`, `Consent::keep_agendas`): its `is_health`.
    pub health: Consent,
    /// The Porch's word on each message asked, once per reading: a message
    /// is asked at each of its ties.
    pub said: std::sync::Mutex<HashMap<String, Option<Admission>>>,
}

impl Rule for Gates {
    fn admission(&self, uri: &str, path: &Path) -> Option<Admission> {
        let Some(gate) = &self.gate else { return Some(Admission::Admitted) };
        if let Some(said) = self.said.lock().ok().and_then(|said| said.get(uri).cloned()) {
            return said;
        }
        let said = gate.admission_of_file(path);
        if let Ok(mut kept) = self.said.lock() {
            kept.insert(uri.to_string(), said.clone());
        }
        said
    }

    fn busy(&self, uri: &str) -> bool {
        self.busy.contains(uri)
    }

    fn health(&self, uri: &str) -> bool {
        self.health.is_health(uri)
    }
}

impl Gates {
    /// As the configuration says: its Porch (`porch::Gate::load`), its
    /// agendas shared for availability alone, its errands and children's
    /// health events (as `sioul mcp` keeps them from agents).
    pub fn read(config: &Config, loaded: &Loaded, sources: &Sources) -> Gates {
        let gate = Gate::load(config, crate::zone::now().timestamp().as_second());
        let collections = crate::vdir::collections(crate::vdir::Kind::Calendars);
        let busy_dirs: BTreeSet<std::path::PathBuf> = crate::agendas::all(config).iter().filter(|a| a.busy()).filter_map(|a| collections.iter().find(|c| crate::agendas::place(c) == a.id).map(|c| c.dir.clone())).collect();
        let busy = loaded.events.iter().filter(|e| Path::new(&e.key).parent().is_some_and(|d| busy_dirs.contains(d))).map(|e| links::event_uri(&e.uid)).collect();
        let mut health = Consent::new(&[], true);
        let recorded = crate::health::HealthState::errand_uids(&crate::health::HealthState::default_path());
        health.keep_errands(loaded, recorded.as_ref());
        health.keep_agendas(loaded, &BTreeSet::new(), &sources.children);
        Gates { gate: Some(gate), busy, health, said: Default::default() }
    }
}

/// The index and the rule it is read by, as a `World` carries them (`Loaded::world_with`).
#[derive(Clone, Copy)]
pub struct Ties<'a> {
    pub index: &'a TieIndex,
    pub rule: &'a dyn Rule,
}

/// What the index read in stores of their own names a thing by: who a
/// movement is with, its amount and its day; a paper's, a contract's, an
/// invoice's, a person's or a budget's title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Named {
    pub title: String,
    /// Names only, no sentence: a paper's holder, a contract's party, an invoice's number.
    pub detail: String,
    /// Its date, Unix seconds: a movement's day, a letter's arrival, an invoice's date.
    pub when: Option<i64>,
    /// A movement's amount, signed (+ in, − out).
    pub amount: Option<Money>,
}

/// Everything the index reads beyond `Loaded`: the stores with ties of their own.
#[derive(Debug, Clone, Default)]
pub struct Sources {
    /// The movements the budgets page derives (`movements::books`).
    pub movements: Vec<Movement>,
    /// Each budget's id and title.
    pub budgets: Vec<(String, String)>,
    pub persons: Persons,
    pub papers: Vec<Paper>,
    pub letters: Option<Letters>,
    pub contracts: Vec<Contract>,
    pub invoices: Vec<Invoice>,
    /// The time noted (`timelog`): folded into each task as a count, never a thing of its own.
    pub sessions: Vec<Session>,
    /// The events of a person's agenda (a child's, a partner's, another parent's): (event UID, person id).
    pub agendas: Vec<(String, String)>,
    /// Who takes a child's event (`holders`): (event UID, their address).
    pub taken: Vec<(String, String)>,
    /// The children's agendas, by place (`agendas::place`).
    pub children: BTreeSet<String>,
}

impl Sources {
    /// Read as the configuration says: the notes folder's budget file, bank,
    /// contracts, letters, papers and money store; the persons; this device's
    /// invoices and time; the agendas and whose they are.
    pub fn read(config: &Config, loaded: &Loaded) -> Sources {
        let root = config.notes_root_path();
        let words = crate::words::Words::of(config);
        let today = crate::zone::now().date();
        // A budget file that does not read counts as none, as `Loaded::read` takes it.
        let books = root.as_deref().filter(|r| crate::budget::Ledger::load(r).is_ok()).map(|r| crate::movements::books(r, &words, today));
        let papers = root.as_deref().and_then(|r| crate::papers::Wallet::load(r).ok()).map(|w| w.papers).unwrap_or_default();
        let own: Vec<String> = config.every_account().filter_map(|a| a.name.clone()).collect();
        let persons = Persons::as_read(&Persons::default_path(), &config.agenda.calendars, &papers, &own);
        let (agendas, taken, children) = agenda_ties(config, &loaded.events, &persons);
        Sources {
            movements: books.as_ref().map(|b| b.movements.clone()).unwrap_or_default(),
            budgets: books.as_ref().map(|b| b.ledger.budgets.iter().map(|budget| (budget.id.clone(), budget.title.clone())).collect()).unwrap_or_default(),
            persons,
            papers,
            letters: root.as_deref().and_then(|r| Letters::load(r).ok()),
            contracts: root.as_deref().and_then(|r| crate::contracts::Contracts::load(r).ok()).map(|c| c.list).unwrap_or_default(),
            invoices: crate::invoice::all_in(&crate::invoice::folder()),
            sessions: crate::timelog::sessions(),
            agendas,
            taken,
            children,
        }
    }
}

/// The events of each person's agenda, who takes each child's event, and
/// the children's agendas by place.
fn agenda_ties(config: &Config, events: &[EventRef], persons: &Persons) -> (Vec<(String, String)>, Vec<(String, String)>, BTreeSet<String>) {
    use crate::agendas::Whose;
    let collections = crate::vdir::collections(crate::vdir::Kind::Calendars);
    let zone = crate::zone::current();
    let (mut held, mut taken, mut children) = (Vec::new(), Vec::new(), BTreeSet::new());
    for agenda in crate::agendas::all(config).into_iter().map(|a| a.with_persons(persons)) {
        if !matches!(agenda.whose, Whose::Child | Whose::Partner | Whose::Parent) {
            continue;
        }
        if agenda.whose == Whose::Child {
            children.insert(agenda.id.clone());
        }
        let Some(dir) = collections.iter().find(|c| crate::agendas::place(c) == agenda.id).map(|c| c.dir.clone()) else { continue };
        for event in events.iter().filter(|e| Path::new(&e.key).parent() == Some(dir.as_path())) {
            if !agenda.person_id.is_empty() {
                held.push((event.uid.clone(), agenda.person_id.clone()));
            }
            // Who takes a child's event: its own lines, read for a child's events alone.
            if agenda.whose == Whose::Child
                && let Ok(text) = std::fs::read_to_string(&event.key)
            {
                let said = crate::holders::taken(&text, (event.start > 0).then_some(event.start), &zone);
                for holder in said.to.iter().chain(said.back.iter()) {
                    taken.push((event.uid.clone(), holder.address.clone()));
                }
            }
        }
    }
    (held, taken, children)
}

/// One tie, as two places in the table of addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tie {
    from: u32,
    to: u32,
    how: How,
    need: Need,
}

/// One tie seen from one of its ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    /// The thing at the other end.
    pub other: u32,
    pub how: How,
    /// Seen from the tie's first end ("made from"), not its second ("made into").
    pub forward: bool,
    pub need: Need,
}

impl Seen {
    /// Its words' id, from the end it is seen from (`how-<id>`).
    pub fn words(&self) -> &'static str {
        if self.forward { self.how.forward() } else { self.how.back() }
    }
}

/// Every tie Sioul knows, from both ends.
#[derive(Debug, Default)]
pub struct TieIndex {
    /// Each thing's address, once.
    names: Vec<Arc<str>>,
    ids: HashMap<Arc<str>, u32>,
    ties: Vec<Tie>,
    /// The ties of thing `n` are `at[start[n]..start[n + 1]]`.
    start: Vec<u32>,
    at: Vec<u32>,
    /// Every other address a movement goes by, to its place.
    movements: HashMap<String, u32>,
    named: HashMap<u32, Named>,
    /// The time sessions noted on each task.
    sessions: HashMap<u32, u32>,
    /// How long the build took, for the `sioul-perf` line.
    pub built_in: std::time::Duration,
}

/// The index as it is filled.
#[derive(Default)]
struct Builder {
    index: TieIndex,
    seen: HashSet<(u32, u32, How)>,
}

impl Builder {
    fn node(&mut self, address: &str) -> u32 {
        if let Some(&id) = self.index.movements.get(address) {
            return id;
        }
        if let Some(&id) = self.index.ids.get(address) {
            return id;
        }
        let id = self.index.names.len() as u32;
        let name: Arc<str> = Arc::from(address);
        self.index.names.push(name.clone());
        self.index.ids.insert(name, id);
        id
    }

    fn tie(&mut self, from: &str, to: &str, how: How, need: Need) {
        if from.is_empty() || to.is_empty() {
            return;
        }
        let (from, to) = (self.node(from), self.node(to));
        if from != to && self.seen.insert((from, to, how)) {
            self.index.ties.push(Tie { from, to, how, need });
        }
    }

    fn name(&mut self, address: &str, named: Named) {
        let id = self.node(address);
        self.index.named.insert(id, named);
    }

    fn finish(mut self) -> TieIndex {
        let n = self.index.names.len();
        let mut count = vec![0u32; n + 1];
        for tie in &self.index.ties {
            count[tie.from as usize + 1] += 1;
            count[tie.to as usize + 1] += 1;
        }
        for i in 0..n {
            count[i + 1] += count[i];
        }
        let mut fill = count.clone();
        let mut at = vec![0u32; self.index.ties.len() * 2];
        for (i, tie) in self.index.ties.iter().enumerate() {
            for end in [tie.from, tie.to] {
                at[fill[end as usize] as usize] = i as u32;
                fill[end as usize] += 1;
            }
        }
        self.index.start = count;
        self.index.at = at;
        self.index.ids.shrink_to_fit();
        self.index.ties.shrink_to_fit();
        self.index
    }
}

/// A day as Unix seconds, at its start where you are.
fn day_seconds(day: jiff::civil::Date, zone: &jiff::tz::TimeZone) -> Option<i64> {
    day.to_zoned(zone.clone()).ok().map(|z| z.timestamp().as_second())
}

/// A name as compared: case and accents aside, spaces made one (`persons::key`).
fn name_key(name: &str) -> String {
    crate::persons::key(name)
}

impl TieIndex {
    /// The index of everything `world` and `sources` hold, built in one pass
    /// (linear in the number of ties, plus a project's routes for each message).
    pub fn build(world: &World, sources: &Sources) -> TieIndex {
        let began = std::time::Instant::now();
        let zone = crate::zone::current();
        let mut b = Builder::default();
        // Cards by address and by name, once: a message's sender, a project's client.
        let mut by_address: HashMap<String, String> = HashMap::new();
        let mut by_name: HashMap<String, Vec<String>> = HashMap::new();
        for contact in world.contacts {
            for email in &contact.emails {
                by_address.entry(email.value.trim().to_ascii_lowercase()).or_insert_with(|| contact.uid.clone());
            }
            for name in [&contact.name, &contact.org].into_iter().filter(|n| !n.trim().is_empty()) {
                let cards = by_name.entry(name_key(name)).or_default();
                if !cards.contains(&contact.uid) {
                    cards.push(contact.uid.clone());
                }
            }
        }
        // The one card a name gives, when it gives one.
        let one_named = |name: &str| by_name.get(&name_key(name)).filter(|cards| cards.len() == 1).map(|cards| links::contact_uri(&cards[0]));

        // Movements first: every key they go by names them from here on.
        for m in &sources.movements {
            let me = m.address();
            let id = b.node(&me);
            for key in std::iter::once(&m.id).chain(m.sources.iter().filter(|s| s.kind != crate::movements::Kind::Contract).map(|s| &s.key)) {
                b.index.movements.entry(links::movement_uri(key)).or_insert(id);
                // A key that is no address of its own (a line's, an occurrence's, a bank line's): the movement's.
                if !matches!(crate::movements::Kind::of_key(key), Some(crate::movements::Kind::Mail | crate::movements::Kind::Letter | crate::movements::Kind::Invoice | crate::movements::Kind::Contract)) {
                    b.index.movements.entry(key.clone()).or_insert(id);
                }
            }
        }
        for m in &sources.movements {
            let me = m.address();
            b.name(&me, Named { title: m.label.clone(), detail: String::new(), when: day_seconds(m.day, &zone), amount: Some(m.sum.amount()) });
            for source in &m.sources {
                use crate::movements::Kind as Source;
                let told = if source.probable { How::ToldProbably } else { How::Told };
                match source.kind {
                    Source::Line if !source.address.is_empty() => b.tie(&me, &source.address, told, Need::Nothing),
                    // A message told of it: never through one the Porch set aside.
                    Source::Mail => b.tie(&me, &world.canonical(&source.key), told, Need::NotSetAside),
                    Source::Letter | Source::Invoice => b.tie(&me, &world.canonical(&source.key), told, Need::Nothing),
                    Source::Contract => b.tie(&me, &source.key, How::Contract, Need::Nothing),
                    _ => {}
                }
            }
            for link in &m.links {
                let other = world.canonical(link);
                let how = match links::kind_of(&other) {
                    Kind::Project => How::Project,
                    Kind::Contact => How::Contact,
                    Kind::Movement => How::Together,
                    _ => How::Link,
                };
                b.tie(&me, &other, how, Need::Nothing);
            }
            for other in &m.together {
                let other = if other.starts_with("sioul:movement/") { other.clone() } else { links::movement_uri(other) };
                b.tie(&me, &other, How::Together, Need::Nothing);
            }
            for budget in m.place.budgets() {
                b.tie(&me, &links::budget_whole_uri(budget), How::Budget, Need::Nothing);
            }
        }
        for (id, title) in &sources.budgets {
            b.name(&links::budget_whole_uri(id), Named { title: title.clone(), ..Named::default() });
        }

        // What `World::edges` gathers: tasks, events, notes, drafts, budget lines, `links.toml`.
        let stand_ins: HashSet<(String, String)> = world.events.iter().flat_map(|e| e.related.iter().filter(|r| r.kind.eq_ignore_ascii_case("PARENT")).map(move |r| (links::event_uri(&e.uid), world.canonical(&format!("uid:{}", r.uid))))).collect();
        for edge in world.edges() {
            let mail_end = links::kind_of(&edge.from) == Kind::Mail || links::kind_of(&edge.to) == Kind::Mail;
            let (how, need) = match edge.how.as_str() {
                "note" => (How::Note, Need::Nothing),
                // What a budget line's links name (all "source" in `edges`): a project it is in, a card it
                // involves, else what tells of it.
                "source" => match links::kind_of(&edge.to) {
                    Kind::Project => (How::Project, Need::Nothing),
                    Kind::Contact => (How::Contact, Need::Nothing),
                    _ => (How::Source, Need::Nothing),
                },
                "waits" => (How::Waits, Need::Nothing),
                "step" => (How::Step, Need::Nothing),
                "contact" => (How::Contact, Need::Nothing),
                // Sioul's own ties of a message to a project, made as it arrives, are weighed as its routes are.
                "project" | "case" => (How::Project, if mail_end { Need::NotSetAside } else { Need::Nothing }),
                "mention" => (How::Mention, Need::Nothing),
                "answers" => (How::Answers, Need::Nothing),
                "appointment" => (How::Appointment, Need::Nothing),
                crate::persons::HOLDER | crate::persons::ADDRESSED => (How::For, Need::Nothing),
                crate::persons::FOR => (How::SiteFor, Need::Nothing),
                _ if stand_ins.contains(&(edge.from.clone(), edge.to.clone())) => (How::StandIn, Need::Nothing),
                _ => (How::Link, Need::Nothing),
            };
            b.tie(&edge.from, &edge.to, how, need);
        }

        // Mail: its sender's card, the message it answers, the others of its conversation.
        let messages = world.mail.iter().map(|(id, mail)| (links::mail_uri(id), mail)).chain(world.mail.without_id());
        for (me, mail) in messages {
            if let Some(card) = by_address.get(&mail.address.trim().to_ascii_lowercase()) {
                b.tie(&me, &links::contact_uri(card), How::Sender, Need::Proven);
            }
            // In-Reply-To first (`MailRef::refs`): the message it answers, here or not; then
            // the others it cites, when they are here.
            for (i, cited) in mail.refs.iter().enumerate() {
                if i == 0 {
                    b.tie(&me, &links::mail_uri(cited), How::Answers, Need::NotSetAside);
                } else if world.mail.get(cited).is_some() {
                    b.tie(&me, &links::mail_uri(cited), How::Conversation, Need::NotSetAside);
                }
            }
            // A project's routes on the sender and the subject (`Route::takes_header`): on the
            // subject alone, any message the Porch does not set aside; on its sender, a proven one.
            for project in world.projects.iter().filter(|p| !p.routes.is_empty()) {
                let by_subject = project.routes.iter().any(|r| r.takes_header("", &mail.subject));
                let by_sender = by_subject || project.routes.iter().any(|r| r.takes_header(&mail.address, &mail.subject));
                if by_sender {
                    b.tie(&me, &links::project_uri(&project.id), How::Routes, if by_subject { Need::NotSetAside } else { Need::Proven });
                }
            }
        }

        // Projects: the notes their files name, their client, their budget.
        for project in world.projects {
            let me = links::project_uri(&project.id);
            let files: Vec<&str> = project.files.iter().map(|f| f.trim().trim_start_matches("./").trim_matches('/')).filter(|f| !f.is_empty()).collect();
            if !files.is_empty()
                && let Some(vault) = world.vault
            {
                for note in &vault.notes {
                    let path = note.path.trim_start_matches('/');
                    if files.iter().any(|f| path == *f || path.strip_prefix(f).is_some_and(|rest| rest.starts_with('/'))) {
                        b.tie(&note.uri(), &me, How::Project, Need::Nothing);
                    }
                }
            }
            if let Some(client) = project.client.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
                let card = world.canonical(client);
                if links::kind_of(&card) == Kind::Contact {
                    b.tie(&me, &card, How::Contact, Need::Nothing);
                } else {
                    // By its name or organisation, every card so named (as agents' consent reads it).
                    for uid in by_name.get(&name_key(client)).into_iter().flatten() {
                        b.tie(&me, &links::contact_uri(uid), How::Client, Need::Nothing);
                    }
                }
            }
            if let Some(budget) = project.budget.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
                b.tie(&me, &links::budget_whole_uri(budget), How::ProjectBudget, Need::Nothing);
            }
        }

        // Persons: their cards, a child's other parent.
        for person in &sources.persons.persons {
            let me = links::person_uri(&person.id);
            b.name(&me, Named { title: person.name.clone(), detail: person.relation.clone(), ..Named::default() });
            for card in &person.cards {
                b.tie(&me, &links::contact_uri(card), How::Card, Need::Nothing);
            }
            for child in &person.of {
                b.tie(&me, &links::person_uri(child), How::Parent, Need::Nothing);
            }
        }
        // Who takes a child's event: a person by one of their cards' addresses, else a card.
        let person_by_address: HashMap<String, String> = sources
            .persons
            .persons
            .iter()
            .flat_map(|p| p.cards.iter().filter_map(|uid| world.contacts.iter().find(|c| c.uid == *uid)).flat_map(|c| c.emails.iter().map(|e| e.value.trim().to_ascii_lowercase())).map(move |a| (a, p.id.clone())))
            .collect();
        for (uid, address) in &sources.taken {
            let address = address.trim().to_ascii_lowercase();
            let who = person_by_address.get(&address).map(|id| links::person_uri(id)).or_else(|| by_address.get(&address).map(|uid| links::contact_uri(uid)));
            if let Some(who) = who {
                b.tie(&links::event_uri(uid), &who, How::TakenBy, Need::Nothing);
            }
        }
        for (uid, person) in &sources.agendas {
            b.tie(&links::event_uri(uid), &links::person_uri(person), How::Agenda, Need::Nothing);
        }

        // Papers: the task made to renew each.
        for paper in &sources.papers {
            let me = links::paper_uri(&paper.id);
            let when = paper.until.or(paper.issued).and_then(|d| day_seconds(d, &zone));
            b.name(&me, Named { title: if paper.title.trim().is_empty() { paper.id.clone() } else { paper.title.clone() }, detail: paper.holder.clone(), when, amount: None });
            if !paper.renewal.trim().is_empty() {
                b.tie(&world.canonical(&format!("uid:{}", paper.renewal.trim())), &me, How::Source, Need::Nothing);
            }
        }

        // Letters: their project, the task made for their date, an event standing for one by its scan.
        if let Some(letters) = &sources.letters {
            for letter in &letters.list {
                let me = links::letter_uri(&letter.id);
                let sender = letter.reading.sender.trim();
                b.name(&me, Named { title: if sender.is_empty() { letter.id.clone() } else { sender.to_string() }, detail: String::new(), when: letter.received.and_then(|d| day_seconds(d, &zone)), amount: None });
                if !letter.project.trim().is_empty() {
                    b.tie(&me, &links::project_uri(letter.project.trim()), How::Project, Need::Nothing);
                }
                if !letter.task.trim().is_empty() {
                    b.tie(&world.canonical(&format!("uid:{}", letter.task.trim())), &me, How::Source, Need::Nothing);
                }
                if !letter.file.is_empty() {
                    for event in world.events {
                        if event.links.iter().any(|l| crate::dated::links_scan(l.uri.trim(), &letter.file, &letters.root)) {
                            b.tie(&links::event_uri(&event.uid), &me, How::Link, Need::Nothing);
                        }
                    }
                }
            }
        }

        // Contracts: the paper each is held in, whom it is with.
        for contract in &sources.contracts {
            let me = links::contract_uri(&contract.id);
            let title = if contract.title.trim().is_empty() { contract.party.clone() } else { contract.title.clone() };
            b.name(&me, Named { title, detail: contract.party.clone(), when: contract.started.and_then(|d| day_seconds(d, &zone)), amount: None });
            if !contract.paper.trim().is_empty() {
                b.tie(&me, &links::paper_uri(contract.paper.trim()), How::ContractPaper, Need::Nothing);
            }
            if let Some(card) = one_named(&contract.party) {
                b.tie(&me, &card, How::Party, Need::Nothing);
            }
        }

        // Invoices: their project, their client.
        for invoice in sources.invoices.iter().filter(|i| !i.number.is_empty()) {
            let me = links::invoice_uri(&invoice.number);
            let when = invoice.date.trim().parse::<jiff::civil::Date>().ok().and_then(|d| day_seconds(d, &zone));
            let title = if invoice.client.trim().is_empty() { invoice.project_title.clone() } else { invoice.client.clone() };
            b.name(&me, Named { title, detail: invoice.number.clone(), when, amount: Some(Money(invoice.total_cents)) });
            if !invoice.project.trim().is_empty() {
                b.tie(&me, &links::project_uri(invoice.project.trim()), How::Project, Need::Nothing);
            }
            if let Some(card) = one_named(&invoice.client) {
                b.tie(&me, &card, How::Client, Need::Nothing);
            }
        }

        // Time noted: a count on its task, never a thing of its own.
        for session in sources.sessions.iter().filter(|s| !s.task.trim().is_empty()) {
            let task = b.node(&links::task_uri(session.task.trim()));
            *b.index.sessions.entry(task).or_default() += 1;
        }

        let mut index = b.finish();
        index.built_in = began.elapsed();
        index
    }

    /// How many ties.
    pub fn len(&self) -> usize {
        self.ties.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ties.is_empty()
    }

    /// How many things it names.
    pub fn things(&self) -> usize {
        self.names.len()
    }

    /// What it holds in memory, about, in bytes: the `sioul-perf` line's measure.
    pub fn bytes(&self) -> usize {
        let names: usize = self.names.iter().map(|n| n.len() + 16 + 8).sum();
        let ids = self.ids.capacity() * (16 + 4 + 8);
        let named: usize = self.named.values().map(|n| n.title.len() + n.detail.len() + 96).sum();
        names + ids + self.ties.capacity() * std::mem::size_of::<Tie>() + (self.start.len() + self.at.len()) * 4 + self.movements.iter().map(|(k, _)| k.len() + 40).sum::<usize>() + named + self.sessions.len() * 16
    }

    /// The place of a thing, by its address as `World::canonical` writes it,
    /// or by any key a movement goes by.
    pub fn node(&self, address: &str) -> Option<u32> {
        self.movements.get(address).or_else(|| self.ids.get(address)).copied()
    }

    /// A thing's address.
    pub fn address(&self, node: u32) -> &str {
        &self.names[node as usize]
    }

    /// The one address of a movement named by any of its keys
    /// (`sioul:movement/<key>`, or the key itself); none for anything else.
    pub fn movement_address(&self, key: &str) -> Option<&str> {
        let key = key.trim();
        if !key.starts_with("sioul:movement/") && !matches!(crate::movements::Kind::of_key(key), Some(crate::movements::Kind::Line | crate::movements::Kind::Preset | crate::movements::Kind::Bank | crate::movements::Kind::Hold | crate::movements::Kind::SetAside)) {
            return None;
        }
        // Written another way than the index wrote it ("%2F" for "/", or not): the same key.
        let found = self.movements.get(key).or_else(|| key.strip_prefix("sioul:movement/").and_then(|rest| self.movements.get(&links::movement_uri(&crate::notes::decode(rest)))));
        found.map(|&n| self.address(n))
    }

    /// What its stores name a thing by (`Named`), by its address.
    pub fn named(&self, address: &str) -> Option<&Named> {
        self.node(address).and_then(|n| self.named.get(&n))
    }

    /// The time sessions noted on a task.
    pub fn sessions(&self, node: u32) -> u32 {
        self.sessions.get(&node).copied().unwrap_or(0)
    }

    /// Every tie of a thing, seen from it, in the order they were read.
    pub fn ties_of(&self, node: u32) -> impl Iterator<Item = Seen> + '_ {
        let n = node as usize;
        let range = self.start.get(n).zip(self.start.get(n + 1)).map_or(0..0, |(a, b)| *a as usize..*b as usize);
        self.at[range].iter().map(move |&i| {
            let tie = self.ties[i as usize];
            let forward = tie.from == node;
            Seen { other: if forward { tie.to } else { tie.from }, how: tie.how, forward, need: tie.need }
        })
    }

    /// Whether a tie may be followed under `rule`: what it needs of the
    /// Porch, at each end that is a message here (one not here passes, as an
    /// id cited but not kept still ties a conversation); and `rule.shows` of
    /// the thing at its other end.
    pub fn follows(&self, world: &World, rule: &dyn Rule, from: u32, seen: &Seen) -> bool {
        let other = self.address(seen.other);
        if !rule.shows(other) {
            return false;
        }
        if seen.need == Need::Nothing {
            return true;
        }
        [self.address(from), other].into_iter().all(|end| admits(world, rule, end, seen.need))
    }

    /// What `World::related` gives with the index: what is tied to `uri`,
    /// both ways, each once, the closest tie saying how, as the rule lets it be followed.
    pub fn related(&self, world: &World, uri: &str, rule: &dyn Rule) -> Vec<Related> {
        let me = world.canonical(uri);
        let Some(node) = self.node(&me) else { return Vec::new() };
        let mut closest: Vec<(u32, How, &'static str)> = Vec::new();
        for seen in self.ties_of(node).filter(|s| self.follows(world, rule, node, s)) {
            match closest.iter_mut().find(|(other, ..)| *other == seen.other) {
                Some(row) if seen.how < row.1 => *row = (seen.other, seen.how, seen.words()),
                Some(_) => {}
                None => closest.push((seen.other, seen.how, seen.words())),
            }
        }
        let mut out: Vec<Related> = closest
            .into_iter()
            .map(|(other, _, words)| {
                let mut found = world.describe(self.address(other));
                found.how = words.to_string();
                found
            })
            .collect();
        out.sort_by(|a, b| (a.kind, a.when.map(|w| -w), a.title.to_lowercase()).cmp(&(b.kind, b.when.map(|w| -w), b.title.to_lowercase())));
        out
    }
}

/// Whether a message at `address` passes what a tie needs of the Porch;
/// anything not a message here passes.
pub fn admits(world: &World, rule: &dyn Rule, address: &str, need: Need) -> bool {
    if need == Need::Nothing || links::kind_of(address) != Kind::Mail {
        return true;
    }
    let Some(mail) = world.mail.get(address) else { return true };
    match rule.admission(address, &mail.path) {
        Some(Admission::Admitted) => true,
        Some(Admission::Unproven) => need == Need::NotSetAside,
        _ => false,
    }
}

/// What the Porch says of a message here: set aside (`Admission::Refused`), with its words' reason.
pub fn set_aside(world: &World, rule: &dyn Rule, address: &str) -> Option<crate::porch::Reason> {
    if links::kind_of(address) != Kind::Mail {
        return None;
    }
    let mail = world.mail.get(address)?;
    match rule.admission(address, &mail.path) {
        Some(Admission::Refused(_, reason)) => Some(reason),
        _ => None,
    }
}

#[cfg(test)]
#[path = "ties_tests.rs"]
pub(crate) mod tests;
