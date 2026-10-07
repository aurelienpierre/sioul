// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The seam to sioul-core's spam module: the tokenizer, the header features
//! and the reduced table, the same code the phone runs.
//!
//! Training reads a corpus record as the Porch reads the message itself, by
//! the same code: `card_of` makes the message again from what the record
//! keeps (its header block; its MIME structure from BODYSTRUCTURE, part by
//! part; the text and the HTML the Porch reads, in their places; every other
//! part empty) and hands it to `Card::from_bytes`. So the card's excerpt
//! (the text part, the HTML read as text after a stand-in) and its shape (its
//! text and HTML versions, its other parts' names and types) are made by
//! sioul-core's own code, mail-parser sorting the parts. `bodies` asks the
//! same parser, before the texts are downloaded, which parts they are.

use crate::corpus::{Leaf, Node, Record, Text};
use mail_parser::{MessageParser, PartType};
pub(crate) use sioul_core::spam::features::{FEATURES, N, NAMES, features};
#[cfg(test)]
pub(crate) use sioul_core::spam::table::ngram_buckets;
pub(crate) use sioul_core::spam::table::{Meta, Table, word_hash};
pub(crate) use sioul_core::spam::tokenize::{TOKENIZER, tokens};
use sioul_core::card::Card;

/// The Porch's card for a corpus record, made by `Card::from_bytes` from the
/// message made again (see the module); its headers are the record's own,
/// MIME fields included.
pub(crate) fn card_of(record: &Record) -> Option<Card> {
    let header = record.header_bytes();
    let raw = rebuild(&header, record.structure.as_ref(), record.plain.as_ref(), record.html.as_ref());
    let mut card = Card::from_bytes(&raw)?;
    card.headers = sioul_core::headers::RawHeaders::parse(&[header.as_slice(), b"\r\n\r\n"].concat());
    Some(card)
}

/// Which parts of a structure are its text and HTML bodies, as mail-parser
/// sorts them for the Porch: its first text body that is text, its first
/// HTML body that is HTML (mail-parser lends one as the other when a message
/// has only one).
pub(crate) fn bodies(structure: &Node) -> (Option<Vec<u32>>, Option<Vec<u32>>) {
    let raw = rebuild(b"", Some(structure), None, None);
    let Some(message) = MessageParser::default().parse(&raw) else { return (None, None) };
    let numbers = numbered(structure);
    let find = |ids: &[u32], html: bool| {
        ids.iter()
            .find(|&&id| match message.parts.get(id as usize).map(|p| &p.body) {
                Some(PartType::Text(_)) => !html,
                Some(PartType::Html(_)) => html,
                _ => false,
            })
            .and_then(|&id| numbers.get(id as usize).cloned().flatten())
    };
    (find(&message.text_body, false), find(&message.html_body, true))
}

/// Every part's number in mail-parser's order (each multipart before its
/// parts): the leaves' part numbers, none for the multiparts.
fn numbered(structure: &Node) -> Vec<Option<Vec<u32>>> {
    fn walk(node: &Node, path: &mut Vec<u32>, out: &mut Vec<Option<Vec<u32>>>) {
        match node {
            Node::Multipart { parts, .. } => {
                out.push(None);
                for (i, part) in parts.iter().enumerate() {
                    path.push(i as u32 + 1);
                    walk(part, path, out);
                    path.pop();
                }
            }
            Node::Leaf(_) => out.push(Some(if path.is_empty() { vec![1] } else { path.clone() })),
        }
    }
    let mut out = Vec::new();
    walk(structure, &mut Vec::new(), &mut out);
    out
}

/// A message made again from a record (see the module). Without a structure
/// (a server that gave none), one part: the HTML if that is all there is,
/// else the text.
pub(crate) fn rebuild(header: &[u8], structure: Option<&Node>, plain: Option<&Text>, html: Option<&Text>) -> Vec<u8> {
    let texts: Vec<(&[u32], &str)> = plain.iter().chain(html.iter()).map(|t| (t.at.as_slice(), t.text.as_str())).collect();
    let single;
    let structure = match structure {
        Some(structure) => structure,
        None => {
            let mime = if plain.is_none() && html.is_some() { "text/html" } else { "text/plain" };
            single = Node::Leaf(Leaf { mime: mime.into(), ..Leaf::default() });
            &single
        }
    };
    // Boundaries no kept text holds: their own mark, and a hash of the texts.
    let mark = format!("{:016x}", crate::labels::fnv64(texts.iter().map(|(_, t)| *t).collect::<String>().as_bytes()));
    let mut out = without_mime_fields(header);
    out.extend_from_slice(b"MIME-Version: 1.0\r\n");
    write_part(&mut out, structure, &mut Vec::new(), &texts, &mark);
    out
}

