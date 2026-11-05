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
            let family = |p: &Paper| ["identity", "health", "home", "money", "legal", "warranty", "other"].iter().position(|f| *f == p.kind.family()).unwrap_or(7);
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
    // Their agendas; a child's other parent's, the children's agendas they share (docs/persons.md).
    let agendas: Vec<serde_json::Value> = if me {
        Vec::new()
    } else {
        sioul_core::agendas::all(&config)
            .into_iter()
            .filter(|a| a.person_id == person.id || (a.whose == sioul_core::agendas::Whose::Child && a.parent_person == person.id))
            .map(|a| serde_json::json!({ "id": a.id, "name": a.name, "child": a.whose == sioul_core::agendas::Whose::Child, "whose": if a.person_id == person.id { String::new() } else { a.person.clone() } }))
            .collect()
    };
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
        "agendas": agendas,
        "letters": letters_for(&store, &config, &local, &own, &person.id),
        // Money: the budgets for them (`sioul:person/<id>` among a budget's links).
        "budgets": budgets_for(&config, &person.id),
        // The sites you open for them (`sioul:site/<id>` → `sioul:person/<id>`, how `for`).
        "sites": sites_for(&config, &local, &person.id),
        // The hand-over line due on a child's page, once, in an admin window (16, 18): "" none.
        "handover": handover_line(&store, &person, today),
        "handover_age": if handover_line(&store, &person, today).is_empty() { 0 } else { store.handover(&person.id, today).unwrap_or(0) },
        // "Ends…" for anyone but you and a child; a partner's papers handed back.
        "can_end": !me && person.relation() != Relation::Child && person.until.is_none(),
        "partner": person.relation() == Relation::Partner,
        "ended": person.until.map(|d| d.to_string()).unwrap_or_default(),
        // "Same as…": the others of the same side, to join into this one.
        "same_as": store.persons.iter().filter(|p| p.id != person.id && p.relation().holds_papers() == person.relation().holds_papers()).map(|p| serde_json::json!({ "id": p.id, "name": p.name })).collect::<Vec<_>>(),
    }))
}

/// The letters for a person (`Persons::letter_for`): their sender, kind and day, by id.
fn letters_for(store: &Persons, config: &Config, local: &LocalLinks, own: &[String], id: &str) -> Vec<serde_json::Value> {
    let Some(letters) = config.notes_root_path().and_then(|root| sioul_core::letters::Letters::load(&root).ok()) else { return Vec::new() };
    let today = sioul_core::zone::now().date();
    let mut theirs: Vec<&sioul_core::letters::Letter> = letters.list.iter().filter(|l| store.letter_for(&l.id, &l.reading.addressee, local, own).iter().any(|p| p == id)).collect();
    theirs.sort_by_key(|l| std::cmp::Reverse(l.received));
    theirs
        .into_iter()
        .map(|l| serde_json::json!({ "id": l.id, "sender": if l.reading.sender.is_empty() { tr().text("letter-unknown-sender", None) } else { l.reading.sender.clone() }, "kind_label": tr().text(&format!("letter-kind-{}", l.reading.kind.id()), None), "received": l.received.map(|d| tr().day_in(d, today)).unwrap_or_default() }))
        .collect()
}

/// The budgets for a person, by id and title (`budget::person_of`).
fn budgets_for(config: &Config, id: &str) -> Vec<serde_json::Value> {
    let Some(ledger) = config.notes_root_path().and_then(|root| sioul_core::budget::Ledger::load_file(&root.join(sioul_core::budget::LEDGER)).ok()) else { return Vec::new() };
    ledger.budgets.iter().filter(|b| sioul_core::budget::person_of(b).as_deref() == Some(id)).map(|b| serde_json::json!({ "id": b.id, "title": b.title })).collect()
}

/// The sites opened for a person (`persons::sites_for`), by id and name.
fn sites_for(config: &Config, local: &LocalLinks, id: &str) -> Vec<serde_json::Value> {
    let ids = persons::sites_for(local, id);
    sioul_core::sites::sites(config).into_iter().filter(|s| ids.contains(&s.id)).map(|s| serde_json::json!({ "id": s.id, "name": s.name, "url": s.url })).collect()
}

