// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What reaches you, read as **the usual rule and its exceptions**
//! (docs/attention.md, "The page: the rule once, then only the exceptions").
//!
//! The stores stay as they are, on every device: the matrix's rows in
//! `[attention]` (a circle's, a kind's, a person's, an app's, a
//! conversation's), the senders' lists (a group of your contacts on one of
//! them), a phone's own table of calls (`[device."<id>"] calls`), three
//! switches of Sioul's own notices (`reminders.mail`,
//! `reminders.mail_newsletters`, `reminders.gather`), and on a phone its own
//! choices for other apps (`app-notes.toml`: a conversation's "through", an
//! app's kind and area). This module reads all of them as one kind of thing,
//! **a difference from the usual** (`Exception`):
//!
//! - **what**: anything (mail, calls and messages; a person's calls and
//!   messages) · mail · calls · messages · an app · a conversation of an app ·
//!   one of Sioul's own notices (`What`);
//! - **from whom**: anyone · a circle · a group of your contacts · a person (`Whom`);
//! - **where**: every device · one device (`Place`);
//! - **when it reaches you**: a named pattern over the nine moments (always,
//!   when you are awake, in your work and admin hours, in your work hours,
//!   gathered outside your work and admin hours, at the gathered times,
//!   shown, never told, never), your own times (the nine cells), or, for what
//!   is no row of the matrix, what its store says (a group's list, a phone's
//!   table, an app's kind) (`When`).
//!
//! **The usual** is the matrix of the start chosen (`start_of`: As Sioul
//! does, Quieter, More reachable; kept as `[attention] start`, which an
//! older Sioul neither reads nor loses, and read from the rows for settings
//! from before it); a row as the start has it is no difference and is
//! listed nowhere. `list` reads every store into exceptions, `rule` says the
//! usual in one sentence per circle, `decide` says which line decides for a
//! kind, a person and a moment (the most precise first: a conversation, a
//! person, an app, a group, a circle; one device before every device), and
//! `adding` / `removing` give the writes an exception makes in the same
//! stores, in the same shapes as before (`apply` writes them; `save` makes
//! a change of the page whole, with what Undo puts back): an older Sioul
//! reads every one of them as it always did.

#[allow(unused_imports)]
use crate::files::Probe as _;
use crate::appnotes::{AppKind, Choices, Through};
use crate::areas::Area;
use crate::attention::{self, Attention, Column, Kind, Level, Person, Preset, Row};
use crate::calls_line::Profile;
use crate::config::{Config, SettingValue};
use crate::i18n::Translator;
use crate::porch::{SenderList, Standing};
use crate::reach::Channel;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------- the objects

/// One of Sioul's own notices an exception may name: a kind of the matrix,
/// new mail told at all (`reminders.mail`), or newsletters told with it
/// (`reminders.mail_newsletters`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Notice {
    Kind(Kind),
    NewMail,
    Newsletters,
}

impl Notice {
    pub fn id(self) -> &'static str {
        match self {
            Notice::Kind(kind) => kind.id(),
            Notice::NewMail => "new-mail",
            Notice::Newsletters => "newsletters",
        }
    }

    pub fn read(text: &str) -> Option<Notice> {
        match text.trim() {
            "new-mail" => Some(Notice::NewMail),
            "newsletters" => Some(Notice::Newsletters),
            other => Kind::read(other).map(Notice::Kind),
        }
    }
}

impl Serialize for Notice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}

impl<'de> Deserialize<'de> for Notice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Notice, D::Error> {
        let text = String::deserialize(deserializer)?;
        Notice::read(&text).ok_or_else(|| serde::de::Error::custom(format!("{text}: no such notice")))
    }
}

/// What reaches you.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum What {
    /// Mail, calls and messages from a circle; a person's calls and messages
    /// (a person's own row holds no mail).
    Anything,
    Channel { channel: Channel },
    /// An app on your phones (`app.<package>`), by its name as last seen.
    App {
        package: String,
        #[serde(default)]
        name: String,
    },
    /// A conversation of an app (`conversation.<key>`, `appnotes::talk_key`);
    /// its title and app's name known on the phone that holds it alone.
    Conversation {
        key: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        app: String,
    },
    Own { notice: Notice },
}

/// From whom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Whom {
    Anyone,
    /// A circle: a row of a people channel (`attention::Person`), group
    /// conversations among them (`Person::Groups`).
    Circle { person: Person },
    /// A group of your contacts (a category of their cards), as written.
    Group { name: String },
    /// A person, as their own row names them (`contact:<UID>`, `tel:<key>`,
    /// an address), with the name their card gives, never stored.
    Person {
        id: String,
        #[serde(default)]
        name: String,
    },
}

/// Where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Place {
    Everywhere,
    Device {
        id: String,
        #[serde(default)]
        name: String,
    },
}

/// A named pattern over the nine moments (`cells` writes it on a row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Pattern {
    /// At every moment (mail shown without a word at night and in a pause).
    Always,
    /// In work, admin, leisure and meals; held at night and in a pause.
    Awake,
    /// Messages: at once in work and admin hours, gathered in leisure and meals.
    WorkAdminGathered,
    WorkAdmin,
    Work,
    /// At the gathered times (Sioul's own that take them, an app).
    Gathered,
    /// Mail: on the Porch, never told.
    Shown,
    /// Never told, never let through.
    Never,
}

impl Pattern {
    /// In the order the card offers them and a row is read by.
    pub const ALL: [Pattern; 8] = [Pattern::Always, Pattern::Awake, Pattern::WorkAdminGathered, Pattern::WorkAdmin, Pattern::Work, Pattern::Gathered, Pattern::Shown, Pattern::Never];

    pub fn id(self) -> &'static str {
        match self {
            Pattern::Always => "always",
            Pattern::Awake => "awake",
            Pattern::WorkAdminGathered => "work-admin-gathered",
            Pattern::WorkAdmin => "work-admin",
            Pattern::Work => "work",
            Pattern::Gathered => "gathered",
            Pattern::Shown => "shown",
            Pattern::Never => "never",
        }
    }
}

/// The lists a group of your contacts may be on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum List {
    Safe,
    /// The list of your contacts (`neutral-senders.txt`), kept from an older page.
    Contacts,
    Restricted,
    Blocked,
}

impl List {
    pub const ALL: [List; 4] = [List::Safe, List::Contacts, List::Restricted, List::Blocked];
    /// In the order the senders' lists decide a tie (`porch::Senders::lists`).
    pub const DECIDING: [List; 4] = [List::Blocked, List::Restricted, List::Contacts, List::Safe];

    pub fn standing(self) -> Standing {
        match self {
            List::Safe => Standing::Safe,
            List::Contacts => Standing::Neutral,
            List::Restricted => Standing::Restricted,
            List::Blocked => Standing::Blocked,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            List::Safe => "safe",
            List::Contacts => "contacts",
            List::Restricted => "restricted",
            List::Blocked => "blocked",
        }
    }

    /// The circle a list puts its people in.
    pub fn person(self) -> Person {
        match self {
            List::Safe => Person::Safe,
            List::Contacts => Person::Neutral,
            List::Restricted => Person::Restricted,
            List::Blocked => Person::Blocked,
        }
    }

    fn path(self, config: &Config) -> PathBuf {
        match self {
            List::Safe => config.safe_senders_path(),
            List::Contacts => config.neutral_senders_path(),
            List::Restricted => config.restricted_senders_path(),
            List::Blocked => config.blocked_senders_path(),
        }
    }
}

/// When it reaches you.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum When {
    Pattern { pattern: Pattern },
    /// Your own times: a level per moment, W A L M S P F Y D; for a person,
    /// an app or a conversation, `as` where the usual still holds.
    Own { cells: [Level; 9] },
    /// A group of your contacts: as the people of a list.
    Like { list: List },
    /// A phone's own table of calls: its profile's id (`calls_line::Profile`;
    /// "none" for a table changed cell by cell on the phone).
    Calls { profile: String },
    /// What an app is, on this phone (`appnotes::AppKind`), and what it is
    /// for ("work", "admin+leisure"; "" for anything).
    Is {
        app: AppKind,
        #[serde(default)]
        area: String,
    },
}

impl When {
    pub fn pattern(pattern: Pattern) -> When {
        When::Pattern { pattern }
    }
}

/// Where an exception is kept: the store it was read from, and the one it is written to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Store {
    /// Rows of the matrix in `[attention]`, by id ("mail.neutral", "before").
    Rows { rows: Vec<String> },
    /// A source's own row in `[attention]` ("person.contact:…", "app.…", "conversation.…").
    Source { id: String },
    /// A group's line on the senders' lists (`category:<name>`).
    Group { name: String },
    /// A phone's own table of calls (`[device."<id>"] calls`).
    Phone { id: String },
    /// A switch of Sioul's own notices ("reminders.mail", "reminders.mail_newsletters", "reminders.gather").
    Switch { key: String },
    /// This phone's choice for a conversation (`app-notes.toml`, `through`).
    Talk { key: String },
    /// This phone's choice for an app (`app-notes.toml`, its kind and area).
    App { package: String },
}

/// A difference from the usual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exception {
    pub what: What,
    pub whom: Whom,
    #[serde(rename = "where")]
    pub place: Place,
    pub when: When,
    /// Where it is kept; none for one being made (`store_of` finds it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<Store>,
}

/// A device, as the caller knows it: its id, its name, whether it is a phone, whether it is this one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub phone: bool,
    pub here: bool,
}

/// Everything an exception is read from.
pub struct Stores<'a> {
    /// The matrix as the configuration says (`Attention::of`).
    pub attention: &'a Attention,
    /// The configuration: the lists' paths, the switches, the phones' lines.
    pub config: &'a Config,
    /// Your devices, as the sharing knows them, this one among them.
    pub devices: &'a [Device],
    /// This phone's own choices for other apps; none on a computer.
    pub apps: Option<&'a Choices>,
    /// A person's name by the id their own row writes (their card's); "" when unknown.
    pub name: &'a dyn Fn(&str) -> String,
}

/// The switches of Sioul's own notices an exception may stand for, with
/// their usual value: new mail told, newsletters not, sites' gathered notice made.
pub const SWITCHES: [(&str, bool); 3] = [("reminders.mail", true), ("reminders.mail_newsletters", false), ("reminders.gather", true)];

/// A switch as the file writes it: none where it says nothing (Sioul's own value holds).
fn switch_written(path: &std::path::Path, key: &str) -> Option<bool> {
    let (table, field) = key.split_once('.')?;
    let text = crate::files::read_to_string(path).ok()?;
    text.parse::<toml::Table>().ok()?.get(table)?.get(field)?.as_bool()
}

/// A switch written, or taken out (none), among its section's keys in the
/// order `SWITCHES` lists them (`config::set_table_ranked`); the section
/// taken out once it holds nothing.
fn write_switch(path: &std::path::Path, key: &str, on: Option<bool>) -> Result<(), String> {
    let (table, field) = key.split_once('.').ok_or_else(|| format!("{key}: no such switch"))?;
    let rank = |k: &str| SWITCHES.iter().position(|(s, _)| s.split_once('.').is_some_and(|(t, f)| t == table && f == k)).map(|i| (i as u32, String::new()));
    crate::config::set_table_ranked(path, table, &[(field.to_string(), on.map(toml_edit::value))], &[], &rank)?;
    if on.is_none() {
        crate::config::drop_empty_table(path, table)?;
    }
    Ok(())
}

fn switch_now(config: &Config, key: &str) -> bool {
    match key {
        "reminders.mail" => config.reminders.mail,
        "reminders.mail_newsletters" => config.reminders.mail_newsletters,
        "reminders.gather" => config.reminders.gather,
        _ => false,
    }
}

// ---------------------------------------------------------------- the start

/// The start the matrix reads from: the one chosen, as `[attention] start`
/// keeps it (`stored_start`); for settings from before it was kept, As
/// Sioul does, as a Sioul until 0.1.0 wrote them (their changed rows are
/// exceptions over it). Choosing a start writes its rows and keeps it
/// (`choosing`); a row changed after is an exception over it, and taking
/// that exception out puts the row back as this start has it.
pub fn start_of(config: &Config, attention: &Attention) -> Preset {
    let _ = attention;
    stored_start(config).unwrap_or(Preset::Usual)
}

/// The start kept in `[attention]` (`attention::START`); none when it says nothing.
pub fn stored_start(config: &Config) -> Option<Preset> {
    let (_, value) = config.attention.as_ref()?.0.iter().find(|(key, _)| key.trim() == attention::START)?;
    Preset::read(value.as_str()?)
}

/// The rows a start changed from another sets differently: written as the
/// new start has them when it is chosen (`starting`); the exceptions kept on
/// them give way (`gives_way`).
pub fn start_rows(from: Preset, to: Preset) -> Vec<Row> {
    let (a, b) = (from.matrix(), to.matrix());
    Row::ALL.into_iter().filter(|r| attention::preset_sets(*r) && Column::ALL.iter().any(|c| a.cell(*r, *c) != b.cell(*r, *c))).collect()
}

