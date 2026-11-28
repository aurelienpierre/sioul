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
            docs: crate::storage::Places::under(std::path::Path::new("/stand-in/notes")).letters,
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
            invoices: vec![crate::invoice::Invoice {
                number: "2026-002".into(),
                date: "2026-10-30".into(),
                project: "studio".into(),
                client: crate::invoice::Client { name: "North Studio".into(), ..Default::default() },
                totals: crate::invoice::Totals { due: 120_000, ..Default::default() },
                ..Default::default()
            }],
            sessions: vec![crate::timelog::Session { task: "pay-rent".into(), minutes: 10, ..Default::default() }, crate::timelog::Session { task: "pay-rent".into(), minutes: 5, ..Default::default() }],
            agendas: vec![("school-trip".into(), "leo".into())],
            taken: vec![("school-trip".into(), "sam@example.org".into())],
            children: BTreeSet::new(),
            // Two calls with Noa (a card of their own, so that the chains' other tests keep theirs):
            // the one that made the task, the one that closed it; one with a hidden number.
            calls: vec![
                call_source(MISSED_AT, false, Some("sioul:contact/noa"), "Missed call from Noa Linden"),
                call_source(MISSED_AT + 3_600_000, false, Some("sioul:contact/noa"), "Call from Noa Linden"),
                call_source(MISSED_AT + 60_000, false, None, "Missed call from a hidden number"),
                // A landline two cards share: tied to both.
                super::CallSource { cards: vec!["sioul:contact/noa".into(), "sioul:contact/home".into()], ..call_source(MISSED_AT + 120_000, false, Some("sioul:contact/noa"), "Call from Noa Linden") },
            ],
            call_tasks: vec![super::CallTask { uid: "sioul-call-noa-1".into(), person: "sioul:contact/noa".into(), calls: vec![crate::callback::Mark { at: MISSED_AT, out: false }], called: Some(crate::callback::Mark { at: MISSED_AT + 3_600_000, out: false }) }],
            clients: vec![crate::clients::ties::ClientTies { uri: "sioul:client/c-north".into(), name: "North Studio".into(), cards: vec!["north".into()], ties: vec!["sioul:invoice/2026-002".into()] }],
        };
        Fixture { tasks, events, contacts, vault, drafts, mail, projects, budget, local, sources }
    }

    pub(crate) fn world(&self) -> World<'_> {
        World { tasks: &self.tasks, events: &self.events, contacts: &self.contacts, vault: Some(&self.vault), places: None, drafts: &self.drafts, mail: &self.mail, projects: &self.projects, budget: &self.budget, local: &self.local, sites: &[], letters: Default::default(), uids: Default::default(), ties: None }
    }
}

/// When Mina's missed call began (ms).
const MISSED_AT: i64 = 1_793_000_000_000;

