// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Several agendas (docs/client.md, "Several agendas"): what each one is,
//! which of them reach you now, and which of them warn when their events
//! overlap.
//!
//! An agenda is a calendar or a task list (a `vdir` collection of
//! calendars), named by its place, "account/id". Each says whose it is
//! (yours, a child's, another parent's, a partner's, or shared with anyone
//! else), what it is for (the areas' three switches: work alone follows the
//! work hours), when a personal one reaches you (all day, or outside work
//! hours), and what you may do with it (read and write, read only, or its
//! availability only: busy times without a title, a place or notes).
//!
//! What you said is kept in the configuration, `[agenda.calendars."<place>"]`,
//! one field each, and travels with the settings (docs/database.md); every
//! field you did not say comes from what Sioul can see (`Agenda::of`). An
//! agenda that is not yours is shown, never holds your time
//! (`agenda::Occurrence::holds_time`), and warns of overlaps only as the
//! matrix says (`Matrix`). One agenda for availability only is read only
//! for every writer (`vdir::calendar_read_only`, `agenda::guard`), and its
//! events carry no details from the moment they are read
//! (`agenda::file_occurrences_as`).
//!
//! The filter (`Override`, `shown`) decides which agendas the views draw:
//! what the hours admit (a work agenda only in work hours), unless you
//! turned some on or off, which holds until work's hours change.

use crate::areas::{Area, Time};
use crate::config::Config;
use crate::vdir::{self, Collection, Kind};
use jiff::Zoned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Whose an agenda is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Whose {
    #[default]
    Yours,
    /// A child's: one agenda per child, often shared with another parent.
    Child,
    /// Another parent's, shared for what that parent is busy with.
    Parent,
    /// A partner's.
    Partner,
    /// Shared with you by anyone else: a colleague, a club, a subscription.
    Shared,
}

impl Whose {
    /// In the order the filter's groups come.
    pub const ALL: [Whose; 5] = [Whose::Yours, Whose::Child, Whose::Parent, Whose::Partner, Whose::Shared];

    pub fn id(self) -> &'static str {
        match self {
            Whose::Yours => "yours",
            Whose::Child => "child",
            Whose::Parent => "parent",
            Whose::Partner => "partner",
            Whose::Shared => "shared",
        }
    }

    pub fn parse(text: &str) -> Option<Whose> {
        Whose::ALL.into_iter().find(|w| w.id() == text.trim().to_ascii_lowercase())
    }
}

/// What you may do with an agenda.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    /// Read and write: what its server allows unless you say less.
    #[default]
    Write,
    /// Read only: its server's rights, or your choice.
    Read,
    /// Its busy times only, never a title, a place or notes, whatever its
    /// server sends; nothing on it is ever written.
    Availability,
}

impl Access {
    pub fn id(self) -> &'static str {
        match self {
            Access::Write => "write",
            Access::Read => "read",
            Access::Availability => "availability",
        }
    }

    pub fn parse(text: &str) -> Option<Access> {
        [Access::Write, Access::Read, Access::Availability].into_iter().find(|a| a.id() == text.trim().to_ascii_lowercase())
    }
}

/// When a personal agenda reaches you.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reaches {
    #[default]
    AllDay,
    /// Only outside work hours.
    OutsideWork,
}

impl Reaches {
    pub fn id(self) -> &'static str {
        match self {
            Reaches::AllDay => "all-day",
            Reaches::OutsideWork => "outside-work",
        }
    }

    pub fn parse(text: &str) -> Option<Reaches> {
        [Reaches::AllDay, Reaches::OutsideWork].into_iter().find(|r| r.id() == text.trim().to_ascii_lowercase())
    }
}

/// When an agenda reaches you, by the hours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reach {
    Always,
    /// A work agenda: in work hours alone; its events' reminders wait while work rests.
    WorkHours,
    /// A personal agenda you keep out of your work hours.
    OutsideWork,
}

/// What you said of one agenda, each field unsaid until you say it
/// (`[agenda.calendars."<place>"]`). Words, read leniently: a word this
/// Sioul does not know is unsaid, never an error that would stop the
/// configuration from reading.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Said {
    /// "yours", "child", "parent", "partner", "shared".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whose: Option<String>,
    /// Whose name: a child's above all ("Léa").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What it is for: "work", "admin", "leisure", joined by "+".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    /// "all-day" or "outside-work", for a personal agenda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaches: Option<String>,
    /// "write", "read", "availability".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    /// A child's agenda: the other parent who may take its events, by name
    /// and address (part 3, `holders`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_address: Option<String>,
    /// How that parent answers: "sioul", their Sioul reads this agenda and
    /// answers in it; "mail", by an invitation (unsaid: by mail).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_answers: Option<String>,
    /// Who usually takes each kind of its events ("dentist" = "other"):
    /// `[agenda.calendars."<place>".holders]`, proposed for each new one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub holders: BTreeMap<String, String>,
    /// An agenda for availability: "shown" shows what its events say (their
    /// titles), still read only; unsaid, plain busy bands (part 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titles: Option<String>,
    /// Whose it is, as a person of `persons` (docs/persons.md), by id: `name`
    /// beside it keeps their name for older versions. Unsaid, `name` is
    /// matched to a person of its relation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
    /// A child's agenda: the other parent as a person, by id; `parent` keeps
    /// their name, `parent_address` where invitations go.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_person: Option<String>,
}

impl Said {
    /// The fields an agenda's settings may write.
    pub const FIELDS: [&'static str; 11] = ["whose", "name", "area", "reaches", "access", "parent", "parent_address", "parent_answers", "titles", "person", "parent_person"];
}

/// How the other parent of a child's agenda answers (`Said::parent_answers`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Answers {
    /// By an invitation Sioul drafts, which leaves when you send it.
    #[default]
    Mail,
    /// In their own Sioul, which reads this agenda: no mail.
    Sioul,
}

impl Answers {
    pub fn id(self) -> &'static str {
        match self {
            Answers::Mail => "mail",
            Answers::Sioul => "sioul",
        }
    }

    pub fn parse(text: &str) -> Option<Answers> {
        [Answers::Mail, Answers::Sioul].into_iter().find(|a| a.id() == text.trim().to_ascii_lowercase())
    }
}

/// One agenda as it is now: what you said of it, the rest by default.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Agenda {
    /// Its place, "account/id".
    pub id: String,
    pub account: String,
    pub name: String,
    /// "#4c6b5c", its server's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// It takes events; tasks.
    pub events: bool,
    pub tasks: bool,
    pub whose: Whose,
    /// Whose name, when said ("Léa"); "" for yours. Their person's name when
    /// one is theirs (`with_persons`).
    pub person: String,
    /// Whose it is as a person, by id (`persons`); "" when none is.
    pub person_id: String,
    pub area: Area,
    pub reaches: Reaches,
    pub access: Access,
    /// Its server lets you read it only: read and write is not offered.
    pub server_read_only: bool,
    /// A child's agenda: the other parent, by name and address ("" unsaid),
    /// and how they answer (`holders`).
    pub parent: String,
    pub parent_address: String,
    pub parent_answers: Answers,
    /// The other parent as a person, by id; "" when none is.
    pub parent_person: String,
    /// Who usually takes each kind of a child's events.
    pub standing: BTreeMap<crate::holders::Kind, crate::holders::Who>,
    /// For availability: its events' titles shown, by your word (`Said::titles`); off, plain bands.
    pub titles: bool,
    /// What you said of it, field by field.
    pub said: Said,
    /// A calendar of busy blocks Sioul keeps for one of your availabilities
    /// (`availability::Availability::mirrors`): derived from your own events,
    /// no agenda of yours to compare, filter, plan around or pick from.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mirror: bool,
}

impl Agenda {
    /// One collection as an agenda: `said` over the defaults. An agenda in an
    /// account for work alone is for work (its own area, or that of the mail
    /// account of the same address, when either says so); one its server
    /// keeps read only is someone else's, for its availability only when its
    /// events carry no details (`carries_details`), else read only;
    /// everything else is yours, personal, all day, read and write. Read and
    /// write is never more than the server allows.
    pub fn of(collection: &Collection, said: &Said, config: &Config, server_read_only: bool, details: &dyn Fn() -> bool) -> Agenda {
        let whose = said.whose.as_deref().and_then(Whose::parse).unwrap_or(if server_read_only { Whose::Shared } else { Whose::Yours });
        let area = said.area.as_deref().and_then(Area::parse).unwrap_or_else(|| if work_account(config, &collection.account) { Area::WORK } else { Area::PERSONAL });
        let reaches = said.reaches.as_deref().and_then(Reaches::parse).unwrap_or_default();
        let access = match said.access.as_deref().and_then(Access::parse) {
            Some(Access::Write) | None if server_read_only => {
                if said.access.is_none() && !details() {
                    Access::Availability
                } else {
                    Access::Read
                }
            }
            Some(chosen) => chosen,
            None => Access::Write,
        };
        let person = if whose == Whose::Yours { String::new() } else { said.name.as_deref().map(str::trim).unwrap_or_default().to_string() };
        let child = whose == Whose::Child;
        let word = |field: &Option<String>| if child { field.as_deref().map(str::trim).unwrap_or_default().to_string() } else { String::new() };
        let standing = if child { said.holders.iter().filter_map(|(k, w)| Some((crate::holders::Kind::parse(k)?, crate::holders::Who::parse(w)?))).collect() } else { BTreeMap::new() };
        Agenda {
            id: place(collection),
            account: collection.account.clone(),
            name: collection.name.clone(),
            colour: collection.color.clone(),
            events: collection.holds("VEVENT"),
            tasks: collection.holds("VTODO"),
            whose,
            person,
            person_id: String::new(),
            area,
            reaches,
            access,
            server_read_only,
            parent: word(&said.parent),
            parent_address: word(&said.parent_address),
            parent_answers: said.parent_answers.as_deref().and_then(Answers::parse).unwrap_or_default(),
            parent_person: String::new(),
            standing,
            titles: said.titles.as_deref().is_some_and(|t| t.trim().eq_ignore_ascii_case("shown")),
            said: said.clone(),
            mirror: is_mirror(config, &collection.account, &collection.id),
        }
    }

    /// Not yours: shown, warned of as the matrix says, never holding your time.
    pub fn theirs(&self) -> bool {
        self.whose != Whose::Yours
    }

    /// Its busy times only.
    pub fn busy(&self) -> bool {
        self.access == Access::Availability
    }

    /// Nothing on it is written.
    pub fn read_only(&self) -> bool {
        self.access != Access::Write
    }

    /// For work alone: it follows the work hours.
    pub fn for_work(&self) -> bool {
        self.area.work && !self.area.admin && !self.area.leisure
    }

    /// When it reaches you: a work agenda in work hours; a personal one all
    /// day, or outside work hours when you said so; one for work and for
    /// you, all day.
    pub fn reach(&self) -> Reach {
        if self.for_work() {
            Reach::WorkHours
        } else if !self.area.work && self.reaches == Reaches::OutsideWork {
            Reach::OutsideWork
        } else {
            Reach::Always
        }
    }

