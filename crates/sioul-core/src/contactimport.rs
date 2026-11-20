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

/// The most cards read from one file, and the largest card (photos
/// included): what goes past is not read, and said (review F7).
pub const MOST_CARDS: usize = 20_000;
pub const MOST_CARD: usize = 2 * 1024 * 1024;

/// The cards a file holds, each with the line it starts on, and what did not read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cards {
    pub cards: Vec<(usize, String)>,
    /// Cards or rows that do not read, by their first line.
    pub broken: Vec<usize>,
    /// Cards larger than `MOST_CARD`, by their first line: not read.
    pub too_big: Vec<usize>,
    /// The file held more than `MOST_CARDS` (or the table's rows more than
    /// `csv::MOST_ROWS`): the first line not read.
    pub cut: Option<usize>,
    /// Birthdays that read two ways ("5/12/1984") or none: kept as the file
    /// writes them in the card's note, never guessed (review F22).
    pub birthdays_as_written: usize,
}

/// A card's UID, as its first `UID` line says.
fn uid_of(text: &str) -> Option<String> {
    crate::contactbatch::uid_of(text)
}

/// Each card of a vCard file: its lines from `BEGIN:VCARD` to `END:VCARD`,
/// read by calcard (folded lines, 2.1's quoted-printable and charsets); a 2.1
/// (or 2.0) card written again as 3.0, any other kept as the file holds it;
/// one without a UID given one. A card calcard does not read, one never
/// closed, or one that gives neither a name, an address, a number nor an
/// organisation, is named by its first line; at most `MOST_CARDS` of
/// `MOST_CARD` bytes each.
pub fn read_vcf(text: &str) -> Cards {
    let mut out = Cards::default();
    let mut current: Option<(usize, Vec<&str>, usize)> = None;
    for (n, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r');
        let bare = line.trim();
        if bare.eq_ignore_ascii_case("BEGIN:VCARD") {
            if let Some((start, _, _)) = current.take() {
                out.broken.push(start);
            }
            if out.cards.len() >= MOST_CARDS {
                out.cut = Some(n + 1);
                return out;
            }
            current = Some((n + 1, vec![line], line.len()));
            continue;
        }
        let Some((start, card, size)) = current.as_mut() else { continue };
        *size += line.len() + 2;
        if *size <= MOST_CARD {
            card.push(line);
        }
        if bare.eq_ignore_ascii_case("END:VCARD") {
            let (start, too_big) = (*start, *size > MOST_CARD);
            let card = card.join("\r\n") + "\r\n";
            current = None;
            if too_big {
                out.too_big.push(start);
                continue;
            }
            match one_card(&card) {
                Some(card) => out.cards.push((start, card)),
                None => out.broken.push(start),
            }
        }
    }
    if let Some((start, _, _)) = current {
        out.broken.push(start);
    }
    out
}

/// One card's text, as Sioul keeps it; none when calcard does not read it,
/// or when it says nothing of anyone.
fn one_card(text: &str) -> Option<String> {
    let parsed = contacts::parse(text)?;
    let old = matches!(parsed.version(), Some(calcard::vcard::VCardVersion::V2_1 | calcard::vcard::VCardVersion::V2_0));
    let mut card = if old {
        let mut out = String::new();
        parsed.write_to(&mut out, calcard::vcard::VCardVersion::V3_0).ok()?;
        // A line break decoded from quoted-printable (=0D=0A) is written "\r\n" by calcard,
        // which vCard 3.0 does not know: one "\n"; and its charsets, decoded, are said no more.
        let lines: Vec<String> = lines::unfold(&out.replace("\\r\\n", "\\n")).into_iter().map(|l| without_charset(&l)).collect();
        lines::fold(&lines)
    } else {
        text.to_string()
    };
    let unfolded = lines::unfold(&card);
    let says = |name: &str| unfolded.iter().any(|l| lines::name(l) == name && !lines::value(l).trim().trim_matches(';').is_empty());
    if !["FN", "N", "ORG", "EMAIL", "TEL"].iter().any(|p| says(p)) {
        return None;
    }
    if uid_of(&card).is_none() {
        let mut all = unfolded;
        let at = all.iter().position(|l| lines::name(l) == "VERSION").map_or(1, |i| i + 1).min(all.len());
        all.insert(at, format!("UID:{}", crate::vdir::new_name()));
        card = lines::fold(&all);
    }
    Some(card)
}

