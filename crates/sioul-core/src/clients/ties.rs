// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Ties between a client (or one row of its file) and the rest of Sioul,
//! both ways (docs/clients.md, "Ties"). **Each tie lives where the thing
//! that makes it keeps its own data**, never on a server's copy:
//!
//! | Tie | Kept in |
//! |---|---|
//! | a card | the client's record (`cards`) |
//! | an invoice, its card, a line's row | the invoice: `client_ref`, `contact`, a line's `row` |
//! | a project | the project's `client` |
//! | a supply used by a row | the supply's movement, `for` |
//! | an appointment, a task, a time entry, a message tied by hand | the client's tie files |
//! | a note's ties | the note's front matter |
//! | an appointment, by itself | a `CONTACT` or guest line naming a client's card; the extension's map |
//!
//! The client's tie files: one small file each, `ties/<tie>.toml`, named
//! from its two ends, so that two devices making one tie write one file:
//!
//! ```toml
//! format = 1
//! at = "sioul:client/3f2a…/db/treatment/7c41"   # the client, or one row
//! other = "sioul:event/<uid>"
//! how = "appointment"
//! day = "2026-11-03"           # what the event said, kept for when it is not here
//! title = "Facial"
//! made = "2026-11-24T21:40:00Z"
//!
//! [shown]                      # reserved: absent unless written (below)
//! title = "Facial, Marie Exemple"
//! place = ""
//! notes = ""
//! ```
//!
//! - **Every key this version does not know is kept**: a tie file is changed
//!   in place, key by key, never written whole over one that exists.
//! - **`[shown]`** holds the real words of an appointment whose event on its
//!   server says something neutral ("Appointment"): written by Sioul alone,
//!   never by `refresh_said`, shown over the event's title, erased with the
//!   client. Nothing writes it in this version; readers prefer it when there.
//! - **An event absent on this device is not gone**: a calendar may live on
//!   another device only. No sweep, merge or move removes or rewrites a tie
//!   because its event is not here; the view says "not on this device" from
//!   the tie's day and title. Only `untie` (the person's word) and
//!   `row_removed` (its row gone) touch ties; `refresh_said` only brings the
//!   day and title up to date from an event that is here.
//!
//! The index (`Index`) reads them all, in memory only, never written, and
//! answers from both ends.

use super::door::{Door, Expect, name_of};
use super::record::Client;
use super::{Error, Read};
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How a thing is tied to a client or to one of its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TieHow {
    /// One of its contact cards.
    Card,
    /// An invoice billing it.
    Invoice,
    /// An invoice's line billing one row.
    InvoiceLine,
    /// A project for it.
    Project,
    /// A supply's movement for a row.
    Supply,
    /// An appointment (an event).
    Appointment,
    Task,
    /// A time entry (`sioul:time/<session>`).
    Time,
    Message,
    /// A note of its file tied to a row or to something else.
    Note,
    /// Anything else tied by hand.
    #[serde(other)]
    Hand,
}

impl TieHow {
    /// How a tie made by hand to `other` reads, by the kind of its address.
    pub fn of(other: &str) -> TieHow {
        if other.starts_with("sioul:time/") {
            return TieHow::Time;
        }
        match crate::links::kind_of(other) {
            crate::links::Kind::Event => TieHow::Appointment,
            crate::links::Kind::Task => TieHow::Task,
            crate::links::Kind::Mail => TieHow::Message,
            crate::links::Kind::Invoice => TieHow::Invoice,
            crate::links::Kind::Project => TieHow::Project,
            crate::links::Kind::Contact => TieHow::Card,
            _ => TieHow::Hand,
        }
    }
}

/// Where a tie is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeptIn {
    /// The client's record (its cards).
    Record,
    /// One of the client's tie files.
    TieFile,
    /// An invoice's `client_ref` or `contact`.
    Invoice,
    /// An invoice's line's `row`.
    InvoiceLine,
    /// A project's `client`.
    Project,
    /// A supply's movement's `for`.
    SupplyMove,
    /// A note's front matter.
    NoteFront,
    /// A `CONTACT` or guest line on the event: someone wrote it, or an
    /// extension's door did for the person the extension said the event is with.
    EventCard,
}

