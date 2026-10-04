// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! How MCP messages travel, apart from what they say (MCP 2025-06-18,
//! "Transports"). The stdio transport is the one Claude Code and Claude
//! Desktop use: the client starts `sioul mcp` and writes one JSON-RPC message
//! per line on its standard input; each answer is one line on its standard
//! output. Streamable HTTP (one POST per message, for ChatGPT and remote
//! clients) will be another `Transport` over the same `protocol::Server`.

use super::protocol::{self, Server};
use std::io::{BufRead, Read, Write};

/// What came: one message, or a line that could not be taken (too long,
/// not UTF-8), which is said to the client before the next one is read.
pub enum Incoming {
    Message(String),
    Unreadable(&'static str),
}

/// Where messages come from and go to.
pub trait Transport {
    /// The next message, without its line ending; None once the client has gone.
    fn receive(&mut self) -> Result<Option<Incoming>, String>;
    /// Sends one message, whole, at once.
    fn send(&mut self, message: &str) -> Result<(), String>;
}

/// Answers every message until the client goes: requests get their response,
/// notifications nothing.
pub fn serve(server: &mut Server, transport: &mut dyn Transport) -> Result<(), String> {
    while let Some(incoming) = transport.receive()? {
        let answer = match incoming {
            Incoming::Message(message) if message.trim().is_empty() => continue,
            Incoming::Message(message) => server.handle(&message),
            Incoming::Unreadable(why) => Some(protocol::unreadable(why)),
        };
        if let Some(answer) = answer {
            transport.send(&answer)?;
        }
    }
    Ok(())
}

/// The longest message taken, in bytes: a note or a draft is far shorter,
/// and a client that sends more cannot fill the memory with one line.
const LONGEST_LINE: usize = 8 << 20;

/// Standard input and output: newline-delimited JSON, as MCP's stdio transport says.
pub struct Stdio {
    input: std::io::StdinLock<'static>,
    output: std::io::Stdout,
}

impl Stdio {
    pub fn new() -> Stdio {
        Stdio { input: std::io::stdin().lock(), output: std::io::stdout() }
    }
}

impl Transport for Stdio {
    fn receive(&mut self) -> Result<Option<Incoming>, String> {
        read_message(&mut self.input, LONGEST_LINE)
    }

    fn send(&mut self, message: &str) -> Result<(), String> {
        // A message holds no line break of its own: serde_json escapes them in strings.
        let mut out = self.output.lock();
        writeln!(out, "{message}").and_then(|()| out.flush()).map_err(|e| format!("standard output: {e}"))
    }
}

/// One line of `input`, at most `longest` bytes; the rest of a longer one skipped.
fn read_message(input: &mut impl BufRead, longest: usize) -> Result<Option<Incoming>, String> {
    let fail = |e: std::io::Error| format!("standard input: {e}");
    let mut line = Vec::new();
    let read = (&mut *input).take(longest as u64 + 1).read_until(b'\n', &mut line).map_err(fail)?;
    if read == 0 {
        return Ok(None);
    }
    if line.len() > longest && line.last() != Some(&b'\n') {
        // The rest of that line, up to its end, left unread.
        loop {
            let buffer = input.fill_buf().map_err(fail)?;
            if buffer.is_empty() {
                break;
            }
            match buffer.iter().position(|&b| b == b'\n') {
                Some(at) => {
                    input.consume(at + 1);
                    break;
                }
                None => {
                    let length = buffer.len();
                    input.consume(length);
                }
            }
        }
        return Ok(Some(Incoming::Unreadable("A message is one line of at most 8 MiB.")));
    }
    while matches!(line.last(), Some(b'\n' | b'\r')) {
        line.pop();
    }
    Ok(Some(match String::from_utf8(line) {
        Ok(text) => Incoming::Message(text),
        Err(_) => Incoming::Unreadable("A message is UTF-8 text."),
    }))
}

/// Messages given in advance, and what was answered: for tests.
#[cfg(test)]
pub struct Script {
    pub incoming: std::collections::VecDeque<String>,
    pub outgoing: Vec<String>,
}

#[cfg(test)]
impl Transport for Script {
    fn receive(&mut self) -> Result<Option<Incoming>, String> {
        Ok(self.incoming.pop_front().map(Incoming::Message))
    }

    fn send(&mut self, message: &str) -> Result<(), String> {
        self.outgoing.push(message.to_string());
        Ok(())
    }
}

/// Bytes given in advance, read as standard input is: for tests.
#[cfg(test)]
pub struct Bytes<'a> {
    pub input: &'a [u8],
    pub longest: usize,
    pub outgoing: Vec<String>,
}

#[cfg(test)]
impl Transport for Bytes<'_> {
    fn receive(&mut self) -> Result<Option<Incoming>, String> {
        read_message(&mut self.input, self.longest)
    }

    fn send(&mut self, message: &str) -> Result<(), String> {
        self.outgoing.push(message.to_string());
        Ok(())
    }
}
