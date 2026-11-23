// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Calendars subscribed by their web address (docs/client.md, "Calendars by
//! their address"; 18 November 2026): a school's timetable from Moodle or
//! PRONOTE, public holidays, school holidays, any calendar published as an
//! `.ics` file.
//!
//! - **The address is an ordinary shared setting**: `[agenda.subscriptions."<id>"]`
//!   in the configuration, typed once and carried to every device with the
//!   settings. It is never written in a log, a notice or what an agent is
//!   given: Sioul says the address's domain alone (`vdir::Collection::place`).
//! - **Each device fetches by itself** (sioul-sync's `feeds`) and keeps its own
//!   copy, `calendars/subscribed/<id>/`, which the sharing never carries; what
//!   this device last read of it is its own state, `state/subscribed/<id>.toml`.
//! - **The whole copy is replaced at once**, never half
//!   (`vdir::replace_subscribed`), and only by what reads as a calendar: a web
//!   page, a line of text, an empty answer replace nothing (`read`).
//! - **Unchanged means nothing is written**: the server's own word first
//!   (304, by the ETag or the date it gave), else what the calendar says once
//!   the stamps that change at each answer are left aside (`steady`: Moodle
//!   and Google stamp every event at each answer, OpenHolidays its creation
//!   and change too).
//! - **How often** (`due`): when Sioul opens, every three hours on a computer,
//!   a few times a day from a phone's background step; a calendar of holidays
//!   once a day; the calendar's own wish (`REFRESH-INTERVAL`, `X-PUBLISHED-TTL`)
//!   obeyed between one hour and one day.
//!
//! A calendar of holidays says which kind it is (`Holidays`): its whole days
//! count as your days off when you say so, by default for public holidays and
//! not for school holidays (`vacations`); school holidays begin after class
//! when the calendar says so (`Subscription::after_class`), as the official
//! French files do.

use crate::config::Config;
use crate::vdir::{self, Kind};
use jiff::civil::Date;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The account of the subscribed calendars' folders.
pub const ACCOUNT: &str = vdir::SUBSCRIBED;
/// The most a calendar may weigh, once unpacked: ten megabytes.
pub const MOST_BYTES: u64 = 10_000_000;
/// The most events a calendar may hold (each series counted once; its
/// times are capped as every agenda's are, `agenda::EXPANSION_LIMIT`).
pub const MOST_EVENTS: usize = 20_000;
/// The most series (events with a rule) a calendar may hold: each is
/// expanded at every reading of the agenda, up to the agenda's own cap
/// (`agenda::EXPANSION_LIMIT`), so a stranger's server could otherwise ask
/// hundreds of millions of steps of each reading (F14).
pub const MOST_SERIES: usize = 2_000;
/// The most redirects followed, each checked as the first address is.
pub const MOST_REDIRECTS: usize = 3;
/// How often a calendar is read on a computer, unless it says otherwise.
pub const EVERY: i64 = 3 * 3600;
/// How often a calendar of holidays is read.
pub const HOLIDAYS_EVERY: i64 = 86_400;
/// The bounds of a calendar's own wish: one hour, one day.
pub const SOONEST: i64 = 3600;
pub const LATEST: i64 = 86_400;
/// Sioul opened again within this time of the last read: not read again.
pub const OPENED_AGAIN: i64 = 10 * 60;

// ---------------------------------------------------------------- the setting

/// One calendar subscribed by its address, as the settings say it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    /// "https://…": a `webcal://` address is kept as `https://` (`address`).
    #[serde(default)]
    pub url: String,
    /// Its name as you gave it; "" the calendar's own, else its domain.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// "public" or "school": a calendar of holidays (`Holidays`); unsaid, none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holidays: Option<String>,
    /// Its whole days count as your days off; unsaid, yes for public
    /// holidays and no for school holidays (`days_off`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_off: Option<bool>,
    /// School holidays that begin after class: the day an event starts is
    /// still a class day, the day it ends is the morning classes resume
    /// (the Ministry of Education's files, `Official::after_class`). Unsaid, no.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_class: Option<bool>,
    /// "#rrggbb", its colour in the agenda; unsaid, the agenda's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// "These are my own engagements": its events mark you busy (do not
    /// disturb, the availability you share) and hold your time, as your own
    /// agendas' do. Unsaid, no: a calendar read from elsewhere never makes
    /// you busy by itself (the review's F41). A calendar of holidays never
    /// does: it acts through your days off alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engagements: Option<bool>,
    /// Its entry does not read here (a field of another type, written by a
    /// later Sioul or by hand; no address): what is here is the last entry
    /// that read on this device (`Fetched::entry`), else what of it reads.
    /// Never fetched, never taken out, said on its line (F20).
    #[serde(skip)]
    pub unread: bool,
}

/// The kinds of calendars of holidays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Holidays {
    Public,
    School,
}

impl Holidays {
    pub fn id(self) -> &'static str {
        match self {
            Holidays::Public => "public",
            Holidays::School => "school",
        }
    }

    pub fn parse(text: &str) -> Option<Holidays> {
        match text.trim().to_ascii_lowercase().as_str() {
            "public" => Some(Holidays::Public),
            "school" => Some(Holidays::School),
            _ => None,
        }
    }
}

impl Subscription {
    /// Which holidays it holds, when it holds holidays.
    pub fn holidays(&self) -> Option<Holidays> {
        self.holidays.as_deref().and_then(Holidays::parse)
    }

    /// Whether its whole days count as your days off: a calendar of
    /// holidays alone, by your word, else public holidays do and school
    /// holidays do not.
    pub fn days_off(&self) -> bool {
        match self.holidays() {
            Some(kind) => self.days_off.unwrap_or(kind == Holidays::Public),
            None => false,
        }
    }

    /// Whether its holidays begin after class (school holidays alone).
    pub fn after_class(&self) -> bool {
        self.holidays() == Some(Holidays::School) && self.after_class.unwrap_or(false)
    }

    /// Whether its events are your own engagements, by your word, never for
    /// a calendar of holidays (`engagements`).
    pub fn engagements(&self) -> bool {
        self.holidays().is_none() && self.engagements.unwrap_or(false)
    }
}

/// Whether a subscribed calendar's events take none of your time
/// (`agendas::Marks::transparent`): unless you said they are your own
/// engagements, a calendar read from elsewhere never marks you busy, for do
/// not disturb, the availability you share nor the plan (F41). `id`: its
/// folder's; a calendar the settings no longer name takes none either.
pub fn takes_no_time(config: &Config, id: &str) -> bool {
    !config.agenda.subscriptions.get(id).is_some_and(Subscription::engagements)
}

/// Whether an id names a subscribed calendar's folder: letters, digits, `-`, `_`.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The subscriptions, never the whole configuration stopped by one: an id
/// that names no folder of its own is passed over; an entry of another
/// shape or without an address stays, marked as not reading (`unread`), as
/// the last entry that read on this device said it, else as what of it reads:
/// absence proves nothing, and an entry that does not read changes nothing
/// (its copy, its days off, the pauses it gave: F20).
pub fn entries<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, Subscription>, D::Error> {
    let value = toml::Value::deserialize(d)?;
    let toml::Value::Table(table) = value else { return Ok(BTreeMap::new()) };
    Ok(table
        .into_iter()
        .filter(|(id, _)| valid_id(id))
        .map(|(id, value)| {
            let read = value.clone().try_into::<Subscription>().ok().filter(|s| !s.url.trim().is_empty());
            let entry = read.unwrap_or_else(|| unread(&id, &value));
            (id, entry)
        })
        .collect())
}

