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
//! - **An animated GIF** goes with its frames as they are, written again
//!   without its comments and other programs' blocks (XMP, a profile), when
//!   the message holds it (`gif_pictures_alone`), else is refused in words.
//! - **A video, a sound, a contact card, a calendar entry**, told by their
//!   first bytes too (`sniff_medium`), go as they are, with what their files
//!   hold, when the message holds them, else are refused in words.
//! - **Other files** are refused, whatever their names say: a phone may
//!   never open them. Send them by mail.
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
    /// An animated GIF: its frames as they are, when it fits.
    Animated,
    /// A video, a sound, a contact card or a calendar entry, as it is, when it fits: its content type.
    AsIs(&'static str),
    /// Refused, said why: "kind" (a picture this computer's Qt cannot read:
    /// HEIC on most), "file" (another file, whatever its name says).
    Refused(&'static str),
}

/// A video's, a sound's, a contact card's or a calendar entry's content
/// type by the file's first bytes (`head`), never by its name: MP4, MOV and
/// 3GP (ISO's boxes, their brands), WebM, AMR, AAC, MP3, Ogg, WAV, vCard,
/// vCalendar and iCalendar. The name only tells an MP4 of sound from one of
/// video, which share their brands. None for anything else (a PDF named
/// `.mp4` included).
pub(crate) fn sniff_medium(head: &[u8], name: &str) -> Option<&'static str> {
    let at = |from: usize, bytes: &[u8]| head.get(from..from + bytes.len()) == Some(bytes);
    if at(4, b"ftyp") {
        let brand = head.get(8..12)?;
        let sound = name.rsplit_once('.').is_some_and(|(_, e)| e.eq_ignore_ascii_case("m4a"));
        return match brand {
            b"qt  " => Some("video/quicktime"),
            b"M4A " | b"M4B " | b"M4P " => Some("audio/mp4"),
            b if [b"3gp".as_slice(), b"3ge", b"3gg", b"3gs", b"3gr", b"3g2"].iter().any(|start| b.starts_with(start)) => Some("video/3gpp"),
            b"isom" | b"iso2" | b"iso3" | b"iso4" | b"iso5" | b"iso6" | b"mp41" | b"mp42" | b"avc1" | b"M4V " | b"M4VH" | b"M4VP" | b"MSNV" | b"dash" | b"mmp4" => Some(if sound { "audio/mp4" } else { "video/mp4" }),
            _ => None,
        };
    }
    if at(0, &[0x1A, 0x45, 0xDF, 0xA3]) {
        return webm(head).then_some("video/webm");
    }
    if at(0, b"#!AMR") {
        return Some("audio/amr");
    }
    if at(0, b"OggS") {
        return Some("audio/ogg");
    }
    if at(0, b"RIFF") && at(8, b"WAVE") {
        return Some("audio/wav");
    }
    // An ID3 tag: its version (2 to 4) and its size in four bytes of seven bits each.
    if at(0, b"ID3") && head.get(3).is_some_and(|v| (2..=4).contains(v)) && head.get(6..10).is_some_and(|size| size.iter().all(|b| *b < 0x80)) {
        return Some("audio/mpeg");
    }
    // A frame, whole and valid, then the next frame's at the length its header gives: AAC's (ADTS),
    // MPEG audio's; never two bytes alone, which a text in UTF-16 begins with (FF FE: the review's L-d).
    let followed = |length: Option<usize>, next: &dyn Fn(&[u8]) -> Option<usize>| length.is_some_and(|length| head.get(length..).is_some_and(|rest| next(rest).is_some()));
    if followed(adts_frame(head), &adts_frame) {
        return Some("audio/aac");
    }
    if followed(mpeg_frame(head), &|rest: &[u8]| mpeg_frame(rest).filter(|_| rest.get(1).map(|b| b & 0xFE) == head.get(1).map(|b| b & 0xFE))) {
        return Some("audio/mpeg");
    }
    // Cards and calendar entries: text, perhaps after a byte-order mark and blank lines.
    let text = String::from_utf8_lossy(head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head)).trim_start().to_ascii_uppercase();
    if text.starts_with("BEGIN:VCARD") {
        return Some("text/x-vCard");
    }
    if text.starts_with("BEGIN:VCALENDAR") {
        return Some(if text.contains("VERSION:1.0") { "text/x-vCalendar" } else { "text/calendar" });
    }
    None
}

