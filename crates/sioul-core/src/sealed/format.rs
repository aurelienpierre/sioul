// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The sealed format (docs/store.md, "The format").
//!
//! **A blob**: a header in clear (`sioul-sealed\n`, the format, the blob's
//! role, its copy, the piece size, its generation), then pieces: each a
//! length (its top bit set for the last), a random 192-bit nonce, and the
//! piece sealed with XChaCha20-Poly1305, bound to the clear header, the
//! blob's own name, the piece's index and whether it is the last, as
//! `sioul_sync::blobs` binds the sharing's pieces (its `bound`: "blob", the
//! name, the index, "end"), the header added. Piece 0 holds the file's
//! logical path, when it was written and by whom, and whether its content is
//! compressed, padded to 512 bytes (or the next multiple) so that its length
//! says nothing of the path's; the pieces after it the content, gzip-compressed
//! (as the sharing's) unless its writer said not (what an outsider can drive
//! the size of); the last one its size before compression. Everything a reader needs is in the blob and its name, so a
//! lost manifest is rebuilt from the blobs; a blob copied under another name,
//! or an older generation put in a newer's place, does not open there.
//!
//! **A file that only grows** (and a manifest's journal): a clear first line
//! (`sioul-lines 1 <role> <generation>`), then one line per line: its number
//! in clear, then the nonce and the sealed bytes in base64, bound to the file
//! (its name, or its folder for a journal), its generation and that number,
//! as `sioul_sync::textseal` seals texts' lines. A line lost or cut costs
//! that line alone, whatever it did to the line ends. Line 0 holds the path,
//! padded as piece 0 is.
//!
//! A format this Sioul does not know is damage here (the store's own format
//! is told by `store.toml`, never by one byte of a blob a flipped bit can
//! change), and an error of the disk stays an error of the disk: the next
//! copy is tried, nothing is marked damaged for it.
//!
//! **Names**: base32 (lower case: one name on systems that ignore case) of
//! HMAC-SHA-256 under the naming key of the role, the logical path, the
//! generation and the copy, as the sharing names its blobs; stored as
//! `ab/<the rest>` (1 024 folders of fan-out).
//!
//! Where it differs from `blobs.rs`: pieces of 64 KiB rather than 1 MiB (the
//! store's files are mostly small, and a piece is held in memory while it is
//! opened); gzip's fastest level rather than its default (the store writes
//! often; measured by the bench); the end piece carries the size, which a
//! stream written once does not know at its start.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::{Aead, AeadCore, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(crate) const MAGIC: &[u8; 13] = b"sioul-sealed\n";
/// The format this Sioul writes and reads.
pub(crate) const FORMAT: u8 = 1;
/// Piece 0 and line 0 are padded to a multiple of this.
const HEAD_PAD: usize = 512;
pub(crate) const HEADER: usize = 28;
/// What is sealed at once.
pub(crate) const PIECE: u32 = 64 << 10;
/// The largest piece a reader accepts, whatever a header says.
const LARGEST_PIECE: u32 = 16 << 20;
/// In a piece's length: the last one.
const END: u32 = 1 << 31;
/// A nonce and a tag around each piece.
const AROUND: usize = 24 + 16;
const LINES: &str = "sioul-lines";

/// What a blob is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    File,
    Manifest,
    Journal,
    Lines,
    Root,
    Lock,
}

impl Role {
    fn byte(self) -> u8 {
        match self {
            Role::File => 1,
            Role::Manifest => 2,
            Role::Journal => 3,
            Role::Lines => 4,
            Role::Root => 5,
            Role::Lock => 6,
        }
    }

    fn from_byte(byte: u8) -> Option<Role> {
        Some(match byte {
            1 => Role::File,
            2 => Role::Manifest,
            3 => Role::Journal,
            4 => Role::Lines,
            5 => Role::Root,
            6 => Role::Lock,
            _ => return None,
        })
    }
}

/// A blob's clear header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Header {
    pub role: Role,
    pub copy: u8,
    pub piece: u32,
    pub generation: u64,
}

impl Header {
    pub fn new(role: Role, copy: u8, generation: u64) -> Header {
        Header { role, copy, piece: PIECE, generation }
    }

