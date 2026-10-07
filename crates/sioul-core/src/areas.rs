// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What things are for, and which time is for what (docs/areas.md).
//!
//! Two axes. Who someone is (a stranger, blocked, or safe, neutral,
//! restricted: `reach::Who`, from the lists and your address books,
//! `porch::Senders`) says who may reach you, and a matrix per channel
//! (`reach::Reach`) when.
//! Areas say what a source (an address, a site, a chat) or a task is for:
//! work, your own admin, or leisure; and the time says what now is for. Five
//! times: work and admin (the hours you set), meals and sleep (from Health),
//! and leisure, every other time. So work does not reach your evenings, and
//! admin waits for its own hours instead of spreading over your free time:
//! recovery needs detachment from work (Sonnentag & Fritz 2007, 2015), a
//! real risk for whoever works from home or for themselves.

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

/// What now is for (docs/areas.md): one of five times, or several hours at
/// once, or no hours set at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Time {
    /// Working hours, and the work you asked for outside them ("A little longer", "Work now").
    Work,
    /// Hours for your own admin.
    Admin,
    /// Every other time: evenings, days without hours, time off, a day closed
    /// early. What you enjoy; work and admin wait.
    Leisure,
    /// A meal, from getting it ready to its end (Health).
    Meals,
    /// The night, from winding down to waking, and naps (Health): nothing disturbs.
    Sleep,
    /// No working or admin hours set at all: everything comes, as before any
    /// were set (meals and sleep still keep their time).
    #[default]
    Any,
    /// Working and admin hours at once: what each of them brings comes, all together.
    Several(Area),
}

impl Time {
    /// The five times, in the order the matrix of who may write to you shows them.
    pub const STATES: [Time; 5] = [Time::Work, Time::Admin, Time::Leisure, Time::Meals, Time::Sleep];

    pub fn parse(text: &str) -> Option<Time> {
        match text.trim().to_lowercase().as_str() {
            "work" | "travail" => Some(Time::Work),
            "admin" | "démarches" | "demarches" => Some(Time::Admin),
            // "rest" and "personal" were the words for time outside every hours set.
            "leisure" | "loisirs" | "free" | "rest" | "personal" | "perso" => Some(Time::Leisure),
            "meals" | "meal" | "repas" => Some(Time::Meals),
            "sleep" | "sommeil" | "night" | "nuit" => Some(Time::Sleep),
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
            Time::Meals => "meals",
            Time::Sleep => "sleep",
            Time::Any => "any",
            // Only work and admin have hours: they are the ones to overlap.
            Time::Several(_) => "work+admin",
        }
    }

