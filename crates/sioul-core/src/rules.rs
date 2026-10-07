// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Mail filters: what a message shows, and what is then done to it on its
//! server (docs/client.md, "Filters").
//!
//! A filter holds conditions, each a field, a test and a value ("From
//! contains @bank.example"), all of them needed or one enough; and actions:
//! into a folder, to the archive, to spam, flagged, read, to the trash, a keyword. One flat
//! list serves every account, each filter on all of them or on some, in the
//! configuration (`[[mail.filter]]`), shared with it. Filters run on what
//! arrives unread in an inbox, on whichever device fetches it first
//! (sioul-sync's `filters`), and on the rest of the inbox when you ask. The
//! Mail page's search reads the same conditions (`Condition::matches` on a
//! `Message`).
//!
//! Text is compared as Sioul's search compares it: case and accents aside
//! (`text::fold_char`), each run of spaces one space. A sender nothing
//! authenticates is read as nobody's: a condition on who sent it holds for no
//! such message, as a project's route does not (docs/porch.md, "Trust").
//!
//! At the end, Virtual Secretary's words (docs/virtual-secretary.md): the
//! folder it moved a message to becomes a proposal here (`FolderMapping`).

use crate::card::Card;
use crate::folders::Role;
use crate::headers::RawHeaders;
use crate::i18n::Translator;
use crate::reach::Who;
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;

// ---------------------------------------------------------------- conditions

/// What a condition reads of a message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Field {
    /// Who sent it: their name and address.
    From,
    /// To whom: names and addresses.
    To,
    /// Who was copied.
    Cc,
    /// Where answers go, when the message names somewhere.
    ReplyTo,
    Subject,
    /// Its text, an HTML message read as text.
    Body,
    /// Its attachments: whether it has one, their names.
    Attachment,
    /// The kind of file attached (`KINDS`).
    AttachmentType,
    /// Who its sender is to you (`WHO`; docs/porch.md, "Who may reach you").
    Sender,
    /// A newsletter or a mailing list, and which one (its List-Id).
    List,
    /// The day it arrived.
    Date,
    /// The day of the week it arrived.
    Weekday,
    /// The time of day it arrived.
    Hour,
    /// Its size.
    Size,
    /// Its sender, recipients, subject or text, any of them: the Mail page's
    /// search starts with it, and a filter made from a search keeps it.
    Anywhere,
    /// The search only: the address it belongs to here, by its account's id
    /// (a filter says that with its `accounts`). Needs `Message::placed`.
    Account,
    /// The search only: its folder, by purpose (`ROLES`: "inbox", "archive"…)
    /// or by its server's name. Needs `Message::placed`.
    Folder,
    /// The search only: read, flagged or answered (`MARKS`). Needs `Message::placed`.
    Mark,
    /// A field this Sioul does not know (a newer one wrote it, or a hand): never holds.
    #[default]
    #[serde(other)]
    Unknown,
}

impl Field {
    /// Every field, in the order the editor offers them.
    pub const ALL: [Field; 15] = [
        Field::From,
        Field::To,
        Field::Cc,
        Field::ReplyTo,
        Field::Subject,
        Field::Body,
        Field::Anywhere,
        Field::Attachment,
        Field::AttachmentType,
        Field::Sender,
        Field::List,
        Field::Date,
        Field::Weekday,
        Field::Hour,
        Field::Size,
    ];

    /// The fields the Mail page's search offers, in its order: the filters'
    /// that say something of mail already here, Anywhere first, and its own
    /// three (address, folder, marks), which no filter reads.
    pub const SEARCH: [Field; 15] = [
        Field::Anywhere,
        Field::From,
        Field::To,
        Field::Cc,
        Field::Subject,
        Field::Body,
        Field::Attachment,
        Field::AttachmentType,
        Field::Date,
        Field::Size,
        Field::Sender,
        Field::List,
        Field::Account,
        Field::Folder,
        Field::Mark,
    ];

    /// As the configuration writes it: "from", "reply-to".
    pub fn id(self) -> &'static str {
        match self {
            Field::From => "from",
            Field::To => "to",
            Field::Cc => "cc",
            Field::ReplyTo => "reply-to",
            Field::Subject => "subject",
            Field::Body => "body",
            Field::Attachment => "attachment",
            Field::AttachmentType => "attachment-type",
            Field::Sender => "sender",
            Field::List => "list",
            Field::Date => "date",
            Field::Weekday => "weekday",
            Field::Hour => "hour",
            Field::Size => "size",
            Field::Anywhere => "anywhere",
            Field::Account => "account",
            Field::Folder => "folder",
            Field::Mark => "mark",
            Field::Unknown => "unknown",
        }
    }

    pub fn parse(id: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| f.id() == id.trim())
    }

    /// The tests it takes, the usual one first.
    pub fn tests(self) -> &'static [Test] {
        match self {
            Field::From | Field::To | Field::Cc | Field::ReplyTo | Field::Subject => &[Test::Contains, Test::NotContains, Test::Is, Test::IsNot],
            Field::Body => &[Test::Contains, Test::NotContains],
            Field::Attachment | Field::List => &[Test::Exists, Test::Missing, Test::Contains, Test::NotContains],
            Field::AttachmentType | Field::Sender | Field::Weekday => &[Test::Is, Test::IsNot],
            Field::Date | Field::Hour => &[Test::After, Test::Before, Test::Between],
            Field::Size => &[Test::Above, Test::Below],
            Field::Anywhere => &[Test::Contains, Test::NotContains],
            // A search's place and marks: an address's id, a folder, one of `MARKS`, all as text.
            Field::Account | Field::Folder | Field::Mark => &[Test::Is, Test::IsNot],
            Field::Unknown => &[],
        }
    }

    /// What its value is, under `test`.
    pub fn value(self, test: Test) -> Value {
        match (self, test) {
            (_, Test::Exists | Test::Missing) => Value::Nothing,
            (Field::Date, Test::Between) => Value::Dates,
            (Field::Date, _) => Value::Date,
            (Field::Hour, Test::Between) => Value::Times,
            (Field::Hour, _) => Value::Time,
            (Field::Weekday, _) => Value::Days,
            (Field::Size, _) => Value::Size,
            (Field::AttachmentType, _) => Value::Kind,
            (Field::Sender, _) => Value::Who,
            _ => Value::Text,
        }
    }
}

/// How a condition compares its field with its value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Test {
    Contains,
    NotContains,
    /// The whole of it: an address or a name, a subject, a list.
    Is,
    IsNot,
    /// Before a day (that day left out), before a time.
    Before,
    /// After a day (that day left out); from a time on.
    After,
    /// From a day to another, both included; from a time to another, across midnight too (22:00 to 06:00).
    Between,
    /// Larger than.
    Above,
    /// Smaller than.
    Below,
    /// There is one: an attachment; a newsletter or a list.
    Exists,
    /// There is none.
    Missing,
    /// A test this Sioul does not know: never holds.
    #[default]
    #[serde(other)]
    Unknown,
}

impl Test {
    pub const ALL: [Test; 11] = [Test::Contains, Test::NotContains, Test::Is, Test::IsNot, Test::Before, Test::After, Test::Between, Test::Above, Test::Below, Test::Exists, Test::Missing];

    /// As the configuration writes it: "contains", "not-contains".
    pub fn id(self) -> &'static str {
        match self {
            Test::Contains => "contains",
            Test::NotContains => "not-contains",
            Test::Is => "is",
            Test::IsNot => "is-not",
            Test::Before => "before",
            Test::After => "after",
            Test::Between => "between",
            Test::Above => "above",
            Test::Below => "below",
            Test::Exists => "exists",
            Test::Missing => "missing",
            Test::Unknown => "unknown",
        }
    }

    pub fn parse(id: &str) -> Option<Test> {
        Test::ALL.into_iter().find(|t| t.id() == id.trim())
    }

    /// The test a negative one turns around: "does not contain" is "contains", turned.
    pub fn turned(self) -> Option<Test> {
        match self {
            Test::NotContains => Some(Test::Contains),
            Test::IsNot => Some(Test::Is),
            Test::Missing => Some(Test::Exists),
            _ => None,
        }
    }
}

