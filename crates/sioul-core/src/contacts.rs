// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contacts: one vCard per person (RFC 6350; 3.0 from RFC 2426 too), as the
//! address books on disk hold them (`vdir`).
//!
//! Sioul shows and edits the fields people use: the name, addresses, phone
//! numbers, the organisation, postal addresses, the birthday, notes and web
//! sites. Whatever else a card holds (a photo, another application's fields)
//! is kept as it was when Sioul writes it back. New cards are written in
//! vCard 3.0, which every server and phone reads.

use crate::lines;
use crate::vdir::{self, Collection, Kind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A value with what it is for: "work", "home", "cell"…; empty when the card does not say.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Labeled {
    #[serde(default)]
    pub label: String,
    pub value: String,
}

/// A contact as the window shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Contact {
    /// The file, to open and change it.
    pub key: String,
    pub uid: String,
    pub name: String,
    pub emails: Vec<Labeled>,
    pub phones: Vec<Labeled>,
    pub org: String,
    pub title: String,
    /// Each postal address on its lines.
    pub addresses: Vec<Labeled>,
    /// As the card writes it: "1984-05-12", or "--05-12" without the year.
    pub birthday: String,
    pub notes: String,
    pub urls: Vec<String>,
    /// The address book it is in, by its name.
    pub book: String,
    /// Its address book's place, "account/id": what the filters, the book
    /// chooser and the batch moves name it by (two books may share a name).
    pub place: String,
    pub read_only: bool,
    /// The name's parts as `N` holds them (RFC 6350 §6.2.2): the title
    /// (honorific prefix: "Mme", "Dr"), given names, additional names, family
    /// name and suffix; empty when the card says none. `title` is the job.
    pub prefix: String,
    pub given: String,
    pub additional: String,
    pub family: String,
    pub suffix: String,
    /// Its CATEGORIES ("Famille", "Amis"…), what Nextcloud Contacts shows as
    /// groups: every CATEGORIES line of the card, each name once
    /// (`category_key`), as the card writes it. The page shows them and
    /// filters by them, and the sender lists can name them
    /// (`category:Amis`, porch.rs's `Senders`): their mail then stands as
    /// their category says, unless an address of theirs has its own entry.
    pub categories: Vec<String>,
    /// Its picture to show, a file in the cache ("file://…"): an embedded one,
    /// or the copy kept of the one the card names on the web once you asked
    /// for it; empty without one. Nothing remote loads by itself.
    pub photo: String,
    /// The web address of a picture the card names (https), while no copy of
    /// it is kept: what "Load the photo" asks for (`web_photo_file`); empty
    /// otherwise (decided 3 November 2026).
    pub photo_web: String,
    /// Its opening hours, when it is an office's (`opening`): Sioul's own
    /// lines, which other programs keep as they are.
    pub hours: crate::opening::CardHours,
    /// What the card says it is (`card_kind`): "org" for an organisation's,
    /// "person" for a person's, "" when it says neither (a card made in the
    /// form, its name line Sioul's guess).
    pub card: &'static str,
}

/// What the contact form gives back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ContactEdit {
    pub name: String,
    #[serde(default)]
    pub emails: Vec<Labeled>,
    #[serde(default)]
    pub phones: Vec<Labeled>,
    #[serde(default)]
    pub org: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub addresses: Vec<Labeled>,
    #[serde(default)]
    pub birthday: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub urls: Vec<String>,
    /// Its categories, all of them; none (left out: an import, a sender added)
    /// leaves the card's as they are.
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    /// The name's parts as the form shows them under the name (wave D, 18
    /// November 2026): kept as given. None (an import, a sender added), or
    /// all empty: the card's own parts kept, or guessed from the name when it
    /// has none (`wanted_n`).
    #[serde(default)]
    pub parts: Option<NameParts>,
    /// For a new card, the address book it goes into, by its place
    /// ("account/id"), as the form chose it (a card's book, 20 November
    /// 2026); None (a sender added, an older Sioul's form): the book Sioul
    /// chooses first (`book_for_new`). A card that exists stays where it is:
    /// its card's chooser moves it.
    #[serde(default)]
    pub book: Option<String>,
}

/// The parts of a name the form gives, as `N` writes them: the title
/// ("Mme", "Dr"), given names, family name and suffix. Additional names stay
/// as the card holds them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameParts {
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub given: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub suffix: String,
}

impl NameParts {
    /// Whether none is given.
    pub fn is_empty(&self) -> bool {
        [&self.prefix, &self.given, &self.family, &self.suffix].iter().all(|p| p.trim().is_empty())
    }

    /// The name they make, without the title (what `FN` shows, so that other
    /// programs sort by the name): "Jeanne Martin".
    pub fn name(&self) -> String {
        [&self.given, &self.family, &self.suffix].iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
    }
}

impl ContactEdit {
    /// The name to save: the one typed, else the one its parts make (a new
    /// card given its parts alone).
    pub fn name_or_parts(&self) -> String {
        let typed = self.name.trim();
        if typed.is_empty() { self.parts.as_ref().map(NameParts::name).unwrap_or_default() } else { typed.to_string() }
    }
}

/// Reads one card with calcard, to tell whether it is valid vCard.
pub fn parse(text: &str) -> Option<calcard::vcard::VCard> {
    calcard::vcard::VCard::parse(text).ok()
}

/// One contact from its file.
pub fn read(path: &Path, book: &Collection) -> Option<Contact> {
    let text = std::fs::read_to_string(path).ok()?;
    let lines = lines::unfold(&text);
    lines.iter().any(|l| lines::name(l) == "BEGIN").then(|| contact(&lines, path, book))
}

/// A content line taken apart: its group, name and parameters; the value stays in the line.
pub(crate) struct Line<'a> {
    pub(crate) name: String,
    /// Upper-case parameter names with their values; vCard 2.1's bare values ("TEL;CELL") as TYPE.
    pub(crate) params: Vec<(String, String)>,
    pub(crate) value: &'a str,
}

pub(crate) fn split_line(line: &str) -> Line<'_> {
    let value = lines::value(line);
    // Before the colon; a line without one (a broken card) is all head, never cut inside a character.
    let head = line.len().checked_sub(value.len() + 1).filter(|&at| line.as_bytes().get(at) == Some(&b':')).map_or(line, |at| &line[..at]);
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
    let mut params = Vec::new();
    for param in parts.iter().skip(1) {
        match param.split_once('=') {
            Some((name, value)) => params.push((name.trim().to_ascii_uppercase(), value.trim().trim_matches('"').to_string())),
            None => params.push(("TYPE".to_string(), param.trim().to_string())),
        }
    }
    Line { name: lines::name(line), params, value }
}

/// Every TYPE value of a line, lower case: "TYPE=HOME,PREF" and "TYPE=home;TYPE=pref" alike.
pub(crate) fn types_of(line: &Line) -> Vec<String> {
    line.params.iter().filter(|(n, _)| n == "TYPE").flat_map(|(_, v)| v.split(',').map(|t| t.trim().to_lowercase())).filter(|t| !t.is_empty()).collect()
}

/// "work", "home", "cell"…; "internet", "voice" and "pref" say nothing.
pub(crate) fn label_of(line: &Line) -> String {
    let mut labels: Vec<String> = Vec::new();
    for kind in types_of(line) {
        if !matches!(kind.as_str(), "internet" | "voice" | "pref" | "x400") && !kind.starts_with("x-") && !labels.contains(&kind) {
            labels.push(kind);
        }
    }
    labels.join(", ")
}

/// Whether the card marks this value as the one to use first (TYPE=PREF, PREF=1).
pub(crate) fn is_preferred(line: &Line) -> bool {
    types_of(line).iter().any(|t| t == "pref") || line.params.iter().any(|(n, _)| n == "PREF")
}

/// The parts of a structured value (N, ADR, ORG), split at unescaped semicolons, unescaped.
fn components(value: &str) -> Vec<String> {
    split_unescaped(value, ';')
}

/// A value split at the separators that are not escaped (`A\,B` stays one), each part unescaped.
fn split_unescaped(value: &str, separator: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            current.push('\\');
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == separator {
            parts.push(lines::unescape(&current));
            current.clear();
        } else {
            current.push(c);
        }
    }
    parts.push(lines::unescape(&current));
    parts
}

/// What two category names are compared by: trimmed, case and accents folded
/// (`text::fold`), so "Amis", " amis" and "AMIS" are one category. The sender
/// lists match categories the same way (porch.rs).
pub fn category_key(name: &str) -> String {
    crate::text::fold(name.trim()).into_iter().collect()
}