    /// The time when the hours of `open` are open: work's, admin's, or both
    /// at once; neither is leisure. Only work and admin have hours.
    pub fn of(open: Area) -> Time {
        match (open.work, open.admin) {
            (false, false) => Time::Leisure,
            (true, false) => Time::Work,
            (false, true) => Time::Admin,
            (true, true) => Time::Several(Area::MIXED),
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

/// Which times your week holds: work and admin when they have hours (until
/// admin has hours of its own, it comes in work time as it always did; until
/// work has some, it comes in admin's), meals and sleep when Health sets them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Week {
    pub work_hours: bool,
    pub admin_hours: bool,
    /// Meals are set (Health).
    pub meals: bool,
    /// A night or naps are set (Health).
    pub sleep: bool,
}

impl Week {
    /// Whether `time` comes in this week: leisure always does.
    pub fn has(self, time: Time) -> bool {
        match time {
            Time::Work => self.work_hours,
            Time::Admin => self.admin_hours,
            Time::Meals => self.meals,
            Time::Sleep => self.sleep,
            Time::Leisure | Time::Any => true,
            Time::Several(open) => (open.work && self.work_hours) || (open.admin && self.admin_hours),
        }
    }
}

/// Whether something for `area` comes forward in `time`: for one of the
/// things the time is for. Admin without hours of its own comes in work
/// time; work without hours comes in admin's. Leisure, meals and sleep bring
/// what is for leisure (a chat with friends), nothing else: a meal is a
/// break, and during sleep nothing notifies anyway, as usual (`notify`);
/// no task then (`quiet::QuietTasks`). Mail has its own rule, by who wrote
/// (`quiet::mail_in_view`).
pub fn in_view(area: Area, time: Time, week: Week) -> bool {
    match time {
        Time::Work => area.work || (!week.admin_hours && area.admin),
        Time::Admin => area.admin || (!week.work_hours && area.work),
        Time::Leisure | Time::Meals | Time::Sleep => area.leisure,
        Time::Any => true,
        // Working and admin hours at once: what each of them brings.
        Time::Several(open) => (open.work && in_view(area, Time::Work, week)) || (open.admin && in_view(area, Time::Admin, week)),
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
        let set = Week { work_hours: true, admin_hours: true, meals: true, sleep: true };
        assert!(in_view(Area::WORK, Time::Work, set));
        assert!(!in_view(Area::WORK, Time::Leisure, set), "never in leisure");
        assert!(!in_view(Area::WORK, Time::Meals, set) && !in_view(Area::WORK, Time::Sleep, set), "nor while you eat or sleep");
        assert!(!in_view(Area::ADMIN, Time::Work, set), "admin has its own hours");
        assert!(in_view(Area::ADMIN, Time::Work, Week::default()), "until it has some");
        assert!(in_view(Area::ADMIN, Time::Admin, set) && !in_view(Area::ADMIN, Time::Leisure, set), "leisure is free");
        assert!(in_view(Area::LEISURE, Time::Sleep, set) && in_view(Area::LEISURE, Time::Meals, set), "a chat with friends");
        assert!(in_view(Area::LEISURE, Time::Leisure, set) && !in_view(Area::LEISURE, Time::Admin, set) && !in_view(Area::LEISURE, Time::Work, set));
        assert!(in_view(Area::PERSONAL, Time::Leisure, set) && in_view(Area::PERSONAL, Time::Admin, set) && !in_view(Area::PERSONAL, Time::Work, set));
        assert!(in_view(Area::MIXED, Time::Work, set) && in_view(Area::MIXED, Time::Admin, set) && !in_view(Area::MIXED, Time::Leisure, set));
        // Work and leisure together, a chat for both: in both their times.
        let both = Area::WORK.with(Area::LEISURE);
        assert!(in_view(both, Time::Work, set) && in_view(both, Time::Leisure, set) && !in_view(both, Time::Admin, set));
        let no_work = Week { admin_hours: true, ..Week::default() };
        assert!(in_view(Area::WORK, Time::Admin, no_work), "no work hours: work comes in admin's");
        assert!(!in_view(Area::WORK, Time::Leisure, no_work) && !in_view(Area::WORK, Time::Sleep, no_work), "leisure and sleep stay free");
        // Working and admin hours at once: both come.
        let both = Time::of(Area::MIXED);
        assert_eq!(both, Time::Several(Area::MIXED));
        assert!(in_view(Area::ADMIN, both, set) && in_view(Area::WORK, both, set) && !in_view(Area::LEISURE, both, set));
        assert_eq!((both.id(), Time::parse("work+admin")), ("work+admin", Some(both)));
        assert!(both.offices() && both.works());
        // Leisure has no hours: a window for it is none.
        assert_eq!(Time::of(Area::LEISURE), Time::Leisure);
        assert_eq!(Time::of(Area::PERSONAL), Time::Admin);
        assert!(in_view(Area::LEISURE, Time::Any, Week::default()) && in_view(Area::WORK, Time::Any, Week::default()));
        // The older words read; each time says its own.
        assert_eq!((Time::parse("rest"), Time::parse("personal"), Time::parse("repas"), Time::parse("Sommeil")), (Some(Time::Leisure), Some(Time::Leisure), Some(Time::Meals), Some(Time::Sleep)));
        assert_eq!(Time::STATES.map(Time::id), ["work", "admin", "leisure", "meals", "sleep"]);
        assert!(!Time::Meals.offices() && !Time::Sleep.offices() && !Time::Leisure.works());
        assert!(set.has(Time::Sleep) && !Week::default().has(Time::Meals) && Week::default().has(Time::Leisure));
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
