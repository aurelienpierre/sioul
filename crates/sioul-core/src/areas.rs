// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What things are for, and which time is for what (docs/areas.md).
//!
//! Two axes. The senders' lists (safe, neutral, blocked: `porch.rs`) say who
//! may reach you. Areas say what a source (an address, a site, a chat) or a
//! task is for: work, your own admin, or leisure; and your week says which
//! hours are for which. Hours set for none of them are personal time: no work,
//! and no assumption on admin or leisure. Time off and a day closed early are
//! free time. So work does not reach your evenings, and admin waits for its
//! own hours instead of spreading over your rest: recovery needs detachment
//! from work (Sonnentag & Fritz 2007, 2015), a real risk for whoever works
//! from home or for themselves.

use serde::{Deserialize, Serialize};

/// What a source or a task is for: work, your own admin, leisure; any of
/// them together. An address for everything personal is for admin and
/// leisure; the bank account a freelancer uses for both, work and admin.
/// None at all: unsaid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Area {
    pub work: bool,
    /// Your own admin: bills, letters, offices, health errands.
    pub admin: bool,
    /// What gives back: friends, family, chats, what you enjoy.
    pub leisure: bool,
}

impl Area {
    pub const WORK: Area = Area { work: true, admin: false, leisure: false };
    pub const ADMIN: Area = Area { work: false, admin: true, leisure: false };
    pub const LEISURE: Area = Area { work: false, admin: false, leisure: true };
    /// Yours, admin or leisure.
    pub const PERSONAL: Area = Area { work: false, admin: true, leisure: true };
    /// Work and your own admin.
    pub const MIXED: Area = Area { work: true, admin: true, leisure: false };
    /// Every kind: hours open to anything (no hours set).
    pub const ALL: Area = Area { work: true, admin: true, leisure: true };

    /// "work", "admin", "leisure", joined by "+" or ","; "personal" is admin
    /// and leisure, "mixed" work and admin; French words too. None when it says none.
    pub fn parse(text: &str) -> Option<Area> {
        let mut area = Area::default();
        for word in text.split(['+', ',', ' ', '/']).map(str::trim).filter(|w| !w.is_empty()) {
            match word.to_lowercase().as_str() {
                "work" | "travail" | "pro" => area.work = true,
                "admin" | "démarches" | "demarches" => area.admin = true,
                "leisure" | "loisirs" | "fun" | "rest" => area.leisure = true,
                "personal" | "perso" | "personnel" => (area.admin, area.leisure) = (true, true),
                "mixed" => (area.work, area.admin) = (true, true),
                _ => {}
            }
        }
        (!area.is_empty()).then_some(area)
    }

    /// As written in files: "work", "admin+leisure"; "" for none.
    pub fn id(self) -> String {
        [(self.work, "work"), (self.admin, "admin"), (self.leisure, "leisure")].iter().filter(|(on, _)| *on).map(|(_, w)| *w).collect::<Vec<_>>().join("+")
    }

    pub fn is_empty(self) -> bool {
        !(self.work || self.admin || self.leisure)
    }

    /// Both together.
    pub fn with(self, other: Area) -> Area {
        Area { work: self.work || other.work, admin: self.admin || other.admin, leisure: self.leisure || other.leisure }
    }

    /// Whether the two share a kind.
    pub fn meets(self, other: Area) -> bool {
        (self.work && other.work) || (self.admin && other.admin) || (self.leisure && other.leisure)
    }

    /// How many kinds it holds, none to three.
    pub fn count(self) -> u32 {
        u32::from(self.work) + u32::from(self.admin) + u32::from(self.leisure)
    }
}

impl Serialize for Area {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.id())
    }
}

impl<'de> serde::Deserialize<'de> for Area {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Area, D::Error> {
        let text = String::deserialize(deserializer)?;
        Area::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("{text}: work, admin, leisure, joined by +")))
    }
}

/// What the hours now are for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Time {
    Work,
    Admin,
    /// Free time: rest, leisure; time off and a day closed early too.
    Leisure,
    /// Hours set for nothing: yours, admin or leisure as you like; never work.
    #[default]
    Personal,
    /// No hours set at all: everything comes, as before any were set.
    Any,
    /// Hours of several kinds at once (admin hours within free time): what
    /// each of them brings comes, all together.
    Several(Area),
}

impl Time {
    pub fn parse(text: &str) -> Option<Time> {
        match text.trim().to_lowercase().as_str() {
            "work" | "travail" => Some(Time::Work),
            "admin" | "démarches" | "demarches" => Some(Time::Admin),
            "leisure" | "loisirs" | "free" | "rest" => Some(Time::Leisure),
            "personal" | "perso" => Some(Time::Personal),
            "any" => Some(Time::Any),
            several if several.contains('+') => Area::parse(several).map(Time::of),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Time::Work => "work",
            Time::Admin => "admin",
            Time::Leisure => "leisure",
            Time::Personal => "personal",
            Time::Any => "any",
            Time::Several(open) => match (open.work, open.admin, open.leisure) {
                (true, true, true) => "work+admin+leisure",
                (true, true, false) => "work+admin",
                (true, false, true) => "work+leisure",
                (false, true, true) => "admin+leisure",
                (true, false, false) => "work",
                (false, true, false) => "admin",
                (false, false, true) => "leisure",
                (false, false, false) => "personal",
            },
        }
    }
}

