// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A message as a rule's conditions read it (docs/rules.md, "A message's
//! fields"): the union of the four ways Sioul said "which mail" (a
//! project's route, a budget's mail rule, a mail filter, the senders'
//! lists), each field read as the place it comes from reads it:
//!
//! - the sender's address and domain (a route's: the domain covers its
//!   subdomains), its card, its circle, From and Reply-To: only for a
//!   sender something proves (`rules::facts_of`, the Porch's `Unproven`):
//!   one nothing proves is nobody, a stranger;
//! - whole words of the subject and the text (a route's: case and accents
//!   aside), or a part of them (a filter's "contains"), To, Cc, the
//!   account, attachments and their kinds, list headers, size, day and
//!   time: as mail filters read them (`rules::Condition`);
//! - its topic, its projects (routes, ties, its conversation), and what
//!   Sioul recognises in it (`kinds_of`: a code, a sign-in link, an
//!   invitation, a newsletter, a payment, an order…).

use crate::card::Card;
use crate::porch::{Lane, Senders, Triaged};
use crate::projects::ProjectStore;
use crate::rules::{self as filters, Facts, Message};
use crate::text::contains_word;
use crate::wiring::store::{Condition, Join, When};
use crate::words::Words;

/// A message as conditions read it, judged once.
pub struct Seen<'a> {
    pub judged: &'a Triaged,
    /// Who its sender is to you, whether anything proves it, its size, when it came.
    pub facts: Facts,
    /// Its projects, its home first: as the Porch's lane, its routes, its ties
    /// and its conversation say (a route on its sender only when proven).
    pub projects: Vec<String>,
    /// Its topic's id, sure; none for none.
    pub topic: Option<String>,
    /// What Sioul recognises in it (`kinds_of`).
    pub kinds: Vec<&'static str>,
    /// Its sender's contact cards, by UID, when something proves the sender.
    pub cards: Vec<String>,
}

impl Seen<'_> {
    pub fn card(&self) -> &Card {
        &self.judged.card
    }

    /// Something proves who sent it.
    pub fn proven(&self) -> bool {
        !self.facts.unproven
    }

    /// Its sender's address, lower case, when something proves it.
    pub fn sender(&self) -> Option<String> {
        self.proven().then(|| self.card().from_address.as_deref().map(|a| a.trim().to_lowercase())).flatten().filter(|a| !a.is_empty())
    }
}

/// A message the Porch judged, read for conditions: `topic` its topic, as
/// the caller reads it (`topics::rules::Lookup`), when a rule asks it.
pub fn seen<'a>(judged: &'a Triaged, senders: &Senders, store: Option<&ProjectStore>, words: &Words, topic: Option<String>) -> Seen<'a> {
    let facts = filters::facts_of(judged, senders);
    let projects = projects_of(judged, facts.unproven, store);
    let kinds = kinds_of(judged, words);
    let cards = if facts.unproven {
        Vec::new()
    } else {
        let address = judged.card.from_address.as_deref().map(|a| a.trim().to_lowercase()).unwrap_or_default();
        senders.contacts.cards_of_address(&address).iter().filter_map(|&i| senders.contacts.card(i)).map(|c| c.uid.clone()).collect()
    };
    Seen { judged, facts, projects, topic, kinds, cards }
}

/// Its projects, its home first: the Porch's lane, then every project a
/// route takes it into (from nobody when nothing proves its sender), or its
/// ties and conversation.
pub fn projects_of(judged: &Triaged, unproven: bool, store: Option<&ProjectStore>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Lane::Project(id) = &judged.lane {
        out.push(id.clone());
    }
    if let Some(store) = store {
        let card = if unproven { Card { from_address: None, ..judged.card.clone() } } else { judged.card.clone() };
        for routing in store.route(&card) {
            if !out.contains(&routing.project.id) {
                out.push(routing.project.id.clone());
            }
        }
    }
    out
}

/// What Sioul recognises in a message, by the recognisers that exist: a
/// code, a temporary password, a reset, a sign-in link, an address to
/// confirm (`codes`); an invitation (a calendar attached); a newsletter or
/// a list; money received, paid, an order, a bill, a refund (`payments`).
pub fn kinds_of(judged: &Triaged, words: &Words) -> Vec<&'static str> {
    use crate::codes::CodeKind;
    use crate::payments::PaymentKind;
    let card = &judged.card;
    let mut out = Vec::new();
    if let Some(code) = judged.code.clone().or_else(|| crate::codes::detect_message(&words.codes, card)) {
        out.push(match code.kind {
            CodeKind::Code => "code",
            CodeKind::Password => "password",
            CodeKind::PasswordReset => "password-reset",
            CodeKind::SignInLink => "sign-in-link",
            CodeKind::Confirmation => "confirmation",
        });
    }
    if card.shape.parts.iter().any(|(name, mime)| filters::attachment_kind(name, mime) == "calendar") {
        out.push("invitation");
    }
    if card.is_list {
        out.push("newsletter");
    }
    if let Some(payment) = crate::payments::detect(words, card) {
        out.push(match payment.kind {
            PaymentKind::Received => "payment-received",
            PaymentKind::Paid => "payment-made",
            PaymentKind::Order => "order",
            PaymentKind::Bill => "bill",
            PaymentKind::Refund => "refund",
        });
    }
    out
}

