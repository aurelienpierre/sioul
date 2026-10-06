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
    /// The address book it is in.
    pub book: String,
    pub read_only: bool,
    /// Its CATEGORIES ("Famille", "Amis"…), what Nextcloud Contacts shows as
    /// groups: every CATEGORIES line of the card, each name once
    /// (`category_key`), as the card writes it. The page shows them and
    /// filters by them, and the sender lists can name them
    /// (`category:Amis`, porch.rs's `Senders`): their mail then stands as
    /// their category says, unless an address of theirs has its own entry.
    pub categories: Vec<String>,
    /// Its picture to show: an embedded one as a file in the cache ("file://…"),
    /// else the web address the card gives; empty without one.
    pub photo: String,
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
    let all = |property: &'static str| card.iter().filter(move |l| lines::name(l) == property);
    let first = |property: &'static str| all(property).next().map(|l| shown(l)).unwrap_or_default();
    let labeled = |property: &'static str| {
        all(property).map(|l| Labeled { label: label_of(&split_line(l)), value: shown(l) }).filter(|l| !l.value.is_empty()).collect::<Vec<_>>()
    };
    let emails = labeled("EMAIL");
    let org = first("ORG");
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
        read_only: book.read_only,
        photo: photo_of(card),
    }
}

/// A card's picture, to show: an embedded photo (vCard 3.0 `ENCODING=b`,
/// vCard 4.0 `data:` address) written once into the cache, named by what it
/// holds, so the same picture is one file; a web address (https only) as it is.
fn photo_of(card: &[String]) -> String {
    photo_in(card, &crate::config::cache_dir().join("photos"))
}

fn photo_in(card: &[String], folder: &Path) -> String {
    let Some(line) = card.iter().find(|l| lines::name(l) == "PHOTO") else { return String::new() };
    let parsed = split_line(line);
    let value = parsed.value.trim();
    let embedded = parsed.params.iter().any(|(k, v)| k == "ENCODING" && (v.eq_ignore_ascii_case("b") || v.eq_ignore_ascii_case("base64")));
    let (bytes, kind) = match value.strip_prefix("data:").and_then(|d| d.split_once(";base64,")) {
        Some((media, data)) => (lines::base64_decode(data), media.rsplit('/').next().unwrap_or("jpeg").to_ascii_lowercase()),
        None if embedded => (lines::base64_decode(value), parsed.params.iter().find(|(k, _)| k == "TYPE").map_or("jpeg".to_string(), |(_, v)| v.to_ascii_lowercase())),
        None if value.starts_with("https://") => return value.to_string(),
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
    let name = edit.name.trim();
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
        if property == "FN" {
            // The structured name follows a new name.
            for (i, line) in original.iter().enumerate() {
                if lines::name(line) == "N" {
                    out[i] = None;
                }
            }
            added.push(format!("N:{}", lines::structured(&structured_name(name))));
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

/// "Jane van Exemple" → N: family "van Exemple"? Names do not split by rule;
/// the last word is taken as the family name, the rest as given names.
fn structured_name(name: &str) -> Vec<String> {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.split_last() {
        Some((family, given)) if !given.is_empty() => vec![family.to_string(), given.join(" "), String::new(), String::new(), String::new()],
        _ => vec![name.to_string(), String::new(), String::new(), String::new(), String::new()],
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
    vdir::collections(Kind::Contacts).into_iter().find(|b| !b.read_only)
}

/// The file a new contact gets in an address book.
pub fn new_path(book: &Collection) -> PathBuf {
    book.dir.join(format!("{}.vcf", vdir::new_name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_photo_to_show() {
        let card = |photo: &str| lines::unfold(&format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Jane\r\n{photo}\r\nEND:VCARD\r\n"));
        let folder = std::env::temp_dir().join(format!("sioul-photos-{}", std::process::id()));
        let photo_of = |card: &[String]| photo_in(card, &folder);
        let three = photo_of(&card("PHOTO;ENCODING=b;TYPE=PNG:aGVsbG8="));
        let four = photo_of(&card("PHOTO:data:image/png;base64,aGVsbG8="));
        assert!(three.starts_with("file:///") && three.ends_with(".png"), "{three}");
        // The same picture, one file.
        assert_eq!(three, four);
        let written = folder.join(format!("{}.png", vdir::content_hash(b"hello")));
        assert_eq!((three.clone(), std::fs::read(&written).unwrap()), (crate::notes::file_url(&written), b"hello".to_vec()));
        assert_eq!(photo_of(&card("PHOTO;VALUE=uri:https://example.org/jane.jpg")), "https://example.org/jane.jpg");
        assert_eq!(photo_of(&card("PHOTO;VALUE=uri:http://example.org/jane.jpg")), "");
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
}
