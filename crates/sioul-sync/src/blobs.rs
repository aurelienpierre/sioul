// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Notes and papers in the shared folder, one file at a time (docs/database.md,
//! "Notes and papers"). Each file is compressed (gzip), sealed in pieces
//! (XChaCha20-Poly1305), and kept once under a name made from its content:
//! `blobs/<name>`. A record in the log says which content each file holds
//! (`share`): a change sends that file alone, and a content in two places is
//! sent once. Written once, under a hidden name then renamed: any sync app
//! carries a new file. The folder's server sees how big each is, never its
//! name nor what it holds.
//!
//! Two keys of its own, made from the records' key (HKDF-SHA-256): one seals
//! the pieces, one names the files; the records' key itself seals nothing here.
//!
//! Never a whole file in memory: read, compressed and sealed a piece at a
//! time, opened and written back the same way beside its place, and checked
//! against its record before it takes that place.

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// Files larger than this stay on their device.
pub const LARGEST: u64 = 64 << 20;
/// A sealed file no record points to any more is removed by its writer after this long.
pub const UNUSED_DAYS: i64 = 90;
/// What is sealed at once.
const PIECE: usize = 1 << 20;
/// What is opened is inflated this much at a time: never far past `LARGEST`.
const INFLATED: usize = 64 << 10;
/// How a sealed file begins.
const MAGIC: &[u8] = b"sioul-blob 1\n";
/// In a piece's length: the last one, which holds nothing and says the file ends there.
const END: u32 = 1 << 31;
/// A nonce and a tag around each piece.
const AROUND: usize = 24 + 16;

/// Why a content could not be had from the folder.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// Not there yet: the sync app has not brought it, or only its empty placeholder.
    Missing,
    /// There, but it does not open whole, or opens to another content: damaged.
    Broken,
    /// It could not be written here (a folder not writable, a name a folder takes here).
    Local(String),
    /// No room for it here, and some left free (`disk::kept_free`).
    Room,
}

/// The keys of sealed files, made from the records' key for this format: one
/// seals the pieces, one names the files.
fn keys(key: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let made = hkdf::Hkdf::<Sha256>::new(None, key);
    let (mut sealing, mut naming) = ([0u8; 32], [0u8; 32]);
    made.expand(b"sioul blobs 1: sealing", &mut sealing).expect("32 bytes is a length HKDF gives");
    made.expand(b"sioul blobs 1: naming", &mut naming).expect("32 bytes is a length HKDF gives");
    (sealing, naming)
}

/// A content's name in the folder: made with a key, so that it tells nothing
/// of the content to whoever lacks it (not even that two folders hold the same file).
pub fn name(key: &[u8; 32], hash: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&keys(key).1).expect("HMAC takes any key");
    mac.update(hash.as_bytes());
    hex(&mac.finalize().into_bytes())
}

pub fn path(folder: &Path, key: &[u8; 32], hash: &str) -> PathBuf {
    folder.join("blobs").join(name(key, hash))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
thread_local! {
    /// Files read through for their hash, in this test's thread.
    pub(crate) static HASHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A file's content hash (SHA-256) and size, read a piece at a time.
pub fn hash_file(path: &Path) -> std::io::Result<(String, u64)> {
    #[cfg(test)]
    HASHED.with(|n| n.set(n.get() + 1));
    let mut reading = Hashing { inner: std::fs::File::open(path)?, hasher: Sha256::new(), size: 0 };
    std::io::copy(&mut reading, &mut std::io::sink())?;
    Ok((hex(&reading.hasher.finalize()), reading.size))
}

/// What tells one copy of a file in the folder from another: its size, its
/// time, and, where the system says, its place on the disk and when that
/// changed (a copy brought again whole is another, even of the same size and time).
pub fn fingerprint(meta: &std::fs::Metadata) -> (u64, u64, u64) {
    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64);
    #[cfg(unix)]
    let other = {
        use std::os::unix::fs::MetadataExt;
        meta.ino().wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (meta.ctime() as u64).wrapping_mul(1_000_000_000).wrapping_add(meta.ctime_nsec() as u64)
    };
    #[cfg(not(unix))]
    let other = meta.created().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as u64);
    (meta.len(), modified, other)
}

