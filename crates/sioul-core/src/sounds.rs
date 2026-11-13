// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! Sounds to rest by or to focus with (docs/sounds.md), all made here, nothing
//! downloaded, nothing to license, and made as they play, so that none ever
//! loops:
//! - **noise** for focus: white, pink, brown, the same in both ears;
//! - **nature** to rest by: waves on a beach, rain, wind in the trees, crickets
//!   at night, a distant storm, birds at dawn, in stereo at 22 kHz (what they
//!   hold lies below 10 kHz). Filtered noise shaped by waves, gusts and drops,
//!   crickets' chirps as pulsed tones, birds' songs from real syllable shapes.
//!   Each wave, strike, drop, chirp and song is drawn when its time comes, from
//!   a new seed at each listen, and each sound drifts over minutes: the sea
//!   calmer, then rougher; the wind softer, then gustier; the storm nearer,
//!   then farther; birds and crickets coming and going. The waves, the wind,
//!   the storm and the birds were tuned against CC0 recordings, measure by
//!   measure (docs/sounds.md, tools/sounds/): only the recipes are kept, never
//!   the recordings.
//!
//! `Stream` makes a sound on a thread of its own, a second ahead of what plays,
//! and the window's audio output reads it (crates/sioul-app/src/livesound.rs).
//! Where that output cannot play it, the window plays a file instead: minutes
//! of the same sound, looping (`noise_file`).
//!
//! Recordings of your own are your files, in the notes' `sounds` folder.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// The noises made here.
pub const NOISES: &[&str] = &["white", "pink", "brown"];
/// The sounds of nature made here.
pub const NATURE: &[&str] = &["waves", "rain", "wind", "crickets", "storm", "birds"];

/// The noises' rate: white noise holds every frequency.
const RATE: u32 = 44_100;
/// The sounds of nature's rate: what they hold lies below 10 kHz.
const NATURE_RATE: u32 = 22_050;
/// Raised when the recipes change, so that the files made where a sound cannot
/// be made as it plays are made again and the older ones taken away (2: the
/// noises were offset, their random source not centred; 3: crickets held in
/// tune; 4: warmer waves; 5: the waves, the wind and the storm tuned against
/// recordings, and the birds; 6: made as they play, drifting over minutes).
const NATURE_VERSION: u32 = 6;
/// The frames made at a time: a tenth of a second of nature, a twentieth of noise.
const BLOCK: usize = 2205;

/// A small, seeded random source.
struct Random(u64);

impl Random {
    /// A source of its own for one part of a sound, from the sound's seed:
    /// SplitMix64's mixing, so that near seeds give unrelated sources, and
    /// never zero, where the xorshift would stay.
    fn part(seed: u64, part: u64) -> Random {
        let mut z = seed ^ part.wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Random((z ^ (z >> 31)) | 1)
    }

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

    /// Seconds until the next of events coming `rate` a second on average, at
    /// random (a Poisson process).
    fn wait(&mut self, rate: f32) -> f64 {
        f64::from(-(1.0 - self.unit()).max(1e-6).ln() / rate)
    }
}

/// A one-pole low-pass filter, its cutoff free to move each sample; its
/// coefficient is computed again only when the cutoff moves.
#[derive(Default, Clone, Copy)]
struct LowPass {
    y: f32,
    cutoff: f32,
    a: f32,
}

impl LowPass {
    fn run(&mut self, x: f32, cutoff: f32) -> f32 {
        if cutoff != self.cutoff {
            self.cutoff = cutoff;
            self.a = 1.0 - (-2.0 * std::f32::consts::PI * cutoff / NATURE_RATE as f32).exp();
        }
        self.y += self.a * (x - self.y);
        self.y
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

/// Pink noise, one channel: Paul Kellett's filter.
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

/// A slow, smooth random curve in [0, 1], made as it goes: a new point every
/// `every` seconds, joined by cosines.
struct Drift {
    random: Random,
    step: u32,
    n: u32,
    from: f32,
    to: f32,
}

impl Drift {
    fn new(mut random: Random, every: f32) -> Drift {
        let (from, to) = (random.unit(), random.unit());
        Drift { step: (every * NATURE_RATE as f32).max(1.0) as u32, n: 0, from, to, random }
    }

    /// Its value now.
    fn now(&self) -> f32 {
        let w = (1.0 - (std::f32::consts::PI * self.n as f32 / self.step as f32).cos()) * 0.5;
        self.from * (1.0 - w) + self.to * w
    }

    /// Its value now, then a frame on.
    fn next(&mut self) -> f32 {
        let value = self.now();
        self.skip(1);
        value
    }

    /// `frames` on.
    fn skip(&mut self, frames: usize) {
        let mut on = frames as u64 + u64::from(self.n);
        while on >= u64::from(self.step) {
            on -= u64::from(self.step);
            self.from = self.to;
            self.to = self.random.unit();
        }
        self.n = on as u32;
    }
}

/// Sound already drawn ahead of the present, both ears: an event (a wave, a
/// strike, a song) is made whole when its time comes and added here from its
/// first frame on; the present takes it frame by frame.
#[derive(Default)]
struct Ahead {
    /// The frame `frames[0]` sounds at, counted from the sound's start.
    start: u64,
    frames: VecDeque<(f32, f32)>,
}

impl Ahead {
    /// `length` frames of `sound` (each from its index) added from frame `at`
    /// on. Frames before the present, gone by, are made all the same (a
    /// filter's state runs through them) and left out.
    fn add(&mut self, at: u64, length: usize, mut sound: impl FnMut(usize) -> (f32, f32)) {
        let skip = usize::try_from(self.start.saturating_sub(at)).unwrap_or(usize::MAX);
        let offset = usize::try_from(at.saturating_sub(self.start)).unwrap_or(usize::MAX);
        let end = offset.saturating_add(length.saturating_sub(skip));
        if self.frames.len() < end {
            self.frames.resize(end, (0.0, 0.0));
        }
        for n in 0..length {
            let (left, right) = sound(n);
            if n >= skip {
                let frame = &mut self.frames[offset + n - skip];
                frame.0 += left;
                frame.1 += right;
            }
        }
    }

    /// The present frame; the present is then a frame on.
    fn take(&mut self) -> (f32, f32) {
        self.start += 1;
        self.frames.pop_front().unwrap_or((0.0, 0.0))
    }
}

/// A sound as it is made: its next frames, both ears, from frame `now` on
/// (counted from its start).
trait Scene: Send {
    fn render(&mut self, now: u64, left: &mut [f32], right: &mut [f32]);
}

/// The frame at `seconds` from a sound of nature's start.
fn frame(seconds: f64) -> u64 {
    (seconds.max(0.0) * f64::from(NATURE_RATE)) as u64
}

/// The second at which `frames` from frame `now` end.
fn block_end(now: u64, frames: usize) -> f64 {
    (now + frames as u64) as f64 / f64::from(NATURE_RATE)
}

/// Weights for each ear of a sound at `pan` (−1 left, 1 right), with equal power.
fn panned(pan: f32) -> (f32, f32) {
    let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
    (angle.cos(), angle.sin())
}

// Noise.

#[derive(Clone, Copy)]
enum Colour {
    White,
    Pink,
    Brown,
}

/// A noise for focus: white (flat), pink (−3 dB per octave, Paul Kellett's
/// filter), brown (−6 dB per octave, a leaky integration); the same in both ears.
struct Noise {
    colour: Colour,
    random: Random,
    pink: Pink,
    last: f32,
}

impl Scene for Noise {
    fn render(&mut self, _now: u64, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let white = self.random.next();
            let value = match self.colour {
                Colour::Pink => self.pink.next(white),
                Colour::Brown => {
                    self.last = (self.last + 0.02 * white) / 1.02;
                    self.last * 3.5
                }
                Colour::White => white,
            };
            (*l, *r) = (value, value);
        }
    }
}

// Nature.

/// A sound that fades out quickly: a tone (`frequency`, `decay` seconds,
/// `amplitude`, `phase`), added from frame `at` with a weight for each ear.
fn add_ping(ahead: &mut Ahead, at: u64, (frequency, decay, amplitude, phase): (f32, f32, f32, f32), (to_left, to_right): (f32, f32)) {
    let rate = NATURE_RATE as f32;
    let length = (decay * 5.0 * rate) as usize;
    let w = 2.0 * std::f32::consts::PI * frequency / rate;
    ahead.add(at, length, |n| {
        let value = amplitude * (-(n as f32) / rate / decay).exp() * (w * n as f32 + phase).sin();
        (value * to_left, value * to_right)
    });
}

/// One wave on the beach: how long it rises, how long its crash and its wash
/// last, how strong it is, and where along the beach it comes in (−1 left, 1
/// right).
#[derive(Clone, Copy)]
struct Break {
    rise: f32,
    crash: f32,
    wash: f32,
    strength: f32,
    pan: f32,
}

/// A wave on a gentle slope, added to both ears from frame `at`, from where it
/// comes in: it swells slowly, rounds over without a crash, never bright, and
/// slides back as slowly, the wash running up and back in a soft, dark fizz of
/// foam.
fn add_break(ahead: &mut Ahead, at: u64, wave: &Break, seed: u64) {
    let rate = NATURE_RATE as f32;
    let mut random = Random::part(seed, 0);
    let length = ((wave.rise + wave.wash * 5.0) * rate) as usize;
    let (mut dark, mut darker, mut floor) = (LowPass::default(), LowPass::default(), HighPass::default());
    let (mut foam_floor, mut foam_top, mut foam_top2) = (HighPass::default(), LowPass::default(), LowPass::default());
    let (mut own_floor, mut own_top, mut own_top2) = (HighPass::default(), LowPass::default(), LowPass::default());
    let (to_left, to_right) = panned(wave.pan);
    ahead.add(at, length, |n| {
        let (white, other) = (random.next(), random.next());
        let t = n as f32 / rate;
        // A gentle slope: the wave swells slowly from nothing, rounds over at its
        // top without a corner, and slides back down as slowly.
        let crash = if t < wave.rise {
            let x = t / wave.rise;
            x * x * (3.0 - 2.0 * x)
        } else {
            let x = (t - wave.rise) / wave.crash;
            (1.0 + x) * (-x).exp()
        };
        let washing = if t < wave.rise * 0.6 {
            0.0
        } else {
            let after = t - wave.rise * 0.6;
            (1.0 - (-after / 1.2).exp()) * (-after / wave.wash).exp()
        };
        let cutoff = 220.0 + 650.0 * crash;
        let body = floor.run(darker.run(dark.run(white, cutoff), cutoff * 1.6), 170.0);
        // The foam each ear hears is partly its own: it fizzes all along the beach.
        let fizz = foam_top2.run(foam_top.run(foam_floor.run(white, 400.0), 1600.0), 2200.0);
        let own = own_top2.run(own_top.run(own_floor.run(other, 400.0), 1600.0), 2200.0);
        let body = wave.strength * body * crash * 1.6;
        let foam = wave.strength * washing * 0.5;
        (body * to_left + foam * (fizz * 0.6 * to_left + own * 0.5), body * to_right + foam * (fizz * 0.6 * to_right - own * 0.5))
    });
}

/// The surf of the rest of the beach, under the waves: pink noise between
/// 140 Hz and 1.8 kHz with foam above it, steady, drifting a little over
/// seconds (never flickering: a level jumping many times a second sounds like
/// a radio's static).
struct Surf {
    random: Random,
    level: Drift,
    pink: Pink,
    floors: [HighPass; 3],
    tops: [LowPass; 4],
}

impl Surf {
    fn new(seed: u64, part: u64) -> Surf {
        Surf { random: Random::part(seed, part), level: Drift::new(Random::part(seed, part + 100), 8.0), pink: Pink::default(), floors: [HighPass::default(); 3], tops: [LowPass::default(); 4] }
    }

