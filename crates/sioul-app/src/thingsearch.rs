// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The Search tab of the Tasks and the Agenda views (8 November 2026): the
//! window's query given to the core's one engine (`sioul_core::thingsearch`)
//! over everything read (`work::loaded`), whatever the hour and whatever the
//! agenda filter: a full list is a tool. A range of days reads the
//! calendars' occurrences in it, for the repeating events.

use crate::backend::{Shared, load_config, tr};
use serde::Serialize;
use sioul_core::thingsearch::{self, Found, Query};

/// What the tab shows: the choices of its filters, and what was found.
#[derive(Serialize)]
struct Shown {
    projects: Vec<Choice>,
    kinds: Vec<Choice>,
    categories: Vec<String>,
    /// Where a batch can move things: the lists and calendars you may write.
    lists: Vec<Place>,
    found: Vec<Found>,
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
    let query: Query = serde_json::from_str(query).unwrap_or_default();
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
    let found = thingsearch::search(&loaded.tasks, &loaded.events, occurrences.as_deref(), &query, &cx);
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
        found,
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
