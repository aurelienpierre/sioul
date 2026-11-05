// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The window's change signals (docs/client.md, "Every view shows a change
//! at once"): each kind of object says it changed from the one place that
//! writes it (`sioul_core::changes`), and here that word becomes what the
//! views showing that kind need, on the window's thread:
//!
//! - a signal for the views that read the kind themselves: `persons_changed`,
//!   `agendas_changed`, `papers_changed`, `letters_changed`, `money_changed`,
//!   `links_changed`, `sites_changed`, `projects_changed`;
//! - the agenda made again where the kind has dates or names in it and its
//!   writers do not ask (persons' names, papers, letters, money).
//!
//! The views made in Rust (tasks, the agenda, contacts, notes, the Porch,
//! budgets, accounts) are made again by the window's own writers, as they
//! write. What came from elsewhere makes them again here (`arrived`): what
//! the sharing wrote, and what another program wrote (the command line, the
//! agents' tools), which leaves a stamp per kind (`watch`). A burst of writes
//! of one kind makes one signal.

use crate::backend::{QtThread, Shared};
use sioul_core::changes::Kind;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// The kinds whose word is on its way to the window's thread.
static PENDING: AtomicU32 = AtomicU32::new(0);

/// What the window does when a kind changed, here or elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Heard {
    /// Its views that read it themselves are told (its signal).
    pub signal: bool,
    /// The agenda made again: the kind has dates or names in it.
    pub agenda: bool,
    /// What the pages read their links, notes and projects from (`Shared::loaded`) read again.
    pub loaded: bool,
}

/// What a kind's own word does in the window (`said` here).
pub(crate) fn heard(kind: Kind) -> Heard {
    let (signal, agenda, loaded) = match kind {
        Kind::Persons | Kind::Papers | Kind::Letters | Kind::Money => (true, true, false),
        Kind::Agendas | Kind::Sites => (true, false, false),
        Kind::Links | Kind::Projects => (true, false, true),
        // Made in Rust by their writers as they write.
        Kind::Calendars | Kind::Contacts | Kind::Notes | Kind::Accounts => (false, false, false),
    };
    Heard { signal, agenda, loaded }
}

/// The window listens, from its start: each kind said becomes what its views need.
pub(crate) fn listen(qt: &QtThread, shared: &Arc<Shared>) {
    let (qt, shared) = (Mutex::new(qt.clone()), Arc::clone(shared));
    sioul_core::changes::listen(move |kind| {
        let heard = heard(kind);
        if heard.loaded
            && let Ok(mut cache) = shared.loaded.lock()
        {
            *cache = None;
        }
        if !heard.signal && !heard.agenda {
            return;
        }
        if PENDING.fetch_or(kind.bit(), Ordering::AcqRel) & kind.bit() != 0 {
            return;
        }
        let Ok(qt) = qt.lock() else { return };
        let (later, shared) = (qt.clone(), Arc::clone(&shared));
        // On the window's thread, never inside the writer's file lock.
        let _ = qt.queue(move |sioul| {
            PENDING.fetch_and(!kind.bit(), Ordering::AcqRel);
            if heard.signal {
                emit(sioul, kind);
            }
            if heard.agenda {
                crate::agenda::show_agenda(&later, &shared);
            }
        });
    });
}

fn emit(mut sioul: std::pin::Pin<&mut crate::backend::qobject::Sioul>, kind: Kind) {
    match kind {
        Kind::Persons => sioul.as_mut().persons_changed(),
        Kind::Agendas => sioul.as_mut().agendas_changed(),
        Kind::Papers => sioul.as_mut().papers_changed(),
        Kind::Letters => sioul.as_mut().letters_changed(),
        Kind::Money => sioul.as_mut().money_changed(),
        Kind::Links => sioul.as_mut().links_changed(),
        Kind::Sites => sioul.as_mut().sites_changed(),
        Kind::Projects => sioul.as_mut().projects_changed(),
        Kind::Calendars | Kind::Contacts | Kind::Notes | Kind::Accounts => {}
    }
}

/// The kinds a store of the sharing holds (`sioul_sync::share`'s names).
pub(crate) fn kinds_of(store: &str) -> Vec<Kind> {
    match store {
        sioul_sync::share::PERSONS => vec![Kind::Persons],
        // The settings: what each agenda is, the accounts, the sites.
        "config/config.toml" => vec![Kind::Agendas, Kind::Accounts, Kind::Sites],
        "data/links.toml" => vec![Kind::Links],
        "notes/sioul-cases.toml" | "notes/sioul-projects.toml" => vec![Kind::Projects],
        "notes/sioul-budgets.toml" | "notes/sioul-bank.toml" | "notes/sioul-contracts.toml" | sioul_sync::share::MOVEMENTS => vec![Kind::Money],
        "notes/sioul-papers.toml" | "files/papers/" => vec![Kind::Papers],
        // The notes folder, paper letters and notes' own links in it.
        "files/notes/" => vec![Kind::Notes, Kind::Letters, Kind::Links],
        // This device's own calendars and address books, as your other devices keep them.
        "data/calendars/local/" | "state/dav/local/" => vec![Kind::Calendars, Kind::Agendas],
        "data/contacts/local/" => vec![Kind::Contacts],
        _ => Vec::new(),
    }
}

/// What the Rust-made views need when these kinds came from elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Remade {
    /// The agenda and the contacts (`pim::show_pim`).
    pub pim: bool,
    /// The Porch, budgets and accounts (`backend::show`).
    pub views: bool,
    /// The tasks, the notes and the plan (`work::show_work`), from a fresh read.
    pub work: bool,
}

