// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a computer attaches to a text, made fit for a multimedia message
//! (docs/texts.md, "Multimedia messages sent from a computer"; decisions 6
//! to 10), before anything travels: so the sharing, the server and the phone
//! only ever carry what will be sent.
//!
//! - **Pictures** (JPEG, PNG, WebP, TIFF, BMP, a still GIF; HEIC, AVIF and
//!   JPEG XL where Qt reads them), told by their first bytes (`sniff`), are
//!   always written again as a baseline JPEG (cpp/pictures.cpp), even one
//!   that fits: their orientation applied, in sRGB, laid on white, with no
//!   metadata at all (no GPS position, camera, time, comment or colour
//!   profile); the long side at most 1,600 pixels (and the carrier's advice),
//!   quality 85, then lowered to 50, then a quarter smaller each time, never
//!   under 480 pixels (`fit`). Only a picture's own reader decodes it: never
//!   a document Qt's plugins could also render (PDF, SVG, PostScript, a
//!   camera's raw file).
//! - **An animated GIF, a video, a sound, a contact card, a calendar entry**
//!   go as they are when the message holds them, else are refused in words.
//! - **Other files** are refused: a phone may never open them. Send them by mail.
//! - **Up to five media** a message (`texts::MEDIA_MAX`), the pictures
//!   sharing equally what the others leave (`share`), 16 KB each at least;
//!   what a picture going at its best leaves goes to those made smaller.
//!
//! The rules are pure and tested here; Qt only reads and writes the pictures,
//! in a thread of their own, never the window's (tests/attach.rs: no
//! metadata left, each format, the whole making).

use sioul_core::mms;
use sioul_core::texts as rules;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A picture's long side at most (OMA's guaranteed 1600 × 1200, decision 6).
pub(crate) const FIRST_SIDE: u32 = 1600;
/// …and at least: smaller than this, a picture is refused rather than sent unreadable.
pub(crate) const LAST_SIDE: u32 = 480;
const FIRST_QUALITY: u8 = 85;
const LAST_QUALITY: u8 = 50;
/// A picture's share at least (AOSP Messaging's `MIN_IMAGE_BYTE_SIZE`).
pub(crate) const MIN_SHARE: u64 = 16 * 1024;
/// What the words may take, kept out of the media's room as they are typed:
/// 1,600 characters of four bytes each.
pub(crate) const WORDS_RESERVE: usize = rules::WRITE_MAX * 4;

/// A picture's long side and quality that fit its share (decision 6):
/// first its long side at `first` (1,600 pixels, less as its carrier
/// advises, never more than its own) at quality 85; over its share, the
/// quality searched down to 50, the interval halved four times at most;
/// still over at 50, a quarter smaller and searched again; never under 480
/// pixels. `write` writes the picture at a long side and a quality and says
/// its size (none: not written). None: it cannot be made small enough.
pub(crate) fn fit(first: u32, share: u64, write: &mut dyn FnMut(u32, u8) -> Option<u64>) -> Option<(u32, u8)> {
    let fits = |size: Option<u64>| size.is_some_and(|size| size <= share);
    let mut side = first;
    loop {
        if fits(write(side, FIRST_QUALITY)) {
            return Some((side, FIRST_QUALITY));
        }
        if fits(write(side, LAST_QUALITY)) {
            let (mut good, mut over) = (LAST_QUALITY, FIRST_QUALITY);
            for _ in 0..4 {
                if over - good <= 1 {
                    break;
                }
                let middle = good + (over - good) / 2;
                if fits(write(side, middle)) {
                    good = middle;
                } else {
                    over = middle;
                }
            }
            return Some((side, good));
        }
        let smaller = (f64::from(side) * 0.75).round() as u32;
        if smaller < LAST_SIDE {
            return None;
        }
        side = smaller;
    }
}

