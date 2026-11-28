// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Moving what exists into clients (docs/clients.md, "Moving what
//! exists"). Each device does the same, with ids made from what moves, so
//! that two devices make one client:
//!
//! - **a client profile** (`sioul-clients.toml`) becomes a client naming it
//!   (`profile`), with its card: its id made from its card's UID when it
//!   names one (so that a project or an invoice naming the same card joins
//!   it), else from the profile's id; a client holding the card already gets
//!   the profile instead. Its billing identity stays the profile's (one
//!   truth, which a 0.1.0 device reads and writes) until the gate, where the
//!   client adopts it and the profile leaves the file (`adopt`);
//! - **a project whose `client` names a card**, **a draft or an issued
//!   invoice billing a card**: that card's human becomes a client (a name
//!   alone makes none);
//! - **drafts' `client_ref`** become the client's address, `contact` beside,
//!   **only once every device knows clients** (`drafts`): a 0.1.0 device
//!   would freeze the address as a name at issue. Until then, a draft's old
//!   reference reads as the client of its card. **Issued invoices are read,
//!   never rewritten**;
//! - **ties Sioul wrote into events** for a client's card (`X-SIOUL-LINK`)
//!   move into the client's tie files; taking them out of the events, where
//!   they may be written, is the caller's (`Plan::events`, through the
//!   agenda's door: `Loaded::untie`).
//!
//! Nothing closes, nothing of the notes, papers and letters moves. The plan
//! is pure (`plan`); `apply` writes it, and a second run writes nothing.

use super::door::Door;
use super::record::{self, Billing, Client};
use super::ties::{self, Said};
use super::{Clients, Error, Profile};
use crate::agenda::EventRef;
use crate::invoice::Invoice;
use crate::projects::Project;
use std::collections::BTreeMap;
use std::path::Path;

/// The id of the client a profile without a card becomes.
pub fn profile_id(profile: &str) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("client\nprofile\n{profile}"))
}

/// A card's UID named by a reference (`sioul:contact/<uid>`).
fn card_of(reference: &str) -> Option<String> {
    ties::card_of_address(reference)
}

/// A draft's new reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftMove {
    /// The draft's own id.
    pub draft: String,
    pub client_ref: String,
    pub contact: String,
}

/// What the move makes and changes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// Every client to make, by id (one per card or profile).
    pub clients: BTreeMap<String, Client>,
    /// A client there already, holding a profile's card: the profile it gets (client id, profile id).
    pub profiles: Vec<(String, String)>,
    /// Drafts whose reference becomes a client's address.
    pub drafts: Vec<DraftMove>,
    /// Events tied by a Sioul link to a client's card: (client id, event UID, its day and title).
    pub event_ties: Vec<(String, String, Said)>,
    /// Those links to take out where they may be written: (event address, card address).
    pub events: Vec<(String, String)>,
}

/// What the move did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Moved {
    pub clients: usize,
    pub drafts: usize,
    pub ties: usize,
    /// Profiles adopted at the gate: their billing copied into their client, the profile out of the file.
    pub adopted: usize,
    /// Clients the plan would make again whose id was erased: left erased, noted as moved.
    pub erased: usize,
}

