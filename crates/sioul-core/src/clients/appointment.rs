// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! How an appointment finds its client (docs/clients.md, "How an
//! appointment finds its client"), in this order: a tie (made by hand, or a
//! row written on it); a card the event names itself (a `CONTACT` line or a
//! guest's address on one of a client's cards; an extension's door writes
//! the `CONTACT` of the person the extension says the event is with). Else, when its
//! title holds exactly one client's name, "With Marie Exemple?" is offered:
//! **a name alone never ties**.

use super::record::Client;
use super::ties::{Index, KeptIn, TieHow};
use crate::agenda::EventRef;
use crate::contacts::Contact;

/// What an appointment's client is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// Tied by hand or by a row written on it: the client, and the row when one.
    Tied { client: String, row: Option<String> },
    /// A card the event names (its `CONTACT`, which an extension's door writes too, or a guest).
    Card(String),
    /// Its title holds exactly this client's name: offered, never tied.
    Offer(String),
    None,
}

/// Words as compared: case and accents aside, each word alone.
fn words(text: &str) -> Vec<String> {
    let folded: String = crate::text::fold(text).into_iter().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    folded.split_whitespace().map(str::to_string).collect()
}

/// Whether `title` holds `name`'s words, together, in order.
fn holds(title: &[String], name: &[String]) -> bool {
    !name.is_empty() && title.windows(name.len()).any(|w| w == name)
}

