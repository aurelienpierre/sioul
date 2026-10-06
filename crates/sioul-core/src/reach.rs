// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Who may reach you, on which channel, and when (docs/porch.md, "Who may
//! reach you, and when").
//!
//! Five states, one vocabulary everywhere: a **stranger** (in none of your
//! address books and on no list), the **blocked** (never, on any channel),
//! and for the people in your address books three levels: **safe**,
//! **neutral** (anyone in your address books, unless you chose otherwise) and
//! **restricted**. Who someone is, the lists and the address books say
//! (`porch::Senders`): their own entry first, then their card's, its
//! categories, a domain or a number's prefix.
//!
//! When each may reach you is a matrix per channel (mail; calls; messages
//! from other apps): rows the states, with hidden numbers for calls; columns
//! the five times of `areas::Time` and the pause. The blocked have no row:
//! never. Free time narrows every matrix to the safe (`Moment::free`).
//!
//! The decision reads no file: a phone writes it ahead for its call screening
//! (`Clock::frames`), which answers within milliseconds.

use crate::areas::{Time, Week};
use crate::config::{Config, ReachSettings, RowSettings, TimeOff};
use crate::needs::{Days, Kept, Needs};
use crate::porch::Standing;
use crate::quiet::{Blocks, Mode, Overrides};
use crate::window::AdminWindow;
use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};

/// How far ahead `next_allowed` looks, and a clock's events are read, in days.
pub const HORIZON_DAYS: i64 = 8;

/// Who someone is to you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Who {
    /// In none of your address books and on no list.
    Stranger,
    /// Never, on any channel: set aside for good.
    Blocked,
    /// Family, friends, chosen colleagues: by default, at any time.
    Safe,
    /// Anyone in your address books, unless you chose otherwise.
    Neutral,
    /// Those you hear from only at chosen times.
    Restricted,
}

impl Who {
    /// The five, in the order they are shown: the three levels, strangers, the blocked.
    pub const ALL: [Who; 5] = [Who::Safe, Who::Neutral, Who::Restricted, Who::Stranger, Who::Blocked];

    pub fn id(self) -> &'static str {
        match self {
            Who::Stranger => "stranger",
            Who::Blocked => "blocked",
            Who::Safe => "safe",
            Who::Neutral => "neutral",
            Who::Restricted => "restricted",
        }
    }

    pub fn read(text: &str) -> Option<Who> {
        Who::ALL.into_iter().find(|w| w.id() == text.trim().to_ascii_lowercase())
    }

    /// Its row in a matrix; the blocked have none: never.
    pub fn row(self) -> Option<Row> {
        match self {
            Who::Safe => Some(Row::Safe),
            Who::Neutral => Some(Row::Neutral),
            Who::Restricted => Some(Row::Restricted),
            Who::Stranger => Some(Row::Stranger),
            Who::Blocked => None,
        }
    }

    /// The list it is written on; a stranger is on none.
    pub fn standing(self) -> Option<Standing> {
        match self {
            Who::Safe => Some(Standing::Safe),
            Who::Neutral => Some(Standing::Neutral),
            Who::Restricted => Some(Standing::Restricted),
            Who::Blocked => Some(Standing::Blocked),
            Who::Stranger => None,
        }
    }

    /// Who a list makes someone.
    pub fn of(standing: Standing) -> Who {
        match standing {
            Standing::Safe => Who::Safe,
            Standing::Neutral => Who::Neutral,
            Standing::Restricted => Who::Restricted,
            Standing::Blocked => Who::Blocked,
        }
    }
}

/// How someone reaches you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Mail,
    /// Phone calls (docs/research/call-screening.md).
    Calls,
    /// Messages from other apps: texts, chats (docs/research/android-chats.md).
    Messages,
}

impl Channel {
    pub const ALL: [Channel; 3] = [Channel::Mail, Channel::Calls, Channel::Messages];

    pub fn id(self) -> &'static str {
        match self {
            Channel::Mail => "mail",
            Channel::Calls => "calls",
            Channel::Messages => "messages",
        }
    }

    pub fn read(text: &str) -> Option<Channel> {
        Channel::ALL.into_iter().find(|c| c.id() == text.trim().to_ascii_lowercase())
    }

    /// Its matrix's rows, in order: calls add hidden numbers.
    pub fn rows(self) -> &'static [Row] {
        match self {
            Channel::Calls => &Row::ALL,
            Channel::Mail | Channel::Messages => &Row::ALL[..4],
        }
    }
}