    fn next(&mut self) -> f32 {
        let ([floor, floor2, foam_floor], [top, top2, foam_top, foam_top2]) = (&mut self.floors, &mut self.tops);
        let p = self.pink.next(self.random.next());
        let shaped = top2.run(top.run(floor2.run(floor.run(p, 140.0), 140.0), 1800.0), 2600.0);
        // The foam fizzing all along the beach, between the waves too.
        let foam = foam_top2.run(foam_top.run(foam_floor.run(self.random.next(), 900.0), 2800.0), 3500.0);
        (shaped + foam * 0.08) * (0.9 + 0.1 * self.level.next())
    }
}

/// Waves on a gently sloping beach (after five CC0 recordings,
/// docs/sounds.md): a wave swelling in every 6 to 11 seconds, small and large,
/// each somewhere along the beach, over the surf of the rest of it, which both
/// ears share in part. The sea calms and rises over minutes: its waves come
/// further apart and smaller, then nearer and larger.
struct Waves {
    plan: Random,
    sea: Drift,
    shared: Surf,
    own: [Surf; 2],
    ahead: Ahead,
    next: f64,
    seed: u64,
}

impl Waves {
    fn new(seed: u64) -> Waves {
        let mut plan = Random::part(seed, 1);
        let next = f64::from(plan.between(0.5, 3.0));
        Waves { sea: Drift::new(Random::part(seed, 2), 240.0), shared: Surf::new(seed, 3), own: [Surf::new(seed, 4), Surf::new(seed, 5)], ahead: Ahead::default(), next, seed: Random::part(seed, 6).0, plan }
    }
}

impl Scene for Waves {
    fn render(&mut self, now: u64, left: &mut [f32], right: &mut [f32]) {
        let end = block_end(now, left.len());
        while self.next < end {
            let (sea, plan) = (self.sea.now(), &mut self.plan);
            let wave = Break { rise: plan.between(1.8, 3.2), crash: plan.between(0.9, 1.5), wash: plan.between(2.5, 4.0), strength: plan.between(0.45, 1.0) * (0.85 + 0.3 * sea), pan: plan.between(-0.6, 0.6) };
            add_break(&mut self.ahead, frame(self.next), &wave, self.seed);
            self.seed = self.seed.wrapping_add(1);
            self.next += f64::from(plan.between(6.0, 11.0) * (1.1 - 0.2 * sea));
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let calm = (0.92 + 0.16 * self.sea.next()) * 0.3;
            let shared = self.shared.next() * 0.8;
            let (wave_left, wave_right) = self.ahead.take();
            *l = (shared + self.own[0].next() * 0.6) * calm + wave_left;
            *r = (shared + self.own[1].next() * 0.6) * calm + wave_right;
        }
    }
}

/// One ear of the rain: its hiss, its many small drops and its few larger ones.
struct RainEar {
    random: Random,
    level: Drift,
    pink: Pink,
    low: LowPass,
    high: LowPass,
    small: f64,
    large: f64,
}

/// Rain: a soft hiss, many small drops, a few larger ones on leaves and
/// stones, each ear its own; heavier, then lighter, over minutes.
struct Rain {
    heavy: Drift,
    ears: [RainEar; 2],
    ahead: Ahead,
}

impl Rain {
    fn new(seed: u64) -> Rain {
        let ear = |part: u64| {
            let mut random = Random::part(seed, part);
            let (small, large) = (random.wait(160.0), random.wait(5.0));
            RainEar { level: Drift::new(Random::part(seed, part + 100), 6.0), random, pink: Pink::default(), low: LowPass::default(), high: LowPass::default(), small, large }
        };
        Rain { heavy: Drift::new(Random::part(seed, 1), 300.0), ears: [ear(2), ear(3)], ahead: Ahead::default() }
    }
}

impl Scene for Rain {
    fn render(&mut self, now: u64, left: &mut [f32], right: &mut [f32]) {
        let end = block_end(now, left.len());
        let heavy = 0.85 + 0.3 * self.heavy.now();
        for (side, ear) in self.ears.iter_mut().enumerate() {
            let to = if side == 0 { (1.0, 0.0) } else { (0.0, 1.0) };
            while ear.small < end {
                let r = &mut ear.random;
                let ping = (r.between(2400.0, 7000.0), r.between(0.0012, 0.004), 0.08 * -(1.0 - r.unit()).max(1e-4).ln().min(4.0), r.between(0.0, 6.28));
                add_ping(&mut self.ahead, frame(ear.small), ping, to);
                ear.small += ear.random.wait(160.0 * heavy);
            }
            while ear.large < end {
                let r = &mut ear.random;
                let ping = (r.between(700.0, 2400.0), r.between(0.008, 0.022), r.between(0.05, 0.14), r.between(0.0, 6.28));
                add_ping(&mut self.ahead, frame(ear.large), ping, to);
                ear.large += ear.random.wait(5.0 * heavy);
            }
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let heavy = (0.85 + 0.3 * self.heavy.next()) * 0.9;
            let [one, two] = self.ears.each_mut().map(|ear| {
                let p = ear.pink.next(ear.random.next());
                let band = ear.high.run(p, 6500.0) - ear.low.run(p, 450.0);
                band * (0.75 + 0.25 * ear.level.next()) * heavy
            });
            let (drops_left, drops_right) = self.ahead.take();
            (*l, *r) = (one + drops_left, two + drops_right);
        }
    }
}

/// One layer of wind: pink noise between `low` and `high` hertz (two poles at
/// the top).
struct Layer {
    random: Random,
    pink: Pink,
    floor: HighPass,
    top: LowPass,
    top2: LowPass,
    low: f32,
    high: f32,
}

impl Layer {
    fn new(random: Random, low: f32, high: f32) -> Layer {
        Layer { random, pink: Pink::default(), floor: HighPass::default(), top: LowPass::default(), top2: LowPass::default(), low, high }
    }

