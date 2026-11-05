// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Persons (docs/persons.md): the people your admin is done for, a child, a
//! parent you help, a partner, a child's other parent, and you.
//!
//! A person is Sioul's own record, distinct from a contact card: a card is
//! for reaching someone, kept on a CardDAV server and read by every app of a
//! phone; a person is who papers, agendas and letters are *for*, kept sealed
//! on your own devices, linked to zero or more cards by their UIDs and never
//! written into them (docs/research.md, findings 55 to 60). You are always
//! one, `me`, with nothing to fill.
//!
//! **The store**, `$XDG_DATA_HOME/sioul/persons.toml`, beside `links.toml`
//! (format 1; every table keeps what a later version writes), each person a
//! table named by its id, so that each of its fields travels on its own (an
//! array's element would travel whole: two devices changing two fields of
//! one person at once would lose one):
//!
//! ```toml
//! format = 1
//!
//! [person.6f0a3c2e-…]
//! name = "Léa"
//! full_name = "Léa Martin"
//! relation = "child"
//! born = 2014-05-12
//! lives_with = true
//! agents = false
//! ```
//!
//! It travels in the settings part, each field of each person an entry, once
//! every other device says it has the store (`sioul_sync::share::PERSONS`).
//!
//! **Ties live where every version keeps them**: a paper's in `links.toml`
//! (`sioul:paper/<id>` → `sioul:person/<id>`, how `holder`), since an older
//! Sioul rewrites the wallet with the fields it knows only; the paper's
//! `holder` still holds the names, which older versions show. A text with
//! no tie is matched at each read: by name or full name, case and accents
//! aside, a text joining names ("Alex, Sam") split; empty, or one of your
//! names, is you.
//!
//! **Old texts are read once on each device** (`read_names`): agendas' names
//! and their other parents, papers' holders, each giving a person whose id is
//! a name-based UUID (RFC 9562 §5.5) of its relation and name, so that two
//! devices reading apart make the same persons, which the sharing joins. What
//! is read so counts until corrected; nothing is written but the persons.
//!
//! **Agents**: each person is closed to AI agents until opened (`agents`);
//! their papers are left out and counted, and others' health papers never
//! reach an agent, whatever the switch (`kept_from_agents`).

use crate::links::{self, Kind, LocalLinks};
use crate::papers::Paper;
use jiff::civil::Date;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The store's file, in Sioul's data folder.
pub const FILE: &str = "persons.toml";
/// The store's format.
pub const FORMAT: u32 = 1;
/// What `names_read` says once old texts were read on this device.
pub const NAMES_READ: u32 = 1;
/// You, always a person, with nothing to fill.
pub const ME: &str = "me";
/// How a paper's tie to a person reads in `links.toml`.
pub const HOLDER: &str = "holder";

/// Who someone is to you, said from your side, in RFC 6350's words where
/// they exist (`RELATED;TYPE=`): "partner", "helped" and "other-parent" are
/// Sioul's own. A word a later version writes reads as unsaid here, and is
/// kept as it was in the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relation {
    Me,
    Child,
    Parent,
    Partner,
    Kin,
    Helped,
    /// In your household, nothing more said.
    Unsaid,
    /// A child's other parent: outside the household, never a holder of papers.
    OtherParent,
}

impl Relation {
    /// What a person's form offers, in the order the household shows them.
    pub const CHOSEN: [Relation; 7] = [Relation::Child, Relation::Parent, Relation::Partner, Relation::Kin, Relation::Helped, Relation::Unsaid, Relation::OtherParent];

    /// Its word in the file and in the strings (`person-relation-<id>`); "" unsaid.
    pub fn id(self) -> &'static str {
        match self {
            Relation::Me => "me",
            Relation::Child => "child",
            Relation::Parent => "parent",
            Relation::Partner => "partner",
            Relation::Kin => "kin",
            Relation::Helped => "helped",
            Relation::Unsaid => "",
            Relation::OtherParent => "other-parent",
        }
    }

    /// The word read leniently: unknown or empty, unsaid.
    pub fn parse(text: &str) -> Relation {
        let word = text.trim().to_ascii_lowercase();
        [Relation::Me, Relation::Child, Relation::Parent, Relation::Partner, Relation::Kin, Relation::Helped, Relation::OtherParent].into_iter().find(|r| r.id() == word).unwrap_or(Relation::Unsaid)
    }

    /// Whether a paper may be for them: everyone but a child's other parent.
    pub fn holds_papers(self) -> bool {
        self != Relation::OtherParent
    }
}

/// One person.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Person {
    /// Made once, the same on every device: random for one added by hand,
    /// name-based for one read from an old text (`name_id`). The name of the
    /// person's table in the file (`[person.<id>]`), not a field of it.
    #[serde(skip)]
    pub id: String,
    /// What Sioul shows: "Léa".
    #[serde(default)]
    pub name: String,
    /// How papers and letters write it: "Léa Martin".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub full_name: String,
    /// `Relation::id`, as written; "" unsaid.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub relation: String,
    /// The ages French papers follow (12, 16, 18, 20, 21, 25), and nothing else.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::budget::dates::optional", serialize_with = "date")]
    pub born: Option<Date>,
    /// Under your roof: the CAF's household, a person under one's roof for taxes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lives_with: Option<bool>,
    /// The contact cards that are this person, by UID.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<String>,
    /// A child's other parent: which children, by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub of: Vec<String>,
    /// The relation's period; `until` set and past, it ended.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::budget::dates::optional", serialize_with = "date")]
    pub since: Option<Date>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::budget::dates::optional", serialize_with = "date")]
    pub until: Option<Date>,
    /// Open to AI agents; closed until opened on the person's page.
    #[serde(default)]
    pub agents: bool,
    /// The last hand-over line said on a child's page (16, then 18): each
    /// said once, on every device (`handover`).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub handed: u8,
    /// What a later version writes here, kept as it is: written back
    /// unchanged, so that this device never sends another version's person as changed.
    #[serde(flatten)]
    pub other: toml::Table,
}

/// A date as TOML writes it bare: `born = 2014-05-12`.
fn date<S: Serializer>(day: &Option<Date>, s: S) -> Result<S::Ok, S::Error> {
    match day {
        Some(d) => toml::value::Datetime { date: Some(toml::value::Date { year: d.year() as u16, month: d.month() as u8, day: d.day() as u8 }), time: None, offset: None }.serialize(s),
        None => s.serialize_none(),
    }
}

impl Person {
    pub fn relation(&self) -> Relation {
        if self.id == ME { Relation::Me } else { Relation::parse(&self.relation) }
    }

    /// Their age today, from `born`.
    pub fn age(&self, today: Date) -> Option<i16> {
        let born = self.born?;
        let mut years = today.year() - born.year();
        if (today.month(), today.day()) < (born.month(), born.day()) {
            years -= 1;
        }
        (years >= 0).then_some(years)
    }

    /// Whether the relation ended (`until` past).
    pub fn ended(&self, today: Date) -> bool {
        self.until.is_some_and(|until| until < today)
    }

    /// Whether `text` is their name or their full name, case and accents aside.
    pub fn named(&self, text: &str) -> bool {
        let wanted = key(text);
        !wanted.is_empty() && (key(&self.name) == wanted || (!self.full_name.trim().is_empty() && key(&self.full_name) == wanted))
    }
}

