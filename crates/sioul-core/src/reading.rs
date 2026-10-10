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
        .clean(&shallow(html))
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
    without_spacers(&out).trim().to_string()
}

/// How deep elements may stand in one another in a message's HTML before the
/// deeper ones are left out, their words kept (`shallow`). Mail nests a few
/// dozen levels; web browsers stop at 512 too.
const DEEPEST: usize = 512;

/// Elements that hold nothing: never left open.
const VOID: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "image", "img", "input", "keygen", "link", "meta", "param",
    "source", "track", "wbr",
];

/// The start tags before which the parser closes an open paragraph.
const CLOSES_P: &[&str] = &[
    "address", "article", "aside", "blockquote", "center", "details", "dialog", "dir", "div", "dl", "fieldset", "figcaption", "figure",
    "footer", "header", "hgroup", "main", "menu", "nav", "ol", "p", "search", "section", "summary", "ul", "h1", "h2", "h3", "h4", "h5",
    "h6", "pre", "listing", "xmp", "plaintext", "li", "dd", "dt", "hr",
];

const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

/// The end tags that close what the parser closes for them on the way, and
/// what, from the innermost out: a paragraph before a block's end, a list
/// item before its list's, a cell and a row before their table's. Any other
/// end tag closes the innermost element only, when it is the one it names.
fn closed_on_the_way(name: &str) -> &'static [&'static [&'static str]] {
    match name {
        "ul" | "ol" => &[&["p"], &["li"]],
        "dl" => &[&["p"], &["dd", "dt"]],
        "table" => &[&["p"], &["td", "th"], &["tr"], &["tbody", "thead", "tfoot"]],
        "tbody" | "thead" | "tfoot" => &[&["p"], &["td", "th"], &["tr"]],
        "tr" => &[&["p"], &["td", "th"]],
        "address" | "article" | "aside" | "blockquote" | "center" | "details" | "dialog" | "dir" | "div" | "fieldset" | "figcaption"
        | "figure" | "footer" | "header" | "hgroup" | "listing" | "main" | "menu" | "nav" | "pre" | "search" | "section" | "summary"
        | "li" | "dd" | "dt" | "td" | "th" | "caption" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => &[&["p"]],
        _ => &[],
    }
}

/// What a start tag closes first when it is innermost, from the innermost
/// out: the next list item closes the last, the next cell the last.
fn closed_before(name: &str) -> &'static [&'static [&'static str]] {
    match name {
        "li" => &[&["li"]],
        "dd" | "dt" => &[&["dd", "dt"]],
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => &[HEADINGS],
        "option" | "optgroup" => &[&["option"]],
        "td" | "th" => &[&["td", "th"]],
        "tr" => &[&["td", "th"], &["tr"]],
        "tbody" | "thead" | "tfoot" => &[&["td", "th"], &["tr"], &["tbody", "thead", "tfoot"]],
        _ => &[],
    }
}

/// The HTML with no element deeper than `DEEPEST`, in one pass. The parser
/// under ammonia looks through the elements still open at most tags, so
/// elements nested by the thousand cost it the square of their number
/// (32,000 empty blocks in one another: 3 seconds; a megabyte of them,
/// minutes). This follows the elements the parser keeps open without ever
/// counting one closed before the parser closes it: an element closes on
/// its own end tag when it is the innermost, or when the parser surely
/// closes it then (`closed_on_the_way`, `closed_before`). Sloppy mail at
/// worst seems deeper than it is; the parser, which also opens elements of
/// its own (a table's body and row), never holds more than a few times what
/// is counted. A start tag past the depth is left out, a space in
/// its place, so that no word runs into the next and no "<" before it starts
/// a tag; its words stay.
fn shallow(html: &str) -> std::borrow::Cow<'_, str> {
    let mut open: Vec<String> = Vec::new();
    // Only once a tag is left out: what came before it, copied.
    let mut out: Option<String> = None;
    let mut copied = 0;
    let mut from = 0;
    while let Some(lt) = html[from..].find('<').map(|at| from + at) {
        let after = &html.as_bytes()[lt + 1..];
        let (end, name_at) = match after {
            [b'/', c, ..] if c.is_ascii_alphabetic() => (true, lt + 2),
            [c, ..] if c.is_ascii_alphabetic() => (false, lt + 1),
            _ => {
                from = lt + 1;
                continue;
            }
        };
        // A tag never ended is words, for the parser too.
        let Some(gt) = html[name_at..].find('>').map(|at| name_at + at) else { break };
        from = gt + 1;
        let name_end = html[name_at..gt].find(|c: char| c.is_ascii_whitespace() || c == '/').map_or(gt, |at| name_at + at);
        let name = html[name_at..name_end].to_ascii_lowercase();
        // The parser keeps one of each, whatever is written.
        if matches!(name.as_str(), "html" | "head" | "body") {
            continue;
        }
        let mut keep = open.len();
        let close = |sets: &[&[&str]], keep: &mut usize| {
            for set in sets {
                if *keep > 0 && set.contains(&open[*keep - 1].as_str()) {
                    *keep -= 1;
                }
            }
        };
        if end {
            close(closed_on_the_way(&name), &mut keep);
            if keep > 0 && open[keep - 1] == name {
                open.truncate(keep - 1);
            }
            continue;
        }
        if CLOSES_P.contains(&name.as_str()) {
            close(&[&["p"]], &mut keep);
        }
        close(closed_before(&name), &mut keep);
        if VOID.contains(&name.as_str()) {
            open.truncate(keep);
        } else if keep < DEEPEST {
            open.truncate(keep);
            open.push(name);
        } else {
            let out = out.get_or_insert_with(|| String::with_capacity(html.len()));
            out.push_str(&html[copied..lt]);
            out.push(' ');
            copied = gt + 1;
        }
    }
    match out {
        None => std::borrow::Cow::Borrowed(html),
        Some(mut out) => {
            out.push_str(&html[copied..]);
            std::borrow::Cow::Owned(out)
        }
    }
}

