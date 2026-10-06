// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! What a day can hold, learned from your own days (docs/capacity.md).
//!
//! - **Heaviness** of a task or an event is its highest cost (`Demands::level`);
//!   unrated, its word (`level_of`).
//! - **Ratings for the plan**: said after the task when you said so (the
//!   median of the last five of the same item, else of its kind), else
//!   forecast (`FeltIndex`). Ratings given after are more accurate, anxiety
//!   is over-predicted beforehand (capacity-budget.md, criteria 6–7; G6).
//! - **Load** of an item: rating × hours, at least half an hour each
//!   (session-RPE, Foster 2001; criteria 10–11), per cost and in total.
//! - **Time**: your estimate corrected by your own record, the log of time
//!   spent over the first guess on finished tasks, recent ones weighing more,
//!   pulled toward 1.1× until about nine tasks stand behind it, per area
//!   pulled toward yours (`Ratios`; time-estimation.md, TE7–15); the day's
//!   margin held as free time at the 85th percentile of the day's total, by
//!   a seeded simulation with a shared day effect (`slack`; TE16–20).
//! - **Budgets**, per cost and in total, learned from days whose outcome you
//!   gave (`learn`; criteria 13–25), replayed from the record each time so
//!   that every device, holding the same record, plans the same.
//! - **The gain minimum**: good hours on good days, their 25th percentile,
//!   never learned downward from a bad stretch (G8–G10).
//!
//! Nothing here is ever shown as a number: the pages get words and reasons.

use crate::agenda::Occurrence;
use crate::demands::{Demands, Level};
use crate::i18n::Translator;
use crate::tasks::{Status, Task};
use crate::timelog::{Kind, Session};
use fluent_bundle::FluentArgs;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The four costs, then their total, in this order everywhere.
pub const SCALES: [&str; 5] = ["cognitive", "emotional", "anxiety", "body", "total"];
/// The total's place in a `Load`.
pub const TOTAL: usize = 4;
/// Rating × hours, per cost and in total.
pub type Load = [f32; 5];

/// Every item counts for at least this long, so a dreaded ten-minute call is
/// not free (criterion 11). A guess to tune.
pub const LEAST_MINUTES: u32 = 30;
/// Each day is planned to this share of each budget (criterion 27). A guess (CB-Q18).
pub const FILL: f32 = 0.85;
/// Days before and after a heavy event hold this share (criterion 34). A guess.
pub const AROUND_HEAVY: f32 = 0.75;
/// An event this heavy (its highest cost) lightens the days around it.
pub const HEAVY_EVENT: u8 = 7;

/// A task's heaviness: from its costs when any is rated, else its word.
pub fn level_of(task: &Task) -> Level {
    task.demands.level().unwrap_or_else(|| Level::of_energy(&task.energy))
}

fn rounded(x: f32) -> u8 {
    x.round().clamp(0.0, 10.0) as u8
}

/// The ratings the plan uses for an item: each cost and the gain, possibly
/// medians of several ratings, and its word for what is unrated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    pub costs: [Option<f32>; 4],
    pub gain: Option<f32>,
    /// Its word, for an item without any cost rated.
    pub word: Level,
}

impl Default for Rates {
    fn default() -> Rates {
        Rates { costs: [None; 4], gain: None, word: Level::Usual }
    }
}

impl Rates {
    pub fn of(demands: &Demands, word: Level) -> Rates {
        Rates { costs: demands.costs().map(|c| c.map(f32::from)), gain: demands.gain.map(f32::from), word }
    }

    /// The ratings an item counts at, as written in it: a task's, an event's.
    pub fn of_task(task: &Task) -> Rates {
        Rates::of(&task.demands, Level::of_energy(&task.energy))
    }

    pub fn of_event(event: &Occurrence) -> Rates {
        Rates::of(&event.demands, Level::Usual)
    }

    pub fn highest(&self) -> Option<f32> {
        self.costs.iter().flatten().copied().fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))))
    }

    /// Heaviness: the highest cost when any is rated, else the word.
    pub fn level(&self) -> Level {
        match self.highest() {
            Some(h) => Level::of(rounded(h), self.gain.map(rounded)),
            None => self.word,
        }
    }

    /// The load of an hour: each cost rated, and the total at the item's
    /// heaviness: its highest cost, or its word's rating when none is rated
    /// (light 2, usual 4, heavy 7; docs/capacity.md, why the highest and not the sum).
    pub fn per_hour(&self) -> Load {
        let mut load = [0.0; 5];
        for (s, cost) in self.costs.iter().enumerate() {
            load[s] = cost.unwrap_or(0.0);
        }
        load[TOTAL] = self.highest().unwrap_or_else(|| self.word.rating());
        load
    }

    /// The load of `minutes` of it, half an hour at least.
    pub fn load(&self, minutes: u32) -> Load {
        let hours = minutes.max(LEAST_MINUTES) as f32 / 60.0;
        self.per_hour().map(|r| r * hours)
    }

    /// The cost weighing most, when it weighs (4 or more): to alternate kinds of cost.
    pub fn dominant(&self) -> Option<usize> {
        let (index, value) = self.costs.iter().enumerate().filter_map(|(i, c)| c.map(|c| (i, c))).fold(None, |best: Option<(usize, f32)>, (i, c)| match best {
            Some((_, b)) if b >= c => best,
            _ => Some((i, c)),
        })?;
        (value >= 4.0).then_some(index)
    }

    /// Which costs are known.
    pub fn known(&self) -> [bool; 5] {
        let mut known = [false; 5];
        for (s, cost) in self.costs.iter().enumerate() {
            known[s] = cost.is_some();
        }
        known[TOTAL] = true;
        known
    }
}

/// Adds `b` into `a`.
pub fn add(a: &mut Load, b: &Load) {
    for s in 0..5 {
        a[s] += b[s];
    }
}

/// The felt ratings of every task, to plan from (G6): by item, a task's
/// folded title (a repeating task, or one made again each time), and by kind.
#[derive(Debug, Clone, Default)]
pub struct FeltIndex {
    items: BTreeMap<String, Vec<(Option<Date>, Demands)>>,
    kinds: BTreeMap<String, Vec<(Option<Date>, Demands)>>,
}

/// How many felt ratings the median looks back over (G6). A guess.
pub const FELT_MEDIAN_OF: usize = 5;

fn item_key(task: &Task) -> String {
    let title: String = crate::text::fold(task.title.trim()).into_iter().collect::<String>().to_lowercase();
    if title.is_empty() { format!("uid:{}", task.uid) } else { title }
}

impl FeltIndex {
    pub fn of(tasks: &[Task]) -> FeltIndex {
        let mut index = FeltIndex::default();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for task in tasks.iter().filter(|t| !t.felt.is_empty()) {
            if !seen.insert(task.uid.as_str()) {
                continue;
            }
            let ratings: Vec<(Option<Date>, Demands)> = task.felt.iter().map(|f| (f.on, f.demands)).collect();
            index.items.entry(item_key(task)).or_default().extend(ratings.iter().copied());
            if !task.kind.is_empty() {
                index.kinds.entry(task.kind.clone()).or_default().extend(ratings);
            }
        }
        for list in index.items.values_mut().chain(index.kinds.values_mut()) {
            list.sort_by_key(|(on, _)| *on);
        }
        index
    }

    /// The median of the last ratings of one value, oldest first.
    fn median(ratings: &[(Option<Date>, Demands)], pick: impl Fn(&Demands) -> Option<u8>) -> Option<f32> {
        let mut last: Vec<f32> = ratings.iter().rev().filter_map(|(_, d)| pick(d)).take(FELT_MEDIAN_OF).map(f32::from).collect();
        if last.is_empty() {
            return None;
        }
        last.sort_by(f32::total_cmp);
        let n = last.len();
        Some(if n % 2 == 1 { last[n / 2] } else { (last[n / 2 - 1] + last[n / 2]) / 2.0 })
    }

    /// The ratings the plan uses for a task, value by value: the median of
    /// the last five felt of the same item; else its forecast; else the median
    /// of the last five felt of its kind (call, write…) for what it leaves unsaid.
    pub fn rates(&self, task: &Task) -> Rates {
        let item = self.items.get(&item_key(task)).map(Vec::as_slice).unwrap_or(&[]);
        let kind = if task.kind.is_empty() { &[][..] } else { self.kinds.get(&task.kind).map(Vec::as_slice).unwrap_or(&[]) };
        let value = |pick: &dyn Fn(&Demands) -> Option<u8>| FeltIndex::median(item, pick).or_else(|| pick(&task.demands).map(f32::from)).or_else(|| FeltIndex::median(kind, pick));
        let costs = [value(&|d: &Demands| d.cognitive), value(&|d: &Demands| d.emotional), value(&|d: &Demands| d.anxiety), value(&|d: &Demands| d.body)];
        Rates { costs, gain: value(&|d: &Demands| d.gain), word: Level::of_energy(&task.energy) }
    }

    /// Whether anything was said after any task.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/* ---------- Time: the estimate corrected by your own record ---------- */

/// Before your own record: tasks take about 1.1× the first guess (TE9: 1.0–1.2×).
pub const PRIOR_RATIO: f32 = 1.1;
/// The spread of the log ratio before your own record (TE9).
pub const PRIOR_SIGMA: f32 = 0.5;
/// Your ratio stands on its own from this many tasks (TE9: 8–10). A guess.
pub const STANDS_FROM: f32 = 9.0;
/// A kind's ratio counts as much as yours from this many tasks (TE10: 4–11). A guess.
pub const KIND_PULL: f32 = 5.0;
/// Days for a task's weight to halve (TE8: 4–5).
pub const RATIO_HALF_LIFE: f32 = 4.5;
/// What a typed or unknown minute weighs against a timed one (TE15: little). A guess.
pub const TYPED_WEIGHT: f32 = 0.25;
/// Finished tasks older than this are not looked at.
pub const RATIO_HORIZON: i64 = 90;
/// The share of a day's spread that is the day itself: a bad day slows every task (TE18). A guess.
pub const DAY_SHARE: f32 = 0.3;
/// The day's total is planned to this percentile (TE17: 80–90). A guess.
pub const DAY_PERCENTILE: f32 = 0.85;
/// Draws of the day's simulation.
pub const DRAWS: usize = 2000;
/// The standard normal's quantile at `DAY_PERCENTILE` (0.85).
pub const Z_DAY: f32 = 1.036_433;
/// The free time a day keeps for steps running long is at most this share of
/// its room for tasks, so that it never empties a day, thin data included. A
/// guess to tune (docs/capacity.md, "The day's free time").
pub const SLACK_SHARE: f32 = 1.0 / 3.0;

/// A finished task, as the ratio learns from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Finished {
    pub uid: String,
    /// Its kind for the ratio: its area ("work", "admin", "leisure").
    pub kind: &'static str,
    pub done: Date,
    /// ln(time spent / first estimate).
    pub log: f32,
    /// What its minutes weigh: timed 1, typed or unknown `TYPED_WEIGHT`, mixed by minutes.
    pub weight: f32,
}