/// The exceptions (by their place in `list`) that a new start replaces:
/// those kept on a row it sets differently.
pub fn gives_way(list: &[Exception], from: Preset, to: Preset) -> Vec<usize> {
    let rows: Vec<String> = start_rows(from, to).iter().map(|r| r.id()).collect();
    list.iter()
        .enumerate()
        .filter(|(_, e)| matches!(&e.store, Some(Store::Rows { rows: kept }) if kept.iter().any(|r| rows.contains(r))))
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------- patterns on rows

/// Whether `row` may take these cells: each one offered, a fixed one unchanged.
fn holds(row: Row, cells: &[Level; 9]) -> bool {
    let usual = Attention::usual();
    Column::ALL.iter().zip(cells).all(|(column, level)| match attention::lock(row, *column) {
        Some(_) => usual.cell(row, *column) == *level,
        None => attention::choices(row, *column).contains(level),
    })
}

/// The cells of `pattern` on a row of the matrix; none where the row cannot
/// take it. Mail and messages as Settings has always read them
/// (`behaviours::pattern`); calls as messages, held being voicemail; Sioul's
/// own kinds at once in their pattern's moments, else later, or not at all
/// where they cannot wait.
pub fn cells(row: Row, pattern: Pattern) -> Option<[Level; 9]> {
    use Level::{Gathered, Later, Now};
    use crate::behaviours::Behaviour;
    let cells = match row {
        Row::People(_, Person::Blocked) => return None,
        Row::People(Channel::Mail | Channel::Messages, _) => {
            let mail = matches!(row, Row::People(Channel::Mail, _));
            let behaviour = match pattern {
                Pattern::Always => Behaviour::Always,
                Pattern::Awake => Behaviour::Awake,
                Pattern::WorkAdminGathered => Behaviour::WorkAdminGathered,
                Pattern::WorkAdmin => Behaviour::WorkAdmin,
                Pattern::Work => Behaviour::Work,
                Pattern::Gathered => return None,
                // Mail on the Porch, never told; a message cannot be shown.
                Pattern::Shown if mail => Behaviour::Never,
                Pattern::Shown => return None,
                // Mail in its lane, never told; a message held for good.
                Pattern::Never if mail => {
                    let cells = [Level::Never; 9];
                    return holds(row, &cells).then_some(cells);
                }
                Pattern::Never => Behaviour::Never,
            };
            return crate::behaviours::pattern(row, behaviour);
        }
        Row::People(Channel::Calls, person) => {
            if attention::screened(row) {
                return None;
            }
            let free = if person.safe() { Now } else { Later };
            match pattern {
                Pattern::Always => [Now; 9],
                Pattern::Awake => [Now, Now, Now, Now, Later, Later, free, Now, Later],
                Pattern::WorkAdmin => [Now, Now, Later, Later, Later, Later, Later, Now, Later],
                Pattern::Work => [Now, Later, Later, Later, Later, Later, Later, Now, Later],
                Pattern::Never => [Later; 9],
                Pattern::WorkAdminGathered | Pattern::Gathered | Pattern::Shown => return None,
            }
        }
        Row::Own(_) => {
            let off = |column: Column| {
                let offered = attention::choices(row, column);
                if offered.contains(&Later) { Later } else { Level::Never }
            };
            let at = |moments: &[Column], level: Level| -> [Level; 9] { Column::ALL.map(|c| if moments.contains(&c) { level } else { off(c) }) };
            use Column::*;
            match pattern {
                Pattern::Always => [Now; 9],
                Pattern::Awake => at(&[Work, Admin, Leisure, Meals, Slot, Dnd], Now),
                Pattern::WorkAdmin => at(&[Work, Admin], Now),
                Pattern::Work => at(&[Work], Now),
                Pattern::Gathered => {
                    let cells = at(&[Work, Admin, Leisure, Meals], Gathered);
                    cells.map(|l| if l == Level::Never { Later } else { l })
                }
                Pattern::Never => Column::ALL.map(off),
                Pattern::WorkAdminGathered | Pattern::Shown => return None,
            }
        }
    };
    holds(row, &cells).then_some(cells)
}

/// The cells of `pattern` on a source's own row (a person's, an app's, a
/// conversation's): what each of its cells may take; none where it cannot.
pub fn source_cells(id: &str, pattern: Pattern) -> Option<[Level; 9]> {
    use Level::{Gathered, Later, Now, Quiet};
    let person = attention::is_person(id);
    let cells = match pattern {
        Pattern::Always => [Now; 9],
        Pattern::Awake if person => [Now, Now, Now, Now, Later, Later, Now, Now, Quiet],
        Pattern::Awake => [Now, Now, Now, Now, Later, Later, Later, Now, Later],
        Pattern::WorkAdmin => [Now, Now, Later, Later, Later, Later, Later, Now, Later],
        Pattern::Work => [Now, Later, Later, Later, Later, Later, Later, Now, Later],
        Pattern::Gathered if !person => [Gathered, Gathered, Gathered, Gathered, Later, Later, Later, Later, Later],
        Pattern::Never => [Later; 9],
        _ => return None,
    };
    let fits = |column: Column, level: Level| if person { attention::person_choices(column).contains(&level) } else { attention::SOURCE_CHOICES.contains(&level) };
    Column::ALL.iter().zip(cells).all(|(c, l)| fits(*c, l)).then_some(cells)
}

/// A row's cells read as a pattern, else as your own times.
pub fn reading(row: Row, cells: &[Level; 9]) -> When {
    Pattern::ALL.into_iter().find(|p| cells_fit(row, *p, cells)).map_or(When::Own { cells: *cells }, When::pattern)
}

/// Whether a row's cells act as a pattern at every moment, each layer on or
/// off (`behaviours::same_effect`).
fn cells_fit(row: Row, pattern: Pattern, written: &[Level; 9]) -> bool {
    cells(row, pattern).is_some_and(|p| crate::behaviours::same_effect(written, &p))
}

/// A source's cells read as a pattern (each of its cells set, the very
/// pattern's), else as your own times.
pub fn source_reading(id: &str, cells: &[Level; 9]) -> When {
    Pattern::ALL.into_iter().find(|p| source_cells(id, *p).as_ref() == Some(cells)).map_or(When::Own { cells: *cells }, When::pattern)
}

// ---------------------------------------------------------------- reading the stores

/// The channels a circle's "anything" covers: mail and messages, and calls
/// where its row is no call screening's (numbers nobody knows, hidden numbers).
fn channels_of(person: Person) -> Vec<Channel> {
    Channel::ALL.into_iter().filter(|c| attention::persons(*c).contains(&person) && !attention::screened(Row::People(*c, person))).collect()
}

/// The rows an exception on a circle writes: its channel's row, your contacts'
/// messages with the group conversations' (as they ship and every preset moves them).
pub fn rows_of(what: &What, person: Person) -> Vec<Row> {
    let channels = match what {
        // Group conversations are messages only: "anything" from them is no row.
        What::Anything if person == Person::Groups => Vec::new(),
        What::Anything => channels_of(person),
        What::Channel { channel } if attention::persons(*channel).contains(&person) && !attention::screened(Row::People(*channel, person)) => vec![*channel],
        _ => Vec::new(),
    };
    let mut rows: Vec<Row> = channels.iter().map(|c| Row::People(*c, person)).collect();
    if person == Person::Neutral && channels.contains(&Channel::Messages) {
        rows.push(Row::People(Channel::Messages, Person::Groups));
    }
    rows
}

/// Whether a row differs from the start: a cell that is not the start's. Your
/// people's row that follows Safe's never does.
fn differs(attention: &Attention, start: &Attention, row: Row) -> bool {
    !attention.follows(row) && Column::ALL.iter().any(|c| attention.cell(row, *c) != start.cell(row, *c))
}

fn row_cells(attention: &Attention, row: Row) -> [Level; 9] {
    Column::ALL.map(|c| attention.cell(row, c))
}

/// The circles of the people channels, in the order they are said; the blocked aside.
pub const CIRCLES: [Person; 7] = [Person::Always, Person::People, Person::Safe, Person::Neutral, Person::Restricted, Person::Stranger, Person::Groups];

/// Every exception the stores hold, in the order the page lists them: the
/// circles', the groups', the people's, the apps' and conversations', the
/// phones' calls, Sioul's own notices.
pub fn list(stores: &Stores) -> Vec<Exception> {
    let attention = stores.attention;
    let start = start_of(stores.config, attention).matrix();
    let everywhere = || Place::Everywhere;
    let mut out: Vec<Exception> = Vec::new();
    // The circles: one exception for all their channels when each differs and reads one pattern.
    for person in CIRCLES {
        let channels: Vec<Channel> = if person == Person::Groups { vec![Channel::Messages] } else { channels_of(person) };
        let differing: Vec<Row> = channels.iter().map(|c| Row::People(*c, person)).filter(|r| differs(attention, &start, *r)).collect();
        if differing.is_empty() {
            continue;
        }
        let read: Vec<When> = differing.iter().map(|r| reading(*r, &row_cells(attention, *r))).collect();
        let whom = Whom::Circle { person };
        if person != Person::Groups && differing.len() == channels.len() && channels.len() > 1 && read.iter().all(|w| matches!(w, When::Pattern { .. }) && *w == read[0]) {
            let mut rows = differing.clone();
            // Your contacts' messages carry the group conversations' row when both say the same.
            let groups = Row::People(Channel::Messages, Person::Groups);
            if person == Person::Neutral && differs(attention, &start, groups) && row_cells(attention, groups) == row_cells(attention, Row::People(Channel::Messages, Person::Neutral)) {
                rows.push(groups);
            }
            out.push(Exception { what: What::Anything, whom, place: everywhere(), when: read[0].clone(), store: Some(Store::Rows { rows: rows.iter().map(|r| r.id()).collect() }) });
            continue;
        }
        for (row, when) in differing.iter().zip(read) {
            let Row::People(channel, _) = row else { continue };
            let mut rows = vec![*row];
            let groups = Row::People(Channel::Messages, Person::Groups);
            if person == Person::Neutral && *channel == Channel::Messages && differs(attention, &start, groups) && row_cells(attention, groups) == row_cells(attention, *row) {
                rows.push(groups);
            }
            out.push(Exception { what: What::Channel { channel: *channel }, whom: whom.clone(), place: everywhere(), when, store: Some(Store::Rows { rows: rows.iter().map(|r| r.id()).collect() }) });
        }
    }
    // The group conversations' row said apart when your contacts' messages did not carry it.
    let carried = |id: &str| out.iter().any(|e| matches!(&e.store, Some(Store::Rows { rows }) if rows.len() > 1 && rows.iter().any(|r| r == id)));
    if carried("messages.groups") {
        out.retain(|e| !(e.whom == Whom::Circle { person: Person::Groups }));
    }
    // A group of your contacts on a list; on two (two devices at once), the
    // one the lists decide by (`porch::Senders`: blocked, restricted, your
    // contacts, then safe).
    for list in List::DECIDING {
        for name in SenderList::load(&list.path(stores.config)).categories() {
            if out.iter().any(|e| matches!(&e.whom, Whom::Group { name: n } if crate::porch::category_key(n) == crate::porch::category_key(name))) {
                continue;
            }
            out.push(Exception { what: What::Anything, whom: Whom::Group { name: name.clone() }, place: everywhere(), when: When::Like { list }, store: Some(Store::Group { name: name.clone() }) });
        }
    }
    // A person's, an app's and a conversation's own rows, shared with the settings.
    for (id, cells) in attention.sources() {
        if let Some(who) = id.strip_prefix(attention::PERSON_ROW) {
            out.push(Exception { what: What::Anything, whom: Whom::Person { id: who.to_string(), name: (stores.name)(who) }, place: everywhere(), when: source_reading(id, cells), store: Some(Store::Source { id: id.clone() }) });
        }
    }
    for (id, cells) in attention.sources() {
        let what = if let Some(package) = id.strip_prefix(attention::APP_ROW) {
            What::App { package: package.to_string(), name: attention.source_name(id).to_string() }
        } else if let Some(key) = id.strip_prefix(attention::CONVERSATION_ROW) {
            let talk = stores.apps.and_then(|a| a.conversation.get(key));
            What::Conversation { key: key.to_string(), title: talk.map(|t| t.title.clone()).unwrap_or_default(), app: talk.map(|t| t.app.clone()).unwrap_or_default() }
        } else {
            continue;
        };
        out.push(Exception { what, whom: Whom::Anyone, place: everywhere(), when: source_reading(id, cells), store: Some(Store::Source { id: id.clone() }) });
    }
    // This phone's own choices for other apps.
    if let Some(apps) = stores.apps {
        let here = stores.devices.iter().find(|d| d.here).map(|d| Place::Device { id: d.id.clone(), name: d.name.clone() }).unwrap_or(Place::Everywhere);
        for (package, choice) in &apps.app {
            if choice.kind == AppKind::Usual && choice.area.is_none() {
                continue;
            }
            let when = When::Is { app: choice.kind, area: choice.area.map(|a| a.id()).unwrap_or_default() };
            out.push(Exception { what: What::App { package: package.clone(), name: choice.label.clone() }, whom: Whom::Anyone, place: here.clone(), when, store: Some(Store::App { package: package.clone() }) });
        }
        for (key, talk) in &apps.conversation {
            let pattern = match talk.through {
                Through::Usual => continue,
                Through::Always => Pattern::Always,
                Through::Gathered => Pattern::Gathered,
                Through::Never => Pattern::Never,
            };
            out.push(Exception {
                what: What::Conversation { key: key.clone(), title: talk.title.clone(), app: talk.app.clone() },
                whom: Whom::Anyone,
                place: here.clone(),
                when: When::pattern(pattern),
                store: Some(Store::Talk { key: key.clone() }),
            });
        }
    }
    // A phone's own table of calls.
    for (id, said) in &stores.config.devices {
        let Some(calls) = &said.calls else { continue };
        let name = stores.devices.iter().find(|d| d.id == *id).map(|d| d.name.clone()).or_else(|| said.name().map(str::to_string)).unwrap_or_default();
        out.push(Exception {
            what: What::Channel { channel: Channel::Calls },
            whom: Whom::Anyone,
            place: Place::Device { id: id.clone(), name },
            when: When::Calls { profile: calls.grid.profile.id().to_string() },
            store: Some(Store::Phone { id: id.clone() }),
        });
    }
    // Sioul's own notices: a kind's row, then the switches.
    for kind in Kind::ALL {
        let row = Row::Own(kind);
        if differs(attention, &start, row) {
            out.push(Exception { what: What::Own { notice: Notice::Kind(kind) }, whom: Whom::Anyone, place: everywhere(), when: reading(row, &row_cells(attention, row)), store: Some(Store::Rows { rows: vec![row.id()] }) });
        }
    }
    for (key, usual) in SWITCHES {
        if switch_now(stores.config, key) == usual {
            continue;
        }
        let (notice, pattern) = match key {
            "reminders.mail" => (Notice::NewMail, Pattern::Never),
            "reminders.mail_newsletters" => (Notice::Newsletters, Pattern::Always),
            _ => (Notice::Kind(Kind::Sites), Pattern::Never),
        };
        out.push(Exception { what: What::Own { notice }, whom: Whom::Anyone, place: everywhere(), when: When::pattern(pattern), store: Some(Store::Switch { key: key.to_string() }) });
    }
    out
}

// ---------------------------------------------------------------- which line decides

/// What is asked: a channel's or one of Sioul's own, from someone, on a device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub what: Asked,
    /// Their own row's id when known (`contact:<UID>`, `tel:<key>`, an address).
    pub person: Option<String>,
    /// The groups of their card.
    pub groups: Vec<String>,
    /// Their circle: the row their list puts them on.
    pub circle: Person,
    /// The device it reaches; none for every device.
    pub device: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    Channel(Channel),
    /// A notification of an app (a message, on a phone).
    App(String),
    /// A conversation's key and its app's package.
    Conversation(String, String),
    Own(Kind),
}

impl Ask {
    /// A circle on a channel, on every device: a cell of the worked-out grid.
    pub fn circle(channel: Channel, circle: Person) -> Ask {
        Ask { what: Asked::Channel(channel), person: None, groups: Vec::new(), circle, device: None }
    }

    /// One of Sioul's own kinds, on every device.
    pub fn own(kind: Kind) -> Ask {
        Ask { what: Asked::Own(kind), person: None, groups: Vec::new(), circle: Person::Neutral, device: None }
    }

    /// A row of the matrix, on every device, as the matrix reads it: your
    /// people's mail or messages not written go as Safe's stand
    /// (`Attention::follows`), so Safe's lines decide them.
    pub fn row(attention: &Attention, row: Row) -> Ask {
        match row {
            Row::People(channel, Person::People) if attention.follows(row) => Ask::circle(channel, Person::Safe),
            Row::People(channel, person) => Ask::circle(channel, person),
            Row::Own(kind) => Ask::own(kind),
        }
    }

    fn channel(&self) -> Option<Channel> {
        match &self.what {
            Asked::Channel(channel) => Some(*channel),
            Asked::App(_) | Asked::Conversation(..) => Some(Channel::Messages),
            Asked::Own(_) => None,
        }
    }
}

/// How precise an exception is, the most precise first: a conversation, a
/// person, an app, a group, a circle on a channel (a phone's calls, a kind),
/// a circle on anything; anyone last.
fn precision(e: &Exception) -> u8 {
    match (&e.what, &e.whom) {
        (What::Conversation { .. }, _) => 0,
        (_, Whom::Person { .. }) => 1,
        (What::App { .. }, _) => 2,
        (_, Whom::Group { .. }) => 3,
        (What::Channel { .. } | What::Own { .. }, _) => 4,
        (What::Anything, Whom::Circle { .. }) => 5,
        _ => 6,
    }
}

