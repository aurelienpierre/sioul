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

use crate::contacts::Contact;
use crate::invoice::{Address, Client, Identifier};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item};

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
    pub fn load(root: &Path) -> Result<Clients, String> {
        let path = root.join(FILE);
        match std::fs::read_to_string(&path) {
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
        let text = match std::fs::read_to_string(file) {
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
        std::fs::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
        std::fs::rename(&temporary, file).map_err(|e| fail(e.to_string()))?;
        crate::changes::said(crate::changes::Kind::Projects);
        Ok(profile.id)
    })
}

/// A profile taken out of `file`; Ok(false) when it was not there.
pub fn remove(file: &Path, id: &str) -> Result<bool, String> {
    crate::filelock::with_lock(file, || {
        let fail = |e: String| format!("{}: {e}", file.display());
        let Ok(text) = std::fs::read_to_string(file) else { return Ok(false) };
        let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| fail(e.to_string()))?;
        let Some(list) = doc.get_mut(TABLE).and_then(Item::as_array_of_tables_mut) else { return Ok(false) };
        let Some(at) = list.iter().position(|t| t.get("id").and_then(Item::as_str) == Some(id)) else { return Ok(false) };
        list.remove(at);
        let temporary = file.with_extension("toml.new");
        std::fs::write(&temporary, doc.to_string()).map_err(|e| fail(e.to_string()))?;
        std::fs::rename(&temporary, file).map_err(|e| fail(e.to_string()))?;
        crate::changes::said(crate::changes::Kind::Projects);
        Ok(true)
    })
}

/// Who a project's client is, for an invoice: the profile of the contact it
/// names (`sioul:contact/<uid>`) or of the name it gives; without one, the
/// contact's card (its organisation and name, its first postal address, its
/// country when the address's last line names one), or the name as written.
/// Without a profile, the client's kind is guessed: a card with an
/// organisation is a business, else a person; the invoice keeps the guess
/// and says it, and a profile corrects it. The profile comes back too, for
/// its language, currency and terms.
pub fn resolve<'a>(reference: &str, clients: &'a Clients, contacts: &[Contact]) -> (Client, Option<&'a Profile>) {
    let reference = reference.trim();
    let uid = reference.strip_prefix("sioul:contact/").map(crate::notes::decode);
    let contact = match &uid {
        Some(uid) => contacts.iter().find(|c| c.uid == *uid),
        None => contacts.iter().find(|c| !reference.is_empty() && (c.name.eq_ignore_ascii_case(reference) || c.org.eq_ignore_ascii_case(reference))),
    };
    let profile = uid
        .as_deref()
        .and_then(|uid| clients.of_contact(uid))
        .or_else(|| contact.and_then(|c| clients.of_contact(&c.uid)))
        .or_else(|| clients.list.iter().find(|p| !reference.is_empty() && (p.name.eq_ignore_ascii_case(reference) || p.id == reference)));
    if let Some(profile) = profile {
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
        return (client, Some(profile));
    }
    match contact {
        Some(card) => (
            Client {
                kind: if card.org.trim().is_empty() { "consumer".into() } else { "business".into() },
                name: card_name(card),
                address: card_address(&card.addresses.first().map(|a| a.value.clone()).unwrap_or_default()),
                email: card.emails.first().map(|e| e.value.clone()).unwrap_or_default(),
                ..Client::default()
            },
            None,
        ),
        None => (Client { kind: "consumer".into(), name: reference.to_string(), ..Client::default() }, None),
    }
}

/// A card's postal address, on its lines; its last line, when it names a
/// country ("Belgique", "Deutschland"), becomes the address's country.
pub fn card_address(text: &str) -> Address {
    let mut address = Address::from_text(text);
    if let Some(code) = address.lines.last().and_then(|l| crate::invoice::presets::country_code(l)) {
        address.lines.pop();
        address.country = code.to_string();
    }
    address
}

/// A card's name as invoices print it: its organisation, then the person.
fn card_name(card: &Contact) -> String {
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
        assert_eq!(resolve("Studio Lumen", &clients, std::slice::from_ref(&anna)).1.map(|p| p.id.as_str()), Some(id.as_str()), "by the card's organisation");
        assert_eq!(resolve("robin example", &clients, &[]).0.kind, "consumer");
        // No profile: a card with an organisation is a business, its country from its address's last line.
        let brussels = Contact { uid: "b-1".into(), name: "Léa Exemple".into(), org: "Atelier Bruxelles".into(), addresses: vec![Labeled { label: String::new(), value: "Rue de l’Exemple 1\n1000 Bruxelles\nBelgique".into() }], ..Contact::default() };
        let (client, _) = resolve("sioul:contact/b-1", &Clients::default(), std::slice::from_ref(&brussels));
        assert_eq!((client.kind.as_str(), client.address.country.as_str(), client.address.lines.len()), ("business", "BE", 2));
        let person = Contact { uid: "p-1".into(), name: "Sam Exemple".into(), ..Contact::default() };
        assert_eq!(resolve("sioul:contact/p-1", &Clients::default(), std::slice::from_ref(&person)).0.kind, "consumer", "a person");
        // No profile: the card, as before; else the name as written.
        let nobody = Clients::default();
        let (card, none) = resolve("sioul:contact/anna-1", &nobody, std::slice::from_ref(&anna));
        assert_eq!((card.name.as_str(), none.is_none()), ("Studio Lumen\nAnna Exemple", true));
        assert_eq!(resolve("Fernhill", &Clients::default(), &[]).0.name, "Fernhill");
        assert!(remove(&file, &id).unwrap());
        assert_eq!(Clients::load(&root).unwrap().list.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