/// Each picture's share of the room a message leaves for its media's bytes
/// (decision 10): the room less what goes as it is, shared equally, 16 KB
/// each at least; none when it cannot be so.
pub(crate) fn share(room: u64, fixed: u64, pictures: u64) -> Option<u64> {
    let left = room.checked_sub(fixed)?;
    if pictures == 0 {
        return Some(left);
    }
    let each = left / pictures;
    (each >= MIN_SHARE).then_some(each)
}

/// A picture's format by its first bytes, among those Sioul sends as a
/// JPEG: Qt's name for it ("jpeg", "png", "gif", "webp", "tiff", "bmp",
/// "heif", "avif", "jxl"); none for anything else, whatever its name says.
pub(crate) fn sniff(head: &[u8]) -> Option<&'static str> {
    let at = |from: usize, bytes: &[u8]| head.get(from..from + bytes.len()) == Some(bytes);
    if at(0, &[0xFF, 0xD8, 0xFF]) {
        return Some("jpeg");
    }
    if at(0, b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if at(0, b"GIF87a") || at(0, b"GIF89a") {
        return Some("gif");
    }
    if at(0, b"RIFF") && at(8, b"WEBP") {
        return Some("webp");
    }
    if at(0, b"II*\0") || at(0, b"MM\0*") {
        return Some("tiff");
    }
    if at(0, b"BM") && head.len() >= 26 {
        return Some("bmp");
    }
    if at(0, &[0xFF, 0x0A]) || at(0, b"\0\0\0\x0cJXL \r\n\x87\n") {
        return Some("jxl");
    }
    // ISO's media files: a box `ftyp`, its major brand, its version, its other brands.
    if at(4, b"ftyp") {
        let length = head.get(0..4).map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize).min(head.len());
        let brands: Vec<&[u8]> = std::iter::once(head.get(8..12).unwrap_or_default()).chain(head.get(16..length).unwrap_or_default().chunks_exact(4)).collect();
        if brands.iter().any(|b| matches!(*b, b"avif" | b"avis")) {
            return Some("avif");
        }
        if brands.iter().any(|b| matches!(*b, b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" | b"mif1" | b"msf1")) {
            return Some("heif");
        }
    }
    None
}

/// A picture's format in words, for "this computer cannot read … pictures".
pub(crate) fn format_words(format: &str) -> &'static str {
    match format {
        "jpeg" => "JPEG",
        "png" => "PNG",
        "gif" => "GIF",
        "webp" => "WebP",
        "tiff" => "TIFF",
        "bmp" => "BMP",
        "heif" => "HEIC",
        "avif" => "AVIF",
        "jxl" => "JPEG XL",
        _ => "",
    }
}

