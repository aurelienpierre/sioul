// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The options that protect (docs/clients.md, "What protects"), stored in
//! `[clients.protect]`, and what the core enforces of them. **Lean and
//! permissive first**: every option is off but "rules may act on the
//! files"; nothing that reaches agents or rules today is closed by the mere
//! fact that someone is a client. "Clients' files stay on this device" is not
//! here: it is the part's own switch in Settings ▸ Sharing, its one place.

use serde::{Deserialize, Serialize};

/// `[clients.protect]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Protect {
    /// "AI agents may read and write clients' files" (off).
    pub agents_files: bool,
    /// "Rules may act on clients' files" (on).
    pub rules_files: bool,
    /// "Keep from AI agents what is tied to a client" (off).
    pub keep_tied: bool,
    /// "Is yours a health profession bound by secrecy where you work?":
    /// "yes", "no", or "" while not asked (asked once, when a health mark is
    /// first used).
    pub health_profession: String,
    /// "Archive by itself" at the end of `active_years` (off).
    pub archive_by_itself: bool,
    /// Years after the last contact a client stays in the lists; 0 not set.
    pub active_years: u32,
    /// Years after the last contact a file is kept, then offered for erasing once a year; 0 not set.
    pub kept_years: u32,
}

impl Default for Protect {
    fn default() -> Protect {
        Protect { agents_files: false, rules_files: true, keep_tied: false, health_profession: String::new(), archive_by_itself: false, active_years: 0, kept_years: 0 }
    }
}

impl Protect {
    /// Whether the person said theirs is a health profession bound by secrecy.
    pub fn health_profession(&self) -> bool {
        self.health_profession == "yes"
    }

    /// `[clients.protect]` as written, key by key, **failing closed**: a key
    /// that does not read takes its protective value (nothing to agents,
    /// nothing done by rules, what is tied kept), never the default of all.
    pub fn of(value: &toml::Value) -> Protect {
        let closed = Protect { agents_files: false, rules_files: false, keep_tied: true, ..Protect::default() };
        let Some(t) = value.as_table() else { return closed };
        let flag = |key: &str, default: bool, protective: bool| match t.get(key) {
            None => default,
            Some(v) => v.as_bool().unwrap_or(protective),
        };
        let years = |key: &str| t.get(key).and_then(toml::Value::as_integer).and_then(|n| u32::try_from(n).ok()).unwrap_or(0);
        let d = Protect::default();
        Protect {
            agents_files: flag("agents_files", d.agents_files, false),
            rules_files: flag("rules_files", d.rules_files, false),
            keep_tied: flag("keep_tied", d.keep_tied, true),
            health_profession: t.get("health_profession").and_then(toml::Value::as_str).unwrap_or("").to_string(),
            archive_by_itself: flag("archive_by_itself", d.archive_by_itself, false),
            active_years: years("active_years"),
            kept_years: years("kept_years"),
        }
    }
}

use super::record::Client;
use super::ties::{Index, TieHow};
use std::collections::BTreeSet;

/// What agents must not get for clients' sake, by address (given to
/// `Consent::keep_persons`, counted and never named): for every client when
/// "Keep from AI agents what is tied to a client" is on, else for each client
/// marked "Kept from AI agents", its address, its cards, the messages from or
/// to their addresses (`mail`: each message's address and its sender's), and
/// what is tied to it (appointments, tasks, time, invoices); and, while
/// agents may not read clients' files, every invoice marked health
/// (`health_invoices`). **Nothing else closes because someone is a client.**
pub fn kept_from_agents<'a>(protect: &Protect, clients: &[Client], index: &Index, contacts: &[crate::contacts::Contact], mail: impl Iterator<Item = (String, &'a str)>, health_invoices: &[String]) -> BTreeSet<String> {
    let mut kept = BTreeSet::new();
    if !protect.agents_files {
        kept.extend(health_invoices.iter().cloned());
    }
    let closed: Vec<&Client> = clients.iter().filter(|c| c.listed() && (protect.keep_tied || c.kept_from_agents)).collect();
    if closed.is_empty() {
        return kept;
    }
    let mut addresses: BTreeSet<String> = BTreeSet::new();
    for c in &closed {
        kept.insert(c.uri());
        for uid in &c.cards {
            kept.insert(crate::links::contact_uri(uid));
            for card in contacts.iter().filter(|card| card.uid == *uid) {
                addresses.extend(card.emails.iter().map(|e| e.value.trim().to_ascii_lowercase()).filter(|e| !e.is_empty()));
            }
        }
        for tie in index.of_client(&c.id) {
            if !matches!(tie.how, TieHow::Project | TieHow::Supply | TieHow::Note) {
                kept.insert(tie.other.clone());
            }
        }
    }
    for (uri, sender) in mail {
        if addresses.contains(&sender.trim().to_ascii_lowercase()) {
            kept.insert(uri);
        }
    }
    kept
}