/// Whether an exception speaks of what is asked.
fn covers(e: &Exception, ask: &Ask) -> bool {
    let what = match &e.what {
        What::Conversation { key, .. } => matches!(&ask.what, Asked::Conversation(k, _) if k == key),
        What::App { package, .. } => matches!(&ask.what, Asked::App(p) | Asked::Conversation(_, p) if p == package),
        What::Own { notice: Notice::Kind(kind) } => ask.what == Asked::Own(*kind),
        What::Own { .. } => false,
        // A person's own row holds their calls and messages, never their mail.
        What::Anything if matches!(e.whom, Whom::Person { .. }) => matches!(ask.channel(), Some(Channel::Calls | Channel::Messages)),
        What::Anything => ask.channel().is_some(),
        What::Channel { channel } => ask.channel() == Some(*channel),
    };
    let whom = match &e.whom {
        Whom::Anyone => match &e.when {
            // A phone's table holds the circles it has rows for: Always through and your people ring as the matrix says.
            When::Calls { .. } => !matches!(ask.circle, Person::Always | Person::People | Person::Blocked),
            _ => true,
        },
        // A circle's line, and the rows it carries (your contacts' messages carry the group conversations').
        Whom::Circle { person } => ask.circle == *person || matches!((&e.store, ask.channel()), (Some(Store::Rows { rows }), Some(channel)) if rows.contains(&Row::People(channel, ask.circle).id())),
        Whom::Group { name } => ask.groups.iter().any(|g| crate::porch::category_key(g) == crate::porch::category_key(name)),
        Whom::Person { id, .. } => ask.person.as_deref() == Some(id.as_str()),
    };
    let place = match &e.place {
        Place::Everywhere => true,
        Place::Device { id, .. } => ask.device.as_deref() == Some(id.as_str()),
    };
    what && whom && place
}

/// Whether an exception says something of this moment: your own times say
/// nothing where they say as usual.
fn says(e: &Exception, column: Column) -> bool {
    match &e.when {
        When::Own { cells } => cells[Column::ALL.iter().position(|c| *c == column).unwrap_or(0)] != Level::As,
        _ => true,
    }
}

/// Which line decides what is asked at a moment: the place in `list` of the
/// most precise exception that speaks of it then (a conversation, a person,
/// an app, a group, a circle; one device before every device); none: the
/// usual rule.
pub fn decide(list: &[Exception], ask: &Ask, column: Column) -> Option<usize> {
    let mut candidates: Vec<(u8, u8, usize)> = list
        .iter()
        .enumerate()
        .filter(|(_, e)| covers(e, ask) && says(e, column))
        .map(|(i, e)| (precision(e), u8::from(matches!(e.place, Place::Everywhere)), i))
        .collect();
    candidates.sort();
    candidates.first().map(|(_, _, i)| *i)
}

// ---------------------------------------------------------------- the card's choices

/// Where an exception goes, from what it names; why not, when nothing keeps it.
pub fn store_of(e: &Exception) -> Result<Store, String> {
    let refused = || format!("{}: not kept anywhere", serde_json::to_string(e).unwrap_or_default());
    match (&e.what, &e.whom, &e.place, &e.when) {
        (What::Channel { channel: Channel::Calls }, Whom::Anyone, Place::Device { id, .. }, When::Calls { .. }) => Ok(Store::Phone { id: id.clone() }),
        (What::App { package, .. }, Whom::Anyone, Place::Device { .. }, When::Is { .. }) => Ok(Store::App { package: package.clone() }),
        (What::Conversation { key, .. }, Whom::Anyone, Place::Device { .. }, When::Pattern { pattern: Pattern::Always | Pattern::Gathered | Pattern::Never }) => Ok(Store::Talk { key: key.clone() }),
        (_, _, Place::Device { .. }, _) => Err(refused()),
        (What::Anything | What::Channel { .. }, Whom::Circle { person }, _, When::Pattern { .. } | When::Own { .. }) => {
            let rows = rows_of(&e.what, *person);
            if rows.is_empty() { Err(refused()) } else { Ok(Store::Rows { rows: rows.iter().map(|r| r.id()).collect() }) }
        }
        (What::Anything, Whom::Group { name }, _, When::Like { .. }) => Ok(Store::Group { name: name.clone() }),
        (What::Anything, Whom::Person { id, .. }, _, When::Pattern { .. } | When::Own { .. }) => Ok(Store::Source { id: attention::person_row(id) }),
        (What::App { package, .. }, Whom::Anyone, _, When::Pattern { .. } | When::Own { .. }) => Ok(Store::Source { id: format!("{}{package}", attention::APP_ROW) }),
        (What::Conversation { key, .. }, Whom::Anyone, _, When::Pattern { .. } | When::Own { .. }) => Ok(Store::Source { id: format!("{}{key}", attention::CONVERSATION_ROW) }),
        (What::Own { notice: Notice::NewMail }, Whom::Anyone, _, When::Pattern { pattern: Pattern::Never }) => Ok(Store::Switch { key: "reminders.mail".into() }),
        (What::Own { notice: Notice::Newsletters }, Whom::Anyone, _, When::Pattern { pattern: Pattern::Always }) => Ok(Store::Switch { key: "reminders.mail_newsletters".into() }),
        (What::Own { notice: Notice::Kind(Kind::Sites) }, Whom::Anyone, _, When::Pattern { pattern: Pattern::Never }) => Ok(Store::Switch { key: "reminders.gather".into() }),
        (What::Own { notice: Notice::Kind(kind) }, Whom::Anyone, _, When::Pattern { .. } | When::Own { .. }) => Ok(Store::Rows { rows: vec![Row::Own(*kind).id()] }),
        _ => Err(refused()),
    }
}

/// Where a new exception goes as the stores stand now: `store_of`, your
/// contacts' messages carrying the group conversations' row only while that
/// row reads as theirs or as the start's (when it reads otherwise it is a
/// line of its own, "Messages in group conversations", left as it is).
pub fn store_in(e: &Exception, attention: &Attention, start: Preset) -> Result<Store, String> {
    let mut rows = match store_of(e)? {
        Store::Rows { rows } => rows,
        other => return Ok(other),
    };
    let groups = Row::People(Channel::Messages, Person::Groups);
    let contacts = Row::People(Channel::Messages, Person::Neutral);
    if rows.len() > 1 && rows.contains(&groups.id()) {
        let own = row_cells(attention, groups);
        if own != row_cells(attention, contacts) && own != row_cells(&start.matrix(), groups) {
            rows.retain(|r| *r != groups.id());
        }
    }
    Ok(Store::Rows { rows })
}

/// Why an exception cannot be kept for the device it names, as the key of
/// the sentence that says so; none when it can. An app's or a
/// conversation's own choice on one device is that phone's alone, kept on
/// it (`app-notes.toml`): this phone only. A table of calls is a phone's.
/// Everything else is the same on every device.
pub fn place_refused(e: &Exception, devices: &[Device]) -> Option<&'static str> {
    let Place::Device { id, .. } = &e.place else { return None };
    let device = devices.iter().find(|d| d.id == *id);
    match &e.what {
        What::App { .. } | What::Conversation { .. } => (!device.is_some_and(|d| d.here && d.phone)).then_some("reach-where-this-phone"),
        What::Channel { channel: Channel::Calls } if e.whom == Whom::Anyone => (!device.is_some_and(|d| d.phone)).then_some("reach-where-phones"),
        _ => Some("reach-where-every"),
    }
}

/// Whether a new exception would write what the start already says: rows
/// as the start has them, a source's row as usual everywhere.
pub fn same_as_start(e: &Exception, store: &Store, start: Preset) -> bool {
    let matrix = start.matrix();
    match store {
        Store::Rows { rows } => rows.iter().filter_map(|r| Row::read(r)).all(|row| when_cells(&e.when, row, &matrix).is_ok_and(|cells| cells == row_cells(&matrix, row))),
        Store::Source { .. } => matches!(&e.when, When::Own { cells } if cells.iter().all(|l| *l == Level::As)),
        _ => false,
    }
}

/// The choices of "When it reaches you" for what is chosen (`offered`), and
/// the exception's own when it is changed and the card cannot say it (a
/// phone's table changed cell by cell, an app's part of life): kept as its
/// own choice while what, from whom and where stay its own, so that Edit
/// never rewrites what it cannot show.
pub fn offered_for(what: &What, whom: &Whom, place: &Place, old: Option<&Exception>) -> Vec<When> {
    let mut out = offered(what, whom, place);
    if let Some(old) = old.filter(|o| o.what == *what && o.whom == *whom && o.place == *place && !matches!(o.when, When::Own { .. }))
        && !out.contains(&old.when)
    {
        out.insert(0, old.when.clone());
    }
    out
}

/// The patterns an exception may take for what it names, in the card's order;
/// none for a choice not made yet (a circle with no channel, anyone on a channel).
pub fn offered(what: &What, whom: &Whom, place: &Place) -> Vec<When> {
    let probe = |when: When| Exception { what: what.clone(), whom: whom.clone(), place: place.clone(), when, store: None };
    match (what, whom, place) {
        (What::Channel { channel: Channel::Calls }, Whom::Anyone, Place::Device { .. }) => Profile::OFFERED.iter().map(|p| When::Calls { profile: p.id().to_string() }).collect(),
        (What::Anything, Whom::Group { .. }, Place::Everywhere) => List::ALL.iter().map(|l| When::Like { list: *l }).collect(),
        (What::App { .. }, Whom::Anyone, Place::Device { .. }) => [AppKind::People, AppKind::Automaton, AppKind::AtOnce].iter().map(|k| When::Is { app: *k, area: String::new() }).collect(),
        (What::Own { notice: Notice::NewMail }, Whom::Anyone, Place::Everywhere) => vec![When::pattern(Pattern::Never)],
        (What::Own { notice: Notice::Newsletters }, Whom::Anyone, Place::Everywhere) => vec![When::pattern(Pattern::Always)],
        _ => {
            // "Never" for a site's gathered news is its switch (`store_of`), offered as the pattern.
            Pattern::ALL
                .into_iter()
                .filter(|p| {
                    let e = probe(When::pattern(*p));
                    match store_of(&e) {
                        Ok(Store::Rows { rows }) => rows.iter().filter_map(|r| Row::read(r)).all(|r| cells(r, *p).is_some()),
                        Ok(Store::Source { id }) => source_cells(&id, *p).is_some(),
                        Ok(_) => true,
                        Err(_) => false,
                    }
                })
                .map(When::pattern)
                .collect()
        }
    }
}

/// Whether your own times (a cell per moment) may be set for what it names.
pub fn own_times(what: &What, whom: &Whom, place: &Place) -> bool {
    let e = Exception { what: what.clone(), whom: whom.clone(), place: place.clone(), when: When::Own { cells: [Level::Now; 9] }, store: None };
    matches!(store_of(&e), Ok(Store::Rows { .. } | Store::Source { .. }))
}

