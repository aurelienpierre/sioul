// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The tie index on stand-ins: every source read from both ends, the
//! request's chain from each of its ends, a forged reply that joins nothing.

use super::*;
use crate::agenda::EventRef;
use crate::compose::Draft;
use crate::contacts::{Contact, Labeled};
use crate::links::{BudgetRef, LocalLinks};
use crate::mailindex::{MailIndex, MailRef};
use crate::notes::{self, Vault};
use crate::projects::{Project, Route};
use crate::tasks::{ContactRef, Link, Relation, Task};
use std::path::PathBuf;

/// A movement as the budgets page derives it, from its JSON form (its amount in cents here, in euros there).
pub(crate) fn movement(id: &str, label: &str, day: &str, cents: i64, sources: serde_json::Value, links: &[&str], budget: &str) -> Movement {
    serde_json::from_value(serde_json::json!({
        "id": id, "account": null, "way": "out", "sum": {"Exact": cents as f64 / 100.0}, "currency": "EUR", "day": day, "expected": null,
        "label": label, "place": {"Budget": budget}, "area": "", "sources": sources, "links": links, "state": "done",
        "together": [], "waits": false, "why": null, "evidence": null, "refs": [], "names": [], "stands": null, "bank_label": null
    }))
    .unwrap()
}

fn source(kind: &str, key: &str, address: &str, probable: bool) -> serde_json::Value {
    serde_json::json!({"kind": kind, "key": key, "address": address, "probable": probable})
}

fn mail(subject: &str, address: &str, date: i64, refs: &[&str]) -> MailRef {
    MailRef { path: PathBuf::from(format!("/stand-in/mail/cur/{}", subject.replace(' ', "-"))), subject: subject.into(), from: address.into(), address: address.into(), date, refs: refs.iter().map(|r| r.to_string()).collect() }
}

/// Everything a person's admin may hold, invented: the rent told by mail,
/// paid, its task in the project Flat; a child's agenda; papers, a
/// contract, a letter, an invoice.
pub(crate) struct Fixture {
    pub tasks: Vec<Task>,
    pub events: Vec<EventRef>,
    pub contacts: Vec<Contact>,
    pub vault: Vault,
    pub drafts: Vec<Draft>,
    pub mail: MailIndex,
    pub projects: Vec<Project>,
    pub budget: Vec<BudgetRef>,
    pub local: LocalLinks,
    pub sources: Sources,
}

pub(crate) const NOV_2: i64 = 1_793_620_800; // 2026-11-02T10:00:00Z

