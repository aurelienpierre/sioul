// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The two pauses (docs/pauses.md): **Free time** (« Temps libre »), taken
//! for a good moment, and **Paused** (« En pause »), for a moment that
//! overwhelms.
//!
//! - Free time is leisure whatever the hour: as usual only your safe senders
//!   reach you (or no one but Always through: "Nothing at all"; Free time is
//!   a column of the matrix of what reaches you, `attention`), with doses,
//!   codes you asked for and your events' alarms; leisure is offered, never a
//!   list to finish. The working
//!   time it took moves the end of today's work later by as much, never past
//!   the wind-down less an hour, your latest end or the evening's time for
//!   you, with light steps only after the usual end; "Keep my usual end" moves
//!   nothing. Sleep comes first: it ends at the night's start.
//! - The pause holds everything Sioul shows, whatever the time: no
//!   notification but doses (unless set otherwise) and your alarms, the
//!   window covered with a few literal lines. It asks nothing, moves nothing
//!   into the evening and sends nothing. Coming back, the rest of today is
//!   lighter.
//!
//! Both are the person's act: Sioul never starts them, never counts them,
//! never infers anything from them (GP1, GP17, P26, P30). Their state is in
//! `quiet.toml` (`quiet::Overrides`), shared with the time part; their setup
//! in the configuration (`[free_time]`, `[pause]`), shared with the settings.

use crate::areas::Area;
use crate::i18n::Translator;
use crate::quiet::{Blocks, Overrides};
use crate::window::AdminWindow;
use jiff::Zoned;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Minutes kept free of work before the wind-down when the end of work moves
/// (GP10a: low-arousal time before sleep; one hour is a guess).
pub const WIND_DOWN_BUFFER: i64 = 60;
/// The latest end of work, unsaid: so many minutes after the usual end (GP10b, a guess).
pub const LATEST_AFTER: u32 = 120;
/// The breathing guide's pace, unsaid: breaths a minute (P16: about five to six to start).
pub const PACE: u32 = 6;

fn yes() -> bool {
    true
}

fn latest_after() -> u32 {
    LATEST_AFTER
}

fn pace() -> u32 {
    PACE
}

/// Free time as set up (`[free_time]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FreeTimeSettings {
    /// "Nothing at all": not even your safe senders (GP6); doses keep their own setting.
    #[serde(default)]
    pub nothing: bool,
    /// The end of work moves later by the working time Free time took (GP10);
    /// off, the usual end is kept, as "Keep my usual end" does (GP11).
    #[serde(default = "yes")]
    pub moves: bool,
    /// The latest end of work, in minutes after the usual end (GP10b).
    #[serde(default = "latest_after")]
    pub latest_after: u32,
    /// Movement and exercise among the offers; unsaid, on unless days are kept
    /// even (energy-limiting illness, GP8).
    #[serde(default)]
    pub movement: Option<bool>,
}

impl Default for FreeTimeSettings {
    fn default() -> FreeTimeSettings {
        FreeTimeSettings { nothing: false, moves: true, latest_after: LATEST_AFTER, movement: None }
    }
}

impl FreeTimeSettings {
    /// Movement and exercise among Free time's offers: as set, else on unless
    /// days are kept even, the setting for energy-limiting illness (GP8).
    pub fn movement(&self, planning: &crate::config::PlanningSettings) -> bool {
        self.movement.unwrap_or(!planning.even_days)
    }
}

/// The pause as set up on a calm day (`[pause]`, P1).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PauseSettings {
    /// Dose reminders still come during the pause (P7, docs/health.md). The
    /// matrix's cell now (`[attention] doses`, `attention::Attention::of`),
    /// read from here once to seed it while no `[attention]` is written.
    #[serde(default = "yes")]
    pub doses: bool,
    /// On the phone, starred contacts and repeat callers get through (P9).
    /// Always through's pause cells now (`attention`), read from here once to seed them.
    #[serde(default = "yes")]
    pub people: bool,
    /// What helps you, in your words, one a line; a line with a link or a
    /// file's path opens it (P2, P15).
    #[serde(default)]
    pub helps: Vec<String>,
    /// The breathing guide: off unless switched on (P16).
    #[serde(default)]
    pub breathing: bool,
    /// Its pace, breaths a minute.
    #[serde(default = "pace")]
    pub pace: u32,
    /// One line to come back to the present, in your words (P17).
    #[serde(default)]
    pub grounding: String,
    /// The rest of today, coming back: "lighter" (unsaid), "rest", "as-is" (P23).
    #[serde(default)]
    pub after: Option<String>,
    /// The country whose numbers show (an ISO code); unsaid, the country of
    /// phone numbers, else the system's (P13).
    #[serde(default)]
    pub country: Option<String>,
    /// GNOME: Sioul may switch GNOME's own Do Not Disturb, for both pauses (`dnd`).
    #[serde(default)]
    pub gnome: bool,
}

impl Default for PauseSettings {
    fn default() -> PauseSettings {
        PauseSettings { doses: true, people: true, helps: Vec::new(), breathing: false, pace: PACE, grounding: String::new(), after: None, country: None, gnome: false }
    }
}

impl PauseSettings {
    pub fn after(&self) -> After {
        After::parse(self.after.as_deref().unwrap_or(""))
    }

    /// Breaths a minute, within reason.
    pub fn pace(&self) -> u32 {
        self.pace.clamp(3, 10)
    }
}

/// The rest of today, coming back from a pause (P23).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    /// The bad-day level: the room and budgets of a hazy day (about 60 %, capacity criterion 36).
    Lighter,
    /// The work day closed, as "Done for today", without its sheet.
    Rest,
    /// As planned.
    AsIs,
}

impl After {
    pub fn parse(text: &str) -> After {
        match text.trim() {
            "rest" => After::Rest,
            "as-is" => After::AsIs,
            _ => After::Lighter,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            After::Lighter => "lighter",
            After::Rest => "rest",
            After::AsIs => "as-is",
        }
    }
}

// ---------------------------------------------------------------- the state

impl Overrides {
    /// Free time pressed and not ended since (whether it still holds is
    /// `quiet::mode`'s to say: the night's start ends it), from then.
    pub fn free_from(&self) -> Option<i64> {
        self.free_since.filter(|since| self.free_ended.is_none_or(|ended| ended < *since))
    }

    /// Paused: pressed and not ended since, from then.
    pub fn paused_from(&self) -> Option<i64> {
        self.paused_since.filter(|since| self.paused_ended.is_none_or(|ended| ended < *since))
    }

    /// Where the end of work moved to on `date` (Unix seconds), unless kept.
    pub fn moved_end(&self, date: Date) -> Option<i64> {
        self.extended_until.filter(|_| self.free_day == Some(date) && self.keep_end_on != Some(date))
    }