/// A row of a matrix: a state that has one, or, for calls, hidden numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Row {
    Safe,
    Neutral,
    Restricted,
    Stranger,
    /// A call without a number: someone who hides it, a hospital's switchboard,
    /// a number from abroad that failed its checks (research, 3.4 and 7.2).
    Hidden,
}

impl Row {
    pub const ALL: [Row; 5] = [Row::Safe, Row::Neutral, Row::Restricted, Row::Stranger, Row::Hidden];

    pub fn id(self) -> &'static str {
        match self {
            Row::Safe => "safe",
            Row::Neutral => "neutral",
            Row::Restricted => "restricted",
            Row::Stranger => "stranger",
            Row::Hidden => "hidden",
        }
    }

    pub fn read(text: &str) -> Option<Row> {
        Row::ALL.into_iter().find(|r| r.id() == text.trim().to_ascii_lowercase())
    }
}

/// A column of a matrix: one of the five times, or the pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Column {
    Work,
    Admin,
    Leisure,
    Meals,
    Sleep,
    /// The pause (« En pause », docs/pauses.md), whatever the time.
    Pause,
}

impl Column {
    pub const ALL: [Column; 6] = [Column::Work, Column::Admin, Column::Leisure, Column::Meals, Column::Sleep, Column::Pause];

    pub fn id(self) -> &'static str {
        match self {
            Column::Work => "work",
            Column::Admin => "admin",
            Column::Leisure => "leisure",
            Column::Meals => "meals",
            Column::Sleep => "sleep",
            Column::Pause => "pause",
        }
    }

    /// A word of the configuration: a time's (`Time::parse`), or "pause".
    pub fn read(word: &str) -> Option<Column> {
        match word.trim().to_lowercase().as_str() {
            "pause" | "paused" | "en pause" => Some(Column::Pause),
            other => match Time::parse(other)? {
                Time::Work => Some(Column::Work),
                Time::Admin => Some(Column::Admin),
                Time::Leisure => Some(Column::Leisure),
                Time::Meals => Some(Column::Meals),
                Time::Sleep => Some(Column::Sleep),
                Time::Any | Time::Several(_) => None,
            },
        }
    }
}

/// A row's boxes: the columns ticked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Times {
    pub work: bool,
    pub admin: bool,
    pub leisure: bool,
    pub meals: bool,
    pub sleep: bool,
    /// The pause: "En pause", whatever the time.
    pub pause: bool,
}

impl Times {
    /// Every column: always.
    pub const ALL: Times = Times { work: true, admin: true, leisure: true, meals: true, sleep: true, pause: true };
    /// No column: never.
    pub const NEVER: Times = Times { work: false, admin: false, leisure: false, meals: false, sleep: false, pause: false };

    /// From the columns' words, as the configuration writes them ("work",
    /// "admin", "leisure", "meals", "sleep", "pause"); others are left aside.
    pub fn parse(words: &[String]) -> Times {
        let mut times = Times::NEVER;
        for column in words.iter().filter_map(|w| Column::read(w)) {
            times.set(column, true);
        }
        times
    }

    /// The columns ticked, as the configuration writes them, in the matrix's order.
    pub fn ids(self) -> Vec<&'static str> {
        Column::ALL.into_iter().filter(|c| self.has(*c)).map(Column::id).collect()
    }

    /// Whether this column is ticked.
    pub fn has(self, column: Column) -> bool {
        match column {
            Column::Work => self.work,
            Column::Admin => self.admin,
            Column::Leisure => self.leisure,
            Column::Meals => self.meals,
            Column::Sleep => self.sleep,
            Column::Pause => self.pause,
        }
    }

    pub fn set(&mut self, column: Column, on: bool) {
        match column {
            Column::Work => self.work = on,
            Column::Admin => self.admin = on,
            Column::Leisure => self.leisure = on,
            Column::Meals => self.meals = on,
            Column::Sleep => self.sleep = on,
            Column::Pause => self.pause = on,
        }
    }

    /// Whether this one time is ticked: no hours set at all, every time is; work
    /// and admin hours open together, either of them.
    pub fn ticked(self, time: Time) -> bool {
        match time {
            Time::Work => self.work,
            Time::Admin => self.admin,
            Time::Leisure => self.leisure,
            Time::Meals => self.meals,
            Time::Sleep => self.sleep,
            Time::Any => true,
            Time::Several(open) => (open.work && self.work) || (open.admin && self.admin),
        }
    }

    /// Whether these times let someone through in `time`. While admin has no
    /// hours of its own, work time takes its ticks too; while work has none,
    /// admin time takes work's, as `areas::in_view` lends them.
    pub fn at(self, time: Time, week: Week) -> bool {
        match time {
            Time::Work => self.work || (!week.admin_hours && self.admin),
            Time::Admin => self.admin || (!week.work_hours && self.work),
            Time::Several(open) => (open.work && self.at(Time::Work, week)) || (open.admin && self.at(Time::Admin, week)),
            one => self.ticked(one),
        }
    }
}