    pub fn bytes(&self) -> [u8; HEADER] {
        let mut bytes = [0u8; HEADER];
        bytes[..13].copy_from_slice(MAGIC);
        bytes[13] = FORMAT;
        bytes[14] = self.role.byte();
        bytes[15] = self.copy;
        bytes[16..20].copy_from_slice(&self.piece.to_be_bytes());
        bytes[20..28].copy_from_slice(&self.generation.to_be_bytes());
        bytes
    }

    pub fn parse(bytes: &[u8; HEADER]) -> Result<Header, Fault> {
        if &bytes[..13] != MAGIC {
            return Err(Fault::Broken("not a sealed file"));
        }
        if bytes[13] != FORMAT {
            return Err(Fault::Broken("not this format"));
        }
        let role = Role::from_byte(bytes[14]).ok_or(Fault::Broken("an unknown role"))?;
        let piece = u32::from_be_bytes(bytes[16..20].try_into().unwrap_or_default());
        if !(1..=LARGEST_PIECE).contains(&piece) {
            return Err(Fault::Broken("a piece size out of bounds"));
        }
        Ok(Header { role, copy: bytes[15], piece, generation: u64::from_be_bytes(bytes[20..28].try_into().unwrap_or_default()) })
    }
}

/// What piece 0 (or line 0) holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Inner {
    pub path: String,
    pub written_ms: i64,
    pub origin: String,
    /// The content is gzip-compressed (blobs only).
    pub compressed: bool,
}

impl Inner {
    /// Its bytes, padded with zeros to a multiple of `HEAD_PAD`: the length says nothing of the path's.
    pub fn encode(&self) -> Vec<u8> {
        let path = self.path.as_bytes();
        let origin = &self.origin.as_bytes()[..self.origin.len().min(255)];
        let mut out = Vec::with_capacity(HEAD_PAD);
        out.push(1);
        out.push(u8::from(self.compressed));
        out.extend_from_slice(&(path.len() as u16).to_be_bytes());
        out.extend_from_slice(path);
        out.extend_from_slice(&self.written_ms.to_be_bytes());
        out.push(origin.len() as u8);
        out.extend_from_slice(origin);
        out.resize(out.len().div_ceil(HEAD_PAD) * HEAD_PAD, 0);
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Inner> {
        let (&version, rest) = bytes.split_first()?;
        let (&flags, rest) = rest.split_first()?;
        if version != 1 || flags > 1 || rest.len() < 2 {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([rest[0], rest[1]]));
        let rest = &rest[2..];
        let path = std::str::from_utf8(rest.get(..length)?).ok()?.to_string();
        let rest = &rest[length..];
        let written_ms = i64::from_be_bytes(rest.get(..8)?.try_into().ok()?);
        let (&origin_length, rest) = rest.get(8..)?.split_first()?;
        let origin = std::str::from_utf8(rest.get(..usize::from(origin_length))?).ok()?.to_string();
        // Nothing but its padding after it.
        rest[usize::from(origin_length)..].iter().all(|&b| b == 0).then_some(Inner { path, written_ms, origin, compressed: flags == 1 })
    }
}

/// Why a blob did not open.
#[derive(Debug)]
pub(crate) enum Fault {
    /// Not there (or only an empty placeholder).
    Missing,
    /// There, but it does not open whole, or it is another blob (why, for a reader of the code and a debugger).
    Broken(#[allow(dead_code)] &'static str),
    /// The disk or the system refused (a read error, rights): never taken for damage.
    Io(std::io::Error),
}

/// A short read is a cut blob; any other error of the disk stays one.
fn short(e: std::io::Error) -> Fault {
    if e.kind() == std::io::ErrorKind::UnexpectedEof { Fault::Broken("cut short") } else { Fault::Io(e) }
}

/// A name on the disk for (role, path, generation, copy): 52 characters of base32.
pub(crate) fn name(naming: &Hmac<Sha256>, role: Role, path: &str, generation: u64, copy: u8) -> String {
    let mut mac = naming.clone();
    mac.update(b"sioul-store 1\x1f");
    mac.update(&[role.byte(), 0x1f]);
    mac.update(path.as_bytes());
    mac.update(&[0x1f]);
    mac.update(&generation.to_be_bytes());
    mac.update(&[0x1f, copy]);
    base32(&mac.finalize().into_bytes())
}

/// A name's place under a `sealed/` folder: `ab/<the rest>`.
pub(crate) fn place(sealed: &Path, name: &str) -> PathBuf {
    sealed.join(&name[..2]).join(&name[2..])
}

/// A name from its place, when it is one.
pub(crate) fn name_of(place: &Path) -> Option<String> {
    let rest = place.file_name()?.to_str()?;
    let fan = place.parent()?.file_name()?.to_str()?;
    let name = format!("{fan}{rest}");
    (name.len() == 52 && name.bytes().all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))).then_some(name)
}