    /// The filter's group: whose, and whose person ("child", their id), else
    /// whose name, case and accents aside ("child", "lea"): two agendas of one
    /// child are one group whatever their names' spelling.
    pub fn group(&self) -> (Whose, String) {
        (self.whose, if self.person_id.is_empty() { crate::persons::key(&self.person) } else { self.person_id.clone() })
    }

    /// Its persons (docs/persons.md): whose it is (`Said::person`, else its
    /// name matched to a person of its relation) and, a child's, the other
    /// parent (`Said::parent_person`, else `parent` matched to a child's other
    /// parent), each shown by their person's name now. Nothing is written.
    pub fn with_persons(mut self, persons: &crate::persons::Persons) -> Agenda {
        use crate::persons::Relation;
        let relation = match self.whose {
            Whose::Child => Relation::Child,
            Whose::Partner => Relation::Partner,
            Whose::Parent => Relation::OtherParent,
            Whose::Yours | Whose::Shared => return self,
        };
        if let Some(person) = persons.of_agenda(&self.said, relation) {
            self.person_id = person.id.clone();
            self.person = person.name.clone();
        }
        if self.whose == Whose::Child
            && let Some(parent) = persons.parent_of_agenda(&self.said)
        {
            self.parent_person = parent.id.clone();
            self.parent = parent.name.clone();
        }
        self
    }
}

/// A collection's place, "account/id", as everything names it.
pub fn place(collection: &Collection) -> String {
    format!("{}/{}", collection.account, collection.id)
}

/// Whether an account is for work alone: its own area says so, or the mail
/// account of the same address says so (a mail address left unsaid counts
/// for nothing here: an agenda left unsaid stays yours, all day).
fn work_account(config: &Config, account: &str) -> bool {
    let only_work = |area: Option<&str>| area.and_then(Area::parse).is_some_and(|a| a.work && !a.admin && !a.leisure);
    let Some(own) = config.every_account().find(|a| a.id == account) else { return false };
    if only_work(own.area.as_deref()) {
        return true;
    }
    let Some(address) = own.address.as_deref().map(str::trim).filter(|a| !a.is_empty()) else { return false };
    config.every_account().filter(|a| a.id != account && a.address.as_deref().is_some_and(|other| other.trim().eq_ignore_ascii_case(address))).any(|a| only_work(a.area.as_deref()))
}

/// Whether a calendar is one of busy blocks Sioul keeps for an availability
/// (its mirrors, and the calendar of the single share of before).
pub fn is_mirror(config: &Config, account: &str, id: &str) -> bool {
    config.agenda.availabilities.iter().flat_map(|a| a.mirrors.iter()).any(|m| m.account == account && m.calendar == id)
        || config.agenda.availability.as_ref().is_some_and(|single| single.account == account && !single.calendar.is_empty() && single.calendar == id)
}

/// Every agenda here, by account then name: the calendars and the task lists.
pub fn all(config: &Config) -> Vec<Agenda> {
    vdir::collections(Kind::Calendars).iter().map(|c| agenda_of(c, config)).collect()
}

/// The agendas as the configuration says now (`read_config`).
pub fn current() -> Vec<Agenda> {
    all(&read_config())
}

/// One collection as an agenda, with the configuration given.
pub fn agenda_of(collection: &Collection, config: &Config) -> Agenda {
    let id = place(collection);
    let said = config.agenda.calendars.get(&id).cloned().unwrap_or_default();
    let server = vdir::server_read_only(&collection.account, &collection.id);
    Agenda::of(collection, &said, config, server, &|| carries_details(&collection.dir)).with_persons(&crate::persons::Persons::cached())
}

/// What the readers of a calendar's events need of its agenda, the
/// configuration read as it is now (`read_config`): its place, whether it is
/// someone else's, for its availability only, for work alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    pub id: String,
    pub name: String,
    pub theirs: bool,
    pub busy: bool,
    pub work: bool,
    /// Its server says it takes none of your time as a whole (`vdir::calendar_transparent`).
    pub transparent: bool,
    /// A personal agenda you keep out of your work hours (`Reach::OutsideWork`).
    pub outside_work: bool,
    /// A child's agenda.
    pub child: bool,
    /// For availability, its events' titles shown by your word (`Agenda::titles`).
    pub titles: bool,
}

/// The marks of a collection's agenda (`Marks`).
pub fn marks_of(collection: &Collection) -> Marks {
    let config = read_config();
    let agenda = agenda_of(collection, &config);
    let transparent = vdir::calendar_transparent(&collection.account, &collection.id);
    Marks { id: agenda.id.clone(), name: agenda.name.clone(), theirs: agenda.theirs(), busy: agenda.busy(), work: agenda.for_work(), transparent, outside_work: agenda.reach() == Reach::OutsideWork, child: agenda.whose == Whose::Child, titles: agenda.titles }
}

/// Whether you chose to keep an agenda from every writer: read only, or its
/// availability only (`[agenda.calendars."<place>"] access`). What its server
/// says is the sync state's (`vdir::server_read_only`).
pub fn chosen_read_only(account: &str, id: &str) -> bool {
    let config = read_config();
    config.agenda.calendars.get(&format!("{account}/{id}")).and_then(|s| s.access.as_deref()).and_then(Access::parse).is_some_and(|a| a != Access::Write)
}

/// Whether a calendar's events say more than that their time is taken: any
/// place, notes or guest, or two different titles. A calendar shared for
/// its availability alone sends one word ("Busy", « Occupé ») or nothing.
/// None yet: it may say more later, so it counts as saying it. Read again
/// only when its folder changed (its time and count).
pub fn carries_details(dir: &Path) -> bool {
    type Seen = BTreeMap<PathBuf, (Option<std::time::SystemTime>, usize, bool)>;
    static SEEN: Mutex<Seen> = Mutex::new(BTreeMap::new());
    let items: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "ics")).collect();
    let time = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    if let Some((t, n, said)) = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(dir)
        && *t == time
        && *n == items.len()
    {
        return *said;
    }
    let said = items_say_details(items.iter().filter_map(|p| std::fs::read_to_string(p).ok()));
    SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(dir.to_path_buf(), (time, items.len(), said));
    said
}

/// Whether these files' events say more than their times (`carries_details`).
fn items_say_details(texts: impl Iterator<Item = String>) -> bool {
    let mut titles = BTreeSet::new();
    let mut events = 0;
    for text in texts {
        let mut inside = false;
        for line in crate::lines::unfold(&text) {
            let upper = line.to_ascii_uppercase();
            if upper.starts_with("BEGIN:VEVENT") {
                inside = true;
                events += 1;
                continue;
            }
            if upper.starts_with("END:VEVENT") {
                inside = false;
                continue;
            }
            if !inside {
                continue;
            }
            let Some((head, value)) = line.split_once(':') else { continue };
            let name = head.split(';').next().unwrap_or_default().to_ascii_uppercase();
            let value = value.trim();
            match name.as_str() {
                "LOCATION" | "DESCRIPTION" | "ATTENDEE" | "URL" if !value.is_empty() => return true,
                "SUMMARY" => {
                    titles.insert(crate::lines::unescape(value).trim().to_lowercase());
                }
                _ => {}
            }
        }
    }
    events == 0 || titles.len() > 1
}

// ---------------------------------------------------------------- the configuration, as it is now

/// The configuration as it is now, parsed again only when the file changed:
/// read at each calendar file, at each write (`chosen_read_only`). A file
/// younger than two seconds is compared byte for byte, as two writings of
/// one size within a tick of the clock look alike by their stamps.
pub fn read_config() -> Arc<Config> {
    type Kept = Option<(PathBuf, u64, Option<std::time::SystemTime>, Vec<u8>, Arc<Config>)>;
    static KEPT: Mutex<Kept> = Mutex::new(None);
    let path = crate::config::default_path();
    let meta = std::fs::metadata(&path).ok();
    let (size, time) = (meta.as_ref().map_or(0, std::fs::Metadata::len), meta.as_ref().and_then(|m| m.modified().ok()));
    let settled = time.is_some_and(|t| t.elapsed().is_ok_and(|age| age > std::time::Duration::from_secs(2)));
    let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((p, s, t, _, config)) = kept.as_ref()
        && *p == path
        && *s == size
        && *t == time
        && settled
    {
        return Arc::clone(config);
    }
    let bytes = std::fs::read(&path).unwrap_or_default();
    if let Some((p, _, _, b, config)) = kept.as_ref()
        && *p == path
        && *b == bytes
    {
        let config = Arc::clone(config);
        *kept = Some((path, size, time, bytes, Arc::clone(&config)));
        return config;
    }
    let config = Arc::new(
        std::str::from_utf8(&bytes)
            .ok()
            .and_then(|text| toml::from_str::<Config>(text).ok())
            .map(|mut config| {
                // As `Config::load` makes it: sites apart, accounts switched off apart.
                config.settle();
                config
            })
            .unwrap_or_default(),
    );
    *kept = Some((path, size, time, bytes, Arc::clone(&config)));
    config
}

// ---------------------------------------------------------------- writing what you said

/// One field of an agenda's settings written into the configuration, its
/// comments kept; "" takes it out (back to the default). `field` is one of
/// `Said::FIELDS`; a word that does not read is refused.
pub fn set_field(path: &Path, id: &str, field: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    let written: Option<String> = match field {
        _ if value.is_empty() => None,
        "whose" => Some(Whose::parse(value).ok_or_else(|| format!("{value}: yours, child, parent, partner or shared"))?.id().to_string()),
        "name" => Some(value.to_string()),
        "area" => Some(Area::parse(value).ok_or_else(|| format!("{value}: work, admin, leisure, joined by +"))?.id()),
        "reaches" => Some(Reaches::parse(value).ok_or_else(|| format!("{value}: all-day or outside-work"))?.id().to_string()),
        "access" => Some(Access::parse(value).ok_or_else(|| format!("{value}: write, read or availability"))?.id().to_string()),
        "parent" => Some(value.to_string()),
        "parent_address" => Some(crate::compose::address_of(value).ok_or_else(|| format!("{value}: not an address"))?),
        "parent_answers" => Some(Answers::parse(value).ok_or_else(|| format!("{value}: mail or sioul"))?.id().to_string()),
        // A person of `persons`, by id (docs/persons.md).
        "person" | "parent_person" => Some(value.to_string()),
        "titles" if value.eq_ignore_ascii_case("shown") => Some("shown".to_string()),
        "titles" => return Err(format!("{value}: shown, or nothing")),
        _ => return Err(format!("{field}: not a field of an agenda")),
    };
    edit_agenda_table(path, "calendars", |calendars| {
        match written {
            Some(text) => {
                if !calendars.contains_key(id) {
                    let mut table = toml_edit::Table::new();
                    table.set_implicit(false);
                    calendars.insert(id, toml_edit::Item::Table(table));
                }
                let table = calendars.get_mut(id).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: not a table"))?;
                table.insert(field, toml_edit::value(text));
            }
            None => {
                if let Some(table) = calendars.get_mut(id).and_then(toml_edit::Item::as_table_like_mut) {
                    table.remove(field);
                    if table.is_empty() {
                        calendars.remove(id);
                    }
                }
            }
        }
        Ok(())
    })
}

