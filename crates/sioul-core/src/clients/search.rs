// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! One search over every client's file (docs/clients.md, "Search"): names,
//! cards' numbers and addresses, descriptions, notes, rows' fields,
//! attachments' names and a PDF's text once read. Every word, accents and
//! case aside (`text::fold`), each place holding them all a result, with
//! the line holding them (`notes::line_holding`). Read in memory at each
//! search, never an index on the disk. Speed: not measured here.

use super::door::Door;
use super::file;
use super::record::Client;
use super::{Error, Read};
use crate::contacts::Contact;

/// Where in a client the words were found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Name,
    /// A card's name, number or address.
    Card(String),
    Description,
    Note(String),
    /// A row's field: its kind, its row, its field.
    Row(String, String, String),
    /// An attachment's name.
    Attachment(String),
    /// A PDF's text.
    PdfText(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub client: String,
    pub place: Place,
    /// The line holding the words, cut around them.
    pub line: String,
}

fn folded(text: &str) -> String {
    crate::text::fold(text).into_iter().collect()
}

/// Every place of every listed client holding all of `words`; files that do not open counted.
pub fn search(door: &dyn Door, clients: &[Client], contacts: &[Contact], words: &str) -> Result<Read<Vec<Hit>>, Error> {
    let words: Vec<String> = folded(words).split_whitespace().map(str::to_string).collect();
    let mut out: Read<Vec<Hit>> = Read::default();
    if words.is_empty() {
        return Ok(out);
    }
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let found = |text: &str| crate::notes::line_holding(text, &refs);
    for client in clients.iter().filter(|c| c.listed()) {
        let mut hit = |place: Place, text: &str| {
            if let Some(line) = found(text) {
                out.items.push(Hit { client: client.id.clone(), place, line });
            }
        };
        hit(Place::Name, &format!("{}\n{}", client.shown_name(contacts), client.billing.name));
        for card in client.cards.iter().filter_map(|uid| contacts.iter().find(|c| c.uid == *uid)) {
            let text: Vec<String> = [card.name.clone(), card.org.clone()].into_iter().chain(card.phones.iter().map(|p| p.value.clone())).chain(card.emails.iter().map(|e| e.value.clone())).collect();
            hit(Place::Card(card.uid.clone()), &text.join("\n"));
        }
        // A damaged part costs itself, never the whole search.
        let mut damaged = 0;
        match file::description(door, &client.id) {
            Ok(Some(d)) => hit(Place::Description, &d.text),
            Ok(None) => {}
            Err(Error::Damaged(_)) => damaged += 1,
            Err(e) => return Err(e),
        }
        let notes = file::notes(door, &client.id)?;
        damaged += notes.damaged;
        for head in notes.items {
            match file::note(door, &client.id, &head.id) {
                Ok(Some(note)) => hit(Place::Note(note.id.clone()), &format!("{}\n{}", note.title, note.text)),
                Ok(None) => {}
                Err(Error::Damaged(_)) => damaged += 1,
                Err(e) => return Err(e),
            }
        }
        let rows = file::rows(door, &client.id)?;
        damaged += rows.damaged;
        for head in rows.items {
            match file::row(door, &client.id, &head.kind, &head.id) {
                Ok(Some(row)) => {
                    for (field, value) in &row.fields {
                        let text = match value {
                            toml::Value::String(s) => s.clone(),
                            toml::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", "),
                            other => other.to_string(),
                        };
                        hit(Place::Row(row.kind.clone(), row.id.clone(), field.clone()), &text);
                    }
                }
                Ok(None) => {}
                Err(Error::Damaged(_)) => damaged += 1,
                Err(e) => return Err(e),
            }
        }
        let attachments = file::attachments(door, &client.id)?;
        damaged += attachments.damaged;
        for a in attachments.items {
            hit(Place::Attachment(a.id.clone()), &a.name);
            if a.has_text {
                match file::pdf_text(door, &client.id, &a.id) {
                    Ok(Some(text)) => hit(Place::PdfText(a.id.clone()), &text),
                    Ok(None) => {}
                    Err(Error::Damaged(_)) => damaged += 1,
                    Err(e) => return Err(e),
                }
            }
        }
        out.damaged += damaged;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::door::Memory;
    use crate::clients::record::{self, Billing};
    use crate::clients::template::{FieldDef, FieldType, KindDef, Settings};

    #[test]
    fn one_search_over_every_file() {
        let door = Memory::new();
        let mut s = Settings { kinds_version: 1, ..Settings::default() };
        let mut kind = KindDef { name: "Treatment".into(), ..KindDef::default() };
        kind.field.insert("skin".into(), FieldDef { name: "Skin".into(), kind: FieldType::Text, ..FieldDef::default() });
        s.kind.insert("treatment".into(), kind);
        let a = record::make(&door, "Marie Exemple", &[], Billing::default()).unwrap();
        let b = record::make(&door, "Robin Exemple", &[], Billing::default()).unwrap();
        file::write_description(&door, &a.id, "Allergique au latex.\nAime le thé.", None, &s).unwrap();
        file::add_row(&door, &b.id, "treatment", "2026-11-03", [("skin".to_string(), toml::Value::from("Rougeurs, peau sèche"))].into(), &s).unwrap();
        let clients = record::list(&door).unwrap().items;
        let hits = search(&door, &clients, &[], "LATEX allergique").unwrap().items;
        assert_eq!(hits, vec![Hit { client: a.id.clone(), place: Place::Description, line: "Allergique au latex.".into() }]);
        let hits = search(&door, &clients, &[], "seche").unwrap().items;
        assert_eq!((hits.len(), &hits[0].place), (1, &Place::Row("treatment".into(), hits[0].place.clone().row_id(), "skin".into())));
        assert_eq!(search(&door, &clients, &[], "exemple").unwrap().items.len(), 2, "both names");
        assert!(search(&door, &clients, &[], "  ").unwrap().items.is_empty());
        door.damage(&file::description_path(&a.id));
        let read = search(&door, &clients, &[], "exemple").unwrap();
        assert_eq!((read.items.len(), read.damaged), (2, 1), "a damaged description costs itself");
    }

    impl Place {
        fn row_id(self) -> String {
            match self {
                Place::Row(_, row, _) => row,
                _ => String::new(),
            }
        }
    }
}
