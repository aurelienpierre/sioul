// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Tasks: to-dos (VTODO, RFC 5545 §3.6.2) in CalDAV task lists, one per file,
//! with what ties them to the rest (RFC 9253, "iCalendar Relationships"):
//!
//! - **a step of a bigger task**: `RELATED-TO;RELTYPE=PARENT:<uid>`, in the step;
//! - **waits for another**: `RELATED-TO;RELTYPE=DEPENDS-ON:<uid>`, in the one
//!   that waits, as Sioul writes it. RFC 9253 §4 puts its temporal links in the
//!   one that comes first instead (`RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:<uid>`
//!   for "two weeks after"): read too, and written when there is a gap;
//! - **its notes, mail, drafts, events**: `LINK` (RFC 9253 §8.2): `mid:` for a
//!   message (RFC 2392), `sioul:` for the rest (`links`);
//! - **the people and offices it involves**: `CONTACT`, its `ALTREP` pointing at their card;
//! - **its case**: `REFID` (docs/case-store.md);
//! - **how long it takes**: `ESTIMATED-DURATION` (draft-ietf-calext-ical-tasks).
//!
//! `DTSTART` says from when a task can start, `DUE` the date asked from
//! outside. Neither turns into "overdue": the plan always starts from today
//! (`plan`, docs/tasks.md).
//!
//! Tasks are changed line by line (`lines`): what Sioul does not edit (alarms,
//! another application's fields) is kept exactly, and a form saved unchanged
//! writes nothing.

use crate::lines;
use crate::vdir::{self, Collection, Kind};
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use jiff::{Span, Zoned};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::path::{Path, PathBuf};

/// A task's state (RFC 5545 §3.8.1.11).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    #[default]
    NeedsAction,
    InProcess,
    Completed,
    Cancelled,
}

impl Status {
    fn read(value: &str) -> Status {
        match value.trim().to_ascii_uppercase().as_str() {
            "IN-PROCESS" => Status::InProcess,
            "COMPLETED" => Status::Completed,
            "CANCELLED" => Status::Cancelled,
            _ => Status::NeedsAction,
        }
    }

    fn written(self) -> &'static str {
        match self {
            Status::NeedsAction => "NEEDS-ACTION",
            Status::InProcess => "IN-PROCESS",
            Status::Completed => "COMPLETED",
            Status::Cancelled => "CANCELLED",
        }
    }

    /// Still to do: neither done nor dropped.
    pub fn is_open(self) -> bool {
        matches!(self, Status::NeedsAction | Status::InProcess)
    }
}

/// How a task is tied to another component (RELATED-TO, RFC 9253 §9.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Relation {
    /// "PARENT" (when unsaid), "CHILD", "SIBLING", "DEPENDS-ON", "FINISHTOSTART", "NEXT"…
    pub kind: String,
    pub uid: String,
    /// GAP, in minutes: the wait between one's end and the other's start; negative for a lead.
    pub gap: i64,
}

/// Something a task points at (LINK, RFC 9253 §8.2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Link {
    /// "mid:…", "sioul:note/…", "https://…"; "uid:<uid>" for another component by its UID.
    pub uri: String,
    #[serde(default)]
    pub label: String,
    /// The link relation (RFC 8288): "describedby" for a note, "via" for what it came from, "related".
    #[serde(default)]
    pub rel: String,
}

/// Someone a task involves (CONTACT, RFC 5545 §3.8.4.2): a name, and where their card is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContactRef {
    pub name: String,
    /// ALTREP: "sioul:contact/<card's UID>", or empty.
    #[serde(default)]
    pub uri: String,
}

/// The kinds of task, by what doing them takes: a call, writing, a form or
/// anything done on a website, going out, reading, thinking, making. Tasks of
/// one kind are easier done together.
pub const KINDS: &[&str] = &["call", "write", "online", "out", "read", "think", "make"];

/// A task's kind is written as a concept (RFC 9253, CONCEPT), under this
/// tag (RFC 4151): servers and other applications keep it as it is.
pub const KIND_CONCEPT: &str = "tag:aurelienpierre.com,2026:sioul/task-type/";

/// A task that can only be done while offices are open (a call to an office, a counter).
pub const OFFICE_CONCEPT: &str = "tag:aurelienpierre.com,2026:sioul/needs/office-hours";

/// Whether a word can name a kind: lowercase letters, digits and hyphens, as
/// Sioul's own kinds and those you add ("errand", "rendez-vous").
pub fn is_kind_id(kind: &str) -> bool {
    !kind.is_empty() && kind.len() <= 40 && kind.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The kind a concept names, when it is one of Sioul's.
fn kind_of_concept(value: &str) -> Option<String> {
    let kind = value.trim().strip_prefix(KIND_CONCEPT)?;
    is_kind_id(kind).then(|| kind.to_string())
}

/// One task, as read from its file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Task {
    /// The file, to open and change it.
    pub key: String,
    pub uid: String,
    pub title: String,
    pub notes: String,
    pub location: String,
    pub status: Status,
    /// From when it can start, in your time zone: "2026-10-05", or "2026-10-05T09:00".
    pub start: String,
    /// The date asked, in the same form.
    pub due: String,
    /// A time given to it for one day, by a drag in the day view (`X-SIOUL-AT`):
    /// "2026-10-06T14:30", in your time zone; "" for none. The day lays it then,
    /// that day only (`at_on`); another day it says nothing.
    pub at: String,
    /// When it was done, in Unix seconds.
    pub completed: Option<i64>,
    /// Minutes it takes, as estimated; 0 when unsaid.
    pub estimate: u32,
    /// The first estimate it was given (`X-SIOUL-ESTIMATE-FIRST`), written
    /// once and never changed by Sioul: what time spent is compared with
    /// (`capacity`). None for a task estimated before it was kept.
    pub estimate_first: Option<u32>,
    /// 1 (first) to 9 (last); 0 when unsaid (RFC 5545 §3.8.1.9).
    pub priority: u8,
    pub categories: Vec<String>,
    /// One of `KINDS`, or "" when unsaid.
    pub kind: String,
    /// Only while offices are open: proposed then, planned on their days.
    pub office_hours: bool,
    /// Its office's own opening hours (`X-SIOUL-OFFICE-HOURS`), as
    /// `window::parse_ranges` reads them: "mo-fr 09:00-12:00, 14:00-17:00";
    /// "" for offices' usual hours.
    pub office_times: String,
    /// What it is for (`X-SIOUL-AREA`): "work", "admin", "leisure", "personal";
    /// "" to go by its categories and projects (`areas::TaskAreas`).
    pub area: String,
    /// Its time billed or not (`X-SIOUL-BILLABLE`); None: as its project says
    /// (work for a client is billed).
    pub billable: Option<bool>,
    /// What it takes (`X-SIOUL-ENERGY`): "light", "heavy", or "rest" for what
    /// gives back (a walk, music); "" for the usual.
    pub energy: String,
    /// Minutes kept before and after it: getting there, getting ready (`demands`).
    pub margins: crate::demands::Margins,
    /// What it costs and gives back, rated 0 to 10 (`demands`).
    pub demands: crate::demands::Demands,
    /// How it went, said after it: one rating per day, the newest last, five at most (`demands::Felt`).
    pub felt: Vec<crate::demands::Felt>,
    pub relations: Vec<Relation>,
    pub links: Vec<Link>,
    pub contacts: Vec<ContactRef>,
    /// Its cases (REFID).
    pub cases: Vec<String>,
    /// "daily", "weekly", "monthly", "yearly" when it comes back; "" otherwise.
    pub repeat: String,
    /// When it was made (CREATED, else DTSTAMP), in Unix seconds: the order among equals.
    pub created: i64,
    /// Its list: name, "account/id", colour, and whether it can be changed.
    pub list: String,
    pub list_id: String,
    pub color: Option<String>,
    pub read_only: bool,
}

impl Task {
    /// The day it can start from.
    pub fn start_date(&self) -> Option<Date> {
        day_of(&self.start)
    }

    /// The day asked.
    pub fn due_date(&self) -> Option<Date> {
        day_of(&self.due)
    }

    /// The time given to it for `date` in the day view (Unix seconds, in
    /// `zone`), when it was given for that day: a time given another day says nothing.
    pub fn at_on(&self, date: Date, zone: &TimeZone) -> Option<i64> {
        if day_of(&self.at) != Some(date) {
            return None;
        }
        let local: DateTime = self.at.parse().ok()?;
        local.to_zoned(zone.clone()).ok().map(|z| z.timestamp().as_second())
    }

    /// The task it is a step of.
    pub fn parent(&self) -> Option<&str> {
        self.relations.iter().find(|r| r.kind == "PARENT").map(|r| r.uid.as_str())
    }

    /// The tasks it says it waits for (DEPENDS-ON).
    pub fn depends_on(&self) -> impl Iterator<Item = &Relation> {
        self.relations.iter().filter(|r| r.kind == "DEPENDS-ON")
    }
}

fn day_of(text: &str) -> Option<Date> {
    text.get(..10)?.parse().ok()
}

