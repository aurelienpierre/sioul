// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The client, an object of its own (docs/clients.md, "The client"): a
//! name, one or several contact cards by their UIDs, and the billing
//! identity today's profile held. **Lean**: a client that only bills holds
//! this and nothing else; no template, folder or question exists until its
//! file is used.
//!
//! One record per client, `data/clients/<id>/client.toml` in the store:
//!
//! ```toml
//! format = 1
//! id = "3f2a9c1e-…"
//! name = "Marie Exemple"
//! profile = "lumen"            # the client profile it came from (`sioul-clients.toml`), if any
//!
//! [cards]                      # its cards by UID, each with its place: each travels on its own
//! "c-41" = 1
//!
//! [billing]
//! kind = "consumer"
//! name = "Marie Exemple"
//! address = { lines = ["1 rue de l'Exemple"], postcode = "75002", city = "Paris", country = "FR" }
//! ```
//!
//! A change is written key by key into the record as it stands (`change`),
//! so that what another device or a newer Sioul wrote stays; a write that
//! finds the record changed meanwhile reads it again and redoes the change.
//!
//! **One billing identity, one truth.** A client moved from a profile names
//! it (`profile`): while that profile is in `sioul-clients.toml` (until every
//! device knows clients, `moving::adopt`), the billing identity is the
//! profile's, read (`billing_of`) and written (`change_billing`) there, as
//! a 0.1.0 device reads and writes it; once adopted, the record's own.
//!
//! **One card, one client.** A client made with a card has the id made from
//! its first card (`card_id`); a card a client holds already gives that client.

use super::door::{Door, Expect};
use super::template::FolderDef;
use super::{Error, Read};
use crate::contacts::Contact;
use crate::invoice::{Address, Identifier};
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use toml_edit::{DocumentMut, Item};

/// Where every client's file is, in the store: the data folder's `clients/`
/// (`FOLDER_NAME`), sealed with everything Sioul keeps there.
pub const ROOT: &str = "data/clients";
/// The clients' folder's name in the data folder (its path, for the sharing and the export).
pub const FOLDER_NAME: &str = "clients";
/// The record's format.
pub const FORMAT: u32 = 1;
/// The folder of the "Not a client" marks (`extension::decline`): not a client's.
pub const DECLINED: &str = "declined";

/// A client's folder in the store.
pub fn folder(id: &str) -> String {
    format!("{ROOT}/{id}")
}

/// A client's record in the store.
pub fn record_path(id: &str) -> String {
    format!("{ROOT}/{id}/client.toml")
}

/// The billing identity: today's client profile's fields, its id and contact aside.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Billing {
    /// "business", "consumer", "public".
    pub kind: String,
    /// Its legal name, as invoices print it; "" for the client's name.
    pub name: String,
    pub address: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<Address>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<Identifier>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub language: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terms_days: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_of_month: Option<bool>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub email: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub order: String,
}

impl Default for Billing {
    fn default() -> Billing {
        Billing::from(&super::Profile::default())
    }
}

impl From<&super::Profile> for Billing {
    fn from(p: &super::Profile) -> Billing {
        Billing {
            kind: p.kind.clone(),
            name: p.name.clone(),
            address: p.address.clone(),
            delivery: p.delivery.clone(),
            ids: p.ids.clone(),
            language: p.language.clone(),
            currency: p.currency.clone(),
            terms_days: p.terms_days,
            end_of_month: p.end_of_month,
            email: p.email.clone(),
            order: p.order.clone(),
        }
    }
}

impl Billing {
    /// As a profile, for what reads profiles (`super::resolve`, the invoices' editor).
    pub fn profile(&self, id: &str, contact: &str) -> super::Profile {
        super::Profile {
            id: id.to_string(),
            contact: contact.to_string(),
            kind: self.kind.clone(),
            name: self.name.clone(),
            address: self.address.clone(),
            delivery: self.delivery.clone(),
            ids: self.ids.clone(),
            language: self.language.clone(),
            currency: self.currency.clone(),
            terms_days: self.terms_days,
            end_of_month: self.end_of_month,
            email: self.email.clone(),
            order: self.order.clone(),
        }
    }
}

