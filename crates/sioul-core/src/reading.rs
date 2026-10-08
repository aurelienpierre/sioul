// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A message laid out for reading: who wrote to whom, the attachments apart,
//! and the text cut into what this message says, the messages it quotes, the
//! headers of the messages it forwards, and the signature.
//!
//! HTML mail is shown formatted once made safe (`safe_html`): only text,
//! structure and links survive; images, styles, scripts and anything that
//! would load from the network are dropped, so nothing remote loads and
//! nothing runs. Plain text keeps its parts, its links made clickable.

use mail_parser::{Address, MessageParser, MimeHeaders, PartType};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// One person, as a header names them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Person {
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attachment {
    /// Its place among the message's attachments, to open it.
    pub index: u32,
    pub name: String,
    /// "application/pdf".
    pub mime: String,
    pub size: usize,
}

/// A piece of a message's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Part {
    /// What this message says.
    Text { text: String },
    /// "Le 1 oct. 2026, Jean a écrit :", above what it quotes.
    Attribution { text: String },
    /// A quoted message; `depth` 1 is the message answered, 2 the one before it.
    Quote { depth: u8, text: String },
    /// The headers of a forwarded or answered message, as the text repeats them.
    Headers { fields: Vec<(String, String)> },
    /// After "-- ".
    Signature { text: String },
}

/// A message, laid out for reading.
#[derive(Debug, Clone, Serialize)]
pub struct Reading {
    pub from: Vec<Person>,
    pub to: Vec<Person>,
    pub cc: Vec<Person>,
    pub subject: String,
    pub attachments: Vec<Attachment>,
    pub parts: Vec<Part>,
    /// The HTML version, made safe, when the message has one.
    pub html: Option<Html>,
}

/// A message's HTML, made safe and calm to read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Html {
    /// What this message says.
    pub main: String,
    /// The messages it answers or forwards, to fold.
    pub quoted: Option<String>,
    /// It had images: none is shown.
    pub images_hidden: bool,
}

/// Reads a message file for the reading pane.
pub fn read(path: &Path) -> Option<Reading> {
    read_bytes(&std::fs::read(path).ok()?)
}

/// A message laid out for reading, from its bytes (a decrypted message lives only in memory).
pub fn read_bytes(raw: &[u8]) -> Option<Reading> {
    let message = MessageParser::default().parse(raw)?;
    let text = message.body_text(0).map(|b| b.into_owned()).unwrap_or_default();
    Some(Reading {
        from: people(message.from()),
        to: people(message.to()),
        cc: people(message.cc()),
        subject: message.subject().unwrap_or("").to_string(),
        attachments: message
            .attachments()
            .enumerate()
            .map(|(i, part)| Attachment {
                index: i as u32,
                name: file_name(part.attachment_name(), i),
                mime: part.content_type().map_or_else(String::new, |t| format!("{}/{}", t.ctype(), t.subtype().unwrap_or("octet-stream"))),
                size: part.contents().len(),
            })
            .collect(),
        parts: parts(&text),
        html: message.html_part(0).and_then(|part| match &part.body {
            PartType::Html(html) => Some(safe_html(html)),
            _ => None,
        }),
    })
}

/// Where the messages a reply quotes or a forward carries begin, in the HTML of
/// Gmail, Apple Mail and Thunderbird, and Outlook.
const QUOTE_MARKERS: &[&str] = &[
    "<div class=\"gmail_quote",
    "<blockquote type=\"cite\"",
    "<div class=\"moz-cite-prefix\"",
    "<div id=\"divrplyfwdmsg\"",
    "<div id=\"appendonsend\"",
    "<hr id=\"stopspelling\"",
    "border-top:solid #e1e1e1",
    "border-top:solid #b5c4df",
];

