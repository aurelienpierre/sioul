// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Writing: Markdown in, mail out.
//!
//! You write Markdown. The message goes out as `multipart/alternative`: the
//! Markdown itself as plain text, readable as it is, and its HTML (CommonMark
//! with tables and strikethrough). A reply or a forward appends the message it
//! answers below your text, after your text was converted: in HTML, the
//! original made safe (`reading::safe_html`) in a blockquote; in plain text,
//! its lines quoted with "> ". `In-Reply-To` and `References` keep the thread
//! together in every client (RFC 5322 §3.6.4).
//!
//! A draft is a small TOML file, saved as you type, so closing the window
//! never loses anything (docs/client.md, "Calm first").

use crate::config::data_dir;
use crate::i18n::{self, Translator};
use crate::reading;
use mail_builder::MessageBuilder;
use mail_parser::{Address, MessageParser};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DraftKind {
    #[default]
    New,
    Reply,
    ReplyAll,
    Forward,
}

/// A message being written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    /// The draft's own name, to save it in place.
    #[serde(default)]
    pub id: String,
    /// The account it goes out from.
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    /// What you write, in Markdown.
    #[serde(default)]
    pub body: String,
    /// Files to attach.
    #[serde(default)]
    pub attachments: Vec<PathBuf>,
    #[serde(default)]
    pub kind: DraftKind,
    /// The message answered or forwarded, quoted below the text when sent.
    #[serde(default)]
    pub original: Option<PathBuf>,
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    /// For a forward: the original's attachments you left out, by index.
    #[serde(default)]
    pub dropped: Vec<u32>,
    /// Signed with your OpenPGP key.
    #[serde(default)]
    pub sign: bool,
    /// Encrypted to every recipient's key.
    #[serde(default)]
    pub encrypt: bool,
    /// What it belongs with: a task, an event, a note (`links`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
}

/// The line above a signature (RFC 3676 §4.3): readers recognise and fold what follows.
pub const SIGNATURE_LINE: &str = "-- ";

impl Draft {
    /// A new draft with its own name.
    pub fn new(account: &str) -> Draft {
        Draft { id: new_id(), account: account.to_string(), ..Draft::default() }
    }

    /// The folder drafts are kept in.
    pub fn folder() -> PathBuf {
        data_dir().join("drafts")
    }

    pub fn path(&self) -> PathBuf {
        Draft::folder().join(format!("{}.toml", self.id))
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let path = self.path();
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        std::fs::create_dir_all(Draft::folder()).map_err(fail)?;
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, text).map_err(fail)?;
        std::fs::rename(&temporary, &path).map_err(fail)?;
        Ok(path)
    }

    pub fn load(path: &Path) -> Option<Draft> {
        toml::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    /// Every saved draft, the newest first.
    pub fn all() -> Vec<Draft> {
        let Ok(entries) = std::fs::read_dir(Draft::folder()) else { return Vec::new() };
        let mut found: Vec<(std::time::SystemTime, Draft)> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .filter_map(|p| Some((std::fs::metadata(&p).and_then(|m| m.modified()).ok()?, Draft::load(&p)?)))
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0));
        found.into_iter().map(|(_, d)| d).collect()
    }

    /// Removes the draft once sent, or when you discard it, with its copy of
    /// the original and the files Sioul copied for it from another
    /// application (`handed`): yours, where they came from, stay.
    pub fn discard(&self) {
        let _ = std::fs::remove_file(self.path());
        let _ = std::fs::remove_file(self.original_copy());
        for path in &self.attachments {
            remove_own_copy(path);
        }
    }

    fn original_copy(&self) -> PathBuf {
        Draft::folder().join(format!("{}.eml", self.id))
    }

    /// Keeps a copy of the message answered or forwarded next to the draft: it
    /// may be archived, deleted or renamed before the answer leaves.
    pub fn keep_original(&mut self) -> Result<(), String> {
        let Some(original) = self.original.clone() else { return Ok(()) };
        let copy = self.original_copy();
        if original == copy {
            return Ok(());
        }
        std::fs::create_dir_all(Draft::folder()).map_err(|e| e.to_string())?;
        std::fs::copy(&original, &copy).map_err(|e| format!("{}: {e}", original.display()))?;
        self.original = Some(copy);
        Ok(())
    }

    /// A saved draft by its name; none for a name that is not one of Sioul's.
    pub fn by_id(id: &str) -> Option<Draft> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        Draft::load(&Draft::folder().join(format!("{id}.toml")))
    }

    /// Every recipient, for the envelope: To, Cc and Bcc.
    pub fn recipients(&self) -> Vec<String> {
        self.to.iter().chain(&self.cc).chain(&self.bcc).filter_map(|r| address_of(r)).collect()
    }

    /// Puts the account's signature below the text, once, after the usual "-- " line.
    pub fn add_signature(&mut self, signature: Option<&str>) {
        let Some(signature) = signature.map(str::trim).filter(|s| !s.is_empty()) else { return };
        if !self.body.contains(signature) {
            self.body = format!("{}\n\n{SIGNATURE_LINE}\n{signature}", self.body.trim_end());
        }
    }

    /// Whether anything was written: an untouched draft is not worth keeping.
    pub fn is_empty(&self, signature: Option<&str>) -> bool {
        let body = self.body.replace(signature.unwrap_or("\u{0}").trim(), "").replace(SIGNATURE_LINE.trim_end(), "");
        body.trim().is_empty() && self.attachments.is_empty() && (self.kind != DraftKind::New || (self.to.is_empty() && self.subject.trim().is_empty()))
    }

    /// The original's attachments a forward carries, by index and name.
    pub fn forwarded(&self) -> Vec<(u32, String)> {
        let Some(original) = self.original.as_deref().filter(|_| self.kind == DraftKind::Forward) else { return Vec::new() };
        let Some(reading) = reading::read(original) else { return Vec::new() };
        reading.attachments.into_iter().filter(|a| !self.dropped.contains(&a.index)).map(|a| (a.index, a.name)).collect()
    }
}