/// One tie, seen from the client's side.
#[derive(Debug, Clone, PartialEq)]
pub struct Tie {
    /// The client's id.
    pub client: String,
    /// The client's address, or one row's.
    pub at: String,
    /// The thing's address.
    pub other: String,
    pub how: TieHow,
    pub kept: KeptIn,
    /// Its day: an appointment's, an invoice's, a row's.
    pub day: Option<Date>,
    /// What an appointment was called, kept in its tie file for when it is not here.
    pub title: String,
    /// The real words Sioul keeps for a neutral event (`[shown]`), when written.
    pub shown: Option<Shown>,
}

/// An appointment's real words, kept by Sioul over a neutral event (`[shown]`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shown {
    pub title: String,
    pub place: String,
    pub notes: String,
}

impl Tie {
    /// What it is called where it is shown: `[shown]`'s title when written, else what the event said.
    pub fn words(&self) -> &str {
        self.shown.as_ref().map(|s| s.title.as_str()).filter(|t| !t.trim().is_empty()).unwrap_or(&self.title)
    }

    /// The client's own address.
    pub fn client_uri(&self) -> String {
        super::address::client_uri(&self.client)
    }
}

/// What a tie file keeps of the other end: an appointment's day and title.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Said {
    pub day: Option<Date>,
    pub title: String,
}

/// A tie file's format.
pub const TIE_FORMAT: u32 = 1;

/// A tie file as read: the keys this version knows (every other key stays
/// in the file, which is only ever changed in place: `in_place`).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct TieFile {
    format: u32,
    at: String,
    other: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    how: Option<TieHow>,
    #[serde(skip_serializing_if = "String::is_empty")]
    day: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    made: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    shown: Option<Shown>,
}

/// A tie file's text changed in place, key by key: every key and table this
/// version does not know stays as written.
fn in_place(text: &str, path: &str, change: impl FnOnce(&mut toml_edit::DocumentMut)) -> Result<String, Error> {
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|_| Error::Damaged(path.to_string()))?;
    change(&mut doc);
    Ok(doc.to_string())
}

/// A tie file's name, made of its two ends: the same on every device.
pub fn tie_name(at: &str, other: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for part in [at, other] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    hash.finalize()[..16].iter().map(|b| format!("{b:02x}")).collect()
}

fn ties_folder(id: &str) -> String {
    format!("{}/ties", super::record::folder(id))
}

fn tie_path(id: &str, at: &str, other: &str) -> String {
    format!("{}/{}.toml", ties_folder(id), tie_name(at, other))
}

/// Where a tie starts: the client's address, or a row's of that client.
fn start(at: &super::address::Address) -> Result<(String, String), Error> {
    use super::address::Address as A;
    match at {
        A::Client { id } => Ok((id.clone(), at.uri())),
        A::Row { client, .. } => Ok((client.clone(), at.uri())),
        _ => Err(Error::Refused("a tie starts at a client or a row")),
    }
}

/// A tie made by hand (or by "Write the session"), kept in the client's tie
/// files: from the client or one of its rows to `other`. Made twice, one file.
pub fn tie(door: &dyn Door, at: &super::address::Address, other: &str, said: &Said) -> Result<(), Error> {
    let (client, at) = start(at)?;
    let other = other.trim();
    if other.is_empty() || other.contains(['\n', '\r']) || other.len() > 2048 {
        return Err(Error::Refused("address"));
    }
    super::record::read(door, &client)?.ok_or(Error::NotFound)?;
    let file = TieFile {
        format: TIE_FORMAT,
        at: at.clone(),
        other: other.to_string(),
        how: Some(TieHow::of(other)),
        day: said.day.map(|d| d.to_string()).unwrap_or_default(),
        title: said.title.replace(['\n', '\r'], " ").trim().to_string(),
        made: jiff::Timestamp::now().to_string(),
        shown: None,
    };
    // Made twice (here, or on another device first): the file there stays as it is.
    match door.write(&tie_path(&client, &at, other), toml::to_string(&file).map_err(|_| Error::Refused("tie"))?.as_bytes(), Expect::Absent) {
        Ok(_) | Err(Error::Changed) => {}
        Err(e) => return Err(e),
    }
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(())
}

/// A tie undone; false when it was not there.
pub fn untie(door: &dyn Door, at: &super::address::Address, other: &str) -> Result<bool, Error> {
    let (client, at) = start(at)?;
    let removed = door.remove(&tie_path(&client, &at, other.trim()))?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(removed)
}