/// A message's HTML, made safe: the quoted history split off, then each part cleaned.
pub fn safe_html(html: &str) -> Html {
    // ASCII lowercase keeps every byte where it was, so positions hold in `html`.
    let lower = html.to_ascii_lowercase();
    // A marker that is a tag splits where it starts; a style splits at the start of its tag.
    let split = QUOTE_MARKERS
        .iter()
        .filter_map(|m| {
            let at = lower.find(m)?;
            if m.starts_with('<') { Some(at) } else { lower[..at].rfind('<') }
        })
        .min()
        .filter(|at| html.is_char_boundary(*at));
    let (main, quoted) = match split {
        Some(at) if at > 0 => (&html[..at], Some(&html[at..])),
        _ => (html, None),
    };
    Html {
        main: clean_html(main),
        quoted: quoted.map(clean_html).filter(|q| !q.trim().is_empty()),
        images_hidden: lower.contains("<img"),
    }
}

/// Only text, structure and links: no images, styles, scripts, forms or frames,
/// no attribute but a link's address, and only web and mail links. Layout
/// tables become plain blocks, headings bold paragraphs, empty blocks go.
fn clean_html(html: &str) -> String {
    let tags: HashSet<&str> = [
        "a", "b", "strong", "i", "em", "u", "p", "br", "div", "span", "ul", "ol", "li", "blockquote", "pre", "code",
        "hr", "table", "thead", "tbody", "tfoot", "tr", "td", "th", "small", "sup", "sub", "dl", "dt", "dd", "h1", "h2",
        "h3", "h4", "h5", "h6",
    ]
    .into_iter()
    .collect();
    let removed: HashSet<&str> = ["style", "script", "head", "title"].into_iter().collect();
    let attributes: HashMap<&str, HashSet<&str>> = [("a", ["href"].into_iter().collect())].into_iter().collect();
    let cleaned = ammonia::Builder::empty()
        .tags(tags)
        .clean_content_tags(removed)
        .tag_attributes(attributes)
        // ammonia keeps "lang" and "title" everywhere unless told otherwise.
        .generic_attributes(HashSet::new())
        .url_schemes(["http", "https", "mailto"].into_iter().collect())
        // A relative link has no page to be relative to in a mail: opened, it would
        // name a file of this computer ("../Downloads/x") or a share ("//host/x").
        .url_relative(ammonia::UrlRelative::Deny)
        .link_rel(None)
        .strip_comments(true)
        .clean(html)
        .to_string();
    let mut out = cleaned;
    // Tables laid out a page, not data: one block after another reads better.
    for (from, to) in [
        ("<table>", "<div>"), ("</table>", "</div>"), ("<tr>", "<div>"), ("</tr>", "</div>"), ("<td>", "<div>"),
        ("</td>", "</div>"), ("<th>", "<div><b>"), ("</th>", "</b></div>"), ("<tbody>", ""), ("</tbody>", ""),
        ("<thead>", ""), ("</thead>", ""), ("<tfoot>", ""), ("</tfoot>", ""),
    ] {
        out = out.replace(from, to);
    }
    for level in 1..=6 {
        out = out.replace(&format!("<h{level}>"), "<p><b>").replace(&format!("</h{level}>"), "</b></p>");
    }
    // Spacer blocks and runs of line breaks, which layouts use for room.
    loop {
        let before = out.len();
        for empty in ["<div></div>", "<div>&nbsp;</div>", "<div> </div>", "<p></p>", "<p>&nbsp;</p>", "<span></span>", "<br><br><br>"] {
            out = out.replace(empty, if empty.starts_with("<br>") { "<br><br>" } else { "" });
        }
        if out.len() == before {
            break;
        }
    }
    out.trim().to_string()
}