/// What a condition's value is, for the editor to ask it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Value {
    /// None: "has an attachment".
    Nothing,
    Text,
    /// A day: "2026-10-01".
    Date,
    /// Two days, `value` and `until`.
    Dates,
    /// A time of day: "18:00".
    Time,
    /// Two times, `value` and `until`.
    Times,
    /// Days of the week, `DAYS` joined by commas: "sat,sun".
    Days,
    /// A size: "5 MB", "500 KB".
    Size,
    /// A kind of attachment, one of `KINDS`.
    Kind,
    /// Who the sender is to you, one of `WHO`.
    Who,
}

/// The kinds of attachment a condition names.
pub const KINDS: [&str; 8] = ["pdf", "image", "document", "archive", "calendar", "audio", "video", "text"];

/// Who a sender may be to you, as a condition names it: "known" is anyone in
/// your address books or on a list (safe, neutral or restricted). The blocked
/// are not among them: their mail is set aside, and no filter touches it.
pub const WHO: [&str; 5] = ["known", "safe", "neutral", "restricted", "stranger"];

/// The days of the week as a condition names them, Monday first.
pub const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// The marks a search's condition names (`Field::Mark`): "is not read" is unread.
pub const MARKS: [&str; 3] = ["read", "flagged", "answered"];

/// The folders a search's condition names by purpose (`Field::Folder`),
/// whatever their name on each server; Gmail's "All Mail" is its archive.
pub const ROLES: [(&str, Role); 6] = [("inbox", Role::Inbox), ("sent", Role::Sent), ("drafts", Role::Drafts), ("archive", Role::Archive), ("junk", Role::Junk), ("trash", Role::Trash)];

/// Whether a folder condition's value (a purpose of `ROLES`, or a server's
/// own name) names a folder of `role` and `name`.
pub fn folder_named(value: &str, role: Role, name: &str) -> bool {
    let value = value.trim();
    match ROLES.iter().find(|(id, _)| id.eq_ignore_ascii_case(value)) {
        Some((_, Role::Archive)) => matches!(role, Role::Archive | Role::All),
        Some((_, wanted)) => *wanted == role,
        None => !name.is_empty() && fold(value) == fold(name),
    }
}

/// Where a message is and its marks, for the search's own conditions
/// (`Field::Account`, `Field::Folder`, `Field::Mark`): a filter has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// Its account's id.
    pub account: String,
    /// Its folder's server name.
    pub folder: String,
    pub role: Role,
    /// Its Maildir letters: S read, F flagged, R answered.
    pub flags: String,
}

/// One condition: a field, a test, a value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    #[serde(default)]
    pub field: Field,
    #[serde(default)]
    pub test: Test,
    /// Text, a day ("2026-10-01"), a time ("18:00"), days ("sat,sun"), a size
    /// ("5 MB"), a kind ("pdf"), who ("stranger"); empty for "has an attachment".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    /// The end of a range ("between"): a day or a time.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub until: String,
}

impl Condition {
    pub fn new(field: Field, test: Test, value: &str) -> Condition {
        Condition { field, test, value: value.to_string(), until: String::new() }
    }

    /// What keeps it from being read, as a message id ("filter-problem-value");
    /// none when it reads. One that does not read never holds.
    pub fn problem(&self) -> Option<&'static str> {
        if !self.field.tests().contains(&self.test) {
            return Some("filter-problem-unknown");
        }
        let kind = self.field.value(self.test);
        let fine = match kind {
            Value::Nothing => true,
            Value::Text => !fold(&self.value).is_empty(),
            Value::Date => day(&self.value).is_some(),
            Value::Dates => matches!((day(&self.value), day(&self.until)), (Some(from), Some(to)) if from <= to),
            Value::Time => time(&self.value).is_some(),
            Value::Times => time(&self.value).is_some() && time(&self.until).is_some(),
            Value::Days => !days(&self.value).is_empty(),
            Value::Size => size(&self.value).is_some(),
            Value::Kind => KINDS.contains(&self.value.trim()),
            Value::Who => WHO.contains(&self.value.trim()),
        };
        (!fine).then_some(match kind {
            Value::Date | Value::Dates => "filter-problem-date",
            Value::Time | Value::Times => "filter-problem-time",
            Value::Size => "filter-problem-size",
            _ => "filter-problem-value",
        })
    }

    /// Whether `message` shows it. One that does not read, or that the
    /// message cannot answer (its size unknown, its sender not judged), never holds.
    pub fn matches(&self, message: &Message) -> bool {
        self.problem().is_none() && self.holds(message).unwrap_or(false)
    }

    /// Whether it holds; none when the message cannot tell.
    fn holds(&self, m: &Message) -> Option<bool> {
        if let Some(positive) = self.test.turned() {
            return Condition { test: positive, ..self.clone() }.holds(m).map(|held| !held);
        }
        let value = fold(&self.value);
        match self.field {
            Field::From | Field::To | Field::Cc | Field::ReplyTo => {
                let people = m.people(self.field);
                Some(match self.test {
                    Test::Contains => people.iter().any(|p| p.both.contains(&value)),
                    _ => people.iter().any(|p| p.address == value || (!p.name.is_empty() && p.name == value)),
                })
            }
            Field::Subject => Some(if self.test == Test::Contains { m.subject().contains(&value) } else { *m.subject() == value }),
            Field::Body => Some(m.text_contains(&value)),
            Field::Attachment => Some(match self.test {
                Test::Exists => !m.card.attachments.is_empty(),
                _ => m.names().iter().any(|name| name.contains(&value)),
            }),
            Field::AttachmentType => Some(m.card.shape.parts.iter().any(|(name, mime)| attachment_kind(name, mime) == self.value.trim())),
            Field::Sender => m.who().map(|who| who_is(who, self.value.trim())),
            Field::List => Some(match self.test {
                Test::Exists => m.card.is_list,
                _ => m.list().contains(&value),
            }),
            Field::Date => {
                let arrived = m.arrived()?.date();
                Some(match self.test {
                    Test::Before => arrived < day(&self.value)?,
                    Test::After => arrived > day(&self.value)?,
                    _ => day(&self.value)? <= arrived && arrived <= day(&self.until)?,
                })
            }
            Field::Weekday => {
                let arrived = m.arrived()?.weekday().to_monday_zero_offset();
                Some(days(&self.value).contains(&(arrived as usize)))
            }
            Field::Hour => {
                let at = m.arrived()?.time();
                let from = time(&self.value)?;
                Some(match self.test {
                    Test::Before => at < from,
                    Test::After => at >= from,
                    _ => {
                        let to = time(&self.until)?;
                        if from <= to { from <= at && at < to } else { at >= from || at < to }
                    }
                })
            }
            Field::Size => {
                let (bytes, limit) = (m.facts.size?, size(&self.value)?);
                Some(if self.test == Test::Above { bytes > limit } else { bytes < limit })
            }
            Field::Anywhere => {
                let people = [Field::From, Field::To, Field::Cc].into_iter().any(|f| m.people(f).iter().any(|p| p.both.contains(&value)));
                Some(people || m.subject().contains(&value) || m.text_contains(&value))
            }
            Field::Account => m.place.as_ref().map(|p| p.account == self.value.trim()),
            Field::Folder => m.place.as_ref().map(|p| folder_named(&self.value, p.role, &p.folder)),
            Field::Mark => m.place.as_ref().map(|p| match self.value.trim() {
                "read" => p.flags.contains('S'),
                "flagged" => p.flags.contains('F'),
                "answered" => p.flags.contains('R'),
                _ => false,
            }),
            Field::Unknown => None,
        }
    }

    /// The condition in words: "From contains “@bank.example”", "has a PDF attached".
    pub fn said(&self, tr: &Translator) -> String {
        if self.problem().is_some() {
            return tr.text("filter-if-unfinished", None);
        }
        let mut args = crate::i18n::args();
        args.set("field", self.field.id());
        args.set("test", self.test.id());
        args.set("value", self.value.trim().to_string());
        let today = jiff::Zoned::now().date();
        let id = match self.field.value(self.test) {
            Value::Nothing => match self.field {
                Field::Attachment => "filter-if-attachment",
                _ => "filter-if-list",
            },
            Value::Text => "filter-if-text",
            Value::Kind => {
                args.set("kind", tr.text(&format!("filter-kind-{}", self.value.trim()), None));
                "filter-if-kind"
            }
            Value::Who => {
                args.set("who", tr.text(&format!("filter-who-{}", self.value.trim()), None));
                "filter-if-sender"
            }
            Value::Date | Value::Dates => {
                args.set("value", day(&self.value).map(|d| tr.day_in(d, today)).unwrap_or_default());
                args.set("until", day(&self.until).map(|d| tr.day_in(d, today)).unwrap_or_default());
                "filter-if-date"
            }
            Value::Days => {
                let names: Vec<String> = days(&self.value).into_iter().map(|d| tr.text(&format!("weekday-{}", d + 1), None)).collect();
                args.set("days", joined(tr, &names, "filter-join-any"));
                "filter-if-weekday"
            }
            Value::Time | Value::Times => {
                args.set("value", time(&self.value).map(|t| t.strftime("%H:%M").to_string()).unwrap_or_default());
                args.set("until", time(&self.until).map(|t| t.strftime("%H:%M").to_string()).unwrap_or_default());
                "filter-if-hour"
            }
            Value::Size => {
                args.set("value", size(&self.value).map(|bytes| said_size(tr, bytes)).unwrap_or_default());
                "filter-if-size"
            }
        };
        tr.text(id, Some(&args))
    }
}