/// What the matrix needs of a moment: what now is for, the week's hours, the pauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    /// What now is for (`quiet::Mode::time`); the night's wind-down and naps are sleep.
    pub time: Time,
    /// Which hours the week has: while admin has none, work time takes its ticks; the reverse too.
    pub week: Week,
    /// The pause (« En pause »): its own column, whatever the time.
    pub paused: bool,
    /// Free time (« Temps libre »): only the safe, at leisure's ticks.
    pub free: bool,
    /// Free time's "Nothing at all": not even the safe.
    pub nothing: bool,
}

impl Moment {
    /// From the time now (`quiet::mode`), and Free time's "Nothing at all" (`pause::nothing_now`).
    pub fn of(mode: &Mode, nothing: bool) -> Moment {
        Moment { time: mode.time, week: mode.week, paused: mode.paused(), free: mode.free(), nothing }
    }

    /// The columns it falls in: one; two when work and admin hours are open
    /// together; work and admin when no hours are set at all (`Time::Any`).
    pub fn columns(&self) -> Vec<Column> {
        if self.paused {
            return vec![Column::Pause];
        }
        match self.time {
            Time::Work => vec![Column::Work],
            Time::Admin => vec![Column::Admin],
            Time::Leisure => vec![Column::Leisure],
            Time::Meals => vec![Column::Meals],
            Time::Sleep => vec![Column::Sleep],
            Time::Any => vec![Column::Work, Column::Admin],
            Time::Several(open) => [(open.work, Column::Work), (open.admin, Column::Admin)].into_iter().filter(|(on, _)| *on).map(|(_, c)| c).collect(),
        }
    }
}

/// One channel's matrix: the columns ticked for each row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Matrix {
    pub safe: Times,
    pub neutral: Times,
    pub restricted: Times,
    pub stranger: Times,
    /// Calls only: hidden numbers.
    pub hidden: Times,
}

impl Matrix {
    /// Mail, as before the five states: the safe at any time (the pause as
    /// sleep), the neutral in work and admin time, the restricted in work
    /// time; strangers as the neutral.
    pub const MAIL: Matrix = Matrix {
        safe: Times::ALL,
        neutral: Times { work: true, admin: true, ..Times::NEVER },
        restricted: Times { work: true, ..Times::NEVER },
        stranger: Times { work: true, admin: true, ..Times::NEVER },
        hidden: Times::NEVER,
    };

    /// Calls (docs/research/call-screening.md, 8.2): the safe ring but during
    /// sleep and the pause; the neutral in work and admin time; the restricted
    /// in work time; strangers never; hidden numbers in work and admin time,
    /// when hospitals call that way.
    pub const CALLS: Matrix = Matrix {
        safe: Times { sleep: false, pause: false, ..Times::ALL },
        neutral: Times { work: true, admin: true, ..Times::NEVER },
        restricted: Times { work: true, ..Times::NEVER },
        stranger: Times::NEVER,
        hidden: Times { work: true, admin: true, ..Times::NEVER },
    };

    pub fn row(&self, row: Row) -> Times {
        match row {
            Row::Safe => self.safe,
            Row::Neutral => self.neutral,
            Row::Restricted => self.restricted,
            Row::Stranger => self.stranger,
            Row::Hidden => self.hidden,
        }
    }

    pub fn set(&mut self, row: Row, times: Times) {
        match row {
            Row::Safe => self.safe = times,
            Row::Neutral => self.neutral = times,
            Row::Restricted => self.restricted = times,
            Row::Stranger => self.stranger = times,
            Row::Hidden => self.hidden = times,
        }
    }

    /// When someone comes; the blocked never.
    pub fn times(&self, who: Who) -> Times {
        who.row().map_or(Times::NEVER, |row| self.row(row))
    }