/// The plan, the same on every device for the same data. `existing`: the
/// clients there already (here, or arrived from another device): a profile
/// is found by the client that names it, then by its card, before one is
/// made; a card a client holds makes none. **One card, one client.**
pub fn plan(profiles: &Clients, drafts: &[Invoice], issued: &[Invoice], projects: &[Project], events: &[EventRef], existing: &[Client]) -> Plan {
    let mut out = Plan::default();
    let listed: Vec<&Client> = existing.iter().filter(|c| c.listed()).collect();
    let holder = |out: &Plan, uid: &str| -> Option<String> {
        record::of_card(existing, uid).map(|c| c.id.clone()).or_else(|| out.clients.values().filter(|c| c.cards.iter().any(|u| u == uid)).map(|c| c.id.clone()).min())
    };
    // Profiles first, in the order of their ids, so that two profiles of one card give one client, the first's.
    let mut sorted: Vec<&Profile> = profiles.list.iter().filter(|p| !p.id.trim().is_empty() && !p.id.starts_with(super::address::PREFIX)).collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut by_profile: BTreeMap<String, String> = BTreeMap::new();
    for profile in sorted {
        let card = card_of(&profile.contact);
        let found = existing.iter().filter(|c| c.profile == profile.id).map(|c| c.id.clone()).min().or_else(|| card.as_deref().and_then(|uid| holder(&out, uid)));
        let id = match found {
            Some(id) => {
                if listed.iter().any(|c| c.id == id && c.profile.is_empty()) && !out.profiles.iter().any(|(c, _)| *c == id) {
                    out.profiles.push((id.clone(), profile.id.clone()));
                }
                id
            }
            None => {
                let id = card.as_deref().map_or_else(|| profile_id(&profile.id), record::card_id);
                // Its billing identity stays the profile's until the gate (`adopt`): one truth.
                let name = if card.is_none() { profile.name.trim().to_string() } else { String::new() };
                out.clients.entry(id.clone()).or_insert_with(|| Client { id: id.clone(), name, cards: card.into_iter().collect(), profile: profile.id.clone(), ..Client::default() });
                id
            }
        };
        by_profile.insert(profile.id.clone(), id);
    }
    let by_card = |out: &mut Plan, uid: &str| {
        if holder(out, uid).is_none() {
            let id = record::card_id(uid);
            out.clients.insert(id.clone(), Client { id, cards: vec![uid.to_string()], ..Client::default() });
        }
    };
    // A project's card, a billed card.
    for project in projects {
        if let Some(uid) = project.client.as_deref().and_then(card_of) {
            by_card(&mut out, &uid);
        }
    }
    for invoice in issued.iter().chain(drafts) {
        if let Some(uid) = card_of(&invoice.client_ref) {
            by_card(&mut out, &uid);
        }
    }
    // Drafts: a card's address, a profile's id, or exactly a profile's name, as `resolve` reads them.
    for draft in drafts.iter().filter(|d| d.number.is_empty() && d.copy_of.is_empty() && !d.draft.is_empty()) {
        let reference = draft.client_ref.trim();
        if reference.starts_with(super::address::PREFIX) {
            continue;
        }
        let found = card_of(reference)
            .and_then(|uid| holder(&out, &uid).map(|id| (id, uid)))
            .or_else(|| {
                let profile = profiles.list.iter().find(|p| !reference.is_empty() && (p.id == reference || p.name.eq_ignore_ascii_case(reference)))?;
                let id = by_profile.get(&profile.id)?.clone();
                Some((id, card_of(&profile.contact).unwrap_or_default()))
            });
        if let Some((id, contact)) = found {
            out.drafts.push(DraftMove { draft: draft.draft.clone(), client_ref: super::address::client_uri(&id), contact });
        }
    }
    // Sioul's links from an event to a client's card.
    let zone = crate::zone::current();
    for event in events {
        for link in &event.links {
            let Some(uid) = card_of(&link.uri) else { continue };
            let Some(client) = holder(&out, &uid) else { continue };
            let day = (event.start > 0).then(|| jiff::Timestamp::from_second(event.start).ok().map(|t| t.to_zoned(zone.clone()).date())).flatten();
            out.event_ties.push((client, event.uid.clone(), Said { day, title: event.summary.clone() }));
            out.events.push((crate::links::event_uri(&event.uid), link.uri.clone()));
        }
    }
    out
}

/// What was moved once, so that nothing erased since comes back: one key
/// each (`"client:<id>"`, `"tie:<client>/<event>"`, `"draft:<id>"`), in a table
/// that travels with the clients' files, each key on its own.
pub const MOVED: &str = "data/clients/moved.toml";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct MovedFile {
    moved: BTreeMap<String, String>,
}

fn moved_keys(door: &dyn Door) -> Result<BTreeMap<String, String>, Error> {
    match door.read_text(MOVED)? {
        None => Ok(BTreeMap::new()),
        Some((text, _)) => toml::from_str::<MovedFile>(&text).map(|f| f.moved).map_err(|_| Error::Damaged(MOVED.to_string())),
    }
}

/// What opens once every device knows clients (`share::client_files_shared`).
#[derive(Debug, Clone, Copy)]
pub struct Gate<'a> {
    /// The invoices' folder: its drafts' references rewritten.
    pub invoices: &'a Path,
    /// `sioul-clients.toml`: its profiles adopted by their clients, then taken out.
    pub profiles: Option<&'a Path>,
}