/// Whether all conditions hold (`Join::All`), or one (`Join::Any`). None never holds.
pub fn holds(conditions: &[Condition], join: Join, message: &Message) -> bool {
    match join {
        Join::All => !conditions.is_empty() && conditions.iter().all(|c| c.matches(message)),
        Join::Any => conditions.iter().any(|c| c.matches(message)),
    }
}

/// Every condition, or one of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Join {
    #[default]
    All,
    Any,
}

impl Join {
    fn is_all(&self) -> bool {
        *self == Join::All
    }
}

// ---------------------------------------------------------------- the message read

/// What a condition knows of a message beyond its card, given by whoever asks.
#[derive(Debug, Clone)]
pub struct Facts {
    /// Who its sender is to you (`porch::Senders::who_of`); none when not
    /// judged: a condition on the sender then never holds.
    pub who: Option<Who>,
    /// Nothing proves its sender (forged; SPF and DKIM failed, nothing
    /// vouching: `trust::authenticated`): read as from nobody.
    pub unproven: bool,
    /// Its size in bytes; none when unknown.
    pub size: Option<u64>,
    /// When it arrived, as Unix seconds (`porch::sent`); none when unknown.
    pub arrived: Option<i64>,
    /// Where its day and its hour are read: this device's time zone.
    pub zone: jiff::tz::TimeZone,
}

impl Facts {
    /// What its file says: its size, when it arrived; its sender not judged.
    pub fn of(card: &Card) -> Facts {
        let size = card.path.as_ref().and_then(|path| std::fs::metadata(path).ok()).map(|m| m.len());
        Facts { who: None, unproven: false, size, arrived: crate::porch::sent(card), zone: jiff::tz::TimeZone::system() }
    }
}

/// One address of a message, folded.
#[derive(Debug, Clone, Default)]
struct Person {
    name: String,
    address: String,
    /// `name <address>`, for "contains".
    both: String,
}

impl Person {
    fn new(name: Option<&str>, address: Option<&str>) -> Person {
        let (name, address) = (fold(name.unwrap_or("")), fold(address.unwrap_or("")));
        let both = match (name.is_empty(), address.is_empty()) {
            (false, false) => format!("{name} <{address}>"),
            (true, _) => address.clone(),
            (false, true) => name.clone(),
        };
        Person { name, address, both }
    }
}

/// A message as conditions read it: its card and the facts beside it, each
/// field folded once, when a condition first asks for it.
pub struct Message<'a> {
    pub card: &'a Card,
    pub facts: Facts,
    from: OnceCell<Vec<Person>>,
    to: OnceCell<Vec<Person>>,
    cc: OnceCell<Vec<Person>>,
    reply_to: OnceCell<Vec<Person>>,
    subject: OnceCell<String>,
    excerpt: OnceCell<String>,
    text: OnceCell<String>,
    names: OnceCell<Vec<String>>,
    list: OnceCell<String>,
    /// Where it is and its marks: the search's (`placed`); none for a filter.
    place: Option<Place>,
}

/// The characters of a card's text kept beside it (`card::EXCERPT_LIMIT`):
/// one shorter holds the whole text; one this long may be cut.
const KEPT: usize = 6000;

impl<'a> Message<'a> {
    pub fn new(card: &'a Card, facts: Facts) -> Message<'a> {
        Message {
            card,
            facts,
            from: OnceCell::new(),
            to: OnceCell::new(),
            cc: OnceCell::new(),
            reply_to: OnceCell::new(),
            subject: OnceCell::new(),
            excerpt: OnceCell::new(),
            text: OnceCell::new(),
            names: OnceCell::new(),
            list: OnceCell::new(),
            place: None,
        }
    }

    /// With where it is and its marks, for the search's own conditions.
    pub fn placed(mut self, place: Place) -> Message<'a> {
        self.place = Some(place);
        self
    }

