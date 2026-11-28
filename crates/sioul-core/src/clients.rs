// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Client profiles: who you bill, as invoices need it (docs/projects.md,
//! "Invoices"). Kept in the notes folder's `sioul-clients.toml`, beside
//! `sioul-projects.toml`, one `[[client]]` per client, tied to its contact
//! by address (`sioul:contact/<uid>`), so that a profile travels with the
//! projects and never adds fields to a card on someone's server: whether it
//! is a business, a consumer or a public body; its legal name; its billing
//! and delivery addresses; its identifiers (a SIREN, a VAT number); the
//! language, currency and terms of its invoices; the address they are
//! mailed to; a purchase order to print.
//!
//! A project's client (`Project::client`) finds its profile by the contact
//! it names, else by the profile's name; a name with no profile bills as it
//! did before, from the contact's card or as written (`resolve`).

//!
//! **Clients as objects of their own** (docs/clients.md) live in the
//! submodules: the client and its record (`record`), its file's parts
//! (`file`), the templates (`template`), the ties and their index (`ties`),
//! the options that protect (`protect`), keeping and erasing (`keep`), an
//! extension's people (`extension`), the move from these profiles
//! (`moving`), all through the store's door (`door`).

pub mod address;
pub mod appointment;
pub mod door;
mod error;
pub mod extension;
pub mod file;
pub mod keep;
pub mod moving;
pub mod protect;
pub mod record;
pub mod search;
pub mod template;
pub mod ties;
#[cfg(test)]
mod files_tests;

pub use address::{Address as ClientAddress, client_uri, valid_id};
pub use error::Error;
pub use record::{Billing, Client as ClientRecord};

use crate::contacts::Contact;
use crate::invoice::{Address, Client, Identifier};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item};

/// Whether Clients is on (`features::on`, `Feature::Clients`, once that
/// list is in the core; on until then).
pub fn on(_config: &crate::config::Config) -> bool {
    true
}

/// What a listing read, and how many of its files did not open: a damaged
/// file is counted, never taken for none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Read<T> {
    pub items: T,
    pub damaged: usize,
}

/// The profiles' file, at the root of the notes folder.
pub const FILE: &str = "sioul-clients.toml";
/// The table each profile is written in.
const TABLE: &str = "client";

/// One client's profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    /// Its contact: `sioul:contact/<uid>`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub contact: String,
    /// "business", "consumer", "public".
    #[serde(default = "business")]
    pub kind: String,
    /// Its legal name, as invoices print it.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: Address,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery: Option<Address>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<Identifier>,
    /// The language of its invoices ("en", "fr"); the settings' when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub currency: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_of_month: Option<bool>,
    /// Where its invoices are mailed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub email: String,
    /// Its purchase order, printed on its invoices (BT-13).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub order: String,
}

fn business() -> String {
    "business".into()
}

impl Default for Profile {
    fn default() -> Profile {
        Profile {
            id: String::new(),
            contact: String::new(),
            kind: business(),
            name: String::new(),
            address: Address::default(),
            delivery: None,
            ids: Vec::new(),
            language: String::new(),
            currency: String::new(),
            terms_days: None,
            end_of_month: None,
            email: String::new(),
            order: String::new(),
        }
    }
}

impl Profile {
    /// The client as an invoice freezes it.
    pub fn client(&self) -> Client {
        Client { kind: self.kind.clone(), name: self.name.clone(), address: self.address.clone(), delivery: self.delivery.clone(), ids: self.ids.clone(), email: self.email.clone() }
    }
}

#[derive(Debug, Default, Deserialize)]
struct Manifest {
    #[serde(rename = "client", default)]
    clients: Vec<Profile>,
}

/// The profiles of a notes folder.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Clients {
    pub list: Vec<Profile>,
}

impl Clients {
    /// The notes folder `root`'s profiles; none when the file is not there.
    /// Then, once the store is open in this program, every client of the
    /// store as a profile whose id is its address (`sioul:client/<id>`), so
    /// that a draft naming a client by its address finds its billing identity
    /// wherever profiles are read (`resolve`); a profile of the file comes first.
    /// While the store is sealed here and cannot be opened yet (the keyring
    /// locked or late), an error: wait, never bill a client by its address
    /// as if no client existed.
    pub fn load(root: &Path) -> Result<Clients, String> {
        Clients::load_in(root, &door::opening())
    }

