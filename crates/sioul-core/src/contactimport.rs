// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Contacts from a file (wave D, 18 November 2026): a vCard file (versions
//! 2.1, 3.0 and 4.0, many cards in one file, as a phone, Google or a
//! Nextcloud writes them) or a CSV file (`csv`), read on this device alone.
//!
//! - **vCard**: each card read by the repository's parser (calcard): folded
//!   lines, 2.1's quoted-printable and charsets. A 2.1 card is written as
//!   3.0, any other as the file holds it. Each keeps its UID, or gets one.
//! - **CSV**: which column is what is a mapping, one field per column
//!   (`Field`), guessed from the headings in French and English
//!   (`data/contact-columns.toml`, data, not code), Google's and Outlook's
//!   exports recognised by their headings. A mapping can be kept under a name
//!   on this device (`$XDG_STATE_HOME/sioul/contacts-import.toml`) and comes
//!   back for a file with the same headings. No preset for any booking
//!   service: its format is not known, so nothing is assumed.
//! - **Those already here**, by the same e-mail address or the same number
//!   (`phones::key`, with the region of Contacts ▸ ⚙): completed with what
//!   the file adds, never doubled, a value already there never overwritten.
//!   Look-alikes by name alone are imported, and left to the Duplicates view.
//!
//! An import is one batch (`contactbatch`): its count first ("312 new, 41
//! completed, 3 rows that do not read"), its progress, Undo as a whole.
//! Numbers are written in their international form; a title column goes to
//! the name's title field (`N`'s prefix).

use crate::contactbatch::{Plan, Step};
use crate::contacts::{self, Contact, ContactEdit, Labeled, NameParts};
use crate::csv::Table;
use crate::lines;
use crate::phones::{self, Region};
use crate::vdir::Collection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The largest file read: 32 MB, room for thousands of cards with photos.
pub const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// What a column holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    #[default]
    Skip,
    Title,
    Given,
    Additional,
    Family,
    Name,
    Suffix,
    Company,
    Role,
    Mobile,
    Phone,
    Email,
    Street,
    Postcode,
    Town,
    Region,
    Country,
    Address,
    Birthday,
    Note,
    Groups,
    Web,
}

impl Field {
    /// Every field a column can be given, in the order the mapping offers them.
    pub const ALL: [Field; 22] = [
        Field::Skip,
        Field::Title,
        Field::Given,
        Field::Additional,
        Field::Family,
        Field::Name,
        Field::Suffix,
        Field::Company,
        Field::Role,
        Field::Mobile,
        Field::Phone,
        Field::Email,
        Field::Street,
        Field::Postcode,
        Field::Town,
        Field::Region,
        Field::Country,
        Field::Address,
        Field::Birthday,
        Field::Note,
        Field::Groups,
        Field::Web,
    ];

    /// Its word in the files and the messages: "given", "skip"…
    pub fn word(self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
    }
}

#[derive(Debug, Deserialize)]
struct Columns {
    #[serde(default)]
    column: Vec<Column>,
    #[serde(default)]
    export: Vec<Export>,
}

#[derive(Debug, Deserialize)]
struct Column {
    field: Field,
    headings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Export {
    name: String,
    headings: Vec<String>,
}

const COLUMNS: &str = include_str!("../data/contact-columns.toml");

fn columns() -> &'static Columns {
    static READ: std::sync::OnceLock<Columns> = std::sync::OnceLock::new();
    READ.get_or_init(|| toml::from_str(COLUMNS).unwrap_or(Columns { column: Vec::new(), export: Vec::new() }))
}

/// A heading as it is compared: without case, accents, spaces and punctuation.
pub fn heading_key(heading: &str) -> String {
    crate::text::fold(&heading.to_lowercase()).into_iter().filter(|c| c.is_alphanumeric()).collect()
}

/// Each column's field, guessed from its heading; one named nowhere left out.
pub fn guess(headings: &[String]) -> Vec<Field> {
    headings
        .iter()
        .map(|h| {
            let key = heading_key(h);
            columns().column.iter().find(|c| c.headings.iter().any(|w| heading_key(w) == key)).map_or(Field::Skip, |c| c.field)
        })
        .collect()
}

/// The program whose export these headings are ("Google Contacts", "Outlook"), when they say it.
pub fn export_of(headings: &[String]) -> Option<String> {
    let keys: Vec<String> = headings.iter().map(|h| heading_key(h)).collect();
    columns().export.iter().find(|e| e.headings.iter().all(|h| keys.contains(&heading_key(h)))).map(|e| e.name.clone())
}