/// The people your admin is done for, as the file holds them (`File`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "File", into = "File")]
pub struct Persons {
    #[serde(default)]
    pub format: u32,
    /// `NAMES_READ` once the old texts were read on this device (`read_names`):
    /// it stays on each device, the sharing leaving it out.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub names_read: u32,
    /// Names that are no person ("Banque"): no later read makes one of them again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_persons: Vec<String>,
    /// Each person, their id from their table's name; in the file's order (by id) once read.
    pub persons: Vec<Person>,
    /// What a later version writes here, kept as it is.
    pub other: toml::Table,
}

/// The file's own shape: each person a table named by its id.
#[derive(Serialize, Deserialize, Default)]
struct File {
    #[serde(default)]
    format: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    names_read: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    not_persons: Vec<String>,
    #[serde(default, rename = "person", skip_serializing_if = "BTreeMap::is_empty")]
    persons: BTreeMap<String, Person>,
    #[serde(flatten)]
    other: toml::Table,
}

impl From<File> for Persons {
    fn from(file: File) -> Persons {
        let persons = file.persons.into_iter().map(|(id, person)| Person { id, ..person }).collect();
        Persons { format: file.format, names_read: file.names_read, not_persons: file.not_persons, persons, other: file.other }
    }
}

impl From<Persons> for File {
    fn from(store: Persons) -> File {
        let persons = store.persons.into_iter().filter(|p| !p.id.is_empty()).map(|p| (p.id.clone(), p)).collect();
        File { format: store.format, names_read: store.names_read, not_persons: store.not_persons, persons, other: store.other }
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

/// How a letter's tie to the person it is addressed to reads in `links.toml`.
pub const ADDRESSED: &str = "to";

/// A name as compared: case and accents aside, spaces made one.
pub fn key(text: &str) -> String {
    crate::words::folded(text).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The words a holder text uses for you, beside your accounts' names.
const ME_WORDS: &[&str] = &["me", "myself", "you", "moi", "moi-meme", "moi meme", "vous"];

/// Whether a holder text names you: empty, one of your names (`own`: your
/// accounts' display names), or a word for you ("moi", "me", "myself", « moi-même »).
pub fn is_me(text: &str, own: &[String]) -> bool {
    let wanted = key(text);
    wanted.is_empty() || ME_WORDS.contains(&wanted.as_str()) || own.iter().any(|name| key(name) == wanted)
}

/// A text joining names, cut at ", ", " et ", " and " and " & ": "Alex, Sam et Léa" → three.
pub fn split(text: &str) -> Vec<String> {
    let mut parts = vec![text.to_string()];
    for separator in [",", ";", " et ", " and ", " & "] {
        parts = parts.iter().flat_map(|p| p.split(separator).map(str::to_string).collect::<Vec<_>>()).collect();
    }
    parts.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

/// An address as compared: its address part ("Sam <sam@example.org>"), without "mailto:", in lower case.
fn address_key(text: &str) -> String {
    let text = text.trim();
    let inner = text.rsplit_once('<').map_or(text, |(_, rest)| rest.trim_end_matches('>'));
    let inner = inner.trim();
    let inner = if inner.len() >= 7 && inner[..7].eq_ignore_ascii_case("mailto:") { &inner[7..] } else { inner };
    inner.trim().to_lowercase()
}

/// A person's address: `sioul:person/<id>`.
pub fn uri(id: &str) -> String {
    links::person_uri(id)
}

/// The id of a person read from an old text: a name-based UUID, version 5,
/// of the relation and the name, case and accents aside, in Sioul's own
/// namespace (`holders::NAMESPACE`), so that a child and an other parent of
/// the same first name stay two, and two devices reading apart agree.
pub fn name_id(relation: Relation, name: &str) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("person\n{}\n{}", relation.id(), key(name)))
}

/// Who a paper is for: persons by id (`ME` for you), and what names nobody,
/// as written (an older Sioul's text that matches no person).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Holders {
    pub persons: Vec<String>,
    pub texts: Vec<String>,
}

impl Holders {
    /// Yours alone: as a paper with no holder always was.
    pub fn only_me(&self) -> bool {
        self.texts.is_empty() && self.persons.iter().all(|p| p == ME)
    }

    pub fn has(&self, id: &str) -> bool {
        self.persons.iter().any(|p| p == id)
    }
}

impl Persons {
    /// `$XDG_DATA_HOME/sioul/persons.toml`.
    pub fn default_path() -> PathBuf {
        crate::config::data_dir().join(FILE)
    }

    /// The store as it is; none there is nobody yet. One that does not read is
    /// an error, never an empty store (a change would otherwise take everyone
    /// out on every device).
    pub fn read(path: &Path) -> Result<Persons, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Persons::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// For a question: what cannot be read is nobody.
    pub fn load(path: &Path) -> Persons {
        Persons::read(path).unwrap_or_default()
    }

    /// Written whole, next to its place then moved: never half a file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        let mut out = self.clone();
        out.format = out.format.max(FORMAT);
        let text = format!("# The people your admin is done for (docs/persons.md).\n\n{}", toml::to_string(&out).map_err(|e| e.to_string())?);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let fresh = path.with_file_name(format!(".{name}.{}.new", std::process::id()));
        std::fs::write(&fresh, text).map_err(fail)?;
        std::fs::rename(&fresh, path).map_err(|e| {
            let _ = std::fs::remove_file(&fresh);
            fail(e)
        })
    }

