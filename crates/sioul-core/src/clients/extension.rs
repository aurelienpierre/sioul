// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An extension's people as the person's clients (docs/clients.md, "An
//! extension's clients").
//!
//! **The extension says it**: a set of contacts with `"clients": true`
//! (`reply::ContactSet::clients`). The core infers nothing from whose an
//! agenda is and names no extension. While Clients is on and the person has
//! not taken the set back (`[extensions.clients]`, "They are not my
//! clients"), each person of the set becomes a client, its id made from the
//! person's mark (the same on every device), `from` the set. "Not a client"
//! for one keeps a small mark so that no refresh makes them again.
//!
//! **Whom an appointment is with** is the extension door's own: the event
//! names the person (`X-SIOUL-PERSON`) and the door writes the person's card
//! into it (`CONTACT;ALTREP="urn:uuid:<UID>"`, `extensions::door`), which the
//! clients' index reads as any card an event names (`ties::Index::build`).

use super::door::{Door, Expect};
use super::record::{self, Client};
use super::{Error, Read};
use crate::extensions::reply::ContactSet;
use std::path::{Path, PathBuf};

/// The id of the client an extension's person becomes: the same on every device.
pub fn mark_id(mark: &str) -> String {
    crate::holders::uuid_v5(&crate::holders::NAMESPACE, &format!("client\nmark\n{mark}"))
}

/// Where "Not a client" is kept for a person's mark.
fn declined_path(mark: &str) -> String {
    format!("{}/{}/{}.toml", record::ROOT, record::DECLINED, super::ties::tie_name("declined", mark))
}

/// Whether a person's mark was said "Not a client".
pub fn is_declined(door: &dyn Door, mark: &str) -> Result<bool, Error> {
    Ok(door.read(&declined_path(mark))?.is_some())
}

/// The set's key, as `[extensions.clients]` writes it.
pub fn set_key(extension: &str, set: &str) -> String {
    crate::config::extension_calendar_key(extension, set)
}

/// Whether the people of a set are the person's clients now: the extension
/// says so, and the person did not take the set back.
pub fn set_says(config: &crate::config::Config, extension: &str, set: &ContactSet) -> bool {
    set.clients && config.extensions.clients.get(&set_key(extension, &set.id)) != Some(&false)
}

/// What taking a set in did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Taken {
    /// Clients made now (said once by the window: "312 clients from …").
    pub made: usize,
    /// People who were clients already (by their mark or their card).
    pub known: usize,
    /// People said "Not a client" before: left.
    pub declined: usize,
}

/// A card's UID, read from its file.
fn card_uid(path: &Path) -> Option<String> {
    let text = crate::files::read_to_string(path).ok()?;
    crate::lines::unfold(&text).iter().find(|l| crate::lines::name(l) == "UID").map(|l| crate::lines::value(l).trim().to_string()).filter(|u| !u.is_empty())
}

/// A set's people made clients, each with the card that is theirs now
/// (`cards::Done::cards`: mark → card file). Only while `set_says`; a person
/// declined is left; a client already holding the card keeps its record
/// (its mark noted, so that appointments find it).
pub fn take(door: &dyn Door, extension: &str, set: &ContactSet, cards: &[(String, PathBuf)], says: bool) -> Result<Taken, Error> {
    let mut taken = Taken::default();
    if !says {
        return Ok(taken);
    }
    let clients = record::every(door)?.items;
    let from = set_key(extension, &set.id);
    for (mark, path) in cards {
        if is_declined(door, mark)? {
            taken.declined += 1;
            continue;
        }
        if let Some(found) = record::by_mark(&clients, mark) {
            if found.declined.is_some() {
                taken.declined += 1;
            } else {
                taken.known += 1;
            }
            continue;
        }
        let Some(uid) = card_uid(path) else { continue };
        if let Some(found) = record::of_card(&clients, &uid) {
            if found.mark.is_empty() {
                record::change(door, &found.id, &record::Change { mark: Some(mark.clone()), ..record::Change::default() })?;
            }
            taken.known += 1;
            continue;
        }
        // No name of its own: its card names it, as the card says it now (`clients::name_shown`).
        let client = Client { id: mark_id(mark), cards: vec![uid], from: from.clone(), mark: mark.clone(), ..Client::default() };
        // A person whose client was erased (a joined one's mark, met before its
        // "Not a client" travelled): left erased and counted so, never an error
        // that would stop the set's other people being made clients.
        if record::was_erased(door, &client.id)? {
            mark_declined(door, mark, jiff::Zoned::now().date())?;
            taken.declined += 1;
            continue;
        }
        let was = record::read(door, &client.id)?.is_some();
        record::make_as(door, &client)?;
        if was { taken.known += 1 } else { taken.made += 1 }
    }
    Ok(taken)
}