fn write_part(out: &mut Vec<u8>, node: &Node, path: &mut Vec<u32>, texts: &[(&[u32], &str)], mark: &str) {
    match node {
        Node::Multipart { subtype, parts } => {
            let level: Vec<String> = path.iter().map(u32::to_string).collect();
            let boundary = format!("=_sioul-learn-{mark}-{}", level.join("."));
            out.extend_from_slice(format!("Content-Type: multipart/{}; boundary=\"{boundary}\"\r\n\r\n", token(subtype)).as_bytes());
            for (i, part) in parts.iter().enumerate() {
                out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                path.push(i as u32 + 1);
                write_part(out, part, path, texts, mark);
                path.pop();
                out.extend_from_slice(b"\r\n");
            }
            out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        }
        Node::Leaf(leaf) => {
            let at = if path.is_empty() { vec![1] } else { path.clone() };
            let text = texts.iter().find(|(p, _)| *p == at.as_slice()).map(|(_, t)| *t);
            let mime = if leaf.mime.contains('/') { token(&leaf.mime) } else { "text/plain".to_string() };
            let mut content_type = format!("Content-Type: {mime}");
            if mime.starts_with("text/") {
                // What is kept is UTF-8, whatever the part was.
                content_type.push_str("; charset=\"utf-8\"");
            }
            if let Some(name) = &leaf.name {
                content_type.push_str(&format!("; name=\"{}\"", quoted(name)));
            }
            out.extend_from_slice(content_type.as_bytes());
            out.extend_from_slice(b"\r\n");
            if let Some(disposition) = &leaf.disposition {
                let mut line = format!("Content-Disposition: {}", token(disposition));
                if let Some(filename) = &leaf.filename {
                    line.push_str(&format!("; filename=\"{}\"", quoted(filename)));
                }
                out.extend_from_slice(line.as_bytes());
                out.extend_from_slice(b"\r\n");
            }
            // An attached message keeps its encoding (mail-parser opens it only
            // unencoded); everything else is written as it is kept.
            let encoding = if mime.starts_with("message/") && !leaf.encoding.is_empty() { token(&leaf.encoding) } else { "8bit".to_string() };
            out.extend_from_slice(format!("Content-Transfer-Encoding: {encoding}\r\n\r\n").as_bytes());
            out.extend_from_slice(text.unwrap_or("").as_bytes());
            out.extend_from_slice(b"\r\n");
        }
    }
}

/// The header block without its MIME fields (Content-*, MIME-Version), which
/// the message made again writes from its structure; lines end in CRLF.
fn without_mime_fields(header: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(header.len());
    let mut skipping = false;
    for line in header.split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            break;
        }
        if !matches!(line[0], b' ' | b'\t') {
            let name = line.split(|&b| b == b':').next().unwrap_or_default().trim_ascii().to_ascii_lowercase();
            skipping = name.starts_with(b"content-") || name == b"mime-version";
        }
        if !skipping {
            out.extend_from_slice(line);
            out.extend_from_slice(b"\r\n");
        }
    }
    out
}

/// A MIME token as written: no quote, space, control or separator.
fn token(text: &str) -> String {
    text.chars().filter(|c| c.is_ascii_graphic() && !matches!(c, '"' | ';' | ',' | '\\' | '(' | ')' | '<' | '>' | '@' | '[' | ']' | '?' | '=')).collect::<String>().to_ascii_lowercase()
}

