// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail topics (docs/topics.md): a standing part of life (Health, Tax,
//! Choir) that Sioul writes on a message as a quiet word, one at most. A
//! topic is never a folder: mail stays where it is, and a topic is a way to
//! look at the one mailbox.
//!
//! What every device runs is here:
//!
//! - [`sender_words`] and [`line`]: what a message's sender adds to the line
//!   the model reads, when Sioul's own checks proved who sent it, and the line itself;
//! - [`seeds`]: the first topics, proposed from what the person already
//!   sorted (folders, projects, contacts' categories);
//! - [`table`]: the model folded into a table that judges a message without
//!   it, on the phone too.
//!
//! The training and its measures are sioul-learn's (`sioul_learn::topics`,
//! `sioul topics seeds` and `sioul topics eval`), on a computer only.

pub mod seeds;
pub mod table;

use crate::card::Card;

/// The sender words' version ([`sender_words`]): a table made with other
/// words reads its senders otherwise, and is refused.
pub const SENDERS: u32 = 1;

/// Words a line holds at most: fastText learns from lines of 1024 words and
/// drops the rest, so the sender's words come first ([`line`]).
pub const LINE_WORDS: usize = 1000;

/// The words a message's sender adds to its line, when Sioul's own checks
/// proved who sent it (`verified`: `trust::judge_sender` says Verified):
/// its address under a hash, `from:` and 12 hexadecimal digits; its domain,
/// `from-dom:example.org`; its mailing list, `list:` and its List-Id.
/// Unproven mail adds none, so that forged mail never borrows a sender's topic.
pub fn sender_words(card: &Card, verified: bool) -> Vec<String> {
    if !verified {
        return Vec::new();
    }
    let mut words = Vec::new();
    if let Some(address) = card.from_address.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        let hash = crate::spam::table::word_hash(&address.to_lowercase()) & 0xffff_ffff_ffff;
        words.push(format!("from:{hash:012x}"));
    }
    if let Some(domain) = card.sender_domain().map(|d| d.trim().trim_end_matches('.').to_lowercase()).filter(|d| !d.is_empty() && !d.contains(char::is_whitespace)) {
        words.push(format!("from-dom:{domain}"));
    }
    if let Some(list) = card.headers.first("List-Id").and_then(list_id) {
        words.push(format!("list:{list}"));
    }
    words
}

/// A List-Id's identifier (RFC 2919): between `<` and `>`, else the whole
/// value; lowercase, without spaces; none when empty.
fn list_id(value: &str) -> Option<String> {
    let inside = match (value.rfind('<'), value.rfind('>')) {
        (Some(open), Some(close)) if open < close => &value[open + 1..close],
        _ => value,
    };
    let id: String = inside.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect::<String>().to_lowercase();
    (!id.is_empty()).then_some(id)
}

/// The line a model reads, and the table: the sender's words first, then the
/// message's words (`spam::tokenize`), [`LINE_WORDS`] at most.
pub fn line(words: &[String], senders: &[String]) -> Vec<String> {
    senders.iter().chain(words).filter(|w| !w.is_empty() && !w.contains(char::is_whitespace)).take(LINE_WORDS).cloned().collect()
}

/// A topic's id from its name: ASCII, lowercase, accents taken off, every
/// other character a dash ("Impôts 2025" → "impots-2025"); "topic" when
/// nothing is left; "-2", "-3"… when it is taken.
pub fn slug(name: &str, taken: &[String]) -> String {
    let folded: String = crate::text::fold(name).into_iter().flat_map(char::to_lowercase).collect();
    let mut out = String::new();
    for c in folded.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let base = match out.trim_end_matches('-') {
        "" => "topic".to_string(),
        trimmed => trimmed.chars().take(40).collect::<String>().trim_end_matches('-').to_string(),
    };
    let mut id = base.clone();
    let mut n = 2;
    while taken.iter().any(|t| t == &id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(raw: &str) -> Card {
        Card::from_bytes(raw.as_bytes()).unwrap()
    }

    /// Proven mail names its sender, domain and list; unproven mail nothing.
    #[test]
    fn sender_words_only_for_proven_mail() {
        let c = card("From: Choir <News@Chorale.Example>\r\nList-Id: Choir news <news.chorale.example>\r\nSubject: Rehearsal\r\n\r\nSee you.\r\n");
        let words = sender_words(&c, true);
        assert_eq!(words.len(), 3, "{words:?}");
        assert!(words[0].starts_with("from:") && words[0].len() == 5 + 12 && words[0][5..].chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(words[1], "from-dom:chorale.example");
        assert_eq!(words[2], "list:news.chorale.example");
        // The same address, another case: the same word.
        let again = card("From: news@chorale.example\r\nSubject: x\r\n\r\ny\r\n");
        assert_eq!(sender_words(&again, true)[0], words[0]);
        assert!(sender_words(&c, false).is_empty(), "unproven: nothing");
        assert_eq!(list_id("plain.list.example"), Some("plain.list.example".into()));
        assert_eq!(list_id(" <> "), None);
    }

    /// The sender first, the words after, a thousand at most, never a space inside.
    #[test]
    fn the_line_puts_the_sender_first() {
        let words: Vec<String> = (0..1500).map(|i| format!("w{i}")).collect();
        let senders = vec!["from-dom:example.org".to_string(), String::new()];
        let line = line(&words, &senders);
        assert_eq!(line.len(), LINE_WORDS);
        assert_eq!(line[0], "from-dom:example.org");
        assert_eq!(line[1], "w0");
    }

    #[test]
    fn slugs_are_ascii_and_unique() {
        assert_eq!(slug("Impôts 2025", &[]), "impots-2025");
        assert_eq!(slug("  Book club! ", &[]), "book-club");
        assert_eq!(slug("Santé", &["sante".into()]), "sante-2");
        assert_eq!(slug("Santé", &["sante".into(), "sante-2".into()]), "sante-3");
        assert_eq!(slug("…", &[]), "topic");
    }
}