/// A client.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Client {
    pub id: String,
    /// As shown; "" for its first card's name.
    pub name: String,
    /// Its contact cards, by UID: none for a walk-in client yet.
    pub cards: Vec<String>,
    /// Its own billing identity: the truth once no profile of the file names it (`billing_of`).
    pub billing: Billing,
    /// The client profile it came from (`sioul-clients.toml`), by id; "" none. Kept
    /// after the profile is adopted, so that an invoice naming the profile finds it.
    pub profile: String,
    /// `"<extension>/<set>"` when an extension said its people are clients; "" otherwise.
    pub from: String,
    /// The extension's mark of this person (`extensions::person_mark`), when `from` is set.
    pub mark: String,
    /// The day the person noted the client agreed (a health fact outside a health profession).
    pub agreed: Option<Date>,
    /// The person's mark "Kept from AI agents".
    pub kept_from_agents: bool,
    /// Out of the lists since then.
    pub archived: Option<Date>,
    /// No longer a client ("Not a client", "They are not my clients"): out of every list.
    pub declined: Option<Date>,
    /// Joined into another by Same as…: that one's id; this record only redirects.
    pub joined: String,
    /// The folder template's version its file was made with; 0 while it has no file.
    pub folders_version: u32,
    /// Folders of this file alone, by id.
    pub own_folders: BTreeMap<String, FolderDef>,
    /// The record's generation as read: give it back to write over it safely.
    pub generation: u64,
}

/// The record as written.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Record {
    format: u32,
    id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    profile: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    from: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    mark: String,
    #[serde(deserialize_with = "lenient_day", skip_serializing_if = "Option::is_none")]
    agreed: Option<Date>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    kept_from_agents: bool,
    #[serde(deserialize_with = "lenient_day", skip_serializing_if = "Option::is_none")]
    archived: Option<Date>,
    #[serde(deserialize_with = "lenient_day", skip_serializing_if = "Option::is_none")]
    declined: Option<Date>,
    #[serde(skip_serializing_if = "String::is_empty")]
    joined: String,
    #[serde(skip_serializing_if = "is_zero")]
    folders_version: u32,
    #[serde(deserialize_with = "lenient_folders", skip_serializing_if = "BTreeMap::is_empty")]
    folder: BTreeMap<String, FolderDef>,
    /// Its cards: UID → place (a list of UIDs, as a first draft wrote it, reads too).
    #[serde(deserialize_with = "lenient_cards", skip_serializing_if = "BTreeMap::is_empty")]
    cards: BTreeMap<String, i64>,
    billing: Billing,
}

/// Cards as a table (UID → place) or a list of UIDs.
fn lenient_cards<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<String, i64>, D::Error> {
    let found: toml::Value = Deserialize::deserialize(d)?;
    Ok(match found {
        toml::Value::Table(t) => t.into_iter().filter(|(k, _)| !k.trim().is_empty()).map(|(k, v)| (k, v.as_integer().unwrap_or(i64::MAX))).collect(),
        toml::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).filter(|u| !u.trim().is_empty()).enumerate().map(|(i, u)| (u.to_string(), i as i64 + 1)).collect(),
        _ => BTreeMap::new(),
    })
}