/// A new message's own name, the left part of its Message-ID: unique to this moment and this program.
pub fn new_message_id() -> String {
    new_id()
}

/// A name unique enough for a draft or a Message-ID: the time, and a little of the process.
fn new_id() -> String {
    use std::hash::{Hash, Hasher};
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    now.hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    format!("{:x}{:08x}", now.as_secs(), hasher.finish() as u32)
}

/// Whether `path` is one of the files Sioul copied for a draft from another
/// application (`handed`), which go with their draft.
pub fn is_own_copy(path: &Path) -> bool {
    own_copy_in(&crate::handed::Incoming::here().files(), path)
}

fn own_copy_in(copies: &Path, path: &Path) -> bool {
    use std::path::Component;
    path.starts_with(copies) && path.components().all(|c| !matches!(c, Component::ParentDir | Component::CurDir))
}

/// One of those copies removed, with its request's folder once empty;
/// nothing for any other file.
pub fn remove_own_copy(path: &Path) {
    remove_own_copy_in(&crate::handed::Incoming::here().files(), path);
}

fn remove_own_copy_in(copies: &Path, path: &Path) {
    if !own_copy_in(copies, path) {
        return;
    }
    let _ = std::fs::remove_file(path);
    if let Some(folder) = path.parent().filter(|folder| *folder != copies) {
        // Only when empty: another of its files may still be attached.
        let _ = std::fs::remove_dir(folder);
    }
}

/// "Jane <jane@example.org>" or "jane@example.org" → "jane@example.org".
pub fn address_of(entry: &str) -> Option<String> {
    let entry = entry.trim();
    let address = match (entry.rfind('<'), entry.rfind('>')) {
        (Some(open), Some(close)) if open < close => &entry[open + 1..close],
        _ => entry,
    };
    let address = address.trim();
    let (local, domain) = address.split_once('@')?;
    (!local.is_empty() && domain.contains('.') && !address.contains(char::is_whitespace)).then(|| address.to_string())
}

/// Splits what was typed in an address field: commas, semicolons and line
/// breaks separate, but not inside a quoted name or between "<" and ">"
/// ("\"Doe, Jane\" <jane@example.org>" is one recipient). A line whose quote
/// or "<" is left open splits at every comma and semicolon, so no one is lost.
pub fn split_addresses(field: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in field.split('\n') {
        let (mut quoted, mut escaped, mut angle) = (false, false, false);
        let mut pieces = vec![String::new()];
        for c in line.chars() {
            match c {
                _ if escaped => escaped = false,
                '\\' if quoted => escaped = true,
                '"' if !angle => quoted = !quoted,
                '<' if !quoted => angle = true,
                '>' if !quoted => angle = false,
                ',' | ';' if !quoted && !angle => {
                    pieces.push(String::new());
                    continue;
                }
                _ => {}
            }
            pieces.last_mut().into_iter().for_each(|p| p.push(c));
        }
        if quoted || angle {
            pieces = line.split([',', ';']).map(str::to_string).collect();
        }
        out.extend(pieces.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).map(str::to_string));
    }
    out
}

/// A reply, a reply to all or a forward, from the message at `original`:
/// recipients, subject and thread prefilled, the text left to you.
/// `own` holds your addresses, never answered to.
pub fn answer(original: &Path, kind: DraftKind, account: &str, own: &[String]) -> Option<Draft> {
    let raw = std::fs::read(original).ok()?;
    let message = MessageParser::default().parse(&raw)?;
    let mine = |address: &str| own.iter().any(|o| o.eq_ignore_ascii_case(address));
    let entries = |list: Option<&Address>| -> Vec<String> {
        list.map(|a| a.iter().filter_map(|p| p.address().map(|addr| entry(p.name(), addr))).collect()).unwrap_or_default()
    };
    let reply_to = entries(message.reply_to());
    let from = entries(message.from());
    // Your own message (in Sent): the answer goes where it went.
    let yours = !from.is_empty() && from.iter().all(|e| address_of(e).is_some_and(|a| mine(&a)));
    let to_answer = if yours {
        entries(message.to())
    } else if reply_to.is_empty() {
        from
    } else {
        reply_to
    };
    let subject = message.subject().unwrap_or("").to_string();
    let message_id = message.message_id().map(str::to_string);
    let mut references: Vec<String> = message.references().as_text_list().map(|l| l.iter().map(|r| r.to_string()).collect()).unwrap_or_default();
    references.extend(message_id.clone());
    let mut draft = Draft::new(account);
    draft.kind = kind;
    draft.original = Some(original.to_path_buf());
    match kind {
        DraftKind::Forward => {
            draft.subject = prefixed(&subject, "Fwd:", &["fwd:", "fw:", "tr:"]);
        }
        DraftKind::Reply | DraftKind::ReplyAll => {
            draft.subject = prefixed(&subject, "Re:", &["re:", "ré:", "aw:"]);
            draft.in_reply_to = message_id;
            draft.references = references;
            draft.to = to_answer.into_iter().filter(|e| address_of(e).is_none_or(|a| !mine(&a))).collect();
            if kind == DraftKind::ReplyAll {
                let already: Vec<String> = draft.to.iter().filter_map(|e| address_of(e)).collect();
                let mut cc: Vec<String> = Vec::new();
                let others = if yours { Vec::new() } else { entries(message.to()) };
                for e in others.into_iter().chain(entries(message.cc())) {
                    let Some(address) = address_of(&e) else { continue };
                    let taken = mine(&address) || already.iter().chain(cc.iter().filter_map(|c| address_of(c)).collect::<Vec<_>>().iter()).any(|a| a.eq_ignore_ascii_case(&address));
                    if !taken {
                        cc.push(e);
                    }
                }
                draft.cc = cc;
            }
        }
        DraftKind::New => {}
    }
    Some(draft)
}

