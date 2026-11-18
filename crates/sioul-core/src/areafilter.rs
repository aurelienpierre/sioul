// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A page's filter by area (docs/areas.md, "A page's filter by area"):
//! Mail's All addresses, the Budgets page, and the Projects and Time pages,
//! which share their ticks. Three switches, Work, Admin and Leisure, ticked
//! as the hours say (`hours_say`, the caller's own rule on top); the person
//! may tick others at once, and that holds until the hours
//! change (the next change of area: the end of the time it was ticked in, or
//! the hours saying otherwise) or until they set it back ("As the hours say").
//! Nothing to watch: the ticks of the person end by themselves.

use crate::areas::Area;
use crate::i18n::Translator;
use crate::quiet::Mode;
use serde::{Deserialize, Serialize};

/// What the hours bring now on this device, area by area (`Mode::brings`
/// for each one alone): work in working hours (and admin, while admin has no
/// hours of its own), admin in admin hours, leisure in leisure, meals and
/// sleep; every area when no hours are set. On a device the person said
/// something of, only what comes forward on it: its cell for now
/// (docs/areas.md, "What each device is for"); `Mode::elsewhere` says what
/// it keeps back, for the filter's "?".
pub fn hours_say(mode: &Mode) -> Area {
    let one = |area: Area| mode.brings(area);
    Area { work: one(Area::WORK), admin: one(Area::ADMIN), leisure: one(Area::LEISURE) }
}

/// The longest the person's ticks hold: a day.
pub const LONGEST: i64 = 86_400;

/// The ticks of the person, while they hold: a file's table, written with the
/// page's other choices (Mail's `mailbox-view.toml`). Empty: as the hours say.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreaFilter {
    /// The areas ticked ("work+leisure"; "" for none), when the person ticked them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticked: Option<String>,
    /// What the hours said then: the ticks hold while the hours still say so.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub under: String,
    /// When the time they were ticked in ends (Unix seconds): they hold until then at the latest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
}

impl AreaFilter {
    /// The areas the person ticked, while they hold under what the hours say
    /// now (`said`), at `now` (Unix seconds); none: as the hours say.
    pub fn own(&self, said: Area, now: i64) -> Option<Area> {
        let ticked = self.ticked.as_deref()?;
        let held = text(&self.under) == said && self.until.is_none_or(|until| now < until);
        held.then(|| text(ticked))
    }

    /// What shows now: the ticks of the person while they hold, else what the hours say.
    pub fn shown(&self, said: Area, now: i64) -> Area {
        self.own(said, now).unwrap_or(said)
    }

    /// The person ticked `areas` at `now`, the hours saying `said` until
    /// `until`: kept until the hours change, and a day at most (`LONGEST`), so
    /// that a time with no end known (no hours set, a pause) or a long one (a
    /// weekend of leisure) never keeps them for good. Ticking what the hours
    /// say is as the hours say.
    pub fn tick(&mut self, areas: Area, said: Area, until: Option<i64>, now: i64) {
        let until = until.map_or(now + LONGEST, |until| until.min(now + LONGEST));
        *self = if areas == said { AreaFilter::default() } else { AreaFilter { ticked: Some(areas.id()), under: said.id(), until: Some(until) } };
    }

    /// "As the hours say": the ticks of the person forgotten.
    pub fn as_hours(&mut self) {
        *self = AreaFilter::default();
    }

    /// The ticks forgotten once they no longer hold, so that a later time
    /// saying the same never brings them back; whether that changed anything.
    pub fn settle(&mut self, said: Area, now: i64) -> bool {
        if self.ticked.is_some() && self.own(said, now).is_none() {
            self.as_hours();
            return true;
        }
        false
    }
}

/// An area as written in the filter's file: "work+admin"; "" for none.
fn text(written: &str) -> Area {
    Area::parse(written).unwrap_or_default()
}

/// What the filter's button says: the areas shown, in their order ("Admin ·
/// Leisure"), or that none is ticked.
pub fn label(tr: &Translator, shown: Area) -> String {
    let words: Vec<String> = [(shown.work, "area-filter-work"), (shown.admin, "area-filter-admin"), (shown.leisure, "area-filter-leisure")].iter().filter(|(on, _)| *on).map(|(_, id)| tr.text(id, None)).collect();
    if words.is_empty() { tr.text("area-filter-none", None) } else { words.join(" · ") }
}

/// The filter as a page shows it: each area ticked or not, the button's
/// words, and whether the person's ticks hold ("As the hours say" offered).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Shown {
    pub work: bool,
    pub admin: bool,
    pub leisure: bool,
    pub label: String,
    /// The person's ticks hold: "As the hours say" brings the hours' back.
    pub own: bool,
    /// Why some areas are not ticked though the hours bring them, when this
    /// device is the reason (`device_line`): said in the filter's "?". ""
    /// when the hours alone decide.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
}