    /// With what its file says (`Facts::of`): its sender not judged.
    pub fn of(card: &'a Card) -> Message<'a> {
        Message::new(card, Facts::of(card))
    }

    fn people(&self, field: Field) -> &[Person] {
        let cell = match field {
            Field::From => &self.from,
            Field::To => &self.to,
            Field::Cc => &self.cc,
            _ => &self.reply_to,
        };
        cell.get_or_init(|| {
            // Whom it claims to come from, or to answer to, says nothing when nothing proves it.
            if self.facts.unproven && matches!(field, Field::From | Field::ReplyTo) {
                return Vec::new();
            }
            match field {
                Field::From if self.card.from_name.is_none() && self.card.from_address.is_none() => Vec::new(),
                Field::From => vec![Person::new(self.card.from_name.as_deref(), self.card.from_address.as_deref())],
                Field::To => addresses(&self.card.headers, "To"),
                Field::Cc => addresses(&self.card.headers, "Cc"),
                _ => addresses(&self.card.headers, "Reply-To"),
            }
        })
    }

    fn subject(&self) -> &String {
        self.subject.get_or_init(|| fold(&self.card.subject))
    }

    /// Whether its text holds `value`: the start kept with the card first,
    /// the whole text read again from its file only when that start may be cut.
    fn text_contains(&self, value: &str) -> bool {
        if self.excerpt.get_or_init(|| fold(&self.card.excerpt)).contains(value) {
            return true;
        }
        self.card.excerpt.chars().count() >= KEPT && self.text.get_or_init(|| self.card.full_text().map(|t| fold(&t)).unwrap_or_default()).contains(value)
    }

    fn names(&self) -> &[String] {
        self.names.get_or_init(|| self.card.attachments.iter().map(|name| fold(name)).collect())
    }

    fn list(&self) -> &String {
        self.list.get_or_init(|| self.card.headers.first("List-Id").map(fold).unwrap_or_default())
    }

    fn who(&self) -> Option<Who> {
        if self.facts.unproven { Some(Who::Stranger) } else { self.facts.who }
    }

    /// When it arrived, in this device's time zone.
    fn arrived(&self) -> Option<jiff::civil::DateTime> {
        let at = jiff::Timestamp::from_second(self.facts.arrived?).ok()?;
        Some(at.to_zoned(self.facts.zone.clone()).datetime())
    }
}

/// The addresses of a header (To, Cc, Reply-To), their names decoded.
fn addresses(headers: &RawHeaders, name: &str) -> Vec<Person> {
    let mut block = String::new();
    for value in headers.all(name) {
        block.push_str(&format!("{name}: {value}\r\n"));
    }
    if block.is_empty() {
        return Vec::new();
    }
    block.push_str("\r\n");
    let Some(parsed) = mail_parser::MessageParser::default().parse_headers(block.as_bytes()) else { return Vec::new() };
    let list = match name {
        "To" => parsed.to(),
        "Cc" => parsed.cc(),
        _ => parsed.reply_to(),
    };
    list.map(|l| l.iter().map(|a| Person::new(a.name(), a.address())).collect()).unwrap_or_default()
}

fn who_is(who: Who, value: &str) -> bool {
    match value {
        "known" => matches!(who, Who::Safe | Who::Neutral | Who::Restricted),
        "safe" => who == Who::Safe,
        "neutral" => who == Who::Neutral,
        "restricted" => who == Who::Restricted,
        "stranger" => who == Who::Stranger,
        _ => false,
    }
}

/// The kind of an attached file, by its type, else by its name's ending (`KINDS`; "other" for the rest).
pub fn attachment_kind(name: &str, mime: &str) -> &'static str {
    let mime = mime.trim().to_ascii_lowercase();
    let ending = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let by_ending = || match ending.as_str() {
        "pdf" => "pdf",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "tif" | "tiff" | "bmp" | "svg" => "image",
        "doc" | "docx" | "odt" | "rtf" | "xls" | "xlsx" | "ods" | "csv" | "ppt" | "pptx" | "odp" | "pages" | "numbers" | "key" => "document",
        "zip" | "7z" | "rar" | "gz" | "tgz" | "tar" | "xz" | "bz2" => "archive",
        "ics" | "vcs" => "calendar",
        "mp3" | "m4a" | "ogg" | "opus" | "wav" | "flac" | "aac" | "amr" => "audio",
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" => "video",
        "txt" | "md" | "eml" => "text",
        _ => "other",
    };
    match mime.as_str() {
        "application/pdf" => "pdf",
        "text/calendar" | "application/ics" => "calendar",
        m if m.starts_with("image/") => "image",
        m if m.starts_with("audio/") => "audio",
        m if m.starts_with("video/") => "video",
        m if m.contains("zip") || m.contains("compressed") || m.contains("x-tar") || m.contains("x-7z") || m.contains("x-rar") => "archive",
        m if m.contains("word") || m.contains("officedocument") || m.contains("opendocument") || m.contains("excel") || m.contains("powerpoint") || m.contains("rtf") || m == "text/csv" => "document",
        m if m.starts_with("text/") => "text",
        _ => by_ending(),
    }
}

/// Text as conditions compare it: case and accents aside, each run of spaces one space, trimmed.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars().map(crate::text::fold_char) {
        if c.is_whitespace() {
            space = !out.is_empty();
        } else {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    out
}

/// A day as a condition writes it: "2026-10-01".
fn day(text: &str) -> Option<jiff::civil::Date> {
    text.trim().parse().ok()
}

/// A time of day as a condition writes it: "18:00", "8:30".
fn time(text: &str) -> Option<jiff::civil::Time> {
    let (hours, minutes) = text.trim().split_once([':', 'h', 'H'])?;
    let (hours, minutes): (i8, i8) = (hours.trim().parse().ok()?, if minutes.trim().is_empty() { 0 } else { minutes.trim().parse().ok()? });
    jiff::civil::Time::new(hours, minutes, 0, 0).ok()
}

/// Days of the week as a condition writes them ("sat,sun"): their places from Monday (0), in order.
fn days(text: &str) -> Vec<usize> {
    let mut found: Vec<usize> = text.split(',').filter_map(|d| DAYS.iter().position(|day| *day == d.trim().to_ascii_lowercase())).collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// A size as a condition writes it, in bytes: "5 MB", "500 KB", "1.5 Go"; megabytes when it says no unit.
pub fn size(text: &str) -> Option<u64> {
    let text = text.trim().replace(',', ".");
    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: f64 = number.parse().ok().filter(|n: &f64| n.is_finite() && *n >= 0.0)?;
    let unit = match unit.trim().to_ascii_lowercase().as_str() {
        "b" | "o" | "bytes" | "octets" => 1.0,
        "k" | "kb" | "ko" | "kib" => 1024.0,
        "" | "m" | "mb" | "mo" | "mib" => 1024.0 * 1024.0,
        "g" | "gb" | "go" | "gib" => 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((number * unit).round() as u64)
}

/// A size in words: "5 MB", "500 KB", in your language's units and decimals.
fn said_size(tr: &Translator, bytes: u64) -> String {
    let mut args = crate::i18n::args();
    let (id, value) = if bytes >= 1024 * 1024 { ("size-mb", tr.decimal(bytes as f32 / (1024.0 * 1024.0))) } else { ("size-kb", (bytes / 1024).to_string()) };
    args.set("n", value);
    tr.text(id, Some(&args))
}

/// Words joined: "Saturday or Sunday"; "Monday, Tuesday or Friday".
fn joined(tr: &Translator, words: &[String], last: &str) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., end] => format!("{}{}{end}", rest.join(", "), tr.text(last, None)),
    }
}

// ---------------------------------------------------------------- actions

/// What a filter does to a message that matches, on its server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "kebab-case")]
pub enum Act {
    /// Into one of its account's folders, named as you read it ("Banque",
    /// "Factures/2026") or as its server does ("INBOX.Banque").
    Move { folder: String },
    /// Into its account's archive, whatever its server calls it ("Archive",
    /// Gmail's "All Mail"), as the Archive button does: out of the inbox, kept.
    Archive,
    /// Spam: marked `$Junk` and moved into its account's Junk folder, as the
    /// Junk button does, but with no line in the spam filter's label log: a
    /// filter is no word of yours on that one message.
    Junk,
    /// Flagged: important, to follow.
    Flag,
    /// Marked read.
    Read,
    /// Into the trash, and kept there: never deleted for good.
    Trash,
    /// An IMAP keyword, which other mail programs show as a tag ("$label1", "Factures").
    Keyword { name: String },
    /// An action this Sioul does not know: the filter waits until it is changed.
    #[serde(other)]
    Unknown,
}

impl Act {
    /// The actions as the editor offers them, by their ids.
    pub const IDS: [&'static str; 7] = ["move", "archive", "junk", "flag", "read", "trash", "keyword"];

    pub fn id(&self) -> &'static str {
        match self {
            Act::Move { .. } => "move",
            Act::Archive => "archive",
            Act::Junk => "junk",
            Act::Flag => "flag",
            Act::Read => "read",
            Act::Trash => "trash",
            Act::Keyword { .. } => "keyword",
            Act::Unknown => "unknown",
        }
    }