// ---------------------------------------------------------------- mappings kept

/// A mapping kept under a name, for files with the same headings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub name: String,
    pub headings: Vec<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mappings {
    #[serde(default, rename = "mapping")]
    pub kept: Vec<Kept>,
}

impl Mappings {
    /// `$XDG_STATE_HOME/sioul/contacts-import.toml`: this device's own, never shared.
    pub fn default_path() -> PathBuf {
        crate::config::state_dir().join("contacts-import.toml")
    }

    pub fn load(path: &Path) -> Mappings {
        std::fs::read_to_string(path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// The mapping kept for these headings, compared as headings are.
    pub fn found(&self, headings: &[String]) -> Option<&Kept> {
        let keys: Vec<String> = headings.iter().map(|h| heading_key(h)).collect();
        self.kept.iter().find(|k| k.headings.iter().map(|h| heading_key(h)).collect::<Vec<_>>() == keys)
    }

    /// Kept under `name`: one of the same name or the same headings replaced.
    pub fn keep(path: &Path, name: &str, headings: &[String], fields: &[Field]) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("a mapping needs a name".into());
        }
        let mut all = Mappings::load(path);
        let keys: Vec<String> = headings.iter().map(|h| heading_key(h)).collect();
        all.kept.retain(|k| !k.name.eq_ignore_ascii_case(name) && k.headings.iter().map(|h| heading_key(h)).collect::<Vec<_>>() != keys);
        all.kept.push(Kept { name: name.to_string(), headings: headings.to_vec(), fields: fields.to_vec() });
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = toml::to_string(&all).map_err(|e| e.to_string())?;
        crate::filelock::replace(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }
}

// ---------------------------------------------------------------- reading

/// The cards a file holds, each with the line it starts on, and the lines that do not read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cards {
    pub cards: Vec<(usize, String)>,
    pub broken: Vec<usize>,
}

/// A card's UID, as its first `UID` line says.
fn uid_of(text: &str) -> Option<String> {
    crate::contactbatch::uid_of(text)
}

/// Each card of a vCard file: its lines from `BEGIN:VCARD` to `END:VCARD`,
/// read by calcard (folded lines, 2.1's quoted-printable and charsets); a 2.1
/// (or 2.0) card written again as 3.0, any other kept as the file holds it;
/// one without a UID given one. A card calcard does not read, or never
/// closed, is named by its first line.
pub fn read_vcf(text: &str) -> Cards {
    let mut out = Cards::default();
    let mut current: Option<(usize, Vec<&str>)> = None;
    for (n, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r');
        let bare = line.trim();
        if bare.eq_ignore_ascii_case("BEGIN:VCARD") {
            if let Some((start, _)) = current.take() {
                out.broken.push(start);
            }
            current = Some((n + 1, vec![line]));
            continue;
        }
        let Some((start, card)) = current.as_mut() else { continue };
        card.push(line);
        if bare.eq_ignore_ascii_case("END:VCARD") {
            let (start, card) = (*start, card.join("\r\n") + "\r\n");
            current = None;
            match one_card(&card) {
                Some(card) => out.cards.push((start, card)),
                None => out.broken.push(start),
            }
        }
    }
    if let Some((start, _)) = current {
        out.broken.push(start);
    }
    out
}

/// One card's text, as Sioul keeps it; none when calcard does not read it.
fn one_card(text: &str) -> Option<String> {
    let parsed = contacts::parse(text)?;
    let old = matches!(parsed.version(), Some(calcard::vcard::VCardVersion::V2_1 | calcard::vcard::VCardVersion::V2_0));
    let mut card = if old {
        let mut out = String::new();
        parsed.write_to(&mut out, calcard::vcard::VCardVersion::V3_0).ok()?;
        // A line break decoded from quoted-printable (=0D=0A) is written "\r\n" by calcard,
        // which vCard 3.0 does not know: one "\n".
        out.replace("\\r\\n", "\\n")
    } else {
        text.to_string()
    };
    if uid_of(&card).is_none() {
        let mut all = lines::unfold(&card);
        let at = all.iter().position(|l| lines::name(l) == "VERSION").map_or(1, |i| i + 1).min(all.len());
        all.insert(at, format!("UID:{}", crate::vdir::new_name()));
        card = lines::fold(&all);
    }
    Some(card)
}