    /// `load` with the store as it stands (`door::opening`).
    pub fn load_in(root: &Path, opening: &door::Opening) -> Result<Clients, String> {
        let clients = Clients::read_file(root)?;
        match opening {
            door::Opening::Open(door) => match record::list(door.as_ref()) {
                Ok(read) => Ok(clients.with_clients(&read.items)),
                Err(e) => Err(e.to_string()),
            },
            door::Opening::NoStore => Ok(clients),
            door::Opening::Wait => Err(Error::NotOpen.to_string()),
        }
    }

    /// These profiles, then each client of the store as a profile whose id is
    /// its address; a client whose profile is still here is that profile,
    /// never listed twice; one whose profile was adopted answers to its old
    /// id too, so that a draft or a project naming it still finds it.
    ///
    /// A client the move or an extension made from a card keeps no name of
    /// its own: `resolve` bills it with its card's name as the card says it
    /// now, and the invoices' picker lists and finds it by that name
    /// (`name_shown`), so that a card renamed is followed (the re-review, R2).
    /// Its billing written from the window goes to the client
    /// (`change_where_it_lives`), never "changed elsewhere" (the review of
    /// the finish, 2).
    pub fn with_clients(mut self, clients: &[record::Client]) -> Clients {
        let mut store = Vec::new();
        for c in clients.iter().filter(|c| c.listed() && c.profile_in(&self).is_none()) {
            let profile = c.as_profile();
            if !c.profile.is_empty() {
                store.push(Profile { id: c.profile.clone(), ..profile.clone() });
            }
            store.push(profile);
        }
        self.list.extend(store);
        self
    }

    /// The profiles of the file alone; none when it is not there.
    pub fn read_file(root: &Path) -> Result<Clients, String> {
        let path = root.join(FILE);
        match crate::files::read_to_string(&path) {
            Ok(text) => toml::from_str::<Manifest>(&text).map(|m| Clients { list: m.clients }).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Clients::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The profile tied to the contact `uid`.
    pub fn of_contact(&self, uid: &str) -> Option<&Profile> {
        let address = crate::links::contact_uri(uid);
        self.list.iter().find(|p| p.contact == address)
    }

    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.list.iter().find(|p| p.id == id)
    }
}

/// The file of the notes folder `root`.
pub fn file_in(root: &Path) -> PathBuf {
    root.join(FILE)
}

/// A profile written into `file` (a new one when its id is empty or not
/// there), every other profile and comment kept, under the file's lock,
/// which the sharing takes too. Returns its id.
pub fn save(file: &Path, profile: &Profile) -> Result<String, String> {
    crate::filelock::with_lock(file, || {
        let fail = |e: String| format!("{}: {e}", file.display());
        let text = match crate::files::read_to_string(file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(fail(e.to_string())),
        };
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let mut profile = profile.clone();
        if profile.id.trim().is_empty() {
            profile.id = crate::ids::new();
        }
        // The profile as a table of its own, then put in its place.
        let own: DocumentMut = toml::to_string(&profile).map_err(|e| fail(e.to_string()))?.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        // Its address and identifiers inline (`address = { … }`), so that each
        // profile stays one table wherever it sits in the file: tables of a
        // table read elsewhere keep their place there and would come apart.
        let mut table = toml_edit::Table::new();
        for (key, item) in own.as_table().iter() {
            let item = match item.clone() {
                Item::Table(t) => Item::Value(toml_edit::Value::InlineTable(t.into_inline_table())),
                Item::ArrayOfTables(list) => Item::Value(toml_edit::Value::Array(list.into_array())),
                other => other,
            };
            table.insert(key, item);
        }
        let list = doc.entry(TABLE).or_insert(Item::ArrayOfTables(ArrayOfTables::new())).as_array_of_tables_mut().ok_or_else(|| fail(format!("`{TABLE}` is not a list of tables")))?;
        let at = list.iter().position(|t| t.get("id").and_then(Item::as_str) == Some(profile.id.as_str()));
        match at {
            Some(at) => *list.get_mut(at).expect("found") = table,
            None => list.push(table),
        }
        let temporary = file.with_extension("toml.new");
        crate::files::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
        crate::files::rename(&temporary, file).map_err(|e| fail(e.to_string()))?;
        crate::changes::said(crate::changes::Kind::Invoices);
        Ok(profile.id)
    })
}

/// What a profile's form changed, each field when it changed; an empty
/// text takes the key out (its default comes back).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ProfileChange {
    /// A new profile's contact (`sioul:contact/<uid>`).
    pub contact: Option<String>,
    pub kind: Option<String>,
    pub name: Option<String>,
    /// The billing address, field by field.
    pub lines: Option<Vec<String>>,
    pub postcode: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    /// The delivery address, field by field; all empty: no delivery address.
    pub delivery_lines: Option<Vec<String>>,
    pub delivery_postcode: Option<String>,
    pub delivery_city: Option<String>,
    pub delivery_country: Option<String>,
    /// Every identifier, with whether it is printed.
    pub ids: Option<Vec<Identifier>>,
    pub language: Option<String>,
    pub currency: Option<String>,
    /// Days; "" for the settings'.
    pub terms_days: Option<String>,
    pub end_of_month: Option<bool>,
    pub email: Option<String>,
    pub order: Option<String>,
}