/// The first estimate a task was given, written once (`Task::estimate_first`).
pub const ESTIMATE_FIRST: &str = "X-SIOUL-ESTIMATE-FIRST";

/// A time given to a task for one day by a drag in the day view (`Task::at`),
/// in UTC. Not DTSTART: RFC 5545 wants DUE of DTSTART's value type and after
/// it, and CalDAV servers that check (Sabre: Nextcloud) refuse a task whose
/// start has a time while its date asked is a date, or has passed. Other
/// applications leave the line as it is: the time stays Sioul's.
pub const AT: &str = "X-SIOUL-AT";

/// An iCalendar duration in minutes (RFC 5545 §3.3.6): "PT15M" → 15,
/// "P1DT2H" → 1560, "-P1W" → −10080. None when it is not one.
pub fn minutes_of(text: &str) -> Option<i64> {
    let text = text.trim();
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text.strip_prefix('+').unwrap_or(text)),
    };
    let rest = rest.strip_prefix(['P', 'p'])?;
    let (mut total, mut number, mut seen) = (0i64, String::new(), false);
    for c in rest.chars() {
        match c.to_ascii_uppercase() {
            '0'..='9' => number.push(c),
            'T' => {}
            unit @ ('W' | 'D' | 'H' | 'M' | 'S') => {
                let n: i64 = number.parse().ok()?;
                number.clear();
                seen = true;
                // Too long to count (a file from elsewhere): not a duration.
                let minutes = match unit {
                    'W' => n.checked_mul(7 * 1440)?,
                    'D' => n.checked_mul(1440)?,
                    'H' => n.checked_mul(60)?,
                    'M' => n,
                    _ => n / 60,
                };
                total = total.checked_add(minutes)?;
            }
            _ => return None,
        }
    }
    (seen && number.is_empty()).then_some(sign * total)
}

/// "LIGHT", "heavy", "Rest" → "light", "heavy", "rest"; anything else, the usual: "".
pub fn energy_of(text: &str) -> String {
    let text = text.trim().to_ascii_lowercase();
    if matches!(text.as_str(), "light" | "heavy" | "rest") { text } else { String::new() }
}

/// Minutes as an iCalendar duration: 15 → "PT15M", 90 → "PT1H30M", 20160 → "P14D".
pub fn duration_of(minutes: i64) -> String {
    let sign = if minutes < 0 { "-" } else { "" };
    let m = minutes.abs();
    if m > 0 && m % 1440 == 0 {
        return format!("{sign}P{}D", m / 1440);
    }
    let (days, hours, mins) = (m / 1440, (m % 1440) / 60, m % 60);
    let mut out = format!("{sign}P");
    if days > 0 {
        out.push_str(&format!("{days}D"));
    }
    out.push('T');
    if hours > 0 {
        out.push_str(&format!("{hours}H"));
    }
    if mins > 0 || hours == 0 {
        out.push_str(&format!("{mins}M"));
    }
    out
}

/// The main VTODO, BEGIN to END: not a changed occurrence (RECURRENCE-ID).
fn master_range(source: &[String]) -> Option<Range<usize>> {
    let (mut start, mut nested, mut is_override) = (None, 0usize, false);
    for (i, line) in source.iter().enumerate() {
        let name = lines::name(line);
        let value = lines::value(line).trim().to_ascii_uppercase();
        match (name.as_str(), start) {
            ("BEGIN", None) if value == "VTODO" => {
                start = Some(i);
                nested = 0;
                is_override = false;
            }
            ("BEGIN", Some(_)) => nested += 1,
            ("END", Some(_)) if nested > 0 => nested -= 1,
            ("RECURRENCE-ID", Some(_)) if nested == 0 => is_override = true,
            ("END", Some(begin)) if value == "VTODO" => {
                if !is_override {
                    return Some(begin..i + 1);
                }
                start = None;
            }
            _ => {}
        }
    }
    None
}

/// The positions of the main VTODO's own lines: not its alarms'.
fn own_lines(source: &[String], range: &Range<usize>) -> Vec<usize> {
    let mut out = Vec::new();
    let mut nested = 0usize;
    for i in range.start + 1..range.end.saturating_sub(1) {
        match lines::name(&source[i]).as_str() {
            "BEGIN" => nested += 1,
            "END" => nested = nested.saturating_sub(1),
            _ if nested == 0 => out.push(i),
            _ => {}
        }
    }
    out
}

/// A DATE-TIME line in `zone`: UTC, with its TZID, or floating (taken in `zone`).
fn zoned(line: &str, zone: &TimeZone) -> Option<Zoned> {
    let value = lines::value(line).trim();
    let civil = DateTime::strptime("%Y%m%dT%H%M%S", value.trim_end_matches(['Z', 'z'])).ok()?;
    let source = if value.ends_with(['Z', 'z']) {
        TimeZone::UTC
    } else {
        lines::param(line, "TZID").and_then(|z| TimeZone::get(z.trim_start_matches('/')).ok()).unwrap_or_else(|| zone.clone())
    };
    civil.to_zoned(source).ok().map(|z| z.with_time_zone(zone.clone()))
}

