// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Persons, for the window (docs/persons.md): the household at the head of
//! the Contacts page (« Proches »), a person's page, their form, their
//! cards, and the persons a paper is for.

use crate::backend::{json, load_config, say, tr};
use serde::{Deserialize, Serialize};
use sioul_core::config::Config;
use sioul_core::links::LocalLinks;
use sioul_core::papers::{Paper, Wallet};
use sioul_core::persons::{self, ME, Person, Persons, Relation};
use std::collections::BTreeMap;

/// Your accounts' display names: what a holder text names you by.
pub(crate) fn own_names(config: &Config) -> Vec<String> {
    config.every_account().filter_map(|a| a.name.clone()).filter(|n| !n.trim().is_empty()).collect()
}

/// The papers of the wallet, none when there is no notes folder or it does not read.
fn papers_of(config: &Config) -> Vec<Paper> {
    config.notes_root_path().and_then(|root| Wallet::load(&root).ok()).map(|w| w.papers).unwrap_or_default()
}

/// The store, its old texts read once on this device and written then
/// (`Persons::read_names`); a store that does not read is taken as read in
/// memory, never written over.
pub(crate) fn store() -> Persons {
    let config = load_config();
    let path = Persons::default_path();
    let persons = Persons::load(&path);
    if persons.names_read >= persons::NAMES_READ {
        return persons;
    }
    let papers = papers_of(&config);
    let own = own_names(&config);
    persons::change(&path, |p| {
        p.read_names(&config.agenda.calendars, &papers, &own);
        p.clone()
    })
    .unwrap_or_else(|_| Persons::as_read(&path, &config.agenda.calendars, &papers, &own))
}

/// "You" (« Vous »).
pub(crate) fn you() -> String {
    tr().text("persons-you", None)
}

/// Who someone is to you, in words: "Child", "Léa’s other parent".
fn relation_label(store: &Persons, person: &Person) -> String {
    match person.relation() {
        Relation::OtherParent => {
            let children: Vec<String> = person.of.iter().filter_map(|id| store.get(id)).map(|c| c.name.clone()).collect();
            if children.is_empty() { tr().text("person-relation-other-parent-alone", None) } else { say("person-relation-other-parent", &[("child", children.join(", "))]) }
        }
        Relation::Unsaid => tr().text("person-relation-unsaid", None),
        relation => tr().text(&format!("person-relation-{}", relation.id()), None),
    }
}

/// One line under a name: "Child · 12 · lives with you".
fn line_of(store: &Persons, person: &Person, today: jiff::civil::Date) -> String {
    let mut parts = vec![relation_label(store, person)];
    if let Some(age) = person.age(today) {
        parts.push(say("person-age", &[("n", age.to_string())]));
    }
    if person.lives_with == Some(true) {
        parts.push(tr().text("person-lives-with", None));
    }
    parts.join(" · ")
}

#[derive(Serialize)]
struct Chip {
    id: String,
    name: String,
    relation: String,
    line: String,
}

fn chip(store: &Persons, person: &Person, today: jiff::civil::Date) -> Chip {
    Chip { id: person.id.clone(), name: person.name.clone(), relation: person.relation().id().to_string(), line: line_of(store, person, today) }
}

fn you_chip() -> Chip {
    Chip { id: ME.into(), name: you(), relation: Relation::Me.id().into(), line: String::new() }
}

#[derive(Serialize)]
struct Choice {
    id: &'static str,
    label: String,
}

/// The household, as JSON: you first, then the others in their order
/// (`Persons::household`), children's other parents apart; the relations a
/// form offers.
pub(crate) fn household() -> String {
    let store = store();
    let today = sioul_core::zone::now().date();
    let (inside, others) = store.household(today);
    let mut persons = vec![you_chip()];
    persons.extend(inside.into_iter().map(|p| chip(&store, p, today)));
    let others: Vec<Chip> = others.into_iter().map(|p| chip(&store, p, today)).collect();
    let relations: Vec<Choice> = Relation::CHOSEN.iter().map(|r| Choice { id: r.id(), label: tr().text(&format!("person-relation-{}", match r { Relation::Unsaid => "unsaid", Relation::OtherParent => "other-parent-alone", other => other.id() }), None) }).collect();
    json(&serde_json::json!({ "persons": persons, "others": others, "relations": relations }))
}

/// The persons a paper may be for, as the form's chooser offers them: you,
/// then the household (never a child's other parent).
pub(crate) fn paper_choices(store: &Persons) -> Vec<serde_json::Value> {
    let today = sioul_core::zone::now().date();
    let (inside, _) = store.household(today);
    std::iter::once(serde_json::json!({ "id": ME, "name": you() })).chain(inside.into_iter().map(|p| serde_json::json!({ "id": p.id, "name": p.name }))).collect()
}

#[derive(Serialize)]
struct CardRow {
    uid: String,
    key: String,
    name: String,
    detail: String,
}