/// The levels each of your own times may take, by moment, for what it
/// names: every row's choices at once (a circle's several rows), a source's.
pub fn own_choices(e: &Exception) -> Vec<Vec<Level>> {
    match store_of(e) {
        Ok(Store::Rows { rows }) => {
            let rows: Vec<Row> = rows.iter().filter_map(|r| Row::read(r)).collect();
            Column::ALL
                .iter()
                .map(|c| {
                    let mut levels: Vec<Level> = rows.first().map(|r| attention::choices(*r, *c).to_vec()).unwrap_or_default();
                    levels.retain(|l| rows.iter().all(|r| attention::choices(*r, *c).contains(l) && attention::lock(*r, *c).is_none_or(|_| Attention::usual().cell(*r, *c) == *l)));
                    levels
                })
                .collect()
        }
        Ok(Store::Source { id }) if attention::is_person(&id) => Column::ALL.iter().map(|c| attention::person_choices(*c).to_vec()).collect(),
        Ok(Store::Source { .. }) => Column::ALL.iter().map(|_| attention::SOURCE_CHOICES.to_vec()).collect(),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------- writing

/// One write in the stores of today, in the shapes of today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Write {
    /// Rows of the matrix, each cell set (`attention::apply_rows`).
    Rows(Vec<(Row, [Level; 9])>),
    /// Rows put back as the start has them, your people's following Safe's
    /// again where the start's do (`attention::apply_back`).
    Back(Vec<Row>),
    /// A source's row, its nine words (`as` where the usual holds; all `as`:
    /// taken out), and an app's name, kept beside it for the devices that never saw it.
    Source(String, [Level; 9], String),
    /// A group's list; none: on no list.
    Group(String, Option<Standing>),
    /// A phone's table of calls, by its profile; none: the line taken out.
    Phone(String, Option<Profile>),
    Switch(String, bool),
    /// This phone's choice for a conversation.
    Talk(String, Through),
    /// This phone's choice for an app: its kind, its area, its name as last seen.
    App(String, AppKind, Option<Area>, String),
    /// The start chosen, kept (`attention::START`); only once `[attention]`
    /// is written, so that the older keys still seed the matrix until then.
    Start(Preset),
}

/// The cells an exception's `when` writes on a row; your own times' `as`
/// stands for the start's cell there.
fn when_cells(when: &When, row: Row, start: &Attention) -> Result<[Level; 9], String> {
    match when {
        When::Pattern { pattern } => cells(row, *pattern).ok_or_else(|| format!("{}: {}: not offered", row.id(), pattern.id())),
        When::Own { cells } => {
            let written = Column::ALL.map(|c| {
                let at = Column::ALL.iter().position(|x| *x == c).unwrap_or(0);
                if cells[at] == Level::As && !row.always() { start.cell(row, c) } else { cells[at] }
            });
            if holds(row, &written) { Ok(written) } else { Err(format!("{}: {}: not offered", row.id(), written.map(|l| l.id()).join(" "))) }
        }
        _ => Err(format!("{}: not a time", row.id())),
    }
}

/// What making an exception writes.
pub fn adding(e: &Exception, start: Preset) -> Result<Vec<Write>, String> {
    let store = match &e.store {
        Some(store) => store.clone(),
        None => store_of(e)?,
    };
    let start = start.matrix();
    Ok(vec![match store {
        Store::Rows { rows } => {
            let mut out = Vec::new();
            for id in rows {
                let row = Row::read(&id).ok_or_else(|| format!("{id}: no such row"))?;
                out.push((row, when_cells(&e.when, row, &start)?));
            }
            Write::Rows(out)
        }
        Store::Source { id } => {
            let cells = match &e.when {
                When::Pattern { pattern } => source_cells(&id, *pattern).ok_or_else(|| format!("{id}: {}: not offered", pattern.id()))?,
                When::Own { cells } => *cells,
                _ => return Err(format!("{id}: not a time")),
            };
            let name = match &e.what {
                What::App { name, .. } => name.clone(),
                _ => String::new(),
            };
            Write::Source(id, cells, name)
        }
        Store::Group { name } => match &e.when {
            When::Like { list } => Write::Group(name, Some(list.standing())),
            _ => return Err(format!("{name}: a list is needed")),
        },
        Store::Phone { id } => match &e.when {
            When::Calls { profile } => Write::Phone(id, Some(Profile::read(profile).filter(|p| *p != Profile::Own).ok_or_else(|| format!("{profile}: no such profile"))?)),
            _ => return Err(format!("{id}: a profile is needed")),
        },
        Store::Switch { key } => {
            let usual = SWITCHES.iter().find(|(k, _)| *k == key).map(|(_, u)| *u).ok_or_else(|| format!("{key}: no such switch"))?;
            Write::Switch(key, !usual)
        }
        Store::Talk { key } => match &e.when {
            When::Pattern { pattern: Pattern::Always } => Write::Talk(key, Through::Always),
            When::Pattern { pattern: Pattern::Gathered } => Write::Talk(key, Through::Gathered),
            When::Pattern { pattern: Pattern::Never } => Write::Talk(key, Through::Never),
            _ => return Err(format!("{key}: always, gathered or never")),
        },
        Store::App { package } => match &e.when {
            When::Is { app, area } => Write::App(package, *app, Area::parse(area), match &e.what {
                What::App { name, .. } => name.clone(),
                _ => String::new(),
            }),
            _ => return Err(format!("{package}: what it is is needed")),
        },
    }])
}

/// What taking an exception out writes: each store back as the start has it.
pub fn removing(e: &Exception) -> Result<Vec<Write>, String> {
    let store = match &e.store {
        Some(store) => store.clone(),
        None => store_of(e)?,
    };
    Ok(vec![match store {
        Store::Rows { rows } => Write::Back(rows.iter().filter_map(|r| Row::read(r)).collect()),
        Store::Source { id } => Write::Source(id, [Level::As; 9], String::new()),
        Store::Group { name } => Write::Group(name, None),
        Store::Phone { id } => Write::Phone(id, None),
        Store::Switch { key } => {
            let usual = SWITCHES.iter().find(|(k, _)| *k == key).map(|(_, u)| *u).ok_or_else(|| format!("{key}: no such switch"))?;
            Write::Switch(key, usual)
        }
        Store::Talk { key } => Write::Talk(key, Through::Usual),
        Store::App { package } => Write::App(package, AppKind::Usual, None, String::new()),
    }])
}

/// The rows a start chosen writes: each it sets differently from the start
/// before, as it has them; then the start, kept.
pub fn starting(from: Preset, to: Preset) -> Vec<Write> {
    vec![Write::Back(start_rows(from, to)), Write::Start(to)]
}

/// What choosing a start on the page writes: `starting`; the start alone
/// when it is the one read already (settings that never kept one read as
/// As Sioul does: choosing it keeps it, every row as it was).
pub fn choosing(from: Preset, to: Preset) -> Vec<Write> {
    if from == to { vec![Write::Start(to)] } else { starting(from, to) }
}

/// Where the writes land: the configuration, and on a phone its own choices for other apps.
pub struct Paths {
    pub config: PathBuf,
    pub apps: Option<PathBuf>,
}

/// The writes made, in order, each under its file's lock as the stores of
/// today are written. `to` is the start a row goes back to.
pub fn apply(paths: &Paths, writes: &[Write], to: Preset) -> Result<(), String> {
    let config = || if paths.config.present() { Config::load(&paths.config) } else { Ok(Config::default()) };
    for write in writes {
        match write {
            Write::Rows(rows) => {
                let cells: Vec<(Row, [Option<Level>; 9])> = rows.iter().map(|(r, c)| (*r, c.map(Some))).collect();
                attention::apply_rows(&paths.config, &config()?, &cells)?;
            }
            Write::Back(rows) => attention::apply_back(&paths.config, &config()?, rows, &to.matrix())?,
            Write::Source(id, cells, name) => {
                let mut words: Vec<String> = Column::ALL.iter().zip(cells).map(|(c, l)| format!("{}:{}", c.id(), l.id())).collect();
                // An app's name kept beside its row, for the devices that never saw it: the one written, else the exception's.
                let attention = Attention::of(&config()?);
                let kept = attention.source_name(id).to_string();
                let name = if kept.is_empty() { name.trim().to_string() } else { kept };
                if !name.is_empty() && !attention::is_person(id) && cells.iter().any(|l| *l != Level::As) {
                    words.push(format!("{}{name}", attention::NAME));
                }
                attention::apply_change(&paths.config, &format!("attention.{id}"), None, &SettingValue::Texts(words))?;
            }
            Write::Group(name, standing) => crate::porch::set_category(&config()?, name, *standing)?,
            Write::Phone(id, Some(profile)) => {
                crate::calls_line::set_profile(&paths.config, id, *profile, &profile.grid())?;
            }
            Write::Phone(id, None) => crate::calls_line::put_raw(&paths.config, id, None)?,
            // As Sioul ships it: taken out (its section with it once empty), so that adding then
            // taking out leaves the file as it was.
            Write::Switch(key, on) => {
                let usual = SWITCHES.iter().find(|(k, _)| *k == key.as_str()).map(|(_, u)| *u);
                write_switch(&paths.config, key, (usual != Some(*on)).then_some(*on))?;
            }
            Write::Talk(key, through) => {
                let path = paths.apps.as_ref().ok_or("a conversation's choice is the phone's own")?;
                crate::filelock::with_lock(path, || {
                    let mut choices = Choices::read(path)?;
                    match through {
                        Through::Usual => {
                            if let Some(talk) = choices.conversation.get_mut(key) {
                                talk.through = Through::Usual;
                                if talk.title.is_empty() {
                                    choices.conversation.remove(key);
                                }
                            }
                        }
                        through => choices.conversation.entry(key.clone()).or_default().through = *through,
                    }
                    choices.save(path)
                })?;
            }
            Write::App(package, kind, area, label) => {
                let path = paths.apps.as_ref().ok_or("an app's choice is the phone's own")?;
                crate::filelock::with_lock(path, || {
                    let mut choices = Choices::read(path)?;
                    let mut app = choices.app(package);
                    app.kind = *kind;
                    app.area = *area;
                    if !label.trim().is_empty() {
                        app.label = label.trim().to_string();
                    }
                    if app.is_usual() {
                        choices.app.remove(package);
                    } else {
                        choices.app.insert(package.clone(), app);
                    }
                    choices.save(path)
                })?;
            }
            Write::Start(preset) => {
                // Not before `[attention]` is written: its first write takes the older keys in.
                if config()?.attention.is_some() {
                    let item = toml_edit::value(preset.id());
                    crate::config::set_table_ranked(&paths.config, "attention", &[(attention::START.to_string(), Some(item))], &[], &attention::rank)?;
                }
            }
        }
    }
    Ok(())
}

/// Why a change of the page is not made: a sentence of the window's
/// (`Words`, a key of the messages), or a write that failed, put back, its
/// words for the log alone (`Failed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Words(&'static str),
    Failed(String),
}

/// A change of Settings ▸ What reaches you made whole: `old` taken out,
/// `new` made, or `old` changed into `new` (Edit); with what Undo puts back
/// (`put_back`), none when nothing changes (Edit saved as it was). Refused,
/// in the window's words, when the device named cannot keep it
/// (`place_refused`) or a new one says what the usual rule already says
/// (`same_as_start`); a write that fails puts back what it touched. The
/// start is kept with the first change when settings from before it say
/// none (`stored_start`), so that it never moves under the exceptions.
pub fn save(paths: &Paths, old: Option<&Exception>, new: Option<&Exception>, devices: &[Device]) -> Result<Option<Kept>, Refusal> {
    let failed = |e: String| Refusal::Failed(e);
    let config = if paths.config.present() { Config::load(&paths.config).map_err(failed)? } else { Config::default() };
    let attention = Attention::of(&config);
    let start = start_of(&config, &attention);
    // The card sends what it shows: an exception saved as it was changes nothing.
    let bare = |e: &Exception| Exception { store: None, ..e.clone() };
    if let (Some(old), Some(new)) = (old, new)
        && bare(old) == bare(new)
    {
        return Ok(None);
    }
    let mut stores: Vec<Store> = Vec::new();
    let mut writes: Vec<Write> = Vec::new();
    let new_store = match new {
        Some(new) => {
            if let Some(words) = place_refused(new, devices) {
                return Err(Refusal::Words(words));
            }
            Some(store_in(new, &attention, start).map_err(|_| Refusal::Words("reach-not-kept"))?)
        }
        None => None,
    };
    if let Some(old) = old {
        let old_store = match &old.store {
            Some(store) => store.clone(),
            None => store_of(old).map_err(failed)?,
        };
        // A change that keeps its store writes over it; another takes the old one out first.
        if new_store.as_ref() != Some(&old_store) {
            writes.extend(removing(&Exception { store: Some(old_store.clone()), ..old.clone() }).map_err(failed)?);
        }
        stores.push(old_store);
    }
    if let (Some(new), Some(store)) = (new, new_store) {
        if old.is_none() && same_as_start(new, &store, start) {
            return Err(Refusal::Words("reach-same-as-usual"));
        }
        writes.extend(adding(&Exception { store: Some(store.clone()), ..new.clone() }, start).map_err(|_| Refusal::Words("reach-not-offered"))?);
        if !stores.contains(&store) {
            stores.push(store);
        }
    }
    if stored_start(&config).is_none() {
        writes.push(Write::Start(start));
    }
    let kept = keep(paths, &stores, &[]);
    if let Err(e) = apply(paths, &writes, start) {
        let _ = put_back(paths, &kept);
        return Err(Refusal::Failed(e));
    }
    Ok(Some(kept))
}

// ---------------------------------------------------------------- undo

/// What the stores a change touches held before it, to put back (Undo in
/// the status line): each row's or source's value in `[attention]`, and the
/// start kept there (none: not written; while `[attention]` was never
/// written, a row as the older keys seeded it), each group's lists, each
/// phone's line as written, each switch, this phone's choice for a
/// conversation or an app.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Kept {
    attention: Vec<(String, Option<toml::Value>)>,
    groups: Vec<(String, Vec<List>)>,
    phones: Vec<(String, Option<String>)>,
    switches: Vec<(String, Option<bool>)>,
    talks: Vec<(String, Option<crate::appnotes::TalkChoice>)>,
    apps: Vec<(String, Option<crate::appnotes::AppChoice>)>,
}

/// The `[attention]` keys a store's writes may touch: its rows, and your
/// people's two, which follow Safe's or stop following it.
fn attention_keys(store: &Store) -> Vec<String> {
    match store {
        Store::Rows { rows } => rows.iter().cloned().chain(["mail.people".to_string(), "messages.people".to_string()]).collect(),
        Store::Source { id } => vec![id.clone()],
        _ => Vec::new(),
    }
}

/// What `stores` hold now, and `rows` of the matrix besides (a start's).
pub fn keep(paths: &Paths, stores: &[Store], rows: &[Row]) -> Kept {
    let config = if paths.config.present() { Config::load(&paths.config).unwrap_or_default() } else { Config::default() };
    // Never written: the rows as the older keys seed them, which its first write takes in.
    let seeded = config.attention.is_none().then(|| Attention::of(&config));
    let usual = Attention::usual();
    let written = |key: &str| -> Option<toml::Value> {
        if let Some(seeded) = &seeded {
            let row = Row::read(key).filter(|r| !seeded.follows(*r) && Column::ALL.iter().any(|c| seeded.cell(*r, *c) != usual.cell(*r, *c)))?;
            return Some(toml::Value::Array(seeded.words(row).into_iter().map(toml::Value::String).collect()));
        }
        config.attention.as_ref()?.0.iter().find(|(k, _)| k.trim() == key).map(|(_, v)| v.clone())
    };
    let choices = paths.apps.as_ref().map(|p| Choices::load(p)).unwrap_or_default();
    let mut kept = Kept::default();
    let mut keys: Vec<String> = stores.iter().flat_map(attention_keys).chain(rows.iter().map(|r| r.id())).collect();
    if !rows.is_empty() {
        keys.extend(["mail.people".to_string(), "messages.people".to_string()]);
    }
    // The start, which the first change keeps when the settings said none.
    keys.push(attention::START.to_string());
    for key in keys {
        if !kept.attention.iter().any(|(k, _)| *k == key) {
            let value = written(&key);
            kept.attention.push((key, value));
        }
    }
    for store in stores {
        match store {
            Store::Group { name } => {
                let on: Vec<List> = List::DECIDING.into_iter().filter(|l| SenderList::load(&l.path(&config)).categories().iter().any(|c| crate::porch::category_key(c) == crate::porch::category_key(name))).collect();
                kept.groups.push((name.clone(), on));
            }
            Store::Phone { id } => kept.phones.push((id.clone(), crate::calls_line::raw(&paths.config, id))),
            Store::Switch { key } => kept.switches.push((key.clone(), switch_written(&paths.config, key))),
            Store::Talk { key } => kept.talks.push((key.clone(), choices.conversation.get(key).cloned())),
            Store::App { package } => kept.apps.push((package.clone(), choices.app.get(package).cloned())),
            Store::Rows { .. } | Store::Source { .. } => {}
        }
    }
    kept
}