/// Who usually takes a kind of a child agenda's events (`Said::holders`):
/// "me", "other"; none takes your word out (each one asked then).
pub fn set_standing(path: &Path, id: &str, kind: crate::holders::Kind, who: Option<crate::holders::Who>) -> Result<(), String> {
    edit_agenda_table(path, "calendars", |calendars| {
        if who.is_some() && !calendars.contains_key(id) {
            let mut table = toml_edit::Table::new();
            table.set_implicit(false);
            calendars.insert(id, toml_edit::Item::Table(table));
        }
        let Some(table) = calendars.get_mut(id).and_then(toml_edit::Item::as_table_like_mut) else { return Ok(()) };
        match who {
            Some(who) => {
                if !table.contains_key("holders") {
                    let mut holders = toml_edit::Table::new();
                    holders.set_implicit(false);
                    table.insert("holders", toml_edit::Item::Table(holders));
                }
                let holders = table.get_mut("holders").and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: holders is not a table"))?;
                holders.insert(kind.id(), toml_edit::value(who.id()));
            }
            None => {
                if let Some(holders) = table.get_mut("holders").and_then(toml_edit::Item::as_table_like_mut) {
                    holders.remove(kind.id());
                    if holders.is_empty() {
                        table.remove("holders");
                    }
                }
            }
        }
        if table.is_empty() {
            calendars.remove(id);
        }
        Ok(())
    })
}

/// Where your stand-ins of a kind go (`[agenda.stand_ins]`, an agenda's
/// place); none: your personal agenda (`holders::stand_in_agenda`).
pub fn set_stand_in_agenda(path: &Path, kind: crate::holders::Kind, place: Option<&str>) -> Result<(), String> {
    edit_agenda_table(path, "stand_ins", |table| {
        match place.map(str::trim).filter(|p| !p.is_empty()) {
            Some(place) => {
                table.insert(kind.id(), toml_edit::value(place));
            }
            None => {
                table.remove(kind.id());
            }
        }
        Ok(())
    })
}

/// How many months after a held event of a kind its follow-up comes
/// (`[agenda.rhythms]`); none: the kind's own (`holders::Kind::rhythm`), 0 none at all.
pub fn set_rhythm(path: &Path, kind: crate::holders::Kind, months: Option<u32>) -> Result<(), String> {
    edit_agenda_table(path, "rhythms", |table| {
        match months {
            Some(months) => {
                table.insert(kind.id(), toml_edit::value(i64::from(months.min(60))));
            }
            None => {
                table.remove(kind.id());
            }
        }
        Ok(())
    })
}

/// A named set of agendas for the filter (research/agendas.md §6,
/// recommendation 17, after Fantastical's sets and TimeTree's filters,
/// §3): `[agenda.sets."Tom and Léa"]`, its agendas by place, and Sioul's own
/// times that bring it by themselves ("work", "admin", "leisure"); never a
/// place, which would need tracking. It travels with the settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Set {
    #[serde(default)]
    pub agendas: Vec<String>,
    /// In these times it is what the filter shows unless you say; none: chosen by hand alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub times: Vec<String>,
}

/// The times a set may come with: Sioul's own, never a place.
pub const SET_TIMES: [&str; 3] = ["work", "admin", "leisure"];

/// The set the time brings now, the first by name: one tied to `time`
/// (the hours' word: "work", "admin", "leisure", "work+admin" for both).
pub fn set_of_time<'a>(sets: &'a BTreeMap<String, Set>, time: &str) -> Option<(&'a String, &'a Set)> {
    sets.iter().find(|(_, set)| set.times.iter().any(|t| time.split('+').any(|now| now == t)))
}

/// A set written by its name, or taken out (none); the times kept to Sioul's own.
pub fn set_set(path: &Path, name: &str, set: Option<&Set>) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a set needs a name".into());
    }
    edit_agenda_table(path, "sets", |sets| {
        match set {
            Some(set) => {
                let mut table = toml_edit::Table::new();
                table.set_implicit(false);
                table.insert("agendas", toml_edit::value(set.agendas.iter().map(String::as_str).collect::<toml_edit::Array>()));
                let times: toml_edit::Array = set.times.iter().filter(|t| SET_TIMES.contains(&t.as_str())).map(String::as_str).collect();
                if !times.is_empty() {
                    table.insert("times", toml_edit::value(times));
                }
                sets.insert(name, toml_edit::Item::Table(table));
            }
            None => {
                sets.remove(name);
            }
        }
        Ok(())
    })
}

/// Your availability's one share of before written (`[agenda.availability]`),
/// or taken out (none): the window's way until it moves to the list
/// (`put_availability`); read once and moved by `config::migrate_availability`.
pub fn set_availability(path: &Path, availability: Option<&crate::availability::Single>) -> Result<(), String> {
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        if !doc.contains_key("agenda") {
            if availability.is_none() {
                return Ok(());
            }
            doc["agenda"] = toml_edit::table();
        }
        let agenda = doc["agenda"].as_table_mut().ok_or_else(|| format!("{}: agenda is not a table", path.display()))?;
        match availability {
            Some(availability) => {
                let text = toml::to_string(availability).map_err(|e| e.to_string())?;
                let table: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
                let mut table = table.as_table().clone();
                table.set_implicit(false);
                agenda.insert("availability", toml_edit::Item::Table(table));
            }
            None => {
                agenda.remove("availability");
            }
        }
        if agenda.is_empty() {
            doc.remove("agenda");
        }
        crate::config::write_document(path, &doc)?;
        crate::changes::said(crate::changes::Kind::Agendas);
        Ok(())
    })
}

// ---------------------------------------------------------------- your availabilities

/// The list `[[agenda.availabilities]]` of a document, made when `make` (else
/// none when it is not there). A list written inline (by hand) becomes a
/// list of tables, its elements as they were.
pub(crate) fn availabilities_in(doc: &mut toml_edit::DocumentMut, make: bool) -> Result<Option<&mut toml_edit::ArrayOfTables>, String> {
    if !doc.contains_key("agenda") {
        if !make {
            return Ok(None);
        }
        doc["agenda"] = toml_edit::table();
    }
    let agenda = doc["agenda"].as_table_mut().ok_or("agenda is not a table")?;
    match agenda.get("availabilities") {
        None if !make => return Ok(None),
        None => {
            agenda.insert("availabilities", toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()));
        }
        Some(toml_edit::Item::Value(toml_edit::Value::Array(elements))) => {
            let mut tables = toml_edit::ArrayOfTables::new();
            for element in elements.iter() {
                let inline = element.as_inline_table().ok_or("agenda.availabilities: not a list of tables")?;
                tables.push(inline.clone().into_table());
            }
            agenda.insert("availabilities", toml_edit::Item::ArrayOfTables(tables));
        }
        Some(toml_edit::Item::ArrayOfTables(_)) => {}
        Some(_) => return Err("agenda.availabilities: not a list of tables".into()),
    }
    Ok(agenda.get_mut("availabilities").and_then(toml_edit::Item::as_array_of_tables_mut))
}

/// A list of words as TOML.
fn words_array(words: &[String]) -> toml_edit::Array {
    words.iter().map(String::as_str).collect()
}

/// An availability's own fields written into its table; what this Sioul
/// does not know of it (a newer one's fields) kept as it was.
fn set_availability_fields(table: &mut toml_edit::Table, availability: &crate::availability::Availability) {
    table.insert("id", toml_edit::value(availability.id.as_str()));
    table.insert("name", toml_edit::value(availability.name.as_str()));
    table.insert("agendas", toml_edit::value(words_array(&availability.agendas)));
    let words: Vec<(&String, &String)> = availability.words.iter().filter(|(_, w)| !w.trim().is_empty()).collect();
    if words.is_empty() {
        table.remove("words");
    } else {
        let mut inline = toml_edit::InlineTable::new();
        for (place, word) in words {
            inline.insert(place.as_str(), word.trim().into());
        }
        table.insert("words", toml_edit::value(inline));
    }
    table.insert("rounding", toml_edit::value(i64::from(availability.rounding)));
    table.insert("horizon", toml_edit::value(i64::from(availability.days_ahead())));
    table.insert("closing", toml_edit::value(availability.closing));
}

fn set_mirror_fields(table: &mut toml_edit::Table, mirror: &crate::availability::Mirror) {
    table.insert("account", toml_edit::value(mirror.account.as_str()));
    table.insert("calendar", toml_edit::value(mirror.calendar.as_str()));
    table.insert("transparent", toml_edit::value(mirror.transparent));
}

fn set_share_fields(table: &mut toml_edit::Table, share: &crate::availability::Share) {
    for (field, text) in [("audience", &share.audience), ("who", &share.who), ("way", &share.way), ("account", &share.account), ("link", &share.link), ("made", &share.made)] {
        table.insert(field, toml_edit::value(text.trim()));
    }
}

/// What a table holds at a field, as text.
fn text_at<'t>(table: &'t toml_edit::Table, field: &str) -> &'t str {
    table.get(field).and_then(toml_edit::Item::as_str).unwrap_or_default()
}

fn mirror_of(table: &toml_edit::Table) -> crate::availability::Mirror {
    crate::availability::Mirror { account: text_at(table, "account").into(), calendar: text_at(table, "calendar").into(), transparent: table.get("transparent").and_then(toml_edit::Item::as_bool).unwrap_or(false) }
}

fn share_of(table: &toml_edit::Table) -> crate::availability::Share {
    let text = |field: &str| text_at(table, field).to_string();
    crate::availability::Share { audience: text("audience"), who: text("who"), way: text("way"), account: text("account"), link: text("link"), made: text("made") }
}

/// The list of tables at `name` in `table`, made when not there (an inline one becoming tables).
fn tables_at<'t>(table: &'t mut toml_edit::Table, name: &str) -> Result<&'t mut toml_edit::ArrayOfTables, String> {
    if let Some(toml_edit::Item::Value(toml_edit::Value::Array(elements))) = table.get(name) {
        let mut tables = toml_edit::ArrayOfTables::new();
        for element in elements.iter() {
            tables.push(element.as_inline_table().ok_or_else(|| format!("{name}: not a list of tables"))?.clone().into_table());
        }
        table.insert(name, toml_edit::Item::ArrayOfTables(tables));
    }
    if !table.contains_key(name) {
        table.insert(name, toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()));
    }
    table.get_mut(name).and_then(toml_edit::Item::as_array_of_tables_mut).ok_or_else(|| format!("{name}: not a list of tables"))
}

/// The element of `list` that `matches`, its fields set by `set`; added when none does.
fn put_element(list: &mut toml_edit::ArrayOfTables, matches: impl Fn(&toml_edit::Table) -> bool, set: impl Fn(&mut toml_edit::Table)) {
    let found = list.iter().position(|t| matches(t));
    match found {
        Some(at) => {
            if let Some(table) = list.get_mut(at) {
                set(table);
            }
        }
        None => {
            let mut table = toml_edit::Table::new();
            set(&mut table);
            list.push(table);
        }
    }
}

