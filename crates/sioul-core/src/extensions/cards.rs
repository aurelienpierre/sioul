// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The contacts' door (docs/extensions.md, "Where contacts go"): a set of
//! contacts an extension brought, a business's clients, written into an
//! address book of the person's own that can be written, chosen by them,
//! which its server then carries to every device.
//!
//! For each person of a set, in this order:
//!
//! 1. **A card already marked for them**, in any address book: Sioul's mark
//!    on it, `X-SIOUL-EXTENSION:<mark>` (`MARK_LINE`), the mark made as an
//!    event's UID is (`super::person_mark`: hashes of the extension's id, the
//!    set's and the person's, nothing of those ids), or a UID of that mark
//!    (a card the door made, whose server dropped Sioul's lines).
//! 2. **Else a card of the chosen book that is the same person, by proof
//!    alone**: a number equal once both are keyed (`phones::key`, in the
//!    country of the Contacts settings; never a short code), else an e-mail
//!    address equal case aside. One card: it is bound (the mark written on
//!    it). Several: the one whose name has the same words whatever their
//!    order (`name_words`), else the first by file name. **A name alone never
//!    binds**: two people share a name.
//! 3. **Else a new card** in the chosen book (`new_card`): its UID the mark,
//!    its file named from it (`super::card_file`), `FN` the name as given,
//!    `N` only from the given and family names the extension keeps apart
//!    (never guessed from one string), the numbers, the addresses, `ORG`,
//!    `NOTE`, the extension's name as its category, the mark and the hash.
//! 4. **Additive only**, on every card, bound or made (`completed`): a number
//!    or an address the card lacks (by the same keys) is added; `ORG` and
//!    `NOTE` only where the card has none; `FN` follows the extension's name
//!    only while it is still the one the door wrote last, and `N` likewise
//!    (hashes of them in this device's notes: a card the person renamed keeps
//!    its name). **Nothing is ever removed or replaced**: a number the
//!    service dropped stays on the card until the person takes it off.
//!
//! - **The door never deletes a card**: someone gone from the reply keeps
//!   theirs as it is; Remove leaves every card (`forget`: this device's notes
//!   alone go). Another book chosen: the cards the door made move to it
//!   (`move_made`); the cards it only bound stay where their person keeps
//!   them, still bound (step 1 looks in every book).
//! - **One writer**: every card goes through the contacts' own door for a
//!   person's change (`vdir::write_item`, the cards' guard: never a book the
//!   person only reads), so the ordinary sync sends it. No file of a card is
//!   written any other way.
//! - **Nothing is written when nothing would change**: a hash of the person
//!   as the extension gave them (`person_hash`), on the card (`HASH_LINE`,
//!   with the mark it belongs to: a card bound by two sets keeps one each)
//!   and in this device's notes (`noted_path`), for a server that drops the
//!   line. While the service does not change a person, their card is left as
//!   it is, whatever the person did to it.

use super::reply::{ContactSet, Person};
use crate::phones::Region;
use crate::{contacts, lines, vdir};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The line that binds a card to a person of an extension: its value the mark.
pub const MARK_LINE: &str = "X-SIOUL-EXTENSION";
/// The line that keeps, on a card, the hash of the person as the extension
/// gave them, after their mark: `X-SIOUL-EXTENSION-HASH:<mark> <hash>`.
pub const HASH_LINE: &str = "X-SIOUL-EXTENSION-HASH";
/// The most a card the door makes weighs.
pub const MOST_FILE_BYTES: usize = 256 << 10;
/// The most people of one extension this device notes (`noted_path`): eight
/// sets of the most people a set holds.
pub const MOST_NOTED: usize = 160_000;
/// A file of an address book larger than this is never read here: neither
/// found marked nor bound (a card with a very large photo).
const READ_MOST: u64 = 1 << 20;

/// Why a set of contacts was not written: one word, said by the window in
/// its own sentence and in the extension's log, never with anything the
/// extension wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// The address book chosen is gone, or can no longer be written (read
    /// only, refused by its server): nothing written; choose another.
    BookGone,
    /// A card the door would make weighs more than it may.
    TooLarge,
}

impl Problem {
    pub fn word(self) -> &'static str {
        match self {
            Self::BookGone => "book-gone",
            Self::TooLarge => "too-large",
        }
    }
}

/// What a refresh did to the address books.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Done {
    /// The people the set holds.
    pub people: usize,
    /// New cards made in the chosen book.
    pub made: usize,
    /// Cards of the person's own found the same person and bound.
    pub bound: usize,
    /// Cards already marked that gained something.
    pub updated: usize,
    /// Cards left as they are: the person unchanged at the service, or
    /// nothing missing on the card.
    pub unchanged: usize,
    /// Marked cards in an address book the person only reads now: left.
    pub left: usize,
    /// Cards the contacts' door refused, or that could not be read or written.
    pub refused: usize,
    /// A person whose card's file name is taken by a card that is not theirs: left out.
    pub clash: usize,
}

/// Whether an address book may receive an extension's contacts: one of the
/// person's own that can be written, its folder there, its server not
/// having refused to make it.
pub fn writable(book: &vdir::Collection) -> bool {
    book.kind == vdir::Kind::Contacts && !book.read_only && book.dir.is_dir() && !vdir::State::load(&book.state_path()).refused
}

/// The address book a place names ("account/id"), when this device holds it.
pub fn book_at(place: &str) -> Option<vdir::Collection> {
    crate::addressbooks::find(place)
}

/// A hash of the person as the extension gave them: what a refresh compares.
fn person_hash(person: &Person) -> String {
    let said = serde_json::json!(["1", person.name, person.given, person.family, person.phones, person.emails, person.org, person.note]).to_string();
    vdir::content_hash(said.as_bytes())
}

/// A text's hash, as this device's notes keep a name the door wrote (never the name itself).
fn said(text: &str) -> String {
    vdir::content_hash(text.as_bytes())
}

