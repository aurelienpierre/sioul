// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! An office's opening hours, on its card, and the hours a task that needs
//! an open office follows (design of 20 November 2026; docs/tasks.md,
//! "Offices' hours"; docs/compatibility.md, the card's lines).
//!
//! **On the card**, written once, beside the number to call: lines of Sioul's
//! own, since vCard has no property for hours, each value in OpenStreetMap's
//! `opening_hours` syntax (wiki.openstreetmap.org/wiki/Key:opening_hours),
//! the part of it Sioul can count (weekly ranges, `PH off`):
//!
//! ```text
//! X-SIOUL-OPENING-HOURS;X-SIOUL-SOURCE=typed;X-SIOUL-CHECKED=2026-11-20:Mo-Fr 09:00-12:00\,14:00-17:00\; PH off
//! X-SIOUL-PHONE-HOURS;X-SIOUL-SOURCE=typed;X-SIOUL-CHECKED=2026-11-20:Mo-Th 09:00-12:00
//! X-SIOUL-HOURS-NOTE:Closed in August
//! ```
//!
//! The phone's hours only when they differ; the free line for what Sioul
//! cannot count (seasons, appointments): shown, never computed. Other
//! programs keep these lines as they keep any `X-` property; an older Sioul
//! keeps them untouched when it writes the card (`contacts::apply` changes
//! only its own fields).
//!
//! **A task follows**, the most specific first (`hours_for`, the one function
//! every reader asks: the plan, the day, Now, the task's panel): the hours
//! typed on the task itself (`X-SIOUL-OFFICE-HOURS`); else those of the first
//! office it involves (`CONTACT`, its card by UID) whose card has hours, the
//! phone's for a call when the card has them; else the usual hours of Tasks ⚙.
//! Nothing is copied to the task: changing the card changes every task of
//! that office. An office is closed on the public holidays Sioul knows
//! (Settings ▸ Hours, the subscribed calendars of public holidays), unless
//! its hours say otherwise.

use crate::window::{self, AdminWindow};
use jiff::Zoned;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The office's opening hours.
pub const OPEN: &str = "X-SIOUL-OPENING-HOURS";
/// When it answers the phone, only when that differs.
pub const PHONE: &str = "X-SIOUL-PHONE-HOURS";
/// What Sioul cannot count, as a sentence.
pub const NOTE: &str = "X-SIOUL-HOURS-NOTE";
/// Where a set of hours comes from (a parameter of its line).
pub const SOURCE: &str = "X-SIOUL-SOURCE";
/// The day they were typed or checked, "2026-11-20" (a parameter of its line).
pub const CHECKED: &str = "X-SIOUL-CHECKED";

/// Hours typed by the person.
pub const TYPED: &str = "typed";
/// Hours from the French administration's directory (the DILA's).
pub const DIRECTORY: &str = "service-public.gouv.fr";
/// Hours from OpenStreetMap, through Nominatim.
pub const MAP: &str = "openstreetmap.org";

/// Hours checked longer ago than this say so, quietly, on the card.
pub const STALE_DAYS: i64 = 365;

/// One set of hours, as a card keeps it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hours {
    /// In OpenStreetMap's syntax, unescaped: "Mo-Fr 09:00-12:00,14:00-17:00; PH off".
    pub value: String,
    /// Where they come from: `TYPED`, `DIRECTORY`, `MAP`, or another word as written; "" unsaid.
    #[serde(default)]
    pub source: String,
    /// The day they were typed or checked, "2026-11-20"; "" unsaid.
    #[serde(default)]
    pub checked: String,
}

impl Hours {
    /// What Sioul counts of them.
    pub fn read(&self) -> Parsed {
        parse(&self.value)
    }

    /// Whether they hold any hours Sioul counts.
    pub fn counts(&self) -> bool {
        !self.read().windows.is_empty()
    }

    /// The day they were checked, when it reads as one.
    pub fn checked_on(&self) -> Option<Date> {
        self.checked.trim().parse().ok()
    }

    /// Their line on a card: `name;X-SIOUL-SOURCE=…;X-SIOUL-CHECKED=…:value`.
    fn line(&self, name: &str) -> String {
        let mut head = name.to_string();
        if !self.source.trim().is_empty() {
            head.push_str(&format!(";{SOURCE}={}", crate::lines::param_value(self.source.trim())));
        }
        if self.checked_on().is_some() {
            head.push_str(&format!(";{CHECKED}={}", self.checked.trim()));
        }
        format!("{head}:{}", crate::lines::escape(self.value.trim()))
    }

    fn of_line(line: &str) -> Hours {
        Hours {
            value: crate::lines::unescape(crate::lines::value(line)).trim().to_string(),
            source: crate::lines::param(line, SOURCE).unwrap_or_default().trim().to_string(),
            checked: crate::lines::param(line, CHECKED).unwrap_or_default().trim().to_string(),
        }
    }
}