/// What passes through, hashed and counted.
struct Hashing<T> {
    inner: T,
    hasher: Sha256,
    size: u64,
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

/// What a piece is bound to: its file, its place, and whether it is the end.
fn bound(name: &str, index: u64, end: bool) -> String {
    format!("blob\u{1f}{name}\u{1f}{index}{}", if end { "\u{1f}end" } else { "" })
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

/// A file sealed into the folder, unless its content is there already (from
/// any device). `hash` is its content when gathered: read again here, a file
/// that changed meanwhile is not sent, the next look sends it. The size sealed
/// when written here; none when it was there. One there for long is made fresh:
/// its writer does not take it out from under the record that names it again.
pub fn put(folder: &Path, key: &[u8; 32], source: &Path, hash: &str) -> Result<Option<u64>, String> {
    let target = path(folder, key, hash);
    if let Ok(meta) = std::fs::metadata(&target)
        && meta.len() > 0
    {
        let old = meta.modified().ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() > 30 * 86_400);
        if old {
            let _ = std::fs::File::options().write(true).open(&target).and_then(|f| f.set_modified(std::time::SystemTime::now()));
        }
        return Ok(None);
    }
    seal_into(folder, key, source, hash).map(Some)
}

/// A content sealed again over a copy in the folder that is damaged or cut
/// (not of the size it was sealed at), or gone: the size sealed.
pub fn put_again(folder: &Path, key: &[u8; 32], source: &Path, hash: &str) -> Result<u64, String> {
    seal_into(folder, key, source, hash)
}

fn seal_into(folder: &Path, key: &[u8; 32], source: &Path, hash: &str) -> Result<u64, String> {
    let target = path(folder, key, hash);
    let name = name(key, hash);
    let dir = folder.join("blobs");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // A hidden name of its own: sync apps leave it alone until it is whole, and
    // two exchanges sealing one content never write into one file.
    let temporary = crate::share::temporary(&target);
    let sealed = (|| {
        let fail = |e: std::io::Error| format!("{}: {e}", temporary.display());
        let mut reading = Hashing { inner: std::fs::File::open(source).map_err(|e| format!("{}: {e}", source.display()))?, hasher: Sha256::new(), size: 0 };
        let mut packed = flate2::read::GzEncoder::new(&mut reading, flate2::Compression::default());
        let mut out = BufWriter::new(std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(fail)?);
        out.write_all(MAGIC).map_err(fail)?;
        let cipher = XChaCha20Poly1305::new((&keys(key).0).into());
        let mut piece = vec![0u8; PIECE];
        let mut index = 0;
        loop {
            let n = fill(&mut packed, &mut piece).map_err(|e| format!("{}: {e}", source.display()))?;
            if n == 0 {
                break;
            }
            if packed.get_ref().size > LARGEST {
                return Err(format!("{}: changed while it was sealed", source.display()));
            }
            write_piece(&mut out, &cipher, &name, index, false, &piece[..n]).map_err(fail)?;
            index += 1;
        }
        write_piece(&mut out, &cipher, &name, index, true, &[]).map_err(fail)?;
        drop(packed);
        if hex(&reading.hasher.finalize()) != hash {
            return Err(format!("{}: changed while it was sealed", source.display()));
        }
        let file = out.into_inner().map_err(|e| fail(e.into_error()))?;
        file.sync_all().map_err(fail)?;
        let size = file.metadata().map_err(fail)?.len();
        std::fs::rename(&temporary, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        Ok(size)
    })();
    if sealed.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    sealed
}

/// A piece sealed and written: its length (its top bit set for the end), its nonce, what is sealed.
fn write_piece(out: &mut impl Write, cipher: &XChaCha20Poly1305, name: &str, index: u64, end: bool, plain: &[u8]) -> std::io::Result<()> {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, Payload { msg: plain, aad: bound(name, index, end).as_bytes() }).map_err(|_| std::io::Error::other("sealing"))?;
    let length = (nonce.len() + sealed.len()) as u32 | if end { END } else { 0 };
    out.write_all(&length.to_be_bytes())?;
    out.write_all(&nonce)?;
    out.write_all(&sealed)
}

/// A content opened from the folder into `target` (a new hidden file beside
/// the one it will replace, `share::temporary`), and checked against its hash:
/// whole and right, or nothing (`target` removed). Never more than `LARGEST`
/// bytes written, whatever the file says.
pub fn get(folder: &Path, key: &[u8; 32], hash: &str, target: &Path) -> Result<(), Fault> {
    let opened = open_into(folder, key, hash, target);
    if opened.is_err() {
        let _ = std::fs::remove_file(target);
    }
    opened
}

fn open_into(folder: &Path, key: &[u8; 32], hash: &str, target: &Path) -> Result<(), Fault> {
    let name = name(key, hash);
    let Ok(file) = std::fs::File::open(folder.join("blobs").join(&name)) else { return Err(Fault::Missing) };
    // An empty file is a placeholder (files kept on demand), not a damaged one.
    if file.metadata().map_or(0, |m| m.len()) == 0 {
        return Err(Fault::Missing);
    }
    let mut reader = BufReader::new(file);
    let mut magic = [0u8; MAGIC.len()];
    if reader.read_exact(&mut magic).is_err() || magic != MAGIC {
        return Err(Fault::Broken);
    }
    let local = |e: std::io::Error| Fault::Local(format!("{}: {e}", target.display()));
    let out = BufWriter::new(std::fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(local)?);
    let mut unpacked = flate2::write::GzDecoder::new(Hashing { inner: out, hasher: Sha256::new(), size: 0 });
    let cipher = XChaCha20Poly1305::new((&keys(key).0).into());
    // A write that fails on bad compressed data, or on data past its end, is
    // a damaged file; on anything else, a problem here.
    let fault = |e: std::io::Error| if matches!(e.kind(), std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData | std::io::ErrorKind::WriteZero) { Fault::Broken } else { local(e) };
    let mut index = 0;
    loop {
        let mut length = [0u8; 4];
        // Cut short before its end: damaged (or still arriving, which a sync app never shows).
        reader.read_exact(&mut length).map_err(|_| Fault::Broken)?;
        let length = u32::from_be_bytes(length);
        let (end, length) = (length & END != 0, (length & !END) as usize);
        if !(AROUND..=PIECE + AROUND).contains(&length) {
            return Err(Fault::Broken);
        }
        let mut sealed = vec![0u8; length];
        reader.read_exact(&mut sealed).map_err(|_| Fault::Broken)?;
        let (nonce, body) = sealed.split_at(24);
        let plain = cipher.decrypt(XNonce::from_slice(nonce), Payload { msg: body, aad: bound(&name, index, end).as_bytes() }).map_err(|_| Fault::Broken)?;
        if end {
            break;
        }
        // Inflated a little at a time: a piece that inflates past the largest file is stopped there.
        for part in plain.chunks(INFLATED) {
            unpacked.write_all(part).map_err(fault)?;
            if unpacked.get_ref().size > LARGEST {
                return Err(Fault::Broken);
            }
        }
        index += 1;
    }
    // Nothing after the end.
    if reader.read(&mut [0u8; 1]).map_or(true, |n| n > 0) {
        return Err(Fault::Broken);
    }
    let Hashing { inner, hasher, .. } = unpacked.finish().map_err(fault)?;
    if hex(&hasher.finalize()) != hash {
        return Err(Fault::Broken);
    }
    inner.into_inner().map_err(|e| local(e.into_error()))?.sync_all().map_err(local)
}

/// The sealed files this device wrote that no record points to any more,
/// removed once that has lasted `UNUSED_DAYS`. `mine` holds when a record last
/// pointed to each (milliseconds), `used` the names records point to now, or
/// named lately. One whose file was made fresh lately (sealed again, named
/// again by another device) stays.
pub fn sweep(folder: &Path, mine: &mut BTreeMap<String, i64>, used: &BTreeSet<String>, now_ms: i64) {
    let unused = UNUSED_DAYS * 86_400_000;
    // Said used again at most once a day: the time counts in months, and the
    // memory holding it is written only when it changes (a computer's held
    // 25 MB, rewritten at every exchange for these times alone, 8 October 2026).
    let day = 86_400_000;
    mine.retain(|name, last| {
        if used.contains(name) {
            if now_ms - *last >= day {
                *last = now_ms;
            }
            return true;
        }
        if now_ms - *last < unused {
            return true;
        }
        let path = folder.join("blobs").join(name);
        let fresh = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| (age.as_millis() as i64) < unused);
        if fresh {
            *last = now_ms;
            return true;
        }
        let _ = std::fs::remove_file(path);
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sioul-blobs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn temporary(base: &Path) -> PathBuf {
        crate::share::temporary(&base.join("scan.pdf"))
    }

    #[test]
    fn sealed_whole_and_opened_whole() {
        let base = scratch("whole");
        let folder = base.join("vault");
        let key = [3u8; 32];
        // Larger than a piece, and hard to compress: several pieces.
        let mut bytes = Vec::new();
        let mut x = 7u32;
        while bytes.len() < 2 * PIECE + 1000 {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            bytes.extend_from_slice(&x.to_le_bytes());
        }
        let source = base.join("scan.pdf");
        std::fs::write(&source, &bytes).unwrap();
        let (hash, size) = hash_file(&source).unwrap();
        assert_eq!(size, bytes.len() as u64);
        let sealed_size = put(&folder, &key, &source, &hash).unwrap().unwrap();
        assert_eq!(std::fs::metadata(path(&folder, &key, &hash)).unwrap().len(), sealed_size);
        assert_eq!(put(&folder, &key, &source, &hash).unwrap(), None, "a content already there is not sealed again");
        let target = temporary(&base);
        get(&folder, &key, &hash, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        // Another key: nothing opens.
        let other = temporary(&base);
        assert_eq!(get(&folder, &[4u8; 32], &hash, &other), Err(Fault::Missing), "another key names it otherwise");
        let sealed = path(&folder, &key, &hash);
        std::fs::copy(&sealed, path(&folder, &[4u8; 32], &hash)).unwrap();
        assert_eq!(get(&folder, &[4u8; 32], &hash, &other), Err(Fault::Broken));
        assert!(!other.exists(), "nothing half written is left");
        // Damaged in the middle, cut short, longer than its end, emptied: never opened to something else.
        let whole = std::fs::read(&sealed).unwrap();
        let mut damaged = whole.clone();
        damaged[whole.len() / 2] ^= 0x40;
        for (bytes, fault) in [(damaged, Fault::Broken), (whole[..whole.len() - 50].to_vec(), Fault::Broken), ([whole.clone(), b"more".to_vec()].concat(), Fault::Broken), (Vec::new(), Fault::Missing)] {
            std::fs::write(&sealed, &bytes).unwrap();
            assert_eq!(get(&folder, &key, &hash, &temporary(&base)), Err(fault));
        }
        // Sealed again over a damaged copy: whole again.
        put_again(&folder, &key, &source, &hash).unwrap();
        get(&folder, &key, &hash, &temporary(&base)).unwrap();
        // The name says nothing of the content: not its hash, not what the records' key alone would make.
        assert!(!name(&key, &hash).contains(&hash[..16]));
        let mut plain = <Hmac<Sha256> as Mac>::new_from_slice(&key).unwrap();
        plain.update(hash.as_bytes());
        assert_ne!(name(&key, &hash), hex(&plain.finalize().into_bytes()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_bomb_stops_at_the_largest_file() {
        let base = scratch("bomb");
        let folder = base.join("vault");
        let key = [5u8; 32];
        // A key holder's file: pieces of zeros that inflate a thousand times.
        let zeros = vec![0u8; 3 * PIECE];
        let mut packed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        for _ in 0..30 {
            packed.write_all(&zeros).unwrap();
        }
        let packed = packed.finish().unwrap();
        let hash = "0".repeat(64);
        let name = name(&key, &hash);
        let cipher = XChaCha20Poly1305::new((&keys(&key).0).into());
        let mut out = MAGIC.to_vec();
        for (index, piece) in packed.chunks(PIECE).enumerate() {
            write_piece(&mut out, &cipher, &name, index as u64, false, piece).unwrap();
        }
        write_piece(&mut out, &cipher, &name, packed.chunks(PIECE).count() as u64, true, &[]).unwrap();
        std::fs::create_dir_all(folder.join("blobs")).unwrap();
        std::fs::write(folder.join("blobs").join(&name), &out).unwrap();
        let target = temporary(&base);
        assert_eq!(get(&folder, &key, &hash, &target), Err(Fault::Broken));
        assert!(!target.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_file_changed_while_sealed_is_not_sent() {
        let base = scratch("changed");
        let folder = base.join("vault");
        let source = base.join("note.md");
        std::fs::write(&source, "before").unwrap();
        let (hash, _) = hash_file(&source).unwrap();
        std::fs::write(&source, "after, longer").unwrap();
        assert!(put(&folder, &[1u8; 32], &source, &hash).is_err());
        assert!(!path(&folder, &[1u8; 32], &hash).exists());
        assert_eq!(std::fs::read_dir(folder.join("blobs")).unwrap().count(), 0, "no hidden file left behind");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn unused_ones_go_after_ninety_days() {
        let base = scratch("sweep");
        let folder = base.join("vault");
        std::fs::create_dir_all(folder.join("blobs")).unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(100 * 86_400);
        for name in ["used", "unused", "recent", "fresh"] {
            std::fs::write(folder.join("blobs").join(name), "x").unwrap();
            if name != "fresh" {
                std::fs::File::options().write(true).open(folder.join("blobs").join(name)).unwrap().set_modified(old).unwrap();
            }
        }
        let day = 86_400_000;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        let mut mine: BTreeMap<String, i64> = [("used".to_string(), now - 100 * day), ("unused".to_string(), now - 100 * day), ("recent".to_string(), now - 20 * day), ("fresh".to_string(), now - 100 * day)].into();
        sweep(&folder, &mut mine, &["used".to_string()].into(), now);
        assert!(folder.join("blobs/used").exists() && folder.join("blobs/recent").exists());
        assert!(folder.join("blobs/fresh").exists(), "sealed or named again lately: kept");
        assert!(!folder.join("blobs/unused").exists());
        assert_eq!(mine.get("used"), Some(&now));
        let _ = std::fs::remove_dir_all(&base);
    }
}
