// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a computer attaches to a text, made fit for a multimedia message
//! (src/attach.rs, cpp/pictures.cpp), with Qt itself. A picture comes out a
//! baseline JPEG that holds its pixels and nothing else, whatever it held:
//! no EXIF (no GPS position, camera or time), no XMP, no IPTC, no comment, no
//! PNG text, no colour profile; turned the way its EXIF says; turned into
//! sRGB from its own profile; laid on white where it was transparent. Each
//! format Sioul turns into a JPEG (JPEG, PNG, WebP, TIFF, BMP, a still GIF)
//! where this computer's Qt reads it, else refused in words; HEIC where Qt
//! reads it, else refused. An animated GIF goes as it is. The making of a
//! whole message: five media at most, the pictures sharing what the others
//! leave, each small enough, the message then within its limit with the
//! most words and the longest number; the refusals; the files made named by
//! when and rank, never by the file chosen.
//!
//! Every picture here is made by the test itself, byte by byte, its
//! metadata made up (a GPS position in the Atlantic), but the WebP, which
//! Pillow made (`WEBP`). The profile is one whose red and blue are sRGB's
//! blue and red: a picture red in it is blue once turned into sRGB, which
//! nothing else could make it.
//!
//! Its own `main` (harness = false): Qt wants its application on the
//! program's first thread. Run it alone: `cargo test --release -p sioul-app
//! --test attach`. With SIOUL_ATTACH_SAMPLES naming a folder, each file in
//! it is made fit too and checked the same way (a HEIC from a phone, say),
//! nothing of it printed but its size and what it became.

use std::path::{Path, PathBuf};

/// Made-up words in every field of metadata a picture here carries: none may come out.
const SECRET: &str = "SIOUL-SECRET";
/// What else no picture made may hold: EXIF's and XMP's names, Photoshop's
/// IPTC, a profile's header, the time written in the EXIF and the TIFF.
const NEVER: [&str; 7] = ["Exif", "http://ns.adobe.com", "Photoshop", "8BIM", "ICC_PROFILE", "2026:10:18", "acsp"];