    fn next(&mut self) -> f32 {
        self.top2.run(self.top.run(self.floor.run(self.pink.next(self.random.next()), self.low), self.high), self.high * 1.4)
    }
}

/// Wind in the trees (after six CC0 recordings, docs/sounds.md): close to pink
/// noise from 60 Hz to 4.5 kHz, soft and dark, louder and brighter in the
/// gusts, which rise and fall slowly, over 15 and 6 seconds, and rest between;
/// softer, then gustier, over minutes. The branches' low roar is nearly the
/// same in both ears; the air's whoosh in part; the leaves, all around, differ
/// in each.
struct Wind {
    slow: Drift,
    gust: Drift,
    strength: Drift,
    /// The roar, the air and the leaves, each shared, then each ear's own.
    layers: [Layer; 9],
}

impl Wind {
    fn new(seed: u64) -> Wind {
        let bands = [(60.0, 350.0), (250.0, 1300.0), (1500.0, 4500.0)];
        let layers: [Layer; 9] = std::array::from_fn(|k| Layer::new(Random::part(seed, 10 + k as u64), bands[k / 3].0, bands[k / 3].1));
        Wind { slow: Drift::new(Random::part(seed, 1), 15.0), gust: Drift::new(Random::part(seed, 2), 6.0), strength: Drift::new(Random::part(seed, 3), 300.0), layers }
    }
}

impl Scene for Wind {
    fn render(&mut self, _now: u64, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let slow = self.slow.next() * (0.6 + 0.45 * self.strength.next());
            let gust = self.gust.next();
            let roar = 0.15 + 0.85 * (0.1 + 0.3 * slow + 0.45 * slow * gust + 0.15 * gust).min(1.0).powf(1.4);
            // The higher, the more a layer follows the gusts.
            let (air, leaves) = (roar.powf(1.3), roar.powf(1.7));
            let v = self.layers.each_mut().map(|layer| layer.next());
            let shared = v[0] * 0.95 * roar * 0.45 + v[3] * 0.85 * air * 0.8 + v[6] * 0.4 * leaves * 0.5;
            *l = shared + v[1] * 0.3 * roar * 0.45 + v[4] * 0.5 * air * 0.8 + v[7] * 0.9 * leaves * 0.5;
            *r = shared + v[2] * 0.3 * roar * 0.45 + v[5] * 0.5 * air * 0.8 + v[8] * 0.9 * leaves * 0.5;
        }
    }
}

/// One cricket of a night: how loud, where it sits, its pitch, its rhythm,
/// when it chirps next, and how near it sings, which changes over a minute
/// or two.
struct Cricket {
    gain: f32,
    to_left: f32,
    to_right: f32,
    pitch: f32,
    period: f32,
    pulses: usize,
    next: f64,
    here: Drift,
}

/// A chirp of `cricket`, from frame `at`: a few pulses of a tone falling a
/// little, `loud` of its full voice.
fn add_chirp(ahead: &mut Ahead, at: u64, cricket: &Cricket, loud: f32) {
    let rate = NATURE_RATE as f32;
    let (pulse, gap) = ((0.014 * rate) as usize, (0.016 * rate) as usize);
    let mut phase = 0.0f32;
    ahead.add(at, cricket.pulses * (pulse + gap), |n| {
        let m = n % (pulse + gap);
        if m == 0 {
            // The phase counted from the pulse's start: exact in single precision.
            phase = 0.0;
        }
        if m >= pulse {
            return (0.0, 0.0);
        }
        let t = m as f32 / pulse as f32;
        let window = (std::f32::consts::PI * t).sin().powi(2);
        phase += 2.0 * std::f32::consts::PI * cricket.pitch * (1.0 - 0.015 * t) / rate;
        let value = cricket.gain * loud * window * phase.sin();
        (value * cricket.to_left, value * cricket.to_right)
    });
}

/// Crickets at night: a few field crickets chirping, each its pitch and
/// rhythm, near and far, each softer, then louder again, over minutes;
/// a tree cricket trilling softly; the night's air under them.
struct Crickets {
    random: Random,
    band: Vec<Cricket>,
    air: [LowPass; 2],
    swell: [Drift; 2],
    ahead: Ahead,
}

impl Crickets {
    fn new(seed: u64) -> Crickets {
        let mut plan = Random::part(seed, 1);
        let band = (0..6u64)
            .map(|k| {
                let (to_left, to_right) = panned(plan.between(-0.85, 0.85));
                let near = plan.between(0.35, 1.0);
                Cricket { gain: near * 0.35, to_left, to_right, pitch: plan.between(4100.0, 4900.0), period: plan.between(0.42, 0.95), pulses: if plan.unit() < 0.5 { 3 } else { 4 }, next: f64::from(plan.between(0.0, 1.0)), here: Drift::new(Random::part(seed, 10 + k), 90.0) }
            })
            .collect();
        Crickets { random: Random::part(seed, 2), band, air: [LowPass::default(); 2], swell: [Drift::new(Random::part(seed, 3), 3.5), Drift::new(Random::part(seed, 4), 3.5)], ahead: Ahead::default() }
    }
}

impl Scene for Crickets {
    fn render(&mut self, now: u64, left: &mut [f32], right: &mut [f32]) {
        let end = block_end(now, left.len());
        for cricket in &mut self.band {
            while cricket.next < end {
                // Never quite gone: a cricket sings softer, farther, then near again.
                let loud = 0.4 + 0.6 * ((cricket.here.now() - 0.1) / 0.3).clamp(0.0, 1.0);
                add_chirp(&mut self.ahead, frame(cricket.next), cricket, loud);
                cricket.next += f64::from(cricket.period * self.random.between(0.95, 1.05));
            }
            cricket.here.skip(left.len());
        }
        for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            // A tree cricket: a soft trill at 2.8 kHz, 40 pulses a second, swelling
            // and resting; in double precision, as hours of a 2.8 kHz tone are
            // tens of millions of radians.
            let t = (now + i as u64) as f64 / f64::from(NATURE_RATE);
            let trill = ((2.0 * std::f64::consts::PI * 40.0 * t).sin().max(0.0).powi(2) * (2.0 * std::f64::consts::PI * 2800.0 * t).sin()) as f32;
            let on = |swell: &mut Drift| (swell.next() - 0.35).max(0.0) / 0.65;
            let air_left = self.air[0].run(self.random.next(), 120.0) * 0.25;
            let air_right = self.air[1].run(self.random.next(), 120.0) * 0.25;
            let (chirps_left, chirps_right) = self.ahead.take();
            *l = air_left + 0.025 * on(&mut self.swell[0]) * trill + chirps_left;
            *r = air_right + 0.035 * on(&mut self.swell[1]) * trill + chirps_right;
        }
    }
}

/// One strike of thunder far away, added to both ears from frame `at`, from
/// where it is: a few bursts within seconds, rolling, low; brighter at its
/// start the nearer it is (`near`, 0 to 1).
fn add_thunder(ahead: &mut Ahead, at: u64, near: f32, pan: f32, seed: u64) {
    let rate = NATURE_RATE as f32;
    let mut random = Random::part(seed, 0);
    let mut bursts = Vec::new();
    let mut offset = 0.0f32;
    let mut strength = 1.0f32;
    for _ in 0..(2 + (random.unit() * 5.0) as usize) {
        bursts.push((offset, random.between(0.06, 0.4), random.between(0.6, 2.2), strength));
        offset += random.between(0.3, 2.5);
        strength *= random.between(0.55, 1.05);
    }
    let length = ((offset + 12.0) * rate) as usize;
    let mut rolling = Drift::new(Random::part(seed, 1), 0.2);
    let (mut brown, mut dark, mut darker, mut floor) = (0.0f32, LowPass::default(), LowPass::default(), HighPass::default());
    let (to_left, to_right) = panned(pan);
    let (base, bright) = (110.0 + 220.0 * near, 300.0 + 2600.0 * near * near);
    ahead.add(at, length, |n| {
        let t = n as f32 / rate;
        let mut sum = 0.0f32;
        for &(begin, rise, fall, strength) in &bursts {
            let since = t - begin;
            if since >= 0.0 {
                sum += strength * if since < rise { since / rise } else { (-(since - rise) / fall).exp() };
            }
        }
        // Bursts that overlap roll on louder, never piling up into one clap.
        let level = 1.0 - (-1.2 * sum).exp();
        let white = random.next();
        brown = (brown + 0.02 * white) / 1.02;
        let cutoff = base + (bright - base) * (-t / 0.9).exp();
        let rumble = floor.run(darker.run(dark.run(brown * 3.5 + white * 0.15, cutoff), cutoff * 1.3), 30.0);
        let value = rumble * level * (0.5 + 0.7 * rolling.next()) * (0.7 + 0.5 * near);
        (value * to_left, value * to_right)
    });
}

