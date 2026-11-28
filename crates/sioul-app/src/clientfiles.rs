// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Clients' files in the window (docs/clients.md, "Moving what exists",
//! "Between devices", "Erasing"): what turns them on once the store is open.
//!
//! - **The move** (`sioul_core::clients::moving::run`): once at the first
//!   exchange's turn after the window opens, with the gate as this device's
//!   sharing memory left it; again once the gate opens (every device holds
//!   clients' files: `share::Outcome::client_files_held`), the drafts then
//!   naming their client by its address and each profile adopted, and Sioul's
//!   links from an event to a client's card taken out where they are written
//!   (`work::unlink`, a meeting with others left as it is); again whenever an
//!   exchange brought clients' files or client profiles.
//! - **Clients erased elsewhere**: each pass purges what is left of them here
//!   (`keep::apply_erased`), and what this device's sharing kept of them
//!   (`share::forget_erased_client`).
//! - **One copy or two**: once another device holds the clients' files (its
//!   files sealed and its part on: `Outcome::client_files_elsewhere`), a
//!   client's file is written once; until then twice, as it exists nowhere
//!   else (`door::set_travels`).
//!
//! Nothing of it runs where the store is not open (`door::Opening`): a phone
//! whose files are not sealed keeps no client, and a store not open yet waits.

use crate::backend::{QtThread, Shared};
use sioul_core::clients::door::{self, Opening};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The first pass ran in this process.
static STARTED: AtomicBool = AtomicBool::new(false);
/// The pass through the open gate ran in this process.
static THROUGH_GATE: AtomicBool = AtomicBool::new(false);
/// One pass at a time.
static RUNNING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// What this device's entry says of clients' files (`devices::Entry::client_files`):
/// it holds them where its files are sealed and the part is on; it knows them otherwise.
pub(crate) fn said_in_entry() -> u32 {
    let held = matches!(door::opening(), Opening::Open(_)) && crate::share::shares(sioul_sync::share::CLIENT_FILES_PART);
    if held { sioul_sync::share::CLIENT_FILES_HELD } else { sioul_sync::share::CLIENT_FILES_SHARED }
}

/// After one of the window's exchanges: the twin's switch, then a pass when
/// one is due (the first, the gate just open, clients' files or profiles come in).
pub(crate) fn after_exchange(qt: &QtThread, shared: &Arc<Shared>, outcome: &sioul_sync::share::Outcome) {
    let open = matches!(door::opening(), Opening::Open(_));
    // One copy only once another device holds them (its files sealed, its part on): a phone whose files are not sealed holds none.
    door::set_travels(open && outcome.client_files_elsewhere && crate::share::shares(sioul_sync::share::CLIENT_FILES_PART));
    let first = !STARTED.load(Ordering::SeqCst);
    let gate = outcome.client_files_held && !THROUGH_GATE.load(Ordering::SeqCst);
    let came = outcome.written.contains(sioul_sync::share::CLIENT_FILES_STORE) || outcome.written.contains(sioul_sync::share::CLIENTS);
    if open && (first || gate || came) {
        let (qt, shared, held) = (qt.clone(), Arc::clone(shared), outcome.client_files_held);
        std::thread::spawn(move || pass(&qt, &shared, held));
    }
}

/// The window's first pass when no exchange runs (no sharing set up): the
/// move with the gate shut, as nothing else holds clients.
pub(crate) fn at_start(qt: &QtThread, shared: &Arc<Shared>) {
    if STARTED.load(Ordering::SeqCst) || !matches!(door::opening(), Opening::Open(_)) {
        return;
    }
    let held = sioul_sync::share::client_files_held_in(&crate::share::memory_path(), &crate::share::here().id);
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || pass(&qt, &shared, held));
}

/// One pass: clients erased elsewhere purged, the move, the links it hands
/// back taken out of the events through the gate.
fn pass(qt: &QtThread, shared: &Arc<Shared>, gate_open: bool) {
    let _one = RUNNING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Ok(door) = door::opened() else { return };
    match sioul_core::clients::keep::apply_erased(door.as_ref()) {
        Ok(ids) => {
            for id in ids {
                if let Err(e) = sioul_sync::share::forget_erased_client(&crate::share::memory_path(), &id) {
                    eprintln!("sioul: clients: the sharing's earlier versions of a client erased stay: {e}");
                }
            }
        }
        Err(e) => eprintln!("sioul: clients: a client erased elsewhere is not purged here yet: {e}"),
    }
    let config = crate::backend::load_config();
    let loaded = crate::work::loaded(shared);
    match sioul_core::clients::moving::run(&config, &loaded, door.as_ref(), gate_open) {
        Ok((moved, links)) => {
            STARTED.store(true, Ordering::SeqCst);
            if gate_open {
                THROUGH_GATE.store(true, Ordering::SeqCst);
            }
            for (event, card) in &links {
                crate::work::unlink(qt, shared, event, card);
            }
            if moved.clients + moved.drafts + moved.ties + moved.adopted > 0 {
                eprintln!("sioul: clients: {} made, {} drafts naming their client, {} appointments tied, {} profiles adopted", moved.clients, moved.drafts, moved.ties, moved.adopted);
            }
            if moved.erased > 0 {
                eprintln!("sioul: clients: {} clients erased earlier were met again and left erased", moved.erased);
            }
        }
        // Tried again at the next exchange's turn: nothing is made of a file that does not read.
        Err(e) => eprintln!("sioul: clients: the move waits: {e}"),
    }
}