    /// An action from its id and what it names (a folder, a keyword).
    pub fn of(id: &str, name: &str) -> Act {
        match id.trim() {
            "move" => Act::Move { folder: name.trim().to_string() },
            "archive" => Act::Archive,
            "junk" => Act::Junk,
            "flag" => Act::Flag,
            "read" => Act::Read,
            "trash" => Act::Trash,
            "keyword" => Act::Keyword { name: name.trim().to_string() },
            _ => Act::Unknown,
        }
    }

    /// What it names: its folder, its keyword; "" for the others.
    pub fn name(&self) -> &str {
        match self {
            Act::Move { folder } => folder,
            Act::Keyword { name } => name,
            _ => "",
        }
    }

    /// What keeps it from being done, as a message id; none when it can be.
    pub fn problem(&self) -> Option<&'static str> {
        match self {
            Act::Move { folder } if folder.trim().is_empty() => Some("filter-problem-folder"),
            Act::Keyword { name } if !keyword(name) => Some("filter-problem-keyword"),
            Act::Unknown => Some("filter-problem-unknown"),
            _ => None,
        }
    }

    /// The action in words: "into “Banque”", "marked read"; one still to
    /// finish (its folder not chosen yet), said so.
    pub fn said(&self, tr: &Translator) -> String {
        if matches!(self, Act::Move { .. } | Act::Keyword { .. }) && self.problem().is_some() {
            return tr.text("filter-then-unfinished", None);
        }
        let mut args = crate::i18n::args();
        args.set("name", self.name().trim().to_string());
        tr.text(&format!("filter-then-{}", self.id()), Some(&args))
    }
}

/// The keyword that says filters went through a message: set on its server
/// before they act on it, so that another device fetching it too leaves it
/// alone (sioul-sync's `filters`).
pub const FILTERED: &str = "$SioulFiltered";

/// Whether `name` can be an IMAP keyword a filter adds (RFC 9051 §2.3.2, an
/// atom): no space, no `( ) { % * " \ ]`, no control character; never a
/// system flag, the spam keywords (the spam action sets those) nor Sioul's own mark.
pub fn keyword(name: &str) -> bool {
    let name = name.trim();
    let lower = name.to_ascii_lowercase();
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('\\')
        && name.chars().all(|c| c.is_ascii_graphic() && !"(){%*\"\\]".contains(c))
        && lower != "$junk"
        && lower != "$notjunk"
        && lower != FILTERED.to_ascii_lowercase()
}

// ---------------------------------------------------------------- filters

/// One filter: its conditions, what it does, on which accounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    /// Its name, as you gave it; "" lets its sentence name it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Switched off, it is kept and not run.
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub enabled: bool,
    /// The accounts it runs on, by id; none: every account.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<String>,
    /// Every condition, or one of them.
    #[serde(default, rename = "match", skip_serializing_if = "Join::is_all")]
    pub join: Join,
    #[serde(default, rename = "if")]
    pub conditions: Vec<Condition>,
    #[serde(default, rename = "then")]
    pub actions: Vec<Act>,
    /// Once it matched, the filters after it are not asked.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stop: bool,
}

fn on() -> bool {
    true
}

fn is_on(enabled: &bool) -> bool {
    *enabled
}

impl Default for Filter {
    fn default() -> Filter {
        Filter { name: String::new(), enabled: true, accounts: Vec::new(), join: Join::All, conditions: Vec::new(), actions: Vec::new(), stop: false }
    }
}

impl Filter {
    /// What keeps it from running, as a message id; none when it can:
    /// a condition at least, an action at least, each readable.
    pub fn problem(&self) -> Option<&'static str> {
        if self.conditions.is_empty() {
            return Some("filter-problem-no-condition");
        }
        if self.actions.is_empty() {
            return Some("filter-problem-no-action");
        }
        self.conditions.iter().find_map(Condition::problem).or_else(|| self.actions.iter().find_map(Act::problem))
    }

    /// Whether it holds a field, a test or an action this Sioul does not
    /// know: a newer one wrote it (`config::set_filters` then keeps it as written).
    pub fn unknown(&self) -> bool {
        self.conditions.iter().any(|c| c.field == Field::Unknown || c.test == Test::Unknown) || self.actions.contains(&Act::Unknown)
    }

    /// Whether it runs on `account`'s mail: on, complete, for that account.
    pub fn runs_on(&self, account: &str) -> bool {
        self.enabled && self.problem().is_none() && (self.accounts.is_empty() || self.accounts.iter().any(|a| a == account))
    }

    /// Whether `message` shows its conditions: all of them, or one.
    pub fn matches(&self, message: &Message) -> bool {
        holds(&self.conditions, self.join, message)
    }

    /// The filter in one line: "From contains “@bank.example” → into “Banque”, marked read".
    pub fn said(&self, tr: &Translator) -> String {
        if self.conditions.is_empty() && self.actions.is_empty() {
            return tr.text("filter-said-empty", None);
        }
        let conditions: Vec<String> = self.conditions.iter().map(|c| c.said(tr)).collect();
        let join = if self.join == Join::Any { "filter-join-any" } else { "filter-join-all" };
        let mut actions: Vec<String> = self.actions.iter().map(|a| a.said(tr)).collect();
        if self.stop {
            actions.push(tr.text("filter-then-stop", None));
        }
        let mut args = crate::i18n::args();
        args.set("conditions", if conditions.is_empty() { tr.text("filter-if-none", None) } else { conditions.join(&tr.text(join, None)) });
        args.set("actions", if self.actions.is_empty() { tr.text("filter-then-none", None) } else { actions.join(", ") });
        // A sentence: its first condition may begin in lower case ("sent by a newsletter…").
        let said = tr.text("filter-said", Some(&args));
        let mut letters = said.chars();
        match letters.next() {
            Some(first) => first.to_uppercase().chain(letters).collect(),
            None => said,
        }
    }
}

/// Reads the filters of the configuration one by one: one that does not read
/// is left out, never the whole configuration with it.
pub fn lenient<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<Filter>, D::Error> {
    let value = toml::Value::deserialize(deserializer)?;
    let toml::Value::Array(items) = value else { return Ok(Vec::new()) };
    Ok(items.into_iter().filter_map(|item| item.try_into::<Filter>().ok()).collect())
}

/// Where a filter sends a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Destination {
    /// One of its account's folders, as the filter names it.
    Folder(String),
    /// Its account's archive, found by its purpose.
    Archive,
    Junk,
    Trash,
}

/// What the filters do to one message, once added up: the first filter that
/// moves it says where it goes; reads, flags and keywords add up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub to: Option<Destination>,
    pub read: bool,
    pub flag: bool,
    pub keywords: Vec<String>,
    /// The filters that matched, by their place in the list.
    pub by: Vec<usize>,
}

impl Plan {
    /// No filter matched.
    pub fn is_empty(&self) -> bool {
        self.by.is_empty()
    }

    /// It leaves the inbox: into a folder, the junk, the trash.
    pub fn leaves(&self) -> bool {
        self.to.is_some()
    }

    /// Never told as new mail: it leaves the inbox, or it is marked read (as mail read elsewhere).
    pub fn quiet(&self) -> bool {
        self.leaves() || self.read
    }
}

/// What `filters` do to `message`, which came to `account`: each filter that
/// runs on that account and matches, in their order, until one that stops
/// the rest.
pub fn plan(filters: &[Filter], account: &str, message: &Message) -> Plan {
    let mut plan = Plan::default();
    for (at, filter) in filters.iter().enumerate() {
        if !filter.runs_on(account) || !filter.matches(message) {
            continue;
        }
        plan.by.push(at);
        for act in &filter.actions {
            match act {
                Act::Move { folder } => {
                    plan.to.get_or_insert_with(|| Destination::Folder(folder.trim().to_string()));
                }
                Act::Archive => {
                    plan.to.get_or_insert(Destination::Archive);
                }
                Act::Junk => {
                    plan.to.get_or_insert(Destination::Junk);
                }
                Act::Trash => {
                    plan.to.get_or_insert(Destination::Trash);
                }
                Act::Read => plan.read = true,
                Act::Flag => plan.flag = true,
                Act::Keyword { name } => {
                    if !plan.keywords.iter().any(|k| k.eq_ignore_ascii_case(name.trim())) {
                        plan.keywords.push(name.trim().to_string());
                    }
                }
                Act::Unknown => {}
            }
        }
        if filter.stop {
            break;
        }
    }
    plan
}

