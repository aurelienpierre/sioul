// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! `sioul mcp`: what Sioul keeps on this computer, served to AI agents over
//! the Model Context Protocol (<https://modelcontextprotocol.io>, revision
//! 2025-06-18), on standard input and output, as Claude Code and Claude
//! Desktop start it (docs/mcp.md).
//!
//! The pieces stay apart, so that each can be reused:
//! - `transport`: how messages travel. Standard input and output now, one
//!   JSON-RPC message per line; Streamable HTTP, for ChatGPT and remote
//!   clients, comes as another transport over the same server.
//! - `protocol`: JSON-RPC 2.0 and the parts of MCP Sioul answers: the
//!   handshake, ping, tools, resources.
//! - `tools`: the registry, as plain data (a name, a description, a JSON
//!   schema, a function), which another agent protocol can serve as it is.
//! - `read` and `write`: the tools themselves, on the core's public
//!   functions, as the command line uses them; `phone` (the phone's
//!   messages, calls) and `records` (papers, contracts, letters, time,
//!   invoices, reminders, the moment now) read the rest, read-only.
//! - `spam`: the spam filter's tools (computers only), on `sioul spam`'s
//!   own reports; `[mcp] spam = false` keeps them from agents.
//! - `access`: consent per project: every project closed to agents until
//!   the person opens it, things in no project as `[mcp] outside_projects`
//!   says (`sioul_core::consent`); every tool keeps to it.
//! - `mask`: what is never handed to an agent as it is (docs/ai.md); the
//!   command line's `sioul spam` lists masks the same way.
//! - `log`: what each agent asked, and what it was given.
//! - `follow`: the program it runs from, followed: when an install replaces
//!   it, the new one takes over the connection between two messages.
//!
//! Every tool works on this computer's files only: nothing is sent, no
//! password or key is read, nothing is deleted. The tools that write add a
//! task, an event, a note, a draft, a tie or a spam label, or mark a task
//! done; the next sync sends a task or an event to its server, as it does
//! for what you change in the window. A draft waits in Sioul's Drafts: only
//! you send it. Two spam tools start a job apart that reads your mail
//! servers (the training corpus, read-only) or trains; none moves mail.

mod access;
mod follow;
mod items;
mod log;
pub(crate) mod mask;
mod phone;
mod protocol;
pub(crate) mod read;
mod records;
#[cfg(not(target_os = "android"))]
mod spam;
mod tools;
mod transport;
mod write;

#[cfg(test)]
pub(crate) mod tests;

use crate::Session;

/// Serves MCP on standard input and output until the client closes them.
/// Standard output carries the protocol alone: nothing else may print there.
/// A program that took over from the one it replaced (follow.rs) goes on with
/// the same connection, and first tells the client its tools may have changed.
pub(crate) fn run(s: &Session) -> Result<(), String> {
    let mut server = protocol::Server::new(&s.config_path, s.tr.language());
    let mut stdio = transport::Stdio::new()?;
    if let Some(from) = std::env::var(follow::HANDOVER).ok().and_then(|handover| server.take_over(&handover)) {
        let said = format!("its program was replaced: version {from} handed the connection over to version {}", env!("CARGO_PKG_VERSION"));
        log::event(server.client(), "followed", &said);
        transport::Transport::send(&mut stdio, protocol::TOOLS_CHANGED)?;
    }
    transport::serve(&mut server, &mut stdio, follow::Follow::start().as_mut())
}