impl Time {
    /// The time when the hours of `open` are open: one kind, or several at once.
    pub fn of(open: Area) -> Time {
        match (open.work, open.admin, open.leisure) {
            (false, false, false) => Time::Personal,
            (true, false, false) => Time::Work,
            (false, true, false) => Time::Admin,
            (false, false, true) => Time::Leisure,
            _ => Time::Several(open),
        }
    }

    /// Whether work's hours are among those open now.
    pub fn works(self) -> bool {
        matches!(self, Time::Work) || matches!(self, Time::Several(open) if open.work)
    }

    /// Whether offices keep hours now: work's or admin's.
    pub fn offices(self) -> bool {
        matches!(self, Time::Work | Time::Admin | Time::Any) || matches!(self, Time::Several(open) if open.work || open.admin)
    }
}

/// Which kinds of hours your week sets: until admin has hours of its own, it
/// comes in work time as it always did; until work has some, it is never set aside.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Week {
    pub work_hours: bool,
    pub admin_hours: bool,
    pub leisure_hours: bool,
}

/// Whether something for `area` comes forward in `time`: for one of the
/// things the hours are for. Admin without hours of its own comes in work
/// time; work without hours is never set aside, except in free time.
pub fn in_view(area: Area, time: Time, week: Week) -> bool {
    match time {
        Time::Work => area.work || (!week.admin_hours && area.admin),
        Time::Admin => area.admin || (!week.work_hours && area.work),
        // Free time is kept free, work hours or not.
        Time::Leisure => area.leisure,
        Time::Personal => area.admin || area.leisure || (!week.work_hours && area.work),
        Time::Any => true,
        // Admin hours within free time: admin's and leisure's both.
        Time::Several(open) => (open.work && in_view(area, Time::Work, week)) || (open.admin && in_view(area, Time::Admin, week)) || (open.leisure && in_view(area, Time::Leisure, week)),
    }
}

/// How tasks without an area of their own get one: by their categories and
/// projects. Work first; then what is yours to enjoy; then what is yours either
/// way (health: an errand, but never set aside on holidays); the rest is admin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskAreas {
    pub work_categories: Vec<String>,
    pub leisure_categories: Vec<String>,
    pub personal_categories: Vec<String>,
    /// Projects for a client, and those marked work.
    pub work_cases: Vec<String>,
    /// Projects marked personal (family, friends, leisure, health).
    pub personal_cases: Vec<String>,
}

impl TaskAreas {
    /// The usual words, in English and French.
    pub fn usual() -> TaskAreas {
        let words = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        TaskAreas {
            work_categories: words(&["work", "travail", "pro", "client", "boulot"]),
            leisure_categories: words(&["joy", "family", "friends", "leisure", "fun", "famille", "amis", "loisirs"]),
            personal_categories: words(&["personal", "perso", "health", "santé"]),
            work_cases: Vec::new(),
            personal_cases: Vec::new(),
        }
    }

    /// As your settings say: the categories of work and those that are yours
    /// (`[quiet]`: leisure among them goes to leisure), and your projects' areas.
    pub fn of_config(config: &crate::config::Config, cases: &[crate::cases::Case]) -> TaskAreas {
        let usual = TaskAreas::usual();
        let fold = |s: &str| crate::text::fold(s.trim()).into_iter().collect::<String>();
        let leisure: Vec<String> = usual.leisure_categories.iter().map(|c| fold(c)).collect();
        let (leisure_categories, personal_categories): (Vec<String>, Vec<String>) = config.quiet.personal_categories().into_iter().partition(|c| leisure.contains(&fold(c)));
        TaskAreas {
            work_categories: config.quiet.work_categories(),
            leisure_categories,
            personal_categories,
            work_cases: cases.iter().filter(|c| c.client.is_some() || c.area.as_deref() == Some("work")).map(|c| c.id.clone()).collect(),
            personal_cases: cases.iter().filter(|c| c.area.as_deref() == Some("personal")).map(|c| c.id.clone()).collect(),
        }
    }

    /// A task's area: its own (`X-SIOUL-AREA`), else what its categories,
    /// projects and list say, together; else your own admin.
    pub fn of(&self, task: &crate::tasks::Task) -> Area {
        Area::parse(&task.area).unwrap_or_else(|| self.by_tags(task))
    }