/// What a file attached is (decisions 7 to 9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A still picture: written again as a JPEG, made small.
    Picture,
    /// An animated GIF: as it is, when it fits.
    Animated,
    /// A video, a sound, a contact card or a calendar entry, as it is, when it fits: its content type.
    AsIs(&'static str),
    /// Refused, said why: "kind" (a picture this computer's Qt cannot read:
    /// HEIC on most), "unreadable" (a picture's name, no picture in it),
    /// "file" (another file).
    Refused(&'static str),
}

/// What a file is (decisions 7 to 9): by its first bytes, a picture
/// (`sniffed`, as `sniff` tells it), an animated GIF when it holds several
/// frames, refused where Qt here reads no such picture (`reads`); else by
/// its name's extension, a video, a sound, a contact card, a calendar entry;
/// refused, a picture's name with no picture in it, or another file.
pub(crate) fn kind_of(name: &str, sniffed: Option<&str>, reads: bool, frames: u32) -> Kind {
    match sniffed {
        Some(_) if !reads => return Kind::Refused("kind"),
        Some("gif") if frames > 1 => return Kind::Animated,
        Some(_) => return Kind::Picture,
        None => {}
    }
    let extension = name.rsplit_once('.').map_or(String::new(), |(_, e)| e.to_ascii_lowercase());
    match extension.as_str() {
        "mp4" | "m4v" => Kind::AsIs("video/mp4"),
        "3gp" | "3gpp" => Kind::AsIs("video/3gpp"),
        "webm" => Kind::AsIs("video/webm"),
        "mov" => Kind::AsIs("video/quicktime"),
        "amr" => Kind::AsIs("audio/amr"),
        "m4a" => Kind::AsIs("audio/mp4"),
        "aac" => Kind::AsIs("audio/aac"),
        "mp3" => Kind::AsIs("audio/mpeg"),
        "ogg" | "oga" => Kind::AsIs("audio/ogg"),
        "opus" => Kind::AsIs("audio/opus"),
        "wav" => Kind::AsIs("audio/wav"),
        "vcf" => Kind::AsIs("text/x-vCard"),
        "vcs" => Kind::AsIs("text/x-vCalendar"),
        "ics" => Kind::AsIs("text/calendar"),
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "tif" | "tiff" | "bmp" | "heic" | "heif" | "avif" | "jxl" => Kind::Refused("unreadable"),
        _ => Kind::Refused("file"),
    }
}

/// A picture decoded by Qt (cpp/pictures.cpp), let go when dropped. Used on
/// the thread that opened it.
#[cfg(not(target_os = "android"))]
pub(crate) struct Picture {
    picture: *mut std::ffi::c_void,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frames: u32,
}

#[cfg(not(target_os = "android"))]
unsafe extern "C" {
    // SAFETY: as cpp/pictures.cpp defines them: C strings in UTF-8.
    fn sioul_picture_reads(format: *const std::ffi::c_char) -> std::ffi::c_int;
    fn sioul_picture_open(path: *const std::ffi::c_char, format: *const std::ffi::c_char, largest: std::ffi::c_int, width: *mut std::ffi::c_int, height: *mut std::ffi::c_int, frames: *mut std::ffi::c_int) -> *mut std::ffi::c_void;
    fn sioul_picture_jpeg(picture: *const std::ffi::c_void, long_side: std::ffi::c_int, quality: std::ffi::c_int, out: *const std::ffi::c_char) -> std::ffi::c_longlong;
    fn sioul_picture_close(picture: *mut std::ffi::c_void);
}

#[cfg(not(target_os = "android"))]
impl Picture {
    /// Whether Qt reads pictures of `format` here (`sniff`'s names).
    pub(crate) fn reads(format: &str) -> bool {
        let Ok(format) = std::ffi::CString::new(format) else { return false };
        // SAFETY: a C string.
        unsafe { sioul_picture_reads(format.as_ptr()) != 0 }
    }

    /// The picture at `path`, decoded by the reader of `format` alone, its
    /// long side at `largest` or more when it is longer; none when it does not read.
    pub(crate) fn open(path: &Path, format: &str, largest: u32) -> Option<Picture> {
        let path = std::ffi::CString::new(path.to_str()?).ok()?;
        let format = std::ffi::CString::new(format).ok()?;
        let (mut width, mut height, mut frames) = (0, 0, 0);
        let largest = std::ffi::c_int::try_from(largest).unwrap_or(std::ffi::c_int::MAX);
        // SAFETY: two C strings, three places to write to.
        let picture = unsafe { sioul_picture_open(path.as_ptr(), format.as_ptr(), largest, &mut width, &mut height, &mut frames) };
        if picture.is_null() {
            return None;
        }
        Some(Picture { picture, width: u32::try_from(width).unwrap_or(0), height: u32::try_from(height).unwrap_or(0), frames: u32::try_from(frames).unwrap_or(0) })
    }

    /// Its long side.
    pub(crate) fn longest(&self) -> u32 {
        self.width.max(self.height)
    }

    /// Written to `out` as a baseline JPEG, no metadata, its long side made
    /// `long_side` when longer, at `quality`: its size, none when not written.
    pub(crate) fn jpeg(&self, long_side: u32, quality: u8, out: &Path) -> Option<u64> {
        let out = std::ffi::CString::new(out.to_str()?).ok()?;
        let long_side = std::ffi::c_int::try_from(long_side).unwrap_or(std::ffi::c_int::MAX);
        // SAFETY: the picture is this one's, alive; a C string.
        let size = unsafe { sioul_picture_jpeg(self.picture, long_side, std::ffi::c_int::from(quality), out.as_ptr()) };
        u64::try_from(size).ok()
    }
}

#[cfg(not(target_os = "android"))]
impl Drop for Picture {
    fn drop(&mut self) {
        // SAFETY: made by sioul_picture_open, let go once.
        unsafe { sioul_picture_close(self.picture) };
    }
}

/// A file's first bytes, enough for `sniff`; none when it cannot be read.
pub(crate) fn head_of(path: &Path) -> Option<Vec<u8>> {
    let mut head = Vec::with_capacity(64);
    std::fs::File::open(path).ok()?.take(64).read_to_end(&mut head).ok()?;
    Some(head)
}

/// A picture made fit for its share: written to `out` as `fit` says, from
/// `source`; its long side, its quality and its size. None: it cannot be
/// made small enough, or is no picture Qt reads here.
#[cfg(not(target_os = "android"))]
pub(crate) fn reduce(source: &Path, share: u64, advised: u32, out: &Path) -> Option<(u32, u8, u64)> {
    let format = sniff(&head_of(source)?)?;
    reduce_picture(&Picture::open(source, format, FIRST_SIDE)?, share, advised, out)
}

/// A picture's long side when it goes at its best: 1,600 pixels, less as
/// its carrier advises (`advised`, 0 none), never more than its own.
#[cfg(not(target_os = "android"))]
fn first_side(picture: &Picture, advised: u32) -> u32 {
    FIRST_SIDE.min(if advised > 0 { advised } else { FIRST_SIDE }).min(picture.longest()).max(1)
}

/// A picture decoded already, made fit for its share (`reduce`).
#[cfg(not(target_os = "android"))]
fn reduce_picture(picture: &Picture, share: u64, advised: u32, out: &Path) -> Option<(u32, u8, u64)> {
    let first = first_side(picture, advised);
    let trying = out.with_extension("trying.jpg");
    let found = fit(first, share, &mut |side, quality| picture.jpeg(side, quality, &trying));
    let _ = std::fs::remove_file(&trying);
    let (side, quality) = found?;
    let size = picture.jpeg(side, quality, out)?;
    (size <= share).then_some((side, quality, size))
}

/// One medium attached on the page, made fit: its file (in Sioul's cache,
/// yours alone, gone once sent or let go; named by when and its rank, never
/// by the file chosen), what it is, its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attached {
    /// The file chosen, as the page gave it: never sent.
    pub(crate) source: String,
    /// "picture", "video", "sound", "card".
    pub(crate) kind: String,
    pub(crate) ct: String,
    pub(crate) path: String,
    pub(crate) size: u64,
}