impl Fixture {
    pub(crate) fn new() -> Fixture {
        let task = |uid: &str, title: &str| Task { uid: uid.into(), title: title.into(), key: format!("/stand-in/cal/{uid}.ics"), ..Task::default() };
        let link = |uri: &str, rel: &str| Link { uri: uri.into(), label: String::new(), rel: rel.into() };
        let tasks = vec![
            Task { links: vec![link("mid:<rent-11@landlord.example>", "via")], projects: vec!["flat".into()], contacts: vec![ContactRef { name: "Mina Okafor".into(), uri: "sioul:contact/mina".into() }], ..task("pay-rent", "Pay the rent") },
            Task { relations: vec![Relation { kind: "PARENT".into(), uid: "pay-rent".into(), gap: 0 }], ..task("find-iban", "Find the IBAN") },
            Task { relations: vec![Relation { kind: "DEPENDS-ON".into(), uid: "find-iban".into(), gap: 0 }], ..task("send-proof", "Send the proof") },
            Task { links: vec![link("uid:dentist", crate::pairs::APPOINTMENT), link("sioul:note/dentist.md", "describedby")], ..task("prepare-dentist", "Prepare the dentist") },
            task("renew-card", "Renew the identity card"),
            task("answer-caf", "Answer the CAF letter"),
        ];
        let events = vec![
            EventRef { uid: "dentist".into(), summary: "Dentist".into(), start: NOV_2 + 86_400, key: "/stand-in/cal/dentist.ics".into(), people: vec!["desk@dentist.example".into()], ..EventRef::default() },
            EventRef { uid: "school-trip".into(), summary: "School trip".into(), start: NOV_2 + 3 * 86_400, key: "/stand-in/child/school-trip.ics".into(), ..EventRef::default() },
            EventRef { uid: "trip-stand-in".into(), summary: "School trip".into(), start: NOV_2 + 3 * 86_400, key: "/stand-in/cal/trip-stand-in.ics".into(), related: vec![Relation { kind: "PARENT".into(), uid: "school-trip".into(), gap: 0 }], ..EventRef::default() },
            EventRef { uid: "caf-visit".into(), summary: "At the CAF".into(), start: NOV_2 + 5 * 86_400, key: "/stand-in/cal/caf-visit.ics".into(), links: vec![link("file:///stand-in/notes/letters/caf.pdf", "related")], ..EventRef::default() },
        ];
        let card = |uid: &str, name: &str, email: &str| Contact { uid: uid.into(), name: name.into(), emails: vec![Labeled { label: String::new(), value: email.into() }], ..Contact::default() };
        let contacts = vec![card("mina", "Mina Okafor", "mina@landlord.example"), card("dentist-desk", "Dr Ilsa Varga", "desk@dentist.example"), card("sam", "Sam Brook", "sam@example.org"), card("edf", "Énergie Exemple", "contact@energy.example"), card("north", "North Studio", "hello@north.example")];
        let vault = Vault {
            root: PathBuf::from("/stand-in/notes"),
            notes: vec![notes::read("dentist.md", "# Dentist\n\nAsk about [the rent](sioul:task/pay-rent).\n"), notes::read("flat/lease.md", "# Lease\n")],
            folders: vec!["flat".into()],
        };
        let drafts = vec![Draft { id: "reply-rent".into(), subject: "Re: Rent for November".into(), in_reply_to: Some("rent-11@landlord.example".into()), ..Draft::default() }];
        let mut mail = MailIndex::default();
        mail.insert("rent-11@landlord.example", mail_ref("Rent for November", "mina@landlord.example", NOV_2, &[]));
        mail.insert("rent-11-re@example.net", mail_ref("Re: Rent for November", "me@example.net", NOV_2 + 3600, &["rent-11@landlord.example"]));
        mail.insert("rent-11-re2@landlord.example", mail_ref("Re: Re: Rent for November", "mina@landlord.example", NOV_2 + 7200, &["rent-11-re@example.net", "rent-11@landlord.example"]));
        mail.insert("studio@north.example", mail_ref("Studio hours", "hello@north.example", NOV_2 - 86_400, &[]));
        let projects = vec![
            Project { id: "flat".into(), title: "Flat".into(), routes: vec![Route { subject_contains: vec!["rent".into()], ..Route::default() }], files: vec!["flat".into()], ..Project::default() },
            Project { id: "studio".into(), title: "Studio work".into(), client: Some("North Studio".into()), budget: Some("work".into()), ..Project::default() },
        ];
        let budget = vec![BudgetRef { uri: links::budget_uri("home", 0), title: "Rent".into(), budget: "Home".into(), date: "2026-11-01".into(), links: vec!["sioul:letter/caf-1".into(), "sioul:project/flat".into()] }];
        let mut local = LocalLinks::default();
        local.add("sioul:paper/id-card", "sioul:person/leo", crate::persons::HOLDER);
        local.add("sioul:letter/caf-1", "sioul:person/leo", crate::persons::ADDRESSED);
        local.add("sioul:site/ameli", "sioul:person/leo", crate::persons::FOR);
        local.add("mid:studio@north.example", "sioul:task/answer-caf", "link");
        let persons = crate::persons::Persons {
            persons: vec![
                crate::persons::Person { id: "leo".into(), name: "Léo".into(), relation: "child".into(), ..Default::default() },
                crate::persons::Person { id: "sam-p".into(), name: "Sam".into(), relation: "other-parent".into(), cards: vec!["sam".into()], of: vec!["leo".into()], ..Default::default() },
            ],
            ..Default::default()
        };
        let letters = crate::letters::Letters {
            root: PathBuf::from("/stand-in/notes"),
            list: vec![crate::letters::Letter { id: "caf-1".into(), file: "letters/caf.pdf".into(), project: "flat".into(), task: "answer-caf".into(), status: "new".into(), reading: crate::letters::Reading { sender: "CAF".into(), ..Default::default() }, ..Default::default() }],
        };
        let sources = Sources {
            movements: vec![
                movement("bank:acct/1", "Mina Okafor", "2026-11-03", -85_000, serde_json::json!([source("bank", "bank:acct/1", "", false), source("mail", "mid:rent-11@landlord.example", "mid:rent-11@landlord.example", false)]), &[], "home"),
                movement("line:l1", "CAF", "2026-11-01", -12_000, serde_json::json!([source("line", "line:l1", &links::budget_uri("home", 0), false), source("letter", "sioul:letter/caf-1", "sioul:letter/caf-1", true)]), &["sioul:task/answer-caf"], "home"),
                movement("preset:energy/2026-11-05", "Énergie Exemple", "2026-11-05", -6_000, serde_json::json!([source("preset", "preset:energy/2026-11-05", "", false), source("contract", "sioul:contract/energy", "sioul:contract/energy", false)]), &[], "home"),
            ],
            budgets: vec![("home".into(), "Home".into()), ("work".into(), "Work".into())],
            persons,
            papers: vec![crate::papers::Paper { id: "id-card".into(), title: "Identity card".into(), renewal: "renew-card".into(), ..Default::default() }],
            letters: Some(letters),
            contracts: vec![crate::contracts::Contract { id: "energy".into(), title: "Electricity".into(), party: "Énergie Exemple".into(), paper: "energy-contract".into(), ..Default::default() }],
            invoices: vec![crate::invoice::Invoice { number: "2026-002".into(), date: "2026-10-30".into(), project: "studio".into(), client: "North Studio".into(), total_cents: 120_000, ..Default::default() }],
            sessions: vec![crate::timelog::Session { task: "pay-rent".into(), minutes: 10, ..Default::default() }, crate::timelog::Session { task: "pay-rent".into(), minutes: 5, ..Default::default() }],
            agendas: vec![("school-trip".into(), "leo".into())],
            taken: vec![("school-trip".into(), "sam@example.org".into())],
            children: BTreeSet::new(),
        };
        Fixture { tasks, events, contacts, vault, drafts, mail, projects, budget, local, sources }
    }