/// An availability as a table of its own, its mirrors and shares within.
pub(crate) fn availability_table(availability: &crate::availability::Availability) -> toml_edit::Table {
    let mut table = toml_edit::Table::new();
    set_availability_fields(&mut table, availability);
    let mut mirrors = toml_edit::ArrayOfTables::new();
    for mirror in &availability.mirrors {
        let mut one = toml_edit::Table::new();
        set_mirror_fields(&mut one, mirror);
        mirrors.push(one);
    }
    if !mirrors.is_empty() {
        table.insert("mirrors", toml_edit::Item::ArrayOfTables(mirrors));
    }
    let mut shares = toml_edit::ArrayOfTables::new();
    for share in &availability.shares {
        let mut one = toml_edit::Table::new();
        set_share_fields(&mut one, share);
        shares.push(one);
    }
    if !shares.is_empty() {
        table.insert("shares", toml_edit::Item::ArrayOfTables(shares));
    }
    table
}

/// One availability of the list changed by `change` (by its id), under the
/// file's lock; the list made when `make`, taken out once empty, and the
/// settings' format marked (`config::FORMAT`). An unknown id is said.
fn edit_availability(path: &Path, id: &str, make: bool, change: impl FnOnce(&mut toml_edit::ArrayOfTables) -> Result<(), String>) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("an availability needs its id".into());
    }
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let Some(list) = availabilities_in(&mut doc, make).map_err(|e| format!("{}: {e}", path.display()))? else { return Err(format!("{id}: no such availability")) };
        change(list)?;
        let empty = list.is_empty();
        if empty && let Some(agenda) = doc.get_mut("agenda").and_then(toml_edit::Item::as_table_mut) {
            agenda.remove("availabilities");
            if agenda.is_empty() {
                doc.remove("agenda");
            }
        }
        crate::config::mark_format(&mut doc);
        crate::config::write_document(path, &doc)?;
        // Your availabilities, said to every view of them (Agenda ⚙, the agenda's mirrors left out).
        crate::changes::said(crate::changes::Kind::Agendas);
        Ok(())
    })
}

/// The table of availability `id` in a list, or what to say.
fn availability_at<'l>(list: &'l mut toml_edit::ArrayOfTables, id: &str) -> Result<&'l mut toml_edit::Table, String> {
    list.iter_mut().find(|t| text_at(t, "id") == id).ok_or_else(|| format!("{id}: no such availability"))
}

/// An availability written (`[[agenda.availabilities]]`), by its id: added,
/// or its fields, mirrors (by account) and shares (by way, whom and
/// account) made what it says, each element's other fields kept; those it
/// no longer holds taken out.
pub fn put_availability(path: &Path, availability: &crate::availability::Availability) -> Result<(), String> {
    edit_availability(path, &availability.id, true, |list| {
        if !list.iter().any(|t| text_at(t, "id") == availability.id) {
            list.push(availability_table(availability));
            return Ok(());
        }
        let table = availability_at(list, &availability.id)?;
        set_availability_fields(table, availability);
        let mirrors = tables_at(table, "mirrors")?;
        let wanted: Vec<&str> = availability.mirrors.iter().map(|m| m.account.as_str()).collect();
        mirrors.retain(|t| wanted.contains(&text_at(t, "account")));
        for mirror in &availability.mirrors {
            put_element(mirrors, |t| text_at(t, "account") == mirror.account, |t| set_mirror_fields(t, mirror));
        }
        let none = mirrors.is_empty();
        if none {
            table.remove("mirrors");
        }
        let shares = tables_at(table, "shares")?;
        shares.retain(|t| availability.shares.iter().any(|s| s.same(&share_of(t))));
        for share in &availability.shares {
            put_element(shares, |t| share.same(&share_of(t)), |t| set_share_fields(t, share));
        }
        if shares.is_empty() {
            table.remove("shares");
        }
        Ok(())
    })
}

/// Your working hours told to an account's server, or no longer
/// (`[agenda] send_hours`, by account id).
pub fn set_send_hours(path: &Path, account: &str, on: bool) -> Result<(), String> {
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        if !doc.contains_key("agenda") {
            if !on {
                return Ok(());
            }
            doc["agenda"] = toml_edit::table();
        }
        let agenda = doc["agenda"].as_table_mut().ok_or_else(|| format!("{}: agenda is not a table", path.display()))?;
        let mut accounts: Vec<String> = agenda.get("send_hours").and_then(toml_edit::Item::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
        accounts.retain(|a| a != account);
        if on {
            accounts.push(account.to_string());
        }
        if accounts.is_empty() {
            agenda.remove("send_hours");
        } else {
            agenda.insert("send_hours", toml_edit::value(words_array(&accounts)));
        }
        if agenda.is_empty() {
            doc.remove("agenda");
        }
        crate::config::write_document(path, &doc)?;
        crate::changes::said(crate::changes::Kind::Agendas);
        Ok(())
    })
}

/// An availability as its table says it; none when it does not read.
fn availability_of(table: &toml_edit::Table) -> Option<crate::availability::Availability> {
    toml::from_str(&toml_edit::DocumentMut::from(table.clone()).to_string()).ok()
}

/// An availability taken out, by its id: what it was, for Undo; none when it was not there.
pub fn remove_availability(path: &Path, id: &str) -> Result<Option<crate::availability::Availability>, String> {
    let mut found = None;
    let result = edit_availability(path, id, false, |list| {
        found = list.iter().find(|t| text_at(t, "id") == id).and_then(availability_of);
        if !list.iter().any(|t| text_at(t, "id") == id) {
            return Err(String::new());
        }
        list.retain(|t| text_at(t, "id") != id);
        Ok(())
    });
    match result {
        // Not there: nothing to take out, nothing written.
        Err(e) if e.is_empty() || e.ends_with("no such availability") => Ok(None),
        Err(e) => Err(e),
        Ok(()) => Ok(found),
    }
}

/// An availability's mirror in an account written (made, or marked transparent).
pub fn put_mirror(path: &Path, id: &str, mirror: &crate::availability::Mirror) -> Result<(), String> {
    edit_availability(path, id, false, |list| {
        let table = availability_at(list, id)?;
        put_element(tables_at(table, "mirrors")?, |t| text_at(t, "account") == mirror.account, |t| set_mirror_fields(t, mirror));
        Ok(())
    })
}

/// An availability's mirror in an account taken out: what it was, if it was there.
pub fn remove_mirror(path: &Path, id: &str, account: &str) -> Result<Option<crate::availability::Mirror>, String> {
    let mut found = None;
    edit_availability(path, id, false, |list| {
        let table = availability_at(list, id)?;
        let mirrors = tables_at(table, "mirrors")?;
        found = mirrors.iter().find(|t| text_at(t, "account") == account).map(mirror_of);
        mirrors.retain(|t| text_at(t, "account") != account);
        if mirrors.is_empty() {
            table.remove("mirrors");
        }
        Ok(())
    })?;
    Ok(found)
}

/// A share of an availability written: added, or the same share (`Share::same`)
/// changed, as when its server said yes (`made`) or gave its link.
pub fn put_share(path: &Path, id: &str, share: &crate::availability::Share) -> Result<(), String> {
    edit_availability(path, id, false, |list| {
        let table = availability_at(list, id)?;
        put_element(tables_at(table, "shares")?, |t| share.same(&share_of(t)), |t| set_share_fields(t, share));
        Ok(())
    })
}

/// A share of an availability taken out (`Share::same`): what it was, for Undo, if it was there.
pub fn remove_share(path: &Path, id: &str, share: &crate::availability::Share) -> Result<Option<crate::availability::Share>, String> {
    let mut found = None;
    edit_availability(path, id, false, |list| {
        let table = availability_at(list, id)?;
        let shares = tables_at(table, "shares")?;
        found = shares.iter().map(share_of).find(|s| s.same(share));
        shares.retain(|t| !share.same(&share_of(t)));
        if shares.is_empty() {
            table.remove("shares");
        }
        Ok(())
    })?;
    Ok(found)
}

/// Follow-up tasks on or off (`[agenda] follow_ups`): on, the default, takes your word out.
pub fn set_follow_ups(path: &Path, on: bool) -> Result<(), String> {
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        if !doc.contains_key("agenda") {
            if on {
                return Ok(());
            }
            doc["agenda"] = toml_edit::table();
        }
        let agenda = doc["agenda"].as_table_mut().ok_or_else(|| format!("{}: agenda is not a table", path.display()))?;
        if on {
            agenda.remove("follow_ups");
        } else {
            agenda.insert("follow_ups", toml_edit::value(false));
        }
        if agenda.is_empty() {
            doc.remove("agenda");
        }
        crate::config::write_document(path, &doc)
    })
}

/// A pair of the matrix ticked or not, as you said it (`Matrix::key`); none
/// takes your word out, back to the default.
pub fn set_warns(path: &Path, a: &str, b: &str, warns: Option<bool>) -> Result<(), String> {
    let key = Matrix::key(a, b);
    edit_agenda_table(path, "overlaps", |pairs| {
        match warns {
            Some(on) => {
                pairs.insert(&key, toml_edit::value(on));
            }
            None => {
                pairs.remove(&key);
            }
        }
        Ok(())
    })
}

/// `[agenda.<name>]` changed by `change`, under the file's lock, which the
/// sharing takes too; an empty table taken out.
fn edit_agenda_table(path: &Path, name: &str, change: impl FnOnce(&mut toml_edit::Table) -> Result<(), String>) -> Result<(), String> {
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        if !doc.contains_key("agenda") {
            doc["agenda"] = toml_edit::table();
        }
        let agenda = doc["agenda"].as_table_mut().ok_or_else(|| format!("{}: agenda is not a table", path.display()))?;
        if !agenda.contains_key(name) {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            agenda.insert(name, toml_edit::Item::Table(table));
        }
        let table = agenda.get_mut(name).and_then(toml_edit::Item::as_table_mut).ok_or_else(|| format!("{}: agenda.{name} is not a table", path.display()))?;
        change(table)?;
        if table.is_empty() {
            agenda.remove(name);
        }
        crate::config::write_document(path, &doc)?;
        // What an agenda is, said to every view of it (Agenda ⚙, a person's page, the agenda).
        crate::changes::said(crate::changes::Kind::Agendas);
        Ok(())
    })
}

// ---------------------------------------------------------------- the hours, and the filter

/// Work's hours now, as the agendas follow them: the hours you set, their
/// overrides ("Work until tonight", "Done for today"), time off, Free time
/// and the pause; a meal or the night inside work hours is still work's
/// day, so a work agenda stays while you eat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hours {
    /// Some hours are set: without any, every agenda reaches you.
    pub set: bool,
    /// Work's hours are on now.
    pub work: bool,
    /// When that next changes (Unix seconds), within a fortnight.
    pub until: Option<i64>,
}

impl Hours {
    /// No hours: everything reaches you.
    pub const NONE: Hours = Hours { set: false, work: false, until: None };