/// What a card says of its hours: when it is open, when it answers the
/// phone (only when that differs), and the free line.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardHours {
    #[serde(default)]
    pub open: Option<Hours>,
    #[serde(default)]
    pub phone: Option<Hours>,
    #[serde(default)]
    pub note: String,
}

impl CardHours {
    /// Read from a card's lines (unfolded): the first line of each name.
    pub fn of(card: &[String]) -> CardHours {
        let first = |name: &str| card.iter().find(|l| crate::lines::name(l) == name);
        let hours = |name: &str| first(name).map(|l| Hours::of_line(l)).filter(|h| !h.value.is_empty());
        CardHours { open: hours(OPEN), phone: hours(PHONE), note: first(NOTE).map(|l| crate::lines::unescape(crate::lines::value(l)).trim().to_string()).unwrap_or_default() }
    }

    /// Nothing at all: no hours, no free line.
    pub fn is_empty(&self) -> bool {
        self.open.is_none() && self.phone.is_none() && self.note.trim().is_empty()
    }

    /// Whether it holds hours Sioul counts, for a call or not.
    pub fn counts(&self) -> bool {
        self.followed(true).is_some()
    }

    /// The hours a task follows: the phone's for a call when they count,
    /// else the opening hours when they count; and whether they are the phone's.
    pub fn followed(&self, call: bool) -> Option<(&Hours, bool)> {
        let phone = self.phone.as_ref().filter(|h| call && h.counts()).map(|h| (h, true));
        phone.or_else(|| self.open.as_ref().filter(|h| h.counts()).map(|h| (h, false)))
    }

    /// Its lines on a card, in this order: open, phone, the free line.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(open) = self.open.as_ref().filter(|h| !h.value.trim().is_empty()) {
            out.push(open.line(OPEN));
        }
        if let Some(phone) = self.phone.as_ref().filter(|h| !h.value.trim().is_empty()) {
            out.push(phone.line(PHONE));
        }
        if !self.note.trim().is_empty() {
            out.push(format!("{NOTE}:{}", crate::lines::escape(self.note.trim())));
        }
        out
    }

    /// Whether a card's line is one of these.
    pub fn is_line(line: &str) -> bool {
        let name = crate::lines::name(line);
        name == OPEN || name == PHONE || name == NOTE
    }
}

/// What Sioul counts of a set of hours.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Each day's ranges, as the usual hours are kept.
    pub windows: Vec<AdminWindow>,
    /// Closed on public holidays: assumed for an office, unless its hours
    /// give public holidays hours of their own (then that rule is in `rest`).
    pub holidays_off: bool,
    /// What it could not count, each part as written ("Aug off", "week 1-53/2 Mo 09:00-12:00"),
    /// a comment without its quotes ("by appointment"). A value Sioul does
    /// not understand at all is here whole, and counts as no hours.
    pub rest: Vec<String>,
}

/// One rule's pieces: days (none said: every day), its time ranges, or closed.
#[derive(Debug, Default)]
struct Segment {
    days: Option<Vec<usize>>,
    times: Vec<(String, String)>,
    off: bool,
}

#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    Comma,
    Comment(String),
}

/// A rule cut into words, commas and quoted comments.
fn tokens(rule: &str) -> Option<Vec<Token>> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut chars = rule.chars();
    let flush = |word: &mut String, out: &mut Vec<Token>| {
        if !word.is_empty() {
            out.push(Token::Word(std::mem::take(word)));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                flush(&mut word, &mut out);
                let mut comment = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some(c) => comment.push(c),
                        // A comment never closed: not a value Sioul reads.
                        None => return None,
                    }
                }
                out.push(Token::Comment(comment.trim().to_string()));
            }
            ',' => {
                flush(&mut word, &mut out);
                out.push(Token::Comma);
            }
            c if c.is_whitespace() => flush(&mut word, &mut out),
            c => word.push(c),
        }
    }
    flush(&mut word, &mut out);
    Some(out)
}

/// "Mo-Fr", "lu-ve", "Sa", "lundi": the days named, Monday 0; a range across the week's end wraps.
fn days_of(word: &str) -> Option<Vec<usize>> {
    match word.split_once('-') {
        Some((from, to)) => {
            let (from, to) = (window::day_index(from)?, window::day_index(to)?);
            let mut days = vec![from];
            let mut day = from;
            while day != to {
                day = (day + 1) % 7;
                days.push(day);
            }
            Some(days)
        }
        None => window::day_index(word).map(|d| vec![d]),
    }
}

