// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Search tab of the Tasks and the Agenda views (8 November 2026): the
//! window's query given to the core's one engine (`sioul_core::thingsearch`)
//! over everything read (`work::loaded`), whatever the hour: a full list is a
//! tool. A range of days reads the calendars' occurrences in it, for the
//! repeating events.
//!
//! The page's agenda filter holds there as in its other full views (20
//! November 2026): Tasks ▸ Search leaves out the lists its List, Board and
//! Gantt leave out (`FilterNow::hidden_by_hand`), Agenda ▸ Search the
//! calendars its Day, Week and Month leave out (`hidden_in_agenda_full`),
//! a set chosen by its button included. What they hold and the search would
//! have found is counted (`left_out`), and "Show them all" asks once more
//! with `every_agenda`, for that search alone.

use crate::backend::{Shared, load_config, tr};
use serde::{Deserialize, Serialize};
use sioul_core::thingsearch::{self, Found, Query, Which};

/// What the tab shows: the choices of its filters, and what was found.
#[derive(Serialize)]
struct Shown {
    projects: Vec<Choice>,
    kinds: Vec<Choice>,
    categories: Vec<String>,
    /// Where a batch can move things: the lists and calendars you may write.
    lists: Vec<Place>,
    found: Vec<Found>,
    /// What the agendas left out hold that the search would have found.
    left_out: usize,
    /// The page's agenda filter as its full views show it: the Agenda shows
    /// it while its Search tab is open, whatever tab it read its days for.
    agendas: crate::agenda::FilterShown,
}

/// What the window asks besides the core's query.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Asked {
    /// Every agenda, whatever the filter: "Show them all", for this search.
    every_agenda: bool,
}

#[derive(Serialize)]
struct Place {
    /// "account/id".
    id: String,
    label: String,
    tasks: bool,
    events: bool,
}

#[derive(Serialize)]
struct Choice {
    id: String,
    label: String,
}

/// The things `query` (JSON, `thingsearch::Query`) asks, with the filters' choices, as JSON.
pub(crate) fn search(shared: &Shared, query: &str) -> String {
    let asked: Asked = serde_json::from_str(query).unwrap_or_default();
    let mut query: Query = serde_json::from_str(query).unwrap_or_default();
    // The page's filter, never one the window would send.
    let filter = crate::agenda::filter_now();
    let tasks = query.which == Which::Tasks;
    query.agendas_off = match (asked.every_agenda, tasks) {
        (true, _) => Default::default(),
        (false, true) => filter.hidden_by_hand(),
        (false, false) => filter.hidden_in_agenda_full(),
    };
    let loaded = crate::work::loaded(shared);
    let config = load_config();
    let own = crate::mail::own_addresses(&config);
    let zone = sioul_core::zone::current();
    let today = sioul_core::zone::now().date();
    let occurrences = query.range(&zone).filter(|_| loaded.events.iter().any(|e| e.recurring)).map(|(from, to)| {
        // An open end reads a year past what is asked, at most: a repeating event placed by its first time there.
        let from = if from == i64::MIN { to.saturating_sub(366 * 86_400) } else { from };
        let to = if to == i64::MAX { from.saturating_add(366 * 86_400) } else { to };
        sioul_core::agenda::occurrences(from, to)
    });
    let cx = thingsearch::Context { own: &own, projects: &loaded.projects, zone: &zone, today, tr: tr() };
    let searched = thingsearch::search(&loaded.tasks, &loaded.events, occurrences.as_deref(), &query, &cx);
    let mut categories = crate::work::categories_of(&loaded.tasks);
    for category in loaded.events.iter().flat_map(|e| e.categories.iter()) {
        if !categories.iter().any(|c| c.to_lowercase() == category.to_lowercase()) {
            categories.push(category.clone());
        }
    }
    let shown = Shown {
        projects: loaded.projects.iter().map(|p| Choice { id: p.id.clone(), label: p.title.clone() }).collect(),
        kinds: config.task_kinds(tr()).into_iter().map(|(id, label)| Choice { id, label }).collect(),
        categories,
        lists: places(&config),
        found: searched.found,
        left_out: searched.left_out,
        agendas: filter.shown_as(tasks, true),
    };
    crate::backend::json(&shown)
}