/// Plain text as HTML: escaped, its web links clickable and shown by their site
/// ("example.org/…"), its lines kept.
pub fn linkify(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push_str("<br>");
        }
        let mut rest = line;
        while let Some(at) = ["https://", "http://"].iter().filter_map(|p| rest.find(p)).min() {
            out.push_str(&escape(&rest[..at]));
            let url_len = rest[at..].find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | ')' | ']')).unwrap_or(rest.len() - at);
            let url = rest[at..at + url_len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
            let site = url.split("://").nth(1).unwrap_or(url).split(['/', '?', '#']).next().unwrap_or(url);
            let shown = if url.trim_end_matches('/').ends_with(site) { site.to_string() } else { format!("{site}/…") };
            out.push_str(&format!("<a href=\"{}\">{}</a>", escape(url), escape(&shown)));
            rest = &rest[at + url.len()..];
        }
        out.push_str(&escape(rest));
    }
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// One attachment's name and bytes, to open or save it.
/// The iCalendar a message carries, inline or attached (an invitation, RFC 6047):
/// the part with a METHOD first, as Gmail and Outlook send both.
pub fn calendar(path: &Path) -> Option<String> {
    let raw = std::fs::read(path).ok()?;
    let message = MessageParser::default().parse(&raw)?;
    let mut found: Vec<String> = message
        .parts
        .iter()
        .filter(|part| {
            part.content_type().is_some_and(|t| {
                let kind = format!("{}/{}", t.ctype(), t.subtype().unwrap_or("")).to_ascii_lowercase();
                kind == "text/calendar" || kind == "application/ics"
            })
        })
        .filter_map(|part| match &part.body {
            mail_parser::PartType::Text(text) => Some(text.to_string()),
            mail_parser::PartType::Binary(bytes) | mail_parser::PartType::InlineBinary(bytes) => String::from_utf8(bytes.to_vec()).ok(),
            _ => None,
        })
        .filter(|text| text.contains("BEGIN:VCALENDAR"))
        .collect();
    found.sort_by_key(|text| !text.contains("METHOD:"));
    found.into_iter().next()
}

pub fn attachment(path: &Path, index: u32) -> Option<(String, Vec<u8>)> {
    attachment_from(&std::fs::read(path).ok()?, index)
}

/// An attachment from a message's bytes: its name and content.
pub fn attachment_from(raw: &[u8], index: u32) -> Option<(String, Vec<u8>)> {
    let message = MessageParser::default().parse(raw)?;
    let part = message.attachments().nth(index as usize)?;
    Some((file_name(part.attachment_name(), index as usize), part.contents().to_vec()))
}

fn people(address: Option<&Address>) -> Vec<Person> {
    address.map_or_else(Vec::new, |a| {
        a.iter().map(|p| Person { name: p.name().map(str::to_string), address: p.address().map(str::to_string) }).collect()
    })
}

/// A name safe to write in a folder, on every system, since what is saved may
/// be synced to another: no path, no control characters, none of the
/// characters Windows forbids (`"report.pdf:x"` would write a hidden stream
/// there), no dot or space at the end, and no device's name ("nul.txt"); the
/// extension kept.
fn file_name(name: Option<&str>, index: usize) -> String {
    let base = name.and_then(|n| n.rsplit(['/', '\\']).next()).unwrap_or("").trim();
    let clean: String = base.chars().filter(|c| !c.is_control()).map(|c| if "<>:\"|?*".contains(c) { '_' } else { c }).collect();
    let clean = clean.trim_start_matches('.').trim().trim_end_matches(['.', ' ']);
    if clean.is_empty() {
        return format!("attachment-{}", index + 1);
    }
    let stem = clean.split('.').next().unwrap_or(clean).trim_end().to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit());
    if device { format!("_{clean}") } else { clean.to_string() }
}

/// Cuts a message's text into parts (see `Part`), its quoted history read
/// with the words in use (`words::current`).
pub fn parts(text: &str) -> Vec<Part> {
    parts_with(&crate::words::current().quotes, text)
}

/// Whether one of a list's words, folded, is what `test` asks of a folded text.
fn any_folded(list: &[String], test: impl Fn(&str) -> bool) -> bool {
    list.iter().map(|w| crate::words::folded(w)).any(|w| !w.is_empty() && test(&w))
}

