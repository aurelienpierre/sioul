// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Duplicates in the address books, looked for and cleaned only when you ask:
//! a number or an address written twice on one card, and two cards that may
//! be one person. Nothing changes without your click, and every change keeps
//! the cards as they were, to put them back.
//!
//! - **On one card**: numbers are compared by their international form
//!   (`phones::key`: "04 65 71 12 34" is "+33 4 65 71 12 34"), e-mail
//!   addresses case aside. Of the same value written twice, the line kept is
//!   the one written internationally, else the one that says most of what it
//!   is for; it takes what the others said ("mobile", "work", to use first),
//!   and its value is never rewritten.
//! - **Two cards**: the same number, the same e-mail address, or the same name
//!   (case, accents, punctuation and the order of words aside: "DUPONT, Jean"
//!   is "Jean Dupont"). What many cards share (a switchboard, an office's
//!   address, a common name) says nothing of one person. "Not the same" is
//!   remembered, never asked again.
//! - **Merging** keeps the card whose name you keep as it is and adds what
//!   only the other one has: numbers, e-mail and postal addresses, web sites,
//!   categories, chat addresses; notes joined; a photo, an organisation, a
//!   birthday when it has none. The other card is deleted, and the sync
//!   deletes it on the server.
//! - **Undo**: before any change the cards as they were are kept in
//!   `$XDG_STATE_HOME/sioul/contacts-undo`, thirty days; undoing writes them
//!   back, and the sync sends them again.

use crate::contacts::{self, Contact};
use crate::lines;
use crate::phones::{self, Region};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A value a cleaning takes off, and the one kept for it, as the card writes them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Removed {
    /// "phone" or "email".
    pub kind: String,
    pub value: String,
    pub kept: String,
}

/// An e-mail address as compared: trimmed, without "mailto:", case aside.
pub fn email_key(value: &str) -> String {
    let value = value.trim();
    let value = value.get(..7).filter(|s| s.eq_ignore_ascii_case("mailto:")).map_or(value, |_| &value[7..]);
    value.trim().to_lowercase()
}

/// What two names are compared by: case, accents, punctuation and the order
/// of words aside ("DUPONT, Jean" is "Jean Dupont", "Zoë" is "Zoe").
pub fn name_key(name: &str) -> String {
    let mut folded = String::new();
    for c in name.chars() {
        match c {
            'ß' => folded.push_str("ss"),
            'æ' | 'Æ' => folded.push_str("ae"),
            'œ' | 'Œ' => folded.push_str("oe"),
            _ => folded.push(latin(crate::text::fold_char(c))),
        }
    }
    let mut words: Vec<&str> = folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    words.sort_unstable();
    words.join(" ")
}

/// Latin letters `text::fold_char` leaves with their marks (it knows French's).
fn latin(c: char) -> char {
    match c {
        'ñ' | 'ń' | 'ň' => 'n',
        'ã' | 'å' | 'ā' | 'ą' | 'ă' => 'a',
        'õ' | 'ø' | 'ō' | 'ő' => 'o',
        'ł' | 'ľ' | 'ĺ' => 'l',
        'ś' | 'š' | 'ş' | 'ș' => 's',
        'ć' | 'č' => 'c',
        'ź' | 'ż' | 'ž' => 'z',
        'ř' => 'r',
        'ě' | 'ę' | 'ē' => 'e',
        'ů' | 'ű' | 'ū' => 'u',
        'ý' => 'y',
        'ğ' => 'g',
        'ı' | 'ī' => 'i',
        'đ' | 'ď' => 'd',
        'ť' | 'ț' => 't',
        _ => c,
    }
}

/// A line's group as written ("item1" of "item1.TEL;…"); none without one.
fn group_of(line: &str) -> Option<&str> {
    let head = line.split([';', ':']).next().unwrap_or("");
    head.rsplit_once('.').map(|(group, _)| group)
}

fn same_group(line: &str, group: &str) -> bool {
    group_of(line).is_some_and(|g| g.eq_ignore_ascii_case(group))
}

/// Whether a line only says something of another line of its group: Apple's
/// labels and address formats (`item1.X-ABLabel`), not its related names and dates.
fn is_companion(line: &str) -> bool {
    let name = lines::name(line);
    name.starts_with("X-AB") && name != "X-ABRELATEDNAMES" && name != "X-ABDATE"
}

/// Whether a kind says what a value is for: "voice", "internet" and "pref" do not.
fn informative(kind: &str) -> bool {
    !matches!(kind, "internet" | "voice" | "pref" | "x400") && !kind.starts_with("x-")
}

/// How much a value's line says of what it is for: its kinds ("work",
/// "cell"), a label of its own (Apple's), whether it comes first.
fn says(card: &[String], i: usize) -> usize {
    let line = contacts::split_line(&card[i]);
    let kinds = contacts::types_of(&line).iter().filter(|k| informative(k)).count();
    let labelled = group_of(&card[i]).is_some_and(|g| card.iter().any(|l| same_group(l, g) && is_companion(l)));
    kinds + usize::from(labelled) + usize::from(contacts::is_preferred(&line))
}

