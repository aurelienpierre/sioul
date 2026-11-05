// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The chain of ties on stand-ins (design-link-graph, phase 2).

use super::*;
use crate::i18n::Translator;
use crate::links::{self, Loaded};
use crate::mailindex::MailRef;
use crate::porch::Admission;
use crate::projects::{Project, Route};
use crate::tasks::{Link, Task};
use crate::ties::tests::{Fixture, NOV_2};
use crate::ties::{Open, Ties};
use std::path::{Path, PathBuf};

fn view(fixture: &Fixture, rule: &dyn Rule, entry: &str, opened: &[&str]) -> ChainView {
    let index = TieIndex::build(&fixture.world(), &fixture.sources);
    let world = World { ties: Some(Ties { index: &index, rule }), ..fixture.world() };
    let opened: Vec<String> = opened.iter().map(|o| o.to_string()).collect();
    walk(&world, &index, rule, entry, &opened, &Weights::computer(), &Translator::new("en"), NOV_2)
}

fn find<'a>(view: &'a ChainView, uri: &str) -> Option<&'a ChainNode> {
    view.things.iter().find(|t| t.uri == uri)
}

fn uris(view: &ChainView) -> Vec<&str> {
    view.things.iter().map(|t| t.uri.as_str()).collect()
}

const PAID: &str = "sioul:movement/bank:acct%2F1";
const RENT: &str = "mid:rent-11@landlord.example";
const TASK: &str = "sioul:task/pay-rent";
const FLAT: &str = "sioul:project/flat";

/// The request's example: from the payment, the message that told of it, the
/// task made from that message, the task's project; and from each of the
/// other three ends, the rest of the chain.
#[test]
fn the_request_s_example() {
    let fixture = Fixture::new();
    let from_payment = view(&fixture, &Open, PAID, &[]);
    let column = |v: &ChainView, uri: &str| find(v, uri).map(|t| t.column);
    assert_eq!([column(&from_payment, PAID), column(&from_payment, RENT), column(&from_payment, TASK), column(&from_payment, FLAT)], [Some(0), Some(1), Some(2), Some(3)], "{:?}", uris(&from_payment));
    let task = find(&from_payment, TASK).unwrap();
    assert_eq!(task.chain, "Told by: Message, Rent for November · Made into: Task, Pay the rent");
    assert_eq!(task.name, "Task: Pay the rent; Made from: Rent for November");
    assert_eq!((task.how, task.how_words.as_str()), ("made", "Made into"));
    let entry = &from_payment.things[0];
    assert_eq!((entry.kind, entry.family, entry.kind_words.as_str(), entry.title.as_str(), entry.detail.as_str()), (Kind::Movement, "money", "Payment", "Mina Okafor", "−€850"));
    assert_eq!(entry.date, "Tuesday 3 November");
    // The budget it counts in: a collection, shown with what it holds, never walked through.
    let home = find(&from_payment, "sioul:budget/home").unwrap();
    assert!(home.collection && home.count == 3 && home.more == 2, "{home:?}");
    assert_eq!(home.more_words, "2 more: 2 payments");
    for end in [RENT, TASK, FLAT] {
        let v = view(&fixture, &Open, end, &[]);
        for other in [PAID, RENT, TASK, FLAT] {
            assert!(find(&v, other).is_some(), "from {end}: {other} in {:?}", uris(&v));
        }
    }
    // The time noted on the task, folded into it.
    assert_eq!(task.sessions, 2);
}

fn statement(n: usize) -> MailRef {
    MailRef { path: PathBuf::from(format!("/stand-in/mail/cur/statement-{n}")), subject: format!("Bank statement {n}"), from: "bank@bank.example".into(), address: "bank@bank.example".into(), date: NOV_2 - n as i64 * 86_400, refs: Vec::new() }
}