/// A DTSTART or DUE line in your time zone: "2026-10-30" for a date,
/// "2026-10-30T17:00" for a time.
fn moment(line: &str, zone: &TimeZone) -> Option<String> {
    let value = lines::value(line).trim();
    let is_date = lines::param(line, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE")) || value.len() == 8;
    if is_date {
        return Date::strptime("%Y%m%d", value.get(..8)?).ok().map(|d| d.to_string());
    }
    zoned(line, zone).map(|z| z.strftime("%Y-%m-%dT%H:%M").to_string())
}

/// A comma-separated text list (CATEGORIES), its escaped commas kept inside.
fn split_list(value: &str) -> Vec<String> {
    let (mut out, mut current, mut escaped) = (Vec::new(), String::new(), false);
    for c in value.chars() {
        if escaped {
            current.push('\\');
            current.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            ',' => out.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    out.push(current);
    out.into_iter().map(|p| lines::unescape(p.trim())).filter(|p| !p.is_empty()).collect()
}

/// A RELATED-TO line, read.
pub fn relation_of(line: &str) -> Relation {
    let kind = lines::param(line, "RELTYPE").map(|k| k.trim().to_ascii_uppercase()).filter(|k| !k.is_empty()).unwrap_or_else(|| "PARENT".into());
    let gap = lines::param(line, "GAP").and_then(|g| minutes_of(&g)).unwrap_or(0);
    Relation { kind, uid: lines::value(line).trim().to_string(), gap }
}

/// A LINK line, read.
pub fn link_of(line: &str) -> Link {
    let value = lines::value(line).trim().to_string();
    let by_uid = lines::param(line, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("UID"));
    Link { uri: if by_uid { format!("uid:{value}") } else { value }, label: lines::param(line, "LABEL").unwrap_or_default(), rel: lines::param(line, "LINKREL").unwrap_or_default() }
}

fn contact(line: &str) -> ContactRef {
    ContactRef { name: lines::unescape(lines::value(line).trim()), uri: lines::param(line, "ALTREP").unwrap_or_default() }
}

/// "FREQ=WEEKLY;INTERVAL=2" → "weekly"; a rule without one of the four frequencies → "".
fn repeat_of(rule: &str) -> String {
    let frequency = rule.split(';').find_map(|p| p.trim().to_ascii_uppercase().strip_prefix("FREQ=").map(str::to_string)).unwrap_or_default();
    match frequency.as_str() {
        "DAILY" | "WEEKLY" | "MONTHLY" | "YEARLY" => frequency.to_lowercase(),
        _ => String::new(),
    }
}

/// A task from the text of its file, its dates in `zone`; None when it holds no task.
pub fn task_of_text(text: &str, zone: &TimeZone) -> Option<Task> {
    let source = lines::unfold(text);
    let range = master_range(&source)?;
    let mut task = Task::default();
    let (mut duration, mut stamp, mut start_line, mut has_status) = (None, 0, None, false);
    let mut felt: Vec<(String, Option<Date>, String)> = Vec::new();
    for i in own_lines(&source, &range) {
        let line = &source[i];
        let value = lines::value(line);
        match lines::name(line).as_str() {
            "UID" => task.uid = value.trim().to_string(),
            "SUMMARY" => task.title = lines::unescape(value.trim()),
            "DESCRIPTION" => task.notes = lines::unescape(value.trim()),
            "LOCATION" => task.location = lines::unescape(value.trim()),
            "STATUS" => {
                task.status = Status::read(value);
                has_status = true;
            }
            "DTSTART" => {
                task.start = moment(line, zone).unwrap_or_default();
                start_line = Some(line.clone());
            }
            "DUE" => task.due = moment(line, zone).unwrap_or_default(),
            "DURATION" => duration = minutes_of(value),
            "COMPLETED" => task.completed = zoned(line, zone).map(|z| z.timestamp().as_second()),
            "ESTIMATED-DURATION" => task.estimate = minutes_of(value).map_or(0, |m| u32::try_from(m.max(0)).unwrap_or(u32::MAX)),
            "PRIORITY" => task.priority = value.trim().parse::<u8>().unwrap_or(0).min(9),
            "CATEGORIES" => task.categories.extend(split_list(value)),
            "CONCEPT" => {
                if let Some(kind) = kind_of_concept(value) {
                    task.kind = kind;
                }
                if value.trim() == OFFICE_CONCEPT {
                    task.office_hours = true;
                }
            }
            "RELATED-TO" => task.relations.push(relation_of(line)),
            "X-SIOUL-BILLABLE" => task.billable = Some(value.trim().eq_ignore_ascii_case("TRUE")),
            "X-SIOUL-ENERGY" => task.energy = energy_of(value),
            "X-SIOUL-OFFICE-HOURS" => task.office_times = crate::window::ranges_text(&crate::window::parse_ranges(&lines::unescape(value))),
            "X-SIOUL-AREA" => task.area = crate::areas::Area::parse(value).map(|a| a.id()).unwrap_or_default(),
            crate::demands::BEFORE => task.margins.before = crate::demands::minutes_of(value),
            crate::demands::AFTER => task.margins.after = crate::demands::minutes_of(value),
            crate::demands::COST => task.demands.read_cost(value),
            crate::demands::GAIN => task.demands.read_gain(value),
            name @ (crate::demands::FELT_COST | crate::demands::FELT_GAIN) => felt.push((name.to_string(), lines::param(line, crate::demands::FELT_ON).and_then(|d| crate::demands::felt_date(&d)), value.to_string())),
            ESTIMATE_FIRST => task.estimate_first = minutes_of(value).and_then(|m| u32::try_from(m).ok()).filter(|m| *m > 0),
            AT => task.at = zoned(line, zone).map(|z| z.strftime("%Y-%m-%dT%H:%M").to_string()).unwrap_or_default(),
            "LINK" => task.links.push(link_of(line)),
            "CONTACT" => task.contacts.push(contact(line)),
            "REFID" => task.cases.push(lines::unescape(value.trim())),
            "RRULE" => task.repeat = repeat_of(value),
            "CREATED" => task.created = zoned(line, zone).map_or(0, |z| z.timestamp().as_second()),
            "DTSTAMP" => stamp = zoned(line, zone).map_or(0, |z| z.timestamp().as_second()),
            _ => {}
        }
    }
    if task.created == 0 {
        task.created = stamp;
    }
    task.felt = crate::demands::felt_of(&felt);
    // DURATION with DTSTART stands for DUE (RFC 5545 §3.6.2).
    if task.due.is_empty()
        && let (Some(minutes), Some(line)) = (duration, start_line)
    {
        // A duration too long for a date gives none, never a crash.
        task.due = match (task.start.len(), task.start_date()) {
            (10, Some(date)) => Span::new().try_days(minutes / 1440).ok().and_then(|span| date.checked_add(span).ok()).map(|d| d.to_string()).unwrap_or_default(),
            _ => zoned(&line, zone).zip(Span::new().try_minutes(minutes).ok()).and_then(|(z, span)| z.checked_add(span).ok()).map(|z| z.strftime("%Y-%m-%dT%H:%M").to_string()).unwrap_or_default(),
        };
    }
    // Older clients say done with COMPLETED alone.
    if task.completed.is_some() && !has_status {
        task.status = Status::Completed;
    }
    Some(task)
}

/// What the task form gives back, and what a task is as the form shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEdit {
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub status: Status,
    /// "", "2026-10-05" or "2026-10-05T09:00", in your time zone.
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub due: String,
    /// A time given for one day in the day view: "2026-10-06T14:30", or "" (`Task::at`).
    #[serde(default)]
    pub at: String,
    /// Minutes; 0 when unsaid.
    #[serde(default)]
    pub estimate: u32,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub categories: Vec<String>,
    /// One of `KINDS`, or "".
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub office_hours: bool,
    /// Its office's opening hours: "mo-fr 09:00-17:00"; "" for the usual.
    #[serde(default)]
    pub office_times: String,
    /// "work", "admin", "leisure", "personal"; "" to go by its categories.
    #[serde(default)]
    pub area: String,
    /// Billed or not; None: as its project says.
    #[serde(default)]
    pub billable: Option<bool>,
    /// "light", "heavy", "rest", or "" for the usual.
    #[serde(default)]
    pub energy: String,
    /// Minutes kept before and after it.
    #[serde(default)]
    pub margins: crate::demands::Margins,
    /// What it costs and gives back, 0 to 10 each; none unsaid.
    #[serde(default)]
    pub demands: crate::demands::Demands,
    /// How it went, the newest rating said after it: shown, never written by
    /// the form (`set_felt` writes it, dated).
    #[serde(default)]
    pub felt: crate::demands::Demands,
    /// The task it is a step of, by UID; "" for none.
    #[serde(default)]
    pub parent: String,
    /// The tasks it waits for, by UID (DEPENDS-ON).
    #[serde(default)]
    pub waits_for: Vec<String>,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub contacts: Vec<ContactRef>,
    #[serde(default)]
    pub cases: Vec<String>,
    /// "", "daily", "weekly", "monthly", "yearly".
    #[serde(default)]
    pub repeat: String,
}

impl TaskEdit {
    /// A task as the form shows it.
    pub fn of(task: &Task) -> TaskEdit {
        TaskEdit {
            title: task.title.clone(),
            notes: task.notes.clone(),
            location: task.location.clone(),
            status: task.status,
            start: task.start.clone(),
            due: task.due.clone(),
            at: task.at.clone(),
            estimate: task.estimate,
            priority: task.priority,
            categories: task.categories.clone(),
            kind: task.kind.clone(),
            office_hours: task.office_hours,
            office_times: task.office_times.clone(),
            area: task.area.clone(),
            billable: task.billable,
            energy: task.energy.clone(),
            margins: task.margins,
            demands: task.demands,
            felt: task.felt.last().map(|f| f.demands).unwrap_or_default(),
            parent: task.parent().unwrap_or_default().to_string(),
            waits_for: task.depends_on().map(|r| r.uid.clone()).collect(),
            links: task.links.clone(),
            contacts: task.contacts.clone(),
            cases: task.cases.clone(),
            repeat: task.repeat.clone(),
        }
    }

    /// Trimmed, without empty or repeated entries, as it will be read back.
    fn tidy(&self) -> TaskEdit {
        fn unique<T: PartialEq + Clone>(items: impl Iterator<Item = T>) -> Vec<T> {
            let mut out: Vec<T> = Vec::new();
            for item in items {
                if !out.contains(&item) {
                    out.push(item);
                }
            }
            out
        }
        TaskEdit {
            title: self.title.trim().to_string(),
            notes: self.notes.trim().to_string(),
            location: self.location.trim().to_string(),
            start: self.start.trim().to_string(),
            due: self.due.trim().to_string(),
            at: self.at.trim().to_string(),
            priority: self.priority.min(9),
            categories: unique(self.categories.iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty())),
            kind: Some(self.kind.trim()).filter(|k| is_kind_id(k)).map(str::to_string).unwrap_or_default(),
            office_hours: self.office_hours,
            office_times: if self.office_hours { crate::window::ranges_text(&crate::window::parse_ranges(&self.office_times)) } else { String::new() },
            area: crate::areas::Area::parse(&self.area).map(|a| a.id()).unwrap_or_default(),
            margins: crate::demands::Margins { before: self.margins.before.min(24 * 60), after: self.margins.after.min(24 * 60) },
            demands: self.demands.bounded(),
            felt: self.felt.bounded(),
            // Heavy or light follows every cost once one is rated: its highest (docs/capacity.md).
            energy: self.demands.level().map_or_else(|| energy_of(&self.energy), |level| level.energy().to_string()),
            parent: self.parent.trim().to_string(),
            waits_for: unique(self.waits_for.iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty())),
            links: unique(self.links.iter().filter(|l| !l.uri.trim().is_empty()).map(|l| Link { uri: l.uri.trim().to_string(), label: l.label.trim().to_string(), rel: l.rel.trim().to_string() })),
            contacts: unique(self.contacts.iter().filter(|c| !c.name.trim().is_empty() || !c.uri.trim().is_empty()).map(|c| ContactRef { name: c.name.trim().to_string(), uri: c.uri.trim().to_string() })),
            cases: unique(self.cases.iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty())),
            repeat: self.repeat.trim().to_lowercase(),
            ..self.clone()
        }
    }
}

fn utc(at: &Zoned) -> String {
    at.timestamp().strftime("%Y%m%dT%H%M%SZ").to_string()
}

/// A DTSTART or DUE line: a date as a DATE, a time in UTC.
fn date_line(name: &str, text: &str, zone: &TimeZone) -> Result<String, String> {
    let bad = || format!("{name}: {text}?");
    if text.len() == 10 {
        let date: Date = text.parse().map_err(|_| bad())?;
        return Ok(format!("{name};VALUE=DATE:{}", date.strftime("%Y%m%d")));
    }
    let local: DateTime = text.parse().map_err(|_| bad())?;
    Ok(format!("{name}:{}", utc(&local.to_zoned(zone.clone()).map_err(|_| bad())?)))
}

/// A value written as it is (a UID, an address): without line breaks or
/// other control characters, which would end its line and start another
/// (an address decoded from `%0D%0A`, a key from an imported file).
fn one_line(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
}