/// What an appointment's tie keeps of it brought up to date (its day and
/// title as the event now says them), so that it is said when the event is gone.
pub fn refresh_said(door: &dyn Door, at: &super::address::Address, other: &str, said: &Said) -> Result<(), Error> {
    let (client, at) = start(at)?;
    // An empty word (an event read without its day or title) never blanks what the tie kept.
    if said.day.is_none() && said.title.trim().is_empty() {
        return Ok(());
    }
    let path = tie_path(&client, &at, other.trim());
    door.update(&path, &mut |now| {
        let Some(now) = now else { return Ok(None) };
        let text = std::str::from_utf8(now).map_err(|_| Error::Damaged(path.clone()))?;
        let Ok(file) = toml::from_str::<TieFile>(text) else { return Ok(None) };
        let (day, title) = (said.day.map(|d| d.to_string()).unwrap_or_default(), said.title.replace(['\n', '\r'], " ").trim().to_string());
        // A tie of a later format is read, never refreshed here (its keys may mean more).
        if file.day == day && file.title == title || file.format > TIE_FORMAT {
            return Ok(None);
        }
        // What the event says now; `[shown]` and every key not known here stay.
        in_place(text, &path, |doc| {
            doc["day"] = toml_edit::value(day.as_str());
            doc["title"] = toml_edit::value(title.as_str());
        })
        .map(|t| Some(t.into_bytes()))
    })?;
    Ok(())
}