    /// Whether a row may reach you at this moment: in the pause, its pause
    /// box; in Free time, the safe alone (no one with "Nothing at all"), at
    /// leisure's ticks; no hours set at all, as in work and admin hours
    /// together; else the time's box, lent as `Times::at` says.
    pub fn allows(&self, row: Row, at: &Moment) -> bool {
        let times = self.row(row);
        if at.paused {
            return times.pause;
        }
        if at.free && (row != Row::Safe || at.nothing) {
            return false;
        }
        match at.time {
            Time::Any => times.at(Time::Work, at.week) || times.at(Time::Admin, at.week),
            time => times.at(time, at.week),
        }
    }

    /// Whether someone may reach you at this moment; the blocked never.
    pub fn allows_who(&self, who: Who, at: &Moment) -> bool {
        who.row().is_some_and(|row| self.allows(row, at))
    }
}

/// Who may reach you when, channel by channel: `[reach]` in the configuration,
/// shared with your settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Reach {
    pub mail: Matrix,
    pub calls: Matrix,
    pub messages: Matrix,
}

impl Default for Reach {
    /// Mail as before; calls as the research proposes; messages as mail.
    fn default() -> Reach {
        Reach { mail: Matrix::MAIL, calls: Matrix::CALLS, messages: Matrix::MAIL }
    }
}

impl Reach {
    /// As the configuration says. A row left out keeps its usual times: mail's
    /// as before, calls' as `Matrix::CALLS`, and messages' those of mail's
    /// row. A `[reach]` written before the five states (no `stranger` row in
    /// it) keeps mail as it was: strangers as the neutral, and each row's
    /// pause as its sleep, since the pause was sleep's then.
    pub fn of(settings: &ReachSettings) -> Reach {
        let before = settings.stranger.is_none();
        let mail_row = |set: &Option<Vec<String>>, usual: Times| match set {
            None => usual,
            Some(words) => {
                let mut times = Times::parse(words);
                if before {
                    times.pause = times.sleep;
                }
                times
            }
        };
        let mut mail = Matrix::MAIL;
        mail.safe = mail_row(&settings.safe, Matrix::MAIL.safe);
        mail.neutral = mail_row(&settings.neutral, Matrix::MAIL.neutral);
        mail.restricted = mail_row(&settings.restricted, Matrix::MAIL.restricted);
        mail.stranger = mail_row(&settings.stranger, mail.neutral);
        let rows = |set: &RowSettings, usual: &Matrix| {
            let row = |words: &Option<Vec<String>>, usual: Times| words.as_deref().map_or(usual, Times::parse);
            Matrix {
                safe: row(&set.safe, usual.safe),
                neutral: row(&set.neutral, usual.neutral),
                restricted: row(&set.restricted, usual.restricted),
                stranger: row(&set.stranger, usual.stranger),
                hidden: row(&set.hidden, usual.hidden),
            }
        };
        let messages_usual = Matrix { hidden: Times::NEVER, ..mail };
        Reach { mail, calls: rows(&settings.calls, &Matrix::CALLS), messages: rows(&settings.messages, &messages_usual) }
    }

    /// Read from the configuration.
    pub fn load(config: &Config) -> Reach {
        Reach::of(&config.reach)
    }

    pub fn matrix(&self, channel: Channel) -> &Matrix {
        match channel {
            Channel::Mail => &self.mail,
            Channel::Calls => &self.calls,
            Channel::Messages => &self.messages,
        }
    }

    pub fn matrix_mut(&mut self, channel: Channel) -> &mut Matrix {
        match channel {
            Channel::Mail => &mut self.mail,
            Channel::Calls => &mut self.calls,
            Channel::Messages => &mut self.messages,
        }
    }

    /// Whether `who` may reach you on `channel` at this moment; the blocked never.
    pub fn allows(&self, channel: Channel, who: Who, at: &Moment) -> bool {
        self.matrix(channel).allows_who(who, at)
    }

    /// Whether a row may reach you on `channel` at this moment: hidden numbers for calls.
    pub fn allows_row(&self, channel: Channel, row: Row, at: &Moment) -> bool {
        self.matrix(channel).allows(row, at)
    }
}

/// A span of time with one moment: from `start` to `end`, Unix seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub start: i64,
    pub end: i64,
    pub moment: Moment,
}

