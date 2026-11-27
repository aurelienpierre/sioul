// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Files sealed line by line on the disk: the manifests' journals, and the
//! files that only grow (`Store::append_line`). Format in `format.rs`: each
//! line carries its number in clear, bound to it. An append writes and
//! flushes one line. A line cut by a power cut is the last one, without its
//! end, and it is dropped by the next append; a line that does not open
//! elsewhere is that line lost, whatever it did to the line ends; a head
//! (line 0) that does not open blocks neither reading nor appending.

use super::disk;
use super::format::{self, Fault, Inner, Role};
use chacha20poly1305::XChaCha20Poly1305;
use std::io::{Read, Seek, Write};
use std::path::Path;

/// A line read: its number (none when even that does not read) and its bytes (none when it does not open).
pub(crate) type Line = (Option<u64>, Option<Vec<u8>>);

/// A lines file read.
pub(crate) struct Scanned {
    pub role: Role,
    pub generation: u64,
    /// Line 0, when it opens.
    pub head: Option<Inner>,
    /// Every other whole line, in the file's order.
    pub lines: Vec<Line>,
    /// Bytes up to the end of the last whole line.
    pub valid: u64,
    /// The next line's number: one past the highest read (line 0 counted).
    pub next: u64,
}

/// A new lines file at `target`: its clear line and its line 0, put whole.
pub(crate) fn create(target: &Path, cipher: &XChaCha20Poly1305, role: Role, generation: u64, bind: &str, head: &Inner) -> std::io::Result<u64> {
    let mut text = format::lines_header(role, generation);
    text.push_str(&format::seal_line(cipher, bind, generation, 0, &head.encode())?);
    disk::place(target, |out| out.write_all(text.as_bytes()))?;
    Ok(text.len() as u64)
}

/// A lines file read whole: `Missing` when it is not there; each other line
/// opened or not. Its clear first line says its role and generation; when it
/// does not read and the caller knows them (`expected`: a file's record, a
/// journal's manifest), the lines after it are read all the same, since each
/// is sealed and bound to what the caller knows: one flipped bit there never
/// costs the whole file. `Broken` only when neither says them, or when no line
/// ends at all.
pub(crate) fn scan(path: &Path, cipher: &XChaCha20Poly1305, bind: &dyn Fn(Role, u64) -> String, expected: Option<(Role, u64)>) -> Result<Scanned, Fault> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Fault::Missing),
        Err(e) => return Err(Fault::Io(e)),
    };
    if bytes.is_empty() {
        return Err(Fault::Missing);
    }
    let first_end = bytes.iter().position(|&b| b == b'\n').ok_or(Fault::Broken("cut short"))?;
    let (role, generation) = match (format::parse_lines_header(&bytes[..first_end]), expected) {
        (Ok(said), Some(known)) if said != known => known,
        (Ok(said), _) => said,
        (Err(_), Some(known)) => known,
        (Err(fault), None) => return Err(fault),
    };
    let bind = bind(role, generation);
    let at = first_end + 1;
    let (mut lines, valid, next) = read_lines(&bytes[at..], at as u64, cipher, &bind, generation);
    let head = lines.iter().position(|(n, _)| *n == Some(0)).and_then(|i| lines.remove(i).1).and_then(|plain| Inner::decode(&plain));
    lines.retain(|(n, _)| *n != Some(0));
    Ok(Scanned { role, generation, head, lines, valid, next: next.max(1) })
}

/// The whole lines in `bytes` (which start at `offset` in the file): each
/// with its number and bytes, where the whole lines end, and one past the
/// highest number read.
pub(crate) fn read_lines(bytes: &[u8], offset: u64, cipher: &XChaCha20Poly1305, bind: &str, generation: u64) -> (Vec<Line>, u64, u64) {
    let mut lines = Vec::new();
    let (mut at, mut valid, mut next) = (0usize, offset, 0u64);
    while at < bytes.len() {
        let Some(end) = bytes[at..].iter().position(|&b| b == b'\n') else { break };
        let line = &bytes[at..at + end];
        let read = match format::open_line(cipher, bind, generation, line) {
            Some((number, plain)) => (Some(number), Some(plain)),
            None => (format::line_number(line), None),
        };
        // Only a line that opens counts: a flipped digit in a clear number never raises the count.
        if let (Some(number), Some(_)) = (read.0, &read.1) {
            next = next.max(number.saturating_add(1));
        }
        lines.push(read);
        at += end + 1;
        valid = offset + at as u64;
    }
    (lines, valid, next)
}

/// The whole lines after byte `offset`, for a file read before and grown since.
pub(crate) fn scan_tail(path: &Path, offset: u64, cipher: &XChaCha20Poly1305, bind: &str, generation: u64) -> Result<(Vec<Line>, u64, u64), Fault> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Fault::Missing),
        Err(e) => return Err(Fault::Io(e)),
    };
    file.seek(std::io::SeekFrom::Start(offset)).map_err(Fault::Io)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(Fault::Io)?;
    Ok(read_lines(&bytes, offset, cipher, bind, generation))
}

/// One line appended as line `number`, what follows `valid` (a cut line) cut first, flushed.
pub(crate) fn append(path: &Path, valid: u64, cipher: &XChaCha20Poly1305, bind: &str, generation: u64, number: u64, plain: &[u8]) -> std::io::Result<u64> {
    let line = format::seal_line(cipher, bind, generation, number, plain)?;
    let mut file = std::fs::OpenOptions::new().write(true).open(path)?;
    if file.metadata()?.len() != valid {
        file.set_len(valid)?;
    }
    file.seek(std::io::SeekFrom::Start(valid))?;
    file.write_all(line.as_bytes())?;
    file.sync_data()?;
    Ok(valid + line.len() as u64)
}