/// The plan written: the clients made (one there already is kept as it
/// is), the profiles noted on clients holding their cards, the events' ties
/// made; and, through the gate (every device knows clients), each draft's
/// reference rewritten and each profile adopted (`adopt`). Each once: what
/// was moved before, here or on another device, is never made again, even
/// once erased.
pub fn apply(door: &dyn Door, plan: &Plan, gate: Option<Gate>) -> Result<Moved, Error> {
    let mut moved = Moved::default();
    let before = moved_keys(door)?;
    let mut done: Vec<String> = Vec::new();
    for client in plan.clients.values() {
        let key = format!("client:{}", client.id);
        if before.contains_key(&key) {
            continue;
        }
        // An erased client met again (its key lost from `moved.toml` with a
        // file another device wrote whole): it stays erased, noted as done,
        // never an error that would stop the move at every pass.
        if record::was_erased(door, &client.id)? {
            moved.erased += 1;
            done.push(key);
            continue;
        }
        if record::read(door, &client.id)?.is_none() {
            record::make_as(door, client)?;
            moved.clients += 1;
        }
        done.push(key);
    }
    for (client, profile) in &plan.profiles {
        if !record::was_erased(door, client)? && record::read(door, client)?.is_some_and(|c| c.profile.is_empty()) {
            record::change(door, client, &record::Change { profile: Some(profile.clone()), ..record::Change::default() })?;
        }
    }
    for (client, event, said) in &plan.event_ties {
        let key = format!("tie:{client}/{event}");
        if before.contains_key(&key) || record::read(door, client)?.is_none() {
            continue;
        }
        let at = super::address::Address::Client { id: client.clone() };
        let other = crate::links::event_uri(event);
        if !ties::ties_of(door, client)?.items.iter().any(|t| t.other == other) {
            ties::tie(door, &at, &other, said)?;
            moved.ties += 1;
        }
        done.push(key);
    }
    if let Some(gate) = gate {
        let now = crate::invoice::drafts_in(gate.invoices);
        for change in &plan.drafts {
            let key = format!("draft:{}", change.draft);
            if before.contains_key(&key) {
                continue;
            }
            // Only toward a client that is there: a draft billing the card of a
            // client erased (or one not made) keeps billing its card as it did,
            // never an address that bills nothing (the re-review, R3). Its key
            // is not noted, so that it moves once a client of that card exists.
            let target = change.client_ref.strip_prefix(super::address::PREFIX).unwrap_or_default();
            if record::find(door, target)?.is_none() {
                continue;
            }
            let Some(mut draft) = now.iter().find(|d| d.draft == change.draft && d.copy_of.is_empty()).cloned() else { continue };
            if draft.client_ref != change.client_ref {
                draft.client_ref = change.client_ref.clone();
                draft.contact = change.contact.clone();
                crate::invoice::save_draft(gate.invoices, &draft).map_err(Error::draft)?;
                moved.drafts += 1;
            }
            done.push(key);
        }
        if let Some(file) = gate.profiles {
            moved.adopted = adopt(door, file)?;
        }
    }
    note_moved(door, &done)?;
    Ok(moved)
}