/// Each finished task with an estimate and time spent: its log ratio, all its
/// sessions summed (TE14); the first estimate, else the current one (a task
/// estimated before the first was kept). Dropped tasks never count.
pub fn finished(tasks: &[Task], sessions: &[Session], kind_of: &dyn Fn(&Task) -> &'static str, zone: &TimeZone) -> Vec<Finished> {
    let mut spent: BTreeMap<&str, (u32, f32)> = BTreeMap::new();
    for s in sessions.iter().filter(|s| !s.task.is_empty()) {
        let entry = spent.entry(s.task.as_str()).or_insert((0, 0.0));
        entry.0 = entry.0.saturating_add(s.minutes);
        entry.1 += s.minutes as f32 * if Kind::timed(s.kind) { 1.0 } else { TYPED_WEIGHT };
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    tasks
        .iter()
        .filter(|t| t.status == Status::Completed && seen.insert(t.uid.as_str()))
        .filter_map(|t| {
            let estimate = t.estimate_first.unwrap_or(t.estimate);
            let &(minutes, weighed) = spent.get(t.uid.as_str())?;
            if estimate == 0 || minutes == 0 {
                return None;
            }
            let done = jiff::Timestamp::from_second(t.completed?).ok()?.to_zoned(zone.clone()).date();
            let log = (minutes as f32 / estimate as f32).ln().clamp(-(8f32.ln()), 8f32.ln());
            Some(Finished { uid: t.uid.clone(), kind: kind_of(t), done, log, weight: weighed / minutes as f32 })
        })
        .collect()
}

/// Your ratio of time spent to first guesses, and each kind's.
#[derive(Debug, Clone, PartialEq)]
pub struct Ratios {
    /// The log of your ratio, pulled toward the prior until it stands.
    pub person: f32,
    /// The weight of tasks behind it (typed ones counting little).
    pub count: f32,
    /// Each kind: its log ratio pulled toward yours, and the weight behind it.
    pub kinds: BTreeMap<&'static str, (f32, f32)>,
    /// The spread of a task's log ratio around its kind's.
    pub sigma: f32,
    /// Today's own effect so far (log), from what was finished today, and how sure it is (its σ).
    pub today: f32,
    pub today_sigma: f32,
}

impl Default for Ratios {
    fn default() -> Ratios {
        let spread = Spread::of(PRIOR_SIGMA);
        Ratios { person: PRIOR_RATIO.ln(), count: 0.0, kinds: BTreeMap::new(), sigma: PRIOR_SIGMA, today: 0.0, today_sigma: spread.day }
    }
}

/// The weighted median of (value, weight).
fn weighted_quantile(mut values: Vec<(f32, f32)>, q: f32) -> Option<f32> {
    values.retain(|(_, w)| *w > 0.0);
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f32 = values.iter().map(|v| v.1).sum();
    let mut running = 0.0;
    for &(value, weight) in &values {
        running += weight;
        if running >= q * total - 1e-6 {
            return Some(value);
        }
    }
    values.last().map(|v| v.0)
}

fn median_of(mut values: Vec<f32>) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    let n = values.len();
    if n % 2 == 1 { values[n / 2] } else { (values[n / 2 - 1] + values[n / 2]) / 2.0 }
}

/// Far values pulled in to three scaled deviations of the median: real overruns
/// stay large, a timer left running all night stops ruling the mean (TE13).
fn winsorised(logs: &[f32]) -> Vec<f32> {
    if logs.len() < 5 {
        return logs.to_vec();
    }
    let centre = median_of(logs.to_vec());
    let mad = 1.4826 * median_of(logs.iter().map(|y| (y - centre).abs()).collect());
    if mad <= 0.0 {
        return logs.to_vec();
    }
    logs.iter().map(|y| y.clamp(centre - 3.0 * mad, centre + 3.0 * mad)).collect()
}

/// The ratios from finished tasks, as of `today`.
pub fn ratios(finished: &[Finished], today: Date) -> Ratios {
    let age = |d: Date| d.until(today).map_or(0, |s| i64::from(s.get_days())).max(0);
    let recent: Vec<&Finished> = finished.iter().filter(|f| age(f.done) <= RATIO_HORIZON && f.done <= today).collect();
    let logs = winsorised(&recent.iter().map(|f| f.log).collect::<Vec<_>>());
    let weight = |f: &Finished| 0.5f32.powf(age(f.done) as f32 / RATIO_HALF_LIFE) * f.weight;
    // A weighted mean, recent tasks weighing more (EWMA by days elapsed).
    let mean = |pick: &dyn Fn(&Finished) -> bool| -> Option<(f32, f32)> {
        let (mut sum, mut weights, mut count) = (0.0, 0.0, 0.0);
        for (f, y) in recent.iter().zip(&logs).filter(|(f, _)| pick(f)) {
            let w = weight(f);
            sum += w * y;
            weights += w;
            count += f.weight;
        }
        (weights > 0.0).then(|| (sum / weights, count))
    };
    let prior = PRIOR_RATIO.ln();
    let mut out = Ratios::default();
    if let Some((mean, count)) = mean(&|_| true) {
        let w = (count / STANDS_FROM).min(1.0);
        out.person = w * mean + (1.0 - w) * prior;
        out.count = count;
    }
    let kinds: BTreeSet<&'static str> = recent.iter().map(|f| f.kind).collect();
    for kind in kinds {
        if let Some((mean, count)) = mean(&|f| f.kind == kind) {
            let w = count / (count + KIND_PULL);
            out.kinds.insert(kind, (w * mean + (1.0 - w) * out.person, count));
        }
    }
    // The spread around each kind, robust; the prior's until ten tasks, widened for few (TE20).
    let residuals: Vec<f32> = recent.iter().zip(&logs).map(|(f, y)| y - out.log_of(f.kind)).collect();
    let n = residuals.len() as f32;
    let sigma = if residuals.len() >= 10 { (1.4826 * median_of(residuals.iter().map(|r| r.abs()).collect())).clamp(0.2, 1.0) } else { PRIOR_SIGMA };
    out.sigma = sigma * (1.0 + 1.0 / n.max(1.0)).sqrt();
    // Today so far: what was finished today says how today goes, shrunk toward an ordinary day (TE19).
    let spread = Spread::of(out.sigma);
    let today_residuals: Vec<f32> = recent.iter().zip(&logs).filter(|(f, _)| f.done == today).map(|(f, y)| y - out.log_of(f.kind)).collect();
    let k = today_residuals.len() as f32;
    if k > 0.0 {
        let ratio = spread.item * spread.item / (spread.day * spread.day);
        out.today = today_residuals.iter().sum::<f32>() / (k + ratio);
        out.today_sigma = (1.0 / (1.0 / (spread.day * spread.day) + k / (spread.item * spread.item))).sqrt();
    } else {
        out.today_sigma = spread.day;
    }
    out
}

impl Ratios {
    /// The log ratio of a kind; yours when it has none.
    pub fn log_of(&self, kind: &str) -> f32 {
        self.kinds.get(kind).map_or(self.person, |k| k.0)
    }

    pub fn factor(&self, kind: &str) -> f32 {
        self.log_of(kind).exp()
    }

    /// A task's length as the plan lays it: its estimate times its kind's
    /// ratio. Only for a task with an estimate of its own: the person's
    /// estimate is corrected, never replaced (TE16, TE32).
    pub fn corrected(&self, estimate: u32, kind: &str) -> Option<u32> {
        (estimate > 0).then(|| ((estimate as f32 * self.factor(kind)).round() as u32).max(1))
    }

    /// Today's factor on what is left of today: a day going slower makes its
    /// steps longer; a faster one changes nothing (the plan never fills more
    /// on the strength of one morning).
    pub fn today_factor(&self) -> f32 {
        self.today.max(0.0).exp()
    }

    /// The line the task panel shows on request (TE25–26): only when the kind's
    /// ratio stands on at least five tasks and differs from 1× by 15 % or more.
    pub fn line(&self, kind: &str, tr: &Translator) -> String {
        let Some(&(log, count)) = self.kinds.get(kind) else { return String::new() };
        let factor = log.exp();
        if count < 5.0 || (factor - 1.0).abs() < 0.15 {
            return String::new();
        }
        let mut args = FluentArgs::new();
        args.set("factor", format!("{:.1}", (factor * 10.0).round() / 10.0).replace('.', if tr.language() == "fr" { "," } else { "." }));
        tr.text(if factor > 1.0 { "capacity-ratio-longer" } else { "capacity-ratio-shorter" }, Some(&args))
    }
}

/// A day's spread: the day's own effect, shared by its tasks, and each task's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spread {
    pub day: f32,
    pub item: f32,
}

impl Spread {
    pub fn of(sigma: f32) -> Spread {
        Spread { day: (DAY_SHARE).sqrt() * sigma, item: (1.0 - DAY_SHARE).sqrt() * sigma }
    }
}

/// A small, seeded generator (SplitMix64): the same seed, the same draws, on every device.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1).
    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// Standard normal (Box–Muller).
    fn normal(&mut self) -> f32 {
        let (u, v) = (self.uniform(), self.uniform());
        ((-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()) as f32
    }
}

/// The same free time as `slack`, by the Fenton–Wilkinson approximation of
/// the same model (Fenton 1960; TE18): a lognormal matched to the mean and
/// variance of the day's total, the shared day effect included. Cheap, from
/// two sums of the steps' medians (`sum` = Σ m, `squares` = Σ m²): the plan
/// checks with it, step by step, that a day holds its steps and their free
/// time; the free time it keeps is then `slack`'s, on what it laid.
pub fn slack_estimate(sum: f32, squares: f32, spread: Spread, effect: f32) -> f32 {
    if sum <= 0.0 {
        return 0.0;
    }
    let (day, item) = (spread.day * spread.day, spread.item * spread.item);
    let mean = (effect + (day + item) / 2.0).exp() * sum;
    // E[T²]: a step with itself, then every pair, which share the day's effect only.
    let second = (2.0 * effect).exp() * (squares * (2.0 * day + 2.0 * item).exp() + (sum * sum - squares).max(0.0) * (2.0 * day + item).exp());
    let variance = (second - mean * mean).max(0.0);
    let s2 = (1.0 + variance / (mean * mean)).ln();
    let mu = mean.ln() - s2 / 2.0;
    ((mu + Z_DAY * s2.sqrt()).exp() - sum).max(0.0)
}