/// An entry that does not read: the last that read here, else its fields that read, marked so.
fn unread(id: &str, value: &toml::Value) -> Subscription {
    let last = Fetched::load(id).entry;
    let word = |field: &str| value.get(field).and_then(toml::Value::as_str).map(str::to_string);
    let yes = |field: &str| value.get(field).and_then(toml::Value::as_bool);
    // The last entry that read here, its address as the settings say it when that still reads
    // (the state keeps no address, F17).
    let last = last.map(|entry| Subscription { url: word("url").unwrap_or_default(), ..entry });
    let entry = last.unwrap_or_else(|| Subscription {
        url: word("url").unwrap_or_default(),
        name: word("name").unwrap_or_default(),
        holidays: word("holidays"),
        days_off: yes("days_off"),
        after_class: yes("after_class"),
        colour: word("colour"),
        engagements: yes("engagements"),
        unread: false,
    });
    Subscription { unread: true, ..entry }
}

/// A subscribed calendar's place among the agendas, `subscribed/<id>`.
pub fn place(id: &str) -> String {
    format!("{ACCOUNT}/{id}")
}

/// The id of a subscribed calendar from its place; none for another agenda.
pub fn id_of(place: &str) -> Option<&str> {
    place.strip_prefix(ACCOUNT).and_then(|rest| rest.strip_prefix('/')).filter(|id| valid_id(id))
}

// ---------------------------------------------------------------- the official calendars, country by country

/// The official calendars of holidays offered ready-made, as data
/// (`data/holiday-calendars.toml`, 23 November 2026): only a state's own
/// calendar, published under an open licence; `tools/check-holidays.py` asks
/// every address before each release.
const HOLIDAY_CALENDARS: &str = include_str!("../data/holiday-calendars.toml");

/// One of the official calendars of holidays offered ready-made
/// (audit-calendars §10), as the data file says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Official {
    /// ISO 3166-1, the country it is offered under: "FR", "GB".
    pub country: String,
    /// Which holidays it holds (the file's `kind`).
    #[serde(rename = "kind")]
    pub holidays: Holidays,
    /// Its id, the folder it is kept in: "fr-public-metropole". Kept for
    /// good once shipped: a calendar subscribed already is known by it.
    pub id: String,
    /// The territory's, the nation's or the zone's name, as its publisher
    /// writes it ("Métropole", "Scotland", "Zone A").
    pub name: String,
    /// Said under its name (a zone's académies); "" nothing.
    #[serde(default)]
    pub hint: String,
    pub url: String,
    /// Who publishes it, and under which licence.
    pub source: String,
    pub licence: String,
    /// Its holidays begin after class, as the Ministry's files are shaped
    /// (`Subscription::after_class`).
    #[serde(default)]
    pub after_class: bool,
}

/// The data file's shape.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Calendars {
    #[serde(default)]
    calendar: Vec<Official>,
}

/// The official calendars offered, in the file's order (by country, public
/// holidays then school holidays); none when the file does not read, which
/// its test forbids (`holiday_calendars_are_well_formed`).
pub fn officials() -> &'static [Official] {
    static LIST: std::sync::OnceLock<Vec<Official>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| match toml::from_str::<Calendars>(HOLIDAY_CALENDARS) {
        Ok(read) => read.calendar,
        Err(e) => {
            eprintln!("holiday-calendars.toml: {e}");
            Vec::new()
        }
    })
}

/// Whether an address is that of an official calendar whose holidays begin
/// after class and end the morning classes resume (the Ministry's school
/// holidays' files).
pub fn official_after_class(url: &str) -> bool {
    let url = url.trim();
    officials().iter().any(|o| o.after_class && o.url == url)
}

/// France's overseas departments and collectivities, which ISO 3166-1 names
/// apart: their official calendars are France's, by territory.
const FRANCE_OVERSEAS: [&str; 11] = ["GP", "MQ", "GF", "RE", "YT", "PM", "BL", "MF", "WF", "PF", "NC"];

/// The person's country, whose official calendars the form offers first: the
/// contacts' setting, else the language's (`reach::region`), else the
/// setting as written when it names a country the phone numbers' table does
/// not know (`clients::country`); France's overseas territories are France
/// (`FRANCE_OVERSEAS`). None when no country is known.
pub fn home_country(config: &Config) -> Option<&'static str> {
    let code = crate::reach::region(config).map(|r| r.code).or_else(|| config.contacts.region.as_deref().and_then(crate::clients::country).map(|c| c.code))?;
    Some(if FRANCE_OVERSEAS.contains(&code) { "FR" } else { code })
}

/// The official calendars as the form offers them (`offer`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The person's country (`home_country`), and its name in the language
    /// shown; "" when none is known.
    pub home: String,
    pub home_name: String,
    /// Each country that has official calendars here: its code, its name in
    /// the language shown, and its calendars, public holidays first, then
    /// school holidays, each in the file's order. The person's country
    /// first; the others by their names.
    pub countries: Vec<(String, String, Vec<&'static Official>)>,
}

impl Offer {
    /// Whether the person's country has official calendars here.
    pub fn home_has(&self) -> bool {
        self.countries.first().is_some_and(|(code, ..)| *code == self.home)
    }
}

/// A country's name in `language`, from the one table of countries
/// (`clients::country`, `data/countries.tsv`); its code when not known.
pub fn country_name(code: &str, language: &str) -> String {
    crate::clients::country(code).map_or_else(|| code.to_string(), |c| c.name(language).to_string())
}

/// The official calendars offered to a person of `home` (`home_country`),
/// in `language`: their country's first, then each other country's, by its
/// name in that language (folded: "États-Unis" among the E).
pub fn offer(home: Option<&str>, language: &str) -> Offer {
    let mut countries: Vec<(String, String, Vec<&'static Official>)> = Vec::new();
    for official in officials() {
        match countries.iter_mut().find(|(code, ..)| *code == official.country) {
            Some((.., list)) => list.push(official),
            None => countries.push((official.country.clone(), country_name(&official.country, language), vec![official])),
        }
    }
    for (.., list) in &mut countries {
        list.sort_by_key(|o| o.holidays);
    }
    let folded = |name: &str| crate::text::fold(name).into_iter().collect::<String>();
    countries.sort_by(|a, b| (Some(a.0.as_str()) != home).cmp(&(Some(b.0.as_str()) != home)).then_with(|| folded(&a.1).cmp(&folded(&b.1))));
    let home = home.unwrap_or_default();
    Offer { home: home.to_string(), home_name: if home.is_empty() { String::new() } else { country_name(home, language) }, countries }
}

// ---------------------------------------------------------------- the address

/// Why a calendar was not read, or not taken: each with its sentence (`said`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Not `https://` nor `webcal://`.
    NotHttps,
    /// Not an address: no host, spaces, a name and password in it.
    NotAddress,
    /// On a private network, or a name pointing there.
    NotPublic,
    /// Sent elsewhere more than three times.
    Redirects,
    /// More than ten megabytes once unpacked.
    TooLarge,
    /// More than twenty thousand events.
    TooMany,
    /// Far heavier kept here than it came (each event with its time zones,
    /// `agenda::kept_most`): a head of megabytes repeated for each event (F7).
    TooLargeKept,
    /// Cut short: a component left open, or no end of the calendar (F9).
    Cut,
    /// A calendar without any event, where the copy kept had some (F10).
    NoEvent,
    /// Read, but its copy could not be written on this device (a full disk) (F4).
    NotWritten,
    /// A web page, not a calendar: a sign-in page, an error page.
    Page,
    /// A line of text, not a calendar: a refusal ("Invalid authentication").
    Text(String),
    /// Nothing at all.
    Empty,
    /// Something else that does not read as a calendar.
    NotCalendar,
    /// Not found, or gone (404, 410).
    Gone,
    /// The server refused or failed (another status).
    Status(u16),
    /// The server could not be reached, or too slowly.
    Network,
}