/// The time at any instant, as `quiet::mode` decides it, and where it
/// changes: the hours, time off and the overrides as kept now, Health's meals,
/// naps and nights for the day asked (with each day's changes), the events
/// that push meals. The pause stays on until you come back; Free time ends by
/// itself (the night's start, else midnight).
#[derive(Debug, Clone)]
pub struct Clock {
    windows: Vec<AdminWindow>,
    time_off: Vec<TimeOff>,
    overrides: Overrides,
    needs: Needs,
    days: Days,
    held: Vec<(i64, i64)>,
    /// Free time's "Nothing at all", for this Free time.
    nothing: bool,
}

impl Clock {
    /// From what is read already: the overrides (`quiet.toml`), Health's needs
    /// and each day's changes, the events' spans with their margins (`plan::event_spans`).
    pub fn new(config: &Config, overrides: Overrides, needs: Needs, days: Days, held: Vec<(i64, i64)>) -> Clock {
        let nothing = crate::pause::nothing_now(&overrides, &config.free_time);
        Clock { windows: config.week_hours(), time_off: config.time_off.clone(), overrides, needs, days, held, nothing }
    }

    /// Read from the files: the overrides, Health, each day's changes, and the
    /// events from the day before `now` to `HORIZON_DAYS` after it.
    pub fn load(config: &Config, now: &Zoned) -> Clock {
        let overrides = Overrides::load(&Overrides::default_path());
        let needs = crate::health::Health::load(&crate::health::Health::default_path()).needs;
        let days = Days::load(&Days::default_path());
        let stamp = now.timestamp().as_second();
        let events = crate::agenda::occurrences(stamp - 86_400, stamp + (HORIZON_DAYS + 1) * 86_400);
        Clock::new(config, overrides, needs, days, crate::plan::event_spans(&events, 0))
    }

    /// Health's blocks around `at`: its day's and the next.
    pub fn blocks(&self, at: &Zoned) -> Blocks {
        Blocks::of(&self.needs, &self.days, &self.held, at)
    }

    /// The time at `at`.
    pub fn mode(&self, at: &Zoned) -> Mode {
        crate::quiet::mode(&self.windows, &self.time_off, &self.overrides, &self.blocks(at), at)
    }

    /// The matrix's moment at `at`.
    pub fn moment(&self, at: &Zoned) -> Moment {
        Moment::of(&self.mode(at), self.nothing)
    }

    /// The moment at `at`, and when it may change next: the time's own end, or
    /// a meal's, a nap's or a night's start or end, whichever comes first;
    /// none in the pause, which lasts until you come back.
    pub fn step(&self, at: &Zoned) -> (Moment, Option<Zoned>) {
        let blocks = self.blocks(at);
        let mode = crate::quiet::mode(&self.windows, &self.time_off, &self.overrides, &blocks, at);
        let moment = Moment::of(&mode, self.nothing);
        if mode.paused() {
            return (moment, None);
        }
        let stamp = at.timestamp().as_second();
        let edge = blocks.kept.iter().flat_map(|k| [k.start, k.end]).filter(|t| *t > stamp).min().and_then(|t| Timestamp::from_second(t).ok()).map(|t| t.to_zoned(at.time_zone().clone()));
        let next = match (mode.until, edge) {
            (Some(a), Some(b)) => Some(if b.timestamp() < a.timestamp() { b } else { a }),
            (a, b) => a.or(b),
        };
        (moment, next.filter(|n| n.timestamp() > at.timestamp()))
    }

    /// The moments from `from` to `until`, each with its span; two spans in a
    /// row with the same moment are one. A pause lasts to `until`.
    pub fn frames(&self, from: &Zoned, until: &Zoned) -> Vec<Frame> {
        let end = until.timestamp().as_second();
        let mut out: Vec<Frame> = Vec::new();
        let mut at = from.clone();
        // A change a minute at most, over the span: never a loop without end.
        for _ in 0..4096 {
            let start = at.timestamp().as_second();
            if start >= end {
                break;
            }
            let (moment, next) = self.step(&at);
            let stop = next.as_ref().map_or(end, |n| n.timestamp().as_second().min(end));
            match out.last_mut() {
                Some(last) if last.moment == moment && last.end == start => last.end = stop,
                _ => out.push(Frame { start, end: stop, moment }),
            }
            match next {
                Some(next) => at = next,
                None => break,
            }
        }
        out
    }