/// `RELATED-TO;RELTYPE=<kind>[;GAP=…]:<uid>`.
pub fn relation_line(kind: &str, uid: &str, gap_minutes: i64) -> String {
    let gap = if gap_minutes == 0 { String::new() } else { format!(";GAP={}", duration_of(gap_minutes)) };
    format!("RELATED-TO;RELTYPE={kind}{gap}:{}", one_line(uid))
}

/// `LINK;LINKREL=…;LABEL=…;VALUE=URI:<uri>`, or `VALUE=UID` for "uid:<uid>".
pub fn link_line(link: &Link) -> String {
    let rel = if link.rel.is_empty() { "related".to_string() } else { lines::param_value(&link.rel) };
    let label = if link.label.is_empty() { String::new() } else { format!(";LABEL={}", lines::param_value(&link.label)) };
    let uri = one_line(&link.uri);
    match uri.strip_prefix("uid:") {
        Some(uid) => format!("LINK;LINKREL={rel}{label};VALUE=UID:{uid}"),
        None => format!("LINK;LINKREL={rel}{label};VALUE=URI:{uri}"),
    }
}

/// `CONTACT;ALTREP="<uri>":<name>`.
pub fn contact_line(contact: &ContactRef) -> String {
    let altrep = if contact.uri.is_empty() { String::new() } else { format!(";ALTREP=\"{}\"", one_line(&contact.uri).replace('"', "%22")) };
    format!("CONTACT{altrep}:{}", lines::escape(&contact.name))
}

/// The status lines: STATUS, and COMPLETED with PERCENT-COMPLETE when done.
fn status_lines(status: Status, now: &Zoned) -> Vec<String> {
    let mut out = vec![format!("STATUS:{}", status.written())];
    if status == Status::Completed {
        out.extend([format!("COMPLETED:{}", utc(now)), "PERCENT-COMPLETE:100".to_string()]);
    }
    out
}

/// What the form changed, field by field.
#[derive(Clone, Copy)]
struct Changed {
    title: bool,
    notes: bool,
    location: bool,
    status: bool,
    start: bool,
    due: bool,
    at: bool,
    estimate: bool,
    priority: bool,
    categories: bool,
    kind: bool,
    office: bool,
    office_times: bool,
    area: bool,
    billable: bool,
    energy: bool,
    margins: bool,
    demands: bool,
    repeat: bool,
}

impl Changed {
    fn between(old: &TaskEdit, new: &TaskEdit) -> Changed {
        Changed {
            title: old.title != new.title,
            notes: old.notes != new.notes,
            location: old.location != new.location,
            status: old.status != new.status,
            start: old.start != new.start,
            due: old.due != new.due,
            at: old.at != new.at,
            estimate: old.estimate != new.estimate,
            priority: old.priority != new.priority,
            categories: old.categories != new.categories,
            kind: old.kind != new.kind,
            office: old.office_hours != new.office_hours,
            office_times: old.office_times != new.office_times || old.office_hours != new.office_hours,
            area: old.area != new.area,
            billable: old.billable != new.billable,
            energy: old.energy != new.energy,
            margins: old.margins != new.margins,
            demands: old.demands != new.demands,
            repeat: old.repeat != new.repeat,
        }
    }

    const ALL: Changed = Changed { title: true, notes: true, location: true, status: true, start: true, due: true, at: true, estimate: true, priority: true, categories: true, kind: true, office: true, office_times: true, area: true, billable: true, energy: true, margins: true, demands: true, repeat: true };

    /// Whether a line of this name is written again.
    fn rewrites(&self, name: &str) -> bool {
        match name {
            "SUMMARY" => self.title,
            "DESCRIPTION" => self.notes,
            "LOCATION" => self.location,
            "STATUS" | "COMPLETED" | "PERCENT-COMPLETE" => self.status,
            "DTSTART" => self.start || self.due,
            "DUE" | "DURATION" => self.start || self.due,
            AT => self.at,
            "ESTIMATED-DURATION" => self.estimate,
            "PRIORITY" => self.priority,
            "CATEGORIES" => self.categories,
            "X-SIOUL-BILLABLE" => self.billable,
            "X-SIOUL-ENERGY" => self.energy,
            "X-SIOUL-OFFICE-HOURS" => self.office_times,
            "X-SIOUL-AREA" => self.area,
            crate::demands::BEFORE | crate::demands::AFTER => self.margins,
            crate::demands::COST | crate::demands::GAIN => self.demands,
            "RRULE" => self.repeat,
            _ => false,
        }
    }
}

/// The lines of the fields that changed, as the form has them.
fn field_lines(edit: &TaskEdit, changed: Changed, zone: &TimeZone, now: &Zoned) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    if changed.title {
        out.push(format!("SUMMARY:{}", lines::escape(&edit.title)));
    }
    if changed.notes && !edit.notes.is_empty() {
        out.push(format!("DESCRIPTION:{}", lines::escape(&edit.notes)));
    }
    if changed.location && !edit.location.is_empty() {
        out.push(format!("LOCATION:{}", lines::escape(&edit.location)));
    }
    if changed.status {
        out.extend(status_lines(edit.status, now));
    }
    if (changed.start || changed.due) && !edit.start.is_empty() {
        out.push(date_line("DTSTART", &edit.start, zone)?);
    }
    if (changed.start || changed.due) && !edit.due.is_empty() {
        out.push(date_line("DUE", &edit.due, zone)?);
    }
    if changed.at && edit.at.len() > 10 {
        out.push(date_line(AT, &edit.at, zone)?);
    }
    if changed.estimate && edit.estimate > 0 {
        out.push(format!("ESTIMATED-DURATION:{}", duration_of(i64::from(edit.estimate))));
    }
    if changed.priority && edit.priority > 0 {
        out.push(format!("PRIORITY:{}", edit.priority));
    }
    if changed.categories && !edit.categories.is_empty() {
        out.push(format!("CATEGORIES:{}", edit.categories.iter().map(|c| lines::escape(c)).collect::<Vec<_>>().join(",")));
    }
    if changed.kind && !edit.kind.is_empty() {
        out.push(format!("CONCEPT:{KIND_CONCEPT}{}", edit.kind));
    }
    if changed.office && edit.office_hours {
        out.push(format!("CONCEPT:{OFFICE_CONCEPT}"));
    }
    if changed.office_times && edit.office_hours && !edit.office_times.is_empty() {
        out.push(format!("X-SIOUL-OFFICE-HOURS:{}", lines::escape(&edit.office_times)));
    }
    if changed.area && !edit.area.is_empty() {
        out.push(format!("X-SIOUL-AREA:{}", edit.area.to_ascii_uppercase()));
    }
    if changed.billable && let Some(billable) = edit.billable {
        out.push(format!("X-SIOUL-BILLABLE:{}", if billable { "TRUE" } else { "FALSE" }));
    }
    if changed.energy && !energy_of(&edit.energy).is_empty() {
        out.push(format!("X-SIOUL-ENERGY:{}", energy_of(&edit.energy).to_ascii_uppercase()));
    }
    if changed.margins {
        if edit.margins.before > 0 {
            out.push(format!("{}:{}", crate::demands::BEFORE, edit.margins.before));
        }
        if edit.margins.after > 0 {
            out.push(format!("{}:{}", crate::demands::AFTER, edit.margins.after));
        }
    }
    if changed.demands {
        if edit.demands.has_cost() {
            out.push(format!("{}:{}", crate::demands::COST, edit.demands.cost_value()));
        }
        if let Some(gain) = edit.demands.gain {
            out.push(format!("{}:{gain}", crate::demands::GAIN));
        }
    }
    if changed.repeat && matches!(edit.repeat.as_str(), "daily" | "weekly" | "monthly" | "yearly") {
        out.push(format!("RRULE:FREQ={}", edit.repeat.to_ascii_uppercase()));
    }
    Ok(out)
}

/// A new task, with its own UID.
pub fn new_task(edit: &TaskEdit, uid: &str, zone: &TimeZone, now: &Zoned) -> Result<String, String> {
    let edit = edit.tidy();
    let mut out = vec!["BEGIN:VCALENDAR".to_string(), "VERSION:2.0".to_string(), "PRODID:-//Sioul//Sioul//EN".to_string(), "BEGIN:VTODO".to_string()];
    out.extend([format!("UID:{}", one_line(uid)), format!("DTSTAMP:{}", utc(now)), format!("CREATED:{}", utc(now)), format!("LAST-MODIFIED:{}", utc(now))]);
    out.extend(field_lines(&edit, Changed::ALL, zone, now)?);
    if edit.estimate > 0 {
        out.push(format!("{ESTIMATE_FIRST}:{}", duration_of(i64::from(edit.estimate))));
    }
    if !edit.parent.is_empty() {
        out.push(relation_line("PARENT", &edit.parent, 0));
    }
    out.extend(edit.waits_for.iter().map(|uid| relation_line("DEPENDS-ON", uid, 0)));
    out.extend(edit.links.iter().map(link_line));
    out.extend(edit.contacts.iter().map(contact_line));
    out.extend(edit.cases.iter().map(|c| format!("REFID:{}", lines::escape(c))));
    out.extend(["END:VTODO".to_string(), "END:VCALENDAR".to_string()]);
    Ok(lines::fold(&out))
}