impl Problem {
    /// Its message's id, kept in this device's state.
    pub fn id(&self) -> &'static str {
        match self {
            Problem::NotHttps => "subscribe-not-https",
            Problem::NotAddress => "subscribe-not-address",
            Problem::NotPublic => "subscribe-not-public",
            Problem::Redirects => "subscribe-redirects",
            Problem::TooLarge => "subscribe-too-large",
            Problem::TooMany => "subscribe-too-many",
            Problem::TooLargeKept => "subscribe-too-large-kept",
            Problem::Cut => "subscribe-cut",
            Problem::NoEvent => "subscribe-no-event",
            Problem::NotWritten => "subscribe-not-written",
            Problem::Page => "subscribe-page",
            Problem::Text(_) => "subscribe-text",
            Problem::Empty => "subscribe-empty",
            Problem::NotCalendar => "subscribe-not-calendar",
            Problem::Gone => "subscribe-gone",
            Problem::Status(_) => "subscribe-status",
            Problem::Network => "subscribe-network",
        }
    }

    /// Which line the agenda says of it (`line`): the address gives no
    /// calendar (with what to do), it was not reached (tried again later),
    /// or what it gave was not taken (a limit met, cut short, no event).
    pub fn line_id(&self) -> &'static str {
        match self {
            Problem::Network | Problem::Status(408 | 429 | 500..=599) => "subscribe-line-unreached",
            Problem::TooLarge | Problem::TooMany | Problem::TooLargeKept | Problem::Cut | Problem::NoEvent | Problem::NotWritten | Problem::NotPublic | Problem::Redirects => "subscribe-line-not-taken",
            _ => "subscribe-line-no-calendar",
        }
    }

    /// Its sentence.
    pub fn said(&self, tr: &crate::i18n::Translator) -> String {
        let mut args = crate::i18n::args();
        match self {
            Problem::Text(said) => args.set("said", said.clone()),
            Problem::Status(status) => args.set("status", *status),
            _ => {}
        }
        tr.text(self.id(), Some(&args))
    }
}

/// An address as Sioul reads it: `webcal://` and `webcals://` as `https://`
/// (the scheme only says "subscribe", RFC 7986 has none), `https://` as it
/// is; anything else refused, an address with a name and password in it too.
pub fn address(text: &str) -> Result<String, Problem> {
    let text = text.trim().trim_start_matches('<').trim_end_matches('>').trim();
    let rest = ["webcals://", "webcal://", "https://"].iter().find_map(|scheme| text.get(..scheme.len()).filter(|head| head.eq_ignore_ascii_case(scheme)).map(|_| &text[scheme.len()..]));
    let Some(rest) = rest else {
        return Err(if text.contains("://") || text.contains('.') { Problem::NotHttps } else { Problem::NotAddress });
    };
    if rest.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(Problem::NotAddress);
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') || authority.starts_with(':') {
        return Err(Problem::NotAddress);
    }
    Ok(format!("https://{rest}"))
}

// ---------------------------------------------------------------- what an answer holds

/// A calendar read from an answer: its events as Sioul keeps them, a file
/// each, and what it says of itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Feed {
    /// Each event's file name and text (`calendar_objects`): the same name at
    /// each fetch, made from its UID.
    pub items: Vec<(String, String)>,
    /// Its own name (RFC 7986 `NAME`, else `X-WR-CALNAME`); "" none.
    pub name: String,
    /// How often it asks to be read, seconds (RFC 7986 `REFRESH-INTERVAL`,
    /// else Microsoft's `X-PUBLISHED-TTL`); none when it says nothing that reads.
    pub refresh: Option<i64>,
    /// What it says, its stamps aside (`steady`).
    pub hash: String,
}

/// The lines of an event that change at each answer, whatever changed:
/// Moodle and Google stamp each event with the time of the request
/// (`DTSTAMP`), OpenHolidays its creation and change too.
/// Sioul's own copy of the stamp (`agenda::ORGANIZER_DTSTAMP`, which every
/// calendar object it keeps carries) changes with it.
const STAMPS: [&str; 4] = ["DTSTAMP", "LAST-MODIFIED", "CREATED", crate::agenda::ORGANIZER_DTSTAMP];