/// The line with more kinds in its TYPE and, when asked, marked as the one
/// to use first; its value as it was. Kinds are written in the case its TYPE
/// uses, else upper case in vCard 3.0 and lower case in 4.0.
fn with_kinds(line: &str, kinds: &[String], first: bool, version4: bool) -> String {
    if kinds.is_empty() && !first {
        return line.to_string();
    }
    let value = lines::value(line);
    let Some(colon) = line.len().checked_sub(value.len() + 1).filter(|&at| line.as_bytes().get(at) == Some(&b':')) else { return line.to_string() };
    let head = &line[..colon];
    // The head's parts, split at semicolons outside quotes.
    let mut parts: Vec<String> = Vec::new();
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in head.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                parts.push(head[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(head[start..].to_string());
    let mut kinds: Vec<String> = kinds.to_vec();
    if first && !version4 {
        kinds.push("pref".into());
    }
    let typed = parts.iter().position(|p| p.split_once('=').is_some_and(|(n, _)| n.trim().eq_ignore_ascii_case("TYPE")));
    if !kinds.is_empty() {
        match typed {
            Some(at) => {
                let (name, written) = parts[at].split_once('=').map(|(n, v)| (n.to_string(), v.to_string())).unwrap_or_default();
                let lower = written.chars().filter(|c| c.is_alphabetic()).all(char::is_lowercase);
                let more = kinds.iter().map(|k| if lower { k.to_lowercase() } else { k.to_uppercase() }).collect::<Vec<_>>().join(",");
                let joined = match written.strip_suffix('"') {
                    Some(open) => format!("{open},{more}\""),
                    None if written.is_empty() => more,
                    None => format!("{written},{more}"),
                };
                parts[at] = format!("{name}={joined}");
            }
            None => {
                let more = kinds.iter().map(|k| if version4 { k.to_lowercase() } else { k.to_uppercase() }).collect::<Vec<_>>().join(",");
                parts.insert(1, format!("TYPE={more}"));
            }
        }
    }
    if first && version4 {
        parts.push("PREF=1".into());
    }
    format!("{}:{value}", parts.join(";"))
}

/// A card's values written twice taken off: the same number (`phones::key`
/// with `region`), the same e-mail address. Returns its new text, REV
/// renewed, and what goes; none when nothing is there twice.
pub fn clean(text: &str, region: Option<&Region>) -> Option<(String, Vec<Removed>)> {
    let card = lines::unfold(text);
    let version4 = card.iter().any(|l| l.eq_ignore_ascii_case("VERSION:4.0"));
    let mut out: Vec<Option<String>> = card.iter().cloned().map(Some).collect();
    let mut removed: Vec<Removed> = Vec::new();
    let mut gone: Vec<usize> = Vec::new();
    for (property, kind) in [("TEL", "phone"), ("EMAIL", "email")] {
        // Each value's lines, in the card's order.
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, line) in card.iter().enumerate().filter(|(_, l)| lines::name(l) == property) {
            let value = contacts::shown(line);
            if value.is_empty() {
                continue;
            }
            let key = if property == "TEL" { phones::key(&value, region) } else { email_key(&value) };
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, members)) => members.push(i),
                None => groups.push((key, vec![i])),
            }
        }
        for (_, members) in groups.into_iter().filter(|(_, m)| m.len() > 1) {
            // Written internationally first, then what says most, then the first.
            let keeper = members
                .iter()
                .copied()
                .max_by_key(|&i| (property == "TEL" && phones::is_international(&contacts::shown(&card[i])), says(&card, i), std::cmp::Reverse(i)))
                .unwrap_or(members[0]);
            let kept_line = contacts::split_line(&card[keeper]);
            let kept_kinds = contacts::types_of(&kept_line);
            let mut more: Vec<String> = Vec::new();
            for &i in members.iter().filter(|&&i| i != keeper) {
                for kind in contacts::types_of(&contacts::split_line(&card[i])) {
                    if informative(&kind) && !kept_kinds.contains(&kind) && !more.contains(&kind) {
                        more.push(kind);
                    }
                }
            }
            let first = !contacts::is_preferred(&kept_line) && members.iter().any(|&i| contacts::is_preferred(&contacts::split_line(&card[i])));
            let mut line = with_kinds(&card[keeper], &more, first, version4);
            // Kept without a group, it takes a label of its own from a line that goes (Apple's).
            if group_of(&card[keeper]).is_none()
                && let Some(group) = members.iter().filter(|&&i| i != keeper).find_map(|&i| group_of(&card[i]).filter(|g| card.iter().any(|l| same_group(l, g) && is_companion(l))))
            {
                line = format!("{group}.{line}");
            }
            let kept_value = contacts::shown(&card[keeper]);
            for &i in members.iter().filter(|&&i| i != keeper) {
                out[i] = None;
                gone.push(i);
                removed.push(Removed { kind: kind.to_string(), value: contacts::shown(&card[i]), kept: kept_value.clone() });
            }
            out[keeper] = Some(line);
        }
    }
    if removed.is_empty() {
        return None;
    }
    // The labels of a line gone go with it, unless a kept line still has their group.
    for &i in &gone {
        let Some(group) = group_of(&card[i]) else { continue };
        let still_used = out.iter().flatten().any(|l| same_group(l, group) && !is_companion(l));
        if !still_used {
            for (j, line) in card.iter().enumerate() {
                if same_group(line, group) {
                    out[j] = None;
                }
            }
        }
    }
    Some((finish(out.into_iter().flatten().collect(), Vec::new()), removed))
}

/// The card's lines with `added` before END and REV renewed, folded.
fn finish(lines_kept: Vec<String>, added: Vec<String>) -> String {
    let mut kept: Vec<String> = lines_kept.into_iter().filter(|l| lines::name(l) != "REV").collect();
    let end = kept.iter().rposition(|l| lines::name(l) == "END").unwrap_or(kept.len());
    let mut more = added;
    more.push(format!("REV:{}", contacts::now_stamp()));
    kept.splice(end..end, more);
    lines::fold(&kept)
}

/// A card that holds a number or an address twice, and what cleaning it takes off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cleaning {
    /// Its file.
    pub key: String,
    pub name: String,
    pub removed: Vec<Removed>,
}

/// The cards holding a number or an address twice, as `contacts` orders
/// them; cards that can only be read are left out. Only those whose values
/// repeat are read again from their files.
pub fn cleanings(contacts: &[Contact], region: Option<&Region>) -> Vec<Cleaning> {
    contacts
        .iter()
        .filter(|c| !c.read_only)
        .filter(|c| {
            let repeats = |keys: Vec<String>| keys.iter().collect::<BTreeSet<_>>().len() < keys.len();
            repeats(c.phones.iter().map(|p| phones::key(&p.value, region)).collect()) || repeats(c.emails.iter().map(|e| email_key(&e.value)).collect())
        })
        .filter_map(|c| {
            let text = std::fs::read_to_string(&c.key).ok()?;
            let (_, removed) = clean(&text, region)?;
            Some(Cleaning { key: c.key.clone(), name: c.name.clone(), removed })
        })
        .collect()
}

