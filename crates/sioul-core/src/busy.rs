// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! When you are busy (design of 18 November 2026, wave E): during an event
//! marked busy, and during a task marked busy while its timer runs.
//!
//! A stand-in: the real module comes from the branch that builds the Busy
//! mark (`reach`), and a merge takes theirs whole. Until then nobody is ever
//! busy, so the calls' table's "While busy" column is never read
//! (`calls::build`).

/// The stretches during which you are marked busy between `from` and `to`
/// (Unix seconds), sorted and merged. The stand-in: none.
pub fn stretches(from: i64, to: i64) -> Vec<(i64, i64)> {
    let _ = (from, to);
    Vec::new()
}
