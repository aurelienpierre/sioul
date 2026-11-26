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
/// "open": open on public holidays (a parameter of its line; unsaid: closed).
pub const HOLIDAYS: &str = "X-SIOUL-HOLIDAYS";

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
    /// Open on public holidays, as the person ticked it or a source said it
    /// (`X-SIOUL-HOLIDAYS=open`, a parameter of Sioul's own on the line, kept
    /// as its other parameters are). A line without it reads as closed on
    /// public holidays, unless its value says otherwise (`holidays_off`).
    #[serde(default)]
    pub holidays_open: bool,
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

    /// Closed on public holidays: as the value says it when it says it (`PH
    /// off`, hours of their own on `PH`, `24/7`), else as the line's
    /// parameter says it, else closed, as Sioul assumes for an office.
    pub fn holidays_off(&self) -> bool {
        let parsed = self.read();
        if parsed.holidays_said { parsed.holidays_off } else { !self.holidays_open }
    }

    /// The day they were checked, when it reads as one.
    pub fn checked_on(&self) -> Option<Date> {
        self.checked.trim().parse().ok()
    }

    /// Their line on a card: `name;X-SIOUL-SOURCE=…;X-SIOUL-CHECKED=…;X-SIOUL-HOLIDAYS=open:value`.
    fn line(&self, name: &str) -> String {
        let mut head = name.to_string();
        if !self.source.trim().is_empty() {
            head.push_str(&format!(";{SOURCE}={}", crate::lines::param_value(self.source.trim())));
        }
        if self.checked_on().is_some() {
            head.push_str(&format!(";{CHECKED}={}", self.checked.trim()));
        }
        if self.holidays_open {
            head.push_str(&format!(";{HOLIDAYS}=open"));
        }
        format!("{head}:{}", crate::lines::escape(self.value.trim()))
    }

    fn of_line(line: &str) -> Hours {
        Hours {
            value: crate::lines::unescape(crate::lines::value(line)).trim().to_string(),
            source: crate::lines::param(line, SOURCE).unwrap_or_default().trim().to_string(),
            checked: crate::lines::param(line, CHECKED).unwrap_or_default().trim().to_string(),
            holidays_open: crate::lines::param(line, HOLIDAYS).is_some_and(|v| v.trim().eq_ignore_ascii_case("open")),
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
    /// The text of a second line of the same name (a card merged from two,
    /// another program's): the first counts; these go to the free line at
    /// the next save by the person (`folded`), never lost unseen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<String>,
}

impl CardHours {
    /// Read from a card's lines (unfolded): the first line of each name, the
    /// others' text kept apart (`extra`); at most `MOST_NOTE` characters of
    /// the free line, the rest cut.
    pub fn of(card: &[String]) -> CardHours {
        let all = |name: &'static str| card.iter().filter(move |l| crate::lines::name(l) == name);
        let text = |l: &String| crate::lines::unescape(crate::lines::value(l)).trim().to_string();
        let hours = |name: &'static str| all(name).next().map(|l| Hours::of_line(l)).filter(|h| !h.value.is_empty());
        let extra: Vec<String> = [OPEN, PHONE, NOTE].into_iter().flat_map(|name| all(name).skip(1).map(text)).filter(|t| !t.is_empty()).map(|t| cut(&t, MOST_NOTE)).collect();
        CardHours { open: hours(OPEN), phone: hours(PHONE), note: all(NOTE).next().map(text).map(|t| cut(&t, MOST_NOTE)).unwrap_or_default(), extra }
    }

    /// The same, the text of its second lines added to the free line (each
    /// once): what a save by the person writes.
    pub fn folded(mut self) -> CardHours {
        for part in std::mem::take(&mut self.extra) {
            if !self.note.contains(&part) {
                self.note = if self.note.trim().is_empty() { part } else { format!("{}; {part}", self.note.trim()) };
            }
        }
        self
    }

    /// Nothing at all: no hours, no free line.
    pub fn is_empty(&self) -> bool {
        self.open.is_none() && self.phone.is_none() && self.note.trim().is_empty() && self.extra.is_empty()
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

/// At most this many characters of a set of hours are read (review of 20
/// November 2026, 13): a longer value counts as no hours, shown cut.
pub const MOST_HOURS: usize = 1000;
/// At most this many characters of the free line are read and shown.
pub const MOST_NOTE: usize = 2000;

/// `text` cut at `most` characters, "…" after it when cut.
pub fn cut(text: &str, most: usize) -> String {
    if text.chars().count() <= most { text.to_string() } else { format!("{}…", text.chars().take(most).collect::<String>()) }
}

/// Why Sioul counts none of a value, when the form says more than "cannot
/// count": the message that says it.
pub const DOUBT_DI: &str = "hours-doubt-di";
pub const DOUBT_LONG: &str = "hours-doubt-long";

/// What Sioul counts of a set of hours.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Each day's ranges, as the usual hours are kept.
    pub windows: Vec<AdminWindow>,
    /// Closed on public holidays: as the value says (`PH off`; hours of
    /// their own on `PH`, or `24/7`, open), else assumed for an office.
    pub holidays_off: bool,
    /// Whether the value says it (else `Hours::holidays_off` asks the line's parameter).
    pub holidays_said: bool,
    /// What it could not count, each part as written ("Aug off", "week 1-53/2 Mo 09:00-12:00"),
    /// a comment without its quotes ("by appointment"). A value Sioul does
    /// not count at all is here whole (cut at `MOST_HOURS`), and counts as no hours.
    pub rest: Vec<String>,
    /// Why it counts none of it, when there is more to say (`DOUBT_DI`,
    /// `DOUBT_LONG`); "" otherwise.
    pub doubt: &'static str,
}

/// One rule's pieces: days (none said: every day; `Some([])`: public
/// holidays alone), its time ranges, or closed; whether it names public holidays.
#[derive(Debug, Default)]
struct Segment {
    days: Option<Vec<usize>>,
    holidays: bool,
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

/// A word Sioul knows how to place in a rule of weekdays and hours: a day,
/// public holidays, `off`, `closed`, `open`, `24/7`, or what looks like hours
/// (it starts with a digit and is not a year).
fn known_word(word: &str) -> bool {
    let lower = word.to_lowercase();
    matches!(lower.as_str(), "off" | "closed" | "open" | "24/7" | "ph") || days_of(word).is_some() || (lower.starts_with(|c: char| c.is_ascii_digit()) && !selector(&lower))
}

/// A word that names weekdays or hours, even ill-written ("Mo", "9:75-10:00").
fn weekdays_or_hours(word: &str) -> bool {
    let lower = word.to_lowercase();
    lower == "24/7" || days_of(word).is_some() || (lower.starts_with(|c: char| c.is_ascii_digit()) && !selector(&lower))
}

/// One of the syntax's selectors Sioul does not count (lower case): a month
/// ("aug", "jul-aug", "dec 25" starts so), a week, a year, Easter, school
/// holidays, the sun's hours, the nth weekday of a month (`mo[1]`).
fn selector(lower: &str) -> bool {
    const MONTHS: &[&str] = &["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let year = |w: &str| w.len() == 4 && w.chars().all(|c| c.is_ascii_digit());
    MONTHS.iter().any(|m| lower.strip_prefix(m).is_some_and(|after| after.is_empty() || after.starts_with(['-', ',']) || after.starts_with(|c: char| c.is_ascii_digit())))
        || ["january", "february", "march", "april", "june", "july", "august", "september", "october", "november", "december"].contains(&lower)
        || matches!(lower, "week" | "easter" | "sh")
        || ["sunrise", "sunset", "dawn", "dusk"].iter().any(|s| lower.contains(s))
        || lower.contains('[')
        || lower.split(['-', ',']).all(year)
}

/// One rule of `;`, read.
enum Rule {
    /// Counted: its segments (a comma before days starts another one, added
    /// to the first), its comment.
    Counted(Vec<Segment>, Option<String>),
    /// It selects by what Sioul does not count (a month, a week, a date,
    /// sunrise): to the free line, as written.
    Foreign,
    /// Weekdays and hours Sioul cannot count: the whole value is not counted
    /// rather than half read (review 5).
    Unread,
}

fn rule_of(text: &str) -> Rule {
    let Some(tokens) = tokens(text) else { return Rule::Unread };
    let words: Vec<&str> = tokens.iter().filter_map(|t| if let Token::Word(w) = t { Some(w.as_str()) } else { None }).collect();
    // A month, a week, sunrise: the design's limit, said in the free line.
    if words.iter().any(|w| selector(&w.to_lowercase())) {
        return Rule::Foreign;
    }
    // Words Sioul does not know: a sentence ("by appointment") goes to the free line;
    // beside weekdays or hours ("Sa 10-12 and 14-16") they make the rule one it cannot count.
    if words.iter().any(|w| !known_word(w)) {
        return if words.iter().any(|w| weekdays_or_hours(w)) { Rule::Unread } else { Rule::Foreign };
    }
    let mut segments: Vec<Segment> = Vec::new();
    let mut current = Segment::default();
    let mut comment = None;
    for token in tokens {
        match token {
            Token::Comma => {}
            Token::Comment(c) => comment = Some(c).filter(|c| !c.is_empty()),
            Token::Word(word) => {
                let lower = word.to_lowercase();
                if lower == "off" || lower == "closed" {
                    current.off = true;
                } else if lower == "open" {
                    // The syntax's word for open: it changes nothing.
                } else if lower == "24/7" {
                    current.days = Some((0..7).collect());
                    current.times.push(("00:00".into(), "24:00".into()));
                } else if lower == "ph" || days_of(&word).is_some() {
                    if !current.times.is_empty() || current.off {
                        segments.push(std::mem::take(&mut current));
                    }
                    let days = current.days.get_or_insert_with(Vec::new);
                    if lower == "ph" {
                        current.holidays = true;
                    } else {
                        days.extend(days_of(&word).unwrap_or_default());
                    }
                } else if let Some(range) = range_of(&word) {
                    // Hours after "off", or that do not go forward in a day ("9-12, 1-5"): never guessed.
                    if current.off || current.times.last().is_some_and(|(_, end)| range.0 < *end) {
                        return Rule::Unread;
                    }
                    current.times.push(range);
                } else {
                    return Rule::Unread;
                }
            }
        }
    }
    if current.days.is_some() || !current.times.is_empty() || current.off {
        segments.push(current);
    }
    // Days alone ("Mo-Fr"), with no hours nor "off", say nothing Sioul would not guess.
    if segments.iter().any(|s| s.times.is_empty() && !s.off) {
        return Rule::Unread;
    }
    if segments.is_empty() && comment.is_none() {
        return Rule::Unread;
    }
    Rule::Counted(segments, comment)
}

/// "Di" is Sunday in French and Tuesday in German: a value where it stands
/// beside days that are not French ("Mo", "Fr"), with no day only French
/// has ("lu", "ma", "me", "je", "ve"), is not counted (review 6).
fn di_in_doubt(text: &str) -> bool {
    let mut plain = String::new();
    let mut quoted = false;
    for c in text.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if !quoted {
            plain.push(c);
        }
    }
    let words: Vec<String> = plain.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '-')).map(str::to_lowercase).filter(|w| !w.is_empty()).collect();
    const NOT_FRENCH: &[&str] = &["mo", "tu", "we", "th", "fr", "su", "mon", "tue", "wed", "thu", "fri", "sun", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
    const FRENCH_ONLY: &[&str] = &["lu", "ma", "me", "je", "ve", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
    words.iter().any(|w| w == "di") && words.iter().any(|w| NOT_FRENCH.contains(&w.as_str())) && !words.iter().any(|w| FRENCH_ONLY.contains(&w.as_str()))
}

/// Opening hours in OpenStreetMap's syntax, or in the short form typed on a
/// task ("mo-fr 9-12, 14-17", French days too), read: each rule of `;`
/// overrides the days it names, as the syntax says ("Mo-Fr 09:00-17:00; We
/// 09:00-12:00" closes Wednesday afternoon); `off` closes them, or cuts out
/// the hours it names ("Mo-Fr 08:00-18:00; Mo-Fr 12:00-14:00 off": a lunch
/// break); `open` changes nothing; `PH off` says public holidays are closed,
/// `PH` with hours that they are open, `PH` in a list of days counts those
/// days. A rule that selects by what Sioul does not count (a month, a week,
/// sunrise) goes to `rest`, as written; a rule of weekdays and hours it
/// cannot count, a value too long or a "di" in doubt make the whole value
/// uncounted, kept whole in `rest`: never half read, never guessed. A value
/// that leaves no hours at all ("closed") is kept the same way.
pub fn parse(text: &str) -> Parsed {
    let text = text.trim();
    let mut out = Parsed { holidays_off: true, ..Parsed::default() };
    if text.is_empty() {
        return out;
    }
    let uncounted = |doubt: &'static str| Parsed { windows: Vec::new(), holidays_off: true, holidays_said: false, rest: vec![cut(text, MOST_HOURS)], doubt };
    if text.chars().count() > MOST_HOURS {
        return uncounted(DOUBT_LONG);
    }
    if di_in_doubt(text) {
        return uncounted(DOUBT_DI);
    }
    let mut week: [Vec<(String, String)>; 7] = Default::default();
    let mut counted = false;
    let (mut holidays_closed, mut holidays_open) = (false, false);
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
        let (segments, comment) = match rule_of(rule) {
            Rule::Counted(segments, comment) => (segments, comment),
            Rule::Foreign => {
                out.rest.push(rule.to_string());
                continue;
            }
            Rule::Unread => return uncounted(""),
        };
        for (i, segment) in segments.iter().enumerate() {
            if segment.holidays {
                if segment.off {
                    holidays_closed = true;
                } else {
                    holidays_open = true;
                    // Hours of their own on public holidays alone: shown, not counted.
                    if segment.days.as_ref().is_some_and(Vec::is_empty) && !out.rest.iter().any(|r| r == rule) {
                        out.rest.push(rule.to_string());
                    }
                }
            }
            if segment.times.first().is_some_and(|t| t.0 == "00:00" && t.1 == "24:00") && segment.days.as_ref().is_some_and(|d| d.len() == 7) && !segment.off {
                holidays_open = true;
            }
            let days = segment.days.clone().unwrap_or_else(|| (0..7).collect());
            for day in days {
                if segment.off {
                    if segment.times.is_empty() { week[day].clear() } else { week[day] = cut_out(&week[day], &segment.times) }
                    continue;
                }
                // The rule's first part overrides those days; a part after a comma adds to them.
                if i == 0 {
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
    for ranges in week.iter_mut() {
        *ranges = joined(ranges);
    }
    // Nothing counted, or no hours left at all ("closed"): kept whole, as written.
    if !counted || week.iter().all(Vec::is_empty) {
        return uncounted("");
    }
    out.holidays_said = holidays_closed || holidays_open;
    out.holidays_off = holidays_closed || !holidays_open;
    for (day, ranges) in week.iter().enumerate() {
        for (start, end) in ranges {
            out.windows.push(AdminWindow { day: window::NAMES[day].into(), start: start.clone(), end: Some(end.clone()), minutes: 0, kind: None });
        }
    }
    out
}

/// A day's ranges with `cut` taken out of them ("08:00-18:00" less
/// "12:00-14:00": "08:00-12:00", "14:00-18:00").
fn cut_out(ranges: &[(String, String)], cut: &[(String, String)]) -> Vec<(String, String)> {
    let mut left: Vec<(String, String)> = ranges.to_vec();
    for (from, to) in cut {
        left = left
            .into_iter()
            .flat_map(|(start, end)| {
                let mut kept = Vec::new();
                if start < *from {
                    kept.push((start.clone(), end.clone().min(from.clone())));
                }
                if end > *to {
                    kept.push((start.max(to.clone()), end));
                }
                kept
            })
            .filter(|(s, e)| s < e)
            .collect();
    }
    left
}

/// A day's ranges in order, those that touch or overlap made one
/// ("09:00-12:00", "12:00-17:00": "09:00-17:00").
fn joined(ranges: &[(String, String)]) -> Vec<(String, String)> {
    let mut sorted = ranges.to_vec();
    sorted.sort();
    let mut out: Vec<(String, String)> = Vec::new();
    for (start, end) in sorted {
        match out.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.clone().max(end),
            _ => out.push((start, end)),
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
/// together: "Mo-Fr 09:00-12:00,14:00-17:00; Sa 09:00-12:00; PH off";
/// "24/7" for every hour of every day.
pub fn written(windows: &[AdminWindow], holidays_off: bool) -> String {
    let short = window::ranges_text(windows);
    let mut parts: Vec<String> = if short == "mo-su 00:00-24:00" {
        vec!["24/7".into()]
    } else {
        short
            .split("; ")
            .filter(|p| !p.is_empty())
            .map(|part| {
                let (days, hours) = part.split_once(' ').unwrap_or((part, ""));
                let days: String = days.split(['-', ',']).map(capitalised).collect::<Vec<_>>().join(if days.contains('-') { "-" } else { "," });
                format!("{days} {}", hours.replace(", ", ","))
            })
            .collect()
    };
    if holidays_off && !parts.is_empty() {
        parts.push("PH off".into());
    }
    parts.join("; ")
}

/// The hours in the short form the person types, in the window's words
/// ("lu-ve 9:00-12:00, 14:00-17:00; sa 9:00-12:00" in French; the days'
/// abbreviations are the words' `hours-short-days`): what the form shows.
pub fn short_form(windows: &[AdminWindow], tr: &crate::i18n::Translator) -> String {
    let names: Vec<String> = tr.text("hours-short-days", None).split_whitespace().map(str::to_string).collect();
    let day = |i: usize| names.get(i).cloned().unwrap_or_else(|| window::SHORT[i].to_string());
    let time = |t: &str| t.strip_prefix('0').filter(|t| t.len() == 4).unwrap_or(t).to_string();
    window::ranges_text(windows)
        .split("; ")
        .filter(|p| !p.is_empty())
        .map(|part| {
            let (days, hours) = part.split_once(' ').unwrap_or((part, ""));
            let days: String = days.split('-').map(|d| window::SHORT.iter().position(|s| *s == d).map(day).unwrap_or_else(|| d.to_string())).collect::<Vec<_>>().join("-");
            let hours: Vec<String> = hours.split(", ").map(|r| r.split('-').map(time).collect::<Vec<_>>().join("-")).collect();
            format!("{days} {}", hours.join(", "))
        })
        .collect::<Vec<_>>()
        .join("; ")
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

    /// The office a task's `CONTACT` points at, when its card has hours: by
    /// `urn:uuid:<UID>` as tasks are written since 26 November 2026, or by
    /// Sioul's own `sioul:contact/<UID>` as older ones and the window give it
    /// (`contacts::names_card`), the card of exactly that UID first.
    pub fn get(&self, uri: &str) -> Option<&Office> {
        (crate::links::kind_of(uri) == crate::links::Kind::Contact)
            .then(|| self.0.get(crate::links::id_of(uri).trim()))
            .flatten()
            .or_else(|| self.0.values().find(|office| crate::contacts::names_card(uri, &office.uid)))
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
            return Followed { windows: parsed.windows, holidays_off: hours.holidays_off(), whose: Whose::Card { uid: office.uid.clone(), name: office.name.clone(), key: office.key.clone(), number: office.phone.clone(), phone } };
        }
    }
    Followed { windows: usual.to_vec(), holidays_off: true, whose: Whose::Usual }
}

/// The kinds of task that need their office open when it has hours: a call, going out.
pub const OFFICE_KINDS: &[&str] = &["call", "out"];

/// Whether a task is to be ticked "Needs an open office" by itself (decided
/// 20 November 2026): a call or an outing, not ticked, that now involves an
/// office whose card has the hours it would follow (the phone's or the
/// counter's for a call, the counter's for an outing) that `before` (its
/// offices as its file had them) did not, or that has just become a call or
/// an outing (`kind_before`, its kind as its file had it; "" for a new task)
/// with such an office. What Sioul sets counts until corrected: it is never
/// set again for an office already there, so an untick holds.
pub fn ticks(kind: &str, ticked: bool, kind_before: &str, before: &[crate::tasks::ContactRef], after: &[crate::tasks::ContactRef], cards: &Cards) -> bool {
    if ticked || !OFFICE_KINDS.contains(&kind) {
        return false;
    }
    let follows = |uri: &str| cards.get(uri).is_some_and(|office| office.hours.followed(kind == "call").is_some());
    let became = !OFFICE_KINDS.contains(&kind_before);
    let had = |uri: &str| before.iter().any(|b| crate::links::id_of(&b.uri).trim() == crate::links::id_of(uri).trim() || crate::contacts::same_card(&b.uri, uri));
    after.iter().any(|c| (became || !had(&c.uri)) && follows(&c.uri))
}

/// The open calls and outings tied to the office's card `uid` that are not
/// ticked "Needs an open office", and would follow `hours`: those ticked
/// when the card gets hours for the first time by the person's act on this
/// device (its form saved, a pick; review 3). A card that arrives by sync
/// never asks: the tick travels with the task.
pub fn to_tick<'a>(tasks: &'a [crate::tasks::Task], uid: &str, hours: &CardHours) -> Vec<&'a crate::tasks::Task> {
    let uid = uid.trim();
    if uid.is_empty() {
        return Vec::new();
    }
    let tied = |t: &crate::tasks::Task| t.contacts.iter().any(|c| (crate::links::kind_of(&c.uri) == crate::links::Kind::Contact && crate::links::id_of(&c.uri).trim() == uid) || crate::contacts::names_card(&c.uri, uid));
    tasks.iter().filter(|t| t.status.is_open() && !t.read_only && !t.office_hours && OFFICE_KINDS.contains(&t.kind.as_str()) && tied(t) && hours.followed(t.kind == "call").is_some()).collect()
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
        // Open the whole of today ("24/7"): said so, never "until 0:00".
        let all_day = followed.windows.iter().any(|w| w.weekday() == Some(now.weekday()) && w.start.trim() == "00:00" && w.end_text() == "24:00");
        if all_day {
            return say(&format!("{lead}open-all-day"), &[]);
        }
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
        assert!(p.holidays_off && p.holidays_said && p.rest.is_empty());
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
        // Hours with no days: every day, as the syntax says.
        assert_eq!(short(&parse("10:00-18:00")), "mo-su 10:00-18:00");
        // Ranges that touch are one.
        assert_eq!(short(&parse("Mo-Fr 09:00-12:00,12:00-17:00")), "mo-fr 09:00-17:00");
    }

    /// `off` after hours cuts those hours out (the review's two inputs);
    /// over a whole day, or with `24/7`, it closes (review 2).
    #[test]
    fn off_cuts_out_what_it_names() {
        assert_eq!(short(&parse("Mo-Fr 08:00-18:00; Mo-Fr 12:00-14:00 off")), "mo-fr 08:00-12:00, 14:00-18:00");
        assert_eq!(short(&parse("Mo-Fr 09:00-17:00; 12:00-13:00 off")), "mo-fr 09:00-12:00, 13:00-17:00");
        assert_eq!(short(&parse("Mo-Fr 09:00-17:00; We 08:00-10:00 off")), "mo-tu 09:00-17:00; we 10:00-17:00; th-fr 09:00-17:00");
        assert_eq!(short(&parse("Mo-Sa 09:00-17:00; Sa off")), "mo-fr 09:00-17:00");
        // Nothing left at all: no hours, kept as written for the free line (review 14).
        for closed in ["24/7 off", "off", "closed", "Mo-Su off"] {
            let p = parse(closed);
            assert!(p.windows.is_empty() && p.rest == vec![closed.to_string()], "{closed}: {p:?}");
            assert_eq!(tidied(closed), (String::new(), closed.to_string()));
        }
        // 24/7: every hour, open on public holidays, written back as it was.
        let always = parse("24/7");
        assert_eq!(short(&always), "mo-su 00:00-24:00");
        assert!(!always.holidays_off && always.holidays_said);
        assert_eq!(written(&always.windows, always.holidays_off), "24/7");
    }

    /// A `PH` rule that ends with `off` or `closed` is closed, whatever shares
    /// its list or follows it; hours of their own on `PH` open them; a value
    /// that says nothing of them follows the line's parameter, else closed (review 4).
    #[test]
    fn public_holidays_as_the_value_or_the_line_says() {
        for closed in ["Mo-Sa 09:00-18:00; PH,Su off", "Mo-Sa 09:00-18:00; PH off \"jours fériés\"", "Mo-Fr 09:00-17:00; PH off; PH 10:00-12:00", "Mo-Fr 09:00-17:00; PH closed"] {
            let p = parse(closed);
            assert!(p.holidays_off && p.holidays_said, "{closed}: {p:?}");
            assert!(!p.windows.is_empty(), "{closed}");
        }
        assert_eq!(short(&parse("Mo-Sa 09:00-18:00; PH,Su off")), "mo-sa 09:00-18:00");
        let open = parse("Mo-Fr 09:00-17:00; PH 10:00-12:00");
        assert!(!open.holidays_off && open.holidays_said && open.rest == vec!["PH 10:00-12:00"]);
        // PH in a list of days: the days counted, public holidays open.
        let listed = parse("Mo-Fr,PH 09:00-17:00");
        assert_eq!(short(&listed), "mo-fr 09:00-17:00");
        assert!(!listed.holidays_off && listed.rest.is_empty());
        // Nothing said in the value: the line's parameter decides, else closed.
        let plain = Hours { value: "Mo-Fr 09:00-17:00".into(), ..Hours::default() };
        assert!(plain.holidays_off());
        assert!(!Hours { holidays_open: true, ..plain.clone() }.holidays_off());
        assert!(Hours { value: "Mo-Fr 09:00-17:00; PH off".into(), holidays_open: true, ..plain }.holidays_off(), "the value's own PH off wins");
    }

    /// A rule Sioul cannot count never leaves standing what it meant to
    /// change: `open` changes nothing, `PH` in a list is taken out, and a rule
    /// of weekdays and hours that still cannot be counted leaves the whole
    /// value uncounted (review 5); hours that do not go forward in a day are
    /// not counted (review 21).
    #[test]
    fn a_rule_it_cannot_count_is_never_half_read() {
        let saturday = parse("Mo-Sa 09:00-17:00; Sa,PH off");
        assert_eq!(short(&saturday), "mo-fr 09:00-17:00");
        assert!(saturday.holidays_off && saturday.rest.is_empty());
        let friday = parse("Mo-Fr 09:00-17:00; Fr 09:00-12:00 open \"sur rdv\"");
        assert_eq!(short(&friday), "mo-th 09:00-17:00; fr 09:00-12:00");
        assert_eq!(friday.rest, vec!["sur rdv"]);
        for odd in ["Mo-Fr 09:00-17:00; Sa 09:00-12:00+", "Mo-Fr 09:00-17:00; Sa 9:75-12:00", "Mo-Fr 9-12, 1-5", "Mo-Fr 14:00-17:00,09:00-12:00", "Mo-Fr 09:00-17:00; Sa 10:00-12:00 and 14:00-16:00"] {
            let p = parse(odd);
            assert!(p.windows.is_empty() && p.rest == vec![odd.to_string()], "{odd}: {p:?}");
        }
    }

    /// "Di" beside days that are not French, with no day only French has, is
    /// not counted, and says why; among French days it is Sunday (review 6).
    #[test]
    fn di_is_not_guessed() {
        for doubt in ["Di-Fr 09:00-12:00", "Mo-Fr 9-12; Di 14-18", "Mo 09:00-12:00; Di,Th 14:00-16:00"] {
            let p = parse(doubt);
            assert!(p.windows.is_empty() && p.doubt == DOUBT_DI, "{doubt}: {p:?}");
        }
        assert_eq!(short(&parse("lu-ve 9-12; di 10-12")), "mo-fr 09:00-12:00; su 10:00-12:00");
        assert_eq!(short(&parse("di 10-12")), "su 10:00-12:00");
        assert_eq!(short(&parse("Mo-Fr 09:00-12:00 \"di\"")), "mo-fr 09:00-12:00", "a comment's words are no days");
    }

    /// At most 1,000 characters of hours and 2,000 of the free line are read;
    /// beyond, nothing counted, shown cut (review 13).
    #[test]
    fn a_value_too_long_is_not_read() {
        let long = format!("Mo-Fr 09:00-12:00; {}", "Sa 09:00-12:00; ".repeat(80));
        let p = parse(&long);
        assert!(p.windows.is_empty() && p.doubt == DOUBT_LONG);
        assert_eq!(p.rest[0].chars().count(), MOST_HOURS + 1);
        assert!(p.rest[0].ends_with('…'));
        let card = crate::lines::unfold(&format!("BEGIN:VCARD\r\nX-SIOUL-HOURS-NOTE:{}\r\nEND:VCARD\r\n", "x".repeat(5000)));
        assert_eq!(CardHours::of(&card).note.chars().count(), MOST_NOTE + 1);
    }

    /// The short form the form shows, in the window's words.
    #[test]
    fn the_short_form_in_the_window_s_words() {
        let hours = parse("Mo-Fr 09:00-12:00,14:00-17:00; Sa 09:00-12:00; PH off");
        assert_eq!(short_form(&hours.windows, &crate::i18n::Translator::new("en")), "mo-fr 9:00-12:00, 14:00-17:00; sa 9:00-12:00");
        assert_eq!(short_form(&hours.windows, &crate::i18n::Translator::new("fr")), "lu-ve 9:00-12:00, 14:00-17:00; sa 9:00-12:00");
        // Read back, the same hours.
        assert_eq!(parse(&short_form(&hours.windows, &crate::i18n::Translator::new("fr"))).windows, hours.windows);
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
        // Open on public holidays: a parameter of Sioul's own, kept beside the others, read back.
        let open_days = CardHours { open: Some(Hours { value: "Mo-Su 08:00-20:00".into(), source: TYPED.into(), checked: "2026-11-20".into(), holidays_open: true }), ..CardHours::default() };
        let line = open_days.lines();
        assert_eq!(line[0], "X-SIOUL-OPENING-HOURS;X-SIOUL-SOURCE=typed;X-SIOUL-CHECKED=2026-11-20;X-SIOUL-HOLIDAYS=open:Mo-Su 08:00-20:00");
        assert_eq!(CardHours::of(&line), open_days);
        assert!(!open_days.open.as_ref().unwrap().holidays_off());
        // A line without it: closed on public holidays, as before.
        assert!(CardHours::of(&plain).open.unwrap().holidays_off());
    }

    /// A card holding two lines of hours (merged from two cards, another
    /// program's): the first counts, the second's text is kept apart and
    /// goes to the free line at the person's next save, never lost (review 20).
    #[test]
    fn a_second_line_is_never_lost_unseen() {
        let card = crate::lines::unfold("BEGIN:VCARD\r\nX-SIOUL-OPENING-HOURS:Mo-Fr 09:00-12:00\r\nX-SIOUL-HOURS-NOTE:By appointment\r\nX-SIOUL-OPENING-HOURS:Sa 10:00-12:00\r\nEND:VCARD\r\n");
        let hours = CardHours::of(&card);
        assert_eq!(hours.open.as_ref().unwrap().value, "Mo-Fr 09:00-12:00");
        assert_eq!(hours.extra, vec!["Sa 10:00-12:00"]);
        let folded = hours.folded();
        assert_eq!((folded.note.as_str(), folded.extra.len()), ("By appointment; Sa 10:00-12:00", 0));
        assert_eq!(folded.lines().len(), 2, "one line of each");
    }

    fn cards() -> Cards {
        let office = |uid: &str, open: &str, phone: &str| crate::contacts::Contact {
            uid: uid.into(),
            name: format!("Office {uid}"),
            key: format!("/books/{uid}.vcf"),
            hours: CardHours { open: Some(Hours { value: open.into(), ..Hours::default() }).filter(|h| !h.value.is_empty()), phone: Some(Hours { value: phone.into(), ..Hours::default() }).filter(|h| !h.value.is_empty()), ..CardHours::default() },
            ..crate::contacts::Contact::default()
        };
        Cards::of(&[office("tax", "Mo-Fr 09:00-12:00", "Mo-Th 14:00-16:00"), office("odd", "by appointment", ""), office("hall", "Sa 09:00-12:00", ""), office("line", "", "Mo-Fr 09:00-17:00")])
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
        assert_eq!(day_lines(&followed.windows, &fr), vec!["Du lundi au vendredi\u{202f}: 9:00–12:00, 14:00–17:00"]);
        // Open every hour: "Open all day", never "until 0:00" (review 14).
        let always = Followed { windows: parse("24/7").windows, holidays_off: false, whose: Whose::Usual };
        assert_eq!(now_line(&always, &none, &morning, None, &tr), "Open all day");
        assert_eq!(now_line(&always, &none, &morning, Some("Helpline"), &tr), "Helpline: open all day");
    }

    /// The edges of time (review 11): open from the minute of opening,
    /// closed at the minute of closing; a day when the clock changes keeps
    /// the hours of the wall; nothing in the coming fortnight is said;
    /// checked more than a year ago is stale, a year exactly is not.
    #[test]
    fn the_edges_of_time() {
        let followed = Followed { windows: parse("Mo-Fr 09:00-12:00").windows, holidays_off: true, whose: Whose::Usual };
        let tr = crate::i18n::Translator::new("en");
        let none = BTreeSet::new();
        // Monday 23 November 2026.
        assert!(followed.open_until(&none, &at("2026-11-23T08:59[Europe/Paris]")).is_none());
        assert_eq!(followed.open_until(&none, &at("2026-11-23T09:00[Europe/Paris]")).map(|z| clock(&z)), Some("12:00".into()));
        assert!(followed.open_until(&none, &at("2026-11-23T11:59[Europe/Paris]")).is_some());
        assert!(followed.open_until(&none, &at("2026-11-23T12:00[Europe/Paris]")).is_none(), "closed at the minute of closing");
        // Sunday 25 October 2026, the clock goes back at 3:00: a Sunday office opens at 9:00 on the wall.
        let sunday = Followed { windows: parse("Su 09:00-12:00").windows, holidays_off: true, whose: Whose::Usual };
        let opens = sunday.next_opening(&none, &at("2026-10-25T01:00[Europe/Paris]")).unwrap();
        assert_eq!((opens.date().to_string(), clock(&opens)), ("2026-10-25".to_string(), "9:00".to_string()));
        assert_eq!(sunday.open_until(&none, &at("2026-10-25T10:00[Europe/Paris]")).map(|z| clock(&z)), Some("12:00".into()));
        // Every weekday a public holiday for three weeks: nothing opens in the coming fortnight.
        let holidays: BTreeSet<Date> = (0..21).map(|d| "2026-11-23".parse::<Date>().unwrap().checked_add(jiff::Span::new().days(d)).unwrap()).collect();
        assert_eq!(now_line(&followed, &holidays, &at("2026-11-23T10:00[Europe/Paris]"), None, &tr), "Closed for the coming two weeks");
        // Stale: more than a year.
        let today: Date = "2026-11-20".parse().unwrap();
        let checked = |day: &str| Hours { value: "Mo 09:00-10:00".into(), checked: day.into(), ..Hours::default() };
        assert!(stale(&checked("2025-11-19"), today) && !stale(&checked("2025-11-20"), today) && !stale(&checked(""), today));
    }

    #[test]
    fn a_call_or_an_outing_is_ticked_when_its_office_comes() {
        let cards = cards();
        let tax = vec![ContactRef { name: "Office tax".into(), uri: crate::links::contact_uri("tax") }];
        assert!(ticks("call", false, "call", &[], &tax, &cards));
        assert!(ticks("out", false, "", &[], &tax, &cards));
        assert!(!ticks("write", false, "write", &[], &tax, &cards), "a letter needs no open office");
        assert!(!ticks("call", true, "call", &[], &tax, &cards), "ticked already");
        // The office already there: an untick holds.
        assert!(!ticks("call", false, "call", &tax, &tax, &cards));
        let nobody = vec![ContactRef { name: "Sam".into(), uri: crate::links::contact_uri("sam") }];
        assert!(!ticks("call", false, "call", &[], &nobody, &cards), "a card without hours");
        // A task already tied that becomes a call in its form: ticked (review 3).
        assert!(ticks("call", false, "write", &tax, &tax, &cards));
        assert!(!ticks("out", false, "call", &tax, &tax, &cards), "a call become an outing: an untick holds");
        // A call to a card with opening hours only follows those.
        let hall = vec![ContactRef { name: "Office hall".into(), uri: crate::links::contact_uri("hall") }];
        assert!(ticks("call", false, "call", &[], &hall, &cards));
        let follows = hours_for(&involving("call", &["hall"], ""), &cards, &window::default_office_hours());
        assert!(matches!(&follows.whose, Whose::Card { phone: false, .. }));
        // A card with phone hours only: a call follows them, an outing does not (review 16).
        let line = vec![ContactRef { name: "Office line".into(), uri: crate::links::contact_uri("line") }];
        assert!(ticks("call", false, "call", &[], &line, &cards));
        assert!(!ticks("out", false, "out", &[], &line, &cards));
        // The office named as tasks are written since 26 November 2026, `urn:uuid:<UID>`: the same office.
        let tax_now = vec![ContactRef { name: "Office tax".into(), uri: "urn:uuid:tax".into() }];
        assert!(ticks("call", false, "call", &[], &tax_now, &cards));
        assert!(!ticks("call", false, "call", &tax, &tax_now, &cards) && !ticks("call", false, "call", &tax_now, &tax, &cards), "there already, in the other form: an untick holds");
        let hall_now = Task { kind: "call".into(), office_hours: true, contacts: vec![ContactRef { name: "Office hall".into(), uri: "urn:uuid:hall".into() }], ..Task::default() };
        assert_eq!(hours_for(&hall_now, &cards, &window::default_office_hours()).whose, follows.whose, "its card's hours, either way");
    }

    /// The tasks ticked when a card gets hours by the person's act: its open
    /// calls and outings not ticked yet, tied to it, that would follow them;
    /// never one done, ticked, read-only, of another kind or another card's.
    #[test]
    fn the_tasks_tied_to_a_card_that_gets_hours() {
        let tied = |uid: &str, kind: &str, to: &str| Task { uid: uid.into(), kind: kind.into(), contacts: vec![ContactRef { name: String::new(), uri: crate::links::contact_uri(to) }], ..Task::default() };
        let tasks = vec![
            tied("call", "call", "hall"),
            tied("out", "out", "hall"),
            tied("letter", "write", "hall"),
            Task { office_hours: true, ..tied("ticked", "call", "hall") },
            Task { status: crate::tasks::Status::Completed, ..tied("done", "call", "hall") },
            Task { read_only: true, ..tied("shared", "call", "hall") },
            tied("other", "call", "elsewhere"),
            // Named as tasks are written since 26 November 2026.
            Task { contacts: vec![ContactRef { name: String::new(), uri: "urn:uuid:hall".into() }], ..tied("call-now", "call", "hall") },
        ];
        let open = CardHours { open: Some(Hours { value: "Mo-Fr 09:00-12:00".into(), ..Hours::default() }), ..CardHours::default() };
        let uids = |hours: &CardHours| to_tick(&tasks, "hall", hours).iter().map(|t| t.uid.clone()).collect::<Vec<_>>();
        assert_eq!(uids(&open), ["call", "out", "call-now"]);
        // Phone hours alone: the calls, not the outing.
        let phone = CardHours { phone: Some(Hours { value: "Mo-Fr 09:00-12:00".into(), ..Hours::default() }), ..CardHours::default() };
        assert_eq!(uids(&phone), ["call", "call-now"]);
        assert!(uids(&CardHours::default()).is_empty());
    }
}
