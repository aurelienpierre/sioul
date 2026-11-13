// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sounds to rest by or to focus with (docs/sounds.md), all made here, nothing
//! downloaded, nothing to license, each looping without a seam (its end faded
//! into its start):
//! - **noise** for focus: white, pink, brown; a minute each, mono;
//! - **nature** to rest by: waves on a beach, rain, wind in the trees, crickets
//!   at night, a distant storm, birds at dawn; one to three minutes each, in
//!   stereo at 22 kHz (what they hold lies below 10 kHz, and the files stay
//!   small). Filtered noise shaped by waves, gusts and drops, crickets' chirps
//!   as pulsed tones, birds' notes as gliding whistles, each sound seeded the
//!   same every time. The waves, the wind, the storm and the birds were tuned
//!   against CC0 recordings, measure by measure (docs/sounds.md,
//!   tools/sounds/): only the recipes are kept, never the recordings.
//!
//! Recordings of your own are your files, in the notes' `sounds` folder.

use std::path::{Path, PathBuf};

/// The noises made here.
pub const NOISES: &[&str] = &["white", "pink", "brown"];
/// The sounds of nature made here.
pub const NATURE: &[&str] = &["waves", "rain", "wind", "crickets", "storm", "birds"];

const RATE: u32 = 44_100;
const SECONDS: usize = 60;
/// The seam's crossfade, in samples: two seconds.
const FADE: usize = 2 * RATE as usize;

/// A small, seeded random source: the same noise each time.
struct Random(u64);

impl Random {
    /// A number in [-1, 1).
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        // The top 24 bits, as [0, 1), then centred.
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }

    /// A number in [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() + 1.0) * 0.5
    }

    /// A number in [low, high).
    fn between(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}

/// `count` samples of a noise: white (flat), pink (−3 dB per octave, Paul
/// Kellett's filter), brown (−6 dB per octave, a leaky integration).
fn noise(kind: &str, count: usize) -> Vec<f32> {
    let mut random = Random(0x5105_1d00_c0ff_ee11);
    let (mut b0, mut b1, mut b2, mut b3, mut b4, mut b5, mut b6) = (0f32, 0f32, 0f32, 0f32, 0f32, 0f32, 0f32);
    let mut last = 0f32;
    (0..count)
        .map(|_| {
            let white = random.next();
            match kind {
                "pink" => {
                    b0 = 0.99886 * b0 + white * 0.0555179;
                    b1 = 0.99332 * b1 + white * 0.0750759;
                    b2 = 0.96900 * b2 + white * 0.1538520;
                    b3 = 0.86650 * b3 + white * 0.3104856;
                    b4 = 0.55000 * b4 + white * 0.5329522;
                    b5 = -0.7616 * b5 - white * 0.0168980;
                    let pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362;
                    b6 = white * 0.115926;
                    pink * 0.11
                }
                "brown" => {
                    last = (last + 0.02 * white) / 1.02;
                    last * 3.5
                }
                _ => white,
            }
        })
        .collect()
}

/// A noise of `seconds` that loops: its last two seconds faded into its first,
/// with equal power, so the end flows into the start. Quiet: about −20 dBFS.
pub fn looped(kind: &str, seconds: usize) -> Vec<i16> {
    let length = seconds * RATE as usize;
    let raw = noise(kind, length + FADE);
    let mut out: Vec<f32> = raw[..length].to_vec();
    for (i, sample) in out.iter_mut().take(FADE).enumerate() {
        let t = i as f32 / FADE as f32;
        *sample = raw[i] * t.sqrt() + raw[length + i] * (1.0 - t).sqrt();
    }
    let rms = (out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32).sqrt().max(1e-6);
    let gain = 0.1 / rms;
    out.iter().map(|s| (s * gain).clamp(-1.0, 1.0)).map(|s| (s * f32::from(i16::MAX)) as i16).collect()
}

/// 16-bit mono WAV bytes, at the noises' rate.
fn wav(samples: &[i16]) -> Vec<u8> {
    wav_of(samples, 1, RATE)
}

/// 16-bit WAV bytes: `channels` interleaved, at `rate`.
fn wav_of(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
    let data = u32::try_from(samples.len() * 2).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend(b"RIFF");
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(channels.to_le_bytes());
    out.extend(rate.to_le_bytes());
    out.extend((rate * 2 * u32::from(channels)).to_le_bytes());
    out.extend((2 * channels).to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    for sample in samples {
        out.extend(sample.to_le_bytes());
    }
    out
}

/// Whether a file of the sounds' folder was made by an older recipe:
/// "waves-4.wav", "pink-noise-3.wav"; never another file.
fn older_recipe(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".wav") else { return false };
    let Some((kind, version)) = stem.rsplit_once('-') else { return false };
    let kind = kind.strip_suffix("-noise").filter(|kind| NOISES.contains(kind)).or_else(|| NATURE.contains(&kind).then_some(kind));
    kind.is_some() && version.parse::<u32>().is_ok_and(|version| version < NATURE_VERSION)
}