/// The cards a CSV table makes with these fields, one per row; a row that
/// gives neither a name, an address nor a number does not read.
pub fn read_csv(table: &Table, fields: &[Field]) -> Cards {
    let mut out = Cards { cards: Vec::new(), broken: table.broken.clone() };
    for row in &table.rows {
        let edit = edit_of_row(row, fields);
        if edit.name.trim().is_empty() && edit.emails.is_empty() && edit.phones.is_empty() && edit.org.trim().is_empty() {
            out.broken.push(row.line);
            continue;
        }
        out.cards.push((row.line, contacts::new_card(&edit)));
    }
    out.broken.sort_unstable();
    out
}

/// One row as the contact form gives a card.
fn edit_of_row(row: &crate::csv::Row, fields: &[Field]) -> ContactEdit {
    let mut edit = ContactEdit { categories: Some(Vec::new()), ..ContactEdit::default() };
    let mut parts = NameParts::default();
    let mut additional = String::new();
    let (mut street, mut postcode, mut town, mut region, mut country) = (Vec::new(), String::new(), String::new(), String::new(), String::new());
    let add = |to: &mut String, value: &str| {
        if !value.is_empty() {
            if !to.is_empty() {
                to.push(' ');
            }
            to.push_str(value);
        }
    };
    for (column, field) in fields.iter().enumerate() {
        let value = row.cell(column);
        if value.is_empty() {
            continue;
        }
        match field {
            Field::Skip => {}
            Field::Title => add(&mut parts.prefix, value),
            Field::Given => add(&mut parts.given, value),
            Field::Additional => add(&mut additional, value),
            Field::Family => add(&mut parts.family, value),
            Field::Suffix => add(&mut parts.suffix, value),
            Field::Name => add(&mut edit.name, value),
            Field::Company => add(&mut edit.org, value),
            Field::Role => add(&mut edit.title, value),
            Field::Mobile => edit.phones.push(Labeled { label: "cell".into(), value: value.into() }),
            Field::Phone => edit.phones.push(Labeled { label: String::new(), value: value.into() }),
            Field::Email => {
                for address in value.split([',', ';', ' ']).map(str::trim).filter(|a| a.contains('@')) {
                    edit.emails.push(Labeled { label: String::new(), value: address.into() });
                }
            }
            Field::Street => street.push(value.to_string()),
            Field::Postcode => postcode = value.into(),
            Field::Town => town = value.into(),
            Field::Region => region = value.into(),
            Field::Country => country = value.into(),
            Field::Address => edit.addresses.push(Labeled { label: String::new(), value: value.into() }),
            Field::Birthday => edit.birthday = birthday(value),
            Field::Note => {
                if !edit.notes.is_empty() {
                    edit.notes.push('\n');
                }
                edit.notes.push_str(value);
            }
            Field::Groups => {
                // Google's labels (" ::: ", its own "* myContacts" aside), else commas or semicolons.
                let names: Vec<&str> = if value.contains(":::") { value.split(":::").collect() } else { value.split([',', ';']).collect() };
                for name in names.into_iter().map(str::trim).filter(|n| !n.is_empty() && !n.starts_with('*')) {
                    edit.categories.get_or_insert_with(Vec::new).push(name.to_string());
                }
            }
            Field::Web => edit.urls.push(value.into()),
        }
    }
    if !street.is_empty() || !postcode.is_empty() || !town.is_empty() {
        let city = [postcode.as_str(), town.as_str()].iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" ");
        let lines: Vec<String> = street.into_iter().chain([city, region, country]).filter(|l| !l.trim().is_empty()).collect();
        edit.addresses.push(Labeled { label: String::new(), value: lines.join("\n") });
    }
    if !additional.is_empty() && !parts.given.is_empty() {
        parts.given = format!("{} {additional}", parts.given);
    }
    // A title written in the name's own column ("Mme Jeanne Martin") stays where it was: "Move the title into its field" offers it.
    if edit.name.trim().is_empty() {
        edit.name = parts.name();
    }
    if !parts.is_empty() {
        edit.parts = Some(parts);
    }
    edit
}

/// A birthday as a card writes it, from what a spreadsheet writes:
/// "12/05/1984" (day first, as in France), "1984-05-12", "12.05.1984"; else as written.
fn birthday(value: &str) -> String {
    let parts: Vec<&str> = value.split(['/', '.', '-']).map(str::trim).collect();
    match parts.as_slice() {
        [y, m, d] if y.len() == 4 => format!("{y}-{m:0>2}-{d:0>2}"),
        [d, m, y] if y.len() == 4 && d.len() <= 2 && m.len() <= 2 => format!("{y}-{m:0>2}-{d:0>2}"),
        _ => value.to_string(),
    }
}