/// Cards in their order.
fn ordered(cards: &BTreeMap<String, i64>) -> Vec<String> {
    let mut list: Vec<(&String, &i64)> = cards.iter().collect();
    list.sort_by(|a, b| a.1.cmp(b.1).then(a.0.cmp(b.0)));
    list.into_iter().map(|(u, _)| u.clone()).collect()
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A day written as a text or as a TOML date; anything else is none.
fn lenient_day<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Date>, D::Error> {
    let found: toml::Value = Deserialize::deserialize(d)?;
    Ok(match found {
        toml::Value::String(s) => s.trim().parse().ok(),
        toml::Value::Datetime(dt) => dt.to_string().get(..10).and_then(|s| s.parse().ok()),
        _ => None,
    })
}

fn lenient_folders<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<String, FolderDef>, D::Error> {
    let found: toml::Value = Deserialize::deserialize(d)?;
    Ok(found.as_table().map(|t| t.iter().filter(|(k, _)| super::address::valid_id(k)).filter_map(|(k, v)| Some((k.clone(), v.clone().try_into().ok()?))).collect()).unwrap_or_default())
}

impl Client {
    /// Its address.
    pub fn uri(&self) -> String {
        super::address::client_uri(&self.id)
    }

    /// Its name as shown: its own, else its first card's (the person, else the organisation).
    pub fn shown_name(&self, contacts: &[Contact]) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        self.cards
            .iter()
            .find_map(|uid| contacts.iter().find(|c| c.uid == *uid))
            .map(|c| if c.name.trim().is_empty() { c.org.trim().to_string() } else { c.name.trim().to_string() })
            .unwrap_or_default()
    }

    /// The card its invoices are addressed to: the one whose address its
    /// billing identity mails, else the first.
    pub fn billing_card<'a>(&self, contacts: &'a [Contact]) -> Option<&'a Contact> {
        let cards: Vec<&Contact> = self.cards.iter().filter_map(|uid| contacts.iter().find(|c| c.uid == *uid)).collect();
        let email = self.billing.email.trim();
        cards.iter().find(|c| !email.is_empty() && c.emails.iter().any(|e| e.value.trim().eq_ignore_ascii_case(email))).or(cards.first()).copied()
    }

    /// Who an invoice bills, as a draft freezes it at issue: its billing
    /// identity, its name and address from the billing card where the
    /// identity leaves them empty (as a profile did, `super::resolve`).
    pub fn invoice_client(&self, profiles: &super::Clients, contacts: &[Contact]) -> crate::invoice::Client {
        let card = self.billing_card(contacts);
        let contact = card.map(|c| crate::links::contact_uri(&c.uid)).unwrap_or_default();
        let mut billing = self.billing_of(profiles);
        if billing.name.trim().is_empty() {
            billing.name = if self.name.trim().is_empty() { String::new() } else { self.name.trim().to_string() };
        }
        // Named by its address, as a store client is wherever profiles are read
        // (`as_profile`): `resolve` then takes the card's postal address when the
        // identity has none (the second review, A).
        let profile = billing.profile(&self.uri(), &contact);
        let profiles = super::Clients { list: vec![profile] };
        let reference = if contact.is_empty() { self.uri() } else { contact };
        super::resolve(&reference, &profiles, contacts).0
    }

    /// Its billing identity: its profile's while the file holds it (until
    /// it is adopted at the gate), else its own.
    pub fn billing_of(&self, profiles: &super::Clients) -> Billing {
        match self.profile_in(profiles) {
            Some(p) => Billing::from(p),
            None => self.billing.clone(),
        }
    }

    /// The profile of the file that is still its billing identity.
    pub fn profile_in<'a>(&self, profiles: &'a super::Clients) -> Option<&'a super::Profile> {
        (!self.profile.is_empty()).then(|| profiles.list.iter().find(|p| p.id == self.profile)).flatten()
    }

    /// As a profile whose id is its address, for what reads profiles
    /// (`super::resolve`): its own billing identity, its first card, its name
    /// where the identity names none.
    pub fn as_profile(&self) -> super::Profile {
        let mut profile = self.billing.profile(&self.uri(), &self.cards.first().map(|u| crate::links::contact_uri(u)).unwrap_or_default());
        if profile.name.trim().is_empty() {
            profile.name = self.name.trim().to_string();
        }
        profile
    }

    /// Whether it is in the lists: neither joined into another nor declined.
    pub fn listed(&self) -> bool {
        self.joined.is_empty() && self.declined.is_none()
    }

    fn of(record: Record, generation: u64) -> Client {
        Client {
            id: record.id,
            name: record.name,
            cards: ordered(&record.cards),
            billing: record.billing,
            profile: record.profile,
            from: record.from,
            mark: record.mark,
            agreed: record.agreed,
            kept_from_agents: record.kept_from_agents,
            archived: record.archived,
            declined: record.declined,
            joined: record.joined,
            folders_version: record.folders_version,
            own_folders: record.folder,
            generation,
        }
    }

    fn record(&self) -> Record {
        Record {
            format: FORMAT,
            id: self.id.clone(),
            name: self.name.trim().to_string(),
            cards: self.cards.iter().enumerate().map(|(i, u)| (u.clone(), i as i64 + 1)).collect(),
            profile: self.profile.clone(),
            from: self.from.clone(),
            mark: self.mark.clone(),
            agreed: self.agreed,
            kept_from_agents: self.kept_from_agents,
            archived: self.archived,
            declined: self.declined,
            joined: self.joined.clone(),
            folders_version: self.folders_version,
            folder: self.own_folders.clone(),
            billing: self.billing.clone(),
        }
    }
}