/// Whether the message shows the conditions: all of them, or one. None never holds.
pub fn holds(when: &When, seen: &Seen) -> bool {
    let message = Message::new(seen.card(), seen.facts.clone());
    let each = |c: &Condition| holds_one(c, seen, &message);
    match when.join {
        Join::All => !when.conditions.is_empty() && when.conditions.iter().all(each),
        Join::Any => when.conditions.iter().any(each),
    }
}

/// "dgfip.finances.example" is "finances.example"'s; "notfinances.example" is not.
pub fn domain_matches(domain: &str, pattern: &str) -> bool {
    let (domain, pattern) = (domain.trim().to_ascii_lowercase(), pattern.trim().trim_start_matches('@').to_ascii_lowercase());
    !pattern.is_empty() && (domain == pattern || domain.ends_with(&format!(".{pattern}")))
}

/// The filter's field a condition of the same meaning reads.
fn filter_field(id: &str) -> Option<filters::Field> {
    use filters::Field;
    Some(match id {
        "from" => Field::From,
        "reply-to" => Field::ReplyTo,
        "to" => Field::To,
        "cc" => Field::Cc,
        "subject" => Field::Subject,
        "text" => Field::Body,
        "anywhere" => Field::Anywhere,
        "attachment" => Field::Attachment,
        "attachment-type" => Field::AttachmentType,
        "circle" => Field::Sender,
        "list" => Field::List,
        "size" => Field::Size,
        "date" => Field::Date,
        "weekday" => Field::Weekday,
        "hour" => Field::Hour,
        _ => return None,
    })
}

/// Whether a test turns its condition around ("does not contain"): then it
/// holds when none of its values does.
fn turned(test: &str) -> bool {
    matches!(test, "not-contains" | "is-not" | "missing")
}

fn holds_one(c: &Condition, seen: &Seen, message: &Message) -> bool {
    let values: Vec<&str> = c.value.iter().map(|v| v.trim()).filter(|v| !v.is_empty()).collect();
    let card = seen.card();
    // Any of the values (none for a test turned around), by `one`.
    let among = |one: &dyn Fn(&str) -> bool| if turned(&c.test) { !values.iter().any(|v| one(v)) } else { values.iter().any(|v| one(v)) };
    match (c.field.as_str(), c.test.as_str()) {
        ("sender", _) => seen.sender().is_some_and(|from| values.iter().any(|v| from.eq_ignore_ascii_case(v))),
        ("sender-domain", _) => seen.proven() && card.sender_domain().is_some_and(|domain| values.iter().any(|v| domain_matches(domain, v))),
        ("card", "exists") => !seen.cards.is_empty(),
        ("card", "missing") => seen.cards.is_empty(),
        ("card", _) => values.iter().any(|v| seen.cards.iter().any(|uid| uid == v)),
        ("account", _) => among(&|v| card.account.as_deref().is_some_and(|a| a == v)),
        ("subject", "has-words") => values.iter().any(|w| contains_word(&card.subject, w)),
        ("text", "has-words") => values.iter().any(|w| contains_word(&card.excerpt, w) || (card.excerpt.chars().count() >= LONG && card.full_text().is_some_and(|t| contains_word(&t, w)))),
        ("topic", _) => among(&|v| seen.topic.as_deref() == Some(v)),
        ("project", _) => among(&|v| seen.projects.iter().any(|p| p == v)),
        ("kind", _) => among(&|v| seen.kinds.contains(&v)),
        (field, test) => {
            let (Some(field), Some(test)) = (filter_field(field), filters::Test::parse(test)) else { return false };
            let condition = |value: &str| filters::Condition { field, test, value: value.to_string(), until: c.until.trim().to_string() };
            match field.value(test) {
                filters::Value::Nothing => condition("").matches(message),
                // Several days in one condition, as a filter says them ("sat,sun").
                filters::Value::Days => condition(&values.join(",")).matches(message),
                // A range: its first value and `until`.
                filters::Value::Dates | filters::Value::Times => values.first().is_some_and(|v| condition(v).matches(message)),
                _ if turned(c.test.as_str()) => !values.is_empty() && values.iter().all(|v| condition(v).matches(message)),
                _ => values.iter().any(|v| condition(v).matches(message)),
            }
        }
    }
}

/// The characters of a card's text kept beside it: one this long may be cut
/// (`card::EXCERPT_LIMIT`), its whole text read then.
const LONG: usize = 6000;
