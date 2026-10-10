// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Colours of the sites on the screen, and calmer colours (docs/colour.md).
//!
//! Chromium, inside Qt WebEngine, paints every page in sRGB numbers, and Qt
//! Quick hands them to the screen as they are: on a screen wider than sRGB
//! (an Adobe RGB panel, a P3 one) every colour comes out more saturated than
//! its page meant. A site's picture goes through a shader
//! (`shaders/colour.frag`) that looks each pixel up in the two tables made
//! here, once per screen and setting:
//!
//! - **the cube**, 33 × 33 × 33 entries over the sRGB numbers a page holds,
//!   each the colour the screen must show, as the screen's own *linear*
//!   values, unclipped (a colour just outside the screen's gamut keeps its
//!   small negative value, so that interpolating across the edge of the gamut
//!   stays smooth); packed in two dimensions, 33 tiles of 33 × 33 side by
//!   side, blue choosing the tile;
//! - **the curve**, 1,024 entries per channel turning linear values into the
//!   screen's numbers (the inverse of its tone curves), read at
//!   `u = y^(1/2.4)`, which spreads the entries where the eye sees most.
//!
//! "Calmer colours" softens loud colours before that: each colour's chroma in
//! OKLCh is limited softly, `C' = k · tanh(C / k)`, its lightness and hue kept
//! (OKLab: Björn Ottosson, "A perceptual color space for image processing",
//! 2020). Greys and soft tints barely move; a pure red becomes a brick red.
//!
//! The screen's profile is read by `cpp/screencolours.cpp` (the X11 atoms
//! `_ICC_PROFILE`, matched to screens through colord), which calls
//! [`sioul_colour_tables`]. Little CMS (`lcms2`) reads the profile, on Linux
//! only: Sioul converts to the screen on X11 alone (macOS converts by itself,
//! Windows and Wayland too or not at all), while calmer colours, computed
//! here, work on every computer. Measured against Little CMS's own transform,
//! the tables give every colour within one 8-bit code (`tests`, and the
//! research note docs/research/colour.md).

#[cfg(target_os = "linux")]
use lcms2::{InfoType, Intent, Locale, PixelFormat, Profile, Tag, TagSignature, ToneCurve, Transform};
use std::sync::Mutex;

/// Entries of the cube on each side.
pub(crate) const CUBE: usize = 33;
/// Entries of the curve.
pub(crate) const CURVE: usize = 1024;

/// How much calmer the sites' colours are (Settings ▸ Display).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Calmer {
    Off,
    /// "A little": chroma softly limited at 0.12 in OKLCh.
    Little,
    /// "More": at 0.08.
    More,
}

impl Calmer {
    /// As `cpp/screencolours.cpp` passes it: 0 off, 1 "A little", 2 "More"
    /// (the configuration's "", "little", "more", mapped by `SitesPage.qml`).
    fn from_index(index: i32) -> Calmer {
        match index {
            1 => Calmer::Little,
            2 => Calmer::More,
            _ => Calmer::Off,
        }
    }

    /// The OKLCh chroma the soft limit tends to; none when off. Pure sRGB red
    /// has a chroma of 0.26, a pastel 0.03, a face 0.05 to 0.08.
    fn limit(self) -> Option<f64> {
        match self {
            Calmer::Off => None,
            Calmer::Little => Some(0.12),
            Calmer::More => Some(0.08),
        }
    }
}

/// The two tables the shader reads, as RGBA floats (alpha unused, 1).
pub(crate) struct Tables {
    /// `CUBE`² × `CUBE` texels, rows of `CUBE`² texels: the texel of the
    /// sRGB numbers (r, g, b), in steps of 1/32, is at x = r + b · 33, y = g.
    pub cube: Vec<f32>,
    /// `CURVE` texels: entry j holds the screen's numbers for the linear
    /// value (j / 1023)^2.4.
    pub curve: Vec<f32>,
}

/// What the screen does with linear values, as the tables need it.
enum Screen {
    /// sRGB: no profile, or conversion left to the desktop.
    Srgb,
    /// A matrix/shaper profile: its linear RGB from XYZ (D50), and the
    /// inverse of its three tone curves.
    #[cfg(target_os = "linux")]
    Matrix { from_xyz: [[f64; 3]; 3], inverse: [ToneCurve; 3] },
    /// A profile of tables: Little CMS's own transform from sRGB, whose
    /// numbers the cube then holds (the curve being identity).
    #[cfg(target_os = "linux")]
    Tables(Transform<[f32; 3], [f32; 3]>),
}