/// What an answer's bytes hold, read as a calendar: refused (`Problem`)
/// when they are no calendar (a page, a line of text, nothing), too large,
/// or hold too many events. A calendar without any event is a calendar: an
/// address whose events all passed (Moodle's coming weeks) gives one.
pub fn read(bytes: &[u8]) -> Result<Feed, Problem> {
    if bytes.len() as u64 > MOST_BYTES {
        return Err(Problem::TooLarge);
    }
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(Problem::Empty);
    }
    let head: String = trimmed.chars().take(2048).collect::<String>().to_ascii_lowercase();
    if head.starts_with('<') || head.contains("<html") || head.contains("<!doctype") || head.contains("<body") {
        return Err(Problem::Page);
    }
    let opens = text.lines().any(|l| l.trim().eq_ignore_ascii_case("BEGIN:VCALENDAR"));
    if !opens {
        // A short answer of words: what the server said (Moodle's "Invalid authentication").
        if trimmed.len() <= 300 && !trimmed.contains(['<', '{']) {
            return Err(Problem::Text(plain_words(trimmed.lines().next().unwrap_or_default(), SAID_MOST)));
        }
        return Err(Problem::NotCalendar);
    }
    // A whole calendar, its events counted as the reader reads them (F8, F9).
    let Some(events) = shape(text) else { return Err(Problem::Cut) };
    if events > MOST_EVENTS {
        return Err(Problem::TooMany);
    }
    // What comes from outside is never read as your settings: the calendar's
    // own lines of Sioul's (a reminder, the time kept around, costs, ties) are
    // dropped (F21), as another person's Sioul that publishes its agenda writes them.
    let foreign: Vec<String> = crate::lines::unfold(text).into_iter().filter(|l| !crate::lines::raw_name(l).starts_with("X-SIOUL-")).collect();
    let text = &crate::lines::fold(&foreign);
    drop(foreign);
    // Each event with what it needs, the whole weighed against what came (F7).
    let objects = match crate::agenda::calendar_objects(text, &|lines: &[String]| stable_uid(lines), crate::agenda::kept_most(bytes.len())) {
        Ok(objects) => objects,
        Err(crate::agenda::FileProblem::NoEvent) => Vec::new(),
        Err(crate::agenda::FileProblem::TooLarge) => return Err(Problem::TooLargeKept),
        Err(_) => return Err(Problem::NotCalendar),
    };
    // A rule that repeats by the second, the minute or the hour and never ends
    // is left out (no timetable, deadline or holiday is one): each reading would
    // expand it to the agenda's cap. Too many series refuse the calendar (F14).
    let mut series = 0usize;
    let objects: Vec<(String, String)> = objects
        .into_iter()
        .filter(|(_, object)| match crate::lines::unfold(object).iter().find(|l| crate::lines::name(l) == "RRULE").map(|l| crate::lines::value(l).trim().to_ascii_uppercase()) {
            None => true,
            Some(rule) => {
                let parts: Vec<&str> = rule.split(';').map(str::trim).collect();
                let restless = parts.iter().any(|p| matches!(*p, "FREQ=SECONDLY" | "FREQ=MINUTELY" | "FREQ=HOURLY")) && !parts.iter().any(|p| p.starts_with("COUNT=") || p.starts_with("UNTIL="));
                series += usize::from(!restless);
                !restless
            }
        })
        .collect();
    if series > MOST_SERIES {
        return Err(Problem::TooMany);
    }
    // The calendar's own lines, outside its components.
    let (mut name, mut refresh) = (String::new(), None);
    let (mut depth, mut alt_name, mut ttl) = (0usize, String::new(), None);
    for line in crate::lines::unfold(text) {
        let field = crate::lines::name(&line);
        let value = crate::lines::value(&line).trim();
        match field.as_str() {
            "BEGIN" if !value.eq_ignore_ascii_case("VCALENDAR") => depth += 1,
            "END" if !value.eq_ignore_ascii_case("VCALENDAR") => depth = depth.saturating_sub(1),
            "BEGIN" | "END" => {}
            // The first VCALENDAR's own, as a stream of several is read as one.
            "NAME" if depth == 0 && name.is_empty() => name = crate::lines::unescape(value),
            "X-WR-CALNAME" if depth == 0 && alt_name.is_empty() => alt_name = crate::lines::unescape(value),
            "REFRESH-INTERVAL" if depth == 0 && refresh.is_none() => refresh = duration(value),
            "X-PUBLISHED-TTL" if depth == 0 && ttl.is_none() => ttl = duration(value),
            _ => {}
        }
    }
    if name.is_empty() {
        name = alt_name;
    }
    let name = plain_words(&name, NAME_MOST);
    let mut items: Vec<(String, String)> = objects.into_iter().map(|(uid, text)| (format!("{}.ics", vdir::content_hash(uid.as_bytes())), text)).collect();
    items.sort();
    items.dedup_by(|a, b| a.0 == b.0);
    let hash = steady(&items);
    Ok(Feed { items, name, refresh: refresh.or(ttl), hash })
}

/// The most of a server's words said (F16), and of a calendar's own name.
const SAID_MOST: usize = 80;
const NAME_MOST: usize = 60;

/// Words from a server as Sioul says them (F16): one line, its control and
/// direction marks taken out (U+202E would turn the line around), any
/// address in it said as "…" (a server repeating the address asked would
/// put its token into a notice and this device's state), cut to `most`
/// characters. Drawn as plain text wherever they are shown.
pub fn plain_words(text: &str, most: usize) -> String {
    let direction = |c: char| matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
    let kept: String = text.chars().filter(|c| !c.is_control() && !direction(*c)).collect();
    let words: Vec<&str> = kept.split_whitespace().map(|word| if word.contains("://") || word.contains('?') || word.contains('=') { "…" } else { word }).collect();
    let line = words.join(" ");
    let mut out: String = line.chars().take(most).collect();
    if line.chars().count() > most {
        out.push('…');
    }
    out.trim().to_string()
}

/// How many events a whole calendar holds, as the reader reads them
/// (lines unfolded, a name read apart from its parameters: `BEGIN:VEV` folded
/// or `BEGIN;X=1:VEVENT` count, F8); none when it is not whole (F9): a
/// component left open or closed out of order, a line outside the calendar,
/// or no end of the calendar, as a server whose script stops half-way
/// answers with a 200 (the parser accepts an unterminated component).
pub(crate) fn shape(text: &str) -> Option<usize> {
    let mut open: Vec<String> = Vec::new();
    let (mut ended, mut events) = (false, 0usize);
    for line in crate::lines::unfold(text) {
        if line.trim().is_empty() {
            continue;
        }
        let name = crate::lines::name(&line);
        let value = crate::lines::value(&line).trim().to_ascii_uppercase();
        match name.as_str() {
            "BEGIN" => {
                if open.is_empty() && value != "VCALENDAR" {
                    return None;
                }
                events += usize::from(value == "VEVENT");
                open.push(value);
                ended = false;
            }
            "END" => {
                if open.pop().as_deref() != Some(value.as_str()) {
                    return None;
                }
                ended = open.is_empty();
            }
            _ if open.is_empty() => return None,
            _ => {}
        }
    }
    ended.then_some(events)
}

/// A UID for an event written without one: made from what it says, its
/// stamps aside, so that each answer gives it the same.
fn stable_uid(lines: &[String]) -> String {
    let said: String = lines.iter().filter(|l| !STAMPS.contains(&crate::lines::name(l).as_str())).map(|l| format!("{l}\n")).collect();
    format!("{}@subscribed.sioul", vdir::content_hash(said.as_bytes()))
}

/// What a calendar's events say, their stamps aside (`STAMPS`), in the
/// order of their files: the same for two answers that differ by their
/// stamps alone.
pub fn steady(items: &[(String, String)]) -> String {
    let mut said = String::new();
    for (file, text) in items {
        said.push_str(file);
        said.push('\n');
        for line in crate::lines::unfold(text) {
            if !STAMPS.contains(&crate::lines::name(&line).as_str()) {
                said.push_str(&line);
                said.push('\n');
            }
        }
    }
    vdir::content_hash(said.as_bytes())
}

/// An iCalendar duration in seconds ("P1D", "PT48H", "P1W", "PT6H30M");
/// none for one that does not read, or that is not forward.
pub fn duration(text: &str) -> Option<i64> {
    let text = text.trim().to_ascii_uppercase();
    let rest = text.strip_prefix('+').unwrap_or(&text).strip_prefix('P')?;
    let (mut seconds, mut number, mut timed, mut any) = (0i64, String::new(), false, false);
    for c in rest.chars() {
        match c {
            '0'..='9' => number.push(c),
            'T' if number.is_empty() => timed = true,
            unit => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                any = true;
                let unit = match (unit, timed) {
                    ('W', false) => 7 * 86_400,
                    ('D', false) => 86_400,
                    ('H', true) => 3600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
                // A number past what a duration can hold reads as none (F15).
                seconds = seconds.checked_add(n.checked_mul(unit)?)?;
            }
        }
    }
    (any && number.is_empty() && seconds > 0).then_some(seconds)
}

// ---------------------------------------------------------------- what this device read