/// The rain of a storm far away: a soft, dark, steady hiss, a soft drop now
/// and then, heard in each ear with a weight.
struct StormRain {
    random: Random,
    pink: Pink,
    floor: HighPass,
    top: LowPass,
    drops: f64,
    to: (f32, f32),
}

impl StormRain {
    fn new(seed: u64, part: u64, to: (f32, f32)) -> StormRain {
        let mut random = Random::part(seed, part);
        StormRain { drops: random.wait(1.0), random, pink: Pink::default(), floor: HighPass::default(), top: LowPass::default(), to }
    }

    fn next(&mut self, level: f32) -> f32 {
        self.top.run(self.floor.run(self.pink.next(self.random.next()), 250.0), 2500.0) * (0.8 + 0.2 * level)
    }
}

/// A distant storm (after six CC0 recordings, docs/sounds.md): soft rain, its
/// level wandering, thunder too far to make out rolling under it, and thunder
/// every 8 to 28 seconds, each strike rolling in a few bursts, the same in both
/// ears from where it is. The storm comes nearer, then goes farther, over
/// minutes: its strikes more often and brighter, its rain heavier, then less.
struct Storm {
    plan: Random,
    level: Drift,
    far: Drift,
    near: Drift,
    /// The rain both ears share, then each ear's own.
    rains: [StormRain; 3],
    random: Random,
    brown: f32,
    deep: LowPass,
    deeper: LowPass,
    ahead: Ahead,
    next: f64,
    seed: u64,
}

impl Storm {
    fn new(seed: u64) -> Storm {
        let mut plan = Random::part(seed, 1);
        let next = f64::from(plan.between(2.0, 10.0));
        Storm {
            level: Drift::new(Random::part(seed, 2), 15.0),
            far: Drift::new(Random::part(seed, 3), 6.0),
            near: Drift::new(Random::part(seed, 4), 240.0),
            rains: [StormRain::new(seed, 5, (0.08, 0.08)), StormRain::new(seed, 6, (0.055, 0.0)), StormRain::new(seed, 7, (0.0, 0.055))],
            random: Random::part(seed, 8),
            brown: 0.0,
            deep: LowPass::default(),
            deeper: LowPass::default(),
            ahead: Ahead::default(),
            next,
            seed: Random::part(seed, 9).0,
            plan,
        }
    }
}

impl Scene for Storm {
    fn render(&mut self, now: u64, left: &mut [f32], right: &mut [f32]) {
        let end = block_end(now, left.len());
        let nearer = self.near.now();
        for rain in &mut self.rains {
            while rain.drops < end {
                let r = &mut rain.random;
                let ping = (r.between(500.0, 1400.0), r.between(0.01, 0.025), r.between(0.01, 0.03), r.between(0.0, 6.28));
                add_ping(&mut self.ahead, frame(rain.drops), ping, rain.to);
                rain.drops += rain.random.wait(1.0);
            }
        }
        while self.next < end {
            let plan = &mut self.plan;
            let near = (plan.unit().powi(2) * (0.5 + 0.6 * nearer)).min(1.0);
            add_thunder(&mut self.ahead, frame(self.next), near, plan.between(-0.5, 0.5), self.seed);
            self.seed = self.seed.wrapping_add(1);
            self.next += f64::from(plan.between(8.0, 28.0) * (1.3 - 0.4 * nearer));
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (level, nearer) = (self.level.next(), self.near.next());
            let [shared, own_left, own_right] = self.rains.each_mut().map(|rain| rain.next(level));
            let heavier = (0.9 + 0.2 * nearer) * 0.1;
            self.brown = (self.brown + 0.02 * self.random.next()) / 1.02;
            let rumble = self.deeper.run(self.deep.run(self.brown * 3.5, 160.0), 220.0) * (0.35 + 0.65 * self.far.next()) * 0.08 * (0.8 + 0.4 * nearer);
            let (strike_left, strike_right) = self.ahead.take();
            *l = (shared * 0.8 + own_left * 0.55) * heavier + rumble + strike_left;
            *r = (shared * 0.8 + own_right * 0.55) * heavier + rumble + strike_right;
        }
    }
}

/// How a bird's syllable moves in pitch, in hertz, as recordings show it:
/// fast sweeps, often an octave in a few hundredths of a second.
#[derive(Clone, Copy)]
enum Contour {
    /// From `from` toward `to`, fast then slower (a hook), in `tau` seconds.
    Hook { from: f32, to: f32, tau: f32 },
    /// From `from` to `to` at an even pace (a stroke).
    Line { from: f32, to: f32 },
    /// Around `centre`, by `depth` (a fraction) `rate` times a second (a
    /// warble when slow, a buzz when fast).
    Wobble { centre: f32, depth: f32, rate: f32 },
    /// From `from` up to `peak` in the first third, then sinking to `to` (a coo).
    Arc { from: f32, peak: f32, to: f32 },
}

/// One syllable of a song: when (seconds from the song's start), how long,
/// its pitch's movement, how loud, and its second harmonic's share.
#[derive(Clone, Copy)]
struct Syllable {
    at: f32,
    length: f32,
    contour: Contour,
    gain: f32,
    harmonic: f32,
    /// A dove's breathy, hollow coo rather than a songbird's clean whistle: this
    /// share of breath (noise around the pitch), soft edges, a fuller harmonic.
    breath: f32,
}

impl Contour {
    fn pitch(self, t: f32, length: f32) -> f32 {
        match self {
            Contour::Hook { from, to, tau } => to + (from - to) * (-t / tau).exp(),
            Contour::Line { from, to } => from + (to - from) * t / length,
            Contour::Wobble { centre, depth, rate } => centre * (1.0 + depth * (std::f32::consts::TAU * rate * t).sin()),
            Contour::Arc { from, peak, to } => {
                let x = t / length;
                if x < 0.3 { from + (peak - from) * (std::f32::consts::FRAC_PI_2 * x / 0.3).sin() } else { peak + (to - peak) * ((x - 0.3) / 0.7).powi(2) }
            }
        }
    }

    /// The same movement, its pitch times `factor`.
    fn scaled(self, factor: f32) -> Contour {
        match self {
            Contour::Hook { from, to, tau } => Contour::Hook { from: from * factor, to: to * factor, tau },
            Contour::Line { from, to } => Contour::Line { from: from * factor, to: to * factor },
            Contour::Wobble { centre, depth, rate } => Contour::Wobble { centre: centre * factor, depth, rate },
            Contour::Arc { from, peak, to } => Contour::Arc { from: from * factor, peak: peak * factor, to: to * factor },
        }
    }
}

/// A syllable added to a song (`song`, the frame it starts at being `at`): a
/// quick attack, a short release, fuller in the middle.
fn add_syllable(song: &mut [f32], at: u64, syllable: &Syllable) {
    let rate = NATURE_RATE as f32;
    let first = (syllable.at * rate) as usize;
    let count = (syllable.length * rate) as usize;
    let breathy = syllable.breath > 0.0;
    let (attack, release) = if breathy { (0.045 * rate, (0.07 * rate).min(count as f32 * 0.4)) } else { (0.003 * rate, (0.008 * rate).min(count as f32 * 0.4)) };
    let mut random = Random(0x0d0e ^ (at + first as u64));
    let (mut breath, mut throat) = (BandPass::default(), LowPass::default());
    let mut phase = 0.0f32;
    for n in 0..count {
        let Some(sample) = song.get_mut(first + n) else { break };
        let t = n as f32 / rate;
        let frequency = syllable.contour.pitch(t, syllable.length).clamp(200.0, 9000.0);
        phase = (phase + std::f32::consts::TAU * frequency / rate) % std::f32::consts::TAU;
        let edges = if breathy {
            // Soft edges: a coo swells in and fades out.
            let rise = (std::f32::consts::FRAC_PI_2 * (n as f32 / attack).min(1.0)).sin().powi(2);
            let fall = (std::f32::consts::FRAC_PI_2 * ((count - n) as f32 / release).min(1.0)).sin().powi(2);
            rise * fall
        } else {
            (n as f32 / attack).min(1.0) * ((count - n) as f32 / release).min(1.0)
        };
        let envelope = edges * (0.75 + 0.25 * (std::f32::consts::PI * n as f32 / count as f32).sin());
        // The harmonic fades out before it passes half the rate, where it would fold back down.
        let harmonic = syllable.harmonic * ((9500.0 - 2.0 * frequency) / 1500.0).clamp(0.0, 1.0);
        let tone = phase.sin() + harmonic * (2.0 * phase).sin();
        let value = if breathy {
            // A hollow third harmonic, breath around the pitch, and the throat's slight roughness.
            let noise = random.next();
            let air = breath.run(noise, frequency * 1.1, 3.0) * 2.5;
            let rough = 1.0 + 0.18 * throat.run(random.next(), 45.0) * 6.0;
            (tone + 0.12 * (3.0 * phase).sin()) * (1.0 - syllable.breath) * rough + air * syllable.breath
        } else {
            tone
        };
        *sample += syllable.gain * envelope * value;
    }
}