/// The tables for a screen's profile (none: sRGB) and calmer colours, or
/// None when they would change nothing (an sRGB screen, calmer colours off).
/// An error when the profile cannot be read, or is not an RGB one.
pub(crate) fn tables(profile: Option<&[u8]>, calmer: Calmer) -> Result<Option<Tables>, String> {
    let screen = match profile {
        Some(bytes) => screen_of(bytes)?,
        None => Screen::Srgb,
    };
    if calmer == Calmer::Off && matches!(screen, Screen::Srgb) {
        return Ok(None);
    }
    let made = make(&screen, calmer);
    if calmer == Calmer::Off && is_identity(&made) {
        return Ok(None);
    }
    Ok(Some(made))
}

/// The screen a profile describes; profiles are read on Linux only.
#[cfg(not(target_os = "linux"))]
fn screen_of(_bytes: &[u8]) -> Result<Screen, String> {
    Err("colour profiles are read on Linux only".into())
}

/// The screen a profile describes.
#[cfg(target_os = "linux")]
fn screen_of(bytes: &[u8]) -> Result<Screen, String> {
    let profile = Profile::new_icc(bytes).map_err(|e| format!("the profile cannot be read: {e}"))?;
    if profile.color_space() != lcms2::ColorSpaceSignature::RgbData {
        return Err("not an RGB profile".into());
    }
    // A profile of tables (some hold a matrix too): Little CMS reads its
    // tables, which describe the screen better than the matrix.
    let has_tables = [TagSignature::BToA0Tag, TagSignature::BToA1Tag].iter().any(|&tag| profile.has_tag(tag));
    if profile.is_matrix_shaper() && !has_tables {
        let colorant = |tag| match profile.read_tag(tag) {
            Tag::CIEXYZ(xyz) => Ok([xyz.X, xyz.Y, xyz.Z]),
            _ => Err("a colorant is missing".to_string()),
        };
        let (r, g, b) = (colorant(TagSignature::RedColorantTag)?, colorant(TagSignature::GreenColorantTag)?, colorant(TagSignature::BlueColorantTag)?);
        let to_xyz = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
        let from_xyz = invert(&to_xyz).ok_or("the profile's primaries are not independent")?;
        let curve = |tag| match profile.read_tag(tag) {
            Tag::ToneCurve(curve) => Ok(curve.reversed()),
            _ => Err("a tone curve is missing".to_string()),
        };
        let inverse = [curve(TagSignature::RedTRCTag)?, curve(TagSignature::GreenTRCTag)?, curve(TagSignature::BlueTRCTag)?];
        return Ok(Screen::Matrix { from_xyz, inverse });
    }
    let transform = Transform::new(&Profile::new_srgb(), PixelFormat::RGB_FLT, &profile, PixelFormat::RGB_FLT, Intent::RelativeColorimetric)
        .map_err(|e| format!("no transform to the profile: {e}"))?;
    Ok(Screen::Tables(transform))
}