/// What this device last read of a subscribed calendar
/// (`state/subscribed/<id>.toml`): its own, never shared.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fetched {
    /// Which address its copy came from, as a hash of it (`address_of`): the
    /// address itself is kept once, in the settings, never a second time in
    /// this device's state (F17). Another one now: its copy goes, and it is
    /// read again whole (F22).
    #[serde(default)]
    pub address: String,
    /// When it was last asked (Unix seconds); 0 never.
    #[serde(default)]
    pub tried: i64,
    /// When its copy was last found current, written or unchanged; 0 never.
    #[serde(default)]
    pub checked: i64,
    /// When its copy last changed here.
    #[serde(default)]
    pub changed: i64,
    /// The server's validators of the copy kept (RFC 9110 §13.1).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub etag: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_modified: String,
    /// What it said, its stamps aside (`steady`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
    /// Why the last asking gave nothing (`Problem::id`), "" when it did;
    /// what the server said with it, and since when.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub problem: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub said: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub status: u16,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub since: i64,
    /// How many askings in a row gave nothing (`every`: tried again at growing intervals, F3).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub failures: u32,
    /// How often it asks to be read, seconds; 0 when it says nothing.
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub refresh: i64,
    /// Its own name, as it last said it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// How many events its copy holds.
    #[serde(default)]
    pub events: usize,
    /// Its entry in the settings as it last read here: what stands for it
    /// while it does not read (`entries`, F20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<Subscription>,
}

fn is_zero(n: &u16) -> bool {
    *n == 0
}

