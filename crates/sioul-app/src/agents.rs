// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What an AI agent changed through `sioul mcp`, said in the window (25
//! October 2026; `sioul_core::agentwrites`): each change its line in the
//! status line, with Undo, which puts its files back while they are still
//! as the agent left them; the agenda and the task pages read again. On a
//! computer: agents run there.

use crate::backend::{QtThread, Shared, tr};
use crate::mail;
use sioul_core::agentwrites;
use std::sync::Arc;
use std::time::Duration;

/// Looked at this often: a stat, most times.
const EVERY: Duration = Duration::from_secs(2);

/// Watches what agents change, from the end of what they changed before this window opened.
pub(crate) fn watch(qt: &QtThread, shared: &Arc<Shared>) {
    if cfg!(target_os = "android") {
        return;
    }
    let (qt, shared) = (qt.clone(), Arc::clone(shared));
    std::thread::spawn(move || {
        let _ = agentwrites::trim(jiff::Timestamp::now().as_millisecond());
        let (_, mut offset) = agentwrites::since(u64::MAX);
        loop {
            std::thread::sleep(EVERY);
            let len = std::fs::metadata(agentwrites::path()).map_or(0, |m| m.len());
            if len == offset {
                continue;
            }
            let (added, now) = agentwrites::since(offset);
            offset = now;
            for said in added {
                say(&qt, &shared, said);
            }
        }
    });
}

/// One change said, with Undo; the pages read again. The window's own
/// changes of several files say themselves so too (`work`, composed tasks).
pub(crate) fn say(qt: &QtThread, shared: &Arc<Shared>, said: agentwrites::Said) {
    let accounts: std::collections::BTreeSet<String> = said.files.iter().filter_map(|f| crate::pim::account_of(&f.path)).collect();
    for account in &accounts {
        crate::pim::nudge(shared, account);
    }
    refresh(qt, shared);
    let line = said.line.clone();
    mail::offer_back(qt, shared, line, move |qt, shared| {
        if !said.put_back() {
            return tr().text("undo-too-late", None);
        }
        for account in &accounts {
            crate::pim::nudge(shared, account);
        }
        refresh(qt, shared);
        String::new()
    });
}

fn refresh(qt: &QtThread, shared: &Arc<Shared>) {
    if let Ok(mut cache) = shared.loaded.lock() {
        *cache = None;
    }
    crate::pim::show_pim(qt, shared);
    crate::work::show_work(qt, shared);
}