/// Spacer blocks and runs of line breaks, which layouts use for room, taken
/// out in one pass: a block is dropped as its end is written, when nothing
/// but a space stands in it, so a block left empty by the spacers it held goes
/// too (`<div><p></p></div>`), and a third line break in a row goes. Done by
/// replacing them over the whole text until none was left, the work grew with
/// the square of the nesting: 8,000 empty blocks in one another took seconds.
fn without_spacers(html: &str) -> String {
    const EMPTY: [&str; 6] = ["<div></div>", "<div>&nbsp;</div>", "<div> </div>", "<p></p>", "<p>&nbsp;</p>", "<span></span>"];
    let mut out = String::with_capacity(html.len());
    // Each piece ends with a ">", the only place a spacer can end.
    for piece in html.split_inclusive('>') {
        out.push_str(piece);
        if let Some(empty) = EMPTY.iter().find(|empty| out.ends_with(*empty)) {
            out.truncate(out.len() - empty.len());
        } else if out.ends_with("<br><br><br>") {
            out.truncate(out.len() - "<br>".len());
        }
    }
    out
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
        while let Some(at) = next_link(rest) {
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

/// Where the next web address starts: "http://" or "https://". One search
/// for both: searched apart, a line of a thousand "https://" links had its
/// whole rest read again for an "http://" before each of them.
fn next_link(text: &str) -> Option<usize> {
    text.match_indices("http").map(|(at, _)| at).find(|&at| {
        let after = &text[at + "http".len()..];
        after.starts_with("://") || after.starts_with("s://")
    })
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

/// The words of `words::Quotes`, folded once for a whole text, not once per line.
struct Folded {
    openings: Vec<String>,
    header_names: Vec<String>,
    from_names: Vec<String>,
    enough_names: Vec<String>,
    wrote: Vec<String>,
}

impl Folded {
    fn of(q: &crate::words::Quotes) -> Folded {
        let fold = |list: &[String]| list.iter().map(|w| crate::words::folded(w)).filter(|w| !w.is_empty()).collect();
        Folded { openings: fold(&q.openings), header_names: fold(&q.header_names), from_names: fold(&q.from_names), enough_names: fold(&q.enough_names), wrote: fold(&q.wrote) }
    }
}

/// What `parts_with` looks ahead for, read once from the end of the text: each
/// line's header field, where each run of header fields ends and whether it
/// names a subject or a date, and the next line that is not blank. Looked for
/// again from every line, they made the work grow with the square of the
/// lines: 8,000 lines of "From: x" took half a minute.
struct Ahead {
    fields: Vec<Option<(String, String)>>,
    run_end: Vec<usize>,
    enough: Vec<bool>,
    next_filled: Vec<Option<usize>>,
}

impl Ahead {
    fn of(words: &Folded, lines: &[&str]) -> Ahead {
        let n = lines.len();
        let fields: Vec<Option<(String, String)>> = lines.iter().map(|line| header_field(words, line)).collect();
        let (mut run_end, mut enough, mut next_filled) = (vec![n; n + 1], vec![false; n + 1], vec![None; n + 1]);
        let mut filled = None;
        for i in (0..n).rev() {
            if let Some((name, _)) = &fields[i] {
                run_end[i] = run_end[i + 1];
                enough[i] = enough[i + 1] || words.enough_names.contains(&crate::words::folded(name));
            } else {
                run_end[i] = i;
            }
            next_filled[i] = filled;
            if !lines[i].trim().is_empty() {
                filled = Some(i);
            }
        }
        Ahead { fields, run_end, enough, next_filled }
    }

    /// A block of repeated headers starting at `i`: at least a sender and a
    /// subject or a date among consecutive "Name: value" lines. Returns them
    /// and the line after.
    fn header_block_at(&self, words: &Folded, i: usize) -> Option<(Vec<(String, String)>, usize)> {
        let (name, _) = self.fields.get(i)?.as_ref()?;
        let end = self.run_end[i];
        (words.from_names.contains(&crate::words::folded(name)) && end - i >= 2 && self.enough[i]).then(|| (self.fields[i..end].iter().flatten().cloned().collect(), end))
    }
}

/// Cuts a message's text into parts with these words (`words::Quotes`): the
/// lines that open a forwarded or answered message, the header names of a
/// repeated header block, the endings of an attribution, in any language
/// your correspondents' mail programs write them. Each line is read a set
/// number of times, whatever comes after it (`Ahead`).
pub fn parts_with(q: &crate::words::Quotes, text: &str) -> Vec<Part> {
    let words = Folded::of(q);
    let lines: Vec<&str> = text.lines().collect();
    let ahead = Ahead::of(&words, &lines);
    let mut parts: Vec<Part> = Vec::new();
    let mut i = 0;
    // After the headers of a message answered or forwarded, the rest is that message.
    let mut quoted_below = 0u8;
    while i < lines.len() {
        let line = lines[i];
        let lower = line.trim().to_lowercase();
        let folded = crate::words::folded(line);
        if words.openings.iter().any(|o| folded.starts_with(o.as_str())) || is_separator(&lower) && ahead.header_block_at(&words, i + 1).is_some() {
            i += 1;
            continue;
        }
        if let Some((fields, next)) = ahead.header_block_at(&words, i) {
            parts.push(Part::Headers { fields });
            quoted_below = quoted_below.saturating_add(1);
            i = next;
            continue;
        }
        if is_attribution(&words, &folded, ahead.next_filled[i].map(|j| lines[j])) {
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
fn header_field(words: &Folded, line: &str) -> Option<(String, String)> {
    let (name, value) = line.split_once(':')?;
    let key = crate::words::folded(name.trim().trim_start_matches(['*', '>', ' ']));
    words.header_names.contains(&key).then(|| (name.trim().trim_matches('*').trim().to_string(), value.trim().to_string()))
}

/// "On Thu, 1 Oct 2026, Jean wrote:" or "Le jeu. 1 oct. 2026, Jean a écrit :"
/// (`folded`), right before a quote (`next`, the next line that is not blank).
fn is_attribution(words: &Folded, folded: &str, next: Option<&str>) -> bool {
    words.wrote.iter().any(|e| folded.ends_with(e.as_str())) && next.is_some_and(|l| quote_depth(l).0 > 0)
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

    #[test]
    fn spacers_go_in_one_pass() {
        assert_eq!(without_spacers("<div><p></p><span></span> </div>x"), "x");
        assert_eq!(without_spacers("<div><div>&nbsp;</div></div><p>&nbsp;</p>"), "");
        assert_eq!(without_spacers("a<br><br><br><br><br>b"), "a<br><br>b");
        assert_eq!(without_spacers("<br><div></div><br><br>"), "<br><br>");
        assert_eq!(without_spacers("<div>x</div><div>  </div><p>y</p>"), "<div>x</div><div>  </div><p>y</p>");
        // A spacer split by the end of a run of breaks, and one with words around it.
        assert_eq!(without_spacers("<div><br><br><br></div>"), "<div><br><br></div>");
        assert_eq!(without_spacers("x<span></span>y"), "xy");
    }

    /// How long a pass may take over one of the texts below (64 to 176 KB, made
    /// to cost the most): linear, it takes milliseconds; quadratic, as it was,
    /// from 10 seconds to a minute. Wide, for a busy machine, and wider for a
    /// debug build.
    fn bound() -> std::time::Duration {
        std::time::Duration::from_secs(if cfg!(debug_assertions) { 20 } else { 2 })
    }

    fn timed<T>(what: &str, f: impl FnOnce() -> T) -> T {
        let start = std::time::Instant::now();
        let out = f();
        let took = start.elapsed();
        eprintln!("{what}: {took:?}");
        assert!(took < bound(), "{what}: {took:?}, more than {:?}", bound());
        out
    }

    #[test]
    fn hostile_html_reads_in_linear_time() {
        // Empty blocks nested in one another: each pass of the old spacer loop took one level off.
        let n = 16_000;
        let html = format!("<html><body>{}{}</body></html>", "<div>".repeat(n), "</div>".repeat(n));
        let safe = timed("nested empty blocks", || safe_html(&html));
        assert!(safe.main.matches("<div>").count() <= DEEPEST, "{}", safe.main.len());
        // The same through the reader's entry point, as a message.
        let raw = format!("From: a@example.org\r\nTo: b@example.org\r\nSubject: hi\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n");
        let reading = timed("a message of nested empty blocks", || read_bytes(raw.as_bytes())).unwrap();
        assert!(reading.html.is_some_and(|h| h.main.matches("<div>").count() <= DEEPEST));
        // Ways to keep elements open in the parser that a count of start and
        // end tags would think closed: end tags it ignores, cells outside a
        // table, items in one another, formatting it opens again.
        let n = 40_000;
        for (what, hostile) in [
            ("ignored end tags", "<div></span>".repeat(n)),
            ("blocks around an ignored end", format!("{}<object>{}", "<div>".repeat(400), "</div>".repeat(400)).repeat(n / 800)),
            ("cells outside a table", "<td><li><dd></td>".repeat(n)),
            ("a list closed by a list it is not in", "<select><ul></select><li><dd></ul>".repeat(n / 4)),
            ("formatting", "<b id=x1><i id=x2><p></b>x".repeat(n / 2)),
            ("paragraphs in buttons", "<button><p>".repeat(n)),
            ("self-closed blocks", "<div/>".repeat(n)),
            ("end tags with nothing to close, deep", format!("{}{}", "<div>".repeat(600), "</p>".repeat(n))),
        ] {
            let safe = timed(what, || safe_html(&hostile));
            assert!(safe.main.len() <= hostile.len(), "{what}");
        }
    }

    #[test]
    fn deep_html_is_cut_and_mail_is_not() {
        // Mail as mail programs write it, sloppy ones too: every element kept.
        let sloppy = "<ul><li>a<li>b</ul><p>c<p>d<div><p>e</div><table><tr><td>f<td>g<tr><td>h</table><dl><dt>i<dd>j</dl><p>k<hr>l<br>".repeat(2_000);
        assert!(matches!(shallow(&sloppy), std::borrow::Cow::Borrowed(_)));
        let nested = format!("{}x{}", "<table><tbody><tr><td><div><p>".repeat(60), "</p></div></td></tr></tbody></table>".repeat(60));
        assert!(matches!(shallow(&nested), std::borrow::Cow::Borrowed(_)));
        // Deeper than `DEEPEST`, start tags go, their words and the end tags stay.
        let deep = format!("{}x{}", "<div class=\"a\">".repeat(1_000), "</div>".repeat(1_000));
        let cut = shallow(&deep);
        assert_eq!(cut.matches("<div").count(), DEEPEST);
        assert!(cut.contains(" x</div>") && cut.matches("</div>").count() == 1_000);
        // A "<" left before a tag taken out never starts another.
        let tricky = format!("{}<<div>img src=x>", "<div>".repeat(DEEPEST));
        assert!(shallow(&tricky).ends_with("< img src=x>"));
    }

    #[test]
    fn hostile_plain_text_reads_in_linear_time() {
        // Lines that each start a header block: the old scan read every line after each.
        let headers = "From: x\n".repeat(8_000);
        let read = timed("header lines", || parts(&headers));
        assert!(!read.is_empty());
        // One word, then blank lines: each looked for the next line that is not blank.
        let blanks = format!("x{}", "\n".repeat(160_000));
        assert_eq!(timed("blank lines", || parts(&blanks)), [Part::Text { text: "x".into() }]);
        // Attributions, each before many blank lines and a quote.
        let attributions = format!("{}\n> quoted", "Jean wrote:\n\n\n\n\n\n\n\n".repeat(8_000));
        assert!(timed("attributions", || parts(&attributions)).iter().any(|p| matches!(p, Part::Quote { .. })));
        // A real header block among them is still one.
        let block = format!("{headers}Subject: y\n\nText.\n");
        assert!(timed("header lines and a subject", || parts(&block)).iter().any(|p| matches!(p, Part::Headers { fields } if fields.len() == 8_001)));
        // A line of links: each looked for "http://" over the rest of the line.
        let links = "https://example.org/a ".repeat(40_000);
        assert_eq!(timed("a line of links", || linkify(&links)).matches("<a href=").count(), 40_000);
    }
}