fn is_zero_i64(n: &i64) -> bool {
    *n == 0
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

impl Fetched {
    pub fn path(id: &str) -> PathBuf {
        crate::config::state_dir().join("subscribed").join(format!("{id}.toml"))
    }

    pub fn load(id: &str) -> Fetched {
        std::fs::read_to_string(Fetched::path(id)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, id: &str) -> Result<(), String> {
        let path = Fetched::path(id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        crate::filelock::replace(&path, toml::to_string(self).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The problem it holds, as a `Problem`; none when the last asking gave a calendar.
    pub fn problem(&self) -> Option<Problem> {
        Some(match self.problem.as_str() {
            "" => return None,
            "subscribe-not-https" => Problem::NotHttps,
            "subscribe-not-address" => Problem::NotAddress,
            "subscribe-not-public" => Problem::NotPublic,
            "subscribe-redirects" => Problem::Redirects,
            "subscribe-too-large" => Problem::TooLarge,
            "subscribe-too-many" => Problem::TooMany,
            "subscribe-too-large-kept" => Problem::TooLargeKept,
            "subscribe-cut" => Problem::Cut,
            "subscribe-no-event" => Problem::NoEvent,
            "subscribe-not-written" => Problem::NotWritten,
            "subscribe-page" => Problem::Page,
            "subscribe-text" => Problem::Text(self.said.clone()),
            "subscribe-empty" => Problem::Empty,
            "subscribe-gone" => Problem::Gone,
            "subscribe-status" => Problem::Status(self.status),
            "subscribe-network" => Problem::Network,
            _ => Problem::NotCalendar,
        })
    }

    /// When its copy was last found current, if it ever was (`checked`).
    pub fn as_of(&self) -> Option<i64> {
        (self.checked > 0).then_some(self.checked)
    }
}

/// What this device's state keeps of an address: a hash of it, never the address (F17).
pub fn address_of(url: &str) -> String {
    vdir::content_hash(url.trim().as_bytes())
}

/// The copy's folder of a subscribed calendar.
pub fn folder(id: &str) -> PathBuf {
    Kind::Calendars.root().join(ACCOUNT).join(id)
}

/// The server's validators to send (ETag, date), when the copy kept here
/// came from this address and is on the disk as it was written (`intact`):
/// else the whole calendar is asked for, and written again (F11).
pub fn validators(id: &str, subscription: &Subscription) -> Option<(String, String)> {
    let state = Fetched::load(id);
    (state.address == address_of(&subscription.url) && state.checked > 0 && intact(id, &state) && !(state.etag.is_empty() && state.last_modified.is_empty())).then_some((state.etag, state.last_modified))
}

/// Whether this device's copy is the one its state notes: its folder there,
/// holding as many events as were written, none of them empty. Not so after
/// a folder cleared by hand, a backup restored without it, or a power cut
/// that kept the renames and not the files' contents (F11, F12): then
/// "unchanged" no longer holds, and the next answer is written whole.
pub fn intact(id: &str, state: &Fetched) -> bool {
    let Ok(entries) = std::fs::read_dir(folder(id)) else { return false };
    let mut count = 0usize;
    for entry in entries.flatten() {
        if entry.path().extension().is_some_and(|x| x == "ics") {
            if entry.metadata().map_or(true, |m| m.len() == 0) {
                return false;
            }
            count += 1;
        }
    }
    count == state.events
}

/// How often a calendar is read on a computer: its own wish, between one
/// hour and one day; else once a day for holidays, every three hours for
/// the rest. After a problem, at growing intervals (F3): an hour, then
/// two, four, eight, sixteen, then once a day, never every hour for ever.
pub fn every(subscription: &Subscription, state: &Fetched) -> i64 {
    let usual = if state.refresh > 0 {
        state.refresh.clamp(SOONEST, LATEST)
    } else if subscription.holidays().is_some() {
        HOLIDAYS_EVERY
    } else {
        EVERY
    };
    if state.problem.is_empty() {
        return usual;
    }
    let doubled = SOONEST.saturating_mul(1i64 << state.failures.saturating_sub(1).min(16));
    doubled.min(LATEST)
}

/// Whether a calendar is to be read now: never read from this address
/// here; when Sioul opens (`opening`), unless read in the last ten minutes;
/// else once its time has come (`every`), and `at_least` seconds after the
/// last asking (a phone's few times a day, F23; 0 on a computer).
pub fn due(id: &str, subscription: &Subscription, now: i64, opening: bool, at_least: i64) -> bool {
    let state = Fetched::load(id);
    if state.address != address_of(&subscription.url) || state.tried == 0 {
        return true;
    }
    let since = now - state.tried;
    since < 0 || since >= if opening { OPENED_AGAIN } else { every(subscription, &state).max(at_least) }
}

/// What the network gave for a calendar (sioul-sync's `feeds`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Its bytes (ten megabytes at most once unpacked), with the server's validators.
    Calendar { bytes: Vec<u8>, etag: String, last_modified: String },
    /// The server said the copy kept is current (304).
    Unchanged,
    /// Nothing that may be read.
    Refused(Problem),
}

/// What became of an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taken {
    /// The copy replaced, whole.
    Changed,
    /// The copy is current: nothing written.
    Unchanged,
    /// The copy kept as it was, and why.
    Kept(Problem),
}

/// An answer taken in (18 November 2026): a calendar replaces the copy
/// whole when it says something else (`steady`) or came from another
/// address; one that says the same, or the server's 304, writes nothing;
/// anything else keeps the last good copy and says why, since when. Each
/// outcome noted in this device's state. `now`, Unix seconds.
pub fn take(id: &str, subscription: &Subscription, answer: Answer, now: i64) -> Result<Taken, String> {
    if !valid_id(id) {
        return Err(format!("{id}: not a subscribed calendar"));
    }
    vdir::finish_subscribed(id);
    let mut state = Fetched::load(id);
    let address = address_of(&subscription.url);
    // The copy of a former address (changed, or an id used again) never stands
    // for this one, whatever this one answers (F22): it goes now.
    if !state.address.is_empty() && state.address != address {
        if let Err(e) = vdir::replace_subscribed(id, &display_name(subscription, ""), subscription.colour.as_deref(), &[]) {
            eprintln!("subscribed calendar {id}: {e}");
        }
        state = Fetched { tried: state.tried, entry: state.entry, ..Fetched::default() };
    }
    state.address = address.clone();
    let same_address = folder(id).is_dir() && state.checked > 0;
    // "Unchanged" is judged against the copy on the disk, never this device's note alone (F11).
    let intact = same_address && intact(id, &state);
    state.tried = now;
    // Its entry as it reads now: what stands for it if it stops reading (F20).
    if !subscription.unread {
        state.entry = Some(Subscription { url: String::new(), ..subscription.clone() });
    }
    let refused = |mut state: Fetched, problem: Problem| -> Result<Taken, String> {
        if state.problem != problem.id() || state.since == 0 {
            state.since = now;
        }
        state.problem = problem.id().to_string();
        state.failures = state.failures.saturating_add(1);
        state.said = if let Problem::Text(said) = &problem { said.clone() } else { String::new() };
        state.status = if let Problem::Status(status) = problem { status } else { 0 };
        state.save(id)?;
        Ok(Taken::Kept(problem))
    };
    let clear = |state: &mut Fetched| {
        state.problem.clear();
        state.said.clear();
        state.status = 0;
        state.since = 0;
        state.failures = 0;
    };
    match answer {
        Answer::Refused(problem) => refused(state, problem),
        Answer::Unchanged if intact => {
            state.checked = now;
            clear(&mut state);
            state.save(id)?;
            Ok(Taken::Unchanged)
        }
        // A 304 for a copy that is not this address's, or no longer whole on the disk:
        // never asked so (`validators`); read whole again next time.
        Answer::Unchanged => refused(state, Problem::NotCalendar),
        Answer::Calendar { bytes, etag, last_modified } => {
            let feed = match read(&bytes) {
                Ok(feed) => feed,
                Err(problem) => return refused(state, problem),
            };
            // No event, where the copy of this address had some: it stays (F10;
            // a generator's bad day must not empty the holidays nor un-pause the series).
            if feed.items.is_empty() && same_address && state.events > 0 {
                return refused(state, Problem::NoEvent);
            }
            (state.etag, state.last_modified, state.refresh, state.name) = (etag, last_modified, feed.refresh.unwrap_or(0), feed.name.clone());
            clear(&mut state);
            if intact && feed.hash == state.hash {
                state.checked = now;
                state.save(id)?;
                return Ok(Taken::Unchanged);
            }
            let name = display_name(subscription, &feed.name);
            // Not written here (a full disk): said, and asked again later at growing
            // intervals, never at each minute (F4); the last copy stays.
            if let Err(e) = vdir::replace_subscribed(id, &name, subscription.colour.as_deref(), &feed.items) {
                eprintln!("subscribed calendar {id}: {e}");
                return refused(state, Problem::NotWritten);
            }
            (state.hash, state.changed, state.checked, state.events) = (feed.hash, now, now, feed.items.len());
            state.save(id)?;
            Ok(Taken::Changed)
        }
    }
}

/// A subscribed calendar's name: yours, else its own, else its address's domain.
pub fn display_name(subscription: &Subscription, own: &str) -> String {
    [subscription.name.trim(), own.trim()].into_iter().find(|n| !n.is_empty()).map_or_else(|| crate::sites::domain_of(&crate::sites::host_of(&subscription.url)), str::to_string)
}

// ---------------------------------------------------------------- each device's copies, as the settings say

/// Each device's folders made to follow the settings: a subscription's
/// folder made, read only, its name and colour kept as said (an empty copy
/// until it is first read here); a folder whose subscription was stopped,
/// here or on another device, taken out with its state. Whether anything changed.
pub fn settle(config: &Config) -> bool {
    let mut changed = false;
    for (id, subscription) in &config.agenda.subscriptions {
        let name = display_name(subscription, &Fetched::load(id).name);
        match vdir::prepare_subscribed(id, &name, subscription.colour.as_deref()) {
            Ok(made) => changed |= made,
            Err(e) => eprintln!("subscribed: {id}: {e}"),
        }
    }
    let here: Vec<String> = std::fs::read_dir(Kind::Calendars.root().join(ACCOUNT)).into_iter().flatten().filter_map(Result::ok).filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    for id in stopped(config, &here) {
        if vdir::remove_subscribed(&id).is_ok() {
            let _ = std::fs::remove_file(Fetched::path(&id));
            changed = true;
        }
    }
    changed
}

/// Which of the folders `here` were stopped: those the settings no longer
/// name at all. One whose entry does not read is named still, and stays
/// (F20); the caller asks only settings that read (F19).
pub fn stopped(config: &Config, here: &[String]) -> Vec<String> {
    here.iter().filter(|id| valid_id(id) && !config.agenda.subscriptions.contains_key(*id)).cloned().collect()
}

// ---------------------------------------------------------------- written into the settings

/// A new subscription, as the window gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct New {
    pub url: String,
    #[serde(default)]
    pub name: String,
    /// "public", "school" or "".
    #[serde(default)]
    pub holidays: String,
    #[serde(default)]
    pub days_off: Option<bool>,
    #[serde(default)]
    pub after_class: Option<bool>,
    /// The id to keep it under (an official calendar's), else one made from its name.
    #[serde(default)]
    pub id: String,
    /// Its events are your own busy time, ticked in the form as it is added
    /// (`Subscription::engagements`): a calendar of events alone; unsaid, no.
    #[serde(default)]
    pub engagements: Option<bool>,
}

/// A subscription written into the settings (`[agenda.subscriptions."<id>"]`),
/// its address checked first; its id, made from its name (or its domain),
/// one not taken; a calendar of events ticked as your own busy time written
/// so, as its line writes it (`engagements = true`), never one of holidays.
/// Fetched by each device by itself.
pub fn subscribe(path: &Path, config: &Config, new: &New, tr: &crate::i18n::Translator) -> Result<String, String> {
    let url = address(&new.url).map_err(|p| p.said(tr))?;
    // The same address twice would show every event twice.
    if config.agenda.subscriptions.values().any(|s| address(&s.url).is_ok_and(|u| u == url)) {
        return Err(tr.text("subscribe-taken", None));
    }
    let host = crate::sites::host_of(&url);
    let base = if valid_id(new.id.trim()) { new.id.trim().to_string() } else { crate::config::slug(if new.name.trim().is_empty() { &host } else { new.name.trim() }) };
    let base: String = if base.is_empty() { "calendar".to_string() } else { base.chars().take(60).collect() };
    let taken = |id: &str| config.agenda.subscriptions.contains_key(id) || folder(id).exists();
    let id = std::iter::once(base.clone()).chain((2..100).map(|n| format!("{base}-{n}"))).find(|id| !taken(id)).ok_or_else(|| format!("{base}: no free name"))?;
    let holidays = Holidays::parse(&new.holidays);
    // The Ministry's files begin after class: said so, unless said otherwise.
    let after_class = match holidays {
        Some(Holidays::School) => new.after_class.or(official_after_class(&url).then_some(true)),
        _ => None,
    };
    crate::agendas::edit_agenda_table(path, "subscriptions", |table| {
        let mut entry = toml_edit::Table::new();
        entry.insert("url", toml_edit::value(url.as_str()));
        if !new.name.trim().is_empty() {
            entry.insert("name", toml_edit::value(new.name.trim()));
        }
        if let Some(kind) = holidays {
            entry.insert("holidays", toml_edit::value(kind.id()));
            if let Some(days_off) = new.days_off {
                entry.insert("days_off", toml_edit::value(days_off));
            }
            if let Some(after) = after_class {
                entry.insert("after_class", toml_edit::value(after));
            }
        } else if new.engagements == Some(true) {
            entry.insert("engagements", toml_edit::value(true));
        }
        table.insert(&id, toml_edit::Item::Table(entry));
        Ok(())
    })?;
    Ok(id)
}

/// The fields of a subscription the window writes.
pub const FIELDS: [&str; 7] = ["url", "name", "holidays", "days_off", "after_class", "engagements", "colour"];

/// One field of a subscription written: its address (checked first), its
/// name, which holidays it holds, whether they count as your days off,
/// whether they begin after class, whether its events are your own
/// engagements, its colour; "" takes it back to unsaid
/// (never the address).
pub fn set(path: &Path, id: &str, field: &str, value: &str, tr: &crate::i18n::Translator) -> Result<(), String> {
    let value = value.trim();
    let written: Option<toml_edit::Item> = match field {
        "url" => Some(toml_edit::value(address(value).map_err(|p| p.said(tr))?)),
        _ if value.is_empty() => None,
        "name" => Some(toml_edit::value(value)),
        "holidays" => Some(toml_edit::value(Holidays::parse(value).ok_or_else(|| format!("{value}: public or school"))?.id())),
        "days_off" | "after_class" | "engagements" => Some(toml_edit::value(matches!(value, "true" | "yes" | "on" | "1"))),
        "colour" if value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit()) => Some(toml_edit::value(value.to_ascii_lowercase())),
        "colour" => return Err(format!("{value}: a colour as #rrggbb")),
        _ => return Err(format!("{field}: not a field of a subscribed calendar")),
    };
    crate::agendas::edit_agenda_table(path, "subscriptions", |table| {
        let entry = table.get_mut(id).and_then(toml_edit::Item::as_table_like_mut).ok_or_else(|| format!("{id}: no such subscription"))?;
        match written {
            Some(item) => {
                entry.insert(field, item);
            }
            None => {
                entry.remove(field);
            }
        }
        Ok(())
    })
}

/// A subscription stopped: taken out of the settings with what was said of
/// its agenda, then its copy here; every other device takes its own out
/// at its next look (`settle`).
pub fn stop(path: &Path, id: &str) -> Result<(), String> {
    crate::agendas::edit_agenda_table(path, "subscriptions", |table| {
        table.remove(id);
        Ok(())
    })?;
    crate::agendas::edit_agenda_table(path, "calendars", |table| {
        table.remove(&place(id));
        Ok(())
    })?;
    forget_place(path, &place(id))?;
    vdir::remove_subscribed(id)?;
    let _ = std::fs::remove_file(Fetched::path(id));
    Ok(())
}

/// A stopped calendar's place taken out of whatever names it in the
/// settings: the filter's sets, each availability's agendas, the pairs of
/// agendas that warn when they overlap.
fn forget_place(path: &Path, place: &str) -> Result<(), String> {
    use toml_edit::Item;
    crate::filelock::with_lock(path, || {
        let mut doc = crate::config::read_document(path)?;
        let Some(agenda) = doc.get_mut("agenda").and_then(Item::as_table_like_mut) else { return Ok(()) };
        let named = |value: &toml_edit::Value| value.as_str() == Some(place);
        if let Some(pairs) = agenda.get_mut("overlaps").and_then(Item::as_table_like_mut) {
            let gone: Vec<String> = pairs.iter().map(|(key, _)| key.to_string()).filter(|key| key.split('|').any(|p| p == place)).collect();
            for key in gone {
                pairs.remove(&key);
            }
        }
        if let Some(sets) = agenda.get_mut("sets").and_then(Item::as_table_like_mut) {
            for (_, set) in sets.iter_mut() {
                if let Some(list) = set.get_mut("agendas").and_then(Item::as_array_mut) {
                    list.retain(|v| !named(v));
                }
            }
        }
        if let Some(list) = agenda.get_mut("availabilities").and_then(Item::as_array_of_tables_mut) {
            for availability in list.iter_mut() {
                if let Some(agendas) = availability.get_mut("agendas").and_then(Item::as_array_mut) {
                    agendas.retain(|v| !named(v));
                }
            }
        }
        crate::config::write_document(path, &doc)
    })
}

// ---------------------------------------------------------------- the agenda's line

/// What a subscribed calendar's line says on this device: when it was last
/// read; never yet; or why the last asking gave nothing, since when, what
/// to do, the last copy kept.
pub fn line(id: &str, now: &jiff::Zoned, tr: &crate::i18n::Translator) -> String {
    line_of(&Fetched::load(id), now, tr)
}

/// A subscribed calendar's line when its entry in the settings does not
/// read here, or the settings themselves do not (F19, F20): said first,
/// then when it was last read.
pub fn line_unread(id: &str, whole_file: bool, now: &jiff::Zoned, tr: &crate::i18n::Translator) -> String {
    let said = tr.text(if whole_file { "subscribe-line-settings-unread" } else { "subscribe-line-entry-unread" }, None);
    format!("{said} {}", line(id, now, tr))
}

fn line_of(state: &Fetched, now: &jiff::Zoned, tr: &crate::i18n::Translator) -> String {
    let when = |at: i64| jiff::Timestamp::from_second(at).ok().map(|t| t.to_zoned(now.time_zone().clone())).map(|z| tr.date(&z, true)).unwrap_or_default();
    let mut args = crate::i18n::args();
    match state.problem() {
        None if state.checked > 0 => {
            args.set("when", when(state.checked));
            tr.text("subscribe-line-read", Some(&args))
        }
        None => tr.text("subscribe-line-never", None),
        Some(problem) => {
            args.set("why", problem.said(tr));
            args.set("since", when(state.since.max(state.tried)));
            // Since when the copy kept stands (F10).
            args.set("kept", if state.checked > 0 { "yes" } else { "no" });
            args.set("when", when(state.changed.max(state.checked)));
            tr.text(problem.line_id(), Some(&args))
        }
    }
}

// ---------------------------------------------------------------- the days of a calendar of holidays

/// The folders of the calendars of holidays that count as your days off:
/// their whole days are drawn as days off (`timeline::days_off`), in place
/// of their own events.
pub fn counted_folders(config: &Config) -> Vec<PathBuf> {
    config.agenda.subscriptions.iter().filter(|(_, s)| s.days_off()).map(|(id, _)| folder(id)).collect()
}

/// A calendar of holidays' days, as this device's copy holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HolidayDays {
    /// Its id, and its place among the agendas.
    pub id: String,
    pub name: String,
    pub holidays: Holidays,
    /// Its days count as your days off (`Subscription::days_off`).
    pub days_off: bool,
    /// Each holiday: its first and last day, both included, and its name.
    /// School holidays that begin after class start the day after their
    /// event's first day and end before the morning classes resume; an event
    /// of no day (the Ministry's zero-length ones) gives none.
    pub periods: Vec<(Date, Date, String)>,
    /// When this device last found its copy current; none before its first read here.
    pub as_of: Option<i64>,
}

