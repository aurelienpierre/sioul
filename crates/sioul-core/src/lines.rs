// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Content lines, the text format vCard (RFC 6350 §3) and iCalendar (RFC 5545
//! §3.1) share: `NAME;PARAM=VALUE:value`, folded at 75 octets.
//!
//! Sioul changes cards and events line by line: the lines it edits are
//! replaced, every other line is kept byte for byte, so a photo, another
//! application's fields or a time zone it does not show come back exactly as
//! they were.

/// The logical lines of a text, folded lines joined again (a line starting
/// with a space or a tab continues the one before).
pub fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        match line.strip_prefix([' ', '\t']) {
            Some(rest) if !lines.is_empty() => lines.last_mut().into_iter().for_each(|last| last.push_str(rest)),
            _ if line.is_empty() => {}
            _ => lines.push(line.to_string()),
        }
    }
    lines
}

/// Lines back into text: CRLF, folded at 75 octets without cutting a character.
pub fn fold(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        let mut width = 0;
        for c in line.chars() {
            let size = c.len_utf8();
            if width + size > 75 {
                out.push_str("\r\n ");
                width = 1;
            }
            out.push(c);
            width += size;
        }
        out.push_str("\r\n");
    }
    out
}

/// A line's property name, uppercase, without its group: "item1.EMAIL;TYPE=…" → "EMAIL".
pub fn name(line: &str) -> String {
    let head = line.split([';', ':']).next().unwrap_or("");
    head.rsplit('.').next().unwrap_or(head).to_ascii_uppercase()
}

/// Where a line's value starts: its first colon outside quotes.
fn colon(line: &str) -> Option<usize> {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => return Some(i),
            _ => {}
        }
    }
    None
}

/// A line's value, after the first colon outside quotes.
pub fn value(line: &str) -> &str {
    colon(line).map_or("", |i| &line[i + 1..])
}

/// A line's parameters, in order: names uppercase, values without their
/// quotes ("ALTREP=\"sioul:contact/x\"" → ("ALTREP", "sioul:contact/x")); a
/// parameter without "=" comes with an empty value.
pub fn params(line: &str) -> Vec<(String, String)> {
    // A line without its colon (from a server or a mail) is all head.
    let head = colon(line).map_or(line, |i| &line[..i]);
    let mut parts = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (i, c) in head.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                parts.push(&head[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&head[start..]);
    parts
        .iter()
        .skip(1)
        .map(|p| match p.split_once('=') {
            Some((name, value)) => (name.trim().to_ascii_uppercase(), caret_decode(value.trim().trim_matches('"'))),
            None => (p.trim().to_ascii_uppercase(), String::new()),
        })
        .collect()
}

/// One parameter's value, the first time it is given.
pub fn param(line: &str, name: &str) -> Option<String> {
    params(line).into_iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

/// A parameter value as written: quoted when it holds what would end it
/// (RFC 5545 §3.2); quotes and line breaks in RFC 6868's caret form.
pub fn param_value(value: &str) -> String {
    let clean = value.replace('^', "^^").replace('"', "^'").replace("\r\n", "^n").replace(['\n', '\r'], "^n");
    // Other control characters have no place in a parameter (RFC 5545 §3.1).
    let clean: String = clean.chars().filter(|c| !c.is_control() || *c == '\t').collect();
    if clean.contains([':', ';', ',']) { format!("\"{clean}\"") } else { clean }
}

/// RFC 6868: "^'" is a quote, "^n" a line break, "^^" a caret.
fn caret_decode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '^' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('\'') => out.push('"'),
            Some('^') => out.push('^'),
            Some(other) => {
                out.push('^');
                out.push(other);
            }
            None => out.push('^'),
        }
    }
    out
}

/// A text value, escaped (RFC 6350 §3.4, RFC 5545 §3.3.11). A line break,
/// CR alone too, is "\n": nothing written here can start a line of its own;
/// other control characters, which text may not hold, are left out.
pub fn escape(text: &str) -> String {
    let text: String = text.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t')).collect();
    text.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace(['\n', '\r'], "\\n")
}

/// A text value as written, unescaped.
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// The parts of a structured value (N, ADR), each escaped, joined by semicolons.
pub fn structured(parts: &[String]) -> String {
    parts.iter().map(|p| escape(p)).collect::<Vec<_>>().join(";")
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// RFC 4648 base64, as Autocrypt headers and Basic authentication carry it.
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(BASE64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Base64 back into bytes; spaces and line breaks are skipped. None when it is not base64.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        let value = BASE64.iter().position(|&b| b == c)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_and_unfolds() {
        let long = format!("NOTE:{}", "é".repeat(60));
        let text = fold(&[long.clone(), "FN:A".into()]);
        assert!(text.lines().all(|l| l.len() <= 76), "{text}");
        assert_eq!(unfold(&text), vec![long, "FN:A".to_string()]);
        assert_eq!(name("item1.EMAIL;TYPE=work:a@example.org"), "EMAIL");
        assert_eq!(value("ATTENDEE;CN=\"Doe: Jane\":mailto:jane@example.org"), "mailto:jane@example.org");
        assert_eq!(params("CONTACT;ALTREP=\"sioul:contact/a;b\";LANGUAGE=fr:Jane"), vec![("ALTREP".to_string(), "sioul:contact/a;b".to_string()), ("LANGUAGE".to_string(), "fr".to_string())]);
        assert_eq!(param("RELATED-TO;reltype=DEPENDS-ON:x", "RELTYPE").as_deref(), Some("DEPENDS-ON"));
        assert_eq!((param_value("Venue"), param_value("Letter: 2")), ("Venue".to_string(), "\"Letter: 2\"".to_string()));
        assert_eq!(param(&format!("LINK;LABEL={}:x", param_value("The \"letter\"; 2^")), "LABEL").as_deref(), Some("The \"letter\"; 2^"));
        assert_eq!(unescape(&escape("a, b; c\\d\ne")), "a, b; c\\d\ne");
        assert_eq!(base64_encode(b"jane:pass word"), "amFuZTpwYXNzIHdvcmQ=");
        assert_eq!(base64_decode("amFuZTpwYXNz\r\n IHdvcmQ=").unwrap(), b"jane:pass word");
    }

    #[test]
    fn odd_lines_from_elsewhere() {
        // No colon, a character of two bytes last: all head, no panic.
        assert_eq!(params("X-FOO;A=é"), vec![("A".to_string(), "é".to_string())]);
        assert_eq!(value("X-FOO;A=é"), "");
        // A CR alone cannot start a line of its own.
        let title = escape("Crash\rSTATUS:COMPLETED\u{7}");
        assert!(!title.contains(['\r', '\n', '\u{7}']), "{title}");
        assert_eq!(unescape(&title), "Crash\nSTATUS:COMPLETED");
        assert!(!param_value("a\rb").contains('\r'));
    }
}