/// A card's categories: every CATEGORIES line, its names split at unescaped
/// commas, each name once (`category_key`), as first written.
pub(crate) fn categories_of(card: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in card.iter().filter(|l| lines::name(l) == "CATEGORIES") {
        for name in split_unescaped(lines::value(line), ',') {
            let name = name.trim();
            if !name.is_empty() && !names.iter().any(|n| category_key(n) == category_key(name)) {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// The one CATEGORIES line of these names, each escaped (`A\,B`).
pub(crate) fn categories_line(names: &[String]) -> String {
    format!("CATEGORIES:{}", names.iter().map(|n| lines::escape(n.trim())).collect::<Vec<_>>().join(","))
}

/// What the window shows of a line, by property: the same reading for showing and for editing.
pub(crate) fn shown(line: &str) -> String {
    let parsed = split_line(line);
    match parsed.name.as_str() {
        "ADR" => address_lines(&components(parsed.value)),
        "ORG" => components(parsed.value).into_iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect::<Vec<_>>().join(", "),
        "BDAY" => birthday_of(&parsed),
        _ => lines::unescape(parsed.value).trim().to_string(),
    }
}

/// The name a card is shown by: its FN, else its structured name, else its
/// organisation, else its first e-mail address.
pub(crate) fn display_name(card: &[String]) -> String {
    let all = |property: &'static str| card.iter().filter(move |l| lines::name(l) == property);
    let first = |property: &'static str| all(property).map(|l| shown(l)).find(|v| !v.is_empty()).unwrap_or_default();
    let structured_name = all("N")
        .next()
        .map(|l| {
            let parts = components(split_line(l).value);
            // Family; given; additional; prefixes; suffixes → "prefixes given additional family suffixes".
            let pick = |i: usize| parts.get(i).cloned().unwrap_or_default().replace(',', " ");
            [pick(3), pick(1), pick(2), pick(0), pick(4)].into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
        })
        .unwrap_or_default();
    [all("FN").next().map(|l| shown(l)).unwrap_or_default(), structured_name, all("ORG").next().map(|l| shown(l)).unwrap_or_default(), first("EMAIL")]
        .into_iter()
        .find(|n| !n.trim().is_empty())
        .unwrap_or_default()
}

/// The contact a card's lines stand for.
pub(crate) fn contact(card: &[String], path: &Path, book: &Collection) -> Contact {
    contact_with(card, path, book, true)
}

/// The contact a card stands for, its photo never written into the cache: a
/// card only looked at (an import counted before anything is written).
pub fn contact_quiet(card: &[String], path: &Path, book: &Collection) -> Contact {
    contact_with(card, path, book, false)
}

fn contact_with(card: &[String], path: &Path, book: &Collection, photos: bool) -> Contact {
    let all = |property: &'static str| card.iter().filter(move |l| lines::name(l) == property);
    let first = |property: &'static str| all(property).next().map(|l| shown(l)).unwrap_or_default();
    let labeled = |property: &'static str| {
        all(property).map(|l| Labeled { label: label_of(&split_line(l)), value: shown(l) }).filter(|l| !l.value.is_empty()).collect::<Vec<_>>()
    };
    let emails = labeled("EMAIL");
    let org = first("ORG");
    let photo = if photos { photo_of(card) } else { (String::new(), String::new()) };
    let parts = n_parts(card).unwrap_or_default();
    let [family, given, additional, prefix, suffix] = parts.map(|p| p.trim().to_string());
    Contact {
        key: path.display().to_string(),
        uid: first("UID"),
        name: display_name(card),
        emails,
        phones: labeled("TEL"),
        org,
        title: first("TITLE"),
        addresses: labeled("ADR"),
        birthday: first("BDAY"),
        notes: all("NOTE").map(|l| shown(l)).filter(|n| !n.is_empty()).collect::<Vec<_>>().join("\n"),
        urls: all("URL").map(|l| shown(l)).filter(|u| !u.is_empty()).collect(),
        categories: categories_of(card),
        book: book.name.clone(),
        place: format!("{}/{}", book.account, book.id),
        read_only: book.read_only,
        prefix,
        given,
        additional,
        family,
        suffix,
        photo: photo.0,
        photo_web: photo.1,
        hours: crate::opening::CardHours::of(card),
        card: match card_kind(card) {
            crate::titles::Card::Company => "org",
            crate::titles::Card::Person => "person",
            crate::titles::Card::Unknown => "",
        },
    }
}

/// `N`'s five parts as the card holds them, unescaped: family, given,
/// additional, prefixes, suffixes (RFC 6350 §6.2.2); none without `N`.
pub(crate) fn n_parts(card: &[String]) -> Option<[String; 5]> {
    let line = card.iter().find(|l| lines::name(l) == "N")?;
    let parts = components(split_line(line).value);
    Some(std::array::from_fn(|i| parts.get(i).cloned().unwrap_or_default()))
}

/// Every line of a property shown, joined by line breaks, as `apply` compares
/// them: what a completion gives back to keep a card's several `ORG` or `TITLE`.
pub(crate) fn joined(card: &[String], property: &str) -> String {
    card.iter().filter(|l| lines::name(l) == property).map(|l| shown(l)).filter(|v| !v.is_empty()).collect::<Vec<_>>().join("\n")
}

/// A structured value's parts as written, split at unescaped semicolons, their escapes kept.
fn raw_parts(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            current.push(c);
            escaped = true;
        } else if c == ';' {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    parts.push(current);
    parts
}

/// The `N` line with these parts: the card's line kept as it is where a part
/// reads the same (its parameters, `SORT-AS`; a list of names stays a list),
/// a part changed written escaped; a card without `N`, a new line.
pub(crate) fn n_line(card: &[String], wanted: &[String; 5]) -> String {
    let Some(line) = card.iter().find(|l| lines::name(l) == "N") else { return format!("N:{}", lines::structured(wanted)) };
    let value = lines::value(line);
    let head = &line[..line.len() - value.len()];
    let raw = raw_parts(value);
    let shown = components(value);
    let mut parts: Vec<String> = (0..5)
        .map(|i| {
            let now = shown.get(i).map(|p| p.trim().to_string()).unwrap_or_default();
            if now == wanted[i].trim() { raw.get(i).cloned().unwrap_or_default() } else { lines::escape(wanted[i].trim()) }
        })
        .collect();
    parts.extend(raw.iter().skip(5).cloned());
    format!("{head}{}", parts.join(";"))
}

/// What a card says of itself, for its title (review F18, V7): a company's
/// (`KIND:org`, its name its organisation's, a name line with no given nor
/// family name), a person's (name parts of its own), or nothing either way
/// (no name line, or one that is only Sioul's guess of its name, as every
/// card made in the form carries: "Mr Bricolage Lyon" there says nothing).
fn card_kind(card: &[String]) -> crate::titles::Card {
    use crate::titles::Card;
    let first = |property: &str| card.iter().find(|l| lines::name(l) == property).map(|l| shown(l)).unwrap_or_default();
    let fold = |t: &str| folded(t).split_whitespace().collect::<Vec<_>>().join(" ");
    let (name, org) = (first("FN"), first("ORG"));
    // vCard 4.0's KIND, Apple's mark on a 3.0 card (`X-ABShowAs:COMPANY`, what `as_office` writes
    // when its organisation is someone else's name), or its name its organisation's.
    if first("KIND").eq_ignore_ascii_case("org") || first(SHOW_AS).eq_ignore_ascii_case("company") || (!org.is_empty() && fold(&org) == fold(&name)) {
        return Card::Company;
    }
    let Some(parts) = n_parts(card) else { return Card::Unknown };
    let names = |p: &[String; 5]| [fold(&p[0]), fold(&p[1])];
    if names(&parts).iter().all(String::is_empty) {
        return Card::Company;
    }
    if names(&parts) == names(&structured_name(&name)) || names(&parts) == names(&guessed_before(&name)) {
        Card::Unknown
    } else {
        Card::Person
    }
}

/// Whether a contact has a title: in its title field (`N`'s prefix), or at the
/// start of its name ("Mme Jeanne Martin", `titles::split`).
pub fn has_title(contact: &Contact) -> bool {
    !contact.prefix.trim().is_empty() || crate::titles::split(&contact.name).is_some()
}

/// The title Sioul's list shows greyed before a contact's name: its title
/// field, unless the name already begins with it.
pub fn title_before_name(contact: &Contact) -> String {
    let prefix = contact.prefix.trim();
    let first = contact.name.split_whitespace().next().unwrap_or("");
    if prefix.is_empty() || crate::titles::same(first, prefix) { String::new() } else { prefix.to_string() }
}

/// Where the pictures shown are kept: the cache's `photos/`.
pub fn photos_folder() -> std::path::PathBuf {
    crate::config::cache_dir().join("photos")
}

/// A card's picture, to show, and its web address while none is kept: an
/// embedded photo (vCard 3.0 `ENCODING=b`, vCard 4.0 `data:` address)
/// written once into the cache, named by what it holds, so the same picture
/// is one file; a web address (https only) never loaded here: its copy, once
/// you asked for it (`web_photo_file`), else the address alone, for "Load the
/// photo" (decided 3 November 2026: nothing remote loads by itself).
fn photo_of(card: &[String]) -> (String, String) {
    photo_in(card, &photos_folder())
}

/// The copy kept of a picture on the web, by its address: `web-<hash>.<kind>` in `folder`.
pub fn web_photo_file(folder: &Path, url: &str) -> Option<std::path::PathBuf> {
    let stem = format!("web-{}", vdir::content_hash(url.trim().as_bytes()));
    ["jpeg", "png", "webp", "gif"].iter().map(|kind| folder.join(format!("{stem}.{kind}"))).find(|p| p.is_file())
}

/// Where the copy of a picture on the web goes, its kind given ("png").
pub fn web_photo_path(folder: &Path, url: &str, kind: &str) -> std::path::PathBuf {
    folder.join(format!("web-{}.{kind}", vdir::content_hash(url.trim().as_bytes())))
}

fn photo_in(card: &[String], folder: &Path) -> (String, String) {
    let shown = photo_shown(card, folder);
    match shown.strip_prefix("web:") {
        Some(url) => match web_photo_file(folder, url) {
            Some(kept) => (crate::notes::file_url(&kept), String::new()),
            None => (String::new(), url.to_string()),
        },
        None => (shown, String::new()),
    }
}

/// The picture as a file URL, `web:<address>` for an https one, "" for none.
fn photo_shown(card: &[String], folder: &Path) -> String {
    let Some(line) = card.iter().find(|l| lines::name(l) == "PHOTO") else { return String::new() };
    let parsed = split_line(line);
    let value = parsed.value.trim();
    let embedded = parsed.params.iter().any(|(k, v)| k == "ENCODING" && (v.eq_ignore_ascii_case("b") || v.eq_ignore_ascii_case("base64")));
    let (bytes, kind) = match value.strip_prefix("data:").and_then(|d| d.split_once(";base64,")) {
        Some((media, data)) => (lines::base64_decode(data), media.rsplit('/').next().unwrap_or("jpeg").to_ascii_lowercase()),
        None if embedded => (lines::base64_decode(value), parsed.params.iter().find(|(k, _)| k == "TYPE").map_or("jpeg".to_string(), |(_, v)| v.to_ascii_lowercase())),
        None if value.starts_with("https://") => return format!("web:{value}"),
        None => return String::new(),
    };
    let Some(bytes) = bytes.filter(|b| !b.is_empty()) else { return String::new() };
    let kind: String = kind.chars().filter(char::is_ascii_alphanumeric).take(5).collect();
    let path = folder.join(format!("{}.{}", vdir::content_hash(&bytes), if kind.is_empty() { "jpeg" } else { &kind }));
    if !path.exists() {
        let written = path.parent().map(std::fs::create_dir_all).transpose().and_then(|_| std::fs::write(&path, &bytes));
        if written.is_err() {
            return String::new();
        }
    }
    // "file:///C:/…" on Windows, not "file://C:\…", which shows nothing.
    crate::notes::file_url(&path)
}

/// ADR's seven parts (box, extended, street, locality, region, code, country) on lines:
/// the street, then "code locality", the region, the country.
fn address_lines(parts: &[String]) -> String {
    let pick = |i: usize| parts.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
    let city = [pick(5), pick(3)].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
    [pick(0), pick(1), pick(2), city, pick(4), pick(6)].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Whether a line is "code locality": its first word four or five digits
/// ("69000 Lyon Cedex 03", "1000 Bruxelles"), not a street number ("1 allée…").
fn is_code_line(line: &str) -> bool {
    let first = line.split_whitespace().next().unwrap_or("");
    (4..=5).contains(&first.len()) && first.chars().all(|c| c.is_ascii_digit())
}

/// Lines back into ADR's parts, for an address typed or changed here: a line
/// whose first word is a postal code is "code locality"; after it, a last line
/// is the country; the lines before make the street.
fn address_parts(text: &str) -> Vec<String> {
    let mut lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let code_line = lines.iter().rposition(|l| is_code_line(l));
    let country = match code_line {
        Some(position) if position + 1 < lines.len() => lines.pop().unwrap_or("").to_string(),
        None if lines.len() >= 3 => lines.pop().unwrap_or("").to_string(),
        _ => String::new(),
    };
    let (mut code, mut locality) = (String::new(), String::new());
    if let Some(position) = lines.iter().rposition(|l| is_code_line(l)) {
        let line = lines.remove(position);
        match line.split_once(' ') {
            Some((c, l)) => (code, locality) = (c.to_string(), l.trim().to_string()),
            None => code = line.to_string(),
        }
    } else if lines.len() >= 2 {
        locality = lines.pop().unwrap_or("").to_string();
    }
    vec![String::new(), String::new(), lines.join(", "), locality, String::new(), code, country]
}

/// An address as it reads back once written: its lines, their parts found.
pub fn tidy_address(text: &str) -> String {
    address_lines(&address_parts(text))
}

/// "1984-05-12", or "--05-12" without the year: from "19840512", "1984-05-12T00:00:00",
/// and Apple's way of saying "no year" (X-APPLE-OMIT-YEAR).
fn birthday_of(line: &Line) -> String {
    let raw = lines::unescape(line.value);
    let digits: String = raw.split('T').next().unwrap_or("").chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
    let compact: String = digits.replace('-', "");
    let omitted = line.params.iter().find(|(n, _)| n == "X-APPLE-OMIT-YEAR").map(|(_, v)| v.clone());
    let (year, rest) = if digits.starts_with("--") {
        (None, compact.as_str())
    } else if compact.len() >= 8 {
        (Some(&compact[..4]), &compact[4..])
    } else {
        return raw.trim().to_string();
    };
    let (month, day) = (rest.get(..2).unwrap_or(""), rest.get(2..4).unwrap_or(""));
    match year.filter(|y| omitted.as_deref() != Some(*y)) {
        Some(year) => format!("{year}-{month}-{day}"),
        None => format!("--{month}-{day}"),
    }
}

/// The card's text with what the form changed, and nothing else: an unchanged
/// value keeps its line byte for byte (its group, its PREF, its spelling), so
/// what another application wrote is never rewritten for nothing. A card
/// nothing changed in comes back exactly as it was.
pub fn apply(text: &str, edit: &ContactEdit) -> String {
    let original = lines::unfold(text);
    let mut out: Vec<Option<String>> = original.iter().cloned().map(Some).collect();
    let mut added: Vec<String> = Vec::new();
    let mut changed = false;
    let typed = edit.name_or_parts();
    let name = typed.as_str();
    // The name's parts first, from the name as the card shows it now (`wanted_n`).
    let old_name = original.iter().filter(|l| lines::name(l) == "FN").map(|l| shown(l)).filter(|v| !v.is_empty()).collect::<Vec<_>>().join("\n");
    if let Some(parts) = wanted_n(&original, edit, &old_name, name) {
        changed = true;
        let line = n_line(&original, &parts);
        let present: Vec<usize> = original.iter().enumerate().filter(|(_, l)| lines::name(l) == "N").map(|(i, _)| i).collect();
        for &i in &present {
            out[i] = None;
        }
        match present.first() {
            Some(&i) => out[i] = Some(line),
            None => added.push(line),
        }
    }
    // Single values: kept when they read the same.
    for (property, wanted, write) in [
        ("FN", name.to_string(), format!("FN:{}", lines::escape(name))),
        ("ORG", edit.org.trim().to_string(), format!("ORG:{}", lines::escape(edit.org.trim()))),
        ("TITLE", edit.title.trim().to_string(), format!("TITLE:{}", lines::escape(edit.title.trim()))),
        ("BDAY", edit.birthday.trim().to_string(), format!("BDAY:{}", one_line(edit.birthday.trim()))),
        ("NOTE", edit.notes.trim().to_string(), format!("NOTE:{}", lines::escape(edit.notes.trim()))),
    ] {
        let present: Vec<usize> = original.iter().enumerate().filter(|(_, l)| lines::name(l) == property).map(|(i, _)| i).collect();
        let current = present.iter().map(|&i| shown(&original[i])).filter(|v| !v.is_empty()).collect::<Vec<_>>().join("\n");
        if current == wanted {
            continue;
        }
        changed = true;
        for &i in &present {
            out[i] = None;
        }
        if !wanted.is_empty() {
            match present.first() {
                Some(&i) => out[i] = Some(write),
                None => added.push(write),
            }
        }
    }
    // Several values: each kept, relabelled, added or removed, by its value.
    let emails: Vec<(String, String)> = edit.emails.iter().map(|e| (e.label.clone(), e.value.trim().to_string())).collect();
    let phones: Vec<(String, String)> = edit.phones.iter().map(|e| (e.label.clone(), e.value.trim().to_string())).collect();
    let addresses: Vec<(String, String)> = edit.addresses.iter().map(|e| (e.label.clone(), e.value.trim().to_string())).collect();
    let urls: Vec<(String, String)> = edit.urls.iter().map(|u| (String::new(), u.trim().to_string())).collect();
    for (property, wanted) in [("EMAIL", emails), ("TEL", phones), ("ADR", addresses), ("URL", urls)] {
        changed |= merge(&original, &mut out, &mut added, property, &wanted);
    }
    // Categories: as they are, unless the form gives others; then one line.
    if let Some(wanted) = &edit.categories {
        changed |= set_categories(&original, &mut out, &mut added, wanted);
    }
    if !changed {
        return text.to_string();
    }
    let mut kept: Vec<String> = out.into_iter().flatten().filter(|l| lines::name(l) != "REV").collect();
    added.push(format!("REV:{}", now_stamp()));
    // Before END:VCARD.
    let end = kept.iter().rposition(|l| lines::name(l) == "END").unwrap_or(kept.len());
    kept.splice(end..end, added);
    lines::fold(&kept)
}

/// The card's categories made these (`category_key` once each): its CATEGORIES
/// lines untouched when they say the same, else one line where the first was.
/// Returns whether anything changed.
fn set_categories(original: &[String], out: &mut [Option<String>], added: &mut Vec<String>, wanted: &[String]) -> bool {
    let mut names: Vec<String> = Vec::new();
    for name in wanted.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
        if !names.iter().any(|n| category_key(n) == category_key(name)) {
            names.push(name.to_string());
        }
    }
    if categories_of(original) == names {
        return false;
    }
    let present: Vec<usize> = original.iter().enumerate().filter(|(_, l)| lines::name(l) == "CATEGORIES").map(|(i, _)| i).collect();
    for &i in &present {
        out[i] = None;
    }
    if !names.is_empty() {
        let line = categories_line(&names);
        match present.first() {
            Some(&i) => out[i] = Some(line),
            None => added.push(line),
        }
    }
    true
}

/// A card with groups added and others taken out (the Contacts page's batch
/// actions, wave D), each compared as the sender lists compare them
/// (`category_key`); its other lines byte for byte, its CATEGORIES lines as
/// `set_categories` writes them, `REV` renewed. None when nothing changes: it
/// has them all already, or none of those to take out.
pub fn with_categories(text: &str, add: &[String], remove: &[String]) -> Option<String> {
    let original = lines::unfold(text);
    let gone: Vec<String> = remove.iter().map(|r| category_key(r)).filter(|k| !k.is_empty()).collect();
    let mut wanted: Vec<String> = categories_of(&original).into_iter().filter(|c| !gone.contains(&category_key(c))).collect();
    for name in add.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
        if !wanted.iter().any(|c| category_key(c) == category_key(name)) {
            wanted.push(name.to_string());
        }
    }
    let mut out: Vec<Option<String>> = original.iter().cloned().map(Some).collect();
    let mut added: Vec<String> = Vec::new();
    if !set_categories(&original, &mut out, &mut added, &wanted) {
        return None;
    }
    Some(rewritten(out, added))
}

/// What "Move the title into its field" does to one card (wave D).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitleMove {
    /// Its name begins with a title: the card with the title in `N`'s prefix
    /// and its displayed name (`FN`) without it. `after` the name it shows then;
    /// `kept` when `N` held that title already, and only the name is cleaned;
    /// `sure` when the title cannot be anything else (`titles::sure`): the
    /// others are offered unticked ("Me Work", "Dr Martens Store").
    Moves { title: String, before: String, after: String, kept: bool, sure: bool, text: String },
    /// Its name begins with no title (or it has no `FN` to clean).
    None,
    /// Its title field holds another title already (this one): left as it is.
    Differs(String),
    /// Its name is two people's ("M. et Mme Dupont"): never changed.
    Household,
}

/// A card's title moved from its displayed name into its title field: `FN`
/// "Mme Jeanne Martin" becomes "Jeanne Martin", `N`'s prefix "Mme" (its given
/// names lose the title too when Sioul's former guess put it there); a card
/// without `N` gets one guessed from the name left, with the title. `TITLE`
/// (the job) and every other line stay byte for byte; `REV` renewed.
pub fn move_title(text: &str) -> TitleMove {
    let card = lines::unfold(text);
    let Some(fn_at) = card.iter().position(|l| lines::name(l) == "FN") else { return TitleMove::None };
    let before = shown(&card[fn_at]);
    // Two people's name, whatever follows its title ("M. et Mme Dupont", "M & Mme Dupont"): never changed.
    if before.split_whitespace().next().is_some_and(crate::titles::is_title) && crate::titles::household(&before) {
        return TitleMove::Household;
    }
    let Some((title, rest)) = crate::titles::split(&before) else { return TitleMove::None };
    let sure = crate::titles::sure(&title, card_kind(&card));
    let current = n_parts(&card);
    let held = current.as_ref().map(|p| p[3].trim().to_string()).unwrap_or_default();
    if !held.is_empty() && !crate::titles::same(&held, &title) {
        return TitleMove::Differs(held);
    }
    let kept = !held.is_empty();
    let parts = match current {
        Some(mut p) => {
            if p[3].trim().is_empty() {
                p[3] = title.clone();
            }
            // Sioul's former guess put the title among the given names: taken out there too.
            if crate::titles::same(&p[1], &title) {
                p[1] = String::new();
            } else if let Some((t, given)) = crate::titles::split(&p[1])
                && crate::titles::same(&t, &title)
            {
                p[1] = given;
            }
            p
        }
        None => {
            let mut p = structured_name(&rest);
            p[3] = title.clone();
            p
        }
    };
    let mut out: Vec<Option<String>> = card.iter().cloned().map(Some).collect();
    // The name's own line keeps its parameters (LANGUAGE…): only its value changes.
    let fn_value = lines::value(&card[fn_at]);
    out[fn_at] = Some(format!("{}{}", &card[fn_at][..card[fn_at].len() - fn_value.len()], lines::escape(&rest)));
    let mut added = Vec::new();
    let line = n_line(&card, &parts);
    match card.iter().position(|l| lines::name(l) == "N") {
        Some(i) => {
            if card[i] != line {
                out[i] = Some(line);
            }
        }
        None => added.push(line),
    }
    TitleMove::Moves { title, before, after: rest, kept, sure, text: rewritten(out, added) }
}

/// A card with these opening hours (`opening`, Sioul's own lines): each of
/// its lines in the place of the first it replaces, the others before
/// `END:VCARD`; every other line byte for byte, `REV` renewed. None when the
/// card says these hours already. Hours taken off (none given) take their
/// lines with them; nothing else is ever removed.
pub fn with_hours(text: &str, hours: &crate::opening::CardHours) -> Option<String> {
    let card = lines::unfold(text);
    if crate::opening::CardHours::of(&card) == *hours {
        return None;
    }
    let wanted = hours.lines();
    let mut out: Vec<Option<String>> = card.iter().cloned().map(Some).collect();
    let mut added = Vec::new();
    for name in [crate::opening::OPEN, crate::opening::PHONE, crate::opening::NOTE] {
        let present: Vec<usize> = card.iter().enumerate().filter(|(_, l)| lines::name(l) == name).map(|(i, _)| i).collect();
        for &i in &present {
            out[i] = None;
        }
        if let Some(line) = wanted.iter().find(|l| lines::name(l) == name) {
            match present.first() {
                Some(&i) => out[i] = Some(line.clone()),
                None => added.push(line.clone()),
            }
        }
    }
    Some(rewritten(out, added))
}

/// Apple's mark of a company's card in vCard 3.0, which has no `KIND`.
const SHOW_AS: &str = "X-ABSHOWAS";

/// A card marked as an organisation's (an office, a shop, a practice: the
/// person's word, "This is an office, a shop or a practice", decided 20
/// November 2026), in the way its version lets other programs read it: a
/// vCard 4.0 card gets `KIND:org` (in the place of another `KIND`); a 3.0
/// card, which Sioul's form writes and which has no `KIND`, gets its name
/// written in its organisation (`ORG`) when that is empty, else Apple's
/// `X-ABShowAs:COMPANY`. Every other line byte for byte, `REV` renewed;
/// `card_kind` reads each of them back. None when the card says so already.
pub fn as_office(text: &str) -> Option<String> {
    let card = lines::unfold(text);
    if card_kind(&card) == crate::titles::Card::Company {
        return None;
    }
    let first = |property: &str| card.iter().position(|l| lines::name(l) == property);
    let four = card.iter().any(|l| lines::name(l) == "VERSION" && lines::value(l).trim() == "4.0");
    let mut out: Vec<Option<String>> = card.iter().cloned().map(Some).collect();
    let mut added = Vec::new();
    if four {
        match first("KIND") {
            Some(i) => out[i] = Some("KIND:org".into()),
            None => added.push("KIND:org".into()),
        }
    } else if let Some(name) = first("FN").map(|i| shown(&card[i])).filter(|n| !n.is_empty())
        && card.iter().filter(|l| lines::name(l) == "ORG").all(|l| shown(l).is_empty())
    {
        for i in card.iter().enumerate().filter(|(_, l)| lines::name(l) == "ORG").map(|(i, _)| i) {
            out[i] = None;
        }
        added.push(format!("ORG:{}", lines::escape(&name)));
    } else {
        added.push("X-ABShowAs:COMPANY".into());
    }
    Some(rewritten(out, added))
}

/// A card with one more number (`TEL`) or postal address (`ADR`), unlabelled,
/// as the form would write it; every other line byte for byte, `REV`
/// renewed. None when the card shows that value already (an office's
/// number a lookup found and the person chose to add).
pub fn with_value(text: &str, property: &str, value: &str) -> Option<String> {
    let card = lines::unfold(text);
    let value = value.trim();
    if value.is_empty() || !matches!(property, "TEL" | "ADR") {
        return None;
    }
    let wanted = if property == "ADR" { tidy_address(value) } else { value.to_string() };
    if card.iter().any(|l| lines::name(l) == property && shown(l) == wanted) {
        return None;
    }
    let out: Vec<Option<String>> = card.iter().cloned().map(Some).collect();
    Some(rewritten(out, vec![write_value("", property, "", value, false)]))
}

/// A card's lines as changed, `REV` renewed, what was added before `END:VCARD`.
pub(crate) fn rewritten(out: Vec<Option<String>>, mut added: Vec<String>) -> String {
    let mut kept: Vec<String> = out.into_iter().flatten().filter(|l| lines::name(l) != "REV").collect();
    added.push(format!("REV:{}", now_stamp()));
    let end = kept.iter().rposition(|l| lines::name(l) == "END").unwrap_or(kept.len());
    kept.splice(end..end, added);
    lines::fold(&kept)
}

/// One property of several values: an unchanged value keeps its line; a new
/// label rewrites only that line (keeping its group and PREF); new values are
/// added; values gone are removed, with the lines their group carried
/// (Apple's `item1.X-ABLabel`). Returns whether anything changed.
fn merge(original: &[String], out: &mut [Option<String>], added: &mut Vec<String>, property: &str, wanted: &[(String, String)]) -> bool {
    let present: Vec<usize> = original.iter().enumerate().filter(|(_, l)| lines::name(l) == property).map(|(i, _)| i).collect();
    let mut used: Vec<usize> = Vec::new();
    let mut changed = false;
    for (label, value) in wanted.iter().filter(|(_, v)| !v.is_empty()) {
        let found = present.iter().copied().find(|i| !used.contains(i) && shown(&original[*i]) == *value);
        match found {
            Some(i) => {
                used.push(i);
                let line = split_line(&original[i]);
                let first = label_of(&line).split(',').next().unwrap_or("").trim().to_string();
                if first != label.trim().to_lowercase() {
                    changed = true;
                    let group = original[i].split([';', ':']).next().and_then(|h| h.rsplit_once('.')).map(|(g, _)| format!("{g}.")).unwrap_or_default();
                    out[i] = Some(write_value(&group, property, label, value, is_preferred(&line)));
                }
            }
            None => {
                changed = true;
                added.push(write_value("", property, label, value, false));
            }
        }
    }
    for &i in present.iter().filter(|i| !used.contains(i)) {
        changed = true;
        out[i] = None;
        // The lines of its group go with it, unless another line still uses the group.
        if let Some((group, _)) = original[i].split([';', ':']).next().and_then(|h| h.rsplit_once('.')) {
            let prefix = format!("{group}.");
            let still_used = original.iter().enumerate().any(|(j, l)| out[j].is_some() && l.starts_with(&prefix) && ["EMAIL", "TEL", "ADR", "URL"].contains(&lines::name(l).as_str()));
            if !still_used {
                for (j, line) in original.iter().enumerate() {
                    if line.starts_with(&prefix) {
                        out[j] = None;
                    }
                }
            }
        }
    }
    changed
}

/// A value's line, as vCard 3.0 writes it.
fn write_value(group: &str, property: &str, label: &str, value: &str, preferred: bool) -> String {
    let mut types: Vec<&str> = Vec::new();
    if property == "EMAIL" {
        types.push("INTERNET");
    }
    types.extend(known_type(label));
    if preferred {
        types.push("PREF");
    }
    let params = if types.is_empty() { String::new() } else { format!(";TYPE={}", types.join(",")) };
    let value = match property {
        "ADR" => lines::structured(&address_parts(value)),
        "URL" => one_line(value),
        _ => lines::escape(value),
    };
    format!("{group}{property}{params}:{value}")
}

/// A value written as it is (a date, a web address): without line breaks or
/// other control characters, which would start another line of the card.
fn one_line(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
}

fn known_type(label: &str) -> Option<&'static str> {
    match label.trim().to_lowercase().as_str() {
        "work" => Some("WORK"),
        "home" => Some("HOME"),
        "cell" | "mobile" => Some("CELL"),
        "fax" => Some("FAX"),
        "pager" => Some("PAGER"),
        _ => None,
    }
}