fn entry(name: Option<&str>, address: &str) -> String {
    match name.filter(|n| !n.trim().is_empty() && *n != address) {
        Some(name) => format!("{} <{address}>", phrase(name.trim())),
        None => address.to_string(),
    }
}

/// A name as an address field writes it (RFC 5322 §3.2.5): as it is when it
/// holds only words, else between quotes, its quotes and backslashes escaped,
/// so that "Doe, Jane" stays one name and never ends the field or the address.
pub fn phrase(name: &str) -> String {
    let special = |c: char| matches!(c, '(' | ')' | '<' | '>' | '[' | ']' | ':' | ';' | '@' | '\\' | ',' | '.' | '"');
    if name.contains(special) { format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")) } else { name.to_string() }
}

/// "Re: Subject", once only: "Re: Re: Re:" helps nobody.
fn prefixed(subject: &str, prefix: &str, known: &[&str]) -> String {
    let lower = subject.trim_start().to_lowercase();
    if known.iter().any(|k| lower.starts_with(k)) { subject.trim().to_string() } else { format!("{prefix} {}", subject.trim()) }
}

/// CommonMark with tables and strikethrough, as HTML. A line break typed in a
/// mail is meant, as in a chat or a GitHub comment: it is kept.
///
/// HTML written in the text (a note quoting a mail, a draft) shows as it was
/// written, never read: only marks of plain formatting (`<b>`, `<br>`) and
/// pictures of this computer or of the text itself stay, as the pictures a
/// vault's notes embed (`Vault::with_links`). A picture from elsewhere
/// ("![](https://…)", a tracking pixel) is a link to it: showing the text
/// fetches nothing.
pub fn markdown_html(markdown: &str) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut events: Vec<Event> = Vec::new();
    // Links open around here, the lines of a block of HTML so far, and a
    // picture from elsewhere being written: its address, whether it has words,
    // whether it is a link of its own (in a link, its words are enough).
    let (mut links, mut lines) = (0usize, 0usize);
    let mut far: Option<(String, bool, bool)> = None;
    for event in Parser::new_ext(markdown, options) {
        if let (Some((_, words, _)), Event::Text(text) | Event::Code(text)) = (far.as_mut(), &event) {
            *words |= !text.trim().is_empty();
        }
        match event {
            Event::SoftBreak => events.push(Event::HardBreak),
            Event::Start(Tag::HtmlBlock) => {
                lines = 0;
                events.push(Event::Start(Tag::Paragraph));
            }
            Event::End(TagEnd::HtmlBlock) => events.push(Event::End(TagEnd::Paragraph)),
            Event::Html(raw) => {
                if lines > 0 {
                    events.push(Event::HardBreak);
                }
                lines += 1;
                events.push(written_html(raw.trim_end_matches('\n')));
            }
            Event::InlineHtml(raw) => events.push(written_html(&raw)),
            Event::Start(Tag::Image { link_type, dest_url, title, id }) if !local_source(&dest_url) => {
                far = Some((dest_url.to_string(), false, links == 0));
                if links == 0 {
                    events.push(Event::Start(Tag::Link { link_type, dest_url, title, id }));
                }
            }
            Event::End(TagEnd::Image) if far.is_some() => {
                if let Some((address, words, link)) = far.take() {
                    if !words {
                        events.push(Event::Text(address.into()));
                    }
                    if link {
                        events.push(Event::End(TagEnd::Link));
                    }
                }
            }
            Event::Start(Tag::Link { .. }) => {
                links += 1;
                events.push(event);
            }
            Event::End(TagEnd::Link) => {
                links = links.saturating_sub(1);
                events.push(event);
            }
            other => events.push(other),
        }
    }
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

/// Marks of plain formatting kept from HTML written in Markdown: they fetch nothing.
const PLAIN_MARKS: &[&str] = &["b", "i", "u", "s", "em", "strong", "del", "sub", "sup", "mark", "kbd", "br"];

/// One piece of HTML written in Markdown: a mark of plain formatting, or a
/// picture of this computer rebuilt from its address, size and words alone;
/// anything else as text, shown as it was written.
fn written_html(raw: &str) -> pulldown_cmark::Event<'static> {
    use pulldown_cmark::{CowStr, Event};
    match plain_mark(raw).or_else(|| local_picture(raw)) {
        Some(kept) => Event::InlineHtml(CowStr::from(kept)),
        None => Event::Text(CowStr::from(raw.to_string())),
    }
}

/// "<b>", "</b>", "<br/>": a mark of `PLAIN_MARKS`, without attributes.
fn plain_mark(raw: &str) -> Option<String> {
    let inner = raw.trim().strip_prefix('<')?.strip_suffix('>')?;
    let (closing, name) = inner.strip_prefix('/').map_or((false, inner), |n| (true, n));
    let name = name.trim_end_matches('/').trim().to_ascii_lowercase();
    PLAIN_MARKS.contains(&name.as_str()).then(|| if closing { format!("</{name}>") } else { format!("<{name}>") })
}