    /// At `now`, by the week's hours and time off of `config` and `overrides`.
    pub fn at(config: &Config, overrides: &crate::quiet::Overrides, now: &Zoned) -> Hours {
        let windows = config.week_hours();
        let blank = crate::quiet::Blocks::default();
        let at = |when: &Zoned| crate::quiet::mode(&windows, &config.time_off, overrides, &blank, when);
        let state = |mode: &crate::quiet::Mode| (mode.time != Time::Any, mode.time.works());
        let first = at(now);
        let (set, work) = state(&first);
        // The next change of what work's hours say, a frame at a time (admin's hours open and close between).
        let mut until = None;
        let mut next = first.until.clone();
        let horizon = now.timestamp().as_second() + 14 * 86_400;
        for _ in 0..32 {
            let Some(when) = next.take().filter(|w| w.timestamp().as_second() <= horizon) else { break };
            let mode = at(&when);
            if state(&mode) != (set, work) {
                until = Some(when.timestamp().as_second());
                break;
            }
            next = mode.until.filter(|n| n.timestamp() > when.timestamp());
        }
        Hours { set, work, until }
    }

    /// As the files say now: the configuration, the hours' overrides.
    pub fn now(config: &Config, now: &Zoned) -> Hours {
        Hours::at(config, &crate::quiet::Overrides::load(&crate::quiet::Overrides::default_path()), now)
    }
}

/// Whether the hours admit an agenda now: a work agenda in work hours, a
/// personal one kept outside them outside them; no hours set, everything.
pub fn usual(agenda: &Agenda, hours: Hours) -> bool {
    admits(agenda.reach(), hours)
}

/// Whether the hours admit an agenda of this reach (`usual`): what the home
/// card asks of each event's (`agenda::Occurrence::reach`), its frames' hours apart.
pub fn admits(reach: Reach, hours: Hours) -> bool {
    match reach {
        Reach::Always => true,
        Reach::WorkHours => !hours.set || hours.work,
        Reach::OutsideWork => !hours.set || !hours.work,
    }
}

/// Agendas turned on or off against the hours, on this device
/// (`agenda-view.toml`'s `[filter]`, the window's): it holds until work's
/// hours change, then the hours decide again; "As the hours say" ends it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Override {
    /// Whether work's hours were on when it was made.
    #[serde(default)]
    pub work: bool,
    /// When they were to change then (Unix seconds); none: until you say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
    /// Shown though the hours would leave them out, by place.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub on: BTreeSet<String>,
    /// Left out though the hours would show them.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub off: BTreeSet<String>,
}

impl Override {
    /// Whether it still holds at `now`: work's hours say what they said
    /// when it was made, and their change has not come.
    pub fn holds(&self, hours: Hours, now: i64) -> bool {
        self.work == hours.work && self.until.is_none_or(|until| now < until)
    }

    pub fn is_empty(&self) -> bool {
        self.on.is_empty() && self.off.is_empty()
    }
}

/// What an agenda does unless your word holds: left out when you keep it
/// hidden on this device (`kept`, the places Show kept hidden for good),
/// else as the hours admit it (`usual`).
pub fn default_on(agenda: &Agenda, hours: Hours, kept: &BTreeSet<String>) -> bool {
    !kept.contains(&agenda.id) && usual(agenda, hours)
}

/// Whether an agenda is shown now: your word while it holds; else left out
/// when kept hidden on this device; else the hours' (`default_on`).
pub fn shown(agenda: &Agenda, hours: Hours, over: Option<&Override>, kept: &BTreeSet<String>, now: i64) -> bool {
    match over.filter(|o| o.holds(hours, now)) {
        Some(o) if o.on.contains(&agenda.id) => true,
        Some(o) if o.off.contains(&agenda.id) => false,
        _ => default_on(agenda, hours, kept),
    }
}

/// The places of the agendas left out now.
pub fn hidden(agendas: &[Agenda], hours: Hours, over: Option<&Override>, kept: &BTreeSet<String>, now: i64) -> BTreeSet<String> {
    agendas.iter().filter(|a| !shown(a, hours, over, kept, now)).map(|a| a.id.clone()).collect()
}

/// The places of the agendas left out by your word alone, never by the
/// hours: turned off while your word holds, or kept hidden on this device
/// and not turned on. The full lists (every task, whatever the hour) leave
/// out these and nothing else.
pub fn hidden_by_hand(agendas: &[Agenda], hours: Hours, over: Option<&Override>, kept: &BTreeSet<String>, now: i64) -> BTreeSet<String> {
    let over = over.filter(|o| o.holds(hours, now));
    agendas
        .iter()
        .filter(|a| match over {
            Some(o) if o.on.contains(&a.id) => false,
            Some(o) if o.off.contains(&a.id) => true,
            _ => kept.contains(&a.id),
        })
        .map(|a| a.id.clone())
        .collect()
}

/// `ids` turned on or off: the override as it becomes, made now if none
/// holds; an agenda turned back to what it does unless you say
/// (`default_on`) leaves it; none left, none at all (back to the hours).
/// One kept hidden and turned on shows until work's hours change, as any.
pub fn turned(over: Option<&Override>, agendas: &[Agenda], ids: &[String], on: bool, hours: Hours, kept: &BTreeSet<String>, now: i64) -> Option<Override> {
    turned_against(over, agendas, ids, on, hours, now, &|agenda| default_on(agenda, hours, kept))
}

/// `ids` turned on or off from a full list, which the hours never shape
/// (`hidden_by_hand`): your word is what differs from "kept hidden on this
/// device, else shown", so that a work list unticked in the evening there
/// is left out until work's hours change, as any word of yours.
pub fn turned_by_hand(over: Option<&Override>, agendas: &[Agenda], ids: &[String], on: bool, hours: Hours, kept: &BTreeSet<String>, now: i64) -> Option<Override> {
    turned_against(over, agendas, ids, on, hours, now, &|agenda| !kept.contains(&agenda.id))
}

fn turned_against(over: Option<&Override>, agendas: &[Agenda], ids: &[String], on: bool, hours: Hours, now: i64, usual: &dyn Fn(&Agenda) -> bool) -> Option<Override> {
    let mut over = over.filter(|o| o.holds(hours, now)).cloned().unwrap_or(Override { work: hours.work, until: hours.until, ..Override::default() });
    for id in ids {
        over.on.remove(id);
        over.off.remove(id);
        let Some(agenda) = agendas.iter().find(|a| &a.id == id) else { continue };
        if on != usual(agenda) {
            if on { over.on.insert(id.clone()) } else { over.off.insert(id.clone()) };
        }
    }
    (!over.is_empty()).then_some(over)
}

// ---------------------------------------------------------------- the matrix of overlaps

/// Which agendas' events are compared for conflicts (research on several
/// agendas, §5.2). A conflict is one person in two places: by the rule,
/// two events conflict when both need you (`agenda::Occurrence::needs_you`:
/// your agendas' events, the invitations you accepted, and the child events
/// you take), none of an agenda for availability only. The matrix of
/// Agenda ⚙ holds the exceptions: a pair ticked compares all its events,
/// whoever they need (two children whose events share one car); one of your
/// pairs unticked compares none. Unsaid, a box shows the rule: your agendas
/// among themselves and each with itself ticked, every other pair not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Matrix {
    said: BTreeMap<String, bool>,
    /// Whose each agenda is and whether it is for availability only, by place.
    kinds: BTreeMap<String, (Whose, bool)>,
}

impl Matrix {
    pub fn of(config: &Config, agendas: &[Agenda]) -> Matrix {
        // A calendar of busy blocks Sioul keeps counts as availability only: never ticked by the rule.
        Matrix { said: config.agenda.overlaps.clone(), kinds: agendas.iter().map(|a| (a.id.clone(), (a.whose, a.busy() || a.mirror))).collect() }
    }

    /// As the configuration says now (`read_config`).
    pub fn current() -> Matrix {
        let config = read_config();
        Matrix::of(&config, &all(&config))
    }

    /// A pair's key: the two places sorted, "a|b"; an agenda with itself "a|a".
    pub fn key(a: &str, b: &str) -> String {
        if a <= b { format!("{a}|{b}") } else { format!("{b}|{a}") }
    }

    /// What a pair's box shows unless you said otherwise: ticked for your own
    /// agendas among themselves and each with itself (their events always
    /// need you), not for the rest (children's agendas, others', those for
    /// availability only), where the rule still compares what needs you. An
    /// agenda not known here (gone, or an event read without its calendar)
    /// counts as yours.
    pub fn usual(&self, a: &str, b: &str) -> bool {
        let kind = |id: &str| self.kinds.get(id).copied().unwrap_or((Whose::Yours, false));
        let ((whose_a, busy_a), (whose_b, busy_b)) = (kind(a), kind(b));
        whose_a == Whose::Yours && whose_b == Whose::Yours && !busy_a && !busy_b
    }

    /// Whether a pair's box is ticked: your word, else what it shows unsaid.
    pub fn ticked(&self, a: &str, b: &str) -> bool {
        self.said(a, b).unwrap_or_else(|| self.usual(a, b))
    }

    /// What you said of this pair, if anything.
    pub fn said(&self, a: &str, b: &str) -> Option<bool> {
        self.said.get(&Matrix::key(a, b)).copied()
    }

    /// The word to write when a pair's box is turned to `ticked`: none when it
    /// is what the box shows unsaid (back to the rule), else your word.
    pub fn word_for(&self, a: &str, b: &str, ticked: bool) -> Option<bool> {
        (ticked != self.usual(a, b)).then_some(ticked)
    }