/// The cube and the curve.
fn make(screen: &Screen, calmer: Calmer) -> Tables {
    let n = CUBE;
    let mut cube = vec![1.0f32; n * n * n * 4];
    // sRGB's linear values to XYZ (D50), as Little CMS's own sRGB profile has them.
    #[cfg(target_os = "linux")]
    let srgb_to_xyz = srgb_to_xyz();
    #[cfg(target_os = "linux")]
    let mut encoded = Vec::new();
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let numbers = [r, g, b].map(|i| i as f64 / (n - 1) as f64);
                let mut linear = numbers.map(srgb_decode);
                if let Some(limit) = calmer.limit() {
                    linear = soften(linear, limit);
                }
                let at = ((r + b * n) + g * n * n) * 4;
                match screen {
                    Screen::Srgb => cube[at..at + 3].copy_from_slice(&linear.map(|v| v as f32)),
                    #[cfg(target_os = "linux")]
                    Screen::Matrix { from_xyz, .. } => {
                        let screen_linear = mul(from_xyz, &mul(&srgb_to_xyz, &linear));
                        cube[at..at + 3].copy_from_slice(&screen_linear.map(|v| v as f32));
                    }
                    #[cfg(target_os = "linux")]
                    Screen::Tables(_) => encoded.push(linear.map(|v| srgb_encode(v) as f32)),
                }
            }
        }
    }
    // A profile of tables: the cube holds the screen's numbers, turned
    // linear by u^2.4 so that the shader's curve (identity, below) gives them back.
    #[cfg(target_os = "linux")]
    if let Screen::Tables(transform) = screen {
        let mut out = vec![[0.0f32; 3]; encoded.len()];
        transform.transform_pixels(&encoded, &mut out);
        let mut i = 0;
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let at = ((r + b * n) + g * n * n) * 4;
                    cube[at..at + 3].copy_from_slice(&out[i].map(|v| v.clamp(0.0, 1.0).powf(2.4)));
                    i += 1;
                }
            }
        }
    }
    let mut curve = vec![1.0f32; CURVE * 4];
    for j in 0..CURVE {
        let linear = (j as f64 / (CURVE - 1) as f64).powf(2.4);
        let numbers: [f64; 3] = match screen {
            Screen::Srgb => [srgb_encode(linear); 3],
            #[cfg(target_os = "linux")]
            Screen::Matrix { inverse, .. } => [0, 1, 2].map(|c| f64::from(inverse[c].eval(linear as f32))),
            #[cfg(target_os = "linux")]
            Screen::Tables(_) => [linear.powf(1.0 / 2.4); 3],
        };
        curve[j * 4..j * 4 + 3].copy_from_slice(&numbers.map(|v| v.clamp(0.0, 1.0) as f32));
    }
    Tables { cube, curve }
}

/// Whether the tables give back what they are given, within a fifth of an
/// 8-bit code: an sRGB screen's profile, calmer colours off.
fn is_identity(tables: &Tables) -> bool {
    let n = CUBE;
    (0..n).all(|b| {
        (0..n).all(|g| {
            (0..n).all(|r| {
                let at = ((r + b * n) + g * n * n) * 4;
                [r, g, b].iter().enumerate().all(|(c, &i)| {
                    let want = i as f64 / (n - 1) as f64;
                    let got = curve_at(tables, c, f64::from(tables.cube[at + c]));
                    (got - want).abs() < 0.2 / 255.0
                })
            })
        })
    })
}

/// The curve at a linear value, as the shader reads it (linear interpolation in u).
fn curve_at(tables: &Tables, channel: usize, linear: f64) -> f64 {
    let u = linear.clamp(0.0, 1.0).powf(1.0 / 2.4) * (CURVE - 1) as f64;
    let j = (u.floor() as usize).min(CURVE - 2);
    let f = u - j as f64;
    let at = |k: usize| f64::from(tables.curve[k * 4 + channel]);
    at(j) * (1.0 - f) + at(j + 1) * f
}

/// sRGB's numbers to linear light (IEC 61966-2-1).
fn srgb_decode(v: f64) -> f64 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// Linear light to sRGB's numbers.
fn srgb_encode(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// sRGB's linear values to XYZ relative to D50: the colorants of Little
/// CMS's own sRGB profile, as every transform from sRGB uses them.
#[cfg(target_os = "linux")]
fn srgb_to_xyz() -> [[f64; 3]; 3] {
    let srgb = Profile::new_srgb();
    let colorant = |tag| match srgb.read_tag(tag) {
        Tag::CIEXYZ(xyz) => [xyz.X, xyz.Y, xyz.Z],
        _ => [0.0; 3],
    };
    let (r, g, b) = (colorant(TagSignature::RedColorantTag), colorant(TagSignature::GreenColorantTag), colorant(TagSignature::BlueColorantTag));
    [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]]
}