/// A site's person chosen ("Odile's ameli account, which you open for
/// Odile"), by id; "" nobody. Returns what went wrong, else "".
pub(crate) fn set_site_for(site: &str, person: &str) -> String {
    let store = store();
    let person = person.trim();
    if !person.is_empty() && store.get(person).is_none() {
        return tr().text("person-gone", None);
    }
    LocalLinks::change(&LocalLinks::default_path(), |local| persons::tie_site(local, site, person)).err().unwrap_or_default()
}

/// The person a site is opened for, as JSON {id, name}, or "null".
pub(crate) fn site_person(site: &str) -> String {
    let store = store();
    let local = LocalLinks::load(&LocalLinks::default_path());
    match persons::site_for(&local, site).and_then(|id| store.get(&id).cloned()) {
        Some(p) => json(&serde_json::json!({ "id": p.id, "name": p.name })),
        None => "null".into(),
    }
}

/// Whether now is an admin window: admin's hours open (docs/areas.md).
fn admin_now(config: &Config) -> bool {
    use sioul_core::areas::Time;
    let now = sioul_core::zone::now();
    let overrides = sioul_core::quiet::Overrides::load(&sioul_core::quiet::Overrides::default_path());
    let blocks = sioul_core::quiet::Blocks::read_now(&now);
    match sioul_core::quiet::mode(&config.week_hours(), &config.time_off, &overrides, &blocks, &now).time {
        Time::Admin | Time::Any => true,
        Time::Several(open) => open.admin,
        _ => false,
    }
}

/// The hand-over line due on a child's page (`Persons::handover`), said in
/// an admin window only; "" otherwise.
fn handover_line(store: &Persons, person: &Person, today: jiff::civil::Date) -> String {
    let Some(age) = store.handover(&person.id, today) else { return String::new() };
    if !admin_now(&load_config()) {
        return String::new();
    }
    say(if age >= 18 { "person-handover-18" } else { "person-handover-16" }, &[("name", person.name.clone())])
}