/// A project of 500 messages is reached, then stopped and counted: none of
/// its messages comes through it, and "+" opens it eight at a time.
#[test]
fn a_project_of_500_messages_stops_and_counts() {
    let mut fixture = Fixture::new();
    fixture.projects.push(Project { id: "bank".into(), title: "Bank".into(), routes: vec![Route { subject_contains: vec!["statement".into()], ..Route::default() }], ..Project::default() });
    fixture.tasks.push(Task { uid: "check-bank".into(), title: "Check the statements".into(), projects: vec!["bank".into()], ..Task::default() });
    for n in 0..500 {
        fixture.mail.insert(&format!("statement-{n}@bank.example"), statement(n));
    }
    let v = view(&fixture, &Open, "sioul:task/check-bank", &[]);
    let bank = find(&v, "sioul:project/bank").unwrap();
    assert!(bank.collection && bank.column == 1, "{bank:?}");
    assert_eq!((bank.count, bank.more), (501, 500));
    assert_eq!(bank.more_words, "500 more: 500 messages");
    assert!(v.things.iter().all(|t| t.kind != Kind::Mail), "{:?}", uris(&v));
    // "+": the eight nearest in time to the entry point's date (today, a task without one).
    let opened = view(&fixture, &Open, "sioul:task/check-bank", &["sioul:project/bank"]);
    let shown: Vec<&str> = opened.things.iter().filter(|t| t.kind == Kind::Mail).map(|t| t.title.as_str()).collect();
    assert_eq!(shown, (0..8).map(|n| format!("Bank statement {n}")).collect::<Vec<_>>());
    assert_eq!(find(&opened, "sioul:project/bank").unwrap().more, 492);
    // From one of them: the project, counted, and no other statement.
    let one = view(&fixture, &Open, "mid:statement-7@bank.example", &[]);
    assert!(find(&one, "sioul:project/bank").is_some_and(|p| p.count == 501));
    assert_eq!(one.things.iter().filter(|t| t.kind == Kind::Mail).count(), 1, "{:?}", uris(&one));
}

/// A correspondent of 300 messages: a place to stop. Opened as the entry
/// point, it is walked: its messages nearest to today first, five.
#[test]
fn a_contact_hub() {
    let mut fixture = Fixture::new();
    for n in 0..300 {
        fixture.mail.insert(&format!("note-{n}@landlord.example"), MailRef { path: PathBuf::from(format!("/stand-in/mail/cur/note-{n}")), subject: format!("Building note {n}"), from: "Mina".into(), address: "mina@landlord.example".into(), date: NOV_2 - (n as i64 + 1) * 3600, refs: Vec::new() });
    }
    let from_one = view(&fixture, &Open, "mid:note-0@landlord.example", &[]);
    let mina = find(&from_one, "sioul:contact/mina").unwrap();
    assert!(mina.collection && mina.count == 303 && mina.more >= 300, "{mina:?}");
    assert!(from_one.things.iter().filter(|t| t.title.starts_with("Building note")).count() == 1, "{:?}", uris(&from_one));
    let entry = view(&fixture, &Open, "sioul:contact/mina", &[]);
    let under: Vec<&str> = entry.things.iter().filter(|t| t.parent == Some(0)).map(|t| t.title.as_str()).collect();
    assert_eq!(under.len(), 5);
    assert_eq!(under[..3], ["Rent for November", "Building note 0", "Building note 1"], "nearest to today first: {under:?}");
    // Its task, whose own place under the card is taken, comes by the message it was made from.
    let task = find(&entry, TASK).unwrap();
    assert_eq!(entry.things[task.parent.unwrap()].uri, RENT);
    assert!(entry.things.len() <= Weights::computer().total);
}

/// The same files give the same picture, twice, index built anew or not.
#[test]
fn the_same_answer_twice() {
    let fixture = Fixture::new();
    for entry in [PAID, RENT, TASK, FLAT, "sioul:person/leo", "sioul:contact/mina"] {
        let (a, b) = (view(&fixture, &Open, entry, &[]), view(&fixture, &Open, entry, &[]));
        assert_eq!(a, b, "{entry}");
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
    }
}

