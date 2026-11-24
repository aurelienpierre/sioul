// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The wiring: "when this happens, do that", for everything Sioul carries
//! (docs/rules.md).
//!
//! Each type of object declares what can happen to it (its events), what
//! can be read on it (its fields, for conditions) and what can be done to
//! it (its actions), as data in code (`declare`): the editor is built from
//! them, never written by hand for a pair of types. A rule is one sentence,
//! "when something happens to an object of a type, and its fields say so,
//! do one action", kept in one of Sioul's own lists (`store`). Rules do not
//! fire on a signal one device may miss: for each object that matches a
//! rule, its result must exist (`engine`). What a rule makes has an
//! identifier worked out from the rule and the object, so that two devices
//! write the same thing once; what it changes is written in a shared record
//! of acts (`acts`), which every device reads before acting, and which also
//! remembers "not for this one".
//!
//! Phase 1 (`declare::PHASE`): a message arrives; tie it to a project, make
//! a task from a model, give it a topic, tell. The existing mechanisms
//! (projects' routes and the tasks they make, budgets' mail rules, mail
//! filters, topics' sender rules) keep running as they do; `readers` shows
//! them as rules of the same shape, and the engine never runs them. The one
//! function the window, the phone's background step and `sioul watch` call
//! when mail arrives is `arrival::arrived`.

pub mod acts;
pub mod arrival;
pub mod declare;
pub mod engine;
pub mod message;
pub mod readers;
pub mod store;
pub mod words;

#[cfg(test)]
mod store_tests;

#[cfg(test)]
mod engine_tests;

#[cfg(test)]
mod readers_tests;
