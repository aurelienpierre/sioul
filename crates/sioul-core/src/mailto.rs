// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `mailto:` addresses (RFC 6068): the message a link, or another
//! application, asks to write. Recipients (the part before "?", and `to`,
//! `cc`, `bcc`), `subject`, `body`, and `In-Reply-To` for the message it
//! answers. Nothing else is read: above all no `attach` or `attachment`,
//! which some programs take and which would let a web page name one of your
//! files, nor `from`. Nothing here sends anything: the address only fills a
//! draft (docs/client.md, "Writing from other applications").

use crate::compose::{address_of, split_addresses};
use mail_parser::MessageParser;

/// What a `mailto:` address asks.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Mailto {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    /// The text, line breaks as "\n".
    pub body: String,
    /// The message answered, its Message-ID with its angle brackets.
    pub in_reply_to: Option<String>,
}

/// Whether `text` is a `mailto:` address, in any case.
pub fn is_mailto(text: &str) -> bool {
    text.trim_start().get(..7).is_some_and(|scheme| scheme.eq_ignore_ascii_case("mailto:"))
}

/// Reads a `mailto:` address; none when `uri` is not one.
pub fn parse(uri: &str) -> Option<Mailto> {
    let uri = uri.trim();
    if !is_mailto(uri) {
        return None;
    }
    // A fragment is no part of a mailto: address (RFC 6068 §2).
    let rest = uri[7..].split('#').next().unwrap_or_default();
    let (to, fields) = rest.split_once('?').unwrap_or((rest, ""));
    let mut mailto = Mailto::default();
    mailto.to.extend(split_addresses(&one_line(&decode(to))));
    for field in fields.split('&').filter(|f| !f.is_empty()) {
        let (name, value) = field.split_once('=').unwrap_or((field, ""));
        let value = decode(value);
        match decode(name).trim().to_ascii_lowercase().as_str() {
            "to" => mailto.to.extend(split_addresses(&header(&value))),
            "cc" => mailto.cc.extend(split_addresses(&header(&value))),
            "bcc" => mailto.bcc.extend(split_addresses(&header(&value))),
            "subject" if mailto.subject.is_empty() => mailto.subject = header(&value).trim().to_string(),
            // Encoded words mean nothing in the body (RFC 6068 §2): kept as written.
            "body" if mailto.body.is_empty() => mailto.body = value.replace("\r\n", "\n").replace('\r', "\n"),
            "in-reply-to" if mailto.in_reply_to.is_none() => {
                let id = one_line(&value).trim().to_string();
                if id.starts_with('<') && id.ends_with('>') && id.len() > 2 {
                    mailto.in_reply_to = Some(id);
                }
            }
            _ => {}
        }
    }
    for list in [&mut mailto.to, &mut mailto.cc, &mut mailto.bcc] {
        once_each(list);
    }
    Some(mailto)
}

/// Each address once in a list, the first spelling kept: "Jane <jane@x.org>"
/// and "JANE@x.org" are one.
pub fn once_each(list: &mut Vec<String>) {
    let mut seen = std::collections::BTreeSet::new();
    list.retain(|entry| seen.insert(address_of(entry).unwrap_or_else(|| entry.trim().to_string()).to_lowercase()));
}