/// A card's numbers written in their international form, by `region`, when
/// they can be (`phones::key`): "04 65 71 12 34" as "+33465711234". A short
/// code, a number abroad already, or one with an extension, as written.
pub fn international(text: &str, region: Option<&Region>) -> String {
    let card = lines::unfold(text);
    let mut changed = false;
    let out: Vec<String> = card
        .into_iter()
        .map(|line| {
            if lines::name(&line) != "TEL" {
                return line;
            }
            let value = lines::unescape(lines::value(&line)).trim().to_string();
            let key = phones::key(&value, region);
            if phones::is_international(&value) || !key.starts_with('+') || key.contains(';') {
                return line;
            }
            changed = true;
            let head = &line[..line.len() - lines::value(&line).len()];
            format!("{head}{key}")
        })
        .collect();
    if changed { lines::fold(&out) } else { text.to_string() }
}

// ---------------------------------------------------------------- plan

/// What an import will do, before it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Imported {
    pub plan: Plan,
    /// Cards made in the book chosen.
    pub new: usize,
    /// Cards already here completed with what the file adds.
    pub completed: usize,
    /// Cards already here, to which the file adds nothing.
    pub same: usize,
    /// Cards already here in a book you only read: left, never doubled.
    pub read_only: usize,
    /// Lines that do not read.
    pub broken: Vec<usize>,
    /// Names that begin with a title (the move is offered).
    pub titled: usize,
}

/// The first word of a label, as the form keeps it: "work, home" → "work".
fn first_label(label: &str) -> String {
    label.split(',').next().unwrap_or("").trim().to_string()
}

/// One card planned: where it goes, what it held, what it will hold.
struct Planned {
    path: PathBuf,
    before: Option<String>,
    after: String,
    read_only: bool,
}

/// The import planned: each card of `cards` matched with those here
/// (`existing`, every book's) by an e-mail address or a number, completed
/// there; else made in `target`. `groups` added to every card imported,
/// those completed too; `titles`: a title at the start of a name moved into
/// its field. Numbers in their international form by `region`.
pub fn plan(cards: &Cards, existing: &[Contact], target: &Collection, groups: &[String], titles: bool, region: Option<&'static Region>) -> Imported {
    let mut planned: Vec<Planned> = Vec::new();
    let mut by_address: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_number: BTreeMap<String, usize> = BTreeMap::new();
    let mut names: Vec<String> = Vec::new();
    let index = |contact: &Contact, at: usize, by_address: &mut BTreeMap<String, usize>, by_number: &mut BTreeMap<String, usize>| {
        for email in &contact.emails {
            by_address.entry(email.value.trim().to_lowercase()).or_insert(at);
        }
        for phone in &contact.phones {
            let key = phones::key(&phone.value, region);
            if phones::is_whole(&key) {
                by_number.entry(key).or_insert(at);
            }
        }
    };
    for contact in existing {
        let Ok(text) = std::fs::read_to_string(&contact.key) else { continue };
        let at = planned.len();
        planned.push(Planned { path: PathBuf::from(&contact.key), before: Some(text.clone()), after: text, read_only: contact.read_only });
        names.push(contact.name.clone());
        index(contact, at, &mut by_address, &mut by_number);
    }
    let here = planned.len();
    let mut out = Imported { broken: cards.broken.clone(), ..Imported::default() };
    let mut taken: Vec<PathBuf> = Vec::new();
    for (_, text) in &cards.cards {
        let mut text = international(text, region);
        if titles && let contacts::TitleMove::Moves { text: moved, .. } = contacts::move_title(&text) {
            text = moved;
        }
        if !groups.is_empty() {
            text = contacts::with_categories(&text, groups, &[]).unwrap_or(text);
        }
        let incoming = contacts::contact(&lines::unfold(&text), Path::new(""), target);
        out.titled += usize::from(crate::titles::split(&incoming.name).is_some());
        let matched = incoming.emails.iter().find_map(|e| by_address.get(&e.value.trim().to_lowercase()).copied()).or_else(|| incoming.phones.iter().find_map(|p| by_number.get(&phones::key(&p.value, region)).copied()));
        match matched {
            Some(at) => {
                if planned[at].read_only {
                    out.read_only += 1;
                    continue;
                }
                match completed(&planned[at].after, &incoming, groups, region) {
                    Some(after) => planned[at].after = after,
                    None => out.same += 1,
                }
                let now = contacts::contact(&lines::unfold(&planned[at].after), &planned[at].path, target);
                index(&now, at, &mut by_address, &mut by_number);
            }
            None => {
                let mut path = contacts::new_path(target);
                while taken.contains(&path) {
                    path = contacts::new_path(target);
                }
                taken.push(path.clone());
                let at = planned.len();
                planned.push(Planned { path, before: None, after: text, read_only: false });
                names.push(incoming.name.clone());
                index(&incoming, at, &mut by_address, &mut by_number);
            }
        }
    }
    for (at, p) in planned.into_iter().enumerate() {
        let name = names.get(at).cloned().unwrap_or_default();
        match p.before {
            None => {
                out.new += 1;
                out.plan.steps.push((name, Step::Write { path: p.path, before: None, after: p.after }));
            }
            Some(before) if at < here && before != p.after => {
                out.completed += 1;
                out.plan.steps.push((name, Step::Write { path: p.path, before: Some(before), after: p.after }));
            }
            Some(_) => {}
        }
    }
    out
}