/// A record read; a record whose `id` is not its folder's is damaged.
fn parse(path: &str, id: &str, text: &str, generation: u64) -> Result<Client, Error> {
    let record: Record = toml::from_str(text).map_err(|_| Error::Damaged(path.to_string()))?;
    if record.id != id {
        return Err(Error::Damaged(path.to_string()));
    }
    Ok(Client::of(record, generation))
}

/// The record of client `id` as stored, joined or not; None when there is none.
pub fn read(door: &dyn Door, id: &str) -> Result<Option<Client>, Error> {
    if !super::address::valid_id(id) {
        return Err(Error::Refused("id"));
    }
    let path = record_path(id);
    match door.read_text(&path)? {
        None => Ok(None),
        Some((text, generation)) => parse(&path, id, &text, generation).map(Some),
    }
}

/// Client `id`, following a join (Same as…) to the client it joined.
pub fn find(door: &dyn Door, id: &str) -> Result<Option<Client>, Error> {
    let mut id = id.to_string();
    for _ in 0..8 {
        match read(door, &id)? {
            Some(c) if !c.joined.is_empty() && c.joined != id => id = c.joined.clone(),
            found => return Ok(found),
        }
    }
    Ok(None)
}

/// Every client's id: the folders directly under the clients' root.
pub fn ids(door: &dyn Door) -> Result<Vec<String>, Error> {
    let mut found: Vec<String> = door
        .folders(ROOT)?
        .into_iter()
        .filter_map(|f| f.strip_prefix(&format!("{ROOT}/")).map(|rest| rest.split('/').next().unwrap_or("").to_string()))
        .filter(|id| id != DECLINED && id != super::keep::ERASED && super::address::valid_id(id))
        .collect();
    found.dedup();
    Ok(found)
}

/// Every client in the lists (archived ones included; joined and declined
/// ones left out), and how many records did not open.
pub fn list(door: &dyn Door) -> Result<Read<Vec<Client>>, Error> {
    every(door).map(|read| Read { items: read.items.into_iter().filter(Client::listed).collect(), damaged: read.damaged })
}

/// Every client record, joined and declined ones included.
pub fn every(door: &dyn Door) -> Result<Read<Vec<Client>>, Error> {
    let mut out: Read<Vec<Client>> = Read::default();
    for id in ids(door)? {
        match read(door, &id) {
            Ok(Some(c)) => out.items.push(c),
            Ok(None) => {}
            Err(Error::Damaged(_)) => out.damaged += 1,
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// The client whose cards hold `uid`: the one of the smallest id when two
/// do (made on two devices before they met), the same on every device.
pub fn of_card<'a>(clients: &'a [Client], uid: &str) -> Option<&'a Client> {
    clients.iter().filter(|c| c.listed() && c.cards.iter().any(|u| u == uid)).min_by(|a, b| a.id.cmp(&b.id))
}

/// The client made of an extension's person, by its mark.
pub fn by_mark<'a>(clients: &'a [Client], mark: &str) -> Option<&'a Client> {
    clients.iter().find(|c| !mark.is_empty() && c.mark == mark)
}

/// The id of the client made from a card: the same on every device, so that
/// two devices that make a client of one card make one (RFC 9562 §5.5).
pub fn card_id(uid: &str) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("client\ncard\n{uid}"))
}