/// The file of a noise or a sound of nature, made the first time in `folder`.
pub fn noise_file(folder: &Path, kind: &str) -> Result<PathBuf, String> {
    let (name, bytes): (String, fn(&str) -> Vec<u8>) = if NOISES.contains(&kind) {
        (format!("{kind}-noise-{NATURE_VERSION}.wav"), |kind| wav(&looped(kind, SECONDS)))
    } else if NATURE.contains(&kind) {
        (format!("{kind}-{NATURE_VERSION}.wav"), |kind| {
            let (left, right) = nature(kind);
            let interleaved: Vec<i16> = left.iter().zip(&right).flat_map(|(l, r)| [*l, *r]).collect();
            wav_of(&interleaved, 2, NATURE_RATE)
        })
    } else {
        return Err(format!("{kind}: no such sound"));
    };
    let path = folder.join(name);
    if !path.exists() {
        // The files of older recipes go: each is minutes of sound.
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                if older_recipe(&entry.file_name().to_string_lossy()) {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let temporary = path.with_extension("wav.new");
        std::fs::write(&temporary, bytes(kind)).map_err(|e| format!("{}: {e}", temporary.display()))?;
        std::fs::rename(&temporary, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(path)
}

// Nature.

/// The sounds of nature's rate: what they hold lies below 10 kHz.
const NATURE_RATE: u32 = 22_050;
/// Raised when the recipes change, so the files are made again and the older
/// ones taken away (2: the noises were offset, their random source not
/// centred; 3: crickets held in tune; 4: warmer waves; 5: the waves, the wind
/// and the storm tuned against recordings, and the birds).
const NATURE_VERSION: u32 = 5;

/// A one-pole low-pass filter, its cutoff free to move each sample.
#[derive(Default, Clone, Copy)]
struct LowPass(f32);

impl LowPass {
    fn run(&mut self, x: f32, cutoff: f32) -> f32 {
        let a = 1.0 - (-2.0 * std::f32::consts::PI * cutoff / NATURE_RATE as f32).exp();
        self.0 += a * (x - self.0);
        self.0
    }
}

/// A slow, smooth random curve in [0, 1]: a new point every `every` seconds,
/// joined by cosines; the same for every sample count.
fn slow_curve(random: &mut Random, count: usize, every: f32) -> Vec<f32> {
    let step = (every * NATURE_RATE as f32).max(1.0) as usize;
    let points: Vec<f32> = (0..count / step + 3).map(|_| random.unit()).collect();
    (0..count)
        .map(|i| {
            let (k, t) = (i / step, (i % step) as f32 / step as f32);
            let w = (1.0 - (t * std::f32::consts::PI).cos()) * 0.5;
            points[k] * (1.0 - w) + points[k + 1] * w
        })
        .collect()
}

/// A sound that fades out quickly: a tone of `frequency` decaying in `decay`
/// seconds, added at `at` with `amplitude`.
fn add_ping(out: &mut [f32], at: usize, frequency: f32, decay: f32, amplitude: f32, phase: f32) {
    let length = (decay * 5.0 * NATURE_RATE as f32) as usize;
    let w = 2.0 * std::f32::consts::PI * frequency / NATURE_RATE as f32;
    for (n, sample) in out.iter_mut().skip(at).take(length).enumerate() {
        let t = n as f32 / NATURE_RATE as f32;
        *sample += amplitude * (-t / decay).exp() * (w * n as f32 + phase).sin();
    }
}

/// Times of events, `rate` a second on average, at random (a Poisson process).
fn events(random: &mut Random, count: usize, rate: f32) -> Vec<usize> {
    let mut at = 0.0f32;
    let mut out = Vec::new();
    loop {
        at += -(1.0 - random.unit()).max(1e-6).ln() / rate * NATURE_RATE as f32;
        if at as usize >= count {
            return out;
        }
        out.push(at as usize);
    }
}

/// A one-pole high-pass filter: what a low-pass leaves out.
#[derive(Default, Clone, Copy)]
struct HighPass(LowPass);

impl HighPass {
    fn run(&mut self, x: f32, cutoff: f32) -> f32 {
        x - self.0.run(x, cutoff)
    }
}

/// One wave breaking on the beach: when (seconds), how long it rises, how
/// long its crash and its wash last, how strong it is, and where along the
/// beach it breaks (−1 left, 1 right).
#[derive(Clone, Copy)]
struct Break {
    at: f32,
    rise: f32,
    crash: f32,
    wash: f32,
    strength: f32,
    pan: f32,
}

/// A wave breaking, added to both ears from where it breaks: a quick rise,
/// a crash bright at its peak that darkens as it falls, then the wash running
/// up and back, a fizz of foam and bubbles, with a few pebbles knocking.
fn add_break(left: &mut [f32], right: &mut [f32], wave: &Break, seed: u64) {
    let rate = NATURE_RATE as f32;
    let mut random = Random(seed);
    // Where each of its samples lands: a wave that began before the loop adds only its end.
    let first = (wave.at * rate) as isize;
    let size = left.len();
    let place = |n: usize| usize::try_from(first + n as isize).ok().filter(|at| *at < size);
    let length = ((wave.rise + wave.wash * 5.0) * rate) as usize;
    let (mut dark, mut darker, mut floor) = (LowPass::default(), LowPass::default(), HighPass::default());
    let (mut foam_floor, mut foam_top, mut foam_top2, mut bubbles) = (HighPass::default(), LowPass::default(), LowPass::default(), LowPass::default());
    let (mut own_floor, mut own_top, mut own_top2) = (HighPass::default(), LowPass::default(), LowPass::default());
    let angle = (wave.pan + 1.0) * std::f32::consts::FRAC_PI_4;
    let (to_left, to_right) = (angle.cos(), angle.sin());
    let mut sound = vec![(0.0f32, 0.0f32); length];
    for (n, sample) in sound.iter_mut().enumerate() {
        let (white, other) = (random.next(), random.next());
        let t = n as f32 / rate;
        // A rounded rise, then the fall: bright in the middle, narrow at both ends.
        let crash = if t < wave.rise { (std::f32::consts::FRAC_PI_2 * t / wave.rise).sin().powi(4) } else { (-(t - wave.rise) / wave.crash).exp() };
        let washing = if t < wave.rise { 0.0 } else { let after = t - wave.rise; (1.0 - (-after / 0.4).exp()) * (-after / wave.wash).exp() };
        let cutoff = 280.0 + 1500.0 * crash;
        let body = floor.run(darker.run(dark.run(white, cutoff), cutoff * 1.6), 170.0);
        // Bubbles: the foam's level jumping many times a second.
        let bubbling = (bubbles.run(other.abs(), 22.0) * 4.0).powi(2);
        // The foam each ear hears is partly its own: it fizzes all along the beach.
        let fizz = foam_top2.run(foam_top.run(foam_floor.run(white, 500.0), 2400.0), 3000.0);
        let own = own_top2.run(own_top.run(own_floor.run(other, 500.0), 2400.0), 3000.0);
        let body = wave.strength * body * crash * 2.2;
        let foam = wave.strength * washing * bubbling * 0.35;
        *sample = (body * to_left + foam * (fizz * 0.6 * to_left + own * 0.5), body * to_right + foam * (fizz * 0.6 * to_right - own * 0.5));
    }
    // Pebbles knocked by the wash: short high clicks, more while it runs.
    let mut pebbles = vec![0.0f32; length];
    for at in events(&mut random, length, 45.0) {
        let t = at as f32 / rate;
        let washing = if t < wave.rise { 0.0 } else { (-(t - wave.rise) / wave.wash).exp() };
        if random.unit() < washing {
            let (frequency, decay, amplitude, phase) = (random.between(1800.0, 5500.0), random.between(0.0008, 0.003), random.between(0.01, 0.05) * wave.strength, random.between(0.0, 6.28));
            add_ping(&mut pebbles, at, frequency, decay, amplitude, phase);
        }
    }
    for (n, ((l, r), pebble)) in sound.iter().zip(&pebbles).enumerate() {
        if let Some(at) = place(n) {
            left[at] += l + pebble * to_left;
            right[at] += r + pebble * to_right;
        }
    }
}

/// The surf of the rest of the beach, under the breaks: pink noise between
/// 80 Hz and 1.8 kHz, its level wandering over seconds and churning many
/// times a second.
fn surf(count: usize, seed: u64) -> Vec<f32> {
    let mut random = Random(seed);
    let level = slow_curve(&mut random, count, 3.5);
    let churn = slow_curve(&mut random, count, 0.06);
    let fast = slow_curve(&mut random, count, 0.025);
    let mut pink = Pink::default();
    let (mut floor, mut top, mut top2) = (HighPass::default(), LowPass::default(), LowPass::default());
    let (mut foam_floor, mut foam_top, mut foam_top2) = (HighPass::default(), LowPass::default(), LowPass::default());
    (0..count)
        .map(|i| {
            let p = pink.next(random.next());
            let shaped = top2.run(top.run(floor.run(p, 140.0), 1800.0), 2600.0);
            // The foam fizzing all along the beach, between the breaks too.
            let foam = foam_top2.run(foam_top.run(foam_floor.run(random.next(), 900.0), 2800.0), 3500.0);
            (shaped + foam * 0.22) * (0.55 + 0.45 * level[i]) * (0.4 + 0.9 * churn[i]) * (0.5 + 1.0 * fast[i])
        })
        .collect()
}

/// Waves on a beach (after five CC0 recordings, docs/sounds.md): a wave
/// breaking every 2.5 to 6.5 seconds, small and large, each somewhere along
/// the beach, over the surf of the rest of it, which both ears share in part.
fn waves(count: usize, plan: &mut Random) -> (Vec<f32>, Vec<f32>) {
    let rate = NATURE_RATE as f32;
    let (shared, own_left, own_right) = (surf(count, 13), surf(count, 14), surf(count, 15));
    let mut left: Vec<f32> = shared.iter().zip(&own_left).map(|(s, o)| (s * 0.8 + o * 0.6) * 0.9).collect();
    let mut right: Vec<f32> = shared.iter().zip(&own_right).map(|(s, o)| (s * 0.8 + o * 0.6) * 0.9).collect();
    let mut at = -3.0f32;
    let mut seed = 100;
    while at < count as f32 / rate {
        let strength = plan.between(0.45, 1.0);
        let wave = Break { at, rise: plan.between(0.35, 0.95), crash: plan.between(0.4, 0.9), wash: plan.between(0.7, 1.5), strength, pan: plan.between(-0.6, 0.6) };
        add_break(&mut left, &mut right, &wave, seed);
        seed += 1;
        at += plan.between(2.5, 6.5);
    }
    (left, right)
}

/// Rain: a soft hiss, many small drops, a few larger ones on leaves and stones.
fn rain(count: usize, seed: u64, heavy: f32) -> Vec<f32> {
    let mut random = Random(seed);
    let mut out = vec![0.0f32; count];
    let (mut low, mut high) = (LowPass::default(), LowPass::default());
    let level = slow_curve(&mut random, count, 6.0);
    let mut pink = Pink::default();
    for (i, sample) in out.iter_mut().enumerate() {
        let p = pink.next(random.next());
        let band = high.run(p, 6500.0) - low.run(p, 450.0);
        *sample = band * (0.75 + 0.25 * level[i]) * 0.9 * heavy;
    }
    for at in events(&mut random, count, 160.0 * heavy) {
        let (frequency, decay, amplitude, phase) = (random.between(2400.0, 7000.0), random.between(0.0012, 0.004), 0.08 * -(1.0 - random.unit()).max(1e-4).ln().min(4.0), random.between(0.0, 6.28));
        add_ping(&mut out, at, frequency, decay, amplitude, phase);
    }
    for at in events(&mut random, count, 5.0 * heavy) {
        let (frequency, decay, amplitude, phase) = (random.between(700.0, 2400.0), random.between(0.008, 0.022), random.between(0.05, 0.14), random.between(0.0, 6.28));
        add_ping(&mut out, at, frequency, decay, amplitude, phase);
    }
    out
}

/// Pink noise, one channel: Paul Kellett's filter, as `noise` uses it.
#[derive(Default)]
struct Pink([f32; 7]);

impl Pink {
    fn next(&mut self, white: f32) -> f32 {
        let b = &mut self.0;
        b[0] = 0.99886 * b[0] + white * 0.0555179;
        b[1] = 0.99332 * b[1] + white * 0.0750759;
        b[2] = 0.96900 * b[2] + white * 0.1538520;
        b[3] = 0.86650 * b[3] + white * 0.3104856;
        b[4] = 0.55000 * b[4] + white * 0.5329522;
        b[5] = -0.7616 * b[5] - white * 0.0168980;
        let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362;
        b[6] = white * 0.115926;
        pink * 0.11
    }
}

/// One layer of wind, one ear: pink noise between `low` and `high` hertz
/// (two poles at the top), its level following the gusts to the power
/// `follow`, fluttering `flutter` deep a few times a second.
fn wind_layer(random: &mut Random, gusts: &[f32], low: f32, high: f32, follow: f32, flutter: f32) -> Vec<f32> {
    let count = gusts.len();
    let shiver = slow_curve(random, count, 0.07);
    let quiver = slow_curve(random, count, 0.03);
    let mut pink = Pink::default();
    let (mut floor, mut top, mut top2) = (HighPass::default(), LowPass::default(), LowPass::default());
    (0..count)
        .map(|i| {
            let p = top2.run(top.run(floor.run(pink.next(random.next()), low), high), high * 1.4);
            let flutter = 1.0 - flutter + flutter * (shiver[i] + quiver[i]);
            p * gusts[i].powf(follow) * flutter
        })
        .collect()
}

/// Wind in the trees (after six CC0 recordings, docs/sounds.md): close to pink
/// noise from 60 Hz to 8 kHz, louder and brighter in the gusts. The branches'
/// low roar is nearly the same in both ears; the air's whoosh in part; the
/// leaves, all around, flutter differently in each.
fn wind(count: usize, plan: &mut Random) -> (Vec<f32>, Vec<f32>) {
    let slow = slow_curve(plan, count, 12.0);
    let gust = slow_curve(plan, count, 4.0);
    let puff = slow_curve(plan, count, 1.8);
    let gusts: Vec<f32> = (0..count).map(|i| 0.12 + 0.88 * (0.08 + 0.22 * slow[i] + 0.45 * slow[i] * gust[i] + 0.15 * gust[i] + 0.15 * puff[i] * gust[i]).min(1.0)).collect();
    let mut random = Random(31);
    let roar = wind_layer(&mut random, &gusts, 60.0, 420.0, 1.0, 0.35);
    let (roar_left, roar_right) = (wind_layer(&mut random, &gusts, 60.0, 420.0, 1.0, 0.35), wind_layer(&mut random, &gusts, 60.0, 420.0, 1.0, 0.35));
    let air = wind_layer(&mut random, &gusts, 300.0, 1600.0, 1.3, 0.3);
    let (air_left, air_right) = (wind_layer(&mut random, &gusts, 300.0, 1600.0, 1.3, 0.3), wind_layer(&mut random, &gusts, 300.0, 1600.0, 1.3, 0.3));
    let leaves = wind_layer(&mut random, &gusts, 1800.0, 9000.0, 1.7, 0.55);
    let (leaves_left, leaves_right) = (wind_layer(&mut random, &gusts, 1800.0, 9000.0, 1.7, 0.55), wind_layer(&mut random, &gusts, 1800.0, 9000.0, 1.7, 0.55));
    let mix = |own_roar: &[f32], own_air: &[f32], own_leaves: &[f32]| -> Vec<f32> {
        (0..count).map(|i| (roar[i] * 0.95 + own_roar[i] * 0.3) * 0.55 + (air[i] * 0.85 + own_air[i] * 0.5) * 0.9 + (leaves[i] * 0.4 + own_leaves[i] * 0.9) * 1.35).collect()
    };
    let (mut left, mut right) = (mix(&roar_left, &air_left, &leaves_left), mix(&roar_right, &air_right, &leaves_right));
    // A twig's crackle now and then in the stronger gusts.
    for (channel, seed) in [(&mut left, 33u64), (&mut right, 34)] {
        let mut random = Random(seed);
        for at in events(&mut random, count, 160.0) {
            if random.unit() < gusts[at].powi(2) * 0.6 {
                let (frequency, decay, amplitude, phase) = (random.between(2800.0, 7500.0), random.between(0.0004, 0.0012), random.between(0.02, 0.06), random.between(0.0, 6.28));
                add_ping(channel, at, frequency, decay, amplitude, phase);
            }
        }
    }
    (left, right)
}

/// One cricket of a night: where it sits (its weight in this ear), its pitch,
/// its rhythm.
struct Cricket {
    gain: f32,
    pitch: f32,
    period: f32,
    pulses: usize,
    offset: f32,
}

/// Crickets at night: a few field crickets chirping, each its pitch and
/// rhythm, near and far, one tree cricket trilling softly; the night's air under them.
fn crickets(count: usize, seed: u64, band: &[Cricket], trill_gain: f32) -> Vec<f32> {
    let mut random = Random(seed);
    let rate = NATURE_RATE as f32;
    let mut air = LowPass::default();
    let mut out: Vec<f32> = (0..count).map(|_| air.run(random.next(), 120.0) * 0.25).collect();
    let pulse = 0.014f32;
    let gap = 0.016f32;
    for cricket in band {
        let mut chirp = cricket.offset;
        while ((chirp * rate) as usize) < count {
            for k in 0..cricket.pulses {
                let start = chirp + k as f32 * (pulse + gap);
                let first = (start * rate) as usize;
                let length = (pulse * rate) as usize;
                // The phase counted from the pulse's start: exact in single precision.
                let mut phase = 0.0f32;
                for n in 0..length {
                    let Some(sample) = out.get_mut(first + n) else { break };
                    let t = n as f32 / length as f32;
                    let window = (std::f32::consts::PI * t).sin().powi(2);
                    let pitch = cricket.pitch * (1.0 - 0.015 * t);
                    phase += 2.0 * std::f32::consts::PI * pitch / rate;
                    *sample += cricket.gain * window * phase.sin();
                }
            }
            chirp += cricket.period * random.between(0.95, 1.05);
        }
    }
    // A tree cricket: a soft trill at 2.8 kHz, 40 pulses a second, swelling and resting.
    let swell = slow_curve(&mut random, count, 3.5);
    for (i, sample) in out.iter_mut().enumerate() {
        // In double precision: a minute of a 2.8 kHz tone is a million radians.
        let t = i as f64 / f64::from(NATURE_RATE);
        let beat = ((2.0 * std::f64::consts::PI * 40.0 * t).sin().max(0.0).powi(2)) as f32;
        let on = (swell[i] - 0.35).max(0.0) / 0.65;
        *sample += trill_gain * on * beat * (2.0 * std::f64::consts::PI * 2800.0 * t).sin() as f32;
    }
    out
}

/// One strike of thunder far away, added to both ears from where it is: a
/// few bursts within seconds, rolling, low; brighter at its start the nearer
/// it is (`near`, 0 to 1).
fn add_thunder(left: &mut [f32], right: &mut [f32], at: f32, near: f32, pan: f32, seed: u64) {
    let rate = NATURE_RATE as f32;
    let mut random = Random(seed);
    let mut bursts = Vec::new();
    let mut offset = 0.0f32;
    let mut strength = 1.0f32;
    for _ in 0..(2 + (random.unit() * 5.0) as usize) {
        bursts.push((offset, random.between(0.06, 0.4), random.between(0.6, 2.2), strength));
        offset += random.between(0.3, 2.5);
        strength *= random.between(0.55, 1.05);
    }
    let length = ((offset + 12.0) * rate) as usize;
    let rolling = slow_curve(&mut random, length, 0.12);
    let rumbling = slow_curve(&mut random, length, 0.04);
    let (mut brown, mut dark, mut darker, mut floor) = (0.0f32, LowPass::default(), LowPass::default(), HighPass::default());
    let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
    let (to_left, to_right) = (angle.cos(), angle.sin());
    let start = (at * rate) as usize;
    let (base, bright) = (110.0 + 220.0 * near, 300.0 + 2600.0 * near * near);
    for n in 0..length {
        if start + n >= left.len() {
            break;
        }
        let t = n as f32 / rate;
        let mut level = 0.0f32;
        for &(begin, rise, fall, strength) in &bursts {
            let since = t - begin;
            if since >= 0.0 {
                level += strength * if since < rise { since / rise } else { (-(since - rise) / fall).exp() };
            }
        }
        let white = random.next();
        brown = (brown + 0.02 * white) / 1.02;
        let cutoff = base + (bright - base) * (-t / 0.9).exp();
        let rumble = floor.run(darker.run(dark.run(brown * 3.5 + white * 0.15, cutoff), cutoff * 1.3), 30.0);
        let value = rumble * level * (0.45 + 0.7 * rolling[n]) * (0.5 + 0.8 * rumbling[n]) * (0.5 + near);
        left[start + n] += value * to_left;
        right[start + n] += value * to_right;
    }
}

/// The rain of a storm far away: a soft, dark hiss, its level wandering over
/// tens of seconds, fewer and softer drops than `rain`.
fn storm_rain(count: usize, seed: u64, level: &[f32]) -> Vec<f32> {
    let mut random = Random(seed);
    let flutter = slow_curve(&mut random, count, 0.05);
    let patter = slow_curve(&mut random, count, 0.02);
    let mut pink = Pink::default();
    let (mut floor, mut top) = (HighPass::default(), LowPass::default());
    let mut out: Vec<f32> = (0..count)
        .map(|i| {
            let p = top.run(floor.run(pink.next(random.next()), 250.0), 3500.0);
            p * (0.35 + 0.65 * level[i]) * (0.5 + 0.7 * flutter[i]) * (0.6 + 0.8 * patter[i])
        })
        .collect();
    for at in events(&mut random, count, 90.0) {
        let (frequency, decay, amplitude, phase) = (random.between(1200.0, 4500.0), random.between(0.0006, 0.002), 0.07 * level[at] * -(1.0 - random.unit()).max(1e-4).ln().min(4.0), random.between(0.0, 6.28));
        add_ping(&mut out, at, frequency, decay, amplitude, phase);
    }
    for at in events(&mut random, count, 3.0) {
        let (frequency, decay, amplitude, phase) = (random.between(600.0, 2000.0), random.between(0.008, 0.02), random.between(0.02, 0.06) * level[at], random.between(0.0, 6.28));
        add_ping(&mut out, at, frequency, decay, amplitude, phase);
    }
    out
}

/// A distant storm (after six CC0 recordings, docs/sounds.md): soft rain,
/// its level wandering, and thunder every 8 to 28 seconds, each strike rolling
/// in a few bursts, the same in both ears from where it is.
fn storm(count: usize, plan: &mut Random) -> (Vec<f32>, Vec<f32>) {
    let rate = NATURE_RATE as f32;
    let level = slow_curve(plan, count, 15.0);
    let (shared, own_left, own_right) = (storm_rain(count, 51, &level), storm_rain(count, 52, &level), storm_rain(count, 53, &level));
    let mut left: Vec<f32> = shared.iter().zip(&own_left).map(|(s, o)| (s * 0.8 + o * 0.55) * 0.1).collect();
    let mut right: Vec<f32> = shared.iter().zip(&own_right).map(|(s, o)| (s * 0.8 + o * 0.55) * 0.1).collect();
    // Thunder too far to make out, rolling on under the rain.
    let far = slow_curve(plan, count, 6.0);
    let (mut deep, mut deeper) = (LowPass::default(), LowPass::default());
    let mut random = Random(55);
    let mut brown = 0.0f32;
    for i in 0..count {
        brown = (brown + 0.02 * random.next()) / 1.02;
        let value = deeper.run(deep.run(brown * 3.5, 160.0), 220.0) * (0.35 + 0.65 * far[i]) * 0.08;
        left[i] += value;
        right[i] += value;
    }
    let mut at = plan.between(-6.0, 4.0);
    let mut seed = 200;
    while at < count as f32 / rate {
        add_thunder(&mut left, &mut right, at.max(0.0), plan.unit().powi(2), plan.between(-0.5, 0.5), seed);
        seed += 1;
        at += plan.between(8.0, 28.0);
    }
    (left, right)
}

/// One note of a bird: a whistle at `at` seconds, gliding from `from` to
/// `to` hertz over `length` seconds, arched by `arch` (a fraction of `from`),
/// buzzing by `buzz` (a fraction of its pitch) `rate` times a second, with
/// its `gain`, from where the bird sits (`pan`, −1 left, 1 right).
#[derive(Clone, Copy)]
struct Note {
    at: f32,
    length: f32,
    from: f32,
    to: f32,
    arch: f32,
    buzz: f32,
    rate: f32,
    gain: f32,
    pan: f32,
}

/// A bird's note added to both ears: a quick attack, a soft second harmonic.
fn add_note(left: &mut [f32], right: &mut [f32], note: &Note) {
    let sample_rate = NATURE_RATE as f32;
    let start = (note.at * sample_rate) as usize;
    let count = (note.length * sample_rate) as usize;
    let angle = (note.pan + 1.0) * std::f32::consts::FRAC_PI_4;
    let (to_left, to_right) = (angle.cos(), angle.sin());
    let attack = (0.006 * sample_rate).min(count as f32 * 0.3);
    let mut phase = 0.0f32;
    for n in 0..count {
        let Some(at) = start.checked_add(n).filter(|at| *at < left.len()) else { break };
        let t = n as f32 / count as f32;
        let frequency = (note.from + (note.to - note.from) * t + note.arch * note.from * (std::f32::consts::PI * t).sin()) * (1.0 + note.buzz * (std::f32::consts::TAU * note.rate * n as f32 / sample_rate).sin());
        phase = (phase + std::f32::consts::TAU * frequency / sample_rate) % std::f32::consts::TAU;
        let envelope = (n as f32 / attack).min(1.0) * (1.0 - t).powf(0.7) * (0.6 + 0.4 * (std::f32::consts::PI * t).sin());
        let value = note.gain * envelope * (phase.sin() + 0.12 * (2.0 * phase).sin());
        left[at] += value * to_left;
        right[at] += value * to_right;
    }
}

/// The notes of one bird singing through `seconds`: `kind` says its song.
/// 0: melodious phrases of gliding notes (a blackbird's or a warbler's);
/// 1: a fast high trill (a wren's or a robin's); 2: two notes repeated (a
/// great tit's); 3: a dove's soft low coos; each where it sits (`pan`),
/// near or far (`gain`); singing more while the wood is `busy`.
fn bird_song(plan: &mut Random, kind: usize, busy: &[f32], pan: f32, gain: f32) -> Vec<Note> {
    let seconds = busy.len() as f32 / NATURE_RATE as f32;
    // A pause, shorter while the wood is busy.
    let rest = |at: f32, pause: f32| pause * (1.6 - 1.2 * busy[((at * NATURE_RATE as f32) as usize).min(busy.len() - 1)]);
    let mut notes = Vec::new();
    let mut at = plan.between(0.0, 6.0);
    // Each bird its own voice, kept through its song.
    let (low, high) = match kind {
        0 => (plan.between(1400.0, 1900.0), plan.between(3200.0, 4400.0)),
        1 => (plan.between(3800.0, 4600.0), plan.between(5200.0, 6800.0)),
        3 => (plan.between(420.0, 560.0), 0.0),
        _ => (plan.between(3800.0, 5000.0), 0.0),
    };
    let second = low * plan.between(0.72, 0.85);
    while at < seconds {
        match kind {
            0 => {
                let mut t = at;
                for _ in 0..(5 + (plan.unit() * 8.0) as usize) {
                    let length = plan.between(0.06, 0.22);
                    let from = plan.between(low, high);
                    let to = from * plan.between(0.75, 1.3);
                    let buzz = if plan.unit() < 0.2 { plan.between(0.02, 0.06) } else { 0.0 };
                    notes.push(Note { at: t, length, from, to, arch: plan.between(-0.15, 0.2), buzz, rate: plan.between(35.0, 70.0), gain: gain * plan.between(0.6, 1.0), pan });
                    t += length + plan.between(0.02, 0.12);
                }
                at = t + rest(t, plan.between(1.5, 5.0));
            }
            1 => {
                let mut t = at;
                let mut pitch = plan.between(low, high);
                for _ in 0..(20 + (plan.unit() * 40.0) as usize) {
                    if plan.unit() < 0.15 {
                        pitch = plan.between(low, high);
                    }
                    let length = plan.between(0.025, 0.05);
                    notes.push(Note { at: t, length, from: pitch * 1.08, to: pitch * 0.9, arch: 0.0, buzz: 0.0, rate: 0.0, gain: gain * plan.between(0.5, 0.9), pan });
                    t += length + plan.between(0.008, 0.025);
                }
                at = t + rest(t, plan.between(3.0, 8.0));
            }
            3 => {
                // Coo, COO-coo, coo-coo: five soft low notes, the second longer.
                let mut t = at;
                for (k, (length, gap)) in [(0.35, 0.15), (0.6, 0.12), (0.3, 0.35), (0.35, 0.12), (0.3, 0.0)].into_iter().enumerate() {
                    let pitch = low * if k == 1 { 1.06 } else { 1.0 };
                    notes.push(Note { at: t, length, from: pitch, to: pitch * 0.92, arch: 0.04, buzz: 0.0, rate: 0.0, gain: gain * if k == 1 { 1.0 } else { 0.8 }, pan });
                    t += length + gap;
                }
                at = t + rest(t, plan.between(14.0, 30.0));
            }
            _ => {
                let mut t = at;
                for k in 0..(6 + (plan.unit() * 8.0) as usize) {
                    let pitch = if k % 2 == 0 { low } else { second };
                    let length = plan.between(0.08, 0.13);
                    notes.push(Note { at: t, length, from: pitch, to: pitch * 0.94, arch: 0.05, buzz: 0.0, rate: 0.0, gain: gain * 0.8, pan });
                    t += if k % 2 == 0 { length + 0.05 } else { length + 0.17 };
                }
                at = t + rest(t, plan.between(4.0, 10.0));
            }
        }
    }
    notes
}

/// A short reverberation of the forest, one ear: four damped combs and two
/// all-passes (Schroeder's, Moorer's damping), each ear its own delays.
fn forest_reverb(dry: &[f32], delays: [f32; 6]) -> Vec<f32> {
    let rate = NATURE_RATE as f32;
    let mut out = vec![0.0f32; dry.len()];
    for &delay in &delays[..4] {
        let d = (delay * rate) as usize;
        let mut line = vec![0.0f32; d];
        let mut damp = 0.0f32;
        for (i, sample) in dry.iter().enumerate() {
            let back = line[i % d];
            damp += 0.45 * (back - damp);
            line[i % d] = sample + damp * 0.72;
            out[i] += back * 0.25;
        }
    }
    for &delay in &delays[4..] {
        let d = (delay * rate) as usize;
        let mut line = vec![0.0f32; d];
        for (i, sample) in out.iter_mut().enumerate() {
            let back = line[i % d];
            let held = *sample + 0.6 * back;
            line[i % d] = held;
            *sample = back - 0.6 * held;
        }
    }
    out
}

/// Birds at dawn in a wood (after six CC0 recordings, docs/sounds.md): a few
/// near birds singing their songs, each where it sits, many far ones chirping
/// softly, a little air under them, and the wood's short echo.
fn birds(count: usize, plan: &mut Random) -> (Vec<f32>, Vec<f32>) {
    let (mut left, mut right) = (vec![0.0f32; count], vec![0.0f32; count]);
    // How busy the wood is: more singing, then less, over tens of seconds.
    let busy = slow_curve(plan, count, 18.0);
    for (kind, gain) in [(0, 0.5), (0, 0.2), (1, 0.3), (2, 0.3), (3, 0.08)] {
        let pan = plan.between(-0.8, 0.8);
        for note in bird_song(plan, kind, &busy, pan, gain) {
            add_note(&mut left, &mut right, &note);
        }
    }
    // Far birds: short soft chirps, here and there, all around.
    let mut random = Random(61);
    for at in events(&mut random, count, 12.0) {
        if random.unit() > busy[at] {
            continue;
        }
        let from = random.between(2800.0, 6500.0);
        let note = Note { at: at as f32 / NATURE_RATE as f32, length: random.between(0.03, 0.12), from, to: from * random.between(0.7, 1.3), arch: random.between(-0.1, 0.15), buzz: 0.0, rate: 0.0, gain: random.between(0.02, 0.07), pan: random.between(-1.0, 1.0) };
        add_note(&mut left, &mut right, &note);
    }
    let (wet_left, wet_right) = (forest_reverb(&left, [0.0297, 0.0371, 0.0411, 0.0437, 0.005, 0.0017]), forest_reverb(&right, [0.0313, 0.0353, 0.0449, 0.0479, 0.0047, 0.0019]));
    // The wood's own breath under them: a light air moving the leaves, low and
    // much the same in both ears, its leaves' hiss each ear's own.
    let breeze: Vec<f32> = slow_curve(plan, count, 7.0).iter().zip(slow_curve(plan, count, 2.0)).map(|(s, g)| 0.3 + 0.7 * (0.3 * s + 0.7 * s * g)).collect();
    let air = wind_layer(&mut random, &breeze, 40.0, 1400.0, 1.0, 0.6);
    let (leaves_left, leaves_right) = (wind_layer(&mut random, &breeze, 1500.0, 10000.0, 1.5, 0.5), wind_layer(&mut random, &breeze, 1500.0, 10000.0, 1.5, 0.5));
    let mix = |dry: &[f32], wet: &[f32], leaves: &[f32]| -> Vec<f32> { (0..count).map(|i| dry[i] * 1.05 + wet[i] * 0.55 + air[i] * 0.35 + leaves[i] * 0.35).collect() };
    (mix(&left, &wet_left, &leaves_left), mix(&right, &wet_right, &leaves_right))
}

/// A sound of nature in stereo: its two channels, each looping, about −22 dBFS.
pub fn nature(kind: &str) -> (Vec<i16>, Vec<i16>) {
    let seconds: usize = match kind {
        "waves" => 120,
        "storm" => 180,
        "wind" => 90,
        "birds" => 90,
        _ => 60,
    };
    let fade = 4 * NATURE_RATE as usize;
    let count = seconds * NATURE_RATE as usize + fade;
    let mut plan = Random(0x5105_7a7e_0000_0000 ^ kind.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b))));
    let (left, right) = match kind {
        "waves" => waves(count, &mut plan),
        "rain" => (rain(count, 21, 1.0), rain(count, 22, 1.0)),
        "wind" => wind(count, &mut plan),
        "birds" => birds(count, &mut plan),
        "crickets" => {
            let band: Vec<(f32, Cricket)> = (0..6)
                .map(|_| {
                    let pan = plan.between(-0.85, 0.85);
                    let near = plan.between(0.15, 1.0);
                    (pan, Cricket { gain: near * 0.35, pitch: plan.between(4100.0, 4900.0), period: plan.between(0.42, 0.95), pulses: if plan.unit() < 0.5 { 3 } else { 4 }, offset: plan.between(0.0, 1.0) })
                })
                .collect();
            let side = |left: bool| -> Vec<Cricket> {
                band.iter()
                    .map(|(pan, c)| {
                        let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
                        let weight = if left { angle.cos() } else { angle.sin() };
                        Cricket { gain: c.gain * weight, pitch: c.pitch, period: c.period, pulses: c.pulses, offset: c.offset }
                    })
                    .collect()
            };
            (crickets(count, 41, &side(true), 0.025), crickets(count, 42, &side(false), 0.035))
        }
        _ => storm(count, &mut plan),
    };
    let target = if kind == "crickets" { 0.05 } else { 0.08 };
    let looped = |raw: &[f32]| -> Vec<f32> {
        let length = raw.len() - fade;
        let mut out: Vec<f32> = raw[..length].to_vec();
        for (i, sample) in out.iter_mut().take(fade).enumerate() {
            let t = i as f32 / fade as f32;
            *sample = raw[i] * t.sqrt() + raw[length + i] * (1.0 - t).sqrt();
        }
        out
    };
    let (left, right) = (looped(&left), looped(&right));
    let rms = (left.iter().chain(&right).map(|s| s * s).sum::<f32>() / (left.len() + right.len()) as f32).sqrt().max(1e-6);
    let gain = target / rms;
    let to_pcm = |channel: &[f32]| channel.iter().map(|s| (s * gain).tanh()).map(|s| (s * f32::from(i16::MAX)) as i16).collect::<Vec<i16>>();
    (to_pcm(&left), to_pcm(&right))
}