/// A file chosen that does not go, for the page to say why: its words' key
/// ("too-many", "kind", "unreadable", "file", "too-large", "gif-too-large",
/// "too-many-pictures", "picture-too-large"), its size, a picture's format
/// in words for "kind".
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Left {
    pub(crate) source: String,
    pub(crate) why: &'static str,
    pub(crate) size: u64,
    pub(crate) format: &'static str,
}

/// The kind of an as-is medium, for the page's words.
fn kind_word(ct: &str) -> &'static str {
    match ct.split('/').next().unwrap_or("") {
        "video" => "video",
        "audio" => "sound",
        _ => "card",
    }
}

/// A medium's content type, by its kind, once made.
fn ct_of(kind: &Kind) -> &'static str {
    match kind {
        Kind::Picture => "image/jpeg",
        Kind::Animated => "image/gif",
        Kind::AsIs(ct) => ct,
        Kind::Refused(_) => "",
    }
}

/// The bytes a message leaves for its media's own, its media of `cts`
/// (their content types) under `limit`: measured as the phone will write it
/// (`mms::room`), to the longest number, its words the most they may weigh
/// (`WORDS_RESERVE`), so that neither typing nor the number pushes it over.
fn room_for(cts: &[&str], limit: u64) -> u64 {
    let media: Vec<mms::Medium> = cts.iter().map(|ct| mms::Medium { ct, bytes: &[] }).collect();
    let reserve = "x".repeat(WORDS_RESERVE);
    let key = "0".repeat(32);
    let message = mms::Message { key: &key, to: &[mms::LONGEST_NUMBER], date: jiff::Timestamp::now().as_second(), words: &reserve, media: &media };
    mms::room(&message, usize::try_from(limit).unwrap_or(usize::MAX)).map_or(0, |room| room as u64)
}