/// A line without its `CHARSET` parameter (a 2.1 card's, decoded already).
fn without_charset(line: &str) -> String {
    let value = lines::value(line);
    if !line.contains(':') {
        return line.to_string();
    }
    let head = &line[..line.len() - value.len() - 1];
    let kept: Vec<&str> = head.split(';').filter(|p| !p.trim().to_ascii_uppercase().starts_with("CHARSET=")).collect();
    format!("{}:{value}", kept.join(";"))
}

/// The cards a CSV table makes with these fields, one per row; a row that
/// gives neither a name, an address nor a number does not read. A birthday
/// that reads two ways is kept in the note as `tr` words it.
pub fn read_csv(table: &Table, fields: &[Field], tr: &crate::i18n::Translator) -> Cards {
    let mut out = Cards { broken: table.broken.clone(), cut: table.cut, ..Cards::default() };
    let day_first = french(&table.headings);
    for row in &table.rows {
        let (edit, as_written) = edit_of_row(row, fields, day_first, tr);
        if edit.name.trim().is_empty() && edit.emails.is_empty() && edit.phones.is_empty() && edit.org.trim().is_empty() {
            out.broken.push(row.line);
            continue;
        }
        out.birthdays_as_written += usize::from(as_written);
        out.cards.push((row.line, contacts::new_card(&edit)));
    }
    out.broken.sort_unstable();
    out
}

/// An e-mail address as a cell writes it: "jeanne@example.org", or with its
/// name, "Jeanne <jeanne@example.org>"; none without an `@`.
fn address_in(text: &str) -> Option<String> {
    let text = text.trim();
    let inside = match (text.rfind('<'), text.rfind('>')) {
        (Some(open), Some(close)) if open < close => &text[open + 1..close],
        _ => text,
    };
    let inside = inside.trim().trim_start_matches("mailto:");
    (inside.contains('@') && !inside.contains(char::is_whitespace)).then(|| inside.to_string())
}

/// One row as the contact form gives a card; whether its birthday was kept as written.
/// Whether a table's headings are French ("Prénom", "Date de naissance"):
/// its dates are read day first when they could be read both ways.
fn french(headings: &[String]) -> bool {
    const FRENCH: [&str; 12] = ["prenom", "prenoms", "nom", "nomdefamille", "civilite", "telephone", "courriel", "datedenaissance", "societe", "ville", "codepostal", "adresse"];
    headings.iter().any(|h| FRENCH.contains(&heading_key(h).as_str()))
}

fn edit_of_row(row: &crate::csv::Row, fields: &[Field], day_first: bool, tr: &crate::i18n::Translator) -> (ContactEdit, bool) {
    let mut edit = ContactEdit { categories: Some(Vec::new()), ..ContactEdit::default() };
    let mut parts = NameParts::default();
    let mut additional = String::new();
    let mut as_written = false;
    let (mut street, mut postcode, mut town, mut region, mut country) = (Vec::new(), String::new(), String::new(), String::new(), String::new());
    let add = |to: &mut String, value: &str| {
        if !value.is_empty() {
            if !to.is_empty() {
                to.push(' ');
            }
            to.push_str(value);
        }
    };
    let mut notes: Vec<String> = Vec::new();
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
                for address in value.split([',', ';']).filter_map(address_in) {
                    edit.emails.push(Labeled { label: String::new(), value: address });
                }
            }
            Field::Street => street.push(value.to_string()),
            Field::Postcode => postcode = value.into(),
            Field::Town => town = value.into(),
            Field::Region => region = value.into(),
            Field::Country => country = value.into(),
            Field::Address => edit.addresses.push(Labeled { label: String::new(), value: value.into() }),
            Field::Birthday => match birthday(value, day_first) {
                Some(day) => edit.birthday = day,
                None => {
                    as_written = true;
                    let mut args = crate::i18n::args();
                    args.set("day", value.to_string());
                    notes.push(tr.text("import-birthday-as-written", Some(&args)));
                }
            },
            Field::Note => notes.insert(0, value.to_string()),
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
    edit.notes = notes.join("\n");
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
    (edit, as_written)
}