/// Whether agents may read and write a client's file (its description,
/// notes, rows, attachments): only while the person let them in, and never
/// for a client marked "Kept from AI agents". A program running as the
/// person gets the files through Sioul's own command line and agents' server
/// alone by this answer.
pub fn files_open_to_agents(protect: &Protect, client: &Client) -> bool {
    protect.agents_files && !client.kept_from_agents
}

/// Whether an invoice for this client is marked health: its file holds a
/// health mark (naming a patient and their care reveals care).
pub fn health_invoice(door: &dyn super::door::Door, client: &str, settings: &super::template::Settings) -> Result<bool, super::Error> {
    super::file::holds_health(door, client, settings)
}

/// Whether an invoice billing `reference` (its `client_ref`, `contact`
/// beside it) is for a client whose file holds a health mark, read now
/// through the store as it stands: what an invoice carries as `health` once
/// issued. The client is found as `resolve` finds a profile: by its address;
/// by a profile's id or name (`profiles`: what 0.1.0 writes, and every draft
/// until the drafts' gate), its client the one that adopted it or holds its
/// card; by the card a bare UID in `contact` names, or the reference's card.
/// **Cannot know is health**: the store waiting for its keyring, a record,
/// part of a file or the configuration that does not read all answer true,
/// so that the invoice is named by its number alone and kept from agents;
/// false only where no client is kept (no store) or none is named.
pub fn health_at_issue(opening: &super::door::Opening, reference: &str, contact: &str, profiles: Option<&super::Clients>) -> bool {
    let door = match opening {
        super::door::Opening::Open(door) => door.clone(),
        super::door::Opening::Wait => return true,
        super::door::Opening::NoStore => return false,
    };
    let reference = reference.trim();
    let Ok(read) = super::record::every(door.as_ref()) else { return true };
    if read.damaged > 0 && !reference.is_empty() {
        return true;
    }
    let clients = read.items;
    let by_card = |address: &str| -> Option<String> {
        let key = super::ties::card_key(address)?;
        clients.iter().filter(|c| c.listed() && c.cards.iter().any(|u| super::ties::card_key(u).as_deref() == Some(key.as_str()))).map(|c| c.id.clone()).min()
    };
    let named_profile = profiles.and_then(|p| p.list.iter().find(|p| !reference.is_empty() && !p.id.starts_with(super::address::PREFIX) && (p.id == reference || p.name.eq_ignore_ascii_case(reference))));
    let id = super::address::Address::parse(reference)
        .map(|a| a.client().to_string())
        .or_else(|| named_profile.and_then(|p| clients.iter().filter(|c| c.profile == p.id).map(|c| c.id.clone()).min().or_else(|| super::ties::card_of_address(&p.contact).and_then(|_| by_card(&p.contact)))))
        .or_else(|| (!contact.trim().is_empty()).then(|| by_card(contact.trim())).flatten())
        .or_else(|| super::ties::card_of_address(reference).and_then(|_| by_card(reference)));
    let Some(id) = id else { return false };
    let client = match super::record::find(door.as_ref(), &id) {
        Ok(Some(c)) => c,
        Ok(None) => return false,
        Err(_) => return true,
    };
    // The templates' marks: none without a configuration; one that does not read cannot be told.
    let path = crate::config::default_path();
    let settings = match crate::config::Config::load(&path) {
        Ok(config) => config.clients,
        Err(_) if !crate::files::exists(&path) => super::template::Settings::default(),
        Err(_) => return true,
    };
    health_invoice(door.as_ref(), &client.id, &settings).unwrap_or(true)
}