// OKLab, from linear sRGB (Björn Ottosson, 2020).
const TO_LMS: [[f64; 3]; 3] = [[0.412_221_470_8, 0.536_332_536_3, 0.051_445_992_9], [0.211_903_498_2, 0.680_699_545_1, 0.107_396_956_6], [0.088_302_461_9, 0.281_718_837_6, 0.629_978_700_5]];
const TO_LAB: [[f64; 3]; 3] = [[0.210_454_255_3, 0.793_617_785_0, -0.004_072_046_8], [1.977_998_495_1, -2.428_592_205_0, 0.450_593_709_9], [0.025_904_037_1, 0.782_771_766_2, -0.808_675_766_0]];
const FROM_LAB: [[f64; 3]; 3] = [[1.0, 0.396_337_777_4, 0.215_803_757_3], [1.0, -0.105_561_345_8, -0.063_854_172_8], [1.0, -0.089_484_177_5, -1.291_485_548_0]];
const FROM_LMS: [[f64; 3]; 3] = [[4.076_741_662_1, -3.307_711_591_3, 0.230_969_929_2], [-1.268_438_004_6, 2.609_757_401_1, -0.341_319_396_5], [-0.004_196_086_3, -0.703_418_614_7, 1.707_614_701_0]];

/// Linear sRGB to OKLab.
fn oklab(linear: [f64; 3]) -> [f64; 3] {
    mul(&TO_LAB, &mul(&TO_LMS, &linear).map(f64::cbrt))
}

/// OKLab to linear sRGB.
fn from_oklab(lab: [f64; 3]) -> [f64; 3] {
    mul(&FROM_LMS, &mul(&FROM_LAB, &lab).map(|v| v * v * v))
}

/// The OKLCh chroma of a linear sRGB colour.
#[cfg(test)]
fn chroma(linear: [f64; 3]) -> f64 {
    let lab = oklab(linear);
    lab[1].hypot(lab[2])
}

/// A colour's chroma softly limited at `limit` (OKLCh), its lightness and hue kept.
fn soften(linear: [f64; 3], limit: f64) -> [f64; 3] {
    let mut lab = oklab(linear);
    let c = lab[1].hypot(lab[2]);
    if c > 1e-9 {
        let k = limit * (c / limit).tanh() / c;
        lab[1] *= k;
        lab[2] *= k;
    }
    from_oklab(lab).map(|v| v.clamp(0.0, 1.0))
}

fn mul(m: &[[f64; 3]; 3], v: &[f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

#[cfg(target_os = "linux")]
fn invert(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-12 {
        return None;
    }
    let c = |r: usize, k: usize| {
        let (r1, r2, k1, k2) = ((r + 1) % 3, (r + 2) % 3, (k + 1) % 3, (k + 2) % 3);
        m[r1][k1] * m[r2][k2] - m[r1][k2] * m[r2][k1]
    };
    Some([0, 1, 2].map(|i| [0, 1, 2].map(|k| c(k, i) / det)))
}

/// A profile's own name ("Built-in display, calibrated"), for the sentence in
/// Settings; none where profiles are not read.
#[cfg(not(target_os = "linux"))]
pub(crate) fn profile_name(_bytes: &[u8]) -> Option<String> {
    None
}

/// A profile's own name ("Built-in display, calibrated"), for the sentence in Settings.
#[cfg(target_os = "linux")]
pub(crate) fn profile_name(bytes: &[u8]) -> Option<String> {
    let profile = Profile::new_icc(bytes).ok()?;
    profile.info(InfoType::Description, Locale::none()).map(|name| name.trim().to_string()).filter(|name| !name.is_empty())
}

/// What `cpp/screencolours.cpp` said last of the screen Sioul's window is on,
/// for the sentence under "Colours for this screen" in Settings ▸ Display.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Said {
    /// Not said yet: the Sites page has not been opened.
    #[default]
    Unknown,
    /// Converted to the screen's profile, named.
    Converted(String),
    /// The desktop converts by itself (Wayland, macOS).
    Desktop,
    /// No profile, or an sRGB one: colours as they are.
    Plain,
    /// Turned off.
    Off,
}

static SAID: Mutex<Said> = Mutex::new(Said::Unknown);

/// The last state said.
pub(crate) fn said() -> Said {
    SAID.lock().map(|s| s.clone()).unwrap_or_default()
}

/// The tables for `cpp/screencolours.cpp`: the profile's `length` bytes (none
/// when null), how calm (0, 1, 2), and where to write them: `cube_size`³ × 4
/// floats at `cube`, `curve_size` × 4 at `curve`, which must be `CUBE` and
/// `CURVE`. Returns 1 when written, 0 when they would change nothing, -1 when
/// the profile cannot be used (then nothing is written), -2 for other sizes.
///
/// # Safety
/// `profile` holds `length` bytes, or is null; `cube` and `curve` have room
/// for what is said above.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_colour_tables(profile: *const u8, length: usize, calmer: i32, cube_size: usize, cube: *mut f32, curve_size: usize, curve: *mut f32) -> i32 {
    if cube_size != CUBE || curve_size != CURVE || cube.is_null() || curve.is_null() {
        return -2;
    }
    // SAFETY: the caller gives `length` bytes at `profile`, or null.
    let bytes = (!profile.is_null() && length > 0).then(|| unsafe { std::slice::from_raw_parts(profile, length) });
    // A panic in Little CMS's wrapper must not cross into C++.
    let made = std::panic::catch_unwind(|| tables(bytes, Calmer::from_index(calmer)));
    match made {
        Ok(Ok(Some(tables))) => {
            // SAFETY: the caller gives room for CUBE³ × 4 and CURVE × 4 floats.
            unsafe {
                std::ptr::copy_nonoverlapping(tables.cube.as_ptr(), cube, tables.cube.len());
                std::ptr::copy_nonoverlapping(tables.curve.as_ptr(), curve, tables.curve.len());
            }
            1
        }
        Ok(Ok(None)) => 0,
        Ok(Err(e)) => {
            eprintln!("Sioul: the screen's colour profile: {e}");
            -1
        }
        Err(_) => -1,
    }
}