    /// Whether `date` is held lighter after a pause.
    pub fn lighter_on(&self, date: Date) -> bool {
        self.lighter.contains(&date)
    }

    /// Whether the Porch rests at `stamp`, after a pause (P24).
    pub fn porch_rests(&self, stamp: i64) -> bool {
        self.porch_rests_until.is_some_and(|until| stamp < until)
    }
}

/// When Free time pressed at `since` ends by itself: at the night's start
/// (sleep comes first), else at midnight; whichever comes first.
pub fn free_until(since: i64, blocks: &Blocks, zone: &TimeZone) -> i64 {
    let midnight = next_midnight(since, zone);
    let night = blocks.kept.iter().filter(|k| k.kind == "sleep" && k.start > since).map(|k| k.start).min();
    night.map_or(midnight, |night| night.min(midnight))
}

/// The midnight after `stamp`, on the clock of `zone`.
fn next_midnight(stamp: i64, zone: &TimeZone) -> i64 {
    jiff::Timestamp::from_second(stamp)
        .ok()
        .map(|t| t.to_zoned(zone.clone()).date())
        .and_then(|d| d.tomorrow().ok())
        .and_then(|d| d.to_zoned(zone.clone()).ok())
        .map_or(stamp + 86_400, |z| z.timestamp().as_second())
}

fn midnight_of(date: Date, zone: &TimeZone) -> i64 {
    date.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second())
}

/// "Nothing at all" now: this Free time's own choice, else the setting.
pub fn nothing_now(overrides: &Overrides, settings: &FreeTimeSettings) -> bool {
    overrides.free_nothing.unwrap_or(settings.nothing)
}

// ---------------------------------------------------------------- the end of work

/// What a day's evening holds, for the end of work to move (GP10).
pub struct Evening<'a> {
    /// The working hours: work's windows only.
    pub work: &'a [AdminWindow],
    /// Every hours, work's and admin's: the end of work moves around admin's.
    pub hours: &'a [AdminWindow],
    /// What holds time: events with their margins and pauses, meals, naps.
    pub held: &'a [(i64, i64)],
    /// Health's blocks: the night's start.
    pub blocks: &'a Blocks,
    /// The latest end of work, minutes after the usual end.
    pub latest_after: u32,
    /// The evening's slot of time for you, in minutes; 0 when slots are off.
    pub slot: u32,
}

/// What today holds, read once, for `Evening`: the working hours, every
/// hours, today's events (margins and pauses) with meals and naps, Health's
/// blocks, and the settings.
pub struct Day {
    pub work: Vec<AdminWindow>,
    pub hours: Vec<AdminWindow>,
    pub held: Vec<(i64, i64)>,
    pub blocks: Blocks,
    pub latest_after: u32,
    pub slot: u32,
}

impl Day {
    /// From the configuration, Health's blocks around now and the events of today (more is fine).
    pub fn of(config: &crate::config::Config, blocks: Blocks, events: &[crate::agenda::Occurrence]) -> Day {
        let mut held = crate::plan::event_spans(events, crate::plan::PAUSE);
        held.extend(blocks.kept.iter().filter(|k| k.kind != "sleep").map(|k| (k.start, k.end)));
        let slot = if config.planning.gain_slots() { crate::capacity::GAIN_SLOT_MINUTES } else { 0 };
        Day { work: config.working_hours(), hours: config.week_hours(), held, blocks, latest_after: config.free_time.latest_after, slot }
    }

    pub fn evening(&self) -> Evening<'_> {
        Evening { work: &self.work, hours: &self.hours, held: &self.held, blocks: &self.blocks, latest_after: self.latest_after, slot: self.slot }
    }
}

/// Minutes of `date`'s working time between `from` and `to` (Unix seconds):
/// the working hours' room as the plan counts it, less meals, naps and events.
pub fn worked_away(work: &[AdminWindow], held: &[(i64, i64)], date: Date, zone: &TimeZone, from: i64, to: i64) -> u32 {
    if to <= from {
        return 0;
    }
    let inside: Vec<(i64, i64, Area)> = crate::plan::stretches(work, date, zone, false)
        .into_iter()
        .filter_map(|(a, b, kinds)| {
            let (a, b) = (a.max(from), b.min(to));
            (b > a).then_some((a, b, kinds))
        })
        .collect();
    let seconds: i64 = crate::plan::less(&inside, held).iter().map(|(a, b, _)| b - a).sum();
    u32::try_from(seconds / 60).unwrap_or(0)
}

/// Where the end of work goes on `date` when `lost` minutes of working time
/// moved (GP10): from the usual end, over the evening's free gaps (events,
/// meals and admin hours go around it), never past the earliest of the
/// wind-down less an hour, the usual end and the latest end setting, the last
/// moment that still leaves the evening's slot for you its room before the
/// night, and midnight. (usual end, new end), Unix seconds; none without
/// working hours that day.
pub fn extend(evening: &Evening, date: Date, zone: &TimeZone, lost: u32) -> Option<(i64, i64)> {
    let (_, closing) = crate::window::hours_on(evening.work, date, zone)?;
    let usual = closing.timestamp().as_second();
    if lost == 0 {
        return Some((usual, usual));
    }
    let midnight = date.tomorrow().ok().map_or(usual + 86_400, |d| midnight_of(d, zone));
    let night = evening.blocks.kept.iter().filter(|k| k.kind == "sleep" && k.start >= usual).map(|k| k.start).min();
    let mut cap = (usual + i64::from(evening.latest_after) * 60).min(midnight);
    if let Some(night) = night {
        cap = cap.min(night - WIND_DOWN_BUFFER * 60);
    }
    // What the evening holds: events, meals, and admin's hours.
    let mut busy = evening.held.to_vec();
    busy.extend(crate::plan::stretches(evening.hours, date, zone, false).into_iter().filter(|s| !s.2.work).map(|s| (s.0, s.1)));
    // The evening's time for you survives (GP10c): a gap that holds it stays before the night.
    if evening.slot > 0
        && let Some(night) = night
    {
        let slot = i64::from(evening.slot) * 60;
        let latest = crate::plan::less(&[(usual, night, Area::ALL)], &busy).iter().filter(|g| g.1 - g.0 >= slot).map(|g| g.1 - slot).max();
        if let Some(latest) = latest {
            cap = cap.min(latest);
        }
    }
    if cap <= usual {
        return Some((usual, usual));
    }
    let (mut owed, mut end) = (i64::from(lost) * 60, usual);
    for (a, b, _) in crate::plan::less(&[(usual, cap, Area::ALL)], &busy) {
        let take = owed.min(b - a);
        end = a + take;
        owed -= take;
        if owed == 0 {
            break;
        }
    }
    Some((usual, end))
}