    pub(crate) fn world(&self) -> World<'_> {
        World { tasks: &self.tasks, events: &self.events, contacts: &self.contacts, vault: Some(&self.vault), drafts: &self.drafts, mail: &self.mail, projects: &self.projects, budget: &self.budget, local: &self.local, sites: &[], letters: Default::default(), uids: Default::default(), ties: None }
    }
}

fn mail_ref(subject: &str, address: &str, date: i64, refs: &[&str]) -> MailRef {
    mail(subject, address, date, refs)
}

/// What `a` is tied to, as (address, words), sorted.
fn tied(index: &TieIndex, world: &World, rule: &dyn Rule, a: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = index.related(world, a, rule).into_iter().map(|r| (r.uri, r.how)).collect();
    out.sort();
    out
}

/// Whether `a` sees `b` with the words `ab`, and `b` sees `a` with `ba`.
fn both(index: &TieIndex, world: &World, a: &str, ab: &str, b: &str, ba: &str) {
    let from_a = tied(index, world, &Open, a);
    assert!(from_a.contains(&(b.to_string(), ab.to_string())), "{a} → {b} ({ab}): {from_a:?}");
    let from_b = tied(index, world, &Open, b);
    assert!(from_b.contains(&(a.to_string(), ba.to_string())), "{b} → {a} ({ba}): {from_b:?}");
}