    /// What a task's categories, projects and list say, its own area aside.
    pub fn by_tags(&self, task: &crate::tasks::Task) -> Area {
        let fold = |s: &str| crate::text::fold(s.trim()).into_iter().collect::<String>();
        let has = |list: &[String]| {
            let list: Vec<String> = list.iter().map(|c| fold(c)).collect();
            task.categories.iter().any(|c| list.contains(&fold(c)))
        };
        let mut area = Area::default();
        if has(&self.work_categories) || task.cases.iter().any(|c| self.work_cases.contains(c)) || task.list_id.starts_with("local/github") {
            area = area.with(Area::WORK);
        }
        if has(&self.leisure_categories) {
            area = area.with(Area::LEISURE);
        }
        if has(&self.personal_categories) || task.cases.iter().any(|c| self.personal_cases.contains(c)) {
            area = area.with(Area::PERSONAL);
        }
        if area.is_empty() { Area::ADMIN } else { area }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_comes_when() {
        let set = Week { work_hours: true, admin_hours: true, leisure_hours: true };
        assert!(in_view(Area::WORK, Time::Work, set));
        assert!(!in_view(Area::WORK, Time::Personal, set), "never in personal time");
        assert!(!in_view(Area::WORK, Time::Leisure, set));
        assert!(!in_view(Area::ADMIN, Time::Work, set), "admin has its own hours");
        assert!(in_view(Area::ADMIN, Time::Work, Week::default()), "until it has some");
        assert!(in_view(Area::ADMIN, Time::Admin, set) && in_view(Area::ADMIN, Time::Personal, set));
        assert!(!in_view(Area::ADMIN, Time::Leisure, set), "free time is free");
        assert!(in_view(Area::LEISURE, Time::Leisure, set) && !in_view(Area::LEISURE, Time::Admin, set) && !in_view(Area::LEISURE, Time::Work, set));
        assert!(in_view(Area::PERSONAL, Time::Leisure, set) && in_view(Area::PERSONAL, Time::Admin, set) && !in_view(Area::PERSONAL, Time::Work, set));
        assert!(in_view(Area::MIXED, Time::Work, set) && in_view(Area::MIXED, Time::Admin, set) && !in_view(Area::MIXED, Time::Leisure, set));
        // Work and leisure together, a chat for both: in both their hours.
        let both = Area::WORK.with(Area::LEISURE);
        assert!(in_view(both, Time::Work, set) && in_view(both, Time::Leisure, set) && !in_view(both, Time::Admin, set));
        let no_work = Week { admin_hours: true, ..Week::default() };
        assert!(in_view(Area::WORK, Time::Personal, no_work), "no work hours: work is never set aside");
        assert!(!in_view(Area::WORK, Time::Leisure, no_work), "but free time stays free");
        // Admin hours within free time: both come, never work.
        let both = Time::of(Area::parse("admin+leisure").unwrap());
        assert_eq!(both, Time::Several(Area::PERSONAL));
        assert!(in_view(Area::ADMIN, both, set) && in_view(Area::LEISURE, both, set) && !in_view(Area::WORK, both, set));
        assert_eq!((both.id(), Time::parse("admin+leisure")), ("admin+leisure", Some(both)));
        assert!(both.offices() && !both.works());
        assert!(in_view(Area::LEISURE, Time::Any, Week::default()) && in_view(Area::WORK, Time::Any, Week::default()));
    }

    #[test]
    fn areas_read() {
        assert_eq!(Area::parse(" Démarches "), Some(Area::ADMIN));
        assert_eq!(Area::parse("work"), Some(Area::WORK));
        assert_eq!(Area::parse("personal"), Some(Area::PERSONAL), "the older word");
        assert_eq!(Area::parse("admin+leisure"), Some(Area::PERSONAL));
        assert_eq!(Area::parse("LEISURE,WORK"), Some(Area::WORK.with(Area::LEISURE)));
        assert_eq!(Area::parse(""), None);
        assert_eq!(Area::MIXED.id(), "work+admin");
        let areas = TaskAreas::usual();
        let task = |categories: &[&str], area: &str| crate::tasks::Task { categories: categories.iter().map(|c| c.to_string()).collect(), area: area.into(), ..crate::tasks::Task::default() };
        assert_eq!(areas.of(&task(&["Famille"], "")), Area::LEISURE);
        assert_eq!(areas.of(&task(&["client"], "")), Area::WORK);
        assert_eq!(areas.of(&task(&["santé"], "")), Area::PERSONAL);
        assert_eq!(areas.of(&task(&["admin"], "")), Area::ADMIN);
        assert_eq!(areas.of(&task(&[], "")), Area::ADMIN, "the rest is admin");
        assert_eq!(areas.of(&task(&["client", "famille"], "")), Area::WORK.with(Area::LEISURE), "together");
        assert_eq!(areas.of(&task(&["client"], "leisure")), Area::LEISURE, "its own first");
    }
}