/// Free time starts at `now` (Unix seconds), "Nothing at all" as set (GP1).
pub fn start_free(overrides: &mut Overrides, now: i64) {
    overrides.free_since = Some(now);
    overrides.free_nothing = None;
}

/// Free time ends at `now`: the working time it took today is added to the
/// day's, and the end of work moves by all of it, unless kept (`moves` off,
/// or "Keep my usual end" today: GP10–GP11). The new end, when it moved.
pub fn end_free(overrides: &mut Overrides, evening: &Evening, now: &Zoned, moves: bool) -> Option<i64> {
    let stamp = now.timestamp().as_second();
    let since = overrides.free_from()?;
    overrides.free_ended = Some(stamp);
    let (today, zone) = (now.date(), now.time_zone());
    if overrides.free_day != Some(today) {
        overrides.free_day = Some(today);
        overrides.free_lost = None;
        overrides.extended_until = None;
    }
    let to = stamp.min(free_until(since, evening.blocks, zone));
    let lost = overrides.free_lost.unwrap_or(0) + worked_away(evening.work, evening.held, today, zone, since.max(midnight_of(today, zone)), to);
    overrides.free_lost = Some(lost);
    if !moves || overrides.keep_end_on == Some(today) {
        overrides.extended_until = None;
        return None;
    }
    let (usual, end) = extend(evening, today, zone, lost)?;
    overrides.extended_until = (end > usual).then_some(end);
    overrides.extended_until
}

/// "Keep my usual end" (GP11, GP19): today's end of work stays where it was,
/// no reason asked; what no longer fits goes to later days.
pub fn keep_usual_end(overrides: &mut Overrides, today: Date) {
    overrides.keep_end_on = Some(today);
    overrides.extended_until = None;
}

/// Today's end of work moved, for the plan: (usual end, moved end), while Free
/// time goes on as if it ended now ("work comes back when you do"), else as
/// it ended. None when nothing moved.
pub fn moved_today(overrides: &Overrides, evening: &Evening, now: &Zoned, moves: bool) -> Option<(i64, i64)> {
    let today = now.date();
    let end = if overrides.free_from().is_some() {
        let mut pending = overrides.clone();
        end_free(&mut pending, evening, now, moves)
    } else {
        overrides.moved_end(today)
    }?;
    let (_, closing) = crate::window::hours_on(evening.work, today, now.time_zone())?;
    let usual = closing.timestamp().as_second();
    (end > usual).then_some((usual, end))
}

// ---------------------------------------------------------------- the emergency pause

/// The pause starts at `now` (Unix seconds): it asks nothing (P6).
pub fn start_pause(overrides: &mut Overrides, now: i64) {
    overrides.paused_since = Some(now);
}

/// What coming back changes (P23–P24): the rest of today lighter (`lighter`),
/// or closed (`rest`: when work comes back), and the Porch resting until
/// `porch` (the next admin hours). Older lighter days are forgotten.
pub fn end_pause(overrides: &mut Overrides, now: &Zoned, after: After, rest: Option<i64>, porch: Option<i64>) {
    let stamp = now.timestamp().as_second();
    overrides.paused_ended = Some(stamp);
    let today = now.date();
    overrides.lighter.retain(|d| *d >= today);
    match after {
        After::Lighter if !overrides.lighter.contains(&today) => overrides.lighter.push(today),
        After::Rest => {
            if let Some(back) = rest {
                overrides.work_until = None;
                overrides.work_now = None;
                overrides.rest_until = Some(back);
                overrides.rest_from = Some(stamp);
            }
        }
        _ => {}
    }
    overrides.porch_rests_until = porch.filter(|p| *p > stamp);
}

/// "Lighten tomorrow", the one offer coming back (default: no, P23).
pub fn lighten(overrides: &mut Overrides, date: Date) {
    if !overrides.lighter.contains(&date) {
        overrides.lighter.push(date);
        overrides.lighter.sort();
    }
}

/// "Forget the last pause": its stamps go (P26); what it changed stays as chosen.
pub fn forget(overrides: &mut Overrides) {
    overrides.paused_since = None;
    overrides.paused_ended = None;
}

/// Today's share of its room and heavy steps (the plan's `today_percent`,
/// `heavy_today`): as the weather says, and no more than a hazy day's when
/// today is held lighter after a pause.
pub fn today_level(overrides: &Overrides, today: Date, weather: crate::today::Weather) -> (u32, u32) {
    let (room, heavy) = (weather.room(), weather.heavy());
    if overrides.lighter_on(today) {
        let haze = crate::today::Weather::Haze;
        (room.min(haze.room()), heavy.min(haze.heavy()))
    } else {
        (room, heavy)
    }
}

// ---------------------------------------------------------------- what is offered

/// Whether a task is movement or exercise, by its categories (`words.tasks.movement`,
/// the words in use: sport, walk, yoga…): left out of Free time's offers when
/// movement is off (GP8).
pub fn is_movement(task: &crate::tasks::Task) -> bool {
    crate::words::named_in(&task.categories, &crate::words::current().tasks.movement)
}

// ---------------------------------------------------------------- the screen

/// One line of what helps, as the screen shows it: the words, and what it opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Help {
    pub text: String,
    /// A link or a file to open; "" for none.
    pub open: String,
}

/// A line of the list read: the words, and a link or a file's path in it if
/// any ("The blue playlist https://…", "Photos ~/Pictures/Calm").
pub fn help(line: &str) -> Help {
    let is_link = |word: &str| {
        let scheme = word.split_once(':').filter(|(s, rest)| s.len() >= 3 && s.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)) && !rest.is_empty());
        word.contains("://") || word.starts_with('/') || word.starts_with("~/") || scheme.is_some()
    };
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.iter().position(|w| is_link(w)) {
        Some(at) => {
            let link = words[at];
            let open = if let Some(rest) = link.strip_prefix("~/") {
                format!("file://{}", crate::config::expand_home(&format!("~/{rest}")).display())
            } else if link.starts_with('/') {
                format!("file://{link}")
            } else {
                link.to_string()
            };
            let text: Vec<&str> = words.iter().enumerate().filter(|(i, _)| *i != at).map(|(_, w)| *w).collect();
            let text = text.join(" ").trim_end_matches([':', '-', '–', '—', ',']).trim().to_string();
            let text = if text.is_empty() { link.trim_end_matches('/').rsplit('/').next().unwrap_or(link).to_string() } else { text };
            Help { text, open }
        }
        None => Help { text: words.join(" "), open: String::new() },
    }
}