/// "<img src=… width=… height=… alt=…>" whose picture is on this computer or
/// in the text itself (`local_source`); its other attributes are left out.
fn local_picture(raw: &str) -> Option<String> {
    let inner = raw.trim().strip_prefix('<')?.strip_suffix('>')?.trim_end_matches('/');
    let rest = inner.get(..3).filter(|tag| tag.eq_ignore_ascii_case("img")).and_then(|_| inner.get(3..))?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let attributes = tag_attributes(rest)?;
    let value = |name: &str| attributes.iter().find(|(n, _)| n == name).map(|(_, v)| v.trim());
    let src = value("src").filter(|s| local_source(s))?;
    let mut out = format!("<img src=\"{}\"", escape_attribute(src));
    for name in ["width", "height"] {
        if let Some(size) = value(name).filter(|v| (1..=5).contains(&v.len()) && v.chars().all(|c| c.is_ascii_digit())) {
            out.push_str(&format!(" {name}=\"{size}\""));
        }
    }
    if let Some(alt) = value("alt") {
        out.push_str(&format!(" alt=\"{}\"", escape_attribute(alt)));
    }
    out.push('>');
    Some(out)
}

/// A tag's attributes after its name, names lowercase; none when the text is
/// not one tag's attributes (another tag, a stray quote).
fn tag_attributes(text: &str) -> Option<Vec<(String, String)>> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        while chars.get(i).is_some_and(|c| c.is_whitespace()) {
            i += 1;
        }
        if i == chars.len() {
            return Some(out);
        }
        let start = i;
        while chars.get(i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_') {
            i += 1;
        }
        if i == start {
            return None;
        }
        let name = chars[start..i].iter().collect::<String>().to_ascii_lowercase();
        while chars.get(i).is_some_and(|c| c.is_whitespace()) {
            i += 1;
        }
        let mut value = String::new();
        if chars.get(i) == Some(&'=') {
            i += 1;
            while chars.get(i).is_some_and(|c| c.is_whitespace()) {
                i += 1;
            }
            match chars.get(i) {
                Some(&quote) if quote == '"' || quote == '\'' => {
                    let end = i + 1 + chars[i + 1..].iter().position(|&c| c == quote)?;
                    value = chars[i + 1..end].iter().collect();
                    i = end + 1;
                }
                _ => {
                    let start = i;
                    while chars.get(i).is_some_and(|c| !c.is_whitespace() && !"\"'<>=`".contains(*c)) {
                        i += 1;
                    }
                    if i == start {
                        return None;
                    }
                    value = chars[start..i].iter().collect();
                }
            }
        }
        out.push((name, value));
    }
}

/// Whether a picture's address stays on this computer: a path (from the
/// note's folder, or from the root), a `file:` address without a host, or the
/// picture itself (`data:image/…`). Never "//host/…", "\\host\…" nor
/// "file://host/…", which reach a server (on Windows, with your credentials).
fn local_source(src: &str) -> bool {
    let lower = src.trim().to_ascii_lowercase();
    if lower.is_empty() || lower.contains('\\') || lower.contains("%2f") || lower.contains("%5c") {
        return false;
    }
    if let Some(data) = lower.strip_prefix("data:") {
        return data.starts_with("image/");
    }
    if let Some(rest) = lower.strip_prefix("file://") {
        let path = rest.strip_prefix("localhost").unwrap_or(rest);
        return path.starts_with('/') && !path.starts_with("//");
    }
    // An address with a scheme ("https:", "smb:", "c:") is not a path.
    let scheme = lower.split_once(':').is_some_and(|(scheme, _)| !scheme.contains(['/', '?', '#']));
    !scheme && !lower.starts_with("//")
}

fn escape_attribute(text: &str) -> String {
    escape(text).replace('"', "&quot;")
}

/// What goes out, besides the bytes: who it is from and to whom it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub raw: Vec<u8>,
    pub from: String,
    pub recipients: Vec<String>,
}

/// The message as it goes out, from a draft: your text, then what it quotes or
/// forwards, the attachments, the thread. `from` is the account's name and address.
pub fn build(draft: &Draft, from: (&str, &str), tr: &Translator, date: i64) -> Result<Outgoing, String> {
    build_with(draft, from, tr, date, None, None)
}

/// How a message is protected with OpenPGP: signed with your key, encrypted
/// to these keys (the recipients' and yours, to read it in Sent).
pub struct Protection<'a> {
    /// Your key's signer: a key here, opened with its passphrase
    /// (`pgp::here_signer`), or a security key's (`securitykey::CardSigner`).
    pub sign: Option<&'a mut (dyn sequoia_openpgp::crypto::Signer + Send + Sync)>,
    pub encrypt_to: Vec<&'a sequoia_openpgp::Cert>,
}