/// Two cards that may be one person, and what they share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pair {
    /// Their files, the first as the list orders them.
    pub first: String,
    pub second: String,
    pub same_name: bool,
    /// The numbers they share, as the first card writes them.
    pub phones: Vec<String>,
    /// The e-mail addresses they share, as the first card writes them.
    pub emails: Vec<String>,
}

impl Pair {
    /// How much says they are one person: the name, a number, an address each count.
    fn strength(&self) -> (usize, usize) {
        let kinds = usize::from(self.same_name) + usize::from(!self.phones.is_empty()) + usize::from(!self.emails.is_empty());
        (kinds, self.phones.len() + self.emails.len())
    }
}

/// How many cards may share a number, an address or a name and still say
/// one person: more is a switchboard, an office, a common name.
const AT_MOST: usize = 4;

/// What a card is known by for "not the same": its UID, else its file.
fn identity(contact: &Contact) -> &str {
    if contact.uid.trim().is_empty() { &contact.key } else { contact.uid.trim() }
}

/// The pairs of cards that may be one person: the same name, number or
/// e-mail address; cards that can only be read, and pairs said not the same,
/// left out. Those sharing most come first.
pub fn pairs(contacts: &[Contact], region: Option<&Region>, apart: &NotTheSame) -> Vec<Pair> {
    let open: Vec<&Contact> = contacts.iter().filter(|c| !c.read_only).collect();
    // What says who a card is: (0 name, 1 number, 2 address; its key) → the cards, and how each writes it.
    let mut said: BTreeMap<(u8, String), Vec<(usize, String)>> = BTreeMap::new();
    for (i, contact) in open.iter().enumerate() {
        let mut own: BTreeSet<(u8, String)> = BTreeSet::new();
        let mut say = |kind: u8, key: String, written: &str| {
            if own.insert((kind, key.clone())) {
                said.entry((kind, key)).or_default().push((i, written.to_string()));
            }
        };
        let name = name_key(&contact.name);
        // A card named by its address is found by its address.
        if !name.is_empty() && !contact.name.contains('@') {
            say(0, name, &contact.name);
        }
        for phone in &contact.phones {
            let key = phones::key(&phone.value, region);
            if phones::is_whole(&key) {
                say(1, key, &phone.value);
            }
        }
        for email in &contact.emails {
            let key = email_key(&email.value);
            if key.contains('@') {
                say(2, key, &email.value);
            }
        }
    }
    let mut found: BTreeMap<(usize, usize), Pair> = BTreeMap::new();
    for ((kind, _), cards) in said.iter().filter(|(_, cards)| (2..=AT_MOST).contains(&cards.len())) {
        for (x, (a, written)) in cards.iter().enumerate() {
            for (b, _) in &cards[x + 1..] {
                let pair = found.entry((*a, *b)).or_insert_with(|| Pair { first: open[*a].key.clone(), second: open[*b].key.clone(), same_name: false, phones: Vec::new(), emails: Vec::new() });
                match *kind {
                    0 => pair.same_name = true,
                    1 => pair.phones.push(written.clone()),
                    _ => pair.emails.push(written.clone()),
                }
            }
        }
    }
    let mut pairs: Vec<((usize, usize), Pair)> = found
        .into_iter()
        .filter(|((a, b), _)| {
            let (first, second) = (identity(open[*a]), identity(open[*b]));
            // Two copies of one card (one UID in two address books): by their files.
            let (first, second) = if first == second { (open[*a].key.as_str(), open[*b].key.as_str()) } else { (first, second) };
            !apart.contains(first, second)
        })
        .collect();
    pairs.sort_by(|(at_a, a), (at_b, b)| b.strength().cmp(&a.strength()).then(at_a.cmp(at_b)));
    pairs.into_iter().map(|(_, pair)| pair).collect()
}

/// Pairs of cards said not to be one person: never offered again on this computer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotTheSame {
    /// By UID (by file for a card without one), each pair in order.
    #[serde(default)]
    pub pairs: BTreeSet<[String; 2]>,
}

impl NotTheSame {
    /// `$XDG_STATE_HOME/sioul/contacts-not-the-same.toml`.
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("contacts-not-the-same.toml")
    }

    pub fn load(path: &Path) -> NotTheSame {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// Written next to its place, then moved: never half a file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("toml.new");
        std::fs::write(&temporary, text).map_err(fail)?;
        std::fs::rename(&temporary, path).map_err(fail)
    }

    fn ordered(a: &str, b: &str) -> [String; 2] {
        if a <= b { [a.to_string(), b.to_string()] } else { [b.to_string(), a.to_string()] }
    }

    pub fn contains(&self, a: &str, b: &str) -> bool {
        self.pairs.contains(&NotTheSame::ordered(a, b))
    }

    pub fn insert(&mut self, a: &str, b: &str) {
        self.pairs.insert(NotTheSame::ordered(a, b));
    }

    /// The pair of these two cards remembered: by UID, by file for a card without one.
    pub fn remember(&mut self, first: &Contact, second: &Contact) {
        let (a, b) = (identity(first), identity(second));
        if a == b { self.insert(&first.key, &second.key) } else { self.insert(a, b) }
    }
}

/// Properties of one value: the card that leads keeps its own; the other one
/// fills it when the leading card has none.
const SINGLE: &[&str] = &["ORG", "TITLE", "ROLE", "BDAY", "ANNIVERSARY", "PHOTO", "LOGO", "GENDER", "TZ", "GEO", "X-ANNIVERSARY", "X-SPOUSE", "X-MANAGER", "X-ASSISTANT"];

/// Properties of several values the other card brings, unless the leading one has the same.
const SEVERAL: &[&str] = &[
    "ADR", "URL", "IMPP", "RELATED", "NICKNAME", "LANG", "KEY", "CALURI", "CALADRURI", "FBURL", "X-SOCIALPROFILE", "X-ABRELATEDNAMES", "X-ABDATE", "X-AIM", "X-ICQ", "X-JABBER", "X-MSN", "X-YAHOO", "X-SKYPE", "X-SKYPE-USERNAME",
    "X-GOOGLE-TALK", "X-TWITTER",
];

