// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Who reaches you, on which channel (docs/porch.md, "Who may reach you,
//! and when"), and the clock that says what time it is, ahead.
//!
//! Five states, one vocabulary everywhere: a **stranger** (in none of your
//! address books and on no list), the **blocked** (never, on any channel),
//! and for the people in your address books three levels: **safe**,
//! **neutral** (anyone in your address books, unless you chose otherwise) and
//! **restricted**. Who someone is, the lists and the address books say
//! (`porch::Senders`): their own entry first, then their card's, its
//! categories, a domain or a number's prefix.
//!
//! When each reaches you is the matrix of what reaches you's to say
//! (`attention`, docs/attention.md): a row per state on each channel.
//!
//! The clock reads no file once made: a phone writes the calls' frames ahead
//! from it (`calls::frames`), which its call screening reads within milliseconds.

use crate::areas::{Time, Week};
use crate::config::{Config, TimeOff};
use crate::needs::{Days, Kept, Needs};
use crate::porch::Standing;
use crate::quiet::{Blocks, Mode, Overrides};
use crate::window::AdminWindow;
use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};

/// How far ahead a clock's events are read, in days.
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
}

/// What the matrix of what reaches you needs of a moment ahead
/// (`attention::Now::of_moment`): what now is for, the week's hours, the pauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    /// What now is for (`quiet::Mode::time`); the night's wind-down and naps are sleep.
    pub time: Time,
    /// Which hours the week has: while admin has none, work time takes its cells; the reverse too.
    pub week: Week,
    /// The pause (« En pause »): its own column, whatever the time.
    pub paused: bool,
    /// Free time (« Temps libre »): its own column.
    pub free: bool,
    /// Free time's "Nothing at all": the states' rows wait, Always through aside.
    pub nothing: bool,
}

impl Moment {
    /// From the time now (`quiet::mode`), and Free time's "Nothing at all" (`pause::nothing_now`).
    pub fn of(mode: &Mode, nothing: bool) -> Moment {
        Moment { time: mode.time, week: mode.week, paused: mode.paused(), free: mode.free(), nothing }
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
    /// a meal's, a nap's or a night's start or end, or Free time's start (a
    /// table made from midnight after it was pressed), whichever comes first;
    /// none in the pause, which lasts until you come back.
    pub fn step(&self, at: &Zoned) -> (Moment, Option<Zoned>) {
        let blocks = self.blocks(at);
        let mode = crate::quiet::mode(&self.windows, &self.time_off, &self.overrides, &blocks, at);
        let moment = Moment::of(&mode, self.nothing);
        if mode.paused() {
            return (moment, None);
        }
        let stamp = at.timestamp().as_second();
        let free = self.overrides.free_from().filter(|since| *since > stamp);
        let edge = blocks.kept.iter().flat_map(|k| [k.start, k.end]).chain(free).filter(|t| *t > stamp).min().and_then(|t| Timestamp::from_second(t).ok()).map(|t| t.to_zoned(at.time_zone().clone()));
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
    use jiff::tz::TimeZone;

    fn at(text: &str) -> Zoned {
        text.parse().unwrap()
    }

    #[test]
    fn five_states_and_three_channels() {
        assert_eq!(Who::ALL.map(Who::id), ["safe", "neutral", "restricted", "stranger", "blocked"]);
        assert_eq!((Who::read(" Stranger "), Who::read("grey")), (Some(Who::Stranger), None));
        assert_eq!(Who::Stranger.standing(), None);
        assert_eq!(Who::of(Standing::Restricted), Who::Restricted);
        assert_eq!(serde_json::to_string(&Who::Stranger).unwrap(), "\"stranger\"");
        assert_eq!((Channel::read("Calls"), Channel::read("letters")), (Some(Channel::Calls), None));
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
        // Friday 2 October 2026 at 10:00: work, until lunch is got ready.
        let friday = at("2026-10-02T10:00[Europe/Paris]");
        let (now, next) = clock.step(&friday);
        assert_eq!((now.time, next.map(|n| n.datetime().to_string())), (Time::Work, Some("2026-10-02T12:10:00".into())));
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
        // Free time pressed at 10:30 on a working day: a frame of its own from then, read from midnight.
        let free = Overrides { free_since: Some(at("2026-10-02T10:30[Europe/Paris]").timestamp().as_second()), ..Overrides::default() };
        let clock = Clock::new(&config, free, health(), Days::default(), Vec::new());
        let frames = clock.frames(&at("2026-10-02T00:00[Europe/Paris]"), &at("2026-10-02T12:00[Europe/Paris]"));
        assert_eq!(frames.iter().map(|f| (hour(f.start), f.moment.free)).collect::<Vec<_>>(), [("Fri 00:00".to_string(), false), ("Fri 07:00".into(), false), ("Fri 07:50".into(), false), ("Fri 08:20".into(), false), ("Fri 09:00".into(), false), ("Fri 10:30".into(), true)]);
        // The pause: one frame to the end, with no end of its own.
        let paused = Overrides { paused_since: Some(friday.timestamp().as_second() - 60), ..Overrides::default() };
        let clock = Clock::new(&config, paused, health(), Days::default(), Vec::new());
        let frames = clock.frames(&friday, &at("2026-10-03T10:00[Europe/Paris]"));
        assert_eq!((frames.len(), frames[0].moment.paused), (1, true));
        assert_eq!(clock.step(&friday).1, None);
    }
}