/// Whether the Porch keeps a message from every filter: set aside (forged,
/// blocked, a borrowed name, your provider's spam), hostile, in your spam
/// filter's review queue (it moves or flags that mail itself), or a code you
/// asked for, which must reach you where you look for it.
pub fn untouched(judged: &crate::porch::Triaged) -> bool {
    use crate::porch::Lane;
    matches!(judged.lane, Lane::SetAside | Lane::Hostile | Lane::Review | Lane::RightNow)
}

/// The facts beside a message the Porch judged: who its sender is to you,
/// and whether anything proves it is them.
pub fn facts_of(judged: &crate::porch::Triaged, senders: &crate::porch::Senders) -> Facts {
    let unproven = judged.trust == crate::trust::Trust::Forged || judged.reasons.contains(&crate::porch::Reason::NotAuthenticated);
    Facts { who: Some(senders.who_of(&judged.card)), unproven, ..Facts::of(&judged.card) }
}

/// What `filters` do to each message the Porch judged, for its account;
/// none for what it protects (`untouched`), nor where no filter matches.
pub fn plans(filters: &[Filter], judged: &[crate::porch::Triaged], senders: &crate::porch::Senders) -> Vec<(std::path::PathBuf, Plan)> {
    judged
        .iter()
        .filter(|t| !untouched(t))
        .filter_map(|t| {
            let path = t.card.path.clone()?;
            let account = t.card.account.as_deref()?;
            let plan = plan(filters, account, &Message::new(&t.card, facts_of(t, senders)));
            (!plan.is_empty()).then_some((path, plan))
        })
        .collect()
}

// ---------------------------------------------------------------- the editor's words

/// One choice the editor offers: its id and its words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

/// A field as the editor offers it: its tests, and what each asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldForm {
    pub id: String,
    pub label: String,
    pub tests: Vec<TestForm>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestForm {
    pub id: String,
    pub label: String,
    pub value: Value,
}

/// Everything the filters' editor offers, in your language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Form {
    pub fields: Vec<FieldForm>,
    pub actions: Vec<Choice>,
    pub kinds: Vec<Choice>,
    pub who: Vec<Choice>,
    /// Monday first, short names.
    pub days: Vec<Choice>,
}

/// The editor's words.
pub fn form(tr: &Translator) -> Form {
    let test_label = |field: Field, test: Test| match test {
        Test::Exists | Test::Missing => tr.text(&format!("filter-test-{}-{}", test.id(), field.id()), None),
        _ => tr.text(&format!("filter-test-{}", test.id()), None),
    };
    let fields = Field::ALL
        .iter()
        .map(|&field| FieldForm {
            id: field.id().into(),
            label: tr.text(&format!("filter-field-{}", field.id()), None),
            tests: field.tests().iter().map(|&test| TestForm { id: test.id().into(), label: test_label(field, test), value: field.value(test) }).collect(),
        })
        .collect();
    let choices = |ids: &[&str], prefix: &str| ids.iter().map(|id| Choice { id: id.to_string(), label: tr.text(&format!("{prefix}{id}"), None) }).collect::<Vec<_>>();
    let days = (0..7).map(|d| Choice { id: DAYS[d].into(), label: tr.text(&format!("weekday-{}", d + 1), None) }).collect();
    Form { fields, actions: choices(&Act::IDS, "filter-act-"), kinds: choices(&KINDS, "filter-kind-"), who: choices(&WHO, "filter-who-"), days }
}

// ---------------------------------------------------------------- Virtual Secretary

/// What Virtual Secretary's sorting proposes for a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposed {
    /// Put the message in a case, by id.
    AssignCase(String),
    /// File it on a shelf, out of the way.
    File(Shelf),
    /// Set it aside, with the reason.
    SetAside(String),
    /// A free label, like Virtual Secretary's tags or IMAP keywords.
    Label(String),
    /// Teach the classifier it is junk, and set it aside.
    Junk,
    /// To the trash, recoverable for 30 days.
    Trash,
    /// Flagged, to follow.
    Flag,
    /// Propose a task in the message's case.
    ProposeTask { title: String, due: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    Newsletters,
    Notifications,
    Receipts,
    Calendar,
}

impl Shelf {
    /// Shelf names as written in the configuration.
    pub fn parse(name: &str) -> Option<Shelf> {
        match name.trim().to_ascii_lowercase().as_str() {
            "newsletters" => Some(Shelf::Newsletters),
            "notifications" => Some(Shelf::Notifications),
            "receipts" => Some(Shelf::Receipts),
            "calendar" => Some(Shelf::Calendar),
            _ => None,
        }
    }
}

/// Virtual Secretary, compatibility mode A: it keeps sorting on the server;
/// the IMAP folder it moved a message to becomes a proposal here.
#[derive(Debug, Clone, Deserialize)]
pub struct FolderMapping {
    /// The IMAP folder, as Virtual Secretary names it: "INBOX.Money.Taxes".
    pub folder: String,
    #[serde(default)]
    pub case: Option<String>,
    #[serde(default)]
    pub shelf: Option<String>,
    #[serde(default)]
    pub junk: bool,
}