/// Lines rewritten at every change (RFC 5545 §3.8.7).
const STAMPS: &[&str] = &["DTSTAMP", "LAST-MODIFIED", "SEQUENCE"];

/// The task's text with the main VTODO's lines that `keep` refuses dropped and
/// `added` put in, before its alarms; the stamps written again. Nothing
/// dropped and nothing added: the text as it was.
fn rewrite(text: &str, keep: impl Fn(&str) -> bool, added: Vec<String>, now: &Zoned) -> Result<String, String> {
    let source = lines::unfold(text);
    let range = master_range(&source).ok_or("no task")?;
    let own = own_lines(&source, &range);
    let dropped: Vec<usize> = own.iter().copied().filter(|&i| !STAMPS.contains(&lines::name(&source[i]).as_str()) && !keep(&source[i])).collect();
    if dropped.is_empty() && added.is_empty() {
        return Ok(text.to_string());
    }
    let sequence = own.iter().find(|&&i| lines::name(&source[i]) == "SEQUENCE").and_then(|&i| lines::value(&source[i]).trim().parse::<u32>().ok()).unwrap_or(0);
    let stamps: Vec<usize> = own.iter().copied().filter(|&i| STAMPS.contains(&lines::name(&source[i]).as_str())).collect();
    // New lines go after the task's last own line, before its alarms.
    let at = own.last().map_or(range.start + 1, |&i| i + 1);
    let mut out = Vec::with_capacity(source.len() + added.len() + 3);
    for (i, line) in source.iter().enumerate() {
        if i == at {
            out.extend([format!("DTSTAMP:{}", utc(now)), format!("LAST-MODIFIED:{}", utc(now)), format!("SEQUENCE:{}", sequence.saturating_add(1))]);
            out.extend(added.iter().cloned());
        }
        if !dropped.contains(&i) && !stamps.contains(&i) {
            out.push(line.clone());
        }
    }
    Ok(lines::fold(&out))
}

/// The task's text with the form's changes: only the lines of what changed
/// are written again; nothing changed, nothing is written.
pub fn apply(text: &str, edit: &TaskEdit, zone: &TimeZone, now: &Zoned) -> Result<String, String> {
    let before = task_of_text(text, zone).ok_or("no task")?;
    let (mut old, mut edit) = (TaskEdit::of(&before).tidy(), edit.tidy());
    // The word as the file has it: rated costs then write their level over a stale one.
    old.energy = before.energy.clone();
    // How it went is written by `set_felt` only, never by the form.
    edit.felt = old.felt;
    if old == edit {
        return Ok(text.to_string());
    }
    // A time given for a day gone by says nothing: written for another reason, the task leaves it.
    if day_of(&edit.at).is_some_and(|day| day < now.date()) {
        edit.at = String::new();
    }
    let changed = Changed::between(&old, &edit);
    let mut added = field_lines(&edit, changed, zone, now)?;
    // Its first estimate, the first time it has one; never changed after.
    if changed.estimate && before.estimate == 0 && before.estimate_first.is_none() && edit.estimate > 0 {
        added.push(format!("{ESTIMATE_FIRST}:{}", duration_of(i64::from(edit.estimate))));
    }
    if edit.parent != old.parent && !edit.parent.is_empty() {
        added.push(relation_line("PARENT", &edit.parent, 0));
    }
    added.extend(edit.waits_for.iter().filter(|u| !old.waits_for.contains(u)).map(|uid| relation_line("DEPENDS-ON", uid, 0)));
    added.extend(edit.links.iter().filter(|l| !old.links.contains(l)).map(link_line));
    added.extend(edit.contacts.iter().filter(|c| !old.contacts.contains(c)).map(contact_line));
    added.extend(edit.cases.iter().filter(|c| !old.cases.contains(c)).map(|c| format!("REFID:{}", lines::escape(c))));
    let keep = |line: &str| {
        let name = lines::name(line);
        match name.as_str() {
            "RELATED-TO" => {
                let r = relation_of(line);
                match r.kind.as_str() {
                    "PARENT" => r.uid == edit.parent,
                    "DEPENDS-ON" => edit.waits_for.contains(&r.uid),
                    _ => true,
                }
            }
            "LINK" => edit.links.contains(&link_of(line)),
            "CONTACT" => edit.contacts.contains(&contact(line)),
            "REFID" => edit.cases.contains(&lines::unescape(lines::value(line).trim())),
            // Only Sioul's own concepts, the kind and the office hours, are written again; others stay.
            "CONCEPT" => {
                let value = lines::value(line);
                !((changed.kind && kind_of_concept(value).is_some()) || (changed.office && value.trim() == OFFICE_CONCEPT))
            }
            other => !changed.rewrites(other),
        }
    };
    rewrite(text, keep, added, now)
}

/// The task marked done, started, open again or dropped. A repeating task
/// done comes back instead, its dates moved to their next turn after today.
pub fn set_status(text: &str, status: Status, zone: &TimeZone, now: &Zoned) -> Result<String, String> {
    let task = task_of_text(text, zone).ok_or("no task")?;
    let mut edit = TaskEdit::of(&task);
    edit.status = status;
    if status == Status::Completed && !task.repeat.is_empty() {
        let source = lines::unfold(text);
        let rule = source.iter().find(|l| lines::name(l) == "RRULE").map(|l| lines::value(l).to_string()).unwrap_or_default();
        let (start, due) = next_turn(&task, &rule, now.date());
        // A rule that ends (UNTIL, as another application may write it) before the next turn: done for good.
        let until = rule.to_ascii_uppercase().split(';').find_map(|p| p.trim().strip_prefix("UNTIL=").and_then(|v| Date::strptime("%Y%m%d", v.get(..8)?).ok()));
        if !until.is_some_and(|until| day_of(&start).or_else(|| day_of(&due)).is_some_and(|next| next > until)) {
            edit.start = start;
            edit.due = due;
            // A time given for one day in the day view stays with that day.
            edit.at = String::new();
            edit.status = Status::NeedsAction;
        }
    }
    apply(text, &edit, zone, now)
}

/// A repeating task's dates at their next turn after `today`, the distance
/// between them kept; one without dates waits one turn from today.
fn next_turn(task: &Task, rule: &str, today: Date) -> (String, String) {
    let upper = rule.to_ascii_uppercase();
    let interval: i64 = upper.split(';').find_map(|p| p.trim().strip_prefix("INTERVAL=").and_then(|n| n.parse().ok())).unwrap_or(1).max(1);
    // `k` turns later; none when the rule (the file's) goes past any date.
    let step = |k: i64| -> Option<Span> {
        let n = k.checked_mul(interval)?;
        match task.repeat.as_str() {
            "daily" => Span::new().try_days(n),
            "weekly" => Span::new().try_weeks(n),
            "monthly" => Span::new().try_months(n),
            _ => Span::new().try_years(n),
        }
        .ok()
    };
    let moved = |text: &str, k: i64| -> String {
        let Some(date) = day_of(text) else { return String::new() };
        let Some(next) = step(k).and_then(|span| date.checked_add(span).ok()) else { return text.to_string() };
        format!("{next}{}", text.get(10..).unwrap_or(""))
    };
    let Some(anchor) = task.due_date().or_else(|| task.start_date()) else {
        return (step(1).and_then(|span| today.checked_add(span).ok()).map(|d| d.to_string()).unwrap_or_default(), String::new());
    };
    let k = (1..=10_000).find(|&k| step(k).and_then(|span| anchor.checked_add(span).ok()).is_some_and(|d| d > today)).unwrap_or(1);
    (moved(&task.start, k), moved(&task.due, k))
}

/// The felt ratings kept in a task's file: the newest five days.
pub const FELT_KEPT: usize = 5;

/// How it went, said after the task on `on`: that day's rating replaced (a
/// rating with nothing said takes it away), the newest `FELT_KEPT` days
/// kept, everything else left as it is.
pub fn set_felt(text: &str, felt: &crate::demands::Demands, on: Date, now: &Zoned) -> Result<String, String> {
    use crate::demands::{FELT_COST, FELT_GAIN, FELT_ON, felt_date};
    let felt = felt.bounded();
    let source = lines::unfold(text);
    let range = master_range(&source).ok_or("no task")?;
    let is_felt = |line: &str| matches!(lines::name(line).as_str(), FELT_COST | FELT_GAIN);
    let date_of = |line: &str| lines::param(line, FELT_ON).and_then(|d| felt_date(&d));
    // The days rated, other than `on`, newest first; what passes the newest kept, dropped.
    let mut days: Vec<Option<Date>> = own_lines(&source, &range).into_iter().map(|i| &source[i]).filter(|l| is_felt(l)).map(|l| date_of(l)).filter(|d| *d != Some(on)).collect();
    days.sort_unstable_by(|a, b| b.cmp(a));
    days.dedup();
    let room = if felt.is_empty() { FELT_KEPT } else { FELT_KEPT - 1 };
    let kept: Vec<Option<Date>> = days.into_iter().take(room).collect();
    let day = on.strftime("%Y%m%d").to_string();
    let mut added = Vec::new();
    if felt.has_cost() {
        added.push(format!("{FELT_COST};{FELT_ON}={day}:{}", felt.cost_value()));
    }
    if let Some(gain) = felt.gain {
        added.push(format!("{FELT_GAIN};{FELT_ON}={day}:{gain}"));
    }
    let drop = |line: &str| is_felt(line) && (date_of(line) == Some(on) || !kept.contains(&date_of(line)));
    // The same rating said again, nothing past the newest kept: the text as it was.
    let dropped: Vec<String> = own_lines(&source, &range).into_iter().map(|i| source[i].clone()).filter(|l| drop(l)).collect();
    if dropped == added {
        return Ok(text.to_string());
    }
    replace_lines(text, drop, added, now)
}

