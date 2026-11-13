// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A notice: one notification Sioul posts, about one of its things
//! (docs/notifications.md). Every source makes it the same way: the thing it
//! is about, by its address (`about`, the address the links and the MCP
//! tools use); its kind; its words; its buttons, each a verb on the thing
//! (`Action`); and the thing's own end, when it has one (`until`).
//!
//! Its class follows from its kind (`Kind::class`): something to know
//! (`Class::Informative`) leaves after a few minutes (`Settings`), is posted
//! only while someone is at the device, and never during a pause or a
//! do-not-disturb; something to do (`Class::Actionable`) stays until its
//! thing is settled or ends, and passes a pause or a do-not-disturb only
//! when the attention model says it pierces. `decide` says it all at once,
//! from the attention model's answer (`attention::Output`: whether it is
//! told now, whether it pierces) and the moment (`Moment`): drop it, or post
//! it with its urgency, its timeout, whether the server keeps a trace of it,
//! and the key of the notice it replaces. Where and how it is posted is each
//! platform's (sioul-sync's `notify`, the window's `tell`).

use crate::attention;
use serde::{Deserialize, Serialize};

/// What a notice is: one of the kinds of Sioul's own the matrix has rows for
/// (`attention::Kind`); new mail, whose rows are the people's and which has
/// no kind of its own; or a minor word of Sioul's own outside the matrix
/// (the offer of a language found in your mail, calendar files handed to an
/// older Sioul).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum Kind {
    Own(attention::Kind),
    Mail,
    Minor,
}

/// The two classes of notice (docs/notifications.md, "Two classes").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// Something to know: a pause to move, a meal, the end of work, the
    /// night, a date, the time to leave, a site's message, new mail.
    Informative,
    /// Something to do: a code, a temporary password, a dose, an event's own
    /// alarm, a call in a site, the alarm at waking.
    Actionable,
}

impl Kind {
    /// Its class; none for what is no notice of Sioul's own: the time running
    /// (a status, while a focus session runs) and other apps' notifications
    /// on a phone, which Sioul holds or lets through but never posts.
    pub fn class(self) -> Option<Class> {
        match self {
            Kind::Own(kind) => kind.class(),
            Kind::Mail | Kind::Minor => Some(Class::Informative),
        }
    }

    /// Its id: the attention row's ("codes", "move"…), "mail" or "minor".
    pub fn id(self) -> &'static str {
        match self {
            Kind::Own(kind) => kind.id(),
            Kind::Mail => "mail",
            Kind::Minor => "minor",
        }
    }

    /// The kind an id names (`id`).
    pub fn read(id: &str) -> Option<Kind> {
        match id {
            "mail" => Some(Kind::Mail),
            "minor" => Some(Kind::Minor),
            _ => attention::Kind::read(id).map(Kind::Own),
        }
    }
}

impl From<Kind> for String {
    fn from(kind: Kind) -> String {
        kind.id().to_string()
    }
}

impl TryFrom<String> for Kind {
    type Error = String;

    fn try_from(id: String) -> Result<Kind, String> {
        Kind::read(&id).ok_or_else(|| format!("{id}: no kind of notice"))
    }
}

/// What a button does: a verb on the thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Opens a thing in the window, by its address; `hint` where this device
    /// last saw it, for the window to find it at once (a message's file, an
    /// event's file and the time it is for: `remind::event_time`), "" none.
    Open { address: String, hint: String },
    /// Puts these words on the clipboard (a code): the notice has done its work then.
    Copy(String),
    /// One of the window's own verbs, by its name (`tell`'s: "read-language:de").
    Verb(String),
}

impl Act {
    /// Its key as the notification server is given it; a second button of
    /// the same act is keyed with its place ("open-1").
    pub fn key(&self) -> &'static str {
        match self {
            Act::Open { .. } => "open",
            Act::Copy(_) => "copy",
            Act::Verb(_) => "do",
        }
    }
}

/// A button: its words, and what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub label: String,
    pub act: Act,
}