/// The message as it goes out, protected when asked: PGP/MIME (RFC 3156),
/// multipart/signed for a signature alone, multipart/encrypted otherwise,
/// the signature then inside. `autocrypt` is your Autocrypt header's value.
pub fn build_with(draft: &Draft, from: (&str, &str), tr: &Translator, date: i64, protection: Option<&mut Protection>, autocrypt: Option<&str>) -> Result<Outgoing, String> {
    use mail_builder::headers::content_type::ContentType;
    use mail_builder::mime::MimePart;
    let recipients = draft.recipients();
    if recipients.is_empty() {
        return Err(tr.text("compose-no-recipient", None));
    }
    let quote = draft.original.as_deref().and_then(|path| quote(path, draft.kind, tr));
    let html_text = markdown_html(&draft.body);
    let (text, html) = match &quote {
        Some(q) => (format!("{}\n\n{}", draft.body.trim_end(), q.text), format!("{html_text}\n{}", q.html)),
        None => (draft.body.clone(), html_text),
    };
    let html = format!("<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>{html}</body></html>");
    // Signed text must come out exactly as it went in: quoted-printable keeps
    // trailing spaces (the "-- " line) and long lines from being changed on the way.
    let protected = protection.is_some();
    let text_part = |kind: &'static str, body: String| match protected {
        true => MimePart::new(ContentType::new(kind).attribute("charset", "utf-8"), quoted_printable(&body)).transfer_encoding("quoted-printable"),
        false => MimePart::new(kind, body),
    };
    let alternative = MimePart::new("multipart/alternative", vec![text_part("text/plain", text), text_part("text/html", html)]);
    let files = attachments(draft)?;
    let inner = if files.is_empty() {
        alternative
    } else {
        let mut parts = vec![alternative];
        parts.extend(files.into_iter().map(|(name, mime, bytes)| MimePart::new(mime, bytes).attachment(name)));
        MimePart::new("multipart/mixed", parts)
    };
    let body = match protection {
        Some(protection) if !protection.encrypt_to.is_empty() || protection.sign.is_some() => {
            let mut entity = Vec::new();
            inner.write_part(&mut entity);
            // The line break before the next boundary belongs to the boundary (RFC 2046 §5.1.1):
            // what is signed ends without it, as receivers cut it.
            if entity.ends_with(b"\r\n") {
                entity.truncate(entity.len() - 2);
            }
            if !protection.encrypt_to.is_empty() {
                let sealed = crate::pgp::encrypt(&entity, &protection.encrypt_to, protection.sign.as_deref_mut())?;
                MimePart::new(
                    ContentType::new("multipart/encrypted").attribute("protocol", "application/pgp-encrypted"),
                    vec![
                        MimePart::new("application/pgp-encrypted", "Version: 1\r\n".as_bytes()).transfer_encoding("7bit"),
                        MimePart::new(ContentType::new("application/octet-stream").attribute("name", "encrypted.asc"), sealed.into_bytes()).inline().transfer_encoding("7bit"),
                    ],
                )
            } else {
                let signer = protection.sign.as_deref_mut().ok_or("no key to sign with")?;
                let (signature, micalg) = crate::pgp::sign_detached(&entity, signer)?;
                MimePart::new(
                    ContentType::new("multipart/signed").attribute("micalg", micalg).attribute("protocol", "application/pgp-signature"),
                    vec![
                        MimePart::raw(entity),
                        MimePart::new(ContentType::new("application/pgp-signature").attribute("name", "signature.asc"), signature.into_bytes()).transfer_encoding("7bit"),
                    ],
                )
            }
        }
        _ => inner,
    };
    let domain = from.1.rsplit_once('@').map_or("localhost", |(_, d)| d);
    let message_id = format!("{}@{domain}", new_id());
    let to: Vec<(String, String)> = draft.to.iter().filter_map(|e| Some((name_of(e), address_of(e)?))).collect();
    let cc: Vec<(String, String)> = draft.cc.iter().filter_map(|e| Some((name_of(e), address_of(e)?))).collect();
    fn as_pairs(list: &[(String, String)]) -> Vec<(&str, &str)> {
        list.iter().map(|(n, a)| (n.as_str(), a.as_str())).collect()
    }
    let mut message = MessageBuilder::new().from(from).subject(draft.subject.as_str()).message_id(message_id.as_str()).date(date).body(body);
    if !to.is_empty() {
        message = message.to(as_pairs(&to));
    }
    if !cc.is_empty() {
        message = message.cc(as_pairs(&cc));
    }
    if let Some(reply_to) = &draft.in_reply_to {
        message = message.in_reply_to(reply_to.as_str());
    }
    if !draft.references.is_empty() {
        message = message.references(draft.references.iter().map(String::as_str).collect::<Vec<_>>());
    }
    if let Some(autocrypt) = autocrypt {
        message = message.header("Autocrypt", mail_builder::headers::raw::Raw::new(autocrypt));
    }
    let raw = message.write_to_vec().map_err(|e| e.to_string())?;
    Ok(Outgoing { raw, from: from.1.to_string(), recipients })
}

/// Quoted-printable (RFC 2045 §6.7), lines kept, CRLF, soft breaks before 76 characters.
fn quoted_printable(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + text.len() / 8);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.extend_from_slice(b"\r\n");
        }
        let line = line.strip_suffix('\r').unwrap_or(line).as_bytes();
        let mut width = 0;
        for (j, &byte) in line.iter().enumerate() {
            let last = j + 1 == line.len();
            let plain = (byte == b' ' || byte == b'\t') && !last || (33..=126).contains(&byte) && byte != b'=';
            let piece: Vec<u8> = if plain { vec![byte] } else { format!("={byte:02X}").into_bytes() };
            if width + piece.len() > 75 {
                out.extend_from_slice(b"=\r\n");
                width = 0;
            }
            width += piece.len();
            out.extend_from_slice(&piece);
        }
    }
    out
}

/// An answer to an invitation (iMIP, RFC 6047 §2.4): a sentence for people,
/// and the iTIP REPLY for their calendar, as two versions of one message.
pub fn invitation_answer(from: (&str, &str), to: &str, subject: &str, text: &str, ics: &str, date: i64) -> Result<Outgoing, String> {
    use mail_builder::headers::content_type::ContentType;
    use mail_builder::mime::MimePart;
    let domain = from.1.rsplit_once('@').map_or("localhost", |(_, d)| d);
    let message_id = format!("{}@{domain}", new_id());
    let calendar = ContentType::new("text/calendar").attribute("method", "REPLY").attribute("charset", "utf-8");
    let body = MimePart::new("multipart/alternative", vec![MimePart::new("text/plain", text), MimePart::new(calendar, ics)]);
    let raw = MessageBuilder::new()
        .from(from)
        .to(to)
        .subject(subject)
        .message_id(message_id.as_str())
        .date(date)
        .body(body)
        .write_to_vec()
        .map_err(|e| e.to_string())?;
    Ok(Outgoing { raw, from: from.1.to_string(), recipients: vec![to.to_string()] })
}

/// The name part of "Jane <jane@example.org>", else empty; a quoted name
/// ("\"Doe, Jane\" <…>") as it reads, without its quotes and escapes.
fn name_of(entry: &str) -> String {
    let Some(open) = entry.rfind('<') else { return String::new() };
    let name = entry[..open].trim();
    let Some(inner) = name.strip_prefix('"').and_then(|n| n.strip_suffix('"')) else { return name.to_string() };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        out.extend(if c == '\\' { chars.next() } else { Some(c) });
    }
    out
}