impl ProfileChange {
    /// A new profile made of it.
    fn profile(&self) -> Profile {
        let text = |o: &Option<String>| o.as_deref().unwrap_or("").trim().to_string();
        let delivery = Address { lines: self.delivery_lines.clone().unwrap_or_default(), postcode: text(&self.delivery_postcode), city: text(&self.delivery_city), country: text(&self.delivery_country) };
        Profile {
            id: String::new(),
            contact: text(&self.contact),
            kind: self.kind.clone().filter(|k| !k.trim().is_empty()).unwrap_or_else(business),
            name: text(&self.name),
            address: Address { lines: self.lines.clone().unwrap_or_default(), postcode: text(&self.postcode), city: text(&self.city), country: text(&self.country) },
            delivery: (!delivery.is_empty() || !delivery.country.is_empty()).then_some(delivery),
            ids: self.ids.clone().unwrap_or_default(),
            language: text(&self.language),
            currency: text(&self.currency),
            terms_days: self.terms_days.as_deref().and_then(|t| t.trim().parse().ok()),
            end_of_month: self.end_of_month.filter(|e| *e),
            email: text(&self.email),
            order: text(&self.order),
        }
    }
}

/// What the form changed written into the profile `id` of `file`, key by
/// key (`ProfileChange`): every other key of it, its comments, an
/// identifier's `printed`, the delivery address's fields, stay as they
/// were written, by hand or by a newer Sioul (the review of phase 3, F6). A
/// profile not there yet is made. Under the file's lock, as `save`.
/// Returns its id.
pub fn change(file: &Path, id: &str, change: &ProfileChange) -> Result<String, String> {
    let found = crate::filelock::with_lock(file, || -> Result<Option<String>, String> {
        let fail = |e: String| format!("{}: {e}", file.display());
        let Ok(text) = crate::files::read_to_string(file) else { return Ok(None) };
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let Some(list) = doc.get_mut(TABLE).and_then(Item::as_array_of_tables_mut) else { return Ok(None) };
        let Some(table) = list.iter_mut().find(|t| !id.is_empty() && t.get("id").and_then(Item::as_str) == Some(id)) else { return Ok(None) };
        apply_change(table, change);
        // What it now says must read back as a profile, else nothing is written.
        toml::from_str::<Manifest>(&doc.to_string()).map_err(|e| fail(e.to_string()))?;
        let temporary = file.with_extension("toml.new");
        crate::files::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
        crate::files::rename(&temporary, file).map_err(|e| fail(e.to_string()))?;
        crate::changes::said(crate::changes::Kind::Invoices);
        Ok(Some(id.to_string()))
    })?;
    match found {
        Some(id) => Ok(id),
        // A new profile is made of what the form says; one that is no longer
        // there (taken out elsewhere while its form was open, or written by
        // hand without an id) is never made again of the changed keys alone
        // (the second review of phase 3, R15).
        None if id.is_empty() => save(file, &change.profile()),
        None => Err(GONE.to_string()),
    }
}

/// A billing form's change written where that billing identity lives: a
/// profile of `file` by its id; a client of the store by its address
/// (`sioul:client/<id>`, as `Clients::load` lists one) or by the id of the
/// profile it adopted, through `record::change_billing`, so that the profile
/// form and a draft's client kind or country never answer "changed elsewhere"
/// for a client (the review of the finish, 2). The id written, or `GONE`.
pub fn change_where_it_lives(file: &Path, id: &str, change: &ProfileChange) -> Result<String, String> {
    let opening = door::opening();
    change_where_it_lives_with(file, id, change, opening.door().ok().as_deref())
}