/// The given and family names, as a hash, as the notes keep them.
fn parts_said(family: &str, given: &str) -> String {
    said(&format!("{}\u{1f}{}", family.trim(), given.trim()))
}

/// A name's words, whatever their order: lower case, accents and punctuation aside.
fn name_words(name: &str) -> Vec<String> {
    let folded: String = crate::text::fold(name).into_iter().collect::<String>().to_lowercase();
    let mut words: Vec<String> = folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect();
    words.sort();
    words
}

/// A card's lines, when its file is one this door reads.
fn read_card(path: &Path) -> Option<Vec<String>> {
    if std::fs::metadata(path).map_or(true, |m| !m.is_file() || m.len() > READ_MOST) {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let card = lines::unfold(&text);
    card.iter().any(|l| lines::name(l) == "BEGIN").then_some(card)
}

/// A card's UID, as its first `UID` line says.
fn uid_of(card: &[String]) -> String {
    card.iter().find(|l| lines::name(l) == "UID").map(|l| lines::value(l).trim().to_string()).unwrap_or_default()
}

/// The hash a card keeps for `mark` (`HASH_LINE`); none when it keeps none.
fn hash_on(card: &[String], mark: &str) -> Option<String> {
    card.iter().filter(|l| lines::raw_name(l) == HASH_LINE).find_map(|l| {
        let (of, hash) = lines::value(l).trim().split_once(' ')?;
        (of == mark && !hash.trim().is_empty()).then(|| hash.trim().to_string())
    })
}

/// The numbers of a card, keyed (`phones::key`).
fn phone_keys(card: &[String], region: Option<&Region>) -> Vec<String> {
    card.iter().filter(|l| lines::name(l) == "TEL").map(|l| contacts::shown(l)).filter(|v| !v.is_empty()).map(|v| crate::phones::key(&v, region)).collect()
}

/// The e-mail addresses of a card, case aside.
fn email_keys(card: &[String]) -> Vec<String> {
    card.iter().filter(|l| lines::name(l) == "EMAIL").map(|l| crate::duplicates::email_key(&contacts::shown(l))).filter(|v| !v.is_empty()).collect()
}

/// A card of the set's found in an address book.
#[derive(Debug, Clone)]
struct Held {
    path: PathBuf,
    /// Its address book can be written.
    writable: bool,
    /// The hash it keeps for its mark.
    hash: Option<String>,
}

/// A card of the chosen book that no person of the set holds yet: what a
/// person may be found by in it.
#[derive(Debug, Clone)]
struct Candidate {
    path: PathBuf,
    words: Vec<String>,
}

/// What the address books hold for a set: its cards by mark, in any book
/// (the chosen book's first, so that a mark on two cards is the one there),
/// and the chosen book's other cards with the numbers and addresses they
/// may be found by, in file order.
#[derive(Debug, Default)]
struct Found {
    held: BTreeMap<String, Held>,
    candidates: Vec<Candidate>,
    by_phone: BTreeMap<String, Vec<usize>>,
    by_email: BTreeMap<String, Vec<usize>>,
}

fn scan(extension: &str, set: &str, chosen: &vdir::Collection, region: Option<&Region>) -> Found {
    let key = super::set_key(extension, set);
    let ours = |mark: &str| super::uid_mark(mark).is_some_and(|m| m.extension == extension && m.agenda == key);
    let mut books = vdir::collections(vdir::Kind::Contacts);
    books.sort_by_key(|b| b.dir != chosen.dir);
    let mut found = Found::default();
    for book in books {
        let can_write = writable(&book);
        let is_chosen = book.dir == chosen.dir;
        for path in book.items() {
            let Some(card) = read_card(&path) else { continue };
            let uid = uid_of(&card);
            let mut marks: Vec<String> = card.iter().filter(|l| lines::raw_name(l) == MARK_LINE).map(|l| lines::value(l).trim().to_string()).chain(std::iter::once(uid)).filter(|m| ours(m)).collect();
            marks.dedup();
            if !marks.is_empty() {
                for mark in marks {
                    let hash = hash_on(&card, &mark);
                    found.held.entry(mark).or_insert_with(|| Held { path: path.clone(), writable: can_write, hash });
                }
            } else if is_chosen {
                let at = found.candidates.len();
                for phone in phone_keys(&card, region).into_iter().filter(|k| crate::phones::is_whole(k)) {
                    let list = found.by_phone.entry(phone).or_default();
                    if list.last() != Some(&at) {
                        list.push(at);
                    }
                }
                for email in email_keys(&card) {
                    let list = found.by_email.entry(email).or_default();
                    if list.last() != Some(&at) {
                        list.push(at);
                    }
                }
                found.candidates.push(Candidate { path, words: name_words(&contacts::display_name(&card)) });
            }
        }
    }
    found
}

/// The card of the chosen book that is this person, by proof alone (the
/// module's step 2), among those no other person took in this refresh.
fn pick(found: &Found, taken: &BTreeSet<usize>, person: &Person, region: Option<&Region>) -> Option<usize> {
    let gather = |index: &BTreeMap<String, Vec<usize>>, keys: Vec<String>| {
        let mut all: Vec<usize> = keys.iter().filter_map(|k| index.get(k)).flatten().copied().filter(|i| !taken.contains(i)).collect();
        all.sort_unstable();
        all.dedup();
        all
    };
    let phones = person.phones.iter().map(|p| crate::phones::key(p, region)).filter(|k| crate::phones::is_whole(k)).collect();
    let mut matched = gather(&found.by_phone, phones);
    if matched.is_empty() {
        matched = gather(&found.by_email, person.emails.iter().map(|e| crate::duplicates::email_key(e)).filter(|e| !e.is_empty()).collect());
    }
    let words = name_words(&person.name);
    matched.iter().copied().find(|i| found.candidates[*i].words == words).or_else(|| matched.first().copied())
}

/// What this device keeps of one person's card: the hash of the person as
/// last taken in, and hashes of the name (`FN`) and of the given and family
/// names (`N`) the door wrote last on the card; "" when it wrote none (a card
/// it only bound keeps its person's own names).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    #[serde(default)]
    pub hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parts: String,
}