    pub fn get(&self, id: &str) -> Option<&Person> {
        self.persons.iter().find(|p| p.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Person> {
        self.persons.iter_mut().find(|p| p.id == id)
    }

    /// Whether a name was said to be no person.
    pub fn not_a_person(&self, name: &str) -> bool {
        let wanted = key(name);
        self.not_persons.iter().any(|n| key(n) == wanted)
    }

    /// A person added (a random id when it has none) or changed; their id.
    pub fn put(&mut self, mut person: Person) -> String {
        if person.id.is_empty() {
            person.id = crate::ids::new();
        }
        let id = person.id.clone();
        match self.get_mut(&id) {
            Some(place) => *place = person,
            None => self.persons.push(person),
        }
        id
    }

    /// Someone taken out of the store; their ties stay where they are.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.persons.len();
        self.persons.retain(|p| p.id != id);
        self.persons.len() != before
    }

    /// The person a paper may be for whose name or full name is `text`: you
    /// by your names, else a person of the household (never a child's other parent).
    pub fn holder_named(&self, text: &str, own: &[String]) -> Option<String> {
        if is_me(text, own) {
            return Some(ME.into());
        }
        self.persons.iter().find(|p| p.relation().holds_papers() && p.named(text)).map(|p| p.id.clone())
    }

    /// Whose a holder text says: the whole text when it names someone, else
    /// each of its parts; what names nobody stays text.
    pub fn read_holder(&self, text: &str, own: &[String]) -> Holders {
        if let Some(id) = self.holder_named(text, own) {
            return Holders { persons: vec![id], texts: Vec::new() };
        }
        let mut out = Holders::default();
        for part in split(text) {
            match self.holder_named(&part, own) {
                Some(id) if !out.has(&id) => out.persons.push(id),
                Some(_) => {}
                None => out.texts.push(part),
            }
        }
        out
    }

    /// Whom a paper is for: its ties (`links.toml`, how `holder`) to persons
    /// still here, else its holder text as `read_holder` reads it.
    pub fn holders_of(&self, paper: &Paper, local: &LocalLinks, own: &[String]) -> Holders {
        let tied = tied_persons(local, &paper.id);
        let persons: Vec<String> = tied.into_iter().filter(|id| id == ME || self.get(id).is_some()).collect();
        if !persons.is_empty() {
            return Holders { persons, texts: Vec::new() };
        }
        self.read_holder(&paper.holder, own)
    }

    /// A person's name as shown: theirs, or `you` for you.
    pub fn name_of(&self, id: &str, you: &str) -> String {
        if id == ME {
            return you.to_string();
        }
        self.get(id).map(|p| p.name.clone()).unwrap_or_default()
    }

    /// The person a contact card is, by its UID.
    pub fn of_card(&self, uid: &str) -> Option<&Person> {
        let uid = uid.trim();
        (!uid.is_empty()).then(|| self.persons.iter().find(|p| p.cards.iter().any(|c| c == uid))).flatten()
    }

    /// A card linked to a person, and to them alone; whether anything changed.
    pub fn link_card(&mut self, id: &str, uid: &str) -> bool {
        let uid = uid.trim();
        if uid.is_empty() || self.get(id).is_none() {
            return false;
        }
        let mut changed = false;
        for person in self.persons.iter_mut().filter(|p| p.id != id) {
            let before = person.cards.len();
            person.cards.retain(|c| c != uid);
            changed |= person.cards.len() != before;
        }
        if let Some(person) = self.get_mut(id)
            && !person.cards.iter().any(|c| c == uid)
        {
            person.cards.push(uid.to_string());
            changed = true;
        }
        changed
    }

    pub fn unlink_card(&mut self, id: &str, uid: &str) -> bool {
        let Some(person) = self.get_mut(id) else { return false };
        let before = person.cards.len();
        person.cards.retain(|c| c != uid.trim());
        person.cards.len() != before
    }

    /// The household as shown (Contacts, « Proches »): you first, then
    /// children, eldest first, parents, partners, family, people you help,
    /// those unsaid; children's other parents apart, after. Ended relations
    /// are left out.
    pub fn household(&self, today: Date) -> (Vec<&Person>, Vec<&Person>) {
        let mut inside: Vec<&Person> = self.persons.iter().filter(|p| !p.ended(today) && p.relation().holds_papers() && p.id != ME).collect();
        inside.sort_by(|a, b| (a.relation(), a.born.unwrap_or(Date::MAX), key(&a.name)).cmp(&(b.relation(), b.born.unwrap_or(Date::MAX), key(&b.name))));
        let mut others: Vec<&Person> = self.persons.iter().filter(|p| !p.ended(today) && !p.relation().holds_papers()).collect();
        others.sort_by_key(|p| key(&p.name));
        (inside, others)
    }

    /// Old texts read once on this device (docs/persons.md, "Read from old
    /// texts"), while `names_read` does not say so: each agenda said a
    /// child's, a partner's or another parent's with a name gives a person;
    /// each child's agenda's other parent gives one too, their `of` naming the
    /// child; each paper's holder text naming nobody yet gives a person of an
    /// unsaid relation, except your names and the empty text, which are you,
    /// and what was said to be no person. Ids are name-based (`name_id`).
    /// Nothing else is written; `names_read` last. Whether anything changed.
    pub fn read_names(&mut self, agendas: &BTreeMap<String, crate::agendas::Said>, papers: &[Paper], own: &[String]) -> bool {
        if self.names_read >= NAMES_READ {
            return false;
        }
        let before = self.clone();
        let said = |field: &Option<String>| field.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
        for agenda in agendas.values() {
            let Some(whose) = said(&agenda.whose).and_then(|w| crate::agendas::Whose::parse(&w)) else { continue };
            let relation = match whose {
                crate::agendas::Whose::Child => Relation::Child,
                crate::agendas::Whose::Partner => Relation::Partner,
                crate::agendas::Whose::Parent => Relation::OtherParent,
                _ => continue,
            };
            let Some(name) = said(&agenda.name) else { continue };
            let Some(id) = self.found(relation, &name) else { continue };
            if relation == Relation::Child
                && let Some(parent) = said(&agenda.parent)
                && let Some(other) = self.found(Relation::OtherParent, &parent)
                && let Some(person) = self.get_mut(&other)
                && !person.of.contains(&id)
            {
                person.of.push(id);
            }
        }
        for paper in papers {
            for part in self.read_holder(&paper.holder, own).texts {
                self.found(Relation::Unsaid, &part);
            }
        }
        self.names_read = NAMES_READ;
        *self != before
    }

    /// The person of this relation and name, made when there is none (and
    /// the name was not said to be no person); their id.
    fn found(&mut self, relation: Relation, name: &str) -> Option<String> {
        let name = name.trim();
        if name.is_empty() || self.not_a_person(name) {
            return None;
        }
        let id = name_id(relation, name);
        if let Some(known) = self.persons.iter().find(|p| p.id == id || (p.relation() == relation && p.named(name))) {
            return Some(known.id.clone());
        }
        self.persons.push(Person { id: id.clone(), name: name.to_string(), relation: relation.id().to_string(), ..Person::default() });
        Some(id)
    }

    /// The letters kept from AI agents for a person's sake, by address: a
    /// letter for a person not opened to agents (by its ties, else its
    /// addressee). `letters`: each letter's id and addressee.
    pub fn kept_letters(&self, letters: &[(String, String)], local: &LocalLinks, own: &[String]) -> BTreeSet<String> {
        let open = |id: &str| id == ME || self.get(id).is_some_and(|p| p.agents);
        letters.iter().filter(|(id, addressee)| self.letter_for(id, addressee, local, own).iter().any(|p| !open(p))).map(|(id, _)| links::letter_uri(id)).collect()
    }

    /// What never reaches an AI agent, by address (docs/mcp.md): the papers
    /// for a person not opened to agents, or for a name that is nobody's (a
    /// paper of two, when either is); health papers (a health card, a health
    /// cover) of anyone but you, whatever the switch; and the closed persons'
    /// own addresses. You are always open.
    pub fn kept_from_agents(&self, papers: &[Paper], local: &LocalLinks, own: &[String]) -> BTreeSet<String> {
        let open = |id: &str| id == ME || self.get(id).is_some_and(|p| p.agents);
        let mut kept: BTreeSet<String> = self.persons.iter().filter(|p| !open(&p.id)).map(|p| uri(&p.id)).collect();
        for paper in papers {
            let holders = self.holders_of(paper, local, own);
            let health = paper.kind.family() == "health";
            let closed = !holders.texts.is_empty() || holders.persons.iter().any(|id| !open(id));
            if closed || (health && !holders.only_me()) {
                kept.insert(links::paper_uri(&paper.id));
            }
        }
        kept
    }

    /// The hand-over line due on a child's page today (docs/persons.md):
    /// 16 from their sixteenth birthday, 18 from their eighteenth, each once
    /// (`handed` says the last said); none for anyone else or without a date
    /// of birth.
    pub fn handover(&self, id: &str, today: Date) -> Option<u8> {
        let person = self.get(id).filter(|p| p.relation() == Relation::Child)?;
        let age = person.age(today)?;
        if age >= 18 && person.handed < 18 {
            Some(18)
        } else if (16..18).contains(&age) && person.handed < 16 {
            Some(16)
        } else {
            None
        }
    }

    /// The persons a letter is addressed to, by its address block's name
    /// (`letters::Reading::addressee`): you by your names, and each person of
    /// the household whose full name (or name, when none is said) is in it,
    /// its words in any order ("Léa MARTIN", "MARTIN Léa"), case and accents
    /// aside. A block naming nobody here gives nobody.
    pub fn addressed(&self, addressee: &str, own: &[String]) -> Vec<String> {
        let words: BTreeSet<String> = key(addressee).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let within = |name: &str| {
            let wanted: Vec<String> = key(name).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect();
            !wanted.is_empty() && wanted.iter().all(|w| words.contains(w))
        };
        let mut out: Vec<String> = Vec::new();
        if own.iter().any(|name| within(name)) {
            out.push(ME.to_string());
        }
        for person in self.persons.iter().filter(|p| p.relation().holds_papers()) {
            let name = if person.full_name.trim().is_empty() { &person.name } else { &person.full_name };
            if within(name) {
                out.push(person.id.clone());
            }
        }
        out
    }

    /// Whom a letter is for: its ties (`links.toml`, how `to`) to persons
    /// still here, else its addressee as `addressed` reads it.
    pub fn letter_for(&self, letter: &str, addressee: &str, local: &LocalLinks, own: &[String]) -> Vec<String> {
        let tied: Vec<String> = tied_to(local, Kind::Letter, letter, ADDRESSED).into_iter().filter(|id| id == ME || self.get(id).is_some()).collect();
        if !tied.is_empty() {
            return tied;
        }
        self.addressed(addressee, own)
    }

    /// Two persons made one ("Same as…": Léa and "Léa M."): `keep` takes
    /// what `other` holds and it lacks (a full name, a date of birth, cards,
    /// children), and `other` goes. The ties and texts naming `other` are the
    /// caller's to move (`retie`, `rename_holders`, `renamed_agendas`).
    /// Whether both were here.
    pub fn join(&mut self, keep: &str, other: &str) -> bool {
        if keep == other {
            return false;
        }
        let (Some(gone), true) = (self.get(other).cloned(), self.get(keep).is_some()) else { return false };
        let person = self.get_mut(keep).expect("checked above");
        if person.full_name.trim().is_empty() {
            person.full_name = gone.full_name.clone();
        }
        person.born = person.born.or(gone.born);
        person.lives_with = person.lives_with.or(gone.lives_with);
        person.since = person.since.or(gone.since);
        person.agents = person.agents && gone.agents;
        for card in gone.cards {
            if !person.cards.contains(&card) {
                person.cards.push(card);
            }
        }
        for child in gone.of {
            if !person.of.contains(&child) {
                person.of.push(child);
            }
        }
        self.remove(other);
        // Another parent naming `other` as their child names `keep` now.
        for parent in self.persons.iter_mut() {
            if parent.of.iter().any(|c| c == other) {
                parent.of.retain(|c| c != other);
                if !parent.of.iter().any(|c| c == keep) {
                    parent.of.push(keep.to_string());
                }
            }
        }
        true
    }

    /// "This is not a person" ("Banque"): taken out, their name kept among
    /// the names that are no person, so that no later read makes them again.
    /// Their papers keep their texts. Whether they were here.
    pub fn not_a_person_now(&mut self, id: &str) -> bool {
        let Some(gone) = self.get(id).cloned() else { return false };
        self.remove(id);
        if !self.not_a_person(&gone.name) {
            self.not_persons.push(gone.name.clone());
        }
        for parent in self.persons.iter_mut() {
            parent.of.retain(|c| c != id);
        }
        true
    }

    /// The store as the file holds it now, read again only when the file
    /// changed (its size and time): what each agenda reads (`agendas::agenda_of`).
    pub fn cached() -> Persons {
        use std::sync::Mutex;
        static CACHE: Mutex<Option<(PathBuf, Option<(u64, std::time::SystemTime)>, Persons)>> = Mutex::new(None);
        let path = Persons::default_path();
        let stamp = std::fs::metadata(&path).ok().map(|m| (m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
        let Ok(mut cache) = CACHE.lock() else { return Persons::load(&path) };
        if let Some((at, was, persons)) = cache.as_ref()
            && *at == path
            && *was == stamp
        {
            return persons.clone();
        }
        let persons = Persons::load(&path);
        *cache = Some((path, stamp, persons.clone()));
        persons
    }

    /// The person an agenda is (docs/persons.md): its `person` key when that
    /// person is here, else its name matched to a person of its relation.
    pub fn of_agenda(&self, said: &crate::agendas::Said, relation: Relation) -> Option<&Person> {
        if let Some(id) = said.person.as_deref().map(str::trim).filter(|id| !id.is_empty())
            && let Some(person) = self.get(id)
        {
            return Some(person);
        }
        let name = said.name.as_deref().map(str::trim).filter(|n| !n.is_empty())?;
        self.persons.iter().find(|p| p.relation() == relation && p.named(name))
    }

    /// A child's agenda's other parent: its `parent_person` key when that
    /// person is here, else its `parent` matched to a child's other parent.
    pub fn parent_of_agenda(&self, said: &crate::agendas::Said) -> Option<&Person> {
        if let Some(id) = said.parent_person.as_deref().map(str::trim).filter(|id| !id.is_empty())
            && let Some(person) = self.get(id)
        {
            return Some(person);
        }
        let name = said.parent.as_deref().map(str::trim).filter(|n| !n.is_empty())?;
        self.persons.iter().find(|p| p.relation() == Relation::OtherParent && p.named(name))
    }

    /// The agendas kept from AI agents for a person's sake (docs/persons.md),
    /// by place: a child's, a partner's or another parent's agenda whose
    /// person is not opened to agents, or that names nobody here. Yours and
    /// those shared with you by anyone else follow their own consent.
    pub fn closed_agendas(&self, agendas: &[crate::agendas::Agenda]) -> BTreeSet<String> {
        use crate::agendas::Whose;
        agendas
            .iter()
            .filter(|a| matches!(a.whose, Whose::Child | Whose::Partner | Whose::Parent))
            .filter(|a| a.person_id.is_empty() || self.get(&a.person_id).is_none_or(|p| !p.agents))
            .map(|a| a.id.clone())
            .collect()
    }

    /// The persons as a read for agents and for the window takes them: the
    /// store, with old texts read as `read_names` would read them when this
    /// device has not yet (nothing written).
    pub fn as_read(path: &Path, agendas: &BTreeMap<String, crate::agendas::Said>, papers: &[Paper], own: &[String]) -> Persons {
        let mut persons = Persons::load(path);
        persons.read_names(agendas, papers, own);
        persons
    }
}

/// The persons a paper is tied to in `links.toml` (how `holder`), by id, in the order written.
pub fn tied_persons(local: &LocalLinks, paper: &str) -> Vec<String> {
    tied_to(local, Kind::Paper, paper, HOLDER)
}

/// The persons a thing of this kind and id is tied to in `links.toml` with
/// this how, by id, in the order written.
pub fn tied_to(local: &LocalLinks, kind: Kind, thing: &str, how: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for edge in local.links.iter().filter(|e| e.how == how) {
        for (from, to) in [(&edge.from, &edge.to), (&edge.to, &edge.from)] {
            if links::kind_of(from) == kind && links::id_of(from) == thing && links::kind_of(to) == Kind::Person {
                let id = links::id_of(to);
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

/// A letter's ties to the persons it is for set to `persons` (none: its
/// addressee read again decides); whether anything changed.
pub fn tie_letter(local: &mut LocalLinks, letter: &str, persons: &[String]) -> bool {
    let before = local.links.clone();
    let mine = |e: &links::Edge| e.how == ADDRESSED && [(&e.from, &e.to), (&e.to, &e.from)].into_iter().any(|(a, b)| links::kind_of(a) == Kind::Letter && links::id_of(a) == letter && links::kind_of(b) == Kind::Person);
    local.links.retain(|e| !mine(e));
    for id in persons {
        local.add(&links::letter_uri(letter), &uri(id), ADDRESSED);
    }
    local.links != before
}

/// Every tie to `from` (a paper's, a letter's) moved to `to` ("Same as…"),
/// or taken out when `to` is "" ("This is not a person"); whether anything changed.
pub fn retie(local: &mut LocalLinks, from: &str, to: &str) -> bool {
    let before = local.links.clone();
    let is_gone = |end: &str| links::kind_of(end) == Kind::Person && links::id_of(end) == from;
    let mut moved: Vec<links::Edge> = Vec::new();
    local.links.retain(|e| {
        let touches = is_gone(&e.from) || is_gone(&e.to);
        if touches && !to.is_empty() {
            let swap = |end: &str| if is_gone(end) { uri(to) } else { end.to_string() };
            moved.push(links::Edge { from: swap(&e.from), to: swap(&e.to), how: e.how.clone() });
        }
        !touches
    });
    for edge in moved {
        local.add(&edge.from, &edge.to, &edge.how);
    }
    local.links != before
}

/// A paper's ties to persons set to `persons` (`ME` alone: none, yours as a
/// paper with no holder is), the others taken out; whether anything changed.
pub fn tie_paper(local: &mut LocalLinks, paper: &str, persons: &[String]) -> bool {
    let before = local.links.len();
    let mine = |e: &links::Edge| e.how == HOLDER && [(&e.from, &e.to), (&e.to, &e.from)].into_iter().any(|(a, b)| links::kind_of(a) == Kind::Paper && links::id_of(a) == paper && links::kind_of(b) == Kind::Person);
    let old: Vec<links::Edge> = local.links.iter().filter(|e| mine(e)).cloned().collect();
    local.links.retain(|e| !mine(e));
    if !persons.iter().all(|p| p == ME) {
        for id in persons {
            local.add(&links::paper_uri(paper), &uri(id), HOLDER);
        }
    }
    let new: Vec<&links::Edge> = local.links.iter().filter(|e| mine(e)).collect();
    local.links.len() != before || old.iter().collect::<Vec<_>>() != new
}

/// The holder text older versions show for these persons and texts: their
/// names joined by ", "; you by `you` (your first name when you have one),
/// alone, nothing (a paper with no holder is yours).
pub fn holder_text(persons: &Persons, chosen: &[String], texts: &[String], you: &str) -> String {
    if chosen.iter().all(|p| p == ME) && texts.is_empty() {
        return String::new();
    }
    let mut names: Vec<String> = chosen.iter().map(|id| persons.name_of(id, you)).filter(|n| !n.trim().is_empty()).collect();
    names.extend(texts.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()));
    names.join(", ")
}

/// A holder text with a person's old name made the new one, the whole text
/// or each part of a text joining names; None when it did not name them.
pub fn renamed(text: &str, old: &str, new: &str) -> Option<String> {
    let old = key(old);
    if old.is_empty() || key(text).is_empty() {
        return None;
    }
    if key(text) == old {
        return Some(new.trim().to_string());
    }
    let parts = split(text);
    if parts.len() < 2 || !parts.iter().any(|p| key(p) == old) {
        return None;
    }
    Some(parts.iter().map(|p| if key(p) == old { new.trim().to_string() } else { p.clone() }).collect::<Vec<_>>().join(", "))
}

/// A person renamed in the papers' holder texts: each paper `before` (the
/// store as it was, with the old name) reads as theirs, tied or by text, has
/// the old name made the new one, so that older versions show it. A text
/// naming them among others keeps the others. How many papers changed.
pub fn rename_holders(papers: &mut [Paper], before: &Persons, local: &LocalLinks, own: &[String], id: &str, new: &str) -> usize {
    let Some(old) = before.get(id).map(|p| p.name.clone()) else { return 0 };
    let mut changed = 0;
    for paper in papers.iter_mut() {
        if before.holders_of(paper, local, own).has(id)
            && let Some(text) = renamed(&paper.holder, &old, new)
            && text != paper.holder
        {
            paper.holder = text;
            changed += 1;
        }
    }
    changed
}

/// A person renamed in the agendas' words (`[agenda.calendars]`): each
/// agenda whose `person` is theirs, or whose `name` named them by the old
/// name (`before`: the store as it was), gets `name` = the new one; each
/// whose `parent_person` is theirs, or whose `parent` named them, gets
/// `parent` = the new one; so that older versions show it. The fields to
/// write: (place, field, value).
pub fn renamed_agendas(calendars: &BTreeMap<String, crate::agendas::Said>, before: &Persons, id: &str, new: &str) -> Vec<(String, &'static str, String)> {
    use crate::agendas::Whose;
    let Some(person) = before.get(id) else { return Vec::new() };
    let mut out = Vec::new();
    for (place, said) in calendars {
        let whose = said.whose.as_deref().and_then(Whose::parse);
        let relation = match whose {
            Some(Whose::Child) => Some(Relation::Child),
            Some(Whose::Partner) => Some(Relation::Partner),
            Some(Whose::Parent) => Some(Relation::OtherParent),
            _ => None,
        };
        if relation.is_some_and(|r| before.of_agenda(said, r).is_some_and(|p| p.id == person.id)) && said.name.as_deref().is_none_or(|n| n.trim() != new.trim()) {
            out.push((place.clone(), "name", new.trim().to_string()));
        }
        if whose == Some(Whose::Child) && before.parent_of_agenda(said).is_some_and(|p| p.id == person.id) && said.parent.as_deref().is_none_or(|n| n.trim() != new.trim()) {
            out.push((place.clone(), "parent", new.trim().to_string()));
        }
    }
    out
}

/// The person an address is, by address alone (a holder line's, a
/// sender's): an address on one of a person's cards, or an agenda's other
/// parent's address for an other parent of that name. `cards`: each card's
/// UID and its addresses.
pub fn by_address(persons: &Persons, address: &str, cards: &[(String, Vec<String>)], agendas: &BTreeMap<String, crate::agendas::Said>) -> Option<String> {
    let wanted = address_key(address);
    if wanted.is_empty() || !wanted.contains('@') {
        return None;
    }
    for person in &persons.persons {
        let on_card = person.cards.iter().any(|uid| cards.iter().any(|(card, addresses)| card == uid && addresses.iter().any(|a| address_key(a) == wanted)));
        if on_card {
            return Some(person.id.clone());
        }
    }
    for agenda in agendas.values() {
        let (Some(parent), Some(address)) = (agenda.parent.as_deref(), agenda.parent_address.as_deref()) else { continue };
        if address_key(address) == wanted
            && let Some(person) = persons.persons.iter().find(|p| p.relation() == Relation::OtherParent && p.named(parent))
        {
            return Some(person.id.clone());
        }
    }
    None
}

/// The cards an other parent is, by their address alone: the one card that
/// holds an agenda's address for that parent, when exactly one does (never
/// by first name: a "Léa" among colleagues). `cards`: each card's UID and its addresses.
pub fn card_of_address(person: &Person, cards: &[(String, Vec<String>)], agendas: &BTreeMap<String, crate::agendas::Said>) -> Option<String> {
    if person.relation() != Relation::OtherParent {
        return None;
    }
    let addresses: BTreeSet<String> = agendas.values().filter(|a| a.parent.as_deref().is_some_and(|p| person.named(p))).filter_map(|a| a.parent_address.as_deref()).map(address_key).filter(|a| a.contains('@')).collect();
    let holding: Vec<&String> = cards.iter().filter(|(_, list)| list.iter().any(|a| addresses.contains(&address_key(a)))).map(|(uid, _)| uid).collect();
    (holding.len() == 1).then(|| holding[0].clone())
}

/// The store changed under its lock, read again first: what the sharing
/// brought meanwhile stays. A file that does not read is left alone, an
/// error. Written only when `change` changed something; returns what it returned.
pub fn change<R>(path: &Path, change: impl FnOnce(&mut Persons) -> R) -> Result<R, String> {
    crate::filelock::with_lock(path, || {
        let mut persons = Persons::read(path)?;
        let before = persons.clone();
        let out = change(&mut persons);
        if persons != before {
            persons.save(path)?;
        }
        Ok(out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-persons-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn agenda(whose: &str, name: &str, parent: &str, address: &str) -> crate::agendas::Said {
        let text = |t: &str| (!t.is_empty()).then(|| t.to_string());
        crate::agendas::Said { whose: text(whose), name: text(name), parent: text(parent), parent_address: text(address), ..Default::default() }
    }

    fn paper(id: &str, holder: &str) -> Paper {
        Paper { id: id.into(), title: id.into(), holder: holder.into(), ..Paper::default() }
    }

    #[test]
    fn the_store_keeps_what_a_later_version_writes() {
        let dir = scratch("store");
        let path = dir.join(FILE);
        let text = "format = 1\nnames_read = 1\nnot_persons = [\"Banque\"]\nlater = \"kept\"\n\n[person.p1]\nname = \"Léa\"\nfull_name = \"Léa Martin\"\nrelation = \"child\"\nborn = 2014-05-12\nlives_with = true\nagents = false\nhobby = \"chess\"\n\n[person.p2]\nname = \"Sam\"\nrelation = \"other-parent\"\nof = [\"p1\"]\ncards = [\"c-sam-1\"]\nsince = 2019-06-01\nrelation_kind = { word = \"later\" }\n";
        std::fs::write(&path, text).unwrap();
        let persons = Persons::read(&path).unwrap();
        assert_eq!(persons.persons.len(), 2);
        let lea = persons.get("p1").unwrap();
        assert_eq!((lea.born, lea.lives_with, lea.relation()), (Some(day("2014-05-12")), Some(true), Relation::Child));
        assert_eq!(lea.other.get("hobby").and_then(|v| v.as_str()), Some("chess"));
        assert_eq!(persons.get("p2").unwrap().relation(), Relation::OtherParent);
        assert!(persons.not_a_person("BANQUE"));
        persons.save(&path).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("born = 2014-05-12") && written.contains("since = 2019-06-01") && written.contains("[person.p1]") && !written.contains("id = "), "{written}");
        let again = Persons::read(&path).unwrap();
        assert_eq!(again, persons, "{written}");
        assert_eq!(again.other.get("later").and_then(|v| v.as_str()), Some("kept"));
        // A word a later version writes: unsaid here, kept as written.
        let later = Person { id: "p3".into(), relation: "co-resident".into(), ..Person::default() };
        assert_eq!(later.relation(), Relation::Unsaid);
        // A store that does not read is an error, never an empty store.
        std::fs::write(&path, "[person.p1\nname = ").unwrap();
        assert!(Persons::read(&path).is_err() && change(&path, |p| p.put(Person::default())).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_match_case_and_accents_aside_and_joined_texts_split() {
        let mut persons = Persons::default();
        let lea = persons.put(Person { name: "Léa".into(), full_name: "Léa Martin".into(), relation: "child".into(), ..Person::default() });
        let sam = persons.put(Person { name: "Sam".into(), relation: "partner".into(), ..Person::default() });
        let other = persons.put(Person { name: "Tom".into(), relation: "other-parent".into(), ..Person::default() });
        let own = vec!["Alex Durand".to_string()];
        assert_eq!(persons.read_holder("LEA", &own).persons, [lea.clone()]);
        assert_eq!(persons.read_holder("lea martin", &own).persons, [lea.clone()]);
        assert_eq!(persons.read_holder("", &own).persons, [ME]);
        assert_eq!(persons.read_holder("Moi-même", &own).persons, [ME]);
        assert_eq!(persons.read_holder("alex durand", &own).persons, [ME]);
        let joint = persons.read_holder("Alex Durand, Sam et Léa & Odile", &own);
        assert_eq!((joint.persons, joint.texts), (vec![ME.to_string(), sam.clone(), lea.clone()], vec!["Odile".to_string()]));
        // A child's other parent is never a paper's holder.
        assert_eq!(persons.read_holder("Tom", &own).texts, ["Tom"]);
        assert!(persons.get(&other).is_some());
        assert_eq!(split("Alex and Sam; Léa"), ["Alex", "Sam", "Léa"]);
        assert_eq!(renamed("Léa, Sam", "lea", "Léa M."), Some("Léa M., Sam".into()));
        assert_eq!(renamed("LÉA", "lea", "Léa M."), Some("Léa M.".into()));
        assert_eq!(renamed("Léana", "lea", "Léa M."), None);
    }

    #[test]
    fn old_texts_are_read_once_into_the_same_persons_on_every_device() {
        let agendas: BTreeMap<String, crate::agendas::Said> = [
            ("home/lea-school".to_string(), agenda("child", "Léa", "Sam", "sam@example.org")),
            ("home/lea-sport".to_string(), agenda("child", "LEA", "sam", "")),
            ("home/sam".to_string(), agenda("parent", "Sam", "", "")),
            ("home/mine".to_string(), agenda("yours", "Alex", "", "")),
            ("home/partner".to_string(), agenda("partner", "Robin", "", "")),
        ]
        .into();
        let papers = [paper("id-lea", "Léa"), paper("tax", "Alex Durand, Robin"), paper("mine", ""), paper("bank", "Banque"), paper("odile", "Odile"), paper("sam", "Sam")];
        let own = vec!["Alex Durand".to_string()];
        let mut computer = Persons { not_persons: vec!["banque".into()], ..Persons::default() };
        assert!(computer.read_names(&agendas, &papers, &own));
        let names: Vec<(String, &str)> = computer.persons.iter().map(|p| (p.name.clone(), p.relation.as_str())).collect();
        assert_eq!(names, [("Léa".to_string(), "child"), ("Sam".to_string(), "other-parent"), ("Robin".to_string(), "partner"), ("Odile".to_string(), ""), ("Sam".to_string(), "")], "two agendas of one child give one person; Sam twice, as other parent and as a paper's holder");
        let lea = computer.persons[0].id.clone();
        assert_eq!(lea, name_id(Relation::Child, "lea"));
        assert_eq!(computer.persons[1].of, [lea.clone()]);
        assert_ne!(computer.persons[1].id, computer.persons[4].id, "a child's other parent and a holder of the same name stay two");
        assert_eq!(computer.names_read, NAMES_READ);
        // Read once: a second time changes nothing, whatever the texts say now.
        assert!(!computer.read_names(&agendas, &[paper("new", "Zoé")], &own));
        // The phone, reading apart, makes the same persons with the same ids.
        let mut phone = Persons { not_persons: vec!["Banque".into()], ..Persons::default() };
        phone.read_names(&agendas, &papers, &own);
        let ids = |p: &Persons| p.persons.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&phone), ids(&computer));
        // A person added by hand is found, not made twice.
        let mut by_hand = Persons::default();
        let kept = by_hand.put(Person { name: "Léa".into(), relation: "child".into(), ..Person::default() });
        by_hand.read_names(&agendas, &papers, &own);
        assert_eq!(by_hand.persons.iter().filter(|p| p.named("Léa")).map(|p| p.id.clone()).collect::<Vec<_>>(), [kept]);
    }

    #[test]
    fn a_paper_is_for_its_ties_then_its_text() {
        let mut persons = Persons::default();
        let lea = persons.put(Person { name: "Léa".into(), relation: "child".into(), ..Person::default() });
        let sam = persons.put(Person { name: "Sam".into(), relation: "partner".into(), ..Person::default() });
        let mut local = LocalLinks::default();
        let lease = paper("lease", "Léa");
        assert_eq!(persons.holders_of(&lease, &local, &[]).persons, [lea.clone()], "by its text");
        assert!(tie_paper(&mut local, "lease", &[ME.to_string(), sam.clone()]));
        assert_eq!(persons.holders_of(&lease, &local, &[]).persons, [ME.to_string(), sam.clone()], "its ties win");
        assert!(!tie_paper(&mut local, "lease", &[ME.to_string(), sam.clone()]), "the same again changes nothing");
        assert_eq!(holder_text(&persons, &[ME.to_string(), sam.clone()], &[], "Alex"), "Alex, Sam");
        assert_eq!(holder_text(&persons, &[ME.to_string()], &[], "Alex"), "");
        // Yours alone: no tie at all.
        assert!(tie_paper(&mut local, "lease", &[ME.to_string()]));
        assert!(local.links.is_empty());
        // A tie to someone gone: the text again.
        tie_paper(&mut local, "lease", &["gone".to_string()]);
        assert_eq!(persons.holders_of(&lease, &local, &[]).persons, [lea]);
    }

    #[test]
    fn a_new_name_is_written_into_the_texts_that_named_the_old_one() {
        let mut before = Persons::default();
        let lea = before.put(Person { name: "Léa".into(), relation: "child".into(), ..Person::default() });
        before.put(Person { name: "Léana".into(), relation: "kin".into(), ..Person::default() });
        let mut papers = vec![paper("id", "LÉA"), paper("lease", "Alex, Léa"), paper("other", "Léana"), paper("tied", "Lea M."), paper("mine", "")];
        let mut local = LocalLinks::default();
        tie_paper(&mut local, "tied", &[lea.clone()]);
        assert_eq!(rename_holders(&mut papers, &before, &local, &["Alex".to_string()], &lea, "Léa M."), 2);
        let holders: Vec<&str> = papers.iter().map(|p| p.holder.as_str()).collect();
        assert_eq!(holders, ["Léa M.", "Alex, Léa M.", "Léana", "Lea M.", ""], "a tied paper whose text named nobody keeps it; another name stays");
    }

    #[test]
    fn agendas_name_their_persons_by_key_then_by_name_and_follow_a_new_name() {
        use crate::agendas::{Agenda, Said};
        let mut persons = Persons::default();
        let lea = persons.put(Person { name: "Léa".into(), relation: "child".into(), ..Person::default() });
        let sam = persons.put(Person { name: "Sam".into(), relation: "other-parent".into(), agents: true, ..Person::default() });
        let text = |t: &str| (!t.is_empty()).then(|| t.to_string());
        let said = |name: &str, person: &str, parent: &str, parent_person: &str| Said { whose: text("child"), name: text(name), person: text(person), parent: text(parent), parent_person: text(parent_person), ..Said::default() };
        let calendars: BTreeMap<String, Said> = [
            ("a/school".to_string(), said("LEA", "", "sam", "")),
            ("a/sport".to_string(), said("Léa old", &lea, "Samuel", &sam)),
            ("a/tom".to_string(), said("Tom", "", "", "")),
        ]
        .into();
        assert_eq!(persons.of_agenda(&calendars["a/school"], Relation::Child).map(|p| p.id.clone()), Some(lea.clone()), "by name, case and accents aside");
        assert_eq!(persons.of_agenda(&calendars["a/sport"], Relation::Child).map(|p| p.id.clone()), Some(lea.clone()), "by its key, whatever its name says");
        assert_eq!(persons.parent_of_agenda(&calendars["a/sport"]).map(|p| p.id.clone()), Some(sam.clone()));
        assert!(persons.of_agenda(&calendars["a/tom"], Relation::Child).is_none());
        // Shown by the person's name; one group for the two agendas of one child.
        let collection = |id: &str| crate::vdir::Collection { kind: crate::vdir::Kind::Calendars, account: "a".into(), id: id.into(), dir: PathBuf::from(format!("/nowhere/a/{id}")), name: id.into(), color: None, read_only: false, components: vec!["VEVENT".into()] };
        let agenda = |id: &str| Agenda::of(&collection(id), &calendars[&format!("a/{id}")], &crate::config::Config::default(), false, &|| true).with_persons(&persons);
        let (school, sport, tom) = (agenda("school"), agenda("sport"), agenda("tom"));
        assert_eq!((school.person.as_str(), sport.person.as_str(), sport.parent.as_str()), ("Léa", "Léa", "Sam"));
        assert_eq!(school.group(), sport.group());
        assert_eq!(tom.group().1, "tom");
        // A child closed to agents: both agendas kept; one naming nobody too; an opened person's not.
        let kept = persons.closed_agendas(&[school.clone(), sport.clone(), tom.clone()]);
        assert_eq!(kept.into_iter().collect::<Vec<_>>(), ["a/school", "a/sport", "a/tom"]);
        persons.get_mut(&lea).unwrap().agents = true;
        assert_eq!(persons.closed_agendas(&[school, sport, tom]).into_iter().collect::<Vec<_>>(), ["a/tom"]);
        // A new name, written where the old one stood for them.
        let mut renamed = renamed_agendas(&calendars, &persons, &lea, "Léa M.");
        renamed.sort();
        assert_eq!(renamed, [("a/school".to_string(), "name", "Léa M.".to_string()), ("a/sport".to_string(), "name", "Léa M.".to_string())]);
        assert_eq!(renamed_agendas(&calendars, &persons, &sam, "Samuel"), [("a/school".to_string(), "parent", "Samuel".to_string())], "the agenda saying Samuel already keeps it");
    }

    #[test]
    fn letters_go_to_whom_their_block_names_and_persons_join_or_go() {
        let mut persons = Persons::default();
        let lea = persons.put(Person { name: "Léa".into(), full_name: "Léa Martin".into(), relation: "child".into(), born: Some(day("2010-01-15")), ..Person::default() });
        let lea_m = persons.put(Person { name: "Léa M.".into(), relation: "child".into(), cards: vec!["c-lea".into()], ..Person::default() });
        let odile = persons.put(Person { name: "Odile".into(), relation: "parent".into(), agents: true, ..Person::default() });
        let own = vec!["Alex Durand".to_string()];
        assert_eq!(persons.addressed("MARTIN Léa", &own), [lea.clone()], "the full name's words in any order");
        assert_eq!(persons.addressed("Alex et Odile Durand", &own), [ME.to_string(), odile.clone()]);
        assert!(persons.addressed("Banque des Berges", &own).is_empty());
        let mut local = LocalLinks::default();
        assert_eq!(persons.letter_for("l1", "Léa MARTIN", &local, &own), [lea.clone()]);
        assert!(tie_letter(&mut local, "l1", &[odile.clone()]));
        assert_eq!(persons.letter_for("l1", "Léa MARTIN", &local, &own), [odile.clone()], "its ties win");
        let kept = persons.kept_letters(&[("l1".into(), "Léa MARTIN".into()), ("l2".into(), "Léa MARTIN".into())], &local, &own);
        assert_eq!(kept.into_iter().collect::<Vec<_>>(), ["sioul:letter/l2"], "Odile opened; Léa closed");
        // Hand-over lines: 16 then 18, each once.
        assert_eq!(persons.handover(&lea, day("2026-01-14")), None);
        assert_eq!(persons.handover(&lea, day("2026-01-15")), Some(16));
        persons.get_mut(&lea).unwrap().handed = 16;
        assert_eq!(persons.handover(&lea, day("2027-06-01")), None);
        assert_eq!(persons.handover(&lea, day("2028-01-15")), Some(18));
        assert_eq!(persons.handover(&odile, day("2028-01-15")), None, "a child's alone");
        // Same as…: Léa M. into Léa, its card and its ties moved.
        tie_paper(&mut local, "card", &[lea_m.clone()]);
        assert!(persons.join(&lea, &lea_m) && retie(&mut local, &lea_m, &lea));
        assert!(persons.get(&lea_m).is_none() && persons.get(&lea).unwrap().cards == ["c-lea"]);
        assert_eq!(tied_persons(&local, "card"), [lea.clone()]);
        // This is not a person: gone, its name kept as no person's, its ties taken out.
        let bank = persons.put(Person { name: "Banque".into(), ..Person::default() });
        tie_paper(&mut local, "rib", &[bank.clone()]);
        assert!(persons.not_a_person_now(&bank) && retie(&mut local, &bank, ""));
        assert!(persons.not_a_person("banque") && tied_persons(&local, "rib").is_empty());
        let mut again = persons.clone();
        again.names_read = 0;
        again.read_names(&BTreeMap::new(), &[paper("rib", "Banque")], &own);
        assert!(!again.persons.iter().any(|p| p.named("Banque")), "never made again from its text");
    }

    #[test]
    fn agents_get_open_persons_papers_and_never_others_health() {
        let mut persons = Persons::default();
        let lea = persons.put(Person { name: "Léa".into(), relation: "child".into(), ..Person::default() });
        let odile = persons.put(Person { name: "Odile".into(), relation: "parent".into(), agents: true, ..Person::default() });
        let health = |id: &str, holder: &str| Paper { kind: crate::papers::Kind::HealthCard, ..paper(id, holder) };
        let papers = [paper("mine", ""), paper("lea-id", "Léa"), paper("odile-id", "Odile"), health("my-card", ""), health("odile-card", "Odile"), paper("joint", "Odile, Léa"), paper("stranger", "Someone")];
        let kept = persons.kept_from_agents(&papers, &LocalLinks::default(), &[]);
        let papers_kept: Vec<String> = kept.iter().filter(|u| u.starts_with("sioul:paper/")).map(|u| links::id_of(u)).collect();
        assert_eq!(papers_kept, ["joint", "lea-id", "odile-card", "stranger"]);
        assert!(kept.contains(&uri(&lea)) && !kept.contains(&uri(&odile)) && !kept.contains(&uri(ME)));
    }

    #[test]
    fn addresses_find_persons_and_one_card() {
        let agendas: BTreeMap<String, crate::agendas::Said> = [("home/lea".to_string(), agenda("child", "Léa", "Sam", "Sam <sam@example.org>"))].into();
        let mut persons = Persons::default();
        persons.read_names(&agendas, &[], &[]);
        let sam = persons.persons.iter().find(|p| p.relation() == Relation::OtherParent).unwrap().clone();
        let cards = vec![("c-sam".to_string(), vec!["SAM@example.org".to_string()]), ("c-lea".to_string(), vec!["lea@example.net".to_string()])];
        assert_eq!(card_of_address(&sam, &cards, &agendas), Some("c-sam".into()));
        assert_eq!(by_address(&persons, "mailto:sam@example.org", &cards, &agendas), Some(sam.id.clone()));
        let two = vec![cards[0].clone(), ("c-sam-work".to_string(), vec!["sam@example.org".to_string()])];
        assert_eq!(card_of_address(&sam, &two, &agendas), None, "two cards hold it: none chosen");
        let lea = persons.persons.iter().find(|p| p.relation() == Relation::Child).unwrap().id.clone();
        assert!(persons.link_card(&lea, "c-lea"));
        assert_eq!(by_address(&persons, "lea@example.net", &cards, &agendas), Some(lea.clone()));
        assert_eq!(persons.of_card("c-lea").map(|p| p.id.clone()), Some(lea.clone()));
        // A card is one person's: linked to another, it leaves the first.
        assert!(persons.link_card(&sam.id, "c-lea"));
        assert!(persons.get(&lea).unwrap().cards.is_empty());
    }

    #[test]
    fn the_household_in_its_order() {
        let mut persons = Persons::default();
        let today = day("2026-11-04");
        for (name, relation, born) in [("Odile", "parent", None), ("Tom", "child", Some("2018-01-02")), ("Léa", "child", Some("2014-05-12")), ("Sam", "other-parent", None), ("Robin", "partner", None), ("Old", "partner", None)] {
            persons.put(Person { name: name.into(), relation: relation.into(), born: born.map(day), until: (name == "Old").then(|| day("2025-01-01")), ..Person::default() });
        }
        let (inside, others) = persons.household(today);
        assert_eq!(inside.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Léa", "Tom", "Odile", "Robin"]);
        assert_eq!(others.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Sam"]);
        assert_eq!(inside[0].age(today), Some(12));
        assert_eq!(inside[0].age(day("2026-05-12")), Some(12));
        assert_eq!(inside[0].age(day("2026-05-11")), Some(11));
    }
}
