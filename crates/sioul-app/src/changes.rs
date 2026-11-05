// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The window's change signals (docs/client.md, "Every view shows a change
//! at once"): each kind of object says it changed from the one place that
//! writes it (`sioul_core::changes`), and here that word becomes the kind's
//! signal on the window's thread (`persons_changed`, `agendas_changed`),
//! which every view showing that kind hears and reads again. A burst of
//! writes makes one signal: a kind already on its way is not sent again.
//! What another device brought is said by the sharing, by the files it wrote
//! (`arrived`).

use crate::backend::QtThread;
use sioul_core::changes::Kind;
use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

/// The kinds whose signal is on its way to the window's thread.
static PENDING: AtomicU32 = AtomicU32::new(0);

/// The window listens, from its start: each kind said becomes its signal.
pub(crate) fn listen(qt: &QtThread) {
    let qt = Mutex::new(qt.clone());
    sioul_core::changes::listen(move |kind| {
        if PENDING.fetch_or(kind.bit(), Ordering::AcqRel) & kind.bit() != 0 {
            return;
        }
        let Ok(qt) = qt.lock() else { return };
        let _ = qt.queue(move |sioul| {
            PENDING.fetch_and(!kind.bit(), Ordering::AcqRel);
            emit(sioul, kind);
        });
    });
}

fn emit(mut sioul: std::pin::Pin<&mut crate::backend::qobject::Sioul>, kind: Kind) {
    match kind {
        Kind::Persons => sioul.persons_changed(),
        Kind::Agendas => sioul.agendas_changed(),
        // The budgets page and the Porch's money read again with the views;
        // a project's invoices and the reminders hear the signal.
        Kind::Movements => {
            sioul.as_mut().movements_changed();
            sioul.refresh();
        }
    }
}

/// The kinds a store of the sharing holds (`sioul_sync::share`'s names).
pub(crate) fn kinds_of(store: &str) -> Vec<Kind> {
    match store {
        sioul_sync::share::PERSONS => vec![Kind::Persons],
        // The money: the budget file, the bank's lines, the movements' own store.
        sioul_sync::share::MOVEMENTS | "notes/sioul-budgets.toml" | "notes/sioul-bank.toml" => vec![Kind::Movements],
        // The settings: what each agenda is, among them.
        "config/config.toml" => vec![Kind::Agendas],
        _ => Vec::new(),
    }
}

/// What the sharing wrote here, said by kind, each once.
pub(crate) fn arrived(written: &BTreeSet<String>) {
    let kinds: BTreeSet<Kind> = written.iter().flat_map(|store| kinds_of(store)).collect();
    for kind in kinds {
        sioul_core::changes::said(kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sharing's stores say their kinds, each once whatever came with it.
    #[test]
    fn what_another_device_brought_is_said_by_kind() {
        let written: BTreeSet<String> = ["data/persons.toml", "config/config.toml", "data/health.toml"].into_iter().map(String::from).collect();
        let ((), said) = sioul_core::changes::heard(|| arrived(&written));
        assert_eq!(said, [Kind::Persons, Kind::Agendas]);
        assert_eq!(kinds_of("files/notes/a.md"), []);
    }
}
