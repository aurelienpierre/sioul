// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The raw header block, unfolded (RFC 5322 §2.2.3).
//!
//! mail-parser decodes what people read: names, subjects, bodies. The headers
//! servers write to record a message's journey (Authentication-Results,
//! Received, the spam verdicts) are read here from the raw block instead,
//! because they repeat and their order matters: each server adds its own on
//! top (RFC 5321 §4.4).

/// Header fields in the order they appear, names as written, values unfolded.
#[derive(Debug, Default, Clone)]
pub struct RawHeaders {
    fields: Vec<(String, String)>,
}

impl RawHeaders {
    /// Reads the header block of a raw message.
    pub fn parse(raw: &[u8]) -> Self {
        let text = String::from_utf8_lossy(header_block(raw));
        let mut fields: Vec<(String, String)> = Vec::new();
        for line in text.split('\n') {
            push_line(&mut fields, line.strip_suffix('\r').unwrap_or(line));
        }
        RawHeaders { fields }
    }

    /// Every value of a header, top to bottom (newest server first).
    pub fn all<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a str> + use<'a> {
        let name = name.to_string();
        self.fields
            .iter()
            .filter(move |(n, _)| n.eq_ignore_ascii_case(&name))
            .map(|(_, v)| v.as_str())
    }

    /// The topmost value of a header.
    pub fn first(&self, name: &str) -> Option<&str> {
        self.all(name).next()
    }

    /// Whether the header is present at all.
    pub fn has(&self, name: &str) -> bool {
        self.first(name).is_some()
    }

    /// Every field, top to bottom: its name as written, its value unfolded.
    pub fn fields(&self) -> impl Iterator<Item = (&str, &str)> {
        self.fields.iter().map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// The `n` topmost fields alone: what servers added above a given one.
    pub fn top(&self, n: usize) -> RawHeaders {
        RawHeaders { fields: self.fields.iter().take(n).cloned().collect() }
    }
}

/// The header block ends at the first empty line (RFC 5322 §2.1), whatever
/// ends the lines: a body's own blank line further down never extends it, so
/// the body cannot pass for headers (an Authentication-Results among them).
fn header_block(raw: &[u8]) -> &[u8] {
    let mut start = 0;
    for line in raw.split(|&b| b == b'\n') {
        if line.is_empty() || line == b"\r" {
            return &raw[..start];
        }
        start += line.len() + 1;
    }
    raw
}

/// A line starting with a space or a tab continues the previous field (folding).
fn push_line(fields: &mut Vec<(String, String)>, line: &str) {
    let continues = line.starts_with(' ') || line.starts_with('\t');
    if let (true, Some(last)) = (continues, fields.last_mut()) {
        last.1.push(' ');
        last.1.push_str(line.trim());
    } else if let Some((name, value)) = line.split_once(':') {
        fields.push((name.trim().to_string(), value.trim().to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unfolds_and_keeps_order() {
        let raw = b"Received: from a\r\n by b\r\nReceived: from c\r\nSubject: hi\r\n\r\nbody: not a header\r\n";
        let h = RawHeaders::parse(raw);
        let received: Vec<&str> = h.all("received").collect();
        assert_eq!(received, vec!["from a by b", "from c"]);
        assert_eq!(h.first("Subject"), Some("hi"));
        assert!(!h.has("body"));
        // Lines ended by LF alone, and a body holding a CRLF blank line: still the body.
        let raw = b"Subject: hi\n\nAuthentication-Results: mx.example.net; dmarc=pass\r\n\r\nbye\r\n";
        assert!(!RawHeaders::parse(raw).has("Authentication-Results"));
    }
}