fn base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(ALPHABET[((buffer >> bits) & 31) as usize]));
        }
    }
    if bits > 0 {
        out.push(char::from(ALPHABET[((buffer << (5 - bits)) & 31) as usize]));
    }
    out
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The first half of a SHA-256, in hex: what tells a blob changed (the seal itself tells it damaged).
pub(crate) fn short_hash(hasher: Sha256) -> String {
    hex(&hasher.finalize()[..16])
}

/// What passes through, hashed and counted.
pub(crate) struct Hashing<T> {
    pub inner: T,
    pub hasher: Sha256,
    pub size: u64,
}

impl<T> Hashing<T> {
    pub fn new(inner: T) -> Hashing<T> {
        Hashing { inner, hasher: Sha256::new(), size: 0 }
    }
}

impl<R: Read> Read for Hashing<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..n]);
        self.size += n as u64;
        Ok(n)
    }
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buffer)?;
        self.hasher.update(&buffer[..n]);
        self.size += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// What passes through, counted.
struct Counting<T> {
    inner: T,
    size: u64,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buffer)?;
        self.size += n as u64;
        Ok(n)
    }
}

impl<W: Write> Write for Counting<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buffer)?;
        self.size += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// What a piece is bound to: the clear header, the blob's own name, its index, whether it ends.
fn bound(header: &[u8; HEADER], name: &str, index: u64, end: bool) -> Vec<u8> {
    let mut aad = Vec::with_capacity(HEADER + name.len() + 20);
    aad.extend_from_slice(header);
    aad.extend_from_slice(b"\x1fsealed\x1f");
    aad.extend_from_slice(name.as_bytes());
    aad.push(0x1f);
    aad.extend_from_slice(&index.to_be_bytes());
    aad.push(u8::from(end));
    aad
}

/// As much as `reader` gives, up to `buffer`'s length.
fn fill(reader: &mut impl Read, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

fn write_piece(out: &mut impl Write, cipher: &XChaCha20Poly1305, header: &[u8; HEADER], name: &str, index: u64, end: bool, plain: &[u8]) -> std::io::Result<()> {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, Payload { msg: plain, aad: &bound(header, name, index, end) }).map_err(|_| std::io::Error::other("sealing"))?;
    let length = (nonce.len() + sealed.len()) as u32 | if end { END } else { 0 };
    out.write_all(&length.to_be_bytes())?;
    out.write_all(&nonce)?;
    out.write_all(&sealed)
}

/// Where an opened blob's content goes: through gzip, or as it is; counted.
enum Sink<W: Write> {
    Packed(flate2::write::GzDecoder<Counting<W>>),
    Plain(Counting<W>),
}

impl<W: Write> Sink<W> {
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            Sink::Packed(packed) => packed.write_all(bytes),
            Sink::Plain(plain) => plain.write_all(bytes),
        }
    }

    fn size(&self) -> u64 {
        match self {
            Sink::Packed(packed) => packed.get_ref().size,
            Sink::Plain(plain) => plain.size,
        }
    }

    fn finish(self) -> std::io::Result<Counting<W>> {
        match self {
            Sink::Packed(packed) => packed.finish(),
            Sink::Plain(plain) => Ok(plain),
        }
    }
}

/// What sealing made.
#[derive(Clone, Debug)]
pub(crate) struct Made {
    pub sealed_size: u64,
    pub hash: String,
    pub plain: u64,
}

/// `content` sealed into `out` as the blob `name`, compressed as `inner` says.
pub(crate) fn seal(out: &mut dyn Write, cipher: &XChaCha20Poly1305, name: &str, header: &Header, inner: &Inner, content: &mut dyn Read) -> std::io::Result<Made> {
    let mut out = Hashing::new(out);
    let clear = header.bytes();
    out.write_all(&clear)?;
    write_piece(&mut out, cipher, &clear, name, 0, false, &inner.encode())?;
    let mut counted = Counting { inner: content, size: 0 };
    let mut piece = vec![0u8; header.piece as usize];
    let mut index = 1;
    {
        let mut source: Box<dyn Read + '_> = if inner.compressed { Box::new(flate2::read::GzEncoder::new(&mut counted, flate2::Compression::fast())) } else { Box::new(&mut counted) };
        loop {
            let n = fill(&mut source, &mut piece)?;
            if n == 0 {
                break;
            }
            write_piece(&mut out, cipher, &clear, name, index, false, &piece[..n])?;
            index += 1;
        }
    }
    let plain = counted.size;
    write_piece(&mut out, cipher, &clear, name, index, true, &plain.to_be_bytes())?;
    out.flush()?;
    Ok(Made { sealed_size: out.size, hash: short_hash(out.hasher), plain })
}