/// Whether client `id` was erased (its mark, `keep::erased_path`): its id is never written again.
pub fn was_erased(door: &dyn Door, id: &str) -> Result<bool, Error> {
    Ok(door.read(&super::keep::erased_path(id))?.is_some())
}

/// A client written whole as new; `Error::Changed` when its id is taken,
/// `Error::Refused` when it was erased (its files would be purged again).
fn create(door: &dyn Door, client: &Client) -> Result<Client, Error> {
    if was_erased(door, &client.id)? {
        return Err(Error::Refused("an erased client's id"));
    }
    let text = toml::to_string(&client.record()).map_err(|_| Error::Refused("record"))?;
    let generation = door.write(&record_path(&client.id), text.as_bytes(), Expect::Absent)?;
    crate::changes::said(crate::changes::Kind::Clients);
    Ok(Client { generation, ..client.clone() })
}

/// A new client, by hand: a name, its cards, its billing identity. With a
/// card, its id is made from its first card (`card_id`), and a card a client
/// holds already gives that client, unchanged: one card, one client.
pub fn make(door: &dyn Door, name: &str, cards: &[String], billing: Billing) -> Result<Client, Error> {
    let cards: Vec<String> = cards.iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    let Some(first) = cards.first() else {
        return create(door, &Client { id: crate::ids::new(), name: name.trim().to_string(), billing, ..Client::default() });
    };
    let all = list(door)?.items;
    if let Some(found) = cards.iter().find_map(|u| of_card(&all, u)) {
        return Ok(found.clone());
    }
    // A card whose client was erased gives a new client, of an id of its own:
    // the erased one's stays erased on every device.
    let id = if was_erased(door, &card_id(first))? { crate::ids::new() } else { card_id(first) };
    if let Some(found) = find(door, &id)? {
        return Ok(found);
    }
    match create(door, &Client { id: id.clone(), name: name.trim().to_string(), cards, billing, ..Client::default() }) {
        Err(Error::Changed) => find(door, &id)?.ok_or(Error::Changed),
        other => other,
    }
}

/// The client of a card, made when there is none (`make`).
pub fn make_from_card(door: &dyn Door, card_uid: &str, billing: Billing) -> Result<Client, Error> {
    make(door, "", &[card_uid.to_string()], billing)
}

/// A client written as made elsewhere (the move, an extension): kept as it
/// is when its id is there already.
pub(crate) fn make_as(door: &dyn Door, client: &Client) -> Result<Client, Error> {
    if let Some(found) = read(door, &client.id)? {
        return Ok(found);
    }
    match create(door, client) {
        Err(Error::Changed) => read(door, &client.id)?.ok_or(Error::Changed),
        other => other,
    }
}

/// What a client's card changed, each part when it changed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Change {
    pub name: Option<String>,
    pub add_cards: Vec<String>,
    pub remove_cards: Vec<String>,
    /// Its billing identity, field by field, as a profile's form says it.
    pub billing: Option<super::ProfileChange>,
    pub kept_from_agents: Option<bool>,
    /// Some(None) takes the day out.
    pub agreed: Option<Option<Date>>,
    pub archived: Option<Option<Date>>,
    pub declined: Option<Option<Date>>,
    pub joined: Option<String>,
    /// An extension's person's mark, noted on a client already holding its card.
    pub mark: Option<String>,
    /// The profile it came from, noted on a client already holding its card (the move).
    pub profile: Option<String>,
    /// Its billing identity written whole (the profile adopted at the gate; Same as…).
    pub billing_whole: Option<Billing>,
    pub folders_version: Option<u32>,
    /// A folder of this file alone, written (Some) or taken out (None).
    pub own_folders: Vec<(String, Option<FolderDef>)>,
}