/// The client's own tie files, those that do not open counted.
pub fn ties_of(door: &dyn Door, id: &str) -> Result<Read<Vec<Tie>>, Error> {
    if !super::address::valid_id(id) {
        return Err(Error::Refused("id"));
    }
    let mut out: Read<Vec<Tie>> = Read::default();
    for path in door.list(&ties_folder(id))? {
        if !name_of(&path).ends_with(".toml") {
            continue;
        }
        match door.read_text(&path) {
            Ok(Some((text, _))) => match toml::from_str::<TieFile>(&text) {
                // A tie file is the client's own: one naming another client is passed over.
                Ok(f) if super::address::Address::parse(&f.at).is_some_and(|a| a.client() == id) && !f.other.is_empty() => out.items.push(Tie {
                    client: id.to_string(),
                    how: f.how.unwrap_or_else(|| TieHow::of(&f.other)),
                    at: f.at,
                    other: f.other,
                    kept: KeptIn::TieFile,
                    day: f.day.parse().ok(),
                    title: f.title,
                    shown: f.shown,
                }),
                _ => out.damaged += 1,
            },
            Ok(None) => {}
            Err(Error::Damaged(_)) => out.damaged += 1,
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// A row taken out: its tie files move to the client as a whole, and the
/// notes about it become notes of the file (`row` emptied). An invoice's
/// line and a supply's movement keep naming it (the index ties them to the
/// client, "a record no longer there").
pub fn row_removed(door: &dyn Door, id: &str, row_uri: &str) -> Result<usize, Error> {
    let client = super::address::client_uri(id);
    let mut moved = 0;
    for path in door.list(&ties_folder(id))? {
        let Ok(Some((text, _))) = door.read_text(&path) else { continue };
        let Ok(file) = toml::from_str::<TieFile>(&text) else { continue };
        if file.at != row_uri {
            continue;
        }
        // The same file, its start the client: every other key kept. A tie of
        // the client to the same thing there already stays as it is.
        let moved_text = in_place(&text, &path, |doc| doc["at"] = toml_edit::value(client.as_str()))?;
        match door.write(&tie_path(id, &client, &file.other), moved_text.as_bytes(), Expect::Absent) {
            Ok(_) | Err(Error::Changed) => {}
            Err(e) => return Err(e),
        }
        door.remove(&path)?;
        moved += 1;
    }
    let relative = super::address::Address::parse(row_uri).and_then(|a| match a {
        super::address::Address::Row { kind, row, .. } => Some(format!("{kind}/{row}")),
        _ => None,
    });
    if let Some(relative) = relative {
        for head in super::file::notes(door, id)?.items.into_iter().filter(|n| n.row == relative) {
            if let Some(mut note) = super::file::note(door, id, &head.id)? {
                note.row.clear();
                let path = super::file::note_path(id, &note.id);
                door.write(&path, super::file::note_text(&note).as_bytes(), Expect::Generation(note.generation))?;
                moved += 1;
            }
        }
    }
    Ok(moved)
}

/// The tie files of `from` moved to client `to` (Same as…), each ending at
/// `to` or at the same row under `to`.
pub(crate) fn move_ties(door: &dyn Door, from: &str, to: &str) -> Result<usize, Error> {
    let mut moved = 0;
    for path in door.list(&ties_folder(from))? {
        let Ok(Some((text, _))) = door.read_text(&path) else { continue };
        let Ok(file) = toml::from_str::<TieFile>(&text) else { continue };
        let at = match super::address::Address::parse(&file.at) {
            Some(super::address::Address::Row { kind, row, .. }) => super::address::Address::Row { client: to.to_string(), kind, row },
            _ => super::address::Address::Client { id: to.to_string() },
        }
        .uri();
        // The same file under `to`, every other key kept; one there already stays.
        let moved_text = in_place(&text, &path, |doc| doc["at"] = toml_edit::value(at.as_str()))?;
        match door.write(&tie_path(to, &at, &file.other), moved_text.as_bytes(), Expect::Absent) {
            Ok(_) | Err(Error::Changed) => {}
            Err(e) => return Err(e),
        }
        door.remove(&path)?;
        moved += 1;
    }
    Ok(moved)
}

// ---------------------------------------------------------------- the index

/// What the index reads beyond the clients' own files.
#[derive(Clone, Copy, Default)]
pub struct Sources<'a> {
    pub contacts: &'a [crate::contacts::Contact],
    pub events: &'a [crate::agenda::EventRef],
    pub invoices: &'a [crate::invoice::Invoice],
    pub projects: &'a [crate::projects::Project],
    pub supply_moves: &'a [crate::supplies::Move],
    /// The profiles of `sioul-clients.toml`: a client's billing name while its profile is there.
    pub profiles: Option<&'a super::Clients>,
}

/// Every tie of every client, from both ends, in memory only.
#[derive(Debug, Default)]
pub struct Index {
    by_client: HashMap<String, Vec<Tie>>,
    by_other: HashMap<String, Vec<usize>>,
    all: Vec<(String, usize)>,
    card_client: HashMap<String, String>,
    /// Each client's rows' days, by row address.
    rows: HashMap<String, Option<Date>>,
    /// Tie files and notes that did not open.
    pub damaged: usize,
}

/// A day from Unix seconds where you are.
fn day_of(seconds: i64) -> Option<Date> {
    (seconds > 0).then(|| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(crate::zone::current()).date())).flatten()
}

/// A day from an RFC 3339 time or a day.
fn day_of_text(text: &str) -> Option<Date> {
    text.trim().get(..10).and_then(|d| d.parse().ok())
}

/// A card's UID from how an address names it: `sioul:contact/<uid>`, `urn:uuid:<uid>`.
pub fn card_of_address(address: &str) -> Option<String> {
    let address = address.trim();
    if let Some(uid) = address.strip_prefix("sioul:contact/") {
        return Some(crate::notes::decode(uid)).filter(|u| !u.is_empty());
    }
    // `urn:uuid:<UID>` as `contacts::uid_uri` writes it (percent-encoded), in any case; a UID that is a URN already as it is.
    if address.len() >= 4 && address[..4].eq_ignore_ascii_case("urn:") {
        let decoded = crate::contacts::percent_decoded(address);
        let bare = if decoded.len() >= 9 && decoded[..9].eq_ignore_ascii_case("urn:uuid:") { decoded[9..].to_string() } else { decoded };
        return Some(bare).filter(|u| !u.trim().is_empty());
    }
    None
}

/// The key a card is found by, from its UID or from any address naming it
/// (`sioul:contact/<UID>`, `urn:uuid:<UID>`, a UID that is a URN already,
/// percent-encoded, in any case): as main's contacts read an event's
/// `CONTACT` (`contacts::uid_key`, `uri_key`), so that a card is found
/// whichever form wrote it.
pub fn card_key(address_or_uid: &str) -> Option<String> {
    let text = address_or_uid.trim();
    if let Some(uid) = text.strip_prefix("sioul:contact/") {
        return Some(crate::contacts::uid_key(&crate::notes::decode(uid))).filter(|k| !k.is_empty());
    }
    crate::contacts::uri_key(text).or_else(|| Some(crate::contacts::uid_key(text))).filter(|k| !k.is_empty())
}

impl Index {
    /// The index of every client's ties: its cards; its tie files; its notes'
    /// front matter; invoices, projects and supplies' movements naming it or
    /// a row of it; events naming one of its cards (a `CONTACT` an extension's door wrote among them).
    pub fn build(door: &dyn Door, clients: &[Client], sources: &Sources) -> Index {
        let mut index = Index::default();
        // By id, so that a card two clients hold (made apart) is the smallest id's, as everywhere.
        let mut listed: Vec<&Client> = clients.iter().filter(|c| c.listed()).collect();
        listed.sort_by(|a, b| a.id.cmp(&b.id));
        let known: HashMap<&str, &Client> = listed.iter().map(|c| (c.id.as_str(), *c)).collect();
        // A client found by an address of it, a join followed.
        let joined: HashMap<&str, &str> = clients.iter().filter(|c| !c.joined.is_empty()).map(|c| (c.id.as_str(), c.joined.as_str())).collect();
        let resolve = |id: &str| -> Option<String> {
            let mut id = id;
            for _ in 0..8 {
                if known.contains_key(id) {
                    return Some(id.to_string());
                }
                id = joined.get(id)?;
            }
            None
        };
        for c in &listed {
            for card in &c.cards {
                if let Some(key) = card_key(card) {
                    index.card_client.entry(key).or_insert_with(|| c.id.clone());
                }
                index.add(Tie { client: c.id.clone(), at: c.uri(), other: crate::links::contact_uri(card), how: TieHow::Card, kept: KeptIn::Record, day: None, title: String::new(), shown: None });
            }
        }
        let mut by_email: HashMap<String, String> = HashMap::new();
        for contact in sources.contacts {
            if let Some(client) = card_key(&contact.uid).and_then(|k| index.card_client.get(&k)) {
                for email in &contact.emails {
                    by_email.entry(email.value.trim().to_ascii_lowercase()).or_insert_with(|| client.clone());
                }
            }
        }
        // Each client's own files: its tie files, its notes, its rows.
        for c in &listed {
            match ties_of(door, &c.id) {
                Ok(read) => {
                    index.damaged += read.damaged;
                    for tie in read.items {
                        index.add(tie);
                    }
                }
                Err(_) => index.damaged += 1,
            }
            if let Ok(read) = super::file::rows(door, &c.id) {
                index.damaged += read.damaged;
                for head in read.items {
                    index.rows.insert(super::address::row_uri(&c.id, &head.kind, &head.id), day_of_text(&head.when));
                }
            }
            if let Ok(read) = super::file::notes(door, &c.id) {
                index.damaged += read.damaged;
                for head in read.items {
                    let note = super::address::Address::Note { client: c.id.clone(), note: head.id.clone() }.uri();
                    if let Some((kind, row)) = head.row.split_once('/') {
                        let row = super::address::row_uri(&c.id, kind, row);
                        index.add(Tie { client: c.id.clone(), at: row, other: note.clone(), how: TieHow::Note, kept: KeptIn::NoteFront, day: day_of_text(&head.made), title: String::new(), shown: None });
                    }
                    for other in &head.ties {
                        index.add(Tie { client: c.id.clone(), at: c.uri(), other: other.clone(), how: TieHow::Note, kept: KeptIn::NoteFront, day: day_of_text(&head.made), title: note.clone(), shown: None });
                    }
                }
            }
        }
        // A profile's id (what 0.1.0 writes for a profile without a card), else exactly one
        // client's billing name, as `resolve` read them: the invoices that name a client so.
        let no_profiles = super::Clients::default();
        let profiles = sources.profiles.unwrap_or(&no_profiles);
        let by_reference = |reference: &str| -> Option<String> {
            let reference = reference.trim();
            if reference.is_empty() {
                return None;
            }
            if let Some(c) = listed.iter().find(|c| c.profile == reference) {
                return Some(c.id.clone());
            }
            let named: Vec<&&Client> = listed.iter().filter(|c| c.billing_of(profiles).name.trim().eq_ignore_ascii_case(reference)).collect();
            (named.len() == 1).then(|| named[0].id.clone())
        };
        // A row's address of a client, or the client's own when the row is gone.
        let row_or_client = |index: &Index, uri: &str| -> Option<(String, String)> {
            let address = super::address::Address::parse(uri)?;
            let client = resolve(address.client())?;
            Some(match address {
                super::address::Address::Row { kind, row, .. } => {
                    let uri = super::address::row_uri(&client, &kind, &row);
                    if index.rows.contains_key(&uri) { (client, uri) } else { (client.clone(), super::address::client_uri(&client)) }
                }
                _ => (client.clone(), super::address::client_uri(&client)),
            })
        };
        // Invoices: their client (its address, or an older invoice's card), their lines' rows.
        for invoice in sources.invoices {
            let other = if invoice.number.is_empty() { crate::links::draft_uri(&invoice.draft) } else { crate::links::invoice_uri(&invoice.number) };
            let day = day_of_text(&invoice.date);
            let client = super::address::Address::parse(&invoice.client_ref)
                .and_then(|a| resolve(a.client()))
                .or_else(|| card_of_address(&invoice.client_ref).and_then(|_| card_key(&invoice.client_ref)).and_then(|k| index.card_client.get(&k).cloned()))
                .or_else(|| by_reference(&invoice.client_ref));
            if let Some(client) = &client {
                index.add(Tie { client: client.clone(), at: super::address::client_uri(client), other: other.clone(), how: TieHow::Invoice, kept: KeptIn::Invoice, day, title: invoice.number.clone(), shown: None });
            }
            for line in invoice.lines.iter().filter(|l| !l.row.is_empty()) {
                if let Some((client, at)) = row_or_client(&index, &line.row) {
                    index.add(Tie { client, at, other: other.clone(), how: TieHow::InvoiceLine, kept: KeptIn::InvoiceLine, day, title: invoice.number.clone(), shown: None });
                }
            }
        }
        // Projects: their client, by its address or by a card's.
        for project in sources.projects {
            let Some(named) = project.client.as_deref().map(str::trim).filter(|c| !c.is_empty()) else { continue };
            let client = super::address::Address::parse(named)
                .and_then(|a| resolve(a.client()))
                .or_else(|| card_of_address(named).and_then(|_| card_key(named)).and_then(|k| index.card_client.get(&k).cloned()))
                .or_else(|| by_reference(named));
            if let Some(client) = client {
                index.add(Tie { client: client.clone(), at: super::address::client_uri(&client), other: crate::links::project_uri(&project.id), how: TieHow::Project, kept: KeptIn::Project, day: None, title: String::new(), shown: None });
            }
        }
        // Supplies' movements for a row.
        for m in sources.supply_moves.iter().filter(|m| !m.for_row.is_empty()) {
            if let Some((client, at)) = row_or_client(&index, &m.for_row) {
                index.add(Tie { client, at, other: format!("sioul:supply/{}", m.supply), how: TieHow::Supply, kept: KeptIn::SupplyMove, day: day_of(m.when()), title: String::new(), shown: None });
            }
        }
        // Events: a card they name (CONTACT, which an extension's door writes too; a guest).
        for event in sources.events {
            let other = crate::links::event_uri(&event.uid);
            let day = day_of(event.start);
            let mut found: Vec<(String, KeptIn)> = Vec::new();
            for key in event.contacts.iter().filter(|a| card_of_address(a).is_some()).filter_map(|a| card_key(a)) {
                if let Some(c) = index.card_client.get(&key) {
                    found.push((c.clone(), KeptIn::EventCard));
                }
            }
            for guest in &event.guests {
                if let Some(c) = by_email.get(&guest.trim().to_ascii_lowercase()) {
                    found.push((c.clone(), KeptIn::EventCard));
                }
            }
            found.dedup_by(|a, b| a.0 == b.0);
            for (client, kept) in found {
                index.add(Tie { client: client.clone(), at: super::address::client_uri(&client), other: other.clone(), how: TieHow::Appointment, kept, day, title: event.summary.clone(), shown: None });
            }
        }
        // An appointment's tie file says the day it holds now, when the event is here.
        let days: HashMap<String, Option<Date>> = sources.events.iter().map(|e| (crate::links::event_uri(&e.uid), day_of(e.start))).collect();
        for ties in index.by_client.values_mut() {
            for tie in ties.iter_mut().filter(|t| t.kept == KeptIn::TieFile) {
                // Never blanked by an event read without its day.
                if let Some(Some(day)) = days.get(&tie.other) {
                    tie.day = Some(*day);
                }
            }
        }
        index
    }

    fn add(&mut self, tie: Tie) {
        let list = self.by_client.entry(tie.client.clone()).or_default();
        if list.iter().any(|t| t.at == tie.at && t.other == tie.other && t.how == tie.how) {
            return;
        }
        let at = list.len();
        self.by_other.entry(tie.other.clone()).or_default().push(self.all.len());
        self.all.push((tie.client.clone(), at));
        list.push(tie);
    }

    fn tie(&self, n: usize) -> Option<&Tie> {
        let (client, at) = self.all.get(n)?;
        self.by_client.get(client)?.get(*at)
    }

    /// Everything tied to a client or to one of its rows.
    pub fn of_client(&self, id: &str) -> &[Tie] {
        self.by_client.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Everything tied to one row.
    pub fn of_row<'a>(&'a self, row_uri: &'a str) -> impl Iterator<Item = &'a Tie> + 'a {
        let client = super::address::Address::parse(row_uri).map(|a| a.client().to_string()).unwrap_or_default();
        self.of_client(&client).iter().filter(move |t| t.at == row_uri)
    }

    /// From the other end: the clients (and rows) a thing is tied to.
    pub fn clients_of(&self, address: &str) -> Vec<(String, Option<String>)> {
        let mut out: Vec<(String, Option<String>)> = Vec::new();
        for tie in self.by_other.get(address.trim()).into_iter().flatten().filter_map(|n| self.tie(*n)) {
            let row = (tie.at != super::address::client_uri(&tie.client)).then(|| tie.at.clone());
            if !out.iter().any(|(c, r)| *c == tie.client && *r == row) {
                out.push((tie.client.clone(), row));
            }
        }
        out
    }

    /// From the other end: every tie a thing has to a client or a row.
    pub fn ties_to(&self, address: &str) -> Vec<&Tie> {
        self.by_other.get(address.trim()).into_iter().flatten().filter_map(|n| self.tie(*n)).collect()
    }

    /// The client a card belongs to.
    pub fn client_of_card(&self, uid: &str) -> Option<&str> {
        card_key(uid).and_then(|k| self.card_client.get(&k)).map(String::as_str)
    }

    /// Whether a row is known in a file here.
    pub fn row_exists(&self, row_uri: &str) -> bool {
        self.rows.contains_key(row_uri)
    }

    /// Client since, last seen, next: from its rows', appointments' and invoices' days.
    pub fn facts(&self, id: &str, today: Date) -> Facts {
        let prefix = format!("{}/", super::address::client_uri(id));
        let days = self.of_client(id).iter().filter(|t| matches!(t.how, TieHow::Appointment | TieHow::Invoice | TieHow::Time)).filter_map(|t| t.day).chain(self.rows.iter().filter(|(uri, _)| uri.starts_with(&prefix)).filter_map(|(_, d)| *d));
        let mut facts = Facts::default();
        for day in days {
            facts.since = Some(facts.since.map_or(day, |s| s.min(day)));
            if day <= today {
                facts.last = Some(facts.last.map_or(day, |l| l.max(day)));
            } else {
                facts.next = Some(facts.next.map_or(day, |n| n.min(day)));
            }
        }
        facts
    }
}

/// What a client's ties say of it, computed, never stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts {
    pub since: Option<Date>,
    pub last: Option<Date>,
    pub next: Option<Date>,
}

