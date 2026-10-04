// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sounds to rest by or to focus with (docs/sounds.md), all made here, nothing
//! downloaded, nothing to license, each looping without a seam (its end faded
//! into its start):
//! - **noise** for focus: white, pink, brown; a minute each, mono;
//! - **nature** to rest by: waves on a beach, rain, wind in the trees, crickets
//!   at night, a distant storm; one to three minutes each, in stereo at
//!   22 kHz (what they hold lies below 10 kHz, and the files stay small).
//!   Filtered noise shaped by waves, gusts and drops, and crickets' chirps as
//!   pulsed tones, each sound seeded the same every time.
//!
//! Recordings of your own are your files, in the notes' `sounds` folder.

use std::path::{Path, PathBuf};

/// The noises made here.
pub const NOISES: &[&str] = &["white", "pink", "brown"];
/// The sounds of nature made here.
pub const NATURE: &[&str] = &["waves", "rain", "wind", "crickets", "storm"];

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
/// Raised when the recipes change, so the files are made again (2: the noises
/// were offset, their random source not centred; 3: crickets held in tune;
/// 4: warmer waves).
const NATURE_VERSION: u32 = 4;

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

/// Waves on a beach: each swells (a few seconds), breaks (brighter, louder),
/// then withdraws in a fizz of foam; a far surf under them all.
fn waves(count: usize, seed: u64, schedule: &[(f32, f32, f32, f32)]) -> Vec<f32> {
    let mut random = Random(seed);
    let rate = NATURE_RATE as f32;
    let (mut body, mut foam, mut foam_top, mut surf) = (LowPass::default(), LowPass::default(), LowPass::default(), LowPass::default());
    (0..count)
        .map(|i| {
            let t = i as f32 / rate;
            let (mut swell, mut fizz) = (0.0f32, 0.0f32);
            for &(start, rise, fall, strength) in schedule {
                let since = t - start;
                if since < 0.0 || since > rise + fall * 4.0 {
                    continue;
                }
                if since < rise {
                    swell += strength * (since / rise).powi(2);
                } else {
                    let after = since - rise;
                    swell += strength * (-after / fall).exp();
                    fizz += strength * (1.0 - (-after / 0.4).exp()) * (-after / (fall * 0.7)).exp();
                }
            }
            let white = random.next();
            let crash = body.run(white, 150.0 + 1500.0 * swell.min(1.2));
            // The foam: a band of hiss, 2.5 to 6 kHz, as the water withdraws.
            let hiss = foam_top.run(white - foam.run(white, 2500.0), 6000.0);
            let far = surf.run(random.next(), 90.0);
            crash * (0.15 + swell) * 1.8 + hiss * fizz * 0.25 + far * 0.9
        })
        .collect()
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

/// Wind in the trees: gusts that rise and fall (a moving band of noise), and
/// the leaves rustling with them.
fn wind(count: usize, seed: u64, gusts: &[f32]) -> Vec<f32> {
    let mut random = Random(seed);
    let mut out = vec![0.0f32; count];
    let (mut whoosh, mut leaves, mut flutter) = (BandPass::default(), LowPass::default(), LowPass::default());
    for (i, sample) in out.iter_mut().enumerate() {
        let gust = 0.2 + 0.8 * gusts[i].powf(1.6);
        let white = random.next();
        let air = whoosh.run(white, 220.0 + 900.0 * gust, 0.9);
        let rustle = white - leaves.run(white, 2400.0);
        let shiver = flutter.run(random.next().abs(), 25.0) * 6.0;
        *sample = air * gust * 1.4 + rustle * gust.powi(2) * shiver.min(1.5) * 0.18;
    }
    let crackles: Vec<usize> = events(&mut random, count, 140.0);
    for at in crackles {
        let gust = gusts[at.min(count - 1)];
        if random.unit() < gust.powi(2) {
            let (frequency, decay, amplitude, phase) = (random.between(2800.0, 7500.0), random.between(0.0004, 0.0012), random.between(0.02, 0.07), random.between(0.0, 6.28));
            add_ping(&mut out, at, frequency, decay, amplitude, phase);
        }
    }
    out
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

/// Thunder far away: a low rumble rolling for seconds, every half minute or so.
fn thunder(count: usize, seed: u64, strikes: &[(f32, f32, f32)]) -> Vec<f32> {
    let mut random = Random(seed);
    let rate = NATURE_RATE as f32;
    let (mut low, mut roll) = (LowPass::default(), LowPass::default());
    let mut brown = 0.0f32;
    (0..count)
        .map(|i| {
            let t = i as f32 / rate;
            brown = (brown + 0.02 * random.next()) / 1.02;
            let rumble = low.run(brown * 3.5, 110.0);
            let rolling = 0.5 + 0.5 * roll.run(random.next(), 4.0) * 8.0;
            let mut level = 0.0f32;
            for &(start, rise, length) in strikes {
                let since = t - start;
                if since >= 0.0 && since < rise + length * 4.0 {
                    level += if since < rise { since / rise } else { (-(since - rise) / length).exp() };
                }
            }
            rumble * level * rolling.clamp(0.2, 1.4) * 4.0
        })
        .collect()
}

/// A sound of nature in stereo: its two channels, each looping, about −22 dBFS.
pub fn nature(kind: &str) -> (Vec<i16>, Vec<i16>) {
    let rate = NATURE_RATE as f32;
    let seconds: usize = match kind {
        "waves" => 120,
        "storm" => 180,
        "wind" => 90,
        _ => 60,
    };
    let fade = 4 * NATURE_RATE as usize;
    let count = seconds * NATURE_RATE as usize + fade;
    let mut plan = Random(0x5105_7a7e_0000_0000 ^ kind.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b))));
    let (left, right) = match kind {
        "waves" => {
            // The same waves for both ears, the right a little later: they come from one sea.
            let mut schedule = Vec::new();
            let mut start = -2.0f32;
            while start < (count as f32 / rate) {
                let rise = plan.between(2.0, 3.8);
                let fall = plan.between(1.6, 2.6);
                schedule.push((start, rise, fall, plan.between(0.55, 1.0)));
                start += plan.between(6.5, 11.5);
            }
            let late: Vec<(f32, f32, f32, f32)> = schedule.iter().map(|&(s, r, f, k)| (s + 0.25, r, f, k * 0.95)).collect();
            (waves(count, 11, &schedule), waves(count, 12, &late))
        }
        "rain" => (rain(count, 21, 1.0), rain(count, 22, 1.0)),
        "wind" => {
            let gusts = slow_curve(&mut plan, count, 4.5);
            let later: Vec<f32> = (0..count).map(|i| gusts[i.saturating_sub((0.3 * rate) as usize)]).collect();
            (wind(count, 31, &gusts), wind(count, 32, &later))
        }
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
        _ => {
            // A distant storm: lighter rain, and thunder far away.
            let mut strikes = Vec::new();
            let mut at = plan.between(4.0, 14.0);
            while at < (count as f32 / rate) {
                strikes.push((at, plan.between(0.4, 1.1), plan.between(1.8, 3.5)));
                at += plan.between(18.0, 42.0);
            }
            let far: Vec<(f32, f32, f32)> = strikes.iter().map(|&(s, r, l)| (s + 0.04, r, l)).collect();
            let mix = |a: Vec<f32>, b: Vec<f32>| a.iter().zip(&b).map(|(x, y)| x * 0.8 + y).collect::<Vec<f32>>();
            (mix(rain(count, 51, 0.6), thunder(count, 53, &strikes)), mix(rain(count, 52, 0.6), thunder(count, 54, &far)))
        }
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