/// `change_where_it_lives` through `door` (none: no store open here, so no client but profiles).
pub fn change_where_it_lives_with(file: &Path, id: &str, change: &ProfileChange, door: Option<&dyn door::Door>) -> Result<String, String> {
    let id = id.trim();
    let client_id = id.strip_prefix(address::PREFIX).map(str::to_string);
    if client_id.is_none() {
        match self::change(file, id, change) {
            Err(e) if e == GONE => {}
            done => return done,
        }
    }
    let Some(door) = door else { return Err(if client_id.is_some() { Error::NotOpen.to_string() } else { GONE.to_string() }) };
    let client = match &client_id {
        Some(cid) => record::find(door, cid).map_err(|e| e.to_string())?,
        // A profile adopted since its form opened: the client that names it.
        None => record::list(door).map_err(|e| e.to_string())?.items.into_iter().find(|c| !id.is_empty() && c.profile == id),
    };
    let Some(client) = client else { return Err(GONE.to_string()) };
    record::change_billing(door, &client.id, change, Some(file)).map_err(|e| e.to_string())?;
    Ok(if client_id.is_some() { id.to_string() } else { client.uri() })
}

/// What a billing form changed written into `table`, key by key
/// (`ProfileChange`): every other key, its comments, an identifier's
/// `printed` and the delivery address's fields stay as they were written.
/// A profile's table and a client's `[billing]` take it alike.
pub(crate) fn apply_change(table: &mut toml_edit::Table, change: &ProfileChange) {
    let texts = |table: &mut dyn toml_edit::TableLike, key: &str, value: &Option<String>| {
        if let Some(v) = value {
            if v.trim().is_empty() {
                table.remove(key);
            } else {
                put(table, key, toml_edit::value(v.trim()));
            }
        }
    };
    texts(table, "kind", &change.kind);
    texts(table, "name", &change.name);
    texts(table, "language", &change.language);
    texts(table, "currency", &change.currency.as_ref().map(|c| c.trim().to_uppercase()));
    texts(table, "email", &change.email);
    texts(table, "order", &change.order);
    // An address, field by field, in its own table as it was written (inline or not).
    let address = |table: &mut toml_edit::Table, key: &str, lines: &Option<Vec<String>>, fields: [(&str, &Option<String>); 3]| {
        if lines.is_none() && fields.iter().all(|(_, v)| v.is_none()) {
            return;
        }
        if !table.contains_key(key) {
            table.insert(key, toml_edit::value(toml_edit::InlineTable::new()));
        }
        let Some(at) = table.get_mut(key).and_then(Item::as_table_like_mut) else { return };
        if let Some(lines) = lines {
            let kept: toml_edit::Array = lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            put(at, "lines", toml_edit::value(kept));
        }
        for (field, value) in fields {
            texts(at, field, value);
        }
        let empty = at.iter().all(|(_, v)| v.as_str().is_some_and(|s| s.trim().is_empty()) || v.as_array().is_some_and(|a| a.is_empty()));
        if empty && key == "delivery" {
            table.remove(key);
        }
    };
    address(table, "address", &change.lines, [("postcode", &change.postcode), ("city", &change.city), ("country", &change.country)]);
    address(table, "delivery", &change.delivery_lines, [("postcode", &change.delivery_postcode), ("city", &change.delivery_city), ("country", &change.delivery_country)]);
    if let Some(ids) = &change.ids {
        let mut array = toml_edit::Array::new();
        for i in ids.iter().filter(|i| !i.value.trim().is_empty()) {
            let mut one = toml_edit::InlineTable::new();
            one.insert("kind", i.kind.trim().into());
            one.insert("value", i.value.trim().into());
            one.insert("printed", i.printed.into());
            array.push(one);
        }
        if array.is_empty() {
            table.remove("ids");
        } else {
            put(table, "ids", toml_edit::value(array));
        }
    }
    if let Some(days) = &change.terms_days {
        match days.trim().parse::<i64>() {
            Ok(n) if n >= 0 => put(table, "terms_days", toml_edit::value(n)),
            _ => {
                table.remove("terms_days");
            }
        }
    }
    if let Some(end) = change.end_of_month {
        if end {
            put(table, "end_of_month", toml_edit::value(true));
        } else {
            table.remove("end_of_month");
        }
    }
}

/// What `change` answers for a profile that is no longer there (said in words by the window).
pub const GONE: &str = "this profile is no longer there";

/// `item` written at `key` of `table`: a key there keeps its place, the
/// comments before it and the spaces around its value.
fn put(table: &mut dyn toml_edit::TableLike, key: &str, item: Item) {
    match table.get_mut(key) {
        Some(old) => {
            let mut item = item;
            if let (Some(was), Some(now)) = (old.as_value(), item.as_value_mut()) {
                *now.decor_mut() = was.decor().clone();
            }
            *old = item;
        }
        None => {
            table.insert(key, item);
        }
    }
}