/// "Jane van Exemple" → N: family "van Exemple"? Names do not split by rule:
/// the guess for a card that gives no parts. A title at the start goes to the
/// title part ("Mme Jeanne Martin": `Martin;Jeanne;;Mme;`); then the last word
/// is taken as the family name, the rest as given names.
fn structured_name(name: &str) -> [String; 5] {
    // Only a sure title, never in a name of two people ("Me Work", "M. et Mme Dupont").
    let (prefix, rest) = match crate::titles::split(name) {
        // The person names a person here (the form, a new card): a title that is not unsure is taken.
        Some((title, rest)) if crate::titles::sure(&title, crate::titles::Card::Person) && !crate::titles::household(name) => (title, rest),
        _ => (String::new(), name.trim().to_string()),
    };
    let words: Vec<&str> = rest.split_whitespace().collect();
    match words.split_last() {
        Some((family, given)) if !given.is_empty() => [family.to_string(), given.join(" "), String::new(), prefix, String::new()],
        _ => [rest, String::new(), String::new(), prefix, String::new()],
    }
}

/// Sioul's guess before titles were known (until 18 November 2026): the last
/// word the family name, the rest given names, "Mme" among them.
fn guessed_before(name: &str) -> [String; 5] {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.split_last() {
        Some((family, given)) if !given.is_empty() => [family.to_string(), given.join(" "), String::new(), String::new(), String::new()],
        _ => [name.to_string(), String::new(), String::new(), String::new(), String::new()],
    }
}