#[test]
fn every_source_reads_from_both_ends() {
    let fixture = Fixture::new();
    let base = fixture.world();
    let index = TieIndex::build(&base, &fixture.sources);
    let world = World { ties: Some(Ties { index: &index, rule: &Open }), ..fixture.world() };
    let rent = "mid:rent-11@landlord.example";
    let paid = "sioul:movement/bank:acct%2F1";
    // Tasks: made from a message, a step, a wait, a contact, a project, an appointment, a note.
    both(&index, &world, "sioul:task/pay-rent", "source", rent, "made");
    both(&index, &world, "sioul:task/find-iban", "part-of", "sioul:task/pay-rent", "step");
    both(&index, &world, "sioul:task/send-proof", "waits-for", "sioul:task/find-iban", "unblocks");
    both(&index, &world, "sioul:task/pay-rent", "contact", "sioul:contact/mina", "involves");
    both(&index, &world, "sioul:task/pay-rent", "project", "sioul:project/flat", "in-project");
    both(&index, &world, "sioul:task/prepare-dentist", "appointment", "sioul:event/dentist", "prepared-by");
    both(&index, &world, "sioul:task/prepare-dentist", "note", "sioul:note/dentist.md", "notes-of");
    // Events: a guest's card, a stand-in, who takes a child's event, a person's agenda, a letter by its scan.
    both(&index, &world, "sioul:event/dentist", "contact", "sioul:contact/dentist-desk", "involves");
    both(&index, &world, "sioul:event/trip-stand-in", "stands-for", "sioul:event/school-trip", "stand-in");
    both(&index, &world, "sioul:event/school-trip", "taken-by", "sioul:person/sam-p", "takes");
    both(&index, &world, "sioul:event/school-trip", "agenda-of", "sioul:person/leo", "agenda");
    both(&index, &world, "sioul:event/caf-visit", "link", "sioul:letter/caf-1", "link");
    // Notes: what one mentions; one in a project's files.
    both(&index, &world, "sioul:note/dentist.md", "mentions", "sioul:task/pay-rent", "mentioned-by");
    both(&index, &world, "sioul:note/flat/lease.md", "project", "sioul:project/flat", "in-project");
    // A draft answering a message; a budget line and what tells of it.
    both(&index, &world, "sioul:draft/reply-rent", "answers", rent, "answered-by");
    both(&index, &world, &links::budget_uri("home", 0), "source", "sioul:letter/caf-1", "made");
    both(&index, &world, &links::budget_uri("home", 0), "project", "sioul:project/flat", "in-project");
    // Kept here: by hand; a paper's holder, a letter's addressee, a site's person.
    both(&index, &world, "mid:studio@north.example", "link", "sioul:task/answer-caf", "link");
    both(&index, &world, "sioul:paper/id-card", "for", "sioul:person/leo", "theirs");
    both(&index, &world, "sioul:letter/caf-1", "for", "sioul:person/leo", "theirs");
    both(&index, &world, "sioul:site/ameli", "opened-for", "sioul:person/leo", "site");
    // Mail: a reply, the rest of its conversation, its sender, a project's routes.
    both(&index, &world, "mid:rent-11-re@example.net", "answers", rent, "answered-by");
    both(&index, &world, "mid:rent-11-re2@landlord.example", "conversation", rent, "conversation");
    both(&index, &world, rent, "from", "sioul:contact/mina", "sent");
    both(&index, &world, rent, "project", "sioul:project/flat", "in-project");
    // Movements: told for sure, probably; its line; its contract; its budget; its ties.
    both(&index, &world, paid, "told-by", rent, "payment");
    both(&index, &world, "sioul:movement/line:l1", "probably-told-by", "sioul:letter/caf-1", "probable-payment");
    both(&index, &world, "sioul:movement/line:l1", "told-by", &links::budget_uri("home", 0), "payment");
    both(&index, &world, "sioul:movement/line:l1", "link", "sioul:task/answer-caf", "link");
    both(&index, &world, "sioul:movement/preset:energy%2F2026-11-05", "contract", "sioul:contract/energy", "payments");
    both(&index, &world, paid, "budget", "sioul:budget/home", "payments");
    // Papers, contracts, persons, letters, invoices, projects.
    both(&index, &world, "sioul:task/renew-card", "source", "sioul:paper/id-card", "made");
    both(&index, &world, "sioul:contract/energy", "paper", "sioul:paper/energy-contract", "contract");
    both(&index, &world, "sioul:contract/energy", "party", "sioul:contact/edf", "contract");
    both(&index, &world, "sioul:person/sam-p", "card", "sioul:contact/sam", "person");
    both(&index, &world, "sioul:person/sam-p", "parent-of", "sioul:person/leo", "other-parent");
    both(&index, &world, "sioul:letter/caf-1", "project", "sioul:project/flat", "in-project");
    both(&index, &world, "sioul:task/answer-caf", "source", "sioul:letter/caf-1", "made");
    both(&index, &world, "sioul:invoice/2026-002", "project", "sioul:project/studio", "in-project");
    both(&index, &world, "sioul:invoice/2026-002", "client", "sioul:contact/north", "client-of");
    both(&index, &world, "sioul:project/studio", "client", "sioul:contact/north", "client-of");
    both(&index, &world, "sioul:project/studio", "budget", "sioul:budget/work", "project");
    // Time noted folds into its task, never a thing of its own.
    assert_eq!(index.sessions(index.node("sioul:task/pay-rent").unwrap()), 2);
    // A budget line's address is its own canonical form (the "/" before its position kept).
    assert_eq!(fixture.world().canonical(&links::budget_uri("home", 0)), "sioul:budget/home/0");
    assert_eq!(fixture.world().canonical(&links::budget_uri("a/b", 3)), "sioul:budget/a%2Fb/3");
    assert_eq!(fixture.world().describe(&links::budget_uri("home", 0)).title, "Rent");
    // A movement goes by every key of its sources, and is named by its store.
    for key in ["sioul:movement/bank:acct/1", "sioul:movement/bank:acct%2F1", "bank:acct/1"] {
        assert_eq!(world.canonical(key), paid, "{key}");
    }
    let named = world.describe(paid);
    assert_eq!((named.kind, named.title.as_str(), named.found), (Kind::Movement, "Mina Okafor", true));
    assert_eq!(world.describe("sioul:budget/home").title, "Home");
    assert_eq!(world.describe("sioul:person/leo").title, "Léo");
    assert_eq!(world.describe("sioul:contract/energy").title, "Electricity");
    // `World::related` reads the index when it carries one: the Related panel gains every tie.
    assert!(world.related(rent).iter().any(|r| r.uri == paid && r.how == "payment"));
    assert!(fixture.world().related(rent).iter().all(|r| r.uri != paid), "without the index, as before");
}