/// The client of an appointment.
pub fn client_of_event(event: &EventRef, index: &Index, clients: &[Client], contacts: &[Contact]) -> Found {
    let me = crate::links::event_uri(&event.uid);
    let ties = index.ties_to(&me);
    let first = |kept: KeptIn| ties.iter().find(|t| t.kept == kept && t.how == TieHow::Appointment);
    if let Some(t) = first(KeptIn::TieFile) {
        let row = (t.at != t.client_uri()).then(|| t.at.clone());
        return Found::Tied { client: t.client.clone(), row };
    }
    if let Some(t) = first(KeptIn::EventCard) {
        return Found::Card(t.client.clone());
    }
    let title = words(&event.summary);
    let named: Vec<&Client> = clients.iter().filter(|c| c.listed() && c.archived.is_none()).filter(|c| holds(&title, &words(&c.shown_name(contacts)))).collect();
    match named.as_slice() {
        [one] => Found::Offer(one.id.clone()),
        _ => Found::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::address::Address;
    use crate::clients::door::Memory;
    use crate::clients::record::{self, Billing};
    use crate::clients::ties::{self, Said, Sources};
    use crate::contacts::Labeled;

    #[test]
    fn an_appointment_finds_its_client_by_proof_never_by_a_name() {
        let door = Memory::new();
        let marie = Contact { uid: "c-1".into(), name: "Marie Exemple".into(), emails: vec![Labeled { label: String::new(), value: "marie@example.org".into() }], ..Contact::default() };
        let robin = Contact { uid: "c-2".into(), name: "Robin Exemple".into(), ..Contact::default() };
        let a = record::make(&door, "", &["c-1".into()], Billing::default()).unwrap();
        let b = record::make(&door, "", &["c-2".into()], Billing::default()).unwrap();
        let c = record::make(&door, "Sam Exemple", &["c-3".into()], Billing::default()).unwrap();
        let clients = record::list(&door).unwrap().items;
        let contacts = vec![marie, robin];
        let event = |uid: &str, title: &str| EventRef { uid: uid.into(), summary: title.into(), start: 1_764_756_000, ..EventRef::default() };
        let by_hand = event("e-hand", "Facial");
        let by_guest = EventRef { guests: vec!["MARIE@example.org".into()], ..event("e-guest", "Facial") };
        let by_contact = EventRef { contacts: vec!["sioul:contact/c-2".into()], ..event("e-contact", "Massage") };
        // An extension's appointment: its door wrote the card of the person it is with (`extensions::door`).
        let by_extension = EventRef { contacts: vec![format!("urn:uuid:{}", c.cards[0])], ..event("e-ext", "Booking") };
        let by_name = event("e-name", "Facial – robin exemple");
        let by_two_names = event("e-two", "Marie Exemple and Robin Exemple");
        ties::tie(&door, &Address::Client { id: b.id.clone() }, "sioul:event/e-hand", &Said::default()).unwrap();
        let events = vec![by_hand.clone(), by_guest.clone(), by_contact.clone(), by_extension.clone(), by_name.clone(), by_two_names.clone()];
        let index = ties::Index::build(&door, &clients, &Sources { contacts: &contacts, events: &events, ..Sources::default() });
        assert_eq!(client_of_event(&by_hand, &index, &clients, &contacts), Found::Tied { client: b.id.clone(), row: None });
        assert_eq!(client_of_event(&by_guest, &index, &clients, &contacts), Found::Card(a.id.clone()));
        assert_eq!(client_of_event(&by_contact, &index, &clients, &contacts), Found::Card(b.id.clone()));
        assert_eq!(client_of_event(&by_extension, &index, &clients, &contacts), Found::Card(c.id.clone()));
        assert_eq!(client_of_event(&by_name, &index, &clients, &contacts), Found::Offer(b.id.clone()), "offered, never tied");
        assert!(index.clients_of("sioul:event/e-name").is_empty(), "a name alone ties nothing");
        assert_eq!(client_of_event(&by_two_names, &index, &clients, &contacts), Found::None, "two names: nothing offered");
        // From the client's side: its appointments, its facts.
        assert_eq!(index.clients_of("sioul:event/e-guest"), vec![(a.id.clone(), None)]);
        let facts = index.facts(&a.id, "2026-12-31".parse().unwrap());
        assert!(facts.last.is_some() && facts.since == facts.last);
    }

    /// An extension's appointment names the card of the person it is with as
    /// main's door writes it (`contacts::uid_uri`): a card whose UID is a URN
    /// already, or holds a space, or is written in another case, still finds
    /// its client, so that the appointment is tied and kept with the client.
    #[test]
    fn every_form_of_a_card_s_address_finds_its_client() {
        let door = Memory::new();
        let urn = record::make(&door, "", &["urn:uuid:0e2a9c4b-1111-4222-8333-444455556666".into()], Billing::default()).unwrap();
        let spaced = record::make(&door, "", &["Card of Léa 1".into()], Billing::default()).unwrap();
        let plain = record::make(&door, "", &["f81d4fae-7dec-11d0-a765-00a0c91e6bf6".into()], Billing::default()).unwrap();
        let clients = record::list(&door).unwrap().items;
        let event = |uid: &str, card: &str| EventRef { uid: uid.into(), summary: "Booking".into(), contacts: vec![crate::contacts::uid_uri(card)], ..EventRef::default() };
        let events = vec![
            event("e-urn", "urn:uuid:0e2a9c4b-1111-4222-8333-444455556666"),
            event("e-spaced", "Card of Léa 1"),
            EventRef { contacts: vec!["URN:UUID:F81D4FAE-7DEC-11D0-A765-00A0C91E6BF6".into()], ..event("e-case", "") },
        ];
        let index = ties::Index::build(&door, &clients, &Sources { events: &events, ..Sources::default() });
        assert_eq!(client_of_event(&events[0], &index, &clients, &[]), Found::Card(urn.id.clone()), "a UID that is a URN, as the door writes it");
        assert_eq!(client_of_event(&events[1], &index, &clients, &[]), Found::Card(spaced.id.clone()), "a UID percent-encoded");
        assert_eq!(client_of_event(&events[2], &index, &clients, &[]), Found::Card(plain.id.clone()), "another program's case");
        assert_eq!(index.client_of_card("urn:uuid:0e2a9c4b-1111-4222-8333-444455556666"), Some(urn.id.as_str()));
    }
}