/// One notice, as every source makes it (docs/notifications.md, "The model").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The thing it is about, by its address: `mid:…`, `sioul:task/…`,
    /// `sioul:event/<UID>#<start>`, `sioul:need/<block>`, `sioul:move`,
    /// `sioul:day/work:<date>`, `sioul:site/<id>#<tag>`… A click on the
    /// notice opens it. "" for a notice of a process that will not hear the
    /// click (the command line, a Sioul handing over and leaving): the
    /// desktop then brings Sioul's window forward by itself.
    pub about: String,
    /// Where this device last saw the thing (`Act::Open`'s hint), "" none.
    pub hint: String,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    /// Its buttons, each a verb on the thing; a click on the notice itself opens `about`.
    pub actions: Vec<Action>,
    /// The thing's own end, Unix seconds, when it has one: a code's expiry, an event's start.
    pub until: Option<i64>,
    /// Its picture: a theme's icon name, or a file's path (a site's own
    /// icon); "" the kind's own.
    pub icon: String,
}

impl Notice {
    pub fn new(about: impl Into<String>, kind: Kind, title: impl Into<String>, body: impl Into<String>) -> Notice {
        Notice { about: about.into(), hint: String::new(), kind, title: title.into(), body: body.into(), actions: Vec::new(), until: None, icon: String::new() }
    }

    /// With a button.
    pub fn action(mut self, label: impl Into<String>, act: Act) -> Notice {
        self.actions.push(Action { label: label.into(), act });
        self
    }

    /// With where this device last saw its thing.
    pub fn hint(mut self, hint: impl Into<String>) -> Notice {
        self.hint = hint.into();
        self
    }

    /// With its thing's own end.
    pub fn until(mut self, until: Option<i64>) -> Notice {
        self.until = until;
        self
    }

    /// With a picture of its own.
    pub fn icon(mut self, icon: impl Into<String>) -> Notice {
        self.icon = icon.into();
        self
    }

    /// The key of the notice it replaces: one per kind and thing ("move" and
    /// `sioul:move`: one "Time to move" at most; a meal's heads-up and its
    /// notice at its time, the same need, one after the other).
    pub fn key(&self) -> String {
        replacing(self.kind, &self.about)
    }

    /// Its class (`Kind::class`).
    pub fn class(&self) -> Option<Class> {
        self.kind.class()
    }
}

/// The key a notice of `kind` about `about` replaces (`Notice::key`).
pub fn replacing(kind: Kind, about: &str) -> String {
    format!("{}|{about}", kind.id())
}

/// The addresses made for notices, beside the links' (`links`): the things
/// that have no address of their own elsewhere.
pub mod address {
    /// A dose to take, by its key (`health::Dose::key`).
    pub fn dose(key: &str) -> String {
        format!("sioul:dose/{}", super::encode(key))
    }

    /// A meal, a nap or the night, by its block's key (`needs::Kept::key`).
    pub fn need(block: &str) -> String {
        format!("sioul:need/{}", super::encode(block))
    }

    /// The pause to move ("Where I stopped").
    pub const MOVE: &str = "sioul:move";

    /// The end of the work day, for that day ("2026-11-13").
    pub fn work_day(date: &str) -> String {
        format!("sioul:day/work:{date}")
    }

    /// The end of the whole day, the night's, for that day.
    pub fn night(date: &str) -> String {
        format!("sioul:day/night:{date}")
    }

    /// A site's notice: the site, and the web notice's own key (its tag, one
    /// per conversation on a chat site; another key of the window's where the
    /// site gave none).
    pub fn site(id: &str, tag: &str) -> String {
        format!("sioul:site/{}#{}", super::encode(id), super::encode(tag))
    }

    /// An event's time: its address and the start it is for.
    pub fn event(uid: &str, start: i64) -> String {
        format!("{}#{start}", crate::links::event_uri(uid))
    }

    /// The mailbox at its top: new mail's notice of several messages, until
    /// each message has its own opener (phase 2 of the notifications).
    pub const MAILBOX: &str = "sioul:mailbox";

    /// The Porch: the sites' news gathered.
    pub const PORCH: &str = "sioul:porch";

    /// A tab of Settings, by its key ("words.languages").
    pub fn settings(key: &str) -> String {
        format!("sioul:settings/{key}")
    }
}