/// A seed from what the day holds: its date and its steps (FNV-1a).
pub fn seed(date: Date, uids: &[&str]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in date.to_string().bytes().chain(uids.iter().flat_map(|u| u.bytes().chain([0]))) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// The free time a day keeps for its steps taking longer than their median:
/// the 85th percentile of the day's total less the sum of the medians, by a
/// simulation in which a shared effect moves every step of the day together
/// (TE16–18). `effect` and `spread.day` are today's as known so far.
pub fn slack(medians: &[u32], spread: Spread, effect: f32, seed: u64) -> u32 {
    let sum: f32 = medians.iter().map(|&m| m as f32).sum();
    if medians.is_empty() || sum <= 0.0 {
        return 0;
    }
    let mut draws = Draws(seed);
    let mut totals: Vec<f32> = (0..DRAWS)
        .map(|_| {
            let day = effect + spread.day * draws.normal();
            medians.iter().map(|&m| m as f32 * (day + spread.item * draws.normal()).exp()).sum::<f32>()
        })
        .collect();
    totals.sort_by(f32::total_cmp);
    let p = totals[((DAY_PERCENTILE * DRAWS as f32) as usize).min(DRAWS - 1)];
    (p - sum).max(0.0).round() as u32
}

/* ---------- Days: their load, their outcome ---------- */

/// How a day went, as you said it (`reviews`): too much, about right, too empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    TooMuch,
    AboutRight,
    TooEmpty,
}

impl Outcome {
    pub fn bad(self) -> bool {
        self == Outcome::TooMuch
    }

    /// Fine for the costs: about right, or too empty.
    pub fn fine(self) -> bool {
        !self.bad()
    }

    /// Good for the gain minimum: about right only.
    pub fn good(self) -> bool {
        self == Outcome::AboutRight
    }
}

/// One day of the record.
#[derive(Debug, Clone, PartialEq)]
pub struct DayRecord {
    pub date: Date,
    pub load: Load,
    /// What is known: a cost when an item of the day was rated on it; the total when anything was recorded.
    pub known: [bool; 5],
    /// Σ gain/10 × hours, each item two hours at most (G8).
    pub good_hours: f32,
    /// Whether any item of the day carried a gain.
    pub gain_known: bool,
    pub outcome: Option<Outcome>,
}

/// A gain counts this long at most (G8). A guess.
pub const GAIN_HOURS_CAP: f32 = 2.0;

impl DayRecord {
    pub fn new(date: Date) -> DayRecord {
        DayRecord { date, load: [0.0; 5], known: [false; 5], good_hours: 0.0, gain_known: false, outcome: None }
    }

    /// Something that took `minutes` that day, rated as `rates`.
    pub fn count(&mut self, rates: &Rates, minutes: u32) {
        if minutes == 0 {
            return;
        }
        add(&mut self.load, &rates.load(minutes));
        for (s, known) in rates.known().iter().enumerate() {
            self.known[s] |= *known;
        }
        if let Some(gain) = rates.gain {
            self.gain_known = true;
            self.good_hours += gain / 10.0 * (minutes as f32 / 60.0).min(GAIN_HOURS_CAP);
        }
    }

    /// Whether anything at all is in it.
    pub fn is_empty(&self) -> bool {
        !self.known[TOTAL] && self.outcome.is_none()
    }
}

/// The days from `from` to `to`, each with what it held and how it went:
/// - tasks by the minutes worked on them that day (their sessions); a task
///   completed that day without any session at all, by its length as planned
///   (`corrected`, else its estimate, else `default_estimate`);
/// - time noted for a project alone, as a usual item;
/// - events by their length, their own ratings, else "usual";
/// - meals, naps and sleep never (they are needs, G4).
///
/// A task's ratings are those said after it that day when there are some,
/// else those the plan uses (`FeltIndex`). Days with nothing are left out.
#[allow(clippy::too_many_arguments)]
pub fn day_records(tasks: &[Task], sessions: &[Session], events: &[Occurrence], index: &FeltIndex, corrected: &dyn Fn(&Task) -> u32, outcomes: &BTreeMap<Date, Outcome>, from: Date, to: Date, zone: &TimeZone) -> Vec<DayRecord> {
    let day_of = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()).date());
    let by_uid: BTreeMap<&str, &Task> = tasks.iter().map(|t| (t.uid.as_str(), t)).collect();
    let mut days: BTreeMap<Date, DayRecord> = BTreeMap::new();
    fn record(days: &mut BTreeMap<Date, DayRecord>, date: Date, from: Date, to: Date) -> Option<&mut DayRecord> {
        (date >= from && date <= to).then(|| days.entry(date).or_insert_with(|| DayRecord::new(date)))
    }
    // The ratings of a task that day: said after it that day, else the plan's.
    let rates_on = |task: &Task, date: Date| -> Rates {
        let planned = index.rates(task);
        match task.felt.iter().find(|f| f.on == Some(date)) {
            Some(felt) => {
                let said = Rates::of(&felt.demands, planned.word);
                Rates { costs: std::array::from_fn(|s| said.costs[s].or(planned.costs[s])), gain: said.gain.or(planned.gain), word: planned.word }
            }
            None => planned,
        }
    };
    let mut worked: BTreeSet<&str> = BTreeSet::new();
    let mut minutes: BTreeMap<(Date, &str), u32> = BTreeMap::new();
    for s in sessions {
        let Some(date) = day_of(s.start) else { continue };
        if !s.task.is_empty() {
            worked.insert(s.task.as_str());
        }
        let key = (date, if s.task.is_empty() { "" } else { s.task.as_str() });
        *minutes.entry(key).or_default() += s.minutes;
    }
    for ((date, uid), m) in minutes {
        let Some(day) = record(&mut days, date, from, to) else { continue };
        match by_uid.get(uid) {
            Some(task) => day.count(&rates_on(task, date), m),
            None => day.count(&Rates::default(), m),
        }
    }
    for task in tasks.iter().filter(|t| t.status == Status::Completed && !worked.contains(t.uid.as_str())) {
        let Some(date) = task.completed.and_then(day_of) else { continue };
        let Some(day) = record(&mut days, date, from, to) else { continue };
        day.count(&rates_on(task, date), corrected(task));
    }
    for event in events.iter().filter(|e| !e.cancelled && !e.all_day && e.end > e.start) {
        let Some(date) = day_of(event.start) else { continue };
        let Some(day) = record(&mut days, date, from, to) else { continue };
        let length = u32::try_from((event.end - event.start) / 60).unwrap_or(0).min(24 * 60);
        day.count(&Rates::of_event(event), length);
    }
    for (&date, &outcome) in outcomes.range(from..=to) {
        if let Some(day) = record(&mut days, date, from, to) {
            day.outcome = Some(outcome);
        }
    }
    days.into_values().filter(|d| !d.is_empty()).collect()
}

/* ---------- Budgets: learned from the days whose outcome you gave ---------- */

/// Where the budgets start, before your days say more (criterion 24).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Start {
    /// As the plan was: your hours full of usual work, and two heavy hours.
    #[default]
    AsNow,
    /// About three quarters of that.
    Lighter,
    /// About half: the advice for energy-limiting illness (start at half, #MEAction).
    MuchLighter,
}

impl Start {
    pub fn share(self) -> f32 {
        match self {
            Start::AsNow => 1.0,
            Start::Lighter => 0.75,
            Start::MuchLighter => 0.5,
        }
    }

    pub fn parse(text: &str) -> Start {
        match text {
            "lighter" => Start::Lighter,
            "much-lighter" => Start::MuchLighter,
            _ => Start::AsNow,
        }
    }
}

/// The budgets to start from: a day of `room` minutes (the fullest day of
/// your week) full of usual work (4 an hour), and two heavy hours (7 instead
/// of 4), fitting under the 85 % the plan fills, times the start chosen. Each
/// cost starts at the total: a cost of an item never passes its highest, so
/// a cost's budget binds only once learning has lowered it.
pub fn start_budget(room: u32, start: Start) -> Load {
    let hours = room as f32 / 60.0;
    let total = (Level::Usual.rating() * hours + 2.0 * (Level::Heavy.rating() - Level::Usual.rating())) / FILL * start.share();
    [total.max(1.0); 5]
}

/// Days for a day's weight to halve in the budgets' mean (criterion 17).
pub const BUDGET_HALF_LIFE: f32 = 5.0;
/// A good day informs the budget from this share of it used (criterion 15a). A guess.
pub const INFORMATIVE: f32 = 0.7;
/// A day this full, followed by a bad one, lowers the budget (criterion 20). A guess.
pub const FULL: f32 = 0.8;
/// By how much a budget falls at once (criterion 20). A guess.
pub const DROP: f32 = 0.8;
/// By how much a budget may rise in a week (criterion 19). A guess.
pub const RISE: f32 = 1.1;
/// Days looked back to blame a bad day on a full one (criterion 20: 1–3). A guess.
pub const BLAME_DAYS: i64 = 2;
/// Stable days asked before a rise, and between two changes (criterion 19).
pub const STABLE_DAYS: usize = 7;
/// Bad days within seven that freeze rises (criterion 21).
pub const FLARE: usize = 3;
/// Days with an outcome before what is learned has full weight (criterion 24).
pub const FULL_WEIGHT_DAYS: f32 = 14.0;
/// A budget never falls below this share of its start. A guess.
pub const FLOOR: f32 = 0.25;
/// One day's load counts at most this much of the budget (criterion 23).
pub const WINSOR: f32 = 1.2;
/// Days looked back for the heaviest day well tolerated (criterion 22).
pub const CAP_DAYS: i64 = 30;
/// A cost needs this many fine days with load on it before its cap applies. A guess.
pub const CAP_FROM: usize = 3;
/// The gain minimum, in good hours: before data, and its bounds (G9–G11).
pub const GAIN_DEFAULT: f32 = 1.0;
pub const GAIN_LOWEST: f32 = 0.5;
pub const GAIN_HIGHEST: f32 = 4.0;
/// Good days asked before the minimum is learned (G10).
pub const GAIN_FROM: usize = 5;
/// The minimum is this percentile of good days' good hours (G9). A guess.
pub const GAIN_PERCENTILE: f32 = 0.25;
/// The learning window, in days: its default and bounds (the person's setting).
pub const WINDOW_DEFAULT: u32 = 28;
pub const WINDOW_LEAST: u32 = 14;
pub const WINDOW_MOST: u32 = 90;

/// A change a rule made to a budget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Change {
    pub date: Date,
    pub scale: usize,
    /// The budget after it over the budget before.
    pub factor: f32,
}

