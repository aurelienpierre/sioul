// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Conversations: messages tied by their Message-ID and the ids they answer
//! or cite (In-Reply-To, References; RFC 5322 §3.6.4), never by subject alone,
//! which would join strangers who both wrote "Hello". A message tied to
//! another through a third is in the same conversation.

use crate::card::Card;
use crate::mailindex::bare_id;
use std::collections::HashMap;

/// The ids a message is tied by: its own first, then those it answers or cites.
pub fn ids_of(card: &Card) -> Vec<String> {
    let mut ids: Vec<String> = card.message_id.iter().map(|id| bare_id(id)).collect();
    for name in ["In-Reply-To", "References"] {
        for value in card.headers.all(name) {
            ids.extend(value.split(|c: char| c.is_whitespace() || c == ',').filter(|t| t.contains('@')).map(bare_id));
        }
    }
    ids.retain(|id| !id.is_empty());
    ids
}

/// The conversation of each card, as a number: equal for messages of the same conversation.
pub fn group(cards: &[&Card]) -> Vec<usize> {
    // Union-find over the cards, joined through the ids they share.
    let mut parent: Vec<usize> = (0..cards.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut first_with: HashMap<String, usize> = HashMap::new();
    for (i, card) in cards.iter().enumerate() {
        for id in ids_of(card) {
            match first_with.get(&id) {
                Some(&j) => {
                    let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                    parent[a.max(b)] = a.min(b);
                }
                None => {
                    first_with.insert(id, i);
                }
            }
        }
    }
    (0..cards.len()).map(|i| root(&mut parent, i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(raw: &str) -> Card {
        Card::from_bytes(raw.replace('\n', "\r\n").as_bytes()).unwrap()
    }

    #[test]
    fn answers_join_their_conversation() {
        let first = card("From: jane@example.org\nSubject: Hello\nMessage-ID: <a@example.org>\n\nHi.\n");
        let answer = card("From: me@example.net\nSubject: Re: Hello\nMessage-ID: <b@example.net>\nIn-Reply-To: <a@example.org>\n\nHi Jane.\n");
        // Tied to the first through the answer only.
        let third = card("From: jane@example.org\nSubject: Re: Hello\nMessage-ID: <c@example.org>\nReferences: <b@example.net>\n\nThanks.\n");
        let stranger = card("From: paul@example.com\nSubject: Hello\nMessage-ID: <d@example.com>\n\nHello too.\n");
        let groups = group(&[&first, &answer, &third, &stranger]);
        assert_eq!(groups[0], groups[1]);
        assert_eq!(groups[0], groups[2]);
        assert_ne!(groups[0], groups[3], "the same subject does not make a conversation");
        assert_eq!(ids_of(&third), vec!["c@example.org", "b@example.net"]);
    }
}