/// The profile's name, written to `out` as UTF-8 with a final zero, cut to
/// `capacity`; returns its length in bytes without the zero (0: no name).
///
/// # Safety
/// `profile` holds `length` bytes; `out` has room for `capacity` bytes.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_colour_profile_name(profile: *const u8, length: usize, out: *mut u8, capacity: usize) -> usize {
    if profile.is_null() || out.is_null() || capacity == 0 {
        return 0;
    }
    // SAFETY: as the caller says.
    let bytes = unsafe { std::slice::from_raw_parts(profile, length) };
    let name = std::panic::catch_unwind(|| profile_name(bytes)).ok().flatten().unwrap_or_default();
    // Cut on a character's boundary, room kept for the zero.
    let mut end = name.len().min(capacity - 1);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    // SAFETY: `end` < `capacity`.
    unsafe {
        std::ptr::copy_nonoverlapping(name.as_ptr(), out, end);
        *out.add(end) = 0;
    }
    end
}

/// What the screen Sioul's window is on gets: 0 converted (to the profile
/// `name`, UTF-8, may be null), 1 by the desktop, 2 nothing (no profile),
/// 3 turned off.
///
/// # Safety
/// `name` is null or a C string.
// SAFETY: no other symbol of the program has this name.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sioul_colour_said(state: i32, name: *const std::ffi::c_char) {
    // SAFETY: as the caller says.
    let name = if name.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy().into_owned() };
    let said = match state {
        0 => Said::Converted(name),
        1 => Said::Desktop,
        3 => Said::Off,
        _ => Said::Plain,
    };
    if let Ok(mut now) = SAID.lock() {
        *now = said;
    }
}