/// A profile taken out of `file`; Ok(false) when it was not there.
pub fn remove(file: &Path, id: &str) -> Result<bool, String> {
    crate::filelock::with_lock(file, || {
        let fail = |e: String| format!("{}: {e}", file.display());
        let Ok(text) = crate::files::read_to_string(file) else { return Ok(false) };
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let Some(list) = doc.get_mut(TABLE).and_then(Item::as_array_of_tables_mut) else { return Ok(false) };
        let Some(at) = list.iter().position(|t| t.get("id").and_then(Item::as_str) == Some(id)) else { return Ok(false) };
        list.remove(at);
        let temporary = file.with_extension("toml.new");
        crate::files::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
        crate::files::rename(&temporary, file).map_err(|e| fail(e.to_string()))?;
        crate::changes::said(crate::changes::Kind::Invoices);
        Ok(true)
    })
}

/// Who a project's client is, for an invoice: the profile of the contact it
/// names (`sioul:contact/<uid>`, or a card's `urn:uuid:<uid>` as a task's
/// `CONTACT` holds it since 26 November 2026: `contacts::card_named`) or of
/// the name it gives; without one, the
/// contact's card (its organisation and name, its first postal address, its
/// country when the address's last line names one), or the name as written.
/// Without a profile, the client is billed as a business, whatever its card
/// says: the side that keeps the lines a business's invoice must carry,
/// harmless on a person's (the second review, G8); only a profile saying
/// "consumer" bills a person. The draft says the kind, so that a profile
/// can change it. The profile comes back too, for its language, currency
/// and terms.
pub fn resolve<'a>(reference: &str, clients: &'a Clients, contacts: &[Contact]) -> (Client, Option<&'a Profile>) {
    let reference = reference.trim();
    let uid = reference.strip_prefix("sioul:contact/").map(crate::notes::decode);
    let by_address = crate::contacts::card_key(reference).is_some();
    let contact = match by_address {
        true => crate::contacts::card_named(contacts, reference),
        false => contacts.iter().find(|c| !reference.is_empty() && (c.name.eq_ignore_ascii_case(reference) || c.org.eq_ignore_ascii_case(reference))),
    };
    let profile = uid
        .as_deref()
        .and_then(|uid| clients.of_contact(uid))
        .or_else(|| contact.and_then(|c| clients.of_contact(&c.uid)))
        // A card not here, named either way: the profile of that card's address.
        .or_else(|| by_address.then(|| clients.list.iter().find(|p| crate::contacts::same_card(&p.contact, reference))).flatten())
        .or_else(|| clients.list.iter().find(|p| !reference.is_empty() && (p.name.eq_ignore_ascii_case(reference) || p.id == reference)));
    if let Some(profile) = profile {
        // A client of the store named by its address: its card is its profile's.
        let contact = contact.or_else(|| profile.contact.strip_prefix("sioul:contact/").map(crate::notes::decode).and_then(|uid| contacts.iter().find(|c| c.uid == uid)));
        let mut client = profile.client();
        if client.name.trim().is_empty()
            && let Some(card) = contact
        {
            client.name = card_name(card);
        }
        if client.email.is_empty()
            && let Some(card) = contact
        {
            client.email = card.emails.first().map(|e| e.value.clone()).unwrap_or_default();
        }
        // A client of the store: what its billing lacks taken from its card,
        // never what it says. Its postal lines, postcode and city from the
        // card's address when it has none of them (one the move made from a
        // card, as a draft naming that card took them before: the second
        // review, A); its country from the card only when it says none, so
        // that a country set in the invoice editor, which decides VAT, is
        // never replaced (the re-review, R1).
        if profile.id.starts_with("sioul:client/")
            && let Some(card) = contact
        {
            let from_card = card_address(&card.addresses.first().map(|a| a.value.clone()).unwrap_or_default());
            let postal_none = client.address.lines.iter().all(|l| l.trim().is_empty()) && client.address.postcode.trim().is_empty() && client.address.city.trim().is_empty();
            if postal_none {
                client.address.lines = from_card.lines;
                client.address.postcode = from_card.postcode;
                client.address.city = from_card.city;
            }
            if client.address.country.trim().is_empty() {
                client.address.country = from_card.country;
            }
        }
        return (client, Some(profile));
    }
    match contact {
        Some(card) => (
            Client {
                kind: "business".into(),
                name: card_name(card),
                address: card_address(&card.addresses.first().map(|a| a.value.clone()).unwrap_or_default()),
                email: card.emails.first().map(|e| e.value.clone()).unwrap_or_default(),
                ..Client::default()
            },
            None,
        ),
        None => (Client { kind: "business".into(), name: reference.to_string(), ..Client::default() }, None),
    }
}