/// A call of the phone "phone-1", as the index reads it.
fn call_source(at: i64, out: bool, card: Option<&str>, title: &str) -> super::CallSource {
    super::CallSource { address: links::call_uri("phone-1", at, out), title: title.into(), detail: String::new(), when: at / 1000, mark: crate::callback::Mark { at, out }, cards: card.map(str::to_string).into_iter().collect() }
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
    // Calls: whom each was with; the task the call-back rules made of one, and the call that closed it.
    let missed = links::call_uri("phone-1", MISSED_AT, false);
    both(&index, &world, &missed, "contact", "sioul:contact/noa", "involves");
    both(&index, &world, "sioul:task/sioul-call-noa-1", "source", &missed, "made");
    both(&index, &world, "sioul:task/sioul-call-noa-1", "link", &links::call_uri("phone-1", MISSED_AT + 3_600_000, false), "link");
    let call = world.describe(&missed);
    assert_eq!((call.kind, call.title.as_str(), call.found, call.when), (Kind::Call, "Missed call from Noa Linden", true, Some(MISSED_AT / 1000)));
    assert!(tied(&index, &world, &Open, &links::call_uri("phone-1", MISSED_AT + 60_000, false)).is_empty(), "a hidden number: no card, no task");
    let shared = links::call_uri("phone-1", MISSED_AT + 120_000, false);
    both(&index, &world, &shared, "contact", "sioul:contact/home", "involves");
    both(&index, &world, &shared, "contact", "sioul:contact/noa", "involves");
    assert_eq!(links::call_parts(&missed), Some(("phone-1".to_string(), MISSED_AT, false)));
    assert_eq!(world.canonical(&missed), missed);
    // Clients (docs/clients.md): a card and what a client's files tie to it say "Client", named by its name.
    both(&index, &world, "sioul:contact/north", "client", "sioul:client/c-north", "client-of");
    both(&index, &world, "sioul:invoice/2026-002", "client", "sioul:client/c-north", "client-of");
    assert_eq!((world.describe("sioul:client/c-north").title.as_str(), world.describe("sioul:client/c-north").kind), ("North Studio", Kind::Client));
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
        letters.docs = crate::storage::Places::under(&dir.join("notes")).letters;
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

/// An event of another program, or of an extension's door, carrying whom it
/// is with by the card's UID (`CONTACT;ALTREP="urn:uuid:<UID>"`, RFC 5545
/// §3.8.4.2), read from its own text (`agenda::event_ref`): tied to the card
/// of that UID, both ends listing each other, a UUID's letters whatever their
/// case; a task's likewise. A UID no card has, and a contact without an
/// address, tie nothing.
#[test]
fn a_contact_named_by_its_uid_ties_an_event_to_its_card() {
    let mut fixture = Fixture::new();
    let event = |uid: &str, contact: &str| {
        let text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Another program//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261103T090000Z\r\nDTSTART:20261103T090000Z\r\nDTEND:20261103T100000Z\r\nSUMMARY:Haircut\r\n{contact}END:VEVENT\r\nEND:VCALENDAR\r\n");
        crate::agenda::event_ref(&text, &format!("/stand-in/cal/{uid}.ics"), &jiff::tz::TimeZone::UTC).unwrap()
    };
    fixture.contacts.push(Contact { uid: "f81d4fae-7dec-11d0-a765-00a0c91e6bf6".into(), name: "Marie Exemple".into(), ..Contact::default() });
    fixture.contacts.push(Contact { uid: "bookings.k1.k2@extensions.sioul".into(), name: "Jean Exemple".into(), ..Contact::default() });
    fixture.events.extend([
        event("haircut", "CONTACT;ALTREP=\"URN:UUID:F81D4FAE-7DEC-11D0-A765-00A0C91E6BF6\":Marie Exemple\r\n"),
        event("colour", &format!("{}\r\n", crate::extensions::door::contact_line("bookings.k1.k2@extensions.sioul", "Jean Exemple"))),
        event("stranger", "CONTACT;ALTREP=\"urn:uuid:9b2d0c4e-0000-4000-8000-000000000000\":Someone\r\n"),
        event("plain", "CONTACT:Marie Exemple\\, +33 1 23 45 67 89\r\n"),
    ]);
    fixture.tasks.push(Task { uid: "call-marie".into(), title: "Call Marie".into(), contacts: vec![ContactRef { name: "Marie Exemple".into(), uri: "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6".into() }], ..Task::default() });
    let index = TieIndex::build(&fixture.world(), &fixture.sources);
    let world = fixture.world();
    let marie = "sioul:contact/f81d4fae-7dec-11d0-a765-00a0c91e6bf6";
    both(&index, &world, "sioul:event/haircut", "contact", marie, "involves");
    both(&index, &world, "sioul:event/colour", "contact", "sioul:contact/bookings.k1.k2@extensions.sioul", "involves");
    both(&index, &world, "sioul:task/call-marie", "contact", marie, "involves");
    assert!(tied(&index, &world, &Open, "sioul:event/stranger").is_empty(), "a UID no card has");
    assert!(tied(&index, &world, &Open, "sioul:event/plain").is_empty(), "a contact without an address");
    // The same without the index (`World::edges` read directly).
    assert!(world.related("sioul:event/haircut").iter().any(|r| r.uri == marie && r.kind == links::Kind::Contact));
}

/// Undoing that tie takes the event's `CONTACT` line off (`Loaded::untie`),
/// every other line as it was.
#[test]
fn a_contact_named_by_its_uid_is_untied() {
    crate::config::stand_ins_only();
    let calendar = crate::vdir::create(crate::vdir::Kind::Calendars, crate::vdir::LOCAL, "Ties by UID", None, &["VEVENT"]).unwrap();
    let path = calendar.dir.join("haircut.ics");
    let text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Another program//EN\r\nBEGIN:VEVENT\r\nUID:haircut-untie\r\nDTSTAMP:20261103T090000Z\r\nDTSTART:20261103T090000Z\r\nDTEND:20261103T100000Z\r\nSUMMARY:Haircut\r\nCONTACT;ALTREP=\"urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6\":Marie Exemple\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    std::fs::write(&path, text).unwrap();
    let event = crate::agenda::event_ref(text, &path.display().to_string(), &jiff::tz::TimeZone::UTC).unwrap();
    let loaded = Loaded { events: vec![event], contacts: vec![Contact { uid: "f81d4fae-7dec-11d0-a765-00a0c91e6bf6".into(), name: "Marie Exemple".into(), ..Contact::default() }], ..Loaded::default() };
    let undone = loaded.untie("sioul:event/haircut-untie", "sioul:contact/f81d4fae-7dec-11d0-a765-00a0c91e6bf6", &jiff::Zoned::now());
    assert_eq!(undone.len(), 1);
    assert!(!undone[0].text.contains("CONTACT") && undone[0].text.contains("SUMMARY:Haircut"), "{}", undone[0].text);
}

/// A task's contact as tasks are written since 26 November 2026
/// (`CONTACT;ALTREP="urn:uuid:<UID>"`, `tasks::contact_line`) and as older
/// tasks hold it (`sioul:contact/<UID>`, never rewritten for that alone):
/// both tie the task to the card, both ends listing each other (the card's
/// sheet lists the task), both find the card's client for an invoice
/// (`clients::resolve`), and a card not here is named as an older task
/// names it. Each is untied by taking its line off; a card tied again in
/// the other form is not written twice.
#[test]
fn a_task_s_contact_ties_in_either_form() {
    let mut fixture = Fixture::new();
    let older = fixture.tasks.iter().find(|t| t.uid == "pay-rent").unwrap().contacts[0].clone();
    assert_eq!(older.uri, "sioul:contact/mina", "the fixture's task, written before");
    let now_form = ContactRef { name: "Mina Okafor".into(), uri: crate::tasks::contact_line(&older).split('"').nth(1).unwrap().to_string() };
    assert_eq!(now_form.uri, "urn:uuid:mina");
    fixture.tasks.push(Task { uid: "thank-mina".into(), title: "Thank Mina".into(), contacts: vec![now_form.clone()], ..Task::default() });
    fixture.tasks.push(Task { uid: "call-gone".into(), title: "Call someone".into(), contacts: vec![ContactRef { name: "Someone".into(), uri: "urn:uuid:gone-1".into() }], ..Task::default() });
    let index = TieIndex::build(&fixture.world(), &fixture.sources);
    let world = fixture.world();
    both(&index, &world, "sioul:task/pay-rent", "contact", "sioul:contact/mina", "involves");
    both(&index, &world, "sioul:task/thank-mina", "contact", "sioul:contact/mina", "involves");
    let sheet = tied(&index, &world, &Open, "sioul:contact/mina");
    assert!(sheet.iter().filter(|(uri, _)| uri.starts_with("sioul:task/")).count() == 2, "{sheet:?}");
    assert!(world.edges().iter().any(|e| e.from == "sioul:task/call-gone" && e.to == "sioul:contact/gone-1" && e.how == "contact"), "a card not here: named as an older task names it");
    for uri in [&older.uri, &now_form.uri] {
        let (client, _) = crate::clients::resolve(uri, &crate::clients::Clients::default(), &fixture.contacts);
        assert_eq!(client.name, "Mina Okafor", "{uri}");
    }

    let dir = std::env::temp_dir().join(format!("sioul-task-contact-forms-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let zone = jiff::tz::TimeZone::UTC;
    let file = |uid: &str, altrep: &str| {
        let text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Sioul//Sioul//EN\r\nBEGIN:VTODO\r\nUID:{uid}\r\nDTSTAMP:20261101T090000Z\r\nSUMMARY:Pay the rent\r\nCONTACT;ALTREP=\"{altrep}\":Mina Okafor\r\nEND:VTODO\r\nEND:VCALENDAR\r\n");
        let path = dir.join(format!("{uid}.ics"));
        std::fs::write(&path, &text).unwrap();
        Task { key: path.display().to_string(), ..crate::tasks::task_of_text(&text, &zone).unwrap() }
    };
    let loaded = Loaded { tasks: vec![file("old", "sioul:contact/mina"), file("new", "urn:uuid:mina"), file("call-gone", "urn:uuid:gone-1")], contacts: fixture.contacts.clone(), ..Loaded::default() };
    let now = jiff::Zoned::now();
    // A card not on this device: the tie shown is undone all the same (review of e72aa225, 1).
    assert!(loaded.world().edges().iter().any(|e| e.from == "sioul:task/call-gone" && e.to == "sioul:contact/gone-1"));
    let undone = loaded.untie("sioul:task/call-gone", "sioul:contact/gone-1", &now);
    assert_eq!(undone.len(), 1, "a card not here");
    assert!(!undone[0].text.contains("CONTACT") && undone[0].text.contains("SUMMARY:Pay the rent"), "{}", undone[0].text);
    for uid in ["old", "new"] {
        let undone = loaded.untie(&format!("sioul:task/{uid}"), "sioul:contact/mina", &now);
        assert_eq!(undone.len(), 1, "{uid}");
        assert!(!undone[0].text.contains("CONTACT") && undone[0].text.contains("SUMMARY:Pay the rent"), "{}", undone[0].text);
        // Tied again: there already, whichever form its line has.
        let again = loaded.tie("sioul:contact/mina", &format!("sioul:task/{uid}"), &now).unwrap();
        assert_eq!(again.text, std::fs::read_to_string(&again.path).unwrap(), "{uid}: nothing written");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