/// "+" on one thing changes no other thing's place or score: each thing
/// keeps its column, the thing it hangs from, its score and its order; what
/// is above the one pressed keeps its row. "−" gives back the earlier picture.
#[test]
fn plus_moves_nothing_and_minus_gives_it_back() {
    let fixture = Fixture::new();
    let before = view(&fixture, &Open, FLAT, &[]);
    let pressed = before.things.iter().find(|t| t.more > 0 && t.parent.is_some()).expect("something to open").uri.clone();
    let after = view(&fixture, &Open, FLAT, &[&pressed]);
    assert!(after.things.len() > before.things.len(), "{:?}", uris(&after));
    let parent_of = |v: &ChainView, t: &ChainNode| t.parent.map(|p| v.things[p].uri.clone());
    let at = before.things.iter().position(|t| t.uri == pressed).unwrap();
    let mut last = 0;
    for (i, thing) in before.things.iter().enumerate() {
        let (j, now) = after.things.iter().enumerate().find(|(_, t)| t.uri == thing.uri).unwrap();
        assert_eq!((now.column, now.score, parent_of(&after, now)), (thing.column, thing.score, parent_of(&before, thing)), "{}", thing.uri);
        assert!(j >= last, "order kept: {}", thing.uri);
        last = j;
        if i <= at {
            assert_eq!(now.row, thing.row, "above it: {}", thing.uri);
        }
    }
    let opened = find(&after, &pressed).unwrap();
    assert_eq!(opened.opened, 1);
    assert!(after.things.iter().filter(|t| parent_of(&after, t).as_deref() == Some(pressed.as_str())).count() > before.things.iter().filter(|t| parent_of(&before, t).as_deref() == Some(pressed.as_str())).count());
    // "−": the same picture as before; and with two pressed, folding one gives back the other's.
    assert_eq!(view(&fixture, &Open, FLAT, &[]), before);
    let second = after.things.iter().find(|t| t.more > 0 && t.uri != pressed && t.parent.is_some()).map(|t| t.uri.clone());
    if let Some(second) = second {
        let both = view(&fixture, &Open, FLAT, &[&pressed, &second]);
        assert!(both.things.len() >= after.things.len());
        assert_eq!(view(&fixture, &Open, FLAT, &[&pressed]), after);
    }
}

/// Equal scores: the nearest to the entry point's date first, then by
/// kind, by title, by address.
#[test]
fn ties_broken_by_date_kind_title_and_address() {
    let mut fixture = Fixture::new();
    let tie = |uri: &str| Link { uri: uri.into(), label: String::new(), rel: "related".into() };
    let task = |uid: &str, title: &str| Task { uid: uid.into(), title: title.into(), ..Task::default() };
    fixture.tasks.push(Task { links: vec![tie("uid:far"), tie("sioul:note/flat/lease.md"), tie("uid:z-alpha"), tie("uid:beta"), tie("uid:a-alpha"), tie("uid:near")], ..task("hub", "Sort these") });
    fixture.tasks.extend([task("z-alpha", "Alpha"), task("a-alpha", "Alpha"), task("beta", "Beta")]);
    fixture.events.push(crate::agenda::EventRef { uid: "far".into(), summary: "Far".into(), start: NOV_2 + 10 * 86_400, ..Default::default() });
    fixture.events.push(crate::agenda::EventRef { uid: "near".into(), summary: "Near".into(), start: NOV_2 + 86_400, ..Default::default() });
    let v = view(&fixture, &Open, "sioul:task/hub", &[]);
    let under: Vec<&str> = v.things.iter().filter(|t| t.parent == Some(0)).map(|t| t.uri.as_str()).collect();
    assert_eq!(under, ["sioul:event/near", "sioul:event/far", "sioul:task/a-alpha", "sioul:task/z-alpha", "sioul:task/beta"]);
    assert_eq!(v.things[0].more_words, "1 more: 1 note");
    let more = view(&fixture, &Open, "sioul:task/hub", &["sioul:task/hub"]);
    assert!(find(&more, "sioul:note/flat/lease.md").is_some_and(|n| n.parent == Some(0)));
}

/// What the window's rule says (decision 20): health shown, the page told;
/// an availability-only event "Busy", never walked through; mail the Porch
/// set aside shown by its tie made by hand, in the Porch's words, never walked through.
struct Window;

impl Rule for Window {
    fn admission(&self, uri: &str, _path: &Path) -> Option<Admission> {
        if uri == "mid:studio@north.example" { Some(Admission::Refused(crate::porch::Lane::SetAside, crate::porch::Reason::Forged)) } else { Some(Admission::Admitted) }
    }
    fn busy(&self, uri: &str) -> bool {
        uri == "sioul:event/school-trip"
    }
    fn health(&self, uri: &str) -> bool {
        uri == "sioul:task/prepare-dentist"
    }
}