/// The name a profile is shown and searched by: its own, else its card's
/// as the card says it now (a client made from a card keeps no name of its
/// own, so that a card renamed is followed: the re-review, R2).
pub fn name_shown(profile: &Profile, contacts: &[Contact]) -> String {
    if !profile.name.trim().is_empty() {
        return profile.name.clone();
    }
    crate::contacts::card_named(contacts, &profile.contact).map(card_name).unwrap_or_default()
}

/// A card's postal address, on its lines; its last line, when it names a
/// country ("Belgique", "Deutschland", "Maroc"), becomes the address's
/// country (`country`). A last line that names none Sioul knows and is no
/// postal line (one with a digit: a postcode) stays, and says the country
/// is not known (`country_unsaid`).
pub fn card_address(text: &str) -> Address {
    let mut address = Address::from_text(text);
    if let Some(found) = address.lines.last().and_then(|l| country(l)) {
        address.lines.pop();
        address.country = found.code.to_string();
    }
    address
}

/// Whether an address without a country says none Sioul can read: its last
/// line is no postal line (no digit, so no postcode), and names no country
/// Sioul knows ("Narnia", "Québec"): where it is cannot be told. An address
/// whose last line is postal is written for the issuer's own post.
pub fn country_unsaid(address: &Address) -> bool {
    if !address.country.trim().is_empty() || !address.postcode.trim().is_empty() || !address.city.trim().is_empty() {
        return false;
    }
    address.lines.iter().rev().find(|l| !l.trim().is_empty()).is_some_and(|last| !last.chars().any(|c| c.is_ascii_digit()) && country(last).is_none())
}

/// A country invoices know (ISO 3166-1): its code, and its name in English and in French.
#[derive(Debug, PartialEq, Eq)]
pub struct Country {
    pub code: &'static str,
    pub en: &'static str,
    pub fr: &'static str,
}

impl Country {
    /// Its name as an invoice in `language` prints it: French, else English.
    pub fn name(&self, language: &str) -> &'static str {
        if language.starts_with("fr") { self.fr } else { self.en }
    }
}

/// Every country with its names (`data/countries.tsv`, from the iso-codes
/// project): code, English name, French name, other names.
const COUNTRIES: &str = include_str!("../data/countries.tsv");

/// The country `text` names: a code ("BE", "BEL"), or a name in English,
/// French, German, Spanish, Italian, Dutch, Portuguese or the country's own
/// languages ("Belgique", "Deutschland", "Maroc", "日本"); None for any other
/// text. The one table of countries: an invoice's checks, its zone and its
/// Factur-X data read it too.
pub fn country(text: &str) -> Option<&'static Country> {
    static TABLE: std::sync::OnceLock<(Vec<Country>, std::collections::HashMap<String, usize>)> = std::sync::OnceLock::new();
    let (list, names) = TABLE.get_or_init(|| {
        let mut list = Vec::new();
        let mut names = std::collections::HashMap::new();
        for line in COUNTRIES.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let mut fields = line.split('\t');
            let (Some(code), Some(en), Some(fr)) = (fields.next(), fields.next(), fields.next()) else { continue };
            let others = fields.next().unwrap_or("");
            let at = list.len();
            for name in [code, en, fr].into_iter().chain(others.split('|')) {
                names.entry(folded_name(name)).or_insert(at);
            }
            list.push(Country { code, en, fr });
        }
        (list, names)
    });
    let key = folded_name(text);
    if key.is_empty() {
        return None;
    }
    names.get(&key).map(|at| &list[*at])
}

/// A name as countries are looked up: folded, its ends' spaces and dots taken off.
fn folded_name(text: &str) -> String {
    let folded: String = crate::text::fold(text.trim()).into_iter().collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string()
}