/// The kinds of birds singing at dawn here, each after a common bird of
/// European woods and gardens, without being any one of them.
#[derive(Clone, Copy)]
enum Singer {
    /// An accelerating run of down-strokes, a second phrase, a flourish (a chaffinch's song).
    Chaffinch,
    /// "Tea-cher", "tea-cher": a high slurred note and a lower one, paired (a great tit's).
    Tit,
    /// Thin, varied phrases of high hooks (a robin's).
    Robin,
    /// Low fluty warbled notes, a quiet twitter at the end (a blackbird's).
    Blackbird,
    /// A fast trill in sections of one syllable each (a wren's).
    Wren,
    /// Soft low coos, "coo, COO-coo, coo-coo" (a collared dove's).
    Dove,
}

/// One kind of song a bird sings, made once and kept while it sings it: its
/// syllables, from the song's start.
fn song_type(plan: &mut Random, singer: Singer) -> Vec<Syllable> {
    let mut song = Vec::new();
    let mut t = 0.0f32;
    match singer {
        Singer::Chaffinch => {
            let (high, low) = (plan.between(5200.0, 6800.0), plan.between(2600.0, 3400.0));
            let mut gap = plan.between(0.1, 0.12);
            for _ in 0..(5 + (plan.unit() * 4.0) as usize) {
                song.push(Syllable { at: t, length: 0.035, contour: Contour::Line { from: high, to: low }, gain: 0.8, harmonic: 0.05, breath: 0.0 });
                t += gap;
                gap *= 0.93;
            }
            let (from, to) = (plan.between(4200.0, 5200.0), plan.between(2200.0, 2800.0));
            for _ in 0..(3 + (plan.unit() * 3.0) as usize) {
                song.push(Syllable { at: t, length: 0.06, contour: Contour::Hook { from, to, tau: 0.018 }, gain: 1.0, harmonic: 0.08, breath: 0.0 });
                t += 0.085;
            }
            song.push(Syllable { at: t, length: 0.07, contour: Contour::Line { from: 2400.0, to: plan.between(4800.0, 5600.0) }, gain: 0.9, harmonic: 0.05, breath: 0.0 });
            song.push(Syllable { at: t + 0.07, length: 0.16, contour: Contour::Hook { from: 5200.0, to: plan.between(2000.0, 2600.0), tau: 0.05 }, gain: 1.0, harmonic: 0.1, breath: 0.0 });
        }
        Singer::Tit => {
            let (high, low) = (plan.between(5500.0, 7000.0), plan.between(3300.0, 4300.0));
            let period = plan.between(0.28, 0.36);
            for _ in 0..(4 + (plan.unit() * 4.0) as usize) {
                song.push(Syllable { at: t, length: 0.07, contour: Contour::Hook { from: high * 1.08, to: high * 0.92, tau: 0.03 }, gain: 0.9, harmonic: 0.04, breath: 0.0 });
                song.push(Syllable { at: t + 0.11, length: 0.09, contour: Contour::Line { from: low * 1.03, to: low * 0.97 }, gain: 1.0, harmonic: 0.06, breath: 0.0 });
                t += period;
            }
        }
        Singer::Robin => {
            for _ in 0..(6 + (plan.unit() * 6.0) as usize) {
                let length = plan.between(0.04, 0.14);
                let contour = match (plan.unit() * 3.0) as usize {
                    0 => Contour::Hook { from: plan.between(6000.0, 8000.0), to: plan.between(3500.0, 5000.0), tau: plan.between(0.01, 0.03) },
                    1 => Contour::Hook { from: plan.between(3500.0, 4500.0), to: plan.between(6000.0, 7500.0), tau: plan.between(0.015, 0.04) },
                    _ => {
                        let centre = plan.between(4000.0, 6500.0);
                        Contour::Line { from: centre * 1.04, to: centre * 0.96 }
                    }
                };
                song.push(Syllable { at: t, length, contour, gain: plan.between(0.6, 1.0), harmonic: 0.03, breath: 0.0 });
                t += length + plan.between(0.03, 0.09);
            }
        }
        Singer::Blackbird => {
            let mut centre = plan.between(1600.0, 2200.0);
            for _ in 0..(3 + (plan.unit() * 4.0) as usize) {
                let length = plan.between(0.12, 0.3);
                song.push(Syllable { at: t, length, contour: Contour::Wobble { centre, depth: plan.between(0.03, 0.08), rate: plan.between(8.0, 16.0) }, gain: 1.0, harmonic: 0.3, breath: 0.0 });
                t += length + plan.between(0.03, 0.08);
                centre = (centre * plan.between(0.85, 1.25)).clamp(1400.0, 3000.0);
            }
            t += 0.05;
            for _ in 0..(2 + (plan.unit() * 4.0) as usize) {
                song.push(Syllable { at: t, length: 0.025, contour: Contour::Line { from: plan.between(6500.0, 8000.0), to: plan.between(4500.0, 5500.0) }, gain: 0.35, harmonic: 0.0, breath: 0.0 });
                t += 0.045;
            }
        }
        Singer::Wren => {
            for _ in 0..(3 + (plan.unit() * 2.0) as usize) {
                let (from, to) = (plan.between(6000.0, 8500.0), plan.between(3500.0, 5000.0));
                let (from, to) = if plan.unit() < 0.3 { (to, from) } else { (from, to) };
                let gap = plan.between(0.045, 0.065);
                for _ in 0..(8 + (plan.unit() * 9.0) as usize) {
                    song.push(Syllable { at: t, length: 0.028, contour: Contour::Line { from, to }, gain: 1.0, harmonic: 0.05, breath: 0.0 });
                    t += gap;
                }
                t += 0.04;
            }
        }
        Singer::Dove => {
            let pitch = plan.between(430.0, 540.0);
            for (k, (length, gap)) in [(0.3, 0.18), (0.55, 0.14), (0.28, 0.4), (0.32, 0.14), (0.3, 0.0)].into_iter().enumerate() {
                let p = pitch * if k == 1 { 1.05 } else { 1.0 };
                song.push(Syllable { at: t, length, contour: Contour::Arc { from: p * 0.96, peak: p * 1.02, to: p * 0.93 }, gain: if k == 1 { 1.0 } else { 0.8 }, harmonic: 0.55, breath: 0.35 });
                t += length + gap;
            }
        }
    }
    song
}

/// One bird of the wood: how loud and how near (its distance darkens it), its
/// branch, its voice ahead of the present, the two or three kinds of song it
/// sings, each three to six times, a little different each time, before the
/// next, and its rests. It comes and goes over minutes; one come back is
/// another bird, with songs of its own, on another branch.
struct Bird {
    singer: Singer,
    gain: f32,
    open: f32,
    to_left: f32,
    to_right: f32,
    distance: LowPass,
    voice: Ahead,
    kinds: Vec<Vec<Syllable>>,
    kind: usize,
    sung: usize,
    times: usize,
    next: f64,
    rests: (f32, f32),
    here: Drift,
    /// How much of `here` it takes to sing: the dove sings least often.
    stays: f32,
    singing: bool,
}

impl Bird {
    /// A bird come to sing here: its songs, its branch.
    fn arrive(&mut self, plan: &mut Random) {
        self.kinds = (0..2 + (plan.unit() * 2.0) as usize).map(|_| song_type(plan, self.singer)).collect();
        (self.kind, self.sung, self.times) = (0, 0, 3 + (plan.unit() * 4.0) as usize);
        let (to_left, to_right) = panned(plan.between(-0.8, 0.8));
        (self.to_left, self.to_right) = (to_left * self.gain, to_right * self.gain);
    }