/// A value as compared between two cards: a web site without its scheme, "www." or last
/// slash; an address by its words; anything case and spaces aside.
fn comparable(property: &str, value: &str) -> String {
    let folded: String = crate::text::fold(value).into_iter().collect();
    let words = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    if property == "URL" {
        let bare = words.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.");
        return bare.trim_end_matches('/').to_string();
    }
    words
}

/// A line of a vCard 4.0 card as vCard 3.0 writes it (`contacts::as_vcard3`); none when 3.0 has no such property.
fn as_vcard3_line(line: &str) -> Option<String> {
    let card = lines::fold(&["BEGIN:VCARD".to_string(), "VERSION:4.0".to_string(), line.to_string(), "END:VCARD".to_string()]);
    lines::unfold(&contacts::as_vcard3(&card)).into_iter().find(|l| !matches!(lines::name(l).as_str(), "BEGIN" | "VERSION" | "END"))
}

/// Two cards made one. `lead` stays as it is: its name, and its organisation,
/// birthday and photo when it has them. `other` adds what only it has:
/// numbers and e-mail addresses (the same ones written twice then cleaned,
/// `clean`), postal addresses, web sites and chat addresses unless the same,
/// its categories, its notes after a blank line (unless one holds the
/// other), and what `lead` lacks of one value. Apple's groups get names of
/// their own; a vCard 4.0 line going into a 3.0 card is written as 3.0.
/// REV renewed.
pub fn merge(lead: &str, other: &str, region: Option<&Region>) -> String {
    let mine = lines::unfold(lead);
    let theirs = lines::unfold(other);
    let version4 = |card: &[String]| card.iter().any(|l| l.eq_ignore_ascii_case("VERSION:4.0"));
    let into_three = !version4(&mine) && version4(&theirs);
    let has = |property: &str| mine.iter().any(|l| lines::name(l) == property);
    let has_value = |property: &str, value: &str| mine.iter().filter(|l| lines::name(l) == property).any(|l| comparable(property, &contacts::shown(l)) == comparable(property, value));
    // Which of their lines come.
    let mut carried: Vec<usize> = Vec::new();
    for (i, line) in theirs.iter().enumerate() {
        if is_companion(line) && group_of(line).is_some() {
            continue;
        }
        let property = lines::name(line);
        let comes = match property.as_str() {
            // Added, then the same ones written twice cleaned.
            "TEL" | "EMAIL" => !contacts::shown(line).is_empty(),
            p if SINGLE.contains(&p) => !has(p),
            p if SEVERAL.contains(&p) => !contacts::shown(line).is_empty() && !has_value(p, &contacts::shown(line)),
            _ => false,
        };
        if comes {
            carried.push(i);
        }
    }
    // The labels of the grouped lines that come, with them.
    let groups: Vec<String> = carried.iter().filter_map(|&i| group_of(&theirs[i]).map(|g| g.to_ascii_lowercase())).collect();
    for (i, line) in theirs.iter().enumerate() {
        if is_companion(line) && group_of(line).is_some_and(|g| groups.contains(&g.to_ascii_lowercase())) {
            carried.push(i);
        }
    }
    carried.sort_unstable();
    // Their groups renamed to names the leading card does not use.
    let mut taken: BTreeSet<String> = mine.iter().filter_map(|l| group_of(l)).map(|g| g.to_ascii_lowercase()).collect();
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    let mut added: Vec<String> = Vec::new();
    for &i in &carried {
        let mut line = theirs[i].clone();
        if let Some(group) = group_of(&line).map(str::to_string) {
            let new = match renamed.get(&group.to_ascii_lowercase()) {
                Some(new) => new.clone(),
                None => {
                    let new = (1..).map(|n| format!("item{n}")).find(|g| !taken.contains(g)).unwrap_or_default();
                    taken.insert(new.clone());
                    renamed.insert(group.to_ascii_lowercase(), new.clone());
                    new
                }
            };
            line = format!("{new}{}", &line[group.len()..]);
        }
        if into_three {
            match as_vcard3_line(&line) {
                Some(three) => line = three,
                None => continue,
            }
        }
        added.push(line);
    }
    let mut out: Vec<Option<String>> = mine.iter().cloned().map(Some).collect();
    // Their categories with the leading card's, as one line.
    let ours = contacts::categories_of(&mine);
    let mut united = ours.clone();
    for name in contacts::categories_of(&theirs) {
        if !united.iter().any(|n| contacts::category_key(n) == contacts::category_key(&name)) {
            united.push(name);
        }
    }
    let mut first_lines: Vec<String> = Vec::new();
    if united != ours {
        replace(&mine, &mut out, &mut first_lines, "CATEGORIES", contacts::categories_line(&united));
    }
    // Notes joined, unless one holds the other.
    let note_of = |card: &[String]| card.iter().filter(|l| lines::name(l) == "NOTE").map(|l| contacts::shown(l)).filter(|n| !n.is_empty()).collect::<Vec<_>>().join("\n");
    let (our_note, their_note) = (note_of(&mine), note_of(&theirs));
    let folded = |text: &str| comparable("NOTE", text);
    let joined = if their_note.is_empty() || folded(&our_note).contains(&folded(&their_note)) {
        our_note.clone()
    } else if our_note.is_empty() || folded(&their_note).contains(&folded(&our_note)) {
        their_note.clone()
    } else {
        format!("{our_note}\n\n{their_note}")
    };
    if joined != our_note {
        replace(&mine, &mut out, &mut first_lines, "NOTE", format!("NOTE:{}", lines::escape(&joined)));
    }
    first_lines.extend(added);
    let text = finish(out.into_iter().flatten().collect(), first_lines);
    clean(&text, region).map_or(text, |(cleaned, _)| cleaned)
}