#[cfg(test)]
mod tests {
    use super::*;

/// A band-pass filter (RBJ's cookbook, constant peak gain), its centre free to move.
#[derive(Default, Clone, Copy)]
struct BandPass {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BandPass {
    fn run(&mut self, x: f32, centre: f32, q: f32) -> f32 {
        let w = 2.0 * std::f32::consts::PI * centre / NATURE_RATE as f32;
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        let (b0, b2, a1, a2) = (alpha / a0, -alpha / a0, -2.0 * w.cos() / a0, (1.0 - alpha) / a0);
        let y = b0 * x + b2 * self.x2 - a1 * self.y1 - a2 * self.y2;
        (self.x2, self.x1, self.y2, self.y1) = (self.x1, x, self.y1, y);
        y
    }
}

    #[test]
    fn the_random_source_is_centred() {
        let mut random = Random(7);
        let values: Vec<f32> = (0..100_000).map(|_| random.next()).collect();
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!(mean.abs() < 0.01, "mean {mean}");
        assert!(values.iter().all(|v| (-1.0..1.0).contains(v)));
    }

    #[test]
    fn noises_loop_without_a_seam() {
        for kind in NOISES {
            let samples = looped(kind, 3);
            assert_eq!(samples.len(), 3 * RATE as usize);
            // The jump from the end to the start is no larger than the noise's own steps.
            let step = |a: i16, b: i16| (i32::from(a) - i32::from(b)).abs();
            let seam = step(samples[samples.len() - 1], samples[0]);
            let typical = samples.windows(2).map(|w| step(w[0], w[1])).max().unwrap_or(0);
            assert!(seam <= typical, "{kind}: seam {seam}, steps up to {typical}");
        }
        // Brown is darker than white: smaller steps between samples.
        let mean_step = |s: &[i16]| s.windows(2).map(|w| (i32::from(w[0]) - i32::from(w[1])).abs() as i64).sum::<i64>() / s.len() as i64;
        assert!(mean_step(&looped("brown", 1)) * 4 < mean_step(&looped("white", 1)));
    }