/// Cuts a message's text into parts with these words (`words::Quotes`): the
/// lines that open a forwarded or answered message, the header names of a
/// repeated header block, the endings of an attribution, in any language
/// your correspondents' mail programs write them.
pub fn parts_with(q: &crate::words::Quotes, text: &str) -> Vec<Part> {
    let lines: Vec<&str> = text.lines().collect();
    let mut parts: Vec<Part> = Vec::new();
    let mut i = 0;
    // After the headers of a message answered or forwarded, the rest is that message.
    let mut quoted_below = 0u8;
    while i < lines.len() {
        let line = lines[i];
        let lower = line.trim().to_lowercase();
        let folded = crate::words::folded(line);
        if any_folded(&q.openings, |o| folded.starts_with(o)) || is_separator(&lower) && header_block_at(q, &lines, i + 1).is_some() {
            i += 1;
            continue;
        }
        if let Some((fields, next)) = header_block_at(q, &lines, i) {
            parts.push(Part::Headers { fields });
            quoted_below = quoted_below.saturating_add(1);
            i = next;
            continue;
        }
        if is_attribution(q, &lines, i) {
            take_previous_line_into_attribution(q, &mut parts, line);
            i += 1;
            continue;
        }
        if line == "-- " || line == "--" {
            push(&mut parts, Part::Signature { text: String::new() });
            i += 1;
            continue;
        }
        let (depth, content) = quote_depth(line);
        let depth = depth.saturating_add(quoted_below);
        let in_signature = matches!(parts.last(), Some(Part::Signature { .. })) && depth == 0;
        let part = if in_signature {
            Part::Signature { text: content.to_string() }
        } else if depth > 0 {
            Part::Quote { depth, text: content.to_string() }
        } else {
            Part::Text { text: content.to_string() }
        };
        push(&mut parts, part);
        i += 1;
    }
    parts.into_iter().filter_map(tidy).collect()
}

/// Adds a line to the last part when it is of the same kind, else starts a new part.
fn push(parts: &mut Vec<Part>, part: Part) {
    match (parts.last_mut(), part) {
        (Some(Part::Text { text }), Part::Text { text: more })
        | (Some(Part::Signature { text }), Part::Signature { text: more }) => {
            text.push('\n');
            text.push_str(&more);
        }
        (Some(Part::Quote { depth, text }), Part::Quote { depth: d, text: more }) if *depth == d => {
            text.push('\n');
            text.push_str(&more);
        }
        (_, part) => parts.push(part),
    }
}

/// Trims blank lines around a part; empty parts go.
fn tidy(part: Part) -> Option<Part> {
    let trim = |t: String| t.trim_matches('\n').trim_end().to_string();
    let part = match part {
        Part::Text { text } => Part::Text { text: trim(text) },
        Part::Signature { text } => Part::Signature { text: trim(text) },
        Part::Quote { depth, text } => Part::Quote { depth, text: trim(text) },
        other => other,
    };
    let empty = match &part {
        Part::Text { text } | Part::Signature { text } | Part::Quote { text, .. } | Part::Attribution { text } => text.trim().is_empty(),
        Part::Headers { fields } => fields.is_empty(),
    };
    (!empty).then_some(part)
}

/// "> > text" → (2, "text").
fn quote_depth(line: &str) -> (u8, &str) {
    let mut depth = 0u8;
    let mut rest = line;
    loop {
        let trimmed = rest.trim_start();
        match trimmed.strip_prefix('>') {
            Some(after) => {
                depth = depth.saturating_add(1);
                rest = after;
            }
            None => break,
        }
    }
    (depth, if depth > 0 { rest.strip_prefix(' ').unwrap_or(rest) } else { line })
}

/// Outlook's line of underscores before "De :".
fn is_separator(lower: &str) -> bool {
    lower.len() >= 10 && lower.chars().all(|c| c == '_' || c == '-')
}

/// "Name: value", with a header name, possibly with a space before the colon.
fn header_field(q: &crate::words::Quotes, line: &str) -> Option<(String, String)> {
    let (name, value) = line.split_once(':')?;
    let key = crate::words::folded(name.trim().trim_start_matches(['*', '>', ' ']));
    any_folded(&q.header_names, |n| n == key).then(|| (name.trim().trim_matches('*').trim().to_string(), value.trim().to_string()))
}

/// A block of repeated headers starting at `i`: at least a sender and a subject
/// or a date among consecutive "Name: value" lines. Returns them and the line after.
fn header_block_at(q: &crate::words::Quotes, lines: &[&str], i: usize) -> Option<(Vec<(String, String)>, usize)> {
    let first = header_field(q, lines.get(i)?)?;
    let first_name = crate::words::folded(&first.0);
    if !any_folded(&q.from_names, |n| n == first_name) {
        return None;
    }
    let mut fields = vec![first];
    let mut j = i + 1;
    while let Some(field) = lines.get(j).and_then(|l| header_field(q, l)) {
        fields.push(field);
        j += 1;
    }
    let names: Vec<String> = fields.iter().map(|(n, _)| crate::words::folded(n)).collect();
    let enough = names.iter().any(|name| any_folded(&q.enough_names, |n| n == name));
    (fields.len() >= 2 && enough).then_some((fields, j))
}