pub(crate) fn remade(kinds: &BTreeSet<Kind>) -> Remade {
    let any = |of: &[Kind]| of.iter().any(|k| kinds.contains(k));
    Remade {
        pim: any(&[Kind::Calendars, Kind::Contacts, Kind::Agendas]),
        views: any(&[Kind::Money, Kind::Accounts, Kind::Projects, Kind::Papers, Kind::Letters, Kind::Sites]),
        work: any(&[Kind::Calendars, Kind::Notes, Kind::Links, Kind::Projects]),
    }
}

/// These kinds came from elsewhere (the sharing, another program): each
/// said, as its own writer would, and the Rust-made views made again.
pub(crate) fn arrived(qt: &QtThread, shared: &Arc<Shared>, kinds: &BTreeSet<Kind>) {
    if kinds.is_empty() {
        return;
    }
    for kind in kinds {
        sioul_core::changes::said(*kind);
    }
    let remade = remade(kinds);
    if remade.work {
        if let Ok(mut cache) = shared.loaded.lock() {
            *cache = None;
        }
        crate::work::show_work(qt, shared);
    }
    if remade.pim {
        crate::pim::show_pim(qt, shared);
    }
    if remade.views {
        crate::backend::show(qt, shared);
    }
}

/// What the sharing wrote here, said by kind, each once.
pub(crate) fn arrived_stores(qt: &QtThread, shared: &Arc<Shared>, written: &BTreeSet<String>) {
    let kinds: BTreeSet<Kind> = written.iter().flat_map(|store| kinds_of(store)).collect();
    arrived(qt, shared, &kinds);
}

/// A kind's stamp: its file's time and size (`changes::stamped_in`).
type Stamps = BTreeMap<Kind, (std::time::SystemTime, u64)>;

/// The kinds whose stamp changed between two looks: written again, or
/// stamped for the first time.
pub(crate) fn stamps_changed(before: &Stamps, now: &Stamps) -> BTreeSet<Kind> {
    now.iter().filter(|(kind, stamp)| before.get(kind) != Some(stamp)).map(|(kind, _)| *kind).collect()
}

/// Every second while the window shows, the stamps another program leaves
/// (the command line, `sioul mcp`) looked at: a kind stamped again arrived.
/// Those of before the window started are not news. A phone has no other
/// program writing Sioul's files.
pub(crate) fn watch(qt: &QtThread, shared: &Arc<Shared>) {
    if cfg!(target_os = "android") {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let dir = sioul_core::changes::stamps_dir();
        let mut before = sioul_core::changes::stamped_in(&dir);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if !crate::share::window_shown() {
                continue;
            }
            let now = sioul_core::changes::stamped_in(&dir);
            let kinds = stamps_changed(&before, &now);
            before = now;
            arrived(&qt, &shared, &kinds);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sharing's stores say their kinds, each once whatever came with
    /// it; another device's settings say the agendas, the accounts and the sites.
    #[test]
    fn what_another_device_brought_is_said_by_kind() {
        let written: BTreeSet<String> = ["data/persons.toml", "config/config.toml", "data/health.toml", "notes/sioul-bank.toml", "files/papers/"].into_iter().map(String::from).collect();
        let kinds: BTreeSet<Kind> = written.iter().flat_map(|s| kinds_of(s)).collect();
        assert_eq!(kinds, [Kind::Persons, Kind::Agendas, Kind::Papers, Kind::Money, Kind::Sites, Kind::Accounts].into_iter().collect());
        assert_eq!(kinds_of("state/today.toml"), []);
        assert!(kinds_of("data/calendars/local/").contains(&Kind::Calendars));
        assert!(kinds_of("files/notes/").contains(&Kind::Letters), "letters live in the notes folder");
    }

    /// Each kind reaches its views: a signal for the views that read it
    /// themselves, the agenda for those with dates or names in it; what came
    /// from elsewhere makes the Rust-made views again.
    #[test]
    fn each_kind_reaches_its_views() {
        for kind in Kind::ALL {
            let h = heard(kind);
            let rust_made = matches!(kind, Kind::Calendars | Kind::Contacts | Kind::Notes | Kind::Accounts);
            assert_eq!(h.signal, !rust_made, "{kind:?}");
            let alone: BTreeSet<Kind> = [kind].into_iter().collect();
            let r = remade(&alone);
            assert!(h.signal || r.pim || r.views || r.work, "{kind:?} reaches no view when it comes from elsewhere");
        }
        assert!(heard(Kind::Papers).agenda && heard(Kind::Money).agenda && heard(Kind::Persons).agenda, "dates and names in the agenda");
        let calendars: BTreeSet<Kind> = [Kind::Calendars].into_iter().collect();
        assert_eq!(remade(&calendars), Remade { pim: true, views: false, work: true });
    }

    /// A stamp written again is a kind arrived; one unchanged is not; the
    /// first stamp of a kind counts.
    #[test]
    fn a_stamp_written_again_is_an_arrival() {
        let at = |s: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(s);
        let before: Stamps = [(Kind::Calendars, (at(1), 13)), (Kind::Notes, (at(1), 13))].into_iter().collect();
        let now: Stamps = [(Kind::Calendars, (at(2), 13)), (Kind::Notes, (at(1), 13)), (Kind::Links, (at(2), 13))].into_iter().collect();
        assert_eq!(stamps_changed(&before, &now), [Kind::Calendars, Kind::Links].into_iter().collect());
        assert!(stamps_changed(&now, &now).is_empty());
    }
}