/// This device's notes of an extension's cards, by mark
/// (`$XDG_STATE_HOME/sioul/extensions/<id>.contacts.toml`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Noted {
    #[serde(default)]
    pub cards: BTreeMap<String, Note>,
}

/// Where this device notes an extension's cards.
pub fn noted_path(extension: &str) -> PathBuf {
    crate::config::state_dir().join("extensions").join(format!("{extension}.contacts.toml"))
}

fn load_noted(extension: &str) -> Noted {
    std::fs::read_to_string(noted_path(extension)).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
}

/// The notes written, `MOST_NOTED` at most (beyond, the cards' own lines stand alone).
fn save_noted(extension: &str, noted: &Noted) -> Result<(), String> {
    let path = noted_path(extension);
    let kept = Noted { cards: noted.cards.iter().take(MOST_NOTED).map(|(k, v)| (k.clone(), v.clone())).collect() };
    let text = toml::to_string(&kept).map_err(|e| e.to_string())?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    crate::filelock::replace(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// A new card for a person (the module's step 3), in vCard 3.0, which every
/// server and phone reads, and what this device notes of it.
fn new_card(person: &Person, mark: &str, hash: &str, category: &str, region: Option<&Region>) -> (String, Note) {
    let mut card = vec!["BEGIN:VCARD".to_string(), "VERSION:3.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), format!("UID:{mark}"), format!("FN:{}", lines::escape(&person.name))];
    let mut note = Note { hash: hash.to_string(), name: said(&person.name), parts: String::new() };
    if !person.given.is_empty() || !person.family.is_empty() {
        card.push(format!("N:{}", lines::structured(&[person.family.clone(), person.given.clone(), String::new(), String::new(), String::new()])));
        note.parts = parts_said(&person.family, &person.given);
    }
    let mut keys: Vec<String> = Vec::new();
    for phone in &person.phones {
        let key = crate::phones::key(phone, region);
        if !keys.contains(&key) {
            card.push(format!("TEL:{}", lines::escape(phone)));
            keys.push(key);
        }
    }
    let mut addresses: Vec<String> = Vec::new();
    for email in &person.emails {
        let key = crate::duplicates::email_key(email);
        if !addresses.contains(&key) {
            card.push(format!("EMAIL;TYPE=INTERNET:{}", lines::escape(email)));
            addresses.push(key);
        }
    }
    if !person.org.is_empty() {
        card.push(format!("ORG:{}", lines::escape(&person.org)));
    }
    if !person.note.is_empty() {
        card.push(format!("NOTE:{}", lines::escape(&person.note)));
    }
    card.push(contacts::categories_line(&[category.to_string()]));
    card.push(format!("{MARK_LINE}:{mark}"));
    card.push(format!("{HASH_LINE}:{mark} {hash}"));
    card.push(format!("REV:{}", contacts::now_stamp()));
    card.push("END:VCARD".to_string());
    (lines::fold(&card), note)
}

/// What the door writes on a card it found or made (the module's step 4):
/// the mark when the card lacks it; numbers and addresses it lacks; `ORG` and
/// `NOTE` where it has none; `FN` and `N` following the extension's while
/// the card still holds what the door wrote last (`note`); the hash of the
/// person. Every other line byte for byte, `REV` renewed; nothing removed.
/// The card's text when something besides the hash changes (none: nothing
/// to write), and what this device notes of it now.
fn completed(text: &str, person: &Person, mark: &str, hash: &str, note: Option<&Note>, region: Option<&Region>) -> (Option<String>, Note) {
    let card = lines::unfold(text);
    let mut out: Vec<Option<String>> = card.iter().cloned().map(Some).collect();
    let mut added: Vec<String> = Vec::new();
    let mut changed = false;
    // A card the door made: its UID is the mark. Its names are the door's while they read as the extension's.
    let made = uid_of(&card) == mark;
    let mut kept = Note { hash: hash.to_string(), ..note.cloned().unwrap_or_default() };
    if !card.iter().any(|l| lines::raw_name(l) == MARK_LINE && lines::value(l).trim() == mark) {
        added.push(format!("{MARK_LINE}:{mark}"));
        changed = true;
    }
    // FN follows the extension's name while it is the one the door wrote last; a card without one gets it.
    match card.iter().position(|l| lines::name(l) == "FN") {
        Some(at) => {
            let now = contacts::shown(&card[at]);
            if now == person.name {
                if made || !kept.name.is_empty() {
                    kept.name = said(&person.name);
                }
            } else if !kept.name.is_empty() && said(&now) == kept.name {
                let value = lines::value(&card[at]);
                out[at] = Some(format!("{}{}", &card[at][..card[at].len() - value.len()], lines::escape(&person.name)));
                kept.name = said(&person.name);
                changed = true;
            }
        }
        None => {
            added.push(format!("FN:{}", lines::escape(&person.name)));
            kept.name = said(&person.name);
            changed = true;
        }
    }
    // N likewise, from the given and family names alone; a card without one gets it.
    if !person.given.is_empty() || !person.family.is_empty() {
        let wanted = parts_said(&person.family, &person.given);
        match contacts::n_parts(&card) {
            None => {
                added.push(format!("N:{}", lines::structured(&[person.family.clone(), person.given.clone(), String::new(), String::new(), String::new()])));
                kept.parts = wanted;
                changed = true;
            }
            Some(parts) => {
                let now = parts_said(&parts[0], &parts[1]);
                if now == wanted {
                    if made || !kept.parts.is_empty() {
                        kept.parts = wanted;
                    }
                } else if !kept.parts.is_empty() && now == kept.parts
                    && let Some(at) = card.iter().position(|l| lines::name(l) == "N")
                {
                    let mut parts = parts;
                    parts[0] = person.family.clone();
                    parts[1] = person.given.clone();
                    out[at] = Some(contacts::n_line(&card, &parts));
                    kept.parts = wanted;
                    changed = true;
                }
            }
        }
    }
    // A number or an address the card lacks, by the same keys.
    let mut keys = phone_keys(&card, region);
    for phone in &person.phones {
        let key = crate::phones::key(phone, region);
        if !keys.contains(&key) {
            added.push(format!("TEL:{}", lines::escape(phone)));
            keys.push(key);
            changed = true;
        }
    }
    let mut addresses = email_keys(&card);
    for email in &person.emails {
        let key = crate::duplicates::email_key(email);
        if !addresses.contains(&key) {
            added.push(format!("EMAIL;TYPE=INTERNET:{}", lines::escape(email)));
            addresses.push(key);
            changed = true;
        }
    }
    // ORG and NOTE where the card has none: an empty line of theirs holds nothing, and takes the value.
    for (property, value) in [("ORG", &person.org), ("NOTE", &person.note)] {
        let present: Vec<usize> = card.iter().enumerate().filter(|(_, l)| lines::name(l) == property).map(|(i, _)| i).collect();
        if value.is_empty() || present.iter().any(|&i| !contacts::shown(&card[i]).is_empty()) {
            continue;
        }
        let line = format!("{property}:{}", lines::escape(value));
        match present.first() {
            Some(&i) => out[i] = Some(line),
            None => added.push(line),
        }
        changed = true;
    }
    if !changed {
        return (None, kept);
    }
    // The hash of this mark: in the place of the one it kept, once; another mark's left as it is.
    let line = format!("{HASH_LINE}:{mark} {hash}");
    let ours: Vec<usize> = card.iter().enumerate().filter(|(_, l)| lines::raw_name(l) == HASH_LINE && lines::value(l).trim().split(' ').next() == Some(mark)).map(|(i, _)| i).collect();
    for &i in &ours {
        out[i] = None;
    }
    match ours.first() {
        Some(&i) => out[i] = Some(line),
        None => added.push(line),
    }
    (Some(contacts::rewritten(out, added)), kept)
}

/// What a refresh does for one person, planned before anything is written.
enum Act {
    /// Their card is as the service last gave them.
    Unchanged,
    /// Their card is in an address book the person only reads now.
    Left,
    /// Their card found, the person changed at the service.
    Update(PathBuf),
    /// A card of the chosen book found to be them (an index of `Found::candidates`).
    Bind(usize),
    /// A new card, its file, its text and what this device notes of it.
    Make(PathBuf, String, Note),
    /// The file their new card would take holds another card.
    Clash,
}

/// A set of contacts written through the contacts' door (the module's
/// rules) into `book`, numbers keyed in `region` (the Contacts settings'
/// country, `reach::region`). The book is checked and every card the door
/// would make weighed before anything is written; past a bound, nothing is.
/// Nothing at all is written, this device's notes included, when nothing differs.
pub fn take(extension: &str, set: &ContactSet, book: &vdir::Collection, region: Option<&Region>) -> Result<Done, Problem> {
    if !writable(book) {
        return Err(Problem::BookGone);
    }
    let category = super::category(extension);
    let found = scan(extension, &set.id, book, region);
    let mut noted = load_noted(extension);
    let mut taken = BTreeSet::new();
    let mut acts: Vec<(String, String, Act)> = Vec::with_capacity(set.people.len());
    for person in &set.people {
        let mark = super::person_mark(extension, &set.id, &person.id);
        let hash = person_hash(person);
        let act = if let Some(there) = found.held.get(&mark) {
            let known = there.hash.as_deref() == Some(hash.as_str()) || noted.cards.get(&mark).is_some_and(|n| n.hash == hash);
            if known {
                Act::Unchanged
            } else if !there.writable {
                Act::Left
            } else {
                Act::Update(there.path.clone())
            }
        } else if let Some(at) = pick(&found, &taken, person, region) {
            taken.insert(at);
            Act::Bind(at)
        } else {
            let path = book.dir.join(super::card_file(&mark));
            if path.exists() {
                Act::Clash
            } else {
                let (text, note) = new_card(person, &mark, &hash, &category, region);
                if text.len() > MOST_FILE_BYTES {
                    return Err(Problem::TooLarge);
                }
                Act::Make(path, text, note)
            }
        };
        acts.push((mark, hash, act));
    }
    let mut done = Done { people: set.people.len(), ..Done::default() };
    let mut noted_changed = false;
    for ((mark, hash, act), person) in acts.into_iter().zip(&set.people) {
        let (path, binding) = match act {
            Act::Unchanged => {
                done.unchanged += 1;
                continue;
            }
            Act::Left => {
                done.left += 1;
                continue;
            }
            Act::Clash => {
                done.clash += 1;
                continue;
            }
            Act::Make(path, text, note) => {
                match vdir::write_item(&path, &text) {
                    Ok(()) => {
                        done.made += 1;
                        noted.cards.insert(mark, note);
                        noted_changed = true;
                    }
                    Err(why) => {
                        eprintln!("sioul: extensions: {extension}: a card not made: {why}");
                        done.refused += 1;
                    }
                }
                continue;
            }
            Act::Update(path) => (path, false),
            Act::Bind(at) => (found.candidates[at].path.clone(), true),
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            done.refused += 1;
            continue;
        };
        let (written, kept) = completed(&text, person, &mark, &hash, noted.cards.get(&mark), region);
        match written {
            None => {
                done.unchanged += 1;
                if noted.cards.get(&mark) != Some(&kept) {
                    noted.cards.insert(mark, kept);
                    noted_changed = true;
                }
            }
            Some(new) => match vdir::write_item(&path, &new) {
                Ok(()) => {
                    if binding { done.bound += 1 } else { done.updated += 1 }
                    noted.cards.insert(mark, kept);
                    noted_changed = true;
                }
                Err(why) => {
                    eprintln!("sioul: extensions: {extension}: a card not written: {why}");
                    done.refused += 1;
                }
            },
        }
    }
    if noted_changed && let Err(why) = save_noted(extension, &noted) {
        eprintln!("sioul: extensions: {extension}: its notes of the cards: {why}");
    }
    Ok(done)
}

/// The cards the door made for one set (their UID of the mark's form) moved
/// from `from` into `to`, another address book chosen for it, each through
/// `vdir::move_item` (its text and UID as they are, the sync sending both
/// changes). Cards the door only bound stay where their person keeps them.
/// One whose name or UID `to` holds already is left. How many moved, and how
/// many were left or refused.
pub fn move_made(from: &vdir::Collection, to: &vdir::Collection, extension: &str, set: &str) -> (usize, usize) {
    if from.dir == to.dir || from.read_only || !writable(to) {
        return (0, 0);
    }
    let key = super::set_key(extension, set);
    let mut uids: BTreeSet<String> = to.items().iter().filter_map(|p| read_card(p)).map(|c| uid_of(&c)).collect();
    let (mut moved, mut refused) = (0, 0);
    for path in from.items() {
        let Some(card) = read_card(&path) else { continue };
        let uid = uid_of(&card);
        if super::uid_mark(&uid).is_none_or(|m| m.extension != extension || m.agenda != key) {
            continue;
        }
        let target = to.dir.join(super::card_file(&uid));
        if target.exists() || uids.contains(&uid) {
            refused += 1;
            continue;
        }
        match vdir::move_item(&path, &target) {
            Ok(()) => {
                moved += 1;
                uids.insert(uid);
            }
            Err(why) => {
                eprintln!("sioul: extensions: {extension}: a card not moved: {why}");
                refused += 1;
            }
        }
    }
    (moved, refused)
}

/// Remove: this device's notes of the extension's cards go; every card stays as it is.
pub fn forget(extension: &str) {
    let _ = std::fs::remove_file(noted_path(extension));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::{card_file, person_mark, reply::refresh_reply};
    use crate::vdir::{Kind, LOCAL};
    use serde_json::json;

    /// Numbers written without their country are French ones here.
    fn fr() -> Option<&'static Region> {
        crate::phones::region_named("FR")
    }

    fn book(name: &str) -> vdir::Collection {
        crate::config::stand_ins_only();
        vdir::create(Kind::Contacts, LOCAL, name, None, &[]).unwrap()
    }

    /// A set as an extension hands it back, read as Sioul reads it.
    fn set(people: serde_json::Value) -> ContactSet {
        refresh_reply(&json!({ "contacts": [{ "id": "clients", "name": "Clients", "people": people }] }).to_string()).unwrap().contacts.remove(0)
    }

    /// Every file of a folder, by name, with its bytes.
    fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.path().is_file()).map(|e| (e.file_name().to_string_lossy().to_string(), std::fs::read(e.path()).unwrap())).collect()
    }

    /// The cards of a folder, by name.
    fn cards(dir: &Path) -> Vec<String> {
        files(dir).into_keys().filter(|n| n.ends_with(".vcf")).collect()
    }

    /// A card's lines unfolded, each ending its line; "" when there is none.
    fn text_at(path: &Path) -> String {
        lines::unfold(&std::fs::read_to_string(path).unwrap_or_default()).iter().map(|l| format!("{l}\r\n")).collect()
    }

    /// A card of the person's own, written as another program would.
    fn own(book: &vdir::Collection, file: &str, lines: &[&str]) -> PathBuf {
        let path = book.dir.join(file);
        std::fs::write(&path, format!("BEGIN:VCARD\r\nVERSION:3.0\r\n{}\r\nEND:VCARD\r\n", lines.join("\r\n"))).unwrap();
        path
    }

    /// The file of the card the door made for a person.
    fn made_at(book: &vdir::Collection, extension: &str, id: &str) -> PathBuf {
        book.dir.join(card_file(&person_mark(extension, "clients", id)))
    }

    /// A new card carries every field, written by Sioul (the extension's own
    /// id nowhere), reads back as Sioul reads a contact; the same reply again
    /// writes nothing, this device's notes neither; a person given one name
    /// alone gets `FN` alone, no `N` guessed from it.
    #[test]
    fn a_new_card_carries_every_field_and_the_same_reply_writes_nothing() {
        const EXT: &str = "cards-new";
        let book = book("Cards new");
        let reply = set(json!([
            { "id": "client-41", "name": "Marie Exemple", "given": "Marie", "family": "Exemple", "phones": ["+33 1 23 45 67 89", "01 23 45 67 89"], "emails": ["marie@example.org"], "org": "Exemple; SARL", "note": "Likes tea.\nAnd cake." },
            { "id": "client-42", "name": "EXEMPLE Jean" }
        ]));
        let done = take(EXT, &reply, &book, fr()).unwrap();
        assert_eq!((done.people, done.made, done.bound, done.refused), (2, 2, 0, 0));
        let mark = person_mark(EXT, "clients", "client-41");
        let path = made_at(&book, EXT, "client-41");
        let text = text_at(&path);
        for line in [
            format!("UID:{mark}\r\n"),
            "FN:Marie Exemple\r\n".into(),
            "N:Exemple;Marie;;;\r\n".into(),
            "TEL:+33 1 23 45 67 89\r\n".into(),
            "EMAIL;TYPE=INTERNET:marie@example.org\r\n".into(),
            "ORG:Exemple\\; SARL\r\n".into(),
            "NOTE:Likes tea.\\nAnd cake.\r\n".into(),
            "CATEGORIES:Cards-new\r\n".into(),
            format!("{MARK_LINE}:{mark}\r\n"),
            format!("{HASH_LINE}:{mark} "),
        ] {
            assert!(text.contains(&line), "{line} missing:\n{text}");
        }
        assert_eq!(text.matches("TEL").count(), 1, "one number written two ways, once:\n{text}");
        assert!(!text.contains("client-41"), "the extension's own id is never written");
        let contact = contacts::read(&path, &book).unwrap();
        assert_eq!((contact.name.as_str(), contact.org.as_str(), contact.notes.as_str(), contact.given.as_str(), contact.family.as_str()), ("Marie Exemple", "Exemple; SARL", "Likes tea.\nAnd cake.", "Marie", "Exemple"));
        assert_eq!(contact.categories, vec!["Cards-new".to_string()]);
        let plain = text_at(&made_at(&book, EXT, "client-42"));
        assert!(plain.contains("FN:EXEMPLE Jean\r\n") && !plain.contains("\r\nN:"), "no name guessed from one string:\n{plain}");
        // The same reply again: nothing written, not even this device's notes.
        let (before, notes) = (files(&book.dir), std::fs::read(noted_path(EXT)).unwrap());
        let again = take(EXT, &reply, &book, fr()).unwrap();
        assert_eq!((again.made, again.updated, again.unchanged), (0, 0, 2));
        assert_eq!(files(&book.dir), before);
        assert_eq!(std::fs::read(noted_path(EXT)).unwrap(), notes);
    }

    /// A card of the person's own with the same number in another spelling
    /// is bound: it gains the mark and the address it lacked, keeps its name,
    /// its UID, its own lines and its number as written; no card is made. Its
    /// name never follows the service's.
    #[test]
    fn a_card_with_the_same_number_is_bound_and_keeps_its_own() {
        const EXT: &str = "cards-bind";
        let book = book("Cards bind");
        let mine = own(&book, "own-1.vcf", &["UID:own-1@example.org", "FN:Marie E.", "TEL;TYPE=CELL:01.23.45.67.89", "X-OTHER-APP:kept"]);
        let reply = set(json!([{ "id": "c-1", "name": "Marie Exemple", "given": "Marie", "family": "Exemple", "phones": ["+33 1 23 45 67 89"], "emails": ["marie@example.org"], "org": "Salon" }]));
        let done = take(EXT, &reply, &book, fr()).unwrap();
        assert_eq!((done.bound, done.made), (1, 0));
        assert_eq!(cards(&book.dir), vec!["own-1.vcf".to_string()], "no card made beside it");
        let text = text_at(&mine);
        let mark = person_mark(EXT, "clients", "c-1");
        for line in ["UID:own-1@example.org\r\n", "FN:Marie E.\r\n", "TEL;TYPE=CELL:01.23.45.67.89\r\n", "X-OTHER-APP:kept\r\n", "EMAIL;TYPE=INTERNET:marie@example.org\r\n", "ORG:Salon\r\n", "N:Exemple;Marie;;;\r\n", format!("{MARK_LINE}:{mark}\r\n").as_str()] {
            assert!(text.contains(line), "{line} missing:\n{text}");
        }
        assert_eq!(text.matches("TEL").count(), 1, "its number not written again:\n{text}");
        assert!(!text.contains("CATEGORIES"), "a card of the person's own keeps its groups");
        // The service renames her: the person's own name stays; nothing else of the card changes.
        let renamed = set(json!([{ "id": "c-1", "name": "Marie Exemple-Martin", "phones": ["+33 1 23 45 67 89"], "emails": ["marie@example.org"], "org": "Salon" }]));
        take(EXT, &renamed, &book, fr()).unwrap();
        assert!(text_at(&mine).contains("FN:Marie E.\r\n"));
    }

    /// A name alone never binds: the person's card of the same name, with
    /// other numbers and addresses, stays byte for byte and a new card is
    /// made; nor does a short code both share.
    #[test]
    fn a_name_alone_never_binds() {
        const EXT: &str = "cards-name";
        let book = book("Cards name");
        let same_name = own(&book, "own-1.vcf", &["UID:own-1@example.org", "FN:Marie Exemple", "TEL:06 11 11 11 11", "EMAIL:other@example.org"]);
        let short = own(&book, "own-2.vcf", &["UID:own-2@example.org", "FN:Help line", "TEL:3631"]);
        let (first, second) = (std::fs::read(&same_name).unwrap(), std::fs::read(&short).unwrap());
        let reply = set(json!([{ "id": "c-1", "name": "Marie Exemple", "phones": ["07 22 22 22 22", "3631"], "emails": ["marie@example.org"] }]));
        let done = take(EXT, &reply, &book, fr()).unwrap();
        assert_eq!((done.made, done.bound), (1, 0));
        assert_eq!((std::fs::read(&same_name).unwrap(), std::fs::read(&short).unwrap()), (first, second));
        assert!(made_at(&book, EXT, "c-1").exists());
    }

    /// Two cards with the number: the one whose name has the same words in
    /// another order and case is bound; the next person of the set with that
    /// number takes the other; another extension finding none of its name
    /// takes the first by file name, and the card keeps a hash for each mark.
    #[test]
    fn two_cards_with_the_number_the_name_s_words_choose() {
        const EXT: &str = "cards-words";
        let book = book("Cards words");
        let jean = own(&book, "a-1.vcf", &["UID:a-1@example.org", "FN:Jean Exemple", "TEL:01 23 45 67 89"]);
        let marie = own(&book, "b-2.vcf", &["UID:b-2@example.org", "FN:EXEMPLE, Marie", "TEL:+33123456789"]);
        let reply = set(json!([
            { "id": "c-1", "name": "Marie Exemple", "phones": ["0123456789"] },
            { "id": "c-2", "name": "Jean Exemple", "phones": ["0033 1 23 45 67 89"] }
        ]));
        let done = take(EXT, &reply, &book, fr()).unwrap();
        assert_eq!((done.bound, done.made), (2, 0));
        assert!(text_at(&marie).contains(&format!("{MARK_LINE}:{}", person_mark(EXT, "clients", "c-1"))));
        assert!(text_at(&jean).contains(&format!("{MARK_LINE}:{}", person_mark(EXT, "clients", "c-2"))));
        // Another extension's person of that number, whose name neither card has: the first by file name.
        let other = set(json!([{ "id": "p-9", "name": "Paul Exemple", "phones": ["01 23 45 67 89"] }]));
        assert_eq!(take("cards-words-other", &other, &book, fr()).unwrap().bound, 1);
        let text = text_at(&jean);
        assert!(text.contains(&format!("{MARK_LINE}:{}", person_mark("cards-words-other", "clients", "p-9"))), "{text}");
        assert_eq!(text.matches(&format!("{HASH_LINE}:")).count(), 2, "a hash for each mark:\n{text}");
        // Each refresh again finds its own card, changes nothing.
        assert_eq!(take(EXT, &reply, &book, fr()).unwrap().unchanged, 2);
        assert_eq!(take("cards-words-other", &other, &book, fr()).unwrap().unchanged, 1);
    }

    /// Additive only: a number the service dropped stays on the card; one it
    /// adds is added; the card's own note is never replaced by the service's.
    #[test]
    fn a_number_dropped_by_the_service_stays() {
        const EXT: &str = "cards-dropped";
        let book = book("Cards dropped");
        let path = made_at(&book, EXT, "c-1");
        take(EXT, &set(json!([{ "id": "c-1", "name": "Marie Exemple", "phones": ["01 23 45 67 89", "06 12 34 56 78"], "note": "First note." }])), &book, fr()).unwrap();
        let dropped = take(EXT, &set(json!([{ "id": "c-1", "name": "Marie Exemple", "phones": ["01 23 45 67 89"], "note": "Another note." }])), &book, fr()).unwrap();
        assert_eq!((dropped.updated, dropped.unchanged), (0, 1), "nothing to add: nothing written");
        let text = text_at(&path);
        assert!(text.contains("TEL:06 12 34 56 78\r\n") && text.contains("NOTE:First note.\r\n") && !text.contains("Another note"), "{text}");
        let added = take(EXT, &set(json!([{ "id": "c-1", "name": "Marie Exemple", "phones": ["01 23 45 67 89", "07 98 76 54 32"] }])), &book, fr()).unwrap();
        assert_eq!(added.updated, 1);
        let text = text_at(&path);
        assert!(text.contains("TEL:06 12 34 56 78\r\n") && text.contains("TEL:07 98 76 54 32\r\n") && text.contains("TEL:01 23 45 67 89\r\n"), "{text}");
    }

    /// A name the service changes follows on a card the door made, `N` too,
    /// not on one the person renamed since.
    #[test]
    fn a_name_changed_by_the_service_follows_on_a_made_card_alone() {
        const EXT: &str = "cards-renamed";
        let book = book("Cards renamed");
        take(EXT, &set(json!([{ "id": "c-1", "name": "Marie Exemple", "given": "Marie", "family": "Exemple" }, { "id": "c-2", "name": "Jean Exemple" }])), &book, fr()).unwrap();
        let (marie, jean) = (made_at(&book, EXT, "c-1"), made_at(&book, EXT, "c-2"));
        let by_hand = std::fs::read_to_string(&jean).unwrap().replace("FN:Jean Exemple", "FN:Jean-Pierre Exemple");
        std::fs::write(&jean, by_hand).unwrap();
        let done = take(EXT, &set(json!([{ "id": "c-1", "name": "Marie Exemple-Martin", "given": "Marie", "family": "Exemple-Martin" }, { "id": "c-2", "name": "Jean Exemple-Martin" }])), &book, fr()).unwrap();
        assert_eq!(done.updated, 1, "the card renamed by hand has nothing to gain");
        let text = text_at(&marie);
        assert!(text.contains("FN:Marie Exemple-Martin\r\n") && text.contains("N:Exemple-Martin;Marie;;;\r\n") && !text.contains("FN:Marie Exemple\r\n"), "{text}");
        assert!(text_at(&jean).contains("FN:Jean-Pierre Exemple\r\n"), "the person's name stays");
        // Renamed again at the service: it follows again, the door's name being the last it wrote.
        take(EXT, &set(json!([{ "id": "c-1", "name": "Marie Martin", "given": "Marie", "family": "Martin" }])), &book, fr()).unwrap();
        let text = text_at(&marie);
        assert!(text.contains("FN:Marie Martin\r\n") && text.contains("N:Martin;Marie;;;\r\n"), "{text}");
        assert_eq!(text.matches("FN:").count(), 1, "replaced in its place, never added beside:\n{text}");
    }

    /// Someone gone from the reply keeps their card as it is; a set with no
    /// one in it takes nothing out.
    #[test]
    fn someone_gone_from_the_reply_keeps_their_card() {
        const EXT: &str = "cards-gone";
        let book = book("Cards gone");
        take(EXT, &set(json!([{ "id": "c-1", "name": "Marie Exemple" }, { "id": "c-2", "name": "Jean Exemple" }])), &book, fr()).unwrap();
        let before = files(&book.dir);
        assert_eq!(take(EXT, &set(json!([{ "id": "c-1", "name": "Marie Exemple" }])), &book, fr()).unwrap().people, 1);
        assert_eq!(take(EXT, &set(json!([])), &book, fr()).unwrap(), Done::default());
        assert_eq!(files(&book.dir), before);
    }

    /// Another book chosen: the cards the door made move there, the cards it
    /// bound stay where their person keeps them, still bound, another set's
    /// made cards stay; the next refresh finds every card and writes nothing.
    /// Remove then leaves every card where it is.
    #[test]
    fn another_book_moves_the_made_cards_alone_and_remove_leaves_every_card() {
        const EXT: &str = "cards-moved";
        let first = book("Cards moved one");
        let bound = own(&first, "own-1.vcf", &["UID:own-1@example.org", "FN:Marie Exemple", "TEL:01 23 45 67 89"]);
        let reply = set(json!([{ "id": "c-1", "name": "Marie Exemple", "phones": ["01 23 45 67 89"] }, { "id": "c-2", "name": "Jean Exemple" }]));
        let done = take(EXT, &reply, &first, fr()).unwrap();
        assert_eq!((done.bound, done.made), (1, 1));
        let staff = ContactSet { id: "staff".into(), name: "Staff".into(), people: set(json!([{ "id": "s-1", "name": "Sam Exemple" }])).people };
        take(EXT, &staff, &first, fr()).unwrap();
        let second = book("Cards moved two");
        assert_eq!(move_made(&first, &second, EXT, "clients"), (1, 0));
        assert_eq!(cards(&second.dir), vec![card_file(&person_mark(EXT, "clients", "c-2"))]);
        assert!(bound.exists() && text_at(&bound).contains(MARK_LINE), "the bound card stays, still bound");
        assert!(first.dir.join(card_file(&person_mark(EXT, "staff", "s-1"))).exists(), "another set's card stays");
        let (one, two) = (files(&first.dir), files(&second.dir));
        let again = take(EXT, &reply, &second, fr()).unwrap();
        assert_eq!((again.made, again.bound, again.unchanged), (0, 0, 2), "each found where it is");
        assert_eq!((files(&first.dir), files(&second.dir)), (one.clone(), two.clone()));
        // Remove: this device's notes go; every card stays as it is.
        assert!(noted_path(EXT).exists());
        forget(EXT);
        assert!(!noted_path(EXT).exists());
        assert_eq!((files(&first.dir), files(&second.dir)), (one, two));
    }

    /// Bounds before any write: a card the door would make weighs more than
    /// it may, and nothing of the set is written, the small one neither.
    #[test]
    fn bounds_come_before_any_write() {
        const EXT: &str = "cards-bounds";
        let book = book("Cards bounds");
        let mut heavy = set(json!([{ "id": "c-1", "name": "Marie Exemple" }, { "id": "c-2", "name": "Jean Exemple" }]));
        heavy.people[1].note = "n".repeat(MOST_FILE_BYTES);
        assert_eq!(take(EXT, &heavy, &book, fr()), Err(Problem::TooLarge));
        assert!(cards(&book.dir).is_empty() && !noted_path(EXT).exists());
    }

    /// An address book that cannot be written (read only, refused by its
    /// server, gone): nothing written, and the problem says to choose again.
    #[test]
    fn a_book_that_cannot_be_written_takes_nothing() {
        const EXT: &str = "cards-closed";
        let reply = set(json!([{ "id": "c-1", "name": "Marie Exemple" }]));
        let read_only = book("Cards read only");
        vdir::State { read_only: true, ..vdir::State::default() }.save(&read_only.state_path()).unwrap();
        let read_only = book_at(&format!("{LOCAL}/{}", read_only.id)).unwrap();
        assert_eq!(take(EXT, &reply, &read_only, fr()), Err(Problem::BookGone));
        let refused = book("Cards refused");
        vdir::State { refused: true, ..vdir::State::default() }.save(&refused.state_path()).unwrap();
        assert_eq!(take(EXT, &reply, &refused, fr()), Err(Problem::BookGone));
        let gone = book("Cards gone away");
        std::fs::remove_dir_all(&gone.dir).unwrap();
        assert_eq!(take(EXT, &reply, &gone, fr()), Err(Problem::BookGone));
        assert!(cards(&read_only.dir).is_empty() && cards(&refused.dir).is_empty());
    }

    /// A server that drops Sioul's own lines: the card the door made is still
    /// found by its UID, the hash this device noted stands for the one it
    /// lost, nothing is written; a device without notes writes the mark back once.
    #[test]
    fn a_card_whose_server_dropped_sioul_s_lines_is_still_found() {
        const EXT: &str = "cards-stripped";
        let book = book("Cards stripped");
        let reply = set(json!([{ "id": "c-1", "name": "Marie Exemple", "phones": ["01 23 45 67 89"] }]));
        take(EXT, &reply, &book, fr()).unwrap();
        let path = made_at(&book, EXT, "c-1");
        let stripped = lines::fold(&lines::unfold(&std::fs::read_to_string(&path).unwrap()).into_iter().filter(|l| !l.starts_with("X-SIOUL")).collect::<Vec<_>>());
        std::fs::write(&path, &stripped).unwrap();
        let done = take(EXT, &reply, &book, fr()).unwrap();
        assert_eq!((done.unchanged, done.made, done.bound), (1, 0, 0));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), stripped);
        forget(EXT);
        assert_eq!(take(EXT, &reply, &book, fr()).unwrap().updated, 1, "another device writes the mark back");
        assert_eq!(cards(&book.dir).len(), 1);
        assert!(text_at(&path).contains(MARK_LINE));
    }

    /// The door writes inside the chosen book's folder alone, whatever ids
    /// the extension hands back: separators, dots, a dash first, a path upwards.
    #[test]
    fn the_door_cannot_write_outside_its_book_whatever_the_ids() {
        const EXT: &str = "cards-ids";
        let book = book("Cards ids");
        let ids = ["../../contacts", "a/b\\c.vcf", "..\\..\\x", "-dash", "x@y.z\r\nBEGIN:VCARD"];
        let reply = set(json!(ids.iter().map(|id| json!({ "id": id, "name": "Someone" })).collect::<Vec<_>>()));
        assert_eq!(take(EXT, &reply, &book, fr()).unwrap().made, ids.len());
        for name in cards(&book.dir) {
            assert!(name.starts_with("cards-ids-") && name.trim_end_matches(".vcf").bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'), "{name}");
        }
        let mut stack = vec![crate::config::data_dir()];
        while let Some(at) = stack.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if !path.starts_with(&book.dir) {
                    assert!(!entry.file_name().to_string_lossy().starts_with("cards-ids-"), "{}", path.display());
                }
            }
        }
    }
}