/// Keys noted as moved once (`MOVED`), each with today's day; one there is
/// left as it is. A list that does not read is damaged, never written over.
pub(crate) fn note_moved(door: &dyn Door, keys: &[String]) -> Result<(), Error> {
    if keys.is_empty() {
        return Ok(());
    }
    let day = jiff::Zoned::now().date().to_string();
    door.update(MOVED, &mut |now| {
        let text = match now {
            Some(bytes) => std::str::from_utf8(bytes).map_err(|_| Error::Damaged(MOVED.to_string()))?.to_string(),
            None => String::new(),
        };
        let mut doc: toml_edit::DocumentMut = text.parse().map_err(|_| Error::Damaged(MOVED.to_string()))?;
        if !doc.contains_key("moved") {
            doc["moved"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        let table = doc["moved"].as_table_mut().ok_or(Error::Damaged(MOVED.to_string()))?;
        let mut changed = false;
        for key in keys {
            if !table.contains_key(key) {
                table.insert(key, toml_edit::value(day.as_str()));
                changed = true;
            }
        }
        Ok(changed.then(|| doc.to_string().into_bytes()))
    })?;
    Ok(())
}

/// Through the gate: each profile of `file` that a client names is
/// adopted, its billing identity copied into the client's record, then the
/// profile taken out of the file, so that one truth remains. A profile no
/// client names stays. How many.
pub fn adopt(door: &dyn Door, file: &Path) -> Result<usize, Error> {
    let root = file.parent().unwrap_or(Path::new("."));
    let profiles = Clients::read_file(root).map_err(Error::profiles)?;
    let mut n = 0;
    for client in record::every(door)?.items {
        let Some(profile) = client.profile_in(&profiles) else { continue };
        record::change(door, &client.id, &record::Change { billing_whole: Some(Billing::from(profile)), ..record::Change::default() })?;
        super::remove(file, &profile.id).map_err(Error::profiles)?;
        n += 1;
    }
    Ok(n)
}

/// The move as this program reads everything: today's profiles, drafts,
/// issued invoices, projects and events, while Clients is on. A profiles'
/// file that does not read stops it with its error (never a move made of
/// nothing). `gate_open` (every device knows clients) opens the drafts'
/// rewrite, the profiles' adoption, and the events' links to take out
/// (returned for the window's agenda door, `Loaded::untie`): before it, a
/// 0.1.0 device would lose them.
pub fn run(config: &crate::config::Config, loaded: &crate::links::Loaded, door: &dyn Door, gate_open: bool) -> Result<(Moved, Vec<(String, String)>), Error> {
    if !super::on(config) {
        return Ok((Moved::default(), Vec::new()));
    }
    let root = config.records_root();
    let profiles = match &root {
        Some(root) => Clients::read_file(root).map_err(Error::profiles)?,
        None => Clients::default(),
    };
    let folder = crate::invoice::folder();
    let drafts = crate::invoice::drafts_in(&folder);
    let issued = crate::invoice::all_in(&folder);
    // Clients erased here or on another device first: whatever is left of them goes.
    super::keep::apply_erased(door)?;
    let existing = record::every(door)?.items;
    let plan = plan(&profiles, &drafts, &issued, &loaded.projects, &loaded.events, &existing);
    let file = root.as_ref().map(|r| super::file_in(r));
    let gate = gate_open.then(|| Gate { invoices: folder.as_path(), profiles: file.as_deref() });
    let moved = apply(door, &plan, gate)?;
    Ok((moved, if gate_open { plan.events } else { Vec::new() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::door::Memory;
    use crate::contacts::{Contact, Labeled};
    use crate::invoice::Address;

    fn profile(id: &str, contact: &str, name: &str, kind: &str) -> Profile {
        Profile { id: id.into(), contact: contact.into(), name: name.into(), kind: kind.into(), language: "fr".into(), address: Address { lines: vec!["1 rue de l'Exemple".into()], postcode: "75002".into(), city: "Paris".into(), country: "FR".into() }, ..Profile::default() }
    }

    fn invoice(draft: &str, number: &str, client_ref: &str) -> Invoice {
        Invoice { draft: draft.into(), number: number.into(), client_ref: client_ref.into(), currency: "EUR".into(), ..Invoice::default() }
    }

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-clients-move-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// An invented profile with profiles, drafts, issued invoices, projects
    /// and an event's link: two devices make the same clients; a second run
    /// makes nothing; one truth for each billing identity, before and after
    /// the gate; what each invoice bills reads the same before and after;
    /// issued invoices' bytes never change; drafts wait for every device.
    #[test]
    fn the_move_changes_nothing_the_person_sees() {
        let records = folder("records");
        let file = crate::clients::file_in(&records);
        let lumen_p = profile("lumen", "sioul:contact/c-lumen", "Studio Lumen SARL", "business");
        let robin_p = profile("robin", "", "Robin Exemple", "consumer");
        crate::clients::save(&file, &lumen_p).unwrap();
        crate::clients::save(&file, &robin_p).unwrap();
        let profiles = Clients::read_file(&records).unwrap();
        let contacts = vec![
            Contact { uid: "c-lumen".into(), name: "Anna Exemple".into(), org: "Studio Lumen".into(), emails: vec![Labeled { label: String::new(), value: "anna@example.org".into() }], ..Contact::default() },
            Contact { uid: "c-sam".into(), name: "Sam Exemple".into(), ..Contact::default() },
            Contact { uid: "c-proj".into(), name: "Léa Exemple".into(), ..Contact::default() },
        ];
        let drafts = vec![invoice("d-1", "", "sioul:contact/c-lumen"), invoice("d-2", "", "Robin Exemple"), invoice("d-3", "", "sioul:contact/c-sam"), invoice("d-4", "", "Someone by name")];
        let issued = vec![invoice("d-0", "2026-001", "sioul:contact/c-lumen"), invoice("d-9", "2026-002", "robin")];
        let projects = vec![Project { id: "site".into(), title: "Site".into(), client: Some("sioul:contact/c-proj".into()), ..Project::default() }, Project { id: "named".into(), title: "Named".into(), client: Some("Garage Exemple".into()), ..Project::default() }];
        let event = EventRef { uid: "e-1".into(), summary: "Meeting".into(), start: 1_764_756_000, links: vec![crate::tasks::Link { uri: "sioul:contact/c-lumen".into(), ..Default::default() }], ..EventRef::default() };
        let p = plan(&profiles, &drafts, &issued, &projects, std::slice::from_ref(&event), &[]);
        assert_eq!(p, plan(&profiles, &drafts, &issued, &projects, std::slice::from_ref(&event), &[]), "the same on every device");
        // Lumen (by its card), Robin (by its profile), Sam (a draft's card), Léa (a project's card); a name alone makes none.
        assert_eq!(p.clients.len(), 4, "{:?}", p.clients.keys());
        let lumen = record::card_id("c-lumen");
        assert_eq!((p.clients[&lumen].profile.as_str(), p.clients[&lumen].billing.clone()), ("lumen", Billing::default()), "its billing stays the profile's");
        assert_eq!(p.clients[&profile_id("robin")].name, "Robin Exemple");
        assert_eq!(p.drafts.len(), 3, "the draft by a name alone stays as written");
        assert_eq!(p.drafts[0], DraftMove { draft: "d-1".into(), client_ref: format!("sioul:client/{lumen}"), contact: "c-lumen".into() });
        assert_eq!(p.events, vec![("sioul:event/e-1".to_string(), "sioul:contact/c-lumen".to_string())]);

        // Before: what each invoice bills, through today's profiles.
        let billed = |profiles: &Clients, reference: &str| crate::clients::resolve(reference, profiles, &contacts).0;
        let before: Vec<_> = drafts.iter().chain(&issued).map(|i| billed(&profiles, &i.client_ref)).collect();

        let (here, there) = (Memory::new(), Memory::new());
        let moved = apply(&here, &p, None).unwrap();
        assert_eq!((moved.clients, moved.ties, moved.drafts, moved.adopted), (4, 1, 0, 0), "drafts and profiles wait for every device");
        apply(&there, &p, None).unwrap();
        assert_eq!(record::ids(&here).unwrap(), record::ids(&there).unwrap(), "two devices, one client each");
        assert_eq!(apply(&here, &p, None).unwrap(), Moved::default(), "a second run writes nothing");
        // One truth before the gate: the client reads its profile's billing; a change goes to the profile.
        let lumen_c = record::find(&here, &lumen).unwrap().unwrap();
        assert_eq!(lumen_c.billing_of(&profiles).name, "Studio Lumen SARL");
        record::change_billing(&here, &lumen, &crate::clients::ProfileChange { city: Some("Lyon".into()), ..Default::default() }, Some(&file)).unwrap();
        let profiles = Clients::read_file(&records).unwrap();
        assert_eq!((profiles.get("lumen").unwrap().address.city.as_str(), record::find(&here, &lumen).unwrap().unwrap().billing.address.city.as_str()), ("Lyon", ""), "written where it lives");
        assert_eq!(Clients::read_file(&records).unwrap().with_clients(&record::list(&here).unwrap().items).list.iter().filter(|c| c.contact == "sioul:contact/c-lumen").count(), 1, "never listed twice");
        // An issued invoice naming the profile's id (0.1.0, a profile without a card) is tied to its client.
        let index = ties::Index::build(&here, &record::list(&here).unwrap().items, &ties::Sources { invoices: &issued, profiles: Some(&profiles), ..Default::default() });
        assert_eq!(index.clients_of("sioul:invoice/2026-002"), vec![(profile_id("robin"), None)]);
        assert_eq!(index.clients_of("sioul:invoice/2026-001"), vec![(lumen.clone(), None)]);
        let before: Vec<_> = before.into_iter().enumerate().map(|(i, mut b)| {
            if b.name == "Studio Lumen SARL" {
                b.address.city = "Lyon".into();
            }
            let _ = i;
            b
        }).collect();
        // A client erased since is never made again by the move.
        let robin = profile_id("robin");
        there.purge(&record::record_path(&robin)).unwrap();
        assert_eq!(apply(&there, &p, None).unwrap(), Moved::default());
        assert!(record::read(&there, &robin).unwrap().is_none(), "erased stays erased");

        // Through the gate: drafts name their client; profiles adopted and out of the file.
        let invoices = folder("invoices");
        for d in &drafts {
            crate::invoice::save_draft(&invoices, d).unwrap();
        }
        for i in &issued {
            std::fs::write(invoices.join(crate::invoice::file_name(&i.number)), toml::to_string(i).unwrap()).unwrap();
        }
        let issued_bytes = |dir: &std::path::Path| std::fs::read_dir(dir).unwrap().flatten().filter(|e| e.path().is_file()).map(|e| std::fs::read(e.path()).unwrap()).collect::<Vec<_>>();
        let issued_before = issued_bytes(&invoices);
        let gate = Gate { invoices: &invoices, profiles: Some(&file) };
        let moved = apply(&here, &p, Some(gate)).unwrap();
        assert_eq!((moved.drafts, moved.adopted), (3, 2));
        assert_eq!(apply(&here, &p, Some(gate)).unwrap().drafts, 0, "once");
        assert_eq!(issued_bytes(&invoices), issued_before, "the records beside the drafts never rewritten");
        assert!(Clients::read_file(&records).unwrap().list.is_empty(), "the profiles left the file");
        let lumen_c = record::find(&here, &lumen).unwrap().unwrap();
        assert_eq!((lumen_c.billing.name.as_str(), lumen_c.billing.address.city.as_str(), lumen_c.profile.as_str()), ("Studio Lumen SARL", "Lyon", "lumen"), "adopted, the profile's id kept");
        let after_drafts = crate::invoice::drafts_in(&invoices);
        let d1 = after_drafts.iter().find(|d| d.draft == "d-1").unwrap();
        assert_eq!((d1.client_ref.as_str(), d1.contact.as_str()), (format!("sioul:client/{lumen}").as_str(), "c-lumen"));
        assert_eq!(after_drafts.iter().find(|d| d.draft == "d-4").unwrap().client_ref, "Someone by name");
        // What every invoice bills, through the profiles as `Clients::load` gives them once the store is open.
        let with_store = Clients::read_file(&records).unwrap().with_clients(&record::list(&here).unwrap().items);
        let after: Vec<_> = drafts
            .iter()
            .map(|d| after_drafts.iter().find(|a| a.draft == d.draft).unwrap().client_ref.clone())
            .chain(issued.iter().map(|i| i.client_ref.clone()))
            .map(|reference| billed(&with_store, &reference))
            .collect();
        for (b, a) in before.iter().zip(&after) {
            assert_eq!((&b.name, &b.kind, &b.address, &b.email), (&a.name, &a.kind, &a.address, &a.email), "billed the same");
        }
        let _ = std::fs::remove_dir_all(&records);
        let _ = std::fs::remove_dir_all(&invoices);
    }

    /// One card, one client: a client made by hand with a card gives the
    /// card's id; the move finds it by its card and notes the profile on it,
    /// and a project naming the card makes no second client.
    #[test]
    fn one_card_one_client() {
        let door = Memory::new();
        let marie = record::make(&door, "Marie", &["c-marie".into()], Billing::default()).unwrap();
        assert_eq!(marie.id, record::card_id("c-marie"));
        assert_eq!(record::make(&door, "Marie again", &["c-marie".into()], Billing::default()).unwrap().id, marie.id, "the card's client, unchanged");
        let profiles = Clients { list: vec![profile("m", "sioul:contact/c-marie", "Marie Exemple EI", "business")] };
        let projects = vec![Project { id: "p".into(), title: "P".into(), client: Some("sioul:contact/c-marie".into()), ..Project::default() }];
        let existing = record::every(&door).unwrap().items;
        let p = plan(&profiles, &[], &[], &projects, &[], &existing);
        assert!(p.clients.is_empty(), "{:?}", p.clients);
        assert_eq!(p.profiles, vec![(marie.id.clone(), "m".to_string())]);
        apply(&door, &p, None).unwrap();
        assert_eq!(record::find(&door, &marie.id).unwrap().unwrap().profile, "m");
        assert_eq!(record::list(&door).unwrap().items.len(), 1);
    }

    /// A client the move makes from a project's card keeps no name of its
    /// own: listed where profiles are read and found there by its card's name
    /// as the card says it now (a card renamed is followed), billed with its
    /// card's address; its kind and country written from the invoice editor go
    /// to the client, never "changed elsewhere", and the country set there is
    /// never replaced by the card's, nor blanked by a card without one.
    #[test]
    fn a_client_made_from_a_card_bills_and_is_found_as_its_card_says_now() {
        let door = Memory::new();
        let lea = Contact { uid: "c-lea".into(), name: "Léa Exemple".into(), addresses: vec![Labeled { label: String::new(), value: "2 rue de l'Exemple\n1000 Bruxelles\nBelgique".into() }], emails: vec![Labeled { label: String::new(), value: "lea@example.org".into() }], ..Contact::default() };
        let sam = Contact { uid: "c-sam".into(), name: "Sam Exemple".into(), ..Contact::default() };
        let mut contacts = vec![lea, sam];
        let projects = vec![
            Project { id: "p".into(), title: "P".into(), client: Some("sioul:contact/c-lea".into()), ..Project::default() },
            Project { id: "q".into(), title: "Q".into(), client: Some("sioul:contact/c-sam".into()), ..Project::default() },
        ];
        apply(&door, &plan(&Clients::default(), &[], &[], &projects, &[], &[]), None).unwrap();
        let (lea_id, sam_id) = (record::card_id("c-lea"), record::card_id("c-sam"));
        assert!(record::list(&door).unwrap().items.iter().all(|c| c.name.is_empty()), "no name frozen into the record");
        let profiles = Clients::default().with_clients(&record::list(&door).unwrap().items);
        let shown = |profiles: &Clients, contacts: &[Contact], id: &str| profiles.list.iter().find(|p| p.id == crate::clients::client_uri(id)).map(|p| crate::clients::name_shown(p, contacts)).unwrap_or_default();
        assert_eq!(shown(&profiles, &contacts, &lea_id), "Léa Exemple", "listed and found by its card's name");
        let (billed, _) = crate::clients::resolve("sioul:contact/c-lea", &profiles, &contacts);
        assert_eq!((billed.name.as_str(), billed.kind.as_str(), billed.email.as_str(), billed.address.country.as_str()), ("Léa Exemple", "business", "lea@example.org", "BE"), "its card's name, e-mail and postal address");
        // The card renamed: the draft and the picker follow it.
        contacts[0].name = "Léa Martin".into();
        assert_eq!(shown(&profiles, &contacts, &lea_id), "Léa Martin");
        assert_eq!(crate::clients::resolve("sioul:contact/c-lea", &profiles, &contacts).0.name, "Léa Martin");
        // The editor's kind and country, written to the client, and kept over the card's.
        let dir = std::env::temp_dir().join(format!("sioul-clients-traits-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = crate::clients::file_in(&dir);
        let set = |id: &str, kind: &str, country: &str| {
            let change = crate::clients::ProfileChange { kind: Some(kind.into()), country: Some(country.into()), ..Default::default() };
            crate::clients::change_where_it_lives_with(&file, &crate::clients::client_uri(id), &change, Some(&door))
        };
        assert_eq!(set(&lea_id, "consumer", "DE").unwrap(), crate::clients::client_uri(&lea_id));
        assert_eq!(set(&sam_id, "business", "IT").unwrap(), crate::clients::client_uri(&sam_id));
        let profiles = Clients::default().with_clients(&record::list(&door).unwrap().items);
        let (lea_billed, _) = crate::clients::resolve("sioul:contact/c-lea", &profiles, &contacts);
        assert_eq!((lea_billed.kind.as_str(), lea_billed.address.country.as_str(), lea_billed.address.lines.join("\n").as_str(), lea_billed.name.as_str()), ("consumer", "DE", "2 rue de l'Exemple\n1000 Bruxelles", "Léa Martin"), "the country set kept over the card's; the card's postal lines and name filled");
        let (sam_billed, _) = crate::clients::resolve("sioul:contact/c-sam", &profiles, &contacts);
        assert_eq!((sam_billed.address.country.as_str(), sam_billed.name.as_str()), ("IT", "Sam Exemple"), "a card without an address never blanks the country set");
        assert_eq!(set("not-there", "business", "FR"), Err(crate::clients::GONE.to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A client erased whose key went missing from `moved.toml` (a file
    /// another device wrote whole) is met again by the plan: it stays erased,
    /// noted as moved, and the move goes on with the rest.
    #[test]
    fn an_erased_client_met_again_stays_erased_and_never_stops_the_move() {
        let door = Memory::new();
        let projects = vec![
            Project { id: "p".into(), title: "P".into(), client: Some("sioul:contact/c-gone".into()), ..Project::default() },
            Project { id: "q".into(), title: "Q".into(), client: Some("sioul:contact/c-new".into()), ..Project::default() },
        ];
        let first = plan(&Clients::default(), &[], &[], &projects[..1], &[], &[]);
        apply(&door, &first, None).unwrap();
        let gone = record::card_id("c-gone");
        crate::clients::keep::erase(&door, &gone, &crate::clients::template::Settings::default(), &Default::default(), "2026-11-28".parse().unwrap(), None).unwrap();
        // Its key lost: `moved.toml` as another device wrote it, without it.
        door.write(MOVED, b"[moved]\n", crate::clients::door::Expect::Any).unwrap();
        let again = plan(&Clients::default(), &[], &[], &projects, &[], &record::every(&door).unwrap().items);
        let moved = apply(&door, &again, None).unwrap();
        assert_eq!((moved.erased, moved.clients), (1, 1), "left erased, the new one made");
        assert!(record::read(&door, &gone).unwrap().is_none());
        assert!(moved_keys(&door).unwrap().contains_key(&format!("client:{gone}")), "noted again: done");
        assert_eq!(apply(&door, &again, None).unwrap(), Moved::default(), "once");
    }

    /// A draft billing the card of a client erased keeps billing the card
    /// through the gate: never rewritten to the erased client's address,
    /// which would bill nothing and print the address as the name.
    #[test]
    fn a_draft_for_an_erased_client_s_card_keeps_billing_the_card() {
        let door = Memory::new();
        let dir = std::env::temp_dir().join(format!("sioul-clients-erased-draft-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ana = Contact { uid: "c-ana".into(), name: "Ana Exemple".into(), ..Contact::default() };
        let draft = invoice("d-ana", "", "sioul:contact/c-ana");
        crate::invoice::save_draft(&dir, &draft).unwrap();
        let drafts = crate::invoice::drafts_in(&dir);
        apply(&door, &plan(&Clients::default(), &drafts, &[], &[], &[], &[]), None).unwrap();
        let id = record::card_id("c-ana");
        crate::clients::keep::erase(&door, &id, &crate::clients::template::Settings::default(), &Default::default(), "2026-11-28".parse().unwrap(), None).unwrap();
        // Its key lost from `moved.toml`, the plan points the draft at the erased id again.
        door.write(MOVED, b"[moved]\n", crate::clients::door::Expect::Any).unwrap();
        let again = plan(&Clients::default(), &crate::invoice::drafts_in(&dir), &[], &[], &[], &record::every(&door).unwrap().items);
        assert!(again.drafts.iter().any(|d| d.client_ref == crate::clients::client_uri(&id)));
        let moved = apply(&door, &again, Some(Gate { invoices: &dir, profiles: None })).unwrap();
        assert_eq!((moved.drafts, moved.erased), (0, 1), "nothing rewritten, the erased id left erased");
        let after = crate::invoice::drafts_in(&dir);
        assert_eq!(after[0].client_ref, "sioul:contact/c-ana", "it bills the card as it did");
        let (billed, _) = crate::clients::resolve(&after[0].client_ref, &Clients::default().with_clients(&record::list(&door).unwrap().items), &[ana]);
        assert_eq!(billed.name, "Ana Exemple", "never an address as the name");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A 0.1.0-shaped reader (today's `resolve` over profiles alone) given a
    /// draft before and after the gate: before, it bills as always; after,
    /// it would print the address as a name. Why drafts wait for every device.
    #[test]
    fn an_older_device_reads_drafts_only_before_the_gate() {
        let profiles = Clients { list: vec![profile("lumen", "sioul:contact/c-lumen", "Studio Lumen SARL", "business")] };
        let p = plan(&profiles, &[invoice("d-1", "", "sioul:contact/c-lumen")], &[], &[], &[], &[]);
        let older = |reference: &str| crate::clients::resolve(reference, &profiles, &[]).0.name;
        assert_eq!(older("sioul:contact/c-lumen"), "Studio Lumen SARL", "before the gate");
        assert!(older(&p.drafts[0].client_ref).starts_with("sioul:client/"), "after it, an older device would print the address");
    }
}