    /// Its next song, from `self.next`, put in its voice; how long it lasts.
    fn sing(&mut self, plan: &mut Random) -> f32 {
        let song = &self.kinds[self.kind];
        let (tune, pace) = (plan.between(0.985, 1.015), plan.between(0.97, 1.03));
        let length = song.last().map_or(0.0, |s| s.at * pace + s.length);
        let at = frame(self.next);
        let mut sound = vec![0.0f32; (length * NATURE_RATE as f32) as usize + 1];
        for syllable in song {
            add_syllable(&mut sound, at, &Syllable { at: syllable.at * pace, contour: syllable.contour.scaled(tune), ..*syllable });
        }
        self.voice.add(at, sound.len(), |n| (sound[n], 0.0));
        self.sung += 1;
        if self.sung == self.times {
            (self.kind, self.sung, self.times) = ((self.kind + 1) % self.kinds.len(), 0, 3 + (plan.unit() * 4.0) as usize);
        }
        length
    }
}

/// A short reverberation of the forest, one ear, made as it goes: four damped
/// combs and two all-passes (Schroeder's, Moorer's damping), each ear its own
/// delays.
struct Reverb {
    combs: Vec<(Vec<f32>, f32)>,
    passes: Vec<Vec<f32>>,
    at: usize,
}

impl Reverb {
    fn new(delays: [f32; 6]) -> Reverb {
        let line = |delay: f32| vec![0.0f32; ((delay * NATURE_RATE as f32) as usize).max(1)];
        Reverb { combs: delays[..4].iter().map(|d| (line(*d), 0.0)).collect(), passes: delays[4..].iter().map(|d| line(*d)).collect(), at: 0 }
    }

    fn next(&mut self, dry: f32) -> f32 {
        let mut out = 0.0f32;
        for (line, damp) in &mut self.combs {
            let k = self.at % line.len();
            let back = line[k];
            *damp += 0.45 * (back - *damp);
            line[k] = dry + *damp * 0.72;
            out += back * 0.25;
        }
        for line in &mut self.passes {
            let k = self.at % line.len();
            let back = line[k];
            let held = out + 0.6 * back;
            line[k] = held;
            out = back - 0.6 * held;
        }
        self.at = self.at.wrapping_add(1);
        out
    }
}

/// Birds at dawn in a wood (after six CC0 recordings, docs/sounds.md): six
/// birds, each singing its songs from where it sits, near or far (the far
/// ones darker), the wood singing more, then less, over tens of seconds, birds
/// leaving and others coming over minutes; the wood's short echo, and a faint,
/// steady air under them.
struct Birds {
    plan: Random,
    busy: Drift,
    birds: Vec<Bird>,
    echo: [Reverb; 2],
    air: Layer,
}

impl Birds {
    fn new(seed: u64) -> Birds {
        let mut plan = Random::part(seed, 1);
        // Each: how loud, how open (its distance), its rests between songs, how much it stays.
        let band = [
            (Singer::Chaffinch, 0.5, 9000.0, (5.0, 10.0), 0.3),
            (Singer::Tit, 0.4, 9000.0, (4.0, 9.0), 0.3),
            (Singer::Robin, 0.3, 7000.0, (2.5, 6.0), 0.3),
            (Singer::Blackbird, 0.45, 9000.0, (3.0, 7.0), 0.3),
            (Singer::Wren, 0.22, 4500.0, (8.0, 16.0), 0.35),
            (Singer::Dove, 0.12, 3000.0, (12.0, 25.0), 0.45),
        ];
        let birds = band
            .into_iter()
            .zip(10u64..)
            .map(|((singer, gain, open, rests, stays), part)| {
                let next = f64::from(plan.between(0.0, rests.1));
                let mut bird = Bird { singer, gain, open, to_left: 0.0, to_right: 0.0, distance: LowPass::default(), voice: Ahead::default(), kinds: Vec::new(), kind: 0, sung: 0, times: 0, next, rests, here: Drift::new(Random::part(seed, part), 150.0), stays, singing: true };
                bird.arrive(&mut plan);
                bird
            })
            .collect();
        Birds {
            plan,
            busy: Drift::new(Random::part(seed, 2), 18.0),
            birds,
            echo: [Reverb::new([0.0297, 0.0371, 0.0411, 0.0437, 0.005, 0.0017]), Reverb::new([0.0313, 0.0353, 0.0449, 0.0479, 0.0047, 0.0019])],
            air: Layer::new(Random::part(seed, 3), 60.0, 700.0),
        }
    }
}

impl Scene for Birds {
    fn render(&mut self, now: u64, left: &mut [f32], right: &mut [f32]) {
        let end = block_end(now, left.len());
        let busy = self.busy.now();
        for bird in &mut self.birds {
            while bird.next < end {
                // Gone, or come back, a little apart, so that a bird never flickers in and out.
                let here = bird.here.now();
                if bird.singing && here < bird.stays - 0.05 {
                    bird.singing = false;
                } else if !bird.singing && here > bird.stays + 0.05 {
                    bird.singing = true;
                    bird.arrive(&mut self.plan);
                }
                let length = if bird.singing { bird.sing(&mut self.plan) } else { 0.0 };
                bird.next += f64::from(length + self.plan.between(bird.rests.0, bird.rests.1) * (1.4 - 0.8 * busy));
            }
            bird.here.skip(left.len());
        }
        self.busy.skip(left.len());
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (mut dry_left, mut dry_right) = (0.0f32, 0.0f32);
            for bird in &mut self.birds {
                let heard = bird.distance.run(bird.voice.take().0, bird.open);
                dry_left += heard * bird.to_left;
                dry_right += heard * bird.to_right;
            }
            // A faint, steady air: the wood's own quiet, low and the same in both ears.
            let air = self.air.next() * 0.06;
            *l = dry_left + self.echo[0].next(dry_left) * 0.45 + air;
            *r = dry_right + self.echo[1].next(dry_right) * 0.45 + air;
        }
    }
}

// Made as it plays.

/// How much each moment is held down so that no peak passes −6 dBFS, made as
/// it goes, 30 ms behind: a gain that dips within 5 ms before a loud moment (a
/// thunderclap, a near bird) and comes back over 0.4 s, the same for both
/// ears. The waveform itself is never clipped nor rounded off, which would add
/// harmonics and crackle.
struct Limiter {
    attack: f32,
    release: f32,
    look: usize,
    /// The frames not yet given, each with the gain it may have at most.
    waiting: VecDeque<(f32, f32, f32)>,
    held: f32,
}

impl Limiter {
    fn new(rate: u32) -> Limiter {
        let rate = rate as f32;
        Limiter { attack: (-1.0 / (0.005 * rate)).exp(), release: (-1.0 / (0.4 * rate)).exp(), look: (0.03 * rate) as usize, waiting: VecDeque::new(), held: 1.0 }
    }

    /// A frame in; the one 30 ms before it out, held down as it must be.
    fn run(&mut self, left: f32, right: f32) -> (f32, f32) {
        let most = (0.5 / left.abs().max(right.abs()).max(1e-9)).min(1.0);
        if most < 1.0 {
            // Backwards: the gain is already down when the peak comes.
            let mut want = most;
            for frame in self.waiting.iter_mut().rev() {
                want = 1.0 - (1.0 - want) * self.attack;
                if want >= frame.2 {
                    break;
                }
                frame.2 = want;
            }
        }
        self.waiting.push_back((left, right, most));
        if self.waiting.len() <= self.look {
            return (0.0, 0.0);
        }
        let (left, right, most) = self.waiting.pop_front().unwrap_or((0.0, 0.0, 1.0));
        // Forwards: it comes back slowly after.
        self.held = most.min(1.0 - (1.0 - self.held) * self.release);
        (left * self.held, right * self.held)
    }
}

/// A sound made as it plays (docs/sounds.md): `fill` gives its next frames.
pub struct Live {
    scene: Box<dyn Scene>,
    rate: u32,
    gain: f32,
    limiter: Limiter,
    now: u64,
    left: Vec<f32>,
    right: Vec<f32>,
    /// The frames of `left` and `right` already given.
    given: usize,
}