impl FolderMapping {
    /// What a message found in `folder` should become.
    pub fn actions_for(mappings: &[FolderMapping], folder: &str) -> Vec<Proposed> {
        let Some(m) = mappings.iter().find(|m| m.folder.eq_ignore_ascii_case(folder)) else { return Vec::new() };
        let case = m.case.clone().map(Proposed::AssignCase);
        let shelf = m.shelf.as_deref().and_then(Shelf::parse).map(Proposed::File);
        let junk = m.junk.then_some(Proposed::Junk);
        [case, shelf, junk].into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_virtual_secretary_folders() {
        let mappings: Vec<FolderMapping> = toml::from_str::<std::collections::HashMap<String, Vec<FolderMapping>>>(
            r#"
            [[map]]
            folder = "INBOX.Money.Taxes"
            case = "taxes"
            [[map]]
            folder = "INBOX.Services.Notifications"
            shelf = "notifications"
            [[map]]
            folder = "INBOX.spam"
            junk = true
            "#,
        )
        .unwrap()
        .remove("map")
        .unwrap();
        assert_eq!(FolderMapping::actions_for(&mappings, "inbox.money.taxes"), vec![Proposed::AssignCase("taxes".into())]);
        assert_eq!(FolderMapping::actions_for(&mappings, "INBOX.spam"), vec![Proposed::Junk]);
        assert!(FolderMapping::actions_for(&mappings, "INBOX").is_empty());
    }

    // ------------------------------------------------------------ filters, on invented mail

    fn card(raw: &str) -> Card {
        Card::from_bytes(raw.replace('\n', "\r\n").as_bytes()).unwrap()
    }

    /// A bank's statement: a name with an accent in Cc, a list, a PDF.
    fn statement() -> Card {
        card(
            "From: \"Banque Exemple\" <Alertes@Bank.example>\n\
             To: \"Noa\" <noa@home.example>, team@work.example\n\
             Cc: =?UTF-8?Q?Ren=C3=A9e?= <renee@family.example>\n\
             Reply-To: support@bank.example\n\
             Subject: Votre RELEVÉ de compte\n\
             List-Id: \"Alertes\" <alertes.bank.example>\n\
             Date: Thu, 01 Oct 2026 10:00:00 +0200\n\
             MIME-Version: 1.0\n\
             Content-Type: multipart/mixed; boundary=b\n\
             \n\
             --b\n\
             Content-Type: text/plain; charset=utf-8\n\
             \n\
             Bonjour, votre relevé   est disponible.\n\
             --b\n\
             Content-Type: application/pdf; name=releve-octobre.pdf\n\
             Content-Disposition: attachment; filename=releve-octobre.pdf\n\
             Content-Transfer-Encoding: base64\n\
             \n\
             JVBERi0=\n\
             --b--\n",
        )
    }

    /// Thursday 1 October 2026, 10:00 at UTC+2; 5,000 bytes; a neutral sender.
    fn facts() -> Facts {
        Facts { who: Some(Who::Neutral), unproven: false, size: Some(5000), arrived: Some(1_790_841_600), zone: jiff::tz::TimeZone::fixed(jiff::tz::offset(2)) }
    }

    fn holds_on(card: &Card, facts: Facts, field: Field, test: Test, value: &str) -> bool {
        Condition::new(field, test, value).matches(&Message::new(card, facts))
    }

    #[test]
    fn every_field_reads_case_and_accents_aside() {
        let mail = statement();
        let yes = |field, test, value: &str| assert!(holds_on(&mail, facts(), field, test, value), "{field:?} {test:?} {value:?} should hold");
        let no = |field, test, value: &str| assert!(!holds_on(&mail, facts(), field, test, value), "{field:?} {test:?} {value:?} should not hold");
        // The sender: its name and address, either whole for "is".
        yes(Field::From, Test::Contains, "@BANK.example");
        yes(Field::From, Test::Is, "alertes@bank.example");
        yes(Field::From, Test::Is, "banque exemple");
        no(Field::From, Test::Is, "bank.example");
        no(Field::From, Test::NotContains, "@bank");
        yes(Field::From, Test::IsNot, "someone@else.example");
        // Recipients, a name decoded and folded.
        yes(Field::To, Test::Contains, "team@work");
        yes(Field::To, Test::Is, "Noa");
        yes(Field::Cc, Test::Contains, "renee");
        yes(Field::Cc, Test::Is, "Renée");
        yes(Field::ReplyTo, Test::Is, "support@bank.example");
        no(Field::ReplyTo, Test::Contains, "alertes");
        // The subject and the text: case, accents and runs of spaces aside.
        yes(Field::Subject, Test::Contains, "releve de");
        no(Field::Subject, Test::Is, "releve");
        yes(Field::Subject, Test::Is, "votre relevé de compte");
        yes(Field::Body, Test::Contains, "Relevé est");
        yes(Field::Body, Test::NotContains, "facture");
        // Attachments: one there, its name, its kind.
        yes(Field::Attachment, Test::Exists, "");
        no(Field::Attachment, Test::Missing, "");
        yes(Field::Attachment, Test::Contains, "OCTOBRE");
        yes(Field::AttachmentType, Test::Is, "pdf");
        yes(Field::AttachmentType, Test::IsNot, "image");
        // A list, and which.
        yes(Field::List, Test::Exists, "");
        yes(Field::List, Test::Contains, "alertes.bank");
        no(Field::List, Test::NotContains, "bank.example");
    }

    #[test]
    fn when_it_arrived_and_how_large() {
        let mail = statement();
        let at = |field, test, value: &str, until: &str| Condition { field, test, value: value.into(), until: until.into() }.matches(&Message::new(&mail, facts()));
        // Days: the day itself is neither before nor after it; "between" holds both ends.
        assert!(at(Field::Date, Test::After, "2026-09-30", ""));
        assert!(!at(Field::Date, Test::After, "2026-10-01", ""));
        assert!(!at(Field::Date, Test::Before, "2026-10-01", ""));
        assert!(at(Field::Date, Test::Between, "2026-10-01", "2026-10-31"));
        assert!(!at(Field::Date, Test::Between, "2026-10-31", "2026-10-01"), "a range backwards does not read");
        // Its day of the week and its hour, in its zone: Thursday, 10:00.
        assert!(at(Field::Weekday, Test::Is, "thu", ""));
        assert!(!at(Field::Weekday, Test::Is, "sat,sun", ""));
        assert!(at(Field::Weekday, Test::IsNot, "sat, SUN", ""));
        assert!(at(Field::Hour, Test::After, "09:30", ""));
        assert!(at(Field::Hour, Test::After, "10:00", ""));
        assert!(!at(Field::Hour, Test::Before, "9h30", ""));
        assert!(at(Field::Hour, Test::Between, "09:00", "11:00"));
        assert!(!at(Field::Hour, Test::Between, "22:00", "06:00"));
        // Its size: 5,000 bytes.
        assert!(at(Field::Size, Test::Above, "4 KB", ""));
        assert!(at(Field::Size, Test::Below, "0,5 Mo", ""));
        assert!(!at(Field::Size, Test::Above, "1", ""), "a size without its unit is in megabytes");
        // Unknown, nothing holds: neither larger nor smaller.
        let unknown = Facts { size: None, arrived: None, ..facts() };
        assert!(!Condition::new(Field::Size, Test::Above, "1 KB").matches(&Message::new(&mail, unknown.clone())));
        assert!(!Condition::new(Field::Size, Test::Below, "1 KB").matches(&Message::new(&mail, unknown.clone())));
        assert!(!Condition::new(Field::Weekday, Test::IsNot, "sat").matches(&Message::new(&mail, unknown)));
    }

    #[test]
    fn a_sender_nothing_proves_is_nobody() {
        let mail = statement();
        // Judged: neutral, someone you know.
        assert!(holds_on(&mail, facts(), Field::Sender, Test::Is, "known"));
        assert!(holds_on(&mail, facts(), Field::Sender, Test::IsNot, "stranger"));
        assert!(!holds_on(&mail, facts(), Field::Sender, Test::Is, "safe"));
        // Not judged: neither "is" nor "is not" holds.
        let unjudged = Facts { who: None, ..facts() };
        assert!(!holds_on(&mail, unjudged.clone(), Field::Sender, Test::Is, "stranger"));
        assert!(!holds_on(&mail, unjudged, Field::Sender, Test::IsNot, "stranger"));
        // Forged, or failing SPF and DKIM: the name it shows is no one's.
        let unproven = Facts { unproven: true, ..facts() };
        assert!(!holds_on(&mail, unproven.clone(), Field::From, Test::Contains, "@bank.example"));
        assert!(holds_on(&mail, unproven.clone(), Field::From, Test::NotContains, "@bank.example"));
        assert!(!holds_on(&mail, unproven.clone(), Field::ReplyTo, Test::Is, "support@bank.example"));
        assert!(holds_on(&mail, unproven.clone(), Field::Sender, Test::Is, "stranger"));
        // What it says beside the sender still reads.
        assert!(holds_on(&mail, unproven, Field::Subject, Test::Contains, "relevé"));
    }

    #[test]
    fn what_does_not_read_never_holds() {
        let mail = statement();
        let problem = |c: Condition| c.problem();
        assert_eq!(problem(Condition::new(Field::Subject, Test::Contains, "  ")), Some("filter-problem-value"));
        assert_eq!(problem(Condition::new(Field::Date, Test::Before, "1er octobre")), Some("filter-problem-date"));
        assert_eq!(problem(Condition::new(Field::Hour, Test::After, "25:00")), Some("filter-problem-time"));
        assert_eq!(problem(Condition::new(Field::Size, Test::Above, "big")), Some("filter-problem-size"));
        assert_eq!(problem(Condition::new(Field::Body, Test::Is, "x")), Some("filter-problem-unknown"), "the text is never whole");
        assert_eq!(problem(Condition::new(Field::Sender, Test::Is, "blocked")), Some("filter-problem-value"));
        assert_eq!(problem(Condition::new(Field::Attachment, Test::Exists, "")), None);
        // Turned around, one that does not read holds no more than before.
        assert!(!Condition::new(Field::Subject, Test::NotContains, "").matches(&Message::of(&mail)));
        // A filter needs a condition and an action.
        let mut filter = Filter::default();
        assert_eq!(filter.problem(), Some("filter-problem-no-condition"));
        filter.conditions.push(Condition::new(Field::From, Test::Contains, "@bank.example"));
        assert_eq!(filter.problem(), Some("filter-problem-no-action"));
        filter.actions.push(Act::Move { folder: " ".into() });
        assert_eq!(filter.problem(), Some("filter-problem-folder"));
        filter.actions = vec![Act::Keyword { name: "$Junk".into() }];
        assert_eq!(filter.problem(), Some("filter-problem-keyword"));
        filter.actions = vec![Act::Keyword { name: "Factures".into() }];
        assert_eq!(filter.problem(), None);
        assert!(filter.runs_on("home"));
        assert!(!keyword("two words") && !keyword(FILTERED) && !keyword("\\Seen") && keyword("$label1"));
    }

    #[test]
    fn the_first_move_decides_the_rest_adds_up() {
        let mail = statement();
        let message = Message::new(&mail, facts());
        let filter = |accounts: &[&str], value: &str, actions: Vec<Act>, stop: bool| Filter {
            accounts: accounts.iter().map(|a| a.to_string()).collect(),
            conditions: vec![Condition::new(Field::From, Test::Contains, value)],
            actions,
            stop,
            ..Filter::default()
        };
        let filters = vec![
            filter(&["work"], "@bank.example", vec![Act::Trash], false),
            filter(&[], "@bank.example", vec![Act::Move { folder: "Banque".into() }, Act::Read], false),
            filter(&["home"], "@nobody.example", vec![Act::Junk], false),
            filter(&[], "bank", vec![Act::Junk, Act::Flag, Act::Keyword { name: "Factures".into() }], true),
            filter(&[], "bank", vec![Act::Keyword { name: "never".into() }], false),
        ];
        let plan = plan(&filters, "home", &message);
        assert_eq!(plan.by, vec![1, 3], "the first is work's, the third does not match, the fifth comes after a stop");
        assert_eq!(plan.to, Some(Destination::Folder("Banque".into())));
        assert!(plan.read && plan.flag && plan.leaves() && plan.quiet());
        assert_eq!(plan.keywords, vec!["Factures".to_string()]);
        // Switched off, a filter is not asked; none matching, nothing to do.
        let mut off = filters.clone();
        off[1].enabled = false;
        assert_eq!(super::plan(&off, "home", &message).to, Some(Destination::Junk));
        assert!(super::plan(&filters[2..3], "home", &message).is_empty());
        // "Any": one condition is enough; "all": every one.
        let mut either = filter(&[], "@nobody.example", vec![Act::Flag], false);
        either.conditions.push(Condition::new(Field::Subject, Test::Contains, "relevé"));
        assert!(!either.matches(&message));
        either.join = Join::Any;
        assert!(either.matches(&message));
    }

    #[derive(Deserialize)]
    struct Mail {
        #[serde(default, rename = "filter", deserialize_with = "lenient")]
        filters: Vec<Filter>,
    }

    #[test]
    fn filters_read_and_write_as_the_configuration_has_them() {
        let text = r#"
            [[filter]]
            name = "Bank"
            accounts = ["home"]
            if = [{ field = "from", test = "contains", value = "@bank.example" }, { field = "date", test = "between", value = "2026-10-01", until = "2026-10-31" }]
            then = [{ do = "move", folder = "Banque" }, { do = "read" }]
            stop = true

            [[filter]]
            match = "any"
            enabled = false
            if = [{ field = "attachment", test = "exists" }, { field = "frobnicate", test = "contains", value = "x" }]
            then = [{ do = "keyword", name = "$label1" }, { do = "explode" }]

            [[filter]]
            if = "not a list"
        "#;
        let read: Mail = toml::from_str(text).unwrap();
        assert_eq!(read.filters.len(), 2, "the third does not read, and is left out alone");
        let bank = &read.filters[0];
        assert_eq!((bank.name.as_str(), bank.accounts.clone(), bank.join, bank.stop, bank.enabled), ("Bank", vec!["home".to_string()], Join::All, true, true));
        assert_eq!(bank.conditions[1], Condition { field: Field::Date, test: Test::Between, value: "2026-10-01".into(), until: "2026-10-31".into() });
        assert_eq!(bank.actions, vec![Act::Move { folder: "Banque".into() }, Act::Read]);
        // What this Sioul does not know is kept as unknown: that filter waits, said.
        let odd = &read.filters[1];
        assert_eq!((odd.join, odd.enabled), (Join::Any, false));
        assert_eq!(odd.conditions[1].field, Field::Unknown);
        assert_eq!(odd.actions[1], Act::Unknown);
        assert_eq!(odd.problem(), Some("filter-problem-unknown"));
        // Written back, it reads the same, the usual values left out.
        #[derive(Serialize)]
        struct Out<'a> {
            filter: &'a [Filter],
        }
        let written = toml::to_string(&Out { filter: &read.filters[..1] }).unwrap();
        assert!(!written.contains("enabled") && !written.contains("match"), "{written}");
        let again: Mail = toml::from_str(&written).unwrap();
        assert_eq!(again.filters, read.filters[..1].to_vec());
    }