/// What a reader expects of a blob: anything else is another blob, and does not open.
pub(crate) struct Expected<'a> {
    pub role: Role,
    pub copy: u8,
    pub generation: Option<u64>,
    pub path: Option<&'a str>,
}

/// A blob opened.
#[derive(Debug)]
pub(crate) struct Got {
    pub header: Header,
    pub inner: Inner,
    pub plain: u64,
    pub sealed_size: u64,
    pub hash: String,
}

fn read_piece(reader: &mut impl Read, cipher: &XChaCha20Poly1305, clear: &[u8; HEADER], header: &Header, name: &str, index: u64) -> Result<(bool, Vec<u8>), Fault> {
    let mut length = [0u8; 4];
    reader.read_exact(&mut length).map_err(short)?;
    let length = u32::from_be_bytes(length);
    let (end, length) = (length & END != 0, (length & !END) as usize);
    if !(AROUND..=header.piece as usize + AROUND + 64).contains(&length) {
        return Err(Fault::Broken("a piece out of bounds"));
    }
    let mut sealed = vec![0u8; length];
    reader.read_exact(&mut sealed).map_err(short)?;
    let (nonce, body) = sealed.split_at(24);
    let plain = cipher.decrypt(XNonce::from_slice(nonce), Payload { msg: body, aad: &bound(clear, name, index, end) }).map_err(|_| Fault::Broken("a piece does not open"))?;
    Ok((end, plain))
}

fn begin(reader: &mut impl Read) -> Result<([u8; HEADER], Header), Fault> {
    let mut clear = [0u8; HEADER];
    let mut got = 0;
    while got < HEADER {
        match reader.read(&mut clear[got..]) {
            Ok(0) if got == 0 => return Err(Fault::Missing),
            Ok(0) => return Err(Fault::Broken("cut short")),
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(Fault::Io(e)),
        }
    }
    let header = Header::parse(&clear)?;
    Ok((clear, header))
}

/// A blob's header and piece 0 only: what it is and which file (a rebuild's look).
pub(crate) fn open_head(mut reader: &mut dyn Read, cipher: &XChaCha20Poly1305, name: &str) -> Result<(Header, Inner), Fault> {
    let (clear, header) = begin(&mut reader)?;
    let (end, plain) = read_piece(&mut reader, cipher, &clear, &header, name, 0)?;
    if end {
        return Err(Fault::Broken("no content"));
    }
    let inner = Inner::decode(&plain).ok_or(Fault::Broken("its head does not read"))?;
    Ok((header, inner))
}

/// A blob opened whole into `out`: never more than `largest` bytes written,
/// whatever the blob says; checked to its end and nothing after it.
pub(crate) fn open(reader: &mut dyn Read, cipher: &XChaCha20Poly1305, name: &str, expected: &Expected, out: &mut dyn Write, largest: u64) -> Result<Got, Fault> {
    let mut reader = Hashing::new(reader);
    let (clear, header) = begin(&mut reader)?;
    if header.role != expected.role || header.copy != expected.copy || expected.generation.is_some_and(|g| g != header.generation) {
        return Err(Fault::Broken("another blob"));
    }
    let (end, plain) = read_piece(&mut reader, cipher, &clear, &header, name, 0)?;
    if end {
        return Err(Fault::Broken("no content"));
    }
    let inner = Inner::decode(&plain).ok_or(Fault::Broken("its head does not read"))?;
    if expected.path.is_some_and(|path| path != inner.path) {
        return Err(Fault::Broken("another file"));
    }
    let fault = |e: std::io::Error| if matches!(e.kind(), std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData | std::io::ErrorKind::WriteZero) { Fault::Broken("its content does not inflate") } else { Fault::Io(e) };
    let counted = Counting { inner: out, size: 0 };
    let mut sink = if inner.compressed { Sink::Packed(flate2::write::GzDecoder::new(counted)) } else { Sink::Plain(counted) };
    let mut index = 1;
    let size = loop {
        let (end, plain) = read_piece(&mut reader, cipher, &clear, &header, name, index)?;
        if end {
            let size: [u8; 8] = plain.as_slice().try_into().map_err(|_| Fault::Broken("its end does not read"))?;
            break u64::from_be_bytes(size);
        }
        // Inflated a little at a time: a piece that inflates past what is allowed stops there.
        for part in plain.chunks(16 << 10) {
            sink.write_all(part).map_err(fault)?;
            if sink.size() > largest {
                return Err(Fault::Broken("larger than allowed"));
            }
        }
        index += 1;
    };
    match reader.read(&mut [0u8; 1]) {
        Ok(0) => {}
        Ok(_) => return Err(Fault::Broken("more after its end")),
        Err(e) => return Err(Fault::Io(e)),
    }
    let counted = sink.finish().map_err(fault)?;
    if counted.size != size {
        return Err(Fault::Broken("not the size it was sealed at"));
    }
    Ok(Got { header, inner, plain: size, sealed_size: reader.size, hash: short_hash(reader.hasher) })
}