/// What the record says, as of a day.
#[derive(Debug, Clone, PartialEq)]
pub struct Learned {
    pub budget: Load,
    pub start: Load,
    /// The heaviest well-tolerated day of the last 30, plus 10 %, per cost; None until known.
    pub cap: [Option<f32>; 5],
    /// Rises held back after a flare.
    pub frozen: bool,
    /// The gain minimum, in good hours.
    pub minimum: f32,
    /// Days with an outcome within the window.
    pub days: usize,
    pub changes: Vec<Change>,
}

fn days_between(a: Date, b: Date) -> i64 {
    a.until(b).map_or(0, |s| i64::from(s.get_days()))
}

/// The budgets and the gain minimum as of `today`, replayed day by day over
/// `records` (oldest first; days before `today` only), from `start`, with
/// `window` days of memory. The rules, in order, on each day with an outcome:
/// a drop when it was bad after a full day; a freeze after a flare; the
/// target from informative good days; a rise after a stable week
/// (docs/capacity.md, "Budgets").
pub fn learn(records: &[DayRecord], start: Load, window: u32, today: Date) -> Learned {
    let window = i64::from(window.clamp(WINDOW_LEAST, WINDOW_MOST));
    let records: Vec<&DayRecord> = records.iter().filter(|r| r.date < today).collect();
    let at: BTreeMap<Date, usize> = records.iter().enumerate().map(|(i, r)| (r.date, i)).collect();
    let floor = start.map(|b| b * FLOOR);
    let mut budget = start;
    let mut in_force: Vec<Load> = Vec::with_capacity(records.len());
    let mut last_change: [Option<Date>; 5] = [None; 5];
    let mut blamed: BTreeSet<(Date, usize)> = BTreeSet::new();
    let mut frozen = false;
    let mut minimum = GAIN_DEFAULT;
    let mut changes: Vec<Change> = Vec::new();
    let mut with_outcome: Vec<usize> = Vec::new();
    let used = |r: &DayRecord, b: &Load, s: usize| if b[s] > 0.0 { r.load[s] / b[s] } else { 0.0 };
    for (i, record) in records.iter().enumerate() {
        in_force.push(budget);
        let Some(outcome) = record.outcome else { continue };
        with_outcome.push(i);
        let d = record.date;
        // 1. A bad day after a full one: that cost falls by a fifth, once per full day.
        if outcome.bad() {
            for s in 0..5 {
                for back in 1..=BLAME_DAYS {
                    let Some(&j) = d.checked_sub(jiff::Span::new().days(back)).ok().and_then(|p| at.get(&p)) else { continue };
                    let prior = records[j];
                    if prior.known[s] && used(prior, &in_force[j], s) >= FULL && blamed.insert((prior.date, s)) {
                        let before = budget[s];
                        budget[s] = (budget[s] * DROP).max(floor[s]);
                        last_change[s] = Some(d);
                        changes.push(Change { date: d, scale: s, factor: budget[s] / before });
                        break;
                    }
                }
            }
        }
        // 2. A flare (three bad days in seven) freezes rises, until seven days with an outcome and none bad.
        let recent_bad = with_outcome.iter().filter(|&&j| days_between(records[j].date, d) < 7 && records[j].outcome.is_some_and(Outcome::bad)).count();
        let last_week: Vec<usize> = with_outcome.iter().rev().take(STABLE_DAYS).copied().collect();
        if recent_bad >= FLARE {
            frozen = true;
        } else if frozen && last_week.len() == STABLE_DAYS && last_week.iter().all(|&j| records[j].outcome.is_some_and(Outcome::fine)) {
            frozen = false;
        }
        // 3–5. Rises: toward what good, full days showed, a tenth a week at most, after a stable week.
        let in_window = |j: usize| days_between(records[j].date, d) < window;
        let outcome_days = with_outcome.iter().filter(|&&j| in_window(j)).count() as f32;
        let alpha = (outcome_days / FULL_WEIGHT_DAYS).min(1.0);
        for s in 0..5 {
            if frozen || last_change[s].is_some_and(|c| days_between(c, d) < STABLE_DAYS as i64) {
                continue;
            }
            let stable = last_week.len() == STABLE_DAYS
                && last_week.iter().all(|&j| in_window(j) && records[j].outcome.is_some_and(Outcome::fine))
                && last_week.iter().map(|&j| used(records[j], &in_force[j], s)).sum::<f32>() / STABLE_DAYS as f32 >= FULL;
            if !stable {
                continue;
            }
            let (mut sum, mut weights) = (0.0, 0.0);
            for &j in with_outcome.iter().filter(|&&j| in_window(j)) {
                let r = records[j];
                if r.known[s] && r.outcome.is_some_and(Outcome::fine) && used(r, &in_force[j], s) >= INFORMATIVE {
                    let w = 0.5f32.powf(days_between(r.date, d) as f32 / BUDGET_HALF_LIFE);
                    sum += w * r.load[s].min(WINSOR * in_force[j][s]);
                    weights += w;
                }
            }
            if weights <= 0.0 {
                continue;
            }
            let target = (1.0 - alpha) * start[s] + alpha * sum / weights;
            if target > budget[s] {
                let before = budget[s];
                budget[s] = target.min(before * RISE);
                last_change[s] = Some(d);
                changes.push(Change { date: d, scale: s, factor: budget[s] / before });
            }
        }
        // The gain minimum: the 25th percentile of good days, never learned downward from a bad stretch.
        let good: Vec<(f32, f32)> = with_outcome
            .iter()
            .filter(|&&j| in_window(j))
            .map(|&j| records[j])
            .filter(|r| r.outcome.is_some_and(Outcome::good) && r.gain_known)
            .map(|r| (r.good_hours, 0.5f32.powf(days_between(r.date, d) as f32 / BUDGET_HALF_LIFE)))
            .collect();
        let candidate = if good.len() < GAIN_FROM { GAIN_DEFAULT } else { weighted_quantile(good, GAIN_PERCENTILE).unwrap_or(GAIN_DEFAULT).clamp(GAIN_LOWEST, GAIN_HIGHEST) };
        let bad_stretch = last_week.iter().filter(|&&j| !records[j].outcome.is_some_and(Outcome::good)).count() >= FLARE;
        if !(candidate < minimum && bad_stretch) {
            minimum = candidate;
        }
    }
    // The cap: the heaviest fine day of the last thirty, plus a tenth, once three such days are known.
    let cap = std::array::from_fn(|s| {
        let fine: Vec<f32> = records.iter().filter(|r| days_between(r.date, today) <= CAP_DAYS && r.known[s] && r.load[s] > 0.0 && r.outcome.is_some_and(Outcome::fine)).map(|r| r.load[s]).collect();
        (fine.len() >= CAP_FROM).then(|| fine.iter().copied().fold(0.0, f32::max) * RISE)
    });
    let days = with_outcome.iter().filter(|&&j| days_between(records[j].date, today) <= window).count();
    Learned { budget, start, cap, frozen, minimum, days, changes }
}

impl Learned {
    /// A day's limit per cost before the fill: the budget, under its cap.
    pub fn limit(&self) -> Load {
        std::array::from_fn(|s| self.cap[s].map_or(self.budget[s], |cap| self.budget[s].min(cap)))
    }
}

/* ---------- What the plan is given ---------- */

/// What the plan needs from the record (`plan::Settings::capacity`). Its
/// default changes nothing: no budget, no correction, no slack, no gain slot.
#[derive(Debug, Clone, Default)]
pub struct Planning {
    /// The ratings each task is planned with, by UID (`FeltIndex::rates`).
    pub rates: BTreeMap<String, Rates>,
    /// Each task's length as the plan lays it, by UID (`Ratios::corrected`).
    pub corrected: BTreeMap<String, u32>,
    /// What events already hold each day.
    pub held: BTreeMap<Date, Load>,
    /// Days before and after a heavy event: lighter.
    pub lightened: BTreeSet<Date>,
    /// Each day's limit per cost before the fill, the cap applied; None: no budget.
    pub limit: Option<Load>,
    /// Days kept even (criterion 35).
    pub even: bool,
    /// The spread for the day's free time; None: none kept.
    pub spread: Option<Spread>,
    /// Today's effect so far (log), and its spread (`Ratios::today`, `today_sigma`).
    pub today_effect: f32,
    pub today_sigma: f32,
    /// Minutes of each day's room kept for the gain slot after its costliest block.
    pub gain_slot: u32,
}

impl Planning {
    /// The ratings a task is planned with.
    pub fn rates_of(&self, task: &Task) -> Rates {
        self.rates.get(&task.uid).copied().unwrap_or_else(|| Rates::of_task(task))
    }

    pub fn level_of(&self, task: &Task) -> Level {
        self.rates.get(&task.uid).map_or_else(|| level_of(task), Rates::level)
    }

    /// A day's limit per cost: the budget's fill, its weather (`share`, today's), lighter around a heavy event.
    pub fn day_limit(&self, date: Date, share: f32) -> Option<Load> {
        let around = if self.lightened.contains(&date) { AROUND_HEAVY } else { 1.0 };
        self.limit.map(|l| l.map(|b| b * FILL * share * around))
    }
}

/// The days before and after each heavy event (its highest cost 7 or more), from `today` on.
pub fn around_heavy(events: &[Occurrence], zone: &TimeZone, today: Date) -> BTreeSet<Date> {
    let mut out = BTreeSet::new();
    for event in events.iter().filter(|e| !e.cancelled && e.demands.highest().is_some_and(|h| h >= HEAVY_EVENT)) {
        let Some(first) = jiff::Timestamp::from_second(event.start).ok().map(|t| t.to_zoned(zone.clone()).date()) else { continue };
        let last = jiff::Timestamp::from_second((event.end - 1).max(event.start)).ok().map_or(first, |t| t.to_zoned(zone.clone()).date());
        for date in [first.yesterday().ok(), last.tomorrow().ok()].into_iter().flatten() {
            if date >= today {
                out.insert(date);
            }
        }
    }
    out
}

/// What each day's events hold, by day.
pub fn held_by_events(events: &[Occurrence], zone: &TimeZone) -> BTreeMap<Date, Load> {
    let mut out: BTreeMap<Date, Load> = BTreeMap::new();
    for event in events.iter().filter(|e| !e.cancelled && !e.all_day && e.end > e.start) {
        let Some(date) = jiff::Timestamp::from_second(event.start).ok().map(|t| t.to_zoned(zone.clone()).date()) else { continue };
        let minutes = u32::try_from((event.end - event.start) / 60).unwrap_or(0).min(24 * 60);
        add(out.entry(date).or_insert([0.0; 5]), &Rates::of_event(event).load(minutes));
    }
    out
}