/// A client as the global tie index (`crate::ties`) reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientTies {
    pub uri: String,
    pub name: String,
    pub cards: Vec<String>,
    /// What is tied to it or to one of its rows, by address (its cards aside).
    pub ties: Vec<String>,
}

/// Every client and its ties, for the global tie index; none while the store
/// is not open in this program.
pub fn for_tie_index(loaded: &crate::links::Loaded, invoices: &[crate::invoice::Invoice]) -> Vec<ClientTies> {
    let Ok(door) = super::door::opened() else { return Vec::new() };
    let Ok(read) = super::record::list(door.as_ref()) else { return Vec::new() };
    let profiles = loaded.places.as_ref().and_then(|p| super::Clients::read_file(&p.records).ok()).unwrap_or_default();
    let sources = Sources { contacts: &loaded.contacts, events: &loaded.events, invoices, projects: &loaded.projects, profiles: Some(&profiles), ..Sources::default() };
    let index = Index::build(door.as_ref(), &read.items, &sources);
    read.items
        .iter()
        .map(|c| ClientTies {
            uri: c.uri(),
            name: c.shown_name(&loaded.contacts),
            cards: c.cards.clone(),
            // The file's own parts (a note) never reach the global index: closed until opened.
            ties: index.of_client(&c.id).iter().filter(|t| t.how != TieHow::Card && super::address::Address::parse(&t.other).is_none()).map(|t| t.other.clone()).collect(),
        })
        .collect()
}