// Files that only grow.

/// The first, clear line of a lines file.
pub(crate) fn lines_header(role: Role, generation: u64) -> String {
    format!("{LINES} {FORMAT} {} {generation}\n", role.byte())
}

/// A lines file's role and generation, from its first line (without its end).
pub(crate) fn parse_lines_header(line: &[u8]) -> Result<(Role, u64), Fault> {
    let text = std::str::from_utf8(line).map_err(|_| Fault::Broken("not a lines file"))?;
    let mut words = text.split(' ');
    if words.next() != Some(LINES) {
        return Err(Fault::Broken("not a lines file"));
    }
    let format: u8 = words.next().and_then(|w| w.parse().ok()).ok_or(Fault::Broken("not a lines file"))?;
    if format != FORMAT {
        return Err(Fault::Broken("not this format"));
    }
    let role = words.next().and_then(|w| w.parse().ok()).and_then(Role::from_byte).ok_or(Fault::Broken("not a lines file"))?;
    let generation = words.next().and_then(|w| w.parse().ok()).ok_or(Fault::Broken("not a lines file"))?;
    if words.next().is_some() {
        return Err(Fault::Broken("not a lines file"));
    }
    Ok((role, generation))
}

fn line_bound(bind: &str, generation: u64, number: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(bind.len() + 32);
    aad.extend_from_slice(b"sioul-lines\x1f");
    aad.extend_from_slice(bind.as_bytes());
    aad.push(0x1f);
    aad.extend_from_slice(&generation.to_be_bytes());
    aad.push(0x1f);
    aad.extend_from_slice(&number.to_be_bytes());
    aad
}

/// One line sealed: its number in clear, then the sealed bytes, its end included.
pub(crate) fn seal_line(cipher: &XChaCha20Poly1305, bind: &str, generation: u64, number: u64, plain: &[u8]) -> std::io::Result<String> {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, Payload { msg: plain, aad: &line_bound(bind, generation, number) }).map_err(|_| std::io::Error::other("sealing"))?;
    let mut bytes = nonce.to_vec();
    bytes.extend(sealed);
    Ok(format!("{number} {}\n", B64.encode(bytes)))
}

/// A line's number, read in clear; none when it does not read.
pub(crate) fn line_number(line: &[u8]) -> Option<u64> {
    let space = line.iter().position(|&b| b == b' ')?;
    std::str::from_utf8(&line[..space]).ok()?.parse().ok()
}