/// A hand-over line shown: said, on every device, never again (`Person::handed`).
pub(crate) fn handover_said(id: &str, age: u8) -> String {
    persons::change(&Persons::default_path(), |store| {
        if let Some(person) = store.get_mut(id) {
            person.handed = person.handed.max(age);
        }
    })
    .err()
    .unwrap_or_default()
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

/// The persons an agenda's "Whose name" and "The other parent" offer, by
/// the agenda's whose: a child's, a partner's, another parent's (docs/persons.md).
pub(crate) fn agenda_choices(store: &Persons) -> serde_json::Value {
    let today = sioul_core::zone::now().date();
    let list = |relation: Relation| store.persons.iter().filter(|p| p.relation() == relation && !p.ended(today)).map(|p| serde_json::json!({ "id": p.id, "name": p.name })).collect::<Vec<_>>();
    serde_json::json!({ "child": list(Relation::Child), "partner": list(Relation::Partner), "parent": list(Relation::OtherParent) })
}

/// An agenda's person chosen (`field`: "person", whose it is, or
/// "parent_person", a child's other parent), by id: the key written, and the
/// person's name beside it (`name`, `parent`), which older versions show; ""
/// takes both out. Returns what went wrong, else "".
pub(crate) fn set_agenda_person(place: &str, field: &str, id: &str) -> String {
    let name_field = match field {
        "person" => "name",
        "parent_person" => "parent",
        _ => return format!("{field}: not a person of an agenda"),
    };
    let store = store();
    let name = match id.trim() {
        "" => String::new(),
        id => match store.get(id) {
            Some(person) => person.name.clone(),
            None => return tr().text("person-gone", None),
        },
    };
    let path = crate::backend::config_path();
    sioul_core::agendas::set_field(&path, place, field, id.trim()).and_then(|()| sioul_core::agendas::set_field(&path, place, name_field, &name)).err().unwrap_or_default()
}

/// A person's new name written into the agendas that named them by the old
/// one or hold their key (`persons::renamed_agendas`), so that older versions show it.
fn rename_agendas(before: &Persons, id: &str, new: &str) {
    let config = load_config();
    let path = crate::backend::config_path();
    for (place, field, value) in persons::renamed_agendas(&config.agenda.calendars, before, id, new) {
        let _ = sioul_core::agendas::set_field(&path, &place, field, &value);
    }
}

/// The papers' holder texts that named a person by `old`, made `new`
/// (`persons::rename_holders`), those tied to them and those matched by text alike.
fn rename_texts(id: &str, old: &str, new: &str) {
    let config = load_config();
    let own = own_names(&config);
    let local = LocalLinks::load(&LocalLinks::default_path());
    // The store as it was before the new name, to see who the old texts named.
    let mut before = Persons::load(&Persons::default_path());
    if let Some(p) = before.get_mut(id) {
        p.name = old.to_string();
    }
    if let Some(root) = config.notes_root_path() {
        let _ = Wallet::change(&root, |wallet| persons::rename_holders(&mut wallet.papers, &before, &local, &own, id, new));
    }
    rename_agendas(&before, id, new);
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

/// A letter's persons chosen (its card's "For"), by id; none: its addressee decides again. Returns what went wrong, else "".
pub(crate) fn set_letter_for(letter: &str, chosen: &str) -> String {
    let chosen: Vec<String> = serde_json::from_str(chosen).unwrap_or_default();
    let store = store();
    let chosen: Vec<String> = chosen.into_iter().filter(|id| id == ME || store.get(id).is_some_and(|p| p.relation().holds_papers())).collect();
    LocalLinks::change(&LocalLinks::default_path(), |local| persons::tie_letter(local, letter, &chosen)).err().unwrap_or_default()
}

/// The persons a letter is for, by id, and in words ("For Léa"; "" yours alone or nobody read).
pub(crate) fn letter_persons(store: &Persons, local: &LocalLinks, own: &[String], letter: &sioul_core::letters::Letter) -> (Vec<String>, String) {
    let ids = store.letter_for(&letter.id, &letter.reading.addressee, local, own);
    let names: Vec<String> = ids.iter().filter(|id| *id != ME).map(|id| store.name_of(id, &you())).filter(|n| !n.is_empty()).collect();
    let said = if names.is_empty() { String::new() } else { say("letter-for", &[("names", names.join(", "))]) };
    (ids, said)
}

/// "Make a person of “…”" on a paper's card: a person of that name, in your household, nothing more said.
pub(crate) fn make_person_of(name: &str) -> String {
    let name = name.trim().to_string();
    if name.is_empty() {
        return tr().text("person-no-name", None);
    }
    let _ = store();
    persons::change(&Persons::default_path(), |store| {
        if store.holder_named(&name, &[]).is_none() {
            let id = store.put(Person { name: name.clone(), ..Person::default() });
            // Said to be no person before: a person now.
            store.not_persons.retain(|n| persons::key(n) != persons::key(&name));
            let _ = id;
        }
    })
    .err()
    .unwrap_or_default()
}

/// "This is not a person": taken out, their name among those that are no
/// person; their ties taken out and the agendas' keys naming them with them;
/// their papers keep their texts. Returns what went wrong, else "".
pub(crate) fn not_a_person(id: &str) -> String {
    if id == ME {
        return String::new();
    }
    if let Err(e) = persons::change(&Persons::default_path(), |store| store.not_a_person_now(id)) {
        return e;
    }
    let _ = LocalLinks::change(&LocalLinks::default_path(), |local| persons::retie(local, id, ""));
    move_agenda_keys(id, "");
    String::new()
}

/// The agendas' person keys naming `from` made `to` ("" takes them out).
fn move_agenda_keys(from: &str, to: &str) {
    let config = load_config();
    let path = crate::backend::config_path();
    for (place, said) in &config.agenda.calendars {
        for (field, value) in [("person", &said.person), ("parent_person", &said.parent_person)] {
            if value.as_deref() == Some(from) {
                let _ = sioul_core::agendas::set_field(&path, place, field, to);
            }
        }
    }
}

/// "Same as…": `other` joined into `keep` (Léa and "Léa M."): what it held
/// and `keep` lacked, its ties, its agendas' keys, and the texts that named it
/// by its name, which now say `keep`'s. Returns what went wrong, else "".
pub(crate) fn same_as(keep: &str, other: &str) -> String {
    let before = store();
    let (Some(kept), Some(_)) = (before.get(keep).cloned(), before.get(other)) else { return tr().text("person-gone", None) };
    let config = load_config();
    let own = own_names(&config);
    let local = LocalLinks::load(&LocalLinks::default_path());
    // The texts first, read with the store as it was: the papers' holders and the agendas' names.
    if let Some(root) = config.notes_root_path() {
        let _ = Wallet::change(&root, |wallet| persons::rename_holders(&mut wallet.papers, &before, &local, &own, other, &kept.name));
    }
    let path = crate::backend::config_path();
    for (place, field, value) in persons::renamed_agendas(&config.agenda.calendars, &before, other, &kept.name) {
        let _ = sioul_core::agendas::set_field(&path, &place, field, &value);
    }
    move_agenda_keys(other, keep);
    let _ = LocalLinks::change(&LocalLinks::default_path(), |local| persons::retie(local, other, keep));
    persons::change(&Persons::default_path(), |store| store.join(keep, other)).err().unwrap_or_default()
}

/// What "Ends…" asks: the day the relation ended, and for a partner, a
/// folder to save their own papers in (to hand back) and whether to take
/// them out of the wallet then.
#[derive(Deserialize)]
struct Ending {
    #[serde(default)]
    until: String,
    #[serde(default)]
    folder: String,
    #[serde(default)]
    take_out: bool,
}

/// "Ends…": the relation's end said (`until`); a partner's own papers (for
/// them alone: those of two stay, the household's past) saved in a folder
/// to hand back, and taken out of the wallet when asked, their files with
/// them only once saved. Returns {"saved", "taken_out"} or {"error"}.
pub(crate) fn end(id: &str, ending: &str) -> String {
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let ending: Ending = match serde_json::from_str(ending) {
        Ok(e) => e,
        Err(e) => return fail(e.to_string()),
    };
    let until = match ending.until.trim() {
        "" => sioul_core::zone::now().date(),
        text => match text.parse::<jiff::civil::Date>() {
            Ok(d) => d,
            Err(_) => return fail(say("papers-bad-date", &[("date", text.to_string())])),
        },
    };
    let store = store();
    let Some(person) = store.get(id).cloned() else { return fail(tr().text("person-gone", None)) };
    let config = load_config();
    let own = own_names(&config);
    let local = LocalLinks::load(&LocalLinks::default_path());
    let (mut saved, mut taken_out) = (0usize, 0usize);
    if person.relation() == Relation::Partner && !ending.folder.trim().is_empty()
        && let Some(root) = config.notes_root_path()
        && let Ok(wallet) = Wallet::load(&root)
    {
        let theirs: Vec<Paper> = wallet.papers.iter().filter(|p| store.holders_of(p, &local, &own).persons == [id.to_string()]).cloned().collect();
        let folder = crate::backend::local_path(ending.folder.trim()).join(&person.name);
        if let Err(e) = std::fs::create_dir_all(&folder) {
            return fail(format!("{}: {e}", folder.display()));
        }
        let mut copied: Vec<String> = Vec::new();
        for paper in &theirs {
            if let Some(file) = wallet.file_path(paper).filter(|f| f.is_file()) {
                let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if let Err(e) = std::fs::copy(&file, folder.join(&name)) {
                    return fail(format!("{}: {e}", file.display()));
                }
                saved += 1;
            }
            copied.push(paper.id.clone());
        }
        if ending.take_out {
            let _ = Wallet::change(&root, |wallet| {
                for id in &copied {
                    if wallet.remove(id) {
                        taken_out += 1;
                    }
                }
            });
            let _ = LocalLinks::change(&LocalLinks::default_path(), |local| copied.iter().fold(false, |changed, paper| persons::tie_paper(local, paper, &[ME.to_string()]) || changed));
        }
    }
    if let Err(e) = persons::change(&Persons::default_path(), |store| {
        if let Some(p) = store.get_mut(id) {
            p.until = Some(until);
        }
    }) {
        return fail(e);
    }
    serde_json::json!({ "saved": saved, "taken_out": taken_out }).to_string()
}

/// "What Sioul keeps about…": the person's papers' files copied into
/// `<folder>/<name>/`, and the page of what Sioul keeps about them, as HTML,
/// with where its PDF goes (written by the window, `PdfWriter`): their
/// papers (kind, dates, where each stands), agendas and letters. Nothing is
/// sent. Returns {"html", "pdf", "files"} or {"error"}.
pub(crate) fn keeps(id: &str, folder: &str) -> String {
    let fail = |e: String| serde_json::json!({ "error": e }).to_string();
    let shown: serde_json::Value = serde_json::from_str(&page(id)).unwrap_or_default();
    if shown.is_null() {
        return fail(tr().text("person-gone", None));
    }
    let name = shown["name"].as_str().unwrap_or_default().to_string();
    let target = crate::backend::local_path(folder.trim()).join(sioul_core::papers::portable_name_of(&name));
    let papers_dir = target.join(sioul_core::papers::portable_name_of(&tr().text("person-keeps-papers", None)));
    if let Err(e) = std::fs::create_dir_all(&papers_dir) {
        return fail(format!("{}: {e}", papers_dir.display()));
    }
    let mut files = 0usize;
    for paper in shown["papers"].as_array().into_iter().flatten() {
        let file = std::path::PathBuf::from(paper["file"].as_str().unwrap_or_default());
        if file.is_file() {
            let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if let Err(e) = std::fs::copy(&file, papers_dir.join(&name)) {
                return fail(format!("{}: {e}", file.display()));
            }
            files += 1;
        }
    }
    let html = keeps_html(&shown);
    let pdf = target.join(format!("{}.pdf", sioul_core::papers::portable_name_of(&tr().text("person-keeps-file", None))));
    serde_json::json!({ "html": html, "pdf": pdf.display().to_string(), "files": files, "folder": target.display().to_string() }).to_string()
}

/// What Sioul keeps about someone, as the PDF says it (`keeps`): their name
/// and line, then their papers (title, kind, where each stands, its file's
/// name), agendas and letters, from their page (`page`); every word
/// escaped, never read as markup.
pub(crate) fn keeps_html(shown: &serde_json::Value) -> String {
    let name = shown["name"].as_str().unwrap_or_default().to_string();
    let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut html = format!("<h1>{}</h1>", esc(&name));
    if let Some(line) = shown["line"].as_str().filter(|l| !l.is_empty()) {
        html.push_str(&format!("<p>{}</p>", esc(line)));
    }
    let section = |title: &str, rows: Vec<String>| if rows.is_empty() { String::new() } else { format!("<h2>{}</h2><ul>{}</ul>", esc(title), rows.into_iter().map(|r| format!("<li>{}</li>", esc(&r))).collect::<String>()) };
    let rows = |key: &str, parts: &[&str]| -> Vec<String> { shown[key].as_array().into_iter().flatten().map(|item| parts.iter().filter_map(|p| item[*p].as_str()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" · ")).collect() };
    html.push_str(&section(&tr().text("person-papers", None), rows("papers", &["title", "kind_label", "line", "file_name"])));
    html.push_str(&section(&tr().text("person-agendas", None), rows("agendas", &["name"])));
    html.push_str(&section(&tr().text("person-letters", None), rows("letters", &["sender", "kind_label", "received"])));
    html.push_str(&format!("<p><small>{}</small></p>", esc(&say("person-keeps-made", &[("date", tr().day(sioul_core::zone::now().date()))]))));
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window's paths for persons, on this device's own files (stand-in
    /// folders, never the person's): a child and a parent saved from their
    /// form; a paper for the new parent, its tie written and its holder
    /// text the parent's name; the parent renamed, the paper's text and a
    /// child's agenda's words following; a card linked, found again and
    /// offered as theirs; a name taken refused.
    /// A test's own configuration and notes folder, on its thread (`backend::TEST_CONFIG`),
    /// and the persons it made taken out of the shared store when it ends, whatever happened.
    struct OwnSetup {
        root: std::path::PathBuf,
        made: std::cell::RefCell<Vec<String>>,
    }

    impl OwnSetup {
        fn new(name: &str) -> OwnSetup {
            sioul_core::config::stand_ins_only();
            let root = sioul_core::config::data_dir().join(format!("{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("notes")).unwrap();
            let config = root.join("config.toml");
            std::fs::write(&config, format!("case_store = {:?}\n", root.join("notes").display().to_string())).unwrap();
            crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = Some(config));
            OwnSetup { root, made: std::cell::RefCell::new(Vec::new()) }
        }

        fn notes(&self) -> std::path::PathBuf {
            self.root.join("notes")
        }

        fn made(&self, id: &str) -> String {
            self.made.borrow_mut().push(id.to_string());
            id.to_string()
        }
    }

    impl Drop for OwnSetup {
        fn drop(&mut self) {
            let made = self.made.borrow().clone();
            let _ = persons::change(&Persons::default_path(), |store| {
                for id in &made {
                    store.remove(id);
                }
            });
            let _ = LocalLinks::change(&LocalLinks::default_path(), |local| {
                let before = local.links.len();
                local.links.retain(|e| !made.iter().any(|id| e.to == persons::uri(id) || e.from == persons::uri(id)));
                local.links.len() != before
            });
            crate::backend::TEST_CONFIG.with(|own| *own.borrow_mut() = None);
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn a_person_saved_renamed_given_a_paper_and_a_card() {
        let own = OwnSetup::new("persons-window");
        let config = crate::backend::config_path();
        let notes = own.notes();
        let saved = |id: &str, edit: serde_json::Value| -> serde_json::Value { serde_json::from_str(&save(id, &edit.to_string())).unwrap() };
        let wren = saved("", serde_json::json!({ "name": "Wren", "relation": "child", "born": "2015-03-04", "lives_with": true }));
        let wren = own.made(wren["id"].as_str().unwrap_or_else(|| panic!("{wren}")));
        let ivo = own.made(saved("", serde_json::json!({ "name": "Ivo", "relation": "parent" }))["id"].as_str().unwrap());
        assert!(saved("", serde_json::json!({ "name": "WREN", "relation": "kin" }))["error"].is_string(), "a name taken within the household");
        let page_of = |id: &str| serde_json::from_str::<serde_json::Value>(&page(id)).unwrap();
        assert_eq!(page_of(&wren)["lives_with"], true);
        assert!(page_of(&wren)["line"].as_str().unwrap().contains(&tr().text("person-relation-child", None)), "{}", page_of(&wren));
        // A paper for Ivo, as the form's "For" says it: the tie, and the name older versions show.
        assert_eq!(crate::papers::save("", &serde_json::json!({ "kind": "identity", "title": "Identity card", "for": [ivo] }).to_string()), "");
        let wallet = Wallet::load(&notes).unwrap();
        let card = wallet.papers.iter().find(|p| p.title == "Identity card").unwrap().clone();
        assert_eq!(card.holder, "Ivo");
        assert_eq!(persons::tied_persons(&LocalLinks::load(&LocalLinks::default_path()), &card.id), [ivo.clone()]);
        assert_eq!(page_of(&ivo)["papers"].as_array().unwrap().len(), 1);
        // A child's agenda naming Wren by name: renamed, its words follow; the paper's text follows Ivo's new name.
        sioul_core::agendas::set_field(&config, "local/wren-test", "whose", "child").unwrap();
        sioul_core::agendas::set_field(&config, "local/wren-test", "name", "Wren").unwrap();
        assert!(saved(&wren, serde_json::json!({ "name": "Wren B.", "relation": "child", "born": "2015-03-04", "lives_with": true }))["id"].is_string());
        assert!(saved(&ivo, serde_json::json!({ "name": "Ivo B.", "relation": "parent" }))["id"].is_string());
        let said = sioul_core::config::Config::load(&config).unwrap().agenda.calendars["local/wren-test"].clone();
        assert_eq!(said.name.as_deref(), Some("Wren B."));
        assert_eq!(Wallet::load(&notes).unwrap().get(&card.id).unwrap().holder, "Ivo B.");
        // Chosen for an agenda: the key and the name beside it.
        assert_eq!(set_agenda_person("local/wren-test", "person", &wren), "");
        let said = sioul_core::config::Config::load(&config).unwrap().agenda.calendars["local/wren-test"].clone();
        assert_eq!((said.person.as_deref(), said.name.as_deref()), (Some(wren.as_str()), Some("Wren B.")));
        // A card linked: found again, on the page, offered as theirs.
        let book = sioul_core::vdir::create(sioul_core::vdir::Kind::Contacts, sioul_core::vdir::LOCAL, "Persons test", None, &[]).unwrap();
        struct Gone(std::path::PathBuf);
        impl Drop for Gone {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _book = Gone(book.dir.clone());
        std::fs::write(book.dir.join("wren.vcf"), "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:card-wren\r\nFN:Wren Blackwood\r\nEMAIL:wren@example.org\r\nEND:VCARD\r\n").unwrap();
        assert_eq!(link_card(&wren, "card-wren", true), "");
        assert_eq!(serde_json::from_str::<serde_json::Value>(&of_card("card-wren")).unwrap()["id"], wren.as_str());
        assert_eq!(page_of(&wren)["cards"][0]["uid"], "card-wren");
        let offered: serde_json::Value = serde_json::from_str(&card_choices("blackwood")).unwrap();
        assert_eq!((offered[0]["uid"].as_str(), offered[0]["of"].as_str()), (Some("card-wren"), Some("Wren B.")));
        assert_eq!(link_card(&wren, "card-wren", false), "");
        assert_eq!(of_card("card-wren"), "null");
    }

    /// Phase 2's paths, on a test's own folders: a letter for whom its block
    /// names, then chosen; what Sioul keeps about someone, their papers'
    /// files copied and the page's words; a partner's end, their own papers
    /// saved to hand back and taken out, one of two kept; "Same as…" moving a
    /// paper's tie and text; "This is not a person" keeping the text, which
    /// "Make a person of" makes a person again.
    #[test]
    fn letters_ends_and_persons_joined_or_gone() {
        let own = OwnSetup::new("persons-phase-2");
        let notes = own.notes();
        let saved = |edit: serde_json::Value| -> String { own.made(serde_json::from_str::<serde_json::Value>(&save("", &edit.to_string())).unwrap()["id"].as_str().unwrap()) };
        let robin = saved(serde_json::json!({ "name": "Robin", "full_name": "Robin Quillfeather", "relation": "partner" }));
        let nell = saved(serde_json::json!({ "name": "Nell", "relation": "child" }));
        let nell_b = saved(serde_json::json!({ "name": "Nell B.", "relation": "kin" }));
        // A letter its block names to Robin; then chosen for Nell.
        let mut letters = sioul_core::letters::Letters { root: notes.clone(), ..Default::default() };
        letters.put(sioul_core::letters::Letter { id: "l-robin".into(), reading: sioul_core::letters::Reading { sender: "Water board".into(), addressee: "QUILLFEATHER Robin".into(), ..Default::default() }, ..Default::default() });
        letters.save().unwrap();
        let page_of = |id: &str| serde_json::from_str::<serde_json::Value>(&page(id)).unwrap();
        assert_eq!(page_of(&robin)["letters"][0]["id"], "l-robin");
        assert_eq!(set_letter_for("l-robin", &serde_json::json!([nell]).to_string()), "");
        assert!(page_of(&robin)["letters"].as_array().unwrap().is_empty() && page_of(&nell)["letters"][0]["id"] == "l-robin");
        // A site opened for Robin, and a budget for Robin (phase 3): on Robin's page.
        let config = crate::backend::config_path();
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}\n[[site]]\nid = \"ameli-robin\"\nname = \"ameli\"\nurl = \"https://ameli.example\"\n")).unwrap();
        assert_eq!(set_site_for("ameli-robin", &robin), "");
        assert_eq!(serde_json::from_str::<serde_json::Value>(&site_person("ameli-robin")).unwrap()["id"], robin.as_str());
        assert_eq!(page_of(&robin)["sites"][0]["id"], "ameli-robin");
        std::fs::write(notes.join(sioul_core::budget::LEDGER), format!("[[budget]]\nid = \"robin-share\"\ntitle = \"Robin's share\"\nperiod = \"month\"\nlinks = [\"sioul:person/{robin}\"]\n")).unwrap();
        assert_eq!(page_of(&robin)["budgets"][0]["id"], "robin-share");
        assert_eq!(set_site_for("ameli-robin", ""), "");
        assert!(page_of(&robin)["sites"].as_array().unwrap().is_empty());
        // Robin's own papers, with files, and one of two.
        let scan = notes.join("scan.pdf");
        std::fs::write(&scan, b"%PDF stand-in").unwrap();
        for (title, holders) in [("Robin's passport", vec![robin.clone()]), ("Lease", vec![ME.to_string(), robin.clone()])] {
            assert_eq!(crate::papers::save("", &serde_json::json!({ "kind": "passport", "title": title, "file": scan.display().to_string(), "for": holders }).to_string()), "");
        }
        let out = own.root.join("out");
        let kept: serde_json::Value = serde_json::from_str(&keeps(&robin, &out.display().to_string())).unwrap();
        assert_eq!(kept["files"], 2, "{kept}");
        assert!(kept["html"].as_str().unwrap().contains("Robin&#39;s passport") || kept["html"].as_str().unwrap().contains("Robin's passport"), "{kept}");
        assert!(kept["pdf"].as_str().unwrap().ends_with(".pdf"));
        let ended: serde_json::Value = serde_json::from_str(&end(&robin, &serde_json::json!({ "until": "2026-11-01", "folder": out.display().to_string(), "take_out": true }).to_string())).unwrap();
        assert_eq!((ended["saved"].as_u64(), ended["taken_out"].as_u64()), (Some(1), Some(1)), "their own one; the lease of two stays: {ended}");
        let wallet = Wallet::load(&notes).unwrap();
        assert!(wallet.papers.iter().all(|p| p.title != "Robin's passport") && wallet.papers.iter().any(|p| p.title == "Lease"));
        assert_eq!(page_of(&robin)["ended"], "2026-11-01");
        // Same as…: Nell B. into Nell, its paper's tie and text with it.
        assert_eq!(crate::papers::save("", &serde_json::json!({ "kind": "identity", "title": "Nell's card", "for": [nell_b] }).to_string()), "");
        assert_eq!(same_as(&nell, &nell_b), "");
        let card = Wallet::load(&notes).unwrap().papers.into_iter().find(|p| p.title == "Nell's card").unwrap();
        assert_eq!((card.holder.as_str(), persons::tied_persons(&LocalLinks::load(&LocalLinks::default_path()), &card.id)), ("Nell", vec![nell.clone()]));
        assert_eq!(page_of(&nell_b), serde_json::Value::Null);
        // This is not a person, then made one again from the paper's card.
        let bank = saved(serde_json::json!({ "name": "Banque Pellucid" }));
        assert_eq!(crate::papers::save("", &serde_json::json!({ "kind": "bank-details", "title": "RIB", "for": [bank] }).to_string()), "");
        assert_eq!(not_a_person(&bank), "");
        let rib = Wallet::load(&notes).unwrap().papers.into_iter().find(|p| p.title == "RIB").unwrap();
        assert_eq!(rib.holder, "Banque Pellucid", "its text kept");
        assert!(store().not_a_person("banque pellucid"));
        assert_eq!(make_person_of("Banque Pellucid"), "");
        let again = store().holder_named("Banque Pellucid", &[]).unwrap();
        own.made(&again);
        assert!(!store().not_a_person("banque pellucid"));
    }
}