    /// Energy around a frequency, per sample and per hertz of the band (Q = 2).
    fn energy(samples: &[i16], frequency: f32) -> f32 {
        let mut band = BandPass::default();
        let power = samples.iter().map(|&x| band.run(f32::from(x) / 32768.0, frequency, 2.0).powi(2)).sum::<f32>() / samples.len() as f32;
        power / (frequency / 2.0)
    }

    #[test]
    fn nature_loops_and_sounds_like_itself() {
        for kind in NATURE {
            let (left, right) = nature(kind);
            assert_eq!(left.len(), right.len());
            assert!(left.len() >= 60 * NATURE_RATE as usize, "{kind}: a minute at least");
            // The jump from the end to the start is no larger than the sound's own steps.
            let step = |a: i16, b: i16| (i32::from(a) - i32::from(b)).abs();
            let seam = step(left[left.len() - 1], left[0]);
            let largest = left.windows(2).map(|w| step(w[0], w[1])).max().unwrap_or(0);
            assert!(seam <= largest, "{kind}: seam {seam}, steps up to {largest}");
            // Quiet, never clipped.
            assert!(left.iter().all(|s| s.unsigned_abs() < 32000), "{kind}: clipped");
        }
        // Crickets sing near 4.5 kHz, far above their night's air; waves rumble low.
        let (crickets, _) = nature("crickets");
        if let Some(folder) = std::env::var_os("SIOUL_SOUNDS_OUT") {
            for kind in NATURE {
                let _ = noise_file(std::path::Path::new(&folder), kind);
            }
        }
        assert!(energy(&crickets, 4500.0) > 5.0 * energy(&crickets, 1000.0));
        let (waves, _) = nature("waves");
        assert!(energy(&waves, 150.0) > energy(&waves, 5000.0));
        // Birds sing between 2 and 5 kHz, above the wood's own air.
        let (birds, _) = nature("birds");
        assert!(energy(&birds, 3500.0) > 2.0 * energy(&birds, 800.0));
    }