// Little CMS is the reference: on Linux, where it is built.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use lcms2::{CIExyY, CIExyYTRIPLE};

    /// A float as RGBA16F keeps it: rounded to 11 significant bits.
    fn half(v: f32) -> f32 {
        if v == 0.0 || !v.is_finite() {
            return v;
        }
        let exponent = v.abs().log2().floor().max(-14.0);
        let step = 2f32.powf(exponent - 10.0);
        (v / step).round_ties_even() * step
    }

    /// What the shader gives for 8-bit sRGB numbers, as the GPU computes it:
    /// tables in half floats, the cube read bilinearly in a tile and mixed
    /// along blue, clamped, the curve read linearly at u = y^(1/2.4).
    fn shade(tables: &Tables, rgb: [u8; 3]) -> [f64; 3] {
        let n = CUBE;
        let cube: Vec<f64> = tables.cube.iter().map(|&v| f64::from(half(v))).collect();
        let curve = Tables { cube: Vec::new(), curve: tables.curve.iter().map(|&v| half(v)).collect() };
        let c = rgb.map(|v| f64::from(v) / 255.0 * (n - 1) as f64);
        let (r0, g0, b0) = (c[0].floor().min((n - 2) as f64), c[1].floor().min((n - 2) as f64), c[2].floor().min((n - 2) as f64));
        let (fr, fg, fb) = (c[0] - r0, c[1] - g0, c[2] - b0);
        let texel = |r: usize, g: usize, b: usize, k: usize| cube[((r + b * n) + g * n * n) * 4 + k];
        let (r0, g0, b0) = (r0 as usize, g0 as usize, b0 as usize);
        [0, 1, 2].map(|k| {
            let tile = |b: usize| {
                let low = texel(r0, g0, b, k) * (1.0 - fr) + texel(r0 + 1, g0, b, k) * fr;
                let high = texel(r0, g0 + 1, b, k) * (1.0 - fr) + texel(r0 + 1, g0 + 1, b, k) * fr;
                low * (1.0 - fg) + high * fg
            };
            let linear = tile(b0) * (1.0 - fb) + tile(b0 + 1) * fb;
            curve_at(&curve, k, linear) * 255.0
        })
    }

    /// The colours checked: an 8-bit cube in steps of 17, and loud colours of the web.
    fn colours() -> Vec<[u8; 3]> {
        let mut all = Vec::new();
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    all.push([r as u8, g as u8, b as u8]);
                }
            }
        }
        all.extend([[0x25, 0xd3, 0x66], [0x58, 0x65, 0xf2], [0x42, 0x85, 0xf4], [0xff, 0x8c, 0x00], [0xff, 0x00, 0x33], [0x4c, 0x6b, 0x5c], [1, 2, 3], [252, 8, 4]]);
        all
    }

    /// An RGB profile from its primaries and white (xy) and a curve per channel.
    fn profile(white: (f64, f64), primaries: [(f64, f64); 3], curve: &ToneCurve) -> Vec<u8> {
        let xy = |(x, y): (f64, f64)| CIExyY { x, y, Y: 1.0 };
        let triple = CIExyYTRIPLE { Red: xy(primaries[0]), Green: xy(primaries[1]), Blue: xy(primaries[2]) };
        Profile::new_rgb(&xy(white), &triple, &[curve, curve, curve]).unwrap().icc().unwrap()
    }

    /// Compatible with Adobe RGB (1998): its published primaries and white, gamma 563/256.
    fn adobe_like() -> Vec<u8> {
        profile((0.3127, 0.3290), [(0.64, 0.33), (0.21, 0.71), (0.15, 0.06)], &ToneCurve::new(563.0 / 256.0))
    }

    /// A screen wider than sRGB in green and red, with 256-entry curves
    /// (a gamma of 2.4 with a linear toe), as profiling tools write them.
    fn table_curves() -> Vec<u8> {
        let values: Vec<u16> = (0..256)
            .map(|i| {
                let x = f64::from(i) / 255.0;
                let y = if x < 0.03 { x / 16.0 } else { ((x + 0.04) / 1.04).powf(2.4) };
                (y * 65535.0).round() as u16
            })
            .collect();
        profile((0.3127, 0.3290), [(0.68, 0.32), (0.20, 0.72), (0.15, 0.06)], &ToneCurve::new_tabulated(&values))
    }

    /// Little CMS's own transform, from 8-bit sRGB numbers to the profile's, in 8-bit units.
    fn reference(bytes: &[u8], colours: &[[u8; 3]]) -> Vec<[f64; 3]> {
        let screen = Profile::new_icc(bytes).unwrap();
        let transform = Transform::new(&Profile::new_srgb(), PixelFormat::RGB_FLT, &screen, PixelFormat::RGB_FLT, Intent::RelativeColorimetric).unwrap();
        let input: Vec<[f32; 3]> = colours.iter().map(|c| c.map(|v| f32::from(v) / 255.0)).collect();
        let mut output = vec![[0.0f32; 3]; input.len()];
        transform.transform_pixels(&input, &mut output);
        output.iter().map(|o| o.map(|v| f64::from(v.clamp(0.0, 1.0)) * 255.0)).collect()
    }

    /// The largest and the mean difference, in 8-bit codes.
    fn differences(tables: &Tables, bytes: &[u8]) -> (f64, f64) {
        let colours = colours();
        let want = reference(bytes, &colours);
        let mut worst: f64 = 0.0;
        let mut sum = 0.0;
        for (colour, want) in colours.iter().zip(&want) {
            let got = shade(tables, *colour);
            for k in 0..3 {
                let d = (got[k] - want[k]).abs();
                worst = worst.max(d);
                sum += d;
            }
        }
        (worst, sum / (colours.len() * 3) as f64)
    }

    #[test]
    fn a_screen_like_adobe_rgb_is_within_one_code_of_little_cms() {
        let bytes = adobe_like();
        let tables = tables(Some(&bytes), Calmer::Off).unwrap().expect("a wide screen changes colours");
        let (worst, mean) = differences(&tables, &bytes);
        assert!(worst <= 1.0 && mean < 0.1, "worst {worst:.3}, mean {mean:.3}");
    }

    #[test]
    fn table_curves_are_within_one_code_of_little_cms() {
        let bytes = table_curves();
        let tables = tables(Some(&bytes), Calmer::Off).unwrap().unwrap();
        let (worst, mean) = differences(&tables, &bytes);
        assert!(worst <= 1.0 && mean < 0.1, "worst {worst:.3}, mean {mean:.3}");
    }

    #[test]
    fn an_srgb_screen_changes_nothing() {
        let srgb = Profile::new_srgb().icc().unwrap();
        assert!(tables(Some(&srgb), Calmer::Off).unwrap().is_none());
        assert!(tables(None, Calmer::Off).unwrap().is_none());
        // An sRGB profile made from sRGB's own primaries and curve, as a desktop writes one.
        let curve = ToneCurve::new_parametric(4, &[2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045]).unwrap();
        let made = profile((0.3127, 0.3290), [(0.64, 0.33), (0.30, 0.60), (0.15, 0.06)], &curve);
        assert!(tables(Some(&made), Calmer::Off).unwrap().is_none());
    }

    #[test]
    fn unreadable_profiles_are_refused() {
        assert!(tables(Some(b"not a profile"), Calmer::Off).is_err());
        let grey = Profile::new_gray(&CIExyY { x: 0.3127, y: 0.3290, Y: 1.0 }, &ToneCurve::new(2.2)).unwrap().icc().unwrap();
        assert!(tables(Some(&grey), Calmer::Off).is_err());
    }

    #[test]
    fn a_profile_of_tables_goes_through_little_cms() {
        // Little CMS's own Lab profile is no screen; a device link would not
        // be read either. A profile of tables is made from the Adobe-like one
        // by Little CMS's own writer, keeping its matrix and adding tables.
        let bytes = adobe_like();
        let screen = screen_of(&bytes).unwrap();
        assert!(matches!(screen, Screen::Matrix { .. }));
        let transform = Transform::new(&Profile::new_srgb(), PixelFormat::RGB_FLT, &Profile::new_icc(&bytes).unwrap(), PixelFormat::RGB_FLT, Intent::RelativeColorimetric).unwrap();
        let tables = make(&Screen::Tables(transform), Calmer::Off);
        // The cube holds the screen's numbers there: near-black colours err
        // most, a few codes at most (docs/colour.md, "Profiles of tables").
        let (worst, mean) = differences(&tables, &bytes);
        assert!(worst <= 4.0 && mean < 0.2, "worst {worst:.3}, mean {mean:.3}");
    }

    #[test]
    fn calmer_colours_soften_loud_colours_and_keep_greys() {
        let little = tables(None, Calmer::Little).unwrap().unwrap();
        let more = tables(None, Calmer::More).unwrap().unwrap();
        let linear = |rgb: [f64; 3]| rgb.map(|v| srgb_decode(v / 255.0));
        let hue = |lin: [f64; 3]| {
            let lab = oklab(lin);
            lab[2].atan2(lab[1]).to_degrees()
        };
        for colour in colours() {
            let before = linear(colour.map(f64::from));
            for (tables, limit) in [(&little, 0.12), (&more, 0.08)] {
                let after = linear(shade(tables, colour));
                let (c0, c1) = (chroma(before), chroma(after));
                // Within half a code's worth of chroma of the soft limit.
                assert!(c1 <= limit * (c0 / limit).tanh() + 0.004, "{colour:?}: {c0:.3} -> {c1:.3}");
                // Lightness kept within 1 %; hue within 2° wherever there is some colour left.
                let (l0, l1) = (oklab(before)[0], oklab(after)[0]);
                assert!((l0 - l1).abs() < 0.01, "{colour:?}: lightness {l0:.3} -> {l1:.3}");
                if c1 > 0.02 {
                    let turn = (hue(before) - hue(after) + 540.0) % 360.0 - 180.0;
                    assert!(turn.abs() < 2.0, "{colour:?}: hue turned {turn:.2}°");
                }
            }
            // Greys stay grey, within half a code.
            if colour[0] == colour[1] && colour[1] == colour[2] {
                for tables in [&little, &more] {
                    let got = shade(tables, colour);
                    assert!(got.iter().all(|v| (v - f64::from(colour[0])).abs() < 0.5), "{colour:?} -> {got:?}");
                }
            }
        }
        // A pure red loses about half its chroma with "A little", more with "More".
        let red = linear([255.0, 0.0, 0.0]);
        let soft = |t: &Tables| chroma(linear(shade(t, [255, 0, 0]))) / chroma(red);
        assert!((0.4..0.55).contains(&soft(&little)) && soft(&more) < soft(&little), "{} {}", soft(&little), soft(&more));
    }

    #[test]
    fn calmer_colours_on_a_wide_screen_are_both_at_once() {
        // Calmer, then converted: the same as Little CMS converting the softened colour.
        let bytes = adobe_like();
        let tables = tables(Some(&bytes), Calmer::Little).unwrap().unwrap();
        let screen = Profile::new_icc(&bytes).unwrap();
        let transform = Transform::new(&Profile::new_srgb(), PixelFormat::RGB_FLT, &screen, PixelFormat::RGB_FLT, Intent::RelativeColorimetric).unwrap();
        let mut worst: f64 = 0.0;
        for colour in colours() {
            let soft = soften(colour.map(|v| srgb_decode(f64::from(v) / 255.0)), 0.12).map(|v| srgb_encode(v) as f32);
            let mut want = [[0.0f32; 3]];
            transform.transform_pixels(&[soft], &mut want);
            let got = shade(&tables, colour);
            for k in 0..3 {
                worst = worst.max((got[k] - f64::from(want[0][k].clamp(0.0, 1.0)) * 255.0).abs());
            }
        }
        assert!(worst <= 1.0, "worst {worst:.3}");
    }

    #[test]
    fn the_c_functions_write_what_they_say() {
        let bytes = adobe_like();
        let mut cube = vec![0.0f32; CUBE * CUBE * CUBE * 4];
        let mut curve = vec![0.0f32; CURVE * 4];
        // SAFETY: the sizes given are those of the vectors.
        let written = unsafe { sioul_colour_tables(bytes.as_ptr(), bytes.len(), 0, CUBE, cube.as_mut_ptr(), CURVE, curve.as_mut_ptr()) };
        assert_eq!(written, 1);
        assert_eq!(cube[3], 1.0, "alpha is 1");
        assert!(curve[(CURVE - 1) * 4] > 0.99);
        // SAFETY: as above.
        assert_eq!(unsafe { sioul_colour_tables(std::ptr::null(), 0, 0, CUBE, cube.as_mut_ptr(), CURVE, curve.as_mut_ptr()) }, 0);
        // SAFETY: as above.
        assert_eq!(unsafe { sioul_colour_tables(b"junk".as_ptr(), 4, 0, CUBE, cube.as_mut_ptr(), CURVE, curve.as_mut_ptr()) }, -1);
        // SAFETY: wrong sizes are refused before anything is written.
        assert_eq!(unsafe { sioul_colour_tables(bytes.as_ptr(), bytes.len(), 0, 17, cube.as_mut_ptr(), CURVE, curve.as_mut_ptr()) }, -2);
        let mut name = [0u8; 8];
        // A profile made by Little CMS has a name; cut to the room given, with its zero.
        // SAFETY: `name` has 8 bytes.
        let length = unsafe { sioul_colour_profile_name(bytes.as_ptr(), bytes.len(), name.as_mut_ptr(), name.len()) };
        assert!(length <= 7 && name[length] == 0);
        // SAFETY: a C string.
        unsafe { sioul_colour_said(0, c"Screen".as_ptr()) };
        assert_eq!(said(), Said::Converted("Screen".into()));
        // SAFETY: null is allowed.
        unsafe { sioul_colour_said(1, std::ptr::null()) };
        assert_eq!(said(), Said::Desktop);
    }
}