/// A person's page, as JSON: their name and line, their papers with where
/// each stands, their cards, and the switch for agents. "null" when gone.
pub(crate) fn page(id: &str) -> String {
    let store = store();
    let config = load_config();
    let today = sioul_core::zone::now().date();
    let me = id == ME;
    let person = if me { Person { id: ME.into(), name: you(), ..Person::default() } } else {
        match store.get(id) {
            Some(p) => p.clone(),
            None => return "null".into(),
        }
    };
    let own = own_names(&config);
    let local = LocalLinks::load(&LocalLinks::default_path());
    let papers: Vec<serde_json::Value> = match config.notes_root_path().map(|root| Wallet::load(&root)) {
        Some(Ok(wallet)) => {
            let mut theirs: Vec<&Paper> = wallet.papers.iter().filter(|p| store.holders_of(p, &local, &own).has(id)).collect();
            // In the Papers page's order: identity, health, home, money, warranties, others.
            let family = |p: &Paper| ["identity", "health", "home", "money", "warranty", "other"].iter().position(|f| *f == p.kind.family()).unwrap_or(6);
            theirs.sort_by(|a, b| (family(a), a.kind.id(), &a.title).cmp(&(family(b), b.kind.id(), &b.title)));
            theirs.into_iter().map(|p| crate::papers::view_json(&wallet, p, &store, &local, &own, today)).collect()
        }
        _ => Vec::new(),
    };
    let contacts = sioul_core::contacts::all();
    let cards: Vec<CardRow> = person
        .cards
        .iter()
        .filter_map(|uid| contacts.iter().find(|c| &c.uid == uid))
        .map(|c| CardRow { uid: c.uid.clone(), key: c.key.clone(), name: c.name.clone(), detail: c.emails.first().map(|e| e.value.clone()).or_else(|| c.phones.first().map(|p| p.value.clone())).unwrap_or_default() })
        .collect();
    let of: Vec<serde_json::Value> = person.of.iter().filter_map(|c| store.get(c)).map(|c| serde_json::json!({ "id": c.id, "name": c.name })).collect();
    json(&serde_json::json!({
        "id": person.id,
        "me": me,
        "name": person.name,
        "full_name": person.full_name,
        "relation": person.relation().id(),
        "line": if me { String::new() } else { line_of(&store, &person, today) },
        "born": person.born.map(|d| d.to_string()).unwrap_or_default(),
        "lives_with": person.lives_with == Some(true),
        "agents": me || person.agents,
        "holds_papers": person.relation().holds_papers(),
        "of": of,
        "papers": papers,
        "cards": cards,
    }))
}

/// What the person's form gives back.
#[derive(Deserialize)]
struct PersonEdit {
    #[serde(default)]
    name: String,
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    relation: String,
    #[serde(default)]
    born: String,
    #[serde(default)]
    lives_with: bool,
    /// A child's other parent: which children, by id.
    #[serde(default)]
    of: Vec<String>,
    /// A card to link at once (a contact's "This is a person in your household…").
    #[serde(default)]
    card: String,
}

/// A person saved from the form: new when `id` is empty. A new name
/// rewrites the papers' holder texts that named the old one, so that older
/// versions show the new one. Returns {"id"} or {"error"}.
pub(crate) fn save(id: &str, edit: &str) -> String {
    let answer = |r: Result<String, String>| match r {
        Ok(id) => serde_json::json!({ "id": id }).to_string(),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    };
    let edit: PersonEdit = match serde_json::from_str(edit) {
        Ok(edit) => edit,
        Err(e) => return answer(Err(e.to_string())),
    };
    let name = edit.name.trim().to_string();
    if name.is_empty() {
        return answer(Err(tr().text("person-no-name", None)));
    }
    let born = match edit.born.trim() {
        "" => None,
        text => match text.parse::<jiff::civil::Date>() {
            Ok(d) => Some(d),
            Err(_) => return answer(Err(say("papers-bad-date", &[("date", text.to_string())]))),
        },
    };
    let relation = Relation::parse(&edit.relation);
    // The store as old texts read it first, so that a new person never takes a name read from them.
    let _ = store();
    let path = Persons::default_path();
    let mut renamed: Option<(String, String)> = None;
    let saved = persons::change(&path, |store| {
        let side = relation.holds_papers();
        if store.persons.iter().any(|p| p.id != id && p.relation().holds_papers() == side && p.named(&name)) {
            return Err(say("person-name-taken", &[("name", name.clone())]));
        }
        let mut person = store.get(id).cloned().unwrap_or_default();
        if !person.id.is_empty() && persons::key(&person.name) != persons::key(&name) {
            renamed = Some((person.name.clone(), name.clone()));
        }
        person.name = name.clone();
        person.full_name = edit.full_name.trim().to_string();
        // A word this Sioul does not know stays as written, unless another is chosen.
        if Relation::parse(&person.relation) != relation || person.id.is_empty() {
            person.relation = relation.id().to_string();
        }
        person.born = born;
        person.lives_with = edit.lives_with.then_some(true).or(person.lives_with.map(|_| false));
        person.of = if relation == Relation::OtherParent { edit.of.iter().filter(|c| store.get(c).is_some_and(|p| p.relation() == Relation::Child)).cloned().collect() } else { Vec::new() };
        let id = store.put(person);
        if !edit.card.trim().is_empty() {
            store.link_card(&id, &edit.card);
        }
        Ok(id)
    });
    let saved = match saved {
        Ok(inner) => inner,
        Err(e) => Err(e),
    };
    if let (Ok(id), Some((old, new))) = (&saved, renamed) {
        rename_texts(id, &old, &new);
    }
    answer(saved)
}