/// A person's mark kept as "Not a client", whatever later refreshes say.
pub fn mark_declined(door: &dyn Door, mark: &str, day: jiff::civil::Date) -> Result<(), Error> {
    door.write(&declined_path(mark), format!("day = \"{day}\"\n").as_bytes(), Expect::Any).map(|_| ())
}

/// "Not a client" for one: out of every list from `day`, and kept so, by
/// its mark, whatever later refreshes say. Its record stays (invoices may name it).
pub fn decline(door: &dyn Door, id: &str, day: jiff::civil::Date) -> Result<Client, Error> {
    let client = record::change(door, id, &record::Change { declined: Some(Some(day)), ..record::Change::default() })?;
    if !client.mark.is_empty() {
        mark_declined(door, &client.mark, day)?;
    }
    Ok(client)
}

/// "Not a client" taken back.
pub fn undecline(door: &dyn Door, id: &str) -> Result<Client, Error> {
    let client = record::change(door, id, &record::Change { declined: Some(None), ..record::Change::default() })?;
    if !client.mark.is_empty() {
        door.remove(&declined_path(&client.mark))?;
    }
    Ok(client)
}

/// "They are not my clients": every client the set made declined (their
/// cards stay); the setting itself is the caller's (`config::set_extension_clients`).
/// How many.
pub fn decline_set(door: &dyn Door, extension: &str, set: &str, day: jiff::civil::Date) -> Result<usize, Error> {
    let from = set_key(extension, set);
    let mut n = 0;
    for c in record::every(door)?.items.into_iter().filter(|c| c.from == from && c.declined.is_none()) {
        decline(door, &c.id, day)?;
        n += 1;
    }
    Ok(n)
}

/// The set taken back again: the clients it made return.
pub fn undecline_set(door: &dyn Door, extension: &str, set: &str) -> Result<usize, Error> {
    let from = set_key(extension, set);
    let mut n = 0;
    for c in record::every(door)?.items.into_iter().filter(|c| c.from == from && c.declined.is_some()) {
        undecline(door, &c.id)?;
        n += 1;
    }
    Ok(n)
}

/// After a set's cards were written: its people made clients, through the
/// door installed in this program, while Clients is on and the set says so;
/// nothing when the store is not open. How many were made.
pub fn after_set(config: &crate::config::Config, extension: &str, set: &ContactSet, cards: &[(String, PathBuf)]) -> usize {
    after_set_in(&super::door::opening(), config, extension, set, cards)
}

/// `after_set` with the store as it stands (`door::opening`): while it is
/// not open (no store here, or one that waits), nothing is made, and the next
/// refresh makes them.
pub fn after_set_in(opening: &super::door::Opening, config: &crate::config::Config, extension: &str, set: &ContactSet, cards: &[(String, PathBuf)]) -> usize {
    if !super::on(config) || !set.clients {
        return 0;
    }
    let Ok(door) = opening.door() else { return 0 };
    match take(door.as_ref(), extension, set, cards, set_says(config, extension, set)) {
        Ok(t) => t.made,
        Err(e) => {
            eprintln!("sioul: extensions: {extension}: clients not made: {e}");
            0
        }
    }
}