/// The stores put back as `keep` found them.
pub fn put_back(paths: &Paths, kept: &Kept) -> Result<(), String> {
    let config = if paths.config.present() { Config::load(&paths.config)? } else { Config::default() };
    // `[attention]` not written by the change (a switch, a list): left unwritten, the older keys still seeding it.
    if !kept.attention.is_empty() && config.attention.is_some() {
        let items: Vec<(String, Option<toml_edit::Item>)> = kept
            .attention
            .iter()
            .map(|(key, value)| (key.clone(), value.as_ref().and_then(|v| v.to_string().parse::<toml_edit::Value>().ok()).map(toml_edit::Item::Value)))
            .collect();
        crate::config::set_table_ranked(&paths.config, "attention", &items, &[], &attention::rank)?;
    }
    for (name, lists) in &kept.groups {
        for list in List::ALL {
            SenderList::remove_category(&list.path(&config), name)?;
        }
        for list in lists {
            SenderList::add_category(&list.path(&config), name)?;
        }
    }
    for (id, value) in &kept.phones {
        crate::calls_line::put_raw(&paths.config, id, value.as_deref())?;
    }
    for (key, on) in &kept.switches {
        write_switch(&paths.config, key, *on)?;
    }
    if !kept.talks.is_empty() || !kept.apps.is_empty() {
        let path = paths.apps.as_ref().ok_or("a phone's own choices")?;
        crate::filelock::with_lock(path, || {
            let mut choices = Choices::read(path)?;
            for (key, talk) in &kept.talks {
                match talk {
                    Some(talk) => choices.conversation.insert(key.clone(), talk.clone()),
                    None => choices.conversation.remove(key),
                };
            }
            for (package, app) in &kept.apps {
                match app {
                    Some(app) => choices.app.insert(package.clone(), app.clone()),
                    None => choices.app.remove(package),
                };
            }
            choices.save(path)
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------- words

/// "a, b and c".
fn listed(tr: &Translator, items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {} {last}", rest.join(", "), tr.text("word-and", None)),
    }
}

fn say(tr: &Translator, id: &str, pairs: &[(&str, String)]) -> String {
    let mut args = crate::i18n::args();
    for (key, value) in pairs {
        args.set(key.to_string(), value.clone());
    }
    tr.text(id, Some(&args))
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Who, inside a sentence: "your contacts", "everyone else".
pub fn whom_words(tr: &Translator, whom: &Whom) -> String {
    match whom {
        Whom::Anyone => tr.text("except-whom-anyone", None),
        Whom::Circle { person } => tr.text(&format!("except-whom-{}", person.id()), None),
        Whom::Group { name } => say(tr, "except-whom-group", &[("group", name.clone())]),
        Whom::Person { id, name } => if name.trim().is_empty() { id.trim_start_matches("tel:").trim_start_matches("contact:").to_string() } else { name.clone() },
    }
}

/// A channel's level in a word for your own times: mail's ◑ is shown
/// without a word, a message's in the shade; a call held goes to voicemail.
pub fn level_words(tr: &Translator, level: Level, channel: Option<Channel>) -> String {
    let id = match (level, channel) {
        (Level::Quiet, Some(Channel::Messages)) => "except-level-quiet-messages".to_string(),
        (Level::Later, Some(Channel::Calls)) => "except-level-later-calls".to_string(),
        (Level::Later, Some(Channel::Messages)) => "except-level-later-messages".to_string(),
        (Level::Never, Some(Channel::Mail)) => "except-level-never-mail".to_string(),
        // Always through's own: at once even under do-not-disturb, not a second "at once".
        (Level::Through, _) => "except-level-through".to_string(),
        (level, _) => format!("except-level-{}", level.id()),
    };
    tr.text(&id, None)
}

/// Your own times in words: the times grouped by level, the most common last
/// as "otherwise", then the two layers, time for you and do-not-disturb,
/// where they say something else: "at once during work, admin and leisure;
/// shown without a word during meals; later otherwise; held during time for
/// you and do-not-disturb".
pub fn cells_words(tr: &Translator, cells: &[Level; 9], channel: Option<Channel>) -> String {
    let mut parts: Vec<(Level, Vec<Column>)> = Vec::new();
    for (i, column) in Column::TIMES.iter().enumerate() {
        match parts.iter_mut().find(|(l, _)| *l == cells[i]) {
            Some((_, columns)) => columns.push(*column),
            None => parts.push((cells[i], vec![*column])),
        }
    }
    // The level of most times said last, as "otherwise".
    let most = parts.iter().enumerate().max_by_key(|(i, (_, c))| (c.len(), usize::MAX - i)).map(|(i, _)| i).unwrap_or(0);
    let otherwise = parts.remove(most);
    let mut said: Vec<String> = parts
        .iter()
        .map(|(level, columns)| {
            let times = listed(tr, &columns.iter().map(|c| tr.text(&format!("attention-during-{}", c.id()), None)).collect::<Vec<_>>());
            say(tr, "except-cells-during", &[("level", level_words(tr, *level, channel)), ("times", times)])
        })
        .collect();
    said.push(if said.is_empty() { level_words(tr, otherwise.0, channel) } else { say(tr, "except-cells-otherwise", &[("level", level_words(tr, otherwise.0, channel))]) });
    // The layers, each said where it differs from what the times mostly say.
    let mut layers: Vec<(Level, Vec<&str>)> = Vec::new();
    for (level, layer) in [(cells[7], "slot"), (cells[8], "dnd")] {
        // Through do-not-disturb, said under it: at once.
        let level = if layer == "dnd" && level == Level::Through { Level::Now } else { level };
        if level == otherwise.0 {
            continue;
        }
        match layers.iter_mut().find(|(l, _)| *l == level) {
            Some((_, names)) => names.push(layer),
            None => layers.push((level, vec![layer])),
        }
    }
    for (level, names) in layers {
        let times = listed(tr, &names.iter().map(|n| tr.text(&format!("except-layer-{n}"), None)).collect::<Vec<_>>());
        said.push(say(tr, "except-cells-during", &[("level", level_words(tr, level, channel)), ("times", times)]));
    }
    said.join(&tr.text("except-cells-separator", None))
}

/// When, in words, for what it is said of.
pub fn when_words(tr: &Translator, when: &When, what: &What) -> String {
    let channel = match what {
        What::Channel { channel } => Some(*channel),
        What::App { .. } | What::Conversation { .. } => Some(Channel::Messages),
        _ => None,
    };
    match when {
        When::Pattern { pattern } => {
            let notice = match what {
                What::Own { notice: Notice::NewMail | Notice::Newsletters } => Some(what),
                _ => None,
            };
            match (pattern, channel, notice) {
                (Pattern::Never, _, Some(What::Own { notice: Notice::NewMail })) => tr.text("except-when-never-new-mail", None),
                (Pattern::Always, _, Some(What::Own { notice: Notice::Newsletters })) => tr.text("except-when-told-newsletters", None),
                (Pattern::Always, Some(Channel::Mail), _) => tr.text("except-when-always-mail", None),
                (Pattern::Never, Some(Channel::Mail), _) => tr.text("except-when-never-mail", None),
                (Pattern::Never, Some(Channel::Calls), _) => tr.text("except-when-never-calls", None),
                (Pattern::Never, Some(Channel::Messages), _) => tr.text("except-when-never-messages", None),
                (pattern, _, _) => tr.text(&format!("except-when-{}", pattern.id()), None),
            }
        }
        When::Own { cells } => cells_words(tr, cells, channel),
        When::Like { list } => tr.text(&format!("except-like-{}", list.id()), None),
        When::Calls { profile } => tr.text(&format!("except-calls-{profile}"), None),
        When::Is { app, area } => {
            let is = tr.text(&format!("except-is-{}", app.id()), None);
            match Area::parse(area) {
                Some(area) => {
                    let areas: Vec<String> = [(area.work, "work"), (area.admin, "admin"), (area.leisure, "leisure")].iter().filter(|(on, _)| *on).map(|(_, id)| tr.text(&format!("attention-during-{id}"), None)).collect();
                    say(tr, "except-is-area", &[("is", is), ("area", listed(tr, &areas))])
                }
                None => is,
            }
        }
    }
}

/// What and from whom, as a sentence begins: "Mail from your contacts",
/// "People of the group Family", "Signal ▸ Sam", "Calls, on the work phone".
pub fn subject_words(tr: &Translator, e: &Exception) -> String {
    let whom = whom_words(tr, &e.whom);
    let what = match (&e.what, &e.whom) {
        (What::Channel { channel: Channel::Messages }, Whom::Circle { person: Person::Groups }) => tr.text("except-messages-groups", None),
        (What::Anything, Whom::Group { name }) => say(tr, "except-group", &[("group", name.clone())]),
        (What::Anything, Whom::Person { .. }) => say(tr, "except-talk-from", &[("whom", whom)]),
        // A circle whose calls are the call screening's (everyone else): its mail and messages alone.
        (What::Anything, Whom::Circle { person }) if !channels_of(*person).contains(&Channel::Calls) => say(tr, "except-mail-messages-from", &[("whom", whom)]),
        (What::Anything, _) => say(tr, "except-anything-from", &[("whom", whom)]),
        (What::Channel { channel }, Whom::Anyone) => tr.text(&format!("except-{}", channel.id()), None),
        (What::Channel { channel }, _) => say(tr, &format!("except-{}-from", channel.id()), &[("whom", whom)]),
        (What::App { package, name }, _) => if name.trim().is_empty() { package.clone() } else { name.clone() },
        (What::Conversation { title, app, .. }, _) => match (title.trim().is_empty(), app.trim().is_empty()) {
            (false, false) => say(tr, "except-conversation", &[("app", app.clone()), ("title", title.clone())]),
            (false, true) => title.clone(),
            (true, false) => say(tr, "except-conversation-of", &[("app", app.clone())]),
            (true, true) => tr.text("except-conversation-unknown", None),
        },
        (What::Own { notice: Notice::Kind(kind) }, _) => attention::row_label(tr, Row::Own(*kind)),
        (What::Own { notice }, _) => tr.text(&format!("except-own-{}", notice.id()), None),
    };
    match &e.place {
        Place::Everywhere => what,
        Place::Device { id, name } => say(tr, "except-on", &[("what", what), ("device", if name.trim().is_empty() { id.chars().take(8).collect() } else { name.clone() })]),
    }
}

/// An exception in one sentence: "Mail from your contacts: when you are awake."
pub fn sentence(tr: &Translator, e: &Exception) -> String {
    capital(&say(tr, "except-line", &[("what", subject_words(tr, e)), ("when", when_words(tr, &e.when, &e.what))]))
}

/// The channels on which your people go as this exception says: it writes
/// Safe's mail or messages, and your people's row of that channel is not
/// written, so it follows Safe's as it stands (`Attention::follows`).
pub fn followed_by_people(e: &Exception, attention: &Attention) -> Vec<Channel> {
    let rows = match &e.store {
        Some(Store::Rows { rows }) => rows.clone(),
        _ => match store_of(e) {
            Ok(Store::Rows { rows }) => rows,
            _ => return Vec::new(),
        },
    };
    [Channel::Mail, Channel::Messages].into_iter().filter(|c| rows.contains(&Row::People(*c, Person::Safe).id()) && attention.follows(Row::People(*c, Person::People))).collect()
}

/// `sentence`, saying your people where they go as it says (`followed_by_people`):
/// "Mail from your safe people and your people: in your work hours."; for
/// Safe's mail, calls and messages, a second sentence names what of your
/// people's follows.
pub fn sentence_for(tr: &Translator, e: &Exception, people: &[Channel]) -> String {
    if people.is_empty() {
        return sentence(tr, e);
    }
    match &e.what {
        What::Channel { channel } if people.contains(channel) => {
            let what = say(tr, &format!("except-{}-from", channel.id()), &[("whom", tr.text("except-whom-safe-people", None))]);
            capital(&say(tr, "except-line", &[("what", what), ("when", when_words(tr, &e.when, &e.what))]))
        }
        _ => {
            let channels: Vec<String> = people.iter().map(|c| tr.text(&format!("except-their-{}", c.id()), None)).collect();
            let mut args = tr.counted(people.len());
            args.set("channels", listed(tr, &channels));
            format!("{} {}", sentence(tr, e), tr.text("except-people-follow", Some(&args)))
        }
    }
}

/// The usual rule for one circle: its patterns per channel, and its sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleLine {
    pub circle: Person,
    /// Each channel's reading, mail, calls, messages (a circle's own channels).
    pub channels: Vec<(Channel, When)>,
    /// "Your contacts: when you are awake; their mail in your work and admin hours."
    pub text: String,
}

/// The usual rule, a sentence per circle (Always through, your people while
/// the circle holds someone, Safe, your contacts, Restricted, everyone
/// else, the blocked), read from the start's table: the pattern of most of
/// a circle's channels first, each channel that differs after it.
pub fn rule(tr: &Translator, start: Preset, people: bool) -> Vec<RuleLine> {
    let matrix = start.matrix();
    let mut out = Vec::new();
    for person in [Person::Always, Person::People, Person::Safe, Person::Neutral, Person::Restricted, Person::Stranger, Person::Blocked] {
        if person == Person::People && !people {
            continue;
        }
        let name = tr.text(&format!("except-circle-{}", person.id()), None);
        if person == Person::Blocked {
            out.push(RuleLine { circle: person, channels: Vec::new(), text: say(tr, "except-rule-line", &[("circle", name), ("when", tr.text("except-when-blocked", None))]) });
            continue;
        }
        // Messages first, then mail, then calls: the order a tie is said in.
        let order = [Channel::Messages, Channel::Mail, Channel::Calls];
        let channels: Vec<(Channel, When)> = order.iter().filter(|c| channels_of(person).contains(*c)).map(|c| (*c, reading(Row::People(*c, person), &row_cells(&matrix, Row::People(*c, person))))).collect();
        let count = |when: &When| channels.iter().filter(|(_, w)| w == when).count();
        let main = channels.iter().max_by_key(|(c, w)| (count(w), usize::MAX - order.iter().position(|o| o == c).unwrap_or(0))).map(|(_, w)| w.clone());
        let Some(main) = main else { continue };
        let main_channel = channels.iter().find(|(_, w)| *w == main).map(|(c, _)| *c);
        let main_what = match (&main, main_channel) {
            // One pattern for every channel: said in its common words; your own times in a channel's.
            (When::Pattern { .. }, _) if count(&main) > 1 => What::Anything,
            (_, Some(channel)) => What::Channel { channel },
            _ => What::Anything,
        };
        let mut when = if person == Person::Always && main == When::pattern(Pattern::Always) { tr.text("except-when-always-through", None) } else { when_words(tr, &main, &main_what) };
        for (channel, other) in channels.iter().filter(|(_, w)| *w != main) {
            when = say(tr, "except-rule-their", &[("main", when), ("channel", tr.text(&format!("except-their-{}", channel.id()), None)), ("when", when_words(tr, other, &What::Channel { channel: *channel }))]);
        }
        // Mail at every moment is shown without a word at night and in a pause: said, once.
        if channels.iter().any(|(c, w)| *c == Channel::Mail && *w == When::pattern(Pattern::Always)) && count(&When::pattern(Pattern::Always)) > 1 {
            when = say(tr, "except-rule-mail-night", &[("main", when)]);
        }
        out.push(RuleLine { circle: person, channels, text: capital(&say(tr, "except-rule-line", &[("circle", name), ("when", when)])) });
    }
    out
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use Level::{Later, Never, Now, Quiet};

    fn no_name(_: &str) -> String {
        String::new()
    }

    /// A configuration in a folder of its own, its lists beside it.
    struct Here {
        dir: PathBuf,
        name: String,
    }

    impl Here {
        fn new(name: &str, text: &str) -> Here {
            let dir = std::env::temp_dir().join(format!("sioul-exceptions-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            // The lists sit beside the configuration (`config_dir` of the file's folder).
            let text = text.replace("{dir}", &dir.display().to_string());
            std::fs::write(dir.join("config.toml"), text).unwrap();
            Here { dir, name: name.to_string() }
        }

        fn path(&self) -> PathBuf {
            self.dir.join("config.toml")
        }

        /// This test's own group of contacts on the shared stand-in lists.
        fn group(&self) -> String {
            format!("{GROUP}-{}", self.name)
        }

        fn config(&self) -> Config {
            Config::load(&self.path()).unwrap()
        }

        fn paths(&self) -> Paths {
            Paths { config: self.path(), apps: Some(self.dir.join("app-notes.toml")) }
        }

        fn read(&self) -> Vec<Exception> {
            let config = self.config();
            let attention = Attention::of(&config);
            let apps = Choices::load(&self.dir.join("app-notes.toml"));
            let devices = [Device { id: "phone-1".into(), name: "The work phone".into(), phone: true, here: true }];
            let mut read = list(&Stores { attention: &attention, config: &config, devices: &devices, apps: Some(&apps), name: &no_name });
            read.retain(|e| !matches!(&e.whom, Whom::Group { name } if *name != self.group()));
            read
        }

        /// What the stores hold: the matrix, the phones' lines and the
        /// switches of the configuration, this phone's choices, the lists' lines of this test's group.
        fn stores(&self) -> (Attention, String, Choices, Vec<String>) {
            let config = self.config();
            let devices = format!("{:?}{:?}{:?}{:?}", config.devices, config.reminders.mail, config.reminders.mail_newsletters, config.reminders.gather);
            let lists: Vec<String> = List::ALL.iter().map(|l| std::fs::read_to_string(l.path(&config)).unwrap_or_default().lines().filter(|line| line.trim_start_matches("category:") == self.group()).collect::<Vec<_>>().join(",")).collect();
            (Attention::of(&config), devices, Choices::load(&self.dir.join("app-notes.toml")), lists)
        }

        /// The stores byte for byte: the configuration, this phone's choices,
        /// and on each list the lines of this test's group (the lists are
        /// shared with the other tests running beside it).
        fn bytes(&self) -> (String, String, Vec<String>) {
            let config = self.config();
            let lists: Vec<String> = List::ALL.iter().map(|l| std::fs::read_to_string(l.path(&config)).unwrap_or_default().lines().filter(|line| line.trim_start_matches("category:") == self.group()).collect::<Vec<_>>().join(",")).collect();
            (std::fs::read_to_string(self.path()).unwrap_or_default(), std::fs::read_to_string(self.dir.join("app-notes.toml")).unwrap_or_default(), lists)
        }

        fn devices(&self) -> Vec<Device> {
            vec![Device { id: "phone-1".into(), name: "The work phone".into(), phone: true, here: true }, Device { id: "laptop".into(), name: "The laptop".into(), phone: false, here: false }]
        }
    }

    impl Drop for Here {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// The senders' lists are the stand-in configuration folder's: a group
    /// a test names is its own, and another test's are left out of what it reads.
    fn lists_here() -> &'static str {
        ""
    }

    /// A group this file's tests own on the shared stand-in lists.
    const GROUP: &str = "Exceptions-test-family";

    /// Four rows a person keeps, written by a Sioul until 0.1.0 (no start).
    const FOUR_ROWS: &str = "[attention]\n\"mail.neutral\" = [\"work\", \"admin\", \"leisure\", \"meals\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot:quiet\", \"dnd:quiet\"]\n\"mail.stranger\" = [\"work\", \"admin\", \"leisure:quiet\", \"meals:later\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot:quiet\", \"dnd:quiet\"]\n\"mail.people\" = [\"work\", \"admin\", \"leisure\", \"meals\", \"sleep:quiet\", \"pause:quiet\", \"free\", \"slot\", \"dnd\"]\n\"messages.people\" = [\"work\", \"admin\", \"leisure\", \"meals\", \"sleep\", \"pause\", \"free\", \"slot\", \"dnd\"]\n";

    /// As Sioul ships it, no exception at all, and the start As Sioul does;
    /// each preset reads as its own start, with no exception either.
    #[test]
    fn as_sioul_ships_it_nothing_differs() {
        let p = Here::new("usual", &format!("language = \"en\"\n{}", lists_here()));
        assert!(p.read().is_empty(), "{:?}", p.read());
        assert_eq!(start_of(&p.config(), &Attention::usual()), Preset::Usual);
    }

    /// Settings as a person keeps them, written before the start was kept:
    /// four rows, read over As Sioul does as a Sioul until 0.1.0 wrote them
    /// (two of them are More reachable's own: no matter), four lines.
    #[test]
    fn four_rows_read_as_four_lines() {
        let p = Here::new("four", &format!("language = \"en\"\n{}{FOUR_ROWS}", lists_here()));
        let config = p.config();
        assert_eq!(stored_start(&config), None);
        assert_eq!(start_of(&config, &Attention::of(&config)), Preset::Usual);
        let read = p.read();
        let tr = Translator::new("en");
        let said: Vec<String> = read.iter().map(|e| sentence(&tr, e)).collect();
        assert_eq!(said.len(), 4, "{said:#?}");
        assert_eq!(read[0].whom, Whom::Circle { person: Person::People });
        assert_eq!(read[0].what, What::Channel { channel: Channel::Mail });
        assert_eq!(read[0].when, When::pattern(Pattern::Always));
        assert_eq!(read[1].what, What::Channel { channel: Channel::Messages });
        assert_eq!(read[1].when, When::pattern(Pattern::Always));
        assert_eq!(read[2].when, When::pattern(Pattern::Awake), "your contacts' mail: when you are awake");
        assert!(matches!(read[3].when, When::Own { .. }), "everyone else's mail: your own times");
        assert_eq!(said[2], "Mail from your contacts: when you are awake.");
        assert_eq!(said[3], "Mail from everyone else: at once during work and admin; shown without a word during leisure; later otherwise; shown without a word during time for you and do-not-disturb.");
        for line in &said {
            assert!(!line.contains("except-") && !line.contains('{'), "{line}");
        }
    }

    /// A row of every store read as an exception, taken out and put back:
    /// the files as they were, byte for byte.
    #[test]
    fn every_store_round_trips() {
        let text = format!(
            "language = \"en\"\n{}[reminders]\nmail = false\nmail_newsletters = true\ngather = false\n[attention]\n\"mail.safe\" = [\"work\", \"admin:later\", \"leisure:later\", \"meals:later\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot:quiet\", \"dnd:quiet\"]\n\"messages.neutral\" = [\"work\", \"admin\", \"leisure:gathered\", \"meals:gathered\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot\", \"dnd:later\"]\n\"messages.groups\" = [\"work\", \"admin\", \"leisure:gathered\", \"meals:gathered\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot\", \"dnd:later\"]\nbefore = [\"work\", \"admin\", \"leisure:later\", \"meals:later\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot\", \"dnd\"]\n\"app.org.example.chat\" = [\"work\", \"leisure:later\", \"name=Chat\"]\n\"conversation.8c1f0e5a2b7d4c69\" = [\"sleep\"]\n\"person.contact:card-1\" = [\"sleep\", \"dnd:quiet\"]\n",
            lists_here()
        );
        crate::config::stand_ins_only();
        let p = Here::new("round", &text);
        // The phone's table as Sioul makes it (a table taken out and made again is written the same way).
        crate::calls_line::set_profile(&p.path(), "phone-1", Profile::Professional, &Profile::Professional.grid()).unwrap();
        crate::porch::set_category(&p.config(), &p.group(), Some(Standing::Safe)).unwrap();
        std::fs::write(p.dir.join("app-notes.toml"), "[conversation.8c1f0e5a2b7d4c69]\nthrough = \"always\"\ntitle = \"Sam\"\napp = \"Signal\"\n\n[app.\"org.example.news\"]\nkind = \"automaton\"\nlabel = \"News\"\n").unwrap();
        let read = p.read();
        let kinds: Vec<&str> = read
            .iter()
            .map(|e| match e.store.as_ref().unwrap() {
                Store::Rows { .. } => "rows",
                Store::Source { .. } => "source",
                Store::Group { .. } => "group",
                Store::Phone { .. } => "phone",
                Store::Switch { .. } => "switch",
                Store::Talk { .. } => "talk",
                Store::App { .. } => "app",
            })
            .collect();
        for kind in ["rows", "source", "group", "phone", "switch", "talk", "app"] {
            assert!(kinds.contains(&kind), "{kind} read: {read:#?}");
        }
        // Your contacts' messages carry the group conversations' row: one line.
        let contacts = read.iter().find(|e| e.whom == Whom::Circle { person: Person::Neutral }).unwrap();
        assert_eq!(contacts.store, Some(Store::Rows { rows: vec!["messages.neutral".into(), "messages.groups".into()] }));
        assert_eq!(contacts.when, When::pattern(Pattern::WorkAdminGathered));
        assert!(!read.iter().any(|e| e.whom == Whom::Circle { person: Person::Groups }));
        // Each store written once as Sioul writes it (the words of a row, a phone's line, this phone's file).
        for e in &read {
            apply(&p.paths(), &adding(e, Preset::Usual).unwrap(), Preset::Usual).unwrap();
        }
        assert_eq!(p.read(), read);
        let before = p.stores();
        let bytes = p.bytes();
        for e in &read {
            apply(&p.paths(), &removing(e).unwrap(), Preset::Usual).unwrap();
            assert!(!p.read().contains(e), "taken out: {e:?}");
            apply(&p.paths(), &adding(e, Preset::Usual).unwrap(), Preset::Usual).unwrap();
            let again = p.read();
            assert!(again.contains(e), "put back: {e:?} in {again:#?}");
            assert_eq!(p.bytes(), bytes, "taken out and put back: {e:?}");
        }
        // Every exception put back: the stores read the same, byte for byte.
        assert_eq!(p.read(), read);
        assert_eq!(p.stores(), before);
        assert_eq!(p.bytes(), bytes);
        // Everything taken out: nothing differs, and nothing of it is left in the files.
        for e in &read {
            apply(&p.paths(), &removing(e).unwrap(), Preset::Usual).unwrap();
        }
        assert!(p.read().is_empty(), "{:#?}", p.read());
        let config = std::fs::read_to_string(p.path()).unwrap();
        assert!(!config.contains("person.") && !config.contains("mail.safe") && !config.contains("calls ="), "{config}");
        assert!(p.stores().3.iter().all(String::is_empty), "the group off every list");
    }

    /// A row as the start has it is no difference and is not stored; your
    /// people's rows that follow Safe's are none either.
    #[test]
    fn a_row_back_to_the_start_is_not_stored() {
        let p = Here::new("back", &format!("language = \"en\"\n{}", lists_here()));
        let e = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Safe }, place: Place::Everywhere, when: When::pattern(Pattern::Work), store: None };
        apply(&p.paths(), &adding(&e, Preset::Usual).unwrap(), Preset::Usual).unwrap();
        assert!(std::fs::read_to_string(p.path()).unwrap().contains("mail.safe"));
        let read = p.read();
        assert_eq!(read.len(), 1, "Safe's mail, and your people's following it is no line: {read:?}");
        apply(&p.paths(), &removing(&read[0]).unwrap(), Preset::Usual).unwrap();
        assert!(!std::fs::read_to_string(p.path()).unwrap().contains("mail.safe"));
        // The same pattern as the start's: written as the start has it, so not stored.
        let same = Exception { when: When::pattern(Pattern::Awake), ..e };
        apply(&p.paths(), &adding(&same, Preset::Usual).unwrap(), Preset::Usual).unwrap();
        assert!(p.read().is_empty());
        assert!(!std::fs::read_to_string(p.path()).unwrap().contains("mail.safe"));
    }

    /// Your people's row made theirs, then taken out: it follows Safe's again.
    #[test]
    fn your_people_s_row_follows_safe_again_once_taken_out() {
        let p = Here::new("people", &format!("language = \"en\"\n{}", lists_here()));
        let e = Exception { what: What::Channel { channel: Channel::Messages }, whom: Whom::Circle { person: Person::People }, place: Place::Everywhere, when: When::pattern(Pattern::Always), store: None };
        apply(&p.paths(), &adding(&e, Preset::Usual).unwrap(), Preset::Usual).unwrap();
        assert_eq!(p.read().len(), 1);
        apply(&p.paths(), &removing(&p.read()[0]).unwrap(), Preset::Usual).unwrap();
        let attention = Attention::of(&p.config());
        assert!(attention.follows(Row::People(Channel::Messages, Person::People)));
    }

    /// The patterns written out on rows of each kind, W A L M S P F Y D.
    #[test]
    fn the_patterns_on_each_kind_of_row() {
        let calls = |p: Person| Row::People(Channel::Calls, p);
        assert_eq!(cells(calls(Person::Safe), Pattern::Awake), Some([Now, Now, Now, Now, Later, Later, Now, Now, Later]));
        assert_eq!(cells(calls(Person::Neutral), Pattern::Awake), Some([Now, Now, Now, Now, Later, Later, Later, Now, Later]));
        assert_eq!(cells(calls(Person::Stranger), Pattern::Awake), None, "the call screening's");
        assert_eq!(cells(Row::People(Channel::Mail, Person::Neutral), Pattern::Never), Some([Never; 9]));
        assert_eq!(cells(Row::People(Channel::Mail, Person::Neutral), Pattern::Shown), Some([Quiet; 9]));
        assert_eq!(cells(Row::People(Channel::Messages, Person::Neutral), Pattern::Shown), None, "a message cannot be shown");
        assert_eq!(cells(Row::People(Channel::Mail, Person::Always), Pattern::Never), None, "Always through's mail is never held");
        assert_eq!(cells(Row::Own(Kind::Before), Pattern::Never), Some([Later; 9]));
        assert_eq!(cells(Row::Own(Kind::Needs), Pattern::Never), Some([Never; 9]));
        assert_eq!(cells(Row::Own(Kind::AppAutomatons), Pattern::Gathered), Some([Level::Gathered, Level::Gathered, Level::Gathered, Level::Gathered, Later, Later, Later, Later, Later]));
        assert_eq!(cells(Row::Own(Kind::Codes), Pattern::Never), None, "a code's fixed cells");
        assert_eq!(source_cells("person.tel:+33199001234", Pattern::Gathered), None);
        assert_eq!(source_cells("app.org.example", Pattern::Gathered).map(|c| c[0]), Some(Level::Gathered));
        // Every row of As Sioul does reads as the pattern its circle's line says.
        let usual = Attention::usual();
        let read = |c: Channel, p: Person| reading(Row::People(c, p), &row_cells(&usual, Row::People(c, p)));
        assert_eq!(read(Channel::Mail, Person::Safe), When::pattern(Pattern::Awake));
        assert_eq!(read(Channel::Calls, Person::Safe), When::pattern(Pattern::Awake));
        assert_eq!(read(Channel::Calls, Person::People), When::pattern(Pattern::Always));
        assert_eq!(read(Channel::Calls, Person::Restricted), When::pattern(Pattern::Work));
        assert_eq!(read(Channel::Messages, Person::Stranger), When::pattern(Pattern::WorkAdminGathered));
        assert_eq!(read(Channel::Mail, Person::Neutral), When::pattern(Pattern::WorkAdmin));
    }

    /// The usual rule in seven sentences, in both languages, each naming its
    /// pattern; where the kinds differ in the table itself, said shortly.
    #[test]
    fn the_usual_rule_in_sentences() {
        let tr = Translator::new("en");
        let lines: Vec<String> = rule(&tr, Preset::Usual, true).into_iter().map(|l| l.text).collect();
        assert_eq!(
            lines,
            [
                "Always through: always, even through do-not-disturb; their mail shown without a word at night and in a pause.",
                "Your people: when you are awake; their calls always.",
                "Safe: when you are awake.",
                "Your contacts: when you are awake; their mail in your work and admin hours.",
                "Restricted: in your work hours.",
                "Everyone else: in your work and admin hours, gathered outside them; their mail in your work and admin hours.",
                "Blocked: never.",
            ]
        );
        assert_eq!(rule(&tr, Preset::Usual, false).len(), 6, "no line for your people while the circle holds nobody");
        for preset in Preset::ALL {
            for language in ["en", "fr"] {
                let tr = Translator::new(language);
                for line in rule(&tr, preset, true) {
                    said_well(&line.text, language);
                }
            }
        }
    }

    /// A sentence said whole: no key, no placeable, a capital, a full stop; French typography, « type ».
    fn said_well(line: &str, language: &str) {
        assert!(!line.contains("except-") && !line.contains("attention-") && !line.contains('{'), "{language}: {line}");
        assert!(line.chars().next().is_some_and(|c| !c.is_lowercase()), "{language}: no capital: {line}");
        assert!(line.ends_with('.'), "{language}: no full stop: {line}");
        assert!(!line.contains("genre") && !line.contains("sorte"), "{language}: {line}");
        if language == "fr" {
            for mark in [" :", " ;", "« ", " »", "'"] {
                assert!(!line.contains(mark), "fr: typography ({mark:?}) in {line}");
            }
        }
    }

    /// Every exception the card can make, said in both languages.
    #[test]
    fn every_exception_said_whole_in_both_languages() {
        let mut all: Vec<Exception> = Vec::new();
        let mut whats = vec![What::Anything, What::App { package: "org.example".into(), name: "Chat".into() }, What::Conversation { key: "abc".into(), title: "Léa".into(), app: "Signal".into() }, What::Conversation { key: "abc".into(), title: String::new(), app: String::new() }];
        whats.extend(Channel::ALL.iter().map(|c| What::Channel { channel: *c }));
        whats.extend(Kind::ALL.iter().map(|k| What::Own { notice: Notice::Kind(*k) }));
        whats.extend([What::Own { notice: Notice::NewMail }, What::Own { notice: Notice::Newsletters }]);
        let mut whoms = vec![Whom::Anyone, Whom::Group { name: "Family".into() }, Whom::Person { id: "contact:x".into(), name: "Léa".into() }];
        whoms.extend(CIRCLES.iter().map(|p| Whom::Circle { person: *p }));
        let places = [Place::Everywhere, Place::Device { id: "p".into(), name: "The work phone".into() }];
        for what in &whats {
            for whom in &whoms {
                for place in &places {
                    for when in offered(what, whom, place) {
                        all.push(Exception { what: what.clone(), whom: whom.clone(), place: place.clone(), when, store: None });
                    }
                    if own_times(what, whom, place) {
                        let probe = Exception { what: what.clone(), whom: whom.clone(), place: place.clone(), when: When::Own { cells: [Now; 9] }, store: None };
                        let choices = own_choices(&probe);
                        // The last level each moment offers, so that most say something other than at once.
                        if choices.iter().all(|c| !c.is_empty()) {
                            let cells: [Level; 9] = std::array::from_fn(|i| *choices[i].last().unwrap());
                            all.push(Exception { when: When::Own { cells }, ..probe });
                        }
                    }
                }
            }
        }
        assert!(all.len() > 60, "{}", all.len());
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for e in &all {
                said_well(&sentence(&tr, e), language);
                assert!(store_of(e).is_ok(), "{e:?}");
                assert!(adding(e, Preset::Usual).is_ok(), "{e:?}");
            }
        }
        // Each of the offered patterns writes what it says on every row it names.
        for e in &all {
            if let (Ok(Store::Rows { rows }), When::Pattern { pattern }) = (store_of(e), &e.when) {
                for row in rows.iter().filter_map(|r| Row::read(r)) {
                    assert!(cells(row, *pattern).is_some(), "{e:?}");
                }
            }
        }
    }

    /// Which line decides, with several that overlap: a person before a
    /// group before a circle; a conversation before an app before a kind;
    /// one device before every device; the usual rule when none speaks.
    #[test]
    fn the_most_precise_line_decides() {
        let e = |what: What, whom: Whom, place: Place, when: When| Exception { what, whom, place, when, store: None };
        let messages = What::Channel { channel: Channel::Messages };
        let list = vec![
            e(messages.clone(), Whom::Circle { person: Person::Safe }, Place::Everywhere, When::pattern(Pattern::Work)),
            e(What::Anything, Whom::Group { name: "Family".into() }, Place::Everywhere, When::Like { list: List::Restricted }),
            e(What::Anything, Whom::Person { id: "contact:lea".into(), name: "Léa".into() }, Place::Everywhere, When::Own { cells: [Level::As, Level::As, Level::As, Level::As, Now, Level::As, Level::As, Level::As, Level::As] }),
            e(What::App { package: "org.signal".into(), name: "Signal".into() }, Whom::Anyone, Place::Everywhere, When::pattern(Pattern::Never)),
            e(What::Conversation { key: "k1".into(), title: "Léa".into(), app: "Signal".into() }, Whom::Anyone, Place::Everywhere, When::pattern(Pattern::Always)),
            e(What::Channel { channel: Channel::Calls }, Whom::Anyone, Place::Device { id: "work".into(), name: "Work".into() }, When::Calls { profile: "professional".into() }),
            e(What::Channel { channel: Channel::Calls }, Whom::Circle { person: Person::Safe }, Place::Everywhere, When::pattern(Pattern::Always)),
        ];
        let ask = |what: Asked, person: Option<&str>, groups: &[&str], circle: Person, device: Option<&str>| Ask { what, person: person.map(str::to_string), groups: groups.iter().map(|g| g.to_string()).collect(), circle, device: device.map(str::to_string) };
        // A safe person's message: the circle's line.
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Messages), None, &[], Person::Safe, None), Column::Work), Some(0));
        // In a group: the group's line before the circle's.
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Messages), None, &["family"], Person::Safe, None), Column::Work), Some(1));
        // Léa, of that group: their own row where it speaks (sleep), the group's elsewhere.
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Messages), Some("contact:lea"), &["Family"], Person::Safe, None), Column::Sleep), Some(2));
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Messages), Some("contact:lea"), &["Family"], Person::Safe, None), Column::Work), Some(1));
        // Through Signal: the app before the group; its conversation with Léa before everything.
        assert_eq!(decide(&list, &ask(Asked::App("org.signal".into()), None, &["Family"], Person::Safe, None), Column::Work), Some(3));
        assert_eq!(decide(&list, &ask(Asked::Conversation("k1".into(), "org.signal".into()), Some("contact:lea"), &["Family"], Person::Safe, None), Column::Sleep), Some(4));
        // A person before an app.
        assert_eq!(decide(&list, &ask(Asked::App("org.signal".into()), Some("contact:lea"), &[], Person::Safe, None), Column::Sleep), Some(2));
        // A safe caller: everywhere the circle's line; on the work phone, its table first.
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Calls), None, &[], Person::Safe, None), Column::Leisure), Some(6));
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Calls), None, &[], Person::Safe, Some("work")), Column::Leisure), Some(5));
        // One of your people on the work phone: its table has no row of theirs: the usual rule.
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Calls), None, &[], Person::People, Some("work")), Column::Leisure), None);
        // Mail from a contact: no line speaks of it.
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Mail), None, &[], Person::Neutral, None), Column::Work), None);
        // Léa's mail: their own row holds no mail.
        assert_eq!(decide(&list, &ask(Asked::Channel(Channel::Mail), Some("contact:lea"), &[], Person::Safe, None), Column::Sleep), None);
    }

    /// A new start writes the rows it sets differently; the exceptions kept
    /// on other rows stay, those on its rows give way.
    #[test]
    fn a_start_keeps_the_exceptions_it_does_not_set() {
        let p = Here::new("start", &format!("language = \"en\"\n{}", lists_here()));
        let restricted = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Restricted }, place: Place::Everywhere, when: When::pattern(Pattern::Never), store: None };
        let safe = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Safe }, place: Place::Everywhere, when: When::pattern(Pattern::Work), store: None };
        for e in [&restricted, &safe] {
            apply(&p.paths(), &adding(e, Preset::Usual).unwrap(), Preset::Usual).unwrap();
        }
        let read = p.read();
        assert_eq!(read.len(), 2);
        // Quieter sets Safe's mail: that exception gives way.
        let gone = gives_way(&read, Preset::Usual, Preset::Quieter);
        assert_eq!(gone.iter().map(|i| read[*i].whom.clone()).collect::<Vec<_>>(), [Whom::Circle { person: Person::Safe }]);
        apply(&p.paths(), &starting(Preset::Usual, Preset::Quieter), Preset::Quieter).unwrap();
        let config = p.config();
        assert_eq!(stored_start(&config), Some(Preset::Quieter), "the start kept");
        assert_eq!(start_of(&config, &Attention::of(&config)), Preset::Quieter);
        let after = p.read();
        assert_eq!(after.len(), 1, "{after:#?}");
        assert_eq!(after[0].whom, Whom::Circle { person: Person::Restricted });
        // Back to As Sioul does: Quieter's rows taken out, the exception kept.
        apply(&p.paths(), &starting(Preset::Quieter, Preset::Usual), Preset::Usual).unwrap();
        let config = p.config();
        assert_eq!(start_of(&config, &Attention::of(&config)), Preset::Usual);
        assert_eq!(p.read().len(), 1);
    }

    /// An older Sioul reads what the exceptions write as it always did: the
    /// rows of `[attention]` its reader knows, a source's row aside, the
    /// lists' lines, a phone's line in the shape it reads.
    #[test]
    fn an_older_sioul_reads_what_is_written() {
        crate::config::stand_ins_only();
        let p = Here::new("older", &format!("language = \"en\"\n{}", lists_here()));
        let writes = [
            Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Neutral }, place: Place::Everywhere, when: When::pattern(Pattern::Awake), store: None },
            Exception { what: What::Anything, whom: Whom::Group { name: format!("{GROUP}-older") }, place: Place::Everywhere, when: When::Like { list: List::Safe }, store: None },
            Exception { what: What::Channel { channel: Channel::Calls }, whom: Whom::Anyone, place: Place::Device { id: "phone-1".into(), name: String::new() }, when: When::Calls { profile: "personal".into() }, store: None },
        ];
        for e in &writes {
            apply(&p.paths(), &adding(e, Preset::Usual).unwrap(), Preset::Usual).unwrap();
        }
        let config = p.config();
        // The row in the words every Sioul since 0.0.12 reads, whole.
        let words = config.attention.as_ref().unwrap().words("mail.neutral").unwrap();
        assert_eq!(words.len(), 9, "{words:?}");
        assert!(std::fs::read_to_string(config.safe_senders_path()).unwrap().lines().any(|l| l == format!("category:{GROUP}-older")));
        crate::porch::set_category(&config, &format!("{GROUP}-older"), None).unwrap();
        // The phone's line as calls_line reads it (format 1, written whole).
        let line = config.devices.get("phone-1").and_then(|d| d.calls.as_ref()).expect("a line");
        assert_eq!(line.grid.profile, Profile::Personal);
    }

    /// Undo: what a change touched put back as it was, store by store.
    #[test]
    fn a_change_is_undone_store_by_store() {
        crate::config::stand_ins_only();
        let text = format!("language = \"en\"\n[attention]\n\"mail.neutral\" = [\"work\", \"admin\", \"leisure\", \"meals\", \"sleep:later\", \"pause:later\", \"free:later\", \"slot:quiet\", \"dnd:quiet\"]\n");
        let p = Here::new("undo", &text);
        let changes = [
            Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Neutral }, place: Place::Everywhere, when: When::pattern(Pattern::Never), store: None },
            Exception { what: What::Anything, whom: Whom::Group { name: format!("{GROUP}-undo") }, place: Place::Everywhere, when: When::Like { list: List::Blocked }, store: None },
            Exception { what: What::Channel { channel: Channel::Calls }, whom: Whom::Anyone, place: Place::Device { id: "phone-1".into(), name: String::new() }, when: When::Calls { profile: "both".into() }, store: None },
            Exception { what: What::Own { notice: Notice::NewMail }, whom: Whom::Anyone, place: Place::Everywhere, when: When::pattern(Pattern::Never), store: None },
            Exception { what: What::Conversation { key: "k1".into(), title: "Léa".into(), app: "Signal".into() }, whom: Whom::Anyone, place: Place::Device { id: "phone-1".into(), name: String::new() }, when: When::pattern(Pattern::Gathered), store: None },
        ];
        let before = p.stores();
        let stores: Vec<Store> = changes.iter().map(|e| store_of(e).unwrap()).collect();
        let kept = keep(&p.paths(), &stores, &[]);
        for e in &changes {
            apply(&p.paths(), &adding(e, Preset::Usual).unwrap(), Preset::Usual).unwrap();
        }
        assert_ne!(p.stores(), before);
        put_back(&p.paths(), &kept).unwrap();
        assert_eq!(p.stores(), before);
        let config = p.config();
        assert!(!std::fs::read_to_string(config.blocked_senders_path()).unwrap_or_default().contains(&format!("{GROUP}-undo")));
    }

    /// What the tests below add under every start: a row a preset sets (Safe's
    /// mail), your contacts' messages with the group conversations', one of
    /// Sioul's own kinds, a person's own row, an app's row, a switch, this
    /// test's group on a list, a phone's table of calls, a conversation on
    /// this phone.
    fn under_test(p: &Here) -> Vec<Exception> {
        let e = |what: What, whom: Whom, place: Place, when: When| Exception { what, whom, place, when, store: None };
        let phone = || Place::Device { id: "phone-1".into(), name: "The work phone".into() };
        vec![
            e(What::Channel { channel: Channel::Mail }, Whom::Circle { person: Person::Safe }, Place::Everywhere, When::pattern(Pattern::Always)),
            e(What::Channel { channel: Channel::Messages }, Whom::Circle { person: Person::Neutral }, Place::Everywhere, When::pattern(Pattern::Never)),
            e(What::Own { notice: Notice::Kind(Kind::Dates) }, Whom::Anyone, Place::Everywhere, When::pattern(Pattern::Never)),
            e(What::Anything, Whom::Person { id: "contact:card-9".into(), name: String::new() }, Place::Everywhere, When::pattern(Pattern::Always)),
            e(What::App { package: "org.example.chat".into(), name: "Chat".into() }, Whom::Anyone, Place::Everywhere, When::pattern(Pattern::Never)),
            e(What::Own { notice: Notice::NewMail }, Whom::Anyone, Place::Everywhere, When::pattern(Pattern::Never)),
            e(What::Anything, Whom::Group { name: p.group() }, Place::Everywhere, When::Like { list: List::Restricted }),
            e(What::Channel { channel: Channel::Calls }, Whom::Anyone, phone(), When::Calls { profile: "professional".into() }),
            e(What::Conversation { key: "8c1f0e5a2b7d4c69".into(), title: "Sam".into(), app: "Signal".into() }, Whom::Anyone, phone(), When::pattern(Pattern::Gathered)),
        ]
    }

    /// The exception the page lists for one made (its store with it).
    fn as_listed(p: &Here, e: &Exception) -> Exception {
        p.read().into_iter().find(|r| r.what == e.what && r.whom == e.whom && r.place == e.place).unwrap_or_else(|| panic!("not listed: {e:?} in {:#?}", p.read()))
    }

    /// A profile of each start, kept: the start chosen written, this phone's
    /// conversation already seen (its title kept by the phone), as Sioul writes them.
    fn at_start(name: &str, to: Preset) -> Here {
        // Its reminders' section holds another key, as nearly every configuration's does.
        let p = Here::new(name, "language = \"en\"\n[reminders]\nbefore_event = 15\n");
        apply(&p.paths(), &starting(Preset::Usual, to), to).unwrap();
        let path = p.dir.join("app-notes.toml");
        std::fs::write(&path, "[conversation.8c1f0e5a2b7d4c69]\ntitle = \"Sam\"\napp = \"Signal\"\n").unwrap();
        Choices::read(&path).unwrap().save(&path).unwrap();
        p
    }

    /// Under each start, kept as `[attention] start`: every exception added
    /// then taken out, and each taken out then added again, leave the stores
    /// byte for byte as they were; the start stays where it was chosen, and
    /// the line taken out goes back to that start's row (Quieter's, not As
    /// Sioul does'), never to a start read anew from the rows.
    #[test]
    fn under_each_start_adding_and_taking_out_leave_the_bytes() {
        crate::config::stand_ins_only();
        for to in Preset::ALL {
            let p = at_start(&format!("bytes-{}", to.id()), to);
            assert_eq!(stored_start(&p.config()), Some(to));
            let devices = p.devices();
            for e in under_test(&p) {
                let before = p.bytes();
                let kept = save(&p.paths(), None, Some(&e), &devices).unwrap_or_else(|r| panic!("{}: {e:?}: {r:?}", to.id()));
                assert!(kept.is_some());
                let config = p.config();
                assert_eq!(start_of(&config, &Attention::of(&config)), to, "{}: the start stays under {e:?}", to.id());
                save(&p.paths(), Some(&as_listed(&p, &e)), None, &devices).unwrap();
                assert_eq!(p.bytes(), before, "{}: added then taken out: {e:?}", to.id());
            }
            for e in under_test(&p) {
                save(&p.paths(), None, Some(&e), &devices).unwrap();
            }
            let all = p.read();
            assert_eq!(all.len(), under_test(&p).len(), "{}: {all:#?}", to.id());
            let before = p.bytes();
            for e in &all {
                save(&p.paths(), Some(e), None, &devices).unwrap();
                save(&p.paths(), None, Some(&Exception { store: None, ..e.clone() }), &devices).unwrap();
                assert_eq!(p.bytes(), before, "{}: taken out then added: {e:?}", to.id());
            }
            // Taken out under Quieter, Safe's mail goes back to Quieter's row.
            let safe = Row::People(Channel::Mail, Person::Safe);
            save(&p.paths(), Some(&as_listed(&p, &under_test(&p)[0])), None, &devices).unwrap();
            let attention = Attention::of(&p.config());
            assert_eq!(row_cells(&attention, safe), row_cells(&to.matrix(), safe), "{}", to.id());
            for e in p.read() {
                save(&p.paths(), Some(&e), None, &devices).unwrap();
            }
        }
    }

    /// Settings from before the start was kept, Quieter's rows written by an
    /// older page: read over As Sioul does (Quieter's rows are lines), that
    /// start kept with the first change; Undo takes it out again.
    #[test]
    fn a_start_from_before_is_read_then_kept() {
        crate::config::stand_ins_only();
        let p = Here::new("before-start", "language = \"en\"\n");
        apply(&p.paths(), &[Write::Back(start_rows(Preset::Usual, Preset::Quieter))], Preset::Quieter).unwrap();
        let config = p.config();
        assert_eq!(stored_start(&config), None);
        assert_eq!(start_of(&config, &Attention::of(&config)), Preset::Usual, "As Sioul does, as 0.1.0 wrote them");
        assert!(!p.read().is_empty(), "Quieter's rows are lines");
        let before = p.bytes();
        let e = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Restricted }, place: Place::Everywhere, when: When::pattern(Pattern::Never), store: None };
        let kept = save(&p.paths(), None, Some(&e), &p.devices()).unwrap().unwrap();
        assert_eq!(stored_start(&p.config()), Some(Preset::Usual), "kept with the first change");
        put_back(&p.paths(), &kept).unwrap();
        assert_eq!(p.bytes(), before, "Undo: the files as they were");
    }

    /// On settings that never kept a start, choosing As Sioul does, the start
    /// they read, writes the start alone: every other byte as it was.
    #[test]
    fn choosing_as_sioul_does_without_a_start_writes_the_start_alone() {
        crate::config::stand_ins_only();
        let p = Here::new("choose-usual", &format!("language = \"en\"\n{}{FOUR_ROWS}", lists_here()));
        let config = p.config();
        let from = start_of(&config, &Attention::of(&config));
        assert_eq!(from, Preset::Usual);
        let lines = p.read();
        let before = std::fs::read_to_string(p.path()).unwrap();
        apply(&p.paths(), &choosing(from, Preset::Usual), Preset::Usual).unwrap();
        let after = std::fs::read_to_string(p.path()).unwrap();
        assert_eq!(stored_start(&p.config()), Some(Preset::Usual));
        let others: String = after.lines().filter(|l| l.trim() != "start = \"usual\"").map(|l| format!("{l}\n")).collect();
        assert_eq!(others, before, "every byte but the start");
        assert_eq!(after.lines().count(), before.lines().count() + 1);
        assert_eq!(p.read(), lines, "the same lines");
    }

    /// Edit touches its own keys alone; saved as it was, it writes nothing; a
    /// line the card cannot say (a phone's table set cell by cell) is offered
    /// as its own choice while it is changed, and survives Edit untouched.
    #[test]
    fn an_edit_touches_its_own_keys_alone() {
        crate::config::stand_ins_only();
        let p = at_start("edit", Preset::Usual);
        let devices = p.devices();
        for e in under_test(&p) {
            save(&p.paths(), None, Some(&e), &devices).unwrap();
        }
        let before = std::fs::read_to_string(p.path()).unwrap();
        let old = as_listed(&p, &under_test(&p)[0]);
        let new = Exception { when: When::pattern(Pattern::Work), store: None, ..old.clone() };
        save(&p.paths(), Some(&old), Some(&new), &devices).unwrap().unwrap();
        let after = std::fs::read_to_string(p.path()).unwrap();
        let changed: Vec<(&str, &str)> = before.lines().zip(after.lines()).filter(|(a, b)| a != b).collect();
        assert_eq!(before.lines().count(), after.lines().count());
        assert_eq!(changed.len(), 1, "{changed:#?}");
        assert!(changed[0].0.starts_with("\"mail.safe\""), "{changed:#?}");
        // Saved as it was: nothing written.
        let now = as_listed(&p, &new);
        assert_eq!(save(&p.paths(), Some(&now), Some(&Exception { store: None, ..now.clone() }), &devices).unwrap(), None);
        assert_eq!(std::fs::read_to_string(p.path()).unwrap(), after);
        // A table set cell by cell on the phone ("none"): its own choice, and Save leaves it.
        crate::calls_line::set_cell(&p.path(), "phone-1", &crate::calls_line::Row::Safe, crate::calls_line::When::Day, crate::calls_line::Cell::Voicemail, &Profile::Professional.grid()).unwrap();
        let table = p.read().into_iter().find(|e| matches!(e.store, Some(Store::Phone { .. }))).unwrap();
        assert_eq!(table.when, When::Calls { profile: "none".into() });
        let offered = offered_for(&table.what, &table.whom, &table.place, Some(&table));
        assert_eq!(offered[0], table.when, "offered as its own");
        assert!(!offered_for(&table.what, &table.whom, &table.place, None).contains(&table.when), "never to a new one");
        let raw = crate::calls_line::raw(&p.path(), "phone-1");
        assert_eq!(save(&p.paths(), Some(&table), Some(&Exception { store: None, ..table.clone() }), &devices).unwrap(), None);
        assert_eq!(crate::calls_line::raw(&p.path(), "phone-1"), raw, "the table untouched");
    }

    /// "Where" names the right device: an app's or a conversation's choice
    /// on this phone alone, a table of calls on a phone alone, everything else
    /// on every device; nothing is written into this device's files for another.
    #[test]
    fn where_names_the_right_device() {
        crate::config::stand_ins_only();
        let p = at_start("where", Preset::Usual);
        let devices = p.devices();
        let laptop = Place::Device { id: "laptop".into(), name: "The laptop".into() };
        let talk = Exception { what: What::Conversation { key: "8c1f0e5a2b7d4c69".into(), title: "Sam".into(), app: "Signal".into() }, whom: Whom::Anyone, place: laptop.clone(), when: When::pattern(Pattern::Always), store: None };
        let notes = std::fs::read_to_string(p.dir.join("app-notes.toml")).unwrap();
        assert_eq!(save(&p.paths(), None, Some(&talk), &devices), Err(Refusal::Words("reach-where-this-phone")));
        assert_eq!(std::fs::read_to_string(p.dir.join("app-notes.toml")).unwrap(), notes, "nothing written for another device");
        let calls = Exception { what: What::Channel { channel: Channel::Calls }, whom: Whom::Anyone, place: laptop.clone(), when: When::Calls { profile: "personal".into() }, store: None };
        assert_eq!(save(&p.paths(), None, Some(&calls), &devices), Err(Refusal::Words("reach-where-phones")));
        let mail = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Safe }, place: laptop, when: When::pattern(Pattern::Always), store: None };
        assert_eq!(place_refused(&mail, &devices), Some("reach-where-every"));
        // A device the sharing does not know: neither a phone nor this one.
        let unknown = Exception { place: Place::Device { id: "gone".into(), name: String::new() }, ..talk };
        assert_eq!(place_refused(&unknown, &devices), Some("reach-where-this-phone"));
        // Said in both languages.
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            for id in ["reach-where-this-phone", "reach-where-phones", "reach-where-every", "reach-same-as-usual", "reach-not-kept", "reach-not-offered", "reach-not-saved"] {
                said_well(&tr.text(id, None), language);
            }
        }
        // A new exception that says what the start already says: refused, in words.
        let same = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Safe }, place: Place::Everywhere, when: When::pattern(Pattern::Awake), store: None };
        assert_eq!(save(&p.paths(), None, Some(&same), &devices), Err(Refusal::Words("reach-same-as-usual")));
    }

    /// Your contacts' messages carry the group conversations' row only while
    /// it reads as theirs or as the start's: a line of its own is left as it is.
    #[test]
    fn group_conversations_keep_a_line_of_their_own() {
        crate::config::stand_ins_only();
        let p = at_start("groups", Preset::Usual);
        let devices = p.devices();
        let groups = Exception { what: What::Channel { channel: Channel::Messages }, whom: Whom::Circle { person: Person::Groups }, place: Place::Everywhere, when: When::pattern(Pattern::Never), store: None };
        save(&p.paths(), None, Some(&groups), &devices).unwrap();
        let contacts = Exception { what: What::Channel { channel: Channel::Messages }, whom: Whom::Circle { person: Person::Neutral }, place: Place::Everywhere, when: When::pattern(Pattern::Always), store: None };
        save(&p.paths(), None, Some(&contacts), &devices).unwrap();
        let read = p.read();
        assert!(read.iter().any(|e| e.whom == Whom::Circle { person: Person::Groups } && e.when == When::pattern(Pattern::Never)), "{read:#?}");
        assert_eq!(as_listed(&p, &contacts).store, Some(Store::Rows { rows: vec!["messages.neutral".into()] }));
        // Taken out, your contacts' line leaves the group conversations' as it was.
        save(&p.paths(), Some(&as_listed(&p, &contacts)), None, &devices).unwrap();
        assert_eq!(as_listed(&p, &groups).when, When::pattern(Pattern::Never));
    }

    /// Your people follow Safe's exception while their rows are not written:
    /// the line says so, and Safe's line decides their cells.
    #[test]
    fn your_people_follow_safe_s_line_and_it_says_so() {
        crate::config::stand_ins_only();
        let p = at_start("follow", Preset::Usual);
        let e = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Safe }, place: Place::Everywhere, when: When::pattern(Pattern::Work), store: None };
        save(&p.paths(), None, Some(&e), &p.devices()).unwrap();
        let attention = Attention::of(&p.config());
        let line = as_listed(&p, &e);
        assert_eq!(followed_by_people(&line, &attention), [Channel::Mail]);
        let tr = Translator::new("en");
        assert_eq!(sentence_for(&tr, &line, &[Channel::Mail]), "Mail from your safe people and your people: in your work hours.");
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            said_well(&sentence_for(&tr, &line, &[Channel::Mail]), language);
            let anything = Exception { what: What::Anything, ..line.clone() };
            said_well(&sentence_for(&tr, &anything, &[Channel::Mail, Channel::Messages]), language);
        }
        let list = p.read();
        let people = Row::People(Channel::Mail, Person::People);
        assert_eq!(decide(&list, &Ask::row(&attention, people), Column::Leisure), list.iter().position(|x| *x == line), "Safe's line decides your people's mail");
        // Their row made theirs: their own, Safe's line no longer.
        let theirs = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::People }, place: Place::Everywhere, when: When::pattern(Pattern::Always), store: None };
        save(&p.paths(), None, Some(&theirs), &p.devices()).unwrap();
        let attention = Attention::of(&p.config());
        assert!(followed_by_people(&as_listed(&p, &e), &attention).is_empty());
    }

    /// Each cell of the worked-out grid against the matrix: the line that
    /// decides it writes that cell; the usual rule's cells are the start's.
    #[test]
    fn each_cell_s_deciding_line_says_the_matrix() {
        crate::config::stand_ins_only();
        let p = at_start("cells", Preset::Quieter);
        let devices = p.devices();
        for e in under_test(&p).into_iter().take(3) {
            save(&p.paths(), None, Some(&e), &devices).unwrap();
        }
        let config = p.config();
        let attention = Attention::of(&config);
        let start = start_of(&config, &attention).matrix();
        let list = p.read();
        for row in Row::ALL.into_iter().filter(|r| !attention::screened(*r) && !matches!(r, Row::People(_, Person::Blocked))) {
            let ask = Ask::row(&attention, row);
            for column in Column::ALL {
                let cell = attention.cell(row, column);
                match decide(&list, &ask, column) {
                    Some(i) => {
                        let e = &list[i];
                        let decider = match row {
                            Row::People(c, Person::People) if attention.follows(row) => Row::People(c, Person::Safe),
                            other => other,
                        };
                        let at = Column::ALL.iter().position(|c| *c == column).unwrap();
                        assert_eq!(when_cells(&e.when, decider, &start).unwrap()[at], cell, "{} at {}: {e:?}", row.id(), column.id());
                    }
                    None => assert_eq!(cell, start.cell(row, column), "{} at {}: the usual rule", row.id(), column.id()),
                }
            }
        }
    }

    /// A group on two lists (two devices at once): shown on the one the lists
    /// decide by (blocked first); Undo puts it back on both.
    #[test]
    fn a_group_on_two_lists() {
        crate::config::stand_ins_only();
        let p = at_start("two-lists", Preset::Usual);
        let config = p.config();
        SenderList::add_category(&List::Safe.path(&config), &p.group()).unwrap();
        SenderList::add_category(&List::Blocked.path(&config), &p.group()).unwrap();
        let line = p.read().into_iter().find(|e| e.whom == Whom::Group { name: p.group() }).unwrap();
        assert_eq!(line.when, When::Like { list: List::Blocked });
        let before = p.bytes();
        let kept = save(&p.paths(), Some(&line), None, &p.devices()).unwrap().unwrap();
        assert!(p.bytes().2.iter().all(String::is_empty), "off both");
        put_back(&p.paths(), &kept).unwrap();
        assert_eq!(p.bytes().2, before.2, "back on both");
        set_category_off(&p);
    }

    fn set_category_off(p: &Here) {
        crate::porch::set_category(&p.config(), &p.group(), None).unwrap();
    }

    /// Your own times say the two layers where they differ; Always through's
    /// "through" is no second "at once".
    #[test]
    fn own_times_say_both_layers() {
        let tr = Translator::new("en");
        let a = Level::As;
        assert_eq!(cells_words(&tr, &[a, a, a, a, a, a, a, Later, Later], Some(Channel::Messages)), "as usual; held during time for you and do-not-disturb");
        assert_ne!(level_words(&tr, Level::Through, None), level_words(&tr, Now, None));
        for language in ["en", "fr"] {
            let tr = Translator::new(language);
            said_well(&capital(&format!("{}.", cells_words(&tr, &[Now, Now, Later, Later, Later, Later, Later, Quiet, Level::Through], Some(Channel::Mail)))), language);
        }
    }

    /// Undo of the first change on settings `[attention]` never held: the
    /// rows the older keys seeded are what it puts back, so the matrix reads
    /// as before.
    #[test]
    fn undo_puts_back_what_the_older_keys_seeded() {
        crate::config::stand_ins_only();
        let p = Here::new("seeded", "language = \"en\"\n[reach]\nneutral = [\"work\", \"admin\", \"leisure\"]\n");
        let before = Attention::of(&p.config());
        let mail = Row::People(Channel::Mail, Person::Neutral);
        assert_eq!(before.cell(mail, Column::Leisure), Level::Now, "seeded by the older key");
        let e = Exception { what: What::Channel { channel: Channel::Mail }, whom: Whom::Circle { person: Person::Neutral }, place: Place::Everywhere, when: When::pattern(Pattern::Never), store: None };
        let kept = save(&p.paths(), None, Some(&e), &p.devices()).unwrap().unwrap();
        assert!(p.config().attention.is_some(), "the first write");
        put_back(&p.paths(), &kept).unwrap();
        assert_eq!(Attention::of(&p.config()), before);
    }
}