/// A change written key by key into client `id`'s record as it stands:
/// what another device or a newer Sioul wrote stays; written again over a
/// change made meanwhile. The client as it now is.
pub fn change(door: &dyn Door, id: &str, change: &Change) -> Result<Client, Error> {
    if !super::address::valid_id(id) {
        return Err(Error::Refused("id"));
    }
    let path = record_path(id);
    let mut found = false;
    door.update(&path, &mut |now| {
        let Some(now) = now else { return Ok(None) };
        found = true;
        let text = std::str::from_utf8(now).map_err(|_| Error::Damaged(path.clone()))?;
        let mut doc: DocumentMut = text.parse().map_err(|_| Error::Damaged(path.clone()))?;
        // A record of a later format is read here, never changed (its meaning may have moved).
        if newer_format(&doc) {
            return Err(Error::NewerTemplate);
        }
        apply(&mut doc, change);
        let out = doc.to_string();
        // What it now says must read back as this client, else nothing is written.
        parse(&path, id, &out, 0)?;
        Ok((out != text).then(|| out.into_bytes()))
    })?;
    if !found {
        return Err(Error::NotFound);
    }
    crate::changes::said(crate::changes::Kind::Clients);
    read(door, id)?.ok_or(Error::NotFound)
}

/// Whether a file of the clients' says a format later than this Sioul's
/// (`format`, a record's or a tie's): read, never changed here.
pub(crate) fn newer_format(doc: &DocumentMut) -> bool {
    doc.get("format").and_then(Item::as_integer).is_some_and(|f| f > i64::from(FORMAT))
}

fn apply(doc: &mut DocumentMut, change: &Change) {
    let text = |doc: &mut DocumentMut, key: &str, value: &str| {
        if value.trim().is_empty() {
            doc.remove(key);
        } else {
            doc[key] = toml_edit::value(value.trim());
        }
    };
    let day = |doc: &mut DocumentMut, key: &str, value: &Option<Option<Date>>| match value {
        Some(Some(d)) => doc[key] = toml_edit::value(d.to_string()),
        Some(None) => {
            doc.remove(key);
        }
        None => {}
    };
    if let Some(name) = &change.name {
        text(doc, "name", name);
    }
    if !change.add_cards.is_empty() || !change.remove_cards.is_empty() {
        // A table, each card its own entry, so that two devices adding one each keep both.
        let mut cards: BTreeMap<String, i64> = match doc.get("cards") {
            Some(Item::Value(toml_edit::Value::Array(a))) => a.iter().filter_map(|v| v.as_str()).enumerate().map(|(i, u)| (u.to_string(), i as i64 + 1)).collect(),
            Some(item) => item.as_table_like().map(|t| t.iter().map(|(k, v)| (k.to_string(), v.as_integer().unwrap_or(i64::MAX))).collect()).unwrap_or_default(),
            None => BTreeMap::new(),
        };
        cards.retain(|c, _| !change.remove_cards.contains(c));
        for card in &change.add_cards {
            let next = cards.values().copied().filter(|p| *p != i64::MAX).max().unwrap_or(0) + 1;
            if !card.trim().is_empty() && !cards.contains_key(card.trim()) {
                cards.insert(card.trim().to_string(), next);
            }
        }
        doc.remove("cards");
        if !cards.is_empty() {
            let mut table = toml_edit::Table::new();
            for (uid, place) in &cards {
                table.insert(uid, toml_edit::value(*place));
            }
            doc["cards"] = Item::Table(table);
        }
    }
    if let Some(profile) = &change.profile {
        text(doc, "profile", profile);
    }
    if let Some(billing) = &change.billing_whole {
        let written: DocumentMut = toml::to_string(billing).unwrap_or_default().parse().unwrap_or_default();
        doc["billing"] = Item::Table(written.as_table().clone());
    }
    if let Some(kept) = change.kept_from_agents {
        if kept {
            doc["kept_from_agents"] = toml_edit::value(true);
        } else {
            doc.remove("kept_from_agents");
        }
    }
    day(doc, "agreed", &change.agreed);
    day(doc, "archived", &change.archived);
    day(doc, "declined", &change.declined);
    if let Some(joined) = &change.joined {
        text(doc, "joined", joined);
    }
    if let Some(mark) = &change.mark {
        text(doc, "mark", mark);
    }
    if let Some(v) = change.folders_version {
        doc["folders_version"] = toml_edit::value(i64::from(v));
    }
    for (id, folder) in &change.own_folders {
        if !super::address::valid_id(id) {
            continue;
        }
        match folder {
            Some(f) => {
                if !doc.contains_key("folder") {
                    let mut t = toml_edit::Table::new();
                    t.set_implicit(true);
                    doc["folder"] = Item::Table(t);
                }
                let written: DocumentMut = toml::to_string(f).unwrap_or_default().parse().unwrap_or_default();
                if let Some(table) = doc["folder"].as_table_mut() {
                    table.insert(id, Item::Table(written.as_table().clone()));
                }
            }
            None => {
                if let Some(table) = doc.get_mut("folder").and_then(Item::as_table_mut) {
                    table.remove(id);
                }
            }
        }
    }
    if let Some(billing) = &change.billing {
        if !doc.contains_key("billing") {
            doc["billing"] = Item::Table(toml_edit::Table::new());
        }
        if let Some(table) = doc["billing"].as_table_mut() {
            super::apply_change(table, billing);
        }
    }
}