/// "On Thu, 1 Oct 2026, Jean wrote:" or "Le jeu. 1 oct. 2026, Jean a écrit :",
/// right before a quote.
fn is_attribution(q: &crate::words::Quotes, lines: &[&str], i: usize) -> bool {
    let line = crate::words::folded(lines[i]);
    let ends = any_folded(&q.wrote, |e| line.ends_with(e));
    let next = lines[i + 1..].iter().find(|l| !l.trim().is_empty());
    ends && next.is_some_and(|l| quote_depth(l).0 > 0)
}

/// Gmail cuts long attributions in two: "Le jeu. 1 oct. 2026 à 12:03, Jean <" and
/// "jean@example.org> a écrit :". The first half is taken back from the text.
fn take_previous_line_into_attribution(q: &crate::words::Quotes, parts: &mut Vec<Part>, line: &str) {
    let mut attribution = line.trim().to_string();
    if let Some(Part::Text { text } | Part::Signature { text }) = parts.last_mut() {
        let trimmed = text.trim_end_matches('\n');
        let (before, last) = match trimmed.rsplit_once('\n') {
            Some((before, last)) => (before.to_string(), last.to_string()),
            None => (String::new(), trimmed.to_string()),
        };
        let starts = q.attribution_starts.iter().map(|s| s.trim()).any(|s| !s.is_empty() && last.starts_with(&format!("{s} ")));
        if starts && !last.trim_end().ends_with('.') {
            attribution = format!("{} {attribution}", last.trim());
            *text = before;
        }
    }
    parts.push(Part::Attribution { text: attribution });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(parts: &[Part]) -> Vec<String> {
        parts
            .iter()
            .map(|p| match p {
                Part::Text { .. } => "text".to_string(),
                Part::Attribution { .. } => "attribution".to_string(),
                Part::Quote { depth, .. } => format!("quote{depth}"),
                Part::Headers { .. } => "headers".to_string(),
                Part::Signature { .. } => "signature".to_string(),
            })
            .collect()
    }

    #[test]
    fn a_reply_with_quotes_and_a_signature() {
        let text = "Merci, c'est noté.\n\n-- \nJean\n\nLe jeu. 1 oct. 2026 à 12:03, Marie Exemple <\nmarie@example.org> a écrit :\n> Bonjour,\n> Voici le document.\n>> Plus ancien.\n";
        let reply = parts(text);
        assert_eq!(kinds(&reply), ["text", "signature", "attribution", "quote1", "quote2"]);
        assert_eq!(reply[0], Part::Text { text: "Merci, c'est noté.".into() });
        assert_eq!(reply[1], Part::Signature { text: "Jean".into() });
        assert_eq!(reply[2], Part::Attribution { text: "Le jeu. 1 oct. 2026 à 12:03, Marie Exemple < marie@example.org> a écrit :".into() });
        assert_eq!(reply[3], Part::Quote { depth: 1, text: "Bonjour,\nVoici le document.".into() });
    }

    #[test]
    fn outlook_answers_and_forwards() {
        let outlook = "Bonjour,\nCi-joint.\n\n________________________________\nDe : Marie Exemple <marie@example.org>\nEnvoyé : jeudi 1 octobre 2026 12:03\nÀ : Jean <jean@example.org>\nObjet : RE: Dossier\n\nBonjour Jean,\nLe dossier est complet.\n";
        let answer = parts(outlook);
        assert_eq!(kinds(&answer), ["text", "headers", "quote1"]);
        let Part::Headers { fields } = &answer[1] else { panic!() };
        assert_eq!(fields[0], ("De".to_string(), "Marie Exemple <marie@example.org>".to_string()));
        assert_eq!(fields.len(), 4);
        let forward = "Pour info.\n\n---------- Forwarded message ---------\nFrom: Shop <shop@example.org>\nDate: Thu, 1 Oct 2026\nSubject: Order\nTo: <jean@example.org>\n\nYour order is confirmed.\n";
        assert_eq!(kinds(&parts(forward)), ["text", "headers", "quote1"]);
        // A German mail program's forward, once its words are added.
        let german = "Siehe unten.\n\n---------- Weitergeleitete Nachricht ---------\nVon: Shop <shop@example.org>\nDatum: Do., 1. Okt. 2026\nBetreff: Bestellung\nAn: <jean@example.org>\n\nIhre Bestellung ist bestätigt.\n";
        let builtin = crate::words::Words::builtin();
        assert_eq!(kinds(&parts_with(&builtin.quotes, german)), ["text"]);
        let mut q = builtin.quotes.clone();
        q.openings.push("---------- Weitergeleitete Nachricht ---------".into());
        q.header_names.extend(["Von", "Datum", "Betreff", "An"].map(String::from));
        q.from_names.push("Von".into());
        q.enough_names.extend(["Betreff", "Datum"].map(String::from));
        assert_eq!(kinds(&parts_with(&q, german)), ["text", "headers", "quote1"]);
    }

    #[test]
    fn html_is_made_safe_and_its_history_folded() {
        let html = "<html><head><style>p{color:red}</style><title>x</title></head><body>\
            <table><tr><td><h1>Hello</h1><p style=\"color:red\">Read <a href=\"https://shop.example/x?t=1\" onclick=\"evil()\">this</a>.</p>\
            <img src=\"https://tracker.example/pixel.gif\"><script>alert(1)</script><form><input></form></td></tr></table>\
            <div class=\"gmail_quote\">On Thu, Jane wrote:<blockquote>Old text</blockquote></div></body></html>";
        let safe = safe_html(html);
        assert!(safe.images_hidden);
        for gone in ["<img", "tracker", "<script", "alert", "style", "onclick", "<form", "<input", "<table", "<td", "<h1", "color"] {
            assert!(!safe.main.contains(gone), "{gone} in {}", safe.main);
        }
        assert!(safe.main.contains("<p><b>Hello</b></p>"), "{}", safe.main);
        assert!(safe.main.contains("<a href=\"https://shop.example/x?t=1\">this</a>"), "{}", safe.main);
        let quoted = safe.quoted.unwrap();
        assert!(quoted.contains("Old text") && !safe.main.contains("Old text"));
        assert!(!clean_html("<a href=\"javascript:alert(1)\">x</a>").contains("javascript"));
        // No link to a file or a share of this computer, no attribute but the address.
        for relative in ["../../Downloads/run.desktop", "//host.example/share/x", "x.sh"] {
            assert!(!clean_html(&format!("<a href=\"{relative}\">x</a>")).contains("href"), "{relative}");
        }
        assert_eq!(clean_html("<table title=\"t\" lang=\"fr\"><tr><td>x</td></tr></table>"), "<div><div><div>x</div></div></div>");
    }

    #[test]
    fn plain_links_show_their_site() {
        assert_eq!(
            linkify("See https://www.example.org/very/long?tracking=1, then <reply>.\nThanks"),
            "See <a href=\"https://www.example.org/very/long?tracking=1\">www.example.org/…</a>, then &lt;reply&gt;.<br>Thanks"
        );
        assert_eq!(linkify("https://example.org/"), "<a href=\"https://example.org/\">example.org</a>");
    }

    #[test]
    fn plain_text_stays_text() {
        assert_eq!(kinds(&parts("Hello.\nDe toute façon: rien.\nSee you.")), ["text"]);
        assert_eq!(file_name(Some("../../etc/passwd"), 0), "passwd");
        assert_eq!(file_name(Some(".hidden"), 2), "hidden");
        assert_eq!(file_name(None, 2), "attachment-3");
        // A name every system can hold, its extension kept.
        assert_eq!(file_name(Some("report.pdf:stream"), 0), "report.pdf_stream");
        assert_eq!(file_name(Some("a<b>?\"*|.pdf"), 0), "a_b_____.pdf");
        assert_eq!(file_name(Some("nul.txt"), 0), "_nul.txt");
        assert_eq!(file_name(Some("COM1"), 0), "_COM1");
        assert_eq!(file_name(Some("scan.pdf. . "), 0), "scan.pdf");
        assert_eq!(file_name(Some("Console.pdf"), 0), "Console.pdf");
    }
}