/// A card's name as invoices print it: its organisation, then the person.
pub(crate) fn card_name(card: &Contact) -> String {
    if card.org.is_empty() || card.org == card.name { card.name.clone() } else { format!("{}\n{}", card.org, card.name) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contacts::Labeled;

    #[test]
    fn a_profile_is_found_by_its_contact_or_its_name() {
        let root = std::env::temp_dir().join(format!("sioul-clients-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file = file_in(&root);
        std::fs::write(&file, "# Who I bill.\n").unwrap();
        let lumen = Profile {
            contact: "sioul:contact/anna-1".into(),
            name: "Studio Lumen SARL".into(),
            address: Address { lines: vec!["1 rue de l’Exemple".into()], postcode: "75002".into(), city: "Paris".into(), country: "FR".into() },
            ids: vec![Identifier { kind: "siren".into(), value: "904 123 452".into(), printed: true }],
            language: "fr".into(),
            ..Profile::default()
        };
        let id = save(&file, &lumen).unwrap();
        let person = Profile { kind: "consumer".into(), name: "Robin Example".into(), language: "en".into(), ..Profile::default() };
        save(&file, &person).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("# Who I bill.\n") && text.matches("[[client]]").count() == 2, "{text}");
        let clients = Clients::load(&root).unwrap();
        assert_eq!(clients.list.len(), 2);
        // Changed in place.
        save(&file, &Profile { id: id.clone(), order: "PO-77".into(), ..lumen.clone() }).unwrap();
        let clients = Clients::load(&root).unwrap();
        assert_eq!((clients.list.len(), clients.get(&id).unwrap().order.as_str()), (2, "PO-77"));
        let anna = Contact { uid: "anna-1".into(), name: "Anna Exemple".into(), org: "Studio Lumen".into(), emails: vec![Labeled { label: String::new(), value: "anna@example.org".into() }], ..Contact::default() };
        let (client, found) = resolve("sioul:contact/anna-1", &clients, std::slice::from_ref(&anna));
        assert_eq!((client.name.as_str(), client.email.as_str(), found.map(|p| p.language.as_str())), ("Studio Lumen SARL", "anna@example.org", Some("fr")));
        // The card named as a task's contact is written since 26 November 2026: the same client.
        assert_eq!(resolve("urn:uuid:anna-1", &clients, std::slice::from_ref(&anna)), (client.clone(), found));
        assert_eq!(resolve("urn:uuid:anna-1", &clients, &[]).1.map(|p| p.id.as_str()), Some(id.as_str()), "its profile, the card not here");
        assert_eq!(resolve("Studio Lumen", &clients, std::slice::from_ref(&anna)).1.map(|p| p.id.as_str()), Some(id.as_str()), "by the card's organisation");
        assert_eq!(resolve("robin example", &clients, &[]).0.kind, "consumer");
        // No profile: a card with an organisation is a business, its country from its address's last line.
        let brussels = Contact { uid: "b-1".into(), name: "Léa Exemple".into(), org: "Atelier Bruxelles".into(), addresses: vec![Labeled { label: String::new(), value: "Rue de l’Exemple 1\n1000 Bruxelles\nBelgique".into() }], ..Contact::default() };
        let (client, _) = resolve("sioul:contact/b-1", &Clients::default(), std::slice::from_ref(&brussels));
        assert_eq!((client.kind.as_str(), client.address.country.as_str(), client.address.lines.len()), ("business", "BE", 2));
        // A card without an organisation, a name alone: a business all the same (G8); only a profile makes a person.
        let person = Contact { uid: "p-1".into(), name: "Sam Exemple".into(), ..Contact::default() };
        assert_eq!(resolve("sioul:contact/p-1", &Clients::default(), std::slice::from_ref(&person)).0.kind, "business");
        assert_eq!(resolve("Garage Exemple", &Clients::default(), &[]).0.kind, "business");
        // No profile: the card, as before; else the name as written.
        let nobody = Clients::default();
        let (card, none) = resolve("sioul:contact/anna-1", &nobody, std::slice::from_ref(&anna));
        assert_eq!((card.name.as_str(), none.is_none()), ("Studio Lumen\nAnna Exemple", true));
        assert_eq!(resolve("Fernhill", &Clients::default(), &[]).0.name, "Fernhill");
        assert!(remove(&file, &id).unwrap());
        assert_eq!(Clients::load(&root).unwrap().list.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The profile's form changes only what it changed, key by key (the
    /// review of phase 3, F6): comments, keys a newer Sioul wrote, an
    /// identifier left unprinted and the delivery address's fields stay.
    #[test]
    fn a_profile_s_form_changes_only_what_it_changed() {
        let root = std::env::temp_dir().join(format!("sioul-clients-change-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file = file_in(&root);
        let written = "# Who I bill.\n\n[[client]]\nid = \"lumen\"\n# Since the first job.\nname = \"Studio Lumen SARL\"\nkind = \"business\"\nmood = \"kept by a newer Sioul\"\naddress = { lines = [\"1 rue de l’Exemple\"], postcode = \"75002\", city = \"Paris\", country = \"FR\" }\nids = [{ kind = \"siren\", value = \"904 123 452\", printed = false }]\nterms_days = 45\n\n[client.delivery]\nlines = [\"Entrepôt\", \"2 quai de l’Exemple\"]\npostcode = \"75004\"\ncity = \"Paris\"\n";
        std::fs::write(&file, written).unwrap();
        let only = ProfileChange { name: Some("Studio Lumen".into()), city: Some("Lyon".into()), postcode: Some("69002".into()), ..ProfileChange::default() };
        assert_eq!(change(&file, "lumen", &only).unwrap(), "lumen");
        let text = std::fs::read_to_string(&file).unwrap();
        for kept in ["# Who I bill.", "# Since the first job.", "mood = \"kept by a newer Sioul\"", "printed = false", "terms_days = 45", "\"Entrepôt\"", "postcode = \"75004\"", "lines = [\"1 rue de l’Exemple\"]"] {
            assert!(text.contains(kept), "{kept} kept: {text}");
        }
        let clients = Clients::load(&root).unwrap();
        let lumen = clients.get("lumen").unwrap();
        assert_eq!((lumen.name.as_str(), lumen.address.city.as_str(), lumen.address.postcode.as_str(), lumen.address.country.as_str()), ("Studio Lumen", "Lyon", "69002", "FR"));
        assert_eq!((lumen.delivery.as_ref().map(|d| d.city.as_str()), lumen.ids[0].printed, lumen.terms_days), (Some("Paris"), false, Some(45)));
        // An emptied delivery address is taken out; terms emptied go back to the settings'.
        let emptied = ProfileChange { delivery_lines: Some(Vec::new()), delivery_postcode: Some(String::new()), delivery_city: Some(String::new()), terms_days: Some(String::new()), ..ProfileChange::default() };
        change(&file, "lumen", &emptied).unwrap();
        let clients = Clients::load(&root).unwrap();
        let lumen = clients.get("lumen").unwrap();
        assert_eq!((lumen.delivery.is_none(), lumen.terms_days, lumen.ids[0].printed), (true, None, false));
        assert!(std::fs::read_to_string(&file).unwrap().contains("mood = "));
        // One that is no longer there is refused, never made again of the changed keys alone (R15).
        assert_eq!(change(&file, "gone", &ProfileChange { email: Some("accounts@example.org".into()), ..ProfileChange::default() }), Err(GONE.to_string()));
        assert_eq!(Clients::load(&root).unwrap().list.len(), 1);
        // A profile not there yet is made of what the form says.
        let made = change(&file, "", &ProfileChange { name: Some("Robin Example".into()), kind: Some("consumer".into()), ..ProfileChange::default() }).unwrap();
        assert_eq!(Clients::load(&root).unwrap().get(&made).map(|p| p.kind.as_str()), Some("consumer"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Countries (G2): by their codes and their names in many languages, as
    /// an address's last line writes them; printed in the invoice's
    /// language; a last line that names none and is no postal line says the
    /// country is not known.
    #[test]
    fn countries_are_read_by_their_names() {
        let code = |text: &str| country(text).map(|c| c.code);
        assert_eq!((code("Belgique"), code(" Deutschland "), code("BE"), code("Royaume-Uni"), code("Maroc"), code("日本"), code("Narnia")), (Some("BE"), Some("DE"), Some("BE"), Some("GB"), Some("MA"), Some("JP"), None));
        assert_eq!((code("USA"), code("États-Unis"), code("Pays-Bas"), code("Suisse"), code("MONACO")), (Some("US"), Some("US"), Some("NL"), Some("CH"), Some("MC")));
        // The codes an invoice's data writes (phase 2, P2-5): "UK" is GB, "EL"
        // (Greece's in VAT numbers) GR, a code in small letters its own; no
        // other text is a country, Northern Ireland's VAT prefix XI included.
        assert_eq!((code("UK"), code("EL"), code(" be "), code("ZZ"), code("XI"), code("")), (Some("GB"), Some("GR"), Some("BE"), None, None, None));
        assert_eq!((country("MA").unwrap().name("fr"), country("MA").unwrap().name("en")), ("Maroc", "Morocco"));
        assert!(country_unsaid(&Address::from_text("1 Example Way\nNarnia")));
        assert!(!country_unsaid(&Address::from_text("1 rue de l’Exemple\n75002 Paris")), "a postal line: written for the issuer's own post");
        assert!(!country_unsaid(&card_address("Rue de l’Exemple 1\n1000 Bruxelles\nBelgique")));
    }
}
