// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What is never handed to an agent as it is (docs/ai.md, "Masking"): the
//! core's mask (`sioul_core::mask`), the same the AI shield's reading uses,
//! so that no AI Sioul hands mail to gets more than another.

pub use sioul_core::mask::{json, mark_in, message, text};