/// A client's billing identity changed field by field where it lives: in
/// its profile of `profiles_file` while that profile names it (one truth,
/// as a 0.1.0 device reads and writes it), else in its record.
pub fn change_billing(door: &dyn Door, id: &str, change: &super::ProfileChange, profiles_file: Option<&std::path::Path>) -> Result<Client, Error> {
    let client = find(door, id)?.ok_or(Error::NotFound)?;
    if let Some(file) = profiles_file
        && !client.profile.is_empty()
    {
        let root = file.parent().unwrap_or(std::path::Path::new("."));
        let profiles = super::Clients::read_file(root).map_err(Error::profiles)?;
        if profiles.get(&client.profile).is_some() {
            super::change(file, &client.profile, change).map_err(Error::profiles)?;
            return Ok(client);
        }
    }
    self::change(door, &client.id, &Change { billing: Some(change.clone()), ..Change::default() })
}

/// Client `id` archived on `day` (out of the lists since then), or brought back (None).
pub fn archive(door: &dyn Door, id: &str, day: Option<Date>) -> Result<Client, Error> {
    change(door, id, &Change { archived: Some(day), ..Change::default() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::door::Memory;
    use crate::contacts::Labeled;

    fn marie() -> Contact {
        Contact { uid: "c-41".into(), name: "Marie Exemple".into(), emails: vec![Labeled { label: String::new(), value: "marie@example.org".into() }], ..Contact::default() }
    }

    /// Lean: a client that only bills is a name, its cards and its billing
    /// identity; its record holds nothing else, and no file exists.
    #[test]
    fn a_client_is_lean() {
        let door = Memory::new();
        let billing = Billing { kind: "consumer".into(), address: Address { lines: vec!["1 rue de l'Exemple".into()], postcode: "75002".into(), city: "Paris".into(), country: "FR".into() }, ..Billing::default() };
        let made = make(&door, "", &["c-41".into()], billing).unwrap();
        assert_eq!(door.paths(), vec![record_path(&made.id)], "one record, nothing else");
        let text = door.read_text(&record_path(&made.id)).unwrap().unwrap().0;
        for absent in ["folder", "agreed", "kept_from_agents", "archived", "from", "joined", "folders_version"] {
            assert!(!text.contains(absent), "{absent} not written: {text}");
        }
        let read = find(&door, &made.id).unwrap().unwrap();
        assert_eq!((read.cards.clone(), read.billing.kind.as_str(), read.billing.address.city.as_str()), (vec!["c-41".to_string()], "consumer", "Paris"));
        assert_eq!(read.shown_name(&[marie()]), "Marie Exemple", "its card's name");
        let billed = read.invoice_client(&super::super::Clients::default(), &[marie()]);
        assert_eq!((billed.name.as_str(), billed.email.as_str(), billed.kind.as_str()), ("Marie Exemple", "marie@example.org", "consumer"));
        assert_eq!(list(&door).unwrap().items.len(), 1);
        assert_eq!(of_card(&list(&door).unwrap().items, "c-41").map(|c| c.id.clone()), Some(made.id.clone()));
    }

    /// Made from a card on two devices, one client: its id comes from the card.
    #[test]
    fn a_card_makes_one_client_everywhere() {
        let (here, there) = (Memory::new(), Memory::new());
        let a = make_from_card(&here, "c-41", Billing::default()).unwrap();
        let b = make_from_card(&there, "c-41", Billing::default()).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(make_from_card(&here, "c-41", Billing::default()).unwrap().id, a.id, "the one there");
        assert_eq!(ids(&here).unwrap().len(), 1);
    }

    /// A change is written key by key: keys a newer Sioul wrote, comments and
    /// the rest of the billing identity stay; a change made meanwhile is kept.
    #[test]
    fn a_change_keeps_what_it_does_not_change() {
        let door = Memory::new();
        let id = "3f2a9c1e";
        let written = "format = 1\nid = \"3f2a9c1e\"\n# Since the first job.\nname = \"Studio Lumen\"\ncards = [\"c-1\"]\nmood = \"kept by a newer Sioul\"\n\n[billing]\nkind = \"business\"\nname = \"Studio Lumen SARL\"\nterms_days = 45\naddress = { lines = [\"1 rue de l'Exemple\"], postcode = \"75002\", city = \"Paris\", country = \"FR\" }\n";
        door.write(&record_path(id), written.as_bytes(), Expect::Any).unwrap();
        let billing = super::super::ProfileChange { city: Some("Lyon".into()), ..Default::default() };
        let now = change(&door, id, &Change { add_cards: vec!["c-2".into()], billing: Some(billing), kept_from_agents: Some(true), archived: Some(Some("2026-11-24".parse().unwrap())), ..Change::default() }).unwrap();
        let text = door.read_text(&record_path(id)).unwrap().unwrap().0;
        for kept in ["# Since the first job.", "mood = \"kept by a newer Sioul\"", "terms_days = 45", "Studio Lumen SARL"] {
            assert!(text.contains(kept), "{kept}: {text}");
        }
        assert_eq!((now.cards.len(), now.billing.address.city.as_str(), now.kept_from_agents, now.archived.map(|d| d.to_string())), (2, "Lyon", true, Some("2026-11-24".into())));
        assert_eq!(change(&door, "absent0", &Change { name: Some("x".into()), ..Change::default() }), Err(Error::NotFound));
        assert_eq!(change(&door, "../x", &Change::default()), Err(Error::Refused("id")));
    }

    /// A damaged record is counted, never taken for an empty list.
    #[test]
    fn a_damaged_record_is_counted() {
        let door = Memory::new();
        let a = make(&door, "A", &[], Billing::default()).unwrap();
        make(&door, "B", &[], Billing::default()).unwrap();
        door.damage(&record_path(&a.id));
        let read = list(&door).unwrap();
        assert_eq!((read.items.len(), read.damaged), (1, 1));
        door.write(&record_path("wrongid1"), b"format = 1\nid = \"another\"\n", Expect::Any).unwrap();
        assert_eq!(list(&door).unwrap().damaged, 2, "a record whose id is not its folder's");
    }

    /// Joined (Same as…) and declined clients leave the lists; a join is followed.
    #[test]
    fn joined_and_declined_leave_the_lists() {
        let door = Memory::new();
        let a = make(&door, "A", &[], Billing::default()).unwrap();
        let b = make(&door, "B", &[], Billing::default()).unwrap();
        let c = make(&door, "C", &[], Billing::default()).unwrap();
        change(&door, &b.id, &Change { joined: Some(a.id.clone()), ..Change::default() }).unwrap();
        change(&door, &c.id, &Change { declined: Some(Some("2026-11-24".parse().unwrap())), ..Change::default() }).unwrap();
        assert_eq!(list(&door).unwrap().items.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["A"]);
        assert_eq!(find(&door, &b.id).unwrap().map(|c| c.id), Some(a.id));
    }
}