/// A number on the screen: what it is, the number, how to reach it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Number {
    /// "Emergency", "To talk to someone now, day and night".
    pub label: String,
    pub number: String,
    /// "tel:112", "sms:85258?body=SHOUT".
    pub url: String,
    /// Who it is for, how: "by text, for deaf and hard-of-hearing people"; "" for none.
    pub about: String,
}

/// The emergency numbers and crisis lines of every country Sioul knows
/// (`data/crisis-lines.toml`), each with its source and the day it was checked.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CrisisLines {
    pub country: Vec<Country>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Country {
    /// An ISO code ("FR"), or "EU" for the European Union as a whole.
    pub code: String,
    pub line: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Line {
    /// "emergency", "medical", "crisis", "text", "care"; "callback": the number
    /// emergency services call back from, never shown to call (`callback`).
    pub kind: String,
    pub number: String,
    /// The service's own name ("SAMU", "Samaritans"); "" for the emergency number.
    #[serde(default)]
    pub name: String,
    /// How: "call", "text", or both.
    pub how: Vec<String>,
    /// The word to text, when one starts the conversation ("SHOUT").
    #[serde(default)]
    pub sms: String,
    /// Who it is for, by language ("en", "fr"); none for everyone.
    #[serde(default)]
    pub about: BTreeMap<String, String>,
    /// Where it was checked, and when ("2026-10-06").
    pub source: String,
    pub checked: String,
}

/// The kinds of numbers, in the order the screen gives them; a callback number is not given.
pub const KINDS: [&str; 6] = ["emergency", "crisis", "medical", "text", "care", "callback"];

const CRISIS_LINES: &str = include_str!("../data/crisis-lines.toml");

impl CrisisLines {
    pub fn built_in() -> CrisisLines {
        toml::from_str(CRISIS_LINES).unwrap_or(CrisisLines { country: Vec::new() })
    }

    /// The country's numbers; Europe's 112 for a country not known here.
    pub fn of(&self, code: &str) -> Option<&Country> {
        let code = code.trim();
        self.country.iter().find(|c| c.code.eq_ignore_ascii_case(code)).or_else(|| self.country.iter().find(|c| c.code == "EU"))
    }
}

/// The number emergency services of the country `code` call back from, where
/// one is known (France: 0 800 112 112): a call from it may need to get
/// through do-not-disturb (P9). None elsewhere; never Europe's for another country.
pub fn callback(code: &str) -> Option<String> {
    let lines = CrisisLines::built_in();
    let country = lines.country.iter().find(|c| c.code.eq_ignore_ascii_case(code.trim()))?;
    country.line.iter().find(|l| l.kind == "callback").map(|l| l.number.clone())
}

/// The country whose numbers show: the pause's setting, else the country of
/// phone numbers (Contacts), else the system's (never guessed from Sioul's
/// language: an English speaker in France needs 3114 and 15). "" when none.
pub fn country(settings: &PauseSettings, contacts_region: Option<&str>) -> String {
    let chosen = |s: Option<&str>| s.map(str::trim).filter(|s| !s.is_empty()).map(str::to_ascii_uppercase);
    chosen(settings.country.as_deref())
        .or_else(|| chosen(contacts_region))
        .or_else(|| ["LC_ALL", "LC_TELEPHONE", "LANG"].iter().filter_map(|v| std::env::var(v).ok()).find_map(|v| crate::phones::region_of_locale(&v)).map(|r| r.code.to_ascii_uppercase()))
        .unwrap_or_default()
}

/// The numbers of `code` as the screen gives them: the first line (the
/// emergency number and the crisis line, one tap each), then the others,
/// unfolded on demand.
pub fn numbers(lines: &CrisisLines, code: &str, tr: &Translator) -> (Vec<Number>, Vec<Number>) {
    let Some(country) = lines.of(code) else { return (Vec::new(), Vec::new()) };
    let mut ordered: Vec<&Line> = country.line.iter().filter(|l| l.kind != "callback").collect();
    ordered.sort_by_key(|l| KINDS.iter().position(|k| *k == l.kind).unwrap_or(KINDS.len()));
    let shown = |line: &Line| {
        let text_only = !line.how.iter().any(|h| h == "call");
        let url = if text_only {
            let digits: String = line.number.chars().filter(char::is_ascii_digit).collect();
            if line.sms.is_empty() { format!("sms:{digits}") } else { format!("sms:{digits}?body={}", line.sms) }
        } else {
            format!("tel:{}", line.number.chars().filter(char::is_ascii_digit).collect::<String>())
        };
        let mut args = crate::i18n::args();
        args.set("word", line.sms.clone());
        let how = if line.how.len() > 1 {
            tr.text("pause-number-call-or-text", None)
        } else if text_only && !line.sms.is_empty() {
            tr.text("pause-number-text-word", Some(&args))
        } else if text_only && line.kind != "text" {
            // "In writing" says it already.
            tr.text("pause-number-by-text", None)
        } else {
            String::new()
        };
        let about = [line.name.clone(), line.about.get(tr.language()).cloned().unwrap_or_default(), how].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        Number { label: tr.text(&format!("pause-number-{}", line.kind), None), number: line.number.clone(), url, about }
    };
    let first: Vec<&Line> = ["emergency", "crisis"].iter().filter_map(|k| ordered.iter().copied().find(|l| l.kind == *k)).collect();
    let main = first.iter().map(|l| shown(l)).collect();
    let more = ordered.iter().filter(|l| !first.iter().any(|f| std::ptr::eq(*f, **l))).map(|l| shown(l)).collect();
    (main, more)
}

/// The pause's screen, as the window draws it (P11–P17): its words, your
/// list, the breathing guide and the grounding line as set, the numbers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Screen {
    /// Dose reminders still come during the pause: said, so that one is no surprise.
    pub doses: bool,
    pub helps: Vec<Help>,
    pub breathing: bool,
    /// Breaths a minute.
    pub pace: u32,
    pub grounding: String,
    pub numbers: Vec<Number>,
    pub more: Vec<Number>,
    /// The country shown, its name; "" for Europe's 112.
    pub country: String,
}