/// A quoted string's inside: quotes and backslashes escaped, line ends out.
fn quoted(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).flat_map(|c| if matches!(c, '"' | '\\') { vec!['\\', c] } else { vec![c] }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::folders::Role;

    fn leaf(mime: &str) -> Leaf {
        Leaf { mime: mime.into(), charset: mime.starts_with("text/").then(|| "utf-8".into()), encoding: "7bit".into(), octets: 10, ..Leaf::default() }
    }

    fn record(header: &str, structure: Node, plain: Option<(&[u32], &str)>, html: Option<(&[u32], &str)>) -> Record {
        let text = |(at, text): (&[u32], &str)| Text { at: at.to_vec(), text: text.to_string() };
        Record {
            account: "home".into(),
            folder: "INBOX".into(),
            role: Role::Inbox,
            uidvalidity: 1,
            uid: 1,
            date: 0,
            flags: vec![],
            size: 0,
            header: header.into(),
            header_latin1: false,
            structure: Some(structure),
            plain: plain.map(text),
            html: html.map(text),
            fetched: 0,
        }
    }

    /// The same message, whole (the phone's card) and as a corpus record
    /// (training's card): the same excerpt, the same shape, the same headers.
    #[test]
    fn a_record_reads_as_its_message() {
        let raw = b"From: Alice <alice@example.org>\r\nSubject: Invoice\r\nMessage-ID: <1@example.org>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"b1\"\r\n\r\n--b1\r\nContent-Type: multipart/alternative; boundary=\"b2\"\r\n\r\n--b2\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPlease find the invoice attached.\r\n--b2\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Please find the invoice attached.</p>\r\n--b2--\r\n--b1\r\nContent-Type: application/pdf; name=\"invoice.pdf\"\r\nContent-Disposition: attachment; filename=\"invoice.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQK\r\n--b1\r\nContent-Type: image/png\r\nContent-Disposition: inline\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--b1--\r\n";
        let whole = Card::from_bytes(raw).unwrap();
        let structure = Node::Multipart {
            subtype: "mixed".into(),
            parts: vec![
                Node::Multipart { subtype: "alternative".into(), parts: vec![Node::Leaf(leaf("text/plain")), Node::Leaf(leaf("text/html"))] },
                Node::Leaf(Leaf { name: Some("invoice.pdf".into()), disposition: Some("attachment".into()), filename: Some("invoice.pdf".into()), encoding: "base64".into(), ..leaf("application/pdf") }),
                Node::Leaf(Leaf { disposition: Some("inline".into()), encoding: "base64".into(), ..leaf("image/png") }),
            ],
        };
        assert_eq!(bodies(&structure), (Some(vec![1, 1]), Some(vec![1, 2])));
        let header = "From: Alice <alice@example.org>\r\nSubject: Invoice\r\nMessage-ID: <1@example.org>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"b1\"\r\n\r\n";
        let card = card_of(&record(header, structure, Some((&[1, 1], "Please find the invoice attached.")), None)).unwrap();
        assert_eq!(card.excerpt.trim(), whole.excerpt.trim());
        assert_eq!(card.shape, whole.shape);
        assert_eq!(card.attachments, whole.attachments);
        assert_eq!((card.subject.as_str(), card.message_id.as_deref()), ("Invoice", whole.message_id.as_deref()));
        assert_eq!(card.headers.first("Content-Type"), whole.headers.first("Content-Type"), "the record's own headers");
    }

    /// HTML alone: mail-parser lends it as the text, and the excerpt is its visible text.
    #[test]
    fn html_alone_and_stand_ins() {
        let html_only = Node::Leaf(leaf("text/html"));
        assert_eq!(bodies(&html_only), (None, Some(vec![1])));
        let page = format!("<html><head><style>p {{ color: red }}</style></head><body><p>{}</p></body></html>", "Real words here. ".repeat(20));
        let card = card_of(&record("Subject: News\r\n", html_only, None, Some((&[1], &page)))).unwrap();
        assert!(card.excerpt.trim().starts_with("Real words here.") && !card.excerpt.contains("color"), "{}", card.excerpt);
        assert!(card.shape.html && !card.shape.text);
        // A text part that only says "view it in your browser": the Porch reads the HTML after it.
        let alternative = Node::Multipart { subtype: "alternative".into(), parts: vec![Node::Leaf(leaf("text/plain")), Node::Leaf(leaf("text/html"))] };
        let card = card_of(&record("Subject: News\r\n", alternative, Some((&[1], "View it in your browser")), Some((&[2], &page)))).unwrap();
        assert!(card.excerpt.starts_with("View it in your browser\n\nReal words here."), "{}", card.excerpt);
        // A named text part, second in a mixed message: an attachment, not the text.
        let mixed = Node::Multipart {
            subtype: "mixed".into(),
            parts: vec![Node::Leaf(leaf("text/plain")), Node::Leaf(Leaf { name: Some("notes.txt".into()), ..leaf("text/plain") })],
        };
        assert_eq!(bodies(&mixed), (Some(vec![1]), None));
        let card = card_of(&record("Subject: Notes\r\n", mixed, Some((&[1], "See the notes.")), None)).unwrap();
        assert_eq!(card.shape.parts, vec![("notes.txt".to_string(), "text/plain".to_string())]);
    }

    #[test]
    fn mime_fields_are_written_again() {
        let header = b"From: a@example.org\r\nContent-Type: multipart/mixed;\r\n\tboundary=\"x\"\r\nSubject: Hi\r\nMIME-Version: 1.0\r\nContent-Transfer-Encoding: 7bit\r\n\r\nbody";
        assert_eq!(without_mime_fields(header), b"From: a@example.org\r\nSubject: Hi\r\n".to_vec());
        assert_eq!(token("Multipart/Mixed\"; x"), "multipart/mixedx");
        assert_eq!(quoted("a \"b\"\\c\r\n"), "a \\\"b\\\"\\\\c");
    }
}