/// A property's lines replaced by one, where the first was; added when there was none.
fn replace(card: &[String], out: &mut [Option<String>], added: &mut Vec<String>, property: &str, line: String) {
    let present: Vec<usize> = card.iter().enumerate().filter(|(_, l)| lines::name(l) == property).map(|(i, _)| i).collect();
    for &i in &present {
        out[i] = None;
    }
    match present.first() {
        Some(&i) => out[i] = Some(line),
        None => added.push(line),
    }
}

/// One file changed: the card before and after it; none before, it was
/// made; none after, it was deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub path: PathBuf,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// What was done to the cards, kept to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Done {
    /// Its file's name in the folder of what was done.
    #[serde(skip)]
    pub id: String,
    /// When, in seconds since 1970.
    pub at: i64,
    /// "clean" or "merge".
    pub kind: String,
    /// The names of the cards, to say what it was.
    pub names: Vec<String>,
    pub changes: Vec<Change>,
}

/// How long the cards as they were are kept, to undo: thirty days.
pub const KEPT_DAYS: i64 = 30;

/// `$XDG_STATE_HOME/sioul/contacts-undo`: what was done to the cards, with them as they were.
pub fn undo_dir() -> PathBuf {
    crate::config::state_dir().join("contacts-undo")
}

/// An id of what was done: letters, digits and dashes, nothing that leaves its folder.
fn is_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn put(path: &Path, text: Option<&str>) -> Result<(), String> {
    match text {
        Some(text) => crate::vdir::write_item(path, text),
        None => match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
            _ => Ok(()),
        },
    }
}

/// Makes `changes`, the cards as they were saved first in `dir`; returns
/// what was done. When one change fails, those made before it are undone.
pub fn apply(dir: &Path, kind: &str, names: Vec<String>, changes: Vec<Change>, now: i64) -> Result<Done, String> {
    let fail = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(fail)?;
    let id = std::iter::once(format!("{now}-{kind}")).chain((2..1000).map(|n| format!("{now}-{kind}-{n}"))).find(|id| !dir.join(format!("{id}.json")).exists()).ok_or_else(|| format!("{}: full", dir.display()))?;
    let done = Done { id: id.clone(), at: now, kind: kind.to_string(), names, changes };
    let record = dir.join(format!("{id}.json"));
    let temporary = dir.join(format!("{id}.json.new"));
    std::fs::write(&temporary, serde_json::to_string(&done).map_err(|e| e.to_string())?).map_err(fail)?;
    std::fs::rename(&temporary, &record).map_err(fail)?;
    for (n, change) in done.changes.iter().enumerate() {
        if let Err(e) = put(&change.path, change.after.as_deref()) {
            for earlier in done.changes[..n].iter().rev() {
                let _ = put(&earlier.path, earlier.before.as_deref());
            }
            let _ = std::fs::remove_file(&record);
            return Err(e);
        }
    }
    Ok(done)
}

/// What was done lately, the newest first. What is older than thirty days
/// goes, with the versions set aside by its undoing.
pub fn done(dir: &Path, now: i64) -> Vec<Done> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<Done> = Vec::new();
    let mut old: Vec<String> = Vec::new();
    for path in entries.filter_map(Result::ok).map(|e| e.path()) {
        let Some(id) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".json")).map(str::to_string) else { continue };
        let Some(mut record) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Done>(&t).ok()) else { continue };
        if now - record.at > KEPT_DAYS * 86_400 {
            let _ = std::fs::remove_file(&path);
            old.push(id);
            continue;
        }
        record.id = id;
        found.push(record);
    }
    // The versions an undoing set aside, from records gone ("<id>.<n>.vcf").
    if let Ok(entries) = std::fs::read_dir(dir) {
        for path in entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "vcf")) {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let age = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()).and_then(|m| m.elapsed().ok()).map_or(0, |d| d.as_secs());
            if old.iter().any(|id| name.starts_with(&format!("{id}."))) || age > (KEPT_DAYS as u64) * 86_400 {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    found.sort_by(|a, b| b.at.cmp(&a.at).then(b.id.cmp(&a.id)));
    found
}

/// Puts the cards back as they were before `id`, then forgets it. A card
/// changed since (a sync, an edit) is set aside first, as "<id>.<n>.vcf"
/// beside the record. Files outside `root` (the address books) are never
/// written; a card whose address book is gone stays gone. Returns the files
/// written back or removed, and how many versions were set aside.
pub fn undo(dir: &Path, id: &str, root: &Path) -> Result<(Vec<PathBuf>, usize), String> {
    if !is_id(id) {
        return Err(format!("{id}: not something done here"));
    }
    let record = dir.join(format!("{id}.json"));
    let done: Done = std::fs::read_to_string(&record).ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or_else(|| format!("{id}: nothing to undo"))?;
    let mut touched = Vec::new();
    let mut aside = 0;
    for (n, change) in done.changes.iter().enumerate().rev() {
        let inside = change.path.starts_with(root) && !change.path.components().any(|c| c == std::path::Component::ParentDir);
        if !inside || change.path.parent().is_some_and(|p| !p.is_dir()) {
            continue;
        }
        let now = std::fs::read_to_string(&change.path).ok();
        if now.is_some() && now != change.after && now != change.before {
            std::fs::write(dir.join(format!("{id}.{n}.vcf")), now.unwrap_or_default()).map_err(|e| format!("{}: {e}", dir.display()))?;
            aside += 1;
        }
        put(&change.path, change.before.as_deref())?;
        touched.push(change.path.clone());
    }
    let _ = std::fs::remove_file(&record);
    Ok((touched, aside))
}

/// Cleans these cards, one write each, the cards as they were kept first in
/// `dir`; none when no card holds anything twice any more.
pub fn clean_files(paths: &[PathBuf], region: Option<&Region>, dir: &Path, now: i64) -> Result<Option<Done>, String> {
    let mut changes = Vec::new();
    let mut names = Vec::new();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        if let Some((cleaned, _)) = clean(&text, region) {
            names.push(contacts::display_name(&lines::unfold(&text)));
            changes.push(Change { path: path.clone(), before: Some(text), after: Some(cleaned) });
        }
    }
    if changes.is_empty() {
        return Ok(None);
    }
    apply(dir, "clean", names, changes, now).map(Some)
}