    /// Whether two events are compared for a conflict: your word on their
    /// agendas' pair, else the rule (both need you, none for availability only).
    pub fn compares(&self, a: &crate::agenda::Occurrence, b: &crate::agenda::Occurrence) -> bool {
        match self.said(&a.agenda, &b.agenda) {
            Some(word) => word,
            None => !a.busy && !b.busy && a.needs_you() && b.needs_you(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection(account: &str, id: &str, components: &[&str]) -> Collection {
        Collection { kind: Kind::Calendars, account: account.into(), id: id.into(), dir: PathBuf::from(format!("/nowhere/{account}/{id}")), name: id.into(), color: None, read_only: false, components: components.iter().map(|c| c.to_string()).collect() }
    }

    fn config(text: &str) -> Config {
        toml::from_str(text).unwrap()
    }

    /// The defaults from what Sioul can see: an agenda in an account for
    /// work alone is work; one its server keeps read only is someone
    /// else's, for its availability alone when it carries no details;
    /// everything else yours, personal, all day, read and write. What you
    /// say wins, but never more writing than the server allows.
    #[test]
    fn each_agenda_says_what_it_is_by_default() {
        let config = config(
            "[[account]]\nid = \"office\"\nkind = \"dav\"\naddress = \"you@work.example\"\nhost = \"dav.work.example\"\narea = \"work\"\n\
             [[account]]\nid = \"home\"\nkind = \"dav\"\naddress = \"you@home.example\"\nhost = \"dav.home.example\"\n\
             [[account]]\nid = \"home-mail\"\nkind = \"imap\"\naddress = \"you@home.example\"\nhost = \"imap.home.example\"\n\
             [[account]]\nid = \"team\"\nkind = \"dav\"\naddress = \"you@team.example\"\nhost = \"dav.team.example\"\n\
             [[account]]\nid = \"team-mail\"\nkind = \"imap\"\naddress = \"you@team.example\"\nhost = \"imap.team.example\"\narea = \"work\"\n",
        );
        let of = |account: &str, read_only: bool, details: bool, said: &Said| Agenda::of(&collection(account, "cal", &["VEVENT"]), said, &config, read_only, &|| details);
        let work = of("office", false, true, &Said::default());
        assert_eq!((work.whose, work.area, work.access, work.reach()), (Whose::Yours, Area::WORK, Access::Write, Reach::WorkHours), "an account for work alone");
        let team = of("team", false, true, &Said::default());
        assert_eq!(team.area, Area::WORK, "the mail account of the same address says work, explicitly");
        let home = of("home", false, true, &Said::default());
        assert_eq!((home.whose, home.area, home.reaches, home.access, home.reach()), (Whose::Yours, Area::PERSONAL, Reaches::AllDay, Access::Write, Reach::Always), "a mail address left unsaid makes nothing work here");
        let shared = of("home", true, true, &Said::default());
        assert_eq!((shared.whose, shared.access, shared.theirs()), (Whose::Shared, Access::Read, true), "read only, with details");
        let busy = of("home", true, false, &Said::default());
        assert_eq!((busy.whose, busy.access, busy.busy()), (Whose::Shared, Access::Availability, true), "read only, no details: availability only");
        // What you say wins; read and write is never more than the server allows.
        let said = Said { whose: Some("child".into()), name: Some(" Léa ".into()), area: Some("leisure".into()), reaches: Some("outside-work".into()), access: Some("write".into()), ..Said::default() };
        let child = of("home", true, true, &said);
        assert_eq!((child.whose, child.person.as_str(), child.area, child.access, child.reach()), (Whose::Child, "Léa", Area::LEISURE, Access::Read, Reach::OutsideWork));
        let chosen = of("home", false, true, &Said { access: Some("availability".into()), ..Said::default() });
        assert!(chosen.busy() && chosen.read_only());
        // Words this Sioul does not know are unsaid.
        let odd = of("home", false, true, &Said { whose: Some("cousin".into()), access: Some("all".into()), ..Said::default() });
        assert_eq!((odd.whose, odd.access), (Whose::Yours, Access::Write));
        // For work and for you: all day, whatever it says of the hours.
        let both = of("home", false, true, &Said { area: Some("work+admin".into()), reaches: Some("outside-work".into()), ..Said::default() });
        assert_eq!(both.reach(), Reach::Always);
    }

    /// A calendar shared for its availability alone says one word, or
    /// nothing; a place, notes, a guest or two titles are details.
    #[test]
    fn what_carries_details() {
        let event = |summary: &str, more: &str| format!("BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x\r\nSUMMARY:{summary}\r\n{more}END:VEVENT\r\nEND:VCALENDAR\r\n");
        let says = |texts: &[String]| items_say_details(texts.iter().cloned());
        assert!(!says(&[event("Busy", ""), event("Busy", ""), event("busy", "")]), "one word");
        assert!(!says(&[event("", ""), event("", "")]), "nothing");
        assert!(says(&[event("Busy", ""), event("Dentist", "")]), "two titles");
        assert!(says(&[event("Busy", "LOCATION:1 rue de l'Exemple\r\n")]), "a place");
        assert!(says(&[event("Busy", "DESCRIPTION:Bring the forms\r\n")]), "notes");
        assert!(says(&[event("Busy", "ATTENDEE;CN=Sam:mailto:sam@example.org\r\n")]), "a guest");
        assert!(!says(&[event("Busy", "LOCATION:\r\n")]), "an empty place says nothing");
        assert!(says(&[]), "nothing yet: it may say more later");
    }

    fn agenda(id: &str, area: Area, reaches: Reaches) -> Agenda {
        let mut a = Agenda::of(&collection("a", id, &["VEVENT"]), &Said::default(), &Config::default(), false, &|| true);
        a.area = area;
        a.reaches = reaches;
        a
    }

    /// By default the filter shows what the hours admit: a work agenda in
    /// work hours only, a personal one kept outside work hours outside them,
    /// everything when no hours are set. Your word holds until work's hours
    /// change, and "As the hours say" (no override) gives the hours back.
    #[test]
    fn the_filter_follows_the_hours_until_you_say_and_then_until_they_change() {
        let work = agenda("work", Area::WORK, Reaches::AllDay);
        let home = agenda("home", Area::PERSONAL, Reaches::AllDay);
        let evening = agenda("evening", Area::LEISURE, Reaches::OutsideWork);
        let all = [work.clone(), home.clone(), evening.clone()];
        let at_work = Hours { set: true, work: true, until: Some(1_000) };
        let after = Hours { set: true, work: false, until: Some(2_000) };
        let ids = |set: BTreeSet<String>| set.into_iter().collect::<Vec<_>>();
        let none = BTreeSet::new();
        assert_eq!(ids(hidden(&all, at_work, None, &none, 500)), ["a/evening"], "at work: the evening's agenda waits");
        assert_eq!(ids(hidden(&all, after, None, &none, 1_500)), ["a/work"], "after work: the work agenda waits");
        assert!(hidden(&all, Hours::NONE, None, &none, 0).is_empty(), "no hours set: everything");
        // At 18:00 the work agenda shown, by hand: it holds until work's hours change.
        let over = turned(None, &all, &["a/work".to_string()], true, after, &none, 1_500).unwrap();
        assert_eq!((over.work, over.until, ids(over.on.clone())), (false, Some(2_000), vec!["a/work".to_string()]));
        assert!(hidden(&all, after, Some(&over), &none, 1_900).is_empty(), "held");
        let next_day = Hours { set: true, work: true, until: Some(3_000) };
        assert_eq!(ids(hidden(&all, next_day, Some(&over), &none, 2_000)), ["a/evening"], "work's hours changed: the hours decide again");
        // The time it was to change came, whatever the hours say now (an hour moved meanwhile).
        assert_eq!(ids(hidden(&all, after, Some(&over), &none, 2_000)), ["a/work"], "its time came");
        // Turned back to what the hours say: nothing left of it.
        assert_eq!(turned(Some(&over), &all, &["a/work".to_string()], false, after, &none, 1_600), None);
        // A whole group off, one of them back on: only the difference is kept.
        let group = ["a/home".to_string(), "a/evening".to_string()];
        let off = turned(None, &all, &group, false, after, &none, 1_500).unwrap();
        assert_eq!(ids(off.off.clone()), ["a/evening", "a/home"]);
        let back = turned(Some(&off), &all, &["a/home".to_string()], true, after, &none, 1_500).unwrap();
        assert_eq!((ids(back.off.clone()), back.on.is_empty()), (vec!["a/evening".to_string()], true));
        // An override no longer holding is never built upon.
        let fresh = turned(Some(&off), &all, &["a/home".to_string()], false, next_day, &none, 2_100).unwrap();
        assert_eq!((fresh.work, ids(fresh.off)), (true, vec!["a/home".to_string()]));
    }

    /// An agenda kept hidden on this device (what Show hid for good) stays
    /// hidden across every change of the hours; ticked, it shows until work's
    /// hours change, as any word of yours, then it is hidden again; your word
    /// turning it back off leaves nothing of it.
    #[test]
    fn an_agenda_kept_hidden_stays_hidden_across_the_hours() {
        let home = agenda("home", Area::PERSONAL, Reaches::AllDay);
        let club = agenda("club", Area::PERSONAL, Reaches::AllDay);
        let all = [home, club];
        let kept: BTreeSet<String> = ["a/club".to_string()].into();
        let ids = |set: BTreeSet<String>| set.into_iter().collect::<Vec<_>>();
        let at_work = Hours { set: true, work: true, until: Some(1_000) };
        let after = Hours { set: true, work: false, until: Some(2_000) };
        for (hours, now) in [(at_work, 500), (after, 1_500), (Hours::NONE, 0)] {
            assert_eq!(ids(hidden(&all, hours, None, &kept, now)), ["a/club"], "hidden for good: {hours:?}");
        }
        let over = turned(None, &all, &["a/club".to_string()], true, at_work, &kept, 500).unwrap();
        assert!(over.on.contains("a/club") && hidden(&all, at_work, Some(&over), &kept, 900).is_empty(), "ticked: shown");
        assert_eq!(ids(hidden(&all, after, Some(&over), &kept, 1_500)), ["a/club"], "work's hours changed: hidden again");
        assert_eq!(turned(Some(&over), &all, &["a/club".to_string()], false, at_work, &kept, 600), None, "back to kept hidden: no word left");
    }

    /// The full lists follow your word alone, never the hours: an agenda the
    /// hours leave out shows there; one kept hidden on this device or turned
    /// off by hand stays out, until work's hours change for the latter; one
    /// unticked from a full list in the evening is your word, recorded.
    #[test]
    fn the_full_lists_leave_out_what_you_said_and_never_what_the_hours_say() {
        let work = agenda("work", Area::WORK, Reaches::AllDay);
        let home = agenda("home", Area::PERSONAL, Reaches::AllDay);
        let club = agenda("club", Area::PERSONAL, Reaches::AllDay);
        let all = [work, home, club];
        let kept: BTreeSet<String> = ["a/club".to_string()].into();
        let ids = |set: BTreeSet<String>| set.into_iter().collect::<Vec<_>>();
        let after = Hours { set: true, work: false, until: Some(2_000) };
        assert_eq!(ids(hidden(&all, after, None, &kept, 1_500)), ["a/club", "a/work"], "the calm views: the hours and your word");
        assert_eq!(ids(hidden_by_hand(&all, after, None, &kept, 1_500)), ["a/club"], "the full lists: your word alone");
        // Unticked from a full list in the evening: recorded though the hours hide it too.
        let off = turned_by_hand(None, &all, &["a/work".to_string()], false, after, &kept, 1_500).unwrap();
        assert_eq!(ids(hidden_by_hand(&all, after, Some(&off), &kept, 1_600)), ["a/club", "a/work"]);
        let next_day = Hours { set: true, work: true, until: Some(3_000) };
        assert_eq!(ids(hidden_by_hand(&all, next_day, Some(&off), &kept, 2_100)), ["a/club"], "work's hours changed: your word ends");
        // Ticked back there: nothing left of the word; the kept one ticked shows while it holds.
        assert_eq!(turned_by_hand(Some(&off), &all, &["a/work".to_string()], true, after, &kept, 1_700), None);
        let club_on = turned_by_hand(None, &all, &["a/club".to_string()], true, after, &kept, 1_500).unwrap();
        assert!(hidden_by_hand(&all, after, Some(&club_on), &kept, 1_600).is_empty());
    }

    /// Work's hours as the agendas follow them, from the week's hours: on
    /// from 9:00 to 17:00, a meal inside them still work's day, their next
    /// change found past admin's hours; time off and Free time are off.
    #[test]
    fn work_hours_and_when_they_change() {
        let zone = jiff::tz::TimeZone::get("Europe/Paris").unwrap();
        let config = config(
            "[[window]]\nday = \"mon\"\nstart = \"09:00\"\nend = \"17:00\"\n\
             [[window]]\nday = \"mon\"\nkind = \"admin\"\nstart = \"18:00\"\nend = \"19:00\"\n\
             [[window]]\nday = \"tue\"\nstart = \"09:00\"\nend = \"17:00\"\n",
        );
        let at = |text: &str| text.parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap();
        let stamp = |text: &str| at(text).timestamp().as_second();
        let none = crate::quiet::Overrides::default();
        let monday = Hours::at(&config, &none, &at("2026-10-05T10:00"));
        assert_eq!(monday, Hours { set: true, work: true, until: Some(stamp("2026-10-05T17:00")) });
        let evening = Hours::at(&config, &none, &at("2026-10-05T17:30"));
        assert_eq!(evening, Hours { set: true, work: false, until: Some(stamp("2026-10-06T09:00")) }, "admin's hour between is no change of work's");
        assert!(!Hours::at(&Config::default(), &none, &at("2026-10-05T10:00")).set, "no hours set");
        // Free time, taken at 10:00: not work's, until it ends (here, at midnight); back from it, work's again.
        let free = crate::quiet::Overrides { free_since: Some(stamp("2026-10-05T10:00")), ..crate::quiet::Overrides::default() };
        let taken = Hours::at(&config, &free, &at("2026-10-05T11:00"));
        assert!(taken.set && !taken.work, "{taken:?}");
        let back = crate::quiet::Overrides { free_ended: Some(stamp("2026-10-05T11:30")), ..free.clone() };
        assert!(Hours::at(&config, &back, &at("2026-10-05T11:45")).work);
        // A day off: not work's.
        let mut off = config.clone();
        off.time_off = toml::from_str::<Config>("[[time_off]]\nfrom = \"2026-10-06\"\nuntil = \"2026-10-06\"\n").unwrap().time_off;
        assert!(!Hours::at(&off, &none, &at("2026-10-06T10:00")).work);
        // A meal inside work hours is still work's day: Hours reads no meals.
        assert!(Hours::at(&config, &none, &at("2026-10-05T12:30")).work);
    }

    /// The matrix's boxes as they show unsaid: your own agendas among
    /// themselves and each with itself ticked; a child's with yours, two
    /// children's, availability only (yours too) and everything else not.
    /// Your word on a pair wins, in either order, and turning a box back to
    /// what it shows unsaid takes your word out.
    #[test]
    fn the_matrix_s_boxes_and_your_word() {
        let mine = |id: &str| agenda(id, Area::PERSONAL, Reaches::AllDay);
        let with = |id: &str, whose: Whose, access: Access| Agenda { whose, access, ..mine(id) };
        let agendas = [mine("home"), mine("work"), with("lea", Whose::Child, Access::Write), with("tom", Whose::Child, Access::Write), with("sam", Whose::Parent, Access::Availability), with("alex", Whose::Partner, Access::Read), with("busy", Whose::Yours, Access::Availability)];
        let matrix = Matrix::of(&Config::default(), &agendas);
        assert!(matrix.ticked("a/home", "a/work") && matrix.ticked("a/work", "a/home") && matrix.ticked("a/home", "a/home"), "yours among themselves, and with itself");
        for (a, b) in [("a/lea", "a/home"), ("a/lea", "a/tom"), ("a/lea", "a/lea"), ("a/sam", "a/home"), ("a/sam", "a/sam"), ("a/busy", "a/home"), ("a/alex", "a/home")] {
            assert!(!matrix.ticked(a, b), "{a} with {b}");
        }
        assert!(matrix.ticked("", "a/home"), "an agenda not known here counts as yours");
        let config = config("[agenda.overlaps]\n\"a/lea|a/tom\" = true\n\"a/home|a/home\" = false\n");
        let said = Matrix::of(&config, &agendas);
        assert!(said.ticked("a/tom", "a/lea") && !said.ticked("a/home", "a/home"));
        assert_eq!((said.said("a/tom", "a/lea"), said.said("a/home", "a/work")), (Some(true), None));
        assert_eq!((said.word_for("a/lea", "a/tom", false), said.word_for("a/lea", "a/tom", true), said.word_for("a/home", "a/work", false)), (None, Some(true), Some(false)));
        assert_eq!(Matrix::key("b/x", "a/y"), "a/y|b/x");
    }

    /// An agenda for its availability only gives its busy times alone,
    /// whatever its server sent: its name for a title, no place, notes,
    /// guests or alarms, its free times left out. One you keep read only,
    /// or for its availability, is never written by any writer, its events
    /// and its tasks alike (`agenda::guard`), and every view sees it read
    /// only; your word taken back, it is written again.
    #[test]
    fn read_only_agendas_are_never_written_and_availability_says_nothing_more() {
        crate::config::stand_ins_only();
        let zone = jiff::tz::TimeZone::get("Europe/Paris").unwrap();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Sam's busy times", None, &["VEVENT"]).unwrap();
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "Read-only errands", None, &["VTODO"]).unwrap();
        let event = calendar.dir.join("dentist.ics");
        let full = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:dentist\r\nDTSTART:20991104T140000Z\r\nDTEND:20991104T150000Z\r\nSUMMARY:Dentist for Tom\r\nLOCATION:1 rue de l'Exemple\r\nDESCRIPTION:Bring the health card\r\nORGANIZER;CN=Sam:mailto:sam@example.org\r\nATTENDEE;CN=Camille:mailto:camille@example.net\r\nX-SIOUL-BEFORE:20\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT30M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        vdir::store_item(&event, full).unwrap();
        let free = calendar.dir.join("holiday.ics");
        vdir::store_item(&free, "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:holiday\r\nDTSTART:20991105T080000Z\r\nDTEND:20991105T170000Z\r\nSUMMARY:Working from home\r\nTRANSP:TRANSPARENT\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let task = list.dir.join("coat.ics");
        let task_text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:coat\r\nSUMMARY:Buy a winter coat\r\nSTATUS:NEEDS-ACTION\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        vdir::store_item(&task, task_text).unwrap();
        // One of no meeting with others, to write (a guest makes the dentist's one: never written).
        let plain = calendar.dir.join("swim.ics");
        let plain_text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:swim\r\nDTSTART:20991106T170000Z\r\nDTEND:20991106T180000Z\r\nSUMMARY:Swimming\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        vdir::store_item(&plain, plain_text).unwrap();
        let read = |c: &Collection| crate::agenda::file_occurrences(&event, c, 4_000_000_000, 4_200_000_000, &zone);
        let mine = read(&calendar);
        assert_eq!((mine.len(), mine[0].summary.as_str(), mine[0].busy, mine[0].agenda.clone()), (1, "Dentist for Tom", false, place(&calendar)), "yours, as it is");
        assert!(vdir::write_item(&plain, plain_text).is_ok() && vdir::write_item(&task, task_text).is_ok(), "written while yours");
        // Sam's agenda, for availability only; the list, read only.
        let config = crate::config::default_path();
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        let had = std::fs::read_to_string(&config).ok();
        set_field(&config, &place(&calendar), "access", "availability").unwrap();
        set_field(&config, &place(&calendar), "whose", "parent").unwrap();
        set_field(&config, &place(&list), "access", "read").unwrap();
        let shown = vdir::collections(Kind::Calendars);
        // By account and folder: other tests make calendars of their own meanwhile, some alike in name.
        let now = |c: &Collection| shown.iter().find(|s| s.account == c.account && s.id == c.id).cloned().unwrap();
        assert!(now(&calendar).read_only && now(&list).read_only, "every view sees them read only");
        let busy = read(&now(&calendar));
        assert_eq!(busy.len(), 1);
        let b = &busy[0];
        assert_eq!((b.summary.as_str(), b.location.as_str(), b.notes.as_str(), b.organizer.as_str()), ("Sam's busy times", "", "", ""), "its name, nothing more");
        assert!(b.attendees.is_empty() && b.alarms.is_empty() && b.margins == crate::demands::Margins::default() && b.busy && b.theirs && b.read_only && !b.holds_time());
        assert!(crate::agenda::file_occurrences(&free, &now(&calendar), 4_000_000_000, 4_200_000_000, &zone).is_empty(), "its free time left out");
        // Its switch (part 4): what its events say shown, its title alone; off again, its name.
        set_field(&config, &place(&calendar), "titles", "shown").unwrap();
        let said = read(&now(&calendar));
        assert_eq!((said[0].summary.as_str(), said[0].location.as_str(), said[0].busy), ("Dentist for Tom", "", true), "its title, nothing more");
        assert!(set_field(&config, &place(&calendar), "titles", "everything").is_err());
        set_field(&config, &place(&calendar), "titles", "").unwrap();
        assert_eq!(read(&now(&calendar))[0].summary, "Sam's busy times");
        // Never given to an agent (part 4, recommendation 14): kept from the MCP whatever its projects.
        let refs: Vec<crate::agenda::EventRef> = [(&event, full), (&plain, plain_text)].iter().filter_map(|(p, t)| crate::agenda::event_ref(t, &p.display().to_string(), &zone)).collect();
        let mut consent = crate::consent::Consent::new(&[], true);
        assert!(consent.allows(&crate::links::event_uri("dentist")));
        consent.keep_private(&refs);
        assert!(!consent.allows(&crate::links::event_uri("dentist")) && !consent.allows(&crate::links::event_uri("swim")), "its events kept from agents");
        assert!(consent.allows(&crate::links::event_uri("elsewhere")));
        for (path, text) in [(&plain, plain_text), (&task, task_text)] {
            assert!(vdir::write_item(path, text).is_err(), "{}: never written", path.display());
            assert!(vdir::remove_item(path).is_err(), "{}: never taken out", path.display());
        }
        assert!(vdir::write_item(&list.dir.join("new.ics"), task_text).is_err(), "nothing new in it either");
        assert!(vdir::write_item(&calendar.dir.join("new.ics"), plain_text).is_err());
        // Your word taken back: written again.
        set_field(&config, &place(&calendar), "access", "").unwrap();
        set_field(&config, &place(&list), "access", "").unwrap();
        assert!(vdir::write_item(&plain, plain_text).is_ok() && vdir::write_item(&task, task_text).is_ok());
        match had {
            Some(text) => std::fs::write(&config, text).unwrap(),
            None => std::fs::remove_file(&config).unwrap(),
        }
        let _ = std::fs::remove_dir_all(&calendar.dir);
        let _ = std::fs::remove_dir_all(&list.dir);
    }

    /// Named sets (part 4): the one Sioul's own time brings, the first by
    /// name, both hours at once bringing either's; one by hand alone none;
    /// written in place with the times kept to Sioul's own (never a place),
    /// its comments kept, and taken out.
    #[test]
    fn a_set_comes_by_hand_or_with_its_times() {
        let set = |agendas: &[&str], times: &[&str]| Set { agendas: agendas.iter().map(|a| a.to_string()).collect(), times: times.iter().map(|t| t.to_string()).collect() };
        let sets: BTreeMap<String, Set> = [("My week".to_string(), set(&["a/work"], &["work"])), ("Tom and Léa".to_string(), set(&["a/tom", "a/lea"], &["leisure"])), ("By hand".to_string(), set(&["a/home"], &[]))].into();
        let name = |time: &str| set_of_time(&sets, time).map(|(n, _)| n.clone());
        assert_eq!((name("leisure"), name("work+admin"), name("admin"), name("any")), (Some("Tom and Léa".into()), Some("My week".into()), None, None));
        let dir = std::env::temp_dir().join(format!("sioul-sets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\n[agenda]\nday_start = 8\n").unwrap();
        set_set(&path, "Tom and Léa", Some(&set(&["a/tom"], &["leisure", "the beach"]))).unwrap();
        let read = || toml::from_str::<Config>(&std::fs::read_to_string(&path).unwrap()).unwrap().agenda.sets;
        assert_eq!(read().get("Tom and Léa"), Some(&set(&["a/tom"], &["leisure"])), "a place is no time of Sioul's");
        assert!(std::fs::read_to_string(&path).unwrap().contains("# mine"));
        set_set(&path, "Tom and Léa", None).unwrap();
        assert!(read().is_empty());
        assert!(set_set(&path, " ", Some(&set(&[], &[]))).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Conflicts follow people (research §5.2): two of your events at once
    /// conflict, whatever their agendas; a child's event nobody takes is
    /// nobody's conflict, with yours or with another child's, until you tick
    /// that pair (one car for both); an invitation you accepted in another's
    /// agenda needs you; availability only conflicts with nothing. What
    /// counts as at the same time is the same as before.
    #[test]
    fn conflicts_follow_people_and_the_matrix_holds_the_exceptions() {
        use crate::agenda::Occurrence;
        let at = |uid: &str, agenda: &str, start: i64| Occurrence { uid: uid.into(), key: format!("{uid}.ics"), summary: uid.into(), start, end: start + 3_600, agenda: agenda.into(), ..Occurrence::default() };
        let theirs = |o: Occurrence| Occurrence { theirs: true, ..o };
        let events = [
            at("meeting", "a/work", 36_000),
            at("call", "a/home", 37_800),
            theirs(at("dentist", "a/lea", 36_000)),
            theirs(at("swim", "a/tom", 37_000)),
            Occurrence { invited: true, accepted: true, ..theirs(at("school-meeting", "a/shared", 38_000)) },
            Occurrence { invited: true, ..at("unanswered", "a/home", 38_500) },
            Occurrence { busy: true, ..theirs(at("sam-busy", "a/sam", 36_500)) },
        ];
        let mine = |id: &str| agenda(id, Area::PERSONAL, Reaches::AllDay);
        let agendas = [mine("work"), mine("home"), Agenda { whose: Whose::Child, ..mine("lea") }, Agenda { whose: Whose::Child, ..mine("tom") }, Agenda { whose: Whose::Shared, ..mine("shared") }, Agenda { whose: Whose::Parent, access: Access::Availability, ..mine("sam") }];
        let pairs = |matrix: &Matrix| crate::overlaps::conflicts(&events, 0, matrix).iter().map(|o| format!("{}+{}", o.first.uid, o.second.uid)).collect::<Vec<_>>();
        assert_eq!(pairs(&Matrix::of(&Config::default(), &agendas)), ["meeting+call", "meeting+school-meeting", "call+school-meeting"], "yours, and the invitation you accepted");
        let ticked = config("[agenda.overlaps]\n\"a/lea|a/tom\" = true\n\"a/home|a/work\" = false\n");
        assert_eq!(pairs(&Matrix::of(&ticked, &agendas)), ["dentist+swim", "meeting+school-meeting", "call+school-meeting"], "one car for both children; work and home never compared");
    }

    /// What you say is written into the configuration in place, one field
    /// at a time, its comments kept, and read back; "" takes a field out,
    /// and the last one its table; a word that does not read is refused.
    #[test]
    fn an_agenda_s_settings_are_written_in_place() {
        let dir = std::env::temp_dir().join(format!("sioul-agendas-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\n[agenda]\nbetween = 10\n").unwrap();
        set_field(&path, "murena/lea-ecole", "whose", "child").unwrap();
        set_field(&path, "murena/lea-ecole", "name", "Léa").unwrap();
        set_field(&path, "murena/lea-ecole", "access", "availability").unwrap();
        set_warns(&path, "murena/lea-ecole", "local/home", Some(true)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# mine") && text.contains("between = 10"), "{text}");
        let read = Config::load(&path).unwrap();
        let said = &read.agenda.calendars["murena/lea-ecole"];
        assert_eq!((said.whose.as_deref(), said.name.as_deref(), said.access.as_deref()), (Some("child"), Some("Léa"), Some("availability")));
        assert_eq!(read.agenda.overlaps.get("local/home|murena/lea-ecole"), Some(&true));
        assert!(set_field(&path, "murena/lea-ecole", "whose", "cousin").is_err());
        assert!(set_field(&path, "murena/lea-ecole", "colour", "red").is_err());
        for field in ["whose", "name", "access"] {
            set_field(&path, "murena/lea-ecole", field, "").unwrap();
        }
        set_warns(&path, "murena/lea-ecole", "local/home", None).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("lea-ecole") && !text.contains("overlaps"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The availabilities are written one by one, by their ids: an
    /// availability, its mirror made then marked transparent, its share said
    /// made with its day, a second one; what a newer Sioul wrote in them kept
    /// through every write; a list written inline by hand read and written
    /// as tables; each taken out, what it was given back for Undo, the list
    /// gone with the last; the settings' format marked; an unknown id said.
    #[test]
    fn availabilities_are_written_one_by_one_and_keep_what_this_sioul_does_not_know() {
        use crate::availability::{Audience, Availability, Mirror, Share};
        let dir = std::env::temp_dir().join(format!("sioul-availabilities-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "# mine\nlanguage = \"fr\"\n\n[agenda]\nbetween = 5\n").unwrap();
        let read = || Config::load(&path).unwrap();
        let share = |way: &str, who: &str| Share { audience: if who.is_empty() { "link".into() } else { "person".into() }, who: who.into(), way: way.into(), account: "cloud".into(), ..Share::default() };
        let mut sam = Availability::new("For Sam", vec!["cloud/personal".into(), "work/main".into()], Audience::Person);
        sam.words.insert("work/main".into(), "Work".into());
        sam.shares.push(share("mirror-share", "sam"));
        put_availability(&path, &sam).unwrap();
        let config = read();
        assert_eq!((config.format, config.agenda.availabilities.clone()), (Some(crate::config::FORMAT), vec![sam.clone()]));
        assert_eq!(config.agenda.between, Some(5));
        put_mirror(&path, &sam.id, &Mirror { account: "cloud".into(), calendar: "busy-7f3c".into(), transparent: false }).unwrap();
        put_mirror(&path, &sam.id, &Mirror { account: "cloud".into(), calendar: "busy-7f3c".into(), transparent: true }).unwrap();
        put_share(&path, &sam.id, &Share { made: "2026-11-05".into(), ..share("mirror-share", "Sam") }).unwrap();
        let back = read().agenda.availabilities[0].clone();
        assert_eq!(back.mirrors, [Mirror { account: "cloud".into(), calendar: "busy-7f3c".into(), transparent: true }], "one mirror per account");
        assert_eq!((back.shares.len(), back.shares[0].made.as_str()), (1, "2026-11-05"), "the same share, said made");
        let public = Availability { shares: vec![share("mirror-link", "")], ..Availability::new("Public", vec!["cloud/personal".into()], Audience::Link) };
        let ((), said) = crate::changes::heard(|| put_availability(&path, &public).unwrap());
        assert_eq!(said, [crate::changes::Kind::Agendas], "every view of them told");
        let both = read().agenda.availabilities;
        assert_eq!((both.len(), both[1].rounding, both[1].horizon), (2, 15, 14), "a link: the quarter hour, two weeks");
        // A newer Sioul's fields, in the availability and in its share: kept through a write.
        let mut doc = crate::config::read_document(&path).unwrap();
        let first = availabilities_in(&mut doc, false).unwrap().unwrap().get_mut(0).unwrap();
        first.insert("colour", toml_edit::value("teal"));
        first["shares"].as_array_of_tables_mut().unwrap().get_mut(0).unwrap().insert("pigeon", toml_edit::value(true));
        crate::config::write_document(&path, &doc).unwrap();
        put_availability(&path, &Availability { name: "Sam".into(), ..back.clone() }).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("colour = \"teal\"") && text.contains("pigeon = true") && text.contains("# mine"), "{text}");
        assert_eq!(read().agenda.availabilities[0].name, "Sam");
        // Taken out one by one, each given back.
        assert_eq!(remove_share(&path, &sam.id, &share("mirror-share", "sam")).unwrap().map(|s| s.made), Some("2026-11-05".to_string()));
        assert_eq!(remove_mirror(&path, &sam.id, "cloud").unwrap().map(|m| m.calendar), Some("busy-7f3c".to_string()));
        assert!(read().agenda.availabilities[0].shares.is_empty() && read().agenda.availabilities[0].mirrors.is_empty());
        assert_eq!(remove_availability(&path, &public.id).unwrap().map(|a| a.name), Some("Public".to_string()));
        assert_eq!(remove_availability(&path, &public.id).unwrap(), None, "not there: nothing");
        assert!(put_share(&path, "no-such-id", &share("mirror-link", "")).is_err());
        assert_eq!(remove_availability(&path, &sam.id).unwrap().map(|a| a.id), Some(sam.id.clone()));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("availabilities") && text.contains("between = 5") && text.contains("format = 2"), "{text}");
        // Written inline, by hand: read, then written as tables.
        std::fs::write(&path, "[agenda]\navailabilities = [{ id = \"x1\", name = \"Inline\", agendas = [\"cloud/personal\"] }]\n").unwrap();
        assert_eq!(read().agenda.availabilities[0].name, "Inline");
        put_share(&path, "x1", &share("mirror-link", "")).unwrap();
        let config = read();
        assert_eq!((config.agenda.availabilities.len(), config.agenda.availabilities[0].shares.len(), config.agenda.availabilities[0].name.as_str()), (1, 1, "Inline"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("[[agenda.availabilities.shares]]"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A calendar of busy blocks Sioul keeps, a mirror of the list or the
    /// calendar of the single share of before, is marked so, and the matrix
    /// never ticks it by the rule: derived from your own events, it is no
    /// agenda of yours to compare.
    #[test]
    fn a_mirror_is_marked_and_never_compared_by_the_rule() {
        let mut config = Config::default();
        config.agenda.availabilities.push(crate::availability::Availability { id: "a1".into(), mirrors: vec![crate::availability::Mirror { account: "cloud".into(), calendar: "busy-7f3c".into(), transparent: true }], ..Default::default() });
        config.agenda.availability = Some(crate::availability::Single { account: "cloud".into(), calendar: "busy-old".into(), ..Default::default() });
        let of = |id: &str| Agenda::of(&collection("cloud", id, &["VEVENT"]), &Said::default(), &config, false, &|| true);
        let (mirror, old, personal) = (of("busy-7f3c"), of("busy-old"), of("personal"));
        assert!(mirror.mirror && old.mirror && !personal.mirror);
        let matrix = Matrix::of(&config, &[mirror.clone(), personal.clone()]);
        assert!(matrix.usual(&personal.id, &personal.id) && !matrix.usual(&mirror.id, &mirror.id) && !matrix.usual(&mirror.id, &personal.id));
    }
}