/// What `N` becomes after an edit: None, kept byte for byte. Parts typed in
/// the form are written as typed (additional names kept as the card holds
/// them). A card whose given, family and suffix are none, or Sioul's guess
/// of its former name, follows a new name by the same guess, keeping its
/// title and its additional names (review F19: a card given its title by the
/// title's move followed no rename any more); other parts are kept, whatever
/// the name becomes.
fn wanted_n(card: &[String], edit: &ContactEdit, old_name: &str, new_name: &str) -> Option<[String; 5]> {
    let current = n_parts(card);
    let renamed = old_name.trim() != new_name;
    let four = |p: &[String; 5]| [p[0].trim().to_string(), p[1].trim().to_string(), p[3].trim().to_string(), p[4].trim().to_string()];
    let names = |p: &[String; 5]| [p[0].trim().to_string(), p[1].trim().to_string(), p[4].trim().to_string()];
    // No names of its own: none at all, or Sioul's guess of the name it had (today's or the former one).
    let guessed = |p: &[String; 5]| names(p).iter().all(String::is_empty) || names(p) == names(&structured_name(old_name)) || names(p) == names(&guessed_before(old_name));
    // The guess of the new name, the title and additional names kept.
    let follow = |p: &[String; 5]| {
        let mut guess = structured_name(new_name);
        if guess[3].is_empty() {
            guess[3] = p[3].clone();
        }
        guess[2] = p[2].clone();
        guess
    };
    let typed = edit.parts.as_ref().filter(|p| !p.is_empty());
    match (typed, current) {
        (Some(typed), Some(current)) => {
            let wanted = [typed.family.trim().to_string(), typed.given.trim().to_string(), current[2].clone(), typed.prefix.trim().to_string(), typed.suffix.trim().to_string()];
            if four(&wanted) != four(&current) {
                Some(wanted)
            } else if renamed && guessed(&current) {
                Some(follow(&current))
            } else {
                None
            }
        }
        (Some(typed), None) => Some([typed.family.trim().to_string(), typed.given.trim().to_string(), String::new(), typed.prefix.trim().to_string(), typed.suffix.trim().to_string()]),
        (None, Some(current)) => (renamed && guessed(&current)).then(|| follow(&current)),
        (None, None) => renamed.then(|| structured_name(new_name)),
    }
}

/// "20261003T120000Z", for REV.
pub(crate) fn now_stamp() -> String {
    jiff::Timestamp::now().strftime("%Y%m%dT%H%M%SZ").to_string()
}

/// A new card from the form, in vCard 3.0, with its own UID.
pub fn new_card(edit: &ContactEdit) -> String {
    new_card_with_uid(edit, &vdir::new_name())
}

/// A new card from the form, with the UID given (an import that must find it again).
pub fn new_card_with_uid(edit: &ContactEdit, uid: &str) -> String {
    let start = lines::fold(&["BEGIN:VCARD".to_string(), "VERSION:3.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), format!("UID:{uid}"), "END:VCARD".to_string()]);
    apply(&start, edit)
}

/// A card as vCard 3.0 (RFC 2426), for servers that take nothing newer
/// (Google): what only 4.0 has goes, `PREF=1` becomes `TYPE=pref`, a photo
/// given as a `data:` address is written inline. A 3.0 card comes back as is.
pub fn as_vcard3(card: &str) -> String {
    let lines = lines::unfold(card);
    if !lines.iter().any(|l| l.eq_ignore_ascii_case("VERSION:4.0")) {
        return card.to_string();
    }
    const ONLY_4: &[&str] = &["KIND", "GENDER", "ANNIVERSARY", "MEMBER", "RELATED", "LANG", "CLIENTPIDMAP", "XML", "CALADRURI", "CALURI", "FBURL"];
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let Some((head, value)) = line.split_once(':') else {
            out.push(line);
            continue;
        };
        let mut params = head.split(';');
        let name = params.next().unwrap_or("");
        let bare = name.rsplit('.').next().unwrap_or(name).to_ascii_uppercase();
        if bare == "VERSION" {
            out.push("VERSION:3.0".to_string());
            continue;
        }
        if ONLY_4.contains(&bare.as_str()) {
            continue;
        }
        let mut kept: Vec<String> = Vec::new();
        for param in params {
            let upper = param.to_ascii_uppercase();
            if upper.starts_with("PREF=") {
                kept.push("TYPE=pref".into());
            } else if !(upper.starts_with("PID=") || upper.starts_with("ALTID=") || upper.starts_with("VALUE=URI") && value.starts_with("data:")) {
                kept.push(param.to_string());
            }
        }
        // PHOTO:data:image/jpeg;base64,… → PHOTO;ENCODING=b;TYPE=JPEG:…
        if let Some((media, data)) = value.strip_prefix("data:").and_then(|d| d.split_once(";base64,")) {
            let format = media.rsplit('/').next().unwrap_or("jpeg").to_ascii_uppercase();
            kept.push("ENCODING=b".into());
            kept.push(format!("TYPE={format}"));
            out.push(format!("{name};{}:{data}", kept.join(";")));
            continue;
        }
        let head = std::iter::once(name.to_string()).chain(kept).collect::<Vec<_>>().join(";");
        out.push(format!("{head}:{value}"));
    }
    lines::fold(&out)
}

/// Every contact of every address book, by name.
pub fn all() -> Vec<Contact> {
    let mut contacts: Vec<Contact> = vdir::collections(Kind::Contacts)
        .iter()
        .flat_map(|book| book.items().into_iter().filter_map(|path| read(&path, book)).collect::<Vec<_>>())
        .collect();
    contacts.sort_by_key(|c| sort_key(&c.name));
    contacts
}

/// Names sort without their accents and case: "Émile" next to "Emma".
fn sort_key(name: &str) -> String {
    folded(name)
}

fn folded(text: &str) -> String {
    crate::text::fold(text).into_iter().collect::<String>().to_lowercase()
}

/// Contacts whose name, organisation, addresses or numbers hold the query.
pub fn search<'a>(contacts: &'a [Contact], query: &str) -> Vec<&'a Contact> {
    let query = folded(query.trim());
    if query.is_empty() {
        return contacts.iter().collect();
    }
    contacts
        .iter()
        .filter(|c| {
            std::iter::once(&c.name)
                .chain(std::iter::once(&c.org))
                .chain(c.emails.iter().map(|e| &e.value))
                .chain(c.phones.iter().map(|p| &p.value))
                .any(|field| folded(field).contains(&query))
        })
        .collect()
}

/// The categories the cards have, to filter by: the most used first, then by
/// name; each name once (`category_key`), as first written.
pub fn categories_in_use(contacts: &[Contact]) -> Vec<String> {
    let mut used: Vec<(usize, String, String)> = Vec::new();
    for name in contacts.iter().flat_map(|c| c.categories.iter()) {
        let key = category_key(name);
        match used.iter_mut().find(|(_, k, _)| *k == key) {
            Some((count, _, _)) => *count += 1,
            None => used.push((1, key, name.clone())),
        }
    }
    used.sort_by(|(a, x, _), (b, y, _)| b.cmp(a).then_with(|| x.cmp(y)));
    used.into_iter().map(|(_, _, name)| name).collect()
}

/// Whether a contact has this category, case and accents aside.
pub fn in_category(contact: &Contact, category: &str) -> bool {
    let wanted = category_key(category);
    contact.categories.iter().any(|c| category_key(c) == wanted)
}

/// The contact with this e-mail address.
pub fn by_address<'a>(contacts: &'a [Contact], address: &str) -> Option<&'a Contact> {
    contacts.iter().find(|c| c.emails.iter().any(|e| e.value.eq_ignore_ascii_case(address.trim())))
}

/// The characters a URI holds as they are in the address Sioul writes for a
/// card (`uid_uri`): RFC 3986's unreserved ones, and those a URN's name may
/// hold that end no parameter of a line (no comma, no semicolon, no quote).
fn uri_safe(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~!$&'()*+=:@/".contains(&byte)
}

/// Whether `text` begins with `prefix`, case aside.
fn starts_ci(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len()).is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