/// The request's chain: a movement, the message that told of it, the task
/// made from that message, the task's project; each found from the other three.
#[test]
fn the_request_s_chain_holds_from_each_end() {
    let fixture = Fixture::new();
    let index = TieIndex::build(&fixture.world(), &fixture.sources);
    let world = World { ties: Some(Ties { index: &index, rule: &Open }), ..fixture.world() };
    let chain = ["sioul:movement/bank:acct%2F1", "mid:rent-11@landlord.example", "sioul:task/pay-rent", "sioul:project/flat"];
    for (i, from) in chain.iter().enumerate() {
        // Each next one is one tie away, both ways; the others a walk away.
        if let Some(next) = chain.get(i + 1) {
            assert!(world.related(from).iter().any(|r| r.uri == *next), "{from} → {next}");
            assert!(world.related(next).iter().any(|r| r.uri == *from), "{next} → {from}");
        }
    }
}

/// A rule whose Porch set one message aside, as forged.
struct Forged(&'static str);

impl Rule for Forged {
    fn admission(&self, uri: &str, _path: &Path) -> Option<Admission> {
        if uri == self.0 { Some(Admission::Refused(crate::porch::Lane::SetAside, crate::porch::Reason::Forged)) } else { Some(Admission::Admitted) }
    }
}

/// A forged message citing two conversations never joins them; its tie
/// made by hand still shows it; its sender's card never takes it.
#[test]
fn a_forged_reply_never_joins_two_conversations() {
    let mut fixture = Fixture::new();
    fixture.mail.insert("other@school.example", mail("School fees", "office@school.example", NOV_2, &[]));
    fixture.mail.insert("forged@landlord.example", mail("Re: Rent for November", "mina@landlord.example", NOV_2 + 60, &["rent-11@landlord.example", "other@school.example"]));
    fixture.local.add("mid:forged@landlord.example", "sioul:task/send-proof", "link");
    let index = TieIndex::build(&fixture.world(), &fixture.sources);
    let rule = Forged("mid:forged@landlord.example");
    let world = World { ties: Some(Ties { index: &index, rule: &rule }), ..fixture.world() };
    let rent: Vec<String> = world.related("mid:rent-11@landlord.example").into_iter().map(|r| r.uri).collect();
    assert!(!rent.contains(&"mid:forged@landlord.example".to_string()), "{rent:?}");
    let school: Vec<String> = world.related("mid:other@school.example").into_iter().map(|r| r.uri).collect();
    assert!(school.is_empty(), "{school:?}");
    let forged: Vec<(String, String)> = world.related("mid:forged@landlord.example").into_iter().map(|r| (r.uri, r.how)).collect();
    assert_eq!(forged, vec![("sioul:task/send-proof".to_string(), "link".to_string())], "by hand only");
    assert!(world.related("sioul:contact/mina").iter().all(|r| r.uri != "mid:forged@landlord.example"));
    // As it stands, with no Porch: the same message ties both, as the files say.
    let open = World { ties: Some(Ties { index: &index, rule: &Open }), ..fixture.world() };
    assert!(open.related("mid:other@school.example").iter().any(|r| r.uri == "mid:forged@landlord.example"));
}

/// The index lives in memory: building it, reading it and walking it write
/// nothing in the folders the stand-ins name; its code holds no writer.
#[test]
fn nothing_is_written_to_disk() {
    let dir = std::env::temp_dir().join(format!("sioul-ties-disk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("notes")).unwrap();
    let mut fixture = Fixture::new();
    fixture.vault.root = dir.join("notes");
    if let Some(letters) = fixture.sources.letters.as_mut() {
        letters.root = dir.join("notes");
    }
    let index = TieIndex::build(&fixture.world(), &fixture.sources);
    let world = World { ties: Some(Ties { index: &index, rule: &Open }), ..fixture.world() };
    let tr = crate::i18n::Translator::new("en");
    let _ = world.related("sioul:task/pay-rent");
    let _ = crate::chain::walk(&world, &index, &Open, "sioul:project/flat", &["sioul:project/flat".into()], &crate::chain::Weights::computer(), &tr, NOV_2);
    let left: Vec<_> = walkdir(&dir);
    assert_eq!(left, vec![dir.join("notes")], "nothing written");
    std::fs::remove_dir_all(&dir).unwrap();
    for code in [include_str!("ties.rs"), include_str!("chain.rs")] {
        for writer in ["fs::write", "File::create", "OpenOptions", "create_dir", "filelock::", ".save(", "rename("] {
            assert!(!code.contains(writer), "{writer}");
        }
    }
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        out.push(entry.path());
        if entry.path().is_dir() {
            out.extend(walkdir(&entry.path()));
        }
    }
    out.sort();
    out
}

/// Every kind of tie has its words in each language's own file.
#[test]
fn every_tie_has_its_words() {
    for (language, text) in crate::i18n::LANGUAGES {
        for how in How::ALL {
            for id in [how.forward(), how.back()] {
                assert!(text.contains(&format!("\nhow-{id} = ")), "{language}: how-{id}");
            }
        }
    }
}