#[test]
fn health_busy_and_set_aside_as_the_window_says() {
    let fixture = Fixture::new();
    let v = view(&fixture, &Window, TASK, &[]);
    assert!(find(&v, "sioul:task/prepare-dentist").is_some_and(|t| t.health) && v.health, "{:?}", uris(&v));
    let leo = view(&fixture, &Window, "sioul:person/leo", &[]);
    let trip = find(&leo, "sioul:event/school-trip").unwrap();
    assert!(trip.busy && trip.title == "Busy" && trip.more == 0, "{trip:?}");
    assert!(find(&leo, "sioul:event/trip-stand-in").is_none(), "not walked through: {:?}", uris(&leo));
    let caf = view(&fixture, &Window, "sioul:task/answer-caf", &["mid:studio@north.example"]);
    let studio = find(&caf, "mid:studio@north.example").unwrap();
    assert!(studio.set_aside && studio.more == 0 && studio.detail == "forged: set aside", "{studio:?}");
    assert!(find(&caf, "sioul:contact/north").is_none(), "{:?}", uris(&caf));
}

/// Things tied to nothing, or not here at all: the thing alone.
#[test]
fn a_thing_alone() {
    let fixture = Fixture::new();
    let v = view(&fixture, &Open, "sioul:task/nowhere", &[]);
    assert_eq!((v.things.len(), v.things[0].found, v.cross.len()), (1, false, 0));
    let loaded = Loaded::default();
    let index = TieIndex::build(&loaded.world(), &crate::ties::Sources::default());
    assert!(index.is_empty());
}

/// The tree: a thing beside the first that hangs from it, rows below for
/// the rest; ties between things shown that are not the tree's are cross-ties.
#[test]
fn rows_columns_and_cross_ties() {
    let fixture = Fixture::new();
    let v = view(&fixture, &Open, PAID, &[]);
    assert_eq!(v.things[0].row, v.things[1].row, "beside its first");
    let rows: std::collections::BTreeSet<usize> = v.things.iter().filter(|t| v.things.iter().all(|o| o.parent != Some(v.things.iter().position(|x| x.uri == t.uri).unwrap()))).map(|t| t.row).collect();
    assert_eq!(rows.len(), v.rows, "a row for each thing at the end of a chain");
    assert!(v.columns >= 4);
    for tie in &v.cross {
        let (a, b) = (&v.things[tie.from], &v.things[tie.to]);
        assert!(b.parent != Some(tie.from) && a.parent != Some(tie.to));
    }
    // The task is in the project the rent's routes take too: a cross-tie, not a second place.
    let task = v.things.iter().position(|t| t.uri == TASK).unwrap();
    let flat = v.things.iter().position(|t| t.uri == FLAT).unwrap();
    let rent = v.things.iter().position(|t| t.uri == RENT).unwrap();
    assert_eq!(v.things[flat].parent, Some(task));
    assert!(v.cross.iter().any(|c| (c.from, c.to) == (rent, flat) && c.words == "Project"), "{:?}", v.cross);
}

/// Every kind and every sentence of the chain in each language's own file.
#[test]
fn the_chain_s_words() {
    for (language, text) in crate::i18n::LANGUAGES {
        for kind in KINDS {
            for id in [format!("chain-kind-{}", kind_id(kind)), format!("chain-count-{}", kind_id(kind))] {
                assert!(text.contains(&format!("\n{id} = ")), "{language}: {id}");
            }
        }
        for id in ["chain-more", "chain-name", "chain-tied", "chain-step", "chain-and", "chain-holds", "chain-sessions", "chain-cross"] {
            assert!(text.contains(&format!("\n{id} = ")), "{language}: {id}");
        }
    }
    let fr = Translator::new("fr");
    assert_eq!(more_sentence(&fr, 3, &[(Kind::Mail, 2), (Kind::Task, 1)]), "3 de plus\u{202f}: 2 messages, 1 tâche");
    let _ = links::kind_of;
}