    /// Health's blocks between `from` and `to` (Unix seconds as `Zoned`):
    /// meals, naps and nights, each once, in order of start.
    pub fn kept(&self, from: &Zoned, to: &Zoned) -> Vec<Kept> {
        let (start, end) = (from.timestamp().as_second(), to.timestamp().as_second());
        let mut out: Vec<Kept> = Vec::new();
        let mut day = from.date();
        while let Ok(noon) = day.at(12, 0, 0, 0).to_zoned(from.time_zone().clone()) {
            if noon.timestamp().as_second() - 86_400 > end {
                break;
            }
            for kept in self.blocks(&noon).kept {
                if kept.end > start && kept.start < end && !out.iter().any(|k| k.key == kept.key && k.start == kept.start) {
                    out.push(kept);
                }
            }
            let Ok(next) = day.tomorrow() else { break };
            day = next;
        }
        out.sort_by_key(|k| k.start);
        out
    }
}

/// The first moment from `from` at which `row` may reach you on `channel`:
/// `from` itself when it may now; none within `HORIZON_DAYS`, or while a pause
/// with no end of its own holds them back.
pub fn next_allowed(reach: &Reach, channel: Channel, row: Row, from: &Zoned, clock: &Clock) -> Option<Zoned> {
    let matrix = reach.matrix(channel);
    let horizon = from.timestamp().as_second() + HORIZON_DAYS * 86_400;
    let mut at = from.clone();
    for _ in 0..1024 {
        let (moment, next) = clock.step(&at);
        if matrix.allows(row, &moment) {
            return Some(at);
        }
        let next = next?;
        if next.timestamp().as_second() > horizon {
            return None;
        }
        at = next;
    }
    None
}