/// One line opened (without its end): its number and its bytes; none when it does not open.
pub(crate) fn open_line(cipher: &XChaCha20Poly1305, bind: &str, generation: u64, line: &[u8]) -> Option<(u64, Vec<u8>)> {
    let number = line_number(line)?;
    let space = line.iter().position(|&b| b == b' ')?;
    let bytes = B64.decode(&line[space + 1..]).ok()?;
    if bytes.len() < AROUND {
        return None;
    }
    let (nonce, rest) = bytes.split_at(24);
    let plain = cipher.decrypt(XNonce::from_slice(nonce), Payload { msg: rest, aad: &line_bound(bind, generation, number) }).ok()?;
    Some((number, plain))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chacha20poly1305::aead::KeyInit;

    #[test]
    fn base32_is_rfc_4648_in_lower_case() {
        assert_eq!(base32(b""), "");
        assert_eq!(base32(b"f"), "my");
        assert_eq!(base32(b"foobar"), "mzxw6ytboi");
        assert_eq!(base32(&[0u8; 32]).len(), 52);
    }

    #[test]
    fn a_blob_opens_only_as_itself() {
        let cipher = XChaCha20Poly1305::new((&[1u8; 32]).into());
        let header = Header::new(Role::File, 0, 7);
        let inner = Inner { path: "config/a.toml".into(), written_ms: 5, origin: "test".into(), compressed: true };
        assert_eq!(inner.encode().len(), HEAD_PAD, "padded");
        assert_eq!(Inner { path: "x".repeat(400), ..inner.clone() }.encode().len(), HEAD_PAD, "the same length for a longer path");
        assert_eq!(Inner::decode(&inner.encode()), Some(inner.clone()));
        let content: Vec<u8> = (0..200_000u32).flat_map(|n| n.to_le_bytes()).collect();
        let mut sealed = Vec::new();
        let made = seal(&mut sealed, &cipher, "name-one", &header, &inner, &mut content.as_slice()).unwrap();
        assert_eq!(made.plain, content.len() as u64);
        assert_eq!(made.sealed_size, sealed.len() as u64);
        let expected = Expected { role: Role::File, copy: 0, generation: Some(7), path: Some("config/a.toml") };
        let mut out = Vec::new();
        let got = open(&mut sealed.as_slice(), &cipher, "name-one", &expected, &mut out, u64::MAX).unwrap();
        assert_eq!(out, content);
        assert_eq!(got.hash, made.hash);
        assert_eq!(got.inner, inner);
        assert!(matches!(open(&mut sealed.as_slice(), &cipher, "name-two", &expected, &mut Vec::new(), u64::MAX), Err(Fault::Broken(_))), "another name");
        let older = Expected { generation: Some(6), ..expected };
        assert!(matches!(open(&mut sealed.as_slice(), &cipher, "name-one", &older, &mut Vec::new(), u64::MAX), Err(Fault::Broken(_))), "another generation");
        let other = XChaCha20Poly1305::new((&[2u8; 32]).into());
        let expected = Expected { role: Role::File, copy: 0, generation: Some(7), path: None };
        assert!(matches!(open(&mut sealed.as_slice(), &other, "name-one", &expected, &mut Vec::new(), u64::MAX), Err(Fault::Broken(_))), "another key");
        // The clear header is bound too: a generation rewritten in clear does not open.
        let mut moved = sealed.clone();
        moved[27] ^= 1;
        let any = Expected { role: Role::File, copy: 0, generation: None, path: None };
        assert!(matches!(open(&mut moved.as_slice(), &cipher, "name-one", &any, &mut Vec::new(), u64::MAX), Err(Fault::Broken(_))));
        assert!(matches!(open(&mut sealed.as_slice(), &cipher, "name-one", &any, &mut Vec::new(), 1000), Err(Fault::Broken(_))), "past what is allowed");
        assert!(matches!(open(&mut [].as_slice(), &cipher, "name-one", &any, &mut Vec::new(), 1000), Err(Fault::Missing)));
        let (head, inner_read) = open_head(&mut sealed.as_slice(), &cipher, "name-one").unwrap();
        assert_eq!((head, inner_read), (header, inner));
    }

    #[test]
    fn a_line_opens_only_at_its_place() {
        let cipher = XChaCha20Poly1305::new((&[3u8; 32]).into());
        let line = seal_line(&cipher, "file", 2, 5, b"{\"a\":1}").unwrap();
        assert!(line.starts_with("5 "), "its number in clear");
        let line = line.trim_end().as_bytes();
        assert_eq!(open_line(&cipher, "file", 2, line), Some((5, b"{\"a\":1}".to_vec())));
        let renumbered = [b"6", &line[1..]].concat();
        assert!(open_line(&cipher, "file", 2, &renumbered).is_none(), "another number, said in clear: bound");
        assert!(open_line(&cipher, "other", 2, line).is_none(), "another file");
        assert!(open_line(&cipher, "file", 3, line).is_none(), "another generation");
        assert_eq!(parse_lines_header(lines_header(Role::Journal, 9).trim_end().as_bytes()).unwrap(), (Role::Journal, 9));
    }
}
