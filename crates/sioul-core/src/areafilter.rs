// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! A page's filter by area (docs/areas.md, "A page's filter by area"):
//! Mail's All addresses, and later Budgets. Three switches, Work, Admin and
//! Leisure, ticked as the hours say (`hours_say`, the caller's own rule on
//! top); the person may tick others at once, and that holds until the hours
//! change (the next change of area: the end of the time it was ticked in, or
//! the hours saying otherwise) or until they set it back ("As the hours say").
//! Nothing to watch: the ticks of the person end by themselves.

use crate::areas::{Area, in_view};
use crate::i18n::Translator;
use crate::quiet::Mode;
use serde::{Deserialize, Serialize};

/// What the hours bring now, area by area (`areas::in_view` for each one
/// alone): work in working hours (and admin, while admin has no hours of its
/// own), admin in admin hours, leisure in leisure, meals and sleep; every
/// area when no hours are set.
pub fn hours_say(mode: &Mode) -> Area {
    let one = |area: Area| in_view(area, mode.time, mode.week);
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
}

/// What a page shows of its filter, the hours saying `said` at `now`.
pub fn shown(filter: &AreaFilter, said: Area, now: i64, tr: &Translator) -> Shown {
    let own = filter.own(said, now);
    let areas = own.unwrap_or(said);
    Shown { work: areas.work, admin: areas.admin, leisure: areas.leisure, label: label(tr, areas), own: own.is_some() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::{Time, Week};
    use crate::quiet::Reason;

    fn mode(time: Time, week: Week) -> Mode {
        Mode { quiet: time != Time::Work, time, week, reason: Reason::Working, until: None, back: None, label: String::new() }
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