impl Live {
    /// `kind` made from `seed`: a noise or a sound of nature; none for another name.
    pub fn new(kind: &str, seed: u64) -> Option<Live> {
        // Each recipe's own level over minutes (measured over five to ten minutes, three
        // seeds), and the level wanted (`levels_hold_over_minutes`): noises
        // −20 dBFS, birds and rain −22, the storm −23; waves, crickets and wind,
        // backgrounds to rest or focus by, lighter: −26, −26, −29.
        let (scene, rate, made, wanted): (Box<dyn Scene>, u32, f32, f32) = match kind {
            "white" | "pink" | "brown" => {
                let (colour, made) = match kind {
                    "white" => (Colour::White, 0.577),
                    "pink" => (Colour::Pink, 0.193),
                    _ => (Colour::Brown, 0.198),
                };
                (Box::new(Noise { colour, random: Random::part(seed, 1), pink: Pink::default(), last: 0.0 }), RATE, made, 0.1)
            }
            "waves" => (Box::new(Waves::new(seed)), NATURE_RATE, 0.0631, 0.05),
            "rain" => (Box::new(Rain::new(seed)), NATURE_RATE, 0.0844, 0.08),
            "wind" => (Box::new(Wind::new(seed)), NATURE_RATE, 0.0254, 0.035),
            "crickets" => (Box::new(Crickets::new(seed)), NATURE_RATE, 0.0419, 0.05),
            "storm" => (Box::new(Storm::new(seed)), NATURE_RATE, 0.0222, 0.068),
            "birds" => (Box::new(Birds::new(seed)), NATURE_RATE, 0.0974, 0.078),
            _ => return None,
        };
        let mut live = Live { scene, rate, gain: wanted / made, limiter: Limiter::new(rate), now: 0, left: Vec::new(), right: Vec::new(), given: 0 };
        // Three seconds made and left out: the filters settle, a wave is under way.
        live.fill(&mut vec![0i16; 3 * rate as usize * 2]);
        Some(live)
    }