/// "09:00-12:00", "9h-12h", "9-12": a range of one day; none past midnight or empty.
fn range_of(word: &str) -> Option<(String, String)> {
    let (start, end) = word.split_once('-')?;
    let (start, end) = (window::tidy_time(start)?, window::tidy_time(end)?);
    (start < end && start.as_str() < "24:00" && end.as_str() <= "24:00").then_some((start, end))
}

/// One rule of `;`, its segments (a comma before days starts another one,
/// added to the first), its comment; None when Sioul cannot count it.
fn rule_of(text: &str) -> Option<(Vec<Segment>, Option<String>)> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut current = Segment::default();
    let mut comment = None;
    for token in tokens(text)? {
        match token {
            Token::Comma => {}
            Token::Comment(c) => comment = Some(c).filter(|c| !c.is_empty()),
            Token::Word(word) => {
                let lower = word.to_lowercase();
                if lower == "off" || lower == "closed" {
                    current.off = true;
                } else if lower == "24/7" {
                    current.days = Some((0..7).collect());
                    current.times.push(("00:00".into(), "24:00".into()));
                } else if let Some(days) = days_of(&word) {
                    if !current.times.is_empty() || current.off {
                        segments.push(std::mem::take(&mut current));
                    }
                    current.days.get_or_insert_with(Vec::new).extend(days);
                } else if let Some(range) = range_of(&word) {
                    if current.off {
                        return None;
                    }
                    current.times.push(range);
                } else {
                    return None;
                }
            }
        }
    }
    if current.days.is_some() || !current.times.is_empty() || current.off {
        segments.push(current);
    }
    // Days alone ("Mo-Fr"), with no hours nor "off", say nothing Sioul would not guess.
    if segments.iter().any(|s| s.times.is_empty() && !s.off) {
        return None;
    }
    (!segments.is_empty() || comment.is_some()).then_some((segments, comment))
}

/// Opening hours in OpenStreetMap's syntax, or in the short form typed on a
/// task ("mo-fr 9-12, 14-17", French days too), read: each rule of `;`
/// overrides the days it names, as the syntax says ("Mo-Fr 09:00-17:00; We
/// 09:00-12:00" closes Wednesday afternoon); `off` closes them; `PH off`
/// says public holidays are closed. What it cannot count goes to `rest`, as
/// written; a value it does not understand at all is kept whole there, and
/// counts as no hours: never guessed.
pub fn parse(text: &str) -> Parsed {
    let text = text.trim();
    let mut out = Parsed { holidays_off: true, ..Parsed::default() };
    if text.is_empty() {
        return out;
    }
    let mut week: [Vec<(String, String)>; 7] = Default::default();
    let mut counted = false;
    // A fallback rule ("||") is never counted: its part goes to the free line.
    let (main, fallback) = match text.split_once("||") {
        Some((main, fallback)) => (main, Some(fallback.trim())),
        None => (text, None),
    };
    for rule in split_rules(main) {
        let rule = rule.trim();
        if rule.is_empty() {
            continue;
        }
        let lower = rule.to_lowercase();
        if lower == "ph off" || lower == "ph closed" {
            out.holidays_off = true;
            counted = true;
            continue;
        }
        if lower.starts_with("ph ") || lower.starts_with("ph,") {
            // Hours of their own on public holidays: shown, not counted; not closed then.
            out.holidays_off = false;
            out.rest.push(rule.to_string());
            continue;
        }
        let Some((segments, comment)) = rule_of(rule) else {
            out.rest.push(rule.to_string());
            continue;
        };
        for (i, segment) in segments.iter().enumerate() {
            let days = segment.days.clone().unwrap_or_else(|| (0..7).collect());
            for day in days {
                // The rule's first part overrides those days; a part after a comma adds to them.
                if i == 0 || segment.off {
                    week[day].clear();
                }
                for range in &segment.times {
                    if !week[day].contains(range) {
                        week[day].push(range.clone());
                    }
                }
            }
            counted = true;
        }
        out.rest.extend(comment);
    }
    out.rest.extend(fallback.filter(|f| !f.is_empty()).map(str::to_string));
    if !counted {
        return Parsed { windows: Vec::new(), holidays_off: true, rest: vec![text.to_string()] };
    }
    for (day, ranges) in week.iter_mut().enumerate() {
        ranges.sort();
        for (start, end) in ranges.iter() {
            out.windows.push(AdminWindow { day: window::NAMES[day].into(), start: start.clone(), end: Some(end.clone()), minutes: 0, kind: None });
        }
    }
    out
}

/// A value's rules, cut at the semicolons outside quotes.
fn split_rules(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut quoted) = (0, false);
    for (i, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&text[start..]);
    out
}