/// The clients an extension's set made, in the lists.
pub fn of_set(clients: &Read<Vec<Client>>, extension: &str, set: &str) -> usize {
    let from = set_key(extension, set);
    clients.items.iter().filter(|c| c.from == from && c.listed()).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::door::Memory;
    use crate::extensions::reply::refresh_reply;

    fn card(dir: &Path, name: &str, uid: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\nFN:Someone\r\nEND:VCARD\r\n")).unwrap();
        path
    }

    /// The extension's word makes clients, once, the same on two devices;
    /// a set without it makes none; "Not a client" holds through refreshes;
    /// "They are not my clients" declines the set's, and is taken back.
    #[test]
    fn an_extension_says_whose_clients() {
        let reply = refresh_reply(
            &serde_json::json!({ "contacts": [
                { "id": "clients", "name": "Clients", "clients": true, "people": [{ "id": "p-1", "name": "Marie Exemple" }, { "id": "p-2", "name": "Robin Exemple" }] },
                { "id": "staff", "name": "Staff", "people": [{ "id": "s-1", "name": "Sam Exemple" }] }
            ] })
            .to_string(),
        )
        .unwrap();
        assert!(reply.contacts[0].clients && !reply.contacts[1].clients, "absent reads false");
        let dir = std::env::temp_dir().join(format!("sioul-clients-ext-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mark = |set: &str, person: &str| crate::extensions::person_mark("booking", set, person);
        let made = vec![(mark("clients", "p-1"), card(&dir, "a.vcf", &mark("clients", "p-1"))), (mark("clients", "p-2"), card(&dir, "b.vcf", "own-card"))];
        let (here, there) = (Memory::new(), Memory::new());
        let config = crate::config::Config::default();
        assert!(set_says(&config, "booking", &reply.contacts[0]) && !set_says(&config, "booking", &reply.contacts[1]));
        let t = take(&here, "booking", &reply.contacts[0], &made, true).unwrap();
        assert_eq!((t.made, t.known), (2, 0));
        take(&there, "booking", &reply.contacts[0], &made, true).unwrap();
        assert_eq!(record::ids(&here).unwrap(), record::ids(&there).unwrap(), "one client on every device");
        assert_eq!(take(&here, "booking", &reply.contacts[0], &made, true).unwrap(), Taken { made: 0, known: 2, declined: 0 }, "said once: a refresh makes none again");
        let staff = vec![(mark("staff", "s-1"), card(&dir, "c.vcf", "sam"))];
        assert_eq!(take(&here, "booking", &reply.contacts[1], &staff, set_says(&config, "booking", &reply.contacts[1])).unwrap().made, 0, "no word, no client");
        let clients = record::list(&here).unwrap().items;
        let robin = clients.iter().find(|c| c.cards == vec!["own-card".to_string()]).unwrap();
        assert_eq!(robin.from, "booking/clients");
        // "Not a client" for one: kept through refreshes, even once erased.
        decline(&here, &robin.id, "2026-11-24".parse().unwrap()).unwrap();
        assert_eq!(record::list(&here).unwrap().items.len(), 1);
        here.purge(&record::record_path(&robin.id)).unwrap();
        assert_eq!(take(&here, "booking", &reply.contacts[0], &made, true).unwrap().declined, 1);
        // "They are not my clients", and back.
        let mut taken_back = config.clone();
        taken_back.extensions.clients.insert(set_key("booking", "clients"), false);
        assert!(!set_says(&taken_back, "booking", &reply.contacts[0]));
        assert_eq!(decline_set(&here, "booking", "clients", "2026-11-24".parse().unwrap()).unwrap(), 1);
        assert!(record::list(&here).unwrap().items.is_empty());
        assert_eq!(undecline_set(&here, "booking", "clients").unwrap(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A client joined into another and erased with it: its person's mark is
    /// declined at the erase, and a person whose client's id was erased is
    /// counted declined, never an error that stops the set's other people.
    #[test]
    fn an_erased_person_never_stops_a_set_s_refresh() {
        let dir = std::env::temp_dir().join(format!("sioul-clients-ext-erased-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let reply = refresh_reply(&serde_json::json!({ "contacts": [{ "id": "clients", "name": "Clients", "clients": true, "people": [{ "id": "p-1", "name": "A" }, { "id": "p-2", "name": "B" }, { "id": "p-3", "name": "C" }] }] }).to_string()).unwrap();
        let mark = |person: &str| crate::extensions::person_mark("booking", "clients", person);
        let cards = vec![(mark("p-1"), card(&dir, "a.vcf", "card-a")), (mark("p-2"), card(&dir, "b.vcf", "card-b")), (mark("p-3"), card(&dir, "c.vcf", "card-c"))];
        let door = Memory::new();
        take(&door, "booking", &reply.contacts[0], &cards[..2], true).unwrap();
        // B joined into A, then A erased: both erased, both marks declined.
        let (a, b) = (mark_id(&mark("p-1")), mark_id(&mark("p-2")));
        let (keep, other) = if a < b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) };
        crate::clients::keep::same_as(&door, &keep, &other).unwrap();
        crate::clients::keep::erase(&door, &keep, &crate::clients::template::Settings::default(), &Default::default(), "2026-11-28".parse().unwrap(), None).unwrap();
        assert!(is_declined(&door, &mark("p-1")).unwrap() && is_declined(&door, &mark("p-2")).unwrap(), "the joined one's mark too");
        // A mark lost (as another device's file would lose it): the erased id is met again.
        door.purge(&declined_path(&mark("p-2"))).unwrap();
        let t = take(&door, "booking", &reply.contacts[0], &cards, true).unwrap();
        assert_eq!((t.made, t.declined), (1, 2), "C made, A and B left erased");
        assert!(record::read(&door, &other).unwrap().is_none() && record::read(&door, &keep).unwrap().is_none());
        assert!(is_declined(&door, &mark("p-2")).unwrap(), "declined again");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