/// The country of numbers written without one: the contacts' setting, else
/// the system's locale, else Sioul's language's (`phones::chosen`).
pub fn region(config: &Config) -> Option<&'static crate::phones::Region> {
    let language = config.language.clone().unwrap_or_else(crate::i18n::system_language);
    // As each language's `qt-locale` says (sioul.ftl): read without loading its sentences.
    let locale = if language.to_ascii_lowercase().starts_with("fr") { "fr_FR" } else { "en_GB" };
    crate::phones::chosen(config.contacts.region.as_deref(), locale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::Area;
    use crate::config::{ReachSettings, RowSettings};
    use jiff::tz::TimeZone;

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    fn set() -> Week {
        Week { work_hours: true, admin_hours: true, meals: true, sleep: true }
    }

    fn moment(time: Time) -> Moment {
        Moment { time, week: set(), paused: false, free: false, nothing: false }
    }

    #[test]
    fn five_states_and_their_rows() {
        assert_eq!(Who::ALL.map(Who::id), ["safe", "neutral", "restricted", "stranger", "blocked"]);
        assert_eq!((Who::read(" Stranger "), Who::read("grey")), (Some(Who::Stranger), None));
        assert_eq!((Who::Blocked.row(), Who::Stranger.row(), Who::Stranger.standing()), (None, Some(Row::Stranger), None));
        assert_eq!(Who::of(Standing::Restricted), Who::Restricted);
        assert_eq!(serde_json::to_string(&Who::Stranger).unwrap(), "\"stranger\"");
        assert_eq!(Channel::Calls.rows().last(), Some(&Row::Hidden));
        assert!(!Channel::Mail.rows().contains(&Row::Hidden) && !Channel::Messages.rows().contains(&Row::Hidden));
        assert_eq!((Channel::read("Calls"), Row::read("hidden")), (Some(Channel::Calls), Some(Row::Hidden)));
        assert_eq!((Column::read("Repas"), Column::read("pause"), Column::read("any")), (Some(Column::Meals), Some(Column::Pause), None));
        assert_eq!(Times::parse(&["Travail".into(), "nonsense".into(), "En pause".into()]).ids(), vec!["work", "pause"]);
    }

    #[test]
    fn calls_refused_but_from_the_lists() {
        let reach = Reach::default();
        let calls = Channel::Calls;
        // Work: the three levels ring, and hidden numbers; strangers and the blocked never.
        let work = moment(Time::Work);
        assert!(reach.allows(calls, Who::Safe, &work) && reach.allows(calls, Who::Neutral, &work) && reach.allows(calls, Who::Restricted, &work));
        assert!(!reach.allows(calls, Who::Stranger, &work) && !reach.allows(calls, Who::Blocked, &work));
        assert!(reach.allows_row(calls, Row::Hidden, &work));
        // Leisure: the safe alone; hidden numbers wait.
        let leisure = moment(Time::Leisure);
        assert!(reach.allows(calls, Who::Safe, &leisure) && !reach.allows(calls, Who::Neutral, &leisure) && !reach.allows_row(calls, Row::Hidden, &leisure));
        // Sleep and the pause: nobody rings (the do-not-disturb people are the calls' floor, not the matrix's).
        let sleep = moment(Time::Sleep);
        let paused = Moment { paused: true, ..sleep };
        assert!(Row::ALL.iter().all(|r| !reach.allows_row(calls, *r, &sleep) && !reach.allows_row(calls, *r, &paused)));
        // Mail: the safe in the pause, as when the pause was sleep's; messages as mail.
        assert!(reach.allows(Channel::Mail, Who::Safe, &paused) && !reach.allows(Channel::Mail, Who::Neutral, &paused));
        assert!(reach.allows(Channel::Messages, Who::Stranger, &work) && !reach.allows(Channel::Messages, Who::Stranger, &leisure));
        // Free time: the safe alone, at leisure's ticks; with "Nothing at all", nobody.
        let free = Moment { free: true, ..leisure };
        assert!(reach.allows(calls, Who::Safe, &free) && !reach.allows(Channel::Mail, Who::Neutral, &Moment { time: Time::Work, ..free }));
        assert!(!reach.allows(calls, Who::Safe, &Moment { nothing: true, ..free }));
        // Work and admin hours at once: either; no hours set at all: as both together.
        assert!(reach.allows(calls, Who::Restricted, &moment(Time::Several(Area::MIXED))));
        let no_hours = Moment { time: Time::Any, week: Week::default(), ..work };
        assert!(reach.allows(calls, Who::Neutral, &no_hours) && !reach.allows(calls, Who::Stranger, &no_hours));
        assert_eq!((no_hours.columns(), paused.columns()), (vec![Column::Work, Column::Admin], vec![Column::Pause]));
        // Lending: without admin hours, work time takes admin's ticks.
        let admin_only = Matrix { neutral: Times { admin: true, ..Times::NEVER }, ..Matrix::CALLS };
        assert!(admin_only.allows(Row::Neutral, &Moment { week: Week { admin_hours: false, ..set() }, ..work }));
        assert!(!admin_only.allows(Row::Neutral, &work));
        // Always and never: a row with every box, or none.
        let always = Matrix { stranger: Times::ALL, ..Matrix::CALLS };
        assert!([work, leisure, sleep, paused].iter().all(|m| always.allows(Row::Stranger, m)));
    }

    #[test]
    fn rows_from_the_configuration() {
        // Before the five states: mail as it was (the pause as sleep, strangers as the neutral); calls as the research proposes; messages as mail.
        let older = ReachSettings { safe: Some(vec!["work".into(), "sleep".into()]), neutral: Some(vec!["leisure".into()]), ..ReachSettings::default() };
        let reach = Reach::of(&older);
        assert_eq!((reach.mail.safe.ids(), reach.mail.stranger.ids()), (vec!["work", "sleep", "pause"], vec!["leisure"]));
        assert_eq!((reach.calls, reach.messages), (Matrix::CALLS, Matrix { hidden: Times::NEVER, ..reach.mail }));
        assert_eq!(Reach::of(&ReachSettings::default()), Reach::default());
        // Written since: as said, the pause too; rows of their own for messages and calls.
        let newer = ReachSettings {
            safe: Some(vec!["work".into(), "sleep".into()]),
            stranger: Some(Vec::new()),
            messages: RowSettings { neutral: Some(vec!["meals".into()]), ..RowSettings::default() },
            calls: RowSettings { hidden: Some(vec!["work".into(), "pause".into()]), ..RowSettings::default() },
            ..ReachSettings::default()
        };
        let reach = Reach::of(&newer);
        assert_eq!((reach.mail.safe.ids(), reach.mail.stranger), (vec!["work", "sleep"], Times::NEVER));
        assert_eq!((reach.messages.neutral.ids(), reach.messages.safe, reach.messages.stranger), (vec!["meals"], reach.mail.safe, Times::NEVER));
        assert_eq!((reach.calls.hidden.ids(), reach.calls.safe), (vec!["work", "pause"], Matrix::CALLS.safe));
    }

    /// Monday to Friday, 9:00–17:00.
    fn weekdays() -> Config {
        let text = ["monday", "tuesday", "wednesday", "thursday", "friday"].map(|d| format!("[[window]]\nday = \"{d}\"\nstart = \"09:00\"\nend = \"17:00\"\n")).join("\n");
        toml::from_str(&text).unwrap()
    }

    /// Meals and nights as Health first sets them: breakfast 07:50–08:20, lunch
    /// 12:10–13:00, dinner 19:00–20:00, the night from 22:00 to 07:00.
    fn health() -> Needs {
        Needs { meals_on: true, sleep_on: true, ..Needs::default() }
    }

    #[test]
    fn the_clock_cuts_at_meals_and_nights() {
        let config = weekdays();
        let clock = Clock::new(&config, Overrides::default(), health(), Days::default(), Vec::new());
        let reach = Reach::default();
        // Friday 2 October 2026 at 10:00: work, until lunch is got ready.
        let friday = at("2026-10-02T10:00[Europe/Paris]");
        let (now, next) = clock.step(&friday);
        assert_eq!((now.time, next.map(|n| n.datetime().to_string())), (Time::Work, Some("2026-10-02T12:10:00".into())));
        // A neutral caller rings now; a stranger never within the horizon.
        assert_eq!(next_allowed(&reach, Channel::Calls, Row::Neutral, &friday, &clock), Some(friday.clone()));
        assert_eq!(next_allowed(&reach, Channel::Calls, Row::Stranger, &friday, &clock), None);
        // Restricted, on Friday evening: Monday at 9:00, past the weekend's meals and nights.
        let evening = at("2026-10-02T19:30[Europe/Paris]");
        assert_eq!(next_allowed(&reach, Channel::Calls, Row::Restricted, &evening, &clock).map(|z| z.datetime().to_string()), Some("2026-10-05T09:00:00".into()));
        // The safe ring in leisure, not at night: at 22:30, waking.
        let late = at("2026-10-02T22:30[Europe/Paris]");
        assert_eq!(next_allowed(&reach, Channel::Calls, Row::Safe, &late, &clock).map(|z| z.datetime().to_string()), Some("2026-10-03T07:00:00".into()));
        // Done for the day: leisure until Monday, yet the meals and the night still cut it.
        let done = Overrides { rest_until: Some(at("2026-10-05T09:00[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let clock = Clock::new(&config, done, health(), Days::default(), Vec::new());
        let paris = TimeZone::get("Europe/Paris").unwrap();
        let hour = |s: i64| Timestamp::from_second(s).unwrap().to_zoned(paris.clone()).strftime("%a %H:%M").to_string();
        let frames = clock.frames(&at("2026-10-02T18:00[Europe/Paris]"), &at("2026-10-03T08:00[Europe/Paris]"));
        let said: Vec<(String, Time)> = frames.iter().map(|f| (hour(f.start), f.moment.time)).collect();
        let want = [("Fri 18:00", Time::Leisure), ("Fri 19:00", Time::Meals), ("Fri 20:00", Time::Leisure), ("Fri 22:00", Time::Sleep), ("Sat 07:00", Time::Leisure), ("Sat 07:50", Time::Meals)];
        assert_eq!(said, want.map(|(h, t)| (h.to_string(), t)).to_vec());
        assert_eq!(frames.last().map(|f| hour(f.end)), Some("Sat 08:00".into()));
        assert_eq!(clock.kept(&at("2026-10-02T18:00[Europe/Paris]"), &at("2026-10-03T08:00[Europe/Paris]")).iter().map(|k| k.kind).collect::<Vec<_>>(), ["meal", "sleep", "meal"]);
        // The pause: one frame to the end, and nobody rings until you come back; mail's safe still come.
        let paused = Overrides { paused_since: Some(friday.timestamp().as_second() - 60), ..Overrides::default() };
        let clock = Clock::new(&config, paused, health(), Days::default(), Vec::new());
        let frames = clock.frames(&friday, &at("2026-10-03T10:00[Europe/Paris]"));
        assert_eq!((frames.len(), frames[0].moment.paused, frames[0].moment.columns()), (1, true, vec![Column::Pause]));
        assert_eq!(next_allowed(&reach, Channel::Calls, Row::Safe, &friday, &clock), None);
        assert_eq!(next_allowed(&reach, Channel::Mail, Row::Safe, &friday, &clock), Some(friday.clone()));
    }
}