/// An MPEG audio frame's length by its header (four bytes): its version,
/// layer, bitrate, sampling rate and padding; none for what is no valid
/// header (a reserved version or layer, a free or bad bitrate, a reserved rate).
fn mpeg_frame(header: &[u8]) -> Option<usize> {
    let [first, second, third, _] = *header.get(..4)? else { return None };
    if first != 0xFF || second & 0xE0 != 0xE0 {
        return None;
    }
    // Version: 0 MPEG-2.5, 1 reserved, 2 MPEG-2, 3 MPEG-1. Layer: 0 reserved, 1 III, 2 II, 3 I.
    let (version, layer) = ((second >> 3) & 3, (second >> 1) & 3);
    let (bitrate, rate) = (usize::from(third >> 4), usize::from((third >> 2) & 3));
    if version == 1 || layer == 0 || bitrate == 0 || bitrate == 15 || rate == 3 {
        return None;
    }
    const MPEG1: [[u32; 15]; 3] = [
        [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448],
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384],
        [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320],
    ];
    const MPEG2: [[u32; 15]; 2] = [[0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256], [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160]];
    let kbps = match (version, layer) {
        (3, 3) => MPEG1[0][bitrate],
        (3, 2) => MPEG1[1][bitrate],
        (3, _) => MPEG1[2][bitrate],
        (_, 3) => MPEG2[0][bitrate],
        _ => MPEG2[1][bitrate],
    };
    let hertz = [[11_025u32, 12_000, 8_000], [0, 0, 0], [22_050, 24_000, 16_000], [44_100, 48_000, 32_000]][usize::from(version)][rate];
    let (bits, padding) = (kbps * 1000, u32::from((third >> 1) & 1));
    let length = match layer {
        3 => (12 * bits / hertz + padding) * 4,
        1 if version != 3 => 72 * bits / hertz + padding,
        _ => 144 * bits / hertz + padding,
    };
    Some(length as usize)
}

/// An AAC frame's length by its ADTS header (seven bytes): its sync, layer
/// 0, a sampling rate that exists; none else.
fn adts_frame(header: &[u8]) -> Option<usize> {
    let header = header.get(..7)?;
    if header[0] != 0xFF || header[1] & 0xF6 != 0xF0 || (header[2] >> 2) & 0x0F >= 13 {
        return None;
    }
    let length = (usize::from(header[3] & 0x03) << 11) | (usize::from(header[4]) << 3) | usize::from(header[5] >> 5);
    (length >= 7).then_some(length)
}

/// Whether a Matroska file is a WebM one, by its EBML header's DocType
/// element ("webm"), never by "webm" anywhere in its first bytes.
fn webm(head: &[u8]) -> bool {
    // A size: its length told by its first byte's leading zeros, that byte's marker left out.
    let size = |at: usize| -> Option<(usize, usize)> {
        let first = *head.get(at)?;
        let length = first.leading_zeros() as usize + 1;
        if length > 8 {
            return None;
        }
        let mut value = u64::from(first) & (0xFF >> length);
        for i in 1..length {
            value = (value << 8) | u64::from(*head.get(at + i)?);
        }
        Some((usize::try_from(value).ok()?, length))
    };
    let Some((header, n)) = size(4) else { return false };
    let end = (4 + n).saturating_add(header).min(head.len());
    let mut at = 4 + n;
    while at < end {
        // An element: its ID (one to four bytes, by its first byte's leading zeros), its size, its body.
        let id_length = head[at].leading_zeros() as usize + 1;
        let (Some(id), Some((length, n))) = (head.get(at..at + id_length).filter(|_| id_length <= 4), size(at + id_length)) else { return false };
        let body = at + id_length + n;
        if id == [0x42, 0x82] {
            return head.get(body..body.saturating_add(length)) == Some(b"webm");
        }
        at = body.saturating_add(length);
    }
    false
}

