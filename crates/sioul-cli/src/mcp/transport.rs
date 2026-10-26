// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! How MCP messages travel, apart from what they say (MCP 2025-06-18,
//! "Transports"). The stdio transport is the one Claude Code and Claude
//! Desktop use: the client starts `sioul mcp` and writes one JSON-RPC message
//! per line on its standard input; each answer is one line on its standard
//! output. Streamable HTTP (one POST per message, for ChatGPT and remote
//! clients) will be another `Transport` over the same `protocol::Server`.

use super::follow::Follow;
use super::protocol::{self, Server};
use std::io::{BufRead, Read, Write};
use std::time::Duration;

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
    /// Whether bytes the client sent were taken from it and wait here,
    /// unread: the program is never replaced while some do, or they would be lost.
    fn holds_unread(&self) -> bool {
        false
    }
    /// Waits at most `longest` for the client's next message: false when none
    /// came meanwhile. A transport with its messages at hand never waits.
    fn wait(&mut self, _longest: Duration) -> bool {
        true
    }
}

/// Answers every message until the client goes: requests get their response,
/// notifications nothing. With `follow`, between two messages, while nothing
/// the client sent waits unread, the program's file is looked at, then once a
/// second until the next message comes: a new program takes this one's place
/// (follow.rs).
pub fn serve(server: &mut Server, transport: &mut dyn Transport, mut follow: Option<&mut Follow>) -> Result<(), String> {
    loop {
        if let Some(follow) = follow.as_deref_mut() {
            while !transport.holds_unread() {
                follow.between(server);
                if transport.wait(super::follow::EVERY) {
                    break;
                }
            }
        }
        let Some(incoming) = transport.receive()? else {
            return Ok(());
        };
        let answer = match incoming {
            Incoming::Message(message) if message.trim().is_empty() => continue,
            Incoming::Message(message) => server.handle(&message),
            Incoming::Unreadable(why) => Some(protocol::unreadable(why)),
        };
        if let Some(answer) = answer {
            transport.send(&answer)?;
        }
    }
}

/// The longest message taken, in bytes: a note or a draft is far shorter,
/// and a client that sends more cannot fill the memory with one line.
const LONGEST_LINE: usize = 8 << 20;

/// Standard input and output: newline-delimited JSON, as MCP's stdio transport says.
pub struct Stdio {
    input: Input,
    output: std::io::Stdout,
}

/// Standard input, read on Unix through a buffer of this transport's own, so
/// that what it holds unread can be told (`holds_unread`).
#[cfg(unix)]
type Input = std::io::BufReader<std::fs::File>;
#[cfg(not(unix))]
type Input = std::io::StdinLock<'static>;

impl Stdio {
    pub fn new() -> Result<Stdio, String> {
        #[cfg(unix)]
        let input = {
            // A copy of descriptor 0, closed at an exec: descriptor 0 itself goes
            // on to the program that takes over, with what the pipe holds unread.
            use std::os::fd::AsFd;
            let copy = std::io::stdin().as_fd().try_clone_to_owned().map_err(|e| format!("standard input: {e}"))?;
            std::io::BufReader::new(std::fs::File::from(copy))
        };
        #[cfg(not(unix))]
        let input = std::io::stdin().lock();
        Ok(Stdio { input, output: std::io::stdout() })
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

    #[cfg(unix)]
    fn holds_unread(&self) -> bool {
        !self.input.buffer().is_empty()
    }

    #[cfg(unix)]
    fn wait(&mut self, longest: Duration) -> bool {
        use std::os::fd::AsRawFd;
        if self.holds_unread() {
            return true;
        }
        let mut asked = libc::pollfd { fd: self.input.get_ref().as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let milliseconds = i32::try_from(longest.as_millis()).unwrap_or(i32::MAX);
        // SAFETY: one pollfd, alive for the call, and their count, 1.
        let ready = unsafe { libc::poll(&mut asked, 1, milliseconds) };
        // Something to read, the client gone or a failure: the read says which.
        // A signal: as if the time had passed.
        ready > 0 || (ready < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted)
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

    fn holds_unread(&self) -> bool {
        !self.incoming.is_empty()
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

    fn holds_unread(&self) -> bool {
        !self.input.is_empty()
    }
}