/// The address a calendar program can follow to a card, by its UID, as an
/// event's `CONTACT;ALTREP` holds it (RFC 5545 §3.8.4.2): `urn:uuid:` and the
/// UID (RFC 9562's URN), or the UID as it is when it is a URN already (vCard
/// 4.0 writes `UID:urn:uuid:…`). A character a URI cannot hold is
/// percent-encoded (RFC 3986 §2.1), so that every reader takes the whole of
/// it: a phone's ical4j parses the parameter as a URI. A card the door of an
/// extension made has a UID of its own form, not a UUID
/// (`extensions::person_mark`): it is written under `urn:uuid:` all the same,
/// which Sioul reads back, and which other programs keep as an opaque address.
pub fn uid_uri(uid: &str) -> String {
    let uid = uid.trim();
    let mut encoded = String::with_capacity(uid.len());
    for byte in uid.bytes() {
        if uri_safe(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    if starts_ci(uid, "urn:") { encoded } else { format!("urn:uuid:{encoded}") }
}

/// `%XX` sequences decoded (a malformed one kept as written).
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let Some(byte) = text.get(at + 1..at + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            at += 3;
            continue;
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A card's UID as an address names it: without `urn:uuid:`, in lower case
/// (a UUID's letters are the same whatever their case, RFC 9562 §4). Every
/// UID is lowered, not only a UUID: two cards whose UIDs differ by case alone
/// are one key, the first found winning (`ByUid::of`).
fn uid_key(uid: &str) -> String {
    let uid = uid.trim();
    let bare = if starts_ci(uid, "urn:uuid:") { &uid["urn:uuid:".len()..] } else { uid };
    bare.to_lowercase()
}

/// The UID key an address names (`uid_uri`'s, or any `urn:` address another
/// program wrote): none for an address that is no URN.
fn uri_key(uri: &str) -> Option<String> {
    let uri = uri.trim();
    starts_ci(uri, "urn:").then(|| uid_key(&percent_decoded(uri)))
}

/// The cards of `contacts` by the UID an address names (`uid_uri`): what
/// reads an event's or a task's `CONTACT;ALTREP="urn:uuid:…"` as a tie to a
/// card, whoever wrote it. Made once for many addresses.
#[derive(Debug, Default)]
pub struct ByUid(std::collections::HashMap<String, usize>);

impl ByUid {
    pub fn of(contacts: &[Contact]) -> ByUid {
        let mut map = std::collections::HashMap::with_capacity(contacts.len());
        for (at, contact) in contacts.iter().enumerate().filter(|(_, c)| !c.uid.trim().is_empty()) {
            map.entry(uid_key(&contact.uid)).or_insert(at);
        }
        ByUid(map)
    }

    /// The card an address names, by its place in the contacts given: none
    /// when the address is no URN or no card has that UID here.
    pub fn find(&self, uri: &str) -> Option<usize> {
        self.0.get(&uri_key(uri)?).copied()
    }
}

/// The UID of the card an address names among `contacts` (`ByUid`, for one address).
pub fn uid_named<'a>(contacts: &'a [Contact], uri: &str) -> Option<&'a str> {
    let key = uri_key(uri)?;
    contacts.iter().find(|c| !c.uid.trim().is_empty() && uid_key(&c.uid) == key).map(|c| c.uid.as_str())
}

/// Sioul's own address of a card (`links::contact_uri`): how agents, the
/// window and the ties name a card, and how a task's `CONTACT;ALTREP` named
/// it until 26 November 2026.
const OWN_ADDRESS: &str = "sioul:contact/";

/// The UID inside Sioul's own address of a card, decoded; none for another address.
fn own_uid(uri: &str) -> Option<String> {
    let id = uri.trim().strip_prefix(OWN_ADDRESS)?;
    Some(crate::notes::decode(id.split('#').next().unwrap_or(id)).trim().to_string()).filter(|uid| !uid.is_empty())
}

/// The address a task's `CONTACT;ALTREP` holds for `uri` (decided 26
/// November 2026, for other calendar programs): a card named by Sioul's own
/// address (`sioul:contact/<UID>`, as the window, the agents and the
/// call-back rules give it) as `urn:uuid:<UID>`, with the encoding an
/// extension's door writes (`uid_uri`): the standard form, an RFC 9562 URN,
/// which other programs keep and send back (a UID that is no UUID, as
/// Sioul's own, as an opaque address); any other address as it is.
pub fn altrep_of(uri: &str) -> String {
    own_uid(uri).map_or_else(|| uri.trim().to_string(), |uid| uid_uri(&uid))
}

/// The key of the card an address names, whichever form it has: a URN
/// (`urn:uuid:<UID>`, what Sioul writes in a task's or an event's `CONTACT`
/// since 26 November 2026, `uid_uri`, or any `urn:` another program wrote),
/// or Sioul's own `sioul:contact/<UID>` (what tasks written before hold, and
/// how agents name a card). None for any other address. Every reader of a
/// task's contact asks here or below, so that both forms name one card.
pub fn card_key(uri: &str) -> Option<String> {
    let key = match own_uid(uri) {
        Some(uid) => uid_key(&uid),
        None => uri_key(uri)?,
    };
    (!key.is_empty()).then_some(key)
}

/// Whether `uri` names the card whose UID is `uid`, in either form (`card_key`).
pub fn names_card(uri: &str, uid: &str) -> bool {
    let key = uid_key(uid);
    !key.is_empty() && card_key(uri).is_some_and(|named| named == key)
}

/// Whether two addresses are one card's, each in either form (`card_key`).
pub fn same_card(a: &str, b: &str) -> bool {
    card_key(a).is_some_and(|key| card_key(b).is_some_and(|other| other == key))
}

/// The card an address names among `contacts`, in either form (`card_key`):
/// for Sioul's own address, the card of exactly that UID first.
pub fn card_named<'a>(contacts: &'a [Contact], uri: &str) -> Option<&'a Contact> {
    let key = card_key(uri)?;
    own_uid(uri)
        .and_then(|uid| contacts.iter().find(|c| c.uid.trim() == uid))
        .or_else(|| contacts.iter().find(|c| !c.uid.trim().is_empty() && uid_key(&c.uid) == key))
}

/// Sioul's own address (`links::contact_uri`) of the card an address names,
/// in either form: by the card's own UID when a card here has it; else by
/// the UID the address gives, `sioul:contact/<UID>` as it is and
/// `urn:uuid:<UID>` without its prefix (a card not on this device, or gone).
/// None for an address that names no card.
pub fn card_address(contacts: &[Contact], uri: &str) -> Option<String> {
    if let Some(card) = card_named(contacts, uri) {
        return Some(crate::links::contact_uri(card.uid.trim()));
    }
    if let Some(uid) = own_uid(uri) {
        return Some(crate::links::contact_uri(&uid));
    }
    let uri = uri.trim();
    let rest = percent_decoded(uri.get("urn:uuid:".len()..).filter(|_| starts_ci(uri, "urn:uuid:"))?);
    (!rest.trim().is_empty()).then(|| crate::links::contact_uri(rest.trim()))
}

/// Addresses to complete what is typed in a To field: "Jane <jane@example.org>".
pub fn completions(contacts: &[Contact], typed: &str, limit: usize) -> Vec<String> {
    let typed = folded(typed.trim());
    if typed.chars().count() < 2 {
        return Vec::new();
    }
    contacts
        .iter()
        .flat_map(|c| c.emails.iter().map(move |e| (c, e)))
        .filter(|(c, e)| folded(&c.name).split_whitespace().any(|w| w.starts_with(&typed)) || folded(&e.value).starts_with(&typed) || folded(&c.name).starts_with(&typed))
        .take(limit)
        .map(|(c, e)| if c.name.is_empty() || c.name == e.value { e.value.clone() } else { format!("{} <{}>", c.name, e.value) })
        .collect()
}

/// Where a new contact goes: the first address book that can be written to.
pub fn default_book() -> Option<Collection> {
    writable_books().into_iter().next()
}

/// The address books a contact can go into, in the order Contacts ⚙ lists
/// them: those that can be written; never a book its server refused to make
/// (review V9), whose cards would stay on this device alone.
pub fn writable_books() -> Vec<Collection> {
    vdir::collections(Kind::Contacts).into_iter().filter(|b| !b.read_only && !vdir::State::load(&b.state_path()).refused).collect()
}

/// The address book Sioul chooses first for a new card, by place
/// ("account/id"), among `writable` (in their order): the one the list is
/// filtered on when it shows one book alone (`shown`, the filter's books
/// that exist) and it can be written; else the book of the last card made on
/// this device (`last`), while it can be written; else the first. None when
/// no book can be written. What it chooses counts until the person chooses
/// another in the form, asked nothing more.
pub fn book_for_new(writable: &[String], shown: &[String], last: &str) -> Option<String> {
    let takes = |place: &str| writable.iter().any(|w| w == place);
    if let [one] = shown
        && takes(one)
    {
        return Some(one.clone());
    }
    if !last.is_empty() && takes(last) {
        return Some(last.to_string());
    }
    writable.first().cloned()
}

/// Why a new card cannot go into the book chosen for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookRefused {
    /// No address book has that place any more (removed, its account gone).
    Gone,
    /// It is there, but takes no new card (read only, or its server refused
    /// to make it): its name, to say it.
    Closed(Collection),
}

/// The address book with this place ("account/id"), for a new card: refused
/// when it is gone or cannot take one.
pub fn book_to_write(place: &str) -> Result<Collection, BookRefused> {
    let book = vdir::collections(Kind::Contacts).into_iter().find(|b| format!("{}/{}", b.account, b.id) == place).ok_or(BookRefused::Gone)?;
    if book.read_only || vdir::State::load(&book.state_path()).refused {
        return Err(BookRefused::Closed(book));
    }
    Ok(book)
}