/// The days a calendar of holidays is read for, the same on every device
/// and every day (`HOLIDAYS_FROM` to `HOLIDAYS_UNTIL`, years): the days a
/// copy gives are its version (`series::Version::Held`), which must not
/// depend on the day it is read; and a holiday counted as a day off shows
/// as far as the agenda shows (the review's F25).
pub const HOLIDAYS_FROM: i16 = 1970;
pub const HOLIDAYS_UNTIL: i16 = 2200;

/// Every calendar of holidays subscribed, with its days (`HOLIDAYS_FROM` to
/// `HOLIDAYS_UNTIL`), from this device's copy, in `zone`; read again only
/// when its copy or its state changed (two looks at their times, never a
/// listing nor a parse: asked by every question "is this a day off", F28).
pub fn holidays(config: &Config, zone: &jiff::tz::TimeZone) -> Vec<HolidayDays> {
    type Key = (Option<std::time::SystemTime>, Option<std::time::SystemTime>, bool, String);
    type Seen = BTreeMap<String, (Key, Vec<(Date, Date, String)>, String, Option<i64>)>;
    static SEEN: std::sync::Mutex<Seen> = std::sync::Mutex::new(BTreeMap::new());
    let modified = |path: &Path| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let mut out = Vec::new();
    for (id, subscription) in &config.agenda.subscriptions {
        let Some(kind) = subscription.holidays() else { continue };
        let dir = folder(id);
        // Between the replacement's two renames there is no copy: waited for, once (F13).
        if !dir.is_dir() {
            vdir::finish_subscribed(id);
        }
        let key: Key = (modified(&dir), modified(&Fetched::path(id)), subscription.after_class(), crate::zone::name_of(zone, jiff::Timestamp::now()));
        let known = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(id).filter(|(k, ..)| *k == key).map(|(_, p, name, as_of)| (p.clone(), name.clone(), *as_of));
        let (periods, own_name, as_of) = known.unwrap_or_else(|| {
            let state = Fetched::load(id);
            let files: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ics")).collect();
            let periods = holiday_periods(&dir, &files, subscription.after_class(), zone);
            SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id.clone(), (key, periods.clone(), state.name.clone(), state.as_of()));
            let as_of = state.as_of();
            (periods, state.name, as_of)
        });
        out.push(HolidayDays { id: id.clone(), name: display_name(subscription, &own_name), holidays: kind, days_off: subscription.days_off(), periods, as_of });
    }
    out
}