/// A card here completed with what `incoming` adds: its addresses, numbers,
/// postal addresses and web sites not there yet (numbers compared by
/// `phones::key`, addresses case aside), a single value it lacks (its
/// organisation, role, birthday, note, the name's parts), and its groups
/// with those asked. A value already there is never overwritten. None when
/// nothing is added.
fn completed(text: &str, incoming: &Contact, groups: &[String], region: Option<&Region>) -> Option<String> {
    let here = contacts::contact(&lines::unfold(text), Path::new(""), &Collection { kind: crate::vdir::Kind::Contacts, account: String::new(), id: String::new(), dir: PathBuf::new(), name: String::new(), color: None, read_only: false, components: Vec::new() });
    let keep = |list: &[Labeled]| list.iter().map(|l| Labeled { label: first_label(&l.label), value: l.value.clone() }).collect::<Vec<_>>();
    let mut emails = keep(&here.emails);
    for e in &incoming.emails {
        if !emails.iter().any(|h| h.value.eq_ignore_ascii_case(&e.value)) {
            emails.push(Labeled { label: first_label(&e.label), value: e.value.clone() });
        }
    }
    let mut phones = keep(&here.phones);
    for p in &incoming.phones {
        if !phones.iter().any(|h| phones::key(&h.value, region) == phones::key(&p.value, region)) {
            phones.push(Labeled { label: first_label(&p.label), value: p.value.clone() });
        }
    }
    let folded = |t: &str| crate::text::fold(&t.to_lowercase()).into_iter().filter(|c| c.is_alphanumeric()).collect::<String>();
    let mut addresses = keep(&here.addresses);
    for a in &incoming.addresses {
        if !addresses.iter().any(|h| folded(&h.value) == folded(&a.value)) {
            addresses.push(Labeled { label: first_label(&a.label), value: a.value.clone() });
        }
    }
    let mut urls = here.urls.clone();
    for u in &incoming.urls {
        if !urls.iter().any(|h| h.trim_end_matches('/').eq_ignore_ascii_case(u.trim_end_matches('/'))) {
            urls.push(u.clone());
        }
    }
    let or = |mine: &str, theirs: &str| if mine.trim().is_empty() { theirs.to_string() } else { mine.to_string() };
    let mut categories = here.categories.clone();
    for name in incoming.categories.iter().chain(groups) {
        if !categories.iter().any(|c| contacts::category_key(c) == contacts::category_key(name)) {
            categories.push(name.clone());
        }
    }
    let edit = ContactEdit {
        name: or(&here.name, &incoming.name),
        emails,
        phones,
        org: or(&here.org, &incoming.org),
        title: or(&here.title, &incoming.title),
        addresses,
        birthday: or(&here.birthday, &incoming.birthday),
        notes: or(&here.notes, &incoming.notes),
        urls,
        categories: Some(categories),
        parts: Some(NameParts { prefix: or(&here.prefix, &incoming.prefix), given: or(&here.given, &incoming.given), family: or(&here.family, &incoming.family), suffix: or(&here.suffix, &incoming.suffix) }),
    };
    let after = contacts::apply(text, &edit);
    (after != text).then_some(after)
}

#[cfg(test)]
#[path = "contactimport_tests.rs"]
mod tests;