    #[test]
    fn a_filter_in_a_sentence() {
        let filter = Filter {
            conditions: vec![Condition::new(Field::From, Test::Contains, "@bank.example"), Condition::new(Field::AttachmentType, Test::Is, "pdf")],
            actions: vec![Act::Move { folder: "Banque".into() }, Act::Read],
            stop: true,
            ..Filter::default()
        };
        let en = Translator::new("en");
        assert_eq!(filter.said(&en), "From contains “@bank.example” and an attachment is a PDF → into “Banque”, marked read, and no other filter");
        let fr = Translator::new("fr");
        assert_eq!(filter.said(&fr), "De contient «\u{202f}@bank.example\u{202f}» et une pièce jointe est un PDF → dans «\u{202f}Banque\u{202f}», marqué comme lu, et aucun autre filtre");
        // Every field, test and action has its words, in both languages.
        for tr in [&en, &fr] {
            let form = form(tr);
            let labels = form.fields.iter().flat_map(|f| std::iter::once(&f.label).chain(f.tests.iter().map(|t| &t.label))).chain(form.actions.iter().chain(&form.kinds).chain(&form.who).chain(&form.days).map(|c| &c.label));
            for label in labels {
                assert!(!label.starts_with("filter-") && !label.is_empty(), "{}: {label}", tr.language());
            }
            for field in Field::ALL {
                for &test in field.tests() {
                    let value = match field.value(test) {
                        Value::Nothing => "",
                        Value::Text => "x",
                        Value::Date | Value::Dates => "2026-10-01",
                        Value::Time | Value::Times => "08:00",
                        Value::Days => "sat,sun",
                        Value::Size => "5 MB",
                        Value::Kind => "image",
                        Value::Who => "stranger",
                    };
                    let said = Condition { field, test, value: value.into(), until: value.into() }.said(tr);
                    assert!(!said.contains("filter-") && !said.contains('{'), "{}: {field:?} {test:?}: {said}", tr.language());
                }
            }
            for id in Act::IDS {
                let said = Act::of(id, "x").said(tr);
                assert!(!said.starts_with("filter-"), "{}: {id}: {said}", tr.language());
            }
        }
    }
}