/* ---------- Gathered from the record ---------- */

/// How far back the record is read to replay the budgets.
pub const HISTORY_DAYS: i64 = 180;
/// How far ahead events weigh on the plan's days (as the plan's room does).
pub const AHEAD_DAYS: i64 = 28;
/// Minutes kept each day for the gain slot after its costliest block (G11). A guess.
pub const GAIN_SLOT_MINUTES: u32 = 30;

/// The days' outcomes, as the reviews say them: the night's mix, else the work review's.
pub fn outcomes_of(reviews: &crate::reviews::Reviews) -> BTreeMap<Date, Outcome> {
    use crate::reviews::Mix;
    reviews
        .days
        .iter()
        .filter_map(|(date, day)| {
            let outcome = match day.outcome()? {
                Mix::TooMuch => Outcome::TooMuch,
                Mix::AboutRight => Outcome::AboutRight,
                Mix::TooEmpty => Outcome::TooEmpty,
            };
            Some((*date, outcome))
        })
        .collect()
}

/// What a task is for, as the ratio sorts tasks: its area, three kinds, always
/// known (TE10: few kinds; docs/capacity.md, "Corrected estimates").
pub fn ratio_kind(areas: &crate::areas::TaskAreas, task: &Task) -> &'static str {
    let area = areas.of(task);
    if area.work {
        "work"
    } else if area.admin {
        "admin"
    } else {
        "leisure"
    }
}

/// Everything the plan and the days' balance need from your record.
#[derive(Debug, Clone)]
pub struct Record {
    pub index: FeltIndex,
    pub ratios: Ratios,
    pub learned: Learned,
    /// The days before today, with what they held and how they went.
    pub records: Vec<DayRecord>,
    pub outcomes: BTreeMap<Date, Outcome>,
    /// What the plan is given.
    pub planning: Planning,
    /// A heavy event tomorrow (true) or yesterday (false): its title, for the reason in words.
    pub heavy_near: Option<(String, bool)>,
    /// The learning window, in days.
    pub window: u32,
    /// Minutes counted for a task without an estimate.
    pub default_estimate: u32,
    /// Each task's kind for the ratio (its area), by UID.
    pub kinds: BTreeMap<String, &'static str>,
}

fn midnight_of(date: Date, zone: &TimeZone) -> i64 {
    date.to_zoned(zone.clone()).map_or(0, |z| z.timestamp().as_second())
}

/// The record read and weighed as of `now`: the ratings said after tasks, the
/// ratio of time spent to first guesses, the days before (their tasks,
/// sessions, events and outcomes) with the budgets replayed over them, the
/// events of the coming weeks; and what the plan is given (`Planning`).
/// Events of the past are read only once an outcome was given; `events`
/// reads those between two instants (`agenda::occurrences`, or a cache of it).
pub fn gather(tasks: &[Task], sessions: &[Session], settings: &crate::plan::Settings, choices: &crate::config::PlanningSettings, events: &dyn Fn(i64, i64) -> Vec<Occurrence>, now: &jiff::Zoned) -> Record {
    let zone = now.time_zone().clone();
    let today = now.date();
    let index = FeltIndex::of(tasks);
    let kind = |t: &Task| ratio_kind(&settings.areas, t);
    let ratios = ratios(&finished(tasks, sessions, &kind, &zone), today);
    let length = |t: &Task| ratios.corrected(t.estimate, kind(t)).unwrap_or(if t.estimate > 0 { t.estimate } else { settings.default_estimate });
    let from = today.checked_sub(jiff::Span::new().days(HISTORY_DAYS)).unwrap_or(today);
    let outcomes = outcomes_of(&crate::reviews::Reviews::load_between(&crate::reviews::Reviews::default_path(), from, today));
    let records = match outcomes.keys().next() {
        Some(first) => {
            let start = first.checked_sub(jiff::Span::new().days(BLAME_DAYS)).unwrap_or(*first).max(from);
            let past = events(midnight_of(start, &zone), midnight_of(today, &zone));
            day_records(tasks, sessions, &past, &index, &length, &outcomes, start, today.yesterday().unwrap_or(today), &zone)
        }
        None => Vec::new(),
    };
    let room = settings.week.iter().map(crate::plan::Room::total).max().unwrap_or(0);
    let learned = learn(&records, start_budget(room, choices.start()), choices.window(), today);
    // From yesterday: a heavy event then lightens today.
    let midnight = midnight_of(today, &zone);
    let ahead = events(midnight - 86_400, midnight + AHEAD_DAYS * 86_400);
    let lightened = around_heavy(&ahead, &zone, today);
    let heavy = |e: &&Occurrence| !e.cancelled && e.demands.highest().is_some_and(|h| h >= HEAVY_EVENT);
    let day_of = |seconds: i64| jiff::Timestamp::from_second(seconds).ok().map(|t| t.to_zoned(zone.clone()).date());
    let heavy_near = ahead
        .iter()
        .filter(heavy)
        .find_map(|e| (day_of(e.start) == today.tomorrow().ok()).then(|| (e.summary.clone(), true)))
        .or_else(|| ahead.iter().filter(heavy).find_map(|e| (day_of((e.end - 1).max(e.start)) == today.yesterday().ok()).then(|| (e.summary.clone(), false))));
    let open: Vec<&Task> = tasks.iter().filter(|t| t.status.is_open()).collect();
    let planning = Planning {
        rates: open.iter().map(|t| (t.uid.clone(), index.rates(t))).collect(),
        corrected: open.iter().filter_map(|t| Some((t.uid.clone(), ratios.corrected(t.estimate, kind(t))?))).collect(),
        held: held_by_events(&ahead, &zone),
        lightened,
        limit: Some(learned.limit()),
        even: choices.even_days,
        spread: Some(Spread::of(ratios.sigma)),
        today_effect: ratios.today,
        today_sigma: ratios.today_sigma,
        gain_slot: if choices.gain_slots() { GAIN_SLOT_MINUTES } else { 0 },
    };
    Record { index, ratios, learned, records, outcomes, planning, heavy_near, window: choices.window(), default_estimate: settings.default_estimate, kinds: tasks.iter().map(|t| (t.uid.clone(), kind(t))).collect() }
}

impl Record {
    /// A task's length as the plan counts it: its corrected estimate, else its estimate, else the default.
    fn length(&self, task: &Task) -> u32 {
        let kind = self.kinds.get(&task.uid).copied().unwrap_or("leisure");
        self.ratios.corrected(task.estimate, kind).unwrap_or(if task.estimate > 0 { task.estimate } else { self.default_estimate })
    }

    /// The line the task panel shows on request: how tasks like this one usually go (`Ratios::line`).
    pub fn ratio_line(&self, task: &Task, tr: &Translator) -> String {
        self.ratios.line(self.kinds.get(&task.uid).copied().unwrap_or("leisure"), tr)
    }

    /// A day's balance in words (for the evening's review): what it held
    /// against what a day holds, as learned before it, its gain against the
    /// minimum, and what learning from it changes for the next day. `events`
    /// are that day's.
    pub fn day_balance(&self, tasks: &[Task], sessions: &[Session], events: &[Occurrence], date: Date, zone: &TimeZone, tr: &Translator) -> DayBalance {
        let day = day_records(tasks, sessions, events, &self.index, &|t| self.length(t), &self.outcomes, date, date, zone).pop().unwrap_or_else(|| DayRecord { outcome: self.outcomes.get(&date).copied(), ..DayRecord::new(date) });
        let before: Vec<DayRecord> = self.records.iter().filter(|r| r.date < date).cloned().collect();
        let learned = learn(&before, self.learned.start, self.window, date);
        let mut with: Vec<DayRecord> = before;
        with.push(day.clone());
        let after = date.tomorrow().ok().map(|next| learn(&with, self.learned.start, self.window, next));
        balance(&day, &learned, after.as_ref().filter(|_| day.outcome.is_some()), tr)
    }

    /// Why today holds what it holds, in at most two lines (`reasons`).
    pub fn reasons(&self, today: Date, tr: &Translator) -> Vec<String> {
        reasons(&self.learned, today, &self.planning.lightened, self.heavy_near.as_ref().map(|(t, before)| (t.as_str(), *before)), tr)
    }
}

/* ---------- In words ---------- */

/// A cost of a day, in a word.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScaleBalance {
    /// "cognitive", "emotional", "anxiety", "body", "total".
    pub scale: &'static str,
    /// "light", "usual", "heavy"; "" when nothing is known of it that day.
    pub level: &'static str,
    /// Not to be shown: for tests and for the record.
    pub load: f32,
    pub budget: f32,
}

/// What gave back that day, in a word.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GainBalance {
    /// "below", "around", "above" the minimum; "" when nothing carried a gain.
    pub level: &'static str,
    /// Not to be shown.
    pub good_hours: f32,
    pub minimum: f32,
}

/// A day's balance, in words: each cost and the total against what the day
/// holds, the gain against its minimum, and what the plan does with it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DayBalance {
    pub date: String,
    pub costs: Vec<ScaleBalance>,
    pub gain: GainBalance,
    /// At most two lines, with their reason; nothing to say, none.
    pub said: Vec<String>,
}

/// A load this share of the budget or less is a light day; above the budget, a heavy one. Guesses.
pub const LIGHT_DAY: f32 = 0.5;
pub const HEAVY_DAY: f32 = 1.0;

/// The balance of `record`'s day against `learned` (as of that day), and what
/// learning from it would change (`after`: as of the next day; None when not known).
pub fn balance(record: &DayRecord, learned: &Learned, after: Option<&Learned>, tr: &Translator) -> DayBalance {
    let limit = learned.limit();
    let costs = (0..5)
        .map(|s| {
            let used = if limit[s] > 0.0 { record.load[s] / limit[s] } else { 0.0 };
            let level = if !record.known[s] {
                ""
            } else if used > HEAVY_DAY {
                "heavy"
            } else if used <= LIGHT_DAY && !record.outcome.is_some_and(Outcome::bad) {
                "light"
            } else {
                "usual"
            };
            ScaleBalance { scale: SCALES[s], level, load: record.load[s], budget: limit[s] }
        })
        .collect();
    let minimum = learned.minimum;
    let gain_level = if !record.gain_known {
        ""
    } else if record.good_hours < 0.75 * minimum {
        "below"
    } else if record.good_hours > 1.25 * minimum {
        "above"
    } else {
        "around"
    };
    let mut said = Vec::new();
    if let Some(after) = after {
        if (0..5).any(|s| after.budget[s] < learned.budget[s] * 0.99) {
            said.push(tr.text("capacity-said-tomorrow-less", None));
        }
        if after.frozen && !learned.frozen {
            said.push(tr.text("capacity-said-held", None));
        }
    }
    said.truncate(2);
    DayBalance { date: record.date.to_string(), costs, gain: GainBalance { level: gain_level, good_hours: record.good_hours, minimum }, said }
}