/// Percent-decoding (RFC 3986 §2.1): the bytes read as UTF-8, a wrong one
/// replaced; "+" stays "+" (a space is "%20" in mailto:, RFC 6068 §5), and a
/// "%" not followed by two hexadecimal digits stays as it is.
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(high), Some(low)) = (bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex))
        {
            out.push((high * 16 + low) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A header's value on one line: a line break would be a header of its own.
fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// A header's value as written, its MIME encoded words decoded (RFC 2047,
/// allowed in header values by RFC 6068 §2): read as a header by the same
/// parser as received mail.
fn header(value: &str) -> String {
    let value = one_line(value);
    if !value.contains("=?") {
        return value;
    }
    let raw = format!("Subject: {value}\r\n\r\n");
    MessageParser::default().parse(raw.as_bytes()).and_then(|m| m.subject().map(str::to_string)).unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to(uri: &str) -> Vec<String> {
        parse(uri).expect("a mailto: address").to
    }

    // RFC 6068 §6.1, basic examples.
    #[test]
    fn basic_examples() {
        assert_eq!(to("mailto:chris@example.com"), ["chris@example.com"]);
        assert_eq!(parse("mailto:infobot@example.com?subject=current-issue").unwrap().subject, "current-issue");
        assert_eq!(parse("mailto:infobot@example.com?body=send%20current-issue").unwrap().body, "send current-issue");
        assert_eq!(parse("mailto:infobot@example.com?body=send%20current-issue%0D%0Asend%20index").unwrap().body, "send current-issue\nsend index");
        let answer = parse("mailto:list@example.org?In-Reply-To=%3C3469A91.D10AF4C@example.com%3E").unwrap();
        assert_eq!((answer.to.as_slice(), answer.in_reply_to.as_deref()), (&["list@example.org".to_string()][..], Some("<3469A91.D10AF4C@example.com>")));
        assert_eq!(parse("mailto:majordomo@example.com?body=subscribe%20bamboo-l").unwrap().body, "subscribe bamboo-l");
        let copy = parse("mailto:joe@example.com?cc=bob@example.com&body=hello").unwrap();
        assert_eq!((copy.to, copy.cc, copy.body.as_str()), (vec!["joe@example.com".to_string()], vec!["bob@example.com".to_string()], "hello"));
        // The RFC's own wrong example: the second "?" belongs to the cc's value.
        assert_eq!(parse("mailto:joe@example.com?cc=bob@example.com?body=hello").unwrap().cc, ["bob@example.com?body=hello"]);
        assert_eq!(to("mailto:gorby%25kremvax@example.com"), ["gorby%kremvax@example.com"]);
        let unlikely = parse("mailto:unlikely%3Faddress@example.com?blat=foop").unwrap();
        assert_eq!((unlikely.to.as_slice(), unlikely.subject.as_str(), unlikely.body.as_str()), (&["unlikely?address@example.com".to_string()][..], "", ""));
        assert_eq!(to("mailto:Mike%26family@example.org"), ["Mike&family@example.org"]);
    }

    // RFC 6068 §6.2, complicated addresses.
    #[test]
    fn quoted_local_parts() {
        assert_eq!(to("mailto:%22not%40me%22@example.org"), ["\"not@me\"@example.org"]);
        assert_eq!(to("mailto:%22oh%5C%5Cno%22@example.org"), ["\"oh\\\\no\"@example.org"]);
        assert_eq!(to("mailto:%22%5C%5C%5C%22it's%5C%20ugly%5C%5C%5C%22%22@example.org"), ["\"\\\\\\\"it's\\ ugly\\\\\\\"\"@example.org"]);
    }

    // RFC 6068 §6.3, beyond ASCII.
    #[test]
    fn beyond_ascii() {
        assert_eq!(parse("mailto:user@example.org?subject=caf%C3%A9").unwrap().subject, "café");
        assert_eq!(parse("mailto:user@example.org?subject=%3D%3Futf-8%3FQ%3Fcaf%3DC3%3DA9%3F%3D").unwrap().subject, "café");
        assert_eq!(parse("mailto:user@example.org?subject=%3D%3Fiso-8859-1%3FQ%3Fcaf%3DE9%3F%3D").unwrap().subject, "café");
        let both = parse("mailto:user@example.org?subject=caf%C3%A9&body=caf%C3%A9").unwrap();
        assert_eq!((both.subject.as_str(), both.body.as_str()), ("café", "café"));
        let natto = parse("mailto:user@%E7%B4%8D%E8%B1%86.example.org?subject=Test&body=NATTO").unwrap();
        assert_eq!((natto.to.as_slice(), natto.subject.as_str(), natto.body.as_str()), (&["user@納豆.example.org".to_string()][..], "Test", "NATTO"));
        // Encoded words mean nothing in the body.
        assert_eq!(parse("mailto:x@example.org?body=%3D%3Futf-8%3FQ%3Fcaf%3DC3%3DA9%3F%3D").unwrap().body, "=?utf-8?Q?caf=C3=A9?=");
        // A wrong byte is replaced, never a failure.
        assert_eq!(parse("mailto:x@example.org?subject=caf%E9").unwrap().subject, "caf\u{FFFD}");
    }

    // RFC 6068 §2: three ways to the same two recipients.
    #[test]
    fn recipients_in_the_path_and_the_fields() {
        let two = ["addr1@an.example".to_string(), "addr2@an.example".to_string()];
        assert_eq!(to("mailto:addr1@an.example,addr2@an.example"), two);
        assert_eq!(to("mailto:?to=addr1@an.example,addr2@an.example"), two);
        assert_eq!(to("mailto:addr1@an.example?to=addr2@an.example"), two);
        // Once each, the first spelling kept.
        assert_eq!(to("mailto:a@example.org?to=A@EXAMPLE.ORG,Ann%20%3Ca@example.org%3E,b@example.org"), ["a@example.org", "b@example.org"]);
        // Names in the fields' values, encoded words among them.
        assert_eq!(parse("mailto:?cc=%22Doe%2C%20Jane%22%20%3Cjane@example.org%3E").unwrap().cc, ["\"Doe, Jane\" <jane@example.org>"]);
        assert_eq!(parse("mailto:?bcc=%3D%3Futf-8%3FQ%3FJos%3DC3%3DA9%3F%3D%20%3Cjose@example.org%3E").unwrap().bcc, ["José <jose@example.org>"]);
    }

    #[test]
    fn what_is_left_out() {
        // "+" is no space; names of fields in any case; the scheme too.
        let plus = parse("MAILTO:a+tag@example.org?SUBJECT=1+1&Body=a+b").unwrap();
        assert_eq!((plus.to.as_slice(), plus.subject.as_str(), plus.body.as_str()), (&["a+tag@example.org".to_string()][..], "1+1", "a+b"));
        // Files, the sender, and anything else are never taken from an address.
        let sly = parse("mailto:x@example.org?attach=/home/me/.ssh/id_rsa&attachment=file:///etc/passwd&from=boss@example.org&reply-to=y@example.org").unwrap();
        assert_eq!(sly, Mailto { to: vec!["x@example.org".to_string()], ..Mailto::default() });
        // A line break cannot make a header of its own; a fragment is no part of it.
        assert_eq!(parse("mailto:x@example.org?subject=a%0D%0ABcc:%20y@example.org#top").unwrap().subject, "a  Bcc: y@example.org");
        assert_eq!(to("mailto:x@example.org#top"), ["x@example.org"]);
        // The first subject and body count; an In-Reply-To without its brackets does not.
        let twice = parse("mailto:?subject=one&subject=two&body=first&body=second&in-reply-to=nobrackets@example.org").unwrap();
        assert_eq!((twice.subject.as_str(), twice.body.as_str(), twice.in_reply_to), ("one", "first", None));
        // Empty, and not one.
        assert_eq!(parse("mailto:"), Some(Mailto::default()));
        assert_eq!(parse("https://example.org/?to=x@example.org"), None);
        assert_eq!(parse("mail"), None);
        assert!(is_mailto("  MailTo:x@example.org") && !is_mailto("mailto"));
        // A "%" that encodes nothing stays.
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("%zz%4"), "%zz%4");
    }
}