/// What a file is (decisions 7 to 9), by its first bytes alone: a picture
/// (`picture`, as `sniff` tells it), an animated GIF when it holds several
/// frames, refused where Qt here reads no such picture (`reads`); else a
/// video, a sound, a contact card or a calendar entry (`medium`, as
/// `sniff_medium` tells it); else another file, refused.
pub(crate) fn kind_of(picture: Option<&str>, reads: bool, frames: u32, medium: Option<&'static str>) -> Kind {
    match (picture, medium) {
        (Some(_), _) if !reads => Kind::Refused("kind"),
        (Some("gif"), _) if frames > 1 => Kind::Animated,
        (Some(_), _) => Kind::Picture,
        (None, Some(ct)) => Kind::AsIs(ct),
        (None, None) => Kind::Refused("file"),
    }
}

/// An animated GIF written again with its pictures alone (the review of 18
/// October 2026, L-1, and of its re-check, L-c): its header, its screen and
/// colour tables and each frame's image kept byte for byte; each frame's
/// timing (its graphic control) written as its own four bytes alone, its
/// loop (NETSCAPE2.0's or ANIMEXTS1.0's block) as its first sub-block alone
/// (`01` and the count), whatever else a crafted GIF puts in them; its
/// comments, plain-text blocks, other programs' blocks (XMP, a colour
/// profile, an editor's notes) and whatever follows its end left out; its
/// end written when it lacks one (Qt reads such a GIF). No frame is decoded:
/// each block says its own length. None for a GIF cut inside a block.
pub(crate) fn gif_pictures_alone(bytes: &[u8]) -> Option<Vec<u8>> {
    // Where a run of sub-blocks (a length, its bytes, until a length of 0) ends.
    let blocks_end = |mut at: usize| -> Option<usize> {
        loop {
            let length = usize::from(*bytes.get(at)?);
            at += 1;
            if length == 0 {
                return Some(at);
            }
            at += length;
            if at > bytes.len() {
                return None;
            }
        }
    };
    let table = |packed: u8| if packed & 0x80 != 0 { 3 * (1usize << ((packed & 0x07) + 1)) } else { 0 };
    let screen = bytes.get(..13)?;
    if !(screen.starts_with(b"GIF87a") || screen.starts_with(b"GIF89a")) {
        return None;
    }
    let mut at = 13 + table(screen[10]);
    let mut out = bytes.get(..at)?.to_vec();
    loop {
        // Its end, or none after its last whole block.
        let Some(&block) = bytes.get(at) else {
            out.push(0x3B);
            return Some(out);
        };
        match block {
            0x3B => {
                out.push(0x3B);
                return Some(out);
            }
            0x2C => {
                let descriptor = bytes.get(at..at + 10)?;
                // Its colour table, the LZW code size, then its data's sub-blocks.
                let end = blocks_end(at + 10 + table(descriptor[9]) + 1)?;
                out.extend_from_slice(&bytes[at..end]);
                at = end;
            }
            0x21 => {
                let label = *bytes.get(at + 1)?;
                let end = blocks_end(at + 2)?;
                // A frame's timing: its four bytes (disposal and transparency, delay, transparent colour) alone.
                if label == 0xF9 && bytes.get(at + 2) == Some(&4) {
                    out.extend_from_slice(&[0x21, 0xF9, 4]);
                    out.extend_from_slice(bytes.get(at + 3..at + 7)?);
                    out.push(0);
                }
                // The loop: its name and the count of its first sub-block (`03 01 lo hi`) alone.
                let looping = label == 0xFF && bytes.get(at + 2) == Some(&11) && matches!(bytes.get(at + 3..at + 14), Some(b"NETSCAPE2.0" | b"ANIMEXTS1.0"));
                if looping && bytes.get(at + 14..at + 16) == Some(&[3u8, 1][..]) {
                    out.extend_from_slice(bytes.get(at..at + 18)?);
                    out.push(0);
                }
                at = end;
            }
            _ => return None,
        }
    }
}