/// Hours written in OpenStreetMap's syntax, days with the same hours
/// together: "Mo-Fr 09:00-12:00,14:00-17:00; Sa 09:00-12:00; PH off".
pub fn written(windows: &[AdminWindow], holidays_off: bool) -> String {
    let short = window::ranges_text(windows);
    let mut parts: Vec<String> = short
        .split("; ")
        .filter(|p| !p.is_empty())
        .map(|part| {
            let (days, hours) = part.split_once(' ').unwrap_or((part, ""));
            let days: String = days.split(['-', ',']).map(capitalised).collect::<Vec<_>>().join(if days.contains('-') { "-" } else { "," });
            format!("{days} {}", hours.replace(", ", ","))
        })
        .collect();
    if holidays_off && !parts.is_empty() {
        parts.push("PH off".into());
    }
    parts.join("; ")
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// A value as Sioul writes it, and the free line's part of it: what it
/// counts in OpenStreetMap's syntax, `PH off` when public holidays are
/// closed; what it does not, joined by "; " for the free line.
pub fn tidied(text: &str) -> (String, String) {
    let parsed = parse(text);
    (written(&parsed.windows, parsed.holidays_off), parsed.rest.join("; "))
}

/// The public holidays Sioul knows, from `from` to `until` (Settings ▸ Hours:
/// the subscribed calendars of public holidays, whether they count as your
/// days off or not): the days an office is closed.
pub fn public_holidays(config: &crate::config::Config, zone: &jiff::tz::TimeZone, from: Date, until: Date) -> BTreeSet<Date> {
    let mut days = BTreeSet::new();
    for calendar in crate::subscribed::holidays(config, zone).iter().filter(|h| h.holidays == crate::subscribed::Holidays::Public) {
        for (first, last, _) in &calendar.periods {
            let mut day = (*first).max(from);
            while day <= *last && day <= until {
                days.insert(day);
                let Ok(next) = day.tomorrow() else { break };
                day = next;
            }
        }
    }
    days
}

/// Whether hours are open at `now`: until when.
pub fn open_until(windows: &[AdminWindow], holidays_off: bool, holidays: &BTreeSet<Date>, now: &Zoned) -> Option<Zoned> {
    let (opening, closing) = window::current(windows, now)?;
    (!(holidays_off && holidays.contains(&opening.date()))).then_some(closing)
}

/// When hours open next after `now`, within the coming fortnight (a public holiday skipped).
pub fn next_opening(windows: &[AdminWindow], holidays_off: bool, holidays: &BTreeSet<Date>, now: &Zoned) -> Option<Zoned> {
    (0..16)
        .filter_map(|d| now.date().checked_add(jiff::Span::new().days(d)).ok())
        .filter(|date| !(holidays_off && holidays.contains(date)))
        .flat_map(|date| window::spans_on(windows, date, now.time_zone()).into_iter().map(|(opening, _)| opening))
        .find(|opening| opening.timestamp() > now.timestamp())
}

/// Whether hours open on `date` at all, its public holiday aside.
pub fn open_on(windows: &[AdminWindow], holidays_off: bool, holidays: &BTreeSet<Date>, date: Date) -> bool {
    window::open_day(windows, date) && !(holidays_off && holidays.contains(&date))
}

/// An office whose card has hours, as the tasks that involve it read it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Office {
    pub uid: String,
    /// The name its card is shown by.
    pub name: String,
    /// Its card's file, to open it.
    pub key: String,
    /// Its first number, to call it from a task.
    pub phone: String,
    pub hours: CardHours,
}

/// The offices' cards with hours, by UID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cards(BTreeMap<String, Office>);

impl Cards {
    /// The cards among `contacts` with hours Sioul counts.
    pub fn of(contacts: &[crate::contacts::Contact]) -> Cards {
        Cards(
            contacts
                .iter()
                .filter(|c| !c.uid.trim().is_empty() && c.hours.counts())
                .map(|c| (c.uid.trim().to_string(), Office { uid: c.uid.trim().to_string(), name: c.name.clone(), key: c.key.clone(), phone: c.phones.first().map(|p| p.value.clone()).unwrap_or_default(), hours: c.hours.clone() }))
                .collect(),
        )
    }

    /// The office a task's `CONTACT` points at ("sioul:contact/<UID>"), when its card has hours.
    pub fn get(&self, uri: &str) -> Option<&Office> {
        (crate::links::kind_of(uri) == crate::links::Kind::Contact).then(|| self.0.get(crate::links::id_of(uri).trim())).flatten()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Whose hours a task follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "from", rename_all = "kebab-case")]
pub enum Whose {
    /// Its own, typed on it (`X-SIOUL-OFFICE-HOURS`).
    Own,
    /// Those of an office's card; `phone`: the hours it answers the phone.
    Card { uid: String, name: String, key: String, number: String, phone: bool },
    /// The usual hours of Tasks ⚙ ("When offices are open").
    Usual,
}