/// Why today holds what it holds, in at most two lines: a budget lowered
/// yesterday after a bad day that followed a full one; a heavy event the day
/// before or after; a change against two weeks ago once it passes 30 %
/// (criterion 48). Nothing when nothing changed the day.
pub fn reasons(learned: &Learned, today: Date, lightened: &BTreeSet<Date>, heavy_event: Option<(&str, bool)>, tr: &Translator) -> Vec<String> {
    let mut out = Vec::new();
    let yesterday = today.yesterday().unwrap_or(today);
    if learned.changes.iter().any(|c| c.date == yesterday && c.factor < 1.0) {
        out.push(tr.text("capacity-reason-after-bad", None));
    }
    if lightened.contains(&today)
        && let Some((title, before)) = heavy_event
    {
        let mut args = FluentArgs::new();
        args.set("title", title.to_string());
        out.push(tr.text(if before { "capacity-reason-before-heavy" } else { "capacity-reason-after-heavy" }, Some(&args)));
    }
    // Against two weeks ago, the total only, past noise.
    let fortnight = today.checked_sub(jiff::Span::new().days(14)).unwrap_or(today);
    let product: f32 = learned.changes.iter().filter(|c| c.scale == TOTAL && c.date > fortnight).map(|c| c.factor).product();
    if product >= 1.3 {
        out.push(tr.text("capacity-reason-more", None));
    } else if product <= 1.0 / 1.3 && out.is_empty() {
        out.push(tr.text("capacity-reason-less", None));
    }
    out.truncate(2);
    out
}

/* ---------- The gain: suggestions from your own items ---------- */

