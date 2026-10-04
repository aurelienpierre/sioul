// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A card: what the Porch knows about one message before you open it.

use crate::headers::RawHeaders;
use mail_parser::{MessageParser, MimeHeaders};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Characters of the body kept for the detectors and the preview.
const EXCERPT_LIMIT: usize = 6000;

/// Where a message sits on its server: the folder's UIDVALIDITY and the message's
/// UID, which together name it for good (RFC 9051 §2.3.1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImapOrigin {
    pub validity: u32,
    pub uid: u32,
}

/// One message, read once, kept small.
#[derive(Debug, Clone)]
pub struct Card {
    /// Where the message is stored, when it came from disk.
    pub path: Option<PathBuf>,
    /// The account it came through, when it was read from one.
    pub account: Option<String>,
    /// Where it sits on the server, when Sioul fetched it.
    pub origin: Option<ImapOrigin>,
    pub from_name: Option<String>,
    pub from_address: Option<String>,
    /// To whom it went: names, else addresses.
    pub to: Vec<String>,
    pub subject: String,
    /// The Date header, as Unix seconds.
    pub date: Option<i64>,
    pub message_id: Option<String>,
    /// The start of the text body (HTML-only mail is converted by mail-parser).
    pub excerpt: String,
    pub attachments: Vec<String>,
    /// A newsletter or a mailing list (List-Id, List-Unsubscribe, Precedence: bulk/list).
    pub is_list: bool,
    pub headers: RawHeaders,
}

impl Card {
    /// Reads a raw RFC 5322 message; none if it cannot be parsed at all.
    pub fn from_bytes(raw: &[u8]) -> Option<Card> {
        let message = MessageParser::default().parse(raw)?;
        let from = message.from().and_then(|a| a.first());
        let headers = RawHeaders::parse(raw);
        let excerpt: String = message.body_text(0).map(|b| b.chars().take(EXCERPT_LIMIT).collect()).unwrap_or_default();
        Some(Card {
            path: None,
            account: None,
            origin: None,
            from_name: from.and_then(|a| a.name()).map(str::to_string),
            from_address: from.and_then(|a| a.address()).map(|a| a.to_ascii_lowercase()),
            to: message.to().map(|a| a.iter().filter_map(|p| p.name().or(p.address()).map(str::to_string)).collect()).unwrap_or_default(),
            subject: message.subject().unwrap_or("").to_string(),
            date: message.date().map(|d| d.to_timestamp()),
            message_id: message.message_id().map(str::to_string),
            excerpt,
            attachments: message.attachments().filter(|p| !is_pgp_part(p)).filter_map(|p| p.attachment_name()).map(str::to_string).collect(),
            is_list: is_list(&headers),
            headers,
        })
    }

    /// The domain of the sender's address.
    pub fn sender_domain(&self) -> Option<&str> {
        self.from_address.as_deref()?.rsplit_once('@').map(|(_, d)| d)
    }

    /// The name to show: the display name, else the address.
    pub fn sender(&self) -> &str {
        self.from_name.as_deref().or(self.from_address.as_deref()).unwrap_or("unknown sender")
    }

    /// The whole text of the message, read again from disk: plain text only,
    /// HTML converted by mail-parser, so nothing remote loads and nothing runs.
    pub fn full_text(&self) -> Option<String> {
        let raw = std::fs::read(self.path.as_ref()?).ok()?;
        let message = MessageParser::default().parse(&raw)?;
        Some(message.body_text(0).map(|b| b.into_owned()).unwrap_or_default())
    }
}

/// An OpenPGP signature or encrypted payload: part of the message's protection, not something attached.
fn is_pgp_part(part: &mail_parser::MessagePart) -> bool {
    use mail_parser::MimeHeaders;
    part.content_type().is_some_and(|t| {
        let kind = format!("{}/{}", t.ctype(), t.subtype().unwrap_or("")).to_ascii_lowercase();
        matches!(kind.as_str(), "application/pgp-signature" | "application/pgp-encrypted" | "application/pgp-keys")
            || (kind == "application/octet-stream" && t.attribute("name").is_some_and(|n| n.eq_ignore_ascii_case("encrypted.asc")))
    })
}

fn is_list(headers: &RawHeaders) -> bool {
    let bulk = headers
        .first("Precedence")
        .is_some_and(|p| matches!(p.trim().to_ascii_lowercase().as_str(), "bulk" | "list"));
    bulk || headers.has("List-Id") || headers.has("List-Unsubscribe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_simple_message() {
        let raw = b"From: \"Jane Doe\" <Jane@Example.org>\r\nSubject: =?UTF-8?Q?R=C3=A9union?=\r\nList-Id: <news.example.org>\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\r\nHello.\r\n";
        let card = Card::from_bytes(raw).unwrap();
        assert_eq!(card.sender(), "Jane Doe");
        assert_eq!(card.from_address.as_deref(), Some("jane@example.org"));
        assert_eq!(card.sender_domain(), Some("example.org"));
        assert_eq!(card.subject, "Réunion");
        assert!(card.is_list);
        assert!(card.date.is_some());
    }
}