    /// The two ears hear one beach, one wood, one storm: partly alike, as the
    /// recordings they were tuned against (0.4 to 1 in their lows), never one
    /// sound twice nor two unrelated ones.
    #[test]
    fn the_two_ears_are_partly_alike() {
        for kind in ["waves", "wind", "storm", "birds"] {
            let (left, right) = nature(kind);
            let (l, r): (Vec<f64>, Vec<f64>) = left.iter().zip(&right).map(|(l, r)| (f64::from(*l), f64::from(*r))).unzip();
            let mean = |x: &[f64]| x.iter().sum::<f64>() / x.len() as f64;
            let (ml, mr) = (mean(&l), mean(&r));
            let cov = l.iter().zip(&r).map(|(a, b)| (a - ml) * (b - mr)).sum::<f64>();
            let var = |x: &[f64], m: f64| x.iter().map(|a| (a - m).powi(2)).sum::<f64>();
            let correlation = cov / (var(&l, ml) * var(&r, mr)).sqrt();
            assert!((0.3..0.99).contains(&correlation), "{kind}: {correlation}");
        }
    }

    #[test]
    fn only_older_recipes_files_are_taken_away() {
        let older = format!("waves-{}.wav", NATURE_VERSION - 1);
        let noise = format!("pink-noise-{}.wav", NATURE_VERSION - 1);
        assert!(older_recipe(&older) && older_recipe(&noise));
        for kept in [format!("waves-{NATURE_VERSION}.wav"), format!("pink-noise-{NATURE_VERSION}.wav"), "my-rain-2.wav".into(), "waves-2.ogg".into(), "waves.wav".into(), "notes-1.wav".into()] {
            assert!(!older_recipe(&kept), "{kept}");
        }
    }

    #[test]
    fn a_wav_header_says_what_follows() {
        let bytes = wav(&[0, 1, -1]);
        assert_eq!(&bytes[..4], b"RIFF");
        let stereo = wav_of(&[0, 1, -1, 2], 2, NATURE_RATE);
        assert_eq!(u16::from_le_bytes(stereo[22..24].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(stereo[28..32].try_into().unwrap()), NATURE_RATE * 4);
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 6);
        assert_eq!(bytes.len(), 44 + 6);
    }
}