/// A medium's file's extension in Sioul's cache, by its content type (never the file chosen's).
fn extension_of(ct: &str) -> &'static str {
    match ct {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "video/mp4" => "mp4",
        "video/quicktime" => "mov",
        "video/3gpp" => "3gp",
        "video/webm" => "webm",
        "audio/mp4" => "m4a",
        "audio/amr" => "amr",
        "audio/aac" => "aac",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/wav" => "wav",
        "text/x-vCard" => "vcf",
        "text/x-vCalendar" => "vcs",
        "text/calendar" => "ics",
        _ => "dat",
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

/// A file's first bytes, enough for `sniff` and `sniff_medium`; none when it cannot be read.
pub(crate) fn head_of(path: &Path) -> Option<Vec<u8>> {
    let mut head = Vec::with_capacity(HEAD);
    std::fs::File::open(path).ok()?.take(HEAD as u64).read_to_end(&mut head).ok()?;
    Some(head)
}

/// How much of a file is read to tell what it is: an audio frame's next
/// frame with it (an AAC frame may be 8 KB).
const HEAD: usize = 16 * 1024;

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
        let medium = if sniffed.is_none() { sniff_medium(&head, &name) } else { None };
        match kind_of(sniffed, reads, picture.as_ref().map_or(0, |p| p.frames), medium) {
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
    // Yours alone, as Sioul's cache around it.
    let _ = std::fs::create_dir_all(folder);
    #[cfg(unix)]
    let _ = std::fs::set_permissions(folder, std::os::unix::fs::PermissionsExt::from_mode(0o700));
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
                let out = out_of(i, extension_of(ct));
                // As much as was weighed, no more: a file grown since is refused, not cut.
                let mut bytes = Vec::new();
                let read = std::fs::File::open(source).and_then(|from| from.take(size + 1).read_to_end(&mut bytes));
                // An animated GIF's frames alone: its comments and other programs' blocks left out.
                let bytes = if *kind == Kind::Animated { gif_pictures_alone(&bytes) } else { Some(bytes) };
                match (read, bytes) {
                    (Ok(length), _) if length as u64 != *size => left.push(refused(source, "too-large", *size, "")),
                    (Ok(_), Some(bytes)) if std::fs::write(&out, &bytes).is_ok() => made.push(Attached { source: shown, kind: if *kind == Kind::Animated { "picture".into() } else { kind_word(ct).into() }, ct: ct.into(), path: out.display().to_string(), size: bytes.len() as u64 }),
                    _ => {
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

    /// What a file is: by its bytes alone, never its name (decisions 7 to 9;
    /// the review of 18 October 2026, I-3).
    #[test]
    fn a_file_is_known_by_what_it_is() {
        assert_eq!(kind_of(Some("jpeg"), true, 1, None), Kind::Picture);
        assert_eq!(kind_of(Some("gif"), true, 1, None), Kind::Picture, "a still GIF, a picture");
        assert_eq!(kind_of(Some("gif"), true, 12, None), Kind::Animated);
        assert_eq!(kind_of(Some("heif"), false, 0, None), Kind::Refused("kind"), "a HEIC Qt cannot read here");
        assert_eq!(kind_of(Some("heif"), true, 1, None), Kind::Picture, "one Qt reads (KDE's plugins, Apple's)");
        assert_eq!(kind_of(None, false, 0, Some("video/mp4")), Kind::AsIs("video/mp4"));
        assert_eq!(kind_of(None, false, 0, None), Kind::Refused("file"));
        let medium = |head: &[u8], name: &str| sniff_medium(head, name);
        assert_eq!(medium(b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2", "Clip.MP4"), Some("video/mp4"));
        assert_eq!(medium(b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2", "voice.m4a"), Some("audio/mp4"), "a sound in an MP4");
        assert_eq!(medium(b"\0\0\0\x18ftypM4A \0\0\x02\0M4A mp42", "voice"), Some("audio/mp4"));
        assert_eq!(medium(b"\0\0\0\x14ftypqt  \0\0\x02\0qt  ", "IMG_0001.MOV"), Some("video/quicktime"));
        assert_eq!(medium(b"\0\0\0\x14ftyp3gp4\0\0\x02\03gp4", "v.3gp"), Some("video/3gpp"));
        assert_eq!(medium(b"\0\0\0\x18ftypheic\0\0\0\0mif1heic", "x.mp4"), None, "a picture's box is no video");
        assert_eq!(medium(b"\x1a\x45\xdf\xa3\x9f\x42\x86\x81\x01\x42\x82\x84webm", "clip.webm"), Some("video/webm"));
        assert_eq!(medium(b"\x1a\x45\xdf\xa3\x9f\x42\x86\x81\x01\x42\x82\x88matroska", "film.mkv"), None, "Matroska, not WebM");
        assert_eq!(medium(b"\x1a\x45\xdf\xa3\x8b\x42\x82\x88matroska\x15\x49\xa9\x66\x84webm", "film.webm"), None, "webm in a later element: its DocType says Matroska");
        assert_eq!(medium(b"#!AMR\n\x3c", "voice.amr"), Some("audio/amr"));
        assert_eq!(medium(b"OggS\0\x02", "song.opus"), Some("audio/ogg"));
        assert_eq!(medium(b"RIFF\x24\0\0\0WAVEfmt ", "a.wav"), Some("audio/wav"));
        assert_eq!(medium(b"ID3\x04\0\0\0\0\0\0", "a.mp3"), Some("audio/mpeg"));
        // A sound by whole frames, the next one's header where the first says (the review's re-check, L-d).
        let mp3 = [&[0xFF, 0xFB, 0x90, 0x64][..], &[0; 413], &[0xFF, 0xFB, 0x90, 0x64], &[0; 64]].concat();
        assert_eq!(mpeg_frame(&mp3), Some(417), "MPEG-1 Layer III, 128 kb/s, 44.1 kHz");
        assert_eq!(medium(&mp3, "a.mp3"), Some("audio/mpeg"));
        assert_eq!(medium(&mp3[..417], "a.mp3"), None, "one frame, its next not there");
        assert_eq!(medium(&[0xFF, 0xFB, 0x90, 0x64], "a.mp3"), None, "two bytes of sync are no sound");
        let adts = [&[0xFF, 0xF1, 0x50, 0x80, 0x0C, 0x9F, 0xFC][..], &[0; 93], &[0xFF, 0xF1, 0x50, 0x80, 0x0C, 0x9F, 0xFC], &[0; 93]].concat();
        assert_eq!(adts_frame(&adts), Some(100));
        assert_eq!(medium(&adts, "a.aac"), Some("audio/aac"));
        assert_eq!(medium(b"ID3\x04\0\0\x80\0\0\0", "a.mp3"), None, "an ID3 tag's size is seven bits a byte");
        // Text in UTF-16, as Windows writes it: its byte-order mark FF FE is no sound, nor a card.
        let utf16 = |text: &str| [&[0xFF, 0xFE][..], &text.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>()].concat();
        assert_eq!(medium(&utf16("Notes for Tuesday: the boiler, the rent, the dentist.\r\n".repeat(40).as_str()), "notes.txt"), None);
        assert_eq!(medium(&utf16(&format!("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Alice Example\r\nTEL:01 99 00 12 34\r\nEND:VCARD\r\n{}", " ".repeat(800))), "alice.vcf"), None);
        assert_eq!(medium(&utf16("Bonjour, à demain.".repeat(60).as_str()), "mot.txt"), None, "its first letter a valid frame's, no next frame");
        assert_eq!(medium(b"\xEF\xBB\xBF\r\nbegin:vcard\r\nVERSION:2.1\r\n", "alice.vcf"), Some("text/x-vCard"));
        assert_eq!(medium(b"BEGIN:VCALENDAR\r\nVERSION:1.0\r\n", "dinner.vcs"), Some("text/x-vCalendar"));
        assert_eq!(medium(b"BEGIN:VCALENDAR\r\nVERSION:2.0\r\n", "dinner.ics"), Some("text/calendar"));
        assert_eq!(medium(b"%PDF-1.7\n%\xe2\xe3", "lease.mp4"), None, "a PDF named .mp4: another file");
        assert_eq!(medium(b"<svg xmlns=", "drawing.mp3"), None);
        assert_eq!(medium(b"", "empty.mp4"), None);
        for ct in ["image/jpeg", "image/gif", "video/mp4", "video/quicktime", "video/3gpp", "video/webm", "audio/mp4", "audio/amr", "audio/aac", "audio/mpeg", "audio/ogg", "audio/wav", "text/x-vCard", "text/x-vCalendar", "text/calendar"] {
            assert!(rules::sendable(ct), "{ct}: the phone sends it");
            assert_ne!(extension_of(ct), "dat", "{ct}");
        }
        assert_eq!(format_words("heif"), "HEIC");
    }

    /// An animated GIF keeps its frames, their timing and its loop, and
    /// loses its comments, its plain text, other programs' blocks (XMP) and
    /// what follows its end (the review of 18 October 2026, L-1).
    #[test]
    fn an_animated_gif_goes_with_its_frames_alone() {
        let frame = |colour: u8| [&[0x21, 0xF9, 4, 0, 10, 0, 0, 0][..], &[0x2C, 0, 0, 0, 0, 2, 0, 1, 0, 0], &[2, 2, 0x4C | colour, 0x01, 0]].concat();
        let netscape = b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\0\0\0".to_vec();
        let comment = [&[0x21, 0xFE, 16][..], b"SIOUL-SECRET-GIF", &[0]].concat();
        let mut xmp = b"\x21\xFF\x0BXMP DataXMP".to_vec();
        xmp.extend_from_slice(b"<x:xmpmeta>SIOUL-SECRET-XMP</x:xmpmeta>");
        // XMP's magic trailer: 0x01, then 0xFF down to 0x00, then the end of the blocks.
        xmp.push(0x01);
        xmp.extend((0..=255u8).rev());
        xmp.push(0);
        let text = [&[0x21, 0x01, 12][..], &[0; 12], &[5][..], b"SECRE", &[0]].concat();
        let screen = [b"GIF89a".as_slice(), &[2, 0, 1, 0, 0x80, 0, 0], &[255, 0, 0, 0, 0, 255]].concat();
        let gif = [screen.clone(), netscape.clone(), comment, xmp, frame(0), text, frame(1), vec![0x3B], b"SIOUL-SECRET-AFTER".to_vec()].concat();
        let alone = gif_pictures_alone(&gif).unwrap();
        assert_eq!(alone, [screen, netscape, frame(0), frame(1), vec![0x3B]].concat());
        assert!(!alone.windows(5).any(|w| w == b"SIOUL" || w == b"SECRE"), "no comment, XMP, text or tail");
        assert_eq!(gif_pictures_alone(&alone).unwrap(), alone, "written again, the same");
        assert_eq!(gif_pictures_alone(&gif[..gif.len() - 30]), None, "cut short: not a whole GIF");
        // No end after its last whole block, which Qt reads: its end written (the re-check, I-h).
        let screen_again = [b"GIF89a".as_slice(), &[2, 0, 1, 0, 0x80, 0, 0], &[255, 0, 0, 0, 0, 255]].concat();
        let open = [screen_again.clone(), b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\0\0\0".to_vec(), frame(0), frame(1)].concat();
        assert_eq!(gif_pictures_alone(&open).unwrap(), [open.clone(), vec![0x3B]].concat());
        // A crafted GIF hides words inside a frame's timing and the loop block: their values alone go (L-c).
        let timing_with_more = [&[0x21, 0xF9, 4, 0, 10, 0, 0, 5][..], b"SECRE", &[0]].concat();
        let timing_too_long = [&[0x21, 0xF9, 20][..], b"SECRET-OF-TWENTY-B..", &[0]].concat();
        let loop_with_more = [b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\0\0".as_slice(), &[5], b"SECRE", &[0]].concat();
        let animexts_with_more = [b"\x21\xFF\x0BANIMEXTS1.0\x03\x01\x02\0".as_slice(), &[5], b"SECRE", &[0]].concat();
        let frame_of = |timing: &[u8], colour: u8| [timing, &[0x2C, 0, 0, 0, 0, 2, 0, 1, 0, 0], &[2, 2, 0x4C | colour, 0x01, 0]].concat();
        let crafted = [screen_again.clone(), loop_with_more, animexts_with_more, frame_of(&timing_with_more, 0), frame_of(&timing_too_long, 1), vec![0x3B]].concat();
        let alone = gif_pictures_alone(&crafted).unwrap();
        assert!(!alone.windows(5).any(|w| w == b"SECRE"), "{:?}", String::from_utf8_lossy(&alone));
        let image = |colour: u8| [&[0x2C, 0, 0, 0, 0, 2, 0, 1, 0, 0][..], &[2, 2, 0x4C | colour, 0x01, 0]].concat();
        let expected = [screen_again, b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\0\0\0".to_vec(), b"\x21\xFF\x0BANIMEXTS1.0\x03\x01\x02\0\0".to_vec(), vec![0x21, 0xF9, 4, 0, 10, 0, 0, 0], image(0), image(1), vec![0x3B]].concat();
        assert_eq!(alone, expected, "the timing's four bytes and the loop's count alone; a timing block of 20 bytes dropped");
        assert_eq!(gif_pictures_alone(b"GIF89a"), None);
        assert_eq!(gif_pictures_alone(b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR"), None);
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
