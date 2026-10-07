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
//!   functions, as the command line uses them.
//! - `spam`: the spam filter's tools (computers only), on `sioul spam`'s
//!   own reports; `[mcp] spam = false` keeps them from agents.
//! - `mask`: what is never handed to an agent as it is (docs/ai.md); the
//!   command line's `sioul spam` lists masks the same way.
//! - `log`: what each agent asked, and what it was given.
//!
//! Every tool works on this computer's files only: nothing is sent, no
//! password or key is read, nothing is deleted. The tools that write add a
//! task, an event, a note, a draft, a tie or a spam label, or mark a task
//! done; the next sync sends a task or an event to its server, as it does
//! for what you change in the window. A draft waits in Sioul's Drafts: only
//! you send it. Two spam tools start a job apart that reads your mail
//! servers (the training corpus, read-only) or trains; none moves mail.

mod log;
pub(crate) mod mask;
mod protocol;
pub(crate) mod read;
#[cfg(not(target_os = "android"))]
mod spam;
mod tools;
mod transport;
mod write;

#[cfg(test)]
mod tests;

use crate::Session;

/// Serves MCP on standard input and output until the client closes them.
/// Standard output carries the protocol alone: nothing else may print there.
pub(crate) fn run(s: &Session) -> Result<(), String> {
    let mut server = protocol::Server::new(&s.config_path, s.tr.language());
    transport::serve(&mut server, &mut transport::Stdio::new())
}