/// Merges two cards (`merge`): the leading one's file holds them both, the
/// other's is deleted; both kept as they were first in `dir`.
pub fn merge_files(lead: &Path, other: &Path, region: Option<&Region>, dir: &Path, now: i64) -> Result<Done, String> {
    if lead == other {
        return Err(format!("{}: the same card", lead.display()));
    }
    let read = |path: &Path| std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()));
    let (lead_text, other_text) = (read(lead)?, read(other)?);
    let merged = merge(&lead_text, &other_text, region);
    let names = vec![contacts::display_name(&lines::unfold(&lead_text)), contacts::display_name(&lines::unfold(&other_text))];
    let changes = vec![Change { path: lead.to_path_buf(), before: Some(lead_text), after: Some(merged) }, Change { path: other.to_path_buf(), before: Some(other_text), after: None }];
    apply(dir, "merge", names, changes, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vdir::{Collection, Kind};

    fn france() -> Option<&'static Region> {
        phones::region_named("FR")
    }

    fn book(read_only: bool) -> Collection {
        Collection { kind: Kind::Contacts, account: "a".into(), id: "contacts".into(), dir: PathBuf::from("/tmp"), name: "Contacts".into(), color: None, read_only, components: vec![] }
    }

    /// A contact from a card's text (no photo: reading one writes into the cache).
    fn contact_of(text: &str, file: &str) -> Contact {
        contacts::contact(&lines::unfold(text), Path::new(file), &book(false))
    }

    fn card(lines_in: &[&str]) -> String {
        let mut all = vec!["BEGIN:VCARD", "VERSION:3.0"];
        all.extend_from_slice(lines_in);
        all.push("END:VCARD");
        lines::fold(&all.iter().map(|l| l.to_string()).collect::<Vec<_>>())
    }

    fn without_rev(text: &str) -> Vec<String> {
        lines::unfold(text).into_iter().filter(|l| lines::name(l) != "REV").collect()
    }

    #[test]
    fn one_number_written_twice_on_a_card() {
        let text = card(&["UID:a", "FN:Lou Exemple", "TEL;TYPE=CELL:04 65 71 12 34", "TEL:+33 4 65 71 12 34", "TEL;TYPE=HOME:05 36 49 00 00", "TEL;TYPE=WORK,PREF:0465711234", "X-OTHER:kept"]);
        let (cleaned, removed) = clean(&text, france()).unwrap();
        // The international writing stays, with what the others said: mobile, work, first.
        assert_eq!(without_rev(&cleaned), ["BEGIN:VCARD", "VERSION:3.0", "UID:a", "FN:Lou Exemple", "TEL;TYPE=CELL,WORK,PREF:+33 4 65 71 12 34", "TEL;TYPE=HOME:05 36 49 00 00", "X-OTHER:kept", "END:VCARD"]);
        assert!(cleaned.contains("\r\nREV:"), "{cleaned}");
        assert_eq!(
            removed,
            [
                Removed { kind: "phone".into(), value: "04 65 71 12 34".into(), kept: "+33 4 65 71 12 34".into() },
                Removed { kind: "phone".into(), value: "0465711234".into(), kept: "+33 4 65 71 12 34".into() }
            ]
        );
        // Cleaned once, nothing is left twice.
        assert_eq!(clean(&cleaned, france()), None);
        // Without the region, the national and the international writings are not the same number.
        let two = card(&["FN:A", "TEL:04 65 71 12 34", "TEL:+33 4 65 71 12 34"]);
        assert_eq!(clean(&two, None), None);
        assert_eq!(clean(&two, france()).map(|(_, r)| r.len()), Some(1));
    }

    #[test]
    fn addresses_twice_case_aside_and_apple_labels() {
        // vCard 4.0: kinds in lower case, PREF=1; a label of Apple's goes to the line kept.
        let text = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Lou\r\nEMAIL;TYPE=home:lou@example.org\r\nitem1.EMAIL:Lou@Example.org\r\nitem1.X-ABLabel:club\r\nEMAIL;PREF=1:LOU@example.ORG\r\nEMAIL:other@example.org\r\nEND:VCARD\r\n";
        let (cleaned, removed) = clean(text, None).unwrap();
        assert_eq!(removed.len(), 2);
        let lines = without_rev(&cleaned);
        assert!(lines.contains(&"item1.EMAIL:Lou@Example.org".to_string()) || lines.iter().any(|l| l.starts_with("item1.EMAIL;TYPE=home")), "{lines:?}");
        assert!(lines.contains(&"item1.X-ABLabel:club".to_string()), "the label stays with a line: {lines:?}");
        assert_eq!(lines.iter().filter(|l| lines::name(l) == "EMAIL").count(), 2, "{lines:?}");
        // A grouped duplicate that goes takes its label with it when the kept line has a group of its own.
        let grouped = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Lou\r\nitem1.TEL:+33 4 65 71 12 34\r\nitem1.X-ABLabel:atelier\r\nitem2.TEL:04 65 71 12 34\r\nitem2.X-ABLabel:vieux\r\nEND:VCARD\r\n";
        let (cleaned, _) = clean(grouped, france()).unwrap();
        assert_eq!(without_rev(&cleaned), ["BEGIN:VCARD", "VERSION:3.0", "FN:Lou", "item1.TEL:+33 4 65 71 12 34", "item1.X-ABLabel:atelier", "END:VCARD"]);
        // Nothing twice: nothing to do.
        assert_eq!(clean(&card(&["FN:A", "EMAIL:a@example.org", "TEL:+33 4 65 71 12 34"]), france()), None);
    }

    #[test]
    fn kinds_added_in_the_case_written() {
        assert_eq!(with_kinds("TEL;TYPE=cell:+33 6", &["work".into()], false, false), "TEL;TYPE=cell,work:+33 6");
        assert_eq!(with_kinds("TEL;TYPE=\"cell,voice\":+33 6", &["work".into()], true, true), "TEL;TYPE=\"cell,voice,work\";PREF=1:+33 6");
        assert_eq!(with_kinds("item1.TEL:+33 6", &["cell".into()], true, false), "item1.TEL;TYPE=CELL,PREF:+33 6");
        assert_eq!(with_kinds("TEL:+33 6", &[], false, false), "TEL:+33 6");
    }

    #[test]
    fn names_compared_as_people_write_them() {
        assert_eq!(name_key("Jean Dupont"), name_key("DUPONT, Jean"));
        assert_eq!(name_key("  Jean   Dupont "), name_key("Dupont Jean"));
        assert_eq!(name_key("Zoë Ångström"), name_key("zoe angstrom"));
        assert_eq!(name_key("Jean-Pierre Lætitia"), name_key("jean pierre laetitia"));
        assert_ne!(name_key("Jean Dupont"), name_key("Jeanne Dupont"));
        assert_eq!(email_key(" mailto:Jane@Example.ORG "), "jane@example.org");
    }

    #[test]
    fn pairs_by_number_address_and_name() {
        let people = [
            contact_of(&card(&["UID:a", "FN:Jean Dupont", "TEL:04 65 71 12 34", "EMAIL:jean@example.org"]), "/tmp/a.vcf"),
            contact_of(&card(&["UID:b", "FN:DUPONT Jean", "TEL:+33 4 65 71 12 34"]), "/tmp/b.vcf"),
            contact_of(&card(&["UID:c", "FN:J. D.", "EMAIL:JEAN@example.org"]), "/tmp/c.vcf"),
            contact_of(&card(&["UID:d", "FN:Marie Exemple", "TEL:3631"]), "/tmp/d.vcf"),
            contact_of(&card(&["UID:e", "FN:Paul Exemple", "TEL:36 31"]), "/tmp/e.vcf"),
        ];
        let found = pairs(&people, france(), &NotTheSame::default());
        // The name and the number first, then the address; a short code shared says nothing.
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!((found[0].first.as_str(), found[0].second.as_str(), found[0].same_name), ("/tmp/a.vcf", "/tmp/b.vcf", true));
        assert_eq!(found[0].phones, ["04 65 71 12 34"]);
        assert_eq!((found[1].first.as_str(), found[1].second.as_str(), found[1].emails.clone()), ("/tmp/a.vcf", "/tmp/c.vcf", vec!["jean@example.org".to_string()]));
        // "Not the same" is never asked again, whichever way round.
        let mut apart = NotTheSame::default();
        apart.remember(&people[1], &people[0]);
        let found = pairs(&people, france(), &apart);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].second, "/tmp/c.vcf");
        // A card that can only be read is left out.
        let mut read_only = people.to_vec();
        read_only[2].read_only = true;
        assert!(pairs(&read_only, france(), &apart).is_empty());
        // A switchboard five cards share says nothing.
        let office: Vec<Contact> = (0..5).map(|n| contact_of(&card(&[&format!("UID:o{n}"), &format!("FN:Person {n}"), "TEL:+33 1 99 00 67 89"]), &format!("/tmp/o{n}.vcf"))).collect();
        assert!(pairs(&office, france(), &NotTheSame::default()).is_empty());
    }

    #[test]
    fn not_the_same_remembered() {
        let dir = std::env::temp_dir().join(format!("sioul-not-same-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("contacts-not-the-same.toml");
        let mut apart = NotTheSame::load(&path);
        assert!(apart.pairs.is_empty());
        apart.insert("uid-b", "uid-a");
        apart.save(&path).unwrap();
        let again = NotTheSame::load(&path);
        assert!(again.contains("uid-a", "uid-b") && again.contains("uid-b", "uid-a") && !again.contains("uid-a", "uid-c"));
        assert_eq!(again, apart);
        // Two copies of one card (one UID): by their files.
        let (a, b) = (contact_of(&card(&["UID:same", "FN:A"]), "/tmp/x.vcf"), contact_of(&card(&["UID:same", "FN:A"]), "/tmp/y.vcf"));
        let mut copies = NotTheSame::default();
        copies.remember(&a, &b);
        assert!(copies.contains("/tmp/y.vcf", "/tmp/x.vcf"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merging_keeps_everything_once() {
        let lead = card(&[
            "UID:lead",
            "FN:Jean Dupont",
            "N:Dupont;Jean;;;",
            "TEL;TYPE=CELL:04 65 71 12 34",
            "EMAIL:jean@example.org",
            "item1.URL:https://example.org/",
            "item1.X-ABLabel:site",
            "CATEGORIES:Amis",
            "NOTE:Met at the market",
            "X-LEAD:kept",
        ]);
        let other = card(&[
            "UID:other",
            "FN:J. Dupont",
            "ORG:Exemple SARL",
            "BDAY:1984-05-12",
            "TEL;TYPE=WORK:+33 4 65 71 12 34",
            "TEL:05 36 49 00 00",
            "EMAIL:JEAN@example.org",
            "EMAIL:jd@example.net",
            "item1.ADR:;;1 rue de l'Exemple;Lyon;;69000;France",
            "item1.X-ABLabel:atelier",
            "URL:http://www.example.org",
            "CATEGORIES:amis,Club\\, échecs",
            "NOTE:Plays chess",
            "PHOTO;ENCODING=b;TYPE=JPEG:AAAA",
            "X-OTHER-APP:not carried",
        ]);
        let merged = merge(&lead, &other, france());
        let lines = without_rev(&merged);
        let has = |l: &str| lines.iter().any(|x| x == l);
        // Its own name and lines stay.
        assert!(has("FN:Jean Dupont") && has("N:Dupont;Jean;;;") && has("UID:lead") && has("X-LEAD:kept") && has("item1.URL:https://example.org/") && has("item1.X-ABLabel:site"), "{lines:#?}");
        // One number, written internationally, with both kinds; the other number added.
        assert!(has("TEL;TYPE=WORK,CELL:+33 4 65 71 12 34") && has("TEL:05 36 49 00 00"), "{lines:#?}");
        assert_eq!(lines.iter().filter(|l| lines::name(l) == "TEL").count(), 2, "{lines:#?}");
        // One address of each, the same one once.
        assert!(has("EMAIL:jean@example.org") && has("EMAIL:jd@example.net"), "{lines:#?}");
        assert_eq!(lines.iter().filter(|l| lines::name(l) == "EMAIL").count(), 2, "{lines:#?}");
        // The postal address and its label, under a group of their own; the same web site once.
        assert!(has("item2.ADR:;;1 rue de l'Exemple;Lyon;;69000;France") && has("item2.X-ABLabel:atelier"), "{lines:#?}");
        assert_eq!(lines.iter().filter(|l| lines::name(l) == "URL").count(), 1, "{lines:#?}");
        // What it had none of; categories united as one line; notes joined.
        assert!(has("ORG:Exemple SARL") && has("BDAY:1984-05-12") && has("PHOTO;ENCODING=b;TYPE=JPEG:AAAA"), "{lines:#?}");
        assert!(has("CATEGORIES:Amis,Club\\, échecs") && has("NOTE:Met at the market\\n\\nPlays chess"), "{lines:#?}");
        // What Sioul does not know of the other card does not come; nor its name or UID.
        assert!(!has("X-OTHER-APP:not carried") && !has("FN:J. Dupont") && !has("UID:other"), "{lines:#?}");
        assert!(contacts::parse(&merged).is_some());
        // Merged again with the same card, nothing more comes.
        assert_eq!(without_rev(&merge(&merged, &other, france())), lines);
    }

    #[test]
    fn a_vcard4_card_merged_into_a_vcard3_one() {
        let lead = card(&["UID:lead", "FN:Lou"]);
        let other = "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:o\r\nFN:Lou\r\nEMAIL;PREF=1:lou@example.org\r\nGENDER:F\r\nPHOTO:data:image/png;base64,iVBORw0K\r\nEND:VCARD\r\n";
        let lines = without_rev(&merge(&lead, other, None));
        assert!(lines.contains(&"EMAIL;TYPE=pref:lou@example.org".to_string()) && lines.contains(&"PHOTO;ENCODING=b;TYPE=PNG:iVBORw0K".to_string()), "{lines:#?}");
        assert!(!lines.iter().any(|l| l.starts_with("GENDER")), "{lines:#?}");
    }

    #[test]
    fn undone_as_it_was() {
        let base = std::env::temp_dir().join(format!("sioul-undo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (books, dir) = (base.join("contacts/local/book"), base.join("undo"));
        std::fs::create_dir_all(&books).unwrap();
        let (a, b, c) = (books.join("a.vcf"), books.join("b.vcf"), books.join("c.vcf"));
        let text_a = card(&["UID:a", "FN:Jean Dupont", "TEL:04 65 71 12 34", "TEL:+33 4 65 71 12 34"]);
        let text_b = card(&["UID:b", "FN:J. Dupont", "EMAIL:jean@example.org"]);
        let text_c = card(&["UID:c", "FN:Marie", "EMAIL:m@example.org", "EMAIL:M@example.org"]);
        for (path, text) in [(&a, &text_a), (&b, &text_b), (&c, &text_c)] {
            std::fs::write(path, text).unwrap();
        }
        // Cleaning: one write per card, kept as it was first.
        let now = 1_791_000_000;
        let cleaned = clean_files(&[a.clone(), c.clone()], france(), &dir, now).unwrap().unwrap();
        assert_eq!((cleaned.kind.as_str(), cleaned.names.clone(), cleaned.changes.len()), ("clean", vec!["Jean Dupont".to_string(), "Marie".to_string()], 2));
        assert_eq!(cleaned.changes[0].before.as_deref(), Some(text_a.as_str()));
        assert!(!std::fs::read_to_string(&a).unwrap().contains("04 65 71 12 34"));
        // Merging: the other card's file goes.
        let merged = merge_files(&a, &b, france(), &dir, now + 60).unwrap();
        assert!(!b.exists() && std::fs::read_to_string(&a).unwrap().contains("jean@example.org"));
        assert_eq!(merged.changes[1], Change { path: b.clone(), before: Some(text_b.clone()), after: None });
        let listed = done(&dir, now + 120);
        assert_eq!(listed.iter().map(|d| d.kind.as_str()).collect::<Vec<_>>(), ["merge", "clean"]);
        // The merge undone: both cards back as they were; a change made since set aside.
        std::fs::write(&a, card(&["UID:a", "FN:Jean Dupont", "NOTE:changed since"])).unwrap();
        let (touched, aside) = undo(&dir, &listed[0].id, &base.join("contacts")).unwrap();
        assert_eq!((touched.len(), aside), (2, 1));
        assert_eq!(std::fs::read_to_string(&b).unwrap(), text_b);
        assert!(std::fs::read_to_string(&a).unwrap().contains("TEL:+33 4 65 71 12 34") && !std::fs::read_to_string(&a).unwrap().contains("jean@"));
        assert!(std::fs::read_to_string(dir.join(format!("{}.1.vcf", listed[0].id))).is_ok_and(|t| t.contains("changed since")) || std::fs::read_to_string(dir.join(format!("{}.0.vcf", listed[0].id))).is_ok_and(|t| t.contains("changed since")));
        // Then the cleaning: the cards as they were at first.
        undo(&dir, &listed[1].id, &base.join("contacts")).unwrap();
        assert_eq!((std::fs::read_to_string(&a).unwrap(), std::fs::read_to_string(&c).unwrap()), (text_a.clone(), text_c.clone()));
        assert!(done(&dir, now + 120).is_empty());
        // Nothing outside the address books, nothing that is not an id.
        assert!(undo(&dir, "../x", &base.join("contacts")).is_err());
        // Kept thirty days, then gone.
        clean_files(&[c.clone()], france(), &dir, now).unwrap().unwrap();
        assert_eq!(done(&dir, now + 29 * 86_400).len(), 1);
        assert!(done(&dir, now + 31 * 86_400).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