/// An item you rated as giving back well (felt gain 6 or more, no felt cost
/// above 3), rotated by day: what a gain slot may suggest (G19). Never one
/// you did not rate yourself, never a social one by default (G21).
pub fn suggestion(tasks: &[Task], index: &FeltIndex, today: Date) -> Option<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut liked: Vec<&Task> = tasks
        .iter()
        .filter(|t| !t.felt.is_empty())
        .filter(|t| {
            let item = index.items.get(&item_key(t)).map(Vec::as_slice).unwrap_or(&[]);
            let gain = FeltIndex::median(item, |d| d.gain);
            let costs = [FeltIndex::median(item, |d| d.cognitive), FeltIndex::median(item, |d| d.emotional), FeltIndex::median(item, |d| d.anxiety), FeltIndex::median(item, |d| d.body)];
            gain.is_some_and(|g| g >= 6.0) && costs.iter().flatten().all(|c| *c <= 3.0)
        })
        .filter(|t| seen.insert(item_key(t)))
        .collect();
    if liked.is_empty() {
        return None;
    }
    liked.sort_by_key(|t| item_key(t));
    let day = Date::constant(2000, 1, 1).until(today).map_or(0, |s| s.get_days().unsigned_abs() as usize);
    Some(liked[day % liked.len()].title.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demands::Felt;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    fn rated(c: Option<u8>, e: Option<u8>, a: Option<u8>, b: Option<u8>, g: Option<u8>) -> Demands {
        Demands { cognitive: c, emotional: e, anxiety: a, body: b, gain: g }
    }

    #[test]
    fn loads_rating_by_hours_half_an_hour_at_least() {
        let call = Rates::of(&rated(None, None, Some(8), None, None), Level::Usual);
        // A ten-minute call counts half an hour: 8 × 0.5.
        assert_eq!(call.load(10), [0.0, 0.0, 4.0, 0.0, 4.0]);
        // Unrated: its word, in the total only (light 2, usual 4, heavy 7).
        assert_eq!(Rates::of(&Demands::default(), Level::Heavy).load(60), [0.0, 0.0, 0.0, 0.0, 7.0]);
        assert_eq!(Rates::of(&Demands::default(), Level::Usual).load(90), [0.0, 0.0, 0.0, 0.0, 6.0]);
        assert_eq!(Rates::of(&Demands::default(), Level::Rest).load(60)[TOTAL], 2.0, "what gives back is not free");
        // The total follows the highest cost, never their sum.
        let mixed = Rates::of(&rated(Some(5), Some(2), Some(3), Some(1), None), Level::Usual);
        assert_eq!(mixed.load(60), [5.0, 2.0, 3.0, 1.0, 5.0]);
        assert_eq!((mixed.dominant(), Rates::of(&rated(Some(3), Some(2), None, None, None), Level::Light).dominant()), (Some(0), None));
    }

    fn felt_task(uid: &str, title: &str, kind: &str, forecast: Demands, felt: &[(&str, Demands)]) -> Task {
        Task { uid: uid.into(), title: title.into(), kind: kind.into(), demands: forecast, felt: felt.iter().map(|(on, d)| Felt { on: Some(day(on)), demands: *d }).collect(), ..Task::default() }
    }

    #[test]
    fn felt_ratings_plan_over_forecasts() {
        let anxious = rated(None, None, Some(9), None, None);
        // The same call made again each week: felt 3, 4, 2, 5, 3, 8 (the last five: 4, 2, 5, 3, 8).
        let history: Vec<(&str, Demands)> = [("2026-09-01", 3), ("2026-09-08", 4), ("2026-09-15", 2), ("2026-09-22", 5), ("2026-09-29", 3), ("2026-10-06", 8)].iter().map(|(d, a)| (*d, rated(None, None, Some(*a), None, None))).collect();
        let old = felt_task("old", "Call the bank", "call", anxious, &history);
        let new = felt_task("new", " call the BANK ", "call", anxious, &[]);
        let other = felt_task("other", "Call the school", "call", rated(Some(2), None, None, None, None), &[]);
        let index = FeltIndex::of(&[old, new.clone(), other.clone()]);
        // The item's median of its last five: 4.
        assert_eq!(index.rates(&new).costs[2], Some(4.0));
        // Another call: its own forecast first; what it leaves unsaid, from calls as felt.
        let rates = index.rates(&other);
        assert_eq!((rates.costs[0], rates.costs[2]), (Some(2.0), Some(4.0)));
        // Nothing felt at all: the forecast.
        assert_eq!(FeltIndex::default().rates(&new).costs[2], Some(9.0));
        assert_eq!(index.rates(&new).level(), Level::Usual, "felt: usual, though forecast heavy");
    }

    fn done(uid: &str, estimate: u32, first: Option<u32>, on: &str, area: &str) -> Task {
        let completed = day(on).at(17, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp().as_second();
        Task { uid: uid.into(), title: uid.into(), estimate, estimate_first: first, status: Status::Completed, completed: Some(completed), area: area.into(), ..Task::default() }
    }

    fn session(uid: &str, on: &str, minutes: u32, kind: Option<Kind>) -> Session {
        Session { task: uid.into(), start: day(on).at(9, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap().timestamp().as_second(), minutes, kind, ..Session::default() }
    }

    fn area(task: &Task) -> &'static str {
        match task.area.as_str() {
            "work" => "work",
            "admin" => "admin",
            _ => "leisure",
        }
    }

    #[test]
    fn the_ratio_learns_from_finished_timed_tasks() {
        let today = day("2026-10-06");
        // Twelve admin tasks, each guessed 30 min (first), timed at 45: ratio 1.5.
        let mut tasks = Vec::new();
        let mut sessions = Vec::new();
        for i in 0..12 {
            let on = format!("2026-10-{:02}", 1 + i % 5);
            tasks.push(done(&format!("a{i}"), 60, Some(30), &on, "admin"));
            sessions.push(session(&format!("a{i}"), &on, 45, Some(Kind::Measured)));
        }
        let all = finished(&tasks, &sessions, &area, &TimeZone::UTC);
        assert_eq!(all.len(), 12);
        assert!((all[0].log - 1.5f32.ln()).abs() < 1e-5, "the first estimate, not the current one: {}", all[0].log);
        let r = ratios(&all, today);
        assert!((r.person.exp() - 1.5).abs() < 0.01, "stands alone past nine tasks: {}", r.person.exp());
        assert!((r.factor("admin") - 1.5).abs() < 0.02, "{}", r.factor("admin"));
        assert_eq!(r.corrected(40, "admin"), Some(60));
        assert_eq!(r.corrected(0, "admin"), None, "no estimate of its own: never invented");
        // A kind with no task: yours.
        assert!((r.factor("work") - r.person.exp()).abs() < 1e-6);
        // Three tasks only: a third of the way from 1.1× to theirs.
        let three = ratios(&all[..3], today);
        let expected = (1.0 / 3.0) * 1.5f32.ln() + (2.0 / 3.0) * 1.1f32.ln();
        assert!((three.person - expected).abs() < 1e-4, "{} vs {expected}", three.person);
        // A kind pulled toward yours: four work tasks at 2×, among admin at 1.5×.
        let mut more = tasks.clone();
        let mut more_sessions = sessions.clone();
        for i in 0..4 {
            more.push(done(&format!("w{i}"), 30, Some(30), "2026-10-05", "work"));
            more_sessions.push(session(&format!("w{i}"), "2026-10-05", 60, Some(Kind::Measured)));
        }
        let r = ratios(&finished(&more, &more_sessions, &area, &TimeZone::UTC), today);
        let work = r.factor("work");
        assert!(work < 2.0 && work > r.person.exp(), "work between yours and its own: {work}");
    }

    #[test]
    fn typed_time_counts_little_and_open_or_dropped_tasks_not_at_all() {
        let today = day("2026-10-06");
        let mut tasks = vec![done("typed", 30, Some(30), "2026-10-05", "admin"), done("timed", 30, Some(30), "2026-10-05", "admin")];
        let mut open = done("open", 30, Some(30), "2026-10-05", "admin");
        open.status = Status::NeedsAction;
        let mut dropped = done("dropped", 30, Some(30), "2026-10-05", "admin");
        dropped.status = Status::Cancelled;
        tasks.extend([open, dropped]);
        let sessions = vec![session("typed", "2026-10-05", 90, Some(Kind::Typed)), session("timed", "2026-10-05", 30, Some(Kind::Corrected)), session("open", "2026-10-05", 300, Some(Kind::Measured)), session("dropped", "2026-10-05", 300, Some(Kind::Measured)), session("typed", "2026-10-05", 0, None)];
        let all = finished(&tasks, &sessions, &area, &TimeZone::UTC);
        assert_eq!(all.iter().map(|f| f.uid.as_str()).collect::<Vec<_>>(), vec!["typed", "timed"], "only finished tasks");
        assert_eq!((all[0].weight, all[1].weight), (TYPED_WEIGHT, 1.0));
        let r = ratios(&all, today);
        // Weighted: the typed 3× counts a quarter as much as the timed 1×.
        let mean = (0.25 * 3f32.ln() + 1.0 * 0.0) / 1.25;
        let w = 1.25 / STANDS_FROM;
        assert!((r.person - (w * mean + (1.0 - w) * 1.1f32.ln())).abs() < 1e-4, "{}", r.person);
        // An old session without a kind counts as typed.
        let unknown = finished(&[done("u", 30, None, "2026-10-05", "admin")], &[session("u", "2026-10-05", 60, None)], &area, &TimeZone::UTC);
        assert_eq!(unknown[0].weight, TYPED_WEIGHT);
    }

    #[test]
    fn recent_tasks_weigh_more() {
        let today = day("2026-10-20");
        // Twenty tasks a month ago at 2×, ten yesterday at 1×.
        let mut tasks = Vec::new();
        let mut sessions = Vec::new();
        for i in 0..20 {
            tasks.push(done(&format!("o{i}"), 30, Some(30), "2026-09-20", "admin"));
            sessions.push(session(&format!("o{i}"), "2026-09-20", 60, Some(Kind::Measured)));
        }
        for i in 0..10 {
            tasks.push(done(&format!("n{i}"), 30, Some(30), "2026-10-19", "admin"));
            sessions.push(session(&format!("n{i}"), "2026-10-19", 30, Some(Kind::Measured)));
        }
        let r = ratios(&finished(&tasks, &sessions, &area, &TimeZone::UTC), today);
        assert!(r.person.exp() < 1.05, "a month old weighs almost nothing at a half-life of 4.5 days: {}", r.person.exp());
    }

    #[test]
    fn today_is_weighed_by_its_finished_tasks() {
        let today = day("2026-10-06");
        let mut tasks = Vec::new();
        let mut sessions = Vec::new();
        for i in 0..10 {
            tasks.push(done(&format!("p{i}"), 30, Some(30), "2026-10-02", "admin"));
            sessions.push(session(&format!("p{i}"), "2026-10-02", 30, Some(Kind::Measured)));
        }
        let calm = ratios(&finished(&tasks, &sessions, &area, &TimeZone::UTC), today);
        assert_eq!((calm.today, calm.today_factor()), (0.0, 1.0));
        // Two done today, each twice as long as usual: the rest of today runs longer, shrunk toward ordinary.
        for i in 0..2 {
            tasks.push(done(&format!("t{i}"), 30, Some(30), "2026-10-06", "admin"));
            sessions.push(session(&format!("t{i}"), "2026-10-06", 60, Some(Kind::Measured)));
        }
        let slow = ratios(&finished(&tasks, &sessions, &area, &TimeZone::UTC), today);
        assert!(slow.today > 0.0 && slow.today_factor() < 2.0 && slow.today_sigma < Spread::of(slow.sigma).day, "{slow:?}");
    }

    #[test]
    fn slack_at_p85_is_deterministic() {
        let spread = Spread::of(PRIOR_SIGMA);
        let medians = [30, 30, 30, 30, 30];
        let a = slack(&medians, spread, 0.0, seed(day("2026-10-06"), &["a", "b"]));
        let b = slack(&medians, spread, 0.0, seed(day("2026-10-06"), &["a", "b"]));
        assert_eq!(a, b, "the same day, the same steps: the same slack");
        // TE's own figures: five 30-min steps, σ 0.5, ρ 0.3: P80 ≈ 255 against 150 + medians' sum.
        assert!((30..=110).contains(&a), "{a}");
        // A wider day, more slack; nothing to do, none.
        assert!(slack(&medians, Spread::of(0.8), 0.0, 1) > slack(&medians, Spread::of(0.3), 0.0, 1));
        assert_eq!(slack(&[], spread, 0.0, 1), 0);
        // A day going slow keeps more.
        assert!(slack(&medians, spread, 0.3, 7) > slack(&medians, spread, 0.0, 7));
        assert_ne!(seed(day("2026-10-06"), &["a"]), seed(day("2026-10-07"), &["a"]));
    }

    #[test]
    fn the_estimate_follows_the_simulation() {
        // Fenton–Wilkinson against 2000 draws: within a tenth, or three minutes, on days of every shape.
        for (medians, sigma, effect) in [(vec![30u32, 30, 30, 30, 30], 0.5f32, 0.0f32), (vec![240, 17], 0.707, 0.0), (vec![15, 15, 15], 0.707, 0.0), (vec![90, 45, 20], 0.3, 0.2), (vec![180], 0.5, 0.0)] {
            let spread = Spread::of(sigma);
            let sum: f32 = medians.iter().map(|&m| m as f32).sum();
            let squares: f32 = medians.iter().map(|&m| (m * m) as f32).sum();
            let simulated = slack(&medians, spread, effect, 42) as f32;
            let estimated = slack_estimate(sum, squares, spread, effect);
            assert!((estimated - simulated).abs() <= (0.1 * simulated).max(3.0), "{medians:?} σ {sigma}: {estimated} against {simulated}");
        }
        assert_eq!(slack_estimate(0.0, 0.0, Spread::of(0.5), 0.0), 0.0);
    }

    /// A record of `n` days from `from`, each `load` (every cost and the total), with `outcome`.
    fn days(from: &str, n: i64, load: f32, outcome: Option<Outcome>) -> Vec<DayRecord> {
        (0..n)
            .map(|i| {
                let date = day(from).checked_add(jiff::Span::new().days(i)).unwrap();
                DayRecord { date, load: [load; 5], known: [true; 5], good_hours: 1.0, gain_known: true, outcome }
            })
            .collect()
    }

    fn after(records: &[DayRecord]) -> Date {
        records.last().unwrap().date.tomorrow().unwrap()
    }

    const START: Load = [10.0; 5];

    #[test]
    fn rises_only_after_seven_stable_full_days() {
        // Six good days at 90 %: no rise yet.
        let six = days("2026-09-01", 6, 9.0, Some(Outcome::AboutRight));
        assert_eq!(learn(&six, START, 28, after(&six)).budget[0], 10.0);
        // Seven: a rise, toward what they showed, a tenth at most; the start still weighs (α = 7/14).
        let seven = days("2026-09-01", 7, 12.0, Some(Outcome::AboutRight));
        let learned = learn(&seven, START, 28, after(&seven));
        assert!((learned.budget[0] - 11.0).abs() < 1e-4, "{:?}", learned.budget);
        // Days at 75 % on average: informative, but not stable at 80 %: none.
        let mild = days("2026-09-01", 10, 7.5, Some(Outcome::AboutRight));
        assert_eq!(learn(&mild, START, 28, after(&mild)).budget[0], 10.0);
        // Light good days say only "at least this much": never a fall.
        let light = days("2026-09-01", 20, 3.0, Some(Outcome::AboutRight));
        assert_eq!(learn(&light, START, 28, after(&light)).budget, START);
        // A week after the rise, another tenth at most: never on a timetable, always after stable days.
        let fourteen = days("2026-09-01", 14, 14.0, Some(Outcome::AboutRight));
        let learned = learn(&fourteen, START, 28, after(&fourteen));
        assert!((learned.budget[0] - 12.1).abs() < 1e-3, "two rises in two weeks: {:?}", learned.budget);
    }

    #[test]
    fn falls_a_fifth_after_a_bad_day_following_a_full_one() {
        let mut record = days("2026-09-01", 3, 5.0, None);
        record[1].load = [9.0; 5]; // 90 %
        record[2].outcome = Some(Outcome::TooMuch);
        let learned = learn(&record, START, 28, after(&record));
        assert!((learned.budget[0] - 8.0).abs() < 1e-5, "{:?}", learned.budget);
        assert_eq!(learned.changes.len(), 5);
        // A bad day after a light day: nothing learned downward.
        let mut light = days("2026-09-01", 3, 5.0, None);
        light[2].outcome = Some(Outcome::TooMuch);
        assert_eq!(learn(&light, START, 28, after(&light)).budget, START);
        // Two bad days after one full day: blamed once.
        let mut twice = days("2026-09-01", 4, 5.0, None);
        twice[1].load = [9.0; 5];
        twice[2].outcome = Some(Outcome::TooMuch);
        twice[3].outcome = Some(Outcome::TooMuch);
        assert!((learn(&twice, START, 28, after(&twice)).budget[0] - 8.0).abs() < 1e-5);
        // Only the cost that was full falls.
        let mut one = days("2026-09-01", 3, 5.0, None);
        one[1].load = [2.0, 9.0, 2.0, 2.0, 9.0];
        one[2].outcome = Some(Outcome::TooMuch);
        let learned = learn(&one, START, 28, after(&one));
        assert_eq!((learned.budget[0], learned.budget[1]), (10.0, 8.0));
        // Never below a quarter of the start.
        let mut crash = Vec::new();
        for k in 0..20 {
            let mut pair = days(&day("2026-09-01").checked_add(jiff::Span::new().days(2 * k)).unwrap().to_string(), 2, 100.0, None);
            pair[1].outcome = Some(Outcome::TooMuch);
            crash.extend(pair);
        }
        assert!((learn(&crash, START, 90, after(&crash)).budget[0] - 2.5).abs() < 1e-5);
    }

    #[test]
    fn freezes_after_three_bad_days_in_seven() {
        // A full week, then a flare: three bad days; then good full days.
        let mut record = days("2026-09-01", 7, 9.0, Some(Outcome::AboutRight));
        for date in ["2026-09-08", "2026-09-10", "2026-09-12"] {
            record.push(DayRecord { outcome: Some(Outcome::TooMuch), ..days(date, 1, 2.0, None).remove(0) });
        }
        record.extend(days("2026-09-13", 6, 12.0, Some(Outcome::AboutRight)));
        let learned = learn(&record, START, 28, after(&record));
        assert!(learned.frozen, "six good days after a flare: still held");
        let before = learned.budget[0];
        // A seventh good day: the freeze lifts, rises may come back after a stable week.
        record.extend(days("2026-09-19", 1, 12.0, Some(Outcome::AboutRight)));
        let learned = learn(&record, START, 28, after(&record));
        assert!(!learned.frozen);
        assert!(learned.budget[0] >= before);
    }

    #[test]
    fn the_cap_and_the_window() {
        // Three fine days in the last thirty, the heaviest at 6: a day planned at 6.6 at most.
        let mut record = days("2026-09-20", 3, 4.0, Some(Outcome::AboutRight));
        record[1].load = [6.0; 5];
        let learned = learn(&record, START, 28, day("2026-09-25"));
        assert!((learned.cap[0].unwrap() - 6.6).abs() < 1e-4);
        assert!((learned.limit()[0] - 6.6).abs() < 1e-4, "the budget under its cap");
        // Two only: not known yet.
        assert_eq!(learn(&record[..2], START, 28, day("2026-09-25")).cap[0], None);
        // Forty days later, out of the last thirty: gone.
        assert_eq!(learn(&record, START, 28, day("2026-11-05")).cap[0], None);
        // The window: a rise needs its stable week inside it; days with no outcome weigh nothing.
        let mut sparse = days("2026-09-01", 7, 12.0, Some(Outcome::AboutRight));
        sparse.extend(days("2026-09-08", 20, 2.0, None));
        let learned = learn(&sparse, START, 28, after(&sparse));
        assert_eq!((learned.days, learned.budget[0] > 10.0), (7, true), "missing days count for nothing");
        let short = learn(&sparse, START, 14, day("2026-10-20"));
        assert_eq!(short.days, 0, "out of a 14-day window");
    }

    #[test]
    fn learned_weight_grows_to_fourteen_days() {
        // Ten days at 150 %: the target is pulled toward the start (α = 10/14).
        let record = days("2026-09-01", 10, 15.0, Some(Outcome::AboutRight));
        let learned = learn(&record, START, 28, after(&record));
        // Rise on day 7 (α 0.5 → target 12.5, a tenth: 11).
        assert!((learned.budget[0] - 11.0).abs() < 1e-4, "{:?}", learned.budget);
    }

    #[test]
    fn the_gain_minimum() {
        let mut record = days("2026-09-01", 4, 1.0, Some(Outcome::AboutRight));
        // Fewer than five good days: the default, one good hour.
        assert_eq!(learn(&record, START, 28, after(&record)).minimum, GAIN_DEFAULT);
        // Good days with 2, 2.5, 3, 3.5, 4, 4.5 good hours: the 25th percentile, weighted toward recent days.
        record = days("2026-09-01", 6, 1.0, Some(Outcome::AboutRight));
        for (k, r) in record.iter_mut().enumerate() {
            r.good_hours = 2.0 + k as f32 * 0.5;
        }
        let learned = learn(&record, START, 28, after(&record));
        assert!(learned.minimum >= 2.5 && learned.minimum <= 3.5, "{}", learned.minimum);
        // Bounded: never above 4 good hours, never below half of one.
        let mut high = days("2026-09-01", 8, 1.0, Some(Outcome::AboutRight));
        high.iter_mut().for_each(|r| r.good_hours = 9.0);
        assert_eq!(learn(&high, START, 28, after(&high)).minimum, GAIN_HIGHEST);
        let mut low = days("2026-09-01", 8, 1.0, Some(Outcome::AboutRight));
        low.iter_mut().for_each(|r| r.good_hours = 0.1);
        assert_eq!(learn(&low, START, 28, after(&low)).minimum, GAIN_LOWEST);
        // A bad stretch never teaches a lower minimum: three too-much days, then good days with little.
        let mut stretch = days("2026-09-01", 6, 1.0, Some(Outcome::AboutRight));
        stretch.iter_mut().for_each(|r| r.good_hours = 3.0);
        let kept = learn(&stretch, START, 28, after(&stretch)).minimum;
        stretch.extend(days("2026-09-07", 3, 1.0, Some(Outcome::TooMuch)));
        let mut thin = days("2026-09-10", 3, 1.0, Some(Outcome::AboutRight));
        thin.iter_mut().for_each(|r| r.good_hours = 0.2);
        stretch.extend(thin);
        assert_eq!(learn(&stretch, START, 28, after(&stretch)).minimum, kept);
    }

    #[test]
    fn the_start_is_as_now_by_default() {
        // Eight hours of room: a full day of usual work and two heavy hours fit under the fill.
        let budget = start_budget(480, Start::AsNow);
        let full_day = 8.0 * 4.0 + 2.0 * 3.0;
        assert!((budget[TOTAL] * FILL - full_day).abs() < 1e-3);
        assert!((start_budget(480, Start::MuchLighter)[TOTAL] - budget[TOTAL] / 2.0).abs() < 1e-3);
        assert_eq!((Start::parse("lighter"), Start::parse("?")), (Start::Lighter, Start::AsNow));
    }

    #[test]
    fn day_records_from_sessions_completions_and_events() {
        let zone = TimeZone::UTC;
        let on = day("2026-10-05");
        let at = |h: i8| on.at(h, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let mut report = Task { uid: "report".into(), title: "Report".into(), demands: rated(Some(6), None, None, None, Some(4)), ..Task::default() };
        report.felt = vec![Felt { on: Some(on), demands: rated(Some(8), None, None, None, None) }];
        let quick = Task { uid: "quick".into(), title: "Quick".into(), estimate: 20, status: Status::Completed, completed: Some(at(15)), energy: "light".into(), ..Task::default() };
        let sessions = vec![Session { task: "report".into(), start: at(9), minutes: 90, kind: Some(Kind::Measured), ..Session::default() }, Session { project: "lumen".into(), start: at(11), minutes: 60, ..Session::default() }];
        let dentist = Occurrence { start: at(16), end: at(17), demands: rated(None, None, Some(7), Some(5), None), ..Occurrence::default() };
        let lunch = Occurrence { start: at(12), end: at(13), all_day: true, ..Occurrence::default() };
        let outcomes = BTreeMap::from([(on, Outcome::TooMuch)]);
        let tasks = vec![report, quick];
        let index = FeltIndex::of(&tasks);
        let records = day_records(&tasks, &sessions, &[dentist, lunch], &index, &|t| t.estimate.max(30), &outcomes, on, on, &zone);
        assert_eq!(records.len(), 1);
        let r = &records[0];
        // Report: felt that day 8 × 1.5 h (its forecast gain 4 kept); project time: usual 4 × 1 h; quick: light 2 × 0.5 h (its 20 min count 30); dentist: 7 × 1 h.
        assert_eq!(r.load[0], 12.0);
        assert_eq!(r.load[2], 7.0);
        assert_eq!(r.load[3], 5.0);
        assert_eq!(r.load[TOTAL], 12.0 + 4.0 + 1.0 + 7.0);
        assert_eq!(r.known, [true, false, true, true, true]);
        assert!((r.good_hours - 0.4 * 1.5).abs() < 1e-5 && r.gain_known);
        assert_eq!(r.outcome, Some(Outcome::TooMuch));
    }

    #[test]
    fn a_balance_in_words_only() {
        let tr = Translator::new("en");
        let learned = Learned { budget: [10.0; 5], start: [10.0; 5], cap: [None; 5], frozen: false, minimum: 1.0, days: 10, changes: Vec::new() };
        let record = DayRecord { date: day("2026-10-05"), load: [2.0, 12.0, 6.0, 0.0, 12.0], known: [true, true, true, false, true], good_hours: 0.2, gain_known: true, outcome: Some(Outcome::AboutRight) };
        let balance = balance(&record, &learned, None, &tr);
        let levels: Vec<(&str, &str)> = balance.costs.iter().map(|c| (c.scale, c.level)).collect();
        assert_eq!(levels, vec![("cognitive", "light"), ("emotional", "heavy"), ("anxiety", "usual"), ("body", ""), ("total", "heavy")]);
        assert_eq!(balance.gain.level, "below");
        let unknown = DayRecord { gain_known: false, ..record.clone() };
        assert_eq!(super::balance(&unknown, &learned, None, &tr).gain.level, "", "nothing carried a gain: not known");
        // A day said too much is never called light.
        let bad = DayRecord { outcome: Some(Outcome::TooMuch), ..record };
        assert_eq!(super::balance(&bad, &learned, None, &tr).costs[0].level, "usual");
        // Tomorrow lowered: said once, in words, no number.
        let lower = Learned { budget: [8.0; 5], ..learned.clone() };
        let said = super::balance(&bad, &learned, Some(&lower), &tr).said;
        assert_eq!(said.len(), 1);
        assert!(!said[0].chars().any(|c| c.is_ascii_digit()) && !said[0].contains('%'), "{said:?}");
    }

    #[test]
    fn heavy_events_lighten_the_days_around() {
        let zone = TimeZone::UTC;
        let at = |d: &str, h: i8| day(d).at(h, 0, 0, 0).to_zoned(zone.clone()).unwrap().timestamp().as_second();
        let court = Occurrence { start: at("2026-10-08", 10), end: at("2026-10-08", 12), demands: rated(None, Some(8), Some(9), None, None), ..Occurrence::default() };
        let coffee = Occurrence { start: at("2026-10-12", 10), end: at("2026-10-12", 11), demands: rated(Some(2), None, None, None, Some(7)), ..Occurrence::default() };
        let around = around_heavy(&[court, coffee], &zone, day("2026-10-06"));
        assert_eq!(around, BTreeSet::from([day("2026-10-07"), day("2026-10-09")]));
        let planning = Planning { limit: Some([10.0; 5]), lightened: around, ..Planning::default() };
        assert_eq!(planning.day_limit(day("2026-10-07"), 1.0).unwrap()[0], 10.0 * FILL * AROUND_HEAVY);
        assert_eq!(planning.day_limit(day("2026-10-08"), 0.6).unwrap()[0], 10.0 * FILL * 0.6);
        assert_eq!(Planning::default().day_limit(day("2026-10-08"), 1.0), None);
    }

    #[test]
    fn suggestions_from_your_own_well_rated_items_rotated() {
        let walk = felt_task("w", "Walk by the river", "", Demands::default(), &[("2026-10-01", rated(Some(1), None, None, Some(2), Some(8)))]);
        let music = felt_task("m", "Play the piano", "", Demands::default(), &[("2026-10-02", rated(Some(2), None, None, None, Some(7)))]);
        let party = felt_task("p", "Party", "", Demands::default(), &[("2026-10-03", rated(None, Some(6), None, Some(7), Some(9)))]);
        let meh = felt_task("x", "Television", "", Demands::default(), &[("2026-10-03", rated(Some(0), None, None, None, Some(4)))]);
        let tasks = vec![walk, music, party, meh];
        let index = FeltIndex::of(&tasks);
        let first = suggestion(&tasks, &index, day("2026-10-06")).unwrap();
        let second = suggestion(&tasks, &index, day("2026-10-07")).unwrap();
        assert_ne!(first, second, "rotated by day");
        for d in ["2026-10-06", "2026-10-07", "2026-10-08"] {
            let s = suggestion(&tasks, &index, day(d)).unwrap();
            assert!(s == "Walk by the river" || s == "Play the piano", "only light, well-rated items: {s}");
        }
        assert_eq!(suggestion(&[], &FeltIndex::default(), day("2026-10-06")), None);
    }
}