/// The holidays a copy holds: each event's whole days (a timed one, the
/// days it covers whole), cancelled ones aside.
fn holiday_periods(dir: &Path, files: &[PathBuf], after_class: bool, zone: &jiff::tz::TimeZone) -> Vec<(Date, Date, String)> {
    let Some(collection) = vdir::collections(Kind::Calendars).into_iter().find(|c| c.dir == dir) else { return Vec::new() };
    let at = |year: i16| Date::new(year, 1, 1).ok().and_then(|d| d.to_zoned(zone.clone()).ok()).map_or(0, |z| z.timestamp().as_second());
    let (from, to) = (at(HOLIDAYS_FROM), at(HOLIDAYS_UNTIL));
    let mut out = Vec::new();
    let shaped = |first: Date, after: Date| if after_class { (first.tomorrow().unwrap_or(first), after) } else { (first, after) };
    for file in files {
        // Whole days that do not repeat (every holidays' file Sioul knows): their own
        // dates, as written; the calendar library gives none for a day whose midnight
        // does not exist (Santiago, Havana, Beirut some years: F29).
        if let Some(days) = std::fs::read_to_string(file).ok().and_then(|text| whole_days(&text)) {
            for (first, after, name) in days {
                let (first, after) = shaped(first, after);
                if let Ok(last) = after.yesterday()
                    && last >= first
                {
                    out.push((first, last, name));
                }
            }
            continue;
        }
        for event in crate::agenda::file_occurrences(file, &collection, from, to, zone) {
            if event.cancelled {
                continue;
            }
            let day = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()));
            let (Some(start), Some(end)) = (day(event.start), day(event.end)) else { continue };
            // Its first whole day and the day after its last (exclusive). A whole
            // day's are its own dates, read at its middle: its start is an hour off
            // its midnight where that midnight does not exist (Santiago, Havana,
            // Beirut some years: F29), or where the calendar library's time zone
            // rules and Sioul's differ by an hour (a far year).
            let noon = |at: &jiff::Zoned| at.checked_add(jiff::Span::new().hours(12)).map_or(at.date(), |z| z.date());
            let first = if event.all_day { noon(&start) } else if start.time() == jiff::civil::Time::midnight() { start.date() } else { start.date().tomorrow().unwrap_or(start.date()) };
            let after = if event.all_day { noon(&end) } else { end.date() };
            let (first, after) = shaped(first, after);
            let Ok(last) = after.yesterday() else { continue };
            if last >= first {
                out.push((first, last, event.summary.trim().to_string()));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// A file's whole-day events, each its first day, the day after its last and
/// its name, read from their own lines; none when one of its events has a
/// time, repeats, or names a time of a series (those are read by expansion).
/// One cancelled is left out; one of no length is a day, as the agenda shows it.
fn whole_days(text: &str) -> Option<Vec<(Date, Date, String)>> {
    let lines = crate::lines::unfold(text);
    let mut out = Vec::new();
    let (mut inside, mut nested) = (false, 0usize);
    let (mut start, mut end, mut length, mut name, mut cancelled) = (None, None, None, String::new(), false);
    let date = |line: &str| Date::strptime("%Y%m%d", crate::lines::value(line).trim()).ok();
    for line in &lines {
        let field = crate::lines::name(line);
        let value = crate::lines::value(line).trim().to_ascii_uppercase();
        if !inside {
            if field == "BEGIN" && value == "VEVENT" {
                (inside, nested, start, end, length, name, cancelled) = (true, 0, None, None, None, String::new(), false);
            }
            continue;
        }
        match field.as_str() {
            "BEGIN" => nested += 1,
            "END" if nested > 0 => nested -= 1,
            "END" => {
                inside = false;
                let first: Date = start?;
                let after = end.or_else(|| length.and_then(|days: i64| first.checked_add(jiff::Span::new().days(days)).ok())).filter(|a| *a > first).unwrap_or(first.tomorrow().ok()?);
                if !cancelled {
                    out.push((first, after, name.clone()));
                }
            }
            _ if nested > 0 => {}
            "RRULE" | "RDATE" | "RECURRENCE-ID" | "EXRULE" => return None,
            "DTSTART" => start = Some(date(line)?),
            "DTEND" => end = Some(date(line)?),
            "DURATION" => length = Some(value.strip_prefix('P').and_then(|d| d.strip_suffix('D').and_then(|n| n.parse().ok()).or_else(|| d.strip_suffix('W').and_then(|n| n.parse::<i64>().ok()).map(|w| w * 7)))?),
            "STATUS" => cancelled = value == "CANCELLED",
            "SUMMARY" => name = crate::lines::unescape(crate::lines::value(line)).trim().to_string(),
            _ => {}
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests;