/// The addresses of a client agents must not get because of what its cards
/// and projects are kept from: a client follows its cards (one kept, the
/// client kept) and the projects its cards are in or that name it (one
/// closed, the client kept; in none, as things outside projects are); and,
/// unless the person opened the files to agents, the parts of its file the
/// index names (rows, notes).
pub fn kept_by_consent(protect: &Protect, clients: &[Client], index: &Index, projects: &[crate::projects::Project], consent: &crate::consent::Consent) -> BTreeSet<String> {
    let mut kept = BTreeSet::new();
    for c in clients.iter().filter(|c| c.listed()) {
        let cards: Vec<String> = c.cards.iter().map(|u| crate::links::contact_uri(u)).collect();
        let mut in_projects: BTreeSet<String> = cards.iter().flat_map(|card| consent.projects_of(card)).collect();
        let me = c.uri();
        in_projects.extend(projects.iter().filter(|p| p.client.as_deref().map(str::trim).is_some_and(|named| named == me || cards.iter().any(|card| card == named))).map(|p| p.id.clone()));
        if cards.iter().any(|card| !consent.allows(card)) || !consent.allows_projects(in_projects.iter().map(String::as_str)) {
            kept.insert(me.clone());
        }
        if !files_open_to_agents(protect, c) {
            for tie in index.of_client(&c.id) {
                for end in [&tie.at, &tie.other] {
                    if super::address::Address::parse(end).is_some_and(|a| !matches!(a, super::address::Address::Client { .. })) {
                        kept.insert(end.clone());
                    }
                }
            }
        }
    }
    kept
}

/// What agents must not get for clients' sake, read as this program reads
/// everything: `kept_by_consent`, `kept_from_agents`, invoices marked health.
/// **Fails closed**: a clients' list that does not open, or a store sealed
/// here that cannot be opened yet (the keyring locked or late), keeps every card.
pub fn kept_for_agents(config: &crate::config::Config, loaded: &crate::links::Loaded, consent: &crate::consent::Consent) -> BTreeSet<String> {
    kept_for_agents_in(&super::door::opening(), config, loaded, consent)
}