/// The hours a task follows, and whose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Followed {
    pub windows: Vec<AdminWindow>,
    pub holidays_off: bool,
    pub whose: Whose,
}

impl Followed {
    /// Open at `now`: until when.
    pub fn open_until(&self, holidays: &BTreeSet<Date>, now: &Zoned) -> Option<Zoned> {
        open_until(&self.windows, self.holidays_off, holidays, now)
    }

    pub fn next_opening(&self, holidays: &BTreeSet<Date>, now: &Zoned) -> Option<Zoned> {
        next_opening(&self.windows, self.holidays_off, holidays, now)
    }

    /// Its spans on `date`, a public holiday closed.
    pub fn spans_on(&self, holidays: &BTreeSet<Date>, date: Date, zone: &jiff::tz::TimeZone) -> Vec<(Zoned, Zoned)> {
        if self.holidays_off && holidays.contains(&date) {
            return Vec::new();
        }
        window::spans_on(&self.windows, date, zone)
    }
}

/// The hours a task that needs an open office follows, the most specific
/// first: its own; else the first office it involves whose card has hours
/// (the phone's for a call, when the card has them); else `usual`. Every
/// reader asks here: the plan, the day, Now, the task's panel.
pub fn hours_for(task: &crate::tasks::Task, cards: &Cards, usual: &[AdminWindow]) -> Followed {
    let own = window::parse_ranges(&task.office_times);
    if !own.is_empty() {
        return Followed { windows: own, holidays_off: true, whose: Whose::Own };
    }
    let call = task.kind == "call";
    for office in task.contacts.iter().filter_map(|c| cards.get(&c.uri)) {
        if let Some((hours, phone)) = office.hours.followed(call) {
            let parsed = hours.read();
            return Followed { windows: parsed.windows, holidays_off: parsed.holidays_off, whose: Whose::Card { uid: office.uid.clone(), name: office.name.clone(), key: office.key.clone(), number: office.phone.clone(), phone } };
        }
    }
    Followed { windows: usual.to_vec(), holidays_off: true, whose: Whose::Usual }
}

/// The kinds of task that need their office open when it has hours: a call, going out.
pub const OFFICE_KINDS: &[&str] = &["call", "out"];

/// Whether a task is to be ticked "Needs an open office" by itself (decided
/// 20 November 2026): a call or an outing, not ticked, that now involves an
/// office with hours that `before` (its offices as its file had them) did
/// not. What Sioul sets counts until corrected: it is never set again for an
/// office already there, so an untick holds.
pub fn ticks(kind: &str, ticked: bool, before: &[crate::tasks::ContactRef], after: &[crate::tasks::ContactRef], cards: &Cards) -> bool {
    if ticked || !OFFICE_KINDS.contains(&kind) {
        return false;
    }
    let had = |uri: &str| before.iter().any(|b| crate::links::id_of(&b.uri).trim() == crate::links::id_of(uri).trim());
    after.iter().any(|c| !had(&c.uri) && cards.get(&c.uri).is_some())
}

/// "9:00", "14:30".
pub fn clock(at: &Zoned) -> String {
    format!("{}:{:02}", at.hour(), at.minute())
}

/// What is true now, in a sentence: "Open until 12:00", "Closed: opens
/// Monday at 9:00", "Closed for the public holiday: opens tomorrow at 9:00";
/// with `office`, the task's line: "Tax office: open until 12:00". "" for
/// hours Sioul cannot count.
pub fn now_line(followed: &Followed, holidays: &BTreeSet<Date>, now: &Zoned, office: Option<&str>, tr: &crate::i18n::Translator) -> String {
    if followed.windows.is_empty() {
        return String::new();
    }
    let say = |id: &str, pairs: &[(&str, String)]| {
        let mut args = fluent_bundle::FluentArgs::new();
        for (name, value) in pairs {
            args.set(*name, value.clone());
        }
        if let Some(office) = office {
            args.set("office", office.to_string());
        }
        tr.text(id, Some(&args))
    };
    let lead = if office.is_some() { "hours-office-" } else { "hours-" };
    if let Some(until) = followed.open_until(holidays, now) {
        return say(&format!("{lead}open"), &[("time", clock(&until))]);
    }
    let holiday = followed.holidays_off && holidays.contains(&now.date()) && window::open_day(&followed.windows, now.date());
    let closed = if holiday { format!("{lead}closed-holiday") } else { format!("{lead}closed") };
    let Some(next) = followed.next_opening(holidays, now) else { return say(&format!("{lead}closed-for-now"), &[]) };
    let days = (next.date() - now.date()).get_days();
    let opens = match days {
        0 => say("hours-opens-today", &[("time", clock(&next))]),
        1 => say("hours-opens-tomorrow", &[("time", clock(&next))]),
        2..=6 => say("hours-opens-day", &[("day", tr.text(&format!("weekday-{}", next.weekday().to_monday_one_offset()), None)), ("time", clock(&next))]),
        _ => say("hours-opens-date", &[("date", tr.day(next.date())), ("time", clock(&next))]),
    };
    say(&closed, &[("opens", opens)])
}