pub fn screen(settings: &PauseSettings, contacts_region: Option<&str>, tr: &Translator) -> Screen {
    let code = country(settings, contacts_region);
    let lines = CrisisLines::built_in();
    let (numbers, more) = numbers(&lines, &code, tr);
    let known = lines.country.iter().any(|c| c.code.eq_ignore_ascii_case(&code) && c.code != "EU");
    Screen {
        doses: settings.doses,
        helps: settings.helps.iter().filter(|l| !l.trim().is_empty()).map(|l| help(l)).collect(),
        breathing: settings.breathing,
        pace: settings.pace(),
        grounding: settings.grounding.trim().to_string(),
        numbers,
        more,
        country: if known { tr.text(&format!("country-{}", code.to_ascii_lowercase()), None) } else { String::new() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{TaskAreas, Time};
    use crate::needs::{Days, Needs};
    use crate::plan::{Settings, plan};
    use crate::attention::{Attention, Column, Kind, Level, Now, Person, Row};
    use crate::quiet::{Reason, mode};
    use crate::reach::Channel;
    use std::collections::BTreeSet;

    fn at(text: &str) -> Zoned {
        format!("{text}[Europe/Paris]").parse().unwrap()
    }

    fn stamp(text: &str) -> i64 {
        at(text).timestamp().as_second()
    }

    /// Monday to Friday, 9:00 to 17:00.
    fn week() -> Vec<AdminWindow> {
        ["monday", "tuesday", "wednesday", "thursday", "friday"].iter().map(|d| AdminWindow { day: d.to_string(), start: "09:00".into(), end: Some("17:00".into()), minutes: 0, kind: None }).collect()
    }

    /// Health as first set: breakfast 07:50–08:20, lunch 12:10–13:00, a nap
    /// 14:00–14:35, dinner 19:00–20:00, the night from 22:00 (bed 23:00) to 07:00.
    fn health(now: &Zoned) -> Blocks {
        Blocks::of(&Needs { meals_on: true, naps_on: true, sleep_on: true, ..Needs::default() }, &Days::default(), &[], now)
    }

    fn evening<'a>(work: &'a [AdminWindow], held: &'a [(i64, i64)], blocks: &'a Blocks) -> Evening<'a> {
        Evening { work, hours: work, held, blocks, latest_after: LATEST_AFTER, slot: 30 }
    }

    fn kept(blocks: &Blocks) -> Vec<(i64, i64)> {
        blocks.kept.iter().filter(|k| k.kind != "sleep").map(|k| (k.start, k.end)).collect()
    }

    fn hm(seconds: i64) -> String {
        jiff::Timestamp::from_second(seconds).unwrap().to_zoned(TimeZone::get("Europe/Paris").unwrap()).strftime("%H:%M").to_string()
    }

    #[test]
    fn the_pause_comes_first_and_doses_and_alarms_still_come() {
        // Friday 2 October 2026.
        let paused = Overrides { paused_since: Some(stamp("2026-10-02T10:00")), ..Overrides::default() };
        for time in ["10:30", "12:30", "14:10", "20:30", "23:30"] {
            let now = at(&format!("2026-10-02T{time}"));
            let m = mode(&week(), &[], &paused, &health(&now), &now);
            assert_eq!((m.reason.clone(), m.time, m.quiet), (Reason::Paused, Time::Sleep, true), "{time}: above work, meals, naps, evenings and the night");
            // As usual (`attention`): doses come unless the setup said otherwise, an
            // event's own alarm (an alarm you set), a code you just asked for (Q6); nothing else.
            let (usual, n) = (Attention::usual(), Now::of(&m));
            let comes = |matrix: &Attention, kind: Kind| matrix.level(Row::Own(kind), &n) == Level::Now;
            let mut held = Attention::usual();
            held.set(Row::Own(Kind::Doses), Column::Pause, Level::Later).unwrap();
            assert!(comes(&usual, Kind::Doses) && !comes(&held, Kind::Doses));
            assert!(comes(&usual, Kind::Alarms) && comes(&usual, Kind::Codes));
            assert!(!comes(&usual, Kind::Move) && !comes(&usual, Kind::Needs));
        }
        // Above "Done for today", "Work now" and Free time too.
        let now = at("2026-10-02T11:00");
        for others in [
            Overrides { rest_until: Some(stamp("2026-10-05T09:00")), ..paused.clone() },
            Overrides { work_now: Some(stamp("2026-10-02T17:00")), ..paused.clone() },
            Overrides { free_since: Some(stamp("2026-10-02T10:30")), ..paused.clone() },
        ] {
            assert_eq!(mode(&week(), &[], &others, &health(&now), &now).reason, Reason::Paused);
        }
        // Ended: the hours again; pressed again later: paused again.
        let ended = Overrides { paused_ended: Some(stamp("2026-10-02T10:45")), ..paused.clone() };
        assert_eq!(mode(&week(), &[], &ended, &health(&now), &now).reason, Reason::Working);
        let again = Overrides { paused_since: Some(stamp("2026-10-02T10:50")), ..ended };
        assert_eq!(mode(&week(), &[], &again, &health(&now), &now).reason, Reason::Paused);
    }

    #[test]
    fn free_time_is_leisure_with_the_safe_list_only() {
        let now = at("2026-10-02T15:00");
        let free = Overrides { free_since: Some(stamp("2026-10-02T14:40")), ..Overrides::default() };
        let m = mode(&week(), &[], &free, &health(&now), &now);
        assert_eq!((m.reason.clone(), m.time, m.quiet), (Reason::FreeTime, Time::Leisure, true), "leisure in working hours");
        assert_eq!(m.until.as_ref().map(|z| z.strftime("%H:%M").to_string()), Some("22:00".into()), "until the night's start");
        // As usual (`attention`): doses, codes, and events' alarms when their event falls in it; nothing else.
        let (usual, n) = (Attention::usual(), Now::of(&m));
        let level = |kind: Kind| usual.level(Row::Own(kind), &n);
        assert!(level(Kind::Doses) == Level::Now && level(Kind::Codes) == Level::Now && level(Kind::Alarms) == Level::Event);
        assert!(level(Kind::Move) != Level::Now);
        // As usual only the safe list: their calls and messages at once, their mail shown, not told; the others wait.
        assert_eq!((usual.person(Channel::Calls, Person::Safe, false, &n), usual.person(Channel::Mail, Person::Safe, false, &n)), (Level::Now, Level::Quiet));
        for person in [Person::Neutral, Person::Restricted, Person::Stranger] {
            assert_eq!(usual.person(Channel::Mail, person, false, &n), Level::Later, "{person:?}");
        }
        // Free time is a column now (Q15): a neutral row may come in it.
        let mut cells = Attention::usual();
        cells.set(Row::People(Channel::Mail, Person::Neutral), Column::Free, Level::Now).unwrap();
        assert_eq!(cells.person(Channel::Mail, Person::Neutral, false, &n), Level::Now);
        // "Nothing at all": the safe wait too, Always through still comes.
        let nothing = Now { nothing: true, ..n.clone() };
        assert_eq!((usual.person(Channel::Calls, Person::Safe, false, &nothing), usual.person(Channel::Calls, Person::Safe, true, &nothing)), (Level::Later, Level::Now));
        assert!(nothing_now(&Overrides { free_nothing: Some(true), ..free.clone() }, &FreeTimeSettings::default()));
        assert!(nothing_now(&free, &FreeTimeSettings { nothing: true, ..FreeTimeSettings::default() }));
        // Sleep still comes first: a nap, then the night, which ends it.
        let nap = at("2026-10-02T14:10");
        let early = Overrides { free_since: Some(stamp("2026-10-02T13:30")), ..Overrides::default() };
        assert_eq!(mode(&week(), &[], &early, &health(&nap), &nap).reason, Reason::Nap);
        let late = at("2026-10-02T21:30");
        assert_eq!(mode(&week(), &[], &free, &health(&late), &late).reason, Reason::FreeTime);
        let night = at("2026-10-02T22:30");
        assert_eq!(mode(&week(), &[], &free, &health(&night), &night).reason, Reason::WindingDown);
        let morning = at("2026-10-03T10:00");
        assert_eq!(mode(&week(), &[], &free, &health(&morning), &morning).reason, Reason::DayOff, "over with the night");
        // Without a night, at midnight.
        let no_night = Blocks::default();
        assert_eq!(free_until(stamp("2026-10-02T14:40"), &no_night, now.time_zone()), stamp("2026-10-03T00:00"));
        // Leisure tasks only, and the movement ones left out of the offers when movement is off.
        let walk = crate::tasks::Task { categories: vec!["Marche".into()], ..crate::tasks::Task::default() };
        assert!(is_movement(&walk) && !is_movement(&crate::tasks::Task { categories: vec!["joy".into()], ..crate::tasks::Task::default() }));
        let planning = crate::config::PlanningSettings { even_days: true, ..Default::default() };
        assert!(!FreeTimeSettings::default().movement(&planning), "energy-limiting illness: no exercise offered");
        assert!(FreeTimeSettings::default().movement(&crate::config::PlanningSettings::default()));
        assert!(FreeTimeSettings { movement: Some(true), ..FreeTimeSettings::default() }.movement(&planning), "unless asked");
    }

    #[test]
    fn the_end_of_work_moves_by_the_working_time_taken_within_its_caps() {
        let now = at("2026-10-02T16:00");
        let blocks = health(&now);
        let held = kept(&blocks);
        let work = week();
        let ev = evening(&work, &held, &blocks);
        let zone = now.time_zone();
        let date = now.date();
        // 14:00–16:00: the nap (14:00–14:35) is not working time: 85 minutes.
        assert_eq!(worked_away(&work, &held, date, zone, stamp("2026-10-02T14:00"), stamp("2026-10-02T16:00")), 85);
        assert_eq!(worked_away(&work, &held, date, zone, stamp("2026-10-02T17:30"), stamp("2026-10-02T18:30")), 0, "after work: nothing moves");
        // An hour: until 18:00.
        assert_eq!(extend(&ev, date, zone, 60).map(|(u, e)| (hm(u), hm(e))), Some(("17:00".into(), "18:00".into())));
        // Two and a half hours: dinner (19:00–20:00) goes around; the usual end + 2 h caps it at 19:00.
        assert_eq!(extend(&ev, date, zone, 150).map(|(_, e)| hm(e)), Some("19:00".into()));
        // A later latest end: past dinner, then the wind-down less an hour (21:00) caps it.
        let late = Evening { latest_after: 300, ..evening(&work, &held, &blocks) };
        assert_eq!(extend(&late, date, zone, 150).map(|(_, e)| hm(e)), Some("20:30".into()));
        assert_eq!(extend(&late, date, zone, 400).map(|(_, e)| hm(e)), Some("21:00".into()), "never past the wind-down less an hour");
        // An evening full of events: the slot for you keeps its half hour before the night.
        let mut busy = held.clone();
        busy.push((stamp("2026-10-02T18:00"), stamp("2026-10-02T19:00")));
        busy.push((stamp("2026-10-02T20:00"), stamp("2026-10-02T21:40")));
        let full = Evening { latest_after: 300, held: &busy, ..evening(&work, &held, &blocks) };
        assert_eq!(extend(&full, date, zone, 400).map(|(_, e)| hm(e)), Some("17:30".into()), "half of the gap at 17:00 kept for you: 21:40–22:00 is too short");
        // No working hours that day: nothing to move.
        let saturday = at("2026-10-03T11:00");
        assert_eq!(extend(&ev, saturday.date(), zone, 60), None);
        // Midnight caps it without a night.
        let none = Blocks::default();
        let no_night = Evening { latest_after: 600, blocks: &none, ..evening(&work, &held, &blocks) };
        assert_eq!(extend(&no_night, date, zone, 600).map(|(_, e)| hm(e)), Some("00:00".into()));
    }

    #[test]
    fn free_time_ending_moves_the_end_and_keep_my_usual_end_moves_nothing() {
        let blocks = health(&at("2026-10-02T12:00"));
        let held = kept(&blocks);
        let work = week();
        let ev = evening(&work, &held, &blocks);
        let mut o = Overrides::default();
        start_free(&mut o, stamp("2026-10-02T14:00"));
        // Coming back at 15:00: 25 minutes of work (the nap aside); the end moves to 17:25.
        let back = at("2026-10-02T15:00");
        assert_eq!(end_free(&mut o, &ev, &back, true).map(hm), Some("17:25".into()));
        assert!(o.free_from().is_none());
        assert_eq!(o.free_lost, Some(25));
        // Work until then, Sioul ends it: leisure after.
        let at_17 = at("2026-10-02T17:10");
        let m = mode(&work, &[], &o, &health(&at_17), &at_17);
        assert_eq!((m.reason.clone(), m.time, m.quiet), (Reason::Extended, Time::Work, false));
        assert_eq!(m.until.map(|z| z.strftime("%H:%M").to_string()), Some("17:25".into()));
        let after = at("2026-10-02T17:30");
        assert_eq!(mode(&work, &[], &o, &health(&after), &after).reason, Reason::Evening);
        // A second free time the same day adds up: 15:30–16:30, an hour more.
        start_free(&mut o, stamp("2026-10-02T15:30"));
        assert_eq!(end_free(&mut o, &ev, &at("2026-10-02T16:30"), true).map(hm), Some("18:25".into()));
        // "Keep my usual end": nothing moves, no reason asked; a later free time moves nothing that day.
        keep_usual_end(&mut o, back.date());
        assert_eq!(o.moved_end(back.date()), None);
        let at_17 = at("2026-10-02T17:10");
        assert_eq!(mode(&work, &[], &o, &health(&at_17), &at_17).reason, Reason::Evening);
        start_free(&mut o, stamp("2026-10-02T16:35"));
        assert_eq!(end_free(&mut o, &ev, &at("2026-10-02T16:50"), true), None);
        // The setting "the end does not move": the same.
        let mut kept_end = Overrides::default();
        start_free(&mut kept_end, stamp("2026-10-05T10:00"));
        assert_eq!(end_free(&mut kept_end, &ev, &at("2026-10-05T11:00"), false), None);
        // "Done for today" ends the moved day.
        let mut done = Overrides::default();
        start_free(&mut done, stamp("2026-10-05T10:00"));
        end_free(&mut done, &ev, &at("2026-10-05T11:00"), true);
        done.rest_until = Some(stamp("2026-10-06T09:00"));
        let monday = at("2026-10-05T17:10");
        assert_eq!(mode(&work, &[], &done, &health(&monday), &monday).reason, Reason::DoneForTheDay);
        // While it goes on, the plan sees the end as if work came back now.
        let mut going = Overrides::default();
        start_free(&mut going, stamp("2026-10-05T15:00"));
        let pending = moved_today(&going, &ev, &at("2026-10-05T16:00"), true).map(|(u, e)| (hm(u), hm(e)));
        assert_eq!(pending, Some(("17:00".into(), "18:00".into())));
        assert!(going.free_from().is_some(), "only seen, not ended");
        // The next day starts as usual.
        let tuesday = at("2026-10-06T17:10");
        assert_eq!(mode(&work, &[], &o, &health(&tuesday), &tuesday).reason, Reason::Evening);
    }

    /// One task with `minutes`, light or not.
    fn task(uid: &str, minutes: u32, light: bool) -> crate::tasks::Task {
        crate::tasks::Task { uid: uid.into(), title: uid.into(), estimate: minutes, energy: if light { "light".into() } else { String::new() }, ..crate::tasks::Task::default() }
    }

    #[test]
    fn the_extension_adds_hours_for_light_steps_and_what_does_not_fit_moves_on() {
        // Monday 5 October at 16:00, back from Free time; the end moved to 18:00.
        let now = at("2026-10-05T16:00");
        let windows = week();
        let usual = Settings::of_hours(&windows, TaskAreas::usual()).with_events(&now, &[]);
        let moved = Settings::of_hours(&windows, TaskAreas::usual()).with_extension(Some((stamp("2026-10-05T17:00"), stamp("2026-10-05T18:00")))).with_events(&now, &[]);
        assert_eq!(usual.room_on(now.date()).total() + 60, moved.room_on(now.date()).total(), "an hour more today");
        assert_eq!(moved.light_only, 60);
        let tasks = vec![task("hard", 90, false), task("easy", 45, true)];
        let (none, set) = (BTreeMap::new(), BTreeSet::new());
        let made = plan(&tasks, now.date(), &moved, &none, &set);
        // The light step fits today; the usual one takes the usual hour left, its rest goes on to Tuesday, said nowhere as a count.
        assert_eq!(made.items["easy"].start, Some(now.date()));
        assert_eq!(made.items["hard"].finish, Some(now.date().tomorrow().unwrap()));
        let today_minutes: u32 = made.days.get(&now.date()).map(|d| d.iter().filter(|(uid, _)| uid == "hard").map(|(_, m)| *m).sum()).unwrap_or(0);
        assert!((1..=60).contains(&today_minutes), "a step that is not light never takes the extension: {today_minutes}");
        // Budgets do not grow with it: what a day holds is the same either way.
        assert_eq!(moved.capacity.day_limit(now.date(), 1.0), usual.capacity.day_limit(now.date(), 1.0));
        // The day laid out: the light step may go after 17:00, the other never.
        let day = crate::dayview::day(&now, &[], &made, &tasks, &moved);
        let five = stamp("2026-10-05T17:00");
        assert!(day.blocks.iter().filter(|b| b.kind == "task" && b.key == "hard").all(|b| b.end <= five), "{:?}", day.blocks);
        assert!(day.blocks.iter().any(|b| b.kind == "task" && b.key == "easy" && b.start >= five), "the light step after the usual end: {:?}", day.blocks);
        assert!(day.hours.last().is_some_and(|h| h.end == stamp("2026-10-05T18:00")), "today's hours run to 18:00");
    }

    #[test]
    fn the_pause_moves_work_to_later_days_never_into_the_evening() {
        // Paused from 14:00 to 16:30 on Monday: no extension, the rest of the day lighter.
        let mut o = Overrides::default();
        start_pause(&mut o, stamp("2026-10-05T14:00"));
        let back = at("2026-10-05T16:30");
        end_pause(&mut o, &back, After::Lighter, None, None);
        assert!(o.paused_from().is_none());
        assert_eq!(o.moved_end(back.date()), None);
        let evening = at("2026-10-05T17:30");
        assert_eq!(mode(&week(), &[], &o, &health(&evening), &evening).reason, Reason::Evening, "the day is not extended");
        // The paused hours' work flows to later days: today keeps only what is left, lighter.
        let windows = week();
        let mut settings = Settings::of_hours(&windows, TaskAreas::usual()).with_events(&back, &[]);
        (settings.today_percent, settings.heavy_today) = today_level(&o, back.date(), crate::today::Weather::Clear);
        assert_eq!((settings.today_percent, settings.heavy_today), (60, 1), "the bad-day level");
        let tasks = vec![task("report", 120, false)];
        let made = plan(&tasks, back.date(), &settings, &BTreeMap::new(), &BTreeSet::new());
        let today: u32 = made.days.get(&back.date()).map(|d| d.iter().map(|(_, m)| *m).sum()).unwrap_or(0);
        assert!(today <= 30 * 60 / 100 + 5, "a lighter half hour at most: {today}");
        assert!(made.items["report"].finish > Some(back.date()));
        // "Lighten tomorrow", the one offer; old lighter days are forgotten.
        lighten(&mut o, back.date().tomorrow().unwrap());
        assert_eq!(today_level(&o, back.date().tomorrow().unwrap(), crate::today::Weather::Clear), (60, 1));
        assert_eq!(today_level(&o, back.date().tomorrow().unwrap(), crate::today::Weather::Fog), (30, 0), "never heavier than the weather said");
        let mut later = o.clone();
        start_pause(&mut later, stamp("2026-10-08T10:00"));
        end_pause(&mut later, &at("2026-10-08T10:20"), After::AsIs, None, None);
        assert!(later.lighter.is_empty(), "{:?}", later.lighter);
        // "Rest": the day closed, as "Done for today"; the Porch resting until the next admin hours.
        let mut rest = Overrides::default();
        start_pause(&mut rest, stamp("2026-10-05T10:00"));
        end_pause(&mut rest, &at("2026-10-05T10:30"), After::Rest, Some(stamp("2026-10-06T09:00")), Some(stamp("2026-10-05T14:00")));
        let noon = at("2026-10-05T12:00");
        assert_eq!(mode(&week(), &[], &rest, &health(&noon), &noon).reason, Reason::DoneForTheDay);
        assert!(rest.porch_rests(stamp("2026-10-05T13:00")) && !rest.porch_rests(stamp("2026-10-05T14:00")));
        // Forgotten: the stamps go, nothing else.
        forget(&mut rest);
        assert_eq!((rest.paused_since, rest.paused_ended), (None, None));
    }

    #[test]
    fn the_pauses_travel_between_devices() {
        // As the sharing carries quiet.toml, key by key: a pause pressed on one
        // device reads paused on the other; ended there, ended here.
        let mut here = Overrides::default();
        start_pause(&mut here, 1_759_390_000);
        start_free(&mut here, 1_759_380_000);
        here.free_ended = Some(1_759_385_000);
        here.free_day = Some("2026-10-02".parse().unwrap());
        here.free_lost = Some(25);
        here.extended_until = Some(1_759_420_000);
        here.lighter = vec!["2026-10-02".parse().unwrap()];
        here.porch_rests_until = Some(1_759_430_000);
        let text = toml::to_string(&here).unwrap();
        let there: Overrides = toml::from_str(&text).unwrap();
        assert_eq!(there, here, "{text}");
        assert!(there.paused_from().is_some() && there.free_from().is_none());
        // One key per field: the other device's "Come back" is one more key, later than the press.
        let mut merged: toml::Table = toml::from_str(&text).unwrap();
        merged.insert("paused_ended".into(), toml::Value::Integer(1_759_391_000));
        let ended: Overrides = toml::from_str(&toml::to_string(&merged).unwrap()).unwrap();
        assert!(ended.paused_from().is_none());
        // An older Sioul's file, without any of it, reads as no pause.
        let old: Overrides = toml::from_str("work_until = 5\n").unwrap();
        assert!(old.paused_from().is_none() && old.free_from().is_none() && old.lighter.is_empty());
        // Nothing of a pause is written when there was none.
        assert_eq!(toml::to_string(&Overrides::default()).unwrap().trim(), "");
    }

    #[test]
    fn crisis_lines_are_well_formed() {
        let lines = CrisisLines::built_in();
        assert!(!lines.country.is_empty(), "data/crisis-lines.toml reads");
        let mut codes = BTreeSet::new();
        let today = jiff::Zoned::now().date();
        for country in &lines.country {
            assert!(codes.insert(country.code.clone()), "{} twice", country.code);
            assert!(country.code == "EU" || (country.code.len() == 2 && country.code.chars().all(|c| c.is_ascii_uppercase())), "{}", country.code);
            assert!(country.line.iter().any(|l| l.kind == "emergency"), "{}: an emergency number", country.code);
            for line in &country.line {
                assert!(KINDS.contains(&line.kind.as_str()), "{}: {}", country.code, line.kind);
                assert!(!line.number.is_empty() && line.number.chars().all(|c| c.is_ascii_digit() || c == ' '), "{}: {}", country.code, line.number);
                assert!(!line.how.is_empty() && line.how.iter().all(|h| h == "call" || h == "text"), "{}: {:?}", line.number, line.how);
                assert!(line.source.starts_with("https://"), "{}: {}", line.number, line.source);
                let checked: Date = line.checked.parse().unwrap_or_else(|_| panic!("{}: {}", line.number, line.checked));
                assert!(checked <= today && checked >= Date::constant(2026, 1, 1), "{}: {}", line.number, line.checked);
                assert!(line.about.keys().all(|l| l == "en" || l == "fr"), "{}", line.number);
            }
        }
        let has = |code: &str, number: &str| lines.of(code).is_some_and(|c| c.code == code && c.line.iter().any(|l| l.number == number));
        for (code, number) in [("FR", "3114"), ("FR", "15"), ("FR", "112"), ("FR", "114"), ("EU", "112"), ("GB", "116 123"), ("GB", "999"), ("US", "988"), ("US", "911"), ("CA", "988"), ("CA", "911")] {
            assert!(has(code, number), "{code} {number}");
        }
        // A country not known here: Europe's 112.
        assert_eq!(lines.of("ZZ").map(|c| c.code.as_str()), Some("EU"));
        // The screen's first line: the emergency number and the crisis line, one tap each.
        let tr = Translator::new("en");
        let (first, more) = numbers(&lines, "FR", &tr);
        assert_eq!(first.iter().map(|n| (n.number.as_str(), n.url.as_str())).collect::<Vec<_>>(), [("112", "tel:112"), ("3114", "tel:3114")]);
        assert!(more.iter().any(|n| n.number == "15") && more.iter().any(|n| n.number == "114" && n.url == "sms:114"));
        assert!(first.iter().chain(&more).all(|n| n.number != "0 800 112 112"), "the callback number is never given to call");
        assert_eq!(callback("fr").as_deref(), Some("0 800 112 112"));
        assert_eq!(callback("GB"), None);
        let (_, uk) = numbers(&lines, "GB", &tr);
        assert!(uk.iter().any(|n| n.url == "sms:85258?body=SHOUT"), "{uk:?}");
        assert!(first.iter().chain(&more).all(|n| !n.label.starts_with("pause-")), "{first:?} {more:?}");
        // The country: the setting, else the phone numbers' country.
        assert_eq!(country(&PauseSettings { country: Some("gb".into()), ..PauseSettings::default() }, Some("FR")), "GB");
        assert_eq!(country(&PauseSettings::default(), Some("ca")), "CA");
    }

    #[test]
    fn what_helps_opens_what_you_chose() {
        assert_eq!(help("Headphones on"), Help { text: "Headphones on".into(), open: String::new() });
        assert_eq!(help("The blue playlist: https://music.example/blue"), Help { text: "The blue playlist".into(), open: "https://music.example/blue".into() });
        assert_eq!(help("/home/me/Pictures/Calm").text, "Calm");
        assert_eq!(help("Photos /home/me/Pictures/Calm").open, "file:///home/me/Pictures/Calm");
        assert_eq!(help("Notes: nothing else").open, "", "a colon is not a link");
        let s = screen(&PauseSettings { helps: vec!["Tea".into(), "  ".into()], country: Some("FR".into()), ..PauseSettings::default() }, None, &Translator::new("fr"));
        assert_eq!((s.helps.len(), s.breathing, s.pace, s.country.as_str()), (1, false, PACE, "France"), "the breathing guide off unless switched on");
    }
}