/// A key or a tag inside an address: "%", "/", "#", "?" and spaces
/// percent-encoded, so that the window's `decodeURIComponent` gives it back.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '%' | '/' | ' ' | '#' | '?' | '"' | '<' | '>' | '&' | '+') || c.is_control() {
            for byte in c.encode_utf8(&mut [0; 4]).bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// How long informative notices last (`[notices]` in the configuration;
/// Settings ▸ What reaches you).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Settings {
    /// Minutes; unsaid, five; read between one and sixty (`minutes`).
    #[serde(default)]
    pub informative_minutes: Option<i64>,
}

/// Informative notices' lifetime when unsaid, in minutes.
pub const INFORMATIVE_MINUTES: u32 = 5;
/// The shortest and the longest lifetime that may be set, in minutes.
pub const SHORTEST: u32 = 1;
pub const LONGEST: u32 = 60;

impl Settings {
    /// How long an informative notice lasts, in minutes: as set, between one
    /// and sixty; five when unsaid.
    pub fn minutes(&self) -> u32 {
        self.informative_minutes.map_or(INFORMATIVE_MINUTES, |m| m.clamp(i64::from(SHORTEST), i64::from(LONGEST)) as u32)
    }
}

/// The moment, as a notice sees it on the device that would post it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Moment {
    /// Someone is at this device (a computer: input in the last ten minutes
    /// and its session unlocked; unknown counts as someone).
    pub present: bool,
    /// Sioul's pause holds now.
    pub paused: bool,
    /// A do-not-disturb holds now: one of Sioul's modes (the switch, a focus
    /// session, the pause, Free time, the night), or the system's own, which
    /// Sioul follows.
    pub silenced: bool,
}

/// The urgency a notice is posted with, as the Desktop Notifications
/// specification numbers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Urgency {
    /// What every notice is posted with: the desktop's do-not-disturb holds it.
    Normal,
    /// An actionable notice that pierces, while a do-not-disturb holds: it passes it.
    Critical,
}

impl Urgency {
    pub fn level(self) -> u8 {
        match self {
            Urgency::Normal => 1,
            Urgency::Critical => 2,
        }
    }
}

/// How a notice is posted (`decide`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    pub class: Class,
    pub urgency: Urgency,
    /// The expire timeout given to the server, milliseconds; 0, never.
    pub timeout_ms: i32,
    /// No trace in the server's history once it goes (the hint `transient`).
    pub transient: bool,
    /// The key of the notice it replaces (`Notice::key`).
    pub key: String,
    /// When Sioul takes it away itself, Unix seconds (some servers keep a
    /// notice past its timeout); none: when its thing is settled.
    pub close_at: Option<i64>,
}

/// Why a notice is not posted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// No notice of Sioul's own (the time running, other apps').
    NotOurs,
    /// The attention model does not tell it now.
    NotTold,
    /// Informative, and nobody is at the device.
    Away,
    /// A pause or a do-not-disturb holds it: informative, or actionable and not piercing.
    Held,
    /// Its thing ended already.
    Over,
}

/// What becomes of a notice now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Drop(Dropped),
    Post(Shape),
}

/// The model's word for a notice its source decided to tell now (a reminder
/// due, a batch of mail its rows let through, a gathered time, a meal's
/// notice): told, piercing as its kind does (`attention::Kind::pierces`);
/// none for new mail and the minor notices, which never pierce here.
pub fn told(kind: Option<attention::Kind>) -> attention::Output {
    attention::Output { level: attention::Level::Now, shown: true, told: true, pierce: kind.is_some_and(attention::Kind::pierces), step: attention::Step::Matrix }
}

/// The moment of a process without the window (the command line), its
/// files read: someone counted as present (nothing says otherwise there),
/// Sioul's pause, and Sioul's do-not-disturb as every device reads it (its
/// modes, the switch: the desktop's own heard by the window lands there).
pub fn moment_from_files(config: &crate::config::Config, now: &jiff::Zoned) -> Moment {
    use crate::{everywhere, quiet};
    let overrides = quiet::Overrides::load(&quiet::Overrides::default_path());
    let blocks = quiet::Blocks::read_now(now);
    let mode = quiet::mode(&config.week_hours(), &config.time_off, &overrides, &blocks, now);
    let sources = everywhere::sources_at(&overrides, &blocks, now);
    let state = everywhere::now(&config.dnd, &sources, everywhere::read_switch("").latest().as_ref(), now.timestamp().as_second());
    Moment { present: true, paused: mode.paused(), silenced: state.on() }
}