/// The lists and calendars you may write, each once, with what it keeps.
fn places(config: &sioul_core::config::Config) -> Vec<Place> {
    use sioul_core::vdir::{Kind, collections};
    let mut out: Vec<Place> = Vec::new();
    // Never a calendar of busy blocks Sioul keeps for your availability.
    let mirrors = sioul_core::availability::mirror_places();
    for c in sioul_core::tasks::lists().into_iter().chain(collections(Kind::Calendars)).filter(|c| !c.read_only && !mirrors.iter().any(|(a, id)| *a == c.account && *id == c.id)) {
        let id = format!("{}/{}", c.account, c.id);
        if out.iter().any(|p| p.id == id) {
            continue;
        }
        let (tasks, events) = (c.holds("VTODO"), c.holds("VEVENT"));
        if tasks || events {
            out.push(Place { id, label: c.label(config, tr()), tasks, events });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sioul_core::vdir::{self, Kind};

    /// Each page's search leaves out what the agenda filter leaves out of its
    /// full views, counts it, and finds it again with `every_agenda`: Tasks a
    /// task by its list, the Agenda an event by its calendar. The answer
    /// carries the filter as those views show it.
    #[test]
    fn the_search_follows_the_page_s_agenda_filter() {
        let _view = crate::agenda::tests::view_held();
        sioul_core::config::stand_ins_only();
        let list = vdir::create(Kind::Calendars, vdir::LOCAL, "Search's list", None, &["VTODO"]).unwrap();
        let calendar = vdir::create(Kind::Calendars, vdir::LOCAL, "Search's calendar", None, &["VEVENT"]).unwrap();
        std::fs::write(list.dir.join("search-filter-task.ics"), "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:search-filter-task\r\nDTSTAMP:20261120T100000Z\r\nSUMMARY:Quillwort form\r\nEND:VTODO\r\nEND:VCALENDAR\r\n").unwrap();
        std::fs::write(calendar.dir.join("search-filter-event.ics"), "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:search-filter-event\r\nDTSTAMP:20261120T100000Z\r\nDTSTART:20261201T090000Z\r\nDTEND:20261201T100000Z\r\nSUMMARY:Quillwort meeting\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        let shared = Shared::default();
        let answer = |query: &str| -> serde_json::Value { serde_json::from_str(&search(&shared, query)).unwrap() };
        let ask = |query: &str| {
            let shown = answer(query);
            let found: Vec<String> = shown["found"].as_array().unwrap().iter().map(|f| f["uid"].as_str().unwrap_or_default().to_string()).collect();
            (found, shown["left_out"].as_u64().unwrap())
        };
        let one = |uid: &str| vec![uid.to_string()];
        assert_eq!(ask(r#"{"which":"tasks","words":"quillwort"}"#), (one("search-filter-task"), 0), "every list shown: a task alone in Tasks");
        assert_eq!(ask(r#"{"which":"events","words":"quillwort"}"#), (one("search-filter-event"), 0), "an event alone in the Agenda");
        let place = sioul_core::agendas::place;
        assert_eq!(crate::agenda::turn_by_hand(&[place(&list), place(&calendar)], false), "");
        assert_eq!(ask(r#"{"which":"tasks","words":"quillwort"}"#), (Vec::new(), 1), "its list left out: counted, not listed");
        assert_eq!(ask(r#"{"which":"events","words":"quillwort"}"#), (Vec::new(), 1), "its calendar left out: counted, not listed");
        assert_eq!(ask(r#"{"which":"tasks","words":"quillwort","every_agenda":true}"#), (one("search-filter-task"), 0), "Show them all, for this search");
        assert_eq!(ask(r#"{"which":"events","words":"quillwort","every_agenda":true}"#), (one("search-filter-event"), 0));
        let off = |which: &str, id: &str| answer(&format!(r#"{{"which":"{which}"}}"#))["agendas"]["groups"].as_array().unwrap().iter().flat_map(|g| g["agendas"].as_array().unwrap().clone()).any(|a| a["id"] == id && a["on"] == false);
        assert!(off("tasks", &place(&list)) && off("events", &place(&calendar)), "the filter as the full views show it");
        assert_eq!(crate::agenda::as_the_hours_say(), "");
        for c in [list, calendar] {
            let _ = std::fs::remove_dir_all(&c.dir);
        }
    }
}