/// A task done at another time than now (an import of what was done before):
/// its COMPLETED line set to `at`; the text as it was when it already says so.
pub fn completed_at(text: &str, at: &Zoned) -> Result<String, String> {
    let wanted = format!("COMPLETED:{}", utc(at));
    if lines::unfold(text).contains(&wanted) {
        return Ok(text.to_string());
    }
    let without = rewrite(text, |l| lines::name(l) != "COMPLETED", Vec::new(), at)?;
    rewrite(&without, |_| true, vec![wanted], at)
}

/// The main VTODO with `added` lines, those not there yet; the text as it was when all are.
pub fn add_lines(text: &str, added: &[String], now: &Zoned) -> Result<String, String> {
    let present: Vec<String> = lines::unfold(text);
    let new: Vec<String> = added.iter().filter(|l| !present.contains(l)).cloned().collect();
    rewrite(text, |_| true, new, now)
}

/// The main VTODO without the lines `drop` picks; the text as it was when none is.
pub fn remove_lines(text: &str, drop: impl Fn(&str) -> bool, now: &Zoned) -> Result<String, String> {
    rewrite(text, |l| !drop(l), Vec::new(), now)
}

/// The main VTODO's lines that `drop` takes replaced by `added`, in one
/// change: the stamps written once.
pub fn replace_lines(text: &str, drop: impl Fn(&str) -> bool, added: Vec<String>, now: &Zoned) -> Result<String, String> {
    rewrite(text, |l| !drop(l), added, now)
}

/// The calendars that hold tasks.
pub fn lists() -> Vec<Collection> {
    vdir::collections(Kind::Calendars).into_iter().filter(|c| c.holds("VTODO")).collect()
}

/// One task from its file; None when the file holds an event.
pub fn read(path: &Path, list: &Collection, zone: &TimeZone) -> Option<Task> {
    let text = std::fs::read_to_string(path).ok()?;
    if !text.to_ascii_uppercase().contains("BEGIN:VTODO") {
        return None;
    }
    let mut task = task_of_text(&text, zone)?;
    task.key = path.display().to_string();
    task.list = list.name.clone();
    task.list_id = format!("{}/{}", list.account, list.id);
    task.color = list.color.clone();
    task.read_only = list.read_only;
    Some(task)
}

/// Every task of every list.
pub fn all(zone: &TimeZone) -> Vec<Task> {
    lists().iter().flat_map(|list| list.items().into_iter().filter_map(|path| read(&path, list, zone)).collect::<Vec<_>>()).collect()
}

/// Where a new task goes: a writable list made for tasks, then one that also
/// takes them, then one that does not say.
pub fn default_list() -> Option<Collection> {
    let writable: Vec<Collection> = lists().into_iter().filter(|c| !c.read_only).collect();
    let only_tasks = |c: &&Collection| c.components.len() == 1 && c.holds("VTODO");
    let says_tasks = |c: &&Collection| c.components.iter().any(|x| x.eq_ignore_ascii_case("VTODO"));
    writable.iter().find(only_tasks).or_else(|| writable.iter().find(says_tasks)).or(writable.first()).cloned()
}