/// The file a new contact gets in an address book.
pub fn new_path(book: &Collection) -> PathBuf {
    book.dir.join(format!("{}.vcf", vdir::new_name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An office's hours written through the contacts' writer: in the place of
    /// their lines, every other line as it was; nothing when they say the same.
    #[test]
    fn opening_hours_on_a_card() {
        let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:hall\r\nFN:Example town hall\r\nTEL:01 23 45 67 89\r\nX-OTHER-APP:kept\r\nEND:VCARD\r\n";
        let hours = crate::opening::CardHours { open: Some(crate::opening::Hours { value: "Mo-Fr 09:00-12:00; PH off".into(), source: "typed".into(), checked: "2026-11-20".into(), holidays_open: false }), phone: None, note: "Closed in August".into(), extra: Vec::new() };
        let written = with_hours(card, &hours).unwrap();
        let lines = lines::unfold(&written);
        assert!(lines.contains(&"X-OTHER-APP:kept".to_string()) && lines.contains(&"TEL:01 23 45 67 89".to_string()));
        assert!(lines.contains(&"X-SIOUL-OPENING-HOURS;X-SIOUL-SOURCE=typed;X-SIOUL-CHECKED=2026-11-20:Mo-Fr 09:00-12:00\\; PH off".to_string()), "{lines:?}");
        assert_eq!(crate::opening::CardHours::of(&lines), hours);
        assert_eq!(with_hours(&written, &hours), None, "the same hours: nothing written");
        // Changed: in the place of the first, the note taken off with nothing else.
        let changed = crate::opening::CardHours { note: String::new(), ..hours.clone() };
        let again = lines::unfold(&with_hours(&written, &crate::opening::CardHours { open: Some(crate::opening::Hours { value: "Sa 09:00-12:00; PH off".into(), ..Default::default() }), ..changed }).unwrap());
        let at = |l: &[String], name: &str| l.iter().position(|x| lines::name(x) == name);
        assert_eq!(at(&again, crate::opening::OPEN), at(&lines, crate::opening::OPEN), "where it was");
        assert!(at(&again, crate::opening::NOTE).is_none() && again.contains(&"X-OTHER-APP:kept".to_string()));
        // An older Sioul's form saved over it keeps the lines (`apply` changes only its fields).
        let older = apply(&written, &ContactEdit { name: "Example town hall".into(), phones: vec![Labeled { label: String::new(), value: "01 23 45 67 89".into() }], notes: "A note".into(), ..ContactEdit::default() });
        assert_eq!(crate::opening::CardHours::of(&lines::unfold(&older)), hours);
        // A number a lookup found, added when asked; never twice.
        let added = with_value(card, "TEL", "+33 1 98 76 54 32").unwrap();
        assert!(lines::unfold(&added).contains(&"TEL:+33 1 98 76 54 32".to_string()));
        assert_eq!(with_value(card, "TEL", "01 23 45 67 89"), None);
    }

    /// The card's writer for hours at its edges (review 11): a line folded
    /// over 75 octets read and replaced where it was; a grouped line
    /// (`item1.`) of another program kept; two lines of hours, one written
    /// back, the second's text in the free line; a source parameter that
    /// would break the line written as a parameter, read back the same; an
    /// address a lookup found, added in its parts.
    #[test]
    fn hours_on_a_card_at_the_edges() {
        let long = format!("Mo-Fr 09:00-12:00,14:00-17:00; Sa 09:00-12:00; {}", "\"by appointment on the first Thursday of the month\" ".repeat(2));
        let card = lines::fold(&["BEGIN:VCARD".to_string(), "VERSION:3.0".into(), "FN:Example office".into(), "item1.TEL:01 23 45 67 89".into(), "item1.X-ABLabel:Counter".into(), format!("X-SIOUL-OPENING-HOURS:{}", lines::escape(&long)), "X-SIOUL-OPENING-HOURS:Su 10:00-11:00".into(), "END:VCARD".into()]);
        assert!(card.contains("\r\n "), "folded");
        let read = crate::opening::CardHours::of(&lines::unfold(&card));
        assert_eq!(read.open.as_ref().unwrap().value, long.trim());
        assert_eq!(read.extra, vec!["Su 10:00-11:00"]);
        let hostile = crate::opening::CardHours { open: Some(crate::opening::Hours { value: "Mo 09:00-10:00".into(), source: "evil\";X-OTHER=1:Mo".into(), ..Default::default() }), ..read.clone().folded() };
        let written = with_hours(&card, &hostile).unwrap();
        let after = lines::unfold(&written);
        assert!(after.contains(&"item1.TEL:01 23 45 67 89".to_string()) && after.contains(&"item1.X-ABLabel:Counter".to_string()), "{after:?}");
        assert_eq!(after.iter().filter(|l| lines::name(l) == crate::opening::OPEN).count(), 1, "one line of hours");
        assert_eq!(after.iter().position(|l| lines::name(l) == crate::opening::OPEN), Some(5), "where the first was");
        let back = crate::opening::CardHours::of(&after);
        assert_eq!(back.open.as_ref().unwrap().source, "evil\";X-OTHER=1:Mo", "the parameter read back as written");
        assert_eq!(back.open.as_ref().unwrap().value, "Mo 09:00-10:00");
        assert_eq!(back.note, "Su 10:00-11:00");
        // An address a lookup found: added in its parts, as the form writes one.
        let with_address = with_value(&written, "ADR", "1 Place de l'Exemple\n99999 Exempleville").unwrap();
        let adr = lines::unfold(&with_address).into_iter().find(|l| lines::name(l) == "ADR").unwrap();
        assert!(adr.contains("1 Place de l'Exemple") && adr.contains("99999") && adr.contains("Exempleville"), "{adr}");
        assert_eq!(with_value(&with_address, "ADR", "1 Place de l'Exemple\n99999 Exempleville"), None, "never twice");
    }

    /// "This is an office, a shop or a practice": a card marked as an
    /// organisation's as its version lets other programs read it, every other
    /// line kept, read back as one; a person's card read from its text is
    /// neither, and nothing can be looked up for it (review of 20 November 2026, 1).
    #[test]
    fn a_card_said_to_be_an_office_s() {
        let book = book();
        let read = |text: &str| contact(&lines::unfold(text), Path::new("/books/x.vcf"), &book);
        let person = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:jeanne\r\nFN:Jeanne Exemple\r\nN:Exemple;Jeanne;;;\r\nTEL:01 23 45 67 89\r\nADR:;;1 rue de l'Exemple;Exempleville;;99999;\r\nX-OTHER-APP:kept\r\nEND:VCARD\r\n";
        let jeanne = read(person);
        assert_eq!(jeanne.card, "", "its name line is Sioul's guess: the card says neither");
        assert_eq!(crate::hourslookup::Asked::of(&jeanne, true), None, "nothing can be asked for it");
        // vCard 3.0, its organisation empty: its name written there, every other line kept.
        let office = as_office(person).unwrap();
        let lines = lines::unfold(&office);
        assert!(lines.contains(&"ORG:Jeanne Exemple".to_string()) && lines.contains(&"X-OTHER-APP:kept".to_string()) && lines.contains(&"N:Exemple;Jeanne;;;".to_string()), "{lines:?}");
        assert_eq!(read(&office).card, "org");
        assert!(crate::hourslookup::Asked::of(&read(&office), true).is_some());
        assert_eq!(as_office(&office), None, "said already");
        // Its organisation another name: Apple's mark, the organisation kept.
        let at_work = person.replace("X-OTHER-APP:kept", "ORG:Exemple SARL");
        let marked = as_office(&at_work).unwrap();
        assert!(marked.contains("X-ABShowAs:COMPANY\r\n") && marked.contains("ORG:Exemple SARL\r\n"), "{marked}");
        assert_eq!(read(&marked).card, "org");
        // vCard 4.0: KIND, in the place of the one there.
        let four = "BEGIN:VCARD\r\nVERSION:4.0\r\nKIND:individual\r\nFN:Cabinet Exemple\r\nN:Exemple;Cabinet;;;\r\nEND:VCARD\r\n";
        let written = as_office(four).unwrap();
        assert!(written.starts_with("BEGIN:VCARD\r\nVERSION:4.0\r\nKIND:org\r\nFN:Cabinet Exemple\r\n"), "{written}");
        assert_eq!(read(&written).card, "org");
        // A card with hours typed on it is the person's word that it is an office's: asked.
        let typed = with_hours(person, &crate::opening::CardHours { note: "By appointment".into(), ..Default::default() }).unwrap();
        assert!(crate::hourslookup::Asked::of(&read(&typed), true).is_some());
    }

    #[test]
    fn a_photo_to_show() {
        let card = |photo: &str| lines::unfold(&format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Jane\r\n{photo}\r\nEND:VCARD\r\n"));
        let folder = std::env::temp_dir().join(format!("sioul-photos-{}", std::process::id()));
        let photo_of = |card: &[String]| photo_in(card, &folder).0;
        let three = photo_of(&card("PHOTO;ENCODING=b;TYPE=PNG:aGVsbG8="));
        let four = photo_of(&card("PHOTO:data:image/png;base64,aGVsbG8="));
        assert!(three.starts_with("file:///") && three.ends_with(".png"), "{three}");
        // The same picture, one file.
        assert_eq!(three, four);
        let written = folder.join(format!("{}.png", vdir::content_hash(b"hello")));
        assert_eq!((three.clone(), std::fs::read(&written).unwrap()), (crate::notes::file_url(&written), b"hello".to_vec()));
        // On the web: never loaded here; its address for "Load the photo", then its copy once kept
        // (decided 3 November 2026).
        let web = card("PHOTO;VALUE=uri:https://example.org/jane.jpg");
        assert_eq!(photo_in(&web, &folder), (String::new(), "https://example.org/jane.jpg".to_string()));
        let kept = web_photo_path(&folder, "https://example.org/jane.jpg", "jpeg");
        std::fs::write(&kept, b"jpeg").unwrap();
        assert_eq!(photo_in(&web, &folder), (crate::notes::file_url(&kept), String::new()));
        assert_eq!(web_photo_file(&folder, "https://example.org/other.jpg"), None, "another address: another copy");
        assert_eq!(photo_in(&card("PHOTO;VALUE=uri:http://example.org/jane.jpg"), &folder), (String::new(), String::new()));
        assert_eq!(photo_of(&card("NOTE:none")), "");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_card_for_google() {
        let card = "BEGIN:VCARD\r\nVERSION:4.0\r\nKIND:individual\r\nFN:Jane\r\nEMAIL;PREF=1;TYPE=work:jane@example.org\r\nGENDER:F\r\nPHOTO:data:image/png;base64,iVBORw0K\r\nEND:VCARD\r\n";
        let three = as_vcard3(card);
        assert_eq!(three, "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Jane\r\nEMAIL;TYPE=pref;TYPE=work:jane@example.org\r\nPHOTO;ENCODING=b;TYPE=PNG:iVBORw0K\r\nEND:VCARD\r\n");
        assert_eq!(as_vcard3(&three), three);
    }

    const CARD: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:jane-1\r\nFN:Jane Exemple\r\nN:Exemple;Jane;;;\r\n\
        EMAIL;TYPE=INTERNET,WORK:jane@example.org\r\nEMAIL;TYPE=HOME:jane@example.net\r\nTEL;TYPE=CELL:+33 6 00 00 00 00\r\n\
        ORG:Exemple SARL\r\nADR;TYPE=WORK:;;1 rue de l'Exemple;Lyon;;69000;France\r\nBDAY:1984-05-12\r\n\
        X-OTHER-APP:keep me\r\nPHOTO;ENCODING=b;TYPE=JPEG:AAAA\r\nNOTE:Met at the market\r\nEND:VCARD\r\n";

    fn book() -> Collection {
        Collection { kind: Kind::Contacts, account: "a".into(), id: "contacts".into(), dir: PathBuf::from("/tmp"), name: "Contacts".into(), color: None, read_only: false, components: vec![] }
    }

    #[test]
    fn reads_the_fields_people_use() {
        let c = contact(&lines::unfold(CARD), Path::new("/tmp/jane.vcf"), &book());
        assert_eq!(c.name, "Jane Exemple");
        assert_eq!(c.emails, vec![Labeled { label: "work".into(), value: "jane@example.org".into() }, Labeled { label: "home".into(), value: "jane@example.net".into() }]);
        assert_eq!(c.phones[0].label, "cell");
        assert_eq!(c.org, "Exemple SARL");
        assert_eq!(c.addresses[0].value, "1 rue de l'Exemple\n69000 Lyon\nFrance");
        assert_eq!(c.birthday, "1984-05-12");
        assert_eq!(c.notes, "Met at the market");
    }

    #[test]
    fn editing_keeps_what_sioul_does_not_show() {
        let edit = ContactEdit {
            name: "Jane Martin".into(),
            emails: vec![Labeled { label: "work".into(), value: "jane@example.org".into() }],
            phones: vec![],
            addresses: vec![Labeled { label: "home".into(), value: "2 place du Square\n69000 Lyon\nFrance".into() }],
            ..ContactEdit::default()
        };
        let text = apply(CARD, &edit);
        assert!(text.contains("X-OTHER-APP:keep me") && text.contains("PHOTO;ENCODING=b;TYPE=JPEG:AAAA"), "{text}");
        assert!(text.contains("FN:Jane Martin") && text.contains("N:Martin;Jane;;;"), "{text}");
        assert!(!text.contains("jane@example.net") && !text.contains("TEL"), "{text}");
        let again = contact(&lines::unfold(&text), Path::new("/tmp/jane.vcf"), &book());
        assert_eq!(again.addresses[0].value, "2 place du Square\n69000 Lyon\nFrance");
        assert_eq!(again.addresses[0].label, "home");
        assert!(text.starts_with("BEGIN:VCARD\r\nVERSION:3.0"), "{text}");
    }

    /// The shapes found in real address books: PREF, VOICE, lower-case types, Apple's
    /// groups and "no year" birthdays, compact dates, every part of an address.
    const MIXED: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:mixed-1\r\nFN:Lou Exemple\r\nN:Exemple;Lou;;;\r\n\
        EMAIL;TYPE=PREF;PREF=1:lou@example.org\r\nEMAIL;TYPE=home:lou@example.net\r\nitem1.EMAIL:lou@example.com\r\n\
        item1.X-ABLabel:club\r\nTEL;TYPE=VOICE,PREF:+33 4 00 00 00 00\r\nTEL;TYPE=cell:+33 6 00 00 00 00\r\n\
        ADR;TYPE=HOME:BP 12;Bât. B;1 rue de l\\'Exemple;Lyon;Rhône;69000;France\r\n\
        BDAY;X-APPLE-OMIT-YEAR=1604:1604-05-12\r\nNOTE:Lunch\\, then the market\r\nEND:VCARD\r\n";

    #[test]
    fn french_addresses_split_where_they_should() {
        assert_eq!(address_parts("12 rue de l'Exemple\nCS 90000\n69000 Lyon Cedex 03"), vec!["", "", "12 rue de l'Exemple, CS 90000", "Lyon Cedex 03", "", "69000", ""]);
        assert_eq!(address_parts("5 place de l'Exemple\n69001 Lyon\nFrance")[6], "France");
        assert_eq!(tidy_address("12 rue de l'Exemple\nCS 90000\n69000 Lyon Cedex 03"), "12 rue de l'Exemple, CS 90000\n69000 Lyon Cedex 03");
    }

    #[test]
    fn an_unchanged_card_comes_back_as_it_was() {
        let c = contact(&lines::unfold(MIXED), Path::new("/tmp/m.vcf"), &book());
        assert_eq!(c.birthday, "--05-12");
        assert_eq!(c.emails.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(), ["", "home", ""]);
        assert_eq!(c.addresses[0].value, "BP 12\nBât. B\n1 rue de l'Exemple\n69000 Lyon\nRhône\nFrance");
        assert_eq!(c.notes, "Lunch, then the market");
        let same = ContactEdit {
            name: c.name.clone(),
            emails: c.emails.clone(),
            phones: c.phones.clone(),
            org: c.org.clone(),
            title: c.title.clone(),
            addresses: c.addresses.clone(),
            birthday: c.birthday.clone(),
            notes: c.notes.clone(),
            urls: c.urls.clone(),
            categories: Some(c.categories.clone()),
            parts: Some(NameParts { prefix: c.prefix.clone(), given: c.given.clone(), family: c.family.clone(), suffix: c.suffix.clone() }),
            book: None,
        };
        assert_eq!(apply(MIXED, &same), MIXED);
        let compact = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nBDAY;VALUE=DATE:19840512\r\nEND:VCARD\r\n";
        assert_eq!(contact(&lines::unfold(compact), Path::new("/tmp/c.vcf"), &book()).birthday, "1984-05-12");
    }

    #[test]
    fn broken_cards_and_values_written_as_they_are() {
        // A line without its colon, ending in an accent: read, not a crash.
        let broken = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:A\r\nNOTE;café\r\nTEL;CELL;é\r\nEND:VCARD\r\n";
        assert_eq!(contact(&lines::unfold(broken), Path::new("/tmp/b.vcf"), &book()).name, "A");
        // A line break in a web address or a birthday stays out of the card.
        let edit = ContactEdit { name: "A".into(), urls: vec!["https://example.org/\r\nEMAIL:x@example.org".into()], birthday: "1984-05-12\nNOTE:x".into(), ..ContactEdit::default() };
        let text = new_card(&edit);
        assert!(!text.contains("\nEMAIL") && !text.contains("\nNOTE") && text.contains("URL:https://example.org/EMAIL:x@example.org"), "{text}");
    }

    #[test]
    fn a_change_touches_only_its_line() {
        let c = contact(&lines::unfold(MIXED), Path::new("/tmp/m.vcf"), &book());
        let mut edit = ContactEdit { name: c.name.clone(), emails: c.emails.clone(), phones: c.phones.clone(), addresses: c.addresses.clone(), birthday: c.birthday.clone(), notes: c.notes.clone(), ..ContactEdit::default() };
        // The club address goes, the mobile becomes "work", a number comes.
        edit.emails.retain(|e| e.value != "lou@example.com");
        edit.phones[1].label = "work".into();
        edit.phones.push(Labeled { label: "home".into(), value: "+33 4 00 00 00 11".into() });
        let text = apply(MIXED, &edit);
        assert!(text.contains("EMAIL;TYPE=PREF;PREF=1:lou@example.org") && text.contains("EMAIL;TYPE=home:lou@example.net"), "{text}");
        assert!(!text.contains("item1.") && !text.contains("lou@example.com"), "{text}");
        assert!(text.contains("TEL;TYPE=VOICE,PREF:+33 4 00 00 00 00") && text.contains("TEL;TYPE=WORK:+33 6 00 00 00 00"), "{text}");
        assert!(text.contains("TEL;TYPE=HOME:+33 4 00 00 00 11") && text.contains("BDAY;X-APPLE-OMIT-YEAR=1604:1604-05-12"), "{text}");
        assert!(text.contains("ADR;TYPE=HOME:BP 12;Bât. B;") && text.contains("REV:"), "{text}");
    }

    /// A card named by its UID as an address (`uid_uri`), and found again by
    /// it (`ByUid`, `uid_named`): a UUID, a URN already, the form of a card
    /// an extension's door made, Sioul's own, characters a URI cannot hold.
    #[test]
    fn a_card_is_named_by_its_uid_as_an_address() {
        assert_eq!(uid_uri("f81d4fae-7dec-11d0-a765-00a0c91e6bf6"), "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6");
        assert_eq!(uid_uri(" urn:uuid:0E2A9C4B-1111-4222-8333-444455556666 "), "urn:uuid:0E2A9C4B-1111-4222-8333-444455556666", "a URN already, as it is");
        assert_eq!(uid_uri("bookings.k1.k2@extensions.sioul"), "urn:uuid:bookings.k1.k2@extensions.sioul");
        assert_eq!(uid_uri("a b\"c,d;e%f\u{e9}"), "urn:uuid:a%20b%22c%2Cd%3Be%25f%C3%A9", "nothing that ends a parameter, nothing a URI cannot hold");
        let card = |uid: &str| Contact { uid: uid.into(), ..Contact::default() };
        let contacts = vec![card("f81d4fae-7dec-11d0-a765-00a0c91e6bf6"), card("urn:uuid:0E2A9C4B-1111-4222-8333-444455556666"), card("bookings.k1.k2@extensions.sioul"), card("a b\"c,d;e%f\u{e9}"), card("sioul-1234"), card("")];
        let by = ByUid::of(&contacts);
        for (at, contact) in contacts.iter().enumerate().take(5) {
            assert_eq!(by.find(&uid_uri(&contact.uid)), Some(at), "{}", contact.uid);
            assert_eq!(uid_named(&contacts, &uid_uri(&contact.uid)), Some(contact.uid.as_str()));
        }
        assert_eq!(by.find("URN:UUID:F81D4FAE-7DEC-11D0-A765-00A0C91E6BF6"), Some(0), "a UUID's letters, whatever their case");
        assert_eq!(by.find("urn:uuid:0e2a9c4b-1111-4222-8333-444455556666"), Some(1), "a card whose UID is the URN");
        for unknown in ["urn:uuid:9b2d0c4e-0000-4000-8000-000000000000", "urn:uuid:", "sioul:contact/sioul-1234", "https://example.org/sioul-1234", ""] {
            assert_eq!(by.find(unknown), None, "{unknown}");
            assert_eq!(uid_named(&contacts, unknown), None, "{unknown}");
        }
    }

    /// A task's contact as it is written since 26 November 2026
    /// (`altrep_of`: `urn:uuid:`, the door's encoding) and read in both
    /// forms (`card_key`, `names_card`, `same_card`, `card_named`,
    /// `card_address`): Sioul's own `sioul:contact/<UID>` of older tasks
    /// names the same card as its `urn:uuid:` form.
    #[test]
    fn a_card_is_named_either_way() {
        assert_eq!(altrep_of("sioul:contact/f81d4fae-7dec-11d0-a765-00a0c91e6bf6"), "urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6");
        assert_eq!(altrep_of(&crate::links::contact_uri("a b\"c,d;e%f\u{e9}")), uid_uri("a b\"c,d;e%f\u{e9}"), "the door's encoding, the UID decoded first");
        assert_eq!(altrep_of(&crate::links::contact_uri("urn:uuid:0E2A9C4B-1111-4222-8333-444455556666")), "urn:uuid:0E2A9C4B-1111-4222-8333-444455556666", "a URN already, as it is");
        for other in ["urn:uuid:f81d4fae-7dec-11d0-a765-00a0c91e6bf6", "mailto:office@example.org", "sioul:contact/", ""] {
            assert_eq!(altrep_of(other), other, "{other}");
        }
        let card = |uid: &str| Contact { uid: uid.into(), ..Contact::default() };
        let contacts = vec![card("f81d4fae-7dec-11d0-a765-00a0c91e6bf6"), card("urn:uuid:0E2A9C4B-1111-4222-8333-444455556666"), card("a b\"c,d;e%f\u{e9}"), card("sioul-1234")];
        for contact in &contacts {
            let (own, urn) = (crate::links::contact_uri(&contact.uid), altrep_of(&crate::links::contact_uri(&contact.uid)));
            assert!(names_card(&own, &contact.uid) && names_card(&urn, &contact.uid), "{own} {urn}");
            assert!(same_card(&own, &urn) && same_card(&urn, &own), "{own} {urn}");
            assert_eq!(card_named(&contacts, &own).map(|c| c.uid.as_str()), Some(contact.uid.as_str()), "{own}");
            assert_eq!(card_named(&contacts, &urn).map(|c| c.uid.as_str()), Some(contact.uid.as_str()), "{urn}");
            assert_eq!(card_address(&contacts, &urn), Some(own.clone()), "{urn}");
            assert_eq!(card_address(&contacts, &own), Some(own), "as it is");
        }
        assert!(names_card("URN:UUID:F81D4FAE-7DEC-11D0-A765-00A0C91E6BF6", "f81d4fae-7dec-11d0-a765-00a0c91e6bf6"), "a UUID's letters, whatever their case");
        assert!(!same_card("sioul:contact/sioul-1234", "urn:uuid:sioul-9999") && !names_card("sioul:contact/sioul-1234", "sioul-9999"));
        assert!(!same_card("mailto:a@example.org", "mailto:a@example.org"), "an address of no card names none");
        // A card not on this device: its address all the same, from the UID the address gives.
        assert_eq!(card_address(&contacts, "urn:uuid:9b2d0c4e-0000-4000-8000-000000000000"), Some("sioul:contact/9b2d0c4e-0000-4000-8000-000000000000".into()));
        assert_eq!(card_address(&contacts, "sioul:contact/gone"), Some("sioul:contact/gone".into()));
        for none in ["urn:uuid:", "urn:isbn:0-486-27557-4", "https://example.org/sioul-1234", "sioul:contact/", ""] {
            assert_eq!(card_address(&contacts, none), None, "{none}");
            assert_eq!(card_named(&contacts, none).map(|c| c.uid.as_str()), None, "{none}");
        }
    }

    #[test]
    fn new_cards_and_completion() {
        let text = new_card(&ContactEdit { name: "Paul Exemple".into(), emails: vec![Labeled { label: String::new(), value: "paul@example.org".into() }], ..ContactEdit::default() });
        assert!(parse(&text).is_some());
        let card = lines::unfold(&text);
        assert!(text.contains("UID:sioul-") && text.contains("FN:Paul Exemple") && text.contains("EMAIL;TYPE=INTERNET:paul@example.org"), "{text}");
        let contacts = vec![contact(&card, Path::new("/tmp/p.vcf"), &book()), contact(&lines::unfold(CARD), Path::new("/tmp/j.vcf"), &book())];
        assert_eq!(completions(&contacts, "pa", 5), vec!["Paul Exemple <paul@example.org>"]);
        assert_eq!(completions(&contacts, "exe", 5).len(), 3);
        assert_eq!(search(&contacts, "SARL").len(), 1);
        assert_eq!(by_address(&contacts, "JANE@example.net").map(|c| c.name.as_str()), Some("Jane Exemple"));
    }

    /// The form's values for a contact, nothing changed.
    fn edit_of(c: &Contact) -> ContactEdit {
        ContactEdit {
            name: c.name.clone(),
            emails: c.emails.clone(),
            phones: c.phones.clone(),
            org: c.org.clone(),
            title: c.title.clone(),
            addresses: c.addresses.clone(),
            birthday: c.birthday.clone(),
            notes: c.notes.clone(),
            urls: c.urls.clone(),
            categories: Some(c.categories.clone()),
            parts: Some(NameParts { prefix: c.prefix.clone(), given: c.given.clone(), family: c.family.clone(), suffix: c.suffix.clone() }),
            book: None,
        }
    }

    #[test]
    fn categories_as_nextcloud_writes_them() {
        // Two lines, an escaped comma, one name twice in another case: one list.
        let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:c-1\r\nFN:Lou Exemple\r\nCATEGORIES:Amis,Famille\r\nX-OTHER:keep\r\nCATEGORIES:amis,Club\\, échecs\r\nEND:VCARD\r\n";
        let c = contact(&lines::unfold(card), Path::new("/tmp/c.vcf"), &book());
        assert_eq!(c.categories, ["Amis", "Famille", "Club, échecs"]);
        // Saved as they are: the card untouched, both lines kept; left out: untouched too.
        let mut edit = edit_of(&c);
        assert_eq!(apply(card, &edit), card);
        edit.categories = None;
        assert_eq!(apply(card, &edit), card);
        // One taken off, one added: one line where the first was, its comma escaped, the rest kept.
        edit.categories = Some(vec!["Famille".into(), "Club, échecs".into(), "Voisins".into(), " famille ".into()]);
        let text = apply(card, &edit);
        assert!(text.contains("\r\nCATEGORIES:Famille,Club\\, échecs,Voisins\r\nX-OTHER:keep\r\n") && text.matches("CATEGORIES").count() == 1, "{text}");
        assert_eq!(contact(&lines::unfold(&text), Path::new("/tmp/c.vcf"), &book()).categories, ["Famille", "Club, échecs", "Voisins"]);
        // All taken off: no line. A new card: one line, each name once.
        edit.categories = Some(Vec::new());
        assert!(!apply(card, &edit).contains("CATEGORIES"));
        let new = new_card(&ContactEdit { name: "Paul".into(), categories: Some(vec!["Amis".into(), " amis ".into()]), ..ContactEdit::default() });
        assert!(new.contains("\r\nCATEGORIES:Amis\r\n"), "{new}");
        // Compared folded, as the sender lists compare them.
        assert_eq!(category_key(" AMIS "), category_key("amis"));
        assert_eq!(category_key("Équipe"), "equipe");
        // The list filters by them: the most used first, each name once, as first written.
        let with = |file: &str, names: &str| contact(&lines::unfold(&format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:{file}\r\nCATEGORIES:{names}\r\nEND:VCARD\r\n")), Path::new(file), &book());
        let everyone = [with("a", "Famille,Club"), with("b", "famille"), with("c", "Équipe"), with("d", "equipe,FAMILLE")];
        assert_eq!(categories_in_use(&everyone), ["Famille", "Équipe", "Club"]);
        assert_eq!(everyone.iter().filter(|c| in_category(c, "FAMILLE")).count(), 3);
        assert!(in_category(&everyone[3], "Équipe") && !in_category(&everyone[1], "Club"));
    }

    /// A card of the parts given (wave D): `N` written from them, the name
    /// shown its own; a name changed leaves typed parts as they are, and
    /// guesses again only the parts of a card that had none of its own.
    #[test]
    fn name_parts_typed_are_kept() {
        let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:p-1\r\nFN:Jeanne Martin\r\nN:Martin-Exemple;Jeanne;Marie;Dr;PhD\r\nEND:VCARD\r\n";
        let c = contact(&lines::unfold(card), Path::new("/tmp/p.vcf"), &book());
        assert_eq!((c.prefix.as_str(), c.given.as_str(), c.additional.as_str(), c.family.as_str(), c.suffix.as_str()), ("Dr", "Jeanne", "Marie", "Martin-Exemple", "PhD"));
        assert_eq!(c.place, "a/contacts");
        // Renamed, parts as they were: kept byte for byte, never rebuilt from the last word.
        let mut edit = edit_of(&c);
        edit.name = "Jeanne M.".into();
        let text = apply(card, &edit);
        assert!(text.contains("\r\nN:Martin-Exemple;Jeanne;Marie;Dr;PhD\r\n") && text.contains("FN:Jeanne M."), "{text}");
        // Left out (an import): the same.
        edit.parts = None;
        assert!(apply(card, &edit).contains("\r\nN:Martin-Exemple;Jeanne;Marie;Dr;PhD\r\n"));
        // Typed: written as typed, additional names kept where they were.
        edit.parts = Some(NameParts { prefix: "Mme".into(), given: "Jeanne".into(), family: "Martin".into(), suffix: String::new() });
        let text = apply(card, &edit);
        assert!(text.contains("\r\nN:Martin;Jeanne;Marie;Mme;\r\n") && text.matches("\nN:").count() == 1, "{text}");
        // A card of no parts: guessed from its name, a title at the start in the title part.
        let bare = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:p-2\r\nFN:Lou\r\nEND:VCARD\r\n";
        let edit = ContactEdit { name: "Mme Jeanne Martin".into(), parts: Some(NameParts::default()), ..ContactEdit::default() };
        assert!(apply(bare, &edit).contains("\r\nN:Martin;Jeanne;;Mme;\r\n"));
        // Sioul's own guess of the former name follows the new one, its former way too.
        let guessed = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:p-3\r\nFN:Mme Lou Exemple\r\nN:Exemple;Mme Lou;;;\r\nEND:VCARD\r\n";
        let c = contact(&lines::unfold(guessed), Path::new("/tmp/g.vcf"), &book());
        let mut edit = edit_of(&c);
        edit.name = "Lou Martin".into();
        assert!(apply(guessed, &edit).contains("\r\nN:Martin;Lou;;;\r\n"), "{}", apply(guessed, &edit));
        // A new card of its parts alone: its name is theirs, without the title.
        let parts = NameParts { prefix: "Mme".into(), given: "Jeanne".into(), family: "Martin".into(), suffix: String::new() };
        let new = new_card(&ContactEdit { parts: Some(parts), ..ContactEdit::default() });
        assert!(new.contains("FN:Jeanne Martin\r\n") && new.contains("N:Martin;Jeanne;;Mme;\r\n"), "{new}");
        let shown = contact(&lines::unfold(&new), Path::new("/tmp/n.vcf"), &book());
        assert_eq!((title_before_name(&shown).as_str(), has_title(&shown)), ("Mme", true));
    }

    /// "Move the title into its field" (wave D): the title found at the start
    /// of the name goes into `N`'s prefix, the name shown loses it; `TITLE`,
    /// the job, and every other line stay.
    #[test]
    fn the_title_moves_into_its_field() {
        let card = |fn_line: &str, n: &str| format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:t-1\r\n{fn_line}\r\n{n}TITLE:Accountant\r\nX-OTHER:keep\r\nEND:VCARD\r\n");
        // Sioul's former guess: the title among the given names, taken out there too.
        let TitleMove::Moves { title, before, after, kept, sure, text } = move_title(&card("FN:Mme Jeanne Martin", "N:Martin;Mme Jeanne;;;\r\n")) else { panic!("found") };
        assert_eq!((title.as_str(), before.as_str(), after.as_str(), kept, sure), ("Mme", "Mme Jeanne Martin", "Jeanne Martin", false, true));
        assert!(text.contains("FN:Jeanne Martin\r\n") && text.contains("N:Martin;Jeanne;;Mme;\r\n") && text.contains("TITLE:Accountant\r\nX-OTHER:keep\r\n") && text.contains("REV:"), "{text}");
        let c = contact(&lines::unfold(&text), Path::new("/tmp/t.vcf"), &book());
        assert_eq!((c.name.as_str(), c.prefix.as_str(), c.title.as_str(), title_before_name(&c).as_str()), ("Jeanne Martin", "Mme", "Accountant", "Mme"));
        // No N: one guessed from the name left, with the title.
        let TitleMove::Moves { text, .. } = move_title(&card("FN:Mr. John Example", "")) else { panic!("found") };
        assert!(text.contains("FN:John Example\r\n") && text.contains("N:Example;John;;Mr.;\r\n"), "{text}");
        // N holds that title already: only the name cleaned, N byte for byte.
        let TitleMove::Moves { kept, text, .. } = move_title(&card("FN:Dr Paul Exemple", "N:Exemple;Paul;;dr.;\r\n")) else { panic!("found") };
        assert!(kept && text.contains("FN:Paul Exemple\r\n") && text.contains("N:Exemple;Paul;;dr.;\r\n"), "{text}");
        // Another title held there: left as it is. No title, or none to clean: nothing.
        assert_eq!(move_title(&card("FN:Mme Lou Exemple", "N:Exemple;Lou;;Dr;\r\n")), TitleMove::Differs("Dr".into()));
        assert_eq!(move_title(&card("FN:Meredith Example", "")), TitleMove::None);
        assert_eq!(move_title(&card("FN:Drake Example", "")), TitleMove::None);
        assert_eq!(move_title(&card("FN:Mme", "")), TitleMove::None);
        assert_eq!(move_title("BEGIN:VCARD\r\nVERSION:3.0\r\nN:Martin;Jeanne;;Mme;\r\nEND:VCARD\r\n"), TitleMove::None);
        // The filter's "has a title": in its field, or at the start of its name.
        let with = |name: &str, prefix: &str| Contact { name: name.into(), prefix: prefix.into(), ..Contact::default() };
        assert!(has_title(&with("Mme Jeanne Martin", "")) && has_title(&with("Jeanne Martin", "Mme")) && !has_title(&with("Meredith Example", "")));
        assert_eq!(title_before_name(&with("Mme Jeanne Martin", "Mme")), "", "never twice");
    }

    /// The title's move (review F18): a name of two people is never changed;
    /// a title is sure with its full stop, or a word that is no other on a
    /// person's card, never on a company's; the name's lines keep their
    /// parameters and their lists of names.
    #[test]
    fn the_title_s_move_keeps_what_the_name_lines_carry() {
        let card = |fn_line: &str, extra: &str| format!("BEGIN:VCARD\r\nVERSION:3.0\r\nUID:t-2\r\nFN:{fn_line}\r\n{extra}END:VCARD\r\n");
        for household in ["M. et Mme Dupont", "Mr and Mrs Smith", "M & Mme Dupont", "Mme Jeanne et Paul Martin", "Mme Dupont, M. Martin", "Mme Jeanne / M. Paul Martin"] {
            assert_eq!(move_title(&card(household, "N:Dupont;;;;\r\n")), TitleMove::Household, "{household}");
        }
        let sure = |fn_line: &str, extra: &str| match move_title(&card(fn_line, extra)) {
            TitleMove::Moves { sure, .. } => Some(sure),
            _ => None,
        };
        let person = "N:Martin;Jeanne;;;\r\n";
        assert_eq!(sure("Mme Jeanne Martin", person), Some(true));
        assert_eq!(sure("Madame Jeanne Martin", person), Some(true));
        assert_eq!(sure("M. Paul Exemple", person), Some(true));
        assert_eq!(sure("Dr Paul Exemple", person), Some(true));
        assert_eq!(sure("Me Work", person), Some(false), "\"me\" in English");
        assert_eq!(sure("PR Agency Example", person), Some(false));
        assert_eq!(sure("M Pokora", person), Some(false), "an initial");
        assert_eq!(sure("Dr Martens Store", "N:;;;;\r\nORG:Dr Martens Store\r\n"), Some(false), "a company's card");
        assert_eq!(sure("Miss Paris Institut", "ORG:Miss Paris Institut\r\n"), Some(false));
        assert_eq!(sure("Mme Jeanne Martin", ""), Some(true), "no name's parts, a title that is nothing else");
        // Review V7: a full stop never ticks a company; a card that only carries the guess of its name tells nothing.
        assert_eq!(sure("Dr. Martens Store", "N:;;;;\r\nORG:Dr. Martens Store\r\n"), Some(false));
        assert_eq!(sure("Mr Bricolage Lyon", "N:Lyon;Mr Bricolage;;;\r\n"), Some(false));
        assert_eq!(sure("Dr Martens Store", "N:Store;Dr Martens;;;\r\n"), Some(false));
        assert_eq!(sure("M. Night Exemple", "N:Exemple;M. Night;;;\r\n"), Some(true));
        // FN's and N's parameters, and N's lists of names, as they were.
        let four = "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:t-4\r\nFN;LANGUAGE=fr:Dr. Paul Exemple\r\nN;SORT-AS=\"Exemple,Paul\":Exemple;Paul;Jean,Marie;;Jr.,M.D.\r\nEND:VCARD\r\n";
        let TitleMove::Moves { sure, text, .. } = move_title(four) else { panic!("found") };
        assert!(sure);
        assert!(text.contains("FN;LANGUAGE=fr:Paul Exemple\r\n") && text.contains("N;SORT-AS=\"Exemple,Paul\":Exemple;Paul;Jean,Marie;Dr.;Jr.,M.D.\r\n"), "{text}");
    }

    /// The name's parts follow a rename where they are Sioul's guess (review
    /// F19): after the title's move too, the title and the additional names kept.
    #[test]
    fn a_rename_reaches_the_name_s_parts() {
        let moved = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:r-1\r\nFN:Jeanne Martin\r\nN:Martin;Jeanne;;Mme;\r\nEND:VCARD\r\n";
        let c = contact(&lines::unfold(moved), Path::new("/tmp/r.vcf"), &book());
        let mut edit = edit_of(&c);
        edit.name = "Jeanne Dupont".into();
        let text = apply(moved, &edit);
        assert!(text.contains("FN:Jeanne Dupont\r\n") && text.contains("\r\nN:Dupont;Jeanne;;Mme;\r\n"), "{text}");
        // Another program's N with a middle name: it follows, the middle name kept.
        let middle = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:r-2\r\nFN:Jeanne Martin\r\nN:Martin;Jeanne;Marie;;\r\nEND:VCARD\r\n";
        let c = contact(&lines::unfold(middle), Path::new("/tmp/m.vcf"), &book());
        let mut edit = edit_of(&c);
        edit.name = "Jeanne Dupont".into();
        let text = apply(middle, &edit);
        assert!(text.contains("\r\nN:Dupont;Jeanne;Marie;;\r\n"), "{text}");
        // Parts of its own (not the guess of its name): kept whatever the name becomes.
        let own = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:r-3\r\nFN:Jeanne\r\nN:Martin-Exemple;Jeanne;;;\r\nEND:VCARD\r\n";
        let c = contact(&lines::unfold(own), Path::new("/tmp/o.vcf"), &book());
        let mut edit = edit_of(&c);
        edit.name = "Jeanne D.".into();
        assert!(apply(own, &edit).contains("\r\nN:Martin-Exemple;Jeanne;;;\r\n"));
    }

    /// Groups added and taken out by the batch actions: one CATEGORIES line,
    /// the rest of the card as it was; nothing when nothing changes.
    #[test]
    fn groups_added_and_taken_out() {
        let card = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:g-1\r\nFN:Lou\r\nCATEGORIES:Amis,Clients\r\nX-OTHER:keep\r\nEND:VCARD\r\n";
        let text = with_categories(card, &["Professional".into()], &["amis".into()]).expect("changed");
        assert!(text.contains("CATEGORIES:Clients,Professional\r\nX-OTHER:keep\r\n"), "{text}");
        assert_eq!(with_categories(card, &["CLIENTS".into()], &["Famille".into()]), None, "has it, lacks the other: nothing");
        let none = with_categories(card, &[], &["Amis".into(), "clients".into()]).expect("changed");
        assert!(!none.contains("CATEGORIES"), "{none}");
    }

    /// The book a new card's form proposes (20 November 2026): the one the
    /// list is filtered on when it is one and can be written, else the last
    /// one a card was made in on this device while it can be written, else
    /// the first; nothing when no book can be written. The form's choice
    /// travels in the edit, by place; an older form's edit, without it, reads.
    #[test]
    fn the_book_a_new_card_goes_into() {
        let writable = ["cloud/contacts".to_string(), "cloud/team".to_string(), "local/family".to_string()];
        assert_eq!(book_for_new(&writable, &[], "").as_deref(), Some("cloud/contacts"), "the first");
        assert_eq!(book_for_new(&writable, &[], "local/family").as_deref(), Some("local/family"), "the last one used");
        assert_eq!(book_for_new(&writable, &["cloud/team".into()], "local/family").as_deref(), Some("cloud/team"), "the filter's, before the last");
        assert_eq!(book_for_new(&writable, &["cloud/team".into(), "local/family".into()], "").as_deref(), Some("cloud/contacts"), "two books shown: neither");
        assert_eq!(book_for_new(&writable, &["google/shared".into()], "local/family").as_deref(), Some("local/family"), "a book shown that cannot be written: the last");
        assert_eq!(book_for_new(&writable, &[], "cloud/gone").as_deref(), Some("cloud/contacts"), "the last one gone: the first");
        assert_eq!(book_for_new(&[], &["cloud/team".into()], "cloud/team"), None, "nothing to write into");
        let chosen: ContactEdit = serde_json::from_str(r#"{"name": "Ada Exemple", "book": "cloud/team"}"#).unwrap();
        assert_eq!(chosen.book.as_deref(), Some("cloud/team"));
        let older: ContactEdit = serde_json::from_str(r#"{"name": "Ada Exemple"}"#).unwrap();
        assert_eq!(older.book, None);
    }
}