    /// Its frames a second.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Its next frames, 16 bits, the two ears in turn (left, right, left…).
    pub fn fill(&mut self, out: &mut [i16]) {
        let pcm = |x: f32| (x.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        for pair in out.chunks_exact_mut(2) {
            if self.given == self.left.len() {
                (self.left, self.right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
                self.scene.render(self.now, &mut self.left, &mut self.right);
                self.now += BLOCK as u64;
                self.given = 0;
            }
            let (left, right) = self.limiter.run(self.left[self.given] * self.gain, self.right[self.given] * self.gain);
            self.given += 1;
            (pair[0], pair[1]) = (pcm(left), pcm(right));
        }
    }
}

/// What a sound's thread and its reader share: the frames made, not yet read.
struct Shared {
    ready: Mutex<VecDeque<i16>>,
    wake: Condvar,
    stop: AtomicBool,
}

/// A sound made on a thread of its own, a second ahead of what plays, and read
/// as it plays (the window's audio output, crates/sioul-app/src/livesound.rs).
/// Its thread ends soon after it is dropped.
pub struct Stream {
    shared: Arc<Shared>,
    rate: u32,
}

impl Stream {
    /// `kind` made from `seed`, a noise or a sound of nature; none for another
    /// name, or when no thread can be started.
    pub fn start(kind: &str, seed: u64) -> Option<Stream> {
        let rate = if NOISES.contains(&kind) {
            RATE
        } else if NATURE.contains(&kind) {
            NATURE_RATE
        } else {
            return None;
        };
        let shared = Arc::new(Shared { ready: Mutex::new(VecDeque::new()), wake: Condvar::new(), stop: AtomicBool::new(false) });
        let (theirs, kind) = (Arc::clone(&shared), kind.to_owned());
        std::thread::Builder::new().name("sioul-sound".into()).spawn(move || make(&kind, seed, &theirs)).ok()?;
        Some(Stream { shared, rate })
    }

    /// Its frames a second, both ears.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Its next samples into `out`, the two ears in turn; silence where none
    /// is made yet (at its start), never waiting.
    pub fn read(&self, out: &mut [i16]) {
        let mut ready = self.shared.ready.lock().unwrap_or_else(PoisonError::into_inner);
        let made = out.len().min(ready.len());
        for (sample, made) in out.iter_mut().zip(ready.drain(..made)) {
            *sample = made;
        }
        out[made..].fill(0);
        drop(ready);
        self.shared.wake.notify_one();
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.wake.notify_all();
    }
}

/// A stream's thread: `kind` made a tenth of a second at a time, a second
/// ahead of its reader, until the stream is dropped.
fn make(kind: &str, seed: u64, shared: &Shared) {
    let Some(mut live) = Live::new(kind, seed) else { return };
    let ahead = live.rate as usize * 2;
    let mut block = vec![0i16; live.rate as usize / 10 * 2];
    loop {
        {
            let mut ready = shared.ready.lock().unwrap_or_else(PoisonError::into_inner);
            while ready.len() >= ahead && !shared.stop.load(Ordering::Acquire) {
                ready = shared.wake.wait(ready).unwrap_or_else(PoisonError::into_inner);
            }
        }
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        live.fill(&mut block);
        shared.ready.lock().unwrap_or_else(PoisonError::into_inner).extend(&block);
    }
}

// Files, where a sound cannot be made as it plays.

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

/// The recipe's version of a file of the sounds' folder made here
/// ("waves-5.wav", "pink-noise-3.wav"; 1 for the noises' first files,
/// "pink-noise.wav"); none for another file.
fn made_here(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(".wav")?;
    if let Some(kind) = stem.strip_suffix("-noise") {
        return NOISES.contains(&kind).then_some(1);
    }
    let (kind, version) = stem.rsplit_once('-')?;
    let ours = kind.strip_suffix("-noise").is_some_and(|kind| NOISES.contains(&kind)) || NATURE.contains(&kind);
    ours.then(|| version.parse::<u32>().ok()).flatten()
}

/// Whether a file of the sounds' folder was made by an older recipe.
fn older_recipe(name: &str) -> bool {
    made_here(name).is_some_and(|version| version < NATURE_VERSION)
}

/// The files made here for where a sound cannot be made as it plays, each
/// minutes of sound, taken away: a sound made as it plays needs none.
pub fn clear_files(folder: &Path) {
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            if made_here(&entry.file_name().to_string_lossy()).is_some() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// `kind` over `seconds`, looping: its last four seconds faded into its first
/// with equal power, so that the end flows into the start; and its rate.
fn looped(kind: &str, seconds: usize) -> Option<(Vec<i16>, u32)> {
    let mut live = Live::new(kind, 0x5105)?;
    let rate = live.rate() as usize;
    let (length, fade) = (seconds * rate * 2, 4 * rate * 2);
    let mut raw = vec![0i16; length + fade];
    live.fill(&mut raw);
    let mut out = raw[..length].to_vec();
    for (i, sample) in out.iter_mut().take(fade).enumerate() {
        let t = (i / 2) as f32 / (fade / 2) as f32;
        *sample = (f32::from(raw[i]) * t.sqrt() + f32::from(raw[length + i]) * (1.0 - t).sqrt()).clamp(-32768.0, 32767.0) as i16;
    }
    Some((out, live.rate()))
}

/// The file of a noise or a sound of nature, made the first time in `folder`,
/// for where it cannot be made as it plays: one to three minutes, looping.
pub fn noise_file(folder: &Path, kind: &str) -> Result<PathBuf, String> {
    let (name, seconds) = if NOISES.contains(&kind) {
        (format!("{kind}-noise-{NATURE_VERSION}.wav"), 60)
    } else if NATURE.contains(&kind) {
        let seconds = match kind {
            "waves" => 120,
            "storm" => 180,
            "wind" | "birds" => 90,
            _ => 60,
        };
        (format!("{kind}-{NATURE_VERSION}.wav"), seconds)
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
        let (samples, rate) = looped(kind, seconds).ok_or_else(|| format!("{kind}: no such sound"))?;
        std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let temporary = path.with_extension("wav.new");
        std::fs::write(&temporary, wav_of(&samples, 2, rate)).map_err(|e| format!("{}: {e}", temporary.display()))?;
        std::fs::rename(&temporary, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(path)
}

/// `seconds` of `kind` made from `seed`, as WAV bytes: to listen to a sound
/// or to measure it (tools/sounds/).
pub fn recording(kind: &str, seed: u64, seconds: usize) -> Option<Vec<u8>> {
    let mut live = Live::new(kind, seed)?;
    let mut samples = vec![0i16; seconds * live.rate() as usize * 2];
    live.fill(&mut samples);
    Some(wav_of(&samples, 2, live.rate()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `seconds` of `kind` from `seed`, each ear apart.
    fn made(kind: &str, seed: u64, seconds: usize) -> (Vec<i16>, Vec<i16>) {
        let mut live = Live::new(kind, seed).unwrap();
        let mut out = vec![0i16; seconds * live.rate() as usize * 2];
        live.fill(&mut out);
        out.chunks_exact(2).map(|pair| (pair[0], pair[1])).unzip()
    }

    fn rms(samples: &[i16]) -> f64 {
        (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt() / 32767.0
    }

    fn correlation(left: &[i16], right: &[i16]) -> f64 {
        let (l, r): (Vec<f64>, Vec<f64>) = left.iter().zip(right).map(|(l, r)| (f64::from(*l), f64::from(*r))).unzip();
        let mean = |x: &[f64]| x.iter().sum::<f64>() / x.len() as f64;
        let (ml, mr) = (mean(&l), mean(&r));
        let cov = l.iter().zip(&r).map(|(a, b)| (a - ml) * (b - mr)).sum::<f64>();
        let var = |x: &[f64], m: f64| x.iter().map(|a| (a - m).powi(2)).sum::<f64>();
        cov / (var(&l, ml) * var(&r, mr)).sqrt()
    }

    /// Energy around a frequency, per sample and per hertz of the band (Q = 2).
    fn energy(samples: &[i16], frequency: f32) -> f32 {
        let mut band = BandPass::default();
        let power = samples.iter().map(|&x| band.run(f32::from(x) / 32768.0, frequency, 2.0).powi(2)).sum::<f32>() / samples.len() as f32;
        power / (frequency / 2.0)
    }

    #[test]
    fn the_random_source_is_centred() {
        let mut random = Random(7);
        let values: Vec<f32> = (0..100_000).map(|_| random.next()).collect();
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!(mean.abs() < 0.01, "mean {mean}");
        assert!(values.iter().all(|v| (-1.0..1.0).contains(v)));
    }

    /// A sound is the same however its reader reads it: made a block at a
    /// time, never where the reader stops.
    #[test]
    fn a_sound_is_the_same_however_it_is_read() {
        for kind in ["waves", "birds", "storm", "pink"] {
            let mut whole = Live::new(kind, 3).unwrap();
            let mut one = vec![0i16; 8 * whole.rate() as usize * 2];
            whole.fill(&mut one);
            let mut pieces = Live::new(kind, 3).unwrap();
            let mut other = vec![0i16; one.len()];
            let (mut at, mut size) = (0, 2);
            while at < other.len() {
                let end = (at + size).min(other.len());
                pieces.fill(&mut other[at..end]);
                at = end;
                size = ((size * 7 + 2) % 9000).max(2) & !1;
            }
            assert!(one == other, "{kind}");
        }
    }

    /// Where one block ends and the next begins, a sound steps no more than
    /// within a block: no click every tenth of a second.
    #[test]
    fn a_sound_flows_on_without_a_seam() {
        for kind in NATURE.iter().chain(NOISES) {
            let (left, _) = made(kind, 5, 20);
            let rate = if NOISES.contains(kind) { RATE } else { NATURE_RATE } as usize;
            // The frames given lag the frames made by the limiter's look ahead, after the three seconds left out.
            let shift = 3 * rate - (0.03 * rate as f32) as usize;
            let (mut seams, mut within) = ((0f64, 0usize), (0f64, 0usize));
            for (i, pair) in left.windows(2).enumerate() {
                let step = (f64::from(pair[1]) - f64::from(pair[0])).abs();
                let sum = if (i + 1 + shift) % BLOCK == 0 { &mut seams } else { &mut within };
                (sum.0, sum.1) = (sum.0 + step, sum.1 + 1);
            }
            let (seam, usual) = (seams.0 / seams.1 as f64, within.0 / within.1 as f64);
            assert!(seams.1 > 50 && seam < 2.0 * usual, "{kind}: {seam} at seams, {usual} within");
        }
    }

    /// Each sound sits near its level, never above −6 dBFS, and is quiet: each
    /// recipe's own level was measured over an hour and is what `Live::new`
    /// brings to the level wanted.
    #[test]
    fn levels_hold_over_minutes() {
        for (kind, wanted) in [("white", 0.1), ("pink", 0.1), ("brown", 0.1), ("waves", 0.05), ("rain", 0.08), ("wind", 0.035), ("crickets", 0.05), ("storm", 0.068), ("birds", 0.078)] {
            let (left, right) = made(kind, 11, 90);
            let both: Vec<i16> = left.iter().chain(&right).copied().collect();
            let db = 20.0 * (rms(&both) / wanted).log10();
            assert!(db.abs() < 3.0, "{kind}: {db:.1} dB from its level");
            assert!(both.iter().all(|s| s.unsigned_abs() <= 16_400), "{kind}: a peak above −6 dBFS");
        }
    }

    #[test]
    fn sounds_sound_like_themselves() {
        // Crickets sing near 4.5 kHz, far above their night's air; waves rumble low.
        let (crickets, _) = made("crickets", 1, 40);
        assert!(energy(&crickets, 4500.0) > 5.0 * energy(&crickets, 1000.0));
        let (waves, _) = made("waves", 1, 60);
        assert!(energy(&waves, 150.0) > energy(&waves, 5000.0));
        // Birds sing between 2 and 5 kHz, above the wood's own air.
        let (birds, _) = made("birds", 1, 60);
        assert!(energy(&birds, 3500.0) > 2.0 * energy(&birds, 800.0));
        // Brown is darker than white: smaller steps between samples.
        let mean_step = |s: &[i16]| s.windows(2).map(|w| (i32::from(w[0]) - i32::from(w[1])).abs() as i64).sum::<i64>() / s.len() as i64;
        assert!(mean_step(&made("brown", 1, 1).0) * 4 < mean_step(&made("white", 1, 1).0));
    }

    /// The two ears hear one beach, one wood, one storm: partly alike, as the
    /// recordings they were tuned against (0.4 to 1 in their lows), never one
    /// sound twice nor two unrelated ones.
    #[test]
    fn the_two_ears_are_partly_alike() {
        for kind in ["waves", "wind", "storm", "birds"] {
            let (left, right) = made(kind, 2, 60);
            let correlation = correlation(&left, &right);
            assert!((0.3..0.99).contains(&correlation), "{kind}: {correlation}");
        }
    }

    /// Each listen is another: two seeds, two unrelated sounds.
    #[test]
    fn each_listen_is_another() {
        for kind in ["waves", "rain", "birds", "pink"] {
            let (one, _) = made(kind, 1, 20);
            let (other, _) = made(kind, 2, 20);
            let correlation = correlation(&one, &other);
            assert!(correlation.abs() < 0.2, "{kind}: {correlation}");
        }
    }

    /// A stream is read as its thread makes it: silence at first, then sound.
    #[test]
    fn a_stream_is_made_ahead_of_its_reader() {
        let stream = Stream::start("pink", 9).unwrap();
        assert_eq!(stream.rate(), RATE);
        let mut out = vec![0i16; 4410];
        let started = std::time::Instant::now();
        while out.iter().all(|s| *s == 0) && started.elapsed() < std::time::Duration::from_secs(20) {
            std::thread::sleep(std::time::Duration::from_millis(20));
            stream.read(&mut out);
        }
        assert!(out.iter().any(|s| *s != 0), "no sound within 20 s");
        assert!(Stream::start("thunder", 9).is_none());
    }

    /// A file of a sound loops without a seam: its end flows into its start.
    #[test]
    fn a_file_loops_without_a_seam() {
        let (samples, rate) = looped("pink", 5).unwrap();
        assert_eq!((samples.len(), rate), (5 * RATE as usize * 2, RATE));
        let step = |a: i16, b: i16| (i32::from(a) - i32::from(b)).abs();
        let seam = step(samples[samples.len() - 2], samples[0]);
        let typical = samples.chunks_exact(2).collect::<Vec<_>>().windows(2).map(|w| step(w[0][0], w[1][0])).max().unwrap_or(0);
        assert!(seam <= typical, "seam {seam}, steps up to {typical}");
    }

    #[test]
    fn only_files_made_here_are_taken_away() {
        let older = format!("waves-{}.wav", NATURE_VERSION - 1);
        let noise = format!("pink-noise-{}.wav", NATURE_VERSION - 1);
        assert!(older_recipe(&older) && older_recipe(&noise) && older_recipe("brown-noise.wav"));
        assert!(made_here(&format!("waves-{NATURE_VERSION}.wav")).is_some() && !older_recipe(&format!("waves-{NATURE_VERSION}.wav")));
        for kept in ["my-rain-2.wav", "waves-2.ogg", "waves.wav", "notes-1.wav", "pink-noise-x.wav", "red-noise.wav"] {
            assert!(made_here(kept).is_none(), "{kept}");
        }
        let folder = std::env::temp_dir().join(format!("sioul-sounds-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        for name in ["waves-6.wav", "pink-noise.wav", "my-rain-2.wav"] {
            std::fs::write(folder.join(name), b"").unwrap();
        }
        clear_files(&folder);
        let left: Vec<String> = std::fs::read_dir(&folder).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, vec!["my-rain-2.wav".to_owned()]);
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_wav_header_says_what_follows() {
        let bytes = wav_of(&[0, 1, -1], 1, RATE);
        assert_eq!(&bytes[..4], b"RIFF");
        let stereo = wav_of(&[0, 1, -1, 2], 2, NATURE_RATE);
        assert_eq!(u16::from_le_bytes(stereo[22..24].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(stereo[28..32].try_into().unwrap()), NATURE_RATE * 4);
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 6);
        assert_eq!(bytes.len(), 44 + 6);
    }
}
