// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Supplies in the window (docs/supplies.md): the upkeep after each
//! re-plan, the tasks to resupply made.

use crate::backend::{QtThread, Shared, load_config, say, tell, tr};
use sioul_core::supplies::{self, Supply};
use sioul_core::tasks::Task;
use std::path::PathBuf;
use std::sync::Arc;

/// Where the supplies' list and movements are: the notes folder, or `records/` once moved.
fn root() -> Option<PathBuf> {
    load_config().records_root()
}

/// This device's id in the sharing: its own movements' file.
fn device() -> String {
    crate::notices::here_id()
}

/// After each re-plan, on its thread: the tasks to resupply made, each said
/// once; their account told, and the plan made again when one was made.
pub(crate) fn upkeep(qt: &QtThread, shared: &Arc<Shared>, tasks: &[Task]) {
    let Some(root) = root() else { return };
    let device = device();
    let now = sioul_core::zone::now();
    let config = load_config();
    // No list at all: one on this device, as Health's errands get theirs.
    let list = match supplies::resupply_list(&config) {
        Some(list) => list,
        None if supplies::List::load(&root).is_ok_and(|l| l.supplies.iter().any(|s| s.low_at.is_some())) => match sioul_core::vdir::create(sioul_core::vdir::Kind::Calendars, sioul_core::vdir::LOCAL, &tr().text("supplies-list-name", None), None, &["VTODO"]) {
            Ok(list) => list,
            Err(_) => return,
        },
        None => return,
    };
    let title = |supply: &Supply| say("supplies-resupply-title", &[("name", supply.name_shown().to_string())]);
    match supplies::make_resupplies(&root, &device, tasks, &list, &title, &sioul_core::zone::current(), &now) {
        Ok(made) if !made.is_empty() => {
            crate::pim::nudge(shared, &list.account);
            for uid in &made {
                let named = sioul_core::tasks::read(&list.dir.join(format!("{uid}.ics")), &list, &sioul_core::zone::current()).map(|t| t.title).unwrap_or_default();
                tell(qt, shared, say("supplies-resupply-made", &[("title", named), ("list", list.name.clone())]));
            }
            crate::work::show_work(qt, shared);
        }
        Ok(_) => {}
        Err(e) => eprintln!("sioul: supplies: {e}"),
    }
}