/// The media chosen (`sources`, in their order) made fit for one message
/// under `limit` bytes (decisions 6 to 10), written into `folder`: those
/// that go, and those that do not and why. `advised`: the carrier's largest
/// picture's long side (0 none). The room is the message's own measure with
/// its media empty, to the longest number, with the words' reserve
/// (`room_for`). Only regular files are read; each picture is decoded once,
/// by its own format's reader.
#[cfg(not(target_os = "android"))]
pub(crate) fn make(sources: &[PathBuf], limit: u64, advised: u32, folder: &Path) -> (Vec<Attached>, Vec<Left>) {
    let mut left = Vec::new();
    // What may go: the file, what it is, its size, and a picture decoded at the size it may go at most.
    let mut chosen: Vec<(PathBuf, Kind, u64, Option<Picture>)> = Vec::new();
    let refused = |source: &Path, why: &'static str, size: u64, format: &'static str| Left { source: source.display().to_string(), why, size, format };
    for (i, source) in sources.iter().enumerate() {
        if i >= rules::MEDIA_MAX {
            left.push(refused(source, "too-many", 0, ""));
            continue;
        }
        let Some(size) = std::fs::metadata(source).ok().filter(std::fs::Metadata::is_file).map(|m| m.len()).filter(|&size| size > 0) else {
            left.push(refused(source, "unreadable", 0, ""));
            continue;
        };
        let Some(head) = head_of(source) else {
            left.push(refused(source, "unreadable", size, ""));
            continue;
        };
        let name = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let sniffed = sniff(&head);
        let reads = sniffed.is_some_and(Picture::reads);
        let picture = sniffed.filter(|_| reads).and_then(|format| Picture::open(source, format, FIRST_SIDE));
        if reads && picture.is_none() {
            left.push(refused(source, "unreadable", size, ""));
            continue;
        }
        match kind_of(&name, sniffed, reads, picture.as_ref().map_or(0, |p| p.frames)) {
            Kind::Refused(why) => left.push(refused(source, why, size, sniffed.map_or("", format_words))),
            Kind::Picture => chosen.push((source.clone(), Kind::Picture, size, picture)),
            kind => chosen.push((source.clone(), kind, size, None)),
        }
    }
    let mut keep: Vec<bool> = vec![true; chosen.len()];
    let room = |keep: &[bool]| room_for(&chosen.iter().zip(keep).filter(|(_, k)| **k).map(|((_, kind, _, _), _)| ct_of(kind)).collect::<Vec<_>>(), limit);
    // What goes as it is, in its order, while the room holds it: each one over it said so.
    let mut fixed = 0u64;
    for i in 0..chosen.len() {
        let (source, kind, size, _) = &chosen[i];
        if matches!(kind, Kind::AsIs(_) | Kind::Animated) {
            if fixed + size > room(&keep) {
                keep[i] = false;
                left.push(refused(source, if *kind == Kind::Animated { "gif-too-large" } else { "too-large" }, *size, ""));
            } else {
                fixed += size;
            }
        }
    }
    // The pictures share what is left; too many for their share, the last ones said so.
    let pictures = |keep: &[bool]| chosen.iter().zip(keep).filter(|((_, kind, _, _), k)| **k && *kind == Kind::Picture).count() as u64;
    while pictures(&keep) > 0 && share(room(&keep), fixed, pictures(&keep)).is_none() {
        let Some(last) = chosen.iter().zip(&keep).rposition(|((_, kind, _, _), k)| *k && *kind == Kind::Picture) else { break };
        keep[last] = false;
        left.push(refused(&chosen[last].0, "too-many-pictures", chosen[last].2, ""));
    }
    let each = share(room(&keep), fixed, pictures(&keep)).unwrap_or(0);
    let _ = std::fs::create_dir_all(folder);
    let stamp = jiff::Timestamp::now().as_millisecond();
    let out_of = |i: usize, extension: &str| folder.join(format!("{stamp}-{i}.{extension}"));
    // The pictures made for their equal shares (decision 10); then what those
    // that went at their best left unused, with the shares of those refused,
    // shared among those made smaller than their best, once.
    let mut reduced: Vec<Option<(u32, u8, u64)>> = vec![None; chosen.len()];
    let kept_pictures: Vec<usize> = (0..chosen.len()).filter(|&i| keep[i] && chosen[i].1 == Kind::Picture).collect();
    for &i in &kept_pictures {
        reduced[i] = chosen[i].3.as_ref().and_then(|picture| reduce_picture(picture, each, advised, &out_of(i, "jpg")));
    }
    let at_best = |i: usize| match (&chosen[i].3, reduced[i]) {
        (Some(picture), Some((side, quality, _))) => side == first_side(picture, advised) && quality == FIRST_QUALITY,
        _ => false,
    };
    let smaller: Vec<usize> = kept_pictures.iter().copied().filter(|&i| reduced[i].is_some() && !at_best(i)).collect();
    let unused: u64 = kept_pictures.iter().filter(|&&i| !smaller.contains(&i)).map(|&i| reduced[i].map_or(each, |(_, _, size)| each.saturating_sub(size))).sum();
    if !smaller.is_empty() && unused > 0 {
        let more = each + unused / smaller.len() as u64;
        for &i in &smaller {
            let again = out_of(i, "again.jpg");
            if let Some(better) = chosen[i].3.as_ref().and_then(|picture| reduce_picture(picture, more, advised, &again))
                && std::fs::rename(&again, out_of(i, "jpg")).is_ok()
            {
                reduced[i] = Some(better);
            }
            let _ = std::fs::remove_file(&again);
        }
    }
    let mut made = Vec::new();
    for (i, ((source, kind, size, _), k)) in chosen.iter().zip(&keep).enumerate() {
        if !k {
            continue;
        }
        let shown = source.display().to_string();
        match kind {
            Kind::Picture => match reduced[i] {
                Some((_, _, written)) => made.push(Attached { source: shown, kind: "picture".into(), ct: "image/jpeg".into(), path: out_of(i, "jpg").display().to_string(), size: written }),
                None => {
                    let _ = std::fs::remove_file(out_of(i, "jpg"));
                    left.push(refused(source, "picture-too-large", *size, ""));
                }
            },
            Kind::Animated | Kind::AsIs(_) => {
                let ct = ct_of(kind);
                let extension = source.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).filter(|e| !e.is_empty() && e.len() <= 8 && e.bytes().all(|b| b.is_ascii_alphanumeric())).unwrap_or_else(|| "dat".into());
                let out = out_of(i, &extension);
                // As much as was weighed, no more: a file grown since is refused, not cut.
                let copied = std::fs::File::open(source).and_then(|from| {
                    let mut to = std::fs::File::create(&out)?;
                    std::io::copy(&mut from.take(size + 1), &mut to)
                });
                match copied {
                    Ok(written) if written == *size => made.push(Attached { source: shown, kind: if *kind == Kind::Animated { "picture".into() } else { kind_word(ct).into() }, ct: ct.into(), path: out.display().to_string(), size: written }),
                    Ok(_) => {
                        let _ = std::fs::remove_file(&out);
                        left.push(refused(source, "too-large", *size, ""));
                    }
                    Err(_) => {
                        let _ = std::fs::remove_file(&out);
                        left.push(refused(source, "unreadable", *size, ""));
                    }
                }
            }
            Kind::Refused(_) => {}
        }
    }
    (made, left)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in of a picture's JPEG size: its pixels and its quality.
    fn sized(side: u32, quality: u8) -> u64 {
        let pixels = u64::from(side) * u64::from(side) * 3 / 4;
        pixels * u64::from(quality) / 200
    }

    /// The steps as decision 6 says them: 85 when it fits; else the quality
    /// searched down to 50; else a quarter smaller; never under 480.
    #[test]
    fn a_picture_is_made_small_in_its_steps() {
        let mut tried = Vec::new();
        let mut write = |side: u32, quality: u8| {
            tried.push((side, quality));
            Some(sized(side, quality))
        };
        assert_eq!(fit(1600, sized(1600, 85), &mut write), Some((1600, 85)), "fits at once");
        assert_eq!(tried, [(1600, 85)]);
        tried.clear();
        let mut write = |side: u32, quality: u8| {
            tried.push((side, quality));
            Some(sized(side, quality))
        };
        let (side, quality) = fit(1600, sized(1600, 70), &mut write).unwrap();
        assert_eq!(side, 1600);
        assert!((66..=70).contains(&quality), "the best quality found within four halvings: {quality}");
        assert!(tried.len() <= 6 && tried[..2] == [(1600, 85), (1600, 50)], "{tried:?}");
        tried.clear();
        let mut write = |side: u32, quality: u8| {
            tried.push((side, quality));
            Some(sized(side, quality))
        };
        let (side, _) = fit(1600, sized(900, 60), &mut write).unwrap();
        assert_eq!(side, 900, "a quarter smaller, twice: {tried:?}");
        assert!(tried.iter().all(|(side, _)| [1600, 1200, 900].contains(side)));
        let mut write = |side: u32, quality: u8| Some(sized(side, quality));
        assert_eq!(fit(1600, sized(480, 50) - 1, &mut write), None, "never under 480 pixels");
        assert!(fit(1600, sized(506, 50), &mut |side, quality| Some(sized(side, quality))).is_some_and(|(side, _)| side == 506), "1600, 1200, 900, 675, 506");
        assert_eq!(fit(1600, u64::MAX, &mut |_, _| None), None, "not written");
    }

    /// The pictures share what the others leave, 16 KB each at least.
    #[test]
    fn pictures_share_what_the_others_leave() {
        assert_eq!(share(600_000, 0, 1), Some(600_000));
        assert_eq!(share(600_000, 100_000, 5), Some(100_000));
        assert_eq!(share(600_000, 700_000, 1), None, "the others over the room");
        assert_eq!(share(80_000, 0, 5), None, "under 16 KB each");
        assert_eq!(share(80_000, 0, 4), Some(20_000));
        assert_eq!(share(5_000, 1_000, 0), Some(4_000), "no picture: what is left");
    }

    /// A picture is told by its first bytes, never by its name.
    #[test]
    fn a_picture_is_told_by_its_first_bytes() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]), Some("jpeg"));
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR"), Some("png"));
        assert_eq!(sniff(b"GIF89a\x01\0\x01\0"), Some("gif"));
        assert_eq!(sniff(b"RIFF\x24\0\0\0WEBPVP8L"), Some("webp"));
        assert_eq!(sniff(b"II*\0\x08\0\0\0"), Some("tiff"));
        assert_eq!(sniff(b"MM\0*\0\0\0\x08"), Some("tiff"));
        assert_eq!(sniff(b"BM\x46\0\0\0\0\0\0\0\x36\0\0\0\x28\0\0\0\x02\0\0\0\x02\0\0\0"), Some("bmp"));
        assert_eq!(sniff(b"BM"), None, "two letters are no bitmap");
        assert_eq!(sniff(&[0xFF, 0x0A, 0xFA]), Some("jxl"));
        assert_eq!(sniff(b"\0\0\0\x18ftypheic\0\0\0\0mif1heic"), Some("heif"));
        assert_eq!(sniff(b"\0\0\0\x1cftypmif1\0\0\0\0mif1avifmiaf"), Some("avif"), "an AVIF whose major brand is mif1");
        assert_eq!(sniff(b"\0\0\0\x18ftypavif\0\0\0\0avifmif1"), Some("avif"));
        assert_eq!(sniff(b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2"), None, "a video");
        assert_eq!(sniff(b"%PDF-1.7\n"), None);
        assert_eq!(sniff(b"<svg xmlns="), None);
        assert_eq!(sniff(b"%!PS-Adobe-3.0 EPSF"), None);
        assert_eq!(sniff(b""), None);
    }

    /// What a file is: a picture by its bytes, else by its name (decisions 7 to 9).
    #[test]
    fn a_file_is_known_by_what_it_is() {
        assert_eq!(kind_of("IMG_2026.JPG", Some("jpeg"), true, 1), Kind::Picture);
        assert_eq!(kind_of("scan.tiff", Some("tiff"), true, 1), Kind::Picture);
        assert_eq!(kind_of("still.gif", Some("gif"), true, 1), Kind::Picture, "a still GIF, a picture");
        assert_eq!(kind_of("dance.gif", Some("gif"), true, 12), Kind::Animated);
        assert_eq!(kind_of("named.pdf", Some("png"), true, 1), Kind::Picture, "its bytes, not its name");
        assert_eq!(kind_of("photo.heic", Some("heif"), false, 0), Kind::Refused("kind"), "a HEIC Qt cannot read here");
        assert_eq!(kind_of("photo.heic", Some("heif"), true, 1), Kind::Picture, "one Qt reads (KDE's plugins, Apple's)");
        assert_eq!(kind_of("Clip.MP4", None, false, 0), Kind::AsIs("video/mp4"));
        assert_eq!(kind_of("voice.amr", None, false, 0), Kind::AsIs("audio/amr"));
        assert_eq!(kind_of("alice.vcf", None, false, 0), Kind::AsIs("text/x-vCard"));
        assert_eq!(kind_of("dinner.ics", None, false, 0), Kind::AsIs("text/calendar"));
        assert_eq!(kind_of("broken.jpg", None, false, 0), Kind::Refused("unreadable"), "a picture's name, no picture in it");
        assert_eq!(kind_of("lease.pdf", None, false, 0), Kind::Refused("file"));
        assert_eq!(kind_of("drawing.svg", None, false, 0), Kind::Refused("file"), "a document Qt could render: never opened");
        assert_eq!(kind_of("noextension", None, false, 0), Kind::Refused("file"));
        for ct in ["image/jpeg", "image/gif", "video/mp4", "video/quicktime", "audio/amr", "audio/wav", "text/x-vCard", "text/x-vCalendar", "text/calendar"] {
            assert!(rules::sendable(ct), "{ct}: the phone sends it");
        }
        assert_eq!(format_words("heif"), "HEIC");
    }

    /// The room is measured to the longest number with the words' most:
    /// a message whose media fill it fits, with any number and any words.
    #[test]
    fn the_room_holds_any_number_and_any_words() {
        let limit = 614_400;
        let cts = ["image/jpeg", "image/jpeg", "video/mp4"];
        let room = room_for(&cts, limit) as usize;
        let bytes: Vec<Vec<u8>> = vec![vec![1; room / 3], vec![2; room / 3], vec![3; room - 2 * (room / 3)]];
        let media: Vec<mms::Medium> = cts.iter().zip(&bytes).map(|(ct, bytes)| mms::Medium { ct, bytes }).collect();
        let key = "1".repeat(32);
        let longest_words = "😀".repeat(rules::WRITE_MAX);
        for words in ["", "À demain.", longest_words.as_str()] {
            for to in ["0199001234", "+33199001234", mms::LONGEST_NUMBER] {
                let message = mms::Message { key: &key, to: &[to], date: 1_791_446_400, words, media: &media };
                let size = mms::size(&message).unwrap();
                assert!(size as u64 <= limit, "{to}, {} characters: {size}", words.chars().count());
            }
        }
        assert_eq!(room_for(&cts, 1_000), 0, "a limit the words' reserve fills leaves none");
    }
}