/// The papers' holder texts that named a person by `old`, made `new`
/// (`persons::rename_holders`), those tied to them and those matched by text alike.
fn rename_texts(id: &str, old: &str, new: &str) {
    let config = load_config();
    let Some(root) = config.notes_root_path() else { return };
    let own = own_names(&config);
    let local = LocalLinks::load(&LocalLinks::default_path());
    // The store as it was before the new name, to see who the old texts named.
    let mut before = Persons::load(&Persons::default_path());
    if let Some(p) = before.get_mut(id) {
        p.name = old.to_string();
    }
    let _ = Wallet::change(&root, |wallet| persons::rename_holders(&mut wallet.papers, &before, &local, &own, id, new));
}

/// The switch "Open to AI agents" on a person's page; returns what went wrong, else "".
pub(crate) fn set_agents(id: &str, on: bool) -> String {
    persons::change(&Persons::default_path(), |store| {
        if let Some(person) = store.get_mut(id) {
            person.agents = on;
        }
    })
    .err()
    .unwrap_or_default()
}

/// A card linked to a person (one person's at a time), or taken off them; returns what went wrong, else "".
pub(crate) fn link_card(id: &str, uid: &str, linked: bool) -> String {
    let _ = store();
    persons::change(&Persons::default_path(), |store| if linked { store.link_card(id, uid) } else { store.unlink_card(id, uid) }).err().unwrap_or_default()
}

/// The person a contact card is, as JSON {id, name}, or "null".
pub(crate) fn of_card(uid: &str) -> String {
    let store = store();
    match store.of_card(uid) {
        Some(p) => json(&serde_json::json!({ "id": p.id, "name": p.name })),
        None => "null".into(),
    }
}

/// Cards to link to a person: those whose name, organisation or address
/// holds every word typed, case and accents aside; 30 at most.
pub(crate) fn card_choices(query: &str) -> String {
    let words: Vec<String> = persons::key(query).split_whitespace().map(str::to_string).collect();
    let store = store();
    let rows: Vec<serde_json::Value> = sioul_core::contacts::all()
        .into_iter()
        .filter(|c| {
            let text = persons::key(&format!("{} {} {}", c.name, c.org, c.emails.iter().map(|e| e.value.as_str()).collect::<Vec<_>>().join(" ")));
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .take(30)
        .map(|c| {
            let of = store.of_card(&c.uid).map(|p| p.name.clone()).unwrap_or_default();
            serde_json::json!({ "uid": c.uid, "name": c.name, "detail": c.emails.first().map(|e| e.value.clone()).unwrap_or_else(|| c.org.clone()), "of": of })
        })
        .collect();
    json(&rows)
}

/// Each paper's persons, as the Papers page and the form read them.
pub(crate) struct ForPapers {
    pub store: Persons,
    pub local: LocalLinks,
    pub own: Vec<String>,
}

impl ForPapers {
    pub(crate) fn read(config: &Config) -> ForPapers {
        ForPapers { store: store(), local: LocalLinks::load(&LocalLinks::default_path()), own: own_names(config) }
    }
}

/// The holder label of a paper's persons: "" yours alone, else their names
/// ("You, Léa"), then what names nobody as written.
pub(crate) fn label(store: &Persons, holders: &persons::Holders) -> String {
    if holders.only_me() {
        return String::new();
    }
    let you = you();
    let mut names: Vec<String> = holders.persons.iter().map(|id| store.name_of(id, &you)).filter(|n| !n.is_empty()).collect();
    names.extend(holders.texts.iter().cloned());
    names.join(", ")
}

/// A paper's ties to persons written (`links.toml`), and the holder text
/// older versions show: the chosen persons' names, then the texts kept.
/// Returns that text.
pub(crate) fn tie_paper(paper: &str, chosen: &[String], texts: &[String]) -> Result<String, String> {
    let store = store();
    let chosen: Vec<String> = chosen.iter().filter(|id| *id == ME || store.get(id).is_some_and(|p| p.relation().holds_papers())).cloned().collect();
    let config = load_config();
    // You as your first account's name when you have one, so that an older Sioul shows a name.
    let you = own_names(&config).into_iter().next().unwrap_or_else(you);
    let text = persons::holder_text(&store, &chosen, texts, &you);
    LocalLinks::change(&LocalLinks::default_path(), |local| persons::tie_paper(local, paper, &chosen))?;
    Ok(text)
}

/// The persons each paper is for, by id, for the Papers page's chips: the
/// persons with at least one paper, you first, in the household's order.
pub(crate) fn chips(store: &Persons, holders: &BTreeMap<String, persons::Holders>) -> Vec<serde_json::Value> {
    paper_choices(store).into_iter().filter(|c| c["id"].as_str().is_some_and(|id| holders.values().any(|h| h.has(id)))).collect()
}