/// `kept_for_agents` with the store as it stands (`door::opening`).
pub fn kept_for_agents_in(opening: &super::door::Opening, config: &crate::config::Config, loaded: &crate::links::Loaded, consent: &crate::consent::Consent) -> BTreeSet<String> {
    let protect = &config.clients.protect;
    let invoices = crate::invoice::all_in(&crate::invoice::folder());
    let health: Vec<String> = invoices.iter().filter(|i| i.health).map(|i| if i.number.is_empty() { crate::links::draft_uri(&i.draft) } else { crate::links::invoice_uri(&i.number) }).collect();
    let every_card = |health: Vec<String>| -> BTreeSet<String> { loaded.contacts.iter().map(|c| crate::links::contact_uri(&c.uid)).chain(health).collect() };
    let profiles = config.records_root().and_then(|r| super::Clients::read_file(&r).ok());
    let door = match opening {
        super::door::Opening::Open(door) => door.clone(),
        // Sealed here, not open yet: who is a client cannot be told; wait, every card kept.
        super::door::Opening::Wait => return every_card(health),
        // No store on this device: no client record here. Under "Keep from AI agents what
        // is tied to a client", the cards of the client profiles are kept; a profiles' file
        // that does not read keeps every card.
        super::door::Opening::NoStore => {
            let mut kept: BTreeSet<String> = health.into_iter().filter(|_| !protect.agents_files).collect();
            if protect.keep_tied {
                match &profiles {
                    Some(p) => kept.extend(p.list.iter().filter_map(|p| super::ties::card_of_address(&p.contact)).map(|uid| crate::links::contact_uri(&uid))),
                    None if config.records_root().is_some() => return every_card(Vec::new()).into_iter().chain(kept).collect(),
                    None => {}
                }
            }
            return kept;
        }
    };
    let clients = match super::record::list(door.as_ref()) {
        Ok(read) => read.items,
        Err(_) => return every_card(health),
    };
    let profiles = profiles.unwrap_or_default();
    let sources = super::ties::Sources { contacts: &loaded.contacts, events: &loaded.events, invoices: &invoices, projects: &loaded.projects, profiles: Some(&profiles), ..Default::default() };
    let index = Index::build(door.as_ref(), &clients, &sources);
    let mail = loaded.mail.iter().map(|(id, m)| (crate::links::mail_uri(id), m.address.as_str()));
    let mut kept = kept_from_agents(protect, &clients, &index, &loaded.contacts, mail, &health);
    kept.extend(kept_by_consent(protect, &clients, &index, &loaded.projects, consent));
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::address::Address;
    use crate::clients::door::Memory;
    use crate::clients::record::{self, Billing};
    use crate::clients::ties::{self, Said, Sources};
    use crate::contacts::{Contact, Labeled};

    /// Lean first: nothing is kept because someone is a client; the global
    /// option keeps every client's ties, the mark one client's; health
    /// invoices stay closed while agents may not read the files.
    #[test]
    fn nothing_closes_because_someone_is_a_client() {
        let door = Memory::new();
        let marie = Contact { uid: "c-1".into(), name: "Marie Exemple".into(), emails: vec![Labeled { label: String::new(), value: "marie@example.org".into() }], ..Contact::default() };
        let a = record::make(&door, "", &["c-1".into()], Billing::default()).unwrap();
        let b = record::make(&door, "Robin", &["c-2".into()], Billing::default()).unwrap();
        ties::tie(&door, &Address::Client { id: a.id.clone() }, "sioul:task/t-1", &Said::default()).unwrap();
        let clients = record::list(&door).unwrap().items;
        let contacts = vec![marie];
        let index = ties::Index::build(&door, &clients, &Sources { contacts: &contacts, ..Sources::default() });
        let mail = || vec![("mid:1".to_string(), "MARIE@example.org"), ("mid:2".to_string(), "someone@example.org")].into_iter();
        let lean = Protect::default();
        assert!(kept_from_agents(&lean, &clients, &index, &contacts, mail(), &[]).is_empty(), "lean: nothing closed");
        let all = Protect { keep_tied: true, ..Protect::default() };
        let kept = kept_from_agents(&all, &clients, &index, &contacts, mail(), &[]);
        for k in ["sioul:contact/c-1", "sioul:contact/c-2", "sioul:task/t-1", "mid:1"] {
            assert!(kept.contains(k), "{k}: {kept:?}");
        }
        assert!(!kept.contains("mid:2"));
        record::change(&door, &b.id, &record::Change { kept_from_agents: Some(true), ..Default::default() }).unwrap();
        let clients = record::list(&door).unwrap().items;
        let kept = kept_from_agents(&lean, &clients, &index, &contacts, mail(), &["sioul:invoice/2026-014".into()]);
        assert!(kept.contains("sioul:contact/c-2") && !kept.contains("sioul:contact/c-1"), "the mark keeps its client alone");
        assert!(kept.contains("sioul:invoice/2026-014"), "a health invoice, files closed");
        let open = Protect { agents_files: true, ..Protect::default() };
        assert!(!kept_from_agents(&open, &clients, &index, &contacts, std::iter::empty(), &["sioul:invoice/2026-014".into()]).contains("sioul:invoice/2026-014"));
        assert!(files_open_to_agents(&open, &clients.iter().find(|c| c.id == a.id).unwrap().clone()));
        assert!(!files_open_to_agents(&open, &clients.iter().find(|c| c.id == b.id).unwrap().clone()), "kept from agents");
        assert!(!files_open_to_agents(&lean, &clients[0]), "closed until opened");
        // A key that does not read fails closed.
        let read: toml::Value = toml::from_str("keep_tied = \"yes\"\nagents_files = 1\n").unwrap();
        let p = Protect::of(&read);
        assert!(p.keep_tied && !p.agents_files);
        assert_eq!(Protect::of(&toml::Value::Table(Default::default())), Protect::default(), "lean when unsaid");
    }

    /// The keyring locked or late (the store sealed here and not open yet):
    /// every hook waits. Agents get no card, nothing is made of an
    /// extension's set, and the profiles' reading says so rather than bill a
    /// client by its address as if none existed. With no store on the device
    /// (a phone), nothing of a client is kept and the lean answer stands.
    #[test]
    fn a_store_that_waits_for_its_keyring_keeps_everything_and_makes_nothing() {
        use crate::clients::door::Opening;
        let config = crate::config::Config::default();
        let loaded = crate::links::Loaded { contacts: vec![Contact { uid: "c-1".into(), ..Contact::default() }, Contact { uid: "c-2".into(), ..Contact::default() }], ..Default::default() };
        let consent = crate::consent::Consent::new(&[], true);
        let kept = kept_for_agents_in(&Opening::Wait, &config, &loaded, &consent);
        assert!(kept.contains("sioul:contact/c-1") && kept.contains("sioul:contact/c-2"), "every card kept while it waits: {kept:?}");
        assert!(kept_for_agents_in(&Opening::NoStore, &config, &loaded, &consent).iter().all(|k| !k.starts_with("sioul:contact/")), "no store, no client: lean");
        let set = crate::extensions::reply::ContactSet { id: "clients".into(), clients: true, ..Default::default() };
        let card = std::env::temp_dir().join(format!("sioul-clients-wait-{}.vcf", std::process::id()));
        assert_eq!(crate::clients::extension::after_set_in(&Opening::Wait, &config, "booking", &set, &[("m".into(), card)]), 0, "nothing made while it waits");
        let root = std::env::temp_dir().join(format!("sioul-clients-wait-{}", std::process::id()));
        assert!(crate::clients::Clients::load_in(&root, &Opening::Wait).is_err(), "wait, never no client");
        assert!(crate::clients::Clients::load_in(&root, &Opening::NoStore).is_ok_and(|c| c.list.is_empty()));
    }

    /// At issue, an invoice for a client whose file holds a health mark is
    /// marked health however its draft names the client: its address, a
    /// profile's id or name (what 0.1.0 writes), a bare card UID beside it.
    /// What cannot be read counts as health; no store, no client.
    #[test]
    fn an_invoice_is_marked_health_at_issue_however_its_draft_names_the_client() {
        use crate::clients::door::Opening;
        let door = std::sync::Arc::new(Memory::new());
        let settings = crate::clients::template::Settings { folders_version: 1, kinds_version: 1, ..Default::default() };
        let care = record::make(door.as_ref(), "", &["c-care".into()], Billing::default()).unwrap();
        record::change(door.as_ref(), &care.id, &record::Change { profile: Some("p-care".into()), ..Default::default() }).unwrap();
        crate::clients::file::write_note(door.as_ref(), &care.id, &crate::clients::file::Note { text: "allergy".into(), health: true, ..Default::default() }, &settings).unwrap();
        let plain = record::make(door.as_ref(), "", &["c-plain".into()], Billing::default()).unwrap();
        let open = Opening::Open(door.clone());
        let profiles = crate::clients::Clients { list: vec![crate::clients::Profile { id: "p-care".into(), name: "Care Exemple".into(), contact: "sioul:contact/c-care".into(), ..Default::default() }] };
        assert!(health_at_issue(&open, &crate::clients::client_uri(&care.id), "", None), "by its address");
        assert!(health_at_issue(&open, "p-care", "", Some(&profiles)), "by a profile's id");
        assert!(health_at_issue(&open, "Care Exemple", "", Some(&profiles)), "by a profile's name");
        assert!(health_at_issue(&open, "Someone", "c-care", None), "by the bare card UID beside it");
        assert!(health_at_issue(&open, "sioul:contact/c-care", "", None), "by its card's address");
        assert!(!health_at_issue(&open, "sioul:contact/c-plain", "", None), "no mark, no health");
        assert!(!health_at_issue(&open, "Nobody", "", Some(&profiles)), "no client named");
        assert!(!health_at_issue(&Opening::NoStore, "p-care", "", Some(&profiles)), "no store, no client");
        assert!(health_at_issue(&Opening::Wait, "p-care", "", Some(&profiles)), "cannot know: health");
        // A note of the plain client that does not open: cannot be told free of a mark.
        let note = crate::clients::file::write_note(door.as_ref(), &plain.id, &crate::clients::file::Note { text: "x".into(), ..Default::default() }, &settings).unwrap();
        door.damage(&crate::clients::file::note_path(&plain.id, &note.id));
        assert!(health_at_issue(&open, "sioul:contact/c-plain", "", None), "a part that does not read counts");
    }

    /// A client follows its cards and projects: kept when a card is, or when a
    /// project naming it is closed; open otherwise, as things outside projects.
    #[test]
    fn a_client_follows_what_its_cards_and_projects_are_kept_from() {
        let door = Memory::new();
        let a = record::make(&door, "", &["c-1".into()], Billing::default()).unwrap();
        let b = record::make(&door, "", &["c-2".into()], Billing::default()).unwrap();
        let clients = record::list(&door).unwrap().items;
        let projects = vec![crate::projects::Project { id: "site".into(), title: "Site".into(), client: Some(a.uri()), ..Default::default() }];
        let index = ties::Index::build(&door, &clients, &Sources::default());
        let consent = crate::consent::Consent::new(&projects, true);
        let kept = kept_by_consent(&Protect::default(), &clients, &index, &projects, &consent);
        assert!(kept.contains(&a.uri()), "its project is closed (the default)");
        assert!(!kept.contains(&b.uri()), "in no project, open as things outside projects are");
        let mut closing = crate::consent::Consent::new(&[], true);
        closing.keep_persons(["sioul:contact/c-2".to_string()]);
        assert!(kept_by_consent(&Protect::default(), &clients, &index, &[], &closing).contains(&b.uri()), "its card kept, the client kept");
    }
}