/// What becomes of `notice` at `now` (Unix seconds), from what the attention
/// model says of it (`out`: told now, piercing), the `moment`, and the
/// lifetime of informative notices (`minutes`, `Settings::minutes`):
/// - informative: posted only when told, someone present, no pause and no
///   do-not-disturb holding; at normal urgency, for `minutes`, leaving no
///   trace; the time to leave (`attention::Kind::Before`) lasts until its
///   event starts instead; a thing that ends sooner takes it away then;
/// - actionable: posted when told, at a pause or a do-not-disturb only when
///   it pierces; at normal urgency, critical only when it pierces a
///   do-not-disturb that holds now; until its thing ends (`until`), else
///   until it is settled; never without someone present to see it later.
pub fn decide(notice: &Notice, out: &attention::Output, moment: Moment, minutes: u32, now: i64) -> Decision {
    let Some(class) = notice.class() else { return Decision::Drop(Dropped::NotOurs) };
    if !out.told {
        return Decision::Drop(Dropped::NotTold);
    }
    if notice.until.is_some_and(|until| until <= now) {
        return Decision::Drop(Dropped::Over);
    }
    let key = notice.key();
    match class {
        Class::Informative => {
            if !moment.present {
                return Decision::Drop(Dropped::Away);
            }
            if moment.paused || moment.silenced {
                return Decision::Drop(Dropped::Held);
            }
            let lifetime = i64::from(minutes.clamp(SHORTEST, LONGEST)) * 60;
            let ends = match (notice.kind, notice.until) {
                // The time to leave: until its event starts.
                (Kind::Own(attention::Kind::Before), Some(until)) => until,
                (_, Some(until)) => until.min(now + lifetime),
                (_, None) => now + lifetime,
            };
            Decision::Post(Shape { class, urgency: Urgency::Normal, timeout_ms: millis(ends - now), transient: true, key, close_at: Some(ends) })
        }
        Class::Actionable => {
            if (moment.paused || moment.silenced) && !out.pierce {
                return Decision::Drop(Dropped::Held);
            }
            let urgency = if out.pierce && moment.silenced { Urgency::Critical } else { Urgency::Normal };
            Decision::Post(Shape { class, urgency, timeout_ms: notice.until.map_or(0, |until| millis(until - now)), transient: false, key, close_at: notice.until })
        }
    }
}