/// The files attached by hand, then, for a forward, the original's attachments.
fn attachments(draft: &Draft) -> Result<Vec<(String, &'static str, Vec<u8>)>, String> {
    let mut out = Vec::new();
    for path in &draft.attachments {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path.file_name().map_or_else(|| "attachment".to_string(), |n| n.to_string_lossy().to_string());
        out.push((name.clone(), mime_of(&name), bytes));
    }
    if let Some(original) = &draft.original {
        for (index, _) in draft.forwarded() {
            if let Some((name, bytes)) = reading::attachment(original, index) {
                out.push((name.clone(), mime_of(&name), bytes));
            }
        }
    }
    Ok(out)
}

/// The media type from a file's extension; unknown ones go as plain bytes.
pub fn mime_of(name: &str) -> &'static str {
    let extension = name.rsplit_once('.').map_or(String::new(), |(_, e)| e.to_ascii_lowercase());
    match extension.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" | "md" => "text/plain",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "ics" => "text/calendar",
        "vcf" => "text/vcard",
        "zip" => "application/zip",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "eml" => "message/rfc822",
        _ => "application/octet-stream",
    }
}

/// The message answered or forwarded, below your text.
struct Quote {
    text: String,
    html: String,
}

fn quote(original: &Path, kind: DraftKind, tr: &Translator) -> Option<Quote> {
    let raw = std::fs::read(original).ok()?;
    let message = MessageParser::default().parse(&raw)?;
    let reading = reading::read(original)?;
    let sender = message.from().and_then(|a| a.first()).map_or_else(String::new, |p| entry(p.name(), p.address().unwrap_or("")));
    let date = message.date().map_or_else(String::new, |d| d.to_rfc822());
    let body_text = message.body_text(0).map(|b| b.into_owned()).unwrap_or_default();
    let body_html = match &reading.html {
        Some(h) => format!("{}{}", h.main, h.quoted.clone().unwrap_or_default()),
        None => reading::linkify(&body_text),
    };
    let mut args = i18n::args();
    args.set("name", sender.clone());
    args.set("date", date.clone());
    if kind == DraftKind::Forward {
        let people = |list: Option<&Address>| list.map(|a| a.iter().map(|p| entry(p.name(), p.address().unwrap_or(""))).collect::<Vec<_>>().join(", ")).unwrap_or_default();
        let mut fields = vec![
            (tr.text("compose-from", None), sender),
            (tr.text("compose-date", None), date),
            (tr.text("compose-subject", None), message.subject().unwrap_or("").to_string()),
            (tr.text("compose-to", None), people(message.to())),
        ];
        let cc = people(message.cc());
        if !cc.is_empty() {
            fields.push((tr.text("compose-cc", None), cc));
        }
        let title = tr.text("compose-forwarded", None);
        let text_fields: String = fields.iter().map(|(n, v)| format!("{n}: {v}\n")).collect();
        let html_fields: String = fields.iter().map(|(n, v)| format!("<b>{}</b>: {}<br>", escape(n), escape(v))).collect();
        return Some(Quote {
            text: format!("{title}\n{text_fields}\n{body_text}"),
            html: format!("<p>{}<br>{html_fields}</p>\n{body_html}", escape(&title)),
        });
    }
    let attribution = tr.text("compose-attribution", Some(&args));
    let quoted: String = body_text.lines().map(|l| if l.is_empty() { ">\n".to_string() } else { format!("> {l}\n") }).collect();
    Some(Quote {
        text: format!("{attribution}\n{quoted}"),
        html: format!(
            "<p>{}</p>\n<blockquote type=\"cite\" style=\"margin:0 0 0 .8ex;border-left:2px solid #999;padding-left:1ex\">{body_html}</blockquote>",
            escape(&attribution)
        ),
    })
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn original(dir: &Path) -> PathBuf {
        let path = dir.join("original.eml");
        let raw = "From: Jane Exemple <jane@example.org>\r\nTo: Me <me@example.net>, Other <other@example.com>\r\n\
                   Cc: me2@example.net\r\nSubject: The lease\r\nDate: Thu, 01 Oct 2026 10:00:00 +0200\r\n\
                   Message-ID: <lease1@example.org>\r\nReferences: <start@example.org>\r\n\
                   Content-Type: text/plain; charset=utf-8\r\n\r\nHere is the lease.\r\nSee https://example.org/lease\r\n";
        std::fs::write(&path, raw).unwrap();
        path
    }

    #[test]
    fn answers_are_prefilled() {
        let dir = std::env::temp_dir().join(format!("sioul-compose-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = original(&dir);
        let own = vec!["me@example.net".to_string(), "me2@example.net".to_string()];
        let reply = answer(&path, DraftKind::Reply, "me", &own).unwrap();
        assert_eq!((reply.to.clone(), reply.subject.as_str()), (vec!["Jane Exemple <jane@example.org>".to_string()], "Re: The lease"));
        assert_eq!(reply.in_reply_to.as_deref(), Some("lease1@example.org"));
        assert_eq!(reply.references, vec!["start@example.org".to_string(), "lease1@example.org".to_string()]);
        let all = answer(&path, DraftKind::ReplyAll, "me", &own).unwrap();
        assert_eq!(all.cc, vec!["Other <other@example.com>".to_string()]);
        let forward = answer(&path, DraftKind::Forward, "me", &own).unwrap();
        assert!(forward.to.is_empty() && forward.subject == "Fwd: The lease");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn markdown_goes_out_as_html_with_the_quote_below() {
        let dir = std::env::temp_dir().join(format!("sioul-build-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = original(&dir);
        let mut draft = answer(&path, DraftKind::Reply, "me", &["me@example.net".to_string()]).unwrap();
        draft.body = "Thanks, **received**.\n\n- signed\n- dated".into();
        let tr = Translator::new("en");
        let out = build(&draft, ("Me", "me@example.net"), &tr, 1_790_000_000).unwrap();
        let text = String::from_utf8_lossy(&out.raw).to_string();
        let parsed = MessageParser::default().parse(&out.raw).unwrap();
        let html = parsed.body_html(0).unwrap().to_string();
        let plain = parsed.body_text(0).unwrap().to_string();
        assert!(html.find("<strong>received</strong>").unwrap() < html.find("<blockquote").unwrap(), "{html}");
        assert!(html.contains("<li>signed</li>") && html.contains("Here is the lease."), "{html}");
        assert!(plain.starts_with("Thanks, **received**.") && plain.contains("> Here is the lease."), "{plain}");
        assert!(text.contains("In-Reply-To: <lease1@example.org>"), "{text}");
        assert_eq!(out.recipients, vec!["jane@example.org".to_string()]);
        assert!(parsed.message_id().unwrap().ends_with("@example.net"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn line_breaks_and_signature_go_out_as_written() {
        let html = markdown_html("Hello,\nsee you soon.");
        assert!(html.contains("Hello,<br />"), "{html}");
        let mut draft = Draft::new("me");
        draft.add_signature(Some("Me\nPhotographer"));
        assert!(draft.is_empty(Some("Me\nPhotographer")));
        draft.body = format!("Thanks.{}", draft.body);
        draft.add_signature(Some("Me\nPhotographer"));
        assert_eq!(draft.body, "Thanks.\n\n-- \nMe\nPhotographer");
        assert!(!draft.is_empty(Some("Me\nPhotographer")));
        let html = markdown_html(&draft.body);
        assert!(html.contains("<p>--<br />\nMe<br />\nPhotographer</p>"), "{html}");
    }

    #[test]
    fn answering_your_own_message_goes_where_it_went() {
        let dir = std::env::temp_dir().join(format!("sioul-own-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sent.eml");
        std::fs::write(&path, "From: Me <me@example.net>\r\nTo: Jane <jane@example.org>\r\nCc: Paul <paul@example.org>\r\nSubject: Hi\r\n\r\nHi.\r\n").unwrap();
        let own = vec!["me@example.net".to_string()];
        let reply = answer(&path, DraftKind::ReplyAll, "me", &own).unwrap();
        assert_eq!(reply.to, vec!["Jane <jane@example.org>".to_string()]);
        assert_eq!(reply.cc, vec!["Paul <paul@example.org>".to_string()]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn protected_messages_open_again() {
        use sequoia_openpgp::cert::prelude::*;
        let make = |address: &str| {
            CertBuilder::general_purpose(Some(format!("<{address}>"))).set_cipher_suite(CipherSuite::Cv25519).generate().unwrap().0
        };
        let (me, jane) = (make("me@example.net"), make("jane@example.org"));
        let mut draft = Draft::new("me");
        draft.to = vec!["Jane <jane@example.org>".into()];
        draft.subject = "Keys".into();
        draft.body = "Hello,\nhere it is.\n\n-- \nMe".into();
        let tr = Translator::new("en");
        let mut signer = crate::pgp::here_signer(&me, "").unwrap();
        let signed = build_with(&draft, ("Me", "me@example.net"), &tr, 1_790_000_000, Some(&mut Protection { sign: Some(&mut signer), encrypt_to: vec![] }), None).unwrap();
        let keys = crate::pgp::Keys { others: vec![me.clone().strip_secret_key_material()], ..crate::pgp::Keys::default() };
        let none = |_: &sequoia_openpgp::Fingerprint| None;
        let sessions = crate::pgp::SessionKeys::new();
        let (_, view) = crate::pgp::open(&signed.raw, &keys, &crate::pgp::Unlock::new(&none, &sessions)).unwrap();
        assert!(!view.encrypted && view.signatures.len() == 1 && view.signatures[0].good, "{view:?}");
        let sealed = build_with(&draft, ("Me", "me@example.net"), &tr, 1_790_000_000, Some(&mut Protection { sign: Some(&mut signer), encrypt_to: vec![&jane, &me] }), Some("addr=me@example.net; keydata=AAAA")).unwrap();
        let text = String::from_utf8_lossy(&sealed.raw).to_string();
        assert!(text.contains("multipart/encrypted") && !text.contains("here it is") && text.contains("Autocrypt: addr=me@example.net"), "{text}");
        let jane_keys = crate::pgp::Keys { own: vec![jane.clone()], others: vec![me.clone().strip_secret_key_material()], ..crate::pgp::Keys::default() };
        let (opened, view) = crate::pgp::open(&sealed.raw, &jane_keys, &crate::pgp::Unlock::new(&none, &sessions)).unwrap();
        let opened = opened.unwrap();
        let parsed = MessageParser::default().parse(&opened).unwrap();
        assert_eq!(parsed.subject(), Some("Keys"));
        // The "-- " line keeps its space through quoted-printable.
        assert!(parsed.body_text(0).unwrap().replace("\r\n", "\n").contains("here it is.\n\n-- \nMe"), "{:?}", parsed.body_text(0));
        assert!(view.encrypted && view.opened && view.signatures[0].good, "{view:?}");
        assert_eq!(quoted_printable("a = b \nnext"), b"a =3D b=20\r\nnext".to_vec());
    }

    #[test]
    fn addresses_as_typed() {
        assert_eq!(split_addresses("a@example.org; Jane <j@example.org>,"), vec!["a@example.org", "Jane <j@example.org>"]);
        assert_eq!(address_of("Jane <j@example.org>").as_deref(), Some("j@example.org"));
        assert_eq!(address_of("not an address"), None);
        assert_eq!(prefixed("RE: x", "Re:", &["re:"]), "RE: x");
        assert_eq!(mime_of("Bail.PDF"), "application/pdf");
    }

    #[test]
    fn a_name_with_a_comma_is_one_recipient() {
        assert_eq!(entry(Some("Doe, Jane"), "jane@example.org"), "\"Doe, Jane\" <jane@example.org>");
        assert_eq!(entry(Some("Jane \"JJ\" Doe"), "jane@example.org"), "\"Jane \\\"JJ\\\" Doe\" <jane@example.org>");
        assert_eq!(entry(Some("Jane Doe"), "jane@example.org"), "Jane Doe <jane@example.org>");
        let field = "\"Doe, Jane\" <jane@example.org>, bob@example.org; \"x;y\" <x@example.org>\nPaul <paul@example.org>";
        let split = split_addresses(field);
        assert_eq!(split, vec!["\"Doe, Jane\" <jane@example.org>", "bob@example.org", "\"x;y\" <x@example.org>", "Paul <paul@example.org>"]);
        assert_eq!((name_of(&split[0]), address_of(&split[0]).as_deref()), ("Doe, Jane".to_string(), Some("jane@example.org")));
        assert_eq!(name_of(&entry(Some("Jane \"JJ\" Doe"), "j@example.org")), "Jane \"JJ\" Doe");
        // A quote left open: every comma separates, as before; no one is lost.
        assert_eq!(split_addresses("\"Doe, Jane <jane@example.org>, bob@example.org"), vec!["\"Doe", "Jane <jane@example.org>", "bob@example.org"]);
        // Answered, saved by the window (joined, split again), sent: still one person.
        let dir = std::env::temp_dir().join(format!("sioul-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("comma.eml");
        std::fs::write(&path, "From: \"Doe, Jane\" <jane@example.org>\r\nTo: me@example.net\r\nSubject: Hi\r\n\r\nHi.\r\n").unwrap();
        let mut reply = answer(&path, DraftKind::Reply, "me", &["me@example.net".to_string()]).unwrap();
        assert_eq!(reply.to, vec!["\"Doe, Jane\" <jane@example.org>".to_string()]);
        reply.to = split_addresses(&reply.to.join(", "));
        reply.body = "Hello.".into();
        let out = build(&reply, ("Me", "me@example.net"), &Translator::new("en"), 1_790_000_000).unwrap();
        let parsed = MessageParser::default().parse(&out.raw).unwrap();
        let to: Vec<(Option<&str>, Option<&str>)> = parsed.to().unwrap().iter().map(|p| (p.name(), p.address())).collect();
        assert_eq!(to, vec![(Some("Doe, Jane"), Some("jane@example.org"))]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn html_in_the_text_is_shown_not_read() {
        // A note quoting a mail: its HTML shows as it was written, nothing loads.
        let html = markdown_html("> <img src=\"https://tracker.example/p.gif\" width=\"1\"> and <a href=\"https://example.org\">x</a>");
        assert!(!html.contains("<img") && !html.contains("<a ") && html.contains("&lt;img src="), "{html}");
        let html = markdown_html("<div>\n<img src=\"https://tracker.example/p.gif\">\n</div>\n\nAfter.");
        assert!(!html.contains("<img") && !html.contains("<div") && html.contains("<p>After.</p>"), "{html}");
        // A picture from elsewhere is a link to it, by its words or its address.
        let html = markdown_html("![chart](https://example.org/c.png) ![](//example.org/d.png)");
        assert!(!html.contains("<img") && html.contains("<a href=\"https://example.org/c.png\">chart</a>"), "{html}");
        assert!(html.contains(">//example.org/d.png</a>"), "{html}");
        // Pictures of the vault, plain marks: kept.
        let html = markdown_html("<img src=\"file:///notes/photo.png\" width=\"560\" height=\"420\" onerror=\"x\">\n\n![x](photo.png) <b>bold</b><br>");
        assert!(html.contains("<img src=\"file:///notes/photo.png\" width=\"560\" height=\"420\">"), "{html}");
        assert!(html.contains("<img src=\"photo.png\" alt=\"x\"") && html.contains("<b>bold</b><br>"), "{html}");
        for far in ["file://server/share/p.png", "\\\\server\\share\\p.png", "file:////server/p.png", "//server/p.png", "smb://server/p.png", "https://example.org/p.png", "data:text/html,x", "/%2F/server/p.png", ""] {
            assert!(!local_source(far), "{far}");
        }
        for near in ["photo.png", "../pictures/p.png", "/notes/p.png", "file:///C:/Notes/p.png", "file://localhost/notes/p.png", "data:image/png;base64,AAAA"] {
            assert!(local_source(near), "{near}");
        }
    }

    #[test]
    fn copies_from_other_applications_go_with_their_draft() {
        let root = std::env::temp_dir().join(format!("sioul-own-copies-{}", std::process::id()));
        let copies = root.join("files");
        let shared = copies.join("r1");
        std::fs::create_dir_all(&shared).unwrap();
        let (photo, pdf, yours) = (shared.join("photo.jpg"), shared.join("lease.pdf"), root.join("yours.txt"));
        for path in [&photo, &pdf, &yours] {
            std::fs::write(path, b"x").unwrap();
        }
        // Yours, or a way out of the folder: never.
        assert!(!own_copy_in(&copies, &yours) && !own_copy_in(&copies, &shared.join("..").join("..").join("yours.txt")));
        remove_own_copy_in(&copies, &yours);
        assert!(yours.exists());
        // One taken off: its folder stays while another file is in it.
        remove_own_copy_in(&copies, &photo);
        assert!(!photo.exists() && shared.exists());
        remove_own_copy_in(&copies, &pdf);
        assert!(!shared.exists() && copies.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