/// The file a new task gets in a list.
pub fn new_path(list: &Collection) -> PathBuf {
    list.dir.join(format!("{}.ics", vdir::new_name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_is_a_concept() {
        let zone = TimeZone::UTC;
        let now = Zoned::now();
        let made = new_task(&TaskEdit { title: "Call the bank".into(), kind: "call".into(), ..TaskEdit::default() }, "k", &zone, &now).unwrap();
        assert!(made.contains("CONCEPT:tag:aurelienpierre.com,2026:sioul/task-type/call"), "{made}");
        let other = made.replace("END:VTODO", "CONCEPT:https://example.org/concepts/money\r\nEND:VTODO");
        let task = task_of_text(&other, &zone).unwrap();
        assert_eq!(task.kind, "call");
        let changed = apply(&other, &TaskEdit { kind: "online".into(), ..TaskEdit::of(&task) }, &zone, &now).unwrap();
        let task = task_of_text(&changed, &zone).unwrap();
        assert_eq!(task.kind, "online");
        assert!(changed.contains("CONCEPT:https://example.org/concepts/money"), "another application's concept stays");
        assert!(!changed.contains("task-type/call"));
        let unsaid = apply(&changed, &TaskEdit { kind: String::new(), ..TaskEdit::of(&task) }, &zone, &now).unwrap();
        assert_eq!(task_of_text(&unsaid, &zone).unwrap().kind, "");
        assert_eq!(TaskEdit { kind: "Not a kind!".into(), ..TaskEdit::default() }.tidy().kind, "");
        // A kind of yours is a concept too, read back as it was written.
        let errand = apply(&unsaid, &TaskEdit { kind: "rendez-vous".into(), ..TaskEdit::of(&task_of_text(&unsaid, &zone).unwrap()) }, &zone, &now).unwrap();
        assert!(errand.contains("task-type/rendez-vous"), "{errand}");
        assert_eq!(task_of_text(&errand, &zone).unwrap().kind, "rendez-vous");
        // Needs an office: a concept of its own, set and taken off without touching the kind.
        let task = task_of_text(&unsaid, &zone).unwrap();
        // Billed or not, said once, read back; unsaid, the line goes.
        let heavy = apply(&unsaid, &TaskEdit { energy: "heavy".into(), ..TaskEdit::of(&task) }, &zone, &now).unwrap();
        assert!(heavy.contains("X-SIOUL-ENERGY:HEAVY"), "{heavy}");
        assert_eq!(task_of_text(&heavy, &zone).unwrap().energy, "heavy");
        let usual = apply(&heavy, &TaskEdit { energy: String::new(), ..TaskEdit::of(&task_of_text(&heavy, &zone).unwrap()) }, &zone, &now).unwrap();
        assert!(!usual.contains("X-SIOUL-ENERGY"), "{usual}");
        let billed = apply(&unsaid, &TaskEdit { billable: Some(false), ..TaskEdit::of(&task) }, &zone, &now).unwrap();
        assert_eq!(task_of_text(&billed, &zone).unwrap().billable, Some(false));
        let again = apply(&billed, &TaskEdit { billable: None, ..TaskEdit::of(&task_of_text(&billed, &zone).unwrap()) }, &zone, &now).unwrap();
        assert!(!again.contains("X-SIOUL-BILLABLE"), "{again}");
        let office = apply(&unsaid, &TaskEdit { office_hours: true, kind: "call".into(), ..TaskEdit::of(&task) }, &zone, &now).unwrap();
        let read = task_of_text(&office, &zone).unwrap();
        assert!(read.office_hours && read.kind == "call");
        let open = apply(&office, &TaskEdit { office_hours: false, ..TaskEdit::of(&read) }, &zone, &now).unwrap();
        let read = task_of_text(&open, &zone).unwrap();
        assert!(!read.office_hours && read.kind == "call");
    }

    const LETTER: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Other//EN\r\nBEGIN:VTODO\r\nUID:letter-1\r\nDTSTAMP:20261001T080000Z\r\n\
        CREATED:20261001T080000Z\r\nSUMMARY:Send the letter\\, registered\r\nDESCRIPTION:With the form \r\n and the copy.\r\n\
        DTSTART;TZID=Europe/Paris:20261005T090000\r\nDUE;VALUE=DATE:20261030\r\nESTIMATED-DURATION:PT1H30M\r\nPRIORITY:2\r\n\
        CATEGORIES:admin,taxes\\, 2025\r\nCATEGORIES:you\r\nRELATED-TO:taxes-all\r\nRELATED-TO;RELTYPE=DEPENDS-ON:gather-papers\r\n\
        RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:wait-answer\r\nLINK;LINKREL=describedby;LABEL=\"Outbox: 5\";VALUE=URI:sioul:note/admin/letters.md\r\n\
        LINK;LINKREL=related;VALUE=UID:event-7\r\nCONTACT;ALTREP=\"sioul:contact/tax-office\":Service des impôts\\, centre\r\nREFID:taxes-2025\r\n\
        X-OTHER-APP:keep me\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Not the notes\r\nTRIGGER:-PT1H\r\nEND:VALARM\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";

    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    fn now() -> Zoned {
        "2026-10-03T10:00:00+02:00[Europe/Paris]".parse().unwrap()
    }

    #[test]
    fn reads_every_field() {
        let task = task_of_text(LETTER, &paris()).unwrap();
        assert_eq!((task.uid.as_str(), task.title.as_str(), task.notes.as_str()), ("letter-1", "Send the letter, registered", "With the form and the copy."));
        assert_eq!((task.start.as_str(), task.due.as_str(), task.estimate, task.priority), ("2026-10-05T09:00", "2026-10-30", 90, 2));
        assert_eq!(task.categories, vec!["admin", "taxes, 2025", "you"]);
        assert_eq!(task.parent(), Some("taxes-all"));
        assert_eq!(task.depends_on().map(|r| r.uid.as_str()).collect::<Vec<_>>(), vec!["gather-papers"]);
        assert_eq!(task.relations[2], Relation { kind: "FINISHTOSTART".into(), uid: "wait-answer".into(), gap: 14 * 1440 });
        assert_eq!(task.links[0], Link { uri: "sioul:note/admin/letters.md".into(), label: "Outbox: 5".into(), rel: "describedby".into() });
        assert_eq!(task.links[1].uri, "uid:event-7");
        assert_eq!(task.contacts, vec![ContactRef { name: "Service des impôts, centre".into(), uri: "sioul:contact/tax-office".into() }]);
        assert_eq!((task.cases.clone(), task.status), (vec!["taxes-2025".to_string()], Status::NeedsAction));
    }

    #[test]
    fn an_unchanged_form_writes_nothing() {
        let task = task_of_text(LETTER, &paris()).unwrap();
        assert_eq!(apply(LETTER, &TaskEdit::of(&task), &paris(), &now()).unwrap(), LETTER);
    }

    #[test]
    fn a_change_touches_only_its_lines() {
        let task = task_of_text(LETTER, &paris()).unwrap();
        let mut edit = TaskEdit::of(&task);
        edit.title = "Send the registered letter".into();
        edit.waits_for = vec!["photocopies".into()];
        let text = apply(LETTER, &edit, &paris(), &now()).unwrap();
        let (before, after) = (lines::unfold(LETTER), lines::unfold(&text));
        let gone: Vec<&String> = before.iter().filter(|l| !after.contains(l)).collect();
        let new: Vec<&String> = after.iter().filter(|l| !before.contains(l)).collect();
        assert_eq!(gone.iter().map(|l| lines::name(l)).collect::<Vec<_>>(), vec!["DTSTAMP", "SUMMARY", "RELATED-TO"], "{gone:?}");
        assert!(new.iter().any(|l| *l == "SUMMARY:Send the registered letter") && new.iter().any(|l| *l == "RELATED-TO;RELTYPE=DEPENDS-ON:photocopies"), "{new:?}");
        assert!(text.contains("RELATED-TO;RELTYPE=FINISHTOSTART;GAP=P14D:wait-answer") && text.contains("X-OTHER-APP:keep me") && text.contains("SEQUENCE:1"), "{text}");
        let again = task_of_text(&text, &paris()).unwrap();
        assert_eq!((again.notes.as_str(), again.estimate, again.title.as_str()), ("With the form and the copy.", 90, "Send the registered letter"));
    }

    #[test]
    fn done_and_open_again() {
        let done = set_status(LETTER, Status::Completed, &paris(), &now()).unwrap();
        assert!(done.contains("STATUS:COMPLETED") && done.contains("COMPLETED:20261003T080000Z") && done.contains("PERCENT-COMPLETE:100"), "{done}");
        let task = task_of_text(&done, &paris()).unwrap();
        assert_eq!((task.status, task.completed), (Status::Completed, Some(now().timestamp().as_second())));
        let open = set_status(&done, Status::NeedsAction, &paris(), &now()).unwrap();
        assert!(open.contains("STATUS:NEEDS-ACTION") && !open.contains("COMPLETED:") && !open.contains("PERCENT-COMPLETE"), "{open}");
    }

    #[test]
    fn a_repeating_task_comes_back() {
        let edit = TaskEdit { title: "Water the plants".into(), start: "2026-09-21".into(), due: "2026-09-22".into(), repeat: "weekly".into(), ..TaskEdit::default() };
        let text = new_task(&edit, "plants", &paris(), &now()).unwrap();
        let done = set_status(&text, Status::Completed, &paris(), &now()).unwrap();
        let task = task_of_text(&done, &paris()).unwrap();
        // The 22nd and 29th have passed on the 3rd of October: the 6th is next.
        assert_eq!((task.start.as_str(), task.due.as_str(), task.status), ("2026-10-05", "2026-10-06", Status::NeedsAction));
    }

    #[test]
    fn new_tasks_read_back_as_written() {
        let edit = TaskEdit {
            title: "Ask for the certificate".into(),
            notes: "Certificate of schooling; dates of the year.".into(),
            due: "2026-10-30T17:00".into(),
            estimate: 15,
            categories: vec!["admin".into(), "you".into()],
            parent: "school".into(),
            waits_for: vec!["form".into()],
            links: vec![Link { uri: "sioul:note/admin/letters.md".into(), label: "Outbox: 2".into(), rel: "describedby".into() }, Link { uri: "mid:abc@example.org".into(), label: String::new(), rel: "via".into() }],
            contacts: vec![ContactRef { name: "School, office".into(), uri: "sioul:contact/school".into() }],
            cases: vec!["school".into()],
            ..TaskEdit::default()
        };
        let text = new_task(&edit, "certificate", &paris(), &now()).unwrap();
        let written = lines::unfold(&text);
        assert!(written.iter().any(|l| l == "LINK;LINKREL=describedby;LABEL=\"Outbox: 2\";VALUE=URI:sioul:note/admin/letters.md"), "{text}");
        assert!(text.contains("DUE:20261030T160000Z") && text.contains("ESTIMATED-DURATION:PT15M"), "{text}");
        assert_eq!(TaskEdit::of(&task_of_text(&text, &paris()).unwrap()), edit);
    }

    #[test]
    fn the_first_estimate_is_kept() {
        // Made with an estimate: the first one is that.
        let made = new_task(&TaskEdit { title: "Fill the form".into(), estimate: 30, ..TaskEdit::default() }, "f", &paris(), &now()).unwrap();
        assert!(made.contains("X-SIOUL-ESTIMATE-FIRST:PT30M"), "{made}");
        let task = task_of_text(&made, &paris()).unwrap();
        assert_eq!((task.estimate, task.estimate_first), (30, Some(30)));
        // The estimate changed after an overrun: the first stays.
        let longer = apply(&made, &TaskEdit { estimate: 90, ..TaskEdit::of(&task) }, &paris(), &now()).unwrap();
        let task = task_of_text(&longer, &paris()).unwrap();
        assert_eq!((task.estimate, task.estimate_first), (90, Some(30)));
        assert_eq!(longer.matches("X-SIOUL-ESTIMATE-FIRST").count(), 1);
        // Taken away and given again: still the first.
        let none = apply(&longer, &TaskEdit { estimate: 0, ..TaskEdit::of(&task) }, &paris(), &now()).unwrap();
        let again = apply(&none, &TaskEdit { estimate: 45, ..TaskEdit::of(&task_of_text(&none, &paris()).unwrap()) }, &paris(), &now()).unwrap();
        assert_eq!(task_of_text(&again, &paris()).unwrap().estimate_first, Some(30));
        // Made without one, estimated later: that is the first.
        let bare = new_task(&TaskEdit { title: "Call".into(), ..TaskEdit::default() }, "c", &paris(), &now()).unwrap();
        assert!(!bare.contains("ESTIMATE-FIRST"));
        let later = apply(&bare, &TaskEdit { estimate: 15, ..TaskEdit::of(&task_of_text(&bare, &paris()).unwrap()) }, &paris(), &now()).unwrap();
        assert_eq!(task_of_text(&later, &paris()).unwrap().estimate_first, Some(15));
        // A task estimated before the first was kept: none, ever (its first guess is unknown).
        let old = LETTER;
        let task = task_of_text(old, &paris()).unwrap();
        assert_eq!((task.estimate, task.estimate_first), (90, None));
        let changed = apply(old, &TaskEdit { estimate: 120, ..TaskEdit::of(&task) }, &paris(), &now()).unwrap();
        assert_eq!(task_of_text(&changed, &paris()).unwrap().estimate_first, None);
    }

    #[test]
    fn heaviness_written_from_the_costs() {
        use crate::demands::Demands;
        let made = new_task(&TaskEdit { title: "Call the bank".into(), energy: "light".into(), demands: Demands { anxiety: Some(8), ..Demands::default() }, ..TaskEdit::default() }, "b", &paris(), &now()).unwrap();
        assert!(made.contains("X-SIOUL-ENERGY:HEAVY") && !made.contains("LIGHT"), "the costs say heavy, whatever the word: {made}");
        let task = task_of_text(&made, &paris()).unwrap();
        // Body and senses written and read back.
        let body = apply(&made, &TaskEdit { demands: Demands { anxiety: Some(2), body: Some(5), ..Demands::default() }, ..TaskEdit::of(&task) }, &paris(), &now()).unwrap();
        assert!(body.contains("X-SIOUL-COST:ANXIETY=2;BODY=5") && !body.contains("X-SIOUL-ENERGY"), "usual: no line: {body}");
        let task = task_of_text(&body, &paris()).unwrap();
        assert_eq!((task.demands.body, task.energy.as_str()), (Some(5), ""));
        // A file whose word disagrees with its costs (an older Sioul): set right at the next save.
        let stale = body.replace("X-SIOUL-COST:ANXIETY=2;BODY=5", "X-SIOUL-COST:ANXIETY=2;BODY=5\r\nX-SIOUL-ENERGY:HEAVY");
        let task = task_of_text(&stale, &paris()).unwrap();
        assert_eq!(task.energy, "heavy");
        let fixed = apply(&stale, &TaskEdit::of(&task), &paris(), &now()).unwrap();
        assert!(!fixed.contains("X-SIOUL-ENERGY"), "{fixed}");
        // No cost rated: the word chosen stands.
        let word = new_task(&TaskEdit { title: "Walk".into(), energy: "rest".into(), demands: Demands { gain: Some(8), ..Demands::default() }, ..TaskEdit::default() }, "w", &paris(), &now()).unwrap();
        assert!(word.contains("X-SIOUL-ENERGY:REST"));
    }

    #[test]
    fn felt_ratings_dated_and_kept_five() {
        use crate::demands::Demands;
        let day = |d: i8| Date::constant(2026, 10, d);
        let felt = Demands { anxiety: Some(3), body: Some(1), gain: Some(6), ..Demands::default() };
        let rated = set_felt(LETTER, &felt, day(6), &now()).unwrap();
        assert!(rated.contains("X-SIOUL-FELT-COST;X-SIOUL-ON=20261006:ANXIETY=3;BODY=1") && rated.contains("X-SIOUL-FELT-GAIN;X-SIOUL-ON=20261006:6"), "{rated}");
        assert!(rated.contains("X-OTHER-APP:keep me") && rated.contains("BEGIN:VALARM"));
        let task = task_of_text(&rated, &paris()).unwrap();
        assert_eq!(task.felt.len(), 1);
        assert_eq!((task.felt[0].on, task.felt[0].demands), (Some(day(6)), felt));
        assert_eq!(TaskEdit::of(&task).felt, felt);
        // The same day again: replaced, not added.
        let again = set_felt(&rated, &Demands { anxiety: Some(4), ..Demands::default() }, day(6), &now()).unwrap();
        let task = task_of_text(&again, &paris()).unwrap();
        assert_eq!((task.felt.len(), task.felt[0].demands.anxiety, task.felt[0].demands.gain), (1, Some(4), None));
        // The same rating again: the text as it was.
        assert_eq!(set_felt(&again, &Demands { anxiety: Some(4), ..Demands::default() }, day(6), &now()).unwrap(), again);
        // Seven days rated: the newest five kept, oldest first.
        let mut text = again;
        for d in 7..=12 {
            text = set_felt(&text, &Demands { cognitive: Some(d as u8 - 6), ..Demands::default() }, day(d), &now()).unwrap();
        }
        let task = task_of_text(&text, &paris()).unwrap();
        assert_eq!(task.felt.iter().map(|f| f.on.unwrap().day()).collect::<Vec<_>>(), vec![8, 9, 10, 11, 12]);
        // Nothing said: that day's rating taken away.
        let cleared = set_felt(&text, &Demands::default(), day(12), &now()).unwrap();
        assert_eq!(task_of_text(&cleared, &paris()).unwrap().felt.len(), 4);
        // The form never writes nor clears it.
        let task = task_of_text(&cleared, &paris()).unwrap();
        let form = apply(&cleared, &TaskEdit { felt: Demands::default(), title: "Send it".into(), ..TaskEdit::of(&task) }, &paris(), &now()).unwrap();
        assert_eq!(task_of_text(&form, &paris()).unwrap().felt.len(), 4);
        assert_eq!(apply(&cleared, &TaskEdit { felt: Demands::default(), ..TaskEdit::of(&task) }, &paris(), &now()).unwrap(), cleared, "felt alone changed in the form: nothing written");
    }

    #[test]
    fn durations() {
        assert_eq!((minutes_of("PT15M"), minutes_of("P1DT2H"), minutes_of("-P1W"), minutes_of("P"), minutes_of("15")), (Some(15), Some(1560), Some(-10080), None, None));
        assert_eq!((duration_of(15), duration_of(90), duration_of(20160), duration_of(0)), ("PT15M".to_string(), "PT1H30M".to_string(), "P14D".to_string(), "PT0M".to_string()));
        let line = add_lines(LETTER, &[relation_line("DEPENDS-ON", "gather-papers", 0)], &now()).unwrap();
        assert_eq!(line, LETTER, "already there: unchanged");
        let removed = remove_lines(LETTER, |l| lines::name(l) == "REFID", &now()).unwrap();
        assert!(!removed.contains("REFID") && removed.contains("X-OTHER-APP"));
    }

    #[test]
    fn files_from_elsewhere_never_crash_nor_inject() {
        // Durations and rules too long for any date: read as none, done without a crash.
        assert_eq!((minutes_of("P99999999999999999W"), minutes_of("PT9223372036854775807M1H")), (None, None));
        let odd = "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:odd\r\nDTSTART;VALUE=DATE:20261005\r\nDURATION:P9999999999D\r\n\
            RRULE:FREQ=YEARLY;INTERVAL=99999999999\r\nEND:VTODO\r\nEND:VCALENDAR\r\n";
        let task = task_of_text(odd, &paris()).unwrap();
        assert_eq!((task.start.as_str(), task.due.as_str(), task.repeat.as_str()), ("2026-10-05", "", "yearly"));
        assert!(set_status(odd, Status::Completed, &paris(), &now()).is_ok());
        // A rule that ended: done for good, not back.
        let ended = odd.replace("DURATION:P9999999999D\r\n", "").replace("INTERVAL=99999999999", "UNTIL=20261231T000000Z");
        let done = task_of_text(&set_status(&ended, Status::Completed, &paris(), &now()).unwrap(), &paris()).unwrap();
        assert_eq!(done.status, Status::Completed);
        // Line breaks in what is written as it is stay out of the file.
        let link = link_line(&Link { uri: "uid:a\r\nATTENDEE:mailto:x@example.org".into(), ..Link::default() });
        assert!(!link.contains(['\r', '\n']) && relation_line("DEPENDS-ON", "b\nX", 0) == "RELATED-TO;RELTYPE=DEPENDS-ON:bX", "{link}");
    }

    #[test]
    fn a_time_given_for_one_day() {
        let zone = paris();
        // A step with a date asked: the time it is given today goes in its own line, its dates as they were.
        let made = new_task(&TaskEdit { title: "Call the bank".into(), start: "2026-10-01".into(), due: "2026-10-09".into(), ..TaskEdit::default() }, "bank", &zone, &now()).unwrap();
        let task = task_of_text(&made, &zone).unwrap();
        let given = apply(&made, &TaskEdit { at: "2026-10-06T14:30".into(), ..TaskEdit::of(&task) }, &zone, &now()).unwrap();
        assert!(given.contains("X-SIOUL-AT:20261006T123000Z") && given.contains("DTSTART;VALUE=DATE:20261001") && given.contains("DUE;VALUE=DATE:20261009"), "{given}");
        let task = task_of_text(&given, &zone).unwrap();
        let today: Date = "2026-10-06".parse().unwrap();
        assert_eq!(task.at, "2026-10-06T14:30");
        assert_eq!(task.at_on(today, &zone), Some(1_791_289_800));
        // Another day it says nothing.
        assert_eq!(task.at_on(today.tomorrow().unwrap(), &zone), None);
        // Left to the plan: the line goes, nothing else changes.
        let placed = apply(&given, &TaskEdit { at: String::new(), ..TaskEdit::of(&task) }, &zone, &now()).unwrap();
        assert!(!placed.contains("X-SIOUL-AT") && placed.contains("DTSTART;VALUE=DATE:20261001"), "{placed}");
        // Changed another day for another reason: the time given that day goes; saved unchanged, nothing is written.
        let next_day: Zoned = "2026-10-07T09:00:00+02:00[Europe/Paris]".parse().unwrap();
        assert_eq!(apply(&given, &TaskEdit::of(&task), &zone, &next_day).unwrap(), given);
        let later = apply(&given, &TaskEdit { estimate: 20, ..TaskEdit::of(&task) }, &zone, &next_day).unwrap();
        assert!(!later.contains("X-SIOUL-AT") && later.contains("ESTIMATED-DURATION:PT20M"), "{later}");
        // A repeating task done: its next turn comes without the time given for this one.
        let weekly = apply(&given, &TaskEdit { repeat: "weekly".into(), ..TaskEdit::of(&task) }, &zone, &now()).unwrap();
        let next = set_status(&weekly, Status::Completed, &zone, &now()).unwrap();
        assert!(!next.contains("X-SIOUL-AT") && next.contains("STATUS:NEEDS-ACTION"), "{next}");
    }
}