fn main() {
    let folder = std::env::temp_dir().join(format!("sioul-attach-test-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a folder for the test's files");
    // SAFETY: nothing else runs yet: no thread of this program reads the environment.
    unsafe {
        std::env::set_var("QT_QPA_PLATFORM", "offscreen");
    }
    let application = cxx_qt_lib::QGuiApplication::new();
    assert!(!application.is_null(), "Qt's application, offscreen");

    let checks: [(&str, fn(&Path) -> Result<String, String>); 9] = [
        ("a camera's JPEG", camera_jpeg),
        ("a PNG with texts, EXIF and transparency", png_with_texts),
        ("a TIFF with its description and GPS", tiff_with_gps),
        ("a WebP with EXIF, XMP and a profile", webp_with_metadata),
        ("a BMP and a still GIF", bmp_and_still_gif),
        ("an animated GIF, its frames alone", animated_gif),
        ("a HEIC where Qt reads it", heic),
        ("a large picture made small, or refused", large_picture),
        ("a whole message", whole_message),
    ];
    let mut failed = Vec::new();
    for (name, check) in checks {
        let here = folder.join(name.replace(' ', "-").replace(',', ""));
        std::fs::create_dir_all(&here).expect("a folder for the check");
        match check(&here) {
            Ok(said) => println!("ok: {name}{}", if said.is_empty() { String::new() } else { format!(" ({said})") }),
            Err(why) => {
                println!("FAILED: {name}: {why}");
                failed.push(name);
            }
        }
    }
    if let Ok(samples) = std::env::var("SIOUL_ATTACH_SAMPLES") {
        let here = folder.join("samples");
        std::fs::create_dir_all(&here).expect("a folder for the samples");
        for (n, entry) in std::fs::read_dir(&samples).expect("SIOUL_ATTACH_SAMPLES, a folder").filter_map(Result::ok).enumerate() {
            match sample(&entry.path(), &here.join(format!("{n}.jpg"))) {
                Ok(said) => println!("ok: sample {n} ({said})"),
                Err(why) => {
                    println!("FAILED: sample {n}: {why}");
                    failed.push("a sample");
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&folder);
    if !failed.is_empty() {
        eprintln!("attach: {} of {} failed", failed.len(), checks.len());
        std::process::exit(1);
    }
    println!("attach: {} passed", checks.len());
}

// ---------------------------------------------------------------- what comes out

/// A JPEG holds its pixels and nothing else: SOI, JFIF's APP0, then only
/// the tables, a baseline frame (SOF0) and one scan; no APP1 (EXIF, XMP),
/// APP2 (a profile), APP13 (IPTC), COM or any other marker; none of the
/// made-up words, nor EXIF's, XMP's, IPTC's or a profile's names, in the
/// segments before the scan, where metadata lives (the scan's own bytes may
/// hold any four letters by chance).
fn clean(bytes: &[u8]) -> Result<(), String> {
    if !bytes.starts_with(&[0xFF, 0xD8]) || !bytes.ends_with(&[0xFF, 0xD9]) {
        return Err("not a JPEG".into());
    }
    let mut at = 2;
    let mut frames = Vec::new();
    let header = loop {
        let (Some(&0xFF), Some(&marker), Some(high), Some(low)) = (bytes.get(at), bytes.get(at + 1), bytes.get(at + 2), bytes.get(at + 3)) else { return Err(format!("no marker at {at}")) };
        let length = usize::from(u16::from_be_bytes([*high, *low]));
        let Some(body) = bytes.get(at + 4..at + 2 + length) else { return Err(format!("a segment cut short at {at}")) };
        match marker {
            0xE0 if body.starts_with(b"JFIF\0") => {}
            0xDB | 0xC4 | 0xDD => {}
            0xC0 => frames.push(marker),
            0xDA => break &bytes[..at + 2 + length],
            other => return Err(format!("a marker FF{other:02X} of {length} bytes")),
        }
        at += 2 + length;
    };
    if frames != [0xC0] {
        return Err(format!("frames {frames:02X?}: not one baseline frame"));
    }
    // In the scan, a 0xFF byte is followed by 0 or a restart marker: another marker only at its end.
    let markers = bytes[header.len()..bytes.len() - 2].windows(2).filter(|w| w[0] == 0xFF && w[1] != 0 && !(0xD0..=0xD7).contains(&w[1])).count();
    if markers != 0 {
        return Err(format!("{markers} markers after the scan began: not a baseline JPEG's one scan"));
    }
    let text = String::from_utf8_lossy(header);
    if let Some(word) = std::iter::once(SECRET).chain(NEVER).find(|word| text.contains(word)) {
        return Err(format!("\"{word}\" left in it"));
    }
    Ok(())
}

/// The picture made, decoded: its width, height, and the colour at a place.
fn decoded(path: &Path) -> Result<(i32, i32, cxx_qt_lib::QImage), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let image = cxx_qt_lib::QImage::from_data(&bytes, Some("JPEG")).ok_or("Qt does not read it back")?;
    Ok((image.width(), image.height(), image))
}

/// The colour at (x, y), within `by` of `want` on each channel.
fn near(image: &cxx_qt_lib::QImage, x: i32, y: i32, want: [i32; 3], by: i32) -> Result<(), String> {
    let colour = image.pixel_color(x, y);
    let got = [colour.red(), colour.green(), colour.blue()];
    if got.iter().zip(want).all(|(g, w)| (g - w).abs() <= by) { Ok(()) } else { Err(format!("({x}, {y}) is {got:?}, not about {want:?}")) }
}

/// A picture made fit, as the page's message would make it, whole and clean.
fn made_clean(source: &Path, out: &Path) -> Result<(u32, u8, u64), String> {
    let made = sioul_app::picture_reduced(source, 600_000, 0, out).ok_or("not made")?;
    clean(&std::fs::read(out).map_err(|e| e.to_string())?)?;
    Ok(made)
}

// ---------------------------------------------------------------- the checks

/// A camera's JPEG, its EXIF turning it a quarter (orientation 6), a GPS
/// position, maker, model and time; XMP, IPTC, a comment, the swapped
/// profile: out, none of it; turned; its red blue.
fn camera_jpeg(here: &Path) -> Result<String, String> {
    // A JPEG of Qt's own first, 64 × 32, its left half red, its right half green.
    let plain = here.join("plain.bmp");
    std::fs::write(&plain, bmp(64, 32, |x, _| if x < 32 { [255, 0, 0] } else { [0, 255, 0] })).map_err(|e| e.to_string())?;
    let first = here.join("first.jpg");
    made_clean(&plain, &first)?;
    let camera = here.join("IMG_0001.JPG");
    std::fs::write(&camera, with_metadata(&std::fs::read(&first).map_err(|e| e.to_string())?)?).map_err(|e| e.to_string())?;
    let (format, reads, read) = sioul_app::picture_read(&camera);
    if format != "jpeg" || !reads || read.map(|(w, h, _)| (w, h)) != Some((32, 64)) {
        return Err(format!("read as {format} ({reads}), {read:?}: not turned by its EXIF"));
    }
    let out = here.join("made.jpg");
    let (side, quality, size) = made_clean(&camera, &out)?;
    let (width, height, image) = decoded(&out)?;
    if (width, height) != (32, 64) {
        return Err(format!("{width} × {height}: not turned"));
    }
    // Its stored left, red in the swapped profile, is at the top, blue in sRGB.
    near(&image, 16, 12, [0, 0, 255], 24)?;
    near(&image, 16, 52, [0, 255, 0], 24)?;
    Ok(format!("{side} px, quality {quality}, {size} bytes"))
}

/// A PNG, its left half transparent, with texts, XMP and EXIF: out, a JPEG
/// with none, its transparent half white.
fn png_with_texts(here: &Path) -> Result<String, String> {
    let exif = chunk(b"eXIf", &exif_tiff(1));
    let comment = chunk(b"tEXt", b"Comment\0SIOUL-SECRET-PNG-COMMENT");
    let author = chunk(b"tEXt", b"Author\0SIOUL-SECRET-PNG-AUTHOR");
    let xmp = chunk(b"iTXt", &[b"XML:com.adobe.xmp\0\0\0\0\0".as_slice(), xmp().as_bytes()].concat());
    let source = here.join("drawing.png");
    std::fs::write(&source, png(40, 20, |x, _| if x < 20 { [0, 0, 0, 0] } else { [255, 0, 0, 255] }, &[exif, comment, author, xmp])).map_err(|e| e.to_string())?;
    let out = here.join("made.jpg");
    let (side, _, size) = made_clean(&source, &out)?;
    let (width, height, image) = decoded(&out)?;
    if (width, height) != (40, 20) {
        return Err(format!("{width} × {height}"));
    }
    near(&image, 6, 10, [255, 255, 255], 12)?;
    near(&image, 34, 10, [255, 0, 0], 24)?;
    Ok(format!("{side} px, {size} bytes"))
}

/// A TIFF with a description, a maker, a time and a GPS position: out, none.
fn tiff_with_gps(here: &Path) -> Result<String, String> {
    let source = here.join("scan.tif");
    std::fs::write(&source, tiff(30, 20, |x, _| if x < 15 { [0, 0, 255] } else { [255, 255, 0] })).map_err(|e| e.to_string())?;
    format_made("tiff", &source, here, (30, 20), [(7, 10, [0, 0, 255]), (22, 10, [255, 255, 0])])
}

/// The WebP Pillow made, with EXIF, XMP and the swapped profile: out, none, its red blue.
fn webp_with_metadata(here: &Path) -> Result<String, String> {
    let source = here.join("shared.webp");
    std::fs::write(&source, unhex(WEBP)).map_err(|e| e.to_string())?;
    format_made("webp", &source, here, (24, 16), [(5, 8, [0, 0, 255]), (18, 8, [0, 255, 0])])
}

/// A picture of `format` made a clean JPEG of its size and colours where
/// this computer's Qt reads such pictures; else refused in words ("kind").
fn format_made(format: &str, source: &Path, here: &Path, size: (i32, i32), colours: [(i32, i32, [i32; 3]); 2]) -> Result<String, String> {
    let (found, reads, _) = sioul_app::picture_read(source);
    if found != format {
        return Err(format!("told as \"{found}\""));
    }
    if !reads {
        let (made, left) = sioul_app::attach_made(&[source.to_path_buf()], 600_000, 0, &here.join("made"));
        return if made.is_empty() && left.iter().map(|(_, why)| why.as_str()).eq(["kind"]) { Ok(format!("Qt here reads no {format}: refused in words")) } else { Err(format!("Qt reads no {format} here, yet {made:?} {left:?}")) };
    }
    let out = here.join("made.jpg");
    let (side, _, written) = made_clean(source, &out)?;
    let (width, height, image) = decoded(&out)?;
    if (width, height) != size {
        return Err(format!("{width} × {height}"));
    }
    for (x, y, want) in colours {
        near(&image, x, y, want, 24)?;
    }
    Ok(format!("{side} px, {written} bytes"))
}

/// A BMP and a GIF of one frame: pictures, made JPEGs.
fn bmp_and_still_gif(here: &Path) -> Result<String, String> {
    let bitmap = here.join("old.bmp");
    std::fs::write(&bitmap, bmp(50, 30, |_, y| if y < 15 { [255, 255, 0] } else { [0, 0, 255] })).map_err(|e| e.to_string())?;
    format_made("bmp", &bitmap, here, (50, 30), [(25, 6, [255, 255, 0]), (25, 24, [0, 0, 255])])?;
    let still = here.join("still.gif");
    std::fs::write(&still, gif(20, 10, &[vec![0; 200]])).map_err(|e| e.to_string())?;
    let (_, _, read) = sioul_app::picture_read(&still);
    if read.map(|(_, _, frames)| frames) != Some(1) {
        return Err(format!("the still GIF read as {read:?}"));
    }
    format_made("gif", &still, here, (20, 10), [(4, 5, [255, 0, 0]), (15, 5, [255, 0, 0])])
}

/// An animated GIF goes as it is: its very bytes, as a GIF; too large, refused.
fn animated_gif(here: &Path) -> Result<String, String> {
    let source = here.join("dance.gif");
    let frame = |n: usize| (0..120 * 120).map(|p| ((p / 120 + p % 120 + n) % 4) as u8).collect::<Vec<u8>>();
    let bytes = gif(120, 120, &[frame(0), frame(1), frame(2)]);
    // A comment and XMP after its loop block, and words after its end, as an editor may leave them (the review's L-1).
    let comment = [&[0x21, 0xFE, 20][..], b"SIOUL-SECRET-COMMENT", &[0]].concat();
    let mut xmp = [&[0x21, 0xFF, 0x0B][..], b"XMP DataXMP", xmp().as_bytes(), &[0x01]].concat();
    xmp.extend((0..=255u8).rev());
    xmp.push(0);
    // And, as a crafted GIF could, words in a second sub-block of its loop block and of its first frame's timing
    // (the re-check's L-c): their values alone go.
    let (screen, looping, timing) = (6 + 7 + 12, 6 + 7 + 12 + 19, 6 + 7 + 12 + 19 + 8);
    let crafted_loop = [&bytes[screen..looping - 1], &[12], b"SIOUL-SECRET", &[0]].concat();
    let crafted_timing = [&bytes[looping..timing - 1], &[12], b"SIOUL-SECRET", &[0]].concat();
    let edited = [&bytes[..screen], &crafted_loop, &comment, &xmp, &crafted_timing, &bytes[timing..], b"SIOUL-SECRET-AFTER"].concat();
    std::fs::write(&source, &edited).map_err(|e| e.to_string())?;
    let (_, _, read) = sioul_app::picture_read(&source);
    if read.map(|(w, h, frames)| (w, h, frames)) != Some((120, 120, 3)) {
        return Err(format!("read as {read:?}"));
    }
    let (made, left) = sioul_app::attach_made(&[source.clone()], 300 * 1024, 0, &here.join("made"));
    let [(kind, ct, path, size)] = made.as_slice() else { return Err(format!("{made:?} {left:?}")) };
    let written = std::fs::read(path).map_err(|e| e.to_string())?;
    if (kind.as_str(), ct.as_str(), *size) != ("picture", "image/gif", bytes.len() as u64) || written != bytes {
        return Err(format!("{kind} {ct} {size}: not its frames alone, byte for byte"));
    }
    if String::from_utf8_lossy(&written).contains(SECRET) {
        return Err("a comment, XMP or a tail left in it".into());
    }
    let (_, _, again) = sioul_app::picture_read(Path::new(path));
    if again.map(|(_, _, frames)| frames) != Some(3) {
        return Err(format!("written again, read as {again:?}"));
    }
    let (made, left) = sioul_app::attach_made(&[source], 20 * 1024, 0, &here.join("small"));
    if !made.is_empty() || !left.iter().map(|(_, why)| why.as_str()).eq(["gif-too-large"]) {
        return Err(format!("{} bytes over a 20 kB message: {made:?} {left:?}", bytes.len()));
    }
    Ok(format!("{} bytes", bytes.len()))
}

/// A HEIC: where Qt reads HEIF pictures, a file that only begins as one is
/// unreadable; where it reads none, refused in words ("kind").
fn heic(here: &Path) -> Result<String, String> {
    let source = here.join("IMG_0002.HEIC");
    std::fs::write(&source, [b"\0\0\0\x18ftypheic\0\0\0\0mif1heic".as_slice(), &[7u8; 64]].concat()).map_err(|e| e.to_string())?;
    let (format, reads, _) = sioul_app::picture_read(&source);
    if format != "heif" {
        return Err(format!("told as \"{format}\""));
    }
    let (made, left) = sioul_app::attach_made(&[source], 600_000, 0, &here.join("made"));
    let want = if reads { "unreadable" } else { "kind" };
    if made.is_empty() && left.iter().map(|(_, why)| why.as_str()).eq([want]) { Ok(format!("Qt here reads HEIF: {reads}")) } else { Err(format!("{made:?} {left:?}")) }
}

/// A large picture of noise, which compresses worst: made small for a
/// share, quality first, then size; for too small a share, refused.
fn large_picture(here: &Path) -> Result<String, String> {
    let source = here.join("noise.bmp");
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let pixels: Vec<[u8; 3]> = (0..2400 * 1800).map(|_| noise().to_le_bytes()[..3].try_into().unwrap_or([0; 3])).collect();
    std::fs::write(&source, bmp(2400, 1800, |x, y| pixels[(y * 2400 + x) as usize])).map_err(|e| e.to_string())?;
    let (_, _, read) = sioul_app::picture_read(&source);
    if read.map(|(w, h, _)| (w, h)) != Some((1600, 1200)) {
        return Err(format!("decoded as {read:?}, not 1,600 pixels long"));
    }
    let whole = sioul_app::picture_reduced(&source, u64::MAX, 0, &here.join("whole.jpg")).ok_or("not written whole")?;
    if whole.0 != 1600 || whole.1 != 85 {
        return Err(format!("written whole as {whole:?}, not at 1,600 pixels and quality 85"));
    }
    let share = whole.2 / 3;
    let out = here.join("made.jpg");
    let (side, quality, size) = sioul_app::picture_reduced(&source, share, 0, &out).ok_or(format!("not made small for {share} bytes"))?;
    clean(&std::fs::read(&out).map_err(|e| e.to_string())?)?;
    let (width, height, _) = decoded(&out)?;
    if size > share || !(480..=1600).contains(&side) || !(50..=85).contains(&quality) || (side, quality) == (1600, 85) || width.max(height) != side as i32 {
        return Err(format!("for {share} bytes: {side} px ({width} × {height}), quality {quality}, {size} bytes"));
    }
    let advised = sioul_app::picture_reduced(&source, u64::MAX, 640, &here.join("advised.jpg")).ok_or("not made for a carrier's 640 pixels")?;
    if (advised.0, advised.1) != (640, 85) {
        return Err(format!("the carrier's 640 pixels: {advised:?}"));
    }
    if let Some(made) = sioul_app::picture_reduced(&source, 4 * 1024, 0, &here.join("tiny.jpg")) {
        return Err(format!("noise made into 4 KB: {made:?}"));
    }
    Ok(format!("whole {} bytes; for {share}: {side} px, quality {quality}, {size} bytes", whole.2))
}

/// A whole message: a picture, a PNG, a video, a card and an animated GIF
/// under 300 kB with a sixth file refused; the message measured with its
/// files, the most words and the longest number, within the limit; the
/// files made named by when and rank. Then the refusals: another file, a
/// folder, a broken picture, a video too large, pictures too many for their share.
fn whole_message(here: &Path) -> Result<String, String> {
    let limit = 300 * 1024;
    let write = |name: &str, bytes: Vec<u8>| -> Result<PathBuf, String> {
        let path = here.join(name);
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(path)
    };
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed.to_le_bytes()[0]
    };
    let noisy: Vec<u8> = (0..900 * 600 * 3).map(|_| noise()).collect();
    let holiday = write("holiday-in-Lisbon.bmp", bmp(900, 600, |x, y| {
        let i = ((y * 900 + x) * 3) as usize;
        [noisy[i], noisy[i + 1], noisy[i + 2]]
    }))?;
    let drawing = write("drawing.png", png(64, 64, |x, y| [(x * 4) as u8, (y * 4) as u8, 128, 255], &[]))?;
    let clip = write("clip-of-Alice.mp4", [b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2".as_slice(), &[0u8; 50_000]].concat())?;
    let card = write("Alice Example.vcf", b"BEGIN:VCARD\r\nVERSION:2.1\r\nN:Example;Alice\r\nTEL;CELL:01 99 00 12 34\r\nEND:VCARD\r\n".to_vec())?;
    let dance = write("dance.gif", gif(16, 16, &[vec![0; 256], vec![3; 256]]))?;
    let sixth = write("sixth.jpg", std::fs::read(&holiday).map_err(|e| e.to_string())?)?;
    let folder = here.join("made");
    let sources = [holiday.clone(), drawing.clone(), clip.clone(), card.clone(), dance.clone(), sixth.clone()];
    let (made, left) = sioul_app::attach_made(&sources, limit, 0, &folder);
    let kinds: Vec<(&str, &str)> = made.iter().map(|(kind, ct, _, _)| (kind.as_str(), ct.as_str())).collect();
    if kinds != [("picture", "image/jpeg"), ("picture", "image/jpeg"), ("video", "video/mp4"), ("card", "text/x-vCard"), ("picture", "image/gif")] {
        return Err(format!("made {kinds:?}"));
    }
    if left != [(sixth.display().to_string(), "too-many".to_string())] {
        return Err(format!("left {left:?}"));
    }
    let mut files = Vec::new();
    for (kind, ct, path, size) in &made {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if bytes.len() as u64 != *size {
            return Err(format!("{path}: {} bytes, said {size}", bytes.len()));
        }
        if ct == "image/jpeg" {
            clean(&bytes).map_err(|why| format!("{kind}: {why}"))?;
        }
        let name = Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (stamp, rest) = name.split_once('-').unwrap_or_default();
        if stamp.is_empty() || !stamp.bytes().all(|b| b.is_ascii_digit()) || !rest.split('.').next().is_some_and(|rank| rank.bytes().all(|b| b.is_ascii_digit())) {
            return Err(format!("a file made named \"{name}\", not by when and rank"));
        }
        files.push((ct.clone(), bytes));
    }
    // The message as the phone would write it, with the most words and the longest number.
    let media: Vec<sioul_core::mms::Medium> = files.iter().map(|(ct, bytes)| sioul_core::mms::Medium { ct, bytes }).collect();
    let words = "é".repeat(800) + &"😀".repeat(800);
    let key = "1".repeat(32);
    let message = sioul_core::mms::Message { key: &key, to: &[sioul_core::mms::LONGEST_NUMBER], date: 1_791_446_400, words: &words, media: &media };
    let whole = sioul_core::mms::size(&message).map_err(|e| format!("{e:?}"))?;
    if whole as u64 > limit {
        return Err(format!("the message is {whole} bytes over {limit}"));
    }
    // The pictures within the room the message leaves (its media empty, the most words, the
    // longest number) less what goes as it is; the noisy one given what the small one left.
    let empty: Vec<sioul_core::mms::Medium> = files.iter().map(|(ct, _)| sioul_core::mms::Medium { ct, bytes: &[] }).collect();
    let reserve = "x".repeat(6_400);
    let zeros = "0".repeat(32);
    let room = sioul_core::mms::room(&sioul_core::mms::Message { key: &zeros, to: &[sioul_core::mms::LONGEST_NUMBER], date: 1_791_446_400, words: &reserve, media: &empty }, limit as usize).ok_or("no room")? as u64;
    let fixed: u64 = made.iter().filter(|(_, ct, _, _)| ct != "image/jpeg").map(|(_, _, _, size)| size).sum();
    let share = (room - fixed) / 2;
    let sizes: Vec<u64> = made.iter().filter(|(_, ct, _, _)| ct == "image/jpeg").map(|(_, _, _, size)| *size).collect();
    let pictures: u64 = sizes.iter().sum();
    if pictures > room - fixed || sizes[1] > share || sizes[0] <= share {
        return Err(format!("the pictures {sizes:?} for {} bytes, an equal share {share}", room - fixed));
    }

    // Each refusal, in its words' key.
    let pdf = write("lease.pdf", b"%PDF-1.7\n%stand-in\n".to_vec())?;
    let broken = write("broken.jpg", vec![7u8; 1000])?;
    let big = write("long.mp4", [b"\0\0\0\x18ftypisom\0\0\x02\0isomiso2".as_slice(), &[0u8; 400_000]].concat())?;
    // A PDF named as a video: another file, told by its bytes (the review's I-3).
    let named = write("lease.mp4", b"%PDF-1.7\n%stand-in\n".to_vec())?;
    let (made, left) = sioul_app::attach_made(&[pdf.clone(), here.to_path_buf(), broken.clone(), big.clone(), drawing.clone()], limit, 0, &here.join("refused"));
    let whys: Vec<(String, String)> = vec![(pdf.display().to_string(), "file".into()), (here.display().to_string(), "unreadable".into()), (broken.display().to_string(), "file".into()), (big.display().to_string(), "too-large".into())];
    if made.len() != 1 || left != whys {
        return Err(format!("refusals: made {made:?}, left {left:?}"));
    }
    let (made, left) = sioul_app::attach_made(&[named.clone()], limit, 0, &here.join("named"));
    if !made.is_empty() || left != [(named.display().to_string(), "file".to_string())] {
        return Err(format!("a PDF named .mp4: made {made:?}, left {left:?}"));
    }
    // Five pictures for 80 kB: room for four of 16 KB each beside the words; the last one said so.
    let five: Vec<PathBuf> = (0..5).map(|n| write(&format!("p{n}.png"), png(32, 32, move |x, y| [(x * 8) as u8, (y * 8) as u8, (n * 50) as u8, 255], &[]))).collect::<Result<_, _>>()?;
    let (made, left) = sioul_app::attach_made(&five, 80 * 1024, 0, &here.join("five"));
    if made.len() != 4 || left != [(five[4].display().to_string(), "too-many-pictures".to_string())] {
        return Err(format!("five pictures in 80 kB: made {}, left {left:?}", made.len()));
    }
    Ok(format!("{whole} of {limit} bytes, the pictures {pictures}"))
}

/// A file of SIOUL_ATTACH_SAMPLES made fit as the page makes it
/// (`attach_made`), and checked: a picture made a clean JPEG; an animated
/// GIF holding none of the made-up words; anything else said as it went,
/// or why it did not.
fn sample(source: &Path, out: &Path) -> Result<String, String> {
    let (format, _, read) = sioul_app::picture_read(source);
    let (made, left) = sioul_app::attach_made(&[source.to_path_buf()], 600_000, 0, &out.with_extension("made"));
    let [(kind, ct, path, size)] = made.as_slice() else { return Ok(format!("\"{format}\" {read:?}: refused, {:?}", left.iter().map(|(_, why)| why.as_str()).collect::<Vec<_>>())) };
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    match ct.as_str() {
        "image/jpeg" => clean(&bytes)?,
        "image/gif" if String::from_utf8_lossy(&bytes).contains(SECRET) || bytes.windows(11).any(|w| w == b"XMP DataXMP") => return Err("an animated GIF kept its comment or XMP".into()),
        _ => {}
    }
    Ok(format!("\"{format}\" {read:?} → {kind} {ct}, {size} bytes{}", if ct == "image/jpeg" { ", clean" } else { "" }))
}

// ---------------------------------------------------------------- pictures, byte by byte

/// A JPEG segment.
fn segment(marker: u8, body: &[u8]) -> Vec<u8> {
    [&[0xFF, marker], &((body.len() + 2) as u16).to_be_bytes()[..], body].concat()
}

/// The JPEG `first` given a camera's metadata after its JFIF segment: EXIF
/// (orientation 6, maker, model, time, GPS), XMP, the swapped profile, IPTC, a comment.
fn with_metadata(first: &[u8]) -> Result<Vec<u8>, String> {
    if first.get(2..4) != Some(&[0xFF, 0xE0]) {
        return Err("Qt's JPEG has no JFIF segment first".into());
    }
    let split = 4 + usize::from(u16::from_be_bytes([first[4], first[5]]));
    let exif = segment(0xE1, &[b"Exif\0\0".as_slice(), &exif_tiff(6)].concat());
    let xmp = segment(0xE1, &[b"http://ns.adobe.com/xap/1.0/\0".as_slice(), xmp().as_bytes()].concat());
    let icc = segment(0xE2, &[b"ICC_PROFILE\0\x01\x01".as_slice(), &swapped_profile()].concat());
    let caption = b"SIOUL-SECRET-IPTC-CAPTION";
    let iptc = [&[0x1C, 0x02, 0x78], &(caption.len() as u16).to_be_bytes()[..], caption].concat();
    let photoshop = segment(0xED, &[b"Photoshop 3.0\0".as_slice(), b"8BIM\x04\x04\0\0", &(iptc.len() as u32).to_be_bytes(), &iptc, if iptc.len() % 2 == 1 { &[0][..] } else { &[] }].concat());
    let comment = segment(0xFE, b"SIOUL-SECRET-COMMENT: written by a stand-in camera");
    Ok([&first[..split], &exif, &xmp, &icc, &photoshop, &comment, &first[split..]].concat())
}

fn xmp() -> String {
    "<x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'><rdf:Description xmlns:exif='http://ns.adobe.com/exif/1.0/' exif:GPSLatitude='12,20.576N' exif:GPSLongitude='30,15.0W' xmlns:photoshop='http://ns.adobe.com/photoshop/1.0/' photoshop:City='SIOUL-SECRET-XMP'/></rdf:RDF></x:xmpmeta>".to_string()
}

/// A TIFF directory's entry: its tag, type (1 byte, 2 ASCII, 3 short, 4
/// long, 5 rational, 7 undefined), count and value (little-endian).
type Entry = (u16, u16, u32, Vec<u8>);

fn text_entry(tag: u16, text: &str) -> Entry {
    let mut value = text.as_bytes().to_vec();
    value.push(0);
    (tag, 2, value.len() as u32, value)
}

fn short(value: u16) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn long(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

/// A directory's length with the values that do not fit in their entry.
fn ifd_len(entries: &[Entry]) -> usize {
    2 + 12 * entries.len() + 4 + entries.iter().map(|e| if e.3.len() > 4 { (e.3.len() + 1) & !1 } else { 0 }).sum::<usize>()
}

/// The directory placed at `at` (from the TIFF header), its longer values after it; no next one.
fn ifd(entries: &[Entry], at: usize) -> Vec<u8> {
    let mut head = (entries.len() as u16).to_le_bytes().to_vec();
    let mut tail = Vec::new();
    let data = at + 2 + 12 * entries.len() + 4;
    for (tag, kind, count, value) in entries {
        head.extend(tag.to_le_bytes());
        head.extend(kind.to_le_bytes());
        head.extend(count.to_le_bytes());
        if value.len() <= 4 {
            let mut inline = value.clone();
            inline.resize(4, 0);
            head.extend(inline);
        } else {
            head.extend(((data + tail.len()) as u32).to_le_bytes());
            tail.extend(value);
            if tail.len() % 2 == 1 {
                tail.push(0);
            }
        }
    }
    head.extend(0u32.to_le_bytes());
    head.extend(tail);
    head
}

/// A GPS position, made up, in the Atlantic: 12° 20′ 34.56″ N, 30° 15′ W; and its method's words.
fn gps_entries() -> Vec<Entry> {
    let rationals = |values: &[(u32, u32)]| values.iter().flat_map(|(n, d)| [n.to_le_bytes(), d.to_le_bytes()].concat()).collect::<Vec<u8>>();
    let method = b"ASCII\0\0\0SIOUL-SECRET-GPS".to_vec();
    vec![
        (0x0000, 1, 4, vec![2, 3, 0, 0]),
        (0x0001, 2, 2, b"N\0".to_vec()),
        (0x0002, 5, 3, rationals(&[(12, 1), (20, 1), (3456, 100)])),
        (0x0003, 2, 2, b"W\0".to_vec()),
        (0x0004, 5, 3, rationals(&[(30, 1), (15, 1), (0, 1)])),
        (0x001B, 7, method.len() as u32, method),
    ]
}

/// A camera's EXIF (TIFF, little-endian): maker, model, time, the
/// orientation given, a pointer to the GPS position.
fn exif_tiff(orientation: u16) -> Vec<u8> {
    let mut first: Vec<Entry> = vec![
        text_entry(0x010F, "SIOUL-SECRET-CAMERA-MAKER"),
        text_entry(0x0110, "SIOUL-SECRET-CAMERA-MODEL"),
        (0x0112, 3, 1, short(orientation)),
        text_entry(0x0132, "2026:10:18 12:34:56"),
        (0x8825, 4, 1, long(0)),
    ];
    let gps = gps_entries();
    let gps_at = 8 + ifd_len(&first);
    first[4].3 = long(gps_at as u32);
    [b"II*\0".to_vec(), long(8), ifd(&first, 8), ifd(&gps, gps_at)].concat()
}

/// An uncompressed RGB TIFF with a description, a maker, a time and a GPS position.
fn tiff(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let strip: Vec<u8> = (0..height).flat_map(|y| (0..width).map(move |x| (x, y))).flat_map(|(x, y)| pixel(x, y)).collect();
    let mut first: Vec<Entry> = vec![
        (256, 4, 1, long(width)),
        (257, 4, 1, long(height)),
        (258, 3, 3, [8u16, 8, 8].iter().flat_map(|b| b.to_le_bytes()).collect()),
        (259, 3, 1, short(1)),
        (262, 3, 1, short(2)),
        text_entry(270, "SIOUL-SECRET-TIFF-DESCRIPTION"),
        text_entry(271, "SIOUL-SECRET-CAMERA-MAKER"),
        (273, 4, 1, long(0)),
        (277, 3, 1, short(3)),
        (278, 4, 1, long(height)),
        (279, 4, 1, long(strip.len() as u32)),
        (284, 3, 1, short(1)),
        text_entry(306, "2026:10:18 12:34:56"),
        (34853, 4, 1, long(0)),
    ];
    let gps = gps_entries();
    let gps_at = 8 + ifd_len(&first);
    let strip_at = gps_at + ifd_len(&gps);
    first[7].3 = long(strip_at as u32);
    first[13].3 = long(gps_at as u32);
    [b"II*\0".to_vec(), long(8), ifd(&first, 8), ifd(&gps, gps_at), strip].concat()
}

/// A 24-bit BMP, its rows from the bottom.
fn bmp(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let row = ((width * 3 + 3) & !3) as usize;
    let size = 54 + row * height as usize;
    let mut out = Vec::with_capacity(size);
    out.extend(b"BM");
    out.extend((size as u32).to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(54u32.to_le_bytes());
    out.extend(40u32.to_le_bytes());
    out.extend(width.to_le_bytes());
    out.extend(height.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(24u16.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(((row * height as usize) as u32).to_le_bytes());
    out.extend(2835u32.to_le_bytes());
    out.extend(2835u32.to_le_bytes());
    out.extend([0u8; 8]);
    for y in (0..height).rev() {
        let start = out.len();
        for x in 0..width {
            let [r, g, b] = pixel(x, y);
            out.extend([b, g, r]);
        }
        out.resize(start + row, 0);
    }
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A PNG chunk.
fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let crc = crc32(&[kind.as_slice(), body].concat());
    [&(body.len() as u32).to_be_bytes()[..], kind, body, &crc.to_be_bytes()].concat()
}

/// An RGBA PNG, its chunks `extra` before its pixels, which zlib stores as they are.
fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4], extra: &[Vec<u8>]) -> Vec<u8> {
    let raw: Vec<u8> = (0..height).flat_map(|y| std::iter::once(0u8).chain((0..width).flat_map(|x| pixel(x, y))).collect::<Vec<_>>()).collect();
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        zlib.push(u8::from(i + 1 == blocks.len()));
        zlib.extend((block.len() as u16).to_le_bytes());
        zlib.extend((!(block.len() as u16)).to_le_bytes());
        zlib.extend(*block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    zlib.extend(((b << 16) | a).to_be_bytes());
    let header = [&width.to_be_bytes()[..], &height.to_be_bytes(), &[8, 6, 0, 0, 0]].concat();
    [b"\x89PNG\r\n\x1a\n".to_vec(), chunk(b"IHDR", &header), extra.concat(), chunk(b"IDAT", &zlib), chunk(b"IEND", &[])].concat()
}

/// A GIF89a of `frames` (each `width` × `height` of colours 0 to 3: red,
/// green, blue, white), looping when more than one. Its codes are written
/// three bits each, a clear code before every two pixels, so that its
/// table never grows past them.
fn gif(width: u16, height: u16, frames: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"GIF89a".to_vec();
    out.extend(width.to_le_bytes());
    out.extend(height.to_le_bytes());
    out.extend([0x81, 0, 0]);
    out.extend([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
    if frames.len() > 1 {
        out.extend(b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\0\0\0");
    }
    for pixels in frames {
        out.extend([0x21, 0xF9, 4, 0, 10, 0, 0, 0]);
        out.push(0x2C);
        out.extend([0, 0, 0, 0]);
        out.extend(width.to_le_bytes());
        out.extend(height.to_le_bytes());
        out.push(0);
        out.push(2);
        let mut codes = Vec::new();
        for pair in pixels.chunks(2) {
            codes.push(4u32);
            codes.extend(pair.iter().map(|&p| u32::from(p)));
        }
        codes.push(5);
        let (mut data, mut bits, mut count) = (Vec::new(), 0u32, 0u32);
        for code in codes {
            bits |= code << count;
            count += 3;
            while count >= 8 {
                data.push((bits & 0xFF) as u8);
                bits >>= 8;
                count -= 8;
            }
        }
        if count > 0 {
            data.push((bits & 0xFF) as u8);
        }
        for block in data.chunks(255) {
            out.push(block.len() as u8);
            out.extend(block);
        }
        out.push(0);
    }
    out.push(0x3B);
    out
}

/// A display profile (ICC 2.1) whose red and blue are sRGB's blue and red,
/// its curves a gamma of 2.2: a pixel red in it is blue in sRGB.
fn swapped_profile() -> Vec<u8> {
    let s15 = |v: f64| ((v * 65_536.0).round() as i32).to_be_bytes();
    let xyz = |x: f64, y: f64, z: f64| [b"XYZ ".as_slice(), &[0; 4], &s15(x), &s15(y), &s15(z)].concat();
    let curve = [b"curv".as_slice(), &[0; 4], &1u32.to_be_bytes(), &[0x02, 0x33]].concat();
    let tags: [(&[u8; 4], Vec<u8>); 7] = [
        (b"wtpt", xyz(0.9642, 1.0, 0.8249)),
        (b"rXYZ", xyz(0.1431, 0.0606, 0.7141)),
        (b"gXYZ", xyz(0.3851, 0.7169, 0.0971)),
        (b"bXYZ", xyz(0.4361, 0.2225, 0.0139)),
        (b"rTRC", curve.clone()),
        (b"gTRC", curve.clone()),
        (b"bTRC", curve),
    ];
    let table = 4 + 12 * tags.len();
    let (mut entries, mut data) = (Vec::new(), Vec::new());
    for (signature, body) in &tags {
        entries.extend(*signature);
        entries.extend(((128 + table + data.len()) as u32).to_be_bytes());
        entries.extend((body.len() as u32).to_be_bytes());
        data.extend(body);
        data.resize(data.len().div_ceil(4) * 4, 0);
    }
    let mut header = vec![0u8; 128];
    header[0..4].copy_from_slice(&((128 + table + data.len()) as u32).to_be_bytes());
    header[8..12].copy_from_slice(&[0x02, 0x10, 0, 0]);
    header[12..16].copy_from_slice(b"mntr");
    header[16..20].copy_from_slice(b"RGB ");
    header[20..24].copy_from_slice(b"XYZ ");
    for (i, value) in [2026u16, 10, 18, 12, 0, 0].iter().enumerate() {
        header[24 + 2 * i..26 + 2 * i].copy_from_slice(&value.to_be_bytes());
    }
    header[36..40].copy_from_slice(b"acsp");
    header[68..80].copy_from_slice(&[s15(0.9642), s15(1.0), s15(0.8249)].concat());
    [header, (tags.len() as u32).to_be_bytes().to_vec(), entries, data].concat()
}

fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len()).step_by(2).filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok()).collect()
}

// The WebP, made with Pillow 12.3 (libwebp 1.6) from a 24 × 16 picture, its
// left half red, its right half green, lossless, with EXIF written by
// Pillow (maker, model and time as above, the GPS position above), the XMP
// above and the swapped profile (built by the same steps as
// `swapped_profile`), all made up.
const WEBP: &str = concat!(
    "524946460e04000057454250565038580a0000002c0000001700000f000049434350580100000000015800000000021000006d6e7472524742205859",
    "5a2007ea000a0012000c0000000061637370000000000000000000000000000000000000000000000000000000000000f6d6000100000000d32d0000",
    "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000777747074000000d80000",
    "00147258595a000000ec000000146758595a00000100000000146258595a000001140000001472545243000001280000000e67545243000001380000",
    "000e62545243000001480000000e58595a20000000000000f6d6000100000000d32d58595a2000000000000024a200000f830000b6cf58595a200000",
    "0000000062960000b787000018dc58595a200000000000006fa4000038f60000038f6375727600000000000000010233000063757276000000000000",
    "000102330000637572760000000000000001023300005650384c1a0000002f17c003000fb0fff31ff31f1584048407feafd52d88e87f243345584946",
    "1c0100004d4d002a000000080004010f00020000001a0000003e011000020000001a0000005801320002000000140000007288250004000000010000",
    "00860000000053494f554c2d5345435245542d43414d4552412d4d414b45520053494f554c2d5345435245542d43414d4552412d4d4f44454c003230",
    "32363a31303a31382031323a33343a353600000600000001000000040203000000010002000000024e0000000002000500000003000000d400030002",
    "00000002570000000004000500000003000000ec001b00070000001800000104000000000000000c0000000100000014000000010000036000000019",
    "0000001e000000010000000f000000010000000000000001415343494900000053494f554c2d5345435245542d475053584d50204a0100003c783a78",
    "6d706d65746120786d6c6e733a783d2761646f62653a6e733a6d6574612f273e3c7264663a52444620786d6c6e733a7264663d27687474703a2f2f77",
    "77772e77332e6f72672f313939392f30322f32322d7264662d73796e7461782d6e7323273e3c7264663a4465736372697074696f6e20786d6c6e733a",
    "657869663d27687474703a2f2f6e732e61646f62652e636f6d2f657869662f312e302f2720657869663a4750534c617469747564653d2731322c3230",
    "2e3537364e2720657869663a4750534c6f6e6769747564653d2733302c31352e30572720786d6c6e733a70686f746f73686f703d27687474703a2f2f",
    "6e732e61646f62652e636f6d2f70686f746f73686f702f312e302f272070686f746f73686f703a436974793d2753494f554c2d5345435245542d584d",
    "50272f3e3c2f7264663a5244463e3c2f783a786d706d6574613e",
);