impl Shown {
    /// The same, saying in its "?" what this device keeps back at `mode` (`device_line`).
    pub fn on(mut self, tr: &Translator, mode: &Mode) -> Shown {
        self.device = device_line(tr, mode).unwrap_or_default();
        self
    }
}

/// What a page shows of its filter, the hours saying `said` at `now`.
pub fn shown(filter: &AreaFilter, said: Area, now: i64, tr: &Translator) -> Shown {
    let own = filter.own(said, now);
    let areas = own.unwrap_or(said);
    Shown { work: areas.work, admin: areas.admin, leisure: areas.leisure, label: label(tr, areas), own: own.is_some(), device: String::new() }
}

/// Areas inside a sentence: "work", "your admin and leisure"; "everything"
/// for the three. `id`: the message that words them (`device-areas`, or
/// `device-areas-wait` with its verb: "leisure waits").
pub fn areas_in_words(tr: &Translator, id: &str, areas: Area) -> String {
    let mut args = crate::i18n::args();
    // A variant's name holds no "+": "work-leisure".
    args.set("areas", if areas == Area::ALL { "all".to_string() } else if areas.is_empty() { "none".to_string() } else { areas.id().replace('+', "-") });
    tr.text(id, Some(&args))
}

/// Why this device keeps back now what the hours bring, when it does, for
/// the status line and a filter's "?" (docs/areas.md, "What each device is
/// for"): "This device is for work: leisure waits on your other devices.",
/// or, where the person changed the cell for this time (or said no area),
/// "On this device, at this time, leisure waits on your other devices: you
/// set it so in Settings ▸ Devices." None when the hours alone decide, and
/// while you sleep or pause, when nothing comes forward anyway.
pub fn device_line(tr: &Translator, mode: &Mode) -> Option<String> {
    let waits = mode.elsewhere();
    if waits.is_empty() || mode.sleeps() {
        return None;
    }
    let line = mode.here.line;
    let changed = match mode.time {
        crate::areas::Time::Several(open) => (open.work && line.changed(crate::areas::Time::Work).is_some()) || (open.admin && line.changed(crate::areas::Time::Admin).is_some()),
        one => line.changed(one).is_some(),
    };
    let mut args = crate::i18n::args();
    args.set("waits", areas_in_words(tr, "device-areas-wait", waits));
    Some(match line.area {
        Some(area) if !changed => {
            args.set("for", areas_in_words(tr, "device-areas", area));
            tr.text("device-waits", Some(&args))
        }
        _ => tr.text("device-waits-cell", Some(&args)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{Time, Week};
    use crate::quiet::Reason;

    fn mode(time: Time, week: Week) -> Mode {
        Mode { quiet: time != Time::Work, time, week, reason: Reason::Working, until: None, back: None, label: String::new(), here: crate::areas::Here::default() }
    }

    /// The hours' ticks, time by time: work alone, and admin with it while
    /// admin has no hours of its own; admin alone, and work while work has
    /// none; leisure in leisure, at meals and while you sleep; everything
    /// when no hours are set; both at once when both hours are open.
    #[test]
    fn the_hours_tick_what_now_is_for() {
        let both = Week { work_hours: true, admin_hours: true, ..Week::default() };
        let work_only = Week { work_hours: true, ..Week::default() };
        let admin_only = Week { admin_hours: true, ..Week::default() };
        assert_eq!(hours_say(&mode(Time::Work, both)), Area::WORK);
        assert_eq!(hours_say(&mode(Time::Work, work_only)), Area::MIXED, "admin without hours of its own comes at work");
        assert_eq!(hours_say(&mode(Time::Admin, both)), Area::ADMIN);
        assert_eq!(hours_say(&mode(Time::Admin, admin_only)), Area::MIXED, "work without hours comes in admin's");
        for time in [Time::Leisure, Time::Meals, Time::Sleep] {
            assert_eq!(hours_say(&mode(time, both)), Area::LEISURE, "{time:?}");
        }
        assert_eq!(hours_say(&mode(Time::Any, Week::default())), Area::ALL);
        assert_eq!(hours_say(&mode(Time::Several(Area::MIXED), both)), Area::MIXED);
    }

    /// On a device the person said something of, the ticks are what comes
    /// forward on it: a computer for work ticks nothing in the evening, and
    /// a phone for admin and leisure nothing in working hours, unless a
    /// cell the person changed says otherwise; an area no device is for is
    /// ticked as the hours say.
    #[test]
    fn the_ticks_follow_what_this_device_is_for() {
        use crate::areas::{Line, Lines};
        let both = Week { work_hours: true, admin_hours: true, ..Week::default() };
        let (computer, phone) = (Line::for_area(Area::WORK), Line::for_area(Area::PERSONAL));
        let on = |mine: Line, other: Line, time: Time| mode(time, both).on(&Lines::of(mine, &[other]));
        assert_eq!(Time::STATES.map(|t| hours_say(&on(computer, phone, t)).id()), ["work", "", "", "", ""]);
        assert_eq!(Time::STATES.map(|t| hours_say(&on(phone, computer, t)).id()), ["", "admin", "leisure", "leisure", "leisure"]);
        assert_eq!(on(computer, phone, Time::Leisure).elsewhere(), Area::LEISURE, "what the filter's ? says waits elsewhere");
        let friends = phone.with_cell(Time::Work, Some(Area::WORK.with(Area::LEISURE)));
        assert_eq!(hours_say(&on(friends, computer, Time::Work)), Area::WORK.with(Area::LEISURE));
        // The phone gone: leisure and admin are for no device, and tick on the computer as the hours say.
        let alone = |time: Time| mode(time, both).on(&Lines::of(computer, &[]));
        assert_eq!(Time::STATES.map(|t| hours_say(&alone(t))), Time::STATES.map(|t| hours_say(&mode(t, both))));
        assert_eq!(hours_say(&mode(Time::Any, Week::default()).on(&Lines::of(computer, &[phone]))), Area::WORK, "no hours set: what the device is for");
    }

    /// The person's ticks hold at once, until the time they were ticked in
    /// ends, or the hours say otherwise before (a day closed early); then the
    /// hours' again, for good: a later time that says the same does not bring
    /// them back once settled. Ticking what the hours say, or "As the hours
    /// say", is the hours' own.
    #[test]
    fn your_ticks_hold_until_the_hours_change() {
        let (work, leisure) = (Area::WORK, Area::LEISURE);
        let end_of_work = 1_800_000_000;
        let mut filter = AreaFilter::default();
        assert_eq!((filter.shown(work, end_of_work - 3600), filter.own(work, 0)), (work, None), "as the hours say");
        // At work, leisure ticked too: at once, until work ends.
        filter.tick(Area { leisure: true, ..work }, work, Some(end_of_work), end_of_work - 7200);
        assert_eq!(filter.shown(work, end_of_work - 3600), Area { leisure: true, ..work });
        assert_eq!(filter.shown(work, end_of_work - 1), Area { leisure: true, ..work });
        // Work ends: the hours' again.
        assert_eq!(filter.shown(leisure, end_of_work), leisure);
        assert_eq!(filter.shown(work, end_of_work), work, "its time over, whatever the hours say then");
        // The hours change before its end (Done for today): the hours' again.
        assert_eq!(filter.shown(leisure, end_of_work - 1800), leisure);
        // Settled once over, it never comes back: tomorrow's work is the hours' own.
        assert!(filter.settle(leisure, end_of_work + 60));
        assert_eq!((filter.clone(), filter.shown(work, end_of_work + 86_400 - 3600)), (AreaFilter::default(), work));
        assert!(!filter.settle(work, end_of_work + 86_400), "nothing to forget");
        // Nothing ticked is the person's word too, said "Nothing ticked". No end
        // known (no hours set, a pause), or a long one (a weekend): a day at most.
        let friday = end_of_work + 7200;
        filter.tick(Area::default(), leisure, None, friday);
        assert_eq!(filter.shown(leisure, friday + LONGEST - 1), Area::default());
        assert_eq!(filter.shown(leisure, friday + LONGEST), leisure, "a day at most");
        assert_eq!(filter.shown(work, friday + 60), work, "the hours changed");
        filter.tick(work, leisure, Some(friday + 3 * LONGEST), friday);
        assert_eq!(filter.until, Some(friday + LONGEST), "a weekend of leisure: a day at most");
        // Ticking what the hours say, or setting it back: the hours' own.
        filter.tick(leisure, leisure, None, friday);
        assert_eq!(filter, AreaFilter::default());
        filter.tick(work, leisure, Some(end_of_work), end_of_work - 60);
        filter.as_hours();
        assert_eq!(filter.own(leisure, 0), None);
        // Written as the page's file keeps it, and read back.
        filter.tick(Area::PERSONAL, work, Some(end_of_work), end_of_work - 3600);
        let written = toml::to_string(&filter).unwrap();
        assert_eq!(written, "ticked = \"admin+leisure\"\nunder = \"work\"\nuntil = 1800000000\n");
        assert_eq!(toml::from_str::<AreaFilter>(&written).unwrap(), filter);
        assert_eq!(toml::from_str::<AreaFilter>("").unwrap(), AreaFilter::default());
    }

    /// Where this device is the reason, one sentence says so, for the status
    /// line and a filter's "?": what the device is for and what waits on the
    /// others; a cell the person changed is said as the time's; none when
    /// the hour is the reason, with nothing said of the device, or asleep.
    #[test]
    fn a_sentence_says_when_the_device_is_the_reason() {
        use crate::areas::{Line, Lines};
        let (en, fr) = (Translator::new("en"), Translator::new("fr"));
        let both = Week { work_hours: true, admin_hours: true, ..Week::default() };
        let (computer, phone) = (Line::for_area(Area::WORK), Line::for_area(Area::PERSONAL));
        let on = |mine: Line, other: Line, time: Time, week: Week| mode(time, week).on(&Lines::of(mine, &[other]));
        let evening = on(computer, phone, Time::Leisure, both);
        assert_eq!(device_line(&en, &evening).as_deref(), Some("This device is for work: leisure waits on your other devices."));
        assert_eq!(device_line(&fr, &evening).as_deref(), Some("Cet appareil est pour le travail\u{202f}: vos loisirs attendent sur vos autres appareils."));
        let at_work = on(phone, computer, Time::Work, both);
        assert_eq!(device_line(&en, &at_work).as_deref(), Some("This device is for your admin and leisure: work waits on your other devices."));
        // Admin without hours of its own comes at work: on the computer for work, both said.
        let lent = Week { work_hours: true, ..Week::default() };
        assert_eq!(device_line(&en, &on(phone, computer, Time::Work, lent)).as_deref(), Some("This device is for your admin and leisure: work waits on your other devices."));
        assert_eq!(device_line(&en, &on(computer, phone, Time::Work, lent)).as_deref(), Some("This device is for work: your admin waits on your other devices."));
        // A cell the person changed: said as the time's, not as what the device is for.
        let silent = on(phone.with_cell(Time::Leisure, Some(Area::default())), computer.with_cell(Time::Leisure, Some(Area::LEISURE)), Time::Leisure, both);
        assert_eq!(device_line(&en, &silent).as_deref(), Some("On this device, at this time, leisure waits on your other devices: you set it so in Settings ▸ Devices."));
        assert_eq!(device_line(&fr, &silent).as_deref(), Some("Sur cet appareil, à cette heure, vos loisirs attendent sur vos autres appareils\u{202f}: vous l’avez réglé ainsi dans Paramètres ▸ Appareils."));
        // The hour is the reason, nothing is said of this device, the area is for no device, or asleep: no sentence.
        assert_eq!(device_line(&en, &on(computer, phone, Time::Work, both)), None);
        assert_eq!(device_line(&en, &on(Line::default(), computer, Time::Leisure, both)), None);
        assert_eq!(device_line(&en, &mode(Time::Leisure, both).on(&Lines::of(computer, &[]))), None);
        assert_eq!(device_line(&en, &on(computer, phone, Time::Sleep, both)), None);
        // In a filter's "?": said with what it shows.
        let shown = shown(&AreaFilter::default(), hours_say(&evening), 0, &en).on(&en, &evening);
        assert_eq!((shown.label.as_str(), shown.device.as_str()), ("Nothing ticked", "This device is for work: leisure waits on your other devices."));
        assert_eq!(super::shown(&AreaFilter::default(), Area::WORK, 0, &en).device, "");
        assert_eq!((areas_in_words(&en, "device-areas", Area::ALL), areas_in_words(&en, "device-areas", Area::default()), areas_in_words(&fr, "device-areas", Area::MIXED)), ("everything".to_string(), "nothing".to_string(), "le travail et vos démarches".to_string()));
    }

    /// The button's words: the areas shown, in their order, or that none is ticked; in both languages.
    #[test]
    fn the_button_says_what_shows() {
        let en = Translator::new("en");
        assert_eq!(label(&en, Area::PERSONAL), "Admin · Leisure");
        assert_eq!(label(&en, Area::ALL), "Work · Admin · Leisure");
        assert_eq!(label(&en, Area::default()), "Nothing ticked");
        assert_eq!(label(&Translator::new("fr"), Area::MIXED), "Travail · Démarches");
        let ticked = AreaFilter { ticked: Some("work".into()), under: "leisure".into(), until: None };
        let said = shown(&ticked, Area::LEISURE, 0, &en);
        assert_eq!((said.work, said.admin, said.leisure, said.own, said.label.as_str()), (true, false, false, true, "Work"));
        assert!(!shown(&ticked, Area::WORK, 0, &en).own, "the hours changed: theirs");
    }
}