/// Seconds as an expire timeout: milliseconds, at least one, at most what the call carries.
fn millis(seconds: i64) -> i32 {
    i32::try_from(seconds.max(1).saturating_mul(1000)).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use attention::{Kind as Row, Level, Output, Step};

    fn told(pierce: bool) -> Output {
        Output { level: Level::Now, shown: true, told: true, pierce, step: Step::Matrix }
    }

    fn untold() -> Output {
        Output { level: Level::Later, shown: false, told: false, pierce: false, step: Step::Matrix }
    }

    const NOW: i64 = 1_794_000_000;
    const HERE: Moment = Moment { present: true, paused: false, silenced: false };

    /// Each kind of the matrix in its class, as decided on 13 November 2026:
    /// what is to know, what is to do, and what is no notice of Sioul's own.
    #[test]
    fn each_kind_has_its_class() {
        use Class::{Actionable, Informative};
        let expected = [
            (Row::Codes, Some(Actionable)),
            (Row::Doses, Some(Actionable)),
            (Row::Wake, Some(Actionable)),
            (Row::Alarms, Some(Actionable)),
            (Row::Before, Some(Informative)),
            (Row::DayBefore, Some(Informative)),
            (Row::Dates, Some(Informative)),
            (Row::Needs, Some(Informative)),
            (Row::Move, Some(Informative)),
            (Row::WorkOver, Some(Informative)),
            (Row::Time, None),
            (Row::Sites, Some(Informative)),
            (Row::SitesLive, Some(Informative)),
            (Row::SiteCalls, Some(Actionable)),
            (Row::AppAutomatons, None),
            (Row::AppAtOnce, None),
        ];
        assert_eq!(expected.len(), Row::ALL.len(), "every kind said");
        for (kind, class) in expected {
            assert_eq!(kind.class(), class, "{kind:?}");
            assert_eq!(Kind::Own(kind).class(), class);
        }
        assert_eq!(Kind::Mail.class(), Some(Informative), "new mail is to know (decision 1)");
        assert_eq!(Kind::Minor.class(), Some(Informative));
        for kind in Row::ALL.map(Kind::Own).into_iter().chain([Kind::Mail, Kind::Minor]) {
            assert_eq!(Kind::read(kind.id()), Some(kind), "{kind:?} read back");
        }
    }

    /// Informative notices: posted at normal urgency for the lifetime set,
    /// leaving no trace, taken away by Sioul at its end; never to an empty
    /// desk, in a pause or under a do-not-disturb; never untold.
    #[test]
    fn something_to_know_leaves_after_its_minutes() {
        let moved = Notice::new(address::MOVE, Kind::Own(Row::Move), "Time to move", "Stand up");
        let Decision::Post(shape) = decide(&moved, &told(false), HERE, 5, NOW) else { panic!("posted") };
        assert_eq!(shape, Shape { class: Class::Informative, urgency: Urgency::Normal, timeout_ms: 300_000, transient: true, key: "move|sioul:move".into(), close_at: Some(NOW + 300) });
        // Its minutes as set, bounded.
        let Decision::Post(shape) = decide(&moved, &told(false), HERE, 12, NOW) else { panic!() };
        assert_eq!((shape.timeout_ms, shape.close_at), (720_000, Some(NOW + 720)));
        let Decision::Post(shape) = decide(&moved, &told(false), HERE, 600, NOW) else { panic!() };
        assert_eq!(shape.timeout_ms, 3_600_000);
        // Through do-not-disturb it never goes, even piercing (Always through's mail is shown on the Porch).
        let mail = Notice::new("mid:a@example.org", Kind::Mail, "One e-mail", "Noa, The lease");
        assert_eq!(decide(&mail, &told(true), Moment { silenced: true, ..HERE }, 5, NOW), Decision::Drop(Dropped::Held));
        assert_eq!(decide(&moved, &told(false), Moment { paused: true, ..HERE }, 5, NOW), Decision::Drop(Dropped::Held));
        assert_eq!(decide(&moved, &told(false), Moment { present: false, ..HERE }, 5, NOW), Decision::Drop(Dropped::Away));
        assert_eq!(decide(&moved, &untold(), HERE, 5, NOW), Decision::Drop(Dropped::NotTold));
        // A thing that ends sooner takes it away then.
        let date = Notice::new("sioul:task/a", Kind::Own(Row::Dates), "Due Friday", "").until(Some(NOW + 60));
        let Decision::Post(shape) = decide(&date, &told(false), HERE, 5, NOW) else { panic!() };
        assert_eq!(shape.close_at, Some(NOW + 60));
    }

    /// The time to leave lasts until its event starts, longer than the
    /// minutes set; once it started, nothing.
    #[test]
    fn the_time_to_leave_lasts_until_its_event() {
        let leave = Notice::new(address::event("dentist@example.org", NOW + 1_200), Kind::Own(Row::Before), "Leave in 5 min", "").until(Some(NOW + 1_200));
        let Decision::Post(shape) = decide(&leave, &told(false), HERE, 5, NOW) else { panic!() };
        assert_eq!((shape.timeout_ms, shape.close_at, shape.transient), (1_200_000, Some(NOW + 1_200), true));
        assert_eq!(decide(&leave, &told(false), HERE, 5, NOW + 1_200), Decision::Drop(Dropped::Over));
    }

    /// Actionable notices: until their thing ends or is settled; at normal
    /// urgency, critical only when they pierce a do-not-disturb that holds
    /// now; held by a pause or a do-not-disturb unless they pierce; posted
    /// with nobody at the desk, to be found there.
    #[test]
    fn something_to_do_stays_and_pierces_only_when_it_must() {
        let code = Notice::new("mid:code@example.org", Kind::Own(Row::Codes), "Code from example.org", "482913").until(Some(NOW + 1_800)).action("Copy", Act::Copy("482913".into()));
        let Decision::Post(shape) = decide(&code, &told(true), HERE, 5, NOW) else { panic!() };
        assert_eq!(shape, Shape { class: Class::Actionable, urgency: Urgency::Normal, timeout_ms: 1_800_000, transient: false, key: "codes|mid:code@example.org".into(), close_at: Some(NOW + 1_800) });
        let Decision::Post(shape) = decide(&code, &told(true), Moment { silenced: true, ..HERE }, 5, NOW) else { panic!() };
        assert_eq!(shape.urgency, Urgency::Critical, "it must pass a do-not-disturb on now");
        // A pause without the system's do-not-disturb: no need to be critical.
        let Decision::Post(shape) = decide(&code, &told(true), Moment { paused: true, ..HERE }, 5, NOW) else { panic!() };
        assert_eq!(shape.urgency, Urgency::Normal);
        let Decision::Post(_) = decide(&code, &told(true), Moment { present: false, ..HERE }, 5, NOW) else { panic!("waits on the screen") };
        // A call in a site that does not pierce waits for the do-not-disturb's end.
        let call = Notice::new(address::site("chat", "c1"), Kind::Own(Row::SiteCalls), "Camille", "Incoming call");
        assert_eq!(decide(&call, &told(false), Moment { silenced: true, ..HERE }, 5, NOW), Decision::Drop(Dropped::Held));
        let Decision::Post(shape) = decide(&call, &told(false), HERE, 5, NOW) else { panic!() };
        assert_eq!((shape.timeout_ms, shape.close_at, shape.urgency), (0, None, Urgency::Normal), "until settled");
        // An expired code, or no notice of Sioul's own: nothing.
        assert_eq!(decide(&code, &told(true), HERE, 5, NOW + 1_800), Decision::Drop(Dropped::Over));
        let time = Notice::new("sioul:task/a", Kind::Own(Row::Time), "Focus", "");
        assert_eq!(decide(&time, &told(false), HERE, 5, NOW), Decision::Drop(Dropped::NotOurs));
    }

    /// A notice its source decided to tell is told, piercing as its kind
    /// does: what you set or asked for, never a reminder, mail or a site.
    #[test]
    fn told_pierces_as_its_kind() {
        use super::told as decided;
        assert!(decided(Some(Row::Codes)).pierce && decided(Some(Row::Alarms)).pierce && decided(Some(Row::Doses)).pierce);
        assert!(!decided(Some(Row::Before)).pierce && !decided(Some(Row::Move)).pierce && !decided(Some(Row::SiteCalls)).pierce && !decided(None).pierce);
        assert!(decided(None).told);
    }

    /// One notice per kind and thing: a meal's heads-up and its notice at its
    /// time replace each other; another kind about the same thing does not.
    #[test]
    fn a_notice_replaces_the_last_of_its_kind_and_thing() {
        let block = address::need("meal:lunch:2026-11-13");
        let heads_up = Notice::new(&block, Kind::Own(Row::Needs), "No new big task", "Lunch at 12:30");
        let at_time = Notice::new(&block, Kind::Own(Row::Needs), "Lunch", "12:30");
        assert_eq!(heads_up.key(), at_time.key());
        assert_ne!(heads_up.key(), Notice::new(&block, Kind::Own(Row::Dates), "", "").key());
        assert_eq!(block, "sioul:need/meal:lunch:2026-11-13");
        assert_eq!(address::site("proton/mail", "thread #4"), "sioul:site/proton%2Fmail#thread%20%234");
        assert_eq!(address::event("a/b", 1_794_000_000), "sioul:event/a%2Fb#1794000000");
    }

    /// The lifetime as set: five minutes unsaid, bounded between one and
    /// sixty; read from `[notices]`, an older configuration without it reads.
    #[test]
    fn the_lifetime_is_read_and_bounded() {
        assert_eq!(Settings::default().minutes(), 5);
        for (set, read) in [(0, 1), (1, 1), (12, 12), (60, 60), (61, 60), (-3, 1)] {
            assert_eq!(Settings { informative_minutes: Some(set) }.minutes(), read);
        }
        let config: crate::config::Config = toml::from_str("[notices]\ninformative_minutes = 15\n").unwrap();
        assert_eq!(config.notices.minutes(), 15);
        let older: crate::config::Config = toml::from_str("[reminders]\nmail = true\n").unwrap();
        assert_eq!(older.notices.minutes(), 5);
    }
}