/// The hours in words, one line per run of days with the same hours:
/// "Monday to Friday: 9:00–12:00, 14:00–17:00", "Saturday: 9:00–12:00".
pub fn day_lines(windows: &[AdminWindow], tr: &crate::i18n::Translator) -> Vec<String> {
    let mut hours: Vec<Vec<String>> = vec![Vec::new(); 7];
    for w in windows {
        if let Some(day) = w.weekday() {
            let time = |t: &str| t.strip_prefix('0').filter(|t| t.len() == 4).unwrap_or(t).to_string();
            let range = format!("{}–{}", time(w.start.trim()), time(&w.end_text()));
            let slot = &mut hours[day.to_monday_zero_offset() as usize];
            if !slot.contains(&range) {
                slot.push(range);
            }
        }
    }
    let name = |d: usize| tr.text(&format!("weekday-{}", d + 1), None);
    let mut out = Vec::new();
    let mut day = 0;
    while day < 7 {
        if hours[day].is_empty() {
            day += 1;
            continue;
        }
        let mut last = day;
        while last + 1 < 7 && hours[last + 1] == hours[day] {
            last += 1;
        }
        let mut args = fluent_bundle::FluentArgs::new();
        let days = if last == day {
            name(day)
        } else {
            let mut range = fluent_bundle::FluentArgs::new();
            range.set("from", name(day));
            range.set("to", name(last));
            tr.text("hours-days-range", Some(&range))
        };
        args.set("days", capitalised(&days));
        args.set("hours", hours[day].join(", "));
        out.push(tr.text("hours-day-line", Some(&args)));
        day = last + 1;
    }
    out
}

/// Where hours come from and when they were checked: "Typed by you on 20
/// November 2026", "From service-public.gouv.fr, checked on 20 November 2026".
pub fn source_line(hours: &Hours, tr: &crate::i18n::Translator) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    let day = hours.checked_on().map(|d| format!("{} {}", tr.day_month(d), d.year())).unwrap_or_default();
    args.set("date", day.clone());
    args.set("source", hours.source.clone());
    let id = match (hours.source.as_str(), day.is_empty()) {
        (TYPED | "", true) => "hours-source-typed",
        (TYPED | "", false) => "hours-source-typed-on",
        (DIRECTORY, false) => "hours-source-directory",
        (MAP, false) => "hours-source-map",
        (_, true) => "hours-source-other",
        (_, false) => "hours-source-other-on",
    };
    tr.text(id, Some(&args))
}