/// A birthday as a card writes it, from what a spreadsheet writes, only when
/// it reads one way: "1984-05-12", "13/05/1984" (a day over 12 first),
/// "05/13/1984" (a day over 12 second); one that reads two ways ("5/12/1984")
/// day first only in a table of French headings (`day_first`). Else, a year
/// in two digits, or no date: none, and it is kept as the file writes it
/// (review F22: Outlook's months first were read as days).
fn birthday(value: &str, day_first: bool) -> Option<String> {
    let parts: Vec<&str> = value.trim().split(['/', '.', '-']).map(str::trim).collect();
    let number = |p: &str| p.parse::<u32>().ok();
    let valid = |y: &str, m: u32, d: u32| y.len() == 4 && number(y).is_some() && (1..=12).contains(&m) && (1..=31).contains(&d);
    match parts.as_slice() {
        [y, m, d] if y.len() == 4 => {
            let (m, d) = (number(m)?, number(d)?);
            valid(y, m, d).then(|| format!("{y}-{m:02}-{d:02}"))
        }
        [a, b, y] if y.len() == 4 => {
            let (a, b) = (number(a)?, number(b)?);
            match (a > 12, b > 12) {
                (true, false) if valid(y, b, a) => Some(format!("{y}-{b:02}-{a:02}")),
                (false, true) if valid(y, a, b) => Some(format!("{y}-{a:02}-{b:02}")),
                // The same day both ways ("05/05/1984"), or a French table's.
                (false, false) if (a == b || day_first) && valid(y, b, a) => Some(format!("{y}-{b:02}-{a:02}")),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A card's numbers written in their international form, by `region`, when
/// they can be (`phones::key`): "04 65 71 12 34" as "+33465711234"; how many.
/// A short code, a number abroad already, one with an extension, and a
/// number given as an address (`VALUE=uri`), as written. A number from
/// another country written without its own code is read as `region`'s: the
/// import asks first (its box says the country, review F22).
pub fn international(text: &str, region: Option<&Region>) -> (String, usize) {
    let card = lines::unfold(text);
    let mut changed = 0;
    let out: Vec<String> = card
        .into_iter()
        .map(|line| {
            if lines::name(&line) != "TEL" || lines::param(&line, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("uri")) {
                return line;
            }
            let value = lines::unescape(lines::value(&line)).trim().to_string();
            let key = phones::key(&value, region);
            if phones::is_international(&value) || !key.starts_with('+') || key.contains(';') {
                return line;
            }
            changed += 1;
            let head = &line[..line.len() - lines::value(&line).len()];
            format!("{head}{key}")
        })
        .collect();
    if changed > 0 { (lines::fold(&out), changed) } else { (text.to_string(), 0) }
}

// ---------------------------------------------------------------- plan

/// What an import is asked to do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Groups added to every card made.
    pub groups: Vec<String>,
    /// Those groups added to the cards already here too.
    pub groups_here: bool,
    /// The cards already here completed with what the file adds (off by default).
    pub complete: bool,
    /// A title at the start of a name moved into its field.
    pub titles: bool,
    /// Numbers written in their international form by the region.
    pub international: bool,
    /// The cards here, by file, that share a UID with a card of the file
    /// under another name, ticked one by one to be completed all the same
    /// (review V5); none by default.
    pub complete_also: Vec<String>,
    /// The groups one of the person's lists reads (`porch::Senders::category_standing`),
    /// as compared (`porch::category_key`): a file's own such group is kept on
    /// its new cards only when `keep_listed` (review V6).
    pub listed: std::collections::BTreeSet<String>,
    pub keep_listed: bool,
}

/// A group the file gives its own new cards, how many, and whether one of
/// the person's lists reads it (review V6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileGroup {
    pub name: String,
    pub cards: usize,
    pub listed: bool,
}

/// A card of the file found already here, and what the file would add to it,
/// each as (kind, value): ("email", "jeanne@example.org"), ("group", "Clients")…
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Match {
    pub name: String,
    pub adds: Vec<(String, String)>,
    /// Found twice in the file, not among the cards here.
    pub in_file: bool,
    /// The card's file (its box, `Options::complete_also`).
    pub key: String,
    /// The name the file gives it, when it is clearly another than the
    /// card's own under the same UID (review V5): left alone unless its own
    /// box is ticked; else "".
    pub other_name: String,
}

/// What an import will do, before it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Imported {
    pub plan: Plan,
    /// Cards read from the file.
    pub read: usize,
    /// Cards made in the book chosen.
    pub new: usize,
    /// Cards of the file already here (the same UID, or the same name and an
    /// address or a number), left as they are unless completing was asked.
    pub here: usize,
    /// Cards of the file found twice in it: the later ones left as the first.
    pub twice: usize,
    /// Of those already here, the cards that will change (completed, given the groups).
    pub completed: usize,
    /// New cards that share an address or a number with another card under
    /// another name: another person, imported as their own card, said.
    pub apart: usize,
    /// Cards already here in a book you only read: left, never doubled.
    pub read_only: usize,
    /// Lines that do not read.
    pub broken: Vec<usize>,
    /// Names that begin with a title (the move is offered).
    pub titled: usize,
    /// Numbers written in their international form.
    pub rewritten: usize,
    /// Those already here, card by card, with what the file would add.
    pub matches: Vec<Match>,
    /// Cards whose UID is here under another name (review V5): listed, left alone by default.
    pub other_names: usize,
    /// The groups the file gives its own new cards (review V6), as first written.
    pub file_groups: Vec<FileGroup>,
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
    name: String,
}

/// A name as two cards of one person compare it: its title aside, then as
/// Duplicates compares names (case, accents, punctuation and word order aside).
fn person_key(name: &str) -> String {
    let name = crate::titles::split(name).map_or_else(|| name.to_string(), |(_, rest)| rest);
    crate::duplicates::name_key(&name)
}

/// A card's title: its title field, else a title at the start of its name.
fn title_of(contact: &Contact) -> String {
    let field = contact.prefix.trim();
    if !field.is_empty() {
        return field.to_string();
    }
    crate::titles::split(&contact.name).map(|(title, _)| title).unwrap_or_default()
}

/// Two cards are two people by their names (review V5): different names
/// once their titles are aside, or two titles that differ ("M. Martin" and
/// "Mme Martin"). A name or a title missing on one side tells nothing.
fn other_person(a: &Contact, b: &Contact) -> bool {
    let (ka, kb) = (person_key(&a.name), person_key(&b.name));
    let (ta, tb) = (title_of(a), title_of(b));
    (!ka.is_empty() && !kb.is_empty() && ka != kb) || (!ta.is_empty() && !tb.is_empty() && !crate::titles::same(&ta, &tb))
}

/// What finds a card again: its UID; its name with each address, each number.
#[derive(Default)]
struct Index {
    uid: BTreeMap<String, usize>,
    named: BTreeMap<(String, String), Vec<usize>>,
    /// Each card as it was indexed, by its place in the plan.
    cards: BTreeMap<usize, Contact>,
    /// Any card's addresses and numbers, whatever its name: to say who shares them.
    any: BTreeMap<String, usize>,
}

impl Index {
    fn keys(contact: &Contact, region: Option<&Region>) -> Vec<String> {
        let addresses = contact.emails.iter().map(|e| e.value.trim().to_lowercase());
        let numbers = contact.phones.iter().map(|p| phones::key(&p.value, region)).filter(|k| phones::is_whole(k));
        addresses.chain(numbers).collect()
    }

    fn add(&mut self, contact: &Contact, text: &str, at: usize, region: Option<&Region>) {
        if let Some(uid) = uid_of(text) {
            self.uid.entry(uid).or_insert(at);
        }
        self.cards.insert(at, contact.clone());
        let name = person_key(&contact.name);
        for key in Self::keys(contact, region) {
            if !name.is_empty() {
                let slot = self.named.entry((name.clone(), key.clone())).or_default();
                if !slot.contains(&at) {
                    slot.push(at);
                }
            }
            self.any.entry(key).or_insert(at);
        }
    }

    /// The card this one is, and whether it holds another name: the same
    /// UID (whatever the name, said when it is clearly another, review V5),
    /// or the same name, its title alike, and an address or a number.
    fn same(&self, contact: &Contact, text: &str, region: Option<&Region>) -> Option<(usize, bool)> {
        if let Some(at) = uid_of(text).and_then(|uid| self.uid.get(&uid).copied()) {
            return Some((at, self.cards.get(&at).is_some_and(|here| other_person(here, contact))));
        }
        let name = person_key(&contact.name);
        if name.is_empty() {
            return None;
        }
        Self::keys(contact, region)
            .into_iter()
            .flat_map(|key| self.named.get(&(name.clone(), key)).cloned().unwrap_or_default())
            .find(|at| self.cards.get(at).is_none_or(|here| !other_person(here, contact)))
            .map(|at| (at, false))
    }

    /// Whether another card shares one of its addresses or numbers.
    fn shares(&self, contact: &Contact, region: Option<&Region>) -> bool {
        Self::keys(contact, region).iter().any(|k| self.any.contains_key(k))
    }
}

/// The import planned (review F13, F14): a card of the file is the card here
/// with its UID, or with its name and one of its addresses or numbers; the
/// same address or number under another name is another person, imported as
/// their own card and counted. A card already here is never changed unless
/// asked (`Options::complete`, `groups_here`), and what the file would add
/// to it is listed card by card first. Two cards of one file follow the
/// same rule. New cards go to `target` with `Options::groups`.
pub fn plan(cards: &Cards, existing: &[Contact], target: &Collection, options: &Options, region: Option<&'static Region>) -> Imported {
    let mut planned: Vec<Planned> = Vec::new();
    let mut index = Index::default();
    for contact in existing {
        let Ok(text) = std::fs::read_to_string(&contact.key) else { continue };
        let at = planned.len();
        index.add(contact, &text, at, region);
        planned.push(Planned { path: PathBuf::from(&contact.key), before: Some(text.clone()), after: text, read_only: contact.read_only, name: contact.name.clone() });
    }
    let here = planned.len();
    let mut out = Imported { broken: cards.broken.clone(), read: cards.cards.len(), ..Imported::default() };
    let mut taken: Vec<PathBuf> = Vec::new();
    for (_, text) in &cards.cards {
        let mut text = text.clone();
        if options.international {
            let (written, rewritten) = international(&text, region);
            text = written;
            out.rewritten += rewritten;
        }
        if options.titles && let contacts::TitleMove::Moves { text: moved, sure: true, .. } = contacts::move_title(&text) {
            text = moved;
        }
        let incoming = contacts::contact_quiet(&lines::unfold(&text), Path::new(""), target);
        out.titled += usize::from(crate::titles::split(&incoming.name).is_some());
        match index.same(&incoming, &text, region) {
            Some((at, _)) if planned[at].read_only => out.read_only += 1,
            Some((at, other_name)) => {
                let in_file = at >= here;
                let key = planned[at].path.display().to_string();
                // The same UID under another name: left alone unless its own box is ticked (review V5).
                let complete = if other_name { options.complete_also.contains(&key) } else { options.complete };
                if other_name {
                    out.other_names += 1;
                } else if in_file {
                    out.twice += 1;
                } else {
                    out.here += 1;
                }
                let groups: &[String] = if (options.groups_here && (!other_name || complete)) || in_file { &options.groups } else { &[] };
                // A file's group that a list reads goes to a card here, too, only with its box (review V6, last check).
                let held = |g: &str| options.listed.contains(&crate::porch::category_key(g)) && !options.keep_listed;
                if complete {
                    for group in incoming.categories.iter().map(|g| g.trim()).filter(|g| !g.is_empty() && options.listed.contains(&crate::porch::category_key(g))) {
                        if !out.file_groups.iter().any(|f| crate::porch::category_key(&f.name) == crate::porch::category_key(group)) {
                            out.file_groups.push(FileGroup { name: group.to_string(), cards: 0, listed: true });
                        }
                        if let Some(found) = out.file_groups.iter_mut().find(|f| crate::porch::category_key(&f.name) == crate::porch::category_key(group)) {
                            found.cards += 1;
                        }
                    }
                }
                let (changed, adds) = completed(&planned[at].after, &incoming, groups, complete, &held, region);
                out.matches.push(Match { name: planned[at].name.clone(), adds, in_file, key, other_name: if other_name { incoming.name.clone() } else { String::new() } });
                if (complete || (options.groups_here && !in_file && !other_name)) && let Some(after) = changed {
                    planned[at].after = after;
                    let now = contacts::contact_quiet(&lines::unfold(&planned[at].after), &planned[at].path, target);
                    index.add(&now, &planned[at].after, at, region);
                }
            }
            None => {
                out.apart += usize::from(index.shares(&incoming, region));
                // The file's own groups, said; one a list of the person's reads kept only when asked (review V6).
                let mut dropped: Vec<String> = Vec::new();
                for group in incoming.categories.iter().map(|g| g.trim()).filter(|g| !g.is_empty()) {
                    let listed = options.listed.contains(&crate::porch::category_key(group));
                    match out.file_groups.iter_mut().find(|f| crate::porch::category_key(&f.name) == crate::porch::category_key(group)) {
                        Some(found) => found.cards += 1,
                        None => out.file_groups.push(FileGroup { name: group.to_string(), cards: 1, listed }),
                    }
                    if listed && !options.keep_listed {
                        dropped.push(group.to_string());
                    }
                }
                if !dropped.is_empty() {
                    text = contacts::with_categories(&text, &[], &dropped).unwrap_or(text);
                }
                if !options.groups.is_empty() {
                    text = contacts::with_categories(&text, &options.groups, &[]).unwrap_or(text);
                }
                let mut path = contacts::new_path(target);
                while taken.contains(&path) {
                    path = contacts::new_path(target);
                }
                taken.push(path.clone());
                let at = planned.len();
                index.add(&incoming, &text, at, region);
                planned.push(Planned { path, before: None, after: text, read_only: false, name: incoming.name.clone() });
            }
        }
    }
    for (at, p) in planned.into_iter().enumerate() {
        match p.before {
            None => {
                out.new += 1;
                out.plan.steps.push((p.name, Step::Write { path: p.path, before: None, after: p.after }));
            }
            Some(before) if at < here && before != p.after => {
                out.completed += 1;
                out.plan.steps.push((p.name, Step::Write { path: p.path, before: Some(before), after: p.after }));
            }
            Some(_) => {}
        }
    }
    out
}

/// A card here completed with what `incoming` adds, when `complete`: its
/// addresses, numbers, postal addresses and web sites not there yet
/// (numbers compared by `phones::key`, addresses case aside), a single value
/// it lacks (its organisation, role, birthday, note, the name's parts when it
/// has none), the file's groups; and `groups` either way. A value already
/// there is never overwritten, and a card's several `ORG` or `TITLE` lines
/// stay (review F21). The card as it would be (None: nothing added), and what
/// is added, as (kind, value), whether asked or not.
fn completed(text: &str, incoming: &Contact, groups: &[String], complete: bool, held: &dyn Fn(&str) -> bool, region: Option<&Region>) -> (Option<String>, Vec<(String, String)>) {
    let card = lines::unfold(text);
    let here = contacts::contact_quiet(&card, Path::new(""), &Collection { kind: crate::vdir::Kind::Contacts, account: String::new(), id: String::new(), dir: PathBuf::new(), name: String::new(), color: None, read_only: false, components: Vec::new() });
    let mut adds: Vec<(String, String)> = Vec::new();
    let mut said = |kind: &str, value: &str| adds.push((kind.to_string(), value.to_string()));
    let keep = |list: &[Labeled]| list.iter().map(|l| Labeled { label: first_label(&l.label), value: l.value.clone() }).collect::<Vec<_>>();
    let mut emails = keep(&here.emails);
    for e in &incoming.emails {
        if !emails.iter().any(|h| h.value.eq_ignore_ascii_case(&e.value)) {
            said("email", &e.value);
            emails.push(Labeled { label: first_label(&e.label), value: e.value.clone() });
        }
    }
    let mut numbers = keep(&here.phones);
    for p in &incoming.phones {
        if !numbers.iter().any(|h| phones::key(&h.value, region) == phones::key(&p.value, region)) {
            said("phone", &p.value);
            numbers.push(Labeled { label: first_label(&p.label), value: p.value.clone() });
        }
    }
    let folded = |t: &str| crate::text::fold(&t.to_lowercase()).into_iter().filter(|c| c.is_alphanumeric()).collect::<String>();
    let mut addresses = keep(&here.addresses);
    for a in &incoming.addresses {
        if !addresses.iter().any(|h| folded(&h.value) == folded(&a.value)) {
            said("address", a.value.lines().next().unwrap_or(""));
            addresses.push(Labeled { label: first_label(&a.label), value: a.value.clone() });
        }
    }
    let mut urls = here.urls.clone();
    for u in &incoming.urls {
        if !urls.iter().any(|h| h.trim_end_matches('/').eq_ignore_ascii_case(u.trim_end_matches('/'))) {
            said("web", u);
            urls.push(u.clone());
        }
    }
    // A single value: the card's own lines kept whole (several ORG stay several), the file's only where it has none.
    let mut single = |kind: &str, property: &str, theirs: &str| {
        let mine = contacts::joined(&card, property);
        if mine.is_empty() && !theirs.trim().is_empty() {
            said(kind, theirs);
            theirs.to_string()
        } else {
            mine
        }
    };
    let org = single("org", "ORG", &incoming.org);
    let role = single("role", "TITLE", &incoming.title);
    let birthday = single("birthday", "BDAY", &incoming.birthday);
    let notes = single("note", "NOTE", &incoming.notes);
    let name = contacts::joined(&card, "FN");
    // Each part of the name it lacks (the same person: its name matched), never one it has.
    let mut part = |kind: &str, mine: &str, theirs: &str| {
        if mine.trim().is_empty() && !theirs.trim().is_empty() {
            said(kind, theirs);
            theirs.to_string()
        } else {
            mine.to_string()
        }
    };
    let parts = NameParts {
        prefix: part("title", &here.prefix, &incoming.prefix),
        given: part("given", &here.given, &incoming.given),
        family: part("family", &here.family, &incoming.family),
        suffix: part("suffix", &here.suffix, &incoming.suffix),
    };
    let mut categories = here.categories.clone();
    // The file's groups, but one that a list of the person's reads while its box is off (review V6, last check).
    for name in incoming.categories.iter().filter(|g| complete && !held(g)).chain(groups) {
        if !categories.iter().any(|c| contacts::category_key(c) == contacts::category_key(name)) {
            said("group", name);
            categories.push(name.clone());
        }
    }
    let edit = if complete {
        ContactEdit { name: if name.is_empty() { incoming.name.clone() } else { name }, emails, phones: numbers, org, title: role, addresses, birthday, notes, urls, categories: Some(categories), parts: Some(parts), book: None }
    } else {
        // Groups alone: everything else as the card holds it.
        let mut adds_only = adds.clone();
        adds_only.retain(|(k, _)| k == "group");
        let after = if adds_only.is_empty() { None } else { contacts::with_categories(text, groups, &[]) };
        return (after, adds);
    };
    let after = contacts::apply(text, &edit);
    ((after != text).then_some(after), adds)
}

#[cfg(test)]
#[path = "contactimport_tests.rs"]
mod tests;
