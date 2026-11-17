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
/// Without a profile, the client is billed as a business, whatever its card
/// says: the side that keeps the lines a business's invoice must carry,
/// harmless on a person's (the second review, G8); only a profile saying
/// "consumer" bills a person. The draft says the kind, so that a profile
/// can change it. The profile comes back too, for its language, currency
/// and terms.
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