/// Whether hours were checked more than a year before `today`.
pub fn stale(hours: &Hours, today: Date) -> bool {
    hours.checked_on().is_some_and(|d| i64::from((today - d).get_days()) > STALE_DAYS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{ContactRef, Task};

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    fn short(parsed: &Parsed) -> String {
        window::ranges_text(&parsed.windows)
    }

    #[test]
    fn the_part_sioul_counts() {
        let p = parse("Mo-Fr 09:00-12:00,14:00-17:00; Sa 09:00-12:00; PH off");
        assert_eq!(short(&p), "mo-fr 09:00-12:00, 14:00-17:00; sa 09:00-12:00");
        assert!(p.holidays_off && p.rest.is_empty());
        assert_eq!(written(&p.windows, p.holidays_off), "Mo-Fr 09:00-12:00,14:00-17:00; Sa 09:00-12:00; PH off");
        // The short form typed, French days and "9h" included.
        assert_eq!(short(&parse("lu-ve 9h-12h, 14-17")), "mo-fr 09:00-12:00, 14:00-17:00");
        assert_eq!(short(&parse("mo-fr 9-12, 14-17")), "mo-fr 09:00-12:00, 14:00-17:00");
        // A later rule overrides the days it names; "off" closes them.
        assert_eq!(short(&parse("Mo-Fr 08:00-18:00; We 08:00-12:00")), "mo-tu 08:00-18:00; we 08:00-12:00; th-fr 08:00-18:00");
        assert_eq!(short(&parse("Mo-Sa 09:00-17:00; Sa off")), "mo-fr 09:00-17:00");
        // A comma before days adds a rule to the first.
        assert_eq!(short(&parse("Mo-Fr 09:00-12:00, Sa 10:00-11:00")), "mo-fr 09:00-12:00; sa 10:00-11:00");
        assert_eq!(short(&parse("Mo,We,Fr 10:00-11:30")), "mo 10:00-11:30; we 10:00-11:30; fr 10:00-11:30");
        assert_eq!(short(&parse("24/7")), "mo-su 00:00-24:00");
        // Hours with no days: every day, as the syntax says.
        assert_eq!(short(&parse("10:00-18:00")), "mo-su 10:00-18:00");
    }

    #[test]
    fn what_it_cannot_count_goes_to_the_free_line() {
        let p = parse("Mo-Fr 09:00-12:00; Aug off; week 1-53/2 Sa 09:00-12:00");
        assert_eq!(short(&p), "mo-fr 09:00-12:00");
        assert_eq!(p.rest, vec!["Aug off", "week 1-53/2 Sa 09:00-12:00"]);
        let commented = parse("Mo-Fr 08:30-12:00 \"sur rendez-vous\"; PH off");
        assert_eq!((short(&commented), commented.rest.clone()), ("mo-fr 08:30-12:00".to_string(), vec!["sur rendez-vous".to_string()]));
        // Hours of their own on public holidays: not closed then, the rule shown.
        let holidays = parse("Mo-Fr 09:00-17:00; PH 10:00-12:00");
        assert!(!holidays.holidays_off && holidays.rest == vec!["PH 10:00-12:00"]);
        // Not understood at all: kept whole, no hours, never guessed.
        for odd in ["sunrise-sunset", "by appointment", "Mo-Fr", "Mo-Fr 09:00-12:00+", "Mo[1] 09:00-12:00", "Mo 9:75-10:00", "\"unclosed"] {
            let p = parse(odd);
            assert!(p.windows.is_empty(), "{odd}: {p:?}");
            assert_eq!(p.rest, vec![odd.to_string()], "{odd}");
        }
        assert_eq!(tidied("Mo-Fr 09:00-12:00 ; Jul-Aug Mo-Fr 10:00-12:00"), ("Mo-Fr 09:00-12:00; PH off".to_string(), "Jul-Aug Mo-Fr 10:00-12:00".to_string()));
        assert_eq!(tidied("Mo-Fr 09:00-17:00 || \"by appointment\""), ("Mo-Fr 09:00-17:00; PH off".to_string(), "\"by appointment\"".to_string()));
    }

    #[test]
    fn a_card_keeps_its_lines() {
        let card = crate::lines::unfold(
            "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Example town hall\r\nX-SIOUL-OPENING-HOURS;X-SIOUL-SOURCE=service-public.gouv.fr;X-SIOUL-CHECKED=2026-11-20:Mo-Fr 09:00-12:00\\,14:00-17:00\\; PH off\r\nX-SIOUL-PHONE-HOURS;X-SIOUL-SOURCE=typed:Mo-Th 09:00-12:00\r\nX-SIOUL-HOURS-NOTE:Closed in August\\, by appointment on Thursdays\r\nEND:VCARD\r\n",
        );
        let hours = CardHours::of(&card);
        let open = hours.open.clone().unwrap();
        assert_eq!((open.value.as_str(), open.source.as_str(), open.checked.as_str()), ("Mo-Fr 09:00-12:00,14:00-17:00; PH off", DIRECTORY, "2026-11-20"));
        assert_eq!(hours.note, "Closed in August, by appointment on Thursdays");
        // Written back, the same lines.
        let written: Vec<String> = hours.lines();
        assert_eq!(written[0], "X-SIOUL-OPENING-HOURS;X-SIOUL-SOURCE=service-public.gouv.fr;X-SIOUL-CHECKED=2026-11-20:Mo-Fr 09:00-12:00\\,14:00-17:00\\; PH off");
        assert_eq!(CardHours::of(&written), hours);
        // Unescaped by another program (no backslashes): read the same.
        let plain = crate::lines::unfold("X-SIOUL-OPENING-HOURS:Mo-Fr 09:00-12:00,14:00-17:00; PH off\r\n");
        assert_eq!(CardHours::of(&plain).open.unwrap().value, "Mo-Fr 09:00-12:00,14:00-17:00; PH off");
        // The phone's hours for a call, the opening hours otherwise.
        assert!(hours.followed(true).unwrap().1 && !hours.followed(false).unwrap().1);
    }

    fn cards() -> Cards {
        let office = |uid: &str, open: &str, phone: &str| crate::contacts::Contact {
            uid: uid.into(),
            name: format!("Office {uid}"),
            key: format!("/books/{uid}.vcf"),
            hours: CardHours { open: Some(Hours { value: open.into(), ..Hours::default() }).filter(|h| !h.value.is_empty()), phone: Some(Hours { value: phone.into(), ..Hours::default() }).filter(|h| !h.value.is_empty()), note: String::new() },
            ..crate::contacts::Contact::default()
        };
        Cards::of(&[office("tax", "Mo-Fr 09:00-12:00", "Mo-Th 14:00-16:00"), office("odd", "by appointment", ""), office("hall", "Sa 09:00-12:00", "")])
    }

    fn involving(kind: &str, uids: &[&str], own: &str) -> Task {
        Task { kind: kind.into(), office_hours: true, office_times: own.into(), contacts: uids.iter().map(|u| ContactRef { name: u.to_string(), uri: crate::links::contact_uri(u) }).collect(), ..Task::default() }
    }

    #[test]
    fn a_task_follows_the_most_specific_hours() {
        let cards = cards();
        let usual = window::default_office_hours();
        // Its own first.
        assert_eq!(hours_for(&involving("call", &["tax"], "sa 10:00-11:00"), &cards, &usual).whose, Whose::Own);
        // The card's, the phone's for a call.
        let call = hours_for(&involving("call", &["tax"], ""), &cards, &usual);
        assert!(matches!(&call.whose, Whose::Card { uid, phone: true, .. } if uid == "tax"));
        assert_eq!(window::ranges_text(&call.windows), "mo-th 14:00-16:00");
        let out = hours_for(&involving("out", &["tax"], ""), &cards, &usual);
        assert_eq!(window::ranges_text(&out.windows), "mo-fr 09:00-12:00");
        // The first office whose card has hours decides; a card Sioul cannot count is passed.
        let several = hours_for(&involving("out", &["nobody", "odd", "hall", "tax"], ""), &cards, &usual);
        assert!(matches!(&several.whose, Whose::Card { uid, .. } if uid == "hall"));
        // None: the usual hours.
        assert_eq!(hours_for(&involving("call", &["nobody"], ""), &cards, &usual).whose, Whose::Usual);
    }

    #[test]
    fn open_now_and_next_with_holidays() {
        let hours = parse("Mo-Fr 09:00-12:00,14:00-17:00; PH off");
        let followed = Followed { windows: hours.windows, holidays_off: true, whose: Whose::Usual };
        let tr = crate::i18n::Translator::new("en");
        let none = BTreeSet::new();
        // Friday 20 November 2026.
        let morning = at("2026-11-20T10:00[Europe/Paris]");
        assert_eq!(now_line(&followed, &none, &morning, None, &tr), "Open until 12:00");
        assert_eq!(now_line(&followed, &none, &at("2026-11-20T12:30[Europe/Paris]"), Some("Tax office"), &tr), "Tax office: closed, opens at 14:00");
        assert_eq!(now_line(&followed, &none, &at("2026-11-20T18:00[Europe/Paris]"), None, &tr), "Closed: opens Monday at 9:00");
        assert_eq!(now_line(&followed, &none, &at("2026-11-19T18:00[Europe/Paris]"), None, &tr), "Closed: opens tomorrow at 9:00");
        // Monday 23 a public holiday (invented): closed then, the opening after it.
        let holidays: BTreeSet<Date> = ["2026-11-23".parse().unwrap()].into();
        assert_eq!(now_line(&followed, &holidays, &at("2026-11-23T10:00[Europe/Paris]"), None, &tr), "Closed for the public holiday: opens tomorrow at 9:00");
        assert_eq!(followed.next_opening(&holidays, &at("2026-11-20T18:00[Europe/Paris]")).unwrap().date().to_string(), "2026-11-24");
        assert!(followed.spans_on(&holidays, "2026-11-23".parse().unwrap(), morning.time_zone()).is_empty());
        let fr = crate::i18n::Translator::new("fr");
        assert_eq!(now_line(&followed, &none, &at("2026-11-20T18:00[Europe/Paris]"), None, &fr), "Fermé\u{202f}: ouvre lundi à 9:00");
        assert_eq!(day_lines(&followed.windows, &tr), vec!["Monday to Friday: 9:00–12:00, 14:00–17:00"]);
        assert_eq!(day_lines(&followed.windows, &fr), vec!["Lundi au vendredi\u{202f}: 9:00–12:00, 14:00–17:00"]);
    }

    #[test]
    fn a_call_or_an_outing_is_ticked_when_its_office_comes() {
        let cards = cards();
        let tax = vec![ContactRef { name: "Office tax".into(), uri: crate::links::contact_uri("tax") }];
        assert!(ticks("call", false, &[], &tax, &cards));
        assert!(ticks("out", false, &[], &tax, &cards));
        assert!(!ticks("write", false, &[], &tax, &cards), "a letter needs no open office");
        assert!(!ticks("call", true, &[], &tax, &cards), "ticked already");
        // The office already there: an untick holds.
        assert!(!ticks("call", false, &tax, &tax, &cards));
        let nobody = vec![ContactRef { name: "Sam".into(), uri: crate::links::contact_uri("sam") }];
        assert!(!ticks("call", false, &[], &nobody, &cards), "a card without hours");
    }
}
